//! Installed first-hour qualification harness — #1674 slices A (harness core),
//! B (fixture + installed check journey), C (negative controls against the
//! production journey), D (rerun comparison), and E (installed init journey).
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
Installs the packaged candidate into a clean prefix, builds disposable
baselines, and runs the installed journeys — check (human Start-here front
door plus JSON evidence), init (advisory defaults, no-overwrite conflicts,
working --force), and agent repair (seam loop with an external test-only
edit, separate focused test execution, verify/outcome movement gates) —
under fresh roots. Records the installed-artifact identity and journey
evidence in the receipt. The explicit agent receipt re-invocation route is
not driven (stale after-verdict binding, #1738); the journey consumes the
phase-written receipt artifact instead.";

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

fn take_value(
    command: &str,
    usage: &str,
    args: &[String],
    index: &mut usize,
    flag: &str,
) -> Result<String, String> {
    *index += 1;
    args.get(*index)
        .cloned()
        .ok_or_else(|| format!("{command} {flag} requires a value\n{usage}"))
}

fn parse_args(command: &str, usage: &str, args: &[String]) -> Result<FirstHourArgs, String> {
    let mut crate_path: Option<String> = None;
    let mut prefix: Option<String> = None;
    let mut out: Option<String> = None;
    let mut fixture_root: Option<String> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--crate" => {
                crate_path = Some(take_value(command, usage, args, &mut index, "--crate")?);
            }
            "--prefix" => {
                prefix = Some(take_value(command, usage, args, &mut index, "--prefix")?);
            }
            "--out" => out = Some(take_value(command, usage, args, &mut index, "--out")?),
            "--fixture-root" => {
                fixture_root = Some(take_value(
                    command,
                    usage,
                    args,
                    &mut index,
                    "--fixture-root",
                )?);
            }
            "--help" | "-h" => return Err(usage.to_string()),
            other => return Err(format!("{command} unknown argument `{other}`\n{usage}")),
        }
        index += 1;
    }
    Ok(FirstHourArgs {
        crate_path: crate_path.ok_or_else(|| format!("{command} --crate is required\n{usage}"))?,
        prefix: prefix.ok_or_else(|| format!("{command} --prefix is required\n{usage}"))?,
        out: out.ok_or_else(|| format!("{command} --out is required\n{usage}"))?,
        fixture_root: fixture_root
            .ok_or_else(|| format!("{command} --fixture-root is required\n{usage}"))?,
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
    env: Vec<(String, String)>,
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
        env: Vec::new(),
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
        env: Vec::new(),
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
/// Committed in the fixture baseline so `cargo test` never creates or
/// rewrites it mid-loop: the repair comparability gate binds manifest and
/// lockfile identities, and a generated lockfile would render before/after
/// incomparable. Zero dependencies, no timestamps — byte-stable.
const FIXTURE_CARGO_LOCK: &str = "# This file is automatically @generated by Cargo.\n# It is not intended for manual editing.\nversion = 4\n\n[[package]]\nname = \"fixture-firsthour\"\nversion = \"0.1.0\"\n";
/// The fixed focused test-only edit the harness applies externally during
/// the repair journey: three exact assertions over the changed boundary.
/// RIPR performs no edit; the harness appends without modifying existing
/// content, and the journey refuses when the file is not byte-identical
/// to the fixture baseline first.
const REPAIR_TEST_EDIT: &str = "\n#[test]\nfn boundary_fee_applies_at_exactly_10_000() {\n    assert_eq!(fixture_firsthour::fee(9_999), 0);\n    assert_eq!(fixture_firsthour::fee(10_000), 500);\n    assert_eq!(fixture_firsthour::fee(10_001), 500);\n}\n";
const REPAIR_TEST_NAME: &str = "boundary_fee_applies_at_exactly_10_000";

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
        env: envs
            .iter()
            .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
            .collect(),
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

/// Builds the disposable baseline repository under `label`: base commit
/// (production + weak mid-range-only test), head commit (exact-boundary `>`
/// to `>=` change the weak test cannot discriminate — or, when
/// `mutate_head` is false, an empty head commit carrying the base tree for
/// the no-change control). Returns base/head SHAs.
fn build_fixture_repo(
    harness: &mut Harness,
    fixture_root: &Path,
    label: &str,
    mutate_head: bool,
) -> Result<FixtureRepo, String> {
    // Each journey owns an isolated checkout: rerunning the builder under
    // one fixture root must never re-init an existing repository.
    let mut repo = fixture_root.join(label);
    for component in FIXTURE_REPO_REL {
        repo.push(component);
    }
    if repo.exists() {
        return Err(format!(
            "fixture checkout `{}` already exists; the harness never reuses a live checkout",
            repo.display()
        ));
    }
    let src = repo.join("src");
    let tests = repo.join("tests");
    std::fs::create_dir_all(&src).map_err(|error| format!("create fixture src: {error}"))?;
    std::fs::create_dir_all(&tests).map_err(|error| format!("create fixture tests: {error}"))?;
    std::fs::write(repo.join("Cargo.toml"), FIXTURE_CARGO_TOML)
        .map_err(|error| format!("write fixture Cargo.toml: {error}"))?;
    std::fs::write(repo.join("Cargo.lock"), FIXTURE_CARGO_LOCK)
        .map_err(|error| format!("write fixture Cargo.lock: {error}"))?;
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
    if mutate_head {
        std::fs::write(src.join("lib.rs"), FIXTURE_LIB_HEAD)
            .map_err(|error| format!("write fixture head lib: {error}"))?;
        fixture_git(
            harness,
            &repo,
            &["commit", "-qam", "head"],
            FIXTURE_HEAD_DATE,
        )?;
    } else {
        // No-change control: the head commit carries the base tree, so the
        // installed check observes an empty diff and must refuse a pass.
        fixture_git(
            harness,
            &repo,
            &["commit", "-qam", "head", "--allow-empty"],
            FIXTURE_HEAD_DATE,
        )?;
    }
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

/// Normalizes installed output for cross-run comparison: absolute run
/// locations (fixture root, checkout path) are attested elsewhere in the
/// receipt (ledger argv, repo SHAs), so the evidence digests hash the
/// output with those locations replaced by a stable placeholder. Two runs
/// of the same candidate agree on the normalized digest; the raw byte
/// count is retained so normalization itself stays auditable.
fn normalize_output(stdout: &str, locations: &[&str]) -> String {
    // Longest first: a fixture root that prefixes the checkout path must
    // not shadow the longer, more specific replacement.
    let mut ordered = locations.to_vec();
    ordered.sort_by_key(|location| std::cmp::Reverse(location.len()));
    let mut normalized = stdout.to_string();
    for location in ordered {
        if !location.is_empty() {
            normalized = normalized.replace(location, "<run-location>");
        }
    }
    normalized
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
    human_normalized_digest: String,
    human_bytes: usize,
    json_digest: String,
    json_normalized_digest: String,
    json_bytes: usize,
    evidence: CheckEvidence,
}

/// Runs the installed check journey (issue steps 1-2): human rendering for
/// the `Start here` front door plus JSON evidence, both through the admitted
/// installed executable under fresh cache/HOME roots. The checked base is a
/// parameter so the invalid-base control can drive the production path with
/// a bogus ref; the positive journey always passes the fixture base SHA.
fn run_check_journey(
    harness: &mut Harness,
    subject: &InstalledSubject,
    repo: &FixtureRepo,
    fixture_root: &Path,
    label: &str,
    base_sha: &str,
) -> Result<JourneyEvidence, String> {
    admit_installed_executable(subject, &subject.executable)?;
    // Each rendering runs cold under its own fresh cache/HOME: the JSON
    // gate must not warm the cache the human front door then runs on, or a
    // cold-cache-only human regression would pass undetected.
    let cache_root = fixture_root.join("cache").join("ripr");
    let home_root = fixture_root.join("home");
    let executable = subject.executable.to_string_lossy().to_string();
    let root = repo.path.to_string_lossy().to_string();
    let human_args = vec![
        "check".to_string(),
        "--root".to_string(),
        root.clone(),
        "--base".to_string(),
        base_sha.to_string(),
    ];
    // Machine evidence before human rendering: the JSON gate pins the exact
    // oracle, so an empty or misclassified observation refuses here with
    // its own typed error instead of falling through to the human gate.
    let json_cache = cache_root.join("json");
    let json_home = home_root.join("json");
    std::fs::create_dir_all(&json_cache)
        .map_err(|error| format!("create journey json cache: {error}"))?;
    std::fs::create_dir_all(&json_home)
        .map_err(|error| format!("create journey json home: {error}"))?;
    let mut json_args = human_args.clone();
    json_args.push("--json".to_string());
    let json_stdout = journey_run(
        harness,
        "installed-check-json",
        &executable,
        &json_args,
        &[
            ("RIPR_CACHE_DIR", &json_cache.to_string_lossy()),
            ("HOME", &json_home.to_string_lossy()),
        ],
    )
    .map_err(|error| format!("installed check (json) failed: {error}"))?;
    let evidence = check_evidence_json(&json_stdout)?;
    require_boundary_oracle(&evidence)?;
    let human_cache = cache_root.join("human");
    let human_home = home_root.join("human");
    std::fs::create_dir_all(&human_cache)
        .map_err(|error| format!("create journey human cache: {error}"))?;
    std::fs::create_dir_all(&human_home)
        .map_err(|error| format!("create journey human home: {error}"))?;
    let human = journey_run(
        harness,
        "installed-check-human",
        &executable,
        &human_args,
        &[
            ("RIPR_CACHE_DIR", &human_cache.to_string_lossy()),
            ("HOME", &human_home.to_string_lossy()),
        ],
    )
    .map_err(|error| format!("installed check (human) failed: {error}"))?;
    require_start_here(&human)?;
    let locations = [
        repo.path.to_string_lossy().to_string(),
        fixture_root.to_string_lossy().to_string(),
    ];
    let location_refs = locations.iter().map(String::as_str).collect::<Vec<_>>();
    Ok(JourneyEvidence {
        repo_rel: format!("{label}/{}", FIXTURE_REPO_REL.join("/")),
        base_sha: repo.base_sha.clone(),
        head_sha: repo.head_sha.clone(),
        human_digest: sha256_hex(human.as_bytes()),
        human_normalized_digest: sha256_hex(normalize_output(&human, &location_refs).as_bytes()),
        human_bytes: human.len(),
        json_digest: sha256_hex(json_stdout.as_bytes()),
        json_normalized_digest: sha256_hex(
            normalize_output(&json_stdout, &location_refs).as_bytes(),
        ),
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
            "normalized_sha256": evidence.human_normalized_digest,
            "bytes": evidence.human_bytes,
            "has_start_here": true,
        },
        "json": {
            "sha256": evidence.json_digest,
            "normalized_sha256": evidence.json_normalized_digest,
            "bytes": evidence.json_bytes,
            "findings": evidence.evidence.findings,
            "classifications": evidence.evidence.classifications,
            "summary_probes": evidence.evidence.summary_probes,
        },
    })
}

// ---------------------------------------------------------------------------
// Slice E: installed init journey (advisory defaults, no overwrite)
// ---------------------------------------------------------------------------

/// Required advisory markers in the generated `ripr.toml`: draft analysis
/// mode with unchanged tests included — the documented built-in defaults.
fn toml_advisory_markers(toml: &str) -> Result<(), String> {
    for marker in ["mode = \"draft\"", "include_unchanged_tests = true"] {
        if !toml.contains(marker) {
            return Err(format!(
                "generated ripr.toml holds no advisory marker `{marker}`"
            ));
        }
    }
    Ok(())
}

/// Required advisory markers in the generated workflow: non-blocking
/// execution, SARIF upload gating, and the supported install reference.
/// A blocking default here would turn an advisory scaffold into an
/// enforcement gate.
fn workflow_advisory_markers(workflow: &str) -> Result<(), String> {
    for marker in [
        "continue-on-error",
        "RIPR_UPLOAD_SARIF",
        "cargo install ripr --locked",
        "ripr pilot",
    ] {
        if !workflow.contains(marker) {
            return Err(format!(
                "generated workflow holds no advisory marker `{marker}`"
            ));
        }
    }
    Ok(())
}

struct InitEvidence {
    repo_rel: String,
    toml_digest: String,
    workflow_digest: String,
    user_workflow_preserved: bool,
    rerun_refused: bool,
    toml_conflict_preserved: bool,
    workflow_conflict_refused: bool,
    force_overwrites: bool,
}

/// Runs the installed init journey (issue step 3) on an isolated checkout:
/// fresh generation with advisory proof, a user-owned workflow sentinel
/// that must survive byte-identical, then the conflict sequence — a rerun
/// over generated output refuses, a preexisting `ripr.toml` alone is
/// preserved while generation continues (exit 0), a preexisting `ripr.yml`
/// refuses without `--force` (nonzero exit, preserved), and `--force`
/// provably overwrites.
fn run_init_journey(
    harness: &mut Harness,
    subject: &InstalledSubject,
    fixture_root: &Path,
) -> Result<InitEvidence, String> {
    admit_installed_executable(subject, &subject.executable)?;
    let repo = build_fixture_repo(harness, fixture_root, "init", true)?;
    let repo_rel = format!("init/{}", FIXTURE_REPO_REL.join("/"));
    let executable = subject.executable.to_string_lossy().to_string();
    let root = repo.path.to_string_lossy().to_string();
    let home_root = fixture_root.join("home-init");
    let cache_root = fixture_root.join("cache-init");
    std::fs::create_dir_all(&home_root).map_err(|error| format!("create init home: {error}"))?;
    std::fs::create_dir_all(&cache_root).map_err(|error| format!("create init cache: {error}"))?;
    let envs = [
        ("RIPR_CACHE_DIR", cache_root.to_string_lossy().to_string()),
        ("HOME", home_root.to_string_lossy().to_string()),
    ];
    let env_refs = envs
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .collect::<Vec<_>>();
    let mut init_args = vec![
        "init".to_string(),
        "--root".to_string(),
        root.clone(),
        "--ci".to_string(),
        "github".to_string(),
    ];
    // A user-owned workflow beside the generated one: init must never
    // touch what it did not generate.
    let workflows = repo.path.join(".github").join("workflows");
    std::fs::create_dir_all(&workflows)
        .map_err(|error| format!("create workflows dir: {error}"))?;
    let sentinel = workflows.join("user.yml");
    std::fs::write(&sentinel, "# user owned\n")
        .map_err(|error| format!("write user sentinel: {error}"))?;
    journey_run(
        harness,
        "installed-init-fresh",
        &executable,
        &init_args,
        &env_refs,
    )
    .map_err(|error| format!("installed init (fresh) failed: {error}"))?;
    let toml_path = repo.path.join("ripr.toml");
    let workflow_path = workflows.join("ripr.yml");
    let toml = std::fs::read_to_string(&toml_path)
        .map_err(|error| format!("read generated ripr.toml: {error}"))?;
    let workflow = std::fs::read_to_string(&workflow_path)
        .map_err(|error| format!("read generated workflow: {error}"))?;
    toml_advisory_markers(&toml)?;
    workflow_advisory_markers(&workflow)?;
    let sentinel_after = std::fs::read_to_string(&sentinel)
        .map_err(|error| format!("read user sentinel: {error}"))?;
    if sentinel_after != "# user owned\n" {
        return Err("installed init modified the user-owned workflow".to_string());
    }
    // Rerun as-is: the generated workflow already exists, so the rerun
    // must refuse with the --force remedy instead of silently succeeding.
    let rerun = journey_run(
        harness,
        "installed-init-rerun-refusal",
        &executable,
        &init_args,
        &env_refs,
    );
    let rerun_error = match rerun {
        Ok(_) => {
            return Err("installed init reran over its own output without --force".to_string());
        }
        Err(error) => error,
    };
    if !rerun_error.contains("--force") {
        return Err(format!(
            "rerun refusal names no --force remedy: {rerun_error}"
        ));
    }
    // Conflict 1: a preexisting user ripr.toml — with no workflow present
    // to trigger the workflow refusal — is preserved byte-identical while
    // generation continues with success exit.
    std::fs::remove_file(&workflow_path)
        .map_err(|error| format!("remove generated workflow: {error}"))?;
    std::fs::write(&toml_path, "user = true\n")
        .map_err(|error| format!("plant user ripr.toml: {error}"))?;
    journey_run(
        harness,
        "installed-init-toml-conflict",
        &executable,
        &init_args,
        &env_refs,
    )
    .map_err(|error| format!("installed init (toml conflict) failed: {error}"))?;
    let toml_kept = std::fs::read_to_string(&toml_path)
        .map_err(|error| format!("read kept ripr.toml: {error}"))?;
    if toml_kept != "user = true\n" {
        return Err("installed init overwrote the user-owned ripr.toml".to_string());
    }
    // Conflict 2: a preexisting user ripr.yml refuses without --force.
    std::fs::write(&workflow_path, "# user workflow\n")
        .map_err(|error| format!("plant user workflow: {error}"))?;
    let conflict = journey_run(
        harness,
        "installed-init-workflow-conflict",
        &executable,
        &init_args,
        &env_refs,
    );
    let conflict_error = match conflict {
        Ok(_) => {
            return Err(
                "installed init overwrote the user-owned workflow without --force".to_string(),
            );
        }
        Err(error) => error,
    };
    if !conflict_error.contains("--force") {
        return Err(format!(
            "workflow-conflict refusal names no --force remedy: {conflict_error}"
        ));
    }
    let workflow_kept = std::fs::read_to_string(&workflow_path)
        .map_err(|error| format!("read kept workflow: {error}"))?;
    if workflow_kept != "# user workflow\n" {
        return Err("installed init overwrote the user-owned workflow".to_string());
    }
    // The documented escape hatch provably works on the disposable checkout.
    init_args.push("--force".to_string());
    journey_run(
        harness,
        "installed-init-force",
        &executable,
        &init_args,
        &env_refs,
    )
    .map_err(|error| format!("installed init (--force) failed: {error}"))?;
    let toml_forced = std::fs::read_to_string(&toml_path)
        .map_err(|error| format!("read forced ripr.toml: {error}"))?;
    let workflow_forced = std::fs::read_to_string(&workflow_path)
        .map_err(|error| format!("read forced workflow: {error}"))?;
    toml_advisory_markers(&toml_forced)?;
    workflow_advisory_markers(&workflow_forced)?;
    // The forced run targets only the two generated files: a --force that
    // modifies or deletes the unrelated user workflow must still fail.
    let sentinel_forced = std::fs::read_to_string(&sentinel)
        .map_err(|error| format!("read user sentinel after --force: {error}"))?;
    if sentinel_forced != "# user owned\n" {
        return Err("installed init --force touched the user-owned workflow".to_string());
    }
    Ok(InitEvidence {
        repo_rel,
        toml_digest: sha256_hex(toml.as_bytes()),
        workflow_digest: sha256_hex(workflow.as_bytes()),
        user_workflow_preserved: true,
        rerun_refused: true,
        toml_conflict_preserved: true,
        workflow_conflict_refused: true,
        force_overwrites: true,
    })
}

fn init_json(evidence: &InitEvidence) -> Value {
    json!({
        "repo": evidence.repo_rel,
        "toml_sha256": evidence.toml_digest,
        "workflow_sha256": evidence.workflow_digest,
        "user_workflow_preserved": evidence.user_workflow_preserved,
        "rerun_refused": evidence.rerun_refused,
        "toml_conflict_preserved": evidence.toml_conflict_preserved,
        "workflow_conflict_refused": evidence.workflow_conflict_refused,
        "force_overwrites": evidence.force_overwrites,
    })
}

// ---------------------------------------------------------------------------
// Slice F: agent repair + test-edit + verify/receipt journey
// ---------------------------------------------------------------------------

/// Keeps a configured toolchain root (`RUSTUP_HOME`, `CARGO_HOME`) exactly
/// as the ambient environment sets it; the `$HOME`-derived guess applies
/// only when the variable is unset, so container layouts with roots
/// outside `$HOME` are never overwritten with nonexistent paths.
fn prefer_ambient_toolchain_root(ambient: Option<String>, derived: &str) -> String {
    ambient.unwrap_or_else(|| derived.to_string())
}

/// Resolves the repair seam for one finding: exactly one packet must exist
/// for the fixture baseline, or the journey fails for redesign instead of
/// guessing which seam the finding means.
fn repair_seam_id(packets_stdout: &str) -> Result<String, String> {
    let parsed: Value = serde_json::from_str(packets_stdout)
        .map_err(|error| format!("parse installed seam packets: {error}"))?;
    let total = parsed
        .get("packets_total")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if total != 1 {
        return Err(format!(
            "installed seam packets must hold exactly one packet; observed {total}"
        ));
    }
    // The counter alone does not bind the array: an inconsistent report
    // (total 1, two entries) must refuse rather than silently take the
    // first packet.
    let packets = parsed
        .get("packets")
        .and_then(Value::as_array)
        .ok_or_else(|| "installed seam packets hold no packets array".to_string())?;
    if packets.len() != 1 {
        return Err(format!(
            "installed seam packets array must hold exactly one entry; observed {}",
            packets.len()
        ));
    }
    packets
        .first()
        .and_then(|packet| packet.get("seam_id"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "installed seam packet holds no seam_id".to_string())
}

/// Reads the single repair-attempt manifest directory the before phase must
/// have created: zero or several attempts means the journey state is not
/// the clean single loop it claims.
fn repair_attempt_id(repo: &Path) -> Result<String, String> {
    let dir = repo.join("target").join("ripr").join("repair-attempts");
    let mut attempts = Vec::new();
    let entries =
        std::fs::read_dir(&dir).map_err(|error| format!("read repair attempts: {error}"))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("read attempt entry: {error}"))?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("repair-attempt-") {
            attempts.push(name);
        }
    }
    if attempts.len() != 1 {
        return Err(format!(
            "repair before phase must leave exactly one attempt; observed {}",
            attempts.len()
        ));
    }
    Ok(attempts.remove(0))
}

/// Focused test counts from `cargo test` output: running total, passed,
/// failed. The journey requires a passing focused run with at least one
/// executed test; selected/executed stay separate in the receipt.
fn cargo_test_counts(stdout: &str) -> Result<(u64, u64, u64), String> {
    let running = stdout
        .lines()
        .find_map(|line| {
            let (_, rest) = line.trim().split_once("running ")?;
            let (count, _) = rest.split_once(' ')?;
            count.parse::<u64>().ok()
        })
        .ok_or_else(|| "cargo test output holds no `running N tests` line".to_string())?;
    let (passed, failed) = stdout
        .lines()
        .find_map(|line| {
            let (_, rest) = line.split_once("test result:")?;
            let passed = rest
                .split("passed")
                .next()?
                .trim()
                .rsplit(' ')
                .next()?
                .parse()
                .ok()?;
            let failed = rest
                .split("failed")
                .next()?
                .trim()
                .rsplit(' ')
                .next()?
                .parse()
                .ok()?;
            Some((passed, failed))
        })
        .ok_or_else(|| "cargo test output holds no `test result:` line".to_string())?;
    Ok((running, passed, failed))
}

struct MovementEvidence {
    closed: u64,
    improved: u64,
    regressed: u64,
}

/// Static-evidence movement gates: the focused edit must close at least
/// one gap and improve at least one seam with zero regressions. Anything
/// else refuses the repair claim.
fn verify_movement(verify_stdout: &str) -> Result<MovementEvidence, String> {
    let parsed: Value = serde_json::from_str(verify_stdout)
        .map_err(|error| format!("parse installed verify JSON: {error}"))?;
    let summary = parsed
        .get("summary")
        .ok_or_else(|| "installed verify JSON holds no summary".to_string())?;
    let gap_movement = summary
        .get("gap_movement")
        .ok_or_else(|| "installed verify JSON holds no gap_movement".to_string())?;
    let closed = gap_movement
        .get("closed")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let improved = summary.get("improved").and_then(Value::as_u64).unwrap_or(0);
    let regressed = summary
        .get("regressed")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if closed < 1 {
        return Err(format!("installed verify closed no gap; summary {summary}"));
    }
    if improved < 1 {
        return Err(format!(
            "installed verify improved no seam; summary {summary}"
        ));
    }
    if regressed > 0 {
        return Err(format!(
            "installed verify regressed {regressed} seam(s); refusing the repair claim"
        ));
    }
    Ok(MovementEvidence {
        closed,
        improved,
        regressed,
    })
}

/// Requires the `outcome` route to show grip movement on exactly the
/// repaired seam: a moved row carrying the repaired `seam_id` with a
/// `weakly_gripped` before class and a `strongly_gripped` after class,
/// plus aggregate weakness before and strength after. A moved row for any
/// other seam cannot satisfy this gate.
fn outcome_movement(outcome_stdout: &str, seam_id: &str) -> Result<(usize, u64, u64), String> {
    let parsed: Value = serde_json::from_str(outcome_stdout)
        .map_err(|error| format!("parse installed outcome JSON: {error}"))?;
    let moved = parsed
        .get("moved")
        .and_then(Value::as_array)
        .ok_or_else(|| "installed outcome JSON holds no moved array".to_string())?;
    if moved.is_empty() {
        return Err("installed outcome moved no seam; refusing the repair claim".to_string());
    }
    let repaired_moved = moved.iter().any(|row| {
        row.get("seam_id").and_then(Value::as_str) == Some(seam_id)
            && row.get("before").and_then(Value::as_str) == Some("weakly_gripped")
            && row.get("after").and_then(Value::as_str) == Some("strongly_gripped")
    });
    if !repaired_moved {
        return Err(format!(
            "installed outcome shows no weak-to-strong move for seam `{seam_id}`; refusing the repair claim"
        ));
    }
    let before_weak = parsed
        .get("before")
        .and_then(|before| before.get("weakly_gripped"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let after_strong = parsed
        .get("after")
        .and_then(|after| after.get("strongly_gripped"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if before_weak < 1 || after_strong < 1 {
        return Err(format!(
            "installed outcome shows no weak-to-strong grip movement (before weak {before_weak}, after strong {after_strong})"
        ));
    }
    Ok((moved.len(), before_weak, after_strong))
}

/// Narrows a receipt artifact to the repaired seam and requires the
/// `improved` change verdict. The receipt JSON is read from the
/// after-phase-written artifact: the explicit `agent receipt` re-invocation
/// route refuses with a stale after-verdict binding on current main
/// (#1738), so the journey consumes the phase-written artifact the loop
/// itself produced and verified instead of driving the broken route.
fn receipt_change(receipt_stdout: &str, seam_id: &str) -> Result<String, String> {
    let parsed: Value = serde_json::from_str(receipt_stdout)
        .map_err(|error| format!("parse installed receipt JSON: {error}"))?;
    let seam = parsed
        .get("seam")
        .ok_or_else(|| "installed receipt JSON holds no seam".to_string())?;
    let observed = seam
        .get("seam_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "installed receipt seam holds no seam_id".to_string())?;
    if observed != seam_id {
        return Err(format!(
            "installed receipt covers seam `{observed}`, not the repaired `{seam_id}`"
        ));
    }
    let change = seam
        .get("change")
        .and_then(Value::as_str)
        .ok_or_else(|| "installed receipt seam holds no change".to_string())?;
    if change != "improved" {
        return Err(format!(
            "installed receipt change is `{change}`, not `improved`; refusing the repair claim"
        ));
    }
    Ok(change.to_string())
}

struct RepairEvidence {
    repo_rel: String,
    finding_id: String,
    seam_id: String,
    attempt_id: String,
    test_running: u64,
    test_passed: u64,
    test_failed: u64,
    movement_closed: u64,
    movement_improved: u64,
    movement_regressed: u64,
    outcome_moved: usize,
    receipt_change: String,
}

/// Runs the installed repair journey (issue steps 4-7) on an isolated
/// checkout: finding to seam resolution, `agent start`, `repair --phase
/// before`, the fixed external test-only edit, separate focused test
/// execution, `repair --phase after`, and the explicit `agent verify` +
/// `agent receipt` routes. RIPR performs no edit and runs no test; the
/// harness owns the edit and the test invocation, and every verdict is
/// re-parsed from installed output — never trusted from exit status.
fn run_repair_journey(
    harness: &mut Harness,
    subject: &InstalledSubject,
    fixture_root: &Path,
) -> Result<RepairEvidence, String> {
    admit_installed_executable(subject, &subject.executable)?;
    let label = "repair";
    let repo = build_fixture_repo(harness, fixture_root, label, true)?;
    let repo_rel = format!("{label}/{}", FIXTURE_REPO_REL.join("/"));
    let executable = subject.executable.to_string_lossy().to_string();
    let root = repo.path.to_string_lossy().to_string();
    let home = fixture_root.join("home-repair");
    let cache = fixture_root.join("cache-repair");
    let build = fixture_root.join("build-repair");
    for dir in [&home, &cache, &build] {
        std::fs::create_dir_all(dir)
            .map_err(|error| format!("create repair dir {}: {error}", dir.display()))?;
    }
    // Toolchain roots stay ambient so `cargo test` resolves the pinned
    // toolchain under the fresh HOME; the build tree stays inside the
    // harness-owned fixture root so build residue never enters the
    // checkout the edit cage guards. Configured roots outside `$HOME`
    // (container layouts) are preserved as-is; the `$HOME`-derived guess
    // applies only when the variable is unset.
    let ambient_home = std::env::var("HOME").map_err(|error| format!("ambient HOME: {error}"))?;
    let isolated = (
        fixture_root
            .join("home-repair")
            .to_string_lossy()
            .to_string(),
        fixture_root
            .join("cache-repair")
            .to_string_lossy()
            .to_string(),
        fixture_root
            .join("build-repair")
            .to_string_lossy()
            .to_string(),
        prefer_ambient_toolchain_root(
            std::env::var("RUSTUP_HOME").ok(),
            &format!("{ambient_home}/.rustup"),
        ),
        prefer_ambient_toolchain_root(
            std::env::var("CARGO_HOME").ok(),
            &format!("{ambient_home}/.cargo"),
        ),
    );
    let product_envs = [
        ("RIPR_CACHE_DIR", isolated.1.as_str()),
        ("HOME", isolated.0.as_str()),
    ];
    let base_sha = repo.base_sha.clone();
    let findings_stdout = journey_run(
        harness,
        "installed-repair-check-json",
        &executable,
        &[
            "check".to_string(),
            "--root".to_string(),
            root.clone(),
            "--base".to_string(),
            base_sha.clone(),
            "--json".to_string(),
        ],
        &product_envs,
    )
    .map_err(|error| format!("installed repair check failed: {error}"))?;
    let findings: Value = serde_json::from_str(&findings_stdout)
        .map_err(|error| format!("parse installed repair findings: {error}"))?;
    let findings_array = findings
        .get("findings")
        .and_then(Value::as_array)
        .ok_or_else(|| "installed repair findings hold no findings array".to_string())?;
    if findings_array.len() != 1 {
        return Err(format!(
            "installed repair check must hold exactly one finding; observed {}",
            findings_array.len()
        ));
    }
    let finding_id = findings_array[0]
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| "installed repair finding holds no id".to_string())?
        .to_string();
    let packets_stdout = journey_run(
        harness,
        "installed-repair-seam-packets",
        &executable,
        &[
            "check".to_string(),
            "--root".to_string(),
            root.clone(),
            "--base".to_string(),
            base_sha.clone(),
            "--format".to_string(),
            "agent-seam-packets-json".to_string(),
        ],
        &product_envs,
    )
    .map_err(|error| format!("installed seam packets failed: {error}"))?;
    let seam_id = repair_seam_id(&packets_stdout)?;
    journey_run(
        harness,
        "installed-agent-start",
        &executable,
        &[
            "agent".to_string(),
            "start".to_string(),
            "--root".to_string(),
            root.clone(),
            "--seam-id".to_string(),
            seam_id.clone(),
        ],
        &product_envs,
    )
    .map_err(|error| format!("installed agent start failed: {error}"))?;
    journey_run(
        harness,
        "installed-repair-before",
        &executable,
        &[
            "agent".to_string(),
            "repair".to_string(),
            "--root".to_string(),
            root.clone(),
            "--seam-id".to_string(),
            seam_id.clone(),
            "--phase".to_string(),
            "before".to_string(),
        ],
        &product_envs,
    )
    .map_err(|error| format!("installed repair before failed: {error}"))?;
    let attempt_id = repair_attempt_id(&repo.path)?;
    // The persisted attempt record must bind our seam: the before phase
    // prints mixed human guidance to stdout, so the manifest — not stdout
    // parsing — is the authority for what the loop actually opened.
    let attempt_pre: Value = serde_json::from_str(
        &std::fs::read_to_string(
            repo.path
                .join("target")
                .join("ripr")
                .join("repair-attempts")
                .join(&attempt_id)
                .join("attempt.json"),
        )
        .map_err(|error| format!("read pre-edit attempt manifest: {error}"))?,
    )
    .map_err(|error| format!("parse pre-edit attempt manifest: {error}"))?;
    let attempt_seam = attempt_pre
        .get("seam_id")
        .and_then(Value::as_str)
        .ok_or_else(|| "attempt manifest holds no seam_id".to_string())?;
    if attempt_seam != seam_id {
        return Err(format!(
            "installed repair before opened seam `{attempt_seam}`, not `{seam_id}`"
        ));
    }
    // The fixed external test-only edit: the harness appends to the test
    // file, which must still be byte-identical to the fixture baseline —
    // modification of existing content refuses here.
    let test_path = repo.path.join("tests").join("boundary.rs");
    let test_before = std::fs::read_to_string(&test_path)
        .map_err(|error| format!("read fixture test: {error}"))?;
    if test_before != FIXTURE_TEST {
        return Err("fixture test changed before the repair edit; refusing".to_string());
    }
    std::fs::write(&test_path, format!("{test_before}{REPAIR_TEST_EDIT}"))
        .map_err(|error| format!("apply repair test edit: {error}"))?;
    // The focused test runs with the checkout as its real working
    // directory (the process cwd is never mutated), under the shared
    // timeout authority so a wedged build cannot hang the harness. The
    // toolchain roots stay ambient; everything else is harness-isolated.
    let cargo_envs = [
        ("RIPR_CACHE_DIR", isolated.1.as_str()),
        ("HOME", isolated.0.as_str()),
        ("CARGO_TARGET_DIR", isolated.2.as_str()),
        ("RUSTUP_HOME", isolated.3.as_str()),
        ("CARGO_HOME", isolated.4.as_str()),
    ];
    let test_argv = [
        "test".to_string(),
        "--test".to_string(),
        "boundary".to_string(),
        REPAIR_TEST_NAME.to_string(),
    ];
    let mut cargo_argv = vec!["cargo".to_string()];
    cargo_argv.extend(test_argv.iter().cloned());
    let ledger_env: Vec<(String, String)> = cargo_envs
        .iter()
        .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
        .collect();
    let started_test = unix_epoch_secs();
    let captured = run::capture_output_in_dir_with_timeout_bounded(
        std::path::Path::new("cargo"),
        &test_argv,
        &cargo_envs,
        &repo.path,
        std::time::Duration::from_mins(10),
        1024 * 1024,
        "focused cargo test",
    )
    .map_err(|error| format!("focused test failed to run: {error}"))?;
    if captured.timed_out {
        harness.ledger.push(LedgerEntry {
            step: "focused-test".to_string(),
            argv: cargo_argv,
            cwd: repo.path.to_string_lossy().to_string(),
            started_epoch_secs: started_test,
            status: "focused test timed out after 600 seconds".to_string(),
            env: ledger_env,
        });
        return Err("focused test timed out after 600 seconds".to_string());
    }
    let test_status = captured
        .status
        .ok_or_else(|| "focused test did not report a process status".to_string())?;
    if !test_status.success() {
        let error = format!(
            "cargo {} failed with {test_status}\nstdout:\n{}\nstderr:\n{}",
            test_argv.join(" "),
            captured.stdout.trim(),
            captured.stderr.trim()
        );
        harness.ledger.push(LedgerEntry {
            step: "focused-test".to_string(),
            argv: cargo_argv,
            cwd: repo.path.to_string_lossy().to_string(),
            started_epoch_secs: started_test,
            status: error.clone(),
            env: ledger_env,
        });
        return Err(format!("focused test failed: {error}"));
    }
    let (running, passed, failed) = cargo_test_counts(&captured.stdout)?;
    harness.ledger.push(LedgerEntry {
        step: "focused-test".to_string(),
        argv: cargo_argv,
        cwd: repo.path.to_string_lossy().to_string(),
        started_epoch_secs: started_test,
        status: "ok".to_string(),
        env: ledger_env,
    });
    if failed > 0 {
        return Err(format!(
            "focused test run failed {failed} test(s); refusing the repair claim"
        ));
    }
    if passed < 1 {
        return Err("focused test run executed no tests; refusing the repair claim".to_string());
    }
    journey_run(
        harness,
        "installed-repair-after",
        &executable,
        &[
            "agent".to_string(),
            "repair".to_string(),
            "--root".to_string(),
            root.clone(),
            "--attempt".to_string(),
            attempt_id.clone(),
            "--phase".to_string(),
            "after".to_string(),
        ],
        &product_envs,
    )
    .map_err(|error| format!("installed repair after failed: {error}"))?;
    let attempt_path = repo
        .path
        .join("target")
        .join("ripr")
        .join("repair-attempts")
        .join(&attempt_id)
        .join("attempt.json");
    let attempt: Value = serde_json::from_str(
        &std::fs::read_to_string(&attempt_path)
            .map_err(|error| format!("read attempt manifest: {error}"))?,
    )
    .map_err(|error| format!("parse attempt manifest: {error}"))?;
    let state = attempt
        .get("state")
        .and_then(Value::as_str)
        .ok_or_else(|| "attempt manifest holds no state".to_string())?;
    if state != "ready_to_finish" {
        return Err(format!(
            "attempt state is `{state}`, not `ready_to_finish`; refusing the repair claim"
        ));
    }
    // The receipt outcome comes from the after-phase-written artifact: the
    // explicit `agent receipt` re-invocation refuses with a stale
    // after-verdict binding on current main (#1738), so the journey reads
    // the artifact the loop itself produced instead of driving that route.
    let workflow = repo.path.join("target").join("ripr").join("workflow");
    let reports = repo.path.join("target").join("ripr").join("reports");
    let receipt_path = reports.join("agent-receipt.json");
    let receipt_stdout = std::fs::read_to_string(&receipt_path)
        .map_err(|error| format!("read phase-written agent receipt: {error}"))?;
    let change = receipt_change(&receipt_stdout, &seam_id)?;
    // The explicit verify route re-runs against the phase snapshots: its
    // stdout feeds the movement gates.
    let verify_stdout = journey_run(
        harness,
        "installed-agent-verify",
        &executable,
        &[
            "agent".to_string(),
            "verify".to_string(),
            "--root".to_string(),
            root.clone(),
            "--before".to_string(),
            workflow
                .join("before.repo-exposure.json")
                .to_string_lossy()
                .to_string(),
            "--after".to_string(),
            workflow
                .join("after.repo-exposure.json")
                .to_string_lossy()
                .to_string(),
            "--json".to_string(),
        ],
        &product_envs,
    )
    .map_err(|error| format!("installed agent verify failed: {error}"))?;
    let movement = verify_movement(&verify_stdout)?;
    // The explicit outcome route renders the review receipt from the same
    // snapshots: at least one moved seam with weak-to-strong grip movement.
    let outcome_stdout = journey_run(
        harness,
        "installed-outcome",
        &executable,
        &[
            "outcome".to_string(),
            "--before".to_string(),
            workflow
                .join("before.repo-exposure.json")
                .to_string_lossy()
                .to_string(),
            "--after".to_string(),
            workflow
                .join("after.repo-exposure.json")
                .to_string_lossy()
                .to_string(),
            "--format".to_string(),
            "json".to_string(),
        ],
        &product_envs,
    )
    .map_err(|error| format!("installed outcome failed: {error}"))?;
    let (outcome_moved, _, _) = outcome_movement(&outcome_stdout, &seam_id)?;
    Ok(RepairEvidence {
        repo_rel,
        finding_id,
        seam_id,
        attempt_id,
        test_running: running,
        test_passed: passed,
        test_failed: failed,
        movement_closed: movement.closed,
        movement_improved: movement.improved,
        movement_regressed: movement.regressed,
        outcome_moved,
        receipt_change: change,
    })
}

fn repair_json(evidence: &RepairEvidence) -> Value {
    json!({
        "repo": evidence.repo_rel,
        "finding_id": evidence.finding_id,
        "seam_id": evidence.seam_id,
        "attempt_id": evidence.attempt_id,
        "test": {
            "running": evidence.test_running,
            "passed": evidence.test_passed,
            "failed": evidence.test_failed,
        },
        "movement": {
            "closed": evidence.movement_closed,
            "improved": evidence.movement_improved,
            "regressed": evidence.movement_regressed,
        },
        "outcome_moved": evidence.outcome_moved,
        "receipt_change": evidence.receipt_change,
    })
}

// ---------------------------------------------------------------------------
// Slice G: standalone-LSP journey (`ripr lsp --stdio` over real pipes)
// ---------------------------------------------------------------------------

/// Product command identifiers the journey observes in the initialize
/// result instead of assuming: the server owns its command vocabulary.
const LSP_STATUS_COMMAND: &str = "ripr.collectWorkspaceStatus";
/// Each blocking wait on the installed server is bounded: a wedged server
/// refuses the journey instead of hanging the harness.
const LSP_RESPONSE_TIMEOUT_SECS: u64 = 60;
/// After the exit notification the server must be gone quickly: a live
/// process past this budget is killed and refuses the journey as an
/// orphan — never silently adopted.
const LSP_EXIT_TIMEOUT_SECS: u64 = 10;
/// The saved-workspace analysis poll budget: didSave refresh runs
/// asynchronously, so the status command repeats until a committed `full`
/// snapshot binds the opened document — or the journey refuses.
const LSP_ANALYSIS_TIMEOUT_SECS: u64 = 60;

/// Percent-encodes one filesystem path as an LSP `file:` URI. Only
/// unreserved characters, `/`, and `:` survive literally; everything else
/// (spaces, non-ASCII, `#`, `?`) becomes UTF-8 `%XX`. The fixture checkout
/// deliberately carries spaces and non-ASCII text, so this encoding — not
/// string concatenation — is what the initialize `rootUri` uses.
fn lsp_file_uri(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let mut encoded = String::with_capacity(normalized.len() + 16);
    for byte in normalized.bytes() {
        if matches!(
            byte,
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' | b':'
        ) {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    if encoded.starts_with('/') {
        format!("file://{encoded}")
    } else {
        format!("file:///{encoded}")
    }
}

/// Frames one JSON-RPC value the way a generic LSP client sends it:
/// `Content-Length` headers, CRLF CRLF, then exactly the body bytes.
fn encode_lsp_message(value: &Value) -> Vec<u8> {
    let body = serde_json::to_string(value).unwrap_or_else(|_| "null".to_string());
    let mut framed = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    framed.extend_from_slice(body.as_bytes());
    framed
}

/// Splits complete LSP frames off the front of a byte buffer: returns the
/// parsed values plus the unconsumed remainder (a trailing partial frame).
/// Anything that is not strict `Content-Length: N CRLF CRLF N JSON bytes`
/// refuses — framing drift is a journey failure, never skipped content.
fn parse_lsp_frames(buffer: &[u8]) -> Result<(Vec<Value>, Vec<u8>), String> {
    const SEPARATOR: &[u8] = b"\r\n\r\n";
    let mut frames = Vec::new();
    let mut rest = buffer;
    loop {
        if rest.is_empty() {
            break;
        }
        let Some(end) = rest
            .windows(SEPARATOR.len())
            .position(|window| window == SEPARATOR)
        else {
            break;
        };
        let (header_bytes, after) = rest.split_at(end);
        let headers = std::str::from_utf8(header_bytes)
            .map_err(|error| format!("LSP header is not UTF-8: {error}"))?;
        let mut length: Option<usize> = None;
        for line in headers.split("\r\n") {
            let Some((name, value)) = line.split_once(':') else {
                return Err(format!("LSP header line holds no colon: `{line}`"));
            };
            if name.trim().eq_ignore_ascii_case("content-length") {
                length = Some(
                    value
                        .trim()
                        .parse::<usize>()
                        .map_err(|error| format!("LSP Content-Length is not a number: {error}"))?,
                );
            }
        }
        let length =
            length.ok_or_else(|| "LSP frame holds no Content-Length header".to_string())?;
        let body = &after[SEPARATOR.len()..];
        if body.len() < length {
            break;
        }
        let (body, remaining) = body.split_at(length);
        let value: Value = serde_json::from_slice(body)
            .map_err(|error| format!("LSP body is not JSON: {error}"))?;
        frames.push(value);
        rest = remaining;
    }
    Ok((frames, rest.to_vec()))
}

/// Waits for the response carrying one request id against one absolute
/// deadline, collecting id-less server notifications on the way. A foreign
/// response id, a response `error`, or the deadline refuses — the journey
/// never pairs a verdict with the wrong request, and a chatty server can
/// never stretch one wait past its deadline by dribbling notifications.
fn await_lsp_response(
    receiver: &std::sync::mpsc::Receiver<Result<Option<Value>, String>>,
    id: u64,
    deadline: std::time::Instant,
    notifications: &mut Vec<Value>,
) -> Result<Value, String> {
    loop {
        if deadline
            .saturating_duration_since(std::time::Instant::now())
            .is_zero()
        {
            return Err(format!(
                "installed LSP request {id} exceeded its response deadline"
            ));
        }
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        let frame = receiver.recv_timeout(remaining).map_err(|error| {
            format!("installed LSP server answered nothing for request {id}: {error}")
        })??;
        let Some(message) = frame else {
            return Err(format!(
                "installed LSP stdout closed before request {id} was answered"
            ));
        };
        match message.get("id").and_then(Value::as_u64) {
            Some(observed) if observed == id => {
                if let Some(error) = message.get("error") {
                    return Err(format!("installed LSP request {id} errored: {error}"));
                }
                return Ok(message);
            }
            Some(observed) => {
                return Err(format!(
                    "installed LSP answered request {id} with foreign response id {observed}; refusing"
                ));
            }
            None => notifications.push(message),
        }
    }
}

/// Requires the initialize result to advertise exactly the surface the
/// journey drives: hover plus the workspace-status command. A server that
/// answers initialize without that surface cannot satisfy the bounded
/// saved-workspace inspection below.
fn lsp_initialize_capabilities(response: &Value) -> Result<Vec<String>, String> {
    let capabilities = response
        .get("result")
        .and_then(|result| result.get("capabilities"))
        .ok_or_else(|| "installed LSP initialize holds no result capabilities".to_string())?;
    if capabilities.get("hoverProvider") != Some(&Value::Bool(true)) {
        return Err("installed LSP server advertises no hover provider".to_string());
    }
    let commands = capabilities
        .get("executeCommandProvider")
        .and_then(|provider| provider.get("commands"))
        .and_then(Value::as_array)
        .ok_or_else(|| "installed LSP server advertises no execute commands".to_string())?;
    let names = commands
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    if !names.iter().any(|name| name == LSP_STATUS_COMMAND) {
        return Err(format!(
            "installed LSP server advertises no `{LSP_STATUS_COMMAND}` command"
        ));
    }
    Ok(names)
}

/// Requires the workspace-status result to bind the saved workspace the
/// journey opened: `selected_single_root` over exactly the fixture
/// checkout. Any other root state, a divergent effective root, or a
/// command-level error refuses the saved-workspace claim.
fn lsp_workspace_status(response: &Value, expected_root: &str) -> Result<String, String> {
    let status = response
        .get("result")
        .and_then(|result| result.get("analysis_status"))
        .ok_or_else(|| "installed LSP status holds no analysis_status".to_string())?;
    let root_state = status
        .get("root_state")
        .and_then(Value::as_str)
        .ok_or_else(|| "installed LSP status holds no root_state".to_string())?;
    if root_state != "selected_single_root" {
        return Err(format!(
            "installed LSP root state is `{root_state}`, not `selected_single_root`"
        ));
    }
    let effective = status
        .get("effective_root")
        .and_then(Value::as_str)
        .ok_or_else(|| "installed LSP status holds no effective_root".to_string())?;
    if effective != expected_root {
        return Err(format!(
            "installed LSP effective root is `{effective}`, not the journey checkout `{expected_root}`"
        ));
    }
    Ok(root_state.to_string())
}

/// Poll outcome for the saved-workspace analysis gate: either the opened
/// document's saved content is committed into a full snapshot, the
/// analysis is still in flight, or the state refuses the claim outright.
enum LspAnalysisPoll {
    Ready(String),
    Pending,
}

/// Requires the workspace-status result to prove the saved workspace was
/// analyzed, not merely selected: a committed `full` snapshot plus the
/// opened document bound to its saved content by digest — clean state,
/// recorded save, analyzed save, all equal. Anything else is either still
/// in flight (`Pending`) or a hard refusal.
fn lsp_saved_workspace_analysis(
    response: &Value,
    doc_uri: &str,
) -> Result<LspAnalysisPoll, String> {
    let result = response
        .get("result")
        .ok_or_else(|| "installed LSP status holds no result".to_string())?;
    let run_status = result
        .get("run_status")
        .and_then(Value::as_str)
        .ok_or_else(|| "installed LSP status holds no run_status".to_string())?;
    if run_status != "full" {
        return Ok(LspAnalysisPoll::Pending);
    }
    let documents = result
        .get("open_documents")
        .and_then(Value::as_array)
        .ok_or_else(|| "installed LSP status holds no open_documents".to_string())?;
    let document = documents
        .iter()
        .find(|document| document.get("uri").and_then(Value::as_str) == Some(doc_uri))
        .ok_or_else(|| format!("installed LSP status omits the opened document `{doc_uri}`"))?;
    if document.get("state").and_then(Value::as_str) != Some("clean") {
        return Err(format!(
            "installed LSP opened document is not clean: {document}"
        ));
    }
    let saved = document.get("last_saved_content_identity");
    let analyzed = document.get("analyzed_saved_content_identity");
    match (saved, analyzed) {
        (Some(saved), Some(analyzed))
            if !saved.is_null() && !analyzed.is_null() && saved == analyzed =>
        {
            Ok(LspAnalysisPoll::Ready(
                analyzed
                    .as_str()
                    .unwrap_or("non-string-identity")
                    .to_string(),
            ))
        }
        _ => Ok(LspAnalysisPoll::Pending),
    }
}

struct LspEvidence {
    repo_rel: String,
    root_state: String,
    effective_root: String,
    run_status: String,
    analyzed_saved_content_identity: String,
    status_polls: u32,
    frames_read: usize,
    notifications: usize,
    stdout_bytes: u64,
    stderr_bytes: u64,
}

/// Runs the standalone-LSP journey (issue step 8) against the installed
/// binary over real OS pipes with a generic framed client: initialize,
/// initialized, didOpen plus didSave of the real fixture file, one bounded
/// `ripr.collectWorkspaceStatus` inspection, shutdown, exit. Stdout must
/// parse as strict LSP frames end to end, and the server process must be
/// gone after exit — no VSIX path, no in-process substitute.
fn run_lsp_journey(
    harness: &mut Harness,
    subject: &InstalledSubject,
    fixture_root: &Path,
) -> Result<LspEvidence, String> {
    admit_installed_executable(subject, &subject.executable)?;
    let label = "lsp";
    let repo = build_fixture_repo(harness, fixture_root, label, true)?;
    let repo_rel = format!("{label}/{}", FIXTURE_REPO_REL.join("/"));
    let home = fixture_root.join("home-lsp");
    let cache = fixture_root.join("cache-lsp");
    for dir in [&home, &cache] {
        std::fs::create_dir_all(dir)
            .map_err(|error| format!("create LSP dir {}: {error}", dir.display()))?;
    }
    let product_envs = [
        ("RIPR_CACHE_DIR", cache.to_string_lossy().to_string()),
        ("HOME", home.to_string_lossy().to_string()),
    ];
    let lib_path = repo.path.join("src").join("lib.rs");
    let lib_text = std::fs::read_to_string(&lib_path)
        .map_err(|error| format!("read fixture lib for didOpen: {error}"))?;
    if lib_text != FIXTURE_LIB_HEAD {
        return Err("fixture lib changed before the LSP journey; refusing".to_string());
    }
    let root_uri = lsp_file_uri(&repo.path);
    let doc_uri = lsp_file_uri(&lib_path);
    let expected_root = repo.path.to_string_lossy().replace('\\', "/");
    let started = unix_epoch_secs();
    let mut child = std::process::Command::new(&subject.executable)
        .args(["lsp", "--stdio"])
        .envs(product_envs.iter().map(|(k, v)| (k, v)))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|error| format!("spawn installed LSP server: {error}"))?;
    // The reader owns server stdout: strict frames stream to the channel,
    // clean EOF (None) or the first framing violation (Err) ends it.
    let (sender, receiver) = std::sync::mpsc::channel::<Result<Option<Value>, String>>();
    let reader = child
        .stdout
        .take()
        .ok_or_else(|| "LSP server holds no stdout".to_string())?;
    let reader_handle = std::thread::spawn(move || {
        use std::io::Read as _;
        let mut stdout = reader;
        let mut buffer = Vec::new();
        let mut chunk = [0_u8; 8192];
        let mut stdout_bytes: u64 = 0;
        loop {
            match stdout.read(&mut chunk) {
                Ok(0) => {
                    if buffer.is_empty() {
                        let _ = sender.send(Ok(None));
                    } else {
                        let _ = sender.send(Err(format!(
                            "installed LSP stdout ends with {} unframed byte(s)",
                            buffer.len()
                        )));
                    }
                    break;
                }
                Ok(read) => {
                    stdout_bytes += read as u64;
                    buffer.extend_from_slice(&chunk[..read]);
                    match parse_lsp_frames(&buffer) {
                        Ok((frames, rest)) => {
                            buffer = rest;
                            for frame in frames {
                                if sender.send(Ok(Some(frame))).is_err() {
                                    return stdout_bytes;
                                }
                            }
                        }
                        Err(error) => {
                            let _ = sender.send(Err(error));
                            break;
                        }
                    }
                }
                Err(error) => {
                    let _ = sender.send(Err(format!("read installed LSP stdout: {error}")));
                    break;
                }
            }
        }
        stdout_bytes
    });
    // Server stderr streams to a second thread so a chatty server can never
    // block on a full pipe while the journey waits for a response.
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "LSP server holds no stderr".to_string())?;
    let stderr_handle = std::thread::spawn(move || {
        use std::io::Read as _;
        let mut stderr = stderr;
        let mut bytes = Vec::new();
        let _ = stderr.read_to_end(&mut bytes);
        bytes
    });
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "LSP server holds no stdin".to_string())?;
    let mut notifications = Vec::new();
    let send = |stdin: &mut std::process::ChildStdin, value: &Value| -> Result<(), String> {
        use std::io::Write as _;
        stdin
            .write_all(&encode_lsp_message(value))
            .and_then(|()| stdin.flush())
            .map_err(|error| format!("write installed LSP stdin: {error}"))
    };
    let response_deadline =
        || std::time::Instant::now() + std::time::Duration::from_secs(LSP_RESPONSE_TIMEOUT_SECS);
    let status_command = |id: u64| {
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "workspace/executeCommand",
            "params": {"command": LSP_STATUS_COMMAND, "arguments": []},
        })
    };
    let mut next_id = 1_u64;
    let mut status_polls = 0_u32;
    let outcome: Result<LspEvidence, String> = (|| {
        let id = next_id;
        next_id += 1;
        send(
            &mut stdin,
            &json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "initialize",
                "params": {
                    "processId": null,
                    "rootUri": root_uri,
                    "capabilities": {},
                },
            }),
        )?;
        let initialize =
            await_lsp_response(&receiver, id, response_deadline(), &mut notifications)?;
        lsp_initialize_capabilities(&initialize)?;
        send(
            &mut stdin,
            &json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}),
        )?;
        send(
            &mut stdin,
            &json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didOpen",
                "params": {
                    "textDocument": {
                        "uri": doc_uri,
                        "languageId": "rust",
                        "version": 1,
                        "text": lib_text,
                    },
                },
            }),
        )?;
        send(
            &mut stdin,
            &json!({
                "jsonrpc": "2.0",
                "method": "textDocument/didSave",
                "params": {"textDocument": {"uri": doc_uri}},
            }),
        )?;
        // Root selection alone does not prove the save was analyzed: poll
        // the status command until a committed `full` snapshot binds the
        // opened document to its saved content by digest, or refuse at the
        // poll deadline.
        let poll_deadline =
            std::time::Instant::now() + std::time::Duration::from_secs(LSP_ANALYSIS_TIMEOUT_SECS);
        let (root_state, analyzed_identity) = loop {
            if std::time::Instant::now() >= poll_deadline {
                return Err(format!(
                    "installed LSP saved workspace analysis never committed within {LSP_ANALYSIS_TIMEOUT_SECS}s"
                ));
            }
            let id = next_id;
            next_id += 1;
            send(&mut stdin, &status_command(id))?;
            status_polls += 1;
            let status = await_lsp_response(&receiver, id, poll_deadline, &mut notifications)?;
            let bound = lsp_workspace_status(&status, &expected_root)?;
            match lsp_saved_workspace_analysis(&status, &doc_uri)? {
                LspAnalysisPoll::Ready(digest) => break (bound, digest),
                LspAnalysisPoll::Pending => {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
            }
        };
        let id = next_id;
        next_id += 1;
        send(
            &mut stdin,
            &json!({"jsonrpc": "2.0", "id": id, "method": "shutdown", "params": null}),
        )?;
        let shutdown = await_lsp_response(&receiver, id, response_deadline(), &mut notifications)?;
        if shutdown.get("result") != Some(&Value::Null) {
            return Err(format!(
                "installed LSP shutdown returned a non-null result: {}",
                shutdown.get("result").unwrap_or(&Value::Null)
            ));
        }
        send(
            &mut stdin,
            &json!({"jsonrpc": "2.0", "method": "exit", "params": null}),
        )?;
        // stdin drops in the outer cleanup below so the reap always runs,
        // even when the exchange above refused midway.
        // The exit notification must actually stop the server: poll for the
        // process, then kill-and-refuse when it lingers as an orphan.
        let mut waited_ms = 0_u64;
        let exit_status = loop {
            if let Some(status) = child
                .try_wait()
                .map_err(|error| format!("poll installed LSP server: {error}"))?
            {
                break status;
            }
            if waited_ms >= LSP_EXIT_TIMEOUT_SECS * 1000 {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "installed LSP server still alive {}s after exit; killed as an orphan",
                    LSP_EXIT_TIMEOUT_SECS
                ));
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
            waited_ms += 50;
        };
        if !exit_status.success() {
            return Err(format!("installed LSP server exited with {exit_status}"));
        }
        // Drain the reader to clean EOF so every stdout byte proves framed;
        // the channel already refused any framing violation inline. Every
        // issued request id consumed exactly one response above, so the
        // total frame count adds those plus the interleaved notifications
        // observed and the drained remainder.
        let drain_deadline = response_deadline();
        let mut drained = 0_usize;
        loop {
            if drain_deadline
                .saturating_duration_since(std::time::Instant::now())
                .is_zero()
            {
                return Err("installed LSP reader stalled before clean EOF".to_string());
            }
            let remaining = drain_deadline.saturating_duration_since(std::time::Instant::now());
            match receiver.recv_timeout(remaining) {
                Ok(Ok(Some(_))) => drained += 1,
                Ok(Ok(None)) => break,
                Ok(Err(error)) => return Err(error),
                Err(_) => {
                    return Err("installed LSP reader stalled before clean EOF".to_string());
                }
            }
        }
        let frames_read = (next_id - 1) as usize + notifications.len() + drained;
        Ok(LspEvidence {
            repo_rel,
            root_state,
            effective_root: expected_root,
            run_status: "full".to_string(),
            analyzed_saved_content_identity: analyzed_identity,
            status_polls,
            frames_read,
            notifications: notifications.len(),
            stdout_bytes: 0,
            stderr_bytes: 0,
        })
    })();
    // Cleanup on every path before the pipe readers join: drop stdin, reap
    // the child (kill when it lingers past a short grace), and only then
    // join. A live server — the malfunctioning candidate this harness is
    // built to test — can never hold the joins hostage or escape as an
    // orphan.
    drop(stdin);
    let mut reaped_ms = 0_u64;
    loop {
        match child
            .try_wait()
            .map_err(|error| format!("reap installed LSP server: {error}"))?
        {
            Some(_) => break,
            None if reaped_ms >= 2_000 => {
                let _ = child.kill();
                let _ = child.wait();
                break;
            }
            None => {
                std::thread::sleep(std::time::Duration::from_millis(50));
                reaped_ms += 50;
            }
        }
    }
    // Reader and stderr threads join after the reap: byte counts come from
    // the threads even when the exchange above refused midway.
    let stdout_bytes = reader_handle.join().unwrap_or(0);
    let stderr_bytes_vec = stderr_handle.join().unwrap_or_default();
    let stderr_bytes = stderr_bytes_vec.len() as u64;
    let mut evidence = outcome?;
    evidence.stdout_bytes = stdout_bytes;
    evidence.stderr_bytes = stderr_bytes;
    if evidence.stdout_bytes == 0 {
        return Err("installed LSP server wrote no framed stdout".to_string());
    }
    let mut argv = vec![subject.executable.to_string_lossy().to_string()];
    argv.extend(["lsp".to_string(), "--stdio".to_string()]);
    harness.ledger.push(LedgerEntry {
        step: "installed-lsp-stdio".to_string(),
        argv,
        cwd: std::env::current_dir()
            .map_err(|error| format!("current dir: {error}"))?
            .to_string_lossy()
            .to_string(),
        started_epoch_secs: started,
        status: format!(
            "ok root_state={} run={} polls={} frames={} notifications={} stdout={}B stderr={}B",
            evidence.root_state,
            evidence.run_status,
            evidence.status_polls,
            evidence.frames_read,
            evidence.notifications,
            evidence.stdout_bytes,
            evidence.stderr_bytes,
        ),
        env: product_envs
            .iter()
            .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
            .collect(),
    });
    Ok(evidence)
}

