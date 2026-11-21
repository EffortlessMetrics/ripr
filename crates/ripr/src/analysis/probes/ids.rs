use crate::domain::{ProbeFamily, ProbeId, SymbolId};
use sha2::{Digest, Sha256};
use std::path::Path;

/// Normalize an expression: trim leading/trailing whitespace and collapse
/// internal whitespace runs to a single space.
pub(crate) fn normalize_expression(expr: &str) -> String {
    let trimmed = expr.trim();
    let mut result = String::with_capacity(trimmed.len());
    let mut last_was_space = false;
    for ch in trimmed.chars() {
        if ch.is_whitespace() {
            if !last_was_space {
                result.push(' ');
            }
            last_was_space = true;
        } else {
            result.push(ch);
            last_was_space = false;
        }
    }
    result
}

/// Compute the 8-char hex fingerprint for a content-addressed probe id.
/// Payload is NUL-separated: `<sanitized_path>\0<family_str>\0<owner_str>\0<normalized_expression>\0`
fn compute_fp8(
    sanitized_path: &str,
    family_str: &str,
    owner_str: &str,
    normalized_expression: &str,
) -> String {
    // Normalize path separators in the owner symbol so the fingerprint is
    // platform-independent. The owner `SymbolId` embeds a file path walked
    // from the filesystem, which uses `\` on Windows and `/` elsewhere;
    // without this, the same code hashes to different ids per OS and goldens
    // blessed on one platform fail CI on another (#1053).
    let owner_normalized = owner_str.replace('\\', "/");
    let mut hasher = Sha256::new();
    hasher.update(sanitized_path.as_bytes());
    hasher.update(b"\0");
    hasher.update(family_str.as_bytes());
    hasher.update(b"\0");
    hasher.update(owner_normalized.as_bytes());
    hasher.update(b"\0");
    hasher.update(normalized_expression.as_bytes());
    hasher.update(b"\0");
    let hash = hasher.finalize();
    format!(
        "{:02x}{:02x}{:02x}{:02x}",
        hash[0], hash[1], hash[2], hash[3]
    )
}

/// Build a content-addressed probe id.
/// Format: `<prefix>:<sanitized_path>:<family_str>:<fp8>[.<ordinal>]`
/// The ordinal suffix (`.2`, `.3`, …) is OMITTED when ordinal == 1.
pub(crate) fn fingerprint_probe_id(
    prefix: &str,
    sanitized_path: &str,
    family_str: &str,
    owner_str: &str,
    normalized_expression: &str,
    ordinal: u32,
) -> ProbeId {
    let fp8 = compute_fp8(sanitized_path, family_str, owner_str, normalized_expression);
    if ordinal <= 1 {
        ProbeId(format!("{prefix}:{sanitized_path}:{family_str}:{fp8}"))
    } else {
        ProbeId(format!(
            "{prefix}:{sanitized_path}:{family_str}:{fp8}.{ordinal}"
        ))
    }
}

pub(crate) fn diff_probe_id(
    path: &Path,
    family: &ProbeFamily,
    owner: Option<&SymbolId>,
    expression: &str,
    ordinal: u32,
) -> ProbeId {
    let sp = sanitize_path(path);
    let family_str = family.as_str();
    let owner_str = owner.map(|o| o.0.as_str()).unwrap_or("");
    let norm = normalize_expression(expression);
    fingerprint_probe_id("probe", &sp, family_str, owner_str, &norm, ordinal)
}

pub(crate) fn repo_probe_id(
    path: &Path,
    family: &ProbeFamily,
    owner: Option<&SymbolId>,
    expression: &str,
    ordinal: u32,
) -> ProbeId {
    let sp = sanitize_path(path);
    let family_str = family.as_str();
    let owner_str = owner.map(|o| o.0.as_str()).unwrap_or("");
    let norm = normalize_expression(expression);
    fingerprint_probe_id("repo-probe", &sp, family_str, owner_str, &norm, ordinal)
}

