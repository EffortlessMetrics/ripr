//! Strict, read-only verification of a history-preserving source promotion.
//!
//! The preflight receipt is evidence about the proposed inputs.  This module
//! verifies the object graph that was actually produced; it never constructs a
//! merge, resolves a conflict, or updates a ref.

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

mod version_identity;
use version_identity::{RELEASE_METADATA_SURFACES, verify_release_metadata_identity};

const PREFLIGHT_SCHEMA: &str = "ripr.source_promotion_preflight.v1";
const RESOLUTION_SCHEMA: &str = "ripr.source_promotion_resolution.v1";
const REPORT_JSON: &str = "source-promotion-verification.json";
const REPORT_MD: &str = "source-promotion-verification.md";

#[derive(Clone, Debug)]
struct Options {
    repo: PathBuf,
    preflight: PathBuf,
    manifest: PathBuf,
    join: String,
    source_main: String,
    main: Option<String>,
    out: PathBuf,
}

pub(crate) fn source_promotion_verify(args: &[String]) -> Result<(), String> {
    let options = match parse_args(args) {
        Ok(options) => options,
        Err(reason) => {
            if let Some(out) = output_path_from_args(args) {
                let options = Options {
                    preflight: PathBuf::new(),
                    repo: std::env::current_dir().unwrap_or_default(),
                    manifest: PathBuf::new(),
                    join: String::new(),
                    source_main: String::new(),
                    main: None,
                    out,
                };
                let report = failure_report(&options, &reason);
                write_report(&options.out, &report)?;
            }
            return Err(reason);
        }
    };
    match verify(&options) {
        Ok(report) => write_report(&options.out, &report),
        Err(reason) => {
            let report = failure_report(&options, &reason);
            let write_result = write_report(&options.out, &report);
            match write_result {
                Ok(()) => Err(reason),
                Err(write_error) => Err(format!(
                    "{reason}; failed to write rejection receipt: {write_error}"
                )),
            }
        }
    }
}

fn output_path_from_args(args: &[String]) -> Option<PathBuf> {
    args.windows(2)
        .find(|pair| pair[0] == "--out" && !pair[1].trim().is_empty())
        .map(|pair| PathBuf::from(&pair[1]))
}

fn verify(options: &Options) -> Result<Value, String> {
    let preflight_bytes = fs::read(&options.preflight)
        .map_err(|error| format!("failed to read preflight receipt: {error}"))?;
    let preflight: Value = serde_json::from_slice(&preflight_bytes)
        .map_err(|error| format!("malformed preflight receipt: {error}"))?;
    // Bind the exact producer file bytes, including pretty-print whitespace and trailing LF.
    let preflight_digest = digest_bytes(&preflight_bytes);
    validate_preflight(&preflight, &options.source_main)?;
    let manifest_bytes = fs::read(&options.manifest)
        .map_err(|error| format!("failed to read resolution manifest: {error}"))?;
    let manifest: Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("malformed resolution manifest: {error}"))?;
    validate_manifest(&manifest, &preflight, &preflight_digest)?;

    let graph = verify_graph(options, &preflight)?;
    let release_metadata_verified =
        verify_release_metadata(&options.repo, &options.join, &options.source_main)
            .map(|()| true)?;
    if let Some(main) = &options.main {
        git_exact_commit(&options.repo, main, "--main-head")?;
        require_ancestor(
            &options.repo,
            &options.join,
            main,
            "declared join is not reachable from merged source main",
        )?;
    }

    let mut checks = Map::new();
    let source = string_field(&preflight, "source_parent")?;
    let swarm = string_field(&preflight, "swarm_parent")?;
    let reviewed_tree = object_field(&preflight, "dry_merge")?
        .get("reviewed_resolved_tree")
        .and_then(Value::as_str);
    checks.insert(
        "head_is_declared_join".into(),
        Value::Bool(graph.parents.len() == 2),
    );
    checks.insert(
        "ordered_parents".into(),
        Value::Bool(graph.parents.as_slice() == [source, swarm]),
    );
    checks.insert(
        "ancestry_and_digest".into(),
        Value::Bool(graph.ancestry_verified),
    );
    checks.insert(
        "reviewed_tree".into(),
        Value::Bool(reviewed_tree == Some(graph.tree.as_str())),
    );
    checks.insert(
        "release_version_identity".into(),
        Value::Bool(release_metadata_verified),
    );
    checks.insert(
        "main_reachability".into(),
        options.main.as_ref().map_or_else(
            || Value::String("not_run".to_string()),
            |_| Value::String("passed".to_string()),
        ),
    );
    checks.insert("caller_state_mutated".into(), Value::Bool(false));

    let report = serde_json::json!({
        "schema": "ripr.source_promotion_verification.v2",
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
        "release_metadata_surfaces": RELEASE_METADATA_SURFACES,
        "checks": checks,
        "failure_reasons": [],
        "invalidation_rules": [
            "Changing the preflight bytes, resolution manifest, exact join, parent identities, reviewed tree, governed release-version identity, source-authoritative CHANGELOG.md bytes, or verified main invalidates this receipt.",
            "A descendant repair commit is not the declared join and must be verified with a fresh exact head.",
            "This receipt proves the exact Git graph, reviewed-tree identity, governed release-version identity, and source-authoritative CHANGELOG.md bytes only; it does not adjudicate conflicts, product correctness, release readiness, or publication.",
        ],
        "non_claims": [
            "No semantic conflict ruling or artifact adequacy claim.",
            "No join construction, ref mutation, publication, release, or K back-sync verification.",
        ],
    });
    Ok(report)
}

