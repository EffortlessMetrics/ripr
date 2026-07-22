//! No-panic-family policy: forbidden panic-pattern scanning, semantic panic
//! finding extraction, allowlist parsing and validation (v0.1/v0.2/v0.3), and
//! allowlist proposal rendering.
//!
//! Extracted verbatim from `main.rs` as the second behavior-preserving
//! decomposition slice of #2119. Shared text/TOML helpers
//! (`contains_word`, `parse_string_value`, and friends) stay re-exported to
//! `main.rs` so existing call sites compile unchanged.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::policy::check_no_panic_family;
use crate::{
    FixKind, PolicyReportSpec, collect_files, finish_policy_report, normalize_path,
    read_text_lossy, write_report,
};

pub(crate) fn check_no_panic_family_impl() -> Result<(), String> {
    check_old_panic_allowlist_exists()?;

    let roots = [
        Path::new("crates/ripr/src"),
        Path::new("crates/ripr/tests"),
        Path::new("xtask/src"),
    ];
    let patterns = forbidden_panic_patterns();

    let mut findings = Vec::new();
    for root in roots {
        if !root.exists() {
            continue;
        }
        findings.extend(collect_panic_findings(root, &patterns)?);
    }

    // Prefer the governed schema 0.3 policy file. The legacy `.ripr/` file
    // remains as a compatibility mirror while older branches drain.
    let policy_allowlist_path = "policy/no-panic-allowlist.toml";
    let legacy_allowlist_path = ".ripr/no-panic-allowlist.toml";
    let allowlist_path = if Path::new(policy_allowlist_path).exists() {
        policy_allowlist_path
    } else {
        legacy_allowlist_path
    };
    let has_semantic_allowlist = if Path::new(allowlist_path).exists() {
        let text = read_text_lossy(Path::new(allowlist_path))?;
        text.contains("schema_version = \"0.2\"") || text.contains("schema_version = \"0.3\"")
    } else {
        false
    };

    let mut violations = Vec::new();
    if has_semantic_allowlist {
        // Schema 0.2/0.3 mode: use semantic findings and versioned parser.
        let mut semantic_findings = Vec::new();
        for root in roots {
            if !root.exists() {
                continue;
            }
            semantic_findings.extend(collect_semantic_panic_findings(root, &patterns)?);
        }

        let versioned_entries = if Path::new(allowlist_path).exists() {
            parse_no_panic_allowlist_toml_v2(allowlist_path)?
        } else {
            Vec::new()
        };

        let report = evaluate_semantic_no_panic_policy(&semantic_findings, &versioned_entries);
        print_no_panic_family_report(&report);
        violations.extend(report.violations);
    } else {
        // v0.1 mode: existing behavior
        let allowlist = if Path::new(allowlist_path).exists() {
            parse_no_panic_allowlist_toml(allowlist_path)?
        } else {
            Vec::new()
        };

        for finding in &findings {
            let matched = allowlist.iter().any(|e| {
                e.path == finding.path
                    && e.line == finding.line
                    && e.family == finding.family
                    && (e.column.is_none() || e.column == finding.column)
            });
            if !matched {
                violations.push(format!(
                    "{}:{}:{} contains unallowed panic-family '{}'; add exact allowlist entry with explanation",
                    finding.path,
                    finding.line,
                    finding.column.unwrap_or(0),
                    finding.family
                ));
            }
        }

        for entry in &allowlist {
            let matched = findings.iter().any(|f| {
                f.path == entry.path
                    && f.line == entry.line
                    && f.family == entry.family
                    && (entry.column.is_none() || entry.column == f.column)
            });
            if !matched {
                violations.push(format!(
                    "stale allowlist entry: {}:{}:{:?} ({}) does not match any current finding",
                    entry.path, entry.line, entry.column, entry.family
                ));
            }
        }
    }

    finish_policy_report(
        PolicyReportSpec {
            report_file: "no-panic-family.md",
            check: "check-no-panic-family",
            why_it_matters: "Product and test code should surface failures explicitly instead of relying on panic-family shortcuts.",
            fix_kind: FixKind::PolicyExceptionRequired,
            recommended_fixes: &[
                "Return `Result` and propagate setup or IO failures.",
                "Pattern-match `Option` values and return explicit errors in tests.",
                "Use an allowlist entry only for reviewed legacy debt or intentional string detection.",
            ],
            rerun_command: "cargo xtask check-no-panic-family",
            exception_template: Some(
                "policy/no-panic-allowlist.toml entry:\n[[allow]]\nid = \"panic-0000\"\npath = \"path/to/file.rs\"\nfamily = \"unwrap\"\nclassification = \"test_only\"\nowner = \"team/area\"\nexplanation = \"Human-readable reason\"\nexpires = \"2026-12-31\"\n\n[allow.selector]\nkind = \"method_call\"\ncontainer = \"test_name\"\ncallee = \"unwrap\"",
            ),
        },
        &violations,
    )
}

pub(crate) fn check_no_panic_family_with_args(args: &[String]) -> Result<(), String> {
    match args {
        [] => check_no_panic_family(),
        [flag] if flag == "--propose" => propose_no_panic_allowlist_impl(),
        _ => Err(format!(
            "unsupported check-no-panic-family argument(s): {}\nusage: cargo xtask check-no-panic-family [--propose]",
            args.join(" ")
        )),
    }
}

fn propose_no_panic_allowlist_impl() -> Result<(), String> {
    check_old_panic_allowlist_exists()?;

    let roots = [
        Path::new("crates/ripr/src"),
        Path::new("crates/ripr/tests"),
        Path::new("xtask/src"),
    ];
    let patterns = forbidden_panic_patterns();
    let mut semantic_findings = Vec::new();
    for root in roots {
        if !root.exists() {
            continue;
        }
        semantic_findings.extend(collect_semantic_panic_findings(root, &patterns)?);
    }

    let allowlist_path = if Path::new("policy/no-panic-allowlist.toml").exists() {
        "policy/no-panic-allowlist.toml"
    } else {
        ".ripr/no-panic-allowlist.toml"
    };
    let versioned_entries = if Path::new(allowlist_path).exists() {
        parse_no_panic_allowlist_toml_v2(allowlist_path)?
    } else {
        Vec::new()
    };

    let proposals = build_no_panic_allowlist_proposals(&semantic_findings, &versioned_entries);
    write_report(
        "no-panic-allowlist-proposals.md",
        &render_no_panic_allowlist_proposals_markdown(&proposals),
    )?;
    write_report(
        "no-panic-allowlist-proposals.toml",
        &render_no_panic_allowlist_proposals_toml(&proposals),
    )?;
    println!(
        "wrote {} no-panic allowlist proposal(s) to target/ripr/reports/no-panic-allowlist-proposals.md and target/ripr/reports/no-panic-allowlist-proposals.toml",
        proposals.len()
    );
    Ok(())
}

fn forbidden_panic_patterns() -> Vec<String> {
    [
        concat!("unwrap", "("),
        concat!("expect", "("),
        concat!("panic", "!"),
        concat!("todo", "!"),
        concat!("unimplemented", "!"),
        concat!("unreachable", "!"),
    ]
    .iter()
    .map(|value| value.to_string())
    .collect()
}

pub(crate) fn contains_word(text: &str, word: &str) -> bool {
    let mut start = 0usize;
    while let Some(offset) = text[start..].find(word) {
        let idx = start + offset;
        let before = text[..idx].chars().next_back();
        let after = text[idx + word.len()..].chars().next();
        if !is_word_char(before) && !is_word_char(after) {
            return true;
        }
        start = idx + word.len();
    }
    false
}

