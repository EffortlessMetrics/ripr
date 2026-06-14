/// Advisory LSP codeLens for changed symbols.
///
/// RIPR-SPEC-0099: emits one advisory `CodeLens` per `Finding` whose probe
/// location maps to the requested document URI. The title cites the finding's
/// `related_tests.len()` from the cached `AnalysisSnapshot` — the same source
/// that feeds `finding.class` — so the count is never fabricated.
///
/// Honesty invariants (enforced by unit tests):
/// - `snapshot == None` → empty Vec (absence of analysis ≠ fabricated 0-count).
/// - Count comes solely from `finding.related_tests.len()`, never from the
///   diagnostic projection or any runtime test runner output.
/// - Static-language vocab only (approved RIPR exposure terms); no runtime
///   mutation-testing vocabulary in any title.
/// - Preview-tier findings (TS / Python — `language_status == Some(Preview)`)
///   are prefixed with `preview:` and softened so the count is not read as
///   confirmed by a static discriminator.
/// - N == 0 uses "no related tests found" + the class label, not the
///   forbidden unexercised-synonym that belongs to runtime mutation tooling.
use super::state::AnalysisSnapshot;
use super::uri::{file_uri_for_path, file_uris_match};
use crate::domain::{ExposureClass, Finding, LanguageStatus};
use std::path::Path;
use std::time::Duration;
use tower_lsp_server::ls_types::{CodeLens, Command, Position, Range, Uri};

/// Produce advisory `CodeLens` items for all findings that map to `uri`
/// inside `snapshot`.
///
/// Returns an empty Vec when `snapshot` is `None` — absence of an analysis
/// snapshot is not the same as confirmed absence of related tests.
pub(super) fn code_lens_response(uri: &Uri, snapshot: Option<&AnalysisSnapshot>) -> Vec<CodeLens> {
    let Some(snapshot) = snapshot else {
        return Vec::new();
    };
    let age = snapshot.refresh.age();
    snapshot
        .findings
        .iter()
        .filter(|finding| finding_matches_uri(finding, uri, &snapshot.root))
        .map(|finding| finding_to_code_lens(finding, age))
        .collect()
}

/// Build one `CodeLens` for a single finding.
///
/// The `command` field carries the advisory text as its `title`. Although the
/// LSP spec allows a CodeLens with `command == null` for display-only lenses,
/// many clients (including VS Code) only render the title when a `Command`
/// object is present. We emit a `Command` with an empty no-op id so the title
/// is displayed without triggering any action. `data` is `None` (no resolve
/// round-trip needed; `resolve_provider` is `false`).
fn finding_to_code_lens(finding: &Finding, age: Option<Duration>) -> CodeLens {
    let line = finding.probe.location.line.saturating_sub(1) as u32;
    let range = Range {
        start: Position { line, character: 0 },
        end: Position { line, character: 0 },
    };
    let title = related_test_lens_title(finding, age);
    CodeLens {
        range,
        command: Some(Command {
            title,
            command: String::new(),
            arguments: None,
        }),
        data: None,
    }
}

/// Decide whether a `Finding` belongs to the requested document URI.
///
/// Uses `file_uri_for_path` (the same helper used by `diagnostics.rs`) to
/// build the finding's canonical file URI, then compares with
/// `file_uris_match` for cross-platform correctness (Windows drive-letter
/// case insensitivity, percent-encoding normalization).
fn finding_matches_uri(finding: &Finding, uri: &Uri, root: &Path) -> bool {
    let file = &finding.probe.location.file;
    let absolute = if file.is_absolute() {
        file.clone()
    } else {
        root.join(file)
    };
    match file_uri_for_path(&absolute) {
        Ok(finding_uri) => file_uris_match(&finding_uri, uri),
        Err(_) => false,
    }
}

