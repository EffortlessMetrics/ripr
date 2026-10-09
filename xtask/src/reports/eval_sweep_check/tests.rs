use super::*;
use serde_json::json;
use std::collections::BTreeSet;

const VALID_SHA_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const VALID_SHA_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const VALID_SHA_C: &str = "cccccccccccccccccccccccccccccccccccccccc";
const VALID_SHA_D: &str = "dddddddddddddddddddddddddddddddddddddddd";
const VALID_SHA_E: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
const VALID_SHA_F: &str = "ffffffffffffffffffffffffffffffffffffffff";
const VALID_SHA_0: &str = "1010101010101010101010101010101010101010";
const VALID_SHA_1: &str = "1111111111111111111111111111111111111111";
const DIGEST_ONE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const DIGEST_TWO: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn subject_json(id: &str, sha: &str, shape: &str, license: &str) -> Value {
    json!({
        "id": id,
        "url": format!("https://example.com/{id}"),
        "sha": sha,
        "license": license,
        "shape": shape,
        "synthetic_diff": format!("fixtures/python-eval-sweep/diffs/{id}.diff"),
    })
}

/// An alternate valid eight-subject manifest: different ids, shas, shapes,
/// and licenses from the retained fixture — proves the validator is
/// data-driven, not fixture-byte-hardcoded.
fn alternate_manifest() -> Value {
    json!({
        "schema_version": "0.1",
        "kind": "python_eval_sweep_manifest",
        "spec": "RIPR-SPEC-0086",
        "tier": "A",
        "description": "alternate data-driven manifest",
        "repos": [
            subject_json("alpha", VALID_SHA_A, "pytest_library", "MIT"),
            subject_json("bravo", VALID_SHA_B, "unittest_library", "Apache-2.0"),
            subject_json("charlie", VALID_SHA_C, "click_typer", "BSD-3-Clause"),
            subject_json("delta", VALID_SHA_D, "pytest_library", "MIT"),
            subject_json("echo", VALID_SHA_E, "flask_web", "MIT"),
            subject_json("foxtrot", VALID_SHA_F, "fastapi_web", "MIT OR Apache-2.0"),
            subject_json("golf", VALID_SHA_0, "pytest_library", "MIT"),
            subject_json("hotel", VALID_SHA_1, "pytest_library", "BSD-2-Clause"),
        ]
    })
}

/// The alternate manifest with every optional identity field present, so
/// accepted validation discloses zero incompletes (the other pole of the
/// top-level verdict contract).
fn identity_complete_manifest() -> Value {
    let mut value = alternate_manifest();
    if let Some(repos) = value.get_mut("repos").and_then(Value::as_array_mut) {
        for repo in repos.iter_mut() {
            if let Some(entry) = repo.as_object_mut() {
                entry.insert("tree_digest".to_string(), json!(DIGEST_ONE));
                entry.insert("snapshot".to_string(), json!("snapshot-identities"));
                entry.insert("provenance".to_string(), json!("campaign-sweep"));
                entry.insert("retention_class".to_string(), json!("retained-evidence"));
            }
        }
    }
    value
}

fn parsed(value: &Value) -> Result<Value, String> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| format!("serialize test JSON: {error}"))?;
    parse_json_without_duplicate_keys(&text)
        .map_err(|error| format!("test JSON must parse: {error}"))
}

fn accepted_manifest(value: &Value) -> Result<(AcceptedManifest, String), String> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| format!("serialize test JSON: {error}"))?;
    let sha = sha256_hex(text.as_bytes());
    let value = parse_json_without_duplicate_keys(&text)
        .map_err(|error| format!("test JSON must parse: {error}"))?;
    let manifest = validate_accepted_manifest(&value, sha.clone())?;
    Ok((manifest, sha))
}

/// Asserts a fail-closed result: Err, mentioning `needle` and the rerun
/// command. Returns a test failure otherwise.
fn expect_fail(result: Result<(), String>, needle: &str) -> Result<(), String> {
    let error = match result {
        Ok(()) => {
            return Err(format!(
                "expected failure containing `{needle}`, got success"
            ));
        }
        Err(error) => error,
    };
    if !error.contains(needle) {
        return Err(format!("failure `{error}` must mention `{needle}`"));
    }
    if !error.contains(RERUN_COMMAND) {
        return Err(format!("failure `{error}` must carry the rerun command"));
    }
    Ok(())
}

/// Fail-closed with `needle`, and the generic envelope unknown-field
/// diagnostic must not be the selected message. Discriminates the
/// schema-0.2 mixed-shape checks from falling through to the generic loop.
fn expect_fail_without_generic_envelope(
    result: Result<(), String>,
    needle: &str,
) -> Result<(), String> {
    let error = match result {
        Ok(()) => {
            return Err(format!(
                "expected failure containing `{needle}`, got success"
            ));
        }
        Err(error) => error,
    };
    if !error.contains(needle) {
        return Err(format!("failure `{error}` must mention `{needle}`"));
    }
    if error.contains("unknown envelope field") {
        return Err(format!(
            "failure `{error}` must not use the generic unknown-field diagnostic"
        ));
    }
    if !error.contains(RERUN_COMMAND) {
        return Err(format!("failure `{error}` must carry the rerun command"));
    }
    Ok(())
}

fn validate_manifest_value(value: &Value) -> Result<(), String> {
    validate_accepted_manifest(value, String::new()).map(|_| ())
}

fn validate_receipt_value(
    receipt: &Value,
    manifest: &AcceptedManifest,
    manifest_sha: &str,
) -> Result<ReceiptCheck, String> {
    validate_run_receipt(receipt, manifest_sha, manifest, "receipt.json")
}

// -- data-driven acceptance -------------------------------------------

#[test]
fn accepts_alternate_valid_manifest_not_fixture_bytes() -> Result<(), String> {
    let (manifest, _) = accepted_manifest(&alternate_manifest())?;
    assert_eq!(manifest.subjects.len(), 8);
    assert!(manifest.subject("alpha").is_some());
    assert!(manifest.subject("hotel").is_some());
    assert!(manifest.subject("click").is_none());
    // All identities the retained fixture lacks are disclosed incomplete:
    // tree_digest, snapshot, provenance, retention_class per subject.
    assert_eq!(manifest.incomplete.len(), 8 * 4);
    Ok(())
}