fn is_word_char(value: Option<char>) -> bool {
    value
        .map(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        .unwrap_or(false)
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct PanicFinding {
    path: String,
    line: usize,
    column: Option<usize>,
    family: String,
}

#[derive(Debug, Clone)]
struct PanicAllowEntry {
    path: String,
    line: usize,
    column: Option<usize>,
    family: String,
    classification: Option<String>,
    explanation: String,
}

#[derive(Debug, Clone)]
struct PanicFamilySelector {
    kind: String,
    container: Option<String>,
    callee: Option<String>,
    receiver_fingerprint: Option<String>,
    text_contains: Option<String>,
    snippet: Option<String>,
}

#[derive(Debug, Clone)]
struct PanicFamilyLastSeen {
    line: usize,
    column: Option<usize>,
}

#[derive(Debug, Clone)]
struct PanicAllowEntryV2 {
    id: Option<String>,
    path: String,
    family: String,
    classification: Option<String>,
    owner: Option<String>,
    explanation: String,
    expires: Option<String>,
    selector: Option<PanicFamilySelector>,
    last_seen: Option<PanicFamilyLastSeen>,
    count: Option<usize>,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct SemanticPanicFinding {
    path: String,
    family: String,
    kind: String,
    line: usize,
    column: Option<usize>,
    container: Option<String>,
    callee: Option<String>,
    receiver_fingerprint: Option<String>,
    snippet_fingerprint: String,
}

fn panic_family_from_pattern(pattern: &str) -> &'static str {
    match pattern {
        s if s.contains("unwrap") => "unwrap",
        s if s.contains("expect") => "expect",
        s if s.contains("panic!") => "panic_macro",
        s if s.contains("todo!") => "todo",
        s if s.contains("unimplemented!") => "unimplemented",
        s if s.contains("unreachable!") => "unreachable",
        _ => "unknown",
    }
}

fn collect_panic_findings(root: &Path, patterns: &[String]) -> Result<Vec<PanicFinding>, String> {
    let mut findings = Vec::new();

    for path in collect_files(root)? {
        if path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }
        let normalized = normalize_path(&path);
        let text = read_text_lossy(&path)?;

        for (line_num, line) in text.lines().enumerate() {
            let line_number = line_num + 1;
            for pattern in patterns {
                let mut start = 0usize;
                while let Some(offset) = line[start..].find(pattern) {
                    let col = start + offset + 1;
                    findings.push(PanicFinding {
                        path: normalized.clone(),
                        line: line_number,
                        column: Some(col),
                        family: panic_family_from_pattern(pattern).to_string(),
                    });
                    start = col;
                }
            }
        }
    }

    findings.sort();
    Ok(findings)
}

fn collect_semantic_panic_findings(
    root: &Path,
    patterns: &[String],
) -> Result<Vec<SemanticPanicFinding>, String> {
    use ra_ap_syntax::{AstNode, Edition, SourceFile};

    let mut findings = Vec::new();

    for path in collect_files(root)? {
        if path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }
        let normalized = normalize_path(&path);
        let text = read_text_lossy(&path)?;

        let parse = SourceFile::parse(&text, Edition::Edition2024);
        let tree = parse.tree();
        let root_node = tree.syntax();
        extract_panic_calls_from_node(root_node, &text, &normalized, patterns, &mut findings);
    }

    findings.sort();
    Ok(findings)
}

fn extract_panic_calls_from_node(
    node: &ra_ap_syntax::SyntaxNode,
    text: &str,
    path: &str,
    patterns: &[String],
    findings: &mut Vec<SemanticPanicFinding>,
) {
    use ra_ap_syntax::ast::{self, AstNode};

    for child in node.children() {
        let matched = if let Some(call_expr) = ast::MethodCallExpr::cast(child.clone()) {
            call_expr.name_ref().and_then(|method_name| {
                let name = method_name.text().to_string();
                if pattern_matches_panic_call(patterns, &name) {
                    extract_call_metadata(call_expr.syntax(), text, path, &name, "method_call")
                } else {
                    None
                }
            })
        } else if let Some(call_expr) = ast::CallExpr::cast(child.clone()) {
            call_expr.expr().and_then(|expr| {
                let func_text = expr.syntax().text().to_string();
                let base_callee = base_name_from_callee_text(&func_text);
                if pattern_matches_panic_call(patterns, base_callee) {
                    extract_call_metadata(call_expr.syntax(), text, path, base_callee, "call")
                } else {
                    None
                }
            })
        } else if let Some(macro_call) = ast::MacroCall::cast(child.clone()) {
            macro_call
                .path()
                .and_then(|p| p.segment())
                .and_then(|path_seg| {
                    let name = path_seg
                        .name_ref()
                        .map(|n| n.text().to_string())
                        .unwrap_or_default();
                    let macro_name = format!("{}!", name);
                    if pattern_matches_panic_call(patterns, &macro_name) {
                        extract_call_metadata(
                            macro_call.syntax(),
                            text,
                            path,
                            &macro_name,
                            "macro_call",
                        )
                    } else {
                        None
                    }
                })
        } else {
            None
        };

        if let Some(finding) = matched {
            findings.push(finding);
        }

        extract_panic_calls_from_node(&child, text, path, patterns, findings);
    }
}

fn base_name_from_callee_text(callee_text: &str) -> &str {
    callee_text.rsplit("::").next().unwrap_or(callee_text)
}

fn pattern_matches_panic_call(patterns: &[String], text: &str) -> bool {
    for pattern in patterns {
        if pattern == text {
            return true;
        }
        let base = pattern.trim_end_matches('(').trim_end_matches('!');
        if base == text && !base.is_empty() {
            return true;
        }
    }
    false
}

fn extract_call_metadata(
    node: &ra_ap_syntax::SyntaxNode,
    text: &str,
    path: &str,
    family_name: &str,
    kind: &str,
) -> Option<SemanticPanicFinding> {
    let (line, column) = line_and_column_for_node(node, text);
    let family = panic_family_from_call_name(family_name).to_string();
    let snippet = node.text().to_string();
    let snippet_fingerprint = snippet.replace('\n', " ").trim().to_string();

    let receiver_fingerprint = if kind == "method_call" {
        extract_method_receiver_fingerprint(node)
    } else {
        None
    };

    Some(SemanticPanicFinding {
        path: path.to_string(),
        family,
        kind: kind.to_string(),
        line,
        column,
        container: extract_container_name(node),
        callee: Some(family_name.to_string()),
        receiver_fingerprint,
        snippet_fingerprint,
    })
}

fn extract_method_receiver_fingerprint(node: &ra_ap_syntax::SyntaxNode) -> Option<String> {
    use ra_ap_syntax::ast::{self, AstNode};

    let method_call = ast::MethodCallExpr::cast(node.clone())?;
    let receiver = method_call.receiver()?;
    let text = receiver.syntax().text().to_string();
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    Some(normalized)
}

fn line_and_column_for_node(node: &ra_ap_syntax::SyntaxNode, text: &str) -> (usize, Option<usize>) {
    let offset: usize = node.text_range().start().into();
    let mut line = 1;
    let mut col = 0;

    for (byte_idx, ch) in text.char_indices() {
        if byte_idx > offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }

    (line, Some(col))
}

fn panic_family_from_call_name(name: &str) -> &'static str {
    match name {
        "unwrap" => "unwrap",
        "expect" => "expect",
        "panic!" => "panic_macro",
        "todo!" => "todo",
        "unimplemented!" => "unimplemented",
        "unreachable!" => "unreachable",
        s if s.starts_with("unwrap") && s.ends_with('(') => "unwrap",
        s if s.starts_with("expect") && s.ends_with('(') => "expect",
        s if s.starts_with("panic") && s.ends_with('!') => "panic_macro",
        s if s.starts_with("todo") && s.ends_with('!') => "todo",
        s if s.starts_with("unimplemented") && s.ends_with('!') => "unimplemented",
        s if s.starts_with("unreachable") && s.ends_with('!') => "unreachable",
        _ => "unknown",
    }
}

