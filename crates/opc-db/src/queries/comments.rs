use opc_core::domain::{CreateComment, IssueComment};
use sqlx::PgPool;
use uuid::Uuid;

pub async fn list_comments(pool: &PgPool, issue_id: Uuid) -> sqlx::Result<Vec<IssueComment>> {
    sqlx::query_as::<_, IssueComment>(
        "SELECT id, issue_id, author_type, author_id, author_name, body, created_at FROM issue_comments WHERE issue_id = $1 ORDER BY created_at ASC"
    )
    .bind(issue_id)
    .fetch_all(pool)
    .await
}

pub async fn create_comment(pool: &PgPool, input: &CreateComment) -> sqlx::Result<IssueComment> {
    sqlx::query_as::<_, IssueComment>(
        "INSERT INTO issue_comments (issue_id, author_type, author_id, author_name, body) VALUES ($1, $2, $3, $4, $5) RETURNING id, issue_id, author_type, author_id, author_name, body, created_at"
    )
    .bind(input.issue_id)
    .bind(&input.author_type)
    .bind(&input.author_id)
    .bind(&input.author_name)
    .bind(&input.body)
    .fetch_one(pool)
    .await
}
