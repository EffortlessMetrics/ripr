//! The mechanical currentness gate (`--check-currentness`): recomputes the
//! identities the pointer binds — accepted-receipt bytes, retained-candidate
//! bytes, the manifest digest plus re-validation against the current
//! accepted state, per-subject row/input/config bindings, and the toolchain
//! source/binary identity — and reports `current`, `stale`, or
//! `unverifiable`. Staleness derives ONLY from digest, binding, and
//! vocabulary comparisons: the as-of disclosure is never an input, so
//! editing it can never repair a stale pointer. `stale` exits nonzero (the
//! gate signal); the other verdicts exit 0 with every reason disclosed.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::{
    CONTRACT_VERSION, NOTE_MAX_CHARS, POINTER_FILE, POINTER_KEYS, POINTER_KIND, POINTER_RIPR_KEYS,
    POINTER_SCHEMA, POINTER_SUBJECT_KEYS, RECEIPTS_DIR, RERUN_COMMAND, ReportArgs, SPEC, as_object,
    check_git_sha, check_portable_components, check_portable_path, check_sha256_digest, fail,
    reject_unknown_keys,
};
use crate::reports::eval_sweep_check::{
    AcceptedManifest, load_strict_json, sha256_hex, validate_accepted_manifest,
    validate_run_receipt,
};
use crate::reports::eval_sweep_refresh::repo_root_anchor;

// ---------------------------------------------------------------------------
// Currentness gate
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CurrentnessVerdict {
    Current,
    Stale,
    Unverifiable,
}

impl CurrentnessVerdict {
    fn as_str(self) -> &'static str {
        match self {
            CurrentnessVerdict::Current => "current",
            CurrentnessVerdict::Stale => "stale",
            CurrentnessVerdict::Unverifiable => "unverifiable",
        }
    }
}

/// The recomputed current identities the pointer is compared against. Fields
/// the loader cannot recompute offline stay `None` — those identities are
/// disclosed `unverifiable`, never assumed current.
pub(super) struct CurrentIdentity {
    /// Flag override or `git rev-parse HEAD` in the repository root.
    pub(super) ripr_source_sha: Option<String>,
    /// sha256 of the supplied `--ripr-bin` bytes.
    pub(super) ripr_binary_digest: Option<String>,
    /// Per-subject recomputed input digest (sha256 over the current synthetic
    /// diff bytes), only where the input file resolves and reads.
    pub(super) subject_inputs: BTreeMap<String, String>,
}

pub(super) struct CurrentnessComparison {
    pub(super) verdict: CurrentnessVerdict,
    pub(super) stale: Vec<String>,
    pub(super) unverifiable: Vec<String>,
}

/// Everything one currentness comparison reads: the parsed pointer, the
/// accepted receipt with its recomputed digest, the retained candidate with
/// its recomputed digest, the CURRENT accepted manifest (when it loads and
/// validates — see `manifest_validation_error`), and the recomputed live
/// identities.
pub(super) struct CurrentnessInputs<'a> {
    pub(super) pointer: &'a serde_json::Map<String, Value>,
    pub(super) accepted_receipt: &'a Value,
    pub(super) receipt_sha256: &'a str,
    pub(super) candidate: &'a Value,
    pub(super) candidate_sha256: &'a str,
    /// The CURRENT accepted manifest after a successful load + validation.
    /// `None` when the manifest fails either: the comparison then records the
    /// failure as a stale reason and skips the manifest-dependent re-checks
    /// (they need the validated manifest and cannot flip an already-stale
    /// verdict).
    pub(super) accepted: Option<&'a AcceptedManifest>,
    pub(super) current_manifest_sha256: &'a str,
    /// The manifest load/validation failure, when the current manifest bytes
    /// do not parse or validate against the accepted-state contract. Never
    /// aborts the verdict path: the digest comparison runs on the raw bytes,
    /// so a moved manifest reaches the promised `stale` verdict even when its
    /// new bytes are malformed, and the failure is named as an additional
    /// stale reason.
    pub(super) manifest_validation_error: Option<&'a str>,
    pub(super) current: &'a CurrentIdentity,
}

