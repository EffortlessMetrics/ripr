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
const OBSERVATION_CLASSIFICATIONS: [&str; 3] = [
    "new_exclusion",
    "duplicate_observation",
    "selected_opportunity",
];
const FORBIDDEN_CORPUS_SUMMARY_KEYS: [&str; 3] = ["route_yield", "repair_success", "route_ladder"];
const ROUTE_CHANNELS: [&str; 2] = ["cli", "editor"];
const ROUTE_UNITS: [&str; 2] = ["opportunity", "repository_observation"];
const ELIGIBILITY_STATES: [&str; 3] = ["admitted", "rejected", "not_observed"];
const FOCUSED_TEST_RESULTS: [&str; 4] = ["passed", "failed", "not_run", "not_observed"];
const EARLIEST_STOPS: [&str; 14] = [
    "unsupported_or_incomplete_input",
    "analysis_timeout",
    "no_canonical_gap",
    "missing_discriminator",
    "missing_related_test",
    "ambiguous_or_unsafe_target",
    "missing_fix_site",
    "missing_verify_route",
    "stale_or_wrong_identity",
    "artifact_archaeology_required",
    "client_or_install_failure",
    "infrastructure_tempfail",
    "not_proven",
    "not_observed",
];
const ROUTE_BOOL_FIELDS: [&str; 9] = [
    "analysis_completed",
    "canonical_gap_identified",
    "complete_route_admitted",
    "packet_ready",
    "attempt_authorized",
    "attempt_started",
    "attempt_finished",
    "correct_route_reviewed",
    "artifact_archaeology",
];
const ROUTE_STRING_FIELDS: [&str; 10] = [
    "opportunity_id",
    "channel",
    "cohort_id",
    "analyzer_generation",
    "unit",
    "canonical_eligibility",
    "earliest_stop",
    "static_movement",
    "focused_test_result",
    "repair_attempt_id",
];
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
    let report = rust_repair_trust_report_value_at(Path::new(CORPUS_PATH))?;
    let json_body = serde_json::to_string_pretty(&report)
        .map_err(|error| format!("serialize Rust repair trust report: {error}"))?;
    crate::write_report("rust-repair-trust.json", &format!("{json_body}\n"))?;
    crate::write_report("rust-repair-trust.md", &markdown_report(&report))
}

pub(crate) fn rust_repair_trust_report_value_at(path: &Path) -> Result<Value, String> {
    let corpus = read_corpus(path)?;
    Ok(build_report(&corpus))
}

fn read_corpus(path: &Path) -> Result<Value, String> {
    let body =
        fs::read_to_string(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    serde_json::from_str(&body).map_err(|error| format!("parse {}: {error}", path.display()))
}

fn measurable_ratio(numerator: u64, denominator: u64, unit: &str) -> Value {
    if denominator == 0 {
        json!({
            "status": "not_measurable",
            "numerator": Value::Null,
            "denominator": 0,
            "unit": unit,
            "display": "not_measurable"
        })
    } else {
        json!({
            "status": "measured",
            "numerator": numerator,
            "denominator": denominator,
            "unit": unit,
            "display": format!("{numerator}/{denominator}")
        })
    }
}

fn route_object(value: &Value) -> Option<&Value> {
    value.get("route").filter(|route| !route.is_null())
}

fn route_str<'a>(observation: &'a Value, field: &str) -> Option<&'a str> {
    route_object(observation)?
        .get(field)?
        .as_str()
        .filter(|value| !value.is_empty())
}

fn route_bool(observation: &Value, field: &str) -> Option<bool> {
    route_object(observation)?.get(field)?.as_bool()
}

fn opportunity_identity(observation: &Value) -> String {
    if let Some(opportunity_id) = route_str(observation, "opportunity_id") {
        return format!("opportunity:{opportunity_id}");
    }
    let repository = observation
        .get("repository")
        .and_then(Value::as_str)
        .unwrap_or("");
    let sha = observation
        .get("analyzed_head_sha")
        .and_then(Value::as_str)
        .unwrap_or("");
    let candidate = observation
        .get("canonical_candidate_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    format!("identity:{repository}|{sha}|{candidate}")
}

fn selected_opportunity_key(observation: &Value) -> String {
    let opportunity = opportunity_identity(observation);
    match route_str(observation, "cohort_id") {
        Some(cohort) => format!("{opportunity}|cohort:{cohort}"),
        None => format!("{opportunity}|cohort:historical"),
    }
}

fn normalize_earliest_stop(reason: &str, explicit: Option<&str>) -> String {
    if let Some(stop) = explicit
        && EARLIEST_STOPS.contains(&stop)
    {
        return stop.to_string();
    }
    if reason == "analysis_timeout" {
        "analysis_timeout".to_string()
    } else {
        "not_observed".to_string()
    }
}

fn observation_is_complete_route(observation: &Value) -> bool {
    route_bool(observation, "complete_route_admitted").unwrap_or(false)
        && route_str(observation, "canonical_eligibility") != Some("rejected")
}

fn focused_test_bucket(result: &str) -> Option<&'static str> {
    match result {
        "passed" => Some("passed"),
        "failed" => Some("failed"),
        "not_run" => Some("not_run"),
        _ => None,
    }
}

fn focused_test_rank(result: &str) -> u8 {
    match result {
        "failed" => 3,
        "not_run" => 2,
        "passed" => 1,
        _ => 0,
    }
}

fn merge_focused_test(current: &mut Option<String>, incoming: Option<&str>) {
    let Some(incoming) = incoming else {
        return;
    };
    let Some(bucket) = focused_test_bucket(incoming) else {
        return;
    };
    if current.as_deref().map(focused_test_rank).unwrap_or(0) < focused_test_rank(bucket) {
        *current = Some(bucket.to_string());
    }
}

fn merge_archaeology(current: &mut Option<bool>, incoming: Option<bool>) {
    match (*current, incoming) {
        (_, None) => {}
        (_, Some(true)) => *current = Some(true),
        (None | Some(false), Some(false)) => *current = Some(false),
        (Some(true), Some(false)) => {}
    }
}

