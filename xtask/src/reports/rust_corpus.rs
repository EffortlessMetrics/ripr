//! `cargo xtask rust-corpus` — the shared, pinned corpus of real Rust
//! repositories that evaluation work (scoreboards, verdict checks, CPU,
//! memory and size measurements, mutation spot-checks, first-run tests)
//! runs against instead of each lane picking its own handful of repos.
//!
//! The manifest (`benchmarks/rust_corpus/manifest.json`) pins each repository
//! to one upstream commit (`sha`) and its first parent (`base_sha`), so every
//! subject has both a whole-repository state and a real upstream diff. Repos
//! carry a tier (`fast` for pull-request and quick local runs, `full` for the
//! nightly/release set) and the stress categories they exist to cover.
//!
//! `check` is offline and validates the manifest's own contract: exact
//! 40-hex pins, unique ids, known tiers and categories, and that every
//! declared category is covered (fast-tier coverage unless the category is
//! `full_only`). `fetch` is opt-in network: it shallow-fetches exactly the
//! two pinned commits per repo, verifies the checkout's HEAD and first parent
//! against the pins, and writes an index consumers read for paths.

use crate::run::{capture_output_with_timeout, run, run_output_owned_with_timeout};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

const DEFAULT_MANIFEST: &str = "benchmarks/rust_corpus/manifest.json";
const DEFAULT_ROOT: &str = "target/ripr/corpus";
const MANIFEST_KIND: &str = "ripr_rust_corpus_manifest";
const MANIFEST_SCHEMA_VERSION: &str = "1";
const INDEX_FILE: &str = "index.json";
/// Written inside `.git` after a successful fetch, so the working tree stays
/// clean and only checkouts this command created are ever replaced.
const OWNER_MARKER: &str = "ripr-corpus-checkout";
const INDEX_SCHEMA_VERSION: &str = "ripr-rust-corpus-index-v1";
const TIER_FAST: &str = "fast";
const TIER_FULL: &str = "full";
const TIER_TARGETS: &str = "targets";
const ROLE_REPRESENTATIVE: &str = "representative";
const ROLE_INTEGRATION_TARGET: &str = "integration_target";
const CRATE_KINDS: [&str; 2] = ["library", "binary"];
/// Every class must appear in both tiers so a fast run still sees
/// well-maintained, legacy, and ordinary code.
const PROFILE_CLASSES: [&str; 3] = ["good", "legacy", "common"];
const DEFAULT_GIT_TIMEOUT_SECS: u64 = 600;
/// The selection_rule bounds on the pinned change. The line bound matches
/// the rule exactly (it counts every changed .rs line). The recorded file
/// count includes test files while the rule bounds non-test files at 12, so
/// the file bound here is looser and only rejects a sprawling pin.
const SELECTION_RUST_FILES: std::ops::RangeInclusive<u64> = 1..=40;
const SELECTION_RUST_LINES: std::ops::RangeInclusive<u64> = 6..=400;
const DEFAULT_SMOKE_TIMEOUT_SECS: u64 = 600;
const SMOKE_SCHEMA_VERSION: &str = "ripr-rust-corpus-smoke-v1";
const SUMMARY_CLASSES: [&str; 7] = [
    "exposed",
    "weakly_exposed",
    "reachable_unrevealed",
    "no_static_path",
    "infection_unknown",
    "propagation_unknown",
    "static_unknown",
];

const USAGE: &str = "usage: cargo xtask rust-corpus check [--manifest <path>]
       cargo xtask rust-corpus list [--manifest <path>] [--tier fast|full|targets] [--root <dir>]
       cargo xtask rust-corpus fetch --allow-network [--manifest <path>] [--tier fast|full|targets] [--repo <id>]... [--root <dir>] [--timeout-secs <n>]
       cargo xtask rust-corpus smoke [--manifest <path>] [--tier fast|full|targets] [--repo <id>]... [--root <dir>] [--ripr <bin>] [--timeout-secs <n>]

