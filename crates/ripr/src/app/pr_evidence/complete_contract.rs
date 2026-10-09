//! Versioned saved-generation DATA. Nothing deserialized here grants execution.
//!
//! Root owns native startup, authenticated input capture, the execution token,
//! early consumer dispatch, staging custody, currentness and publication. The
//! generation ID precedes payload hashes. The final saved proof hashes the
//! manifest bytes and is never embedded in the manifest or a payload.
//!
//! This contract refuses the configured Perl external-producer route. Capturing every
//! Perl configuration field as data does not authenticate its external inputs
//! or authorize a process. Extending supported execution requires its owner.

use super::complete_request::{CommittedRequestBinding, POLICY_PATH};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{self, Write};
use std::path::{Component, Path};

pub(super) const BINDING_SCHEMA: &str = "ripr.complete_binding.v3";
pub(super) const MANIFEST_SCHEMA: &str = "ripr.complete_manifest.v1";
pub(super) const PAYLOAD_SCHEMA: &str = "ripr.complete_payload.v1";
pub(super) const MANIFEST_FILE: &str = "complete-manifest.json";
pub(super) const INVENTORY_REPRESENTATION: &str = "linux_utf8_pathbuf_order.v1";

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq, Ord, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub(super) enum ArtifactRole {
    OriginalRaw,
    CheckDiff,
    PresentationDiff,
    RawLedger,
    FullCheck,
    FindingIndex,
    ReviewInput,
    PrJson,
    PrMarkdown,
}

impl ArtifactRole {
    pub(super) const ALL: [Self; 9] = [
        Self::OriginalRaw,
        Self::CheckDiff,
        Self::PresentationDiff,
        Self::RawLedger,
        Self::FullCheck,
        Self::FindingIndex,
        Self::ReviewInput,
        Self::PrJson,
        Self::PrMarkdown,
    ];

