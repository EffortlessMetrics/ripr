//! General fixture-corpus validators for `check-fixture-contracts`:
//! evidence-record contract, lane1 evidence-quality-failure,
//! evidence-quality-benchmark, finding-alignment dogfood, real-repair
//! attempts, python real-repo evals, TypeScript calibration/repair-loop/
//! false-actionable corpora, cross-language oracle graph and Bun UB
//! dogfood, and surface-projection alignment corpora.
//!
//! Extracted verbatim from `main.rs` as a behavior-preserving decomposition
//! slice of #2119. Items referenced outside this module are `pub(crate)` and
//! re-exported from `main.rs` so existing call sites (`dispatch.rs`,
//! `dogfood.rs`, and `tests.rs`) compile unchanged.

use super::*;

pub(crate) fn validate_evidence_record_contract_fixture_corpus(
    violations: &mut Vec<String>,
) -> Result<(), String> {
    validate_evidence_record_contract_fixture_corpus_at(
        Path::new(EVIDENCE_RECORD_CONTRACT_CORPUS),
        violations,
    )
}

pub(crate) fn validate_evidence_record_contract_fixture_corpus_at(
    path: &Path,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    if !path.exists() {
        violations.push(format!(
            "evidence-record contract corpus is missing {}",
            normalize_path(path)
        ));
        return Ok(());
    }

    let corpus = match read_json_value(path) {
        Ok(value) => value,
        Err(err) => {
            violations.push(err);
            return Ok(());
        }
    };
    validate_evidence_record_contract_corpus_value(path, &corpus, violations);
    Ok(())
}

fn validate_evidence_record_contract_corpus_value(
    path: &Path,
    corpus: &Value,
    violations: &mut Vec<String>,
) {
    if json_string_field(corpus, "kind").as_deref() != Some("evidence_record_contract_corpus") {
        violations.push(format!(
            "{} kind must be evidence_record_contract_corpus",
            normalize_path(path)
        ));
    }
    if json_string_field(corpus, "schema_version").as_deref() != Some("0.1") {
        violations.push(format!(
            "{} schema_version must be 0.1",
            normalize_path(path)
        ));
    }
    if json_string_field(corpus, "spec").as_deref() != Some("RIPR-SPEC-0021") {
        violations.push(format!(
            "{} spec must be RIPR-SPEC-0021",
            normalize_path(path)
        ));
    }

    let Some(cases) = corpus.get("cases").and_then(Value::as_array) else {
        violations.push(format!("{} is missing cases array", normalize_path(path)));
        return;
    };

    let mut seen = BTreeSet::new();
    for case in cases {
        let case_id = json_string_field(case, "id").unwrap_or_else(|| "unknown".to_string());
        if !seen.insert(case_id.clone()) {
            violations.push(format!("evidence-record case {case_id} is duplicated"));
        }
        if json_string_field(case, "description").is_none() {
            violations.push(format!(
                "evidence-record case {case_id} is missing description"
            ));
        }
        if json_string_field(case, "source").is_none() {
            violations.push(format!("evidence-record case {case_id} is missing source"));
        }
        match case.get("record") {
            Some(Value::Object(record)) => validate_evidence_record_contract_record(
                &case_id,
                &Value::Object(record.clone()),
                violations,
            ),
            Some(_) => violations.push(format!(
                "evidence-record case {case_id} record must be an object"
            )),
            None => violations.push(format!("evidence-record case {case_id} is missing record")),
        }
    }

    for required in EVIDENCE_RECORD_REQUIRED_CASES {
        if !seen.contains(*required) {
            violations.push(format!("evidence-record corpus is missing case {required}"));
        }
    }
}

pub(crate) fn validate_lane1_evidence_quality_failure_fixture_corpus(
    violations: &mut Vec<String>,
) -> Result<(), String> {
    validate_lane1_evidence_quality_failure_fixture_corpus_at(
        Path::new(LANE1_EVIDENCE_QUALITY_FAILURE_CORPUS),
        violations,
    )
}

pub(crate) fn validate_lane1_evidence_quality_failure_fixture_corpus_at(
    path: &Path,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    if !path.exists() {
        violations.push(format!(
            "Lane 1 evidence-quality failure corpus is missing {}",
            normalize_path(path)
        ));
        return Ok(());
    }

    let corpus = match read_json_value(path) {
        Ok(value) => value,
        Err(err) => {
            violations.push(err);
            return Ok(());
        }
    };
    validate_lane1_evidence_quality_failure_corpus_value(path, &corpus, violations);
    Ok(())
}

fn validate_lane1_evidence_quality_failure_corpus_value(
    path: &Path,
    corpus: &Value,
    violations: &mut Vec<String>,
) {
    if json_string_field(corpus, "kind").as_deref() != Some("lane1_evidence_quality_failure_corpus")
    {
        violations.push(format!(
            "{} kind must be lane1_evidence_quality_failure_corpus",
            normalize_path(path)
        ));
    }
    if json_string_field(corpus, "schema_version").as_deref() != Some("0.1") {
        violations.push(format!(
            "{} schema_version must be 0.1",
            normalize_path(path)
        ));
    }
    if json_string_field(corpus, "spec").as_deref() != Some("RIPR-SPEC-0032") {
        violations.push(format!(
            "{} spec must be RIPR-SPEC-0032",
            normalize_path(path)
        ));
    }

    match corpus.get("source_report") {
        Some(source @ Value::Object(_)) => {
            require_lane1_json_string_at(source, "command", "source_report", violations);
            require_lane1_json_string_at(source, "report", "source_report", violations);
            match source.get("summary") {
                Some(summary @ Value::Object(_)) => {
                    for field in [
                        "raw_headline_gaps",
                        "canonical_gap_groups",
                        "duplicate_looking_groups",
                        "missing_discriminators",
                        "static_limitations",
                        "uncalibrated_records",
                    ] {
                        require_lane1_json_usize_at(
                            summary,
                            field,
                            "source_report.summary",
                            violations,
                        );
                    }
                }
                _ => violations.push(
                    "Lane 1 evidence-quality source_report is missing summary object".to_string(),
                ),
            }
        }
        _ => violations
            .push("Lane 1 evidence-quality corpus is missing source_report object".to_string()),
    }

    let Some(cases) = corpus.get("cases").and_then(Value::as_array) else {
        violations.push(format!("{} is missing cases array", normalize_path(path)));
        return;
    };

    let mut seen = BTreeSet::new();
    let mut has_failure_mode = false;
    let mut has_negative_guard = false;
    for case in cases {
        let case_id = json_string_field(case, "id").unwrap_or_else(|| "unknown".to_string());
        if !seen.insert(case_id.clone()) {
            violations.push(format!(
                "Lane 1 evidence-quality case {case_id} is duplicated"
            ));
        }
        validate_lane1_evidence_quality_failure_case(
            &case_id,
            case,
            &mut has_failure_mode,
            &mut has_negative_guard,
            violations,
        );
    }

    for required in LANE1_EVIDENCE_QUALITY_REQUIRED_CASES {
        if !seen.contains(*required) {
            violations.push(format!(
                "Lane 1 evidence-quality corpus is missing case {required}"
            ));
        }
    }
    if !has_failure_mode {
        violations.push(
            "Lane 1 evidence-quality corpus must include at least one failure_mode case"
                .to_string(),
        );
    }
    if !has_negative_guard {
        violations.push(
            "Lane 1 evidence-quality corpus must include at least one negative_guard case"
                .to_string(),
        );
    }
}

