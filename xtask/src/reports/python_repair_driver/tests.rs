// ---------------------------------------------------------------------------
// Tests (module named `python_repair_driver_binding` under the file module
// `python_repair_driver`, so `cargo test -p xtask python_repair_driver`
// selects exactly these tests; no unwrap/expect — Result returns only)
// ---------------------------------------------------------------------------

use super::super::python_repair_trust::{MANIFEST_KIND, SCHEMA_VERSION};
use super::*;
use crate::python_judged_panel_replay::sha256_hex;

const GIT_SHA_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const GIT_SHA_C: &str = "cccccccccccccccccccccccccccccccccccccccc";
const DIGEST_ONE: &str = "1010101010101010101010101010101010101010101010101010101010101010";
const DIGEST_TWO: &str = "2020202020202020202020202020202020202020202020202020202020202020";

/// A complete, well-formed selection row whose recorded `selection_digest`
/// is recomputed from the content.
fn selection_row(attempt_id: &str, target_path: &str, target_state: &str) -> Value {
    let mut row = json!({
        "attempt_id": attempt_id,
        "case_id": format!("case-{attempt_id}"),
        "subject_id": format!("subj-{attempt_id}"),
        "repository": "https://example.com/repo",
        "base": GIT_SHA_B,
        "head": GIT_SHA_C,
        "tree": DIGEST_ONE,
        "source_currentness": "candidate_current",
        "selection_reason": "behavior changed in the diff and the case discriminates it",
        "diversity_stratum": "pytest_library",
        "family": "error_path_gating",
        "owner": "module.handler",
        "discriminator": "raises ValueError on empty payload",
        "relation": "case calls owner directly",
        "oracle": "pytest.raises exact message pin",
        "expected_direction": "should_gap",
        "claim_boundary": "static exposure evidence only",
        "target_path": target_path,
        "target_state": target_state,
        "selected_at": "2026-09-10T00:00:00Z",
        "selector": "campaign-selector",
        "authority_snapshot_digest": DIGEST_TWO,
    });
    if let Some(entry) = row.as_object_mut() {
        let digest = canonical_selection_digest(entry, attempt_id).unwrap_or_default();
        entry.insert("selection_digest".to_string(), json!(digest));
    }
    row
}

struct Fixture {
    text: String,
    sha256: String,
    value: Value,
}

/// The accepted manifest for one row, with its exact-bytes digest.
fn build_fixture(row: Value) -> Result<Fixture, String> {
    let manifest = json!({
        "schema_version": SCHEMA_VERSION,
        "kind": MANIFEST_KIND,
        "spec": KNOWN_SPEC,
        "description": "driver binding fixture",
        "selections": [row],
    });
    let text = serde_json::to_string_pretty(&manifest)
        .map_err(|error| format!("serialize fixture manifest: {error}"))?;
    let sha256 = sha256_hex(text.as_bytes());
    let value = serde_json::from_str(&text).map_err(|error| format!("reparse fixture: {error}"))?;
    Ok(Fixture {
        text,
        sha256,
        value,
    })
}

fn row_digest(fixture: &Fixture, attempt_id: &str) -> Result<String, String> {
    let row = fixture
        .value
        .get("selections")
        .and_then(Value::as_array)
        .and_then(|rows| rows.first())
        .cloned()
        .ok_or("fixture manifest carries no row")?;
    let canonical = row.as_object().cloned().ok_or("row is not an object")?;
    canonical_selection_digest(&canonical, attempt_id)
}

fn prepare_record(fixture: &Fixture, attempt_id: &str, target_path: &str) -> Result<Value, String> {
    let record = json!({
        "schema_version": BINDING_SCHEMA_VERSION,
        "kind": BINDING_KIND,
        "spec": BINDING_SPEC,
        "phase": "prepare",
        "seam_id": "seam-under-test",
        "repository_head": GIT_SHA_C,
        "selection_manifest_path": "target/ripr/trust-manifest.json",
        "driver": {
            "binary_sha256": DIGEST_ONE,
            "version": "0.11.0",
        },
        "config": {
            "profile": "subject-ripr-toml",
        },
        "input": {
            "packet_sha256": DIGEST_ONE,
            "before_snapshot_sha256": DIGEST_TWO,
        },
        "trust": {
            "attempt_id": attempt_id,
            "case_id": format!("case-{attempt_id}"),
            "subject_id": format!("subj-{attempt_id}"),
            "repository": "https://example.com/repo",
            "base": GIT_SHA_B,
            "head": GIT_SHA_C,
            "tree": DIGEST_ONE,
            "source_currentness": "candidate_current",
            "selection_manifest_sha256": fixture.sha256,
            "selection_digest": row_digest(fixture, attempt_id)?,
            "target_path": target_path,
            "target_state": "existing",
            "family": "error_path_gating",
            "owner": "module.handler",
            "discriminator": "raises ValueError on empty payload",
            "relation": "case calls owner directly",
            "oracle": "pytest.raises exact message pin",
        },
        "edit_surface": {
            "allowed": [target_path],
            "forbidden": [],
        },
        "authorization": {
            "status": "granted",
            "authority": "operator-a",
            "method": "explicit-operator-flags",
        },
        "non_claims": BINDING_NON_CLAIMS,
    });
    Ok(record)
}

fn apply_record(fixture: &Fixture, attempt_id: &str, target_path: &str) -> Result<Value, String> {
    let mut record = prepare_record(fixture, attempt_id, target_path)?;
    let object = record
        .as_object_mut()
        .ok_or_else(|| "apply record is not an object".to_string())?;
    object.insert("phase".to_string(), json!("apply"));
    object.insert(
        "durable_attempt_id".to_string(),
        json!("repair-attempt-0123456789abcdef01234567"),
    );
    object.insert("binding_artifact_sha256".to_string(), json!(DIGEST_ONE));
    object.insert(
        "apply".to_string(),
        json!({
            "patch_sha256": DIGEST_TWO,
            "changed_paths": [target_path],
            "cage_status": "compliant",
            "repository_head_after": GIT_SHA_C,
            "current": true,
        }),
    );
    Ok(record)
}

/// Sets one field of the record's trust block.
fn set_trust_field(record: &mut Value, field: &str, value: Value) {
    if let Some(object) = record.as_object_mut()
        && let Some(trust) = object.get_mut("trust").and_then(Value::as_object_mut)
    {
        trust.insert(field.to_string(), value);
    }
}

