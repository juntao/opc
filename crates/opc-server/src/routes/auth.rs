use crate::error::AppError;
use crate::state::AppState;
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::extract::State;
use axum::http::header::SET_COOKIE;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use opc_db::queries;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

pub async fn login_post(
    State(state): State<AppState>,
    Form(form): Form<LoginForm>,
) -> Result<Response, AppError> {
    let user = queries::users::get_user_by_username(&state.pool, &form.username)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Invalid credentials"))?;

    let parsed_hash =
        PasswordHash::new(&user.password_hash).map_err(|e| anyhow::anyhow!("Hash error: {}", e))?;

    Argon2::default()
        .verify_password(form.password.as_bytes(), &parsed_hash)
        .map_err(|_| anyhow::anyhow!("Invalid credentials"))?;

    // Set session cookie (simplified: user ID as session)
    let cookie = format!(
        "opc_session={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=86400",
        user.id
    );

    Ok(([(SET_COOKIE, cookie)], Redirect::to("/")).into_response())
}

pub async fn logout() -> Response {
    let cookie = "opc_session=; Path=/; HttpOnly; Max-Age=0";
    ([(SET_COOKIE, cookie.to_string())], Redirect::to("/login")).into_response()
}

/// Hash a password using argon2.
pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("Hash error: {}", e))?
        .to_string();
    Ok(hash)
}
