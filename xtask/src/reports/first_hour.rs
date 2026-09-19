//! Installed first-hour qualification harness — #1674 slices A (harness core)
//! and B (fixture + installed check journey).
//!
//! Report-only `cargo xtask first-hour`. Owns the installed-artifact
//! authority every later slice consumes: an explicit `.crate`/installed
//! executable identity, fixture roots, a per-step process/file/command
//! ledger, cleanup authority, and a deterministic receipt skeleton.
//! Product behavior is exercised only through the installed executable;
//! a PATH/worktree binary, existing cache, or helper API is never a
//! substitute.
//!
//! Slice B builds the disposable baseline repository (exact-boundary change
//! under a weak mid-range-only test) and runs the installed `check` journey
//! (human `Start here` front door plus JSON evidence) under fresh
//! cache/HOME roots.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use flate2::read::GzDecoder;
use serde_json::{Value, json};
use tar::Archive;

use crate::run;

use super::eval_sweep_check::sha256_hex;

const USAGE: &str = "\
cargo xtask first-hour --crate <path.crate> --prefix <clean-dir> --out <receipt-dir>
  --fixture-root <dir>
Installs the packaged candidate into a clean prefix, builds the disposable
baseline fixture, and runs the installed check journey (human Start-here
front door plus JSON evidence) under fresh cache/HOME roots. Records the
installed-artifact identity and journey evidence in the receipt.";

const RECEIPT_FILE: &str = "first-hour.json";
const SCHEMA_VERSION: &str = "0.1";

// ---------------------------------------------------------------------------
// Args
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct FirstHourArgs {
    crate_path: String,
    prefix: String,
    out: String,
    fixture_root: String,
}

fn take_value(args: &[String], index: &mut usize, flag: &str) -> Result<String, String> {
    *index += 1;
    args.get(*index)
        .cloned()
        .ok_or_else(|| format!("first-hour {flag} requires a value\n{USAGE}"))
}

fn parse_args(args: &[String]) -> Result<FirstHourArgs, String> {
    let mut crate_path: Option<String> = None;
    let mut prefix: Option<String> = None;
    let mut out: Option<String> = None;
    let mut fixture_root: Option<String> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--crate" => crate_path = Some(take_value(args, &mut index, "--crate")?),
            "--prefix" => prefix = Some(take_value(args, &mut index, "--prefix")?),
            "--out" => out = Some(take_value(args, &mut index, "--out")?),
            "--fixture-root" => {
                fixture_root = Some(take_value(args, &mut index, "--fixture-root")?);
            }
            "--help" | "-h" => return Err(USAGE.to_string()),
            other => return Err(format!("first-hour unknown argument `{other}`\n{USAGE}")),
        }
        index += 1;
    }
    Ok(FirstHourArgs {
        crate_path: crate_path.ok_or_else(|| format!("first-hour --crate is required\n{USAGE}"))?,
        prefix: prefix.ok_or_else(|| format!("first-hour --prefix is required\n{USAGE}"))?,
        out: out.ok_or_else(|| format!("first-hour --out is required\n{USAGE}"))?,
        fixture_root: fixture_root
            .ok_or_else(|| format!("first-hour --fixture-root is required\n{USAGE}"))?,
    })
}

// ---------------------------------------------------------------------------
// Installed-artifact identity
// ---------------------------------------------------------------------------

/// Identity of one installed candidate. Every later slice resolves product
/// invocations through exactly this executable; nothing else may stand in.
#[derive(Clone, Debug)]
struct InstalledSubject {
    crate_path: String,
    crate_sha256: String,
    prefix: PathBuf,
    executable: PathBuf,
    executable_sha256: String,
    executable_size: u64,
    version_output: String,
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let bytes =
        std::fs::read(path).map_err(|error| format!("read `{}`: {error}", path.display()))?;
    Ok(sha256_hex(&bytes))
}

/// Base directory for harness-owned staging: the process temp directory
/// unless it sits inside the enclosing Cargo workspace (observed when
/// `TMPDIR` points at a repo-local target dir), in which case the user's
/// home cache. An extracted candidate staged inside the workspace would
/// be discovered as a member and refused by `cargo install`.
fn staging_base() -> Result<PathBuf, String> {
    let temp = std::env::temp_dir();
    let workspace = std::env::current_dir()
        .ok()
        .and_then(|cwd| enclosing_workspace_root(&cwd));
    match workspace {
        Some(root) if temp.starts_with(&root) => std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join(".cache").join("ripr-first-hour"))
            .ok_or_else(|| {
                "temp dir sits inside the cargo workspace and HOME is unset".to_string()
            }),
        _ => Ok(temp),
    }
}

fn unix_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Ledger and cleanup authority
// ---------------------------------------------------------------------------

/// One retained step record: exact argv, cwd, outcome. The receipt replays
/// the journey from the ledger, never from memory.
#[derive(Clone, Debug)]
struct LedgerEntry {
    step: String,
    argv: Vec<String>,
    cwd: String,
    started_epoch_secs: u64,
    status: String,
}

struct Harness {
    roots: Vec<PathBuf>,
    ledger: Vec<LedgerEntry>,
}

/// Workspace root enclosing `dir`, if any: the nearest ancestor holding a
/// manifest that declares `[workspace]`.
fn enclosing_workspace_root(dir: &Path) -> Option<PathBuf> {
    let mut current = if dir.is_file() {
        dir.parent().map(Path::to_path_buf)
    } else {
        Some(dir.to_path_buf())
    };
    while let Some(candidate) = current {
        let manifest = candidate.join("Cargo.toml");
        if manifest.is_file()
            && let Ok(text) = std::fs::read_to_string(&manifest)
            && text.lines().any(|line| line.trim() == "[workspace]")
        {
            return Some(candidate);
        }
        current = candidate.parent().map(Path::to_path_buf);
    }
    None
}