/// Sets (or removes) one field of the record's authorization block.
fn set_authorization_field(record: &mut Value, field: &str, value: Option<Value>) {
    if let Some(object) = record.as_object_mut()
        && let Some(authorization) = object
            .get_mut("authorization")
            .and_then(Value::as_object_mut)
    {
        match value {
            Some(value) => {
                authorization.insert(field.to_string(), value);
            }
            None => {
                authorization.remove(field);
            }
        }
    }
}

fn checked(record: &Value, fixture: &Fixture) -> Result<DriverBindingRecord, String> {
    let manifest = validate_selection_manifest(&fixture.value, fixture.sha256.clone())?;
    // The record-file digest is an input of the caller (the artifact
    // loader); the unit route supplies the digest of a synthetic file so
    // per-record validation stays the subject under test.
    validate_binding_record(
        "record.json",
        record,
        &manifest,
        &fixture.sha256,
        &sha256_hex(b"record.json"),
    )
}

fn expect_rejection(record: &Value, fixture: &Fixture, needle: &str) -> Result<(), String> {
    match checked(record, fixture) {
        Err(error) if error.contains(needle) => Ok(()),
        Err(error) => Err(format!("rejection did not name `{needle}`: {error}")),
        Ok(_) => Err(format!("expected rejection containing `{needle}`, got Ok")),
    }
}

#[test]
fn accepts_valid_prepare_record() -> Result<(), String> {
    let fixture = build_fixture(selection_row("att-ok", "tests/test_handler.py", "existing"))?;
    let record = prepare_record(&fixture, "att-ok", "tests/test_handler.py")?;
    checked(&record, &fixture)?;
    Ok(())
}

#[test]
fn accepts_valid_apply_record() -> Result<(), String> {
    let fixture = build_fixture(selection_row("att-ok", "tests/test_handler.py", "existing"))?;
    let record = apply_record(&fixture, "att-ok", "tests/test_handler.py")?;
    checked(&record, &fixture)?;
    Ok(())
}

#[test]
fn rejects_stale_manifest_digest() -> Result<(), String> {
    let fixture = build_fixture(selection_row(
        "att-stale",
        "tests/test_handler.py",
        "existing",
    ))?;
    let mut record = apply_record(&fixture, "att-stale", "tests/test_handler.py")?;
    if let Some(object) = record.as_object_mut()
        && let Some(trust) = object.get_mut("trust").and_then(Value::as_object_mut)
    {
        trust.insert("selection_manifest_sha256".to_string(), json!(DIGEST_ONE));
    }
    expect_rejection(&record, &fixture, "stale digest")
}

#[test]
fn rejects_unknown_trust_attempt() -> Result<(), String> {
    let fixture = build_fixture(selection_row(
        "att-known",
        "tests/test_handler.py",
        "existing",
    ))?;
    let mut record = prepare_record(&fixture, "att-unknown", "tests/test_handler.py")?;
    if let Some(object) = record.as_object_mut()
        && let Some(trust) = object.get_mut("trust").and_then(Value::as_object_mut)
    {
        trust.insert("attempt_id".to_string(), json!("att-unknown"));
    }
    expect_rejection(
        &record,
        &fixture,
        "outside the accepted selection denominator",
    )
}

#[test]
fn rejects_replaced_selection_row_digest() -> Result<(), String> {
    let fixture = build_fixture(selection_row(
        "att-row",
        "tests/test_handler.py",
        "existing",
    ))?;
    let mut record = prepare_record(&fixture, "att-row", "tests/test_handler.py")?;
    if let Some(object) = record.as_object_mut()
        && let Some(trust) = object.get_mut("trust").and_then(Value::as_object_mut)
    {
        trust.insert("selection_digest".to_string(), json!(DIGEST_TWO));
    }
    expect_rejection(&record, &fixture, "selection row digest mismatch")
}

#[test]
fn rejects_target_disagreement_with_the_row() -> Result<(), String> {
    let fixture = build_fixture(selection_row(
        "att-target",
        "tests/test_handler.py",
        "existing",
    ))?;
    let mut record = prepare_record(&fixture, "att-target", "tests/other_test.py")?;
    set_trust_field(&mut record, "target_path", json!("tests/other_test.py"));
    expect_rejection(&record, &fixture, "target identity disagreement")
}

#[test]
fn rejects_denied_and_unbindable_targets() -> Result<(), String> {
    // The row itself declares the denied surface `unsafe` (accepted by the
    // corpus), but the driver record that names it must still fail: the
    // driver binds only `existing` targets.
    let unsafe_fixture = build_fixture(selection_row("att-unsafe", "vendor/lib/ext.py", "unsafe"))?;
    let mut unsafe_record = prepare_record(&unsafe_fixture, "att-unsafe", "vendor/lib/ext.py")?;
    set_trust_field(&mut unsafe_record, "target_state", json!("unsafe"));
    expect_rejection(
        &unsafe_record,
        &unsafe_fixture,
        "driver binding target state must be `existing`",
    )?;
    // Every unbindable state fails the driver check even when its path is
    // not denied.
    for state in ["proposed", "ambiguous", "unavailable", "unsafe"] {
        let fixture = build_fixture(selection_row(
            "att-unbindable",
            "tests/test_handler.py",
            state,
        ))?;
        let mut record = prepare_record(&fixture, "att-unbindable", "tests/test_handler.py")?;
        set_trust_field(&mut record, "target_state", json!(state));
        expect_rejection(
            &record,
            &fixture,
            "driver binding target state must be `existing`",
        )?;
    }
    // An `existing` record naming a denied surface fails the surface
    // rule. The corpus validator would refuse such a row first (a denied
    // surface must be declared `unsafe`), so the accepted manifest is
    // built directly: the driver check owns its own refusal regardless
    // of how such a row arrived.
    let denied_record = {
        let row = json!({
            "attempt_id": "att-denied",
            "case_id": "case-att-denied",
            "subject_id": "subj-att-denied",
            "repository": "https://example.com/repo",
            "base": GIT_SHA_B,
            "head": GIT_SHA_C,
            "family": "error_path_gating",
            "owner": "module.handler",
            "discriminator": "raises ValueError on empty payload",
            "relation": "case calls owner directly",
            "oracle": "pytest.raises exact message pin",
            "target_path": "generated/helper.py",
            "target_state": "existing",
        });
        let mut preimage = row
            .as_object()
            .cloned()
            .ok_or_else(|| "denied row is not an object".to_string())?;
        let row_digest = canonical_selection_digest(&preimage, "att-denied")?;
        preimage.insert("selection_digest".to_string(), json!(row_digest));
        let manifest = SelectionManifest {
            sha256: sha256_hex(b"denied-manifest"),
            selections: vec![super::super::python_repair_trust::Selection {
                attempt_id: "att-denied".to_string(),
                diversity_stratum: "pytest_library".to_string(),
                target_path: "generated/helper.py".to_string(),
                target_state: "existing".to_string(),
            }],
            incomplete: Vec::new(),
            row_preimages: std::collections::BTreeMap::from([("att-denied".to_string(), preimage)]),
        };
        let mut record = prepare_record_fixture("att-denied", "generated/helper.py")?;
        if let Some(object) = record.as_object_mut()
            && let Some(trust) = object.get_mut("trust").and_then(Value::as_object_mut)
        {
            trust.insert(
                "selection_manifest_sha256".to_string(),
                json!(sha256_hex(b"denied-manifest")),
            );
            trust.insert("selection_digest".to_string(), json!(row_digest));
        }
        let outcome = validate_binding_record(
            "denied.json",
            &record,
            &manifest,
            &sha256_hex(b"denied-manifest"),
            &sha256_hex(b"denied.json"),
        );
        outcome.map(|_| ())
    };
    let error = match denied_record {
        Err(error) => error,
        Ok(()) => {
            return Err("a denied-surface target passed the driver check".to_string());
        }
    };
    if !error.contains("production/generated/vendor/environment edit surface") {
        return Err(format!("unexpected denied-target failure: {error}"));
    }
    Ok(())
}

