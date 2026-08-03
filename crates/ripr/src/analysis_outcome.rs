//! Versioned producer-owned analysis completeness and limitation contract.
//!
//! This module is internal until the parser and public output projections have
//! adopted the contract. Renderers must consume these typed facts rather than
//! infer completeness from empty findings or `probe_count == 0`. (#2827)

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;

pub(crate) const ANALYSIS_OUTCOME_SCHEMA_VERSION: &str = "0.1";
pub(crate) const ANALYSIS_OUTCOME_CLAIM_BOUNDARY: &str = "Static analysis outcome only; no correctness, test-adequacy, runtime-execution, or merge-readiness claim.";
pub(crate) const MAX_ANALYSIS_LIMITATION_DETAIL_CHARS: usize = 512;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AnalysisOutcomeKind {
    NoScope,
    NoChangedLines,
    NoBehavioralCandidates,
    CompleteNoFindings,
    CompleteWithFindings,
    PartialWithLimitations,
    UnsupportedInput,
    AnalysisFailed,
}

impl AnalysisOutcomeKind {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::NoScope => "no_scope",
            Self::NoChangedLines => "no_changed_lines",
            Self::NoBehavioralCandidates => "no_behavioral_candidates",
            Self::CompleteNoFindings => "complete_no_findings",
            Self::CompleteWithFindings => "complete_with_findings",
            Self::PartialWithLimitations => "partial_with_limitations",
            Self::UnsupportedInput => "unsupported_input",
            Self::AnalysisFailed => "analysis_failed",
        }
    }

    pub(crate) const fn is_complete(self) -> bool {
        matches!(
            self,
            Self::NoScope
                | Self::NoChangedLines
                | Self::NoBehavioralCandidates
                | Self::CompleteNoFindings
                | Self::CompleteWithFindings
        )
    }
}

impl AnalysisLimitationKind {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::CombinedHunkUnsupported => "combined_hunk_unsupported",
            Self::UnresolvedConflictMarkers => "unresolved_conflict_markers",
            Self::MalformedDiff => "malformed_diff",
            Self::DiffScopeOversized => "diff_scope_oversized",
            Self::LanguageAdapterUnavailable => "language_adapter_unavailable",
            Self::LanguageScopeUnsupported => "language_scope_unsupported",
            Self::ProducerTimeout => "producer_timeout",
            Self::ProducerFailure => "producer_failure",
        }
    }
}

impl AnalysisStage {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::DiffLoad => "diff_load",
            Self::DiffParse => "diff_parse",
            Self::LanguageAdapter => "language_adapter",
            Self::ProbeGeneration => "probe_generation",
            Self::FindingClassification => "finding_classification",
            Self::AnalysisPipeline => "analysis_pipeline",
        }
    }
}