fast selects repos with tier = fast; full selects every repo; targets selects
repos whose roles include integration_target.
fetch writes <root>/<id> (default root target/ripr/corpus) and <root>/index.json.
smoke runs `ripr check --base <base_sha> --format json` on each fetched repo (default
binary: a fresh release build) and writes target/ripr/reports/rust-corpus-smoke.{json,md}.";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: String,
    kind: String,
    corpus_version: String,
    description: String,
    selection_rule: String,
    limits: Vec<String>,
    roles: BTreeMap<String, String>,
    tiers: BTreeMap<String, TierSpec>,
    stress_categories: Vec<StressCategory>,
    profile_classes: BTreeMap<String, String>,
    known_gaps: Vec<String>,
    repos: Vec<RepoEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TierSpec {
    purpose: String,
    selects: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StressCategory {
    id: String,
    description: String,
    full_only: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RepoEntry {
    id: String,
    url: String,
    tier: String,
    crate_kind: String,
    license: String,
    sha: String,
    base_sha: String,
    change: ChangeSummary,
    roles: Vec<String>,
    profile: Profile,
    stresses: Vec<String>,
    why: String,
    probe: Probe,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Profile {
    class: String,
    style: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChangeSummary {
    subject: String,
    committed: String,
    rust_files_changed: u64,
    rust_lines_changed: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Probe {
    rust_files: u64,
    rust_lines: u64,
    cargo_manifests: u64,
    build_rs: u64,
    proc_macro_crates: u64,
    no_std_files: u64,
    harness_false_targets: u64,
    checkout_mb: u64,
    extern_crate_files: u64,
    files_over_3000_lines: u64,
}

#[derive(Debug, PartialEq, Eq)]
enum Action {
    Check,
    List,
    Fetch,
    Smoke,
}

#[derive(Debug)]
struct Options {
    action: Action,
    manifest: PathBuf,
    root: PathBuf,
    tier: String,
    repos: Vec<String>,
    allow_network: bool,
    timeout: Option<Duration>,
    ripr: Option<PathBuf>,
}

pub(crate) fn rust_corpus(args: &[String]) -> Result<(), String> {
    if args.is_empty() || args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{USAGE}");
        return Ok(());
    }
    let options = parse_options(args)?;
    let manifest = load_manifest(&options.manifest)?;
    match options.action {
        Action::Check => {
            let fast = select(&manifest, TIER_FAST, &[])?;
            let fast_mb: u64 = fast.iter().map(|repo| repo.probe.checkout_mb).sum();
            let full_mb: u64 = manifest
                .repos
                .iter()
                .map(|repo| repo.probe.checkout_mb)
                .sum();
            println!(
                "rust-corpus check: ok ({} repos; fast {} repos ~{fast_mb} MB, full ~{full_mb} MB checked out; corpus_version {})",
                manifest.repos.len(),
                fast.len(),
                manifest.corpus_version
            );
            Ok(())
        }
        Action::List => {
            for repo in select(&manifest, &options.tier, &options.repos)? {
                println!(
                    "{}\t{}\t{}\t{}\t{}",
                    repo.id,
                    repo.tier,
                    options.root.join(&repo.id).display(),
                    repo.base_sha,
                    repo.sha
                );
            }
            Ok(())
        }
        Action::Fetch => fetch(&manifest, &options),
        Action::Smoke => smoke(&manifest, &options),
    }
}

fn parse_options(args: &[String]) -> Result<Options, String> {
    let (first, rest) = args
        .split_first()
        .ok_or_else(|| format!("rust-corpus requires a subcommand\n{USAGE}"))?;
    let action = match first.as_str() {
        "check" => Action::Check,
        "list" => Action::List,
        "fetch" => Action::Fetch,
        "smoke" => Action::Smoke,
        other => return Err(format!("unknown rust-corpus subcommand `{other}`\n{USAGE}")),
    };
    let mut options = Options {
        action,
        manifest: PathBuf::from(DEFAULT_MANIFEST),
        root: PathBuf::from(DEFAULT_ROOT),
        tier: TIER_FAST.to_string(),
        repos: Vec::new(),
        allow_network: false,
        timeout: None,
        ripr: None,
    };
    let mut index = 0;
    while index < rest.len() {
        let flag = rest[index].as_str();
        let mut value = || -> Result<String, String> {
            index += 1;
            rest.get(index)
                .cloned()
                .ok_or_else(|| format!("rust-corpus {flag} requires a value"))
        };
        match flag {
            "--manifest" => options.manifest = PathBuf::from(value()?),
            "--root" => options.root = PathBuf::from(value()?),
            "--tier" => {
                let tier = value()?;
                if ![TIER_FAST, TIER_FULL, TIER_TARGETS].contains(&tier.as_str()) {
                    return Err(format!(
                        "rust-corpus --tier expects `fast`, `full`, or `targets`, got `{tier}`"
                    ));
                }
                options.tier = tier;
            }
            "--repo" => options.repos.push(value()?),
            "--allow-network" => options.allow_network = true,
            "--ripr" => options.ripr = Some(PathBuf::from(value()?)),
            "--timeout-secs" => {
                let raw = value()?;
                let secs = raw.parse::<u64>().map_err(|err| {
                    format!("rust-corpus --timeout-secs expects an integer, got `{raw}`: {err}")
                })?;
                if secs == 0 {
                    return Err("rust-corpus --timeout-secs must be positive".to_string());
                }
                options.timeout = Some(Duration::from_secs(secs));
            }
            other => return Err(format!("unknown rust-corpus argument `{other}`\n{USAGE}")),
        }
        index += 1;
    }
    if options.action == Action::Fetch && !options.allow_network {
        return Err(
            "rust-corpus fetch clones external repositories; pass --allow-network to opt in"
                .to_string(),
        );
    }
    Ok(options)
}

fn load_manifest(path: &Path) -> Result<Manifest, String> {
    let text = fs::read_to_string(path)
        .map_err(|err| format!("read rust-corpus manifest {}: {err}", path.display()))?;
    parse_manifest(&text).map_err(|err| format!("{}: {err}", path.display()))
}

fn parse_manifest(text: &str) -> Result<Manifest, String> {
    let manifest: Manifest =
        serde_json::from_str(text).map_err(|err| format!("invalid rust-corpus manifest: {err}"))?;
    let problems = validate(&manifest);
    if problems.is_empty() {
        Ok(manifest)
    } else {
        Err(format!(
            "rust-corpus manifest failed {} check(s):\n  - {}",
            problems.len(),
            problems.join("\n  - ")
        ))
    }
}

/// Returns every contract violation instead of the first, so a corpus
/// refresh sees the whole repair list in one run.
fn validate(manifest: &Manifest) -> Vec<String> {
    let mut problems = Vec::new();
    if manifest.schema_version != MANIFEST_SCHEMA_VERSION {
        problems.push(format!(
            "schema_version must be `{MANIFEST_SCHEMA_VERSION}`, got `{}`",
            manifest.schema_version
        ));
    }
    if manifest.kind != MANIFEST_KIND {
        problems.push(format!(
            "kind must be `{MANIFEST_KIND}`, got `{}`",
            manifest.kind
        ));
    }
    for (field, value) in [
        ("corpus_version", &manifest.corpus_version),
        ("description", &manifest.description),
        ("selection_rule", &manifest.selection_rule),
    ] {
        if value.trim().is_empty() {
            problems.push(format!("{field} must not be empty"));
        }
    }
    if manifest.limits.is_empty() {
        problems.push("limits must state at least one claim boundary".to_string());
    }
    if manifest.known_gaps.iter().any(|gap| gap.trim().is_empty()) {
        problems.push("known_gaps entries must not be empty".to_string());
    }
    let tier_names: BTreeSet<&str> = manifest.tiers.keys().map(String::as_str).collect();
    if tier_names != BTreeSet::from([TIER_FAST, TIER_FULL, TIER_TARGETS]) {
        problems.push(format!(
            "tiers must be exactly `fast`, `full`, and `targets`, got {tier_names:?}"
        ));
    }
    let role_names: BTreeSet<&str> = manifest.roles.keys().map(String::as_str).collect();
    if role_names != BTreeSet::from([ROLE_REPRESENTATIVE, ROLE_INTEGRATION_TARGET]) {
        problems.push(format!(
            "roles must be exactly `{ROLE_REPRESENTATIVE}` and `{ROLE_INTEGRATION_TARGET}`, got {role_names:?}"
        ));
    }
    for (name, tier) in &manifest.tiers {
        if tier.purpose.trim().is_empty() || tier.selects.trim().is_empty() {
            problems.push(format!("tier `{name}` needs a purpose and a selects rule"));
        }
    }

    let mut categories = BTreeMap::new();
    for category in &manifest.stress_categories {
        if !is_slug(&category.id, '_') {
            problems.push(format!(
                "stress category id `{}` must be lowercase snake_case",
                category.id
            ));
        }
        if category.description.trim().is_empty() {
            problems.push(format!(
                "stress category `{}` needs a description",
                category.id
            ));
        }
        if categories
            .insert(category.id.as_str(), category.full_only)
            .is_some()
        {
            problems.push(format!("duplicate stress category `{}`", category.id));
        }
    }

    let mut ids = BTreeSet::new();
    let mut urls = BTreeSet::new();
    for repo in &manifest.repos {
        let id = repo.id.as_str();
        if !is_slug(id, '-') {
            problems.push(format!("repo id `{id}` must be lowercase kebab-case"));
        }
        if !ids.insert(id) {
            problems.push(format!("duplicate repo id `{id}`"));
        }
        if !is_github_repo_url(&repo.url) {
            problems.push(format!(
                "repo `{id}` url `{}` must be https://github.com/<owner>/<repo> without .git or a trailing slash",
                repo.url
            ));
        }
        if !urls.insert(repo.url.to_ascii_lowercase()) {
            problems.push(format!("repo `{id}` duplicates url `{}`", repo.url));
        }
        if repo.tier != TIER_FAST && repo.tier != TIER_FULL {
            problems.push(format!(
                "repo `{id}` tier must be `fast` or `full`, got `{}`",
                repo.tier
            ));
        }
        if !CRATE_KINDS.contains(&repo.crate_kind.as_str()) {
            problems.push(format!(
                "repo `{id}` crate_kind must be one of {CRATE_KINDS:?}, got `{}`",
                repo.crate_kind
            ));
        }
        for (field, sha) in [("sha", &repo.sha), ("base_sha", &repo.base_sha)] {
            if !is_full_sha(sha) {
                problems.push(format!(
                    "repo `{id}` {field} `{sha}` must be a full 40-character lowercase hex commit id"
                ));
            }
        }
        if repo.sha == repo.base_sha {
            problems.push(format!(
                "repo `{id}` sha and base_sha are identical, so the pinned diff is empty"
            ));
        }
        for (field, value) in [
            ("license", &repo.license),
            ("why", &repo.why),
            ("change.subject", &repo.change.subject),
        ] {
            if value.trim().is_empty() {
                problems.push(format!("repo `{id}` {field} must not be empty"));
            }
        }
        if !is_iso_date(&repo.change.committed) {
            problems.push(format!(
                "repo `{id}` change.committed `{}` must be YYYY-MM-DD",
                repo.change.committed
            ));
        }
        if !SELECTION_RUST_FILES.contains(&repo.change.rust_files_changed)
            || !SELECTION_RUST_LINES.contains(&repo.change.rust_lines_changed)
        {
            problems.push(format!(
                "repo `{id}` pinned change ({} Rust files, {} Rust lines) is outside the selection_rule bounds ({SELECTION_RUST_FILES:?} files, {SELECTION_RUST_LINES:?} lines)",
                repo.change.rust_files_changed, repo.change.rust_lines_changed
            ));
        }
        if repo.probe.rust_files == 0 || repo.probe.rust_lines == 0 {
            problems.push(format!(
                "repo `{id}` probe must record Rust files and lines"
            ));
        }
        if repo.probe.cargo_manifests == 0 {
            problems.push(format!(
                "repo `{id}` probe must record at least one Cargo manifest"
            ));
        }
        if repo.roles.is_empty() {
            problems.push(format!("repo `{id}` must have at least one role"));
        }
        let mut seen_roles = BTreeSet::new();
        for role in &repo.roles {
            if !manifest.roles.contains_key(role) {
                problems.push(format!("repo `{id}` names undeclared role `{role}`"));
            }
            if !seen_roles.insert(role.as_str()) {
                problems.push(format!("repo `{id}` repeats role `{role}`"));
            }
        }
        if !PROFILE_CLASSES.contains(&repo.profile.class.as_str()) {
            problems.push(format!(
                "repo `{id}` profile.class must be one of {PROFILE_CLASSES:?}, got `{}`",
                repo.profile.class
            ));
        }
        if repo.profile.style.trim().is_empty() {
            problems.push(format!("repo `{id}` profile.style must not be empty"));
        }
        if repo.stresses.is_empty() {
            problems.push(format!(
                "repo `{id}` must name at least one stress category"
            ));
        }
        let mut seen = BTreeSet::new();
        for stress in &repo.stresses {
            if !categories.contains_key(stress.as_str()) {
                problems.push(format!("repo `{id}` names undeclared stress `{stress}`"));
            }
            if !seen.insert(stress.as_str()) {
                problems.push(format!("repo `{id}` repeats stress `{stress}`"));
            }
        }
    }
    let declared: BTreeSet<&str> = manifest
        .profile_classes
        .keys()
        .map(String::as_str)
        .collect();
    if declared != BTreeSet::from(PROFILE_CLASSES) {
        problems.push(format!(
            "profile_classes must be exactly {PROFILE_CLASSES:?}, got {declared:?}"
        ));
    }
    for class in PROFILE_CLASSES {
        for tier in [TIER_FAST, TIER_FULL] {
            let covered = manifest.repos.iter().any(|repo| {
                repo.profile.class == class && (tier == TIER_FULL || repo.tier == tier)
            });
            if !covered {
                problems.push(format!(
                    "profile class `{class}` has no repo in the {tier} tier"
                ));
            }
        }
    }
    if !manifest
        .repos
        .iter()
        .any(|repo| has_role(repo, ROLE_INTEGRATION_TARGET))
    {
        problems.push("the targets tier must select at least one repo".to_string());
    }
    if !manifest.repos.iter().any(|repo| repo.tier == TIER_FAST) {
        problems.push("the fast tier must select at least one repo".to_string());
    }

    for (category, full_only) in &categories {
        let covering: Vec<&RepoEntry> = manifest
            .repos
            .iter()
            .filter(|repo| repo.stresses.iter().any(|stress| stress == category))
            .collect();
        if covering.is_empty() {
            problems.push(format!(
                "stress category `{category}` is declared but no repo covers it"
            ));
        } else if !full_only && !covering.iter().any(|repo| repo.tier == TIER_FAST) {
            problems.push(format!(
                "stress category `{category}` is not full_only but no fast-tier repo covers it"
            ));
        }
    }
    problems
}

fn select<'a>(
    manifest: &'a Manifest,
    tier: &str,
    only: &[String],
) -> Result<Vec<&'a RepoEntry>, String> {
    for wanted in only {
        if !manifest.repos.iter().any(|repo| &repo.id == wanted) {
            return Err(format!("rust-corpus has no repo `{wanted}`"));
        }
    }
    Ok(manifest
        .repos
        .iter()
        .filter(|repo| {
            if only.is_empty() {
                match tier {
                    TIER_FULL => true,
                    TIER_TARGETS => has_role(repo, ROLE_INTEGRATION_TARGET),
                    _ => repo.tier == tier,
                }
            } else {
                only.contains(&repo.id)
            }
        })
        .collect())
}

fn has_role(repo: &RepoEntry, role: &str) -> bool {
    repo.roles.iter().any(|candidate| candidate == role)
}

fn fetch(manifest: &Manifest, options: &Options) -> Result<(), String> {
    let repos = select(manifest, &options.tier, &options.repos)?;
    fs::create_dir_all(&options.root)
        .map_err(|err| format!("create corpus root {}: {err}", options.root.display()))?;
    let mut entries = Vec::new();
    let mut failures = Vec::new();
    for repo in &repos {
        let dir = options.root.join(&repo.id);
        let timeout = options
            .timeout
            .unwrap_or(Duration::from_secs(DEFAULT_GIT_TIMEOUT_SECS));
        match materialize(repo, &dir, timeout) {
            Ok(reused) => {
                eprintln!(
                    "rust-corpus: {} @ {} {}",
                    repo.id,
                    short(&repo.sha),
                    if reused { "(reused)" } else { "(fetched)" }
                );
                entries.push(json!({
                    "id": repo.id,
                    "tier": repo.tier,
                    "crate_kind": repo.crate_kind,
                    "path": dir.to_string_lossy(),
                    "url": repo.url,
                    "sha": repo.sha,
                    "base_sha": repo.base_sha,
                    "roles": repo.roles,
                    "profile": repo.profile,
                    "stresses": repo.stresses,
                    "probe": repo.probe,
                }));
            }
            Err(err) => {
                eprintln!("rust-corpus: {} failed: {err}", repo.id);
                failures.push(format!("{}: {err}", repo.id));
            }
        }
    }
    // A partial fetch (--repo, or a smaller tier) keeps the entries an earlier
    // fetch recorded for other repos, so consumers reading the index for paths
    // do not lose them. Entries from another corpus version are dropped.
    let index_path = options.root.join(INDEX_FILE);
    // A repo that failed this time drops its old entry: its checkout may be gone.
    let attempted: BTreeSet<&str> = repos.iter().map(|repo| repo.id.as_str()).collect();
    let kept = previous_index_entries(&index_path, &manifest.corpus_version)
        .into_iter()
        .filter(|entry| {
            entry["id"]
                .as_str()
                .is_some_and(|id| !attempted.contains(id))
        });
    entries.extend(kept);
    entries.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    let index = json!({
        "schema_version": INDEX_SCHEMA_VERSION,
        "corpus_version": manifest.corpus_version,
        "tier": if options.repos.is_empty() { options.tier.as_str() } else { "selected" },
        "repos": entries,
        "failed": failures,
    });
    let rendered = serde_json::to_string_pretty(&index)
        .map_err(|err| format!("render corpus index: {err}"))?;
    fs::write(&index_path, format!("{rendered}\n"))
        .map_err(|err| format!("write corpus index {}: {err}", index_path.display()))?;
    println!("rust-corpus: wrote {}", index_path.display());
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "rust-corpus fetch failed for {} repo(s):\n  - {}",
            failures.len(),
            failures.join("\n  - ")
        ))
    }
}