/// A prepare record fixture that does not depend on a built fixture: the
/// digest anchors are filled in by the caller.
fn prepare_record_fixture(attempt_id: &str, target_path: &str) -> Result<Value, String> {
    let record = json!({
        "schema_version": BINDING_SCHEMA_VERSION,
        "kind": BINDING_KIND,
        "spec": BINDING_SPEC,
        "phase": "prepare",
        "seam_id": "seam-under-test",
        "repository_head": GIT_SHA_C,
        "selection_manifest_path": "target/ripr/trust-manifest.json",
        "driver": {
            "binary_sha256": DIGEST_ONE,
            "version": "0.11.0",
        },
        "config": {
            "profile": "subject-ripr-toml",
        },
        "input": {
            "packet_sha256": DIGEST_ONE,
            "before_snapshot_sha256": DIGEST_TWO,
        },
        "trust": {
            "attempt_id": attempt_id,
            "case_id": format!("case-{attempt_id}"),
            "subject_id": format!("subj-{attempt_id}"),
            "repository": "https://example.com/repo",
            "base": GIT_SHA_B,
            "head": GIT_SHA_C,
            "selection_manifest_sha256": DIGEST_ONE,
            "selection_digest": DIGEST_TWO,
            "target_path": target_path,
            "target_state": "existing",
            "family": "error_path_gating",
            "owner": "module.handler",
            "discriminator": "raises ValueError on empty payload",
            "relation": "case calls owner directly",
            "oracle": "pytest.raises exact message pin",
        },
        "edit_surface": {
            "allowed": [target_path],
            "forbidden": [],
        },
        "authorization": {
            "status": "granted",
            "authority": "operator-a",
            "method": "explicit-operator-flags",
        },
        "non_claims": BINDING_NON_CLAIMS,
    });
    Ok(record)
}

#[test]
fn rejects_inferred_or_incomplete_authorization() -> Result<(), String> {
    let fixture = build_fixture(selection_row(
        "att-auth",
        "tests/test_handler.py",
        "existing",
    ))?;
    for (field, value, needle) in [
        ("status", json!("inferred"), "unknown authorization status"),
        ("authority", json!(None::<String>), "required field"),
        ("method", json!("automatic"), "unknown authorization method"),
    ] {
        let mut record = prepare_record(&fixture, "att-auth", "tests/test_handler.py")?;
        let replacement = if value.is_null() {
            None
        } else {
            Some(value.clone())
        };
        set_authorization_field(&mut record, field, replacement);
        expect_rejection(&record, &fixture, needle)?;
    }
    Ok(())
}

#[test]
fn rejects_missing_apply_block_on_apply_phase() -> Result<(), String> {
    let fixture = build_fixture(selection_row(
        "att-apply",
        "tests/test_handler.py",
        "existing",
    ))?;
    let mut record = apply_record(&fixture, "att-apply", "tests/test_handler.py")?;
    if let Some(object) = record.as_object_mut() {
        object.remove("apply");
    }
    expect_rejection(&record, &fixture, "must carry the `apply` block")
}

#[test]
fn rejects_malformed_durable_attempt_identity() -> Result<(), String> {
    let fixture = build_fixture(selection_row(
        "att-durable",
        "tests/test_handler.py",
        "existing",
    ))?;
    let mut record = apply_record(&fixture, "att-durable", "tests/test_handler.py")?;
    if let Some(object) = record.as_object_mut() {
        object.insert("durable_attempt_id".to_string(), json!("my-attempt-1"));
    }
    expect_rejection(&record, &fixture, "durable attempt id must be")
}

#[test]
fn rejects_malformed_apply_block() -> Result<(), String> {
    let fixture = build_fixture(selection_row(
        "att-cage",
        "tests/test_handler.py",
        "existing",
    ))?;
    for (mutation, needle) in [
        ("cage", "unknown edit-cage decision"),
        ("patch", "must be bare lowercase sha256 hex"),
        ("head", "must be a bare lowercase 40-character"),
    ] {
        let mut record = apply_record(&fixture, "att-cage", "tests/test_handler.py")?;
        if let Some(object) = record.as_object_mut()
            && let Some(apply) = object.get_mut("apply").and_then(Value::as_object_mut)
        {
            match mutation {
                "cage" => {
                    apply.insert("cage_status".to_string(), json!("looks_fine"));
                }
                "patch" => {
                    apply.insert("patch_sha256".to_string(), json!("sha256:deadbeef"));
                }
                _ => {
                    apply.insert("repository_head_after".to_string(), json!("deadbeef"));
                }
            }
        }
        expect_rejection(&record, &fixture, needle)?;
    }
    Ok(())
}

