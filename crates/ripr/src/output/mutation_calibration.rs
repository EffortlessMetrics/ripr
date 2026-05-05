//! Calibration scaffold for comparing static seam classifications against
//! cargo-mutants mutation outcomes.
//!
//! This module imports cargo-mutants JSON output and maps mutations to seams,
//! producing advisory calibration reports. Runtime mutation vocabulary (killed,
//! survived, etc.) is isolated to calibration reports only; static seam reports
//! maintain the audit vocabulary (test_grip, missing_discriminator, etc.).
//!
//! Schema is documented in `docs/OUTPUT_SCHEMA.md` under `mutation-calibration.json`.

use crate::analysis::ClassifiedSeam;
use crate::analysis::seams::{SeamGripClass, SeamKind};
use crate::domain::{OracleKind, OracleStrength};
use std::collections::HashMap;

pub(crate) const MUTATION_CALIBRATION_SCHEMA_VERSION: &str = "0.1";

/// Runtime outcome of a single mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MutationOutcome {
    Killed,
    Survived,
    Timeout,
    Unchosen,
}

impl MutationOutcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Killed => "killed",
            Self::Survived => "survived",
            Self::Timeout => "timeout",
            Self::Unchosen => "unchosen",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "killed" => Some(Self::Killed),
            "survived" => Some(Self::Survived),
            "timeout" => Some(Self::Timeout),
            "unchosen" => Some(Self::Unchosen),
            _ => None,
        }
    }
}

/// A parsed mutation record from cargo-mutants.
#[derive(Debug, Clone)]
pub struct ParsedMutation {
    pub mutation_id: String,
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub mutation_operator: String,
    pub text: String,
    pub replacement: String,
    pub outcome: MutationOutcome,
    pub duration_ms: u64,
    pub test_command: String,
}

/// A mutation joined to a static seam (if matching).
#[derive(Debug, Clone)]
pub struct MatchedMutation {
    pub mutation_id: String,
    pub seam_id: String,
    pub seam_kind: SeamKind,
    pub grip_class: SeamGripClass,
    pub oracle_kind: Option<OracleKind>,
    pub oracle_strength: Option<OracleStrength>,
    pub observed_values_count: usize,
    pub missing_discriminators_count: usize,
    pub mutation_operator: String,
    pub outcome: MutationOutcome,
    pub duration_ms: u64,
    pub test_command: String,
}

/// An unmatched mutation (could not find corresponding seam).
#[derive(Debug, Clone)]
pub struct UnmatchedMutation {
    pub mutation_id: String,
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub mutation_operator: String,
    pub outcome: MutationOutcome,
    pub reason: String,
}

/// Calibration results: matched mutations, unmatched mutations, and metrics.
pub struct CalibrationResults {
    pub matched: Vec<MatchedMutation>,
    pub unmatched: Vec<UnmatchedMutation>,
    pub metrics: CalibrationMetrics,
}

#[derive(Debug, Default)]
pub struct CalibrationMetrics {
    pub matched_count: usize,
    pub unmatched_count: usize,
    pub outcome_distribution: HashMap<MutationOutcome, usize>,
    pub class_outcome_distribution: HashMap<SeamGripClass, HashMap<MutationOutcome, usize>>,
}

