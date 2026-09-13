//! End-to-end tests for the #3569 driver binding: the two-phase external-edit
//! driver (`ripr agent repair`) bound to the durable attempt identity (#2927)
//! and the accepted Python repair-trust selection model (RIPR-SPEC-0176,
//! #3568).
//!
//! Every test drives the real binary against a throwaway git repository with
//! a committed behavior change, a digest-verified selection manifest, and a
//! focused test-only edit. The case matrix pins the issue's safety rules:
//! stale packet, wrong target, zero and multiple (ambiguous) target matches,
//! denied edit surfaces, outside-root manifests, missing or mismatched
//! authorization, cage escape, production edits, generated paths, tampered
//! retained bindings, deterministic preparation, and the four distinct
//! durable states (prepared-but-not-applied, rejected, stale,
//! applied-but-unverified).
//!
//! The driver records preparation and application evidence only; the tests
//! also pin that no verification result, static movement, or closure is ever
//! claimed by the driver.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const TARGET_TEST_FILE: &str = "tests/pricing.rs";
const PRODUCTION_FILE: &str = "src/lib.rs";
/// Drop guard so a failing test never leaves a fixture repository behind.
struct TempFixture {
    root: PathBuf,
}

impl Drop for TempFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut rendered = String::with_capacity(64);
    for byte in digest {
        rendered.push_str(&format!("{byte:02x}"));
    }
    rendered
}

/// The canonical selection-row digest: sha256 over the row JSON with only
/// `selection_digest` removed, re-serialized with sorted keys (serde_json's
/// default map ordering) and compact separators — the same preimage the
/// driver and the corpus validator recompute.
fn canonical_row_digest(row: &Value) -> Result<String, String> {
    let object = row
        .as_object()
        .ok_or_else(|| "selection row is not an object".to_string())?;
    let mut canonical = object.clone();
    canonical.remove("selection_digest");
    let text = serde_json::to_string(&Value::Object(canonical))
        .map_err(|error| format!("canonical serialization failed: {error}"))?;
    Ok(sha256_hex(text.as_bytes()))
}

fn run_ripr(root: &Path, args: &[&str]) -> Result<Output, String> {
    let bin = env!("CARGO_BIN_EXE_ripr");
    Command::new(bin)
        .current_dir(root)
        .args(args)
        .output()
        .map_err(|error| format!("spawn ripr {args:?} failed: {error}"))
}

/// Runs `ripr agent repair` with a working directory that is NOT the
/// workspace root, so cwd-relative path resolution bugs surface. `root_arg`
/// is the value passed as `--root`; `args` are the flags after it.
fn run_ripr_repair_from(cwd: &Path, root_arg: &Path, args: &[&str]) -> Result<Output, String> {
    let bin = env!("CARGO_BIN_EXE_ripr");
    let mut command = Command::new(bin);
    command
        .current_dir(cwd)
        .args(["agent", "repair", "--root"])
        .arg(root_arg);
    for value in args {
        command.arg(value);
    }
    command
        .output()
        .map_err(|error| format!("spawn ripr agent repair {args:?} failed: {error}"))
}

/// An empty hooks directory passed via `-c core.hooksPath` on every fixture
/// git invocation: a host-configured `core.hooksPath` must never run inside
/// the fixture repository, because a host hook could reject or mutate a
/// fixture commit. The directory is created empty and portable across
/// platforms.
fn hooks_disabled_config(root: &Path) -> Result<Vec<String>, String> {
    let hooks_dir = root.join("ripr-empty-hooks");
    std::fs::create_dir_all(&hooks_dir)
        .map_err(|error| format!("create {} failed: {error}", hooks_dir.display()))?;
    Ok(vec![
        "-c".to_string(),
        format!("core.hooksPath={}", hooks_dir.display()),
    ])
}

fn run_git(root: &Path, args: &[&str]) -> Result<(), String> {
    let mut command = Command::new("git");
    command.current_dir(root);
    for value in hooks_disabled_config(root)? {
        command.arg(value);
    }
    command.args(args);
    let output = command
        .output()
        .map_err(|error| format!("spawn git {args:?} failed: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn stdout_text(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr_text(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

fn require_success(output: &Output, context: &str) -> Result<(), String> {
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "{context} failed (exit {:?}): {}{}",
        output.status.code(),
        stdout_text(output),
        stderr_text(output)
    ))
}

fn require_failure(output: &Output, context: &str, needle: &str) -> Result<String, String> {
    let combined = format!("{}{}", stdout_text(output), stderr_text(output));
    if output.status.success() {
        return Err(format!(
            "{context} was expected to fail naming `{needle}` but succeeded: {combined}"
        ));
    }
    if !combined.contains(needle) {
        return Err(format!(
            "{context} failed without naming `{needle}`: {combined}"
        ));
    }
    Ok(combined)
}

/// The sample workspace's source, embedded verbatim; the fixture commits the
/// `>` boundary as the base and the sample's `>=` as the analyzed behavior
/// change.
const SAMPLE_SRC: &str = include_str!("../examples/sample/src/lib.rs");

fn sample_src() -> String {
    SAMPLE_SRC.replace(
        "if amount >= discount_threshold",
        "if amount > discount_threshold",
    )
}

/// The committed behavior change: the sample's `>=` boundary as analyzed.
fn changed_src() -> String {
    SAMPLE_SRC.to_string()
}

const SAMPLE_TEST: &str = "#[test]
fn premium_customer_gets_discount() {
    let quote = ripr_sample::price(10_000, 100);
    assert!(quote.total > 0);
}

#[test]
fn rejects_bad_currency() {
    let result = ripr_sample::validate_currency(\"XYZ\");
    assert!(result.is_err());
}
";

fn unique_root(label: &str) -> Result<PathBuf, String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("test clock failed: {error}"))?
        .as_nanos();
    Ok(std::env::temp_dir().join(format!(
        "ripr-python-repair-attempt-{label}-{}-{stamp}",
        std::process::id()
    )))
}

/// A git repository with a committed behavior change, a clean worktree, and
/// `origin/main` pinned to the pre-change commit — the state the driver's
/// before phase requires.
fn build_fixture(label: &str) -> Result<TempFixture, String> {
    let root = unique_root(label)?;
    std::fs::create_dir_all(root.join("src")).map_err(|error| format!("create src: {error}"))?;
    std::fs::create_dir_all(root.join("tests"))
        .map_err(|error| format!("create tests: {error}"))?;
    std::fs::write(root.join("Cargo.toml"), "[workspace]\n")
        .map_err(|error| format!("write Cargo.toml: {error}"))?;
    std::fs::write(root.join(PRODUCTION_FILE), sample_src())
        .map_err(|error| format!("write baseline src: {error}"))?;
    std::fs::write(root.join(TARGET_TEST_FILE), SAMPLE_TEST)
        .map_err(|error| format!("write baseline test: {error}"))?;
    run_git(&root, &["init", "-q"])?;
    run_git(
        &root,
        &["config", "user.email", "ripr-test@example.invalid"],
    )?;
    run_git(&root, &["config", "user.name", "RIPR Test"])?;
    run_git(&root, &["config", "commit.gpgSign", "false"])?;
    run_git(&root, &["add", "."])?;
    run_git(&root, &["commit", "-qm", "base"])?;
    run_git(&root, &["update-ref", "refs/remotes/origin/main", "HEAD"])?;
    std::fs::write(root.join(PRODUCTION_FILE), changed_src())
        .map_err(|error| format!("write changed src: {error}"))?;
    run_git(&root, &["add", PRODUCTION_FILE])?;
    run_git(&root, &["commit", "-qm", "changed behavior"])?;
    std::fs::create_dir_all(root.join("target/ripr"))
        .map_err(|error| format!("create target/ripr: {error}"))?;
    Ok(TempFixture { root })
}

/// Writes the accepted selection manifest naming one attempt identity whose
/// target is the fixture's focused test file, with the canonical
/// `selection_digest` recomputed over the row content.
fn write_trust_manifest(
    root: &Path,
    attempt_id: &str,
    target_path: &str,
    target_state: &str,
) -> Result<(), String> {
    let head = current_head(root)?;
    let row = serde_json::json!({
        "attempt_id": attempt_id,
        "case_id": format!("case-{attempt_id}"),
        "subject_id": "fixture-subject",
        "repository": "https://example.com/fixture-subject",
        "base": "1111111111111111111111111111111111111111",
        "head": head,
        "selection_reason": "behavior changed in the diff and the case discriminates it",
        "diversity_stratum": "pytest_library",
        "family": "predicate_boundary",
        "owner": "price",
        "discriminator": "amount >= discount_threshold",
        "relation": "test calls owner directly",
        "oracle": "assert exact boundary value",
        "expected_direction": "should_gap",
        "claim_boundary": "static exposure evidence only",
        "target_path": target_path,
        "target_state": target_state,
        "selected_at": "2026-09-10T00:00:00Z",
        "selector": "campaign-selector",
        "authority_snapshot_digest": "2020202020202020202020202020202020202020202020202020202020202020",
    });
    let digest = canonical_row_digest(&row)?;
    let mut stored = row;
    if let Some(object) = stored.as_object_mut() {
        object.insert("selection_digest".to_string(), Value::String(digest));
    }
    let manifest = serde_json::json!({
        "schema_version": "0.1",
        "kind": "python_repair_trust_manifest",
        "spec": "RIPR-SPEC-0176",
        "description": "python repair attempt fixture",
        "selections": [stored],
    });
    let text = serde_json::to_string_pretty(&manifest)
        .map_err(|error| format!("serialize fixture manifest: {error}"))?;
    let path = root.join("target/ripr/trust-manifest.json");
    std::fs::write(&path, text).map_err(|error| format!("write {}: {error}", path.display()))
}

