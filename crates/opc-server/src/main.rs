use opc_core::events::EventBus;
use opc_server::state::AppState;
use std::sync::Arc;
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
            let password_hash = opc_server::routes::auth::hash_password("admin")?;
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
        opc_server::routes::heartbeat::start_event_listener(listener_state).await;
    });

    // Build router
    let app = opc_server::build_app(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    info!("OPC server running at http://localhost:{}", port);
    info!("Login with username: admin, password: admin");

    axum::serve(listener, app).await?;
    Ok(())
}
