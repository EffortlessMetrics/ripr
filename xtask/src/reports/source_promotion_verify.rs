//! Strict, read-only verification of a history-preserving source promotion.
//!
//! The preflight receipt is evidence about the proposed inputs.  This module
//! verifies the object graph that was actually produced; it never constructs a
//! merge, resolves a conflict, or updates a ref.

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const PREFLIGHT_SCHEMA: &str = "ripr.source_promotion_preflight.v1";
const RESOLUTION_SCHEMA: &str = "ripr.source_promotion_resolution.v1";
const REPORT_JSON: &str = "source-promotion-verification.json";
const REPORT_MD: &str = "source-promotion-verification.md";

const METADATA_SURFACES: &[&str] = &[
    "Cargo.toml",
    "crates/ripr/Cargo.toml",
    "Cargo.lock",
    "editors/vscode/package.json",
    "editors/vscode/package-lock.json",
    "CHANGELOG.md",
];

#[derive(Debug)]
struct Options {
    preflight: PathBuf,
    manifest: PathBuf,
    join: String,
    source_main: String,
    main: Option<String>,
    out: PathBuf,
}

pub(crate) fn source_promotion_verify(args: &[String]) -> Result<(), String> {
    let options = parse_args(args)?;
    let preflight_bytes = fs::read(&options.preflight)
        .map_err(|error| format!("failed to read preflight receipt: {error}"))?;
    let preflight_digest = digest_bytes(&preflight_bytes);
    let preflight: Value = serde_json::from_slice(&preflight_bytes)
        .map_err(|error| format!("malformed preflight receipt: {error}"))?;
    validate_preflight(&preflight, &options.source_main)?;
    let manifest_bytes = fs::read(&options.manifest)
        .map_err(|error| format!("failed to read resolution manifest: {error}"))?;
    let manifest: Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("malformed resolution manifest: {error}"))?;
    validate_manifest(&manifest, &preflight, &preflight_digest)?;

    let graph = verify_graph(&options, &preflight)?;
    verify_metadata(&options.join, &options.source_main)?;
    if let Some(main) = &options.main {
        git_exact_commit(main, "--main-head")?;
        require_ancestor(
            &options.join,
            main,
            "declared join is not reachable from merged source main",
        )?;
    }

    let mut checks = Map::new();
    checks.insert("head_is_declared_join".into(), Value::Bool(true));
    checks.insert("ordered_parents".into(), Value::Bool(true));
    checks.insert("ancestry_and_digest".into(), Value::Bool(true));
    checks.insert("reviewed_tree".into(), Value::Bool(true));
    checks.insert("metadata_byte_identity".into(), Value::Bool(true));
    checks.insert(
        "main_reachability".into(),
        Value::Bool(options.main.is_some()),
    );
    checks.insert("caller_state_mutated".into(), Value::Bool(false));

    let report = serde_json::json!({
        "schema": "ripr.source_promotion_verification.v1",
        "status": "verified",
        "join_head": options.join,
        "source_main": options.source_main,
        "main_head": options.main,
        "parents": graph.parents,
        "tree": graph.tree,
        "preflight_sha256": preflight_digest,
        "resolution_manifest_sha256": digest_bytes(&manifest_bytes),
        "merge_base": graph.merge_base,
        "swarm_reachability": {
            "all_reachable_count": graph.swarm_all_count,
            "first_parent_count": graph.swarm_first_count,
            "all_reachable_sha256": graph.swarm_all_digest,
            "first_parent_ordered_sha256": graph.swarm_first_digest,
            "verified_through_parent_2": true,
        },
        "metadata_surfaces": METADATA_SURFACES,
        "checks": checks,
        "failure_reasons": [],
        "invalidation_rules": [
            "Changing the preflight bytes, resolution manifest, exact join, parent identities, reviewed tree, or verified main invalidates this receipt.",
            "A descendant repair commit is not the declared join and must be verified with a fresh exact head.",
            "This receipt proves Git graph and byte identity only; it does not adjudicate conflicts, product correctness, release readiness, or publication.",
        ],
        "non_claims": [
            "No semantic conflict ruling or artifact adequacy claim.",
            "No join construction, ref mutation, publication, release, or K back-sync verification.",
        ],
    });
    let json = serde_json::to_string_pretty(&report)
        .map_err(|error| format!("failed to serialize verification receipt: {error}"))?;
    let markdown = render_markdown(&report)?;
    fs::create_dir_all(&options.out)
        .map_err(|error| format!("failed to create {}: {error}", options.out.display()))?;
    fs::write(options.out.join(REPORT_JSON), format!("{json}\n"))
        .map_err(|error| format!("failed to write verification JSON: {error}"))?;
    fs::write(options.out.join(REPORT_MD), markdown)
        .map_err(|error| format!("failed to write verification Markdown: {error}"))?;
    println!("Wrote {}", options.out.join(REPORT_JSON).display());
    println!("Wrote {}", options.out.join(REPORT_MD).display());
    Ok(())
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    if args.first().map(String::as_str) != Some("verify") {
        return Err(usage());
    }
    let mut values = BTreeMap::<&str, String>::new();
    let mut index = 1;
    while index < args.len() {
        let key = args[index].as_str();
        if !key.starts_with("--") || index + 1 >= args.len() {
            return Err(usage());
        }
        if !matches!(
            key,
            "--preflight"
                | "--resolution-manifest"
                | "--join-head"
                | "--source-main"
                | "--main-head"
                | "--out"
        ) {
            return Err(format!("unknown option {key}\n{}", usage()));
        }
        if values.insert(key, args[index + 1].clone()).is_some() {
            return Err(format!("duplicate option {key}"));
        }
        index += 2;
    }
    let required = |key: &str| {
        values
            .get(key)
            .cloned()
            .filter(|v| !v.trim().is_empty())
            .ok_or_else(|| format!("missing {key}\n{}", usage()))
    };
    let join = required("--join-head")?;
    let source_main = required("--source-main")?;
    validate_identity("--join-head", &join)?;
    validate_identity("--source-main", &source_main)?;
    let main = values.get("--main-head").cloned();
    if let Some(value) = &main {
        validate_identity("--main-head", value)?;
    }
    Ok(Options {
        preflight: PathBuf::from(required("--preflight")?),
        manifest: PathBuf::from(required("--resolution-manifest")?),
        join,
        source_main,
        main,
        out: values
            .get("--out")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("target/ripr/source-promotion")),
    })
}

