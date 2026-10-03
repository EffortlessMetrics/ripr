//! Related-test honesty for unchanged Rust files indexed on lexical fallback.
//!
//! #4775: when an unchanged test file fails the reference parser, it is
//! indexed lexically. Compact `#[test] fn ...` registrations then drop out of
//! related-test discovery, so a changed production owner can read
//! `no_static_path` while the run stays complete.
//!
//! This module is the single owner that decides whether such a file
//! contributed, or would have contributed, related-test evidence to a
//! classified finding. Unrelated parser-refused files in the same crate do
//! not make the run partial (TypeScript #4261 analog). Changed files stay
//! out: they are the #4722 producer-failure lane, not this claim.

use super::mask_rust_comments_and_strings;
use super::owner_name_from_id;
use crate::analysis::facts::{FileFacts, RustIndex};
use crate::analysis::rust_index;
use crate::analysis::workspace;
use crate::analysis_outcome::{
    AnalysisLimitation, AnalysisLimitationKind, AnalysisRecovery, AnalysisRecoveryKind,
    AnalysisStage, MAX_ANALYSIS_LIMITATION_DETAIL_CHARS,
};
use crate::domain::Finding;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Taxonomy name JSON consumers can key on. Distinct from #4722's
/// changed-file `producer_failure` limitation.
pub(crate) const LIMITATION_NAME: &str = "rust_lexical_test_index_partial";

const RECOVERY: &str = "An unchanged test file was indexed by lexical fallback after the reference parser refused it, so related-test evidence for a classified owner was weaker or missing. Check whether tests in the named file already exercise the change before trusting a no-path or weak result; rewrite nightly-only syntax or move those tests to a parser-valid file.";

const SAMPLE_FILES: usize = 3;

pub(super) fn limitation_for_consulted_unchanged_lexical_tests(
    index: &RustIndex,
    findings: &[Finding],
    changed_paths: &[PathBuf],
    workspace_root: &Path,
) -> Result<Option<AnalysisLimitation>, String> {
    let gaps = consulted_gaps(index, findings, changed_paths, workspace_root);
    summary_limitation(&gaps)
}

fn consulted_gaps(
    index: &RustIndex,
    findings: &[Finding],
    changed_paths: &[PathBuf],
    workspace_root: &Path,
) -> Vec<PathBuf> {
    if findings.is_empty() {
        return Vec::new();
    }
    let changed = changed_paths
        .iter()
        .map(|path| repo_relative(path, workspace_root))
        .collect::<BTreeSet<_>>();
    let owners = findings
        .iter()
        .filter_map(|finding| {
            let name = owner_name_from_id(&finding.probe.owner, &finding.probe.location.file)?;
            Some((
                name,
                repo_relative(&finding.probe.location.file, workspace_root),
            ))
        })
        .collect::<Vec<_>>();
    let related_files = findings
        .iter()
        .flat_map(|finding| finding.related_tests.iter())
        .map(|test| repo_relative(&test.file, workspace_root))
        .collect::<BTreeSet<_>>();

    let mut gaps = BTreeSet::new();
    for (path, facts) in &index.files {
        let normalized = repo_relative(path, workspace_root);
        if !facts.used_lexical_fallback || changed.contains(&normalized) {
            continue;
        }
        if !is_test_evidence_file(path, facts) {
            continue;
        }
        let contributed = related_files.contains(&normalized);
        let lost_owner_call = owners.iter().any(|(owner, owner_file)| {
            crate_key(owner_file) == crate_key(&normalized)
                && source_contains_owner_invocation(&facts.source, owner)
        });
        if contributed || lost_owner_call {
            gaps.insert(PathBuf::from(&normalized));
        }
    }
    gaps.into_iter().collect()
}

fn is_test_evidence_file(path: &Path, facts: &FileFacts) -> bool {
    rust_index::is_test_file(path)
        || !facts.tests.is_empty()
        || source_registers_executable_test(&facts.source)
}

fn source_registers_executable_test(source: &str) -> bool {
    source.lines().any(line_registers_executable_test)
}

fn line_registers_executable_test(line: &str) -> bool {
    let trimmed = line.trim_start();
    let Some(after_attr) = trimmed.strip_prefix("#[") else {
        return false;
    };
    let inner = after_attr
        .split_once(']')
        .map(|(inner, _)| inner)
        .unwrap_or(after_attr)
        .trim();
    let name = inner
        .split(|ch: char| ch == '(' || ch.is_whitespace())
        .next()
        .unwrap_or("")
        .trim();
    name == "test"
        || name == "tokio::test"
        || name == "async_std::test"
        || name == "rstest"
        || name.starts_with("rstest::")
}