#[test]
fn check_artifacts_passes_on_alternate_manifest_in_temp_dir() -> Result<(), String> {
    let dir = std::env::temp_dir().join(format!(
        "ripr-evalsweep-check-valid-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|error| format!("create dir: {error}"))?;
    let path = dir.join("manifest.json");
    let text =
        serde_json::to_string_pretty(&alternate_manifest()).map_err(|error| error.to_string())?;
    std::fs::write(&path, &text).map_err(|error| format!("write manifest: {error}"))?;

    let outcome = check_artifacts(path.to_string_lossy().as_ref(), None);
    let _ = std::fs::remove_dir_all(&dir);

    let outcome = outcome?;
    assert_eq!(outcome.accepted.subjects.len(), 8);
    assert_eq!(outcome.verdict(), Verdict::NotRun);
    Ok(())
}

// -- manifest failure shapes -------------------------------------------

#[test]
fn rejects_manifest_with_wrong_kind() -> Result<(), String> {
    let mut value = alternate_manifest();
    value["kind"] = json!("some_other_manifest");
    expect_fail(
        validate_manifest_value(&parsed(&value)?),
        "expected `python_eval_sweep_manifest`",
    )
}

#[test]
fn rejects_manifest_with_seven_subjects_changed_denominator() -> Result<(), String> {
    let mut value = alternate_manifest();
    if let Some(repos) = value["repos"].as_array_mut() {
        repos.pop();
    }
    expect_fail(
        validate_manifest_value(&parsed(&value)?),
        "exactly 8 selected subjects, got 7",
    )
}

/// Subject ids are safe identifiers: path-traversal and hidden/relative
/// ids fail with a named diagnostic, while the full safe charset (at the
/// length cap) passes. Existing valid ids (the fixture's and the
/// alternate manifest's) are covered by the acceptance tests and the
/// `eval-sweep check` gate over the canonical fixture.
#[test]
fn manifest_rejects_unsafe_subject_ids() -> Result<(), String> {
    let swapped = |id: &str| -> Result<Value, String> {
        let mut value = alternate_manifest();
        let entry = value
            .get_mut("repos")
            .and_then(Value::as_array_mut)
            .and_then(|repos| repos.first_mut())
            .and_then(Value::as_object_mut)
            .ok_or_else(|| "alternate manifest repos[0]".to_string())?;
        entry.insert("id".to_string(), json!(id));
        Ok(value)
    };
    for (bad, needle) in [
        ("../../outside", "must not start with"),
        ("..", "must not start with"),
        (".hidden", "must not start with"),
        ("a/b", "must use only"),
        ("a\\b", "must use only"),
        ("a b", "must use only"),
    ] {
        expect_fail(validate_manifest_value(&parsed(&swapped(bad)?)?), needle)?;
    }
    let long_id = "a".repeat(65);
    expect_fail(
        validate_manifest_value(&parsed(&swapped(&long_id)?)?),
        "at most 64 characters",
    )?;

    // The full safe charset at the exact length cap passes.
    let edge_ok = "b.x_y-9".repeat(9) + "c";
    assert_eq!(edge_ok.len(), 64, "edge id must sit at the cap");
    validate_manifest_value(&parsed(&swapped(&edge_ok)?)?)?;
    Ok(())
}

#[test]
fn rejects_duplicate_subject_ids() -> Result<(), String> {
    let mut value = alternate_manifest();
    if let Some(repos) = value["repos"].as_array_mut()
        && let Some(first) = repos.first().cloned()
    {
        repos[1] = first;
    }
    expect_fail(
        validate_manifest_value(&parsed(&value)?),
        "duplicate subject id",
    )
}

/// Subject ids are unique CASE-insensitively (ASCII): they become
/// filesystem path components, and on a case-insensitive filesystem
/// (Windows, default macOS) `Alpha` and `alpha` would resolve to one
/// candidate directory — two distinct subjects colliding into one tree.
/// The charset already bars `~` (short-name aliases) and separators; the
/// case alias is rejected at the duplicate check.
#[test]
fn rejects_subject_ids_differing_only_by_case() -> Result<(), String> {
    let mut value = alternate_manifest();
    let first_id = value["repos"][0]["id"]
        .as_str()
        .ok_or_else(|| "first subject id".to_string())?
        .to_string();
    let uppercase = first_id.to_ascii_uppercase();
    assert_ne!(
        first_id, uppercase,
        "the test needs an id with a distinct uppercase form"
    );
    value["repos"][1]["id"] = json!(uppercase);
    value["repos"][1]["url"] = json!(format!("https://example.com/{uppercase}"));
    expect_fail(
        validate_manifest_value(&parsed(&value)?),
        "duplicate subject id",
    )?;
    expect_fail(
        validate_manifest_value(&parsed(&value)?),
        "case-insensitively",
    )
}

#[test]
fn rejects_non_https_and_credential_urls() -> Result<(), String> {
    let mut value = alternate_manifest();
    value["repos"][0]["url"] = json!("http://example.com/alpha");
    expect_fail(validate_manifest_value(&parsed(&value)?), "must be https")?;

    let mut value = alternate_manifest();
    value["repos"][0]["url"] = json!("https://user:pass@example.com/alpha");
    expect_fail(validate_manifest_value(&parsed(&value)?), "credentials")
}

#[test]
fn rejects_absolute_and_secret_bearing_paths() -> Result<(), String> {
    let mut value = alternate_manifest();
    // The drive letter is assembled at runtime so this source file never
    // contains a local absolute path for the local-context gate.
    let drive_letter_absolute = format!("{}:/repo/fixtures/alpha.diff", 'C');
    value["repos"][0]["synthetic_diff"] = json!(drive_letter_absolute);
    expect_fail(
        validate_manifest_value(&parsed(&value)?),
        "drive-letter absolute path",
    )?;

    let mut value = alternate_manifest();
    value["repos"][0]["synthetic_diff"] = json!("configs/api_key/alpha.diff");
    expect_fail(
        validate_manifest_value(&parsed(&value)?),
        "secret-shaped token",
    )
}

#[test]
fn rejects_unknown_shape_tag_and_malformed_sha() -> Result<(), String> {
    let mut value = alternate_manifest();
    value["repos"][0]["shape"] = json!("pytest_monorepo");
    expect_fail(
        validate_manifest_value(&parsed(&value)?),
        "unknown shape/layout tag",
    )?;

    let mut value = alternate_manifest();
    value["repos"][0]["sha"] = json!("deadbeef");
    expect_fail(
        validate_manifest_value(&parsed(&value)?),
        "40-character commit SHA",
    )
}

#[test]
fn rejects_duplicate_json_keys_at_load() {
    let body = format!(
        "{{\"kind\": \"a\", \"kind\": \"b\", \"schema_version\": \"{MANIFEST_SCHEMA_VERSION}\", \"spec\": \"{KNOWN_SPEC}\", \"tier\": \"{KNOWN_TIER}\", \"repos\": []}}"
    );
    let parsed = parse_json_without_duplicate_keys(&body);
    assert!(parsed.is_err(), "duplicate keys must fail at load");
}

#[test]
fn manifest_rejects_unknown_top_level_and_repo_keys() -> Result<(), String> {
    let mut value = alternate_manifest();
    value["notes"] = json!("unexpected");
    expect_fail(
        validate_manifest_value(&parsed(&value)?),
        "unknown field `notes`",
    )?;

    let mut value = alternate_manifest();
    value["repos"][0]["coverage"] = json!(0.9);
    expect_fail(
        validate_manifest_value(&parsed(&value)?),
        "unknown field `coverage`",
    )
}

#[test]
fn canonical_fixture_manifest_passes_with_owned_keys_only() -> Result<(), String> {
    // The deny-unknown decision is only free if the canonical fixture
    // carries exactly the owned keys; run the real bytes end to end.
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fixtures/python-eval-sweep/manifest.json"
    );
    let outcome = check_artifacts(path, None)?;
    assert_eq!(outcome.accepted.subjects.len(), 8);
    assert_eq!(outcome.accepted.subjects[0].id, "click");
    assert_eq!(outcome.verdict(), Verdict::NotRun);
    Ok(())
}

// -- retained historical receipt (0.2) ---------------------------------

/// A historical Tier A receipt shape (schema 0.2) for the alternate
/// subjects, mirroring the retained #1160 receipt: 8 rows, 2 parse
/// failures, 1 timeout, gate pass with full stability. Rows: outcomes
/// index-aligned; runtimes 1000..8000 (min 1000, median 5000, max 8000,
/// total 36000).
fn historical_receipt_0_2(value_manifest: &Value) -> Value {
    let repos = value_manifest
        .get("repos")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let statuses = [
        "ok",
        "parse_failure",
        "ok",
        "ok",
        "ok",
        "parse_failure",
        "timed_out",
        "ok",
    ];
    let mut rows = Vec::new();
    for (index, repo) in repos.iter().enumerate() {
        let id = repo.get("id").and_then(Value::as_str).unwrap_or_default();
        let sha = repo.get("sha").and_then(Value::as_str).unwrap_or_default();
        let shape = repo
            .get("shape")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let outcome = statuses.get(index).copied().unwrap_or("ok");
        rows.push(json!({
            "id": id,
            "sha": sha,
            "shape": shape,
            "outcome": outcome,
            "runtime_ms": 1000 + index as u64 * 1000,
            "gap_ids": [],
            "gap_ids_stable": true,
            "unstable_gap_ids": [],
            "stderr_excerpt": "",
            "classification_counts": {
                "exposed": 0,
                "weakly_exposed": 0,
                "reachable_unrevealed": 0,
                "no_static_path": 0,
                "infection_unknown": 0,
                "propagation_unknown": 0,
                "static_unknown": if outcome == "parse_failure" { 1 } else { 0 },
            },
            "alignment_counts": {
                "direct": 0,
                "alias": 0,
                "changed_sink_token": 0,
                "orthogonal": 0,
                "unknown": 0,
                "absent": 0,
            },
        }));
    }
    json!({
        "schema_version": "0.2",
        "kind": "python_eval_sweep_report",
        "spec": KNOWN_SPEC,
        "tier": "A",
        "summary": {
            "repos_total": 8,
            "repos_run": 8,
            "repos_skipped": 0,
            "repos_clone_failed": 0,
            "crash_count": 0,
            "crash_rate": 0.0,
            "parse_failure_count": 2,
            "parse_failure_rate": 0.25,
            "timed_out_count": 1,
            "runtime_ms_min": 1000,
            "runtime_ms_median": 5000,
            "runtime_ms_max": 8000,
            "runtime_ms_total": 36000,
            "gap_id_stable_count": 8,
            "gap_id_unstable_count": 0,
            "gap_id_stability_rate": 1.0,
            "classification_counts": {
                "exposed": 0,
                "weakly_exposed": 0,
                "reachable_unrevealed": 0,
                "no_static_path": 0,
                "infection_unknown": 0,
                "propagation_unknown": 0,
                "static_unknown": 2,
            },
            "alignment_counts": {
                "direct": 0,
                "alias": 0,
                "changed_sink_token": 0,
                "orthogonal": 0,
                "unknown": 0,
                "absent": 0,
            },
            "gate_status": "pass",
            "gate_reason": "8 repo(s) analyzed; no crashes; canonical gap IDs stable across the re-run",
        },
        "repos": rows,
    })
}

#[test]
fn historical_receipt_validates_incomplete_without_rewrite() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
    let receipt = historical_receipt_0_2(&alternate_manifest());
    let check = validate_receipt_value(&receipt, &manifest, &sha)?;
    assert_eq!(check.schema_version, RECEIPT_SCHEMA_0_2);
    // Every row stays selected; all eight outcomes count as run.
    assert_eq!(check.denominator_selected, 8);
    assert_eq!(check.denominator_run, 8);
    // Historical rows disclose their missing currentness identities.
    assert!(
        !check.incomplete.is_empty(),
        "historical receipt must disclose incomplete identities"
    );
    assert!(check.incomplete.iter().any(|diagnostic| {
        diagnostic.subject == "alpha" && diagnostic.field == "currentness identities"
    }));
    assert!(
        check
            .incomplete
            .iter()
            .any(|diagnostic| diagnostic.field == "manifest_digest")
    );
    Ok(())
}

#[test]
fn schema_0_2_receipt_with_ripr_emits_specific_currentness_diagnostic() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
    let mut receipt = historical_receipt_0_2(&alternate_manifest());
    if let Some(object) = receipt.as_object_mut() {
        object.insert(
            "ripr".to_string(),
            json!({
                "version": "0.0.0",
                "features": [],
                "build_profile": "debug",
            }),
        );
    }
    expect_fail_without_generic_envelope(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "schema-0.2 receipts must not carry the 0.3 currentness block",
    )
}

#[test]
fn schema_0_2_receipt_with_manifest_digest_emits_specific_binding_diagnostic() -> Result<(), String>
{
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
    let mut receipt = historical_receipt_0_2(&alternate_manifest());
    if let Some(object) = receipt.as_object_mut() {
        object.insert("manifest_digest".to_string(), json!(DIGEST_ONE));
    }
    expect_fail_without_generic_envelope(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "schema-0.2 receipts must not carry the 0.3 manifest binding",
    )
}

#[test]
fn schema_0_2_receipt_with_unknown_envelope_key_still_uses_generic_diagnostic() -> Result<(), String>
{
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
    let mut receipt = historical_receipt_0_2(&alternate_manifest());
    if let Some(object) = receipt.as_object_mut() {
        object.insert("notes".to_string(), json!("unexpected"));
    }
    let error = match validate_receipt_value(&receipt, &manifest, &sha) {
        Ok(_) => {
            return Err(
                "expected 0.2 receipt with unknown envelope key `notes` to fail".to_string(),
            );
        }
        Err(error) => error,
    };
    if !error.contains("unknown envelope field `notes` for receipt schema 0.2") {
        return Err(format!(
            "failure `{error}` must use the generic unknown-field diagnostic for `notes`"
        ));
    }
    if error.contains("schema-0.2 receipts must not carry") {
        return Err(format!(
            "failure `{error}` must not steal a mixed-shape diagnostic for an unrelated key"
        ));
    }
    if !error.contains(RERUN_COMMAND) {
        return Err(format!("failure `{error}` must carry the rerun command"));
    }
    Ok(())
}