impl AnalysisRecoveryKind {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::NarrowDiff => "narrow_diff",
            Self::UseTwoWayDiff => "use_two_way_diff",
            Self::ResolveConflicts => "resolve_conflicts",
            Self::EnableLanguage => "enable_language",
            Self::IncreaseConfiguredLimit => "increase_configured_limit",
            Self::Retry => "retry",
            Self::InspectFailure => "inspect_failure",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AnalysisStage {
    DiffLoad,
    DiffParse,
    LanguageAdapter,
    ProbeGeneration,
    FindingClassification,
    AnalysisPipeline,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AnalysisLimitationKind {
    CombinedHunkUnsupported,
    UnresolvedConflictMarkers,
    MalformedDiff,
    DiffScopeOversized,
    LanguageAdapterUnavailable,
    LanguageScopeUnsupported,
    ProducerTimeout,
    ProducerFailure,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AnalysisRecoveryKind {
    NarrowDiff,
    UseTwoWayDiff,
    ResolveConflicts,
    EnableLanguage,
    IncreaseConfiguredLimit,
    Retry,
    InspectFailure,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub(crate) struct AnalysisIdentity {
    /// Stable repository identity or commitment. Absolute checkout paths do
    /// not belong in this portable identity field.
    pub(crate) repository_identity: Option<String>,
    /// Stable root identity or commitment. Absolute checkout paths do not
    /// belong in this portable identity field.
    pub(crate) root_identity: Option<String>,
    pub(crate) config_identity: Option<String>,
    pub(crate) base_revision: Option<String>,
    pub(crate) input_identity: Option<String>,
    pub(crate) snapshot_identity: Option<String>,
}

impl AnalysisIdentity {
    fn normalize(&mut self) -> Result<(), String> {
        for (field, value) in [
            ("repository_identity", &mut self.repository_identity),
            ("root_identity", &mut self.root_identity),
            ("config_identity", &mut self.config_identity),
            ("base_revision", &mut self.base_revision),
            ("input_identity", &mut self.input_identity),
            ("snapshot_identity", &mut self.snapshot_identity),
        ] {
            normalize_optional_bounded_text(field, value)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub(crate) struct AnalysisOutcomeCounts {
    pub(crate) changed_file_count: u64,
    pub(crate) changed_line_count: u64,
    pub(crate) candidate_line_count: u64,
    pub(crate) probe_count: u64,
    pub(crate) finding_count: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub(crate) struct AnalysisRecovery {
    pub(crate) kind: AnalysisRecoveryKind,
    pub(crate) detail: String,
}

impl AnalysisRecovery {
    pub(crate) fn new(
        kind: AnalysisRecoveryKind,
        detail: impl Into<String>,
    ) -> Result<Self, String> {
        let detail = bounded_nonempty_text("analysis recovery detail", detail.into())?;
        Ok(Self { kind, detail })
    }

    fn normalize(&mut self) -> Result<(), String> {
        self.detail = bounded_nonempty_text("analysis recovery detail", self.detail.clone())?;
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub(crate) struct AnalysisLimitation {
    pub(crate) kind: AnalysisLimitationKind,
    pub(crate) producer_stage: AnalysisStage,
    /// Repository-relative portable path. Absolute paths and parent traversal
    /// are rejected before serialization.
    pub(crate) path: Option<String>,
    pub(crate) affected_items: Option<u64>,
    pub(crate) bounded_detail: Option<String>,
    pub(crate) recovery: AnalysisRecovery,
}

impl AnalysisLimitation {
    pub(crate) fn new(
        kind: AnalysisLimitationKind,
        producer_stage: AnalysisStage,
        recovery: AnalysisRecovery,
    ) -> Self {
        Self {
            kind,
            producer_stage,
            path: None,
            affected_items: None,
            bounded_detail: None,
            recovery,
        }
    }

    pub(crate) fn with_path(mut self, path: impl AsRef<str>) -> Result<Self, String> {
        self.path = Some(normalize_portable_analysis_path(path.as_ref())?);
        Ok(self)
    }

    pub(crate) fn with_affected_items(mut self, affected_items: u64) -> Result<Self, String> {
        if affected_items == 0 {
            return Err("analysis limitation affected_items must be positive".to_string());
        }
        self.affected_items = Some(affected_items);
        Ok(self)
    }

    pub(crate) fn with_detail(mut self, detail: impl Into<String>) -> Result<Self, String> {
        self.bounded_detail = Some(bounded_nonempty_text(
            "analysis limitation detail",
            detail.into(),
        )?);
        Ok(self)
    }

    fn normalize(&mut self) -> Result<(), String> {
        if let Some(path) = self.path.take() {
            self.path = Some(normalize_portable_analysis_path(&path)?);
        }
        if self.affected_items == Some(0) {
            return Err("analysis limitation affected_items must be positive".to_string());
        }
        normalize_optional_bounded_text("analysis limitation detail", &mut self.bounded_detail)?;
        self.recovery.normalize()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct AnalysisOutcome {
    pub(crate) schema_version: String,
    pub(crate) kind: AnalysisOutcomeKind,
    pub(crate) identity: AnalysisIdentity,
    pub(crate) counts: AnalysisOutcomeCounts,
    pub(crate) limitations: Vec<AnalysisLimitation>,
    pub(crate) claim_boundary: String,
}

#[derive(Deserialize)]
struct AnalysisOutcomeWire {
    schema_version: String,
    kind: AnalysisOutcomeKind,
    identity: AnalysisIdentity,
    counts: AnalysisOutcomeCounts,
    limitations: Vec<AnalysisLimitation>,
    claim_boundary: String,
}

impl<'de> Deserialize<'de> for AnalysisOutcome {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = AnalysisOutcomeWire::deserialize(deserializer)?;
        if wire.schema_version != ANALYSIS_OUTCOME_SCHEMA_VERSION {
            return Err(D::Error::custom(format!(
                "analysis outcome schema_version must be {ANALYSIS_OUTCOME_SCHEMA_VERSION}, got {}",
                wire.schema_version
            )));
        }
        if wire.claim_boundary != ANALYSIS_OUTCOME_CLAIM_BOUNDARY {
            return Err(D::Error::custom(
                "analysis outcome claim_boundary does not match the versioned contract",
            ));
        }
        AnalysisOutcome::new(wire.kind, wire.identity, wire.counts, wire.limitations)
            .map_err(D::Error::custom)
    }
}

impl AnalysisOutcome {
    pub(crate) fn new(
        kind: AnalysisOutcomeKind,
        mut identity: AnalysisIdentity,
        counts: AnalysisOutcomeCounts,
        mut limitations: Vec<AnalysisLimitation>,
    ) -> Result<Self, String> {
        identity.normalize()?;
        for limitation in &mut limitations {
            limitation.normalize()?;
        }
        limitations.sort();
        limitations.dedup();
        validate_outcome(kind, counts, &limitations)?;
        Ok(Self {
            schema_version: ANALYSIS_OUTCOME_SCHEMA_VERSION.to_string(),
            kind,
            identity,
            counts,
            limitations,
            claim_boundary: ANALYSIS_OUTCOME_CLAIM_BOUNDARY.to_string(),
        })
    }

    /// Stable semantic commitment over the versioned typed outcome. The DTO
    /// contains no timestamp, duration, map, or absolute checkout path; the
    /// constructor sorts and deduplicates limitations before hashing.
    pub(crate) fn semantic_digest(&self) -> Result<String, String> {
        let bytes = serde_json::to_vec(self)
            .map_err(|error| format!("serialize analysis outcome for digest failed: {error}"))?;
        let digest = Sha256::digest(bytes);
        let mut hex = String::with_capacity(digest.len() * 2);
        for byte in digest {
            write!(&mut hex, "{byte:02x}")
                .map_err(|error| format!("format analysis outcome digest failed: {error}"))?;
        }
        Ok(format!("sha256:{hex}"))
    }
}

pub(crate) fn normalize_portable_analysis_path(path: &str) -> Result<String, String> {
    let normalized = path.trim().replace('\\', "/");
    let without_current = normalized.trim_start_matches("./");
    let bytes = without_current.as_bytes();
    let has_drive_prefix = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
    if without_current.is_empty() || without_current.starts_with('/') || has_drive_prefix {
        return Err(format!(
            "analysis limitation path must be repository-relative: {path}"
        ));
    }

    let mut segments = Vec::new();
    for segment in without_current.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                return Err(format!(
                    "analysis limitation path contains parent traversal: {path}"
                ));
            }
            value => segments.push(value),
        }
    }
    if segments.is_empty() {
        return Err(format!(
            "analysis limitation path has no portable segments: {path}"
        ));
    }
    Ok(segments.join("/"))
}

fn normalize_optional_bounded_text(field: &str, value: &mut Option<String>) -> Result<(), String> {
    if let Some(current) = value.take() {
        *value = Some(bounded_nonempty_text(field, current)?);
    }
    Ok(())
}

fn bounded_nonempty_text(field: &str, value: String) -> Result<String, String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(format!("{field} must not be empty"));
    }
    let count = value.chars().count();
    if count > MAX_ANALYSIS_LIMITATION_DETAIL_CHARS {
        return Err(format!(
            "{field} has {count} characters; maximum is {MAX_ANALYSIS_LIMITATION_DETAIL_CHARS}"
        ));
    }
    Ok(value)
}

fn validate_outcome(
    kind: AnalysisOutcomeKind,
    counts: AnalysisOutcomeCounts,
    limitations: &[AnalysisLimitation],
) -> Result<(), String> {
    let complete_or_subject_absent = matches!(
        kind,
        AnalysisOutcomeKind::NoScope
            | AnalysisOutcomeKind::NoChangedLines
            | AnalysisOutcomeKind::NoBehavioralCandidates
            | AnalysisOutcomeKind::CompleteNoFindings
            | AnalysisOutcomeKind::CompleteWithFindings
    );
    if complete_or_subject_absent && !limitations.is_empty() {
        return Err(format!(
            "analysis outcome {kind:?} cannot carry incomplete-analysis limitations"
        ));
    }

    let limited_or_failed = matches!(
        kind,
        AnalysisOutcomeKind::PartialWithLimitations
            | AnalysisOutcomeKind::UnsupportedInput
            | AnalysisOutcomeKind::AnalysisFailed
    );
    if limited_or_failed && limitations.is_empty() {
        return Err(format!(
            "analysis outcome {kind:?} requires at least one typed limitation"
        ));
    }

    match kind {
        AnalysisOutcomeKind::NoScope => {
            if counts != AnalysisOutcomeCounts::default() {
                return Err("analysis outcome NoScope requires every count to be zero".to_string());
            }
        }
        AnalysisOutcomeKind::NoChangedLines => {
            if counts.changed_line_count != 0
                || counts.candidate_line_count != 0
                || counts.probe_count != 0
                || counts.finding_count != 0
            {
                return Err(
                    "analysis outcome NoChangedLines requires zero changed-line, candidate, probe, and finding counts"
                        .to_string(),
                );
            }
        }
        AnalysisOutcomeKind::NoBehavioralCandidates => {
            if counts.changed_line_count == 0
                || counts.candidate_line_count != 0
                || counts.probe_count != 0
                || counts.finding_count != 0
            {
                return Err(format!(
                    "no_behavioral_candidates requires changed lines and zero candidate, probe, and finding counts (changed_line_count={}, candidate_line_count={}, probe_count={}, finding_count={})",
                    counts.changed_line_count,
                    counts.candidate_line_count,
                    counts.probe_count,
                    counts.finding_count,
                ));
            }
        }
        AnalysisOutcomeKind::CompleteNoFindings => {
            if counts.finding_count != 0 {
                return Err("complete_no_findings requires finding_count = 0".to_string());
            }
            if counts.candidate_line_count == 0 && counts.probe_count == 0 {
                return Err(
                    "complete_no_findings requires a behavioral candidate or probe subject"
                        .to_string(),
                );
            }
        }
        AnalysisOutcomeKind::CompleteWithFindings => {
            if counts.finding_count == 0 {
                return Err("complete_with_findings requires finding_count > 0".to_string());
            }
        }
        AnalysisOutcomeKind::PartialWithLimitations
        | AnalysisOutcomeKind::UnsupportedInput
        | AnalysisOutcomeKind::AnalysisFailed => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::{Debug, Display};

    fn recovery() -> Result<AnalysisRecovery, String> {
        AnalysisRecovery::new(
            AnalysisRecoveryKind::InspectFailure,
            "Inspect the typed limitation before retrying.",
        )
    }

    fn limitation(kind: AnalysisLimitationKind) -> Result<AnalysisLimitation, String> {
        Ok(AnalysisLimitation::new(
            kind,
            AnalysisStage::DiffParse,
            recovery()?,
        ))
    }

    fn outcome(
        kind: AnalysisOutcomeKind,
        counts: AnalysisOutcomeCounts,
        limitations: Vec<AnalysisLimitation>,
    ) -> Result<AnalysisOutcome, String> {
        AnalysisOutcome::new(kind, AnalysisIdentity::default(), counts, limitations)
    }

    fn expect_error<T: Debug, E: Display>(
        result: Result<T, E>,
        expected: &str,
    ) -> Result<(), String> {
        match result {
            Ok(value) => Err(format!(
                "expected error containing `{expected}`, got success: {value:?}"
            )),
            Err(error) if error.to_string().contains(expected) => Ok(()),
            Err(error) => Err(format!(
                "expected error containing `{expected}`, got `{error}`"
            )),
        }
    }

    #[test]
    fn closed_outcome_kinds_round_trip() -> Result<(), String> {
        let cases = [
            outcome(
                AnalysisOutcomeKind::NoScope,
                AnalysisOutcomeCounts::default(),
                Vec::new(),
            )?,
            outcome(
                AnalysisOutcomeKind::NoChangedLines,
                AnalysisOutcomeCounts {
                    changed_file_count: 1,
                    ..AnalysisOutcomeCounts::default()
                },
                Vec::new(),
            )?,
            outcome(
                AnalysisOutcomeKind::NoBehavioralCandidates,
                AnalysisOutcomeCounts {
                    changed_file_count: 1,
                    changed_line_count: 2,
                    ..AnalysisOutcomeCounts::default()
                },
                Vec::new(),
            )?,
            outcome(
                AnalysisOutcomeKind::CompleteNoFindings,
                AnalysisOutcomeCounts {
                    changed_file_count: 1,
                    changed_line_count: 1,
                    candidate_line_count: 1,
                    probe_count: 1,
                    finding_count: 0,
                },
                Vec::new(),
            )?,
            outcome(
                AnalysisOutcomeKind::CompleteWithFindings,
                AnalysisOutcomeCounts {
                    changed_file_count: 1,
                    changed_line_count: 1,
                    candidate_line_count: 1,
                    probe_count: 1,
                    finding_count: 1,
                },
                Vec::new(),
            )?,
            outcome(
                AnalysisOutcomeKind::PartialWithLimitations,
                AnalysisOutcomeCounts::default(),
                vec![limitation(AnalysisLimitationKind::DiffScopeOversized)?],
            )?,
            outcome(
                AnalysisOutcomeKind::UnsupportedInput,
                AnalysisOutcomeCounts::default(),
                vec![limitation(AnalysisLimitationKind::CombinedHunkUnsupported)?],
            )?,
            outcome(
                AnalysisOutcomeKind::AnalysisFailed,
                AnalysisOutcomeCounts::default(),
                vec![limitation(AnalysisLimitationKind::ProducerFailure)?],
            )?,
        ];

        for expected in cases {
            let json = serde_json::to_string(&expected)
                .map_err(|error| format!("serialize outcome fixture failed: {error}"))?;
            let actual: AnalysisOutcome = serde_json::from_str(&json)
                .map_err(|error| format!("parse outcome fixture failed: {error}"))?;
            assert_eq!(actual, expected);
        }
        Ok(())
    }

    #[test]
    fn typed_limitation_tokens_match_the_wire_contract() {
        let limitation_kinds = [
            (
                AnalysisLimitationKind::CombinedHunkUnsupported,
                "combined_hunk_unsupported",
            ),
            (
                AnalysisLimitationKind::UnresolvedConflictMarkers,
                "unresolved_conflict_markers",
            ),
            (AnalysisLimitationKind::MalformedDiff, "malformed_diff"),
            (
                AnalysisLimitationKind::DiffScopeOversized,
                "diff_scope_oversized",
            ),
            (
                AnalysisLimitationKind::LanguageAdapterUnavailable,
                "language_adapter_unavailable",
            ),
            (
                AnalysisLimitationKind::LanguageScopeUnsupported,
                "language_scope_unsupported",
            ),
            (AnalysisLimitationKind::ProducerTimeout, "producer_timeout"),
            (AnalysisLimitationKind::ProducerFailure, "producer_failure"),
        ];
        for (kind, expected) in limitation_kinds {
            assert_eq!(kind.as_str(), expected);
        }

        let stages = [
            (AnalysisStage::DiffLoad, "diff_load"),
            (AnalysisStage::DiffParse, "diff_parse"),
            (AnalysisStage::LanguageAdapter, "language_adapter"),
            (AnalysisStage::ProbeGeneration, "probe_generation"),
            (
                AnalysisStage::FindingClassification,
                "finding_classification",
            ),
            (AnalysisStage::AnalysisPipeline, "analysis_pipeline"),
        ];
        for (stage, expected) in stages {
            assert_eq!(stage.as_str(), expected);
        }

        let recoveries = [
            (AnalysisRecoveryKind::NarrowDiff, "narrow_diff"),
            (AnalysisRecoveryKind::UseTwoWayDiff, "use_two_way_diff"),
            (AnalysisRecoveryKind::ResolveConflicts, "resolve_conflicts"),
            (AnalysisRecoveryKind::EnableLanguage, "enable_language"),
            (
                AnalysisRecoveryKind::IncreaseConfiguredLimit,
                "increase_configured_limit",
            ),
            (AnalysisRecoveryKind::Retry, "retry"),
            (AnalysisRecoveryKind::InspectFailure, "inspect_failure"),
        ];
        for (recovery, expected) in recoveries {
            assert_eq!(recovery.as_str(), expected);
        }
    }

    #[test]
    fn no_behavioral_candidates_requires_positive_producer_facts() -> Result<(), String> {
        expect_error(
            outcome(
                AnalysisOutcomeKind::NoBehavioralCandidates,
                AnalysisOutcomeCounts::default(),
                Vec::new(),
            ),
            "requires changed lines",
        )?;

        let valid = outcome(
            AnalysisOutcomeKind::NoBehavioralCandidates,
            AnalysisOutcomeCounts {
                changed_file_count: 1,
                changed_line_count: 3,
                ..AnalysisOutcomeCounts::default()
            },
            Vec::new(),
        )?;
        assert_eq!(valid.kind, AnalysisOutcomeKind::NoBehavioralCandidates);
        Ok(())
    }

    #[test]
    fn complete_outcome_cannot_hide_a_limitation() -> Result<(), String> {
        expect_error(
            outcome(
                AnalysisOutcomeKind::CompleteNoFindings,
                AnalysisOutcomeCounts {
                    changed_file_count: 1,
                    changed_line_count: 1,
                    candidate_line_count: 1,
                    probe_count: 1,
                    finding_count: 0,
                },
                vec![limitation(AnalysisLimitationKind::CombinedHunkUnsupported)?],
            ),
            "cannot carry incomplete-analysis limitations",
        )
    }

    #[test]
    fn complete_no_findings_requires_a_behavioral_subject() -> Result<(), String> {
        expect_error(
            outcome(
                AnalysisOutcomeKind::CompleteNoFindings,
                AnalysisOutcomeCounts {
                    changed_file_count: 1,
                    changed_line_count: 2,
                    ..AnalysisOutcomeCounts::default()
                },
                Vec::new(),
            ),
            "requires a behavioral candidate or probe subject",
        )
    }

    #[test]
    fn limitations_are_sorted_deduplicated_and_digest_stable() -> Result<(), String> {
        let combined = limitation(AnalysisLimitationKind::CombinedHunkUnsupported)?
            .with_path("src\\lib.rs")?
            .with_affected_items(1)?;
        let conflicts = limitation(AnalysisLimitationKind::UnresolvedConflictMarkers)?
            .with_path("./src/lib.rs")?
            .with_affected_items(2)?;
        let left = outcome(
            AnalysisOutcomeKind::UnsupportedInput,
            AnalysisOutcomeCounts::default(),
            vec![conflicts.clone(), combined.clone(), combined.clone()],
        )?;
        let right = outcome(
            AnalysisOutcomeKind::UnsupportedInput,
            AnalysisOutcomeCounts::default(),
            vec![combined, conflicts],
        )?;
        assert_eq!(left.limitations, right.limitations);
        assert_eq!(left.semantic_digest()?, right.semantic_digest()?);
        assert_eq!(left.limitations[0].path.as_deref(), Some("src/lib.rs"));
        Ok(())
    }

    #[test]
    fn adding_a_limitation_changes_semantic_identity() -> Result<(), String> {
        let one = outcome(
            AnalysisOutcomeKind::UnsupportedInput,
            AnalysisOutcomeCounts::default(),
            vec![limitation(AnalysisLimitationKind::CombinedHunkUnsupported)?],
        )?;
        let two = outcome(
            AnalysisOutcomeKind::UnsupportedInput,
            AnalysisOutcomeCounts::default(),
            vec![
                limitation(AnalysisLimitationKind::CombinedHunkUnsupported)?,
                limitation(AnalysisLimitationKind::UnresolvedConflictMarkers)?,
            ],
        )?;
        assert_ne!(one.semantic_digest()?, two.semantic_digest()?);
        Ok(())
    }

    #[test]
    fn unsupported_input_and_producer_failure_remain_distinct() -> Result<(), String> {
        let unsupported = outcome(
            AnalysisOutcomeKind::UnsupportedInput,
            AnalysisOutcomeCounts::default(),
            vec![limitation(AnalysisLimitationKind::CombinedHunkUnsupported)?],
        )?;
        let failed = outcome(
            AnalysisOutcomeKind::AnalysisFailed,
            AnalysisOutcomeCounts::default(),
            vec![limitation(AnalysisLimitationKind::ProducerFailure)?],
        )?;
        assert_ne!(unsupported.kind, failed.kind);
        assert_ne!(unsupported.semantic_digest()?, failed.semantic_digest()?);
        Ok(())
    }

    #[test]
    fn portable_paths_reject_absolute_and_parent_traversal() -> Result<(), String> {
        expect_error(
            normalize_portable_analysis_path("/tmp/file.rs"),
            "repository-relative",
        )?;
        expect_error(
            normalize_portable_analysis_path("../file.rs"),
            "parent traversal",
        )?;
        let windows_absolute = ["C:", "tmp", "file.rs"].join("\\");
        expect_error(
            normalize_portable_analysis_path(&windows_absolute),
            "repository-relative",
        )?;
        let normalized = normalize_portable_analysis_path("src\\module\\file.rs")?;
        assert_eq!(normalized, "src/module/file.rs");
        Ok(())
    }

    #[test]
    fn limitation_text_and_affected_counts_are_bounded() -> Result<(), String> {
        let too_long = "x".repeat(MAX_ANALYSIS_LIMITATION_DETAIL_CHARS + 1);
        expect_error(
            AnalysisRecovery::new(AnalysisRecoveryKind::Retry, too_long.clone()),
            "maximum",
        )?;
        expect_error(
            limitation(AnalysisLimitationKind::ProducerFailure)?.with_detail(too_long),
            "maximum",
        )?;
        expect_error(
            limitation(AnalysisLimitationKind::ProducerFailure)?.with_affected_items(0),
            "must be positive",
        )?;
        Ok(())
    }

    #[test]
    fn deserialization_revalidates_schema_claims_and_paths() -> Result<(), String> {
        let valid = outcome(
            AnalysisOutcomeKind::UnsupportedInput,
            AnalysisOutcomeCounts::default(),
            vec![limitation(AnalysisLimitationKind::CombinedHunkUnsupported)?],
        )?;
        let mut value = serde_json::to_value(valid)
            .map_err(|error| format!("serialize outcome value failed: {error}"))?;
        value["schema_version"] = serde_json::json!("9.9");
        expect_error(
            serde_json::from_value::<AnalysisOutcome>(value),
            "schema_version must be",
        )?;

        let malformed = serde_json::json!({
            "schema_version": ANALYSIS_OUTCOME_SCHEMA_VERSION,
            "kind": "unsupported_input",
            "identity": {},
            "counts": {
                "changed_file_count": 0,
                "changed_line_count": 0,
                "candidate_line_count": 0,
                "probe_count": 0,
                "finding_count": 0
            },
            "limitations": [{
                "kind": "combined_hunk_unsupported",
                "producer_stage": "diff_parse",
                "path": "../outside.rs",
                "affected_items": 1,
                "bounded_detail": null,
                "recovery": {
                    "kind": "use_two_way_diff",
                    "detail": "Use a two-way diff."
                }
            }],
            "claim_boundary": ANALYSIS_OUTCOME_CLAIM_BOUNDARY
        });
        expect_error(
            serde_json::from_value::<AnalysisOutcome>(malformed),
            "parent traversal",
        )
    }
}
