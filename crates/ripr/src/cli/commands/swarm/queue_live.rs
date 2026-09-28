include!("queue.rs");

pub(super) fn run_with_live_currentness(options: Options) -> Result<(), String> {
    ensure_command_root(&options.root, "swarm queue")?;
    let contents = std::fs::read_to_string(&options.gap_ledger).map_err(|err| {
        format!(
            "swarm queue --gap-ledger {} is invalid: read failed: {err}",
            options.gap_ledger.display()
        )
    })?;
    match render_from_gap_ledger_contents_with_live_currentness(&options, &contents)? {
        Some(rendered) => {
            print!("{rendered}");
            Ok(())
        }
        None => run(options),
    }
}

fn render_from_gap_ledger_contents_with_live_currentness(
    options: &Options,
    contents: &str,
) -> Result<Option<String>, String> {
    let source =
        output::gap_decision_ledger_live::parse_gap_record_source_with_provenance_json(contents)
            .map_err(|err| {
                format!(
                    "swarm queue --gap-ledger {} is invalid: {err}",
                    options.gap_ledger.display()
                )
            })?;
    if gap_ledger_root_status(&options.root, source.root.as_deref()) != GapLedgerRootStatus::Match {
        return Ok(None);
    }

    let currentness = output::agent_seam_packets::evaluate_gap_record_source_currentness(
        output::agent_seam_packets::GapRecordSourceInput {
            root: &options.root,
            gap_ledger_path: &options.gap_ledger,
            ledger_root: source.root.as_deref(),
            source_kind: source.source_kind.as_deref(),
            records_path: source.records_path.as_deref(),
            source_identity_error: source.source_identity_error.as_deref(),
            records: &source.records,
        },
    );
    let rendered = output::agent_seam_packets::render_agent_gap_record_queue_json_with_currentness(
        &output::outcome::display_path(&options.root),
        &output::outcome::display_path(&options.gap_ledger),
        &source.records,
        &options.language,
        options.top,
        &currentness,
    )?;
    Ok(Some(rendered))
}

#[cfg(test)]
mod live_currentness_tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn unique_command_test_dir(name: &str) -> PathBuf {
        let nanos = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            Ok(duration) => duration.as_nanos(),
            Err(_) => 0,
        };
        std::env::temp_dir().join(format!("ripr-{name}-{nanos}"))
    }

    fn python_swarm_queue_gap_ledger(root: &Path) -> String {
        serde_json::json!({
            "root": output::outcome::display_path(root),
            "generated_at": "unix_ms:1778240100000",
            "records": [{
                "gap_id": "gap:python:pricing-boundary",
                "source_currentness": "candidate_current",
                "canonical_gap_id": "gap:python:src/pricing.py:calculate_discount:predicate_boundary",
                "kind": "MissingBoundaryAssertion",
                "language": "python",
                "language_status": "preview",
                "scope": "repo",
                "evidence_class": "predicate_boundary",
                "gap_state": "actionable",
                "policy_state": "new",
                "repairability": "repairable",
                "anchor": {
                    "file": "src/pricing.py",
                    "line": 7,
                    "owner": "calculate_discount"
                },
                "repair_route": {
                    "route_kind": "AddBoundaryAssertion",
                    "target_file": "tests/test_pricing.py",
                    "assertion_shape": "assert calculate_discount(100, 100) == 90",
                    "changed_behavior": "amount >= threshold"
                },
                "verification_commands": ["pytest tests/test_pricing.py"],
                "receipt_command": "ripr agent receipt --verify-json target/ripr/workflow/verify.json --seam-id gap:python:pricing-boundary --test-changed tests/test_pricing.py",
                "projection_eligibility": {
                    "agent_packet": {
                        "eligible": true,
                        "reason": "bounded repair route"
                    }
                }
            }]
        })
        .to_string()
    }

    #[test]
    fn matching_legacy_ledger_is_visible_but_not_assignable() -> Result<(), String> {
        let root = unique_command_test_dir("swarm-queue-live-currentness-legacy");
        std::fs::create_dir_all(&root).map_err(|error| format!("create root: {error}"))?;
        let options = Options {
            root: root.clone(),
            gap_ledger: root.join("gap-ledger.json"),
            language: "python".to_string(),
            top: 10,
        };
        let rendered = render_from_gap_ledger_contents_with_live_currentness(
            &options,
            &python_swarm_queue_gap_ledger(&root),
        )?
        .ok_or_else(|| {
            "matching-root ledger unexpectedly delegated to legacy rendering".to_string()
        })?;
        let value = serde_json::from_str::<serde_json::Value>(&rendered)
            .map_err(|error| format!("live queue JSON should parse: {error}"))?;
        assert_eq!(
            value.get("status").and_then(serde_json::Value::as_str),
            Some("blocked")
        );
        assert!(
            value
                .get("packets")
                .and_then(serde_json::Value::as_array)
                .is_some_and(Vec::is_empty),
            "a not_evaluated source must never expose assignable packets: {rendered}"
        );
        let packet = value
            .get("blocked_review")
            .and_then(serde_json::Value::as_array)
            .and_then(|packets| packets.first())
            .ok_or_else(|| format!("missing review-only candidate: {rendered}"))?;
        assert_eq!(
            packet
                .get("queue_state")
                .and_then(serde_json::Value::as_str),
            Some("blocked_not_evaluated")
        );
        assert_eq!(
            packet
                .get("staleness_status")
                .and_then(serde_json::Value::as_str),
            Some("not_evaluated")
        );
        assert!(packet.get("packet_command_args").is_none());
        assert!(packet.get("verify_command").is_none());
        assert!(
            packet
                .get("refresh_commands")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|commands| !commands.is_empty()),
            "review-only candidates must carry their refresh route: {rendered}"
        );
        assert_eq!(
            value
                .get("summary")
                .and_then(|summary| summary.get("assignable_total"))
                .and_then(serde_json::Value::as_u64),
            Some(0)
        );
        std::fs::remove_dir_all(&root).map_err(|error| format!("remove root: {error}"))?;
        Ok(())
    }
}
