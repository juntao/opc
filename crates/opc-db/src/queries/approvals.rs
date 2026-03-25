use opc_core::domain::{ApprovalRequest, CreateApprovalRequest};
use sqlx::PgPool;
use uuid::Uuid;

const APPROVAL_COLS: &str = "id, issue_id, company_id, agent_id, status, summary, artifacts, reviewed_by, review_comment, created_at, resolved_at";

pub async fn list_pending_approvals(
    pool: &PgPool,
    company_id: Uuid,
) -> sqlx::Result<Vec<ApprovalRequest>> {
    let q = format!("SELECT {} FROM approval_requests WHERE company_id = $1 AND status = 'pending' ORDER BY created_at DESC", APPROVAL_COLS);
    sqlx::query_as::<_, ApprovalRequest>(&q)
        .bind(company_id)
        .fetch_all(pool)
        .await
}

pub async fn list_all_approvals(
    pool: &PgPool,
    company_id: Uuid,
) -> sqlx::Result<Vec<ApprovalRequest>> {
    let q = format!(
        "SELECT {} FROM approval_requests WHERE company_id = $1 ORDER BY created_at DESC",
        APPROVAL_COLS
    );
    sqlx::query_as::<_, ApprovalRequest>(&q)
        .bind(company_id)
        .fetch_all(pool)
        .await
}

pub async fn get_approval(pool: &PgPool, id: Uuid) -> sqlx::Result<Option<ApprovalRequest>> {
    let q = format!(
        "SELECT {} FROM approval_requests WHERE id = $1",
        APPROVAL_COLS
    );
    sqlx::query_as::<_, ApprovalRequest>(&q)
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn create_approval(
    pool: &PgPool,
    input: &CreateApprovalRequest,
) -> sqlx::Result<ApprovalRequest> {
    let artifacts = input.artifacts.clone().unwrap_or(serde_json::json!([]));
    let q = format!("INSERT INTO approval_requests (issue_id, company_id, agent_id, summary, artifacts) VALUES ($1, $2, $3, $4, $5) RETURNING {}", APPROVAL_COLS);
    sqlx::query_as::<_, ApprovalRequest>(&q)
        .bind(input.issue_id)
        .bind(input.company_id)
        .bind(input.agent_id)
        .bind(&input.summary)
        .bind(artifacts)
        .fetch_one(pool)
        .await
}

pub async fn resolve_approval(
    pool: &PgPool,
    id: Uuid,
    status: &str,
    reviewed_by: &str,
    review_comment: Option<&str>,
) -> sqlx::Result<Option<ApprovalRequest>> {
    let q = format!("UPDATE approval_requests SET status = $2, reviewed_by = $3, review_comment = $4, resolved_at = now() WHERE id = $1 AND status = 'pending' RETURNING {}", APPROVAL_COLS);
    sqlx::query_as::<_, ApprovalRequest>(&q)
        .bind(id)
        .bind(status)
        .bind(reviewed_by)
        .bind(review_comment)
        .fetch_optional(pool)
        .await
}

pub async fn count_pending(pool: &PgPool, company_id: Uuid) -> sqlx::Result<i64> {
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM approval_requests WHERE company_id = $1 AND status = 'pending'",
    )
    .bind(company_id)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}