/// Build the advisory lens title for a finding.
///
/// Rules (all vocabulary passes `check-static-language`):
///
/// - Preview tier (`language_status == Some(Preview)`): prefix `preview:`,
///   use softened phrasing — the count may be incomplete for preview adapters.
/// - N > 0 + discriminating class (`Exposed`):
///   `ripr: N related tests (exposed) · static, cached`
/// - N > 0 + non-discriminating class (anything except `Exposed`):
///   `ripr: N related tests, no static discriminator (CLASS) · static, cached`
/// - N == 0: `ripr: no related tests found (CLASS) · static, cached`
/// - Appends `· as of Xs ago` when the snapshot age is known.
///
/// Vocabulary enforcement: titles must not contain runtime mutation-testing
/// terms (the gate bans those words in all tracked prose). // ripr-allow: static-language: doc describing the vocabulary constraint, not emitting the terms
pub(super) fn related_test_lens_title(finding: &Finding, age: Option<Duration>) -> String {
    let is_preview = finding
        .language_status
        .as_ref()
        .is_some_and(|s| *s == LanguageStatus::Preview);
    let class_label = finding.class.as_str();
    let count = finding.related_tests.len();

    let body = if is_preview {
        // Preview tier: count may be incomplete; soften claim.
        if count == 0 {
            format!("preview: no related tests found ({class_label}) · static, cached")
        } else {
            format!("preview: {count} related tests (preview, {class_label}) · static, cached")
        }
    } else {
        match (count, &finding.class) {
            (0, _) => {
                format!("ripr: no related tests found ({class_label}) · static, cached")
            }
            (n, ExposureClass::Exposed) => {
                format!("ripr: {n} related tests (exposed) · static, cached")
            }
            (n, _) => {
                format!(
                    "ripr: {n} related tests, no static discriminator ({class_label}) · static, cached"
                )
            }
        }
    };

    match age {
        Some(d) => format!("{body} · as of {}s ago", d.as_secs()),
        None => body,
    }
}

