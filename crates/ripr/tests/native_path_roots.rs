//! Real-binary journeys under workspace roots that Windows handles differently
//! from Linux (#3922): a root with spaces and an apostrophe, a root with
//! non-ASCII and non-BMP characters, and a root longer than Windows `MAX_PATH`.
//!
//! Each journey runs the built `ripr` binary against a fixture repository with
//! its own git history, so the process path (ripr spawning `git` with the root
//! as its working directory), the diff reader, and the per-file fact cache
//! under `<root>/target/ripr/cache` all see the unusual root. The PowerShell
//! journey launches the binary from Windows PowerShell and PowerShell 7 with the
//! root passed as a PowerShell-quoted argument.
//!
//! The tests run on every platform. Only the Windows Advisory lane gives them
//! their native meaning, which is why they are listed as release-seam controls
//! in `xtask/src/windows_advisory.rs`. On Windows the long root is a 0.11
//! non-claim (#4350): its control asserts ripr's actionable MAX_PATH message
//! instead of a completed journey.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "common/mod.rs"]
mod common;

use common::fixture_git::fixture_git_ok;

static NEXT_BASE: AtomicU64 = AtomicU64::new(0);

/// Windows `MAX_PATH`. The long-root fixture must exceed it, or it proves
/// nothing about long paths.
const WINDOWS_MAX_PATH: usize = 260;

/// A temp base that owns one fixture tree and removes it on drop.
struct FixtureBase {
    path: PathBuf,
}

impl FixtureBase {
    fn new(label: &str) -> Result<Self, String> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let sequence = NEXT_BASE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "ripr-native-root-{label}-{}-{nonce}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&path)
            .map_err(|err| format!("fixture setup: create {}: {err}", path.display()))?;
        Ok(Self { path })
    }
}

