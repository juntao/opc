use crate::error::AppError;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::Json;
use opc_core::domain::{CreateComment, IssueComment, OpcEvent};
use opc_db::queries;
use serde::Deserialize;
use uuid::Uuid;

pub async fn api_list(
    State(state): State<AppState>,
    Path(issue_id): Path<Uuid>,
) -> Result<Json<Vec<IssueComment>>, AppError> {
    let comments = queries::comments::list_comments(&state.pool, issue_id).await?;
    Ok(Json(comments))
}

#[derive(Deserialize)]
pub struct AddCommentInput {
    pub body: String,
    pub author_name: Option<String>,
}

/// Human posting a comment on an issue.
pub async fn api_create_human(
    State(state): State<AppState>,
    Path(issue_id): Path<Uuid>,
    user: axum::Extension<opc_core::domain::BoardUser>,
    Json(input): Json<AddCommentInput>,
) -> Result<Json<IssueComment>, AppError> {
    let create = CreateComment {
        issue_id,
        author_type: "human".to_string(),
        author_id: user.id.to_string(),
        author_name: input.author_name.unwrap_or_else(|| user.username.clone()),
        body: input.body,
    };
    let comment = queries::comments::create_comment(&state.pool, &create).await?;

    state.event_bus.publish(OpcEvent::CommentAdded {
        issue_id,
        comment_id: comment.id,
        company_id: state.company_id,
    });

    Ok(Json(comment))
}

/// Agent posting a comment on an issue.
pub async fn api_create_agent(
    State(state): State<AppState>,
    Path(issue_id): Path<Uuid>,
    agent: axum::Extension<opc_core::domain::Agent>,
    Json(input): Json<AddCommentInput>,
) -> Result<Json<IssueComment>, AppError> {
    let create = CreateComment {
        issue_id,
        author_type: "agent".to_string(),
        author_id: agent.id.to_string(),
        author_name: agent.name.clone(),
        body: input.body,
    };
    let comment = queries::comments::create_comment(&state.pool, &create).await?;

    state.event_bus.publish(OpcEvent::CommentAdded {
        issue_id,
        comment_id: comment.id,
        company_id: state.company_id,
    });

    Ok(Json(comment))
}
