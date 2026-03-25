use crate::error::AppError;
use crate::state::AppState;
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHasher, SaltString};
use argon2::Argon2;
use axum::extract::{Path, State};
use axum::Json;
use opc_core::domain::{Agent, CreateAgent, UpdateAgent};
use opc_db::queries;
use serde::Deserialize;
use uuid::Uuid;

// --- API Endpoints ---

pub async fn api_list(State(state): State<AppState>) -> Result<Json<Vec<Agent>>, AppError> {
    let agents = queries::agents::list_agents(&state.pool, state.company_id).await?;
    Ok(Json(agents))
}

pub async fn api_get(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Option<Agent>>, AppError> {
    let agent = queries::agents::get_agent(&state.pool, id).await?;
    Ok(Json(agent))
}

pub async fn api_create(
    State(state): State<AppState>,
    Json(mut input): Json<CreateAgent>,
) -> Result<Json<Agent>, AppError> {
    input.company_id = state.company_id;
    let agent = queries::agents::create_agent(&state.pool, &input).await?;

    // For OpenClaw agents, auto-generate an API key and store it in adapter_config
    if input.adapter_type == "openclaw" {
        let random_bytes: [u8; 24] = rand::random();
        let key_suffix = hex::encode(random_bytes);
        let prefix = &key_suffix[..8];
        let raw_key = format!("opc_{}{}", prefix, &key_suffix[8..]);

        let salt = SaltString::generate(&mut OsRng);
        let hash = Argon2::default()
            .hash_password(raw_key.as_bytes(), &salt)
            .map_err(|e| anyhow::anyhow!("Hash error: {}", e))?
            .to_string();

        queries::agents::create_api_key(&state.pool, agent.id, state.company_id, &hash, prefix)
            .await?;

        // Inject the raw key into adapter_config
        let mut config = input.adapter_config.clone();
        config["opc_api_key"] = serde_json::Value::String(raw_key);
        let update = UpdateAgent {
            name: None,
            title: None,
            role: None,
            capabilities: None,
            adapter_type: None,
            adapter_config: Some(config),
            monthly_budget_cents: None,
            manager_id: None,
        };
        let agent = queries::agents::update_agent(&state.pool, agent.id, &update)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Failed to update agent config"))?;
        return Ok(Json(agent));
    }

    Ok(Json(agent))
}

pub async fn api_update(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateAgent>,
) -> Result<Json<Option<Agent>>, AppError> {
    let agent = queries::agents::update_agent(&state.pool, id, &input).await?;
    Ok(Json(agent))
}

pub async fn api_delete(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<bool>, AppError> {
    let deleted = queries::agents::delete_agent(&state.pool, id).await?;
    Ok(Json(deleted))
}

/// Generate a new API key for an agent. Returns the raw key (only shown once).
pub async fn api_generate_key(
    State(state): State<AppState>,
    Path(agent_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Generate random key
    let random_bytes: [u8; 24] = rand::random();
    let key_suffix = hex::encode(random_bytes);
    let prefix = &key_suffix[..8];
    let raw_key = format!("opc_{}{}", prefix, &key_suffix[8..]);

    // Hash the key
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(raw_key.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("Hash error: {}", e))?
        .to_string();

    queries::agents::create_api_key(&state.pool, agent_id, state.company_id, &hash, prefix).await?;

    Ok(Json(serde_json::json!({
        "api_key": raw_key,
        "prefix": prefix,
        "note": "Save this key - it will not be shown again"
    })))
}

pub async fn api_pause(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<bool>, AppError> {
    queries::agents::update_agent_status(&state.pool, id, "paused").await?;
    Ok(Json(true))
}

pub async fn api_resume(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<bool>, AppError> {
    queries::agents::update_agent_status(&state.pool, id, "idle").await?;
    Ok(Json(true))
}

pub async fn api_invoke(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let agent = queries::agents::get_agent(&state.pool, id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Agent not found"))?;

    // Spawn heartbeat in background
    let pool = state.pool.clone();
    let event_bus = state.event_bus.clone();
    let api_base_url = state.api_base_url.clone();
    tokio::spawn(async move {
        if let Err(e) = opc_agents::heartbeat::execute_heartbeat(
            &pool,
            &event_bus,
            &agent,
            "manual",
            &api_base_url,
        )
        .await
        {
            tracing::error!("Heartbeat failed for agent {}: {}", agent.name, e);
        }
    });

    Ok(Json(serde_json::json!({"status": "invoked"})))
}

// --- Agent Self-Service (API key auth) ---

pub async fn agent_me(agent: axum::Extension<Agent>) -> Json<Agent> {
    Json(agent.0)
}

pub async fn agent_assignments(
    State(state): State<AppState>,
    agent: axum::Extension<Agent>,
) -> Result<Json<Vec<opc_core::domain::Issue>>, AppError> {
    let issues = queries::issues::get_agent_assignments(&state.pool, agent.id).await?;
    Ok(Json(issues))
}

pub async fn agent_checkout(
    State(state): State<AppState>,
    agent: axum::Extension<Agent>,
    Path(issue_id): Path<Uuid>,
) -> Result<Json<Option<opc_core::domain::Issue>>, AppError> {
    let issue = queries::issues::checkout_issue(&state.pool, issue_id, agent.id).await?;
    Ok(Json(issue))
}

pub async fn agent_checkin(
    State(state): State<AppState>,
    agent: axum::Extension<Agent>,
    Path(issue_id): Path<Uuid>,
) -> Result<Json<bool>, AppError> {
    let result = queries::issues::checkin_issue(&state.pool, issue_id, agent.id).await?;
    Ok(Json(result))
}

#[derive(Deserialize)]
pub struct AgentSubmitInput {
    pub summary: String,
    pub artifacts: Option<serde_json::Value>,
}

pub async fn agent_submit(
    State(state): State<AppState>,
    agent: axum::Extension<Agent>,
    Path(issue_id): Path<Uuid>,
    Json(input): Json<AgentSubmitInput>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Submit the issue
    let issue = queries::issues::submit_issue(&state.pool, issue_id, agent.id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Issue not checked out by this agent"))?;

    // Create approval request
    let approval_input = opc_core::domain::CreateApprovalRequest {
        issue_id: issue.id,
        company_id: agent.company_id,
        agent_id: agent.id,
        summary: input.summary,
        artifacts: input.artifacts,
    };
    let approval = queries::approvals::create_approval(&state.pool, &approval_input).await?;

    state
        .event_bus
        .publish(opc_core::domain::OpcEvent::ApprovalRequested {
            approval_id: approval.id,
            issue_id: issue.id,
            agent_id: agent.id,
            company_id: agent.company_id,
        });

    Ok(Json(serde_json::json!({
        "approval_id": approval.id,
        "status": "awaiting_approval"
    })))
}
