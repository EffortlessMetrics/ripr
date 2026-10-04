//! `cargo xtask agentic-bench` — shared runner skeleton for the
//! `benchmarks/agentic/*` benches.
//!
//! The skeleton discovers one manifest per bench, validates it strictly,
//! verifies every `sha256:`-bound fixture file (the same rendering as
//! `ripr::agent::provenance::sha256_file`), and writes a versioned
//! `ripr-agentic-bench-v1` JSON/Markdown receipt pair. It claims fixture and
//! provenance readiness only; each bench's behavioral oracle runs under its
//! own `cargo test` command, named in the receipt.

use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

const SCHEMA_VERSION: &str = "ripr-agentic-bench-v1";
const MANIFEST_SCHEMA: &str = "ripr-agentic-bench-manifest-v1";
const DEFAULT_FIXTURES_DIR: &str = "benchmarks/agentic";

pub(crate) fn agentic_bench(args: &[String]) -> Result<(), String> {
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
    crate::write_report("agentic-bench.json", &format!("{json_text}\n"))?;
    crate::write_report("agentic-bench.md", &report_markdown(&report))?;
    println!("Wrote target/ripr/reports/agentic-bench.json");
    println!("Wrote target/ripr/reports/agentic-bench.md");
    Ok(())
}

const USAGE: &str = "usage: cargo xtask agentic-bench [--bench <id>] [--fixtures <dir>]";

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
        if !manifest.is_file() {
            return Err(format!(
                "unknown bench `{id}`: {} does not exist",
                manifest.display()
            ));
        }
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

fn sha256_file(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    let sum = Sha256::digest(&bytes);
    let mut rendered = String::from("sha256:");
    for byte in sum {
        rendered.push_str(&format!("{byte:02x}"));
    }
    Ok(rendered)
}

#[derive(Clone, Debug)]
struct BenchOutcome {
    bench: String,
    bench_index: String,
    title: String,
    manifest: String,
    status: &'static str,
    detail: Option<String>,
    fixtures_checked: usize,
    oracle_command: Option<String>,
}

/// Strict per-bench verification: an invalid manifest or an unverified
/// fixture fails that bench closed. Bench-specific checks plug in by bench
/// id; benches without one keep generic manifest plus digest verification.
fn verify_bench(fixtures_dir: &Path, manifest_path: &Path) -> BenchOutcome {
    let manifest_label = manifest_path.to_string_lossy().to_string();
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
                   checked: usize| {
        BenchOutcome {
            bench: manifest.map_or_else(|| dir_id.clone(), |loaded| loaded.bench.clone()),
            bench_index: manifest.map_or_else(String::new, |loaded| loaded.bench_index.clone()),
            title: manifest.map_or_else(String::new, |loaded| loaded.title.clone()),
            manifest: manifest_label.clone(),
            status,
            detail,
            fixtures_checked: checked,
            oracle_command: manifest.map(|loaded| loaded.oracle_command.clone()),
        }
    };
    if !manifest_path.is_file() {
        return outcome(
            "missing_manifest",
            None,
            Some("manifest.json is missing".to_string()),
            0,
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
            );
        }
    };
    let mut manifest: Manifest = match serde_json::from_slice(&bytes) {
        Ok(manifest) => manifest,
        Err(err) => {
            return outcome(
                "invalid_manifest",
                None,
                Some(format!("parse manifest: {err}")),
                0,
            );
        }
    };
    if let Err(detail) = validate_manifest(&mut manifest, &dir_id) {
        return outcome("invalid_manifest", None, Some(detail), 0);
    }
    if manifest.bench == "edit-cage"
        && let Err(detail) = validate_edit_cage_surface(&manifest)
    {
        return outcome("invalid_manifest", Some(&manifest), Some(detail), 0);
    }
    let mut checked = 0;
    for fixture in &manifest.fixtures {
        let path = bench_dir.join(&fixture.path);
        match sha256_file(&path) {
            Ok(observed) if observed == fixture.sha256 => {
                checked += 1;
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
                );
            }
            Err(err) => {
                return outcome(
                    "fixture_unverified",
                    Some(&manifest),
                    Some(format!("fixture unreadable {}: {err}", fixture.path)),
                    checked,
                );
            }
        }
    }
    outcome("ready", Some(&manifest), None, checked)
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
/// surface and outside every forbidden path. No B6 manifest ships yet (the
/// production-routed B6 rebuild is tracked separately); this stays as the
/// bench-specific plug-in point.
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
        "fixtures_dir": options.fixtures_dir.to_string_lossy(),
        "bench_filter": options.bench,
        "ready_benches": ready,
        "bench_count": benches.len(),
        "benches": benches.iter().map(|bench| json!({
            "bench": bench.bench,
            "bench_index": bench.bench_index,
            "title": bench.title,
            "manifest": bench.manifest,
            "status": bench.status,
            "detail": bench.detail,
            "fixtures_checked": bench.fixtures_checked,
            "oracle_command": bench.oracle_command,
        })).collect::<Vec<_>>(),
        "claim_boundary": "Fixture and provenance readiness only: manifests validated and fixture digests verified. Behavioral verdicts come from each bench's oracle command, not this receipt."
    })
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
        normalize_bench_id, normalize_repo_path, parse_options, valid_digest, verify_bench,
    };

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
    fn bench_dir_without_manifest_reports_missing_manifest() -> Result<(), String> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|err| format!("clock error: {err}"))?
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "ripr-agentic-bench-missing-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(dir.join("no-manifest"))
            .map_err(|err| format!("create temp bench dir: {err}"))?;
        let outcome = verify_bench(&dir, &dir.join("no-manifest").join("manifest.json"));
        std::fs::remove_dir_all(&dir).map_err(|err| format!("remove temp bench dir: {err}"))?;
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
}
