//! Exact-input, read-only source/swarm promotion preflight.
//!
//! This command is deliberately a preflight only. It validates two named
//! repository identities and exact commit inputs, then performs `git
//! merge-tree` in a disposable repository containing fetched objects. It
//! never changes either caller checkout and never creates the source join.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const SCHEMA: &str = "ripr.source_promotion_preflight.v1";
const REPORT_JSON: &str = "source-promotion-preflight.json";
const REPORT_MD: &str = "source-promotion-preflight.md";
const DEFAULT_OUT: &str = "target/ripr/source-promotion";

#[derive(Clone, Debug)]
struct Options {
    source_parent: String,
    swarm_parent: String,
    swarm_ref: String,
    source_repo: PathBuf,
    swarm_repo: PathBuf,
    source_main: String,
    swarm_main: String,
    version: String,
    resolved_tree: Option<String>,
    out: PathBuf,
    source_remote: String,
    swarm_remote: String,
}

#[derive(Clone, Debug, Serialize)]
struct RepositoryIdentity {
    role: String,
    remote: String,
    expected_remote: String,
    common_dir_verified: bool,
    immutable_ref: Option<String>,
    immutable_ref_sha: Option<String>,
    identity: String,
    root_verified: bool,
    remote_verified: bool,
}

#[derive(Clone, Debug, Serialize)]
struct CommitRange {
    all_reachable_count: usize,
    first_parent_count: usize,
    all_reachable_sha256: String,
    first_parent_ordered_sha256: String,
    all_reachable_ordered_recipe: String,
    first_parent_ordered_recipe: String,
}

#[derive(Clone, Debug, Serialize)]
struct VersionSide {
    workspace_version: Option<String>,
    crate_version: Option<String>,
    extension_version: Option<String>,
    cargo_lock_package_version: Option<String>,
    npm_lock_root_version: Option<String>,
    changelog_mentions_version: Option<bool>,
}

#[derive(Clone, Debug, Serialize)]
struct VersionState {
    requested_version: String,
    source: VersionSide,
    swarm: VersionSide,
}

#[derive(Clone, Debug, Serialize)]
struct DryMerge {
    status: String,
    preview_tree: Option<String>,
    reviewed_resolved_tree: Option<String>,
    reviewed_resolved_tree_verified: bool,
    conflicts: Vec<String>,
    diagnostics: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
struct Receipt {
    schema: String,
    mode: String,
    source_parent: String,
    swarm_parent: String,
    swarm_ref: String,
    swarm_ref_sha: String,
    source_main: String,
    swarm_main: String,
    merge_base: String,
    source_repository: RepositoryIdentity,
    swarm_repository: RepositoryIdentity,
    source_range: CommitRange,
    swarm_range: CommitRange,
    source_survivor_candidates: Vec<String>,
    swarm_only_paths: Vec<String>,
    swarm_authority_resolution_candidates: Vec<String>,
    dry_merge: DryMerge,
    version_state: VersionState,
    invalidation_rules: Vec<String>,
    next_commands: Vec<String>,
}

pub(crate) fn source_promotion(args: &[String]) -> Result<(), String> {
    let options = parse_args(args)?;
    let receipt = build_receipt(&options)?;
    let json = serde_json::to_string_pretty(&receipt)
        .map_err(|error| format!("failed to serialize source-promotion receipt: {error}"))?;
    let markdown = render_markdown(&receipt);
    fs::create_dir_all(&options.out)
        .map_err(|error| format!("failed to create {}: {error}", options.out.display()))?;
    fs::write(options.out.join(REPORT_JSON), format!("{json}\n"))
        .map_err(|error| format!("failed to write source-promotion JSON: {error}"))?;
    fs::write(options.out.join(REPORT_MD), markdown)
        .map_err(|error| format!("failed to write source-promotion Markdown: {error}"))?;
    println!("Wrote {}", options.out.join(REPORT_JSON).display());
    println!("Wrote {}", options.out.join(REPORT_MD).display());
    Ok(())
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    if args.first().map(String::as_str) != Some("preflight") {
        return Err(usage());
    }
    let mut values = BTreeMap::new();
    let mut index = 1;
    while index < args.len() {
        let key = args[index].as_str();
        if !key.starts_with("--") || index + 1 >= args.len() {
            return Err(usage());
        }
        if !matches!(
            key,
            "--source-parent"
                | "--swarm-parent"
                | "--swarm-ref"
                | "--source-repo"
                | "--swarm-repo"
                | "--source-main"
                | "--swarm-main"
                | "--version"
                | "--resolved-tree"
                | "--source-remote"
                | "--swarm-remote"
                | "--out"
        ) {
            return Err(format!("unknown option {key}\n{}", usage()));
        }
        if values.contains_key(key) {
            return Err(format!("duplicate option {key}\n{}", usage()));
        }
        values.insert(key, args[index + 1].clone());
        index += 2;
    }
    let required = |key: &str| -> Result<String, String> {
        values
            .get(key)
            .filter(|value| !value.trim().is_empty())
            .cloned()
            .ok_or_else(|| format!("missing {key}\n{}", usage()))
    };
    let source_parent = required("--source-parent")?;
    let swarm_parent = required("--swarm-parent")?;
    let swarm_ref = required("--swarm-ref")?;
    validate_sha("--source-parent", "commit", &source_parent)?;
    validate_sha("--swarm-parent", "commit", &swarm_parent)?;
    let version = required("--version")?;
    validate_swarm_ref(&swarm_ref, &version, &swarm_parent)?;
    let source_repo = PathBuf::from(required("--source-repo")?);
    let swarm_repo = PathBuf::from(required("--swarm-repo")?);
    let resolved_tree = values.get("--resolved-tree").cloned();
    if let Some(tree) = &resolved_tree {
        validate_sha("--resolved-tree", "tree", tree)?;
    }
    Ok(Options {
        source_parent,
        swarm_parent,
        swarm_ref,
        source_repo,
        swarm_repo,
        source_main: values
            .get("--source-main")
            .cloned()
            .unwrap_or_else(|| "origin/main".to_string()),
        swarm_main: values
            .get("--swarm-main")
            .cloned()
            .unwrap_or_else(|| "origin/main".to_string()),
        version,
        resolved_tree,
        out: values
            .get("--out")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_OUT)),
        source_remote: values
            .get("--source-remote")
            .cloned()
            .unwrap_or_else(|| "EffortlessMetrics/ripr".to_string()),
        swarm_remote: values
            .get("--swarm-remote")
            .cloned()
            .unwrap_or_else(|| "EffortlessMetrics/ripr-swarm".to_string()),
    })
}

fn usage() -> String {
    "usage: cargo xtask source-promotion preflight --source-parent <full-sha> --swarm-parent <full-sha> --swarm-ref <immutable-ref> --source-repo <path> --swarm-repo <path> --version <version> [--resolved-tree <full-tree-sha>] [--source-main <rev>] [--swarm-main <rev>] [--source-remote <owner/repo>] [--swarm-remote <owner/repo>] [--out <dir>]".to_string()
}

fn validate_sha(name: &str, object_kind: &str, value: &str) -> Result<(), String> {
    if value.len() != 40 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!(
            "{name} must be a complete 40-character hexadecimal {object_kind} SHA"
        ));
    }
    Ok(())
}

fn validate_swarm_ref(reference: &str, version: &str, parent: &str) -> Result<(), String> {
    let expected = format!("refs/ripr/release-{version}-{parent}");
    if reference != expected {
        return Err(format!(
            "--swarm-ref must use the governed immutable format {expected}"
        ));
    }
    Ok(())
}