fn validate_lane1_evidence_quality_failure_case(
    case_id: &str,
    case: &Value,
    has_failure_mode: &mut bool,
    has_negative_guard: &mut bool,
    violations: &mut Vec<String>,
) {
    require_lane1_json_string_at(case, "description", case_id, violations);
    require_lane1_json_string_at(case, "source", case_id, violations);

    match json_string_field(case, "case_kind").as_deref() {
        Some("failure_mode") => *has_failure_mode = true,
        Some("negative_guard") => *has_negative_guard = true,
        Some(other) => violations.push(format!(
            "Lane 1 evidence-quality case {case_id} has unsupported case_kind {other}"
        )),
        None => violations.push(format!(
            "Lane 1 evidence-quality case {case_id} is missing string field case_kind"
        )),
    }

    let category = match case.get("audit_signal") {
        Some(signal @ Value::Object(_)) => {
            require_lane1_json_string_at(signal, "category", case_id, violations);
            require_lane1_json_string_at(signal, "metric_path", case_id, violations);
            require_lane1_json_string_at(signal, "evidence", case_id, violations);
            require_lane1_json_usize_at(signal, "observed_count", case_id, violations);
            json_string_field(signal, "category")
        }
        _ => {
            violations.push(format!(
                "Lane 1 evidence-quality case {case_id} is missing audit_signal object"
            ));
            None
        }
    };
    if let Some(category) = &category
        && !matches!(
            category.as_str(),
            "duplicate_canonical_gap"
                | "missing_discriminator"
                | "static_limitation"
                | "oracle_semantics"
                | "calibration_gap"
        )
    {
        violations.push(format!(
            "Lane 1 evidence-quality case {case_id} has unsupported audit category {category}"
        ));
    }

    let expected = match case.get("expected_repo_exposure") {
        Some(expected @ Value::Object(_)) => {
            require_lane1_json_string_at(expected, "source", case_id, violations);
            expected
        }
        _ => {
            violations.push(format!(
                "Lane 1 evidence-quality case {case_id} is missing expected_repo_exposure object"
            ));
            return;
        }
    };
    let record = match expected.get("evidence_record") {
        Some(record @ Value::Object(_)) => record,
        Some(_) => {
            violations.push(format!(
                "Lane 1 evidence-quality case {case_id} expected_repo_exposure.evidence_record must be an object"
            ));
            return;
        }
        None => {
            violations.push(format!(
                "Lane 1 evidence-quality case {case_id} is missing expected_repo_exposure.evidence_record"
            ));
            return;
        }
    };
    validate_lane1_evidence_quality_record(case_id, record, violations);

    require_non_empty_string_array_at(case, "expected_claims", case_id, violations);
    require_non_empty_string_array_at(case, "must_not_claim", case_id, violations);

    match category.as_deref() {
        Some("duplicate_canonical_gap") => {
            if json_string_field(record, "canonical_gap_id").is_none() {
                violations.push(format!(
                    "Lane 1 evidence-quality duplicate case {case_id} must pin canonical_gap_id"
                ));
            }
            let group_size =
                json_usize_field(record, "canonical_gap_group_size").unwrap_or_default();
            let corrected_duplicate_case = group_size == 1
                && string_array_contains_case_insensitive(case, "must_not_claim", "generic");
            if group_size <= 1 && !corrected_duplicate_case {
                violations.push(format!(
                    "Lane 1 evidence-quality duplicate case {case_id} must pin group size greater than 1 or pin corrected group size 1 with a generic identity regression guard"
                ));
            }
        }
        Some("missing_discriminator")
            if lane1_count_field(record, "missing_discriminators").unwrap_or_default() == 0 =>
        {
            violations.push(format!(
                "Lane 1 evidence-quality missing-discriminator case {case_id} must pin missing discriminator count"
            ));
        }
        Some("missing_discriminator") => {}
        Some("static_limitation") => {
            if lane1_count_field(record, "static_limitations").unwrap_or_default() == 0 {
                violations.push(format!(
                    "Lane 1 evidence-quality static-limitation case {case_id} must pin static limitation count"
                ));
            }
            if !matches!(record.get("static_limitations"), Some(Value::Array(items)) if !items.is_empty())
            {
                violations.push(format!(
                    "Lane 1 evidence-quality static-limitation case {case_id} must list static_limitations"
                ));
            }
        }
        Some("oracle_semantics") => {
            let oracle_kind = record
                .get("top_related_test")
                .and_then(|test| json_string_field(test, "oracle_kind"));
            if oracle_kind.as_deref() != Some("mock_expectation")
                && json_string_field(record, "seam_kind").as_deref() != Some("side_effect")
            {
                violations.push(format!(
                    "Lane 1 evidence-quality oracle-semantics case {case_id} must pin a mock_expectation or side_effect signal"
                ));
            }
        }
        Some("calibration_gap") => match record.get("calibration") {
            Some(calibration) => {
                if json_string_field(calibration, "availability").as_deref() != Some("not_imported")
                    || json_string_field(calibration, "agreement").as_deref()
                        != Some("no_runtime_data")
                {
                    violations.push(format!(
                        "Lane 1 evidence-quality calibration case {case_id} must stay not_imported/no_runtime_data"
                    ));
                }
            }
            None => violations.push(format!(
                "Lane 1 evidence-quality calibration case {case_id} is missing calibration"
            )),
        },
        _ => {}
    }
}

pub(crate) fn validate_evidence_quality_benchmark_fixture_corpus(
    violations: &mut Vec<String>,
) -> Result<(), String> {
    validate_evidence_quality_benchmark_fixture_corpus_at(
        Path::new(EVIDENCE_QUALITY_BENCHMARK_CORPUS),
        violations,
    )
}

pub(crate) fn validate_evidence_quality_benchmark_fixture_corpus_at(
    path: &Path,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    if !path.exists() {
        violations.push(format!(
            "Lane 1 evidence-quality benchmark corpus is missing {}",
            normalize_path(path)
        ));
        return Ok(());
    }

    let corpus = match read_json_value(path) {
        Ok(value) => value,
        Err(err) => {
            violations.push(err);
            return Ok(());
        }
    };
    validate_evidence_quality_benchmark_corpus_value(path, &corpus, violations);
    Ok(())
}

