use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde_json::{Map, Value, json};

const CORPUS_PATH: &str = "metrics/rust-repair-trust/corpus.json";
const MIN_REPOSITORIES: usize = 3;
const MIN_ATTEMPTS: usize = 20;
const MOVEMENTS: [&str; 5] = ["closed", "improved", "unchanged", "regressed", "limited"];
const EXCLUSION_REASONS: [&str; 7] = [
    "analysis_timeout",
    "diff_scope_oversized",
    "static_limitation_no_repair_packet",
    "false_actionability",
    "production_test_path_rejected",
    "verification_failed",
    "no_current_behavior_change",
];
const OBSERVATION_CLASSIFICATIONS: [&str; 2] = ["new_exclusion", "duplicate_observation"];
const REQUIRED_ROUTE_FIELDS: [&str; 19] = [
    "attempt_id",
    "repository",
    "analyzed_head_sha",
    "canonical_gap_id",
    "seam_id",
    "file_line",
    "changed_behavior",
    "missing_discriminator",
    "related_test_or_production_caller",
    "focused_test_intent",
    "before_receipt",
    "repair_intent",
    "verification_command",
    "verification_result",
    "targeted_rerun_command",
    "receipt_command",
    "inspection_command",
    "after_receipt",
    "claim_boundary",
];

pub(crate) fn rust_repair_trust_report() -> Result<(), String> {
    let corpus = read_corpus(Path::new(CORPUS_PATH))?;
    let report = build_report(&corpus);
    let json_body = serde_json::to_string_pretty(&report)
        .map_err(|error| format!("serialize Rust repair trust report: {error}"))?;
    crate::write_report("rust-repair-trust.json", &format!("{json_body}\n"))?;
    crate::write_report("rust-repair-trust.md", &markdown_report(&report))
}