fn build_receipt(options: &Options) -> Result<Receipt, String> {
    let source = inspect_repository(
        "source",
        &options.source_repo,
        &options.source_remote,
        &options.source_parent,
        &options.source_main,
        None,
    )?;
    let swarm = inspect_repository(
        "swarm",
        &options.swarm_repo,
        &options.swarm_remote,
        &options.swarm_parent,
        &options.swarm_main,
        Some(&options.swarm_ref),
    )?;
    let source_root = repository_root(&options.source_repo)?;
    let swarm_root = repository_root(&options.swarm_repo)?;
    ensure_distinct_repositories(
        &source_root,
        &swarm_root,
        &source.common_dir,
        &swarm.common_dir,
    )?;
    let reviewed_resolved_tree_verified = options
        .resolved_tree
        .as_deref()
        .map(|tree| verify_tree_in_repositories(&source_root, &swarm_root, tree))
        .transpose()?
        .is_some();
    let (merge_base, source_range, swarm_range, source_paths, swarm_paths, dry_merge) =
        with_disposable_repo(
            &source_root,
            &swarm_root,
            &options.source_parent,
            &options.swarm_parent,
            |repo| {
                let merge_base = git(
                    repo,
                    &["merge-base", &options.source_parent, &options.swarm_parent],
                )?
                .trim()
                .to_string();
                let source_range = commit_range(repo, &merge_base, &options.source_parent)?;
                let swarm_range = commit_range(repo, &merge_base, &options.swarm_parent)?;
                let source_paths = changed_paths(repo, &merge_base, &options.source_parent)?;
                let swarm_paths = changed_paths(repo, &merge_base, &options.swarm_parent)?;
                let dry_merge = dry_merge_from_repo(
                    repo,
                    &options.source_parent,
                    &options.swarm_parent,
                    options.resolved_tree.as_deref(),
                    reviewed_resolved_tree_verified,
                )?;
                Ok((
                    merge_base,
                    source_range,
                    swarm_range,
                    source_paths,
                    swarm_paths,
                    dry_merge,
                ))
            },
        )?;
    let source_path_set = source_paths.iter().cloned().collect::<BTreeSet<_>>();
    let mut swarm_only_paths = swarm_paths
        .iter()
        .filter(|path| !source_path_set.contains(*path))
        .cloned()
        .collect::<Vec<_>>();
    swarm_only_paths.sort();
    let mut swarm_authority_resolution_candidates = swarm_only_paths
        .iter()
        .filter(|path| is_swarm_authority_path(path))
        .cloned()
        .collect::<Vec<_>>();
    swarm_authority_resolution_candidates.sort();
    Ok(Receipt {
        schema: SCHEMA.to_string(),
        mode: if merge_base == options.source_parent {
            "fast_forward".to_string()
        } else {
            "two_parent_join".to_string()
        },
        source_parent: options.source_parent.clone(),
        swarm_parent: options.swarm_parent.clone(),
        swarm_ref: options.swarm_ref.clone(),
        swarm_ref_sha: swarm
            .identity
            .immutable_ref_sha
            .clone()
            .ok_or_else(|| "swarm immutable ref was not resolved".to_string())?,
        source_main: source.resolved_main,
        swarm_main: swarm.resolved_main,
        merge_base,
        source_repository: source.identity,
        swarm_repository: swarm.identity,
        source_range,
        swarm_range,
        source_survivor_candidates: source_paths,
        swarm_only_paths,
        swarm_authority_resolution_candidates,
        dry_merge,
        version_state: VersionState {
            requested_version: options.version.clone(),
            source: version_side(&source_root, &options.source_parent, &options.version),
            swarm: version_side(&swarm_root, &options.swarm_parent, &options.version),
        },
        invalidation_rules: vec![
            "Any change to SOURCE_PARENT or the declared source main, SWARM_PARENT or the declared swarm main, or SWARM_REF resolution invalidates this receipt; regenerate rather than editing it.".to_string(),
            "A changed repository identity, merge base, ancestry count or order, ordered SHA digest, machine-readable conflict path list, or resolved tree requires a fresh receipt.".to_string(),
            "This receipt does not construct or prove the source join, version metadata, qualification, publication, or back-sync.".to_string(),
        ],
        next_commands: vec![
            "Review every dry-merge conflict and semantic overlap before constructing a join.".to_string(),
            "Pass this exact parent pair, validated SWARM_REF, and receipt to source preflight; do not substitute a branch name or later main.".to_string(),
        ],
    })
}

fn ensure_distinct_repositories(
    source_root: &Path,
    swarm_root: &Path,
    source_common_dir: &Path,
    swarm_common_dir: &Path,
) -> Result<(), String> {
    if source_root == swarm_root || source_common_dir == swarm_common_dir {
        return Err(
            "source and swarm must be distinct repositories; refusing an ambiguous identity"
                .to_string(),
        );
    }
    Ok(())
}

struct InspectedRepository {
    identity: RepositoryIdentity,
    common_dir: PathBuf,
    resolved_main: String,
}

fn inspect_repository(
    role: &str,
    repo: &Path,
    expected_remote: &str,
    parent: &str,
    main: &str,
    immutable_ref: Option<&str>,
) -> Result<InspectedRepository, String> {
    let root = repository_root(repo)?;
    let common_dir = git_common_dir(&root)?;
    let remote = git(&root, &["remote", "get-url", "origin"])?;
    let remote = remote.trim().to_string();
    if !remote_matches(&remote, expected_remote) {
        return Err(format!(
            "{role} repository origin `{remote}` does not match expected `{expected_remote}`"
        ));
    }
    let resolved_parent = exact_commit(&root, parent, role)?;
    let resolved_main = git(
        &root,
        &["rev-parse", "--verify", &format!("{main}^{{commit}}")],
    )?
    .trim()
    .to_string();
    if role == "source" && resolved_main != resolved_parent {
        return Err(format!(
            "source parent {parent} is not the declared current source main {resolved_main}; hold source main or repin"
        ));
    }
    if role == "swarm"
        && !git_status_ok(
            &root,
            &["merge-base", "--is-ancestor", parent, &resolved_main],
        )
    {
        return Err(format!(
            "swarm parent {parent} is not reachable from declared swarm main {main} ({resolved_main})"
        ));
    }
    let immutable_ref_sha = immutable_ref
        .map(|reference| exact_ref_commit(&root, reference, parent, role))
        .transpose()?;
    Ok(InspectedRepository {
        identity: RepositoryIdentity {
            role: role.to_string(),
            remote,
            expected_remote: expected_remote.to_string(),
            common_dir_verified: true,
            immutable_ref: immutable_ref.map(str::to_string),
            immutable_ref_sha: immutable_ref_sha.clone(),
            identity: "origin-remote-and-git-root-verified".to_string(),
            root_verified: true,
            remote_verified: true,
        },
        common_dir,
        resolved_main,
    })
}

fn repository_root(repo: &Path) -> Result<PathBuf, String> {
    let root = git(repo, &["rev-parse", "--show-toplevel"])?;
    let root = PathBuf::from(root.trim());
    if !root.is_dir() {
        return Err(format!(
            "git repository root does not exist: {}",
            root.display()
        ));
    }
    Ok(root)
}

fn git_common_dir(root: &Path) -> Result<PathBuf, String> {
    let raw = git(root, &["rev-parse", "--git-common-dir"])?;
    let path = PathBuf::from(raw.trim());
    let path = if path.is_absolute() {
        path
    } else {
        root.join(path)
    };
    path.canonicalize().map_err(|error| {
        format!(
            "failed to resolve git common directory {}: {error}",
            path.display()
        )
    })
}

fn exact_commit(repo: &Path, sha: &str, role: &str) -> Result<String, String> {
    let object_type = git(repo, &["cat-file", "-t", sha])
        .map_err(|error| format!("{role} parent {sha} does not resolve to an object: {error}"))?
        .trim()
        .to_string();
    if object_type != "commit" {
        return Err(format!(
            "{role} parent {sha} resolves to object type {object_type}, expected commit"
        ));
    }
    let resolved = git(
        repo,
        &["rev-parse", "--verify", &format!("{sha}^{{commit}}")],
    )?
    .trim()
    .to_string();
    if resolved != sha {
        return Err(format!(
            "{role} parent must resolve to the exact supplied SHA {sha}"
        ));
    }
    Ok(resolved)
}

