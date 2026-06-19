//! Reference adapter for Rust.
//!
//! See `docs/specs/RIPR-SPEC-0026-language-adapter-contract.md`.
//!
//! This adapter hosts the existing Rust analysis pipeline behind the
//! `LanguageAdapter` seam. The bodies of `analyze_diff` and `analyze_repo`
//! are relocated from `analysis::pipeline` without behavior change; the
//! pipeline module is now a language-neutral orchestrator that loads the
//! diff, dispatches to this adapter, and applies sort + summary on the
//! returned findings.

use super::super::{
    AnalysisOptions, classifier, classify, diff::ChangedFile, probes, rust_index, workspace,
};
use super::{LanguageAdapter, LanguageDiffResult, LanguageId, LanguageRepoResult, route};
use crate::analysis::facts::{FunctionSummary, RustIndex};
use crate::config::OraclePolicy;
use crate::domain::{ExposureClass, Finding, Probe, StaticLimitKind, StopReason};
use std::path::Path;

/// Default ceiling on the number of Rust files a diff-scoped analysis will
/// load into the index. A large multi-crate diff expands the index far beyond
/// the changed files (`select_rust_files_for_mode` pulls in whole touched
/// packages), and building that working set can exhaust a constrained runner
/// (issue #1023). Above this many files the analysis fails closed with a named
/// `diff_scope_oversized` error rather than exhausting host memory and aborting.
const DIFF_INDEX_FILE_LIMIT: usize = 800;

/// Env override for [`DIFF_INDEX_FILE_LIMIT`]. Operators on larger, well-resourced
/// runners raise it; CI can lower it to exercise the guard.
const DIFF_INDEX_FILE_LIMIT_ENV: &str = "RIPR_MAX_DIFF_INDEX_FILES";

/// Default ceiling on the number of added/removed Rust diff lines that may be
/// expanded into probes. Large code-motion PRs can touch only one indexed file
/// but still create thousands of probe/classifier records, exhausting
/// constrained runners before an artifact is written (#1324).
const DIFF_CHANGED_RUST_LINE_LIMIT: usize = 2_000;

/// Env override for [`DIFF_CHANGED_RUST_LINE_LIMIT`]. Operators can raise it
/// for larger runners or lower it to exercise the guard.
const DIFF_CHANGED_RUST_LINE_LIMIT_ENV: &str = "RIPR_MAX_DIFF_CHANGED_RUST_LINES";
const NO_TESTS_INFECTION_SUMMARY: &str =
    "No tests were found, so activation/infection cannot be estimated";
const NO_STATICALLY_REACHABLE_TEST_PATH_INFECTION_SUMMARY: &str =
    "No statically reachable test path was found, so activation/infection cannot be estimated";

fn diff_index_file_limit() -> Result<usize, String> {
    diff_index_file_limit_from_env(std::env::var(DIFF_INDEX_FILE_LIMIT_ENV))
}

fn diff_index_file_limit_from_env(
    value: Result<String, std::env::VarError>,
) -> Result<usize, String> {
    positive_limit_from_env(DIFF_INDEX_FILE_LIMIT_ENV, DIFF_INDEX_FILE_LIMIT, value)
}

fn diff_changed_rust_line_limit() -> Result<usize, String> {
    diff_changed_rust_line_limit_from_env(std::env::var(DIFF_CHANGED_RUST_LINE_LIMIT_ENV))
}

fn diff_changed_rust_line_limit_from_env(
    value: Result<String, std::env::VarError>,
) -> Result<usize, String> {
    positive_limit_from_env(
        DIFF_CHANGED_RUST_LINE_LIMIT_ENV,
        DIFF_CHANGED_RUST_LINE_LIMIT,
        value,
    )
}

fn positive_limit_from_env(
    env_name: &str,
    default: usize,
    value: Result<String, std::env::VarError>,
) -> Result<usize, String> {
    match value {
        Ok(raw) => {
            let parsed = raw
                .trim()
                .parse::<usize>()
                .map_err(|err| format!("{env_name} must be a positive integer: {err}"))?;
            if parsed == 0 {
                return Err(format!("{env_name} must be a positive integer"));
            }
            Ok(parsed)
        }
        Err(std::env::VarError::NotPresent) => Ok(default),
        Err(std::env::VarError::NotUnicode(_)) => Err(format!("{env_name} must be valid UTF-8")),
    }
}