/// The pure currentness comparison: pointer-bound identities vs the accepted
/// receipt bytes, the retained candidate bytes, and the recomputed current
/// state. Staleness derives ONLY from digest, binding, and vocabulary
/// comparisons — the `as_of` disclosure is never an input, so editing it can
/// never repair a stale pointer.
pub(super) fn compare_currentness(
    inputs: &CurrentnessInputs,
) -> Result<CurrentnessComparison, String> {
    let pointer = inputs.pointer;
    let accepted_receipt = inputs.accepted_receipt;
    let receipt_sha256 = inputs.receipt_sha256;
    let candidate = inputs.candidate;
    let candidate_sha256 = inputs.candidate_sha256;
    let current_manifest_sha256 = inputs.current_manifest_sha256;
    let current = inputs.current;
    let mut stale: Vec<String> = Vec::new();
    let mut unverifiable: Vec<String> = Vec::new();

    // 1. The accepted receipt's bytes must still hash to the bound digest.
    let bound_receipt = pointer
        .get("receipt_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            fail(
                "pointer",
                "receipt_sha256",
                "pointer must bind the receipt digest",
            )
        })?
        .to_string();
    check_sha256_digest("pointer", "receipt_sha256", &bound_receipt)?;
    if bound_receipt != receipt_sha256 {
        stale.push(format!(
            "accepted receipt bytes changed: pointer binds sha256 `{bound_receipt}` but the artifact hashes to `{receipt_sha256}`"
        ));
    }

    // 2. The manifest file's current digest against the bound copy.
    let bound_manifest = pointer
        .get("manifest_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            fail(
                "pointer",
                "manifest_sha256",
                "pointer must bind the manifest digest",
            )
        })?
        .to_string();
    check_sha256_digest("pointer", "manifest_sha256", &bound_manifest)?;
    if bound_manifest != current_manifest_sha256 {
        stale.push(format!(
            "accepted manifest changed: pointer binds sha256 `{bound_manifest}` but the manifest hashes to `{current_manifest_sha256}`"
        ));
    }

    // 2b. The manifest must also still load and validate against the
    // accepted-state contract. The failure never aborts the verdict path:
    // the digest comparison above ran on the raw bytes, so a moved manifest
    // is recorded stale even when its new bytes are malformed, and the
    // failure itself is named as an additional stale reason.
    if let Some(error) = inputs.manifest_validation_error {
        let first = error.lines().next().unwrap_or_default();
        stale.push(format!("accepted manifest failed validation: {first}"));
    }

    // 3. The command contract version.
    let bound_contract = pointer
        .get("command_contract_version")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            fail(
                "pointer",
                "command_contract_version",
                "pointer must bind the command contract version",
            )
        })?
        .to_string();
    if bound_contract != CONTRACT_VERSION {
        stale.push(format!(
            "command contract moved: pointer was accepted under contract `{bound_contract}`, this checker enforces `{CONTRACT_VERSION}`"
        ));
    }

    // 4. The accepted receipt's candidate binding: the retained candidate
    // must still be the exact bytes the accepted receipt was derived from.
    let bound_candidate = accepted_receipt
        .get("candidate")
        .and_then(|candidate| candidate.get("sha256"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            fail(
                "accepted receipt",
                "candidate.sha256",
                "accepted receipt must bind its candidate digest",
            )
        })?
        .to_string();
    if bound_candidate != candidate_sha256 {
        stale.push(format!(
            "retained candidate changed: the accepted receipt binds sha256 `{bound_candidate}` but the retained candidate hashes to `{candidate_sha256}`"
        ));
    }

    // 5. The pointer's per-subject identity map must agree with the accepted
    // receipt's own identity projection (both derive from the same validated
    // rows; a disagreement is an edit in one of them).
    let bound_subjects = pointer.get("subjects").cloned().ok_or_else(|| {
        fail(
            "pointer",
            "subjects",
            "pointer must bind per-subject identities",
        )
    })?;
    let receipt_subjects = accepted_receipt
        .get("identities")
        .and_then(|identities| identities.get("subjects"))
        .cloned()
        .ok_or_else(|| {
            fail(
                "accepted receipt",
                "identities.subjects",
                "accepted receipt must carry its per-subject identity projection",
            )
        })?;
    if bound_subjects != receipt_subjects {
        stale.push(
            "pointer subject identities disagree with the accepted receipt's identity projection"
                .to_string(),
        );
    }

    // 6. Accepted-row digests and pointer-vs-candidate per-subject identity
    // bindings. Every row of the retained candidate must still hash to its
    // bound row digest, and every bound per-subject identity copy must agree
    // with the candidate rows (a tree/input/config identity edit in either
    // place is stale).
    let pointer_subjects = bound_subjects.as_object().ok_or_else(|| {
        fail(
            "pointer",
            "subjects",
            "pointer subject identities must be an object",
        )
    })?;
    let candidate_rows = candidate
        .get("repos")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            fail(
                "candidate",
                "repos",
                "retained candidate must carry a repos array",
            )
        })?;
    let mut row_digests: BTreeMap<String, String> = BTreeMap::new();
    for row in candidate_rows {
        let entry = as_object(row, "candidate", "repos", "retained candidate row")?;
        let id = entry
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let canonical = serde_json::to_vec_pretty(row).map_err(|error| {
            fail(
                &id,
                "row",
                format!("cannot canonicalize the accepted row: {error}"),
            )
        })?;
        let row_digest = sha256_hex(&canonical);
        row_digests.insert(id.clone(), row_digest);

        let bound = match pointer_subjects.get(&id) {
            Some(bound) => bound,
            None => {
                stale.push(format!(
                    "accepted subject `{id}` carries no pointer identity binding"
                ));
                continue;
            }
        };
        let bound = as_object(bound, &id, "subjects", "pointer subject identity")?;
        reject_unknown_keys(
            bound,
            &POINTER_SUBJECT_KEYS,
            &id,
            "pointer subject identity",
        )?;
        for field in POINTER_SUBJECT_KEYS {
            let Some(bound_value) = bound.get(field) else {
                // A missing subject identity is never silently skipped: an
                // unbound identity cannot be re-verified, so the verdict can
                // never read `current` on its strength. (`row_sha256`
                // absence is additionally refused outright by the row-digest
                // binding below; the toolchain-level analogues are disclosed
                // by the live source/binary comparisons in step 9.)
                unverifiable.push(format!(
                    "pointer binds no `{field}` for subject `{id}`; that identity cannot be re-verified"
                ));
                continue;
            };
            let current_value = match field {
                "row_sha256" => Some(Value::String(sha256_hex(&canonical))),
                "tree_digest" => entry.get("tree_digest").cloned(),
                "input_digest" => entry.get("input_digest").cloned(),
                "config_input" => entry
                    .get("config")
                    .and_then(|config| config.get("input"))
                    .cloned(),
                "config_profile" => entry
                    .get("config")
                    .and_then(|config| config.get("profile"))
                    .cloned(),
                _ => None,
            };
            match current_value {
                Some(value) if value != *bound_value => stale.push(format!(
                    "bound identity moved for subject `{id}`: pointer `{field}` no longer matches the accepted receipt"
                )),
                Some(_) => {}
                None => stale.push(format!(
                    "bound identity moved for subject `{id}`: pointer binds `{field}` but the accepted receipt records none"
                )),
            }
        }
    }
    if row_digests.len() != pointer_subjects.len() {
        stale.push(format!(
            "pointer binds {} subject identity entries but the accepted receipt carries {} rows",
            pointer_subjects.len(),
            row_digests.len()
        ));
    }
    for (id, digest) in &row_digests {
        match pointer_subjects.get(id).map(|bound| {
            bound
                .get("row_sha256")
                .and_then(Value::as_str)
                .map(str::to_string)
        }) {
            Some(Some(bound_row)) => {
                if bound_row != *digest {
                    stale.push(format!(
                        "accepted row bytes changed for subject `{id}`: pointer binds row sha256 `{bound_row}` but the row hashes to `{digest}`"
                    ));
                }
            }
            Some(None) => {
                return Err(fail(
                    id,
                    "subjects.row_sha256",
                    "pointer subject must bind its row digest",
                ));
            }
            None => stale.push(format!(
                "accepted subject `{id}` carries no pointer identity binding"
            )),
        }
    }

    // 7. The toolchain identity block: the pointer's bound copies must agree
    // with the retained candidate's envelope (an edited feature/build/source
    // copy in either place is stale).
    let pointer_ripr = pointer
        .get("ripr")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            fail(
                "pointer",
                "ripr",
                "pointer must bind the toolchain identity block",
            )
        })?;
    reject_unknown_keys(
        pointer_ripr,
        &POINTER_RIPR_KEYS,
        "pointer",
        "pointer ripr identity",
    )?;
    if let Some(bound_source) = pointer_ripr.get("source_sha").and_then(Value::as_str) {
        check_git_sha("pointer", "ripr.source_sha", bound_source)?;
    }
    if let Some(bound_binary) = pointer_ripr.get("binary_digest").and_then(Value::as_str) {
        check_sha256_digest("pointer", "ripr.binary_digest", bound_binary)?;
    }
    let candidate_ripr = candidate.get("ripr").and_then(Value::as_object);
    for field in POINTER_RIPR_KEYS {
        let Some(bound_value) = pointer_ripr.get(field) else {
            // A missing toolchain identity is never silently skipped: an
            // unbound identity cannot be re-verified, so the verdict can
            // never read `current` on its strength (the step-6 subject rule
            // applied to the toolchain block; the source/binary analogues
            // are additionally disclosed by the live comparisons in step 9).
            unverifiable.push(format!(
                "pointer binds no toolchain identity `{field}`; that identity cannot be re-verified"
            ));
            continue;
        };
        match candidate_ripr.and_then(|block| block.get(field)) {
            Some(candidate_value) if candidate_value != bound_value => stale.push(format!(
                "bound toolchain identity `{field}` no longer matches the retained candidate"
            )),
            Some(_) => {}
            None => stale.push(format!(
                "pointer binds toolchain identity `{field}` but the retained candidate records none"
            )),
        }
    }

    // 8. The receipt-vs-CURRENT-manifest validation: subject tree pins,
    // licenses, and the manifest digest binding inside the retained candidate
    // are re-checked against the manifest as it exists now. A changed tree
    // pin or any other manifest movement makes the accepted receipt
    // structurally stale. Skipped when the manifest itself failed to load or
    // validate: the 2b stale reason already owns the verdict and this
    // re-validation needs the validated manifest.
    if let Some(accepted) = inputs.accepted
        && let Err(error) = validate_run_receipt(
            candidate,
            current_manifest_sha256,
            accepted,
            "retained candidate",
        )
    {
        let first = error.lines().next().unwrap_or_default().to_string();
        stale.push(format!(
            "accepted receipt no longer validates against the current accepted state: {first}"
        ));
    }

    // 9. Live toolchain movement: analyzer source sha and binary bytes
    // against the recomputed current values; absent recompute inputs are
    // unverifiable (never assumed current).
    match (
        pointer_ripr.get("source_sha").and_then(Value::as_str),
        current.ripr_source_sha.as_deref(),
    ) {
        (Some(bound), Some(current_sha)) if bound != current_sha => stale.push(format!(
            "analyzer source moved: pointer binds source sha `{bound}` but the current source is `{current_sha}`"
        )),
        (Some(_), Some(_)) => {}
        (Some(_), None) => unverifiable.push(
            "analyzer source identity could not be recomputed (no git HEAD and no --ripr-source-sha); source currentness is unverifiable"
                .to_string(),
        ),
        (None, _) => unverifiable.push(
            "pointer binds no analyzer source sha; source currentness is unverifiable".to_string(),
        ),
    }
    match (
        pointer_ripr.get("binary_digest").and_then(Value::as_str),
        current.ripr_binary_digest.as_deref(),
    ) {
        (Some(bound), Some(current_digest)) if bound != current_digest => stale.push(format!(
            "analyzer binary moved: pointer binds binary sha256 `{bound}` but the supplied binary hashes to `{current_digest}`"
        )),
        (Some(_), Some(_)) => {}
        (Some(_), None) => unverifiable.push(
            "binary identity could not be recomputed (no --ripr-bin supplied); binary currentness is unverifiable"
                .to_string(),
        ),
        (None, _) => unverifiable.push(
            "pointer binds no binary digest; binary currentness is unverifiable".to_string(),
        ),
    }

    // 10. Config/input identity, PATH-BOUND: the pointer's per-subject
    // `config_input` must BE the manifest-declared synthetic diff for that
    // subject — a candidate that hashed a different portable file is stale,
    // not current, even when the digest matches the substituted bytes — and
    // the bound `input_digest` must match the recomputed digest of the
    // CURRENT bytes at the manifest-declared path (the recomputation hashes
    // the declared path only). A missing or unrecomputable subject identity
    // is disclosed unverifiable and can never leave the verdict `current`.
    // Skipped when the manifest failed to load or validate: the declared
    // input paths are unavailable then, and the 2b stale reason already owns
    // the verdict.
    if let Some(accepted) = inputs.accepted {
        for (id, bound) in pointer_subjects.iter() {
            let declared = accepted
                .subject(id)
                .map(|subject| subject.synthetic_diff.as_str());
            let bound_config = bound.get("config_input").and_then(Value::as_str);
            match (bound_config, declared) {
                (Some(bound), Some(declared_path)) if bound != declared_path => {
                    stale.push(format!(
                        "input path substituted for subject `{id}`: pointer binds config_input `{bound}` but the accepted manifest declares `{declared_path}`; the hashed input must be the manifest-declared input"
                    ));
                }
                (Some(_), Some(_)) => {}
                (Some(_), None) => {
                    // Outside the manifest denominator; step 8's re-validation
                    // against the current manifest already reports it stale.
                }
                (None, Some(declared_path)) => unverifiable.push(format!(
                    "input path for subject `{id}` could not be verified (the pointer binds no config_input while the accepted manifest declares `{declared_path}`)"
                )),
                (None, None) => {}
            }
            let bound_input = bound
                .get("input_digest")
                .and_then(Value::as_str)
                .map(str::to_string);
            match (bound_input, current.subject_inputs.get(id)) {
                (Some(bound), Some(recomputed)) if bound != *recomputed => stale.push(format!(
                    "input moved for subject `{id}`: pointer binds input sha256 `{bound}` but the current input hashes to `{recomputed}`"
                )),
                (Some(_), Some(_)) => {}
                (Some(_), None) => unverifiable.push(format!(
                    "input identity for subject `{id}` could not be recomputed (the manifest-declared input file did not resolve or read)"
                )),
                (None, recomputed) => unverifiable.push(format!(
                    "input identity for subject `{id}` could not be verified (the pointer binds no input digest; the manifest-declared input {})",
                    match recomputed {
                        Some(digest) => format!("currently hashes to `{digest}`"),
                        None => "did not resolve or read".to_string(),
                    }
                )),
            }
        }
    }

    let verdict = if !stale.is_empty() {
        CurrentnessVerdict::Stale
    } else if !unverifiable.is_empty() {
        CurrentnessVerdict::Unverifiable
    } else {
        CurrentnessVerdict::Current
    };
    Ok(CurrentnessComparison {
        verdict,
        stale,
        unverifiable,
    })
}