impl Drop for FixtureBase {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// The root shapes under test, relative to the fixture base.
fn spaced_relative_root() -> PathBuf {
    Path::new("with spaces and it's").join("repo")
}

fn unicode_relative_root() -> PathBuf {
    // é and 日本語 are BMP characters; 😀 is a UTF-16 surrogate pair on Windows.
    Path::new("ünïcødé 日本語 😀").join("repo")
}

fn long_relative_root() -> PathBuf {
    let mut root = PathBuf::new();
    for index in 0..6 {
        root.push(format!(
            "long-path-segment-0123456789-abcdefghijklmnopqrstuvwxyz-{index}"
        ));
    }
    root.join("repo")
}

/// Build the fixture repository at `base/relative`: a base commit with a
/// passing test that calls `gate_state`, then a committed production change.
///
/// Git commits the fixture at a short staging path before it moves to the
/// requested root. Only ripr's own spawns see the long root as a working
/// directory. `core.longpaths` is written into the fixture's config so ripr's
/// git children inherit it, as a user with a long checkout on Windows must
/// configure.
fn build_fixture(base: &Path, relative: &Path) -> Result<PathBuf, String> {
    let root = base.join(relative);
    let staging = base.join("fixture-staging");
    fs::create_dir_all(staging.join("src")).map_err(|err| {
        format!(
            "fixture setup: create src under {}: {err}",
            staging.display()
        )
    })?;
    fs::create_dir_all(staging.join("tests"))
        .map_err(|err| format!("fixture setup: create tests: {err}"))?;
    let write = |relative_file: &str, text: &str| {
        fs::write(staging.join(relative_file), text)
            .map_err(|err| format!("fixture setup: write {relative_file}: {err}"))
    };
    write(
        "Cargo.toml",
        "[package]\nname = \"native_root\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )?;
    write(
        "src/lib.rs",
        "pub fn gate_state(flag: bool) -> bool { flag }\n",
    )?;
    write(
        "tests/end_to_end.rs",
        "#[test]\nfn gate_state_passes_the_flag() {\n    assert!(native_root::gate_state(true));\n}\n",
    )?;
    let git = |args: &[&str]| -> Result<(), String> {
        let mut full = vec!["-c", "core.longpaths=true", "-C", "fixture-staging"];
        full.extend_from_slice(args);
        fixture_git_ok(base, &full).map_err(|err| format!("fixture setup: {err}"))
    };
    git(&["init", "-q"])?;
    git(&["config", "core.longpaths", "true"])?;
    git(&["config", "core.autocrlf", "false"])?;
    git(&["config", "user.email", "ripr@example.invalid"])?;
    git(&["config", "user.name", "RIPR Test"])?;
    git(&["add", "Cargo.toml", "src/lib.rs", "tests/end_to_end.rs"])?;
    git(&["commit", "-q", "-m", "base"])?;
    write(
        "src/lib.rs",
        "pub fn gate_state(flag: bool) -> bool {\n    if flag { true } else { false }\n}\n",
    )?;
    git(&["add", "src/lib.rs"])?;
    git(&["commit", "-q", "-m", "change production"])?;
    let parent = root
        .parent()
        .ok_or_else(|| format!("fixture setup: root has no parent: {}", root.display()))?;
    fs::create_dir_all(parent)
        .map_err(|err| format!("fixture setup: create {}: {err}", parent.display()))?;
    fs::rename(&staging, &root).map_err(|err| {
        format!(
            "fixture setup: move {} to {}: {err}",
            staging.display(),
            root.display()
        )
    })?;
    Ok(root)
}

fn run_ripr(args: &[&std::ffi::OsStr]) -> Result<Output, String> {
    Command::new(env!("CARGO_BIN_EXE_ripr"))
        .args(args)
        .env_remove("RIPR_CACHE_DIR")
        .output()
        .map_err(|err| format!("spawn ripr {args:?}: {err}"))
}

fn describe(output: &Output) -> String {
    format!(
        "status: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn parse_json(bytes: &[u8], what: &str) -> Result<serde_json::Value, String> {
    serde_json::from_slice(bytes).map_err(|err| {
        format!(
            "{what} is not JSON ({err}):\n{}",
            String::from_utf8_lossy(bytes)
        )
    })
}

fn same_file(reported: &str, expected: &Path) -> bool {
    match (fs::canonicalize(reported), fs::canonicalize(expected)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

/// `ripr check --base HEAD~1 --json` under `root`: the git diff runs with the
/// root as its working directory, and every finding names the changed file.
fn assert_check_reports_the_changed_file(root: &Path) -> Result<(), String> {
    let output = run_ripr(&[
        "check".as_ref(),
        "--root".as_ref(),
        root.as_os_str(),
        "--base".as_ref(),
        "HEAD~1".as_ref(),
        "--json".as_ref(),
    ])?;
    if !output.status.success() {
        return Err(format!("ripr check failed\n{}", describe(&output)));
    }
    let report = parse_json(&output.stdout, "ripr check stdout")?;
    if report.pointer("/analysis_outcome/analysis_complete") != Some(&serde_json::json!(true)) {
        return Err(format!("ripr check did not complete: {report}"));
    }
    if report.pointer("/summary/changed_rust_files") != Some(&serde_json::json!(1)) {
        return Err(format!("ripr check must see one changed file: {report}"));
    }
    let findings = report
        .get("findings")
        .and_then(serde_json::Value::as_array)
        .filter(|findings| !findings.is_empty())
        .ok_or_else(|| format!("ripr check must report findings: {report}"))?;
    let expected = root.join("src").join("lib.rs");
    for finding in findings {
        let file = finding
            .pointer("/probe/file")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("finding without probe.file: {finding}"))?;
        if file.starts_with(r"\\?\") {
            return Err(format!("finding exposes a verbatim Windows path: {file}"));
        }
        if !same_file(file, &expected) {
            return Err(format!(
                "finding file {file:?} is not {}",
                expected.display()
            ));
        }
    }
    Ok(())
}

fn rerun_report(root: &Path, out: &Path) -> Result<serde_json::Value, String> {
    let output = run_ripr(&[
        "rerun".as_ref(),
        "--changed-test".as_ref(),
        "tests/end_to_end.rs".as_ref(),
        "--root".as_ref(),
        root.as_os_str(),
        "--json".as_ref(),
        "--out".as_ref(),
        out.as_os_str(),
    ])?;
    if !output.status.success() {
        return Err(format!("ripr rerun failed\n{}", describe(&output)));
    }
    let bytes =
        fs::read(out).map_err(|err| format!("read rerun report {}: {err}", out.display()))?;
    parse_json(&bytes, "ripr rerun report")
}

fn cache_field(report: &serde_json::Value, key: &str) -> Option<u64> {
    report
        .pointer(&format!("/cache/{key}"))
        .and_then(serde_json::Value::as_u64)
}

/// Count the cached fact files under `dir`, whatever schema-version
/// subdirectory the cache layer uses.
fn cached_fact_files(dir: &Path) -> Result<u64, String> {
    let mut count = 0;
    for entry in fs::read_dir(dir).map_err(|err| format!("read {}: {err}", dir.display()))? {
        let path = entry
            .map_err(|err| format!("read entry under {}: {err}", dir.display()))?
            .path();
        if path.is_dir() {
            count += cached_fact_files(&path)?;
        } else if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            count += 1;
        }
    }
    Ok(count)
}

/// Per-file facts written by `ripr check` under `<root>/target/ripr/cache` are
/// read back by `ripr rerun`: one hit per cached fact file, no miss and no
/// store error, and the seam is named by its root-relative path. A write or
/// read that failed on the unusual root would surface as a miss or a hit
/// count short of the cached files.
fn assert_file_fact_cache_round_trips(root: &Path) -> Result<(), String> {
    let cache_layer = root
        .join("target")
        .join("ripr")
        .join("cache")
        .join("repo-file-facts");
    let cached = cached_fact_files(&cache_layer)
        .map_err(|err| format!("check left no file-fact cache: {err}"))?;
    if cached == 0 {
        return Err(format!(
            "check left no cached fact file under {}",
            cache_layer.display()
        ));
    }
    let report = rerun_report(root, &root.join("target").join("rerun.json"))?;
    if report.pointer("/cache/reuse_state") != Some(&serde_json::json!("reused_file_facts"))
        || cache_field(&report, "misses") != Some(0)
        || cache_field(&report, "store_errors") != Some(0)
        || cache_field(&report, "hits") != Some(cached)
    {
        return Err(format!(
            "rerun must reuse all {cached} file facts check cached: {}",
            report["cache"]
        ));
    }
    assert_rerun_names_the_seam(&report, "rerun")
}

fn assert_rerun_names_the_seam(report: &serde_json::Value, what: &str) -> Result<(), String> {
    let seams = report
        .get("seams")
        .and_then(serde_json::Value::as_array)
        .filter(|seams| !seams.is_empty())
        .ok_or_else(|| format!("{what} must select the gate_state seam: {report}"))?;
    if seams
        .iter()
        .any(|seam| seam.get("file") != Some(&serde_json::json!("src/lib.rs")))
    {
        return Err(format!("{what} seams must name src/lib.rs: {report}"));
    }
    Ok(())
}

fn assert_root_journeys(label: &str, relative: &Path) -> Result<(), String> {
    let base = FixtureBase::new(label)?;
    let root = build_fixture(&base.path, relative)?;
    assert_check_reports_the_changed_file(&root)?;
    assert_file_fact_cache_round_trips(&root)
}

#[test]
fn spaced_root_check_and_file_fact_cache_round_trip() -> Result<(), String> {
    assert_root_journeys("spaced", &spaced_relative_root())
}

#[test]
fn unicode_root_check_and_file_fact_cache_round_trip() -> Result<(), String> {
    assert_root_journeys("unicode", &unicode_relative_root())
}

/// The text a Windows `ripr check` must lead with under a root past
/// `MAX_PATH` (#4350): the limit and the remedy, not `os error 267`.
#[cfg(windows)]
const WINDOWS_LONG_ROOT_LIMITATION: [&str; 2] = [
    "failed to run git diff: failed to run git: the workspace root is ",
    " characters long, and Windows cannot start git in a directory longer than 258 \
     characters (MAX_PATH). Clone or move the repository to a shorter path and rerun ripr",
];

/// Windows cannot start git under a root past `MAX_PATH`, and 0.11 does not
/// claim it can (#4350, accepted as a 0.11 non-claim in #2766). There the
/// control pins the actionable limitation; everywhere else the root must
/// work like any other.
#[cfg(windows)]
fn assert_long_root_outcome(root: &Path) -> Result<(), String> {
    let output = run_ripr(&[
        "check".as_ref(),
        "--root".as_ref(),
        root.as_os_str(),
        "--base".as_ref(),
        "HEAD~1".as_ref(),
        "--json".as_ref(),
    ])?;
    if output.status.success() {
        return Err(format!(
            "ripr check succeeded under a {}-byte root; #4350's MAX_PATH non-claim no longer \
             holds, so this control must assert the full journey again\n{}",
            root.as_os_str().len(),
            describe(&output)
        ));
    }
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let [lead, limit] = WINDOWS_LONG_ROOT_LIMITATION;
    if !(text.contains(lead) && text.contains(limit)) {
        return Err(format!(
            "ripr check must name the MAX_PATH limit and remedy\n{}",
            describe(&output)
        ));
    }
    Ok(())
}

#[cfg(not(windows))]
fn assert_long_root_outcome(root: &Path) -> Result<(), String> {
    assert_check_reports_the_changed_file(root)?;
    assert_file_fact_cache_round_trips(root)
}

#[test]
fn long_root_check_round_trips_or_names_the_windows_path_limit() -> Result<(), String> {
    let base = FixtureBase::new("long")?;
    let root = build_fixture(&base.path, &long_relative_root())?;
    let length = root.as_os_str().len();
    if length <= WINDOWS_MAX_PATH {
        return Err(format!(
            "fixture setup: long root is only {length} bytes, not beyond MAX_PATH: {}",
            root.display()
        ));
    }
    assert_long_root_outcome(&root)
}

/// Quote `value` as one PowerShell single-quoted string literal.
#[cfg(windows)]
fn powershell_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// Launch ripr from Windows PowerShell and PowerShell 7 with a root holding
/// spaces, an apostrophe and non-BMP characters. The report goes through
/// ripr's own `--out` file, because PowerShell re-encodes a native command's
/// stdout through the console code page. A missing shell fails closed.
#[cfg(windows)]
#[test]
fn powershell_launch_passes_a_quoted_unicode_root_to_ripr() -> Result<(), String> {
    let base = FixtureBase::new("powershell")?;
    let root = build_fixture(&base.path, &Path::new("it's ünïcødé 😀 root").join("repo"))?;
    let binary = env!("CARGO_BIN_EXE_ripr");
    let root_text = root
        .to_str()
        .ok_or_else(|| format!("fixture setup: root is not UTF-8: {}", root.display()))?;
    for shell in ["powershell", "pwsh"] {
        let out = root.join("target").join(format!("{shell}-rerun.json"));
        let out_text = out
            .to_str()
            .ok_or_else(|| format!("fixture setup: out is not UTF-8: {}", out.display()))?;
        let script = format!(
            "& {} rerun --changed-test tests/end_to_end.rs --root {} --json --out {}; exit $LASTEXITCODE",
            powershell_literal(binary),
            powershell_literal(root_text),
            powershell_literal(out_text),
        );
        let output = Command::new(shell)
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .env_remove("RIPR_CACHE_DIR")
            .output()
            .map_err(|err| format!("{shell} is required on the Windows lane: {err}"))?;
        if !output.status.success() {
            return Err(format!("{shell} launch failed\n{}", describe(&output)));
        }
        let bytes = fs::read(&out)
            .map_err(|err| format!("{shell}: read rerun report {}: {err}", out.display()))?;
        let report = parse_json(&bytes, "PowerShell rerun report")?;
        assert_rerun_names_the_seam(&report, shell)?;
    }
    Ok(())
}
