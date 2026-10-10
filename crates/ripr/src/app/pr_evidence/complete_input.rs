//! Inactive single-checker whole-input factory and in-run validators.
//!
//! This leaf is compiled only for inactive Linux Rust tests. Production
//! dispatch, qualified raw capture and artifact/custody integration remain
//! root-owned dependencies. Neither startup DATA, native limits, a frozen Arc
//! nor a saved generation can construct this token.
//! Root owns worker/cohort custody, bounded live-config reobservation,
//! artifact qualification and publication. Retained config equality alone
//! cannot prove that the caller's live config stayed unchanged.

use super::complete_contract::{
    AnalyzerBinding, ArtifactRole, BINDING_SCHEMA, CompleteBinding, CompleteMode,
    CompleteVerificationLimits, ConfigurationBinding, EffectiveOptions, FullConfiguration,
    INVENTORY_REPRESENTATION, InventoryBinding, InventoryFile, PolicyField,
    PresentationBinding, ProducerSurface, RawBinding, RustDependentScopePolicy,
    RustExecutionPolicy, SubjectBinding,
};
use super::complete_execution::{CaptureBudget, QualifiedWholeInvocation};
use super::complete_verifier::{
    VerifiedGeneration, artifact_buffer_allowance, verify_staged_generation,
};
use super::complete_native::NativeStartup;
use super::complete_request::{CompleteRequest, CompleteSubject, POLICY_PATH};
use super::raw_coverage::{
    RawCoverage, RawCoverageLimits, build_raw_coverage, semantic_projection_digest,
};
use super::PrEvidenceOptions;
use crate::analysis::committed_source::frozen::{self, FrozenFileMode, FrozenSourceAuthority};
use crate::analysis::diff::parse::ParsedDiff;
use crate::analysis::diff::ChangedFile;
use crate::analysis::git_candidate_execution::staged::{
    CompleteTreeBudget, prepare_staged_named_tree,
};
use crate::analysis::{
    AnalysisOptions, CompleteRustPolicySnapshot, capture_complete_rust_policy,
};
use crate::app::{CheckInput, CheckOutput, Mode, OutputFormat};
use crate::config::{CheckInputExplicit, OraclePolicy, RiprConfig, RustLanguageConfig};
use crate::domain::LanguageId;
use sha2::Digest;
use crate::analysis::committed_source::staged::{
    ArtifactBudget, ArtifactClosureData, ArtifactDirectory, ArtifactFileData, ArtifactPayloadData, ArtifactSlot,
    RetainedDirectory,
};
use std::cell::RefCell;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

const RAW_MAX: u64 = 256 * 1024 * 1024;
const SOURCE_MAX: u64 = 512 * 1024 * 1024;
const POLICY_MAX: u64 = 4096;

#[derive(Clone, Copy, Eq, PartialEq)]
enum Phase {
    Fresh,
    Running,
    Closed,
}

/// Only prepare constructs this, after the sealed invocation's actual checks.
/// There is no Clone/Default/Serde implementation or public constructor.
pub(crate) struct VerifiedWholeInput {
    phase: Phase,
    deadline: Instant,
    authority: Arc<FrozenSourceAuthority>,
    canonical: Arc<str>,
    config: Arc<RiprConfig>,
    check: CheckInput,
    options: AnalysisOptions,
    languages: Vec<LanguageId>,
    policy: CompleteRustPolicySnapshot,
    projection: String,
    projection_limit: usize,
    // ALL language files, after the pipeline's pure-rename filter and before
    // Rust's generated-source filter. This is not a full workspace index.
    rust_subject: Vec<ChangedFile>,
}

/// Consuming execution keeps actual native and invocation owners alive.
pub(super) struct FreshWholeInput {
    whole: VerifiedWholeInput,
    startup: NativeStartup,
    invocation: QualifiedWholeInvocation,
    request: CompleteRequest,
    subject: CompleteSubject,
    caller_root: PathBuf,
    surface: ProducerSurface,
    raw: Vec<u8>,
    coverage: RawCoverage,
    presentation: String,
    binding: CompleteBinding,
    generation_id: String,
    build_identity: String,
    profile: CompleteVerificationLimits,
    retained_input_bytes: u64,
}

pub(super) struct AnalyzedWholeInput {
    input: FreshWholeInput,
    output: CheckOutput,
}

pub(super) struct StagedWholeInput<T> {
    analyzed: AnalyzedWholeInput,
    staged: T,
}

/// Observed source/check DATA after postflight, still no publication grant.
/// Root must independently settle its cohort and qualify the closed artifacts.
pub(super) struct WholeInputEvidence<T> {
    staged: T,
}

/// Borrowed DATA for root's fallible output rebase/reducers/artifact preparation.
/// This never lends the token or a mutable native/invocation/source owner.
pub(super) struct WholeInputData<'a> {
    input: &'a FreshWholeInput,
    artifact_attempt: ArtifactAttempt,
}


/// Live one-shot emitter bookkeeping, never serialized or an execution grant.
/// No mutable latch borrow survives external manifest preparation.
enum ArtifactAttemptState {
    Fresh,
    Claimed,
    Closed,
    Verifying,
    Verified,
    Failed(String),
}

#[derive(Clone, Copy)]
struct ObservedArtifacts {
    files: [ArtifactFileData; 9],
    manifest: ArtifactFileData,
}

struct ArtifactAttempt {
    state: RefCell<ArtifactAttemptState>,
    observed: RefCell<Option<ObservedArtifacts>>,
}

impl ArtifactAttempt {
    fn new() -> Self {
        Self {
            state: RefCell::new(ArtifactAttemptState::Fresh),
            observed: RefCell::new(None),
        }
    }

    fn claim(&self) -> Result<(), String> {
        let mut state = self.state.try_borrow_mut()
            .map_err(|error| format!("whole-input artifact claim is borrowed: {error}"))?;
        match &*state {
            ArtifactAttemptState::Fresh => {
                *state = ArtifactAttemptState::Claimed;
                Ok(())
            }
            ArtifactAttemptState::Failed(error) => Err(error.clone()),
            _ => {
                let error = "whole-input artifact attempt repeated or reentrant".to_string();
                *state = ArtifactAttemptState::Failed(error.clone());
                Err(error)
            }
        }
    }

    fn ensure_claimed(&self) -> Result<(), String> {
        let state = self.state.try_borrow()
            .map_err(|error| format!("whole-input artifact phase is borrowed: {error}"))?;
        match &*state {
            ArtifactAttemptState::Claimed => Ok(()),
            ArtifactAttemptState::Failed(error) => Err(error.clone()),
            _ => Err("whole-input artifact phase is not claimed".into()),
        }
    }

    fn remember<T>(&self, result: Result<T, String>) -> Result<T, String> {
        match result {
            Ok(value) => Ok(value),
            Err(error) => {
                let mut state = self.state.try_borrow_mut().map_err(|borrow| {
                    format!("{error}; whole-input artifact fault is borrowed: {borrow}")
                })?;
                if let ArtifactAttemptState::Failed(first) = &*state {
                    return if &error == first {
                        Err(first.clone())
                    } else {
                        Err(format!("{error}; whole-input artifact first fault: {first}"))
                    };
                }
                *state = ArtifactAttemptState::Failed(error.clone());
                Err(error)
            }
        }
    }

    fn close_after_io(&self, observed: &ArtifactClosureData) -> Result<(), String> {
        self.ensure_claimed()?;
        if observed.manifest().name() != super::complete_contract::MANIFEST_FILE
            || observed.payloads().files().iter().zip(ArtifactRole::ALL)
                .any(|(file, role)| file.name() != role.path())
        {
            return Err("whole-input artifact closure names differ from actual contract".into());
        }
        let mut state = self.state.try_borrow_mut()
            .map_err(|error| format!("whole-input artifact closeout is borrowed: {error}"))?;
        match &*state {
            ArtifactAttemptState::Claimed => {
                let mut slot = self.observed.try_borrow_mut()
                    .map_err(|error| format!("whole-input artifact observations are borrowed: {error}"))?;
                if slot.is_some() {
                    return Err("whole-input artifact observations already exist".into());
                }
                *slot = Some(ObservedArtifacts {
                    files: *observed.payloads().files(), manifest: *observed.manifest(),
                });
                *state = ArtifactAttemptState::Closed;
                Ok(())
            }
            ArtifactAttemptState::Failed(error) => Err(error.clone()),
            _ => Err("whole-input artifact closeout is not claimed".into()),
        }
    }

    fn claim_verification(&self) -> Result<ObservedArtifacts, String> {
        // No mutable borrow survives into the verifier or any caller callback.
        {
            let mut state = self.state.try_borrow_mut()
                .map_err(|error| format!("whole-input verification claim is borrowed: {error}"))?;
            match &*state {
                ArtifactAttemptState::Closed => *state = ArtifactAttemptState::Verifying,
                ArtifactAttemptState::Failed(error) => return Err(error.clone()),
                _ => {
                    let error = "whole-input artifact verification repeated or precedes closure".to_string();
                    *state = ArtifactAttemptState::Failed(error.clone());
                    return Err(error);
                }
            }
        }
        let observed = self.observed.try_borrow()
            .map_err(|error| format!("whole-input artifact observations are borrowed: {error}"))?
            .as_ref().copied().ok_or("whole-input actual artifact observations are missing");
        self.remember(observed.map_err(str::to_owned))
    }

    fn ensure_verifying(&self) -> Result<(), String> {
        let state = self.state.try_borrow()
            .map_err(|error| format!("whole-input verifying phase is borrowed: {error}"))?;
        match &*state {
            ArtifactAttemptState::Verifying => Ok(()),
            ArtifactAttemptState::Failed(error) => Err(error.clone()),
            _ => Err("whole-input artifact phase is not verifying".into()),
        }
    }

    fn verified_after_success(&self, _: &VerifiedGeneration) -> Result<(), String> {
        self.ensure_verifying()?;
        let mut state = self.state.try_borrow_mut()
            .map_err(|error| format!("whole-input verified phase is borrowed: {error}"))?;
        match &*state {
            ArtifactAttemptState::Verifying => {
                *state = ArtifactAttemptState::Verified;
                Ok(())
            }
            ArtifactAttemptState::Failed(error) => Err(error.clone()),
            _ => Err("whole-input artifact phase changed before actual verification".into()),
        }
    }

    // IO-only helper for its actual closure controls; not the stage admission.
    fn finish_work<T>(&self, work: Result<T, String>) -> Result<T, String> {
        self.finish_phase(work, false)
    }

    fn finish_verified_work<T>(&self, work: Result<T, String>) -> Result<T, String> {
        self.finish_phase(work, true)
    }

    fn finish_phase<T>(&self, work: Result<T, String>, require_verified: bool) -> Result<T, String> {
        let state = match self.state.try_borrow() {
            Ok(state) => state,
            Err(error) => {
                return match work {
                    Err(primary) => Err(format!(
                        "{primary}; whole-input artifact final phase is borrowed: {error}"
                    )),
                    Ok(_) => Err(format!("whole-input artifact final phase is borrowed: {error}")),
                };
            }
        };
        match (work, &*state) {
            (Err(primary), ArtifactAttemptState::Failed(first)) if &primary != first => {
                Err(format!("{primary}; whole-input artifact first fault: {first}"))
            }
            (Err(primary), _) => Err(primary),
            (Ok(value), ArtifactAttemptState::Verified) => Ok(value),
            (Ok(value), ArtifactAttemptState::Closed) if !require_verified => Ok(value),
            (Ok(_), ArtifactAttemptState::Failed(first)) => Err(first.clone()),
            (Ok(_), _) if require_verified => {
                Err("whole-input artifact attempt lacks actual in-custody verification".into())
            }
            (Ok(_), _) => Err("whole-input artifact attempt did not finish its actual closure".into()),
        }
    }
}