fn usage() -> String {
    "usage: cargo xtask source-promotion verify --preflight <receipt.json> --resolution-manifest <manifest.json> --join-head <40-char-sha> --source-main <40-char-sha> [--main-head <40-char-sha>] [--out <dir>]".to_string()
}

fn validate_identity(name: &str, value: &str) -> Result<(), String> {
    if value.len() != 40
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(format!(
            "{name} must be an exact 40-character lowercase hexadecimal commit SHA; floating refs and abbreviations are rejected"
        ));
    }
    Ok(())
}

fn validate_preflight(preflight: &Value, source_main: &str) -> Result<(), String> {
    let schema = string_field(preflight, "schema")?;
    if schema != PREFLIGHT_SCHEMA {
        return Err(format!("unsupported preflight schema {schema:?}"));
    }
    if string_field(preflight, "mode")? != "two_parent_join" {
        return Err("preflight mode must be two_parent_join".to_string());
    }
    let source = string_field(preflight, "source_parent")?;
    let swarm = string_field(preflight, "swarm_parent")?;
    let merge_base = string_field(preflight, "merge_base")?;
    for (name, value) in [
        ("source_parent", source),
        ("swarm_parent", swarm),
        ("merge_base", merge_base),
    ] {
        validate_hex(name, value, 40)?;
    }
    if source != source_main {
        return Err("preflight source parent does not match exact --source-main".to_string());
    }
    if string_field(preflight, "source_main")? != source {
        return Err("preflight source_main is not bound to source parent".to_string());
    }
    let swarm_ref = string_field(preflight, "swarm_ref")?;
    if !swarm_ref.starts_with("refs/") {
        return Err("preflight swarm_ref is not immutable".to_string());
    }
    if string_field(preflight, "swarm_ref_sha")? != swarm {
        return Err("preflight immutable swarm ref is not bound to swarm parent".to_string());
    }
    for role in ["source_repository", "swarm_repository"] {
        let repository = object_field(preflight, role)?;
        for flag in ["common_dir_verified", "root_verified", "remote_verified"] {
            if repository.get(flag).and_then(Value::as_bool) != Some(true) {
                return Err(format!("preflight {role} is not fully identity-verified"));
            }
        }
    }
    let dry = object_field(preflight, "dry_merge")?;
    let reviewed = dry
        .get("reviewed_resolved_tree")
        .and_then(Value::as_str)
        .ok_or_else(|| "preflight is missing reviewed resolved tree".to_string())?;
    validate_hex("reviewed_resolved_tree", reviewed, 40)?;
    if dry
        .get("reviewed_resolved_tree_verified")
        .and_then(Value::as_bool)
        != Some(true)
    {
        return Err("preflight reviewed resolved tree is not verified".to_string());
    }
    for key in [
        "source_range",
        "swarm_range",
        "source_repository",
        "swarm_repository",
        "version_state",
        "invalidation_rules",
    ] {
        let _ = preflight
            .get(key)
            .ok_or_else(|| format!("preflight is incomplete: missing {key}"))?;
    }
    Ok(())
}

