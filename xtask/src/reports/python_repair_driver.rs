//! Driver binding validation for the governed Python repair-trust corpus —
//! `cargo xtask python-repair-trust check-driver` (RIPR-SPEC-0176, #3569).
//!
//! The two-phase external-edit driver (#2443) binds one durable repair
//! attempt (#2927) to one accepted selection row (#3568) by digests and
//! retains a typed binding record inside the attempt. This validator is the
//! offline promotion-side counterpart: given the accepted selection manifest
//! and a set of retained driver binding records, it re-checks every digest
//! anchor and claim boundary the records assert, so a promotion pass can
//! trust the retained evidence without re-running the driver.
//!
//! What the check enforces, fail closed:
//!
//! - the record binds to the ACCEPTED selection manifest by exact-bytes
//!   sha256 — a changed manifest makes every retained record stale;
//! - the record binds one selection row by its recomputed canonical
//!   `selection_digest` over the retained preimage — a replaced or edited row
//!   makes the record stale;
//! - every identity the record copies from the selected row (case, subject,
//!   repository, base, head, optional tree/currentness/limitation, family,
//!   owner, discriminator, relation, oracle) agrees exactly with the accepted
//!   row — a rewritten copy fails even while the digest anchors stay valid;
//! - the record's target identity agrees with the row (`target_path`,
//!   `target_state`); the driver binds only `existing` targets, and the
//!   normalized target path is what is compared and recorded;
//! - the driver issued the binding only under explicit operator/agent
//!   authorization (`status: granted`, named authority, the
//!   explicit-operator-flags method) — an inferred or absent authorization
//!   fails;
//! - the standing non-claims ride on every record: the driver claims no
//!   verification result, no static movement, and no closure. Records carry
//!   no lifecycle, movement, or execution fields at all (deny-unknown), so a
//!   driver record can never appear completed;
//! - apply-phase records additionally carry the durable attempt identity
//!   (#2927 shape), the prepare-record digest chain, the patch digest, the
//!   changed-file set, the edit-cage decision, and the resulting head; every
//!   apply record must chain to exactly one supplied prepare record by its
//!   exact-byte digest AND agree with that prepare record on every
//!   prepare-bound identity field (trust attempt, selection digest, seam,
//!   prepare-time head, target, input digests, edit surface, authorization) —
//!   an apply that names another attempt's prepare digest fails, naming both
//!   records and the disagreeing field. A compliant record must include the
//!   selected target within the declared cage. A `violated` record is
//!   accepted as the producer's typed failed result: its escaped paths are
//!   validated structurally (portable spellings) and retained verbatim as
//!   failure evidence, and the report renders the cage decision so
//!   `compliant` and failed(`violated`) dispositions stay distinguishable.
//! - production/generated/vendor/environment edit surfaces fail: a record
//!   whose target falls under a denied surface prefix is rejected even when
//!   the row declared it `unsafe`, because the driver binds only test-only,
//!   existing, unambiguous targets.
//!
//! Verdict vocabulary: `valid`/`inconsistent`/`not_run` (no records
//! supplied). Violations take precedence: any violation makes the verdict
//! `inconsistent` and the command exits nonzero — an invalid-only binding
//! set never passes. `not_run` is reserved for an empty input with no
//! violations and is not a pass. None of the three is a support-tier,
//! repair-correctness, gate, badge, or promotion claim; #3570 owns the
//! verification phase.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::{Value, json};

use super::python_repair_trust::{
    KNOWN_SPEC, SelectionManifest, canonical_selection_digest, check_git_sha, check_portable_path,
    check_sha256_digest, known_value_or_fail, load_strict_json, opt_string, reject_secret_tokens,
    reject_unknown_keys, require_string, validate_selection_manifest,
};

const RERUN_COMMAND: &str = "cargo xtask python-repair-trust check-driver";
const CHECK_REPORT_JSON: &str = "python-repair-driver-check.json";
const CHECK_REPORT_MD: &str = "python-repair-driver-check.md";

/// The record schema mirrors the driver's binding record exactly. The crate
/// and xtask share the schema by construction, not by a shared dependency:
/// the digest anchors are the contract.
const BINDING_KIND: &str = "python_repair_driver_binding";
const BINDING_SPEC: &str = KNOWN_SPEC;
const BINDING_SCHEMA_VERSION: &str = "0.1";

