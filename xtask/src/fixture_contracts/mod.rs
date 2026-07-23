//! Fixture-contract cluster: the `check-fixture-contracts` orchestrator, its
//! corpus const tables, and the python/perl corpus validators that sit
//! physically inside this region. The editor corpus validators live in
//! `editor_validators`; the remaining general corpus validators live in
//! `general_validators`.
//!
//! Extracted verbatim from `main.rs` as a behavior-preserving decomposition
//! slice of #2119. Items referenced outside this module are `pub(crate)` and
//! re-exported from `main.rs` so existing call sites (`dispatch.rs`,
//! `dogfood.rs`, and `tests.rs`) compile unchanged.

use super::*;

mod editor_validators;
mod general_validators;

pub(crate) use editor_validators::*;
pub(crate) use general_validators::*;

pub(crate) fn check_fixture_contracts() -> Result<(), String> {
    let fixtures_dir = Path::new("fixtures");
    if !fixtures_dir.exists() {
        return finish_policy_report(
            PolicyReportSpec {
                report_file: "fixture-contracts.md",
                check: "check-fixture-contracts",
                why_it_matters: "Fixtures are the BDD control bench for analyzer behavior and output contracts.",
                fix_kind: FixKind::AuthorDecisionRequired,
                recommended_fixes: &[
                    "Add BDD fixture directories only with SPEC.md, diff.patch, and expected/check.json.",
                    "Use manifest-only fixture directories only when a dedicated validator owns their corpus contract.",
                    "Use Given/When/Then/Must Not sections for agent-readable fixture intent.",
                ],
                rerun_command: "cargo xtask check-fixture-contracts",
                exception_template: None,
            },
            &[],
        );
    }

    let mut violations = Vec::new();
    validate_evidence_record_contract_fixture_corpus(&mut violations)?;
    validate_lane1_evidence_quality_failure_fixture_corpus(&mut violations)?;
    validate_evidence_quality_benchmark_fixture_corpus(&mut violations)?;
    validate_editor_gap_cockpit_fixture_corpus(&mut violations)?;
    validate_editor_first_run_usability_fixture_corpus(&mut violations)?;
    validate_editor_first_pr_bridge_fixture_corpus(&mut violations)?;
    validate_editor_adoption_assurance_fixture_corpus(&mut violations)?;
    validate_editor_actionable_gap_queue_fixture_corpus(&mut violations)?;
    validate_perl_lsp_facts_exporter_fixture_corpus(&mut violations)?;
    validate_perl_real_repo_eval_fixture_corpus(&mut violations)?;
    validate_python_project_detection_fixture_corpus(&mut violations)?;
    validate_first_successful_pr_fixture_corpus(&mut violations)?;
    validate_finding_alignment_dogfood_fixture_corpus(&mut violations)?;
    validate_gap_decision_ledger_fixture_corpus(&mut violations)?;
    validate_real_repair_attempt_fixture_corpus(&mut violations)?;
    validate_python_real_repo_eval_fixture_corpus(&mut violations)?;
    validate_surface_projection_alignment_fixture_corpus(&mut violations)?;
    validate_typescript_bun_ub_calibration_fixture_corpus(&mut violations)?;
    validate_cross_language_oracle_graph_fixture_corpus(&mut violations)?;
    validate_bun_ub_cross_language_dogfood_fixture_corpus(&mut violations)?;
    validate_typescript_preview_repair_loop_fixture_corpus(&mut violations)?;
    validate_typescript_preview_false_actionable_audit_fixture_corpus(&mut violations)?;
    validate_evidence_promotion_honesty_corpus(&mut violations)?;
    validate_user_surface_projection_alignment_fixture_corpus(&mut violations)?;
    validate_swarm_plan_packet_fixture_corpus(&mut violations)?;
    validate_actionable_gap_outcomes_fixture_corpus(&mut violations)?;
    validate_pr_review_front_panel_fixture_corpus(&mut violations)?;
    validate_report_packet_index_fixture_corpus(&mut violations)?;
    validate_pr_inline_comment_publisher_fixture_corpus(&mut violations)?;
    for entry in
        fs::read_dir(fixtures_dir).map_err(|err| format!("failed to read fixtures: {err}"))?
    {
        let entry = entry.map_err(|err| format!("failed to read fixtures: {err}"))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if is_manifest_only_fixture_dir(&path) {
            continue;
        }
        let normalized = normalize_path(&path);
        let spec = path.join("SPEC.md");
        let diff = path.join("diff.patch");
        let expected_check = path.join("expected/check.json");

        if !spec.exists() {
            violations.push(format!("{normalized} is missing SPEC.md"));
            continue;
        }
        if !diff.exists() {
            violations.push(format!("{normalized} is missing diff.patch"));
        }
        if !expected_check.exists() {
            violations.push(format!("{normalized} is missing expected/check.json"));
        }

        let text = read_text_lossy(&spec)?;
        if !text
            .lines()
            .any(|line| line.starts_with("Spec: RIPR-SPEC-"))
        {
            violations.push(format!(
                "{} is missing `Spec: RIPR-SPEC-NNNN`",
                normalize_path(&spec)
            ));
        }
        for heading in ["## Given", "## When", "## Then", "## Must Not"] {
            if !has_markdown_heading(&text, heading) {
                violations.push(format!("{} is missing `{heading}`", normalize_path(&spec)));
            }
        }
    }

    finish_policy_report(
        PolicyReportSpec {
            report_file: "fixture-contracts.md",
            check: "check-fixture-contracts",
            why_it_matters: "Fixtures are the BDD control bench for analyzer behavior and output contracts.",
            fix_kind: FixKind::AuthorDecisionRequired,
            recommended_fixes: &[
                "Add missing fixture contract files.",
                "Use Given/When/Then/Must Not sections in fixture SPEC.md.",
                "Keep manifest-only fixture corpora covered by their dedicated validators.",
                "Keep expected output files aligned with the fixture behavior.",
            ],
            rerun_command: "cargo xtask check-fixture-contracts",
            exception_template: None,
        },
        &violations,
    )
}