pub(crate) fn validate_evidence_quality_benchmark_corpus_value(
    path: &Path,
    corpus: &Value,
    violations: &mut Vec<String>,
) {
    let normalized = normalize_path(path);
    if json_string_field(corpus, "kind").as_deref()
        != Some("lane1_evidence_quality_benchmark_corpus")
    {
        violations.push(format!(
            "{normalized} kind must be lane1_evidence_quality_benchmark_corpus"
        ));
    }
    if json_string_field(corpus, "schema_version").as_deref() != Some("0.1") {
        violations.push(format!("{normalized} schema_version must be 0.1"));
    }
    if json_string_field(corpus, "spec").as_deref() != Some("RIPR-SPEC-0035") {
        violations.push(format!("{normalized} spec must be RIPR-SPEC-0035"));
    }

    require_string_array_contains_all(
        corpus,
        "evidence_classes",
        EVIDENCE_QUALITY_BENCHMARK_REQUIRED_CLASSES,
        "Lane 1 evidence-quality benchmark",
        violations,
    );
    require_string_array_contains_all(
        corpus,
        "required_case_kinds",
        EVIDENCE_QUALITY_BENCHMARK_REQUIRED_CASE_KINDS,
        "Lane 1 evidence-quality benchmark",
        violations,
    );
    if !matches!(corpus.get("capability_scope"), Some(Value::Object(_))) {
        violations.push(
            "Lane 1 evidence-quality benchmark is missing capability_scope object".to_string(),
        );
    }
    if !matches!(corpus.get("calibration_scope"), Some(Value::Object(_))) {
        violations.push(
            "Lane 1 evidence-quality benchmark is missing calibration_scope object".to_string(),
        );
    }
    if !matches!(corpus.get("audit_expectations"), Some(Value::Object(_))) {
        violations.push(
            "Lane 1 evidence-quality benchmark is missing audit_expectations object".to_string(),
        );
    }

    let Some(cases) = corpus.get("cases").and_then(Value::as_array) else {
        violations.push(format!("{normalized} is missing cases array"));
        return;
    };

    let mut seen_ids = BTreeSet::new();
    let mut seen_classes = BTreeSet::new();
    let mut seen_kinds = BTreeSet::new();
    let mut has_runtime_only_guard = false;
    let mut has_line_movement_guard = false;
    for case in cases {
        let case_id = json_string_field(case, "id").unwrap_or_else(|| "unknown".to_string());
        if !seen_ids.insert(case_id.clone()) {
            violations.push(format!(
                "Lane 1 evidence-quality benchmark case {case_id} is duplicated"
            ));
        }
        validate_evidence_quality_benchmark_case(
            &case_id,
            case,
            &mut seen_classes,
            &mut seen_kinds,
            &mut has_runtime_only_guard,
            &mut has_line_movement_guard,
            violations,
        );
    }

    for required in EVIDENCE_QUALITY_BENCHMARK_REQUIRED_CLASSES {
        if !seen_classes.contains(*required) {
            violations.push(format!(
                "Lane 1 evidence-quality benchmark is missing evidence_class {required}"
            ));
        }
    }
    for required in EVIDENCE_QUALITY_BENCHMARK_REQUIRED_CASE_KINDS {
        if !seen_kinds.contains(*required) {
            violations.push(format!(
                "Lane 1 evidence-quality benchmark is missing case_kind {required}"
            ));
        }
    }
    for required in EVIDENCE_QUALITY_BENCHMARK_REQUIRED_CONFIG_POLICY_CASES {
        if !seen_ids.contains(*required) {
            violations.push(format!(
                "Lane 1 evidence-quality benchmark is missing config/policy case {required}"
            ));
        }
    }
    if !has_runtime_only_guard {
        violations.push(
            "Lane 1 evidence-quality benchmark must include a runtime-only nonstatic guard"
                .to_string(),
        );
    }
    if !has_line_movement_guard {
        violations.push(
            "Lane 1 evidence-quality benchmark must include a line-movement identity guard"
                .to_string(),
        );
    }
}

fn validate_evidence_quality_benchmark_case(
    case_id: &str,
    case: &Value,
    seen_classes: &mut BTreeSet<String>,
    seen_kinds: &mut BTreeSet<String>,
    has_runtime_only_guard: &mut bool,
    has_line_movement_guard: &mut bool,
    violations: &mut Vec<String>,
) {
    for field in ["description", "fixture_reference", "expected_claim"] {
        require_lane1_json_string_at(case, field, case_id, violations);
    }
    require_non_empty_string_array_at(case, "must_not_claim", case_id, violations);
    require_lane1_json_string_at(case, "repair_route", case_id, violations);

    let evidence_class = json_string_field(case, "evidence_class");
    match evidence_class.as_deref() {
        Some(class) if EVIDENCE_QUALITY_BENCHMARK_REQUIRED_CLASSES.contains(&class) => {
            seen_classes.insert(class.to_string());
        }
        Some(class) => violations.push(format!(
            "Lane 1 evidence-quality benchmark case {case_id} has unsupported evidence_class {class}"
        )),
        None => violations.push(format!(
            "Lane 1 evidence-quality benchmark case {case_id} is missing evidence_class"
        )),
    }

    let case_kind = json_string_field(case, "case_kind");
    match case_kind.as_deref() {
        Some(kind) if EVIDENCE_QUALITY_BENCHMARK_REQUIRED_CASE_KINDS.contains(&kind) => {
            seen_kinds.insert(kind.to_string());
        }
        Some(kind) => violations.push(format!(
            "Lane 1 evidence-quality benchmark case {case_id} has unsupported case_kind {kind}"
        )),
        None => violations.push(format!(
            "Lane 1 evidence-quality benchmark case {case_id} is missing case_kind"
        )),
    }

    match json_string_field(case, "maturity_scope").as_deref() {
        Some("static_only" | "fixture_backed" | "calibrated" | "ambiguous" | "unsupported") => {}
        Some(scope) => violations.push(format!(
            "Lane 1 evidence-quality benchmark case {case_id} has unsupported maturity_scope {scope}"
        )),
        None => violations.push(format!(
            "Lane 1 evidence-quality benchmark case {case_id} is missing maturity_scope"
        )),
    }

    match case.get("expected_repo_exposure") {
        Some(expected @ Value::Object(_)) => {
            let creates_static_gap = json_bool_field(expected, "static_gap_created");
            let record = expected.get("evidence_record");
            if evidence_class.as_deref() == Some("runtime_only_signal") {
                if creates_static_gap != Some(false) || !matches!(record, Some(Value::Null)) {
                    violations.push(format!(
                        "Lane 1 evidence-quality benchmark runtime-only case {case_id} must keep static_gap_created=false and evidence_record=null"
                    ));
                }
            } else if !matches!(record, Some(Value::Object(_))) {
                violations.push(format!(
                    "Lane 1 evidence-quality benchmark case {case_id} expected_repo_exposure.evidence_record must be an object"
                ));
            } else if record
                .and_then(|record| audit_string(record, &["gap_state"]))
                .as_deref()
                == Some("actionable")
                && record.is_none_or(audit_verify_command_is_missing)
            {
                violations.push(format!(
                    "Lane 1 evidence-quality benchmark actionable case {case_id} must include a concrete expected_repo_exposure.evidence_record.verify_command"
                ));
            }
        }
        _ => violations.push(format!(
            "Lane 1 evidence-quality benchmark case {case_id} is missing expected_repo_exposure object"
        )),
    }

    if !matches!(case.get("expected_audit_signal"), Some(Value::Object(_))) {
        violations.push(format!(
            "Lane 1 evidence-quality benchmark case {case_id} is missing expected_audit_signal object"
        ));
    }

    if case_kind.as_deref() == Some("static_limitation") {
        if json_string_field(case, "static_limitation_category").is_none() {
            violations.push(format!(
                "Lane 1 evidence-quality benchmark static-limitation case {case_id} is missing static_limitation_category"
            ));
        }
        if audit_string(
            case,
            &[
                "expected_repo_exposure",
                "evidence_record",
                "static_limitation",
                "category",
            ],
        )
        .is_some()
        {
            violations.push(format!(
                "Lane 1 evidence-quality benchmark static-limitation case {case_id} must keep static_limitation_category at the case level"
            ));
        }
    }

    if evidence_class.as_deref() == Some("runtime_only_signal") {
        match case.get("calibration") {
            Some(calibration @ Value::Object(_)) => {
                if json_bool_field(calibration, "runtime_signal") != Some(true) {
                    violations.push(format!(
                        "Lane 1 evidence-quality benchmark runtime-only case {case_id} must set calibration.runtime_signal=true"
                    ));
                }
            }
            _ => violations.push(format!(
                "Lane 1 evidence-quality benchmark runtime-only case {case_id} is missing calibration object"
            )),
        }
        *has_runtime_only_guard = true;
    }

    if evidence_class.as_deref() == Some("ambiguous_runtime_join") {
        match case.get("calibration") {
            Some(calibration @ Value::Object(_)) => {
                if json_string_field(calibration, "join_status").as_deref() != Some("ambiguous") {
                    violations.push(format!(
                        "Lane 1 evidence-quality benchmark ambiguous join case {case_id} must set calibration.join_status=ambiguous"
                    ));
                }
            }
            _ => violations.push(format!(
                "Lane 1 evidence-quality benchmark ambiguous join case {case_id} is missing calibration object"
            )),
        }
    }

    if case_kind.as_deref() == Some("metamorphic_line_movement") {
        validate_benchmark_line_movement(case_id, case, has_line_movement_guard, violations);
    }
}