const ARTIFACT_MAPPING: [(ArtifactRole, ArtifactSlot); 9] = [
    (ArtifactRole::OriginalRaw, ArtifactSlot::OriginalRaw),
    (ArtifactRole::CheckDiff, ArtifactSlot::CheckDiff),
    (ArtifactRole::PresentationDiff, ArtifactSlot::PresentationDiff),
    (ArtifactRole::RawLedger, ArtifactSlot::RawLedger),
    (ArtifactRole::FullCheck, ArtifactSlot::FullCheck),
    (ArtifactRole::FindingIndex, ArtifactSlot::FindingIndex),
    (ArtifactRole::ReviewInput, ArtifactSlot::ReviewInput),
    (ArtifactRole::PrJson, ArtifactSlot::PrJson),
    (ArtifactRole::PrMarkdown, ArtifactSlot::PrMarkdown),
];

fn require_artifact_mapping(mapping: &[(ArtifactRole, ArtifactSlot); 9]) -> Result<(), String> {
    for (ordinal, (role, slot)) in mapping.iter().enumerate() {
        if *role != ArtifactRole::ALL[ordinal] || *slot != ArtifactSlot::ALL[ordinal]
            || role.ordinal() != ordinal || slot.ordinal() != ordinal || role.path() != slot.path()
        {
            return Err("whole-input artifact role/path/ordinal mapping differs".into());
        }
    }
    Ok(())
}

fn artifact_budget(profile: &CompleteVerificationLimits) -> Result<ArtifactBudget, String> {
    // SAME profile/consumer/native-usize DATA validation; no cap normalization.
    profile.validate()?;
    require_artifact_mapping(&ARTIFACT_MAPPING)?;
    let mut caps = [0_u64; 9];
    for (role, slot) in ARTIFACT_MAPPING {
        caps[slot.ordinal()] = profile.max_artifact_bytes[role.ordinal()];
    }
    ArtifactBudget::new(
        caps, profile.max_total_artifact_bytes, profile.max_manifest_bytes, profile.file_size_bytes,
    )
}

fn require_artifact_context(
    authority: &Arc<FrozenSourceAuthority>,
    canonical: &Arc<str>,
) -> Result<(), String> {
    let current = frozen::current()
        .ok_or("whole-input artifact preparation lacks its frozen context")?;
    if !Arc::ptr_eq(authority, &current) {
        return Err("whole-input artifact frozen context identity differs".into());
    }
    let current = frozen::canonical_diff()
        .ok_or("whole-input artifact preparation lacks its canonical diff context")?;
    if !Arc::ptr_eq(canonical, &current) {
        return Err("whole-input artifact canonical context identity differs".into());
    }
    authority.ensure_clean().map_err(|error| error.to_string())
}

fn verify_artifact_native_limits(
    profile: &CompleteVerificationLimits,
    deadline: Instant,
) -> Result<(), String> {
    checkpoint(deadline)?;
    super::complete_execution::verify_limits(
        profile.address_space_bytes, profile.file_size_bytes,
    )?;
    checkpoint(deadline)
}

/// Private IO routine after the unique claim. It grants no analyzer, verifier,
/// cleanup or publication authority. The genuine factory supplies checkpoint.
fn emit_artifacts_after_claim(
    attempt: &ArtifactAttempt,
    directory: &RetainedDirectory,
    deadline: Instant,
    budget: ArtifactBudget,
    payloads: [&[u8]; 9],
    mut check: impl FnMut() -> Result<(), String>,
    manifest: impl FnOnce(&ArtifactPayloadData) -> Result<Vec<u8>, String>,
) -> Result<ArtifactClosureData, String> {
    attempt.ensure_claimed()?;
    check()?;
    let mut writer = ArtifactDirectory::new(directory, payloads, budget, deadline)?;
    for slot in ArtifactSlot::ALL {
        attempt.ensure_claimed()?;
        check()?;
        writer.write_slot(slot)?;
    }
    let finished = writer.finish_payloads()?;
    // Root's callback uses the existing bounded serializer and BEFORE-growth
    // logical phase admission. A returned Vec length is not allocation proof.
    let bytes = manifest(finished.receipts())?;
    // A callback may swallow a nested reuse failure; never create the manifest
    // after that sticky fault, even when it returns successful bytes.
    attempt.ensure_claimed()?;
    check()?;
    let observed = finished.write_manifest(&bytes)?;
    check()?;
    attempt.ensure_claimed()?;
    attempt.close_after_io(&observed)?;
    Ok(observed)
}

/// Same saved-verifier allowance plus actual caller-owned retained phases.
/// The typed-output allowance is a protocol payload estimate, not Serialize
/// on CheckOutput or exact allocator/RSS accounting. Root separately admits
/// the unchanged renderer and manifest serializer BEFORE their allocations.
fn combined_verifier_bytes(
    retained_input: u64,
    files: &[u64; 9],
    manifest: u64,
    profile: &CompleteVerificationLimits,
) -> Result<u64, String> {
    let payloads = files.iter().try_fold(0_u64, |sum, bytes| {
        sum.checked_add(*bytes).ok_or("whole-input caller payload byte overflow")
    })?;
    let verifier = artifact_buffer_allowance(manifest, files, profile)?;
    admit_sum(profile.max_buffered_bytes, &[
        retained_input, payloads, manifest,
        multiply(files[ArtifactRole::FullCheck.ordinal()], 64)?, verifier,
    ])
}

fn reconcile_observed_generation(
    observed: &ObservedArtifacts,
    verified: &VerifiedGeneration,
    expected_generation: &str,
) -> Result<(), String> {
    if verified.generation_id() != expected_generation
        || verified.manifest_sha256() != digest_text(observed.manifest.sha256())?
        || verified.manifest_identity() != (
            observed.manifest.identity().0, observed.manifest.identity().1,
            observed.manifest.bytes(),
        )
    {
        return Err("whole-input verified manifest differs from actual IO closure".into());
    }
    for (file, role) in observed.files.iter().zip(ArtifactRole::ALL) {
        if file.name() != role.path()
            || verified.artifact_sha256(role) != digest_text(file.sha256())?
            || verified.artifact_identity(role) != (
                file.identity().0, file.identity().1, file.bytes(),
            )
        {
            return Err("whole-input verified artifact differs from actual IO closure".into());
        }
    }
    Ok(())
}

fn checkpoint(deadline: Instant) -> Result<(), String> {
    crate::analysis::cancellation::checkpoint_typed().map_err(|error| error.to_string())?;
    if Instant::now() >= deadline {
        return Err("whole-input held worker deadline expired".into());
    }
    Ok(())
}

fn native_size(value: u64) -> Result<usize, String> {
    usize::try_from(value).map_err(|error| format!("whole-input native byte/count conversion: {error}"))
}

fn admit_sum(limit: u64, terms: &[u64]) -> Result<u64, String> {
    let total = terms.iter().try_fold(0_u64, |total, term| {
        total.checked_add(*term).ok_or("whole-input retained-byte accounting overflow")
    })?;
    if total > limit {
        return Err("whole-input retained byte phase exceeds admitted buffer bound".into());
    }
    Ok(total)
}

fn admitted_file_limit(profile_limit: u64, actual_index_limit: usize) -> Result<usize, String> {
    let limit = native_size(profile_limit)?;
    if limit > actual_index_limit {
        return Err("whole-input profile file limit exceeds the actual diff index bound".into());
    }
    // The ledger header and actual saved verifier both commit profile_limit.
    // For admitted profiles this is exactly min(profile, fresh index).
    Ok(limit)
}

fn multiply(value: u64, factor: u64) -> Result<u64, String> {
    value.checked_mul(factor).ok_or_else(|| "whole-input retained-byte accounting overflow".into())
}

struct ByteCounter { bytes: u64, limit: u64 }
impl Write for ByteCounter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes = self.bytes.checked_add(bytes.len() as u64)
            .filter(|total| *total <= self.limit)
            .ok_or_else(|| io::Error::other("whole-input configuration representation exceeds admitted bound"))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}
fn configuration_bytes(value: &FullConfiguration, limit: u64) -> Result<u64, String> {
    let mut counter = ByteCounter { bytes: 0, limit };
    serde_json::to_writer(&mut counter, value)
        .map_err(|error| format!("whole-input configuration representation: {error}"))?;
    Ok(counter.bytes)
}

fn digest_text(bytes: &[u8; 32]) -> Result<String, String> {
    use std::fmt::Write as _;
    let mut digest = String::with_capacity(71);
    digest.push_str("sha256:");
    for byte in bytes {
        write!(&mut digest, "{byte:02x}")
            .map_err(|error| format!("whole-input observed digest formatting: {error}"))?;
    }
    Ok(digest)
}

fn binding_bytes(value: &CompleteBinding, limit: u64) -> Result<u64, String> {
    let mut counter = ByteCounter { bytes: 0, limit };
    serde_json::to_writer(&mut counter, value)
        .map_err(|error| format!("whole-input binding representation: {error}"))?;
    Ok(counter.bytes)
}

fn wire_policy(policy: &CompleteRustPolicySnapshot) -> Result<RustExecutionPolicy, String> {
    if capture_complete_rust_policy()? != *policy {
        return Err("whole-input sealed Rust policy changed before binding".into());
    }
    let number = |value: usize| {
        u64::try_from(value).map_err(|error| format!("whole-input policy conversion: {error}"))
    };
    let dependent_scope = match policy.dependent_scope_mode() {
        crate::analysis::CompleteDependentScopePolicy::Auto => RustDependentScopePolicy::Auto,
        crate::analysis::CompleteDependentScopePolicy::NameAdmitted => RustDependentScopePolicy::NameAdmitted,
        crate::analysis::CompleteDependentScopePolicy::Full => RustDependentScopePolicy::Full,
    };
    Ok(RustExecutionPolicy {
        changed_rust_line_limit: number(policy.changed_rust_line_limit())?,
        diff_index_file_limit: number(policy.diff_index_file_limit())?,
        diff_narrow_index_limit: number(policy.diff_narrow_index_limit())?,
        partial_diff_file_budget: number(policy.partial_diff_file_budget())?,
        partial_diff_line_budget: number(policy.partial_diff_line_budget())?,
        partial_budget_disclosures: policy.partial_budget_disclosures().to_vec(),
        partial_selection_version: policy.partial_selection_version().into(),
        partial_language_tier_version: policy.partial_language_tier_version().into(),
        dependent_scope,
    })
}

/// Construct only worker-local expected DATA from the authentic held inputs.
/// Parent and external saved consumers still authenticate their own expectation.
struct BindingInputs<'a> {
    subject: &'a CompleteSubject,
    request: &'a CompleteRequest,
    authority: &'a Arc<FrozenSourceAuthority>,
    coverage: &'a RawCoverage,
    presentation: &'a str,
    changed_paths: Vec<String>,
    configuration: FullConfiguration,
    config: &'a RiprConfig,
    check: &'a CheckInput,
    surface: ProducerSurface,
    policy: &'a CompleteRustPolicySnapshot,
    build_identity: &'a str,
    nonce: &'a str,
    profile: &'a CompleteVerificationLimits,
    deadline: Instant,
    retained: u64,
}

