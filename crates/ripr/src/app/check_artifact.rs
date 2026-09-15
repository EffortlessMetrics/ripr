//! Check-artifact reuse for `explain` / `context` (RIPR-SPEC-0140, #2107).
//!
//! `ripr check --write-artifact <path>` writes a full-fidelity
//! [`CheckArtifactV1`] envelope (the complete `Finding` set plus an input
//! identity block) so a later `ripr explain --from <path>` or
//! `ripr context --from <path>` can select and render from the recorded
//! findings instead of re-running the whole pipeline. Both directions are
//! explicit user intent: there is no implicit cross-invocation cache.
//!
//! Reuse is gated fail-closed on the recorded identity: the recorded diff
//! source is re-resolved and re-hashed, and the root, mode, enabled
//! languages, analysis input options, config identity, and analyzer version
//! are recomputed from the current invocation. Any deviation is a typed
//! error naming the mismatched fields — never a silent recompute. The
//! existing `check --json` render is untouched: the JSON findings projection
//! is lossy (related tests capped, conditional probe owner, render-time
//! severity, no diff identity) and cannot be a reuse source.

use super::CheckInput;
use super::check::options_builder::analysis_options_from_input_and_config;
use crate::analysis::AnalysisOptions;
use crate::config::{
    CHECK_ARTIFACT_CONFIG_IDENTITY_VERSION, RiprConfig, check_artifact_config_identity_hash,
    config_fingerprint,
};
use crate::domain::{Finding, LanguageId};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Artifact envelope schema version. Bump on any envelope shape change; old
/// artifacts fail closed at load instead of being misread.
pub(crate) const CHECK_ARTIFACT_SCHEMA_VERSION: &str = "ripr-check-artifact-v1";

/// Full-fidelity check artifact written by `ripr check --write-artifact`.
///
/// This is a local, disposable derivative of one `check` run. It must never
/// feed a support-tier row, gate, badge, or proof route.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CheckArtifactV1 {
    pub(crate) schema_version: String,
    pub(crate) tool: String,
    /// `env!("CARGO_PKG_VERSION")` of the writing binary; part of the gate.
    pub(crate) analyzer_version: String,
    pub(crate) identity: CheckArtifactIdentityV1,
    /// The complete finding set: uncapped related-tests lists and probe
    /// owners included, exactly as the producing run computed them.
    pub(crate) findings: Vec<Finding>,
}

/// Input identity recorded at check time and re-verified at reuse time.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CheckArtifactIdentityV1 {
    /// Where the analyzed diff came from, so the consumer can re-resolve it.
    pub(crate) diff_source: DiffSourceIdentity,
    /// Hash of the exact diff bytes the analysis consumed.
    pub(crate) diff_bytes_hash: String,
    /// Canonicalized workspace root.
    pub(crate) root: String,
    /// Resolved analysis effort profile (`CheckInput::mode`).
    pub(crate) mode: String,
    /// Resolved enabled languages (sorted), including the explicit
    /// `--perl-facts` opt-in.
    pub(crate) enabled_languages: Vec<String>,
    /// The `CheckInput` analysis-option surface that flows into
    /// `analysis_options_from_input_and_config` and is not already recorded
    /// elsewhere in this identity.
    pub(crate) analysis_options: AnalysisOptionsIdentity,
    /// Version of the closed config-identity allowlist contract.
    pub(crate) config_identity_version: u32,
    /// Hash over the finding-affecting config allowlist fields.
    pub(crate) config_identity_hash: String,
}

/// The diff source the producing run analyzed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DiffSourceIdentity {
    /// A `--diff` file; the canonicalized path is re-read and re-hashed.
    DiffFile { path: String },
    /// A `base...HEAD` git diff; the pair is re-resolved through git. `base`
    /// is `None` when the producing run used dynamic default-base
    /// resolution (RIPR-SPEC-0084); `head` is always `HEAD` today.
    BaseHead { base: Option<String>, head: String },
    /// A `--worktree` run: the `base`-to-live-working-tree diff is
    /// re-resolved through git. `base` is `None` when the producing run
    /// used dynamic default-base resolution (RIPR-SPEC-0084); a dirty or
    /// advanced worktree between write and reuse trips `diff_bytes_hash`.
    Worktree { base: Option<String> },
}