#[test]
fn rejects_dropped_extra_or_non_string_non_claims() -> Result<(), String> {
    let fixture = build_fixture(selection_row(
        "att-claims",
        "tests/test_handler.py",
        "existing",
    ))?;
    // A dropped standing non-claim fails.
    let mut dropped = prepare_record(&fixture, "att-claims", "tests/test_handler.py")?;
    if let Some(object) = dropped.as_object_mut() {
        object.insert(
            "non_claims".to_string(),
            json!(["no verification result is claimed by the driver"]),
        );
    }
    expect_rejection(
        &dropped,
        &fixture,
        "must be exactly the standing non-claims",
    )?;
    // An extra string can smuggle a claim and fails too.
    let mut extra = prepare_record(&fixture, "att-claims", "tests/test_handler.py")?;
    let mut entries = BINDING_NON_CLAIMS
        .iter()
        .map(|value| json!(value))
        .collect::<Vec<_>>();
    entries.push(json!("repair verified"));
    if let Some(object) = extra.as_object_mut() {
        object.insert("non_claims".to_string(), json!(entries));
    }
    expect_rejection(&extra, &fixture, "must be exactly the standing non-claims")?;
    // A non-string element is a typed failure, never a silent skip.
    let mut non_string = prepare_record(&fixture, "att-claims", "tests/test_handler.py")?;
    if let Some(object) = non_string.as_object_mut() {
        object.insert(
            "non_claims".to_string(),
            json!(["no closure is claimed by the driver", 1]),
        );
    }
    expect_rejection(&non_string, &fixture, "non-claim must be a string")
}

#[test]
fn rejects_rewritten_identity_copies() -> Result<(), String> {
    // The record can rewrite a copied identity while both digest anchors
    // stay valid; the row agreement check must reject each rewrite.
    let fixture = build_fixture(selection_row(
        "att-identity",
        "tests/test_handler.py",
        "existing",
    ))?;
    for (field, value, needle) in [
        (
            "owner",
            json!("other.handler"),
            "identity disagreement: the record names `other.handler`",
        ),
        (
            "case_id",
            json!("case-rewritten"),
            "identity disagreement: the record names `case-rewritten`",
        ),
        (
            "head",
            json!("dddddddddddddddddddddddddddddddddddddddd"),
            "identity disagreement",
        ),
    ] {
        let mut record = prepare_record(&fixture, "att-identity", "tests/test_handler.py")?;
        set_trust_field(&mut record, field, value);
        expect_rejection(&record, &fixture, needle)?;
    }
    // An optional identity that silently appears or disappears fails.
    let mut removed_tree = prepare_record(&fixture, "att-identity", "tests/test_handler.py")?;
    if let Some(object) = removed_tree.as_object_mut()
        && let Some(trust) = object.get_mut("trust").and_then(Value::as_object_mut)
    {
        trust.remove("tree");
    }
    expect_rejection(&removed_tree, &fixture, "optional identity copy")?;
    Ok(())
}

#[test]
fn normalizes_raw_target_path_spellings() -> Result<(), String> {
    // `./tests/x.rs` and `tests//x.rs` bind identically to `tests/x.rs`:
    // the normalized form is what is compared and recorded.
    for spelling in ["./tests/test_handler.py", "tests//test_handler.py"] {
        let fixture = build_fixture(selection_row("att-spelling", spelling, "existing"))?;
        let mut record = prepare_record(&fixture, "att-spelling", spelling)?;
        if let Some(object) = record.as_object_mut()
            && let Some(edit_surface) = object
                .get_mut("edit_surface")
                .and_then(Value::as_object_mut)
        {
            edit_surface.insert("allowed".to_string(), json!([spelling]));
        }
        let validated = checked(&record, &fixture)?;
        if validated.target_path != "tests/test_handler.py" {
            return Err(format!(
                "raw spelling `{spelling}` was not normalized: `{}`",
                validated.target_path
            ));
        }
    }
    Ok(())
}

#[test]
fn rejects_compliant_apply_that_omits_the_selected_target() -> Result<(), String> {
    let fixture = build_fixture(selection_row(
        "att-omit",
        "tests/test_handler.py",
        "existing",
    ))?;
    // An empty changed set is not an applied edit.
    let mut empty = apply_record(&fixture, "att-omit", "tests/test_handler.py")?;
    if let Some(object) = empty.as_object_mut()
        && let Some(apply) = object.get_mut("apply").and_then(Value::as_object_mut)
    {
        apply.insert("changed_paths".to_string(), json!([]));
    }
    expect_rejection(
        &empty,
        &fixture,
        "must include the selected target `tests/test_handler.py`",
    )?;
    // An unrelated tests/ path inside the declared surface still omits
    // the selected target.
    let mut unrelated = apply_record(&fixture, "att-omit", "tests/test_handler.py")?;
    if let Some(object) = unrelated.as_object_mut() {
        if let Some(apply) = object.get_mut("apply").and_then(Value::as_object_mut) {
            apply.insert("changed_paths".to_string(), json!(["tests/other_test.py"]));
        }
        if let Some(edit_surface) = object
            .get_mut("edit_surface")
            .and_then(Value::as_object_mut)
        {
            edit_surface.insert(
                "allowed".to_string(),
                json!(["tests/test_handler.py", "tests/other_test.py"]),
            );
        }
    }
    expect_rejection(
        &unrelated,
        &fixture,
        "must include the selected target `tests/test_handler.py`",
    )
}

#[test]
fn rejects_cage_violating_changed_paths() -> Result<(), String> {
    let fixture = build_fixture(selection_row(
        "att-cage-paths",
        "tests/test_handler.py",
        "existing",
    ))?;
    for (changed, needle) in [
        (
            "vendor/lib.py",
            "production/generated/vendor/environment edit surface",
        ),
        (
            "generated/helper.py",
            "production/generated/vendor/environment edit surface",
        ),
        ("src/lib.py", "outside the declared allowed edit surface"),
        (
            "tests/undeclared_test.py",
            "outside the declared allowed edit surface",
        ),
    ] {
        let mut record = apply_record(&fixture, "att-cage-paths", "tests/test_handler.py")?;
        if let Some(object) = record.as_object_mut()
            && let Some(apply) = object.get_mut("apply").and_then(Value::as_object_mut)
        {
            apply.insert(
                "changed_paths".to_string(),
                json!(["tests/test_handler.py", changed]),
            );
        }
        expect_rejection(&record, &fixture, needle)?;
    }
    // A declared forbidden path fails even when it is also listed as
    // allowed.
    let mut record = apply_record(&fixture, "att-cage-paths", "tests/test_handler.py")?;
    if let Some(object) = record.as_object_mut() {
        if let Some(apply) = object.get_mut("apply").and_then(Value::as_object_mut) {
            apply.insert(
                "changed_paths".to_string(),
                json!(["tests/test_handler.py", "tests/scratch.py"]),
            );
        }
        if let Some(edit_surface) = object
            .get_mut("edit_surface")
            .and_then(Value::as_object_mut)
        {
            edit_surface.insert(
                "allowed".to_string(),
                json!(["tests/test_handler.py", "tests/scratch.py"]),
            );
            edit_surface.insert("forbidden".to_string(), json!(["tests/scratch.py"]));
        }
    }
    expect_rejection(&record, &fixture, "declared forbidden edit surface")
}

