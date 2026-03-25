use opc_core::domain::CostEvent;
use opc_core::services::cost_service::CreateCostEvent;
use sqlx::PgPool;
use uuid::Uuid;

const COST_COLS: &str = "id, company_id, agent_id, issue_id, project_id, heartbeat_run_id, model, input_tokens, output_tokens, cost_cents, created_at";

pub async fn create_cost_event(pool: &PgPool, input: &CreateCostEvent) -> sqlx::Result<CostEvent> {
    let q = format!("INSERT INTO cost_events (company_id, agent_id, issue_id, project_id, heartbeat_run_id, model, input_tokens, output_tokens, cost_cents) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING {}", COST_COLS);
    sqlx::query_as::<_, CostEvent>(&q)
        .bind(input.company_id)
        .bind(input.agent_id)
        .bind(input.issue_id)
        .bind(input.project_id)
        .bind(input.heartbeat_run_id)
        .bind(&input.model)
        .bind(input.input_tokens)
        .bind(input.output_tokens)
        .bind(input.cost_cents)
        .fetch_one(pool)
        .await
}

pub async fn total_cost_by_agent(pool: &PgPool, agent_id: Uuid) -> sqlx::Result<i64> {
    let row: (i64,) =
        sqlx::query_as("SELECT COALESCE(SUM(cost_cents), 0) FROM cost_events WHERE agent_id = $1")
            .bind(agent_id)
            .fetch_one(pool)
            .await?;
    Ok(row.0)
}

pub async fn total_cost_by_company(pool: &PgPool, company_id: Uuid) -> sqlx::Result<i64> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COALESCE(SUM(cost_cents), 0) FROM cost_events WHERE company_id = $1",
    )
    .bind(company_id)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}