fn exact_ref_commit(
    repo: &Path,
    reference: &str,
    expected: &str,
    role: &str,
) -> Result<String, String> {
    let ref_spec = format!("{reference}^{{commit}}");
    let resolved = git(repo, &["rev-parse", "--verify", &ref_spec])?
        .trim()
        .to_string();
    if resolved != expected {
        return Err(format!(
            "{role} immutable ref {reference:?} resolves to {resolved}, not exact parent {expected}"
        ));
    }
    Ok(resolved)
}

fn remote_matches(actual: &str, expected: &str) -> bool {
    canonical_remote(actual).is_some() && canonical_remote(actual) == canonical_remote(expected)
}

fn canonical_remote(value: &str) -> Option<String> {
    let mut value = value.trim().to_ascii_lowercase();
    let prefix = [
        "https://github.com/",
        "ssh://git@github.com/",
        "git@github.com:",
    ]
    .into_iter()
    .find(|prefix| value.starts_with(prefix));
    if let Some(prefix) = prefix {
        value = value[prefix.len()..].to_string();
    } else if value.contains("://") || value.contains('@') {
        return None;
    }
    value = value.trim_end_matches('/').to_string();
    if value.ends_with(".git") {
        value.truncate(value.len() - ".git".len());
    }
    let mut segments = value.split('/');
    let owner = segments.next()?.trim();
    let repository = segments.next()?.trim();
    if segments.next().is_some()
        || owner.is_empty()
        || repository.is_empty()
        || owner == "."
        || owner == ".."
        || repository == "."
        || repository == ".."
        || owner
            .chars()
            .any(|character| matches!(character, '?' | '#'))
        || repository
            .chars()
            .any(|character| matches!(character, '?' | '#'))
    {
        return None;
    }
    Some(format!("{owner}/{repository}"))
}

fn commit_range(repo: &Path, base: &str, head: &str) -> Result<CommitRange, String> {
    let all = lines(git(
        repo,
        &[
            "rev-list",
            "--topo-order",
            "--reverse",
            &format!("{base}..{head}"),
        ],
    )?);
    let first = lines(git(
        repo,
        &[
            "rev-list",
            "--first-parent",
            "--reverse",
            &format!("{base}..{head}"),
        ],
    )?);
    let first_forward = lines(git(
        repo,
        &["rev-list", "--first-parent", &format!("{base}..{head}")],
    )?);
    Ok(CommitRange {
        all_reachable_count: all.len(),
        first_parent_count: first.len(),
        all_reachable_sha256: digest_lines(&all),
        first_parent_ordered_sha256: digest_lines(&first),
        all_reachable_ordered_recipe:
            "git rev-list --topo-order --reverse MERGE_BASE..PARENT; UTF-8 SHA lines joined with LF; SHA-256".to_string(),
        first_parent_ordered_recipe: "git rev-list --first-parent --reverse MERGE_BASE..PARENT; UTF-8 SHA lines joined with LF; SHA-256".to_string(),
    })
    .and_then(|range| {
        if first.iter().rev().ne(first_forward.iter()) {
            Err("git produced inconsistent first-parent reverse ordering".to_string())
        } else {
            Ok(range)
        }
    })
}

fn digest_lines(lines: &[String]) -> String {
    let mut hasher = Sha256::new();
    for line in lines {
        hasher.update(line.as_bytes());
        hasher.update([b'\n']);
    }
    format!("sha256:{:x}", hasher.finalize())
}

fn changed_paths(repo: &Path, base: &str, head: &str) -> Result<Vec<String>, String> {
    let mut paths = lines(git(
        repo,
        &["diff", "--name-only", &format!("{base}..{head}")],
    )?);
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn is_swarm_authority_path(path: &str) -> bool {
    path.starts_with(".github/")
        || path == ".github/settings.yml"
        || path.starts_with("policy/")
        || path.starts_with(".ripr/")
}

fn with_disposable_repo<T>(
    source: &Path,
    swarm: &Path,
    source_parent: &str,
    swarm_parent: &str,
    action: impl FnOnce(&Path) -> Result<T, String>,
) -> Result<T, String> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock before epoch: {error}"))?
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "ripr-source-promotion-{nonce}-{}",
        std::process::id()
    ));
    create_disposable_repo_dir(&path)?;
    let result = (|| {
        git(&path, &["init", "--quiet"])?;
        git(
            &path,
            &[
                "fetch",
                "--no-tags",
                source.to_string_lossy().as_ref(),
                source_parent,
            ],
        )?;
        git(
            &path,
            &[
                "fetch",
                "--no-tags",
                swarm.to_string_lossy().as_ref(),
                swarm_parent,
            ],
        )?;
        action(&path)
    })();
    let cleanup = fs::remove_dir_all(&path).map_err(|error| {
        format!(
            "failed to remove disposable merge repository {}: {error}",
            path.display()
        )
    });
    match (result, cleanup) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(error),
        (Err(action_error), Err(cleanup_error)) => {
            Err(format!("{action_error}; additionally, {cleanup_error}"))
        }
    }
}

fn create_disposable_repo_dir(path: &Path) -> Result<(), String> {
    fs::create_dir(path).map_err(|error| format!("failed to create disposable merge repo: {error}"))
}

fn dry_merge_from_repo(
    repo: &Path,
    source_parent: &str,
    swarm_parent: &str,
    reviewed_resolved_tree: Option<&str>,
    reviewed_resolved_tree_verified: bool,
) -> Result<DryMerge, String> {
    ensure_merge_tree_capability(repo)?;
    let output = Command::new("git")
        .current_dir(repo)
        .args([
            "merge-tree",
            "--write-tree",
            "--name-only",
            "-z",
            source_parent,
            swarm_parent,
        ])
        .output()
        .map_err(|error| format!("failed to execute disposable git merge-tree: {error}"))?;
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() && output.stdout.is_empty() && stderr.trim().is_empty() {
        return Err(format!(
            "disposable git merge-tree failed ({})",
            output.status
        ));
    }
    let (preview_tree, conflicts) = parse_merge_tree_output(&output.stdout)?;
    let diagnostics = lines(stderr);
    if !output.status.success() && conflicts.is_empty() {
        return Err(format!(
            "disposable git merge-tree failed ({}): {}",
            output.status,
            diagnostics.join(" | ")
        ));
    }
    let preview_tree = (preview_tree.is_ascii()
        && preview_tree.len() == 40
        && preview_tree.chars().all(|c| c.is_ascii_hexdigit()))
    .then_some(preview_tree);
    if output.status.success() && preview_tree.is_none() {
        return Err(format!(
            "disposable git merge-tree returned success without a merged tree: {}",
            diagnostics.join(" | ")
        ));
    }
    Ok(DryMerge {
        status: if conflicts.is_empty() {
            "clean"
        } else {
            "conflicts"
        }
        .to_string(),
        preview_tree,
        reviewed_resolved_tree: reviewed_resolved_tree.map(str::to_string),
        reviewed_resolved_tree_verified,
        conflicts,
        diagnostics,
    })
}

fn parse_merge_tree_output(stdout: &[u8]) -> Result<(String, Vec<String>), String> {
    let mut fields = stdout.split(|byte| *byte == 0);
    let tree = fields
        .next()
        .filter(|field| !field.is_empty())
        .ok_or_else(|| "disposable git merge-tree returned no tree field".to_string())?;
    let tree = std::str::from_utf8(tree)
        .map_err(|error| format!("disposable git merge-tree returned a non-UTF-8 tree: {error}"))?
        .trim()
        .to_string();
    let mut conflicts = Vec::new();
    for field in fields.take_while(|field| !field.is_empty()) {
        conflicts.push(
            std::str::from_utf8(field)
                .map(str::to_string)
                .map_err(|error| {
                    format!("disposable git merge-tree returned a non-UTF-8 path: {error}")
                })?,
        );
    }
    conflicts.sort();
    conflicts.dedup();
    Ok((tree, conflicts))
}

