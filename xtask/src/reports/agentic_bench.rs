//! `cargo xtask agentic-bench` — shared runner skeleton for the
//! `benchmarks/agentic/*` benches.
//!
//! The skeleton discovers one manifest per bench, validates it strictly,
//! verifies every `sha256:`-bound fixture file (the same rendering as
//! `ripr::agent::provenance::sha256_file`), and writes a versioned
//! `ripr-agentic-bench-v1` JSON/Markdown receipt pair. It claims fixture and
//! provenance readiness only; each bench's behavioral oracle runs under its
//! own `cargo test` command, named in the receipt.
//!
//! The command fails closed (#6587): when the rollup is not `pass`, the
//! receipt is still written, but the process exits nonzero so the declared
//! `covered_by` control in `policy/non-rust-allowlist.toml` can actually gate
//! a pipeline over corrupt or tampered fixtures. Each receipt also carries a
//! run-identity overlay (#6595) mirroring `bench-agent-surfaces`, so a
//! receipt is traceable to the source revision, build, and time that
//! produced the verdict.

use crate::run::run_output;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const SCHEMA_VERSION: &str = "ripr-agentic-bench-v1";
const MANIFEST_SCHEMA: &str = "ripr-agentic-bench-manifest-v1";
const DEFAULT_FIXTURES_DIR: &str = "benchmarks/agentic";
const VERIFIER: &str = "cargo xtask agentic-bench";

pub(crate) fn agentic_bench(args: &[String]) -> Result<(), String> {
    agentic_bench_in(args, &crate::reports_dir())
}

/// The command body against an explicit report directory, so tests can run
/// the full verification hermetically and observe the receipt on disk even
/// when the run fails closed.
fn agentic_bench_in(args: &[String], report_dir: &Path) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{USAGE}");
        return Ok(());
    }
    let options = parse_options(args)?;
    let manifests = discover_manifests(&options.fixtures_dir, options.bench.as_deref())?;
    let mut benches = Vec::new();
    for manifest_path in &manifests {
        benches.push(verify_bench(&options.fixtures_dir, manifest_path));
    }
    let report = build_report(&options, &benches);
    let json_text = serde_json::to_string_pretty(&report)
        .map_err(|err| format!("serialize agentic bench report: {err}"))?;
    crate::write_report_in(report_dir, "agentic-bench.json", &format!("{json_text}\n"))?;
    crate::write_report_in(report_dir, "agentic-bench.md", &report_markdown(&report))?;
    let json_display = display_report_path(report_dir, "agentic-bench.json");
    println!("Wrote {json_display}");
    println!(
        "Wrote {}",
        display_report_path(report_dir, "agentic-bench.md")
    );
    finish_bench_run(&report, &benches, &json_display)
}

const USAGE: &str = "usage: cargo xtask agentic-bench [--bench <id>] [--fixtures <dir>]\n\nExit: 0 when every bench verifies ready; 1 when any bench fails verification (fixture_unverified, missing_manifest, invalid_manifest) or on usage errors (unknown bench, missing fixtures dir, malformed flag). On verification failure the receipt is still written to target/ripr/reports/agentic-bench.{json,md}; usage errors write none.";

fn display_report_path(report_dir: &Path, file_name: &str) -> String {
    absolute_display(&report_dir.join(file_name))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Options {
    bench: Option<String>,
    fixtures_dir: PathBuf,
}

fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut bench = None;
    let mut fixtures_dir = PathBuf::from(DEFAULT_FIXTURES_DIR);
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--bench" => {
                index += 1;
                bench = Some(required_arg(args, index, "--bench")?.to_string());
            }
            "--fixtures" => {
                index += 1;
                fixtures_dir = PathBuf::from(required_arg(args, index, "--fixtures")?);
            }
            other => {
                return Err(format!("unknown agentic-bench argument `{other}`; {USAGE}"));
            }
        }
        index += 1;
    }
    if let Some(id) = &bench {
        normalize_bench_id(id)?;
    }
    Ok(Options {
        bench,
        fixtures_dir,
    })
}

fn required_arg<'a>(args: &'a [String], index: usize, flag: &str) -> Result<&'a str, String> {
    let Some(value) = args.get(index) else {
        return Err(format!("missing value for {flag}; {USAGE}"));
    };
    if value.trim().is_empty() {
        return Err(format!("{flag} requires a non-empty value; {USAGE}"));
    }
    Ok(value)
}

fn normalize_bench_id(id: &str) -> Result<String, String> {
    let trimmed = id.trim();
    if trimmed.is_empty()
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed == "."
        || trimmed == ".."
    {
        return Err(format!("invalid --bench id `{id}`; {USAGE}"));
    }
    Ok(trimmed.to_string())
}