/// Writes one stable accepted manifest carrying one row per requested
/// attempt identity, all targeting the same test file.
fn write_trust_manifest_rows(
    root: &Path,
    attempt_ids: &[&str],
    target_path: &str,
    target_state: &str,
) -> Result<(), String> {
    let head = current_head(root)?;
    let mut rows = Vec::new();
    for attempt_id in attempt_ids {
        let row = serde_json::json!({
            "attempt_id": attempt_id,
            "case_id": format!("case-{attempt_id}"),
            "subject_id": "fixture-subject",
            "repository": "https://example.com/fixture-subject",
            "base": "1111111111111111111111111111111111111111",
            "head": head,
            "selection_reason": "behavior changed in the diff and the case discriminates it",
            "diversity_stratum": "pytest_library",
            "family": "predicate_boundary",
            "owner": "price",
            "discriminator": "amount >= discount_threshold",
            "relation": "test calls owner directly",
            "oracle": "assert exact boundary value",
            "expected_direction": "should_gap",
            "claim_boundary": "static exposure evidence only",
            "target_path": target_path,
            "target_state": target_state,
            "selected_at": "2026-09-10T00:00:00Z",
            "selector": "campaign-selector",
            "authority_snapshot_digest": "2020202020202020202020202020202020202020202020202020202020202020",
        });
        let digest = canonical_row_digest(&row)?;
        let mut stored = row;
        if let Some(object) = stored.as_object_mut() {
            object.insert("selection_digest".to_string(), Value::String(digest));
        }
        rows.push(stored);
    }
    let manifest = serde_json::json!({
        "schema_version": "0.1",
        "kind": "python_repair_trust_manifest",
        "spec": "RIPR-SPEC-0176",
        "description": "python repair attempt fixture",
        "selections": rows,
    });
    let text = serde_json::to_string_pretty(&manifest)
        .map_err(|error| format!("serialize fixture manifest: {error}"))?;
    let path = root.join("target/ripr/trust-manifest.json");
    std::fs::write(&path, text).map_err(|error| format!("write {}: {error}", path.display()))
}

fn current_head(root: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .current_dir(root)
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|error| format!("spawn git rev-parse failed: {error}"))?;
    if !output.status.success() {
        return Err("git rev-parse HEAD failed".to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Collects every `seam_id` string in a document, in order, deduplicated.
fn collect_seam_ids(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(text)
            if text.len() == 16
                && text.bytes().all(|byte| byte.is_ascii_hexdigit())
                && !out.iter().any(|seen| seen == text) =>
        {
            out.push(text.clone());
        }
        Value::String(_) => {}
        Value::Array(items) => {
            for item in items {
                collect_seam_ids(item, out);
            }
        }
        Value::Object(map) => {
            for item in map.values() {
                collect_seam_ids(item, out);
            }
        }
        _ => {}
    }
}

fn parse_json(text: &str, context: &str) -> Result<Value, String> {
    serde_json::from_str(text)
        .map_err(|error| format!("{context} is not valid JSON: {error}\n{text}"))
}

/// True when a packet document names `target` as its focused test target,
/// wherever the packet nests it (the renderer may wrap one seam packet in a
/// report envelope).
fn packet_selects_target(value: &Value, target: &str) -> bool {
    match value {
        Value::Object(map) => {
            let recommended = map
                .get("recommended_test")
                .and_then(|test| test.get("file"))
                .and_then(Value::as_str);
            let allowed = map
                .get("allowed_edit_surface")
                .and_then(Value::as_array)
                .map(|paths| {
                    paths
                        .iter()
                        .filter_map(Value::as_str)
                        .any(|path| path == target)
                })
                .unwrap_or(false);
            if recommended == Some(target) || allowed {
                return true;
            }
            map.values()
                .any(|child| packet_selects_target(child, target))
        }
        Value::Array(items) => items.iter().any(|item| packet_selects_target(item, target)),
        _ => false,
    }
}

/// Finds the seam whose packet selects `tests/pricing.rs` as the focused test
/// target, mirroring the binding's own identity-alignment rule.
fn find_seam_for_target(root: &Path, target: &str) -> Result<String, String> {
    let output = run_ripr(
        root,
        &["check", "--root", ".", "--format", "repo-exposure-json"],
    )?;
    require_success(&output, "repo exposure scan")?;
    let value = parse_json(&stdout_text(&output), "repo exposure JSON")?;
    let mut seam_ids = Vec::new();
    collect_seam_ids(&value, &mut seam_ids);
    for seam_id in &seam_ids {
        let packet = run_ripr(
            root,
            &[
                "agent",
                "packet",
                "--root",
                ".",
                "--seam-id",
                seam_id,
                "--json",
            ],
        )?;
        if !packet.status.success() {
            continue;
        }
        let value = parse_json(&stdout_text(&packet), "agent packet JSON")?;
        if packet_selects_target(&value, target) {
            return Ok(seam_id.clone());
        }
    }
    Err(format!(
        "no seam packet selects `{target}`; the fixture must expose an actionable gap whose repair target is the selected test file"
    ))
}

const AUTHORITY: &str = "test-operator";

fn prepare_args(seam_id: &str, manifest: &str, trust_attempt: &str) -> Vec<String> {
    vec![
        "agent".to_string(),
        "repair".to_string(),
        "--root".to_string(),
        ".".to_string(),
        "--seam-id".to_string(),
        seam_id.to_string(),
        "--phase".to_string(),
        "before".to_string(),
        "--python-repair-trust-manifest".to_string(),
        manifest.to_string(),
        "--python-repair-trust-attempt".to_string(),
        trust_attempt.to_string(),
        "--edit-authorized".to_string(),
        "--edit-authority".to_string(),
        AUTHORITY.to_string(),
    ]
}

fn run_prepare(
    fixture: &TempFixture,
    seam_id: &str,
    trust_attempt: &str,
) -> Result<Output, String> {
    write_trust_manifest(&fixture.root, trust_attempt, TARGET_TEST_FILE, "existing")?;
    let args = prepare_args(seam_id, "target/ripr/trust-manifest.json", trust_attempt);
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    run_ripr(&fixture.root, &borrowed)
}

fn run_apply(
    fixture: &TempFixture,
    attempt_id: &str,
    authority: Option<&str>,
) -> Result<Output, String> {
    let mut args: Vec<String> = vec![
        "agent".to_string(),
        "repair".to_string(),
        "--root".to_string(),
        ".".to_string(),
        "--attempt".to_string(),
        attempt_id.to_string(),
        "--phase".to_string(),
        "after".to_string(),
    ];
    if let Some(authority) = authority {
        args.push("--edit-authorized".to_string());
        args.push("--edit-authority".to_string());
        args.push(authority.to_string());
    }
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    run_ripr(&fixture.root, &borrowed)
}

/// The single repair attempt directory of a fixture (tests publish one
/// attempt at a time unless they deliberately publish several).
fn sole_attempt(fixture: &TempFixture) -> Result<String, String> {
    attempt_ids(fixture).map(|ids| {
        if ids.len() != 1 {
            return Err(format!(
                "expected exactly one published attempt, found {}: {:?}",
                ids.len(),
                ids
            ));
        }
        Ok(ids.into_iter().next().unwrap_or_default())
    })?
}

fn attempt_ids(fixture: &TempFixture) -> Result<Vec<String>, String> {
    let directory = fixture.root.join("target/ripr/repair-attempts");
    if !directory.is_dir() {
        return Ok(Vec::new());
    }
    let mut ids = Vec::new();
    for entry in std::fs::read_dir(&directory)
        .map_err(|error| format!("read {}: {error}", directory.display()))?
    {
        let entry = entry.map_err(|error| format!("read attempt entry: {error}"))?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("repair-attempt-") {
            ids.push(name);
        }
    }
    ids.sort();
    Ok(ids)
}

fn attempt_manifest(fixture: &TempFixture, attempt_id: &str) -> Result<Value, String> {
    let path = fixture
        .root
        .join("target/ripr/repair-attempts")
        .join(attempt_id)
        .join("attempt.json");
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("read {}: {error}", path.display()))?;
    parse_json(&text, "attempt manifest")
}

fn binding_artifact_path(fixture: &TempFixture, attempt_id: &str) -> Result<PathBuf, String> {
    Ok(fixture
        .root
        .join("target/ripr/repair-attempts")
        .join(attempt_id)
        .join("artifacts/python-repair-trust-binding.json"))
}

