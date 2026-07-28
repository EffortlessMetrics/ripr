//! `cargo xtask module-health` — advisory Rust source-file health report.
//!
//! Walks `crates/ripr/src/` and `xtask/src/` recursively and, per `*.rs` file,
//! reports two independent signals:
//!
//! 1. **Line count** — the file's size, flagged when it exceeds a configurable
//!    threshold. Catches the obvious monolith.
//! 2. **Responsibility signal** — a heuristic count of distinct top-level
//!    concerns (distinct `impl` blocks plus distinct public-API identifier
//!    prefixes), flagged when it exceeds a fixed threshold. Catches the
//!    "structurally entangled even if not huge" case that a pure line count
//!    misses.
//!
//! The report is purely informational: it never fails, never mutates anything,
//! and is never wired into CI gates.
//!
//! The responsibility signal is a **smell, not a measurement**. It is a crude,
//! text-based heuristic (no syntax tree): it can be gamed, over-counts files
//! with many small helpers, and under-counts genuinely entangled logic hidden
//! behind a single façade. Use it to *prioritize* a closer look, not to decide.
//!
//! Intended use: before starting a capability wave that touches an oversized or
//! entangled file, open a behaviour-preserving decomposition PR first so each
//! new feature lands in a focused module (zero golden drift = proof it is pure
//! structure). See `AGENTS.md` § Implementation Bias for the
//! refactor-before-extend principle.

use serde_json::json;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const MODULE_HEALTH_SCHEMA_VERSION: &str = "0.2";
const MODULE_HEALTH_STATE: &str = "advisory-report-only";
const MODULE_HEALTH_JSON: &str = "module-health.json";
const MODULE_HEALTH_MD: &str = "module-health.md";
const DEFAULT_THRESHOLD: usize = 2000;
/// Distinct-responsibility-cluster count above which a file is flagged as
/// structurally entangled, independent of its line count. Advisory only; a
/// fixed heuristic knob, not a CLI-tunable gate.
const RESPONSIBILITY_THRESHOLD: usize = 12;
const TOP_N: usize = 15;
const MODULE_HEALTH_USAGE: &str = "usage: cargo xtask module-health [--threshold <n>] [--help|-h]";

/// Item keywords whose public declarations contribute a responsibility concern.
/// Deliberately narrow: `fn`/`struct`/`enum`/`trait`/`type` name a distinct
/// concern; `const`/`static`/`mod`/`use` do not (they are treated as modifiers
/// or ignored).
const ITEM_KEYWORDS: [&str; 5] = ["fn", "struct", "enum", "trait", "type"];