fn validate_benchmark_line_movement(
    case_id: &str,
    case: &Value,
    has_line_movement_guard: &mut bool,
    violations: &mut Vec<String>,
) {
    let Some(metamorphic @ Value::Object(_)) = case.get("metamorphic") else {
        violations.push(format!(
            "Lane 1 evidence-quality benchmark line-movement case {case_id} is missing metamorphic object"
        ));
        return;
    };
    let before_id = audit_string(metamorphic, &["before", "canonical_gap_id"]);
    let after_id = audit_string(metamorphic, &["after", "canonical_gap_id"]);
    let before_line = audit_usize(metamorphic, &["before", "line"]);
    let after_line = audit_usize(metamorphic, &["after", "line"]);
    if before_id.is_none() || before_id != after_id {
        violations.push(format!(
            "Lane 1 evidence-quality benchmark line-movement case {case_id} must preserve canonical_gap_id"
        ));
    }
    if before_line.is_none() || after_line.is_none() || before_line == after_line {
        violations.push(format!(
            "Lane 1 evidence-quality benchmark line-movement case {case_id} must move line numbers"
        ));
    }
    *has_line_movement_guard = true;
}

pub(crate) fn validate_finding_alignment_dogfood_fixture_corpus(
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let root = Path::new("fixtures/finding-alignment-dogfood");
    for required in ["SPEC.md", "corpus.json"] {
        let path = root.join(required);
        if !path.exists() {
            violations.push(format!(
                "finding alignment dogfood fixture corpus is missing {}",
                normalize_path(&path)
            ));
        }
    }

    validate_finding_alignment_dogfood_fixture_corpus_at(
        Path::new(FINDING_ALIGNMENT_DOGFOOD_CORPUS),
        violations,
    )
}

fn validate_finding_alignment_dogfood_fixture_corpus_at(
    path: &Path,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    if !path.exists() {
        violations.push(format!(
            "finding alignment dogfood corpus is missing {}",
            normalize_path(path)
        ));
        return Ok(());
    }

    let scenarios = dogfood_finding_alignment_scenarios();
    let mut seen = BTreeMap::new();
    for scenario in &scenarios {
        if seen
            .insert(scenario.name.clone(), scenario.gap_state.clone())
            .is_some()
        {
            violations.push(format!(
                "finding alignment dogfood case {} is duplicated",
                scenario.name
            ));
        }
        let run = dogfood_finding_alignment_run(scenario);
        for error in run.errors {
            violations.push(format!(
                "finding alignment dogfood case {}: {error}",
                scenario.name
            ));
        }
    }

    for (case_id, gap_state) in FINDING_ALIGNMENT_DOGFOOD_REQUIRED_CASES {
        match seen.get(*case_id) {
            Some(actual) if actual == gap_state => {}
            Some(actual) => violations.push(format!(
                "finding alignment dogfood case {case_id} must have gap_state {gap_state}, got {actual}"
            )),
            None => violations.push(format!(
                "finding alignment dogfood corpus is missing case {case_id}"
            )),
        }
    }

    Ok(())
}

pub(crate) fn validate_real_repair_attempt_fixture_corpus(
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let root = Path::new("fixtures/real-repair-attempts");
    let corpus = root.join("corpus.json");
    if !corpus.exists() {
        violations.push(format!(
            "real repair attempt fixture corpus is missing {}",
            normalize_path(&corpus)
        ));
    }
    validate_real_repair_attempt_fixture_corpus_at(
        Path::new(REAL_REPAIR_ATTEMPTS_CORPUS),
        violations,
    )
}

