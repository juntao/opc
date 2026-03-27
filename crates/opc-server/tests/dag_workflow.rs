//! Integration tests for the DAG (blocked_by) dependency workflow.
//!
//! Tests the full lifecycle: create a project with a diamond dependency graph,
//! approve the project, have agents check out and submit issues one by one,
//! simulate human feedback (request changes + re-submit), and verify that
//! downstream issues are only activated when ALL their blockers are resolved.
//!
//! Diamond DAG:
//! ```
//!   A (root - no deps)
//!  / \
//! B   C  (each blocked_by: [A])
//!  \ /
//!   D    (blocked_by: [B, C] — fan-in)
//! ```
//!
//! Run with: cargo test -p opc-server --test dag_workflow

use axum::body::Body;
use http::Request;
use http_body_util::BodyExt;
use opc_core::events::EventBus;
use opc_server::state::AppState;
use serde_json::Value;
use std::sync::{Arc, OnceLock};
use tower::ServiceExt;
use uuid::Uuid;

// ─── Shared database ────────────────────────────────────────────────────
//
// All tests share a single embedded PostgreSQL instance. Each test creates
// its own company, user, and app state for isolation. This avoids port
// conflicts, duplicate PG downloads, and lets tests run in parallel.

/// Stores just the database URL so each test can create its own pool
/// on its own tokio runtime (sqlx 0.6 pools are runtime-bound).
static DB_URL: OnceLock<String> = OnceLock::new();

/// Start embedded PG once (or reuse from a previous run), return the database URL.
/// Each test creates its own pool from this URL on its own runtime.
fn get_database_url() -> String {
    DB_URL
        .get_or_init(|| {
            std::thread::spawn(|| {
                let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
                let url = rt.block_on(async {
                    let pg_port: u16 = std::env::var("PG_TEST_PORT")
                        .ok()
                        .and_then(|p| p.parse().ok())
                        .unwrap_or(15432);
                    let database_url =
                        format!("postgresql://opc:opc@localhost:{}/opc", pg_port);

                    // Try connecting to an existing PG first (from a previous test run)
                    let need_start =
                        sqlx::postgres::PgPoolOptions::new()
                            .max_connections(1)
                            .acquire_timeout(std::time::Duration::from_secs(2))
                            .connect(&database_url)
                            .await
                            .is_err();

                    if need_start {
                        let data_dir = std::env::temp_dir().join("opc_test_shared");
                        let (pg, _) =
                            opc_db::embedded::start_embedded_postgres(Some(data_dir), pg_port)
                                .await
                                .expect("Failed to start embedded PostgreSQL");
                        std::mem::forget(pg);
                    }

                    // Run migrations (idempotent)
                    let pool = opc_db::create_pool(&database_url)
                        .await
                        .expect("Failed to create pool");
                    opc_db::migrate::run_migrations(&pool)
                        .await
                        .expect("Failed to run migrations");
                    pool.close().await;

                    database_url
                });
                // Leak runtime so embedded PG process stays alive
                std::mem::forget(rt);
                url
            })
            .join()
            .expect("Database init thread panicked")
        })
        .clone()
}

/// Create a fresh pool on the current runtime.
async fn create_test_pool() -> sqlx::PgPool {
    let url = get_database_url();
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(20)
        .connect(&url)
        .await
        .expect("Failed to create test pool")
}

// ─── Test helpers ────────────────────────────────────────────────────────

/// Build a JSON request with board user session cookie.
fn board_request(method: &str, uri: &str, user_id: Uuid, body: Option<Value>) -> Request<Body> {
    let cookie = format!("opc_session={}", user_id);
    let builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("Cookie", cookie)
        .header("Content-Type", "application/json");

    match body {
        Some(json) => builder.body(Body::from(json.to_string())).unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    }
}

/// Build a JSON request with agent API key.
fn agent_request(method: &str, uri: &str, api_key: &str, body: Option<Value>) -> Request<Body> {
    let builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json");

    match body {
        Some(json) => builder.body(Body::from(json.to_string())).unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    }
}

/// Send a request to the app and return (status_code, body_json).
async fn send(app: &axum::Router, req: Request<Body>) -> (u16, Value) {
    let response = app.clone().oneshot(req).await.unwrap();
    let status = response.status().as_u16();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    (status, json)
}