fn build_expected_binding(inputs: BindingInputs<'_>) -> Result<CompleteBinding, String> {
    let BindingInputs {
        subject, request, authority, coverage, presentation, changed_paths,
        configuration, config, check, surface, policy, build_identity, nonce,
        profile, deadline, retained,
    } = inputs;
    checkpoint(deadline)?;
    let inventory = authority.inventory();
    let count = inventory.files().len().checked_add(inventory.directories().len())
        .ok_or("whole-input binding inventory count overflow")?;
    let copies = inventory_retention(authority)?
        .checked_add(multiply(count as u64, 256)?)
        .ok_or("whole-input binding inventory copy overflow")?;
    // Pre-admit copied path/record/config metadata and canonical wire allowance.
    admit_sum(profile.max_buffered_bytes, &[
        retained, copies, multiply(profile.max_binding_bytes, 2)?,
    ])?;
    let text = |path: &Path| -> Result<String, String> {
        path.to_str().map(str::to_owned)
            .ok_or_else(|| "whole-input binding native path is not UTF-8".into())
    };
    let mut directories = Vec::new();
    directories.try_reserve_exact(inventory.directories().len())
        .map_err(|error| format!("whole-input binding directories: {error}"))?;
    for path in inventory.directories() {
        checkpoint(deadline)?;
        directories.push(text(path)?);
    }
    let mut files = Vec::new();
    files.try_reserve_exact(inventory.files().len())
        .map_err(|error| format!("whole-input binding files: {error}"))?;
    for (path, file) in inventory.files() {
        checkpoint(deadline)?;
        files.push(InventoryFile {
            path: text(path)?, git_mode: file.mode.git_mode().into(),
            blob_oid: file.blob_oid.as_str().into(), bytes: file.size,
            sha256: digest_text(&file.sha256)?,
        });
    }
    let configuration_binding = match authority.captured_configuration() {
        crate::analysis::git_candidate_execution::CapturedConfiguration::Absent => ConfigurationBinding::Absent,
        crate::analysis::git_candidate_execution::CapturedConfiguration::Present { blob_oid, text } => {
            ConfigurationBinding::Present {
                blob_oid: blob_oid.as_str().into(),
                sha256: super::complete_contract::sha256_bytes(text.as_bytes()),
                text: text.clone(),
            }
        }
        crate::analysis::git_candidate_execution::CapturedConfiguration::NotRequested => {
            return Err("whole-input binding configuration was not captured".into());
        }
    };
    let mode = match check.mode {
        Mode::Instant => CompleteMode::Instant,
        Mode::Draft => CompleteMode::Draft,
        Mode::Fast => CompleteMode::Fast,
        Mode::Deep => CompleteMode::Deep,
        Mode::Ready => CompleteMode::Ready,
    };
    let mut enabled_languages = config.languages().enabled().iter()
        .map(|language| language.as_str().to_string()).collect::<Vec<_>>();
    enabled_languages.sort_unstable();
    enabled_languages.dedup();
    let mut fields = config.check_artifact_identity_fields().into_iter()
        .filter(|field| field.role == crate::config::ConfigIdentityRole::FindingAffecting)
        .map(|field| PolicyField { name: field.name.into(), value: field.value.unwrap_or_default() })
        .collect::<Vec<_>>();
    fields.sort_by(|left, right| left.name.cmp(&right.name));
    let raw = coverage.summary();
    let options = subject.requested_options();
    let binding = CompleteBinding {
        schema_version: BINDING_SCHEMA.into(),
        subject: SubjectBinding {
            requested_root: options.root.clone(), requested_base: options.base.clone(),
            requested_head: options.head.clone(),
            invocation_repository: text(&subject.invocation_repository)?,
            logical_root: text(&subject.root)?, work_tree: text(&subject.work_tree)?,
            base_commit: subject.base_commit.as_str().into(), head_commit: subject.head_commit.as_str().into(),
            base_tree: subject.base_tree.as_str().into(), head_tree: subject.head_tree.as_str().into(),
            origin_commit: subject.origin_commit.as_str().into(), origin_tree: subject.origin_tree.as_str().into(),
            changed_paths,
        },
        committed_request: request.binding().clone(),
        raw: RawBinding {
            raw_sha256: raw.raw_sha256.clone(), ledger_sha256: raw.ledger_sha256.clone(),
            projection_sha256: raw.projection_sha256.clone(),
            raw_bytes: raw.raw_bytes as u64, ledger_bytes: raw.ledger_bytes as u64,
            records: raw.records as u64, sections: raw.sections as u64, hunks: raw.hunks as u64,
            changed_files: raw.changed_files as u64, added_lines: raw.added_lines as u64,
            removed_lines: raw.removed_lines as u64,
        },
        presentation: PresentationBinding {
            bytes: presentation.len() as u64,
            sha256: super::complete_contract::sha256_bytes(presentation.as_bytes()),
        },
        inventory: InventoryBinding {
            representation: INVENTORY_REPRESENTATION.into(), files, directories,
            logical_bytes: inventory.files().try_fold(0_u64, |sum, (_, file)| {
                sum.checked_add(file.size).ok_or("whole-input binding source byte overflow")
            })?,
        },
        configuration: configuration_binding, full_configuration: configuration,
        rust_execution_policy: wire_policy(policy)?,
        effective_options: EffectiveOptions {
            surface, mode, include_unchanged_tests: check.include_unchanged_tests,
            enabled_languages, check_input_base: check.base.clone(),
            git_timeout_ms: check.git_timeout.map(|duration| u64::try_from(duration.as_millis())
                .map_err(|error| format!("whole-input timeout conversion: {error}"))).transpose()?,
            config_identity_version: crate::config::CHECK_ARTIFACT_CONFIG_IDENTITY_VERSION,
            config_identity_hash: crate::config::check_artifact_config_identity_hash(config),
            loaded_config_identity: crate::config::loaded_config_identity(config),
            finding_affecting_fields: fields,
        },
        analyzer: AnalyzerBinding {
            build_identity: build_identity.into(),
            check_schema: crate::app::CHECK_OUTPUT_SCHEMA_VERSION.into(),
            analyzer_generation: crate::review_input::REVIEW_ANALYZER_GENERATION.into(),
        },
        nonce: nonce.into(), profile: profile.clone(),
    };
    binding.validate()?;
    checkpoint(deadline)?;
    authority.ensure_clean().map_err(|error| error.to_string())?;
    Ok(binding)
}

/// Compare components only to authenticate the original spelling's location.
/// The original OsStr is retained separately; internal analysis uses the exact
/// authority root. No live canonicalization or lexical-root substitution here.
fn caller_root_at(authority_root: &Path, root: &Path) -> Result<(), String> {
    if !root.is_absolute() || root.components().any(|part| matches!(part, Component::ParentDir)) {
        return Err("whole-input caller root is not an absolute admitted spelling".into());
    }
    if !root.components().eq(authority_root.components()) {
        return Err("whole-input caller root components differ from whole-subject root".into());
    }
    Ok(())
}

fn same_path(left: &Path, right: &Path) -> bool {
    left.as_os_str() == right.as_os_str()
}

fn same_optional_path(left: &Option<PathBuf>, right: &Option<PathBuf>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => same_path(left, right),
        (None, None) => true,
        _ => false,
    }
}

fn check_equal(left: &CheckInput, right: &CheckInput) -> bool {
    let CheckInput {
        root, base, diff_file, mode, format, include_unchanged_tests,
        perl_facts_path, suppression_policy, git_timeout, git_candidate,
    } = left;
    same_path(root, &right.root)
        && base == &right.base
        && same_optional_path(diff_file, &right.diff_file)
        && mode == &right.mode
        && format == &right.format
        && include_unchanged_tests == &right.include_unchanged_tests
        && same_optional_path(perl_facts_path, &right.perl_facts_path)
        && same_optional_path(suppression_policy, &right.suppression_policy)
        && git_timeout == &right.git_timeout
        && git_candidate.is_none() && right.git_candidate.is_none()
}

fn analysis_equal(left: &AnalysisOptions, right: &AnalysisOptions) -> bool {
    let AnalysisOptions {
        root, open_rust_index_paths, base, diff_file, mode, include_unchanged_tests,
        resolve_tsconfig_paths, perl_facts_path, git_timeout, production_like_targets,
        test_harnesses, git_candidate, resolved_subject_identity,
    } = left;
    same_path(root, &right.root)
        && open_rust_index_paths == &right.open_rust_index_paths
        && base == &right.base
        && same_optional_path(diff_file, &right.diff_file)
        && mode == &right.mode
        && include_unchanged_tests == &right.include_unchanged_tests
        && resolve_tsconfig_paths == &right.resolve_tsconfig_paths
        && same_optional_path(perl_facts_path, &right.perl_facts_path)
        && git_timeout == &right.git_timeout
        && production_like_targets == &right.production_like_targets
        && test_harnesses == &right.test_harnesses
        && git_candidate.is_none() && right.git_candidate.is_none()
        && resolved_subject_identity.is_none() && right.resolved_subject_identity.is_none()
}

fn changed_equal(left: &[ChangedFile], right: &[ChangedFile]) -> bool {
    left.len() == right.len() && left.iter().zip(right).all(|(left, right)| {
        let ChangedFile { path, added_lines, removed_lines } = left;
        same_path(path, &right.path)
            && added_lines == &right.added_lines
            && removed_lines == &right.removed_lines
    })
}

fn validate_surface_seed(
    check: &CheckInput,
    surface: ProducerSurface,
    options: &PrEvidenceOptions,
) -> Result<(), String> {
    if check.mode != Mode::Draft || check.format != OutputFormat::Json
        || check.diff_file.is_none() || check.perl_facts_path.is_some()
        || check.suppression_policy.is_some() || check.git_candidate.is_some()
    {
        return Err("whole-input surface seed differs from supported actual producer input".into());
    }
    match surface {
        ProducerSurface::Installed => {
            if check.base.is_some() || check.git_timeout.is_some() || !check.include_unchanged_tests {
                return Err("whole-input installed surface seed differs".into());
            }
        }
        ProducerSurface::Xtask => {
            // This is INPUT base only. The supplied-diff loader and ordinary
            // output builder still produce output.base / outcome base None.
            if check.base.as_deref() != Some(options.base.as_str()) || check.include_unchanged_tests {
                return Err("whole-input xtask surface seed differs".into());
            }
            // The sealed invocation must authenticate the actual existing CLI
            // timeout decoder's value, including None for explicit zero.
        }
    }
    Ok(())
}

fn tree_budget(profile: &CompleteVerificationLimits) -> Result<CompleteTreeBudget, String> {
    if profile.max_source_bytes > SOURCE_MAX {
        return Err("whole-input source profile exceeds the unchanged materializer cap".into());
    }
    Ok(CompleteTreeBudget {
        max_listing_bytes: native_size(profile.max_inventory_bytes.min(RAW_MAX))?,
        max_entries: native_size(profile.max_inventory_entries)?,
        max_path_bytes: profile.max_retained_path_bytes,
        max_source_bytes: profile.max_source_bytes,
        max_file_bytes: profile.file_size_bytes.min(RAW_MAX),
        max_configuration_bytes: crate::bounded_input::MAX_CLI_INPUT_BYTES
            .min(profile.max_binding_bytes).min(profile.max_buffered_bytes),
        max_buffered_bytes: profile.max_buffered_bytes,
    })
}

fn inventory_retention(authority: &FrozenSourceAuthority) -> Result<u64, String> {
    let mut bytes = 0_u64;
    for path in authority.inventory().directories() {
        let path = path.to_str().ok_or("whole-input inventory directory is not admitted UTF-8")?;
        bytes = bytes.checked_add(path.len() as u64).ok_or("whole-input inventory byte overflow")?;
    }
    for (path, _) in authority.inventory().files() {
        let path = path.to_str().ok_or("whole-input inventory file is not admitted UTF-8")?;
        bytes = bytes.checked_add(path.len() as u64).ok_or("whole-input inventory byte overflow")?;
    }
    // Logical path and per-entry record payload only; tree/node/allocator overhead is native-AS bounded.
    let entries = authority.inventory().directories().len()
        .checked_add(authority.inventory().files().len())
        .ok_or("whole-input inventory count overflow")?;
    bytes.checked_add(multiply(entries as u64, std::mem::size_of::<crate::analysis::committed_source::frozen::FrozenFile>() as u64)?)
        .ok_or_else(|| "whole-input inventory representation overflow".into())
}