fn validate_manifest(manifest: &Value, preflight: &Value, digest: &str) -> Result<(), String> {
    if string_field(manifest, "schema")? != RESOLUTION_SCHEMA {
        return Err("unsupported resolution manifest schema".to_string());
    }
    let bound = manifest
        .get("preflight_sha256")
        .or_else(|| manifest.get("preflight_digest"))
        .and_then(Value::as_str)
        .ok_or_else(|| "resolution manifest is missing preflight_sha256".to_string())?;
    if bound != digest {
        return Err("resolution manifest is bound to a different preflight digest".to_string());
    }
    for key in ["source_parent", "swarm_parent", "merge_base"] {
        if string_field(manifest, key)? != string_field(preflight, key)? {
            return Err(format!(
                "resolution manifest {key} does not match preflight"
            ));
        }
    }
    let tree = manifest
        .get("reviewed_join_tree")
        .or_else(|| manifest.get("reviewed_resolved_tree"))
        .and_then(Value::as_str)
        .ok_or_else(|| "resolution manifest is missing reviewed_join_tree".to_string())?;
    validate_hex("reviewed_join_tree", tree, 40)?;
    let receipt_tree = object_field(preflight, "dry_merge")?
        .get("reviewed_resolved_tree")
        .and_then(Value::as_str)
        .ok_or_else(|| "preflight is missing reviewed resolved tree".to_string())?;
    if tree != receipt_tree {
        return Err("resolution manifest reviewed tree does not match preflight".to_string());
    }
    let rows = manifest
        .get("dispositions")
        .or_else(|| manifest.get("resolutions"))
        .and_then(Value::as_array)
        .ok_or_else(|| "resolution manifest is missing dispositions".to_string())?;
    let dry = object_field(preflight, "dry_merge")?;
    let mut expected = BTreeSet::<String>::new();
    for key in dry
        .get("conflicts")
        .and_then(Value::as_array)
        .ok_or_else(|| "preflight conflict inventory is malformed".to_string())?
    {
        expected.insert(format!(
            "conflict:{}",
            key.as_str()
                .ok_or_else(|| "conflict inventory contains a non-string".to_string())?
        ));
    }
    for key in preflight
        .get("source_survivor_candidates")
        .and_then(Value::as_array)
        .ok_or_else(|| "preflight source survivor inventory is malformed".to_string())?
    {
        expected.insert(format!(
            "source_survivor:{}",
            key.as_str()
                .ok_or_else(|| "source survivor inventory contains a non-string".to_string())?
        ));
    }
    for key in preflight
        .get("swarm_authority_resolution_candidates")
        .and_then(Value::as_array)
        .ok_or_else(|| "preflight swarm exclusion inventory is malformed".to_string())?
    {
        expected.insert(format!(
            "swarm_exclusion:{}",
            key.as_str()
                .ok_or_else(|| "swarm exclusion inventory contains a non-string".to_string())?
        ));
    }
    let mut actual = BTreeSet::new();
    for row in rows {
        let kind = row
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| "resolution row is missing kind".to_string())?;
        let key = row
            .get("key")
            .or_else(|| row.get("path"))
            .and_then(Value::as_str)
            .ok_or_else(|| "resolution row is missing key".to_string())?;
        let disposition = row
            .get("disposition")
            .and_then(Value::as_str)
            .filter(|v| !v.trim().is_empty())
            .ok_or_else(|| "resolution row is missing disposition".to_string())?;
        if row
            .get("rationale")
            .and_then(Value::as_str)
            .is_none_or(|value| value.trim().is_empty())
        {
            return Err("resolution row is missing rationale".to_string());
        }
        if row
            .get("evidence")
            .or_else(|| row.get("evidence_ref"))
            .and_then(Value::as_str)
            .is_none_or(|value| value.trim().is_empty())
        {
            return Err("resolution row is missing evidence".to_string());
        }
        if !matches!(kind, "conflict" | "source_survivor" | "swarm_exclusion")
            || disposition.is_empty()
        {
            return Err("resolution row has an invalid kind or disposition".to_string());
        }
        if !actual.insert(format!("{kind}:{key}")) {
            return Err(format!("duplicate resolution row {kind}:{key}"));
        }
    }
    if actual != expected {
        return Err(format!(
            "resolution rows do not exactly cover inventory: expected {expected:?}, actual {actual:?}"
        ));
    }
    Ok(())
}