/// Check that a generated lens title contains none of the forbidden
/// static-language vocabulary terms. Used by tests.
#[cfg(test)]
pub(super) fn lens_title_is_static_language_clean(title: &str) -> bool {
    // These string literals are test guards checking that the words do not
    // appear in production output — the strings themselves are not output.
    let forbidden = [
        "killed", // ripr-allow: static-language: test guard verifying this term does not appear in lens output
        "survived", // ripr-allow: static-language: test guard verifying this term does not appear in lens output
        "proven", // ripr-allow: static-language: test guard verifying this term does not appear in lens output
        "adequate", // ripr-allow: static-language: test guard verifying this term does not appear in lens output
        "covered", // ripr-allow: static-language: test guard verifying this term does not appear in lens output
        "passing", // ripr-allow: static-language: test guard verifying this term does not appear in lens output
        "untested", // ripr-allow: static-language: test guard verifying this term does not appear in lens output
    ];
    !forbidden.iter().any(|word| title.contains(word))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Mode;
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, Finding, LanguageStatus,
        OracleKind, OracleStrength, Probe, ProbeFamily, ProbeId, RelatedTest, RevealEvidence,
        RiprEvidence, SourceLocation, StageEvidence, StageState, SymbolId,
    };
    use crate::lsp::gap_artifacts::{GapArtifactRejection, ValidatedGapArtifact};
    use crate::lsp::state::{AnalysisSnapshot, RefreshMetadata};
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use tower_lsp_server::ls_types::{Diagnostic, Position, Range};

    fn parse_uri(s: &str) -> Result<Uri, String> {
        s.parse()
            .map_err(|e| format!("failed to parse test URI {s}: {e}"))
    }

    fn make_related_test(i: usize) -> RelatedTest {
        RelatedTest {
            name: format!("test_{i}"),
            file: PathBuf::from("tests/lib.rs"),
            line: 10 + i,
            oracle: None,
            oracle_kind: OracleKind::Unknown,
            oracle_strength: OracleStrength::Weak,
            relation_reason: None,
            relation_confidence: None,
        }
    }

    fn make_finding(
        file: &str,
        line: usize,
        class: ExposureClass,
        related_test_count: usize,
        language_status: Option<LanguageStatus>,
    ) -> Finding {
        let related_tests = (0..related_test_count).map(make_related_test).collect();
        Finding {
            id: format!("probe:{file}:{line}:test_family:aabbccdd"),
            canonical_gap: None,
            probe: Probe {
                id: ProbeId(format!("probe:{file}:{line}:test_family:aabbccdd")),
                location: SourceLocation::new(file, line, 1),
                owner: Some(SymbolId("owner::fn".to_string())),
                family: ProbeFamily::Predicate,
                delta: DeltaKind::Value,
                before: None,
                after: Some("true".to_string()),
                expression: "x > 0".to_string(),
                expected_sinks: Vec::new(),
                required_oracles: Vec::new(),
            },
            class,
            ripr: RiprEvidence {
                reach: StageEvidence::new(StageState::Yes, Confidence::High, "reachable"),
                infect: StageEvidence::new(StageState::Yes, Confidence::High, "infectable"),
                propagate: StageEvidence::new(StageState::Yes, Confidence::Medium, "propagatable"),
                reveal: RevealEvidence {
                    observe: StageEvidence::new(StageState::Weak, Confidence::Medium, "observed"),
                    discriminate: StageEvidence::new(
                        StageState::Weak,
                        Confidence::Medium,
                        "discriminated",
                    ),
                },
            },
            confidence: 1.0,
            evidence: Vec::new(),
            missing: Vec::new(),
            flow_sinks: Vec::new(),
            activation: ActivationEvidence::default(),
            stop_reasons: Vec::new(),
            related_tests,
            recommended_next_step: None,
            language: None,
            language_status,
            owner_kind: None,
            static_limit_kind: None,
            changed_sink: None,
            observed_sink: None,
            oracle_alignment: None,
            alignment_reason: None,
        }
    }

    fn make_snapshot_with_findings(root: &str, findings: Vec<Finding>) -> AnalysisSnapshot {
        // Build a diagnostics_by_uri that satisfies is_consistent():
        // one diagnostic per finding, each carrying "finding_id".
        let mut diagnostics_by_uri = BTreeMap::new();
        for finding in &findings {
            let file = &finding.probe.location.file;
            let absolute = if file.is_absolute() {
                file.clone()
            } else {
                PathBuf::from(root).join(file)
            };
            if let Ok(uri) = file_uri_for_path(&absolute) {
                let line = finding.probe.location.line.saturating_sub(1) as u32;
                let diag = Diagnostic {
                    range: Range {
                        start: Position { line, character: 0 },
                        end: Position {
                            line,
                            character: 120,
                        },
                    },
                    severity: None,
                    code: None,
                    code_description: None,
                    source: Some("ripr".to_string()),
                    message: "test".to_string(),
                    related_information: None,
                    tags: None,
                    data: Some(serde_json::json!({ "finding_id": finding.id })),
                };
                diagnostics_by_uri
                    .entry(uri)
                    .or_insert_with(Vec::new)
                    .push(diag);
            }
        }
        AnalysisSnapshot {
            root: PathBuf::from(root),
            base: None,
            mode: Mode::Draft,
            refresh: RefreshMetadata::default(),
            findings,
            classified_seams: Vec::new(),
            gap_artifacts: Vec::<ValidatedGapArtifact>::new(),
            gap_artifact_rejections: Vec::<GapArtifactRejection>::new(),
            diagnostics_by_uri,
        }
    }

    #[test]
    fn code_lens_response_reports_real_related_test_count() -> Result<(), String> {
        let root = "/workspace";
        let finding = make_finding("src/lib.rs", 10, ExposureClass::Exposed, 3, None);
        let snapshot = make_snapshot_with_findings(root, vec![finding]);
        let uri = parse_uri("file:///workspace/src/lib.rs")?;

        let lenses = code_lens_response(&uri, Some(&snapshot));

        if lenses.len() != 1 {
            return Err(format!("expected exactly 1 lens, got {}", lenses.len()));
        }
        let title = lenses[0]
            .command
            .as_ref()
            .ok_or("expected command (title) in lens")?
            .title
            .clone();
        if !title.contains("3") {
            return Err(format!("title should cite 3 related tests, got: {title}"));
        }
        if !title.contains("exposed") {
            return Err(format!("title should cite class 'exposed', got: {title}"));
        }
        // Line should be 0-indexed (10 - 1 = 9)
        let line = lenses[0].range.start.line;
        if line != 9 {
            return Err(format!("expected lens at line 9 (0-indexed), got {line}"));
        }
        Ok(())
    }

    #[test]
    fn code_lens_response_zero_related_tests_is_honest() -> Result<(), String> {
        let root = "/workspace";
        let finding = make_finding("src/lib.rs", 5, ExposureClass::NoStaticPath, 0, None);
        let snapshot = make_snapshot_with_findings(root, vec![finding]);
        let uri = parse_uri("file:///workspace/src/lib.rs")?;

        let lenses = code_lens_response(&uri, Some(&snapshot));

        if lenses.len() != 1 {
            return Err(format!("expected 1 lens, got {}", lenses.len()));
        }
        let title = lenses[0]
            .command
            .as_ref()
            .ok_or("expected command in lens")?
            .title
            .clone();
        if !title.contains("no related tests found") {
            return Err(format!(
                "title must say 'no related tests found', got: {title}"
            ));
        }
        if !title.contains("no_static_path") {
            return Err(format!(
                "title must include class 'no_static_path', got: {title}"
            ));
        }
        // Test guard: confirm the lens title uses none of the forbidden
        // static-language vocabulary (delegates to the shared helper, which
        // holds the flagged terms on stable, fmt-safe array lines).
        if !lens_title_is_static_language_clean(&title) {
            return Err(format!(
                "title must not use forbidden static-language vocabulary, got: {title}"
            ));
        }
        Ok(())
    }

    #[test]
    fn code_lens_response_preview_finding_softens() -> Result<(), String> {
        let root = "/workspace";
        let finding = make_finding(
            "src/service.ts",
            20,
            ExposureClass::Exposed,
            1,
            Some(LanguageStatus::Preview),
        );
        let snapshot = make_snapshot_with_findings(root, vec![finding]);
        let uri = parse_uri("file:///workspace/src/service.ts")?;

        let lenses = code_lens_response(&uri, Some(&snapshot));

        if lenses.len() != 1 {
            return Err(format!("expected 1 lens, got {}", lenses.len()));
        }
        let title = lenses[0]
            .command
            .as_ref()
            .ok_or("expected command in lens")?
            .title
            .clone();
        if !title.contains("preview") {
            return Err(format!(
                "preview finding must have 'preview' in title, got: {title}"
            ));
        }
        Ok(())
    }

    #[test]
    fn code_lens_response_uncertain_class_does_not_claim_grip() -> Result<(), String> {
        let root = "/workspace";
        let finding = make_finding("src/lib.rs", 8, ExposureClass::ReachableUnrevealed, 2, None);
        let snapshot = make_snapshot_with_findings(root, vec![finding]);
        let uri = parse_uri("file:///workspace/src/lib.rs")?;

        let lenses = code_lens_response(&uri, Some(&snapshot));

        if lenses.len() != 1 {
            return Err(format!("expected 1 lens, got {}", lenses.len()));
        }
        let title = lenses[0]
            .command
            .as_ref()
            .ok_or("expected command in lens")?
            .title
            .clone();
        if !title.contains("no static discriminator") {
            return Err(format!(
                "uncertain class must state 'no static discriminator', got: {title}"
            ));
        }
        if !title.contains("reachable_unrevealed") {
            return Err(format!(
                "title must include class 'reachable_unrevealed', got: {title}"
            ));
        }
        Ok(())
    }

    #[test]
    fn code_lens_response_no_snapshot_emits_nothing() -> Result<(), String> {
        let uri = parse_uri("file:///workspace/src/lib.rs")?;
        let lenses = code_lens_response(&uri, None);
        if !lenses.is_empty() {
            return Err(
                "no snapshot must produce empty Vec, not a fabricated 0-count lens".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn code_lens_response_filters_to_requested_file() -> Result<(), String> {
        let root = "/workspace";
        let finding_a = make_finding("src/a.rs", 10, ExposureClass::Exposed, 2, None);
        let finding_b = make_finding("src/b.rs", 20, ExposureClass::NoStaticPath, 0, None);
        let snapshot = make_snapshot_with_findings(root, vec![finding_a, finding_b]);

        let uri_a = parse_uri("file:///workspace/src/a.rs")?;
        let lenses = code_lens_response(&uri_a, Some(&snapshot));

        if lenses.len() != 1 {
            return Err(format!(
                "expected 1 lens for file a (only its finding), got {}",
                lenses.len()
            ));
        }
        let title = lenses[0]
            .command
            .as_ref()
            .ok_or("expected command in lens")?
            .title
            .clone();
        if !title.contains("2") {
            return Err(format!(
                "lens for a.rs must cite 2 related tests, got: {title}"
            ));
        }
        Ok(())
    }

    #[test]
    fn code_lens_response_static_language_clean() -> Result<(), String> {
        let root = "/workspace";
        let classes: &[(ExposureClass, usize)] = &[
            (ExposureClass::Exposed, 3),
            (ExposureClass::WeaklyExposed, 1),
            (ExposureClass::ReachableUnrevealed, 2),
            (ExposureClass::NoStaticPath, 0),
            (ExposureClass::InfectionUnknown, 1),
            (ExposureClass::PropagationUnknown, 0),
            (ExposureClass::StaticUnknown, 0),
        ];
        for (i, (class, count)) in classes.iter().enumerate() {
            let finding = make_finding(&format!("src/file{i}.rs"), 10, class.clone(), *count, None);
            let snapshot = make_snapshot_with_findings(root, vec![finding]);
            let uri = parse_uri(&format!("file:///workspace/src/file{i}.rs"))?;
            let lenses = code_lens_response(&uri, Some(&snapshot));
            for lens in &lenses {
                if let Some(cmd) = &lens.command
                    && !lens_title_is_static_language_clean(&cmd.title)
                {
                    return Err(format!(
                        "forbidden vocab in lens title for class {:?}: {}",
                        class.as_str(),
                        cmd.title
                    ));
                }
            }
        }
        // Also check preview tier.
        let preview_finding = make_finding(
            "src/preview.ts",
            5,
            ExposureClass::Exposed,
            1,
            Some(LanguageStatus::Preview),
        );
        let snapshot = make_snapshot_with_findings(root, vec![preview_finding]);
        let uri = parse_uri("file:///workspace/src/preview.ts")?;
        let lenses = code_lens_response(&uri, Some(&snapshot));
        for lens in &lenses {
            if let Some(cmd) = &lens.command
                && !lens_title_is_static_language_clean(&cmd.title)
            {
                return Err(format!(
                    "forbidden vocab in preview lens title: {}",
                    cmd.title
                ));
            }
        }
        Ok(())
    }
}