/// The `CheckInput` analysis options recorded in the identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AnalysisOptionsIdentity {
    pub(crate) include_unchanged_tests: bool,
    /// Canonicalized path of the explicit `--perl-facts` packet, when used.
    pub(crate) perl_facts_path: Option<String>,
    /// Content hash of that packet: the Perl adapter reads
    /// findings-changing facts from the file, so the path alone is not an
    /// identity.
    pub(crate) perl_facts_content_hash: Option<String>,
}

/// Raw-byte diff paths cannot serialize into JSON strings, so any
/// non-UTF-8 path field renders through the injective stable text
/// projection (#3609): the artifact stays writable and each distinct
/// path keeps a distinct, resolvable spelling. Valid-UTF-8 paths are
/// untouched, keeping existing artifacts byte-identical.
fn rewrite_non_utf8_finding_paths(findings: &[Finding]) -> Vec<Finding> {
    findings
        .iter()
        .map(|finding| {
            let mut finding = finding.clone();
            if finding.probe.location.file.to_str().is_none() {
                finding.probe.location.file = std::path::PathBuf::from(
                    crate::analysis::stable_path_text(&finding.probe.location.file),
                );
            }
            for related in &mut finding.related_tests {
                if related.file.to_str().is_none() {
                    related.file =
                        std::path::PathBuf::from(crate::analysis::stable_path_text(&related.file));
                }
            }
            finding
        })
        .collect()
}

pub(crate) fn write_check_artifact(
    path: &Path,
    input: &CheckInput,
    config: &RiprConfig,
    findings: &[Finding],
    worktree: bool,
) -> Result<(), String> {
    let identity = build_identity_at_write(input, config, worktree)?;
    let artifact = CheckArtifactV1 {
        schema_version: CHECK_ARTIFACT_SCHEMA_VERSION.to_string(),
        tool: "ripr".to_string(),
        analyzer_version: env!("CARGO_PKG_VERSION").to_string(),
        identity,
        findings: rewrite_non_utf8_finding_paths(findings),
    };
    let json = serde_json::to_string_pretty(&artifact)
        .map_err(|err| format!("failed to serialize check artifact: {err}"))?;
    crate::atomic_file::write(path, json.as_bytes(), "check artifact")
}

/// Load an artifact and verify its identity against the current invocation,
/// returning the recorded findings. Fail-closed: any structural deviation,
/// unresolvable recorded diff source, or identity mismatch is a typed error;
/// there is no silent fallback to recompute.
///
/// `asserted_base` is the explicitly passed `--base` value, when any; an
/// explicitly passed `--diff` is read from `input.diff_file`. Both are
/// assertions verified against the recorded diff source, not overrides.
pub(crate) fn load_findings_for_reuse(
    path: &Path,
    input: &CheckInput,
    config: &RiprConfig,
    asserted_base: Option<&str>,
) -> Result<Vec<Finding>, String> {
    let text = std::fs::read_to_string(path).map_err(|err| {
        format!(
            "check artifact at {} not found or unreadable: {err}",
            path.display()
        )
    })?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|err| {
        format!(
            "check artifact at {} is malformed: invalid JSON: {err}",
            path.display()
        )
    })?;
    match value.get("schema_version").and_then(|v| v.as_str()) {
        Some(CHECK_ARTIFACT_SCHEMA_VERSION) => {}
        Some(other) => {
            return Err(format!(
                "check artifact at {} has unsupported schema_version {other:?} (expected {CHECK_ARTIFACT_SCHEMA_VERSION:?}); re-run `ripr check --write-artifact` with this ripr",
                path.display()
            ));
        }
        None => {
            return Err(format!(
                "check artifact at {} is malformed: missing schema_version",
                path.display()
            ));
        }
    }
    let artifact: CheckArtifactV1 = serde_json::from_value(value)
        .map_err(|err| format!("check artifact at {} is malformed: {err}", path.display()))?;
    if artifact.tool != "ripr" {
        return Err(format!(
            "check artifact at {} was not written by ripr (tool {:?})",
            path.display(),
            artifact.tool
        ));
    }

    verify_scope_assertions(&artifact.identity.diff_source, input, asserted_base, path)?;
    let current = build_identity_at_load(input, config, &artifact.identity, path)?;
    verify_identity(&artifact, &current, path)?;
    Ok(artifact.findings)
}

