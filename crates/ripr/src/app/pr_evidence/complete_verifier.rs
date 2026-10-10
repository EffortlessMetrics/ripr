//! Saved-proof verification, never producer execution permission.
//!
//! Call ONLY inside the freshly validated bounded worker, under the matching
//! frozen source context. Root must dispatch complete markers/manifest before
//! legacy large reads, retain marker-removal refusal at every consumer, settle
//! the owned process group, recheck original caller literals and configuration,
//! finalize/revoke source custody, and publish atomically. This helper proves
//! none of those caller obligations. It never runs an analyzer or Git command.
//!
//! Dependencies supplied by their exclusive owners: raw50 parser observer,
//! raw_coverage0885, frozen133c borrowed original-mode inventory, and crate
//! visibility on the EXISTING diff::load::decode_diff_text. No fallback exists.

use super::complete_contract::*;
use super::complete_request::POLICY_PATH;
use super::raw_coverage::{RawCoverageLimits, RawCoverageSummary, verify_raw_coverage};
use crate::analysis::committed_source::frozen;
use crate::analysis_outcome::AnalysisOutcome;
use crate::review_input::{
    CanonicalFindingIndexV1, REVIEW_INPUT_PROJECTION_LIMIT, REVIEW_INPUT_SCHEMA_VERSION,
    REVIEW_INPUT_SELECTION_POLICY, REVIEW_INPUT_SELECTION_POLICY_VERSION, ReviewInputV1,
    canonical_finding_index, canonical_projection_from_index,
};
use serde::Deserializer;
use serde::de::{DeserializeOwned, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs::{File, Metadata, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Eq, PartialEq)]
pub(super) struct CompleteVerificationError(String);
impl fmt::Display for CompleteVerificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for CompleteVerificationError {}
impl From<String> for CompleteVerificationError {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// Saved authority only. Private construction, no Deserialize/Default/Clone,
/// no method converting it into the producer's VerifiedWholeInput capability.
#[derive(Debug)]
pub(super) struct VerifiedGeneration {
    generation_id: String,
    manifest_sha256: String,
    artifact_sha256: [String; 9],
    total_finding_count: u64,
    // Actual opened, final-rechecked files; observed DATA, not custody authority.
    manifest_identity: (u64, u64, u64),
    artifact_identity: [(u64, u64, u64); 9],
}
impl VerifiedGeneration {
    pub(super) fn generation_id(&self) -> &str {
        &self.generation_id
    }
    pub(super) fn manifest_sha256(&self) -> &str {
        &self.manifest_sha256
    }
    pub(super) fn artifact_sha256(&self, role: ArtifactRole) -> &str {
        &self.artifact_sha256[role.ordinal()]
    }
    pub(super) fn total_finding_count(&self) -> u64 {
        self.total_finding_count
    }
    pub(super) fn manifest_identity(&self) -> (u64, u64, u64) {
        self.manifest_identity
    }
    pub(super) fn artifact_identity(&self, role: ArtifactRole) -> (u64, u64, u64) {
        self.artifact_identity[role.ordinal()]
    }
}

pub(super) fn verify_staged_generation(
    generation_root: &Path,
    expected: &CompleteBinding,
    limits: &CompleteVerificationLimits,
) -> Result<VerifiedGeneration, CompleteVerificationError> {
    verify_inner(generation_root, expected, limits).map_err(Into::into)
}

fn verify_inner(
    root: &Path,
    expected: &CompleteBinding,
    limits: &CompleteVerificationLimits,
) -> Result<VerifiedGeneration, String> {
    limits.validate()?;
    if &expected.profile != limits {
        return Err("caller limits differ from bound profile".into());
    }
    expected.validate()?;
    if RustExecutionPolicy::capture()? != expected.rust_execution_policy {
        return Err("fresh Rust execution policy differs from complete binding".into());
    }
    let authority = frozen::current()
        .ok_or("complete saved verification requires an active frozen source authority")?;
    authority.ensure_clean().map_err(|e| e.to_string())?;
    if authority.logical_root().to_str() != Some(expected.subject.logical_root.as_str())
        || authority.head_tree().as_str() != expected.subject.head_tree
    {
        return Err("frozen source subject differs from expected complete binding".into());
    }
    verify_committed_request(&authority, expected)?;
    verify_inventory(&authority, expected)?;
    verify_configuration(&authority, expected)?;

    let directory = StagedDirectory::open(root)?;
    directory.verify_closed()?;
    let mut manifest_file = directory.open_file(MANIFEST_FILE, limits.max_manifest_bytes)?;
    let (manifest_bytes, manifest_sha256) =
        manifest_file.read(Some(limits.max_manifest_bytes), None)?;
    // Wire length is known before parsing. This finite representation allowance
    // is protocol admission, not an assertion about exact allocator overhead;
    // the caller's validated native address-space limit remains authoritative.
    if json_allowance(manifest_bytes.len() as u64)? > limits.max_buffered_bytes {
        return Err("manifest JSON representation admission exceeded".into());
    }
    let manifest: CompleteManifest = strict_json(&manifest_bytes)?;
    manifest.validate(expected, limits)?;
    admit_buffers(&manifest, manifest_bytes.len() as u64, limits)?;

    let mut files = Vec::new();
    files
        .try_reserve_exact(9)
        .map_err(|e| format!("reserve artifact handles: {e}"))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(9)
        .map_err(|e| format!("reserve artifact buffers: {e}"))?;
    for descriptor in &manifest.artifacts {
        let mut file = directory.open_file(&descriptor.path, descriptor.bytes)?;
        if file.identity.bytes != descriptor.bytes {
            return Err("staged artifact length differs from manifest".into());
        }
        let (contents, _) = file.read(Some(descriptor.bytes), Some(&descriptor.sha256))?;
        files.push(file);
        bytes.push(contents);
    }
    let raw = &bytes[ArtifactRole::OriginalRaw.ordinal()];
    let ledger = &bytes[ArtifactRole::RawLedger.ordinal()];
    let (_, coverage) = verify_raw_coverage(raw, ledger, raw_limits(limits)?)?;
    verify_raw_summary(&coverage, &expected.raw)?;
    // The decoder consumes Vec. Admission accounts for that retained raw copy
    // and a worst-case UTF-8 replacement expansion before cloning/calling it.
    let decoded =
        crate::analysis::diff::load::decode_diff_text("complete canonical raw", raw.clone())?;
    if decoded.as_bytes() != bytes[ArtifactRole::CheckDiff.ordinal()] {
        return Err(
            "saved check.diff is not the exact existing decoder output of original raw".into(),
        );
    }
    drop(decoded);

    let check_payload: CompleteCheck =
        payload(&bytes[4], ArtifactRole::FullCheck, &manifest.generation_id)?;
    if check_payload.effective_options != expected.effective_options {
        return Err("saved check effective options differ from caller binding".into());
    }
    let check = &check_payload.check;
    let findings = validate_check(check, expected, &coverage, &manifest.artifacts[1].sha256)?;
    // Admit the complete reducer input before the actual production reducers
    // clone values or build indexes. Fixed framing and eight bounded relation
    // planes are reserved by admit_buffers before any payload is parsed.
    let framing = (findings.len() as u64)
        .checked_mul(1024)
        .ok_or("finding relation framing overflow")?;
    let input_limit = limits
        .max_relation_bytes
        .checked_sub(framing)
        .ok_or("finding relation framing admission exceeded")?
        / 8;
    serialized_digest(findings, input_limit, b"")?;
    let (derived_index, _) = canonical_finding_index(findings, authority.logical_root())?;
    serialized_digest(&derived_index, limits.max_relation_bytes, b"")?;
    // Strict full-value equality is stronger than trusting index scalars.
    let saved_index: CanonicalFindingIndexV1 = payload(
        &bytes[5],
        ArtifactRole::FindingIndex,
        &manifest.generation_id,
    )?;
    if saved_index != derived_index {
        return Err("saved full finding index differs from actual check reducer".into());
    }
    let selected = canonical_projection_from_index(&derived_index)?;
    let projected_count = selected.len() as u64;
    let total = findings.len() as u64;
    let projection_sha256 = serialized_digest(&selected, limits.max_relation_bytes, b"")?;
    let derived_review = ReviewInputV1 {
        schema_version: REVIEW_INPUT_SCHEMA_VERSION.into(),
        root_identity: expected.subject.logical_root.clone(),
        base_sha: expected.subject.base_commit.clone(),
        head_sha: expected.subject.head_commit.clone(),
        head_tree: expected.subject.head_tree.clone(),
        check_sha256: manifest.artifacts[4].sha256.clone(),
        canonical_diff_sha256: manifest.artifacts[1].sha256.clone(),
        mode: expected.effective_options.mode.as_str().into(),
        analysis_complete: true,
        total_finding_count: total,
        projected_finding_count: projected_count,
        projection_limit: REVIEW_INPUT_PROJECTION_LIMIT,
        projection_truncated: projected_count < total,
        projection_selection_policy: REVIEW_INPUT_SELECTION_POLICY.into(),
        projection_selection_policy_version: REVIEW_INPUT_SELECTION_POLICY_VERSION.into(),
        reviewed_count: projected_count,
        projection_sha256,
        findings: selected,
        analysis_outcome: Some(
            check
                .get("analysis_outcome")
                .ok_or("missing outcome")?
                .clone(),
        ),
    };
    let saved_review: ReviewInputV1 = payload(
        &bytes[6],
        ArtifactRole::ReviewInput,
        &manifest.generation_id,
    )?;
    if saved_review != derived_review {
        return Err(
            "saved review projection/identity/count/outcome differs from actual reducer".into(),
        );
    }
    let options = super::PrEvidenceOptions {
        root: expected.subject.requested_root.clone(),
        base: expected.subject.requested_base.clone(),
        base_explicit: true,
        head: expected.subject.requested_head.clone(),
        check: false,
    };
    let derived_pr = super::pr_evidence_packet(&options, &expected.subject.changed_paths, check);
    serialized_digest(&derived_pr, limits.max_relation_bytes, b"")?;
    let saved_pr: Value = payload(&bytes[7], ArtifactRole::PrJson, &manifest.generation_id)?;
    if saved_pr != derived_pr || saved_pr.get("status").and_then(Value::as_str) != Some("advisory")
    {
        return Err("saved PR JSON differs from actual successful advisory reducer".into());
    }
    let markdown = super::render_pr_evidence_markdown(&derived_pr);
    if markdown.len() as u64 > limits.max_relation_bytes {
        return Err("actual Markdown relation exceeds finite admission".into());
    }
    let prefix = markdown_prefix(&manifest.generation_id);
    let markdown_bytes = &bytes[8];
    if !markdown_bytes.starts_with(prefix.as_bytes())
        || markdown_bytes.get(prefix.len()..) != Some(markdown.as_bytes())
    {
        return Err(
            "saved Markdown differs from actual reducer or complete marker was removed".into(),
        );
    }
    authority.ensure_clean().map_err(|e| e.to_string())?;
    // Rehash retained handles and re-open relative to the retained directory.
    // Equal-size replacements, changed bytes, removed markers/manifest, extra
    // names and changes to the caller's root path revoke the saved proof.
    manifest_file.read(None, Some(&manifest_sha256))?;
    directory.verify_same_file(MANIFEST_FILE, &manifest_file.identity, &manifest_sha256)?;
    for (file, descriptor) in files.iter_mut().zip(&manifest.artifacts) {
        file.read(None, Some(&descriptor.sha256))?;
        directory.verify_same_file(&descriptor.path, &file.identity, &descriptor.sha256)?;
    }
    directory.verify_closed()?;
    directory.verify_root_current()?;
    authority.ensure_clean().map_err(|e| e.to_string())?;
    Ok(VerifiedGeneration {
        generation_id: manifest.generation_id,
        manifest_sha256,
        artifact_sha256: std::array::from_fn(|i| manifest.artifacts[i].sha256.clone()),
        total_finding_count: total,
        manifest_identity: (
            manifest_file.identity.device, manifest_file.identity.inode,
            manifest_file.identity.bytes,
        ),
        artifact_identity: std::array::from_fn(|i| (
            files[i].identity.device, files[i].identity.inode, files[i].identity.bytes,
        )),
    })
}

fn raw_limits(limits: &CompleteVerificationLimits) -> Result<RawCoverageLimits, String> {
    let native =
        |value| usize::try_from(value).map_err(|_| "raw bound exceeds native usize".to_string());
    Ok(RawCoverageLimits {
        file_limit: native(limits.file_limit)?,
        max_raw_bytes: native(limits.max_artifact_bytes[0])?,
        max_records: native(limits.max_raw_records)?,
        max_ledger_bytes: native(limits.max_artifact_bytes[3])?,
        max_retained_path_bytes: native(limits.max_retained_path_bytes)?,
        max_projection_bytes: native(limits.max_raw_projection_bytes)?,
    })
}

fn verify_raw_summary(summary: &RawCoverageSummary, expected: &RawBinding) -> Result<(), String> {
    let actual = RawBinding {
        raw_sha256: summary.raw_sha256.clone(),
        ledger_sha256: summary.ledger_sha256.clone(),
        projection_sha256: summary.projection_sha256.clone(),
        raw_bytes: summary.raw_bytes as u64,
        ledger_bytes: summary.ledger_bytes as u64,
        records: summary.records as u64,
        sections: summary.sections as u64,
        hunks: summary.hunks as u64,
        changed_files: summary.changed_files as u64,
        added_lines: summary.added_lines as u64,
        removed_lines: summary.removed_lines as u64,
    };
    if actual != *expected {
        return Err("raw replay digest/count/EOF identity differs from expected binding".into());
    }
    Ok(())
}

fn verify_committed_request(
    authority: &frozen::FrozenSourceAuthority,
    expected: &CompleteBinding,
) -> Result<(), String> {
    let request = &expected.committed_request;
    let (_, actual) = authority
        .inventory()
        .files()
        .find(|(path, _)| *path == Path::new(POLICY_PATH))
        .ok_or("committed request is missing from frozen inventory")?;
    use std::fmt::Write as _;
    let mut digest = String::from("sha256:");
    for byte in actual.sha256 {
        write!(&mut digest, "{byte:02x}").map_err(|error| error.to_string())?;
    }
    if actual.mode.git_mode() != "100644"
        || actual.mode.git_mode() != request.git_mode
        || actual.blob_oid.as_str() != request.blob_oid
        || actual.size != request.original_bytes.len() as u64
        || digest != request.sha256
    {
        return Err("frozen committed request mode/blob/size/digest differs from binding".into());
    }
    let original = frozen::fs::read_with_limit(
        authority.logical_root().join(POLICY_PATH),
        request.original_bytes.len() as u64,
    )
    .map_err(|error| format!("read original frozen committed request: {error}"))?;
    if original.as_slice() != request.original_bytes.as_slice() {
        return Err("original frozen committed request bytes differ from binding".into());
    }
    Ok(())
}

fn verify_inventory(
    authority: &frozen::FrozenSourceAuthority,
    expected: &CompleteBinding,
) -> Result<(), String> {
    let inventory = authority.inventory();
    if inventory.files().len() != expected.inventory.files.len()
        || inventory.directories().len() != expected.inventory.directories.len()
    {
        return Err("frozen full inventory cardinality differs from complete binding".into());
    }
    for ((path, actual), saved) in inventory.files().zip(&expected.inventory.files) {
        use std::fmt::Write as _;
        let mut digest = String::from("sha256:");
        for byte in actual.sha256 {
            write!(&mut digest, "{byte:02x}").map_err(|e| e.to_string())?;
        }
        if path.to_str() != Some(saved.path.as_str())
            || actual.mode.git_mode() != saved.git_mode
            || actual.blob_oid.as_str() != saved.blob_oid
            || actual.size != saved.bytes
            || digest != saved.sha256
        {
            return Err(
                "frozen inventory path/mode/blob/size/digest differs from complete binding".into(),
            );
        }
    }
    for (actual, saved) in inventory.directories().zip(&expected.inventory.directories) {
        if actual.to_str() != Some(saved.as_str()) {
            return Err("frozen directory inventory differs from binding".into());
        }
    }
    Ok(())
}

fn verify_configuration(
    authority: &frozen::FrozenSourceAuthority,
    expected: &CompleteBinding,
) -> Result<(), String> {
    use crate::analysis::git_candidate_execution::CapturedConfiguration;
    match (authority.captured_configuration(), &expected.configuration) {
        (CapturedConfiguration::Absent, ConfigurationBinding::Absent) => {}
        (
            CapturedConfiguration::Present { blob_oid, text },
            ConfigurationBinding::Present {
                blob_oid: saved_oid,
                sha256,
                text: saved_text,
            },
        ) if blob_oid.as_str() == saved_oid
            && text == saved_text
            && sha256_bytes(text.as_bytes()) == *sha256 => {}
        _ => return Err("captured config state/blob/text differs from binding".into()),
    }
    let root = authority.logical_root();
    let config = crate::config::config_for_captured_snapshot(
        root,
        &root.join("ripr.toml"),
        authority.captured_configuration(),
    )?;
    let options = &expected.effective_options;
    let full = FullConfiguration::capture(&config, &expected.profile)?;
    if full != expected.full_configuration {
        return Err(
            "every-field typed configuration/defaults/provenance differs from binding".into(),
        );
    }
    let mut languages = config
        .languages()
        .enabled()
        .iter()
        .map(|l| l.as_str().to_string())
        .collect::<Vec<_>>();
    languages.sort();
    languages.dedup();
    let mut fields = config
        .check_artifact_identity_fields()
        .into_iter()
        .filter(|field| field.role == crate::config::ConfigIdentityRole::FindingAffecting)
        .map(|field| PolicyField {
            name: field.name.into(),
            value: field.value.unwrap_or_default(),
        })
        .collect::<Vec<_>>();
    fields.sort_by(|a, b| a.name.cmp(&b.name));
    // Same installed producer defaults and config application, without running
    // the checker. Xtask's supplied-diff options must be captured by its caller
    // and match this route's mode/include policy; overrides need a new schema.
    let mode = config
        .analysis()
        .mode()
        .map_or("draft", |mode| mode.as_str());
    let include = match options.surface {
        ProducerSurface::Installed => config.analysis().include_unchanged_tests().unwrap_or(true),
        // Actual xtask run_ripr_check passes --no-unchanged-tests explicitly.
        ProducerSurface::Xtask => false,
    };
    let declared_input_base = match options.surface {
        ProducerSurface::Installed => None,
        ProducerSurface::Xtask => Some(expected.subject.requested_base.as_str()),
    };
    if options.check_input_base.as_deref() != declared_input_base
        || (options.surface == ProducerSurface::Installed && options.git_timeout_ms.is_some())
    {
        return Err("actual supplied-diff declared input surface differs from binding".into());
    }
    // Declared input base is serialized separately. The supplied canonical
    // loader still reports effective base None in check/outcome below.
    if options.config_identity_version != crate::config::CHECK_ARTIFACT_CONFIG_IDENTITY_VERSION
        || options.config_identity_hash
            != crate::config::check_artifact_config_identity_hash(&config)
        || options.loaded_config_identity != crate::config::loaded_config_identity(&config)
        || options.enabled_languages != languages
        || options.finding_affecting_fields != fields
        || options.mode.as_str() != mode
        || options.include_unchanged_tests != include
    {
        return Err("actual captured configuration/effective policy differs from binding".into());
    }
    authority.ensure_clean().map_err(|e| e.to_string())
}

fn validate_check<'a>(
    check: &'a Value,
    expected: &CompleteBinding,
    raw: &RawCoverageSummary,
    diff_digest: &str,
) -> Result<&'a [Value], String> {
    let rendered_root = super::command_root_path(
        Path::new(&expected.subject.invocation_repository),
        &expected.subject.requested_root,
    );
    if check.get("schema_version").and_then(Value::as_str)
        != Some(expected.analyzer.check_schema.as_str())
        || check.get("tool").and_then(Value::as_str) != Some("ripr")
        || check.get("mode").and_then(Value::as_str)
            != Some(expected.effective_options.mode.as_str())
        || check.get("root").and_then(Value::as_str) != rendered_root.to_str()
        || check.get("base").is_some_and(|base| !base.is_null())
        || check
            .get("partial_scope")
            .is_some_and(|scope| !scope.is_null())
        || check.get("analysis_scope").is_some()
        || check.get("findings_bound").is_some()
        || check.get("suppression").is_some()
        || check.get("suppression_policy").is_some()
        || check
            .get("no_scope_provided")
            .is_some_and(|v| v != &Value::Bool(false))
    {
        return Err("check schema/root/mode/scope is contradictory, partial or bounded".into());
    }
    if check
        .get("language_runs")
        .is_some_and(|v| v.as_array().is_none_or(|a| !a.is_empty()))
        || check
            .get("run_limitations")
            .is_some_and(|v| v.as_array().is_none_or(|a| !a.is_empty()))
    {
        return Err("check carries an unsuccessful language run or render limitation".into());
    }
    let findings = check
        .get("findings")
        .and_then(Value::as_array)
        .ok_or("check findings must be an array")?;
    if findings.len() > crate::review_input::REVIEW_INDEX_MAX_ENTRIES {
        return Err("full finding set exceeds canonical index admission".into());
    }
    let projection = check
        .get("analysis_outcome")
        .ok_or("complete check outcome is missing")?;
    require_keys(projection, &["analysis_complete", "outcome"])?;
    if projection.get("analysis_complete") != Some(&Value::Bool(true)) {
        return Err("check analysis did not complete".into());
    }
    let wire = projection.get("outcome").ok_or("missing typed outcome")?;
    strict_outcome_keys(wire)?;
    let outcome: AnalysisOutcome = serde_json::from_value(wire.clone())
        .map_err(|e| format!("validate actual typed outcome: {e}"))?;
    let changed_lines = (raw.added_lines as u64)
        .checked_add(raw.removed_lines as u64)
        .ok_or("raw changed-line count overflow")?;
    if !outcome.kind.is_complete()
        || outcome.counts.changed_file_count != raw.changed_files as u64
        || outcome.counts.changed_line_count != changed_lines
        || outcome.counts.finding_count != findings.len() as u64
        || outcome.counts.probe_count != findings.len() as u64
        || outcome.counts.candidate_line_count > changed_lines
        || outcome.identity.input_identity.as_deref() != Some(diff_digest)
        || outcome.identity.config_identity != expected.effective_options.loaded_config_identity
        || outcome.identity.base_revision.is_some()
        || outcome.identity.git_candidate_subject.is_some()
        || outcome.identity.repository_identity.is_some()
        || outcome.identity.root_identity.is_some()
        || outcome.identity.snapshot_identity.is_some()
    {
        return Err(
            "actual outcome kind/counts/input/config/source identity is contradictory".into(),
        );
    }
    verify_summary(
        check.get("summary").ok_or("missing check summary")?,
        findings,
    )?;
    Ok(findings)
}