fn failure_report(options: &Options, reason: &str) -> Value {
    serde_json::json!({
        "schema": "ripr.source_promotion_verification.v2",
        "status": "rejected",
        "join_head": options.join,
        "source_main": options.source_main,
        "main_head": options.main,
        "parents": [],
        "tree": null,
        "preflight_sha256": null,
        "resolution_manifest_sha256": null,
        "merge_base": null,
        "swarm_reachability": {
            "all_reachable_count": null,
            "first_parent_count": null,
            "all_reachable_sha256": null,
            "first_parent_ordered_sha256": null,
            "verified_through_parent_2": false,
        },
        "release_metadata_surfaces": RELEASE_METADATA_SURFACES,
        "checks": {
            "head_is_declared_join": "not_run",
            "ordered_parents": "not_run",
            "ancestry_and_digest": "not_run",
            "reviewed_tree": "not_run",
            "release_version_identity": "not_run",
            "main_reachability": "not_run",
            "caller_state_mutated": "not_run",
        },
        "failure_reasons": [reason],
        "invalidation_rules": [
            "A rejected receipt is not evidence of a valid source promotion.",
            "Changing any input or the exact Git object view requires a fresh verification receipt.",
        ],
        "non_claims": [
            "No semantic conflict ruling or artifact adequacy claim.",
            "No join construction, ref mutation, publication, release, or K back-sync verification.",
        ],
    })
}

fn write_report(out: &std::path::Path, report: &Value) -> Result<(), String> {
    let json = serde_json::to_string_pretty(report)
        .map_err(|error| format!("failed to serialize verification receipt: {error}"))?;
    let markdown = render_markdown(report)?;
    fs::create_dir_all(out)
        .map_err(|error| format!("failed to create {}: {error}", out.display()))?;
    fs::write(out.join(REPORT_JSON), format!("{json}\n"))
        .map_err(|error| format!("failed to write verification JSON: {error}"))?;
    fs::write(out.join(REPORT_MD), markdown)
        .map_err(|error| format!("failed to write verification Markdown: {error}"))?;
    println!("Wrote {}", out.join(REPORT_JSON).display());
    println!("Wrote {}", out.join(REPORT_MD).display());
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
    let out = match values.get("--out") {
        Some(value) if value.trim().is_empty() => {
            return Err("--out must be a non-empty directory".to_string());
        }
        Some(value) => PathBuf::from(value),
        None => PathBuf::from("target/ripr/source-promotion"),
    };
    Ok(Options {
        repo: std::env::current_dir().map_err(|error| error.to_string())?,
        preflight: PathBuf::from(required("--preflight")?),
        manifest: PathBuf::from(required("--resolution-manifest")?),
        join,
        source_main,
        main,
        out,
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
    let requested_version = preflight
        .get("version_state")
        .and_then(|version_state| version_state.get("requested_version"))
        .and_then(Value::as_str)
        .filter(|version| !version.is_empty())
        .ok_or_else(|| "missing string field requested_version".to_string())?;
    validate_protected_candidate_tag_ref(swarm_ref, requested_version, swarm)?;
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

fn validate_protected_candidate_tag_ref(
    reference: &str,
    version: &str,
    parent: &str,
) -> Result<(), String> {
    let expected = format!("refs/tags/ripr-release-{version}-{parent}");
    if reference != expected {
        return Err(format!(
            "preflight swarm_ref must be the exact fully-qualified protected candidate tag {expected}"
        ));
    }
    Ok(())
}

fn validate_manifest(manifest: &Value, preflight: &Value, digest: &str) -> Result<(), String> {
    if string_field(manifest, "schema")? != RESOLUTION_SCHEMA {
        return Err("unsupported resolution manifest schema".to_string());
    }
    let bound = manifest
        .get("preflight_sha256")
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
    ancestry_verified: bool,
}

fn verify_graph(options: &Options, preflight: &Value) -> Result<Graph, String> {
    git_exact_commit(&options.repo, &options.join, "--join-head")?;
    let parents_line = git(
        &options.repo,
        &["rev-list", "--parents", "-n", "1", &options.join],
    )?;
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
        &options.repo,
        source,
        &options.join,
        "source parent is not an ancestor of J",
    )?;
    require_ancestor(
        &options.repo,
        swarm,
        &options.join,
        "swarm parent is not an ancestor of J",
    )?;
    let merge_base = unique_merge_base(&git(
        &options.repo,
        &["merge-base", "--all", source, swarm],
    )?)?;
    if merge_base != string_field(preflight, "merge_base")? {
        return Err("merge base differs from preflight".to_string());
    }
    let tree = git(
        &options.repo,
        &["rev-parse", &format!("{}^{{tree}}", options.join)],
    )?
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
    let source_range = recompute_range(&options.repo, &merge_base, source)?;
    let swarm_range = recompute_range(&options.repo, &merge_base, swarm)?;
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
        ancestry_verified: true,
    })
}