/// Identity built at check time from the resolved input and config.
fn build_identity_at_write(
    input: &CheckInput,
    config: &RiprConfig,
    worktree: bool,
) -> Result<CheckArtifactIdentityV1, String> {
    let diff_source = diff_source_at_write(input, worktree)?;
    let diff_text = resolve_diff_text(&diff_source, &input.root, input.git_timeout)?;
    let options = analysis_options_from_input_and_config(input, config);
    let (include_unchanged_tests, perl_facts_path) = closed_analysis_options_view(&options);
    let perl_facts_path = canonical_optional_path(perl_facts_path)?;
    Ok(CheckArtifactIdentityV1 {
        diff_bytes_hash: config_fingerprint(&diff_text),
        diff_source,
        root: canonical_root(&input.root)?,
        mode: input.mode.as_str().to_string(),
        enabled_languages: resolved_enabled_languages(config, perl_facts_path.is_some()),
        analysis_options: analysis_options_identity(
            include_unchanged_tests,
            perl_facts_path.as_deref(),
        )?,
        config_identity_version: CHECK_ARTIFACT_CONFIG_IDENTITY_VERSION,
        config_identity_hash: check_artifact_config_identity_hash(config),
    })
}

/// Identity recomputed at reuse time: root, mode, languages, options, and
/// config identity come from the current invocation; the diff source and
/// Perl facts packet come from the recording (re-resolved and re-hashed).
fn build_identity_at_load(
    input: &CheckInput,
    config: &RiprConfig,
    recorded: &CheckArtifactIdentityV1,
    artifact_path: &Path,
) -> Result<CheckArtifactIdentityV1, String> {
    let diff_text = resolve_diff_text(&recorded.diff_source, &input.root, input.git_timeout)
        .map_err(|err| {
            format!(
                "check artifact at {} cannot be reused: {err}",
                artifact_path.display()
            )
        })?;
    let perl_facts_path = recorded
        .analysis_options
        .perl_facts_path
        .as_deref()
        .map(PathBuf::from);
    Ok(CheckArtifactIdentityV1 {
        diff_bytes_hash: config_fingerprint(&diff_text),
        diff_source: recorded.diff_source.clone(),
        root: canonical_root(&input.root)?,
        mode: input.mode.as_str().to_string(),
        enabled_languages: resolved_enabled_languages(config, perl_facts_path.is_some()),
        analysis_options: analysis_options_identity(
            input.include_unchanged_tests,
            perl_facts_path.as_deref(),
        )
        .map_err(|err| {
            format!(
                "check artifact at {} cannot be reused: {err}",
                artifact_path.display()
            )
        })?,
        config_identity_version: CHECK_ARTIFACT_CONFIG_IDENTITY_VERSION,
        config_identity_hash: check_artifact_config_identity_hash(config),
    })
}

/// Compare the recomputed identity against the recording, field by field,
/// and fail closed naming every mismatched field.
fn verify_identity(
    artifact: &CheckArtifactV1,
    current: &CheckArtifactIdentityV1,
    path: &Path,
) -> Result<(), String> {
    let recorded = &artifact.identity;
    let mut mismatched: Vec<&str> = Vec::new();
    if recorded.diff_bytes_hash != current.diff_bytes_hash {
        mismatched.push("diff_bytes_hash");
    }
    if recorded.root != current.root {
        mismatched.push("root");
    }
    if recorded.mode != current.mode {
        mismatched.push("mode");
    }
    if recorded.enabled_languages != current.enabled_languages {
        mismatched.push("enabled_languages");
    }
    if recorded.analysis_options.include_unchanged_tests
        != current.analysis_options.include_unchanged_tests
    {
        mismatched.push("analysis_options.include_unchanged_tests");
    }
    if recorded.analysis_options.perl_facts_path != current.analysis_options.perl_facts_path {
        mismatched.push("analysis_options.perl_facts_path");
    }
    if recorded.analysis_options.perl_facts_content_hash
        != current.analysis_options.perl_facts_content_hash
    {
        mismatched.push("analysis_options.perl_facts_content_hash");
    }
    if recorded.config_identity_version != current.config_identity_version {
        mismatched.push("config_identity_version");
    }
    if recorded.config_identity_hash != current.config_identity_hash {
        mismatched.push("config_identity_hash");
    }
    if artifact.analyzer_version != env!("CARGO_PKG_VERSION") {
        mismatched.push("analyzer_version");
    }
    if mismatched.is_empty() {
        return Ok(());
    }
    Err(format!(
        "check artifact at {} cannot be reused: identity mismatch on {}. \
         The artifact was produced from a different analysis input; \
         re-run `ripr check --write-artifact` with the current input.",
        path.display(),
        mismatched.join(", ")
    ))
}