fn verify_summary(summary: &Value, findings: &[Value]) -> Result<(), String> {
    // Adapter-local changed_rust_files/changed_files_by_language are retained
    // forensic output. They are not raw denominators or completion authority:
    // those come from exact replay and validated AnalysisOutcome counts. Their
    // derivation needs analyzer-local selection, which saved verification must
    // not rerun. All full-finding class/probe counts below are independently
    // reduced; an unbound suppression count is refused.
    if summary.get("suppressed_by_policy").is_some() {
        return Err("unbound suppressed count in check summary".into());
    }
    let classes = [
        "exposed",
        "weakly_exposed",
        "reachable_unrevealed",
        "no_static_path",
        "infection_unknown",
        "propagation_unknown",
        "static_unknown",
    ];
    let mut counts = [0u64; 7];
    let mut ids = std::collections::BTreeSet::new();
    for finding in findings {
        let id = finding
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or("missing finding ID")?;
        if !ids.insert(id) {
            return Err("duplicate full finding ID".into());
        }
        if finding
            .get("suppressed")
            .is_some_and(|s| s != &Value::Bool(false))
        {
            return Err("unbound suppression in check finding".into());
        }
        let class = finding
            .get("classification")
            .and_then(Value::as_str)
            .ok_or("missing finding class")?;
        let index = classes
            .iter()
            .position(|expected| *expected == class)
            .ok_or("unknown finding class")?;
        counts[index] = counts[index]
            .checked_add(1)
            .ok_or("finding class count overflow")?;
    }
    for (class, count) in classes.into_iter().zip(counts) {
        if summary.get(class).and_then(Value::as_u64) != Some(count) {
            return Err(format!(
                "actual summary.{class} differs from full finding set"
            ));
        }
    }
    for name in ["findings", "probes"] {
        if summary.get(name).and_then(Value::as_u64) != Some(findings.len() as u64) {
            return Err(format!("actual summary.{name} differs from full findings"));
        }
    }
    Ok(())
}

