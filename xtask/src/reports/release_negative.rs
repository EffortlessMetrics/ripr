//! Integrated negative corpus for the release-readiness artifact, pair,
//! verify, and receipt authority chain (#2824).
//!
//! Architecture: this harness orchestrates fixture creation, one-mutation
//! injection, command execution, failure-receipt retention, byte-exact
//! restoration, and reporting. The installed candidate binary remains the
//! ONLY artifact / pair / verify / receipt validator — this module never
//! parses artifact validity independently of that binary, never duplicates
//! comparability rules, and asserts process exit status before the closed
//! reason token. Two deferred negatives (#2921 claim boundary) have no real
//! producer on main, so they are recorded as explicit `not_applicable`
//! dispositions rather than fabricated rejections.

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(test)]
use super::release::release_temp_root;
use super::release::{
    BOUNDARY_GAP_SEAM_ID, CommandResult, absolute_installed_binary, artifact_string,
    checkout_fixture_commit, command_details, create_authentic_repo_exposure_fixture, fixture_head,
    git_worktree_is_clean, installed_ripr_binary, produce_authentic_chain_in_fixture,
    read_crate_version, read_json_value, run_command_in_dir, run_fixture_git_command,
    run_packaged_install, run_producer_check,
};
use super::release_server::{hex_lower, sha256_file};

const NEGATIVE_WORK_DIR: &str = "target/ripr/release-negative-corpus";
const BEFORE_ARTIFACT: &str = "before.repo-exposure.json";
const AFTER_ARTIFACT: &str = "after.repo-exposure.json";
const THIRD_ARTIFACT: &str = "after-third.repo-exposure.json";
const ANALYSIS_OUTCOME: &str = "analysis-outcome.json";
const VERIFY_JSON: &str = "agent-verify.json";
const RECEIPT_JSON: &str = "agent-receipt.json";
const CASE_RECEIPT_OUT: &str = "case-receipt-out.json";
const CHAIN_FILES: [&str; 5] = [
    BEFORE_ARTIFACT,
    AFTER_ARTIFACT,
    ANALYSIS_OUTCOME,
    VERIFY_JSON,
    RECEIPT_JSON,
];
/// The producer's fixed `raw_json_placeholder_v1` placeholder digest. The
/// harness uses it only to re-commit a deliberately mutated artifact so the
/// installed binary — still the only validator — reaches the intended gate
/// instead of the generic commitment gate.
const CONTENT_PLACEHOLDER: &str =
    "sha256:0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReleaseNegativeArgs {
    version: String,
}

#[derive(Clone, Debug)]
struct CandidateIdentity {
    path: String,
    version_output: String,
    sha256: String,
}

#[derive(Clone, Debug)]
struct ArtifactDigestRecord {
    name: String,
    sha256: String,
    input_identity: Option<String>,
    snapshot_identity: Option<String>,
}

#[derive(Clone, Debug)]
struct BaselineContext {
    fixture_root: PathBuf,
    before_sha: String,
    after_sha: String,
    artifacts: Vec<ArtifactDigestRecord>,
}

#[derive(Clone, Debug)]
struct StateFile {
    name: String,
    sha256: String,
    bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
struct StateSnapshot {
    head: String,
    files: Vec<StateFile>,
}

/// What the installed binary must do with the mutated state. Rejection
/// cases assert the exit status FIRST and the closed reason token second;
/// pass cases pin the exact typed projection the honest output must keep.
#[derive(Clone, Debug)]
enum Expectation {
    Reject {
        token: &'static str,
        out_absent: Option<&'static str>,
        out_preserved: Option<(String, String)>,
    },
    Pass {
        out_file: &'static str,
        pointers: Vec<(&'static str, &'static str)>,
    },
}

impl Expectation {
    fn reject(token: &'static str) -> Self {
        Self::Reject {
            token,
            out_absent: None,
            out_preserved: None,
        }
    }

    fn reject_without_out(token: &'static str) -> Self {
        Self::Reject {
            token,
            out_absent: Some(CASE_RECEIPT_OUT),
            out_preserved: None,
        }
    }

    fn expected_kind(&self) -> String {
        match self {
            Self::Reject { token, .. } => (*token).to_string(),
            Self::Pass { .. } => "none (expected pass with a pinned honest projection)".to_string(),
        }
    }
}

#[derive(Clone, Debug)]
struct CaseExecution {
    mutation: String,
    argv: Vec<String>,
    control_argv: Vec<String>,
    expected: Expectation,
    original: Vec<ArtifactDigestRecord>,
    mutated: Vec<ArtifactDigestRecord>,
    retain: Vec<(String, String)>,
    snapshot: StateSnapshot,
    run_cwd: Option<PathBuf>,
    extra_cleanup: Vec<PathBuf>,
    /// Case-specific before/after revisions when the case builds its own
    /// chain (the two-seam honesty case); `None` means the baseline pair.
    /// Retained receipts must name the revisions the case actually produced
    /// — never the baseline's (#2824 review).
    fixture_shas: Option<(String, String)>,
}

struct CaseEnv<'a> {
    binary: &'a Path,
    root: &'a Path,
    case_dir: &'a Path,
    before_sha: &'a str,
    after_sha: &'a str,
}

struct CaseSpec {
    id: &'static str,
    group: &'static str,
    description: &'static str,
    run: fn(&CaseEnv) -> Result<CaseExecution, String>,
}

#[derive(Clone, Debug)]
struct CaseReceipt {
    case_id: String,
    group: String,
    description: String,
    mutation: String,
    candidate_path: String,
    candidate_version: String,
    candidate_sha256: String,
    fixture_root: String,
    before_sha: String,
    after_sha: String,
    argv: Vec<String>,
    control_argv: Vec<String>,
    original_artifacts: Vec<ArtifactDigestRecord>,
    mutated_artifacts: Vec<ArtifactDigestRecord>,
    expected_failure_kind: String,
    actual_failure_kind: String,
    exit_status: Option<i32>,
    process_outcome: String,
    restoration_outcome: String,
    control_outcome: String,
    cleanup_outcome: String,
    status: String,
    violations: Vec<String>,
    details: Vec<String>,
}

impl CaseReceipt {
    fn new(spec: &CaseSpec, candidate: &CandidateIdentity, baseline: &BaselineContext) -> Self {
        Self {
            case_id: spec.id.to_string(),
            group: spec.group.to_string(),
            description: spec.description.to_string(),
            mutation: String::new(),
            candidate_path: candidate.path.clone(),
            candidate_version: candidate.version_output.clone(),
            candidate_sha256: candidate.sha256.clone(),
            fixture_root: String::new(),
            before_sha: baseline.before_sha.clone(),
            after_sha: baseline.after_sha.clone(),
            argv: Vec::new(),
            control_argv: Vec::new(),
            original_artifacts: Vec::new(),
            mutated_artifacts: Vec::new(),
            expected_failure_kind: String::new(),
            actual_failure_kind: String::new(),
            exit_status: None,
            process_outcome: "not_run".to_string(),
            restoration_outcome: "not_run".to_string(),
            control_outcome: "not_run".to_string(),
            cleanup_outcome: "not_run".to_string(),
            status: "fail".to_string(),
            violations: Vec::new(),
            details: Vec::new(),
        }
    }

    fn finalize(&mut self) {
        // Real-producers-only (#2824 review): every accepted token names the
        // producer that emits it in this tree —
        // - `rejected_as_expected`: evaluate_rejection on a Reject-arm case;
        // - `passed_with_pinned_projection`: evaluate_pass on the Pass-arm
        //   honesty case (receipt-unmoved-retained-target);
        // - `restored_byte_exact`: restore_state after every case.
        // `verified_unchanged` is deliberately not accepted: no producer in
        // this tree emits it.
        let passed = self.process_outcome == "rejected_as_expected"
            || self.process_outcome == "passed_with_pinned_projection";
        let restoration_ok = self.restoration_outcome == "restored_byte_exact";
        // A recorded violation fails the case even when every outcome token
        // matched: a corpus that records violations but ignores them would
        // report passes on partial-output regressions (#2824 review).
        if passed
            && restoration_ok
            && self.control_outcome == "pass"
            && self.cleanup_outcome == "removed"
            && self.violations.is_empty()
        {
            self.status = "pass".to_string();
        } else {
            self.status = "fail".to_string();
        }
    }
}

struct NegativeCorpusReport {
    version: String,
    status: String,
    run_status: String,
    covered_families: Vec<String>,
    deferred_families: Vec<String>,
    candidate: CandidateIdentity,
    baseline_root: String,
    baseline_before_sha: String,
    baseline_after_sha: String,
    baseline_artifacts: Vec<ArtifactDigestRecord>,
    baseline_cleanup: String,
    cases: Vec<CaseReceipt>,
    dispositions: Vec<(String, String)>,
}

pub(crate) fn release_negative_corpus(args: &[String]) -> Result<(), String> {
    let args = parse_release_negative_args(args)?;
    fs::create_dir_all(NEGATIVE_WORK_DIR)
        .map_err(|err| format!("failed to create {NEGATIVE_WORK_DIR}: {err}"))?;
    match run_negative_corpus(&args.version) {
        Ok(report) => {
            let json = negative_corpus_json(&report)?;
            crate::write_report("release-negative-corpus.json", &json)?;
            crate::write_report(
                "release-negative-corpus.md",
                &negative_corpus_markdown(&report),
            )?;
            if report.status == "fail" {
                return Err(
                    "release negative corpus failed; see target/ripr/reports/release-negative-corpus.md"
                        .to_string(),
                );
            }
            Ok(())
        }
        Err(phase_error) => {
            let report = json!({
                "report": "release-negative-corpus",
                "schema_version": "1",
                "version": args.version,
                "status": "fail",
                "phase_error": phase_error.clone(),
            });
            let body = serde_json::to_string_pretty(&report)
                .map_err(|err| format!("render phase-failure report failed: {err}"))?;
            crate::write_report("release-negative-corpus.json", &body)?;
            crate::write_report(
                "release-negative-corpus.md",
                &format!(
                    "# release-negative-corpus\n\nStatus: fail\n\nThe corpus run failed before the case matrix completed:\n\n```text\n{phase_error}\n```\n"
                ),
            )?;
            Err(
                "release negative corpus failed; see target/ripr/reports/release-negative-corpus.md"
                    .to_string(),
            )
        }
    }
}

fn run_negative_corpus(version: &str) -> Result<NegativeCorpusReport, String> {
    let (binary, candidate) = resolve_candidate(version)?;
    let baseline = build_baseline(&binary)?;
    let corpus_dir = Path::new(NEGATIVE_WORK_DIR);
    // Never let stale per-case receipts from an older matrix linger next to
    // a fresh run: the case evidence for this run starts empty (#2824).
    let _ = fs::remove_dir_all(corpus_dir.join("cases"));
    let mut cases = Vec::new();
    for spec in case_specs() {
        cases.push(execute_case(
            &spec, &binary, &candidate, &baseline, corpus_dir,
        ));
    }
    let baseline_cleanup = match fs::remove_dir_all(&baseline.fixture_root) {
        Ok(()) => "removed".to_string(),
        Err(err) => format!("failed: {err}"),
    };
    let status = if baseline_cleanup == "removed" && cases.iter().all(|case| case.status == "pass")
    {
        "pass"
    } else {
        "fail"
    };
    // Disclose the partial matrix truthfully (#2824 review): a slice that
    // lands only some case families reports `families_deferred`, never a
    // silent pass that reads as the full corpus.
    let (covered_families, deferred_families) = family_coverage();
    let run_status = if deferred_families.is_empty() {
        "complete"
    } else {
        "families_deferred"
    };
    Ok(NegativeCorpusReport {
        version: version.to_string(),
        status: status.to_string(),
        run_status: run_status.to_string(),
        covered_families,
        deferred_families,
        candidate,
        baseline_root: crate::normalize_path(&baseline.fixture_root),
        baseline_before_sha: baseline.before_sha.clone(),
        baseline_after_sha: baseline.after_sha.clone(),
        baseline_artifacts: baseline.artifacts.clone(),
        baseline_cleanup,
        cases,
        dispositions: deferred_dispositions(),
    })
}

/// The closed family vocabulary and the case group that produces each
/// family. Coverage is derived from the actual case registry — never from
/// hardcoded prose — so a slice cannot claim a family it does not run.
const CASE_FAMILY_GROUPS: [(&str, &[&str]); 3] = [
    ("artifact", &["artifact"]),
    ("pair", &["pair"]),
    ("verify_receipt", &["verify", "receipt"]),
];

fn family_coverage() -> (Vec<String>, Vec<String>) {
    let groups = case_specs()
        .iter()
        .map(|spec| spec.group)
        .collect::<Vec<_>>();
    let mut covered = Vec::new();
    let mut deferred = Vec::new();
    for (group, families) in CASE_FAMILY_GROUPS {
        for family in families {
            if groups.contains(&group) {
                covered.push(family.to_string());
            } else {
                deferred.push(family.to_string());
            }
        }
    }
    (covered, deferred)
}

/// Resolve the candidate exactly like `release_readiness`: the packaged crate
/// is installed from a clean tree and the installed binary is the only
/// validator this corpus uses.
fn resolve_candidate(version: &str) -> Result<(PathBuf, CandidateIdentity), String> {
    let crate_version =
        read_crate_version(Path::new("crates/ripr/Cargo.toml"), Path::new("Cargo.toml"))
            .ok_or_else(|| {
                "crate version could not be read; release-prep should run this gate explicitly"
                    .to_string()
            })?;
    if crate_version != version {
        return Err(format!(
            "requested version {version} does not match crates/ripr version {crate_version}"
        ));
    }
    if !git_worktree_is_clean()? {
        return Err(
            "dirty tree; the negative corpus packages the committed candidate, so rerun on a clean tree"
                .to_string(),
        );
    }
    let install = run_packaged_install(version, &crate_version)?;
    if !install.success {
        return Err(format!(
            "packaged install failed: {}",
            install.details.join("; ")
        ));
    }
    let binary = absolute_installed_binary(&installed_ripr_binary())?;
    let version_args = vec!["--version".to_string()];
    let cwd =
        std::env::current_dir().map_err(|err| format!("read current directory failed: {err}"))?;
    let version_result = run_command_in_dir(&binary, &version_args, &cwd, "candidate --version")?;
    if !version_result.success {
        return Err(format!(
            "candidate --version failed: {}",
            command_details(&version_result).join("; ")
        ));
    }
    let sha256 = sha256_file(&binary)?;
    Ok((
        binary,
        CandidateIdentity {
            path: crate::normalize_path(&installed_ripr_binary()),
            version_output: version_result.stdout.trim().to_string(),
            sha256: format!("sha256:{sha256}"),
        },
    ))
}