#[test]
fn historical_receipt_keeps_failed_rows_selected() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
    let mut receipt = historical_receipt_0_2(&alternate_manifest());
    // Flip one row to each non-run outcome; both must remain selected.
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut) {
        if let Some(entry) = rows[0].as_object_mut() {
            entry.insert("outcome".to_string(), json!("clone_failed"));
        }
        if let Some(entry) = rows[7].as_object_mut() {
            entry.insert("outcome".to_string(), json!("skipped_missing_checkout"));
        }
    }
    // Re-derive the hand-entered aggregates honestly (run = 6).
    if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
        summary.insert("repos_run".to_string(), json!(6));
        summary.insert("repos_clone_failed".to_string(), json!(1));
        summary.insert("repos_skipped".to_string(), json!(1));
        summary.insert("gap_id_stable_count".to_string(), json!(6));
        summary.insert("runtime_ms_min".to_string(), json!(2000));
        summary.insert("runtime_ms_median".to_string(), json!(5000));
        summary.insert("runtime_ms_max".to_string(), json!(7000));
        summary.insert("runtime_ms_total".to_string(), json!(27000));
        summary.insert("parse_failure_rate".to_string(), json!(0.3333333333333333));
    }
    let check = validate_receipt_value(&receipt, &manifest, &sha)?;
    assert_eq!(check.denominator_selected, 8);
    assert_eq!(check.denominator_run, 6);
    Ok(())
}

#[test]
fn receipt_rejects_unknown_subject_and_missing_subject() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

    let mut receipt = historical_receipt_0_2(&alternate_manifest());
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[0].as_object_mut()
    {
        entry.insert("id".to_string(), json!("outsider"));
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "outside the accepted denominator",
    )?;

    let mut receipt = historical_receipt_0_2(&alternate_manifest());
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut) {
        rows.pop();
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "changed denominator",
    )
}

#[test]
fn receipt_rejects_duplicate_rows() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
    let mut receipt = historical_receipt_0_2(&alternate_manifest());
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(first) = rows.first().cloned()
    {
        rows[1] = first;
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "duplicate subject row",
    )
}

#[test]
fn receipt_rejects_unknown_outcome_state() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
    let mut receipt = historical_receipt_0_2(&alternate_manifest());
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[0].as_object_mut()
    {
        entry.insert("outcome".to_string(), json!("mostly_fine"));
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "unknown run outcome",
    )
}

#[test]
fn receipt_rejects_identity_contradiction_with_manifest_pin() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
    let mut receipt = historical_receipt_0_2(&alternate_manifest());
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[0].as_object_mut()
    {
        entry.insert(
            "sha".to_string(),
            json!("9999999999999999999999999999999999999999"),
        );
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "does not match the accepted manifest pin",
    )
}

#[test]
fn receipt_rejects_stale_manifest_digest() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
    let mut receipt = current_receipt_0_3(&manifest);
    if let Some(object) = receipt.as_object_mut() {
        object.insert("manifest_digest".to_string(), json!(DIGEST_ONE));
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "stale digest",
    )
}

#[test]
fn gate_status_must_equal_the_derived_gate() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

    // Analyzed, crash-free, fully stable rows derive `pass`: a claimed
    // `not_run` fails even though it is in the vocabulary.
    let mut receipt = historical_receipt_0_2(&alternate_manifest());
    if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
        summary.insert("gate_status".to_string(), json!("not_run"));
        summary.insert("gate_reason".to_string(), json!("unwarranted"));
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "does not equal the gate derived from the rows",
    )?;

    // The same rows with an unwarranted `review` fail too.
    let mut receipt = historical_receipt_0_2(&alternate_manifest());
    if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
        summary.insert("gate_status".to_string(), json!("review"));
        summary.insert("gate_reason".to_string(), json!("unwarranted"));
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "does not equal the gate derived from the rows",
    )
}

#[test]
fn analyzed_row_missing_distribution_fails_and_rejects_summary_totals() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
    let mut receipt = current_receipt_0_3(&manifest);
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[0].as_object_mut()
    {
        entry.remove("classification_counts");
    }
    // Arbitrary summary totals cannot rescue a row that omits its
    // aggregate source evidence.
    if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
        summary.insert(
                "classification_counts".to_string(),
                json!({"exposed": 99, "weakly_exposed": 0, "reachable_unrevealed": 0, "no_static_path": 0, "infection_unknown": 0, "propagation_unknown": 0, "static_unknown": 0}),
            );
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "omits its classification distribution",
    )
}

#[test]
fn analyzed_row_missing_runtime_fails() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
    let mut receipt = current_receipt_0_3(&manifest);
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[0].as_object_mut()
    {
        entry.remove("runtime_ms");
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "omits its runtime",
    )
}

#[test]
fn under_evidenced_stability_rejects_recorded_summary_and_discloses_absent() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

    // 0.3 stability evidence lives in the optional `repeat` block: a run
    // row without it disables the stability aggregate, so a recorded
    // summary stability value fails (it cannot be derived).
    let mut receipt = current_receipt_0_3(&manifest);
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[0].as_object_mut()
        && let Some(Value::Object(repeat)) = entry.get_mut("repeat")
    {
        repeat.remove("gap_ids_stable");
    }
    if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
        summary.insert("gap_id_stable_count".to_string(), json!(5));
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "stability aggregate recorded but analyzed rows lack complete stability evidence",
    )?;

    // Without recorded values the gap is disclosed incomplete, not
    // invented and not failed: over rows lacking `repeat` evidence the
    // emitter could not have written the stability aggregates, so they
    // are omitted here and the absence is disclosed.
    let mut receipt = current_receipt_0_3(&manifest);
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[0].as_object_mut()
        && let Some(Value::Object(repeat)) = entry.get_mut("repeat")
    {
        repeat.remove("gap_ids_stable");
    }
    if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
        summary.remove("gap_id_stable_count");
        summary.remove("gap_id_unstable_count");
        summary.remove("gap_id_stability_rate");
    }
    let check = validate_receipt_value(&receipt, &manifest, &sha)?;
    assert!(
        check
            .incomplete
            .iter()
            .any(|diagnostic| diagnostic.field == "summary.gap_id_stable_count"),
        "under-evidenced stability must be disclosed incomplete: {:?}",
        check.incomplete
    );
    Ok(())
}

#[test]
fn summary_distribution_extra_bucket_fails() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

    // An extra classification bucket cannot hide behind derived-key
    // iteration.
    let mut receipt = historical_receipt_0_2(&alternate_manifest());
    if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut)
        && let Some(Value::Object(counts)) = summary.get_mut("classification_counts")
    {
        counts.insert("flawlessly_verified".to_string(), json!(1));
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "never establish",
    )?;

    // An extra alignment bucket fails the same way.
    let mut receipt = historical_receipt_0_2(&alternate_manifest());
    if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut)
        && let Some(Value::Object(counts)) = summary.get_mut("alignment_counts")
    {
        counts.insert("repair_placement_present".to_string(), json!(1));
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "never establish",
    )
}

#[test]
fn zero_valued_buckets_participate_in_exact_agreement() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

    // Zero-valued buckets are part of the emitted shape (the sweep writes
    // every bucket), so a missing zero bucket still fails.
    let mut receipt = historical_receipt_0_2(&alternate_manifest());
    if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut)
        && let Some(Value::Object(counts)) = summary.get_mut("alignment_counts")
    {
        counts.remove("direct");
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "but the summary omits it",
    )?;

    // The full zero-filled bucket set agrees.
    let receipt = historical_receipt_0_2(&alternate_manifest());
    validate_receipt_value(&receipt, &manifest, &sha)?;
    Ok(())
}

#[test]
fn runtime_total_overflow_fails_structurally_without_panic() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
    let mut receipt = historical_receipt_0_2(&alternate_manifest());
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut) {
        for row in rows.iter_mut().take(2) {
            if let Some(entry) = row.as_object_mut() {
                entry.insert("runtime_ms".to_string(), json!(u64::MAX));
            }
        }
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "summary.runtime_ms_total",
    )?;
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "aggregate overflow",
    )
}

#[test]
fn distribution_merge_overflow_fails_structurally_without_panic() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
    let mut receipt = historical_receipt_0_2(&alternate_manifest());
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut) {
        for row in rows.iter_mut().take(2) {
            if let Some(entry) = row.as_object_mut()
                && let Some(Value::Object(counts)) = entry.get_mut("classification_counts")
            {
                counts.insert("static_unknown".to_string(), json!(u64::MAX));
            }
        }
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "summary.classification_counts.static_unknown",
    )?;
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "aggregate overflow",
    )
}

#[test]
fn receipt_rejects_hand_edited_aggregates() -> Result<(), String> {
    for (field, wrong) in [
        ("repos_total", json!(9)),
        ("repos_run", json!(7)),
        ("crash_count", json!(3)),
        ("parse_failure_count", json!(5)),
        ("gap_id_stable_count", json!(4)),
        ("runtime_ms_total", json!(99)),
        ("gap_id_stability_rate", json!(0.5)),
    ] {
        let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
        let mut receipt = historical_receipt_0_2(&alternate_manifest());
        if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
            summary.insert(field.to_string(), wrong);
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "hand-edited aggregate",
        )?;
    }
    Ok(())
}

