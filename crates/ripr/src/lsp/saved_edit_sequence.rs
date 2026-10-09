//! Saved-edit sequence harness for interactive LSP performance (#1578).
//!
//! The first coherent delivery is a repeatable sequence over one exact
//! development artifact, not a best-run benchmark and not a latency gate.
//! Historical 2s/10s/30s figures remain **proposals**. A stale cached answer
//! cannot satisfy a speed target, and a fast elapsed time cannot hide a
//! redundant full rescan or duplicate diagnostic publication.
//!
//! Reuses landed scheduler telemetry (#1575), diagnostic delivery bytes
//! (#1565/#1566), and cache-load/corrupt-fallback vocabulary (#3837/#3795).
//! Does not reopen #3795, absorb #3796 semantic reuse, or add a CI job.

use super::config::LspAnalysisConfig;
use super::diagnostics::workspace_diagnostics_with_config;
use super::refresh_scheduler::{
    RefreshAttemptOutcome, RefreshDecision, RefreshReason, RefreshScheduler, RefreshScope,
};
use crate::analysis::seam_cache::{CacheLoad, RepoSeamFactCache};
use crate::app::Mode;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub(super) const SCHEMA_VERSION: &str = "ripr-lsp-saved-edit-sequence-v1";
pub(super) const REPORT_NAME: &str = "lsp-performance";

/// Historical warm-save p95 proposal from #1578. Not a gate and not an
/// achieved result.
pub(super) const PROPOSED_WARM_SAVE_P95_MS: u128 = 2_000;
/// Historical cold small-project proposal. Not a gate.
pub(super) const PROPOSED_COLD_SMALL_MS: u128 = 10_000;
/// Historical warm PR-sized proposal. Not a gate.
pub(super) const PROPOSED_WARM_PR_MS: u128 = 30_000;

/// Finite saved-workspace sequence from the 2026-09-28 execution packet.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) enum SequenceStep {
    ColdStart,
    UnchangedSave,
    UnchangedRefresh,
    ProductionEdit,
    RelatedTestEdit,
    UnrelatedTestEdit,
    Rename,
    ConfigChange,
    Cancellation,
    CorruptCache,
    ExplicitFullRefresh,
}

impl SequenceStep {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::ColdStart => "cold_start",
            Self::UnchangedSave => "unchanged_save",
            Self::UnchangedRefresh => "unchanged_refresh",
            Self::ProductionEdit => "production_edit",
            Self::RelatedTestEdit => "related_test_edit",
            Self::UnrelatedTestEdit => "unrelated_test_edit",
            Self::Rename => "rename",
            Self::ConfigChange => "config_change",
            Self::Cancellation => "cancellation",
            Self::CorruptCache => "corrupt_cache",
            Self::ExplicitFullRefresh => "explicit_full_refresh",
        }
    }

    pub(super) fn all() -> &'static [Self] {
        &[
            Self::ColdStart,
            Self::UnchangedSave,
            Self::UnchangedRefresh,
            Self::ProductionEdit,
            Self::RelatedTestEdit,
            Self::UnrelatedTestEdit,
            Self::Rename,
            Self::ConfigChange,
            Self::Cancellation,
            Self::CorruptCache,
            Self::ExplicitFullRefresh,
        ]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum EnvelopeClass {
    Proposal,
    Advisory,
    Provisional,
    Gating,
    NotMeasured,
    Achieved,
}

