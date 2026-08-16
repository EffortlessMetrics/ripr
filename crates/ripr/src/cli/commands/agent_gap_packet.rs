use std::path::Path;

pub(super) fn render_agent_packet_from_gap_ledger(
    root: &Path,
    gap_ledger: &Path,
    gap_id: &str,
) -> Result<String, String> {
    crate::app::agent_gap_packet::render_agent_packet_from_gap_ledger(root, gap_ledger, gap_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn unique_command_test_dir(name: &str) -> PathBuf {
        let nanos = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            Ok(duration) => duration.as_nanos(),
            Err(_) => 0,
        };
        std::env::temp_dir().join(format!("ripr-{name}-{nanos}"))
    }

    #[test]
    fn agent_packet_gap_ledger_renders_without_analysis() -> Result<(), String> {
        let root = unique_command_test_dir("agent-packet-gap-ledger");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        let gap_ledger = root.join("gap-ledger.json");
        std::fs::write(
            &gap_ledger,
            r#"{"records":[{"gap_id":"gap:pr:pricing","canonical_gap_id":"gap:rust:pricing","kind":"MissingBoundaryAssertion","language":"rust","language_status":"stable","scope":"pr_local","evidence_class":"predicate_boundary","gap_state":"actionable","policy_state":"new","repairability":"repairable","anchor":{"file":"src/pricing.rs","line":42,"owner":"pricing::discount"},"repair_route":{"route_kind":"AddBoundaryAssertion","target_file":"tests/pricing.rs","assertion_shape":"assert_eq!(discount(100, 100), 90)","changed_behavior":"amount == threshold"},"verification_commands":["cargo xtask fixtures boundary_gap"],"receipt_command":"ripr outcome --before target/ripr/workflow/before.json --after target/ripr/workflow/after.json --out target/ripr/receipts/gap-pr-pricing.targeted-test-outcome.json","projection_eligibility":{"agent_packet":{"eligible":true,"reason":"bounded repair route"}}}]}"#,
        )
        .map_err(|err| format!("write gap ledger: {err}"))?;

        let rendered = render_agent_packet_from_gap_ledger(&root, &gap_ledger, "gap:rust:pricing")?;
        assert!(rendered.contains(r#""source": "gap_decision_ledger""#));
        assert!(rendered.contains(r#""gap_id": "gap:pr:pricing""#));
        assert!(rendered.contains(r#""repair_kind": "AddBoundaryAssertion""#));
        assert!(rendered.contains(r#""verify_command": "cargo xtask fixtures boundary_gap""#));
        assert!(
            !rendered.contains(r#""confidence""#),
            "gap packet should not expose generic confidence: {rendered}"
        );

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_packet_gap_ledger_reports_missing_and_ineligible_records() -> Result<(), String> {
        let root = unique_command_test_dir("agent-packet-gap-ledger-errors");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        let gap_ledger = root.join("gap-ledger.json");
        std::fs::write(
            &gap_ledger,
            r#"{"records":[{"gap_id":"gap:no-action","kind":"NoActionAlreadyObserved","language":"rust","language_status":"stable","scope":"pr_local","policy_state":"resolved","repairability":"no_action","repair_route":{"route_kind":"NoAction"},"verification_commands":["cargo xtask fixtures"],"projection_eligibility":{"agent_packet":{"eligible":false,"reason":"already_observed"}}}]}"#,
        )
        .map_err(|err| format!("write gap ledger: {err}"))?;

        assert_eq!(
            render_agent_packet_from_gap_ledger(&root, &gap_ledger, "gap:missing"),
            Err("agent packet gap_id gap:missing was not found".to_string())
        );
        assert_eq!(
            render_agent_packet_from_gap_ledger(&root, &gap_ledger, "gap:no-action"),
            Err(
                "agent packet gap_id gap:no-action is not agent-packet eligible: already_observed"
                    .to_string()
            )
        );

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }
}