/// A zero-run receipt: every row is a skipped non-run row with zero
/// analysis counts, and the hand-entered aggregates are honestly all-zero
/// with the not_run gate. The stability rate is 0.0 here; the live
/// emitter's own zero-run shape records the vacuous 1.0 instead, which
/// validates as a named incomplete disclosure (see
/// `emitter_shaped_zero_run_stability_rate_discloses_instead_of_failing`).
fn zero_run_receipt_0_2(value_manifest: &Value) -> Value {
    let mut receipt = historical_receipt_0_2(value_manifest);
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut) {
        for row in rows.iter_mut() {
            if let Some(entry) = row.as_object_mut() {
                entry.insert("outcome".to_string(), json!("skipped_missing_checkout"));
                entry.insert(
                        "classification_counts".to_string(),
                        json!({"exposed": 0, "weakly_exposed": 0, "reachable_unrevealed": 0, "no_static_path": 0, "infection_unknown": 0, "propagation_unknown": 0, "static_unknown": 0}),
                    );
                entry.insert(
                        "alignment_counts".to_string(),
                        json!({"direct": 0, "alias": 0, "changed_sink_token": 0, "orthogonal": 0, "unknown": 0, "absent": 0}),
                    );
            }
        }
    }
    if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
        summary.insert("repos_run".to_string(), json!(0));
        summary.insert("repos_skipped".to_string(), json!(8));
        summary.insert("gap_id_stable_count".to_string(), json!(0));
        summary.insert("gap_id_stability_rate".to_string(), json!(0.0));
        summary.insert("crash_rate".to_string(), json!(0.0));
        summary.insert("parse_failure_count".to_string(), json!(0));
        summary.insert("parse_failure_rate".to_string(), json!(0.0));
        summary.insert("timed_out_count".to_string(), json!(0));
        summary.insert("runtime_ms_min".to_string(), json!(0));
        summary.insert("runtime_ms_median".to_string(), json!(0));
        summary.insert("runtime_ms_max".to_string(), json!(0));
        summary.insert("runtime_ms_total".to_string(), json!(0));
        summary.insert(
                "classification_counts".to_string(),
                json!({"exposed": 0, "weakly_exposed": 0, "reachable_unrevealed": 0, "no_static_path": 0, "infection_unknown": 0, "propagation_unknown": 0, "static_unknown": 0}),
            );
        // The emitter zero-fills every alignment bucket, at zero runs
        // included (eval_sweep.rs `to_json`), so the recorded summary
        // carries the full nine-key set — `absent`/`unknown` and the
        // three repair-packet counters included.
        summary.insert(
                "alignment_counts".to_string(),
                json!({"direct": 0, "alias": 0, "changed_sink_token": 0, "orthogonal": 0, "unknown": 0, "absent": 0, "repair_placement_present": 0, "verify_command_present": 0, "python_repair_card_present": 0}),
            );
        summary.insert("gate_status".to_string(), json!("not_run"));
        summary.insert("gate_reason".to_string(), json!("no repos analyzed"));
    }
    receipt
}

#[test]
fn receipt_rejects_vacuous_pass_and_accepts_not_run() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
    // A zero-run receipt claiming pass fails: never a vacuous pass.
    let mut receipt = zero_run_receipt_0_2(&alternate_manifest());
    if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
        summary.insert("gate_status".to_string(), json!("pass"));
        summary.insert("gate_reason".to_string(), json!("nothing ran"));
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "repos_run == 0 is `not_run`, never `pass`",
    )?;

    // The same zero-run receipt with the honest not_run gate validates.
    let receipt = zero_run_receipt_0_2(&alternate_manifest());
    let check = validate_receipt_value(&receipt, &manifest, &sha)?;
    assert_eq!(check.denominator_selected, 8);
    assert_eq!(check.denominator_run, 0);
    Ok(())
}

#[test]
fn zero_run_receipt_rejects_fabricated_summary_aggregates() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

    // A nonzero classification bucket claims analysis that never ran.
    let mut receipt = zero_run_receipt_0_2(&alternate_manifest());
    if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut)
        && let Some(Value::Object(counts)) = summary.get_mut("classification_counts")
    {
        counts.insert("static_unknown".to_string(), json!(5));
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "summary.classification_counts.static_unknown",
    )?;

    // The same zero-run law bounds the runtime aggregates and the
    // stability counts — any nonzero value is fabricated. For the
    // stability rate only values OTHER than the emitter's vacuous
    // zero-run 1.0 fail; exactly 1.0 discloses incomplete instead (the
    // emitter-shaped receipt is pinned separately below).
    for (field, value) in [
        ("runtime_ms_total", json!(1234)),
        ("gap_id_stable_count", json!(3)),
        ("gap_id_stability_rate", json!(0.5)),
    ] {
        let mut receipt = zero_run_receipt_0_2(&alternate_manifest());
        if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
            summary.insert(field.to_string(), value);
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            &format!("summary.{field}"),
        )?;
    }

    // The all-zero summary validates (absent would too).
    let receipt = zero_run_receipt_0_2(&alternate_manifest());
    validate_receipt_value(&receipt, &manifest, &sha)?;
    Ok(())
}

#[test]
fn emitter_shaped_zero_run_stability_rate_discloses_instead_of_failing() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

    // A fresh zero-run receipt straight from the live emitter records
    // `gap_id_stability_rate: 1.0` (eval_sweep.rs `compute_metrics`
    // empty-set guard), so a real untouched `eval-sweep` report must not
    // fail check. The rate validates — as a named incomplete disclosure
    // (`vacuous zero-run stability rate`), which keeps the receipt
    // verdict `incomplete`, never a pass.
    let mut receipt = zero_run_receipt_0_2(&alternate_manifest());
    if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
        summary.insert("gap_id_stability_rate".to_string(), json!(1.0));
    }
    let check = validate_receipt_value(&receipt, &manifest, &sha)?;
    assert_eq!(check.denominator_run, 0);
    assert!(
        check.incomplete.iter().any(|diagnostic| {
            diagnostic.field == "summary.gap_id_stability_rate"
                && diagnostic
                    .reason
                    .contains("vacuous zero-run stability rate")
        }),
        "the emitter's zero-run 1.0 rate must be disclosed vacuous, not failed: {:?}",
        check.incomplete
    );
    assert_eq!(check.verdict(), Verdict::Incomplete);

    // Any OTHER nonzero rate at zero runs is still a fabricated claim
    // about rows that never ran.
    let mut receipt = zero_run_receipt_0_2(&alternate_manifest());
    if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
        summary.insert("gap_id_stability_rate".to_string(), json!(0.5));
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "summary.gap_id_stability_rate",
    )
}

#[test]
fn receipt_rejects_pass_claim_without_stability_evidence() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

    // A missing 0.2 stability field on an analyzed row fails at the row
    // level (H4: aggregate source evidence is required there).
    let mut receipt = historical_receipt_0_2(&alternate_manifest());
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[0].as_object_mut()
    {
        entry.remove("gap_ids_stable");
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "omits its gap-ID stability evidence",
    )?;

    // With evidence present but not all-stable (stable=false plus the
    // unstable IDs the comparison produced), a claimed `pass` still fails
    // at the gate. The stability aggregates are re-derived honestly so
    // the gate equality is what fails.
    let mut receipt = historical_receipt_0_2(&alternate_manifest());
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[0].as_object_mut()
    {
        entry.insert("gap_ids_stable".to_string(), json!(false));
        entry.insert("unstable_gap_ids".to_string(), json!(["gap:python:x"]));
    }
    if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
        summary.insert("gap_id_stable_count".to_string(), json!(7));
        summary.insert("gap_id_unstable_count".to_string(), json!(1));
        summary.insert("gap_id_stability_rate".to_string(), json!(0.875));
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "`pass` claimed without full per-row stability evidence",
    )
}

#[test]
fn receipt_rejects_contradictory_stability_claims() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
    let mut receipt = historical_receipt_0_2(&alternate_manifest());
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[0].as_object_mut()
    {
        entry.insert("unstable_gap_ids".to_string(), json!(["gap:python:x"]));
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "contradictory status",
    )
}

#[test]
fn receipt_keeps_absent_distinct_from_unknown_distributions() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

    // A recorded distribution missing the `absent` key fails.
    let mut receipt = historical_receipt_0_2(&alternate_manifest());
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[0].as_object_mut()
        && let Some(Value::Object(align)) = entry.get_mut("alignment_counts")
    {
        align.remove("absent");
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "keep `absent` (field not emitted) distinct from `unknown` (emitted value)",
    )?;

    // A distribution with distinct absent and unknown populations validates.
    let mut receipt = historical_receipt_0_2(&alternate_manifest());
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[0].as_object_mut()
    {
        entry.insert(
                    "alignment_counts".to_string(),
                    json!({"direct": 1, "alias": 0, "changed_sink_token": 0, "orthogonal": 2, "unknown": 3, "absent": 0}),
                );
        entry.insert(
                    "classification_counts".to_string(),
                    json!({"exposed": 0, "weakly_exposed": 1, "reachable_unrevealed": 0, "no_static_path": 2, "infection_unknown": 0, "propagation_unknown": 0, "static_unknown": 0}),
                );
    }
    if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
        summary.insert(
                "alignment_counts".to_string(),
                json!({"direct": 1, "alias": 0, "changed_sink_token": 0, "orthogonal": 2, "unknown": 3, "absent": 0}),
            );
        summary.insert(
                "classification_counts".to_string(),
                json!({"exposed": 0, "weakly_exposed": 1, "reachable_unrevealed": 0, "no_static_path": 2, "infection_unknown": 0, "propagation_unknown": 0, "static_unknown": 2}),
            );
    }
    validate_receipt_value(&receipt, &manifest, &sha)?;
    Ok(())
}

#[test]
fn receipt_rejects_unknown_classification_vocabulary() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
    let mut receipt = historical_receipt_0_2(&alternate_manifest());
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[0].as_object_mut()
    {
        // Any value outside the conservative 7-class vocabulary fails —
        // here a made-up strong-claim word, which is exactly the family
        // the language rules ban from real outputs.
        entry.insert(
            "classification_counts".to_string(),
            json!({"flawlessly_verified": 1}),
        );
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "unknown classification",
    )
}

// -- currentness receipt (0.3) ------------------------------------------

