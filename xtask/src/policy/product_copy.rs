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
    let status = if report.findings.is_empty() {
        "pass"
    } else {
        "fail"
    };
    println!("Status: {status}");
    println!(
        "Public surfaces checked: {} (bridged: {})",
        report.total_files, report.bridged_files
    );
    if !report.missing_files.is_empty() {
        println!("Missing public surface files (skipped):");
        for f in &report.missing_files {
            println!("  - {f}");
        }
    }
    println!("Allowlisted internal surfaces (not scanned):");
    for s in ALLOWLISTED_INTERNAL_SURFACES {
        println!("  - {s}");
    }
    println!();
    if report.findings.is_empty() {
        println!("No unbridged internal vocabulary in public surfaces.");
        return;
    }
    println!("Findings ({}):", report.findings.len());
    let mut current_file = String::new();
    for finding in &report.findings {
        if finding.file != current_file {
            current_file = finding.file.clone();
            println!();
            println!("{current_file}:");
        }
        println!(
            "  line {}: `{}` -> {} ({})",
            finding.line, finding.term, finding.suggestion, finding.excerpt
        );
    }
    println!();
    println!("Repair: link to docs/TERMINOLOGY.md before the internal term appears,");
    println!("or replace with the plain-language suggestion. The bridge link makes");
    println!("the term teachable; the suggestion makes the first-hour copy readable.");
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
}
