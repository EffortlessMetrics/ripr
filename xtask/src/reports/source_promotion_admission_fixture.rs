//! Deterministic, source-owned fixtures for the hosted promotion-admission workflow.
//!
//! Fixture repository state is isolated from the invoking checkout. The positive
//! route reviews the exact source tree. The negative route adds the retained J5
//! network-policy under-description without weakening or replacing the production
//! resolved-tree validator.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const VERSION: &str = "0.11.0";
const PACKET_INDEX: &str = "packet-index.json";
const VALIDATION_REPORT: &str = "resolved-tree-validation.json";
const ADMISSION_REPORT: &str = "resolved-tree-admission.json";
const INTEGRATION_INDEX_SCHEMA: &str = "ripr.source_promotion_integration_index.v1";
const QUALIFICATION_SCHEMA: &str = "ripr.source_promotion_tree_qualification.v1";
const QUALIFICATION_LANES: &[&str] = &[
    "editor_package_linux",
    "editor_package_windows",
    "rust_product",
    "source_governance",
    "source_survivors",
    "trusted_product_journeys",
    "untrusted_workspace_contract",
    "w7_product",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SyntheticProfile {
    Positive,
    J5Negative,
}

impl SyntheticProfile {
    fn as_str(self) -> &'static str {
        match self {
            Self::Positive => "positive_synthetic",
            Self::J5Negative => "j5_negative",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LocatorMaterial {
    pub(crate) path: PathBuf,
    pub(crate) sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SyntheticFixture {
    pub(crate) profile: SyntheticProfile,
    pub(crate) repository: PathBuf,
    pub(crate) source_parent: String,
    pub(crate) swarm_parent: String,
    pub(crate) reviewed_tree: String,
    pub(crate) reviewed_tree_carrier: String,
    pub(crate) protected_w7_ref: String,
    pub(crate) preflight: LocatorMaterial,
    pub(crate) resolution: LocatorMaterial,
    pub(crate) validation_packet_index: LocatorMaterial,
    pub(crate) integration_index: LocatorMaterial,
    pub(crate) fixture_identity: String,
    pub(crate) refs_after_setup: BTreeMap<String, String>,
    pub(crate) refs_after_validation: BTreeMap<String, String>,
}

/// Materialize one source-owned fixture without moving the invoking checkout.
pub(crate) fn prepare_source_owned_fixture(
    source_checkout: &Path,
    workspace: &Path,
    profile: SyntheticProfile,
) -> Result<SyntheticFixture, String> {
    let workspace = if workspace.is_absolute() {
        workspace.to_path_buf()
    } else {
        source_checkout.join(workspace)
    };
    let workspace = workspace.as_path();
    require_empty_destination(workspace)?;
    fs::create_dir_all(workspace)
        .map_err(|error| format!("failed to create fixture workspace: {error}"))?;
    let repository = workspace.join("fixture-repository");
    run_checked(
        source_checkout,
        Path::new("git"),
        &[
            "clone",
            "--local",
            "--no-hardlinks",
            "--no-checkout",
            "--quiet",
            path_text(source_checkout)?,
            path_text(&repository)?,
        ],
        &[],
        "clone isolated source fixture",
    )?;

    configure_fixture_repository(&repository)?;
    let source_parent = git_output(source_checkout, &["rev-parse", "HEAD"])?;
    validate_hex(&source_parent, 40, "source parent")?;
    run_git(
        &repository,
        &["checkout", "--quiet", "--detach", &source_parent],
    )?;
    run_git(
        &repository,
        &["update-ref", "refs/heads/main", &source_parent],
    )?;
    let source_tree = git_output(
        &repository,
        &["rev-parse", &format!("{source_parent}^{{tree}}")],
    )?;
    let j5_tree = build_j5_tree(&repository, &source_tree, workspace)?;
    let swarm_parent = deterministic_w7_commit(&repository, &source_parent, &j5_tree)?;
    if swarm_parent == source_parent {
        return Err("synthetic W7 commit must be distinct from SOURCE_PARENT".to_string());
    }
    let protected_w7_ref = format!("refs/tags/ripr-release-{VERSION}-{swarm_parent}");
    run_git(
        &repository,
        &["update-ref", &protected_w7_ref, &swarm_parent],
    )?;

    let reviewed_tree = match profile {
        SyntheticProfile::Positive => source_tree,
        SyntheticProfile::J5Negative => j5_tree,
    };
    let reviewed_tree_carrier = match profile {
        SyntheticProfile::Positive => source_parent.clone(),
        SyntheticProfile::J5Negative => swarm_parent.clone(),
    };
    let evidence = repository.join(".git/source-promotion-admission-fixture");
    fs::create_dir_all(&evidence)
        .map_err(|error| format!("failed to create fixture evidence root: {error}"))?;
    let preflight = write_preflight(
        &evidence,
        &source_parent,
        &swarm_parent,
        &reviewed_tree,
        &protected_w7_ref,
    )?;
    let resolution = write_resolution(
        &evidence,
        &source_parent,
        &swarm_parent,
        &reviewed_tree,
        &preflight.sha256,
    )?;

    let refs_after_setup = snapshot_refs(&repository)?;
    let validation_root = evidence.join("validation-packet");
    let validation_result = run_validator(
        &repository,
        &source_parent,
        &swarm_parent,
        &reviewed_tree,
        &preflight,
        &resolution,
        &validation_root,
    )?;
    require_validation_disposition(profile, &validation_result, &validation_root)?;
    let refs_after_validation = snapshot_refs(&repository)?;
    if refs_after_validation != refs_after_setup {
        return Err("source-owned fixture validator changed repository refs".to_string());
    }
    let validation_packet_index = material(&validation_root.join(PACKET_INDEX))?;

    let executable = std::env::current_exe()
        .map_err(|error| format!("failed to identify running xtask executable: {error}"))?;
    let executable_sha256 = digest_file(&executable, "running xtask executable")?;
    let integration_index = write_integration_packet(
        &evidence.join("integration"),
        &source_parent,
        &swarm_parent,
        &reviewed_tree,
        &preflight.sha256,
        &resolution.sha256,
        &executable_sha256,
    )?;
    let fixture_identity = digest_bytes(
        format!(
            "{}:{source_parent}:{swarm_parent}:{reviewed_tree}:{}:{}:{}:{}",
            profile.as_str(),
            preflight.sha256,
            resolution.sha256,
            validation_packet_index.sha256,
            integration_index.sha256,
        )
        .as_bytes(),
    );

    Ok(SyntheticFixture {
        profile,
        repository,
        source_parent,
        swarm_parent,
        reviewed_tree,
        reviewed_tree_carrier,
        protected_w7_ref,
        preflight,
        resolution,
        validation_packet_index,
        integration_index,
        fixture_identity,
        refs_after_setup,
        refs_after_validation,
    })
}

/// Create the terminal qualification only after admission exists, so its
/// admission packet and receipt bindings are genuine rather than predicted.
pub(crate) fn write_bound_qualification(
    fixture: &SyntheticFixture,
    admission_packet: &Path,
) -> Result<LocatorMaterial, String> {
    let admission_index = material(&admission_packet.join(PACKET_INDEX))?;
    let admission = read_json(
        &admission_packet.join(ADMISSION_REPORT),
        "admission receipt",
    )?;
    if string(&admission, "status") != Some("admitted") {
        return Err("qualification requires one admitted controller packet".to_string());
    }
    for (field, expected) in [
        ("source_parent", fixture.source_parent.as_str()),
        ("swarm_parent", fixture.swarm_parent.as_str()),
        ("join_tree", fixture.reviewed_tree.as_str()),
        ("preflight_sha256", fixture.preflight.sha256.as_str()),
        (
            "resolution_manifest_sha256",
            fixture.resolution.sha256.as_str(),
        ),
    ] {
        if string(&admission, field) != Some(expected) {
            return Err(format!(
                "admission receipt moved fixture identity field {field}"
            ));
        }
    }
    let admission_receipt = material(&admission_packet.join(ADMISSION_REPORT))?;
    let validation_receipt =
        packet_member_digest(&fixture.validation_packet_index.path, VALIDATION_REPORT)?;
    let network_policy_receipt = integration_receipt_digest(
        &fixture.integration_index.path,
        "network_policy_integration",
    )?;
    let lanes = QUALIFICATION_LANES
        .iter()
        .map(|name| {
            serde_json::json!({
                "name": name,
                "state": "passed",
                "evidence_sha256": digest_bytes(
                    format!("{}:{name}", fixture.fixture_identity).as_bytes()
                ),
            })
        })
        .collect::<Vec<_>>();
    let qualification = serde_json::json!({
        "schema": QUALIFICATION_SCHEMA,
        "status": "qualified",
        "source_parent": fixture.source_parent,
        "swarm_parent": fixture.swarm_parent,
        "join_tree": fixture.reviewed_tree,
        "preflight_sha256": fixture.preflight.sha256,
        "resolution_manifest_sha256": fixture.resolution.sha256,
        "admission_packet_index_sha256": admission_index.sha256,
        "admission_receipt_sha256": admission_receipt.sha256,
        "resolved_tree_validation_receipt_sha256": validation_receipt,
        "network_policy_receipt_sha256": network_policy_receipt,
        "promotion_ref_mutation_attempted": false,
        "lanes": lanes,
        "failure_reasons": [],
    });
    write_json(
        &fixture
            .repository
            .join(".git/source-promotion-admission-fixture/tree-qualification.json"),
        &qualification,
        "tree qualification receipt",
    )
}

fn configure_fixture_repository(repo: &Path) -> Result<(), String> {
    for (key, value) in [
        ("user.name", "RIPR Source Promotion Fixture"),
        ("user.email", "source-promotion-fixture@invalid"),
        ("commit.gpgsign", "false"),
        ("tag.gpgSign", "false"),
        ("core.autocrlf", "false"),
    ] {
        run_git(repo, &["config", key, value])?;
    }
    Ok(())
}

fn build_j5_tree(repo: &Path, source_tree: &str, workspace: &Path) -> Result<String, String> {
    let index = workspace.join("j5.index");
    let index_text = path_text(&index)?;
    run_checked(
        repo,
        Path::new("git"),
        &["read-tree", source_tree],
        &[("GIT_INDEX_FILE", index_text)],
        "seed J5 tree index",
    )?;
    let current_ledger = git_bytes(repo, &["show", "HEAD:policy/network_allowlist.txt"])?;
    let mut ledger = String::from_utf8(current_ledger)
        .map_err(|error| format!("network allowlist is not UTF-8: {error}"))?;
    if !ledger.ends_with('\n') {
        ledger.push('\n');
    }
    ledger.push_str(
        ".github/workflows/stale-network-surface.yml|curl|3|source|stale zero-count row\n",
    );
    let additions = [
        ("policy/network_allowlist.txt", ledger),
        (
            ".github/workflows/server-archive-qualification.yml",
            "name: server archive qualification\n# retained J5 fixture: curl\n".to_string(),
        ),
        (
            "crates/ripr/src/output/perl_gap_record_projection.rs",
            repeated_literal("// retained J5 fixture: curl", 5),
        ),
        (
            "xtask/src/tests.rs",
            repeated_literal("// retained J5 fixture: curl", 2),
        ),
        (
            ".github/workflows/stale-network-surface.yml",
            "name: stale network surface\n".to_string(),
        ),
    ];
    for (path, contents) in additions {
        let blob = hash_blob(repo, contents.as_bytes())?;
        let cache = format!("100644,{blob},{path}");
        run_checked(
            repo,
            Path::new("git"),
            &["update-index", "--add", "--cacheinfo", &cache],
            &[("GIT_INDEX_FILE", index_text)],
            "stage J5 fixture blob",
        )?;
    }
    let tree = run_output(
        repo,
        Path::new("git"),
        &["write-tree"],
        &[("GIT_INDEX_FILE", index_text)],
        "write J5 tree",
    )?;
    fs::remove_file(&index).map_err(|error| format!("failed to remove J5 index: {error}"))?;
    validate_hex(&tree, 40, "J5 tree")?;
    Ok(tree)
}

fn deterministic_w7_commit(repo: &Path, source_parent: &str, tree: &str) -> Result<String, String> {
    let fixed = [
        ("GIT_AUTHOR_NAME", "RIPR Source Promotion Fixture"),
        ("GIT_AUTHOR_EMAIL", "source-promotion-fixture@invalid"),
        ("GIT_AUTHOR_DATE", "2000-01-01T00:00:00+00:00"),
        ("GIT_COMMITTER_NAME", "RIPR Source Promotion Fixture"),
        ("GIT_COMMITTER_EMAIL", "source-promotion-fixture@invalid"),
        ("GIT_COMMITTER_DATE", "2000-01-01T00:00:00+00:00"),
    ];
    let commit = run_output_with_input(
        repo,
        Path::new("git"),
        &["commit-tree", tree, "-p", source_parent],
        &fixed,
        b"test(promotion): deterministic protected W7 fixture\n",
        "create deterministic W7 commit",
    )?;
    validate_hex(&commit, 40, "synthetic W7 commit")?;
    Ok(commit)
}

/// Verify that an immutable commit carries the reviewed tree with the exact
/// ordered source and W7 ancestry required by source promotion.
pub(crate) fn verify_reviewed_tree_carrier(
    repo: &Path,
    carrier: &str,
    source_parent: &str,
    swarm_parent: &str,
    reviewed_tree: &str,
) -> Result<(), String> {
    validate_hex(carrier, 40, "reviewed-tree carrier")?;
    validate_hex(source_parent, 40, "reviewed-tree source parent")?;
    validate_hex(swarm_parent, 40, "reviewed-tree W7 parent")?;
    validate_hex(reviewed_tree, 40, "reviewed tree")?;

    let resolved_carrier = git_output(repo, &["rev-parse", &format!("{carrier}^{{commit}}")])?;
    if resolved_carrier != carrier {
        return Err("reviewed-tree carrier did not resolve to the exact commit".to_string());
    }
    let ancestry = git_output(repo, &["rev-list", "--parents", "-n", "1", carrier])?;
    let expected_ancestry = format!("{carrier} {source_parent} {swarm_parent}");
    if ancestry != expected_ancestry {
        return Err(format!(
            "reviewed-tree carrier ancestry mismatch: expected {expected_ancestry}, observed {ancestry}"
        ));
    }
    let observed_tree = git_output(repo, &["rev-parse", &format!("{carrier}^{{tree}}")])?;
    if observed_tree != reviewed_tree {
        return Err(format!(
            "reviewed-tree carrier tree mismatch: expected {reviewed_tree}, observed {observed_tree}"
        ));
    }
    Ok(())
}

fn write_preflight(
    root: &Path,
    source: &str,
    swarm: &str,
    tree: &str,
    swarm_ref: &str,
) -> Result<LocatorMaterial, String> {
    let value = serde_json::json!({
        "schema": "ripr.source_promotion_preflight.v1",
        "mode": "two_parent_join",
        "source_parent": source,
        "source_main": source,
        "swarm_parent": swarm,
        "swarm_ref": swarm_ref,
        "swarm_ref_sha": swarm,
        "merge_base": source,
        "source_repository": {"common_dir_verified": true, "root_verified": true, "remote_verified": true},
        "swarm_repository": {"common_dir_verified": true, "root_verified": true, "remote_verified": true},
        "dry_merge": {"reviewed_resolved_tree": tree, "reviewed_resolved_tree_verified": true, "conflicts": []},
        "source_range": {},
        "swarm_range": {},
        "version_state": {"requested_version": VERSION},
        "invalidation_rules": {},
        "source_survivor_candidates": [],
        "swarm_authority_resolution_candidates": [],
    });
    write_json(&root.join("preflight.json"), &value, "synthetic preflight")
}

fn write_resolution(
    root: &Path,
    source: &str,
    swarm: &str,
    tree: &str,
    preflight_sha256: &str,
) -> Result<LocatorMaterial, String> {
    let value = serde_json::json!({
        "schema": "ripr.source_promotion_resolution.v1",
        "preflight_sha256": preflight_sha256,
        "source_parent": source,
        "swarm_parent": swarm,
        "merge_base": source,
        "reviewed_join_tree": tree,
        "dispositions": [],
    });
    write_json(
        &root.join("resolution.json"),
        &value,
        "synthetic resolution manifest",
    )
}

fn run_validator(
    repo: &Path,
    source: &str,
    swarm: &str,
    tree: &str,
    preflight: &LocatorMaterial,
    resolution: &LocatorMaterial,
    out: &Path,
) -> Result<Output, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("failed to identify running xtask executable: {error}"))?;
    Command::new(executable)
        .current_dir(repo)
        .args([
            "source-promotion",
            "validate-resolved-tree",
            "--source-parent",
            source,
            "--swarm-parent",
            swarm,
            "--reviewed-tree",
            tree,
            "--preflight",
            path_text(&preflight.path)?,
            "--preflight-sha256",
            preflight.sha256.as_str(),
            "--resolution-manifest",
            path_text(&resolution.path)?,
            "--resolution-sha256",
            resolution.sha256.as_str(),
            "--out",
            path_text(out)?,
        ])
        .output()
        .map_err(|error| format!("failed to run production resolved-tree validator: {error}"))
}

fn require_validation_disposition(
    profile: SyntheticProfile,
    output: &Output,
    packet: &Path,
) -> Result<(), String> {
    let report = read_json(
        &packet.join(VALIDATION_REPORT),
        "resolved-tree validation receipt",
    )?;
    let observed = string(&report, "status").unwrap_or("missing");
    let expected = match profile {
        SyntheticProfile::Positive => "validated",
        SyntheticProfile::J5Negative => "rejected",
    };
    if observed != expected || output.status.success() != (expected == "validated") {
        return Err(format!(
            "{} validator disposition mismatch: expected {expected}, observed {observed}; {}",
            profile.as_str(),
            combined_output(output)
        ));
    }
    if profile == SyntheticProfile::J5Negative {
        require_exact_j5_failure(packet, &report)?;
    }
    Ok(())
}

fn require_exact_j5_failure(packet: &Path, report: &Value) -> Result<(), String> {
    let commands = report
        .get("commands")
        .and_then(Value::as_array)
        .ok_or_else(|| "J5 validation receipt is missing command evidence".to_string())?;
    let network = commands
        .first()
        .ok_or_else(|| "J5 validation receipt has no network-policy command".to_string())?;
    if string(network, "command") != Some("check-network-policy")
        || string(network, "state") != Some("failed")
    {
        return Err("J5 fixture did not fail at the production network-policy seam".to_string());
    }
    let stderr = string(network, "stderr_path")
        .ok_or_else(|| "J5 network-policy receipt has no stderr path".to_string())?;
    let text = fs::read_to_string(packet.join(stderr))
        .map_err(|error| format!("failed to read J5 network-policy evidence: {error}"))?;
    for fragment in [
        ".github/workflows/server-archive-qualification.yml",
        ".github/workflows/stale-network-surface.yml",
        "crates/ripr/src/output/perl_gap_record_projection.rs",
        "xtask/src/tests.rs",
    ] {
        if !text.contains(fragment) {
            return Err(format!("J5 network-policy evidence omitted {fragment}"));
        }
    }
    Ok(())
}

fn write_integration_packet(
    root: &Path,
    source: &str,
    swarm: &str,
    tree: &str,
    preflight: &str,
    resolution: &str,
    executable: &str,
) -> Result<LocatorMaterial, String> {
    fs::create_dir_all(root)
        .map_err(|error| format!("failed to create integration fixture root: {error}"))?;
    let mut rows = Vec::new();
    for (kind, schema, name) in [
        (
            "command_catalog_integration",
            "ripr.source_promotion_command_catalog_integration.v1",
            "command-catalog.json",
        ),
        (
            "network_policy_integration",
            "ripr.source_promotion_network_policy_integration.v1",
            "network-policy.json",
        ),
    ] {
        let receipt = serde_json::json!({
            "schema": schema,
            "status": "integrated",
            "source_parent": source,
            "swarm_parent": swarm,
            "join_tree": tree,
            "preflight_sha256": preflight,
            "resolution_manifest_sha256": resolution,
            "producer_source_sha": source,
            "producer_executable_sha256": executable,
            "ref_mutation_attempted": false,
            "failure_reasons": [],
        });
        let material = write_json(&root.join(name), &receipt, "typed integration receipt")?;
        rows.push(serde_json::json!({"kind": kind, "path": name, "sha256": material.sha256}));
    }
    let index = serde_json::json!({
        "schema": INTEGRATION_INDEX_SCHEMA,
        "status": "complete",
        "source_parent": source,
        "swarm_parent": swarm,
        "join_tree": tree,
        "preflight_sha256": preflight,
        "resolution_manifest_sha256": resolution,
        "required_kinds": ["command_catalog_integration", "network_policy_integration"],
        "receipts": rows,
        "failure_reasons": [],
    });
    write_json(
        &root.join("integration-index.json"),
        &index,
        "integration index",
    )
}

fn packet_member_digest(index_path: &Path, member: &str) -> Result<String, String> {
    let index = read_json(index_path, "packet index")?;
    index
        .get("files")
        .and_then(Value::as_array)
        .and_then(|rows| {
            rows.iter().find_map(|row| {
                (string(row, "path") == Some(member))
                    .then(|| string(row, "sha256").map(str::to_string))
                    .flatten()
            })
        })
        .ok_or_else(|| format!("packet index omitted {member}"))
}

fn integration_receipt_digest(index_path: &Path, kind: &str) -> Result<String, String> {
    let index = read_json(index_path, "integration index")?;
    index
        .get("receipts")
        .and_then(Value::as_array)
        .and_then(|rows| {
            rows.iter().find_map(|row| {
                (string(row, "kind") == Some(kind))
                    .then(|| string(row, "sha256").map(str::to_string))
                    .flatten()
            })
        })
        .ok_or_else(|| format!("integration index omitted {kind}"))
}

fn snapshot_refs(repo: &Path) -> Result<BTreeMap<String, String>, String> {
    let output = git_output(repo, &["for-each-ref", "--format=%(refname) %(objectname)"])?;
    output
        .lines()
        .map(|line| {
            let (name, object) = line
                .split_once(' ')
                .ok_or_else(|| format!("malformed ref snapshot row {line:?}"))?;
            Ok((name.to_string(), object.to_string()))
        })
        .collect()
}

fn require_empty_destination(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("failed to inspect fixture destination: {error}")),
        Ok(_) => Err(format!(
            "fixture destination already exists: {}",
            path.display()
        )),
    }
}