fn extract_container_name(node: &ra_ap_syntax::SyntaxNode) -> Option<String> {
    use ra_ap_syntax::ast::{self, AstNode, HasName};

    let mut current = node.parent();
    while let Some(parent) = current {
        let result = (|| {
            if let Some(func) = ast::Fn::cast(parent.clone()) {
                return func.name().map(|n| n.text().to_string());
            }
            if let Some(impl_block) = ast::Impl::cast(parent.clone()) {
                return impl_block.self_ty().and_then(|t| {
                    if let ast::Type::PathType(pt) = t {
                        pt.path().and_then(|p| {
                            p.segment()
                                .and_then(|s| s.name_ref().map(|n| n.text().to_string()))
                        })
                    } else {
                        None
                    }
                });
            }
            None
        })();
        if result.is_some() {
            return result;
        }
        current = parent.parent();
    }
    None
}

fn semantic_selector_matches(
    selector: &PanicFamilySelector,
    finding: &SemanticPanicFinding,
) -> bool {
    let valid_kind = matches!(
        selector.kind.as_str(),
        "method_call" | "call" | "macro_call" | "string_literal"
    );
    if !valid_kind {
        return false;
    }

    if selector.kind == "string_literal" {
        if finding.kind != "string_literal" {
            return false;
        }
        return selector
            .text_contains
            .as_ref()
            .is_some_and(|tc| finding.snippet_fingerprint.contains(tc));
    }

    selector.kind == finding.kind
        && (selector.container.is_none()
            || finding.container.as_ref() == selector.container.as_ref())
        && (selector.callee.is_none() || finding.callee.as_ref() == selector.callee.as_ref())
        && (selector.receiver_fingerprint.is_none()
            || finding.receiver_fingerprint.as_ref() == selector.receiver_fingerprint.as_ref())
        && (selector.snippet.is_none()
            || selector
                .snippet
                .as_deref()
                .is_some_and(|s| finding.snippet_fingerprint.contains(s)))
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
struct NoPanicFamilyReport {
    allowed_findings: Vec<String>,
    advisory_drift: Vec<String>,
    stale_entries: Vec<String>,
    unallowed_findings: Vec<String>,
    warnings: Vec<String>,
    violations: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct NoPanicAllowlistProposal {
    id: String,
    current_finding: String,
    path: String,
    family: String,
    kind: String,
    container: Option<String>,
    callee: Option<String>,
    receiver_fingerprint: Option<String>,
    text_contains: Option<String>,
    confidence: String,
    replaces_v1_entry: bool,
    existing_entry: String,
    old_coordinates: Option<String>,
    last_seen_line: usize,
    last_seen_column: Option<usize>,
    classification: Option<String>,
    owner: Option<String>,
    explanation: String,
    expires: Option<String>,
    warnings: Vec<String>,
}

fn evaluate_semantic_no_panic_policy(
    findings: &[SemanticPanicFinding],
    entries: &[PanicAllowEntryVersioned],
) -> NoPanicFamilyReport {
    let mut report = NoPanicFamilyReport::default();
    let mut match_counts_by_entry = vec![0usize; entries.len()];

    for finding in findings {
        let mut matched_entries = Vec::new();
        for (index, entry) in entries.iter().enumerate() {
            if panic_allow_entry_matches(entry, finding) {
                matched_entries.push(index);
                match_counts_by_entry[index] += 1;
            }
        }

        if matched_entries.is_empty() {
            let message = format!(
                "{} contains unallowed panic-family '{}'; add exact allowlist entry with explanation",
                semantic_panic_finding_label(finding),
                finding.family
            );
            report.unallowed_findings.push(message.clone());
            report.violations.push(message);
            continue;
        }

        let allowed_by = matched_entries
            .iter()
            .map(|index| panic_allow_entry_label(&entries[*index]))
            .collect::<Vec<_>>()
            .join(", ");
        report.allowed_findings.push(format!(
            "{} allowed by {}",
            semantic_panic_finding_label(finding),
            allowed_by
        ));

        if matched_entries.len() > 1 {
            let message = format!(
                "duplicate semantic identity detected: {} matched {} allowlist entries ({allowed_by})",
                semantic_panic_finding_label(finding),
                matched_entries.len()
            );
            report.warnings.push(message.clone());
            report.violations.push(message);
        }

        for index in matched_entries {
            if let PanicAllowEntryVersioned::V2(entry) = &entries[index]
                && let Some(last_seen) = &entry.last_seen
                && (last_seen.line != finding.line || last_seen.column != finding.column)
            {
                report.advisory_drift.push(format!(
                    "allowed by semantic selector; last_seen changed from {}:{} to {} ({})",
                    last_seen.line,
                    last_seen.column.unwrap_or(0),
                    semantic_panic_finding_label(finding),
                    panic_allow_entry_label(&entries[index])
                ));
            }
        }
    }

    let mut identity_to_entries = BTreeMap::<String, Vec<String>>::new();
    for (index, entry) in entries.iter().enumerate() {
        match entry {
            PanicAllowEntryVersioned::V1(v1) => {
                if match_counts_by_entry[index] == 0 {
                    let message = format!(
                        "stale allowlist entry: {}:{}:{:?} ({}) does not match any current finding",
                        v1.path, v1.line, v1.column, v1.family
                    );
                    report.stale_entries.push(message.clone());
                    report.violations.push(message);
                }
            }
            PanicAllowEntryVersioned::V2(v2) => {
                if let Some(selector) = &v2.selector {
                    identity_to_entries
                        .entry(semantic_selector_identity(v2, selector))
                        .or_default()
                        .push(panic_allow_entry_label(entry));
                }
                let expected = v2.count.unwrap_or(1);
                let actual = match_counts_by_entry[index];
                if actual == 0 {
                    let message = format!(
                        "stale semantic allowlist entry: {} selector does not match any current finding",
                        panic_allow_entry_label(entry)
                    );
                    report.stale_entries.push(message.clone());
                    report.violations.push(message);
                } else if actual < expected {
                    report.advisory_drift.push(format!(
                        "stale-count drift: {} expected entry match count {expected}, matched {actual} (entry match count shrank; debt moved or was removed)",
                        panic_allow_entry_label(entry)
                    ));
                } else if actual > expected {
                    let message = if expected == 1 {
                        format!(
                            "ambiguous semantic allowlist entry: {} matches {actual} current findings",
                            panic_allow_entry_label(entry)
                        )
                    } else {
                        format!(
                            "count exceeded: {} expected entry match count {expected}, matched {actual} (new panic-family call site under existing entry)",
                            panic_allow_entry_label(entry)
                        )
                    };
                    report.warnings.push(message.clone());
                    report.violations.push(message);
                }
            }
        }
    }

    for labels in identity_to_entries.values() {
        if labels.len() > 1 {
            let message = format!(
                "duplicate semantic allowlist identity detected: {}",
                labels.join(", ")
            );
            report.warnings.push(message.clone());
            report.violations.push(message);
        }
    }

    report.allowed_findings.sort();
    report.allowed_findings.dedup();
    report.advisory_drift.sort();
    report.advisory_drift.dedup();
    report.stale_entries.sort();
    report.stale_entries.dedup();
    report.unallowed_findings.sort();
    report.unallowed_findings.dedup();
    report.warnings.sort();
    report.warnings.dedup();
    report.violations.sort();
    report.violations.dedup();
    report
}

fn build_no_panic_allowlist_proposals(
    findings: &[SemanticPanicFinding],
    entries: &[PanicAllowEntryVersioned],
) -> Vec<NoPanicAllowlistProposal> {
    let mut proposals = Vec::new();
    let mut seen = BTreeSet::new();

    for entry in entries {
        match entry {
            PanicAllowEntryVersioned::V1(v1) => {
                let exact_matches = findings
                    .iter()
                    .filter(|finding| {
                        v1.path == finding.path
                            && v1.family == finding.family
                            && v1.line == finding.line
                            && (v1.column.is_none() || v1.column == finding.column)
                    })
                    .collect::<Vec<_>>();
                let (candidate_findings, coordinates_drifted) = if exact_matches.is_empty() {
                    (
                        findings
                            .iter()
                            .filter(|finding| {
                                v1.path == finding.path && v1.family == finding.family
                            })
                            .collect::<Vec<_>>(),
                        true,
                    )
                } else {
                    (exact_matches, false)
                };

                for finding in candidate_findings {
                    let mut proposal = proposal_from_finding(
                        finding,
                        Some(entry),
                        true,
                        Some(format!("{}:{}", v1.line, v1.column.unwrap_or(0))),
                        findings,
                    );
                    if coordinates_drifted {
                        proposal.confidence = "review".to_string();
                        proposal.warnings.push(
                            "v0.1 coordinates did not match a current finding; candidate matched by path and family".to_string(),
                        );
                    }
                    push_no_panic_allowlist_proposal(&mut proposals, &mut seen, proposal);
                }
            }
            PanicAllowEntryVersioned::V2(_) => {
                let matching_findings = findings
                    .iter()
                    .filter(|finding| panic_allow_entry_matches(entry, finding))
                    .collect::<Vec<_>>();
                for finding in matching_findings {
                    push_no_panic_allowlist_proposal(
                        &mut proposals,
                        &mut seen,
                        proposal_from_finding(finding, Some(entry), false, None, findings),
                    );
                }
            }
        }
    }

    proposals.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.last_seen_line.cmp(&right.last_seen_line))
            .then(left.family.cmp(&right.family))
            .then(left.id.cmp(&right.id))
    });
    proposals
}