const RECORD_KEYS_PREPARE: [&str; 14] = [
    "schema_version",
    "kind",
    "spec",
    "phase",
    "seam_id",
    "repository_head",
    "selection_manifest_path",
    "driver",
    "config",
    "input",
    "trust",
    "edit_surface",
    "authorization",
    "non_claims",
];

const RECORD_KEYS_APPLY: [&str; 17] = [
    "schema_version",
    "kind",
    "spec",
    "phase",
    "seam_id",
    "repository_head",
    "selection_manifest_path",
    "durable_attempt_id",
    "binding_artifact_sha256",
    "driver",
    "config",
    "input",
    "trust",
    "edit_surface",
    "authorization",
    "apply",
    "non_claims",
];

const TRUST_KEYS: [&str; 18] = [
    "attempt_id",
    "case_id",
    "subject_id",
    "repository",
    "base",
    "head",
    "tree",
    "source_currentness",
    "selection_manifest_sha256",
    "selection_digest",
    "target_path",
    "target_state",
    "family",
    "owner",
    "discriminator",
    "relation",
    "oracle",
    "limitation",
];

const DRIVER_KEYS: [&str; 2] = ["binary_sha256", "version"];
const CONFIG_KEYS: [&str; 1] = ["profile"];
const INPUT_KEYS: [&str; 2] = ["packet_sha256", "before_snapshot_sha256"];
const EDIT_SURFACE_KEYS: [&str; 2] = ["allowed", "forbidden"];
const AUTHORIZATION_KEYS: [&str; 3] = ["status", "authority", "method"];
const APPLY_KEYS: [&str; 5] = [
    "patch_sha256",
    "changed_paths",
    "cage_status",
    "repository_head_after",
    "current",
];

const CAGE_STATUSES: [&str; 3] = ["compliant", "violated", "incomparable"];
const AUTHORIZATION_STATUSES: [&str; 1] = ["granted"];
const AUTHORIZATION_METHODS: [&str; 1] = ["explicit-operator-flags"];

/// The only target state a driver binding may declare: the offline check
/// mirrors the producer, which refuses proposed/ambiguous/unavailable/unsafe
/// targets before any attempt exists.
const BINDABLE_TARGET_STATE: &str = "existing";

/// The standing non-claims every driver record must carry verbatim.
const BINDING_NON_CLAIMS: [&str; 3] = [
    "no verification result is claimed by the driver",
    "no static movement is claimed by the driver",
    "no closure is claimed by the driver",
];

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

/// Fail-closed error text for the driver check: names subject/field/reason
/// plus the driver rerun command (the shared `fail` names the corpus check).
fn driver_fail(subject: &str, field: &str, reason: impl std::fmt::Display) -> String {
    format!(
        "python-repair-trust check-driver failed: subject=`{subject}` field=`{field}`: {reason}
rerun: {RERUN_COMMAND}"
    )
}

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

/// One validated record, reduced to what the report and the prepare-to-apply
/// digest chain need. The prepare-bound identity fields (seam, head, target,
/// selection digest, input digests, edit surface, authorization) are retained
/// so the chain can require the apply record to agree with ITS prepare record,
/// not merely to name a valid prepare digest.
struct DriverBindingRecord {
    display: String,
    phase: String,
    trust_attempt_id: String,
    target_path: String,
    selection_digest: String,
    seam_id: String,
    repository_head: String,
    packet_sha256: String,
    before_snapshot_sha256: String,
    allowed_surface: BTreeSet<String>,
    forbidden_surface: BTreeSet<String>,
    authorization_status: String,
    authorization_authority: String,
    authorization_method: String,
    /// The apply block's edit-cage decision, verbatim (`None` on prepare
    /// records). `violated` records are typed failed results: the offline
    /// validator accepts them as retained failure evidence while a
    /// `compliant` record must still cover the selected target in-cage.
    cage_status: Option<String>,
    /// The recomputed exact-byte sha256 of the loaded record file.
    artifact_sha256: String,
    /// The apply-phase claim into the prepare-record digest chain.
    binding_artifact_sha256: Option<String>,
}

struct DriverCheckOutcome {
    manifest_path: String,
    manifest: Option<SelectionManifest>,
    bindings_input: Option<String>,
    records: Vec<DriverBindingRecord>,
    violations: Vec<String>,
}