/// Build the authentic baseline chain in a controlled external fixture with
/// the shared journey producer and retain the immutable positive artifacts.
/// Every failure path — chain production or retention — removes the external
/// fixture root so a failed baseline never leaks it.
fn build_baseline(binary: &Path) -> Result<BaselineContext, String> {
    let fixture = create_authentic_repo_exposure_fixture()?;
    let result = (|| {
        produce_authentic_chain_in_fixture(
            binary,
            &fixture.root,
            &fixture.before_commit,
            &fixture.after_commit,
        )
        .map_err(|error| format!("authentic baseline chain failed: {error}"))?;
        let baseline_dir = Path::new(NEGATIVE_WORK_DIR).join("baseline");
        fs::create_dir_all(&baseline_dir)
            .map_err(|err| format!("create baseline retention dir failed: {err}"))?;
        let mut artifacts = Vec::new();
        for name in CHAIN_FILES {
            let retained = baseline_dir.join(name);
            fs::copy(fixture.root.join(name), &retained)
                .map_err(|err| format!("retain baseline artifact {name} failed: {err}"))?;
            artifacts.push(artifact_digest(&baseline_dir, name)?);
        }
        Ok(BaselineContext {
            fixture_root: fixture.root.clone(),
            before_sha: fixture.before_commit.clone(),
            after_sha: fixture.after_commit.clone(),
            artifacts,
        })
    })();
    result.inspect_err(|_error| {
        let _ = fs::remove_dir_all(&fixture.root);
    })
}

fn execute_case(
    spec: &CaseSpec,
    binary: &Path,
    candidate: &CandidateIdentity,
    baseline: &BaselineContext,
    corpus_dir: &Path,
) -> CaseReceipt {
    let mut receipt = CaseReceipt::new(spec, candidate, baseline);
    let case_dir = corpus_dir.join("cases").join(spec.id);
    let workspace = case_dir.join("workspace");
    let run = (|| -> Result<(), String> {
        fs::create_dir_all(&case_dir).map_err(|err| format!("create case dir failed: {err}"))?;
        copy_dir_recursive(&baseline.fixture_root, &workspace)?;
        // The copied chain artifacts are bound to the baseline root; each
        // case re-produces its own authentic chain before any mutation.
        for name in CHAIN_FILES {
            let _ = fs::remove_file(workspace.join(name));
        }
        receipt.fixture_root = crate::normalize_path(&workspace);
        let env = CaseEnv {
            binary,
            root: &workspace,
            case_dir: &case_dir,
            before_sha: &baseline.before_sha,
            after_sha: &baseline.after_sha,
        };
        let execution = (spec.run)(&env)?;
        if matches!(&execution.expected, Expectation::Reject { token, .. } if token.is_empty()) {
            return Err(format!(
                "case {} did not declare an expected failure token",
                spec.id
            ));
        }
        // Symmetric fail-closed guard (#2824 review): a pass expectation
        // without pinned projection pointers would credit any successful
        // command — a vacuous pass.
        if matches!(&execution.expected, Expectation::Pass { pointers, .. } if pointers.is_empty())
        {
            return Err(format!(
                "case {} declared a pass expectation without a pinned projection",
                spec.id
            ));
        }
        receipt.mutation = execution.mutation.clone();
        receipt.argv = execution.argv.clone();
        receipt.control_argv = execution.control_argv.clone();
        receipt.original_artifacts = execution.original.clone();
        receipt.mutated_artifacts = execution.mutated.clone();
        receipt.expected_failure_kind = execution.expected.expected_kind();
        // Provenance (#2824 review): a case that builds its own chain must
        // name the revisions it actually produced, not the baseline pair.
        if let Some((before_sha, after_sha)) = &execution.fixture_shas {
            receipt.before_sha = before_sha.clone();
            receipt.after_sha = after_sha.clone();
        }

        let run_cwd = execution
            .run_cwd
            .clone()
            .unwrap_or_else(|| workspace.clone());
        let result = run_command_in_dir(binary, &execution.argv, &run_cwd, spec.id)?;
        receipt.exit_status = result.status;
        match &execution.expected {
            Expectation::Reject {
                token,
                out_absent,
                out_preserved,
            } => {
                let (outcome, actual, mut violations) = evaluate_rejection(token, &result);
                receipt.process_outcome = outcome;
                receipt.actual_failure_kind = actual;
                if let Some(out) = out_absent
                    && run_cwd.join(out).exists()
                {
                    violations.push(format!(
                        "a rejected issuance must not create output file {out}"
                    ));
                }
                if let Some((path, digest)) = out_preserved {
                    let preserved = run_cwd.join(path);
                    match file_digest_prefixed(&preserved) {
                        Ok(actual) if actual == *digest => {}
                        Ok(actual) => violations.push(format!(
                            "a rejected issuance must not update {path}: digest changed from {digest} to {actual}"
                        )),
                        Err(err) => violations.push(format!(
                            "a rejected issuance must preserve {path}: {err}"
                        )),
                    }
                }
                receipt.violations.append(&mut violations);
            }
            Expectation::Pass { out_file, pointers } => {
                let (outcome, violations) = evaluate_pass(&run_cwd, out_file, pointers, &result);
                // "none" only on a real pass; a failed honesty pin records
                // the actual outcome as the failure kind (#2824 review).
                receipt.actual_failure_kind = if outcome == "passed_with_pinned_projection" {
                    "none".to_string()
                } else {
                    outcome.clone()
                };
                receipt.process_outcome = outcome;
                receipt.violations.extend(violations);
            }
        }

        // Retain the mutated evidence before restoration reverts it.
        let mutated_dir = case_dir.join("mutated");
        for (source, destination) in &execution.retain {
            let source_path = run_cwd.join(source);
            if !source_path.is_file() {
                receipt
                    .violations
                    .push(format!("retained mutation source {source} is missing"));
                continue;
            }
            let destination_path = mutated_dir.join(destination);
            if let Some(parent) = destination_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|err| format!("create mutation retention dir failed: {err}"))?;
            }
            fs::copy(&source_path, &destination_path)
                .map_err(|err| format!("retain mutated artifact {source} failed: {err}"))?;
        }

        // Restore the original bytes/state byte-exactly, verified by digest.
        match restore_state(&workspace, &execution.snapshot) {
            Ok(mut restore_details) => {
                receipt.restoration_outcome = "restored_byte_exact".to_string();
                receipt.details.append(&mut restore_details);
            }
            Err(err) => {
                receipt.restoration_outcome = format!("failed: {err}");
            }
        }

        // Rerun the original command: the restored state must pass again.
        let control = run_command_in_dir(binary, &execution.control_argv, &workspace, "control")?;
        if control.success {
            receipt.control_outcome = "pass".to_string();
        } else {
            receipt.control_outcome = "fail".to_string();
            receipt.details.push(format!(
                "control rerun after restoration failed: {}",
                command_details(&control).join("; ")
            ));
        }

        for extra in &execution.extra_cleanup {
            let _ = fs::remove_dir_all(extra);
        }
        Ok(())
    })();
    let cleanup = fs::remove_dir_all(&workspace);
    receipt.cleanup_outcome = match cleanup {
        Ok(()) => "removed".to_string(),
        Err(err) => format!("failed: {err}"),
    };
    if let Err(err) = run {
        receipt.details.push(err);
    }
    receipt.finalize();
    if let Err(err) = write_case_receipt(&case_dir, &receipt) {
        receipt
            .details
            .push(format!("write case receipt failed: {err}"));
        receipt.status = "fail".to_string();
    }
    receipt
}

/// Evaluate a rejection case: exit status FIRST, then the closed reason
/// token. Stdout must stay empty on a rejected command — the typed failure
/// lives on stderr and no partial output may reach the human surface.
fn evaluate_rejection(token: &str, result: &CommandResult) -> (String, String, Vec<String>) {
    let mut violations = Vec::new();
    if result.success {
        return (
            "unexpected_pass".to_string(),
            "none (command passed)".to_string(),
            vec!["command passed; a rejection was expected".to_string()],
        );
    }
    if !result.stdout.is_empty() {
        violations.push("a rejected command must render nothing to stdout".to_string());
    }
    if result.stderr.contains(token) {
        (
            "rejected_as_expected".to_string(),
            token.to_string(),
            violations,
        )
    } else {
        (
            "wrong_failure_kind".to_string(),
            first_stderr_line(&result.stderr),
            violations,
        )
    }
}

fn evaluate_pass(
    run_cwd: &Path,
    out_file: &str,
    pointers: &[(&str, &str)],
    result: &CommandResult,
) -> (String, Vec<String>) {
    // Fail closed on a vacuous expectation (#2824 review): no pinned
    // projection pointer means any successful command would pass.
    if pointers.is_empty() {
        return (
            "output_contract_violation".to_string(),
            vec!["a pass expectation must pin at least one projection pointer".to_string()],
        );
    }
    if !result.success {
        return (
            "unexpected_failure".to_string(),
            vec![format!(
                "honesty-pin command failed: {}",
                command_details(result).join("; ")
            )],
        );
    }
    let out_path = run_cwd.join(out_file);
    let value = match read_json_value(&out_path) {
        Ok(value) => value,
        Err(err) => {
            return (
                "output_contract_violation".to_string(),
                vec![format!("read pinned output {out_file} failed: {err}")],
            );
        }
    };
    let mut violations = Vec::new();
    for (pointer, expected) in pointers {
        let actual = value.pointer(pointer).and_then(Value::as_str);
        if actual != Some(*expected) {
            violations.push(format!(
                "pinned projection {pointer} must stay {expected:?}, got {:?}",
                actual.unwrap_or("<missing>")
            ));
        }
    }
    if violations.is_empty() {
        ("passed_with_pinned_projection".to_string(), violations)
    } else {
        ("output_contract_violation".to_string(), violations)
    }
}

