use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::*;
use crate::rust_judged_panel::host_run::ValidatedInputDigest;
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(windows)]
use super::{remove_replacement_backup, replacement_backup_path};

fn scratch(name: &str) -> Result<PathBuf, String> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("clock before epoch: {error}"))?
        .as_nanos();
    let root = std::env::temp_dir().join(format!("ripr-packet-{name}-{nonce}"));
    fs::create_dir_all(&root).map_err(|error| format!("create scratch root: {error}"))?;
    Ok(root)
}

#[test]
fn rust_judged_panel_packet_replaces_existing_current_on_second_publication() -> Result<(), String>
{
    let root = scratch("replace")?;
    let current = root.join("current.json");
    let first = root.join("first.tmp");
    let second = root.join("second.tmp");
    fs::write(&first, b"generation-a\n").map_err(|error| error.to_string())?;
    replace_file(&first, &current)?;
    fs::write(&second, b"generation-b\n").map_err(|error| error.to_string())?;
    replace_file(&second, &current)?;
    let actual = fs::read(&current).map_err(|error| error.to_string())?;
    let cleanup = fs::remove_dir_all(&root);
    if cleanup.is_err() || actual != b"generation-b\n" || first.exists() || second.exists() {
        return Err("second publication did not replace the prior current atomically".to_string());
    }
    Ok(())
}

#[test]
fn rust_judged_panel_packet_failed_current_replacement_preserves_previous_pointer()
-> Result<(), String> {
    let root = scratch("failed-replace")?;
    let current = root.join("current.json");
    let missing = root.join("missing.tmp");
    fs::write(&current, b"prior-generation\n").map_err(|error| error.to_string())?;
    if replace_file(&missing, &current).is_ok() {
        return Err("missing replacement unexpectedly succeeded".to_string());
    }
    let actual = fs::read(&current).map_err(|error| error.to_string())?;
    fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
    if actual == b"prior-generation\n" {
        Ok(())
    } else {
        Err("failed replacement destroyed the previous current pointer".to_string())
    }
}

#[cfg(windows)]
#[test]
fn rust_judged_panel_packet_reconciles_stale_backup_before_publication() -> Result<(), String> {
    let root = scratch("stale-backup")?;
    let current = root.join("current.json");
    let backup = replacement_backup_path(&current);
    let next = root.join("next.tmp");
    fs::write(&current, b"current-generation\n").map_err(|error| error.to_string())?;
    fs::write(&backup, b"stale-generation\n").map_err(|error| error.to_string())?;
    fs::write(&next, b"next-generation\n").map_err(|error| error.to_string())?;
    replace_file(&next, &current)?;
    let actual = fs::read(&current).map_err(|error| error.to_string())?;
    let cleanup = fs::remove_dir_all(&root);
    if cleanup.is_err() || actual != b"next-generation\n" || backup.exists() {
        return Err("stale backup was not reconciled after publication".to_string());
    }
    Ok(())
}

#[cfg(windows)]
#[test]
fn rust_judged_panel_packet_recovers_interrupted_staging_before_publication() -> Result<(), String>
{
    let root = scratch("recover-backup")?;
    let current = root.join("current.json");
    let backup = replacement_backup_path(&current);
    let next = root.join("next.tmp");
    fs::write(&backup, b"prior-generation\n").map_err(|error| error.to_string())?;
    fs::write(&next, b"next-generation\n").map_err(|error| error.to_string())?;
    replace_file(&next, &current)?;
    let actual = fs::read(&current).map_err(|error| error.to_string())?;
    let cleanup = fs::remove_dir_all(&root);
    if cleanup.is_err() || actual != b"next-generation\n" || backup.exists() {
        return Err("interrupted staging was not recovered before publication".to_string());
    }
    Ok(())
}

#[cfg(windows)]
#[test]
fn rust_judged_panel_packet_reports_backup_cleanup_failure() -> Result<(), String> {
    let root = scratch("cleanup-failure")?;
    let current = root.join("current.json");
    let backup = replacement_backup_path(&current);
    fs::write(&current, b"prior-generation\n").map_err(|error| error.to_string())?;
    fs::create_dir(&backup).map_err(|error| error.to_string())?;
    let result = remove_replacement_backup(&current, &backup);
    fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
    if result.is_ok() {
        return Err("backup cleanup failure was not reported after publication".to_string());
    }
    Ok(())
}

#[test]
fn rust_judged_panel_packet_strict_outer_current_rejects_duplicate_and_unknown_keys()
-> Result<(), String> {
    let valid = br#"{"schema_version":"0.1","kind":"rust_judged_panel_portable_current","generation_id":"generation","index_path":"metrics/rust-judged-behavior-panel/portable/generations/generation/packet-index.json","index_sha256":"sha256:index"}"#;
    let duplicate = br#"{"schema_version":"0.1","kind":"rust_judged_panel_portable_current","kind":"duplicate","generation_id":"generation","index_path":"metrics/rust-judged-behavior-panel/portable/generations/generation/packet-index.json","index_sha256":"sha256:index"}"#;
    let unknown = br#"{"schema_version":"0.1","kind":"rust_judged_panel_portable_current","generation_id":"generation","index_path":"metrics/rust-judged-behavior-panel/portable/generations/generation/packet-index.json","index_sha256":"sha256:index","unexpected":true}"#;
    let parsed: PortableCurrent = read_strict_json_bytes(valid, "valid current")?;
    if parsed.generation_id != "generation"
        || read_strict_json_bytes::<PortableCurrent>(duplicate, "duplicate current").is_ok()
        || read_strict_json_bytes::<PortableCurrent>(unknown, "unknown current").is_ok()
    {
        return Err("strict current readback accepted invalid JSON".to_string());
    }
    Ok(())
}

#[test]
fn rust_judged_panel_packet_retained_validator_reaches_committed_generation() -> Result<(), String>
{
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| "xtask has no workspace parent".to_string())?;
    let manifest = crate::rust_judged_panel::load_and_validate_at(
        root,
        Path::new("metrics/rust-judged-behavior-panel/manifest.json"),
    )?;
    super::validate_at(root, &manifest)
}

/// Resolve `<repository>/.git` to the real git directory it names.
///
/// A linked worktree's `.git` is a gitfile (`gitdir: <path>`), not a git
/// directory, so the disposable fixture cannot bind to it directly. Git
/// interprets a relative gitdir against the directory containing the gitfile,
/// so resolve it there before returning; a normal checkout (`.git` is a
/// directory) resolves to itself.
fn resolve_repository_gitdir(repository: &Path) -> Result<String, String> {
    let repository_git = repository.join(".git");
    if repository_git.is_dir() {
        return Ok(repository_git.display().to_string());
    }
    if !repository_git.is_file() {
        return Err(format!(
            "`{}` is neither a git directory nor a gitfile",
            repository_git.display()
        ));
    }
    let link = fs::read_to_string(&repository_git)
        .map_err(|error| format!("read worktree gitfile: {error}"))?;
    let target = link
        .trim()
        .strip_prefix("gitdir: ")
        .ok_or_else(|| "worktree `.git` gitfile has no `gitdir:` prefix".to_string())?;
    // `join` replaces the base when the gitdir is absolute, so both spellings
    // resolve against the gitfile's directory, exactly as Git does. Normalize
    // the joined path lexically (`wt/../meta` -> `meta`) so the fixture binds
    // to a clean absolute gitdir.
    let resolved = repository.join(target);
    let mut normalized = PathBuf::new();
    for component in resolved.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            component => normalized.push(component.as_os_str()),
        }
    }
    if !normalized.is_dir() {
        return Err(format!(
            "worktree gitfile points at `{}`, which is not a git directory",
            normalized.display()
        ));
    }
    Ok(normalized.display().to_string())
}

