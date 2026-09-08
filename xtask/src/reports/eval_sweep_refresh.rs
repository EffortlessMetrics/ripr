//! Managed currentness-bound external sweep refresh — `cargo xtask eval-sweep
//! refresh` (RIPR-SPEC-0086, issue #3566).
//!
//! The historical Tier A sweep receipt is not current for promotion. This route
//! reruns the exact accepted eight-subject denominator against one explicit
//! RIPR binary and writes CANDIDATE artifacts outside accepted/current state:
//! every subject receives one terminal candidate row in the schema-0.3 receipt
//! shape `eval_sweep_check` validates, plus a managed execution receipt naming
//! the binary, manifest, host class, network authorization, and exact subject
//! identities.
//!
//! The route consumes #3565's validated subjects directly
//! (`eval_sweep_check::validate_accepted_manifest` — one loader owns manifest
//! semantics; refresh never re-parses), and it self-validates its own candidate
//! receipt through `eval_sweep_check::validate_run_receipt` before writing a
//! byte, so a produced candidate and `eval-sweep check --runs <candidate>`
//! agree by construction. That symmetry — refresh produces exactly what check
//! validates — is the core proof of this route, exercised offline by the
//! `python_eval_sweep_refresh` tests with synthetic local subjects (local
//! seed clones through git's local transport, never a network run).
//!
//! Honesty boundaries, each load-bearing:
//!
//! - Authorization gate: the route refuses without BOTH the managed env signal
//!   (`RIPR_EVAL_SWEEP_NETWORK=1`) and the explicit `--allow-network` flag.
//!   Ordinary CI never refreshes live repositories; the refusal is typed and
//!   names the missing signals.
//! - Candidate separation: `--out` is mandatory and rejected when it equals or
//!   overlaps accepted state (the `fixtures/` tree, or the repository root
//!   itself). The comparison runs on canonicalized paths on both sides, so a
//!   `--out` symlink resolving into accepted state is refused too. A candidate
//!   receipt cannot rewrite expected status, subject selection, the historical
//!   receipt, or the current pointer; promotion is #3567's validator/publisher,
//!   not this route.
//! - Explicit binary: `--ripr-bin` is mandatory and must name an existing file;
//!   the route resolves it to an absolute path before any invocation, so PATH
//!   can never select an installed binary. The binary's content identity is
//!   re-verified after EACH subject's run: a concurrent rebuild is a typed
//!   `tempfail` row disposition named in the execution receipt — never a
//!   silent stale digest over changed bytes.
//! - Subjects remain selected after ANY failure: materialization, input,
//!   infrastructure, parse, timeout, unsupported, partial, and stale outcomes
//!   all stay rows in the candidate denominator under the owned 0.3 status
//!   vocabulary; infrastructure failures at or after materialization (cache
//!   creation, analysis spawn, raw retention, binary-identity drift) are
//!   contained as typed `tempfail` rows naming their cause, so the route
//!   ALWAYS produces the full candidate receipt denominator; a failed row
//!   retains its available evidence and can never count as complete.
//! - Terminal states stay distinct: `complete`/`partial`/`parse-failed`/
//!   `timed-out`/`crashed`/`unsupported`/`tempfail`/`stale` are derived from
//!   producer facts (rail-enforced timeout, exit status, captured JSON,
//!   `analysis_outcome` kind and completeness) with no two outcomes merged.
//! - Stability: the route runs a second pass ONLY where the first result is
//!   `complete` — the one state where a gap/output identity comparison is
//!   meaningful — and records the comparison in the row's `repeat` block
//!   (stable) or its typed unstable list (mismatch); output-identity drift
//!   across passes is a typed execution-receipt note, never folded into the
//!   gap verdict.
//! - Missing identities are typed incomplete (disclosed in the candidate
//!   verdict), never invented: the route records only what a real producer
//!   observed (resolved HEAD SHAs, binary digest, version, exit codes, digests
//!   of captured bytes) and omits what it cannot observe (the analyzer source
//!   SHA of an arbitrary supplied binary, ripr feature sets without a
//!   producer, whole-tree sha256 digests git does not emit).
//! - Process hygiene reuses the existing controlled-execution rails only:
//!   every spawn — `ripr check`, `git clone`/`checkout`/`rev-parse`, the
//!   version probe — goes through the allowlisted `crate::run` bounded-capture
//!   helpers (wall-clock timeout, captured stdout/stderr, process-tree
//!   termination, cwd anchored inside the candidate tree, isolated
//!   `RIPR_CACHE_DIR`, terminal prompts disabled for git); no new
//!   process-spawn surface is introduced.
//! - Candidate publication is generation-staged: raw evidence and receipts are
//!   written under `<out>/.refresh-staging` and moved into their final
//!   locations by a single rename per file at finalization, so an interrupted
//!   refresh cannot leave a mixed old/new generation in the published paths.
//!   Full one-receipt-to-one-raw-set generation binding across reruns remains
//!   #3567's promotion contract.
//! - Claim boundary: a candidate receipt is structural currentness evidence
//!   over the retained denominator. Self-validation is structural — digest
//!   syntax, status vocabulary, and schema shape are what `eval-sweep check`
//!   accepts; the route does not recompute retained artifact bytes against
//!   their digests. That recomputation is the promotion-time binding #3567's
//!   validator/publisher owns. The receipt is not accepted promotion evidence,
//!   no structural-accuracy, repair-correctness, gate, badge, or support claim
//!   is inferred, and distributions stay descriptive (they never gate).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{Value, json};

use super::eval_sweep::{
    AlignmentCounts, ClassificationCounts, count_distributions, findings_have_parse_failure,
    gap_ids,
};
use super::eval_sweep_check::{
    AcceptedManifest, AcceptedSubject, load_strict_json, sha256_hex, validate_accepted_manifest,
    validate_run_receipt,
};
use crate::run::{capture_bytes_in_dir_with_timeout, capture_output_with_timeout};

const DEFAULT_MANIFEST: &str = "fixtures/python-eval-sweep/manifest.json";
const DEFAULT_CHECKOUT_ROOT: &str = "target/ripr/eval-sweep/checkouts";
const DEFAULT_TIMEOUT_SECS: u64 = 120;
const DEFAULT_CLONE_TIMEOUT_SECS: u64 = 600;
const USAGE: &str = "usage: cargo xtask eval-sweep refresh --manifest <path> --ripr-bin <path> --out <dir> --allow-network [--checkout-root <dir>] [--timeout-secs <secs>] [--clone-timeout-secs <secs>]";
const RERUN_COMMAND: &str = "cargo xtask eval-sweep refresh";

/// The managed authorization signals: BOTH are required. Following the typed
/// refusal precedent (`python_judged_panel_replay::NETWORK_REFUSAL`), the
/// refusal names the missing signals instead of failing silently.
const NETWORK_ENV: &str = "RIPR_EVAL_SWEEP_NETWORK";
const NETWORK_ENV_VALUE: &str = "1";
const ALLOW_FLAG: &str = "--allow-network";

const RECEIPT_SCHEMA_VERSION: &str = "0.3";
const RECEIPT_KIND: &str = "python_eval_sweep_report";
const RECEIPT_FILE: &str = "eval-sweep-refresh-receipt.json";
const EXEC_RECEIPT_SCHEMA_VERSION: &str = "0.1";
const EXEC_RECEIPT_KIND: &str = "python_eval_sweep_refresh_execution_receipt";
const EXEC_RECEIPT_FILE: &str = "execution-receipt.json";
const SPEC: &str = "RIPR-SPEC-0086";
const REPORT_JSON: &str = "eval-sweep-refresh.json";
const REPORT_MD: &str = "eval-sweep-refresh.md";

/// Candidate row paths are recorded relative to the candidate out directory
/// (the absolute out directory is named in the execution receipt), so every
/// path inside the receipt stays portable.
const SUBJECTS_DIR: &str = "subjects";
const RAW_DIR: &str = "raw";
const CACHE_DIR: &str = "cache";

/// Generation staging directory under `--out`: raw evidence and receipts land
/// here first and are moved into their final locations by one rename per file
/// at finalization. A refresh interrupted before finalization leaves only this
/// directory behind (cleared by the next run) — never a mixed old/new
/// generation in the published paths.
const STAGING_DIR: &str = ".refresh-staging";

/// Bounded corpus walk: the working-set cap on entries visited per subject
/// before the walk discloses `partial` selection instead of a silently
/// truncated count. Recorded in the execution receipt as `working_set_cap`.
const WORKING_SET_CAP: usize = 500_000;

/// Per-invocation deadline for the binary version probe.
const VERSION_PROBE_TIMEOUT: Duration = Duration::from_secs(30);

/// The config identity recorded on every row, honestly: `ripr check --root
/// <tree>` loads the subject root's own `ripr.toml` when present, so a
/// materialized tree carrying that file records `subject-ripr-toml` (the
/// execution receipt retains the relative path); `default` only when the
/// materialized tree genuinely carries no subject config; `unobserved` when
/// no tree was materialized to inspect.
const CONFIG_PROFILE_SUBJECT: &str = "subject-ripr-toml";
const CONFIG_PROFILE_DEFAULT: &str = "default";
const CONFIG_PROFILE_UNOBSERVED: &str = "unobserved";

/// The subject-root config file `ripr check` loads from the analyzed root.
const SUBJECT_CONFIG_FILE: &str = "ripr.toml";

/// Detects the subject-root config identity from a materialized tree: the
/// file's presence is the real producer (the analyzer loads it from the
/// root), so the row records the configured identity instead of claiming the
/// default configuration.
fn detect_subject_config(dir: &Path) -> (String, Option<String>) {
    if dir.join(SUBJECT_CONFIG_FILE).is_file() {
        (
            CONFIG_PROFILE_SUBJECT.to_string(),
            Some(SUBJECT_CONFIG_FILE.to_string()),
        )
    } else {
        (CONFIG_PROFILE_DEFAULT.to_string(), None)
    }
}

/// The repeat-run identity: the first complete analysis pass of the same
/// subject under the same binary/config/input.
const REPEAT_COMPARABLE_WITH: &str = "pass-1";

// ---------------------------------------------------------------------------
// Entry point and args
// ---------------------------------------------------------------------------

pub(crate) fn run_refresh(args: &[String]) -> Result<(), String> {
    let env_value = std::env::var(NETWORK_ENV).ok();
    run_refresh_with_env(args, env_value.as_deref())
}

/// The route body with the env signal injected, so the authorization gate and
/// the full route are testable without mutating process environment state.
pub(crate) fn run_refresh_with_env(
    args: &[String],
    network_env: Option<&str>,
) -> Result<(), String> {
    let parsed = parse_refresh_args(args)?;
    check_network_authorization(parsed.allow_network, network_env)?;
    let ripr_bin = parsed.ripr_bin.clone().ok_or_else(|| {
        format!(
            "eval-sweep refresh requires --ripr-bin <explicit binary path>; PATH cannot select the analyzer\n{USAGE}"
        )
    })?;
    let out_display = parsed.out.clone().ok_or_else(|| {
        format!(
            "eval-sweep refresh requires --out <candidate-dir>; candidates must be written outside accepted/current state\n{USAGE}"
        )
    })?;

    // Manifest semantics come from the one accepted-artifact loader (#3565);
    // refresh never re-parses.
    let (manifest_value, manifest_sha256) = load_strict_json(&parsed.manifest)?;
    let manifest = validate_accepted_manifest(&manifest_value, manifest_sha256.clone())?;

    let binary = resolve_binary(&ripr_bin)?;
    let out_abs = validate_out_separation(&out_display)?;
    std::fs::create_dir_all(&out_abs).map_err(|error| {
        format!(
            "eval-sweep refresh cannot create out dir `{}`: {error}",
            out_abs.display()
        )
    })?;
    let manifest_dir = Path::new(&parsed.manifest)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    // Generation staging: fresh per run, so a previous interrupted refresh
    // cannot mix its half-written generation into this one.
    let staging = out_abs.join(STAGING_DIR);
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|error| {
        format!(
            "eval-sweep refresh cannot create staging dir `{}`: {error}",
            staging.display()
        )
    })?;
    let destinations = CandidateOut {
        out: out_abs,
        staging,
    };

    let started = std::time::Instant::now();
    let mut executions: Vec<SubjectExecution> = Vec::new();
    for subject in &manifest.subjects {
        // A subject's failure never aborts the route: refresh_subject always
        // yields this subject's row, so the candidate denominator always
        // equals the accepted manifest.
        executions.push(refresh_subject(
            subject,
            &binary,
            &manifest_dir,
            &destinations,
            &parsed.checkout_root,
            parsed.timeout,
            parsed.clone_timeout,
        ));
    }

    // Assemble the candidate receipt, then self-validate it through the exact
    // retained-receipt validator before writing anything: refresh produces
    // only what `eval-sweep check --runs` accepts.
    let receipt = assemble_receipt(&manifest_sha256, &binary, &executions);
    let receipt_display = destinations
        .out
        .join(RECEIPT_FILE)
        .to_string_lossy()
        .to_string();
    let validated = validate_run_receipt(&receipt, &manifest_sha256, &manifest, &receipt_display)?;
    let exec_receipt = assemble_execution_receipt(
        &manifest,
        &manifest_sha256,
        &parsed,
        &binary,
        &destinations.out,
        &executions,
        started.elapsed().as_millis(),
    );

    write_candidate_file(&destinations.staging, RECEIPT_FILE, &receipt)?;
    write_candidate_file(&destinations.staging, EXEC_RECEIPT_FILE, &exec_receipt)?;
    finalize_candidate_generation(&destinations.staging, &destinations.out)?;

    println!(
        "eval-sweep refresh: manifest={} subjects={} binary={} (sha256 {})",
        parsed.manifest,
        manifest.subjects.len(),
        binary.absolute.display(),
        binary.binary_digest
    );
    for execution in &executions {
        println!(
            "  {} status={} run={}",
            execution.subject_id, execution.row.status, execution.row.counts_as_run
        );
    }
    println!(
        "eval-sweep refresh: candidate receipt self-validated: schema={} denominator selected={} run={} verdict={} incomplete_disclosures={}",
        validated.schema_version,
        validated.denominator_selected,
        validated.denominator_run,
        validated.verdict().as_str(),
        validated.incomplete.len(),
    );
    println!(
        "eval-sweep refresh: candidates written under {} — accepted/current state untouched; promotion requires #3567",
        destinations.out.display()
    );
    println!("rerun: {RERUN_COMMAND}");

    let report = render_report_json(&exec_receipt)?;
    crate::write_report(REPORT_JSON, &format!("{report}\n"))?;
    crate::write_report(REPORT_MD, &render_report_markdown(&exec_receipt))?;
    Ok(())
}

struct RefreshArgs {
    manifest: String,
    ripr_bin: Option<String>,
    out: Option<String>,
    checkout_root: String,
    timeout: Duration,
    clone_timeout: Duration,
    allow_network: bool,
}

fn parse_refresh_args(args: &[String]) -> Result<RefreshArgs, String> {
    let mut parsed = RefreshArgs {
        manifest: DEFAULT_MANIFEST.to_string(),
        ripr_bin: None,
        out: None,
        checkout_root: DEFAULT_CHECKOUT_ROOT.to_string(),
        timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
        clone_timeout: Duration::from_secs(DEFAULT_CLONE_TIMEOUT_SECS),
        allow_network: false,
    };
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--manifest" => parsed.manifest = take_value(args, &mut index, "--manifest")?,
            "--ripr-bin" => parsed.ripr_bin = Some(take_value(args, &mut index, "--ripr-bin")?),
            "--out" => parsed.out = Some(take_value(args, &mut index, "--out")?),
            "--checkout-root" => {
                parsed.checkout_root = take_value(args, &mut index, "--checkout-root")?
            }
            "--timeout-secs" => {
                let raw = take_value(args, &mut index, "--timeout-secs")?;
                parsed.timeout = Duration::from_secs(parse_secs(&raw, "--timeout-secs")?);
            }
            "--clone-timeout-secs" => {
                let raw = take_value(args, &mut index, "--clone-timeout-secs")?;
                parsed.clone_timeout =
                    Duration::from_secs(parse_secs(&raw, "--clone-timeout-secs")?);
            }
            "--allow-network" => parsed.allow_network = true,
            other => {
                return Err(format!(
                    "unknown eval-sweep refresh argument: {other}\n{USAGE}"
                ));
            }
        }
        index += 1;
    }
    Ok(parsed)
}

fn take_value(args: &[String], index: &mut usize, flag: &str) -> Result<String, String> {
    *index += 1;
    args.get(*index)
        .cloned()
        .ok_or_else(|| format!("eval-sweep refresh {flag} requires a value\n{USAGE}"))
}

fn parse_secs(raw: &str, flag: &str) -> Result<u64, String> {
    let secs = raw.trim().parse::<u64>().map_err(|error| {
        format!(
            "eval-sweep refresh {flag} expects a positive integer, got `{raw}`: {error}\n{USAGE}"
        )
    })?;
    if secs == 0 {
        return Err(format!(
            "eval-sweep refresh {flag} must be a positive integer\n{USAGE}"
        ));
    }
    Ok(secs)
}

// ---------------------------------------------------------------------------
// Authorization gate (pure; env injected)
// ---------------------------------------------------------------------------

/// The typed managed-authorization refusal: refresh is a live-repository
/// capability and refuses closed without BOTH signals, before any filesystem
/// work. Offline refusal is itself testable.
fn check_network_authorization(flag_present: bool, env_value: Option<&str>) -> Result<(), String> {
    let env_ok = env_value == Some(NETWORK_ENV_VALUE);
    if env_ok && flag_present {
        return Ok(());
    }
    let missing = if env_ok {
        format!("the flag `{ALLOW_FLAG}` is absent")
    } else {
        format!("the env signal `{NETWORK_ENV}={NETWORK_ENV_VALUE}` is not set")
    };
    Err(format!(
        "eval-sweep refresh refuses to run: managed network materialization requires explicit authorization, but {missing}; ordinary CI never refreshes live repositories\nrerun: {RERUN_COMMAND} --manifest <path> --ripr-bin <path> --out <dir> --allow-network (with {NETWORK_ENV}={NETWORK_ENV_VALUE})"
    ))
}

// ---------------------------------------------------------------------------
// Candidate out-directory separation
// ---------------------------------------------------------------------------