/// Post-hoc collision dedup for adapter findings: scan `findings` in order;
/// for any probe id that appears more than once, rewrite the 2nd+
/// occurrences to append `.2`, `.3`, … (the first keeps its id as-is, i.e.
/// ordinal 1). Same-fingerprint collisions arise when one owner has several
/// same-text lines (e.g. repeated `}` closers): the content-addressed id is
/// deliberately line-independent so it survives line movement, so the
/// ordinal carries within-run uniqueness instead.
///
/// Mirrors `dedup_probe_ids` for bare probes, extended to findings:
/// `Finding.id` follows the probe, and `source_id=<id>` evidence lines are
/// rebound so consumers that join evidence to probes keep resolving.
/// Without this, duplicate stable ids fail the PR evidence contract.
pub(crate) fn dedup_finding_probe_ids(findings: &mut [crate::domain::Finding]) {
    use std::collections::HashMap;
    let mut seen: HashMap<String, u32> = HashMap::new();
    for finding in findings.iter_mut() {
        let count = seen.entry(finding.probe.id.0.clone()).or_insert(0);
        *count += 1;
        if *count > 1 {
            let old = finding.probe.id.0.clone();
            let new = format!("{old}.{count}");
            finding.probe.id.0 = new.clone();
            if finding.id == old {
                finding.id = new.clone();
            }
            let needle = format!("source_id={old}");
            let replacement = format!("source_id={new}");
            for line in finding.evidence.iter_mut() {
                if line.contains(&needle) {
                    *line = line.replace(&needle, &replacement);
                }
            }
        }
    }
}