/// Absolute, lexically normalized form of a caller-supplied path for
/// containment checks. Relative paths resolve against the current
/// directory; `.` pops nothing and `..` pops one normal component, so a
/// `..` that would escape the anchor is refused instead of silently
/// changing which tree a check compares. Symlinked ancestors of
/// not-yet-existing paths cannot be resolved — callers pass paths that
/// must not exist yet, so the check stays lexical and fail-closed.
fn absolute_normalized(path: &Path) -> Result<PathBuf, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| format!("current dir: {error}"))?
            .join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        use std::path::Component::{CurDir, Normal, ParentDir, Prefix, RootDir};
        match component {
            Prefix(_) | RootDir => normalized.push(component.as_os_str()),
            CurDir => {}
            ParentDir => {
                if !normalized.pop() {
                    return Err(format!(
                        "first-hour path `{}` escapes its anchor",
                        path.display()
                    ));
                }
            }
            Normal(_) => normalized.push(component.as_os_str()),
        }
    }
    Ok(normalized)
}

impl Harness {
    /// Creates a harness-owned staging directory registered for Drop-guard
    /// cleanup. Only directories this function creates are ever registered:
    /// caller-supplied paths are never adopted for deletion. Ownership is
    /// claimed with exclusive creation: a fresh salted candidate is created
    /// with `create_dir`, and an `AlreadyExists` collision retries with a
    /// new candidate instead of deleting whatever occupies the path — the
    /// harness never removes a directory it did not create. Staging is
    /// always placed outside the enclosing Cargo workspace: an extracted
    /// package staged inside it would be discovered as a workspace member
    /// and `cargo install` would refuse it.
    fn owned_staging(&mut self, label: &str) -> Result<PathBuf, String> {
        let base = staging_base()?;
        Self::owned_staging_in(self, &base, label)
    }

    /// Staging creation under an explicit base, so tests can prove a
    /// not-yet-existing base (the home-cache fallback on a fresh runner) is
    /// created, not assumed.
    fn owned_staging_in(harness: &mut Self, base: &Path, label: &str) -> Result<PathBuf, String> {
        // The base itself (temp dir or home cache fallback) may not exist
        // yet — the fallback is only constructed, never created. Creating
        // it is safe: create_dir_all never deletes, and exclusivity below
        // still applies to the candidate, never the base.
        std::fs::create_dir_all(base)
            .map_err(|error| format!("create {label} staging base: {error}"))?;
        for attempt in 0..100 {
            let root = base.join(format!(
                "ripr-first-hour-{label}-{}-{:?}-{attempt}-{}",
                std::process::id(),
                std::thread::current().id(),
                unix_epoch_secs()
            ));
            match std::fs::create_dir(&root) {
                Ok(()) => {
                    harness.roots.push(root.clone());
                    return Ok(root);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(format!("create {label} staging: {error}"));
                }
            }
        }
        Err(format!(
            "create {label} staging: no free candidate after 100 tries"
        ))
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        for root in &self.roots {
            let _ = std::fs::remove_dir_all(root);
        }
    }
}

// ---------------------------------------------------------------------------
// Install into a clean prefix
// ---------------------------------------------------------------------------

fn executable_name() -> String {
    format!("ripr{}", std::env::consts::EXE_SUFFIX)
}

/// Extracts a `.crate` archive into an owned staging directory, enforcing
/// the packaged-crate shape (one top-level root, no traversal/absolute/link
/// members — the same admission policy as the release installer). Returns
/// the extracted package root for `cargo install --path`.
fn extract_crate_archive(harness: &mut Harness, archive: &Path) -> Result<PathBuf, String> {
    let staging = harness.owned_staging("crate-extract")?;
    let file = std::fs::File::open(archive)
        .map_err(|error| format!("open package archive `{}`: {error}", archive.display()))?;
    let mut tar = Archive::new(GzDecoder::new(file));
    let mut expected_root: Option<PathBuf> = None;
    for (index, entry_result) in tar
        .entries()
        .map_err(|error| format!("read package archive entries: {error}"))?
        .enumerate()
    {
        let mut entry =
            entry_result.map_err(|error| format!("read package entry {index}: {error}"))?;
        let enclosed = entry
            .path()
            .map_err(|error| format!("read package entry {index} path: {error}"))?
            .into_owned();
        if enclosed.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        }) {
            return Err(format!(
                "package entry {enclosed:?} escapes extraction root"
            ));
        }
        let mut root = PathBuf::new();
        root.push(
            enclosed
                .components()
                .next()
                .ok_or_else(|| format!("package entry {index} is empty"))?,
        );
        match &expected_root {
            Some(expected) if *expected == root => {}
            Some(expected) => {
                return Err(format!(
                    "package entry {enclosed:?} is outside expected root {expected:?}"
                ));
            }
            None => expected_root = Some(root),
        }
        if entry.header().entry_type().is_symlink() || entry.header().entry_type().is_hard_link() {
            return Err(format!("package entry {enclosed:?} is a link"));
        }
        let output = staging.join(&enclosed);
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("create package parent: {error}"))?;
        }
        entry
            .unpack(&output)
            .map_err(|error| format!("extract package entry {enclosed:?}: {error}"))?;
    }
    let root = expected_root.ok_or_else(|| "package archive holds no entries".to_string())?;
    let package_root = staging.join(root);
    if !package_root.join("Cargo.toml").is_file() {
        return Err(format!(
            "extracted package root `{}` holds no Cargo.toml",
            package_root.display()
        ));
    }
    Ok(package_root)
}