/// Rejects a candidate out directory that equals or overlaps accepted state:
/// inside `fixtures/`, containing `fixtures/`, or at/above the repository root
/// (which would put candidates next to `.git` and the accepted manifest). A
/// dedicated directory INSIDE the repository root (e.g. under
/// `target/ripr/eval-sweep/refresh/`) is the recommended shape and is
/// accepted. The repository root is anchored at the compiled workspace layout
/// so the check stays stable regardless of the process cwd.
///
/// The comparison runs on canonicalized paths on BOTH sides (`--out` and the
/// accepted-state roots): a `--out` path that is a symlink (or junction) into
/// accepted state resolves to the same canonical target and is refused, where
/// a lexical comparison would wave it through. Canonicalization walks to the
/// deepest existing ancestor when the leaf does not exist yet, and a
/// canonicalization failure is a typed refusal, never a skip.
fn validate_out_separation(out_display: &str) -> Result<PathBuf, String> {
    if out_display.trim().is_empty() {
        return Err(format!(
            "eval-sweep refresh --out requires a non-empty candidate directory\n{USAGE}"
        ));
    }
    let out_abs = std::path::absolute(out_display).map_err(|error| {
        format!("eval-sweep refresh --out `{out_display}` cannot be resolved: {error}\n{USAGE}")
    })?;
    let out_canonical = canonical_deep(&out_abs).map_err(|error| {
        format!(
            "eval-sweep refresh --out `{out_display}` cannot be canonicalized: {error}\n{USAGE}"
        )
    })?;
    let repo_root = std::path::absolute(repo_root_anchor()).map_err(|error| {
        format!("eval-sweep refresh cannot resolve the repository root: {error}")
    })?;
    let repo_root_canonical = canonical_deep(&repo_root).map_err(|error| {
        format!("eval-sweep refresh cannot canonicalize the repository root: {error}")
    })?;
    let fixtures_canonical = canonical_deep(&repo_root.join("fixtures")).map_err(|error| {
        format!("eval-sweep refresh cannot canonicalize the accepted `fixtures/` tree: {error}")
    })?;
    if path_overlaps(&out_canonical, &fixtures_canonical) {
        return Err(format!(
            "eval-sweep refresh --out `{out_display}` overlaps accepted state under `fixtures/`; candidate artifacts must be written outside accepted/current state\n{USAGE}"
        ));
    }
    if out_canonical == repo_root_canonical || repo_root_canonical.starts_with(&out_canonical) {
        return Err(format!(
            "eval-sweep refresh --out `{out_display}` is the repository root or contains it; candidate artifacts must live in a dedicated directory (e.g. under `target/ripr/eval-sweep/refresh/`)\n{USAGE}"
        ));
    }
    if out_abs.exists() && !out_abs.is_dir() {
        return Err(format!(
            "eval-sweep refresh --out `{out_display}` exists and is not a directory\n{USAGE}"
        ));
    }
    Ok(out_abs)
}

/// Canonicalizes a path through `std::fs::canonicalize`, which requires every
/// component to exist: when the leaf (or any intermediate) does not exist yet,
/// the deepest existing ancestor is canonicalized and the non-existing tail is
/// re-joined lexically. This resolves symlinks and junctions on every existing
/// component — the property the candidate-separation check needs — while
/// still accepting a not-yet-created candidate directory.
fn canonical_deep(path: &Path) -> std::io::Result<PathBuf> {
    let mut existing = path.to_path_buf();
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    loop {
        match std::fs::canonicalize(&existing) {
            Ok(canonical) => {
                let mut resolved = canonical;
                for component in tail.iter().rev() {
                    resolved.push(component);
                }
                return Ok(resolved);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                match (existing.parent(), existing.file_name()) {
                    (Some(parent), Some(name)) => {
                        tail.push(name.to_os_string());
                        existing = parent.to_path_buf();
                    }
                    _ => return Err(error),
                }
            }
            Err(error) => return Err(error),
        }
    }
}

/// The repository root anchor: the xtask package is compiled inside the
/// repository workspace, so its manifest directory's parent is the repo
/// root. This anchor stays stable when the process cwd differs (parallel
/// tests transiently change cwd); when the compiled layout is absent, the
/// cwd is the anchor, which is the repo root for `cargo xtask` runs.
/// Shared with the report route (#3567), which resolves the same anchor for
/// its currentness input recomputation.
pub(crate) fn repo_root_anchor() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .filter(|root| root.is_dir())
        .unwrap_or_else(|| PathBuf::from("."))
}

/// True when `a` equals `b`, sits inside `b`, or contains `b`.
fn path_overlaps(a: &Path, b: &Path) -> bool {
    a == b || a.starts_with(b) || b.starts_with(a)
}

// ---------------------------------------------------------------------------
// Binary identity (explicit; PATH can never select)
// ---------------------------------------------------------------------------

struct BinaryIdentity {
    /// Absolute path passed to every invocation — the supplied path, resolved.
    /// A bare command name cannot resolve to an existing file before any
    /// spawn, so PATH is out of the loop by construction.
    absolute: PathBuf,
    binary_digest: String,
    version: String,
    /// Recorded only when the supplied path's parent directory names a cargo
    /// build profile (`debug`/`release`); otherwise omitted (typed
    /// incomplete) — the directory name is a real producer, a guess is not.
    build_profile: Option<String>,
}

impl std::fmt::Debug for BinaryIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BinaryIdentity")
            .field("absolute", &self.absolute)
            .field("binary_digest", &self.binary_digest)
            .field("version", &self.version)
            .field("build_profile", &self.build_profile)
            .finish()
    }
}

fn resolve_binary(supplied: &str) -> Result<BinaryIdentity, String> {
    if supplied.trim().is_empty() {
        return Err(format!(
            "eval-sweep refresh --ripr-bin requires a non-empty explicit binary path; PATH cannot select the analyzer\n{USAGE}"
        ));
    }
    let absolute = std::path::absolute(supplied).map_err(|error| {
        format!("eval-sweep refresh --ripr-bin `{supplied}` cannot be resolved: {error}\n{USAGE}")
    })?;
    if !absolute.is_file() {
        return Err(format!(
            "eval-sweep refresh --ripr-bin `{supplied}` does not name an existing file; supply the explicit built binary (e.g. target/debug/ripr.exe); PATH cannot select the analyzer\n{USAGE}"
        ));
    }
    let bytes = std::fs::read(&absolute).map_err(|error| {
        format!(
            "eval-sweep refresh cannot read the supplied binary `{supplied}` for its digest: {error}\n{USAGE}"
        )
    })?;
    let binary_digest = sha256_hex(&bytes);

    let probe = capture_output_with_timeout(
        &absolute.to_string_lossy(),
        &["--version".to_string()],
        &[],
        VERSION_PROBE_TIMEOUT,
        "eval-sweep refresh ripr --version probe",
    )
    .map_err(|error| {
        format!("eval-sweep refresh could not probe the supplied binary: {error}\n{USAGE}")
    })?;
    if probe.timed_out || !probe.status.is_some_and(|status| status.success()) {
        return Err(format!(
            "eval-sweep refresh: the supplied binary `{supplied}` did not report a version; refresh binds one explicit working binary\n{USAGE}"
        ));
    }
    let version = probe.stdout.trim().to_string();
    if version.is_empty() {
        return Err(format!(
            "eval-sweep refresh: the supplied binary `{supplied}` printed an empty version; refresh binds one explicit working binary\n{USAGE}"
        ));
    }
    let build_profile = absolute
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
        .filter(|name| *name == "debug" || *name == "release")
        .map(str::to_string);
    Ok(BinaryIdentity {
        absolute,
        binary_digest,
        version,
        build_profile,
    })
}

/// Re-verifies the binary's content identity against the digest recorded at
/// resolve time. `None` means the identity held; `Some(reason)` names the
/// drift — including the fail-closed case where the binary cannot be re-read
/// at all (unverifiable identity is not identity). The route re-verifies
/// after EACH subject's run: a concurrent rebuild must never let rows keep a
/// stale digest while executed bytes changed underneath them.
fn binary_identity_drift(binary: &BinaryIdentity) -> Option<String> {
    match std::fs::read(&binary.absolute) {
        Ok(bytes) => {
            let post_run_digest = sha256_hex(&bytes);
            if post_run_digest == binary.binary_digest {
                None
            } else {
                Some(format!(
                    "binary content identity drifted during the run: recorded sha256 `{}`, post-run sha256 `{}` — the executed bytes can no longer be attributed to the recorded identity",
                    binary.binary_digest, post_run_digest
                ))
            }
        }
        Err(error) => Some(format!(
            "binary could not be re-read after the run for its identity check: {error}"
        )),
    }
}

// ---------------------------------------------------------------------------
// Materialization
// ---------------------------------------------------------------------------

/// One subject's materialization result. Every failure stays typed and keeps
/// the subject selected in the denominator.
enum Materialization {
    /// The pinned tree is materialized at `dir` with its HEAD verified to be
    /// the pinned SHA.
    Materialized { dir: PathBuf, source: &'static str },
    /// Candidate content exists but the pinned SHA could not be verified on
    /// it (drifted HEAD, not a git checkout). Terminal status `stale`.
    Stale { limitation: String },
    /// Materialization infrastructure failed (clone/checkout failure).
    /// Terminal status `tempfail`.
    Failed { limitation: String },
    /// Materialization was never attempted because the input was unusable.
    /// Terminal status `tempfail` with state `skipped`.
    Skipped { limitation: String },
}

fn materialize_subject(
    subject: &AcceptedSubject,
    out_abs: &Path,
    checkout_root: &str,
    clone_timeout: Duration,
) -> Materialization {
    let dir = out_abs.join(SUBJECTS_DIR).join(&subject.id);
    if dir.exists() {
        return verify_materialized_dir(&dir, subject, "candidate_reuse", clone_timeout);
    }
    // Local seed first: a pre-placed checkout under the checkout root is a
    // filesystem-local clone source (git local transport, no network), so an
    // authorized rerun can materialize offline from retained checkouts.
    let seed = Path::new(checkout_root).join(&subject.id);
    if seed.join(".git").exists() {
        if clone_into(&seed, &dir, clone_timeout, subject).is_ok() {
            return verify_materialized_dir(&dir, subject, "local_seed_clone", clone_timeout);
        }
        // The seed lacked the pin or the local clone failed; the network
        // clone may still satisfy the pin.
        discard_partial_dir(&dir);
    }
    match clone_into(Path::new(&subject.url), &dir, clone_timeout, subject) {
        Ok(()) => verify_materialized_dir(&dir, subject, "network_clone", clone_timeout),
        Err(limitation) => {
            discard_partial_dir(&dir);
            // Only a genuinely absent pin is accepted-state staleness;
            // infrastructure checkout failures stay `tempfail`
            // (#3735 review round 3).
            if limitation.starts_with("pin-unavailable:") {
                Materialization::Stale { limitation }
            } else {
                Materialization::Failed { limitation }
            }
        }
    }
}

/// Clones `from` into `to` and checks out the pinned SHA detached. Local
/// paths use git's local transport; the manifest URL is https. Bounded by
/// `clone_timeout` through the shared capture helper, with terminal prompts
/// disabled so a credential prompt can never hang the run.
fn clone_into(
    from: &Path,
    to: &Path,
    clone_timeout: Duration,
    subject: &AcceptedSubject,
) -> Result<(), String> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create candidate subjects directory `{}`: {error}",
                parent.display()
            )
        })?;
    }
    let to_text = to.to_string_lossy().to_string();
    let clone = capture_output_with_timeout(
        "git",
        &[
            "clone".to_string(),
            "--filter=blob:none".to_string(),
            "--no-checkout".to_string(),
            from.to_string_lossy().to_string(),
            to_text.clone(),
        ],
        &[("GIT_TERMINAL_PROMPT", "0")],
        clone_timeout,
        &format!("eval-sweep refresh git clone for `{}`", subject.id),
    )
    .map_err(|error| format!("clone of `{}` failed: {error}", subject.id))?;
    if clone.timed_out {
        return Err(format!(
            "clone of `{}` timed out after {}s; the process tree was terminated",
            subject.id,
            clone_timeout.as_secs()
        ));
    }
    if !clone.status.is_some_and(|status| status.success()) {
        return Err(format!(
            "clone of `{}` failed: {}",
            subject.id,
            first_line(&clone.stderr)
        ));
    }
    let checkout = capture_output_with_timeout(
        "git",
        &[
            "-C".to_string(),
            to_text,
            "checkout".to_string(),
            "--detach".to_string(),
            subject.sha.clone(),
        ],
        &[("GIT_TERMINAL_PROMPT", "0")],
        clone_timeout,
        &format!("eval-sweep refresh git checkout for `{}`", subject.id),
    )
    .map_err(|error| {
        format!(
            "checkout of `{}`@{} failed: {error}",
            subject.id, subject.sha
        )
    })?;
    if checkout.timed_out {
        return Err(format!(
            "checkout of `{}`@{} timed out after {}s",
            subject.id,
            subject.sha,
            clone_timeout.as_secs()
        ));
    }
    if !checkout.status.is_some_and(|status| status.success()) {
        // A successful blob-filtered clone whose pinned SHA is simply ABSENT
        // is accepted-state staleness: `git checkout` names it with
        // "reference is not a tree" / "bad object". Every other checkout
        // failure (lazy blob fetch, network, disk, permissions, locks) is
        // infrastructure and stays `tempfail` (#3735 review round 3).
        // The missing-pin signature can sit behind earlier warnings in the
        // stderr stream, so classification scans the WHOLE stderr; only the
        // first line is quoted in the diagnostic.
        let stderr = first_line(&checkout.stderr);
        let missing_pin = checkout.stderr.contains("reference is not a tree")
            || checkout.stderr.contains("bad object")
            || checkout.stderr.contains("invalid reference");
        let marker = if missing_pin {
            "pin-unavailable"
        } else {
            "checkout-infrastructure"
        };
        return Err(format!(
            "{marker}: pinned SHA {} could not be checked out in the cloned repository of `{}`: {}",
            subject.sha, subject.id, stderr
        ));
    }
    Ok(())
}

/// Verifies a materialized directory: it must be a git checkout whose HEAD is
/// exactly the pinned SHA (after a bounded detached re-checkout when HEAD
/// drifted — bounded by the CONFIGURED `clone_timeout`, the same deadline the
/// clone itself runs under) AND whose worktree is clean — local modifications
/// or untracked files mean the content cannot be attributed to the accepted
/// pin even when HEAD matches, so a reused-but-dirty checkout is a typed
/// `Stale`, never an analysis of altered content.
fn verify_materialized_dir(
    dir: &Path,
    subject: &AcceptedSubject,
    source: &'static str,
    clone_timeout: Duration,
) -> Materialization {
    if !dir.join(".git").exists() {
        return Materialization::Stale {
            limitation: format!(
                "candidate materialization directory `{}` exists but is not a git checkout; the pinned SHA cannot be verified",
                dir.display()
            ),
        };
    }
    let head = git_head(dir, &subject.id);
    if head.as_deref() == Some(subject.sha.as_str()) {
        return match git_worktree_dirty(dir, &subject.id) {
            Some(dirty) => Materialization::Stale {
                limitation: format!(
                    "candidate materialization directory `{}` is at the pinned SHA `{}` but {dirty}; the pinned SHA alone does not certify altered content",
                    dir.display(),
                    subject.sha
                ),
            },
            None => Materialization::Materialized {
                dir: dir.to_path_buf(),
                source,
            },
        };
    }
    let reset = capture_output_with_timeout(
        "git",
        &[
            "-C".to_string(),
            dir.to_string_lossy().to_string(),
            "checkout".to_string(),
            "--detach".to_string(),
            subject.sha.clone(),
        ],
        &[("GIT_TERMINAL_PROMPT", "0")],
        clone_timeout,
        &format!("eval-sweep refresh re-checkout for `{}`", subject.id),
    );
    let rechecked = match reset {
        Ok(output) => {
            !output.timed_out
                && output.status.is_some_and(|status| status.success())
                && git_head(dir, &subject.id).as_deref() == Some(subject.sha.as_str())
        }
        Err(_) => false,
    };
    if rechecked {
        return match git_worktree_dirty(dir, &subject.id) {
            Some(dirty) => Materialization::Stale {
                limitation: format!(
                    "candidate materialization directory `{}` was re-checked out to the pinned SHA `{}` but {dirty}; the pinned SHA alone does not certify altered content",
                    dir.display(),
                    subject.sha
                ),
            },
            None => Materialization::Materialized {
                dir: dir.to_path_buf(),
                source,
            },
        };
    }
    Materialization::Stale {
        limitation: format!(
            "materialization HEAD is `{}` but the accepted pin is `{}` and the pin could not be checked out",
            head.unwrap_or_else(|| "unresolved".to_string()),
            subject.sha
        ),
    }
}

/// Bounded `git status --porcelain` inside a materialized directory. Returns
/// `None` only when the worktree is verifiably clean; ANY output (local
/// modifications or untracked files) is `Some(reason)`. A status command that
/// fails or times out is fail-closed `Some(reason)` too: unverifiable
/// cleanliness is not cleanliness.
fn git_worktree_dirty(dir: &Path, subject_id: &str) -> Option<String> {
    let output = capture_output_with_timeout(
        "git",
        &[
            "-C".to_string(),
            dir.to_string_lossy().to_string(),
            "status".to_string(),
            "--porcelain".to_string(),
        ],
        &[],
        Duration::from_secs(30),
        &format!("eval-sweep refresh status for `{subject_id}`"),
    );
    match output {
        Ok(result) if !result.timed_out && result.status.is_some_and(|status| status.success()) => {
            if result.stdout.trim().is_empty() {
                None
            } else {
                Some(format!(
                    "the checkout carries local modifications or untracked files ({} porcelain line(s): {})",
                    result.stdout.lines().count(),
                    first_line(&result.stdout)
                ))
            }
        }
        Ok(result) => Some(format!(
            "`git status` could not be evaluated: {}",
            first_line(&result.stderr)
        )),
        Err(error) => Some(format!("`git status` could not run: {error}")),
    }
}

/// Bounded `git rev-parse HEAD` inside a materialized directory.
fn git_head(dir: &Path, subject_id: &str) -> Option<String> {
    let output = capture_output_with_timeout(
        "git",
        &[
            "-C".to_string(),
            dir.to_string_lossy().to_string(),
            "rev-parse".to_string(),
            "HEAD".to_string(),
        ],
        &[],
        Duration::from_secs(30),
        &format!("eval-sweep refresh rev-parse for `{subject_id}`"),
    )
    .ok()?;
    if output.timed_out || !output.status.is_some_and(|status| status.success()) {
        return None;
    }
    let sha = output.stdout.trim().to_string();
    if sha.is_empty() { None } else { Some(sha) }
}

fn discard_partial_dir(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

fn first_line(text: &str) -> String {
    text.lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| "no diagnostic output".to_string())
}

// ---------------------------------------------------------------------------
// Corpus selection (bounded walk; real path-shaped producers only)
// ---------------------------------------------------------------------------

struct CorpusCounts {
    source_files: u64,
    test_files: u64,
    generated_files: u64,
    vendor_files: u64,
    /// False when the bounded walk could not enumerate the whole tree: the
    /// working-set cap was hit, or a directory listing failed. The counts are
    /// then omitted (never a silently truncated number) and the row's
    /// corpus-selection state cannot claim `selected`.
    complete: bool,
    /// Names why the walk is incomplete — the failed subtree (a `read_dir`
    /// failure) or the working-set cap — so a truncated count is disclosed
    /// with its cause in the execution receipt, never silent.
    limitation: Option<String>,
}

/// The corpus-selection state a row/execution record may claim for these
/// counts: `selected` only when the walk enumerated the whole tree.
fn corpus_selection_state(corpus: Option<&CorpusCounts>) -> &'static str {
    match corpus {
        Some(counts) if counts.complete => "selected",
        Some(_) => "partial",
        None => "absent",
    }
}