/// Runs the diff-scoped `ripr check` each corpus lane starts from and records
/// the run status per repo. A missing, unpinned or modified checkout is a recorded
/// `not_fetched` row, never a silent skip, and any row that is not a clean
/// analysis makes the receipt `inconclusive` rather than a pass.
fn smoke(manifest: &Manifest, options: &Options) -> Result<(), String> {
    let repos = select(manifest, &options.tier, &options.repos)?;
    let ripr = match &options.ripr {
        Some(path) => path.clone(),
        None => {
            run("cargo", &["build", "-p", "ripr", "--release", "--quiet"])?;
            std::env::var_os("CARGO_TARGET_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("target"))
                .join("release")
                .join(format!("ripr{}", std::env::consts::EXE_SUFFIX))
        }
    };
    let timeout = options
        .timeout
        .unwrap_or(Duration::from_secs(DEFAULT_SMOKE_TIMEOUT_SECS));
    let ripr_text = ripr.to_string_lossy().into_owned();
    let mut rows = Vec::new();
    for repo in repos {
        let dir = options.root.join(&repo.id);
        // ripr reads the working tree, so a tracked edit left by another lane
        // would be measured under the pinned repository's name.
        if let Some(problem) = checkout_problem(repo, &dir, timeout) {
            rows.push(json!({
                "id": repo.id,
                "tier": repo.tier,
                "profile": repo.profile.class,
                "status": "not_fetched",
                "reason": format!("{problem}; run `cargo xtask rust-corpus fetch --allow-network --repo {}`", repo.id),
            }));
            continue;
        }
        let args = vec![
            "check".to_string(),
            "--root".to_string(),
            dir.to_string_lossy().into_owned(),
            "--base".to_string(),
            repo.base_sha.clone(),
            "--format".to_string(),
            "json".to_string(),
        ];
        let output = match capture_output_with_timeout(
            &ripr_text,
            &args,
            &[],
            timeout,
            &format!("rust-corpus smoke ripr check for `{}`", repo.id),
        ) {
            Ok(output) => output,
            Err(err) => {
                eprintln!("rust-corpus smoke: {} spawn_failed: {err}", repo.id);
                rows.push(json!({
                    "id": repo.id,
                    "tier": repo.tier,
                    "profile": repo.profile.class,
                    "status": "spawn_failed",
                    "reason": err,
                }));
                continue;
            }
        };
        let row = smoke_row(
            repo,
            &output.stdout,
            output.status.and_then(|s| s.code()),
            output.timed_out,
            output.duration,
        );
        eprintln!(
            "rust-corpus smoke: {} {} ({} ms)",
            repo.id,
            row["status"].as_str().unwrap_or("unknown"),
            output.duration.as_millis()
        );
        rows.push(row);
    }
    let clean = rows.iter().all(|row| row["status"] == "analyzed");
    let receipt = json!({
        "schema_version": SMOKE_SCHEMA_VERSION,
        "corpus_version": manifest.corpus_version,
        "tier": if options.repos.is_empty() { options.tier.as_str() } else { "selected" },
        "ripr": ripr_text,
        "status": if clean { "pass" } else { "inconclusive" },
        "claim_boundary": "diff-scoped default-mode run status, wall time, and finding counts for this binary on this runner; not verdict correctness and not a memory measurement",
        "repos": rows,
    });
    let rendered = serde_json::to_string_pretty(&receipt)
        .map_err(|err| format!("render smoke receipt: {err}"))?;
    crate::write_report("rust-corpus-smoke.json", &format!("{rendered}\n"))?;
    crate::write_report("rust-corpus-smoke.md", &smoke_markdown(&receipt))?;
    println!(
        "rust-corpus smoke: {} (target/ripr/reports/rust-corpus-smoke.{{json,md}})",
        receipt["status"].as_str().unwrap_or("unknown")
    );
    Ok(())
}

fn smoke_row(
    repo: &RepoEntry,
    stdout: &str,
    exit_code: Option<i32>,
    timed_out: bool,
    duration: Duration,
) -> Value {
    let parsed: Option<Value> = serde_json::from_str(stdout).ok();
    let run_status = parsed
        .as_ref()
        .and_then(|value| value.pointer("/analysis_scope/run_status"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let summary = parsed.as_ref().and_then(|value| value.get("summary"));
    let count = |key: &str| summary.and_then(|s| s.get(key)).and_then(Value::as_u64);
    // `analyzed` needs a clean exit and a findings count. A fail-closed
    // run_status is reported as itself; a run that claims `analyzed` but
    // exited non-zero or carried no summary is not counted as analyzed.
    let status = match (&run_status, exit_code) {
        _ if timed_out => "timed_out".to_string(),
        _ if parsed.is_none() => "no_json".to_string(),
        (Some(run_status), _) if run_status != "analyzed" => run_status.clone(),
        (_, Some(0)) if count("findings").is_some() => "analyzed".to_string(),
        (_, Some(0)) => "no_summary".to_string(),
        _ => "nonzero_exit".to_string(),
    };
    let classes: BTreeMap<&str, Option<u64>> = SUMMARY_CLASSES
        .iter()
        .map(|class| (*class, count(class)))
        .collect();
    json!({
        "id": repo.id,
        "tier": repo.tier,
        "profile": repo.profile.class,
        "status": status,
        "exit_code": exit_code,
        "duration_ms": duration.as_millis(),
        "changed_rust_files": count("changed_rust_files"),
        "findings": count("findings"),
        "classes": classes,
    })
}

fn smoke_markdown(receipt: &Value) -> String {
    let mut out = format!(
        "# Rust corpus smoke\n\nStatus: {}. Corpus {}, tier {}, binary `{}`.\n\n{}.\n\n| repo | tier | profile | status | exit | ms | findings |\n|---|---|---|---|---:|---:|---:|\n",
        receipt["status"].as_str().unwrap_or("unknown"),
        receipt["corpus_version"].as_str().unwrap_or(""),
        receipt["tier"].as_str().unwrap_or(""),
        receipt["ripr"].as_str().unwrap_or(""),
        receipt["claim_boundary"].as_str().unwrap_or(""),
    );
    let cell = |value: &Value| match value {
        Value::Null => "-".to_string(),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    };
    for row in receipt["repos"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            cell(&row["id"]),
            cell(&row["tier"]),
            cell(&row["profile"]),
            cell(&row["status"]),
            cell(&row["exit_code"]),
            cell(&row["duration_ms"]),
            cell(&row["findings"]),
        ));
    }
    let reasons: Vec<String> = receipt["repos"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|row| {
            row["reason"]
                .as_str()
                .map(|reason| format!("- {}: {reason}\n", cell(&row["id"])))
        })
        .collect();
    if !reasons.is_empty() {
        out.push_str("\nNot run:\n\n");
        out.push_str(&reasons.concat());
    }
    out
}

/// Fetches exactly the pinned commit at depth 2 (the commit and its first
/// parent) and verifies both against the manifest. Returns `true` when an
/// existing checkout already matched and was reused.
fn materialize(repo: &RepoEntry, dir: &Path, timeout: Duration) -> Result<bool, String> {
    // A lane that edited a checkout (mutation spot-checks do) must not leave
    // that edit behind as the "pinned" subject. Untracked files such as a
    // build's target/ do not count, so a built checkout is still reused.
    if is_clean_pinned_checkout(repo, dir, timeout) {
        return Ok(true);
    }
    // Only a directory this command could have created may be replaced: an
    // empty one, or a git checkout whose origin is this repo's URL. Anything
    // else under --root (a user's own clone, notes, another project) is left
    // alone and the fetch for this repo is refused.
    if dir.exists() && !is_replaceable_checkout(dir) {
        return Err(format!(
            "{} exists and is neither empty nor a checkout created by rust-corpus fetch; \
             refusing to replace it. Move or delete it yourself, or pass a different --root",
            dir.display()
        ));
    }
    if dir.exists() {
        fs::remove_dir_all(dir)
            .map_err(|err| format!("remove stale checkout {}: {err}", dir.display()))?;
    }
    fs::create_dir_all(dir).map_err(|err| format!("create {}: {err}", dir.display()))?;
    git(dir, &["init", "--quiet"], timeout)?;
    git(dir, &["remote", "add", "origin", &repo.url], timeout)?;
    git(
        dir,
        &[
            "fetch",
            "--quiet",
            "--depth",
            "2",
            "--no-tags",
            "origin",
            &repo.sha,
        ],
        timeout,
    )?;
    git(
        dir,
        &["checkout", "--quiet", "--detach", "FETCH_HEAD"],
        timeout,
    )?;
    verify_pins(repo, dir, timeout)?;
    let marker = dir.join(".git").join(OWNER_MARKER);
    fs::write(&marker, format!("{}\n", repo.url))
        .map_err(|err| format!("write ownership marker {}: {err}", marker.display()))?;
    Ok(false)
}

fn previous_index_entries(index_path: &Path, corpus_version: &str) -> Vec<Value> {
    let Ok(text) = fs::read_to_string(index_path) else {
        return Vec::new();
    };
    let Ok(previous) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    if previous["schema_version"] != INDEX_SCHEMA_VERSION
        || previous["corpus_version"] != corpus_version
    {
        return Vec::new();
    }
    previous["repos"].as_array().cloned().unwrap_or_default()
}

/// An origin URL alone does not prove ownership (a user's own clone of the
/// same repository matches it), so a non-empty directory is replaceable only
/// when it carries the marker a previous fetch wrote.
fn is_replaceable_checkout(dir: &Path) -> bool {
    let empty = fs::read_dir(dir).is_ok_and(|mut entries| entries.next().is_none());
    empty || dir.join(".git").join(OWNER_MARKER).is_file()
}

/// Why a checkout is not usable as the pinned subject, or `None` when it is.
fn checkout_problem(repo: &RepoEntry, dir: &Path, timeout: Duration) -> Option<String> {
    if !dir.exists() {
        return Some("checkout missing".to_string());
    }
    if !dir.join(".git").exists() {
        return Some("not a git checkout".to_string());
    }
    if let Err(err) = verify_pins(repo, dir, timeout) {
        return Some(err);
    }
    match git(
        dir,
        &["status", "--porcelain", "--untracked-files=no"],
        timeout,
    ) {
        Ok(status) if status.is_empty() => None,
        Ok(_) => Some("tracked files are modified".to_string()),
        Err(err) => Some(err),
    }
}

/// A checkout counts as the pinned subject only when both pins match and no
/// tracked file is modified.
fn is_clean_pinned_checkout(repo: &RepoEntry, dir: &Path, timeout: Duration) -> bool {
    checkout_problem(repo, dir, timeout).is_none()
}

fn verify_pins(repo: &RepoEntry, dir: &Path, timeout: Duration) -> Result<(), String> {
    let head = git(dir, &["rev-parse", "HEAD"], timeout)?;
    if head != repo.sha {
        return Err(format!(
            "checkout HEAD {head} does not match pinned sha {}",
            repo.sha
        ));
    }
    let parent = git(dir, &["rev-parse", "HEAD^1"], timeout)?;
    if parent != repo.base_sha {
        return Err(format!(
            "pinned sha's first parent {parent} does not match base_sha {}",
            repo.base_sha
        ));
    }
    Ok(())
}

fn git(dir: &Path, args: &[&str], timeout: Duration) -> Result<String, String> {
    let mut owned = vec!["-C".to_string(), dir.to_string_lossy().into_owned()];
    owned.extend(args.iter().map(|arg| (*arg).to_string()));
    let context = format!("rust-corpus git {}", args.first().copied().unwrap_or(""));
    run_output_owned_with_timeout("git", &owned, timeout, &context)
        .map(|stdout| stdout.trim().to_string())
}

fn short(sha: &str) -> &str {
    sha.get(..12).unwrap_or(sha)
}

fn is_full_sha(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn is_slug(value: &str, separator: char) -> bool {
    !value.is_empty()
        && !value.starts_with(separator)
        && !value.ends_with(separator)
        && value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == separator)
}

fn is_github_repo_url(url: &str) -> bool {
    let Some(path) = url.strip_prefix("https://github.com/") else {
        return false;
    };
    let mut parts = path.split('/');
    let (Some(owner), Some(name), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    let valid = |part: &str| {
        !part.is_empty()
            && part
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    };
    valid(owner) && valid(name) && !name.ends_with(".git")
}

fn is_iso_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn committed_manifest() -> Result<Value, String> {
        let text = fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join(DEFAULT_MANIFEST),
        )
        .map_err(|err| format!("read committed manifest: {err}"))?;
        serde_json::from_str(&text).map_err(|err| format!("parse committed manifest: {err}"))
    }

    fn problems_for(value: &Value) -> Result<String, String> {
        let text = serde_json::to_string(value).map_err(|err| err.to_string())?;
        match parse_manifest(&text) {
            Ok(_) => Err("mutated manifest unexpectedly validated".to_string()),
            Err(err) => Ok(err),
        }
    }

    fn repo_mut<'a>(value: &'a mut Value, id: &str) -> Result<&'a mut Value, String> {
        value["repos"]
            .as_array_mut()
            .and_then(|repos| repos.iter_mut().find(|repo| repo["id"] == id))
            .ok_or_else(|| format!("committed manifest has no repo `{id}`"))
    }

    #[test]
    fn committed_manifest_validates_and_has_both_tiers() -> Result<(), String> {
        let text = serde_json::to_string(&committed_manifest()?).map_err(|err| err.to_string())?;
        let manifest = parse_manifest(&text)?;
        let fast = select(&manifest, TIER_FAST, &[])?;
        let full = select(&manifest, TIER_FULL, &[])?;
        assert!(!fast.is_empty());
        assert!(
            fast.len() < full.len(),
            "full tier must add repos beyond fast"
        );
        assert_eq!(full.len(), manifest.repos.len());
        Ok(())
    }

    #[test]
    fn abbreviated_or_uppercase_pin_is_rejected() -> Result<(), String> {
        let mut value = committed_manifest()?;
        repo_mut(&mut value, "serde")?["sha"] = json!("9d3410e");
        let err = problems_for(&value)?;
        assert!(err.contains("repo `serde` sha `9d3410e`"), "{err}");

        let mut value = committed_manifest()?;
        let base = repo_mut(&mut value, "serde")?["base_sha"]
            .as_str()
            .map(str::to_ascii_uppercase)
            .ok_or("base_sha missing")?;
        repo_mut(&mut value, "serde")?["base_sha"] = json!(base);
        let err = problems_for(&value)?;
        assert!(err.contains("repo `serde` base_sha"), "{err}");
        Ok(())
    }

    #[test]
    fn identical_sha_and_base_is_rejected() -> Result<(), String> {
        let mut value = committed_manifest()?;
        let repo = repo_mut(&mut value, "anyhow")?;
        repo["base_sha"] = repo["sha"].clone();
        let err = problems_for(&value)?;
        assert!(err.contains("pinned diff is empty"), "{err}");
        Ok(())
    }

    #[test]
    fn undeclared_stress_and_duplicate_id_are_rejected() -> Result<(), String> {
        let mut value = committed_manifest()?;
        repo_mut(&mut value, "regex")?["stresses"] = json!(["generated_code", "gpu_shaders"]);
        let duplicate = repo_mut(&mut value, "anyhow")?.clone();
        value["repos"]
            .as_array_mut()
            .ok_or("repos is not an array")?
            .push(duplicate);
        let err = problems_for(&value)?;
        assert!(err.contains("undeclared stress `gpu_shaders`"), "{err}");
        assert!(err.contains("duplicate repo id `anyhow`"), "{err}");
        Ok(())
    }

    #[test]
    fn category_only_covered_in_full_tier_must_be_full_only() -> Result<(), String> {
        // rusqlite is the only fast-tier `bundled_c` repo; demoting it to the
        // full tier leaves a non-full_only category without fast coverage.
        let mut value = committed_manifest()?;
        repo_mut(&mut value, "rusqlite")?["tier"] = json!("full");
        let err = problems_for(&value)?;
        assert!(
            err.contains(
                "stress category `bundled_c` is not full_only but no fast-tier repo covers it"
            ),
            "{err}"
        );
        Ok(())
    }

    #[test]
    fn declared_but_uncovered_category_is_rejected() -> Result<(), String> {
        let mut value = committed_manifest()?;
        value["stress_categories"]
            .as_array_mut()
            .ok_or("stress_categories is not an array")?
            .push(json!({"id": "gpu_shaders", "description": "x", "full_only": true}));
        let err = problems_for(&value)?;
        assert!(
            err.contains("`gpu_shaders` is declared but no repo covers it"),
            "{err}"
        );
        Ok(())
    }

    #[test]
    fn unknown_fields_and_bad_urls_are_rejected() -> Result<(), String> {
        let mut value = committed_manifest()?;
        repo_mut(&mut value, "tokio")?["branch"] = json!("master");
        let err = problems_for(&value)?;
        assert!(err.contains("unknown field `branch`"), "{err}");

        let mut value = committed_manifest()?;
        repo_mut(&mut value, "tokio")?["url"] = json!("https://github.com/tokio-rs/tokio.git");
        let err = problems_for(&value)?;
        assert!(err.contains("repo `tokio` url"), "{err}");
        Ok(())
    }

    #[test]
    fn select_honours_tier_and_explicit_repos() -> Result<(), String> {
        let text = serde_json::to_string(&committed_manifest()?).map_err(|err| err.to_string())?;
        let manifest = parse_manifest(&text)?;
        assert!(
            select(&manifest, TIER_FAST, &[])?
                .iter()
                .all(|repo| repo.tier == TIER_FAST)
        );
        let targets: Vec<&str> = select(&manifest, TIER_TARGETS, &[])?
            .iter()
            .map(|repo| repo.id.as_str())
            .collect();
        assert!(targets.contains(&"cargo-mutants"), "{targets:?}");
        assert!(
            targets.contains(&"tokio"),
            "full-tier targets are still selected: {targets:?}"
        );
        assert!(!targets.contains(&"serde"), "{targets:?}");
        let chosen = select(&manifest, TIER_FAST, &["bevy".to_string()])?;
        assert_eq!(chosen.len(), 1);
        assert_eq!(chosen[0].id, "bevy");
        let missing = select(&manifest, TIER_FULL, &["nope".to_string()]).err();
        assert_eq!(missing.as_deref(), Some("rust-corpus has no repo `nope`"));
        Ok(())
    }

    #[test]
    fn fetch_requires_explicit_network_opt_in() {
        let args = vec!["fetch".to_string()];
        let err = parse_options(&args).err().unwrap_or_default();
        assert!(err.contains("--allow-network"), "{err}");
        let args = vec!["fetch".to_string(), "--allow-network".to_string()];
        let options = parse_options(&args).map_err(|err| format!("opt-in rejected: {err}"));
        assert_eq!(
            options.map(|options| (options.action, options.allow_network)),
            Ok((Action::Fetch, true))
        );
    }

    #[test]
    fn pinned_change_outside_selection_bounds_is_rejected() -> Result<(), String> {
        let mut value = committed_manifest()?;
        repo_mut(&mut value, "serde")?["change"]["rust_lines_changed"] = json!(1);
        repo_mut(&mut value, "fd")?["change"]["rust_lines_changed"] = json!(401);
        let err = problems_for(&value)?;
        assert!(
            err.contains("repo `serde` pinned change (3 Rust files, 1 Rust lines)"),
            "{err}"
        );
        assert!(err.contains("repo `fd` pinned change"), "{err}");
        Ok(())
    }

    #[test]
    fn undeclared_or_missing_role_is_rejected() -> Result<(), String> {
        let mut value = committed_manifest()?;
        repo_mut(&mut value, "fd")?["roles"] = json!(["sponsor"]);
        repo_mut(&mut value, "serde")?["roles"] = json!([]);
        let err = problems_for(&value)?;
        assert!(
            err.contains("repo `fd` names undeclared role `sponsor`"),
            "{err}"
        );
        assert!(
            err.contains("repo `serde` must have at least one role"),
            "{err}"
        );
        Ok(())
    }

    #[test]
    fn profile_class_missing_from_fast_tier_is_rejected() -> Result<(), String> {
        // atuin, rstest, prost, and rusqlite are the fast-tier `common` repos;
        // reclassifying all four leaves the fast tier without ordinary code.
        let mut value = committed_manifest()?;
        for id in ["atuin", "rstest", "prost", "rusqlite"] {
            repo_mut(&mut value, id)?["profile"]["class"] = json!("good");
        }
        let err = problems_for(&value)?;
        assert!(
            err.contains("profile class `common` has no repo in the fast tier"),
            "{err}"
        );

        let mut value = committed_manifest()?;
        repo_mut(&mut value, "image")?["profile"]["class"] = json!("vintage");
        let err = problems_for(&value)?;
        assert!(err.contains("repo `image` profile.class"), "{err}");
        Ok(())
    }

    fn fixture_repo() -> Result<RepoEntry, String> {
        let text = serde_json::to_string(&committed_manifest()?).map_err(|err| err.to_string())?;
        parse_manifest(&text)?
            .repos
            .into_iter()
            .find(|repo| repo.id == "anyhow")
            .ok_or_else(|| "committed manifest has no anyhow".to_string())
    }

    #[test]
    fn smoke_row_separates_analyzed_fail_closed_and_broken_runs() -> Result<(), String> {
        let repo = fixture_repo()?;
        let ok = r#"{"summary":{"changed_rust_files":1,"findings":2,"weakly_exposed":2}}"#;
        let row = smoke_row(&repo, ok, Some(0), false, Duration::from_millis(5));
        assert_eq!(row["status"], "analyzed");
        assert_eq!(row["findings"], 2);
        assert_eq!(row["classes"]["weakly_exposed"], 2);

        let capped =
            r#"{"analysis_scope":{"run_status":"diff_scope_oversized"},"summary":{"findings":0}}"#;
        let row = smoke_row(&repo, capped, Some(2), false, Duration::from_millis(5));
        assert_eq!(row["status"], "diff_scope_oversized");

        let row = smoke_row(
            &repo,
            "panic text",
            Some(101),
            false,
            Duration::from_millis(5),
        );
        assert_eq!(row["status"], "no_json");
        let row = smoke_row(&repo, "{}", None, true, Duration::from_millis(5));
        assert_eq!(row["status"], "timed_out");
        let row = smoke_row(&repo, "{}", Some(1), false, Duration::from_millis(5));
        assert_eq!(row["status"], "nonzero_exit");

        let claimed = r#"{"analysis_scope":{"run_status":"analyzed"},"summary":{"findings":3}}"#;
        let row = smoke_row(&repo, claimed, Some(2), false, Duration::from_millis(5));
        assert_eq!(row["status"], "nonzero_exit");
        let row = smoke_row(&repo, claimed, Some(0), false, Duration::from_millis(5));
        assert_eq!(row["status"], "analyzed");
        let row = smoke_row(&repo, "{}", Some(0), false, Duration::from_millis(5));
        assert_eq!(row["status"], "no_summary");
        Ok(())
    }

    #[test]
    fn fetch_only_replaces_empty_dirs_or_its_own_checkouts() -> Result<(), String> {
        let repo = fixture_repo()?;
        let timeout = Duration::from_secs(30);
        let root = crate::tests::temp_dir("rust-corpus-ownership");
        let dir = root.join(&repo.id);

        fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
        assert!(is_replaceable_checkout(&dir));

        fs::write(dir.join("notes.txt"), "mine").map_err(|err| err.to_string())?;
        assert!(!is_replaceable_checkout(&dir));
        let refused = materialize(&repo, &dir, timeout);
        assert_eq!(
            refused
                .as_ref()
                .map_err(|err| err.contains("refusing to replace it")),
            Err(true)
        );
        assert!(dir.join("notes.txt").exists(), "user file must survive");

        // A user's own clone of the same repository is not ours to replace.
        git(&dir, &["init", "--quiet"], timeout)?;
        git(&dir, &["remote", "add", "origin", &repo.url], timeout)?;
        assert!(!is_replaceable_checkout(&dir));
        fs::write(dir.join(".git").join(OWNER_MARKER), &repo.url).map_err(|err| err.to_string())?;
        assert!(is_replaceable_checkout(&dir));
        assert_eq!(
            checkout_problem(&repo, &root.join("absent"), timeout).as_deref(),
            Some("checkout missing")
        );

        fs::remove_dir_all(&root).map_err(|err| err.to_string())
    }

    #[test]
    fn previous_index_entries_only_carry_the_same_corpus_version() -> Result<(), String> {
        let root = crate::tests::temp_dir("rust-corpus-index");
        let path = root.join(INDEX_FILE);
        assert!(previous_index_entries(&path, "v1").is_empty());
        let index = json!({
            "schema_version": INDEX_SCHEMA_VERSION,
            "corpus_version": "v1",
            "repos": [{"id": "serde"}],
        });
        fs::write(&path, index.to_string()).map_err(|err| err.to_string())?;
        assert_eq!(previous_index_entries(&path, "v1").len(), 1);
        assert!(previous_index_entries(&path, "v2").is_empty());
        fs::remove_dir_all(&root).map_err(|err| err.to_string())
    }

    #[test]
    fn helpers_reject_near_misses() {
        assert!(is_github_repo_url("https://github.com/serde-rs/serde"));
        assert!(!is_github_repo_url("https://github.com/serde-rs/serde/"));
        assert!(!is_github_repo_url("http://github.com/serde-rs/serde"));
        assert!(!is_github_repo_url("https://gitlab.com/serde-rs/serde"));
        assert!(is_iso_date("2026-10-04"));
        assert!(!is_iso_date("2026-1-04"));
        assert!(is_slug("rust-analyzer", '-'));
        assert!(!is_slug("Rust-analyzer", '-'));
        assert!(!is_slug("-x", '-'));
    }
}