fn lsp_json(evidence: &LspEvidence) -> Value {
    json!({
        "repo": evidence.repo_rel,
        "root_state": evidence.root_state,
        "effective_root": evidence.effective_root,
        "run_status": evidence.run_status,
        "analyzed_saved_content_identity": evidence.analyzed_saved_content_identity,
        "status_polls": evidence.status_polls,
        "frames_read": evidence.frames_read,
        "notifications": evidence.notifications,
        "stdout_bytes": evidence.stdout_bytes,
        "stderr_bytes": evidence.stderr_bytes,
        "orphan": false,
    })
}

// ---------------------------------------------------------------------------
// Receipt skeleton
// ---------------------------------------------------------------------------

fn ledger_json(entry: &LedgerEntry) -> Value {
    let mut env_map = serde_json::Map::new();
    let mut pairs = entry.env.clone();
    pairs.sort();
    for (name, value) in pairs {
        env_map.insert(name, Value::String(value));
    }
    json!({
        "step": entry.step,
        "argv": entry.argv,
        "cwd": entry.cwd,
        "started_epoch_secs": entry.started_epoch_secs,
        "status": entry.status,
        "env": env_map,
    })
}

fn write_receipt(
    out: &Path,
    subject: &InstalledSubject,
    harness: &Harness,
    journey: Option<&JourneyEvidence>,
    init: Option<&InitEvidence>,
    repair: Option<&RepairEvidence>,
    lsp: Option<&LspEvidence>,
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
        "init": init.map(init_json),
        "repair": repair.map(repair_json),
        "lsp": lsp.map(lsp_json),
        "slices_completed": if lsp.is_some() {
            vec!["A", "B", "E", "F", "G"]
        } else if repair.is_some() {
            vec!["A", "B", "E", "F"]
        } else if init.is_some() {
            vec!["A", "B", "E"]
        } else if journey.is_some() {
            vec!["A", "B"]
        } else {
            vec!["A"]
        },
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

/// Resolved and ownership-checked harness paths, shared by the journey and
/// the controls entry: no command installs or creates anything before these
/// checks pass.
struct FirstHourPaths {
    prefix: PathBuf,
    out: PathBuf,
    fixture_root: PathBuf,
}

fn guard_paths(command: &str, parsed: &FirstHourArgs) -> Result<FirstHourPaths, String> {
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
            "{command} --fixture-root `{}` already exists; the harness never deletes pre-existing directories",
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
            "{command} --out `{}` must not equal or nest under --fixture-root `{}`; the receipt would be cleaned before return",
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
            "{command} --prefix `{}` must not equal or nest under --fixture-root `{}`; the install tree would be cleaned before return",
            prefix.display(),
            fixture_root.display()
        ));
    }
    Ok(FirstHourPaths {
        prefix,
        out: out_root,
        fixture_root,
    })
}