fn repeated_literal(literal: &str, count: usize) -> String {
    let mut value = std::iter::repeat_n(literal, count)
        .collect::<Vec<_>>()
        .join("\n");
    value.push('\n');
    value
}

fn hash_blob(repo: &Path, bytes: &[u8]) -> Result<String, String> {
    let blob = run_output_with_input(
        repo,
        Path::new("git"),
        &["hash-object", "-w", "--stdin"],
        &[],
        bytes,
        "write fixture blob",
    )?;
    validate_hex(&blob, 40, "fixture blob")?;
    Ok(blob)
}

fn git_bytes(repo: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .map_err(|error| format!("failed to run git: {error}"))?;
    if !output.status.success() {
        return Err(format!("git failed: {}", combined_output(&output)));
    }
    Ok(output.stdout)
}

fn run_git(repo: &Path, args: &[&str]) -> Result<(), String> {
    run_checked(repo, Path::new("git"), args, &[], "Git fixture command")
}

fn git_output(repo: &Path, args: &[&str]) -> Result<String, String> {
    run_output(repo, Path::new("git"), args, &[], "Git fixture command")
}

fn run_checked(
    cwd: &Path,
    program: &Path,
    args: &[&str],
    environment: &[(&str, &str)],
    label: &str,
) -> Result<(), String> {
    run_output(cwd, program, args, environment, label).map(|_| ())
}

