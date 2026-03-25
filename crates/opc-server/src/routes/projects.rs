use crate::error::AppError;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::Json;
use opc_core::domain::{CreateProject, OpcEvent, Project, UpdateProject};
use opc_db::queries;
use uuid::Uuid;

pub async fn api_list(State(state): State<AppState>) -> Result<Json<Vec<Project>>, AppError> {
    let projects = queries::projects::list_projects(&state.pool, state.company_id).await?;
    Ok(Json(projects))
}

pub async fn api_get(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Option<Project>>, AppError> {
    let project = queries::projects::get_project(&state.pool, id).await?;
    Ok(Json(project))
}

pub async fn api_create(
    State(state): State<AppState>,
    Json(mut input): Json<CreateProject>,
) -> Result<Json<Project>, AppError> {
    input.company_id = state.company_id;
    let project = queries::projects::create_project(&state.pool, &input).await?;
    Ok(Json(project))
}

pub async fn api_update(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateProject>,
) -> Result<Json<Option<Project>>, AppError> {
    let project = queries::projects::update_project(&state.pool, id, &input).await?;
    Ok(Json(project))
}

/// Approve a draft project: sets status to "active" and triggers root-level issues.
pub async fn api_approve(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Project>, AppError> {
    let project = queries::projects::get_project(&state.pool, id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Project not found"))?;

    if project.status != "draft" {
        return Err(anyhow::anyhow!("Project is not in draft status").into());
    }

    let update = UpdateProject {
        name: None,
        description: None,
        repo_url: None,
        status: Some("active".to_string()),
    };
    let project = queries::projects::update_project(&state.pool, id, &update)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Failed to update project"))?;

    state.event_bus.publish(OpcEvent::ProjectApproved {
        project_id: id,
        company_id: state.company_id,
    });

    Ok(Json(project))
}

/// Delete a project and all its issues (cascaded by FK).
pub async fn api_delete(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<bool>, AppError> {
    let deleted = queries::projects::delete_project(&state.pool, id).await?;
    Ok(Json(deleted))
}