/// Installs the packaged candidate into a clean prefix with ordinary Cargo
/// installation tooling and binds the installed identity. The prefix must
/// not exist: reinstalling over a live prefix would mix generations. The
/// `.crate` archive is extracted into harness-owned staging first: `cargo
/// install --path` requires a package directory, never the archive itself.
fn install_package(
    harness: &mut Harness,
    crate_path: &str,
    prefix: &Path,
) -> Result<InstalledSubject, String> {
    if prefix.exists() {
        return Err(format!(
            "first-hour --prefix `{}` already exists; slice A never reinstalls over a live prefix",
            prefix.display()
        ));
    }
    let crate_digest = sha256_file(Path::new(crate_path))?;
    let package_root = extract_crate_archive(harness, Path::new(crate_path))?;
    // `--locked`: the shipped package carries its Cargo.lock. Fresh index
    // resolution drifts dependency versions (observed: unicode-ident /
    // unicode-properties skew breaking the build); qualification installs
    // the exact locked bytes or fails loudly.
    let argv = vec![
        "install".to_string(),
        "--locked".to_string(),
        "--root".to_string(),
        prefix.to_string_lossy().to_string(),
        "--path".to_string(),
        package_root.to_string_lossy().to_string(),
    ];
    let step_argv = std::iter::once("cargo".to_string())
        .chain(argv.clone())
        .collect::<Vec<_>>();
    let cwd = std::env::current_dir().map_err(|error| format!("current dir: {error}"))?;
    let started = unix_epoch_secs();
    let status = run::run_output_owned("cargo", &argv)
        .map(|_| "installed".to_string())
        .map_err(|error| format!("cargo install failed: {error}"));
    harness.ledger.push(LedgerEntry {
        step: "install".to_string(),
        argv: step_argv,
        cwd: cwd.to_string_lossy().to_string(),
        started_epoch_secs: started,
        status: status.clone().unwrap_or_else(|error| error),
    });
    status?;
    // `cargo install --root` places executables under `<prefix>/bin` on
    // every host; Windows adds `.exe` through `executable_name`.
    let executable = prefix.join("bin").join(executable_name());
    if !executable.is_file() {
        return Err(format!(
            "cargo install reported success but `{}` is absent",
            executable.display()
        ));
    }
    let executable_sha256 = sha256_file(&executable)?;
    let executable_size = std::fs::metadata(&executable)
        .map_err(|error| format!("stat installed binary: {error}"))?
        .len();
    let version_argv = vec!["--version".to_string()];
    let step_argv = vec![
        executable.to_string_lossy().to_string(),
        "--version".to_string(),
    ];
    let started_version = unix_epoch_secs();
    let version_output = run::run_output_owned(&executable.to_string_lossy(), &version_argv)
        .map_err(|error| format!("installed --version failed: {error}"))?;
    harness.ledger.push(LedgerEntry {
        step: "installed-version".to_string(),
        argv: step_argv,
        cwd: cwd.to_string_lossy().to_string(),
        started_epoch_secs: started_version,
        status: "recorded".to_string(),
    });
    Ok(InstalledSubject {
        crate_path: crate_path.to_string(),
        crate_sha256: crate_digest,
        prefix: prefix.to_path_buf(),
        executable,
        executable_sha256,
        executable_size,
        version_output: version_output.trim().to_string(),
    })
}