fn unique_merge_base(output: &str) -> Result<String, String> {
    let merge_bases = lines(output.to_string());
    if merge_bases.len() != 1 {
        return Err(format!(
            "merge-base is ambiguous: expected exactly one best base, found {}",
            merge_bases.len()
        ));
    }
    Ok(merge_bases[0].clone())
}

fn recompute_range(
    repo: &Path,
    base: &str,
    head: &str,
) -> Result<(Vec<String>, Vec<String>), String> {
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

fn verify_release_metadata(repo: &Path, join: &str, source: &str) -> Result<(), String> {
    verify_release_metadata_identity(repo, join, source)
}

fn render_markdown(report: &Value) -> Result<String, String> {
    let schema = string_field(report, "schema")?;
    let status = string_field(report, "status")?;
    let join = report
        .get("join_head")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or("not available");
    let source = report
        .get("source_main")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or("not available");
    let tree = report
        .get("tree")
        .and_then(Value::as_str)
        .unwrap_or("not available");
    let preflight_digest = report
        .get("preflight_sha256")
        .and_then(Value::as_str)
        .unwrap_or("not available");
    let parents = report
        .get("parents")
        .and_then(Value::as_array)
        .ok_or_else(|| "report parents missing".to_string())?;
    let structured = serde_json::to_string_pretty(report)
        .map_err(|error| format!("failed to serialize Markdown structured receipt: {error}"))?;
    let main = report
        .get("main_head")
        .and_then(Value::as_str)
        .unwrap_or("not supplied (post-merge reachability not_run)");
    let merge_base = report
        .get("merge_base")
        .and_then(Value::as_str)
        .unwrap_or("not available");
    let reachability = report
        .get("swarm_reachability")
        .ok_or_else(|| "report swarm reachability missing".to_string())?;
    let checks = report
        .get("checks")
        .ok_or_else(|| "report checks missing".to_string())?;
    let failures = report
        .get("failure_reasons")
        .ok_or_else(|| "report failure reasons missing".to_string())?;
    let invalidation = report
        .get("invalidation_rules")
        .ok_or_else(|| "report invalidation rules missing".to_string())?;
    let non_claims = report
        .get("non_claims")
        .ok_or_else(|| "report non-claims missing".to_string())?;
    let header = format!(
        "# Source-promotion verification\n\n- Schema: {schema}\n- Status: **{status}**\n- J: `{join}`\n- SOURCE_PARENT: `{source}`\n- Parents (ordered): `{}` then `{}`\n- Tree: `{tree}`\n- Preflight SHA-256: `{}`\n- Resolution manifest SHA-256: `{}`\n\n## Claim boundary\n\nThe receipt proves the exact two-parent Git graph, reviewed tree identity, ancestry denominators/digests, release-version identity and source-authoritative changelog bytes, and optional merged-main reachability. It does not adjudicate semantic conflict resolutions, product correctness, release readiness, publication, or K back-sync.\n",
        parents.first().and_then(Value::as_str).unwrap_or(""),
        parents.get(1).and_then(Value::as_str).unwrap_or(""),
        preflight_digest,
        string_field(report, "resolution_manifest_sha256").unwrap_or("not available"),
    );
    Ok(header
        + &format!(
            "\n- MAIN_HEAD: `{main}`\n- MERGE_BASE: `{merge_base}`\n\n## Swarm reachability\n\n```json\n{reachability}\n```\n\n## Checks\n\n```json\n{checks}\n```\n\n## Failure reasons\n\n```json\n{failures}\n```\n\n## Invalidation rules\n\n```json\n{invalidation}\n```\n\n## Non-claims\n\n```json\n{non_claims}\n```\n\n## Structured receipt\n\n```json\n{structured}\n```\n"
        ))
}

fn git_exact_commit(repo: &Path, value: &str, name: &str) -> Result<(), String> {
    let resolved = git(
        repo,
        &["rev-parse", "--verify", &format!("{value}^{{commit}}")],
    )?
    .trim()
    .to_string();
    if resolved != value {
        return Err(format!("{name} is not an exact commit object"));
    }
    Ok(())
}

fn require_ancestor(
    repo: &Path,
    ancestor: &str,
    descendant: &str,
    message: &str,
) -> Result<(), String> {
    let output = git_command(repo, ["merge-base", "--is-ancestor", ancestor, descendant])
        .output()
        .map_err(|error| format!("failed to test ancestry: {error}"))?;
    match output.status.code() {
        Some(0) => Ok(()),
        Some(1) => Err(message.to_string()),
        _ => Err(format!(
            "git merge-base --is-ancestor failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )),
    }
}

fn git(repo: &Path, args: &[&str]) -> Result<String, String> {
    let output = git_command(repo, args.iter().copied())
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

fn git_bytes(repo: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let output = git_command(repo, args.iter().copied())
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

fn git_command<'a>(repo: &Path, args: impl IntoIterator<Item = &'a str>) -> Command {
    let mut command = Command::new("git");
    command
        .current_dir(repo)
        .arg("--no-replace-objects")
        .args(args);
    command
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
    use std::time::{SystemTime, UNIX_EPOCH};

    static CWD_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn fixture_root(label: &str) -> Result<PathBuf, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        for attempt in 0..32u32 {
            let root = std::env::temp_dir().join(format!("ripr-{label}-{stamp}-{attempt}"));
            match fs::create_dir(&root) {
                Ok(()) => return Ok(root),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.to_string()),
            }
        }
        Err(format!(
            "could not allocate collision-resistant {label} fixture path"
        ))
    }

    fn write_release_metadata_fixture(
        root: &Path,
        version: &str,
        workspace_inherited: bool,
    ) -> Result<(), String> {
        fs::create_dir_all(root.join("crates/ripr")).map_err(|error| error.to_string())?;
        fs::create_dir_all(root.join("editors/vscode")).map_err(|error| error.to_string())?;
        let workspace = if workspace_inherited {
            format!(
                "[workspace]\nmembers = [\"crates/ripr\"]\n[workspace.package]\nversion = \"{version}\"\n[workspace.dependencies]\nserde = \"1\"\n"
            )
        } else {
            "[workspace]\nmembers = [\"crates/ripr\"]\n".to_string()
        };
        let crate_manifest = if workspace_inherited {
            "[package]\nname = \"ripr\"\nversion.workspace = true\n[dependencies]\nrayon = \"1\"\n"
                .to_string()
        } else {
            format!(
                "[package]\nname = \"ripr\"\nversion = \"{version}\"\n[dependencies]\nserde = \"1\"\n"
            )
        };
        let lock = format!(
            "version = 3\n[[package]]\nname = \"ripr\"\nversion = \"{version}\"\ndependencies = [\"serde\"]\n"
        );
        let package = format!(
            "{{\"name\":\"ripr\",\"version\":\"{version}\",\"scripts\":{{\"compile\":\"tsc\"}}}}\n"
        );
        let package_lock = format!(
            "{{\"name\":\"ripr\",\"version\":\"{version}\",\"lockfileVersion\":3,\"packages\":{{\"\":{{\"name\":\"ripr\",\"version\":\"{version}\",\"dependencies\":{{\"vscode-languageclient\":\"^9\"}}}}}}}}\n"
        );
        fs::write(root.join("Cargo.toml"), workspace).map_err(|error| error.to_string())?;
        fs::write(root.join("crates/ripr/Cargo.toml"), crate_manifest)
            .map_err(|error| error.to_string())?;
        fs::write(root.join("Cargo.lock"), lock).map_err(|error| error.to_string())?;
        fs::write(root.join("editors/vscode/package.json"), package)
            .map_err(|error| error.to_string())?;
        fs::write(root.join("editors/vscode/package-lock.json"), package_lock)
            .map_err(|error| error.to_string())?;
        fs::write(root.join("CHANGELOG.md"), "# Changelog\n").map_err(|error| error.to_string())?;
        Ok(())
    }

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
    fn merge_base_selection_rejects_ambiguous_history() -> Result<(), String> {
        if unique_merge_base("base-one\nbase-two\n").is_ok() {
            return Err("ambiguous merge-base output was accepted".into());
        }
        if unique_merge_base("").is_ok() {
            return Err("missing merge-base output was accepted".into());
        }
        if unique_merge_base("base\n")? != "base" {
            return Err("unique merge-base output changed".into());
        }
        Ok(())
    }

    #[test]
    fn protected_candidate_tag_ref_requires_exact_version_and_parent_binding() -> Result<(), String>
    {
        let version = "0.11.0";
        let source = "0000000000000000000000000000000000000000";
        let parent = "1111111111111111111111111111111111111111";
        let valid_preflight = |reference: &str| {
            serde_json::json!({
                "schema": PREFLIGHT_SCHEMA,
                "mode": "two_parent_join",
                "source_parent": source,
                "swarm_parent": parent,
                "swarm_ref": reference,
                "swarm_ref_sha": parent,
                "source_main": source,
                "merge_base": source,
                "source_repository": {"common_dir_verified":true,"root_verified":true,"remote_verified":true},
                "swarm_repository": {"common_dir_verified":true,"root_verified":true,"remote_verified":true},
                "source_range": {},
                "swarm_range": {},
                "dry_merge": {"reviewed_resolved_tree": source,"reviewed_resolved_tree_verified":true},
                "version_state": {"requested_version": version},
                "invalidation_rules": []
            })
        };
        let expected = format!("refs/tags/ripr-release-{version}-{parent}");
        validate_preflight(&valid_preflight(&expected), source)?;

        for reference in [
            "refs/ripr/release-0.11.0-1111111111111111111111111111111111111111",
            "ripr-release-0.11.0-1111111111111111111111111111111111111111",
            "refs/heads/main",
            "refs/tags/ripr-release-0.10.0-1111111111111111111111111111111111111111",
            "refs/tags/ripr-release-0.11.0-2222222222222222222222222222222222222222",
            "refs/tags/ripr-release-other-1111111111111111111111111111111111111111",
        ] {
            if validate_preflight(&valid_preflight(reference), source).is_ok() {
                return Err(format!("invalid candidate ref was accepted: {reference}"));
            }
        }
        Ok(())
    }

    #[test]
    fn blank_output_directory_is_rejected() -> Result<(), String> {
        let args = vec![
            "verify".to_string(),
            "--preflight".to_string(),
            "preflight.json".to_string(),
            "--resolution-manifest".to_string(),
            "manifest.json".to_string(),
            "--join-head".to_string(),
            "0123456789abcdef0123456789abcdef01234567".to_string(),
            "--source-main".to_string(),
            "1123456789abcdef0123456789abcdef01234567".to_string(),
            "--out".to_string(),
            "   ".to_string(),
        ];
        if parse_args(&args).is_ok() {
            return Err("blank --out was accepted".into());
        }
        Ok(())
    }

    #[test]
    fn ordered_digest_is_not_commutative() -> Result<(), String> {
        if digest_lines(&["a".into(), "b".into()]) == digest_lines(&["b".into(), "a".into()]) {
            return Err("ordered digest ignored order".into());
        }
        Ok(())
    }

    #[test]
    fn producer_receipt_and_empty_ancestry_serialization_are_stable() -> Result<(), String> {
        let bytes = br#"{
  "mode": "two_parent_join",
  "schema": "ripr.source_promotion_preflight.v1"
}
"#;
        if digest_bytes(bytes).is_empty() {
            return Err("producer receipt digest is empty".into());
        }
        if digest_lines(&[]) != digest_bytes(b"") {
            return Err("empty ancestry must preserve producer empty-stream semantics".into());
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
    fn manifest_accepts_only_canonical_v1_field_names() -> Result<(), String> {
        let preflight = serde_json::json!({
            "schema": PREFLIGHT_SCHEMA,
            "source_parent":"0000000000000000000000000000000000000000",
            "swarm_parent":"1111111111111111111111111111111111111111",
            "merge_base":"2222222222222222222222222222222222222222",
            "dry_merge":{"reviewed_resolved_tree":"3333333333333333333333333333333333333333","reviewed_resolved_tree_verified":true,"conflicts":["x"]},
            "source_survivor_candidates":[], "swarm_authority_resolution_candidates":[]
        });
        let digest = digest_bytes(
            serde_json::to_string(&preflight)
                .map_err(|error| error.to_string())?
                .as_bytes(),
        );
        let canonical = serde_json::json!({
            "schema": RESOLUTION_SCHEMA, "preflight_sha256": digest,
            "source_parent":"0000000000000000000000000000000000000000",
            "swarm_parent":"1111111111111111111111111111111111111111",
            "merge_base":"2222222222222222222222222222222222222222",
            "reviewed_join_tree":"3333333333333333333333333333333333333333",
            "dispositions":[{"kind":"conflict","key":"x","disposition":"source","rationale":"reviewed","evidence":"review"}]
        });
        validate_manifest(&canonical, &preflight, &digest)?;
        let mut missing = canonical.clone();
        missing["dispositions"] = serde_json::json!([]);
        if validate_manifest(&missing, &preflight, &digest).is_ok() {
            return Err("missing inventory row accepted".into());
        }
        let mut extra = canonical.clone();
        extra["dispositions"].as_array_mut().ok_or_else(|| "dispositions fixture malformed".to_string())?.push(serde_json::json!({
            "kind":"conflict", "key":"extra", "disposition":"source", "rationale":"extra", "evidence":"extra"
        }));
        if validate_manifest(&extra, &preflight, &digest).is_ok() {
            return Err("extra inventory row accepted".into());
        }
        for (canonical_name, alias) in [
            ("preflight_sha256", "preflight_digest"),
            ("reviewed_join_tree", "reviewed_resolved_tree"),
            ("dispositions", "resolutions"),
        ] {
            let mut aliased = canonical.clone();
            let value = aliased
                .as_object_mut()
                .and_then(|object| object.remove(canonical_name))
                .ok_or_else(|| format!("missing canonical field {canonical_name}"))?;
            aliased[alias] = value;
            if validate_manifest(&aliased, &preflight, &digest).is_ok() {
                return Err(format!("noncanonical alias accepted: {alias}"));
            }
        }
        let mut row_alias = canonical.clone();
        let key = row_alias["dispositions"][0]["key"].take();
        row_alias["dispositions"][0]["path"] = key;
        if validate_manifest(&row_alias, &preflight, &digest).is_ok() {
            return Err("path alias accepted for resolution key".into());
        }
        let mut evidence_alias = canonical;
        let evidence = evidence_alias["dispositions"][0]["evidence"].take();
        evidence_alias["dispositions"][0]["evidence_ref"] = evidence;
        if validate_manifest(&evidence_alias, &preflight, &digest).is_ok() {
            return Err("evidence_ref alias accepted for resolution evidence".into());
        }
        Ok(())
    }

    #[test]
    fn range_and_main_reachability_fail_closed() -> Result<(), String> {
        let expected = serde_json::json!({
            "all_reachable_count": 1, "first_parent_count": 1,
            "all_reachable_sha256": digest_lines(&["a".into()]),
            "first_parent_ordered_sha256": digest_lines(&["a".into()])
        });
        let expected = expected
            .as_object()
            .ok_or_else(|| "range fixture malformed".to_string())?;
        compare_range(&(vec!["a".into()], vec!["a".into()]), expected, "source")?;
        if compare_range(&(vec!["b".into()], vec!["a".into()]), expected, "source").is_ok() {
            return Err("ancestry digest drift accepted".into());
        }
        if compare_range(
            &(vec!["a".into(), "b".into()], vec!["a".into()]),
            expected,
            "source",
        )
        .is_ok()
        {
            return Err("ancestry count drift accepted".into());
        }
        Ok(())
    }

    #[test]
    fn receipts_render_every_contract_field_and_rejections_are_structured() -> Result<(), String> {
        let options = Options {
            repo: PathBuf::new(),
            preflight: PathBuf::new(),
            manifest: PathBuf::new(),
            join: "0123456789abcdef0123456789abcdef01234567".into(),
            source_main: "1123456789abcdef0123456789abcdef01234567".into(),
            main: None,
            out: PathBuf::new(),
        };
        let rejected = failure_report(&options, "synthetic failure");
        let markdown = render_markdown(&rejected)?;
        for field in [
            "failure_reasons",
            "invalidation_rules",
            "non_claims",
            "main_reachability",
            "caller_state_mutated",
        ] {
            if !markdown.contains(field) {
                return Err(format!("Markdown omitted {field}"));
            }
        }
        if rejected.get("status").and_then(Value::as_str) != Some("rejected") {
            return Err("rejection receipt did not carry rejected status".into());
        }
        if rejected["checks"]["main_reachability"] != "not_run" {
            return Err("omitted main head was not explicit not_run".into());
        }
        Ok(())
    }

    #[test]
    fn git_object_view_ignores_replacement_refs() -> Result<(), String> {
        let _guard = CWD_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .map_err(|error| format!("cwd test lock poisoned: {error}"))?;
        let root = fixture_root("replace")?;
        let result = (|| {
            test_git(&root, &["init", "--quiet"])?;
            test_git(&root, &["config", "user.email", "test@example.invalid"])?;
            test_git(&root, &["config", "user.name", "test"])?;
            fs::write(root.join("base.txt"), "base\n").map_err(|e| e.to_string())?;
            test_git(&root, &["add", "base.txt"])?;
            test_git(&root, &["commit", "--quiet", "-m", "base"])?;
            let original = test_git_output(&root, &["rev-parse", "HEAD"])?;
            let original_tree =
                test_git_output(&root, &["rev-parse", &format!("{original}^{{tree}}")])?;
            fs::write(root.join("replacement.txt"), "replacement\n").map_err(|e| e.to_string())?;
            test_git(&root, &["add", "replacement.txt"])?;
            let tree = test_git_output(&root, &["write-tree"])?;
            let replacement = test_git_output_with_input(
                &root,
                &["commit-tree", &tree, "-p", &original],
                "replacement\n",
            )?;
            test_git(
                &root,
                &[
                    "update-ref",
                    &format!("refs/replace/{original}"),
                    &replacement,
                ],
            )?;
            let exact = git_exact_commit(&root, &original, "--join-head");
            let observed = git(&root, &["rev-parse", &format!("{original}^{{tree}}")]);
            exact?;
            if observed?.trim() != original_tree {
                return Err("replacement ref changed exact object view".into());
            }
            Ok(())
        })();
        let _ = fs::remove_dir_all(&root);
        result
    }

    #[test]
    fn metadata_main_and_caller_state_contracts_are_executable() -> Result<(), String> {
        let _guard = CWD_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .map_err(|error| format!("cwd test lock poisoned: {error}"))?;
        let root = fixture_root("metadata")?;
        fs::create_dir_all(root.join("crates/ripr"))
            .and_then(|_| fs::create_dir_all(root.join("editors/vscode")))
            .map_err(|error| error.to_string())?;
        let result = (|| {
            test_git(&root, &["init", "--quiet"])?;
            test_git(&root, &["config", "user.email", "test@example.invalid"])?;
            test_git(&root, &["config", "user.name", "test"])?;
            write_release_metadata_fixture(&root, "0.10.1", false)?;
            test_git(&root, &["add", "."])?;
            test_git(&root, &["commit", "--quiet", "-m", "base"])?;
            let source = test_git_output(&root, &["rev-parse", "HEAD"])?;
            write_release_metadata_fixture(&root, "0.10.1", true)?;
            test_git(&root, &["add", "."])?;
            test_git(
                &root,
                &["commit", "--quiet", "-m", "dependency-and-layout-change"],
            )?;
            let dependency_only = test_git_output(&root, &["rev-parse", "HEAD"])?;
            if verify_release_metadata(&root, &dependency_only, &source).is_err() {
                return Err(
                    "dependency/layout changes with unchanged effective versions were rejected"
                        .into(),
                );
            }
            write_release_metadata_fixture(&root, "0.10.2", true)?;
            test_git(&root, &["add", "."])?;
            test_git(&root, &["commit", "--quiet", "-m", "version-mutated"])?;
            let mutated = test_git_output(&root, &["rev-parse", "HEAD"])?;
            test_git(&root, &["reset", "--quiet", "--hard", &dependency_only])?;
            fs::write(root.join("CHANGELOG.md"), "# Changelog\nchanged\n")
                .map_err(|error| error.to_string())?;
            test_git(&root, &["add", "CHANGELOG.md"])?;
            test_git(&root, &["commit", "--quiet", "-m", "changelog-mutated"])?;
            let changelog_mutated = test_git_output(&root, &["rev-parse", "HEAD"])?;
            test_git(&root, &["reset", "--quiet", "--hard", &mutated])?;
            let before_refs = test_git_output(
                &root,
                &["for-each-ref", "--format=%(refname)=%(objectname)"],
            )?;
            let before_index = test_git_output(&root, &["diff", "--cached", "--binary"])?;
            let before_worktree =
                test_git_output(&root, &["status", "--porcelain=v2", "--branch"])?;
            let before_remotes = test_git_output(&root, &["remote", "-v"])?;
            if verify_release_metadata(&root, &source, &source).is_err() {
                return Err("unchanged release metadata was rejected".into());
            }
            if verify_release_metadata(&root, &mutated, &source).is_ok() {
                return Err("governed release-version mutation was accepted".into());
            }
            if verify_release_metadata(&root, &changelog_mutated, &source).is_ok() {
                return Err("source-authoritative changelog mutation was accepted".into());
            }
            let tree = test_git_output(&root, &["rev-parse", &format!("{source}^{{tree}}")])?;
            let unrelated_main =
                test_git_output_with_input(&root, &["commit-tree", &tree], "unrelated main\n")?;
            if require_ancestor(
                &root,
                &mutated,
                &unrelated_main,
                "main unexpectedly reaches J",
            )
            .is_ok()
            {
                return Err("equivalent-tree main without J was accepted".into());
            }
            let after_refs = test_git_output(
                &root,
                &["for-each-ref", "--format=%(refname)=%(objectname)"],
            )?;
            let after_index = test_git_output(&root, &["diff", "--cached", "--binary"])?;
            let after_worktree = test_git_output(&root, &["status", "--porcelain=v2", "--branch"])?;
            let after_remotes = test_git_output(&root, &["remote", "-v"])?;
            if before_refs != after_refs
                || before_index != after_index
                || before_worktree != after_worktree
                || before_remotes != after_remotes
            {
                return Err("verification changed caller repository state".into());
            }
            Ok(())
        })();
        let _ = fs::remove_dir_all(&root);
        result
    }

    #[test]
    fn synthetic_join_rejects_graph_and_tree_adversaries() -> Result<(), String> {
        let _guard = CWD_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .map_err(|error| format!("cwd test lock poisoned: {error}"))?;
        let root = fixture_root("source-verify")?;
        let result = (|| {
            test_git(&root, &["init", "--quiet"])?;
            test_git(&root, &["config", "user.email", "test@example.invalid"])?;
            test_git(&root, &["config", "user.name", "test"])?;
            fs::write(root.join("base.txt"), "base\n").map_err(|error| error.to_string())?;
            fs::create_dir_all(root.join("crates/ripr")).map_err(|error| error.to_string())?;
            fs::create_dir_all(root.join("editors/vscode")).map_err(|error| error.to_string())?;
            write_release_metadata_fixture(&root, "0.10.1", false)?;
            test_git(&root, &["add", "."])?;
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
                "swarm_parent": swarm, "swarm_ref": format!("refs/tags/ripr-release-0.11.0-{swarm}"), "swarm_ref_sha": swarm,
                "source_main": source, "merge_base": base,
                "source_repository": {"common_dir_verified":true,"root_verified":true,"remote_verified":true},
                "swarm_repository": {"common_dir_verified":true,"root_verified":true,"remote_verified":true},
                "source_range": range_json(&source_range), "swarm_range": range_json(&swarm_range),
                "dry_merge": {"reviewed_resolved_tree": tree, "reviewed_resolved_tree_verified":true, "preview_tree": "", "conflicts":[]},
                "source_survivor_candidates":[], "swarm_authority_resolution_candidates":[],
                "version_state":{"requested_version":"0.11.0"}, "invalidation_rules":[]
            });
            let preflight_bytes =
                serde_json::to_vec(&preflight).map_err(|error| error.to_string())?;
            fs::write(root.join("preflight.json"), &preflight_bytes)
                .map_err(|error| error.to_string())?;
            let manifest = serde_json::json!({
                "schema": RESOLUTION_SCHEMA,
                "preflight_sha256": digest_bytes(&preflight_bytes),
                "source_parent": source,
                "swarm_parent": swarm,
                "merge_base": base,
                "reviewed_join_tree": tree,
                "dispositions": []
            });
            fs::write(
                root.join("manifest.json"),
                serde_json::to_vec(&manifest).map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
            let end_to_end_out = root.join("receipt");
            let rejected_options = Options {
                repo: root.clone(),
                preflight: root.join("preflight.json"),
                manifest: root.join("manifest.json"),
                join: repair.clone(),
                source_main: source.clone(),
                main: None,
                out: end_to_end_out.clone(),
            };
            let rejected_reason = match verify(&rejected_options) {
                Ok(_) => return Err("end-to-end appended repair was accepted".into()),
                Err(reason) => reason,
            };
            write_report(
                &end_to_end_out,
                &failure_report(&rejected_options, &rejected_reason),
            )?;
            let end_to_end_receipt = fs::read_to_string(end_to_end_out.join(REPORT_JSON))
                .map_err(|error| error.to_string())?;
            if !end_to_end_receipt.contains("\"status\": \"rejected\"") {
                return Err("end-to-end rejection receipt was not emitted".into());
            }
            let valid_out = root.join("valid-receipt");
            let valid_receipt_options = Options {
                repo: root.clone(),
                preflight: root.join("preflight.json"),
                manifest: root.join("manifest.json"),
                join: join.clone(),
                source_main: source.clone(),
                main: None,
                out: valid_out.clone(),
            };
            let valid_report = verify(&valid_receipt_options)?;
            write_report(&valid_out, &valid_report)?;
            let valid_json = fs::read_to_string(valid_out.join(REPORT_JSON))
                .map_err(|error| error.to_string())?;
            let valid_markdown =
                fs::read_to_string(valid_out.join(REPORT_MD)).map_err(|error| error.to_string())?;
            if !valid_json.contains("\"status\": \"verified\"")
                || !valid_markdown.contains("## Structured receipt")
            {
                return Err("valid end-to-end receipt was incomplete".into());
            }
            let options = Options {
                repo: root.clone(),
                preflight: PathBuf::new(),
                manifest: PathBuf::new(),
                join: repair,
                source_main: source.clone(),
                main: None,
                out: PathBuf::new(),
            };
            let valid_options = Options {
                join: join.clone(),
                source_main: source.clone(),
                ..options.clone()
            };
            verify_graph(&valid_options, &preflight)?;
            let squash = test_git_output_with_input(
                &root,
                &["commit-tree", &tree, "-p", &source],
                "squash\n",
            )?;
            let squash_options = Options {
                join: squash,
                ..valid_options.clone()
            };
            if verify_graph(&squash_options, &preflight).is_ok() {
                return Err("tree-equivalent squash was accepted".into());
            }
            let cherry_pick = test_git_output_with_input(
                &root,
                &["commit-tree", &tree, "-p", &base],
                "cherry-pick\n",
            )?;
            if verify_graph(
                &Options {
                    join: cherry_pick,
                    ..valid_options.clone()
                },
                &preflight,
            )
            .is_ok()
            {
                return Err("rebased/cherry-picked history was accepted".into());
            }
            let substituted_parent = test_git_output_with_input(
                &root,
                &["commit-tree", &tree, "-p", &source, "-p", &base],
                "substituted parent\n",
            )?;
            if verify_graph(
                &Options {
                    join: substituted_parent,
                    ..valid_options.clone()
                },
                &preflight,
            )
            .is_ok()
            {
                return Err("substituted parent history was accepted".into());
            }
            let mut preview_substitution = preflight.clone();
            preview_substitution["dry_merge"]["preview_tree"] = Value::String(tree.clone());
            preview_substitution["dry_merge"]["conflicts"] = serde_json::json!(["synthetic"]);
            if verify_graph(&valid_options, &preview_substitution).is_ok() {
                return Err("automatic preview tree substitution was accepted".into());
            }
            let mut wrong_tree = preflight.clone();
            wrong_tree["dry_merge"]["reviewed_resolved_tree"] =
                Value::String("4444444444444444444444444444444444444444".into());
            if verify_graph(&valid_options, &wrong_tree).is_ok() {
                return Err("reviewed-tree mismatch was accepted".into());
            }
            let mut reversed = preflight.clone();
            reversed["source_parent"] = Value::String(swarm.clone());
            reversed["swarm_parent"] = Value::String(source.clone());
            if verify_graph(&valid_options, &reversed).is_ok() {
                return Err("reversed parent identities were accepted".into());
            }
            let failed = verify_graph(&options, &preflight).is_err();
            if !failed {
                return Err("appended repair commit was accepted as J".to_string());
            }
            Ok(())
        })();
        let _ = fs::remove_dir_all(&root);
        result
    }

    #[test]
    fn parse_failures_write_rejection_receipts_when_output_is_known() -> Result<(), String> {
        let root = fixture_root("parse-receipt")?;
        let result = (|| {
            let args = vec![
                "verify".to_string(),
                "--join-head".to_string(),
                "not-a-sha".to_string(),
                "--source-main".to_string(),
                "not-a-sha".to_string(),
                "--out".to_string(),
                root.to_string_lossy().into_owned(),
            ];
            if source_promotion_verify(&args).is_ok() {
                return Err("invalid identity unexpectedly accepted".into());
            }
            let receipt =
                fs::read_to_string(root.join(REPORT_JSON)).map_err(|error| error.to_string())?;
            let report: Value =
                serde_json::from_str(&receipt).map_err(|error| error.to_string())?;
            if report["status"] != "rejected" || report["checks"]["main_reachability"] != "not_run"
            {
                return Err("parse rejection receipt was not structured".into());
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
