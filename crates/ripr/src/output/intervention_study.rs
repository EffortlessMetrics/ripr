//! Deterministic JSON and Markdown projections for a preregistered
//! intervention study (RIPR-SPEC-0216 / #4649).
//!
//! Projections are derived from one validated semantic object. This module
//! does not execute the study, grade attempts, or claim intervention value.

use sha2::{Digest, Sha256};

use crate::domain::{
    InterventionStudyError, RiprInterventionStudyV1, intervention_study_codes as codes,
};
use crate::output::json;

/// Compute the protocol digest over the canonical semantic payload.
pub(crate) fn protocol_digest(
    study: &RiprInterventionStudyV1,
) -> Result<String, InterventionStudyError> {
    let payload = study.canonical_protocol_payload();
    let bytes = serde_json::to_vec(&payload).map_err(|error| InterventionStudyError {
        code: codes::MALFORMED_IDENTITY,
        message: format!("failed to canonicalize protocol JSON: {error}"),
    })?;
    Ok(format!("sha256:{:x}", Sha256::digest(&bytes)))
}

/// Fill `protocol_digest` from the canonical payload.
pub(crate) fn seal(
    mut study: RiprInterventionStudyV1,
) -> Result<RiprInterventionStudyV1, InterventionStudyError> {
    study.protocol_digest = protocol_digest(&study)?;
    study.validate()?;
    Ok(study)
}

/// Parse, validate study laws, and require the digest to match the object.
pub(crate) fn parse_study_json(
    text: &str,
) -> Result<RiprInterventionStudyV1, InterventionStudyError> {
    let study: RiprInterventionStudyV1 =
        serde_json::from_str(text).map_err(|error| InterventionStudyError {
            code: codes::UNSUPPORTED_SCHEMA,
            message: format!("intervention study JSON did not parse: {error}"),
        })?;
    study.validate()?;
    let expected = protocol_digest(&study)?;
    if study.protocol_digest != expected {
        return Err(InterventionStudyError {
            code: codes::PROTOCOL_DIGEST_MISMATCH,
            message: format!(
                "protocol_digest {} does not match canonical payload {expected}",
                study.protocol_digest
            ),
        });
    }
    Ok(study)
}

/// Render the sealed protocol as pretty JSON with a trailing newline.
pub(crate) fn render_study_json(
    study: &RiprInterventionStudyV1,
) -> Result<String, InterventionStudyError> {
    require_sealed(study, "JSON")?;
    json::render_pretty_with_newline(study, "intervention study").map_err(|error| {
        InterventionStudyError {
            code: codes::MALFORMED_IDENTITY,
            message: error,
        }
    })
}

/// Validate study laws and require the stored digest to match the payload,
/// so every projection shares one sealed-object boundary.
fn require_sealed(
    study: &RiprInterventionStudyV1,
    projection: &str,
) -> Result<(), InterventionStudyError> {
    study.validate()?;
    let expected = protocol_digest(study)?;
    if study.protocol_digest != expected {
        return Err(InterventionStudyError {
            code: codes::PROTOCOL_DIGEST_MISMATCH,
            message: format!("protocol must be sealed before {projection} projection"),
        });
    }
    Ok(())
}

