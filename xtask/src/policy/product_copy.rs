use std::fs;
use std::path::{Path, PathBuf};

const PUBLIC_FILES: &[&str] = &[
    "README.md",
    "crates/ripr/README.md",
    "docs/QUICKSTART.md",
    "docs/EDITOR_EXTENSION.md",
    "editors/vscode/README.md",
    "editors/vscode/package.json",
    "docs/RELEASE.md",
    "docs/RELEASE_MARKETPLACE.md",
    "docs/RELEASE_COPY_CHECKLIST.md",
];

const ALLOWLISTED_INTERNAL_SURFACES: &[&str] = &[
    "docs/specs/**",
    "docs/OUTPUT_SCHEMA.md",
    "fixtures/**",
    "metrics/**",
    "docs/IMPLEMENTATION_CAMPAIGNS.md",
    "CHANGELOG.md",
];

const BRIDGE_PATTERNS: &[&str] = &["TERMINOLOGY.md"];

#[derive(Clone, Copy)]
struct FlaggedTerm {
    needle: &'static str,
    suggestion: &'static str,
    word_boundary: bool,
}

const FLAGGED_TERMS: &[FlaggedTerm] = &[
    FlaggedTerm {
        needle: "test oracle",
        suggestion: "changed code where tests may not catch the behavior",
        word_boundary: false,
    },
    FlaggedTerm {
        needle: "discriminator",
        suggestion: "assertion or check that would catch the changed behavior",
        word_boundary: false,
    },
    FlaggedTerm {
        needle: "seam-native",
        suggestion: "ripr-flagged changes",
        word_boundary: false,
    },
    FlaggedTerm {
        needle: "evidence spine",
        suggestion: "shared evidence model",
        word_boundary: false,
    },
    FlaggedTerm {
        needle: "canonical gap",
        suggestion: "test-gap identity",
        word_boundary: false,
    },
    FlaggedTerm {
        needle: "no-actionable-seam",
        suggestion: "no focused test gap found",
        word_boundary: false,
    },
    FlaggedTerm {
        needle: "front panel",
        suggestion: "PR review summary",
        word_boundary: false,
    },
    FlaggedTerm {
        needle: "report packet",
        suggestion: "uploaded review artifacts",
        word_boundary: false,
    },
    FlaggedTerm {
        needle: "grip",
        suggestion: "behavior evidence",
        word_boundary: true,
    },
];

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct ProductCopyFinding {
    pub file: String,
    pub line: usize,
    pub term: String,
    pub excerpt: String,
    pub suggestion: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct ProductCopyReport {
    pub findings: Vec<ProductCopyFinding>,
    pub bridged_files: usize,
    pub total_files: usize,
    pub missing_files: Vec<String>,
}

pub(crate) fn run_product_copy_scan(root: &Path) -> Result<ProductCopyReport, String> {
    let mut findings = Vec::new();
    let mut bridged_files = 0usize;
    let mut total_files = 0usize;
    let mut missing_files = Vec::new();

    for rel in PUBLIC_FILES {
        let path = root.join(rel);
        if !path.exists() {
            missing_files.push((*rel).to_string());
            continue;
        }
        let content =
            fs::read_to_string(&path).map_err(|err| format!("read {}: {err}", path.display()))?;
        total_files += 1;
        let bridged = BRIDGE_PATTERNS.iter().any(|t| content.contains(t));
        if bridged {
            bridged_files += 1;
            continue;
        }
        scan_file_unbridged(rel, &content, &mut findings);
    }

    findings.sort_by(|a, b| {
        a.file
            .cmp(&b.file)
            .then(a.line.cmp(&b.line))
            .then(a.term.cmp(&b.term))
    });

    Ok(ProductCopyReport {
        findings,
        bridged_files,
        total_files,
        missing_files,
    })
}

fn scan_file_unbridged(rel: &str, content: &str, findings: &mut Vec<ProductCopyFinding>) {
    for (idx, line) in content.lines().enumerate() {
        let lower = line.to_ascii_lowercase();
        for term in FLAGGED_TERMS {
            let hit = if term.word_boundary {
                contains_word(&lower, term.needle)
            } else {
                lower.contains(term.needle)
            };
            if hit {
                findings.push(ProductCopyFinding {
                    file: rel.to_string(),
                    line: idx + 1,
                    term: term.needle.to_string(),
                    excerpt: line_excerpt(line),
                    suggestion: term.suggestion.to_string(),
                });
            }
        }
    }
}

fn contains_word(haystack_lower: &str, word: &str) -> bool {
    let bytes = haystack_lower.as_bytes();
    let needle_len = word.len();
    let mut start = 0usize;
    while let Some(idx) = haystack_lower[start..].find(word) {
        let abs_start = start + idx;
        let abs_end = abs_start + needle_len;
        let before_ok = abs_start == 0 || !is_word_char(bytes[abs_start - 1]);
        let after_ok = abs_end == bytes.len() || !is_word_char(bytes[abs_end]);
        if before_ok && after_ok {
            return true;
        }
        start = abs_start + 1;
    }
    false
}

fn is_word_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'
}