fn validate_python_project_detection_fixture_corpus(
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let root = Path::new("fixtures/python");
    for required in [
        "SPEC.md",
        "basic/pyproject.toml",
        "basic/src/pricing.py",
        "basic/tests/test_pricing.py",
        "basic/diff.patch",
    ] {
        let path = root.join(required);
        if !path.exists() {
            violations.push(format!(
                "python project detection fixture corpus is missing {}",
                normalize_path(&path)
            ));
        }
    }

    let spec = root.join("SPEC.md");
    if spec.exists() {
        let text = read_text_lossy(&spec)?;
        if !text
            .lines()
            .any(|line| line.starts_with("Spec: RIPR-SPEC-"))
        {
            violations.push(format!(
                "{} is missing `Spec: RIPR-SPEC-NNNN`",
                normalize_path(&spec)
            ));
        }
        for heading in ["## Given", "## When", "## Then", "## Must Not"] {
            if !has_markdown_heading(&text, heading) {
                violations.push(format!("{} is missing `{heading}`", normalize_path(&spec)));
            }
        }
    }
    Ok(())
}

pub(crate) fn validate_perl_lsp_facts_exporter_fixture_corpus(
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let root = Path::new("fixtures/perl_lsp_facts_exporter");
    for required in [
        "SPEC.md",
        "corpus.json",
        "input/lib/My/App.pm",
        "input/t/app.t",
        "expected/ripr-perl-facts-v1.json",
    ] {
        let path = root.join(required);
        if !path.exists() {
            violations.push(format!(
                "perl-lsp facts exporter fixture corpus is missing {}",
                normalize_path(&path)
            ));
        }
    }

    let spec = root.join("SPEC.md");
    if spec.exists() {
        let spec_text = read_text_lossy(&spec)?;
        if !spec_text
            .lines()
            .any(|line| line.starts_with("Spec: RIPR-SPEC-0064"))
        {
            violations.push(format!(
                "{} is missing `Spec: RIPR-SPEC-0064`",
                normalize_path(&spec)
            ));
        }
        for heading in ["## Given", "## When", "## Then", "## Must Not"] {
            if !has_markdown_heading(&spec_text, heading) {
                violations.push(format!("{} is missing `{heading}`", normalize_path(&spec)));
            }
        }
    }

    validate_perl_lsp_facts_exporter_fixture_corpus_at(
        Path::new(PERL_LSP_FACTS_EXPORTER_CORPUS),
        violations,
    )
}

