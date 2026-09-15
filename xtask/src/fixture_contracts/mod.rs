//! Fixture-contract cluster: the `check-fixture-contracts` orchestrator, its
//! corpus const tables, the shared corpus-record helpers, and the python/perl
//! corpus validators that sit physically inside this region. The editor
//! corpus validators live in `editor_validators`; the remaining general
//! corpus validators live in `general_validators`; the swarm-plan /
//! actionable-gap-outcomes / first-successful-pr / gap-decision-ledger corpus
//! validators live in `gap_validators`; the assistant-loop-health /
//! pr-review-front-panel / report-packet-index / pr-inline-comment-publisher
//! corpus validators live in `report_validators`.
//!
//! Extracted verbatim from `main.rs` as a behavior-preserving decomposition
//! slice of #2119. Items referenced outside this module are `pub(crate)` and
//! re-exported from `main.rs` so existing call sites (`dispatch.rs`,
//! `dogfood.rs`, and `tests.rs`) compile unchanged.

use super::*;
use sha2::{Digest, Sha256};

mod editor_validators;
mod gap_validators;
mod general_validators;
mod report_validators;

pub(crate) use editor_validators::*;
pub(crate) use gap_validators::*;
pub(crate) use general_validators::*;
pub(crate) use report_validators::*;

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
    validate_perl_packet_contract_migration_corpus(&mut violations)?;
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
    validate_assistant_loop_health_fixture_corpus(&mut violations)?;
    validate_release_control_fixture_corpus(&mut violations)?;
    validate_release_scope_fixture_corpus(&mut violations)?;
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

fn validate_release_control_fixture_corpus(violations: &mut Vec<String>) -> Result<(), String> {
    let root = Path::new("fixtures/release_control");
    for required in ["SPEC.md", "complete.json", "reconcile-required.json"] {
        let path = root.join(required);
        if !path.exists() {
            violations.push(format!(
                "release-control fixture corpus is missing {}",
                normalize_path(&path)
            ));
        }
    }
    let spec = root.join("SPEC.md");
    if spec.exists() {
        let text = read_text_lossy(&spec)?;
        if !text
            .lines()
            .any(|line| line.starts_with("Spec: RIPR-SPEC-0144"))
        {
            violations.push(format!(
                "{} is missing `Spec: RIPR-SPEC-0144`",
                normalize_path(&spec)
            ));
        }
        for heading in ["## Given", "## When", "## Then", "## Must Not"] {
            if !has_markdown_heading(&text, heading) {
                violations.push(format!("{} is missing `{heading}`", normalize_path(&spec)));
            }
        }
    }
    for name in ["complete.json", "reconcile-required.json"] {
        let path = root.join(name);
        if !path.exists() {
            continue;
        }
        let value = match read_json_value(&path) {
            Ok(value) => value,
            Err(err) => {
                violations.push(err);
                continue;
            }
        };
        if json_string_field(&value, "schema_version").as_deref() != Some("0.1") {
            violations.push(format!(
                "{} schema_version must be 0.1",
                normalize_path(&path)
            ));
        }
        if json_string_field(&value, "kind").as_deref() != Some("release_control_snapshot") {
            violations.push(format!(
                "{} kind must be release_control_snapshot",
                normalize_path(&path)
            ));
        }
        match value.get("source").and_then(Value::as_object) {
            Some(source) => {
                if source
                    .get("worktree_inventory_complete")
                    .and_then(Value::as_bool)
                    .is_none()
                {
                    violations.push(format!(
                        "{} source is missing boolean worktree_inventory_complete",
                        normalize_path(&path)
                    ));
                }
                if source
                    .get("worktree_count")
                    .and_then(Value::as_u64)
                    .is_none()
                {
                    violations.push(format!(
                        "{} source is missing numeric worktree_count",
                        normalize_path(&path)
                    ));
                }
            }
            None => violations.push(format!(
                "{} is missing source object",
                normalize_path(&path)
            )),
        }
        if value.get("prs").and_then(Value::as_array).is_none() {
            violations.push(format!("{} is missing prs array", normalize_path(&path)));
        }
    }
    Ok(())
}