#[test]
fn rust_judged_panel_packet_public_publish_validates_disposable_host_run() -> Result<(), String> {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| "xtask manifest lacks repository parent".to_string())?;
    let root = TestRoot(scratch("public-publish")?);
    copy_tree(
        &repository.join("metrics/rust-judged-behavior-panel"),
        &root.0.join("metrics/rust-judged-behavior-panel"),
    )?;
    // A linked worktree's `.git` is a gitfile (`gitdir: <real git dir>`), not
    // a git directory. The fixture's gitfile must bind to the real git data,
    // so follow the gitfile chain once; a normal checkout resolves to itself.
    let gitdir = resolve_repository_gitdir(repository)?;
    fs::write(root.0.join(".git"), format!("gitdir: {gitdir}\n"))
        .map_err(|error| format!("bind disposable fixture to repository git data: {error}"))?;
    let state = crate::rust_judged_panel::subject::repository_state(repository)?;
    let subjects_path = root.0.join(SUBJECTS_PATH);
    let mut authority: serde_json::Value = read_strict_json(&subjects_path, "subject authority")?;
    for case in authority
        .get_mut("cases")
        .and_then(serde_json::Value::as_array_mut)
        .ok_or_else(|| "subject authority cases are not an array".to_string())?
    {
        case["expected_base"] = serde_json::Value::String(state.head.clone());
        case["expected_head"] = serde_json::Value::String(state.head.clone());
        case["expected_tree"] = serde_json::Value::String(state.tree.clone());
    }
    fs::write(
        subjects_path,
        serde_json::to_vec_pretty(&authority).map_err(|error| error.to_string())?,
    )
    .map_err(|error| format!("write disposable subject identities: {error}"))?;
    let manifest =
        crate::rust_judged_panel::load_and_validate_at(&root.0, Path::new(MANIFEST_PATH))?;
    let subjects = crate::rust_judged_panel::subject::load_for_packet(&root.0, &manifest)?;
    if subjects.len() != 3
        || subjects
            .iter()
            .map(|subject| subject.expected_direction.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            != ["should_gap", "should_stay_quiet", "should_limit"]
                .into_iter()
                .collect()
    {
        return Err("fixture does not cover all three governed directions".to_string());
    }
    let host_current = crate::rust_judged_panel::host_run::write_test_host_run(&root.0, &subjects)?;
    super::publish(&root.0, &manifest, &host_current)?;

    let current: PortableCurrent =
        read_strict_json(&root.0.join(CURRENT_PATH), "public publish current")?;
    let index: PortableIndex =
        read_strict_json(&root.0.join(&current.index_path), "public publish index")?;
    if index.generation_id.is_empty()
        || index.packets.len() != 3
        || index
            .packets
            .iter()
            .any(|entry| entry.packet_path.split('/').nth(4) != Some(index.generation_id.as_str()))
    {
        return Err(
            "public publication did not bind every packet to its final generation".to_string(),
        );
    }
    for entry in &index.packets {
        let packet: PortablePacket =
            read_strict_json(&root.0.join(&entry.packet_path), "public packet")?;
        let attestation: RetainedAttestation =
            read_strict_json(&root.0.join(&entry.attestation_path), "public attestation")?;
        if packet.case_id != entry.case_id
            || attestation.case_id != entry.case_id
            || packet.host_evidence.run_id != "run-fixture"
            || packet.judgment.disposition != "unjudged"
        {
            return Err(format!(
                "public publication lost generation or unjudged identity for `{}`",
                entry.case_id
            ));
        }
    }
    Ok(())
}

static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TestRoot(PathBuf);

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _cleanup = fs::remove_dir_all(&self.0);
    }
}

struct Fixture {
    _root: TestRoot,
    subject: PacketSubject,
    case: ValidatedHostCase,
    host: ValidatedHostRun,
}

fn file(role: &str, repository_path: &str) -> ReplaySubjectFile {
    ReplaySubjectFile {
        source_path: format!("metrics/subjects/{role}"),
        repository_path: repository_path.to_string(),
        sha256: format!("sha256:{role}"),
    }
}

fn copy_tree(source: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination).map_err(|error| error.to_string())?;
    for entry in fs::read_dir(source).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let target = destination.join(entry.file_name());
        if entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_dir()
        {
            copy_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

fn fixture(direction: &str) -> Result<Fixture, String> {
    let (
        case_id,
        owner,
        family,
        producer_family,
        expression,
        class,
        action,
        limit,
        missing,
        recommendation,
    ) = match direction {
        "should_gap" => (
            "rust-boundary-missing-equality-should-gap",
            "discounted_total",
            "predicate_boundary",
            "predicate",
            "amount >= threshold",
            "weakly_exposed",
            "repair_candidate",
            None,
            vec!["Missing discriminator value: amount == threshold"],
            "Add boundary tests for below, equal, and above the changed threshold with exact assertions.",
        ),
        "should_stay_quiet" => (
            "rust-boundary-exact-equality-should-stay-quiet",
            "discounted_total",
            "predicate_boundary",
            "predicate",
            "amount >= threshold",
            "exposed",
            "no_action",
            None,
            Vec::new(),
            "",
        ),
        "should_limit" => (
            "rust-macro-wrapped-reach-should-limit",
            "normalize_score",
            "return_value",
            "return_value",
            "value.max(1)",
            "no_static_path",
            "inspect_static_limitation",
            Some("rust_macro_wrapped_test_call_unresolved"),
            vec![
                "No relevant oracle was detected",
                "No static test path reaches the changed owner",
                "No strong discriminator was detected",
            ],
            "ripr found no static test path to this change — this is not a coverage assessment. A test may already exercise it through macros, helper-call chains, or integration tests that ripr's static model does not yet trace. If none does, add a co-located test that reaches and observes the changed behavior so a discriminator exists.",
        ),
        other => return Err(format!("unsupported direction `{other}`")),
    };
    let cargo_toml = file("cargo_toml", "Cargo.toml");
    let cargo_lock = file("cargo_lock", "Cargo.lock");
    let config = file("config", "ripr.toml");
    let source_before = file("source_before", "src/lib.rs");
    let source_after = file("source_after", "src/lib.rs");
    let diff = file("diff", "selected.diff");
    let tests = vec![file("test", "tests/case.rs")];
    let subject = PacketSubject {
        case_id: case_id.to_string(),
        subject_id: case_id.to_string(),
        repository: "synthetic".to_string(),
        expected_direction: direction.to_string(),
        anchor_file: "src/lib.rs".to_string(),
        anchor_line: 2,
        owner: owner.to_string(),
        behavior_family: family.to_string(),
        changed_behavior: expression.to_string(),
        expected_classification: class.to_string(),
        expected_actionability: action.to_string(),
        expected_static_limit_kind: limit.map(ToString::to_string),
        expected_missing: missing.iter().map(ToString::to_string).collect(),
        expected_recommendation: recommendation.to_string(),
        cargo_toml,
        cargo_lock,
        config,
        source_before,
        source_after,
        tests,
        diff,
        expected_base: "base".to_string(),
        expected_head: "head".to_string(),
        expected_tree: "tree".to_string(),
    };
    let inputs = expected_inputs(&subject);
    let reported_root = format!("target/ripr/rust-judged-panel/.staging-run-a/subjects/{case_id}");
    let mut finding = serde_json::json!({
        "id": format!("finding-{direction}"), "classification": class,
        "probe": {"family": producer_family, "file": format!("{reported_root}/src/lib.rs"), "line": 2, "expression": expression},
        "missing": missing, "recommended_next_step": recommendation
    });
    if let Some(kind) = limit {
        finding["static_limit_kind"] = Value::String(kind.to_string());
        finding["static_limitation"] = serde_json::json!({"kind": kind});
    }
    let report = serde_json::json!({
        "root": reported_root,
        "analysis_outcome": {"analysis_complete": true, "outcome": {"kind": "complete_with_findings", "limitations": []}},
        "findings": [finding]
    });
    let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "ripr-packet-fixture-{}-{sequence}-{direction}",
        std::process::id()
    ));
    let materialized_root = root.join(case_id);
    fs::create_dir_all(materialized_root.join("src")).map_err(|error| error.to_string())?;
    fs::write(
        materialized_root.join("src/lib.rs"),
        format!("\nfn {owner}() {{ {expression}; }}\n"),
    )
    .map_err(|error| error.to_string())?;
    let case = ValidatedHostCase {
        case_id: case_id.to_string(),
        subject_id: case_id.to_string(),
        expected_direction: direction.to_string(),
        repository_base: "base".to_string(),
        repository_head: "head".to_string(),
        repository_tree: "tree".to_string(),
        argv: [
            "check",
            "--root",
            "<materialized-subject>",
            "--base",
            "base",
            "--mode",
            "draft",
            "--json",
        ]
        .into_iter()
        .map(ToString::to_string)
        .collect(),
        mode: "draft".to_string(),
        format: "json".to_string(),
        config_path: subject.config.repository_path.clone(),
        config_sha256: subject.config.sha256.clone(),
        diff_path: subject.diff.source_path.clone(),
        diff_sha256: subject.diff.sha256.clone(),
        executed_diff_identity: "sha256:executed".to_string(),
        subject_inputs: inputs
            .iter()
            .map(|input| ValidatedInputDigest {
                role: input.role.clone(),
                source_path: input.source_path.clone(),
                repository_path: input.repository_path.clone(),
                sha256: input.sha256.clone(),
            })
            .collect(),
        disposition: "complete".to_string(),
        analyzer_input_identity: "sha256:executed".to_string(),
        receipt_ref: format!(
            "target/ripr/rust-judged-panel/runs/run-a/cases/{case_id}/receipt.json"
        ),
        receipt_sha256: "sha256:receipt".to_string(),
        stdout_ref: format!("target/ripr/rust-judged-panel/runs/run-a/cases/{case_id}/stdout.bin"),
        stdout_sha256: "sha256:stdout".to_string(),
        stderr_ref: format!("target/ripr/rust-judged-panel/runs/run-a/cases/{case_id}/stderr.bin"),
        stderr_sha256: "sha256:stderr".to_string(),
        stdout: serde_json::to_vec(&report).map_err(|error| error.to_string())?,
        materialized_root,
    };
    let host = ValidatedHostRun {
        current_ref: "target/ripr/rust-judged-panel/current.json".to_string(),
        current_sha256: "sha256:current".to_string(),
        index_ref: "target/ripr/rust-judged-panel/runs/run-a/run-index.json".to_string(),
        index_sha256: "sha256:index".to_string(),
        run_id: "run-a".to_string(),
        source_head: "producer-head".to_string(),
        source_tree: "producer-tree".to_string(),
        cargo_lock_sha256: "sha256:producer-lock".to_string(),
        cargo_toml_sha256: "sha256:producer-manifest".to_string(),
        profile: "dev".to_string(),
        features: vec!["default".to_string()],
        host_target: "test-target".to_string(),
        binary_sha256: "sha256:binary".to_string(),
        binary_version: "ripr 0.12.0".to_string(),
        cases: vec![case.clone()],
    };
    Ok(Fixture {
        _root: TestRoot(root),
        subject,
        case,
        host,
    })
}