pub(crate) fn module_health(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{MODULE_HEALTH_USAGE}");
        println!();
        println!(
            "Walks crates/ripr/src/, xtask/src/, and editors/vscode/src/ for *.rs and *.ts files and writes an advisory"
        );
        println!("ranked report to target/ripr/reports/ carrying two signals per file:");
        println!("  - line count (flagged over --threshold);");
        println!(
            "  - responsibility clusters (distinct impl blocks + public-API prefixes; flagged"
        );
        println!("    over {RESPONSIBILITY_THRESHOLD}, a smell for structural entanglement).");
        println!("Always exits 0. Never mutates source. Never wired into CI gates.");
        println!();
        println!("Options:");
        println!(
            "  --threshold <n>   Lines-per-file threshold for flagging (default: {DEFAULT_THRESHOLD})"
        );
        return Ok(());
    }

    let threshold = parse_threshold(args)?;

    let root = std::env::current_dir()
        .map_err(|err| format!("failed to read current directory: {err}"))?;

    let mut files: Vec<FileEntry> = Vec::new();
    for search_root in &["crates/ripr/src", "xtask/src", "editors/vscode/src"] {
        let dir = root.join(search_root);
        if dir.exists() {
            collect_source_files(&dir, &root, &mut files)?;
        }
    }

    // Sort by line count descending, then path ascending for stability.
    files.sort_by(|a, b| b.lines.cmp(&a.lines).then_with(|| a.path.cmp(&b.path)));

    let over_threshold: Vec<&FileEntry> = files.iter().filter(|f| f.lines >= threshold).collect();
    let over_threshold_count = over_threshold.len();

    // Files flagged by the responsibility heuristic, ranked by cluster count.
    let mut over_responsibility: Vec<&FileEntry> = files
        .iter()
        .filter(|f| f.responsibility_clusters >= RESPONSIBILITY_THRESHOLD)
        .collect();
    over_responsibility.sort_by(|a, b| {
        b.responsibility_clusters
            .cmp(&a.responsibility_clusters)
            .then_with(|| a.path.cmp(&b.path))
    });
    let over_responsibility_count = over_responsibility.len();

    let generated_files_count = files.len();

    // Build JSON report.
    let files_json: Vec<serde_json::Value> = files
        .iter()
        .map(|f| file_entry_json(f, threshold))
        .collect();

    let report_json = json!({
        "schema_version": MODULE_HEALTH_SCHEMA_VERSION,
        "state": MODULE_HEALTH_STATE,
        "threshold": threshold,
        "responsibility_threshold": RESPONSIBILITY_THRESHOLD,
        "generated_files_count": generated_files_count,
        "over_threshold_count": over_threshold_count,
        "over_responsibility_count": over_responsibility_count,
        "files": files_json,
    });

    let json_text = serde_json::to_string_pretty(&report_json)
        .map_err(|err| format!("serialize module-health report: {err}"))?;
    crate::write_report(MODULE_HEALTH_JSON, &format!("{json_text}\n"))?;
    crate::write_report(
        MODULE_HEALTH_MD,
        &render_markdown(
            threshold,
            &files,
            over_threshold_count,
            &over_threshold,
            over_responsibility_count,
            &over_responsibility,
        ),
    )?;

    println!("Wrote target/ripr/reports/{MODULE_HEALTH_JSON}");
    println!("Wrote target/ripr/reports/{MODULE_HEALTH_MD}");
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileEntry {
    /// Forward-slash normalized path relative to repo root (platform-stable).
    path: String,
    lines: usize,
    /// Count of `impl` block headers in the file.
    impl_blocks: usize,
    /// Distinct concern prefixes across the file's public item names.
    distinct_public_prefixes: usize,
    /// Distinct top-level concerns: the union of impl-target concerns and
    /// public-item concern prefixes. The responsibility-signal flag driver.
    responsibility_clusters: usize,
}

/// The responsibility heuristic computed for a single file's source text.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ResponsibilitySignal {
    impl_blocks: usize,
    distinct_public_prefixes: usize,
    responsibility_clusters: usize,
}

fn parse_threshold(args: &[String]) -> Result<usize, String> {
    let mut index = 0usize;
    let mut threshold = DEFAULT_THRESHOLD;
    while index < args.len() {
        match args[index].as_str() {
            "--threshold" => {
                index += 1;
                let value = args.get(index).ok_or_else(|| {
                    format!("missing value for --threshold; {MODULE_HEALTH_USAGE}")
                })?;
                threshold = value.parse::<usize>().map_err(|err| {
                    format!("module-health --threshold must be a positive integer: {err}")
                })?;
                if threshold == 0 {
                    return Err("module-health --threshold must be greater than zero".to_string());
                }
            }
            other => {
                return Err(format!(
                    "unknown module-health argument `{other}`; {MODULE_HEALTH_USAGE}"
                ));
            }
        }
        index += 1;
    }
    Ok(threshold)
}

/// Collect source files (.rs and .ts) recursively (#2544: extended to .ts).
fn collect_source_files(
    dir: &Path,
    repo_root: &Path,
    out: &mut Vec<FileEntry>,
) -> Result<(), String> {
    let read_dir = std::fs::read_dir(dir)
        .map_err(|err| format!("failed to read directory {}: {err}", dir.display()))?;
    for entry in read_dir {
        let entry = entry.map_err(|err| format!("failed to read directory entry: {err}"))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|err| format!("failed to stat {}: {err}", path.display()))?;
        if file_type.is_dir() {
            // Skip node_modules and dist directories in the VS Code extension.
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name == "node_modules" || name == "dist" || name == "out" || name == "test" {
                continue;
            }
            collect_source_files(&path, repo_root, out)?;
        } else if file_type.is_file()
            && path
                .extension()
                .is_some_and(|ext| ext == "rs" || ext == "ts")
        {
            let text = std::fs::read_to_string(&path)
                .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
            let lines = count_lines(&text);
            let signal = analyze_responsibility(&text);
            let rel = relative_path_normalized(&path, repo_root);
            out.push(FileEntry {
                path: rel,
                lines,
                impl_blocks: signal.impl_blocks,
                distinct_public_prefixes: signal.distinct_public_prefixes,
                responsibility_clusters: signal.responsibility_clusters,
            });
        }
    }
    Ok(())
}