fn first_stderr_line(stderr: &str) -> String {
    let line = stderr.lines().find(|line| !line.trim().is_empty());
    match line {
        Some(line) => line.trim().chars().take(200).collect(),
        None => "<no stderr>".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Case setups and mutations
// ---------------------------------------------------------------------------

fn produce_case_chain(env: &CaseEnv) -> Result<(), String> {
    produce_authentic_chain_in_fixture(env.binary, env.root, env.before_sha, env.after_sha)?;
    Ok(())
}

fn verify_case_execution(env: &CaseEnv, mutation: &str) -> Result<CaseExecution, String> {
    produce_case_chain(env)?;
    let snapshot = snapshot_state(env.root)?;
    Ok(CaseExecution {
        mutation: mutation.to_string(),
        argv: agent_verify_argv(BEFORE_ARTIFACT, AFTER_ARTIFACT),
        control_argv: agent_verify_argv(BEFORE_ARTIFACT, AFTER_ARTIFACT),
        expected: Expectation::reject(""),
        original: Vec::new(),
        mutated: Vec::new(),
        retain: Vec::new(),
        snapshot,
        run_cwd: None,
        extra_cleanup: Vec::new(),
        fixture_shas: None,
    })
}

fn receipt_case_execution(env: &CaseEnv, mutation: &str) -> Result<CaseExecution, String> {
    produce_case_chain(env)?;
    let snapshot = snapshot_state(env.root)?;
    Ok(CaseExecution {
        mutation: mutation.to_string(),
        argv: agent_receipt_argv(BOUNDARY_GAP_SEAM_ID, CASE_RECEIPT_OUT),
        control_argv: agent_receipt_argv(BOUNDARY_GAP_SEAM_ID, RECEIPT_JSON),
        expected: Expectation::reject_without_out(""),
        original: Vec::new(),
        mutated: Vec::new(),
        retain: Vec::new(),
        snapshot,
        run_cwd: None,
        extra_cleanup: Vec::new(),
        fixture_shas: None,
    })
}

fn agent_verify_argv(before: &str, after: &str) -> Vec<String> {
    [
        "agent", "verify", "--root", ".", "--before", before, "--after", after, "--json",
    ]
    .iter()
    .map(|arg| (*arg).to_string())
    .collect()
}

fn agent_receipt_argv(seam_id: &str, out: &str) -> Vec<String> {
    [
        "agent",
        "receipt",
        "--root",
        ".",
        "--verify-json",
        VERIFY_JSON,
        "--seam-id",
        seam_id,
        "--json",
        "--out",
        out,
    ]
    .iter()
    .map(|arg| (*arg).to_string())
    .collect()
}

fn retain_pair() -> Vec<(String, String)> {
    vec![
        (BEFORE_ARTIFACT.to_string(), BEFORE_ARTIFACT.to_string()),
        (AFTER_ARTIFACT.to_string(), AFTER_ARTIFACT.to_string()),
    ]
}

fn pair_digests(env: &CaseEnv) -> Result<Vec<ArtifactDigestRecord>, String> {
    artifact_digests(env.root, &[BEFORE_ARTIFACT, AFTER_ARTIFACT])
}

fn artifact_digests(root: &Path, names: &[&str]) -> Result<Vec<ArtifactDigestRecord>, String> {
    names
        .iter()
        .map(|name| artifact_digest(root, name))
        .collect()
}

fn artifact_digest(root: &Path, name: &str) -> Result<ArtifactDigestRecord, String> {
    let path = root.join(name);
    let sha256 = file_digest_prefixed(&path)?;
    let (input_identity, snapshot_identity) = if name.ends_with("repo-exposure.json") {
        let value = read_json_value(&path)?;
        (
            Some(artifact_string(&value, &["artifact", "analysis", "input_identity"])?.to_string()),
            Some(artifact_string(&value, &["artifact", "snapshot_identity"])?.to_string()),
        )
    } else {
        (None, None)
    };
    Ok(ArtifactDigestRecord {
        name: name.to_string(),
        sha256,
        input_identity,
        snapshot_identity,
    })
}

fn file_digest_prefixed(path: &Path) -> Result<String, String> {
    Ok(format!("sha256:{}", sha256_file(path)?))
}

/// Rebind an artifact to a different revision while keeping the artifact
/// internally consistent (snapshot identity follows the declared head) and
/// re-committing the content commitment, so the installed binary reaches the
/// intended lineage/revision gate.
fn rebind_artifact_head(root: &Path, name: &str, new_head: &str) -> Result<(), String> {
    mutate_artifact_value(root, name, |value| {
        let input = value
            .pointer("/artifact/analysis/input_identity")
            .and_then(Value::as_str)
            .ok_or_else(|| "artifact input identity is missing".to_string())?
            .to_string();
        set_json_string(value, "/artifact/repository/head", new_head)?;
        set_json_string(
            value,
            "/artifact/snapshot_identity",
            &format!("snapshot:{input};revision:{new_head}"),
        )
    })
}

/// Apply a JSON-value mutation to one artifact and re-commit its content
/// commitment over the exact new bytes (the producer's
/// `raw_json_placeholder_v1` rule), so the mutated artifact stays
/// well-formed for every gate except the one under test.
fn mutate_artifact_value(
    root: &Path,
    name: &str,
    mutate: impl FnOnce(&mut Value) -> Result<(), String>,
) -> Result<(), String> {
    let path = root.join(name);
    let text = crate::read_text_lossy(&path)?;
    let mut value: Value = serde_json::from_str(&text).map_err(|err| {
        format!(
            "parse {} for mutation failed: {err}",
            crate::normalize_path(&path)
        )
    })?;
    mutate(&mut value)?;
    *value_placeholder_slot(&mut value)? = Value::String(CONTENT_PLACEHOLDER.to_string());
    let raw = serde_json::to_string_pretty(&value)
        .map_err(|err| format!("render mutated {name} failed: {err}"))?;
    let committed = recommit_repo_exposure_bytes(&raw)?;
    fs::write(&path, committed).map_err(|err| {
        format!(
            "write mutated {} failed: {err}",
            crate::normalize_path(&path)
        )
    })
}

/// Borrow the artifact content-commitment slot for placeholder staging.
fn value_placeholder_slot(value: &mut Value) -> Result<&mut Value, String> {
    value
        .pointer_mut("/artifact/content_sha256")
        .ok_or_else(|| "artifact content_sha256 slot is missing".to_string())
}

fn set_json_string(value: &mut Value, pointer: &str, new_value: &str) -> Result<(), String> {
    let slot = value
        .pointer_mut(pointer)
        .ok_or_else(|| format!("mutation pointer {pointer} is missing"))?;
    if !slot.is_string() {
        return Err(format!("mutation pointer {pointer} is not a string slot"));
    }
    *slot = Value::String(new_value.to_string());
    Ok(())
}

/// Locate the value span of the one `content_sha256` key in a raw artifact
/// document. The exactly-one-key assertion is part of the contract, so every
/// commitment mutation and recommitment site shares this single scanner
/// (#2824 review); the returned range covers the bytes between the quotes.
fn content_commitment_value_span(text: &str) -> Result<std::ops::Range<usize>, String> {
    let key = "\"content_sha256\"";
    if text.matches(key).count() != 1 {
        return Err(format!("expected exactly one {key} key"));
    }
    let Some(key_start) = text.find(key) else {
        return Err(format!("expected the {key} key"));
    };
    let value_search_start = key_start + key.len();
    let Some(value_offset) = text[value_search_start..].find('"') else {
        return Err(format!("{key} must have a string value"));
    };
    let value_start = value_search_start + value_offset + 1;
    let Some(end_offset) = text[value_start..].find('"') else {
        return Err(format!("{key} value must be terminated"));
    };
    Ok(value_start..value_start + end_offset)
}

/// Re-commit the artifact content commitment: replace the one
/// `content_sha256` value with the fixed placeholder, hash the exact bytes,
/// and write the recomputed digest back. Mirrors the producer's documented
/// `raw_json_placeholder_v1` canonicalization; the installed binary remains
/// the only authority that validates the result.
fn recommit_repo_exposure_bytes(raw: &str) -> Result<String, String> {
    let mut text = raw.to_string();
    let span = content_commitment_value_span(&text)?;
    text.replace_range(span, CONTENT_PLACEHOLDER);
    if text.matches(CONTENT_PLACEHOLDER).count() != 1 {
        return Err("recommit placeholder must appear exactly once".to_string());
    }
    let digest = format!("sha256:{}", hex_lower(&Sha256::digest(text.as_bytes())));
    Ok(text.replace(CONTENT_PLACEHOLDER, &digest))
}

/// Recompute the declared commitment over raw bytes; used by tests to show
/// the recommit binds the mutated bytes and detects stale commitments.
#[cfg(test)]
fn recompute_content_commitment(raw: &str) -> Result<String, String> {
    let mut text = raw.to_string();
    let span = content_commitment_value_span(&text)?;
    text.replace_range(span, CONTENT_PLACEHOLDER);
    Ok(format!(
        "sha256:{}",
        hex_lower(&Sha256::digest(text.as_bytes()))
    ))
}

fn replace_once(text: &str, from: &str, to: &str) -> Result<String, String> {
    let count = text.matches(from).count();
    if count != 1 {
        return Err(format!(
            "mutation anchor {from:?} occurs {count} times (expected exactly one)"
        ));
    }
    Ok(text.replacen(from, to, 1))
}

fn mutate_file_text(root: &Path, name: &str, from: &str, to: &str) -> Result<(), String> {
    let path = root.join(name);
    let text = crate::read_text_lossy(&path)?;
    let updated = replace_once(&text, from, to)?;
    fs::write(&path, updated).map_err(|err| {
        format!(
            "write mutated {} failed: {err}",
            crate::normalize_path(&path)
        )
    })
}

/// Shift the final lowercase hex character of an identity digest so the
/// mutated identity stays well-formed but no longer matches its pair.
fn shift_final_hex(text: &str) -> Result<String, String> {
    let mut chars = text.chars().collect::<Vec<_>>();
    let Some(last) = chars.pop() else {
        return Err("cannot shift an empty identity".to_string());
    };
    let shifted = match last {
        '0' => '1',
        '1' => '2',
        '2' => '3',
        '3' => '4',
        '4' => '5',
        '5' => '6',
        '6' => '7',
        '7' => '8',
        '8' => '9',
        '9' => 'a',
        'a' => 'b',
        'b' => 'c',
        'c' => 'd',
        'd' => 'e',
        'e' => 'f',
        'f' => '0',
        other => {
            return Err(format!(
                "identity does not end in a lowercase hex digit: {other:?}"
            ));
        }
    };
    chars.push(shifted);
    Ok(chars.into_iter().collect())
}

fn commit_empty(root: &Path, message: &str) -> Result<String, String> {
    run_fixture_git_command(
        root,
        &[
            "-c",
            "core.hooksPath=",
            "commit",
            "--allow-empty",
            "-m",
            message,
        ],
        "corpus movement commit",
    )?;
    fixture_head(root)
}

fn snapshot_state(root: &Path) -> Result<StateSnapshot, String> {
    let head = fixture_head(root)?;
    let mut files = Vec::new();
    for name in CHAIN_FILES {
        let path = root.join(name);
        if !path.is_file() {
            return Err(format!(
                "snapshot requires the produced chain file {} at {}",
                name,
                crate::normalize_path(&path)
            ));
        }
        let bytes =
            fs::read(&path).map_err(|err| format!("read {} for snapshot failed: {err}", name))?;
        files.push(StateFile {
            name: name.to_string(),
            sha256: format!("sha256:{}", hex_lower(&Sha256::digest(&bytes))),
            bytes,
        });
    }
    Ok(StateSnapshot { head, files })
}

/// Restore the snapshotted bytes and repository head, verifying every
/// restored file by digest and the head by exact SHA equality.
fn restore_state(root: &Path, snapshot: &StateSnapshot) -> Result<Vec<String>, String> {
    for file in &snapshot.files {
        let path = root.join(&file.name);
        fs::write(&path, &file.bytes)
            .map_err(|err| format!("restore {} failed: {err}", file.name))?;
        let actual = file_digest_prefixed(&path)?;
        if actual != file.sha256 {
            return Err(format!(
                "restored {} digest {actual} does not match snapshot {}",
                file.name, file.sha256
            ));
        }
    }
    // Remove command outputs the failing run may have created so the control
    // rerun starts from the snapshotted state exactly.
    let _ = fs::remove_file(root.join(CASE_RECEIPT_OUT));
    checkout_fixture_commit(root, &snapshot.head)?;
    let head = fixture_head(root)?;
    if head != snapshot.head {
        return Err(format!(
            "restored head {head} does not match snapshot head {}",
            snapshot.head
        ));
    }
    Ok(vec![format!(
        "restored {} chain files and head {} byte-exactly",
        snapshot.files.len(),
        snapshot.head
    )])
}

fn copy_dir_recursive(source: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination).map_err(|err| {
        format!(
            "create copy destination {} failed: {err}",
            crate::normalize_path(destination)
        )
    })?;
    let entries = fs::read_dir(source).map_err(|err| {
        format!(
            "read copy source {} failed: {err}",
            crate::normalize_path(source)
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("read copy entry failed: {err}"))?;
        let file_type = entry
            .file_type()
            .map_err(|err| format!("read copy entry type failed: {err}"))?;
        let target = destination.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), &target).map_err(|err| {
                format!(
                    "copy {} failed: {err}",
                    crate::normalize_path(&entry.path())
                )
            })?;
        } else {
            return Err(format!(
                "unexpected non-file entry {} in fixture copy",
                crate::normalize_path(&entry.path())
            ));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Case matrix
// ---------------------------------------------------------------------------

fn case_artifact_producer_tool(env: &CaseEnv) -> Result<CaseExecution, String> {
    let mut execution = verify_case_execution(
        env,
        "replace the producer tool marker with a non-ripr tool in both artifacts (commitments re-committed)",
    )?;
    execution.original = pair_digests(env)?;
    for name in [BEFORE_ARTIFACT, AFTER_ARTIFACT] {
        mutate_artifact_value(env.root, name, |value| {
            set_json_string(value, "/artifact/producer/tool", "forged-tool")
        })?;
    }
    execution.mutated = pair_digests(env)?;
    execution.expected = Expectation::reject("invalid or unknown producer identity");
    execution.retain = retain_pair();
    Ok(execution)
}

fn case_artifact_repo_exposure_schema(env: &CaseEnv) -> Result<CaseExecution, String> {
    let mut execution = verify_case_execution(
        env,
        "downgrade the after artifact repo-exposure schema_version 0.3 -> 0.2 (commitment re-committed)",
    )?;
    execution.original = artifact_digests(env.root, &[AFTER_ARTIFACT])?;
    mutate_artifact_value(env.root, AFTER_ARTIFACT, |value| {
        set_json_string(value, "/schema_version", "0.2")
    })?;
    execution.mutated = artifact_digests(env.root, &[AFTER_ARTIFACT])?;
    execution.expected = Expectation::reject("unsupported repo-exposure schema");
    execution.retain = vec![(AFTER_ARTIFACT.to_string(), AFTER_ARTIFACT.to_string())];
    Ok(execution)
}

fn case_artifact_wrong_repository_root(env: &CaseEnv) -> Result<CaseExecution, String> {
    let mut execution = verify_case_execution(
        env,
        "present the unchanged pair at a second equivalent clone root: the portable v2 input identity agrees, so the concrete repository.root equality check is the rejecting gate",
    )?;
    let second_root = env.case_dir.join("workspace-b");
    copy_dir_recursive(env.root, &second_root)?;
    execution.original = pair_digests(env)?;
    execution.mutated = artifact_digests(&second_root, &[BEFORE_ARTIFACT, AFTER_ARTIFACT])?;
    // The token must discriminate the concrete-root rejection ("agent verify
    // <label> artifact repository root <A> does not match <B>") from the
    // snapshot-identity rejection, which also contains "does not match"
    // (#2824 review): `repository root` appears only in the root gate.
    execution.expected = Expectation::reject("repository root");
    execution.run_cwd = Some(second_root.clone());
    execution.extra_cleanup = vec![second_root];
    execution.retain = vec![
        (
            BEFORE_ARTIFACT.to_string(),
            "workspace-b/before.repo-exposure.json".to_string(),
        ),
        (
            AFTER_ARTIFACT.to_string(),
            "workspace-b/after.repo-exposure.json".to_string(),
        ),
    ];
    Ok(execution)
}

fn case_artifact_revision_symbolic(env: &CaseEnv) -> Result<CaseExecution, String> {
    let mut execution = verify_case_execution(
        env,
        "rebind the after artifact repository head to the symbolic revision HEAD~1 (snapshot follows; commitment re-committed)",
    )?;
    execution.original = artifact_digests(env.root, &[AFTER_ARTIFACT])?;
    rebind_artifact_head(env.root, AFTER_ARTIFACT, "HEAD~1")?;
    execution.mutated = artifact_digests(env.root, &[AFTER_ARTIFACT])?;
    execution.expected = Expectation::reject("invalid repository HEAD");
    execution.retain = vec![(AFTER_ARTIFACT.to_string(), AFTER_ARTIFACT.to_string())];
    Ok(execution)
}

fn case_artifact_revision_absent(env: &CaseEnv) -> Result<CaseExecution, String> {
    let mut execution = verify_case_execution(
        env,
        "rebind the after artifact repository head to a well-formed 40-hex revision the checked repository does not hold (snapshot follows; commitment re-committed)",
    )?;
    execution.original = artifact_digests(env.root, &[AFTER_ARTIFACT])?;
    rebind_artifact_head(
        env.root,
        AFTER_ARTIFACT,
        "0123456789abcdef0123456789abcdef01234567",
    )?;
    execution.mutated = artifact_digests(env.root, &[AFTER_ARTIFACT])?;
    execution.expected = Expectation::reject("is not present in the checked repository");
    execution.retain = vec![(AFTER_ARTIFACT.to_string(), AFTER_ARTIFACT.to_string())];
    Ok(execution)
}

fn case_artifact_revision_noncommit(env: &CaseEnv) -> Result<CaseExecution, String> {
    let mut execution = verify_case_execution(
        env,
        "rebind the after artifact repository head to a blob object present in the repository (snapshot follows; commitment re-committed)",
    )?;
    execution.original = artifact_digests(env.root, &[AFTER_ARTIFACT])?;
    let blob = run_fixture_git_command(
        env.root,
        &["rev-parse", "HEAD:Cargo.toml"],
        "resolve fixture blob revision",
    )?;
    let blob = blob.stdout.trim();
    if blob.len() != 40 || !blob.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("fixture blob revision is not a full SHA: {blob}"));
    }
    rebind_artifact_head(env.root, AFTER_ARTIFACT, blob)?;
    execution.mutated = artifact_digests(env.root, &[AFTER_ARTIFACT])?;
    execution.expected = Expectation::reject("not a commit");
    execution.retain = vec![(AFTER_ARTIFACT.to_string(), AFTER_ARTIFACT.to_string())];
    Ok(execution)
}