/// Get issue status by ID via the board user API.
async fn get_issue_status(app: &axum::Router, user_id: Uuid, issue_id: Uuid) -> String {
    let req = board_request("GET", &format!("/api/issues/{}", issue_id), user_id, None);
    let (status, body) = send(app, req).await;
    assert_eq!(status, 200, "Failed to get issue: {:?}", body);
    body["issue"]["status"]
        .as_str()
        .unwrap_or("unknown")
        .to_string()
}

/// Per-test environment. Each test gets its own company, user, event bus,
/// and app instance — all sharing the same database pool.
struct TestEnv {
    app: axum::Router,
    #[allow(dead_code)]
    state: AppState,
    user_id: Uuid,
}

async fn setup_test_env() -> TestEnv {
    let pool = create_test_pool().await;

    // Each test gets a unique company for isolation
    let company = opc_db::queries::companies::create_company(
        &pool,
        &format!("TestCompany-{}", Uuid::new_v4()),
        Some("Integration test company"),
        None,
    )
    .await
    .expect("Failed to create company");

    let password_hash = opc_server::routes::auth::hash_password("testpass").unwrap();
    let user = opc_db::queries::users::create_user(
        &pool,
        company.id,
        &format!("admin_{}", &Uuid::new_v4().to_string()[..8]),
        &password_hash,
        "owner",
    )
    .await
    .expect("Failed to create user");

    let event_bus = Arc::new(EventBus::default());

    let state = AppState {
        pool: pool.clone(),
        event_bus: event_bus.clone(),
        company_id: company.id,
        api_base_url: "http://localhost:0".to_string(),
    };

    // Start event listener (handles DAG cascade on approval)
    let listener_state = state.clone();
    tokio::spawn(async move {
        opc_server::routes::heartbeat::start_event_listener(listener_state).await;
    });

    let app = opc_server::build_app(state.clone());

    TestEnv {
        app,
        state,
        user_id: user.id,
    }
}

/// Create an agent (paused, HTTP adapter) and generate an API key.
/// Returns (agent_id, raw_api_key).
async fn create_agent_with_key(
    app: &axum::Router,
    user_id: Uuid,
    name: &str,
    title: &str,
) -> (Uuid, String) {
    let req = board_request(
        "POST",
        "/api/agents",
        user_id,
        Some(serde_json::json!({
            "name": name,
            "title": title,
            "capabilities": "integration testing",
            "adapter_type": "http",
            "adapter_config": {"webhook_url": "http://127.0.0.1:1/noop"}
        })),
    );
    let (status, body) = send(app, req).await;
    assert_eq!(status, 200, "Failed to create agent {}: {:?}", name, body);
    let agent_id = Uuid::parse_str(body["id"].as_str().unwrap()).unwrap();

    // Pause agent so heartbeats don't try to execute the HTTP adapter
    let req = board_request(
        "POST",
        &format!("/api/agents/{}/pause", agent_id),
        user_id,
        None,
    );
    let (status, _) = send(app, req).await;
    assert_eq!(status, 200, "Failed to pause agent {}", name);

    // Generate API key
    let req = board_request(
        "POST",
        &format!("/api/agents/{}/keys", agent_id),
        user_id,
        None,
    );
    let (status, body) = send(app, req).await;
    assert_eq!(
        status, 200,
        "Failed to generate key for {}: {:?}",
        name, body
    );
    let api_key = body["api_key"].as_str().unwrap().to_string();

    (agent_id, api_key)
}

/// Agent checks out an issue. Asserts success.
async fn agent_checkout(app: &axum::Router, api_key: &str, issue_id: Uuid) -> Value {
    let req = agent_request(
        "POST",
        &format!("/api/agent/issues/{}/checkout", issue_id),
        api_key,
        None,
    );
    let (status, body) = send(app, req).await;
    assert_eq!(
        status, 200,
        "Checkout failed for issue {}: {:?}",
        issue_id, body
    );
    assert!(
        !body.is_null(),
        "Checkout returned null for issue {} — already checked out or wrong status",
        issue_id
    );
    body
}

/// Agent submits an issue. Returns {approval_id, status}.
async fn agent_submit(app: &axum::Router, api_key: &str, issue_id: Uuid, summary: &str) -> Value {
    let req = agent_request(
        "POST",
        &format!("/api/agent/issues/{}/submit", issue_id),
        api_key,
        Some(serde_json::json!({
            "summary": summary,
            "artifacts": null
        })),
    );
    let (status, body) = send(app, req).await;
    assert_eq!(
        status, 200,
        "Submit failed for issue {}: {:?}",
        issue_id, body
    );
    body
}