/// Counts the materialized Python working set by path shape. Producers are
/// real and deterministic: directory names (`test`/`tests`/`testing`,
/// `vendor`/`vendored`/`_vendor`/`third_party`, `generated`/`_generated`)
/// and generator filename conventions (`*_pb2.py`, `*_pb2_grpc.py`). Only
/// `.py` files count; other files are not part of the analyzer's Python
/// working set. Vendor classification precedes generated/test so a vendored
/// generated file is counted once, as vendored.
fn count_corpus(dir: &Path) -> CorpusCounts {
    let mut counts = CorpusCounts {
        source_files: 0,
        test_files: 0,
        generated_files: 0,
        vendor_files: 0,
        complete: true,
        limitation: None,
    };
    let mut stack = vec![dir.to_path_buf()];
    let mut seen = 0usize;
    while let Some(current) = stack.pop() {
        let entries = match std::fs::read_dir(&current) {
            Ok(entries) => entries,
            // An unreadable subtree is a truncated walk, not a smaller
            // corpus: the selection state can never claim complete, and the
            // cause names the subtree (consistent with the working-set cap).
            Err(error) => {
                counts.complete = false;
                if counts.limitation.is_none() {
                    counts.limitation = Some(format!(
                        "corpus walk could not list `{}`: {error}",
                        current.display()
                    ));
                }
                continue;
            }
        };
        for entry in entries {
            // A per-entry iteration error (concurrent removal, metadata
            // failure) means the walk is truncated: the corpus selection can
            // never claim `complete` over a walk that skipped an entry
            // (#3735 review).
            let Ok(entry) = entry else {
                counts.complete = false;
                if counts.limitation.is_none() {
                    counts.limitation = Some(format!(
                        "corpus walk could not list an entry under `{}`",
                        current.display()
                    ));
                }
                continue;
            };
            seen += 1;
            if seen > WORKING_SET_CAP {
                counts.complete = false;
                if counts.limitation.is_none() {
                    counts.limitation = Some(format!(
                        "the bounded corpus walk hit the working-set cap ({WORKING_SET_CAP} entries) before finishing; the counts are truncated"
                    ));
                }
                return counts;
            }
            let path = entry.path();
            if path.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name == ".git" {
                    continue;
                }
                stack.push(path);
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.ends_with(".py") {
                continue;
            }
            let lowered = name.to_ascii_lowercase();
            let segments = path
                .strip_prefix(dir)
                .map(|relative| {
                    relative
                        .components()
                        .map(|component| {
                            component.as_os_str().to_string_lossy().to_ascii_lowercase()
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let file_segments = &segments[..segments.len().saturating_sub(1)];
            let in_tests = file_segments
                .iter()
                .any(|part| matches!(part.as_str(), "test" | "tests" | "testing"));
            let in_vendor = file_segments.iter().any(|part| {
                matches!(
                    part.as_str(),
                    "vendor" | "vendored" | "_vendor" | "third_party"
                )
            });
            let generated = file_segments
                .iter()
                .any(|part| matches!(part.as_str(), "generated" | "_generated"))
                || lowered.ends_with("_pb2.py")
                || lowered.ends_with("_pb2_grpc.py");
            let test_file =
                in_tests || lowered.starts_with("test_") || lowered.ends_with("_test.py");
            if in_vendor {
                counts.vendor_files += 1;
            } else if generated {
                counts.generated_files += 1;
            } else if test_file {
                counts.test_files += 1;
            } else {
                counts.source_files += 1;
            }
        }
    }
    counts
}

// ---------------------------------------------------------------------------
// Analysis passes
// ---------------------------------------------------------------------------

/// The owned 0.3 terminal-status vocabulary. Every member keeps the subject
/// selected; exactly the five run statuses evidence an analysis attempt.
#[derive(Clone, Copy)]
enum RunStatus {
    Complete,
    Partial,
    ParseFailed,
    TimedOut,
    Crashed,
    Unsupported,
    Tempfail,
    Stale,
}

impl RunStatus {
    fn as_str(&self) -> &'static str {
        match self {
            RunStatus::Complete => "complete",
            RunStatus::Partial => "partial",
            RunStatus::ParseFailed => "parse-failed",
            RunStatus::TimedOut => "timed-out",
            RunStatus::Crashed => "crashed",
            RunStatus::Unsupported => "unsupported",
            RunStatus::Tempfail => "tempfail",
            RunStatus::Stale => "stale",
        }
    }

    /// The validator's run/non-run split: exactly these five evidence an
    /// analysis attempt and count toward `repos_run`.
    fn counts_as_run(&self) -> bool {
        matches!(
            self,
            RunStatus::Complete
                | RunStatus::Partial
                | RunStatus::ParseFailed
                | RunStatus::TimedOut
                | RunStatus::Crashed
        )
    }
}

fn execution_state(status: &RunStatus) -> &'static str {
    match status {
        RunStatus::Complete
        | RunStatus::Partial
        | RunStatus::ParseFailed
        | RunStatus::Unsupported => "executed",
        RunStatus::Crashed => "failed",
        RunStatus::TimedOut => "timed-out",
        RunStatus::Tempfail | RunStatus::Stale => "not-executed",
    }
}

fn detection_state(status: &RunStatus) -> &'static str {
    match status {
        RunStatus::Complete
        | RunStatus::Partial
        | RunStatus::ParseFailed
        | RunStatus::Unsupported => "detected",
        RunStatus::Crashed => "failed",
        RunStatus::TimedOut => "unknown",
        RunStatus::Tempfail | RunStatus::Stale => "absent",
    }
}

fn materialization_state(materialization: &Materialization) -> &'static str {
    match materialization {
        Materialization::Materialized { .. } => "materialized",
        // A pinned tree that could not be verified/obtained is a failed
        // materialization; the row's terminal status distinguishes stale
        // (unverifiable content) from tempfail (infrastructure failure).
        Materialization::Stale { .. } | Materialization::Failed { .. } => "failed",
        Materialization::Skipped { .. } => "skipped",
    }
}

/// One captured `ripr check` invocation over a materialized subject.
struct AnalysisPass {
    status: RunStatus,
    runtime_ms: u64,
    /// Completeness as reported by the producer (`analysis_outcome`).
    complete: bool,
    limitations: Vec<String>,
    gap_ids: BTreeSet<String>,
    classification: ClassificationCounts,
    alignment: AlignmentCounts,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    output_json: Option<Value>,
}