fn changed_rust_line_count(changed_files: &[ChangedFile]) -> usize {
    changed_files
        .iter()
        .filter(|file| route(&file.path) == Some(LanguageId::Rust))
        .map(|file| {
            file.added_lines
                .len()
                .saturating_add(file.removed_lines.len())
        })
        .sum()
}

fn enforce_changed_rust_line_limit(
    changed_files: &[ChangedFile],
    line_limit: usize,
) -> Result<(), String> {
    let changed_line_count = changed_rust_line_count(changed_files);
    if changed_line_count <= line_limit {
        return Ok(());
    }
    let changed_file_count = changed_files
        .iter()
        .filter(|file| route(&file.path) == Some(LanguageId::Rust))
        .count();
    Err(format!(
        "diff_scope_oversized: {changed_line_count} changed Rust lines across \
         {changed_file_count} Rust files exceed the {DIFF_CHANGED_RUST_LINE_LIMIT_ENV} \
         limit ({line_limit}); analysis was not run to protect runner memory before \
         probe expansion. Repair route: reduce the diff scope, split the extraction \
         PR, run a narrower diff, or raise the limit via \
         {DIFF_CHANGED_RUST_LINE_LIMIT_ENV}=<number>."
    ))
}

/// Returns `true` when the owner function carries an FFI or language-binding
/// attribute that indicates its surface may be exercised by an external-language
/// test oracle rather than a Rust test.
///
/// The markers checked are the standard attribute substrings used by the major
/// Rust FFI and binding crates. `extern "C"` is intentionally excluded: it is
/// an ABI qualifier on the `fn` keyword and is not captured in
/// `FunctionFact.attrs`.
fn owner_has_ffi_attr(owner_fn: &FunctionSummary) -> bool {
    const FFI_MARKERS: &[&str] = &[
        "no_mangle",
        "export_name",
        "wasm_bindgen",
        "napi",
        "pyo3",
        "uniffi",
        "cxx",
    ];
    owner_fn.attrs.iter().any(|attr| {
        let lowered = attr.to_lowercase();
        FFI_MARKERS.iter().any(|marker| lowered.contains(marker))
    })
}

/// Resolve the probe's owner function from the index and check for FFI attrs.
/// Returns `Some(StaticLimitKind::CrossLanguageOracleVisibilityUnresolved)` when
/// the probe owner is FFI/binding-exposed and the finding class is an
/// unrevealed gap; `None` otherwise. Pure-Rust owners (no FFI attrs) return
/// `None` unconditionally.
fn cross_language_limit_kind(
    probe: &crate::domain::Probe,
    index: &rust_index::RustIndex,
    class: &ExposureClass,
) -> Option<StaticLimitKind> {
    let is_gap_class = matches!(
        class,
        ExposureClass::WeaklyExposed
            | ExposureClass::ReachableUnrevealed
            | ExposureClass::InfectionUnknown
    );
    if !is_gap_class {
        return None;
    }
    let owner_id = probe.owner.as_ref()?;
    let owner_fn = index
        .functions
        .iter()
        .find(|function| &function.id == owner_id)?;
    if owner_has_ffi_attr(owner_fn) {
        Some(StaticLimitKind::CrossLanguageOracleVisibilityUnresolved)
    } else {
        None
    }
}

