use anyhow::Result;
use pg_embed::pg_enums::PgAuthMethod;
use pg_embed::pg_fetch::{PgFetchSettings, PG_V15};
use pg_embed::postgres::{PgEmbed, PgSettings};
use std::path::PathBuf;
use std::time::Duration;
use tracing::info;

/// Default data directory for embedded PostgreSQL: `./db/` relative to the binary.
fn default_data_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("db")
}

/// Start an embedded PostgreSQL instance and return a connection string.
pub async fn start_embedded_postgres(
    data_dir: Option<PathBuf>,
    port: u16,
) -> Result<(PgEmbed, String)> {
    let data_dir = data_dir.unwrap_or_else(default_data_dir);

    info!(
        "Starting embedded PostgreSQL at {:?}, port {}",
        data_dir, port
    );

    let pg_settings = PgSettings {
        database_dir: data_dir,
        port,
        user: "opc".to_string(),
        password: "opc".to_string(),
        auth_method: PgAuthMethod::Plain,
        persistent: true,
        timeout: Some(Duration::from_secs(30)),
        migration_dir: None,
    };

    let fetch_settings = PgFetchSettings {
        version: PG_V15,
        ..Default::default()
    };

    let mut pg = PgEmbed::new(pg_settings, fetch_settings).await?;

    pg.setup().await?;
    pg.start_db().await?;

    if !pg.database_exists("opc").await? {
        pg.create_database("opc").await?;
    }

    let connection_string = format!("postgresql://opc:opc@localhost:{}/opc", port);

    info!("Embedded PostgreSQL running on port {}", port);

    Ok((pg, connection_string))
}