/// A fully-current 0.3 receipt for the given manifest: every status in
/// the eight-word vocabulary appears once; failed/unavailable rows stay
/// selected and only the five analysis-attempting statuses count as run.
/// Row identities restate the manifest's own values when it pins them
/// (the receipt binds against the manifest side), and the summary carries
/// the full emitted aggregate set the sweep writes (#3733 review) minus
/// the stability aggregates: the builder attaches the `repeat` block only
/// to `complete` rows — the producer shape, since a stability pass only
/// runs where the first result is complete — so stability is under-
/// evidenced whenever any run row is non-complete and the aggregates are
/// absent (typed incomplete, never derived over partial evidence).
fn current_receipt_0_3(manifest: &AcceptedManifest) -> Value {
    let statuses = [
        "complete",
        "partial",
        "parse-failed",
        "timed-out",
        "crashed",
        "unsupported",
        "tempfail",
        "stale",
    ];
    let executions = [
        "executed",
        "executed",
        "executed",
        "timed-out",
        "failed",
        "not-executed",
        "not-executed",
        "unknown",
    ];
    let mut rows = Vec::new();
    for (index, subject) in manifest.subjects.iter().enumerate() {
        let status = statuses[index];
        let execution = executions[index];
        let ran = matches!(
            status,
            "complete" | "partial" | "parse-failed" | "timed-out" | "crashed"
        );
        let tree = subject
            .tree_digest
            .clone()
            .unwrap_or_else(|| DIGEST_ONE.to_string());
        let snapshot = subject
            .snapshot
            .clone()
            .unwrap_or_else(|| format!("snapshot-{}", subject.id));
        let provenance = subject
            .provenance
            .clone()
            .unwrap_or_else(|| "campaign-sweep".to_string());
        let retention = subject
            .retention_class
            .clone()
            .unwrap_or_else(|| "retained-evidence".to_string());
        let mut row = json!({
            "id": subject.id,
            "status": status,
            "repository": {
                "url": subject.url,
                "sha": subject.sha,
                "tree_digest": tree,
            },
            "tree_digest": tree,
            "snapshot": snapshot,
            "license": subject.license,
            "retention_class": retention,
            "provenance": provenance,
            "selected_root": format!("target/ripr/eval-sweep/checkouts/{}", subject.id),
            "layout": ["pytest_library"],
            "binary": {
                "digest": DIGEST_TWO,
                "version": "ripr 0.11.0",
                "features": ["python"],
                "build_profile": "debug",
            },
            "config": {
                "profile": "ripr-default",
                "input": "ripr.toml.example",
            },
            "input_digest": DIGEST_ONE,
            "materialization": if ran { "materialized" } else { "absent" },
            "detection": if ran { "detected" } else { "absent" },
            "corpus_selection": {
                "state": if ran { "selected" } else { "absent" },
                "source_files": 10 + index as u64,
                "test_files": 5 + index as u64,
                "generated_files": 0,
                "vendor_files": 0,
            },
            "execution": execution,
            "digests": {
                "raw": DIGEST_ONE,
                "output": DIGEST_TWO,
                "evidence": DIGEST_ONE,
            },
            "runtime_ms": 100 * (index as u64 + 1),
            "classification_counts": {
                "exposed": 0,
                "weakly_exposed": if status == "complete" { 1 } else { 0 },
                "reachable_unrevealed": 0,
                "no_static_path": 0,
                "infection_unknown": 0,
                "propagation_unknown": 0,
                "static_unknown": if status == "parse-failed" { 1 } else { 0 },
            },
            "alignment_counts": {
                "direct": 0,
                "alias": 0,
                "changed_sink_token": 0,
                "orthogonal": if status == "complete" { 1 } else { 0 },
                "unknown": if status == "partial" { 1 } else { 0 },
                "absent": if matches!(status, "parse-failed" | "crashed" | "timed-out") { 1 } else { 0 },
            },
        });
        // Non-run rows carry no analysis counts (nothing was analyzed).
        if !ran && let Some(entry) = row.as_object_mut() {
            entry.insert(
                        "classification_counts".to_string(),
                        json!({"exposed": 0, "weakly_exposed": 0, "reachable_unrevealed": 0, "no_static_path": 0, "infection_unknown": 0, "propagation_unknown": 0, "static_unknown": 0}),
                    );
            entry.insert(
                        "alignment_counts".to_string(),
                        json!({"direct": 0, "alias": 0, "changed_sink_token": 0, "orthogonal": 0, "unknown": 0, "absent": 0}),
                    );
        }
        // The `repeat` block exists only where the stability pass actually
        // compared — a `complete` first result. A compared non-complete
        // row would claim stability over no comparison (equal failure gap
        // sets must never read as `stable`), so the builder matches the
        // producer: non-complete rows omit the block and disclose the
        // absent comparison as typed incomplete.
        if status == "complete"
            && let Some(entry) = row.as_object_mut()
        {
            entry.insert(
                "repeat".to_string(),
                json!({
                    "comparable_with": format!("{}#run-1", subject.id),
                    "gap_ids_stable": true,
                    "unstable_gap_ids": [],
                }),
            );
        }
        rows.push(row);
    }
    // Derived aggregates over the five run rows (complete/partial/
    // parse-failed/timed-out/crashed): one crash, one parse failure, one
    // timeout, one tempfail; runtimes 100..500 (min 100, median 300, max
    // 500, total 1500); classification weakly_exposed=1 + static_unknown=1;
    // alignment orthogonal=1, unknown=1, absent=3. Only the complete row
    // carries `repeat` evidence, so the stability aggregates are absent —
    // the under-evidenced path the validator discloses, never a value
    // derived over partial evidence. The rest of the emitted summary is
    // present, exactly as the sweep writes it (#3733 review).
    json!({
        "schema_version": "0.3",
        "kind": "python_eval_sweep_report",
        "spec": KNOWN_SPEC,
        "tier": "A",
        "manifest_digest": manifest.sha256,
        "ripr": {
            "source_sha": VALID_SHA_A,
            "tree_digest": DIGEST_ONE,
            "binary_digest": DIGEST_TWO,
            "version": "ripr 0.11.0",
            "features": ["python"],
            "build_profile": "debug",
        },
        "summary": {
            "repos_total": 8,
            "repos_run": 5,
            "repos_skipped": 0,
            "repos_clone_failed": 1,
            "crash_count": 1,
            "crash_rate": 0.2,
            "parse_failure_count": 1,
            "parse_failure_rate": 0.2,
            "timed_out_count": 1,
            "runtime_ms_min": 100,
            "runtime_ms_median": 300,
            "runtime_ms_max": 500,
            "runtime_ms_total": 1500,
            "classification_counts": {
                "exposed": 0,
                "weakly_exposed": 1,
                "reachable_unrevealed": 0,
                "no_static_path": 0,
                "infection_unknown": 0,
                "propagation_unknown": 0,
                "static_unknown": 1,
            },
            "alignment_counts": {
                "direct": 0,
                "alias": 0,
                "changed_sink_token": 0,
                "orthogonal": 1,
                "unknown": 1,
                "absent": 3,
            },
            "gate_status": "review",
            "gate_reason": "1 crash over 5 run rows",
        },
        "repos": rows,
    })
}

#[test]
fn current_receipt_all_eight_statuses_validate_and_stay_selected() -> Result<(), String> {
    // The identity-complete manifest pins every optional identity, so the
    // receipt's restated identities bind cleanly. The builder attaches
    // `repeat` only to the `complete` row (the producer shape), so the
    // seven other rows disclose the absent comparison as typed incomplete,
    // and the under-evidenced summary stability aggregate is disclosed at
    // the receipt level — nothing else is incomplete.
    let complete = identity_complete_manifest();
    let (manifest, sha) = accepted_manifest(&complete)?;
    let receipt = current_receipt_0_3(&manifest);
    let check = validate_receipt_value(&receipt, &manifest, &sha)?;
    assert_eq!(check.schema_version, RECEIPT_SCHEMA_0_3);
    // Every row stays selected; only five attempted analysis.
    assert_eq!(check.denominator_selected, 8);
    assert_eq!(check.denominator_run, 5);
    assert_eq!(
        check
            .incomplete
            .iter()
            .filter(|diagnostic| diagnostic.field != "repeat"
                && diagnostic.field != "summary.gap_id_stable_count")
            .count(),
        0,
        "only the absent repeat comparisons and the under-evidenced stability aggregate are disclosed: {:?}",
        check.incomplete
    );
    assert_eq!(
        check.incomplete.len(),
        8,
        "one repeat disclosure per non-complete row plus the summary disclosure: {:?}",
        check.incomplete
    );
    Ok(())
}

#[test]
fn current_receipt_rejects_unknown_status_and_states() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
    for (field, value, needle) in [
        ("status", json!("mostly_worked"), "unknown run status"),
        (
            "materialization",
            json!("teleported"),
            "unknown materialization state",
        ),
        ("detection", json!("guessed"), "unknown detection state"),
        ("execution", json!("vibes"), "unknown execution state"),
    ] {
        let mut receipt = current_receipt_0_3(&manifest);
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
        {
            entry.insert(field.to_string(), value);
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            needle,
        )?;
    }
    Ok(())
}

#[test]
fn current_receipt_rejects_contradictory_status_pairs() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
    for (field, value) in [
        ("execution", json!("timed-out")),
        ("materialization", json!("absent")),
        ("detection", json!("failed")),
    ] {
        let mut receipt = current_receipt_0_3(&manifest);
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
        {
            entry.insert(field.to_string(), value);
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            "contradictory status",
        )?;
    }
    Ok(())
}

#[test]
fn current_receipt_rejects_nested_stability_contradictions_in_both_directions() -> Result<(), String>
{
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

    // Direction 1: claims stable while listing unstable IDs in `repeat`.
    let mut receipt = current_receipt_0_3(&manifest);
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[0].as_object_mut()
        && let Some(Value::Object(repeat)) = entry.get_mut("repeat")
    {
        repeat.insert("unstable_gap_ids".to_string(), json!(["gap:python:x"]));
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "claims stable gap IDs while listing unstable ones",
    )?;

    // Direction 2: claims unstable while listing no unstable IDs.
    let mut receipt = current_receipt_0_3(&manifest);
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[0].as_object_mut()
        && let Some(Value::Object(repeat)) = entry.get_mut("repeat")
    {
        repeat.insert("gap_ids_stable".to_string(), json!(false));
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "claims unstable gap IDs but lists none",
    )
}