/// Human approves an approval request.
async fn human_approve(
    app: &axum::Router,
    user_id: Uuid,
    approval_id: Uuid,
    comment: Option<&str>,
) -> Value {
    let req = board_request(
        "POST",
        &format!("/api/approvals/{}/approve", approval_id),
        user_id,
        Some(serde_json::json!({ "comment": comment })),
    );
    let (status, body) = send(app, req).await;
    assert_eq!(
        status, 200,
        "Approve failed for approval {}: {:?}",
        approval_id, body
    );
    body
}

/// Human requests changes on an approval.
async fn human_request_changes(
    app: &axum::Router,
    user_id: Uuid,
    approval_id: Uuid,
    feedback: &str,
) -> Value {
    let req = board_request(
        "POST",
        &format!("/api/approvals/{}/request-changes", approval_id),
        user_id,
        Some(serde_json::json!({ "comment": feedback })),
    );
    let (status, body) = send(app, req).await;
    assert_eq!(
        status, 200,
        "Request changes failed for approval {}: {:?}",
        approval_id, body
    );
    body
}

/// Wait for the event listener to process events.
async fn wait_for_events() {
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
}

// ─── Tests ───────────────────────────────────────────────────────────────

/// Full diamond DAG workflow with human feedback loop.
///
///   A (root, Alice)
///  / \
/// B   C  (Bob, Alice — each blocked_by A)
///  \ /
///   D    (Bob — blocked_by B+C, fan-in)
///
/// Steps:
/// 1. Create project (draft) + 4 issues with diamond deps
/// 2. Approve project → A becomes "todo"
/// 3. Alice works on A, submits
/// 4. Human requests changes → Alice re-works, re-submits
/// 5. Human approves A → B and C become "todo", D stays backlog
/// 6. Bob completes B → D still backlog (C not done)
/// 7. Alice completes C → D becomes "todo" (fan-in: all deps resolved)
/// 8. Bob completes D → all done
#[tokio::test]
async fn test_diamond_dag_workflow() {
    let env = setup_test_env().await;
    let app = &env.app;
    let user_id = env.user_id;

    // --- Create agents ---
    let (alice_id, alice_key) = create_agent_with_key(app, user_id, "Alice", "Developer").await;
    let (bob_id, bob_key) = create_agent_with_key(app, user_id, "Bob", "QA Engineer").await;

    // --- Create project (draft) ---
    let req = board_request(
        "POST",
        "/api/projects",
        user_id,
        Some(serde_json::json!({
            "name": "Diamond DAG Test",
            "description": "Integration test for DAG dependency workflow",
            "repo_url": "https://github.com/test/repo.git"
        })),
    );
    let (status, project) = send(app, req).await;
    assert_eq!(status, 200);
    let project_id = Uuid::parse_str(project["id"].as_str().unwrap()).unwrap();
    assert_eq!(project["status"].as_str().unwrap(), "draft");

    // --- Create diamond DAG ---

    // Issue A: root
    let req = board_request(
        "POST",
        "/api/issues",
        user_id,
        Some(serde_json::json!({
            "title": "Write requirements spec",
            "description": "Document all project requirements.",
            "priority": "high",
            "project_id": project_id,
            "assignee_id": alice_id
        })),
    );
    let (_, issue_a) = send(app, req).await;
    let a_id = Uuid::parse_str(issue_a["id"].as_str().unwrap()).unwrap();

    // Issue B: blocked by A
    let req = board_request(
        "POST",
        "/api/issues",
        user_id,
        Some(serde_json::json!({
            "title": "Build frontend",
            "description": "Implement frontend from requirements.",
            "priority": "high",
            "project_id": project_id,
            "assignee_id": bob_id,
            "blocked_by": [a_id]
        })),
    );
    let (_, issue_b) = send(app, req).await;
    let b_id = Uuid::parse_str(issue_b["id"].as_str().unwrap()).unwrap();

    // Issue C: blocked by A
    let req = board_request(
        "POST",
        "/api/issues",
        user_id,
        Some(serde_json::json!({
            "title": "Build backend API",
            "description": "Implement backend API from requirements.",
            "priority": "medium",
            "project_id": project_id,
            "assignee_id": alice_id,
            "blocked_by": [a_id]
        })),
    );
    let (_, issue_c) = send(app, req).await;
    let c_id = Uuid::parse_str(issue_c["id"].as_str().unwrap()).unwrap();

    // Issue D: blocked by B AND C (fan-in)
    let req = board_request(
        "POST",
        "/api/issues",
        user_id,
        Some(serde_json::json!({
            "title": "End-to-end testing",
            "description": "Integration tests for frontend + backend.",
            "priority": "high",
            "project_id": project_id,
            "assignee_id": bob_id,
            "blocked_by": [b_id, c_id]
        })),
    );
    let (_, issue_d) = send(app, req).await;
    let d_id = Uuid::parse_str(issue_d["id"].as_str().unwrap()).unwrap();

    // --- Verify all backlog (draft project) ---
    assert_eq!(get_issue_status(app, user_id, a_id).await, "backlog");
    assert_eq!(get_issue_status(app, user_id, b_id).await, "backlog");
    assert_eq!(get_issue_status(app, user_id, c_id).await, "backlog");
    assert_eq!(get_issue_status(app, user_id, d_id).await, "backlog");

    // --- Verify dependency graph ---
    let req = board_request("GET", &format!("/api/issues/{}", d_id), user_id, None);
    let (_, detail) = send(app, req).await;
    assert_eq!(detail["blocked_by"].as_array().unwrap().len(), 2);

    let req = board_request("GET", &format!("/api/issues/{}", a_id), user_id, None);
    let (_, detail) = send(app, req).await;
    assert_eq!(detail["blocks"].as_array().unwrap().len(), 2);

    // === Step 1: Approve project ===
    let req = board_request(
        "POST",
        &format!("/api/projects/{}/approve", project_id),
        user_id,
        None,
    );
    let (status, proj) = send(app, req).await;
    assert_eq!(status, 200);
    assert_eq!(proj["status"].as_str().unwrap(), "active");

    wait_for_events().await;

    assert_eq!(get_issue_status(app, user_id, a_id).await, "todo");
    assert_eq!(get_issue_status(app, user_id, b_id).await, "backlog");
    assert_eq!(get_issue_status(app, user_id, c_id).await, "backlog");
    assert_eq!(get_issue_status(app, user_id, d_id).await, "backlog");

    // === Step 2: Alice works on A, submits ===
    agent_checkout(app, &alice_key, a_id).await;
    assert_eq!(get_issue_status(app, user_id, a_id).await, "in_progress");

    let result = agent_submit(app, &alice_key, a_id, "Initial requirements drafted.").await;
    let appr_a1 = Uuid::parse_str(result["approval_id"].as_str().unwrap()).unwrap();
    assert_eq!(
        get_issue_status(app, user_id, a_id).await,
        "awaiting_approval"
    );

    // === Step 3: Human requests changes ===
    human_request_changes(
        app,
        user_id,
        appr_a1,
        "Add performance requirements and error handling.",
    )
    .await;
    wait_for_events().await;
    assert_eq!(
        get_issue_status(app, user_id, a_id).await,
        "changes_requested"
    );

    // Alice re-works and re-submits
    agent_checkout(app, &alice_key, a_id).await;
    let result = agent_submit(
        app,
        &alice_key,
        a_id,
        "Updated: added perf targets and error handling specs.",
    )
    .await;
    let appr_a2 = Uuid::parse_str(result["approval_id"].as_str().unwrap()).unwrap();

    // === Step 4: Human approves A → B,C activate, D stays backlog ===
    human_approve(app, user_id, appr_a2, Some("Looks good!")).await;
    wait_for_events().await;

    assert_eq!(get_issue_status(app, user_id, a_id).await, "done");
    assert_eq!(get_issue_status(app, user_id, b_id).await, "todo");
    assert_eq!(get_issue_status(app, user_id, c_id).await, "todo");
    assert_eq!(get_issue_status(app, user_id, d_id).await, "backlog");

    // === Step 5: Bob completes B → D still blocked ===
    agent_checkout(app, &bob_key, b_id).await;
    let result = agent_submit(
        app,
        &bob_key,
        b_id,
        "Frontend built with responsive design.",
    )
    .await;
    let appr_b = Uuid::parse_str(result["approval_id"].as_str().unwrap()).unwrap();
    human_approve(app, user_id, appr_b, None).await;
    wait_for_events().await;

    assert_eq!(get_issue_status(app, user_id, b_id).await, "done");
    assert_eq!(get_issue_status(app, user_id, d_id).await, "backlog");

    // === Step 6: Alice completes C → D activates (fan-in) ===
    agent_checkout(app, &alice_key, c_id).await;
    let result = agent_submit(app, &alice_key, c_id, "REST API with auth and CRUD.").await;
    let appr_c = Uuid::parse_str(result["approval_id"].as_str().unwrap()).unwrap();
    human_approve(app, user_id, appr_c, Some("Backend approved.")).await;
    wait_for_events().await;

    assert_eq!(get_issue_status(app, user_id, c_id).await, "done");
    assert_eq!(
        get_issue_status(app, user_id, d_id).await,
        "todo",
        "D should activate after BOTH B and C are done (fan-in)"
    );

    // === Step 7: Bob completes D → all done ===
    agent_checkout(app, &bob_key, d_id).await;
    let result = agent_submit(
        app,
        &bob_key,
        d_id,
        "All integration tests passing end-to-end.",
    )
    .await;
    let appr_d = Uuid::parse_str(result["approval_id"].as_str().unwrap()).unwrap();
    human_approve(app, user_id, appr_d, Some("Project complete!")).await;
    wait_for_events().await;

    // === Verify final state ===
    assert_eq!(get_issue_status(app, user_id, a_id).await, "done");
    assert_eq!(get_issue_status(app, user_id, b_id).await, "done");
    assert_eq!(get_issue_status(app, user_id, c_id).await, "done");
    assert_eq!(get_issue_status(app, user_id, d_id).await, "done");

    // No pending approvals left
    let req = board_request("GET", "/api/approvals", user_id, None);
    let (_, approvals) = send(app, req).await;
    assert_eq!(approvals.as_array().unwrap().len(), 0);

    // Verify comments from feedback cycle exist
    let req = board_request("GET", &format!("/api/issues/{}", a_id), user_id, None);
    let (_, detail) = send(app, req).await;
    let comments = detail["comments"].as_array().unwrap();
    assert!(
        !comments.is_empty(),
        "Issue A should have comments from the change-request cycle"
    );
}