fn push_no_panic_allowlist_proposal(
    proposals: &mut Vec<NoPanicAllowlistProposal>,
    seen: &mut BTreeSet<String>,
    proposal: NoPanicAllowlistProposal,
) {
    let key = format!(
        "{}:{}:{}:{}",
        proposal.path,
        proposal.family,
        proposal.last_seen_line,
        proposal.last_seen_column.unwrap_or(0)
    );
    if seen.insert(key) {
        proposals.push(proposal);
    }
}

fn proposal_from_finding(
    finding: &SemanticPanicFinding,
    entry: Option<&PanicAllowEntryVersioned>,
    replaces_v1_entry: bool,
    old_coordinates: Option<String>,
    all_findings: &[SemanticPanicFinding],
) -> NoPanicAllowlistProposal {
    let mut warnings = no_panic_selector_proposal_warnings(finding);
    let selector = selector_from_semantic_panic_finding(finding);
    let selector_match_count = all_findings
        .iter()
        .filter(|candidate| {
            candidate.path == finding.path
                && candidate.family == finding.family
                && semantic_selector_matches(&selector, candidate)
        })
        .count();
    if selector_match_count != 1 {
        warnings.push(format!(
            "suggested selector matches {selector_match_count} current findings; review before adopting"
        ));
    }
    if let Some(PanicAllowEntryVersioned::V2(existing)) = entry {
        let existing_entry = PanicAllowEntryVersioned::V2(existing.clone());
        let existing_match_count = all_findings
            .iter()
            .filter(|candidate| panic_allow_entry_matches(&existing_entry, candidate))
            .count();
        if existing_match_count > 1 {
            warnings.push(format!(
                "existing selector matches {existing_match_count} current findings"
            ));
        }
        if existing.selector.as_ref().is_some_and(|existing_selector| {
            existing_selector.receiver_fingerprint.is_none()
                && selector.receiver_fingerprint.is_some()
        }) {
            warnings.push("proposal adds receiver_fingerprint to disambiguate".to_string());
        }
    }

    let confidence = if warnings.is_empty() && selector_match_count == 1 {
        "high"
    } else {
        "review"
    }
    .to_string();
    let (existing_entry, classification, owner, explanation, expires) = match entry {
        Some(PanicAllowEntryVersioned::V1(v1)) => (
            panic_allow_entry_label(&PanicAllowEntryVersioned::V1(v1.clone())),
            v1.classification.clone(),
            None,
            v1.explanation.clone(),
            None,
        ),
        Some(PanicAllowEntryVersioned::V2(v2)) => (
            panic_allow_entry_label(&PanicAllowEntryVersioned::V2(v2.clone())),
            v2.classification.clone(),
            v2.owner.clone(),
            v2.explanation.clone(),
            v2.expires.clone(),
        ),
        None => (
            "new finding".to_string(),
            None,
            None,
            "TODO: review why this panic-family call is allowed".to_string(),
            None,
        ),
    };

    NoPanicAllowlistProposal {
        id: proposal_id_for_finding(finding),
        current_finding: semantic_panic_finding_label(finding),
        path: finding.path.clone(),
        family: finding.family.clone(),
        kind: selector.kind,
        container: selector.container,
        callee: selector.callee,
        receiver_fingerprint: selector.receiver_fingerprint,
        text_contains: selector.text_contains,
        confidence,
        replaces_v1_entry,
        existing_entry,
        old_coordinates,
        last_seen_line: finding.line,
        last_seen_column: finding.column,
        classification,
        owner,
        explanation,
        expires,
        warnings,
    }
}

fn selector_from_semantic_panic_finding(finding: &SemanticPanicFinding) -> PanicFamilySelector {
    if finding.kind == "string_literal" {
        return PanicFamilySelector {
            kind: finding.kind.clone(),
            container: None,
            callee: None,
            receiver_fingerprint: None,
            text_contains: Some(finding.snippet_fingerprint.clone()),
            snippet: None,
        };
    }

    PanicFamilySelector {
        kind: finding.kind.clone(),
        container: finding.container.clone(),
        callee: finding.callee.clone(),
        receiver_fingerprint: finding.receiver_fingerprint.clone(),
        text_contains: None,
        snippet: None,
    }
}

fn no_panic_selector_proposal_warnings(finding: &SemanticPanicFinding) -> Vec<String> {
    let mut warnings = Vec::new();
    if finding.kind == "string_literal" {
        if finding.snippet_fingerprint.trim().is_empty() {
            warnings.push("string_literal proposal has empty text fragment".to_string());
        }
        return warnings;
    }

    if finding
        .container
        .as_ref()
        .is_none_or(|value| value.trim().is_empty())
    {
        warnings.push("finding has no stable container".to_string());
    }
    if finding
        .container
        .as_ref()
        .is_some_and(|container| container.starts_with("closure_"))
    {
        warnings.push("finding uses unstable synthetic closure container".to_string());
    }
    if finding
        .callee
        .as_ref()
        .is_none_or(|value| value.trim().is_empty())
    {
        warnings.push("finding has no exact callee".to_string());
    }
    warnings
}

fn proposal_id_for_finding(finding: &SemanticPanicFinding) -> String {
    format!(
        "proposal-{}-{}-{}",
        no_panic_proposal_id_fragment(&finding.path),
        finding.family,
        finding.line
    )
}

fn no_panic_proposal_id_fragment(value: &str) -> String {
    let mut fragment = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    while fragment.contains("--") {
        fragment = fragment.replace("--", "-");
    }
    fragment.trim_matches('-').to_string()
}

