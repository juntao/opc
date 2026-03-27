use opc_core::domain::{CreateProject, Issue, Project, UpdateProject};
use sqlx::PgPool;
use uuid::Uuid;

const PROJECT_COLS: &str =
    "id, company_id, name, description, repo_url, status, created_at, updated_at";

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

pub async fn delete_project(pool: &PgPool, id: Uuid) -> sqlx::Result<bool> {
    let result = sqlx::query("DELETE FROM projects WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

const ISSUE_COLS: &str = "id, company_id, project_id, title, description, status, priority, assignee_id, checked_out_by, checked_out_at, approved_by, approved_at, created_at, updated_at";

/// Get root-level backlog issues with assignees, ready to be activated.
/// Root = no entries in issue_dependencies (not blocked by anything).
pub async fn get_root_issues_for_activation(
    pool: &PgPool,
    project_id: Uuid,
) -> sqlx::Result<Vec<Issue>> {
    let q = format!(
        "SELECT {} FROM issues WHERE project_id = $1 AND id NOT IN (SELECT issue_id FROM issue_dependencies) AND assignee_id IS NOT NULL AND status = 'backlog' ORDER BY created_at ASC",
        ISSUE_COLS
    );
    sqlx::query_as::<_, Issue>(&q)
        .bind(project_id)
        .fetch_all(pool)
        .await
}