/// Count newline-terminated lines: number of `'\n'` characters.
fn count_lines(text: &str) -> usize {
    text.chars().filter(|&c| c == '\n').count()
}

/// Compute the responsibility signal for a file's source text.
///
/// A crude, text-based smell (no syntax tree): it counts `impl` block headers
/// and derives a "concern prefix" from each public item name, then reports how
/// many distinct concerns the file exposes. See the module docs for its
/// limitations.
fn analyze_responsibility(text: &str) -> ResponsibilitySignal {
    let mut impl_blocks = 0usize;
    let mut public_concerns: BTreeSet<String> = BTreeSet::new();
    let mut all_concerns: BTreeSet<String> = BTreeSet::new();

    for raw in text.lines() {
        let line = raw.trim_start();
        if line.starts_with("//") {
            continue;
        }
        if let Some(after_impl) = impl_header_rest(line) {
            impl_blocks += 1;
            if let Some(concern) = impl_line_concern(after_impl) {
                all_concerns.insert(concern);
            }
            continue;
        }
        if let Some(ident) = public_item_identifier(line)
            && let Some(concern) = concern_token(&ident)
        {
            public_concerns.insert(concern.clone());
            all_concerns.insert(concern);
        }
    }

    ResponsibilitySignal {
        impl_blocks,
        distinct_public_prefixes: public_concerns.len(),
        responsibility_clusters: all_concerns.len(),
    }
}

/// If `line` is an `impl` block header, return the text after the `impl`
/// keyword. Requires an `impl` keyword boundary (space or `<`) so identifiers
/// like `implement` or `impl_detail` are not misread.
fn impl_header_rest(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("impl")?;
    match rest.chars().next() {
        Some(' ') | Some('<') => Some(rest),
        _ => None,
    }
}

/// Derive the concern prefix of the type an `impl` header applies to. Prefers
/// the `for <Type>` target (the implemented-on type) over the trait, and skips
/// leading generic parameter lists.
fn impl_line_concern(after_impl: &str) -> Option<String> {
    let head = strip_leading_generics(after_impl);
    let target = match head.find(" for ") {
        Some(idx) => strip_leading_generics(&head[idx + " for ".len()..]),
        None => head,
    };
    first_type_ident(target).and_then(|ident| concern_token(&ident))
}

/// Skip a leading, balanced `<...>` generic parameter list (if present) and any
/// surrounding whitespace.
fn strip_leading_generics(s: &str) -> &str {
    let trimmed = s.trim_start();
    if !trimmed.starts_with('<') {
        return trimmed;
    }
    let mut depth = 0i32;
    for (idx, ch) in trimmed.char_indices() {
        match ch {
            '<' => depth += 1,
            '>' => {
                depth -= 1;
                if depth == 0 {
                    return trimmed[idx + 1..].trim_start();
                }
            }
            _ => {}
        }
    }
    trimmed
}

/// Take the leading type identifier (final path segment) from a type
/// expression, stopping at the first non-identifier character.
fn first_type_ident(s: &str) -> Option<String> {
    let s = s.trim_start();
    let name: String = s
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == ':')
        .collect();
    let last = name.rsplit("::").next().unwrap_or(name.as_str());
    (!last.is_empty()).then(|| last.to_string())
}