fn render_no_panic_allowlist_proposals_markdown(proposals: &[NoPanicAllowlistProposal]) -> String {
    let mut body = String::new();
    body.push_str("# No-Panic Allowlist Proposals\n\n");
    body.push_str("Status: review-only\n\n");
    body.push_str("These proposals are generated hints. They do not rewrite the canonical allowlist and must be reviewed before adoption.\n\n");
    body.push_str(&format!("Candidates: {}\n\n", proposals.len()));
    if proposals.is_empty() {
        body.push_str("No matching allowlist entries need migration proposals.\n");
        return body;
    }

    for proposal in proposals {
        body.push_str(&format!("## {}\n\n", proposal.id));
        body.push_str(&format!(
            "- Current finding: `{}`\n",
            proposal.current_finding
        ));
        body.push_str(&format!("- Confidence: `{}`\n", proposal.confidence));
        body.push_str(&format!(
            "- Replaces v0.1 entry: `{}`\n",
            proposal.replaces_v1_entry
        ));
        body.push_str(&format!(
            "- Existing entry: `{}`\n",
            proposal.existing_entry
        ));
        body.push_str(&format!(
            "- Old coordinates: `{}`\n",
            proposal
                .old_coordinates
                .as_deref()
                .unwrap_or("not applicable")
        ));
        body.push_str(&format!(
            "- New `last_seen`: `{}:{}`\n",
            proposal.last_seen_line,
            proposal.last_seen_column.unwrap_or(0)
        ));
        body.push_str(&format!("- Reason: {}\n", proposal.explanation));
        body.push_str("- Suggested selector:\n\n");
        body.push_str("```toml\n");
        body.push_str(&render_no_panic_selector_toml(proposal));
        body.push_str("```\n\n");
        if proposal.warnings.is_empty() {
            body.push_str("- Warnings: none\n\n");
        } else {
            body.push_str("- Warnings:\n");
            for warning in &proposal.warnings {
                body.push_str(&format!("  - {warning}\n"));
            }
            body.push('\n');
        }
    }
    body
}

fn render_no_panic_allowlist_proposals_toml(proposals: &[NoPanicAllowlistProposal]) -> String {
    let mut body = String::new();
    body.push_str("# Generated by `cargo xtask check-no-panic-family --propose`.\n");
    body.push_str("# Review-only: fill every TODO-* field before pasting any entry into\n");
    body.push_str("# policy/no-panic-allowlist.toml — the check-no-panic-family gate rejects\n");
    body.push_str("# entries that still contain TODO-* placeholders (#2090).\n");
    body.push_str("schema_version = \"0.3\"\n");
    body.push_str("policy = \"no-panic-allowlist\"\n");
    body.push_str("owner = \"core/policy\"\n");
    body.push_str("status = \"proposal\"\n\n");
    if proposals.is_empty() {
        body.push_str("# No migration proposals.\n");
        return body;
    }

    for proposal in proposals {
        body.push_str(&format!("# Proposal: {}\n", proposal.id));
        body.push_str(&format!(
            "# Current finding: {}\n",
            proposal.current_finding
        ));
        body.push_str(&format!("# Confidence: {}\n", proposal.confidence));
        body.push_str(&format!(
            "# Replaces v0.1 entry: {}\n",
            proposal.replaces_v1_entry
        ));
        if let Some(old_coordinates) = &proposal.old_coordinates {
            body.push_str(&format!("# Old coordinates: {old_coordinates}\n"));
        }
        if proposal.warnings.is_empty() {
            body.push_str("# Warnings: none\n");
        } else {
            body.push_str("# Warnings:\n");
            for warning in &proposal.warnings {
                body.push_str(&format!("# - {warning}\n"));
            }
        }
        body.push_str("[[allow]]\n");
        body.push_str(&format!(
            "id = \"{}\"\n",
            no_panic_toml_string(
                proposal
                    .owner
                    .as_deref()
                    .map_or("TODO-review-id", |_| proposal.id.as_str())
            )
        ));
        body.push_str(&format!(
            "path = \"{}\"\n",
            no_panic_toml_string(&proposal.path)
        ));
        body.push_str(&format!(
            "family = \"{}\"\n",
            no_panic_toml_string(&proposal.family)
        ));
        body.push_str(&format!(
            "classification = \"{}\"\n",
            no_panic_toml_string(
                proposal
                    .classification
                    .as_deref()
                    .unwrap_or("review_required")
            )
        ));
        body.push_str(&format!(
            "owner = \"{}\"\n",
            no_panic_toml_string(proposal.owner.as_deref().unwrap_or("TODO-owner"))
        ));
        body.push_str(&format!(
            "explanation = \"{}\"\n",
            no_panic_toml_string(&proposal.explanation)
        ));
        body.push_str(&format!(
            "expires = \"{}\"\n\n",
            no_panic_toml_string(proposal.expires.as_deref().unwrap_or("TODO-expiry"))
        ));
        body.push_str(&render_no_panic_selector_toml(proposal));
        body.push('\n');
    }
    body
}

fn render_no_panic_selector_toml(proposal: &NoPanicAllowlistProposal) -> String {
    let mut body = String::new();
    body.push_str("[allow.selector]\n");
    body.push_str(&format!(
        "kind = \"{}\"\n",
        no_panic_toml_string(&proposal.kind)
    ));
    if let Some(container) = &proposal.container {
        body.push_str(&format!(
            "container = \"{}\"\n",
            no_panic_toml_string(container)
        ));
    }
    if let Some(callee) = &proposal.callee {
        body.push_str(&format!("callee = \"{}\"\n", no_panic_toml_string(callee)));
    }
    if let Some(receiver_fingerprint) = &proposal.receiver_fingerprint {
        body.push_str(&format!(
            "receiver_fingerprint = \"{}\"\n",
            no_panic_toml_string(receiver_fingerprint)
        ));
    }
    if let Some(text_contains) = &proposal.text_contains {
        body.push_str(&format!(
            "text_contains = \"{}\"\n",
            no_panic_toml_string(text_contains)
        ));
    }
    body.push_str("\n[allow.last_seen]\n");
    body.push_str(&format!("line = {}\n", proposal.last_seen_line));
    if let Some(column) = proposal.last_seen_column {
        body.push_str(&format!("column = {column}\n"));
    }
    body
}

fn no_panic_toml_string(value: &str) -> String {
    let mut escaped = String::new();
    for ch in value.chars() {
        match ch {
            '\u{08}' => escaped.push_str("\\b"),
            '\t' => escaped.push_str("\\t"),
            '\n' => escaped.push_str("\\n"),
            '\u{0c}' => escaped.push_str("\\f"),
            '\r' => escaped.push_str("\\r"),
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            ch if ch <= '\u{1f}' || ch == '\u{7f}' => {
                escaped.push_str(&format!("\\u{:04X}", ch as u32));
            }
            ch => escaped.push(ch),
        }
    }
    escaped
}

fn panic_allow_entry_matches(
    entry: &PanicAllowEntryVersioned,
    finding: &SemanticPanicFinding,
) -> bool {
    match entry {
        PanicAllowEntryVersioned::V1(v1) => {
            v1.path == finding.path
                && v1.family == finding.family
                && v1.line == finding.line
                && (v1.column.is_none() || v1.column == finding.column)
        }
        PanicAllowEntryVersioned::V2(v2) => {
            v2.path == finding.path
                && v2.family == finding.family
                && v2
                    .selector
                    .as_ref()
                    .is_some_and(|selector| semantic_selector_matches(selector, finding))
        }
    }
}