fn prepare_record(fixture: &TempFixture, attempt_id: &str) -> Result<Value, String> {
    let path = binding_artifact_path(fixture, attempt_id)?;
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("read {}: {error}", path.display()))?;
    parse_json(&text, "prepare binding record")
}

fn apply_record(fixture: &TempFixture) -> Result<Value, String> {
    let path = fixture
        .root
        .join("target/ripr/workflow/python-repair-driver-after.json");
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("read {}: {error}", path.display()))?;
    parse_json(&text, "apply record")
}

/// Appends the focused boundary discriminator to the selected test file.
fn edit_target_file(fixture: &TempFixture) -> Result<(), String> {
    let path = fixture.root.join(TARGET_TEST_FILE);
    let mut current = std::fs::read_to_string(&path)
        .map_err(|error| format!("read {}: {error}", path.display()))?;
    current.push_str("\n#[test]\nfn boundary_discriminator() {\n    assert!(ripr_sample::price(100, 100).discount_applied);\n}\n");
    std::fs::write(&path, current).map_err(|error| format!("write {}: {error}", path.display()))
}

const NON_CLAIM_COUNT: usize = 3;

fn assert_prepare_record_identity(record: &Value) -> Result<(), String> {
    for field in ["schema_version", "kind", "spec", "phase", "seam_id"] {
        if record.get(field).and_then(Value::as_str).is_none() {
            return Err(format!("prepare record is missing `{field}`"));
        }
    }
    if record.get("phase").and_then(Value::as_str) != Some("prepare") {
        return Err("prepare record carries the wrong phase".to_string());
    }
    let trust = record
        .get("trust")
        .and_then(Value::as_object)
        .ok_or("prepare record carries no trust block")?;
    for field in [
        "attempt_id",
        "selection_manifest_sha256",
        "selection_digest",
        "target_path",
        "target_state",
        "family",
        "owner",
        "discriminator",
    ] {
        if !trust.contains_key(field) {
            return Err(format!("prepare trust block is missing `{field}`"));
        }
    }
    for field in ["packet_sha256", "before_snapshot_sha256"] {
        match record.get("input").and_then(|input| input.get(field)) {
            Some(Value::String(digest)) if digest.len() == 64 => {}
            _ => return Err(format!("prepare input block is missing `{field}`")),
        }
    }
    let authorization = record
        .get("authorization")
        .and_then(Value::as_object)
        .ok_or("prepare record carries no authorization")?;
    if authorization.get("status").and_then(Value::as_str) != Some("granted")
        || authorization.get("method").and_then(Value::as_str) != Some("explicit-operator-flags")
        || authorization.get("authority").and_then(Value::as_str) != Some(AUTHORITY)
    {
        return Err("prepare record authorization is not the explicit granted pair".to_string());
    }
    let non_claims = record
        .get("non_claims")
        .and_then(Value::as_array)
        .ok_or("prepare record carries no non_claims")?;
    if non_claims.len() != NON_CLAIM_COUNT {
        return Err(format!(
            "prepare record carries {} non-claims, expected {NON_CLAIM_COUNT}",
            non_claims.len()
        ));
    }
    if record.get("apply").is_some() || record.get("durable_attempt_id").is_some() {
        return Err("prepare record must not carry apply-phase fields".to_string());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Case matrix
// ---------------------------------------------------------------------------

#[test]
fn clean_test_only_edit_binds_prepare_and_apply() -> Result<(), String> {
    let fixture = build_fixture("clean-positive")?;
    let seam_id = find_seam_for_target(&fixture.root, TARGET_TEST_FILE)?;
    let prepared = run_prepare(&fixture, &seam_id, "att-clean")?;
    require_success(&prepared, "bound before phase")?;
    let attempt_id = sole_attempt(&fixture)?;

    let manifest = attempt_manifest(&fixture, &attempt_id)?;
    if manifest.get("state").and_then(Value::as_str) != Some("awaiting_edit") {
        return Err(format!(
            "prepared attempt state is not awaiting_edit: {manifest:?}"
        ));
    }
    // A trust-bound attempt always re-verifies the explicit authorization at
    // apply time, so the published follow-up command must name the required
    // flags with an explicit placeholder identity — executing it verbatim
    // cannot bypass the authorization pair.
    let next_command = manifest
        .get("next_command")
        .and_then(Value::as_str)
        .ok_or("attempt manifest carries no next command")?;
    for fragment in ["--edit-authorized", "--edit-authority"] {
        if !next_command.contains(fragment) {
            return Err(format!(
                "trust-bound next command does not name `{fragment}`: {next_command}"
            ));
        }
    }
    let record = prepare_record(&fixture, &attempt_id)?;
    assert_prepare_record_identity(&record)?;
    if record
        .get("trust")
        .and_then(|trust| trust.get("attempt_id"))
        .and_then(Value::as_str)
        != Some("att-clean")
    {
        return Err("prepare record does not name the requested trust attempt".to_string());
    }

    edit_target_file(&fixture)?;
    let applied = run_apply(&fixture, &attempt_id, Some(AUTHORITY))?;
    require_success(&applied, "bound after phase")?;

    let manifest = attempt_manifest(&fixture, &attempt_id)?;
    if manifest.get("state").and_then(Value::as_str) != Some("ready_to_finish") {
        return Err(format!(
            "applied attempt state is not ready_to_finish: {manifest:?}"
        ));
    }
    let after = manifest
        .get("after")
        .ok_or("applied attempt carries no after block")?;
    if after
        .get("verdict")
        .and_then(|verdict| verdict.get("status"))
        .and_then(Value::as_str)
        != Some("compliant")
    {
        return Err(format!(
            "clean edit cage verdict is not compliant: {after:?}"
        ));
    }
    let apply = apply_record(&fixture)?;
    if apply.get("phase").and_then(Value::as_str) != Some("apply") {
        return Err("apply record carries the wrong phase".to_string());
    }
    if apply.get("durable_attempt_id").and_then(Value::as_str) != Some(attempt_id.as_str()) {
        return Err("apply record does not name the durable attempt".to_string());
    }
    let apply_block = apply
        .get("apply")
        .and_then(Value::as_object)
        .ok_or("apply record carries no apply block")?;
    if apply_block.get("cage_status").and_then(Value::as_str) != Some("compliant") {
        return Err("apply record cage status is not compliant".to_string());
    }
    if apply_block.get("current") != Some(&Value::Bool(true)) {
        return Err("apply record does not record a current repository".to_string());
    }
    // The applied patch digest must be the durable attempt's own delta digest.
    let delta = after
        .get("delta_sha256")
        .and_then(Value::as_str)
        .ok_or("durable after block carries no delta digest")?;
    let patch = apply_block
        .get("patch_sha256")
        .and_then(Value::as_str)
        .ok_or("apply record carries no patch digest")?;
    if delta.trim_start_matches("sha256:") != patch {
        return Err(
            "apply record patch digest does not match the durable delta digest".to_string(),
        );
    }
    if apply
        .get("binding_artifact_sha256")
        .and_then(Value::as_str)
        .map(str::len)
        != Some(64)
    {
        return Err("apply record binding artifact digest is not 64 hex".to_string());
    }
    Ok(())
}

#[test]
fn relative_manifest_resolves_against_the_selected_root() -> Result<(), String> {
    let fixture = build_fixture("relative-manifest")?;
    let seam_id = find_seam_for_target(&fixture.root, TARGET_TEST_FILE)?;
    write_trust_manifest(&fixture.root, "att-rel", TARGET_TEST_FILE, "existing")?;

    // Launch from a working directory that is NOT the workspace: the
    // root-relative manifest argument must resolve against --root (the same
    // semantics as every other workflow artifact path), never against the
    // process working directory.
    let launcher = unique_root("relative-cwd")?;
    std::fs::create_dir_all(&launcher).map_err(|error| format!("create launcher: {error}"))?;
    let guard = TempFixture {
        root: launcher.clone(),
    };
    let args: Vec<String> = vec![
        "--seam-id".to_string(),
        seam_id.clone(),
        "--phase".to_string(),
        "before".to_string(),
        "--python-repair-trust-manifest".to_string(),
        "target/ripr/trust-manifest.json".to_string(),
        "--python-repair-trust-attempt".to_string(),
        "att-rel".to_string(),
        "--edit-authorized".to_string(),
        "--edit-authority".to_string(),
        AUTHORITY.to_string(),
    ];
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    let prepared = run_ripr_repair_from(&launcher, &fixture.root, &borrowed)?;
    require_success(
        &prepared,
        "prepare with a root-relative manifest from another working directory",
    )?;

    // The attempt (and its staged binding record) landed inside the selected
    // workspace, proving the manifest was read from the root-relative
    // location.
    let attempt_id = sole_attempt(&fixture)?;
    let record = prepare_record(&fixture, &attempt_id)?;
    assert_prepare_record_identity(&record)?;

    drop(guard);
    Ok(())
}

#[test]
fn stale_packet_fails_before_the_edit_is_recorded() -> Result<(), String> {
    let fixture = build_fixture("stale-packet")?;
    let seam_id = find_seam_for_target(&fixture.root, TARGET_TEST_FILE)?;
    let prepared = run_prepare(&fixture, &seam_id, "att-stale")?;
    require_success(&prepared, "bound before phase")?;
    let attempt_id = sole_attempt(&fixture)?;
    edit_target_file(&fixture)?;

    // Mutate the accepted manifest after preparation: the retained binding
    // pins its exact-bytes digest, so the apply must refuse.
    let manifest_path = fixture.root.join("target/ripr/trust-manifest.json");
    let mut manifest = parse_json(
        &std::fs::read_to_string(&manifest_path)
            .map_err(|error| format!("read manifest: {error}"))?,
        "trust manifest",
    )?;
    if let Some(object) = manifest.as_object_mut() {
        object.insert(
            "description".to_string(),
            Value::String("mutated after prepare".to_string()),
        );
    }
    let text = serde_json::to_string_pretty(&manifest)
        .map_err(|error| format!("serialize mutated manifest: {error}"))?;
    std::fs::write(&manifest_path, text)
        .map_err(|error| format!("write mutated manifest: {error}"))?;

    let applied = run_apply(&fixture, &attempt_id, Some(AUTHORITY))?;
    require_failure(
        &applied,
        "apply over a mutated manifest",
        "stale selection manifest",
    )?;
    let manifest = attempt_manifest(&fixture, &attempt_id)?;
    if manifest.get("state").and_then(Value::as_str) != Some("awaiting_edit") {
        return Err("stale-manifest refusal changed the durable state".to_string());
    }
    if fixture
        .root
        .join("target/ripr/workflow/python-repair-driver-after.json")
        .exists()
    {
        return Err("stale-manifest refusal wrote an apply record".to_string());
    }
    Ok(())
}

#[test]
fn wrong_target_fails_before_the_attempt_is_published() -> Result<(), String> {
    let fixture = build_fixture("wrong-target")?;
    let seam_id = find_seam_for_target(&fixture.root, TARGET_TEST_FILE)?;
    // The selection row names an existing repository file that is not the
    // packet's selected edit target.
    write_trust_manifest(&fixture.root, "att-wrong", PRODUCTION_FILE, "existing")?;
    let args = prepare_args(&seam_id, "target/ripr/trust-manifest.json", "att-wrong");
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    let prepared = run_ripr(&fixture.root, &borrowed)?;
    require_failure(&prepared, "prepare with a wrong target", "wrong target")?;
    let ids = attempt_ids(&fixture)?;
    if !ids.is_empty() {
        return Err("a wrong-target preparation published a durable attempt".to_string());
    }
    Ok(())
}

#[test]
fn generated_and_vendor_targets_fail_before_editing() -> Result<(), String> {
    let fixture = build_fixture("denied-target")?;
    let seam_id = find_seam_for_target(&fixture.root, TARGET_TEST_FILE)?;

    // A generated-surface target is refused even before its state matters.
    write_trust_manifest(&fixture.root, "att-gen", "generated/helper.py", "existing")?;
    let args = prepare_args(&seam_id, "target/ripr/trust-manifest.json", "att-gen");
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    let prepared = run_ripr(&fixture.root, &borrowed)?;
    require_failure(
        &prepared,
        "prepare with a generated target",
        "production/generated/vendor/environment edit surface",
    )?;

    // A vendor target declared `unsafe` is refused too: the driver binds only
    // existing, test-only targets.
    write_trust_manifest(&fixture.root, "att-vendor", "vendor/lib/ext.py", "unsafe")?;
    let args = prepare_args(&seam_id, "target/ripr/trust-manifest.json", "att-vendor");
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    let prepared = run_ripr(&fixture.root, &borrowed)?;
    require_failure(
        &prepared,
        "prepare with a vendor target",
        "target_state `unsafe`",
    )?;
    let ids = attempt_ids(&fixture)?;
    if !ids.is_empty() {
        return Err("a denied-surface preparation published a durable attempt".to_string());
    }
    Ok(())
}

#[test]
fn outside_root_manifest_refused() -> Result<(), String> {
    let fixture = build_fixture("outside-root")?;
    let seam_id = find_seam_for_target(&fixture.root, TARGET_TEST_FILE)?;
    let outside = unique_root("outside-root-manifest")?;
    std::fs::create_dir_all(&outside).map_err(|error| format!("create outside dir: {error}"))?;
    let guard = TempFixture {
        root: outside.clone(),
    };

    // Write a manifest that lives outside the repository root.
    write_trust_manifest(&fixture.root, "att-outside", TARGET_TEST_FILE, "existing")?;
    let text = std::fs::read(fixture.root.join("target/ripr/trust-manifest.json"))
        .map_err(|error| format!("read manifest: {error}"))?;
    std::fs::write(outside.join("manifest.json"), text)
        .map_err(|error| format!("write outside manifest: {error}"))?;
    let outside_arg = outside.join("manifest.json").to_string_lossy().to_string();
    let args = vec![
        "agent".to_string(),
        "repair".to_string(),
        "--root".to_string(),
        ".".to_string(),
        "--seam-id".to_string(),
        seam_id,
        "--phase".to_string(),
        "before".to_string(),
        "--python-repair-trust-manifest".to_string(),
        outside_arg,
        "--python-repair-trust-attempt".to_string(),
        "att-outside".to_string(),
        "--edit-authorized".to_string(),
        "--edit-authority".to_string(),
        AUTHORITY.to_string(),
    ];
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    let prepared = run_ripr(&fixture.root, &borrowed)?;
    require_failure(
        &prepared,
        "prepare with an outside-root manifest",
        "outside the repository root",
    )?;
    drop(guard);
    Ok(())
}

#[test]
fn missing_authorization_refused_with_typed_signals() -> Result<(), String> {
    let fixture = build_fixture("missing-auth")?;
    let seam_id = find_seam_for_target(&fixture.root, TARGET_TEST_FILE)?;
    write_trust_manifest(&fixture.root, "att-auth", TARGET_TEST_FILE, "existing")?;

    // Trust flags without the explicit authorization pair.
    let args = vec![
        "agent".to_string(),
        "repair".to_string(),
        "--root".to_string(),
        ".".to_string(),
        "--seam-id".to_string(),
        seam_id.clone(),
        "--phase".to_string(),
        "before".to_string(),
        "--python-repair-trust-manifest".to_string(),
        "target/ripr/trust-manifest.json".to_string(),
        "--python-repair-trust-attempt".to_string(),
        "att-auth".to_string(),
    ];
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    let prepared = run_ripr(&fixture.root, &borrowed)?;
    require_failure(
        &prepared,
        "prepare without authorization",
        "--edit-authorized",
    )?;

    // The flag without the authority identity is refused too.
    let args = vec![
        "agent".to_string(),
        "repair".to_string(),
        "--root".to_string(),
        ".".to_string(),
        "--seam-id".to_string(),
        seam_id,
        "--phase".to_string(),
        "before".to_string(),
        "--python-repair-trust-manifest".to_string(),
        "target/ripr/trust-manifest.json".to_string(),
        "--python-repair-trust-attempt".to_string(),
        "att-auth".to_string(),
        "--edit-authorized".to_string(),
    ];
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    let prepared = run_ripr(&fixture.root, &borrowed)?;
    require_failure(
        &prepared,
        "prepare with a flag but no authority",
        "--edit-authority",
    )?;
    let ids = attempt_ids(&fixture)?;
    if !ids.is_empty() {
        return Err("an unauthorized preparation published a durable attempt".to_string());
    }
    Ok(())
}

#[test]
fn apply_without_or_with_wrong_authority_refused() -> Result<(), String> {
    let fixture = build_fixture("apply-auth")?;
    let seam_id = find_seam_for_target(&fixture.root, TARGET_TEST_FILE)?;
    let prepared = run_prepare(&fixture, &seam_id, "att-apply-auth")?;
    require_success(&prepared, "bound before phase")?;
    let attempt_id = sole_attempt(&fixture)?;
    edit_target_file(&fixture)?;

    // No authorization flags at all.
    let applied = run_apply(&fixture, &attempt_id, None)?;
    require_failure(&applied, "apply without authorization", "--edit-authorized")?;
    let manifest = attempt_manifest(&fixture, &attempt_id)?;
    if manifest.get("state").and_then(Value::as_str) != Some("awaiting_edit") {
        return Err("unauthorized apply changed the durable state".to_string());
    }

    // A different authority must re-authorize through a new attempt.
    let applied = run_apply(&fixture, &attempt_id, Some("someone-else"))?;
    require_failure(
        &applied,
        "apply with the wrong authority",
        "does not match the retained authorization authority",
    )?;
    let manifest = attempt_manifest(&fixture, &attempt_id)?;
    if manifest.get("state").and_then(Value::as_str) != Some("awaiting_edit") {
        return Err("wrong-authority apply changed the durable state".to_string());
    }
    Ok(())
}

#[test]
fn zero_target_match_fails_before_editing() -> Result<(), String> {
    let fixture = build_fixture("zero-target")?;
    let seam_id = find_seam_for_target(&fixture.root, TARGET_TEST_FILE)?;
    // The row claims an existing target that resolves to no file.
    write_trust_manifest(
        &fixture.root,
        "att-zero",
        "tests/missing_test.rs",
        "existing",
    )?;
    let args = prepare_args(&seam_id, "target/ripr/trust-manifest.json", "att-zero");
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    let prepared = run_ripr(&fixture.root, &borrowed)?;
    require_failure(
        &prepared,
        "prepare with a zero-match target",
        "matches no file",
    )?;
    let ids = attempt_ids(&fixture)?;
    if !ids.is_empty() {
        return Err("a zero-match preparation published a durable attempt".to_string());
    }
    Ok(())
}

#[test]
fn ambiguous_target_fails_before_editing() -> Result<(), String> {
    let fixture = build_fixture("ambiguous-target")?;
    let seam_id = find_seam_for_target(&fixture.root, TARGET_TEST_FILE)?;
    // The model's own `ambiguous` state must not be bound: a changed or
    // ambiguous target requires a new selection.
    write_trust_manifest(&fixture.root, "att-amb", TARGET_TEST_FILE, "ambiguous")?;
    let args = prepare_args(&seam_id, "target/ripr/trust-manifest.json", "att-amb");
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    let prepared = run_ripr(&fixture.root, &borrowed)?;
    require_failure(
        &prepared,
        "prepare with an ambiguous target",
        "target_state `ambiguous`",
    )?;
    let ids = attempt_ids(&fixture)?;
    if !ids.is_empty() {
        return Err("an ambiguous-target preparation published a durable attempt".to_string());
    }
    Ok(())
}

#[test]
fn prepared_but_not_applied_is_awaiting_edit() -> Result<(), String> {
    let fixture = build_fixture("prepared-state")?;
    let seam_id = find_seam_for_target(&fixture.root, TARGET_TEST_FILE)?;
    let prepared = run_prepare(&fixture, &seam_id, "att-awaiting")?;
    require_success(&prepared, "bound before phase")?;
    let attempt_id = sole_attempt(&fixture)?;
    let manifest = attempt_manifest(&fixture, &attempt_id)?;
    if manifest.get("state").and_then(Value::as_str) != Some("awaiting_edit") {
        return Err(format!(
            "prepared attempt is not awaiting_edit: {manifest:?}"
        ));
    }
    if manifest
        .get("after")
        .map(|after| !after.is_null())
        .unwrap_or(false)
    {
        return Err("a prepared attempt carries an after verdict".to_string());
    }
    Ok(())
}

#[test]
fn repository_drift_records_the_typed_stale_state() -> Result<(), String> {
    let fixture = build_fixture("stale-state")?;
    let seam_id = find_seam_for_target(&fixture.root, TARGET_TEST_FILE)?;
    let prepared = run_prepare(&fixture, &seam_id, "att-drift")?;
    require_success(&prepared, "bound before phase")?;
    let attempt_id = sole_attempt(&fixture)?;

    // Move the repository HEAD after preparation; the binding itself is
    // intact, so the durable finish must record the typed stale state rather
    // than the apply being refused.
    std::fs::write(fixture.root.join("README.md"), "# drifted\n")
        .map_err(|error| format!("write README: {error}"))?;
    run_git(&fixture.root, &["add", "README.md"])?;
    run_git(&fixture.root, &["commit", "-qm", "drift"])?;
    edit_target_file(&fixture)?;

    let applied = run_apply(&fixture, &attempt_id, Some(AUTHORITY))?;
    // The after phase surfaces the refusal of the receipt (a stale attempt is
    // not receipt-ready), but the durable state must be stale either way.
    let manifest = attempt_manifest(&fixture, &attempt_id)?;
    if manifest.get("state").and_then(Value::as_str) != Some("stale") {
        return Err(format!(
            "drifted attempt did not end stale (apply exit {:?}): {manifest:?}",
            applied.status.code()
        ));
    }
    let after = manifest
        .get("after")
        .ok_or("stale attempt carries no after block")?;
    if after.get("current") != Some(&Value::Bool(false)) {
        return Err("stale attempt is recorded as current".to_string());
    }
    if fixture
        .root
        .join("target/ripr/workflow/python-repair-driver-after.json")
        .exists()
    {
        let apply = apply_record(&fixture)?;
        if apply.get("apply").and_then(|block| block.get("current")) != Some(&Value::Bool(false)) {
            return Err("apply record does not carry the drift as not-current".to_string());
        }
    }
    Ok(())
}

#[test]
fn cage_escape_production_and_generated_edits_fail_closed_and_are_retained() -> Result<(), String> {
    let fixture = build_fixture("escape")?;
    let seam_id = find_seam_for_target(&fixture.root, TARGET_TEST_FILE)?;
    let prepared = run_prepare(&fixture, &seam_id, "att-escape")?;
    require_success(&prepared, "bound before phase")?;
    let attempt_id = sole_attempt(&fixture)?;

    // Edit the declared target AND escape the cage: a production edit and a
    // generated path.
    edit_target_file(&fixture)?;
    let production = fixture.root.join(PRODUCTION_FILE);
    let mut src = std::fs::read_to_string(&production)
        .map_err(|error| format!("read production file: {error}"))?;
    src.push_str("\n// smuggled production edit\n");
    std::fs::write(&production, src).map_err(|error| format!("write production file: {error}"))?;
    let generated_dir = fixture.root.join("generated");
    std::fs::create_dir_all(&generated_dir)
        .map_err(|error| format!("create generated dir: {error}"))?;
    std::fs::write(generated_dir.join("helper.py"), "value = 1\n")
        .map_err(|error| format!("write generated file: {error}"))?;

    let applied = run_apply(&fixture, &attempt_id, Some(AUTHORITY))?;
    if applied.status.success() {
        return Err("an escaping edit was accepted by the after phase".to_string());
    }
    let manifest = attempt_manifest(&fixture, &attempt_id)?;
    if manifest.get("state").and_then(Value::as_str) != Some("failed") {
        return Err(format!("escaping attempt did not end failed: {manifest:?}"));
    }
    let after = manifest
        .get("after")
        .ok_or("escaping attempt carries no after block")?;
    if after
        .get("verdict")
        .and_then(|verdict| verdict.get("status"))
        .and_then(Value::as_str)
        != Some("violated")
    {
        return Err("escaping attempt verdict is not violated".to_string());
    }
    let changed = after
        .get("verdict")
        .and_then(|verdict| verdict.get("changed_paths"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for expected in [PRODUCTION_FILE, "generated/helper.py", TARGET_TEST_FILE] {
        if !changed.contains(&Value::String(expected.to_string())) {
            return Err(format!(
                "the retained escape verdict does not record `{expected}`: {changed:?}"
            ));
        }
    }
    // The typed apply record must also carry the violated cage decision.
    let apply = apply_record(&fixture)?;
    if apply
        .get("apply")
        .and_then(|block| block.get("cage_status"))
        .and_then(Value::as_str)
        != Some("violated")
    {
        return Err("apply record does not retain the violated cage decision".to_string());
    }
    Ok(())
}

#[test]
fn failed_apply_record_publication_stays_retryable() -> Result<(), String> {
    let fixture = build_fixture("record-retry")?;
    let seam_id = find_seam_for_target(&fixture.root, TARGET_TEST_FILE)?;
    let prepared = run_prepare(&fixture, &seam_id, "att-retry")?;
    require_success(&prepared, "bound before phase")?;
    let attempt_id = sole_attempt(&fixture)?;
    edit_target_file(&fixture)?;

    // Block the apply-record destination with a directory: the record write
    // fails after the durable finish has advanced, and the attempt must be
    // restored to awaiting_edit instead of being stranded unrecoverably.
    let apply_record_path = fixture
        .root
        .join("target/ripr/workflow/python-repair-driver-after.json");
    std::fs::create_dir_all(&apply_record_path)
        .map_err(|error| format!("create blocking directory: {error}"))?;
    let blocked = run_apply(&fixture, &attempt_id, Some(AUTHORITY))?;
    require_failure(
        &blocked,
        "apply with a blocked record destination",
        "python-repair-driver-after",
    )?;
    let manifest = attempt_manifest(&fixture, &attempt_id)?;
    if manifest.get("state").and_then(Value::as_str) != Some("awaiting_edit") {
        return Err(format!(
            "a failed record publication left the attempt non-retryable: {manifest:?}"
        ));
    }

    // Unblock the destination: the identical retry completes with the record.
    std::fs::remove_dir_all(&apply_record_path)
        .map_err(|error| format!("remove blocking directory: {error}"))?;
    let retried = run_apply(&fixture, &attempt_id, Some(AUTHORITY))?;
    require_success(&retried, "retry after a failed record publication")?;
    let apply = apply_record(&fixture)?;
    if apply.get("phase").and_then(Value::as_str) != Some("apply") {
        return Err("the retried apply record carries the wrong phase".to_string());
    }
    Ok(())
}

#[test]
fn tampered_retained_binding_fails_closed() -> Result<(), String> {
    let fixture = build_fixture("tampered")?;
    let seam_id = find_seam_for_target(&fixture.root, TARGET_TEST_FILE)?;
    let prepared = run_prepare(&fixture, &seam_id, "att-tamper")?;
    require_success(&prepared, "bound before phase")?;
    let attempt_id = sole_attempt(&fixture)?;
    edit_target_file(&fixture)?;

    // Inject an unknown field into the retained binding artifact: its digest
    // is pinned by the durable manifest, so the apply must refuse.
    let path = binding_artifact_path(&fixture, &attempt_id)?;
    let original = std::fs::read_to_string(&path)
        .map_err(|error| format!("read binding artifact: {error}"))?;
    let tampered = original.replacen('{', "{\n  \"smuggled_field\": true,", 1);
    if tampered == original {
        return Err("tampering did not change the binding artifact".to_string());
    }
    std::fs::write(&path, tampered).map_err(|error| format!("write tampered artifact: {error}"))?;

    let applied = run_apply(&fixture, &attempt_id, Some(AUTHORITY))?;
    require_failure(
        &applied,
        "apply over a tampered binding",
        "artifact binding failed",
    )?;
    Ok(())
}

#[test]
fn deterministic_preparation_is_byte_identical() -> Result<(), String> {
    let fixture = build_fixture("deterministic")?;
    let seam_id = find_seam_for_target(&fixture.root, TARGET_TEST_FILE)?;
    // Two durable preparations binding the SAME selection: the row identity
    // is fixed, and each prepare publishes its own durable attempt.
    let first = run_prepare(&fixture, &seam_id, "att-det")?;
    require_success(&first, "first bound before phase")?;
    let second = run_prepare(&fixture, &seam_id, "att-det")?;
    require_success(&second, "second bound before phase")?;

    let ids = attempt_ids(&fixture)?;
    if ids.len() != 2 {
        return Err(format!(
            "expected two published attempts, found {}",
            ids.len()
        ));
    }
    if ids[0] == ids[1] {
        return Err("two preparations collided on one durable attempt identity".to_string());
    }
    let first_record = std::fs::read(binding_artifact_path(&fixture, &ids[0])?)
        .map_err(|error| format!("read first binding: {error}"))?;
    let second_record = std::fs::read(binding_artifact_path(&fixture, &ids[1])?)
        .map_err(|error| format!("read second binding: {error}"))?;
    if first_record != second_record {
        return Err(
            "equivalent preparations produced different binding records; only declared telemetry may differ and the record carries none"
                .to_string(),
        );
    }
    Ok(())
}

#[test]
fn driver_claims_no_verification_movement_or_closure() -> Result<(), String> {
    let fixture = build_fixture("no-claims")?;
    let seam_id = find_seam_for_target(&fixture.root, TARGET_TEST_FILE)?;
    let prepared = run_prepare(&fixture, &seam_id, "att-claims")?;
    require_success(&prepared, "bound before phase")?;
    let attempt_id = sole_attempt(&fixture)?;
    edit_target_file(&fixture)?;
    let applied = run_apply(&fixture, &attempt_id, Some(AUTHORITY))?;
    require_success(&applied, "bound after phase")?;

    let forbidden_record_fields = [
        "states",
        "movement",
        "execution",
        "verified",
        "reviewed",
        "accepted",
        "closure",
    ];
    for (name, text) in [
        (
            "prepare record",
            std::fs::read_to_string(binding_artifact_path(&fixture, &attempt_id)?)
                .map_err(|error| format!("read prepare record: {error}"))?,
        ),
        (
            "apply record",
            std::fs::read_to_string(
                fixture
                    .root
                    .join("target/ripr/workflow/python-repair-driver-after.json"),
            )
            .map_err(|error| format!("read apply record: {error}"))?,
        ),
    ] {
        let value = parse_json(&text, name)?;
        for field in forbidden_record_fields {
            if value.get(field).is_some() {
                return Err(format!("{name} carries the lifecycle field `{field}`"));
            }
        }
        let non_claims = value
            .get("non_claims")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("{name} carries no non_claims"))?;
        if non_claims.len() != NON_CLAIM_COUNT {
            return Err(format!("{name} carries {} non-claims", non_claims.len()));
        }
        for expected in ["verification", "static movement", "closure"] {
            if !non_claims.iter().any(|non_claim| {
                non_claim
                    .as_str()
                    .map(|text| text.contains(expected))
                    .unwrap_or(false)
            }) {
                return Err(format!(
                    "{name} non-claims do not cover `{expected}`: {non_claims:?}"
                ));
            }
        }
        if non_claims.iter().any(|claim| claim.as_str().is_none()) {
            return Err(format!("{name} carries a non-string non-claim"));
        }
    }
    Ok(())
}

#[test]
fn states_remain_distinct_across_one_repository() -> Result<(), String> {
    let fixture = build_fixture("state-matrix")?;
    let seam_id = find_seam_for_target(&fixture.root, TARGET_TEST_FILE)?;

    // One stable accepted manifest carries all four selections; a manifest
    // rewritten after a prepare would (by design) stale that prepare.
    write_trust_manifest_rows(
        &fixture.root,
        &["att-awaiting", "att-applied", "att-rejected", "att-stale"],
        TARGET_TEST_FILE,
        "existing",
    )?;

    // All four attempts are prepared BEFORE the first edit: once the focused
    // discriminator lands, the seam stops being actionable and further
    // preparations would (correctly) fail.
    for trust_attempt in ["att-awaiting", "att-applied", "att-rejected", "att-stale"] {
        let args = prepare_args(&seam_id, "target/ripr/trust-manifest.json", trust_attempt);
        let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
        let prepared = run_ripr(&fixture.root, &borrowed)?;
        require_success(&prepared, &format!("prepare for {trust_attempt}"))?;
    }
    let ids = attempt_ids(&fixture)?;
    if ids.len() != 4 {
        return Err(format!(
            "expected four published attempts, found {}",
            ids.len()
        ));
    }
    // The four preparations ran sequentially over one pristine worktree, so
    // the durable attempts differ only in identity; bind each scenario to one
    // attempt by its position and verify every state at the end.
    let awaiting_id = ids[0].clone();
    let applied_id = ids[1].clone();
    let rejected_id = ids[2].clone();
    let stale_id = ids[3].clone();

    // Attempt 2: applied cleanly becomes applied-but-unverified.
    edit_target_file(&fixture)?;
    let result = run_apply(&fixture, &applied_id, Some(AUTHORITY))?;
    require_success(&result, "clean apply")?;

    // Attempt 3: an escaping edit is rejected (retained as failed).
    let mut test_path = std::fs::read_to_string(fixture.root.join(TARGET_TEST_FILE))
        .map_err(|error| format!("read target: {error}"))?;
    test_path.push_str(
        "
#[test]
fn another_discriminator() {
    assert!(ripr_sample::price(101, 100).discount_applied);
}
",
    );
    std::fs::write(fixture.root.join(TARGET_TEST_FILE), test_path)
        .map_err(|error| format!("write target: {error}"))?;
    let production = fixture.root.join(PRODUCTION_FILE);
    let mut src = std::fs::read_to_string(&production)
        .map_err(|error| format!("read production: {error}"))?;
    src.push_str(
        "
// smuggled
",
    );
    std::fs::write(&production, src).map_err(|error| format!("write production: {error}"))?;
    let result = run_apply(&fixture, &rejected_id, Some(AUTHORITY))?;
    if result.status.success() {
        return Err("escaping attempt was accepted".to_string());
    }

    // Attempt 4: repository drift records the typed stale state.
    std::fs::write(
        fixture.root.join("DRIFT.md"),
        "drifted
",
    )
    .map_err(|error| format!("write drift file: {error}"))?;
    run_git(&fixture.root, &["add", "DRIFT.md"])?;
    run_git(&fixture.root, &["commit", "-qm", "drift"])?;
    let _ = run_apply(&fixture, &stale_id, Some(AUTHORITY))?;

    // Attempt 1 was never applied: it must remain prepared-but-not-applied.
    let state_of = |id: &str| -> Result<String, String> {
        let manifest = attempt_manifest(&fixture, id)?;
        manifest
            .get("state")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| format!("attempt {id} carries no state"))
    };
    let awaiting = state_of(&awaiting_id)?;
    let applied = state_of(&applied_id)?;
    let rejected = state_of(&rejected_id)?;
    let stale = state_of(&stale_id)?;
    if awaiting != "awaiting_edit" {
        return Err(format!("prepared-but-not-applied state is `{awaiting}`"));
    }
    if applied != "ready_to_finish" {
        return Err(format!("applied-but-unverified state is `{applied}`"));
    }
    if rejected != "failed" {
        return Err(format!("rejected state is `{rejected}`"));
    }
    if stale != "stale" {
        return Err(format!("stale state is `{stale}`"));
    }
    let distinct = [
        awaiting.as_str(),
        applied.as_str(),
        rejected.as_str(),
        stale.as_str(),
    ];
    let mut unique = distinct.to_vec();
    unique.sort();
    unique.dedup();
    if unique.len() != 4 {
        return Err(format!("the four durable states collapsed: {distinct:?}"));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Verification phase (#3570)
// ---------------------------------------------------------------------------

const RECEIPT_PATH: &str = "target/ripr/workflow/python-repair-driver-verification.json";

fn run_verify(
    fixture: &TempFixture,
    attempt_id: &str,
    authority: Option<&str>,
    rollback: bool,
) -> Result<Output, String> {
    let mut args: Vec<String> = vec![
        "agent".to_string(),
        "repair".to_string(),
        "--root".to_string(),
        ".".to_string(),
        "--attempt".to_string(),
        attempt_id.to_string(),
        "--phase".to_string(),
        "verify".to_string(),
    ];
    if let Some(authority) = authority {
        args.push("--verify-authorized".to_string());
        args.push("--verify-authority".to_string());
        args.push(authority.to_string());
    }
    if rollback {
        args.push("--verify-rollback".to_string());
    }
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    run_ripr(&fixture.root, &borrowed)
}

fn verification_receipt(fixture: &TempFixture) -> Result<Value, String> {
    let text = std::fs::read_to_string(fixture.root.join(RECEIPT_PATH))
        .map_err(|error| format!("read verification receipt: {error}"))?;
    parse_json(&text, "verification receipt")
}

/// Writes the accepted selection manifest naming one attempt whose native
/// identity matches the analyzer's exact seam spelling for the sample's
/// boundary predicate, so the verification movement join resolves.
fn write_verify_trust_manifest(
    root: &Path,
    attempt_id: &str,
    target_path: &str,
) -> Result<(), String> {
    write_verify_trust_manifest_named(
        root,
        attempt_id,
        target_path,
        "target/ripr/trust-manifest.json",
    )
}

fn write_verify_trust_manifest_named(
    root: &Path,
    attempt_id: &str,
    target_path: &str,
    manifest_path: &str,
) -> Result<(), String> {
    let head = current_head(root)?;
    let row = serde_json::json!({
        "attempt_id": attempt_id,
        "case_id": format!("case-{attempt_id}"),
        "subject_id": "fixture-subject",
        "repository": "https://example.com/fixture-subject",
        "base": "1111111111111111111111111111111111111111",
        "head": head,
        "selection_reason": "behavior changed in the diff and the case discriminates it",
        "diversity_stratum": "pytest_library",
        "family": "predicate_boundary",
        "owner": "src/lib.rs::price",
        "discriminator": "amount >= discount_threshold",
        "relation": "test calls owner directly",
        "oracle": "assert exact boundary value",
        "expected_direction": "should_gap",
        "claim_boundary": "static exposure evidence only",
        "target_path": target_path,
        "target_state": "existing",
        "selected_at": "2026-09-10T00:00:00Z",
        "selector": "campaign-selector",
        "authority_snapshot_digest": "2020202020202020202020202020202020202020202020202020202020202020",
    });
    let digest = canonical_row_digest(&row)?;
    let mut stored = row;
    if let Some(object) = stored.as_object_mut() {
        object.insert("selection_digest".to_string(), Value::String(digest));
    }
    let manifest = serde_json::json!({
        "schema_version": "0.1",
        "kind": "python_repair_trust_manifest",
        "spec": "RIPR-SPEC-0176",
        "description": "python repair verification fixture",
        "selections": [stored],
    });
    let text = serde_json::to_string_pretty(&manifest)
        .map_err(|error| format!("serialize fixture manifest: {error}"))?;
    let path = root.join(manifest_path);
    std::fs::write(&path, text).map_err(|error| format!("write {}: {error}", path.display()))
}

/// Drives prepare -> edit -> apply for one attempt and returns its durable
/// identity, so every verification test starts from the same applied state.
fn prepare_edit_apply(fixture: &TempFixture, label: &str) -> Result<String, String> {
    let seam_id = find_seam_for_target(&fixture.root, TARGET_TEST_FILE)?;
    write_verify_trust_manifest(&fixture.root, label, TARGET_TEST_FILE)?;
    let args: Vec<String> = vec![
        "agent".to_string(),
        "repair".to_string(),
        "--root".to_string(),
        ".".to_string(),
        "--seam-id".to_string(),
        seam_id,
        "--phase".to_string(),
        "before".to_string(),
        "--python-repair-trust-manifest".to_string(),
        "target/ripr/trust-manifest.json".to_string(),
        "--python-repair-trust-attempt".to_string(),
        label.to_string(),
        "--edit-authorized".to_string(),
        "--edit-authority".to_string(),
        AUTHORITY.to_string(),
    ];
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    let prepared = run_ripr(&fixture.root, &borrowed)?;
    require_success(&prepared, "bound before phase")?;
    let attempt_id = sole_attempt(fixture)?;
    edit_target_file(fixture)?;
    let applied = run_apply(fixture, &attempt_id, Some(AUTHORITY))?;
    require_success(&applied, "bound after phase")?;
    Ok(attempt_id)
}

#[test]
fn verification_phase_records_execution_and_movement_separately() -> Result<(), String> {
    let fixture = build_fixture("verify-clean")?;
    let attempt_id = prepare_edit_apply(&fixture, "att-verify")?;

    let verified = run_verify(&fixture, &attempt_id, Some(AUTHORITY), false)?;
    require_success(&verified, "verify phase")?;
    let receipt = verification_receipt(&fixture)?;

    if receipt.get("kind").and_then(Value::as_str) != Some("python_repair_verification_receipt") {
        return Err("the receipt kind is wrong".to_string());
    }
    if receipt.get("spec").and_then(Value::as_str) != Some("RIPR-SPEC-0176") {
        return Err("the receipt does not carry the RIPR-SPEC-0176 identity".to_string());
    }
    if receipt.get("durable_attempt_id").and_then(Value::as_str) != Some(attempt_id.as_str()) {
        return Err("the receipt does not name the durable attempt".to_string());
    }

    // Execution axis: this fixture's packet carries no producer-owned typed
    // verify route (the sample is a Rust seam, and verification routes are
    // producer-owned), so the run is typed unavailable with its reason — and
    // nothing else about the receipt changes. A run through a real route
    // would retain exit status plus stdout/stderr commitments in the same
    // block, independently of the movement block below.
    let execution = receipt
        .get("execution")
        .and_then(Value::as_object)
        .ok_or("the receipt carries no execution block")?;
    if execution.get("state").and_then(Value::as_str) != Some("unavailable") {
        return Err(format!(
            "a packet with no producer route must record the typed unavailable state: {execution:?}"
        ));
    }
    if execution
        .get("reason")
        .and_then(Value::as_str)
        .map(str::is_empty)
        .unwrap_or(true)
    {
        return Err("the unavailable execution must retain its reason".to_string());
    }

    // Movement axis: computed from the native-identity before/after join,
    // never from the run — an unavailable execution must NOT force the
    // movement to degrade, and here it stays a confident classification.
    let movement = receipt
        .get("movement")
        .and_then(Value::as_object)
        .ok_or("the receipt carries no movement block")?;
    let movement_state = movement
        .get("state")
        .and_then(Value::as_str)
        .ok_or("the movement block carries no state")?;
    // A trust-bound clean edit over the focused seam must produce a
    // confident, comparable movement: the join resolved exactly once in both
    // snapshots, so stale/uncertain/limited would mean the join failed.
    if !matches!(
        movement_state,
        "closed" | "improved" | "unchanged" | "regressed"
    ) {
        return Err(format!(
            "the movement join did not resolve confidently: {movement:?}"
        ));
    }
    let identity = movement
        .get("identity")
        .and_then(Value::as_object)
        .ok_or("the movement block carries no native identity")?;
    if identity.get("owner").and_then(Value::as_str) != Some("src/lib.rs::price")
        || identity.get("discriminator").and_then(Value::as_str)
            != Some("amount >= discount_threshold")
    {
        return Err(format!(
            "the movement identity does not restate the binding's native identity: {identity:?}"
        ));
    }

    // Separation: both axes ride as independent blocks with their own
    // evidence, and no lifecycle/acceptance field exists anywhere.
    for forbidden in [
        "states",
        "lifecycle",
        "accepted",
        "verified",
        "reviewed",
        "closure",
    ] {
        if receipt.get(forbidden).is_some() {
            return Err(format!(
                "the receipt carries the lifecycle field `{forbidden}`"
            ));
        }
    }
    let rollback = receipt
        .get("rollback")
        .and_then(Value::as_object)
        .ok_or("the receipt carries no rollback block")?;
    if rollback.get("state").and_then(Value::as_str) != Some("not_run") {
        return Err("an unrequested rollback must be recorded not_run".to_string());
    }
    let non_claims = receipt
        .get("non_claims")
        .and_then(Value::as_array)
        .ok_or("the receipt carries no non_claims")?;
    if non_claims.len() != 3 {
        return Err(format!(
            "the receipt carries {} non-claims, expected 3",
            non_claims.len()
        ));
    }

    // The verification run leaves no intermediate staging residue: the fresh
    // after snapshot is published atomically and its staging file is removed.
    let workflow = fixture.root.join("target/ripr/workflow");
    let mut residue = Vec::new();
    for entry in std::fs::read_dir(&workflow)
        .map_err(|error| format!("read {}: {error}", workflow.display()))?
    {
        let entry = entry.map_err(|error| format!("read workflow entry: {error}"))?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("after-verification.repo-exposure.json.tmp-") {
            residue.push(name);
        }
    }
    if !residue.is_empty() {
        return Err(format!(
            "the verification run left staging residue: {residue:?}"
        ));
    }
    Ok(())
}

#[test]
fn verification_without_or_with_wrong_authority_refused() -> Result<(), String> {
    let fixture = build_fixture("verify-auth")?;
    let attempt_id = prepare_edit_apply(&fixture, "att-verify-auth")?;

    // No verification authorization at all.
    let refused = run_verify(&fixture, &attempt_id, None, false)?;
    require_failure(
        &refused,
        "verify without authorization",
        "--verify-authorized",
    )?;
    if fixture.root.join(RECEIPT_PATH).exists() {
        return Err("an unauthorized verification wrote a receipt".to_string());
    }

    // The flag without the authority identity is refused at parse time.
    let args = vec![
        "agent".to_string(),
        "repair".to_string(),
        "--root".to_string(),
        ".".to_string(),
        "--attempt".to_string(),
        attempt_id.clone(),
        "--phase".to_string(),
        "verify".to_string(),
        "--verify-authorized".to_string(),
    ];
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    let refused = run_ripr(&fixture.root, &borrowed)?;
    require_failure(
        &refused,
        "verify with a flag but no authority",
        "--verify-authority",
    )?;

    // A different authority than the retained edit authorization is refused:
    // the verify invocation must re-affirm the retained authority.
    let refused = run_verify(&fixture, &attempt_id, Some("someone-else"), false)?;
    require_failure(
        &refused,
        "verify with the wrong authority",
        "does not match the retained authorization authority",
    )?;
    if fixture.root.join(RECEIPT_PATH).exists() {
        return Err("a refused verification wrote a receipt".to_string());
    }
    Ok(())
}

#[test]
fn verification_refuses_a_stale_tree() -> Result<(), String> {
    let fixture = build_fixture("verify-stale-tree")?;
    let attempt_id = prepare_edit_apply(&fixture, "att-verify-stale")?;

    // Move the repository HEAD after the apply: the exact post-edit tree is
    // gone, so the phase refuses before executing anything.
    std::fs::write(fixture.root.join("DRIFT.md"), "drifted\n")
        .map_err(|error| format!("write drift file: {error}"))?;
    run_git(&fixture.root, &["add", "DRIFT.md"])?;
    run_git(&fixture.root, &["commit", "-qm", "drift"])?;
    let refused = run_verify(&fixture, &attempt_id, Some(AUTHORITY), false)?;
    require_failure(&refused, "verify over a moved HEAD", "stale tree")?;
    if fixture.root.join(RECEIPT_PATH).exists() {
        return Err("a stale-tree verification wrote a receipt".to_string());
    }
    Ok(())
}

#[test]
fn verification_refuses_a_stale_command_packet() -> Result<(), String> {
    let fixture = build_fixture("verify-stale-packet")?;
    let attempt_id = prepare_edit_apply(&fixture, "att-verify-cmd")?;

    // Tamper the retained packet: its digest is pinned by the durable
    // attempt manifest, so the command identity check refuses before any run.
    let packet_path = fixture
        .root
        .join("target/ripr/repair-attempts")
        .join(attempt_id.as_str())
        .join("artifacts/agent-packet.json");
    let original = std::fs::read_to_string(&packet_path)
        .map_err(|error| format!("read retained packet: {error}"))?;
    std::fs::write(
        &packet_path,
        original.replacen('{', "{\n  \"smuggled\": true,", 1),
    )
    .map_err(|error| format!("write tampered packet: {error}"))?;
    let refused = run_verify(&fixture, &attempt_id, Some(AUTHORITY), false)?;
    require_failure(
        &refused,
        "verify over a tampered packet",
        "artifact binding failed",
    )?;
    if fixture.root.join(RECEIPT_PATH).exists() {
        return Err("a stale-command verification wrote a receipt".to_string());
    }
    Ok(())
}

#[test]
fn verification_receipt_is_immutable() -> Result<(), String> {
    let fixture = build_fixture("verify-immutable")?;
    let attempt_id = prepare_edit_apply(&fixture, "att-verify-immutable")?;
    let first = run_verify(&fixture, &attempt_id, Some(AUTHORITY), false)?;
    require_success(&first, "first verify phase")?;
    let second = run_verify(&fixture, &attempt_id, Some(AUTHORITY), false)?;
    require_failure(
        &second,
        "second verify phase over an existing receipt",
        "already exists",
    )?;
    Ok(())
}

#[test]
fn verification_rollback_restores_the_tree_and_records_the_proof() -> Result<(), String> {
    let fixture = build_fixture("verify-rollback")?;
    let attempt_id = prepare_edit_apply(&fixture, "att-verify-rollback")?;

    let verified = run_verify(&fixture, &attempt_id, Some(AUTHORITY), true)?;
    require_success(&verified, "verify phase with rollback")?;
    let receipt = verification_receipt(&fixture)?;
    let rollback = receipt
        .get("rollback")
        .and_then(Value::as_object)
        .ok_or("the receipt carries no rollback block")?;
    if rollback.get("state").and_then(Value::as_str) != Some("proved") {
        return Err(format!(
            "the rollback proof was not recorded as proved: {rollback:?}"
        ));
    }
    // The proof must land on the exact head the receipt pins: a proof at any
    // other head is not a rollback demonstration.
    let recorded_head = rollback
        .get("post_rollback_head")
        .and_then(Value::as_str)
        .ok_or("the rollback proof does not name the restored head")?;
    let receipt_head = receipt
        .get("repository_head")
        .and_then(Value::as_str)
        .ok_or("the receipt carries no repository head")?;
    if recorded_head != receipt_head {
        return Err(format!(
            "the rollback proof head {recorded_head} does not match the receipt's pinned head {receipt_head}"
        ));
    }
    if rollback
        .get("post_rollback_head")
        .and_then(Value::as_str)
        .map(str::len)
        != Some(40)
    {
        return Err("the rollback proof does not name the restored head".to_string());
    }
    // No tracked file may carry the applied edit anymore (the fixture's
    // target/ workflow directory is untracked and is not the edit).
    let status = Command::new("git")
        .current_dir(&fixture.root)
        .args(["status", "--porcelain", "--untracked-files=no"])
        .output()
        .map_err(|error| format!("spawn git status failed: {error}"))?;
    if !status.status.success() || !String::from_utf8_lossy(&status.stdout).trim().is_empty() {
        return Err(format!(
            "the rollback left worktree residue: {}",
            String::from_utf8_lossy(&status.stdout)
        ));
    }
    Ok(())
}

#[test]
fn verification_refuses_an_earlier_attempt_after_a_later_apply() -> Result<(), String> {
    let fixture = build_fixture("verify-latest-apply")?;
    // Both attempts are prepared on the same seam BEFORE any edit, so each
    // binds the same packet-selected target from the same clean tree. Each
    // attempt keeps its own manifest file: the retained binding pins the
    // manifest digest, so the file one binding points at must never change
    // under it.
    let seam_id = find_seam_for_target(&fixture.root, TARGET_TEST_FILE)?;
    for (label, manifest_path) in [
        ("att-a", "target/ripr/trust-manifest-a.json"),
        ("att-b", "target/ripr/trust-manifest-b.json"),
    ] {
        write_verify_trust_manifest_named(&fixture.root, label, TARGET_TEST_FILE, manifest_path)?;
        let prepared = run_ripr(
            &fixture.root,
            &prepare_args(&seam_id, manifest_path, label)
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
        )?;
        require_success(&prepared, &format!("bound before phase for {label}"))?;
    }
    let attempt_a = attempt_ids(&fixture)?
        .first()
        .cloned()
        .ok_or("attempt A was not published")?;
    let attempt_b = attempt_ids(&fixture)?
        .into_iter()
        .find(|id| id != &attempt_a)
        .ok_or("attempt B was not published")?;

    // Apply A, then apply B: each finish measures only its own edit against
    // its own baseline, and the global apply record now names B.
    edit_target_file(&fixture)?;
    let applied_a = run_apply(&fixture, &attempt_a, Some(AUTHORITY))?;
    require_success(&applied_a, "bound after phase for attempt A")?;
    edit_target_file(&fixture)?;
    let applied_b = run_apply(&fixture, &attempt_b, Some(AUTHORITY))?;
    require_success(&applied_b, "bound after phase for attempt B")?;

    // Verifying A refuses: the latest-apply projection no longer names A.
    // The refusal is the documented fail-closed boundary (the phase verifies
    // only the repository's latest applied attempt), and it never writes a
    // receipt for an attempt it could not revalidate.
    let refused = run_verify(&fixture, &attempt_a, Some(AUTHORITY), false)?;
    require_failure(
        &refused,
        "verify of an earlier attempt after a later apply",
        "the apply record names durable attempt",
    )?;
    if fixture.root.join(RECEIPT_PATH).exists() {
        return Err("a refused earlier-attempt verification wrote a receipt".to_string());
    }
    Ok(())
}