/// Fan-out: one root issue triggers three parallel children.
#[tokio::test]
async fn test_fan_out_parallel_activation() {
    let env = setup_test_env().await;
    let app = &env.app;
    let user_id = env.user_id;

    let (agent_id, agent_key) = create_agent_with_key(app, user_id, "Worker", "Developer").await;

    // Create project
    let req = board_request(
        "POST",
        "/api/projects",
        user_id,
        Some(serde_json::json!({
            "name": "Fan-out Test",
            "description": "One root fans out to three children"
        })),
    );
    let (_, project) = send(app, req).await;
    let project_id = Uuid::parse_str(project["id"].as_str().unwrap()).unwrap();

    // Root issue
    let req = board_request(
        "POST",
        "/api/issues",
        user_id,
        Some(serde_json::json!({
            "title": "Design architecture",
            "description": "Design the system.",
            "project_id": project_id,
            "assignee_id": agent_id
        })),
    );
    let (_, root) = send(app, req).await;
    let root_id = Uuid::parse_str(root["id"].as_str().unwrap()).unwrap();

    // Three children, all blocked by root
    let mut child_ids = Vec::new();
    for title in ["Build service A", "Build service B", "Build service C"] {
        let req = board_request(
            "POST",
            "/api/issues",
            user_id,
            Some(serde_json::json!({
                "title": title,
                "description": format!("Implement {}.", title),
                "project_id": project_id,
                "assignee_id": agent_id,
                "blocked_by": [root_id]
            })),
        );
        let (_, child) = send(app, req).await;
        child_ids.push(Uuid::parse_str(child["id"].as_str().unwrap()).unwrap());
    }

    // Approve project
    let req = board_request(
        "POST",
        &format!("/api/projects/{}/approve", project_id),
        user_id,
        None,
    );
    send(app, req).await;
    wait_for_events().await;

    // Root is todo, children are backlog
    assert_eq!(get_issue_status(app, user_id, root_id).await, "todo");
    for &cid in &child_ids {
        assert_eq!(get_issue_status(app, user_id, cid).await, "backlog");
    }

    // Complete root
    agent_checkout(app, &agent_key, root_id).await;
    let result = agent_submit(app, &agent_key, root_id, "Architecture designed.").await;
    let approval_id = Uuid::parse_str(result["approval_id"].as_str().unwrap()).unwrap();
    human_approve(app, user_id, approval_id, None).await;
    wait_for_events().await;

    // All three children should now be "todo" (fan-out activation)
    assert_eq!(get_issue_status(app, user_id, root_id).await, "done");
    for &cid in &child_ids {
        assert_eq!(
            get_issue_status(app, user_id, cid).await,
            "todo",
            "All children should activate when their single blocker is resolved"
        );
    }
}