fn semantic_selector_identity(entry: &PanicAllowEntryV2, selector: &PanicFamilySelector) -> String {
    format!(
        "path={}|family={}|kind={}|container={}|callee={}|receiver={}|text={}|snippet={}",
        entry.path,
        entry.family,
        selector.kind,
        selector.container.as_deref().unwrap_or(""),
        selector.callee.as_deref().unwrap_or(""),
        selector.receiver_fingerprint.as_deref().unwrap_or(""),
        selector.text_contains.as_deref().unwrap_or(""),
        selector.snippet.as_deref().unwrap_or("")
    )
}

fn panic_allow_entry_label(entry: &PanicAllowEntryVersioned) -> String {
    match entry {
        PanicAllowEntryVersioned::V1(v1) => format!(
            "{}:{}:{} ({})",
            v1.path,
            v1.line,
            v1.column.unwrap_or(0),
            v1.family
        ),
        PanicAllowEntryVersioned::V2(v2) => {
            let id = v2.id.as_deref().unwrap_or("unversioned");
            let selector = v2
                .selector
                .as_ref()
                .map(panic_selector_label)
                .unwrap_or_else(|| "selector=missing".to_string());
            format!("{id} {} ({}) {selector}", v2.path, v2.family)
        }
    }
}

fn panic_selector_label(selector: &PanicFamilySelector) -> String {
    format!(
        "selector={} container={} callee={} receiver={} text={} snippet={}",
        selector.kind,
        selector.container.as_deref().unwrap_or(""),
        selector.callee.as_deref().unwrap_or(""),
        selector.receiver_fingerprint.as_deref().unwrap_or(""),
        selector.text_contains.as_deref().unwrap_or(""),
        selector.snippet.as_deref().unwrap_or("")
    )
}

fn semantic_panic_finding_label(finding: &SemanticPanicFinding) -> String {
    format!(
        "{}:{}:{}",
        finding.path,
        finding.line,
        finding.column.unwrap_or(0)
    )
}

fn print_no_panic_family_report(report: &NoPanicFamilyReport) {
    let status = if report.violations.is_empty() {
        "pass"
    } else {
        "fail"
    };
    println!("Status: {status}");
    println!();
    print_report_section("Allowed findings", &report.allowed_findings);
    print_report_section("Advisory drift", &report.advisory_drift);
    print_report_section("Stale entries", &report.stale_entries);
    print_report_section("Unallowed findings", &report.unallowed_findings);
    print_report_section("Warnings", &report.warnings);
}

fn print_report_section(title: &str, items: &[String]) {
    println!("{title}:");
    if items.is_empty() {
        println!("- none");
    } else {
        for item in items {
            println!("- {item}");
        }
    }
    println!();
}

fn parse_no_panic_allowlist_toml(path: &str) -> Result<Vec<PanicAllowEntry>, String> {
    let text = read_text_lossy(Path::new(path))?;
    let mut entries = Vec::new();
    let mut in_allow_section = false;
    let mut current_entry = PanicAllowEntry {
        path: String::new(),
        line: 0,
        column: None,
        family: String::new(),
        classification: None,
        explanation: String::new(),
    };
    let mut has_entry_started = false;
    let mut entry_start_line = 0;

    for (line_num, line) in text.lines().enumerate() {
        let line_number = line_num + 1;
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if trimmed == "schema_version = \"0.1\"" {
            continue;
        }

        if trimmed == "[[allow]]" {
            if has_entry_started {
                validate_panic_allow_entry(&current_entry, path, entry_start_line)?;
                entries.push(current_entry.clone());
            }
            current_entry = PanicAllowEntry {
                path: String::new(),
                line: 0,
                column: None,
                family: String::new(),
                classification: None,
                explanation: String::new(),
            };
            has_entry_started = true;
            in_allow_section = true;
            entry_start_line = line_number;
            continue;
        }

        if !in_allow_section {
            return Err(format!(
                "{path}:{} unexpected content outside [[allow]] section",
                line_number
            ));
        }

        let Some((key, value)) = parse_toml_key_value(trimmed) else {
            return Err(format!(
                "{path}:{} invalid TOML syntax (expected key = value)",
                line_number
            ));
        };

        match key {
            "path" => current_entry.path = parse_string_value(value, path, line_number)?,
            "line" => current_entry.line = parse_usize_value(value, path, line_number)?,
            "column" => current_entry.column = Some(parse_usize_value(value, path, line_number)?),
            "family" => current_entry.family = parse_string_value(value, path, line_number)?,
            "classification" => {
                current_entry.classification = Some(parse_string_value(value, path, line_number)?)
            }
            "explanation" => {
                current_entry.explanation = parse_string_value(value, path, line_number)?
            }
            _ => {
                return Err(format!(
                    "{path}:{} unknown field '{key}' in [[allow]] section",
                    line_number
                ));
            }
        }
    }

    if has_entry_started {
        validate_panic_allow_entry(&current_entry, path, entry_start_line)?;
        entries.push(current_entry);
    }

    check_duplicate_panic_allow_entries(&entries, path)?;
    Ok(entries)
}

pub(crate) fn parse_toml_key_value(trimmed: &str) -> Option<(&str, &str)> {
    let equals_idx = trimmed.find('=')?;
    let key = trimmed[..equals_idx].trim();
    let value_part = trimmed[equals_idx + 1..].trim();
    Some((key, value_part))
}

pub(crate) fn parse_string_value(
    value: &str,
    path: &str,
    line_number: usize,
) -> Result<String, String> {
    let v = strip_toml_value_comment(value).trim();
    if v.starts_with('"') && v.ends_with('"') && v.len() >= 2 {
        Ok(unescape_toml_string(&v[1..v.len() - 1]))
    } else {
        Err(format!(
            "{path}:{} string value must be quoted (got: {value})",
            line_number
        ))
    }
}

pub(crate) fn strip_toml_value_comment(value: &str) -> &str {
    let mut in_double = false;
    let mut escaped = false;

    for (idx, ch) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if in_double && ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' {
            in_double = !in_double;
            continue;
        }
        if ch == '#' && !in_double {
            return &value[..idx];
        }
    }

    value
}

fn unescape_toml_string(value: &str) -> String {
    let mut out = String::new();
    let mut chars = value.chars();

    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }

        let Some(next) = chars.next() else {
            out.push('\\');
            break;
        };
        match next {
            '"' => out.push('"'),
            '\\' => out.push('\\'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            other => {
                out.push('\\');
                out.push(other);
            }
        }
    }

    out
}

pub(crate) fn parse_usize_value(
    value: &str,
    path: &str,
    line_number: usize,
) -> Result<usize, String> {
    let v = value.split('#').next().unwrap_or(value).trim();
    v.parse::<usize>()
        .map_err(|_err| format!("{path}:{} invalid number (got: {value})", line_number))
}

fn validate_panic_allow_entry(
    entry: &PanicAllowEntry,
    path: &str,
    line_number: usize,
) -> Result<(), String> {
    if entry.path.is_empty() {
        return Err(format!(
            "{path}:{} missing required field: path",
            line_number
        ));
    }
    if entry.line == 0 {
        return Err(format!(
            "{path}:{} missing required field: line",
            line_number
        ));
    }
    if entry.family.is_empty() {
        return Err(format!(
            "{path}:{} missing required field: family",
            line_number
        ));
    }
    if entry.explanation.is_empty() {
        return Err(format!(
            "{path}:{} missing required field: explanation",
            line_number
        ));
    }
    Ok(())
}