fn projected(
    fixture: &Fixture,
) -> Result<(PortablePacket, RetainedAttestation, PortableIndexEntry), String> {
    let packet = project_one(&fixture.subject, &fixture.case, &fixture.host, "m", "s")?;
    let attestation =
        retained_attestation(&fixture.subject, &fixture.case, &fixture.host, "m", "s")?;
    let entry = PortableIndexEntry {
        case_id: packet.case_id.clone(),
        packet_path: "portable/packet.json".to_string(),
        packet_sha256: sha256_serialized(&packet)?,
        semantic_sha256: packet.semantic_sha256.clone(),
        attestation_path: "portable/attestation.json".to_string(),
        attestation_sha256: sha256_serialized(&attestation)?,
    };
    Ok((packet, attestation, entry))
}

fn rejects_reseal(fixture: &Fixture, mutate: fn(&mut PortablePacket)) -> Result<(), String> {
    rejects_structured_validate_at_reseal(
        "authority",
        &fixture.subject.case_id,
        |packet, _attestation| mutate(packet),
    )
}

fn rejects_direction_reseal(
    fixture: &Fixture,
    mutate: fn(&mut ObservedFinding),
) -> Result<(), String> {
    rejects_structured_validate_at_reseal(
        "direction",
        &fixture.subject.case_id,
        |packet, attestation| {
            mutate(&mut packet.semantic.observed);
            mutate(&mut attestation.semantic.observed);
        },
    )
}

fn rejects_validate_at_reseal(
    name: &str,
    case_id: &str,
    retain_member_paths: bool,
    prepare: impl FnOnce(
        &PortablePacket,
        &RetainedAttestation,
    ) -> Result<(Vec<u8>, String, Vec<u8>), String>,
) -> Result<(), String> {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| "xtask manifest lacks repository parent".to_string())?;
    let root = TestRoot(scratch(name)?);
    copy_tree(
        &repository.join("metrics/rust-judged-behavior-panel"),
        &root.0.join("metrics/rust-judged-behavior-panel"),
    )?;
    let manifest = super::super::load_and_validate_at(&root.0, Path::new(MANIFEST_PATH))?;
    validate_at(&root.0, &manifest)?;
    let current_path = root.0.join(CURRENT_PATH);
    let mut current: PortableCurrent = read_strict_json(&current_path, "test current")?;
    let original_generation = root
        .0
        .join(&current.index_path)
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "test current lacks generation".to_string())?;
    let mut index: PortableIndex =
        read_strict_json(&original_generation.join("packet-index.json"), "test index")?;
    let original_entry = index
        .packets
        .iter()
        .find(|entry| entry.case_id == case_id)
        .ok_or_else(|| format!("test index lacks `{case_id}`"))?
        .clone();
    let packet: PortablePacket =
        read_strict_json(&root.0.join(&original_entry.packet_path), "test packet")?;
    let attestation: RetainedAttestation = read_strict_json(
        &root.0.join(&original_entry.attestation_path),
        "test attestation",
    )?;
    let (packet_bytes, semantic_sha256, attestation_bytes) = prepare(&packet, &attestation)?;
    let entry = index
        .packets
        .iter_mut()
        .find(|entry| entry.case_id == case_id)
        .ok_or_else(|| format!("test index lacks `{case_id}`"))?;
    entry.semantic_sha256 = semantic_sha256;
    entry.packet_sha256 = sha256_bytes(&packet_bytes);
    entry.attestation_sha256 = sha256_bytes(&attestation_bytes);
    let next_id = generation_id(
        &index.manifest_sha256,
        &index.subjects_sha256,
        &index.packets,
    )?;
    index.generation_id = next_id.clone();
    for entry in &mut index.packets {
        if !retain_member_paths || entry.case_id != case_id {
            entry.packet_path = format!(
                "{PORTABLE_ROOT}/generations/{next_id}/packets/{}.json",
                entry.case_id
            );
            entry.attestation_path = format!(
                "{PORTABLE_ROOT}/generations/{next_id}/attestations/{}.json",
                entry.case_id
            );
        }
    }
    let next_generation = root
        .0
        .join(PORTABLE_ROOT)
        .join("generations")
        .join(&next_id);
    copy_tree(&original_generation, &next_generation)?;
    let changed_packet_path = next_generation
        .join("packets")
        .join(format!("{case_id}.json"));
    let changed_attestation_path = next_generation
        .join("attestations")
        .join(format!("{case_id}.json"));
    fs::write(changed_packet_path, packet_bytes).map_err(|error| error.to_string())?;
    fs::write(changed_attestation_path, attestation_bytes).map_err(|error| error.to_string())?;
    let index_bytes = pretty_json(&index)?;
    fs::write(next_generation.join("packet-index.json"), &index_bytes)
        .map_err(|error| error.to_string())?;
    current.generation_id = next_id.clone();
    current.index_path = format!("{PORTABLE_ROOT}/generations/{next_id}/packet-index.json");
    current.index_sha256 = sha256_bytes(&index_bytes);
    fs::write(&current_path, pretty_json(&current)?).map_err(|error| error.to_string())?;
    if validate_at(&root.0, &manifest).is_err() {
        Ok(())
    } else {
        Err(format!(
            "full packet/index/generation/current `{name}` reseal passed production validation"
        ))
    }
}