fn validate_release_scope_fixture_corpus(violations: &mut Vec<String>) -> Result<(), String> {
    let root = Path::new("fixtures/release_scope");
    for required in ["SPEC.md", "accepted-outcome-a.json"] {
        let path = root.join(required);
        if !path.exists() {
            violations.push(format!(
                "release-scope fixture corpus is missing {}",
                normalize_path(&path)
            ));
        }
    }
    let spec = root.join("SPEC.md");
    if spec.exists() {
        let text = read_text_lossy(&spec)?;
        if !text
            .lines()
            .any(|line| line.starts_with("Spec: RIPR-SPEC-0145"))
        {
            violations.push(format!(
                "{} is missing `Spec: RIPR-SPEC-0145`",
                normalize_path(&spec)
            ));
        }
        for heading in ["## Given", "## When", "## Then", "## Must Not"] {
            if !has_markdown_heading(&text, heading) {
                violations.push(format!("{} is missing `{heading}`", normalize_path(&spec)));
            }
        }
    }
    let input = root.join("accepted-outcome-a.json");
    if input.exists() {
        let value = match read_json_value(&input) {
            Ok(value) => value,
            Err(err) => {
                violations.push(err);
                return Ok(());
            }
        };
        for (field, expected) in [
            ("schema_version", "0.1"),
            ("kind", "release_execution_scope"),
            ("outcome", "preserve_accepted_0_11"),
        ] {
            if json_string_field(&value, field).as_deref() != Some(expected) {
                violations.push(format!(
                    "{} {field} must be {expected}",
                    normalize_path(&input)
                ));
            }
        }
        for field in [
            "candidate_parent_sha",
            "execution_commit",
            "release_non_claim",
            "issue_2332_state",
        ] {
            if json_string_field(&value, field).is_none() {
                violations.push(format!(
                    "{} is missing string {field}",
                    normalize_path(&input)
                ));
            }
        }
        for field in [
            "execution_only_paths",
            "candidate_excluded_paths",
            "preserved_paths",
        ] {
            if value.get(field).and_then(Value::as_array).is_none() {
                violations.push(format!(
                    "{} is missing array {field}",
                    normalize_path(&input)
                ));
            }
        }
        if value
            .get("candidate_tree")
            .and_then(Value::as_object)
            .is_none()
        {
            violations.push(format!(
                "{} is missing candidate_tree object",
                normalize_path(&input)
            ));
        }
    }
    Ok(())
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

/// Migration-oracle corpus for the Perl v2 consumer train (#3217): packets
/// generated by the real `perl-ripr-facts` producer, pinned byte-exact with
/// their inputs and identities, plus the recorded contradictions between real
/// producer output and current RIPR consumer expectations. Manifest-only: a
/// dedicated validator owns the contract (see
/// `validate_perl_packet_contract_migration_corpus`).
const PERL_PACKET_CONTRACT_MIGRATION_CORPUS: &str =
    "fixtures/perl_packet_contract_migration/corpus.json";

const PERL_PACKET_CONTRACT_IMPACTS: &[&str] = &[
    "decode",
    "validation",
    "identity",
    "classification",
    "output",
];

const PERL_PACKET_CONTRACT_CLASSES: &[&str] = &[
    "implementation_defect",
    "contract_change",
    "fixture_correction",
    "unsupported_behavior",
];

const PERL_PACKET_CONTRACT_DISPOSITIONS: &[&str] =
    &["preserve", "rename", "split", "remove", "make_explicit"];

/// Hex-encode a SHA-256 digest as the corpus `sha256:<hex>` digest form.
fn perl_migration_corpus_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    format!(
        "sha256:{}",
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

/// Collect every regular file under a producer-inputs directory as sorted,
/// forward-slash relative paths (the corpus digest-key convention).
fn perl_migration_corpus_input_inventory(inputs_dir: &Path) -> Result<Vec<String>, String> {
    let mut inventory = Vec::new();
    let mut stack = vec![inputs_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).map_err(|err| {
            format!(
                "failed to read producer inputs {}: {err}",
                normalize_path(&dir)
            )
        })? {
            let entry = entry.map_err(|err| err.to_string())?;
            let path = entry.path();
            let metadata = entry.metadata().map_err(|err| err.to_string())?;
            if metadata.is_dir() {
                stack.push(path);
            } else if metadata.is_file() {
                let relative = path
                    .strip_prefix(inputs_dir)
                    .map_err(|err| err.to_string())?
                    .to_string_lossy()
                    .replace('\\', "/");
                inventory.push(relative);
            }
        }
    }
    inventory.sort();
    Ok(inventory)
}

/// Validate that a corpus-referenced path stays inside the repository and is
/// repo-relative (no absolute, drive, or traversal segments).
fn validate_perl_migration_corpus_path(path: &str) -> Result<(), String> {
    if path.is_empty() {
        return Err("path must be non-empty".to_string());
    }
    if path.starts_with('/') || path.contains('\\') {
        return Err(format!("path `{path}` must be repo-relative"));
    }
    let drive_prefix = path.len() >= 2 && path.as_bytes()[1] == b':';
    if drive_prefix {
        return Err(format!("path `{path}` must not carry a drive prefix"));
    }
    if path.split('/').any(|segment| segment == "..") {
        return Err(format!(
            "path `{path}` must not traverse outside the repository"
        ));
    }
    Ok(())
}

pub(crate) fn validate_perl_packet_contract_migration_corpus(
    violations: &mut Vec<String>,
) -> Result<(), String> {
    let root = Path::new("fixtures/perl_packet_contract_migration");
    for required in [
        "SPEC.md",
        "corpus.json",
        "expected/contradictions.v1.json",
        "expected/consumer-dispositions.v1.json",
    ] {
        let path = root.join(required);
        if !path.exists() {
            violations.push(format!(
                "perl packet contract migration corpus is missing {}",
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

    validate_perl_packet_contract_migration_corpus_at(
        Path::new(PERL_PACKET_CONTRACT_MIGRATION_CORPUS),
        violations,
    )
}

/// Corpus validator for the real-producer Perl migration oracle. Paths inside
/// the corpus are repo-root relative (resolved from the process cwd), matching
/// the other Perl corpora. The checks are deliberately byte-level: the packet
/// and every producer input must hash to the pinned digest, so any packet or
/// input edit without a reviewed corpus update fails, and every required
/// contradiction row must exist, so silently deleting a recorded mismatch
/// fails.
pub(crate) fn validate_perl_packet_contract_migration_corpus_at(
    corpus_path: &Path,
    violations: &mut Vec<String>,
) -> Result<(), String> {
    if !corpus_path.exists() {
        violations.push(format!(
            "perl packet contract migration corpus is missing {}",
            normalize_path(corpus_path)
        ));
        return Ok(());
    }
    let corpus = match read_json_value(corpus_path) {
        Ok(value) => value,
        Err(err) => {
            violations.push(err);
            return Ok(());
        }
    };
    if json_string_field(&corpus, "kind").as_deref()
        != Some("perl_packet_contract_migration_corpus")
    {
        violations.push(format!(
            "{} kind must be perl_packet_contract_migration_corpus",
            normalize_path(corpus_path)
        ));
    }
    if json_string_field(&corpus, "schema_version").as_deref() != Some("0.1") {
        violations.push(format!(
            "{} schema_version must be 0.1",
            normalize_path(corpus_path)
        ));
    }
    if json_string_field(&corpus, "spec").as_deref() != Some("RIPR-SPEC-0064") {
        violations.push(format!(
            "{} spec must be RIPR-SPEC-0064",
            normalize_path(corpus_path)
        ));
    }
    if json_string_field(&corpus, "authority_boundary").as_deref() != Some("preview_advisory_only")
    {
        violations.push(format!(
            "{} authority_boundary must be preview_advisory_only",
            normalize_path(corpus_path)
        ));
    }

    let required_ids: Vec<String> = corpus
        .get("required_contradiction_ids")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    if required_ids.is_empty() {
        violations.push(format!(
            "{} required_contradiction_ids must be a non-empty array",
            normalize_path(corpus_path)
        ));
    }

    let Some(cases) = corpus.get("cases").and_then(Value::as_array) else {
        violations.push(format!(
            "{} is missing cases array",
            normalize_path(corpus_path)
        ));
        return Ok(());
    };
    if cases.is_empty() {
        violations.push(format!(
            "{} cases array must not be empty",
            normalize_path(corpus_path)
        ));
    }

    let mut seen = BTreeSet::new();
    for case in cases {
        let case_id = json_string_field(case, "id").unwrap_or_else(|| "unknown".to_string());
        if !seen.insert(case_id.clone()) {
            violations.push(format!(
                "perl packet contract migration case {case_id} is duplicated"
            ));
        }
        validate_perl_packet_contract_migration_case(case, &case_id, &required_ids, violations)?;
    }

    Ok(())
}

fn validate_perl_packet_contract_migration_case(
    case: &Value,
    case_id: &str,
    required_ids: &[String],
    violations: &mut Vec<String>,
) -> Result<(), String> {
    // Producer identity: a real producer at a pinned commit, never a
    // hand-authored packet.
    let Some(producer) = case.get("producer") else {
        violations.push(format!(
            "perl packet contract migration case {case_id} is missing producer identity"
        ));
        return Ok(());
    };
    if json_string_field(producer, "repository").as_deref()
        != Some("EffortlessMetrics/perl-lsp-swarm")
    {
        violations.push(format!(
            "perl packet contract migration case {case_id} producer.repository must be EffortlessMetrics/perl-lsp-swarm"
        ));
    }
    match json_string_field(producer, "commit") {
        Some(commit) if commit.len() == 40 && commit.chars().all(|c| c.is_ascii_hexdigit()) => {}
        _ => violations.push(format!(
            "perl packet contract migration case {case_id} producer.commit must be a pinned 40-hex commit"
        )),
    }
    if json_string_field(producer, "version")
        .unwrap_or_default()
        .is_empty()
    {
        violations.push(format!(
            "perl packet contract migration case {case_id} producer.version must be non-empty"
        ));
    }
    if json_string_field(producer, "binary").as_deref() != Some("perl-ripr-facts") {
        violations.push(format!(
            "perl packet contract migration case {case_id} producer.binary must be perl-ripr-facts"
        ));
    }
    let command = json_string_field(case, "producer_command").unwrap_or_default();
    if !command.contains("ripr-facts --schema ripr-perl-facts-v1") {
        violations.push(format!(
            "perl packet contract migration case {case_id} producer_command must contain `ripr-facts --schema ripr-perl-facts-v1`"
        ));
    }

    // Producer inputs: the committed bytes must hash to the pinned digests.
    let inputs_dir = json_string_field(case, "producer_inputs_dir").unwrap_or_default();
    if let Err(reason) = validate_perl_migration_corpus_path(&inputs_dir) {
        violations.push(format!(
            "perl packet contract migration case {case_id} producer_inputs_dir invalid: {reason}"
        ));
    } else if !Path::new(&inputs_dir).is_dir() {
        violations.push(format!(
            "perl packet contract migration case {case_id} producer_inputs_dir {} is missing",
            normalize_path(Path::new(&inputs_dir))
        ));
    }
    let Some(digests) = case
        .get("producer_input_digests")
        .and_then(Value::as_object)
    else {
        violations.push(format!(
            "perl packet contract migration case {case_id} is missing producer_input_digests"
        ));
        return Ok(());
    };
    if digests.is_empty() {
        violations.push(format!(
            "perl packet contract migration case {case_id} producer_input_digests must not be empty"
        ));
    }
    let mut pinned_paths: BTreeSet<String> = BTreeSet::new();
    for (relative, expected) in digests {
        pinned_paths.insert(relative.clone());
        let expected = expected.as_str().unwrap_or_default();
        if let Err(reason) = validate_perl_migration_corpus_path(relative) {
            violations.push(format!(
                "perl packet contract migration case {case_id} input `{relative}` invalid: {reason}"
            ));
            continue;
        }
        let path = Path::new(&inputs_dir).join(relative);
        match std::fs::read(&path) {
            Ok(bytes) => {
                let actual = perl_migration_corpus_sha256(&bytes);
                if actual != expected {
                    violations.push(format!(
                        "perl packet contract migration case {case_id} input `{relative}` digest drift: pinned `{expected}` but committed bytes hash to `{actual}`; re-run the pinned producer and update the corpus in a reviewed disposition change"
                    ));
                }
            }
            Err(err) => violations.push(format!(
                "perl packet contract migration case {case_id} input `{relative}` is unreadable: {err}"
            )),
        }
    }
    // Every regular file under the input dir must carry a pinned digest: an
    // unlisted producer-visible file would leave input bytes unpinned while
    // the byte checks above still pass.
    if Path::new(&inputs_dir).is_dir() {
        for relative in perl_migration_corpus_input_inventory(Path::new(&inputs_dir))? {
            if !pinned_paths.contains(&relative) {
                violations.push(format!(
                    "perl packet contract migration case {case_id} input inventory drift: `{relative}` exists under {inputs_dir} but has no pinned digest; add it to producer_input_digests with a producer run in a reviewed disposition change"
                ));
            }
        }
    }

    // The bound packet: byte-exact against its pinned digest.
    let packet_path = json_string_field(case, "packet").unwrap_or_default();
    let packet_sha = json_string_field(case, "packet_sha256").unwrap_or_default();
    if let Err(reason) = validate_perl_migration_corpus_path(&packet_path) {
        violations.push(format!(
            "perl packet contract migration case {case_id} packet invalid: {reason}"
        ));
    } else {
        match std::fs::read(&packet_path) {
            Ok(bytes) => {
                let actual = perl_migration_corpus_sha256(&bytes);
                if actual != packet_sha {
                    violations.push(format!(
                        "perl packet contract migration case {case_id} packet digest drift: pinned `{packet_sha}` but committed bytes hash to `{actual}`; re-run the pinned producer and update the corpus in a reviewed disposition change"
                    ));
                }
            }
            Err(err) => violations.push(format!(
                "perl packet contract migration case {case_id} packet is unreadable: {err}"
            )),
        }
    }

    // Contradiction record: typed rows, required ids present, bound to the
    // same packet digest. An absent pointer, unreadable file, or missing
    // array degrades to zero recorded rows so the required-id checks below
    // still fire — the path never skips validation.
    let contradictions_path = json_string_field(case, "contradictions").unwrap_or_default();
    if contradictions_path.is_empty() {
        violations.push(format!(
            "perl packet contract migration case {case_id} is missing contradictions"
        ));
    }
    let contradictions = if contradictions_path.is_empty() {
        None
    } else {
        match read_json_value(Path::new(&contradictions_path)) {
            Ok(value) => Some(value),
            Err(err) => {
                violations.push(err);
                None
            }
        }
    };
    let mut recorded_ids = BTreeSet::new();
    if let Some(record) = &contradictions {
        if json_string_field(record, "kind").as_deref()
            != Some("perl_packet_contract_contradictions")
        {
            violations.push(format!(
                "perl packet contract migration case {case_id} contradictions kind must be perl_packet_contract_contradictions"
            ));
        }
        if json_string_field(record, "case_id").as_deref() != Some(case_id) {
            violations.push(format!(
                "perl packet contract migration case {case_id} contradictions case_id must match the corpus case"
            ));
        }
        if json_string_field(record, "packet_sha256").as_deref() != Some(packet_sha.as_str()) {
            violations.push(format!(
                "perl packet contract migration case {case_id} contradictions must pin the same packet_sha256 as the corpus case"
            ));
        }
        let empty_rows: Vec<Value> = Vec::new();
        let rows: &[Value] = match record.get("contradictions").and_then(Value::as_array) {
            Some(rows) => rows,
            None => {
                violations.push(format!(
                    "perl packet contract migration case {case_id} contradictions is missing the contradictions array"
                ));
                &empty_rows
            }
        };
        if rows.is_empty() {
            violations.push(format!(
                "perl packet contract migration case {case_id} contradictions array must not be empty"
            ));
        }
        for row in rows {
            let row_id = json_string_field(row, "id").unwrap_or_default();
            if row_id.is_empty() {
                violations.push(format!(
                    "perl packet contract migration case {case_id} has a contradiction row without id"
                ));
                continue;
            }
            if !recorded_ids.insert(row_id.clone()) {
                violations.push(format!(
                    "perl packet contract migration case {case_id} contradiction row {row_id} is duplicated"
                ));
            }
            match json_string_field(row, "json_pointer") {
                Some(pointer) if pointer.starts_with('/') => {}
                _ => violations.push(format!(
                    "perl packet contract migration case {case_id} contradiction row {row_id} json_pointer must start with `/`"
                )),
            }
            for field in ["producer_shape", "consumer_behavior"] {
                if json_string_field(row, field).unwrap_or_default().is_empty() {
                    violations.push(format!(
                        "perl packet contract migration case {case_id} contradiction row {row_id} is missing {field}"
                    ));
                }
            }
            for (field, allowed) in [
                ("impact", PERL_PACKET_CONTRACT_IMPACTS),
                ("class", PERL_PACKET_CONTRACT_CLASSES),
                ("v2_disposition", PERL_PACKET_CONTRACT_DISPOSITIONS),
            ] {
                let value = json_string_field(row, field).unwrap_or_default();
                if !allowed.contains(&value.as_str()) {
                    violations.push(format!(
                        "perl packet contract migration case {case_id} contradiction row {row_id} {field} `{value}` must be one of {allowed:?}"
                    ));
                }
            }
            let owner = json_string_field(row, "owner_issue").unwrap_or_default();
            if !owner.starts_with('#') || owner.len() <= 1 {
                violations.push(format!(
                    "perl packet contract migration case {case_id} contradiction row {row_id} owner_issue must name an owning issue"
                ));
            }
        }
        for required in required_ids {
            if !recorded_ids.contains(required) {
                violations.push(format!(
                    "perl packet contract migration case {case_id} is missing required contradiction row `{required}`; deleting a recorded mismatch requires a reviewed disposition update"
                ));
            }
        }
    }

    // Consumer dispositions: the pinned pipeline result must keep the
    // advisory-only authority boundary and fail-closed actionability pins.
    let dispositions_path = json_string_field(case, "consumer_dispositions").unwrap_or_default();
    if dispositions_path.is_empty() {
        violations.push(format!(
            "perl packet contract migration case {case_id} is missing consumer_dispositions"
        ));
        return Ok(());
    }
    let dispositions = match read_json_value(Path::new(&dispositions_path)) {
        Ok(value) => value,
        Err(err) => {
            violations.push(err);
            return Ok(());
        }
    };
    if json_string_field(&dispositions, "kind").as_deref()
        != Some("perl_packet_contract_consumer_dispositions")
    {
        violations.push(format!(
            "perl packet contract migration case {case_id} dispositions kind must be perl_packet_contract_consumer_dispositions"
        ));
    }
    if json_string_field(&dispositions, "case_id").as_deref() != Some(case_id) {
        violations.push(format!(
            "perl packet contract migration case {case_id} dispositions case_id must match the corpus case"
        ));
    }
    if json_string_field(&dispositions, "packet_sha256").as_deref() != Some(packet_sha.as_str()) {
        violations.push(format!(
            "perl packet contract migration case {case_id} dispositions must pin the same packet_sha256 as the corpus case"
        ));
    }
    if json_string_field(&dispositions, "authority_boundary").as_deref()
        != Some("preview_advisory_only")
    {
        violations.push(format!(
            "perl packet contract migration case {case_id} dispositions authority_boundary must stay preview_advisory_only"
        ));
    }
    for field in [
        "decode_result",
        "structural_validation_result",
        "packet_status_observed",
    ] {
        if json_string_field(&dispositions, field)
            .unwrap_or_default()
            .is_empty()
        {
            violations.push(format!(
                "perl packet contract migration case {case_id} dispositions is missing {field}"
            ));
        }
    }
    let pipeline = dispositions.get("pipeline");
    for field in [
        "canonical_gap_emitted",
        "repair_packet_ready",
        "agent_packet_ready",
    ] {
        if pipeline.and_then(|p| json_bool_field(p, field)) != Some(false) {
            violations.push(format!(
                "perl packet contract migration case {case_id} dispositions pipeline.{field} must stay false (preview advisory-only authority)"
            ));
        }
    }
    let must_not = dispositions
        .get("must_not_claim")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !must_not.iter().any(|claim| claim == "v1_compatibility") {
        violations.push(format!(
            "perl packet contract migration case {case_id} dispositions must_not_claim must include v1_compatibility"
        ));
    }

    Ok(())
}

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

fn validate_lane1_evidence_quality_record(
    case_id: &str,
    record: &Value,
    violations: &mut Vec<String>,
) {
    require_lane1_json_string_at(record, "schema_version", case_id, violations);
    if json_string_field(record, "schema_version").as_deref() != Some("0.1") {
        violations.push(format!(
            "Lane 1 evidence-quality case {case_id} evidence_record.schema_version must be 0.1"
        ));
    }
    for field in ["seam_id", "owner", "seam_kind", "grip_class"] {
        require_lane1_json_string_at(record, field, case_id, violations);
    }
    if !matches!(
        record.get("canonical_gap_id"),
        Some(Value::Null | Value::String(_))
    ) {
        violations.push(format!(
            "Lane 1 evidence-quality case {case_id} canonical_gap_id must be string or null"
        ));
    }
    if !matches!(
        record.get("canonical_gap_reason"),
        Some(Value::Null | Value::String(_))
    ) {
        violations.push(format!(
            "Lane 1 evidence-quality case {case_id} canonical_gap_reason must be string or null"
        ));
    }
    if !matches!(
        record.get("canonical_gap_group_size"),
        Some(Value::Null | Value::Number(_))
    ) {
        violations.push(format!(
            "Lane 1 evidence-quality case {case_id} canonical_gap_group_size must be number or null"
        ));
    }
    if !matches!(record.get("headline_eligible"), Some(Value::Bool(_))) {
        violations.push(format!(
            "Lane 1 evidence-quality case {case_id} headline_eligible must be boolean"
        ));
    }
    match record.get("location") {
        Some(location @ Value::Object(_)) => {
            require_lane1_json_string_at(location, "file", case_id, violations);
            require_lane1_json_usize_at(location, "line", case_id, violations);
        }
        _ => violations.push(format!(
            "Lane 1 evidence-quality case {case_id} location must be an object"
        )),
    }
    match record.get("evidence_path") {
        Some(path @ Value::Object(_)) => {
            for stage in ["reach", "activate", "propagate", "observe", "discriminate"] {
                match path.get(stage) {
                    Some(stage_value @ Value::Object(_)) => {
                        require_lane1_json_string_at(stage_value, "state", case_id, violations);
                        require_lane1_json_string_at(
                            stage_value,
                            "confidence",
                            case_id,
                            violations,
                        );
                        require_lane1_json_string_at(stage_value, "summary", case_id, violations);
                    }
                    _ => violations.push(format!(
                        "Lane 1 evidence-quality case {case_id} evidence_path.{stage} must be an object"
                    )),
                }
            }
        }
        _ => violations.push(format!(
            "Lane 1 evidence-quality case {case_id} evidence_path must be an object"
        )),
    }
    match record.get("counts") {
        Some(counts @ Value::Object(_)) => {
            for field in [
                "observed_values",
                "missing_discriminators",
                "static_limitations",
                "related_tests_total",
            ] {
                require_lane1_json_usize_at(counts, field, case_id, violations);
            }
        }
        _ => violations.push(format!(
            "Lane 1 evidence-quality case {case_id} counts must be an object"
        )),
    }
    match record.get("top_related_test") {
        Some(test @ Value::Object(_)) => {
            for field in [
                "name",
                "file",
                "oracle_kind",
                "oracle_strength",
                "relation_reason",
                "relation_confidence",
            ] {
                require_lane1_json_string_at(test, field, case_id, violations);
            }
            require_lane1_json_usize_at(test, "line", case_id, violations);
        }
        _ => violations.push(format!(
            "Lane 1 evidence-quality case {case_id} top_related_test must be an object"
        )),
    }
    match record.get("recommendation") {
        Some(recommendation @ Value::Object(_)) => {
            require_lane1_json_string_at(recommendation, "action", case_id, violations);
            require_lane1_json_string_at(recommendation, "reason", case_id, violations);
            require_lane1_json_usize_at(
                recommendation,
                "candidate_values_count",
                case_id,
                violations,
            );
            if !matches!(
                recommendation.get("verify_command"),
                Some(Value::Null | Value::String(_))
            ) {
                violations.push(format!(
                    "Lane 1 evidence-quality case {case_id} recommendation.verify_command must be string or null"
                ));
            }
        }
        _ => violations.push(format!(
            "Lane 1 evidence-quality case {case_id} recommendation must be an object"
        )),
    }
    match record.get("actionability") {
        Some(actionability @ Value::Object(_)) => {
            require_lane1_json_string_at(actionability, "class", case_id, violations);
            if !matches!(
                actionability.get("has_concrete_guidance"),
                Some(Value::Bool(_))
            ) {
                violations.push(format!(
                    "Lane 1 evidence-quality case {case_id} actionability.has_concrete_guidance must be boolean"
                ));
            }
        }
        _ => violations.push(format!(
            "Lane 1 evidence-quality case {case_id} actionability must be an object"
        )),
    }
    match record.get("calibration") {
        Some(calibration @ Value::Object(_)) => {
            for field in ["availability", "confidence", "agreement"] {
                require_lane1_json_string_at(calibration, field, case_id, violations);
            }
        }
        _ => violations.push(format!(
            "Lane 1 evidence-quality case {case_id} calibration must be an object"
        )),
    }
    if !matches!(record.get("static_limitations"), Some(Value::Array(_))) {
        violations.push(format!(
            "Lane 1 evidence-quality case {case_id} static_limitations must be an array"
        ));
    }
}

fn lane1_count_field(record: &Value, field: &str) -> Option<usize> {
    record
        .get("counts")
        .and_then(|counts| json_usize_field(counts, field))
}

fn require_lane1_json_string_at(
    value: &Value,
    field: &str,
    case_id: &str,
    violations: &mut Vec<String>,
) {
    if json_string_field(value, field).is_none() {
        violations.push(format!(
            "Lane 1 evidence-quality case {case_id} is missing string field {field}"
        ));
    }
}

fn require_lane1_json_usize_at(
    value: &Value,
    field: &str,
    case_id: &str,
    violations: &mut Vec<String>,
) {
    if json_usize_field(value, field).is_none() {
        violations.push(format!(
            "Lane 1 evidence-quality case {case_id} is missing numeric field {field}"
        ));
    }
}

fn require_non_empty_string_array_at(
    value: &Value,
    field: &str,
    case_id: &str,
    violations: &mut Vec<String>,
) {
    match value.get(field) {
        Some(Value::Array(items))
            if !items.is_empty() && items.iter().all(|item| item.as_str().is_some()) => {}
        _ => violations.push(format!(
            "Lane 1 evidence-quality case {case_id} {field} must be a non-empty string array"
        )),
    }
}

fn require_string_array_contains_all(
    value: &Value,
    field: &str,
    required: &[&str],
    label: &str,
    violations: &mut Vec<String>,
) {
    let Some(items) = value.get(field).and_then(Value::as_array) else {
        violations.push(format!("{label} {field} must be a string array"));
        return;
    };
    let mut actual = BTreeSet::new();
    for item in items {
        match item.as_str() {
            Some(item) => {
                actual.insert(item.to_string());
            }
            None => violations.push(format!("{label} {field} contains a non-string item")),
        }
    }
    for expected in required {
        if !actual.contains(*expected) {
            violations.push(format!("{label} {field} is missing {expected}"));
        }
    }
}

fn string_array_contains_case_insensitive(value: &Value, field: &str, needle: &str) -> bool {
    let needle = needle.to_ascii_lowercase();
    value
        .get(field)
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .any(|item| item.to_ascii_lowercase().contains(&needle))
        })
}

fn validate_evidence_record_contract_record(
    case_id: &str,
    record: &Value,
    violations: &mut Vec<String>,
) {
    require_json_string_at(record, "schema_version", case_id, violations);
    if json_string_field(record, "schema_version").as_deref() != Some("0.1") {
        violations.push(format!(
            "evidence-record case {case_id} record schema_version must be 0.1"
        ));
    }
    for field in ["seam_id", "owner", "seam_kind", "grip_class"] {
        require_json_string_at(record, field, case_id, violations);
    }
    if !matches!(
        record.get("canonical_gap_id"),
        Some(Value::Null | Value::String(_))
    ) {
        violations.push(format!(
            "evidence-record case {case_id} canonical_gap_id must be string or null"
        ));
    }
    let canonical_gap_group_size_valid = match record.get("canonical_gap_group_size") {
        Some(Value::Null) => true,
        Some(Value::Number(number)) => number.as_u64().is_some(),
        _ => false,
    };
    if !canonical_gap_group_size_valid {
        violations.push(format!(
            "evidence-record case {case_id} canonical_gap_group_size must be number or null"
        ));
    }
    if !matches!(
        record.get("canonical_gap_reason"),
        Some(Value::Null | Value::String(_))
    ) {
        violations.push(format!(
            "evidence-record case {case_id} canonical_gap_reason must be string or null"
        ));
    }
    match record.get("canonical_gap_id") {
        Some(Value::Null) => {
            if !matches!(record.get("canonical_gap_group_size"), Some(Value::Null)) {
                violations.push(format!(
                    "evidence-record case {case_id} canonical_gap_group_size must be null when canonical_gap_id is null"
                ));
            }
            if !matches!(record.get("canonical_gap_reason"), Some(Value::Null)) {
                violations.push(format!(
                    "evidence-record case {case_id} canonical_gap_reason must be null when canonical_gap_id is null"
                ));
            }
        }
        Some(Value::String(_)) => {
            if json_usize_field(record, "canonical_gap_group_size").is_none() {
                violations.push(format!(
                    "evidence-record case {case_id} canonical_gap_group_size must be numeric when canonical_gap_id is present"
                ));
            }
            if json_string_field(record, "canonical_gap_reason").is_none() {
                violations.push(format!(
                    "evidence-record case {case_id} canonical_gap_reason must be string when canonical_gap_id is present"
                ));
            }
        }
        _ => {}
    }
    if !matches!(record.get("headline_eligible"), Some(Value::Bool(_))) {
        violations.push(format!(
            "evidence-record case {case_id} headline_eligible must be boolean"
        ));
    }

    match record.get("location") {
        Some(location @ Value::Object(_)) => {
            require_json_string_at(location, "file", case_id, violations);
            require_json_usize_at(location, "line", case_id, violations);
        }
        _ => violations.push(format!(
            "evidence-record case {case_id} location must be an object"
        )),
    }

    match record.get("evidence_path") {
        Some(path @ Value::Object(_)) => {
            for stage in ["reach", "activate", "propagate", "observe", "discriminate"] {
                match path.get(stage) {
                    Some(stage_value @ Value::Object(_)) => {
                        require_json_string_at(stage_value, "state", case_id, violations);
                        require_json_string_at(stage_value, "confidence", case_id, violations);
                        require_json_string_at(stage_value, "summary", case_id, violations);
                    }
                    _ => violations.push(format!(
                        "evidence-record case {case_id} evidence_path.{stage} must be an object"
                    )),
                }
            }
        }
        _ => violations.push(format!(
            "evidence-record case {case_id} evidence_path must be an object"
        )),
    }

    require_json_array_at(record, "observed_values", case_id, violations);
    require_json_array_at(record, "missing_discriminators", case_id, violations);
    require_json_usize_at(record, "related_tests_total", case_id, violations);
    require_json_array_at(record, "related_tests", case_id, violations);
    validate_evidence_record_related_tests(case_id, record.get("related_tests"), violations);
    validate_evidence_record_recommendation(case_id, record.get("recommendation"), violations);
    validate_evidence_record_actionability(case_id, record.get("actionability"), violations);
    validate_evidence_record_calibration(case_id, record.get("calibration"), violations);
    require_json_array_at(record, "static_limitations", case_id, violations);
}

fn validate_evidence_record_related_tests(
    case_id: &str,
    related_tests: Option<&Value>,
    violations: &mut Vec<String>,
) {
    let Some(Value::Array(tests)) = related_tests else {
        return;
    };
    for (idx, test) in tests.iter().enumerate() {
        validate_evidence_record_related_test(
            case_id,
            &format!("related_tests[{idx}]"),
            test,
            violations,
        );
    }
}

fn validate_evidence_record_related_test(
    case_id: &str,
    path: &str,
    test: &Value,
    violations: &mut Vec<String>,
) {
    let Value::Object(_) = test else {
        violations.push(format!(
            "evidence-record case {case_id} {path} must be an object"
        ));
        return;
    };
    for field in [
        "name",
        "file",
        "oracle_kind",
        "oracle_strength",
        "evidence_summary",
        "relation_reason",
        "relation_confidence",
    ] {
        require_json_string_at(test, field, case_id, violations);
    }
    require_json_usize_at(test, "line", case_id, violations);
    match test.get("oracle_semantics") {
        Some(semantics @ Value::Object(_)) => {
            require_json_string_at(semantics, "observes", case_id, violations);
            require_json_string_at(semantics, "missing", case_id, violations);
            if !matches!(
                semantics.get("upgrade_suggestion"),
                Some(Value::Null | Value::String(_))
            ) {
                violations.push(format!(
                    "evidence-record case {case_id} {path}.oracle_semantics.upgrade_suggestion must be string or null"
                ));
            }
        }
        _ => violations.push(format!(
            "evidence-record case {case_id} {path}.oracle_semantics must be an object"
        )),
    }
}

fn validate_evidence_record_recommendation(
    case_id: &str,
    recommendation: Option<&Value>,
    violations: &mut Vec<String>,
) {
    let Some(recommendation @ Value::Object(_)) = recommendation else {
        violations.push(format!(
            "evidence-record case {case_id} recommendation must be an object"
        ));
        return;
    };
    require_json_string_at(recommendation, "action", case_id, violations);
    require_json_string_at(recommendation, "reason", case_id, violations);
    require_json_array_at(recommendation, "candidate_values", case_id, violations);
    for optional in [
        "recommended_test",
        "nearest_test_to_imitate",
        "assertion_shape",
        "verify_command",
    ] {
        if !matches!(
            recommendation.get(optional),
            Some(Value::Null | Value::Object(_) | Value::String(_))
        ) {
            violations.push(format!(
                "evidence-record case {case_id} recommendation.{optional} must be present"
            ));
        }
    }
    if let Some(nearest @ Value::Object(_)) = recommendation.get("nearest_test_to_imitate") {
        validate_evidence_record_related_test(
            case_id,
            "recommendation.nearest_test_to_imitate",
            nearest,
            violations,
        );
    }
}

fn validate_evidence_record_actionability(
    case_id: &str,
    actionability: Option<&Value>,
    violations: &mut Vec<String>,
) {
    let Some(actionability @ Value::Object(_)) = actionability else {
        violations.push(format!(
            "evidence-record case {case_id} actionability must be an object"
        ));
        return;
    };
    let class = json_string_field(actionability, "class").unwrap_or_default();
    if !matches!(
        class.as_str(),
        "actionable_focused_test"
            | "actionable_assertion_upgrade"
            | "actionable_related_test_extension"
            | "needs_human_design"
            | "static_limitation"
            | "not_policy_relevant"
    ) {
        violations.push(format!(
            "evidence-record case {case_id} actionability.class is unsupported: {class}"
        ));
    }
    require_json_string_at(actionability, "reason", case_id, violations);
    if !matches!(
        actionability.get("has_concrete_guidance"),
        Some(Value::Bool(_))
    ) {
        violations.push(format!(
            "evidence-record case {case_id} actionability.has_concrete_guidance must be boolean"
        ));
    }
    let Some(signals @ Value::Object(_)) = actionability.get("signals") else {
        violations.push(format!(
            "evidence-record case {case_id} actionability.signals must be an object"
        ));
        return;
    };
    for signal in [
        "missing_discriminator",
        "candidate_value",
        "assertion_shape",
        "related_test",
        "recommended_test_target",
        "verification_command",
    ] {
        if !matches!(signals.get(signal), Some(Value::Bool(_))) {
            violations.push(format!(
                "evidence-record case {case_id} actionability.signals.{signal} must be boolean"
            ));
        }
    }
}

fn validate_evidence_record_calibration(
    case_id: &str,
    calibration: Option<&Value>,
    violations: &mut Vec<String>,
) {
    let Some(calibration @ Value::Object(_)) = calibration else {
        violations.push(format!(
            "evidence-record case {case_id} calibration must be an object"
        ));
        return;
    };
    for field in ["availability", "confidence", "agreement"] {
        require_json_string_at(calibration, field, case_id, violations);
    }
}

fn require_json_string_at(value: &Value, field: &str, case_id: &str, violations: &mut Vec<String>) {
    if json_string_field(value, field).is_none() {
        violations.push(format!(
            "evidence-record case {case_id} is missing string field {field}"
        ));
    }
}

fn require_json_usize_at(value: &Value, field: &str, case_id: &str, violations: &mut Vec<String>) {
    if json_usize_field(value, field).is_none() {
        violations.push(format!(
            "evidence-record case {case_id} is missing numeric field {field}"
        ));
    }
}

fn require_json_array_at(value: &Value, field: &str, case_id: &str, violations: &mut Vec<String>) {
    if !matches!(value.get(field), Some(Value::Array(_))) {
        violations.push(format!(
            "evidence-record case {case_id} is missing array field {field}"
        ));
    }
}