fn check_duplicate_panic_allow_entries(
    entries: &[PanicAllowEntry],
    path: &str,
) -> Result<(), String> {
    let mut seen = BTreeMap::new();
    for entry in entries {
        let key = (
            entry.path.clone(),
            entry.line,
            entry.column,
            entry.family.clone(),
        );
        if seen.contains_key(&key) {
            return Err(format!(
                "{path}: duplicate allowlist entry for {}:{}:{:?} ({})",
                entry.path, entry.line, entry.column, entry.family
            ));
        }
        seen.insert(key, entry.line);
    }
    Ok(())
}

fn check_old_panic_allowlist_exists() -> Result<(), String> {
    if Path::new(".ripr/no-panic-allowlist.txt").exists() {
        return Err(
            ".ripr/no-panic-allowlist.txt still exists; use .ripr/no-panic-allowlist.toml instead"
                .to_string(),
        );
    }
    Ok(())
}

#[derive(Debug, Clone)]
#[allow(
    clippy::large_enum_variant,
    reason = "V2 grows with exact-identity snippet/count fields; boxing is a post-0.5.1 refactor"
)]
enum PanicAllowEntryVersioned {
    V1(PanicAllowEntry),
    V2(PanicAllowEntryV2),
}

fn parse_no_panic_allowlist_toml_v2(path: &str) -> Result<Vec<PanicAllowEntryVersioned>, String> {
    let text = read_text_lossy(Path::new(path))?;
    let mut entries = Vec::new();
    let mut schema_version = "0.1".to_string();

    // Accumulated fields for current entry
    let mut entry_id: Option<String> = None;
    let mut entry_path = String::new();
    let mut entry_line: usize = 0;
    let mut entry_column: Option<usize> = None;
    let mut entry_family = String::new();
    let mut entry_classification: Option<String> = None;
    let mut entry_owner: Option<String> = None;
    let mut entry_explanation = String::new();
    let mut entry_expires: Option<String> = None;
    let mut selector_kind = String::new();
    let mut selector_container: Option<String> = None;
    let mut selector_callee: Option<String> = None;
    let mut selector_receiver_fingerprint: Option<String> = None;
    let mut selector_text_contains: Option<String> = None;
    let mut selector_snippet: Option<String> = None;
    let mut last_seen_line: usize = 0;
    let mut last_seen_column: Option<usize> = None;
    let mut entry_count: Option<usize> = None;

    let mut in_allow_section = false;
    let mut in_selector_section = false;
    let mut in_last_seen_section = false;
    let mut has_entry_started = false;
    let mut entry_start_line = 0;

    let flush_entry = |has_entry: bool,
                       e_id: &Option<String>,
                       e_path: &str,
                       e_line: usize,
                       e_column: Option<usize>,
                       e_family: &str,
                       e_classification: &Option<String>,
                       e_owner: &Option<String>,
                       e_explanation: &str,
                       e_expires: &Option<String>,
                       e_count: Option<usize>,
                       s_kind: &str,
                       s_container: &Option<String>,
                       s_callee: &Option<String>,
                       s_receiver_fp: &Option<String>,
                       s_text_contains: &Option<String>,
                       s_snippet: &Option<String>,
                       ls_line: usize,
                       ls_column: Option<usize>,
                       schema_version: &str,
                       path: &str,
                       start_line: usize|
     -> Result<Option<PanicAllowEntryVersioned>, String> {
        if !has_entry {
            return Ok(None);
        }
        if e_path.is_empty() {
            return Err(format!("{path}:{start_line} missing required field: path"));
        }
        if e_family.is_empty() {
            return Err(format!(
                "{path}:{start_line} missing required field: family"
            ));
        }
        if e_explanation.is_empty() {
            return Err(format!(
                "{path}:{start_line} missing required field: explanation"
            ));
        }
        if schema_version == "0.3" {
            if e_id.as_ref().is_none_or(|value| value.trim().is_empty()) {
                return Err(format!("{path}:{start_line} missing required field: id"));
            }
            if e_classification
                .as_ref()
                .is_none_or(|value| value.trim().is_empty())
            {
                return Err(format!(
                    "{path}:{start_line} missing required field: classification"
                ));
            }
            if e_owner.as_ref().is_none_or(|value| value.trim().is_empty()) {
                return Err(format!("{path}:{start_line} missing required field: owner"));
            }
            if e_expires
                .as_ref()
                .is_none_or(|value| value.trim().is_empty())
            {
                return Err(format!(
                    "{path}:{start_line} missing required field: expires"
                ));
            }
            if s_kind.is_empty() {
                return Err(format!(
                    "{path}:{start_line} schema 0.3 entries require [allow.selector]"
                ));
            }
        }

        if !s_kind.is_empty() {
            // Semantic entry with selector.
            let selector = PanicFamilySelector {
                kind: s_kind.to_string(),
                container: s_container.clone(),
                callee: s_callee.clone(),
                receiver_fingerprint: s_receiver_fp.clone(),
                text_contains: s_text_contains.clone(),
                snippet: s_snippet.clone(),
            };
            let last_seen = if ls_line > 0 {
                Some(PanicFamilyLastSeen {
                    line: ls_line,
                    column: ls_column,
                })
            } else {
                None
            };
            let entry = PanicAllowEntryV2 {
                id: e_id.clone(),
                path: e_path.to_string(),
                family: e_family.to_string(),
                classification: e_classification.clone(),
                owner: e_owner.clone(),
                explanation: e_explanation.to_string(),
                expires: e_expires.clone(),
                selector: Some(selector),
                last_seen,
                count: e_count,
            };
            validate_panic_allow_entry_v2(&entry, path, start_line, schema_version)?;
            Ok(Some(PanicAllowEntryVersioned::V2(entry)))
        } else if e_line > 0 {
            // v0.1 entry with line number
            let entry = PanicAllowEntry {
                path: e_path.to_string(),
                line: e_line,
                column: e_column,
                family: e_family.to_string(),
                classification: e_classification.clone(),
                explanation: e_explanation.to_string(),
            };
            validate_panic_allow_entry(&entry, path, start_line)?;
            Ok(Some(PanicAllowEntryVersioned::V1(entry)))
        } else {
            Err(format!(
                "{path}:{start_line} entry must have either a [allow.selector] or line number"
            ))
        }
    };

    for (line_num, line) in text.lines().enumerate() {
        let line_number = line_num + 1;
        let trimmed = line.trim().trim_start_matches('\u{feff}');

        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some((key, value)) = parse_toml_key_value(trimmed)
            && key == "schema_version"
        {
            schema_version = parse_string_value(value, path, line_number)?;
            if schema_version != "0.1" && schema_version != "0.2" && schema_version != "0.3" {
                return Err(format!(
                    "{path}:{line_number} unsupported no-panic allowlist schema_version {schema_version:?}"
                ));
            }
            continue;
        }

        if !has_entry_started
            && matches!(
                parse_toml_key_value(trimmed).map(|(key, _value)| key),
                Some("policy" | "owner" | "status")
            )
        {
            continue;
        }

        if trimmed == "[allow.selector]" {
            in_selector_section = true;
            in_last_seen_section = false;
            in_allow_section = false;
            continue;
        }

        if trimmed == "[allow.last_seen]" {
            in_last_seen_section = true;
            in_selector_section = false;
            in_allow_section = false;
            continue;
        }

        if trimmed == "[[allow]]" {
            // Flush previous entry
            let result = flush_entry(
                has_entry_started,
                &entry_id,
                &entry_path,
                entry_line,
                entry_column,
                &entry_family,
                &entry_classification,
                &entry_owner,
                &entry_explanation,
                &entry_expires,
                entry_count,
                &selector_kind,
                &selector_container,
                &selector_callee,
                &selector_receiver_fingerprint,
                &selector_text_contains,
                &selector_snippet,
                last_seen_line,
                last_seen_column,
                &schema_version,
                path,
                entry_start_line,
            )?;
            if let Some(entry) = result {
                entries.push(entry);
            }

            // Reset all fields for new entry
            entry_id = None;
            entry_path = String::new();
            entry_line = 0;
            entry_column = None;
            entry_family = String::new();
            entry_classification = None;
            entry_owner = None;
            entry_explanation = String::new();
            entry_expires = None;
            selector_kind = String::new();
            selector_container = None;
            selector_callee = None;
            selector_receiver_fingerprint = None;
            selector_text_contains = None;
            selector_snippet = None;
            last_seen_line = 0;
            last_seen_column = None;
            entry_count = None;

            in_selector_section = false;
            in_last_seen_section = false;
            in_allow_section = true;
            has_entry_started = true;
            entry_start_line = line_number;
            continue;
        }

        if !in_allow_section && !in_selector_section && !in_last_seen_section {
            return Err(format!(
                "{path}:{line_number} unexpected content outside [[allow]] section"
            ));
        }

        let Some((key, value)) = parse_toml_key_value(trimmed) else {
            return Err(format!(
                "{path}:{line_number} invalid TOML syntax (expected key = value)"
            ));
        };

        if in_selector_section {
            match key {
                "kind" => {
                    selector_kind = parse_string_value(value, path, line_number)?;
                }
                "container" => {
                    selector_container = Some(parse_string_value(value, path, line_number)?);
                }
                "callee" => {
                    selector_callee = Some(parse_string_value(value, path, line_number)?);
                }
                "receiver_fingerprint" => {
                    selector_receiver_fingerprint =
                        Some(parse_string_value(value, path, line_number)?);
                }
                "text_contains" => {
                    selector_text_contains = Some(parse_string_value(value, path, line_number)?);
                }
                "snippet" => {
                    selector_snippet = Some(parse_string_value(value, path, line_number)?);
                }
                _ => {
                    return Err(format!(
                        "{path}:{line_number} unknown field '{key}' in [allow.selector] section"
                    ));
                }
            }
            continue;
        }

        if in_last_seen_section {
            match key {
                "line" => {
                    last_seen_line = parse_usize_value(value, path, line_number)?;
                }
                "column" => {
                    last_seen_column = Some(parse_usize_value(value, path, line_number)?);
                }
                _ => {
                    return Err(format!(
                        "{path}:{line_number} unknown field '{key}' in [allow.last_seen] section"
                    ));
                }
            }
            continue;
        }

        // In [[allow]] section
        match key {
            "id" => entry_id = Some(parse_string_value(value, path, line_number)?),
            "path" => entry_path = parse_string_value(value, path, line_number)?,
            "line" => entry_line = parse_usize_value(value, path, line_number)?,
            "column" => entry_column = Some(parse_usize_value(value, path, line_number)?),
            "family" => entry_family = parse_string_value(value, path, line_number)?,
            "classification" => {
                entry_classification = Some(parse_string_value(value, path, line_number)?)
            }
            "owner" => entry_owner = Some(parse_string_value(value, path, line_number)?),
            "explanation" => entry_explanation = parse_string_value(value, path, line_number)?,
            "expires" => entry_expires = Some(parse_string_value(value, path, line_number)?),
            "count" => entry_count = Some(parse_usize_value(value, path, line_number)?),
            _ => {
                return Err(format!(
                    "{path}:{line_number} unknown field '{key}' in [[allow]] section"
                ));
            }
        }
    }

    // Flush final entry
    let result = flush_entry(
        has_entry_started,
        &entry_id,
        &entry_path,
        entry_line,
        entry_column,
        &entry_family,
        &entry_classification,
        &entry_owner,
        &entry_explanation,
        &entry_expires,
        entry_count,
        &selector_kind,
        &selector_container,
        &selector_callee,
        &selector_receiver_fingerprint,
        &selector_text_contains,
        &selector_snippet,
        last_seen_line,
        last_seen_column,
        &schema_version,
        path,
        entry_start_line,
    )?;
    if let Some(entry) = result {
        entries.push(entry);
    }

    Ok(entries)
}