/// Parse cargo-mutants JSON output.
///
/// Expects the cargo-mutants `mutants.json` format with an array of mutation records.
pub fn parse_mutants_json(json_str: &str) -> Result<Vec<ParsedMutation>, String> {
    let value: serde_json::Value =
        serde_json::from_str(json_str).map_err(|e| format!("Failed to parse JSON: {}", e))?;

    let array = value.as_array().ok_or("Expected JSON array of mutations")?;

    let mut mutations = Vec::new();
    for (idx, item) in array.iter().enumerate() {
        let obj = item
            .as_object()
            .ok_or_else(|| format!("Mutation {} is not a JSON object", idx))?;

        let mutation_id = obj
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("Mutation {} missing 'id'", idx))?
            .to_string();

        let file = obj
            .get("file")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("Mutation {} missing 'file'", idx))?
            .to_string();

        let line = obj
            .get("line")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| format!("Mutation {} missing 'line'", idx))? as usize;

        let column =
            obj.get("column")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| format!("Mutation {} missing 'column'", idx))? as usize;

        let mutation_operator = obj
            .get("operator")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("Mutation {} missing 'operator'", idx))?
            .to_string();

        let text = obj
            .get("text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("Mutation {} missing 'text'", idx))?
            .to_string();

        let replacement = obj
            .get("replacement")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("Mutation {} missing 'replacement'", idx))?
            .to_string();

        let outcome_str = obj
            .get("outcome")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("Mutation {} missing 'outcome'", idx))?;

        let outcome = MutationOutcome::from_str(outcome_str)
            .ok_or_else(|| format!("Unknown outcome '{}' for mutation {}", outcome_str, idx))?;

        let duration_ms = obj
            .get("duration_ms")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| format!("Mutation {} missing 'duration_ms'", idx))?;

        let test_command = obj
            .get("test_command")
            .and_then(|v| v.as_str())
            .unwrap_or("(unknown)")
            .to_string();

        mutations.push(ParsedMutation {
            mutation_id,
            file,
            line,
            column,
            mutation_operator,
            text,
            replacement,
            outcome,
            duration_ms,
            test_command,
        });
    }

    Ok(mutations)
}

/// Join mutations to classified seams by location matching.
pub fn join_mutations_to_seams(
    mutations: Vec<ParsedMutation>,
    classified: &[ClassifiedSeam],
) -> CalibrationResults {
    let mut seam_index: HashMap<(String, usize), &ClassifiedSeam> = HashMap::new();

    for seam in classified {
        let file = seam.seam.file().to_string_lossy().to_string();
        let line = seam.seam.display_line();
        seam_index.insert((file, line), seam);
    }

    let mut matched = Vec::new();
    let mut unmatched = Vec::new();

    for mutation in mutations {
        if let Some(seam) = seam_index.get(&(mutation.file.clone(), mutation.line)) {
            let (oracle_kind, oracle_strength) =
                if let Some(related_test) = seam.evidence.related_tests.first() {
                    (
                        Some(related_test.oracle_kind.clone()),
                        Some(related_test.oracle_strength.clone()),
                    )
                } else {
                    (None, None)
                };

            let matched_mutation = MatchedMutation {
                mutation_id: mutation.mutation_id,
                seam_id: seam.seam.id().as_str().to_string(),
                seam_kind: seam.seam.kind(),
                grip_class: seam.class,
                oracle_kind,
                oracle_strength,
                observed_values_count: seam.evidence.observed_values.len(),
                missing_discriminators_count: seam.evidence.missing_discriminators.len(),
                mutation_operator: mutation.mutation_operator,
                outcome: mutation.outcome,
                duration_ms: mutation.duration_ms,
                test_command: mutation.test_command,
            };
            matched.push(matched_mutation);
        } else {
            let unmatched_mutation = UnmatchedMutation {
                mutation_id: mutation.mutation_id,
                file: mutation.file,
                line: mutation.line,
                column: mutation.column,
                mutation_operator: mutation.mutation_operator,
                outcome: mutation.outcome,
                reason: "Seam not found at this location".to_string(),
            };
            unmatched.push(unmatched_mutation);
        }
    }

    let metrics = calculate_metrics(&matched);

    CalibrationResults {
        matched,
        unmatched,
        metrics,
    }
}

fn calculate_metrics(matched: &[MatchedMutation]) -> CalibrationMetrics {
    let mut metrics = CalibrationMetrics::default();

    metrics.matched_count = matched.len();

    for mutation in matched {
        *metrics
            .outcome_distribution
            .entry(mutation.outcome)
            .or_insert(0) += 1;

        metrics
            .class_outcome_distribution
            .entry(mutation.grip_class)
            .or_insert_with(HashMap::new)
            .entry(mutation.outcome)
            .and_modify(|e| *e += 1)
            .or_insert(1);
    }

    metrics
}

