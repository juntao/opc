use crate::state::AppState;
use axum::extract::State;
use axum::response::sse::{Event, Sse};
use futures::stream::Stream;
use opc_core::domain::OpcEvent;
use std::convert::Infallible;

/// SSE endpoint that broadcasts system events to the UI.
pub async fn event_stream(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut rx = state.event_bus.subscribe();

    let stream = async_stream::stream! {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let event_name = match &event {
                        OpcEvent::IssueCreated { .. } => "issue_created",
                        OpcEvent::IssueAssigned { .. } => "issue_assigned",
                        OpcEvent::IssueStatusChanged { .. } => "issue_status_changed",
                        OpcEvent::ApprovalRequested { .. } => "approval_requested",
                        OpcEvent::ApprovalResolved { .. } => "approval_resolved",
                        OpcEvent::AgentMentioned { .. } => "agent_mentioned",
                        OpcEvent::CommentAdded { .. } => "comment_added",
                        OpcEvent::HeartbeatCompleted { .. } => "heartbeat_completed",
                        OpcEvent::CostEvent { .. } => "cost_event",
                    };

                    if let Ok(data) = serde_json::to_string(&event) {
                        yield Ok(Event::default().event(event_name).data(data));
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    continue;
                }
                Err(_) => break,
            }
        }
    };

    Sse::new(stream)
}
