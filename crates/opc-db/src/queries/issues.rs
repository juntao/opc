use opc_core::domain::{CreateIssue, Issue, UpdateIssue};
use sqlx::PgPool;
use uuid::Uuid;

const ISSUE_COLS: &str = "id, company_id, project_id, title, description, status, priority, assignee_id, checked_out_by, checked_out_at, approved_by, approved_at, created_at, updated_at";

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
        "INSERT INTO issues (company_id, project_id, title, description, priority, assignee_id, status) VALUES ($1, $2, $3, $4, $5, $6, CASE WHEN $6::UUID IS NOT NULL THEN 'todo' ELSE 'backlog' END) RETURNING {}",
        ISSUE_COLS
    );
    sqlx::query_as::<_, Issue>(&q)
        .bind(input.company_id)
        .bind(input.project_id)
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

/// Insert dependency edges: `issue_id` is blocked by each ID in `depends_on_ids`.
pub async fn add_dependencies(
    pool: &PgPool,
    issue_id: Uuid,
    depends_on_ids: &[Uuid],
) -> sqlx::Result<()> {
    for dep_id in depends_on_ids {
        sqlx::query("INSERT INTO issue_dependencies (issue_id, depends_on_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
            .bind(issue_id)
            .bind(dep_id)
            .execute(pool)
            .await?;
    }
    Ok(())
}

/// Get issues that block this issue (direct dependencies).
pub async fn get_dependencies(pool: &PgPool, issue_id: Uuid) -> sqlx::Result<Vec<Issue>> {
    let q = format!(
        "SELECT {} FROM issues WHERE id IN (SELECT depends_on_id FROM issue_dependencies WHERE issue_id = $1) ORDER BY created_at ASC",
        ISSUE_COLS
    );
    sqlx::query_as::<_, Issue>(&q)
        .bind(issue_id)
        .fetch_all(pool)
        .await
}

/// Get issues that are blocked BY this issue (downstream dependents).
pub async fn get_dependents(pool: &PgPool, issue_id: Uuid) -> sqlx::Result<Vec<Issue>> {
    let q = format!(
        "SELECT {} FROM issues WHERE id IN (SELECT issue_id FROM issue_dependencies WHERE depends_on_id = $1) ORDER BY created_at ASC",
        ISSUE_COLS
    );
    sqlx::query_as::<_, Issue>(&q)
        .bind(issue_id)
        .fetch_all(pool)
        .await
}

/// Check if ALL dependencies of an issue are resolved (status = 'done' or 'approved').
/// Returns true if the issue has zero dependencies OR all are resolved.
pub async fn are_all_dependencies_resolved(pool: &PgPool, issue_id: Uuid) -> sqlx::Result<bool> {
    let unresolved: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM issue_dependencies d JOIN issues i ON i.id = d.depends_on_id WHERE d.issue_id = $1 AND i.status NOT IN ('done', 'approved')",
    )
    .bind(issue_id)
    .fetch_one(pool)
    .await?;
    Ok(unresolved == 0)
}

/// Get all resolved (done/approved) issues in the transitive dependency graph.
/// Walks the DAG upward via recursive CTE.
pub async fn get_resolved_dependency_chain(
    pool: &PgPool,
    issue_id: Uuid,
) -> sqlx::Result<Vec<Issue>> {
    let q = format!(
        "WITH RECURSIVE dep_chain AS (SELECT depends_on_id AS id FROM issue_dependencies WHERE issue_id = $1 UNION SELECT d.depends_on_id FROM issue_dependencies d JOIN dep_chain dc ON dc.id = d.issue_id) SELECT {} FROM issues WHERE id IN (SELECT id FROM dep_chain) AND status IN ('done', 'approved') ORDER BY created_at ASC",
        ISSUE_COLS
    );
    sqlx::query_as::<_, Issue>(&q)
        .bind(issue_id)
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