/// Render calibration results to JSON.
pub(crate) fn render_calibration_json(results: &CalibrationResults) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "  \"schema_version\": \"{}\",\n",
        MUTATION_CALIBRATION_SCHEMA_VERSION
    ));
    out.push_str("  \"scope\": \"calibration\",\n");
    out.push_str("  \"note\": \"Advisory only. Runtime mutation vocabulary appears in calibration reports; static seam reports use audit vocabulary (test_grip, missing_discriminator, etc.)\",\n");

    out.push_str("  \"metrics\": {\n");
    out.push_str(&format!(
        "    \"matched_mutations\": {},\n",
        results.metrics.matched_count
    ));
    out.push_str(&format!(
        "    \"unmatched_mutations\": {}\n",
        results.metrics.unmatched_count
    ));
    out.push_str("  },\n");

    out.push_str("  \"matched_mutations\": [\n");
    for (idx, mutation) in results.matched.iter().enumerate() {
        push_matched_mutation_json(&mut out, mutation);
        if idx + 1 != results.matched.len() {
            out.push_str(",\n");
        } else {
            out.push('\n');
        }
    }
    out.push_str("  ],\n");

    out.push_str("  \"unmatched_mutations\": [\n");
    for (idx, mutation) in results.unmatched.iter().enumerate() {
        push_unmatched_mutation_json(&mut out, mutation);
        if idx + 1 != results.unmatched.len() {
            out.push_str(",\n");
        } else {
            out.push('\n');
        }
    }
    out.push_str("  ]\n");
    out.push_str("}\n");
    out
}

fn push_matched_mutation_json(out: &mut String, mutation: &MatchedMutation) {
    out.push_str("    {\n");
    out.push_str(&format!(
        "      \"mutation_id\": \"{}\",\n",
        escape_json(&mutation.mutation_id)
    ));
    out.push_str(&format!(
        "      \"seam_id\": \"{}\",\n",
        escape_json(&mutation.seam_id)
    ));
    out.push_str(&format!(
        "      \"seam_kind\": \"{}\",\n",
        mutation.seam_kind.as_str()
    ));
    out.push_str(&format!(
        "      \"grip_class\": \"{}\",\n",
        mutation.grip_class.as_str()
    ));
    if let Some(kind) = &mutation.oracle_kind {
        out.push_str(&format!("      \"oracle_kind\": \"{}\",\n", kind.as_str()));
    }
    if let Some(strength) = &mutation.oracle_strength {
        out.push_str(&format!(
            "      \"oracle_strength\": \"{}\",\n",
            strength.as_str()
        ));
    }
    out.push_str(&format!(
        "      \"observed_values_count\": {},\n",
        mutation.observed_values_count
    ));
    out.push_str(&format!(
        "      \"missing_discriminators_count\": {},\n",
        mutation.missing_discriminators_count
    ));
    out.push_str(&format!(
        "      \"mutation_operator\": \"{}\",\n",
        escape_json(&mutation.mutation_operator)
    ));
    out.push_str(&format!(
        "      \"outcome\": \"{}\",\n",
        mutation.outcome.as_str()
    ));
    out.push_str(&format!("      \"duration_ms\": {}", mutation.duration_ms));
    out.push_str("\n    }");
}

fn push_unmatched_mutation_json(out: &mut String, mutation: &UnmatchedMutation) {
    out.push_str("    {\n");
    out.push_str(&format!(
        "      \"mutation_id\": \"{}\",\n",
        escape_json(&mutation.mutation_id)
    ));
    out.push_str(&format!(
        "      \"file\": \"{}\",\n",
        escape_json(&mutation.file)
    ));
    out.push_str(&format!("      \"line\": {},\n", mutation.line));
    out.push_str(&format!("      \"column\": {},\n", mutation.column));
    out.push_str(&format!(
        "      \"mutation_operator\": \"{}\",\n",
        escape_json(&mutation.mutation_operator)
    ));
    out.push_str(&format!(
        "      \"outcome\": \"{}\",\n",
        mutation.outcome.as_str()
    ));
    out.push_str(&format!(
        "      \"reason\": \"{}\"\n",
        escape_json(&mutation.reason)
    ));
    out.push_str("    }");
}