fn case_artifact_snapshot_wrong_revision(env: &CaseEnv) -> Result<CaseExecution, String> {
    let mut execution = verify_case_execution(
        env,
        "rebind the after artifact snapshot identity to the before revision while the declared head stays at the after revision (commitment re-committed)",
    )?;
    execution.original = artifact_digests(env.root, &[AFTER_ARTIFACT])?;
    let before_sha = env.before_sha.to_string();
    mutate_artifact_value(env.root, AFTER_ARTIFACT, |value| {
        let input = value
            .pointer("/artifact/analysis/input_identity")
            .and_then(Value::as_str)
            .ok_or_else(|| "artifact input identity is missing".to_string())?
            .to_string();
        set_json_string(
            value,
            "/artifact/snapshot_identity",
            &format!("snapshot:{input};revision:{before_sha}"),
        )
    })?;
    execution.mutated = artifact_digests(env.root, &[AFTER_ARTIFACT])?;
    execution.expected = Expectation::reject("snapshot identity does not match");
    execution.retain = vec![(AFTER_ARTIFACT.to_string(), AFTER_ARTIFACT.to_string())];
    Ok(execution)
}

fn case_artifact_commitment_tampered(env: &CaseEnv) -> Result<CaseExecution, String> {
    let mut execution = verify_case_execution(
        env,
        "append one trailing byte to the after artifact WITHOUT re-committing its content commitment",
    )?;
    execution.original = artifact_digests(env.root, &[AFTER_ARTIFACT])?;
    let path = env.root.join(AFTER_ARTIFACT);
    let mut bytes = fs::read(&path).map_err(|err| format!("read after artifact failed: {err}"))?;
    bytes.push(b' ');
    fs::write(&path, &bytes).map_err(|err| format!("tamper after artifact failed: {err}"))?;
    execution.mutated = artifact_digests(env.root, &[AFTER_ARTIFACT])?;
    execution.expected = Expectation::reject("content commitment mismatch");
    execution.retain = vec![(AFTER_ARTIFACT.to_string(), AFTER_ARTIFACT.to_string())];
    Ok(execution)
}

fn case_artifact_commitment_missing(env: &CaseEnv) -> Result<CaseExecution, String> {
    let mut execution = verify_case_execution(
        env,
        "remove the artifact content_sha256 commitment key entirely (no re-commitment possible)",
    )?;
    execution.original = artifact_digests(env.root, &[AFTER_ARTIFACT])?;
    let path = env.root.join(AFTER_ARTIFACT);
    let text = crate::read_text_lossy(&path)?;
    let mut value: Value = serde_json::from_str(&text)
        .map_err(|err| format!("parse after artifact for mutation failed: {err}"))?;
    let Some(artifact) = value.get_mut("artifact").and_then(Value::as_object_mut) else {
        return Err("after artifact envelope is missing".to_string());
    };
    if artifact.remove("content_sha256").is_none() {
        return Err("after artifact content_sha256 key is already absent".to_string());
    }
    let raw = serde_json::to_string_pretty(&value)
        .map_err(|err| format!("render mutated after artifact failed: {err}"))?;
    fs::write(&path, raw).map_err(|err| format!("write mutated after artifact failed: {err}"))?;
    execution.mutated = artifact_digests(env.root, &[AFTER_ARTIFACT])?;
    execution.expected = Expectation::reject(
        "is not a canonical repo-exposure artifact: missing field `content_sha256`",
    );
    execution.retain = vec![(AFTER_ARTIFACT.to_string(), AFTER_ARTIFACT.to_string())];
    Ok(execution)
}

fn case_artifact_commitment_duplicate(env: &CaseEnv) -> Result<CaseExecution, String> {
    let mut execution = verify_case_execution(
        env,
        "insert a second content_sha256 commitment key at the canonical artifact path",
    )?;
    execution.original = artifact_digests(env.root, &[AFTER_ARTIFACT])?;
    let path = env.root.join(AFTER_ARTIFACT);
    let text = crate::read_text_lossy(&path)?;
    let span = content_commitment_value_span(&text)?;
    let insertion_point = span.end + 1;
    let duplicate = ",\n    \"content_sha256\": \"sha256:1111111111111111111111111111111111111111111111111111111111111111\"";
    let mut mutated = text;
    mutated.insert_str(insertion_point, duplicate);
    fs::write(&path, mutated)
        .map_err(|err| format!("write mutated after artifact failed: {err}"))?;
    execution.mutated = artifact_digests(env.root, &[AFTER_ARTIFACT])?;
    execution.expected = Expectation::reject(
        "is not a canonical repo-exposure artifact: duplicate field `content_sha256`",
    );
    execution.retain = vec![(AFTER_ARTIFACT.to_string(), AFTER_ARTIFACT.to_string())];
    Ok(execution)
}

fn case_artifact_commitment_malformed(env: &CaseEnv) -> Result<CaseExecution, String> {
    let mut execution = verify_case_execution(
        env,
        "replace the content_sha256 digest with a non-hex sha256:-prefixed value (no re-commitment)",
    )?;
    execution.original = artifact_digests(env.root, &[AFTER_ARTIFACT])?;
    let path = env.root.join(AFTER_ARTIFACT);
    let value = read_json_value(&path)?;
    let declared = artifact_string(&value, &["artifact", "content_sha256"])?.to_string();
    let malformed = format!("sha256:{}", "z".repeat(64));
    mutate_file_text(env.root, AFTER_ARTIFACT, &declared, &malformed)?;
    execution.mutated = artifact_digests(env.root, &[AFTER_ARTIFACT])?;
    execution.expected = Expectation::reject("must be a sha256:<64 hex> value");
    execution.retain = vec![(AFTER_ARTIFACT.to_string(), AFTER_ARTIFACT.to_string())];
    Ok(execution)
}

fn case_pair_reversed_revisions(env: &CaseEnv) -> Result<CaseExecution, String> {
    let mut execution = verify_case_execution(
        env,
        "present the after artifact as --before and the before artifact as --after (lineage reversal; no byte mutation)",
    )?;
    execution.original = pair_digests(env)?;
    execution.mutated = pair_digests(env)?;
    execution.argv = agent_verify_argv(AFTER_ARTIFACT, BEFORE_ARTIFACT);
    execution.expected = Expectation::reject("revisions are reversed");
    Ok(execution)
}

fn case_pair_unrelated_revisions(env: &CaseEnv) -> Result<CaseExecution, String> {
    let mut execution = verify_case_execution(
        env,
        "rebind the after artifact to a fresh orphan root commit that shares no ancestry with the before revision (snapshot follows; commitment re-committed)",
    )?;
    execution.original = pair_digests(env)?;
    run_fixture_git_command(
        env.root,
        &["checkout", "--quiet", "--orphan", "corpus-unrelated"],
        "create unrelated orphan branch",
    )?;
    run_fixture_git_command(
        env.root,
        &[
            "-c",
            "core.hooksPath=",
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            "corpus unrelated root",
        ],
        "commit unrelated orphan root",
    )?;
    let orphan = fixture_head(env.root)?;
    rebind_artifact_head(env.root, AFTER_ARTIFACT, &orphan)?;
    execution.mutated = artifact_digests(env.root, &[AFTER_ARTIFACT])?;
    execution.expected = Expectation::reject("revisions are unrelated");
    execution.retain = vec![(AFTER_ARTIFACT.to_string(), AFTER_ARTIFACT.to_string())];
    Ok(execution)
}

fn case_pair_incompatible_mode(env: &CaseEnv) -> Result<CaseExecution, String> {
    let mut execution = verify_case_execution(
        env,
        "switch the after artifact analysis mode and bound profile draft -> release (input identity unchanged; commitment re-committed)",
    )?;
    execution.original = artifact_digests(env.root, &[AFTER_ARTIFACT])?;
    mutate_artifact_value(env.root, AFTER_ARTIFACT, |value| {
        set_json_string(value, "/artifact/analysis/mode", "release")?;
        set_json_string(value, "/artifact/analysis/profile", "release")
    })?;
    execution.mutated = artifact_digests(env.root, &[AFTER_ARTIFACT])?;
    execution.expected = Expectation::reject("analysis modes differ");
    execution.retain = vec![(AFTER_ARTIFACT.to_string(), AFTER_ARTIFACT.to_string())];
    Ok(execution)
}

fn case_pair_incompatible_base(env: &CaseEnv) -> Result<CaseExecution, String> {
    let mut execution = verify_case_execution(
        env,
        "set the after artifact base_revision null -> base:corpus (commitment re-committed)",
    )?;
    execution.original = artifact_digests(env.root, &[AFTER_ARTIFACT])?;
    mutate_artifact_value(env.root, AFTER_ARTIFACT, |value| {
        let slot = value
            .pointer_mut("/artifact/analysis/base_revision")
            .ok_or_else(|| "after artifact base_revision slot is missing".to_string())?;
        *slot = Value::String("base:corpus".to_string());
        Ok(())
    })?;
    execution.mutated = artifact_digests(env.root, &[AFTER_ARTIFACT])?;
    execution.expected = Expectation::reject("base revisions differ");
    execution.retain = vec![(AFTER_ARTIFACT.to_string(), AFTER_ARTIFACT.to_string())];
    Ok(execution)
}

fn case_pair_producer_version_drift(env: &CaseEnv) -> Result<CaseExecution, String> {
    let mut execution = verify_case_execution(
        env,
        "shift the after artifact producer version to 0.0.0-corpus (commitment re-committed)",
    )?;
    execution.original = artifact_digests(env.root, &[AFTER_ARTIFACT])?;
    mutate_artifact_value(env.root, AFTER_ARTIFACT, |value| {
        set_json_string(value, "/artifact/producer/version", "0.0.0-corpus")
    })?;
    execution.mutated = artifact_digests(env.root, &[AFTER_ARTIFACT])?;
    execution.expected = Expectation::reject("producer versions differ");
    execution.retain = vec![(AFTER_ARTIFACT.to_string(), AFTER_ARTIFACT.to_string())];
    Ok(execution)
}

fn case_pair_input_identity_drift(env: &CaseEnv) -> Result<CaseExecution, String> {
    let mut execution = verify_case_execution(
        env,
        "shift the final hex digit of the after artifact input identity digest (snapshot follows; commitment re-committed): both artifacts stay individually valid but the pair is incomparable",
    )?;
    execution.original = pair_digests(env)?;
    let after_value = read_json_value(&env.root.join(AFTER_ARTIFACT))?;
    let input =
        artifact_string(&after_value, &["artifact", "analysis", "input_identity"])?.to_string();
    let shifted = shift_final_hex(&input)?;
    if shifted == input {
        return Err("input identity shift must change the identity".to_string());
    }
    mutate_artifact_value(env.root, AFTER_ARTIFACT, |value| {
        let head = value
            .pointer("/artifact/repository/head")
            .and_then(Value::as_str)
            .ok_or_else(|| "after artifact head is missing".to_string())?
            .to_string();
        set_json_string(value, "/artifact/analysis/input_identity", &shifted)?;
        set_json_string(
            value,
            "/artifact/snapshot_identity",
            &format!("snapshot:{shifted};revision:{head}"),
        )
    })?;
    execution.mutated = artifact_digests(env.root, &[AFTER_ARTIFACT])?;
    execution.expected = Expectation::reject("analysis input identities differ");
    execution.retain = vec![(AFTER_ARTIFACT.to_string(), AFTER_ARTIFACT.to_string())];
    Ok(execution)
}

fn case_pair_no_movement_same_clean_revision(env: &CaseEnv) -> Result<CaseExecution, String> {
    let mut execution = verify_case_execution(
        env,
        "check out the before revision (real producer worktree state) and present the same current before artifact as both sides of the pair",
    )?;
    execution.original = artifact_digests(env.root, &[BEFORE_ARTIFACT])?;
    checkout_fixture_commit(env.root, env.before_sha)?;
    execution.mutated = artifact_digests(env.root, &[BEFORE_ARTIFACT])?;
    execution.argv = agent_verify_argv(BEFORE_ARTIFACT, BEFORE_ARTIFACT);
    execution.expected = Expectation::reject("no repository movement");
    Ok(execution)
}

fn case_verify_unsupported_schema(env: &CaseEnv) -> Result<CaseExecution, String> {
    let mut execution = receipt_case_execution(
        env,
        "rewrite the canonical verify document schema_version 0.3 -> 0.2: a canonical-in-every-other-way older document is rejected by the fail-closed schema gate, never migrated",
    )?;
    execution.original = artifact_digests(env.root, &[VERIFY_JSON])?;
    mutate_file_text(
        env.root,
        VERIFY_JSON,
        "\"schema_version\": \"0.3\"",
        "\"schema_version\": \"0.2\"",
    )?;
    execution.mutated = artifact_digests(env.root, &[VERIFY_JSON])?;
    execution.expected = Expectation::reject_without_out("[unsupported_schema]");
    execution.retain = vec![(VERIFY_JSON.to_string(), VERIFY_JSON.to_string())];
    Ok(execution)
}