struct Graph {
    parents: Vec<String>,
    tree: String,
    merge_base: String,
    swarm_all_count: usize,
    swarm_first_count: usize,
    swarm_all_digest: String,
    swarm_first_digest: String,
}

fn verify_graph(options: &Options, preflight: &Value) -> Result<Graph, String> {
    git_exact_commit(&options.join, "--join-head")?;
    let parents_line = git(&["rev-list", "--parents", "-n", "1", &options.join])?;
    let parts: Vec<_> = parents_line.split_whitespace().collect();
    if parts.len() != 3 || parts[0] != options.join {
        return Err("head_is_declared_join failed: join must have exactly two parents".to_string());
    }
    let source = string_field(preflight, "source_parent")?;
    let swarm = string_field(preflight, "swarm_parent")?;
    if parts[1] != source || parts[2] != swarm {
        return Err(
            "ordered parents do not match preflight source_parent, swarm_parent".to_string(),
        );
    }
    require_ancestor(
        source,
        &options.join,
        "source parent is not an ancestor of J",
    )?;
    require_ancestor(swarm, &options.join, "swarm parent is not an ancestor of J")?;
    let merge_base = git(&["merge-base", source, swarm])?.trim().to_string();
    if merge_base != string_field(preflight, "merge_base")? {
        return Err("merge base differs from preflight".to_string());
    }
    let tree = git(&["rev-parse", &format!("{}^{{tree}}", options.join)])?
        .trim()
        .to_string();
    let dry_tree = object_field(preflight, "dry_merge")?
        .get("reviewed_resolved_tree")
        .and_then(Value::as_str)
        .ok_or_else(|| "reviewed tree is malformed".to_string())?;
    if tree != dry_tree {
        return Err("J tree does not equal reviewed resolved tree".to_string());
    }
    let manifest_tree = tree.clone();
    if object_field(preflight, "dry_merge")?
        .get("preview_tree")
        .and_then(Value::as_str)
        == Some(manifest_tree.as_str())
        && object_field(preflight, "dry_merge")?
            .get("conflicts")
            .and_then(Value::as_array)
            .is_some_and(|v| !v.is_empty())
    {
        return Err(
            "automatic preview_tree was substituted for reviewed resolved tree".to_string(),
        );
    }
    let source_range = recompute_range(&merge_base, source)?;
    let swarm_range = recompute_range(&merge_base, swarm)?;
    compare_range(
        &source_range,
        object_field(preflight, "source_range")?,
        "source",
    )?;
    compare_range(
        &swarm_range,
        object_field(preflight, "swarm_range")?,
        "swarm",
    )?;
    Ok(Graph {
        parents: vec![source.to_string(), swarm.to_string()],
        tree,
        merge_base,
        swarm_all_count: swarm_range.0.len(),
        swarm_first_count: swarm_range.1.len(),
        swarm_all_digest: digest_lines(&swarm_range.0),
        swarm_first_digest: digest_lines(&swarm_range.1),
    })
}