#[test]
fn rejects_lifecycle_vocabulary_on_driver_records() -> Result<(), String> {
    // A driver record can never appear completed: lifecycle, movement, and
    // execution fields are unknown fields and fail.
    let fixture = build_fixture(selection_row(
        "att-vocab",
        "tests/test_handler.py",
        "existing",
    ))?;
    for field in ["states", "movement", "execution"] {
        let mut record = prepare_record(&fixture, "att-vocab", "tests/test_handler.py")?;
        if let Some(object) = record.as_object_mut() {
            object.insert(field.to_string(), json!(["selected"]));
        }
        expect_rejection(&record, &fixture, "unknown field")?;
    }
    Ok(())
}

#[test]
fn end_to_end_check_driver_over_files_and_directory() -> Result<(), String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("clock before epoch: {error}"))?
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "ripr-python-repair-driver-check-{}-{stamp}",
        std::process::id()
    ));
    let manifest_dir = root.join("corpus");
    let bindings_dir = root.join("bindings");
    std::fs::create_dir_all(&manifest_dir).map_err(|error| format!("create corpus: {error}"))?;
    std::fs::create_dir_all(&bindings_dir).map_err(|error| format!("create bindings: {error}"))?;

    let fixture = build_fixture(selection_row(
        "att-e2e",
        "tests/test_handler.py",
        "existing",
    ))?;
    let manifest_path = manifest_dir.join("manifest.json");
    std::fs::write(&manifest_path, &fixture.text)
        .map_err(|error| format!("write manifest: {error}"))?;
    // The apply record chains to the supplied prepare record by its
    // exact-byte digest: the intact chain validates.
    let prepare = prepare_record(&fixture, "att-e2e", "tests/test_handler.py")?;
    let prepare_text = serde_json::to_string_pretty(&prepare)
        .map_err(|error| format!("serialize prepare record: {error}"))?;
    let prepare_bytes = format!("{prepare_text}\n");
    std::fs::write(bindings_dir.join("prepare.json"), &prepare_bytes)
        .map_err(|error| format!("write prepare record: {error}"))?;
    let prepare_digest = sha256_hex(prepare_bytes.as_bytes());
    let apply = apply_record(&fixture, "att-e2e", "tests/test_handler.py")?;
    let chained = clone_with_binding_digest(&apply, &prepare_digest)?;
    let apply_text = serde_json::to_string_pretty(&chained)
        .map_err(|error| format!("serialize apply record: {error}"))?;
    std::fs::write(bindings_dir.join("apply.json"), &apply_text)
        .map_err(|error| format!("write apply record: {error}"))?;

    // The directory input validates every record and exits clean.
    run_check_driver(&args_of(
        manifest_path.to_string_lossy().as_ref(),
        bindings_dir.to_string_lossy().as_ref(),
    ))?;

    // A record that no longer matches the accepted manifest fails.
    let stale = json!({
        "schema_version": BINDING_SCHEMA_VERSION,
        "kind": BINDING_KIND,
        "spec": BINDING_SPEC,
        "phase": "prepare",
        "seam_id": "seam-under-test",
        "repository_head": GIT_SHA_C,
        "selection_manifest_path": "target/ripr/trust-manifest.json",
        "driver": {"binary_sha256": DIGEST_ONE, "version": "0.11.0"},
        "config": {"profile": "default"},
        "input": {"packet_sha256": DIGEST_ONE, "before_snapshot_sha256": DIGEST_TWO},
        "trust": {
            "attempt_id": "att-e2e",
            "case_id": "case-att-e2e",
            "subject_id": "subj-att-e2e",
            "repository": "https://example.com/repo",
            "base": GIT_SHA_B,
            "head": GIT_SHA_C,
            "tree": DIGEST_ONE,
            "source_currentness": "candidate_current",
            "selection_manifest_sha256": DIGEST_ONE,
            "selection_digest": DIGEST_TWO,
            "target_path": "tests/test_handler.py",
            "target_state": "existing",
            "family": "error_path_gating",
            "owner": "module.handler",
            "discriminator": "raises ValueError on empty payload",
            "relation": "case calls owner directly",
            "oracle": "pytest.raises exact message pin",
        },
        "edit_surface": {"allowed": ["tests/test_handler.py"], "forbidden": []},
        "authorization": {
            "status": "granted",
            "authority": "operator-a",
            "method": "explicit-operator-flags",
        },
        "non_claims": BINDING_NON_CLAIMS,
    });
    let stale_text = serde_json::to_string_pretty(&stale)
        .map_err(|error| format!("serialize stale record: {error}"))?;
    std::fs::write(bindings_dir.join("stale.json"), &stale_text)
        .map_err(|error| format!("write stale record: {error}"))?;
    let failure = run_check_driver(&args_of(
        manifest_path.to_string_lossy().as_ref(),
        bindings_dir.to_string_lossy().as_ref(),
    ));
    std::fs::remove_file(bindings_dir.join("stale.json"))
        .map_err(|error| format!("remove stale record: {error}"))?;
    let error = match failure {
        Err(error) => error,
        Ok(()) => return Err("a stale binding record passed the end-to-end check".to_string()),
    };
    if !error.contains("1 violation(s)") {
        return Err(format!("unexpected end-to-end failure: {error}"));
    }

    std::fs::remove_dir_all(&root).map_err(|error| format!("remove temp root: {error}"))?;
    Ok(())
}