fn line_excerpt(line: &str) -> String {
    let trimmed = line.trim();
    let max_chars = 140usize;
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let mut taken: String = trimmed.chars().take(max_chars).collect();
    taken.push('…');
    taken
}

pub(crate) fn check_product_copy() -> Result<(), String> {
    let root = repo_root()?;
    let report = run_product_copy_scan(&root)?;
    print_report(&report);
    if report.findings.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{} unbridged internal-vocabulary finding(s) in public surfaces; \
             add a docs/TERMINOLOGY.md link or replace with plain-language copy",
            report.findings.len()
        ))
    }
}

fn print_report(report: &ProductCopyReport) {
    print!("{}", format_report(report));
}

pub(crate) fn format_report(report: &ProductCopyReport) -> String {
    let mut out = String::new();
    let status = if report.findings.is_empty() {
        "pass"
    } else {
        "fail"
    };
    out.push_str(&format!("Status: {status}\n"));
    out.push_str(&format!(
        "Public surfaces checked: {} (bridged: {})\n",
        report.total_files, report.bridged_files
    ));
    if !report.missing_files.is_empty() {
        out.push_str("Missing public surface files (skipped):\n");
        for f in &report.missing_files {
            out.push_str(&format!("  - {f}\n"));
        }
    }
    out.push_str("Allowlisted internal surfaces (not scanned):\n");
    for s in ALLOWLISTED_INTERNAL_SURFACES {
        out.push_str(&format!("  - {s}\n"));
    }
    out.push('\n');
    if report.findings.is_empty() {
        out.push_str("No unbridged internal vocabulary in public surfaces.\n");
        return out;
    }
    out.push_str(&format!("Findings ({}):\n", report.findings.len()));
    let mut current_file = String::new();
    for finding in &report.findings {
        if finding.file != current_file {
            current_file = finding.file.clone();
            out.push('\n');
            out.push_str(&format!("{current_file}:\n"));
        }
        out.push_str(&format!(
            "  line {}: `{}` -> {} ({})\n",
            finding.line, finding.term, finding.suggestion, finding.excerpt
        ));
    }
    out.push('\n');
    out.push_str("Repair: link to docs/TERMINOLOGY.md before the internal term appears,\n");
    out.push_str("or replace with the plain-language suggestion. The bridge link makes\n");
    out.push_str("the term teachable; the suggestion makes the first-hour copy readable.\n");
    out
}