/// Verify explicitly passed scope flags (`--diff`, `--base`) against the
/// recorded diff source. They are assertions, never overrides.
fn verify_scope_assertions(
    recorded: &DiffSourceIdentity,
    input: &CheckInput,
    asserted_base: Option<&str>,
    artifact_path: &Path,
) -> Result<(), String> {
    let mut mismatched: Vec<&str> = Vec::new();
    if let Some(asserted_diff) = input.diff_file.as_ref() {
        let asserted = std::fs::canonicalize(asserted_diff).map_err(|err| {
            format!(
                "asserted --diff {} cannot be resolved: {err}",
                asserted_diff.display()
            )
        })?;
        let matches = matches!(
            recorded,
            DiffSourceIdentity::DiffFile { path } if Path::new(path) == asserted
        );
        if !matches {
            mismatched.push("diff_source");
        }
    }
    if let Some(base) = asserted_base {
        let recorded_base = match recorded {
            DiffSourceIdentity::BaseHead { base, .. } | DiffSourceIdentity::Worktree { base } => {
                base.as_deref()
            }
            DiffSourceIdentity::DiffFile { .. } => None,
        };
        if recorded_base != Some(base) {
            mismatched.push("diff_source.base");
        }
    }
    if mismatched.is_empty() {
        return Ok(());
    }
    Err(format!(
        "check artifact at {} cannot be reused: asserted scope does not match the recording on {}. \
         Drop the assertion or re-run `ripr check --write-artifact` with that scope.",
        artifact_path.display(),
        mismatched.join(", ")
    ))
}

/// Record the diff source from the resolved check input. `worktree` marks a
/// `--worktree` producing run: its diff source is the base-to-live-working-
/// tree diff, which is re-resolvable at reuse time through git.
fn diff_source_at_write(input: &CheckInput, worktree: bool) -> Result<DiffSourceIdentity, String> {
    if let Some(diff_file) = input.diff_file.as_ref() {
        let canonical = std::fs::canonicalize(diff_file).map_err(|err| {
            format!(
                "failed to canonicalize diff file {}: {err}",
                diff_file.display()
            )
        })?;
        return Ok(DiffSourceIdentity::DiffFile {
            path: canonical.to_string_lossy().to_string(),
        });
    }
    if worktree {
        return Ok(DiffSourceIdentity::Worktree {
            base: input.base.clone(),
        });
    }
    Ok(DiffSourceIdentity::BaseHead {
        base: input.base.clone(),
        head: "HEAD".to_string(),
    })
}

/// Re-resolve a recorded diff source to its exact bytes: a recorded `--diff`
/// path is re-read; a recorded base/head pair or worktree diff is
/// re-resolved through git. Missing or unresolvable sources fail closed.
fn resolve_diff_text(
    source: &DiffSourceIdentity,
    root: &Path,
    git_timeout: Option<std::time::Duration>,
) -> Result<String, String> {
    match source {
        DiffSourceIdentity::DiffFile { path } => std::fs::read_to_string(path).map_err(|err| {
            format!("recorded diff file {path} no longer exists or is unreadable: {err}")
        }),
        DiffSourceIdentity::BaseHead { base, .. } => {
            crate::analysis::load_diff(root, base.as_deref(), None, git_timeout)
                .map_err(|err| format!("recorded base/head diff could not be re-resolved: {err}"))
        }
        DiffSourceIdentity::Worktree { base } => {
            crate::analysis::load_worktree_diff(root, base.as_deref(), git_timeout)
                .map_err(|err| format!("recorded worktree diff could not be re-resolved: {err}"))
        }
    }
}

