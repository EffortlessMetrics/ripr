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

use super::super::{AnalysisOptions, classifier, diff::ChangedFile, probes, rust_index, workspace};
use super::{LanguageAdapter, LanguageDiffResult, LanguageId, LanguageRepoResult, route};
use crate::analysis::facts::FunctionSummary;
use crate::config::OraclePolicy;
use crate::domain::{ExposureClass, StaticLimitKind};
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

fn diff_index_file_limit() -> Result<usize, String> {
    diff_index_file_limit_from_env(std::env::var(DIFF_INDEX_FILE_LIMIT_ENV))
}

fn diff_index_file_limit_from_env(
    value: Result<String, std::env::VarError>,
) -> Result<usize, String> {
    match value {
        Ok(raw) => {
            let parsed = raw.trim().parse::<usize>().map_err(|err| {
                format!("{DIFF_INDEX_FILE_LIMIT_ENV} must be a positive integer: {err}")
            })?;
            if parsed == 0 {
                return Err(format!(
                    "{DIFF_INDEX_FILE_LIMIT_ENV} must be a positive integer"
                ));
            }
            Ok(parsed)
        }
        Err(std::env::VarError::NotPresent) => Ok(DIFF_INDEX_FILE_LIMIT),
        Err(std::env::VarError::NotUnicode(_)) => {
            Err(format!("{DIFF_INDEX_FILE_LIMIT_ENV} must be valid UTF-8"))
        }
    }
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
        DIFF_INDEX_FILE_LIMIT, cross_language_limit_kind, diff_index_file_limit_from_env,
        owner_has_ffi_attr,
    };
    use crate::analysis::facts::{FunctionSummary, RustIndex};
    use crate::domain::{
        DeltaKind, ExposureClass, Probe, ProbeFamily, ProbeId, SourceLocation, StaticLimitKind,
        SymbolId,
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