pub(crate) fn validate_perl_lsp_facts_exporter_fixture_corpus_at(
    path: &Path,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    if !path.exists() {
        violations.push(format!(
            "perl-lsp facts exporter corpus is missing {}",
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
    if json_string_field(&corpus, "kind").as_deref() != Some("perl_lsp_facts_exporter_corpus") {
        violations.push(format!(
            "{} kind must be perl_lsp_facts_exporter_corpus",
            normalize_path(path)
        ));
    }
    if json_string_field(&corpus, "schema_version").as_deref() != Some("0.1") {
        violations.push(format!(
            "{} schema_version must be 0.1",
            normalize_path(path)
        ));
    }
    if json_string_field(&corpus, "spec").as_deref() != Some("RIPR-SPEC-0064") {
        violations.push(format!(
            "{} spec must be RIPR-SPEC-0064",
            normalize_path(path)
        ));
    }

    let Some(cases) = corpus.get("cases").and_then(Value::as_array) else {
        violations.push(format!("{} is missing cases array", normalize_path(path)));
        return Ok(());
    };
    if cases.is_empty() {
        violations.push(format!(
            "{} cases array must not be empty",
            normalize_path(path)
        ));
    }

    let mut seen = BTreeSet::new();
    for case in cases {
        let case_id = json_string_field(case, "id").unwrap_or_else(|| "unknown".to_string());
        if !seen.insert(case_id.clone()) {
            violations.push(format!(
                "perl-lsp facts exporter case {case_id} is duplicated"
            ));
        }
        validate_perl_lsp_facts_exporter_fixture_case(case, &case_id, violations)?;
    }

    Ok(())
}

fn validate_perl_lsp_facts_exporter_fixture_case(
    case: &Value,
    case_id: &str,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let exporter = json_string_field(case, "exporter");
    if !matches!(
        exporter.as_deref(),
        Some("perl-ripr-facts" | "perllsp" | "perl-lsp")
    ) {
        violations.push(format!(
            "perl-lsp facts exporter case {case_id} exporter must be perl-ripr-facts, perllsp, or perl-lsp"
        ));
    }
    if json_string_field(case, "packet_schema").as_deref() != Some("ripr-perl-facts-v1") {
        violations.push(format!(
            "perl-lsp facts exporter case {case_id} packet_schema must be ripr-perl-facts-v1"
        ));
    }
    if json_string_field(case, "authority_boundary").as_deref() != Some("preview_advisory_only") {
        violations.push(format!(
            "perl-lsp facts exporter case {case_id} authority_boundary must be preview_advisory_only"
        ));
    }

    let must_not_claims = case
        .get("must_not_claim")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    for required in [
        "ripr_check_executes_perl_lsp",
        "canonical_gap_id_emitted_by_perl_lsp",
        "gap_state_emitted_by_perl_lsp",
        "repair_packet_ready",
        "default_gate_authority",
        "public_badge_contribution",
        "support_tier_promotion",
    ] {
        if !must_not_claims.contains(required) {
            violations.push(format!(
                "perl-lsp facts exporter case {case_id} must_not_claim is missing {required}"
            ));
        }
    }

    let Some(packet_path) = json_string_field(case, "expected_packet") else {
        violations.push(format!(
            "perl-lsp facts exporter case {case_id} is missing expected_packet"
        ));
        return Ok(());
    };
    let packet_path = Path::new(&packet_path);
    if !packet_path.exists() {
        violations.push(format!(
            "perl-lsp facts exporter case {case_id} missing packet {}",
            normalize_path(packet_path)
        ));
        return Ok(());
    }

    let packet = match read_json_value(packet_path) {
        Ok(value) => value,
        Err(err) => {
            violations.push(err);
            return Ok(());
        }
    };
    if json_string_field(&packet, "schema_version").as_deref() != Some("ripr-perl-facts-v1") {
        violations.push(format!(
            "perl-lsp facts exporter case {case_id} packet schema_version must be ripr-perl-facts-v1"
        ));
    }
    if !matches!(
        packet
            .get("producer")
            .and_then(|producer| json_string_field(producer, "name"))
            .as_deref(),
        Some("perl-ripr-facts" | "perllsp" | "perl-lsp")
    ) {
        violations.push(format!(
            "perl-lsp facts exporter case {case_id} packet producer.name must be perl-ripr-facts, perllsp, or perl-lsp"
        ));
    }
    if packet.get("canonical_gap_id").is_some() || packet.get("gap_state").is_some() {
        violations.push(format!(
            "perl-lsp facts exporter case {case_id} packet must not emit RIPR-derived gap state"
        ));
    }

    let Some(files) = packet.get("files").and_then(Value::as_array) else {
        violations.push(format!(
            "perl-lsp facts exporter case {case_id} packet is missing files array"
        ));
        return Ok(());
    };
    for file in files {
        let Some(file_path) = json_string_field(file, "path") else {
            violations.push(format!(
                "perl-lsp facts exporter case {case_id} file fact is missing path"
            ));
            continue;
        };
        if file_path.contains('\\') || file_path.contains(':') || file_path.starts_with('/') {
            violations.push(format!(
                "perl-lsp facts exporter case {case_id} file path {file_path} must be repo-relative"
            ));
        }
    }

    Ok(())
}

const PERL_REAL_REPO_EVAL_REQUIRED_CASES: &[(&str, &str)] = &[
    ("cpan_alpha_actionable_real_exporter_eval", "actionable"),
    (
        "cpan_alpha_already_observed_real_exporter_eval",
        "already_observed",
    ),
    ("cpan_alpha_dynamic_dispatch_real_exporter_eval", "limited"),
];

pub(crate) fn validate_perl_real_repo_eval_fixture_corpus(
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let root = Path::new("fixtures/perl-real-repo-evals");
    for required in ["SPEC.md", "corpus.json"] {
        let path = root.join(required);
        if !path.exists() {
            violations.push(format!(
                "Perl real-repo eval fixture corpus is missing {}",
                normalize_path(&path)
            ));
        }
    }

    let spec = root.join("SPEC.md");
    if spec.exists() {
        let spec_text = read_text_lossy(&spec)?;
        if !spec_text
            .lines()
            .any(|line| line.starts_with("Spec: RIPR-SPEC-0064"))
        {
            violations.push(format!(
                "{} is missing `Spec: RIPR-SPEC-0064`",
                normalize_path(&spec)
            ));
        }
        for heading in ["## Given", "## When", "## Then", "## Must Not"] {
            if !has_markdown_heading(&spec_text, heading) {
                violations.push(format!("{} is missing `{heading}`", normalize_path(&spec)));
            }
        }
    }

    validate_perl_real_repo_eval_fixture_corpus_at(
        Path::new(PERL_REAL_REPO_EVAL_CORPUS),
        violations,
    )
}

pub(crate) fn validate_perl_real_repo_eval_fixture_corpus_at(
    path: &Path,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    if !path.exists() {
        violations.push(format!(
            "Perl real-repo eval corpus is missing {}",
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
    if json_string_field(&corpus, "kind").as_deref() != Some("perl_real_repo_eval_corpus") {
        violations.push(format!(
            "{} kind must be perl_real_repo_eval_corpus",
            normalize_path(path)
        ));
    }
    if json_string_field(&corpus, "schema_version").as_deref() != Some("0.1") {
        violations.push(format!(
            "{} schema_version must be 0.1",
            normalize_path(path)
        ));
    }
    if json_string_field(&corpus, "spec").as_deref() != Some("RIPR-SPEC-0064") {
        violations.push(format!(
            "{} spec must be RIPR-SPEC-0064",
            normalize_path(path)
        ));
    }

    let limits = corpus
        .get("limits")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    for required in [
        "producer_required_on_path",
        "no_five_repo_metrics",
        "no_public_repair_packet_authority",
        "no_support_tier_promotion",
    ] {
        if !limits.contains(required) {
            violations.push(format!(
                "{} limits is missing {required}",
                normalize_path(path)
            ));
        }
    }

    let Some(cases) = corpus.get("cases").and_then(Value::as_array) else {
        violations.push(format!("{} is missing cases array", normalize_path(path)));
        return Ok(());
    };
    if cases.is_empty() {
        violations.push(format!(
            "{} cases array must not be empty",
            normalize_path(path)
        ));
    }

    let mut seen = BTreeMap::new();
    for case in cases {
        let case_id = json_string_field(case, "id").unwrap_or_else(|| "unknown".to_string());
        let outcome =
            json_string_field(case, "expected_outcome").unwrap_or_else(|| "unknown".to_string());
        if seen.insert(case_id.clone(), outcome.clone()).is_some() {
            violations.push(format!("Perl real-repo eval case {case_id} is duplicated"));
        }
        validate_perl_real_repo_eval_fixture_case(case, &case_id, violations)?;
    }

    for (case_id, expected_outcome) in PERL_REAL_REPO_EVAL_REQUIRED_CASES {
        match seen.get(*case_id) {
            Some(actual) if actual == expected_outcome => {}
            Some(actual) => violations.push(format!(
                "Perl real-repo eval case {case_id} must have expected_outcome {expected_outcome}, got {actual}"
            )),
            None => violations.push(format!(
                "Perl real-repo eval corpus is missing case {case_id}"
            )),
        }
    }

    Ok(())
}

fn validate_perl_real_repo_eval_fixture_case(
    case: &Value,
    case_id: &str,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    for field in [
        "repo_shape",
        "source_kind",
        "source_ref",
        "command",
        "producer",
        "packet_schema",
        "diff",
        "oracle_shape",
        "expected_outcome",
        "expected_classification",
        "changed_owner",
        "missing_discriminator",
        "harness_assertion",
        "evidence_source",
        "reason",
    ] {
        let value = json_string_field(case, field).unwrap_or_else(|| "unknown".to_string());
        if value.trim().is_empty() || value == "unknown" {
            violations.push(format!(
                "Perl real-repo eval case {case_id} field {field} must be present"
            ));
        }
    }

    if !matches!(
        json_string_field(case, "source_kind").as_deref(),
        Some("local_repo_fixture" | "external_repo" | "scratch_repo")
    ) {
        violations.push(format!(
            "Perl real-repo eval case {case_id} source_kind must be local_repo_fixture, external_repo, or scratch_repo"
        ));
    }
    if !matches!(
        json_string_field(case, "producer").as_deref(),
        Some("perl-ripr-facts" | "perllsp" | "perl-lsp")
    ) {
        violations.push(format!(
            "Perl real-repo eval case {case_id} producer must be perl-ripr-facts, perllsp, or perl-lsp"
        ));
    }
    if json_string_field(case, "packet_schema").as_deref() != Some("ripr-perl-facts-v1") {
        violations.push(format!(
            "Perl real-repo eval case {case_id} packet_schema must be ripr-perl-facts-v1"
        ));
    }
    if !matches!(
        json_string_field(case, "expected_outcome").as_deref(),
        Some("actionable" | "already_observed" | "limited")
    ) {
        violations.push(format!(
            "Perl real-repo eval case {case_id} expected_outcome must be actionable, already_observed, or limited"
        ));
    }
    if !json_string_field(case, "changed_owner").is_some_and(|owner| owner.starts_with("perl:")) {
        violations.push(format!(
            "Perl real-repo eval case {case_id} changed_owner must use perl: identity"
        ));
    }
    if !json_string_field(case, "command").is_some_and(|command| {
        command.contains("cargo test -p ripr --features lang-perl --test perl_two_binary_harness")
    }) {
        violations.push(format!(
            "Perl real-repo eval case {case_id} command must run the perl_two_binary_harness"
        ));
    }
    if !json_string_field(case, "diff").is_some_and(|diff| {
        diff.starts_with("fixtures/perl_cpan_alpha/input/")
            && (diff.ends_with(".diff") || diff.ends_with("diff.patch"))
    }) {
        violations.push(format!(
            "Perl real-repo eval case {case_id} diff must point at a perl_cpan_alpha diff"
        ));
    }

    match json_string_field(case, "expected_outcome").as_deref() {
        Some("actionable")
            if json_string_field(case, "missing_discriminator")
                .is_some_and(|value| value.starts_with("not_applicable")) =>
        {
            violations.push(format!(
                "Perl real-repo eval case {case_id} actionable outcome must name a missing discriminator"
            ));
        }
        Some("already_observed")
            if json_string_field(case, "expected_classification").as_deref() != Some("exposed") =>
        {
            violations.push(format!(
                "Perl real-repo eval case {case_id} already_observed outcome must expect exposed"
            ));
        }
        Some("limited")
            if !json_string_field(case, "expected_classification")
                .is_some_and(|classification| classification.contains("limitation")) =>
        {
            violations.push(format!(
                "Perl real-repo eval case {case_id} limited outcome must name a limitation classification"
            ));
        }
        _ => {}
    }

    for field in [
        "repair_packet_expected",
        "agent_packet_expected",
        "receipt_expected",
    ] {
        match json_bool_field(case, field) {
            Some(false) => {}
            Some(true) => violations.push(format!(
                "Perl real-repo eval case {case_id} {field} must stay false before public Perl projection"
            )),
            None => violations.push(format!(
                "Perl real-repo eval case {case_id} field {field} must be present"
            )),
        }
    }

    let claim_boundary = case
        .get("claim_boundary")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();
    for required in [
        "No >=5 real Perl repo evidence",
        "No public repair-packet authority",
        "No support-tier promotion",
    ] {
        if !claim_boundary.iter().any(|claim| claim.contains(required)) {
            violations.push(format!(
                "Perl real-repo eval case {case_id} claim_boundary must include {required}"
            ));
        }
    }
    if !json_string_field(case, "evidence_source").is_some_and(|source| source.contains("#1491")) {
        violations.push(format!(
            "Perl real-repo eval case {case_id} evidence_source must cite PR #1491"
        ));
    }

    Ok(())
}

pub(crate) const EVIDENCE_RECORD_CONTRACT_CORPUS: &str =
    "fixtures/boundary_gap/expected/evidence-record-contract/corpus.json";

const EVIDENCE_RECORD_REQUIRED_CASES: &[&str] = &[
    "predicate_boundary_missing_equality",
    "exact_error_variant_gap",
    "strong_exact_value_oracle",
    "broad_is_err_oracle",
    "field_output_assertion",
    "whole_object_equality",
    "snapshot_oracle",
    "side_effect_observer",
    "opaque_helper_static_limitation",
    "baseline_known_canonical_gap_identity",
    "calibration_placeholder_no_runtime_data",
];

const LANE1_EVIDENCE_QUALITY_FAILURE_CORPUS: &str =
    "fixtures/boundary_gap/expected/evidence-quality-failures/corpus.json";

const LANE1_EVIDENCE_QUALITY_REQUIRED_CASES: &[&str] = &[
    "duplicate_canonical_gap_overcount_suppressions_match_arm",
    "missing_equality_boundary_discriminator",
    "static_activation_limitation_without_candidate_values",
    "side_effect_observer_not_static_limitation",
    "calibration_no_runtime_data_gap",
];

const EVIDENCE_QUALITY_BENCHMARK_CORPUS: &str = "fixtures/evidence-quality-benchmark/corpus.json";

const EVIDENCE_QUALITY_BENCHMARK_REQUIRED_CLASSES: &[&str] = &[
    "duplicate_canonical_gap",
    "match_arm_discriminator_split",
    "wrong_related_test_top_choice",
    "broad_vs_exact_error_oracle",
    "self_computed_expected_value",
    "opaque_helper_static_limitation",
    "cross_file_constant_limitation",
    "activation_value_resolution",
    "presentation_text",
    "config_or_policy_constant",
    "side_effect_observer",
    "snapshot_discriminator",
    "mock_expectation",
    "call_presence",
    "runtime_only_signal",
    "ambiguous_runtime_join",
];

const EVIDENCE_QUALITY_BENCHMARK_REQUIRED_CASE_KINDS: &[&str] = &[
    "positive",
    "negative_guard",
    "metamorphic_line_movement",
    "equivalent_code",
    "static_limitation",
    "calibration",
];

const EVIDENCE_QUALITY_BENCHMARK_REQUIRED_CONFIG_POLICY_CASES: &[&str] = &[
    "config_policy_internal_metadata_no_action",
    "config_policy_rendered_label_unobserved",
    "config_policy_behavior_selector_unobserved",
    "config_policy_behavior_selector_observed",
    "config_policy_schema_label_observed",
    "config_policy_cross_file_flow_unknown",
    "config_policy_opaque_lookup_report_unobserved",
    "config_policy_opaque_lookup_unknown",
];

const FINDING_ALIGNMENT_DOGFOOD_CORPUS: &str = "fixtures/finding-alignment-dogfood/corpus.json";
pub(crate) const REAL_REPAIR_ATTEMPTS_CORPUS: &str = "fixtures/real-repair-attempts/corpus.json";
const PERL_REAL_REPO_EVAL_CORPUS: &str = "fixtures/perl-real-repo-evals/corpus.json";
pub(crate) const PYTHON_REAL_REPO_EVAL_CORPUS: &str = "fixtures/python-real-repo-evals/corpus.json";
pub(crate) const SURFACE_PROJECTION_ALIGNMENT_CORPUS: &str =
    "fixtures/surface-projection-alignment/corpus.json";
pub(crate) const TYPESCRIPT_BUN_UB_CALIBRATION_CORPUS: &str =
    "fixtures/typescript-bun-ub-calibration/corpus.json";
pub(crate) const CROSS_LANGUAGE_ORACLE_GRAPH_CORPUS: &str =
    "fixtures/cross-language-oracle-graph-corpus/corpus.json";
pub(crate) const BUN_BLOB_ARRAY_BUFFER_TS_TEST_FILE: &str = "test/js/web/fetch/blob.test.ts";
pub(crate) const BUN_MARKDOWN_TS_TEST_FILE: &str = "test/js/bun/md/md-edge-cases.test.ts";
pub(crate) const BUN_NODE_FS_TS_TEST_FILE: &str = "test/js/node/fs/fs.test.ts";
pub(crate) const BUN_WRITE_TS_TEST_FILE: &str = "test/js/bun/write.test.ts";
pub(crate) const BUN_FFI_NEGATIVE_OFFSET_TS_TEST_SURFACE: &str =
    "unresolved:typescript-test-surface";
pub(crate) const BUN_UB_CROSS_LANGUAGE_DOGFOOD_CORPUS: &str =
    "fixtures/bun-ub-cross-language-dogfood/corpus.json";
pub(crate) const TYPESCRIPT_PREVIEW_REPAIR_LOOP_CORPUS: &str =
    "fixtures/typescript-preview-repair-loop/corpus.json";
pub(crate) const TYPESCRIPT_PREVIEW_FALSE_ACTIONABLE_AUDIT_CORPUS: &str =
    "fixtures/typescript-preview-false-actionable-audit/corpus.json";
pub(crate) const USER_SURFACE_PROJECTION_ALIGNMENT_CORPUS: &str =
    "fixtures/user-surface-projection-alignment/corpus.json";

const FINDING_ALIGNMENT_DOGFOOD_REQUIRED_CASES: &[(&str, &str)] = &[
    ("presentation_text_actionable_output_observer", "actionable"),
    (
        "presentation_text_already_observed_output",
        "already_observed",
    ),
    ("config_policy_internal_metadata_no_action", "internal_only"),
    ("config_policy_rendered_label_unobserved", "actionable"),
    ("config_policy_behavior_selector_unobserved", "actionable"),
    (
        "config_policy_behavior_selector_observed",
        "already_observed",
    ),
    ("config_policy_flow_unknown_limitation", "static_limitation"),
];

const SURFACE_PROJECTION_ALIGNMENT_REQUIRED_CASES: &[(&str, &str)] = &[(
    "receipt_improved_top_next_action_alignment",
    "evidence_improved",
)];

pub(crate) const REAL_REPAIR_ATTEMPTS_REQUIRED_CASES: &[(&str, &str)] = &[
    ("repair_route_quality_metrics_improved", "evidence_improved"),
    ("readiness_top_next_action_resolved", "resolved"),
    ("projection_alignment_receipt_improved", "evidence_improved"),
    ("targeted_outcome_left_gap_unchanged", "evidence_unchanged"),
    (
        "exact_error_variant_guidance_route_improved",
        "evidence_improved",
    ),
    (
        "targeted_outcome_exact_error_variant_receipt_improved",
        "evidence_improved",
    ),
    (
        "dry_run_receipt_command_missing_named",
        "attempted_no_receipt",
    ),
    (
        "missing_repair_kind_blocker_route_improved",
        "evidence_improved",
    ),
    (
        "local_computed_boundary_limitation_route_improved",
        "evidence_improved",
    ),
    (
        "local_member_boundary_operand_route_split",
        "evidence_improved",
    ),
    (
        "same_file_owner_call_route_samples_improved",
        "evidence_improved",
    ),
    (
        "same_file_test_local_helper_owner_call_route_improved",
        "evidence_improved",
    ),
    (
        "same_file_method_owner_call_route_improved",
        "evidence_improved",
    ),
    (
        "same_file_method_chain_owner_call_helper_tests_improved",
        "evidence_improved",
    ),
    (
        "activation_body_line_helper_owner_call_tests_improved",
        "evidence_improved",
    ),
    (
        "call_presence_imported_module_wrapper_route_improved",
        "evidence_improved",
    ),
    (
        "typescript_preview_lsp_repair_context_improved",
        "evidence_improved",
    ),
    (
        "javascript_preview_generated_ci_grouping_improved",
        "evidence_improved",
    ),
    (
        "typescript_preview_mocked_module_limitation_improved",
        "evidence_improved",
    ),
    (
        "typescript_module_initializer_observer_route_improved",
        "evidence_improved",
    ),
    (
        "typescript_method_receiver_observer_route_improved",
        "evidence_improved",
    ),
    (
        "typescript_preview_weak_oracle_downgrade_unchanged",
        "evidence_unchanged",
    ),
    (
        "typescript_preview_weak_oracle_guidance_improved",
        "evidence_improved",
    ),
    (
        "python_cli_output_repair_card_route_improved",
        "evidence_improved",
    ),
    (
        "python_argparse_output_repair_card_route_improved",
        "evidence_improved",
    ),
    (
        "python_api_json_field_repair_card_route_improved",
        "evidence_improved",
    ),
    (
        "python_parametrized_boundary_repair_card_route_improved",
        "evidence_improved",
    ),
    ("python_preview_boundary_gap_test_only_closed", "resolved"),
];

pub(crate) const PYTHON_REAL_REPO_EVAL_REQUIRED_CASES: &[(&str, &str)] = &[
    ("tiny_controlled_pytest_boundary_receipt", "closed"),
    ("no_config_pyproject_boundary_receipt", "closed"),
    ("normal_pytest_app_boundary_receipt", "closed"),
    ("src_layout_pytest_boundary_receipt", "closed"),
    ("multi_card_pytest_top3_receipt", "closed"),
    ("async_return_pytest_receipt", "closed"),
    ("parametrized_boundary_pytest_receipt", "closed"),
    ("cli_output_pytest_receipt", "closed"),
    ("log_output_pytest_receipt", "closed"),
    ("argparse_cli_output_pytest_receipt", "closed"),
    ("click_cli_output_pytest_receipt", "closed"),
    ("typer_cli_output_pytest_receipt", "closed"),
    ("cli_exit_code_pytest_receipt", "closed"),
    ("exception_path_pytest_receipt", "closed"),
    ("custom_exception_pytest_receipt", "closed"),
    ("unittest_exception_path_receipt", "closed"),
    ("api_status_pytest_receipt", "closed"),
    ("api_json_detail_pytest_receipt", "closed"),
    ("flask_route_json_detail_pytest_receipt", "closed"),
    ("fastapi_route_json_detail_pytest_receipt", "closed"),
    ("api_exception_response_pytest_receipt", "closed"),
    ("mixed_rust_python_pytest_receipt", "closed"),
    ("decorated_route_status_pytest_receipt", "closed"),
    ("unittest_return_value_receipt", "closed"),
    ("unittest_dict_field_receipt", "closed"),
    ("model_field_pytest_receipt", "closed"),
];

pub(crate) const PYTHON_REAL_REPO_EVAL_REQUIRED_STATIC_LIMIT_CASES: &[(&str, &str)] = &[
    ("dynamic_dispatch_no_packet_eval", "dynamic_dispatch"),
    (
        "decorator_indirection_no_packet_eval",
        "decorator_indirection",
    ),
    (
        "missing_import_graph_no_packet_eval",
        "missing_import_graph",
    ),
    ("metaprogramming_no_packet_eval", "metaprogramming"),
    ("mocked_module_no_packet_eval", "mocked_module"),
    (
        "opaque_custom_helper_no_packet_eval",
        "opaque_custom_assertion_helper",
    ),
    ("property_based_no_packet_eval", "property_based_test"),
    (
        "unresolved_fixture_no_packet_eval",
        "unresolved_pytest_fixture",
    ),
    ("generated_file_no_packet_eval", "generated_file"),
    ("unsupported_syntax_no_packet_eval", "unsupported_syntax"),
];

pub(crate) const PYTHON_REAL_REPO_EVAL_REQUIRED_NO_ACTION_CASES: &[(&str, &str)] = &[
    ("no_related_test_no_packet_eval", "no_related_test"),
    ("already_observed_no_packet_eval", "already_observed"),
    ("heuristic_only_no_packet_eval", "heuristic_only"),
];

pub(crate) const TYPESCRIPT_PREVIEW_REPAIR_LOOP_REQUIRED_CASES: &[(&str, &str)] = &[
    ("typescript_boundary_predicate_proof", "proof_improved"),
    (
        "typescript_smoke_return_weak_oracle",
        "weak_oracle_downgraded",
    ),
    (
        "typescript_snapshot_return_weak_oracle",
        "weak_oracle_downgraded",
    ),
    (
        "typescript_async_rejection_broad_error",
        "weak_oracle_downgraded",
    ),
    (
        "javascript_mock_interaction_preview",
        "intentionally_skipped",
    ),
    (
        "typescript_mocked_module_static_limit",
        "static_limitation_recorded",
    ),
    (
        "javascript_already_observed_unchanged",
        "already_observed_unchanged",
    ),
    ("typescript_complete_boundary_packet_closed", "resolved"),
];

pub(crate) const TYPESCRIPT_BUN_UB_CALIBRATION_REQUIRED_CASES: &[(&str, &str)] = &[
    ("bun_blob_shared_and_resizable_present", "ts_discriminated"),
    ("bun_blob_resizable_missing", "ts_missing_resizable"),
    ("bun_blob_shared_missing", "ts_missing_shared"),
    (
        "bun_blob_neither_present",
        "ts_missing_shared_and_resizable",
    ),
    (
        "bun_blob_partial_observer_missing_external_oracle",
        "ts_missing_external_oracle",
    ),
    (
        "bun_blob_max_byte_length_mention_not_observer",
        "ts_mention_not_observer",
    ),
    ("bun_blob_bridge_unknown_without_hint", "bridge_unknown"),
];

pub(crate) const CROSS_LANGUAGE_ORACLE_GRAPH_REQUIRED_CASES: &[(&str, &str)] = &[
    (
        "bun_blob_complete_ts_discriminated_advisory",
        "rust_ungripped_ts_discriminated",
    ),
    (
        "bun_blob_missing_resizable_oracle_limitation",
        "rust_ungripped_ts_missing_discriminator",
    ),
    (
        "bun_blob_missing_external_oracle_limitation",
        "rust_ungripped_ts_missing_external_oracle",
    ),
    (
        "bun_blob_mention_not_observer_limitation",
        "ts_mention_not_observer",
    ),
    ("bun_blob_bridge_unknown_limitation", "bridge_unknown"),
    (
        "bun_blob_target_unresolved_limitation",
        "cross_language_target_unresolved",
    ),
    (
        "bun_array_buffer_copy_to_unshared_configured_bridge_advisory",
        "rust_ungripped_ts_discriminated",
    ),
    (
        "bun_markdown_resizable_array_buffer_configured_bridge_advisory",
        "rust_ungripped_ts_discriminated",
    ),
    (
        "bun_ffi_negative_offset_panic_boundary_limitation",
        "public_reachable_panic_boundary_unrevealed",
    ),
    (
        "bun_node_fs_scalar_write_manifest_only_profile",
        "named_static_limitation",
    ),
    (
        "bun_write_helper_gated_manifest_only_profile",
        "named_static_limitation",
    ),
];

pub(crate) const BUN_UB_CROSS_LANGUAGE_DOGFOOD_REQUIRED_CASES: &[(&str, &str)] = &[
    (
        "bun_blob_31648_known_good",
        "rust_ungripped_ts_discriminated",
    ),
    (
        "bun_array_buffer_copy_to_unshared_live_receipt",
        "rust_ungripped_ts_discriminated",
    ),
    (
        "bun_markdown_resizable_array_buffer_live_receipt",
        "rust_ungripped_ts_discriminated",
    ),
    (
        "bun_blob_stripped_resizable",
        "rust_ungripped_ts_missing_discriminator",
    ),
    ("bun_blob_mention_only", "ts_mention_not_observer"),
    ("bun_blob_bridge_unknown_live_receipt", "bridge_unknown"),
    (
        "bun_node_fs_scalar_write_manifest_only_receipt",
        "named_static_limitation",
    ),
    (
        "bun_write_helper_gated_manifest_only_receipt",
        "named_static_limitation",
    ),
    (
        "bun_ffi_negative_offset_panic_boundary",
        "public_reachable_panic_boundary_unrevealed",
    ),
];

pub(crate) const TYPESCRIPT_PREVIEW_FALSE_ACTIONABLE_AUDIT_REQUIRED_CASES: &[(&str, &str)] = &[
    (
        "mock_interaction_without_payload_proof",
        "candidate_future_support",
    ),
    (
        "broad_throw_or_rejects_without_payload",
        "candidate_future_support",
    ),
    ("snapshot_only_weak_oracle", "safe_advisory"),
    ("smoke_only_truthiness", "safe_advisory"),
    ("heuristic_related_test_link", "must_remain_non_actionable"),
    (
        "owner_name_in_test_title_only",
        "must_remain_non_actionable",
    ),
    ("method_receiver_ambiguity", "candidate_future_support"),
    (
        "class_method_static_call_incomplete_packet",
        "candidate_future_support",
    ),
    ("module_initializer_ambiguity", "candidate_future_support"),
    (
        "oracle_helper_gated_named_limitation",
        "candidate_future_support",
    ),
    ("table_case_named_limitation", "candidate_future_support"),
    ("mocked_module_limit", "named_static_limitation"),
    ("decorator_indirection_limit", "named_static_limitation"),
    ("dynamic_dispatch_limit", "named_static_limitation"),
];

pub(crate) const USER_SURFACE_PROJECTION_REQUIRED_SURFACES: &[&str] =
    &["badge", "lsp", "pr_comment", "ci"];
pub(crate) const USER_SURFACE_PROJECTION_REQUIRED_RUN_STATUSES: &[&str] = &[
    "full",
    "limited_large_cache_skip",
    "limited_incomplete_input",
    "limited_sampled_input",
    "limited_stale_input",
];

const PERL_LSP_FACTS_EXPORTER_CORPUS: &str = "fixtures/perl_lsp_facts_exporter/corpus.json";