fn verify_policy_file(
    authority: &FrozenSourceAuthority,
    request: &CompleteRequest,
) -> Result<(), String> {
    let bound = request.binding();
    bound.validate()?;
    let file = authority.inventory().files()
        .find(|(path, _)| same_path(path, Path::new(POLICY_PATH)))
        .map(|(_, file)| file)
        .ok_or("whole-input frozen committed request is missing")?;
    if file.mode != FrozenFileMode::Regular || file.blob_oid.as_str() != bound.blob_oid.as_str()
        || file.size > POLICY_MAX || file.size != bound.original_bytes.len() as u64
        || format!("sha256:{:x}", sha2::Sha256::digest(&bound.original_bytes)) != bound.sha256
    {
        return Err("whole-input frozen committed request identity differs".into());
    }
    let bytes = frozen::fs::read_with_limit(authority.logical_root().join(POLICY_PATH), POLICY_MAX)
        .map_err(|error| format!("whole-input frozen committed request read: {error}"))?;
    if bytes != bound.original_bytes {
        return Err("whole-input frozen committed request original bytes differ".into());
    }
    Ok(())
}

fn verify_source_current(
    authority: &Arc<FrozenSourceAuthority>,
    deadline: Instant,
) -> Result<(), String> {
    frozen::with_context(Some(Arc::clone(authority)), || {
        // Each directory list is consumed and dropped before the next. EOF
        // checks both actual anchored membership and complete named inventory.
        for path in authority.inventory().directories() {
            checkpoint(deadline)?;
            let entries = frozen::fs::read_dir(authority.logical_root().join(path))
                .map_err(|error| format!("whole-input source directory postflight: {error}"))?;
            for entry in entries {
                entry.map_err(|error| format!("whole-input source membership postflight: {error}"))?;
            }
        }
        for (path, _) in authority.inventory().files() {
            checkpoint(deadline)?;
            // Existing prefix reader hashes the whole opened file through EOF
            // while retaining zero prefix bytes, preserving SHA/type checks.
            frozen::fs::read_prefix(authority.logical_root().join(path), 0)
                .map_err(|error| format!("whole-input source byte postflight: {error}"))?;
        }
        authority.ensure_clean().map_err(|error| error.to_string())
    })
}

/// Only the root-owned actual worker-entry/cohort type can cross this boundary.
/// Its constructor and capture dispatch are genuine missing integration seams;
/// this leaf supplies no adapter, mock, default or no-op for either.
pub(super) fn prepare(
    mut invocation: QualifiedWholeInvocation,
    startup: NativeStartup,
    request: CompleteRequest,
    options: &PrEvidenceOptions,
    surface: ProducerSurface,
    mut surface_check: CheckInput,
    profile: &CompleteVerificationLimits,
) -> Result<FreshWholeInput, String> {
    invocation.validate_startup(&startup, options, surface, &surface_check)?;
    if startup.profile() != profile {
        return Err("whole-input profile differs from actual native startup".into());
    }
    profile.validate()?;
    checkpoint(startup.deadline())?;
    startup.verify_stage_current()?;
    if startup.source().deadline() != startup.deadline() {
        return Err("whole-input source deadline differs from actual held worker deadline".into());
    }
    startup.source().verify_fresh()?;
    validate_surface_seed(&surface_check, surface, options)?;
    let subject = invocation.take_subject()?;
    caller_root_at(&subject.root, &surface_check.root)?;
    let caller_root = surface_check.root.clone();
    let prepared = prepare_staged_named_tree(
        &subject.invocation_repository, subject.head_tree.clone(),
        Arc::clone(startup.source()), startup.deadline(), tree_budget(profile)?,
    ).map_err(|error| error.to_string())?;
    let authority = prepared.frozen_source_authority(&subject.root)
        .map_err(|error| error.to_string())?;
    if !same_path(authority.logical_root(), &subject.root)
        || authority.head_tree() != &subject.head_tree
    {
        return Err("whole-input frozen whole-head identity differs".into());
    }
    let inventory_bytes = inventory_retention(&authority)?;
    let captured_text_bytes = match authority.captured_configuration() {
        crate::analysis::git_candidate_execution::CapturedConfiguration::Present { text, .. } => text.len() as u64,
        _ => 0,
    };
    // Admit constructor/copy payloads before the existing config parser and
    // bounded FullConfiguration copier allocate. Parser/node overhead remains
    // under the actual native AS bound.
    admit_sum(profile.max_buffered_bytes, &[
        inventory_bytes, multiply(captured_text_bytes, 4)?,
        multiply(profile.max_binding_bytes, 2)?, 128 * 1024,
    ])?;
    let config = frozen::with_context(Some(Arc::clone(&authority)), || {
        verify_policy_file(&authority, &request)?;
        crate::config::config_for_captured_snapshot(
            authority.logical_root(), &authority.logical_root().join("ripr.toml"),
            authority.captured_configuration(),
        )
    })?;
    let configuration = FullConfiguration::capture(&config, profile)?;
    if config.perl().producer().is_some() {
        return Err("whole-input configured external Perl producer is unsupported".into());
    }
    // Use the same config application as each actual producer. Installed lets
    // config select both values; xtask's --no-unchanged-tests is explicit.
    crate::config::apply_to_check_input(&mut surface_check, &config, CheckInputExplicit {
        mode: false,
        include_unchanged_tests: matches!(surface, ProducerSurface::Xtask),
    });
    surface_check.root = authority.logical_root().to_path_buf();
    let analysis = crate::app::check::options_builder::analysis_options_from_input_and_config(
        &surface_check, &config,
    );
    let languages = config.languages().enabled().to_vec();
    let policy = capture_complete_rust_policy()?;
    let file_limit = admitted_file_limit(profile.file_limit, policy.diff_index_file_limit())?;
    let config_bytes = config.source_text.as_ref().map_or(0, |text| text.len() as u64);
    let configuration_size = configuration_bytes(&configuration, profile.max_binding_bytes)?;
    let fixed = admit_sum(profile.max_buffered_bytes, &[
        inventory_bytes, multiply(config_bytes, 4)?, multiply(configuration_size, 2)?, 128 * 1024,
    ])?;
    let budget = CaptureBudget::new(profile, fixed)?;
    let original = invocation.capture_original_inputs(&subject, budget)?;
    let (raw, presentation, changed_paths, name_bytes) = original.into_parts();
    checkpoint(startup.deadline())?;
    let capture_cap = profile.max_artifact_bytes[ArtifactRole::OriginalRaw.ordinal()]
        .min(profile.file_size_bytes).min(RAW_MAX);
    // Raw + decoder clone + simultaneous String/Arc payloads (3x lossy each), LF line-slice
    // scratch and both parsed changed-file payloads coexist in this phase.
    // Ledger observer preflights LF count before parser/hash; native AS also
    // bounds existing parser/node/allocator costs, not asserted by this sum.
    let records = raw.iter().filter(|byte| **byte == b'\n').count()
        .checked_add(usize::from(raw.last().is_some_and(|byte| *byte != b'\n')))
        .ok_or("whole-input raw record count overflow")?;
    if records as u64 > profile.max_raw_records {
        return Err("whole-input raw framing record admission exceeded".into());
    }
    let decode_lines = records.checked_add(1).ok_or("whole-input decoder line count overflow")?;
    let decode_scratch = multiply(decode_lines as u64, std::mem::size_of::<&str>() as u64)?;
    let projection_cap = profile.max_raw_projection_bytes;
    let retained = admit_sum(profile.max_buffered_bytes, &[
        fixed, presentation.len() as u64, name_bytes,
        multiply(raw.len() as u64, 8)?, decode_scratch, multiply(projection_cap, 2)?,
    ])?;
    let ledger_cap = profile.max_relation_bytes
        .min(profile.max_artifact_bytes[ArtifactRole::RawLedger.ordinal()])
        .min(profile.max_buffered_bytes.saturating_sub(retained));
    if ledger_cap == 0 {
        return Err("whole-input ledger has no admitted retained-byte capacity".into());
    }
    let limits = RawCoverageLimits {
        file_limit,
        max_raw_bytes: native_size(capture_cap)?,
        max_records: native_size(profile.max_raw_records)?,
        max_ledger_bytes: native_size(ledger_cap)?,
        max_retained_path_bytes: native_size(profile.max_retained_path_bytes)?,
        max_projection_bytes: native_size(projection_cap)?,
    };
    let (mut parsed, coverage) = build_raw_coverage(&raw, limits)?;
    let decoded = crate::analysis::diff::load::decode_diff_text(
        "whole-input canonical u0", raw.clone(),
    )?;
    let canonical: Arc<str> = Arc::from(decoded);
    let decoded_parsed = crate::analysis::diff::parse_unified_diff_bounded_with_metadata(&canonical)?;
    let projection = semantic_projection_digest(&decoded_parsed, limits.max_projection_bytes)?;
    if projection != coverage.summary().projection_sha256 {
        return Err("whole-input original raw and decoded full semantic projections differ".into());
    }
    drop(decoded_parsed);
    let pure_renames = std::mem::take(&mut parsed.pure_rename_paths);
    let mut rust_subject = std::mem::take(&mut parsed.changed_files);
    rust_subject.retain(|file| !pure_renames.iter().any(|path| same_path(path, &file.path)));
    let retained_input_bytes = admit_sum(profile.max_buffered_bytes, &[
        fixed, raw.len() as u64, canonical.len() as u64, coverage.ledger_bytes().len() as u64,
        projection_cap, presentation.len() as u64, name_bytes,
    ])?;
    let build_identity = crate::build_identity::cache_identity().to_string();
    let binding = build_expected_binding(BindingInputs {
        subject: &subject, request: &request, authority: &authority, coverage: &coverage,
        presentation: &presentation, changed_paths, configuration, config: &config,
        check: &surface_check, surface, policy: &policy, build_identity: &build_identity,
        nonce: startup.generation_nonce(), profile, deadline: startup.deadline(),
        retained: retained_input_bytes,
    })?;
    let generation_id = binding.generation_id()?;
    let binding_size = binding_bytes(&binding, profile.max_binding_bytes)?;
    let binding_entries = binding.inventory.files.len()
        .checked_add(binding.inventory.directories.len())
        .ok_or("whole-input retained binding count overflow")?;
    let retained_input_bytes = admit_sum(profile.max_buffered_bytes, &[
        retained_input_bytes, inventory_retention(&authority)?,
        multiply(binding_entries as u64, 256)?, multiply(binding_size, 2)?,
    ])?;
    startup.verify_stage_current()?;
    authority.ensure_clean().map_err(|error| error.to_string())?;
    checkpoint(startup.deadline())?;
    Ok(FreshWholeInput {
        whole: VerifiedWholeInput {
            phase: Phase::Fresh, deadline: startup.deadline(), authority, canonical,
            config: Arc::new(config), check: surface_check, options: analysis,
            languages, policy, projection, projection_limit: limits.max_projection_bytes,
            rust_subject,
        },
        startup, invocation, request, subject, caller_root, surface, raw, coverage,
        presentation, binding, generation_id, build_identity,
        profile: profile.clone(), retained_input_bytes,
    })
}

impl VerifiedWholeInput {
    fn running(&self) -> Result<(), String> {
        if self.phase != Phase::Running {
            return Err("whole-input validator requires the one running checker".into());
        }
        checkpoint(self.deadline)?;
        let current = frozen::current().ok_or("whole-input running source context is missing")?;
        let canonical = frozen::canonical_diff().ok_or("whole-input running canonical context is missing")?;
        if !Arc::ptr_eq(&current, &self.authority) || !Arc::ptr_eq(&canonical, &self.canonical) {
            return Err("whole-input running source or canonical Arc identity differs".into());
        }
        self.authority.ensure_clean().map_err(|error| error.to_string())
    }

    pub(crate) fn validate_check(&self, input: &CheckInput, config: &RiprConfig) -> Result<(), String> {
        self.running()?;
        if !check_equal(input, &self.check) || config != self.config.as_ref() {
            return Err("whole-input actual check input or complete configuration differs".into());
        }
        Ok(())
    }

    pub(crate) fn validate_analysis(
        &self, options: &AnalysisOptions, oracle: &OraclePolicy,
        languages: &[LanguageId], rust: &RustLanguageConfig,
    ) -> Result<(), String> {
        self.running()?;
        if !analysis_equal(options, &self.options) || oracle != self.config.oracles()
            || languages != self.languages || rust != &self.config.languages().rust
        {
            return Err("whole-input actual analysis options, oracle, language order or Rust configuration differs".into());
        }
        Ok(())
    }

