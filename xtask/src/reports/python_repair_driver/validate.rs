//! Identity, currentness, and semantic validation of one driver binding
//! record against the accepted selection manifest.
//!
//! Digest anchors are recomputed, closed field sets are deny-unknown, and
//! every claim a record could overstate is pinned by the standing non-claims.
//! Schema key lists and DTO shape stay in `schema`.

use std::collections::BTreeSet;

use serde_json::Value;

use super::super::python_repair_trust::{
    SelectionManifest, canonical_selection_digest, check_git_sha, check_portable_path,
    check_sha256_digest, known_value_or_fail, opt_string, reject_secret_tokens,
    reject_unknown_keys, require_string,
};
use super::schema::{
    APPLY_KEYS, AUTHORIZATION_KEYS, AUTHORIZATION_METHODS, AUTHORIZATION_STATUSES,
    BINDABLE_TARGET_STATE, BINDING_KIND, BINDING_NON_CLAIMS, BINDING_SCHEMA_VERSION, BINDING_SPEC,
    CAGE_STATUSES, CONFIG_KEYS, DRIVER_KEYS, DriverBindingRecord, EDIT_SURFACE_KEYS, INPUT_KEYS,
    RECORD_KEYS_APPLY, RECORD_KEYS_PREPARE, TRUST_KEYS, driver_fail,
};

/// Denied edit-surface prefixes, mirroring the corpus vocabulary in
/// `python_repair_trust` (shared invariant, local matcher by design).
const DENIED_SURFACE_PREFIXES: [&str; 14] = [
    "target/",
    "dist/",
    "build/",
    "vendor/",
    "vendored/",
    "node_modules/",
    "generated/",
    "__pycache__/",
    "site-packages/",
    ".venv/",
    "venv/",
    "env/",
    ".tox/",
    ".eggs/",
];

fn is_denied_edit_surface(path: &str) -> bool {
    let lowered = path.to_ascii_lowercase();
    DENIED_SURFACE_PREFIXES
        .iter()
        .any(|prefix| lowered.starts_with(prefix))
        || lowered
            .split('/')
            .any(|component| component.contains(".generated."))
}

/// Normalizes a portable repo-relative path: `./` segments dropped, empty
/// segments collapsed. The normalized form is what is compared against the
/// accepted row, matched against the edit cage, and recorded, so the raw
/// spellings `./tests/x.rs` and `tests//x.rs` bind identically to
/// `tests/x.rs`. Callers run the portability check on the raw spelling
/// first, so absolute and `..`-carrying paths never reach this function.
fn normalize_repo_relative_path(path: &str) -> String {
    path.split('/')
        .filter(|component| !component.is_empty() && *component != ".")
        .collect::<Vec<_>>()
        .join("/")
}

/// The durable attempt identity shape (#2927): `repair-attempt-` plus 24
/// lowercase hexadecimal characters.
fn check_durable_attempt_id(subject: &str, field: &str, value: &str) -> Result<(), String> {
    let suffix = value.strip_prefix("repair-attempt-");
    let shaped = match suffix {
        Some(suffix) => {
            suffix.len() == 24
                && suffix
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        }
        None => false,
    };
    if shaped {
        Ok(())
    } else {
        Err(driver_fail(
            subject,
            field,
            "durable attempt id must be `repair-attempt-` plus 24 lowercase hexadecimal characters",
        ))
    }
}

