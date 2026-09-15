use crate::output::gap_decision_ledger::{
    GapDecisionLedgerInput, GapDecisionLedgerSourceKind, build_gap_decision_ledger_report,
    render_gap_decision_ledger_json,
};
use serde_json::Value;
use std::fs;

#[test]
fn check_output_python_bound_receiver_direct_card_is_agent_packet_eligible() -> Result<(), String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("system time: {error}"))?
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "ripr-python-bound-receiver-{}-{stamp}",
        std::process::id()
    ));
    let proof = (|| -> Result<String, String> {
        fs::create_dir_all(root.join("src"))
            .map_err(|error| format!("create source fixture: {error}"))?;
        fs::create_dir_all(root.join("tests"))
            .map_err(|error| format!("create test fixture: {error}"))?;
        fs::write(
            root.join("src/pricing.py"),
            "class Parser:\n    def parse(self, text):\n        if not text:\n            raise KeyError(\"empty\")\n        return int(text)\n",
        )
        .map_err(|error| format!("write source fixture: {error}"))?;
        fs::write(
            root.join("tests/test_pricing.py"),
            "from src.pricing import Parser\n\ndef test_parse_ok():\n    parser = Parser()\n    assert parser.parse(\"42\") == 42\n",
        )
        .map_err(|error| format!("write test fixture: {error}"))?;
        fs::write(
            root.join("diff.patch"),
            "diff --git a/src/pricing.py b/src/pricing.py\nindex 1111111..2222222 100644\n--- a/src/pricing.py\n+++ b/src/pricing.py\n@@ -1,5 +1,5 @@\n class Parser:\n     def parse(self, text):\n         if not text:\n-            raise ValueError(\"empty\")\n+            raise KeyError(\"empty\")\n         return int(text)\n",
        )
        .map_err(|error| format!("write diff fixture: {error}"))?;
        let config =
            crate::config::tests_only_parse("[languages]\nenabled = [\"rust\", \"python\"]\n")?;
        let output = crate::app::check_workspace_with_config(
            crate::CheckInput {
                root: root.clone(),
                base: None,
                diff_file: Some(root.join("diff.patch")),
                mode: crate::Mode::Draft,
                format: crate::OutputFormat::Json,
                include_unchanged_tests: true,
                perl_facts_path: None,
                suppression_policy: None,
                git_timeout: None,
                git_candidate: None,
            },
            &config,
        )?;
        crate::render_check(&output, &crate::OutputFormat::Json)
    })();
    let cleanup = fs::remove_dir_all(&root)
        .map_err(|error| format!("remove source fixture {}: {error}", root.display()));
    let check_output = proof?;
    cleanup?;

    let value: Value = serde_json::from_str(&check_output)
        .map_err(|error| format!("parse production check output: {error}"))?;
    let finding = value
        .get("findings")
        .and_then(Value::as_array)
        .and_then(|findings| findings.first())
        .ok_or_else(|| "production check output finding missing".to_string())?;
    if finding.get("oracle_alignment").and_then(Value::as_str) != Some("direct")
        || finding.get("alignment_reason").and_then(Value::as_str)
            != Some("strong_oracle_observes_owner_method_on_bound_receiver")
        || finding.get("python_repair_card").is_none()
    {
        return Err(format!(
            "source fixture did not produce bound-receiver repair card: {finding}"
        ));
    }

    let report = build_gap_decision_ledger_report(GapDecisionLedgerInput {
        root: "source-fixture/python-bound-receiver".to_string(),
        generated_at: "test".to_string(),
        source_kind: GapDecisionLedgerSourceKind::CheckOutput,
        records_path: "producer-shaped-bound-receiver.json".to_string(),
        records_json: Ok(check_output),
    });
    let ledger = render_gap_decision_ledger_json(&report)?;
    let ledger: Value = serde_json::from_str(&ledger)
        .map_err(|error| format!("parse gap decision ledger: {error}"))?;
    let record = ledger
        .get("records")
        .and_then(Value::as_array)
        .and_then(|records| records.first())
        .ok_or_else(|| "bound-receiver direct card did not produce a record".to_string())?;
    if record
        .get("projection_eligibility")
        .and_then(|projection| projection.get("agent_packet"))
        .and_then(|packet| packet.get("eligible"))
        .and_then(Value::as_bool)
        != Some(true)
    {
        return Err("producer-valid bound-receiver direct card was denied".to_string());
    }
    Ok(())
}
