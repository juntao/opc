use crate::domain::HeartbeatTrigger;

/// Determine which trigger should wake an agent based on an event.
pub fn trigger_from_event(event_type: &str) -> Option<HeartbeatTrigger> {
    match event_type {
        "assignment" => Some(HeartbeatTrigger::Assignment),
        "mention" => Some(HeartbeatTrigger::Mention),
        "approval" => Some(HeartbeatTrigger::Approval),
        "changes_requested" => Some(HeartbeatTrigger::ChangesRequested),
        "manual" => Some(HeartbeatTrigger::Manual),
        "schedule" => Some(HeartbeatTrigger::Schedule),
        _ => None,
    }
}
