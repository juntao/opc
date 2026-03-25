use opc_core::domain::BoardUser;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn get_user_by_username(
    pool: &PgPool,
    username: &str,
) -> sqlx::Result<Option<BoardUser>> {
    sqlx::query_as::<_, BoardUser>(
        "SELECT id, company_id, username, password_hash, role, created_at FROM board_users WHERE username = $1"
    )
    .bind(username)
    .fetch_optional(pool)
    .await
}

pub async fn get_user(pool: &PgPool, id: Uuid) -> sqlx::Result<Option<BoardUser>> {
    sqlx::query_as::<_, BoardUser>(
        "SELECT id, company_id, username, password_hash, role, created_at FROM board_users WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn create_user(
    pool: &PgPool,
    company_id: Uuid,
    username: &str,
    password_hash: &str,
    role: &str,
) -> sqlx::Result<BoardUser> {
    sqlx::query_as::<_, BoardUser>(
        "INSERT INTO board_users (company_id, username, password_hash, role) VALUES ($1, $2, $3, $4) RETURNING id, company_id, username, password_hash, role, created_at"
    )
    .bind(company_id)
    .bind(username)
    .bind(password_hash)
    .bind(role)
    .fetch_one(pool)
    .await
}
