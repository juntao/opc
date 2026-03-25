use anyhow::{bail, Result};

/// Check if an agent has exceeded its monthly budget.
pub fn check_budget(current_spent_cents: i64, monthly_budget_cents: i64) -> BudgetStatus {
    if monthly_budget_cents <= 0 {
        return BudgetStatus::Unlimited;
    }

    let ratio = current_spent_cents as f64 / monthly_budget_cents as f64;

    if ratio >= 1.0 {
        BudgetStatus::Exceeded
    } else if ratio >= 0.8 {
        BudgetStatus::Warning
    } else {
        BudgetStatus::Ok
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BudgetStatus {
    Ok,
    Warning,
    Exceeded,
    Unlimited,
}

/// Validate that an agent can be invoked (not paused, terminated, or over budget).
pub fn validate_agent_invocable(status: &str, budget_status: &BudgetStatus) -> Result<()> {
    match status {
        "paused" => bail!("Agent is paused"),
        "terminated" => bail!("Agent is terminated"),
        _ => {}
    }

    if *budget_status == BudgetStatus::Exceeded {
        bail!("Agent has exceeded monthly budget");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_budget_check() {
        assert_eq!(check_budget(0, 10000), BudgetStatus::Ok);
        assert_eq!(check_budget(7999, 10000), BudgetStatus::Ok);
        assert_eq!(check_budget(8000, 10000), BudgetStatus::Warning);
        assert_eq!(check_budget(10000, 10000), BudgetStatus::Exceeded);
        assert_eq!(check_budget(0, 0), BudgetStatus::Unlimited);
    }

    #[test]
    fn test_agent_invocable() {
        assert!(validate_agent_invocable("idle", &BudgetStatus::Ok).is_ok());
        assert!(validate_agent_invocable("paused", &BudgetStatus::Ok).is_err());
        assert!(validate_agent_invocable("terminated", &BudgetStatus::Ok).is_err());
        assert!(validate_agent_invocable("idle", &BudgetStatus::Exceeded).is_err());
    }
}