/// Render the sealed protocol as bounded Markdown.
pub(crate) fn render_study_markdown(
    study: &RiprInterventionStudyV1,
) -> Result<String, InterventionStudyError> {
    require_sealed(study, "Markdown")?;
    let mut markdown = String::new();
    markdown.push_str("# RIPR intervention study preregistration\n\n");
    markdown.push_str(&format!("- schema: `{}`\n", study.schema_version));
    markdown.push_str(&format!("- kind: `{}`\n", study.kind));
    markdown.push_str(&format!(
        "- implementation_state: `{}`\n",
        study.implementation_state
    ));
    markdown.push_str(&format!("- study_id: `{}`\n", study.study_id));
    markdown.push_str(&format!(
        "- protocol_version: `{}`\n",
        study.protocol_version
    ));
    markdown.push_str(&format!("- protocol_digest: `{}`\n", study.protocol_digest));
    markdown.push_str(&format!("- parent_issue: {}\n", study.parent_issue));
    markdown.push_str(&format!("- sequence: `{}`\n", study.sequence));
    markdown.push_str(&format!(
        "- task_series: `{}`\n",
        study.task_series.task_series_id
    ));
    markdown.push_str(&format!(
        "- repository: `{}` @ `{}`\n",
        study.repository.repository_id, study.repository.analyzed_head_sha
    ));
    markdown.push_str("\n## Conditions\n\n");
    for condition in &study.conditions {
        markdown.push_str(&format!(
            "- `{}`: {}\n",
            condition.condition_id.as_str(),
            condition.description
        ));
    }
    markdown.push_str("\n## Assignment\n\n");
    markdown.push_str(
        "- freeze: before first outcome or grader signal; assignment cannot change afterward\n",
    );
    for pair in &study.assignment.pairs {
        markdown.push_str(&format!(
            "- `{}` task `{}` order `{}` then `{}`\n",
            pair.pair_id,
            pair.task_id,
            pair.first_condition.as_str(),
            pair.second_condition.as_str()
        ));
    }
    markdown.push_str("\n## Shared budget\n\n");
    markdown.push_str(&format!("- model: `{}`\n", study.shared_budget.model_id));
    markdown.push_str(&format!(
        "- operator: `{}`\n",
        study.shared_budget.operator_profile
    ));
    markdown.push_str(&format!(
        "- runtime: `{}`\n",
        study.shared_budget.runtime_id
    ));
    markdown.push_str(&format!(
        "- tools: `{}`\n",
        study.shared_budget.tool_ids.join("`, `")
    ));
    markdown.push_str(&format!(
        "- wall_clock_ms: {}\n",
        study.shared_budget.wall_clock_ms
    ));
    markdown.push_str(&format!(
        "- token_budget: {}\n",
        study.shared_budget.token_budget
    ));
    markdown.push_str(&format!(
        "- retry_limit: {}\n",
        study.shared_budget.retry_limit
    ));
    markdown.push_str("\n## RIPR-assisted evidence surface\n\n");
    push_list(
        &mut markdown,
        "receipts",
        &study
            .intervention_surface
            .ripr_assisted
            .allowed_ripr_receipts,
    );
    push_list(
        &mut markdown,
        "views",
        &study.intervention_surface.ripr_assisted.allowed_ripr_views,
    );
    push_list(
        &mut markdown,
        "commands",
        &study
            .intervention_surface
            .ripr_assisted
            .allowed_ripr_commands,
    );
    markdown.push_str("\nControl receives none of those RIPR outputs.\n");
    markdown.push_str("\n## Outcome axes\n\n");
    markdown.push_str("Axes are independently observable and non-compensating.\n\n");
    for axis in &study.outcome_axes {
        markdown.push_str(&format!("- `{}`: {}\n", axis.axis_id, axis.description));
    }
    markdown.push_str("\n## Adjudication\n\n");
    markdown.push_str(&format!(
        "- graders: `{}`\n",
        study.adjudication.grader_identities.join("`, `")
    ));
    markdown.push_str(&format!(
        "- rubric: `{}` version `{}`\n",
        study.adjudication.rubric_id, study.adjudication.rubric_version
    ));
    markdown.push_str("- condition identity is withheld from graders\n");
    markdown.push_str("\n## Stopping and claim ceiling\n\n");
    markdown.push_str(&format!(
        "- fixed sample of {} matched pairs\n",
        study.stopping_rule.planned_pair_count
    ));
    markdown.push_str("- stopping does not depend on a favorable interim estimate\n");
    markdown.push_str(
        "- conclusions are bounded to this task series, model/operator profile, and intervention form\n",
    );
    markdown.push_str("- a valid preregistration does not prove intervention value\n");
    markdown.push_str("\n## Non-claims\n\n");
    for claim in &study.non_claims {
        markdown.push_str(&format!("- `{claim}`\n"));
    }
    Ok(markdown)
}

