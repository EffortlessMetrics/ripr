//! Row joins, counts, and terminal disposition derivation for the driver
//! binding check.
//!
//! Loads the accepted selection manifest and retained binding files, joins
//! valid records, enforces the prepare-to-apply digest chain, and derives
//! the fail-closed `valid` / `inconsistent` / `not_run` verdict.

use std::path::Path;

use super::super::python_repair_trust::{
    SelectionManifest, load_strict_json, validate_selection_manifest,
};
use super::schema::{DriverBindingRecord, driver_fail};
use super::validate::validate_binding_record;

pub(crate) struct DriverCheckOutcome {
    pub(crate) manifest_path: String,
    pub(crate) manifest: Option<SelectionManifest>,
    pub(crate) bindings_input: Option<String>,
    pub(crate) records: Vec<DriverBindingRecord>,
    pub(crate) violations: Vec<String>,
}

impl DriverCheckOutcome {
    pub(crate) fn verdict(&self) -> &'static str {
        // Violations take precedence: a set of ALL-invalid bindings has zero
        // valid records but must fail closed as `inconsistent` (nonzero
        // exit), never pass as `not_run`. `not_run` is only an empty input
        // with no violations at all.
        if !self.violations.is_empty() {
            return "inconsistent";
        }
        if self.records.is_empty() {
            "not_run"
        } else {
            "valid"
        }
    }
}

pub(crate) fn check_driver_artifacts(
    manifest_path: &str,
    bindings_input: &str,
) -> Result<DriverCheckOutcome, String> {
    if !Path::new(manifest_path).exists() {
        return Err(driver_fail(
            manifest_path,
            "manifest",
            "driver binding records bind to an accepted selection manifest; no manifest exists at this path",
        ));
    }
    let (value, manifest_sha256) = load_strict_json(manifest_path)?;
    let manifest = validate_selection_manifest(&value, manifest_sha256.clone())?;

    let path = Path::new(bindings_input);
    if !path.exists() {
        return Err(driver_fail(
            bindings_input,
            "bindings",
            "bindings input path does not exist",
        ));
    }
    let files = if path.is_dir() {
        let mut entries = Vec::new();
        let read = std::fs::read_dir(path).map_err(|error| {
            driver_fail(
                bindings_input,
                "bindings",
                format!("failed to read directory: {error}"),
            )
        })?;
        for entry in read {
            let entry = entry.map_err(|error| {
                driver_fail(
                    bindings_input,
                    "bindings",
                    format!("failed to read directory entry: {error}"),
                )
            })?;
            let name = entry.file_name().to_string_lossy().to_string();
            let full = entry.path();
            if full.is_file() && name.ends_with(".json") {
                entries.push(full.to_string_lossy().to_string());
            }
        }
        entries.sort();
        entries
    } else {
        vec![bindings_input.to_string()]
    };

    let mut records = Vec::new();
    let mut violations = Vec::new();
    for display in &files {
        match load_strict_json(display) {
            Ok((record, sha256)) => {
                match validate_binding_record(
                    display,
                    &record,
                    &manifest,
                    &manifest_sha256,
                    &sha256,
                ) {
                    Ok(record) => records.push(record),
                    Err(violation) => violations.push(violation),
                }
            }
            Err(error) => violations.push(error),
        }
    }
    enforce_prepare_to_apply_chain(&records, &mut violations);
    Ok(DriverCheckOutcome {
        manifest_path: manifest_path.to_string(),
        manifest: Some(manifest),
        bindings_input: Some(bindings_input.to_string()),
        records,
        violations,
    })
}