#[test]
fn invalid_only_bindings_fail_closed_with_a_failure_verdict() -> Result<(), String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("clock before epoch: {error}"))?
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "ripr-python-repair-driver-invalid-only-{}-{stamp}",
        std::process::id()
    ));
    let manifest_dir = root.join("corpus");
    let bindings_dir = root.join("bindings");
    std::fs::create_dir_all(&manifest_dir).map_err(|error| format!("create corpus: {error}"))?;
    std::fs::create_dir_all(&bindings_dir).map_err(|error| format!("create bindings: {error}"))?;

    let fixture = build_fixture(selection_row(
        "att-invalid",
        "tests/test_handler.py",
        "existing",
    ))?;
    let manifest_path = manifest_dir.join("manifest.json");
    std::fs::write(&manifest_path, &fixture.text)
        .map_err(|error| format!("write manifest: {error}"))?;
    // The only supplied record is invalid (fabricated digest anchors): an
    // all-invalid set must be verdict `inconsistent` with a nonzero exit,
    // never `not_run`/exit 0.
    let stale = json!({
        "schema_version": BINDING_SCHEMA_VERSION,
        "kind": BINDING_KIND,
        "spec": BINDING_SPEC,
        "phase": "prepare",
        "seam_id": "seam-under-test",
        "repository_head": GIT_SHA_C,
        "selection_manifest_path": "target/ripr/trust-manifest.json",
        "driver": {"binary_sha256": DIGEST_ONE, "version": "0.11.0"},
        "config": {"profile": "default"},
        "input": {"packet_sha256": DIGEST_ONE, "before_snapshot_sha256": DIGEST_TWO},
        "trust": {
            "attempt_id": "att-invalid",
            "case_id": "case-att-invalid",
            "subject_id": "subj-att-invalid",
            "repository": "https://example.com/repo",
            "base": GIT_SHA_B,
            "head": GIT_SHA_C,
            "tree": DIGEST_ONE,
            "source_currentness": "candidate_current",
            "selection_manifest_sha256": DIGEST_ONE,
            "selection_digest": DIGEST_TWO,
            "target_path": "tests/test_handler.py",
            "target_state": "existing",
            "family": "error_path_gating",
            "owner": "module.handler",
            "discriminator": "raises ValueError on empty payload",
            "relation": "case calls owner directly",
            "oracle": "pytest.raises exact message pin",
        },
        "edit_surface": {"allowed": ["tests/test_handler.py"], "forbidden": []},
        "authorization": {
            "status": "granted",
            "authority": "operator-a",
            "method": "explicit-operator-flags",
        },
        "non_claims": BINDING_NON_CLAIMS,
    });
    let stale_text = serde_json::to_string_pretty(&stale)
        .map_err(|error| format!("serialize invalid record: {error}"))?;
    std::fs::write(bindings_dir.join("invalid.json"), &stale_text)
        .map_err(|error| format!("write invalid record: {error}"))?;

    let outcome = check_driver_artifacts(
        manifest_path.to_string_lossy().as_ref(),
        bindings_dir.to_string_lossy().as_ref(),
    )?;
    if outcome.verdict() != "inconsistent" {
        return Err(format!(
            "an all-invalid binding set was verdict `{}`, expected `inconsistent`",
            outcome.verdict()
        ));
    }
    if !outcome.records.is_empty() || outcome.violations.len() != 1 {
        return Err(format!(
            "unexpected invalid-only outcome: {} record(s), {} violation(s)",
            outcome.records.len(),
            outcome.violations.len()
        ));
    }
    let failure = run_check_driver(&args_of(
        manifest_path.to_string_lossy().as_ref(),
        bindings_dir.to_string_lossy().as_ref(),
    ));
    let error = match failure {
        Err(error) => error,
        Ok(()) => return Err("an all-invalid binding set exited 0".to_string()),
    };
    if !error.contains("1 violation(s)") {
        return Err(format!("unexpected invalid-only failure: {error}"));
    }
    std::fs::remove_dir_all(&root).map_err(|error| format!("remove temp root: {error}"))?;
    Ok(())
}

#[test]
fn prepare_to_apply_digest_chain_is_enforced() -> Result<(), String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("clock before epoch: {error}"))?
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "ripr-python-repair-driver-chain-{}-{stamp}",
        std::process::id()
    ));
    let manifest_dir = root.join("corpus");
    let bindings_dir = root.join("bindings");
    std::fs::create_dir_all(&manifest_dir).map_err(|error| format!("create corpus: {error}"))?;
    std::fs::create_dir_all(&bindings_dir).map_err(|error| format!("create bindings: {error}"))?;

    let fixture = build_fixture(selection_row(
        "att-chain",
        "tests/test_handler.py",
        "existing",
    ))?;
    let manifest_path = manifest_dir.join("manifest.json");
    std::fs::write(&manifest_path, &fixture.text)
        .map_err(|error| format!("write manifest: {error}"))?;
    let prepare = prepare_record(&fixture, "att-chain", "tests/test_handler.py")?;
    let prepare_text = serde_json::to_string_pretty(&prepare)
        .map_err(|error| format!("serialize prepare record: {error}"))?;
    let prepare_bytes = format!("{prepare_text}\n");
    let prepare_digest = sha256_hex(prepare_bytes.as_bytes());
    std::fs::write(bindings_dir.join("prepare.json"), &prepare_bytes)
        .map_err(|error| format!("write prepare record: {error}"))?;
    let apply = apply_record(&fixture, "att-chain", "tests/test_handler.py")?;

    // A fabricated digest (no supplied prepare artifact carries it)
    // breaks the chain and fails.
    let fabricated = clone_with_binding_digest(&apply, DIGEST_ONE)?;
    let fabricated_text = serde_json::to_string_pretty(&fabricated)
        .map_err(|error| format!("serialize fabricated apply: {error}"))?;
    std::fs::write(bindings_dir.join("apply.json"), &fabricated_text)
        .map_err(|error| format!("write fabricated apply: {error}"))?;
    let outcome = check_driver_artifacts(
        manifest_path.to_string_lossy().as_ref(),
        bindings_dir.to_string_lossy().as_ref(),
    )?;
    if outcome.violations.len() != 1
        || !outcome.violations[0].contains("prepare-to-apply digest chain broken")
    {
        return Err(format!(
            "a fabricated binding digest did not break the chain: {:?}",
            outcome.violations
        ));
    }
    if run_check_driver(&args_of(
        manifest_path.to_string_lossy().as_ref(),
        bindings_dir.to_string_lossy().as_ref(),
    ))
    .is_ok()
    {
        return Err("a fabricated binding digest exited 0".to_string());
    }

    // The intact chain passes.
    let chained = clone_with_binding_digest(&apply, &prepare_digest)?;
    let chained_text = serde_json::to_string_pretty(&chained)
        .map_err(|error| format!("serialize chained apply: {error}"))?;
    std::fs::write(bindings_dir.join("apply.json"), &chained_text)
        .map_err(|error| format!("write chained apply: {error}"))?;
    run_check_driver(&args_of(
        manifest_path.to_string_lossy().as_ref(),
        bindings_dir.to_string_lossy().as_ref(),
    ))?;

    // A tampered prepare artifact moves its recomputed digest away from
    // the apply record's claim and fails the chain.
    let tampered_bytes = prepare_bytes.replace("operator-a", "operator-tampered");
    if tampered_bytes == prepare_bytes {
        return Err("tampering did not change the prepare bytes".to_string());
    }
    std::fs::write(bindings_dir.join("prepare.json"), &tampered_bytes)
        .map_err(|error| format!("write tampered prepare: {error}"))?;
    let outcome = check_driver_artifacts(
        manifest_path.to_string_lossy().as_ref(),
        bindings_dir.to_string_lossy().as_ref(),
    )?;
    if outcome.violations.len() != 1
        || !outcome.violations[0].contains("prepare-to-apply digest chain broken")
    {
        return Err(format!(
            "a tampered prepare artifact did not break the chain: {:?}",
            outcome.violations
        ));
    }
    std::fs::write(bindings_dir.join("prepare.json"), &prepare_bytes)
        .map_err(|error| format!("write prepare record: {error}"))?;

    // Duplicate prepare records sharing one digest make the reference
    // ambiguous and fail.
    std::fs::write(bindings_dir.join("prepare-duplicate.json"), &prepare_bytes)
        .map_err(|error| format!("write duplicate prepare: {error}"))?;
    let outcome = check_driver_artifacts(
        manifest_path.to_string_lossy().as_ref(),
        bindings_dir.to_string_lossy().as_ref(),
    )?;
    std::fs::remove_file(bindings_dir.join("prepare-duplicate.json"))
        .map_err(|error| format!("remove duplicate prepare: {error}"))?;
    if outcome.violations.len() != 1
        || !outcome.violations[0].contains("ambiguous prepare reference")
    {
        return Err(format!(
            "a duplicated prepare record was not ambiguous: {:?}",
            outcome.violations
        ));
    }

    std::fs::remove_dir_all(&root).map_err(|error| format!("remove temp root: {error}"))?;
    Ok(())
}