fn case_selected_opportunity_key(case: &Value) -> String {
    let repository = case.get("repository").and_then(Value::as_str).unwrap_or("");
    let sha = case
        .get("analyzed_head_sha")
        .and_then(Value::as_str)
        .unwrap_or("");
    let gap = case
        .get("canonical_gap_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    format!("identity:{repository}|{sha}|{gap}|cohort:historical")
}

fn observation_identity_matches_case(observation: &Value, case: &Value) -> bool {
    observation.get("repository") == case.get("repository")
        && observation.get("analyzed_head_sha") == case.get("analyzed_head_sha")
}

fn linked_case_opportunity_key(
    case: &Value,
    observations: &[&Value],
    validation_errors: &mut Vec<String>,
) -> String {
    let attempt_id = case.get("attempt_id").and_then(Value::as_str).unwrap_or("");
    let named = observations
        .iter()
        .copied()
        .filter(|observation| route_str(observation, "repair_attempt_id") == Some(attempt_id))
        .collect::<Vec<_>>();
    if named.is_empty() {
        return case_selected_opportunity_key(case);
    }
    for observation in &named {
        if !observation_identity_matches_case(observation, case) {
            validation_errors.push(format!(
                "repair_attempt_id {attempt_id} does not match observation repository and analyzed head"
            ));
        }
    }
    let matching_keys = named
        .iter()
        .filter(|observation| observation_identity_matches_case(observation, case))
        .map(|observation| selected_opportunity_key(observation))
        .collect::<BTreeSet<_>>();
    if matching_keys.len() == 1
        && let Some(key) = matching_keys.iter().next()
    {
        return key.clone();
    }
    if matching_keys.len() > 1 {
        validation_errors.push(format!(
            "repair_attempt_id {attempt_id} names multiple selected opportunities"
        ));
    }
    case_selected_opportunity_key(case)
}

fn normalize_ladder(ladder: &mut OpportunityLadder) {
    if !ladder.analysis_completed {
        ladder.canonical_gap_identified = false;
    }
    if !ladder.canonical_gap_identified {
        ladder.complete_route = false;
    }
    if !ladder.complete_route {
        ladder.attempt_authorized = false;
    }
    if !ladder.attempt_authorized {
        ladder.attempt_started = false;
    }
    if !ladder.attempt_started {
        ladder.attempt_finished = false;
    }
    if !ladder.attempt_finished {
        ladder.static_improved_or_closed = false;
    }
}

#[derive(Default)]
struct OpportunityLadder {
    analysis_completed: bool,
    canonical_gap_identified: bool,
    complete_route: bool,
    attempt_authorized: bool,
    attempt_started: bool,
    attempt_finished: bool,
    static_improved_or_closed: bool,
    artifact_archaeology: Option<bool>,
    correct_route_reviewed: bool,
    focused_test: Option<String>,
    earliest_stop: String,
    channels: BTreeSet<String>,
    cohort: String,
}

fn absorb_observation(ladder: &mut OpportunityLadder, observation: &Value) {
    ladder.analysis_completed |= route_bool(observation, "analysis_completed").unwrap_or(false);
    ladder.canonical_gap_identified |=
        route_bool(observation, "canonical_gap_identified").unwrap_or(false);
    ladder.complete_route |= observation_is_complete_route(observation);
    ladder.attempt_authorized |= route_bool(observation, "attempt_authorized").unwrap_or(false);
    ladder.attempt_started |= route_bool(observation, "attempt_started").unwrap_or(false);
    ladder.attempt_finished |= route_bool(observation, "attempt_finished").unwrap_or(false);
    if matches!(
        route_str(observation, "static_movement"),
        Some("improved" | "closed")
    ) {
        ladder.static_improved_or_closed = true;
    }
    merge_archaeology(
        &mut ladder.artifact_archaeology,
        route_bool(observation, "artifact_archaeology"),
    );
    ladder.correct_route_reviewed |=
        route_bool(observation, "correct_route_reviewed").unwrap_or(false);
    merge_focused_test(
        &mut ladder.focused_test,
        route_str(observation, "focused_test_result"),
    );
    if let Some(channel) = route_str(observation, "channel") {
        ladder.channels.insert(channel.to_string());
    }
    if ladder.cohort.is_empty() {
        ladder.cohort = route_str(observation, "cohort_id")
            .unwrap_or("historical")
            .to_string();
    }
    let stop = normalize_earliest_stop(
        observation
            .get("reason")
            .and_then(Value::as_str)
            .unwrap_or(""),
        route_str(observation, "earliest_stop"),
    );
    if ladder.earliest_stop.is_empty() || ladder.earliest_stop == "not_observed" {
        ladder.earliest_stop = stop;
    }
}

fn absorb_case(ladder: &mut OpportunityLadder, case: &Value) {
    ladder.analysis_completed = true;
    ladder.canonical_gap_identified = true;
    ladder.complete_route = true;
    ladder.attempt_authorized = true;
    ladder.attempt_started = true;
    ladder.attempt_finished = true;
    if matches!(
        case.get("movement").and_then(Value::as_str),
        Some("improved" | "closed")
    ) {
        ladder.static_improved_or_closed = true;
    }
    merge_archaeology(
        &mut ladder.artifact_archaeology,
        case.get("artifact_archaeology").and_then(Value::as_bool),
    );
    merge_focused_test(
        &mut ladder.focused_test,
        case.get("verification_result").and_then(Value::as_str),
    );
}

fn build_report(corpus: &Value) -> Value {
    let mut validation_errors = Vec::new();
    if let Some(object) = corpus.as_object() {
        for key in FORBIDDEN_CORPUS_SUMMARY_KEYS {
            if object.contains_key(key) {
                validation_errors.push(format!(
                    "hand-edited derived total `{key}` cannot enter trusted success counts"
                ));
            }
        }
    }
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
    let mut valid_case_rows = Vec::new();

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
            valid_case_rows.push(case);
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
    let mut seen_canonical_candidate_ids = BTreeMap::<String, usize>::new();
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
        if let Some(candidate_id) = exclusion
            .get("canonical_candidate_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|candidate_id| !candidate_id.is_empty())
        {
            if let Some(first) = seen_canonical_candidate_ids.get(candidate_id) {
                errors.push(format!(
                    "canonical_candidate_id {candidate_id} duplicates exclusions[{first}]"
                ));
            } else {
                seen_canonical_candidate_ids.insert(candidate_id.to_string(), index);
            }
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
    let mut valid_observation_rows = Vec::new();
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
            valid_observation_rows.push(observation);
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

    let mut opportunity_ladders = BTreeMap::<String, OpportunityLadder>::new();
    for observation in &valid_observation_rows {
        let key = selected_opportunity_key(observation);
        absorb_observation(opportunity_ladders.entry(key).or_default(), observation);
    }
    for case in &valid_case_rows {
        let key =
            linked_case_opportunity_key(case, &valid_observation_rows, &mut validation_errors);
        absorb_case(opportunity_ladders.entry(key).or_default(), case);
    }
    for ladder in opportunity_ladders.values_mut() {
        normalize_ladder(ladder);
    }

    let selected_opportunities = opportunity_ladders.len() as u64;
    let analysis_completed = opportunity_ladders
        .values()
        .filter(|ladder| ladder.analysis_completed)
        .count() as u64;
    let canonical_behavior_or_gap_identified = opportunity_ladders
        .values()
        .filter(|ladder| ladder.canonical_gap_identified)
        .count() as u64;
    let complete_routes_admitted = opportunity_ladders
        .values()
        .filter(|ladder| ladder.complete_route)
        .count() as u64;
    let attempts_authorized_or_eligible = opportunity_ladders
        .values()
        .filter(|ladder| ladder.attempt_authorized)
        .count() as u64;
    let attempts_started = opportunity_ladders
        .values()
        .filter(|ladder| ladder.attempt_started)
        .count() as u64;
    let attempts_finished = opportunity_ladders
        .values()
        .filter(|ladder| ladder.attempt_finished)
        .count() as u64;
    let static_improved_or_closed = opportunity_ladders
        .values()
        .filter(|ladder| ladder.static_improved_or_closed)
        .count() as u64;
    let correct_routes_reviewed = opportunity_ladders
        .values()
        .filter(|ladder| ladder.complete_route && ladder.correct_route_reviewed)
        .count() as u64;
    let archaeology_observed = opportunity_ladders
        .values()
        .filter(|ladder| ladder.complete_route && ladder.artifact_archaeology.is_some())
        .count() as u64;
    let completion_without_archaeology = opportunity_ladders
        .values()
        .filter(|ladder| ladder.complete_route && ladder.artifact_archaeology == Some(false))
        .count() as u64;
    let mut earliest_stop_counts = BTreeMap::<String, usize>::new();
    for ladder in opportunity_ladders.values() {
        if ladder.complete_route {
            continue;
        }
        let stop = if ladder.earliest_stop.is_empty() {
            "not_observed"
        } else {
            ladder.earliest_stop.as_str()
        };
        *earliest_stop_counts.entry(stop.to_string()).or_default() += 1;
    }
    let mut channel_observation_counts = BTreeMap::<String, usize>::new();
    for observation in &valid_observation_rows {
        if let Some(channel) = route_str(observation, "channel") {
            *channel_observation_counts
                .entry(channel.to_string())
                .or_default() += 1;
        }
    }
    let cohort_count = opportunity_ladders
        .values()
        .map(|ladder| {
            if ladder.cohort.is_empty() {
                "historical"
            } else {
                ladder.cohort.as_str()
            }
        })
        .collect::<BTreeSet<_>>()
        .len() as u64;
    let mut focused_test_execution = BTreeMap::<&str, u64>::new();
    for key in ["passed", "failed", "not_run", "not_observed"] {
        focused_test_execution.insert(key, 0);
    }
    for ladder in opportunity_ladders.values() {
        if let Some(bucket) = ladder.focused_test.as_deref().and_then(focused_test_bucket) {
            *focused_test_execution.entry(bucket).or_default() += 1;
        } else {
            *focused_test_execution.entry("not_observed").or_default() += 1;
        }
    }
    let repair_success_numerator =
        (*movements.get("improved").unwrap_or(&0) + *movements.get("closed").unwrap_or(&0)) as u64;
    let route_yield = measurable_ratio(
        complete_routes_admitted,
        selected_opportunities,
        "complete_routes / selected_opportunities",
    );
    let repair_success = measurable_ratio(
        repair_success_numerator,
        eligible_attempts as u64,
        "improved_or_closed / eligible_attempts",
    );
    let correct_routes = measurable_ratio(
        correct_routes_reviewed,
        complete_routes_admitted,
        "independently_reviewed_correct / complete_routes",
    );
    let completion_without_hidden_help = measurable_ratio(
        completion_without_archaeology,
        archaeology_observed,
        "completed_without_artifact_archaeology / complete_routes_with_observed_archaeology",
    );

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

    let mut report = json!({
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
            "A faster rerun without selected-scope parity is limited.",
            "Observations are not attempts; zero eligible attempts makes repair success not_measurable."
        ]
    });
    if let Some(object) = report.as_object_mut() {
        object.insert(
            "unique_opportunity_count".to_string(),
            Value::from(selected_opportunities),
        );
        object.insert("cohort_count".to_string(), Value::from(cohort_count));
        object.insert(
            "channel_observation_counts".to_string(),
            json!(channel_observation_counts),
        );
        object.insert(
            "earliest_stop_counts".to_string(),
            json!(earliest_stop_counts),
        );
        object.insert(
            "route_ladder".to_string(),
            json!({
                "selected_opportunities": selected_opportunities,
                "analysis_completed": analysis_completed,
                "canonical_behavior_or_gap_identified": canonical_behavior_or_gap_identified,
                "complete_routes_admitted": complete_routes_admitted,
                "attempts_authorized_or_eligible": attempts_authorized_or_eligible,
                "attempts_started": attempts_started,
                "attempts_finished": attempts_finished,
                "static_improved_or_closed": static_improved_or_closed
            }),
        );
        object.insert("route_yield".to_string(), route_yield);
        object.insert("repair_success".to_string(), repair_success);
        object.insert("correct_routes".to_string(), correct_routes);
        object.insert(
            "completion_without_hidden_help".to_string(),
            completion_without_hidden_help,
        );
        object.insert(
            "focused_test_execution".to_string(),
            json!(focused_test_execution),
        );
    }
    report
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
    if let Some(route) = observation.get("route") {
        errors.extend(route_field_errors(route));
    }
    errors
}

fn route_field_errors(route: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    let Some(object) = route.as_object() else {
        errors.push("route must be an object".to_string());
        return errors;
    };
    for key in object.keys() {
        if !ROUTE_STRING_FIELDS.contains(&key.as_str())
            && !ROUTE_BOOL_FIELDS.contains(&key.as_str())
        {
            errors.push(format!("route field {key} is not in the route vocabulary"));
        }
    }
    for field in ROUTE_STRING_FIELDS {
        match object.get(field) {
            None => {}
            Some(Value::String(value)) if !value.is_empty() => {}
            Some(_) => errors.push(format!("route.{field} must be a non-empty string")),
        }
    }
    for field in ROUTE_BOOL_FIELDS {
        match object.get(field) {
            None | Some(Value::Bool(_)) => {}
            Some(_) => errors.push(format!("route.{field} must be a boolean")),
        }
    }
    if let Some(channel) = object.get("channel").and_then(Value::as_str)
        && !ROUTE_CHANNELS.contains(&channel)
    {
        errors.push(format!(
            "route.channel {channel} is not in the channel vocabulary"
        ));
    }
    if let Some(unit) = object.get("unit").and_then(Value::as_str)
        && !ROUTE_UNITS.contains(&unit)
    {
        errors.push(format!("route.unit {unit} is not in the unit vocabulary"));
    }
    if let Some(eligibility) = object.get("canonical_eligibility").and_then(Value::as_str)
        && !ELIGIBILITY_STATES.contains(&eligibility)
    {
        errors.push(format!(
            "route.canonical_eligibility {eligibility} is not in the eligibility vocabulary"
        ));
    }
    if let Some(stop) = object.get("earliest_stop").and_then(Value::as_str)
        && !EARLIEST_STOPS.contains(&stop)
    {
        errors.push(format!(
            "route.earliest_stop {stop} is not in the earliest-stop vocabulary"
        ));
    }
    if let Some(movement) = object.get("static_movement").and_then(Value::as_str)
        && movement != "not_observed"
        && !MOVEMENTS.contains(&movement)
    {
        errors.push(format!(
            "route.static_movement {movement} is not in the movement vocabulary"
        ));
    }
    if let Some(result) = object.get("focused_test_result").and_then(Value::as_str)
        && !FOCUSED_TEST_RESULTS.contains(&result)
    {
        errors.push(format!(
            "route.focused_test_result {result} is not in the focused-test vocabulary"
        ));
    }
    const STAGE_IMPLICATIONS: [(&str, &str); 6] = [
        ("canonical_gap_identified", "analysis_completed"),
        ("complete_route_admitted", "analysis_completed"),
        ("complete_route_admitted", "canonical_gap_identified"),
        ("attempt_authorized", "complete_route_admitted"),
        ("attempt_started", "attempt_authorized"),
        ("attempt_finished", "attempt_started"),
    ];
    for (downstream, upstream) in STAGE_IMPLICATIONS {
        if object.get(downstream) == Some(&Value::Bool(true))
            && object.get(upstream) == Some(&Value::Bool(false))
        {
            errors.push(format!(
                "route.{downstream} cannot be true while route.{upstream} is false"
            ));
        }
    }
    if matches!(
        object.get("static_movement").and_then(Value::as_str),
        Some("improved" | "closed")
    ) && object.get("attempt_finished") == Some(&Value::Bool(false))
    {
        errors.push(
            "route.static_movement improved/closed cannot be claimed while route.attempt_finished is false"
                .to_string(),
        );
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
    body.push_str("## Route-yield ladder\n\n");
    if let Some(ladder) = report.get("route_ladder") {
        body.push_str(&format!(
            "- Selected opportunities: {}\n- Analysis completed: {}\n- Canonical behavior/gap identified: {}\n- Complete routes admitted: {}\n- Attempts authorized/eligible: {}\n- Attempts started: {}\n- Attempts finished: {}\n- Static improved/closed: {}\n",
            ladder
                .get("selected_opportunities")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            ladder
                .get("analysis_completed")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            ladder
                .get("canonical_behavior_or_gap_identified")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            ladder
                .get("complete_routes_admitted")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            ladder
                .get("attempts_authorized_or_eligible")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            ladder
                .get("attempts_started")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            ladder
                .get("attempts_finished")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            ladder
                .get("static_improved_or_closed")
                .and_then(Value::as_u64)
                .unwrap_or(0),
        ));
    }
    let route_yield_display = report
        .pointer("/route_yield/display")
        .and_then(Value::as_str)
        .unwrap_or("not_measurable");
    let repair_success_display = report
        .pointer("/repair_success/display")
        .and_then(Value::as_str)
        .unwrap_or("not_measurable");
    let route_yield_unit = report
        .pointer("/route_yield/unit")
        .and_then(Value::as_str)
        .unwrap_or("complete_routes / selected_opportunities");
    let repair_success_unit = report
        .pointer("/repair_success/unit")
        .and_then(Value::as_str)
        .unwrap_or("improved_or_closed / eligible_attempts");
    body.push_str(&format!(
        "- Route yield: `{route_yield_display}` ({route_yield_unit})\n- Repair success: `{repair_success_display}` ({repair_success_unit})\n"
    ));
    if let Some(stops) = report
        .get("earliest_stop_counts")
        .and_then(Value::as_object)
    {
        body.push_str("- Earliest stops:\n");
        for (reason, count) in stops {
            body.push_str(&format!(
                "  - `{reason}`: {}\n",
                count.as_u64().unwrap_or(0)
            ));
        }
    }
    body.push('\n');
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
    use super::{build_report, markdown_report, rust_repair_trust_report_value_at};
    use serde_json::{Value, json};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

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
            ("observation_count", 28),
            ("unique_exclusion_count", 24),
            ("duplicate_observation_count", 4),
            ("timeout_observation_count", 6),
            ("eligible_attempt_count", 0),
            ("repository_count", 3),
        ] {
            if report.get(field).and_then(Value::as_u64) != Some(expected) {
                return Err(format!("{field} must be {expected}: {}", report[field]));
            }
        }
        if report["observation_classification_counts"]["new_exclusion"] != 11 {
            return Err(
                "eleven follow-up/pilot observations must map to new exclusions".to_string(),
            );
        }
        if report["observation_classification_counts"]["duplicate_observation"] != 4 {
            return Err(
                "the repeated #747 observation plus three post-#2933 FieldConstruction reruns must remain duplicates"
                    .to_string(),
            );
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

    fn live_corpus() -> Result<Value, String> {
        serde_json::from_str(include_str!(
            "../../../metrics/rust-repair-trust/corpus.json"
        ))
        .map_err(|error| format!("parse corpus fixture: {error}"))
    }

    fn fieldconstruction_pilot_exclusion<'a>(
        corpus: &'a Value,
        exclusion_id: &str,
    ) -> Result<&'a Value, String> {
        corpus
            .get("exclusions")
            .and_then(Value::as_array)
            .and_then(|exclusions| {
                exclusions.iter().find(|exclusion| {
                    exclusion.get("exclusion_id").and_then(Value::as_str) == Some(exclusion_id)
                })
            })
            .ok_or_else(|| format!("missing exclusion {exclusion_id}"))
    }

    fn duplicate_observation_from_exclusion(
        exclusion: &Value,
        observation_id: &str,
        reason: &str,
    ) -> Result<Value, String> {
        let exclusion_id = exclusion
            .get("exclusion_id")
            .and_then(Value::as_str)
            .ok_or_else(|| "exclusion_id required".to_string())?;
        let repository = exclusion
            .get("repository")
            .and_then(Value::as_str)
            .ok_or_else(|| "repository required".to_string())?;
        let analyzed_head_sha = exclusion
            .get("analyzed_head_sha")
            .and_then(Value::as_str)
            .ok_or_else(|| "analyzed_head_sha required".to_string())?;
        let source_ref = exclusion
            .get("source_ref")
            .and_then(Value::as_str)
            .ok_or_else(|| "source_ref required".to_string())?;
        let canonical_candidate_id = exclusion
            .get("canonical_candidate_id")
            .and_then(Value::as_str)
            .ok_or_else(|| "canonical_candidate_id required".to_string())?;
        Ok(json!({
            "observation_id": observation_id,
            "repository": repository,
            "analyzed_head_sha": analyzed_head_sha,
            "source_ref": source_ref,
            "canonical_candidate_id": canonical_candidate_id,
            "reason": reason,
            "evidence_ref": format!("metrics/rust-repair-trust/{observation_id}.json"),
            "classification": "duplicate_observation",
            "duplicate_of": exclusion_id,
            "claim_boundary": "identity-matched rerun is not a new exclusion or attempt"
        }))
    }

    #[test]
    fn fieldconstruction_identity_reruns_do_not_inflate_cases_or_exclusions() -> Result<(), String>
    {
        let mut corpus = live_corpus()?;
        let baseline = build_report(&corpus);
        let baseline_unique = baseline["unique_exclusion_count"]
            .as_u64()
            .ok_or_else(|| "unique_exclusion_count must be a number".to_string())?;
        let baseline_duplicates = baseline["duplicate_observation_count"]
            .as_u64()
            .ok_or_else(|| "duplicate_observation_count must be a number".to_string())?;
        let baseline_timeouts = baseline["timeout_observation_count"]
            .as_u64()
            .ok_or_else(|| "timeout_observation_count must be a number".to_string())?;
        let identities = [
            (
                "ripr-1580-static-limitation",
                "identity-control-ripr-1580",
                "analysis_timeout",
            ),
            (
                "ub-772-static-limitation",
                "identity-control-ub-772",
                "no_current_behavior_change",
            ),
            (
                "ub-744-static-limitation",
                "identity-control-ub-744",
                "analysis_timeout",
            ),
        ];
        let mut added_timeout_duplicates = 0u64;
        for (exclusion_id, observation_id, reason) in identities {
            let exclusion = fieldconstruction_pilot_exclusion(&corpus, exclusion_id)?.clone();
            let observation =
                duplicate_observation_from_exclusion(&exclusion, observation_id, reason)?;
            if reason == "analysis_timeout" {
                added_timeout_duplicates += 1;
            }
            corpus
                .get_mut("observations")
                .and_then(Value::as_array_mut)
                .ok_or_else(|| "observations must be an array".to_string())?
                .push(observation);
        }

        let report = build_report(&corpus);
        if report["unique_exclusion_count"] != baseline_unique {
            return Err(format!(
                "identity-matched reruns must not add exclusions: {}",
                report["unique_exclusion_count"]
            ));
        }
        if report["eligible_attempt_count"] != 0 {
            return Err(
                "identity-matched reruns must not enter the attempt denominator".to_string(),
            );
        }
        if corpus.get("cases").and_then(Value::as_array).map(Vec::len) != Some(0) {
            return Err("identity-matched reruns must not inflate cases".to_string());
        }
        if report["duplicate_observation_count"].as_u64() != Some(baseline_duplicates + 3) {
            return Err(format!(
                "three identity-matched reruns must count as duplicates: {}",
                report["duplicate_observation_count"]
            ));
        }
        if report["timeout_observation_count"].as_u64()
            != Some(baseline_timeouts + added_timeout_duplicates)
        {
            return Err(format!(
                "timeout duplicate observations must increment timeout count without new exclusions: {}",
                report["timeout_observation_count"]
            ));
        }
        Ok(())
    }

    #[test]
    fn mismatched_head_cannot_be_a_duplicate_of_an_existing_exclusion() -> Result<(), String> {
        let mut corpus = live_corpus()?;
        let exclusion =
            fieldconstruction_pilot_exclusion(&corpus, "ripr-1580-static-limitation")?.clone();
        let mut observation = duplicate_observation_from_exclusion(
            &exclusion,
            "identity-control-ripr-1580-wrong-head",
            "analysis_timeout",
        )?;
        // Sibling #1581 head is authorized for ripr-swarm observation, but it is
        // not the #1580 FieldConstruction identity.
        observation["analyzed_head_sha"] = json!("86fbe048eeedd0eeb6090db96355791c40b3486b");

        corpus
            .get_mut("observations")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| "observations must be an array".to_string())?
            .push(observation);

        let report = build_report(&corpus);
        if report["duplicate_observation_count"]
            != build_report(&live_corpus()?)["duplicate_observation_count"]
        {
            return Err("mismatched head must not count as a duplicate observation".to_string());
        }
        let errors = report["validation_errors"]
            .as_array()
            .ok_or_else(|| "validation_errors must be an array".to_string())?;
        if !errors.iter().filter_map(Value::as_str).any(|error| {
            error.contains("duplicate observation does not match exclusion ripr-1580-static-limitation field analyzed_head_sha")
        }) {
            return Err(format!("missing mismatched-head duplicate error: {errors:?}"));
        }
        Ok(())
    }

    #[test]
    fn second_exclusion_for_the_same_canonical_candidate_is_rejected() -> Result<(), String> {
        let mut corpus = live_corpus()?;
        let baseline_unique = build_report(&corpus)["unique_exclusion_count"]
            .as_u64()
            .ok_or_else(|| "unique_exclusion_count must be a number".to_string())?;
        let mut duplicate =
            fieldconstruction_pilot_exclusion(&corpus, "ub-772-static-limitation")?.clone();
        duplicate["exclusion_id"] = json!("identity-control-ub-772-duplicate-exclusion");
        duplicate["reason"] = json!("no_current_behavior_change");
        corpus
            .get_mut("exclusions")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| "exclusions must be an array".to_string())?
            .push(duplicate);

        let report = build_report(&corpus);
        if report["unique_exclusion_count"].as_u64() != Some(baseline_unique) {
            return Err(format!(
                "duplicate canonical identity must not inflate unique exclusions: {}",
                report["unique_exclusion_count"]
            ));
        }
        if report["eligible_attempt_count"] != 0 {
            return Err(
                "duplicate canonical identity must not enter the attempt denominator".to_string(),
            );
        }
        let errors = report["validation_errors"]
            .as_array()
            .ok_or_else(|| "validation_errors must be an array".to_string())?;
        if !errors.iter().filter_map(Value::as_str).any(|error| {
            error.contains("canonical_candidate_id EffortlessMetrics/ub-review#772 duplicates")
        }) {
            return Err(format!(
                "missing duplicate canonical-candidate error: {errors:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn in_place_exclusion_reason_update_keeps_identity_and_denominator() -> Result<(), String> {
        let mut corpus = live_corpus()?;
        let baseline = build_report(&corpus);
        let exclusions = corpus
            .get_mut("exclusions")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| "exclusions must be an array".to_string())?;
        let exclusion = exclusions
            .iter_mut()
            .find(|exclusion| {
                exclusion.get("exclusion_id").and_then(Value::as_str)
                    == Some("ripr-1580-static-limitation")
            })
            .ok_or_else(|| "missing ripr-1580 exclusion".to_string())?;
        exclusion["reason"] = json!("analysis_timeout");
        exclusion["evidence_ref"] = json!("metrics/rust-repair-trust/post-2933-ripr-1580.json");

        let report = build_report(&corpus);
        if report["unique_exclusion_count"] != baseline["unique_exclusion_count"] {
            return Err("in-place identity update must not add an exclusion".to_string());
        }
        if report["exclusion_count"] != baseline["exclusion_count"] {
            return Err("in-place identity update must keep the same exclusion_id row".to_string());
        }
        if report["eligible_attempt_count"] != 0 {
            return Err("in-place identity update must not invent an attempt".to_string());
        }
        if report["exclusion_reason_counts"]["analysis_timeout"].as_u64()
            != baseline["exclusion_reason_counts"]["analysis_timeout"]
                .as_u64()
                .map(|count| count + 1)
        {
            return Err(
                "updated reason must move the existing exclusion into the new bucket".to_string(),
            );
        }
        if report["exclusion_reason_counts"]["static_limitation_no_repair_packet"].as_u64()
            != baseline["exclusion_reason_counts"]["static_limitation_no_repair_packet"]
                .as_u64()
                .map(|count| count.saturating_sub(1))
        {
            return Err("updated reason must leave the prior bucket".to_string());
        }
        Ok(())
    }

    #[test]
    fn duplicate_observations_require_valid_exclusion_targets() -> Result<(), String> {
        let mut corpus = live_corpus()?;
        let baseline_duplicates = build_report(&corpus)["duplicate_observation_count"]
            .as_u64()
            .ok_or_else(|| "duplicate_observation_count must be a number".to_string())?;
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
            .and_then(|observations| {
                observations.iter().find(|observation| {
                    observation.get("observation_id").and_then(Value::as_str)
                        == Some("follow-up-ub-747-duplicate")
                })
            })
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
        if report["duplicate_observation_count"].as_u64() != Some(baseline_duplicates) {
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

    const RIPR_REPO: &str = "EffortlessMetrics/ripr-swarm";
    const PERL_REPO: &str = "EffortlessMetrics/perl-lsp-swarm";
    const UB_REPO: &str = "EffortlessMetrics/ub-review";
    const RIPR_HEAD: &str = "86fbe048eeedd0eeb6090db96355791c40b3486b";
    const PERL_HEAD: &str = "968464954e5da8e69a0b6b55de8ac349056924f6";
    const UB_HEAD: &str = "217633ca232120a021c7dc975973abdcb5056d39";

    fn blank_authorized_corpus() -> Result<Value, String> {
        let mut corpus = live_corpus()?;
        corpus["cases"] = json!([]);
        corpus["exclusions"] = json!([]);
        corpus["observations"] = json!([]);
        Ok(corpus)
    }

    fn observation_row(
        observation_id: &str,
        repository: &str,
        sha: &str,
        candidate: &str,
        reason: &str,
        classification: &str,
    ) -> Value {
        json!({
            "observation_id": observation_id,
            "repository": repository,
            "analyzed_head_sha": sha,
            "source_ref": format!("test:{observation_id}"),
            "canonical_candidate_id": candidate,
            "reason": reason,
            "evidence_ref": format!("target/ripr/{observation_id}.json"),
            "classification": classification,
            "claim_boundary": "fixture observation for route-yield ladder proof"
        })
    }

    fn three_unrouted_observations() -> Vec<Value> {
        vec![
            observation_row(
                "obs-ripr",
                RIPR_REPO,
                RIPR_HEAD,
                "EffortlessMetrics/ripr-swarm#1581",
                "static_limitation_no_repair_packet",
                "new_exclusion",
            ),
            observation_row(
                "obs-perl",
                PERL_REPO,
                PERL_HEAD,
                "EffortlessMetrics/perl-lsp-swarm#4022",
                "analysis_timeout",
                "new_exclusion",
            ),
            observation_row(
                "obs-ub",
                UB_REPO,
                UB_HEAD,
                "EffortlessMetrics/ub-review#772",
                "static_limitation_no_repair_packet",
                "new_exclusion",
            ),
        ]
    }

    fn report_from_public_entry(corpus: &Value) -> Result<Value, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("clock: {error}"))?
            .as_nanos();
        let path: PathBuf = std::env::temp_dir().join(format!(
            "ripr-rust-repair-trust-{}-{stamp}.json",
            std::process::id()
        ));
        let body = serde_json::to_string_pretty(corpus)
            .map_err(|error| format!("serialize temp corpus: {error}"))?;
        fs::write(&path, body).map_err(|error| format!("write {}: {error}", path.display()))?;
        let report = rust_repair_trust_report_value_at(&path);
        let _ = fs::remove_file(&path);
        report
    }

    fn require_measured_ratio(
        value: &Value,
        field: &str,
        numerator: u64,
        denominator: u64,
        unit: &str,
    ) -> Result<(), String> {
        let ratio = value
            .get(field)
            .ok_or_else(|| format!("{field} must be present"))?;
        if ratio.get("status").and_then(Value::as_str) != Some("measured") {
            return Err(format!(
                "{field}.status must be measured, got {}",
                ratio.get("status").cloned().unwrap_or(Value::Null)
            ));
        }
        if ratio.get("numerator").and_then(Value::as_u64) != Some(numerator) {
            return Err(format!(
                "{field}.numerator must be {numerator}, got {}",
                ratio.get("numerator").cloned().unwrap_or(Value::Null)
            ));
        }
        if ratio.get("denominator").and_then(Value::as_u64) != Some(denominator) {
            return Err(format!(
                "{field}.denominator must be {denominator}, got {}",
                ratio.get("denominator").cloned().unwrap_or(Value::Null)
            ));
        }
        if ratio.get("unit").and_then(Value::as_str) != Some(unit) {
            return Err(format!(
                "{field}.unit must be {unit}, got {}",
                ratio.get("unit").cloned().unwrap_or(Value::Null)
            ));
        }
        let display = format!("{numerator}/{denominator}");
        if ratio.get("display").and_then(Value::as_str) != Some(display.as_str()) {
            return Err(format!(
                "{field}.display must be {display}, got {}",
                ratio.get("display").cloned().unwrap_or(Value::Null)
            ));
        }
        Ok(())
    }

    fn require_not_measurable(value: &Value, field: &str, denominator: u64) -> Result<(), String> {
        let ratio = value
            .get(field)
            .ok_or_else(|| format!("{field} must be present"))?;
        if ratio.get("status").and_then(Value::as_str) != Some("not_measurable") {
            return Err(format!(
                "{field}.status must be not_measurable, got {}",
                ratio.get("status").cloned().unwrap_or(Value::Null)
            ));
        }
        if !ratio.get("numerator").is_some_and(Value::is_null) {
            return Err(format!(
                "{field}.numerator must be null when not measurable, got {}",
                ratio.get("numerator").cloned().unwrap_or(Value::Null)
            ));
        }
        if ratio.get("denominator").and_then(Value::as_u64) != Some(denominator) {
            return Err(format!(
                "{field}.denominator must be {denominator}, got {}",
                ratio.get("denominator").cloned().unwrap_or(Value::Null)
            ));
        }
        if ratio.get("display").and_then(Value::as_str) != Some("not_measurable") {
            return Err(format!(
                "{field}.display must be not_measurable, got {}",
                ratio.get("display").cloned().unwrap_or(Value::Null)
            ));
        }
        if ratio.get("rate").and_then(Value::as_f64).is_some() {
            return Err(format!("{field} must not invent a numeric rate"));
        }
        Ok(())
    }

    #[test]
    fn empty_observations_keep_route_yield_and_repair_success_unmeasurable() -> Result<(), String> {
        let corpus = blank_authorized_corpus()?;
        let report = report_from_public_entry(&corpus)?;
        require_not_measurable(&report, "route_yield", 0)?;
        require_not_measurable(&report, "repair_success", 0)?;
        if report["route_ladder"]["selected_opportunities"] != 0 {
            return Err("empty observations must select zero opportunities".to_string());
        }
        if report["eligible_attempt_count"] != 0 {
            return Err("empty corpus must keep zero eligible attempts".to_string());
        }
        let markdown = markdown_report(&report);
        if markdown.contains("0%") || markdown.contains("100%") {
            return Err(format!(
                "empty corpus markdown must not invent a percentage: {markdown}"
            ));
        }
        Ok(())
    }

    #[test]
    fn three_observations_without_a_complete_route_are_zero_of_three_not_attempts()
    -> Result<(), String> {
        let mut corpus = blank_authorized_corpus()?;
        corpus["observations"] = Value::Array(three_unrouted_observations());
        let report = report_from_public_entry(&corpus)?;
        if report["valid_observation_count"] != 3 {
            return Err(format!(
                "setup must retain three valid observations, got {}",
                report["valid_observation_count"]
            ));
        }
        if report["eligible_attempt_count"] != 0 {
            return Err("observations must not become eligible attempts".to_string());
        }
        require_measured_ratio(
            &report,
            "route_yield",
            0,
            3,
            "complete_routes / selected_opportunities",
        )?;
        require_not_measurable(&report, "repair_success", 0)?;
        if report["route_ladder"]["selected_opportunities"] != 3
            || report["route_ladder"]["complete_routes_admitted"] != 0
            || report["route_ladder"]["attempts_authorized_or_eligible"] != 0
        {
            return Err(format!(
                "ladder must keep 3 selected opportunities, 0 routes, 0 attempts: {}",
                report["route_ladder"]
            ));
        }
        let markdown = markdown_report(&report);
        if !markdown.contains("`0/3`") {
            return Err(format!("markdown must show route yield 0/3: {markdown}"));
        }
        if !markdown.contains("not_measurable") {
            return Err(format!(
                "markdown must keep repair success unmeasurable: {markdown}"
            ));
        }
        Ok(())
    }

    #[test]
    fn three_observations_do_not_inherit_exclusion_rows_as_the_route_denominator()
    -> Result<(), String> {
        let mut corpus = live_corpus()?;
        corpus["cases"] = json!([]);
        corpus["observations"] = Value::Array(three_unrouted_observations());
        let report = report_from_public_entry(&corpus)?;
        require_measured_ratio(
            &report,
            "route_yield",
            0,
            3,
            "complete_routes / selected_opportunities",
        )?;
        if report["exclusion_count"].as_u64().unwrap_or(0) < 3 {
            return Err(
                "setup must keep historical exclusions so they cannot be the 3".to_string(),
            );
        }
        if report["route_yield"]["denominator"] == report["exclusion_count"] {
            return Err("route yield must not use exclusion_count as its denominator".to_string());
        }
        require_not_measurable(&report, "repair_success", 0)?;
        Ok(())
    }

    #[test]
    fn live_corpus_counts_unique_observations_not_attempts_for_route_yield() -> Result<(), String> {
        let report = report_from_public_entry(&live_corpus()?)?;
        require_measured_ratio(
            &report,
            "route_yield",
            0,
            12,
            "complete_routes / selected_opportunities",
        )?;
        require_not_measurable(&report, "repair_success", 0)?;
        if report["eligible_attempt_count"] != 0 {
            return Err("live corpus must still have zero eligible attempts".to_string());
        }
        if report["route_yield"]["denominator"] == report["eligible_attempt_count"] {
            return Err("route yield must not collapse onto the attempt denominator".to_string());
        }
        if report["route_yield"]["denominator"] == report["observation_count"] {
            return Err("route yield must not reuse the mixed observation_count field".to_string());
        }
        Ok(())
    }

    #[test]
    fn one_complete_route_without_edit_authorization_is_not_an_attempt() -> Result<(), String> {
        let mut corpus = blank_authorized_corpus()?;
        let mut observation = observation_row(
            "obs-complete-route",
            RIPR_REPO,
            RIPR_HEAD,
            "EffortlessMetrics/ripr-swarm#1581",
            "static_limitation_no_repair_packet",
            "selected_opportunity",
        );
        observation["reason"] = json!("no_current_behavior_change");
        observation["route"] = json!({
            "opportunity_id": "opt-complete-no-attempt",
            "channel": "cli",
            "cohort_id": "cohort-a",
            "unit": "opportunity",
            "analysis_completed": true,
            "canonical_gap_identified": true,
            "complete_route_admitted": true,
            "packet_ready": true,
            "canonical_eligibility": "admitted",
            "attempt_authorized": false,
            "attempt_started": false,
            "attempt_finished": false,
            "earliest_stop": "not_observed"
        });
        corpus["observations"] = json!([observation]);
        let report = report_from_public_entry(&corpus)?;
        require_measured_ratio(
            &report,
            "route_yield",
            1,
            1,
            "complete_routes / selected_opportunities",
        )?;
        require_not_measurable(&report, "repair_success", 0)?;
        require_not_measurable(&report, "completion_without_hidden_help", 0)?;
        if report["route_ladder"]["attempts_authorized_or_eligible"] != 0
            || report["route_ladder"]["attempts_started"] != 0
            || report["eligible_attempt_count"] != 0
        {
            return Err(format!(
                "an unauthorized complete route must not mint attempts: {}",
                report["route_ladder"]
            ));
        }
        Ok(())
    }

    #[test]
    fn failed_focused_test_and_improved_static_evidence_stay_separate() -> Result<(), String> {
        let mut corpus = blank_authorized_corpus()?;
        let mut observation = observation_row(
            "obs-attempted",
            RIPR_REPO,
            RIPR_HEAD,
            "EffortlessMetrics/ripr-swarm#1581",
            "verification_failed",
            "selected_opportunity",
        );
        observation["route"] = json!({
            "opportunity_id": "opt-improved-failed-test",
            "channel": "cli",
            "cohort_id": "cohort-a",
            "unit": "opportunity",
            "analysis_completed": true,
            "canonical_gap_identified": true,
            "complete_route_admitted": true,
            "packet_ready": true,
            "canonical_eligibility": "admitted",
            "attempt_authorized": true,
            "attempt_started": true,
            "attempt_finished": true,
            "static_movement": "improved",
            "focused_test_result": "failed",
            "repair_attempt_id": "attempt-improved-failed-test",
            "earliest_stop": "not_observed"
        });
        corpus["observations"] = json!([observation]);
        let mut attempt = valid_attempt(
            "attempt-improved-failed-test",
            RIPR_REPO,
            "gap:opt-improved-failed-test",
            1,
            "improved",
            Vec::new(),
        );
        attempt["analyzed_head_sha"] = json!(RIPR_HEAD);
        attempt["verification_result"] = json!("failed");
        corpus["cases"] = json!([attempt]);
        let report = report_from_public_entry(&corpus)?;
        if report["movement_counts"]["improved"] != 1 {
            return Err("improved static movement must remain visible".to_string());
        }
        if report["focused_test_execution"]["failed"] != 1
            || report["focused_test_execution"]["passed"] != 0
        {
            return Err(format!(
                "failed focused test must stay separate from static movement: {}",
                report["focused_test_execution"]
            ));
        }
        require_measured_ratio(
            &report,
            "repair_success",
            1,
            1,
            "improved_or_closed / eligible_attempts",
        )?;
        if report["repair_success"]["numerator"] == report["focused_test_execution"]["passed"] {
            return Err(
                "repair success must not be inferred from a passing focused test".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn cli_and_editor_observations_of_one_opportunity_do_not_inflate_route_yield()
    -> Result<(), String> {
        let mut corpus = blank_authorized_corpus()?;
        let mut cli = observation_row(
            "obs-cli",
            RIPR_REPO,
            RIPR_HEAD,
            "EffortlessMetrics/ripr-swarm#1581",
            "static_limitation_no_repair_packet",
            "selected_opportunity",
        );
        cli["route"] = json!({
            "opportunity_id": "opt-shared",
            "channel": "cli",
            "cohort_id": "cohort-a",
            "unit": "opportunity",
            "complete_route_admitted": false,
            "focused_test_result": "failed",
            "earliest_stop": "not_observed"
        });
        let mut editor = cli.clone();
        editor["observation_id"] = json!("obs-editor");
        editor["route"]["channel"] = json!("editor");
        corpus["observations"] = json!([cli, editor]);
        let report = report_from_public_entry(&corpus)?;
        if report["valid_observation_count"] != 2 {
            return Err("both channel observations must remain valid".to_string());
        }
        require_measured_ratio(
            &report,
            "route_yield",
            0,
            1,
            "complete_routes / selected_opportunities",
        )?;
        if report["channel_observation_counts"]["cli"] != 1
            || report["channel_observation_counts"]["editor"] != 1
        {
            return Err(format!(
                "channel evidence must stay visible: {}",
                report["channel_observation_counts"]
            ));
        }
        if report["focused_test_execution"]["failed"] != 1
            || report["focused_test_execution"]["passed"] != 0
            || report["focused_test_execution"]["not_observed"] != 0
        {
            return Err(format!(
                "duplicate channel focused-test results must count once per opportunity: {}",
                report["focused_test_execution"]
            ));
        }
        Ok(())
    }

    #[test]
    fn analyzer_rerun_keeps_distinct_cohorts_and_the_original_result() -> Result<(), String> {
        let mut corpus = blank_authorized_corpus()?;
        let mut original = observation_row(
            "obs-original",
            RIPR_REPO,
            RIPR_HEAD,
            "EffortlessMetrics/ripr-swarm#1581",
            "static_limitation_no_repair_packet",
            "selected_opportunity",
        );
        original["route"] = json!({
            "opportunity_id": "opt-shared",
            "channel": "cli",
            "cohort_id": "cohort-before-fix",
            "analyzer_generation": "pre-fix",
            "unit": "opportunity",
            "complete_route_admitted": false,
            "earliest_stop": "not_observed"
        });
        let mut rerun = original.clone();
        rerun["observation_id"] = json!("obs-rerun");
        rerun["route"]["cohort_id"] = json!("cohort-after-fix");
        rerun["route"]["analyzer_generation"] = json!("post-fix");
        rerun["route"]["complete_route_admitted"] = json!(true);
        rerun["route"]["canonical_eligibility"] = json!("admitted");
        rerun["route"]["analysis_completed"] = json!(true);
        rerun["route"]["canonical_gap_identified"] = json!(true);
        corpus["observations"] = json!([original, rerun]);
        let report = report_from_public_entry(&corpus)?;
        require_measured_ratio(
            &report,
            "route_yield",
            1,
            2,
            "complete_routes / selected_opportunities",
        )?;
        if report["cohort_count"] != 2 {
            return Err(format!(
                "distinct analyzer cohorts must remain two experiments: {}",
                report["cohort_count"]
            ));
        }
        if report["route_ladder"]["complete_routes_admitted"] != 1 {
            return Err("the original failure must not be pooled into a 2/2 success".to_string());
        }
        Ok(())
    }

    #[test]
    fn timeout_before_item_discovery_does_not_mint_a_gap_id() -> Result<(), String> {
        let mut corpus = blank_authorized_corpus()?;
        let mut observation = observation_row(
            "obs-timeout",
            PERL_REPO,
            PERL_HEAD,
            "EffortlessMetrics/perl-lsp-swarm#4022",
            "analysis_timeout",
            "selected_opportunity",
        );
        observation["route"] = json!({
            "opportunity_id": "repo-obs-perl-4022",
            "channel": "cli",
            "cohort_id": "cohort-a",
            "unit": "repository_observation",
            "analysis_completed": false,
            "canonical_gap_identified": false,
            "complete_route_admitted": false,
            "earliest_stop": "analysis_timeout"
        });
        corpus["observations"] = json!([observation]);
        let report = report_from_public_entry(&corpus)?;
        require_measured_ratio(
            &report,
            "route_yield",
            0,
            1,
            "complete_routes / selected_opportunities",
        )?;
        if report["route_ladder"]["canonical_behavior_or_gap_identified"] != 0 {
            return Err("a timeout must not invent a canonical gap".to_string());
        }
        if report["earliest_stop_counts"]["analysis_timeout"] != 1 {
            return Err(format!(
                "timeout must remain the earliest stop: {}",
                report["earliest_stop_counts"]
            ));
        }
        let errors = report["validation_errors"]
            .as_array()
            .ok_or_else(|| "validation_errors must be an array".to_string())?;
        if errors
            .iter()
            .filter_map(Value::as_str)
            .any(|error| error.contains("gap:"))
        {
            return Err("timeout observations must not be forced through gap identity".to_string());
        }
        Ok(())
    }

    #[test]
    fn legacy_static_limitation_is_not_relabelled_into_a_precise_stop() -> Result<(), String> {
        let mut corpus = blank_authorized_corpus()?;
        corpus["observations"] = json!([observation_row(
            "obs-legacy",
            RIPR_REPO,
            RIPR_HEAD,
            "EffortlessMetrics/ripr-swarm#1581",
            "static_limitation_no_repair_packet",
            "new_exclusion",
        )]);
        let report = report_from_public_entry(&corpus)?;
        if report["earliest_stop_counts"]["not_observed"] != 1 {
            return Err(format!(
                "legacy static_limitation_no_repair_packet must stay not_observed, got {}",
                report["earliest_stop_counts"]
            ));
        }
        for forbidden in [
            "missing_discriminator",
            "missing_related_test",
            "ambiguous_or_unsafe_target",
            "missing_fix_site",
        ] {
            if report["earliest_stop_counts"]
                .get(forbidden)
                .and_then(Value::as_u64)
                .unwrap_or(0)
                != 0
            {
                return Err(format!("legacy rows must not be relabelled as {forbidden}"));
            }
        }
        Ok(())
    }

    #[test]
    fn packet_ready_rejected_by_canonical_eligibility_is_not_a_complete_route() -> Result<(), String>
    {
        let mut corpus = blank_authorized_corpus()?;
        let mut observation = observation_row(
            "obs-rejected-packet",
            RIPR_REPO,
            RIPR_HEAD,
            "EffortlessMetrics/ripr-swarm#1581",
            "static_limitation_no_repair_packet",
            "selected_opportunity",
        );
        observation["route"] = json!({
            "opportunity_id": "opt-rejected",
            "channel": "cli",
            "cohort_id": "cohort-a",
            "unit": "opportunity",
            "analysis_completed": true,
            "canonical_gap_identified": true,
            "complete_route_admitted": true,
            "packet_ready": true,
            "canonical_eligibility": "rejected",
            "attempt_authorized": false,
            "earliest_stop": "ambiguous_or_unsafe_target"
        });
        corpus["observations"] = json!([observation]);
        let report = report_from_public_entry(&corpus)?;
        require_measured_ratio(
            &report,
            "route_yield",
            0,
            1,
            "complete_routes / selected_opportunities",
        )?;
        if report["route_ladder"]["complete_routes_admitted"] != 0 {
            return Err("canonical eligibility rejection must veto packet-ready".to_string());
        }
        if report["earliest_stop_counts"]["ambiguous_or_unsafe_target"] != 1 {
            return Err("the rejected eligibility stop must remain visible".to_string());
        }
        Ok(())
    }

    #[test]
    fn duplicate_ids_and_hand_edited_totals_cannot_enter_trusted_success_counts()
    -> Result<(), String> {
        let mut corpus = blank_authorized_corpus()?;
        let first = observation_row(
            "obs-dup",
            RIPR_REPO,
            RIPR_HEAD,
            "EffortlessMetrics/ripr-swarm#1581",
            "static_limitation_no_repair_packet",
            "new_exclusion",
        );
        let duplicate = first.clone();
        corpus["observations"] = json!([first, duplicate]);
        corpus["route_yield"] = json!({
            "status": "measured",
            "numerator": 99,
            "denominator": 99,
            "unit": "invented",
            "display": "99/99"
        });
        let report = report_from_public_entry(&corpus)?;
        let errors = report["validation_errors"]
            .as_array()
            .ok_or_else(|| "validation_errors must be an array".to_string())?;
        let errors = errors.iter().filter_map(Value::as_str).collect::<Vec<_>>();
        if !errors
            .iter()
            .any(|error| error.contains("duplicate observation id obs-dup"))
        {
            return Err("duplicate observation ids must fail closed".to_string());
        }
        if !errors
            .iter()
            .any(|error| error.contains("hand-edited") && error.contains("route_yield"))
        {
            return Err(format!(
                "hand-edited totals must be rejected, got {errors:?}"
            ));
        }
        if report
            .get("route_yield")
            .and_then(|value| value.get("numerator"))
            == Some(&json!(99))
        {
            return Err("hand-edited totals must not become the trusted numerator".to_string());
        }
        require_measured_ratio(
            &report,
            "route_yield",
            0,
            1,
            "complete_routes / selected_opportunities",
        )?;
        require_not_measurable(&report, "repair_success", 0)?;
        Ok(())
    }

    #[test]
    fn timeout_reason_normalizes_exactly_and_does_not_require_a_gap() -> Result<(), String> {
        let mut corpus = blank_authorized_corpus()?;
        corpus["observations"] = json!([observation_row(
            "obs-timeout-legacy",
            PERL_REPO,
            PERL_HEAD,
            "EffortlessMetrics/perl-lsp-swarm#4022",
            "analysis_timeout",
            "new_exclusion",
        )]);
        let report = report_from_public_entry(&corpus)?;
        if report["earliest_stop_counts"]["analysis_timeout"] != 1 {
            return Err(format!(
                "analysis_timeout is an exact historical mapping, got {}",
                report["earliest_stop_counts"]
            ));
        }
        require_measured_ratio(
            &report,
            "route_yield",
            0,
            1,
            "complete_routes / selected_opportunities",
        )?;
        Ok(())
    }

    #[test]
    fn unlinked_repeat_attempts_do_not_mint_attempt_id_opportunities() -> Result<(), String> {
        let mut corpus = blank_authorized_corpus()?;
        let first = valid_attempt("attempt-1", RIPR_REPO, "gap:foo", 1, "improved", Vec::new());
        let mut second = valid_attempt("attempt-2", RIPR_REPO, "gap:foo", 2, "closed", Vec::new());
        second["analyzed_head_sha"] = first["analyzed_head_sha"].clone();
        corpus["cases"] = json!([first, second]);
        let report = report_from_public_entry(&corpus)?;
        if report["eligible_attempt_count"] != 2 {
            return Err("both eligible cases must remain counted attempts".to_string());
        }
        require_measured_ratio(
            &report,
            "route_yield",
            1,
            1,
            "complete_routes / selected_opportunities",
        )?;
        require_measured_ratio(
            &report,
            "repair_success",
            2,
            2,
            "improved_or_closed / eligible_attempts",
        )?;
        Ok(())
    }

    #[test]
    fn cross_repository_attempt_link_cannot_complete_the_wrong_opportunity() -> Result<(), String> {
        let mut corpus = blank_authorized_corpus()?;
        let mut observation = observation_row(
            "obs-ripr",
            RIPR_REPO,
            RIPR_HEAD,
            "EffortlessMetrics/ripr-swarm#1581",
            "static_limitation_no_repair_packet",
            "selected_opportunity",
        );
        observation["route"] = json!({
            "opportunity_id": "opt-ripr",
            "channel": "cli",
            "cohort_id": "cohort-a",
            "unit": "opportunity",
            "complete_route_admitted": false,
            "repair_attempt_id": "attempt-perl",
            "earliest_stop": "not_observed"
        });
        let mut attempt = valid_attempt(
            "attempt-perl",
            PERL_REPO,
            "gap:perl",
            1,
            "improved",
            Vec::new(),
        );
        attempt["analyzed_head_sha"] = json!(PERL_HEAD);
        corpus["observations"] = json!([observation]);
        corpus["cases"] = json!([attempt]);
        let report = report_from_public_entry(&corpus)?;
        if report["valid_observation_count"] != 1 {
            return Err("the unmatched observation must remain a selected opportunity".to_string());
        }
        if report["eligible_attempt_count"] != 1 {
            return Err("the perl case must remain an eligible attempt".to_string());
        }
        require_measured_ratio(
            &report,
            "route_yield",
            1,
            2,
            "complete_routes / selected_opportunities",
        )?;
        let errors = report["validation_errors"]
            .as_array()
            .ok_or_else(|| "validation_errors must be an array".to_string())?;
        if !errors.iter().filter_map(Value::as_str).any(|error| {
            error.contains("repair_attempt_id attempt-perl")
                && error.contains("repository")
                && error.contains("analyzed head")
        }) {
            return Err(format!(
                "cross-repository attempt links must be rejected, got {errors:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn finished_attempt_without_a_start_cannot_enter_the_trusted_ladder() -> Result<(), String> {
        let mut corpus = blank_authorized_corpus()?;
        let mut observation = observation_row(
            "obs-unstarted-finish",
            RIPR_REPO,
            RIPR_HEAD,
            "EffortlessMetrics/ripr-swarm#1581",
            "no_current_behavior_change",
            "selected_opportunity",
        );
        observation["route"] = json!({
            "opportunity_id": "opt-unstarted",
            "channel": "cli",
            "cohort_id": "cohort-a",
            "unit": "opportunity",
            "analysis_completed": false,
            "attempt_started": false,
            "attempt_finished": true
        });
        corpus["observations"] = json!([observation]);
        let report = report_from_public_entry(&corpus)?;
        let errors = report["validation_errors"]
            .as_array()
            .ok_or_else(|| "validation_errors must be an array".to_string())?;
        if !errors
            .iter()
            .filter_map(Value::as_str)
            .any(|error| error.contains("attempt_finished") && error.contains("attempt_started"))
        {
            return Err(format!(
                "explicit stage contradictions must be rejected, got {errors:?}"
            ));
        }
        if report["valid_observation_count"] != 0
            || report["route_ladder"]["selected_opportunities"] != 0
            || report["route_ladder"]["attempts_finished"] != 0
        {
            return Err(format!(
                "contradictory stages must not enter trusted counts: {}",
                report["route_ladder"]
            ));
        }
        require_not_measurable(&report, "route_yield", 0)?;
        Ok(())
    }

    #[test]
    fn improved_static_movement_without_a_finished_attempt_is_not_ladder_success()
    -> Result<(), String> {
        let mut corpus = blank_authorized_corpus()?;
        let mut observation = observation_row(
            "obs-unattempted-improved",
            RIPR_REPO,
            RIPR_HEAD,
            "EffortlessMetrics/ripr-swarm#1581",
            "no_current_behavior_change",
            "selected_opportunity",
        );
        observation["route"] = json!({
            "opportunity_id": "opt-unattempted",
            "channel": "cli",
            "cohort_id": "cohort-a",
            "unit": "opportunity",
            "analysis_completed": true,
            "canonical_gap_identified": true,
            "complete_route_admitted": true,
            "canonical_eligibility": "admitted",
            "attempt_authorized": false,
            "attempt_started": false,
            "attempt_finished": false,
            "static_movement": "improved"
        });
        corpus["observations"] = json!([observation]);
        let report = report_from_public_entry(&corpus)?;
        let errors = report["validation_errors"]
            .as_array()
            .ok_or_else(|| "validation_errors must be an array".to_string())?;
        if !errors
            .iter()
            .filter_map(Value::as_str)
            .any(|error| error.contains("static_movement") && error.contains("attempt_finished"))
        {
            return Err(format!(
                "unattempted static improvement must be rejected, got {errors:?}"
            ));
        }
        if report["route_ladder"]["static_improved_or_closed"] != 0
            || report["eligible_attempt_count"] != 0
        {
            return Err(format!(
                "unattempted improvement must not count as ladder success: {}",
                report["route_ladder"]
            ));
        }
        Ok(())
    }

    #[test]
    fn omitted_archaeology_cannot_count_as_help_free_completion() -> Result<(), String> {
        let mut corpus = blank_authorized_corpus()?;
        let mut observation = observation_row(
            "obs-unknown-help",
            RIPR_REPO,
            RIPR_HEAD,
            "EffortlessMetrics/ripr-swarm#1581",
            "no_current_behavior_change",
            "selected_opportunity",
        );
        observation["route"] = json!({
            "opportunity_id": "opt-unknown-help",
            "channel": "cli",
            "cohort_id": "cohort-a",
            "unit": "opportunity",
            "analysis_completed": true,
            "canonical_gap_identified": true,
            "complete_route_admitted": true,
            "canonical_eligibility": "admitted",
            "attempt_authorized": false
        });
        corpus["observations"] = json!([observation]);
        let report = report_from_public_entry(&corpus)?;
        require_measured_ratio(
            &report,
            "route_yield",
            1,
            1,
            "complete_routes / selected_opportunities",
        )?;
        require_not_measurable(&report, "completion_without_hidden_help", 0)?;
        Ok(())
    }

    #[test]
    fn observed_absent_archaeology_is_help_free_completion() -> Result<(), String> {
        let mut corpus = blank_authorized_corpus()?;
        let mut observation = observation_row(
            "obs-help-free",
            RIPR_REPO,
            RIPR_HEAD,
            "EffortlessMetrics/ripr-swarm#1581",
            "no_current_behavior_change",
            "selected_opportunity",
        );
        observation["route"] = json!({
            "opportunity_id": "opt-help-free",
            "channel": "cli",
            "cohort_id": "cohort-a",
            "unit": "opportunity",
            "analysis_completed": true,
            "canonical_gap_identified": true,
            "complete_route_admitted": true,
            "canonical_eligibility": "admitted",
            "artifact_archaeology": false,
            "attempt_authorized": false
        });
        corpus["observations"] = json!([observation]);
        let report = report_from_public_entry(&corpus)?;
        require_measured_ratio(
            &report,
            "completion_without_hidden_help",
            1,
            1,
            "completed_without_artifact_archaeology / complete_routes_with_observed_archaeology",
        )?;
        Ok(())
    }

    #[test]
    fn conflicting_channel_focused_tests_fail_closed_to_failed() -> Result<(), String> {
        let mut corpus = blank_authorized_corpus()?;
        let mut cli = observation_row(
            "obs-cli-pass",
            RIPR_REPO,
            RIPR_HEAD,
            "EffortlessMetrics/ripr-swarm#1581",
            "verification_failed",
            "selected_opportunity",
        );
        cli["route"] = json!({
            "opportunity_id": "opt-conflict",
            "channel": "cli",
            "cohort_id": "cohort-a",
            "unit": "opportunity",
            "focused_test_result": "passed"
        });
        let mut editor = cli.clone();
        editor["observation_id"] = json!("obs-editor-fail");
        editor["route"]["channel"] = json!("editor");
        editor["route"]["focused_test_result"] = json!("failed");
        corpus["observations"] = json!([cli, editor]);
        let report = report_from_public_entry(&corpus)?;
        require_measured_ratio(
            &report,
            "route_yield",
            0,
            1,
            "complete_routes / selected_opportunities",
        )?;
        if report["focused_test_execution"]["failed"] != 1
            || report["focused_test_execution"]["passed"] != 0
        {
            return Err(format!(
                "conflicting channel tests must fail closed: {}",
                report["focused_test_execution"]
            ));
        }
        Ok(())
    }
}
