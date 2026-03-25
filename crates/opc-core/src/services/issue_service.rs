use crate::domain::{IssueStatus, OpcEvent};
use crate::events::EventBus;
use anyhow::{bail, Result};
use uuid::Uuid;

/// Validates that a status transition is allowed.
pub fn validate_status_transition(from: &str, to: &str) -> Result<()> {
    let from_status = IssueStatus::parse(from);
    let to_status = IssueStatus::parse(to);

    let (Some(from_s), Some(to_s)) = (from_status, to_status) else {
        bail!("Invalid status value");
    };

    let allowed = match from_s {
        IssueStatus::Backlog => matches!(to_s, IssueStatus::Todo | IssueStatus::Cancelled),
        IssueStatus::Todo => matches!(
            to_s,
            IssueStatus::InProgress | IssueStatus::Backlog | IssueStatus::Cancelled
        ),
        IssueStatus::InProgress => matches!(
            to_s,
            IssueStatus::AwaitingApproval | IssueStatus::Blocked | IssueStatus::Cancelled
        ),
        IssueStatus::AwaitingApproval => matches!(
            to_s,
            IssueStatus::Approved | IssueStatus::ChangesRequested | IssueStatus::Cancelled
        ),
        IssueStatus::Approved => matches!(
            to_s,
            IssueStatus::Done | IssueStatus::InProgress | IssueStatus::Todo
        ),
        IssueStatus::ChangesRequested => {
            matches!(to_s, IssueStatus::InProgress | IssueStatus::Cancelled)
        }
        IssueStatus::InReview => matches!(
            to_s,
            IssueStatus::Done | IssueStatus::InProgress | IssueStatus::Cancelled
        ),
        IssueStatus::Blocked => matches!(
            to_s,
            IssueStatus::Todo | IssueStatus::InProgress | IssueStatus::Cancelled
        ),
        IssueStatus::Done => matches!(to_s, IssueStatus::Todo),
        IssueStatus::Cancelled => matches!(to_s, IssueStatus::Backlog),
    };

    if !allowed {
        bail!("Cannot transition from '{}' to '{}'", from, to);
    }

    Ok(())
}

/// Emit an issue status change event.
pub fn emit_status_change(
    event_bus: &EventBus,
    issue_id: Uuid,
    company_id: Uuid,
    old_status: &str,
    new_status: &str,
) {
    event_bus.publish(OpcEvent::IssueStatusChanged {
        issue_id,
        company_id,
        old_status: old_status.to_string(),
        new_status: new_status.to_string(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_transitions() {
        assert!(validate_status_transition("backlog", "todo").is_ok());
        assert!(validate_status_transition("todo", "in_progress").is_ok());
        assert!(validate_status_transition("in_progress", "awaiting_approval").is_ok());
        assert!(validate_status_transition("awaiting_approval", "approved").is_ok());
        assert!(validate_status_transition("awaiting_approval", "changes_requested").is_ok());
        assert!(validate_status_transition("changes_requested", "in_progress").is_ok());
        assert!(validate_status_transition("approved", "done").is_ok());
    }

    #[test]
    fn test_invalid_transitions() {
        assert!(validate_status_transition("backlog", "done").is_err());
        assert!(validate_status_transition("todo", "approved").is_err());
        assert!(validate_status_transition("done", "in_progress").is_err());
        assert!(validate_status_transition("in_progress", "done").is_err());
    }
}