fn case_verify_replayed_against_another_pair(env: &CaseEnv) -> Result<CaseExecution, String> {
    let mut execution = receipt_case_execution(
        env,
        "advance the repository, produce a third authentic after artifact, and replay the original verify JSON against the new pair bytes",
    )?;
    execution.original = artifact_digests(env.root, &[AFTER_ARTIFACT, VERIFY_JSON])?;
    commit_empty(env.root, "corpus third state")?;
    run_producer_check(env.binary, env.root, THIRD_ARTIFACT)?;
    fs::copy(env.root.join(THIRD_ARTIFACT), env.root.join(AFTER_ARTIFACT))
        .map_err(|err| format!("replay third artifact as after failed: {err}"))?;
    execution.mutated = artifact_digests(env.root, &[AFTER_ARTIFACT, VERIFY_JSON])?;
    execution.expected = Expectation::reject_without_out("[not_canonical]");
    execution.retain = vec![
        (AFTER_ARTIFACT.to_string(), AFTER_ARTIFACT.to_string()),
        (THIRD_ARTIFACT.to_string(), THIRD_ARTIFACT.to_string()),
    ];
    Ok(execution)
}

fn case_verify_artifact_bytes_changed_after_verify(env: &CaseEnv) -> Result<CaseExecution, String> {
    let mut execution = receipt_case_execution(
        env,
        "change after-artifact bytes the movement render never reads (run_status complete -> stalled) and re-commit, then replay the original verify JSON",
    )?;
    execution.original = artifact_digests(env.root, &[AFTER_ARTIFACT])?;
    mutate_artifact_value(env.root, AFTER_ARTIFACT, |value| {
        set_json_string(value, "/run_status", "stalled")
    })?;
    execution.mutated = artifact_digests(env.root, &[AFTER_ARTIFACT])?;
    execution.expected = Expectation::reject_without_out("[not_canonical]");
    execution.retain = vec![(AFTER_ARTIFACT.to_string(), AFTER_ARTIFACT.to_string())];
    Ok(execution)
}

fn case_verify_stale_after_repository_movement(env: &CaseEnv) -> Result<CaseExecution, String> {
    let mut execution = receipt_case_execution(
        env,
        "advance the repository after the verify result was produced, then replay the stale verify JSON; the prior authoritative receipt must stay byte-identical and no new output may appear",
    )?;
    execution.original = artifact_digests(env.root, &[VERIFY_JSON, RECEIPT_JSON])?;
    let prior_receipt_digest = file_digest_prefixed(&env.root.join(RECEIPT_JSON))?;
    commit_empty(env.root, "corpus post-verify movement")?;
    execution.mutated = artifact_digests(env.root, &[VERIFY_JSON, RECEIPT_JSON])?;
    execution.expected = Expectation::Reject {
        token: "[not_canonical]",
        out_absent: Some(CASE_RECEIPT_OUT),
        out_preserved: Some((RECEIPT_JSON.to_string(), prior_receipt_digest)),
    };
    Ok(execution)
}

fn case_verify_tampered_digest(env: &CaseEnv) -> Result<CaseExecution, String> {
    let mut execution = receipt_case_execution(
        env,
        "flip one hex digit of the verify document after_content_sha256 binding",
    )?;
    execution.original = artifact_digests(env.root, &[VERIFY_JSON])?;
    let verify_value = read_json_value(&env.root.join(VERIFY_JSON))?;
    let bound = artifact_string(&verify_value, &["inputs", "after_content_sha256"])?.to_string();
    let shifted = shift_final_hex(&bound)?;
    if shifted == bound {
        return Err("digest flip must change the bound digest".to_string());
    }
    mutate_file_text(env.root, VERIFY_JSON, &bound, &shifted)?;
    execution.mutated = artifact_digests(env.root, &[VERIFY_JSON])?;
    execution.expected = Expectation::reject_without_out("[not_canonical]");
    execution.retain = vec![(VERIFY_JSON.to_string(), VERIFY_JSON.to_string())];
    Ok(execution)
}

fn case_verify_tampered_status(env: &CaseEnv) -> Result<CaseExecution, String> {
    let mut execution = receipt_case_execution(
        env,
        "rewrite the verify document status advisory -> failed: the status field is governed canonical output",
    )?;
    execution.original = artifact_digests(env.root, &[VERIFY_JSON])?;
    mutate_file_text(
        env.root,
        VERIFY_JSON,
        "\"status\": \"advisory\"",
        "\"status\": \"failed\"",
    )?;
    execution.mutated = artifact_digests(env.root, &[VERIFY_JSON])?;
    execution.expected = Expectation::reject_without_out("[not_canonical]");
    execution.retain = vec![(VERIFY_JSON.to_string(), VERIFY_JSON.to_string())];
    Ok(execution)
}

fn case_receipt_input_rerendered_verify_bytes(env: &CaseEnv) -> Result<CaseExecution, String> {
    let mut execution = receipt_case_execution(
        env,
        "re-render the verify document from its parsed value with different byte layout: identical values, different bytes than the exact canonical verify output",
    )?;
    execution.original = artifact_digests(env.root, &[VERIFY_JSON])?;
    let path = env.root.join(VERIFY_JSON);
    let text = crate::read_text_lossy(&path)?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|err| format!("parse verify document for mutation failed: {err}"))?;
    let rerendered = serde_json::to_string(&value)
        .map_err(|err| format!("re-render verify document failed: {err}"))?;
    if rerendered == text {
        return Err("re-rendered verify document must differ byte-wise".to_string());
    }
    fs::write(&path, rerendered)
        .map_err(|err| format!("write re-rendered verify document failed: {err}"))?;
    execution.mutated = artifact_digests(env.root, &[VERIFY_JSON])?;
    execution.expected = Expectation::reject_without_out("[not_canonical]");
    execution.retain = vec![(VERIFY_JSON.to_string(), VERIFY_JSON.to_string())];
    Ok(execution)
}

fn case_receipt_from_incomparable_verification(env: &CaseEnv) -> Result<CaseExecution, String> {
    let mut execution = receipt_case_execution(
        env,
        "retarget the verify document inputs.after at the before artifact and check out the before revision: a no-movement verification cannot issue an authoritative receipt",
    )?;
    execution.original = artifact_digests(env.root, &[VERIFY_JSON])?;
    let verify_value = read_json_value(&env.root.join(VERIFY_JSON))?;
    let before_input = artifact_string(&verify_value, &["inputs", "before"])?.to_string();
    let after_input = artifact_string(&verify_value, &["inputs", "after"])?.to_string();
    mutate_file_text(
        env.root,
        VERIFY_JSON,
        &format!("\"after\": \"{after_input}\""),
        &format!("\"after\": \"{before_input}\""),
    )?;
    checkout_fixture_commit(env.root, env.before_sha)?;
    execution.mutated = artifact_digests(env.root, &[VERIFY_JSON])?;
    execution.expected = Expectation::reject_without_out("[no_movement]");
    execution.retain = vec![(VERIFY_JSON.to_string(), VERIFY_JSON.to_string())];
    Ok(execution)
}

fn case_receipt_target_absent_from_both_states(env: &CaseEnv) -> Result<CaseExecution, String> {
    let mut execution = receipt_case_execution(
        env,
        "request a receipt for a target seam that does not exist in either state (no artifact mutation)",
    )?;
    execution.original = artifact_digests(env.root, &[VERIFY_JSON])?;
    execution.mutated = artifact_digests(env.root, &[VERIFY_JSON])?;
    execution.argv = agent_receipt_argv("corpus-absent-target", CASE_RECEIPT_OUT);
    execution.expected = Expectation::reject_without_out("was not found in agent verify JSON");
    Ok(execution)
}

/// The two-seam honesty fixture texts. The loyalty call is fully qualified:
/// `tests/pricing.rs` imports only `discounted_total`, so an unqualified
/// `loyalty_points(...)` would not compile (E0425) and the fixture would
/// credit a discriminator that cannot execute (#2824 review). Shared by the
/// case and the PR-time compilability test — one code path, one oracle.
const LOYALTY_FN: &str = "\npub fn loyalty_points(total: i32, threshold: i32) -> i32 {\n    if total >= threshold {\n        total / 10\n    } else {\n        0\n    }\n}\n";
const LOYALTY_TEST: &str = "\n#[test]\nfn loyalty_below_threshold_has_no_points() {\n    assert_eq!(boundary_gap_fixture::loyalty_points(50, 100), 0);\n}\n";
const EQUALITY_TEST: &str = "\n#[test]\nfn equality_boundary_discounts() {\n    assert_eq!(discounted_total(100, 100), 90);\n}\n";

/// Build the linear two-seam chain on top of the fixture's before revision:
/// seam X (loyalty_points) keeps one weak discriminator in both states while
/// seam Y (discounted_total) gains the boundary test in the second commit.
/// The chain must be linear — commits on divergent branches share no
/// ancestry and fail the lineage gate instead. Returns (before, after) SHAs.
fn build_two_seam_commits(root: &Path, before_sha: &str) -> Result<(String, String), String> {
    checkout_fixture_commit(root, before_sha)?;
    let lib_path = root.join("src/lib.rs");
    let mut lib = crate::read_text_lossy(&lib_path)?;
    lib.push_str(LOYALTY_FN);
    fs::write(&lib_path, lib).map_err(|err| format!("add loyalty seam failed: {err}"))?;
    let tests_path = root.join("tests/pricing.rs");
    let mut tests = crate::read_text_lossy(&tests_path)?;
    tests.push_str(LOYALTY_TEST);
    fs::write(&tests_path, tests).map_err(|err| format!("add loyalty weak test failed: {err}"))?;
    run_fixture_git_command(
        root,
        &["-c", "core.hooksPath=", "add", "."],
        "stage two-seam before state",
    )?;
    run_fixture_git_command(
        root,
        &[
            "-c",
            "core.hooksPath=",
            "commit",
            "--quiet",
            "-m",
            "corpus two-seam before",
        ],
        "commit two-seam before state",
    )?;
    let before_two_seam = fixture_head(root)?;
    let mut tests = crate::read_text_lossy(&tests_path)?;
    tests.push_str(EQUALITY_TEST);
    fs::write(&tests_path, tests)
        .map_err(|err| format!("add equality boundary test failed: {err}"))?;
    run_fixture_git_command(
        root,
        &["-c", "core.hooksPath=", "add", "."],
        "stage two-seam after state",
    )?;
    run_fixture_git_command(
        root,
        &[
            "-c",
            "core.hooksPath=",
            "commit",
            "--quiet",
            "-m",
            "corpus two-seam after",
        ],
        "commit two-seam after state",
    )?;
    let after_two_seam = fixture_head(root)?;
    if before_two_seam == after_two_seam {
        return Err("two-seam before and after commits are identical".to_string());
    }
    Ok((before_two_seam, after_two_seam))
}