fn recompute_range(base: &str, head: &str) -> Result<(Vec<String>, Vec<String>), String> {
    let all = lines(git(&[
        "rev-list",
        "--topo-order",
        "--reverse",
        &format!("{base}..{head}"),
    ])?);
    let first = lines(git(&[
        "rev-list",
        "--first-parent",
        "--reverse",
        &format!("{base}..{head}"),
    ])?);
    Ok((all, first))
}

fn compare_range(
    range: &(Vec<String>, Vec<String>),
    expected: &Map<String, Value>,
    role: &str,
) -> Result<(), String> {
    let all_count = expected
        .get("all_reachable_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("{role} range missing all_reachable_count"))?
        as usize;
    let first_count = expected
        .get("first_parent_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("{role} range missing first_parent_count"))?
        as usize;
    if all_count != range.0.len() || first_count != range.1.len() {
        return Err(format!("{role} ancestry count differs from preflight"));
    }
    for (key, actual) in [
        ("all_reachable_sha256", digest_lines(&range.0)),
        ("first_parent_ordered_sha256", digest_lines(&range.1)),
    ] {
        if expected.get(key).and_then(Value::as_str) != Some(actual.as_str()) {
            return Err(format!("{role} {key} differs from preflight"));
        }
    }
    Ok(())
}

fn verify_metadata(join: &str, source: &str) -> Result<(), String> {
    for path in METADATA_SURFACES {
        let source_bytes = git_bytes(&["show", &format!("{source}:{path}")])?;
        let join_bytes = git_bytes(&["show", &format!("{join}:{path}")])?;
        if source_bytes != join_bytes {
            return Err(format!("metadata byte identity failed for {path}"));
        }
    }
    Ok(())
}

fn render_markdown(report: &Value) -> Result<String, String> {
    let schema = string_field(report, "schema")?;
    let status = string_field(report, "status")?;
    let join = string_field(report, "join_head")?;
    let source = string_field(report, "source_main")?;
    let tree = string_field(report, "tree")?;
    let parents = report
        .get("parents")
        .and_then(Value::as_array)
        .ok_or_else(|| "report parents missing".to_string())?;
    Ok(format!(
        "# Source-promotion verification\n\n- Schema: {schema}\n- Status: **{status}**\n- J: `{join}`\n- SOURCE_PARENT: `{source}`\n- Parents (ordered): `{}` then `{}`\n- Tree: `{tree}`\n- Preflight SHA-256: `{}`\n- Resolution manifest SHA-256: `{}`\n\n## Claim boundary\n\nThe receipt proves the exact two-parent Git graph, reviewed tree identity, ancestry denominators/digests, metadata byte identity, and optional merged-main reachability. It does not adjudicate semantic conflict resolutions, product correctness, release readiness, publication, or K back-sync.\n",
        parents.first().and_then(Value::as_str).unwrap_or(""),
        parents.get(1).and_then(Value::as_str).unwrap_or(""),
        string_field(report, "preflight_sha256")?,
        string_field(report, "resolution_manifest_sha256")?
    ))
}

fn git_exact_commit(value: &str, name: &str) -> Result<(), String> {
    let resolved = git(&["rev-parse", "--verify", &format!("{value}^{{commit}}")])?
        .trim()
        .to_string();
    if resolved != value {
        return Err(format!("{name} is not an exact commit object"));
    }
    Ok(())
}

fn require_ancestor(ancestor: &str, descendant: &str, message: &str) -> Result<(), String> {
    let output = Command::new("git")
        .args(["merge-base", "--is-ancestor", ancestor, descendant])
        .output()
        .map_err(|error| format!("failed to test ancestry: {error}"))?;
    if !output.status.success() {
        return Err(message.to_string());
    }
    Ok(())
}

fn git(args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .output()
        .map_err(|error| format!("failed to execute git {}: {error}", args.join(" ")))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| format!("git returned non-UTF-8 output: {error}"))
}

fn git_bytes(args: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .args(args)
        .output()
        .map_err(|error| format!("failed to execute git {}: {error}", args.join(" ")))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output.stdout)
}