fn repo_root() -> Result<PathBuf, String> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.parent().map(Path::to_path_buf).ok_or_else(|| {
        format!(
            "failed to resolve repo root from {}",
            manifest_dir.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_copy_baseline_is_clean() -> Result<(), String> {
        let root = repo_root()?;
        let report = run_product_copy_scan(&root)?;
        if !report.findings.is_empty() {
            let mut lines = Vec::with_capacity(report.findings.len());
            for f in &report.findings {
                lines.push(format!(
                    "  {}:{} `{}` -> {} ({})",
                    f.file, f.line, f.term, f.suggestion, f.excerpt
                ));
            }
            return Err(format!(
                "expected zero unbridged internal-vocabulary findings on public surfaces; got {}:\n{}",
                report.findings.len(),
                lines.join("\n")
            ));
        }
        if !report.missing_files.is_empty() {
            return Err(format!(
                "expected every public surface file to exist; missing: {:?}",
                report.missing_files
            ));
        }
        Ok(())
    }

    #[test]
    fn product_copy_flags_unbridged_terms_via_synthetic_text() -> Result<(), String> {
        let text = "ripr inspects test oracles to discover missing discriminators.";
        let mut findings = Vec::new();
        scan_file_unbridged("synth.md", text, &mut findings);
        let has_test_oracle = findings.iter().any(|f| f.term == "test oracle");
        let has_discriminator = findings.iter().any(|f| f.term == "discriminator");
        if !has_test_oracle {
            return Err("expected 'test oracle' to be flagged in synthetic line".to_string());
        }
        if !has_discriminator {
            return Err("expected 'discriminator' to be flagged in synthetic line".to_string());
        }
        Ok(())
    }

    #[test]
    fn product_copy_grip_uses_word_boundaries() -> Result<(), String> {
        // Word-bounded match: should fire.
        let mut hit = Vec::new();
        scan_file_unbridged("synth.md", "Tracks behavior grip across stages.", &mut hit);
        if !hit.iter().any(|f| f.term == "grip") {
            return Err("expected 'grip' to be flagged when surrounded by whitespace".to_string());
        }
        // Hyphen-bounded identifier: must NOT fire (hyphen is treated as a word char so
        // `coverage-grip-frontier` is one compound identifier, not a use of the term `grip`).
        let mut compound = Vec::new();
        scan_file_unbridged(
            "synth.md",
            "Output path is target/ripr/reports/coverage-grip-frontier.json",
            &mut compound,
        );
        if compound.iter().any(|f| f.term == "grip") {
            return Err(format!(
                "expected `grip` inside `coverage-grip-frontier` to be ignored as a compound identifier; got: {:?}",
                compound
            ));
        }
        // Substring inside a longer word: must NOT fire.
        let mut substring = Vec::new();
        scan_file_unbridged("synth.md", "She gripes about CI", &mut substring);
        if substring.iter().any(|f| f.term == "grip") {
            return Err("expected `grip` inside `gripes` to be ignored".to_string());
        }
        Ok(())
    }

    #[test]
    fn product_copy_bridged_files_are_skipped() -> Result<(), String> {
        // If a file links to TERMINOLOGY.md anywhere, internal terms in that file are not
        // flagged. The bridge marker is checked at file scope, not inline.
        let bridged_marker = BRIDGE_PATTERNS
            .iter()
            .find(|p| **p == "TERMINOLOGY.md")
            .copied();
        if bridged_marker.is_none() {
            return Err("expected TERMINOLOGY.md to be among the bridge patterns".to_string());
        }
        Ok(())
    }

    #[test]
    fn product_copy_line_excerpt_truncates_long_lines() -> Result<(), String> {
        let short = line_excerpt("  short line  ");
        if short != "short line" {
            return Err(format!("expected trimmed short line, got {short:?}"));
        }
        let long_input: String = "x".repeat(200);
        let excerpt = line_excerpt(&long_input);
        let char_count = excerpt.chars().count();
        if char_count != 141 {
            return Err(format!(
                "expected truncated line to be exactly 140 chars plus ellipsis (141 total), got {char_count}"
            ));
        }
        if !excerpt.ends_with('…') {
            return Err(format!(
                "expected truncated line to end with ellipsis, got {excerpt:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn product_copy_scan_flags_unbridged_tempdir_file() -> Result<(), String> {
        let root = unique_temp_root("product-copy-unbridged");
        write_synthetic_workspace(
            &root,
            "Find weak test oracles and write a discriminator-aware test.",
            None,
        )?;
        let report = run_product_copy_scan(&root).map_err(|err| format!("scan tempdir: {err}"))?;
        let terms: Vec<&str> = report.findings.iter().map(|f| f.term.as_str()).collect();
        if !terms.contains(&"test oracle") {
            return Err(format!(
                "expected 'test oracle' to be flagged in unbridged tempdir; got terms: {terms:?}"
            ));
        }
        if !terms.contains(&"discriminator") {
            return Err(format!(
                "expected 'discriminator' to be flagged in unbridged tempdir; got terms: {terms:?}"
            ));
        }
        let formatted = format_report(&report);
        if !formatted.contains("Status: fail") {
            return Err(format!(
                "expected formatted report to mark fail status; got:\n{formatted}"
            ));
        }
        if !formatted.contains("Repair: link to docs/TERMINOLOGY.md") {
            return Err(format!(
                "expected formatted report to include repair guidance; got:\n{formatted}"
            ));
        }
        clean_temp_root(&root);
        Ok(())
    }

    #[test]
    fn product_copy_scan_skips_bridged_tempdir_file() -> Result<(), String> {
        let root = unique_temp_root("product-copy-bridged");
        let bridged_marker = "See [Terminology](https://github.com/EffortlessMetrics/ripr/blob/main/docs/TERMINOLOGY.md).";
        write_synthetic_workspace(
            &root,
            &format!(
                "{bridged_marker}\n\nFind weak test oracles and write a discriminator-aware test.\n"
            ),
            None,
        )?;
        let report = run_product_copy_scan(&root).map_err(|err| format!("scan tempdir: {err}"))?;
        if !report.findings.is_empty() {
            return Err(format!(
                "expected no findings when file links to TERMINOLOGY.md; got: {:?}",
                report.findings
            ));
        }
        if report.bridged_files == 0 {
            return Err("expected at least one bridged file to be counted".to_string());
        }
        let formatted = format_report(&report);
        if !formatted.contains("Status: pass") {
            return Err(format!(
                "expected formatted report to mark pass status; got:\n{formatted}"
            ));
        }
        if !formatted.contains("No unbridged internal vocabulary in public surfaces.") {
            return Err(format!("expected clean-status message; got:\n{formatted}"));
        }
        clean_temp_root(&root);
        Ok(())
    }

    #[test]
    fn product_copy_scan_records_missing_files() -> Result<(), String> {
        let root = unique_temp_root("product-copy-missing");
        // Build only some of the expected public-surface files; the rest should be
        // reported as missing without producing scan errors.
        std::fs::create_dir_all(root.join("docs"))
            .map_err(|err| format!("create docs dir: {err}"))?;
        std::fs::write(
            root.join("README.md"),
            "Bridged README. See docs/TERMINOLOGY.md.\n",
        )
        .map_err(|err| format!("write README.md: {err}"))?;
        let report = run_product_copy_scan(&root).map_err(|err| format!("scan tempdir: {err}"))?;
        if report.total_files != 1 {
            return Err(format!(
                "expected exactly one public-surface file to exist; got {}",
                report.total_files
            ));
        }
        let expected_missing = PUBLIC_FILES.len() - 1;
        if report.missing_files.len() != expected_missing {
            return Err(format!(
                "expected {expected_missing} missing public-surface files; got {} ({:?})",
                report.missing_files.len(),
                report.missing_files
            ));
        }
        let formatted = format_report(&report);
        if !formatted.contains("Missing public surface files (skipped):") {
            return Err(format!(
                "expected formatted report to mention missing files; got:\n{formatted}"
            ));
        }
        clean_temp_root(&root);
        Ok(())
    }

    fn unique_temp_root(label: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("ripr-{label}-{}-{nanos}", std::process::id()));
        clean_temp_root(&dir);
        dir
    }

    fn clean_temp_root(root: &Path) {
        if root.exists() {
            let _ = std::fs::remove_dir_all(root);
        }
    }

    fn write_synthetic_workspace(
        root: &Path,
        readme_body: &str,
        crate_readme_body: Option<&str>,
    ) -> Result<(), String> {
        std::fs::create_dir_all(root).map_err(|err| format!("create root: {err}"))?;
        std::fs::write(root.join("README.md"), readme_body)
            .map_err(|err| format!("write README.md: {err}"))?;
        if let Some(body) = crate_readme_body {
            std::fs::create_dir_all(root.join("crates").join("ripr"))
                .map_err(|err| format!("create crates/ripr dir: {err}"))?;
            std::fs::write(root.join("crates").join("ripr").join("README.md"), body)
                .map_err(|err| format!("write crates/ripr/README.md: {err}"))?;
        }
        Ok(())
    }
}