fn case_receipt_unmoved_retained_target(env: &CaseEnv) -> Result<CaseExecution, String> {
    // The receipt for the retained target must issue but stay `unchanged` —
    // movement on seam Y can never strengthen seam X.
    let (before_two_seam, after_two_seam) = build_two_seam_commits(env.root, env.before_sha)?;
    produce_authentic_chain_in_fixture(env.binary, env.root, &before_two_seam, &after_two_seam)?;
    // Grip the producer output itself (#2824 review): the moved seam must be
    // the discounted_total boundary seam this fixture family pins, and the
    // retained target must be the loyalty seam this case constructed — never
    // `unchanged_seams.first()` on faith.
    let verify_value = read_json_value(&env.root.join(VERIFY_JSON))?;
    let changed = verify_value
        .get("changed_seams")
        .and_then(Value::as_array)
        .ok_or_else(|| "verify document changed_seams is missing".to_string())?;
    if changed.len() != 1 {
        return Err(format!(
            "two-seam control failed: expected exactly one moved seam, got {}",
            changed.len()
        ));
    }
    let moved = &changed[0];
    if moved.get("seam_id").and_then(Value::as_str) != Some(BOUNDARY_GAP_SEAM_ID)
        || moved.get("before").and_then(Value::as_str) != Some("weakly_gripped")
        || moved.get("after").and_then(Value::as_str) != Some("strongly_gripped")
    {
        return Err(format!(
            "two-seam control failed: the moved seam is not the discounted_total boundary seam: {moved}"
        ));
    }
    let unchanged = verify_value
        .get("unchanged_seams")
        .and_then(Value::as_array)
        .ok_or_else(|| "verify document unchanged_seams is missing".to_string())?;
    if unchanged.len() != 1 {
        return Err(format!(
            "two-seam control failed: expected exactly one retained seam, got {}",
            unchanged.len()
        ));
    }
    let retained = &unchanged[0];
    if retained.get("seam_id").and_then(Value::as_str) == Some(BOUNDARY_GAP_SEAM_ID)
        || retained.get("file").and_then(Value::as_str) != Some("src/lib.rs")
        || retained.get("before").and_then(Value::as_str) != Some("weakly_gripped")
        || retained.get("after").and_then(Value::as_str) != Some("weakly_gripped")
    {
        return Err(format!(
            "two-seam control failed: the retained seam is not the unchanged loyalty seam: {retained}"
        ));
    }
    let retained_id = retained
        .get("seam_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "retained target seam_id is missing".to_string())?
        .to_string();
    let snapshot = snapshot_state(env.root)?;
    Ok(CaseExecution {
        mutation: "no mutation: honesty pin — receipt for the retained target seam must stay `unchanged` while only the other seam moves".to_string(),
        argv: agent_receipt_argv(&retained_id, RECEIPT_JSON),
        control_argv: agent_receipt_argv(&retained_id, RECEIPT_JSON),
        expected: Expectation::Pass {
            out_file: RECEIPT_JSON,
            pointers: vec![
                ("/seam/change", "unchanged"),
                ("/summary/receipt_state", "receipt_movement_unchanged"),
                ("/provenance/movement", "unchanged"),
            ],
        },
        original: artifact_digests(env.root, &[BEFORE_ARTIFACT, AFTER_ARTIFACT, VERIFY_JSON])?,
        mutated: artifact_digests(env.root, &[BEFORE_ARTIFACT, AFTER_ARTIFACT, VERIFY_JSON])?,
        retain: vec![
            (BEFORE_ARTIFACT.to_string(), BEFORE_ARTIFACT.to_string()),
            (AFTER_ARTIFACT.to_string(), AFTER_ARTIFACT.to_string()),
            (VERIFY_JSON.to_string(), VERIFY_JSON.to_string()),
        ],
        snapshot,
        run_cwd: None,
        extra_cleanup: Vec::new(),
        fixture_shas: Some((before_two_seam, after_two_seam)),
    })
}

fn case_specs() -> Vec<CaseSpec> {
    vec![
        CaseSpec {
            id: "artifact-producer-tool",
            group: "artifact",
            description: "wrong producer tool marker",
            run: case_artifact_producer_tool,
        },
        CaseSpec {
            id: "artifact-repo-exposure-schema",
            group: "artifact",
            description: "unsupported producer/artifact schema",
            run: case_artifact_repo_exposure_schema,
        },
        CaseSpec {
            id: "artifact-wrong-repository-root",
            group: "artifact",
            description: "artifact ported to a second equivalent root (concrete-root check)",
            run: case_artifact_wrong_repository_root,
        },
        CaseSpec {
            id: "artifact-revision-symbolic",
            group: "artifact",
            description: "symbolic (malformed) revision",
            run: case_artifact_revision_symbolic,
        },
        CaseSpec {
            id: "artifact-revision-absent",
            group: "artifact",
            description: "well-formed revision absent from the checked repository",
            run: case_artifact_revision_absent,
        },
        CaseSpec {
            id: "artifact-revision-noncommit",
            group: "artifact",
            description: "revision names a blob, not a commit",
            run: case_artifact_revision_noncommit,
        },
        CaseSpec {
            id: "artifact-snapshot-wrong-revision",
            group: "artifact",
            description: "snapshot identity bound to the wrong revision",
            run: case_artifact_snapshot_wrong_revision,
        },
        CaseSpec {
            id: "artifact-commitment-tampered",
            group: "artifact",
            description: "tampered content commitment (stale digest)",
            run: case_artifact_commitment_tampered,
        },
        CaseSpec {
            id: "artifact-commitment-missing",
            group: "artifact",
            description: "missing content commitment",
            run: case_artifact_commitment_missing,
        },
        CaseSpec {
            id: "artifact-commitment-duplicate",
            group: "artifact",
            description: "duplicate content commitment",
            run: case_artifact_commitment_duplicate,
        },
        CaseSpec {
            id: "artifact-commitment-malformed",
            group: "artifact",
            description: "malformed content commitment digest shape",
            run: case_artifact_commitment_malformed,
        },
        CaseSpec {
            id: "pair-reversed-revisions",
            group: "pair",
            description: "reversed revisions",
            run: case_pair_reversed_revisions,
        },
        CaseSpec {
            id: "pair-unrelated-revisions",
            group: "pair",
            description: "unrelated revisions",
            run: case_pair_unrelated_revisions,
        },
        CaseSpec {
            id: "pair-incompatible-mode",
            group: "pair",
            description: "incompatible analysis mode/profile",
            run: case_pair_incompatible_mode,
        },
        CaseSpec {
            id: "pair-incompatible-base",
            group: "pair",
            description: "incompatible base revision configuration",
            run: case_pair_incompatible_base,
        },
        CaseSpec {
            id: "pair-producer-version-drift",
            group: "pair",
            description: "producer version drift across the pair",
            run: case_pair_producer_version_drift,
        },
        CaseSpec {
            id: "pair-input-identity-drift",
            group: "pair",
            description: "unexpected input-identity drift (individually valid but incomparable pair)",
            run: case_pair_input_identity_drift,
        },
        CaseSpec {
            id: "pair-no-movement-same-clean-revision",
            group: "pair",
            description: "same clean revision with no actual target movement (pinned producer worktree state)",
            run: case_pair_no_movement_same_clean_revision,
        },
        CaseSpec {
            id: "verify-unsupported-schema",
            group: "verify_receipt",
            description: "canonical 0.2 verify document rejected by the 0.3 schema gate",
            run: case_verify_unsupported_schema,
        },
        CaseSpec {
            id: "verify-replayed-against-another-pair",
            group: "verify_receipt",
            description: "verify replayed against another pair",
            run: case_verify_replayed_against_another_pair,
        },
        CaseSpec {
            id: "verify-artifact-bytes-changed-after-verify",
            group: "verify_receipt",
            description: "artifact bytes changed after verify",
            run: case_verify_artifact_bytes_changed_after_verify,
        },
        CaseSpec {
            id: "verify-stale-after-repository-movement",
            group: "verify_receipt",
            description: "stale verify after repository movement; failed issuance leaves no output and preserves the prior receipt",
            run: case_verify_stale_after_repository_movement,
        },
        CaseSpec {
            id: "verify-tampered-digest",
            group: "verify_receipt",
            description: "tampered verify digest binding",
            run: case_verify_tampered_digest,
        },
        CaseSpec {
            id: "verify-tampered-status",
            group: "verify_receipt",
            description: "tampered verify status",
            run: case_verify_tampered_status,
        },
        CaseSpec {
            id: "receipt-input-rerendered-verify-bytes",
            group: "verify_receipt",
            description: "receipt input differing from the exact canonical verify bytes",
            run: case_receipt_input_rerendered_verify_bytes,
        },
        CaseSpec {
            id: "receipt-from-incomparable-verification",
            group: "verify_receipt",
            description: "no-movement verification attempting to issue an authoritative receipt",
            run: case_receipt_from_incomparable_verification,
        },
        CaseSpec {
            id: "receipt-target-absent-from-both-states",
            group: "verify_receipt",
            description: "target absent from both states",
            run: case_receipt_target_absent_from_both_states,
        },
        CaseSpec {
            id: "receipt-unmoved-retained-target",
            group: "verify_receipt",
            description: "another target moves while the retained target does not (honesty pin)",
            run: case_receipt_unmoved_retained_target,
        },
    ]
}

/// Deferred negatives with a recorded disposition (#2921 claim boundary): no
/// migration producer and no binary/artifact-inventory producer exists on
/// main, so these conditions have no real production path to validate
/// against. They are recorded, not fabricated.
fn deferred_dispositions() -> Vec<(String, String)> {
    vec![
        (
            "migration-claims-fresh-production".to_string(),
            "not_applicable: no migration producer exists on main; deferred until a real migration producer makes the condition producible".to_string(),
        ),
        (
            "binary-artifact-inventory-disagreement".to_string(),
            "not_applicable: no inventory-bearing producer exists on main; deferred until a real inventory producer makes the condition producible".to_string(),
        ),
    ]
}

// ---------------------------------------------------------------------------
// Reporting
// ---------------------------------------------------------------------------

fn write_case_receipt(case_dir: &Path, receipt: &CaseReceipt) -> Result<(), String> {
    let body = serde_json::to_string_pretty(&case_receipt_json(receipt))
        .map_err(|err| format!("render case receipt failed: {err}"))?;
    let path = case_dir.join("receipt.json");
    fs::write(&path, body)
        .map_err(|err| format!("write {} failed: {err}", crate::normalize_path(&path)))
}

fn artifact_digest_json(record: &ArtifactDigestRecord) -> Value {
    json!({
        "name": record.name,
        "sha256": record.sha256,
        "input_identity": record.input_identity,
        "snapshot_identity": record.snapshot_identity,
    })
}

fn case_receipt_json(receipt: &CaseReceipt) -> Value {
    json!({
        "case_id": receipt.case_id,
        "group": receipt.group,
        "description": receipt.description,
        "mutation": receipt.mutation,
        "candidate": {
            "path": receipt.candidate_path,
            "version": receipt.candidate_version,
            "sha256": receipt.candidate_sha256,
        },
        "fixture": {
            "root": receipt.fixture_root,
            "before_sha": receipt.before_sha,
            "after_sha": receipt.after_sha,
        },
        "argv": receipt.argv,
        "control_argv": receipt.control_argv,
        "original_artifacts": receipt.original_artifacts.iter().map(artifact_digest_json).collect::<Vec<_>>(),
        "mutated_artifacts": receipt.mutated_artifacts.iter().map(artifact_digest_json).collect::<Vec<_>>(),
        "expected_failure_kind": receipt.expected_failure_kind,
        "actual_failure_kind": receipt.actual_failure_kind,
        "exit_status": receipt.exit_status,
        "process_outcome": receipt.process_outcome,
        "restoration_outcome": receipt.restoration_outcome,
        "control_outcome": receipt.control_outcome,
        "cleanup_outcome": receipt.cleanup_outcome,
        "status": receipt.status,
        "violations": receipt.violations,
        "details": receipt.details,
    })
}

fn negative_corpus_json(report: &NegativeCorpusReport) -> Result<String, String> {
    let passed = report
        .cases
        .iter()
        .filter(|case| case.status == "pass")
        .count();
    let value = json!({
        "report": "release-negative-corpus",
        "schema_version": "1",
        "version": report.version,
        "status": report.status,
        "candidate": {
            "path": report.candidate.path,
            "version": report.candidate.version_output,
            "sha256": report.candidate.sha256,
        },
        "baseline": {
            "fixture_root": report.baseline_root,
            "before_sha": report.baseline_before_sha,
            "after_sha": report.baseline_after_sha,
            "artifacts": report.baseline_artifacts.iter().map(artifact_digest_json).collect::<Vec<_>>(),
            "retained_under": "target/ripr/release-negative-corpus/baseline",
            "cleanup": report.baseline_cleanup,
        },
        "cases": report.cases.iter().map(case_receipt_json).collect::<Vec<_>>(),
        "dispositions": report
            .dispositions
            .iter()
            .map(|(case_id, disposition)| json!({ "case_id": case_id, "disposition": disposition }))
            .collect::<Vec<_>>(),
        "summary": {
            "run_status": report.run_status,
            "covered_families": report.covered_families,
            "deferred_families": report.deferred_families,
            "total_cases": report.cases.len(),
            "passed": passed,
            "failed": report.cases.len() - passed,
            "cases_with_violations": report
                .cases
                .iter()
                .filter(|case| !case.violations.is_empty())
                .count(),
            "not_applicable": report.dispositions.len(),
        },
    });
    serde_json::to_string_pretty(&value)
        .map_err(|err| format!("render release-negative-corpus JSON failed: {err}"))
}

fn negative_corpus_markdown(report: &NegativeCorpusReport) -> String {
    let mut body = String::new();
    body.push_str("# release-negative-corpus\n\n");
    body.push_str(&format!("Status: {}\n\n", report.status));
    body.push_str(&format!(
        "Run status: `{}` — covered families: {}; deferred families: {}\n\n",
        report.run_status,
        report.covered_families.join(", "),
        if report.deferred_families.is_empty() {
            "none".to_string()
        } else {
            report.deferred_families.join(", ")
        }
    ));
    body.push_str(&format!("Version: {}\n\n", report.version));
    body.push_str("Integrated negative corpus for the release-readiness artifact/pair/verify/receipt authority chain (#2824). The installed candidate binary is the only validator; every case asserts exit status first, then the closed reason token, then byte-exact restoration and a passing control rerun.\n\n");
    body.push_str("## Candidate\n\n");
    body.push_str(&format!("- path: `{}`\n", report.candidate.path));
    body.push_str(&format!(
        "- version: `{}`\n",
        report.candidate.version_output
    ));
    body.push_str(&format!("- sha256: `{}`\n\n", report.candidate.sha256));
    body.push_str("## Baseline (immutable positive)\n\n");
    body.push_str(&format!("- fixture root: `{}`\n", report.baseline_root));
    body.push_str(&format!("- before SHA: `{}`\n", report.baseline_before_sha));
    body.push_str(&format!("- after SHA: `{}`\n", report.baseline_after_sha));
    body.push_str(&format!("- fixture cleanup: {}\n", report.baseline_cleanup));
    body.push_str("- retained artifacts (target/ripr/release-negative-corpus/baseline):\n");
    for artifact in &report.baseline_artifacts {
        body.push_str(&format!("  - `{}` `{}`\n", artifact.name, artifact.sha256));
    }
    body.push('\n');
    body.push_str("## Case matrix\n\n");
    body.push_str("| case | group | expected failure kind | actual failure kind | exit | process | restoration | control | violations | status |\n");
    body.push_str("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n");
    for case in &report.cases {
        body.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            case.case_id,
            case.group,
            md_escape_inline(&case.expected_failure_kind),
            md_escape_inline(&case.actual_failure_kind),
            case.exit_status
                .map(|status| status.to_string())
                .unwrap_or_else(|| "n/a".to_string()),
            case.process_outcome,
            md_escape_inline(&case.restoration_outcome),
            md_escape_inline(&case.control_outcome),
            case.violations.len(),
            case.status
        ));
    }
    body.push('\n');
    body.push_str("## Violations\n\n");
    if report.cases.iter().all(|case| case.violations.is_empty()) {
        body.push_str("None recorded — every case is clean.\n");
    } else {
        for case in report
            .cases
            .iter()
            .filter(|case| !case.violations.is_empty())
        {
            for violation in &case.violations {
                body.push_str(&format!("- `{}`: {violation}\n", case.case_id));
            }
        }
    }
    body.push('\n');
    body.push_str("## Deferred dispositions (not fabricated)\n\n");
    for (case_id, disposition) in &report.dispositions {
        body.push_str(&format!("- `{case_id}`: {disposition}\n"));
    }
    body.push_str("\nCase receipts and mutated evidence are retained under target/ripr/release-negative-corpus/cases/<case_id>/.\n");
    body
}

fn md_escape_inline(value: &str) -> String {
    value.replace('|', "\\|")
}

// ---------------------------------------------------------------------------
// Arguments
// ---------------------------------------------------------------------------