    pub(crate) fn validate_canonical_diff(
        &self, authority: &Arc<FrozenSourceAuthority>, canonical: &Arc<str>,
    ) -> Result<(), String> {
        self.running()?;
        if !Arc::ptr_eq(authority, &self.authority) || !Arc::ptr_eq(canonical, &self.canonical) {
            return Err("whole-input supplied source or canonical Arc identity differs".into());
        }
        Ok(())
    }

    pub(crate) fn validate_parsed_diff(&self, parsed: &ParsedDiff) -> Result<(), String> {
        self.running()?;
        if semantic_projection_digest(parsed, self.projection_limit)? != self.projection {
            return Err("whole-input actual full parsed diff differs".into());
        }
        Ok(())
    }

    pub(crate) fn validate_rust_subject(&self, files: &[ChangedFile]) -> Result<(), String> {
        self.running()?;
        if !changed_equal(files, &self.rust_subject) {
            return Err("whole-input actual cross-language rename-filtered Rust subject differs".into());
        }
        Ok(())
    }

    pub(crate) fn validate_rust_policy(&self, policy: &CompleteRustPolicySnapshot) -> Result<(), String> {
        self.running()?;
        if policy != &self.policy {
            return Err("whole-input actual Rust policy snapshot differs".into());
        }
        Ok(())
    }

    pub(crate) fn validate_changed_line_limit(&self, limit: usize) -> Result<(), String> {
        self.running()?;
        // This read site supplies the existing numeric guard, not a newly
        // observed eligible-line count. Subject equality precedes filtering.
        if limit != self.policy.changed_rust_line_limit() {
            return Err("whole-input changed-Rust-line execution bound changed".into());
        }
        Ok(())
    }

    pub(crate) fn validate_partial_budgets(
        &self, files: usize, lines: usize, disclosures: &[String],
    ) -> Result<(), String> {
        self.running()?;
        if files != self.policy.partial_diff_file_budget()
            || lines != self.policy.partial_diff_line_budget()
            || disclosures != self.policy.partial_budget_disclosures()
        {
            return Err("whole-input actual partial budgets or clamp disclosures changed".into());
        }
        Ok(())
    }

    pub(crate) fn validate_index_limit(&self, limit: usize) -> Result<(), String> {
        self.running()?;
        if limit != self.policy.diff_index_file_limit() {
            return Err("whole-input actual diff index bound changed".into());
        }
        Ok(())
    }

    pub(crate) fn validate_narrow_limit(&self, limit: usize) -> Result<(), String> {
        self.running()?;
        if limit != self.policy.diff_narrow_index_limit() {
            return Err("whole-input actual narrow index bound changed".into());
        }
        Ok(())
    }

    pub(crate) fn validate_dependent_scope(&self, mode: &str) -> Result<(), String> {
        self.running()?;
        if mode != self.policy.dependent_scope_mode().as_str() {
            return Err("whole-input actual dependent scope mode changed".into());
        }
        Ok(())
    }
}

impl FreshWholeInput {
    pub(super) fn execute(mut self) -> Result<AnalyzedWholeInput, String> {
        checkpoint(self.startup.deadline())?;
        self.startup.verify_stage_current()?;
        self.whole.phase = Phase::Running;
        let output = frozen::with_context(Some(Arc::clone(&self.whole.authority)), || {
            frozen::with_canonical_diff(Arc::clone(&self.whole.canonical), || {
                crate::app::check::check_workspace_with_verified_whole(
                    self.whole.check.clone(), &self.whole.config, &self.whole,
                )
            })
        });
        self.whole.phase = Phase::Closed;
        let output = output?;
        self.whole.authority.ensure_clean().map_err(|error| error.to_string())?;
        self.startup.verify_stage_current()?;
        checkpoint(self.startup.deadline())?;
        Ok(AnalyzedWholeInput { input: self, output })
    }
}

impl<'a> WholeInputData<'a> {
    pub(super) fn subject(&self) -> &CompleteSubject { &self.input.subject }
    pub(super) fn caller_root(&self) -> &Path { &self.input.caller_root }
    pub(super) fn authority_root(&self) -> &Path { self.input.whole.authority.logical_root() }
    pub(super) fn surface(&self) -> ProducerSurface { self.input.surface }
    pub(super) fn check_input(&self) -> &CheckInput { &self.input.whole.check }
    pub(super) fn analysis_options(&self) -> &AnalysisOptions { &self.input.whole.options }
    pub(super) fn configuration(&self) -> &FullConfiguration { &self.input.binding.full_configuration }
    pub(super) fn config(&self) -> &RiprConfig { &self.input.whole.config }
    pub(super) fn rust_policy(&self) -> &CompleteRustPolicySnapshot { &self.input.whole.policy }
    pub(super) fn frozen(&self) -> &Arc<FrozenSourceAuthority> { &self.input.whole.authority }
    pub(super) fn raw(&self) -> &[u8] { &self.input.raw }
    pub(super) fn presentation(&self) -> &str { &self.input.presentation }
    pub(super) fn changed_paths(&self) -> &[String] { &self.input.binding.subject.changed_paths }
    pub(super) fn binding(&self) -> &CompleteBinding { &self.input.binding }
    pub(super) fn generation_id(&self) -> &str { &self.input.generation_id }
    pub(super) fn canonical_diff(&self) -> &str { &self.input.whole.canonical }
    pub(super) fn coverage(&self) -> &RawCoverage { &self.input.coverage }
    pub(super) fn committed_request(&self) -> &super::complete_request::CommittedRequestBinding {
        self.input.request.binding()
    }
    pub(super) fn build_identity(&self) -> &str { &self.input.build_identity }
    pub(super) fn generation_nonce(&self) -> &str { self.input.startup.generation_nonce() }
    pub(super) fn profile(&self) -> &CompleteVerificationLimits { &self.input.profile }

    /// Private genuine role/clock borrow, never returned to root's callback.
    fn artifact_context(&self) -> Result<(&RetainedDirectory, Instant), String> {
        let deadline = self.input.startup.deadline();
        checkpoint(deadline)?;
        if self.input.whole.phase != Phase::Closed {
            return Err("whole-input artifact preparation precedes completed analysis".into());
        }
        if self.input.whole.deadline != deadline
            || self.input.startup.source().deadline() != deadline
        {
            return Err("whole-input artifact original held deadlines differ".into());
        }
        if self.input.startup.profile() != &self.input.profile {
            return Err("whole-input artifact profile differs from actual startup".into());
        }
        require_artifact_context(&self.input.whole.authority, &self.input.whole.canonical)?;
        self.input.startup.source().verify_materialized()?;
        verify_artifact_native_limits(&self.input.profile, deadline)?;
        self.input.startup.verify_stage_current()?;
        checkpoint(deadline)?;
        Ok((self.input.startup.artifacts(), deadline))
    }

    /// One live consuming attempt for the actual artifact role. Root admits
    /// all serialization/retained aliases and manifest growth before calling;
    /// file DATA never proves allocated stage usage or publication authority.
    pub(super) fn stage_artifacts(
        &self,
        payloads: [&[u8]; 9],
        manifest: impl FnOnce(&ArtifactPayloadData) -> Result<Vec<u8>, String>,
    ) -> Result<ArtifactClosureData, String> {
        self.artifact_attempt.claim()?;
        let result = (|| {
            let (directory, deadline) = self.artifact_context()?;
            let budget = artifact_budget(&self.input.profile)?;
            emit_artifacts_after_claim(
                &self.artifact_attempt, directory, deadline, budget, payloads,
                || self.artifact_context().map(|_| ()), manifest,
            )
        })();
        self.artifact_attempt.remember(result)
    }

    /// Saved verification runs under the still-held genuine source/native
    /// owners. Neither IO closure nor caller expectation can mint this result.
    /// Root's outer worker custody supplies syscall/process deadline enforcement.
    pub(super) fn verify_artifacts(&self) -> Result<VerifiedGeneration, String> {
        let observed = self.artifact_attempt.claim_verification()?;
        let result = (|| {
            let (_, deadline) = self.artifact_context()?;
            self.artifact_attempt.ensure_verifying()?;
            let files = observed.files.map(|file| file.bytes());
            combined_verifier_bytes(
                self.input.retained_input_bytes, &files,
                observed.manifest.bytes(), &self.input.profile,
            )?;
            self.input.binding.validate()?;
            if self.input.binding.generation_id()? != self.input.generation_id {
                return Err("whole-input expected generation binding changed".into());
            }
            if capture_complete_rust_policy()? != self.input.whole.policy
                || crate::build_identity::cache_identity() != self.input.build_identity
            {
                return Err("whole-input policy or build changed before verification".into());
            }
            // Actual verifier opens/rechecks the bound artifact role itself;
            // no raw FD or wider native-owner getter escapes this leaf.
            let verified = verify_staged_generation(
                Path::new(&self.input.startup.binding().artifacts.path),
                &self.input.binding, &self.input.profile,
            ).map_err(|error| error.to_string())?;
            reconcile_observed_generation(&observed, &verified, &self.input.generation_id)?;
            self.artifact_attempt.ensure_verifying()?;
            self.artifact_context()?;
            if capture_complete_rust_policy()? != self.input.whole.policy
                || crate::build_identity::cache_identity() != self.input.build_identity
            {
                return Err("whole-input policy or build changed during verification".into());
            }
            checkpoint(deadline)?;
            self.artifact_attempt.verified_after_success(&verified)?;
            Ok(verified)
        })();
        self.artifact_attempt.remember(result)
    }

}

impl AnalyzedWholeInput {
    /// Root preserves caller-facing output/probe spelling with its typed
    /// rebase, while keeping supplied-diff output/outcome base None. All
    /// preparation here precedes original-literal postflight and is unqualified.
    pub(super) fn stage<T>(
        mut self,
        work: impl FnOnce(&mut CheckOutput, &WholeInputData<'_>) -> Result<T, String>,
    ) -> Result<StagedWholeInput<T>, String> {
        checkpoint(self.input.startup.deadline())?;
        let data = WholeInputData {
            input: &self.input,
            artifact_attempt: ArtifactAttempt::new(),
        };
        let staged = frozen::with_context(Some(Arc::clone(&self.input.whole.authority)), || {
            frozen::with_canonical_diff(Arc::clone(&self.input.whole.canonical), || {
                work(&mut self.output, &data)
            })
        });
        let staged = data.artifact_attempt.finish_verified_work(staged)?;
        self.input.whole.authority.ensure_clean().map_err(|error| error.to_string())?;
        checkpoint(self.input.startup.deadline())?;
        Ok(StagedWholeInput { analyzed: self, staged })
    }
}

impl<T> StagedWholeInput<T> {
    /// This observed postflight is not group settlement or return-time atomic
    /// currentness. Root still owns final custody and generation qualification.
    pub(super) fn validate_current(self) -> Result<WholeInputEvidence<T>, String> {
        let StagedWholeInput { analyzed, staged } = self;
        let FreshWholeInput {
            whole, startup, invocation, request, subject, caller_root: _, surface: _,
            raw: _, coverage: _, presentation: _, binding, generation_id: _,
            build_identity, profile, retained_input_bytes,
        } = analyzed.input;
        checkpoint(startup.deadline())?;
        startup.verify_stage_current()?;
        super::complete_execution::verify_limits(
            profile.address_space_bytes, profile.file_size_bytes,
        )?;
        if startup.profile() != &profile || capture_complete_rust_policy()? != whole.policy
            || crate::build_identity::cache_identity() != build_identity
        {
            return Err("whole-input native profile, Rust policy or actual build identity changed".into());
        }
        // The bounded copier's maximum new payload is reserved before growth.
        // Root's staged artifact/output buffers have their own admitted phase.
        admit_sum(profile.max_buffered_bytes, &[retained_input_bytes, profile.max_binding_bytes])?;
        // This reconciles the retained immutable effective config. It is not
        // a reobservation of dirty live ripr.toml. Root must supply the genuine
        // bounded live-config pre/postflight join before production admission.
        let observed = FullConfiguration::capture(&whole.config, &profile)?;
        if observed != binding.full_configuration {
            return Err("whole-input complete effective configuration changed".into());
        }
        drop(observed);
        verify_source_current(&whole.authority, startup.deadline())?;
        frozen::with_context(Some(Arc::clone(&whole.authority)), || {
            verify_policy_file(&whole.authority, &request)
        })?;
        request.validate_whole_current(&subject, startup.deadline())?;
        startup.verify_stage_current()?;
        checkpoint(startup.deadline())?;
        let VerifiedWholeInput { authority, .. } = whole;
        authority.finalize().map_err(|error| format!("whole-input source lease closeout: {error}"))?;
        // Retain qualified invocation/native custody through all observations
        // and source-context closeout. Their release grants no publication.
        drop(invocation);
        drop(startup);
        Ok(WholeInputEvidence { staged })
    }
}

impl<T> WholeInputEvidence<T> {
    pub(super) fn into_staged_data(self) -> T { self.staged }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::diff::ChangedLine;
    use crate::analysis::diff::parse::parse_unified_diff_bounded_with_metadata;

