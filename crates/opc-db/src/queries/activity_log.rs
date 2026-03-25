use opc_core::domain::ActivityLogEntry;
use sqlx::PgPool;
use uuid::Uuid;

#[allow(clippy::too_many_arguments)]
pub async fn log_activity(
    pool: &PgPool,
    company_id: Uuid,
    actor_type: &str,
    actor_id: &str,
    action: &str,
    entity_type: &str,
    entity_id: Uuid,
    details: serde_json::Value,
) -> sqlx::Result<ActivityLogEntry> {
    sqlx::query_as::<_, ActivityLogEntry>(
        "INSERT INTO activity_log (company_id, actor_type, actor_id, action, entity_type, entity_id, details) VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id, company_id, actor_type, actor_id, action, entity_type, entity_id, details, created_at"
    )
    .bind(company_id)
    .bind(actor_type)
    .bind(actor_id)
    .bind(action)
    .bind(entity_type)
    .bind(entity_id)
    .bind(details)
    .fetch_one(pool)
    .await
}

pub async fn list_activity(
    pool: &PgPool,
    company_id: Uuid,
    limit: i64,
) -> sqlx::Result<Vec<ActivityLogEntry>> {
    sqlx::query_as::<_, ActivityLogEntry>(
        "SELECT id, company_id, actor_type, actor_id, action, entity_type, entity_id, details, created_at FROM activity_log WHERE company_id = $1 ORDER BY created_at DESC LIMIT $2"
    )
    .bind(company_id)
    .bind(limit)
    .fetch_all(pool)
    .await
}