fn parse_release_negative_args(args: &[String]) -> Result<ReleaseNegativeArgs, String> {
    let mut version: Option<String> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--version" => {
                let Some(value) = args.get(index + 1) else {
                    return Err(release_negative_usage());
                };
                version = Some(value.clone());
                index += 2;
            }
            "--help" | "-h" => return Err(release_negative_usage()),
            other => {
                return Err(format!(
                    "unknown release-negative-corpus argument {other:?}\n{}",
                    release_negative_usage()
                ));
            }
        }
    }
    let Some(version) = version else {
        return Err(release_negative_usage());
    };
    if version.trim().is_empty() {
        return Err(release_negative_usage());
    }
    Ok(ReleaseNegativeArgs { version })
}

fn release_negative_usage() -> String {
    "Usage: cargo xtask release-negative-corpus --version <version>".to_string()
}

/// Run the fixture's own test suite from inside a running `cargo test`
/// process without inheriting the parent's build state (#2824 review):
/// `CARGO_TARGET_DIR` lives inside the fixture root so the nested build
/// never shares the parent's lock region or CARGO_* settings, and
/// `--offline` keeps a network failure from ever reading as fixture evidence
/// (the fixture has no dependencies). The outcome is classified before any
/// fixture conclusion, mirroring the repo's infra-vs-real-failure doctrine:
/// a spawn failure or lock/busy contention is an infrastructure error — a
/// persistent lock error gets exactly one bounded retry and is still
/// reported as infrastructure — and only a genuine non-zero cargo exit
/// (compile or test diagnostic) is fixture evidence.
#[cfg(test)]
fn run_nested_fixture_cargo_test(root: &Path) -> Result<CommandResult, String> {
    let target_dir = root.join("target").join("nested-cargo");
    let target = target_dir.to_string_lossy().into_owned();
    let args = vec!["test".to_string(), "--offline".to_string()];
    let envs = [("CARGO_TARGET_DIR", target.as_str())];
    let spawn = |attempt: &str| {
        crate::run::capture_output_in_dir_with_envs(
            "cargo",
            &args,
            root,
            "two-seam fixture cargo test",
            &envs,
            &[],
        )
        .map_err(|err| format!("infrastructure error: {attempt} nested cargo spawn failed: {err}"))
        .map(|output| CommandResult {
            status: output.status.code(),
            success: output.status.success(),
            stdout: output.stdout,
            stderr: output.stderr,
        })
    };
    let output = spawn("initial")?;
    if !output.success && nested_cargo_lock_contention(&output.stderr) {
        let retry = spawn("retry")?;
        if !retry.success && nested_cargo_lock_contention(&retry.stderr) {
            return Err(format!(
                "infrastructure error: nested cargo lock contention persisted after one retry: {}",
                retry.stderr.trim().lines().next().unwrap_or("<no stderr>")
            ));
        }
        return Ok(retry);
    }
    Ok(output)
}

