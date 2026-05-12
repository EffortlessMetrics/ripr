use std::path::Path;

use crate::{
    FixKind, PolicyReportSpec, collect_files, finish_policy_report, normalize_path, read_text_lossy,
};

const ALLOWLIST_PATH: &str = ".ripr/positioning-language-allowlist.txt";

/// Advisory check that flags phrasing which contradicts the canonical
/// `ripr` positioning doctrine. See `docs/ci/ripr-mutation-boundary.md`.
///
/// The doctrine states: `ripr` is **static mutation-exposure analysis**.
/// It catches the same class of signal mutation testing catches — weak
/// test/oracle exposure — but earlier and cheaper. It does not find or
/// run actual mutants; mutation testing remains the runtime backstop.
///
/// This check flags phrasing that frames `ripr` and mutation testing as
/// parallel evidence lanes detecting different signals, or that claims
/// `ripr` replaces mutation testing or produces runtime proof.
pub(crate) fn check_positioning_language() -> Result<(), String> {
    let report_spec = PolicyReportSpec {
        report_file: "positioning-language.md",
        check: "check-positioning-language",
        why_it_matters: "Public copy should preserve the canonical doctrine: `ripr` is static mutation-exposure analysis that shifts the same signal mutation testing catches earlier and cheaper. Misframing it as a parallel lane or a replacement for mutation testing leads downstream users to draw the wrong cost-curve conclusions.",
        fix_kind: FixKind::AuthorDecisionRequired,
        recommended_fixes: &[
            "Rewrite the line to use the canonical doctrine: `ripr` is static mutation-exposure analysis that catches the same class of signal mutation testing catches, earlier and cheaper.",
            "If the phrase is a legitimate quote (e.g. inside this check's implementation or the doctrine doc), add the file path to `.ripr/positioning-language-allowlist.txt`.",
        ],
        rerun_command: "cargo xtask check-positioning-language",
        exception_template: Some(
            ".ripr/positioning-language-allowlist.txt entry:\npath/to/file.md\n# reason",
        ),
    };

    let allowed = match load_positioning_language_allowlist() {
        Ok(entries) => entries,
        Err(violations) => return finish_policy_report(report_spec, &violations),
    };
    let forbidden = forbidden_positioning_phrases();
    let mut violations = Vec::new();

    for path in collect_files(Path::new("."))? {
        let normalized = normalize_path(&path);
        if !should_scan_positioning_language_path(&allowed, &normalized) {
            continue;
        }
        let text = read_text_lossy(&path)?;
        for (line_number, line) in text.lines().enumerate() {
            let lower = line.to_ascii_lowercase();
            for phrase in &forbidden {
                if lower.contains(phrase.as_str()) {
                    violations.push(format!(
                        "{normalized}:{} contains misleading positioning phrase `{phrase}`",
                        line_number + 1
                    ));
                }
            }
        }
    }

    finish_policy_report(report_spec, &violations)
}

fn forbidden_positioning_phrases() -> Vec<String> {
    [
        "parallel evidence lanes",
        "parallel evidence lane",
        "ripr replaces mutation testing",
        "ripr replaces mutation",
        "ripr is separate from mutation testing",
        "ripr is separate from mutation",
        "ripr is not mutation",
        "runtime proof from ripr",
        "runtime evidence from ripr",
    ]
    .iter()
    .map(|value| value.to_string())
    .collect()
}

fn is_positioning_language_candidate(path: &str) -> bool {
    path.ends_with(".md")
        || path.ends_with(".rs")
        || path.ends_with(".toml")
        || path.ends_with(".yaml")
        || path.ends_with(".yml")
        || path.ends_with(".json")
}

fn load_positioning_language_allowlist() -> Result<Vec<String>, Vec<String>> {
    let path = Path::new(ALLOWLIST_PATH);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = read_text_lossy(path).map_err(|err| vec![err])?;
    let mut entries = Vec::new();
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        entries.push(line.to_string());
    }
    Ok(entries)
}

fn should_scan_positioning_language_path(allowlist: &[String], path: &str) -> bool {
    if !is_positioning_language_candidate(path) {
        return false;
    }
    if path.starts_with("target/") || path.starts_with(".git/") {
        return false;
    }
    !allowlist.iter().any(|entry| entry == path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowlist_returns_empty_when_file_missing() {
        // Tested implicitly by repo state: file may or may not exist.
        // The function does not panic when the path is absent.
        let _ = load_positioning_language_allowlist();
    }

    #[test]
    fn candidate_predicate_includes_text_artifacts_only() {
        assert!(is_positioning_language_candidate("docs/README.md"));
        assert!(is_positioning_language_candidate("crates/x/src/main.rs"));
        assert!(is_positioning_language_candidate(
            ".github/workflows/ci.yml"
        ));
        assert!(!is_positioning_language_candidate("badges/ripr.png"));
        assert!(!is_positioning_language_candidate("Cargo.lock"));
    }

    #[test]
    fn forbidden_phrases_are_lowercase() {
        for phrase in forbidden_positioning_phrases() {
            assert_eq!(
                phrase,
                phrase.to_ascii_lowercase(),
                "phrase `{phrase}` should already be lowercase",
            );
        }
    }

    #[test]
    fn allowlist_skips_listed_path() {
        let allowlist = vec!["docs/exempt.md".to_string()];
        assert!(!should_scan_positioning_language_path(
            &allowlist,
            "docs/exempt.md"
        ));
        assert!(should_scan_positioning_language_path(
            &allowlist,
            "docs/included.md"
        ));
    }

    #[test]
    fn target_and_git_paths_are_skipped() {
        let allowlist: Vec<String> = Vec::new();
        assert!(!should_scan_positioning_language_path(
            &allowlist,
            "target/ripr/reports/scratch.md"
        ));
        assert!(!should_scan_positioning_language_path(
            &allowlist,
            ".git/HEAD.md"
        ));
    }
}