fn run_output(
    cwd: &Path,
    program: &Path,
    args: &[&str],
    environment: &[(&str, &str)],
    label: &str,
) -> Result<String, String> {
    let output = Command::new(program)
        .current_dir(cwd)
        .args(args)
        .envs(environment.iter().copied())
        .output()
        .map_err(|error| format!("failed to run {label}: {error}"))?;
    if !output.status.success() {
        return Err(format!("{label} failed: {}", combined_output(&output)));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_string())
        .map_err(|error| format!("{label} output is not UTF-8: {error}"))
}

fn run_output_with_input(
    cwd: &Path,
    program: &Path,
    args: &[&str],
    environment: &[(&str, &str)],
    input: &[u8],
    label: &str,
) -> Result<String, String> {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = Command::new(program)
        .current_dir(cwd)
        .args(args)
        .envs(environment.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("failed to start {label}: {error}"))?;
    child
        .stdin
        .take()
        .ok_or_else(|| format!("{label} has no stdin"))?
        .write_all(input)
        .map_err(|error| format!("failed to write {label} input: {error}"))?;
    let output = child
        .wait_with_output()
        .map_err(|error| format!("failed to wait for {label}: {error}"))?;
    if !output.status.success() {
        return Err(format!("{label} failed: {}", combined_output(&output)));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_string())
        .map_err(|error| format!("{label} output is not UTF-8: {error}"))
}