#[cfg(test)]
fn nested_cargo_lock_contention(stderr: &str) -> bool {
    stderr.contains("Blocking waiting for file lock")
        || stderr.contains("Text file busy")
        || stderr.contains("os error 26")
        || stderr.contains("resource temporarily unavailable")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    fn command_result(success: bool, stdout: &str, stderr: &str) -> CommandResult {
        CommandResult {
            status: Some(if success { 0 } else { 1 }),
            success,
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
        }
    }

    #[test]
    fn release_negative_args_parse_version() -> Result<(), String> {
        let parsed = parse_release_negative_args(&args(&["--version", "0.10.0"]))?;
        if parsed.version != "0.10.0" {
            return Err(format!("unexpected version {}", parsed.version));
        }
        Ok(())
    }

    #[test]
    fn release_negative_args_require_version() -> Result<(), String> {
        for argv in [
            args(&[]),
            args(&["--version"]),
            args(&["--version", "  "]),
            args(&["--bogus"]),
        ] {
            if parse_release_negative_args(&argv).is_ok() {
                return Err(format!("argv {argv:?} must be rejected"));
            }
        }
        Ok(())
    }

    fn sample_artifact() -> String {
        format!(
            r#"{{"schema_version":"0.3","artifact":{{"content_sha256":"{CONTENT_PLACEHOLDER}"}},"scope":"repo"}}"#
        )
    }

    #[test]
    fn recommit_binds_mutated_bytes_and_detects_stale_commitment() -> Result<(), String> {
        let committed = recommit_repo_exposure_bytes(&sample_artifact())?;
        if committed.contains(CONTENT_PLACEHOLDER) {
            return Err("recommit left the placeholder in the artifact".to_string());
        }
        let declared = recompute_content_commitment(&committed)?;
        if !committed.contains(&declared) {
            return Err(
                "recommitted artifact does not contain its recomputed commitment".to_string(),
            );
        }
        // A later byte change without re-commitment makes the declared
        // commitment stale — the mechanism the tampered-commitment case
        // relies on the installed binary to reject.
        let mut tampered = committed;
        tampered.push(' ');
        let recomputed = recompute_content_commitment(&tampered)?;
        if recomputed == declared {
            return Err("stale commitment was not detected after a byte change".to_string());
        }
        Ok(())
    }

    #[test]
    fn recommit_rejects_missing_or_duplicate_commitment_keys() -> Result<(), String> {
        if recommit_repo_exposure_bytes(r#"{"artifact":{}}"#).is_ok() {
            return Err("recommit accepted a missing commitment key".to_string());
        }
        let duplicate = format!(
            r#"{{"artifact":{{"content_sha256":"{CONTENT_PLACEHOLDER}","content_sha256":"{CONTENT_PLACEHOLDER}"}}}}"#
        );
        if recommit_repo_exposure_bytes(&duplicate).is_ok() {
            return Err("recommit accepted a duplicate commitment key".to_string());
        }
        Ok(())
    }

    #[test]
    fn replace_once_requires_a_unique_anchor() -> Result<(), String> {
        if replace_once("nothing to find here", "anchor", "x").is_ok() {
            return Err("replace_once accepted a missing anchor".to_string());
        }
        // The whole-string "anchor anchor" contains the anchor twice.
        if replace_once("anchor anchor", "anchor", "x").is_ok() {
            return Err("replace_once accepted a repeated anchor".to_string());
        }
        let replaced = replace_once("one anchor only", "anchor", "x")?;
        if replaced != "one x only" {
            return Err(format!("unexpected replacement: {replaced}"));
        }
        Ok(())
    }

    #[test]
    fn shift_final_hex_stays_lowercase_hex_and_changes_value() -> Result<(), String> {
        for (input, expected) in [("abc0", "abc1"), ("abc9", "abca"), ("abcf", "abc0")] {
            let shifted = shift_final_hex(input)?;
            if shifted != expected {
                return Err(format!(
                    "shift_final_hex({input}) = {shifted}, want {expected}"
                ));
            }
        }
        if shift_final_hex("").is_ok() {
            return Err("shift_final_hex accepted an empty identity".to_string());
        }
        if shift_final_hex("abcZ").is_ok() {
            return Err("shift_final_hex accepted a non-hex tail".to_string());
        }
        Ok(())
    }

    #[test]
    fn rejection_evaluation_checks_exit_status_before_the_token() -> Result<(), String> {
        // A passing command is a wrong outcome even when the token appears.
        let (outcome, _, _) = evaluate_rejection(
            "[not_canonical]",
            &command_result(true, "", "error: [not_canonical]"),
        );
        if outcome != "unexpected_pass" {
            return Err(format!("unexpected outcome for passing command: {outcome}"));
        }
        // A failing command with a different token is a wrong failure kind.
        let (outcome, actual, _) = evaluate_rejection(
            "[not_canonical]",
            &command_result(false, "", "error: [unsupported_schema]"),
        );
        if outcome != "wrong_failure_kind" {
            return Err(format!("unexpected outcome for wrong token: {outcome}"));
        }
        if !actual.contains("[unsupported_schema]") {
            return Err(format!("actual failure kind was not captured: {actual}"));
        }
        // Exit status first, then the closed token: this is the only pass.
        let (outcome, actual, violations) = evaluate_rejection(
            "[not_canonical]",
            &command_result(
                false,
                "",
                "error: agent receipt verify input [not_canonical]: ...",
            ),
        );
        if outcome != "rejected_as_expected" || actual != "[not_canonical]" {
            return Err(format!("unexpected outcome: {outcome} / {actual}"));
        }
        if !violations.is_empty() {
            return Err(format!("unexpected violations: {violations:?}"));
        }
        // Stdout on a rejected command violates the output contract.
        let (_, _, violations) = evaluate_rejection(
            "[x]",
            &command_result(false, "partial output", "error: [x]"),
        );
        if violations.is_empty() {
            return Err("stdout on a rejected command must be flagged".to_string());
        }
        Ok(())
    }

    #[test]
    fn rejection_evaluation_discriminates_repository_root_from_snapshot_mismatch()
    -> Result<(), String> {
        // The wrong-repository-root case token must not match the
        // snapshot-identity rejection, which also contains "does not match"
        // (#2824 review): fabricated stderr with the snapshot rejection must
        // evaluate to wrong_failure_kind for the root token.
        let snapshot_rejection = "error: agent verify after artifact snapshot identity does not match the declared analysis input identity and repository head";
        if !snapshot_rejection.contains("does not match") {
            return Err("test premise drifted: snapshot rejection lost does-not-match".to_string());
        }
        let (outcome, actual, _) = evaluate_rejection(
            "repository root",
            &command_result(false, "", snapshot_rejection),
        );
        if outcome != "wrong_failure_kind" {
            return Err(format!(
                "snapshot rejection must not satisfy the root token: {outcome} / {actual}"
            ));
        }
        // The real root rejection satisfies it.
        let root_rejection =
            "error: agent verify before artifact repository root /clone-b does not match /clone-a";
        let (outcome, actual, violations) = evaluate_rejection(
            "repository root",
            &command_result(false, "", root_rejection),
        );
        if outcome != "rejected_as_expected" || actual != "repository root" {
            return Err(format!(
                "root rejection must satisfy the root token: {outcome} / {actual}"
            ));
        }
        if !violations.is_empty() {
            return Err(format!("unexpected violations: {violations:?}"));
        }
        Ok(())
    }

    #[test]
    fn case_receipt_finalize_requires_the_full_chain() -> Result<(), String> {
        let spec = CaseSpec {
            id: "case",
            group: "artifact",
            description: "case",
            run: case_artifact_producer_tool,
        };
        let candidate = CandidateIdentity {
            path: "p".to_string(),
            version_output: "ripr 0.10.0".to_string(),
            sha256: "sha256:0".to_string(),
        };
        let baseline = BaselineContext {
            fixture_root: PathBuf::from("fixture"),
            before_sha: "b".to_string(),
            after_sha: "a".to_string(),
            artifacts: Vec::new(),
        };
        let mut receipt = CaseReceipt::new(&spec, &candidate, &baseline);
        receipt.process_outcome = "rejected_as_expected".to_string();
        receipt.restoration_outcome = "restored_byte_exact".to_string();
        receipt.control_outcome = "pass".to_string();
        receipt.cleanup_outcome = "removed".to_string();
        receipt.finalize();
        if receipt.status != "pass" {
            return Err("a complete chain must pass".to_string());
        }
        receipt.control_outcome = "fail".to_string();
        receipt.finalize();
        if receipt.status != "fail" {
            return Err("a failed control rerun must fail the case".to_string());
        }
        // A recorded violation fails the case even when every outcome token
        // matched: violations are first-class, never informational (#2824
        // review — finalize must consult them).
        receipt.control_outcome = "pass".to_string();
        receipt
            .violations
            .push("a rejected command must render nothing to stdout".to_string());
        receipt.finalize();
        if receipt.status != "fail" {
            return Err("a recorded violation must fail the case".to_string());
        }
        receipt.violations.clear();
        receipt.finalize();
        if receipt.status != "pass" {
            return Err("clearing violations must restore the pass".to_string());
        }
        Ok(())
    }

    #[test]
    fn case_receipt_json_carries_the_required_receipt_fields() -> Result<(), String> {
        let spec = CaseSpec {
            id: "case",
            group: "verify_receipt",
            description: "case",
            run: case_verify_tampered_status,
        };
        let candidate = CandidateIdentity {
            path: "p".to_string(),
            version_output: "ripr 0.10.0".to_string(),
            sha256: "sha256:0".to_string(),
        };
        let baseline = BaselineContext {
            fixture_root: PathBuf::from("fixture"),
            before_sha: "b".to_string(),
            after_sha: "a".to_string(),
            artifacts: Vec::new(),
        };
        let receipt = CaseReceipt::new(&spec, &candidate, &baseline);
        let value = case_receipt_json(&receipt);
        for key in [
            "case_id",
            "group",
            "description",
            "mutation",
            "candidate",
            "fixture",
            "argv",
            "control_argv",
            "original_artifacts",
            "mutated_artifacts",
            "expected_failure_kind",
            "actual_failure_kind",
            "exit_status",
            "process_outcome",
            "restoration_outcome",
            "control_outcome",
            "cleanup_outcome",
            "status",
            "violations",
        ] {
            if value.get(key).is_none() {
                return Err(format!("case receipt JSON is missing {key}"));
            }
        }
        for key in ["path", "version", "sha256"] {
            if value["candidate"].get(key).is_none() {
                return Err(format!("case receipt candidate is missing {key}"));
            }
        }
        for key in ["root", "before_sha", "after_sha"] {
            if value["fixture"].get(key).is_none() {
                return Err(format!("case receipt fixture is missing {key}"));
            }
        }
        Ok(())
    }

    #[test]
    fn corpus_covers_every_required_case_once() -> Result<(), String> {
        let specs = case_specs();
        let mut ids = std::collections::BTreeSet::new();
        for spec in &specs {
            if !ids.insert(spec.id) {
                return Err(format!("duplicate case id {}", spec.id));
            }
        }
        let required = [
            "artifact-producer-tool",
            "artifact-repo-exposure-schema",
            "artifact-wrong-repository-root",
            "artifact-revision-symbolic",
            "artifact-revision-absent",
            "artifact-revision-noncommit",
            "artifact-snapshot-wrong-revision",
            "artifact-commitment-tampered",
            "artifact-commitment-missing",
            "artifact-commitment-duplicate",
            "artifact-commitment-malformed",
            "pair-reversed-revisions",
            "pair-unrelated-revisions",
            "pair-incompatible-mode",
            "pair-incompatible-base",
            "pair-producer-version-drift",
            "pair-input-identity-drift",
            "pair-no-movement-same-clean-revision",
            "verify-unsupported-schema",
            "verify-replayed-against-another-pair",
            "verify-artifact-bytes-changed-after-verify",
            "verify-stale-after-repository-movement",
            "verify-tampered-digest",
            "verify-tampered-status",
            "receipt-input-rerendered-verify-bytes",
            "receipt-from-incomparable-verification",
            "receipt-target-absent-from-both-states",
            "receipt-unmoved-retained-target",
        ];
        for id in required {
            if !ids.contains(id) {
                return Err(format!("required case {id} is missing from the corpus"));
            }
        }
        if specs.len() != required.len() {
            return Err(format!(
                "corpus has {} cases; the required matrix has {}",
                specs.len(),
                required.len()
            ));
        }
        Ok(())
    }

    #[test]
    fn family_coverage_discloses_the_deferred_families() -> Result<(), String> {
        let (covered, deferred) = family_coverage();
        let expected_covered = vec![
            "artifact".to_string(),
            "pair".to_string(),
            "verify".to_string(),
            "receipt".to_string(),
        ];
        if covered != expected_covered {
            return Err(format!(
                "the full matrix must cover every family: {covered:?}"
            ));
        }
        if !deferred.is_empty() {
            return Err(format!(
                "the full matrix must defer no family: {deferred:?}"
            ));
        }
        Ok(())
    }

    fn snapshot_fixture_root(label: &str) -> Result<PathBuf, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("read clock failed: {err}"))?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "ripr-negative-corpus-restore-{label}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&root).map_err(|err| format!("create restore fixture failed: {err}"))?;
        run_fixture_git_command(&root, &["init", "--quiet", "--template="], "initialize")?;
        run_fixture_git_command(
            &root,
            &["config", "user.name", "RIPR Corpus Test"],
            "user name",
        )?;
        run_fixture_git_command(
            &root,
            &["config", "user.email", "corpus-test@example.invalid"],
            "user email",
        )?;
        run_fixture_git_command(&root, &["config", "commit.gpgSign", "false"], "signing")?;
        fs::write(root.join("marker.txt"), "fixture\n")
            .map_err(|err| format!("write restore fixture marker failed: {err}"))?;
        run_fixture_git_command(&root, &["-c", "core.hooksPath=", "add", "."], "stage")?;
        run_fixture_git_command(
            &root,
            &[
                "-c",
                "core.hooksPath=",
                "commit",
                "--quiet",
                "-m",
                "fixture",
            ],
            "commit",
        )?;
        Ok(root)
    }

    fn write_chain_files(root: &Path, marker: &str) -> Result<(), String> {
        for name in CHAIN_FILES {
            fs::write(root.join(name), format!("{marker}:{name}\n"))
                .map_err(|err| format!("write chain file {name} failed: {err}"))?;
        }
        Ok(())
    }

    #[test]
    fn restore_state_recovers_bytes_and_head_after_mutation() -> Result<(), String> {
        let root = snapshot_fixture_root("roundtrip")?;
        let result = (|| -> Result<(), String> {
            write_chain_files(&root, "original")?;
            let snapshot = snapshot_state(&root)?;
            write_chain_files(&root, "mutated")?;
            restore_state(&root, &snapshot)?;
            for name in CHAIN_FILES {
                let restored = crate::read_text_lossy(&root.join(name))?;
                if restored != format!("original:{name}\n") {
                    return Err(format!("restored bytes drifted for {name}"));
                }
            }
            if fixture_head(&root)? != snapshot.head {
                return Err("restored head does not match the snapshot head".to_string());
            }
            Ok(())
        })();
        let cleanup = fs::remove_dir_all(&root);
        result?;
        cleanup.map_err(|err| format!("remove restore fixture failed: {err}"))?;
        Ok(())
    }

    #[test]
    fn restore_state_rejects_digest_mismatch_and_unreturnable_head() -> Result<(), String> {
        let root = snapshot_fixture_root("reject")?;
        let result = (|| -> Result<(), String> {
            write_chain_files(&root, "original")?;
            let snapshot = snapshot_state(&root)?;
            // A snapshot whose recorded digest does not match its own bytes
            // must fail the digest verification, not restore silently.
            let mut corrupted = snapshot.clone();
            for file in &mut corrupted.files {
                file.sha256 =
                    "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                        .to_string();
            }
            if restore_state(&root, &corrupted).is_ok() {
                return Err("a digest-mismatched snapshot must be rejected".to_string());
            }
            // A snapshot head the repository cannot return to must fail the
            // restoration, not leave the workspace on a drifted revision.
            let unreturnable = StateSnapshot {
                head: "0123456789abcdef0123456789abcdef01234567".to_string(),
                files: Vec::new(),
            };
            if restore_state(&root, &unreturnable).is_ok() {
                return Err("an unreturnable snapshot head must be rejected".to_string());
            }
            Ok(())
        })();
        let cleanup = fs::remove_dir_all(&root);
        result?;
        cleanup.map_err(|err| format!("remove restore fixture failed: {err}"))?;
        Ok(())
    }

    #[test]
    fn deferred_dispositions_record_both_unproducible_negatives() -> Result<(), String> {
        let dispositions = deferred_dispositions();
        if dispositions.len() != 2 {
            return Err(format!(
                "expected two deferred dispositions, got {}",
                dispositions.len()
            ));
        }
        for (case_id, disposition) in &dispositions {
            if !disposition.starts_with("not_applicable:") {
                return Err(format!(
                    "disposition for {case_id} must be an explicit not_applicable record"
                ));
            }
        }
        let ids = dispositions
            .iter()
            .map(|(id, _)| id.as_str())
            .collect::<Vec<_>>();
        for required in [
            "migration-claims-fresh-production",
            "binary-artifact-inventory-disagreement",
        ] {
            if !ids.contains(&required) {
                return Err(format!("deferred disposition {required} is missing"));
            }
        }
        Ok(())
    }

    #[test]
    fn copy_dir_recursive_rejects_symlink_entries() -> Result<(), String> {
        // Platform disclosure (#2824 review): the symlink-rejection proof is
        // `#[cfg(unix)]` — on Windows this test proves regular-file copying
        // only, and the fail-closed symlink rejection carries no
        // cross-platform evidence. Do not read a green Windows run as
        // covering the symlink path.
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("read clock failed: {err}"))?
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "ripr-negative-corpus-copy-{}-{stamp}",
            std::process::id()
        ));
        let source = base.join("source");
        let result = (|| -> Result<(), String> {
            fs::create_dir_all(&source).map_err(|err| format!("create source failed: {err}"))?;
            fs::write(source.join("file.txt"), "content")
                .map_err(|err| format!("write source file failed: {err}"))?;
            let destination = base.join("destination");
            copy_dir_recursive(&source, &destination)?;
            let copied = crate::read_text_lossy(&destination.join("file.txt"))?;
            if copied != "content" {
                return Err("copied content drifted".to_string());
            }
            #[cfg(unix)]
            {
                // Unix-only evidence: the fail-closed symlink rejection is
                // exercised here; other platforms exercise only the
                // regular-file copy above.
                std::os::unix::fs::symlink(source.join("file.txt"), source.join("link.txt"))
                    .map_err(|err| format!("create symlink failed: {err}"))?;
                if copy_dir_recursive(&source, &base.join("destination-b")).is_ok() {
                    return Err("symlink entries must be rejected fail-closed".to_string());
                }
            }
            Ok(())
        })();
        let cleanup = fs::remove_dir_all(&base);
        result?;
        cleanup.map_err(|err| format!("remove copy test dir failed: {err}"))?;
        Ok(())
    }

    #[test]
    fn evaluate_pass_requires_a_pinned_projection() -> Result<(), String> {
        // A pass expectation with no pinned projection pointers must never
        // evaluate to a vacuous pass (#2824 review): empty pointers plus a
        // successful command is an output-contract violation, not a pass.
        let (outcome, violations) = evaluate_pass(
            Path::new("."),
            "unused.json",
            &[],
            &command_result(true, "", ""),
        );
        if outcome == "passed_with_pinned_projection" {
            return Err("an empty pass expectation must not produce a vacuous pass".to_string());
        }
        if outcome != "output_contract_violation" || violations.is_empty() {
            return Err(format!(
                "empty pointers must be an output_contract_violation with a recorded violation: {outcome}"
            ));
        }
        Ok(())
    }

    #[test]
    fn two_seam_honesty_fixture_compiles_and_its_tests_pass() -> Result<(), String> {
        // PR-time proof for the honesty-case fixture (#2824 review): build
        // the two-seam before/after states through the same code path the
        // case uses (build_two_seam_commits, on a copy of the real
        // boundary_gap fixture input), then run the fixture's own test
        // suite. Cargo is available in this environment — the xtask suite
        // itself builds with it — so no skip path is documented or taken.
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("read clock failed: {err}"))?
            .as_nanos();
        // The fixture must be isolated from the repository workspace: under
        // `cargo test` the temporary directory can resolve inside the
        // workspace tree, and a nested package manifest then collides with
        // the repo workspace. An empty `[workspace]` table is cargo's own
        // isolation mechanism; the seam construction below stays the exact
        // code path the case uses.
        let root = release_temp_root()?.join(format!(
            "ripr-negative-corpus-two-seam-{}-{stamp}",
            std::process::id()
        ));
        let result = (|| -> Result<(), String> {
            let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .ok_or_else(|| "xtask manifest dir has no parent".to_string())?;
            copy_dir_recursive(&workspace_root.join("fixtures/boundary_gap/input"), &root)?;
            let mut manifest = crate::read_text_lossy(&root.join("Cargo.toml"))?;
            manifest.push_str("\n[workspace]\n");
            fs::write(root.join("Cargo.toml"), manifest)
                .map_err(|err| format!("isolate two-seam fixture workspace failed: {err}"))?;
            run_fixture_git_command(&root, &["init", "--quiet", "--template="], "initialize")?;
            run_fixture_git_command(
                &root,
                &["config", "user.name", "RIPR Corpus Test"],
                "user name",
            )?;
            run_fixture_git_command(
                &root,
                &["config", "user.email", "corpus-test@example.invalid"],
                "user email",
            )?;
            run_fixture_git_command(&root, &["config", "commit.gpgSign", "false"], "signing")?;
            run_fixture_git_command(&root, &["-c", "core.hooksPath=", "add", "."], "stage")?;
            run_fixture_git_command(
                &root,
                &[
                    "-c",
                    "core.hooksPath=",
                    "commit",
                    "--quiet",
                    "-m",
                    "boundary_gap input",
                ],
                "commit",
            )?;
            let before_sha = fixture_head(&root)?;
            let (_before_two_seam, after_two_seam) = build_two_seam_commits(&root, &before_sha)?;
            checkout_fixture_commit(&root, &after_two_seam)?;
            // Infrastructure errors (spawn, lock contention) are already
            // classified inside the helper and never reach this conclusion;
            // a non-zero exit here is a compile or test diagnostic and is
            // the only shape that counts as fixture evidence.
            let cargo = run_nested_fixture_cargo_test(&root)?;
            if !cargo.success {
                return Err(format!(
                    "fixture evidence: the two-seam honesty fixture failed to compile or its tests failed: {}",
                    command_details(&cargo).join("; ")
                ));
            }
            if !cargo
                .stdout
                .contains("loyalty_below_threshold_has_no_points")
            {
                return Err(
                    "the loyalty discriminator test did not run in the two-seam fixture"
                        .to_string(),
                );
            }
            if !cargo.stdout.contains("equality_boundary_discounts") {
                return Err(
                    "the boundary discriminator test did not run in the two-seam fixture"
                        .to_string(),
                );
            }
            Ok(())
        })();
        let cleanup = fs::remove_dir_all(&root);
        result?;
        cleanup.map_err(|err| format!("remove two-seam compile fixture failed: {err}"))?;
        Ok(())
    }

    #[test]
    fn report_markdown_renders_case_matrix_and_dispositions() -> Result<(), String> {
        let spec = CaseSpec {
            id: "case",
            group: "pair",
            description: "case",
            run: case_pair_reversed_revisions,
        };
        let candidate = CandidateIdentity {
            path: "p".to_string(),
            version_output: "ripr 0.10.0".to_string(),
            sha256: "sha256:0".to_string(),
        };
        let baseline = BaselineContext {
            fixture_root: PathBuf::from("fixture"),
            before_sha: "b".to_string(),
            after_sha: "a".to_string(),
            artifacts: Vec::new(),
        };
        let receipt = CaseReceipt::new(&spec, &candidate, &baseline);
        let report = NegativeCorpusReport {
            version: "0.10.0".to_string(),
            status: "fail".to_string(),
            run_status: "complete".to_string(),
            covered_families: vec![
                "artifact".to_string(),
                "pair".to_string(),
                "verify".to_string(),
                "receipt".to_string(),
            ],
            deferred_families: Vec::new(),
            candidate,
            baseline_root: "fixture".to_string(),
            baseline_before_sha: "b".to_string(),
            baseline_after_sha: "a".to_string(),
            baseline_artifacts: Vec::new(),
            baseline_cleanup: "removed".to_string(),
            cases: vec![receipt],
            dispositions: deferred_dispositions(),
        };
        let markdown = negative_corpus_markdown(&report);
        for needle in [
            "Status: fail",
            "Run status: `complete`",
            "## Case matrix",
            "| case |",
            "## Deferred dispositions",
            "migration-claims-fresh-production",
        ] {
            if !markdown.contains(needle) {
                return Err(format!("markdown is missing {needle:?}"));
            }
        }
        let json = negative_corpus_json(&report)?;
        let value: Value = serde_json::from_str(&json)
            .map_err(|err| format!("report JSON is malformed: {err}"))?;
        if value["summary"]["total_cases"] != json!(1) {
            return Err("report summary lost the case count".to_string());
        }
        if value["summary"]["not_applicable"] != json!(2) {
            return Err("report summary lost the disposition count".to_string());
        }
        Ok(())
    }
}