fn rejects_structured_validate_at_reseal(
    name: &str,
    case_id: &str,
    mutate: impl FnOnce(&mut PortablePacket, &mut RetainedAttestation),
) -> Result<(), String> {
    rejects_validate_at_reseal(name, case_id, false, |packet, attestation| {
        let mut packet = packet.clone();
        let mut attestation = attestation.clone();
        mutate(&mut packet, &mut attestation);
        packet.semantic_sha256 = sha256_serialized(&packet.semantic)?;
        Ok((
            pretty_json(&packet)?,
            packet.semantic_sha256.clone(),
            pretty_json(&attestation)?,
        ))
    })
}

#[test]
fn rust_judged_panel_packet_rejects_all_authority_family_reseals() -> Result<(), String> {
    let fixture = fixture("should_gap")?;
    for mutate in [
        (|p: &mut PortablePacket| p.semantic.producer_source_head = "stale".to_string())
            as fn(&mut PortablePacket),
        |p| p.semantic.producer_source_tree = "stale".to_string(),
        |p| p.semantic.producer_cargo_toml_sha256 = "stale".to_string(),
        |p| p.semantic.producer_cargo_lock_sha256 = "stale".to_string(),
        |p| p.semantic.producer_version = "stale".to_string(),
        |p| p.semantic.profile = "release".to_string(),
        |p| p.semantic.features.clear(),
        |p| p.semantic.argv = vec!["stale".to_string()],
        |p| p.semantic.mode = "stale".to_string(),
        |p| p.semantic.format = "stale".to_string(),
        |p| p.semantic.config_path = "stale".to_string(),
        |p| p.semantic.config_sha256 = "stale".to_string(),
        |p| p.semantic.diff_path = "stale".to_string(),
        |p| p.semantic.diff_sha256 = "stale".to_string(),
        |p| p.semantic.executed_diff_identity = "stale".to_string(),
        |p| p.semantic.subject_inputs.clear(),
        |p| p.semantic.observed.finding_id = "stale".to_string(),
        |p| p.semantic.observed.probe_family = "stale".to_string(),
        |p| p.semantic.observed.probe_file = "stale".to_string(),
        |p| p.semantic.observed.probe_line = 99,
        |p| p.semantic.observed.probe_expression = "stale".to_string(),
        |p| p.semantic.observed.static_limit_kind = Some("stale".to_string()),
        |p| p.host_evidence.host_target = "stale".to_string(),
        |p| p.host_evidence.binary_sha256 = "stale".to_string(),
        |p| p.host_evidence.run_id = "stale".to_string(),
        |p| p.host_evidence.receipt_sha256 = "stale".to_string(),
        |p| p.host_evidence.receipt_ref = "stale/receipt.json".to_string(),
        |p| p.host_evidence.current_ref = "stale/current.json".to_string(),
        |p| p.host_evidence.current_sha256 = "stale".to_string(),
        |p| p.host_evidence.index_ref = "stale/index.json".to_string(),
        |p| p.host_evidence.index_sha256 = "stale".to_string(),
        |p| p.host_evidence.stdout_ref = "stale/stdout.bin".to_string(),
        |p| p.host_evidence.stdout_sha256 = "stale".to_string(),
        |p| p.host_evidence.stderr_ref = "stale/stderr.bin".to_string(),
        |p| p.host_evidence.stderr_sha256 = "stale".to_string(),
        |p| p.host_evidence.analyzer_input_identity = "stale".to_string(),
    ] {
        rejects_reseal(&fixture, mutate)?;
    }
    Ok(())
}

#[test]
fn rust_judged_panel_packet_validate_at_rejects_full_digest_chain_reseal() -> Result<(), String> {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| "xtask manifest lacks repository parent".to_string())?;
    let root = std::env::temp_dir().join(format!("ripr-packet-reseal-{}", std::process::id()));
    if root.exists() {
        fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
    }
    copy_tree(
        &repository.join("metrics/rust-judged-behavior-panel"),
        &root.join("metrics/rust-judged-behavior-panel"),
    )?;
    let manifest = super::super::load_and_validate_at(&root, Path::new(MANIFEST_PATH))?;
    validate_at(&root, &manifest)?;
    let current_path = root.join(CURRENT_PATH);
    let mut current: PortableCurrent = read_strict_json(&current_path, "test current")?;
    let original_generation = root
        .join(&current.index_path)
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "test current lacks generation".to_string())?;
    let mut index: PortableIndex =
        read_strict_json(&original_generation.join("packet-index.json"), "test index")?;
    let entry = index
        .packets
        .first_mut()
        .ok_or_else(|| "test index is empty".to_string())?;
    let mut packet: PortablePacket =
        read_strict_json(&root.join(&entry.packet_path), "test packet")?;
    packet.semantic.producer_source_head = "coordinated-stale-source".to_string();
    packet.semantic_sha256 = sha256_serialized(&packet.semantic)?;
    let packet_bytes = pretty_json(&packet)?;
    entry.semantic_sha256 = packet.semantic_sha256.clone();
    entry.packet_sha256 = sha256_bytes(&packet_bytes);
    let next_id = generation_id(
        &index.manifest_sha256,
        &index.subjects_sha256,
        &index.packets,
    )?;
    index.generation_id = next_id.clone();
    for entry in &mut index.packets {
        entry.packet_path = format!(
            "{PORTABLE_ROOT}/generations/{next_id}/packets/{}.json",
            entry.case_id
        );
        entry.attestation_path = format!(
            "{PORTABLE_ROOT}/generations/{next_id}/attestations/{}.json",
            entry.case_id
        );
    }
    let next_generation = root.join(PORTABLE_ROOT).join("generations").join(&next_id);
    copy_tree(&original_generation, &next_generation)?;
    let changed_entry = index
        .packets
        .first()
        .ok_or_else(|| "test index is empty".to_string())?;
    fs::write(root.join(&changed_entry.packet_path), packet_bytes)
        .map_err(|error| error.to_string())?;
    let index_bytes = pretty_json(&index)?;
    fs::write(next_generation.join("packet-index.json"), &index_bytes)
        .map_err(|error| error.to_string())?;
    current.generation_id = next_id.clone();
    current.index_path = format!("{PORTABLE_ROOT}/generations/{next_id}/packet-index.json");
    current.index_sha256 = sha256_bytes(&index_bytes);
    fs::write(&current_path, pretty_json(&current)?).map_err(|error| error.to_string())?;
    let rejected = validate_at(&root, &manifest).is_err();
    fs::remove_dir_all(root).map_err(|error| error.to_string())?;
    if rejected {
        Ok(())
    } else {
        Err("full packet/index/generation/current reseal passed retained attestation".to_string())
    }
}

