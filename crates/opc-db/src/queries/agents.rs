use opc_core::domain::{Agent, AgentApiKey, CreateAgent, UpdateAgent};
use sqlx::PgPool;
use uuid::Uuid;

const AGENT_COLS: &str = "id, company_id, name, title, role, capabilities, adapter_type, adapter_config, monthly_budget_cents, current_month_spent_cents, status, manager_id, created_at, updated_at";

pub async fn list_agents(pool: &PgPool, company_id: Uuid) -> sqlx::Result<Vec<Agent>> {
    let q = format!(
        "SELECT {} FROM agents WHERE company_id = $1 ORDER BY name ASC",
        AGENT_COLS
    );
    sqlx::query_as::<_, Agent>(&q)
        .bind(company_id)
        .fetch_all(pool)
        .await
}

pub async fn get_agent(pool: &PgPool, id: Uuid) -> sqlx::Result<Option<Agent>> {
    let q = format!("SELECT {} FROM agents WHERE id = $1", AGENT_COLS);
    sqlx::query_as::<_, Agent>(&q)
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn create_agent(pool: &PgPool, input: &CreateAgent) -> sqlx::Result<Agent> {
    let q = format!(
        "INSERT INTO agents (company_id, name, title, role, capabilities, adapter_type, adapter_config, monthly_budget_cents, manager_id) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING {}",
        AGENT_COLS
    );
    sqlx::query_as::<_, Agent>(&q)
        .bind(input.company_id)
        .bind(&input.name)
        .bind(&input.title)
        .bind(&input.role)
        .bind(&input.capabilities)
        .bind(&input.adapter_type)
        .bind(&input.adapter_config)
        .bind(input.monthly_budget_cents.unwrap_or(0))
        .bind(input.manager_id)
        .fetch_one(pool)
        .await
}

pub async fn update_agent(
    pool: &PgPool,
    id: Uuid,
    input: &UpdateAgent,
) -> sqlx::Result<Option<Agent>> {
    let q = format!(
        "UPDATE agents SET name = COALESCE($2, name), title = COALESCE($3, title), role = COALESCE($4, role), capabilities = COALESCE($5, capabilities), adapter_type = COALESCE($6, adapter_type), adapter_config = COALESCE($7, adapter_config), monthly_budget_cents = COALESCE($8, monthly_budget_cents), manager_id = COALESCE($9, manager_id), updated_at = now() WHERE id = $1 RETURNING {}",
        AGENT_COLS
    );
    sqlx::query_as::<_, Agent>(&q)
        .bind(id)
        .bind(&input.name)
        .bind(&input.title)
        .bind(&input.role)
        .bind(&input.capabilities)
        .bind(&input.adapter_type)
        .bind(&input.adapter_config)
        .bind(input.monthly_budget_cents)
        .bind(input.manager_id)
        .fetch_optional(pool)
        .await
}

pub async fn update_agent_status(pool: &PgPool, id: Uuid, status: &str) -> sqlx::Result<()> {
    sqlx::query("UPDATE agents SET status = $2, updated_at = now() WHERE id = $1")
        .bind(id)
        .bind(status)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_agent(pool: &PgPool, id: Uuid) -> sqlx::Result<bool> {
    let result = sqlx::query("DELETE FROM agents WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn increment_agent_spending(
    pool: &PgPool,
    id: Uuid,
    cost_cents: i64,
) -> sqlx::Result<()> {
    sqlx::query("UPDATE agents SET current_month_spent_cents = current_month_spent_cents + $2, updated_at = now() WHERE id = $1")
        .bind(id)
        .bind(cost_cents)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn create_api_key(
    pool: &PgPool,
    agent_id: Uuid,
    company_id: Uuid,
    key_hash: &str,
    key_prefix: &str,
) -> sqlx::Result<AgentApiKey> {
    sqlx::query_as::<_, AgentApiKey>(
        "INSERT INTO agent_api_keys (agent_id, company_id, key_hash, key_prefix) VALUES ($1, $2, $3, $4) RETURNING id, agent_id, company_id, key_hash, key_prefix, last_used_at, created_at"
    )
    .bind(agent_id)
    .bind(company_id)
    .bind(key_hash)
    .bind(key_prefix)
    .fetch_one(pool)
    .await
}

pub async fn find_api_key_by_prefix(pool: &PgPool, prefix: &str) -> sqlx::Result<Vec<AgentApiKey>> {
    sqlx::query_as::<_, AgentApiKey>(
        "SELECT id, agent_id, company_id, key_hash, key_prefix, last_used_at, created_at FROM agent_api_keys WHERE key_prefix = $1"
    )
    .bind(prefix)
    .fetch_all(pool)
    .await
}

pub async fn update_api_key_last_used(pool: &PgPool, id: Uuid) -> sqlx::Result<()> {
    sqlx::query("UPDATE agent_api_keys SET last_used_at = now() WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