fn write_json(path: &Path, value: &Value, label: &str) -> Result<LocatorMaterial, String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("{label} has no parent directory"))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create {label} parent: {error}"))?;
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("failed to encode {label}: {error}"))?;
    bytes.push(b'\n');
    fs::write(path, &bytes).map_err(|error| format!("failed to write {label}: {error}"))?;
    Ok(LocatorMaterial {
        path: path.to_path_buf(),
        sha256: digest_bytes(&bytes),
    })
}

fn material(path: &Path) -> Result<LocatorMaterial, String> {
    Ok(LocatorMaterial {
        path: path.to_path_buf(),
        sha256: digest_file(path, "fixture material")?,
    })
}

fn read_json(path: &Path, label: &str) -> Result<Value, String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("failed to read {label} {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("malformed {label}: {error}"))
}

fn string<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn digest_file(path: &Path, label: &str) -> Result<String, String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("failed to read {label} {}: {error}", path.display()))?;
    Ok(digest_bytes(&bytes))
}

fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn validate_hex(value: &str, length: usize, label: &str) -> Result<(), String> {
    if value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        Ok(())
    } else {
        Err(format!("{label} is not exact lowercase {length}-hex"))
    }
}

fn path_text(path: &Path) -> Result<&str, String> {
    path.to_str()
        .ok_or_else(|| format!("path is not UTF-8: {}", path.display()))
}