#[test]
fn rust_judged_panel_packet_rejects_resealed_stale_generation_member_paths() -> Result<(), String> {
    let fixture = fixture("should_gap")?;
    rejects_validate_at_reseal(
        "stale-member-path",
        &fixture.subject.case_id,
        true,
        |packet, attestation| {
            let mut packet = packet.clone();
            packet.semantic.producer_source_head = "next-generation".to_string();
            packet.semantic_sha256 = sha256_serialized(&packet.semantic)?;
            Ok((
                pretty_json(&packet)?,
                packet.semantic_sha256.clone(),
                pretty_json(attestation)?,
            ))
        },
    )
}

#[test]
fn rust_judged_panel_packet_binds_all_three_exact_direction_witnesses() -> Result<(), String> {
    for direction in ["should_gap", "should_stay_quiet", "should_limit"] {
        let fixture = fixture(direction)?;
        let (packet, attestation, entry) = projected(&fixture)?;
        validate_retained_packet(&packet, &entry, &attestation, &fixture.subject, "m", "s")?;
        rejects_direction_reseal(&fixture, |observed| {
            observed.recommendation.push_str("stale")
        })?;
        rejects_direction_reseal(&fixture, |observed| {
            observed.missing.push("stale".to_string())
        })?;
        rejects_direction_reseal(&fixture, |observed| {
            observed.static_limit_kind = Some("stale".to_string())
        })?;
    }
    Ok(())
}