/// Refuses any executable that is not the installed identity. Later slices
/// resolve every product invocation through this check, so a PATH/worktree
/// binary can never silently substitute for the candidate.
fn admit_installed_executable(subject: &InstalledSubject, candidate: &Path) -> Result<(), String> {
    let digest = sha256_file(candidate)?;
    if digest != subject.executable_sha256 {
        return Err(format!(
            "wrong binary: `{}` digest does not match the installed identity; refusing",
            candidate.display()
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Slice B: fixture journey (baseline + installed check evidence)
// ---------------------------------------------------------------------------

/// Fixture repository layout: the checkout path itself carries spaces and
/// non-ASCII text, so the journey proves argv-array invocation (never shell
/// interpolation) survives hostile-but-legal paths.
const FIXTURE_REPO_REL: [&str; 3] = ["first hour", "grüße", "repo"];

const FIXTURE_CARGO_TOML: &str =
    "[package]\nname = \"fixture-firsthour\"\nversion = \"0.1.0\"\nedition = \"2021\"\n";
const FIXTURE_LIB_BASE: &str = "pub fn fee(order_total_cents: u64) -> u64 {\n    if order_total_cents > 10_000 {\n        500\n    } else {\n        0\n    }\n}\n";
const FIXTURE_LIB_HEAD: &str = "pub fn fee(order_total_cents: u64) -> u64 {\n    if order_total_cents >= 10_000 {\n        500\n    } else {\n        0\n    }\n}\n";
const FIXTURE_TEST: &str =
    "#[test]\nfn mid_range_pays_no_fee() {\n    assert_eq!(fixture_firsthour::fee(100), 0);\n}\n";

/// Fixed commit timestamps keep fixture SHAs deterministic across runs.
const FIXTURE_BASE_DATE: &str = "2026-01-01T00:00:00Z";
const FIXTURE_HEAD_DATE: &str = "2026-01-02T00:00:00Z";

/// Human marker the journey requires in the installed human output: the
/// exact `Start here` section header observed in the real rendering.
const HUMAN_START_HERE: &str = "Start here:";

struct FixtureRepo {
    path: PathBuf,
    base_sha: String,
    head_sha: String,
}

/// Runs one child with the harness ledger recording the exact argv and the
/// real child cwd. Commands that must run inside the fixture repo use the
/// tool's own location flag (`git -C`, `ripr --root`); the process cwd is
/// never mutated, so parallel lanes cannot disturb each other.
fn journey_run(
    harness: &mut Harness,
    step: &str,
    program: &str,
    args: &[String],
    envs: &[(&str, &str)],
) -> Result<String, String> {
    let mut argv = vec![program.to_string()];
    argv.extend(args.iter().cloned());
    let cwd = std::env::current_dir().map_err(|error| format!("current dir: {error}"))?;
    let started = unix_epoch_secs();
    let outcome = run::run_output_owned_with_envs(program, args, envs);
    let status = match &outcome {
        Ok(_) => "ok".to_string(),
        Err(error) => error.clone(),
    };
    harness.ledger.push(LedgerEntry {
        step: step.to_string(),
        argv,
        cwd: cwd.to_string_lossy().to_string(),
        started_epoch_secs: started,
        status,
    });
    outcome
}

fn fixture_git(
    harness: &mut Harness,
    repo: &Path,
    args: &[&str],
    date: &str,
) -> Result<String, String> {
    let repo_arg = repo.to_string_lossy().to_string();
    let owned = args
        .iter()
        .map(|arg| (*arg).to_string())
        .collect::<Vec<_>>();
    let mut full = vec![
        "-C".to_string(),
        repo_arg,
        "-c".to_string(),
        "user.name=first-hour".to_string(),
        "-c".to_string(),
        "user.email=first-hour@example.com".to_string(),
        "-c".to_string(),
        "commit.gpgsign=false".to_string(),
    ];
    full.extend(owned);
    journey_run(
        harness,
        "fixture-git",
        "git",
        &full,
        &[
            ("GIT_AUTHOR_NAME", "first-hour"),
            ("GIT_AUTHOR_EMAIL", "first-hour@example.com"),
            ("GIT_COMMITTER_NAME", "first-hour"),
            ("GIT_COMMITTER_EMAIL", "first-hour@example.com"),
            ("GIT_AUTHOR_DATE", date),
            ("GIT_COMMITTER_DATE", date),
        ],
    )
}

/// Builds the disposable baseline repository: base commit (production +
/// weak mid-range-only test), head commit (exact-boundary `>` to `>=`
/// change the weak test cannot discriminate). Returns base/head SHAs.
fn build_fixture_repo(harness: &mut Harness, fixture_root: &Path) -> Result<FixtureRepo, String> {
    let mut repo = fixture_root.to_path_buf();
    for component in FIXTURE_REPO_REL {
        repo.push(component);
    }
    let src = repo.join("src");
    let tests = repo.join("tests");
    std::fs::create_dir_all(&src).map_err(|error| format!("create fixture src: {error}"))?;
    std::fs::create_dir_all(&tests).map_err(|error| format!("create fixture tests: {error}"))?;
    std::fs::write(repo.join("Cargo.toml"), FIXTURE_CARGO_TOML)
        .map_err(|error| format!("write fixture Cargo.toml: {error}"))?;
    std::fs::write(src.join("lib.rs"), FIXTURE_LIB_BASE)
        .map_err(|error| format!("write fixture lib: {error}"))?;
    std::fs::write(tests.join("boundary.rs"), FIXTURE_TEST)
        .map_err(|error| format!("write fixture test: {error}"))?;
    fixture_git(
        harness,
        &repo,
        &["init", "-qb", "main", "."],
        FIXTURE_BASE_DATE,
    )?;
    fixture_git(harness, &repo, &["add", "-A"], FIXTURE_BASE_DATE)?;
    fixture_git(
        harness,
        &repo,
        &["commit", "-qm", "base"],
        FIXTURE_BASE_DATE,
    )?;
    let base_sha = fixture_git(harness, &repo, &["rev-parse", "HEAD"], FIXTURE_BASE_DATE)?
        .trim()
        .to_string();
    std::fs::write(src.join("lib.rs"), FIXTURE_LIB_HEAD)
        .map_err(|error| format!("write fixture head lib: {error}"))?;
    fixture_git(
        harness,
        &repo,
        &["commit", "-qam", "head"],
        FIXTURE_HEAD_DATE,
    )?;
    let head_sha = fixture_git(harness, &repo, &["rev-parse", "HEAD"], FIXTURE_HEAD_DATE)?
        .trim()
        .to_string();
    Ok(FixtureRepo {
        path: repo,
        base_sha,
        head_sha,
    })
}

/// Observed evidence from one installed `ripr check` JSON rendering.
#[derive(Clone, Debug, PartialEq, Eq)]
struct CheckEvidence {
    findings: usize,
    classifications: Vec<String>,
    summary_probes: u64,
}

/// Parses installed JSON output and requires at least one finding: a
/// journey that finds nothing proves the invocation ran, not that the
/// installed binary discriminates the boundary change.
fn check_evidence_json(stdout: &str) -> Result<CheckEvidence, String> {
    let parsed: Value =
        serde_json::from_str(stdout).map_err(|error| format!("parse installed JSON: {error}"))?;
    let findings = parsed
        .get("findings")
        .and_then(Value::as_array)
        .ok_or_else(|| "installed JSON holds no findings array".to_string())?;
    if findings.is_empty() {
        return Err(
            "installed JSON reports zero findings; the boundary change went unobserved".to_string(),
        );
    }
    let mut classifications = Vec::new();
    for finding in findings {
        if let Some(classification) = finding.get("classification").and_then(Value::as_str)
            && !classifications.contains(&classification.to_string())
        {
            classifications.push(classification.to_string());
        }
    }
    let summary_probes = parsed
        .get("summary")
        .and_then(|summary| summary.get("probes"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    Ok(CheckEvidence {
        findings: findings.len(),
        classifications,
        summary_probes,
    })
}

/// Requires the fixture's designed oracle: exactly one finding classified
/// `weakly_exposed`. The mid-range-only test cannot discriminate the
/// exact-boundary change, so `weakly_exposed` is the earned strength — a
/// promotion to `exposed` would be the false-confidence family (a strong
/// oracle must observe the changed sink), and any other shape means the
/// fixture no longer proves what it claims. Either case fails loudly for
/// fixture redesign instead of recording a passing slice B.
fn require_boundary_oracle(evidence: &CheckEvidence) -> Result<(), String> {
    if evidence.findings == 1 && evidence.classifications == ["weakly_exposed".to_string()] {
        Ok(())
    } else {
        Err(format!(
            "installed journey must observe exactly one weakly_exposed finding; observed {} finding(s) [{}]; refusing a pass the fixture cannot prove",
            evidence.findings,
            evidence.classifications.join(",")
        ))
    }
}

/// Requires the installed human rendering to contain its `Start here`
/// action section: exit 0 plus JSON alone would not prove the human front
/// door renders.
fn require_start_here(human: &str) -> Result<(), String> {
    if human.contains(HUMAN_START_HERE) {
        Ok(())
    } else {
        Err("installed human output holds no `Start here:` section".to_string())
    }
}

struct JourneyEvidence {
    repo_rel: String,
    base_sha: String,
    head_sha: String,
    human_digest: String,
    human_bytes: usize,
    json_digest: String,
    json_bytes: usize,
    evidence: CheckEvidence,
}

/// Runs the installed check journey (issue steps 1-2): human rendering for
/// the `Start here` front door plus JSON evidence, both through the admitted
/// installed executable under fresh cache/HOME roots.
fn run_check_journey(
    harness: &mut Harness,
    subject: &InstalledSubject,
    repo: &FixtureRepo,
    fixture_root: &Path,
) -> Result<JourneyEvidence, String> {
    admit_installed_executable(subject, &subject.executable)?;
    let cache_dir = fixture_root.join("cache").join("ripr");
    let home_dir = fixture_root.join("home");
    std::fs::create_dir_all(&cache_dir)
        .map_err(|error| format!("create journey cache: {error}"))?;
    std::fs::create_dir_all(&home_dir).map_err(|error| format!("create journey home: {error}"))?;
    let executable = subject.executable.to_string_lossy().to_string();
    let root = repo.path.to_string_lossy().to_string();
    let human_args = vec![
        "check".to_string(),
        "--root".to_string(),
        root.clone(),
        "--base".to_string(),
        repo.base_sha.clone(),
    ];
    let human = journey_run(
        harness,
        "installed-check-human",
        &executable,
        &human_args,
        &[
            ("RIPR_CACHE_DIR", &cache_dir.to_string_lossy()),
            ("HOME", &home_dir.to_string_lossy()),
        ],
    )
    .map_err(|error| format!("installed check (human) failed: {error}"))?;
    require_start_here(&human)?;
    let mut json_args = human_args.clone();
    json_args.push("--json".to_string());
    let json_stdout = journey_run(
        harness,
        "installed-check-json",
        &executable,
        &json_args,
        &[
            ("RIPR_CACHE_DIR", &cache_dir.to_string_lossy()),
            ("HOME", &home_dir.to_string_lossy()),
        ],
    )
    .map_err(|error| format!("installed check (json) failed: {error}"))?;
    let evidence = check_evidence_json(&json_stdout)?;
    require_boundary_oracle(&evidence)?;
    Ok(JourneyEvidence {
        repo_rel: FIXTURE_REPO_REL.join("/"),
        base_sha: repo.base_sha.clone(),
        head_sha: repo.head_sha.clone(),
        human_digest: sha256_hex(human.as_bytes()),
        human_bytes: human.len(),
        json_digest: sha256_hex(json_stdout.as_bytes()),
        json_bytes: json_stdout.len(),
        evidence,
    })
}

fn journey_json(evidence: &JourneyEvidence) -> Value {
    json!({
        "repo": evidence.repo_rel,
        "base_sha": evidence.base_sha,
        "head_sha": evidence.head_sha,
        "human": {
            "sha256": evidence.human_digest,
            "bytes": evidence.human_bytes,
            "has_start_here": true,
        },
        "json": {
            "sha256": evidence.json_digest,
            "bytes": evidence.json_bytes,
            "findings": evidence.evidence.findings,
            "classifications": evidence.evidence.classifications,
            "summary_probes": evidence.evidence.summary_probes,
        },
    })
}

// ---------------------------------------------------------------------------
// Receipt skeleton
// ---------------------------------------------------------------------------

fn ledger_json(entry: &LedgerEntry) -> Value {
    json!({
        "step": entry.step,
        "argv": entry.argv,
        "cwd": entry.cwd,
        "started_epoch_secs": entry.started_epoch_secs,
        "status": entry.status,
    })
}

fn write_receipt(
    out: &Path,
    subject: &InstalledSubject,
    harness: &Harness,
    journey: Option<&JourneyEvidence>,
) -> Result<(), String> {
    let receipt = json!({
        "schema_version": SCHEMA_VERSION,
        "subject": {
            "crate_path": subject.crate_path,
            "crate_sha256": subject.crate_sha256,
            "prefix": subject.prefix.to_string_lossy(),
            "executable": subject.executable.to_string_lossy(),
            "executable_sha256": subject.executable_sha256,
            "executable_size": subject.executable_size,
            "version_output": subject.version_output,
        },
        "ledger": harness.ledger.iter().map(ledger_json).collect::<Vec<_>>(),
        "journey": journey.map(journey_json),
        "slices_completed": if journey.is_some() { vec!["A", "B"] } else { vec!["A"] },
    });
    std::fs::create_dir_all(out).map_err(|error| format!("create out dir: {error}"))?;
    let text = serde_json::to_string_pretty(&receipt)
        .map_err(|error| format!("render receipt: {error}"))?;
    std::fs::write(out.join(RECEIPT_FILE), format!("{text}\n"))
        .map_err(|error| format!("write receipt: {error}"))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub(crate) fn first_hour(args: &[String]) -> Result<(), String> {
    let parsed = parse_args(args)?;
    let mut harness = Harness {
        roots: Vec::new(),
        ledger: Vec::new(),
    };
    // Fail-closed ownership, checked before any install work: a
    // pre-existing fixture root is never adopted for deletion. The harness
    // creates it, so cleanup can only remove what the harness made.
    // Comparisons use absolute lexically-normalized paths: a raw
    // `starts_with` on unresolved components would miss a `--fixture-root`
    // containing `..` and let the receipt (or the install tree) land inside
    // the cleanup tree.
    let fixture_root = PathBuf::from(&parsed.fixture_root);
    if fixture_root.exists() {
        return Err(format!(
            "first-hour --fixture-root `{}` already exists; slice A never deletes pre-existing directories",
            fixture_root.display()
        ));
    }
    let fixture_norm = absolute_normalized(&fixture_root)?;
    // The receipt must outlive cleanup: --out equal to or nested under the
    // harness-cleaned fixture root would be written and then deleted before
    // the command returns success.
    let out_root = PathBuf::from(&parsed.out);
    let out_norm = absolute_normalized(&out_root)?;
    if out_norm == fixture_norm || out_norm.starts_with(&fixture_norm) {
        return Err(format!(
            "first-hour --out `{}` must not equal or nest under --fixture-root `{}`; the receipt would be cleaned before return",
            out_root.display(),
            fixture_root.display()
        ));
    }
    let prefix = PathBuf::from(&parsed.prefix);
    // The install tree must outlive cleanup too: a --prefix equal to or
    // nested under --fixture-root would be created by `cargo install`
    // before the fixture root exists, then adopted into the cleanup set —
    // the command would return success after deleting the installed
    // executable the receipt points at.
    let prefix_norm = absolute_normalized(&prefix)?;
    if prefix_norm == fixture_norm || prefix_norm.starts_with(&fixture_norm) {
        return Err(format!(
            "first-hour --prefix `{}` must not equal or nest under --fixture-root `{}`; the install tree would be cleaned before return",
            prefix.display(),
            fixture_root.display()
        ));
    }
    let subject = install_package(&mut harness, &parsed.crate_path, &prefix)?;
    admit_installed_executable(&subject, &subject.executable)?;
    std::fs::create_dir_all(&fixture_root)
        .map_err(|error| format!("create fixture root: {error}"))?;
    harness.roots.push(fixture_root.clone());
    // Slice B: baseline fixture plus the installed check journey. Every
    // product invocation resolves through the admitted installed
    // executable; the journey fails loudly when the boundary change goes
    // unobserved or the human front door does not render.
    let repo = build_fixture_repo(&mut harness, &fixture_root)?;
    let journey = run_check_journey(&mut harness, &subject, &repo, &fixture_root)?;
    write_receipt(Path::new(&parsed.out), &subject, &harness, Some(&journey))?;
    println!(
        "first-hour slice B: {} finding(s) [{}] through {} ledger {} steps",
        journey.evidence.findings,
        journey.evidence.classifications.join(","),
        subject.executable.display(),
        harness.ledger.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Returns the harness's typed error, or fails the test when the call
    /// succeeded (the expect_err shape without the banned method).
    fn refusal_of<T>(result: Result<T, String>) -> Result<String, String> {
        match result {
            Ok(_) => Err("expected a typed refusal; the call succeeded".to_string()),
            Err(error) => Ok(error),
        }
    }

    #[test]
    fn wrong_binary_is_refused_against_the_installed_identity() -> Result<(), String> {
        let dir =
            std::env::temp_dir().join(format!("ripr-first-hour-wrong-bin-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|error| format!("fixture dir: {error}"))?;
        let real = dir.join("ripr");
        let impostor = dir.join("other-ripr");
        std::fs::write(&real, b"installed-bytes")
            .map_err(|error| format!("write real: {error}"))?;
        std::fs::write(&impostor, b"different-bytes")
            .map_err(|error| format!("write impostor: {error}"))?;
        let subject = InstalledSubject {
            crate_path: "fixture.crate".to_string(),
            crate_sha256: "0".repeat(64),
            prefix: dir.clone(),
            executable: real.clone(),
            executable_sha256: sha256_hex(b"installed-bytes"),
            executable_size: 15,
            version_output: "ripr 0.11.0".to_string(),
        };
        assert!(matches!(
            admit_installed_executable(&subject, &real),
            Ok(())
        ));
        assert!(matches!(
            admit_installed_executable(&subject, &impostor),
            Err(error) if error.contains("wrong binary")
        ));
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn install_refuses_a_live_prefix() -> Result<(), String> {
        let dir = std::env::temp_dir().join(format!(
            "ripr-first-hour-live-prefix-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|error| format!("live prefix fixture: {error}"))?;
        let mut harness = Harness {
            roots: Vec::new(),
            ledger: Vec::new(),
        };
        assert!(matches!(
            install_package(&mut harness, "missing.crate", &dir),
            Err(error) if error.contains("already exists")
        ));
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn hostile_archive_members_are_rejected_before_extraction() -> Result<(), String> {
        use flate2::Compression;
        use flate2::write::GzEncoder;
        let dir = std::env::temp_dir().join(format!(
            "ripr-first-hour-hostile-crate-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|error| format!("fixture dir: {error}"))?;
        let archive = dir.join("evil.crate");
        {
            let file = std::fs::File::create(&archive)
                .map_err(|error| format!("archive file: {error}"))?;
            let mut raw: Vec<u8> = Vec::new();
            // Raw ustar member with an exact (possibly hostile) name,
            // bypassing safe builders that refuse to construct it.
            // Decimal byte offsets per POSIX ustar: name 0, mode 100,
            // uid 108, gid 116, size 124, mtime 136, chksum 148,
            // typeflag 156, magic 257.
            fn raw_member(out: &mut Vec<u8>, name: &[u8], data: &[u8]) {
                let mut header = [0u8; 512];
                header[..name.len().min(100)].copy_from_slice(&name[..name.len().min(100)]);
                header[100..108].copy_from_slice(b"0000644\0");
                header[108..116].copy_from_slice(b"0000000\0");
                header[116..124].copy_from_slice(b"0000000\0");
                let size = format!("{:011o}\0", data.len());
                header[124..136].copy_from_slice(&size.as_bytes()[..12]);
                header[136..148].copy_from_slice(b"00000000000\0");
                header[148..156].copy_from_slice(b"        ");
                header[156] = b'0';
                header[257..263].copy_from_slice(b"ustar\0");
                let checksum: u32 = header.iter().map(|byte| *byte as u32).sum();
                header[148..156].copy_from_slice(format!("{checksum:06o}\0 ").as_bytes());
                out.extend_from_slice(&header);
                out.extend_from_slice(data);
                out.resize(out.len() + (512 - data.len() % 512) % 512, 0);
            }
            raw_member(&mut raw, b"pkg-evil/Cargo.toml", b"evil");
            raw_member(&mut raw, b"../escape", b"evil");
            use std::io::Write;
            let mut encoder = GzEncoder::new(file, Compression::default());
            encoder
                .write_all(&raw)
                .map_err(|error| format!("gzip body: {error}"))?;
            encoder
                .write_all(&[0u8; 1024])
                .map_err(|error| format!("gzip trailer: {error}"))?;
            encoder
                .finish()
                .map_err(|error| format!("gzip flush: {error}"))?;
        }
        let mut harness = Harness {
            roots: Vec::new(),
            ledger: Vec::new(),
        };
        assert!(matches!(
            extract_crate_archive(&mut harness, &archive),
            Err(error) if error.contains("escapes extraction root")
        ));
        // Nothing materialized outside staging.
        assert!(!dir.join("escape").exists());
        drop(harness);
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn pre_existing_fixture_root_is_never_adopted_for_deletion() -> Result<(), String> {
        let dir = std::env::temp_dir().join(format!(
            "ripr-first-hour-preexisting-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|error| format!("pre-existing dir: {error}"))?;
        let sentinel = dir.join("user-data.txt");
        std::fs::write(&sentinel, b"precious").map_err(|error| format!("sentinel: {error}"))?;
        assert!(matches!(
            first_hour(&[
                "--crate".to_string(),
                "missing.crate".to_string(),
                "--prefix".to_string(),
                dir.join("prefix").to_string_lossy().to_string(),
                "--out".to_string(),
                dir.join("out").to_string_lossy().to_string(),
                "--fixture-root".to_string(),
                dir.to_string_lossy().to_string(),
            ]),
            Err(error) if error.contains("already exists")
        ));
        // The pre-existing directory and its contents survive.
        assert!(
            std::fs::read(&sentinel).map_err(|error| format!("sentinel survives: {error}"))?
                == b"precious"
        );
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn receipt_nested_under_fixture_root_is_refused_before_any_work() -> Result<(), String> {
        let base =
            std::env::temp_dir().join(format!("ripr-first-hour-overlap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        assert!(matches!(
            first_hour(&[
                "--crate".to_string(),
                "missing.crate".to_string(),
                "--prefix".to_string(),
                base.join("prefix").to_string_lossy().to_string(),
                "--out".to_string(),
                base.join("fixtures")
                    .join("out")
                    .to_string_lossy()
                    .to_string(),
                "--fixture-root".to_string(),
                base.join("fixtures").to_string_lossy().to_string(),
            ]),
            Err(error) if error.contains("must not equal or nest under")
        ));
        // Nothing was created: no install, no fixture root, no receipt.
        assert!(!base.exists());
        Ok(())
    }

    #[test]
    fn prefix_nested_under_fixture_root_is_refused_before_any_work() -> Result<(), String> {
        let base = std::env::temp_dir().join(format!(
            "ripr-first-hour-prefix-nest-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        assert!(matches!(
            first_hour(&[
                "--crate".to_string(),
                "missing.crate".to_string(),
                "--prefix".to_string(),
                base.join("fixtures")
                    .join("prefix")
                    .to_string_lossy()
                    .to_string(),
                "--out".to_string(),
                base.join("out").to_string_lossy().to_string(),
                "--fixture-root".to_string(),
                base.join("fixtures").to_string_lossy().to_string(),
            ]),
            Err(error) if error.contains("must not equal or nest under")
        ));
        // Nothing was created: no install tree, no fixture root, no receipt.
        assert!(!base.exists());
        Ok(())
    }

    #[test]
    fn dotdot_evasion_of_receipt_containment_is_refused() -> Result<(), String> {
        let base =
            std::env::temp_dir().join(format!("ripr-first-hour-dotdot-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        // `--out base/fixtures/../fixtures/out` normalizes inside the
        // fixture root; a raw lexical check on the un-normalized form would
        // still catch this spelling, so also cover the reverse: a fixture
        // root that escapes to the parent of --out.
        assert!(matches!(
            first_hour(&[
                "--crate".to_string(),
                "missing.crate".to_string(),
                "--prefix".to_string(),
                base.join("prefix").to_string_lossy().to_string(),
                "--out".to_string(),
                base.join("fixtures")
                    .join("out")
                    .to_string_lossy()
                    .to_string(),
                "--fixture-root".to_string(),
                base.join("other")
                    .join("..")
                    .join("fixtures")
                    .to_string_lossy()
                    .to_string(),
            ]),
            Err(error) if error.contains("must not equal or nest under")
        ));
        assert!(!base.exists());
        Ok(())
    }

    #[test]
    fn staging_claims_exclusive_ownership_without_deleting() -> Result<(), String> {
        let mut harness = Harness {
            roots: Vec::new(),
            ledger: Vec::new(),
        };
        let first = harness.owned_staging("exclusive")?;
        let second = harness.owned_staging("exclusive")?;
        assert!(first != second);
        // A squatter the harness did not create is never removed: occupy a
        // neighbouring path and prove it survives staging creation.
        let squatter = first
            .parent()
            .ok_or_else(|| "staging has no parent".to_string())?
            .join("ripr-first-hour-squatter");
        let _ = std::fs::remove_dir_all(&squatter);
        std::fs::create_dir_all(&squatter).map_err(|error| format!("squatter: {error}"))?;
        let sentinel = squatter.join("keep.txt");
        std::fs::write(&sentinel, b"not-ours").map_err(|error| format!("sentinel: {error}"))?;
        let _third = harness.owned_staging("exclusive")?;
        assert!(
            std::fs::read(&sentinel).map_err(|error| format!("squatter survives: {error}"))?
                == b"not-ours"
        );
        drop(harness);
        assert!(!first.exists());
        assert!(!second.exists());
        assert!(squatter.exists());
        let _ = std::fs::remove_dir_all(&squatter);
        Ok(())
    }

    #[test]
    fn staging_creates_a_missing_base_instead_of_assuming_it() -> Result<(), String> {
        // The home-cache fallback on a fresh runner does not exist yet;
        // exclusive creation must build the base, or every later step fails
        // with a staging error that masks the real work (observed on the
        // hosted lane: hostile-archive test failed with a staging error).
        let missing = std::env::temp_dir().join(format!(
            "ripr-first-hour-missing-base-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&missing);
        let mut harness = Harness {
            roots: Vec::new(),
            ledger: Vec::new(),
        };
        let root = Harness::owned_staging_in(&mut harness, &missing, "missing-base")?;
        assert!(root.is_dir());
        drop(harness);
        assert!(!root.exists());
        let _ = std::fs::remove_dir_all(&missing);
        Ok(())
    }

    #[test]
    fn staging_base_never_sits_inside_the_enclosing_workspace() -> Result<(), String> {
        let base = staging_base()?;
        let cwd = std::env::current_dir().map_err(|error| format!("current dir: {error}"))?;
        if let Some(root) = enclosing_workspace_root(&cwd) {
            assert!(
                !base.starts_with(&root),
                "staging `{}` sits inside workspace `{}`",
                base.display(),
                root.display()
            );
        }
        Ok(())
    }

    #[test]
    fn installed_json_evidence_requires_an_observed_finding() -> Result<(), String> {
        let observed = json!({
            "schema_version": "1",
            "summary": {"probes": 1, "findings": 1},
            "findings": [{"id": "probe:src_lib.rs:predicate:e4a96c16", "classification": "weakly_exposed"}],
        });
        let evidence = check_evidence_json(&observed.to_string())?;
        assert_eq!(
            evidence,
            CheckEvidence {
                findings: 1,
                classifications: vec!["weakly_exposed".to_string()],
                summary_probes: 1,
            }
        );
        // Exit 0 plus valid JSON is not enough: zero findings means the
        // boundary change went unobserved and the journey must fail.
        let empty = json!({"summary": {"probes": 0}, "findings": []});
        assert!(matches!(
            check_evidence_json(&empty.to_string()),
            Err(error) if error.contains("zero findings")
        ));
        assert!(matches!(
            check_evidence_json("{not json"),
            Err(error) if error.contains("parse installed JSON")
        ));
        assert!(matches!(
            check_evidence_json("{}"),
            Err(error) if error.contains("no findings array")
        ));
        Ok(())
    }

    #[test]
    fn journey_requires_the_designed_weakly_exposed_oracle() -> Result<(), String> {
        require_boundary_oracle(&CheckEvidence {
            findings: 1,
            classifications: vec!["weakly_exposed".to_string()],
            summary_probes: 1,
        })?;
        // A promotion to exposed is the false-confidence family: the
        // mid-range-only test never observes the changed sink.
        assert!(matches!(
            require_boundary_oracle(&CheckEvidence {
                findings: 1,
                classifications: vec!["exposed".to_string()],
                summary_probes: 1,
            }),
            Err(error) if error.contains("exactly one weakly_exposed")
        ));
        // Any other shape (demotion, multiplicity, silence) also refuses.
        assert!(matches!(
            require_boundary_oracle(&CheckEvidence {
                findings: 1,
                classifications: vec!["reachable_unrevealed".to_string()],
                summary_probes: 1,
            }),
            Err(error) if error.contains("exactly one weakly_exposed")
        ));
        assert!(matches!(
            require_boundary_oracle(&CheckEvidence {
                findings: 2,
                classifications: vec!["weakly_exposed".to_string()],
                summary_probes: 2,
            }),
            Err(error) if error.contains("exactly one weakly_exposed")
        ));
        Ok(())
    }

    #[test]
    fn human_front_door_requires_its_start_here_section() -> Result<(), String> {
        require_start_here(
            "ripr static RIPR exposure analysis\n\nStart here:\n  State: top_gap\n",
        )?;
        assert!(matches!(
            require_start_here("Summary: 0 probe(s)\n"),
            Err(error) if error.contains("Start here")
        ));
        Ok(())
    }

    #[test]
    fn journey_run_records_exact_argv_and_real_cwd() -> Result<(), String> {
        let mut harness = Harness {
            roots: Vec::new(),
            ledger: Vec::new(),
        };
        let output = journey_run(
            &mut harness,
            "probe-step",
            "git",
            &["--version".to_string()],
            &[],
        )?;
        assert!(output.contains("git version"));
        assert_eq!(harness.ledger.len(), 1);
        let entry = &harness.ledger[0];
        assert_eq!(entry.step, "probe-step");
        assert_eq!(entry.argv, vec!["git".to_string(), "--version".to_string()]);
        assert_eq!(entry.status, "ok");
        // The recorded cwd is the real child cwd, never a claim.
        assert_eq!(
            entry.cwd,
            std::env::current_dir()
                .map_err(|error| format!("current dir: {error}"))?
                .to_string_lossy()
        );
        Ok(())
    }

    #[test]
    fn arg_surface_requires_every_identity_input() -> Result<(), String> {
        let _ = refusal_of(parse_args(&[]))?;
        let _ = refusal_of(parse_args(&["--crate".to_string(), "a.crate".to_string()]))?;
        let Ok(parsed) = parse_args(&[
            "--crate".to_string(),
            "a.crate".to_string(),
            "--prefix".to_string(),
            "p".to_string(),
            "--out".to_string(),
            "o".to_string(),
            "--fixture-root".to_string(),
            "f".to_string(),
        ]) else {
            return Err("complete arg surface must parse".to_string());
        };
        assert_eq!(parsed.prefix, "p");
        Ok(())
    }
}