fn read_corpus(path: &Path) -> Result<Value, String> {
    let body =
        fs::read_to_string(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    serde_json::from_str(&body).map_err(|error| format!("parse {}: {error}", path.display()))
}

fn build_report(corpus: &Value) -> Value {
    let mut validation_errors = Vec::new();
    let schema_ok = corpus.get("schema_version").and_then(Value::as_str) == Some("0.1");
    let kind_ok = corpus.get("kind").and_then(Value::as_str) == Some("rust_repair_trust_corpus");
    if !schema_ok {
        validation_errors.push("schema_version must be 0.1".to_string());
    }
    if !kind_ok {
        validation_errors.push("kind must be rust_repair_trust_corpus".to_string());
    }

    let authorization = corpus.get("authorization");
    let authorization_status = authorization
        .and_then(|value| value.get("status"))
        .and_then(Value::as_str)
        .unwrap_or("missing")
        .to_string();
    let authorization_repository_count = authorization
        .and_then(|value| value.get("repositories"))
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let authorized_repositories = authorization
        .and_then(|value| value.get("repositories"))
        .and_then(Value::as_array)
        .map(|repositories| {
            repositories
                .iter()
                .filter_map(|repository| {
                    let name = repository.get("name").and_then(Value::as_str)?;
                    let reference = repository
                        .get("authorization_ref")
                        .and_then(Value::as_str)?;
                    let revision_or_branch = repository
                        .get("authorized_revision_or_branch")
                        .and_then(Value::as_str)?;
                    let write_policy = repository.get("write_policy").and_then(Value::as_str)?;
                    let authorization_date = repository
                        .get("authorization_date")
                        .and_then(Value::as_str)?;
                    let artifact_paths = repository
                        .get("allowed_artifact_paths")
                        .and_then(Value::as_array)?;
                    let analysis_actions = repository
                        .get("allowed_analysis_actions")
                        .and_then(Value::as_array)?;
                    if name.trim().is_empty()
                        || reference.trim().is_empty()
                        || revision_or_branch.trim().is_empty()
                        || write_policy.trim().is_empty()
                        || authorization_date.trim().is_empty()
                        || artifact_paths.is_empty()
                        || analysis_actions.is_empty()
                    {
                        None
                    } else {
                        Some(name.to_string())
                    }
                })
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    let authorized_observation_heads = authorization
        .and_then(|value| value.get("repositories"))
        .and_then(Value::as_array)
        .map(|repositories| {
            repositories
                .iter()
                .filter_map(|repository| {
                    let name = repository.get("name").and_then(Value::as_str)?;
                    let heads = repository
                        .get("authorized_observation_heads")
                        .and_then(Value::as_array)
                        .map(|heads| {
                            heads
                                .iter()
                                .filter_map(Value::as_str)
                                .map(str::to_string)
                                .collect::<BTreeSet<_>>()
                        })
                        .unwrap_or_default();
                    Some((name.to_string(), heads))
                })
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    if authorization_repository_count != authorized_repositories.len() {
        validation_errors.push(
            "every authorization repository requires revision/branch, artifact paths, analysis actions, write policy, and date"
                .to_string(),
        );
    }
    if authorization_status != "complete" {
        validation_errors.push("authorization.status must be complete".to_string());
    }
    if authorized_repositories.len() < MIN_REPOSITORIES {
        validation_errors.push(format!(
            "at least {MIN_REPOSITORIES} authorized repositories are required"
        ));
    }

    let cases = corpus.get("cases").and_then(Value::as_array);
    if cases.is_none() {
        validation_errors.push("cases must be an array".to_string());
    }
    let cases = cases.cloned().unwrap_or_default();
    let exclusions = corpus
        .get("exclusions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let observations = corpus
        .get("observations")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut case_ids = BTreeSet::new();
    let mut exclusion_ids = BTreeSet::new();
    let mut valid_exclusion_ids = BTreeSet::new();
    let mut movements = BTreeMap::<String, usize>::new();
    let mut exclusion_reason_counts = BTreeMap::<String, usize>::new();
    let mut repository_names = BTreeSet::new();
    let mut eligible_attempts = 0usize;
    let mut groups = BTreeMap::<(String, String), Vec<(usize, String)>>::new();
    let mut boolean_counts = BTreeMap::<&str, usize>::new();
    let mut missing_route_fields = BTreeMap::<String, usize>::new();
    let mut attempts_with_limitations = 0usize;
    let mut attempts_with_call_presence_limitations = 0usize;

    for (index, case) in cases.iter().enumerate() {
        let prefix = format!("cases[{index}]");
        for field in REQUIRED_ROUTE_FIELDS {
            if case
                .get(field)
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
            {
                *missing_route_fields.entry(field.to_string()).or_default() += 1;
            }
        }
        let mut errors = case_errors(case, &authorized_repositories);
        let id = case.get("attempt_id").and_then(Value::as_str).unwrap_or("");
        if !id.is_empty() && !case_ids.insert(id.to_string()) {
            errors.push(format!("duplicate id {id}"));
        }
        if let Some(repository) = case.get("repository").and_then(Value::as_str) {
            repository_names.insert(repository.to_string());
        }
        if errors.is_empty() {
            eligible_attempts += 1;
            let repository = case
                .get("repository")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let gap = case
                .get("canonical_gap_id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let attempt_number = case
                .get("attempt_number")
                .and_then(Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .unwrap_or(0);
            let movement = case
                .get("movement")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            *movements.entry(movement.clone()).or_default() += 1;
            groups
                .entry((repository, gap))
                .or_default()
                .push((attempt_number, movement));
            if case
                .get("limitations")
                .and_then(Value::as_array)
                .is_some_and(|limitations| !limitations.is_empty())
            {
                attempts_with_limitations += 1;
                if case
                    .get("limitations")
                    .and_then(Value::as_array)
                    .is_some_and(|limitations| {
                        limitations
                            .iter()
                            .filter_map(Value::as_str)
                            .any(|limitation| {
                                let normalized = limitation.to_ascii_lowercase();
                                normalized.contains("callpresence")
                                    || normalized.contains("call_presence")
                            })
                    })
                {
                    attempts_with_call_presence_limitations += 1;
                }
            }
            for field in [
                "false_actionability",
                "known_impossible_recommendation",
                "parity_failure",
                "artifact_archaeology",
            ] {
                if case.get(field).and_then(Value::as_bool).unwrap_or(false) {
                    *boolean_counts.entry(field).or_default() += 1;
                }
            }
        } else {
            for error in errors {
                validation_errors.push(format!("{prefix}: {error}"));
            }
        }
    }

    let mut valid_exclusions = 0usize;
    for (index, exclusion) in exclusions.iter().enumerate() {
        let prefix = format!("exclusions[{index}]");
        let mut errors = exclusion_errors(exclusion, &authorized_repositories);
        let id = exclusion
            .get("exclusion_id")
            .and_then(Value::as_str)
            .unwrap_or("");
        if !id.is_empty() && !exclusion_ids.insert(id.to_string()) {
            errors.push(format!("duplicate exclusion id {id}"));
        }
        if !id.is_empty() && case_ids.contains(id) {
            errors.push(format!("exclusion id {id} collides with an attempt id"));
        }
        if errors.is_empty() {
            valid_exclusions += 1;
            valid_exclusion_ids.insert(id.to_string());
            if let Some(repository) = exclusion.get("repository").and_then(Value::as_str) {
                repository_names.insert(repository.to_string());
            }
            if let Some(reason) = exclusion.get("reason").and_then(Value::as_str) {
                *exclusion_reason_counts
                    .entry(reason.to_string())
                    .or_default() += 1;
            }
        } else {
            for error in errors {
                validation_errors.push(format!("{prefix}: {error}"));
            }
        }
    }

    let mut valid_observations = 0usize;
    let mut duplicate_observations = 0usize;
    let mut new_exclusion_observations = 0usize;
    let mut duplicate_timeout_observations = 0usize;
    let mut observation_classification_counts = BTreeMap::<String, usize>::new();
    let mut observation_ids = BTreeSet::new();
    let timeout_exclusion_count = exclusion_reason_counts
        .get("analysis_timeout")
        .copied()
        .unwrap_or(0);
    for (index, observation) in observations.iter().enumerate() {
        let prefix = format!("observations[{index}]");
        let mut errors = observation_errors(
            observation,
            &authorized_repositories,
            &authorized_observation_heads,
            &exclusions,
            &valid_exclusion_ids,
        );
        let id = observation
            .get("observation_id")
            .and_then(Value::as_str)
            .unwrap_or("");
        if !id.is_empty() && !observation_ids.insert(id.to_string()) {
            errors.push(format!("duplicate observation id {id}"));
        }
        if errors.is_empty() {
            valid_observations += 1;
            if let Some(repository) = observation.get("repository").and_then(Value::as_str) {
                repository_names.insert(repository.to_string());
            }
            let classification = observation
                .get("classification")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            *observation_classification_counts
                .entry(classification.clone())
                .or_default() += 1;
            if classification == "duplicate_observation" {
                duplicate_observations += 1;
                if observation.get("reason").and_then(Value::as_str) == Some("analysis_timeout") {
                    duplicate_timeout_observations += 1;
                }
            } else if classification == "new_exclusion" {
                new_exclusion_observations += 1;
            }
        } else {
            for error in errors {
                validation_errors.push(format!("{prefix}: {error}"));
            }
        }
    }

    let mut movement_counts = Map::new();
    for movement in MOVEMENTS {
        movement_counts.insert(
            movement.to_string(),
            Value::from(*movements.get(movement).unwrap_or(&0)),
        );
    }
    let improvement_groups = groups
        .values()
        .filter_map(|attempts| {
            attempts
                .iter()
                .filter(|(_, movement)| movement == "closed" || movement == "improved")
                .min_by_key(|(attempt, _)| *attempt)
                .map(|(attempt, _)| *attempt)
        })
        .collect::<Vec<_>>();
    let one_attempt_improvement_rate = if groups.is_empty() {
        Value::Null
    } else {
        Value::from(
            improvement_groups
                .iter()
                .filter(|attempt| **attempt == 1)
                .count() as f64
                / groups.len() as f64,
        )
    };
    let attempts_to_first_improvement = if improvement_groups.is_empty() {
        Value::Null
    } else {
        Value::from(
            improvement_groups.iter().sum::<usize>() as f64 / improvement_groups.len() as f64,
        )
    };
    let one_attempt_improvement_numerator = improvement_groups
        .iter()
        .filter(|attempt| **attempt == 1)
        .count();
    let one_attempt_improvement_denominator = groups.len();
    let call_presence_limitation_frequency = if eligible_attempts == 0 {
        Value::Null
    } else {
        Value::from(attempts_with_call_presence_limitations as f64 / eligible_attempts as f64)
    };

    let threshold_met = authorization_status == "complete"
        && authorized_repositories.len() >= MIN_REPOSITORIES
        && eligible_attempts >= MIN_ATTEMPTS
        && validation_errors.is_empty();
    if eligible_attempts < MIN_ATTEMPTS {
        validation_errors.push(format!(
            "at least {MIN_ATTEMPTS} eligible attempts are required"
        ));
    }
    let status = if threshold_met { "complete" } else { "limited" };
    let mut limitations = Vec::new();
    if authorization_status != "complete" {
        limitations.push("authorization_missing".to_string());
    }
    if authorized_repositories.len() < MIN_REPOSITORIES {
        limitations.push("fewer_than_three_authorized_repositories".to_string());
    }
    if eligible_attempts < MIN_ATTEMPTS {
        limitations.push("fewer_than_twenty_eligible_attempts".to_string());
    }
    if eligible_attempts == 0 {
        limitations.push("no_real_rust_attempts_recorded".to_string());
    }

    json!({
        "schema_version": "0.1",
        "report": "rust-repair-trust",
        "status": status,
        "run_status": if threshold_met { "full" } else { "limited_incomplete_input" },
        "source_path": CORPUS_PATH,
        "authorization_status": authorization_status,
        "repository_count": repository_names.len(),
        "authorized_repository_count": authorized_repositories.len(),
        "attempt_count": cases.len(),
        "eligible_attempt_count": eligible_attempts,
        "exclusion_count": exclusions.len(),
        "valid_exclusion_count": valid_exclusions,
        "observation_count": valid_exclusions + valid_observations - new_exclusion_observations,
        "valid_observation_count": valid_observations,
        "unique_exclusion_count": valid_exclusions,
        "duplicate_observation_count": duplicate_observations,
        "timeout_observation_count": timeout_exclusion_count + duplicate_timeout_observations,
        "observation_classification_counts": observation_classification_counts,
        "requirements": {
            "minimum_repositories": MIN_REPOSITORIES,
            "minimum_attempts": MIN_ATTEMPTS,
            "movement_vocabulary": MOVEMENTS,
            "selected_scope_parity_required": true,
            "test_only_repairs_required": true,
            "exact_revision_required": true
        },
        "movement_counts": movement_counts,
        "exclusion_reason_counts": exclusion_reason_counts,
        "scorecard": {
            "one_attempt_improvement_rate": one_attempt_improvement_rate,
            "one_attempt_improvement_numerator": one_attempt_improvement_numerator,
            "one_attempt_improvement_denominator": one_attempt_improvement_denominator,
            "attempts_to_first_improvement_average": attempts_to_first_improvement,
            "attempts_to_first_improvement_denominator": improvement_groups.len(),
            "repair_rounds_total": eligible_attempts,
            "metric_attempt_denominator": eligible_attempts,
            "limitation_frequency": if eligible_attempts == 0 {
                Value::Null
            } else {
                Value::from(attempts_with_limitations as f64 / eligible_attempts as f64)
            },
            "limitation_frequency_numerator": attempts_with_limitations,
            "limitation_frequency_denominator": eligible_attempts,
            "call_presence_limitation_frequency": call_presence_limitation_frequency,
            "call_presence_limitation_frequency_numerator": attempts_with_call_presence_limitations,
            "call_presence_limitation_frequency_denominator": eligible_attempts,
            "missing_route_fields": missing_route_fields,
            "missing_route_fields_denominator": cases.len(),
            "false_actionability_incidents": boolean_counts.get("false_actionability").copied().unwrap_or(0),
            "known_impossible_recommendations": boolean_counts.get("known_impossible_recommendation").copied().unwrap_or(0),
            "parity_failures": boolean_counts.get("parity_failure").copied().unwrap_or(0),
            "artifact_archaeology_incidents": boolean_counts.get("artifact_archaeology").copied().unwrap_or(0)
        },
        "limitations": limitations,
        "validation_errors": validation_errors,
        "claim_boundary": [
            "This report measures route evidence, not developers or agents.",
            "It is not runtime mutation evidence, coverage evidence, or a correctness proof.",
            "Synthetic fixtures and preview-language cases do not satisfy the Rust corpus threshold.",
            "A faster rerun without selected-scope parity is limited."
        ]
    })
}

fn case_errors(case: &Value, authorized_repositories: &BTreeSet<String>) -> Vec<String> {
    let mut errors = Vec::new();
    for field in REQUIRED_ROUTE_FIELDS {
        if case
            .get(field)
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
        {
            errors.push(format!("{field} must be a non-empty string"));
        }
    }
    let Some(repository) = case.get("repository").and_then(Value::as_str) else {
        return errors;
    };
    if !authorized_repositories.contains(repository) {
        errors.push(format!("repository {repository} is not authorized"));
    }
    let Some(revision) = case.get("analyzed_head_sha").and_then(Value::as_str) else {
        return errors;
    };
    if revision.len() != 40
        || !revision
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        errors.push("revision must be a 40-character commit SHA".to_string());
    }
    let Some(movement) = case.get("movement").and_then(Value::as_str) else {
        errors.push("movement must use the closed vocabulary".to_string());
        return errors;
    };
    if !MOVEMENTS.contains(&movement) {
        errors.push(format!(
            "movement {movement} is not in the closed vocabulary"
        ));
    }
    if case
        .get("attempt_number")
        .and_then(Value::as_u64)
        .is_none_or(|number| number == 0)
    {
        errors.push("attempt_number must be a positive integer".to_string());
    }
    for field in [
        "changed_test_files",
        "allowed_edit_surface",
        "limitations",
        "source_refs",
    ] {
        match case.get(field).and_then(Value::as_array) {
            Some(values) if field != "limitations" && values.is_empty() => {
                errors.push(format!("{field} must not be empty"));
            }
            Some(values)
                if values
                    .iter()
                    .any(|value| value.as_str().is_none_or(str::is_empty)) =>
            {
                errors.push(format!("{field} must contain only non-empty strings"));
            }
            None => errors.push(format!("{field} must be an array")),
            _ => {}
        }
    }
    for field in [
        "test_only",
        "production_files_changed",
        "false_actionability",
        "known_impossible_recommendation",
        "parity_failure",
        "artifact_archaeology",
    ] {
        if case.get(field).and_then(Value::as_bool).is_none() {
            errors.push(format!("{field} must be a boolean"));
        }
    }
    if case.get("test_only").and_then(Value::as_bool) != Some(true) {
        errors.push("test_only must be true".to_string());
    }
    if case
        .get("production_files_changed")
        .and_then(Value::as_bool)
        != Some(false)
    {
        errors.push("production_files_changed must be false".to_string());
    }
    if case
        .get("canonical_gap_id")
        .and_then(Value::as_str)
        .is_some_and(|id| !id.starts_with("gap:"))
    {
        errors.push("canonical_gap_id must start with gap:".to_string());
    }
    errors
}

fn exclusion_errors(exclusion: &Value, authorized_repositories: &BTreeSet<String>) -> Vec<String> {
    let mut errors = Vec::new();
    for field in [
        "exclusion_id",
        "repository",
        "analyzed_head_sha",
        "source_ref",
        "reason",
        "evidence_ref",
        "command",
        "claim_boundary",
    ] {
        if exclusion
            .get(field)
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
        {
            errors.push(format!("{field} must be a non-empty string"));
        }
    }
    if let Some(repository) = exclusion.get("repository").and_then(Value::as_str)
        && !authorized_repositories.contains(repository)
    {
        errors.push(format!("repository {repository} is not authorized"));
    }
    if let Some(revision) = exclusion.get("analyzed_head_sha").and_then(Value::as_str)
        && (revision.len() != 40
            || !revision
                .chars()
                .all(|character| character.is_ascii_hexdigit()))
    {
        errors.push("revision must be a 40-character commit SHA".to_string());
    }
    if let Some(reason) = exclusion.get("reason").and_then(Value::as_str)
        && !EXCLUSION_REASONS.contains(&reason)
    {
        errors.push(format!(
            "reason {reason} is not in the exclusion vocabulary"
        ));
    }
    errors
}

fn observation_errors(
    observation: &Value,
    authorized_repositories: &BTreeSet<String>,
    authorized_observation_heads: &BTreeMap<String, BTreeSet<String>>,
    exclusions: &[Value],
    exclusion_ids: &BTreeSet<String>,
) -> Vec<String> {
    let mut errors = Vec::new();
    for field in [
        "observation_id",
        "repository",
        "analyzed_head_sha",
        "source_ref",
        "canonical_candidate_id",
        "reason",
        "evidence_ref",
        "classification",
        "claim_boundary",
    ] {
        if observation
            .get(field)
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
        {
            errors.push(format!("{field} must be a non-empty string"));
        }
    }
    let Some(repository) = observation.get("repository").and_then(Value::as_str) else {
        return errors;
    };
    if !authorized_repositories.contains(repository) {
        errors.push(format!("repository {repository} is not authorized"));
    }
    let Some(revision) = observation.get("analyzed_head_sha").and_then(Value::as_str) else {
        return errors;
    };
    if revision.len() != 40
        || !revision
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        errors.push("revision must be a 40-character commit SHA".to_string());
    }
    match authorized_observation_heads.get(repository) {
        None => errors.push(format!(
            "repository {repository} has no authorized observation heads"
        )),
        Some(heads) if heads.is_empty() => errors.push(format!(
            "repository {repository} has an empty authorized observation head allowlist"
        )),
        Some(heads) if !heads.contains(revision) => errors.push(format!(
            "revision {revision} is not explicitly authorized for observation"
        )),
        Some(_) => {}
    }
    if let Some(reason) = observation.get("reason").and_then(Value::as_str)
        && !EXCLUSION_REASONS.contains(&reason)
    {
        errors.push(format!(
            "reason {reason} is not in the exclusion vocabulary"
        ));
    }
    let Some(classification) = observation.get("classification").and_then(Value::as_str) else {
        return errors;
    };
    if !OBSERVATION_CLASSIFICATIONS.contains(&classification) {
        errors.push(format!(
            "classification {classification} is not in the observation vocabulary"
        ));
    }
    let duplicate_of = observation.get("duplicate_of").and_then(Value::as_str);
    if classification == "duplicate_observation" {
        let Some(duplicate_of) = duplicate_of else {
            errors.push("duplicate_of is required for duplicate observations".to_string());
            return errors;
        };
        let Some(target) = exclusions.iter().find(|exclusion| {
            exclusion.get("exclusion_id").and_then(Value::as_str) == Some(duplicate_of)
        }) else {
            errors.push(format!(
                "duplicate_of {duplicate_of} does not name an exclusion"
            ));
            return errors;
        };
        if !exclusion_ids.contains(duplicate_of) {
            errors.push(format!(
                "duplicate_of {duplicate_of} names an invalid exclusion"
            ));
        }
        for field in ["repository", "analyzed_head_sha", "source_ref"] {
            if observation.get(field) != target.get(field) {
                errors.push(format!(
                    "duplicate observation does not match exclusion {duplicate_of} field {field}"
                ));
            }
        }
        if observation.get("canonical_candidate_id") != target.get("canonical_candidate_id") {
            errors.push(format!(
                "duplicate observation does not match exclusion {duplicate_of} candidate identity"
            ));
        }
    } else if duplicate_of.is_some() {
        errors.push("duplicate_of is only valid for duplicate observations".to_string());
    }
    errors
}

fn markdown_report(report: &Value) -> String {
    let status = report
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("limited");
    let run_status = report
        .get("run_status")
        .and_then(Value::as_str)
        .unwrap_or("limited_incomplete_input");
    let attempt_count = report
        .get("attempt_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let observation_count = report
        .get("observation_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let unique_exclusion_count = report
        .get("unique_exclusion_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let duplicate_observation_count = report
        .get("duplicate_observation_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let timeout_observation_count = report
        .get("timeout_observation_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let eligible = report
        .get("eligible_attempt_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let repository_count = report
        .get("repository_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let authorized = report
        .get("authorized_repository_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let exclusion_count = report
        .get("exclusion_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let valid_exclusion_count = report
        .get("valid_exclusion_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let mut body =
        format!("# Rust repair trust report\n\nStatus: `{status}`\nRun status: `{run_status}`\n\n");
    body.push_str("## Denominators\n\n");
    body.push_str(&format!(
        "- Attempts supplied: {attempt_count}\n- Eligible attempts: {eligible}\n- Observed runs: {observation_count}\n- Exclusions supplied: {exclusion_count}\n- Valid/unique exclusions: {valid_exclusion_count} / {unique_exclusion_count}\n- Duplicate observations: {duplicate_observation_count}\n- Timeout observations: {timeout_observation_count}\n- Repositories supplied: {repository_count}\n- Authorized repositories: {authorized}\n\n"
    ));
    body.push_str("## Movement\n\n| Movement | Count |\n| --- | ---: |\n");
    if let Some(counts) = report.get("movement_counts").and_then(Value::as_object) {
        for movement in MOVEMENTS {
            let count = counts.get(movement).and_then(Value::as_u64).unwrap_or(0);
            body.push_str(&format!("| `{movement}` | {count} |\n"));
        }
    }
    body.push_str("\n## Limitations\n\n");
    if let Some(limitations) = report.get("limitations").and_then(Value::as_array) {
        for limitation in limitations.iter().filter_map(Value::as_str) {
            body.push_str(&format!("- `{limitation}`\n"));
        }
    }
    body.push_str("\n## Exclusions\n\n");
    if let Some(reasons) = report
        .get("exclusion_reason_counts")
        .and_then(Value::as_object)
    {
        for (reason, count) in reasons {
            body.push_str(&format!("- `{reason}`: {}\n", count.as_u64().unwrap_or(0)));
        }
    }
    body.push_str("\n## Route-quality scorecard\n\n");
    if let Some(scorecard) = report.get("scorecard") {
        let one_attempt = scorecard
            .get("one_attempt_improvement_rate")
            .and_then(Value::as_f64)
            .map_or_else(|| "N/A".to_string(), |value| format!("{value:.3}"));
        let limitation_frequency = scorecard
            .get("limitation_frequency")
            .and_then(Value::as_f64)
            .map_or_else(|| "N/A".to_string(), |value| format!("{value:.3}"));
        let call_presence_limitation_frequency = scorecard
            .get("call_presence_limitation_frequency")
            .and_then(Value::as_f64)
            .map_or_else(|| "N/A".to_string(), |value| format!("{value:.3}"));
        body.push_str(&format!(
            "- One-attempt improvement rate: `{one_attempt}` ({} / {})\n- Limitation frequency: `{limitation_frequency}` ({} / {})\n- CallPresence limitation frequency: `{call_presence_limitation_frequency}` ({} / {})\n- Repair rounds counted: `{}`\n- False-actionability incidents: `{}`\n- Known-impossible recommendations: `{}`\n- Parity failures: `{}`\n- Artifact-archaeology incidents: `{}`\n",
            scorecard
                .get("one_attempt_improvement_numerator")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            scorecard
                .get("one_attempt_improvement_denominator")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            scorecard
                .get("limitation_frequency_numerator")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            scorecard
                .get("limitation_frequency_denominator")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            scorecard
                .get("call_presence_limitation_frequency_numerator")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            scorecard
                .get("call_presence_limitation_frequency_denominator")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            scorecard
                .get("repair_rounds_total")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            scorecard
                .get("false_actionability_incidents")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            scorecard
                .get("known_impossible_recommendations")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            scorecard
                .get("parity_failures")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            scorecard
                .get("artifact_archaeology_incidents")
                .and_then(Value::as_u64)
                .unwrap_or(0),
        ));
        if let Some(fields) = scorecard
            .get("missing_route_fields")
            .and_then(Value::as_object)
        {
            body.push_str("- Missing route fields: ");
            let entries = fields
                .iter()
                .map(|(field, count)| format!("`{field}`={}", count.as_u64().unwrap_or(0)))
                .collect::<Vec<_>>();
            body.push_str(&entries.join(", "));
            body.push('\n');
        }
    }
    body.push_str("\nThis report is not runtime mutation evidence, coverage evidence, or a correctness proof. Synthetic and preview cases do not satisfy the Rust corpus threshold.\n");
    body
}

#[cfg(test)]
mod tests {
    use super::{build_report, markdown_report};
    use serde_json::{Value, json};

    fn valid_attempt(
        attempt_id: &str,
        repository: &str,
        gap: &str,
        attempt_number: u64,
        movement: &str,
        limitations: Vec<&str>,
    ) -> Value {
        json!({
            "attempt_id": attempt_id,
            "repository": repository,
            "analyzed_head_sha": "0123456789abcdef0123456789abcdef01234567",
            "canonical_gap_id": gap,
            "seam_id": format!("seam:{attempt_id}"),
            "file_line": "src/lib.rs:10",
            "changed_behavior": "changed call effect",
            "missing_discriminator": "test observes the effect",
            "related_test_or_production_caller": "tests::observes_effect",
            "focused_test_intent": "assert the changed effect",
            "before_receipt": format!("target/receipts/{attempt_id}-before.json"),
            "repair_intent": "add one focused test-only assertion",
            "verification_command": "cargo test -p fixture tests::observes_effect",
            "verification_result": "passed",
            "targeted_rerun_command": "cargo xtask targeted-rerun --gap {gap}",
            "receipt_command": "cargo xtask rust-repair-trust-report",
            "inspection_command": "git diff --check",
            "after_receipt": format!("target/receipts/{attempt_id}-after.json"),
            "claim_boundary": "static route evidence only",
            "attempt_number": attempt_number,
            "changed_test_files": ["tests/observes_effect.rs"],
            "allowed_edit_surface": ["tests/observes_effect.rs"],
            "limitations": limitations,
            "source_refs": ["pr:1", "receipt:before", "receipt:after"],
            "movement": movement,
            "test_only": true,
            "production_files_changed": false,
            "false_actionability": attempt_number == 3,
            "known_impossible_recommendation": attempt_number == 4,
            "parity_failure": attempt_number == 5,
            "artifact_archaeology": attempt_number == 6
        })
    }

    #[test]
    fn empty_corpus_is_limited_and_preserves_missing_denominators() -> Result<(), String> {
        let report = build_report(&json!({
            "schema_version": "0.1",
            "kind": "rust_repair_trust_corpus",
            "authorization": {"status": "missing", "repositories": []},
            "cases": []
        }));
        if report.get("status").and_then(|value| value.as_str()) != Some("limited") {
            return Err("empty corpus must remain limited".to_string());
        }
        if report.get("attempt_count").and_then(|value| value.as_u64()) != Some(0) {
            return Err("empty corpus must preserve zero attempt denominator".to_string());
        }
        if report["scorecard"]["one_attempt_improvement_rate"]
            .as_f64()
            .is_some()
        {
            return Err("empty corpus must not invent an improvement rate".to_string());
        }
        Ok(())
    }

    #[test]
    fn malformed_attempt_cannot_enter_the_scorecard() -> Result<(), String> {
        let report = build_report(&json!({
            "schema_version": "0.1",
            "kind": "rust_repair_trust_corpus",
            "authorization": {
                "status": "complete",
                "repositories": [{"name": "example", "authorization_ref": "issue-1"}]
            },
            "cases": [{
                "attempt_id": "attempt-1",
                "repository": "example",
                "analyzed_head_sha": "not-a-commit",
                "movement": "improved",
                "attempt_number": 1,
                "test_only": false,
                "production_files_changed": false
            }]
        }));
        if report.get("status").and_then(|value| value.as_str()) != Some("limited") {
            return Err("malformed attempt must keep the report limited".to_string());
        }
        if report
            .get("eligible_attempt_count")
            .and_then(|value| value.as_u64())
            != Some(0)
        {
            return Err("malformed attempt must not enter the eligible denominator".to_string());
        }
        let limitations = report["limitations"]
            .as_array()
            .ok_or_else(|| "limitations must be an array".to_string())?;
        if !limitations
            .iter()
            .any(|value| value.as_str() == Some("no_real_rust_attempts_recorded"))
        {
            return Err("missing eligible-attempt limitation".to_string());
        }
        if report["scorecard"]["missing_route_fields"]["seam_id"] != 1 {
            return Err("missing route fields must be counted by field".to_string());
        }
        Ok(())
    }

    #[test]
    fn exclusions_are_validated_and_stay_out_of_attempt_denominators() -> Result<(), String> {
        let mut corpus: Value = serde_json::from_str(include_str!(
            "../../../metrics/rust-repair-trust/corpus.json"
        ))
        .map_err(|error| format!("parse corpus fixture: {error}"))?;
        corpus["exclusions"] = json!([
            {
                "exclusion_id": "pilot-timeout",
                "repository": "EffortlessMetrics/ub-review",
                "analyzed_head_sha": "9838259a704a5cf3748eb81af29536b99bf7cf3b",
                "source_ref": "pilot",
                "reason": "analysis_timeout",
                "evidence_ref": "target/ripr/pilot/pilot-summary.json",
                "command": "ripr pilot --timeout-ms 120000",
                "claim_boundary": "timeout is not a route result"
            },
            {
                "exclusion_id": "bad",
                "repository": "not-authorized",
                "analyzed_head_sha": "short",
                "source_ref": "pilot",
                "reason": "invented",
                "evidence_ref": "receipt",
                "command": "command",
                "claim_boundary": "boundary"
            }
        ]);

        let report = build_report(&corpus);
        if report["exclusion_count"] != 2 || report["valid_exclusion_count"] != 1 {
            return Err("only complete exclusions should be accepted".to_string());
        }
        if report["exclusion_reason_counts"]["analysis_timeout"] != 1 {
            return Err("valid exclusion reasons must be counted".to_string());
        }
        if report["eligible_attempt_count"] != 0 {
            return Err("exclusions must not enter the attempt denominator".to_string());
        }
        let errors = report["validation_errors"]
            .as_array()
            .ok_or_else(|| "validation_errors must be an array".to_string())?;
        if !errors
            .iter()
            .filter_map(Value::as_str)
            .any(|error| error.contains("exclusions[1]") && error.contains("not authorized"))
        {
            return Err("invalid exclusion must retain its repository error".to_string());
        }
        Ok(())
    }

    #[test]
    fn repeated_observations_do_not_inflate_unique_exclusions_or_timeouts() -> Result<(), String> {
        let corpus: Value = serde_json::from_str(include_str!(
            "../../../metrics/rust-repair-trust/corpus.json"
        ))
        .map_err(|error| format!("parse corpus fixture: {error}"))?;
        let report = build_report(&corpus);
        for (field, expected) in [
            ("observation_count", 19),
            ("unique_exclusion_count", 18),
            ("duplicate_observation_count", 1),
            ("timeout_observation_count", 4),
            ("eligible_attempt_count", 0),
            ("repository_count", 3),
        ] {
            if report.get(field).and_then(Value::as_u64) != Some(expected) {
                return Err(format!("{field} must be {expected}: {}", report[field]));
            }
        }
        if report["observation_classification_counts"]["new_exclusion"] != 5 {
            return Err("five follow-up observations must map to new exclusions".to_string());
        }
        if report["observation_classification_counts"]["duplicate_observation"] != 1 {
            return Err("the repeated #747 observation must remain a duplicate".to_string());
        }
        Ok(())
    }

    #[test]
    fn observations_require_non_empty_authorized_head_allowlists() -> Result<(), String> {
        let mut corpus: Value = serde_json::from_str(include_str!(
            "../../../metrics/rust-repair-trust/corpus.json"
        ))
        .map_err(|error| format!("parse corpus fixture: {error}"))?;
        corpus["authorization"]["repositories"][0]["authorized_observation_heads"] = json!([]);

        let report = build_report(&corpus);
        let observation_count = corpus
            .get("observations")
            .and_then(Value::as_array)
            .ok_or_else(|| "observations must be an array".to_string())?
            .len() as u64;
        if report["valid_observation_count"]
            .as_u64()
            .is_some_and(|count| count >= observation_count)
        {
            return Err(
                "an observation must not count without an authorized head allowlist".to_string(),
            );
        }
        let errors = report["validation_errors"]
            .as_array()
            .ok_or_else(|| "validation_errors must be an array".to_string())?;
        if !errors
            .iter()
            .filter_map(Value::as_str)
            .any(|error| error.contains("empty authorized observation head allowlist"))
        {
            return Err("missing empty observation-head allowlist error".to_string());
        }
        Ok(())
    }

    #[test]
    fn duplicate_observations_require_valid_exclusion_targets() -> Result<(), String> {
        let mut corpus: Value = serde_json::from_str(include_str!(
            "../../../metrics/rust-repair-trust/corpus.json"
        ))
        .map_err(|error| format!("parse corpus fixture: {error}"))?;
        let exclusions = corpus
            .get_mut("exclusions")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| "exclusions must be an array".to_string())?;
        exclusions.push(json!({
            "exclusion_id": "invalid-target",
            "repository": "EffortlessMetrics/ub-review",
            "analyzed_head_sha": "98aea1868f92c6c0ffe89d9faae83fba11de3019",
            "source_ref": "pilot",
            "reason": "not-a-governed-reason",
            "evidence_ref": "target/ripr/pilot/invalid.json",
            "command": "ripr pilot",
            "claim_boundary": "invalid exclusion"
        }));
        let duplicate_source = corpus
            .get("observations")
            .and_then(Value::as_array)
            .and_then(|observations| observations.get(5))
            .cloned()
            .ok_or_else(|| "expected duplicate observation fixture".to_string())?;
        let mut duplicate = duplicate_source;
        duplicate["observation_id"] = json!("duplicate-invalid-target");
        duplicate["duplicate_of"] = json!("invalid-target");
        corpus
            .get_mut("observations")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| "observations must be an array".to_string())?
            .push(duplicate);

        let report = build_report(&corpus);
        if report["duplicate_observation_count"] != 1 {
            return Err("duplicate of an invalid exclusion must not count".to_string());
        }
        let errors = report["validation_errors"]
            .as_array()
            .ok_or_else(|| "validation_errors must be an array".to_string())?;
        if !errors
            .iter()
            .filter_map(Value::as_str)
            .any(|error| error.contains("duplicate_of invalid-target names an invalid exclusion"))
        {
            return Err("missing invalid duplicate target error".to_string());
        }
        Ok(())
    }

    #[test]
    fn invalid_attempt_fields_fail_closed_without_entering_denominators() -> Result<(), String> {
        let mut corpus: Value = serde_json::from_str(include_str!(
            "../../../metrics/rust-repair-trust/corpus.json"
        ))
        .map_err(|error| format!("parse corpus fixture: {error}"))?;
        corpus["cases"] = json!([{
            "attempt_id": "invalid-1",
            "repository": "not-authorized",
            "analyzed_head_sha": "not-a-sha",
            "canonical_gap_id": "wrong-prefix",
            "seam_id": "seam:invalid-1",
            "file_line": "src/lib.rs:10",
            "changed_behavior": "changed behavior",
            "missing_discriminator": "missing discriminator",
            "related_test_or_production_caller": "caller",
            "focused_test_intent": "observe behavior",
            "before_receipt": "before.json",
            "repair_intent": "add assertion",
            "verification_command": "cargo test",
            "verification_result": "failed",
            "targeted_rerun_command": "cargo xtask targeted-rerun",
            "receipt_command": "cargo xtask receipt",
            "inspection_command": "git diff --check",
            "after_receipt": "after.json",
            "claim_boundary": "static evidence",
            "attempt_number": 0,
            "changed_test_files": [],
            "allowed_edit_surface": [""],
            "limitations": "not-an-array",
            "source_refs": [],
            "movement": "invented",
            "test_only": false,
            "production_files_changed": true,
            "false_actionability": "unknown",
            "known_impossible_recommendation": false,
            "parity_failure": false,
            "artifact_archaeology": false
        }]);

        let report = build_report(&corpus);
        if report["eligible_attempt_count"] != 0 {
            return Err("invalid attempt must not enter the eligible denominator".to_string());
        }
        let errors = report["validation_errors"]
            .as_array()
            .ok_or_else(|| "validation_errors must be an array".to_string())?;
        let errors = errors.iter().filter_map(Value::as_str).collect::<Vec<_>>();
        for expected in [
            "repository not-authorized is not authorized",
            "revision must be a 40-character commit SHA",
            "movement invented is not in the closed vocabulary",
            "attempt_number must be a positive integer",
            "changed_test_files must not be empty",
            "limitations must be an array",
            "source_refs must not be empty",
            "test_only must be true",
            "production_files_changed must be false",
            "canonical_gap_id must start with gap:",
        ] {
            if !errors.iter().any(|error| error.contains(expected)) {
                return Err(format!("missing validation error: {expected}"));
            }
        }
        Ok(())
    }

    #[test]
    fn complete_corpus_scores_metrics_and_markdown_with_explicit_denominators() -> Result<(), String>
    {
        let repositories = [
            "EffortlessMetrics/ripr-swarm",
            "EffortlessMetrics/perl-lsp-swarm",
            "EffortlessMetrics/ub-review",
        ];
        let mut cases = Vec::new();
        for index in 0..20u64 {
            let repository = repositories[(index as usize) % repositories.len()];
            let movement = match index % 5 {
                0 => "improved",
                1 => "closed",
                2 => "unchanged",
                3 => "regressed",
                _ => "limited",
            };
            let limitations = if index == 7 {
                vec!["call_presence_effect_observer_unresolved"]
            } else if index == 8 {
                vec!["selected_scope_parity_unknown"]
            } else {
                Vec::new()
            };
            cases.push(valid_attempt(
                &format!("attempt-{index}"),
                repository,
                &format!("gap:behavior-{}", index % 3),
                (index % 3) + 1,
                movement,
                limitations,
            ));
        }
        let mut corpus: Value = serde_json::from_str(include_str!(
            "../../../metrics/rust-repair-trust/corpus.json"
        ))
        .map_err(|error| format!("parse corpus fixture: {error}"))?;
        corpus["cases"] = Value::Array(cases);

        let report = build_report(&corpus);
        if report.get("status").and_then(Value::as_str) != Some("complete") {
            return Err(format!("complete corpus did not complete: {report}"));
        }
        if report.get("eligible_attempt_count").and_then(Value::as_u64) != Some(20) {
            return Err("complete corpus must count all 20 eligible attempts".to_string());
        }
        if report["scorecard"]["limitation_frequency_denominator"] != 20 {
            return Err("limitation frequency must expose the attempt denominator".to_string());
        }
        if report["scorecard"]["call_presence_limitation_frequency_numerator"] != 1 {
            return Err("CallPresence limitation frequency must count its numerator".to_string());
        }
        if report["scorecard"]["one_attempt_improvement_denominator"] != 3 {
            return Err(
                "one-attempt improvement must expose the gap-group denominator".to_string(),
            );
        }
        let markdown = markdown_report(&report);
        if !markdown.contains("CallPresence limitation frequency")
            || !markdown.contains("Status: `complete`")
        {
            return Err("Markdown must expose complete status and CallPresence metric".to_string());
        }
        Ok(())
    }
}