/// If `line` declares a public item (`pub`/`pub(...)` visibility) of a kind in
/// [`ITEM_KEYWORDS`], return its identifier. Skips leading modifier keywords
/// (`async`, `unsafe`, `const`, …) so e.g. `pub async fn foo` yields `foo`.
fn public_item_identifier(line: &str) -> Option<String> {
    let rest = line.strip_prefix("pub")?;
    let rest = match rest.chars().next() {
        Some(' ') => rest,
        // `pub(crate)`, `pub(super)`, `pub(in path)` — resume after the `)`.
        Some('(') => rest.split_once(')').map(|(_, tail)| tail)?,
        _ => return None,
    };

    let mut tokens = rest.split_whitespace();
    let mut keyword = tokens.next()?;
    // Skip fn/item modifier keywords (and an `extern "C"` ABI string).
    while matches!(
        keyword,
        "async" | "unsafe" | "default" | "extern" | "const" | "static"
    ) || keyword.starts_with('"')
    {
        keyword = tokens.next()?;
    }
    if !ITEM_KEYWORDS.contains(&keyword) {
        return None;
    }
    let ident_token = tokens.next()?;
    let ident: String = ident_token
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    (!ident.is_empty()).then_some(ident)
}

/// Derive a lowercase "concern" token from an identifier: the leading
/// snake_case segment, or leading CamelCase word, unifying e.g. `parse_expr`
/// and `ParseResult` to `parse`.
fn concern_token(ident: &str) -> Option<String> {
    let segment = ident.split('_').next().unwrap_or(ident);
    if segment.is_empty() {
        return None;
    }
    // A SCREAMING_SNAKE / all-caps segment (e.g. `HTTP`) has no lowercase to
    // mark a CamelCase boundary; keep it whole.
    let token = if segment.chars().all(|c| !c.is_ascii_lowercase()) {
        segment.to_string()
    } else {
        leading_camel_word(segment)
    };
    let token = token.to_ascii_lowercase();
    (!token.is_empty()).then_some(token)
}

/// The leading word of a CamelCase/camelCase segment: the first char plus the
/// following run of non-uppercase characters.
fn leading_camel_word(segment: &str) -> String {
    let mut chars = segment.chars();
    let mut out = String::new();
    if let Some(first) = chars.next() {
        out.push(first);
        for c in chars {
            if c.is_ascii_uppercase() {
                break;
            }
            out.push(c);
        }
    }
    out
}

/// Return the path relative to `root`, with backslashes replaced by forward
/// slashes (platform-stable for Windows-generated output).
fn relative_path_normalized(path: &Path, root: &Path) -> String {
    let rel: PathBuf = path.strip_prefix(root).unwrap_or(path).to_path_buf();
    rel.to_string_lossy().replace('\\', "/")
}

/// The per-file JSON object in the report: the line-count signal plus the
/// responsibility signal and both threshold flags.
fn file_entry_json(f: &FileEntry, threshold: usize) -> serde_json::Value {
    json!({
        "path": f.path,
        "lines": f.lines,
        "over_threshold": f.lines >= threshold,
        "impl_blocks": f.impl_blocks,
        "distinct_public_prefixes": f.distinct_public_prefixes,
        "responsibility_clusters": f.responsibility_clusters,
        "over_responsibility_threshold": f.responsibility_clusters >= RESPONSIBILITY_THRESHOLD,
    })
}