/// Classifies one captured pass from producer facts. Terminal states stay
/// distinct: rail-enforced timeout, process failure exit, non-JSON stdout,
/// `unsupported_input` analysis kind, parse degradation to a named
/// static-unknown limitation, and producer-reported completeness.
fn classify_pass(timed_out: bool, exit_ok: bool, parsed: Option<&Value>) -> RunStatus {
    if timed_out {
        return RunStatus::TimedOut;
    }
    if !exit_ok {
        return RunStatus::Crashed;
    }
    let Some(value) = parsed else {
        // SPEC-0086 consistency: a success exit whose stdout is not the
        // emitted JSON is a process-level failure, not a subject parse
        // degradation.
        return RunStatus::Crashed;
    };
    let analysis_kind = value
        .get("analysis_outcome")
        .and_then(|outcome| outcome.get("outcome"))
        .and_then(|outcome| outcome.get("kind"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if analysis_kind == "unsupported_input" {
        return RunStatus::Unsupported;
    }
    if analysis_kind == "analysis_failed" || findings_have_parse_failure(value) {
        return RunStatus::ParseFailed;
    }
    let complete = value
        .get("analysis_outcome")
        .and_then(|outcome| outcome.get("analysis_complete"))
        .and_then(Value::as_bool);
    if complete == Some(true) {
        RunStatus::Complete
    } else {
        RunStatus::Partial
    }
}

fn run_analysis_pass(
    binary: &BinaryIdentity,
    subject_dir: &Path,
    diff_absolute: &Path,
    cache_dir: &Path,
    timeout: Duration,
    subject_id: &str,
) -> Result<AnalysisPass, String> {
    let args: Vec<String> = [
        "check".to_string(),
        "--root".to_string(),
        subject_dir.to_string_lossy().to_string(),
        "--diff".to_string(),
        diff_absolute.to_string_lossy().to_string(),
        "--mode".to_string(),
        "fast".to_string(),
        "--json".to_string(),
    ]
    .to_vec();
    let output = capture_bytes_in_dir_with_timeout(
        &binary.absolute,
        &args,
        subject_dir,
        &[(
            "RIPR_CACHE_DIR",
            cache_dir.to_string_lossy().to_string().as_str(),
        )],
        &[],
        timeout,
        &format!("eval-sweep refresh ripr check for `{subject_id}`"),
    )?;
    let runtime_ms = output.duration.as_millis() as u64;
    let stdout_text = String::from_utf8_lossy(&output.stdout).to_string();
    let parsed: Option<Value> = serde_json::from_str(&stdout_text).ok();
    let exit_ok = output.status.is_some_and(|status| status.success());
    let status = classify_pass(output.timed_out, exit_ok, parsed.as_ref());
    let complete = parsed
        .as_ref()
        .and_then(|value| {
            value
                .get("analysis_outcome")
                .and_then(|outcome| outcome.get("analysis_complete"))
                .and_then(Value::as_bool)
        })
        .unwrap_or(false);
    let limitations = parsed
        .as_ref()
        .and_then(|value| value.get("analysis_outcome"))
        .and_then(|outcome| outcome.get("outcome"))
        .and_then(|outcome| outcome.get("limitations"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    item.get("kind")
                        .and_then(Value::as_str)
                        .unwrap_or("unreported_limitation")
                        .to_string()
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let (classification, alignment) = parsed.as_ref().map(count_distributions).unwrap_or_default();
    Ok(AnalysisPass {
        status,
        runtime_ms,
        complete,
        limitations,
        gap_ids: parsed.as_ref().map(gap_ids).unwrap_or_default(),
        classification,
        alignment,
        stdout: output.stdout,
        stderr: output.stderr,
        output_json: parsed,
    })
}

// ---------------------------------------------------------------------------
// Input resolution
// ---------------------------------------------------------------------------

/// Resolves the synthetic diff for one subject. Resolution order (documented;
/// the process cwd is deliberately out of the loop, so an absolute
/// `--manifest` supplied from a foreign working directory cannot turn a valid
/// input into a `tempfail` row):
///
/// 1. manifest-directory-relative — the layout synthetic refresh manifests
///    use (`diffs/<id>.diff` next to `manifest.json`);
/// 2. repository-root-relative — the canonical accepted-manifest layout
///    (`fixtures/python-eval-sweep/diffs/<id>.diff`), anchored at the
///    compiled workspace root, never the cwd.
///
/// Returns the absolute path; the manifest-declared portable path is recorded
/// on the row.
fn resolve_diff(manifest_dir: &Path, declared: &str) -> Result<PathBuf, String> {
    let from_manifest_dir = manifest_dir.join(declared);
    if from_manifest_dir.is_file() {
        return std::path::absolute(&from_manifest_dir)
            .map_err(|error| format!("cannot resolve diff `{declared}`: {error}"));
    }
    let from_repo_root = repo_root_anchor().join(declared);
    if from_repo_root.is_file() {
        return std::path::absolute(&from_repo_root)
            .map_err(|error| format!("cannot resolve diff `{declared}`: {error}"));
    }
    Err(format!(
        "synthetic diff `{declared}` was not found relative to the manifest directory or the repository root"
    ))
}

fn diff_bytes_digest(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path)
        .map_err(|error| format!("cannot read diff `{}`: {error}", path.display()))?;
    Ok(sha256_hex(&bytes))
}

// ---------------------------------------------------------------------------
// Stability phase
// ---------------------------------------------------------------------------

/// The repeat-run comparison result for one subject. Performed only where the
/// first pass is `complete` — the one state where the comparison is meaningful.
enum StabilityResult {
    Compared {
        gap_stable: bool,
        unstable_gap_ids: Vec<String>,
        /// Raw-stdout digest identity across passes. A drift with stable gap
        /// identity is a typed note, never folded into the gap verdict.
        output_identity_stable: bool,
        repeat_raw_hex: String,
        /// The retained second-pass stdout, written to the raw evidence dir.
        repeat_stdout: Vec<u8>,
        /// The retained second-pass stderr, written to the raw evidence dir
        /// and digested in the row's `repeat.repeat_stderr` (SPEC-0086
        /// retention covers each pass).
        repeat_stderr: Vec<u8>,
    },
    NotCompared {
        limitation: String,
    },
}

fn run_stability_pass(
    binary: &BinaryIdentity,
    subject_dir: &Path,
    diff_absolute: &Path,
    cache_dir: &Path,
    timeout: Duration,
    subject_id: &str,
    first: &AnalysisPass,
) -> StabilityResult {
    // Fail-closed at the owner: only a complete first result may enter the
    // repeat phase at all. Equal gap sets of two failures would otherwise
    // read as `stable`; a non-complete first result never runs a repeat and
    // never claims stability, whatever the call site does.
    if !matches!(first.status, RunStatus::Complete) {
        return StabilityResult::NotCompared {
            limitation: format!(
                "first pass ended in status `{}`; the stability pass runs only where the first result is complete, so no repeat is spent and no stability is claimed",
                first.status.as_str()
            ),
        };
    }
    let second = match run_analysis_pass(
        binary,
        subject_dir,
        diff_absolute,
        cache_dir,
        timeout,
        subject_id,
    ) {
        Ok(pass) => pass,
        Err(error) => {
            return StabilityResult::NotCompared {
                limitation: format!("stability pass could not run: {error}"),
            };
        }
    };
    if !second.status.counts_as_run() {
        return StabilityResult::NotCompared {
            limitation: format!(
                "stability pass ended in status `{}`; a comparison needs a complete second pass",
                second.status.as_str()
            ),
        };
    }
    let unstable: Vec<String> = first
        .gap_ids
        .symmetric_difference(&second.gap_ids)
        .cloned()
        .collect();
    let gap_stable = unstable.is_empty();
    let repeat_raw_hex = sha256_hex(&second.stdout);
    let output_identity_stable = repeat_raw_hex == sha256_hex(&first.stdout);
    StabilityResult::Compared {
        gap_stable,
        unstable_gap_ids: unstable,
        output_identity_stable,
        repeat_raw_hex,
        repeat_stdout: second.stdout,
        repeat_stderr: second.stderr,
    }
}

// ---------------------------------------------------------------------------
// Candidate row (pure assembly)
// ---------------------------------------------------------------------------

/// Everything one row assembly needs; a struct keeps the pure function's
/// signature bounded.
struct RowInputs<'a> {
    subject: &'a AcceptedSubject,
    binary: &'a BinaryIdentity,
    /// The manifest-declared portable diff path recorded as the config input.
    diff_portable: &'a str,
    diff_digest: Option<String>,
    materialization: &'a Materialization,
    first: Option<&'a AnalysisPass>,
    stability: Option<&'a StabilityResult>,
    corpus: Option<&'a CorpusCounts>,
    /// The honest config identity for this row (`subject-ripr-toml`,
    /// `default`, or `unobserved` — see `detect_subject_config`).
    config_profile: &'a str,
}

/// One candidate row: the terminal 0.3 shape `eval_sweep_check` validates.
/// Assembled only from observed facts; absent identities are omitted (typed
/// incomplete downstream), never invented.
struct CandidateRow {
    value: Value,
    status: &'static str,
    counts_as_run: bool,
    runtime_ms: Option<u64>,
    stability: Option<bool>,
    classification: Value,
    alignment: Value,
}

fn assemble_row(inputs: RowInputs) -> CandidateRow {
    let RowInputs {
        subject,
        binary,
        diff_portable,
        diff_digest,
        materialization,
        first,
        stability,
        corpus,
        config_profile,
    } = inputs;
    let id = &subject.id;

    // Terminal status from the phases: input/materialization failures keep
    // the subject selected under the owned terminal vocabulary.
    let status: RunStatus = match materialization {
        Materialization::Stale { .. } => RunStatus::Stale,
        Materialization::Failed { .. } | Materialization::Skipped { .. } => RunStatus::Tempfail,
        Materialization::Materialized { .. } => match first {
            // Fail-closed backstop: a verified tree with no captured pass is
            // an infrastructure failure, never a quiet complete.
            None => RunStatus::Tempfail,
            Some(pass) => pass.status,
        },
    };
    let ran = status.counts_as_run();

    // Digests over retained bytes; every present digest is real. Rows that
    // never ran carry no digests block (typed incomplete).
    let digests_value = match first {
        Some(pass) => {
            let raw_hex = sha256_hex(&pass.stdout);
            let stderr_hex = sha256_hex(&pass.stderr);
            let mut digests = serde_json::Map::new();
            digests.insert("raw".to_string(), json!(raw_hex));
            if let Some(output) = &pass.output_json {
                let canonical = serde_json::to_vec_pretty(output).unwrap_or_default();
                digests.insert("output".to_string(), json!(sha256_hex(&canonical)));
            }
            let repeat_hex = match stability {
                Some(StabilityResult::Compared { repeat_raw_hex, .. }) => {
                    Some(repeat_raw_hex.as_str())
                }
                _ => None,
            };
            digests.insert(
                "evidence".to_string(),
                json!(evidence_digest(&raw_hex, &stderr_hex, repeat_hex)),
            );
            Value::Object(digests)
        }
        None => Value::Null,
    };

    // Distributions: required on analyzed rows (zero-filled over the full
    // vocabulary), omitted on rows that did not run (a non-run row must
    // carry no analysis counts).
    let (classification, alignment) = if ran {
        match first {
            Some(pass) => (pass.classification.to_json(), pass.alignment.to_json()),
            None => (
                ClassificationCounts::default().to_json(),
                AlignmentCounts::default().to_json(),
            ),
        }
    } else {
        (Value::Null, Value::Null)
    };

    // Corpus selection from the bounded walk.
    let corpus_value = match corpus {
        Some(counts) if counts.complete => json!({
            "state": "selected",
            "source_files": counts.source_files,
            "test_files": counts.test_files,
            "generated_files": counts.generated_files,
            "vendor_files": counts.vendor_files,
        }),
        Some(_) => json!({ "state": "partial" }),
        None => json!({ "state": "absent" }),
    };

    // Repository identity: the accepted pin, restated. tree/snapshot
    // identities are copied only from the manifest (never invented).
    let repository = json!({
        "url": subject.url,
        "sha": subject.sha,
    });

    let binary_block = {
        let mut block = json!({
            "digest": binary.binary_digest,
            "version": binary.version,
        });
        if let Some(profile) = &binary.build_profile {
            block["build_profile"] = json!(profile);
        }
        block
    };

    let mut row = serde_json::Map::new();
    row.insert("id".to_string(), json!(id));
    row.insert("status".to_string(), json!(status.as_str()));
    row.insert("repository".to_string(), repository);
    if let Some(tree) = &subject.tree_digest {
        row.insert("tree_digest".to_string(), json!(tree));
    }
    if let Some(snapshot) = &subject.snapshot {
        row.insert("snapshot".to_string(), json!(snapshot));
    }
    row.insert("license".to_string(), json!(subject.license));
    if let Some(retention) = &subject.retention_class {
        row.insert("retention_class".to_string(), json!(retention));
    }
    if let Some(provenance) = &subject.provenance {
        row.insert("provenance".to_string(), json!(provenance));
    }
    row.insert(
        "selected_root".to_string(),
        json!(format!("{SUBJECTS_DIR}/{id}")),
    );
    row.insert("layout".to_string(), json!([subject.shape]));
    row.insert("binary".to_string(), binary_block);
    row.insert(
        "config".to_string(),
        json!({
            "profile": config_profile,
            "input": diff_portable,
        }),
    );
    if let Some(digest) = diff_digest {
        row.insert("input_digest".to_string(), json!(digest));
    }
    row.insert(
        "materialization".to_string(),
        json!(materialization_state(materialization)),
    );
    row.insert("detection".to_string(), json!(detection_state(&status)));
    row.insert("execution".to_string(), json!(execution_state(&status)));
    row.insert("corpus_selection".to_string(), corpus_value);
    row.insert("digests".to_string(), digests_value);
    if let Some(StabilityResult::Compared {
        gap_stable,
        unstable_gap_ids,
        repeat_stderr,
        ..
    }) = stability
    {
        row.insert(
            "repeat".to_string(),
            json!({
                "comparable_with": REPEAT_COMPARABLE_WITH,
                "gap_ids_stable": gap_stable,
                "unstable_gap_ids": unstable_gap_ids,
                "repeat_stderr": sha256_hex(repeat_stderr),
            }),
        );
    }
    // A non-comparable first result omits the repeat block entirely; the
    // validator discloses the missing comparison as typed incomplete.
    if ran {
        let runtime = first.map(|pass| pass.runtime_ms).unwrap_or(0);
        row.insert("runtime_ms".to_string(), json!(runtime));
    }
    if !classification.is_null() {
        row.insert("classification_counts".to_string(), classification.clone());
    }
    if !alignment.is_null() {
        row.insert("alignment_counts".to_string(), alignment.clone());
    }

    CandidateRow {
        status: status.as_str(),
        counts_as_run: ran,
        runtime_ms: if ran {
            first.map(|pass| pass.runtime_ms)
        } else {
            None
        },
        stability: match stability {
            Some(StabilityResult::Compared { gap_stable, .. }) => Some(*gap_stable),
            _ => None,
        },
        classification,
        alignment,
        value: Value::Object(row),
    }
}

/// Digest preimage for the `evidence` digest: binds the retained raw stdout,
/// the raw stderr, and (when the stability pass compared) the second raw
/// stdout. Defined once here and documented in the spec.
fn evidence_digest(raw_hex: &str, stderr_hex: &str, repeat_hex: Option<&str>) -> String {
    let mut preimage = format!("raw={raw_hex}\nstderr={stderr_hex}\n");
    if let Some(repeat_hex) = repeat_hex {
        preimage.push_str(&format!("repeat_raw={repeat_hex}\n"));
    }
    sha256_hex(preimage.as_bytes())
}

// ---------------------------------------------------------------------------
// Summary derivation (mirrors the validator's arithmetic)
// ---------------------------------------------------------------------------

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

/// Sums one row distribution (always the full zero-filled key set on run
/// rows) into the derived summary map.
fn merge_distribution_value(target: &mut BTreeMap<String, u64>, value: &Value) {
    let Some(map) = value.as_object() else {
        return;
    };
    for (name, count) in map {
        let count = count.as_u64().unwrap_or(0);
        *target.entry(name.clone()).or_insert(0) += count;
    }
}

fn derive_summary(executions: &[SubjectExecution]) -> Value {
    let total = executions.len();
    let run_rows: Vec<&CandidateRow> = executions
        .iter()
        .filter(|execution| execution.row.counts_as_run)
        .map(|execution| &execution.row)
        .collect();
    let run = run_rows.len();
    let crashed = executions
        .iter()
        .filter(|execution| execution.row.status == "crashed")
        .count();
    let parse_failed = executions
        .iter()
        .filter(|execution| execution.row.status == "parse-failed")
        .count();
    let timed_out = executions
        .iter()
        .filter(|execution| execution.row.status == "timed-out")
        .count();
    let tempfail = executions
        .iter()
        .filter(|execution| execution.row.status == "tempfail")
        .count();

    let mut runtimes: Vec<u64> = run_rows.iter().filter_map(|row| row.runtime_ms).collect();
    runtimes.sort_unstable();
    let mut runtime_total: u64 = 0;
    for runtime in &runtimes {
        runtime_total = runtime_total.saturating_add(*runtime);
    }
    let (min, median, max) = if runtimes.is_empty() {
        (0, 0, 0)
    } else {
        (
            runtimes[0],
            runtimes[runtimes.len() / 2],
            runtimes[runtimes.len() - 1],
        )
    };

    // Stability is recorded only when every run row carries repeat evidence;
    // a recorded aggregate over partial evidence is exactly what the
    // validator rejects, so the producer omits it instead.
    let all_evidenced = run_rows.iter().all(|row| row.stability.is_some());
    let stable_count = run_rows
        .iter()
        .filter(|row| row.stability == Some(true))
        .count();

    let mut classification: BTreeMap<String, u64> = BTreeMap::new();
    let mut alignment: BTreeMap<String, u64> = BTreeMap::new();
    for row in &run_rows {
        merge_distribution_value(&mut classification, &row.classification);
        merge_distribution_value(&mut alignment, &row.alignment);
    }
    // Zero run rows still record the emitter's zero-filled key set, never an
    // absent distinction.
    let classification_value = if classification.is_empty() {
        ClassificationCounts::default().to_json()
    } else {
        json!(classification)
    };
    let alignment_value = if alignment.is_empty() {
        AlignmentCounts::default().to_json()
    } else {
        json!(alignment)
    };

    let (gate_status, gate_reason) = if run == 0 {
        (
            "not_run",
            format!(
                "no subjects reached an analysis attempt ({total} total, {tempfail} tempfail); not_run is never a vacuous pass"
            ),
        )
    } else if crashed == 0 && all_evidenced && stable_count == run {
        (
            "pass",
            format!(
                "{run} subject(s) analyzed; no crashes; repeated complete runs reported stable native gap identity"
            ),
        )
    } else {
        (
            "review",
            format!(
                "{crashed} crash(es), {parse_failed} parse-failed, {timed_out} timed-out over {run} analyzed subject(s); incomplete or unstable evidence requires review before any promotion use"
            ),
        )
    };

    let mut summary = json!({
        "repos_total": total,
        "repos_run": run,
        "repos_skipped": 0,
        "repos_clone_failed": tempfail,
        "crash_count": crashed,
        "crash_rate": ratio(crashed, run),
        "parse_failure_count": parse_failed,
        "parse_failure_rate": ratio(parse_failed, run),
        "timed_out_count": timed_out,
        "runtime_ms_min": min,
        "runtime_ms_median": median,
        "runtime_ms_max": max,
        "runtime_ms_total": runtime_total,
        "classification_counts": classification_value,
        "alignment_counts": alignment_value,
        "gate_status": gate_status,
        "gate_reason": gate_reason,
    });
    if all_evidenced && run > 0 {
        summary["gap_id_stable_count"] = json!(stable_count);
        summary["gap_id_unstable_count"] = json!(run - stable_count);
        summary["gap_id_stability_rate"] = json!(ratio(stable_count, run));
    }
    summary
}

// ---------------------------------------------------------------------------
// Execution record and receipt assembly (pure)
// ---------------------------------------------------------------------------

/// One subject's full execution record: the candidate row plus the typed
/// per-phase facts the execution receipt retains (the 0.3 row schema owns no
/// limitations field).
struct SubjectExecution {
    subject_id: String,
    row: CandidateRow,
    materialization_state: &'static str,
    materialization_source: Option<&'static str>,
    materialization_limitation: Option<String>,
    analysis_complete: Option<bool>,
    analysis_limitations: Vec<String>,
    stability_performed: bool,
    stability_limitation: Option<String>,
    output_identity_stable: Option<bool>,
    /// The honest config identity detected for this subject's tree.
    config_profile: String,
    /// The subject-root config file's out-relative path, when one exists.
    subject_config_path: Option<String>,
    /// The corpus-selection state this row's counts support (`selected`,
    /// `partial`, or `absent`).
    corpus_state: &'static str,
    /// Why the corpus selection is not `selected` (unreadable subtree,
    /// working-set cap); `None` when the walk was complete or never ran.
    corpus_limitation: Option<String>,
    /// A named infrastructure failure contained at this subject (cache
    /// creation, spawn, raw retention, binary-identity drift): the row is a
    /// typed `tempfail` and the execution receipt names the cause.
    infrastructure_limitation: Option<String>,
}

fn assemble_receipt(
    manifest_sha256: &str,
    binary: &BinaryIdentity,
    executions: &[SubjectExecution],
) -> Value {
    let mut ripr_block = json!({
        "binary_digest": binary.binary_digest,
        "version": binary.version,
    });
    if let Some(profile) = &binary.build_profile {
        ripr_block["build_profile"] = json!(profile);
    }
    let rows: Vec<Value> = executions
        .iter()
        .map(|execution| execution.row.value.clone())
        .collect();
    json!({
        "schema_version": RECEIPT_SCHEMA_VERSION,
        "kind": RECEIPT_KIND,
        "spec": SPEC,
        "tier": "A",
        "manifest_digest": manifest_sha256,
        "ripr": ripr_block,
        "summary": derive_summary(executions),
        "repos": rows,
    })
}

/// The managed execution receipt: names its binary, manifest, host class,
/// network authorization, and exact subject identities, plus the per-phase
/// typed records (materialization source/limitation, analysis completeness
/// and limitations, stability comparison) the 0.3 row schema has no place
/// for. Only the wall-clock telemetry field varies between equivalent runs.
fn assemble_execution_receipt(
    manifest: &AcceptedManifest,
    manifest_sha256: &str,
    args: &RefreshArgs,
    binary: &BinaryIdentity,
    out_abs: &Path,
    executions: &[SubjectExecution],
    wall_clock_ms: u128,
) -> Value {
    let mut ripr = json!({
        "supplied_path": args.ripr_bin.clone().unwrap_or_default(),
        "absolute_path": binary.absolute.to_string_lossy(),
        "binary_digest": binary.binary_digest,
        "version": binary.version,
    });
    if let Some(profile) = &binary.build_profile {
        ripr["build_profile"] = json!(profile);
    }
    let subjects: Vec<Value> = executions
        .iter()
        .map(|execution| {
            json!({
                "id": execution.subject_id,
                "status": execution.row.status,
                "materialization": {
                    "state": execution.materialization_state,
                    "source": execution.materialization_source,
                    "limitation": execution.materialization_limitation,
                },
                "config": {
                    "profile": execution.config_profile,
                    "subject_config": execution.subject_config_path,
                },
                "corpus_selection": {
                    "state": execution.corpus_state,
                    "limitation": execution.corpus_limitation,
                },
                "analysis": {
                    "complete": execution.analysis_complete,
                    "limitations": execution.analysis_limitations,
                },
                "stability": {
                    "performed": execution.stability_performed,
                    "gap_stable": execution.row.stability,
                    "output_identity_stable": execution.output_identity_stable,
                    "limitation": execution.stability_limitation,
                },
                "infrastructure_limitation": execution.infrastructure_limitation,
            })
        })
        .collect();
    json!({
        "schema_version": EXEC_RECEIPT_SCHEMA_VERSION,
        "kind": EXEC_RECEIPT_KIND,
        "spec": SPEC,
        "manifest": {
            "path": args.manifest,
            "sha256": manifest_sha256,
            "subject_ids": manifest.ids(),
        },
        "ripr": ripr,
        "host_class": format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        "network_authorization": {
            "env_signal": format!("{NETWORK_ENV}={NETWORK_ENV_VALUE}"),
            "flag": ALLOW_FLAG,
            "granted": true,
        },
        "out_dir": out_abs.to_string_lossy(),
        "checkout_root": args.checkout_root,
        "timeout_secs": args.timeout.as_secs(),
        "clone_timeout_secs": args.clone_timeout.as_secs(),
        "working_set_cap": WORKING_SET_CAP,
        "subjects": subjects,
        "telemetry": {
            "wall_clock_ms": wall_clock_ms,
            "declaration": "wall-clock telemetry is the only run-varying field; identities and digests are deterministic over the same inputs",
        },
        "claim_boundary": "candidate currentness evidence only; accepted/current state untouched; promotion requires #3567",
    })
}

// ---------------------------------------------------------------------------
// Per-subject orchestration
// ---------------------------------------------------------------------------

/// One subject's phase outcome before row assembly.
enum SubjectPhase {
    /// The input identity could not be resolved: nothing further runs.
    InputUnavailable { limitation: String },
    /// Materialization ended in a terminal non-ready state (stale/tempfail).
    NotMaterialized(Materialization),
    /// The pinned tree is verified and the input is resolved.
    Ready {
        dir: PathBuf,
        source: &'static str,
        diff_absolute: PathBuf,
        diff_digest: String,
    },
}

fn plan_subject(
    subject: &AcceptedSubject,
    manifest_dir: &Path,
    out_abs: &Path,
    checkout_root: &str,
    clone_timeout: Duration,
) -> SubjectPhase {
    let diff_absolute = match resolve_diff(manifest_dir, &subject.synthetic_diff) {
        Ok(path) => path,
        Err(limitation) => return SubjectPhase::InputUnavailable { limitation },
    };
    let diff_digest = match diff_bytes_digest(&diff_absolute) {
        Ok(digest) => digest,
        Err(limitation) => return SubjectPhase::InputUnavailable { limitation },
    };
    match materialize_subject(subject, out_abs, checkout_root, clone_timeout) {
        Materialization::Materialized { dir, source } => SubjectPhase::Ready {
            dir,
            source,
            diff_absolute,
            diff_digest,
        },
        other => SubjectPhase::NotMaterialized(other),
    }
}

/// The verified-tree context a post-materialization failure still carries:
/// the row stays a typed `tempfail` while the facts already established
/// (verified tree, resolved input, counted corpus, detected config) remain
/// attached — evidence is kept where it is real.
struct ReadyContext<'a> {
    subject: &'a AcceptedSubject,
    binary: &'a BinaryIdentity,
    diff_portable: &'a str,
    diff_digest: Option<String>,
    dir: PathBuf,
    source: &'static str,
    corpus: Option<&'a CorpusCounts>,
    config_profile: String,
    subject_config_path: Option<String>,
}

/// The candidate output destinations: the published out directory and the
/// per-run generation staging directory under it. Candidate evidence lands in
/// staging and moves into `out` by rename at finalization; the materialized
/// subject trees and the disposable analyzer cache live directly under `out`.
struct CandidateOut {
    out: PathBuf,
    staging: PathBuf,
}

/// Contains a post-materialization infrastructure failure (cache creation,
/// analysis spawn, raw retention, binary-identity drift) at its subject: the
/// row is a typed `tempfail` assembled from the verified facts with no
/// captured pass — the route ALWAYS produces the full eight-row candidate
/// denominator, and the execution receipt names the cause. Captured bytes
/// whose retention failed are not digested (digests bind retained bytes).
fn infrastructure_failure_execution(context: ReadyContext, limitation: String) -> SubjectExecution {
    let row = assemble_row(RowInputs {
        subject: context.subject,
        binary: context.binary,
        diff_portable: context.diff_portable,
        diff_digest: context.diff_digest,
        materialization: &Materialization::Materialized {
            dir: context.dir,
            source: context.source,
        },
        first: None,
        stability: None,
        corpus: context.corpus,
        config_profile: &context.config_profile,
    });
    SubjectExecution {
        subject_id: context.subject.id.clone(),
        row,
        materialization_state: "materialized",
        materialization_source: Some(context.source),
        materialization_limitation: None,
        analysis_complete: None,
        analysis_limitations: Vec::new(),
        stability_performed: false,
        stability_limitation: None,
        output_identity_stable: None,
        config_profile: context.config_profile,
        subject_config_path: context.subject_config_path,
        corpus_state: corpus_selection_state(context.corpus),
        corpus_limitation: context.corpus.and_then(|counts| counts.limitation.clone()),
        infrastructure_limitation: Some(limitation),
    }
}

/// Runs one subject's phases to a terminal execution record. The route never
/// aborts on a subject's failure: every phase outcome below `Ready` — and
/// every infrastructure failure at or after `Ready` — still yields this
/// subject's row, so the candidate denominator always equals the manifest.
fn refresh_subject(
    subject: &AcceptedSubject,
    binary: &BinaryIdentity,
    manifest_dir: &Path,
    destinations: &CandidateOut,
    checkout_root: &str,
    timeout: Duration,
    clone_timeout: Duration,
) -> SubjectExecution {
    let id = &subject.id;
    let out_abs = destinations.out.as_path();
    let staging = destinations.staging.as_path();
    match plan_subject(subject, manifest_dir, out_abs, checkout_root, clone_timeout) {
        SubjectPhase::InputUnavailable { limitation } => {
            let row = assemble_row(RowInputs {
                subject,
                binary,
                diff_portable: &subject.synthetic_diff,
                diff_digest: None,
                materialization: &Materialization::Skipped {
                    limitation: limitation.clone(),
                },
                first: None,
                stability: None,
                corpus: None,
                config_profile: CONFIG_PROFILE_UNOBSERVED,
            });
            SubjectExecution {
                subject_id: id.clone(),
                row,
                materialization_state: "skipped",
                materialization_source: None,
                materialization_limitation: Some(limitation),
                analysis_complete: None,
                analysis_limitations: Vec::new(),
                stability_performed: false,
                stability_limitation: None,
                output_identity_stable: None,
                config_profile: CONFIG_PROFILE_UNOBSERVED.to_string(),
                subject_config_path: None,
                corpus_state: corpus_selection_state(None),
                corpus_limitation: None,
                infrastructure_limitation: None,
            }
        }
        SubjectPhase::NotMaterialized(materialization) => {
            let state = materialization_state(&materialization);
            let limitation = materialization_limitation(&materialization);
            let row = assemble_row(RowInputs {
                subject,
                binary,
                diff_portable: &subject.synthetic_diff,
                diff_digest: None,
                materialization: &materialization,
                first: None,
                stability: None,
                corpus: None,
                config_profile: CONFIG_PROFILE_UNOBSERVED,
            });
            SubjectExecution {
                subject_id: id.clone(),
                row,
                materialization_state: state,
                materialization_source: None,
                materialization_limitation: limitation,
                analysis_complete: None,
                analysis_limitations: Vec::new(),
                stability_performed: false,
                stability_limitation: None,
                output_identity_stable: None,
                config_profile: CONFIG_PROFILE_UNOBSERVED.to_string(),
                subject_config_path: None,
                corpus_state: corpus_selection_state(None),
                corpus_limitation: None,
                infrastructure_limitation: None,
            }
        }
        SubjectPhase::Ready {
            dir,
            source,
            diff_absolute,
            diff_digest,
        } => {
            // Bounded corpus walk over the verified tree.
            let corpus = count_corpus(&dir);
            // Honest config identity from the materialized tree: a subject
            // root carrying `ripr.toml` means `ripr check` runs configured,
            // not default.
            let (config_profile, subject_config_path) = detect_subject_config(&dir);
            let context = || ReadyContext {
                subject,
                binary,
                diff_portable: &subject.synthetic_diff,
                diff_digest: Some(diff_digest.clone()),
                dir: dir.clone(),
                source,
                corpus: Some(&corpus),
                config_profile: config_profile.clone(),
                subject_config_path: subject_config_path.clone(),
            };
            // Isolated per-subject cache so runs never share analyzer state.
            let cache_dir = out_abs.join(CACHE_DIR).join(id);
            if let Err(error) = std::fs::create_dir_all(&cache_dir) {
                return infrastructure_failure_execution(
                    context(),
                    format!(
                        "eval-sweep refresh cannot create cache dir `{}` for `{id}`: {error}",
                        cache_dir.display()
                    ),
                );
            }
            let first =
                match run_analysis_pass(binary, &dir, &diff_absolute, &cache_dir, timeout, id) {
                    Ok(pass) => pass,
                    Err(error) => {
                        return infrastructure_failure_execution(
                            context(),
                            format!("the analysis pass could not run: {error}"),
                        );
                    }
                };
            if let Err(error) = write_raw_output(staging, id, "pass1", &first.stdout, &first.stderr)
            {
                return infrastructure_failure_execution(
                    context(),
                    format!(
                        "the analysis ran but its pass-1 raw evidence could not be retained under the candidate tree: {error}"
                    ),
                );
            }

            // Stability runs only where the first result is complete enough
            // for a meaningful comparison (gated again inside the pass).
            let stability = if matches!(first.status, RunStatus::Complete) {
                let result = run_stability_pass(
                    binary,
                    &dir,
                    &diff_absolute,
                    &cache_dir,
                    timeout,
                    id,
                    &first,
                );
                if let StabilityResult::Compared {
                    repeat_stdout,
                    repeat_stderr,
                    ..
                } = &result
                    && let Err(error) =
                        write_repeat_raw_output(staging, id, repeat_stdout, repeat_stderr)
                {
                    return infrastructure_failure_execution(
                        context(),
                        format!(
                            "the repeat pass ran but its raw evidence could not be retained under the candidate tree: {error}"
                        ),
                    );
                }
                Some(result)
            } else {
                None
            };

            // Binary identity re-verification after THIS subject's runs: a
            // concurrent rebuild is a typed tempfail row disposition named in
            // the execution receipt — never a silent stale identity, and the
            // captured passes are discarded rather than attributed to an
            // unknown binary.
            if let Some(drift) = binary_identity_drift(binary) {
                return infrastructure_failure_execution(context(), drift);
            }

            let row = assemble_row(RowInputs {
                subject,
                binary,
                diff_portable: &subject.synthetic_diff,
                diff_digest: Some(diff_digest),
                materialization: &Materialization::Materialized {
                    dir: dir.clone(),
                    source,
                },
                first: Some(&first),
                stability: stability.as_ref(),
                corpus: Some(&corpus),
                config_profile: &config_profile,
            });
            let analysis_limitations = analysis_limitations(&first);
            let (stability_performed, stability_limitation, output_identity_stable) =
                match &stability {
                    Some(StabilityResult::Compared {
                        output_identity_stable,
                        ..
                    }) => (true, None, Some(*output_identity_stable)),
                    Some(StabilityResult::NotCompared { limitation }) => {
                        (true, Some(limitation.clone()), None)
                    }
                    None => (
                        false,
                        Some(format!(
                            "first pass ended in status `{}`; a stability comparison needs a complete first pass",
                            first.status.as_str()
                        )),
                        None,
                    ),
                };
            SubjectExecution {
                subject_id: id.clone(),
                row,
                materialization_state: "materialized",
                materialization_source: Some(source),
                materialization_limitation: None,
                analysis_complete: Some(first.complete),
                analysis_limitations,
                stability_performed,
                stability_limitation,
                output_identity_stable,
                config_profile,
                subject_config_path,
                corpus_state: corpus_selection_state(Some(&corpus)),
                corpus_limitation: corpus.limitation.clone(),
                infrastructure_limitation: None,
            }
        }
    }
}

fn analysis_limitations(first: &AnalysisPass) -> Vec<String> {
    let mut limitations = first.limitations.clone();
    if !first.complete && matches!(first.status, RunStatus::Complete) {
        limitations.push("completeness not reported by the producer".to_string());
    }
    limitations
}

fn materialization_limitation(materialization: &Materialization) -> Option<String> {
    match materialization {
        Materialization::Materialized { .. } => None,
        Materialization::Stale { limitation } => Some(limitation.clone()),
        Materialization::Failed { limitation } => Some(limitation.clone()),
        Materialization::Skipped { limitation } => Some(limitation.clone()),
    }
}

/// Retains one pass's raw stdout/stderr under `<staging>/raw/<id>/`. The bytes
/// bound the row digests; retention is part of the candidate evidence. Files
/// land in the generation staging directory and are renamed into their final
/// `raw/` locations at finalization.
fn write_raw_output(
    staging: &Path,
    subject_id: &str,
    pass: &str,
    stdout: &[u8],
    stderr: &[u8],
) -> Result<(), String> {
    let dir = staging.join(RAW_DIR).join(subject_id);
    std::fs::create_dir_all(&dir).map_err(|error| {
        format!(
            "eval-sweep refresh cannot create raw dir `{}` for `{subject_id}`: {error}",
            dir.display()
        )
    })?;
    std::fs::write(dir.join(format!("{pass}-stdout.txt")), stdout).map_err(|error| {
        format!("eval-sweep refresh cannot retain raw stdout for `{subject_id}`: {error}")
    })?;
    std::fs::write(dir.join(format!("{pass}-stderr.txt")), stderr).map_err(|error| {
        format!("eval-sweep refresh cannot retain raw stderr for `{subject_id}`: {error}")
    })?;
    Ok(())
}

/// Retains the stability pass's raw stdout AND stderr under
/// `<staging>/raw/<id>/` (`pass2-stdout.txt` / `pass2-stderr.txt`):
/// SPEC-0086's retention contract covers each pass, so the second pass keeps
/// both streams and binds the stderr digest into the row's `repeat` block.
fn write_repeat_raw_output(
    staging: &Path,
    subject_id: &str,
    stdout: &[u8],
    stderr: &[u8],
) -> Result<(), String> {
    let dir = staging.join(RAW_DIR).join(subject_id);
    std::fs::create_dir_all(&dir).map_err(|error| {
        format!(
            "eval-sweep refresh cannot create raw dir `{}` for `{subject_id}`: {error}",
            dir.display()
        )
    })?;
    for (name, bytes) in [("pass2-stdout.txt", stdout), ("pass2-stderr.txt", stderr)] {
        std::fs::write(dir.join(name), bytes).map_err(|error| {
            format!(
                "eval-sweep refresh cannot retain raw output `{name}` for `{subject_id}`: {error}"
            )
        })?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Candidate file writing and report rendering
// ---------------------------------------------------------------------------

fn write_candidate_file(staging: &Path, name: &str, value: &Value) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| format!("eval-sweep refresh cannot render {name}: {error}"))?;
    std::fs::write(staging.join(name), format!("{text}\n")).map_err(|error| {
        format!(
            "eval-sweep refresh cannot write `{}`: {error}",
            staging.join(name).display()
        )
    })
}

/// Moves every staged candidate file into its final location under `out`, one
/// rename per file, then removes the staging directory. A refresh interrupted
/// before this point leaves the published paths untouched (only staging
/// residue, cleared by the next run), so an interrupted run can never leave a
/// mixed old/new generation among the published candidate files.
fn finalize_candidate_generation(staging: &Path, out_abs: &Path) -> Result<(), String> {
    let mut stack = vec![staging.to_path_buf()];
    while let Some(current) = stack.pop() {
        let entries = std::fs::read_dir(&current).map_err(|error| {
            format!(
                "eval-sweep refresh cannot read the staging dir `{}`: {error}",
                current.display()
            )
        })?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let relative = path.strip_prefix(staging).map_err(|error| {
                format!(
                    "eval-sweep refresh cannot relativize staged file `{}`: {error}",
                    path.display()
                )
            })?;
            let final_path = out_abs.join(relative);
            if let Some(parent) = final_path.parent()
                && !parent.exists()
            {
                std::fs::create_dir_all(parent).map_err(|error| {
                    format!(
                        "eval-sweep refresh cannot create `{}`: {error}",
                        parent.display()
                    )
                })?;
            }
            // A rerun over an existing candidate replaces the published file;
            // remove-then-rename keeps the move working on platforms whose
            // rename refuses an existing destination.
            if final_path.exists() {
                std::fs::remove_file(&final_path).map_err(|error| {
                    format!(
                        "eval-sweep refresh cannot replace `{}`: {error}",
                        final_path.display()
                    )
                })?;
            }
            std::fs::rename(&path, &final_path).map_err(|error| {
                format!(
                    "eval-sweep refresh cannot move `{}` into place at `{}`: {error}",
                    path.display(),
                    final_path.display()
                )
            })?;
        }
    }
    std::fs::remove_dir_all(staging).map_err(|error| {
        format!(
            "eval-sweep refresh cannot remove the staging dir `{}`: {error}",
            staging.display()
        )
    })?;
    Ok(())
}