/// Path prefix before `src`/`tests`/`benches`/`examples`. Empty for a
/// crate-root layout (`src/lib.rs`, `tests/price.rs`). Cross-crate name
/// coincidence must not mark this crate's run partial; extracted
/// related-tests still arrive through `contributed`.
fn repo_relative(path: &Path, workspace_root: &Path) -> String {
    let normalized = workspace::normalize_path(path);
    let root = workspace::normalize_path(workspace_root);
    let root = root.trim_end_matches('/');
    if root.is_empty() {
        return normalized;
    }
    normalized
        .strip_prefix(root)
        .map(|rest| rest.trim_start_matches('/').to_string())
        .filter(|rest| !rest.is_empty())
        .unwrap_or(normalized)
}

fn crate_key(path: &str) -> &str {
    for marker in ["/src/", "/tests/", "/benches/", "/examples/"] {
        if let Some(idx) = path.find(marker) {
            return &path[..idx];
        }
    }
    for prefix in ["src/", "tests/", "benches/", "examples/"] {
        if path.starts_with(prefix) {
            return "";
        }
    }
    path.rsplit_once('/').map(|(dir, _)| dir).unwrap_or(path)
}

fn source_contains_owner_invocation(source: &str, owner: &str) -> bool {
    if owner.is_empty() {
        return false;
    }
    let body = mask_rust_comments_and_strings(source);
    body.match_indices(owner).any(|(start, _)| {
        let end = start.saturating_add(owner.len());
        let before_ok = start == 0
            || !body
                .as_bytes()
                .get(start.saturating_sub(1))
                .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_');
        if !before_ok || is_function_declaration_name(&body[..start]) {
            return false;
        }
        let Some(tail) = body.get(end..) else {
            return false;
        };
        let tail = tail.trim_start();
        tail.starts_with('(') || turbofish_then_call(tail)
    })
}

fn is_function_declaration_name(prefix: &str) -> bool {
    let trimmed = prefix.trim_end();
    let Some(before_fn) = trimmed.strip_suffix("fn") else {
        return false;
    };
    before_fn.is_empty()
        || before_fn
            .as_bytes()
            .last()
            .is_some_and(|byte| !(byte.is_ascii_alphanumeric() || *byte == b'_'))
}

fn turbofish_then_call(tail: &str) -> bool {
    let Some(after_colon) = tail.strip_prefix("::") else {
        return false;
    };
    let after_colon = after_colon.trim_start();
    skip_angle_group(after_colon).is_some_and(|rest| rest.trim_start().starts_with('('))
}

fn skip_angle_group(source: &str) -> Option<&str> {
    if !source.starts_with('<') {
        return None;
    }
    let mut depth = 0_usize;
    for (idx, ch) in source.char_indices() {
        match ch {
            '<' => depth = depth.saturating_add(1),
            '>' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return source.get(idx.saturating_add(ch.len_utf8())..);
                }
            }
            _ => {}
        }
    }
    None
}

fn summary_limitation(gaps: &[PathBuf]) -> Result<Option<AnalysisLimitation>, String> {
    let recovery =
        || AnalysisRecovery::new(AnalysisRecoveryKind::InspectFailure, RECOVERY.to_string());
    match gaps {
        [] => Ok(None),
        [path] => {
            let normalized = workspace::normalize_path(path);
            let head = format!(
                "{LIMITATION_NAME}: unchanged lexical-fallback test file contributed or lost related-test evidence for a classified owner"
            );
            let preferred = format!("{head}: {normalized}");
            Ok(Some(
                AnalysisLimitation::new(
                    AnalysisLimitationKind::LanguageScopeUnsupported,
                    AnalysisStage::LanguageAdapter,
                    recovery()?,
                )
                .with_path(normalized)?
                .with_affected_items(1)?
                .with_detail(bounded_detail(preferred, head))?,
            ))
        }
        _ => {
            let files = gaps
                .iter()
                .map(|path| workspace::normalize_path(path))
                .collect::<BTreeSet<_>>();
            let samples = files.iter().take(SAMPLE_FILES).cloned().collect::<Vec<_>>();
            let remainder = files.len().saturating_sub(samples.len());
            let head = format!(
                "{LIMITATION_NAME}: {} unchanged test file(s) were indexed by lexical fallback while related-test evidence was consulted",
                files.len()
            );
            let mut preferred = format!("{head}; e.g. {}", samples.join(", "));
            if remainder > 0 {
                preferred.push_str(&format!(" (+{remainder} more)"));
            }
            Ok(Some(
                AnalysisLimitation::new(
                    AnalysisLimitationKind::LanguageScopeUnsupported,
                    AnalysisStage::LanguageAdapter,
                    recovery()?,
                )
                .with_affected_items(u64::try_from(files.len()).unwrap_or(u64::MAX))?
                .with_detail(bounded_detail(preferred, head))?,
            ))
        }
    }
}