    fn error<T>(result: Result<T, String>, category: &str) -> Result<(), String> {
        match result {
            Err(error) if error.contains(category) => Ok(()),
            Err(error) => Err(format!("wrong error category: {error}; expected {category}")),
            Ok(_) => Err(format!("unexpected success; expected {category}")),
        }
    }

    #[test]
    fn input_and_analysis_comparisons_bind_base_timeout_and_exact_root_spelling() {
        let input = CheckInput { root: PathBuf::from("/repo"), ..CheckInput::default() };
        let options = crate::app::check::options_builder::analysis_options_from_input_and_config(
            &input, &RiprConfig::default(),
        );
        assert!(check_equal(&input, &input));
        assert!(analysis_equal(&options, &options));
        let mut changed = input.clone();
        changed.root = PathBuf::from("/repo/.");
        assert!(!check_equal(&input, &changed));
        changed = input.clone();
        changed.base = Some("original-base".into());
        assert!(!check_equal(&input, &changed));
        let mut changed_options = options.clone();
        changed_options.git_timeout = Some(std::time::Duration::from_secs(300));
        assert!(!analysis_equal(&options, &changed_options));
        changed_options = options.clone();
        changed_options.base = Some("original-base".into());
        assert!(!analysis_equal(&options, &changed_options));
    }

    #[test]
    fn changed_subject_comparison_preserves_order_paths_text_and_both_coordinates() {
        let line = ChangedLine { line: 2, new_side_line: 7, text: "old".into() };
        let original = vec![
            ChangedFile { path: PathBuf::from("a.rs"), removed_lines: vec![line], ..ChangedFile::default() },
            ChangedFile { path: PathBuf::from("b.py"), ..ChangedFile::default() },
        ];
        assert!(changed_equal(&original, &original));
        let mut changed = original.clone();
        changed.swap(0, 1);
        assert!(!changed_equal(&original, &changed));
        changed = original.clone();
        changed[0].removed_lines[0].new_side_line = 2;
        assert!(!changed_equal(&original, &changed));
        changed = original.clone();
        changed[0].removed_lines[0].line = 3;
        assert!(!changed_equal(&original, &changed));
        changed = original.clone();
        changed[0].removed_lines[0].text = "different".into();
        assert!(!changed_equal(&original, &changed));
        changed = original.clone();
        changed[1].path = PathBuf::from("a.rs");
        assert!(!changed_equal(&original, &changed));
    }

    #[test]
    fn full_projection_preserves_metadata_while_adapter_subject_filters_pure_renames() -> Result<(), String> {
        let text = "diff --git a/a.rs b/b.rs\nsimilarity index 100%\nrename from a.rs\nrename to b.rs\ndiff --git a/c.py b/c.py\n--- a/c.py\n+++ b/c.py\n@@ -1 +1 @@\n-old\n+new\n";
        let mut parsed = parse_unified_diff_bounded_with_metadata(text)?;
        let original = semantic_projection_digest(&parsed, 16384)?;
        assert_eq!(parsed.pure_rename_file_count, 1);
        parsed.deleted_file_count += 1;
        assert_ne!(semantic_projection_digest(&parsed, 16384)?, original);
        parsed.deleted_file_count -= 1;
        let pure = std::mem::take(&mut parsed.pure_rename_paths);
        parsed.changed_files.retain(|file| !pure.iter().any(|path| same_path(path, &file.path)));
        assert_eq!(parsed.changed_files.len(), 1);
        assert_eq!(parsed.changed_files[0].path, PathBuf::from("c.py"));
        Ok(())
    }