impl DriverCheckOutcome {
    fn verdict(&self) -> &'static str {
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

fn parse_check_driver_args(args: &[String]) -> Result<(String, String), String> {
    let mut manifest = super::python_repair_trust::DEFAULT_MANIFEST.to_string();
    let mut bindings: Option<String> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--manifest" => {
                index += 1;
                manifest = args.get(index).cloned().ok_or_else(|| {
                    format!(
                        "python-repair-trust check-driver --manifest requires a value\nrerun: {RERUN_COMMAND}"
                    )
                })?;
            }
            "--bindings" => {
                index += 1;
                bindings = Some(args.get(index).cloned().ok_or_else(|| {
                    format!(
                        "python-repair-trust check-driver --bindings requires a value\nrerun: {RERUN_COMMAND}"
                    )
                })?);
            }
            other => {
                return Err(format!(
                    "unknown python-repair-trust check-driver argument: {other}\nusage: cargo xtask python-repair-trust check-driver [--manifest <path>] --bindings <dir-or-file>\nrerun: {RERUN_COMMAND}"
                ));
            }
        }
        index += 1;
    }
    let bindings = bindings.ok_or_else(|| {
        format!(
            "python-repair-trust check-driver requires --bindings <dir-or-file>\nrerun: {RERUN_COMMAND}"
        )
    })?;
    Ok((manifest, bindings))
}

/// The command entry: `python-repair-trust check-driver`, dispatched from the
/// `python-repair-trust` subcommand router.
pub(crate) fn run_check_driver(args: &[String]) -> Result<(), String> {
    let (manifest_path, bindings_input) = parse_check_driver_args(args)?;
    let outcome = check_driver_artifacts(&manifest_path, &bindings_input)?;
    let verdict = outcome.verdict();

    match &outcome.manifest {
        None => println!(
            "python-repair-trust check-driver: manifest={manifest_path} selections=<none> (no accepted selection manifest; binding records cannot bind without one)"
        ),
        Some(manifest) => println!(
            "python-repair-trust check-driver: manifest={manifest_path} selections={} sha256={}",
            manifest.selections.len(),
            manifest.sha256
        ),
    }
    match &outcome.bindings_input {
        None => println!(
            "python-repair-trust check-driver: bindings=<none> verdict={verdict} (no records supplied; not_run is not a pass)"
        ),
        Some(input) => println!(
            "python-repair-trust check-driver: bindings={input} records={} violations={} verdict={verdict}",
            outcome.records.len(),
            outcome.violations.len()
        ),
    }
    for violation in &outcome.violations {
        println!("  violation: {violation}");
    }
    println!(
        "python-repair-trust check-driver verdict: {verdict} — binding structural validation only; no completed or correct repair is established and no verification phase is claimed"
    );
    println!("rerun: {RERUN_COMMAND}");

    let report = render_check_driver_json(&outcome)?;
    crate::write_report(CHECK_REPORT_JSON, &format!("{report}\n"))?;
    let markdown = render_check_driver_markdown(&outcome);
    crate::write_report(CHECK_REPORT_MD, &markdown)?;
    if verdict == "inconsistent" {
        return Err(format!(
            "python-repair-trust check-driver found {} violation(s); see target/ripr/reports/{CHECK_REPORT_JSON}",
            outcome.violations.len()
        ));
    }
    Ok(())
}