fn bounded_detail(preferred: String, fallback: String) -> String {
    if preferred.chars().count() <= MAX_ANALYSIS_LIMITATION_DETAIL_CHARS {
        preferred
    } else {
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::diff;
    use crate::analysis::facts::TestFact;
    use crate::analysis::language::{LanguageAdapter, LanguageId};
    use crate::analysis::syntax::{RaRustSyntaxAdapter, RustSyntaxAdapter};
    use crate::analysis::{AnalysisMode, AnalysisOptions};
    use crate::config::OraclePolicy;
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, Probe, ProbeFamily, ProbeId,
        RelatedTest, RevealEvidence, RiprEvidence, SourceCurrentness, SourceLocation,
        StageEvidence, StageState, SymbolId,
    };
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    const PARSER_REFUSAL: &str = "\nfn refuse_reference_parser(\n";

    fn temp_root(name: &str) -> Result<PathBuf, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("ripr-lexical-test-grip-{name}-{stamp}"));
        fs::create_dir_all(&root).map_err(|err| format!("create temp root failed: {err}"))?;
        Ok(root)
    }

    fn write(path: &Path, text: &str) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| format!("create parent failed: {err}"))?;
        }
        fs::write(path, text).map_err(|err| format!("write {} failed: {err}", path.display()))
    }

    fn analysis_options(root: PathBuf) -> AnalysisOptions {
        AnalysisOptions {
            root,
            base: None,
            diff_file: None,
            mode: AnalysisMode::Draft,
            resolved_subject_identity: None,
            open_rust_index_paths: Default::default(),
            include_unchanged_tests: true,
            resolve_tsconfig_paths: false,
            perl_facts_path: None,
            git_timeout: None,
            git_candidate: None,
            production_like_targets: Default::default(),
            test_harnesses: Vec::new(),
        }
    }

    fn price_lib() -> &'static str {
        "pub fn price(total: u32, d: u32) -> u32 {\n    if total >= 100 { total - d } else { total }\n}\n"
    }

    fn price_diff() -> &'static str {
        "diff --git a/src/lib.rs b/src/lib.rs\n\
         new file mode 100644\n\
         --- /dev/null\n\
         +++ b/src/lib.rs\n\
         @@ -0,0 +1,3 @@\n\
         +pub fn price(total: u32, d: u32) -> u32 {\n\
         +    if total >= 100 { total - d } else { total }\n\
         +}\n"
    }

    fn compact_owner_test(owner: &str) -> String {
        format!(
            "use demo::{owner};\n#[test] fn p() {{ assert_eq!({owner}(200, 10), 190); }}\n{PARSER_REFUSAL}"
        )
    }

    fn formatted_owner_test(owner: &str) -> String {
        format!(
            "use demo::{owner};\n#[test]\nfn p() {{\n    assert_eq!({owner}(200, 10), 190);\n}}\n{PARSER_REFUSAL}"
        )
    }

    fn formatted_parser_ok_owner_test(owner: &str) -> String {
        format!(
            "use demo::{owner};\n#[test]\nfn p() {{\n    assert_eq!({owner}(200, 10), 190);\n}}\n"
        )
    }

    fn write_demo_crate(root: &Path, test_files: &[(&str, &str)]) -> Result<(), String> {
        write(
            &root.join("Cargo.toml"),
            "[package]\nname='demo'\nversion='0.1.0'\nedition='2024'\n",
        )?;
        write(&root.join("src/lib.rs"), price_lib())?;
        for (path, source) in test_files {
            write(&root.join(path), source)?;
        }
        Ok(())
    }

    fn analyze_price_diff(
        root: PathBuf,
    ) -> Result<crate::analysis::language::LanguageDiffResult, String> {
        let changed_files = diff::parse_unified_diff(price_diff());
        super::super::RustAdapter.analyze_diff(
            &analysis_options(root),
            &OraclePolicy::default(),
            &changed_files,
        )
    }

    fn limitation_details(result: &crate::analysis::language::LanguageDiffResult) -> Vec<String> {
        result
            .limitations
            .iter()
            .filter_map(|limitation| limitation.bounded_detail.clone())
            .collect()
    }

    fn names_this_limitation(result: &crate::analysis::language::LanguageDiffResult) -> bool {
        limitation_details(result)
            .iter()
            .any(|detail| detail.contains(LIMITATION_NAME))
    }

    fn names_producer_failure(result: &crate::analysis::language::LanguageDiffResult) -> bool {
        result.limitations.iter().any(|limitation| {
            limitation.kind == crate::analysis_outcome::AnalysisLimitationKind::ProducerFailure
        })
    }

    fn fallback_file(path: &str, source: &str, test_names: &[&str]) -> FileFacts {
        FileFacts {
            path: PathBuf::from(path),
            functions: Vec::new(),
            tests: test_names
                .iter()
                .map(|name| TestFact {
                    name: (*name).to_string(),
                    file: PathBuf::from(path),
                    start_line: 1,
                    end_line: 2,
                    body: source.to_string(),
                    calls: Vec::new(),
                    assertions: Vec::new(),
                    literals: Vec::new(),
                    attrs: Vec::new(),
                    nested_fn_names: Vec::new(),
                    let_bindings: Vec::new(),
                })
                .collect(),
            calls: Vec::new(),
            returns: Vec::new(),
            literals: Vec::new(),
            probe_shapes: Vec::new(),
            used_lexical_fallback: true,
            module_declarations: Vec::new(),
            unresolved_property_macros: Vec::new(),
            role_provenance: Default::default(),
            source: source.to_string(),
        }
    }

    fn price_finding(related_file: Option<&str>) -> Finding {
        let stage = |state| StageEvidence::new(state, Confidence::Low, "stage");
        Finding {
            id: "probe:src_lib.rs:predicate:price".to_string(),
            canonical_gap: None,
            probe: Probe {
                id: ProbeId("probe:src_lib.rs:predicate:price".to_string()),
                location: SourceLocation::new("src/lib.rs", 2, 1),
                owner: Some(SymbolId("src/lib.rs::price".to_string())),
                family: ProbeFamily::Predicate,
                delta: DeltaKind::Control,
                before: None,
                after: Some("if total >= 100 {".to_string()),
                expression: "if total >= 100 {".to_string(),
                expected_sinks: Vec::new(),
                required_oracles: Vec::new(),
            },
            class: ExposureClass::NoStaticPath,
            ripr: RiprEvidence {
                reach: stage(StageState::No),
                infect: stage(StageState::Unknown),
                propagate: stage(StageState::Yes),
                reveal: RevealEvidence {
                    observe: stage(StageState::No),
                    discriminate: stage(StageState::No),
                },
            },
            confidence: 0.4,
            evidence: Vec::new(),
            missing: Vec::new(),
            flow_sinks: Vec::new(),
            activation: ActivationEvidence::default(),
            stop_reasons: Vec::new(),
            related_tests: related_file
                .map(|file| RelatedTest {
                    name: "p".to_string(),
                    file: PathBuf::from(file),
                    line: 2,
                    oracle: None,
                    oracle_kind: crate::domain::OracleKind::Unknown,
                    oracle_strength: crate::domain::OracleStrength::None,
                    relation_reason: None,
                    relation_confidence: None,
                })
                .into_iter()
                .collect(),
            recommended_next_step: None,
            language: Some(LanguageId::Rust),
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

    fn require_named_limitation(
        gaps: Vec<PathBuf>,
        findings: &[Finding],
        changed: &[PathBuf],
    ) -> Result<AnalysisLimitation, String> {
        let mut index = RustIndex::default();
        for path in &gaps {
            let source = format!("#[test] fn p() {{ price(200, 10); }}\n{PARSER_REFUSAL}");
            index.files.insert(
                path.clone(),
                fallback_file(&path.to_string_lossy(), &source, &[]),
            );
        }
        limitation_for_consulted_unchanged_lexical_tests(&index, findings, changed, Path::new(""))?
            .ok_or_else(|| "expected a rust_lexical_test_index_partial limitation".to_string())
    }

    #[test]
    fn detector_names_a_dropped_owner_call_in_an_unchanged_fallback_test_file() -> Result<(), String>
    {
        let limitation = require_named_limitation(
            vec![PathBuf::from("tests/price.rs")],
            &[price_finding(None)],
            &[PathBuf::from("src/lib.rs")],
        )?;
        assert_eq!(
            limitation.kind,
            AnalysisLimitationKind::LanguageScopeUnsupported
        );
        assert_eq!(limitation.path.as_deref(), Some("tests/price.rs"));
        let detail = limitation.bounded_detail.as_deref().unwrap_or_default();
        assert!(
            detail.starts_with(LIMITATION_NAME),
            "detail must carry the taxonomy name, got {detail}"
        );
        Ok(())
    }

    #[test]
    fn detector_names_a_fallback_file_that_already_contributed_related_tests() -> Result<(), String>
    {
        let mut index = RustIndex::default();
        index.files.insert(
            PathBuf::from("tests/price.rs"),
            fallback_file(
                "tests/price.rs",
                "#[test]\nfn p() {\n    helper();\n}\n",
                &["p"],
            ),
        );
        let limitation = limitation_for_consulted_unchanged_lexical_tests(
            &index,
            &[price_finding(Some("tests/price.rs"))],
            &[PathBuf::from("src/lib.rs")],
            Path::new(""),
        )?
        .ok_or_else(|| "contributed fallback evidence must be limited".to_string())?;
        assert_eq!(limitation.path.as_deref(), Some("tests/price.rs"));
        Ok(())
    }

    #[test]
    fn detector_ignores_an_unrelated_fallback_file_that_does_not_name_the_owner()
    -> Result<(), String> {
        let mut index = RustIndex::default();
        index.files.insert(
            PathBuf::from("tests/nightly.rs"),
            fallback_file(
                "tests/nightly.rs",
                "#[test] fn p() { other(1); }\nfn refuse(\n",
                &[],
            ),
        );
        let limitation = limitation_for_consulted_unchanged_lexical_tests(
            &index,
            &[price_finding(None)],
            &[PathBuf::from("src/lib.rs")],
            Path::new(""),
        )?;
        assert!(
            limitation.is_none(),
            "an unused nightly-syntax file must not make the run partial, got {limitation:?}"
        );
        Ok(())
    }

    #[test]
    fn detector_ignores_owner_mentions_inside_comments_and_strings() -> Result<(), String> {
        let mut index = RustIndex::default();
        index.files.insert(
            PathBuf::from("tests/nightly.rs"),
            fallback_file(
                "tests/nightly.rs",
                "#[test] fn p() { other(1); }\n// price(200, 10)\nlet _ = \"price(\";\nfn refuse(\n",
                &[],
            ),
        );
        let limitation = limitation_for_consulted_unchanged_lexical_tests(
            &index,
            &[price_finding(None)],
            &[PathBuf::from("src/lib.rs")],
            Path::new(""),
        )?;
        assert!(
            limitation.is_none(),
            "comment/string mentions must not count as lost related-test evidence, got {limitation:?}"
        );
        Ok(())
    }

    #[test]
    fn detector_skips_changed_files_so_it_does_not_absorb_producer_failure() -> Result<(), String> {
        let mut index = RustIndex::default();
        index.files.insert(
            PathBuf::from("tests/price.rs"),
            fallback_file(
                "tests/price.rs",
                "#[test] fn p() { price(200, 10); }\nfn refuse(\n",
                &[],
            ),
        );
        let limitation = limitation_for_consulted_unchanged_lexical_tests(
            &index,
            &[price_finding(None)],
            &[PathBuf::from("tests/price.rs")],
            Path::new(""),
        )?;
        assert!(
            limitation.is_none(),
            "changed lexical-fallback files belong to #4722, got {limitation:?}"
        );
        Ok(())
    }

    #[test]
    fn detector_stays_quiet_when_no_owner_was_classified() -> Result<(), String> {
        let mut index = RustIndex::default();
        index.files.insert(
            PathBuf::from("tests/price.rs"),
            fallback_file(
                "tests/price.rs",
                "#[test] fn p() { price(200, 10); }\nfn refuse(\n",
                &[],
            ),
        );
        let limitation = limitation_for_consulted_unchanged_lexical_tests(
            &index,
            &[],
            &[PathBuf::from("src/lib.rs")],
            Path::new(""),
        )?;
        assert!(limitation.is_none());
        Ok(())
    }

    #[test]
    fn detector_collapses_several_consulted_files_into_one_summary() -> Result<(), String> {
        let limitation = require_named_limitation(
            vec![
                PathBuf::from("tests/a.rs"),
                PathBuf::from("tests/b.rs"),
                PathBuf::from("tests/c.rs"),
                PathBuf::from("tests/d.rs"),
                PathBuf::from("tests/e.rs"),
            ],
            &[price_finding(None)],
            &[PathBuf::from("src/lib.rs")],
        )?;
        assert!(limitation.path.is_none());
        assert_eq!(limitation.affected_items, Some(5));
        let detail = limitation.bounded_detail.as_deref().unwrap_or_default();
        assert!(
            detail.starts_with(
                "rust_lexical_test_index_partial: 5 unchanged test file(s) were indexed by lexical fallback while related-test evidence was consulted; e.g. "
            ) && detail.ends_with("(+2 more)"),
            "{detail}"
        );
        Ok(())
    }

    fn price_finding_at(owner_file: &str, related_file: Option<&str>) -> Finding {
        let mut finding = price_finding(related_file);
        finding.probe.location = SourceLocation::new(owner_file, 2, 1);
        finding.probe.owner = Some(SymbolId(format!("{owner_file}::price")));
        finding.id = format!("probe:{}:predicate:price", owner_file.replace('/', "_"));
        finding
    }

    #[test]
    fn detector_ignores_an_unrelated_fn_declaration_with_the_owner_name() -> Result<(), String> {
        let mut index = RustIndex::default();
        index.files.insert(
            PathBuf::from("tests/nightly.rs"),
            fallback_file(
                "tests/nightly.rs",
                "#[test] fn p() { other(1); }\nfn price() {}\nfn refuse(\n",
                &[],
            ),
        );
        let limitation = limitation_for_consulted_unchanged_lexical_tests(
            &index,
            &[price_finding(None)],
            &[PathBuf::from("src/lib.rs")],
            Path::new(""),
        )?;
        assert!(
            limitation.is_none(),
            "a function declaration must not count as a lost owner call, got {limitation:?}"
        );
        Ok(())
    }

    #[test]
    fn detector_names_a_dropped_turbofish_owner_call() -> Result<(), String> {
        let mut index = RustIndex::default();
        index.files.insert(
            PathBuf::from("tests/price.rs"),
            fallback_file(
                "tests/price.rs",
                "#[test] fn p() { price::<u32>(200, 10); }\nfn refuse(\n",
                &[],
            ),
        );
        let limitation = limitation_for_consulted_unchanged_lexical_tests(
            &index,
            &[price_finding(None)],
            &[PathBuf::from("src/lib.rs")],
            Path::new(""),
        )?
        .ok_or_else(|| {
            "turbofish owner calls must count as lost related-test evidence".to_string()
        })?;
        assert_eq!(limitation.path.as_deref(), Some("tests/price.rs"));
        Ok(())
    }

    #[test]
    fn detector_ignores_a_same_named_call_in_another_crate() -> Result<(), String> {
        let mut index = RustIndex::default();
        index.files.insert(
            PathBuf::from("crate_b/tests/nightly.rs"),
            fallback_file(
                "crate_b/tests/nightly.rs",
                "#[test] fn p() { price(1); }\nfn refuse(\n",
                &[],
            ),
        );
        let limitation = limitation_for_consulted_unchanged_lexical_tests(
            &index,
            &[price_finding_at("crate_a/src/lib.rs", None)],
            &[PathBuf::from("crate_a/src/lib.rs")],
            Path::new(""),
        )?;
        assert!(
            limitation.is_none(),
            "a same-named call in another crate must not make this crate partial, got {limitation:?}"
        );
        Ok(())
    }

    #[test]
    fn detector_names_a_same_crate_call_under_a_package_prefix() -> Result<(), String> {
        let mut index = RustIndex::default();
        index.files.insert(
            PathBuf::from("crate_a/tests/price.rs"),
            fallback_file(
                "crate_a/tests/price.rs",
                "#[test] fn p() { price(200, 10); }\nfn refuse(\n",
                &[],
            ),
        );
        let limitation = limitation_for_consulted_unchanged_lexical_tests(
            &index,
            &[price_finding_at("crate_a/src/lib.rs", None)],
            &[PathBuf::from("crate_a/src/lib.rs")],
            Path::new(""),
        )?
        .ok_or_else(|| "same-crate dropped owner calls must still be limited".to_string())?;
        assert_eq!(limitation.path.as_deref(), Some("crate_a/tests/price.rs"));
        Ok(())
    }

    #[test]
    fn detector_matches_absolute_index_paths_against_a_relative_owner() -> Result<(), String> {
        let root = PathBuf::from("/tmp/demo-crate");
        let mut index = RustIndex::default();
        index.files.insert(
            root.join("tests/price.rs"),
            fallback_file(
                "tests/price.rs",
                "#[test] fn p() { price(200, 10); }\nfn refuse(\n",
                &[],
            ),
        );
        let limitation = limitation_for_consulted_unchanged_lexical_tests(
            &index,
            &[price_finding(None)],
            &[PathBuf::from("src/lib.rs")],
            &root,
        )?
        .ok_or_else(|| {
            "workspace-root stripping must still see a same-crate dropped owner call".to_string()
        })?;
        assert_eq!(limitation.path.as_deref(), Some("tests/price.rs"));
        Ok(())
    }

    #[test]
    fn detector_names_a_spaced_test_attribute_outside_tests_dir() -> Result<(), String> {
        let mut index = RustIndex::default();
        index.files.insert(
            PathBuf::from("src/discount_tests.rs"),
            fallback_file(
                "src/discount_tests.rs",
                "#[ test ] fn check() { price(200, 10); }\nfn refuse(\n",
                &[],
            ),
        );
        let limitation = limitation_for_consulted_unchanged_lexical_tests(
            &index,
            &[price_finding(None)],
            &[PathBuf::from("src/lib.rs")],
            Path::new(""),
        )?
        .ok_or_else(|| {
            "spaced #[ test ] outside tests/ must still count as test evidence".to_string()
        })?;
        assert_eq!(limitation.path.as_deref(), Some("src/discount_tests.rs"));
        Ok(())
    }

    #[test]
    fn summary_limitation_bounds_a_long_single_file_path() -> Result<(), String> {
        let long_dir = "a".repeat(220);
        let path = PathBuf::from(format!("tests/{long_dir}/{long_dir}/case.rs"));
        let limitation = require_named_limitation(
            vec![path.clone()],
            &[price_finding(None)],
            &[PathBuf::from("src/lib.rs")],
        )?;
        let expected_path = workspace::normalize_path(&path);
        assert_eq!(limitation.path.as_deref(), Some(expected_path.as_str()));
        let detail = limitation.bounded_detail.as_deref().unwrap_or_default();
        assert!(
            detail.starts_with(LIMITATION_NAME),
            "bounded detail must keep the taxonomy name, got {detail}"
        );
        assert!(
            detail.chars().count() <= MAX_ANALYSIS_LIMITATION_DETAIL_CHARS,
            "single-file detail must not abort analysis, got {} chars",
            detail.chars().count()
        );
        Ok(())
    }

    #[test]
    fn compact_unchanged_unparseable_test_file_is_a_partial_adapter_result() -> Result<(), String> {
        let source = compact_owner_test("price");
        RaRustSyntaxAdapter
            .summarize_file(Path::new("tests/price.rs"), &source)
            .err()
            .ok_or_else(|| {
                "fixture precondition: the reference parser must refuse the compact test file"
                    .to_string()
            })?;

        let root = temp_root("compact-dropout")?;
        write_demo_crate(&root, &[("tests/price.rs", source.as_str())])?;
        let result = analyze_price_diff(root.clone())?;
        assert!(
            names_this_limitation(&result),
            "dropped tests in an unchanged lexical-fallback file must be a typed limitation, got {:?}",
            result.limitations
        );
        assert!(
            result.findings.iter().any(|finding| {
                finding.class == ExposureClass::NoStaticPath && finding.related_tests.is_empty()
            }),
            "the compact lexical scan must actually drop the discriminating test: {:?}",
            result.findings
        );
        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn formatted_unchanged_unparseable_test_file_is_still_limited_once_it_relates()
    -> Result<(), String> {
        let source = formatted_owner_test("price");
        RaRustSyntaxAdapter
            .summarize_file(Path::new("tests/price.rs"), &source)
            .err()
            .ok_or_else(|| {
                "fixture precondition: the reference parser must refuse the formatted test file"
                    .to_string()
            })?;

        let root = temp_root("formatted-contributed")?;
        write_demo_crate(&root, &[("tests/price.rs", source.as_str())])?;
        let result = analyze_price_diff(root.clone())?;
        assert!(
            names_this_limitation(&result),
            "lexical-fallback related-test evidence must still mark the run limited, got {:?}",
            result.limitations
        );
        assert!(
            result.findings.iter().any(|finding| finding
                .related_tests
                .iter()
                .any(|test| { workspace::normalize_path(&test.file) == "tests/price.rs" })),
            "the formatted lexical scan must still extract the test: {:?}",
            result.findings
        );
        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn unrelated_unchanged_unparseable_test_file_does_not_make_the_crate_partial()
    -> Result<(), String> {
        let nightly = compact_owner_test("other");
        let ok = formatted_parser_ok_owner_test("price");
        RaRustSyntaxAdapter
            .summarize_file(Path::new("tests/nightly.rs"), &nightly)
            .err()
            .ok_or_else(|| {
                "fixture precondition: nightly.rs must fail the reference parser".to_string()
            })?;
        RaRustSyntaxAdapter
            .summarize_file(Path::new("tests/ok.rs"), &ok)
            .map_err(|err| format!("fixture precondition: ok.rs must parse: {err}"))?;

        let root = temp_root("unrelated-nightly")?;
        write_demo_crate(
            &root,
            &[
                ("tests/nightly.rs", nightly.as_str()),
                ("tests/ok.rs", ok.as_str()),
            ],
        )?;
        let result = analyze_price_diff(root.clone())?;
        assert!(
            !names_this_limitation(&result),
            "an unused nightly-syntax file must not make a crate with a parser-backed discriminator partial, got {:?}",
            result.limitations
        );
        assert!(
            result
                .findings
                .iter()
                .any(|finding| !finding.related_tests.is_empty()),
            "the parser-backed test must still relate: {:?}",
            result.findings
        );
        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn parser_backed_unchanged_test_file_stays_complete() -> Result<(), String> {
        let source = formatted_parser_ok_owner_test("price");
        RaRustSyntaxAdapter
            .summarize_file(Path::new("tests/price.rs"), &source)
            .map_err(|err| format!("fixture precondition: parser-ok file must parse: {err}"))?;

        let root = temp_root("parser-ok")?;
        write_demo_crate(&root, &[("tests/price.rs", source.as_str())])?;
        let result = analyze_price_diff(root.clone())?;
        assert!(
            !names_this_limitation(&result),
            "parser-backed related tests must not be reported as lexical-fallback gaps, got {:?}",
            result.limitations
        );
        assert!(
            result
                .findings
                .iter()
                .any(|finding| !finding.related_tests.is_empty()),
            "parser-backed tests must relate: {:?}",
            result.findings
        );
        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn changed_unparseable_test_file_is_not_this_limitation() -> Result<(), String> {
        let source = compact_owner_test("price");
        let root = temp_root("changed-test-file")?;
        write_demo_crate(&root, &[("tests/price.rs", source.as_str())])?;
        let changed_files = diff::parse_unified_diff(
            "diff --git a/tests/price.rs b/tests/price.rs\n\
             new file mode 100644\n\
             --- /dev/null\n\
             +++ b/tests/price.rs\n\
             @@ -0,0 +1,3 @@\n\
             +use demo::price;\n\
             +#[test] fn p() { assert_eq!(price(200, 10), 190); }\n\
             +fn refuse_reference_parser(\n",
        );
        let result = super::super::RustAdapter.analyze_diff(
            &analysis_options(root.clone()),
            &OraclePolicy::default(),
            &changed_files,
        )?;
        assert!(
            !names_this_limitation(&result),
            "a changed test file is #4722, not #4775, got {:?}",
            result.limitations
        );
        assert!(
            names_producer_failure(&result),
            "the #4722 producer_failure limitation must still be present for a changed unparseable file, got {:?}",
            result.limitations
        );
        let _ = fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn producer_failure_does_not_drop_unchanged_lexical_test_limitation() -> Result<(), String> {
        let source = compact_owner_test("price");
        let broken = format!("pub fn noise() {{}}\n{PARSER_REFUSAL}");
        RaRustSyntaxAdapter
            .summarize_file(Path::new("tests/price.rs"), &source)
            .err()
            .ok_or_else(|| {
                "fixture precondition: tests/price.rs must fail the reference parser".to_string()
            })?;
        RaRustSyntaxAdapter
            .summarize_file(Path::new("src/broken.rs"), &broken)
            .err()
            .ok_or_else(|| {
                "fixture precondition: src/broken.rs must fail the reference parser".to_string()
            })?;

        let root = temp_root("compose-4722-4775")?;
        write_demo_crate(&root, &[("tests/price.rs", source.as_str())])?;
        write(&root.join("src/broken.rs"), &broken)?;
        let changed_files = diff::parse_unified_diff(&format!(
            "{}\
             diff --git a/src/broken.rs b/src/broken.rs\n\
             new file mode 100644\n\
             --- /dev/null\n\
             +++ b/src/broken.rs\n\
             @@ -0,0 +1,2 @@\n\
             +pub fn noise() {{}}\n\
             +fn refuse_reference_parser(\n",
            price_diff()
        ));
        let result = super::super::RustAdapter.analyze_diff(
            &analysis_options(root.clone()),
            &OraclePolicy::default(),
            &changed_files,
        )?;
        assert!(
            names_this_limitation(&result),
            "an unchanged consulted fallback test must still be #4775 when a changed file is producer_failure, got {:?}",
            result.limitations
        );
        assert!(
            names_producer_failure(&result),
            "composing #4775 must not drop #4722 producer_failure, got {:?}",
            result.limitations
        );
        let _ = fs::remove_dir_all(root);
        Ok(())
    }
}
