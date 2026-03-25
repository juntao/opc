mod error;
mod middleware;
mod routes;
mod sse;
mod state;

use axum::middleware as axum_mw;
use axum::routing::{get, post, put};
use axum::Router;
use opc_core::events::EventBus;
use state::AppState;
use std::sync::Arc;
use tower_http::services::ServeDir;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("opc=info".parse()?))
        .init();

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3100);

    let pg_port: u16 = std::env::var("PG_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(5433);

    // Start embedded PostgreSQL or use DATABASE_URL
    let database_url = if let Ok(url) = std::env::var("DATABASE_URL") {
        info!("Using external database from DATABASE_URL");
        url
    } else {
        info!("Starting embedded PostgreSQL...");
        let (_pg, url) = opc_db::embedded::start_embedded_postgres(None, pg_port).await?;
        // Leak the PgEmbed handle so it stays alive for the lifetime of the process
        std::mem::forget(_pg);
        url
    };

    // Create connection pool
    let pool = opc_db::create_pool(&database_url).await?;

    // Run migrations
    opc_db::migrate::run_migrations(&pool).await?;

    // Ensure default company and admin user exist
    let company = match opc_db::queries::companies::get_first_company(&pool).await? {
        Some(c) => c,
        None => {
            info!("Creating default company and admin user...");
            let c = opc_db::queries::companies::create_company(
                &pool,
                "My Company",
                Some("Default OPC company"),
                Some("Build great products with AI agents"),
            )
            .await?;

            // Create default admin user (password: admin)
            let password_hash = routes::auth::hash_password("admin")?;
            opc_db::queries::users::create_user(&pool, c.id, "admin", &password_hash, "owner")
                .await?;
            info!("Default admin user created (username: admin, password: admin)");

            c
        }
    };

    let event_bus = Arc::new(EventBus::default());
    let api_base_url = format!("http://localhost:{}", port);

    let state = AppState {
        pool: pool.clone(),
        event_bus: event_bus.clone(),
        company_id: company.id,
        api_base_url,
    };

    // Start event listener for agent triggers
    let listener_state = state.clone();
    tokio::spawn(async move {
        routes::heartbeat::start_event_listener(listener_state).await;
    });

    // Build router
    let app = Router::new()
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
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    info!("OPC server running at http://localhost:{}", port);
    info!("Login with username: admin, password: admin");

    axum::serve(listener, app).await?;
    Ok(())
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
            get(routes::projects::api_get).put(routes::projects::api_update),
        )
        .layer(axum_mw::from_fn_with_state(state, middleware::board_auth))
}