pub(crate) fn first_hour(args: &[String]) -> Result<(), String> {
    let parsed = parse_args("first-hour", USAGE, args)?;
    let mut harness = Harness {
        roots: Vec::new(),
        ledger: Vec::new(),
    };
    let paths = guard_paths("first-hour", &parsed)?;
    let subject = install_package(&mut harness, &parsed.crate_path, &paths.prefix)?;
    admit_installed_executable(&subject, &subject.executable)?;
    std::fs::create_dir_all(&paths.fixture_root)
        .map_err(|error| format!("create fixture root: {error}"))?;
    harness.roots.push(paths.fixture_root.clone());
    // Slice B: baseline fixture plus the installed check journey. Every
    // product invocation resolves through the admitted installed
    // executable; the journey fails loudly when the boundary change goes
    // unobserved or the human front door does not render.
    let repo = build_fixture_repo(&mut harness, &paths.fixture_root, "journey", true)?;
    let base_sha = repo.base_sha.clone();
    let journey = run_check_journey(
        &mut harness,
        &subject,
        &repo,
        &paths.fixture_root,
        "journey",
        &base_sha,
    )?;
    // Slice E: installed init journey on its own isolated checkout.
    let init = run_init_journey(&mut harness, &subject, &paths.fixture_root)?;
    // Slice F: installed repair journey on its own isolated checkout.
    let repair = run_repair_journey(&mut harness, &subject, &paths.fixture_root)?;
    // Slice G: standalone-LSP journey on its own isolated checkout.
    let lsp = run_lsp_journey(&mut harness, &subject, &paths.fixture_root)?;
    write_receipt(
        &paths.out,
        &subject,
        &harness,
        Some(&journey),
        Some(&init),
        Some(&repair),
        Some(&lsp),
    )?;
    println!(
        "first-hour slice G: {} finding(s) [{}] + init advisory-ok + repair {} + lsp {} through {} ledger {} steps",
        journey.evidence.findings,
        journey.evidence.classifications.join(","),
        repair.receipt_change,
        lsp.root_state,
        subject.executable.display(),
        harness.ledger.len()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Slice C: negative controls against the production journey
// ---------------------------------------------------------------------------

const CONTROLS_USAGE: &str = "\
cargo xtask first-hour-controls --crate <path.crate> --prefix <clean-dir> --out <receipt-dir>
  --fixture-root <dir>
Installs the packaged candidate once, then drives the production journey
with three dishonest states. Each control passes only when the journey
refuses with its typed error; an unexpected success or a different error
fails the run. Writes a controls receipt and exits nonzero unless every
control observes its refusal.";

const CONTROLS_RECEIPT_FILE: &str = "first-hour-controls.json";

/// A control that must refuse: a setup failure is reported as
/// setup-not-established, never as a pass and never as a journey failure.
struct ControlReport {
    name: String,
    expected: String,
    observed: String,
    pass: bool,
    setup_ok: bool,
}

fn expect_refusal(
    name: &str,
    expected: &str,
    outcome: Result<JourneyEvidence, String>,
) -> ControlReport {
    match outcome {
        Ok(_) => ControlReport {
            name: name.to_string(),
            expected: expected.to_string(),
            observed: "unexpected success: the journey recorded a pass it cannot prove".to_string(),
            pass: false,
            setup_ok: true,
        },
        Err(error) if error.contains(expected) => ControlReport {
            name: name.to_string(),
            expected: expected.to_string(),
            observed: error,
            pass: true,
            setup_ok: true,
        },
        Err(error) => ControlReport {
            name: name.to_string(),
            expected: expected.to_string(),
            observed: error,
            pass: false,
            setup_ok: true,
        },
    }
}

/// Flips one bit mid-file so the installed digest no longer binds the
/// executable. The journey must refuse at admission, before any product
/// invocation — the control additionally asserts no `installed-check-*`
/// ledger step exists past the refusal point.
fn tamper_installed_executable(subject: &InstalledSubject) -> Result<(), String> {
    let mut bytes = std::fs::read(&subject.executable)
        .map_err(|error| format!("read installed executable for tampering: {error}"))?;
    if bytes.is_empty() {
        return Err("installed executable is empty; nothing to tamper".to_string());
    }
    let middle = bytes.len() / 2;
    bytes[middle] ^= 0xFF;
    std::fs::write(&subject.executable, &bytes)
        .map_err(|error| format!("write tampered executable: {error}"))?;
    Ok(())
}

fn product_steps_since(harness: &Harness, ledger_len: usize) -> Vec<String> {
    harness.ledger[ledger_len..]
        .iter()
        .filter(|entry| entry.step.starts_with("installed-check-"))
        .map(|entry| entry.step.clone())
        .collect()
}

fn control_report_json(report: &ControlReport) -> Value {
    json!({
        "name": report.name,
        "expected_refusal": report.expected,
        "observed": report.observed,
        "pass": report.pass,
        "setup_ok": report.setup_ok,
    })
}

fn write_controls_receipt(
    out: &Path,
    subject: &InstalledSubject,
    harness: &Harness,
    reports: &[ControlReport],
) -> Result<bool, String> {
    let all_pass = reports.iter().all(|report| report.pass);
    let receipt = json!({
        "schema_version": SCHEMA_VERSION,
        "subject": {
            "crate_sha256": subject.crate_sha256,
            "executable": subject.executable.to_string_lossy(),
            "version_output": subject.version_output,
        },
        "controls": reports.iter().map(control_report_json).collect::<Vec<_>>(),
        "all_pass": all_pass,
        "ledger": harness.ledger.iter().map(ledger_json).collect::<Vec<_>>(),
    });
    std::fs::create_dir_all(out).map_err(|error| format!("create out dir: {error}"))?;
    let text = serde_json::to_string_pretty(&receipt)
        .map_err(|error| format!("render controls receipt: {error}"))?;
    std::fs::write(out.join(CONTROLS_RECEIPT_FILE), format!("{text}\n"))
        .map_err(|error| format!("write controls receipt: {error}"))?;
    Ok(all_pass)
}

pub(crate) fn first_hour_controls(args: &[String]) -> Result<(), String> {
    let parsed = parse_args("first-hour-controls", CONTROLS_USAGE, args)?;
    let mut harness = Harness {
        roots: Vec::new(),
        ledger: Vec::new(),
    };
    let paths = guard_paths("first-hour-controls", &parsed)?;
    let subject = install_package(&mut harness, &parsed.crate_path, &paths.prefix)?;
    admit_installed_executable(&subject, &subject.executable)?;
    std::fs::create_dir_all(&paths.fixture_root)
        .map_err(|error| format!("create fixture root: {error}"))?;
    harness.roots.push(paths.fixture_root.clone());
    let mut reports = Vec::new();

    // Control 1: invalid base. Fixture setup must succeed first; the
    // installed check then fails on the bogus ref and the journey must
    // propagate that failure instead of recording evidence. The JSON gate
    // runs first, so the refusal names the json invocation.
    let invalid_setup = build_fixture_repo(&mut harness, &paths.fixture_root, "control-base", true);
    reports.push(match invalid_setup {
        Err(error) => ControlReport {
            name: "invalid-base".to_string(),
            expected: "installed check (json) failed".to_string(),
            observed: format!("setup failed: {error}"),
            pass: false,
            setup_ok: false,
        },
        Ok(repo) => {
            let bogus = "0000000000000000000000000000000000000000";
            expect_refusal(
                "invalid-base",
                "installed check (json) failed",
                run_check_journey(
                    &mut harness,
                    &subject,
                    &repo,
                    &paths.fixture_root,
                    "control-base",
                    bogus,
                ),
            )
        }
    });

    // Control 2: no change. The empty diff must surface zero findings and
    // the oracle gate must refuse the pass.
    let nochange_setup =
        build_fixture_repo(&mut harness, &paths.fixture_root, "control-empty", false);
    reports.push(match nochange_setup {
        Err(error) => ControlReport {
            name: "no-change".to_string(),
            expected: "zero findings".to_string(),
            observed: format!("setup failed: {error}"),
            pass: false,
            setup_ok: false,
        },
        Ok(repo) => {
            let base_sha = repo.base_sha.clone();
            expect_refusal(
                "no-change",
                "zero findings",
                run_check_journey(
                    &mut harness,
                    &subject,
                    &repo,
                    &paths.fixture_root,
                    "control-empty",
                    &base_sha,
                ),
            )
        }
    });

    // Control 3: tampered binary, last — tampering destroys the installed
    // tree for any later product use. Admission must refuse before any
    // product invocation: no installed-check-* step may follow.
    let tampered = (|| -> Result<ControlReport, String> {
        let repo = build_fixture_repo(&mut harness, &paths.fixture_root, "control-tamper", true)
            .map_err(|error| format!("setup failed: {error}"))?;
        tamper_installed_executable(&subject).map_err(|error| format!("setup failed: {error}"))?;
        let ledger_len = harness.ledger.len();
        let base_sha = repo.base_sha.clone();
        let mut report = expect_refusal(
            "tampered-binary",
            "wrong binary",
            run_check_journey(
                &mut harness,
                &subject,
                &repo,
                &paths.fixture_root,
                "control-tamper",
                &base_sha,
            ),
        );
        // Tripwire: with admission intact, a refusal precedes every product
        // step, so this branch fires only when admission was bypassed or
        // moved — yet product steps still ran against a tampered binary.
        let product_steps = product_steps_since(&harness, ledger_len);
        if !product_steps.is_empty() {
            report.pass = false;
            report.observed = format!(
                "product steps ran against a tampered binary: {}; {}",
                product_steps.join(","),
                report.observed
            );
        }
        Ok(report)
    })();
    reports.push(match tampered {
        Err(setup) => ControlReport {
            name: "tampered-binary".to_string(),
            expected: "wrong binary".to_string(),
            observed: setup,
            pass: false,
            setup_ok: false,
        },
        Ok(report) => report,
    });

    let all_pass = write_controls_receipt(&paths.out, &subject, &harness, &reports)?;
    let passed = reports.iter().filter(|report| report.pass).count();
    println!(
        "first-hour-controls: {passed}/{} controls observed their refusal",
        reports.len()
    );
    if all_pass {
        Ok(())
    } else {
        Err(reports
            .iter()
            .filter(|report| !report.pass)
            .map(|report| format!("control `{}`: {}", report.name, report.observed))
            .collect::<Vec<_>>()
            .join("\n"))
    }
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
    fn control_refusal_matrix_separates_pass_from_setup() -> Result<(), String> {
        let ok: Result<JourneyEvidence, String> = Ok(JourneyEvidence {
            repo_rel: "r".to_string(),
            base_sha: "b".to_string(),
            head_sha: "h".to_string(),
            human_digest: "d".to_string(),
            human_normalized_digest: "dn".to_string(),
            human_bytes: 1,
            json_digest: "j".to_string(),
            json_normalized_digest: "jn".to_string(),
            json_bytes: 1,
            evidence: CheckEvidence {
                findings: 1,
                classifications: vec!["weakly_exposed".to_string()],
                summary_probes: 1,
            },
        });
        // An unexpected success is a control failure, never a pass.
        let surprise = expect_refusal("probe", "wrong binary", ok);
        assert!(!surprise.pass);
        assert!(surprise.setup_ok);
        assert!(surprise.observed.contains("unexpected success"));
        // The typed refusal passes.
        let refused: Result<JourneyEvidence, String> =
            Err("wrong binary: `x` digest does not match".to_string());
        let report = expect_refusal("probe", "wrong binary", refused);
        assert!(report.pass);
        assert!(report.setup_ok);
        // A different error fails without implicating setup.
        let other: Result<JourneyEvidence, String> = Err("disk on fire".to_string());
        let mismatch = expect_refusal("probe", "wrong binary", other);
        assert!(!mismatch.pass);
        assert!(mismatch.setup_ok);
        Ok(())
    }

    #[test]
    fn tampering_breaks_the_installed_digest_binding() -> Result<(), String> {
        let dir =
            std::env::temp_dir().join(format!("ripr-first-hour-tamper-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|error| format!("fixture dir: {error}"))?;
        let executable = dir.join("ripr");
        std::fs::write(&executable, b"installed-bytes-0123456789")
            .map_err(|error| format!("write executable: {error}"))?;
        let subject = InstalledSubject {
            crate_path: "fixture.crate".to_string(),
            crate_sha256: "0".repeat(64),
            prefix: dir.clone(),
            executable: executable.clone(),
            executable_sha256: sha256_hex(b"installed-bytes-0123456789"),
            executable_size: 26,
            version_output: "ripr 0.11.0".to_string(),
        };
        assert!(matches!(
            admit_installed_executable(&subject, &executable),
            Ok(())
        ));
        tamper_installed_executable(&subject)?;
        assert!(matches!(
            admit_installed_executable(&subject, &executable),
            Err(error) if error.contains("wrong binary")
        ));
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn journey_run_propagates_child_failure_with_named_status() -> Result<(), String> {
        let mut harness = Harness {
            roots: Vec::new(),
            ledger: Vec::new(),
        };
        let outcome = journey_run(
            &mut harness,
            "probe-fail",
            "git",
            &[
                "-C".to_string(),
                "/nonexistent-ripr-first-hour-dir".to_string(),
                "--version".to_string(),
            ],
            &[],
        );
        let error = refusal_of(outcome)?;
        assert!(!error.is_empty());
        // The ledger records the failure verbatim; a failing child is never
        // recorded as ok.
        assert_eq!(harness.ledger.len(), 1);
        assert_ne!(harness.ledger[0].status, "ok");
        Ok(())
    }

    #[test]
    fn controls_entry_shares_the_fail_closed_guards() -> Result<(), String> {
        let dir = std::env::temp_dir().join(format!(
            "ripr-first-hour-controls-guard-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|error| format!("pre-existing dir: {error}"))?;
        assert!(matches!(
            first_hour_controls(&[
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
        assert!(!dir.join("prefix").exists());
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn output_normalization_hides_only_run_locations() -> Result<(), String> {
        let locations = ["/tmp/fh-d1/fixtures", "/tmp/fh-d1/fixtures/journey/x"];
        let normalized = normalize_output(
            "root: /tmp/fh-d1/fixtures/journey/x finding weakly_exposed",
            &locations,
        );
        assert_eq!(normalized, "root: <run-location> finding weakly_exposed");
        // Longer locations first is unnecessary: replacement is exact and
        // the evidence vocabulary survives byte-identical.
        assert_eq!(
            normalize_output("weakly_exposed: 1 probe", &locations),
            "weakly_exposed: 1 probe"
        );
        assert_eq!(normalize_output("unchanged", &[""]), "unchanged");
        Ok(())
    }

    #[test]
    fn init_generation_must_stay_advisory() -> Result<(), String> {
        toml_advisory_markers("mode = \"draft\"\ninclude_unchanged_tests = true\n")?;
        workflow_advisory_markers(
            "continue-on-error: true\nRIPR_UPLOAD_SARIF: \"true\"\nrun: cargo install ripr --locked\nrun: ripr pilot\n",
        )?;
        // A blocking default or an unpinned install reference refuses.
        assert!(matches!(
            toml_advisory_markers("mode = \"ready\"\ninclude_unchanged_tests = true\n"),
            Err(error) if error.contains("mode = \"draft\"")
        ));
        assert!(matches!(
            workflow_advisory_markers("run: cargo install ripr\nrun: ripr pilot\n"),
            Err(error) if error.contains("continue-on-error")
        ));
        assert!(matches!(
            workflow_advisory_markers(
                "continue-on-error: true\nRIPR_UPLOAD_SARIF: \"true\"\nrun: ripr pilot\n"
            ),
            Err(error) if error.contains("cargo install ripr --locked")
        ));
        Ok(())
    }

    #[test]
    fn repair_seam_resolution_requires_exactly_one_packet() -> Result<(), String> {
        let one = json!({
            "packets_total": 1,
            "packets": [{"seam_id": "f5d59ba47104d31d"}],
        });
        assert_eq!(repair_seam_id(&one.to_string())?, "f5d59ba47104d31d");
        for (name, doc) in [
            ("zero", json!({"packets_total": 0, "packets": []})),
            (
                "two",
                json!({"packets_total": 2, "packets": [{"seam_id": "a"}, {"seam_id": "b"}]}),
            ),
            (
                "inconsistent-count",
                json!({"packets_total": 1, "packets": [{"seam_id": "a"}, {"seam_id": "b"}]}),
            ),
            ("missing", json!({"packets_total": 1, "packets": [{}]})),
        ] {
            assert!(
                repair_seam_id(&doc.to_string()).is_err(),
                "packet shape `{name}` must refuse"
            );
        }
        Ok(())
    }

    #[test]
    fn cargo_counts_separate_selected_from_executed() -> Result<(), String> {
        let output = "running 2 tests\ntest a ... ok\ntest b ... ok\n\ntest result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n";
        assert_eq!(cargo_test_counts(output)?, (2, 2, 0));
        let failing = "running 1 test\ntest a ... FAILED\n\ntest result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n";
        assert_eq!(cargo_test_counts(failing)?, (1, 0, 1));
        let _ = refusal_of(cargo_test_counts("no test lines here").map(|_| ()))?;
        Ok(())
    }

    #[test]
    fn movement_gates_require_closed_improved_and_clean() -> Result<(), String> {
        let good = json!({
            "summary": {"improved": 1, "regressed": 0, "gap_movement": {"closed": 1}},
        });
        let movement = verify_movement(&good.to_string())?;
        assert_eq!(
            (movement.closed, movement.improved, movement.regressed),
            (1, 1, 0)
        );
        for (name, doc) in [
            (
                "unclosed",
                json!({"summary": {"improved": 1, "regressed": 0, "gap_movement": {"closed": 0}}}),
            ),
            (
                "unimproved",
                json!({"summary": {"improved": 0, "regressed": 0, "gap_movement": {"closed": 1}}}),
            ),
            (
                "regressed",
                json!({"summary": {"improved": 1, "regressed": 1, "gap_movement": {"closed": 1}}}),
            ),
        ] {
            assert!(
                verify_movement(&doc.to_string()).is_err(),
                "movement `{name}` must refuse"
            );
        }
        Ok(())
    }

    #[test]
    fn outcome_route_requires_weak_to_strong_movement() -> Result<(), String> {
        let good = json!({
            "moved": [{"seam_id": "s1", "before": "weakly_gripped", "after": "strongly_gripped"}],
            "before": {"weakly_gripped": 1},
            "after": {"strongly_gripped": 1},
        });
        assert_eq!(outcome_movement(&good.to_string(), "s1")?, (1, 1, 1));
        for (name, doc) in [
            (
                "unmoved",
                json!({"moved": [], "before": {"weakly_gripped": 1}, "after": {"strongly_gripped": 1}}),
            ),
            (
                "no-weakness",
                json!({"moved": [{"seam_id": "s1", "before": "weakly_gripped", "after": "strongly_gripped"}], "before": {"weakly_gripped": 0}, "after": {"strongly_gripped": 1}}),
            ),
            (
                "no-strength",
                json!({"moved": [{"seam_id": "s1", "before": "weakly_gripped", "after": "strongly_gripped"}], "before": {"weakly_gripped": 1}, "after": {"strongly_gripped": 0}}),
            ),
            (
                "wrong-seam",
                json!({"moved": [{"seam_id": "other", "before": "weakly_gripped", "after": "strongly_gripped"}], "before": {"weakly_gripped": 1}, "after": {"strongly_gripped": 1}}),
            ),
            (
                "row-not-weak-to-strong",
                json!({"moved": [{"seam_id": "s1", "before": "strongly_gripped", "after": "strongly_gripped"}], "before": {"weakly_gripped": 1}, "after": {"strongly_gripped": 1}}),
            ),
        ] {
            assert!(
                outcome_movement(&doc.to_string(), "s1").is_err(),
                "outcome `{name}` must refuse"
            );
        }
        Ok(())
    }

    #[test]
    fn lsp_file_uri_percent_encodes_hostile_paths() -> Result<(), String> {
        assert_eq!(
            lsp_file_uri(Path::new("/tmp/ripr fixtures/grüße/repo")),
            "file:///tmp/ripr%20fixtures/gr%C3%BC%C3%9Fe/repo"
        );
        assert_eq!(
            lsp_file_uri(Path::new("/tmp/a#b?.rs")),
            "file:///tmp/a%23b%3F.rs"
        );
        Ok(())
    }

    #[test]
    fn lsp_framing_round_trips_strict_frames() -> Result<(), String> {
        let first = json!({"jsonrpc": "2.0", "id": 1, "result": null});
        let second = json!({"jsonrpc": "2.0", "method": "exit"});
        let mut buffer = encode_lsp_message(&first);
        buffer.extend_from_slice(&encode_lsp_message(&second));
        let (frames, rest) = parse_lsp_frames(&buffer)?;
        assert_eq!(frames, vec![first.clone(), second]);
        assert!(rest.is_empty());
        // A trailing partial frame stays remainder, never an error here:
        // the reader refuses it only when stdout closes mid-frame.
        let mut partial = encode_lsp_message(&first);
        partial.truncate(partial.len() - 4);
        let (frames, rest) = parse_lsp_frames(&partial)?;
        assert!(frames.is_empty());
        assert!(!rest.is_empty());
        // Headerless bytes stay remainder mid-stream (indistinguishable
        // from a partial frame); the reader refuses them only when stdout
        // closes with bytes still unframed.
        let (frames, rest) = parse_lsp_frames(b"{\"jsonrpc\":\"2.0\"}".as_slice())?;
        assert!(frames.is_empty());
        assert!(!rest.is_empty());
        for (name, bytes) in [
            (
                "no-length",
                b"Content-Type: application/json\r\n\r\n{}".as_slice(),
            ),
            ("bad-length", b"Content-Length: many\r\n\r\n{}".as_slice()),
            ("colonless", b"Content-Length\r\n\r\n{}".as_slice()),
            ("non-json", b"Content-Length: 4\r\n\r\nnope".as_slice()),
        ] {
            assert!(
                parse_lsp_frames(bytes).is_err(),
                "framing `{name}` must refuse"
            );
        }
        Ok(())
    }

    #[test]
    fn lsp_initialize_requires_hover_and_status_command() -> Result<(), String> {
        let good = json!({
            "result": {
                "capabilities": {
                    "hoverProvider": true,
                    "executeCommandProvider": {"commands": ["ripr.refresh", "ripr.collectWorkspaceStatus"]},
                },
            },
        });
        assert_eq!(
            lsp_initialize_capabilities(&good)?,
            vec![
                "ripr.refresh".to_string(),
                "ripr.collectWorkspaceStatus".to_string()
            ]
        );
        for (name, doc) in [
            (
                "no-hover",
                json!({"result": {"capabilities": {"executeCommandProvider": {"commands": ["ripr.collectWorkspaceStatus"]}}}}),
            ),
            (
                "no-commands",
                json!({"result": {"capabilities": {"hoverProvider": true}}}),
            ),
            (
                "no-status-command",
                json!({"result": {"capabilities": {"hoverProvider": true, "executeCommandProvider": {"commands": ["ripr.refresh"]}}}}),
            ),
        ] {
            assert!(
                lsp_initialize_capabilities(&doc).is_err(),
                "capabilities `{name}` must refuse"
            );
        }
        Ok(())
    }

    #[test]
    fn lsp_status_binds_the_saved_checkout() -> Result<(), String> {
        let good = json!({
            "result": {"analysis_status": {"root_state": "selected_single_root", "effective_root": "/tmp/root"}},
        });
        assert_eq!(
            lsp_workspace_status(&good, "/tmp/root")?,
            "selected_single_root"
        );
        for (name, doc, root) in [
            (
                "ambiguous",
                json!({"result": {"analysis_status": {"root_state": "workspace_ambiguous", "effective_root": "/tmp/root"}}}),
                "/tmp/root",
            ),
            (
                "divergent-root",
                json!({"result": {"analysis_status": {"root_state": "selected_single_root", "effective_root": "/tmp/other"}}}),
                "/tmp/root",
            ),
            ("no-status", json!({"result": {}}), "/tmp/root"),
        ] {
            assert!(
                lsp_workspace_status(&doc, root).is_err(),
                "status `{name}` must refuse"
            );
        }
        Ok(())
    }

    fn lsp_test_deadline() -> std::time::Instant {
        std::time::Instant::now() + std::time::Duration::from_secs(5)
    }

    #[test]
    fn lsp_response_wait_pairs_ids_and_surfaces_errors() -> Result<(), String> {
        let (sender, receiver) = std::sync::mpsc::channel::<Result<Option<Value>, String>>();
        sender
            .send(Ok(Some(
                json!({"jsonrpc": "2.0", "method": "ripr/analysisStatus"}),
            )))
            .map_err(|error| format!("stage notification: {error}"))?;
        sender
            .send(Ok(Some(
                json!({"jsonrpc": "2.0", "id": 2, "result": {"ok": true}}),
            )))
            .map_err(|error| format!("stage response: {error}"))?;
        let mut notifications = Vec::new();
        let response = await_lsp_response(&receiver, 2, lsp_test_deadline(), &mut notifications)?;
        assert_eq!(response["result"], json!({"ok": true}));
        assert_eq!(notifications.len(), 1);
        // An error response never passes as a verdict.
        sender
            .send(Ok(Some(
                json!({"jsonrpc": "2.0", "id": 3, "error": {"code": -32601}}),
            )))
            .map_err(|error| format!("stage error: {error}"))?;
        let _ = refusal_of(
            await_lsp_response(&receiver, 3, lsp_test_deadline(), &mut Vec::new()).map(|_| ()),
        )?;
        // A foreign response id refuses at once instead of queuing as a
        // notification for a later verdict to absorb.
        sender
            .send(Ok(Some(json!({"jsonrpc": "2.0", "id": 99, "result": {}}))))
            .map_err(|error| format!("stage foreign id: {error}"))?;
        sender
            .send(Ok(Some(
                json!({"jsonrpc": "2.0", "id": 4, "result": {"ok": true}}),
            )))
            .map_err(|error| format!("stage late response: {error}"))?;
        let _ = refusal_of(
            await_lsp_response(&receiver, 4, lsp_test_deadline(), &mut Vec::new()).map(|_| ()),
        )?;
        // An expired deadline refuses without waiting, even with a queued
        // notification a fresh timeout would have consumed first.
        sender
            .send(Ok(Some(
                json!({"jsonrpc": "2.0", "method": "ripr/analysisStatus"}),
            )))
            .map_err(|error| format!("stage chatter: {error}"))?;
        let past = std::time::Instant::now() - std::time::Duration::from_secs(1);
        let _ = refusal_of(await_lsp_response(&receiver, 5, past, &mut Vec::new()).map(|_| ()))?;
        // A closed stdout without the id refuses instead of hanging.
        drop(sender);
        let _ = refusal_of(
            await_lsp_response(&receiver, 9, lsp_test_deadline(), &mut Vec::new()).map(|_| ()),
        )?;
        Ok(())
    }

    #[test]
    fn lsp_analysis_gate_requires_committed_saved_content() -> Result<(), String> {
        let ready = json!({
            "result": {
                "run_status": "full",
                "open_documents": [{
                    "uri": "file:///tmp/root/src/lib.rs",
                    "state": "clean",
                    "last_saved_content_identity": "sha256:abc",
                    "analyzed_saved_content_identity": "sha256:abc",
                }],
            },
        });
        assert!(matches!(
            lsp_saved_workspace_analysis(&ready, "file:///tmp/root/src/lib.rs")?,
            LspAnalysisPoll::Ready(digest) if digest == "sha256:abc"
        ));
        // Still in flight: stale snapshot or unanalyzed save polls on.
        for (name, doc) in [
            (
                "stale-run",
                json!({"result": {"run_status": "stale", "open_documents": []}}),
            ),
            (
                "no-snapshot",
                json!({"result": {"run_status": "no_snapshot", "open_documents": []}}),
            ),
            (
                "unanalyzed-save",
                json!({"result": {"run_status": "full", "open_documents": [{
                    "uri": "file:///tmp/root/src/lib.rs",
                    "state": "clean",
                    "last_saved_content_identity": "sha256:abc",
                    "analyzed_saved_content_identity": serde_json::Value::Null,
                }]}}),
            ),
            (
                "divergent-digest",
                json!({"result": {"run_status": "full", "open_documents": [{
                    "uri": "file:///tmp/root/src/lib.rs",
                    "state": "clean",
                    "last_saved_content_identity": "sha256:new",
                    "analyzed_saved_content_identity": "sha256:old",
                }]}}),
            ),
        ] {
            assert!(
                matches!(
                    lsp_saved_workspace_analysis(&doc, "file:///tmp/root/src/lib.rs")?,
                    LspAnalysisPoll::Pending
                ),
                "analysis `{name}` must poll, not decide"
            );
        }
        // Hard refusals: quarantined, missing, or malformed.
        for (name, doc) in [
            (
                "quarantined",
                json!({"result": {"run_status": "full", "open_documents": [{
                    "uri": "file:///tmp/root/src/lib.rs",
                    "state": "quarantined",
                    "last_saved_content_identity": "sha256:abc",
                    "analyzed_saved_content_identity": "sha256:abc",
                }]}}),
            ),
            (
                "missing-doc",
                json!({"result": {"run_status": "full", "open_documents": []}}),
            ),
            ("no-result", json!({})),
        ] {
            let _ = refusal_of(
                lsp_saved_workspace_analysis(&doc, "file:///tmp/root/src/lib.rs").map(|_| ()),
            )
            .map_err(|error| format!("analysis `{name}` must refuse: {error}"))?;
        }
        Ok(())
    }

    #[test]
    fn toolchain_roots_prefer_configured_ambient_values() -> Result<(), String> {
        // The ambient lookup stays outside the helper so this test never
        // mutates process environment.
        assert_eq!(
            prefer_ambient_toolchain_root(
                Some("/usr/local/custom-toolchain".to_string()),
                "/home/tester/.rustup",
            ),
            "/usr/local/custom-toolchain"
        );
        assert_eq!(
            prefer_ambient_toolchain_root(None, "/home/tester/.rustup"),
            "/home/tester/.rustup"
        );
        Ok(())
    }

    #[test]
    fn receipt_route_requires_the_repaired_improved_seam() -> Result<(), String> {
        let good = json!({"seam": {"seam_id": "s1", "change": "improved"}});
        assert_eq!(receipt_change(&good.to_string(), "s1")?, "improved");
        let wrong_seam = json!({"seam": {"seam_id": "s2", "change": "improved"}});
        assert!(matches!(
            receipt_change(&wrong_seam.to_string(), "s1"),
            Err(error) if error.contains("not the repaired")
        ));
        let unchanged = json!({"seam": {"seam_id": "s1", "change": "unchanged"}});
        assert!(matches!(
            receipt_change(&unchanged.to_string(), "s1"),
            Err(error) if error.contains("not `improved`")
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
        let _ = refusal_of(parse_args("first-hour", USAGE, &[]))?;
        let _ = refusal_of(parse_args(
            "first-hour",
            USAGE,
            &["--crate".to_string(), "a.crate".to_string()],
        ))?;
        let Ok(parsed) = parse_args(
            "first-hour",
            USAGE,
            &[
                "--crate".to_string(),
                "a.crate".to_string(),
                "--prefix".to_string(),
                "p".to_string(),
                "--out".to_string(),
                "o".to_string(),
                "--fixture-root".to_string(),
                "f".to_string(),
            ],
        ) else {
            return Err("complete arg surface must parse".to_string());
        };
        assert_eq!(parsed.prefix, "p");
        // The controls entry parses the same surface under its own name.
        let Ok(controls) = parse_args(
            "first-hour-controls",
            CONTROLS_USAGE,
            &[
                "--crate".to_string(),
                "a.crate".to_string(),
                "--prefix".to_string(),
                "p".to_string(),
                "--out".to_string(),
                "o".to_string(),
                "--fixture-root".to_string(),
                "f".to_string(),
            ],
        ) else {
            return Err("controls arg surface must parse".to_string());
        };
        assert_eq!(controls.fixture_root, "f");
        Ok(())
    }
}