/// Rejection cancels the issue.
#[tokio::test]
async fn test_rejection_cancels_issue() {
    let env = setup_test_env().await;
    let app = &env.app;
    let user_id = env.user_id;

    let (agent_id, agent_key) = create_agent_with_key(app, user_id, "Worker", "Developer").await;

    // Create project + issue
    let req = board_request(
        "POST",
        "/api/projects",
        user_id,
        Some(serde_json::json!({
            "name": "Rejection Test",
            "description": "Test that rejection cancels"
        })),
    );
    let (_, project) = send(app, req).await;
    let project_id = Uuid::parse_str(project["id"].as_str().unwrap()).unwrap();

    let req = board_request(
        "POST",
        "/api/issues",
        user_id,
        Some(serde_json::json!({
            "title": "Write code",
            "description": "Write some code.",
            "project_id": project_id,
            "assignee_id": agent_id
        })),
    );
    let (_, issue) = send(app, req).await;
    let issue_id = Uuid::parse_str(issue["id"].as_str().unwrap()).unwrap();

    // Approve project → work → submit
    let req = board_request(
        "POST",
        &format!("/api/projects/{}/approve", project_id),
        user_id,
        None,
    );
    send(app, req).await;
    wait_for_events().await;

    agent_checkout(app, &agent_key, issue_id).await;
    let result = agent_submit(app, &agent_key, issue_id, "Done.").await;
    let approval_id = Uuid::parse_str(result["approval_id"].as_str().unwrap()).unwrap();

    // Reject
    let req = board_request(
        "POST",
        &format!("/api/approvals/{}/reject", approval_id),
        user_id,
        Some(serde_json::json!({"comment": "Wrong approach."})),
    );
    let (status, _) = send(app, req).await;
    assert_eq!(status, 200);

    wait_for_events().await;
    assert_eq!(get_issue_status(app, user_id, issue_id).await, "cancelled");
}

