use opc_core::domain::Company;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn get_company(pool: &PgPool, id: Uuid) -> sqlx::Result<Option<Company>> {
    sqlx::query_as::<_, Company>(
        "SELECT id, name, description, mission, monthly_budget_cents, created_at, updated_at FROM companies WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn get_first_company(pool: &PgPool) -> sqlx::Result<Option<Company>> {
    sqlx::query_as::<_, Company>(
        "SELECT id, name, description, mission, monthly_budget_cents, created_at, updated_at FROM companies ORDER BY created_at ASC LIMIT 1"
    )
    .fetch_optional(pool)
    .await
}

pub async fn create_company(
    pool: &PgPool,
    name: &str,
    description: Option<&str>,
    mission: Option<&str>,
) -> sqlx::Result<Company> {
    sqlx::query_as::<_, Company>(
        "INSERT INTO companies (name, description, mission) VALUES ($1, $2, $3) RETURNING id, name, description, mission, monthly_budget_cents, created_at, updated_at"
    )
    .bind(name)
    .bind(description)
    .bind(mission)
    .fetch_one(pool)
    .await
}
