pub mod error;
pub mod middleware;
pub mod routes;
pub mod sse;
pub mod state;

use axum::middleware as axum_mw;
use axum::routing::{get, post, put};
use axum::Router;
use state::AppState;
use tower_http::services::ServeDir;

/// Build the full application router (used by both main and tests).
pub fn build_app(state: AppState) -> Router {
    Router::new()
        // Public routes
        .route(
            "/login",
            get(routes::pages::login_page).post(routes::auth::login_post),
        )
        .route("/logout", get(routes::auth::logout))
        .route("/api/health", get(routes::health::health))
        // SSE events
        .route("/api/events", get(sse::event_stream))
        // Agent self-service API (API key auth)
        .nest("/api/agent", agent_api_routes(state.clone()))
        // Board user pages and API (session auth)
        .merge(authenticated_routes(state.clone()))
        // Static files
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state)
}

fn agent_api_routes(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/me", get(routes::agents::agent_me))
        .route("/assignments", get(routes::agents::agent_assignments))
        .route(
            "/issues/{id}/checkout",
            post(routes::agents::agent_checkout),
        )
        .route("/issues/{id}/checkin", post(routes::agents::agent_checkin))
        .route("/issues/{id}/submit", post(routes::agents::agent_submit))
        .route(
            "/issues/{id}/comments",
            get(routes::comments::api_list).post(routes::comments::api_create_agent),
        )
        .route("/issues", post(routes::agents::agent_create_issue))
        .route("/projects", post(routes::agents::agent_create_project))
        .route("/agents", get(routes::agents::agent_list_agents))
        .route(
            "/projects/{id}/updates",
            post(routes::agents::agent_post_project_update),
        )
        .layer(axum_mw::from_fn_with_state(state, middleware::agent_auth))
}

fn authenticated_routes(state: AppState) -> Router<AppState> {
    Router::new()
        // Pages
        .route("/", get(routes::pages::dashboard))
        .route("/agents", get(routes::pages::agents_page))
        .route("/agents/new", get(routes::pages::agent_new_page))
        .route("/agents/{id}", get(routes::pages::agent_detail_page))
        .route("/issues", get(routes::pages::issues_page))
        .route("/issues/new", get(routes::pages::issue_new_page))
        .route("/issues/{id}", get(routes::pages::issue_detail_page))
        .route("/approvals", get(routes::pages::approvals_page))
        .route("/approvals/{id}", get(routes::pages::approval_detail_page))
        .route("/projects", get(routes::pages::projects_page))
        .route("/projects/{id}", get(routes::pages::project_detail_page))
        // API
        .route(
            "/api/agents",
            get(routes::agents::api_list).post(routes::agents::api_create),
        )
        .route(
            "/api/agents/{id}",
            get(routes::agents::api_get)
                .put(routes::agents::api_update)
                .delete(routes::agents::api_delete),
        )
        .route(
            "/api/agents/{id}/keys",
            post(routes::agents::api_generate_key),
        )
        .route("/api/agents/{id}/invoke", post(routes::agents::api_invoke))
        .route("/api/agents/{id}/pause", post(routes::agents::api_pause))
        .route("/api/agents/{id}/resume", post(routes::agents::api_resume))
        .route(
            "/api/issues",
            get(routes::issues::api_list).post(routes::issues::api_create),
        )
        .route(
            "/api/issues/{id}",
            get(routes::issues::api_get).put(routes::issues::api_update),
        )
        .route("/api/issues/{id}/assign", put(routes::issues::api_assign))
        .route(
            "/api/issues/{id}/comments",
            get(routes::comments::api_list).post(routes::comments::api_create_human),
        )
        .route("/api/approvals", get(routes::approvals::api_list_pending))
        .route("/api/approvals/{id}", get(routes::approvals::api_get))
        .route(
            "/api/approvals/{id}/approve",
            post(routes::approvals::api_approve),
        )
        .route(
            "/api/approvals/{id}/request-changes",
            post(routes::approvals::api_request_changes),
        )
        .route(
            "/api/approvals/{id}/reject",
            post(routes::approvals::api_reject),
        )
        .route(
            "/api/approvals/{id}/reassign",
            post(routes::approvals::api_reassign),
        )
        .route(
            "/api/projects",
            get(routes::projects::api_list).post(routes::projects::api_create),
        )
        .route(
            "/api/projects/{id}",
            get(routes::projects::api_get)
                .put(routes::projects::api_update)
                .delete(routes::projects::api_delete),
        )
        .route(
            "/api/projects/{id}/approve",
            post(routes::projects::api_approve),
        )
        .layer(axum_mw::from_fn_with_state(state, middleware::board_auth))
}