    pub(super) const fn path(self) -> &'static str {
        match self {
            Self::OriginalRaw => "canonical-u0.raw",
            Self::CheckDiff => "check.diff",
            Self::PresentationDiff => "pr.diff",
            Self::RawLedger => "raw-ledger.jsonl",
            Self::FullCheck => "check.json",
            Self::FindingIndex => "finding-index.json",
            Self::ReviewInput => "review-input.json",
            Self::PrJson => "repo-exposure.json",
            Self::PrMarkdown => "repo-exposure.md",
        }
    }

    pub(super) const fn ordinal(self) -> usize {
        match self {
            Self::OriginalRaw => 0,
            Self::CheckDiff => 1,
            Self::PresentationDiff => 2,
            Self::RawLedger => 3,
            Self::FullCheck => 4,
            Self::FindingIndex => 5,
            Self::ReviewInput => 6,
            Self::PrJson => 7,
            Self::PrMarkdown => 8,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct SubjectBinding {
    pub(super) requested_root: String,
    pub(super) requested_base: String,
    pub(super) requested_head: String,
    pub(super) invocation_repository: String,
    pub(super) logical_root: String,
    pub(super) work_tree: String,
    pub(super) base_commit: String,
    pub(super) head_commit: String,
    pub(super) base_tree: String,
    pub(super) head_tree: String,
    pub(super) origin_commit: String,
    pub(super) origin_tree: String,
    /// Parent-captured complete Git name-only inventory, including deletions
    /// and non-textual changes; ParsedDiff's source paths are not this set.
    pub(super) changed_paths: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct RawBinding {
    pub(super) raw_sha256: String,
    pub(super) ledger_sha256: String,
    pub(super) projection_sha256: String,
    pub(super) raw_bytes: u64,
    pub(super) ledger_bytes: u64,
    pub(super) records: u64,
    pub(super) sections: u64,
    pub(super) hunks: u64,
    pub(super) changed_files: u64,
    pub(super) added_lines: u64,
    pub(super) removed_lines: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct PresentationBinding {
    pub(super) bytes: u64,
    pub(super) sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct InventoryFile {
    pub(super) path: String,
    pub(super) git_mode: String,
    pub(super) blob_oid: String,
    pub(super) bytes: u64,
    pub(super) sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct InventoryBinding {
    pub(super) representation: String,
    pub(super) files: Vec<InventoryFile>,
    /// Includes the empty path for the root and authenticated empty directories.
    pub(super) directories: Vec<String>,
    pub(super) logical_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum ConfigurationBinding {
    Absent,
    Present {
        blob_oid: String,
        sha256: String,
        text: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(super) enum ProducerSurface {
    Installed,
    Xtask,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(super) enum CompleteMode {
    Instant,
    Draft,
    Fast,
    Deep,
    Ready,
}

impl CompleteMode {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Instant => "instant",
            Self::Draft => "draft",
            Self::Fast => "fast",
            Self::Deep => "deep",
            Self::Ready => "ready",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct PolicyField {
    pub(super) name: String,
    pub(super) value: String,
}

/// Exhaustive typed config projection, including fields excluded by the
/// existing finding-affecting fingerprint. The owning config model's closed
/// field enumerator enforces classification of future fields. This V1 capture
/// follows its complete current field inventory, including excluded fields.
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct FullConfiguration {
    pub(super) analysis_mode: Option<String>,
    pub(super) analysis_include_unchanged_tests: Option<bool>,
    pub(super) production_like_targets: Vec<String>,
    pub(super) test_harnesses: Vec<FullHarness>,
    pub(super) oracle_snapshot_strength: String,
    pub(super) oracle_mock_expectation_strength: String,
    pub(super) oracle_broad_error_strength: String,
    /// Order: exposed, weakly_exposed, reachable_unrevealed, no_static_path,
    /// infection_unknown, propagation_unknown, static_unknown.
    pub(super) finding_severity: [String; 7],
    /// Order: strongly_gripped, weakly_gripped, ungripped, reachable_unrevealed,
    /// activation_unknown, propagation_unknown, observation_unknown,
    /// discrimination_unknown, opaque, intentional, suppressed.
    pub(super) seam_severity: [String; 11],
    pub(super) lsp_seam_diagnostics: Option<bool>,
    pub(super) lsp_diagnostic_profile: Option<String>,
    pub(super) reports_max_related_tests: u64,
    pub(super) suppressions_path: String,
    pub(super) languages_enabled: Vec<String>,
    pub(super) rust_generated_file_patterns: Vec<String>,
    pub(super) rust_handwritten_files: Vec<String>,
    pub(super) bun_ub: Option<FullBunProfile>,
    pub(super) typescript_resolve_tsconfig_paths: bool,
    pub(super) perl: FullPerlConfiguration,
    pub(super) source_path: Option<String>,
    pub(super) source_text: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct FullHarness {
    pub(super) registration_id: String,
    pub(super) target: String,
    pub(super) kind: String,
    pub(super) adapter: String,
    pub(super) marker: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct FullBunProfile {
    pub(super) test_roots: Vec<String>,
    pub(super) bridge_hints: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct FullPerlConfiguration {
    pub(super) producer: Option<String>,
    pub(super) executable: Option<String>,
    pub(super) timeout_ms: u64,
    pub(super) cache_dir: Option<String>,
}

struct ConfigCopyBudget {
    bytes: u64,
    entries: u64,
    limit: u64,
    entry_limit: u64,
}
impl ConfigCopyBudget {
    fn text(&mut self, text: &str) -> Result<String, String> {
        self.entries = self
            .entries
            .checked_add(1)
            .filter(|n| *n <= self.entry_limit)
            .ok_or("full config entry bound exceeded")?;
        // JSON escaping is at most six bytes per original UTF8 byte; framing
        // is admitted before each retained clone. Native worker bounds own
        // allocator/container overhead, not this byte accounting.
        let framed = (text.len() as u64)
            .checked_mul(6)
            .and_then(|n| n.checked_add(128))
            .ok_or("config framed-byte overflow")?;
        self.bytes = self
            .bytes
            .checked_add(framed)
            .filter(|n| *n <= self.limit)
            .ok_or("full config framed-byte bound exceeded")?;
        Ok(text.to_string())
    }
    fn path(&mut self, path: &Path) -> Result<String, String> {
        self.text(path.to_str().ok_or("full config native path is not UTF8")?)
    }
    fn strings(&mut self, strings: &[String]) -> Result<Vec<String>, String> {
        strings.iter().map(|s| self.text(s)).collect()
    }
}

impl FullConfiguration {
    pub(super) fn capture(
        config: &crate::config::RiprConfig,
        limits: &CompleteVerificationLimits,
    ) -> Result<Self, String> {
        use crate::config::*;
        let RiprConfig {
            analysis,
            oracles,
            severity,
            lsp,
            reports,
            suppressions,
            languages,
            profiles,
            typescript,
            perl,
            source_path,
            source_text,
        } = config;
        // Nested type names are intentionally private to config::model. Their
        // public fields remain accessible through the actual inferred values.
        let mode = &analysis.mode;
        let include_unchanged_tests = &analysis.include_unchanged_tests;
        let production_like_targets = &analysis.production_like_targets;
        let test_harnesses = &analysis.test_harnesses;
        let OraclePolicy {
            snapshot_strength,
            mock_expectation_strength,
            broad_error_strength,
        } = oracles;
        let SeverityConfig { findings, seams } = severity;
        let finding_levels = [
            findings.exposed,
            findings.weakly_exposed,
            findings.reachable_unrevealed,
            findings.no_static_path,
            findings.infection_unknown,
            findings.propagation_unknown,
            findings.static_unknown,
        ];
        let seam_levels = [
            seams.strongly_gripped,
            seams.weakly_gripped,
            seams.ungripped,
            seams.reachable_unrevealed,
            seams.activation_unknown,
            seams.propagation_unknown,
            seams.observation_unknown,
            seams.discrimination_unknown,
            seams.opaque,
            seams.intentional,
            seams.suppressed,
        ];
        let seam_diagnostics = &lsp.seam_diagnostics;
        let diagnostic_profile = &lsp.diagnostic_profile;
        let max_related_tests = &reports.max_related_tests;
        let suppressions_path = &suppressions.path;
        let enabled = &languages.enabled;
        let rust = &languages.rust;
        let RustLanguageConfig {
            generated_file_patterns,
            handwritten_files,
        } = rust;
        let bun_ub = &profiles.bun_ub;
        let TypescriptConfig {
            resolve_tsconfig_paths,
        } = typescript;
        let PerlConfig {
            producer,
            executable,
            timeout_ms,
            cache_dir,
        } = perl;
        let mut budget = ConfigCopyBudget {
            bytes: 0,
            entries: 0,
            limit: limits.max_binding_bytes,
            entry_limit: limits.max_inventory_entries.saturating_add(128),
        };
        let analysis_mode = mode.as_ref().map(|m| budget.text(m.as_str())).transpose()?;
        let production_like_targets = production_like_targets
            .iter()
            .map(|p| budget.path(p))
            .collect::<Result<Vec<_>, _>>()?;
        let test_harnesses = test_harnesses
            .iter()
            .map(|h| {
                let TestHarnessRegistration {
                    registration_id,
                    target,
                    kind,
                    adapter,
                    marker,
                } = h;
                Ok(FullHarness {
                    registration_id: budget.text(registration_id)?,
                    target: budget.path(target)?,
                    kind: budget.text(kind.as_str())?,
                    adapter: budget.text(adapter.as_str())?,
                    marker: budget.text(marker)?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let finding_severity = finding_levels
            .into_iter()
            .map(|l| budget.text(l.as_str()))
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| "finding severity cardinality")?;
        let seam_severity = seam_levels
            .into_iter()
            .map(|l| budget.text(l.as_str()))
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| "seam severity cardinality")?;
        let bun_ub = bun_ub
            .as_ref()
            .map(|p| -> Result<FullBunProfile, String> {
                let test_roots = &p.test_roots;
                let bridge_hints = &p.bridge_hints;
                Ok(FullBunProfile {
                    test_roots: budget.strings(test_roots)?,
                    bridge_hints: budget.path(bridge_hints)?,
                })
            })
            .transpose()?;
        let full = Self {
            analysis_mode,
            analysis_include_unchanged_tests: *include_unchanged_tests,
            production_like_targets,
            test_harnesses,
            oracle_snapshot_strength: budget.text(snapshot_strength.as_str())?,
            oracle_mock_expectation_strength: budget.text(mock_expectation_strength.as_str())?,
            oracle_broad_error_strength: budget.text(broad_error_strength.as_str())?,
            finding_severity,
            seam_severity,
            lsp_seam_diagnostics: *seam_diagnostics,
            lsp_diagnostic_profile: diagnostic_profile
                .map(|p| budget.text(p.as_str()))
                .transpose()?,
            reports_max_related_tests: *max_related_tests as u64,
            suppressions_path: budget.path(suppressions_path)?,
            languages_enabled: enabled
                .iter()
                .map(|l| budget.text(l.as_str()))
                .collect::<Result<Vec<_>, _>>()?,
            rust_generated_file_patterns: budget.strings(generated_file_patterns)?,
            rust_handwritten_files: budget.strings(handwritten_files)?,
            bun_ub,
            typescript_resolve_tsconfig_paths: *resolve_tsconfig_paths,
            perl: FullPerlConfiguration {
                producer: producer.as_ref().map(|p| budget.text(p)).transpose()?,
                executable: executable.as_ref().map(|p| budget.path(p)).transpose()?,
                timeout_ms: *timeout_ms,
                cache_dir: cache_dir.as_ref().map(|p| budget.path(p)).transpose()?,
            },
            source_path: source_path.as_ref().map(|p| budget.path(p)).transpose()?,
            source_text: source_text.as_ref().map(|t| budget.text(t)).transpose()?,
        };
        serialized_digest(
            &full,
            limits.max_binding_bytes,
            b"ripr.complete_full_config.v1\0",
        )?;
        Ok(full)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct EffectiveOptions {
    pub(super) surface: ProducerSurface,
    pub(super) mode: CompleteMode,
    pub(super) include_unchanged_tests: bool,
    pub(super) enabled_languages: Vec<String>,
    /// Supplied canonical diff: no declared base, candidate subject, facts or
    /// suppression input. External effects require a successor contract.
    pub(super) git_timeout_ms: Option<u64>,
    pub(super) config_identity_version: u32,
    pub(super) config_identity_hash: String,
    pub(super) loaded_config_identity: Option<String>,
    pub(super) finding_affecting_fields: Vec<PolicyField>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct AnalyzerBinding {
    pub(super) build_identity: String,
    pub(super) check_schema: String,
    pub(super) analyzer_generation: String,
}

/// Explicit finite DATA copied from the caller's admitted profile. These
/// bounds are not a native-limit receipt and have no environment defaults.
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct CompleteVerificationLimits {
    pub(super) address_space_bytes: u64,
    pub(super) file_size_bytes: u64,
    pub(super) deadline_ms: u64,
    pub(super) max_manifest_bytes: u64,
    pub(super) max_binding_bytes: u64,
    pub(super) max_artifact_bytes: [u64; 9],
    pub(super) max_total_artifact_bytes: u64,
    /// Retained byte buffers, not Rust allocator overhead or an RSS claim.
    pub(super) max_buffered_bytes: u64,
    pub(super) max_relation_bytes: u64,
    pub(super) max_inventory_entries: u64,
    pub(super) max_inventory_bytes: u64,
    pub(super) max_source_bytes: u64,
    pub(super) file_limit: u64,
    pub(super) max_raw_records: u64,
    pub(super) max_retained_path_bytes: u64,
    pub(super) max_raw_projection_bytes: u64,
}

/// Saved policy data. The sealed core observation and this serializable
/// projection never grant execution permission or prove later consumption.
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(super) enum RustDependentScopePolicy {
    Auto,
    #[serde(rename = "named")]
    NameAdmitted,
    Full,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct RustExecutionPolicy {
    pub(super) changed_rust_line_limit: u64,
    pub(super) diff_index_file_limit: u64,
    pub(super) diff_narrow_index_limit: u64,
    pub(super) partial_diff_file_budget: u64,
    pub(super) partial_diff_line_budget: u64,
    pub(super) partial_budget_disclosures: Vec<String>,
    pub(super) partial_selection_version: String,
    pub(super) partial_language_tier_version: String,
    pub(super) dependent_scope: RustDependentScopePolicy,
}

impl RustExecutionPolicy {
    pub(super) fn capture() -> Result<Self, String> {
        let observed: crate::analysis::CompleteRustPolicySnapshot =
            crate::analysis::capture_complete_rust_policy()?;
        let number = |value: usize| {
            u64::try_from(value).map_err(|e| format!("Rust policy limit conversion: {e}"))
        };
        let disclosures = observed.partial_budget_disclosures();
        if disclosures.len() > 2
            || disclosures
                .iter()
                .any(|text| text.len() > 1024 || text.contains('\0'))
        {
            return Err("Rust policy disclosure admission exceeded".into());
        }
        let dependent_scope = match observed.dependent_scope_mode() {
            crate::analysis::CompleteDependentScopePolicy::Auto => RustDependentScopePolicy::Auto,
            crate::analysis::CompleteDependentScopePolicy::NameAdmitted => {
                RustDependentScopePolicy::NameAdmitted
            }
            crate::analysis::CompleteDependentScopePolicy::Full => RustDependentScopePolicy::Full,
        };
        let policy = Self {
            changed_rust_line_limit: number(observed.changed_rust_line_limit())?,
            diff_index_file_limit: number(observed.diff_index_file_limit())?,
            diff_narrow_index_limit: number(observed.diff_narrow_index_limit())?,
            partial_diff_file_budget: number(observed.partial_diff_file_budget())?,
            partial_diff_line_budget: number(observed.partial_diff_line_budget())?,
            partial_budget_disclosures: disclosures.to_vec(),
            partial_selection_version: observed.partial_selection_version().into(),
            partial_language_tier_version: observed.partial_language_tier_version().into(),
            dependent_scope,
        };
        policy.validate()?;
        Ok(policy)
    }

    fn validate(&self) -> Result<(), String> {
        if [
            self.changed_rust_line_limit,
            self.diff_index_file_limit,
            self.diff_narrow_index_limit,
            self.partial_diff_file_budget,
            self.partial_diff_line_budget,
        ]
        .contains(&0)
            || self.diff_narrow_index_limit > self.diff_index_file_limit
            || self.partial_diff_file_budget > self.diff_index_file_limit
            || self.partial_diff_line_budget > self.changed_rust_line_limit
        {
            return Err("invalid Rust execution policy limits or clamps".into());
        }
        // These are closed supported wire versions of the existing selectors.
        // Capturing a future selector version fails until the contract changes.
        if self.partial_selection_version != "partial-diff-v1"
            || self.partial_language_tier_version != "lang-tier-v1"
        {
            return Err("unsupported Rust partial-selection policy version".into());
        }
        if self.partial_budget_disclosures.len() > 2
            || self
                .partial_budget_disclosures
                .iter()
                .any(|text| text.is_empty() || text.len() > 1024 || text.contains('\0'))
        {
            return Err("invalid Rust policy disclosures".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct CompleteBinding {
    pub(super) schema_version: String,
    pub(super) subject: SubjectBinding,
    pub(super) committed_request: CommittedRequestBinding,
    pub(super) raw: RawBinding,
    /// Original bounded three-context capture, pinned by the caller before
    /// payload generation. It cannot be reconstructed from zero-context raw.
    pub(super) presentation: PresentationBinding,
    pub(super) inventory: InventoryBinding,
    pub(super) configuration: ConfigurationBinding,
    pub(super) full_configuration: FullConfiguration,
    pub(super) rust_execution_policy: RustExecutionPolicy,
    pub(super) effective_options: EffectiveOptions,
    pub(super) analyzer: AnalyzerBinding,
    pub(super) nonce: String,
    pub(super) profile: CompleteVerificationLimits,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct ArtifactDescriptor {
    pub(super) role: ArtifactRole,
    pub(super) path: String,
    pub(super) bytes: u64,
    pub(super) sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct CompleteManifest {
    pub(super) schema_version: String,
    pub(super) generation_id: String,
    pub(super) binding: CompleteBinding,
    pub(super) artifacts: Vec<ArtifactDescriptor>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CompletePayload<T> {
    pub(super) schema_version: String,
    pub(super) generation_id: String,
    pub(super) role: ArtifactRole,
    pub(super) value: T,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CompleteCheck {
    /// Observed producer data; equality with caller expectations is required.
    /// This assertion alone never proves the analyzer consumed those options.
    pub(super) effective_options: EffectiveOptions,
    pub(super) check: serde_json::Value,
}

pub(super) fn markdown_prefix(generation_id: &str) -> String {
    format!("<!-- ripr.complete_payload.v1 pr_markdown {generation_id} -->\n")
}

impl CompleteVerificationLimits {
    pub(super) fn validate(&self) -> Result<(), String> {
        if self.address_space_bytes == 0
            || self.address_space_bytes > 2 * 1024 * 1024 * 1024
            || self.file_size_bytes == 0
            || self.file_size_bytes > 256 * 1024 * 1024
            || self.deadline_ms == 0
        {
            return Err("invalid finite native-profile data".into());
        }
        for limit in [
            self.max_manifest_bytes,
            self.max_binding_bytes,
            self.max_total_artifact_bytes,
            self.max_buffered_bytes,
            self.max_relation_bytes,
            self.max_inventory_bytes,
            self.max_source_bytes,
            self.max_retained_path_bytes,
            self.max_raw_projection_bytes,
        ] {
            if limit == 0 || usize::try_from(limit).is_err() {
                return Err("invalid finite byte bound".into());
            }
        }
        for cap in self.max_artifact_bytes {
            if cap == 0 || cap > self.file_size_bytes || usize::try_from(cap).is_err() {
                return Err("invalid artifact byte bound".into());
            }
        }
        if self.max_manifest_bytes > self.file_size_bytes
            || self.max_artifact_bytes[ArtifactRole::FindingIndex.ordinal()] > 2 * 1024 * 1024
            || self.max_artifact_bytes[ArtifactRole::ReviewInput.ordinal()] > 128 * 1024
        {
            return Err("compact artifact bound exceeds existing consumer limit".into());
        }
        for count in [
            self.max_inventory_entries,
            self.file_limit,
            self.max_raw_records,
        ] {
            usize::try_from(count).map_err(|_| "count bound exceeds native usize")?;
        }
        Ok(())
    }
}

impl CompleteBinding {
    pub(super) fn validate(&self) -> Result<(), String> {
        self.profile.validate()?;
        self.committed_request.validate()?;
        self.rust_execution_policy.validate()?;
        if self.schema_version != BINDING_SCHEMA {
            return Err("unsupported complete binding schema".into());
        }
        if self.nonce.len() != 32 || !lower_hex(&self.nonce) {
            return Err("invalid generation nonce".into());
        }
        let s = &self.subject;
        for literal in [&s.requested_root, &s.requested_base, &s.requested_head] {
            if literal.is_empty() || literal.len() > 4096 || literal.contains(['\0', '\n', '\r']) {
                return Err("invalid original request literal".into());
            }
        }
        for root in [&s.invocation_repository, &s.logical_root, &s.work_tree] {
            if !Path::new(root).is_absolute()
                || root.contains('\0')
                || Path::new(root)
                    .components()
                    .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
            {
                return Err("invalid absolute native root identity".into());
            }
        }
        if s.logical_root != s.work_tree {
            return Err("whole-head-v1 requires repository-root analysis".into());
        }
        if !Path::new(&s.logical_root).starts_with(&s.work_tree)
            || !Path::new(&s.invocation_repository).starts_with(&s.work_tree)
        {
            return Err("root lies outside bound work tree".into());
        }
        if s.changed_paths.len() as u64 > self.profile.max_inventory_entries
            || s.changed_paths.windows(2).any(|w| w[0] >= w[1])
        {
            return Err("noncanonical changed-path inventory".into());
        }
        for path in &s.changed_paths {
            validate_relative(path)?;
        }
        let width = s.base_commit.len();
        validate_oid(&self.committed_request.blob_oid, width)?;
        for oid in [
            &s.base_commit,
            &s.head_commit,
            &s.base_tree,
            &s.head_tree,
            &s.origin_commit,
            &s.origin_tree,
        ] {
            validate_oid(oid, width)?;
        }
        for digest in [
            &self.raw.raw_sha256,
            &self.raw.ledger_sha256,
            &self.raw.projection_sha256,
        ] {
            validate_sha256(digest)?;
        }
        validate_sha256(&self.presentation.sha256)?;
        if self.presentation.bytes > self.profile.max_artifact_bytes[2] {
            return Err("presentation input exceeds admitted byte bound".into());
        }
        if self.raw.raw_bytes > self.profile.max_artifact_bytes[0]
            || self.raw.ledger_bytes > self.profile.max_artifact_bytes[3]
            || self.raw.records > self.profile.max_raw_records
            || self.raw.changed_files > self.profile.file_limit
        {
            return Err("raw binding exceeds admitted bounds".into());
        }
        self.raw
            .added_lines
            .checked_add(self.raw.removed_lines)
            .ok_or("raw side total overflow")?;
        validate_inventory(&self.inventory, &self.profile, width)?;
        let request = &self.committed_request;
        let request_file = self
            .inventory
            .files
            .iter()
            .find(|file| file.path == POLICY_PATH)
            .ok_or("committed request is missing from full inventory")?;
        if request_file.git_mode != request.git_mode
            || request_file.blob_oid != request.blob_oid
            || request_file.bytes != request.original_bytes.len() as u64
            || request_file.sha256 != request.sha256
        {
            return Err("committed request disagrees with admitted inventory".into());
        }
        match &self.configuration {
            ConfigurationBinding::Absent => {
                if self.full_configuration.source_path.is_some()
                    || self.full_configuration.source_text.is_some()
                {
                    return Err("absent config carries loaded provenance".into());
                }
                if self.inventory.files.iter().any(|f| f.path == "ripr.toml") {
                    return Err("absent config contradicts inventory".into());
                }
            }
            ConfigurationBinding::Present {
                blob_oid,
                sha256,
                text,
            } => {
                validate_oid(blob_oid, width)?;
                validate_sha256(sha256)?;
                if sha256_bytes(text.as_bytes()) != *sha256 {
                    return Err("config text digest mismatch".into());
                }
                let file = self
                    .inventory
                    .files
                    .iter()
                    .find(|f| f.path == "ripr.toml")
                    .ok_or("config is missing from full inventory")?;
                if file.blob_oid != *blob_oid
                    || file.sha256 != *sha256
                    || file.bytes != text.len() as u64
                {
                    return Err("config disagrees with admitted inventory".into());
                }
                let path = Path::new(&s.logical_root).join("ripr.toml");
                if self.full_configuration.source_text.as_deref() != Some(text)
                    || self.full_configuration.source_path.as_deref() != path.to_str()
                {
                    return Err(
                        "full config original text/source path differs from captured configuration"
                            .into(),
                    );
                }
            }
        }
        let options = &self.effective_options;
        validate_config_identity(&options.config_identity_hash)?;
        if let Some(identity) = &options.loaded_config_identity {
            validate_config_identity(identity)?;
        }
        if options.git_timeout_ms == Some(0) {
            return Err("zero optional Git deadline".into());
        }
        if options.enabled_languages.is_empty()
            || options.enabled_languages.windows(2).any(|w| w[0] >= w[1])
            || options
                .enabled_languages
                .iter()
                .any(|s| !matches!(s.as_str(), "rust" | "typescript" | "javascript" | "python"))
            || options
                .finding_affecting_fields
                .windows(2)
                .any(|w| w[0].name >= w[1].name)
        {
            return Err("noncanonical or unsupported effective policy".into());
        }
        if self.analyzer.build_identity != crate::build_identity::cache_identity()
            || self.analyzer.check_schema != crate::app::CHECK_OUTPUT_SCHEMA_VERSION
            || self.analyzer.analyzer_generation != crate::review_input::REVIEW_ANALYZER_GENERATION
        {
            return Err("stale analyzer/build identity".into());
        }
        serialized_digest(
            self,
            self.profile.max_binding_bytes,
            b"ripr.complete_binding.validation.v1\0",
        )?;
        Ok(())
    }

    pub(super) fn generation_id(&self) -> Result<String, String> {
        self.validate()?;
        serialized_digest(
            self,
            self.profile.max_binding_bytes,
            b"ripr.complete_generation.v1\0",
        )
    }
}

impl CompleteManifest {
    pub(super) fn validate(
        &self,
        expected: &CompleteBinding,
        limits: &CompleteVerificationLimits,
    ) -> Result<(), String> {
        if &expected.profile != limits {
            return Err("verification profile differs from admitted binding".into());
        }
        expected.validate()?;
        if self.schema_version != MANIFEST_SCHEMA
            || &self.binding != expected
            || self.generation_id != expected.generation_id()?
        {
            return Err("stale or contradictory complete manifest".into());
        }
        if self.artifacts.len() != ArtifactRole::ALL.len() {
            return Err("complete manifest must have exactly nine roles".into());
        }
        let mut total = 0u64;
        for (descriptor, role) in self.artifacts.iter().zip(ArtifactRole::ALL) {
            if descriptor.role != role || descriptor.path != role.path() {
                return Err("missing, duplicate, unknown or overlapping artifact role/path".into());
            }
            validate_sha256(&descriptor.sha256)?;
            if descriptor.bytes > limits.max_artifact_bytes[role.ordinal()] {
                return Err("artifact exceeds its admitted byte bound".into());
            }
            total = total
                .checked_add(descriptor.bytes)
                .ok_or("artifact byte total overflow")?;
        }
        if total > limits.max_total_artifact_bytes {
            return Err("aggregate artifact byte bound exceeded".into());
        }
        if self.artifacts[0].bytes != expected.raw.raw_bytes
            || self.artifacts[0].sha256 != expected.raw.raw_sha256
            || self.artifacts[3].bytes != expected.raw.ledger_bytes
            || self.artifacts[3].sha256 != expected.raw.ledger_sha256
        {
            return Err("manifest raw/ledger identities differ from input binding".into());
        }
        if self.artifacts[2].bytes != expected.presentation.bytes
            || self.artifacts[2].sha256 != expected.presentation.sha256
        {
            return Err("presentation capture identity differs from caller binding".into());
        }
        Ok(())
    }
}

fn validate_inventory(
    inventory: &InventoryBinding,
    limits: &CompleteVerificationLimits,
    oid_width: usize,
) -> Result<(), String> {
    if inventory.representation != INVENTORY_REPRESENTATION {
        return Err("unsupported inventory native representation".into());
    }
    let entries = inventory
        .files
        .len()
        .checked_add(inventory.directories.len())
        .ok_or("inventory count overflow")?;
    if entries as u64 > limits.max_inventory_entries {
        return Err("inventory entry bound exceeded".into());
    }
    if inventory.directories.first().map(String::as_str) != Some("") {
        return Err("inventory root directory is missing".into());
    }
    if inventory
        .files
        .windows(2)
        .any(|w| Path::new(&w[0].path) >= Path::new(&w[1].path))
        || inventory
            .directories
            .windows(2)
            .any(|w| Path::new(&w[0]) >= Path::new(&w[1]))
    {
        return Err("inventory ordering or uniqueness differs from native owner".into());
    }
    let dirs = inventory
        .directories
        .iter()
        .map(|p| Path::new(p))
        .collect::<std::collections::BTreeSet<_>>();
    let mut bytes = 0u64;
    for directory in &inventory.directories {
        if directory.is_empty() {
            continue;
        }
        validate_relative(directory)?;
        if !dirs.contains(
            Path::new(directory)
                .parent()
                .ok_or("directory lacks parent")?,
        ) {
            return Err("inventory directory ancestor is missing".into());
        }
    }
    for file in &inventory.files {
        validate_relative(&file.path)?;
        if !matches!(file.git_mode.as_str(), "100644" | "100755") {
            return Err("unsupported original Git file mode".into());
        }
        validate_oid(&file.blob_oid, oid_width)?;
        validate_sha256(&file.sha256)?;
        if dirs.contains(Path::new(&file.path))
            || !dirs.contains(Path::new(&file.path).parent().ok_or("file lacks parent")?)
        {
            return Err("inventory file/directory overlap or missing parent".into());
        }
        bytes = bytes
            .checked_add(file.bytes)
            .ok_or("inventory logical byte total overflow")?;
    }
    if bytes != inventory.logical_bytes || bytes > limits.max_source_bytes {
        return Err("inventory logical byte total mismatch/overflow".into());
    }
    serialized_digest(
        inventory,
        limits.max_inventory_bytes,
        b"ripr.complete_inventory.v1\0",
    )?;
    Ok(())
}

fn validate_relative(path: &str) -> Result<(), String> {
    if path.is_empty()
        || path.contains('\0')
        || path.split('/').any(|s| matches!(s, "" | "." | ".."))
        || Path::new(path)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("invalid native inventory relative path".into());
    }
    Ok(())
}

fn lower_hex(value: &str) -> bool {
    value
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn validate_oid(value: &str, width: usize) -> Result<(), String> {
    let oid = crate::domain::GitObjectId::parse(value).map_err(|e| e.to_string())?;
    if value.len() != width || value != oid.as_str() {
        return Err("noncanonical or mixed-format Git object ID".into());
    }
    Ok(())
}
pub(super) fn validate_sha256(value: &str) -> Result<(), String> {
    if value.len() != 71 || !value.starts_with("sha256:") || !lower_hex(&value[7..]) {
        return Err("invalid canonical SHA256 identity".into());
    }
    Ok(())
}
fn validate_config_identity(value: &str) -> Result<(), String> {
    if value.len() != 24 || !value.starts_with("fnv1a64:") || !lower_hex(&value[8..]) {
        return Err("invalid existing configuration fingerprint".into());
    }
    Ok(())
}
pub(super) fn sha256_bytes(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

struct HashWriter {
    bytes: u64,
    limit: u64,
    hash: Sha256,
}
impl Write for HashWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes = self
            .bytes
            .checked_add(bytes.len() as u64)
            .filter(|n| *n <= self.limit)
            .ok_or_else(|| io::Error::other("canonical data byte bound exceeded"))?;
        self.hash.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
pub(super) fn serialized_digest<T: Serialize + ?Sized>(
    value: &T,
    limit: u64,
    domain: &[u8],
) -> Result<String, String> {
    let mut writer = HashWriter {
        bytes: 0,
        limit,
        hash: Sha256::new(),
    };
    writer.hash.update(domain);
    serde_json::to_writer(&mut writer, value)
        .map_err(|e| format!("bounded canonical data: {e}"))?;
    Ok(format!("sha256:{:x}", writer.hash.finalize()))
}

/// Bounded DATA serialization for the root-owned producer. No filesystem,
/// execution, atomic publication, final authority or capability is minted.
pub(super) fn encode_payload<T: Serialize>(
    role: ArtifactRole,
    generation_id: &str,
    value: &T,
    limits: &CompleteVerificationLimits,
) -> Result<Vec<u8>, String> {
    limits.validate()?;
    validate_sha256(generation_id)?;
    if !matches!(
        role,
        ArtifactRole::FullCheck
            | ArtifactRole::FindingIndex
            | ArtifactRole::ReviewInput
            | ArtifactRole::PrJson
    ) {
        return Err("raw/diff/ledger/Markdown roles cannot use JSON envelopes".into());
    }
    serialize_bounded(
        &CompletePayload {
            schema_version: PAYLOAD_SCHEMA.to_string(),
            generation_id: generation_id.to_string(),
            role,
            value,
        },
        limits.max_artifact_bytes[role.ordinal()],
    )
}

pub(super) fn encode_manifest(
    manifest: &CompleteManifest,
    expected: &CompleteBinding,
    limits: &CompleteVerificationLimits,
) -> Result<Vec<u8>, String> {
    manifest.validate(expected, limits)?;
    serialize_bounded(manifest, limits.max_manifest_bytes)
}

fn serialize_bounded<T: Serialize>(value: &T, limit: u64) -> Result<Vec<u8>, String> {
    struct Bytes {
        bytes: Vec<u8>,
        limit: u64,
    }
    impl Write for Bytes {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            let length = self
                .bytes
                .len()
                .checked_add(bytes.len())
                .filter(|n| *n as u64 <= self.limit)
                .ok_or_else(|| io::Error::other("canonical output byte bound exceeded"))?;
            if length > self.bytes.capacity() {
                let target = length
                    .max(self.bytes.capacity().saturating_mul(2))
                    .min(usize::try_from(self.limit).map_err(io::Error::other)?);
                self.bytes
                    .try_reserve_exact(target - self.bytes.len())
                    .map_err(io::Error::other)?;
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut writer = Bytes {
        bytes: Vec::new(),
        limit,
    };
    serde_json::to_writer(&mut writer, value)
        .map_err(|e| format!("bounded complete serialization: {e}"))?;
    Ok(writer.bytes)
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    pub(in crate::app::pr_evidence) fn require_error<T, E>(
        result: Result<T, E>,
        accepted: &str,
    ) -> Result<E, String> {
        match result {
            Err(error) => Ok(error),
            Ok(_) => Err(accepted.into()),
        }
    }


    pub(in crate::app::pr_evidence) fn fixture_policy_bytes() -> &'static [u8] {
        br#"{"schema_version":"ripr.complete_request.v1","request":"complete","profile":"whole-head-v1"}"#
    }

    pub(in crate::app::pr_evidence) fn fixture_binding() -> Result<CompleteBinding, String> {
        let committed_request = CommittedRequestBinding::from_original_blob(
            &"6".repeat(40),
            fixture_policy_bytes().to_vec(),
        )?;
        let inventory = InventoryBinding {
            representation: INVENTORY_REPRESENTATION.into(),
            files: vec![InventoryFile {
                path: committed_request.path.clone(),
                git_mode: committed_request.git_mode.clone(),
                blob_oid: committed_request.blob_oid.clone(),
                bytes: committed_request.original_bytes.len() as u64,
                sha256: committed_request.sha256.clone(),
            }],
            directories: vec!["".into(), ".ripr".into()],
            logical_bytes: committed_request.original_bytes.len() as u64,
        };
        Ok(CompleteBinding {
            schema_version: BINDING_SCHEMA.into(),
            subject: SubjectBinding {
                requested_root: ".".into(),
                requested_base: "base".into(),
                requested_head: "HEAD".into(),
                invocation_repository: "/repo".into(),
                logical_root: "/repo".into(),
                work_tree: "/repo".into(),
                base_commit: "1".repeat(40),
                head_commit: "2".repeat(40),
                base_tree: "3".repeat(40),
                head_tree: "4".repeat(40),
                origin_commit: "1".repeat(40),
                origin_tree: "3".repeat(40),
                changed_paths: vec![],
            },
            committed_request,
            raw: RawBinding {
                raw_sha256: sha256_bytes(b""),
                ledger_sha256: sha256_bytes(b"ledger"),
                projection_sha256: sha256_bytes(b"projection"),
                raw_bytes: 0,
                ledger_bytes: 6,
                records: 0,
                sections: 0,
                hunks: 0,
                changed_files: 0,
                added_lines: 0,
                removed_lines: 0,
            },
            presentation: PresentationBinding {
                bytes: 0,
                sha256: sha256_bytes(b""),
            },
            inventory,
            configuration: ConfigurationBinding::Absent,
            full_configuration: FullConfiguration {
                analysis_mode: None,
                analysis_include_unchanged_tests: None,
                production_like_targets: vec![],
                test_harnesses: vec![],
                oracle_snapshot_strength: "medium".into(),
                oracle_mock_expectation_strength: "medium".into(),
                oracle_broad_error_strength: "weak".into(),
                finding_severity: std::array::from_fn(|_| "warning".into()),
                seam_severity: std::array::from_fn(|_| "warning".into()),
                lsp_seam_diagnostics: Some(true),
                lsp_diagnostic_profile: None,
                reports_max_related_tests: 5,
                suppressions_path: ".ripr/suppressions.toml".into(),
                languages_enabled: vec!["rust".into()],
                rust_generated_file_patterns: vec![],
                rust_handwritten_files: vec![],
                bun_ub: None,
                typescript_resolve_tsconfig_paths: false,
                perl: FullPerlConfiguration {
                    producer: None,
                    executable: None,
                    timeout_ms: 1000,
                    cache_dir: None,
                },
                source_path: None,
                source_text: None,
            },
            rust_execution_policy: RustExecutionPolicy::capture()?,
            effective_options: EffectiveOptions {
                surface: ProducerSurface::Installed,
                mode: CompleteMode::Draft,
                include_unchanged_tests: true,
                enabled_languages: vec!["rust".into()],
                git_timeout_ms: None,
                config_identity_version: crate::config::CHECK_ARTIFACT_CONFIG_IDENTITY_VERSION,
                config_identity_hash: crate::config::config_fingerprint("config"),
                loaded_config_identity: None,
                finding_affecting_fields: vec![],
            },
            analyzer: AnalyzerBinding {
                build_identity: crate::build_identity::cache_identity().into(),
                check_schema: crate::app::CHECK_OUTPUT_SCHEMA_VERSION.into(),
                analyzer_generation: crate::review_input::REVIEW_ANALYZER_GENERATION.into(),
            },
            nonce: "a".repeat(32),
            profile: CompleteVerificationLimits {
                address_space_bytes: 128 * 1024 * 1024,
                file_size_bytes: 1024 * 1024,
                deadline_ms: 1000,
                max_manifest_bytes: 128 * 1024,
                max_binding_bytes: 64 * 1024,
                max_artifact_bytes: [128 * 1024; 9],
                max_total_artifact_bytes: 1024 * 1024,
                max_buffered_bytes: 64 * 1024 * 1024,
                max_relation_bytes: 2 * 1024 * 1024,
                max_inventory_entries: 100,
                max_inventory_bytes: 32 * 1024,
                max_source_bytes: 1024 * 1024,
                file_limit: 100,
                max_raw_records: 1000,
                max_retained_path_bytes: 32 * 1024,
                max_raw_projection_bytes: 128 * 1024,
            },
        })
    }

    pub(in crate::app::pr_evidence) fn fixture_manifest(
        binding: CompleteBinding,
    ) -> Result<CompleteManifest, String> {
        let mut artifacts = ArtifactRole::ALL
            .iter()
            .map(|role| ArtifactDescriptor {
                role: *role,
                path: role.path().into(),
                bytes: 0,
                sha256: sha256_bytes(b""),
            })
            .collect::<Vec<_>>();
        artifacts[3].bytes = binding.raw.ledger_bytes;
        artifacts[3].sha256.clone_from(&binding.raw.ledger_sha256);
        Ok(CompleteManifest {
            schema_version: MANIFEST_SCHEMA.into(),
            generation_id: binding.generation_id()?,
            binding,
            artifacts,
        })
    }

    #[test]
    fn configured_external_perl_route_is_explicitly_refused() -> Result<(), String> {
        let mut binding = fixture_binding()?;
        binding.validate()?;
        binding.effective_options.enabled_languages = vec!["perl".into(), "rust".into()];
        let error = require_error(
            binding.validate(),
            "configured external Perl route was accepted",
        )?;
        assert_eq!(error, "noncanonical or unsupported effective policy");
        // Configuration remains exhaustive data, including inactive Perl
        // settings; their presence cannot expand the admitted language route.
        binding.effective_options.enabled_languages = vec!["rust".into()];
        binding.full_configuration.perl.executable = Some("/external/perl".into());
        binding.validate()?;
        Ok(())
    }

    #[test]
    fn closed_nine_roles_and_no_unknown_or_missing_schema_fields() -> Result<(), String> {
        let binding = fixture_binding()?;
        let manifest = fixture_manifest(binding.clone())?;
        manifest.validate(&binding, &binding.profile)?;
        for index in 0..9 {
            let mut wrong = manifest.clone();
            wrong.artifacts.remove(index);
            let error = require_error(
                wrong.validate(&binding, &binding.profile),
                "missing artifact role was accepted",
            )?;
            assert_eq!(error, "complete manifest must have exactly nine roles");
            let mut wrong = manifest.clone();
            wrong.artifacts[index].role = ArtifactRole::OriginalRaw;
            wrong.artifacts[index].path = "alias".into();
            let error = require_error(
                wrong.validate(&binding, &binding.profile),
                "aliased artifact role/path was accepted",
            )?;
            assert_eq!(
                error,
                "missing, duplicate, unknown or overlapping artifact role/path"
            );
        }
        let mut value = serde_json::to_value(&manifest).map_err(|e| e.to_string())?;
        value["binding"]["subject"]["execution_permission"] = serde_json::json!(true);
        let _error = require_error(
            serde_json::from_value::<CompleteManifest>(value),
            "unexpected success in closed_nine_roles_and_no_unknown_or_missing_schema_fields",
        )?;
        let mut value = serde_json::to_value(&manifest).map_err(|e| e.to_string())?;
        value["artifacts"][0]["role"] = serde_json::json!("unknown");
        let _error = require_error(
            serde_json::from_value::<CompleteManifest>(value),
            "unexpected success in closed_nine_roles_and_no_unknown_or_missing_schema_fields",
        )?;
        let mut value = serde_json::to_value(&binding).map_err(|e| e.to_string())?;
        value
            .as_object_mut()
            .ok_or("object")?
            .remove("schema_version");
        let _error = require_error(
            serde_json::from_value::<CompleteBinding>(value),
            "unexpected success in closed_nine_roles_and_no_unknown_or_missing_schema_fields",
        )?;
        Ok(())
    }

    #[test]
    fn generation_binds_same_tree_commit_literals_mode_include_languages_and_nonce()
    -> Result<(), String> {
        let binding = fixture_binding()?;
        let id = binding.generation_id()?;
        for change in 0..6 {
            let mut other = binding.clone();
            match change {
                0 => other.subject.head_commit = "5".repeat(40),
                1 => other.subject.requested_head = "alias".into(),
                2 => other.effective_options.mode = CompleteMode::Deep,
                3 => other.effective_options.include_unchanged_tests = false,
                4 => {
                    other.effective_options.enabled_languages = vec!["python".into(), "rust".into()]
                }
                _ => other.nonce = "b".repeat(32),
            }
            assert_ne!(other.generation_id()?, id);
            let _error = require_error(
                fixture_manifest(other)?.validate(&binding, &binding.profile),
                "unexpected success in generation_binds_same_tree_commit_literals_mode_include_languages_and_nonce",
            )?;
        }
        let mut manifest = fixture_manifest(binding.clone())?;
        let prior = serialized_digest(&manifest, binding.profile.max_manifest_bytes, b"")?;
        manifest.artifacts[8].sha256 = sha256_bytes(b"changed markdown");
        assert_eq!(manifest.generation_id, id);
        assert_ne!(
            serialized_digest(&manifest, binding.profile.max_manifest_bytes, b"")?,
            prior
        );
        Ok(())
    }

    #[test]
    fn inventory_config_order_paths_modes_counts_and_caps_refuse() -> Result<(), String> {
        let mut binding = fixture_binding()?;
        binding.inventory.files.push(InventoryFile {
            path: "ripr.toml".into(),
            git_mode: "100644".into(),
            blob_oid: "6".repeat(40),
            bytes: 1,
            sha256: sha256_bytes(b"#"),
        });
        binding.inventory.logical_bytes += 1;
        binding.configuration = ConfigurationBinding::Present {
            blob_oid: "6".repeat(40),
            sha256: sha256_bytes(b"#"),
            text: "#".into(),
        };
        binding.full_configuration.source_path = Some("/repo/ripr.toml".into());
        binding.full_configuration.source_text = Some("#".into());
        binding.validate()?;
        for change in 0..7 {
            let mut wrong = binding.clone();
            match change {
                0 => wrong.configuration = ConfigurationBinding::Absent,
                1 => wrong.inventory.files[0].path = "../ripr.toml".into(),
                2 => wrong.inventory.files[0].git_mode = "120000".into(),
                3 => wrong.inventory.files.push(wrong.inventory.files[0].clone()),
                4 => wrong.inventory.logical_bytes = u64::MAX,
                5 => wrong.profile.max_inventory_bytes = 1,
                _ => wrong.profile.max_binding_bytes = 1,
            }
            let _error = require_error(
                wrong.validate(),
                "unexpected success in inventory_config_order_paths_modes_counts_and_caps_refuse",
            )?;
        }
        Ok(())
    }

    #[test]
    fn aggregate_overflow_profile_mismatch_and_payload_marker_removal_refuse() -> Result<(), String>
    {
        let binding = fixture_binding()?;
        let mut manifest = fixture_manifest(binding.clone())?;
        manifest.artifacts[8].bytes = u64::MAX;
        let _error = require_error(
            manifest.validate(&binding, &binding.profile),
            "unexpected success in aggregate_overflow_profile_mismatch_and_payload_marker_removal_refuse",
        )?;
        let mut limits = binding.profile.clone();
        limits.deadline_ms += 1;
        let error = require_error(
            fixture_manifest(binding.clone())?.validate(&binding, &limits),
            "verification profile mismatch was accepted",
        )?;
        assert_eq!(error, "verification profile differs from admitted binding");
        let _error = require_error(
            serde_json::from_str::<CompletePayload<serde_json::Value>>("{\"findings\":[]}"),
            "unexpected success in aggregate_overflow_profile_mismatch_and_payload_marker_removal_refuse",
        )?;
        let _error = require_error(
            serialized_digest(&vec!["oversized"; 100], 1, b""),
            "unexpected success in aggregate_overflow_profile_mismatch_and_payload_marker_removal_refuse",
        )?;
        Ok(())
    }
    #[test]
    fn committed_request_is_required_closed_inventory_bound_and_generation_bound()
    -> Result<(), String> {
        let binding = fixture_binding()?;
        binding.validate()?;
        let mut missing = serde_json::to_value(&binding).map_err(|error| error.to_string())?;
        missing
            .as_object_mut()
            .ok_or("binding is not an object")?
            .remove("committed_request");
        let error = require_error(
            serde_json::from_value::<CompleteBinding>(missing),
            "missing committed request was accepted",
        )?;
        assert!(error.to_string().contains("missing field `committed_request`"));
        let mut unknown = serde_json::to_value(&binding).map_err(|error| error.to_string())?;
        unknown["committed_request"]["grant"] = serde_json::json!(true);
        let error = require_error(
            serde_json::from_value::<CompleteBinding>(unknown),
            "unknown committed request field was accepted",
        )?;
        assert!(error.to_string().contains("unknown field `grant`"));
        let mut stale = binding.clone();
        stale.schema_version = "ripr.complete_binding.v2".into();
        assert_eq!(
            require_error(stale.validate(), "stale binding v2 was accepted")?,
            "unsupported complete binding schema"
        );
        let mut missing = binding.clone();
        missing.inventory.files.clear();
        missing.inventory.logical_bytes = 0;
        assert_eq!(
            require_error(
                missing.validate(),
                "request absent from inventory was accepted",
            )?,
            "committed request is missing from full inventory"
        );
        for change in 0..4 {
            let mut wrong = binding.clone();
            let file = wrong
                .inventory
                .files
                .first_mut()
                .ok_or("request file missing")?;
            match change {
                0 => file.git_mode = "100755".into(),
                1 => file.blob_oid = "7".repeat(40),
                2 => file.bytes += 1,
                _ => file.sha256 = sha256_bytes(b"stale request"),
            }
            wrong.inventory.logical_bytes = wrong.inventory.files.iter().map(|file| file.bytes).sum();
            assert_eq!(
                require_error(
                    wrong.validate(),
                    "request inventory mismatch was accepted",
                )?,
                "committed request disagrees with admitted inventory"
            );
        }
        // Semantically equivalent policy JSON is still distinct original input.
        let mut equivalent = binding.clone();
        let mut original_bytes = equivalent.committed_request.original_bytes.clone();
        original_bytes.push(b'\n');
        equivalent.committed_request = CommittedRequestBinding::from_original_blob(
            &equivalent.committed_request.blob_oid,
            original_bytes,
        )?;
        let file = equivalent
            .inventory
            .files
            .first_mut()
            .ok_or("request file missing")?;
        file.bytes = equivalent.committed_request.original_bytes.len() as u64;
        file.sha256.clone_from(&equivalent.committed_request.sha256);
        equivalent.inventory.logical_bytes = file.bytes;
        equivalent.validate()?;
        assert_eq!(
            equivalent.committed_request.effective_request,
            binding.committed_request.effective_request
        );
        assert_ne!(equivalent.generation_id()?, binding.generation_id()?);
        Ok(())
    }

    #[test]
    fn whole_head_request_requires_repository_root_without_rewriting_literals()
    -> Result<(), String> {
        let binding = fixture_binding()?;
        let mut subroot = binding.clone();
        subroot.subject.logical_root = "/repo/sub".into();
        assert_eq!(
            require_error(subroot.validate(), "whole-head subroot was accepted")?,
            "whole-head-v1 requires repository-root analysis"
        );
        let mut nested_invocation = binding.clone();
        nested_invocation.subject.invocation_repository = "/repo/nested".into();
        nested_invocation.subject.requested_root = "..".into();
        nested_invocation.validate()?;
        assert_eq!(nested_invocation.subject.requested_root, "..");
        assert_eq!(nested_invocation.subject.logical_root, "/repo");
        Ok(())
    }

    #[test]
    fn rust_execution_policy_is_required_closed_bounded_and_generation_bound()
    -> Result<(), String> {
        let binding = fixture_binding()?;
        binding.validate()?;
        let mut missing = serde_json::to_value(&binding).map_err(|e| e.to_string())?;
        missing
            .as_object_mut()
            .ok_or("binding is not an object")?
            .remove("rust_execution_policy");
        let error = require_error(
            serde_json::from_value::<CompleteBinding>(missing),
            "missing Rust policy field was accepted",
        )?;
        assert!(error.to_string().contains("missing field `rust_execution_policy`"));
        let mut unknown = serde_json::to_value(&binding).map_err(|e| e.to_string())?;
        unknown["rust_execution_policy"]["grant"] = serde_json::json!(true);
        let error = require_error(
            serde_json::from_value::<CompleteBinding>(unknown),
            "unknown Rust policy field was accepted",
        )?;
        assert!(error.to_string().contains("unknown field `grant`"));
        let mut unknown = serde_json::to_value(&binding).map_err(|e| e.to_string())?;
        unknown["rust_execution_policy"]["dependent_scope"] = serde_json::json!("core_only");
        let error = require_error(
            serde_json::from_value::<CompleteBinding>(unknown),
            "unsupported dependent scope was accepted",
        )?;
        assert!(error.to_string().contains("unknown variant `core_only`"));
        for change in 0..7 {
            let mut wrong = binding.clone();
            match change {
                0 => wrong.schema_version = "ripr.complete_binding.v1".into(),
                1 => wrong.rust_execution_policy.changed_rust_line_limit = 0,
                2 => {
                    wrong.rust_execution_policy.diff_index_file_limit = 1;
                    wrong.rust_execution_policy.partial_diff_file_budget = 1;
                    wrong.rust_execution_policy.diff_narrow_index_limit = 2;
                }
                3 => {
                    wrong.rust_execution_policy.diff_index_file_limit = 1;
                    wrong.rust_execution_policy.diff_narrow_index_limit = 1;
                    wrong.rust_execution_policy.partial_diff_file_budget = 2;
                }
                4 => {
                    wrong.rust_execution_policy.changed_rust_line_limit = 1;
                    wrong.rust_execution_policy.partial_diff_line_budget = 2;
                }
                5 => wrong.rust_execution_policy.partial_selection_version = "stale".into(),
                _ => wrong.rust_execution_policy.partial_language_tier_version = "stale".into(),
            }
            let error = require_error(wrong.validate(), "invalid Rust policy was accepted")?;
            let expected = match change {
                0 => "unsupported complete binding schema",
                1..=4 => "invalid Rust execution policy limits or clamps",
                _ => "unsupported Rust partial-selection policy version",
            };
            assert_eq!(error, expected);
        }
        for disclosures in [vec!["".into()], vec!["x".repeat(1025)], vec!["x".into(); 3]] {
            let mut wrong = binding.clone();
            wrong.rust_execution_policy.partial_budget_disclosures = disclosures;
            let error = require_error(wrong.validate(), "invalid Rust policy disclosures were accepted")?;
            assert_eq!(error, "invalid Rust policy disclosures");
        }
        let mut changed = binding.clone();
        changed.rust_execution_policy.dependent_scope =
            match &changed.rust_execution_policy.dependent_scope {
                RustDependentScopePolicy::Auto => RustDependentScopePolicy::Full,
                _ => RustDependentScopePolicy::Auto,
            };
        changed.validate()?;
        assert_ne!(changed.generation_id()?, binding.generation_id()?);
        Ok(())
    }

}