fn ensure_merge_tree_capability(repo: &Path) -> Result<(), String> {
    let version = git(repo, &["version"])?;
    if !git_version_at_least(&version, 2, 38) {
        return Err(format!(
            "git merge-tree --write-tree requires Git >= 2.38; observed {version:?}"
        ));
    }
    Ok(())
}

fn git_version_at_least(output: &str, required_major: u32, required_minor: u32) -> bool {
    let Some(version) = output.trim().strip_prefix("git version ") else {
        return false;
    };
    let mut parts = version.split('.');
    let Some(major) = parts.next().and_then(|value| value.parse::<u32>().ok()) else {
        return false;
    };
    let Some(minor) = parts.next().and_then(|value| value.parse::<u32>().ok()) else {
        return false;
    };
    (major, minor) >= (required_major, required_minor)
}

fn verify_tree_object(repo: &Path, tree: &str) -> Result<bool, String> {
    let output = Command::new("git")
        .current_dir(repo)
        .args(["cat-file", "-t", tree])
        .output()
        .map_err(|error| format!("failed to inspect resolved tree {tree}: {error}"))?;
    if !output.status.success() {
        return Ok(false);
    }
    let object_type = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if object_type != "tree" {
        return Ok(false);
    }
    Ok(true)
}

fn verify_tree_in_repositories(source: &Path, swarm: &Path, tree: &str) -> Result<(), String> {
    if verify_tree_object(source, tree)? || verify_tree_object(swarm, tree)? {
        Ok(())
    } else {
        Err(format!(
            "resolved tree {tree} is not a tree object in either supplied repository object store"
        ))
    }
}

struct GitOutput {
    stdout: String,
}

fn run_git(repo: &Path, args: &[&str]) -> Result<GitOutput, String> {
    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .map_err(|error| format!("failed to execute git {}: {error}", args.join(" ")))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() {
        return Err(format!(
            "git {} failed ({}): {}",
            args.join(" "),
            output.status,
            stderr.trim()
        ));
    }
    Ok(GitOutput { stdout })
}

fn git(repo: &Path, args: &[&str]) -> Result<String, String> {
    Ok(run_git(repo, args)?.stdout)
}

fn git_status_ok(repo: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .current_dir(repo)
        .args(args)
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn lines(text: String) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

fn version_side(root: &Path, parent: &str, version: &str) -> VersionSide {
    let read = |relative: &str| git_show(root, parent, relative);
    let cargo = read("Cargo.toml");
    let crate_manifest = read("crates/ripr/Cargo.toml");
    let package = read("editors/vscode/package.json");
    let cargo_lock = read("Cargo.lock");
    let npm_lock = read("editors/vscode/package-lock.json");
    let changelog = read("CHANGELOG.md");
    let (workspace_version, crate_version) =
        cargo_version_surfaces(cargo.as_deref(), crate_manifest.as_deref());
    let extension_version = package.and_then(|body| {
        body.lines().find_map(|line| {
            let line = line.trim();
            line.strip_prefix("\"version\": \"")?
                .split('"')
                .next()
                .map(str::to_string)
        })
    });
    let cargo_lock_package_version =
        cargo_lock.and_then(|body| lock_package_version(&body, "ripr"));
    let npm_lock_root_version = npm_lock.and_then(|body| {
        serde_json::from_str::<serde_json::Value>(&body)
            .ok()?
            .get("packages")?
            .get("")?
            .get("version")?
            .as_str()
            .map(str::to_string)
    });
    VersionSide {
        workspace_version,
        crate_version,
        extension_version,
        cargo_lock_package_version,
        npm_lock_root_version,
        changelog_mentions_version: changelog.map(|body| {
            body.contains(&format!("[{version}]")) || body.contains(&format!("## {version}"))
        }),
    }
}

fn cargo_version_surfaces(
    workspace_manifest: Option<&str>,
    crate_manifest: Option<&str>,
) -> (Option<String>, Option<String>) {
    let workspace_value =
        workspace_manifest.and_then(|body| toml::from_str::<toml::Value>(body).ok());
    let workspace_version = workspace_value
        .as_ref()
        .and_then(|value| value.get("workspace"))
        .and_then(|value| value.get("package"))
        .and_then(|value| value.get("version"))
        .and_then(toml::Value::as_str)
        .map(str::to_string);
    let crate_value = crate_manifest.and_then(|body| toml::from_str::<toml::Value>(body).ok());
    let package = crate_value.as_ref().and_then(|value| value.get("package"));
    let crate_version = package
        .and_then(|value| value.get("version"))
        .and_then(toml::Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            package
                .and_then(|value| value.get("version"))
                .and_then(|value| value.get("workspace"))
                .and_then(toml::Value::as_bool)
                .and_then(|workspace| workspace.then(|| workspace_version.clone()))
                .flatten()
        });
    (workspace_version, crate_version)
}

fn lock_package_version(text: &str, package_name: &str) -> Option<String> {
    let mut in_package = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == "[[package]]" {
            in_package = false;
            continue;
        }
        if trimmed == format!("name = \"{package_name}\"") {
            in_package = true;
            continue;
        }
        if in_package {
            if let Some(version) = trimmed
                .strip_prefix("version = \"")
                .and_then(|value| value.strip_suffix('"'))
            {
                return Some(version.to_string());
            }
            if trimmed.starts_with("[[") {
                in_package = false;
            }
        }
    }
    None
}

fn git_show(root: &Path, commit: &str, path: &str) -> Option<String> {
    let output = Command::new("git")
        .current_dir(root)
        .args(["show", &format!("{commit}:{path}")])
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        None
    }
}

