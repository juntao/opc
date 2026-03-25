use opc_core::events::EventBus;
use std::sync::Arc;
use uuid::Uuid;

/// Shared application state passed to all route handlers.
#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::PgPool,
    pub event_bus: Arc<EventBus>,
    pub company_id: Uuid,
    pub api_base_url: String,
}