fn strict_outcome_keys(wire: &Value) -> Result<(), String> {
    require_keys(
        wire,
        &[
            "schema_version",
            "kind",
            "identity",
            "counts",
            "limitations",
            "claim_boundary",
        ],
    )?;
    require_keys(
        &wire["identity"],
        &[
            "repository_identity",
            "root_identity",
            "config_identity",
            "base_revision",
            "input_identity",
            "snapshot_identity",
            "git_candidate_subject",
        ],
    )?;
    require_keys(
        &wire["counts"],
        &[
            "changed_file_count",
            "changed_line_count",
            "candidate_line_count",
            "probe_count",
            "finding_count",
        ],
    )?;
    for limitation in wire["limitations"]
        .as_array()
        .ok_or("outcome limitations must be an array")?
    {
        require_keys(
            limitation,
            &[
                "kind",
                "producer_stage",
                "path",
                "affected_items",
                "bounded_detail",
                "recovery",
            ],
        )?;
        require_keys(&limitation["recovery"], &["kind", "detail"])?;
    }
    Ok(())
}

fn require_keys(value: &Value, names: &[&str]) -> Result<(), String> {
    let map = value.as_object().ok_or("strict data must be an object")?;
    if map.len() != names.len() || names.iter().any(|name| !map.contains_key(*name)) {
        return Err("strict data contains unknown or missing fields".into());
    }
    Ok(())
}

/// Logical verifier-buffer DATA only; caller-retained buffers are additional.
/// Reused by saved verification and the genuine factory's combined phase.
pub(super) fn artifact_buffer_allowance(
    manifest_bytes: u64,
    artifact_bytes: &[u64; 9],
    limits: &CompleteVerificationLimits,
) -> Result<u64, String> {
    let mut retained = json_allowance(manifest_bytes)?;
    for (bytes, role) in artifact_bytes.iter().zip(ArtifactRole::ALL) {
        retained = retained
            .checked_add(*bytes)
            .ok_or("retained buffer total overflow")?;
        if matches!(
            role,
            ArtifactRole::FullCheck
                | ArtifactRole::FindingIndex
                | ArtifactRole::ReviewInput
                | ArtifactRole::PrJson
        ) {
            retained = retained
                .checked_add(json_allowance(*bytes)?)
                .ok_or("retained JSON representation total overflow")?;
        }
    }
    retained = retained
        .checked_add(
            limits
                .max_relation_bytes
                .checked_mul(8)
                .ok_or("reducer relation allowance overflow")?,
        )
        .ok_or("retained relation total overflow")?;
    // Existing Vec-consuming decoder + worst-case lossy UTF8 expansion.
    let copies = artifact_bytes[0]
        .checked_mul(4)
        .ok_or("raw decoder expansion overflow")?;
    retained = retained
        .checked_add(copies)
        .ok_or("retained decoder byte total overflow")?;
    Ok(retained)
}

fn admit_buffers(
    manifest: &CompleteManifest,
    manifest_bytes: u64,
    limits: &CompleteVerificationLimits,
) -> Result<(), String> {
    // manifest.validate has already established this exact closed order.
    let bytes = std::array::from_fn(|index| manifest.artifacts[index].bytes);
    let retained = artifact_buffer_allowance(manifest_bytes, &bytes, limits)?;
    if retained > limits.max_buffered_bytes {
        return Err("artifact/JSON/raw/reducer buffer admission exceeded".into());
    }
    Ok(())
}

fn json_allowance(wire_bytes: u64) -> Result<u64, String> {
    wire_bytes
        .checked_mul(64)
        .ok_or("JSON representation allowance overflow".into())
}

fn payload<T: DeserializeOwned>(
    bytes: &[u8],
    role: ArtifactRole,
    generation: &str,
) -> Result<T, String> {
    let payload: CompletePayload<T> = strict_json(bytes)?;
    if payload.schema_version != PAYLOAD_SCHEMA
        || payload.role != role
        || payload.generation_id != generation
    {
        return Err("complete payload role/schema/generation marker mismatch or downgrade".into());
    }
    Ok(payload.value)
}

