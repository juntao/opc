use opc_core::domain::{CreateProjectUpdate, ProjectUpdate};
use sqlx::PgPool;
use uuid::Uuid;

const UPDATE_COLS: &str = "id, project_id, company_id, agent_id, issue_id, body, created_at";

pub async fn create_project_update(
    pool: &PgPool,
    input: &CreateProjectUpdate,
) -> sqlx::Result<ProjectUpdate> {
    let q = format!(
        "INSERT INTO project_updates (project_id, company_id, agent_id, issue_id, body) VALUES ($1, $2, $3, $4, $5) RETURNING {}",
        UPDATE_COLS
    );
    sqlx::query_as::<_, ProjectUpdate>(&q)
        .bind(input.project_id)
        .bind(input.company_id)
        .bind(input.agent_id)
        .bind(input.issue_id)
        .bind(&input.body)
        .fetch_one(pool)
        .await
}

pub async fn list_project_updates(
    pool: &PgPool,
    project_id: Uuid,
    limit: i64,
) -> sqlx::Result<Vec<ProjectUpdate>> {
    let q = format!(
        "SELECT {} FROM project_updates WHERE project_id = $1 ORDER BY created_at DESC LIMIT $2",
        UPDATE_COLS
    );
    sqlx::query_as::<_, ProjectUpdate>(&q)
        .bind(project_id)
        .bind(limit)
        .fetch_all(pool)
        .await
}