fn validate_real_repair_attempt_fixture_corpus_at(
    path: &Path,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    if !path.exists() {
        violations.push(format!(
            "real repair attempt corpus is missing {}",
            normalize_path(path)
        ));
        return Ok(());
    }

    let scenarios = dogfood_real_repair_attempt_scenarios();
    let mut seen = BTreeMap::new();
    let mut outcomes = BTreeMap::<String, usize>::new();
    for scenario in &scenarios {
        if seen
            .insert(scenario.name.clone(), scenario.outcome.clone())
            .is_some()
        {
            violations.push(format!(
                "real repair attempt case {} is duplicated",
                scenario.name
            ));
        }
        *outcomes.entry(scenario.outcome.clone()).or_default() += 1;
        let run = dogfood_real_repair_attempt_run(scenario);
        for error in run.errors {
            violations.push(format!(
                "real repair attempt case {}: {error}",
                scenario.name
            ));
        }
    }

    for (case_id, outcome) in REAL_REPAIR_ATTEMPTS_REQUIRED_CASES {
        match seen.get(*case_id) {
            Some(actual) if actual == outcome => {}
            Some(actual) => violations.push(format!(
                "real repair attempt case {case_id} must have outcome {outcome}, got {actual}"
            )),
            None => violations.push(format!(
                "real repair attempt corpus is missing case {case_id}"
            )),
        }
    }
    if scenarios.len() < 3 {
        violations
            .push("real repair attempt corpus must record at least three attempts".to_string());
    }
    if !outcomes.contains_key("evidence_improved") {
        violations.push(
            "real repair attempt corpus must include at least one evidence_improved attempt"
                .to_string(),
        );
    }
    if !outcomes.contains_key("evidence_unchanged") {
        violations.push(
            "real repair attempt corpus must include at least one evidence_unchanged attempt"
                .to_string(),
        );
    }
    if !outcomes.contains_key("attempted_no_receipt") {
        violations.push(
            "real repair attempt corpus must include at least one attempted_no_receipt attempt"
                .to_string(),
        );
    }

    Ok(())
}

pub(crate) fn validate_python_real_repo_eval_fixture_corpus(
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let root = Path::new("fixtures/python-real-repo-evals");
    for required in ["SPEC.md", "corpus.json"] {
        let path = root.join(required);
        if !path.exists() {
            violations.push(format!(
                "Python real-repo eval fixture corpus is missing {}",
                normalize_path(&path)
            ));
        }
    }
    validate_python_real_repo_eval_fixture_corpus_at(
        Path::new(PYTHON_REAL_REPO_EVAL_CORPUS),
        violations,
    )
}

fn validate_python_real_repo_eval_fixture_corpus_at(
    path: &Path,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    if !path.exists() {
        violations.push(format!(
            "Python real-repo eval corpus is missing {}",
            normalize_path(path)
        ));
        return Ok(());
    }

    let scenarios = dogfood_python_real_repo_eval_scenarios_at(path);
    let mut seen = BTreeMap::new();
    let mut closed_cases = 0usize;
    let mut full_ranked_top_3_cases = 0usize;
    let mut runs = Vec::new();
    for scenario in &scenarios {
        if seen
            .insert(scenario.name.clone(), scenario.gap_movement.clone())
            .is_some()
        {
            violations.push(format!(
                "Python real-repo eval case {} is duplicated",
                scenario.name
            ));
        }
        if scenario.gap_movement == "closed" {
            closed_cases += 1;
        }
        if scenario.ranked_top_3_findings.len() == 3 {
            full_ranked_top_3_cases += 1;
        }
        let run = dogfood_python_real_repo_eval_run(scenario);
        for error in &run.errors {
            violations.push(format!(
                "Python real-repo eval case {}: {error}",
                scenario.name
            ));
        }
        runs.push(run);
    }

    for (case_id, gap_movement) in PYTHON_REAL_REPO_EVAL_REQUIRED_CASES {
        match seen.get(*case_id) {
            Some(actual) if actual == gap_movement => {}
            Some(actual) => violations.push(format!(
                "Python real-repo eval case {case_id} must have gap_movement {gap_movement}, got {actual}"
            )),
            None => violations.push(format!(
                "Python real-repo eval corpus is missing case {case_id}"
            )),
        }
    }
    if scenarios.is_empty() {
        violations.push("Python real-repo eval corpus must record at least one case".to_string());
    }
    if closed_cases == 0 {
        violations.push(
            "Python real-repo eval corpus must include at least one closed gap receipt".to_string(),
        );
    }
    if full_ranked_top_3_cases == 0 {
        violations.push(
            "Python real-repo eval corpus must include at least one full top-3 repair-card capture"
                .to_string(),
        );
    }
    let quality = dogfood_python_repair_routing_quality_summary(&runs);
    if quality.gate_status != "pass" {
        violations.push(format!(
            "Python repair-routing quality gate is {}: {}",
            quality.gate_status, quality.gate_reason
        ));
    }

    let static_limit_scenarios = dogfood_python_static_limit_eval_scenarios_at(path);
    let mut seen_static_limits = BTreeMap::new();
    for scenario in &static_limit_scenarios {
        if seen_static_limits
            .insert(scenario.name.clone(), scenario.static_limit_kind.clone())
            .is_some()
        {
            violations.push(format!(
                "Python static-limit eval case {} is duplicated",
                scenario.name
            ));
        }
        let run = dogfood_python_static_limit_eval_run(scenario);
        for error in &run.errors {
            violations.push(format!(
                "Python static-limit eval case {}: {error}",
                scenario.name
            ));
        }
    }
    for (case_id, static_limit_kind) in PYTHON_REAL_REPO_EVAL_REQUIRED_STATIC_LIMIT_CASES {
        match seen_static_limits.get(*case_id) {
            Some(actual) if actual == static_limit_kind => {}
            Some(actual) => violations.push(format!(
                "Python static-limit eval case {case_id} must have static_limit_kind {static_limit_kind}, got {actual}"
            )),
            None => violations.push(format!(
                "Python real-repo eval corpus is missing static-limit case {case_id}"
            )),
        }
    }

    let no_action_scenarios = dogfood_python_no_action_eval_scenarios_at(path);
    let mut seen_no_actions = BTreeMap::new();
    for scenario in &no_action_scenarios {
        if seen_no_actions
            .insert(scenario.name.clone(), scenario.no_action_kind.clone())
            .is_some()
        {
            violations.push(format!(
                "Python no-action eval case {} is duplicated",
                scenario.name
            ));
        }
        let run = dogfood_python_no_action_eval_run(scenario);
        for error in &run.errors {
            violations.push(format!(
                "Python no-action eval case {}: {error}",
                scenario.name
            ));
        }
    }
    for (case_id, no_action_kind) in PYTHON_REAL_REPO_EVAL_REQUIRED_NO_ACTION_CASES {
        match seen_no_actions.get(*case_id) {
            Some(actual) if actual == no_action_kind => {}
            Some(actual) => violations.push(format!(
                "Python no-action eval case {case_id} must have no_action_kind {no_action_kind}, got {actual}"
            )),
            None => violations.push(format!(
                "Python real-repo eval corpus is missing no-action case {case_id}"
            )),
        }
    }

    Ok(())
}

