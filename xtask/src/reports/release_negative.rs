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

use super::release::{
    CommandResult, absolute_installed_binary, artifact_string, checkout_fixture_commit,
    command_details, create_authentic_repo_exposure_fixture, fixture_head, git_worktree_is_clean,
    installed_ripr_binary, produce_authentic_chain_in_fixture, read_crate_version, read_json_value,
    run_command_in_dir, run_fixture_git_command, run_packaged_install,
};
use super::release_server::{hex_lower, sha256_file};

const NEGATIVE_WORK_DIR: &str = "target/ripr/release-negative-corpus";
const BEFORE_ARTIFACT: &str = "before.repo-exposure.json";
const AFTER_ARTIFACT: &str = "after.repo-exposure.json";
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

/// What the installed binary must do with the mutated state: assert the
/// exit status FIRST and the closed reason token second.
#[derive(Clone, Debug)]
enum Expectation {
    Reject {
        token: &'static str,
        out_absent: Option<&'static str>,
        out_preserved: Option<(String, String)>,
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

    fn expected_kind(&self) -> String {
        match self {
            Self::Reject { token, .. } => (*token).to_string(),
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
        // Real-producers-only (#2824 review): this slice emits exactly one
        // passing shape — a Reject-arm case evaluated `rejected_as_expected`
        // by evaluate_rejection, restored `restored_byte_exact` by
        // restore_state. The pinned-projection pass token has no producer in
        // this slice (the Expectation::Pass arm lands with the verify/receipt
        // families) and must not be accepted before it exists.
        let passed = self.process_outcome == "rejected_as_expected";
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
        receipt.mutation = execution.mutation.clone();
        receipt.argv = execution.argv.clone();
        receipt.control_argv = execution.control_argv.clone();
        receipt.original_artifacts = execution.original.clone();
        receipt.mutated_artifacts = execution.mutated.clone();
        receipt.expected_failure_kind = execution.expected.expected_kind();

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
    fn corpus_covers_every_landed_case_once() -> Result<(), String> {
        let specs = case_specs();
        let mut ids = std::collections::BTreeSet::new();
        for spec in &specs {
            if !ids.insert(spec.id) {
                return Err(format!("duplicate case id {}", spec.id));
            }
        }
        // Staged coverage (#2824 slice B1): exactly the artifact family
        // landed in this slice; the verify/receipt families extend this to
        // the full 28-case matrix in the follow-up slice.
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
        ];
        for id in required {
            if !ids.contains(id) {
                return Err(format!("required case {id} is missing from the corpus"));
            }
        }
        if specs.len() != required.len() {
            return Err(format!(
                "corpus has {} cases; the landed matrix has {}",
                specs.len(),
                required.len()
            ));
        }
        Ok(())
    }

    #[test]
    fn family_coverage_discloses_the_deferred_families() -> Result<(), String> {
        let (covered, deferred) = family_coverage();
        if covered != vec!["artifact".to_string()] {
            return Err(format!(
                "this slice covers only the artifact family: {covered:?}"
            ));
        }
        let expected_deferred = vec![
            "pair".to_string(),
            "verify".to_string(),
            "receipt".to_string(),
        ];
        if deferred != expected_deferred {
            return Err(format!(
                "deferred families must be derived from the case registry: {deferred:?}"
            ));
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
}