fn render_report_json(exec_receipt: &Value) -> Result<String, String> {
    serde_json::to_string_pretty(exec_receipt)
        .map_err(|error| format!("eval-sweep refresh cannot render the run report: {error}"))
}

fn render_report_markdown(exec_receipt: &Value) -> String {
    let mut out = String::new();
    out.push_str("# Eval Sweep Refresh (candidates)\n\n");
    out.push_str("Candidate currentness evidence only — accepted/current state untouched; promotion requires #3567.\n\n");
    if let Some(manifest) = exec_receipt.get("manifest") {
        out.push_str(&format!(
            "- manifest: {} (sha256 `{}`)\n",
            manifest.get("path").and_then(Value::as_str).unwrap_or("?"),
            manifest
                .get("sha256")
                .and_then(Value::as_str)
                .unwrap_or("?")
        ));
    }
    if let Some(ripr) = exec_receipt.get("ripr") {
        out.push_str(&format!(
            "- binary: {} (sha256 `{}`)\n",
            ripr.get("absolute_path")
                .and_then(Value::as_str)
                .unwrap_or("?"),
            ripr.get("binary_digest")
                .and_then(Value::as_str)
                .unwrap_or("?")
        ));
    }
    if let Some(host_class) = exec_receipt.get("host_class").and_then(Value::as_str) {
        out.push_str(&format!("- host class: {host_class}\n"));
    }
    if let Some(subjects) = exec_receipt.get("subjects").and_then(Value::as_array) {
        out.push_str(
            "\n| subject | status | materialization | stability |\n| --- | --- | --- | --- |\n",
        );
        for subject in subjects {
            out.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                subject.get("id").and_then(Value::as_str).unwrap_or("?"),
                subject.get("status").and_then(Value::as_str).unwrap_or("?"),
                subject
                    .pointer("/materialization/state")
                    .and_then(Value::as_str)
                    .unwrap_or("?"),
                subject
                    .pointer("/stability/gap_stable")
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "not-compared".to_string()),
            ));
        }
    }
    out.push_str(
        "\nNot a structural-accuracy, repair-correctness, gate, badge, or support claim.\n",
    );
    out
}

