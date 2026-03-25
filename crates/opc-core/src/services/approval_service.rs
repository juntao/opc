use crate::domain::{ApprovalStatus, OpcEvent};
use crate::events::EventBus;
use anyhow::{bail, Result};
use uuid::Uuid;

/// Validate that an approval resolution is valid.
pub fn validate_resolution(current_status: &str, new_status: &str) -> Result<()> {
    let current = ApprovalStatus::parse(current_status);
    let new = ApprovalStatus::parse(new_status);

    match (current, new) {
        (Some(ApprovalStatus::Pending), Some(ApprovalStatus::Approved)) => Ok(()),
        (Some(ApprovalStatus::Pending), Some(ApprovalStatus::ChangesRequested)) => Ok(()),
        (Some(ApprovalStatus::Pending), Some(ApprovalStatus::Rejected)) => Ok(()),
        _ => bail!(
            "Cannot resolve approval from '{}' to '{}'",
            current_status,
            new_status
        ),
    }
}

/// Emit approval resolution event and return the corresponding issue status.
pub fn emit_approval_resolved(
    event_bus: &EventBus,
    approval_id: Uuid,
    issue_id: Uuid,
    company_id: Uuid,
    status: &str,
) {
    event_bus.publish(OpcEvent::ApprovalResolved {
        approval_id,
        issue_id,
        company_id,
        status: status.to_string(),
    });
}

/// Map an approval resolution to the resulting issue status.
pub fn approval_to_issue_status(approval_status: &str) -> &'static str {
    match approval_status {
        "approved" => "approved",
        "changes_requested" => "changes_requested",
        "rejected" => "cancelled",
        _ => "awaiting_approval",
    }
}