/// Recomputes the per-subject input digest from the CURRENT input files: the
/// manifest-DECLARED synthetic diff for each accepted subject — never a
/// candidate-named substitute path — resolved manifest-directory-relative
/// first, then repository-root-relative (the same resolution order the
/// refresh route documents). The pointer-vs-manifest path binding itself is
/// enforced in the comparison; this recomputation makes the hashed bytes the
/// bytes of the declared input by construction. Subjects whose input does
/// not resolve or read are omitted — disclosed unverifiable, never assumed
/// current.
pub(super) fn recompute_subject_inputs(
    accepted: &AcceptedManifest,
    manifest_path: &str,
) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let manifest_dir = Path::new(manifest_path)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    for subject in &accepted.subjects {
        let from_manifest_dir = manifest_dir.join(&subject.synthetic_diff);
        let from_repo_root = repo_root_anchor().join(&subject.synthetic_diff);
        let resolved = if from_manifest_dir.is_file() {
            Some(from_manifest_dir)
        } else if from_repo_root.is_file() {
            Some(from_repo_root)
        } else {
            None
        };
        if let Some(path) = resolved
            && let Ok(bytes) = std::fs::read(&path)
        {
            out.insert(subject.id.clone(), sha256_hex(&bytes));
        }
    }
    out
}