// ---------------------------------------------------------------------------
// Tests (module named `python_eval_sweep_refresh` so
// `cargo test -p xtask python_eval_sweep_refresh` selects exactly this module;
// no unwrap/expect — assert macros and Result returns only). Every test is
// offline: materialization goes through local seed clones (git local
// transport), never a network run.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod python_eval_sweep_refresh {
    use super::super::eval_sweep_check::check_artifacts;
    use super::*;

    const DIGEST_ONE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const VALID_SHA_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const VALID_SHA_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const VALID_SHA_C: &str = "cccccccccccccccccccccccccccccccccccccccc";
    const VALID_SHA_D: &str = "dddddddddddddddddddddddddddddddddddddddd";
    const VALID_SHA_E: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
    const VALID_SHA_F: &str = "ffffffffffffffffffffffffffffffffffffffff";
    const VALID_SHA_0: &str = "1010101010101010101010101010101010101010";
    const VALID_SHA_1: &str = "1111111111111111111111111111111111111111";

    fn temp_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "ripr-evalsweep-refresh-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ))
    }

    /// Resolves the built ripr binary anchored at the workspace root. The
    /// shared fixture builder resolves `target/debug/ripr` against the test
    /// process cwd (the xtask package dir), which is the wrong target dir for
    /// a workspace-member test run; the workspace root is `xtask/..`.
    fn built_ripr_binary() -> Result<String, String> {
        let binary_name = format!("ripr{}", std::env::consts::EXE_SUFFIX);
        let target_dir = match std::env::var_os("CARGO_TARGET_DIR") {
            Some(dir) => PathBuf::from(dir),
            None => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default()
                .join("target"),
        };
        let binary = target_dir.join("debug").join(binary_name);
        if !binary.is_file() {
            // Cold checkout: build through the shared fixture builder.
            return crate::ripr_fixture_binary();
        }
        std::path::absolute(&binary)
            .map(|path| path.to_string_lossy().to_string())
            .map_err(|error| format!("resolve built ripr binary failed: {error}"))
    }

    fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
        let mut owned: Vec<String> = vec!["-C".to_string(), dir.to_string_lossy().to_string()];
        owned.extend(args.iter().map(|argument| (*argument).to_string()));
        let output = capture_output_with_timeout(
            "git",
            &owned,
            &[("GIT_TERMINAL_PROMPT", "0")],
            Duration::from_mins(2),
            "python_eval_sweep_refresh test git",
        )?;
        if output.timed_out || !output.status.is_some_and(|status| status.success()) {
            return Err(format!("git {args:?} failed: {}", output.stderr.trim()));
        }
        Ok(output.stdout)
    }

    /// Creates a directory link (`link` -> `target`): a real symlink on Unix,
    /// a junction on Windows (no privilege required). Both resolve through
    /// `std::fs::canonicalize`, which is what the candidate-separation check
    /// must see through.
    #[cfg(unix)]
    fn create_dir_link(link: &Path, target: &Path) -> Result<(), String> {
        std::os::unix::fs::symlink(target, link).map_err(|error| {
            format!(
                "create symlink `{} -> {}`: {error}",
                link.display(),
                target.display()
            )
        })
    }

    #[cfg(windows)]
    fn create_dir_link(link: &Path, target: &Path) -> Result<(), String> {
        let output = capture_output_with_timeout(
            "cmd",
            &[
                "/c".to_string(),
                "mklink".to_string(),
                "/J".to_string(),
                link.to_string_lossy().to_string(),
                target.to_string_lossy().to_string(),
            ],
            &[],
            Duration::from_secs(30),
            "python_eval_sweep_refresh test junction",
        )?;
        if output.timed_out || !output.status.is_some_and(|status| status.success()) {
            return Err(format!(
                "mklink /J `{} -> {}` failed: {}",
                link.display(),
                target.display(),
                first_line(&output.stderr)
            ));
        }
        Ok(())
    }

    const SUBJECT_SHAPES: [&str; 8] = [
        "pytest_library",
        "unittest_library",
        "click_typer",
        "fastapi_web",
        "flask_web",
        "pytest_library",
        "pytest_library",
        "pytest_library",
    ];

    /// A synthetic Python subject seed: one tiny module whose boundary the
    /// synthetic diff flips, plus the language-enablement config. The commit
    /// SHA becomes the manifest pin, so materialization verifies exactly.
    fn build_seed(seed_dir: &Path) -> Result<String, String> {
        std::fs::create_dir_all(seed_dir).map_err(|error| format!("create seed: {error}"))?;
        let app = seed_dir.join("app.py");
        std::fs::write(
            &app,
            "def boundary(value):\n    if value >= 0:\n        return \"ok\"\n    return \"negative\"\n",
        )
        .map_err(|error| format!("write app.py: {error}"))?;
        std::fs::write(
            seed_dir.join("ripr.toml"),
            "[languages]\nenabled = [\"python\"]\n",
        )
        .map_err(|error| format!("write ripr.toml: {error}"))?;
        std::fs::write(
            seed_dir.join("test_app.py"),
            "from app import boundary\n\ndef test_zero():\n    assert boundary(0) == \"ok\"\n",
        )
        .map_err(|error| format!("write test_app.py: {error}"))?;
        git(seed_dir, &["init", "-q"])?;
        git(seed_dir, &["add", "."])?;
        git(
            seed_dir,
            &[
                "-c",
                "user.name=ripr-test",
                "-c",
                "user.email=ripr-test@example.invalid",
                "commit",
                "-q",
                "-m",
                "synthetic subject",
            ],
        )?;
        let head = git(seed_dir, &["rev-parse", "HEAD"])?;
        Ok(head.trim().to_string())
    }

    const SYNTHETIC_DIFF: &str = "diff --git a/app.py b/app.py\n--- a/app.py\n+++ b/app.py\n@@ -1,4 +1,4 @@\n def boundary(value):\n-    if value >= 0:\n+    if value > 0:\n         return \"ok\"\n     return \"negative\"\n";

    fn write_manifest_and_diffs(root: &Path, pins: &[String]) -> Result<PathBuf, String> {
        let diffs = root.join("diffs");
        std::fs::create_dir_all(&diffs).map_err(|error| format!("create diffs: {error}"))?;
        let mut repos = Vec::new();
        for (index, sha) in pins.iter().enumerate() {
            let id = format!("s{index}");
            std::fs::write(diffs.join(format!("{id}.diff")), SYNTHETIC_DIFF)
                .map_err(|error| format!("write diff: {error}"))?;
            repos.push(json!({
                "id": id,
                "url": format!("https://example.com/{id}"),
                "sha": sha,
                "license": "MIT",
                "shape": SUBJECT_SHAPES[index],
                "synthetic_diff": format!("diffs/{id}.diff"),
                "why": "synthetic offline refresh subject",
            }));
        }
        let manifest = json!({
            "schema_version": "0.1",
            "kind": "python_eval_sweep_manifest",
            "spec": "RIPR-SPEC-0086",
            "tier": "A",
            "description": "synthetic offline refresh subjects",
            "repos": repos,
        });
        let path = root.join("manifest.json");
        let text = serde_json::to_string_pretty(&manifest)
            .map_err(|error| format!("render manifest: {error}"))?;
        std::fs::write(&path, &text).map_err(|error| format!("write manifest: {error}"))?;
        Ok(path)
    }

    // -- authorization gate --------------------------------------------------

    /// Returns the route's typed error, or fails the test when the route
    /// succeeded (the expect_err shape without the banned method).
    fn refusal_of(result: Result<(), String>) -> Result<String, String> {
        match result {
            Ok(()) => Err("expected a typed refusal; the route succeeded".to_string()),
            Err(error) => Ok(error),
        }
    }

    #[test]
    fn refuses_without_managed_authorization() -> Result<(), String> {
        let error = refusal_of(check_network_authorization(false, None))?;
        assert!(error.contains(NETWORK_ENV), "names the env signal: {error}");
        assert!(error.contains("ordinary CI never refreshes live repositories"));

        let error = refusal_of(check_network_authorization(true, None))?;
        assert!(error.contains(NETWORK_ENV));

        let error = refusal_of(check_network_authorization(false, Some(NETWORK_ENV_VALUE)))?;
        assert!(error.contains(ALLOW_FLAG), "{error}");

        // A wrong env value is not the managed signal.
        let error = refusal_of(check_network_authorization(false, Some("0")))?;
        assert!(error.contains(NETWORK_ENV));

        assert!(matches!(
            check_network_authorization(true, Some(NETWORK_ENV_VALUE)),
            Ok(())
        ));
        assert!(matches!(
            check_network_authorization(true, Some("1")),
            Ok(())
        ));
        Ok(())
    }

    #[test]
    fn full_route_refuses_before_any_filesystem_work() -> Result<(), String> {
        // No env signal: the route refuses before reading any manifest, even
        // with default args (offline refusal is itself testable).
        let error = refusal_of(run_refresh_with_env(&[], None))?;
        assert!(error.contains("refuses to run"), "{error}");
        assert!(error.contains(NETWORK_ENV));
        assert!(error.contains(RERUN_COMMAND));
        Ok(())
    }

    #[test]
    fn authorized_route_requires_explicit_binary_and_out() -> Result<(), String> {
        // The authorization gate passes (flag + env), then the required
        // argument checks fire in order.
        let error = refusal_of(run_refresh_with_env(
            &["--allow-network".to_string()],
            Some("1"),
        ))?;
        assert!(error.contains("--ripr-bin"), "{error}");
        assert!(error.contains("PATH cannot select"));

        let error = refusal_of(run_refresh_with_env(
            &[
                "--allow-network".to_string(),
                "--ripr-bin".to_string(),
                "unused".to_string(),
            ],
            Some("1"),
        ))?;
        assert!(error.contains("--out"), "{error}");
        Ok(())
    }

    // -- candidate separation ------------------------------------------------

    #[test]
    fn rejects_out_overlapping_accepted_state() -> Result<(), String> {
        // Anchored at the repository root so the test is cwd-independent.
        let root = std::path::absolute(repo_root_anchor())
            .map_err(|error| format!("resolve root: {error}"))?;
        let fixtures = root.join("fixtures").to_string_lossy().to_string();
        let error = refusal_of(validate_out_separation(&fixtures).map(|_| ()))?;
        assert!(error.contains("accepted state"), "{error}");
        let accepted_manifest_dir = root
            .join("fixtures")
            .join("python-eval-sweep")
            .to_string_lossy()
            .to_string();
        let error = refusal_of(validate_out_separation(&accepted_manifest_dir).map(|_| ()))?;
        assert!(error.contains("accepted state"), "{error}");
        // The repository root itself (and any ancestor) is rejected. The
        // root contains the accepted fixtures tree, so the accepted-state
        // overlap check fires first; an ancestor above the root hits the
        // contains-the-repo-root check.
        let error = refusal_of(validate_out_separation(&root.to_string_lossy()).map(|_| ()))?;
        assert!(
            error.contains("accepted state") || error.contains("repository root or contains it"),
            "the repo root must fail: {error}"
        );
        // An ancestor above the repository root contains it (and the
        // accepted fixtures tree), so it is rejected. Anchored, never
        // cwd-relative: parallel tests transiently change the process cwd.
        let ancestor = root
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| "workspace root has a parent".to_string())?;
        let error = refusal_of(validate_out_separation(&ancestor.to_string_lossy()).map(|_| ()))?;
        assert!(
            error.contains("accepted state") || error.contains("repository root or contains it"),
            "an ancestor of the repo root must fail: {error}"
        );
        let error = refusal_of(validate_out_separation("").map(|_| ()))?;
        assert!(error.contains("non-empty"), "{error}");

        // A dedicated directory inside the repository root (the recommended
        // shape, e.g. under target/) is accepted.
        let ok = validate_out_separation(
            root.join("target")
                .join("ripr")
                .join("eval-sweep")
                .join("refresh")
                .join("candidates")
                .to_string_lossy()
                .as_ref(),
        )?;
        assert!(
            ok.to_string_lossy().contains("refresh"),
            "a dedicated candidate dir under target/ is accepted: {}",
            ok.display()
        );
        let _ = std::fs::remove_dir_all(&ok);
        Ok(())
    }

    /// An existing `--out` symlink that resolves into accepted state is
    /// refused: the containment comparison runs on canonicalized paths, so a
    /// `target/x -> fixtures/python-eval-sweep` link cannot launder an
    /// accepted-state write into a lexical pass. Unix uses a real symlink;
    /// Windows uses a junction (no privilege required); both resolve through
    /// `std::fs::canonicalize`.
    #[test]
    fn symlinked_out_into_accepted_state_is_refused() -> Result<(), String> {
        let root = std::path::absolute(repo_root_anchor())
            .map_err(|error| format!("resolve root: {error}"))?;
        let link = root
            .join("target")
            .join("ripr")
            .join("eval-sweep")
            .join(format!(
                "symlink-out-check-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
        let target = root.join("fixtures").join("python-eval-sweep");
        let _ = std::fs::remove_dir_all(&link);
        let _ = std::fs::remove_file(&link);
        if let Some(parent) = link.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("create link parent: {error}"))?;
        }
        create_dir_link(&link, &target)?;

        let refused =
            refusal_of(validate_out_separation(link.to_string_lossy().as_ref()).map(|_| ()));
        let _ = std::fs::remove_dir_all(&link);
        let _ = std::fs::remove_file(&link);
        let error = refused?;
        assert!(
            error.contains("accepted state"),
            "the symlinked --out must be refused as accepted-state overlap: {error}"
        );
        Ok(())
    }

    #[test]
    fn binary_path_must_name_existing_file() -> Result<(), String> {
        let error = refusal_of(resolve_binary("").map(|_| ()))?;
        assert!(error.contains("non-empty explicit binary path"), "{error}");
        let error = refusal_of(resolve_binary("ripr").map(|_| ()))?;
        assert!(
            error.contains("does not name an existing file"),
            "PATH cannot select: {error}"
        );
        let error = refusal_of(resolve_binary("target/debug/ripr-missing.exe").map(|_| ()))?;
        assert!(error.contains("PATH cannot select"), "{error}");
        Ok(())
    }

    // -- input resolution ----------------------------------------------------

    /// Diff resolution is anchored, never process-cwd dependent (#3735 N1):
    /// the manifest directory wins first, then the repository root — anchored
    /// at the compiled workspace, not the cwd. A decoy file at the declared
    /// relative path inside the process cwd must never win, and the canonical
    /// fixture layout (repo-root-relative) must resolve through the anchored
    /// root even from a foreign cwd: an absolute `--manifest` run from
    /// elsewhere must find its inputs instead of recording `tempfail` rows.
    #[test]
    fn resolve_diff_is_manifest_dir_first_then_anchored_repo_root() -> Result<(), String> {
        crate::tests::with_temp_cwd("resolve-diff", |cwd| -> Result<(), String> {
            let manifest_dir = cwd.join("manifest-home");
            std::fs::create_dir_all(manifest_dir.join("diffs"))
                .map_err(|error| format!("create manifest diffs: {error}"))?;
            std::fs::write(
                manifest_dir.join("diffs").join("x.diff"),
                b"manifest-dir bytes",
            )
            .map_err(|error| format!("write manifest diff: {error}"))?;
            // The decoy: the SAME relative path inside the process cwd. A
            // cwd-resolved lookup would find these bytes.
            std::fs::create_dir_all(cwd.join("diffs"))
                .map_err(|error| format!("create decoy: {error}"))?;
            std::fs::write(cwd.join("diffs").join("x.diff"), b"cwd decoy")
                .map_err(|error| format!("write decoy: {error}"))?;

            let resolved = resolve_diff(&manifest_dir, "diffs/x.diff")?;
            assert!(
                resolved.starts_with(&manifest_dir),
                "the manifest directory wins: {}",
                resolved.display()
            );
            let bytes = std::fs::read(&resolved).map_err(|error| error.to_string())?;
            assert_eq!(
                bytes, b"manifest-dir bytes",
                "the resolved diff is the manifest-dir file, never the cwd decoy"
            );

            // Repo-root fallback: the canonical accepted manifest declares its
            // diff repository-root-relative; that layout resolves through the
            // ANCHORED repo root even from this foreign cwd (which carries no
            // fixtures/ tree at all).
            let canonical = resolve_diff(
                &manifest_dir,
                "fixtures/python-eval-sweep/synthetic-diff.diff",
            )?;
            let repo_root = std::path::absolute(repo_root_anchor())
                .map_err(|error| format!("resolve repo root: {error}"))?;
            assert!(
                canonical.starts_with(&repo_root),
                "the repo-root-relative layout resolves through the anchored root: {}",
                canonical.display()
            );
            assert!(
                canonical.is_file(),
                "the fixture diff exists: {}",
                canonical.display()
            );

            // Found nowhere: typed error, never a cwd guess.
            let error = refusal_of(resolve_diff(&manifest_dir, "diffs/missing.diff").map(|_| ()))?;
            assert!(
                error.contains(
                    "not found relative to the manifest directory or the repository root"
                ),
                "{error}"
            );
            Ok(())
        })
    }

    // -- row assembly (pure) -------------------------------------------------

    fn test_subject(id: &str, sha: &str, shape: &str) -> AcceptedSubject {
        AcceptedSubject {
            id: id.to_string(),
            url: format!("https://example.com/{id}"),
            sha: sha.to_string(),
            license: "MIT".to_string(),
            shape: shape.to_string(),
            synthetic_diff: format!("diffs/{id}.diff"),
            tree_digest: None,
            snapshot: None,
            provenance: None,
            retention_class: None,
        }
    }

    fn test_binary() -> BinaryIdentity {
        BinaryIdentity {
            absolute: PathBuf::from("bin").join("ripr"),
            binary_digest: DIGEST_ONE.to_string(),
            version: "ripr 0.0.0-test".to_string(),
            build_profile: Some("debug".to_string()),
        }
    }

    fn analysis_pass(status: RunStatus, runtime_ms: u64) -> AnalysisPass {
        AnalysisPass {
            status,
            runtime_ms,
            complete: matches!(status, RunStatus::Complete),
            limitations: Vec::new(),
            gap_ids: BTreeSet::new(),
            classification: ClassificationCounts::default(),
            alignment: AlignmentCounts::default(),
            stdout: b"{\"findings\":[]}".to_vec(),
            stderr: Vec::new(),
            output_json: Some(json!({ "findings": [] })),
        }
    }

    fn materialized() -> Materialization {
        Materialization::Materialized {
            dir: PathBuf::from("subjects").join("x"),
            source: "local_seed_clone",
        }
    }

    fn stable_comparison() -> StabilityResult {
        StabilityResult::Compared {
            gap_stable: true,
            unstable_gap_ids: Vec::new(),
            output_identity_stable: true,
            repeat_raw_hex: sha256_hex(b"{\"findings\":[]}"),
            repeat_stdout: b"{\"findings\":[]}".to_vec(),
            repeat_stderr: Vec::new(),
        }
    }

    #[test]
    fn stale_materialization_keeps_subject_selected_without_counts() -> Result<(), String> {
        let subject = test_subject("stale", VALID_SHA_A, "pytest_library");
        let row = assemble_row(RowInputs {
            subject: &subject,
            binary: &test_binary(),
            diff_portable: "diffs/stale.diff",
            diff_digest: None,
            materialization: &Materialization::Stale {
                limitation: "pin unavailable".to_string(),
            },
            first: None,
            stability: None,
            corpus: None,
            config_profile: CONFIG_PROFILE_UNOBSERVED,
        });
        assert_eq!(row.status, "stale");
        assert!(!row.counts_as_run);
        assert!(row.runtime_ms.is_none());
        assert!(row.stability.is_none());
        let entry = row
            .value
            .as_object()
            .ok_or_else(|| "row must be a JSON object".to_string())?;
        assert_eq!(
            entry.get("materialization").and_then(Value::as_str),
            Some("failed"),
            "an unverifiable pinned tree is a failed materialization"
        );
        assert_eq!(
            entry.get("execution").and_then(Value::as_str),
            Some("not-executed")
        );
        assert!(
            entry.get("classification_counts").is_none(),
            "a row that did not run carries no analysis counts"
        );
        assert!(entry.get("runtime_ms").is_none());
        Ok(())
    }

    #[test]
    fn clone_failure_row_shape_is_tempfail() -> Result<(), String> {
        let subject = test_subject("tempfail", VALID_SHA_B, "unittest_library");
        let row = assemble_row(RowInputs {
            subject: &subject,
            binary: &test_binary(),
            diff_portable: "diffs/tempfail.diff",
            diff_digest: None,
            materialization: &Materialization::Failed {
                limitation: "clone refused".to_string(),
            },
            first: None,
            stability: None,
            corpus: None,
            config_profile: CONFIG_PROFILE_UNOBSERVED,
        });
        assert_eq!(row.status, "tempfail");
        assert!(!row.counts_as_run);
        let entry = row
            .value
            .as_object()
            .ok_or_else(|| "row must be a JSON object".to_string())?;
        assert!(entry.get("classification_counts").is_none());
        assert_eq!(
            entry.get("detection").and_then(Value::as_str),
            Some("absent")
        );
        Ok(())
    }

    #[test]
    fn corpus_classification_counts_by_real_path_shapes() -> Result<(), String> {
        let dir = temp_root("corpus");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).map_err(|error| error.to_string())?;
        std::fs::create_dir_all(dir.join("tests")).map_err(|error| error.to_string())?;
        std::fs::create_dir_all(dir.join("vendor")).map_err(|error| error.to_string())?;
        std::fs::create_dir_all(dir.join("gen")).map_err(|error| error.to_string())?;
        std::fs::write(dir.join("src").join("app.py"), "x = 1\n")
            .map_err(|error| error.to_string())?;
        std::fs::write(dir.join("tests").join("test_app.py"), "x = 1\n")
            .map_err(|error| error.to_string())?;
        std::fs::write(dir.join("vendor").join("dep.py"), "x = 1\n")
            .map_err(|error| error.to_string())?;
        std::fs::write(dir.join("gen").join("app_pb2.py"), "x = 1\n")
            .map_err(|error| error.to_string())?;
        std::fs::write(dir.join("notes.md"), "not python\n").map_err(|error| error.to_string())?;
        let counts = count_corpus(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(counts.complete);
        assert_eq!(counts.source_files, 1);
        assert_eq!(counts.test_files, 1);
        assert_eq!(counts.generated_files, 1);
        assert_eq!(counts.vendor_files, 1);
        Ok(())
    }

    /// An unreadable subtree is a truncated walk, not a smaller corpus
    /// (#3735 N3): the counts can never claim complete, so the row's
    /// corpus-selection state is `partial` (consistent with the working-set-
    /// cap rule) with the counts omitted, and the limitation names the failed
    /// subtree for the execution receipt. Windows denies the listing with an
    /// ACL; Unix removes the read permission.
    #[test]
    fn unreadable_subtree_marks_corpus_selection_incomplete() -> Result<(), String> {
        let dir = temp_root("corpus-unreadable");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).map_err(|error| error.to_string())?;
        std::fs::create_dir_all(dir.join("sealed")).map_err(|error| error.to_string())?;
        std::fs::write(dir.join("src").join("app.py"), "x = 1\n")
            .map_err(|error| error.to_string())?;
        std::fs::write(dir.join("sealed").join("hidden.py"), "x = 1\n")
            .map_err(|error| error.to_string())?;
        let sealed = dir.join("sealed");

        // Deny only the LISTING of `sealed/`: the walk still sees the
        // directory (metadata stays readable), then cannot enumerate it.
        #[cfg(windows)]
        let denied: Result<(), String> = capture_output_with_timeout(
            "icacls",
            &[
                sealed.to_string_lossy().to_string(),
                "/deny".to_string(),
                "*S-1-1-0:(OI)(CI)(RD)".to_string(),
            ],
            &[],
            Duration::from_secs(30),
            "python_eval_sweep_refresh test icacls deny",
        )
        .and_then(|output| {
            if output.timed_out || !output.status.is_some_and(|status| status.success()) {
                Err(format!(
                    "icacls deny failed: {}",
                    first_line(&output.stderr)
                ))
            } else {
                Ok(())
            }
        });
        #[cfg(unix)]
        let denied: Result<(), String> = {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&sealed, std::fs::Permissions::from_mode(0o000))
                .map_err(|error| format!("chmod sealed: {error}"))
        };
        denied.map_err(|error| format!("deny the sealed subtree: {error}"))?;

        let counts = count_corpus(&dir);

        // Restore access BEFORE asserting, so cleanup cannot fail.
        #[cfg(windows)]
        {
            let _ = capture_output_with_timeout(
                "icacls",
                &[
                    sealed.to_string_lossy().to_string(),
                    "/remove:d".to_string(),
                    "*S-1-1-0".to_string(),
                ],
                &[],
                Duration::from_secs(30),
                "python_eval_sweep_refresh test icacls restore",
            );
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&sealed, std::fs::Permissions::from_mode(0o755));
        }

        assert!(
            !counts.complete,
            "a walk that could not enumerate a subtree is never complete"
        );
        let limitation = counts
            .limitation
            .as_deref()
            .ok_or_else(|| "the truncated walk names its cause".to_string())?;
        assert!(
            limitation.contains("sealed"),
            "the limitation names the failed subtree: {limitation}"
        );
        assert_eq!(
            corpus_selection_state(Some(&counts)),
            "partial",
            "a truncated count cannot claim selected"
        );

        // The row cannot claim complete counts either: state `partial`, no
        // counts emitted (never a silently truncated number).
        let subject = test_subject("sealed", VALID_SHA_A, "pytest_library");
        let row = assemble_row(RowInputs {
            subject: &subject,
            binary: &test_binary(),
            diff_portable: "diffs/sealed.diff",
            diff_digest: None,
            materialization: &materialized(),
            first: None,
            stability: None,
            corpus: Some(&counts),
            config_profile: CONFIG_PROFILE_SUBJECT,
        });
        let corpus_json = row
            .value
            .get("corpus_selection")
            .cloned()
            .ok_or_else(|| "the row records its corpus selection".to_string())?;
        assert_eq!(corpus_json.get("state"), Some(&json!("partial")));
        assert!(
            corpus_json.get("source_files").is_none(),
            "truncated counts are omitted, not emitted: {corpus_json}"
        );

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    /// Candidate publication is generation-staged (#3735 N4): finalization
    /// moves every staged file into place by rename — replacing any previous
    /// generation's file on a rerun — and leaves no staging residue behind, so
    /// an interrupted refresh can never leave a mixed old/new generation in
    /// the published paths.
    #[test]
    fn staged_generation_finalizes_by_rename_without_residue() -> Result<(), String> {
        let root = temp_root("staging");
        let _ = std::fs::remove_dir_all(&root);
        let out = root.join("out");
        let staging = out.join(STAGING_DIR);
        std::fs::create_dir_all(staging.join(RAW_DIR).join("s0"))
            .map_err(|error| format!("create staging raw: {error}"))?;
        std::fs::write(
            staging.join(RAW_DIR).join("s0").join("pass1-stdout.txt"),
            b"pass one",
        )
        .map_err(|error| format!("write staged raw: {error}"))?;
        std::fs::write(staging.join(RECEIPT_FILE), b"{\"generation\": 2}\n")
            .map_err(|error| format!("write staged receipt: {error}"))?;

        finalize_candidate_generation(&staging, &out)?;

        let raw = std::fs::read(out.join(RAW_DIR).join("s0").join("pass1-stdout.txt"))
            .map_err(|error| error.to_string())?;
        assert_eq!(raw, b"pass one", "the staged raw bytes moved into place");
        let receipt = std::fs::read(out.join(RECEIPT_FILE)).map_err(|error| error.to_string())?;
        assert_eq!(
            receipt, b"{\"generation\": 2}\n",
            "the staged receipt moved into place"
        );
        assert!(
            !staging.exists(),
            "finalization leaves no staging residue behind"
        );

        // A rerun replaces the published generation instead of failing on an
        // existing destination.
        std::fs::create_dir_all(&staging).map_err(|error| format!("recreate staging: {error}"))?;
        std::fs::write(staging.join(RECEIPT_FILE), b"{\"generation\": 3}\n")
            .map_err(|error| format!("write second staged receipt: {error}"))?;
        finalize_candidate_generation(&staging, &out)?;
        let receipt =
            std::fs::read_to_string(out.join(RECEIPT_FILE)).map_err(|error| error.to_string())?;
        assert_eq!(
            receipt, "{\"generation\": 3}\n",
            "the rerun replaces the published generation"
        );
        assert!(!staging.exists());
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn evidence_digest_binds_raw_stderr_and_repeat() {
        let base = evidence_digest("aa", "bb", None);
        let with_repeat = evidence_digest("aa", "bb", Some("cc"));
        assert_ne!(base, with_repeat);
        assert_eq!(base, evidence_digest("aa", "bb", None));
        assert_eq!(evidence_digest("aa", "bb", Some("cc")), with_repeat);
    }

    #[test]
    fn deterministic_row_and_summary_assembly() -> Result<(), String> {
        let subject = test_subject("det", VALID_SHA_C, "click_typer");
        let binary = test_binary();
        let first = analysis_pass(RunStatus::Complete, 120);
        let corpus = CorpusCounts {
            source_files: 3,
            test_files: 2,
            generated_files: 0,
            vendor_files: 0,
            complete: true,
            limitation: None,
        };
        let build = || {
            assemble_row(RowInputs {
                subject: &subject,
                binary: &binary,
                diff_portable: "diffs/det.diff",
                diff_digest: Some(DIGEST_ONE.to_string()),
                materialization: &materialized(),
                first: Some(&first),
                stability: Some(&stable_comparison()),
                corpus: Some(&corpus),
                config_profile: CONFIG_PROFILE_SUBJECT,
            })
        };
        let a = build();
        let b = build();
        assert_eq!(a.value, b.value, "identical inputs assemble identical rows");
        let row_text_a = serde_json::to_string(&a.value).map_err(|error| error.to_string())?;
        let row_text_b = serde_json::to_string(&b.value).map_err(|error| error.to_string())?;
        assert_eq!(row_text_a, row_text_b);
        assert_eq!(a.status, "complete");
        assert_eq!(a.stability, Some(true));
        // The row records the configured identity honestly, not `default`.
        assert_eq!(
            a.value
                .get("config")
                .and_then(|config| config.get("profile"))
                .and_then(Value::as_str),
            Some(CONFIG_PROFILE_SUBJECT),
            "a subject-ripr-toml tree records the configured identity"
        );

        // The summary derivation is equally deterministic and reports the
        // stability aggregates when every run row carries repeat evidence.
        let execution = SubjectExecution {
            subject_id: subject.id.clone(),
            row: a,
            materialization_state: "materialized",
            materialization_source: Some("local_seed_clone"),
            materialization_limitation: None,
            analysis_complete: Some(true),
            analysis_limitations: Vec::new(),
            stability_performed: true,
            stability_limitation: None,
            output_identity_stable: Some(true),
            config_profile: CONFIG_PROFILE_SUBJECT.to_string(),
            subject_config_path: Some("ripr.toml".to_string()),
            corpus_state: "selected",
            corpus_limitation: None,
            infrastructure_limitation: None,
        };
        let executions = vec![execution];
        let summary_a = derive_summary(&executions);
        let summary_b = derive_summary(&executions);
        assert_eq!(summary_a, summary_b);
        assert_eq!(
            summary_a.get("gate_status").and_then(Value::as_str),
            Some("pass"),
            "one stable complete run derives the pass gate"
        );
        assert_eq!(
            summary_a
                .get("gap_id_stability_rate")
                .and_then(Value::as_f64),
            Some(1.0)
        );
        Ok(())
    }

    /// The producer/validator symmetry over the FULL 0.3 status vocabulary:
    /// rows for all eight terminal states, assembled by the refresh route's
    /// own `assemble_row`, validate through #3565's retained-receipt
    /// validator, and every status stays selected in the denominator.
    #[test]
    fn all_eight_statuses_produce_a_validatable_receipt() -> Result<(), String> {
        let shas = [
            VALID_SHA_A,
            VALID_SHA_B,
            VALID_SHA_C,
            VALID_SHA_D,
            VALID_SHA_E,
            VALID_SHA_F,
            VALID_SHA_0,
            VALID_SHA_1,
        ];
        let subjects: Vec<AcceptedSubject> = (0..8)
            .map(|index| test_subject(&format!("x{index}"), shas[index], SUBJECT_SHAPES[index]))
            .collect();
        let binary = test_binary();

        // Statuses in vocabulary order: complete, partial, parse-failed,
        // timed-out, crashed (the run statuses), then unsupported, tempfail,
        // stale (non-run).
        let statuses = [
            RunStatus::Complete,
            RunStatus::Partial,
            RunStatus::ParseFailed,
            RunStatus::TimedOut,
            RunStatus::Crashed,
            RunStatus::Unsupported,
            RunStatus::Tempfail,
            RunStatus::Stale,
        ];
        let mut executions = Vec::new();
        for (index, subject) in subjects.iter().enumerate() {
            let status = statuses[index];
            let (materialization, first) = match status {
                RunStatus::Tempfail => (
                    Materialization::Failed {
                        limitation: "clone refused".to_string(),
                    },
                    None,
                ),
                RunStatus::Stale => (
                    Materialization::Stale {
                        limitation: "pin unavailable".to_string(),
                    },
                    None,
                ),
                other => (
                    materialized(),
                    Some(analysis_pass(other, 100 + index as u64)),
                ),
            };
            let stability = if matches!(status, RunStatus::Complete) {
                Some(stable_comparison())
            } else {
                None
            };
            let row = assemble_row(RowInputs {
                subject,
                binary: &binary,
                diff_portable: &subject.synthetic_diff,
                diff_digest: Some(DIGEST_ONE.to_string()),
                materialization: &materialization,
                first: first.as_ref(),
                stability: stability.as_ref(),
                corpus: None,
                config_profile: CONFIG_PROFILE_SUBJECT,
            });
            executions.push(SubjectExecution {
                subject_id: subject.id.clone(),
                row,
                materialization_state: materialization_state(&materialization),
                materialization_source: None,
                materialization_limitation: None,
                analysis_complete: first.as_ref().map(|pass| pass.complete),
                analysis_limitations: Vec::new(),
                stability_performed: stability.is_some(),
                stability_limitation: None,
                output_identity_stable: None,
                config_profile: CONFIG_PROFILE_SUBJECT.to_string(),
                subject_config_path: None,
                corpus_state: corpus_selection_state(None),
                corpus_limitation: None,
                infrastructure_limitation: None,
            });
        }

        // The receipt binds to a manifest carrying exactly these subjects.
        let manifest_value = json!({
            "schema_version": "0.1",
            "kind": "python_eval_sweep_manifest",
            "spec": "RIPR-SPEC-0086",
            "tier": "A",
            "description": "synthetic full-vocabulary subjects",
            "repos": subjects
                .iter()
                .map(|subject| {
                    json!({
                        "id": subject.id,
                        "url": subject.url,
                        "sha": subject.sha,
                        "license": subject.license,
                        "shape": subject.shape,
                        "synthetic_diff": subject.synthetic_diff,
                    })
                })
                .collect::<Vec<_>>(),
        });
        let manifest_text =
            serde_json::to_string_pretty(&manifest_value).map_err(|error| error.to_string())?;
        let manifest_sha = sha256_hex(manifest_text.as_bytes());
        let parsed_manifest =
            crate::python_judged_panel::parse_json_without_duplicate_keys(&manifest_text)
                .map_err(|error| format!("manifest must parse: {error}"))?;
        let manifest = validate_accepted_manifest(&parsed_manifest, manifest_sha.clone())?;

        let receipt = assemble_receipt(&manifest_sha, &binary, &executions);
        let validated = validate_run_receipt(&receipt, &manifest_sha, &manifest, "receipt.json")
            .map_err(|error| format!("producer rows must validate: {error}"))?;
        assert_eq!(validated.denominator_selected, 8);
        assert_eq!(
            validated.denominator_run, 5,
            "the five run statuses count; unsupported/tempfail/stale do not"
        );

        let summary = receipt
            .get("summary")
            .cloned()
            .ok_or_else(|| "summary present".to_string())?;
        assert_eq!(summary.get("repos_total"), Some(&json!(8)));
        assert_eq!(summary.get("repos_run"), Some(&json!(5)));
        assert_eq!(summary.get("crash_count"), Some(&json!(1)));
        // A crashed row keeps the derived gate at review, and the stability
        // aggregates are omitted (not every run row carries repeat evidence).
        assert_eq!(
            summary.get("gate_status").and_then(Value::as_str),
            Some("review")
        );
        assert!(summary.get("gap_id_stability_rate").is_none());
        assert_eq!(
            validated.verdict(),
            super::super::eval_sweep_check::Verdict::Incomplete
        );
        Ok(())
    }

    #[test]
    fn unstable_repeat_comparison_records_the_mismatch_list() -> Result<(), String> {
        let subject = test_subject("unstable", VALID_SHA_D, "pytest_library");
        let binary = test_binary();
        let first = analysis_pass(RunStatus::Complete, 90);
        let comparison = StabilityResult::Compared {
            gap_stable: false,
            unstable_gap_ids: vec!["gap:python:app:boundary".to_string()],
            output_identity_stable: false,
            repeat_raw_hex: DIGEST_ONE.to_string(),
            repeat_stdout: Vec::new(),
            repeat_stderr: b"pass2 stderr".to_vec(),
        };
        let row = assemble_row(RowInputs {
            subject: &subject,
            binary: &binary,
            diff_portable: "diffs/unstable.diff",
            diff_digest: None,
            materialization: &materialized(),
            first: Some(&first),
            stability: Some(&comparison),
            corpus: None,
            config_profile: CONFIG_PROFILE_SUBJECT,
        });
        assert_eq!(row.stability, Some(false));
        let repeat = row
            .value
            .get("repeat")
            .cloned()
            .ok_or_else(|| "a compared row carries the repeat block".to_string())?;
        assert_eq!(repeat.get("gap_ids_stable"), Some(&json!(false)));
        assert_eq!(
            repeat.get("unstable_gap_ids"),
            Some(&json!(["gap:python:app:boundary"])),
            "a false stability claim carries its unstable gap-ID list"
        );
        assert_eq!(
            repeat.get("repeat_stderr"),
            Some(&json!(sha256_hex(b"pass2 stderr"))),
            "a compared row digests the retained second-pass stderr"
        );
        Ok(())
    }

    /// Non-complete first results never enter the repeat phase and never
    /// claim stability: the guard lives inside `run_stability_pass`, so no
    /// call site can compare repeats of crashed/timed-out/partial/
    /// parse-failed passes (whose equal failure gap sets would otherwise read
    /// as `stable`). The binary path is deliberately nonexistent: if the
    /// function attempted a repeat, the limitation would name a spawn
    /// failure instead of the first-pass status.
    #[test]
    fn non_complete_first_results_never_enter_the_repeat_phase() -> Result<(), String> {
        let binary = BinaryIdentity {
            absolute: PathBuf::from("definitely-missing-binary-path"),
            binary_digest: DIGEST_ONE.to_string(),
            version: "unused".to_string(),
            build_profile: None,
        };
        for status in [
            RunStatus::Partial,
            RunStatus::ParseFailed,
            RunStatus::TimedOut,
            RunStatus::Crashed,
            RunStatus::Unsupported,
            RunStatus::Tempfail,
            RunStatus::Stale,
        ] {
            let first = analysis_pass(status, 10);
            let result = run_stability_pass(
                &binary,
                Path::new("unused"),
                Path::new("unused"),
                Path::new("unused"),
                Duration::from_secs(1),
                "s",
                &first,
            );
            match result {
                StabilityResult::NotCompared { limitation } => {
                    assert!(
                        limitation.contains(status.as_str()),
                        "the refusal names the non-complete first status `{}`: {limitation}",
                        status.as_str()
                    );
                    assert!(
                        limitation.contains("runs only where the first result is complete"),
                        "the refusal states the complete-first gate, not a spawn failure: {limitation}"
                    );
                }
                StabilityResult::Compared { .. } => {
                    return Err(format!(
                        "a non-complete first result (`{}`) must never be compared",
                        status.as_str()
                    ));
                }
            }
        }
        Ok(())
    }

    /// F-E seam: the per-subject post-run identity re-verification detects a
    /// swapped binary. The identity file stands in for the built binary — a
    /// concurrent rebuild between two subjects' runs changes its bytes, and
    /// the mismatch is named (recorded vs post-run digest), never silently
    /// carried as a stale identity. An unreadable binary fails closed too.
    #[test]
    fn binary_swap_between_subjects_is_detected() -> Result<(), String> {
        let root = temp_root("drift-binary");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).map_err(|error| format!("create root: {error}"))?;
        let binary_path = root.join("ripr-under-test");
        std::fs::write(&binary_path, b"binary bytes subject 1-3")
            .map_err(|error| format!("write binary: {error}"))?;
        let recorded = sha256_hex(&std::fs::read(&binary_path).map_err(|error| error.to_string())?);
        let binary = BinaryIdentity {
            absolute: binary_path.clone(),
            binary_digest: recorded.clone(),
            version: "ripr 0.0.0-test".to_string(),
            build_profile: None,
        };
        assert!(
            binary_identity_drift(&binary).is_none(),
            "the identity holds before any swap"
        );

        // The concurrent rebuild lands between subjects.
        std::fs::write(&binary_path, b"REBUILT binary bytes subject 4")
            .map_err(|error| format!("swap binary: {error}"))?;
        let drift = binary_identity_drift(&binary)
            .ok_or_else(|| "the swapped binary must be detected as drift".to_string())?;
        assert!(
            drift.contains(&recorded),
            "the drift names the recorded digest: {drift}"
        );
        assert!(
            drift.contains(&sha256_hex(b"REBUILT binary bytes subject 4")),
            "the drift names the post-run digest: {drift}"
        );
        assert!(
            drift.contains("drifted") || drift.contains("attributed"),
            "the drift is a named typed disposition, not a silent identity: {drift}"
        );

        // A binary that cannot be re-read is fail-closed drift as well.
        std::fs::remove_file(&binary_path).map_err(|error| format!("remove binary: {error}"))?;
        let unreadable = binary_identity_drift(&binary)
            .ok_or_else(|| "an unreadable binary must fail closed".to_string())?;
        assert!(unreadable.contains("re-read"), "{unreadable}");
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    /// A post-materialization infrastructure failure (or binary-identity
    /// drift) keeps the subject selected as a typed tempfail row over the
    /// verified facts, with the cause named on the execution record — the
    /// denominator never shrinks.
    #[test]
    fn infrastructure_failure_row_keeps_the_denominator_shape() -> Result<(), String> {
        let subject = test_subject("infra", VALID_SHA_E, "flask_web");
        let execution = infrastructure_failure_execution(
            ReadyContext {
                subject: &subject,
                binary: &test_binary(),
                diff_portable: &subject.synthetic_diff,
                diff_digest: Some(DIGEST_ONE.to_string()),
                dir: PathBuf::from("subjects").join("infra"),
                source: "local_seed_clone",
                corpus: None,
                config_profile: CONFIG_PROFILE_SUBJECT.to_string(),
                subject_config_path: Some("ripr.toml".to_string()),
            },
            "the analysis pass could not run: spawn refused".to_string(),
        );
        assert_eq!(execution.row.status, "tempfail");
        assert!(!execution.row.counts_as_run);
        assert_eq!(execution.materialization_state, "materialized");
        assert_eq!(
            execution.infrastructure_limitation.as_deref(),
            Some("the analysis pass could not run: spawn refused")
        );
        assert!(execution.row.stability.is_none());
        let entry = execution
            .row
            .value
            .as_object()
            .ok_or_else(|| "row must be a JSON object".to_string())?;
        assert!(
            entry.get("classification_counts").is_none(),
            "a contained row carries no analysis counts"
        );
        assert!(
            entry.get("digests").and_then(Value::as_null).is_some(),
            "no captured pass means no digests (digests bind retained bytes)"
        );
        assert_eq!(
            entry
                .get("config")
                .and_then(|config| config.get("profile"))
                .and_then(Value::as_str),
            Some(CONFIG_PROFILE_SUBJECT),
            "the detected config identity survives the containment"
        );
        Ok(())
    }

    /// F-G: the config identity is honest — a subject root carrying its own
    /// `ripr.toml` (which `ripr check --root` loads) records the configured
    /// identity with the relative path; `default` only when the file is
    /// genuinely absent.
    #[test]
    fn subject_ripr_toml_is_recorded_not_default() -> Result<(), String> {
        let root = temp_root("config-detect");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).map_err(|error| format!("create root: {error}"))?;
        let (profile, path) = detect_subject_config(&root);
        assert_eq!(profile, CONFIG_PROFILE_DEFAULT, "no subject config file");
        assert_eq!(path, None);
        std::fs::write(
            root.join("ripr.toml"),
            "[languages]\nenabled = [\"python\"]\n",
        )
        .map_err(|error| format!("write ripr.toml: {error}"))?;
        let (profile, path) = detect_subject_config(&root);
        assert_eq!(profile, CONFIG_PROFILE_SUBJECT);
        assert_eq!(path.as_deref(), Some("ripr.toml"));
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    /// A subject whose cache dir cannot be created is contained as a
    /// `tempfail` row — the route still completes with the full eight-row
    /// candidate denominator, self-validated, with the cause named on the
    /// execution record. The cache path is sabotaged with a regular file, a
    /// deterministic stand-in for an uncreatable directory.
    #[test]
    fn cache_creation_failure_becomes_tempfail_and_route_completes() -> Result<(), String> {
        let root = temp_root("cache-fail");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).map_err(|error| format!("create root: {error}"))?;
        let mut pins = Vec::new();
        for index in 0..8 {
            let seed = root.join("seeds").join(format!("s{index}"));
            pins.push(build_seed(&seed)?);
        }
        let manifest_path = write_manifest_and_diffs(&root, &pins)?;

        // Sabotage s7's cache dir: a regular FILE where the route needs a
        // directory. Every other subject materializes and analyzes normally.
        let out = root.join("out");
        std::fs::create_dir_all(out.join(CACHE_DIR))
            .map_err(|error| format!("create cache root: {error}"))?;
        std::fs::write(out.join(CACHE_DIR).join("s7"), "not a directory")
            .map_err(|error| format!("write cache blocker: {error}"))?;

        let binary = built_ripr_binary()?;
        let args = vec![
            "--manifest".to_string(),
            manifest_path.to_string_lossy().to_string(),
            "--ripr-bin".to_string(),
            binary,
            "--out".to_string(),
            out.to_string_lossy().to_string(),
            "--checkout-root".to_string(),
            root.join("seeds").to_string_lossy().to_string(),
            "--timeout-secs".to_string(),
            "60".to_string(),
            "--allow-network".to_string(),
        ];
        let run = run_refresh_with_env(&args, Some("1"));
        if let Err(error) = &run {
            let _ = std::fs::remove_dir_all(&root);
            return Err(format!(
                "the route must contain the failure, not abort: {error}"
            ));
        }

        let receipt_text = std::fs::read_to_string(out.join(RECEIPT_FILE))
            .map_err(|error| format!("read receipt: {error}"))?;
        let receipt: Value = serde_json::from_str(&receipt_text)
            .map_err(|error| format!("parse receipt: {error}"))?;
        let rows = receipt
            .get("repos")
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| "receipt rows".to_string())?;
        assert_eq!(rows.len(), 8, "the denominator never shrinks: all 8 rows");
        let blocked = rows
            .iter()
            .find(|row| row.get("id").and_then(Value::as_str) == Some("s7"))
            .ok_or_else(|| "s7 row present".to_string())?;
        assert_eq!(
            blocked.get("status").and_then(Value::as_str),
            Some("tempfail"),
            "the cache failure is a contained tempfail row: {blocked}"
        );

        // The produced receipt still validates end to end.
        let outcome = check_artifacts(
            manifest_path.to_string_lossy().as_ref(),
            Some(out.join(RECEIPT_FILE).to_string_lossy().as_ref()),
        );
        if let Err(error) = &outcome {
            let _ = std::fs::remove_dir_all(&root);
            return Err(format!("the contained receipt must validate: {error}"));
        }

        // The execution receipt names the contained failure.
        let exec_text = std::fs::read_to_string(out.join(EXEC_RECEIPT_FILE))
            .map_err(|error| format!("read execution receipt: {error}"))?;
        let exec: Value = serde_json::from_str(&exec_text)
            .map_err(|error| format!("parse execution receipt: {error}"))?;
        let blocked_record = exec
            .get("subjects")
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| "execution receipt subjects".to_string())?
            .into_iter()
            .find(|subject| subject.get("id").and_then(Value::as_str) == Some("s7"))
            .ok_or_else(|| "s7 execution record".to_string())?;
        let limitation = blocked_record
            .get("infrastructure_limitation")
            .and_then(Value::as_str)
            .ok_or_else(|| "the contained failure must be named".to_string())?;
        assert!(
            limitation.contains("cache dir"),
            "the limitation names the cache-creation cause: {limitation}"
        );
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    /// The end-to-end symmetry proof, offline: refresh runs over synthetic
    /// local subjects (seed clones through git's local transport) with the
    /// real built binary, and the produced candidate receipt validates
    /// through the full `eval-sweep check` artifact path.
    #[test]
    fn refresh_candidate_validates_through_eval_sweep_check() -> Result<(), String> {
        let root = temp_root("e2e");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).map_err(|error| format!("create root: {error}"))?;

        // Eight local seeds; each seed's real HEAD becomes the manifest pin.
        let mut pins = Vec::new();
        for index in 0..8 {
            let seed = root.join("seeds").join(format!("s{index}"));
            pins.push(build_seed(&seed)?);
        }
        let manifest_path = write_manifest_and_diffs(&root, &pins)?;
        let binary = built_ripr_binary()?;
        let out = root.join("out");

        let args = vec![
            "--manifest".to_string(),
            manifest_path.to_string_lossy().to_string(),
            "--ripr-bin".to_string(),
            binary.clone(),
            "--out".to_string(),
            out.to_string_lossy().to_string(),
            "--checkout-root".to_string(),
            root.join("seeds").to_string_lossy().to_string(),
            "--timeout-secs".to_string(),
            "60".to_string(),
            "--allow-network".to_string(),
        ];
        let run = run_refresh_with_env(&args, Some("1"));
        if let Err(error) = &run {
            let _ = std::fs::remove_dir_all(&root);
            return Err(format!("authorized offline refresh must run: {error}"));
        }

        // The candidate receipt exists and validates through the exact
        // `eval-sweep check` artifact path (manifest + receipt, offline).
        let receipt_path = out.join(RECEIPT_FILE);
        let outcome = check_artifacts(
            manifest_path.to_string_lossy().as_ref(),
            Some(receipt_path.to_string_lossy().as_ref()),
        );
        if let Err(error) = &outcome {
            let _ = std::fs::remove_dir_all(&root);
            return Err(format!(
                "the produced candidate must pass eval-sweep check: {error}"
            ));
        }
        let outcome = outcome?;
        if outcome.verdict() != super::super::eval_sweep_check::Verdict::Incomplete {
            let _ = std::fs::remove_dir_all(&root);
            return Err(format!(
                "expected an incomplete verdict (the synthetic manifest records no optional identities), got {:?}",
                outcome.verdict()
            ));
        }
        assert_eq!(outcome.accepted.subjects.len(), 8);

        // Every subject stayed selected with one terminal row.
        let receipt_text = std::fs::read_to_string(&receipt_path)
            .map_err(|error| format!("read receipt: {error}"))?;
        let receipt: Value = serde_json::from_str(&receipt_text)
            .map_err(|error| format!("parse receipt: {error}"))?;
        let rows = receipt
            .get("repos")
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| "receipt rows".to_string())?;
        assert_eq!(rows.len(), 8);
        let mut seen = BTreeSet::new();
        for row in &rows {
            let id = row
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| "row id".to_string())?;
            seen.insert(id.to_string());
            let status = row
                .get("status")
                .and_then(Value::as_str)
                .ok_or_else(|| "status".to_string())?;
            assert!(
                [
                    "complete",
                    "partial",
                    "parse-failed",
                    "timed-out",
                    "crashed",
                    "unsupported",
                    "tempfail",
                    "stale"
                ]
                .contains(&status),
                "terminal status in the owned vocabulary, got {status}"
            );
            // Every synthetic seed carries a subject-root `ripr.toml`, and
            // every tree materialized, so the row records the configured
            // identity — never `default`.
            assert_eq!(
                row.pointer("/config/profile").and_then(Value::as_str),
                Some(CONFIG_PROFILE_SUBJECT),
                "{id} records the configured identity, not default"
            );
        }
        assert_eq!(seen.len(), 8, "all eight subjects received a row");

        // #3735 review: the offline synthetic route is deterministic, so the
        // PR's "8/8 complete" claim gets its oracle here — every row is
        // `complete` with both passes executed and compared.
        let complete_rows = rows
            .iter()
            .filter(|row| row.get("status").and_then(Value::as_str) == Some("complete"))
            .count();
        assert_eq!(complete_rows, 8, "all 8 rows complete offline: {rows:?}");
        for row in &rows {
            assert!(
                row.get("repeat").is_some(),
                "every complete row carries its stability comparison"
            );
        }

        // SPEC-0086 retention covers each pass: every compared (complete)
        // row retains its second-pass stdout AND stderr under raw/, and the
        // row's `repeat.repeat_stderr` digests exactly the retained bytes.
        let mut compared = 0usize;
        for row in &rows {
            let status = row.get("status").and_then(Value::as_str).unwrap_or("");
            let repeat = match row.get("repeat") {
                Some(repeat) => repeat.clone(),
                None => continue,
            };
            compared += 1;
            let id = row.get("id").and_then(Value::as_str).unwrap_or("?");
            let raw_dir = out.join(RAW_DIR).join(id);
            assert!(
                status == "complete",
                "only complete rows carry the compared repeat block, got {status}"
            );
            let stdout_path = raw_dir.join("pass2-stdout.txt");
            let stderr_path = raw_dir.join("pass2-stderr.txt");
            let stderr_bytes = std::fs::read(&stderr_path)
                .map_err(|error| format!("read {path}: {error}", path = stderr_path.display()))?;
            assert!(stdout_path.is_file(), "pass2 stdout retained for {id}");
            assert_eq!(
                repeat.get("repeat_stderr").and_then(Value::as_str),
                Some(sha256_hex(&stderr_bytes).as_str()),
                "the repeat block digests the retained second-pass stderr for {id}"
            );
        }
        assert!(compared > 0, "the synthetic subjects include complete runs");

        // The execution receipt names binary, manifest, host class, network
        // authorization, and exact subject identities.
        let exec_text = std::fs::read_to_string(out.join(EXEC_RECEIPT_FILE))
            .map_err(|error| format!("read execution receipt: {error}"))?;
        let exec: Value = serde_json::from_str(&exec_text)
            .map_err(|error| format!("parse execution receipt: {error}"))?;
        assert!(
            exec.pointer("/ripr/binary_digest")
                .and_then(Value::as_str)
                .is_some()
        );
        assert!(
            exec.pointer("/network_authorization/granted")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        );
        assert!(exec.get("host_class").and_then(Value::as_str).is_some());
        // The execution receipt retains the honest config identity with the
        // subject-config relative path.
        let record = exec
            .pointer("/subjects/0")
            .cloned()
            .ok_or_else(|| "first execution record".to_string())?;
        assert_eq!(
            record.pointer("/config/profile").and_then(Value::as_str),
            Some(CONFIG_PROFILE_SUBJECT)
        );
        assert_eq!(
            record
                .pointer("/config/subject_config")
                .and_then(Value::as_str),
            Some("ripr.toml")
        );
        assert_eq!(
            exec.pointer("/manifest/subject_ids")
                .and_then(Value::as_array)
                .map(|ids| ids.len()),
            Some(8)
        );
        // Generation staging leaves no residue: the published candidate paths
        // carry exactly this run's generation.
        assert!(
            !out.join(STAGING_DIR).exists(),
            "finalization must remove the staging directory: {}",
            out.join(STAGING_DIR).display()
        );

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    /// The stale-subject fail-closed path: a candidate materialization
    /// directory that is not a verified checkout keeps the subject selected
    /// as `stale` and the route still produces a validatable receipt
    /// denominator (local seeds cover the rest; no network anywhere).
    #[test]
    fn stale_subject_path_keeps_the_denominator_without_loss() -> Result<(), String> {
        let root = temp_root("stale-e2e");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).map_err(|error| format!("create root: {error}"))?;

        // Seven seeds; the eighth subject gets a non-git candidate dir, so
        // its pin cannot be verified and its row must be `stale`.
        let mut pins = Vec::new();
        for index in 0..7 {
            let seed = root.join("seeds").join(format!("s{index}"));
            pins.push(build_seed(&seed)?);
        }
        pins.push(VALID_SHA_A.to_string());
        let manifest_path = write_manifest_and_diffs(&root, &pins)?;
        // Pre-create s7's candidate dir as a plain (non-git) directory.
        std::fs::create_dir_all(root.join("out").join(SUBJECTS_DIR).join("s7"))
            .map_err(|error| format!("create stale dir: {error}"))?;

        let binary = built_ripr_binary()?;
        let out = root.join("out");
        let args = vec![
            "--manifest".to_string(),
            manifest_path.to_string_lossy().to_string(),
            "--ripr-bin".to_string(),
            binary,
            "--out".to_string(),
            out.to_string_lossy().to_string(),
            "--checkout-root".to_string(),
            root.join("seeds").to_string_lossy().to_string(),
            "--timeout-secs".to_string(),
            "60".to_string(),
            "--allow-network".to_string(),
        ];
        let run = run_refresh_with_env(&args, Some("1"));
        if let Err(error) = &run {
            let _ = std::fs::remove_dir_all(&root);
            return Err(format!("stale-subject refresh must still run: {error}"));
        }

        let receipt_text = std::fs::read_to_string(out.join(RECEIPT_FILE))
            .map_err(|error| format!("read receipt: {error}"))?;
        let receipt: Value = serde_json::from_str(&receipt_text)
            .map_err(|error| format!("parse receipt: {error}"))?;
        let rows = receipt
            .get("repos")
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| "receipt rows".to_string())?;
        assert_eq!(rows.len(), 8, "the denominator is retained");
        let stale_row = rows
            .iter()
            .find(|row| row.get("id").and_then(Value::as_str) == Some("s7"))
            .ok_or_else(|| "s7 row present".to_string())?;
        assert_eq!(
            stale_row.get("status").and_then(Value::as_str),
            Some("stale"),
            "the unverifiable subject is dispositioned stale, never complete: {stale_row}"
        );

        // The receipt still validates end to end.
        let outcome = check_artifacts(
            manifest_path.to_string_lossy().as_ref(),
            Some(out.join(RECEIPT_FILE).to_string_lossy().as_ref()),
        );
        if let Err(error) = &outcome {
            let _ = std::fs::remove_dir_all(&root);
            return Err(format!("stale-subject receipt must validate: {error}"));
        }
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    /// A reused checkout whose HEAD matches the pin but whose worktree is
    /// dirty is NOT treated as the pinned tree: the modified content cannot
    /// be attributed to the accepted sha, so the row is `stale` while the
    /// route still produces the full validatable denominator.
    #[test]
    fn dirty_reused_checkout_is_stale_not_the_pinned_tree() -> Result<(), String> {
        let root = temp_root("dirty-reuse");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).map_err(|error| format!("create root: {error}"))?;

        // Eight local seeds; each seed's real HEAD becomes the manifest pin.
        let mut pins = Vec::new();
        for index in 0..8 {
            let seed = root.join("seeds").join(format!("s{index}"));
            pins.push(build_seed(&seed)?);
        }
        let manifest_path = write_manifest_and_diffs(&root, &pins)?;

        // Pre-place s7's candidate dir as a clone of its seed at the pin —
        // HEAD matches — then modify a tracked file, so the content diverges
        // from the pinned tree while the HEAD identity still agrees.
        let reused = root.join("out").join(SUBJECTS_DIR).join("s7");
        git(&root, &["clone", "-q", "seeds/s7", "out/subjects/s7"])?;
        std::fs::write(
            reused.join("app.py"),
            "def boundary(value):\n    if value >= 0:\n        return \"MODIFIED\"\n    return \"negative\"\n",
        )
        .map_err(|error| format!("modify reused checkout: {error}"))?;
        let status = git(&reused, &["status", "--porcelain"])?;
        assert!(
            !status.trim().is_empty(),
            "the reused checkout must be dirty before the route runs"
        );

        let binary = built_ripr_binary()?;
        let out = root.join("out");
        let args = vec![
            "--manifest".to_string(),
            manifest_path.to_string_lossy().to_string(),
            "--ripr-bin".to_string(),
            binary,
            "--out".to_string(),
            out.to_string_lossy().to_string(),
            "--checkout-root".to_string(),
            root.join("seeds").to_string_lossy().to_string(),
            "--timeout-secs".to_string(),
            "60".to_string(),
            "--allow-network".to_string(),
        ];
        let run = run_refresh_with_env(&args, Some("1"));
        if let Err(error) = &run {
            let _ = std::fs::remove_dir_all(&root);
            return Err(format!("dirty-reuse refresh must still run: {error}"));
        }

        let receipt_text = std::fs::read_to_string(out.join(RECEIPT_FILE))
            .map_err(|error| format!("read receipt: {error}"))?;
        let receipt: Value = serde_json::from_str(&receipt_text)
            .map_err(|error| format!("parse receipt: {error}"))?;
        let rows = receipt
            .get("repos")
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| "receipt rows".to_string())?;
        assert_eq!(rows.len(), 8, "the denominator is retained");
        let stale_row = rows
            .iter()
            .find(|row| row.get("id").and_then(Value::as_str) == Some("s7"))
            .ok_or_else(|| "s7 row present".to_string())?;
        assert_eq!(
            stale_row.get("status").and_then(Value::as_str),
            Some("stale"),
            "the dirty reused checkout is dispositioned stale, never analyzed as the pinned tree: {stale_row}"
        );

        let outcome = check_artifacts(
            manifest_path.to_string_lossy().as_ref(),
            Some(out.join(RECEIPT_FILE).to_string_lossy().as_ref()),
        );
        if let Err(error) = &outcome {
            let _ = std::fs::remove_dir_all(&root);
            return Err(format!("dirty-reuse receipt must validate: {error}"));
        }
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    /// The HEAD-drifted re-checkout runs under the CONFIGURED
    /// `--clone-timeout-secs`, not a hardcoded default (#3735 N2): a
    /// re-checkout whose post-checkout hook sleeps far past the configured
    /// deadline is bounded by it, so the drifted subject lands `stale` and the
    /// route still produces the full validatable denominator. Under a
    /// hardcoded 600s deadline the same tree would have been re-checked out
    /// (the sleep would have finished) and analyzed.
    #[test]
    fn drifted_recheckout_runs_under_the_configured_clone_timeout() -> Result<(), String> {
        let root = temp_root("recheckout-timeout");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).map_err(|error| format!("create root: {error}"))?;

        // Eight local seeds; each seed's real HEAD becomes the manifest pin.
        let mut pins = Vec::new();
        for index in 0..8 {
            let seed = root.join("seeds").join(format!("s{index}"));
            pins.push(build_seed(&seed)?);
        }
        let manifest_path = write_manifest_and_diffs(&root, &pins)?;

        // Pre-place s7's candidate dir as a local clone whose HEAD has
        // DRIFTED past the pin (one extra empty commit), then install a
        // post-checkout hook that sleeps far past the configured deadline:
        // the route must attempt the pin's re-checkout, and that re-checkout
        // must die at the CONFIGURED deadline.
        git(&root, &["clone", "-q", "seeds/s7", "out/subjects/s7"])?;
        let drifted = root.join("out").join(SUBJECTS_DIR).join("s7");
        git(
            &drifted,
            &[
                "-c",
                "user.name=ripr-test",
                "-c",
                "user.email=ripr-test@example.invalid",
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                "drift past the pin",
            ],
        )?;
        let head = git(&drifted, &["rev-parse", "HEAD"])?.trim().to_string();
        assert_ne!(
            head, pins[7],
            "the reused checkout must be drifted before the route runs"
        );
        std::fs::create_dir_all(drifted.join(".git").join("hooks"))
            .map_err(|error| format!("create hooks dir: {error}"))?;
        let hook_path = drifted.join(".git").join("hooks").join("post-checkout");
        std::fs::write(&hook_path, b"#!/bin/sh\nsleep 60\n")
            .map_err(|error| format!("write post-checkout hook: {error}"))?;
        // Linux git skips hooks without the executable bit; Windows git runs
        // them regardless. Grant the bit so the discriminator works on both.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&hook_path, std::fs::Permissions::from_mode(0o755))
                .map_err(|error| format!("mark post-checkout hook executable: {error}"))?;
        }

        let binary = built_ripr_binary()?;
        let out = root.join("out");
        let args = vec![
            "--manifest".to_string(),
            manifest_path.to_string_lossy().to_string(),
            "--ripr-bin".to_string(),
            binary,
            "--out".to_string(),
            out.to_string_lossy().to_string(),
            "--checkout-root".to_string(),
            root.join("seeds").to_string_lossy().to_string(),
            "--timeout-secs".to_string(),
            "60".to_string(),
            "--clone-timeout-secs".to_string(),
            "2".to_string(),
            "--allow-network".to_string(),
        ];
        let run = run_refresh_with_env(&args, Some("1"));
        if let Err(error) = &run {
            let _ = std::fs::remove_dir_all(&root);
            return Err(format!(
                "recheckout-timeout refresh must still run: {error}"
            ));
        }

        let receipt_text = std::fs::read_to_string(out.join(RECEIPT_FILE))
            .map_err(|error| format!("read receipt: {error}"))?;
        let receipt: Value = serde_json::from_str(&receipt_text)
            .map_err(|error| format!("parse receipt: {error}"))?;
        let rows = receipt
            .get("repos")
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| "receipt rows".to_string())?;
        assert_eq!(rows.len(), 8, "the denominator is retained");
        let stale_row = rows
            .iter()
            .find(|row| row.get("id").and_then(Value::as_str) == Some("s7"))
            .ok_or_else(|| "s7 row present".to_string())?;
        assert_eq!(
            stale_row.get("status").and_then(Value::as_str),
            Some("stale"),
            "the re-checkout died at the configured deadline, so the drifted subject stays stale, never analyzed: {stale_row}"
        );

        let outcome = check_artifacts(
            manifest_path.to_string_lossy().as_ref(),
            Some(out.join(RECEIPT_FILE).to_string_lossy().as_ref()),
        );
        if let Err(error) = &outcome {
            let _ = std::fs::remove_dir_all(&root);
            return Err(format!("recheckout-timeout receipt must validate: {error}"));
        }
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }
}