fn escape_json(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}

/// Render calibration results to Markdown.
pub(crate) fn render_calibration_markdown(results: &CalibrationResults) -> String {
    let mut out = String::new();
    out.push_str("# Mutation Calibration Report\n\n");
    out.push_str("> **Advisory only.** Runtime mutation vocabulary (killed, survived, timeout, unchosen) appears in calibration reports. Static seam reports use audit vocabulary (test_grip, missing_discriminator, static_evidence, runtime_confirmation).\n\n");

    out.push_str("## Summary\n\n");
    out.push_str(&format!(
        "- **Matched mutations**: {}\n",
        results.metrics.matched_count
    ));
    out.push_str(&format!(
        "- **Unmatched mutations**: {}\n\n",
        results.metrics.unmatched_count
    ));

    out.push_str("## Outcome Distribution\n\n");
    out.push_str("| Outcome | Count |\n");
    out.push_str("|---------|-------|\n");

    let mut outcomes: Vec<_> = results.metrics.outcome_distribution.iter().collect();
    outcomes.sort_by_key(|(outcome, _)| outcome.as_str());

    for (outcome, count) in outcomes {
        out.push_str(&format!("| {} | {} |\n", outcome.as_str(), count));
    }
    out.push('\n');

    if !results.metrics.class_outcome_distribution.is_empty() {
        out.push_str("## Outcome by Seam Grip Class\n\n");
        for class in SeamGripClass::ALL {
            if let Some(outcomes) = results.metrics.class_outcome_distribution.get(&class) {
                out.push_str(&format!("### {}\n\n", class.as_str()));
                out.push_str("| Outcome | Count |\n");
                out.push_str("|---------|-------|\n");

                let mut outcome_vec: Vec<_> = outcomes.iter().collect();
                outcome_vec.sort_by_key(|(outcome, _)| outcome.as_str());

                for (outcome, count) in outcome_vec {
                    out.push_str(&format!("| {} | {} |\n", outcome.as_str(), count));
                }
                out.push('\n');
            }
        }
    }

    out.push_str("## Unmatched Mutations\n\n");
    if results.unmatched.is_empty() {
        out.push_str("All mutations were matched to seams.\n");
    } else {
        out.push_str(&format!(
            "Found {} unmatched mutations:\n\n",
            results.unmatched.len()
        ));
        for mutation in &results.unmatched {
            out.push_str(&format!(
                "- `{}` at {}:{} — {}\n",
                mutation.mutation_id, mutation.file, mutation.line, mutation.reason
            ));
        }
    }
    out.push('\n');

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mutants_json_valid() -> Result<(), String> {
        let json = r#"[
            {
                "id": "mutant_1",
                "file": "src/lib.rs",
                "line": 42,
                "column": 10,
                "operator": "binary_operator",
                "text": "+",
                "replacement": "-",
                "outcome": "killed",
                "duration_ms": 1500,
                "test_command": "cargo test"
            }
        ]"#;

        let mutations = parse_mutants_json(json)?;
        assert_eq!(mutations.len(), 1);
        assert_eq!(mutations[0].mutation_id, "mutant_1");
        assert_eq!(mutations[0].outcome, MutationOutcome::Killed);
        Ok(())
    }

    #[test]
    fn parse_mutants_json_invalid_format() {
        let json = "{ \"not\": \"array\" }";
        assert!(parse_mutants_json(json).is_err());
    }

    #[test]
    fn mutation_outcome_round_trip() {
        for outcome in [
            MutationOutcome::Killed,
            MutationOutcome::Survived,
            MutationOutcome::Timeout,
            MutationOutcome::Unchosen,
        ] {
            let s = outcome.as_str();
            assert_eq!(MutationOutcome::from_str(s), Some(outcome));
        }
    }
}