/// Copies one record and replaces its `binding_artifact_sha256` claim.
fn clone_with_binding_digest(record: &Value, digest: &str) -> Result<Value, String> {
    let mut copy = record.clone();
    let object = copy
        .as_object_mut()
        .ok_or_else(|| "apply record is not an object".to_string())?;
    object.insert("binding_artifact_sha256".to_string(), json!(digest));
    Ok(copy)
}

/// A two-row manifest fixture so an apply can reference another attempt's
/// prepare record.
fn build_two_row_fixture() -> Result<Fixture, String> {
    let mut rows = Vec::new();
    for attempt in ["att-one", "att-two"] {
        let mut row = json!({
            "attempt_id": attempt,
            "case_id": format!("case-{attempt}"),
            "subject_id": format!("subj-{attempt}"),
            "repository": "https://example.com/repo",
            "base": GIT_SHA_B,
            "head": GIT_SHA_C,
            "tree": DIGEST_ONE,
            "source_currentness": "candidate_current",
            "selection_reason": "behavior changed in the diff and the case discriminates it",
            "diversity_stratum": "pytest_library",
            "family": "error_path_gating",
            "owner": "module.handler",
            "discriminator": "raises ValueError on empty payload",
            "relation": "case calls owner directly",
            "oracle": "pytest.raises exact message pin",
            "expected_direction": "should_gap",
            "claim_boundary": "static exposure evidence only",
            "target_path": "tests/test_handler.py",
            "target_state": "existing",
            "selected_at": "2026-09-10T00:00:00Z",
            "selector": "campaign-selector",
            "authority_snapshot_digest": DIGEST_TWO,
        });
        if let Some(object) = row.as_object_mut() {
            let digest = canonical_selection_digest(object, attempt)?;
            object.insert("selection_digest".to_string(), json!(digest));
        }
        rows.push(row);
    }
    let manifest = json!({
        "schema_version": SCHEMA_VERSION,
        "kind": MANIFEST_KIND,
        "spec": KNOWN_SPEC,
        "description": "driver binding two-row fixture",
        "selections": rows,
    });
    let text = serde_json::to_string_pretty(&manifest)
        .map_err(|error| format!("serialize two-row manifest: {error}"))?;
    let sha256 = sha256_hex(text.as_bytes());
    let value = serde_json::from_str(&text).map_err(|error| format!("reparse two-row: {error}"))?;
    Ok(Fixture {
        text,
        sha256,
        value,
    })
}

/// The recomputed canonical digest of one row, selected by attempt
/// identity (the shared `row_digest` helper reads only the first row).
fn row_digest_by_attempt(fixture: &Fixture, attempt_id: &str) -> Result<String, String> {
    let rows = fixture
        .value
        .get("selections")
        .and_then(Value::as_array)
        .ok_or("two-row fixture carries no selections")?;
    let row = rows
        .iter()
        .find(|row| row.get("attempt_id").and_then(Value::as_str) == Some(attempt_id))
        .ok_or_else(|| format!("two-row fixture carries no row for `{attempt_id}`"))?;
    let canonical = row.as_object().cloned().ok_or("row is not an object")?;
    canonical_selection_digest(&canonical, attempt_id)
}

