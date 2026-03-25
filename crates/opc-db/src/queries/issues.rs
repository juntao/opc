use opc_core::domain::{CreateIssue, Issue, UpdateIssue};
use sqlx::PgPool;
use uuid::Uuid;

const ISSUE_COLS: &str = "id, company_id, project_id, parent_issue_id, title, description, status, priority, assignee_id, checked_out_by, checked_out_at, approved_by, approved_at, created_at, updated_at";

pub async fn list_issues(
    pool: &PgPool,
    company_id: Uuid,
    status: Option<&str>,
    assignee_id: Option<Uuid>,
    project_id: Option<Uuid>,
) -> sqlx::Result<Vec<Issue>> {
    let q = format!(
        "SELECT {} FROM issues WHERE company_id = $1 AND ($2::TEXT IS NULL OR status = $2) AND ($3::UUID IS NULL OR assignee_id = $3) AND ($4::UUID IS NULL OR project_id = $4) ORDER BY CASE priority WHEN 'urgent' THEN 0 WHEN 'high' THEN 1 WHEN 'medium' THEN 2 WHEN 'low' THEN 3 END, created_at DESC",
        ISSUE_COLS
    );
    sqlx::query_as::<_, Issue>(&q)
        .bind(company_id)
        .bind(status)
        .bind(assignee_id)
        .bind(project_id)
        .fetch_all(pool)
        .await
}