/// Flips a builder receipt into eight fully-observed complete runs with
/// `repeat` evidence on every row and a re-derived honest summary (8
/// complete runs, no crashes/parse failures/timeouts, runtimes 100..800 —
/// min 100, median 500, max 800, total 3600 — 8/8 stable,
/// weakly_exposed=8, direct=8, `pass` gate). This is the one shape under
/// which the stability aggregates are derivable: every run row carries a
/// compared pass (#3733 review).
fn flip_to_fully_complete_0_3(receipt: &mut Value) {
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut) {
        for row in rows.iter_mut() {
            if let Some(entry) = row.as_object_mut() {
                entry.insert("status".to_string(), json!("complete"));
                entry.insert("execution".to_string(), json!("executed"));
                entry.insert("materialization".to_string(), json!("materialized"));
                entry.insert("detection".to_string(), json!("detected"));
                if let Some(Value::Object(corpus)) = entry.get_mut("corpus_selection") {
                    corpus.insert("state".to_string(), json!("selected"));
                }
                entry.insert(
                        "classification_counts".to_string(),
                        json!({"exposed": 0, "weakly_exposed": 1, "reachable_unrevealed": 0, "no_static_path": 0, "infection_unknown": 0, "propagation_unknown": 0, "static_unknown": 0}),
                    );
                entry.insert(
                        "alignment_counts".to_string(),
                        json!({"direct": 1, "alias": 0, "changed_sink_token": 0, "orthogonal": 0, "unknown": 0, "absent": 0}),
                    );
                entry.insert(
                    "repeat".to_string(),
                    json!({
                        "comparable_with": "pass-1",
                        "gap_ids_stable": true,
                        "unstable_gap_ids": [],
                    }),
                );
            }
        }
    }
    if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
        summary.insert("repos_run".to_string(), json!(8));
        summary.insert("repos_clone_failed".to_string(), json!(0));
        summary.insert("crash_count".to_string(), json!(0));
        summary.insert("crash_rate".to_string(), json!(0.0));
        summary.insert("parse_failure_count".to_string(), json!(0));
        summary.insert("parse_failure_rate".to_string(), json!(0.0));
        summary.insert("timed_out_count".to_string(), json!(0));
        summary.insert("runtime_ms_min".to_string(), json!(100));
        summary.insert("runtime_ms_median".to_string(), json!(500));
        summary.insert("runtime_ms_max".to_string(), json!(800));
        summary.insert("runtime_ms_total".to_string(), json!(3600));
        summary.insert("gap_id_stable_count".to_string(), json!(8));
        summary.insert("gap_id_unstable_count".to_string(), json!(0));
        summary.insert("gap_id_stability_rate".to_string(), json!(1.0));
        summary.insert(
                "classification_counts".to_string(),
                json!({"exposed": 0, "weakly_exposed": 8, "reachable_unrevealed": 0, "no_static_path": 0, "infection_unknown": 0, "propagation_unknown": 0, "static_unknown": 0}),
            );
        summary.insert(
                "alignment_counts".to_string(),
                json!({"direct": 8, "alias": 0, "changed_sink_token": 0, "orthogonal": 0, "unknown": 0, "absent": 0}),
            );
        summary.insert("gate_status".to_string(), json!("pass"));
        summary.insert(
            "gate_reason".to_string(),
            json!("8 complete runs; no crashes; repeat evidence stable on every row"),
        );
    }
}

#[test]
fn current_receipt_with_full_repeat_evidence_reaches_pass() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
    let mut receipt = current_receipt_0_3(&manifest);
    // Every row becomes a fully-observed complete run whose stability
    // evidence lives in `repeat`; the rows now derive the `pass` gate.
    flip_to_fully_complete_0_3(&mut receipt);
    let check = validate_receipt_value(&receipt, &manifest, &sha)?;
    assert_eq!(check.denominator_selected, 8);
    assert_eq!(check.denominator_run, 8);
    Ok(())
}

#[test]
fn current_receipt_rejects_malformed_digests_and_paths() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

    let mut receipt = current_receipt_0_3(&manifest);
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[0].as_object_mut()
    {
        entry.insert("input_digest".to_string(), json!("not-a-digest"));
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "bare lowercase sha256 hex",
    )?;

    let mut receipt = current_receipt_0_3(&manifest);
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[0].as_object_mut()
    {
        entry.insert("selected_root".to_string(), json!("/var/lib/ripr"));
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "repo-relative, not absolute",
    )
}

#[test]
fn current_receipt_requires_manifest_binding() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
    let mut receipt = current_receipt_0_3(&manifest);
    if let Some(object) = receipt.as_object_mut() {
        object.remove("manifest_digest");
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "schema-0.3 receipts are bound to the accepted manifest by digest",
    )
}

#[test]
fn current_receipt_discloses_missing_identity_fields_as_incomplete() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
    let mut receipt = current_receipt_0_3(&manifest);
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[0].as_object_mut()
    {
        for field in [
            "binary",
            "config",
            "digests",
            "repeat",
            "corpus_selection",
            "materialization",
            "detection",
            "execution",
            "input_digest",
            "selected_root",
        ] {
            entry.remove(field);
        }
    }
    // With the row's `repeat` evidence gone, the stability aggregates are
    // no longer derivable — the emitter could not have written them, so
    // they are omitted here and the absence is disclosed below.
    if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
        summary.remove("gap_id_stable_count");
        summary.remove("gap_id_unstable_count");
        summary.remove("gap_id_stability_rate");
    }
    let check = validate_receipt_value(&receipt, &manifest, &sha)?;
    let fields: BTreeSet<String> = check
        .incomplete
        .iter()
        .filter(|diagnostic| diagnostic.subject == "alpha")
        .map(|diagnostic| diagnostic.field.clone())
        .collect();
    for field in [
        "binary",
        "config",
        "digests",
        "repeat",
        "corpus_selection",
        "materialization",
        "detection",
        "execution",
        "input_digest",
        "selected_root",
    ] {
        assert!(
            fields.contains(field),
            "missing `{field}` must be typed incomplete, got: {fields:?}"
        );
    }
    Ok(())
}

// -- #3733 review fixes ---------------------------------------------------

#[test]
fn analyzed_receipt_missing_summary_aggregate_fails() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
    // An analyzed receipt must carry the emitted summary in full: the
    // emitter records every aggregate, and a deleted field would silently
    // disable its row-agreement check (#3733 review).
    for field in ["runtime_ms_total", "classification_counts", "crash_rate"] {
        let mut receipt = current_receipt_0_3(&manifest);
        if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
            summary.remove(field);
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            &format!("summary.{field}"),
        )?;
    }
    Ok(())
}

#[test]
fn fully_evidenced_receipt_requires_stability_aggregates() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
    // Every run row carries `repeat` evidence (the fully-complete shape is
    // the one shape where stability is derivable), so the emitter writes
    // the stability aggregates; deleting one must not silently disable the
    // stability comparison.
    let mut receipt = current_receipt_0_3(&manifest);
    flip_to_fully_complete_0_3(&mut receipt);
    if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
        summary.remove("gap_id_stable_count");
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "summary.gap_id_stable_count",
    )
}

#[test]
fn manifest_rejects_null_empty_and_malformed_identities() -> Result<(), String> {
    // Explicit null: a present-but-null identity is not an absent one.
    let mut value = identity_complete_manifest();
    value["repos"][0]["provenance"] = Value::Null;
    expect_fail(validate_manifest_value(&parsed(&value)?), "explicitly null")?;

    // Malformed digest: present-but-garbage fails instead of completing.
    let mut value = identity_complete_manifest();
    value["repos"][0]["tree_digest"] = json!("nothex");
    expect_fail(validate_manifest_value(&parsed(&value)?), "sha256 hex")?;

    // Empty string.
    let mut value = identity_complete_manifest();
    value["repos"][0]["snapshot"] = json!("");
    expect_fail(
        validate_manifest_value(&parsed(&value)?),
        "must be non-empty",
    )?;

    // License stays required: null and empty fail outright, with the
    // subject and field named.
    let mut value = alternate_manifest();
    value["repos"][0]["license"] = Value::Null;
    expect_fail(validate_manifest_value(&parsed(&value)?), "field=`license`")?;

    let mut value = alternate_manifest();
    value["repos"][2]["license"] = json!("");
    expect_fail(
        validate_manifest_value(&parsed(&value)?),
        "subject=`charlie`",
    )
}

#[test]
fn receipt_identity_must_match_the_manifest_binding() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

    // License mismatch: both sides record the identity, so a difference
    // fails naming both sides.
    let mut receipt = current_receipt_0_3(&manifest);
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[0].as_object_mut()
    {
        entry.insert("license".to_string(), json!("GPL-3.0-only"));
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "does not match the accepted manifest license",
    )?;

    // The same for an optional identity the manifest pins.
    let (complete_manifest, complete_sha) = accepted_manifest(&identity_complete_manifest())?;
    let mut receipt = current_receipt_0_3(&complete_manifest);
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[0].as_object_mut()
    {
        entry.insert("retention_class".to_string(), json!("discarded"));
    }
    expect_fail(
        validate_receipt_value(&receipt, &complete_manifest, &complete_sha).map(|_| ()),
        "does not match the accepted manifest retention_class",
    )
}

#[test]
fn receipt_identity_without_manifest_side_discloses_incomplete() -> Result<(), String> {
    // Manifest without tree_digest; the receipt carries one. The value
    // cannot be bound, so the manifest side discloses incomplete — the
    // outcome is neither valid nor a failure (#3733 review).
    let mut gappy = identity_complete_manifest();
    if let Some(repos) = gappy.get_mut("repos").and_then(Value::as_array_mut) {
        for repo in repos.iter_mut() {
            if let Some(entry) = repo.as_object_mut() {
                entry.remove("tree_digest");
            }
        }
    }
    let (manifest, sha) = accepted_manifest(&gappy)?;
    let receipt_value = current_receipt_0_3(&manifest);
    let check = validate_receipt_value(&receipt_value, &manifest, &sha)?;
    assert!(
        check.incomplete.iter().any(|diagnostic| {
            diagnostic.subject == "alpha" && diagnostic.field == "manifest.tree_digest"
        }),
        "an unbindable receipt tree_digest must disclose the manifest side: {:?}",
        check.incomplete
    );
    assert_eq!(check.verdict(), Verdict::Incomplete);
    Ok(())
}