fn check_driver_artifacts(
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

/// Validates one driver binding record against the accepted selection
/// manifest. Every digest anchor is recomputed, every closed field set is
/// deny-unknown, and every claim the record could overstate is pinned by the
/// non-claims list.
fn validate_binding_record(
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
            &super::python_repair_trust::SOURCE_CURRENTNESS,
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
        &super::python_repair_trust::TARGET_STATES,
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

    let record_value = Value::Object(top.clone());
    reject_secret_tokens(&record_value, display, "binding record")?;

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

fn render_check_driver_json(outcome: &DriverCheckOutcome) -> Result<String, String> {
    let verdict = outcome.verdict();
    let manifest_json = match &outcome.manifest {
        None => json!(null),
        Some(manifest) => json!({
            "path": outcome.manifest_path,
            "sha256": manifest.sha256,
            "selections": manifest.selections.len(),
        }),
    };
    let records: Vec<Value> = outcome
        .records
        .iter()
        .map(|record| {
            json!({
                "file": record.display,
                "phase": record.phase,
                "trust_attempt_id": record.trust_attempt_id,
                "target_path": record.target_path,
                "cage_status": record.cage_status,
            })
        })
        .collect();
    let violations: Vec<Value> = outcome
        .violations
        .iter()
        .map(|violation| json!(violation))
        .collect();
    let document = json!({
        "schema_version": "0.1",
        "kind": "python_repair_driver_check_report",
        "spec": BINDING_SPEC,
        "verdict": verdict,
        "manifest": manifest_json,
        "bindings": {
            "input": outcome.bindings_input,
            "records": records,
            "violations": violations,
        },
        "offline": true,
        "claim_boundary": "binding structural validation only; no completed or correct repair is established and no verification phase is claimed",
        "rerun": RERUN_COMMAND,
    });
    serde_json::to_string_pretty(&document)
        .map_err(|error| format!("failed to render python-repair-trust check-driver JSON: {error}"))
}

fn render_check_driver_markdown(outcome: &DriverCheckOutcome) -> String {
    let verdict = outcome.verdict();
    let mut out = String::new();
    out.push_str("# Python Repair Driver Binding Check\n\n");
    out.push_str(&format!("Verdict: **{verdict}**\n\n"));
    out.push_str(
        "Binding structural validation only — no completed or correct repair is established and no verification phase is claimed.\n\n",
    );
    match &outcome.manifest {
        None => {
            out.push_str("- manifest: none (records cannot bind without an accepted manifest)\n")
        }
        Some(manifest) => out.push_str(&format!(
            "- manifest: {} ({} selections, sha256 `{}`)\n",
            outcome.manifest_path,
            manifest.selections.len(),
            manifest.sha256
        )),
    }
    match &outcome.bindings_input {
        None => out.push_str("- bindings: none supplied (not_run; not a pass)\n"),
        Some(input) => {
            out.push_str(&format!(
                "- bindings: {input} ({} record(s), {} violation(s))\n",
                outcome.records.len(),
                outcome.violations.len()
            ));
            for record in &outcome.records {
                out.push_str(&format!(
                    "  - {} phase={} trust attempt `{}` target `{}` cage {}\n",
                    record.display,
                    record.phase,
                    record.trust_attempt_id,
                    record.target_path,
                    record.cage_status.as_deref().unwrap_or("n/a (prepare)"),
                ));
            }
        }
    }
    if outcome.violations.is_empty() {
        out.push_str("\n## Violations\n\nNone.\n");
    } else {
        out.push_str("\n## Violations\n\n");
        for violation in &outcome.violations {
            out.push_str(&format!("- {violation}\n"));
        }
    }
    out.push_str(&format!("\nrerun: `{RERUN_COMMAND}`\n"));
    out
}

// ---------------------------------------------------------------------------
// Tests (module named `python_repair_driver_binding` under the file module
// `python_repair_driver`, so `cargo test -p xtask python_repair_driver`
// selects exactly these tests; no unwrap/expect — Result returns only)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod python_repair_driver_binding {
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
        let value =
            serde_json::from_str(&text).map_err(|error| format!("reparse fixture: {error}"))?;
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

    fn prepare_record(
        fixture: &Fixture,
        attempt_id: &str,
        target_path: &str,
    ) -> Result<Value, String> {
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

    fn apply_record(
        fixture: &Fixture,
        attempt_id: &str,
        target_path: &str,
    ) -> Result<Value, String> {
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
        let unsafe_fixture =
            build_fixture(selection_row("att-unsafe", "vendor/lib/ext.py", "unsafe"))?;
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
                row_preimages: std::collections::BTreeMap::from([(
                    "att-denied".to_string(),
                    preimage,
                )]),
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
        std::fs::create_dir_all(&manifest_dir)
            .map_err(|error| format!("create corpus: {error}"))?;
        std::fs::create_dir_all(&bindings_dir)
            .map_err(|error| format!("create bindings: {error}"))?;

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
        std::fs::create_dir_all(&manifest_dir)
            .map_err(|error| format!("create corpus: {error}"))?;
        std::fs::create_dir_all(&bindings_dir)
            .map_err(|error| format!("create bindings: {error}"))?;

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
        std::fs::create_dir_all(&manifest_dir)
            .map_err(|error| format!("create corpus: {error}"))?;
        std::fs::create_dir_all(&bindings_dir)
            .map_err(|error| format!("create bindings: {error}"))?;

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
        let value =
            serde_json::from_str(&text).map_err(|error| format!("reparse two-row: {error}"))?;
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
        std::fs::create_dir_all(&manifest_dir)
            .map_err(|error| format!("create corpus: {error}"))?;
        std::fs::create_dir_all(&bindings_dir)
            .map_err(|error| format!("create bindings: {error}"))?;

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
}