fn discover_manifests(fixtures_dir: &Path, bench: Option<&str>) -> Result<Vec<PathBuf>, String> {
    if !fixtures_dir.is_dir() {
        return Err(format!(
            "agentic fixtures dir {} does not exist",
            fixtures_dir.display()
        ));
    }
    if let Some(id) = bench {
        let manifest = fixtures_dir.join(id).join("manifest.json");
        // Only expected absence is a usage-level "unknown bench"; an
        // inspection failure (permissions, symlink loop) must not wear that
        // label (PR #6683 review).
        match fs::metadata(fixtures_dir.join(id)) {
            Ok(metadata) if metadata.is_dir() => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Err(format!(
                    "unknown bench `{id}`: {} does not exist",
                    manifest.display()
                ));
            }
            Err(err) => {
                return Err(format!("inspect bench directory `{id}`: {err}"));
            }
            Ok(_) => {
                return Err(format!(
                    "unknown bench `{id}`: {} does not exist",
                    manifest.display()
                ));
            }
        }
        // A bench directory without its manifest is a verification
        // outcome, not a discovery error: return it so a filtered run
        // reports `missing_manifest` with a receipt, exactly like the
        // unfiltered path, instead of leaving any prior receipt in place.
        return Ok(vec![manifest]);
    }
    let mut manifests = Vec::new();
    let mut entries: Vec<PathBuf> = fs::read_dir(fixtures_dir)
        .map_err(|err| format!("list {}: {err}", fixtures_dir.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("list {}: {err}", fixtures_dir.display()))?
        .into_iter()
        .map(|entry| entry.path())
        .collect();
    entries.sort();
    for entry in entries {
        if !entry.is_dir() {
            continue;
        }
        let manifest = entry.join("manifest.json");
        // Every bench directory is reported, even without a manifest: a
        // missing manifest is a `missing_manifest` outcome, never a silent
        // denominator narrowing.
        manifests.push(manifest);
    }
    if manifests.is_empty() {
        return Err(format!(
            "no bench manifests under {}",
            fixtures_dir.display()
        ));
    }
    Ok(manifests)
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Scope {
    Exact,
    Subtree,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rule {
    path: String,
    scope: Scope,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureRef {
    path: String,
    sha256: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Bounds {
    max_paths: usize,
    max_file_bytes: u64,
    max_total_bytes: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: String,
    bench: String,
    bench_index: String,
    title: String,
    stimulus: String,
    oracle_states: Vec<String>,
    oracle_command: String,
    selected_target: Rule,
    allowed_surface: Vec<Rule>,
    #[serde(default)]
    forbidden_paths: Vec<Rule>,
    fixtures: Vec<FixtureRef>,
    bounds: Bounds,
}

fn normalize_repo_path(raw: &str) -> Result<String, String> {
    if raw.is_empty() {
        return Err("path must not be empty".to_string());
    }
    if raw.starts_with('/') || raw.contains('\\') {
        return Err(format!("path escapes the bench root: {raw}"));
    }
    let mut parts = Vec::new();
    for component in raw.split('/') {
        if component.is_empty() || component == "." {
            return Err(format!("path has an empty component: {raw}"));
        }
        if component == ".." {
            return Err(format!("path escapes the bench root: {raw}"));
        }
        parts.push(component);
    }
    Ok(parts.join("/"))
}

fn rule_matches(rule: &Rule, candidate: &str) -> bool {
    match rule.scope {
        Scope::Exact => candidate == rule.path,
        Scope::Subtree => {
            candidate == rule.path
                || candidate
                    .strip_prefix(rule.path.as_str())
                    .is_some_and(|tail| tail.starts_with('/'))
        }
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].chars().all(|char| char.is_ascii_hexdigit())
}

fn sha256_bytes(bytes: &[u8]) -> String {
    prefixed_digest(&Sha256::digest(bytes))
}

/// Bare-hex rendering, uniform with `bench-agent-surfaces`' `binary_sha256`.
fn sha256_hex(bytes: &[u8]) -> String {
    let sum = Sha256::digest(bytes);
    let mut rendered = String::with_capacity(64);
    for byte in sum {
        rendered.push_str(&format!("{byte:02x}"));
    }
    rendered
}

fn prefixed_digest(digest: &[u8]) -> String {
    let mut rendered = String::from("sha256:");
    for byte in digest {
        rendered.push_str(&format!("{byte:02x}"));
    }
    rendered
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    Ok(sha256_bytes(&bytes))
}

/// Absolute, forward-slash rendering of a path for receipts: consumers string
/// match across platforms, so a path mixing forward and back separators (as a
/// Windows join of a forward-slash input root produces) and verbatim echoes of
/// a relative input are avoided (#6595).
fn absolute_display(path: &Path) -> String {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    display_normalized(absolute)
}

/// Symlink-resolved absolute rendering for the input root: a retargeted alias
/// must not silently change which corpus a retained receipt describes (PR
/// #6683 review). Falls back to the unresolved spelling when the path cannot
/// be canonicalized (for example a removed root between discovery and
/// reporting).
fn resolved_root_display(path: &Path) -> String {
    match fs::canonicalize(path) {
        Ok(canonical) => display_normalized(canonical),
        Err(_) => absolute_display(path),
    }
}

fn display_normalized(absolute: PathBuf) -> String {
    let text = absolute.to_string_lossy().to_string();
    let text = if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        text
    };
    text.replace('\\', "/")
}

#[derive(Clone, Debug)]
struct VerifiedFixture {
    path: String,
    sha256: String,
}

#[derive(Clone, Debug)]
struct BenchOutcome {
    bench: String,
    bench_index: String,
    title: String,
    manifest: String,
    manifest_sha256: Option<String>,
    status: &'static str,
    detail: Option<String>,
    fixtures_checked: usize,
    fixtures_verified: Vec<VerifiedFixture>,
    oracle_command: Option<String>,
}

/// Strict per-bench verification: an invalid manifest or an unverified
/// fixture fails that bench closed. Bench-specific checks plug in by bench
/// id; benches without one keep generic manifest plus digest verification.
fn verify_bench(fixtures_dir: &Path, manifest_path: &Path) -> BenchOutcome {
    let manifest_label = absolute_display(manifest_path);
    let bench_dir = manifest_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| fixtures_dir.to_path_buf());
    let dir_id = bench_dir
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    let outcome = |status: &'static str,
                   manifest: Option<&Manifest>,
                   detail: Option<String>,
                   checked: usize,
                   verified: Vec<VerifiedFixture>,
                   digest: Option<String>| {
        BenchOutcome {
            bench: manifest.map_or_else(|| dir_id.clone(), |loaded| loaded.bench.clone()),
            bench_index: manifest.map_or_else(String::new, |loaded| loaded.bench_index.clone()),
            title: manifest.map_or_else(String::new, |loaded| loaded.title.clone()),
            manifest: manifest_label.clone(),
            manifest_sha256: digest,
            status,
            detail,
            fixtures_checked: checked,
            fixtures_verified: verified,
            oracle_command: manifest.map(|loaded| loaded.oracle_command.clone()),
        }
    };
    if !manifest_path.is_file() {
        return outcome(
            "missing_manifest",
            None,
            Some("manifest.json is missing".to_string()),
            0,
            Vec::new(),
            None,
        );
    }
    let bytes = match fs::read(manifest_path) {
        Ok(bytes) => bytes,
        Err(err) => {
            return outcome(
                "invalid_manifest",
                None,
                Some(format!("read manifest: {err}")),
                0,
                Vec::new(),
                None,
            );
        }
    };
    let manifest_digest = sha256_bytes(&bytes);
    let mut manifest: Manifest = match serde_json::from_slice(&bytes) {
        Ok(manifest) => manifest,
        Err(err) => {
            return outcome(
                "invalid_manifest",
                None,
                Some(format!("parse manifest: {err}")),
                0,
                Vec::new(),
                Some(manifest_digest),
            );
        }
    };
    if let Err(detail) = validate_manifest(&mut manifest, &dir_id) {
        return outcome(
            "invalid_manifest",
            None,
            Some(detail),
            0,
            Vec::new(),
            Some(manifest_digest),
        );
    }
    if manifest.bench == "edit-cage"
        && let Err(detail) = validate_edit_cage_surface(&manifest)
    {
        return outcome(
            "invalid_manifest",
            Some(&manifest),
            Some(detail),
            0,
            Vec::new(),
            Some(manifest_digest),
        );
    }
    let mut checked = 0;
    let mut verified = Vec::new();
    for fixture in &manifest.fixtures {
        let path = bench_dir.join(&fixture.path);
        match sha256_file(&path) {
            Ok(observed) if observed == fixture.sha256 => {
                checked += 1;
                verified.push(VerifiedFixture {
                    path: fixture.path.clone(),
                    sha256: observed,
                });
            }
            Ok(observed) => {
                return outcome(
                    "fixture_unverified",
                    Some(&manifest),
                    Some(format!(
                        "fixture digest drift for {}: manifest {}, worktree {observed}",
                        fixture.path, fixture.sha256
                    )),
                    checked,
                    verified,
                    Some(manifest_digest),
                );
            }
            Err(err) => {
                return outcome(
                    "fixture_unverified",
                    Some(&manifest),
                    Some(format!("fixture unreadable {}: {err}", fixture.path)),
                    checked,
                    verified,
                    Some(manifest_digest),
                );
            }
        }
    }
    outcome(
        "ready",
        Some(&manifest),
        None,
        checked,
        verified,
        Some(manifest_digest),
    )
}

fn validate_manifest(manifest: &mut Manifest, dir_id: &str) -> Result<(), String> {
    if manifest.schema_version != MANIFEST_SCHEMA {
        return Err(format!(
            "manifest schema is {}, want {MANIFEST_SCHEMA}",
            manifest.schema_version
        ));
    }
    if manifest.bench != dir_id {
        return Err(format!(
            "manifest bench {} does not match directory {dir_id}",
            manifest.bench
        ));
    }
    if manifest.bench_index.trim().is_empty()
        || manifest.title.trim().is_empty()
        || manifest.stimulus.trim().is_empty()
        || manifest.oracle_command.trim().is_empty()
    {
        return Err("manifest identity/stimulus/oracle text must be non-empty".to_string());
    }
    if manifest.oracle_states.is_empty() {
        return Err("manifest must declare oracle states".to_string());
    }
    manifest.selected_target.path = normalize_repo_path(&manifest.selected_target.path)?;
    if manifest.allowed_surface.is_empty() {
        return Err("manifest allowed surface must be non-empty".to_string());
    }
    for rule in &mut manifest.allowed_surface {
        rule.path = normalize_repo_path(&rule.path)?;
    }
    for rule in &mut manifest.forbidden_paths {
        rule.path = normalize_repo_path(&rule.path)?;
    }
    if manifest.fixtures.is_empty() {
        return Err("manifest must bind at least one fixture".to_string());
    }
    let mut seen = std::collections::BTreeSet::new();
    for fixture in &mut manifest.fixtures {
        fixture.path = normalize_repo_path(&fixture.path)?;
        if !valid_digest(&fixture.sha256) {
            return Err(format!("fixture has a malformed digest: {}", fixture.path));
        }
        if !seen.insert(fixture.path.clone()) {
            return Err(format!("fixture listed twice: {}", fixture.path));
        }
    }
    if manifest.bounds.max_paths == 0
        || manifest.bounds.max_file_bytes == 0
        || manifest.bounds.max_total_bytes == 0
    {
        return Err("manifest bounds must be positive".to_string());
    }
    Ok(())
}

/// B6 bench-specific check: the selected target must sit inside the allowed
/// surface and outside every forbidden path. The production-routed suite
/// ships `benchmarks/agentic/edit-cage/manifest.json`.
fn validate_edit_cage_surface(manifest: &Manifest) -> Result<(), String> {
    let target = manifest.selected_target.path.as_str();
    if !manifest
        .allowed_surface
        .iter()
        .any(|rule| rule_matches(rule, target))
    {
        return Err("selected target is outside the allowed surface".to_string());
    }
    if manifest
        .forbidden_paths
        .iter()
        .any(|rule| rule_matches(rule, target))
    {
        return Err("selected target falls on a forbidden path".to_string());
    }
    Ok(())
}

fn build_report(options: &Options, benches: &[BenchOutcome]) -> Value {
    let ready = benches
        .iter()
        .filter(|bench| bench.status == "ready")
        .count();
    json!({
        "schema_version": SCHEMA_VERSION,
        "tool": "ripr",
        "report": "agentic-bench",
        "status": if !benches.is_empty() && ready == benches.len() { "pass" } else { "inconclusive" },
        "identity": identity_overlay(options),
        "fixtures_dir": options.fixtures_dir.to_string_lossy(),
        "bench_filter": options.bench,
        "ready_benches": ready,
        "bench_count": benches.len(),
        "benches": benches.iter().map(|bench| json!({
            "bench": bench.bench,
            "bench_index": bench.bench_index,
            "title": bench.title,
            "manifest": bench.manifest,
            "manifest_sha256": bench.manifest_sha256,
            "status": bench.status,
            "detail": bench.detail,
            "fixtures_checked": bench.fixtures_checked,
            "fixtures_verified": bench.fixtures_verified.iter().map(|fixture| json!({
                "path": fixture.path,
                "sha256": fixture.sha256,
            })).collect::<Vec<_>>(),
            "oracle_command": bench.oracle_command,
        })).collect::<Vec<_>>(),
        "claim_boundary": "Fixture and provenance readiness only: manifests validated and fixture digests verified. Behavioral verdicts come from each bench's oracle command, not this receipt."
    })
}

/// Run identity mirroring the `bench-agent-surfaces` precedent (#6595): a
/// receipt must be traceable to the source revision, verifier build, and
/// time that produced the verdict, and pin the resolved input root.
/// `source_sha` identifies the verifier's own source tree (the sibling's
/// convention); the verified inputs are pinned per bench by
/// `fixtures_verified` digests and `manifest_sha256`, with the input root
/// named by `fixtures_dir_resolved`. Each field degrades to
/// `"unavailable"` rather than failing the verification run; identity
/// collection must never mask a fixture verdict.
fn identity_overlay(options: &Options) -> Value {
    let binary = std::env::current_exe().ok();
    let binary_sha256 = binary
        .as_deref()
        .and_then(|path| fs::read(path).ok())
        .map(|bytes| sha256_hex(&bytes));
    json!({
        "verifier": VERIFIER,
        "source_sha": git_revision(),
        "binary_path": binary.as_deref().map(absolute_display).unwrap_or_else(|| "unavailable".to_string()),
        "binary_sha256": binary_sha256.unwrap_or_else(|| "unavailable".to_string()),
        "host_class": format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        "toolchain": run_output("rustc", &["--version"])
            .map(|output| output.trim().to_string())
            .unwrap_or_else(|_| "unavailable".to_string()),
        "timestamp_unix_ms": unix_stamp(),
        "fixtures_dir_resolved": resolved_root_display(&options.fixtures_dir),
    })
}

fn git_revision() -> String {
    run_output("git", &["rev-parse", "HEAD"])
        .map(|output| output.trim().to_string())
        .unwrap_or_else(|_| "unavailable".to_string())
}

fn unix_stamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

/// Fail closed (#6587): the written receipt is the structured record, but a
/// rollup that is not `pass` is a check failure — not usage — and must exit
/// nonzero. `cargo xtask agentic-bench` is a declared `covered_by` control
/// for `benchmarks/agentic/**` (`policy/non-rust-allowlist.toml`), so a
/// pipeline chaining it must stop over corrupt or tampered fixtures.
fn finish_bench_run(
    report: &Value,
    benches: &[BenchOutcome],
    json_display: &str,
) -> Result<(), String> {
    let status = report["status"].as_str().unwrap_or("unknown");
    let ready = report["ready_benches"].as_u64().unwrap_or(0);
    let count = report["bench_count"].as_u64().unwrap_or(0);
    let summary = format!("agentic-bench: {status} ({ready} of {count} benches ready)");
    if status != "pass" {
        let failing = benches
            .iter()
            .filter(|bench| bench.status != "ready")
            .map(|bench| format!("{} ({})", bench.bench, bench.status))
            .collect::<Vec<_>>()
            .join(", ");
        let failing = if failing.is_empty() {
            "none named".to_string()
        } else {
            failing
        };
        return Err(format!(
            "{summary}; failing benches: {failing}; receipt written to {json_display}"
        ));
    }
    println!("{summary}");
    Ok(())
}

fn report_markdown(report: &Value) -> String {
    let mut body = String::from("# Agentic Bench\n\n");
    body.push_str(&format!(
        "Status: `{}`\n\nFixtures: `{}`\nReady: `{}` of `{}`\n\n",
        report["status"].as_str().unwrap_or("unknown"),
        report["fixtures_dir"].as_str().unwrap_or("unknown"),
        report["ready_benches"],
        report["bench_count"],
    ));
    let identity = &report["identity"];
    body.push_str(&format!(
        "Source: `{}` at `{}` (unix ms)\n\nVerifier: `{}` (`{}`)\n\nFixtures resolved: `{}`\n\n",
        identity["source_sha"].as_str().unwrap_or("unavailable"),
        identity["timestamp_unix_ms"],
        identity["verifier"].as_str().unwrap_or("unavailable"),
        identity["binary_sha256"].as_str().unwrap_or("unavailable"),
        identity["fixtures_dir_resolved"]
            .as_str()
            .unwrap_or("unknown"),
    ));
    body.push_str("| Bench | Index | Status | Fixtures | Oracle |\n");
    body.push_str("| --- | --- | --- | ---: | --- |\n");
    for bench in report["benches"].as_array().cloned().unwrap_or_default() {
        body.push_str(&format!(
            "| {} | {} | `{}` | {} | `{}` |\n",
            bench["bench"].as_str().unwrap_or("unknown"),
            bench["bench_index"].as_str().unwrap_or("?"),
            bench["status"].as_str().unwrap_or("unknown"),
            bench["fixtures_checked"],
            bench["oracle_command"].as_str().unwrap_or("unavailable"),
        ));
    }
    for bench in report["benches"].as_array().cloned().unwrap_or_default() {
        if let Some(detail) = bench["detail"].as_str() {
            body.push_str(&format!(
                "\n## {}\n\n{}\n",
                bench["bench"].as_str().unwrap_or("unknown"),
                detail,
            ));
        }
    }
    body.push_str(&format!(
        "\nClaim boundary: {}\n",
        report["claim_boundary"].as_str().unwrap_or("unknown"),
    ));
    body
}

#[cfg(test)]
mod tests {
    use super::{
        absolute_display, agentic_bench_in, normalize_bench_id, normalize_repo_path, parse_options,
        valid_digest, verify_bench,
    };
    use serde_json::Value;
    use sha2::{Digest, Sha256};
    use std::fs;
    use std::path::{Path, PathBuf};

    #[test]
    fn parses_bench_filter_and_fixtures_dir() -> Result<(), String> {
        let options = parse_options(&[
            "--bench".to_string(),
            "edit-cage".to_string(),
            "--fixtures".to_string(),
            "benches".to_string(),
        ])?;
        assert_eq!(options.bench, Some("edit-cage".to_string()));
        assert_eq!(options.fixtures_dir, std::path::PathBuf::from("benches"));
        Ok(())
    }

    #[test]
    fn rejects_unknown_arguments_and_bench_ids() -> Result<(), String> {
        for bad in [vec!["--bogus".to_string()], vec!["--bench".to_string()]] {
            if parse_options(&bad).is_ok() {
                return Err(format!("parse_options({bad:?}) unexpectedly succeeded"));
            }
        }
        for bad in ["../edit-cage", ""] {
            if normalize_bench_id(bad).is_ok() {
                return Err(format!(
                    "normalize_bench_id({bad:?}) unexpectedly succeeded"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn repo_paths_reject_escapes_and_digests_require_shape() -> Result<(), String> {
        normalize_repo_path("tests/cage.rs")?;
        for bad in ["../outside", ""] {
            if normalize_repo_path(bad).is_ok() {
                return Err(format!(
                    "normalize_repo_path({bad:?}) unexpectedly succeeded"
                ));
            }
        }
        if !valid_digest("sha256:59a7833db39e0b2fd948aa3ea98b891455ebd4c15d8e6df0453ab6f1116f0e88")
        {
            return Err("valid_digest rejected a shaped digest".to_string());
        }
        if valid_digest("nope") {
            return Err("valid_digest accepted an unshaped digest".to_string());
        }
        Ok(())
    }

    #[test]
    fn absolute_display_resolves_and_normalizes_separators() -> Result<(), String> {
        let display = absolute_display(Path::new("target/ripr/reports"));
        if !Path::new(&display).is_absolute() {
            return Err(format!("absolute_display kept a relative path: {display}"));
        }
        if display.contains('\\') {
            return Err(format!("absolute_display kept a backslash: {display}"));
        }
        Ok(())
    }

    #[test]
    fn bench_dir_without_manifest_reports_missing_manifest() -> Result<(), String> {
        let root = unique_temp_root("missing")?;
        let dir = root.join("fixtures");
        fs::create_dir_all(dir.join("no-manifest"))
            .map_err(|err| format!("create temp bench dir: {err}"))?;
        let outcome = verify_bench(&dir, &dir.join("no-manifest").join("manifest.json"));
        fs::remove_dir_all(&root).map_err(|err| format!("remove temp root: {err}"))?;
        if outcome.status != "missing_manifest" {
            return Err(format!("expected missing_manifest, got {}", outcome.status));
        }
        if outcome.bench != "no-manifest" {
            return Err(format!(
                "expected dir-id bench label, got {}",
                outcome.bench
            ));
        }
        Ok(())
    }

    // ── #6587/#6595 command-contract tests ──────────────────────────────

    const FIXTURE_BODY: &str = "fn answer() -> u32 { 42 }\n";

    fn unique_temp_root(label: &str) -> Result<PathBuf, String> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|err| format!("clock error: {err}"))?
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "ripr-agentic-bench-{label}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).map_err(|err| format!("create temp root: {err}"))?;
        Ok(dir)
    }

    fn digest_of(bytes: &[u8]) -> String {
        let mut rendered = String::from("sha256:");
        for byte in Sha256::digest(bytes) {
            rendered.push_str(&format!("{byte:02x}"));
        }
        rendered
    }

    /// Writes `<fixtures>/<bench_id>/manifest.json` binding a real
    /// `input/src/lib.rs` fixture by digest, shaped like the shipped B1-B6
    /// manifests.
    fn write_ready_bench(fixtures: &Path, bench_id: &str, body: &str) -> Result<(), String> {
        let bench_dir = fixtures.join(bench_id);
        let lib = bench_dir.join("input/src/lib.rs");
        let lib_parent = lib.parent().ok_or("fixture path has no parent")?;
        fs::create_dir_all(lib_parent).map_err(|err| format!("create bench input dir: {err}"))?;
        fs::write(&lib, body).map_err(|err| format!("write fixture: {err}"))?;
        let forbidden = if bench_id == "edit-cage" {
            ",\n  \"forbidden_paths\": [{ \"path\": \"input/src\", \"scope\": \"subtree\" }]"
        } else {
            ""
        };
        let manifest = format!(
            concat!(
                "{{\n",
                "  \"schema_version\": \"ripr-agentic-bench-manifest-v1\",\n",
                "  \"bench\": \"{bench_id}\",\n",
                "  \"bench_index\": \"B9\",\n",
                "  \"title\": \"Test bench\",\n",
                "  \"stimulus\": \"Stimulus text.\",\n",
                "  \"oracle_states\": [\"pass\", \"fail\"],\n",
                "  \"oracle_command\": \"cargo test --help\",\n",
                "  \"selected_target\": {{ \"path\": \"input/src/lib.rs\", \"scope\": \"exact\" }},\n",
                "  \"allowed_surface\": [{{ \"path\": \"input\", \"scope\": \"subtree\" }}]{forbidden},\n",
                "  \"fixtures\": [\n",
                "    {{\n",
                "      \"path\": \"input/src/lib.rs\",\n",
                "      \"sha256\": \"{digest}\"\n",
                "    }}\n",
                "  ],\n",
                "  \"bounds\": {{ \"max_paths\": 64, \"max_file_bytes\": 65536, \"max_total_bytes\": 1048576 }}\n",
                "}}\n"
            ),
            bench_id = bench_id,
            forbidden = forbidden,
            digest = digest_of(body.as_bytes()),
        );
        fs::write(bench_dir.join("manifest.json"), manifest)
            .map_err(|err| format!("write manifest: {err}"))?;
        Ok(())
    }

    fn run_command(root: &Path, fixtures: &Path, extra: &[&str]) -> (Result<(), String>, PathBuf) {
        let report_dir = root.join("reports");
        let mut args = vec![
            "--fixtures".to_string(),
            fixtures.to_string_lossy().to_string(),
        ];
        args.extend(extra.iter().map(|arg| (*arg).to_string()));
        (agentic_bench_in(&args, &report_dir), report_dir)
    }

    fn read_receipt(report_dir: &Path) -> Result<Value, String> {
        let text = fs::read_to_string(report_dir.join("agentic-bench.json"))
            .map_err(|err| format!("read receipt: {err}"))?;
        serde_json::from_str(&text).map_err(|err| format!("parse receipt: {err}"))
    }

    fn cleanup(root: &Path) -> Result<(), String> {
        fs::remove_dir_all(root).map_err(|err| format!("remove temp root: {err}"))
    }

    /// The command must have failed; the pre-#6587 contract returned `Ok(())`
    /// for every verification outcome.
    fn expect_err(result: Result<(), String>, scenario: &str) -> Result<String, String> {
        match result {
            Ok(()) => Err(format!(
                "old behavior exited 0 on {scenario}; the command must fail closed"
            )),
            Err(err) => Ok(err),
        }
    }

    /// Control: a clean corpus passes, and the receipt carries the #6595 run
    /// identity (source revision, verifier build, timestamp, resolved input
    /// root) plus the verified per-bench digest set.
    #[test]
    fn ready_run_passes_and_receipt_carries_run_identity() -> Result<(), String> {
        let root = unique_temp_root("ready")?;
        let fixtures = root.join("agentic");
        write_ready_bench(&fixtures, "probe-bench", FIXTURE_BODY)?;
        let (result, report_dir) = run_command(&root, &fixtures, &[]);
        result.map_err(|err| format!("clean corpus must pass: {err}"))?;
        let receipt = read_receipt(&report_dir)?;
        if receipt["status"].as_str() != Some("pass") {
            return Err(format!("expected pass rollup, got {receipt}"));
        }
        let identity = &receipt["identity"];
        let source_sha = identity["source_sha"]
            .as_str()
            .ok_or("identity.source_sha missing")?;
        // Outside a git checkout the documented fallback is "unavailable";
        // inside one it must be a real revision.
        if source_sha != "unavailable"
            && (source_sha.len() != 40 || !source_sha.chars().all(|char| char.is_ascii_hexdigit()))
        {
            return Err(format!(
                "identity.source_sha is not a git revision: {source_sha}"
            ));
        }
        if identity["timestamp_unix_ms"].as_u64().unwrap_or(0) == 0 {
            return Err("identity.timestamp_unix_ms missing".to_string());
        }
        let binary_sha256 = identity["binary_sha256"]
            .as_str()
            .ok_or("identity.binary_sha256 missing")?;
        if binary_sha256.len() != 64 || !binary_sha256.chars().all(|char| char.is_ascii_hexdigit())
        {
            return Err(format!(
                "identity.binary_sha256 is not a sha256 digest: {binary_sha256}"
            ));
        }
        if identity["verifier"].as_str() != Some("cargo xtask agentic-bench") {
            return Err("identity.verifier must name the xtask command".to_string());
        }
        let resolved = identity["fixtures_dir_resolved"]
            .as_str()
            .ok_or("identity.fixtures_dir_resolved missing")?;
        if !Path::new(resolved).is_absolute() || !resolved.ends_with("/agentic") {
            return Err(format!(
                "identity.fixtures_dir_resolved is not the absolute input root: {resolved}"
            ));
        }
        let bench = &receipt["benches"][0];
        let verified = bench["fixtures_verified"]
            .as_array()
            .ok_or("bench fixtures_verified missing")?;
        if verified.len() != 1 {
            return Err(format!("expected one verified fixture, got {verified:?}"));
        }
        let expected_digest = digest_of(FIXTURE_BODY.as_bytes());
        if verified[0]["sha256"].as_str() != Some(expected_digest.as_str())
            || verified[0]["path"].as_str() != Some("input/src/lib.rs")
        {
            return Err(format!(
                "verified fixture digest set does not pin the input: {verified:?}"
            ));
        }
        if bench["manifest_sha256"].as_str().is_none() {
            return Err("bench manifest_sha256 missing".to_string());
        }
        let manifest = bench["manifest"]
            .as_str()
            .ok_or("bench manifest path missing")?;
        if !Path::new(manifest).is_absolute() || manifest.contains('\\') {
            return Err(format!(
                "bench manifest path is not absolute/normalized: {manifest}"
            ));
        }
        cleanup(&root)
    }

    /// #6587 scenario 1: an appended line drifts a digest-bound fixture; the
    /// command must exit nonzero (the old code exited 0) while keeping the
    /// `fixture_unverified` receipt on disk.
    #[test]
    fn fixture_digest_drift_fails_the_command_and_keeps_the_receipt() -> Result<(), String> {
        let root = unique_temp_root("drift")?;
        let fixtures = root.join("agentic");
        write_ready_bench(&fixtures, "drift-bench", FIXTURE_BODY)?;
        let lib = fixtures.join("drift-bench/input/src/lib.rs");
        let mut corrupted =
            fs::read_to_string(&lib).map_err(|err| format!("read fixture: {err}"))?;
        corrupted.push_str("fn tampered() {}\n");
        fs::write(&lib, corrupted).map_err(|err| format!("write tampered fixture: {err}"))?;
        let (result, report_dir) = run_command(&root, &fixtures, &[]);
        let err = expect_err(result, "fixture_unverified")?;
        if !err.contains("fixture_unverified") {
            return Err(format!("error must name the failure class: {err}"));
        }
        if !err.contains("drift-bench") {
            return Err(format!("error must name the failing bench: {err}"));
        }
        let receipt = read_receipt(&report_dir)?;
        if receipt["status"].as_str() != Some("inconclusive") {
            return Err(format!("expected inconclusive rollup, got {receipt}"));
        }
        if receipt["benches"][0]["status"].as_str() != Some("fixture_unverified") {
            return Err(format!("expected fixture_unverified bench, got {receipt}"));
        }
        cleanup(&root)
    }

    /// #6587 scenario 2: a bench directory without `manifest.json` must fail
    /// the command while preserving the denominator in the receipt.
    #[test]
    fn missing_manifest_fails_the_command_and_keeps_the_receipt() -> Result<(), String> {
        let root = unique_temp_root("nomanifest")?;
        let fixtures = root.join("agentic");
        write_ready_bench(&fixtures, "ready-bench", FIXTURE_BODY)?;
        let ghost = fixtures.join("ghost-bench");
        fs::create_dir_all(&ghost).map_err(|err| format!("create ghost bench dir: {err}"))?;
        let (result, report_dir) = run_command(&root, &fixtures, &[]);
        let err = expect_err(result, "missing_manifest")?;
        if !err.contains("missing_manifest") {
            return Err(format!("error must name the failure class: {err}"));
        }
        let receipt = read_receipt(&report_dir)?;
        if receipt["status"].as_str() != Some("inconclusive") {
            return Err(format!("expected inconclusive rollup, got {receipt}"));
        }
        if receipt["bench_count"].as_u64() != Some(2) {
            return Err(format!("denominator must keep both benches, got {receipt}"));
        }
        let Some(benches) = receipt["benches"].as_array() else {
            return Err("receipt benches array missing".to_string());
        };
        let statuses: Vec<_> = benches
            .iter()
            .filter_map(|bench| bench["status"].as_str())
            .collect();
        if !statuses.contains(&"missing_manifest") || !statuses.contains(&"ready") {
            return Err(format!(
                "expected mixed ready/missing_manifest, got {statuses:?}"
            ));
        }
        cleanup(&root)
    }

    /// #6587 scenario 3: a truncated manifest must fail the command as
    /// `invalid_manifest`, with the parse failure in the receipt.
    #[test]
    fn invalid_manifest_fails_the_command_and_keeps_the_receipt() -> Result<(), String> {
        let root = unique_temp_root("truncated")?;
        let fixtures = root.join("agentic");
        write_ready_bench(&fixtures, "truncated-bench", FIXTURE_BODY)?;
        let manifest = fixtures.join("truncated-bench/manifest.json");
        let text = fs::read_to_string(&manifest).map_err(|err| format!("read manifest: {err}"))?;
        let cut = text.len() - 20;
        fs::write(&manifest, &text[..cut])
            .map_err(|err| format!("write truncated manifest: {err}"))?;
        let (result, report_dir) = run_command(&root, &fixtures, &[]);
        let err = expect_err(result, "invalid_manifest")?;
        if !err.contains("invalid_manifest") {
            return Err(format!("error must name the failure class: {err}"));
        }
        let receipt = read_receipt(&report_dir)?;
        if receipt["benches"][0]["status"].as_str() != Some("invalid_manifest") {
            return Err(format!("expected invalid_manifest bench, got {receipt}"));
        }
        let detail = receipt["benches"][0]["detail"]
            .as_str()
            .ok_or("invalid_manifest must carry the parse detail")?;
        if !detail.contains("parse manifest") {
            return Err(format!("detail must carry the parse failure: {detail}"));
        }
        cleanup(&root)
    }

    /// #6587 scenario 4: an edit-cage manifest whose selected target lands on
    /// a forbidden path must fail the command as `invalid_manifest`.
    #[test]
    fn edit_cage_forbidden_target_fails_the_command_and_keeps_the_receipt() -> Result<(), String> {
        let root = unique_temp_root("forbidden")?;
        let fixtures = root.join("agentic");
        write_ready_bench(&fixtures, "edit-cage", FIXTURE_BODY)?;
        let (result, report_dir) = run_command(&root, &fixtures, &[]);
        let err = expect_err(result, "a forbidden selected target")?;
        if !err.contains("invalid_manifest") {
            return Err(format!("error must name the failure class: {err}"));
        }
        let receipt = read_receipt(&report_dir)?;
        if receipt["benches"][0]["status"].as_str() != Some("invalid_manifest") {
            return Err(format!("expected invalid_manifest bench, got {receipt}"));
        }
        let detail = receipt["benches"][0]["detail"]
            .as_str()
            .ok_or("invalid_manifest must carry the surface detail")?;
        if !detail.contains("forbidden path") {
            return Err(format!("detail must name the forbidden path: {detail}"));
        }
        cleanup(&root)
    }

    /// PR #6683 review: `--bench` naming an existing bench directory whose
    /// manifest was deleted is a `missing_manifest` verification outcome with
    /// a receipt, not a discovery error that would leave a prior receipt in
    /// place.
    #[test]
    fn filtered_missing_manifest_fails_with_a_receipt() -> Result<(), String> {
        let root = unique_temp_root("filtered")?;
        let fixtures = root.join("agentic");
        let ghost = fixtures.join("ghost-bench");
        fs::create_dir_all(&ghost).map_err(|err| format!("create ghost bench dir: {err}"))?;
        let (result, report_dir) = run_command(&root, &fixtures, &["--bench", "ghost-bench"]);
        let err = expect_err(result, "a filtered missing manifest")?;
        if !err.contains("missing_manifest") {
            return Err(format!("error must name the failure class: {err}"));
        }
        let receipt = read_receipt(&report_dir)?;
        if receipt["status"].as_str() != Some("inconclusive") {
            return Err(format!("expected inconclusive rollup, got {receipt}"));
        }
        if receipt["bench_count"].as_u64() != Some(1) {
            return Err(format!("filtered denominator must stay one, got {receipt}"));
        }
        if receipt["benches"][0]["status"].as_str() != Some("missing_manifest") {
            return Err(format!("expected missing_manifest bench, got {receipt}"));
        }
        cleanup(&root)
    }
}