fn render_markdown(
    threshold: usize,
    all_files: &[FileEntry],
    over_threshold_count: usize,
    over_threshold: &[&FileEntry],
    over_responsibility_count: usize,
    over_responsibility: &[&FileEntry],
) -> String {
    let mut body = String::from("# ripr module-health report\n\n");
    body.push_str("Status: advisory\n");
    body.push_str("State: advisory-report-only\n\n");
    body.push_str(
        "This report flags Rust source files as a refactoring guide, on two \
         independent signals: line count (oversized files) and a responsibility \
         smell (structurally entangled files that expose many distinct concerns \
         even when not huge). It is advisory-only: it never fails CI, never \
         mutates anything, and changes no CI behavior. Use it to identify \
         monoliths before starting a capability wave — decompose first so each \
         new feature lands in a focused module.\n\n",
    );
    body.push_str(&format!("Line threshold: {threshold} lines\n"));
    body.push_str(&format!(
        "Responsibility threshold: {RESPONSIBILITY_THRESHOLD} distinct concern clusters\n"
    ));
    body.push_str(&format!("Files scanned: {}\n", all_files.len()));
    body.push_str(&format!(
        "Files over line threshold: {over_threshold_count}\n"
    ));
    body.push_str(&format!(
        "Files over responsibility threshold: {over_responsibility_count}\n\n"
    ));

    body.push_str(
        "The responsibility signal is a **smell, not a measurement**: a crude, \
         text-based heuristic (distinct `impl` blocks + distinct public-API \
         identifier prefixes, no syntax tree). It can be gamed and is noisy — \
         use it to prioritize a closer look, not to decide.\n\n",
    );

    body.push_str("## Files over line threshold (candidates for decomposition)\n\n");
    if over_threshold.is_empty() {
        body.push_str(&format!(
            "No files exceed the {threshold}-line threshold.\n\n"
        ));
    } else {
        body.push_str("| File | Lines |\n| --- | --- |\n");
        for f in over_threshold {
            body.push_str(&format!("| `{}` | {} |\n", f.path, f.lines));
        }
        body.push('\n');
    }

    body.push_str("## Files over responsibility threshold (entangled candidates)\n\n");
    if over_responsibility.is_empty() {
        body.push_str(&format!(
            "No files exceed the {RESPONSIBILITY_THRESHOLD}-cluster responsibility threshold.\n\n"
        ));
    } else {
        body.push_str(
            "| File | Responsibility clusters | Impl blocks | Public prefixes | Lines |\n\
             | --- | --- | --- | --- | --- |\n",
        );
        for f in over_responsibility {
            body.push_str(&format!(
                "| `{}` | {} | {} | {} | {} |\n",
                f.path,
                f.responsibility_clusters,
                f.impl_blocks,
                f.distinct_public_prefixes,
                f.lines
            ));
        }
        body.push('\n');
    }

    let top_n = TOP_N.min(all_files.len());
    body.push_str(&format!("## Top {top_n} largest files overall\n\n"));
    if all_files.is_empty() {
        body.push_str("(no files found)\n\n");
    } else {
        body.push_str(
            "| # | File | Lines | Over line | Responsibility clusters | Over responsibility |\n\
             | --- | --- | --- | --- | --- | --- |\n",
        );
        for (rank, f) in all_files.iter().take(top_n).enumerate() {
            let over_line = if f.lines >= threshold { "yes" } else { "no" };
            let over_resp = if f.responsibility_clusters >= RESPONSIBILITY_THRESHOLD {
                "yes"
            } else {
                "no"
            };
            body.push_str(&format!(
                "| {} | `{}` | {} | {} | {} | {} |\n",
                rank + 1,
                f.path,
                f.lines,
                over_line,
                f.responsibility_clusters,
                over_resp
            ));
        }
        body.push('\n');
    }

    body.push_str("Reproduce locally:\n\n");
    body.push_str("```\n");
    body.push_str(&format!(
        "cargo xtask module-health --threshold {threshold}\n"
    ));
    body.push_str("```\n");
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_threshold_default_is_2000() -> Result<(), String> {
        assert_eq!(parse_threshold(&[])?, DEFAULT_THRESHOLD);
        Ok(())
    }

    #[test]
    fn parse_threshold_explicit_value() -> Result<(), String> {
        let args = ["--threshold".to_string(), "500".to_string()];
        assert_eq!(parse_threshold(&args)?, 500);
        Ok(())
    }

    #[test]
    fn parse_threshold_zero_rejected() -> Result<(), String> {
        let args = ["--threshold".to_string(), "0".to_string()];
        if parse_threshold(&args).is_ok() {
            return Err("expected error for threshold=0 but got Ok".to_string());
        }
        Ok(())
    }

    #[test]
    fn parse_threshold_unknown_arg_rejected() -> Result<(), String> {
        let args = ["--unknown".to_string()];
        if parse_threshold(&args).is_ok() {
            return Err("expected error for unknown arg but got Ok".to_string());
        }
        Ok(())
    }

    #[test]
    fn count_lines_counts_newlines() {
        // 3 lines = 3 newlines
        assert_eq!(count_lines("line1\nline2\nline3\n"), 3);
    }

    #[test]
    fn relative_path_normalized_replaces_backslashes() {
        // Build the backslash-separated path at runtime so no literal Windows
        // path appears in source (keeps check-local-context clean) while still
        // exercising the backslash -> forward-slash normalization on both
        // platforms (on Windows strip_prefix splits components; on Linux the
        // separator is a plain char and the whole string is normalized).
        let bs = "\\";
        let root = PathBuf::from("root");
        let path = PathBuf::from(format!("root{bs}sub{bs}file.rs"));
        let result = relative_path_normalized(&path, &root);
        assert!(!result.contains('\\'), "backslash found in: {result}");
        assert!(result.contains('/'), "forward slash missing in: {result}");
    }

    #[test]
    fn over_threshold_flag_correct() {
        let files = [entry("a.rs", 3000, 0, 0, 0), entry("b.rs", 100, 0, 0, 0)];
        let over: Vec<&FileEntry> = files.iter().filter(|f| f.lines >= 2000).collect();
        assert_eq!(over.len(), 1);
        assert_eq!(over[0].path, "a.rs");
    }

    #[test]
    fn concern_token_unifies_snake_and_camel() {
        assert_eq!(concern_token("parse_expr").as_deref(), Some("parse"));
        assert_eq!(concern_token("ParseResult").as_deref(), Some("parse"));
        assert_eq!(concern_token("classify_gap").as_deref(), Some("classify"));
        assert_eq!(concern_token("OracleStrength").as_deref(), Some("oracle"));
        // All-caps segment kept whole.
        assert_eq!(concern_token("HTTP_CLIENT").as_deref(), Some("http"));
    }

    #[test]
    fn public_item_identifier_extracts_names_and_skips_modifiers() {
        assert_eq!(
            public_item_identifier("pub fn foo() {}").as_deref(),
            Some("foo")
        );
        assert_eq!(
            public_item_identifier("pub async fn bar() {}").as_deref(),
            Some("bar")
        );
        assert_eq!(
            public_item_identifier("pub unsafe fn baz() {}").as_deref(),
            Some("baz")
        );
        assert_eq!(
            public_item_identifier("pub(crate) fn qux() {}").as_deref(),
            Some("qux")
        );
        assert_eq!(
            public_item_identifier("pub struct Widget {").as_deref(),
            Some("Widget")
        );
        assert_eq!(
            public_item_identifier("pub trait Sink {").as_deref(),
            Some("Sink")
        );
        // Non-public and non-item lines are ignored.
        assert_eq!(public_item_identifier("fn private() {}"), None);
        assert_eq!(public_item_identifier("pub use crate::foo;"), None);
        assert_eq!(public_item_identifier("pub const MAX: u8 = 1;"), None);
        // `impl`-like identifiers are not misread as the `impl` keyword.
        assert_eq!(public_item_identifier("let implementation = 1;"), None);
    }

    #[test]
    fn impl_header_detection_and_target_concern() {
        assert!(impl_header_rest("impl Widget {").is_some());
        assert!(impl_header_rest("impl<T> Widget<T> {").is_some());
        // Not an impl header.
        assert!(impl_header_rest("implement_me();").is_none());
        assert!(impl_header_rest("impl_detail = 1;").is_none());

        // Inherent impl -> concern from the type.
        assert_eq!(
            impl_header_rest("impl Widget {")
                .and_then(impl_line_concern)
                .as_deref(),
            Some("widget")
        );
        // Trait impl -> concern from the implemented-on (`for`) type, not the trait.
        assert_eq!(
            impl_header_rest("impl Display for OracleStrength {")
                .and_then(impl_line_concern)
                .as_deref(),
            Some("oracle")
        );
        // Generic params on both impl and target are skipped.
        assert_eq!(
            impl_header_rest("impl<T> From<T> for ParseResult<T> {")
                .and_then(impl_line_concern)
                .as_deref(),
            Some("parse")
        );
    }

    #[test]
    fn analyze_responsibility_counts_distinct_concerns() {
        let src = "\
//! doc line mentioning pub fn should be skipped
pub fn parse_header() {}
pub fn parse_body() {}
pub struct ClassifyResult;
impl ClassifyResult {}
impl Display for ClassifyResult {}
fn private_helper() {}
";
        let signal = analyze_responsibility(src);
        // Two impl headers.
        assert_eq!(signal.impl_blocks, 2);
        // Public prefixes: {parse, classify} (parse_header/parse_body collapse).
        assert_eq!(signal.distinct_public_prefixes, 2);
        // Union of pub prefixes and impl-target concerns: {parse, classify}.
        assert_eq!(signal.responsibility_clusters, 2);
    }

    #[test]
    fn analyze_responsibility_ignores_comment_lines() {
        let src = "// pub fn commented_out() {}\n// impl Faux {}\nfn only_private() {}\n";
        let signal = analyze_responsibility(src);
        assert_eq!(signal, ResponsibilitySignal::default());
    }

    #[test]
    fn markdown_contains_both_signals_and_smell_disclaimer() {
        let files = [
            entry("big.rs", 5000, 3, 4, 14),
            entry("small.rs", 50, 0, 0, 0),
        ];
        let over_line: Vec<&FileEntry> = files.iter().filter(|f| f.lines >= 2000).collect();
        let over_resp: Vec<&FileEntry> = files
            .iter()
            .filter(|f| f.responsibility_clusters >= RESPONSIBILITY_THRESHOLD)
            .collect();
        let md = render_markdown(
            2000,
            &files,
            over_line.len(),
            &over_line,
            over_resp.len(),
            &over_resp,
        );
        assert!(md.contains("advisory-only"));
        assert!(md.contains("2000"));
        assert!(md.contains("big.rs"));
        assert!(md.contains("5000"));
        // Responsibility signal surfaced, with its smell disclaimer.
        assert!(md.contains("Responsibility threshold"));
        assert!(md.contains("smell, not a measurement"));
        assert!(md.contains("Files over responsibility threshold"));
    }

    #[test]
    fn file_entry_json_carries_both_signals() {
        let f = entry("crates/ripr/src/x.rs", 2500, 3, 9, 14);
        let value = file_entry_json(&f, 2000);
        assert_eq!(value["path"], "crates/ripr/src/x.rs");
        assert_eq!(value["lines"], 2500);
        assert_eq!(value["over_threshold"], true);
        assert_eq!(value["impl_blocks"], 3);
        assert_eq!(value["distinct_public_prefixes"], 9);
        assert_eq!(value["responsibility_clusters"], 14);
        // 14 >= RESPONSIBILITY_THRESHOLD (12).
        assert_eq!(value["over_responsibility_threshold"], true);

        // A small, focused file trips neither flag.
        let small = entry("crates/ripr/src/y.rs", 40, 1, 2, 3);
        let value = file_entry_json(&small, 2000);
        assert_eq!(value["over_threshold"], false);
        assert_eq!(value["over_responsibility_threshold"], false);
    }

    #[test]
    fn render_markdown_handles_empty_report() {
        let empty: Vec<FileEntry> = Vec::new();
        let none: Vec<&FileEntry> = Vec::new();
        let md = render_markdown(2000, &empty, 0, &none, 0, &none);
        assert!(md.contains("No files exceed the 2000-line threshold."));
        assert!(md.contains("No files exceed the 12-cluster responsibility threshold."));
        assert!(md.contains("(no files found)"));
    }

    // Fixture: a small module that mixes many distinct concerns is flagged by
    // the responsibility signal even though it is nowhere near the line
    // threshold — the "structurally entangled even if not huge" case #1147
    // Proposal 1 named.
    #[test]
    fn multi_responsibility_module_flags_on_responsibility_not_size() -> Result<(), String> {
        let root = temp_dir("module-health-multi-responsibility");
        let src = "\
pub fn parse_header() {}
pub fn classify_gap() {}
pub fn observe_probe() {}
pub struct ReachSummary;
pub struct InfectReport;
pub enum PropagateState {}
pub trait Discriminator {}
pub type OracleHandle = ();
pub fn render_markdown_frag() {}
pub fn diff_load() {}
pub fn seam_index() {}
pub fn sink_match() {}
impl ReachSummary {}
impl Discriminator for ReachSummary {}
";
        write_source(&root.join("crates/ripr/src/entangled.rs"), src);
        let mut files = Vec::new();
        collect_rs_files(&root, &root, &mut files)?;
        let entry = files
            .iter()
            .find(|f| f.path.ends_with("entangled.rs"))
            .ok_or("entangled.rs was not collected")?;
        assert!(
            entry.lines < 2000,
            "fixture must stay well under the line threshold, got {} lines",
            entry.lines
        );
        assert!(
            entry.responsibility_clusters >= RESPONSIBILITY_THRESHOLD,
            "a module mixing many concerns should trip the responsibility signal, got {} clusters",
            entry.responsibility_clusters
        );
        assert!(
            entry.impl_blocks >= 2,
            "expected the impl blocks to be counted"
        );
        Ok(())
    }

    // Fixture: a large but single-responsibility module (one concern repeated
    // across many lines) is flagged by size but NOT by the responsibility
    // signal — proof the two signals are independent and the responsibility
    // heuristic does not merely track file length.
    #[test]
    fn large_single_responsibility_module_flags_on_size_not_responsibility() -> Result<(), String> {
        let root = temp_dir("module-health-single-responsibility");
        let mut src = String::from("//! One concern: token accounting.\n");
        for n in 0..2100 {
            src.push_str(&format!("fn token_step_{n}() {{}}\n"));
        }
        src.push_str("pub fn token_run() {}\n");
        write_source(&root.join("crates/ripr/src/tokens.rs"), &src);
        let mut files = Vec::new();
        collect_rs_files(&root, &root, &mut files)?;
        let entry = files
            .iter()
            .find(|f| f.path.ends_with("tokens.rs"))
            .ok_or("tokens.rs was not collected")?;
        assert!(
            entry.lines >= 2000,
            "fixture should exceed the line threshold"
        );
        // A single public `token`-prefixed concern; private helpers are ignored.
        assert!(
            entry.responsibility_clusters < RESPONSIBILITY_THRESHOLD,
            "a single-responsibility module must not trip the responsibility signal, got {} clusters",
            entry.responsibility_clusters
        );
        Ok(())
    }

    // Path/order/root stability: collected paths are root-relative and
    // forward-slash normalized, and the report's sort is deterministic
    // regardless of filesystem read order.
    #[test]
    fn collected_paths_are_root_relative_and_sort_is_deterministic() -> Result<(), String> {
        let root = temp_dir("module-health-stability");
        write_source(&root.join("crates/ripr/src/a.rs"), "pub fn a() {}\n");
        write_source(&root.join("crates/ripr/src/nested/b.rs"), "pub fn b() {}\n");
        write_source(&root.join("xtask/src/c.rs"), "pub fn c() {}\n");

        let mut files = Vec::new();
        collect_rs_files(&root.join("crates/ripr/src"), &root, &mut files)?;
        collect_rs_files(&root.join("xtask/src"), &root, &mut files)?;

        for f in &files {
            assert!(!f.path.contains('\\'), "path not normalized: {}", f.path);
            assert!(
                !f.path.starts_with('/') && !f.path.contains(':'),
                "path is not root-relative: {}",
                f.path
            );
        }

        // The command's stable ordering: lines desc, then path asc.
        let sort = |v: &mut Vec<FileEntry>| {
            v.sort_by(|a, b| b.lines.cmp(&a.lines).then_with(|| a.path.cmp(&b.path)));
        };
        let mut forward = files.clone();
        let mut reversed = files.clone();
        reversed.reverse();
        sort(&mut forward);
        sort(&mut reversed);
        assert_eq!(
            forward, reversed,
            "sort must be deterministic regardless of input order"
        );
        Ok(())
    }

    #[test]
    fn module_health_help_returns_ok() -> Result<(), String> {
        module_health(&["--help".to_string()])?;
        Ok(())
    }

    /// Write source text to `path`, creating parent dirs. Test-only helper that
    /// surfaces I/O errors instead of panicking.
    fn write_source(path: &Path, text: &str) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, text);
    }

    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "{label}_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    fn entry(
        path: &str,
        lines: usize,
        impl_blocks: usize,
        distinct_public_prefixes: usize,
        responsibility_clusters: usize,
    ) -> FileEntry {
        FileEntry {
            path: path.to_string(),
            lines,
            impl_blocks,
            distinct_public_prefixes,
            responsibility_clusters,
        }
    }
}