#[test]
fn rust_judged_panel_packet_strict_nested_readback_rejects_duplicate_and_unknown_keys()
-> Result<(), String> {
    fn rejects<T: for<'de> Deserialize<'de>>(body: &[u8]) -> Result<(), String> {
        let text = std::str::from_utf8(body).map_err(|error| error.to_string())?;
        for hostile in [
            text.replacen('{', "{\n  \"unexpected\": true,", 1),
            text.replacen("\"kind\":", "\"kind\": \"duplicate\",\n  \"kind\":", 1),
        ] {
            if read_strict_json_bytes::<T>(hostile.as_bytes(), "hostile DTO").is_ok() {
                return Err("hostile DTO passed strict readback".to_string());
            }
        }
        Ok(())
    }
    let fixture = fixture("should_gap")?;
    let (packet, attestation, entry) = projected(&fixture)?;
    let current = PortableCurrent {
        schema_version: "0.1".to_string(),
        kind: "rust_judged_panel_portable_current".to_string(),
        generation_id: "generation".to_string(),
        index_path: "portable/index.json".to_string(),
        index_sha256: "sha256:index".to_string(),
    };
    let index = PortableIndex {
        schema_version: "0.1".to_string(),
        kind: "rust_judged_panel_portable_index".to_string(),
        publication_state: "complete".to_string(),
        generation_id: "generation".to_string(),
        manifest_sha256: "m".to_string(),
        subjects_sha256: "s".to_string(),
        packets: vec![entry],
        non_claims: expected_non_claims(),
    };
    rejects::<PortablePacket>(&pretty_json(&packet)?)?;
    rejects::<RetainedAttestation>(&pretty_json(&attestation)?)?;
    fn rejects_nested<T: for<'de> Deserialize<'de>>(body: &[u8]) -> Result<(), String> {
        let text = std::str::from_utf8(body).map_err(|error| error.to_string())?;
        for hostile in [
            text.replacen(
                "\"host_evidence\": {",
                "\"host_evidence\": {\n    \"unexpected\": true,",
                1,
            ),
            text.replacen(
                "\"availability\":",
                "\"availability\": \"duplicate\",\n    \"availability\":",
                1,
            ),
        ] {
            if read_strict_json_bytes::<T>(hostile.as_bytes(), "hostile nested DTO").is_ok() {
                return Err("hostile nested host DTO passed strict readback".to_string());
            }
        }
        Ok(())
    }
    rejects_nested::<PortablePacket>(&pretty_json(&packet)?)?;
    rejects_nested::<RetainedAttestation>(&pretty_json(&attestation)?)?;
    rejects::<PortableIndex>(&pretty_json(&index)?)?;
    rejects::<PortableCurrent>(&pretty_json(&current)?)?;
    let case_id = fixture.subject.case_id.clone();
    for (name, needle, replacement) in [
        (
            "nested-unknown",
            "\"host_evidence\": {",
            "\"host_evidence\": {\n    \"unexpected\": true,",
        ),
        (
            "nested-duplicate",
            "\"availability\":",
            "\"availability\": \"duplicate\",\n    \"availability\":",
        ),
    ] {
        rejects_validate_at_reseal(name, &case_id, false, |packet, attestation| {
            let packet_bytes = pretty_json(packet)?;
            let hostile_packet = std::str::from_utf8(&packet_bytes)
                .map_err(|error| error.to_string())?
                .replacen(needle, replacement, 1)
                .into_bytes();
            Ok((
                hostile_packet,
                packet.semantic_sha256.clone(),
                pretty_json(attestation)?,
            ))
        })?;
        rejects_validate_at_reseal(name, &case_id, false, |packet, attestation| {
            let attestation_bytes = pretty_json(attestation)?;
            let hostile_attestation = std::str::from_utf8(&attestation_bytes)
                .map_err(|error| error.to_string())?
                .replacen(needle, replacement, 1)
                .into_bytes();
            Ok((
                pretty_json(packet)?,
                packet.semantic_sha256.clone(),
                hostile_attestation,
            ))
        })?;
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn rust_judged_panel_packet_rejects_in_repo_sibling_symlink() -> Result<(), String> {
    let root = std::env::temp_dir().join(format!("ripr-packet-link-{}", std::process::id()));
    let portable = root.join(PORTABLE_ROOT);
    let sibling = root.join("metrics/rust-judged-behavior-panel/sibling");
    let outside =
        std::env::temp_dir().join(format!("ripr-packet-link-outside-{}", std::process::id()));
    fs::create_dir_all(&portable).map_err(|error| error.to_string())?;
    fs::create_dir_all(&sibling).map_err(|error| error.to_string())?;
    fs::write(sibling.join("packet.json"), b"outside").map_err(|error| error.to_string())?;
    fs::create_dir_all(&outside).map_err(|error| error.to_string())?;
    fs::write(outside.join("packet.json"), b"outside").map_err(|error| error.to_string())?;
    let actual = portable.join("actual");
    fs::create_dir_all(&actual).map_err(|error| error.to_string())?;
    fs::write(actual.join("packet.json"), b"inside").map_err(|error| error.to_string())?;
    std::os::unix::fs::symlink(&sibling, portable.join("linked"))
        .map_err(|error| error.to_string())?;
    std::os::unix::fs::symlink(&outside, portable.join("outside"))
        .map_err(|error| error.to_string())?;
    std::os::unix::fs::symlink(&actual, portable.join("alias"))
        .map_err(|error| error.to_string())?;
    let rejected =
        require_canonical_portable(&root, &portable.join("linked/packet.json"), "packet").is_err();
    let outside_rejected =
        require_canonical_portable(&root, &portable.join("outside/packet.json"), "packet").is_err();
    let alias_rejected =
        require_canonical_portable(&root, &portable.join("alias/packet.json"), "packet").is_err();
    fs::remove_dir_all(&portable).map_err(|error| error.to_string())?;
    std::os::unix::fs::symlink(&sibling, &portable).map_err(|error| error.to_string())?;
    let fixtures = ["should_gap", "should_stay_quiet", "should_limit"]
        .into_iter()
        .map(fixture)
        .collect::<Result<Vec<_>, _>>()?;
    let packets = fixtures
        .iter()
        .map(|fixture| projected(fixture).map(|value| value.0))
        .collect::<Result<Vec<_>, _>>()?;
    let attestations = fixtures
        .iter()
        .map(|fixture| {
            retained_attestation(&fixture.subject, &fixture.case, &fixture.host, "m", "s")
        })
        .collect::<Result<Vec<_>, _>>()?;
    let write_rejected = publish_all(&root, "m", "s", &packets, &attestations, None).is_err();
    let sibling_unchanged =
        !sibling.join("current.json").exists() && !sibling.join("generations").exists();
    fs::remove_dir_all(root).map_err(|error| error.to_string())?;
    fs::remove_dir_all(outside).map_err(|error| error.to_string())?;
    if rejected && outside_rejected && alias_rejected && write_rejected && sibling_unchanged {
        Ok(())
    } else {
        Err("portable sibling symlink escaped canonical root".to_string())
    }
}

#[test]
fn rust_judged_panel_packet_rejects_noncanonical_portable_spellings() -> Result<(), String> {
    for path in [
        "metrics\\rust-judged-behavior-panel\\portable\\current.json",
        "metrics/rust-judged-behavior-panel/portable//current.json",
        "metrics/rust-judged-behavior-panel/portable/../portable/current.json",
    ] {
        if safe_portable_path(path).is_ok() {
            return Err(format!("noncanonical portable spelling passed: `{path}`"));
        }
    }
    Ok(())
}

#[test]
fn rust_judged_panel_packet_reuse_validates_every_member_before_current() -> Result<(), String> {
    let fixtures = ["should_gap", "should_stay_quiet", "should_limit"]
        .into_iter()
        .map(fixture)
        .collect::<Result<Vec<_>, _>>()?;
    let projected = fixtures
        .iter()
        .map(projected)
        .collect::<Result<Vec<_>, _>>()?;
    let packets = projected
        .iter()
        .map(|(packet, _, _)| packet.clone())
        .collect::<Vec<_>>();
    let attestations = projected
        .iter()
        .map(|(_, attestation, _)| attestation.clone())
        .collect::<Vec<_>>();
    let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let root = TestRoot(std::env::temp_dir().join(format!(
        "ripr-packet-reuse-{}-{sequence}",
        std::process::id()
    )));
    fs::create_dir_all(&root.0).map_err(|error| error.to_string())?;
    publish_all(
        &root.0,
        "sha256:m",
        "sha256:s",
        &packets,
        &attestations,
        None,
    )?;
    let current_path = root.0.join(CURRENT_PATH);
    let prior = fs::read(&current_path).map_err(|error| error.to_string())?;
    let current: PortableCurrent = read_strict_json(&current_path, "test current")?;
    let index: PortableIndex = read_strict_json(&root.0.join(&current.index_path), "test index")?;
    let entry = index
        .packets
        .first()
        .ok_or_else(|| "test index is empty".to_string())?;
    fs::remove_file(root.0.join(&entry.packet_path)).map_err(|error| error.to_string())?;
    if publish_all(
        &root.0,
        "sha256:m",
        "sha256:s",
        &packets,
        &attestations,
        None,
    )
    .is_ok()
        || fs::read(&current_path).map_err(|error| error.to_string())? != prior
    {
        return Err("tampered reuse advanced or changed the prior current".to_string());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// #3407: disposable on-disk validated host-run fixture plus the public
// `packet::publish` -> `packet::validate_at` integration route.
// ---------------------------------------------------------------------------

struct HostDiskFixture {
    _root: TestRoot,
    manifest: RustJudgedPanelManifest,
    host_current: String,
    run_id: String,
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    format!("sha256:{:x}", sha2::Sha256::digest(bytes))
}

fn repository_root() -> Result<PathBuf, String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| "xtask manifest lacks repository parent".to_string())?;
    if root.as_os_str().is_empty() {
        return Err("repository root resolved to an empty path".to_string());
    }
    Ok(root.to_path_buf())
}

type TreeSnapshotEntries = Vec<(String, bool, Vec<u8>)>;
type TreeSnapshot = Option<TreeSnapshotEntries>;

fn snapshot_tree(root: &Path) -> Result<TreeSnapshot, String> {
    if !root.exists() {
        return Ok(None);
    }

    fn visit(root: &Path, path: &Path, entries: &mut TreeSnapshotEntries) -> Result<(), String> {
        let relative = path
            .strip_prefix(root)
            .map_err(|error| format!("snapshot path outside root: {error}"))?
            .to_string_lossy()
            .replace('\\', "/");
        let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
        if metadata.is_dir() {
            entries.push((relative, true, Vec::new()));
            for entry in fs::read_dir(path).map_err(|error| error.to_string())? {
                visit(
                    root,
                    &entry.map_err(|error| error.to_string())?.path(),
                    entries,
                )?;
            }
        } else {
            entries.push((
                relative,
                false,
                fs::read(path).map_err(|error| error.to_string())?,
            ));
        }
        Ok(())
    }

    let mut entries = Vec::new();
    visit(root, root, &mut entries)?;
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(Some(entries))
}

fn assert_tree_unchanged(root: &Path, before: &TreeSnapshot, label: &str) -> Result<(), String> {
    let after = snapshot_tree(root)?;
    if &after != before {
        return Err(format!("{label} changed the portable tree"));
    }
    Ok(())
}

fn write_json_bytes(path: &Path, value: &serde_json::Value) -> Result<Vec<u8>, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    fs::write(path, &bytes).map_err(|error| error.to_string())?;
    Ok(bytes)
}

fn copy_panel_authority(repository: &Path, root: &Path) -> Result<(), String> {
    let source = repository.join("metrics/rust-judged-behavior-panel");
    let destination = root.join("metrics/rust-judged-behavior-panel");
    fs::create_dir_all(&destination).map_err(|error| error.to_string())?;
    for name in ["manifest.json", "subjects.json"] {
        fs::copy(source.join(name), destination.join(name)).map_err(|error| error.to_string())?;
    }
    copy_tree(&source.join("diffs"), &destination.join("diffs"))?;
    copy_tree(&source.join("subjects"), &destination.join("subjects"))
}

/// Materialize a complete validated host-run generation on disk for the three
/// governed directions. Subject repositories are produced by the real
/// deterministic materializer (identity checked against the committed
/// expected base/head/tree); receipts, raw evidence, and the build identity
/// carry real digests so `load_validated_current` accepts the artifact set.
fn on_disk_host_fixture(name: &str) -> Result<HostDiskFixture, String> {
    use crate::rust_judged_panel::subject::{self, ReplaySubjectFile};

    let repository = repository_root()?;
    let root = TestRoot(scratch(name)?);
    copy_panel_authority(&repository, &root.0)?;
    let manifest = crate::rust_judged_panel::load_and_validate_at(
        &root.0,
        Path::new("metrics/rust-judged-behavior-panel/manifest.json"),
    )?;

    const OUTPUT_RELATIVE: &str = "target/ripr/judged-panel-fixture";
    let output_root = root.0.join(OUTPUT_RELATIVE);
    let sequence = TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let run_id = format!("run-fixture-{sequence}");
    let staging = output_root.join(format!(".staging-{run_id}"));
    let replays = subject::materialize_for_replay(&root.0, &staging.join("subjects"), &manifest)?;
    let subjects_by_id = subject::load_for_packet(&root.0, &manifest)?
        .into_iter()
        .map(|subject| (subject.case_id.clone(), subject))
        .collect::<std::collections::BTreeMap<_, _>>();

    // Build identity: exactly the owned locked/offline dev build shape.
    let binary_bytes = b"ripr judged-panel fixture retained binary\n";
    let binary_sha256 = sha256_hex(binary_bytes);
    fs::create_dir_all(staging.join("build")).map_err(|error| error.to_string())?;
    fs::write(staging.join("build/retained-ripr.bin"), binary_bytes)
        .map_err(|error| error.to_string())?;
    let build_stdout = b"fixture cargo build stdout\n";
    let build_stderr = b"";
    fs::write(staging.join("build/stdout.bin"), build_stdout).map_err(|error| error.to_string())?;
    fs::write(staging.join("build/stderr.bin"), build_stderr).map_err(|error| error.to_string())?;
    let build = serde_json::json!({
        "command": ["cargo", "build", "-p", "ripr", "--locked", "--offline",
                    "--target-dir", "fixture-target"],
        "package": "ripr",
        "profile": "dev",
        "features": ["default"],
        "locked": true,
        "offline": true,
        "cargo_version": "cargo fixture-version",
        "rustc_verbose_version": "rustc fixture-version",
        "host_target": "fixture-host-target",
        "cargo_home": null,
        "executed_binary_path": "fixture-target/debug/retained-ripr.bin",
        "retained_binary_path": "build/retained-ripr.bin",
        "binary_sha256": binary_sha256,
        "binary_bytes": binary_bytes.len() as u64,
        "binary_version": "ripr 0.0.0-judged-panel-fixture",
        "build_stdout_sha256": sha256_hex(build_stdout),
        "build_stderr_sha256": sha256_hex(build_stderr),
    });

    let source_head = "fixture-source-head";
    let source_tree = "fixture-source-tree";
    let mut index_cases = Vec::new();
    for replay in &replays {
        let subject = subjects_by_id
            .get(&replay.case_id)
            .ok_or_else(|| format!("authority lacks `{}`", replay.case_id))?;
        let executed = subject::executed_diff_identity(&replay.root, &replay.base)?;
        let staging_subject_root = format!(
            "{OUTPUT_RELATIVE}/.staging-{run_id}/subjects/{}",
            replay.case_id
        );
        let probe_file = format!("{staging_subject_root}/{}", subject.anchor_file);
        let family = match subject.behavior_family.as_str() {
            "predicate_boundary" => "predicate",
            "return_value" => "return_value",
            other => return Err(format!("unsupported behavior family `{other}`")),
        };
        let input_value = |role: &str, file: &ReplaySubjectFile| {
            serde_json::json!({
                "role": role,
                "source_path": file.source_path,
                "repository_path": file.repository_path,
                "sha256": file.sha256,
            })
        };
        let mut subject_inputs = vec![
            input_value("cargo_toml", &replay.cargo_toml),
            input_value("cargo_lock", &replay.cargo_lock),
            input_value("config", &replay.config),
            input_value("source_before", &replay.source_before),
            input_value("source_after", &replay.source_after),
            input_value("diff", &replay.diff),
        ];
        for test in &replay.tests {
            subject_inputs.push(input_value("test", test));
        }

        let mut finding = serde_json::json!({
            "id": format!("finding-{}", replay.case_id),
            "classification": subject.expected_classification,
            "probe": {
                "family": family,
                "file": probe_file,
                "line": subject.anchor_line,
                "expression": subject.changed_behavior,
            },
            "missing": subject.expected_missing,
            "recommended_next_step": subject.expected_recommendation,
        });
        if let Some(kind) = &subject.expected_static_limit_kind {
            finding["static_limit_kind"] = serde_json::Value::String(kind.clone());
        }
        let report = serde_json::json!({
            "root": staging_subject_root,
            "analysis_outcome": {
                "analysis_complete": true,
                "outcome": {
                    "kind": "complete_with_findings",
                    "limitations": [],
                    "identity": {"input_identity": executed},
                },
            },
            "findings": [finding],
        });
        let stdout_bytes = serde_json::to_vec_pretty(&report).map_err(|error| error.to_string())?;
        let stderr_bytes: &[u8] = b"";
        let stdout_relative = format!("cases/{}/stdout.bin", replay.case_id);
        let stderr_relative = format!("cases/{}/stderr.bin", replay.case_id);
        fs::create_dir_all(staging.join("cases").join(&replay.case_id))
            .map_err(|error| error.to_string())?;
        fs::write(staging.join(&stdout_relative), &stdout_bytes)
            .map_err(|error| error.to_string())?;
        fs::write(staging.join(&stderr_relative), stderr_bytes)
            .map_err(|error| error.to_string())?;

        let receipt_relative = format!("cases/{}/receipt.json", replay.case_id);
        let receipt = serde_json::json!({
            "schema_version": "0.1",
            "kind": "rust_judged_panel_host_run_receipt",
            "case_id": replay.case_id,
            "subject_id": replay.subject_id,
            "expected_direction": replay.expected_direction,
            "source_head": source_head,
            "source_tree": source_tree,
            "binary_sha256": binary_sha256,
            "repository_base": replay.base,
            "repository_head": replay.head,
            "repository_tree": replay.tree,
            "plan": {
                "argv": ["check", "--root", "<materialized-subject>", "--base",
                         replay.base, "--mode", "draft", "--json"],
                "root": staging_subject_root,
                "base": replay.base,
                "head": replay.head,
                "tree": replay.tree,
                "mode": "draft",
                "format": "json",
                "config_path": replay.config.repository_path,
                "config_sha256": replay.config.sha256,
                "diff_path": replay.diff.source_path,
                "diff_sha256": replay.diff.sha256,
                "executed_diff_identity": executed,
                "subject_inputs": subject_inputs,
            },
            "disposition": "complete",
            "exit_code": 0,
            "timed_out": false,
            "duration_ms": 1,
            "analyzer_input_identity": executed,
            "raw": {
                "stdout_path": stdout_relative,
                "stdout_sha256": sha256_hex(&stdout_bytes),
                "stdout_bytes": stdout_bytes.len() as u64,
                "stderr_path": stderr_relative,
                "stderr_sha256": sha256_hex(stderr_bytes),
                "stderr_bytes": stderr_bytes.len() as u64,
            },
            "error": null,
        });
        let receipt_bytes = write_json_bytes(&staging.join(&receipt_relative), &receipt)?;
        index_cases.push(serde_json::json!({
            "case_id": replay.case_id,
            "expected_direction": replay.expected_direction,
            "receipt_path": receipt_relative,
            "receipt_sha256": sha256_hex(&receipt_bytes),
            "stdout_sha256": sha256_hex(&stdout_bytes),
            "stderr_sha256": sha256_hex(stderr_bytes),
        }));
    }
    index_cases.sort_by(|left, right| {
        left["case_id"]
            .as_str()
            .unwrap_or_default()
            .cmp(right["case_id"].as_str().unwrap_or_default())
    });

    let index = serde_json::json!({
        "schema_version": "0.1",
        "kind": "rust_judged_panel_host_run_index",
        "publication_state": "complete",
        "run_id": run_id,
        "source": {
            "head": source_head,
            "tree": source_tree,
            "cargo_lock_sha256": "sha256:fixture-producer-lock",
            "cargo_toml_sha256": "sha256:fixture-producer-manifest",
            "dirty": false,
        },
        "build": build,
        "cases": index_cases,
        "non_claims": [
            "host-bound raw execution evidence only",
            "no portable semantic packet or judgment",
            "no rate, gate, badge, or support claim",
        ],
    });
    let index_bytes = write_json_bytes(&staging.join("run-index.json"), &index)?;

    // Publish the staged attempt as the immutable final generation, exactly
    // like a real host-run transaction: the retained reports keep their
    // staging-path spellings while the artifacts move to `runs/{run_id}`.
    let runs_root = output_root.join("runs");
    fs::create_dir_all(&runs_root).map_err(|error| error.to_string())?;
    let final_run = runs_root.join(&run_id);
    fs::rename(&staging, &final_run).map_err(|error| error.to_string())?;
    let current = serde_json::json!({
        "schema_version": "0.1",
        "kind": "rust_judged_panel_current_host_run",
        "run_id": run_id,
        "index_path": format!("runs/{run_id}/run-index.json"),
        "index_sha256": sha256_hex(&index_bytes),
    });
    write_json_bytes(&output_root.join("current.json"), &current)?;

    Ok(HostDiskFixture {
        _root: root,
        manifest,
        host_current: format!("{OUTPUT_RELATIVE}/current.json"),
        run_id,
    })
}

#[test]
fn judgment_sidecar_public_publish_end_to_end() -> Result<(), String> {
    let fixture = on_disk_host_fixture("public-publish")?;
    super::publish(&fixture._root.0, &fixture.manifest, &fixture.host_current)?;
    super::validate_at(&fixture._root.0, &fixture.manifest)?;

    // Packet, packet index, and current pointer share one generation
    // identity with the exact selected case set and the fixture's host run.
    let current: PortableCurrent =
        read_strict_json(&fixture._root.0.join(CURRENT_PATH), "portable current")?;
    let index: PortableIndex =
        read_strict_json(&fixture._root.0.join(&current.index_path), "portable index")?;
    if index.generation_id != current.generation_id {
        return Err(format!(
            "portable index generation `{}` does not match current `{}`",
            index.generation_id, current.generation_id
        ));
    }
    if index.packets.len() != 3 {
        return Err(format!(
            "portable index must carry the three canonical cases, got {}",
            index.packets.len()
        ));
    }
    for entry in &index.packets {
        for path in [&entry.packet_path, &entry.attestation_path] {
            if !path.contains(&current.generation_id) {
                return Err(format!(
                    "index member `{path}` is outside the current generation"
                ));
            }
        }
        let packet: PortablePacket =
            read_strict_json(&fixture._root.0.join(&entry.packet_path), "packet")?;
        if packet.case_id != entry.case_id {
            return Err(format!(
                "packet `{}` does not match its index entry",
                packet.case_id
            ));
        }
        if packet.judgment.disposition != "unjudged" {
            return Err(format!("packet `{}` must stay unjudged", packet.case_id));
        }
        if packet.host_evidence.run_id != fixture.run_id {
            return Err(format!(
                "packet `{}` binds run `{}` instead of `{}`",
                packet.case_id, packet.host_evidence.run_id, fixture.run_id
            ));
        }
        if packet.semantic.manifest_sha256 != index.manifest_sha256
            || packet.semantic.subjects_sha256 != index.subjects_sha256
        {
            return Err(format!(
                "packet `{}` does not bind the published authority digests",
                packet.case_id
            ));
        }
    }

    // Republishing the same evidence is content-addressed: the accepted
    // generation equals the final published generation.
    super::publish(&fixture._root.0, &fixture.manifest, &fixture.host_current)?;
    let republished: PortableCurrent =
        read_strict_json(&fixture._root.0.join(CURRENT_PATH), "republished current")?;
    if republished.generation_id != current.generation_id {
        return Err(format!(
            "content-addressed generation drifted: `{}` -> `{}`",
            current.generation_id, republished.generation_id
        ));
    }
    Ok(())
}

#[test]
fn judgment_sidecar_public_publish_fails_closed_without_a_valid_host_run() -> Result<(), String> {
    // Absent sidecar: publish must fail closed without writing any portable
    // artifact when the referenced host current does not exist.
    let absent = on_disk_host_fixture("absent-sidecar")?;
    let absent_portable = absent
        ._root
        .0
        .join("metrics/rust-judged-behavior-panel/portable");
    let absent_before = snapshot_tree(&absent_portable)?;
    match super::publish(
        &absent._root.0,
        &absent.manifest,
        "target/ripr/judged-panel-fixture/current-missing.json",
    ) {
        Ok(()) => return Err("publish accepted an absent host current".to_string()),
        Err(error) if !error.contains("resolve host current") => {
            return Err(format!(
                "absent host current failed for the wrong reason: {error}"
            ));
        }
        Err(_) => {}
    }
    assert_tree_unchanged(&absent_portable, &absent_before, "absent publication")?;

    // Stale/tampered sidecar: flipping retained raw bytes after the digests
    // were bound must fail closed and leave the portable tree untouched.
    let tampered = on_disk_host_fixture("tampered-sidecar")?;
    let cases_root = tampered
        ._root
        .0
        .join("target/ripr/judged-panel-fixture/runs")
        .join(&tampered.run_id)
        .join("cases");
    let case_dirs = fs::read_dir(&cases_root)
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    let first_case = case_dirs
        .first()
        .ok_or_else(|| "fixture produced no case directories".to_string())?;
    fs::write(
        first_case.path().join("stdout.bin"),
        b"{\"tampered\": true}\n",
    )
    .map_err(|error| error.to_string())?;
    let tampered_portable = tampered
        ._root
        .0
        .join("metrics/rust-judged-behavior-panel/portable");
    let tampered_before = snapshot_tree(&tampered_portable)?;
    match super::publish(
        &tampered._root.0,
        &tampered.manifest,
        &tampered.host_current,
    ) {
        Ok(()) => return Err("publish accepted tampered retained raw evidence".to_string()),
        Err(error) if !error.contains("host receipt raw stdout identity mismatch") => {
            return Err(format!(
                "tampered evidence failed for the wrong reason: {error}"
            ));
        }
        Err(_) => {}
    }
    assert_tree_unchanged(&tampered_portable, &tampered_before, "tampered publication")?;
    Ok(())
}

#[test]
fn resolve_repository_gitdir_follows_relative_and_absolute_gitfiles() -> Result<(), String> {
    let root = scratch("resolve-gitdir")?;
    // A normal checkout: `.git` is a directory and resolves to itself.
    let checkout = root.join("checkout");
    fs::create_dir_all(checkout.join(".git")).map_err(|error| error.to_string())?;
    let resolved = resolve_repository_gitdir(&checkout)?;
    if resolved != checkout.join(".git").display().to_string() {
        return Err("directory `.git` did not resolve to itself".to_string());
    }
    // A linked worktree with a RELATIVE gitdir (interpreted against the
    // gitfile's directory, per git): the resolved gitdir must be absolute
    // and point at the real git data so a fixture can rebind to it.
    let real_git = root.join("meta").join("real-git");
    fs::create_dir_all(&real_git).map_err(|error| error.to_string())?;
    let worktree = root.join("wt");
    fs::create_dir_all(&worktree).map_err(|error| error.to_string())?;
    fs::write(worktree.join(".git"), "gitdir: ../meta/real-git\n")
        .map_err(|error| error.to_string())?;
    let resolved = resolve_repository_gitdir(&worktree)?;
    if resolved != real_git.display().to_string() {
        return Err(format!(
            "relative gitdir resolved to `{resolved}` instead of `{}`",
            real_git.display()
        ));
    }
    // An ABSOLUTE gitdir must also resolve through `join`'s absolute-path
    // replacement.
    fs::write(
        worktree.join(".git"),
        format!("gitdir: {}\n", real_git.display()),
    )
    .map_err(|error| error.to_string())?;
    let resolved = resolve_repository_gitdir(&worktree)?;
    if resolved != real_git.display().to_string() {
        return Err("absolute gitdir did not resolve to the real git dir".to_string());
    }
    // A dangling gitdir is a named error, never a silent bind.
    fs::write(worktree.join(".git"), "gitdir: ../meta/missing\n")
        .map_err(|error| error.to_string())?;
    let error = resolve_repository_gitdir(&worktree)
        .err()
        .ok_or_else(|| "dangling gitdir was accepted".to_string())?;
    if !error.contains("is not a git directory") {
        return Err(format!("unexpected dangling-gitdir error: {error}"));
    }
    fs::remove_dir_all(&root).map_err(|error| error.to_string())?;
    Ok(())
}