/// Extract the identity-relevant analysis options with a closed contract:
/// every `AnalysisOptions` field is destructured explicitly (no `..`), so a
/// future analysis input option fails compilation here until it is either
/// recorded in the identity or consciously mapped to an existing slot.
fn closed_analysis_options_view(options: &AnalysisOptions) -> (bool, Option<&Path>) {
    let AnalysisOptions {
        root: _,      // recorded as identity.root
        base: _,      // recorded via identity.diff_source
        diff_file: _, // recorded via identity.diff_source
        mode: _,      // recorded as identity.mode
        include_unchanged_tests,
        resolve_tsconfig_paths: _, // recorded via the config identity allowlist
        perl_facts_path,
        git_timeout: _, // resource bound (#2303), not analysis identity: a
        // deadline never changes what the analysis computes,
        // only whether a hung git invocation aborts the run
        resolved_subject_identity: _, // derived, not caller input: the R2
        // producer sets it on the internal options clone only (#3278);
        // the caller-visible subject flows through `git_candidate`
        git_candidate: _, // unreachable at write time: `run_check` rejects
        // Git candidate subjects before analysis, so an
        // artifact can never carry one until #3277/#3278
        // record the subject's resolved identity here
        test_harnesses: _, // recorded via the config identity
        production_like_targets: _, // recorded via the config identity
                           // allowlist: the opt-in originates in
                           // `[analysis]` (#3283), which the
                           // versioned config identity already
                           // hashes field-by-field
    } = options;
    (*include_unchanged_tests, perl_facts_path.as_deref())
}

/// Build the analysis-options identity, hashing the Perl facts packet
/// content when one was consumed.
fn analysis_options_identity(
    include_unchanged_tests: bool,
    perl_facts_path: Option<&Path>,
) -> Result<AnalysisOptionsIdentity, String> {
    let perl_facts_content_hash = match perl_facts_path {
        Some(path) => {
            let text = std::fs::read_to_string(path).map_err(|err| {
                format!(
                    "recorded Perl facts packet {} no longer exists or is unreadable: {err}",
                    path.display()
                )
            })?;
            Some(config_fingerprint(&text))
        }
        None => None,
    };
    Ok(AnalysisOptionsIdentity {
        include_unchanged_tests,
        perl_facts_path: perl_facts_path.map(|path| path.to_string_lossy().to_string()),
        perl_facts_content_hash,
    })
}

/// Resolved enabled languages (sorted, deduplicated), mirroring the
/// `run_check` rule that an explicit `--perl-facts` packet opts Perl in.
fn resolved_enabled_languages(config: &RiprConfig, perl_facts_present: bool) -> Vec<String> {
    let mut languages = config
        .languages()
        .enabled()
        .iter()
        .map(|language| language.as_str().to_string())
        .collect::<Vec<_>>();
    let perl = LanguageId::Perl.as_str().to_string();
    if perl_facts_present && !languages.contains(&perl) {
        languages.push(perl);
    }
    languages.sort_unstable();
    languages.dedup();
    languages
}

/// Canonicalize an optional path, failing closed when a present path cannot
/// be resolved.
fn canonical_optional_path(path: Option<&Path>) -> Result<Option<PathBuf>, String> {
    match path {
        Some(path) => std::fs::canonicalize(path)
            .map(Some)
            .map_err(|err| format!("failed to canonicalize {}: {err}", path.display())),
        None => Ok(None),
    }
}

/// Canonicalized root string for identity comparison.
fn canonical_root(root: &Path) -> Result<String, String> {
    std::fs::canonicalize(root)
        .map(|path| path.to_string_lossy().to_string())
        .map_err(|err| format!("failed to canonicalize root {}: {err}", root.display()))
}

#[cfg(test)]
mod tests;

