use anyhow::Result;
use sqlx::PgPool;
use tracing::info;

/// Run all SQL migrations in order.
pub async fn run_migrations(pool: &PgPool) -> Result<()> {
    // Create migrations tracking table if not exists
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS _migrations (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            applied_at TIMESTAMPTZ NOT NULL DEFAULT now()
        )
        "#,
    )
    .execute(pool)
    .await?;

    let migrations = vec![
        (
            "001_initial",
            include_str!("../../../migrations/001_initial.sql"),
        ),
        (
            "002_add_openclaw_adapter",
            include_str!("../../../migrations/002_add_openclaw_adapter.sql"),
        ),
        (
            "003_add_reassigned_approval_status",
            include_str!("../../../migrations/003_add_reassigned_approval_status.sql"),
        ),
        (
            "004_add_issue_repo_url",
            include_str!("../../../migrations/004_add_issue_repo_url.sql"),
        ),
        (
            "005_move_repo_url_to_projects",
            include_str!("../../../migrations/005_move_repo_url_to_projects.sql"),
        ),
        (
            "006_project_approval_and_updates",
            include_str!("../../../migrations/006_project_approval_and_updates.sql"),
        ),
        (
            "007_issue_dependencies",
            include_str!("../../../migrations/007_issue_dependencies.sql"),
        ),
    ];

    for (name, sql) in migrations {
        let applied: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM _migrations WHERE name = $1)")
                .bind(name)
                .fetch_one(pool)
                .await?;

        if !applied {
            info!("Applying migration: {}", name);
            sqlx::raw_sql(sql).execute(pool).await?;
            sqlx::query("INSERT INTO _migrations (name) VALUES ($1)")
                .bind(name)
                .execute(pool)
                .await?;
            info!("Migration {} applied successfully", name);
        }
    }

    Ok(())
}