pub async fn get_issue(pool: &PgPool, id: Uuid) -> sqlx::Result<Option<Issue>> {
    let q = format!("SELECT {} FROM issues WHERE id = $1", ISSUE_COLS);
    sqlx::query_as::<_, Issue>(&q)
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn create_issue(pool: &PgPool, input: &CreateIssue) -> sqlx::Result<Issue> {
    let q = format!(
        "INSERT INTO issues (company_id, project_id, parent_issue_id, title, description, priority, assignee_id, status) VALUES ($1, $2, $3, $4, $5, $6, $7, CASE WHEN $7::UUID IS NOT NULL THEN 'todo' ELSE 'backlog' END) RETURNING {}",
        ISSUE_COLS
    );
    sqlx::query_as::<_, Issue>(&q)
        .bind(input.company_id)
        .bind(input.project_id)
        .bind(input.parent_issue_id)
        .bind(&input.title)
        .bind(&input.description)
        .bind(input.priority.as_deref().unwrap_or("medium"))
        .bind(input.assignee_id)
        .fetch_one(pool)
        .await
}

pub async fn update_issue(
    pool: &PgPool,
    id: Uuid,
    input: &UpdateIssue,
) -> sqlx::Result<Option<Issue>> {
    let q = format!(
        "UPDATE issues SET title = COALESCE($2, title), description = COALESCE($3, description), status = COALESCE($4, status), priority = COALESCE($5, priority), assignee_id = COALESCE($6, assignee_id), project_id = COALESCE($7, project_id), updated_at = now() WHERE id = $1 RETURNING {}",
        ISSUE_COLS
    );
    sqlx::query_as::<_, Issue>(&q)
        .bind(id)
        .bind(&input.title)
        .bind(&input.description)
        .bind(&input.status)
        .bind(&input.priority)
        .bind(input.assignee_id)
        .bind(input.project_id)
        .fetch_optional(pool)
        .await
}

pub async fn checkout_issue(
    pool: &PgPool,
    issue_id: Uuid,
    agent_id: Uuid,
) -> sqlx::Result<Option<Issue>> {
    let q = format!(
        "UPDATE issues SET checked_out_by = $2, checked_out_at = now(), status = 'in_progress', updated_at = now() WHERE id = $1 AND checked_out_by IS NULL AND status IN ('todo', 'approved', 'changes_requested') RETURNING {}",
        ISSUE_COLS
    );
    sqlx::query_as::<_, Issue>(&q)
        .bind(issue_id)
        .bind(agent_id)
        .fetch_optional(pool)
        .await
}

pub async fn checkin_issue(pool: &PgPool, issue_id: Uuid, agent_id: Uuid) -> sqlx::Result<bool> {
    let result = sqlx::query("UPDATE issues SET checked_out_by = NULL, checked_out_at = NULL, updated_at = now() WHERE id = $1 AND checked_out_by = $2")
        .bind(issue_id)
        .bind(agent_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn submit_issue(
    pool: &PgPool,
    issue_id: Uuid,
    agent_id: Uuid,
) -> sqlx::Result<Option<Issue>> {
    let q = format!(
        "UPDATE issues SET status = 'awaiting_approval', checked_out_by = NULL, checked_out_at = NULL, updated_at = now() WHERE id = $1 AND checked_out_by = $2 RETURNING {}",
        ISSUE_COLS
    );
    sqlx::query_as::<_, Issue>(&q)
        .bind(issue_id)
        .bind(agent_id)
        .fetch_optional(pool)
        .await
}

pub async fn get_agent_assignments(pool: &PgPool, agent_id: Uuid) -> sqlx::Result<Vec<Issue>> {
    let q = format!(
        "SELECT {} FROM issues WHERE assignee_id = $1 AND status IN ('todo', 'approved', 'changes_requested', 'in_progress') ORDER BY CASE status WHEN 'in_progress' THEN 0 WHEN 'changes_requested' THEN 1 WHEN 'approved' THEN 2 WHEN 'todo' THEN 3 END, CASE priority WHEN 'urgent' THEN 0 WHEN 'high' THEN 1 WHEN 'medium' THEN 2 WHEN 'low' THEN 3 END",
        ISSUE_COLS
    );
    sqlx::query_as::<_, Issue>(&q)
        .bind(agent_id)
        .fetch_all(pool)
        .await
}

pub async fn approve_issue(
    pool: &PgPool,
    issue_id: Uuid,
    approved_by: &str,
) -> sqlx::Result<Option<Issue>> {
    let q = format!(
        "UPDATE issues SET status = 'approved', approved_by = $2, approved_at = now(), updated_at = now() WHERE id = $1 AND status = 'awaiting_approval' RETURNING {}",
        ISSUE_COLS
    );
    sqlx::query_as::<_, Issue>(&q)
        .bind(issue_id)
        .bind(approved_by)
        .fetch_optional(pool)
        .await
}

pub async fn get_parent_chain(pool: &PgPool, issue_id: Uuid) -> sqlx::Result<Vec<Issue>> {
    let q = format!(
        "WITH RECURSIVE ancestors AS (SELECT {} FROM issues WHERE id = $1 UNION ALL SELECT i.id, i.company_id, i.project_id, i.parent_issue_id, i.title, i.description, i.status, i.priority, i.assignee_id, i.checked_out_by, i.checked_out_at, i.approved_by, i.approved_at, i.created_at, i.updated_at FROM issues i JOIN ancestors a ON a.parent_issue_id = i.id) SELECT {} FROM ancestors WHERE id != $1 ORDER BY created_at ASC",
        ISSUE_COLS,
        "id, company_id, project_id, parent_issue_id, title, description, status, priority, assignee_id, checked_out_by, checked_out_at, approved_by, approved_at, created_at, updated_at"
    );
    sqlx::query_as::<_, Issue>(&q)
        .bind(issue_id)
        .fetch_all(pool)
        .await
}

pub async fn get_children(pool: &PgPool, parent_id: Uuid) -> sqlx::Result<Vec<Issue>> {
    let q = format!(
        "SELECT {} FROM issues WHERE parent_issue_id = $1 ORDER BY created_at ASC",
        ISSUE_COLS
    );
    sqlx::query_as::<_, Issue>(&q)
        .bind(parent_id)
        .fetch_all(pool)
        .await
}

pub async fn reassign_issue(
    pool: &PgPool,
    issue_id: Uuid,
    new_agent_id: Uuid,
) -> sqlx::Result<Option<Issue>> {
    let q = format!(
        "UPDATE issues SET assignee_id = $2, status = 'todo', checked_out_by = NULL, checked_out_at = NULL, updated_at = now() WHERE id = $1 RETURNING {}",
        ISSUE_COLS
    );
    sqlx::query_as::<_, Issue>(&q)
        .bind(issue_id)
        .bind(new_agent_id)
        .fetch_optional(pool)
        .await
}

pub async fn update_issue_status(
    pool: &PgPool,
    issue_id: Uuid,
    status: &str,
) -> sqlx::Result<bool> {
    let result = sqlx::query("UPDATE issues SET status = $2, updated_at = now() WHERE id = $1")
        .bind(issue_id)
        .bind(status)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}