// Value's normal deserializer accepts duplicate keys. Reject them recursively
// before converting strict envelopes and production values, including index
// entries and identities. serde_json's recursion limit remains enabled.
struct JsonBudget {
    remaining: u64,
}
impl JsonBudget {
    fn claim<E: serde::de::Error>(&mut self, bytes: u64) -> Result<(), E> {
        self.remaining = self
            .remaining
            .checked_sub(bytes)
            .ok_or_else(|| E::custom("JSON representation admission exceeded"))?;
        Ok(())
    }
}
struct UniqueSeed<'a> {
    budget: &'a mut JsonBudget,
    array_limit: usize,
}
impl<'de> DeserializeSeed<'de> for UniqueSeed<'_> {
    type Value = Value;
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        self.budget.claim::<D::Error>(64)?;
        struct UniqueVisitor<'a> {
            budget: &'a mut JsonBudget,
            array_limit: usize,
        }
        impl<'de> Visitor<'de> for UniqueVisitor<'_> {
            type Value = Value;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("JSON without duplicate object keys")
            }
            fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Self::Value, E> {
                Ok(Value::Bool(v))
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
                Ok(v.into())
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
                Ok(v.into())
            }
            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Self::Value, E> {
                serde_json::Number::from_f64(v)
                    .map(Value::Number)
                    .ok_or_else(|| E::custom("nonfinite JSON number"))
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                self.budget.claim::<E>(v.len() as u64)?;
                Ok(Value::String(v.into()))
            }
            fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Self::Value, E> {
                self.budget.claim::<E>(v.len() as u64)?;
                Ok(Value::String(v))
            }
            fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(Value::Null)
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(Value::Null)
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                loop {
                    // Reject the next node before constructing it or growing
                    // this Vec. A completed exact-limit array may still end.
                    let next_limit = if values.len() == self.array_limit {
                        0
                    } else {
                        usize::MAX
                    };
                    let next = a.next_element_seed(UniqueSeed {
                        budget: &mut *self.budget,
                        array_limit: next_limit,
                    })?;
                    let Some(v) = next else { break };
                    if values.len() == self.array_limit {
                        return Err(serde::de::Error::custom(
                            "JSON array entry admission exceeded",
                        ));
                    }
                    values
                        .try_reserve_exact(1)
                        .map_err(serde::de::Error::custom)?;
                    values.push(v);
                }
                Ok(Value::Array(values))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Self::Value, A::Error> {
                let mut values = serde_json::Map::new();
                while let Some(k) = a.next_key::<String>()? {
                    if values.contains_key(&k) {
                        return Err(serde::de::Error::custom("duplicate JSON object key"));
                    }
                    self.budget.claim::<A::Error>(
                        (k.len() as u64).checked_add(128).ok_or_else(|| {
                            serde::de::Error::custom("JSON key admission overflow")
                        })?,
                    )?;
                    let array_limit = match k.as_str() {
                        "artifacts" => 9,
                        "entries" | "findings" => crate::review_input::REVIEW_INDEX_MAX_ENTRIES,
                        _ => usize::MAX,
                    };
                    let v = a.next_value_seed(UniqueSeed {
                        budget: &mut *self.budget,
                        array_limit,
                    })?;
                    values.insert(k, v);
                }
                Ok(Value::Object(values))
            }
        }
        if self.array_limit == 0 {
            return Err(serde::de::Error::custom(
                "JSON array entry admission exceeded",
            ));
        }
        d.deserialize_any(UniqueVisitor {
            budget: self.budget,
            array_limit: self.array_limit,
        })
    }
}
fn strict_json<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, String> {
    let mut budget = JsonBudget {
        remaining: json_allowance(bytes.len() as u64)?,
    };
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    let value = UniqueSeed {
        budget: &mut budget,
        array_limit: usize::MAX,
    }
    .deserialize(&mut decoder)
    .map_err(|e| format!("strict complete JSON: {e}"))?;
    decoder
        .end()
        .map_err(|e| format!("strict complete JSON trailing input: {e}"))?;
    serde_json::from_value(value).map_err(|e| format!("strict complete schema: {e}"))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileIdentity {
    device: u64,
    inode: u64,
    bytes: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

fn identity(metadata: &Metadata, directory: bool) -> Result<FileIdentity, String> {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.file_type().is_symlink()
            || (directory && !metadata.is_dir())
            || (!directory && (!metadata.is_file() || metadata.nlink() != 1))
        {
            return Err("staging entry is not a regular nonlink unaliased file/directory".into());
        }
        Ok(FileIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
            bytes: metadata.len(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (metadata, directory);
        Err("complete staging verification requires qualified Linux no-follow descriptors".into())
    }
}

fn open_no_follow(path: &Path, directory: bool) -> Result<File, String> {
    #[cfg(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // SAME kernel ABI flags as the retained SourceAnchor owner.
        #[cfg(target_arch = "x86_64")]
        let (no_follow, directory_flag) = (0x0002_0000, 0x0001_0000);
        #[cfg(target_arch = "aarch64")]
        let (no_follow, directory_flag) = (0x0000_8000, 0x0000_4000);
        OpenOptions::new()
            .read(true)
            .custom_flags(no_follow | 0x0000_0800 | if directory { directory_flag } else { 0 })
            .open(path)
            .map_err(|e| format!("open no-follow staging descriptor: {e}"))
    }
    #[cfg(not(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )))]
    {
        let _ = (path, directory);
        Err("complete no-follow staging target is unsupported".into())
    }
}

struct StagedDirectory {
    root: PathBuf,
    handle: File,
    identity: FileIdentity,
    anchored: PathBuf,
}
impl StagedDirectory {
    fn open(path: &Path) -> Result<Self, String> {
        let root = std::path::absolute(path).map_err(|e| e.to_string())?;
        if root
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        {
            return Err("noncanonical staging root path".into());
        }
        // Anchor each directory below the previous opened descriptor. Only
        // /proc/self/fd links naming OUR retained descriptors are traversed.
        let mut current = open_no_follow(Path::new("/"), true)?;
        for component in root.components() {
            match component {
                Component::RootDir => {}
                Component::Normal(name) => {
                    let next = descriptor_path(&current)?.join(name);
                    let opened = open_no_follow(&next, true)?;
                    identity(&opened.metadata().map_err(|e| e.to_string())?, true)?;
                    current = opened;
                }
                _ => return Err("unsupported staging root component".into()),
            }
        }
        let id = identity(&current.metadata().map_err(|e| e.to_string())?, true)?;
        let anchored = descriptor_path(&current)?;
        let result = Self {
            root,
            handle: current,
            identity: id,
            anchored,
        };
        result.verify_root_current()?;
        Ok(result)
    }
    fn verify_root_current(&self) -> Result<(), String> {
        let current = identity(
            &std::fs::symlink_metadata(&self.root).map_err(|e| e.to_string())?,
            true,
        )?;
        let opened = identity(&self.handle.metadata().map_err(|e| e.to_string())?, true)?;
        if current != self.identity || opened != self.identity {
            return Err("staging root identity changed during verification".into());
        }
        Ok(())
    }
    fn verify_closed(&self) -> Result<(), String> {
        let mut names = std::collections::BTreeSet::new();
        for entry in std::fs::read_dir(&self.anchored).map_err(|e| e.to_string())? {
            if names.len() >= 10 {
                return Err("extra staging entry exceeds closed manifest set".into());
            }
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| "nonUTF8 extra staged name")?;
            if name != MANIFEST_FILE && !ArtifactRole::ALL.iter().any(|r| r.path() == name) {
                return Err("unknown extra staged artifact".into());
            }
            identity(
                &std::fs::symlink_metadata(entry.path()).map_err(|e| e.to_string())?,
                false,
            )?;
            if !names.insert(name) {
                return Err("duplicate staged artifact name".into());
            }
        }
        if names.len() != 10 {
            return Err("missing complete manifest/artifact".into());
        }
        self.verify_root_current()
    }
    fn open_file(&self, name: &str, cap: u64) -> Result<StagedFile, String> {
        if name != MANIFEST_FILE && !ArtifactRole::ALL.iter().any(|r| r.path() == name) {
            return Err("unregistered artifact path".into());
        }
        let path = self.anchored.join(name);
        let before = identity(
            &std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?,
            false,
        )?;
        if before.bytes > cap {
            return Err("staged file exceeds byte admission before open/read/growth".into());
        }
        let handle = open_no_follow(&path, false)?;
        let opened = identity(&handle.metadata().map_err(|e| e.to_string())?, false)?;
        if before != opened {
            return Err("staged file was replaced while opening".into());
        }
        Ok(StagedFile {
            handle,
            identity: opened,
        })
    }
    fn verify_same_file(
        &self,
        name: &str,
        expected: &FileIdentity,
        digest: &str,
    ) -> Result<(), String> {
        let mut file = self.open_file(name, expected.bytes)?;
        if &file.identity != expected {
            return Err("staged artifact was replaced during verification".into());
        }
        file.read(None, Some(digest))?;
        Ok(())
    }
}

fn descriptor_path(file: &File) -> Result<PathBuf, String> {
    #[cfg(target_os = "linux")]
    {
        use std::os::fd::AsRawFd;
        Ok(PathBuf::from(format!("/proc/self/fd/{}", file.as_raw_fd())))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = file;
        Err("descriptor-relative staging unavailable".into())
    }
}

struct StagedFile {
    handle: File,
    identity: FileIdentity,
}
impl StagedFile {
    fn read(
        &mut self,
        retain: Option<u64>,
        expected_digest: Option<&str>,
    ) -> Result<(Vec<u8>, String), String> {
        if identity(&self.handle.metadata().map_err(|e| e.to_string())?, false)? != self.identity {
            return Err("opened artifact metadata changed".into());
        }
        if retain.is_some_and(|cap| self.identity.bytes > cap) {
            return Err("artifact buffer byte admission exceeded".into());
        }
        self.handle
            .seek(SeekFrom::Start(0))
            .map_err(|e| e.to_string())?;
        let mut bytes = Vec::new();
        let mut scratch = [0u8; 64 * 1024];
        let mut hash = Sha256::new();
        let mut count = 0u64;
        loop {
            let read = self.handle.read(&mut scratch).map_err(|e| e.to_string())?;
            if read == 0 {
                break;
            }
            count = count
                .checked_add(read as u64)
                .filter(|n| *n <= self.identity.bytes)
                .ok_or("staged artifact grew/overflowed")?;
            hash.update(&scratch[..read]);
            if let Some(cap) = retain {
                if count > cap {
                    return Err("staged artifact byte cap exceeded before buffer growth".into());
                }
                bytes
                    .try_reserve_exact(read)
                    .map_err(|e| format!("reserve staged bytes: {e}"))?;
                bytes.extend_from_slice(&scratch[..read]);
            }
        }
        if count != self.identity.bytes
            || identity(&self.handle.metadata().map_err(|e| e.to_string())?, false)?
                != self.identity
        {
            return Err("staged artifact shrank or changed during read".into());
        }
        let digest = format!("sha256:{:x}", hash.finalize());
        if expected_digest.is_some_and(|expected| expected != digest) {
            return Err("actual streamed artifact digest mismatch".into());
        }
        Ok((bytes, digest))
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "linux")]
    use super::super::complete_contract::tests::fixture_policy_bytes;
    use super::super::complete_contract::tests::{
        fixture_binding, fixture_manifest, require_error,
    };
    #[cfg(target_os = "linux")]
    use super::super::complete_request::CommittedRequestBinding;
    use super::*;

    #[test]
    fn payload_rejects_duplicate_unknown_stale_role_and_removed_markers() -> Result<(), String> {
        let generation = fixture_binding()?.generation_id()?;
        let good = serde_json::json!({"schema_version": PAYLOAD_SCHEMA, "generation_id": generation, "role": "pr_json", "value": {"status":"advisory"}});
        let bytes = serde_json::to_vec(&good).map_err(|e| e.to_string())?;
        assert_eq!(
            payload::<Value>(&bytes, ArtifactRole::PrJson, &generation)?["status"],
            "advisory"
        );
        let error = require_error(
            payload::<Value>(&bytes, ArtifactRole::FullCheck, &generation),
            "mismatched payload role was accepted",
        )?;
        assert_eq!(
            error,
            "complete payload role/schema/generation marker mismatch or downgrade"
        );
        let error = require_error(
            payload::<Value>(&bytes, ArtifactRole::PrJson, "stale"),
            "stale payload generation was accepted",
        )?;
        assert_eq!(
            error,
            "complete payload role/schema/generation marker mismatch or downgrade"
        );
        let _error = require_error(
            payload::<Value>(
                b"{\"status\":\"advisory\"}",
                ArtifactRole::PrJson,
                &generation,
            ),
            "unexpected success in payload_rejects_duplicate_unknown_stale_role_and_removed_markers",
        )?;
        let error = require_error(
            strict_json::<Value>(b"{\"a\":{\"x\":1,\"x\":2}}"),
            "nested duplicate JSON key was accepted",
        )?;
        assert!(error.contains("duplicate JSON object key"));
        let mut wrong = good.clone();
        wrong["permission"] = Value::Bool(true);
        let _error = require_error(
            payload::<Value>(
                &serde_json::to_vec(&wrong).map_err(|e| e.to_string())?,
                ArtifactRole::PrJson,
                &generation,
            ),
            "unexpected success in payload_rejects_duplicate_unknown_stale_role_and_removed_markers",
        )?;
        Ok(())
    }

    #[test]
    fn buffer_admission_counts_original_raw_decoder_copy_and_expansion() -> Result<(), String> {
        let binding = fixture_binding()?;
        let mut manifest = fixture_manifest(binding.clone())?;
        manifest.artifacts[0].bytes = 100;
        let mut limits = binding.profile.clone();
        let required = 506 + 8 * limits.max_relation_bytes;
        limits.max_buffered_bytes = required - 1;
        let _error = require_error(
            admit_buffers(&manifest, 0, &limits),
            "unexpected success in buffer_admission_counts_original_raw_decoder_copy_and_expansion",
        )?;
        limits.max_buffered_bytes = required;
        admit_buffers(&manifest, 0, &limits)?;
        manifest.artifacts[4].bytes = 1;
        limits.max_buffered_bytes = required + 64;
        let _error = require_error(
            admit_buffers(&manifest, 0, &limits),
            "unexpected success in buffer_admission_counts_original_raw_decoder_copy_and_expansion",
        )?;
        limits.max_buffered_bytes += 1;
        admit_buffers(&manifest, 0, &limits)?;
        manifest.artifacts[0].bytes = u64::MAX;
        let _error = require_error(
            admit_buffers(&manifest, 0, &limits),
            "unexpected success in buffer_admission_counts_original_raw_decoder_copy_and_expansion",
        )?;
        Ok(())
    }

    #[test]
    fn full_finding_summary_refuses_counts_duplicate_ids_and_false_zero() -> Result<(), String> {
        let findings = vec![
            serde_json::json!({"id":"a", "classification":"weakly_exposed"}),
            serde_json::json!({"id":"b", "classification":"no_static_path"}),
        ];
        let summary = serde_json::json!({"probes":2,"findings":2,"exposed":0,"weakly_exposed":1,"reachable_unrevealed":0,"no_static_path":1,"infection_unknown":0,"propagation_unknown":0,"static_unknown":0});
        verify_summary(&summary, &findings)?;
        let mut wrong = summary.clone();
        wrong["weakly_exposed"] = 0.into();
        let _error = require_error(
            verify_summary(&wrong, &findings),
            "unexpected success in full_finding_summary_refuses_counts_duplicate_ids_and_false_zero",
        )?;
        let duplicate = vec![findings[0].clone(), findings[0].clone()];
        let _error = require_error(
            verify_summary(&summary, &duplicate),
            "unexpected success in full_finding_summary_refuses_counts_duplicate_ids_and_false_zero",
        )?;
        let _error = require_error(
            verify_summary(&summary, &[]),
            "unexpected success in full_finding_summary_refuses_counts_duplicate_ids_and_false_zero",
        )?;
        Ok(())
    }

    #[test]
    fn exact_raw_replay_refuses_occurrence_eof_and_coalescing_corruption() -> Result<(), String> {
        use super::super::raw_coverage::build_raw_coverage;
        let raw = b"diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -1 +1 @@\n-old\n+new\ndiff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -3 +3 @@\n-before\n+after\n";
        let binding = fixture_binding()?;
        let limits = raw_limits(&binding.profile)?;
        let (_, coverage) = build_raw_coverage(raw, limits)?;
        let ledger = coverage.ledger_bytes();
        let (_, summary) = verify_raw_coverage(raw, ledger, limits)?;
        assert_eq!(summary.changed_files, 1);
        assert_eq!(summary.added_lines, 2);
        assert_eq!(summary.removed_lines, 2);
        for changed in [
            ledger[..ledger.len() - 1].to_vec(),
            [ledger, ledger].concat(),
        ] {
            let _error = require_error(
                verify_raw_coverage(raw, &changed, limits),
                "unexpected success in exact_raw_replay_refuses_occurrence_eof_and_coalescing_corruption",
            )?;
        }
        let mut changed_raw = raw.to_vec();
        changed_raw[0] = b'D';
        let _error = require_error(
            verify_raw_coverage(&changed_raw, ledger, limits),
            "unexpected success in exact_raw_replay_refuses_occurrence_eof_and_coalescing_corruption",
        )?;
        let mut expected = binding.raw;
        expected.raw_sha256 = summary.raw_sha256.clone();
        let _error = require_error(
            verify_raw_summary(&summary, &expected),
            "unexpected success in exact_raw_replay_refuses_occurrence_eof_and_coalescing_corruption",
        )?;
        Ok(())
    }

    #[test]
    fn json_entry_and_representation_admission_precede_collection_growth() -> Result<(), String> {
        let nine = serde_json::json!({"artifacts": vec![0; 9]});
        strict_json::<Value>(&serde_json::to_vec(&nine).map_err(|e| e.to_string())?)?;
        let ten = serde_json::json!({"artifacts": vec![0; 10]});
        let _error = require_error(
            strict_json::<Value>(&serde_json::to_vec(&ten).map_err(|e| e.to_string())?),
            "unexpected success in json_entry_and_representation_admission_precede_collection_growth",
        )?;
        let overflow = serde_json::json!({"findings": vec![0; crate::review_input::REVIEW_INDEX_MAX_ENTRIES + 1]});
        let _error = require_error(
            strict_json::<Value>(&serde_json::to_vec(&overflow).map_err(|e| e.to_string())?),
            "unexpected success in json_entry_and_representation_admission_precede_collection_growth",
        )?;
        let mut budget = JsonBudget { remaining: 63 };
        let mut decoder = serde_json::Deserializer::from_slice(b"[]");
        let _error = require_error(
            UniqueSeed {
                budget: &mut budget,
                array_limit: usize::MAX,
            }
            .deserialize(&mut decoder),
            "unexpected success in json_entry_and_representation_admission_precede_collection_growth",
        )?;
        let _error = require_error(
            json_allowance(u64::MAX),
            "unexpected success in json_entry_and_representation_admission_precede_collection_growth",
        )?;
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn frozen_source_fixture() -> Result<frozen::tests::Fixture, String> {
        frozen::tests::Fixture::new(&[
            ("a.rs", b"value > 1\n"),
            (POLICY_PATH, fixture_policy_bytes()),
        ])
        .map_err(|error| error.to_string())
    }

    /// Core's actual owned frozen fixture, actual config capture and actual
    /// reducers; no analyzer, Git command, execution capability or fake runner.
    #[cfg(target_os = "linux")]
    fn saved_fixture(
        source: &frozen::tests::Fixture,
    ) -> Result<(CompleteBinding, [Vec<u8>; 9]), String> {
        saved_fixture_for_surface(source, ProducerSurface::Installed)
    }

    #[cfg(target_os = "linux")]
    fn saved_fixture_for_surface(
        source: &frozen::tests::Fixture,
        surface: ProducerSurface,
    ) -> Result<(CompleteBinding, [Vec<u8>; 9]), String> {
        use super::super::raw_coverage::build_raw_coverage;
        use crate::analysis::git_candidate_execution::CapturedConfiguration;
        use crate::analysis_outcome::{
            AnalysisIdentity, AnalysisOutcomeCounts, AnalysisOutcomeKind,
        };
        let mut binding = fixture_binding()?;
        binding.effective_options.surface = surface;
        if surface == ProducerSurface::Xtask {
            binding.effective_options.include_unchanged_tests = false;
            binding.effective_options.check_input_base =
                Some(binding.subject.requested_base.clone());
            binding.effective_options.git_timeout_ms = Some(300_000);
        }
        let logical = source
            .logical
            .to_str()
            .ok_or("fixture path is not UTF8")?
            .to_string();
        binding.subject.logical_root = logical.clone();
        binding.subject.work_tree = logical.clone();
        binding.subject.invocation_repository = logical;
        binding.subject.head_tree = source.authority.head_tree().as_str().into();
        binding.subject.changed_paths = vec!["a.rs".into()];
        binding.inventory.files = source
            .authority
            .inventory()
            .files()
            .map(|(path, file)| {
                Ok(InventoryFile {
                    path: path.to_str().ok_or("fixture inventory is not UTF8")?.into(),
                    git_mode: file.mode.git_mode().into(),
                    blob_oid: file.blob_oid.as_str().into(),
                    bytes: file.size,
                    sha256: format!(
                        "sha256:{:x}",
                        Sha256::digest(
                            std::fs::read(source.physical.join(path)).map_err(|e| e.to_string())?
                        )
                    ),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let (_, request_file) = source
            .authority
            .inventory()
            .files()
            .find(|(path, _)| *path == Path::new(POLICY_PATH))
            .ok_or("fixture committed request is missing")?;
        if request_file.mode.git_mode() != "100644" {
            return Err("fixture committed request is not a regular 100644 file".into());
        }
        binding.committed_request = CommittedRequestBinding::from_original_blob(
            request_file.blob_oid.as_str(),
            frozen::fs::read_with_limit(source.logical.join(POLICY_PATH), request_file.size)
                .map_err(|error| error.to_string())?,
        )?;
        binding.inventory.logical_bytes = binding.inventory.files.iter().map(|f| f.bytes).sum();
        binding.inventory.directories = source
            .authority
            .inventory()
            .directories()
            .map(|path| {
                path.to_str()
                    .map(str::to_string)
                    .ok_or("fixture directory is not UTF8".to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let config = crate::config::config_for_captured_snapshot(
            &source.logical,
            &source.logical.join("ripr.toml"),
            &CapturedConfiguration::Absent,
        )?;
        binding.full_configuration = FullConfiguration::capture(&config, &binding.profile)?;
        binding.effective_options.config_identity_hash =
            crate::config::check_artifact_config_identity_hash(&config);
        binding.effective_options.loaded_config_identity =
            crate::config::loaded_config_identity(&config);
        binding.effective_options.enabled_languages = config
            .languages()
            .enabled()
            .iter()
            .map(|l| l.as_str().into())
            .collect();
        binding.effective_options.enabled_languages.sort();
        binding.effective_options.enabled_languages.dedup();
        binding.effective_options.finding_affecting_fields = config
            .check_artifact_identity_fields()
            .into_iter()
            .filter(|f| f.role == crate::config::ConfigIdentityRole::FindingAffecting)
            .map(|f| PolicyField {
                name: f.name.into(),
                value: f.value.unwrap_or_default(),
            })
            .collect();
        binding
            .effective_options
            .finding_affecting_fields
            .sort_by(|a, b| a.name.cmp(&b.name));
        let raw = b"diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -1 +1 @@\n-value > 0\n+value > 1\n";
        let (_, coverage) = build_raw_coverage(raw, raw_limits(&binding.profile)?)?;
        let summary = coverage.summary();
        binding.raw = RawBinding {
            raw_sha256: summary.raw_sha256.clone(),
            ledger_sha256: summary.ledger_sha256.clone(),
            projection_sha256: summary.projection_sha256.clone(),
            raw_bytes: summary.raw_bytes as u64,
            ledger_bytes: summary.ledger_bytes as u64,
            records: summary.records as u64,
            sections: summary.sections as u64,
            hunks: summary.hunks as u64,
            changed_files: summary.changed_files as u64,
            added_lines: summary.added_lines as u64,
            removed_lines: summary.removed_lines as u64,
        };
        binding.presentation = PresentationBinding {
            bytes: raw.len() as u64,
            sha256: sha256_bytes(raw),
        };
        let generation = binding.generation_id()?;
        let diff = crate::analysis::diff::load::decode_diff_text("fixture raw", raw.to_vec())?
            .into_bytes();
        let outcome = AnalysisOutcome::new(
            AnalysisOutcomeKind::CompleteWithFindings,
            AnalysisIdentity {
                input_identity: Some(sha256_bytes(&diff)),
                ..Default::default()
            },
            AnalysisOutcomeCounts {
                changed_file_count: 1,
                changed_line_count: 2,
                candidate_line_count: 1,
                probe_count: 1,
                finding_count: 1,
            },
            vec![],
        )?;
        let finding = serde_json::json!({"id":"one", "severity":"warning", "classification":"weakly_exposed",
            "source_currentness":"candidate_current", "probe":{"file":"a.rs", "line":1, "family":"predicate", "expression":"value > 1"},
            "recommended_next_step":"Add a boundary test.", "related_tests":[], "evidence":{}});
        let check = serde_json::json!({"schema_version":binding.analyzer.check_schema, "tool":"ripr", "mode":"draft",
            "root":super::super::command_root_path(&source.logical, ".").to_str().ok_or("fixture rendered root is not UTF8")?,
            "base":null, "summary":{"probes":1,"findings":1,"changed_rust_files":1,"changed_files_by_language":{"rust":1},
                "exposed":0,"weakly_exposed":1,"reachable_unrevealed":0,"no_static_path":0,"infection_unknown":0,"propagation_unknown":0,"static_unknown":0},
            "analysis_outcome":{"analysis_complete":true,"outcome":outcome}, "findings":[finding]});
        let check_bytes = encode_payload(
            ArtifactRole::FullCheck,
            &generation,
            &CompleteCheck {
                effective_options: binding.effective_options.clone(),
                check: check.clone(),
            },
            &binding.profile,
        )?;
        let findings = check["findings"]
            .as_array()
            .ok_or("fixture findings missing")?;
        let (index, _) = canonical_finding_index(findings, &source.logical)?;
        let selected = canonical_projection_from_index(&index)?;
        assert_eq!(index.entries.len(), 1);
        assert_eq!(index.entries[0].stable_id, "one");
        assert_eq!(index.entries[0].file, "a.rs");
        let review = ReviewInputV1 {
            schema_version: REVIEW_INPUT_SCHEMA_VERSION.into(),
            root_identity: binding.subject.logical_root.clone(),
            base_sha: binding.subject.base_commit.clone(),
            head_sha: binding.subject.head_commit.clone(),
            head_tree: binding.subject.head_tree.clone(),
            check_sha256: sha256_bytes(&check_bytes),
            canonical_diff_sha256: sha256_bytes(&diff),
            mode: "draft".into(),
            analysis_complete: true,
            total_finding_count: 1,
            projected_finding_count: 1,
            projection_limit: REVIEW_INPUT_PROJECTION_LIMIT,
            projection_truncated: false,
            projection_selection_policy: REVIEW_INPUT_SELECTION_POLICY.into(),
            projection_selection_policy_version: REVIEW_INPUT_SELECTION_POLICY_VERSION.into(),
            reviewed_count: 1,
            projection_sha256: serialized_digest(
                &selected,
                binding.profile.max_relation_bytes,
                b"",
            )?,
            findings: selected,
            analysis_outcome: Some(check["analysis_outcome"].clone()),
        };
        let options = super::super::PrEvidenceOptions {
            root: ".".into(),
            base: "base".into(),
            base_explicit: true,
            head: "HEAD".into(),
            check: false,
        };
        let pr = super::super::pr_evidence_packet(&options, &binding.subject.changed_paths, &check);
        assert_eq!(pr["summary"]["severe_gaps"], 1);
        let markdown = format!(
            "{}{}",
            markdown_prefix(&generation),
            super::super::render_pr_evidence_markdown(&pr)
        );
        let artifacts = [
            raw.to_vec(),
            diff,
            raw.to_vec(),
            coverage.ledger_bytes().to_vec(),
            check_bytes,
            encode_payload(
                ArtifactRole::FindingIndex,
                &generation,
                &index,
                &binding.profile,
            )?,
            encode_payload(
                ArtifactRole::ReviewInput,
                &generation,
                &review,
                &binding.profile,
            )?,
            encode_payload(ArtifactRole::PrJson, &generation, &pr, &binding.profile)?,
            markdown.into_bytes(),
        ];
        Ok((binding, artifacts))
    }

    #[cfg(target_os = "linux")]
    fn save_fixture(
        root: &Path,
        binding: &CompleteBinding,
        bytes: &[Vec<u8>; 9],
    ) -> Result<(), String> {
        let manifest = CompleteManifest {
            schema_version: MANIFEST_SCHEMA.into(),
            generation_id: binding.generation_id()?,
            binding: binding.clone(),
            artifacts: ArtifactRole::ALL
                .into_iter()
                .map(|role| ArtifactDescriptor {
                    role,
                    path: role.path().into(),
                    bytes: bytes[role.ordinal()].len() as u64,
                    sha256: sha256_bytes(&bytes[role.ordinal()]),
                })
                .collect(),
        };
        for role in ArtifactRole::ALL {
            std::fs::write(root.join(role.path()), &bytes[role.ordinal()])
                .map_err(|e| e.to_string())?;
        }
        std::fs::write(
            root.join(MANIFEST_FILE),
            encode_manifest(&manifest, binding, &binding.profile)?,
        )
        .map_err(|e| e.to_string())
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn all_nine_nonempty_saved_relations_and_resigned_tampering() -> Result<(), String> {
        let source = frozen_source_fixture()?;
        let stage = stage()?;
        frozen::with_context(Some(source.authority.clone()), || -> Result<(), String> {
            let (binding, original) = saved_fixture(&source)?;
            assert!(original.iter().all(|b| !b.is_empty()));
            save_fixture(&stage.0, &binding, &original)?;
            let proof = verify_staged_generation(&stage.0, &binding, &binding.profile)
                .map_err(|e| e.to_string())?;
            assert_eq!(proof.total_finding_count(), 1);
            assert_eq!(proof.generation_id(), binding.generation_id()?);
            assert_eq!(
                proof.artifact_sha256(ArtifactRole::FullCheck),
                sha256_bytes(&original[4])
            );
            // Re-sign descriptor hashes without changing the authenticated
            // input binding. Presence/hash checks alone would accept these.
            for (role, pointer, wrong) in [
                (
                    ArtifactRole::FindingIndex,
                    "/value/entries/0/severity",
                    serde_json::json!("error"),
                ),
                (
                    ArtifactRole::FindingIndex,
                    "/value/total_finding_count",
                    serde_json::json!(0),
                ),
                (
                    ArtifactRole::ReviewInput,
                    "/value/findings/0/line",
                    serde_json::json!(2),
                ),
                (
                    ArtifactRole::ReviewInput,
                    "/value/head_sha",
                    serde_json::json!("9".repeat(40)),
                ),
                (
                    ArtifactRole::ReviewInput,
                    "/value/analysis_outcome/analysis_complete",
                    serde_json::json!(false),
                ),
                (
                    ArtifactRole::PrJson,
                    "/value/summary/severe_gaps",
                    serde_json::json!(0),
                ),
                (
                    ArtifactRole::PrJson,
                    "/value/status",
                    serde_json::json!("error"),
                ),
                (
                    ArtifactRole::FullCheck,
                    "/value/check/summary/weakly_exposed",
                    serde_json::json!(0),
                ),
                (
                    ArtifactRole::FullCheck,
                    "/value/check/analysis_outcome/outcome/counts/changed_line_count",
                    serde_json::json!(0),
                ),
                (
                    ArtifactRole::FullCheck,
                    "/value/effective_options/include_unchanged_tests",
                    serde_json::json!(false),
                ),
            ] {
                let mut changed = original.clone();
                let mut wire: Value = strict_json(&changed[role.ordinal()])?;
                *wire
                    .pointer_mut(pointer)
                    .ok_or("fixture mutation target missing")? = wrong;
                changed[role.ordinal()] = serde_json::to_vec(&wire).map_err(|e| e.to_string())?;
                save_fixture(&stage.0, &binding, &changed)?;
                let _error = require_error(
                    verify_staged_generation(&stage.0, &binding, &binding.profile),
                    pointer,
                )?;
            }
            for key in [
                "analysis_scope",
                "run_limitations",
                "suppression_policy",
                "language_runs",
            ] {
                let mut changed = original.clone();
                let mut wire: Value = strict_json(&changed[4])?;
                wire["value"]["check"][key] = serde_json::json!([{"run_status":"failed"}]);
                changed[4] = serde_json::to_vec(&wire).map_err(|e| e.to_string())?;
                save_fixture(&stage.0, &binding, &changed)?;
                let _error = require_error(
                    verify_staged_generation(&stage.0, &binding, &binding.profile),
                    key,
                )?;
            }
            let mut changed = original.clone();
            changed[1][0] = b'D';
            save_fixture(&stage.0, &binding, &changed)?;
            let _error = require_error(
                verify_staged_generation(&stage.0, &binding, &binding.profile),
                "unexpected success in all_nine_nonempty_saved_relations_and_resigned_tampering",
            )?;
            let mut changed = original.clone();
            changed[8] = b"marker removed".to_vec();
            save_fixture(&stage.0, &binding, &changed)?;
            let _error = require_error(
                verify_staged_generation(&stage.0, &binding, &binding.profile),
                "unexpected success in all_nine_nonempty_saved_relations_and_resigned_tampering",
            )?;
            save_fixture(&stage.0, &binding, &original)?;
            verify_staged_generation(&stage.0, &binding, &binding.profile)
                .map_err(|e| e.to_string())?;
            Ok(())
        })
    }

    #[cfg(target_os = "linux")]
    struct Fixture(PathBuf);
    #[cfg(target_os = "linux")]
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[cfg(target_os = "linux")]
    fn stage() -> Result<Fixture, String> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "ripr-saved-complete-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).map_err(|e| e.to_string())?;
        for name in std::iter::once(MANIFEST_FILE)
            .chain(ArtifactRole::ALL.into_iter().map(ArtifactRole::path))
        {
            std::fs::write(root.join(name), b"original").map_err(|e| e.to_string())?;
        }
        Ok(Fixture(root))
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn closed_stage_rejects_missing_extra_symlink_hardlink_and_replacement() -> Result<(), String> {
        use std::os::unix::fs::symlink;
        let fixture = stage()?;
        let directory = StagedDirectory::open(&fixture.0)?;
        directory.verify_closed()?;
        let mut file = directory.open_file("check.json", 8)?;
        let (_, digest) = file.read(Some(8), None)?;
        std::fs::write(fixture.0.join("extra"), b"x").map_err(|e| e.to_string())?;
        let _error = require_error(
            directory.verify_closed(),
            "unexpected success in closed_stage_rejects_missing_extra_symlink_hardlink_and_replacement",
        )?;
        std::fs::remove_file(fixture.0.join("extra")).map_err(|e| e.to_string())?;
        std::fs::remove_file(fixture.0.join("check.json")).map_err(|e| e.to_string())?;
        let _error = require_error(
            directory.verify_closed(),
            "unexpected success in closed_stage_rejects_missing_extra_symlink_hardlink_and_replacement",
        )?;
        symlink("pr.diff", fixture.0.join("check.json")).map_err(|e| e.to_string())?;
        let _error = require_error(
            directory.open_file("check.json", 100),
            "unexpected success in closed_stage_rejects_missing_extra_symlink_hardlink_and_replacement",
        )?;
        std::fs::remove_file(fixture.0.join("check.json")).map_err(|e| e.to_string())?;
        std::fs::hard_link(fixture.0.join("pr.diff"), fixture.0.join("check.json"))
            .map_err(|e| e.to_string())?;
        let _error = require_error(
            directory.open_file("check.json", 100),
            "unexpected success in closed_stage_rejects_missing_extra_symlink_hardlink_and_replacement",
        )?;
        std::fs::remove_file(fixture.0.join("check.json")).map_err(|e| e.to_string())?;
        std::fs::write(fixture.0.join("check.json"), b"original").map_err(|e| e.to_string())?;
        let _error = require_error(
            directory.verify_same_file("check.json", &file.identity, &digest),
            "unexpected success in closed_stage_rejects_missing_extra_symlink_hardlink_and_replacement",
        )?;
        Ok(())
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn stream_hash_caps_growth_same_length_mutation_and_manifest_removal_refuse()
    -> Result<(), String> {
        let fixture = stage()?;
        let directory = StagedDirectory::open(&fixture.0)?;
        let _error = require_error(
            directory.open_file("check.json", 7),
            "unexpected success in stream_hash_caps_growth_same_length_mutation_and_manifest_removal_refuse",
        )?;
        let mut file = directory.open_file("check.json", 8)?;
        let (_, digest) = file.read(Some(8), None)?;
        std::fs::write(fixture.0.join("check.json"), b"mutated!").map_err(|e| e.to_string())?;
        let _error = require_error(
            file.read(None, Some(&digest)),
            "unexpected success in stream_hash_caps_growth_same_length_mutation_and_manifest_removal_refuse",
        )?;
        std::fs::write(fixture.0.join("check.json"), b"original-longer")
            .map_err(|e| e.to_string())?;
        let _error = require_error(
            file.read(None, Some(&digest)),
            "unexpected success in stream_hash_caps_growth_same_length_mutation_and_manifest_removal_refuse",
        )?;
        std::fs::remove_file(fixture.0.join(MANIFEST_FILE)).map_err(|e| e.to_string())?;
        let _error = require_error(
            directory.verify_closed(),
            "unexpected success in stream_hash_caps_growth_same_length_mutation_and_manifest_removal_refuse",
        )?;
        Ok(())
    }
    #[cfg(target_os = "linux")]
    fn resign_policy_fixture(
        binding: &CompleteBinding,
        original: &[Vec<u8>; 9],
        old_generation: &str,
    ) -> Result<[Vec<u8>; 9], String> {
        let generation = binding.generation_id()?;
        let mut changed = original.clone();
        for role in [
            ArtifactRole::FullCheck,
            ArtifactRole::FindingIndex,
            ArtifactRole::ReviewInput,
            ArtifactRole::PrJson,
        ] {
            let mut value: Value = payload(&original[role.ordinal()], role, old_generation)?;
            if role == ArtifactRole::ReviewInput {
                value["check_sha256"] = Value::String(sha256_bytes(&changed[4]));
            }
            changed[role.ordinal()] = encode_payload(role, &generation, &value, &binding.profile)?;
        }
        let markdown = std::str::from_utf8(&original[8]).map_err(|e| e.to_string())?;
        let old_prefix = markdown_prefix(old_generation);
        let body = markdown
            .strip_prefix(&old_prefix)
            .ok_or("fixture Markdown generation prefix missing")?;
        changed[8] = format!("{}{body}", markdown_prefix(&generation)).into_bytes();
        Ok(changed)
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn resigned_committed_request_drift_refuses_before_stage_and_recovers() -> Result<(), String> {
        let source = frozen_source_fixture()?;
        let stage = stage()?;
        frozen::with_context(Some(source.authority.clone()), || -> Result<(), String> {
            let (binding, original) = saved_fixture(&source)?;
            let old_generation = binding.generation_id()?;
            save_fixture(&stage.0, &binding, &original)?;
            verify_staged_generation(&stage.0, &binding, &binding.profile)
                .map_err(|error| error.to_string())?;
            for change in 0..3 {
                let mut wrong = binding.clone();
                let mut original_bytes = wrong.committed_request.original_bytes.clone();
                let blob_oid = if change == 0 {
                    "2".repeat(wrong.committed_request.blob_oid.len())
                } else {
                    wrong.committed_request.blob_oid.clone()
                };
                match change {
                    1 => original_bytes.push(b'\n'),
                    2 => original_bytes.insert(0, b' '),
                    _ => {}
                }
                wrong.committed_request =
                    CommittedRequestBinding::from_original_blob(&blob_oid, original_bytes)?;
                let file = wrong
                    .inventory
                    .files
                    .iter_mut()
                    .find(|file| file.path == POLICY_PATH)
                    .ok_or("fixture request inventory entry is missing")?;
                file.blob_oid.clone_from(&wrong.committed_request.blob_oid);
                file.bytes = wrong.committed_request.original_bytes.len() as u64;
                file.sha256.clone_from(&wrong.committed_request.sha256);
                wrong.inventory.logical_bytes =
                    wrong.inventory.files.iter().map(|file| file.bytes).sum();
                wrong.validate()?;
                assert_ne!(wrong.generation_id()?, old_generation);
                let resigned = resign_policy_fixture(&wrong, &original, &old_generation)?;
                save_fixture(&stage.0, &wrong, &resigned)?;
                let absent = stage.0.join("absent-generation");
                for root in [&stage.0, &absent] {
                    let error = require_error(
                        verify_staged_generation(root, &wrong, &wrong.profile),
                        "re-signed committed request drift was accepted",
                    )?;
                    assert_eq!(
                        error.to_string(),
                        "frozen committed request mode/blob/size/digest differs from binding"
                    );
                }
            }
            save_fixture(&stage.0, &binding, &original)?;
            let recovered = verify_staged_generation(&stage.0, &binding, &binding.profile)
                .map_err(|error| error.to_string())?;
            assert_eq!(recovered.generation_id(), old_generation);
            assert_eq!(recovered.total_finding_count(), 1);
            Ok(())
        })
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn frozen_committed_request_removal_refuses_before_absent_stage() -> Result<(), String> {
        let source = frozen_source_fixture()?;
        let (binding, original) =
            frozen::with_context(Some(source.authority.clone()), || saved_fixture(&source))?;
        let missing = frozen::tests::Fixture::new(&[("a.rs", b"value > 1\n")])
            .map_err(|error| error.to_string())?;
        let stage = stage()?;
        let mut wrong = binding.clone();
        let root = missing.logical.to_str().ok_or("fixture root is not UTF8")?;
        wrong.subject.logical_root = root.into();
        wrong.subject.work_tree = root.into();
        wrong.subject.invocation_repository = root.into();
        wrong.subject.head_tree = missing.authority.head_tree().as_str().into();
        wrong.validate()?;
        let old_generation = binding.generation_id()?;
        let resigned = resign_policy_fixture(&wrong, &original, &old_generation)?;
        save_fixture(&stage.0, &wrong, &resigned)?;
        frozen::with_context(Some(missing.authority.clone()), || -> Result<(), String> {
            let absent = stage.0.join("absent-generation");
            for root in [&stage.0, &absent] {
                let error = require_error(
                    verify_staged_generation(root, &wrong, &wrong.profile),
                    "missing frozen committed request was accepted",
                )?;
                assert_eq!(
                    error.to_string(),
                    "committed request is missing from frozen inventory"
                );
            }
            Ok(())
        })?;
        save_fixture(&stage.0, &binding, &original)?;
        frozen::with_context(Some(source.authority.clone()), || -> Result<(), String> {
            let recovered = verify_staged_generation(&stage.0, &binding, &binding.profile)
                .map_err(|error| error.to_string())?;
            assert_eq!(recovered.generation_id(), old_generation);
            assert_eq!(recovered.total_finding_count(), 1);
            Ok(())
        })
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn original_frozen_request_bytes_are_read_before_absent_stage() -> Result<(), String> {
        let source = frozen_source_fixture()?;
        let (binding, _) =
            frozen::with_context(Some(source.authority.clone()), || saved_fixture(&source))?;
        let mut changed = binding.committed_request.original_bytes.clone();
        let byte = changed.first_mut().ok_or("fixture policy is empty")?;
        *byte = b'[';
        std::fs::write(source.physical.join(POLICY_PATH), &changed)
            .map_err(|error| error.to_string())?;
        frozen::with_context(Some(source.authority.clone()), || -> Result<(), String> {
            let error = require_error(
                verify_staged_generation(
                    &source.logical.join("absent-generation"),
                    &binding,
                    &binding.profile,
                ),
                "changed original frozen request bytes were accepted",
            )?;
            let error = error.to_string();
            assert!(error.starts_with("read original frozen committed request: "));
            assert!(error.contains("source bytes differ from admitted blob"));
            Ok(())
        })?;
        // The frozen fault is sticky. Recovery requires a fresh owned source,
        // rather than resetting or laundering the mutated authority.
        let recovered_source = frozen_source_fixture()?;
        let stage = stage()?;
        frozen::with_context(
            Some(recovered_source.authority.clone()),
            || -> Result<(), String> {
                let (binding, original) = saved_fixture(&recovered_source)?;
                save_fixture(&stage.0, &binding, &original)?;
                let recovered = verify_staged_generation(&stage.0, &binding, &binding.profile)
                    .map_err(|error| error.to_string())?;
                assert_eq!(recovered.generation_id(), binding.generation_id()?);
                assert_eq!(recovered.total_finding_count(), 1);
                Ok(())
            },
        )
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn resigned_policy_drift_refuses_before_reads_and_baseline_recovers() -> Result<(), String> {
        let source = frozen_source_fixture()?;
        let stage = stage()?;
        frozen::with_context(Some(source.authority.clone()), || -> Result<(), String> {
            let (binding, original) = saved_fixture(&source)?;
            let old_generation = binding.generation_id()?;
            save_fixture(&stage.0, &binding, &original)?;
            verify_staged_generation(&stage.0, &binding, &binding.profile)
                .map_err(|e| e.to_string())?;
            for change in 0..7 {
                let mut wrong = binding.clone();
                let policy = &mut wrong.rust_execution_policy;
                let alternate = |number: u64| match number.checked_add(1) {
                    Some(next) => next,
                    None => number - 1,
                };
                match change {
                    0 => {
                        policy.changed_rust_line_limit = alternate(policy.changed_rust_line_limit);
                        policy.partial_diff_line_budget = policy
                            .partial_diff_line_budget
                            .min(policy.changed_rust_line_limit);
                    }
                    1 => {
                        policy.diff_index_file_limit = alternate(policy.diff_index_file_limit);
                        policy.diff_narrow_index_limit = policy
                            .diff_narrow_index_limit
                            .min(policy.diff_index_file_limit);
                        policy.partial_diff_file_budget = policy
                            .partial_diff_file_budget
                            .min(policy.diff_index_file_limit);
                    }
                    2 => {
                        if policy.diff_narrow_index_limit < policy.diff_index_file_limit {
                            policy.diff_narrow_index_limit += 1;
                        } else if policy.diff_narrow_index_limit > 1 {
                            policy.diff_narrow_index_limit -= 1;
                        } else {
                            policy.diff_index_file_limit += 1;
                            policy.diff_narrow_index_limit += 1;
                        }
                    }
                    3 => {
                        if policy.partial_diff_file_budget < policy.diff_index_file_limit {
                            policy.partial_diff_file_budget += 1;
                        } else if policy.partial_diff_file_budget > 1 {
                            policy.partial_diff_file_budget -= 1;
                        } else {
                            policy.diff_index_file_limit += 1;
                            policy.partial_diff_file_budget += 1;
                        }
                    }
                    4 => {
                        if policy.partial_diff_line_budget < policy.changed_rust_line_limit {
                            policy.partial_diff_line_budget += 1;
                        } else if policy.partial_diff_line_budget > 1 {
                            policy.partial_diff_line_budget -= 1;
                        } else {
                            policy.changed_rust_line_limit += 1;
                            policy.partial_diff_line_budget += 1;
                        }
                    }
                    5 => {
                        policy.dependent_scope = match &policy.dependent_scope {
                            RustDependentScopePolicy::Auto => RustDependentScopePolicy::Full,
                            _ => RustDependentScopePolicy::Auto,
                        };
                    }
                    _ => {
                        if policy.partial_budget_disclosures.is_empty() {
                            policy
                                .partial_budget_disclosures
                                .push("stale disclosure".into());
                        } else {
                            policy.partial_budget_disclosures.clear();
                        }
                    }
                }
                wrong.validate()?;
                let resigned = resign_policy_fixture(&wrong, &original, &old_generation)?;
                save_fixture(&stage.0, &wrong, &resigned)?;
                let error = require_error(
                    verify_staged_generation(&stage.0, &wrong, &wrong.profile),
                    "re-signed policy drift was accepted",
                )?;
                assert_eq!(
                    error.to_string(),
                    "fresh Rust execution policy differs from complete binding"
                );
            }
            // A missing stage cannot replace the specific early policy error.
            let mut wrong = binding.clone();
            wrong.rust_execution_policy.changed_rust_line_limit = match wrong
                .rust_execution_policy
                .changed_rust_line_limit
                .checked_add(1)
            {
                Some(next) => next,
                None => wrong.rust_execution_policy.changed_rust_line_limit - 1,
            };
            wrong.rust_execution_policy.partial_diff_line_budget = wrong
                .rust_execution_policy
                .partial_diff_line_budget
                .min(wrong.rust_execution_policy.changed_rust_line_limit);
            let missing = stage.0.join("absent-generation");
            let error = require_error(
                verify_staged_generation(&missing, &wrong, &wrong.profile),
                "policy drift with absent stage was accepted",
            )?;
            assert_eq!(
                error.to_string(),
                "fresh Rust execution policy differs from complete binding"
            );
            save_fixture(&stage.0, &binding, &original)?;
            let recovered = verify_staged_generation(&stage.0, &binding, &binding.profile)
                .map_err(|e| e.to_string())?;
            assert_eq!(recovered.generation_id(), old_generation);
            assert_eq!(recovered.total_finding_count(), 1);
            Ok(())
        })
    }

    #[test]
    fn wrong_declared_input_base_refuses_before_stage_or_frozen_authority() -> Result<(), String> {
        let mut binding = fixture_binding()?;
        binding.effective_options.surface = ProducerSurface::Xtask;
        binding.effective_options.include_unchanged_tests = false;
        binding.effective_options.check_input_base = Some("wrong-original-base".into());
        binding.effective_options.git_timeout_ms = Some(300_000);
        let error = require_error(
            verify_staged_generation(
                Path::new("/absent-complete-stage"),
                &binding,
                &binding.profile,
            ),
            "wrong declared input base reached stage verification",
        )?;
        assert!(
            error
                .to_string()
                .contains("xtask declared input base differs"),
            "wrong precedence: {error}"
        );
        Ok(())
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn xtask_input_base_is_bound_while_saved_effective_base_stays_absent() -> Result<(), String> {
        let source = frozen_source_fixture()?;
        let stage = stage()?;
        frozen::with_context(Some(source.authority.clone()), || -> Result<(), String> {
            let (binding, original) = saved_fixture_for_surface(&source, ProducerSurface::Xtask)?;
            let wire: CompleteCheck = payload(
                &original[ArtifactRole::FullCheck.ordinal()],
                ArtifactRole::FullCheck,
                &binding.generation_id()?,
            )?;
            assert_eq!(
                wire.effective_options.check_input_base.as_deref(),
                Some("base")
            );
            assert!(wire.check["base"].is_null());
            let outcome: AnalysisOutcome =
                serde_json::from_value(wire.check["analysis_outcome"]["outcome"].clone())
                    .map_err(|error| error.to_string())?;
            assert_eq!(
                outcome.identity.base_revision, None,
                "declared input must not replace effective supplied-diff provenance"
            );
            save_fixture(&stage.0, &binding, &original)?;
            let proof = verify_staged_generation(&stage.0, &binding, &binding.profile)
                .map_err(|error| error.to_string())?;
            assert_eq!(proof.total_finding_count(), 1);
            for (pointer, wrong) in [
                (
                    "/value/effective_options/check_input_base",
                    serde_json::json!("alias"),
                ),
                ("/value/check/base", serde_json::json!("base")),
                (
                    "/value/check/analysis_outcome/outcome/identity/base_revision",
                    serde_json::json!("base"),
                ),
            ] {
                let mut changed = original.clone();
                let mut value: Value = strict_json(&changed[ArtifactRole::FullCheck.ordinal()])?;
                *value
                    .pointer_mut(pointer)
                    .ok_or("fixture base mutation target missing")? = wrong;
                changed[ArtifactRole::FullCheck.ordinal()] =
                    serde_json::to_vec(&value).map_err(|error| error.to_string())?;
                // Re-sign every artifact descriptor; relational refusal must survive.
                save_fixture(&stage.0, &binding, &changed)?;
                let _error = require_error(
                    verify_staged_generation(&stage.0, &binding, &binding.profile),
                    "re-signed input/effective base contradiction was accepted",
                )?;
            }
            let mut missing = original.clone();
            let mut value: Value = strict_json(&missing[ArtifactRole::FullCheck.ordinal()])?;
            value
                .pointer_mut("/value/effective_options")
                .and_then(Value::as_object_mut)
                .ok_or("fixture effective options missing")?
                .remove("check_input_base");
            missing[ArtifactRole::FullCheck.ordinal()] =
                serde_json::to_vec(&value).map_err(|error| error.to_string())?;
            save_fixture(&stage.0, &binding, &missing)?;
            let _error = require_error(
                verify_staged_generation(&stage.0, &binding, &binding.profile),
                "re-signed missing declared input field was accepted",
            )?;
            save_fixture(&stage.0, &binding, &original)?;
            verify_staged_generation(&stage.0, &binding, &binding.profile)
                .map_err(|error| error.to_string())?;
            source
                .authority
                .ensure_clean()
                .map_err(|error| error.to_string())
        })
    }

    #[test]
    fn saved_verifier_allowance_preserves_role_formula_exact_bound_and_overflow()
    -> Result<(), String> {
        let mut limits = fixture_binding()?.profile;
        limits.max_relation_bytes = 0;
        let files = [1_u64, 2, 3, 4, 5, 6, 7, 8, 9];
        // EXACT existing manifest/json/raw/reducer formula; no caller allowance.
        let expected = 2 * 64 + 45 + (5 + 6 + 7 + 8) * 64 + 4;
        assert_eq!(artifact_buffer_allowance(2, &files, &limits)?, expected);
        limits.max_buffered_bytes = expected;
        let mut manifest = fixture_manifest(fixture_binding()?)?;
        for (entry, bytes) in manifest.artifacts.iter_mut().zip(files) {
            entry.bytes = bytes;
        }
        admit_buffers(&manifest, 2, &limits)?;
        limits.max_buffered_bytes -= 1;
        let error = require_error(
            admit_buffers(&manifest, 2, &limits),
            "saved verifier ignored exact combined allowance",
        )?;
        assert_eq!(error, "artifact/JSON/raw/reducer buffer admission exceeded");
        let error = require_error(
            artifact_buffer_allowance(u64::MAX, &files, &limits),
            "saved verifier ignored manifest multiplication overflow",
        )?;
        assert_eq!(error, "JSON representation allowance overflow");
        let mut overflow = [0_u64; 9];
        overflow[ArtifactRole::CheckDiff.ordinal()] = u64::MAX;
        overflow[ArtifactRole::PresentationDiff.ordinal()] = 1;
        let error = require_error(
            artifact_buffer_allowance(0, &overflow, &limits),
            "saved verifier ignored aggregate overflow",
        )?;
        assert_eq!(error, "retained buffer total overflow");
        Ok(())
    }


    #[cfg(target_os = "linux")]
    #[test]
    fn verified_descriptor_data_observes_actual_files_and_equal_byte_replacement()
    -> Result<(), String> {
        use std::os::unix::fs::MetadataExt;
        let source = frozen_source_fixture()?;
        let stage = stage()?;
        frozen::with_context(Some(source.authority.clone()), || -> Result<(), String> {
            let (binding, original) = saved_fixture(&source)?;
            save_fixture(&stage.0, &binding, &original)?;
            let observed = verify_staged_generation(&stage.0, &binding, &binding.profile)
                .map_err(|error| error.to_string())?;
            for role in ArtifactRole::ALL {
                let metadata = std::fs::symlink_metadata(stage.0.join(role.path()))
                    .map_err(|error| format!("actual proof identity fixture: {error}"))?;
                assert_eq!(observed.artifact_identity(role),
                    (metadata.dev(), metadata.ino(), metadata.len()));
                assert_eq!(observed.artifact_sha256(role),
                    sha256_bytes(&original[role.ordinal()]));
            }
            let metadata = std::fs::symlink_metadata(stage.0.join(MANIFEST_FILE))
                .map_err(|error| format!("actual manifest identity fixture: {error}"))?;
            assert_eq!(observed.manifest_identity(),
                (metadata.dev(), metadata.ino(), metadata.len()));
            // Equal bytes can form a fresh saved proof, but its observed file
            // identity cannot reconcile an earlier IO closure's actual inode.
            let role = ArtifactRole::FullCheck;
            let replacement = stage.0.join("replacement");
            std::fs::write(&replacement, &original[role.ordinal()])
                .map_err(|error| format!("actual replacement fixture: {error}"))?;
            std::fs::rename(replacement, stage.0.join(role.path()))
                .map_err(|error| format!("actual replacement publish fixture: {error}"))?;
            let fresh = verify_staged_generation(&stage.0, &binding, &binding.profile)
                .map_err(|error| error.to_string())?;
            assert_eq!(fresh.artifact_sha256(role), observed.artifact_sha256(role));
            assert_ne!(fresh.artifact_identity(role), observed.artifact_identity(role));
            Ok(())
        })
    }

}