#[test]
fn absent_features_disclose_incomplete_and_malformed_features_fail() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

    // Receipt-level: absent ripr.features discloses incomplete.
    let mut receipt = current_receipt_0_3(&manifest);
    if let Some(ripr) = receipt.get_mut("ripr").and_then(Value::as_object_mut) {
        ripr.remove("features");
    }
    let check = validate_receipt_value(&receipt, &manifest, &sha)?;
    assert!(
        check
            .incomplete
            .iter()
            .any(|diagnostic| diagnostic.field == "ripr.features"),
        "absent ripr.features must disclose incomplete: {:?}",
        check.incomplete
    );

    // Receipt-level: present-but-malformed fails.
    let mut receipt = current_receipt_0_3(&manifest);
    if let Some(ripr) = receipt.get_mut("ripr").and_then(Value::as_object_mut) {
        ripr.insert("features".to_string(), json!("python"));
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "array of strings",
    )?;

    // Row-level: absent binary.features discloses incomplete.
    let mut receipt = current_receipt_0_3(&manifest);
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[0].as_object_mut()
        && let Some(Value::Object(binary)) = entry.get_mut("binary")
    {
        binary.remove("features");
    }
    let check = validate_receipt_value(&receipt, &manifest, &sha)?;
    assert!(
        check.incomplete.iter().any(
            |diagnostic| diagnostic.subject == "alpha" && diagnostic.field == "binary.features"
        ),
        "absent binary.features must disclose incomplete: {:?}",
        check.incomplete
    );

    // Row-level: present-but-malformed fails.
    let mut receipt = current_receipt_0_3(&manifest);
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[0].as_object_mut()
        && let Some(Value::Object(binary)) = entry.get_mut("binary")
    {
        binary.insert("features".to_string(), json!(42));
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "array of strings",
    )
}

#[test]
fn present_null_receipt_identities_fail_while_absent_discloses_incomplete() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

    // Row-level binding identities: an explicit null is a present-but-
    // garbage identity, not an absent one — it fails naming the field
    // (the receipt-side twin of the manifest's null-identity rule).
    let mut receipt = current_receipt_0_3(&manifest);
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[0].as_object_mut()
    {
        entry.insert("provenance".to_string(), Value::Null);
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "field=`provenance`",
    )?;

    let mut receipt = current_receipt_0_3(&manifest);
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[0].as_object_mut()
    {
        entry.insert("tree_digest".to_string(), Value::Null);
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "field=`tree_digest`",
    )?;

    // Receipt level: a null field in the `ripr` identity block fails.
    let mut receipt = current_receipt_0_3(&manifest);
    if let Some(ripr) = receipt.get_mut("ripr").and_then(Value::as_object_mut) {
        ripr.insert("source_sha".to_string(), Value::Null);
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "field=`ripr.source_sha`",
    )?;

    // The copy-check blocks fail the same way: a null inside a present
    // `repository` / `binary` block is garbage, not absence.
    let mut receipt = current_receipt_0_3(&manifest);
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[0].as_object_mut()
        && let Some(Value::Object(repository)) = entry.get_mut("repository")
    {
        repository.insert("url".to_string(), Value::Null);
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "field=`repository.url`",
    )?;

    let mut receipt = current_receipt_0_3(&manifest);
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[0].as_object_mut()
        && let Some(Value::Object(binary)) = entry.get_mut("binary")
    {
        binary.insert("digest".to_string(), Value::Null);
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "field=`binary.digest`",
    )?;

    // Key ABSENT keeps the typed-incomplete disclosure, never a failure.
    let mut receipt = current_receipt_0_3(&manifest);
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[0].as_object_mut()
    {
        entry.remove("provenance");
    }
    let check = validate_receipt_value(&receipt, &manifest, &sha)?;
    assert!(
        check.incomplete.iter().any(|diagnostic| {
            diagnostic.subject == "alpha" && diagnostic.field == "provenance"
        }),
        "an absent provenance key must disclose incomplete: {:?}",
        check.incomplete
    );
    Ok(())
}

#[test]
fn run_status_split_pins_which_statuses_count_as_run() -> Result<(), String> {
    // The 0.3 denominator split is exact (SPEC-0086): the five
    // analysis-attempting statuses count toward `repos_run`, and
    // `unsupported`/`tempfail`/`stale` do not — while every status stays
    // selected either way.
    let subject = AcceptedSubject {
        id: "alpha".to_string(),
        url: "https://example.com/alpha".to_string(),
        sha: VALID_SHA_A.to_string(),
        license: "MIT".to_string(),
        shape: "pytest_library".to_string(),
        synthetic_diff: "fixtures/python-eval-sweep/diffs/alpha.diff".to_string(),
        tree_digest: None,
        snapshot: None,
        provenance: None,
        retention_class: None,
    };
    assert_eq!(STATUS_VOCABULARY.len(), 8);
    for status in STATUS_VOCABULARY {
        let counts_as_run = RUN_STATUSES.contains(&status);
        let mut row = json!({
            "id": subject.id,
            "status": status,
        });
        if counts_as_run {
            row["runtime_ms"] = json!(100);
            row["classification_counts"] = json!({
                "exposed": 0, "weakly_exposed": 0, "reachable_unrevealed": 0,
                "no_static_path": 0, "infection_unknown": 0,
                "propagation_unknown": 0, "static_unknown": 0,
            });
            row["alignment_counts"] = json!({
                "direct": 0, "alias": 0, "changed_sink_token": 0,
                "orthogonal": 0, "unknown": 0, "absent": 0,
            });
        }
        let mut incomplete = Vec::new();
        let summary = validate_row(&row, &subject, true, None, &mut incomplete)?;
        assert_eq!(
            summary.counts_as_run,
            counts_as_run,
            "status `{status}` must {} toward repos_run",
            if counts_as_run { "count" } else { "not count" },
        );
    }
    Ok(())
}

// -- focused fix round: execution/stability contradictions, identity
//    copies, binary disclosure, owned-field type checks -------------------

#[test]
fn partial_row_with_not_executed_state_fails() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
    // `partial` is a run-status row (it counts toward `repos_run`), so an
    // execution state of `not-executed` contradicts it.
    let mut receipt = current_receipt_0_3(&manifest);
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[1].as_object_mut()
    {
        entry.insert("execution".to_string(), json!("not-executed"));
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "status `partial` cannot coexist with execution state `not-executed`",
    )
}

#[test]
fn disagreeing_tree_digest_copies_fail_and_matching_copies_pass() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

    // The `repository` block copy disagrees with the row-level digest:
    // both locations record one identity, so the mismatch fails naming
    // both.
    let mut receipt = current_receipt_0_3(&manifest);
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[0].as_object_mut()
        && let Some(Value::Object(repository)) = entry.get_mut("repository")
    {
        repository.insert("tree_digest".to_string(), json!(DIGEST_TWO));
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "`tree_digest` does not match `repository.tree_digest`",
    )?;

    // Matching copies validate (the fixture restates the same digest).
    let receipt = current_receipt_0_3(&manifest);
    validate_receipt_value(&receipt, &manifest, &sha)?;
    Ok(())
}

#[test]
fn row_binary_identity_must_match_the_receipt_ripr_block() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
    // The row `binary` block restates the receipt-level `ripr` identity;
    // a disagreeing copy of any owned field fails naming both locations.
    for (field, wrong) in [
        ("digest", json!(DIGEST_ONE)),
        ("version", json!("ripr 9.9.9")),
        ("features", json!(["rust"])),
        ("build_profile", json!("release")),
    ] {
        let mut receipt = current_receipt_0_3(&manifest);
        if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
            && let Some(entry) = rows[0].as_object_mut()
            && let Some(Value::Object(binary)) = entry.get_mut("binary")
        {
            binary.insert(field.to_string(), wrong);
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            &format!("`binary.{field}` does not match"),
        )?;
    }
    Ok(())
}

#[test]
fn false_stability_claim_requires_the_unstable_list() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

    // 0.3: a false claim inside `repeat` with the list omitted fails
    // naming the omitted field.
    let mut receipt = current_receipt_0_3(&manifest);
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[0].as_object_mut()
        && let Some(Value::Object(repeat)) = entry.get_mut("repeat")
    {
        repeat.insert("gap_ids_stable".to_string(), json!(false));
        repeat.remove("unstable_gap_ids");
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "field=`repeat.unstable_gap_ids`",
    )?;

    // 0.2: the same omission at the row level fails the same way.
    let mut receipt = historical_receipt_0_2(&alternate_manifest());
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[0].as_object_mut()
    {
        entry.insert("gap_ids_stable".to_string(), json!(false));
        entry.remove("unstable_gap_ids");
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "field=`unstable_gap_ids`",
    )
}

#[test]
fn present_binary_block_without_build_profile_discloses_incomplete() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

    // A present `binary` block missing `build_profile` is a named
    // incomplete disclosure, not a silent pass.
    let mut receipt = current_receipt_0_3(&manifest);
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[0].as_object_mut()
        && let Some(Value::Object(binary)) = entry.get_mut("binary")
    {
        binary.remove("build_profile");
    }
    let check = validate_receipt_value(&receipt, &manifest, &sha)?;
    assert!(
        check.incomplete.iter().any(|diagnostic| {
            diagnostic.subject == "alpha" && diagnostic.field == "binary.build_profile"
        }),
        "missing binary.build_profile must be disclosed incomplete: {:?}",
        check.incomplete
    );
    assert_eq!(check.verdict(), Verdict::Incomplete);

    // A malformed profile value still fails outright.
    let mut receipt = current_receipt_0_3(&manifest);
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[0].as_object_mut()
        && let Some(Value::Object(binary)) = entry.get_mut("binary")
    {
        binary.insert("build_profile".to_string(), json!("ultra"));
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "unknown build profile",
    )
}

#[test]
fn owned_fields_get_type_checks_per_emitted_shape() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

    // Summary: wrong-typed `gate_reason` (number) fails naming the field.
    let mut receipt = current_receipt_0_3(&manifest);
    if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut) {
        summary.insert("gate_reason".to_string(), json!(7));
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "field=`gate_reason`",
    )?;

    // 0.2 row: wrong-typed `gap_ids` (object) fails.
    let mut receipt = historical_receipt_0_2(&alternate_manifest());
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[0].as_object_mut()
    {
        entry.insert("gap_ids".to_string(), json!({"gap:python:x": 1}));
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "field=`gap_ids`",
    )?;

    // 0.2 row: wrong-typed `stderr_excerpt` (array) fails.
    let mut receipt = historical_receipt_0_2(&alternate_manifest());
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[0].as_object_mut()
    {
        entry.insert("stderr_excerpt".to_string(), json!(["boom"]));
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "field=`stderr_excerpt`",
    )?;

    // Manifest: wrong-typed `why` (array) fails naming the subject.
    let mut value = alternate_manifest();
    value["repos"][0]["why"] = json!(["because"]);
    expect_fail(validate_manifest_value(&parsed(&value)?), "subject=`alpha`")?;

    // Manifest: wrong-typed `limits` (object) fails.
    let mut value = alternate_manifest();
    value["limits"] = json!({"static": true});
    expect_fail(validate_manifest_value(&parsed(&value)?), "field=`limits`")?;

    // Manifest: wrong-typed `description` (number) fails.
    let mut value = alternate_manifest();
    value["description"] = json!(42);
    expect_fail(
        validate_manifest_value(&parsed(&value)?),
        "field=`description`",
    )
}

