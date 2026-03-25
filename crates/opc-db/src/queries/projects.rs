use opc_core::domain::{CreateProject, Project, UpdateProject};
use sqlx::PgPool;
use uuid::Uuid;

const PROJECT_COLS: &str = "id, company_id, name, description, repo_url, status, created_at, updated_at";

pub async fn list_projects(pool: &PgPool, company_id: Uuid) -> sqlx::Result<Vec<Project>> {
    let q = format!(
        "SELECT {} FROM projects WHERE company_id = $1 ORDER BY name ASC",
        PROJECT_COLS
    );
    sqlx::query_as::<_, Project>(&q)
        .bind(company_id)
        .fetch_all(pool)
        .await
}

pub async fn get_project(pool: &PgPool, id: Uuid) -> sqlx::Result<Option<Project>> {
    let q = format!("SELECT {} FROM projects WHERE id = $1", PROJECT_COLS);
    sqlx::query_as::<_, Project>(&q)
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn create_project(pool: &PgPool, input: &CreateProject) -> sqlx::Result<Project> {
    let q = format!(
        "INSERT INTO projects (company_id, name, description, repo_url) VALUES ($1, $2, $3, $4) RETURNING {}",
        PROJECT_COLS
    );
    sqlx::query_as::<_, Project>(&q)
        .bind(input.company_id)
        .bind(&input.name)
        .bind(&input.description)
        .bind(&input.repo_url)
        .fetch_one(pool)
        .await
}

pub async fn update_project(
    pool: &PgPool,
    id: Uuid,
    input: &UpdateProject,
) -> sqlx::Result<Option<Project>> {
    let q = format!("UPDATE projects SET name = COALESCE($2, name), description = COALESCE($3, description), repo_url = COALESCE($4, repo_url), status = COALESCE($5, status), updated_at = now() WHERE id = $1 RETURNING {}", PROJECT_COLS);
    sqlx::query_as::<_, Project>(&q)
        .bind(id)
        .bind(&input.name)
        .bind(&input.description)
        .bind(&input.repo_url)
        .bind(&input.status)
        .fetch_optional(pool)
        .await
}