/// Extract the bare function name from a probe's owner SymbolId for the
/// transitive-reach walk. The SymbolId format is "path::fn_name" or
/// "path::module::fn_name"; we return the last segment.
/// Returns None when the owner id is absent or the name is empty.
fn owner_name_from_id(
    owner: &Option<crate::domain::SymbolId>,
    _file: &std::path::Path,
) -> Option<String> {
    let id = owner.as_ref()?;
    // SymbolId format: "crates/ripr/src/lib.rs::pricing::score" or similar.
    // Take the last "::"-delimited segment.
    let name = id.0.split("::").last().unwrap_or("");
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn apply_rust_no_static_path_limit(finding: &mut Finding, probe: &Probe, index: &RustIndex) {
    if !(finding.class == ExposureClass::NoStaticPath
        && finding.related_tests.is_empty()
        && finding.static_limit_kind.is_none())
    {
        return;
    }

    let Some(owner_name) = owner_name_from_id(&probe.owner, &probe.location.file) else {
        return;
    };

    if let Some(witness) = classify::find_transitive_witness(&owner_name, index) {
        replace_witnessed_no_path_infection_summary(finding);
        finding.static_limit_kind = Some(StaticLimitKind::RustTransitiveReachUnresolved);
        finding
            .stop_reasons
            .push(StopReason::TransitiveReachUnresolved);
        finding
            .evidence
            .push(classify::RUST_TRANSITIVE_REACH_MESSAGE.to_string());
        finding
            .evidence
            .push(classify::transitive_reach_witness_pointer(&witness));
    } else if let Some(witness) = classify::find_macro_reach_witness(&owner_name, index) {
        replace_witnessed_no_path_infection_summary(finding);
        finding.static_limit_kind = Some(StaticLimitKind::RustMacroReachUnresolved);
        finding.stop_reasons.push(StopReason::MacroReachUnresolved);
        finding
            .evidence
            .push(classify::RUST_MACRO_REACH_MESSAGE.to_string());
        finding
            .evidence
            .push(classify::macro_reach_witness_pointer(&witness));
    }
}

fn replace_witnessed_no_path_infection_summary(finding: &mut Finding) {
    if finding.ripr.infect.summary == NO_TESTS_INFECTION_SUMMARY {
        finding.ripr.infect.summary =
            NO_STATICALLY_REACHABLE_TEST_PATH_INFECTION_SUMMARY.to_string();
    }
    for evidence in &mut finding.evidence {
        if evidence == NO_TESTS_INFECTION_SUMMARY {
            *evidence = NO_STATICALLY_REACHABLE_TEST_PATH_INFECTION_SUMMARY.to_string();
        }
    }
}

/// Reference adapter for Rust.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct RustAdapter;

impl LanguageAdapter for RustAdapter {
    fn accepts_path(&self, path: &Path) -> bool {
        matches!(route(path), Some(LanguageId::Rust))
    }

    fn analyze_diff(
        &self,
        options: &AnalysisOptions,
        oracle_policy: &OraclePolicy,
        changed_files: &[ChangedFile],
    ) -> Result<LanguageDiffResult, String> {
        let changed_rust_paths = changed_files
            .iter()
            .filter(|file| self.accepts_path(&file.path))
            .map(|file| file.path.clone())
            .collect::<Vec<_>>();
        enforce_changed_rust_line_limit(changed_files, diff_changed_rust_line_limit()?)?;
        let rust_files = workspace::discover_rust_files(&options.root)?;
        let index_files = workspace::select_rust_files_for_mode(
            &rust_files,
            &changed_rust_paths,
            options.mode,
            options.include_unchanged_tests,
        );
        // Fail closed before the working-set build that can exhaust a
        // constrained runner's memory (#1023): a too-large index is a named
        // limited state with a repair route, not an analysis result.
        let scope_limit = diff_index_file_limit()?;
        if index_files.len() > scope_limit {
            return Err(format!(
                "diff_scope_oversized: {} indexed Rust files exceed the \
                 {DIFF_INDEX_FILE_LIMIT_ENV} limit ({scope_limit}); analysis was not run to \
                 protect runner memory. Repair route: reduce the diff scope, run a narrower \
                 mode, or raise the limit via {DIFF_INDEX_FILE_LIMIT_ENV}=<number>.",
                index_files.len()
            ));
        }
        let mut index = rust_index::build_index(&options.root, &index_files)?;
        rust_index::apply_oracle_policy(&mut index, oracle_policy);

        let mut findings = Vec::new();
        let mut changed_rust_files = 0usize;

        for changed in changed_files
            .iter()
            .filter(|file| self.accepts_path(&file.path))
        {
            changed_rust_files += 1;
            let probes = probes::probes_for_file(&options.root, changed, &index);
            for probe in probes {
                let mut finding = classifier::classify_probe(&probe, &index);
                finding.language = Some(LanguageId::Rust);
                // `language_status` is omitted for Rust per RIPR-SPEC-0026.
                // RIPR-SPEC-0114: when the direct-call classifier finds no related
                // test (no_static_path + empty related_tests), run the bounded
                // transitive-reach walk. If a candidate path is found, name the
                // limitation. Classification NEVER changes (fail-closed).
                // RIPR-SPEC-0115: the walk returns the witnessing test so the
                // limitation can name something concrete to open (file:line +
                // entry symbol). The witness is NOT added to related_tests.
                // RIPR-SPEC-0117: when no lexical transitive path is available,
                // name a macro-reach limitation only when a same-repo macro
                // definition lexically mentions the changed owner.
                apply_rust_no_static_path_limit(&mut finding, &probe, &index);
                // Fail closed on cross-language seams: when the probe owner
                // carries an FFI/binding attribute, replace any Rust-gap
                // static_limit_kind with the cross-language limitation so
                // downstream consumers know to verify the external oracle
                // rather than acting on a Rust repair packet. (#910)
                if let Some(limit) = cross_language_limit_kind(&probe, &index, &finding.class) {
                    finding.static_limit_kind = Some(limit);
                }
                findings.push(finding);
            }
        }

        Ok(LanguageDiffResult {
            findings,
            changed_files: changed_rust_files,
        })
    }

    fn analyze_repo(
        &self,
        options: &AnalysisOptions,
        oracle_policy: &OraclePolicy,
    ) -> Result<LanguageRepoResult, String> {
        let rust_files = workspace::discover_rust_files(&options.root)?;
        let production_files = rust_files
            .iter()
            .filter(|path| workspace::is_production_rust_path(path))
            .cloned()
            .collect::<Vec<_>>();

        // Index all discovered Rust files (production + tests + benches +
        // examples). The classifier's `find_related_tests` looks up tests
        // in the index; without test files the repo headline silently
        // inflates `no_static_path` for owners that *are* exercised by
        // integration tests under `tests/` or `examples/`. Probe seeding
        // stays production-only so test bodies do not generate findings.
        let mut index = rust_index::build_index(&options.root, &rust_files)?;
        rust_index::apply_oracle_policy(&mut index, oracle_policy);

        let mut findings = Vec::new();

        for path in &production_files {
            let probes = probes::probes_for_repo_file(&options.root, path, &index);
            for probe in probes {
                let mut finding = classifier::classify_probe(&probe, &index);
                finding.language = Some(LanguageId::Rust);
                // `language_status` is omitted for Rust per RIPR-SPEC-0026.
                // RIPR-SPEC-0114 + 0115 + 0117: no_static_path limitation
                // disclosure for repo-mode (same logic as diff-mode).
                apply_rust_no_static_path_limit(&mut finding, &probe, &index);
                // Fail closed on cross-language seams (#910).
                if let Some(limit) = cross_language_limit_kind(&probe, &index, &finding.class) {
                    finding.static_limit_kind = Some(limit);
                }
                findings.push(finding);
            }
        }

        Ok(LanguageRepoResult {
            findings,
            production_files: production_files.len(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DIFF_CHANGED_RUST_LINE_LIMIT, DIFF_INDEX_FILE_LIMIT, changed_rust_line_count,
        cross_language_limit_kind, diff_changed_rust_line_limit_from_env,
        diff_index_file_limit_from_env, enforce_changed_rust_line_limit, owner_has_ffi_attr,
        replace_witnessed_no_path_infection_summary,
    };
    use crate::analysis::diff::{ChangedFile, ChangedLine};
    use crate::analysis::facts::{FunctionSummary, RustIndex};
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, Finding, Probe, ProbeFamily,
        ProbeId, RevealEvidence, RiprEvidence, SourceLocation, StageEvidence, StageState,
        StaticLimitKind, SymbolId,
    };
    use std::env::VarError;
    use std::path::PathBuf;

    #[test]
    fn diff_index_file_limit_defaults_when_unset() {
        assert_eq!(
            diff_index_file_limit_from_env(Err(VarError::NotPresent)),
            Ok(DIFF_INDEX_FILE_LIMIT)
        );
    }

    #[test]
    fn diff_index_file_limit_parses_positive_override() {
        assert_eq!(
            diff_index_file_limit_from_env(Ok("  50 ".to_string())),
            Ok(50)
        );
    }

    fn rejection_message(value: &str) -> String {
        match diff_index_file_limit_from_env(Ok(value.to_string())) {
            Ok(parsed) => format!("expected rejection of {value:?}, got Ok({parsed})"),
            Err(message) => message,
        }
    }

    #[test]
    fn diff_index_file_limit_rejects_zero() {
        let message = rejection_message("0");
        assert!(message.contains("positive integer"), "got: {message}");
    }

    #[test]
    fn diff_index_file_limit_rejects_non_numeric() {
        let message = rejection_message("lots");
        assert!(message.contains("positive integer"), "got: {message}");
    }

    #[test]
    fn diff_index_file_limit_rejects_non_unicode() {
        let result = diff_index_file_limit_from_env(Err(VarError::NotUnicode("x".into())));
        assert!(
            matches!(&result, Err(err) if err.contains("valid UTF-8")),
            "non-unicode must error with a UTF-8 message, got {result:?}"
        );
    }

    #[test]
    fn diff_changed_rust_line_limit_defaults_when_unset() -> Result<(), String> {
        let parsed = diff_changed_rust_line_limit_from_env(Err(VarError::NotPresent))?;
        if parsed != DIFF_CHANGED_RUST_LINE_LIMIT {
            return Err(format!(
                "expected default {DIFF_CHANGED_RUST_LINE_LIMIT}, got {parsed}"
            ));
        }
        Ok(())
    }

    #[test]
    fn diff_changed_rust_line_limit_parses_positive_override() -> Result<(), String> {
        let parsed = diff_changed_rust_line_limit_from_env(Ok("  1500 ".to_string()))?;
        if parsed != 1500 {
            return Err(format!("expected parsed limit 1500, got {parsed}"));
        }
        Ok(())
    }

    #[test]
    fn changed_rust_line_count_ignores_non_rust_paths() -> Result<(), String> {
        let files = vec![
            changed_file("src/lib.rs", 2, 1),
            changed_file("tests/example.test.ts", 30, 30),
        ];

        let count = changed_rust_line_count(&files);
        if count != 3 {
            return Err(format!(
                "expected only Rust changed lines to count, got {count}"
            ));
        }
        Ok(())
    }

    #[test]
    fn changed_rust_line_limit_rejects_oversized_diff_before_probe_expansion() -> Result<(), String>
    {
        let files = vec![changed_file("src/lib.rs", 2, 1)];

        let message = match enforce_changed_rust_line_limit(&files, 2) {
            Ok(()) => return Err("three changed Rust lines should exceed limit two".to_string()),
            Err(message) => message,
        };

        for needle in [
            "diff_scope_oversized",
            "3 changed Rust lines across 1 Rust files",
            "RIPR_MAX_DIFF_CHANGED_RUST_LINES",
            "split the extraction PR",
        ] {
            if !message.contains(needle) {
                return Err(format!("missing `{needle}` in message: {message}"));
            }
        }
        Ok(())
    }

    #[test]
    fn changed_rust_line_limit_accepts_at_limit() -> Result<(), String> {
        let files = vec![changed_file("src/lib.rs", 1, 1)];
        enforce_changed_rust_line_limit(&files, 2)
    }

    #[test]
    fn witnessed_no_path_limitation_does_not_claim_no_tests_found() {
        let mut finding = no_path_finding_with_infection_summary(
            super::NO_TESTS_INFECTION_SUMMARY,
            vec![
                "first evidence".to_string(),
                super::NO_TESTS_INFECTION_SUMMARY.to_string(),
            ],
        );

        replace_witnessed_no_path_infection_summary(&mut finding);

        assert_eq!(
            finding.ripr.infect.summary,
            super::NO_STATICALLY_REACHABLE_TEST_PATH_INFECTION_SUMMARY
        );
        assert!(
            finding
                .evidence
                .iter()
                .all(|line| line != super::NO_TESTS_INFECTION_SUMMARY),
            "witnessed limitations must not say no tests were found: {:?}",
            finding.evidence
        );
        assert!(
            finding
                .evidence
                .iter()
                .any(|line| line == super::NO_STATICALLY_REACHABLE_TEST_PATH_INFECTION_SUMMARY),
            "replacement evidence line should be preserved for renderers"
        );
    }

    #[test]
    fn witnessed_no_path_limitation_preserves_other_infection_summaries() {
        let summary = "No reachable tests were found, so infection cannot be established";
        let mut finding =
            no_path_finding_with_infection_summary(summary, vec![summary.to_string()]);

        replace_witnessed_no_path_infection_summary(&mut finding);

        assert_eq!(finding.ripr.infect.summary, summary);
        assert_eq!(finding.evidence, vec![summary.to_string()]);
    }

    fn changed_file(path: &str, added: usize, removed: usize) -> ChangedFile {
        ChangedFile {
            path: PathBuf::from(path),
            added_lines: changed_lines(added),
            removed_lines: changed_lines(removed),
        }
    }

    fn changed_lines(count: usize) -> Vec<ChangedLine> {
        (1..=count)
            .map(|line| ChangedLine {
                line,
                text: "let value = input + 1;".to_string(),
                new_side_line: line,
            })
            .collect()
    }

    fn no_path_finding_with_infection_summary(summary: &str, evidence: Vec<String>) -> Finding {
        let stage = |state| StageEvidence::new(state, Confidence::Low, "stage");
        Finding {
            id: "probe:src_lib.rs:predicate:test".to_string(),
            canonical_gap: None,
            probe: Probe {
                id: ProbeId("probe:src_lib.rs:predicate:test".to_string()),
                location: SourceLocation::new("src/lib.rs", 2, 1),
                owner: Some(SymbolId("src/lib.rs::inner".to_string())),
                family: ProbeFamily::Predicate,
                delta: DeltaKind::Control,
                before: None,
                after: Some("if a >= b {".to_string()),
                expression: "if a >= b {".to_string(),
                expected_sinks: Vec::new(),
                required_oracles: Vec::new(),
            },
            class: ExposureClass::NoStaticPath,
            ripr: RiprEvidence {
                reach: stage(StageState::No),
                infect: StageEvidence::new(StageState::Unknown, Confidence::Low, summary),
                propagate: stage(StageState::Yes),
                reveal: RevealEvidence {
                    observe: stage(StageState::No),
                    discriminate: stage(StageState::No),
                },
            },
            confidence: 0.48,
            evidence,
            missing: Vec::new(),
            flow_sinks: Vec::new(),
            activation: ActivationEvidence::default(),
            stop_reasons: Vec::new(),
            related_tests: Vec::new(),
            recommended_next_step: None,
            language: None,
            language_status: None,
            owner_kind: None,
            static_limit_kind: None,
            changed_sink: None,
            observed_sink: None,
            oracle_alignment: None,
            alignment_reason: None,
        }
    }

    // --- FFI / cross-language guard tests (#910) ---

    fn ffi_function(file: &str, name: &str, attrs: Vec<&str>) -> FunctionSummary {
        FunctionSummary {
            id: SymbolId(format!("{file}::{name}")),
            name: name.to_string(),
            file: PathBuf::from(file),
            start_line: 1,
            end_line: 5,
            body: format!("pub fn {name}(x: i32) -> i32 {{ x }}"),
            calls: vec![],
            returns: vec![],
            literals: vec![],
            is_test: false,
            attrs: attrs.into_iter().map(|s| s.to_string()).collect(),
        }
    }

    fn probe_for_owner(file: &str, name: &str, family: ProbeFamily) -> Probe {
        Probe {
            id: ProbeId(format!("probe:{file}::{name}")),
            location: SourceLocation::new(file, 2, 1),
            owner: Some(SymbolId(format!("{file}::{name}"))),
            family,
            delta: DeltaKind::Control,
            before: None,
            after: Some("x > 0".to_string()),
            expression: "x > 0".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        }
    }

    #[test]
    fn owner_with_no_mangle_attr_is_ffi() {
        let owner = ffi_function("src/lib.rs", "ffi_fn", vec!["#[no_mangle]"]);
        assert!(owner_has_ffi_attr(&owner));
    }

    #[test]
    fn owner_with_wasm_bindgen_attr_is_ffi() {
        let owner = ffi_function("src/lib.rs", "wasm_fn", vec!["#[wasm_bindgen]"]);
        assert!(owner_has_ffi_attr(&owner));
    }

    #[test]
    fn owner_with_no_attrs_is_not_ffi() {
        let owner = ffi_function("src/lib.rs", "pure_fn", vec![]);
        assert!(!owner_has_ffi_attr(&owner));
    }

    #[test]
    fn owner_with_plain_test_attr_is_not_ffi() {
        let owner = ffi_function("src/lib.rs", "plain_fn", vec!["#[test]"]);
        assert!(!owner_has_ffi_attr(&owner));
    }

    #[test]
    fn cross_language_guard_fires_for_weakly_exposed_with_ffi_attr() {
        let owner = ffi_function("src/lib.rs", "exported_fn", vec!["#[no_mangle]"]);
        let probe = probe_for_owner("src/lib.rs", "exported_fn", ProbeFamily::Predicate);
        let index = RustIndex {
            functions: vec![owner],
            ..RustIndex::default()
        };
        let result = cross_language_limit_kind(&probe, &index, &ExposureClass::WeaklyExposed);
        assert_eq!(
            result,
            Some(StaticLimitKind::CrossLanguageOracleVisibilityUnresolved),
            "FFI-marked owner with WeaklyExposed gap should set cross-language limit"
        );
    }

    #[test]
    fn cross_language_guard_fires_for_reachable_unrevealed_with_wasm_bindgen() {
        let owner = ffi_function("src/lib.rs", "wasm_fn", vec!["#[wasm_bindgen]"]);
        let probe = probe_for_owner("src/lib.rs", "wasm_fn", ProbeFamily::ReturnValue);
        let index = RustIndex {
            functions: vec![owner],
            ..RustIndex::default()
        };
        let result = cross_language_limit_kind(&probe, &index, &ExposureClass::ReachableUnrevealed);
        assert_eq!(
            result,
            Some(StaticLimitKind::CrossLanguageOracleVisibilityUnresolved),
            "FFI-marked owner with ReachableUnrevealed gap should set cross-language limit"
        );
    }

    #[test]
    fn cross_language_guard_fires_for_infection_unknown_with_ffi_attr() {
        let owner = ffi_function("src/lib.rs", "exported_fn", vec!["#[no_mangle]"]);
        let probe = probe_for_owner("src/lib.rs", "exported_fn", ProbeFamily::Predicate);
        let index = RustIndex {
            functions: vec![owner],
            ..RustIndex::default()
        };
        let result = cross_language_limit_kind(&probe, &index, &ExposureClass::InfectionUnknown);
        assert_eq!(
            result,
            Some(StaticLimitKind::CrossLanguageOracleVisibilityUnresolved),
            "FFI-marked owner with InfectionUnknown gap should set cross-language limit"
        );
    }

    #[test]
    fn cross_language_guard_does_not_fire_for_pure_rust_owner_weakly_exposed() {
        // Pure-Rust control: no FFI attr — guard must NOT fire even for a gap class.
        let owner = ffi_function("src/lib.rs", "pure_fn", vec![]);
        let probe = probe_for_owner("src/lib.rs", "pure_fn", ProbeFamily::Predicate);
        let index = RustIndex {
            functions: vec![owner],
            ..RustIndex::default()
        };
        let result = cross_language_limit_kind(&probe, &index, &ExposureClass::WeaklyExposed);
        assert_eq!(
            result, None,
            "Pure-Rust owner must NOT receive cross-language static_limit_kind"
        );
    }

    #[test]
    fn cross_language_guard_does_not_fire_for_exposed_class_even_with_ffi() {
        // Even an FFI-marked owner should not gain the limitation on Exposed
        // (no gap = nothing to reclassify).
        let owner = ffi_function("src/lib.rs", "exported_fn", vec!["#[no_mangle]"]);
        let probe = probe_for_owner("src/lib.rs", "exported_fn", ProbeFamily::ReturnValue);
        let index = RustIndex {
            functions: vec![owner],
            ..RustIndex::default()
        };
        let result = cross_language_limit_kind(&probe, &index, &ExposureClass::Exposed);
        assert_eq!(
            result, None,
            "Exposed class must not receive cross-language static_limit_kind regardless of FFI"
        );
    }
}