// -- hardening round: malformed blocks fail, host rules, zero-run keys ---

#[test]
fn malformed_repository_block_fails_while_absence_discloses_incomplete() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

    // A wrong-typed `repository` block is malformed, not absent: it fails
    // naming the field instead of disclosing an incomplete identity
    // (#3733 review).
    let mut receipt = current_receipt_0_3(&manifest);
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[0].as_object_mut()
    {
        entry.insert("repository".to_string(), json!("x"));
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "repository identity must be an object when present",
    )?;

    // Only ABSENCE discloses: removing the block stays a typed-incomplete
    // disclosure, not a failure.
    let mut receipt = current_receipt_0_3(&manifest);
    if let Some(rows) = receipt.get_mut("repos").and_then(Value::as_array_mut)
        && let Some(entry) = rows[0].as_object_mut()
    {
        entry.remove("repository");
    }
    let check = validate_receipt_value(&receipt, &manifest, &sha)?;
    assert!(
        check.incomplete.iter().any(|diagnostic| {
            diagnostic.subject == "alpha" && diagnostic.field == "repository"
        }),
        "an absent repository block must disclose incomplete: {:?}",
        check.incomplete
    );
    assert_eq!(check.verdict(), Verdict::Incomplete);
    Ok(())
}

#[test]
fn malformed_synthetic_diff_fails_even_with_a_valid_fallback() -> Result<(), String> {
    // A per-subject wrong type cannot hide behind a valid top-level
    // fallback: a fallback repairs absence, never malformedness
    // (#3733 review).
    let mut value = alternate_manifest();
    value["synthetic_diff"] = json!("fixtures/python-eval-sweep/synthetic-diff.diff");
    value["repos"][0]["synthetic_diff"] = json!(42);
    expect_fail(
        validate_manifest_value(&parsed(&value)?),
        "field=`synthetic_diff`",
    )?;

    // The same for a present non-portable path at the subject level.
    let mut value = alternate_manifest();
    value["synthetic_diff"] = json!("fixtures/python-eval-sweep/synthetic-diff.diff");
    value["repos"][0]["synthetic_diff"] = json!("../escape.diff");
    expect_fail(
        validate_manifest_value(&parsed(&value)?),
        "must not contain `..`",
    )?;

    // And a malformed top-level fallback fails at its own level even
    // when every subject carries a valid path.
    let mut value = alternate_manifest();
    value["synthetic_diff"] = json!(["fixtures/python-eval-sweep/synthetic-diff.diff"]);
    expect_fail(
        validate_manifest_value(&parsed(&value)?),
        "field=`synthetic_diff`",
    )
}

#[test]
fn rejects_hostless_whitespace_and_dotless_https_urls() -> Result<(), String> {
    for (url, needle) in [
        ("https:///path", "has no host"),
        ("https://ex ample.com/x", "must not contain whitespace"),
        // A dotless host is malformed under the same conservative host
        // rule: `https://host` carries no repository host shape, so it is
        // rejected by design, not overlooked.
        ("https://host", "no dotted host"),
    ] {
        let mut value = alternate_manifest();
        value["repos"][0]["url"] = json!(url);
        expect_fail(validate_manifest_value(&parsed(&value)?), needle)?;
    }

    // Existing valid URLs keep passing.
    let mut value = alternate_manifest();
    value["repos"][0]["url"] = json!("https://example.com/alpha/nested/path");
    validate_manifest_value(&parsed(&value)?)?;
    Ok(())
}

#[test]
fn zero_run_summary_distribution_requires_the_emitted_key_set() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;

    // A zero-run alignment_counts without `absent`: every recorded bucket
    // is zero, but the distribution drops a required emitted bucket, so
    // it fails (#3733 review).
    let mut receipt = zero_run_receipt_0_2(&alternate_manifest());
    if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut)
        && let Some(Value::Object(counts)) = summary.get_mut("alignment_counts")
    {
        counts.remove("absent");
    }
    expect_fail(
        validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
        "omits required bucket `absent`",
    )?;

    // The same law for `unknown` and for a classification bucket: the
    // emitter zero-fills every bucket (eval_sweep.rs `to_json`), so a
    // recorded distribution must carry the full emitted key set.
    for (field, bucket) in [
        ("alignment_counts", "unknown"),
        ("classification_counts", "static_unknown"),
    ] {
        let mut receipt = zero_run_receipt_0_2(&alternate_manifest());
        if let Some(summary) = receipt.get_mut("summary").and_then(Value::as_object_mut)
            && let Some(Value::Object(counts)) = summary.get_mut(field)
        {
            counts.remove(bucket);
        }
        expect_fail(
            validate_receipt_value(&receipt, &manifest, &sha).map(|_| ()),
            &format!("omits required bucket `{bucket}`"),
        )?;
    }

    // The full zero-filled emitted key set validates.
    let receipt = zero_run_receipt_0_2(&alternate_manifest());
    validate_receipt_value(&receipt, &manifest, &sha)?;
    Ok(())
}

// -- verdict + rendering -------------------------------------------------

#[test]
fn not_run_is_never_a_pass_in_verdict_vocabulary() -> Result<(), String> {
    let (manifest, _) = accepted_manifest(&alternate_manifest())?;
    let outcome = CheckOutcome {
        manifest_path: "manifest.json".to_string(),
        accepted: manifest,
        receipt: None,
    };
    assert_eq!(outcome.verdict(), Verdict::NotRun);
    assert_eq!(outcome.verdict().as_str(), "not_run");
    Ok(())
}

#[test]
fn top_level_verdict_is_incomplete_when_manifest_gaps_survive_a_complete_receipt()
-> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
    // The retained manifest discloses 32 incomplete identities (four per
    // subject); the 0.3 receipt is otherwise complete on its own — but the
    // identities it restates have no manifest side to bind against, so
    // each one discloses the unbindable manifest gap (#3733 review)
    // instead of fabricating a binding.
    assert_eq!(manifest.incomplete.len(), 8 * 4);
    let receipt_value = current_receipt_0_3(&manifest);
    let receipt = validate_receipt_value(&receipt_value, &manifest, &sha)?;
    assert!(
        receipt
            .incomplete
            .iter()
            .any(|diagnostic| diagnostic.field == "manifest.tree_digest"),
        "a receipt identity with no manifest side must disclose the gap: {:?}",
        receipt.incomplete
    );
    let outcome = CheckOutcome {
        manifest_path: "manifest.json".to_string(),
        accepted: manifest,
        receipt: Some(receipt),
    };
    // A complete receipt must not hide manifest gaps: the top-level
    // verdict is exactly `incomplete`.
    assert_eq!(outcome.verdict(), Verdict::Incomplete);
    assert_eq!(outcome.verdict().as_str(), "incomplete");
    Ok(())
}

#[test]
fn top_level_verdict_is_valid_only_when_both_artifacts_carry_zero_incompletes() -> Result<(), String>
{
    let complete = identity_complete_manifest();
    let (manifest, sha) = accepted_manifest(&complete)?;
    assert!(manifest.incomplete.is_empty());
    // The fully-complete shape (every row complete with compared repeat
    // evidence) is the one receipt shape with zero incompletes: rows
    // without a compared pass disclose the absent comparison, so a
    // builder receipt over even a pinned manifest is `incomplete`.
    let mut receipt_value = current_receipt_0_3(&manifest);
    flip_to_fully_complete_0_3(&mut receipt_value);
    let receipt = validate_receipt_value(&receipt_value, &manifest, &sha)?;
    assert!(receipt.incomplete.is_empty());
    let outcome = CheckOutcome {
        manifest_path: "manifest.json".to_string(),
        accepted: manifest,
        receipt: Some(receipt),
    };
    assert_eq!(outcome.verdict(), Verdict::Valid);
    assert_eq!(outcome.verdict().as_str(), "valid");
    Ok(())
}

#[test]
fn check_report_json_is_stable_and_versioned() -> Result<(), String> {
    let (manifest, sha) = accepted_manifest(&alternate_manifest())?;
    let receipt_value = current_receipt_0_3(&manifest);
    let receipt = validate_receipt_value(&receipt_value, &manifest, &sha)?;
    let outcome = CheckOutcome {
        manifest_path: "manifest.json".to_string(),
        accepted: manifest,
        receipt: Some(receipt),
    };
    let rendered_a = render_check_json(&outcome);
    let rendered_b = render_check_json(&outcome);
    let text_a = rendered_a?;
    assert_eq!(
        Ok(text_a.clone()),
        rendered_b,
        "check JSON must be deterministic"
    );
    assert!(text_a.contains("\"kind\": \"python_eval_sweep_check_report\""));
    // The receipt validates to `incomplete` or `valid`, never a bare pass.
    assert!(
        text_a.contains("\"verdict\": \"valid\"") || text_a.contains("\"verdict\": \"incomplete\"")
    );
    Ok(())
}

#[test]
fn diagnostics_name_subject_field_reason_and_rerun() -> Result<(), String> {
    let mut value = alternate_manifest();
    value["repos"][2]["license"] = json!("");
    let error = match validate_manifest_value(&parsed(&value)?) {
        Ok(()) => return Err("empty license must fail the manifest".to_string()),
        Err(error) => error,
    };
    for needle in ["subject=`charlie`", "field=`license`", RERUN_COMMAND] {
        assert!(
            error.contains(needle),
            "diagnostic `{error}` must mention `{needle}`"
        );
    }
    Ok(())
}