pub(crate) fn validate_typescript_bun_ub_calibration_fixture_corpus(
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let root = Path::new("fixtures/typescript-bun-ub-calibration");
    for required in ["SPEC.md", "corpus.json"] {
        let path = root.join(required);
        if !path.exists() {
            violations.push(format!(
                "TypeScript Bun UB calibration fixture corpus is missing {}",
                normalize_path(&path)
            ));
        }
    }
    validate_typescript_bun_ub_calibration_fixture_corpus_at(
        Path::new(TYPESCRIPT_BUN_UB_CALIBRATION_CORPUS),
        violations,
    )
}

pub(crate) fn validate_typescript_bun_ub_calibration_fixture_corpus_at(
    path: &Path,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    if !path.exists() {
        violations.push(format!(
            "TypeScript Bun UB calibration corpus is missing {}",
            normalize_path(path)
        ));
        return Ok(());
    }

    let cases = typescript_bun_ub_calibration_cases_at(path);
    let mut seen = BTreeMap::new();
    let mut verdicts = BTreeSet::<String>::new();
    for case in &cases {
        if seen
            .insert(case.name.clone(), case.expected_verdict.clone())
            .is_some()
        {
            violations.push(format!(
                "TypeScript Bun UB calibration case {} is duplicated",
                case.name
            ));
        }
        verdicts.insert(case.expected_verdict.clone());
        for error in typescript_bun_ub_calibration_case_errors(case) {
            violations.push(format!(
                "TypeScript Bun UB calibration case {}: {error}",
                case.name
            ));
        }
    }

    for (case_id, verdict) in TYPESCRIPT_BUN_UB_CALIBRATION_REQUIRED_CASES {
        match seen.get(*case_id) {
            Some(actual) if actual == verdict => {}
            Some(actual) => violations.push(format!(
                "TypeScript Bun UB calibration case {case_id} must have expected_verdict {verdict}, got {actual}"
            )),
            None => violations.push(format!(
                "TypeScript Bun UB calibration corpus is missing case {case_id}"
            )),
        }
    }
    for required in [
        "ts_discriminated",
        "ts_missing_shared",
        "ts_missing_resizable",
        "ts_missing_external_oracle",
        "ts_mention_not_observer",
        "bridge_unknown",
    ] {
        if !verdicts.contains(required) {
            violations.push(format!(
                "TypeScript Bun UB calibration corpus must include verdict {required}"
            ));
        }
    }

    Ok(())
}

pub(crate) fn validate_cross_language_oracle_graph_fixture_corpus(
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let root = Path::new("fixtures/cross-language-oracle-graph-corpus");
    for required in ["SPEC.md", "corpus.json"] {
        let path = root.join(required);
        if !path.exists() {
            violations.push(format!(
                "Cross-language oracle graph fixture corpus is missing {}",
                normalize_path(&path)
            ));
        }
    }
    validate_cross_language_oracle_graph_fixture_corpus_at(
        Path::new(CROSS_LANGUAGE_ORACLE_GRAPH_CORPUS),
        violations,
    )
}

pub(crate) fn validate_cross_language_oracle_graph_fixture_corpus_at(
    path: &Path,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    if !path.exists() {
        violations.push(format!(
            "Cross-language oracle graph corpus is missing {}",
            normalize_path(path)
        ));
        return Ok(());
    }

    let cases = cross_language_oracle_graph_cases_at(path);
    let mut seen = BTreeMap::new();
    let mut states = BTreeSet::<String>::new();
    for case in &cases {
        if seen
            .insert(case.name.clone(), case.expected_state.clone())
            .is_some()
        {
            violations.push(format!(
                "Cross-language oracle graph case {} is duplicated",
                case.name
            ));
        }
        states.insert(case.expected_state.clone());
        for error in cross_language_oracle_graph_case_errors(case) {
            violations.push(format!(
                "Cross-language oracle graph case {}: {error}",
                case.name
            ));
        }
    }

    for (case_id, expected_state) in CROSS_LANGUAGE_ORACLE_GRAPH_REQUIRED_CASES {
        match seen.get(*case_id) {
            Some(actual) if actual == expected_state => {}
            Some(actual) => violations.push(format!(
                "Cross-language oracle graph case {case_id} must have expected_state {expected_state}, got {actual}"
            )),
            None => violations.push(format!(
                "Cross-language oracle graph corpus is missing case {case_id}"
            )),
        }
    }
    for required in [
        "rust_ungripped_ts_discriminated",
        "rust_ungripped_ts_missing_discriminator",
        "rust_ungripped_ts_missing_external_oracle",
        "ts_mention_not_observer",
        "bridge_unknown",
        "cross_language_target_unresolved",
        "public_reachable_panic_boundary_unrevealed",
        "named_static_limitation",
    ] {
        if !states.contains(required) {
            violations.push(format!(
                "Cross-language oracle graph corpus must include state {required}"
            ));
        }
    }

    Ok(())
}

pub(crate) fn validate_bun_ub_cross_language_dogfood_fixture_corpus(
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let root = Path::new("fixtures/bun-ub-cross-language-dogfood");
    for required in ["SPEC.md", "corpus.json"] {
        let path = root.join(required);
        if !path.exists() {
            violations.push(format!(
                "Bun UB cross-language dogfood fixture corpus is missing {}",
                normalize_path(&path)
            ));
        }
    }
    validate_bun_ub_cross_language_dogfood_fixture_corpus_at(
        Path::new(BUN_UB_CROSS_LANGUAGE_DOGFOOD_CORPUS),
        violations,
    )
}

fn validate_bun_ub_cross_language_dogfood_fixture_corpus_at(
    path: &Path,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    if !path.exists() {
        violations.push(format!(
            "Bun UB cross-language dogfood corpus is missing {}",
            normalize_path(path)
        ));
        return Ok(());
    }

    let scenarios = dogfood_bun_ub_cross_language_scenarios_at(path);
    let mut seen = BTreeMap::new();
    let mut observed_states = BTreeSet::<String>::new();
    let mut packet_ready_cases = 0usize;

    for scenario in &scenarios {
        if seen
            .insert(scenario.name.clone(), scenario.observed_state.clone())
            .is_some()
        {
            violations.push(format!(
                "Bun UB cross-language dogfood case {} is duplicated",
                scenario.name
            ));
        }
        observed_states.insert(scenario.observed_state.clone());
        if scenario.repair_packet_ready {
            packet_ready_cases += 1;
        }
        let run = dogfood_bun_ub_cross_language_run(scenario);
        for error in run.errors {
            violations.push(format!(
                "Bun UB cross-language dogfood case {}: {error}",
                scenario.name
            ));
        }
    }

    for (case_id, observed_state) in BUN_UB_CROSS_LANGUAGE_DOGFOOD_REQUIRED_CASES {
        match seen.get(*case_id) {
            Some(actual) if actual == observed_state => {}
            Some(actual) => violations.push(format!(
                "Bun UB cross-language dogfood case {case_id} must have observed_state {observed_state}, got {actual}"
            )),
            None => violations.push(format!(
                "Bun UB cross-language dogfood corpus is missing case {case_id}"
            )),
        }
    }
    if scenarios.len() < BUN_UB_CROSS_LANGUAGE_DOGFOOD_REQUIRED_CASES.len() {
        violations.push(
            "Bun UB cross-language dogfood corpus must record the calibrated receipt set"
                .to_string(),
        );
    }
    for required_state in [
        "rust_ungripped_ts_discriminated",
        "rust_ungripped_ts_missing_discriminator",
        "ts_mention_not_observer",
        "public_reachable_panic_boundary_unrevealed",
    ] {
        if !observed_states.contains(required_state) {
            violations.push(format!(
                "Bun UB cross-language dogfood corpus must include state {required_state}"
            ));
        }
    }
    if packet_ready_cases > 0 {
        violations
            .push("Bun UB cross-language dogfood corpus must not claim repair packets".to_string());
    }

    Ok(())
}