pub(crate) fn sanitize_path(path: &Path) -> String {
    crate::analysis::stable_path_text(path)
        .replace(['/', '\\', ':'], "_")
        .trim_matches('_')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ProbeFamily;
    use std::path::PathBuf;

    #[test]
    fn sanitize_path_converts_separators_and_colons() {
        let path = PathBuf::from("src/lib.rs");
        let sanitized = sanitize_path(&path);
        assert_eq!(sanitized, "src_lib.rs");
    }

    #[test]
    fn sanitize_path_handles_windows_paths() {
        let path = PathBuf::from("workspace\\src\\lib.rs");
        let sanitized = sanitize_path(&path);
        assert_eq!(sanitized, "workspace_src_lib.rs");
    }

    #[test]
    fn sanitize_path_trims_underscores() {
        let path = PathBuf::from(":src/lib:");
        let sanitized = sanitize_path(&path);
        assert_eq!(sanitized, "src_lib");
    }

    #[cfg(unix)]
    #[test]
    fn sanitize_path_keeps_invalid_bytes_distinct_from_utf8_text() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let invalid = PathBuf::from(OsString::from_vec(b"pricing_\xff.rs".to_vec()));
        let literal = PathBuf::from("pricing_%FF.rs");
        assert_ne!(sanitize_path(&invalid), sanitize_path(&literal));
    }

    #[test]
    fn content_addressed_id_stable_across_line_movement() {
        // Same (path, family, owner, expression) with two different lines → equal ids.
        let path = PathBuf::from("src/lib.rs");
        let family = ProbeFamily::Predicate;
        let owner = Some(SymbolId("my_module::my_fn".to_string()));
        let expression = "if x > 0 {";

        // ordinal 1 in both cases (different lines would previously produce different ids)
        let id_line3 = diff_probe_id(&path, &family, owner.as_ref(), expression, 1);
        let id_line99 = diff_probe_id(&path, &family, owner.as_ref(), expression, 1);

        assert_eq!(
            id_line3, id_line99,
            "ids must be identical regardless of line number"
        );
    }

    #[test]
    fn changed_expression_changes_id() {
        // Same (path, family, owner) but different expression → different ids.
        let path = PathBuf::from("src/lib.rs");
        let family = ProbeFamily::Predicate;
        let owner = Some(SymbolId("my_module::my_fn".to_string()));

        let id_gte = diff_probe_id(&path, &family, owner.as_ref(), "if a >= b {", 1);
        let id_gt = diff_probe_id(&path, &family, owner.as_ref(), "if a > b {", 1);

        assert_ne!(
            id_gte, id_gt,
            "changed expression must yield a different id"
        );
    }

    #[test]
    fn collision_suffix_appended_for_ordinal_2() {
        let path = PathBuf::from("src/lib.rs");
        let family = ProbeFamily::Predicate;
        let owner = Some(SymbolId("my_module::my_fn".to_string()));
        let expression = "if x > 0 {";

        let id1 = diff_probe_id(&path, &family, owner.as_ref(), expression, 1);
        let id2 = diff_probe_id(&path, &family, owner.as_ref(), expression, 2);

        // The id should NOT end with a `.N` collision suffix for ordinal 1.
        // (The id may contain '.' as part of the sanitized path like "lib.rs",
        //  so we check the fp8 hex segment does not have a dot after it.)
        assert!(
            !id1.0.ends_with(".1"),
            "ordinal 1 must not end with .1, got: {}",
            id1.0
        );
        // Verify the base ids are the same (same fingerprint), just different ordinal suffix.
        let base1 = id1.0.as_str();
        assert!(
            id2.0.ends_with(".2"),
            "ordinal 2 must end with .2, got: {}",
            id2.0
        );
        // The base (without .2) of ordinal-2 id should equal the ordinal-1 id.
        let base2 = id2.0.strip_suffix(".2").unwrap_or("");
        assert_eq!(
            base1, base2,
            "base ids must match; ordinal-1={base1}, ordinal-2={base2}"
        );
    }

    #[test]
    fn normalize_expression_collapses_whitespace() {
        assert_eq!(normalize_expression("  if   x > 0  {  "), "if x > 0 {");
        assert_eq!(normalize_expression("hello"), "hello");
        assert_eq!(normalize_expression(""), "");
    }

    #[test]
    fn dedup_finding_probe_ids_suffixes_repeats_and_rebinds_evidence() {
        use crate::domain::{
            ActivationEvidence, Confidence, DeltaKind, ExposureClass, Finding, Probe, ProbeFamily,
            ProbeId, RevealEvidence, RiprEvidence, SourceLocation, StageEvidence, StageState,
        };

        fn stage() -> StageEvidence {
            StageEvidence::new(StageState::Unknown, Confidence::Unknown, "dedup test")
        }

        fn finding(id: &str, line: usize) -> Finding {
            let probe_id = format!("probe:file_ts:typescript_preview:{id}");
            Finding {
                id: probe_id.clone(),
                canonical_gap: None,
                probe: Probe {
                    id: ProbeId(probe_id.clone()),
                    location: SourceLocation::new("file.ts", line, 1),
                    owner: None,
                    family: ProbeFamily::Predicate,
                    delta: DeltaKind::Control,
                    before: None,
                    after: Some("}".to_string()),
                    expression: "}".to_string(),
                    expected_sinks: Vec::new(),
                    required_oracles: Vec::new(),
                },
                class: ExposureClass::ReachableUnrevealed,
                ripr: RiprEvidence {
                    reach: stage(),
                    infect: stage(),
                    propagate: stage(),
                    reveal: RevealEvidence {
                        observe: stage(),
                        discriminate: stage(),
                    },
                },
                confidence: 0.0,
                evidence: vec![format!(
                    "raw_evidence_ref: leg=x;source_id={probe_id};owner=o"
                )],
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
                source_currentness: crate::domain::SourceCurrentness::CandidateCurrent,
            }
        }

        // Two same-fingerprint findings (repeated `}` closers) plus one
        // distinct probe: only the repeat is suffixed, in order.
        let mut findings = vec![
            finding("bcd59d90", 48),
            finding("bcd59d90", 51),
            finding("aaaa1111", 7),
        ];
        dedup_finding_probe_ids(&mut findings);
        assert_eq!(
            findings[0].probe.id.0,
            "probe:file_ts:typescript_preview:bcd59d90"
        );
        assert_eq!(findings[0].id, "probe:file_ts:typescript_preview:bcd59d90");
        assert_eq!(
            findings[1].probe.id.0,
            "probe:file_ts:typescript_preview:bcd59d90.2"
        );
        assert_eq!(
            findings[1].id,
            "probe:file_ts:typescript_preview:bcd59d90.2"
        );
        assert!(
            findings[1].evidence[0]
                .contains("source_id=probe:file_ts:typescript_preview:bcd59d90.2"),
            "evidence must rebind to the suffixed id: {}",
            findings[1].evidence[0]
        );
        assert!(
            !findings[0].evidence[0].contains(".2"),
            "first occurrence keeps the bare id: {}",
            findings[0].evidence[0]
        );
        assert_eq!(
            findings[2].probe.id.0,
            "probe:file_ts:typescript_preview:aaaa1111"
        );
        // Stable ids are unique across the run.
        let mut ids: Vec<&str> = findings
            .iter()
            .map(|finding| finding.probe.id.0.as_str())
            .collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), findings.len(), "stable ids must be unique");
    }
}
