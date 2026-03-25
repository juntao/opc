use crate::state::AppState;
use argon2::password_hash::PasswordHash;
use argon2::{Argon2, PasswordVerifier};
use axum::extract::{Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use opc_core::domain::Agent;
use opc_db::queries;

/// Extract agent from API key in Authorization header.
pub async fn agent_auth(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    let Some(api_key) = auth_header else {
        return Err(StatusCode::UNAUTHORIZED);
    };

    // Key format: opc_XXXXXXXX_YYYYYYYYYYYY
    // Prefix is first 12 chars (opc_ + 8 char prefix)
    if api_key.len() < 13 {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let prefix = &api_key[4..12]; // 8 chars after "opc_"

    let keys = queries::agents::find_api_key_by_prefix(&state.pool, prefix)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Verify hash
    let argon2 = Argon2::default();
    let mut authenticated_agent: Option<Agent> = None;

    for key_record in &keys {
        if PasswordHash::new(&key_record.key_hash)
            .ok()
            .and_then(|hash| argon2.verify_password(api_key.as_bytes(), &hash).ok())
            .is_some()
        {
            // Update last used
            let _ = queries::agents::update_api_key_last_used(&state.pool, key_record.id).await;

            // Fetch agent
            if let Ok(Some(agent)) =
                queries::agents::get_agent(&state.pool, key_record.agent_id).await
            {
                // Verify company scope
                if agent.company_id == state.company_id {
                    authenticated_agent = Some(agent);
                    break;
                }
            }
        }
    }

    let Some(agent) = authenticated_agent else {
        return Err(StatusCode::UNAUTHORIZED);
    };

    req.extensions_mut().insert(agent);
    Ok(next.run(req).await)
}

/// Extract board user session from cookie.
pub async fn board_auth(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Check for session cookie
    let session_cookie = req
        .headers()
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|c| {
                let c = c.trim();
                c.strip_prefix("opc_session=")
            })
        });

    let Some(session_id) = session_cookie else {
        // Redirect to login for page requests, 401 for API
        let path = req.uri().path();
        if path.starts_with("/api/") {
            return Err(StatusCode::UNAUTHORIZED);
        }
        return Ok(axum::response::Redirect::to("/login").into_response());
    };

    // Parse session ID as user ID (simplified session management)
    let user_id = uuid::Uuid::parse_str(session_id).map_err(|_| StatusCode::UNAUTHORIZED)?;

    let user = queries::users::get_user(&state.pool, user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;

    if user.company_id != state.company_id {
        return Err(StatusCode::UNAUTHORIZED);
    }

    req.extensions_mut().insert(user);
    Ok(next.run(req).await)
}
