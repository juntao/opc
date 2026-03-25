use opc_core::domain::HeartbeatRun;
use sqlx::PgPool;
use uuid::Uuid;

const HB_COLS: &str =
    "id, agent_id, issue_id, trigger_type, status, started_at, completed_at, error_message";

pub async fn create_heartbeat_run(
    pool: &PgPool,
    agent_id: Uuid,
    issue_id: Option<Uuid>,
    trigger_type: &str,
) -> sqlx::Result<HeartbeatRun> {
    let q = format!("INSERT INTO heartbeat_runs (agent_id, issue_id, trigger_type) VALUES ($1, $2, $3) RETURNING {}", HB_COLS);
    sqlx::query_as::<_, HeartbeatRun>(&q)
        .bind(agent_id)
        .bind(issue_id)
        .bind(trigger_type)
        .fetch_one(pool)
        .await
}

pub async fn complete_heartbeat_run(
    pool: &PgPool,
    id: Uuid,
    status: &str,
    error_message: Option<&str>,
) -> sqlx::Result<()> {
    sqlx::query("UPDATE heartbeat_runs SET status = $2, completed_at = now(), error_message = $3 WHERE id = $1")
        .bind(id)
        .bind(status)
        .bind(error_message)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_heartbeat_runs(
    pool: &PgPool,
    agent_id: Uuid,
    limit: i64,
) -> sqlx::Result<Vec<HeartbeatRun>> {
    let q = format!(
        "SELECT {} FROM heartbeat_runs WHERE agent_id = $1 ORDER BY started_at DESC LIMIT $2",
        HB_COLS
    );
    sqlx::query_as::<_, HeartbeatRun>(&q)
        .bind(agent_id)
        .bind(limit)
        .fetch_all(pool)
        .await
}