/// Resolves the current analyzer source sha: the explicit flag wins;
/// otherwise a bounded `git rev-parse HEAD` in the repository root. Any
/// failure is `None` — disclosed unverifiable, never guessed.
fn resolve_current_source_sha(flag: Option<&str>) -> Option<String> {
    if let Some(sha) = flag {
        return Some(sha.to_string());
    }
    let root = repo_root_anchor();
    let output = crate::run::capture_output_with_timeout(
        "git",
        &[
            "-C".to_string(),
            root.to_string_lossy().to_string(),
            "rev-parse".to_string(),
            "HEAD".to_string(),
        ],
        &[],
        std::time::Duration::from_secs(30),
        "eval-sweep report currentness source-sha probe",
    )
    .ok()?;
    if output.timed_out || !output.status.is_some_and(|status| status.success()) {
        return None;
    }
    let sha = output.stdout.trim().to_string();
    if sha.len() == 40 && sha.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Some(sha)
    } else {
        None
    }
}

pub(super) fn run_currentness_check(parsed: &ReportArgs) -> Result<(), String> {
    let pointer_path = Path::new(&parsed.state_dir).join(POINTER_FILE);
    if !pointer_path.exists() {
        println!(
            "eval-sweep report currentness: pointer=<none> verdict=not_run (no accepted receipt published; not_run is not a pass)"
        );
        println!("rerun: {RERUN_COMMAND}");
        return Ok(());
    }
    let (pointer_value, _pointer_sha) = load_strict_json(&pointer_path.to_string_lossy())?;
    let pointer = as_object(&pointer_value, "pointer", "pointer", "current pointer")?;
    reject_unknown_keys(pointer, &POINTER_KEYS, "pointer", "current pointer")?;
    for (field, expected) in [
        ("schema_version", POINTER_SCHEMA),
        ("kind", POINTER_KIND),
        ("spec", SPEC),
    ] {
        let actual = pointer
            .get(field)
            .and_then(Value::as_str)
            .ok_or_else(|| fail("pointer", field, "current pointer must declare this field"))?;
        if actual != expected {
            return Err(fail(
                "pointer",
                field,
                format!("expected `{expected}`, got `{actual}`"),
            ));
        }
    }
    // The as-of disclosure is hygiene-checked but never enters the identity
    // comparison below (editing it can never repair staleness): when present
    // it must be a non-empty bounded string, so a malformed value cannot ride
    // along inside an otherwise current pointer.
    if let Some(as_of) = pointer.get("as_of") {
        let text = as_of.as_str().ok_or_else(|| {
            fail(
                "pointer",
                "as_of",
                "as-of must be a JSON string when present",
            )
        })?;
        if text.trim().is_empty() {
            return Err(fail(
                "pointer",
                "as_of",
                "as-of must be non-empty when present",
            ));
        }
        let length = text.chars().count();
        if length > NOTE_MAX_CHARS {
            return Err(fail(
                "pointer",
                "as_of",
                format!(
                    "as-of exceeds the {NOTE_MAX_CHARS}-character bound ({length} characters); accepted artifacts carry bounded excerpts, never unbounded logs"
                ),
            ));
        }
    }
    let receipt_file = pointer
        .get("receipt_file")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            fail(
                "pointer",
                "receipt_file",
                "pointer must name its accepted receipt",
            )
        })?;
    check_portable_path("pointer", "receipt_file", receipt_file)?;
    let receipt_path = split_portable("pointer", "receipt_file", &parsed.state_dir, receipt_file)?;
    if !receipt_path.exists() {
        return Err(fail(
            &receipt_path.to_string_lossy(),
            "file",
            "the pointer names an accepted receipt that does not exist; the accepted artifact is missing",
        ));
    }
    let receipt_bytes = std::fs::read(&receipt_path).map_err(|error| {
        fail(
            &receipt_path.to_string_lossy(),
            "file",
            format!("accepted receipt cannot be read: {error}"),
        )
    })?;
    let receipt_sha256 = sha256_hex(&receipt_bytes);
    let (accepted_receipt, _receipt_file_sha) = load_strict_json(&receipt_path.to_string_lossy())?;

    // The retained candidate: addressed by the digest the accepted receipt
    // binds. The rows, identity blocks, and validator bindings all compare
    // against these exact bytes.
    let candidate_binding = accepted_receipt
        .get("candidate")
        .and_then(|candidate| candidate.get("sha256"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            fail(
                "accepted receipt",
                "candidate.sha256",
                "accepted receipt must bind its candidate digest",
            )
        })?
        .to_string();
    let candidate_path = split_portable(
        "accepted receipt",
        "candidate.sha256",
        &parsed.state_dir,
        &format!("{RECEIPTS_DIR}/{candidate_binding}.candidate.json"),
    )?;
    if !candidate_path.exists() {
        return Err(fail(
            &candidate_path.to_string_lossy(),
            "file",
            "the accepted receipt's retained candidate does not exist; the accepted artifact is missing",
        ));
    }
    let candidate_bytes = std::fs::read(&candidate_path).map_err(|error| {
        fail(
            &candidate_path.to_string_lossy(),
            "file",
            format!("retained candidate cannot be read: {error}"),
        )
    })?;
    let candidate_sha256 = sha256_hex(&candidate_bytes);
    let (candidate_value, _candidate_file_sha) =
        load_strict_json(&candidate_path.to_string_lossy())?;

    // The current accepted manifest: the comparison baseline for both the
    // bound manifest digest and the candidate re-validation. The raw bytes
    // are hashed FIRST: a moved manifest is itself the promised `stale`
    // verdict, so the parse/validation is attempted separately and its
    // failure is carried into the comparison as an additional named stale
    // reason — a malformed changed manifest must reach the `stale` verdict,
    // never abort the check with a schema error.
    let manifest_bytes = std::fs::read(&parsed.manifest).map_err(|error| {
        fail(
            &parsed.manifest,
            "file",
            format!("accepted manifest cannot be read: {error}"),
        )
    })?;
    let current_manifest_sha256 = sha256_hex(&manifest_bytes);
    let manifest_state = load_strict_json(&parsed.manifest).and_then(|(manifest_value, _)| {
        validate_accepted_manifest(&manifest_value, current_manifest_sha256.clone())
    });
    let (accepted, manifest_validation_error) = match manifest_state {
        Ok(accepted) => (Some(accepted), None),
        Err(error) => (None, Some(error)),
    };

    let current = CurrentIdentity {
        ripr_source_sha: resolve_current_source_sha(parsed.ripr_source_sha.as_deref()),
        ripr_binary_digest: match &parsed.ripr_bin {
            Some(path) => {
                let bytes = std::fs::read(path).map_err(|error| {
                    fail(
                        path,
                        "ripr-bin",
                        format!("supplied binary cannot be read: {error}"),
                    )
                })?;
                Some(sha256_hex(&bytes))
            }
            None => None,
        },
        // Without a validated manifest the declared input paths are
        // unavailable; the comparison already carries the manifest failure as
        // a stale reason, so no per-subject recomputation is attempted.
        subject_inputs: accepted
            .as_ref()
            .map(|accepted| recompute_subject_inputs(accepted, &parsed.manifest))
            .unwrap_or_default(),
    };

    let comparison = compare_currentness(&CurrentnessInputs {
        pointer,
        accepted_receipt: &accepted_receipt,
        receipt_sha256: &receipt_sha256,
        candidate: &candidate_value,
        candidate_sha256: &candidate_sha256,
        accepted: accepted.as_ref(),
        current_manifest_sha256: &current_manifest_sha256,
        manifest_validation_error: manifest_validation_error.as_deref(),
        current: &current,
    })?;

    println!(
        "eval-sweep report currentness: pointer={} receipt={} verdict={}",
        pointer_path.to_string_lossy(),
        receipt_path.to_string_lossy(),
        comparison.verdict.as_str(),
    );
    for reason in &comparison.stale {
        println!("  stale: {reason}");
    }
    for reason in &comparison.unverifiable {
        println!("  unverifiable: {reason}");
    }
    println!(
        "eval-sweep report currentness verdict: {} — mechanical identity comparison; not a robustness or adequacy claim",
        comparison.verdict.as_str()
    );
    println!("rerun: {RERUN_COMMAND} --check-currentness");
    match comparison.verdict {
        CurrentnessVerdict::Current | CurrentnessVerdict::Unverifiable => Ok(()),
        // Stale is the gate signal: the accepted receipt is no longer current
        // for promotion consumption, so the command exits nonzero — with the
        // exact movements named, so the consumer knows what to re-accept.
        CurrentnessVerdict::Stale => {
            let mut error = format!(
                "eval-sweep report currentness: pointer is STALE ({} stale reason(s)); the accepted receipt must be re-accepted from a fresh candidate before promotion consumption",
                comparison.stale.len()
            );
            for reason in &comparison.stale {
                error.push_str(&format!("\n  stale: {reason}"));
            }
            error.push_str(&format!("\nrerun: {RERUN_COMMAND}"));
            Err(error)
        }
    }
}

/// Joins a state dir with a portable (forward-slash) pointer-relative path on
/// any host. The split is the containment boundary: the portable path is
/// rejected unless every `/`-separated component is non-empty and non-dot
/// (`split` preserves empty components, and host path resolution skips them,
/// so an unguarded join could walk outside the accepted state directory).
pub(super) fn split_portable(
    subject: &str,
    field: &str,
    state_dir: &str,
    portable: &str,
) -> Result<PathBuf, String> {
    check_portable_components(subject, field, portable)?;
    let mut path = PathBuf::from(state_dir);
    for component in portable.split('/') {
        path.push(component);
    }
    Ok(path)
}