pub(crate) fn validate_typescript_preview_repair_loop_fixture_corpus(
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let root = Path::new("fixtures/typescript-preview-repair-loop");
    let corpus = root.join("corpus.json");
    if !corpus.exists() {
        violations.push(format!(
            "TypeScript preview repair-loop fixture corpus is missing {}",
            normalize_path(&corpus)
        ));
    }
    validate_typescript_preview_repair_loop_fixture_corpus_at(
        Path::new(TYPESCRIPT_PREVIEW_REPAIR_LOOP_CORPUS),
        violations,
    )
}

fn validate_typescript_preview_repair_loop_fixture_corpus_at(
    path: &Path,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    if !path.exists() {
        violations.push(format!(
            "TypeScript preview repair-loop corpus is missing {}",
            normalize_path(path)
        ));
        return Ok(());
    }

    let scenarios = dogfood_typescript_preview_repair_loop_scenarios_at(path);
    let mut seen = BTreeMap::new();
    let mut outcomes = BTreeMap::<String, usize>::new();
    let mut languages = BTreeSet::<String>::new();
    let mut static_limit_cases = 0usize;
    let mut weak_oracle_cases = 0usize;
    let mut skipped_cases = 0usize;
    let mut packet_ready_cases = 0usize;

    for scenario in &scenarios {
        if seen
            .insert(scenario.name.clone(), scenario.outcome.clone())
            .is_some()
        {
            violations.push(format!(
                "TypeScript preview repair-loop case {} is duplicated",
                scenario.name
            ));
        }
        *outcomes.entry(scenario.outcome.clone()).or_default() += 1;
        languages.insert(scenario.language.clone());
        if scenario.gap_state == "static_limitation" {
            static_limit_cases += 1;
        }
        if scenario.outcome == "weak_oracle_downgraded" {
            weak_oracle_cases += 1;
        }
        if scenario.outcome == "intentionally_skipped" {
            skipped_cases += 1;
        }
        if scenario.repair_packet_ready {
            packet_ready_cases += 1;
        }
        let run = dogfood_typescript_preview_repair_loop_run(scenario);
        for error in run.errors {
            violations.push(format!(
                "TypeScript preview repair-loop case {}: {error}",
                scenario.name
            ));
        }
    }

    for (case_id, outcome) in TYPESCRIPT_PREVIEW_REPAIR_LOOP_REQUIRED_CASES {
        match seen.get(*case_id) {
            Some(actual) if actual == outcome => {}
            Some(actual) => violations.push(format!(
                "TypeScript preview repair-loop case {case_id} must have outcome {outcome}, got {actual}"
            )),
            None => violations.push(format!(
                "TypeScript preview repair-loop corpus is missing case {case_id}"
            )),
        }
    }
    if scenarios.len() < 5 {
        violations.push(
            "TypeScript preview repair-loop corpus must record at least five cases".to_string(),
        );
    }
    if !languages.contains("typescript") {
        violations
            .push("TypeScript preview repair-loop corpus must include TypeScript".to_string());
    }
    if !languages.contains("javascript") {
        violations
            .push("TypeScript preview repair-loop corpus must include JavaScript".to_string());
    }
    if !outcomes.contains_key("proof_improved") {
        violations
            .push("TypeScript preview repair-loop corpus must include improved proof".to_string());
    }
    if static_limit_cases == 0 {
        violations.push(
            "TypeScript preview repair-loop corpus must include a static limitation".to_string(),
        );
    }
    if weak_oracle_cases == 0 {
        violations.push(
            "TypeScript preview repair-loop corpus must include a weak-oracle downgrade"
                .to_string(),
        );
    }
    if skipped_cases == 0 {
        violations.push(
            "TypeScript preview repair-loop corpus must include an intentionally skipped case"
                .to_string(),
        );
    }
    if packet_ready_cases == 0 {
        violations.push(
            "TypeScript preview repair-loop corpus must include a checked packet-ready advisory receipt"
                .to_string(),
        );
    }

    Ok(())
}

pub(crate) fn validate_typescript_preview_false_actionable_audit_fixture_corpus(
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let root = Path::new("fixtures/typescript-preview-false-actionable-audit");
    let corpus = root.join("corpus.json");
    if !corpus.exists() {
        violations.push(format!(
            "TypeScript preview false-actionable audit corpus is missing {}",
            normalize_path(&corpus)
        ));
    }
    validate_typescript_preview_false_actionable_audit_fixture_corpus_at(
        Path::new(TYPESCRIPT_PREVIEW_FALSE_ACTIONABLE_AUDIT_CORPUS),
        violations,
    )
}

fn validate_typescript_preview_false_actionable_audit_fixture_corpus_at(
    path: &Path,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    if !path.exists() {
        violations.push(format!(
            "TypeScript preview false-actionable audit corpus is missing {}",
            normalize_path(path)
        ));
        return Ok(());
    }

    let cases = typescript_preview_false_actionable_audit_cases_at(path);
    let mut seen = BTreeMap::new();
    let mut dispositions = BTreeSet::<String>::new();
    for case in &cases {
        if seen
            .insert(case.name.clone(), case.disposition.clone())
            .is_some()
        {
            violations.push(format!(
                "TypeScript preview false-actionable audit case {} is duplicated",
                case.name
            ));
        }
        dispositions.insert(case.disposition.clone());
        for error in typescript_preview_false_actionable_audit_case_errors(case) {
            violations.push(format!(
                "TypeScript preview false-actionable audit case {}: {error}",
                case.name
            ));
        }
    }

    for (case_id, disposition) in TYPESCRIPT_PREVIEW_FALSE_ACTIONABLE_AUDIT_REQUIRED_CASES {
        match seen.get(*case_id) {
            Some(actual) if actual == disposition => {}
            Some(actual) => violations.push(format!(
                "TypeScript preview false-actionable audit case {case_id} must have disposition {disposition}, got {actual}"
            )),
            None => violations.push(format!(
                "TypeScript preview false-actionable audit corpus is missing case {case_id}"
            )),
        }
    }
    for required in [
        "safe_advisory",
        "named_static_limitation",
        "candidate_future_support",
        "must_remain_non_actionable",
    ] {
        if !dispositions.contains(required) {
            violations.push(format!(
                "TypeScript preview false-actionable audit corpus must include disposition {required}"
            ));
        }
    }

    Ok(())
}

