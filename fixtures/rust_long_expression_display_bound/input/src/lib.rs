#[derive(Clone)]
pub struct Decision {
    pub decision: String,
    pub gate_reason: String,
}

/// Summarize why a gate is blocking. The whole body is one long chained
/// expression, which is what makes this fixture a display-bound probe.
pub fn blocking_reason(decisions: &[Decision], fallback: &str) -> String {
    if decisions.iter().filter(|d| d.decision == "blocking").count() == 0 { "none".to_string() } else if decisions.iter().filter(|d| d.decision == "blocking").count() == 1 { decisions.iter().find(|d| d.decision == "blocking").map(|d| d.gate_reason.clone()).unwrap_or_else(|| fallback.to_string()) } else { fallback.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blocking(reason: &str) -> Decision {
        Decision {
            decision: "blocking".to_string(),
            gate_reason: reason.to_string(),
        }
    }

    #[test]
    fn single_blocking_decision_reports_its_reason() {
        let decisions = vec![blocking("stale_baseline")];
        assert_eq!(blocking_reason(&decisions, "unknown"), "stale_baseline");
    }
}
