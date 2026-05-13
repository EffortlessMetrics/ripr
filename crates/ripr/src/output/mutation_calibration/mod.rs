//! Render advisory static/runtime calibration reports.
//!
//! `ripr calibrate cargo-mutants` imports already-produced cargo-mutants JSON
//! and joins it to a `repo-exposure-json` snapshot. Runtime mutation
//! vocabulary is intentionally isolated to this calibration report; static
//! RIPR outputs keep their evidence vocabulary.

#[cfg(test)]
use serde_json::Value;

mod parse;
mod render;
mod report;
mod types;

use parse::{parse_mutation_outcomes_json, parse_repo_exposure_static_seams};
pub(crate) use render::{render_mutation_calibration_json, render_mutation_calibration_md};
use report::build_mutation_calibration_report;
use types::*;

pub(crate) const MUTATION_CALIBRATION_SCHEMA_VERSION: &str = "0.1";

const STATIC_WITHOUT_RUNTIME_SAMPLE_LIMIT: usize = 50;
const AGREEMENT_SAMPLE_LIMIT: usize = 50;

pub(crate) fn mutation_calibration_report_from_json(
    repo_exposure_json: &str,
    mutants_json: &str,
) -> Result<MutationCalibrationReport, String> {
    let static_seams = parse_repo_exposure_static_seams(repo_exposure_json)?;
    let runtime_mutants = parse_mutation_outcomes_json(mutants_json)?;
    Ok(build_mutation_calibration_report(
        static_seams,
        runtime_mutants,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutation_calibration_summarizes_static_runtime_agreement() -> Result<(), String> {
        let static_seams = vec![
            static_seam("gap-runtime", "weakly_gripped", "src/pricing.rs", 10),
            static_seam("gap-clean", "weakly_gripped", "src/pricing.rs", 20),
            static_seam("clean-clean", "strongly_gripped", "src/pricing.rs", 30),
            static_seam("clean-gap", "strongly_gripped", "src/pricing.rs", 40),
            static_seam("gap-none", "ungripped", "src/pricing.rs", 50),
        ];
        let runtime_mutants = vec![
            runtime("m1", Some("gap-runtime"), None, None, "missed"),
            runtime("m2", Some("gap-clean"), None, None, "caught"),
            runtime("m3", Some("clean-clean"), None, None, "caught"),
            runtime("m4", Some("clean-gap"), None, None, "missed"),
            runtime("m5", None, Some("src/other.rs"), Some(99), "missed"),
        ];

        let report = build_mutation_calibration_report(static_seams, runtime_mutants);
        assert_eq!(report.agreement.static_gap_and_runtime_signal, 1);
        assert_eq!(report.agreement.static_gap_without_runtime_signal, 2);
        assert_eq!(report.agreement.static_clean_and_runtime_clean, 1);
        assert_eq!(report.agreement.runtime_signal_without_static_gap, 2);
        assert_eq!(report.missed_runtime_signals.len(), 2);
        assert_eq!(report.static_only_findings.len(), 2);

        let json = render_mutation_calibration_json(&report)?;
        assert!(json.contains(r#""schema_version": "0.1""#));
        assert!(json.contains(r#""static_gap_and_runtime_signal": 1"#));
        assert!(json.contains(r#""confidence_label": "supports_static_gap""#));
        assert!(json.contains(r#""confidence_label": "contradicts_static_gap""#));
        assert!(json.contains(r#""confidence_label": "supports_static_clean""#));
        assert!(json.contains(r#""confidence_label": "contradicts_static_clean""#));
        assert!(json.contains(r#""confidence_label": "runtime_only_signal""#));
        assert!(json.contains(r#""confidence_label": "no_runtime_data""#));

        let markdown = render_mutation_calibration_md(&report);
        assert!(markdown.contains("# ripr mutation calibration report"));
        assert!(markdown.contains("| static_gap_and_runtime_signal | 1 |"));
        assert!(markdown.contains("Confidence label"));
        assert!(markdown.contains("Runtime signals without static gaps"));
        assert!(markdown.contains("Static gaps without runtime signals"));
        Ok(())
    }

    #[test]
    fn mutation_calibration_joins_by_seam_id_then_file_line_and_keeps_ambiguous() {
        let static_seams = vec![
            static_seam("id-match", "weakly_gripped", "src/pricing.rs", 10),
            static_seam("line-a", "weakly_gripped", "src/pricing.rs", 20),
            static_seam("line-b", "ungripped", "src/pricing.rs", 30),
            static_seam("ambiguous-a", "ungripped", "src/ambiguous.rs", 40),
            static_seam("ambiguous-b", "ungripped", "src/ambiguous.rs", 40),
        ];
        let runtime_mutants = vec![
            runtime("m-id", Some("id-match"), None, None, "missed"),
            runtime("m-line", None, Some("src/pricing.rs"), Some(20), "missed"),
            runtime(
                "m-ambiguous",
                None,
                Some("src/ambiguous.rs"),
                Some(40),
                "missed",
            ),
        ];

        let report = build_mutation_calibration_report(static_seams, runtime_mutants);
        assert_eq!(report.matched.len(), 2);
        assert_eq!(report.matched[0].join_method, "seam_id");
        assert_eq!(report.matched[1].join_method, "file_line");
        assert_eq!(report.ambiguous_file_line.len(), 1);
        assert_eq!(report.ambiguous_file_line[0].candidates.len(), 2);
        assert!(report.unmatched_mutants.is_empty());
    }

    #[test]
    fn mutation_calibration_parses_repo_exposure_and_cargo_mutants_json() -> Result<(), String> {
        let repo = r#"{
  "schema_version": "0.2",
  "scope": "repo",
  "seams": [
    {
      "seam_id": "seam-a",
      "kind": "predicate_boundary",
      "file": ".\\src\\pricing.rs",
      "line": "42",
      "grip_class": "weakly_gripped",
      "related_tests": [
        {"oracle_kind": "exact_value", "oracle_strength": "weak"}
      ],
      "observed_values": [50, true],
      "missing_discriminators": [
        {"value": "threshold equality", "reason": "not observed"}
      ]
    }
  ]
}"#;
        let mutants = r#"{
  "outcomes": [
    {
      "id": "m1",
      "mutant": {
        "seam_id": "seam-a",
        "operator": "replace >= with >"
      },
      "outcome": "missed"
    }
  ]
}"#;

        let report = mutation_calibration_report_from_json(repo, mutants)?;
        assert_eq!(report.static_seams_total, 1);
        assert_eq!(report.mutants_total, 1);
        assert_eq!(report.matched.len(), 1);
        assert_eq!(report.matched[0].seam.file, "src/pricing.rs");
        assert_eq!(
            report.matched[0].mutation.mutation_operator,
            "replace >= with >"
        );
        Ok(())
    }

    #[test]
    fn mutation_calibration_parses_nested_runtime_locations_and_aliases() -> Result<(), String> {
        let repo = r#"{
  "schema_version": "0.2",
  "scope": "repo",
  "seams": [
    {
      "seam_id": "seam-nested",
      "kind": "predicate_boundary",
      "file": "src/pricing.rs",
      "line": 42,
      "grip_class": "weakly_gripped",
      "related_tests": [
        {"oracle_kind": "smoke", "oracle_strength": "smoke"}
      ],
      "observed_values": [],
      "missing_discriminators": [
        "scalar discriminator",
        {"value": "boundary value"}
      ]
    },
    {
      "seam_id": "seam-location",
      "kind": "predicate_boundary",
      "file": "src/location.rs",
      "line": 17,
      "grip_class": "weakly_gripped",
      "related_tests": [
        {"oracle_kind": "none", "oracle_strength": "none"}
      ],
      "observed_values": [],
      "missing_discriminators": []
    },
    {
      "seam_id": "seam-span",
      "kind": "predicate_boundary",
      "file": "src/span.rs",
      "line": 18,
      "grip_class": "weakly_gripped",
      "related_tests": [
        {"oracle_kind": "unknown", "oracle_strength": "custom"}
      ],
      "observed_values": [],
      "missing_discriminators": []
    }
  ]
}"#;
        let mutants = r#"[
  true,
  {
    "mutation": {
      "seamId": "seam-nested",
      "path": "./src/pricing.rs",
      "startLine": "42",
      "replacement": "replace >= with >"
    },
    "status": "missed"
  },
  {
    "location": {
      "file_name": "src/location.rs",
      "line_start": "17"
    },
    "mutator": "replace location",
    "result": "caught"
  },
  {
    "span": {
      "file_name": "src/span.rs",
      "start": {
        "line": "18"
      }
    },
    "operator": "replace span",
    "state": "not caught"
  }
]"#;

        let report = mutation_calibration_report_from_json(repo, mutants)?;
        assert_eq!(report.mutants_total, 3);
        assert_eq!(report.matched.len(), 3);
        assert!(
            report
                .matched
                .iter()
                .any(|record| record.join_method == "file_line"
                    && record.seam.seam_id == "seam-location")
        );
        assert!(
            report
                .matched
                .iter()
                .any(|record| record.mutation.mutation_operator == "replace >= with >")
        );
        assert!(
            report
                .matched
                .iter()
                .any(|record| record.mutation.runtime_outcome == "not caught")
        );

        let json = render_mutation_calibration_json(&report)?;
        assert!(json.contains("scalar discriminator"));
        assert!(json.contains("boundary value"));
        assert!(json.contains("not_caught"));
        Ok(())
    }

    #[test]
    fn mutation_calibration_renders_empty_ambiguous_unmatched_and_inconclusive()
    -> Result<(), String> {
        let empty_report = build_mutation_calibration_report(Vec::new(), Vec::new());
        let empty_markdown = render_mutation_calibration_md(&empty_report);
        assert!(empty_markdown.contains("| none | 0 |"));
        assert!(empty_markdown.contains("No runtime mutants matched static seams."));

        let static_seams = vec![
            static_seam("ambiguous-a", "ungripped", "src/ambiguous.rs", 40),
            static_seam("ambiguous-b", "ungripped", "src/ambiguous.rs", 40),
            static_seam("inconclusive", "weakly_gripped", "src/inconclusive.rs", 50),
        ];
        let runtime_mutants = vec![
            runtime(
                "m-ambiguous",
                None,
                Some("src/ambiguous.rs"),
                Some(40),
                "missed",
            ),
            runtime(
                "m-inconclusive",
                Some("inconclusive"),
                None,
                None,
                "skipped",
            ),
            MutationOutcomeRecord {
                mutant_id: Some("m-line-only".to_string()),
                seam_id: None,
                file: None,
                line: Some(77),
                mutation_operator: "replace line".to_string(),
                runtime_outcome: "missed".to_string(),
                duration: None,
                test_command: Some("cargo test targeted".to_string()),
            },
            MutationOutcomeRecord {
                mutant_id: Some("m-unknown".to_string()),
                seam_id: None,
                file: None,
                line: None,
                mutation_operator: "replace unknown".to_string(),
                runtime_outcome: "missed".to_string(),
                duration: None,
                test_command: None,
            },
        ];

        let report = build_mutation_calibration_report(static_seams, runtime_mutants);
        assert_eq!(report.matched.len(), 1);
        assert_eq!(report.ambiguous_file_line.len(), 1);
        assert_eq!(report.unmatched_mutants.len(), 2);
        assert_eq!(report.agreement.runtime_inconclusive, 2);
        assert!(
            report
                .static_only_findings
                .iter()
                .any(|record| record.reason
                    == "static gap seam matched only runtime-inconclusive labels")
        );

        let markdown = render_mutation_calibration_md(&report);
        assert!(markdown.contains("| `m-ambiguous` | src/ambiguous.rs:40 | missed |"));
        assert!(markdown.contains("| line 77 | replace line | missed | cargo test targeted |"));
        assert!(markdown.contains("| unknown | replace unknown | missed | unknown |"));

        let json = render_mutation_calibration_json(&report)?;
        let value: Value = serde_json::from_str(&json)
            .map_err(|err| format!("mutation calibration JSON should parse: {err}"))?;
        assert_eq!(
            value["ambiguous_file_line_matches"]
                .as_array()
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(
            value["ambiguous_file_line_matches"][0]["confidence_label"],
            "ambiguous_runtime_join"
        );
        assert_eq!(
            value["static_only_findings"][0]["confidence_label"],
            "no_runtime_data"
        );
        assert_eq!(value["unmatched_mutants"].as_array().map(Vec::len), Some(2));
        Ok(())
    }

    #[test]
    fn mutation_calibration_merges_mutants_and_outcomes_by_id() -> Result<(), String> {
        let repo = repo_json_for("seam-a", "weakly_gripped", "src/pricing.rs", 42);
        let mutants = r#"[
  {"mutants": [{"id": "m1", "file": "src/pricing.rs", "line": 42, "operator": "replace"}]},
  {"outcomes": [{"id": "m1", "outcome": "caught", "duration_ms": 10}]}
]"#;

        let report = mutation_calibration_report_from_json(&repo, mutants)?;
        assert_eq!(report.matched.len(), 1);
        assert_eq!(report.matched[0].mutation.runtime_outcome, "caught");
        assert_eq!(report.matched[0].mutation.duration.as_deref(), Some("10"));
        Ok(())
    }

    #[test]
    fn mutation_calibration_reports_are_advisory_and_structured() -> Result<(), String> {
        let report = mutation_calibration_report_from_json(
            &repo_json_for("seam-a", "weakly_gripped", "src/pricing.rs", 42),
            r#"[{"id":"m1","seam_id":"seam-a","outcome":"missed","operator":"replace"}]"#,
        )?;
        let json = render_mutation_calibration_json(&report)?;
        let value: Value = serde_json::from_str(&json)
            .map_err(|err| format!("mutation calibration JSON should parse: {err}"))?;
        assert_eq!(value["status"], "advisory");
        assert_eq!(value["metrics"]["matched_total"], 1);
        assert_eq!(
            value["agreement"]["static_gap_and_runtime_signal"],
            Value::from(1)
        );

        let markdown = render_mutation_calibration_md(&report);
        assert!(markdown.contains("Status: advisory"));
        assert!(markdown.contains("Runtime Outcome Counts"));
        Ok(())
    }

    fn repo_json_for(id: &str, grip_class: &str, file: &str, line: usize) -> String {
        format!(
            r#"{{
  "schema_version": "0.2",
  "scope": "repo",
  "seams": [
    {{
      "seam_id": "{id}",
      "kind": "predicate_boundary",
      "file": "{file}",
      "line": {line},
      "grip_class": "{grip_class}",
      "related_tests": [],
      "observed_values": [],
      "missing_discriminators": []
    }}
  ]
}}"#
        )
    }

    fn static_seam(id: &str, grip_class: &str, file: &str, line: usize) -> StaticSeamRecord {
        StaticSeamRecord {
            seam_id: id.to_string(),
            seam_kind: "predicate_boundary".to_string(),
            file: file.to_string(),
            line,
            seam_grip_class: grip_class.to_string(),
            oracle_kind: "exact_value".to_string(),
            oracle_strength: "unknown".to_string(),
            observed_values: Vec::new(),
            missing_discriminators: Vec::new(),
        }
    }

    fn runtime(
        id: &str,
        seam_id: Option<&str>,
        file: Option<&str>,
        line: Option<usize>,
        outcome: &str,
    ) -> MutationOutcomeRecord {
        MutationOutcomeRecord {
            mutant_id: Some(id.to_string()),
            seam_id: seam_id.map(str::to_string),
            file: file.map(str::to_string),
            line,
            mutation_operator: "replace".to_string(),
            runtime_outcome: outcome.to_string(),
            duration: None,
            test_command: None,
        }
    }
}