pub(crate) fn validate_surface_projection_alignment_fixture_corpus(
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let root = Path::new("fixtures/surface-projection-alignment");
    let corpus = root.join("corpus.json");
    if !corpus.exists() {
        violations.push(format!(
            "surface projection alignment fixture corpus is missing {}",
            normalize_path(&corpus)
        ));
    }
    validate_surface_projection_alignment_fixture_corpus_at(
        Path::new(SURFACE_PROJECTION_ALIGNMENT_CORPUS),
        violations,
    )
}

fn validate_surface_projection_alignment_fixture_corpus_at(
    path: &Path,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    if !path.exists() {
        violations.push(format!(
            "surface projection alignment corpus is missing {}",
            normalize_path(path)
        ));
        return Ok(());
    }

    let scenarios = dogfood_surface_projection_alignment_scenarios();
    let mut seen = BTreeMap::new();
    for scenario in &scenarios {
        if seen
            .insert(scenario.name.clone(), scenario.outcome.clone())
            .is_some()
        {
            violations.push(format!(
                "surface projection alignment case {} is duplicated",
                scenario.name
            ));
        }
        let run = dogfood_surface_projection_alignment_run(scenario);
        for error in run.errors {
            violations.push(format!(
                "surface projection alignment case {}: {error}",
                scenario.name
            ));
        }
    }

    for (case_id, outcome) in SURFACE_PROJECTION_ALIGNMENT_REQUIRED_CASES {
        match seen.get(*case_id) {
            Some(actual) if actual == outcome => {}
            Some(actual) => violations.push(format!(
                "surface projection alignment case {case_id} must have outcome {outcome}, got {actual}"
            )),
            None => violations.push(format!(
                "surface projection alignment corpus is missing case {case_id}"
            )),
        }
    }

    Ok(())
}

pub(crate) fn validate_user_surface_projection_alignment_fixture_corpus(
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let root = Path::new("fixtures/user-surface-projection-alignment");
    let corpus = root.join("corpus.json");
    if !corpus.exists() {
        violations.push(format!(
            "user surface projection alignment fixture corpus is missing {}",
            normalize_path(&corpus)
        ));
    }
    validate_user_surface_projection_alignment_fixture_corpus_at(
        Path::new(USER_SURFACE_PROJECTION_ALIGNMENT_CORPUS),
        violations,
    )
}

fn validate_user_surface_projection_alignment_fixture_corpus_at(
    path: &Path,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    if !path.exists() {
        violations.push(format!(
            "user surface projection alignment corpus is missing {}",
            normalize_path(path)
        ));
        return Ok(());
    }

    let scenarios = dogfood_user_surface_projection_scenarios();
    let surface_projection_scenarios = dogfood_surface_projection_alignment_scenarios();
    let mut seen_names = BTreeSet::new();
    let mut seen_surfaces = BTreeSet::new();
    for scenario in &scenarios {
        if !seen_names.insert(scenario.name.clone()) {
            violations.push(format!(
                "user surface projection alignment case {} is duplicated",
                scenario.name
            ));
        }
        seen_surfaces.insert(scenario.surface.clone());
        for error in dogfood_user_surface_projection_run(scenario).errors {
            violations.push(format!(
                "user surface projection alignment case {}: {error}",
                scenario.name
            ));
        }
        for error in
            user_surface_projection_source_alignment_errors(scenario, &surface_projection_scenarios)
        {
            violations.push(format!(
                "user surface projection alignment case {}: {error}",
                scenario.name
            ));
        }
    }

    for required in USER_SURFACE_PROJECTION_REQUIRED_SURFACES {
        if !seen_surfaces.contains(*required) {
            violations.push(format!(
                "user surface projection alignment corpus is missing surface {required}"
            ));
        }
    }
    violations.extend(user_surface_projection_required_run_status_violations(
        &scenarios,
    ));

    Ok(())
}

pub(crate) fn user_surface_projection_source_alignment_errors(
    scenario: &DogfoodUserSurfaceProjectionScenario,
    surface_projection_scenarios: &[DogfoodSurfaceProjectionAlignmentScenario],
) -> Vec<String> {
    let mut errors = Vec::new();
    if scenario.run_status != "full" {
        if !scenario.source_alignment_case.trim().is_empty() {
            errors.push(
                "limited or stale run_status must not carry source_alignment_case".to_string(),
            );
        }
        return errors;
    }

    if scenario.source_alignment_case.trim().is_empty()
        || scenario.source_alignment_case == "unknown"
    {
        errors.push("full run_status must name source_alignment_case".to_string());
        return errors;
    }

    let Some(source) = surface_projection_scenarios
        .iter()
        .find(|source| source.name == scenario.source_alignment_case)
    else {
        errors.push(format!(
            "source_alignment_case {} must exist in surface projection alignment corpus",
            scenario.source_alignment_case
        ));
        return errors;
    };

    for (label, expected, actual) in [
        (
            "canonical_gap_id",
            &source.canonical_gap_id,
            &scenario.canonical_gap_id,
        ),
        ("packet_id", &source.packet_id, &scenario.packet_id),
        ("repair_kind", &source.repair_kind, &scenario.repair_kind),
        (
            "verify_command",
            &source.verify_command,
            &scenario.verify_command,
        ),
        (
            "receipt_command",
            &source.receipt_command,
            &scenario.receipt_command,
        ),
        (
            "top_next_action_kind",
            &source.expected_top_next_action_kind,
            &scenario.top_next_action_kind,
        ),
    ] {
        if expected != actual {
            errors.push(format!(
                "{label} must match source_alignment_case {}, expected {}, got {}",
                scenario.source_alignment_case, expected, actual
            ));
        }
    }

    if !source
        .advisory_consumers
        .iter()
        .any(|consumer| consumer == &scenario.surface)
    {
        errors.push(format!(
            "surface {} must be listed by source_alignment_case {} advisory_consumers",
            scenario.surface, scenario.source_alignment_case
        ));
    }

    errors
}

pub(crate) fn user_surface_projection_required_run_status_violations(
    scenarios: &[DogfoodUserSurfaceProjectionScenario],
) -> Vec<String> {
    let seen_run_statuses = scenarios
        .iter()
        .map(|scenario| scenario.run_status.as_str())
        .collect::<BTreeSet<_>>();
    USER_SURFACE_PROJECTION_REQUIRED_RUN_STATUSES
        .iter()
        .filter(|required| !seen_run_statuses.contains(**required))
        .map(|required| {
            format!("user surface projection alignment corpus is missing run_status {required}")
        })
        .collect()
}