    #[test]
    fn surface_seed_preserves_input_only_base_and_distinct_unchanged_options() -> Result<(), String> {
        let options = PrEvidenceOptions {
            root: ".".into(), base: "original-base".into(), base_explicit: true,
            head: "HEAD".into(), check: false,
        };
        let installed = CheckInput {
            root: PathBuf::from("/repo/."), diff_file: Some(PathBuf::from("check.diff")),
            format: OutputFormat::Json, ..CheckInput::default()
        };
        validate_surface_seed(&installed, ProducerSurface::Installed, &options)?;
        let mut xtask = installed.clone();
        xtask.base = Some(options.base.clone());
        xtask.include_unchanged_tests = false;
        xtask.git_timeout = Some(std::time::Duration::from_secs(300));
        validate_surface_seed(&xtask, ProducerSurface::Xtask, &options)?;
        error(validate_surface_seed(&xtask, ProducerSurface::Installed, &options), "installed surface")?;
        xtask.base = None;
        error(validate_surface_seed(&xtask, ProducerSurface::Xtask, &options), "xtask surface")?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn native_path_comparison_does_not_merge_distinct_lossy_spellings() {
        use std::os::unix::ffi::OsStringExt;
        let left = vec![ChangedFile {
            path: PathBuf::from(std::ffi::OsString::from_vec(b"a\xff.rs".to_vec())),
            ..ChangedFile::default()
        }];
        let right = vec![ChangedFile {
            path: PathBuf::from(std::ffi::OsString::from_vec(b"a\xfe.rs".to_vec())),
            ..ChangedFile::default()
        }];
        assert_eq!(left[0].path.to_string_lossy(), right[0].path.to_string_lossy());
        assert!(!changed_equal(&left, &right));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn caller_root_components_admit_spelling_without_equating_internal_root() -> Result<(), String> {
        let authority = Path::new("/repo");
        let caller = Path::new("/repo/.");
        caller_root_at(authority, caller)?;
        assert!(!same_path(authority, caller));
        error(caller_root_at(authority, Path::new("/repo/sub")), "components differ")?;
        error(caller_root_at(authority, Path::new("/repo/sub/..")), "absolute admitted spelling")?;
        Ok(())
    }

    #[test]
    fn raw_profile_admission_keeps_saved_ledger_header_compatible_with_index_bound() -> Result<(), String> {
        use super::super::raw_coverage::verify_raw_coverage;
        let raw = b"diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -1 +1 @@\n-old\n+new\n";
        let limits = |file_limit| RawCoverageLimits {
            file_limit, max_raw_bytes: 4096, max_records: 32,
            max_ledger_bytes: 32768, max_retained_path_bytes: 4096,
            max_projection_bytes: 32768,
        };
        for profile in [10, 20] {
            let admitted = admitted_file_limit(profile, 20)?;
            let (_, coverage) = build_raw_coverage(raw, limits(admitted))?;
            let (_, observed) = verify_raw_coverage(raw, coverage.ledger_bytes(), limits(native_size(profile)?))?;
            assert_eq!(&observed, coverage.summary());
        }
        error(admitted_file_limit(21, 20), "profile file limit exceeds the actual diff index bound")?;
        // The previous min-only path could build evidence no actual consumer
        // accepted: its committed header says20 while the profile says21.
        let (_, old) = build_raw_coverage(raw, limits(20))?;
        error(verify_raw_coverage(raw, old.ledger_bytes(), limits(21)), "canonical ledger differs")?;
        Ok(())
    }

    #[test]
    fn logical_phase_admission_refuses_overflow_and_combined_buffers() -> Result<(), String> {
        assert_eq!(admit_sum(16, &[3, 5, 8])?, 16);
        error(admit_sum(16, &[3, 5, 9]), "buffer bound")?;
        error(admit_sum(u64::MAX, &[u64::MAX, 1]), "overflow")?;
        error(multiply(u64::MAX, 3), "overflow")?;
        Ok(())
    }


    use std::fs;
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    use std::os::unix::fs::MetadataExt;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    static ARTIFACT_NEXT: AtomicU64 = AtomicU64::new(0);

    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    struct ArtifactFixture {
        root: PathBuf,
        directory: RetainedDirectory,
        deadline: Instant,
    }

    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    impl ArtifactFixture {
        fn new() -> Result<Self, String> {
            let base = std::env::temp_dir().canonicalize()
                .map_err(|error| format!("whole artifact fixture root: {error}"))?;
            let root = base.join(format!(
                "ripr-whole-artifact-{}-{}",
                std::process::id(), ARTIFACT_NEXT.fetch_add(1, Ordering::Relaxed),
            ));
            fs::create_dir(&root)
                .map_err(|error| format!("whole artifact fixture create: {error}"))?;
            let role = root.join("artifacts");
            fs::create_dir(&role)
                .map_err(|error| format!("whole artifact fixture role: {error}"))?;
            let metadata = fs::metadata(&role)
                .map_err(|error| format!("whole artifact fixture metadata: {error}"))?;
            let identity = crate::analysis::committed_source::staged::DirectoryIdentity {
                path: role.to_str().ok_or("whole artifact fixture path is not UTF-8")?.into(),
                dev: metadata.dev(), ino: metadata.ino(),
            };
            let deadline = Instant::now().checked_add(Duration::from_secs(60))
                .ok_or("whole artifact fixture deadline overflow")?;
            let directory = RetainedDirectory::open_absolute(&identity, 4096, deadline)?;
            Ok(Self { root, directory, deadline })
        }

        fn cleanup(self) -> Result<(), String> {
            drop(self.directory);
            fs::remove_dir_all(self.root)
                .map_err(|error| format!("whole artifact fixture cleanup: {error}"))
        }
    }

    fn artifact_test_profile() -> Result<CompleteVerificationLimits, String> {
        Ok(super::super::complete_contract::tests::fixture_binding()?.profile)
    }

    #[test]
    fn artifact_actual_contract_mapping_and_consumer_caps_are_not_normalized() -> Result<(), String> {
        require_artifact_mapping(&ARTIFACT_MAPPING)?;
        assert_eq!(ArtifactRole::ALL.len(), ArtifactSlot::ALL.len());
        for (role, slot) in ARTIFACT_MAPPING {
            assert_eq!(role.path(), slot.path());
            assert_eq!(role.ordinal(), slot.ordinal());
        }
        let mut reordered = ARTIFACT_MAPPING;
        reordered.swap(0, 1);
        error(require_artifact_mapping(&reordered), "role/path/ordinal")?;
        let mut mismatched = ARTIFACT_MAPPING;
        mismatched[1].1 = ArtifactSlot::PresentationDiff;
        error(require_artifact_mapping(&mismatched), "role/path/ordinal")?;
        let profile = artifact_test_profile()?;
        artifact_budget(&profile)?;
        let mut changed = profile.clone();
        changed.file_size_bytes = 4 * 1024 * 1024;
        changed.max_artifact_bytes[ArtifactRole::FindingIndex.ordinal()] = 2 * 1024 * 1024 + 1;
        error(artifact_budget(&changed), "existing consumer limit")?;
        changed = profile.clone();
        changed.max_artifact_bytes[ArtifactRole::ReviewInput.ordinal()] = 128 * 1024 + 1;
        error(artifact_budget(&changed), "existing consumer limit")?;
        changed = profile;
        changed.max_artifact_bytes[ArtifactRole::OriginalRaw.ordinal()] = 0;
        error(artifact_budget(&changed), "artifact byte bound")?;
        Ok(())
    }

    #[test]
    fn artifact_claim_before_admission_keeps_first_failure_and_unfinished_work_refuses() -> Result<(), String> {
        let attempt = ArtifactAttempt::new();
        error(attempt.finish_work(Ok(())), "did not finish")?;
        attempt.claim()?;
        error(attempt.finish_work(Ok(())), "did not finish")?;
        error(attempt.remember::<()>(Err("actual initial admission failure".into())), "initial admission")?;
        error(attempt.claim(), "initial admission")?;
        let later = match attempt.remember::<()>(Err("later failure".into())) {
            Err(error) => error,
            Ok(()) => return Err("later failed artifact operation unexpectedly succeeded".into()),
        };
        assert!(later.contains("later failure"));
        assert!(later.contains("actual initial admission failure"));
        error(attempt.finish_work(Ok(())), "initial admission")?;
        let combined = match attempt.finish_work::<()>(Err("outer work failure".into())) {
            Err(error) => error,
            Ok(()) => return Err("failed artifact work unexpectedly finished".into()),
        };
        assert!(combined.contains("outer work failure"));
        assert!(combined.contains("actual initial admission failure"));
        Ok(())
    }

    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    #[test]
    fn artifact_manifest_callback_cannot_swallow_reentrant_reuse_or_create_authority_file() -> Result<(), String> {
        let fixture = ArtifactFixture::new()?;
        let attempt = ArtifactAttempt::new();
        attempt.claim()?;
        let result = emit_artifacts_after_claim(
            &attempt, &fixture.directory, fixture.deadline,
            artifact_budget(&artifact_test_profile()?)?, [b"payload"; 9],
            || checkpoint(fixture.deadline),
            |_| {
                error(attempt.claim(), "repeated or reentrant")?;
                Ok(b"callback swallowed reuse".to_vec())
            },
        );
        error(attempt.remember(result), "repeated or reentrant")?;
        error(attempt.finish_work(Ok(())), "repeated or reentrant")?;
        assert!(!fixture.root.join("artifacts")
            .join(super::super::complete_contract::MANIFEST_FILE).exists());
        for role in ArtifactRole::ALL {
            assert_eq!(fs::read(fixture.root.join("artifacts").join(role.path()))
                .map_err(|error| format!("whole artifact emitted bytes: {error}"))?, b"payload");
        }
        fixture.cleanup()
    }

    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    #[test]
    fn artifact_actual_io_closure_is_one_shot_and_new_directory_recovers() -> Result<(), String> {
        let fixture = ArtifactFixture::new()?;
        let attempt = ArtifactAttempt::new();
        attempt.claim()?;
        let observed = emit_artifacts_after_claim(
            &attempt, &fixture.directory, fixture.deadline,
            artifact_budget(&artifact_test_profile()?)?, [b"payload"; 9],
            || checkpoint(fixture.deadline), |_| Ok(b"manifest DATA only".to_vec()),
        )?;
        assert_eq!(observed.manifest().name(), super::super::complete_contract::MANIFEST_FILE);
        assert_eq!(attempt.finish_work(Ok("IO-only completion"))?, "IO-only completion");
        error(attempt.claim(), "repeated or reentrant")?;
        error(attempt.finish_work(Ok(())), "repeated or reentrant")?;
        fixture.cleanup()?;

        let fixture = ArtifactFixture::new()?;
        let attempt = ArtifactAttempt::new();
        attempt.claim()?;
        let result = emit_artifacts_after_claim(
            &attempt, &fixture.directory, fixture.deadline,
            artifact_budget(&artifact_test_profile()?)?, [b"payload"; 9],
            || checkpoint(fixture.deadline),
            |_| Err("actual bounded manifest preparation refusal".into()),
        );
        error(attempt.remember(result), "manifest preparation refusal")?;
        assert!(!fixture.root.join("artifacts")
            .join(super::super::complete_contract::MANIFEST_FILE).exists());
        error(attempt.finish_work(Ok(())), "manifest preparation refusal")?;
        fixture.cleanup()?;

        let fixture = ArtifactFixture::new()?;
        let attempt = ArtifactAttempt::new();
        attempt.claim()?;
        let result = emit_artifacts_after_claim(
            &attempt, &fixture.directory, fixture.deadline,
            artifact_budget(&artifact_test_profile()?)?, [b"recovery"; 9],
            || checkpoint(fixture.deadline), |_| Ok(b"new owned stage".to_vec()),
        );
        attempt.remember(result)?;
        attempt.finish_work(Ok(()))?;
        fixture.cleanup()
    }

    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    #[test]
    fn artifact_post_callback_original_deadline_refusal_precedes_manifest_creation() -> Result<(), String> {
        let fixture = ArtifactFixture::new()?;
        let held = Instant::now().checked_add(Duration::from_secs(1))
            .ok_or("whole artifact callback clock overflow")?;
        let callback_reached = std::cell::Cell::new(false);
        let attempt = ArtifactAttempt::new();
        attempt.claim()?;
        let result = emit_artifacts_after_claim(
            &attempt, &fixture.directory, held,
            artifact_budget(&artifact_test_profile()?)?, [b"payload"; 9],
            || checkpoint(held),
            |_| {
                callback_reached.set(true);
                std::thread::sleep(Duration::from_secs(1));
                Ok(b"late manifest".to_vec())
            },
        );
        assert!(callback_reached.get(), "deadline control never reached actual manifest callback");
        error(attempt.remember(result), "deadline expired")?;
        error(attempt.finish_work(Ok(())), "deadline expired")?;
        assert!(!fixture.root.join("artifacts")
            .join(super::super::complete_contract::MANIFEST_FILE).exists());
        fixture.cleanup()
    }

    #[test]
    fn artifact_native_helper_preserves_expired_and_real_nonmatching_limit_refusals() -> Result<(), String> {
        let mut profile = artifact_test_profile()?;
        error(verify_artifact_native_limits(&profile, Instant::now()), "deadline expired")?;
        profile.address_space_bytes = 1;
        let deadline = Instant::now().checked_add(Duration::from_secs(10))
            .ok_or("whole artifact native observation clock overflow")?;
        match verify_artifact_native_limits(&profile, deadline) {
            Err(error) if error.starts_with("experimental worker nonfinite Max address space")
                || error.starts_with("experimental worker Max address space is not the requested") => Ok(()),
            Err(error) => Err(format!("wrong actual artifact native refusal: {error}")),
            Ok(()) => Err("actual artifact native limits unexpectedly matched one byte".into()),
        }
    }

    #[test]
    fn artifact_context_requires_both_actual_arc_identities_and_sticky_source_cleanliness() -> Result<(), String> {
        let base = std::env::temp_dir().canonicalize()
            .map_err(|error| format!("artifact context fixture root: {error}"))?;
        let root = base.join(format!(
            "ripr-artifact-context-{}-{}",
            std::process::id(), ARTIFACT_NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root)
            .map_err(|error| format!("artifact context fixture create: {error}"))?;
        let git = |args: &[&str]| crate::testing::fixture_git::fixture_git_ok(&root, args);
        git(&["init", "--initial-branch=main"])?;
        git(&["config", "user.email", "ripr@example.invalid"])?;
        git(&["config", "user.name", "ripr test"])?;
        fs::write(root.join("head.rs"), b"pub fn head() {}\n")
            .map_err(|error| format!("artifact context source fixture: {error}"))?;
        git(&["add", "."])?;
        git(&["commit", "-m", "actual frozen context"])?;
        let prepare = || -> Result<Arc<FrozenSourceAuthority>, String> {
            crate::analysis::git_candidate_execution::prepare_named_tree(
                &root, "HEAD", Some(Duration::from_secs(30)),
            ).map_err(|error| error.to_string())?
                .frozen_source_authority(&root).map_err(|error| error.to_string())
        };
        let prepared = crate::analysis::git_candidate_execution::prepare_named_tree(
            &root, "HEAD", Some(Duration::from_secs(30)),
        ).map_err(|error| error.to_string())?;
        let physical = prepared.physical_root().to_path_buf();
        let authority = prepared.frozen_source_authority(&root)
            .map_err(|error| error.to_string())?;
        let foreign = prepare()?;
        assert_eq!(authority.head_tree(), foreign.head_tree());
        let canonical: Arc<str> = Arc::from("canonical original diff");
        let same_bytes: Arc<str> = Arc::from("canonical original diff");
        error(require_artifact_context(&authority, &canonical), "lacks its frozen context")?;
        frozen::with_context(Some(Arc::clone(&authority)), || -> Result<(), String> {
            error(require_artifact_context(&authority, &canonical), "lacks its canonical diff")?;
            frozen::with_canonical_diff(Arc::clone(&canonical), || -> Result<(), String> {
                require_artifact_context(&authority, &canonical)?;
                frozen::with_context(Some(Arc::clone(&foreign)), || {
                    error(require_artifact_context(&authority, &canonical), "frozen context identity")
                })?;
                frozen::with_canonical_diff(Arc::clone(&same_bytes), || {
                    error(require_artifact_context(&authority, &canonical), "canonical context identity")
                })?;
                // Untracked absence is legitimate; mutate an actually admitted
                // physical blob to exercise the real sticky source fault.
                fs::write(physical.join("head.rs"), b"pub fn evil() {}\n")
                    .map_err(|error| format!("artifact context actual blob mutation: {error}"))?;
                error(
                    frozen::fs::read(root.join("head.rs"))
                        .map_err(|error| error.to_string()),
                    "frozen source mismatch",
                )?;
                error(require_artifact_context(&authority, &canonical), "frozen source mismatch")?;
                Ok(())
            })
        })?;
        drop(canonical);
        drop(same_bytes);
        drop(foreign);
        drop(authority);
        fs::remove_dir_all(root).map_err(|error| format!("artifact context fixture cleanup: {error}"))
    }


    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    #[test]
    fn artifact_scalar_adapter_preserves_each_actual_role_cap_before_creation() -> Result<(), String> {
        let mut profile = artifact_test_profile()?;
        profile.file_size_bytes = 64;
        profile.max_manifest_bytes = 64;
        profile.max_artifact_bytes = std::array::from_fn(|ordinal| ordinal as u64 + 1);
        profile.max_total_artifact_bytes = 45;
        let originals: [Vec<u8>; 9] = std::array::from_fn(|ordinal| vec![b'x'; ordinal + 1]);
        for ordinal in 0..9 {
            let fixture = ArtifactFixture::new()?;
            let mut changed = originals.clone();
            changed[ordinal].push(b'x');
            let payloads = std::array::from_fn(|index| changed[index].as_slice());
            let result = ArtifactDirectory::new(
                &fixture.directory, payloads, artifact_budget(&profile)?, fixture.deadline,
            );
            error(result, "payload exceeds its admitted cap before creation")?;
            fixture.directory.require_empty(fixture.deadline)?;
            fixture.cleanup()?;
        }
        let fixture = ArtifactFixture::new()?;
        let payloads = std::array::from_fn(|index| originals[index].as_slice());
        let attempt = ArtifactAttempt::new();
        attempt.claim()?;
        let result = emit_artifacts_after_claim(
            &attempt, &fixture.directory, fixture.deadline, artifact_budget(&profile)?, payloads,
            || checkpoint(fixture.deadline), |_| Ok(b"exact caps".to_vec()),
        );
        let observed = attempt.remember(result)?;
        for (ordinal, file) in observed.payloads().files().iter().enumerate() {
            assert_eq!(file.bytes(), profile.max_artifact_bytes[ordinal]);
        }
        assert_eq!(observed.payloads().payload_bytes(), 45);
        attempt.finish_work(Ok(()))?;
        fixture.cleanup()
    }


    #[test]
    fn verifier_combined_phase_charges_actual_caller_retention_and_overflow()
    -> Result<(), String> {
        let mut profile = artifact_test_profile()?;
        let files = [10_u64; 9];
        let manifest = 20_u64;
        let own = artifact_buffer_allowance(manifest, &files, &profile)?;
        let exact = own.checked_add(90 + 20 + 640 + 7)
            .ok_or("combined fixture accounting overflow")?;
        profile.max_buffered_bytes = exact;
        assert_eq!(combined_verifier_bytes(7, &files, manifest, &profile)?, exact);
        profile.max_buffered_bytes = exact - 1;
        assert!(own <= profile.max_buffered_bytes);
        error(combined_verifier_bytes(7, &files, manifest, &profile), "buffer bound")?;
        error(combined_verifier_bytes(u64::MAX, &files, manifest, &profile), "overflow")?;
        profile.max_buffered_bytes = exact;
        assert_eq!(combined_verifier_bytes(7, &files, manifest, &profile)?, exact);
        Ok(())
    }

    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    #[test]
    fn actual_io_closed_state_cannot_finish_analyzed_stage_without_verification()
    -> Result<(), String> {
        let fixture = ArtifactFixture::new()?;
        let attempt = ArtifactAttempt::new();
        attempt.claim()?;
        let observed = emit_artifacts_after_claim(
            &attempt, &fixture.directory, fixture.deadline,
            artifact_budget(&artifact_test_profile()?)?, [b"payload"; 9],
            || checkpoint(fixture.deadline), |_| Ok(b"not a saved proof".to_vec()),
        )?;
        // Real IO completion is allowed by its test helper, never by stage.
        attempt.finish_work(Ok(()))?;
        error(attempt.finish_verified_work(Ok(())), "in-custody verification")?;
        let closed = attempt.claim_verification()?;
        assert_eq!(closed.manifest.identity(), observed.manifest().identity());
        assert_eq!(closed.manifest.bytes(), observed.manifest().bytes());
        attempt.ensure_verifying()?;
        error(attempt.finish_verified_work(Ok(())), "in-custody verification")?;
        error(attempt.claim_verification(), "repeated or precedes closure")?;
        error(attempt.ensure_verifying(), "repeated or precedes closure")?;
        error(attempt.finish_verified_work(Ok(())), "repeated or precedes closure")?;
        fixture.cleanup()
    }

    #[test]
    fn verification_claim_is_first_fault_and_cannot_be_swallowed_by_outer_work()
    -> Result<(), String> {
        let attempt = ArtifactAttempt::new();
        error(attempt.claim_verification(), "precedes closure")?;
        error(attempt.claim(), "precedes closure")?;
        let failure = match attempt.finish_verified_work::<()>(Err("actual later work failure".into())) {
            Err(error) => error,
            Ok(()) => return Err("unverified failed work unexpectedly passed".into()),
        };
        assert!(failure.contains("actual later work failure"));
        assert!(failure.contains("precedes closure"));
        error(attempt.finish_verified_work(Ok(())), "precedes closure")?;
        let fresh = ArtifactAttempt::new();
        fresh.claim()?;
        fresh.ensure_claimed()?;
        error(fresh.finish_verified_work(Ok(())), "in-custody verification")?;
        Ok(())
    }

    #[test]
    fn actual_binding_generation_commits_full_names_and_original_presentation()
    -> Result<(), String> {
        let mut binding = super::super::complete_contract::tests::fixture_binding()?;
        binding.subject.changed_paths = vec!["binary.dat".into(), "deleted.rs".into()];
        let presentation = "diff --git a/deleted.rs b/deleted.rs\ndeleted file mode 100644\n";
        binding.presentation.bytes = presentation.len() as u64;
        binding.presentation.sha256 =
            super::super::complete_contract::sha256_bytes(presentation.as_bytes());
        binding.validate()?;
        let original = binding.generation_id()?;
        let mut altered = binding.clone();
        altered.subject.changed_paths.remove(1);
        assert_ne!(altered.generation_id()?, original);
        altered = binding.clone();
        altered.presentation.sha256 =
            super::super::complete_contract::sha256_bytes(b"different u3");
        assert_ne!(altered.generation_id()?, original);
        altered = binding.clone();
        altered.subject.changed_paths.swap(0, 1);
        error(altered.validate(), "path")?;
        assert_eq!(binding.generation_id()?, original);
        Ok(())
    }


    #[test]
    fn expected_binding_joins_actual_git_frozen_config_names_and_both_surface_inputs()
    -> Result<(), String> {
        use super::super::complete_request::{RequestedRoute, select_request_with_deadline};
        let base = std::env::temp_dir().canonicalize()
            .map_err(|error| format!("binding fixture root: {error}"))?;
        let root = base.join(format!(
            "ripr-whole-binding-{}-{}",
            std::process::id(), ARTIFACT_NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir_all(root.join(".ripr")).map_err(|error| error.to_string())?;
        let git = |args: &[&str]| crate::testing::fixture_git::fixture_git_ok(&root, args);
        git(&["-c", "init.templateDir=", "init", "--quiet", "-b", "whole"])?;
        git(&["config", "--local", "user.name", "RIPR binding fixture"])?;
        git(&["config", "--local", "user.email", "binding@example.invalid"])?;
        git(&["config", "--local", "commit.gpgsign", "false"])?;
        fs::write(root.join(POLICY_PATH), super::super::complete_contract::tests::fixture_policy_bytes())
            .map_err(|error| error.to_string())?;
        fs::write(root.join("ripr.toml"), b"").map_err(|error| error.to_string())?;
        fs::write(root.join("a.rs"), b"pub const VALUE: u8 = 1;\n")
            .map_err(|error| error.to_string())?;
        fs::write(root.join("deleted.rs"), b"pub const OLD: u8 = 1;\n")
            .map_err(|error| error.to_string())?;
        // Native Path ordering is intentionally distinct from String order.
        for path in ["a/x.rs", "a-y.rs", "a/sub/context.rs", "a-y/context.rs"] {
            let path = root.join(path);
            fs::create_dir_all(path.parent().ok_or("binding context path lacks parent")?)
                .map_err(|error| error.to_string())?;
            fs::write(path, b"pub fn unchanged_context() {}\n")
                .map_err(|error| error.to_string())?;
        }
        git(&["add", "--all"])?;
        git(&["commit", "--quiet", "-m", "binding base"])?;
        git(&["tag", "binding-base"])?;
        fs::write(root.join("a.rs"), b"pub const VALUE: u8 = 2;\n")
            .map_err(|error| error.to_string())?;
        fs::remove_file(root.join("deleted.rs")).map_err(|error| error.to_string())?;
        git(&["add", "--all"])?;
        git(&["commit", "--quiet", "-m", "binding head"])?;
        let root = fs::canonicalize(root).map_err(|error| error.to_string())?;
        let options = PrEvidenceOptions {
            root: ".".into(), base: "refs/tags/binding-base".into(), base_explicit: true,
            head: "HEAD".into(), check: false,
        };
        let deadline = Instant::now().checked_add(Duration::from_secs(60))
            .ok_or("binding fixture clock overflow")?;
        let mut request = match select_request_with_deadline(&root, "HEAD", deadline)? {
            RequestedRoute::Complete(request) => request,
            RequestedRoute::Ordinary => return Err("actual binding policy not selected".into()),
        };
        let subject = request.resolve_whole_subject(&options)?;
        let authority = crate::analysis::git_candidate_execution::prepare_named_tree(
            &root, subject.head_commit.as_str(), Some(Duration::from_secs(30)),
        ).map_err(|error| error.to_string())?.frozen_source_authority(&root)
            .map_err(|error| error.to_string())?;
        let profile = artifact_test_profile()?;
        frozen::with_context(Some(Arc::clone(&authority)), || -> Result<(), String> {
            let config = crate::config::config_for_captured_snapshot(
                &root, &root.join("ripr.toml"), authority.captured_configuration(),
            )?;
            let raw = crate::analysis::diff::load::load_canonical_pr_evidence_diff_bytes_bounded(
                &root, subject.base_commit.as_str(), subject.head_commit.as_str(), 128 * 1024,
            ).map_err(|error| error.to_string())?;
            let (_, coverage) = build_raw_coverage(&raw, RawCoverageLimits {
                file_limit: native_size(profile.file_limit)?,
                max_raw_bytes: 128 * 1024, max_records: native_size(profile.max_raw_records)?,
                max_ledger_bytes: 128 * 1024,
                max_retained_path_bytes: native_size(profile.max_retained_path_bytes)?,
                max_projection_bytes: native_size(profile.max_raw_projection_bytes)?,
            })?;
            let presentation = crate::analysis::load_pr_evidence_diff_range(
                &root, subject.base_commit.as_str(), subject.head_commit.as_str(),
            )?;
            let policy = capture_complete_rust_policy()?;
            for surface in [ProducerSurface::Installed, ProducerSurface::Xtask] {
                let mut check = CheckInput {
                    root: root.clone(), diff_file: Some(PathBuf::from("check.diff")),
                    mode: Mode::Draft, format: OutputFormat::Json,
                    include_unchanged_tests: surface == ProducerSurface::Installed,
                    base: if surface == ProducerSurface::Xtask { Some(options.base.clone()) } else { None },
                    ..CheckInput::default()
                };
                if surface == ProducerSurface::Xtask {
                    check.git_timeout = crate::cli::commands::git_timeout_from_env(
                        false, std::env::var("RIPR_GIT_TIMEOUT"),
                    )?.unwrap_or(Some(crate::app::default_cli_git_timeout()));
                }
                let full = FullConfiguration::capture(&config, &profile)?;
                let binding = build_expected_binding(BindingInputs {
                    subject: &subject, request: &request, authority: &authority,
                    coverage: &coverage, presentation: &presentation,
                    changed_paths: vec!["a.rs".into(), "deleted.rs".into()],
                    configuration: full, config: &config, check: &check, surface,
                    policy: &policy, build_identity: crate::build_identity::cache_identity(),
                    nonce: &"a".repeat(32), profile: &profile, deadline, retained: 0,
                })?;
                assert_eq!(binding.subject.head_tree, authority.head_tree().as_str());
                assert_eq!(binding.subject.changed_paths, ["a.rs", "deleted.rs"]);
                assert_eq!(binding.presentation.sha256,
                    super::super::complete_contract::sha256_bytes(presentation.as_bytes()));
                assert_eq!(binding.effective_options.check_input_base, check.base);
                assert_eq!(binding.full_configuration.source_text.as_deref(), Some(""));
                assert!(matches!(&binding.configuration, ConfigurationBinding::Present { .. }));
                for (expected, (path, file)) in binding.inventory.files.iter()
                    .zip(authority.inventory().files())
                {
                    assert_eq!(expected.path.as_str(), path.to_str().ok_or("fixture path not UTF-8")?);
                    assert_eq!(expected.sha256, digest_text(&file.sha256)?);
                    assert_eq!(expected.git_mode, file.mode.git_mode());
                }
                let nested = binding.inventory.files.iter()
                    .position(|file| file.path == "a/x.rs").ok_or("nested context absent")?;
                let hyphen = binding.inventory.files.iter()
                    .position(|file| file.path == "a-y.rs").ok_or("hyphen context absent")?;
                assert!(nested < hyphen, "binding lost native inventory owner ordering");
                assert!("a-y.rs" < "a/x.rs");
                binding.validate()?;
                binding_bytes(&binding, profile.max_binding_bytes)?;
                binding.generation_id()?;
            }
            Ok(())
        })?;
        drop(authority);
        fs::remove_dir_all(root).map_err(|error| format!("binding fixture cleanup: {error}"))
    }

    // There is deliberately no positive factory fixture here. A real factory
    // control must enter root's qualified hidden worker, actual finite native
    // startup/fresh held stage and real qualified Git/source/raw helpers.
}