/// The prepare-to-apply digest chain: an apply record's
/// `binding_artifact_sha256` must be the recomputed exact-byte sha256 of
/// exactly one supplied prepare record, AND the apply record must agree with
/// that prepare record on every prepare-bound identity field (trust attempt,
/// selection digest, seam, prepare-time repository head, target path, input
/// digests, edit surface, and authorization). A fabricated digest, a tampered
/// prepare artifact (its bytes moved, so the recomputed digest moved), a
/// duplicated prepare record (two files share the digest), or an apply that
/// copies an unrelated attempt's prepare digest while carrying its own
/// identities fails, naming both records and the disagreeing field. The apply
/// must also agree on its own `durable_attempt_id` boundary only indirectly:
/// the trusted binding is the digest chain plus the identity agreement, so a
/// cross-attempt reference cannot validate. Prepare-only records stay valid:
/// they are the `awaiting_edit` state.
fn enforce_prepare_to_apply_chain(records: &[DriverBindingRecord], violations: &mut Vec<String>) {
    let mut prepares_by_digest: std::collections::BTreeMap<&str, Vec<&DriverBindingRecord>> =
        std::collections::BTreeMap::new();
    for record in records.iter().filter(|record| record.phase == "prepare") {
        prepares_by_digest
            .entry(record.artifact_sha256.as_str())
            .or_default()
            .push(record);
    }
    for record in records.iter().filter(|record| record.phase == "apply") {
        let Some(claimed) = record.binding_artifact_sha256.as_deref() else {
            continue;
        };
        match prepares_by_digest.get(claimed).map(Vec::as_slice) {
            None => violations.push(driver_fail(
                &record.display,
                "binding_artifact_sha256",
                format!(
                    "prepare-to-apply digest chain broken: no supplied prepare record's exact bytes digest to `{claimed}`; a tampered or missing prepare artifact, or a fabricated digest, fails"
                ),
            )),
            Some(matches) if matches.len() > 1 => violations.push(driver_fail(
                &record.display,
                "binding_artifact_sha256",
                format!(
                    "ambiguous prepare reference: {} supplied prepare records share the digest `{claimed}`; duplicate phase records fail",
                    matches.len()
                ),
            )),
            Some([prepare]) => {
                let subject = format!("{} -> {}", record.display, prepare.display);
                let pairs: [(&str, &str, &str); 10] = [
                    ("trust.attempt_id", &record.trust_attempt_id, &prepare.trust_attempt_id),
                    ("trust.selection_digest", &record.selection_digest, &prepare.selection_digest),
                    ("seam_id", &record.seam_id, &prepare.seam_id),
                    ("repository_head", &record.repository_head, &prepare.repository_head),
                    ("trust.target_path", &record.target_path, &prepare.target_path),
                    ("input.packet_sha256", &record.packet_sha256, &prepare.packet_sha256),
                    (
                        "input.before_snapshot_sha256",
                        &record.before_snapshot_sha256,
                        &prepare.before_snapshot_sha256,
                    ),
                    (
                        "authorization.status",
                        &record.authorization_status,
                        &prepare.authorization_status,
                    ),
                    (
                        "authorization.authority",
                        &record.authorization_authority,
                        &prepare.authorization_authority,
                    ),
                    (
                        "authorization.method",
                        &record.authorization_method,
                        &prepare.authorization_method,
                    ),
                ];
                for (field, apply_value, prepare_value) in pairs {
                    if apply_value != prepare_value {
                        violations.push(driver_fail(
                            &subject,
                            field,
                            format!(
                                "prepare-to-apply identity disagreement: the apply record names `{apply_value}` but its prepare record names `{prepare_value}`; an apply bound to another attempt's prepare fails"
                            ),
                        ));
                    }
                }
                if record.allowed_surface != prepare.allowed_surface {
                    violations.push(driver_fail(
                        &subject,
                        "edit_surface.allowed",
                        format!(
                            "prepare-to-apply identity disagreement: the apply record declares {:?} but its prepare record declares {:?}; a changed edit surface breaks the binding",
                            record.allowed_surface, prepare.allowed_surface
                        ),
                    ));
                }
                if record.forbidden_surface != prepare.forbidden_surface {
                    violations.push(driver_fail(
                        &subject,
                        "edit_surface.forbidden",
                        format!(
                            "prepare-to-apply identity disagreement: the apply record declares {:?} but its prepare record declares {:?}; a changed edit surface breaks the binding",
                            record.forbidden_surface, prepare.forbidden_surface
                        ),
                    ));
                }
            }
            Some(_) => {
                // Unreachable: map values are always non-empty, and the
                // single-entry case matched `Some([prepare])` above.
            }
        }
    }
}