fn push_list(markdown: &mut String, label: &str, values: &[String]) {
    markdown.push_str(&format!("- {label}:"));
    if values.is_empty() {
        markdown.push_str(" (none)\n");
        return;
    }
    markdown.push('\n');
    for value in values {
        markdown.push_str(&format!("  - `{value}`\n"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        RIPR_INTERVENTION_STUDY_SCHEMA_VERSION, example_preregistered_study,
        intervention_study_codes as codes,
    };
    use crate::output::test_support::repo_root;
    use serde_json::{Value, json};
    use std::path::Path;

    fn load(path: &Path) -> Result<String, String> {
        std::fs::read_to_string(path).map_err(|error| format!("read {}: {error}", path.display()))
    }

    fn should_write_fixtures() -> bool {
        std::env::var("RIPR_UPDATE_FIXTURES").as_deref() == Ok("1")
    }

    fn write_file(path: &Path, contents: &str) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("create {}: {error}", parent.display()))?;
        }
        std::fs::write(path, contents).map_err(|error| format!("write {}: {error}", path.display()))
    }

    fn patch_valid(patch: fn(&mut Value)) -> Result<Value, String> {
        let mut value = serde_json::to_value(example_preregistered_study())
            .map_err(|error| error.to_string())?;
        patch(&mut value);
        Ok(value)
    }

    fn corpus_document() -> Result<Value, String> {
        let mut predecessor = serde_json::to_value(example_preregistered_study())
            .map_err(|error| error.to_string())?;
        predecessor["protocol_lock"]["first_attempt_recorded"] = json!(true);
        let mut successor = predecessor.clone();
        successor["shared_budget"]["token_budget"] = json!(40_000);
        Ok(json!({
            "schema_version": "ripr_intervention_study_corpus.v1",
            "kind": "ripr_intervention_study_corpus",
            "spec": "RIPR-SPEC-0216",
            "valid": serde_json::to_value(seal(example_preregistered_study()).map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string())?,
            "falsifiers": [
                {
                    "id": "assignment_after_outcome",
                    "expected_code": "assignment_after_outcome",
                    "protocol": patch_valid(|value| {
                        value["assignment"]["may_change_after_outcome_or_grader_signal"] = json!(true);
                    })?
                },
                {
                    "id": "unequal_condition_budgets",
                    "expected_code": "unequal_condition_budgets",
                    "protocol": patch_valid(|value| {
                        value["condition_budget_overlays"]["ripr_assisted"]["additional_token_budget"] = json!(8000);
                    })?
                },
                {
                    "id": "control_can_read_ripr_outputs",
                    "expected_code": "control_can_read_ripr_outputs",
                    "protocol": patch_valid(|value| {
                        value["intervention_surface"]["control"]["may_read_ripr_outputs"] = json!(true);
                    })?
                },
                {
                    "id": "assisted_undeclared_extra_context",
                    "expected_code": "assisted_undeclared_extra_context",
                    "protocol": patch_valid(|value| {
                        value["intervention_surface"]["ripr_assisted"]["undeclared_repository_context"] =
                            json!(["undeclared issue analysis"]);
                    })?
                },
                {
                    "id": "task_replacement_after_failure",
                    "expected_code": "task_replacement_after_failure",
                    "protocol": patch_valid(|value| {
                        value["attempt_policy"]["task_replacement_after_failure"] =
                            json!("allowed_after_difficult_failure");
                    })?
                },
                {
                    "id": "retry_only_in_weaker_condition",
                    "expected_code": "retry_only_in_weaker_condition",
                    "protocol": patch_valid(|value| {
                        value["attempt_policy"]["retry_policy_equal_across_conditions"] = json!(false);
                    })?
                },
                {
                    "id": "drop_timeouts_or_invalid_from_denominator",
                    "expected_code": "drop_timeouts_or_invalid_from_denominator",
                    "protocol": patch_valid(|value| {
                        value["invalid_attempt_rules"]["drop_timeouts"] = json!(true);
                    })?
                },
                {
                    "id": "stopping_on_favorable_interim",
                    "expected_code": "stopping_on_favorable_interim",
                    "protocol": patch_valid(|value| {
                        value["stopping_rule"]["kind"] = json!("stop_on_favorable_interim");
                        value["stopping_rule"]["may_depend_on_interim_estimate"] = json!(true);
                        value["stopping_rule"]["favorable_interim_stop"] = json!("allowed");
                    })?
                },
                {
                    "id": "grader_or_rubric_absent",
                    "expected_code": "grader_or_rubric_absent",
                    "protocol": patch_valid(|value| {
                        value["adjudication"]["grader_identities"] = json!([]);
                    })?
                },
                {
                    "id": "protocol_mutation_after_first_attempt",
                    "expected_code": "protocol_mutation_requires_new_study_id",
                    "predecessor": predecessor,
                    "protocol": successor
                }
            ]
        }))
    }

    #[test]
    fn json_and_markdown_derive_from_one_sealed_object() -> Result<(), String> {
        let study = seal(example_preregistered_study()).map_err(|error| error.to_string())?;
        let json = render_study_json(&study).map_err(|error| error.to_string())?;
        let markdown = render_study_markdown(&study).map_err(|error| error.to_string())?;
        let parsed = parse_study_json(&json).map_err(|error| error.to_string())?;
        if parsed != study {
            return Err("JSON round-trip changed the semantic object".to_string());
        }
        if !markdown.contains(&study.study_id)
            || !markdown.contains(&study.protocol_digest)
            || !markdown.contains("`control`")
            || !markdown.contains("`ripr_assisted`")
            || markdown.contains("intervention value established")
        {
            return Err(
                "markdown is missing identity or overclaims intervention value".to_string(),
            );
        }
        let again = render_study_markdown(&parsed).map_err(|error| error.to_string())?;
        if again != markdown {
            return Err("markdown projection is not deterministic".to_string());
        }
        Ok(())
    }

    #[test]
    fn committed_valid_fixture_matches_sealed_example() -> Result<(), String> {
        let root = repo_root()?;
        let sealed = seal(example_preregistered_study()).map_err(|error| error.to_string())?;
        let rendered = render_study_json(&sealed).map_err(|error| error.to_string())?;
        let markdown = render_study_markdown(&sealed).map_err(|error| error.to_string())?;
        let corpus = serde_json::to_string_pretty(&corpus_document()?)
            .map_err(|error| error.to_string())?
            + "\n";
        let valid_path = root.join("fixtures/intervention-study/valid.json");
        let markdown_path = root.join("fixtures/intervention-study/expected.md");
        let corpus_path = root.join("fixtures/intervention-study/corpus.json");
        if should_write_fixtures() {
            write_file(&valid_path, &rendered)?;
            write_file(&markdown_path, &markdown)?;
            write_file(&corpus_path, &corpus)?;
        }
        let fixture = load(&valid_path)?;
        let parsed = parse_study_json(&fixture).map_err(|error| error.to_string())?;
        if parsed != sealed {
            return Err("valid.json drifted from example_preregistered_study()".to_string());
        }
        if rendered != fixture {
            return Err("valid.json is not the deterministic JSON projection".to_string());
        }
        let expected_md = load(&markdown_path)?;
        if markdown != expected_md {
            return Err("expected.md drifted from the Markdown projection".to_string());
        }
        if load(&corpus_path)? != corpus {
            return Err("corpus.json drifted from the generated falsifier corpus".to_string());
        }
        if parsed.schema_version != RIPR_INTERVENTION_STUDY_SCHEMA_VERSION {
            return Err("fixture schema_version drifted".to_string());
        }
        Ok(())
    }

    #[test]
    fn corpus_falsifiers_are_rejected_with_preregistered_codes() -> Result<(), String> {
        let root = repo_root()?;
        let corpus_text = load(&root.join("fixtures/intervention-study/corpus.json"))?;
        let corpus: Value =
            serde_json::from_str(&corpus_text).map_err(|error| format!("corpus JSON: {error}"))?;
        let falsifiers = corpus
            .get("falsifiers")
            .and_then(Value::as_array)
            .ok_or("corpus missing falsifiers")?;
        if falsifiers.len() != 10 {
            return Err(format!(
                "expected 10 falsifiers, found {}",
                falsifiers.len()
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        for case in falsifiers {
            let id = case
                .get("id")
                .and_then(Value::as_str)
                .ok_or("falsifier missing id")?;
            if !seen.insert(id) {
                return Err(format!("duplicate falsifier {id}"));
            }
            let expected = case
                .get("expected_code")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("{id} missing expected_code"))?;
            let error = if let Some(predecessor) = case.get("predecessor") {
                let previous = parse_protocol_value(predecessor)?;
                let next = parse_protocol_value(
                    case.get("protocol")
                        .ok_or_else(|| format!("{id} missing protocol"))?,
                )?;
                match previous.validate_successor(&next) {
                    Ok(()) => return Err(format!("{id} successor was accepted")),
                    Err(error) => error,
                }
            } else {
                let protocol = case
                    .get("protocol")
                    .ok_or_else(|| format!("{id} missing protocol"))?;
                let study: RiprInterventionStudyV1 = serde_json::from_value(protocol.clone())
                    .map_err(|error| format!("{id} protocol parse: {error}"))?;
                match study.validate() {
                    Ok(()) => return Err(format!("{id} protocol was accepted")),
                    Err(error) => error,
                }
            };
            if error.code != expected {
                return Err(format!(
                    "{id} expected {expected}, got {}: {}",
                    error.code, error.message
                ));
            }
        }
        for required in [
            "assignment_after_outcome",
            "unequal_condition_budgets",
            "control_can_read_ripr_outputs",
            "assisted_undeclared_extra_context",
            "task_replacement_after_failure",
            "retry_only_in_weaker_condition",
            "drop_timeouts_or_invalid_from_denominator",
            "stopping_on_favorable_interim",
            "grader_or_rubric_absent",
            "protocol_mutation_after_first_attempt",
        ] {
            if !seen.contains(required) {
                return Err(format!("corpus missing falsifier {required}"));
            }
        }
        Ok(())
    }

    fn parse_protocol_value(value: &Value) -> Result<RiprInterventionStudyV1, String> {
        serde_json::from_value(value.clone())
            .map_err(|error| format!("protocol value parse: {error}"))
    }

    #[test]
    fn digest_mismatch_is_rejected_after_laws_pass() -> Result<(), String> {
        let mut study = seal(example_preregistered_study()).map_err(|error| error.to_string())?;
        study.protocol_digest =
            "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_string();
        match parse_study_json(&serde_json::to_string(&study).map_err(|error| error.to_string())?) {
            Err(error) if error.code == codes::PROTOCOL_DIGEST_MISMATCH => Ok(()),
            Err(error) => Err(format!(
                "expected protocol_digest_mismatch, got {}: {}",
                error.code, error.message
            )),
            Ok(_) => Err("digest mismatch was accepted".to_string()),
        }
    }

    #[test]
    fn markdown_rejects_a_stale_digest_after_post_seal_mutation() -> Result<(), String> {
        let mut study = seal(example_preregistered_study()).map_err(|error| error.to_string())?;
        render_study_markdown(&study).map_err(|error| error.to_string())?;
        study.shared_budget.token_budget = 40_000;
        study.validate().map_err(|error| error.to_string())?;
        for (surface, result) in [
            ("markdown", render_study_markdown(&study)),
            ("json", render_study_json(&study)),
        ] {
            match result {
                Err(error) if error.code == codes::PROTOCOL_DIGEST_MISMATCH => {}
                Err(error) => {
                    return Err(format!(
                        "{surface}: expected protocol_digest_mismatch, got {}: {}",
                        error.code, error.message
                    ));
                }
                Ok(_) => return Err(format!("{surface} projected a stale digest")),
            }
        }
        Ok(())
    }

    #[test]
    fn projections_do_not_execute_or_adjudicate() -> Result<(), String> {
        let source = include_str!("intervention_study.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .ok_or("missing production module")?;
        for forbidden in [
            concat!("std::", "process"),
            concat!("Command::", "new"),
            "adjudicate",
            "publish_pilot",
        ] {
            if production.contains(forbidden) {
                return Err(format!(
                    "projection module contains forbidden token {forbidden}"
                ));
            }
        }
        Ok(())
    }
}