#[cfg(all(test, unix))]
mod raw_path_tests {
    use super::*;
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, Finding, FlowSinkKind,
        OracleKind, OracleStrength, ProbeFamily, ProbeId, RelatedTest, RevealEvidence,
        RiprEvidence, SourceCurrentness, StageEvidence, StageState, StopReason,
    };
    use crate::domain::{Probe, SourceLocation};
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    use std::path::PathBuf;

    fn raw_byte_finding() -> Finding {
        let raw = PathBuf::from(OsStr::from_bytes(b"pricing_\xff.rs"));
        Finding {
            id: "probe:pricing:1:error_path".to_string(),
            canonical_gap: None,
            probe: Probe {
                id: ProbeId("probe:pricing:1:error_path".to_string()),
                family: ProbeFamily::ErrorPath,
                location: SourceLocation::new(raw.clone(), 1, 1),
                owner: None,
                delta: DeltaKind::Control,
                before: None,
                after: None,
                expression: "sample_expr".to_string(),
                expected_sinks: Vec::new(),
                required_oracles: Vec::new(),
            },
            class: ExposureClass::WeaklyExposed,
            ripr: RiprEvidence {
                reach: StageEvidence::new(StageState::Yes, Confidence::Medium, "reached"),
                infect: StageEvidence::new(StageState::Weak, Confidence::Low, "infected"),
                propagate: StageEvidence::new(StageState::No, Confidence::Medium, "none"),
                reveal: RevealEvidence {
                    observe: StageEvidence::new(StageState::Weak, Confidence::Low, "observed"),
                    discriminate: StageEvidence::new(
                        StageState::No,
                        Confidence::Medium,
                        "no discriminator",
                    ),
                },
            },
            confidence: 0.5,
            evidence: Vec::new(),
            missing: Vec::new(),
            flow_sinks: Vec::new(),
            activation: ActivationEvidence::default(),
            stop_reasons: vec![StopReason::NoChangedRustLine],
            related_tests: vec![RelatedTest {
                name: "nearby".to_string(),
                file: raw,
                line: 2,
                oracle: None,
                oracle_kind: OracleKind::Unknown,
                oracle_strength: OracleStrength::Weak,
                relation_reason: None,
                relation_confidence: None,
            }],
            recommended_next_step: None,
            language: None,
            language_status: None,
            owner_kind: None,
            static_limit_kind: None,
            changed_sink: None,
            observed_sink: None,
            oracle_alignment: None,
            alignment_reason: None,
            source_currentness: SourceCurrentness::CandidateCurrent,
        }
    }

    #[test]
    fn raw_byte_finding_paths_render_through_stable_text() -> Result<(), String> {
        let finding = raw_byte_finding();
        // Sanity: the fixture really carries a non-UTF-8 path, the shape
        // that motivated the rewrite.
        assert!(finding.probe.location.file.to_str().is_none());
        let rewritten = rewrite_non_utf8_finding_paths(&[finding]);
        assert_eq!(
            rewritten[0].probe.location.file,
            PathBuf::from("pricing_%FF.rs")
        );
        assert_eq!(
            rewritten[0].related_tests[0].file,
            PathBuf::from("pricing_%FF.rs")
        );
        // The rewritten artifact serializes without the JSON
        // invalid-UTF-8 rejection that motivated the rewrite.
        let serialized = serde_json::to_string(&rewritten)
            .map_err(|error| format!("artifact serializes: {error}"))?;
        assert!(serialized.contains("pricing_%FF.rs"));
        let _ = FlowSinkKind::ReturnValue;
        Ok(())
    }

    #[test]
    fn valid_utf8_paths_stay_untouched() {
        let mut finding = raw_byte_finding();
        finding.probe.location.file = PathBuf::from("src/pricing.rs");
        finding.related_tests[0].file = PathBuf::from("tests/pricing_test.rs");
        let rewritten = rewrite_non_utf8_finding_paths(&[finding]);
        assert_eq!(
            rewritten[0].probe.location.file,
            PathBuf::from("src/pricing.rs")
        );
        assert_eq!(
            rewritten[0].related_tests[0].file,
            PathBuf::from("tests/pricing_test.rs")
        );
    }
}