fn render_markdown(receipt: &Receipt) -> String {
    let list = |items: &[String]| {
        if items.is_empty() {
            "- none\n".to_string()
        } else {
            items.iter().map(|item| format!("- `{item}`\n")).collect()
        }
    };
    let schema = &receipt.schema;
    let mode = &receipt.mode;
    let source_parent = &receipt.source_parent;
    let swarm_parent = &receipt.swarm_parent;
    let swarm_ref = &receipt.swarm_ref;
    let swarm_ref_sha = &receipt.swarm_ref_sha;
    let merge_base = &receipt.merge_base;
    let source_remote = &receipt.source_repository.remote;
    let source_expected_remote = &receipt.source_repository.expected_remote;
    let source_common_dir_verified = receipt.source_repository.common_dir_verified;
    let swarm_remote = &receipt.swarm_repository.remote;
    let swarm_expected_remote = &receipt.swarm_repository.expected_remote;
    let swarm_common_dir_verified = receipt.swarm_repository.common_dir_verified;
    let source_all_count = receipt.source_range.all_reachable_count;
    let source_first_count = receipt.source_range.first_parent_count;
    let source_all_digest = &receipt.source_range.all_reachable_sha256;
    let source_first_digest = &receipt.source_range.first_parent_ordered_sha256;
    let swarm_all_count = receipt.swarm_range.all_reachable_count;
    let swarm_first_count = receipt.swarm_range.first_parent_count;
    let swarm_all_digest = &receipt.swarm_range.all_reachable_sha256;
    let swarm_first_digest = &receipt.swarm_range.first_parent_ordered_sha256;
    let all_recipe = &receipt.source_range.all_reachable_ordered_recipe;
    let first_recipe = &receipt.source_range.first_parent_ordered_recipe;
    let dry_status = &receipt.dry_merge.status;
    let preview_tree = &receipt.dry_merge.preview_tree;
    let reviewed_tree = &receipt.dry_merge.reviewed_resolved_tree;
    let reviewed_tree_verified = receipt.dry_merge.reviewed_resolved_tree_verified;
    let conflicts = list(&receipt.dry_merge.conflicts);
    let source_survivors = list(&receipt.source_survivor_candidates);
    let swarm_only = list(&receipt.swarm_only_paths);
    let authority_candidates = list(&receipt.swarm_authority_resolution_candidates);
    let requested_version = &receipt.version_state.requested_version;
    let source_workspace = &receipt.version_state.source.workspace_version;
    let source_crate = &receipt.version_state.source.crate_version;
    let source_extension = &receipt.version_state.source.extension_version;
    let source_cargo_lock = &receipt.version_state.source.cargo_lock_package_version;
    let source_npm_lock = &receipt.version_state.source.npm_lock_root_version;
    let source_changelog = &receipt.version_state.source.changelog_mentions_version;
    let swarm_workspace = &receipt.version_state.swarm.workspace_version;
    let swarm_crate = &receipt.version_state.swarm.crate_version;
    let swarm_extension = &receipt.version_state.swarm.extension_version;
    let swarm_cargo_lock = &receipt.version_state.swarm.cargo_lock_package_version;
    let swarm_npm_lock = &receipt.version_state.swarm.npm_lock_root_version;
    let swarm_changelog = &receipt.version_state.swarm.changelog_mentions_version;
    let invalidation_rules = list(&receipt.invalidation_rules);
    let next_commands = list(&receipt.next_commands);
    format!(
        "# Source-promotion preflight\n\n- Schema: {schema}\n- Mode: {mode}\n- SOURCE_PARENT: {source_parent}\n- SWARM_PARENT: {swarm_parent}\n- SWARM_REF: {swarm_ref}\n- SWARM_REF_SHA: {swarm_ref_sha}\n- MERGE_BASE: {merge_base}\n\n## Repository identity\n\n- source: {source_remote} (expected {source_expected_remote}; common dirs distinct={source_common_dir_verified})\n- swarm: {swarm_remote} (expected {swarm_expected_remote}; common dirs distinct={swarm_common_dir_verified})\n\n## Ancestry\n\n| range | all reachable | first parent | all digest | ordered first-parent digest |\n| --- | ---: | ---: | --- | --- |\n| source | {source_all_count} | {source_first_count} | {source_all_digest} | {source_first_digest} |\n| swarm | {swarm_all_count} | {swarm_first_count} | {swarm_all_digest} | {swarm_first_digest} |\n\nAll-reachable digest recipe: {all_recipe}\nFirst-parent digest recipe: {first_recipe}\n\n## Dry merge\n\nStatus: **{dry_status}**\nPreview tree (automatic, non-final): {preview_tree:?}\nReviewed resolved tree: {reviewed_tree:?}\nReviewed resolved tree verified in supplied repository object store: {reviewed_tree_verified}\n\nConflicts:\n{conflicts}\n\n## Source survivors\n\n{source_survivors}\n## Swarm-only paths\n\n{swarm_only}\n## Swarm authority-resolution candidates (non-dispositive)\n\n{authority_candidates}\n## Version state\n\n- requested: {requested_version}\n- source: workspace={source_workspace:?}, crate={source_crate:?}, extension={source_extension:?}, cargo-lock-ripr={source_cargo_lock:?}, npm-lock-root={source_npm_lock:?}, changelog-mentions={source_changelog:?}\n- swarm: workspace={swarm_workspace:?}, crate={swarm_crate:?}, extension={swarm_extension:?}, cargo-lock-ripr={swarm_cargo_lock:?}, npm-lock-root={swarm_npm_lock:?}, changelog-mentions={swarm_changelog:?}\n\n## Invalidation rules\n\n{invalidation_rules}\n## Next commands\n\n{next_commands}",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn rejects_abbreviated_parent() -> Result<(), String> {
        let result = parse_args(&[
            "preflight".to_string(),
            "--source-parent".to_string(),
            "deadbeef".to_string(),
        ]);
        if result.is_ok() {
            return Err("abbreviated SHA unexpectedly accepted".to_string());
        }
        Ok(())
    }

    #[test]
    fn sha_validation_names_the_required_object_kind() -> Result<(), String> {
        let commit_error = validate_sha("--source-parent", "commit", "deadbeef")
            .err()
            .ok_or_else(|| "abbreviated commit SHA unexpectedly accepted".to_string())?;
        let tree_error = validate_sha("--resolved-tree", "tree", "deadbeef")
            .err()
            .ok_or_else(|| "abbreviated tree SHA unexpectedly accepted".to_string())?;
        if !commit_error.contains("commit SHA") || !tree_error.contains("tree SHA") {
            return Err(format!(
                "SHA validation errors omitted object kinds: {commit_error:?}, {tree_error:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn merge_tree_z_output_preserves_conflict_paths() -> Result<(), String> {
        let tree = "0123456789abcdef0123456789abcdef01234567";
        let output = format!(
            "{tree}\0shared.txt\0nested/conflict.rs\0shared.txt\0 leading-and-trailing \0\01\0shared.txt\0Auto-merging\0"
        );
        let (parsed_tree, paths) = parse_merge_tree_output(output.as_bytes())?;
        if parsed_tree != tree
            || paths != [" leading-and-trailing ", "nested/conflict.rs", "shared.txt"]
        {
            return Err(format!(
                "NUL-delimited merge-tree output was misparsed: tree={parsed_tree:?}, paths={paths:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn requires_an_immutable_swarm_ref() -> Result<(), String> {
        let args = vec![
            "preflight".to_string(),
            "--source-parent".to_string(),
            "0000000000000000000000000000000000000000".to_string(),
            "--swarm-parent".to_string(),
            "1111111111111111111111111111111111111111".to_string(),
        ];
        let error = parse_args(&args)
            .err()
            .ok_or_else(|| "missing swarm ref was unexpectedly accepted".to_string())?;
        if !error.contains("--swarm-ref") {
            return Err(format!("missing-ref error omitted --swarm-ref: {error}"));
        }
        Ok(())
    }

    #[test]
    fn rejects_a_branch_or_wrongly_named_swarm_ref() -> Result<(), String> {
        let parent = "1111111111111111111111111111111111111111";
        for reference in ["main", "refs/heads/main", "refs/ripr/release-0.11.0-wrong"] {
            if validate_swarm_ref(reference, "0.11.0", parent).is_ok() {
                return Err(format!(
                    "non-governed immutable ref was accepted: {reference}"
                ));
            }
        }
        validate_swarm_ref(
            "refs/ripr/release-0.11.0-1111111111111111111111111111111111111111",
            "0.11.0",
            parent,
        )
    }

    #[test]
    fn digest_recipe_is_order_sensitive() -> Result<(), String> {
        let left = digest_lines(&["a".to_string(), "b".to_string()]);
        let right = digest_lines(&["b".to_string(), "a".to_string()]);
        if left == right {
            return Err("ordered digest ignored input order".to_string());
        }
        Ok(())
    }

    #[test]
    fn git_version_gate_is_fail_closed() -> Result<(), String> {
        if !git_version_at_least("git version 2.38.0.windows.1", 2, 38)
            || !git_version_at_least("git version 3.0.0", 2, 38)
        {
            return Err("supported Git versions were rejected".to_string());
        }
        for version in ["git version 2.37.9", "Git version 2.54.0", "git unknown"] {
            if git_version_at_least(version, 2, 38) {
                return Err(format!(
                    "unsupported or malformed Git version accepted: {version}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn version_observation_reads_exact_parent_not_checkout_files() -> Result<(), String> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("system clock before epoch: {error}"))?
            .as_nanos();
        let root = std::env::temp_dir().join(format!("ripr-version-parent-{nonce}"));
        fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        test_git(&root, &["init", "--quiet"])?;
        test_git(&root, &["config", "user.email", "test@example.invalid"])?;
        test_git(&root, &["config", "user.name", "test"])?;
        fs::write(
            root.join("Cargo.toml"),
            "[workspace.package]\nversion = \"0.10.1\"\n",
        )
        .map_err(|error| error.to_string())?;
        fs::write(
            root.join("Cargo.lock"),
            "[[package]]\nname = \"ripr\"\nversion = \"0.10.1\"\n",
        )
        .map_err(|error| error.to_string())?;
        fs::create_dir_all(root.join("editors/vscode")).map_err(|error| error.to_string())?;
        fs::write(
            root.join("editors/vscode/package.json"),
            "{\n  \"version\": \"0.10.1\"\n}\n",
        )
        .map_err(|error| error.to_string())?;
        fs::write(
            root.join("editors/vscode/package-lock.json"),
            "{\n  \"packages\": {\"\": {\"version\": \"0.10.1\"}}\n}\n",
        )
        .map_err(|error| error.to_string())?;
        test_git(&root, &["add", "Cargo.toml"])?;
        test_git(&root, &["add", "Cargo.lock", "editors/vscode"])?;
        test_git(&root, &["commit", "--quiet", "-m", "version"])?;
        let parent = test_git_output(&root, &["rev-parse", "HEAD"])?;
        fs::write(
            root.join("Cargo.toml"),
            "[workspace.package]\nversion = \"9.9.9\"\n",
        )
        .map_err(|error| error.to_string())?;
        let observed = version_side(&root, &parent, "0.11.0");
        if observed.workspace_version.as_deref() != Some("0.10.1") {
            return Err(format!(
                "version observation used checkout state: {observed:?}"
            ));
        }
        if observed.cargo_lock_package_version.as_deref() != Some("0.10.1")
            || observed.npm_lock_root_version.as_deref() != Some("0.10.1")
        {
            return Err(format!(
                "lockfile version observations were not read from parent: {observed:?}"
            ));
        }
        if observed.changelog_mentions_version.is_some() {
            return Err(format!(
                "missing changelog was reported as an observed boolean: {observed:?}"
            ));
        }
        fs::remove_dir_all(&root).map_err(|error| format!("cleanup failed: {error}"))?;
        Ok(())
    }

    #[test]
    fn inherited_crate_version_resolves_workspace_authority() -> Result<(), String> {
        let (workspace, crate_version) = cargo_version_surfaces(
            Some("[workspace.package]\nversion = \"0.10.1\"\n"),
            Some("[package]\nversion.workspace = true\n"),
        );
        if workspace.as_deref() != Some("0.10.1") || crate_version.as_deref() != Some("0.10.1") {
            return Err(format!(
                "workspace-inherited crate version was not resolved: {workspace:?}, {crate_version:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn repository_identity_serialization_has_no_checkout_path() -> Result<(), String> {
        let identity = RepositoryIdentity {
            role: "source".to_string(),
            remote: "https://github.com/EffortlessMetrics/ripr.git".to_string(),
            expected_remote: "EffortlessMetrics/ripr".to_string(),
            common_dir_verified: true,
            immutable_ref: None,
            immutable_ref_sha: None,
            identity: "origin-remote-and-git-root-verified".to_string(),
            root_verified: true,
            remote_verified: true,
        };
        let json = serde_json::to_string(&identity)
            .map_err(|error| format!("failed to serialize identity: {error}"))?;
        if !json.contains("common_dir_verified") || json.contains("git_common_dir") {
            return Err(format!(
                "identity serialization is not location-independent: {json}"
            ));
        }
        Ok(())
    }

    #[test]
    fn source_parent_must_equal_declared_source_main() -> Result<(), String> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("system clock before epoch: {error}"))?
            .as_nanos();
        let root = std::env::temp_dir().join(format!("ripr-source-main-{nonce}"));
        fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        test_git(&root, &["init", "--quiet"])?;
        test_git(&root, &["config", "user.email", "test@example.invalid"])?;
        test_git(&root, &["config", "user.name", "test"])?;
        test_git(
            &root,
            &[
                "remote",
                "add",
                "origin",
                "https://github.com/EffortlessMetrics/ripr.git",
            ],
        )?;
        fs::write(root.join("file"), "one\n").map_err(|error| error.to_string())?;
        test_git(&root, &["add", "file"])?;
        test_git(&root, &["commit", "--quiet", "-m", "one"])?;
        let parent = test_git_output(&root, &["rev-parse", "HEAD"])?;
        fs::write(root.join("file"), "two\n").map_err(|error| error.to_string())?;
        test_git(&root, &["commit", "--quiet", "-am", "two"])?;
        let error = inspect_repository(
            "source",
            &root,
            "EffortlessMetrics/ripr",
            &parent,
            "HEAD",
            None,
        )
        .err()
        .ok_or_else(|| "stale source parent was unexpectedly accepted".to_string())?;
        if !error.contains("not the declared current source main") {
            return Err(format!("unexpected source-main error: {error}"));
        }
        fs::remove_dir_all(&root).map_err(|error| format!("cleanup failed: {error}"))?;
        Ok(())
    }

    #[test]
    fn parent_sha_rejects_non_commit_object_with_kind() -> Result<(), String> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("system clock before epoch: {error}"))?
            .as_nanos();
        let root = std::env::temp_dir().join(format!("ripr-object-kind-{nonce}"));
        fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        test_git(&root, &["init", "--quiet"])?;
        fs::write(root.join("blob"), "not a commit\n").map_err(|error| error.to_string())?;
        let blob = test_git_output(&root, &["hash-object", "-w", "blob"])?;
        let error = exact_commit(&root, &blob, "source")
            .err()
            .ok_or_else(|| "blob object was unexpectedly accepted as a parent".to_string())?;
        if !error.contains("object type blob") || !error.contains("expected commit") {
            return Err(format!("non-commit kind was not reported: {error}"));
        }
        fs::remove_dir_all(&root).map_err(|error| format!("cleanup failed: {error}"))?;
        Ok(())
    }

    #[test]
    fn shared_common_dir_is_rejected() -> Result<(), String> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("system clock before epoch: {error}"))?
            .as_nanos();
        let root = std::env::temp_dir().join(format!("ripr-common-dir-{nonce}"));
        let worktree = root.join("worktree");
        fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        test_git(&root, &["init", "--quiet"])?;
        test_git(&root, &["config", "user.email", "test@example.invalid"])?;
        test_git(&root, &["config", "user.name", "test"])?;
        test_git(
            &root,
            &[
                "remote",
                "add",
                "origin",
                "https://github.com/EffortlessMetrics/ripr.git",
            ],
        )?;
        fs::write(root.join("file"), "one\n").map_err(|error| error.to_string())?;
        test_git(&root, &["add", "file"])?;
        test_git(&root, &["commit", "--quiet", "-m", "one"])?;
        let parent = test_git_output(&root, &["rev-parse", "HEAD"])?;
        test_git(
            &root,
            &[
                "worktree",
                "add",
                "--quiet",
                worktree.to_string_lossy().as_ref(),
                &parent,
            ],
        )?;
        if git_common_dir(&root)? != git_common_dir(&worktree)? {
            return Err("linked worktree did not share the common directory".to_string());
        }
        let error = ensure_distinct_repositories(
            &repository_root(&root)?,
            &repository_root(&worktree)?,
            &git_common_dir(&root)?,
            &git_common_dir(&worktree)?,
        )
        .err()
        .ok_or_else(|| "shared common directory was unexpectedly accepted".to_string())?;
        if !error.contains("distinct repositories") {
            return Err(format!("unexpected common-dir error: {error}"));
        }
        test_git(
            &root,
            &[
                "worktree",
                "remove",
                "--force",
                worktree.to_string_lossy().as_ref(),
            ],
        )?;
        fs::remove_dir_all(&root).map_err(|error| format!("cleanup failed: {error}"))?;
        Ok(())
    }

    #[test]
    fn disposable_repo_creation_rejects_existing_path() -> Result<(), String> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("system clock before epoch: {error}"))?
            .as_nanos();
        let root = std::env::temp_dir().join(format!("ripr-disposable-collision-{nonce}"));
        fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        let error = create_disposable_repo_dir(&root)
            .err()
            .ok_or_else(|| "existing disposable path was unexpectedly reused".to_string())?;
        if !error.contains("failed to create disposable merge repo") {
            return Err(format!("unexpected collision error: {error}"));
        }
        fs::remove_dir_all(&root).map_err(|error| format!("cleanup failed: {error}"))?;
        Ok(())
    }

    #[test]
    fn swarm_parent_must_be_ancestor_of_declared_main() -> Result<(), String> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("system clock before epoch: {error}"))?
            .as_nanos();
        let root = std::env::temp_dir().join(format!("ripr-swarm-ancestry-{nonce}"));
        fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        test_git(&root, &["init", "--quiet"])?;
        test_git(&root, &["config", "user.email", "test@example.invalid"])?;
        test_git(&root, &["config", "user.name", "test"])?;
        test_git(
            &root,
            &[
                "remote",
                "add",
                "origin",
                "https://github.com/EffortlessMetrics/ripr-swarm.git",
            ],
        )?;
        fs::write(root.join("base.txt"), "base\n").map_err(|error| error.to_string())?;
        test_git(&root, &["add", "base.txt"])?;
        test_git(&root, &["commit", "--quiet", "-m", "base"])?;
        let parent = test_git_output(&root, &["rev-parse", "HEAD"])?;
        test_git(&root, &["checkout", "--quiet", "--orphan", "unrelated"])?;
        fs::remove_file(root.join("base.txt")).map_err(|error| error.to_string())?;
        fs::write(root.join("unrelated.txt"), "unrelated\n").map_err(|error| error.to_string())?;
        test_git(&root, &["add", "--all"])?;
        test_git(&root, &["commit", "--quiet", "-m", "unrelated"])?;
        test_git(&root, &["branch", "-M", "main"])?;
        let error = inspect_repository(
            "swarm",
            &root,
            "EffortlessMetrics/ripr-swarm",
            &parent,
            "main",
            None,
        )
        .err()
        .ok_or_else(|| "unrelated swarm main was unexpectedly accepted".to_string())?;
        if !error.contains("not reachable") {
            return Err(format!("unexpected ancestry error: {error}"));
        }
        fs::remove_dir_all(&root).map_err(|error| format!("cleanup failed: {error}"))?;
        Ok(())
    }

    #[test]
    fn fixture_points_to_source_promotion_spec() -> Result<(), String> {
        let text = fs::read_to_string("../fixtures/source_promotion/SPEC.md")
            .map_err(|error| format!("failed to read source-promotion fixture: {error}"))?;
        if !text
            .lines()
            .any(|line| line.trim() == "Spec: RIPR-SPEC-0148")
        {
            return Err("fixture does not reference RIPR-SPEC-0148".to_string());
        }
        Ok(())
    }

    #[test]
    fn moved_or_missing_swarm_ref_is_rejected() -> Result<(), String> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("system clock before epoch: {error}"))?
            .as_nanos();
        let root = std::env::temp_dir().join(format!("ripr-swarm-ref-{nonce}"));
        fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        test_git(&root, &["init", "--quiet"])?;
        test_git(&root, &["config", "user.email", "test@example.invalid"])?;
        test_git(&root, &["config", "user.name", "test"])?;
        fs::write(root.join("file"), "one\n").map_err(|error| error.to_string())?;
        test_git(&root, &["add", "file"])?;
        test_git(&root, &["commit", "--quiet", "-m", "one"])?;
        let first = test_git_output(&root, &["rev-parse", "HEAD"])?;
        let reference = "refs/releases/test";
        test_git(&root, &["update-ref", reference, &first])?;
        fs::write(root.join("file"), "two\n").map_err(|error| error.to_string())?;
        test_git(&root, &["commit", "--quiet", "-am", "two"])?;
        let second = test_git_output(&root, &["rev-parse", "HEAD"])?;
        test_git(&root, &["update-ref", reference, &second])?;
        let moved = exact_ref_commit(&root, reference, &first, "swarm")
            .err()
            .ok_or_else(|| "moved immutable ref was unexpectedly accepted".to_string())?;
        if !moved.contains("not exact parent") {
            return Err(format!("unexpected moved-ref error: {moved}"));
        }
        if exact_ref_commit(&root, "refs/releases/missing", &first, "swarm").is_ok() {
            return Err("missing immutable ref was unexpectedly accepted".to_string());
        }
        fs::remove_dir_all(&root).map_err(|error| format!("cleanup failed: {error}"))?;
        Ok(())
    }

    #[test]
    fn remote_identity_normalizes_git_urls() -> Result<(), String> {
        if !remote_matches(
            "git@github.com:EffortlessMetrics/ripr-swarm.git",
            "EffortlessMetrics/ripr-swarm",
        ) {
            return Err("SSH remote was not normalized".to_string());
        }
        if remote_matches(
            "https://github.com/other/ripr.git",
            "EffortlessMetrics/ripr",
        ) {
            return Err("wrong remote was accepted".to_string());
        }
        for (actual, expected) in [
            (
                "https://github.com/evil/effortlessmetrics/ripr.git",
                "EffortlessMetrics/ripr",
            ),
            (
                "https://github.com/EffortlessMetrics/ripr-evil.git",
                "EffortlessMetrics/ripr",
            ),
            (
                "https://github.com/EffortlessMetrics/ripr?redirect=evil",
                "EffortlessMetrics/ripr",
            ),
        ] {
            if remote_matches(actual, expected) {
                return Err(format!("remote suffix trick was accepted: {actual}"));
            }
        }
        Ok(())
    }

    #[test]
    fn remote_identity_accepts_supported_windows_safe_forms() -> Result<(), String> {
        for actual in [
            "https://github.com/EffortlessMetrics/ripr-swarm.git",
            "ssh://git@github.com/EffortlessMetrics/ripr-swarm",
            "git@github.com:EffortlessMetrics/ripr-swarm.git",
        ] {
            if !remote_matches(actual, "EffortlessMetrics/ripr-swarm") {
                return Err(format!("supported remote form was rejected: {actual}"));
            }
        }
        Ok(())
    }

    #[test]
    fn authority_path_filter_is_narrow_and_explicit() -> Result<(), String> {
        if !is_swarm_authority_path(".github/settings.yml")
            || !is_swarm_authority_path("policy/process_allowlist.txt")
        {
            return Err("authority paths were not classified".to_string());
        }
        if is_swarm_authority_path("src/lib.rs") {
            return Err("product path was misclassified".to_string());
        }
        Ok(())
    }

    #[test]
    fn fixture_receipt_contract_is_discriminating() -> Result<(), String> {
        let text = fs::read_to_string("../fixtures/source_promotion/diverged-conflict.json")
            .map_err(|error| format!("failed to read source-promotion fixture: {error}"))?;
        let value: serde_json::Value = serde_json::from_str(&text)
            .map_err(|error| format!("failed to parse source-promotion fixture: {error}"))?;
        if value["expected"]["mode"] != "two_parent_join"
            || value["expected"]["dry_merge_status"] != "conflicts"
            || value["expected"]["conflict_paths"][0] != "shared.txt"
            || !value["expected"]["swarm_only_paths"]
                .as_array()
                .is_some_and(Vec::is_empty)
            || value["expected"]["source_first_parent_count"] != 1
            || value["expected"]["swarm_first_parent_count"] != 1
        {
            return Err(
                "fixture does not pin a divergent, conflicting two-parent case".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn disposable_preflight_keeps_authoritative_checkouts_unchanged() -> Result<(), String> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("system clock before epoch: {error}"))?
            .as_nanos();
        let root = std::env::temp_dir().join(format!("ripr-preflight-fixture-{nonce}"));
        let source = root.join("source");
        let swarm = root.join("swarm");
        fs::create_dir_all(&source).map_err(|error| error.to_string())?;
        test_git(&source, &["init", "--quiet"])?;
        test_git(&source, &["config", "user.email", "test@example.invalid"])?;
        test_git(&source, &["config", "user.name", "test"])?;
        fs::write(source.join("shared.txt"), "base\n").map_err(|error| error.to_string())?;
        test_git(&source, &["add", "shared.txt"])?;
        test_git(&source, &["commit", "--quiet", "-m", "base"])?;
        let base = test_git_output(&source, &["rev-parse", "HEAD"])?;
        fs::write(source.join("shared.txt"), "source\n").map_err(|error| error.to_string())?;
        test_git(&source, &["commit", "--quiet", "-am", "source"])?;
        let source_parent = test_git_output(&source, &["rev-parse", "HEAD"])?;
        test_git(
            &source,
            &[
                "remote",
                "add",
                "origin",
                "https://github.com/EffortlessMetrics/ripr.git",
            ],
        )?;
        test_git(&source, &["clone", ".", swarm.to_string_lossy().as_ref()])?;
        test_git(&swarm, &["config", "user.email", "test@example.invalid"])?;
        test_git(&swarm, &["config", "user.name", "test"])?;
        test_git(&swarm, &["checkout", "--quiet", "-B", "main", &base])?;
        fs::write(swarm.join("shared.txt"), "swarm\n").map_err(|error| error.to_string())?;
        test_git(&swarm, &["commit", "--quiet", "-am", "swarm"])?;
        let swarm_parent = test_git_output(&swarm, &["rev-parse", "HEAD"])?;
        test_git(
            &swarm,
            &[
                "remote",
                "set-url",
                "origin",
                "https://github.com/EffortlessMetrics/ripr-swarm.git",
            ],
        )?;
        let source_head_before = test_git_output(&source, &["rev-parse", "HEAD"])?;
        let swarm_head_before = test_git_output(&swarm, &["rev-parse", "HEAD"])?;
        let swarm_ref = "refs/releases/test-swarm";
        test_git(&swarm, &["update-ref", swarm_ref, &swarm_parent])?;
        let receipt = build_receipt(&Options {
            source_parent,
            swarm_parent,
            swarm_ref: swarm_ref.to_string(),
            source_repo: source.clone(),
            swarm_repo: swarm.clone(),
            source_main: "HEAD".to_string(),
            swarm_main: "HEAD".to_string(),
            version: "0.11.0".to_string(),
            resolved_tree: None,
            out: root.join("out"),
            source_remote: "EffortlessMetrics/ripr".to_string(),
            swarm_remote: "EffortlessMetrics/ripr-swarm".to_string(),
        })?;
        if receipt.mode != "two_parent_join" || receipt.dry_merge.status != "conflicts" {
            return Err(
                "diverged fixture did not produce a conflicting two-parent receipt".to_string(),
            );
        }
        if receipt.dry_merge.conflicts != ["shared.txt"] || !receipt.swarm_only_paths.is_empty() {
            return Err(format!(
                "shared path was not classified by stable set semantics: conflicts={:?}, swarm_only={:?}",
                receipt.dry_merge.conflicts, receipt.swarm_only_paths
            ));
        }
        if receipt.source_range.first_parent_count != 1
            || receipt.swarm_range.first_parent_count != 1
        {
            return Err("first-parent denominator was not preserved".to_string());
        }
        let markdown = render_markdown(&receipt);
        let dry_merge = markdown
            .find("## Dry merge")
            .ok_or_else(|| "Markdown omitted the named dry-merge section".to_string())?;
        let conflicts = markdown[dry_merge..]
            .find("Conflicts:\n- `shared.txt`")
            .ok_or_else(|| "Markdown misplaced the named conflict field".to_string())?;
        let source_survivors = markdown
            .find("## Source survivors")
            .ok_or_else(|| "Markdown omitted the named source-survivors section".to_string())?;
        if conflicts == 0 || source_survivors <= dry_merge {
            return Err(
                "Markdown named fields were not rendered in their governed sections".to_string(),
            );
        }
        if test_git_output(&source, &["rev-parse", "HEAD"])? != source_head_before
            || test_git_output(&swarm, &["rev-parse", "HEAD"])? != swarm_head_before
        {
            return Err("preflight moved an authoritative checkout".to_string());
        }
        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn disposable_dry_merge_reports_clean_non_conflicting_history() -> Result<(), String> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("system clock before epoch: {error}"))?
            .as_nanos();
        let root = std::env::temp_dir().join(format!("ripr-preflight-clean-{nonce}"));
        let source = root.join("source");
        let swarm = root.join("swarm");
        fs::create_dir_all(&source).map_err(|error| error.to_string())?;
        test_git(&source, &["init", "--quiet"])?;
        test_git(&source, &["config", "user.email", "test@example.invalid"])?;
        test_git(&source, &["config", "user.name", "test"])?;
        fs::write(source.join("base.txt"), "base\n").map_err(|error| error.to_string())?;
        test_git(&source, &["add", "base.txt"])?;
        test_git(&source, &["commit", "--quiet", "-m", "base"])?;
        let base = test_git_output(&source, &["rev-parse", "HEAD"])?;
        test_git(&source, &["clone", ".", swarm.to_string_lossy().as_ref()])?;
        test_git(&source, &["checkout", "--quiet", "-B", "main", &base])?;
        test_git(&swarm, &["config", "user.email", "test@example.invalid"])?;
        test_git(&swarm, &["config", "user.name", "test"])?;
        fs::write(source.join("source.txt"), "source\n").map_err(|error| error.to_string())?;
        test_git(&source, &["add", "source.txt"])?;
        test_git(&source, &["commit", "--quiet", "-m", "source"])?;
        let source_parent = test_git_output(&source, &["rev-parse", "HEAD"])?;
        fs::write(source.join("reviewed.txt"), "reviewed\n").map_err(|error| error.to_string())?;
        test_git(&source, &["add", "reviewed.txt"])?;
        let reviewed_tree = test_git_output(&source, &["write-tree"])?;
        if !verify_tree_object(&source, &reviewed_tree)? {
            return Err("write-tree did not create a source object-store tree".to_string());
        }
        fs::write(swarm.join("swarm.txt"), "swarm\n").map_err(|error| error.to_string())?;
        test_git(&swarm, &["add", "swarm.txt"])?;
        test_git(&swarm, &["commit", "--quiet", "-m", "swarm"])?;
        let swarm_parent = test_git_output(&swarm, &["rev-parse", "HEAD"])?;
        let reviewed_resolved_tree_verified =
            verify_tree_in_repositories(&source, &swarm, &reviewed_tree).is_ok();
        if !reviewed_resolved_tree_verified {
            return Err("reviewed write-tree object was not found".to_string());
        }
        let dry_merge =
            with_disposable_repo(&source, &swarm, &source_parent, &swarm_parent, |repo| {
                dry_merge_from_repo(
                    repo,
                    &source_parent,
                    &swarm_parent,
                    Some(&reviewed_tree),
                    reviewed_resolved_tree_verified,
                )
            })?;
        if dry_merge.status != "clean"
            || !dry_merge.conflicts.is_empty()
            || dry_merge.preview_tree.is_none()
            || dry_merge.reviewed_resolved_tree.as_deref() != Some(reviewed_tree.as_str())
            || !dry_merge.reviewed_resolved_tree_verified
        {
            return Err(format!(
                "non-conflicting pair was not reported cleanly: {dry_merge:?}"
            ));
        }
        fs::remove_dir_all(&root).map_err(|error| format!("cleanup failed: {error}"))?;
        Ok(())
    }

    fn test_git(repo: &Path, args: &[&str]) -> Result<(), String> {
        let output = Command::new("git")
            .current_dir(repo)
            .args(args)
            .output()
            .map_err(|error| format!("test git failed to start: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "test git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(())
    }

    fn test_git_output(repo: &Path, args: &[&str]) -> Result<String, String> {
        let output = Command::new("git")
            .current_dir(repo)
            .args(args)
            .output()
            .map_err(|error| format!("test git failed to start: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "test git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}