fn validate_panic_allow_entry_v2(
    entry: &PanicAllowEntryV2,
    path: &str,
    line_number: usize,
    schema_version: &str,
) -> Result<(), String> {
    if entry.path.is_empty() {
        return Err(format!("{path}:{line_number} missing required field: path"));
    }
    if entry.family.is_empty() {
        return Err(format!(
            "{path}:{line_number} missing required field: family"
        ));
    }
    if entry.explanation.is_empty() {
        return Err(format!(
            "{path}:{line_number} missing required field: explanation"
        ));
    }
    if schema_version == "0.3" {
        if entry
            .id
            .as_ref()
            .is_none_or(|value| value.trim().is_empty())
        {
            return Err(format!("{path}:{line_number} missing required field: id"));
        }
        if entry
            .classification
            .as_ref()
            .is_none_or(|value| value.trim().is_empty())
        {
            return Err(format!(
                "{path}:{line_number} missing required field: classification"
            ));
        }
        if entry
            .owner
            .as_ref()
            .is_none_or(|value| value.trim().is_empty())
        {
            return Err(format!(
                "{path}:{line_number} missing required field: owner"
            ));
        }
        if entry
            .expires
            .as_ref()
            .is_none_or(|value| value.trim().is_empty())
        {
            return Err(format!(
                "{path}:{line_number} missing required field: expires"
            ));
        }
    }
    // A pasted --propose entry is not policy (#2090): unfilled proposal
    // placeholders must fail the gate, not pass as reviewed data.
    for (field, value) in [
        ("id", entry.id.as_ref()),
        ("owner", entry.owner.as_ref()),
        ("expires", entry.expires.as_ref()),
    ] {
        if let Some(value) = value
            && value.starts_with("TODO")
        {
            return Err(format!(
                "{path}:{line_number} {field} contains an unfilled proposal placeholder \
                 ({value}); replace every TODO-* field from the --propose output with \
                 reviewed content before committing"
            ));
        }
    }
    if let Some(ref selector) = entry.selector {
        if selector.kind.is_empty() {
            return Err(format!(
                "{path}:{line_number} selector missing required field: kind"
            ));
        }
        let supported_kinds = ["method_call", "macro_call", "call", "string_literal"];
        if !supported_kinds.contains(&selector.kind.as_str()) {
            return Err(format!(
                "{path}:{line_number} invalid selector kind '{}' in {path}; expected one of: {}",
                selector.kind,
                supported_kinds.join(", ")
            ));
        }
        if selector.kind == "string_literal" && selector.text_contains.is_none() {
            return Err(format!(
                "{path}:{line_number} string_literal selector requires text_contains"
            ));
        }
        if matches!(
            selector.kind.as_str(),
            "method_call" | "macro_call" | "call"
        ) {
            if selector
                .container
                .as_ref()
                .is_none_or(|value| value.trim().is_empty())
            {
                return Err(format!(
                    "{path}:{line_number} {} selector requires container",
                    selector.kind
                ));
            }
            if selector
                .callee
                .as_ref()
                .is_none_or(|value| value.trim().is_empty())
            {
                return Err(format!(
                    "{path}:{line_number} {} selector requires callee",
                    selector.kind
                ));
            }
            if let Some(container) = selector.container.as_deref()
                && container.starts_with("closure_")
            {
                return Err(format!(
                    "{path}:{line_number} {} selector uses unstable synthetic container `{container}`",
                    selector.kind
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "xtask test code uses unwrap for fail-fast assertion. Production paths are receipted via policy/no-panic-allowlist.toml; the test scope is governed by this single module-level expect."
)]
mod tests;