fn string_field<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| format!("missing string field {key}"))
}
fn object_field<'a>(value: &'a Value, key: &str) -> Result<&'a Map<String, Value>, String> {
    value
        .get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| format!("missing object field {key}"))
}
fn validate_hex(name: &str, value: &str, length: usize) -> Result<(), String> {
    if value.len() != length || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!(
            "{name} is not a {length}-character hexadecimal value"
        ));
    }
    Ok(())
}
fn lines(value: String) -> Vec<String> {
    value
        .lines()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
        .collect()
}
fn digest_lines(values: &[String]) -> String {
    let mut hasher = Sha256::new();
    for value in values {
        hasher.update(value.as_bytes());
        hasher.update([b'\n']);
    }
    format!("sha256:{:x}", hasher.finalize())
}
fn digest_bytes(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::{Mutex, OnceLock};

    static CWD_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    #[test]
    fn identity_arguments_are_exact_and_lowercase() -> Result<(), String> {
        for value in [
            "HEAD",
            "origin/main",
            "deadbeef",
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        ] {
            if validate_identity("--join-head", value).is_ok() {
                return Err(format!("floating or non-exact identity accepted: {value}"));
            }
        }
        validate_identity("--join-head", "0123456789abcdef0123456789abcdef01234567")
    }

    #[test]
    fn ordered_digest_is_not_commutative() -> Result<(), String> {
        if digest_lines(&["a".into(), "b".into()]) == digest_lines(&["b".into(), "a".into()]) {
            return Err("ordered digest ignored order".into());
        }
        Ok(())
    }

    #[test]
    fn manifest_rejects_duplicate_rows() -> Result<(), String> {
        let preflight = serde_json::json!({"schema": PREFLIGHT_SCHEMA, "source_parent":"0000000000000000000000000000000000000000", "swarm_parent":"1111111111111111111111111111111111111111", "merge_base":"2222222222222222222222222222222222222222", "dry_merge":{"reviewed_resolved_tree":"3333333333333333333333333333333333333333","reviewed_resolved_tree_verified":true,"conflicts":["x"]}, "source_survivor_candidates":[], "swarm_authority_resolution_candidates":[]});
        let digest = digest_bytes(
            serde_json::to_string(&preflight)
                .map_err(|e| e.to_string())?
                .as_bytes(),
        );
        let manifest = serde_json::json!({"schema": RESOLUTION_SCHEMA, "preflight_sha256": digest, "source_parent":"0000000000000000000000000000000000000000", "swarm_parent":"1111111111111111111111111111111111111111", "merge_base":"2222222222222222222222222222222222222222", "reviewed_join_tree":"3333333333333333333333333333333333333333", "dispositions":[{"kind":"conflict","key":"x","disposition":"source","rationale":"reviewed","evidence":"review"},{"kind":"conflict","key":"x","disposition":"source","rationale":"duplicate","evidence":"duplicate"}]});
        if validate_manifest(&manifest, &preflight, &digest).is_ok() {
            return Err("duplicate manifest row accepted".into());
        }
        Ok(())
    }

    #[test]
    fn synthetic_join_rejects_appended_repair_head() -> Result<(), String> {
        let _guard = CWD_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .map_err(|_| "cwd test lock poisoned".to_string())?;
        let root = std::env::temp_dir().join(format!("ripr-source-verify-{}", std::process::id()));
        if root.exists() {
            return Err("synthetic fixture path already exists".to_string());
        }
        fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        let result = (|| {
            test_git(&root, &["init", "--quiet"])?;
            test_git(&root, &["config", "user.email", "test@example.invalid"])?;
            test_git(&root, &["config", "user.name", "test"])?;
            fs::write(root.join("base.txt"), "base\n").map_err(|error| error.to_string())?;
            test_git(&root, &["add", "base.txt"])?;
            test_git(&root, &["commit", "--quiet", "-m", "base"])?;
            let base = test_git_output(&root, &["rev-parse", "HEAD"])?;
            fs::write(root.join("source.txt"), "source\n").map_err(|error| error.to_string())?;
            test_git(&root, &["add", "source.txt"])?;
            test_git(&root, &["commit", "--quiet", "-m", "source"])?;
            let source = test_git_output(&root, &["rev-parse", "HEAD"])?;
            test_git(&root, &["reset", "--quiet", "--hard", &base])?;
            fs::write(root.join("swarm.txt"), "swarm\n").map_err(|error| error.to_string())?;
            test_git(&root, &["add", "swarm.txt"])?;
            test_git(&root, &["commit", "--quiet", "-m", "swarm"])?;
            let swarm = test_git_output(&root, &["rev-parse", "HEAD"])?;
            test_git(&root, &["read-tree", &source])?;
            fs::write(root.join("swarm.txt"), "swarm\n").map_err(|error| error.to_string())?;
            test_git(&root, &["add", "swarm.txt"])?;
            let tree = test_git_output(&root, &["write-tree"])?;
            let join = test_git_output_with_input(
                &root,
                &["commit-tree", &tree, "-p", &source, "-p", &swarm],
                "join\n",
            )?;
            let repair_tree = test_git_output(&root, &["rev-parse", &format!("{join}^{{tree}}")])?;
            fs::write(root.join("repair.txt"), "repair\n").map_err(|error| error.to_string())?;
            test_git(&root, &["add", "repair.txt"])?;
            let repair_tree = test_git_output(&root, &["write-tree"])?;
            let repair = test_git_output_with_input(
                &root,
                &["commit-tree", &repair_tree, "-p", &join],
                "repair\n",
            )?;
            let source_range = test_recompute_range(&root, &base, &source)?;
            let swarm_range = test_recompute_range(&root, &base, &swarm)?;
            let preflight = serde_json::json!({
                "schema": PREFLIGHT_SCHEMA, "mode": "two_parent_join", "source_parent": source,
                "swarm_parent": swarm, "swarm_ref": "refs/ripr/release-0.11.0", "swarm_ref_sha": swarm,
                "source_main": source, "merge_base": base,
                "source_repository": {"common_dir_verified":true,"root_verified":true,"remote_verified":true},
                "swarm_repository": {"common_dir_verified":true,"root_verified":true,"remote_verified":true},
                "source_range": range_json(&source_range), "swarm_range": range_json(&swarm_range),
                "dry_merge": {"reviewed_resolved_tree": tree, "reviewed_resolved_tree_verified":true, "preview_tree": "", "conflicts":[]},
                "source_survivor_candidates":[], "swarm_authority_resolution_candidates":[], "version_state":{}, "invalidation_rules":[]
            });
            let options = Options {
                preflight: PathBuf::new(),
                manifest: PathBuf::new(),
                join: repair,
                source_main: source,
                main: None,
                out: PathBuf::new(),
            };
            let old = std::env::current_dir().map_err(|error| error.to_string())?;
            std::env::set_current_dir(&root).map_err(|error| error.to_string())?;
            let failed = verify_graph(&options, &preflight).is_err();
            std::env::set_current_dir(old).map_err(|error| error.to_string())?;
            if !failed || repair_tree.is_empty() {
                return Err("appended repair commit was accepted as J".to_string());
            }
            Ok(())
        })();
        let _ = fs::remove_dir_all(&root);
        result
    }

    fn range_json(range: &(Vec<String>, Vec<String>)) -> Value {
        serde_json::json!({
            "all_reachable_count": range.0.len(), "first_parent_count": range.1.len(),
            "all_reachable_sha256": digest_lines(&range.0), "first_parent_ordered_sha256": digest_lines(&range.1)
        })
    }

    fn test_git(repo: &Path, args: &[&str]) -> Result<(), String> {
        let output = Command::new("git")
            .current_dir(repo)
            .args(args)
            .output()
            .map_err(|error| error.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }
        Ok(())
    }

    fn test_recompute_range(
        repo: &Path,
        base: &str,
        head: &str,
    ) -> Result<(Vec<String>, Vec<String>), String> {
        let range = format!("{base}..{head}");
        let all = lines(test_git_output(
            repo,
            &["rev-list", "--topo-order", "--reverse", &range],
        )?);
        let first = lines(test_git_output(
            repo,
            &["rev-list", "--first-parent", "--reverse", &range],
        )?);
        Ok((all, first))
    }

    fn test_git_output(repo: &Path, args: &[&str]) -> Result<String, String> {
        let output = Command::new("git")
            .current_dir(repo)
            .args(args)
            .output()
            .map_err(|error| error.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn test_git_output_with_input(
        repo: &Path,
        args: &[&str],
        input: &str,
    ) -> Result<String, String> {
        let mut child = Command::new("git")
            .current_dir(repo)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|error| error.to_string())?;
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin
                .write_all(input.as_bytes())
                .map_err(|error| error.to_string())?;
        }
        let output = child
            .wait_with_output()
            .map_err(|error| error.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}