/// Comments from both humans and agents appear in the thread.
#[tokio::test]
async fn test_agent_comments_visible_in_thread() {
    let env = setup_test_env().await;
    let app = &env.app;
    let user_id = env.user_id;

    let (_agent_id, agent_key) =
        create_agent_with_key(app, user_id, "Commenter", "Developer").await;

    // Create project + issue
    let req = board_request(
        "POST",
        "/api/projects",
        user_id,
        Some(serde_json::json!({
            "name": "Comments Test",
            "description": "Test comment thread"
        })),
    );
    let (_, project) = send(app, req).await;
    let project_id = Uuid::parse_str(project["id"].as_str().unwrap()).unwrap();

    let req = board_request(
        "POST",
        "/api/issues",
        user_id,
        Some(serde_json::json!({
            "title": "Task with comments",
            "description": "A task for testing comments.",
            "project_id": project_id,
            "assignee_id": _agent_id
        })),
    );
    let (_, issue) = send(app, req).await;
    let issue_id = Uuid::parse_str(issue["id"].as_str().unwrap()).unwrap();

    // Approve project so agent can interact
    let req = board_request(
        "POST",
        &format!("/api/projects/{}/approve", project_id),
        user_id,
        None,
    );
    send(app, req).await;
    wait_for_events().await;

    // Human posts a comment
    let req = board_request(
        "POST",
        &format!("/api/issues/{}/comments", issue_id),
        user_id,
        Some(serde_json::json!({"body": "Focus on error handling please."})),
    );
    let (status, _) = send(app, req).await;
    assert_eq!(status, 200);

    // Agent posts a comment
    let req = agent_request(
        "POST",
        &format!("/api/agent/issues/{}/comments", issue_id),
        &agent_key,
        Some(serde_json::json!({"body": "Got it, adding error handling now."})),
    );
    let (status, _) = send(app, req).await;
    assert_eq!(status, 200);

    // Verify both comments in thread
    let req = agent_request(
        "GET",
        &format!("/api/agent/issues/{}/comments", issue_id),
        &agent_key,
        None,
    );
    let (status, comments) = send(app, req).await;
    assert_eq!(status, 200);
    let arr = comments.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["author_type"].as_str().unwrap(), "human");
    assert_eq!(arr[1]["author_type"].as_str().unwrap(), "agent");
}