fn combined_output(output: &Output) -> String {
    format!(
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout).trim(),
        String::from_utf8_lossy(&output.stderr).trim(),
    )
}

#[cfg(test)]
mod tests {
    use super::{
        QUALIFICATION_LANES, SyntheticProfile, hash_blob, repeated_literal, run_output_with_input,
        validate_hex, verify_reviewed_tree_carrier,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST_ROOT: AtomicU64 = AtomicU64::new(0);

    #[derive(Debug, Eq, PartialEq)]
    struct CarrierGraph {
        source_parent: String,
        swarm_parent: String,
        source_tree: String,
        swarm_tree: String,
        reviewed_tree: String,
        carrier: String,
    }

    #[test]
    fn profile_names_are_closed_and_stable() -> Result<(), String> {
        if SyntheticProfile::Positive.as_str() != "positive_synthetic"
            || SyntheticProfile::J5Negative.as_str() != "j5_negative"
        {
            return Err("synthetic profile names moved".to_string());
        }
        Ok(())
    }

    #[test]
    fn retained_j5_literals_have_exact_requested_cardinality() -> Result<(), String> {
        let five = repeated_literal("curl", 5);
        let two = repeated_literal("curl", 2);
        if five.matches("curl").count() != 5 || two.matches("curl").count() != 2 {
            return Err("J5 fixture literal cardinality moved".to_string());
        }
        Ok(())
    }

    #[test]
    fn qualification_denominator_is_exact_and_ordered() -> Result<(), String> {
        let expected = [
            "editor_package_linux",
            "editor_package_windows",
            "rust_product",
            "source_governance",
            "source_survivors",
            "trusted_product_journeys",
            "untrusted_workspace_contract",
            "w7_product",
        ];
        if QUALIFICATION_LANES != expected {
            return Err("qualification lane denominator moved".to_string());
        }
        validate_hex(&"a".repeat(40), 40, "test identity")
    }

    #[test]
    fn reviewed_tree_carrier_binds_exact_ordered_parents_and_third_tree() -> Result<(), String> {
        let first_root = unique_test_root("first");
        let second_root = unique_test_root("second");
        let result = (|| {
            let first = build_carrier_graph(&first_root)?;
            let second = build_carrier_graph(&second_root)?;
            if first != second {
                return Err("reviewed-tree carrier graph is not deterministic".to_string());
            }
            if first.source_tree == first.swarm_tree
                || first.reviewed_tree == first.source_tree
                || first.reviewed_tree == first.swarm_tree
            {
                return Err(
                    "reviewed tree must be distinct from both distinct parent trees".to_string(),
                );
            }
            verify_reviewed_tree_carrier(
                &first_root,
                &first.carrier,
                &first.source_parent,
                &first.swarm_parent,
                &first.reviewed_tree,
            )?;
            if verify_reviewed_tree_carrier(
                &first_root,
                &first.carrier,
                &first.swarm_parent,
                &first.source_parent,
                &first.reviewed_tree,
            )
            .is_ok()
            {
                return Err("reversed reviewed-tree carrier parents were accepted".to_string());
            }
            if verify_reviewed_tree_carrier(
                &first_root,
                &first.carrier,
                &first.source_parent,
                &first.swarm_parent,
                &first.source_tree,
            )
            .is_ok()
            {
                return Err("parent tree was accepted as the reviewed carrier tree".to_string());
            }
            Ok(())
        })();
        let cleanup = remove_test_roots(&[&first_root, &second_root]);
        result.and(cleanup)
    }

    fn unique_test_root(label: &str) -> PathBuf {
        let sequence = NEXT_TEST_ROOT.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "ripr-reviewed-tree-carrier-{}-{sequence}-{label}",
            std::process::id()
        ))
    }

    fn remove_test_roots(roots: &[&Path]) -> Result<(), String> {
        for root in roots {
            match fs::remove_dir_all(root) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(format!(
                        "failed to remove reviewed-tree carrier test root {}: {error}",
                        root.display()
                    ));
                }
            }
        }
        Ok(())
    }

    fn build_carrier_graph(root: &Path) -> Result<CarrierGraph, String> {
        fs::create_dir_all(root).map_err(|error| {
            format!(
                "failed to create reviewed-tree carrier test root {}: {error}",
                root.display()
            )
        })?;
        super::run_git(root, &["init", "--quiet"])?;
        let source_blob = hash_blob(root, b"source-owned\n")?;
        let swarm_blob = hash_blob(root, b"w7-owned\n")?;
        let source_tree = make_tree(root, &[("source.txt", &source_blob)])?;
        let swarm_tree = make_tree(root, &[("w7.txt", &swarm_blob)])?;
        let reviewed_tree = make_tree(
            root,
            &[("source.txt", &source_blob), ("w7.txt", &swarm_blob)],
        )?;
        let empty_tree = make_tree(root, &[])?;
        let base = make_commit(root, &empty_tree, &[], b"base\n")?;
        let source_parent = make_commit(root, &source_tree, &[&base], b"source parent\n")?;
        let swarm_parent = make_commit(root, &swarm_tree, &[&base], b"W7 parent\n")?;
        let carrier = make_commit(
            root,
            &reviewed_tree,
            &[&source_parent, &swarm_parent],
            b"reviewed-tree carrier\n",
        )?;
        Ok(CarrierGraph {
            source_parent,
            swarm_parent,
            source_tree,
            swarm_tree,
            reviewed_tree,
            carrier,
        })
    }

    fn make_tree(root: &Path, entries: &[(&str, &str)]) -> Result<String, String> {
        let mut input = Vec::new();
        for (path, blob) in entries {
            input.extend_from_slice(format!("100644 blob {blob}\t{path}\n").as_bytes());
        }
        let tree = run_output_with_input(
            root,
            Path::new("git"),
            &["mktree"],
            &[],
            &input,
            "create reviewed-tree carrier test tree",
        )?;
        validate_hex(&tree, 40, "reviewed-tree carrier test tree")?;
        Ok(tree)
    }

    fn make_commit(
        root: &Path,
        tree: &str,
        parents: &[&str],
        message: &[u8],
    ) -> Result<String, String> {
        let fixed = [
            ("GIT_AUTHOR_NAME", "RIPR Reviewed Tree Fixture"),
            ("GIT_AUTHOR_EMAIL", "reviewed-tree-fixture@invalid"),
            ("GIT_AUTHOR_DATE", "2000-01-01T00:00:00+00:00"),
            ("GIT_COMMITTER_NAME", "RIPR Reviewed Tree Fixture"),
            ("GIT_COMMITTER_EMAIL", "reviewed-tree-fixture@invalid"),
            ("GIT_COMMITTER_DATE", "2000-01-01T00:00:00+00:00"),
        ];
        let mut args = vec!["commit-tree", tree];
        for parent in parents {
            args.extend(["-p", parent]);
        }
        let commit = run_output_with_input(
            root,
            Path::new("git"),
            &args,
            &fixed,
            message,
            "create reviewed-tree carrier test commit",
        )?;
        validate_hex(&commit, 40, "reviewed-tree carrier test commit")?;
        Ok(commit)
    }
}