#[test]
fn prepare_to_apply_chain_requires_identity_agreement() -> Result<(), String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("clock before epoch: {error}"))?
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "ripr-python-repair-driver-chain-identity-{}-{stamp}",
        std::process::id()
    ));
    let manifest_dir = root.join("corpus");
    let bindings_dir = root.join("bindings");
    std::fs::create_dir_all(&manifest_dir).map_err(|error| format!("create corpus: {error}"))?;
    std::fs::create_dir_all(&bindings_dir).map_err(|error| format!("create bindings: {error}"))?;

    let fixture_one = build_two_row_fixture()?;
    let manifest_path = manifest_dir.join("manifest.json");
    std::fs::write(&manifest_path, &fixture_one.text)
        .map_err(|error| format!("write manifest: {error}"))?;

    // Prepare records for BOTH attempts, plus a valid apply for att-one.
    let prepare_one = prepare_record(&fixture_one, "att-one", "tests/test_handler.py")?;
    let mut prepare_two = prepare_record(&fixture_one, "att-one", "tests/test_handler.py")?;
    set_trust_field(&mut prepare_two, "attempt_id", json!("att-two"));
    set_trust_field(&mut prepare_two, "case_id", json!("case-att-two"));
    set_trust_field(&mut prepare_two, "subject_id", json!("subj-att-two"));
    set_trust_field(
        &mut prepare_two,
        "selection_digest",
        json!(row_digest_by_attempt(&fixture_one, "att-two")?),
    );
    let write_record = |name: &str, record: &Value| -> Result<String, String> {
        let text = serde_json::to_string_pretty(record)
            .map_err(|error| format!("serialize {name}: {error}"))?;
        let bytes = format!("{text}\n");
        std::fs::write(bindings_dir.join(name), &bytes)
            .map_err(|error| format!("write {name}: {error}"))?;
        Ok(sha256_hex(bytes.as_bytes()))
    };
    let digest_two = write_record("prepare-two.json", &prepare_two)?;
    write_record("prepare-one.json", &prepare_one)?;

    // Cross-attempt digest reference: the att-one apply claims
    // prepare-two's exact digest. Both records validate individually, so
    // only the identity agreement can catch the splice.
    let apply_one = apply_record(&fixture_one, "att-one", "tests/test_handler.py")?;
    let spliced = clone_with_binding_digest(&apply_one, &digest_two)?;
    let spliced_text = serde_json::to_string_pretty(&spliced)
        .map_err(|error| format!("serialize spliced apply: {error}"))?;
    std::fs::write(bindings_dir.join("apply.json"), &spliced_text)
        .map_err(|error| format!("write spliced apply: {error}"))?;
    let outcome = check_driver_artifacts(
        manifest_path.to_string_lossy().as_ref(),
        bindings_dir.to_string_lossy().as_ref(),
    )?;
    // The splice disagrees on every identity that differs between the two
    // attempts (the attempt id and the row digest); every violation must
    // be an identity disagreement naming both records and its field.
    if outcome.violations.is_empty()
        || !outcome.violations.iter().all(|violation| {
            violation.contains("prepare-to-apply identity disagreement")
                && violation.contains("apply.json")
                && violation.contains("->")
                && violation.contains("prepare-two.json")
        })
        || !outcome
            .violations
            .iter()
            .any(|violation| violation.contains("field=`trust.attempt_id`"))
    {
        return Err(format!(
            "a cross-attempt prepare reference did not fail on identity: {:?}",
            outcome.violations
        ));
    }

    // A modified edit surface on an otherwise intact chain fails too.
    let mut modified = apply_record(&fixture_one, "att-one", "tests/test_handler.py")?;
    if let Some(object) = modified.as_object_mut()
        && let Some(edit_surface) = object
            .get_mut("edit_surface")
            .and_then(Value::as_object_mut)
    {
        edit_surface.insert(
            "allowed".to_string(),
            json!(["tests/test_handler.py", "tests/extra_test.py"]),
        );
    }
    let digest_one = sha256_hex(
        format!(
            "{}\n",
            serde_json::to_string_pretty(&prepare_one)
                .map_err(|error| format!("re-serialize prepare-one: {error}"))?
        )
        .as_bytes(),
    );
    let chained = clone_with_binding_digest(&modified, &digest_one)?;
    let chained_text = serde_json::to_string_pretty(&chained)
        .map_err(|error| format!("serialize surface-drift apply: {error}"))?;
    std::fs::write(bindings_dir.join("apply.json"), &chained_text)
        .map_err(|error| format!("write surface-drift apply: {error}"))?;
    let outcome = check_driver_artifacts(
        manifest_path.to_string_lossy().as_ref(),
        bindings_dir.to_string_lossy().as_ref(),
    )?;
    if outcome.violations.len() != 1
        || !outcome.violations[0].contains("field=`edit_surface.allowed`")
    {
        return Err(format!(
            "a modified edit surface did not break the chain: {:?}",
            outcome.violations
        ));
    }

    // The matching chain (same attempt, same identities) passes.
    let intact = clone_with_binding_digest(&apply_one, &digest_one)?;
    let intact_text = serde_json::to_string_pretty(&intact)
        .map_err(|error| format!("serialize intact apply: {error}"))?;
    std::fs::write(bindings_dir.join("apply.json"), &intact_text)
        .map_err(|error| format!("write intact apply: {error}"))?;
    let outcome = check_driver_artifacts(
        manifest_path.to_string_lossy().as_ref(),
        bindings_dir.to_string_lossy().as_ref(),
    )?;
    if !outcome.violations.is_empty() || outcome.verdict() != "valid" {
        return Err(format!(
            "a matching prepare-to-apply chain was rejected: {:?}",
            outcome.violations
        ));
    }

    std::fs::remove_dir_all(&root).map_err(|error| format!("remove temp root: {error}"))?;
    Ok(())
}

#[test]
fn violated_apply_record_validates_as_typed_failed_result() -> Result<(), String> {
    let fixture = build_fixture(selection_row(
        "att-violated",
        "tests/test_handler.py",
        "existing",
    ))?;
    // The producer's typed failed result: the edit escaped the cage, and
    // the record retains the escaped paths as evidence. The offline
    // validator accepts it (structure validated) instead of rejecting the
    // only durable failure record.
    let mut violated = apply_record(&fixture, "att-violated", "tests/test_handler.py")?;
    if let Some(object) = violated.as_object_mut()
        && let Some(apply) = object.get_mut("apply").and_then(Value::as_object_mut)
    {
        apply.insert(
            "changed_paths".to_string(),
            json!(["tests/test_handler.py", "vendor/lib.py", "src/smuggled.py"]),
        );
        apply.insert("cage_status".to_string(), json!("violated"));
    }
    let validated = checked(&violated, &fixture)?;
    if validated.cage_status.as_deref() != Some("violated") {
        return Err("the violated disposition was not carried".to_string());
    }
    // A non-portable escaped path still fails structurally: the
    // acceptance covers cage membership, not path hygiene.
    let mut malformed = violated.clone();
    if let Some(object) = malformed.as_object_mut()
        && let Some(apply) = object.get_mut("apply").and_then(Value::as_object_mut)
    {
        apply.insert("changed_paths".to_string(), json!(["../outside.py"]));
    }
    expect_rejection(&malformed, &fixture, "must not contain `..` components")?;
    Ok(())
}

fn args_of(manifest: &str, bindings: &str) -> Vec<String> {
    vec![
        "--manifest".to_string(),
        manifest.to_string(),
        "--bindings".to_string(),
        bindings.to_string(),
    ]
}