/// Validates one driver binding record against the accepted selection
/// manifest. Every digest anchor is recomputed, every closed field set is
/// deny-unknown, and every claim the record could overstate is pinned by the
/// non-claims list.
pub(crate) fn validate_binding_record(
    display: &str,
    record: &Value,
    manifest: &SelectionManifest,
    manifest_sha256: &str,
    artifact_sha256: &str,
) -> Result<DriverBindingRecord, String> {
    let top = record.as_object().ok_or_else(|| {
        driver_fail(
            display,
            "record",
            "driver binding record must be a JSON object",
        )
    })?;
    let phase_value = require_string(display, top, "phase")?;
    let apply_phase = match phase_value.as_str() {
        "prepare" => false,
        "apply" => true,
        other => {
            return Err(driver_fail(
                display,
                "phase",
                format!("unknown phase `{other}`; a driver record is `prepare` or `apply`"),
            ));
        }
    };
    let allowed_keys: &[&str] = if apply_phase {
        &RECORD_KEYS_APPLY
    } else {
        &RECORD_KEYS_PREPARE
    };
    reject_unknown_keys(top, allowed_keys, display, "driver binding record")?;
    for (field, expected) in [
        ("schema_version", BINDING_SCHEMA_VERSION),
        ("kind", BINDING_KIND),
        ("spec", BINDING_SPEC),
    ] {
        let actual = require_string(display, top, field)?;
        if actual != expected {
            return Err(driver_fail(
                display,
                field,
                format!("expected `{expected}`, got `{actual}`"),
            ));
        }
    }
    let record_seam_id = require_string(display, top, "seam_id")?;
    let repository_head = require_string(display, top, "repository_head")?;
    check_git_sha(display, "repository_head", &repository_head)?;
    require_string(display, top, "selection_manifest_path")?;

    let mut binding_artifact_sha256 = None;
    if apply_phase {
        let durable = require_string(display, top, "durable_attempt_id")?;
        check_durable_attempt_id(display, "durable_attempt_id", &durable)?;
        let binding_digest = require_string(display, top, "binding_artifact_sha256")?;
        check_sha256_digest(display, "binding_artifact_sha256", &binding_digest)?;
        binding_artifact_sha256 = Some(binding_digest);
    }

    // Driver identity: the running binary digest and version, recorded from
    // the real producer, never a fabricated pin.
    let driver = match top.get("driver") {
        Some(Value::Object(map)) => map,
        _ => {
            return Err(driver_fail(
                display,
                "driver",
                "`driver` identity block must be an object",
            ));
        }
    };
    reject_unknown_keys(driver, &DRIVER_KEYS, display, "`driver` identity block")?;
    let driver_binary = require_string(display, driver, "binary_sha256")?;
    check_sha256_digest(display, "driver.binary_sha256", &driver_binary)?;
    require_string(display, driver, "version")?;

    let config = match top.get("config") {
        Some(Value::Object(map)) => map,
        _ => {
            return Err(driver_fail(
                display,
                "config",
                "`config` identity block must be an object",
            ));
        }
    };
    reject_unknown_keys(config, &CONFIG_KEYS, display, "`config` identity block")?;
    require_string(display, config, "profile")?;

    let input = match top.get("input") {
        Some(Value::Object(map)) => map,
        _ => {
            return Err(driver_fail(
                display,
                "input",
                "`input` identity block must be an object",
            ));
        }
    };
    reject_unknown_keys(input, &INPUT_KEYS, display, "`input` identity block")?;
    let packet_digest = require_string(display, input, "packet_sha256")?;
    check_sha256_digest(display, "input.packet_sha256", &packet_digest)?;
    let before_digest = require_string(display, input, "before_snapshot_sha256")?;
    check_sha256_digest(display, "input.before_snapshot_sha256", &before_digest)?;

    // Trust block: the digest anchors and the identity agreement with the
    // accepted selection row.
    let trust = match top.get("trust") {
        Some(Value::Object(map)) => map,
        _ => {
            return Err(driver_fail(
                display,
                "trust",
                "`trust` identity block must be an object",
            ));
        }
    };
    reject_unknown_keys(trust, &TRUST_KEYS, display, "`trust` identity block")?;
    let trust_attempt_id = require_string(display, trust, "attempt_id")?;
    let selection = manifest
        .selections
        .iter()
        .find(|selection| selection.attempt_id == trust_attempt_id)
        .ok_or_else(|| {
            driver_fail(
                &trust_attempt_id,
                "trust.attempt_id",
                "names an attempt identity outside the accepted selection denominator; selected rows cannot be substituted by name",
            )
        })?;
    // The accepted row's canonical preimage anchors both the digest recompute
    // and the identity-agreement checks below.
    let row_preimage = manifest
        .row_preimages
        .get(&trust_attempt_id)
        .ok_or_else(|| {
            driver_fail(
                &trust_attempt_id,
                "trust.selection_digest",
                "the accepted selection manifest retained no canonical preimage for this row",
            )
        })?;
    // Identity agreement: every identity the record copies from the selected
    // row must equal the accepted row's value. The selection digest covers
    // the row, so a rewritten copy in the record is rejected even while the
    // digest anchors stay valid.
    for field in [
        "case_id",
        "subject_id",
        "repository",
        "base",
        "head",
        "family",
        "owner",
        "discriminator",
        "relation",
        "oracle",
    ] {
        let record_value = require_string(display, trust, field)?;
        let row_value = row_preimage
            .get(field)
            .and_then(Value::as_str)
            .ok_or_else(|| {
                driver_fail(
                    &trust_attempt_id,
                    &format!("trust.{field}"),
                    format!("the accepted selection row carries no comparable `{field}`"),
                )
            })?;
        if record_value != row_value {
            return Err(driver_fail(
                &trust_attempt_id,
                &format!("trust.{field}"),
                format!(
                    "identity disagreement: the record names `{record_value}` but the accepted selection row names `{row_value}`; a rewritten identity copy is rejected"
                ),
            ));
        }
    }
    for field in ["tree", "source_currentness", "limitation"] {
        // Optional identities must agree in presence and value: an explicit
        // JSON null and an absent field are both "absent".
        let row_present = matches!(row_preimage.get(field), Some(Value::String(_)));
        let record_present = matches!(trust.get(field), Some(Value::String(_)));
        if row_present != record_present
            || (record_present && trust.get(field) != row_preimage.get(field))
        {
            return Err(driver_fail(
                &trust_attempt_id,
                &format!("trust.{field}"),
                "identity disagreement: the record's optional identity copy does not match the accepted selection row; presence and value must agree",
            ));
        }
    }
    check_git_sha(
        display,
        "trust.base",
        &require_string(display, trust, "base")?,
    )?;
    check_git_sha(
        display,
        "trust.head",
        &require_string(display, trust, "head")?,
    )?;
    if let Some(tree) = opt_string(display, trust, "tree")? {
        check_sha256_digest(display, "trust.tree", &tree)?;
    }
    if let Some(currentness) = opt_string(display, trust, "source_currentness")? {
        known_value_or_fail(
            display,
            "trust.source_currentness",
            &currentness,
            &super::super::python_repair_trust::SOURCE_CURRENTNESS,
            "source-currentness disposition",
        )?;
    }
    if let Some(limitation) = opt_string(display, trust, "limitation")?
        && limitation.trim().is_empty()
    {
        return Err(driver_fail(
            display,
            "trust.limitation",
            "must be non-empty when present",
        ));
    }
    let recorded_manifest_digest = require_string(display, trust, "selection_manifest_sha256")?;
    check_sha256_digest(
        display,
        "trust.selection_manifest_sha256",
        &recorded_manifest_digest,
    )?;
    if recorded_manifest_digest != manifest_sha256 {
        return Err(driver_fail(
            &trust_attempt_id,
            "trust.selection_manifest_sha256",
            format!(
                "stale digest: the record binds to manifest {recorded_manifest_digest} but the accepted selection manifest is {manifest_sha256}"
            ),
        ));
    }
    let recorded_selection_digest = require_string(display, trust, "selection_digest")?;
    check_sha256_digest(
        display,
        "trust.selection_digest",
        &recorded_selection_digest,
    )?;
    let raw_target_path = require_string(display, trust, "target_path")?;
    check_portable_path(display, "trust.target_path", &raw_target_path)?;
    // The normalized spelling is what is compared and recorded: a raw
    // `./tests/x.rs` or `tests//x.rs` binds identically to `tests/x.rs`.
    let target_path = normalize_repo_relative_path(&raw_target_path);
    let row_target_path = normalize_repo_relative_path(&selection.target_path);
    let target_state = require_string(display, trust, "target_state")?;
    known_value_or_fail(
        display,
        "trust.target_state",
        &target_state,
        &super::super::python_repair_trust::TARGET_STATES,
        "target state",
    )?;
    if target_state != BINDABLE_TARGET_STATE {
        return Err(driver_fail(
            &trust_attempt_id,
            "trust.target_state",
            format!(
                "driver binding target state must be `{BINDABLE_TARGET_STATE}`, got `{target_state}`; proposed/ambiguous/unavailable/unsafe targets require a new or re-authorized selection"
            ),
        ));
    }
    if target_path != row_target_path || target_state != selection.target_state {
        return Err(driver_fail(
            &trust_attempt_id,
            "trust.target_path",
            format!(
                "target identity disagreement: the record names `{target_path}` (`{target_state}`) but the accepted selection row names `{row_target_path}` (`{}`)",
                selection.target_state
            ),
        ));
    }
    if is_denied_edit_surface(&target_path) {
        return Err(driver_fail(
            &trust_attempt_id,
            "trust.target_path",
            format!(
                "target `{target_path}` falls under a production/generated/vendor/environment edit surface; the driver binds only test-only targets"
            ),
        ));
    }
    // The row digest anchor: recompute the canonical content digest of the
    // retained preimage and require equality, so a replaced or edited
    // selected row fails the record that trusted it.
    let recomputed = canonical_selection_digest(row_preimage, &trust_attempt_id)?;
    if recorded_selection_digest != recomputed {
        return Err(driver_fail(
            &trust_attempt_id,
            "trust.selection_digest",
            format!(
                "selection row digest mismatch: the record pins `{recorded_selection_digest}` but the accepted row now digests to `{recomputed}`; a replaced or edited selected row makes the binding stale"
            ),
        ));
    }

    // Edit surface: the declared cage, portable paths only.
    let edit_surface = match top.get("edit_surface") {
        Some(Value::Object(map)) => map,
        _ => {
            return Err(driver_fail(
                display,
                "edit_surface",
                "`edit_surface` identity block must be an object",
            ));
        }
    };
    reject_unknown_keys(
        edit_surface,
        &EDIT_SURFACE_KEYS,
        display,
        "`edit_surface` block",
    )?;
    let allowed_paths = match edit_surface.get("allowed") {
        Some(Value::Array(values)) => values,
        _ => {
            return Err(driver_fail(
                display,
                "edit_surface.allowed",
                "must be an array of paths",
            ));
        }
    };
    if allowed_paths.is_empty() {
        return Err(driver_fail(
            display,
            "edit_surface.allowed",
            "must name at least one allowed path",
        ));
    }
    let mut allowed_set = BTreeSet::new();
    for (index, value) in allowed_paths.iter().enumerate() {
        let path = value.as_str().ok_or_else(|| {
            driver_fail(
                display,
                &format!("edit_surface.allowed[{index}]"),
                "path must be a string",
            )
        })?;
        check_portable_path(display, &format!("edit_surface.allowed[{index}]"), path)?;
        allowed_set.insert(normalize_repo_relative_path(path));
    }
    let forbidden_paths = match edit_surface.get("forbidden") {
        Some(Value::Array(values)) => values,
        _ => {
            return Err(driver_fail(
                display,
                "edit_surface.forbidden",
                "must be an array of paths",
            ));
        }
    };
    let mut forbidden_set = BTreeSet::new();
    for (index, value) in forbidden_paths.iter().enumerate() {
        let path = value.as_str().ok_or_else(|| {
            driver_fail(
                display,
                &format!("edit_surface.forbidden[{index}]"),
                "path must be a string",
            )
        })?;
        check_portable_path(display, &format!("edit_surface.forbidden[{index}]"), path)?;
        forbidden_set.insert(normalize_repo_relative_path(path));
    }

    // Authorization: the driver applies no edit without the explicit pair,
    // and it never infers one — anything but the granted explicit record
    // fails.
    let authorization = match top.get("authorization") {
        Some(Value::Object(map)) => map,
        _ => {
            return Err(driver_fail(
                display,
                "authorization",
                "`authorization` block must be an object",
            ));
        }
    };
    reject_unknown_keys(
        authorization,
        &AUTHORIZATION_KEYS,
        display,
        "`authorization` block",
    )?;
    let status = require_string(display, authorization, "status")?;
    known_value_or_fail(
        display,
        "authorization.status",
        &status,
        &AUTHORIZATION_STATUSES,
        "authorization status",
    )?;
    let authority = require_string(display, authorization, "authority")?;
    let method = require_string(display, authorization, "method")?;
    known_value_or_fail(
        display,
        "authorization.method",
        &method,
        &AUTHORIZATION_METHODS,
        "authorization method",
    )?;

    // Non-claims: the standing claim boundary must ride on the record
    // verbatim. Every element must be a string and the collection must be
    // EXACTLY the standing list: a dropped non-claim weakens the boundary and
    // an extra string can smuggle a claim.
    let non_claims = match top.get("non_claims") {
        Some(Value::Array(values)) => values,
        _ => {
            return Err(driver_fail(
                display,
                "non_claims",
                "must be an array of non-claim strings",
            ));
        }
    };
    let mut recorded_non_claims = BTreeSet::new();
    for (index, value) in non_claims.iter().enumerate() {
        let text = value.as_str().ok_or_else(|| {
            driver_fail(
                display,
                &format!("non_claims[{index}]"),
                "non-claim must be a string",
            )
        })?;
        recorded_non_claims.insert(text);
    }
    let expected_non_claims: BTreeSet<&str> = BINDING_NON_CLAIMS.into_iter().collect();
    if recorded_non_claims != expected_non_claims {
        return Err(driver_fail(
            display,
            "non_claims",
            format!(
                "must be exactly the standing non-claims {BINDING_NON_CLAIMS:?}; a dropped or extra entry fails"
            ),
        ));
    }

    // Apply block: present exactly on apply-phase records, carrying the
    // applied-edit evidence and nothing beyond it — no verification verdict,
    // no movement, no closure.
    let mut apply_cage_status = None;
    if apply_phase {
        let apply = match top.get("apply") {
            Some(Value::Object(map)) => map,
            _ => {
                return Err(driver_fail(
                    display,
                    "apply",
                    "apply records must carry the `apply` block",
                ));
            }
        };
        reject_unknown_keys(apply, &APPLY_KEYS, display, "`apply` block")?;
        let patch = require_string(display, apply, "patch_sha256")?;
        check_sha256_digest(display, "apply.patch_sha256", &patch)?;
        let changed_paths = match apply.get("changed_paths") {
            Some(Value::Array(values)) => values,
            _ => {
                return Err(driver_fail(
                    display,
                    "apply.changed_paths",
                    "must be an array of paths",
                ));
            }
        };
        let cage_status = require_string(display, apply, "cage_status")?;
        known_value_or_fail(
            display,
            "apply.cage_status",
            &cage_status,
            &CAGE_STATUSES,
            "edit-cage decision",
        )?;
        // The edit cage binds every changed path of a COMPLIANT record: a
        // denied production/generated/vendor/environment surface, a declared
        // forbidden path, or any path outside the declared allowed surface
        // fails. A `violated` record is the producer's typed failed result:
        // the escaped paths ARE the retained evidence, so they are validated
        // structurally (strings, portable spellings) but not against the cage
        // — rejecting them would erase the producer's only durable failure
        // record. `incomparable` records stay outside this offline
        // acceptance: their cage truth lives in the durable attempt authority.
        let violated_cage = cage_status == "violated";
        let mut changed_set = BTreeSet::new();
        for (index, value) in changed_paths.iter().enumerate() {
            let path = value.as_str().ok_or_else(|| {
                driver_fail(
                    display,
                    &format!("apply.changed_paths[{index}]"),
                    "path must be a string",
                )
            })?;
            check_portable_path(display, &format!("apply.changed_paths[{index}]"), path)?;
            let normalized = normalize_repo_relative_path(path);
            if !violated_cage {
                if is_denied_edit_surface(&normalized) {
                    return Err(driver_fail(
                        display,
                        &format!("apply.changed_paths[{index}]"),
                        format!(
                            "changed path `{normalized}` falls under a production/generated/vendor/environment edit surface; the driver records only test-only edits"
                        ),
                    ));
                }
                if forbidden_set.contains(normalized.as_str()) {
                    return Err(driver_fail(
                        display,
                        &format!("apply.changed_paths[{index}]"),
                        format!(
                            "changed path `{normalized}` matches the declared forbidden edit surface"
                        ),
                    ));
                }
                if !allowed_set.contains(normalized.as_str()) {
                    return Err(driver_fail(
                        display,
                        &format!("apply.changed_paths[{index}]"),
                        format!(
                            "changed path `{normalized}` is outside the declared allowed edit surface {:?}",
                            allowed_set
                        ),
                    ));
                }
            }
            changed_set.insert(normalized);
        }
        if cage_status == "compliant" && !changed_set.contains(&row_target_path) {
            return Err(driver_fail(
                &trust_attempt_id,
                "apply.changed_paths",
                format!(
                    "a compliant apply record must include the selected target `{row_target_path}` among its changed paths; the authorized edit moves the selected target, so a compliant record without it records no applied edit"
                ),
            ));
        }
        apply_cage_status = Some(cage_status);
        let head_after = require_string(display, apply, "repository_head_after")?;
        check_git_sha(display, "apply.repository_head_after", &head_after)?;
        if !matches!(apply.get("current"), Some(Value::Bool(_))) {
            return Err(driver_fail(display, "apply.current", "must be a boolean"));
        }
    }

    reject_secret_tokens(record, display, "binding record")?;

    Ok(DriverBindingRecord {
        display: display.to_string(),
        phase: phase_value,
        trust_attempt_id,
        target_path,
        selection_digest: recorded_selection_digest,
        seam_id: record_seam_id,
        repository_head,
        packet_sha256: packet_digest,
        before_snapshot_sha256: before_digest,
        allowed_surface: allowed_set,
        forbidden_surface: forbidden_set,
        authorization_status: status,
        authorization_authority: authority,
        authorization_method: method,
        cage_status: apply_cage_status,
        artifact_sha256: artifact_sha256.to_string(),
        binding_artifact_sha256,
    })
}