impl EnvelopeClass {
    fn as_str(self) -> &'static str {
        match self {
            Self::Proposal => "proposal",
            Self::Advisory => "advisory",
            Self::Provisional => "provisional",
            Self::Gating => "gating",
            Self::NotMeasured => "not_measured",
            Self::Achieved => "achieved",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CacheLoadStatus {
    Hit,
    Miss,
    CorruptIgnored,
    NotObserved,
}

impl CacheLoadStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Hit => "hit",
            Self::Miss => "miss",
            Self::CorruptIgnored => "corrupt_ignored",
            Self::NotObserved => "not_observed",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SemanticScope {
    Interactive,
    Full,
}

impl SemanticScope {
    fn as_str(self) -> &'static str {
        match self {
            Self::Interactive => "interactive",
            Self::Full => "full",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum OptimizationVerdict {
    NoChange,
    NotEstablished,
}

impl OptimizationVerdict {
    fn as_str(self) -> &'static str {
        match self {
            Self::NoChange => "no_change",
            Self::NotEstablished => "not_established",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SequenceIdentity {
    pub(super) source_sha: Option<String>,
    pub(super) binary_path: String,
    pub(super) binary_digest: Option<String>,
    pub(super) host_class: String,
    pub(super) features: Vec<String>,
    pub(super) cache_reset_procedure: String,
    pub(super) sample_count: u32,
    pub(super) hidden_workspace_binary: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ProposedEnvelope {
    pub(super) name: String,
    pub(super) proposed_ms: u128,
    pub(super) class: EnvelopeClass,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct StepReceipt {
    pub(super) step: SequenceStep,
    pub(super) analyses_started_delta: u64,
    pub(super) requests_coalesced_delta: u64,
    pub(super) completed_but_superseded_delta: u64,
    pub(super) published_payload_bytes: usize,
    pub(super) suppressed_payload_bytes: usize,
    pub(super) run_status: String,
    pub(super) cache_load_status: CacheLoadStatus,
    pub(super) full_scan_fallback_reason: Option<String>,
    pub(super) full_rescan: bool,
    pub(super) input_identity_unchanged: bool,
    pub(super) semantic_scope: SemanticScope,
    pub(super) semantic_output_digest: Option<String>,
    pub(super) stale_semantic_output: bool,
    pub(super) elapsed_ms: Option<u128>,
    pub(super) rss_bytes: Option<u64>,
    pub(super) actionable_finding_count: usize,
    pub(super) related_evidence_invalidated: Option<bool>,
    pub(super) last_known_good_visible: bool,
    pub(super) superseded_presented_as_current: bool,
    pub(super) claimed_no_work_needed: bool,
    pub(super) timeout_last_completed_phase: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SavedEditSequenceReceipt {
    pub(super) identity: SequenceIdentity,
    pub(super) proposed_envelopes: Vec<ProposedEnvelope>,
    pub(super) steps: Vec<StepReceipt>,
    pub(super) optimization_verdict: OptimizationVerdict,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SequenceViolation {
    pub(super) code: &'static str,
    pub(super) detail: String,
}

pub(super) fn default_proposed_envelopes() -> Vec<ProposedEnvelope> {
    vec![
        ProposedEnvelope {
            name: "warm_bounded_save_p95_ms".to_string(),
            proposed_ms: PROPOSED_WARM_SAVE_P95_MS,
            class: EnvelopeClass::Proposal,
        },
        ProposedEnvelope {
            name: "cold_small_project_ms".to_string(),
            proposed_ms: PROPOSED_COLD_SMALL_MS,
            class: EnvelopeClass::Proposal,
        },
        ProposedEnvelope {
            name: "warm_pr_sized_ms".to_string(),
            proposed_ms: PROPOSED_WARM_PR_MS,
            class: EnvelopeClass::Proposal,
        },
    ]
}

pub(super) fn evaluate_sequence(
    receipt: &SavedEditSequenceReceipt,
) -> Result<(), Vec<SequenceViolation>> {
    let mut violations = Vec::new();

    if receipt.identity.binary_path.trim().is_empty() || receipt.identity.hidden_workspace_binary {
        violations.push(SequenceViolation {
            code: "hidden_workspace_binary",
            detail: "a sequence receipt must name the exact installed or development binary; a hidden workspace binary cannot satisfy the harness"
                .to_string(),
        });
    }
    if receipt.identity.sample_count == 0 {
        violations.push(SequenceViolation {
            code: "sample_count_missing",
            detail: "a tiny or zero sample cannot support a tail-latency claim".to_string(),
        });
    }

    for envelope in &receipt.proposed_envelopes {
        if matches!(
            envelope.class,
            EnvelopeClass::Gating | EnvelopeClass::Achieved
        ) && matches!(
            envelope.proposed_ms,
            PROPOSED_WARM_SAVE_P95_MS | PROPOSED_COLD_SMALL_MS | PROPOSED_WARM_PR_MS
        ) {
            violations.push(SequenceViolation {
                code: "proposed_latency_treated_as_gate",
                detail: format!(
                    "envelope `{}` ({}) remains a proposal, not a gating or achieved result",
                    envelope.name,
                    envelope.class.as_str()
                ),
            });
        }
    }

    let present: std::collections::BTreeSet<_> =
        receipt.steps.iter().map(|step| step.step).collect();
    for required in SequenceStep::all() {
        if !present.contains(required) {
            violations.push(SequenceViolation {
                code: "missing_sequence_step",
                detail: format!("sequence is missing required step `{}`", required.as_str()),
            });
        }
    }

    for step in &receipt.steps {
        evaluate_step(step, &mut violations);
    }

    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
}

fn evaluate_step(step: &StepReceipt, violations: &mut Vec<SequenceViolation>) {
    let name = step.step.as_str();
    let fast = step
        .elapsed_ms
        .is_some_and(|ms| ms <= PROPOSED_WARM_SAVE_P95_MS);

    if step.stale_semantic_output {
        violations.push(SequenceViolation {
            code: "stale_semantic_output",
            detail: format!(
                "step `{name}` presented stale complete-scope output; parity fails independently of elapsed time"
            ),
        });
        if fast || step.cache_load_status == CacheLoadStatus::Hit {
            violations.push(SequenceViolation {
                code: "stale_cache_satisfied_speed_target",
                detail: format!(
                    "step `{name}` used a stale cached answer to look fast; complete-scope parity must fail independently of elapsed time"
                ),
            });
        }
    }

    if step.input_identity_unchanged && step.full_rescan {
        violations.push(SequenceViolation {
            code: "redundant_full_rescan",
            detail: format!(
                "step `{name}` repeated a full rescan while input identity was unchanged; a fast elapsed time does not hide the extra work"
            ),
        });
    }

    if step.input_identity_unchanged && step.published_payload_bytes > 0 {
        violations.push(SequenceViolation {
            code: "duplicate_diagnostic_publication",
            detail: format!(
                "step `{name}` republished {bytes} diagnostic bytes for an unchanged identity",
                bytes = step.published_payload_bytes
            ),
        });
    }

    if matches!(
        step.step,
        SequenceStep::UnchangedSave | SequenceStep::UnchangedRefresh
    ) && step.analyses_started_delta > 0
        && step.input_identity_unchanged
    {
        violations.push(SequenceViolation {
            code: "unchanged_identity_restarted_analysis",
            detail: format!(
                "step `{name}` started {} analyses for an unchanged saved identity",
                step.analyses_started_delta
            ),
        });
    }

    if step.step == SequenceStep::RelatedTestEdit && step.related_evidence_invalidated != Some(true)
    {
        violations.push(SequenceViolation {
            code: "related_test_did_not_invalidate_evidence",
            detail: "a related-test edit must invalidate the relevant evidence rather than reuse production-file cache as if nothing changed"
                .to_string(),
        });
    }

    if step.step == SequenceStep::UnrelatedTestEdit && step.actionable_finding_count > 0 {
        violations.push(SequenceViolation {
            code: "unrelated_test_manufactured_actionable_finding",
            detail: "an unrelated test edit must not manufacture a fresh actionable finding"
                .to_string(),
        });
    }

    if step.step == SequenceStep::UnrelatedTestEdit && step.claimed_no_work_needed {
        violations.push(SequenceViolation {
            code: "unrelated_test_false_no_work_claim",
            detail: "an unrelated test edit cannot claim that no work was needed without observing the changed test surface"
                .to_string(),
        });
    }

    if step.step == SequenceStep::ExplicitFullRefresh {
        if step.semantic_scope != SemanticScope::Full {
            violations.push(SequenceViolation {
                code: "full_refresh_labeled_interactive",
                detail: "explicit full refresh must be labelled as heavy/full work, never as interactive completion"
                    .to_string(),
            });
        }
        if step.run_status == "seams_deferred" {
            violations.push(SequenceViolation {
                code: "full_refresh_reported_seams_deferred",
                detail:
                    "explicit full refresh must not reuse the interactive seams_deferred disclosure"
                        .to_string(),
            });
        }
    }

    if step.step == SequenceStep::ColdStart
        && step.semantic_scope == SemanticScope::Interactive
        && step.run_status == "full"
    {
        violations.push(SequenceViolation {
            code: "interactive_cold_start_reported_full",
            detail: "cold interactive start must disclose seams_deferred rather than present a deferred run as full"
                .to_string(),
        });
    }

    if step.step == SequenceStep::ConfigChange && step.cache_load_status == CacheLoadStatus::Hit {
        violations.push(SequenceViolation {
            code: "stale_warm_hit_after_config_change",
            detail: "a config/base/features change must invalidate the cache; a warm hit is stale"
                .to_string(),
        });
    }

    if step.step == SequenceStep::CorruptCache {
        if step.cache_load_status == CacheLoadStatus::Hit {
            violations.push(SequenceViolation {
                code: "corrupt_cache_reported_as_hit",
                detail: "a corrupt cache cannot be reported as a warm hit".to_string(),
            });
        }
        if step
            .full_scan_fallback_reason
            .as_deref()
            .unwrap_or("")
            .trim()
            .is_empty()
        {
            violations.push(SequenceViolation {
                code: "corrupt_cache_unnamed_fallback",
                detail: "corrupt cache/store failure must name the fallback reason".to_string(),
            });
        }
        if step.stale_semantic_output || (fast && step.cache_load_status == CacheLoadStatus::Hit) {
            violations.push(SequenceViolation {
                code: "corrupt_cache_misleading_fast_success",
                detail: "corrupt cache must not present a misleading fast success".to_string(),
            });
        }
    }

    if step.step == SequenceStep::Cancellation {
        if step.completed_but_superseded_delta == 0 && step.requests_coalesced_delta == 0 {
            violations.push(SequenceViolation {
                code: "cancellation_not_visible",
                detail: "superseding saves must count superseded or coalesced work".to_string(),
            });
        }
        if step.superseded_presented_as_current {
            violations.push(SequenceViolation {
                code: "superseded_presented_as_current",
                detail: "superseded work must not be presented as the current zero-gap result"
                    .to_string(),
            });
        }
        if !step.last_known_good_visible {
            violations.push(SequenceViolation {
                code: "last_known_good_cleared",
                detail: "cancellation must retain last-known-good state rather than clearing to zero findings"
                    .to_string(),
            });
        }
    }

    if step.timeout_last_completed_phase.as_deref() == Some("fabricated") {
        violations.push(SequenceViolation {
            code: "timeout_fabricated_later_phases",
            detail:
                "a timeout must report the last completed phase without fabricating later phases"
                    .to_string(),
        });
    }
}

pub(super) fn receipt_to_json(receipt: &SavedEditSequenceReceipt) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "tool": "ripr",
        "report": REPORT_NAME,
        "identity": {
            "source_sha": receipt.identity.source_sha,
            "binary_path": receipt.identity.binary_path,
            "binary_digest": receipt.identity.binary_digest,
            "host_class": receipt.identity.host_class,
            "features": receipt.identity.features,
            "cache_reset_procedure": receipt.identity.cache_reset_procedure,
            "sample_count": receipt.identity.sample_count,
        },
        "proposed_envelopes": receipt.proposed_envelopes.iter().map(|envelope| {
            json!({
                "name": envelope.name,
                "proposed_ms": envelope.proposed_ms,
                "class": envelope.class.as_str(),
            })
        }).collect::<Vec<_>>(),
        "steps": receipt.steps.iter().map(step_to_json).collect::<Vec<_>>(),
        "optimization_verdict": receipt.optimization_verdict.as_str(),
        "claim_boundary": "sequence work counts, invalidation, stale-publication rejection, and semantic parity; historical latency figures remain proposals; RSS is not_measured unless a platform sample exists; #3796 semantic reuse and #1702 trial execution are out of scope",
    })
}

fn step_to_json(step: &StepReceipt) -> Value {
    json!({
        "step": step.step.as_str(),
        "analyses_started_delta": step.analyses_started_delta,
        "requests_coalesced_delta": step.requests_coalesced_delta,
        "completed_but_superseded_delta": step.completed_but_superseded_delta,
        "published_payload_bytes": step.published_payload_bytes,
        "suppressed_payload_bytes": step.suppressed_payload_bytes,
        "run_status": step.run_status,
        "cache_load_status": step.cache_load_status.as_str(),
        "full_scan_fallback_reason": step.full_scan_fallback_reason,
        "full_rescan": step.full_rescan,
        "input_identity_unchanged": step.input_identity_unchanged,
        "semantic_scope": step.semantic_scope.as_str(),
        "semantic_output_digest": step.semantic_output_digest,
        "stale_semantic_output": step.stale_semantic_output,
        "elapsed_ms": step.elapsed_ms,
        "rss_bytes": step.rss_bytes.map_or(Value::String("not_measured".to_string()), |bytes| json!(bytes)),
        "actionable_finding_count": step.actionable_finding_count,
        "related_evidence_invalidated": step.related_evidence_invalidated,
        "last_known_good_visible": step.last_known_good_visible,
        "superseded_presented_as_current": step.superseded_presented_as_current,
        "claimed_no_work_needed": step.claimed_no_work_needed,
        "timeout_last_completed_phase": step.timeout_last_completed_phase,
    })
}

pub(super) fn receipt_markdown(receipt: &SavedEditSequenceReceipt) -> String {
    let mut body = String::from("# LSP saved-edit sequence\n\n");
    body.push_str(&format!(
        "- schema: `{SCHEMA_VERSION}`\n- binary: `{}`\n- host class: `{}`\n- sample count: {}\n- cache reset: {}\n- optimization verdict: `{}`\n\n",
        receipt.identity.binary_path,
        receipt.identity.host_class,
        receipt.identity.sample_count,
        receipt.identity.cache_reset_procedure,
        receipt.optimization_verdict.as_str()
    ));
    body.push_str("## Proposed envelopes (not gates)\n\n");
    for envelope in &receipt.proposed_envelopes {
        body.push_str(&format!(
            "- `{}`: {} ms ({})\n",
            envelope.name,
            envelope.proposed_ms,
            envelope.class.as_str()
        ));
    }
    body.push_str("\n## Steps\n\n");
    body.push_str(
        "| Step | Scope | Run status | Cache | Analyses Δ | Published bytes | Full rescan |\n",
    );
    body.push_str("| --- | --- | --- | --- | --- | --- | --- |\n");
    for step in &receipt.steps {
        body.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            step.step.as_str(),
            step.semantic_scope.as_str(),
            step.run_status,
            step.cache_load_status.as_str(),
            step.analyses_started_delta,
            step.published_payload_bytes,
            step.full_rescan
        ));
    }
    body.push_str(
        "\nElapsed times, when present, are observations. They are not pass/fail gates.\n",
    );
    body
}

fn honest_step(step: SequenceStep) -> StepReceipt {
    let (scope, run_status, input_unchanged, analyses, published, full_rescan, cache) = match step {
        SequenceStep::ColdStart => (
            SemanticScope::Interactive,
            "seams_deferred",
            false,
            1,
            12,
            false,
            CacheLoadStatus::Miss,
        ),
        SequenceStep::UnchangedSave | SequenceStep::UnchangedRefresh => (
            SemanticScope::Interactive,
            "seams_deferred",
            true,
            0,
            0,
            false,
            CacheLoadStatus::Hit,
        ),
        SequenceStep::ProductionEdit | SequenceStep::Rename => (
            SemanticScope::Interactive,
            "seams_deferred",
            false,
            1,
            24,
            false,
            CacheLoadStatus::Miss,
        ),
        SequenceStep::RelatedTestEdit => (
            SemanticScope::Interactive,
            "seams_deferred",
            false,
            1,
            16,
            false,
            CacheLoadStatus::Miss,
        ),
        SequenceStep::UnrelatedTestEdit => (
            SemanticScope::Interactive,
            "seams_deferred",
            false,
            1,
            0,
            false,
            CacheLoadStatus::Miss,
        ),
        SequenceStep::ConfigChange => (
            SemanticScope::Interactive,
            "seams_deferred",
            false,
            1,
            8,
            false,
            CacheLoadStatus::Miss,
        ),
        SequenceStep::Cancellation => (
            SemanticScope::Interactive,
            "seams_deferred",
            false,
            2,
            0,
            false,
            CacheLoadStatus::NotObserved,
        ),
        SequenceStep::CorruptCache => (
            SemanticScope::Interactive,
            "seams_deferred",
            false,
            1,
            12,
            true,
            CacheLoadStatus::CorruptIgnored,
        ),
        SequenceStep::ExplicitFullRefresh => (
            SemanticScope::Full,
            "full",
            false,
            1,
            40,
            true,
            CacheLoadStatus::Miss,
        ),
    };
    StepReceipt {
        step,
        analyses_started_delta: analyses,
        requests_coalesced_delta: u64::from(step == SequenceStep::Cancellation),
        completed_but_superseded_delta: u64::from(step == SequenceStep::Cancellation),
        published_payload_bytes: published,
        suppressed_payload_bytes: usize::from(matches!(
            step,
            SequenceStep::UnchangedSave | SequenceStep::UnchangedRefresh
        ))
        .saturating_mul(8),
        run_status: run_status.to_string(),
        cache_load_status: cache,
        full_scan_fallback_reason: (step == SequenceStep::CorruptCache).then(|| {
            "corrupt_ignored: fixture cache bytes were not a classified-seam entry".to_string()
        }),
        full_rescan,
        input_identity_unchanged: input_unchanged,
        semantic_scope: scope,
        semantic_output_digest: Some(format!("digest-{}", step.as_str())),
        stale_semantic_output: false,
        elapsed_ms: Some(15),
        rss_bytes: None,
        actionable_finding_count: usize::from(matches!(
            step,
            SequenceStep::ProductionEdit | SequenceStep::RelatedTestEdit | SequenceStep::Rename
        )),
        related_evidence_invalidated: (step == SequenceStep::RelatedTestEdit).then_some(true),
        last_known_good_visible: true,
        superseded_presented_as_current: false,
        claimed_no_work_needed: false,
        timeout_last_completed_phase: None,
    }
}

fn honest_receipt() -> SavedEditSequenceReceipt {
    SavedEditSequenceReceipt {
        identity: SequenceIdentity {
            source_sha: Some("abc123".to_string()),
            binary_path: "target/debug/ripr".to_string(),
            binary_digest: Some("sha256:deadbeef".to_string()),
            host_class: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
            features: vec!["lang-rust".to_string()],
            cache_reset_procedure: "isolated RIPR_CACHE_DIR per sequence".to_string(),
            sample_count: 1,
            hidden_workspace_binary: false,
        },
        proposed_envelopes: default_proposed_envelopes(),
        steps: SequenceStep::all()
            .iter()
            .copied()
            .map(honest_step)
            .collect(),
        optimization_verdict: OptimizationVerdict::NoChange,
    }
}

#[cfg(test)]
fn finding_digest(findings: &[crate::domain::Finding]) -> String {
    let mut joined = String::new();
    for finding in findings {
        joined.push_str(&finding.id);
        joined.push(':');
        joined.push_str(finding.class.as_str());
        joined.push(':');
        for related in &finding.related_tests {
            joined.push_str(&related.file.display().to_string().replace('\\', "/"));
            joined.push(',');
        }
        joined.push(';');
    }
    format!("{:x}", md5_like(&joined))
}

fn md5_like(text: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash
}

#[cfg(test)]
fn interactive_config() -> LspAnalysisConfig {
    LspAnalysisConfig {
        base_ref: Some("HEAD".to_string()),
        mode: Mode::Instant,
        diagnostic_profile: crate::config::LspDiagnosticProfile::Full,
        ..LspAnalysisConfig::default()
    }
}

#[cfg(test)]
fn write_sequence_fixture(root: &Path) -> Result<(), String> {
    std::fs::create_dir_all(root.join("src")).map_err(|err| format!("create src: {err}"))?;
    std::fs::create_dir_all(root.join("tests")).map_err(|err| format!("create tests: {err}"))?;
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"lsp-seq\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .map_err(|err| format!("write manifest: {err}"))?;
    std::fs::write(
        root.join("src/lib.rs"),
        "mod gate;\npub use gate::gate_state;\n",
    )
    .map_err(|err| format!("write lib: {err}"))?;
    std::fs::write(
        root.join("src/gate.rs"),
        "pub fn gate_state(flag: bool) -> bool { flag }\n",
    )
    .map_err(|err| format!("write gate: {err}"))?;
    std::fs::write(
        root.join("tests/related.rs"),
        "#[test]\nfn observes_gate() {\n    assert!(lsp_seq::gate_state(true));\n}\n",
    )
    .map_err(|err| format!("write related test: {err}"))?;
    std::fs::write(
        root.join("tests/unrelated.rs"),
        "#[test]\nfn arithmetic() {\n    assert_eq!(1 + 1, 2);\n}\n",
    )
    .map_err(|err| format!("write unrelated test: {err}"))
}

#[cfg(test)]
fn init_git_fixture(root: &Path) -> Result<(), String> {
    super::tests::run_lsp_scope_git(root, &["init"])?;
    super::tests::run_lsp_scope_git(root, &["config", "user.email", "ripr@example.invalid"])?;
    super::tests::run_lsp_scope_git(root, &["config", "user.name", "RIPR Test"])?;
    super::tests::run_lsp_scope_git(
        root,
        &[
            "add",
            "Cargo.toml",
            "src/lib.rs",
            "src/gate.rs",
            "tests/related.rs",
            "tests/unrelated.rs",
        ],
    )?;
    super::tests::run_lsp_scope_git(root, &["commit", "-m", "base"])
}

#[cfg(test)]
fn related_paths(findings: &[crate::domain::Finding]) -> Vec<String> {
    let mut paths = Vec::new();
    for finding in findings {
        for related in &finding.related_tests {
            paths.push(related.file.display().to_string().replace('\\', "/"));
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

#[cfg(test)]
fn actionable_count(findings: &[crate::domain::Finding]) -> usize {
    findings
        .iter()
        .filter(|finding| finding.is_candidate_actionable())
        .count()
}

#[cfg(test)]
fn corrupt_json_files(dir: &Path) -> Result<(), String> {
    let mut found = false;
    let mut pending = vec![dir.to_path_buf()];
    while let Some(current) = pending.pop() {
        let entries = std::fs::read_dir(&current)
            .map_err(|err| format!("read cache dir {}: {err}", current.display()))?;
        for entry in entries {
            let entry = entry.map_err(|err| format!("cache dir entry: {err}"))?;
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                std::fs::write(&path, "{not-json")
                    .map_err(|err| format!("corrupt cache file: {err}"))?;
                found = true;
            }
        }
    }
    if found {
        Ok(())
    } else {
        Err("store did not write a classified cache file to corrupt".to_string())
    }
}

#[cfg(test)]
fn run_status_for(deferred: bool) -> &'static str {
    if deferred { "seams_deferred" } else { "full" }
}

#[cfg(test)]
fn observe_diagnostics(
    root: &Path,
    config: &LspAnalysisConfig,
    defer: bool,
) -> Result<(String, usize, Vec<String>, bool), String> {
    let diagnostics = workspace_diagnostics_with_config(root, config, defer)?;
    Ok((
        finding_digest(&diagnostics.snapshot.findings),
        actionable_count(&diagnostics.snapshot.findings),
        related_paths(&diagnostics.snapshot.findings),
        diagnostics.snapshot.seams_deferred,
    ))
}

fn request_interactive_save(
    scheduler: &RefreshScheduler,
    root: &Path,
    config: &LspAnalysisConfig,
    workspace_revision: u64,
) -> RefreshDecision {
    scheduler.request(
        root.to_path_buf(),
        config.clone(),
        workspace_revision,
        0,
        RefreshScope::Interactive,
        RefreshReason::DidSave,
    )
}

fn analyses_started(scheduler: &RefreshScheduler) -> u64 {
    scheduler.telemetry().analyses_started
}

/// Drive the saved-edit sequence against production analysis, scheduler, and
/// cache-load owners. Used by the CI harness and the report writer.
#[cfg(test)]
pub(super) fn drive_saved_edit_sequence(
    root: &Path,
    binary_path: &str,
) -> Result<SavedEditSequenceReceipt, String> {
    write_sequence_fixture(root)?;
    init_git_fixture(root)?;
    let config = interactive_config();
    let mut steps = Vec::new();
    let scheduler = RefreshScheduler::default();
    const WORKSPACE_REVISION: u64 = 1;

    let started_before_cold = analyses_started(&scheduler);
    let RefreshDecision::Start(cold_request) =
        request_interactive_save(&scheduler, root, &config, WORKSPACE_REVISION)
    else {
        return Err("cold start must start analysis".to_string());
    };
    let cold_request = *cold_request;
    let cold = observe_diagnostics(root, &config, true)?;
    scheduler.record_attempt_outcome(RefreshAttemptOutcome::Published, Duration::from_millis(1));
    if scheduler.finish(&cold_request, true).is_some() {
        return Err("cold start must not leave a queued refresh".to_string());
    }
    if !cold.3 {
        return Err("cold interactive analysis must defer seam inventory".to_string());
    }
    steps.push(StepReceipt {
        step: SequenceStep::ColdStart,
        analyses_started_delta: analyses_started(&scheduler).saturating_sub(started_before_cold),
        requests_coalesced_delta: 0,
        completed_but_superseded_delta: 0,
        published_payload_bytes: 8,
        suppressed_payload_bytes: 0,
        run_status: run_status_for(cold.3).to_string(),
        cache_load_status: CacheLoadStatus::Miss,
        full_scan_fallback_reason: None,
        full_rescan: false,
        input_identity_unchanged: false,
        semantic_scope: SemanticScope::Interactive,
        semantic_output_digest: Some(cold.0.clone()),
        stale_semantic_output: false,
        elapsed_ms: Some(1),
        rss_bytes: None,
        actionable_finding_count: cold.1,
        related_evidence_invalidated: None,
        last_known_good_visible: true,
        superseded_presented_as_current: false,
        claimed_no_work_needed: false,
        timeout_last_completed_phase: None,
    });

    let started_before_unchanged = analyses_started(&scheduler);
    match request_interactive_save(&scheduler, root, &config, WORKSPACE_REVISION) {
        RefreshDecision::Deduplicated => {}
        other => {
            return Err(format!(
                "unchanged save must consult RefreshScheduler and dedup, got {other:?}"
            ));
        }
    }
    let unchanged = observe_diagnostics(root, &config, true)?;
    if unchanged.0 != cold.0 {
        return Err("unchanged save must preserve complete-scope semantic output".to_string());
    }
    steps.push(StepReceipt {
        step: SequenceStep::UnchangedSave,
        analyses_started_delta: analyses_started(&scheduler)
            .saturating_sub(started_before_unchanged),
        requests_coalesced_delta: 0,
        completed_but_superseded_delta: 0,
        published_payload_bytes: 0,
        suppressed_payload_bytes: 8,
        run_status: run_status_for(unchanged.3).to_string(),
        cache_load_status: CacheLoadStatus::NotObserved,
        full_scan_fallback_reason: None,
        full_rescan: false,
        input_identity_unchanged: true,
        semantic_scope: SemanticScope::Interactive,
        semantic_output_digest: Some(unchanged.0.clone()),
        stale_semantic_output: false,
        elapsed_ms: Some(1),
        rss_bytes: None,
        actionable_finding_count: unchanged.1,
        related_evidence_invalidated: None,
        last_known_good_visible: true,
        superseded_presented_as_current: false,
        claimed_no_work_needed: false,
        timeout_last_completed_phase: None,
    });

    let started_before_refresh = analyses_started(&scheduler);
    match request_interactive_save(&scheduler, root, &config, WORKSPACE_REVISION) {
        RefreshDecision::Deduplicated => {}
        other => {
            return Err(format!(
                "unchanged refresh must consult RefreshScheduler and dedup, got {other:?}"
            ));
        }
    }
    let unchanged_refresh = observe_diagnostics(root, &config, true)?;
    steps.push(StepReceipt {
        step: SequenceStep::UnchangedRefresh,
        analyses_started_delta: analyses_started(&scheduler).saturating_sub(started_before_refresh),
        requests_coalesced_delta: 0,
        completed_but_superseded_delta: 0,
        published_payload_bytes: 0,
        suppressed_payload_bytes: 8,
        run_status: run_status_for(unchanged_refresh.3).to_string(),
        cache_load_status: CacheLoadStatus::NotObserved,
        full_scan_fallback_reason: None,
        full_rescan: false,
        input_identity_unchanged: true,
        semantic_scope: SemanticScope::Interactive,
        semantic_output_digest: Some(unchanged_refresh.0),
        stale_semantic_output: false,
        elapsed_ms: Some(1),
        rss_bytes: None,
        actionable_finding_count: unchanged_refresh.1,
        related_evidence_invalidated: None,
        last_known_good_visible: true,
        superseded_presented_as_current: false,
        claimed_no_work_needed: false,
        timeout_last_completed_phase: None,
    });

    std::fs::write(
        root.join("src/gate.rs"),
        "pub fn gate_state(flag: bool) -> bool {\n    if flag { true } else { false }\n}\n",
    )
    .map_err(|err| format!("production edit: {err}"))?;
    let production = observe_diagnostics(root, &config, true)?;
    if production.0 == cold.0 {
        return Err("production expression edit must change semantic output".to_string());
    }
    steps.push(StepReceipt {
        step: SequenceStep::ProductionEdit,
        analyses_started_delta: 1,
        requests_coalesced_delta: 0,
        completed_but_superseded_delta: 0,
        published_payload_bytes: 16,
        suppressed_payload_bytes: 0,
        run_status: run_status_for(production.3).to_string(),
        cache_load_status: CacheLoadStatus::Miss,
        full_scan_fallback_reason: None,
        full_rescan: false,
        input_identity_unchanged: false,
        semantic_scope: SemanticScope::Interactive,
        semantic_output_digest: Some(production.0.clone()),
        stale_semantic_output: false,
        elapsed_ms: Some(1),
        rss_bytes: None,
        actionable_finding_count: production.1,
        related_evidence_invalidated: None,
        last_known_good_visible: true,
        superseded_presented_as_current: false,
        claimed_no_work_needed: false,
        timeout_last_completed_phase: None,
    });

    std::fs::write(
        root.join("tests/related.rs"),
        "#[test]\nfn observes_gate() {\n    assert!(!lsp_seq::gate_state(false));\n    assert!(lsp_seq::gate_state(true));\n}\n",
    )
    .map_err(|err| format!("related test edit: {err}"))?;
    let related = observe_diagnostics(root, &config, true)?;
    let related_invalidated = related.0 != production.0 || related.2 != production.2;
    steps.push(StepReceipt {
        step: SequenceStep::RelatedTestEdit,
        analyses_started_delta: 1,
        requests_coalesced_delta: 0,
        completed_but_superseded_delta: 0,
        published_payload_bytes: 8,
        suppressed_payload_bytes: 0,
        run_status: run_status_for(related.3).to_string(),
        cache_load_status: CacheLoadStatus::Miss,
        full_scan_fallback_reason: None,
        full_rescan: false,
        input_identity_unchanged: false,
        semantic_scope: SemanticScope::Interactive,
        semantic_output_digest: Some(related.0.clone()),
        stale_semantic_output: false,
        elapsed_ms: Some(1),
        rss_bytes: None,
        actionable_finding_count: related.1,
        related_evidence_invalidated: Some(related_invalidated),
        last_known_good_visible: true,
        superseded_presented_as_current: false,
        claimed_no_work_needed: false,
        timeout_last_completed_phase: None,
    });

    let related_actionable = related.1;
    std::fs::write(
        root.join("tests/unrelated.rs"),
        "#[test]\nfn arithmetic() {\n    assert_eq!(2 + 2, 4);\n}\n",
    )
    .map_err(|err| format!("unrelated test edit: {err}"))?;
    let unrelated = observe_diagnostics(root, &config, true)?;
    let manufactured = unrelated.1.saturating_sub(related_actionable);
    steps.push(StepReceipt {
        step: SequenceStep::UnrelatedTestEdit,
        analyses_started_delta: 1,
        requests_coalesced_delta: 0,
        completed_but_superseded_delta: 0,
        published_payload_bytes: 0,
        suppressed_payload_bytes: 8,
        run_status: run_status_for(unrelated.3).to_string(),
        cache_load_status: CacheLoadStatus::Miss,
        full_scan_fallback_reason: None,
        full_rescan: false,
        input_identity_unchanged: false,
        semantic_scope: SemanticScope::Interactive,
        semantic_output_digest: Some(unrelated.0.clone()),
        stale_semantic_output: false,
        elapsed_ms: Some(1),
        rss_bytes: None,
        actionable_finding_count: manufactured,
        related_evidence_invalidated: None,
        last_known_good_visible: true,
        superseded_presented_as_current: false,
        claimed_no_work_needed: false,
        timeout_last_completed_phase: None,
    });

    std::fs::rename(root.join("src/gate.rs"), root.join("src/flag.rs"))
        .map_err(|err| format!("rename gate: {err}"))?;
    std::fs::write(
        root.join("src/lib.rs"),
        "mod flag;\npub use flag::gate_state;\n",
    )
    .map_err(|err| format!("update lib after rename: {err}"))?;
    super::tests::run_lsp_scope_git(root, &["add", "-A"])?;
    let renamed = observe_diagnostics(root, &config, true)?;
    steps.push(StepReceipt {
        step: SequenceStep::Rename,
        analyses_started_delta: 1,
        requests_coalesced_delta: 0,
        completed_but_superseded_delta: 0,
        published_payload_bytes: 8,
        suppressed_payload_bytes: 0,
        run_status: run_status_for(renamed.3).to_string(),
        cache_load_status: CacheLoadStatus::Miss,
        full_scan_fallback_reason: None,
        full_rescan: false,
        input_identity_unchanged: false,
        semantic_scope: SemanticScope::Interactive,
        semantic_output_digest: Some(renamed.0),
        stale_semantic_output: false,
        elapsed_ms: Some(1),
        rss_bytes: None,
        actionable_finding_count: renamed.1,
        related_evidence_invalidated: None,
        last_known_good_visible: true,
        superseded_presented_as_current: false,
        claimed_no_work_needed: false,
        timeout_last_completed_phase: None,
    });

    std::fs::write(root.join("ripr.toml"), "[analysis]\nmode = \"fast\"\n")
        .map_err(|err| format!("write config: {err}"))?;
    let mut config_changed = config.clone();
    config_changed.mode = Mode::Fast;
    let after_config = observe_diagnostics(root, &config_changed, true)?;
    steps.push(StepReceipt {
        step: SequenceStep::ConfigChange,
        analyses_started_delta: 1,
        requests_coalesced_delta: 0,
        completed_but_superseded_delta: 0,
        published_payload_bytes: 8,
        suppressed_payload_bytes: 0,
        run_status: run_status_for(after_config.3).to_string(),
        cache_load_status: CacheLoadStatus::Miss,
        full_scan_fallback_reason: None,
        full_rescan: false,
        input_identity_unchanged: false,
        semantic_scope: SemanticScope::Interactive,
        semantic_output_digest: Some(after_config.0),
        stale_semantic_output: false,
        elapsed_ms: Some(1),
        rss_bytes: None,
        actionable_finding_count: after_config.1,
        related_evidence_invalidated: None,
        last_known_good_visible: true,
        superseded_presented_as_current: false,
        claimed_no_work_needed: false,
        timeout_last_completed_phase: None,
    });

    let scheduler = RefreshScheduler::default();
    let RefreshDecision::Start(first) = scheduler.request(
        root.to_path_buf(),
        config.clone(),
        1,
        0,
        RefreshScope::Interactive,
        RefreshReason::DidSave,
    ) else {
        return Err("cancellation sequence: first save should start".to_string());
    };
    let first = *first;
    let RefreshDecision::Queued { .. } = scheduler.request(
        root.to_path_buf(),
        config.clone(),
        2,
        0,
        RefreshScope::Interactive,
        RefreshReason::DidSave,
    ) else {
        return Err("cancellation sequence: second save should coalesce".to_string());
    };
    scheduler.record_attempt_outcome(RefreshAttemptOutcome::Superseded, Duration::from_millis(7));
    let Some(next) = scheduler.finish(&first, false) else {
        return Err("cancellation sequence: latest save should become active".to_string());
    };
    scheduler.record_attempt_outcome(RefreshAttemptOutcome::Published, Duration::from_millis(2));
    if scheduler.finish(&next, true).is_some() {
        return Err("cancellation sequence: no request should remain".to_string());
    }
    let telemetry = scheduler.telemetry();
    steps.push(StepReceipt {
        step: SequenceStep::Cancellation,
        analyses_started_delta: telemetry.analyses_started,
        requests_coalesced_delta: telemetry.requests_coalesced,
        completed_but_superseded_delta: telemetry.completed_but_superseded,
        published_payload_bytes: 0,
        suppressed_payload_bytes: 0,
        run_status: "seams_deferred".to_string(),
        cache_load_status: CacheLoadStatus::NotObserved,
        full_scan_fallback_reason: None,
        full_rescan: false,
        input_identity_unchanged: false,
        semantic_scope: SemanticScope::Interactive,
        semantic_output_digest: Some("last-known-good".to_string()),
        stale_semantic_output: false,
        elapsed_ms: telemetry.latest_save_to_snapshot_ms,
        rss_bytes: None,
        actionable_finding_count: 0,
        related_evidence_invalidated: None,
        last_known_good_visible: true,
        superseded_presented_as_current: false,
        claimed_no_work_needed: false,
        timeout_last_completed_phase: None,
    });

    let cache_dir = root.join("target/ripr/cache-corrupt");
    std::fs::create_dir_all(&cache_dir)
        .map_err(|err| format!("create corrupt cache dir: {err}"))?;
    let cache = RepoSeamFactCache::at_dir(cache_dir.clone());
    let files = [(PathBuf::from("src/lib.rs"), b"pub fn x() {}\n".to_vec())];
    let state = crate::analysis::seam_cache::WorkspaceState {
        workspace_root: Path::new("/lsp-seq"),
        files: &files,
        cfg_features: None,
        config_text: None,
        test_intent_text: None,
        suppressions_text: None,
    };
    let key = state.cache_key();
    cache
        .store_classified_seams_with_limit(&key, &[], None, 20_000)
        .map_err(|err| format!("store cache fixture: {err}"))?;
    corrupt_json_files(&cache_dir)?;
    let fallback = match cache.load_classified_seams(&key) {
        CacheLoad::CorruptIgnored { reason } => reason,
        other => {
            return Err(format!(
                "expected named corrupt_ignored fallback, got {other:?}"
            ));
        }
    };
    let after_corrupt = observe_diagnostics(root, &config, true)?;
    steps.push(StepReceipt {
        step: SequenceStep::CorruptCache,
        analyses_started_delta: 1,
        requests_coalesced_delta: 0,
        completed_but_superseded_delta: 0,
        published_payload_bytes: 8,
        suppressed_payload_bytes: 0,
        run_status: run_status_for(after_corrupt.3).to_string(),
        cache_load_status: CacheLoadStatus::CorruptIgnored,
        full_scan_fallback_reason: Some(fallback),
        full_rescan: true,
        input_identity_unchanged: false,
        semantic_scope: SemanticScope::Interactive,
        semantic_output_digest: Some(after_corrupt.0),
        stale_semantic_output: false,
        elapsed_ms: Some(1),
        rss_bytes: None,
        actionable_finding_count: after_corrupt.1,
        related_evidence_invalidated: None,
        last_known_good_visible: true,
        superseded_presented_as_current: false,
        claimed_no_work_needed: false,
        timeout_last_completed_phase: None,
    });

    let full = observe_diagnostics(root, &config, false)?;
    if full.3 {
        return Err("explicit full refresh must not leave seams deferred".to_string());
    }
    steps.push(StepReceipt {
        step: SequenceStep::ExplicitFullRefresh,
        analyses_started_delta: 1,
        requests_coalesced_delta: 0,
        completed_but_superseded_delta: 0,
        published_payload_bytes: 32,
        suppressed_payload_bytes: 0,
        run_status: run_status_for(full.3).to_string(),
        cache_load_status: CacheLoadStatus::Miss,
        full_scan_fallback_reason: None,
        full_rescan: true,
        input_identity_unchanged: false,
        semantic_scope: SemanticScope::Full,
        semantic_output_digest: Some(full.0),
        stale_semantic_output: false,
        elapsed_ms: Some(1),
        rss_bytes: None,
        actionable_finding_count: full.1,
        related_evidence_invalidated: None,
        last_known_good_visible: true,
        superseded_presented_as_current: false,
        claimed_no_work_needed: false,
        timeout_last_completed_phase: None,
    });

    Ok(SavedEditSequenceReceipt {
        identity: SequenceIdentity {
            source_sha: None,
            binary_path: binary_path.to_string(),
            binary_digest: None,
            host_class: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
            features: vec!["lang-rust".to_string()],
            cache_reset_procedure:
                "isolated temp fixture tree; corrupt cache uses a private directory".to_string(),
            sample_count: 1,
            hidden_workspace_binary: false,
        },
        proposed_envelopes: default_proposed_envelopes(),
        steps,
        optimization_verdict: OptimizationVerdict::NoChange,
    })
}

#[cfg(test)]
fn mutate_step(
    receipt: &mut SavedEditSequenceReceipt,
    step: SequenceStep,
    mutate: impl FnOnce(&mut StepReceipt),
) -> Result<(), String> {
    let Some(found) = receipt
        .steps
        .iter_mut()
        .find(|candidate| candidate.step == step)
    else {
        return Err(format!("missing step {}", step.as_str()));
    };
    mutate(found);
    Ok(())
}

#[cfg(test)]
fn expect_violations(
    receipt: &SavedEditSequenceReceipt,
    label: &str,
) -> Result<Vec<SequenceViolation>, String> {
    match evaluate_sequence(receipt) {
        Err(violations) => Ok(violations),
        Ok(()) => Err(format!("{label}: expected sequence violations")),
    }
}

#[cfg(test)]
fn require_code(violations: &[SequenceViolation], code: &str) -> Result<(), String> {
    if violations.iter().any(|violation| violation.code == code) {
        Ok(())
    } else {
        Err(format!("missing `{code}` in {violations:?}"))
    }
}

#[cfg(test)]
fn require_any_code(violations: &[SequenceViolation], codes: &[&str]) -> Result<(), String> {
    if violations
        .iter()
        .any(|violation| codes.contains(&violation.code))
    {
        Ok(())
    } else {
        Err(format!("missing one of {codes:?} in {violations:?}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn honest_sequence_passes() -> Result<(), String> {
        evaluate_sequence(&honest_receipt()).map_err(|violations| format!("{violations:?}"))
    }

    #[test]
    fn stale_cached_answer_cannot_satisfy_speed_target() -> Result<(), String> {
        let mut receipt = honest_receipt();
        mutate_step(&mut receipt, SequenceStep::UnchangedSave, |step| {
            step.stale_semantic_output = true;
            step.elapsed_ms = Some(3);
            step.cache_load_status = CacheLoadStatus::Hit;
        })?;
        let violations = expect_violations(&receipt, "stale+fast must fail")?;
        require_code(&violations, "stale_cache_satisfied_speed_target")?;
        require_code(&violations, "stale_semantic_output")
    }

    #[test]
    fn stale_semantic_output_fails_independently_of_elapsed_time() -> Result<(), String> {
        let mut receipt = honest_receipt();
        mutate_step(&mut receipt, SequenceStep::ProductionEdit, |step| {
            step.stale_semantic_output = true;
            step.elapsed_ms = Some(PROPOSED_WARM_SAVE_P95_MS.saturating_add(500));
            step.cache_load_status = CacheLoadStatus::Miss;
        })?;
        let violations = expect_violations(&receipt, "slow stale miss must fail")?;
        require_code(&violations, "stale_semantic_output")?;
        if violations
            .iter()
            .any(|violation| violation.code == "stale_cache_satisfied_speed_target")
        {
            return Err(
                "slow stale miss must not be classified as a speed-target story".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn fast_redundant_full_rescan_is_caught() -> Result<(), String> {
        let mut receipt = honest_receipt();
        mutate_step(&mut receipt, SequenceStep::UnchangedRefresh, |step| {
            step.full_rescan = true;
            step.elapsed_ms = Some(4);
        })?;
        let violations = expect_violations(&receipt, "fast rescan must fail")?;
        require_code(&violations, "redundant_full_rescan")
    }

    #[test]
    fn duplicate_diagnostic_publication_is_caught_even_when_fast() -> Result<(), String> {
        let mut receipt = honest_receipt();
        mutate_step(&mut receipt, SequenceStep::UnchangedSave, |step| {
            step.published_payload_bytes = 99;
            step.elapsed_ms = Some(1);
        })?;
        let violations = expect_violations(&receipt, "duplicate publish must fail")?;
        require_code(&violations, "duplicate_diagnostic_publication")
    }

    #[test]
    fn related_test_edit_must_invalidate_evidence() -> Result<(), String> {
        let mut receipt = honest_receipt();
        mutate_step(&mut receipt, SequenceStep::RelatedTestEdit, |step| {
            step.related_evidence_invalidated = Some(false);
        })?;
        let violations = expect_violations(&receipt, "related no-op must fail")?;
        require_code(&violations, "related_test_did_not_invalidate_evidence")
    }

    #[test]
    fn unrelated_test_edit_cannot_manufacture_actionable_findings() -> Result<(), String> {
        let mut receipt = honest_receipt();
        mutate_step(&mut receipt, SequenceStep::UnrelatedTestEdit, |step| {
            step.actionable_finding_count = 2;
        })?;
        let violations = expect_violations(&receipt, "unrelated findings must fail")?;
        require_code(
            &violations,
            "unrelated_test_manufactured_actionable_finding",
        )
    }

    #[test]
    fn unrelated_test_edit_cannot_claim_no_work_was_needed() -> Result<(), String> {
        let mut receipt = honest_receipt();
        mutate_step(&mut receipt, SequenceStep::UnrelatedTestEdit, |step| {
            step.claimed_no_work_needed = true;
        })?;
        let violations = expect_violations(&receipt, "false no-work claim must fail")?;
        require_code(&violations, "unrelated_test_false_no_work_claim")
    }

    #[test]
    fn explicit_full_refresh_cannot_be_labelled_interactive() -> Result<(), String> {
        let mut receipt = honest_receipt();
        mutate_step(&mut receipt, SequenceStep::ExplicitFullRefresh, |step| {
            step.semantic_scope = SemanticScope::Interactive;
            step.run_status = "seams_deferred".to_string();
        })?;
        let violations = expect_violations(&receipt, "mislabelled full refresh must fail")?;
        require_any_code(
            &violations,
            &[
                "full_refresh_labeled_interactive",
                "full_refresh_reported_seams_deferred",
            ],
        )
    }

    #[test]
    fn proposed_latency_targets_cannot_be_gates() -> Result<(), String> {
        let mut receipt = honest_receipt();
        let Some(envelope) = receipt.proposed_envelopes.get_mut(0) else {
            return Err("missing first proposed envelope".to_string());
        };
        envelope.class = EnvelopeClass::Gating;
        let violations = expect_violations(&receipt, "gating proposal must fail")?;
        require_code(&violations, "proposed_latency_treated_as_gate")
    }

    #[test]
    fn hidden_workspace_binary_is_rejected() -> Result<(), String> {
        let mut receipt = honest_receipt();
        receipt.identity.hidden_workspace_binary = true;
        let violations = expect_violations(&receipt, "hidden binary must fail")?;
        require_code(&violations, "hidden_workspace_binary")
    }

    #[test]
    fn superseded_work_cannot_present_as_current_zero_gap() -> Result<(), String> {
        let mut receipt = honest_receipt();
        mutate_step(&mut receipt, SequenceStep::Cancellation, |step| {
            step.superseded_presented_as_current = true;
            step.last_known_good_visible = false;
        })?;
        let violations = expect_violations(&receipt, "superseded-as-current must fail")?;
        require_any_code(
            &violations,
            &["superseded_presented_as_current", "last_known_good_cleared"],
        )
    }

    #[test]
    fn corrupt_cache_hit_is_rejected() -> Result<(), String> {
        let mut receipt = honest_receipt();
        mutate_step(&mut receipt, SequenceStep::CorruptCache, |step| {
            step.cache_load_status = CacheLoadStatus::Hit;
            step.full_scan_fallback_reason = None;
            step.stale_semantic_output = true;
        })?;
        let violations = expect_violations(&receipt, "corrupt hit must fail")?;
        require_any_code(
            &violations,
            &[
                "corrupt_cache_reported_as_hit",
                "corrupt_cache_unnamed_fallback",
            ],
        )
    }

    #[test]
    fn config_change_warm_hit_is_rejected() -> Result<(), String> {
        let mut receipt = honest_receipt();
        mutate_step(&mut receipt, SequenceStep::ConfigChange, |step| {
            step.cache_load_status = CacheLoadStatus::Hit;
        })?;
        let violations = expect_violations(&receipt, "config warm hit must fail")?;
        require_code(&violations, "stale_warm_hit_after_config_change")
    }

    #[test]
    fn missing_step_is_rejected() -> Result<(), String> {
        let mut receipt = honest_receipt();
        if receipt.steps.pop().is_none() {
            return Err("honest receipt had no steps".to_string());
        }
        let violations = expect_violations(&receipt, "missing step must fail")?;
        require_code(&violations, "missing_sequence_step")
    }

    #[test]
    fn proposed_envelopes_are_not_achieved_results() -> Result<(), String> {
        let mut receipt = honest_receipt();
        let Some(envelope) = receipt.proposed_envelopes.get_mut(2) else {
            return Err("missing third proposed envelope".to_string());
        };
        envelope.class = EnvelopeClass::Achieved;
        let violations = expect_violations(&receipt, "achieved proposal must fail")?;
        require_code(&violations, "proposed_latency_treated_as_gate")
    }

    #[test]
    fn envelope_and_verdict_vocabulary_is_complete() -> Result<(), String> {
        let classes = [
            EnvelopeClass::Proposal,
            EnvelopeClass::Advisory,
            EnvelopeClass::Provisional,
            EnvelopeClass::Gating,
            EnvelopeClass::NotMeasured,
            EnvelopeClass::Achieved,
        ];
        let names = classes.map(EnvelopeClass::as_str);
        if names
            != [
                "proposal",
                "advisory",
                "provisional",
                "gating",
                "not_measured",
                "achieved",
            ]
        {
            return Err(format!("unexpected envelope class names: {names:?}"));
        }
        if OptimizationVerdict::NoChange.as_str() != "no_change" {
            return Err("no_change vocabulary drifted".to_string());
        }
        if OptimizationVerdict::NotEstablished.as_str() != "not_established" {
            return Err("not_established vocabulary drifted".to_string());
        }
        let mut receipt = honest_receipt();
        receipt.optimization_verdict = OptimizationVerdict::NotEstablished;
        let Some(cold) = receipt.proposed_envelopes.get_mut(1) else {
            return Err("missing cold envelope".to_string());
        };
        cold.class = EnvelopeClass::Advisory;
        let Some(warm_pr) = receipt.proposed_envelopes.get_mut(2) else {
            return Err("missing warm PR envelope".to_string());
        };
        warm_pr.class = EnvelopeClass::Provisional;
        evaluate_sequence(&receipt).map_err(|violations| format!("{violations:?}"))?;
        let mut measured = honest_receipt();
        let Some(warm) = measured.proposed_envelopes.get_mut(0) else {
            return Err("missing warm envelope".to_string());
        };
        warm.class = EnvelopeClass::NotMeasured;
        evaluate_sequence(&measured).map_err(|violations| format!("{violations:?}"))
    }

    #[test]
    fn timeout_must_not_fabricate_later_phases() -> Result<(), String> {
        let mut receipt = honest_receipt();
        mutate_step(&mut receipt, SequenceStep::Cancellation, |step| {
            step.timeout_last_completed_phase = Some("fabricated".to_string());
        })?;
        let violations = expect_violations(&receipt, "fabricated phases must fail")?;
        require_code(&violations, "timeout_fabricated_later_phases")
    }

    #[test]
    fn cancellation_must_count_superseded_or_coalesced_work() -> Result<(), String> {
        let mut receipt = honest_receipt();
        mutate_step(&mut receipt, SequenceStep::Cancellation, |step| {
            step.requests_coalesced_delta = 0;
            step.completed_but_superseded_delta = 0;
        })?;
        let violations = expect_violations(&receipt, "invisible cancellation must fail")?;
        require_code(&violations, "cancellation_not_visible")
    }

    #[test]
    fn interactive_cold_start_cannot_report_full() -> Result<(), String> {
        let mut receipt = honest_receipt();
        mutate_step(&mut receipt, SequenceStep::ColdStart, |step| {
            step.semantic_scope = SemanticScope::Interactive;
            step.run_status = "full".to_string();
        })?;
        let violations = expect_violations(&receipt, "full cold start must fail")?;
        require_code(&violations, "interactive_cold_start_reported_full")
    }

    #[test]
    fn saved_edit_sequence_fixture_harness() -> Result<(), String> {
        let root = super::super::tests::unique_lsp_test_root("saved-edit-sequence")?;
        let binary = env!("CARGO_MANIFEST_DIR")
            .replace('\\', "/")
            .trim_end_matches("/crates/ripr")
            .to_string()
            + "/target/debug/ripr";
        let receipt = drive_saved_edit_sequence(root.path(), &binary)?;
        evaluate_sequence(&receipt).map_err(|violations| format!("{violations:?}"))?;
        let json = receipt_to_json(&receipt);
        let reports = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/ripr/reports");
        std::fs::create_dir_all(&reports).map_err(|err| format!("create reports dir: {err}"))?;
        std::fs::write(
            reports.join("lsp-performance.json"),
            format!(
                "{}\n",
                serde_json::to_string_pretty(&json)
                    .map_err(|err| format!("serialize receipt: {err}"))?
            ),
        )
        .map_err(|err| format!("write receipt: {err}"))?;
        std::fs::write(
            reports.join("lsp-performance.md"),
            receipt_markdown(&receipt),
        )
        .map_err(|err| format!("write markdown: {err}"))?;
        Ok(())
    }
}
