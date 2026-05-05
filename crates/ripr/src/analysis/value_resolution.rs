//! Resolve test argument expressions to literal values for
//! `analysis::test_grip_evidence::activate_evidence` (Campaign 5A,
//! `analysis/value-extraction-v2`).
//!
//! Before this module, `scalar_values` rejected every bare identifier
//! at the call site, so a test like:
//!
//! ```ignore
//! let threshold = 100;
//! discounted_total(threshold, threshold);
//! ```
//!
//! produced zero observed values and the seam classified as
//! `activation_unknown`. `ValueEnv` resolves identifiers through a
//! priority chain that stays purely syntactic - no symbol-table, no
//! HIR, no proc-macro expansion.
//!
//! Resolution priority for `analysis/value-extraction-v2`:
//!
//! 1. literal argument (`scalar_values` direct hit)
//! 2. `let IDENT = LITERAL;` in the same test body
//! 3. `#[case(LITERAL, ...)]` rstest parameter at a matching position
//! 4. `for (IDENT, ...) in [(LITERAL, ...), ...] { ... }` table-row binding
//! 5. `const NAME: T = LITERAL;` / `static NAME: T = LITERAL;` in the
//!    same source file
//! 6. `Some(L)` / `Err(L)` constructor unwrap (one level)
//!
//! Builder method values (`.amount(100).threshold(100)`) are handled
//! by a separate scan in `extract_builder_facts`; they don't fit the
//! single-arg resolver shape, and they only count when the method name
//! aligns with seam/owner tokens.
//!
//! All scans strip `//` line comments and string-literal contents
//! before matching, mirroring the comment/string-stripping defense
//! `analysis/related-test-precision-v1` added for `import_path_affinity`.
//! Without that, a comment like `// let threshold = 999;` would
//! shadow the real binding.

use super::rust_index::{FileFacts, RustIndex, TestSummary};
use super::seams::{RepoSeam, RequiredDiscriminator};
use crate::domain::{ValueContext, ValueFact};
use std::collections::BTreeMap;

/// Per-test resolution environment. Built once per (seam, test); each
/// call-arg lookup is a `BTreeMap` lookup plus a small list scan.
pub(crate) struct ValueEnv<'a> {
    seam: &'a RepoSeam,
    /// Test body with comments stripped so binding scans don't pick
    /// up `// let threshold = 999;` shadows.
    body_clean: String,
    /// `IDENT -> LITERAL` from `let IDENT = LITERAL;` lines in the
    /// test body (single-test scope).
    let_bindings: BTreeMap<String, String>,
    /// Each row of `#[case(L, L, ...)]`. `case_param_names` carries
    /// the test fn's parameter names in source order so a positional
    /// IDENT can be looked up across cases.
    rstest_cases: Vec<Vec<String>>,
    case_param_names: Vec<String>,
    /// `IDENT -> [row0_value, row1_value, ...]` from table-driven
    /// `for (IDENT, ...) in [(L, ...), ...]` loops in the test body.
    table_bindings: BTreeMap<String, Vec<String>>,
    /// `NAME -> LITERAL` from `const NAME: T = LITERAL;` and
    /// `static NAME: T = LITERAL;` at the test's source-file top
    /// level (same-file scope).
    module_constants: BTreeMap<String, String>,
}

impl<'a> ValueEnv<'a> {
    pub(crate) fn build(seam: &'a RepoSeam, test: &'a TestSummary, index: &'a RustIndex) -> Self {
        let body_clean = strip_comments_and_strings(&test.body);
        let let_bindings = extract_let_bindings(&body_clean);
        let (rstest_cases, case_param_names) = extract_rstest_cases(test);
        let table_bindings = extract_table_bindings(&body_clean);
        let module_constants = file_facts_for(test, index)
            .map(|facts| extract_module_constants(&facts.source))
            .unwrap_or_default();
        Self {
            seam,
            body_clean,
            let_bindings,
            rstest_cases,
            case_param_names,
            table_bindings,
            module_constants,
        }
    }

    /// Resolve a single owner-call argument to one or more
    /// `(value, ValueContext)` records. Empty vec means "could not
    /// resolve" - caller leaves the arg as opaque (preserves the
    /// existing `activation_unknown` classification semantics).
    pub(crate) fn resolve(&self, arg: &str) -> Vec<(String, ValueContext)> {
        let trimmed = arg.trim().trim_end_matches([',', ';']);
        // 1. Literal argument (delegate to existing scanner upstream
        // - caller still calls scalar_values first; this resolver
        // only handles the cases scalar_values rejects).
        if trimmed.is_empty() {
            return Vec::new();
        }

        // 6. Option/Result constructor unwrap (one level): try inner
        // arg recursively. Catches `Some(threshold)` -> resolve
        // `threshold`. Stays one level deep - no transitive peeling.
        if let Some(inner) = unwrap_option_or_result(trimmed) {
            // Recurse once. The inner can itself be a literal, a let,
            // a const, etc.
            return self.resolve_identifier_or_literal(inner.as_str());
        }

        // Bare identifier: priority 2-5.
        self.resolve_identifier_or_literal(trimmed)
    }

    fn resolve_identifier_or_literal(&self, expr: &str) -> Vec<(String, ValueContext)> {
        // If it parses as a literal, just emit it. Re-uses the upstream
        // scalar_values shape implicitly: integers, floats, strings,
        // chars, simple paths.
        if looks_like_literal(expr) {
            return vec![(expr.to_string(), ValueContext::FunctionArgument)];
        }
        if !is_simple_identifier(expr) {
            return Vec::new();
        }

        // 2. Let binding.
        if let Some(value) = self.let_bindings.get(expr) {
            return vec![(value.clone(), ValueContext::FunctionArgument)];
        }
        // 3. Rstest case (positional).
        if let Some(idx) = self.case_param_names.iter().position(|n| n == expr) {
            let mut out = Vec::new();
            for case in &self.rstest_cases {
                if let Some(value) = case.get(idx) {
                    out.push((value.clone(), ValueContext::TableRow));
                }
            }
            if !out.is_empty() {
                return out;
            }
        }
        // 4. Table-row binding.
        if let Some(values) = self.table_bindings.get(expr) {
            return values
                .iter()
                .map(|v| (v.clone(), ValueContext::TableRow))
                .collect();
        }
        // 5. Same-file const/static.
        if let Some(value) = self.module_constants.get(expr) {
            return vec![(value.clone(), ValueContext::FunctionArgument)];
        }
        Vec::new()
    }

    /// Builder-method facts for the test body. The method name must
    /// align with one of the seam's interesting tokens
    /// (required-discriminator token, expected-sink token, or wrapped
    /// fixture override like `with_amount`) before the value counts.
    /// Without that guard, every `.with_seed(42)` would inflate
    /// observed values for unrelated seams.
    pub(crate) fn builder_facts(&self) -> Vec<ValueFact> {
        let allowed = self.allowed_builder_method_names();
        if allowed.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for cap in scan_builder_calls(&self.body_clean) {
            if !builder_method_matches_allowed(&cap.method, &allowed) {
                continue;
            }
            for value in extract_inner_literals(&cap.arg) {
                out.push(ValueFact {
                    line: cap.line,
                    text: format!(".{}({})", cap.method, cap.arg),
                    value,
                    context: ValueContext::BuilderMethod,
                });
            }
        }
        out
    }

    fn allowed_builder_method_names(&self) -> std::collections::BTreeSet<String> {
        use std::collections::BTreeSet;
        let mut allowed: BTreeSet<String> = BTreeSet::new();
        // Required-discriminator tokens.
        let rd_text = match self.seam.required_discriminator() {
            RequiredDiscriminator::BoundaryValue { description }
            | RequiredDiscriminator::ReturnValue { description } => description.as_str(),
            RequiredDiscriminator::ErrorVariant { variant } => variant.as_str(),
            RequiredDiscriminator::FieldValue { field } => field.as_str(),
            RequiredDiscriminator::Effect { sink } => sink.as_str(),
            RequiredDiscriminator::MatchArmTaken { arm } => arm.as_str(),
            RequiredDiscriminator::CallSite { target } => target.as_str(),
        };
        for token in identifier_tokens(rd_text) {
            allowed.insert(token);
        }
        // Expected-sink tag (e.g., `return_value`, `error_channel`).
        for token in identifier_tokens(self.seam.expected_sink().as_str()) {
            allowed.insert(token);
        }
        allowed
    }
}

/// Look up the test's home-file facts in the index. The test fact
/// stores the original file path; we use it to find the matching
/// FileFacts entry.
fn file_facts_for<'a>(test: &TestSummary, index: &'a RustIndex) -> Option<&'a FileFacts> {
    index.files.get(&test.file)
}

/// `let IDENT = LITERAL;` and `let IDENT: T = LITERAL;` scan. Walks
/// every `let ` token in the cleaned body (comments and string
/// contents already stripped) and parses the binding statement up to
/// the next top-level `;`. Handles multi-statement-per-line bodies
/// (`fn t() { let a = 1; let b = 2; ... }`). Non-literal RHS yields
/// no binding (stays syntactic).
fn extract_let_bindings(body: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let cleaned = strip_comments_and_strings(body);
    for start in find_all(&cleaned, "let ") {
        let after_let = &cleaned[start + 4..];
        // Find the end of this binding statement: the next top-level
        // `;` (depth 0 of paren/bracket/brace).
        let stmt_end = top_level_semicolon(after_let).unwrap_or(after_let.len());
        let stmt = &after_let[..stmt_end];
        // Split into LHS / RHS at the first top-level `=` (avoiding
        // `==` and similar).
        let Some(eq_idx) = first_single_eq(stmt) else {
            continue;
        };
        let (lhs, rhs) = stmt.split_at(eq_idx);
        let rhs = rhs[1..].trim();
        // LHS may have type ascription `IDENT: T`. Take everything
        // before the first `:`.
        let ident_part = lhs.split(':').next().unwrap_or(lhs).trim();
        // Strip optional `mut` keyword.
        let ident = ident_part.strip_prefix("mut ").unwrap_or(ident_part).trim();
        if !is_simple_identifier(ident) {
            continue;
        }
        if !looks_like_literal(rhs) {
            continue;
        }
        out.insert(ident.to_string(), rhs.to_string());
    }
    out
}

/// Position of the first top-level `;` in `text`, or `None` if no
/// such terminator exists. Top-level = depth 0 of `()`/`[]`/`{}`.
fn top_level_semicolon(text: &str) -> Option<usize> {
    let mut depth: i32 = 0;
    for (i, b) in text.bytes().enumerate() {
        match b {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b';' if depth == 0 => return Some(i),
            _ => {}
        }
    }
    None
}

/// Position of the first top-level `=` that is NOT part of `==`,
/// `!=`, `<=`, `>=`. Used to split `IDENT[: T] = RHS` cleanly.
fn first_single_eq(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth: i32 = 0;
    for i in 0..bytes.len() {
        match bytes[i] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b'=' if depth == 0 => {
                let next = bytes.get(i + 1).copied();
                let prev = if i > 0 { Some(bytes[i - 1]) } else { None };
                if next == Some(b'=') {
                    continue;
                }
                if matches!(prev, Some(b'!') | Some(b'<') | Some(b'>')) {
                    continue;
                }
                return Some(i);
            }
            _ => {}
        }
    }
    None
}

/// Find `const NAME: T = LITERAL;` and `static NAME: T = LITERAL;`
/// at the file's top level. Naive line scan; stays inside the
/// "same source file" scope.
fn extract_module_constants(file_source: &str) -> BTreeMap<String, String> {
    let cleaned = strip_comments_and_strings(file_source);
    let mut out = BTreeMap::new();
    for line in cleaned.lines() {
        let trimmed = line.trim();
        let rest = trimmed
            .strip_prefix("pub const ")
            .or_else(|| trimmed.strip_prefix("const "))
            .or_else(|| trimmed.strip_prefix("pub static "))
            .or_else(|| trimmed.strip_prefix("static "))
            .or_else(|| trimmed.strip_prefix("pub(crate) const "))
            .or_else(|| trimmed.strip_prefix("pub(crate) static "));
        let Some(rest) = rest else { continue };
        let rest = rest.trim_end_matches(';').trim();
        let Some(eq_idx) = rest.find('=') else {
            continue;
        };
        if rest.as_bytes().get(eq_idx + 1) == Some(&b'=') {
            continue;
        }
        let (lhs, rhs) = rest.split_at(eq_idx);
        let rhs = rhs[1..].trim();
        let ident = lhs.split(':').next().unwrap_or(lhs).trim();
        let ident = ident.strip_prefix("mut ").unwrap_or(ident);
        if !is_simple_identifier(ident) {
            continue;
        }
        if !looks_like_literal(rhs) {
            continue;
        }
        out.insert(ident.to_string(), rhs.to_string());
    }
    out
}

/// Parse `#[case(L, L, ...)]` attributes captured on the test fn,
/// plus the test fn's parameter names so a positional case literal
/// can be mapped to an identifier. Returns `(cases, param_names)`.
/// Read attrs from `TestFact.attrs` (populated by the parser-backed
/// index path); no filesystem reads.
fn extract_rstest_cases(test: &TestSummary) -> (Vec<Vec<String>>, Vec<String>) {
    let mut cases: Vec<Vec<String>> = Vec::new();
    let mut is_rstest = false;
    for attr in &test.attrs {
        if attr_matches_name_or_call(attr, "rstest") {
            is_rstest = true;
            continue;
        }
        if let Some(args) = attr_call_args(attr, "case") {
            // `#[case]` without args - no scalar values to capture.
            if args.is_empty() {
                continue;
            }
            // Split on top-level commas.
            cases.push(split_top_level(args));
        }
    }
    if !is_rstest && cases.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let params = extract_fn_param_names(&test.body);
    (cases, params)
}

fn attr_matches_name_or_call(attr: &str, name: &str) -> bool {
    let Some(inner) = attr_inner(attr) else {
        return false;
    };
    if inner == name {
        return true;
    }
    attr_call_args(attr, name).is_some()
}

fn attr_call_args<'a>(attr: &'a str, name: &str) -> Option<&'a str> {
    let inner = attr_inner(attr)?;
    let rest = inner.strip_prefix(name)?.trim_start();
    let args = rest.strip_prefix('(')?.strip_suffix(')')?.trim();
    Some(args)
}

fn attr_inner(attr: &str) -> Option<&str> {
    let inner = attr.trim().strip_prefix("#[")?.strip_suffix(']')?.trim();
    Some(inner)
}

/// Pull parameter names out of a `fn name(p1: T, p2: T, ...)` header.
/// Test bodies start at the `fn` keyword, so the parameter list is
/// always present on the first non-attr line. Best-effort: skip
/// `&self` / `self` and reject anything not identifier-shaped.
fn extract_fn_param_names(body: &str) -> Vec<String> {
    let Some(open) = body.find('(') else {
        return Vec::new();
    };
    let after = &body[open + 1..];
    let Some(close) = after.find(')') else {
        return Vec::new();
    };
    let raw = &after[..close];
    let mut out = Vec::new();
    for part in split_top_level(raw) {
        let part = part.trim();
        if part.is_empty() || part == "self" || part.starts_with('&') {
            continue;
        }
        let ident = part.split(':').next().unwrap_or(part).trim();
        let ident = ident.strip_prefix("mut ").unwrap_or(ident).trim();
        if is_simple_identifier(ident) {
            out.push(ident.to_string());
        }
    }
    out
}

/// `for (a, b) in [(L, L), ...] { ... }` and
/// `for &(a, b) in &[(L, L), ...] { ... }` shapes. Each named
/// destructure component maps to the column of literals across the
/// table rows (literal tuple tables only, no macros, no runtime-built
/// vectors).
fn extract_table_bindings(body: &str) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for cap in scan_for_table_loops(body) {
        for (col, idents) in cap.idents.iter().enumerate() {
            for row in &cap.rows {
                if let Some(value) = row.get(col)
                    && let Some(ident) = idents
                    && is_simple_identifier(ident)
                {
                    out.entry(ident.clone()).or_default().push(value.clone());
                }
            }
        }
    }
    out
}

struct TableLoopCapture {
    /// Names per column. `None` when the destructure component is
    /// `_` or otherwise not a simple identifier.
    idents: Vec<Option<String>>,
    /// Each row's column values.
    rows: Vec<Vec<String>>,
}

/// Find every `for PATTERN in [...]` shape with literal-tuple rows.
/// Best-effort syntactic scan - does not handle macro tables, fn
/// calls returning Vec, or anything beyond inline literal arrays.
fn scan_for_table_loops(body: &str) -> Vec<TableLoopCapture> {
    let mut out = Vec::new();
    for line_start in find_all(body, "for ") {
        let after_for = &body[line_start + 4..];
        let Some(in_idx) = after_for.find(" in ") else {
            continue;
        };
        let pattern = after_for[..in_idx].trim();
        let after_in = after_for[in_idx + 4..].trim_start();
        // Allow optional leading `&` or `&[`.
        let after_in = after_in.strip_prefix('&').unwrap_or(after_in);
        let after_in = after_in.trim_start();
        // Pattern must be a tuple destructure: `(a, b, c)` or
        // `&(a, b, c)`.
        let pattern = pattern.strip_prefix('&').unwrap_or(pattern).trim();
        let Some(pattern_inner) = pattern.strip_prefix('(').and_then(|s| s.strip_suffix(')'))
        else {
            continue;
        };
        let idents: Vec<Option<String>> = split_top_level(pattern_inner)
            .into_iter()
            .map(|p| {
                let p = p.trim();
                if p == "_" || p.is_empty() {
                    None
                } else if is_simple_identifier(p) {
                    Some(p.to_string())
                } else {
                    None
                }
            })
            .collect();
        if idents.is_empty() || idents.iter().all(|i| i.is_none()) {
            continue;
        }
        // RHS must start with `[` (array of tuples).
        let Some(arr_inner) = balanced_bracket_contents(after_in, '[', ']') else {
            continue;
        };
        let mut rows: Vec<Vec<String>> = Vec::new();
        for row_text in split_top_level_at_brackets(arr_inner) {
            let row_text = row_text.trim();
            // Each row should be `(L, L, ...)`.
            let Some(row_inner) = row_text.strip_prefix('(').and_then(|s| s.strip_suffix(')'))
            else {
                continue;
            };
            let parts = split_top_level(row_inner);
            if parts.len() != idents.len() {
                continue;
            }
            if !parts.iter().all(|p| looks_like_literal(p.trim())) {
                continue;
            }
            rows.push(parts.into_iter().map(|p| p.trim().to_string()).collect());
        }
        if !rows.is_empty() {
            out.push(TableLoopCapture { idents, rows });
        }
    }
    out
}

struct BuilderCallCapture {
    method: String,
    arg: String,
    line: usize,
}

/// Find every `.method_name(LITERAL_OR_EXPR)` segment in the test
/// body. The caller filters by name alignment before counting the
/// value as observed.
fn scan_builder_calls(body: &str) -> Vec<BuilderCallCapture> {
    let mut out = Vec::new();
    let bytes = body.as_bytes();
    let mut line: usize = 1;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\n' {
            line += 1;
            i += 1;
            continue;
        }
        if bytes[i] != b'.' {
            i += 1;
            continue;
        }
        // After `.`: identifier, then `(`.
        let name_start = i + 1;
        let mut name_end = name_start;
        while name_end < bytes.len()
            && (bytes[name_end].is_ascii_alphanumeric() || bytes[name_end] == b'_')
        {
            name_end += 1;
        }
        if name_end == name_start || name_end >= bytes.len() || bytes[name_end] != b'(' {
            i += 1;
            continue;
        }
        let method = &body[name_start..name_end];
        // Find matching `)`.
        let arg_start = name_end + 1;
        let mut depth: i32 = 1;
        let mut j = arg_start;
        while j < bytes.len() && depth > 0 {
            match bytes[j] {
                b'(' => depth += 1,
                b')' => depth -= 1,
                _ => {}
            }
            if depth == 0 {
                break;
            }
            j += 1;
        }
        if depth != 0 {
            break;
        }
        let arg = &body[arg_start..j];
        out.push(BuilderCallCapture {
            method: method.to_string(),
            arg: arg.to_string(),
            line,
        });
        i = j + 1;
    }
    out
}

/// Pull literals out of a builder arg expression. A bare literal
/// returns `[itself]`; nested expressions yield empty.
fn extract_inner_literals(arg: &str) -> Vec<String> {
    let trimmed = arg.trim().trim_end_matches([',', ';']);
    if looks_like_literal(trimmed) {
        return vec![trimmed.to_string()];
    }
    Vec::new()
}

/// Strip `Some(x)` / `Ok(x)` / `Err(x)` to the inner expression.
/// Returns the inner text (trimmed). One level only.
fn unwrap_option_or_result(text: &str) -> Option<String> {
    for ctor in ["Some(", "Ok(", "Err("] {
        if let Some(rest) = text.strip_prefix(ctor)
            && let Some(inner) = rest.strip_suffix(')')
        {
            return Some(inner.trim().to_string());
        }
    }
    None
}

fn looks_like_literal(expr: &str) -> bool {
    let trimmed = expr.trim().trim_end_matches([',', ';']);
    if trimmed.is_empty() {
        return false;
    }
    // String / char literal.
    if trimmed.starts_with('"') || trimmed.starts_with('\'') {
        return true;
    }
    // Numeric literal (with optional negative sign and `_`).
    let body = trimmed.strip_prefix('-').unwrap_or(trimmed);
    if !body.is_empty()
        && body.chars().next().is_some_and(|c| c.is_ascii_digit())
        && body
            .chars()
            .all(|c| c.is_ascii_digit() || c == '_' || c == '.')
    {
        return true;
    }
    // bool, None - emit as their token text.
    if matches!(trimmed, "true" | "false" | "None") {
        return true;
    }
    // Path-shaped enum literal, e.g. `Color::Red` or
    // `MyError::ParseError`. Same shape `scalar_values` already accepts.
    if trimmed.contains("::")
        && trimmed
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == ':')
    {
        return true;
    }
    false
}

fn is_simple_identifier(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && text.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Split text on top-level commas (depth 0 of `()`/`[]`/`{}`).
fn split_top_level(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth: i32 = 0;
    let mut start = 0;
    let bytes = text.as_bytes();
    for i in 0..bytes.len() {
        match bytes[i] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b',' if depth == 0 => {
                out.push(text[start..i].to_string());
                start = i + 1;
            }
            _ => {}
        }
    }
    if start <= text.len() {
        let tail = text[start..].trim();
        if !tail.is_empty() {
            out.push(tail.to_string());
        }
    }
    out
}

/// Split a bracket-delimited table on top-level row commas (depth 0
/// outside the surrounding `[`/`]`). Tuples nest brackets so we count
/// `(`/`)` as depth too.
fn split_top_level_at_brackets(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth: i32 = 0;
    let mut start = 0;
    let bytes = text.as_bytes();
    for i in 0..bytes.len() {
        match bytes[i] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b',' if depth == 0 => {
                out.push(&text[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    if start <= text.len() {
        out.push(&text[start..]);
    }
    out
}

/// Find the contents inside the next balanced `open`...`close` pair
/// in `text`, starting from offset 0. Returns `None` if no balanced
/// pair exists.
fn balanced_bracket_contents(text: &str, open: char, close: char) -> Option<&str> {
    let bytes = text.as_bytes();
    let open_b = open as u8;
    let close_b = close as u8;
    let start = bytes.iter().position(|&b| b == open_b)?;
    let mut depth: i32 = 0;
    for i in start..bytes.len() {
        if bytes[i] == open_b {
            depth += 1;
        } else if bytes[i] == close_b {
            depth -= 1;
            if depth == 0 {
                return Some(&text[start + 1..i]);
            }
        }
    }
    None
}

fn find_all(haystack: &str, needle: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let mut start = 0;
    while let Some(idx) = haystack[start..].find(needle) {
        let abs = start + idx;
        // Word-boundary check: previous char must not be alpha/_
        // (avoids `before_for` matching `for `).
        let ok = abs == 0
            || haystack
                .as_bytes()
                .get(abs - 1)
                .is_some_and(|b| !(b.is_ascii_alphanumeric() || *b == b'_'));
        if ok {
            out.push(abs);
        }
        start = abs + 1;
    }
    out
}

fn identifier_tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            current.push(ch);
        } else {
            if !current.is_empty() && current.len() > 2 {
                out.push(current.clone());
            }
            current.clear();
        }
    }
    if !current.is_empty() && current.len() > 2 {
        out.push(current);
    }
    out
}

fn builder_method_matches_allowed(
    method: &str,
    allowed: &std::collections::BTreeSet<String>,
) -> bool {
    allowed.iter().any(|token| {
        method == token
            || method.strip_prefix("with_") == Some(token.as_str())
            || method.strip_prefix("set_") == Some(token.as_str())
            || method
                .strip_suffix(token.as_str())
                .is_some_and(|prefix| prefix.ends_with('_'))
            || method
                .strip_prefix(token.as_str())
                .is_some_and(|suffix| suffix.starts_with('_'))
    })
}

/// Drop `//` line-comment tails and replace string-literal contents
/// with empty text, so binding scans don't pick up `// let x = 1;`
/// or string-embedded names. Mirrors the helper added in
/// `analysis/related-test-precision-v1` for `import_path_affinity`.
fn strip_comments_and_strings(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    for raw_line in source.lines() {
        let without_comment = match raw_line.find("//") {
            Some(idx) => &raw_line[..idx],
            None => raw_line,
        };
        let mut in_string = false;
        let mut escaped = false;
        for ch in without_comment.chars() {
            if in_string {
                if escaped {
                    escaped = false;
                    continue;
                }
                match ch {
                    '\\' => escaped = true,
                    '"' => {
                        in_string = false;
                        out.push('"');
                    }
                    _ => {}
                }
                continue;
            }
            if ch == '"' {
                in_string = true;
                out.push('"');
                continue;
            }
            out.push(ch);
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::seams::{ExpectedSink, RepoSeam, SeamKind};

    fn predicate_seam() -> RepoSeam {
        RepoSeam::new(
            "src/pricing.rs",
            "pricing::discounted_total",
            SeamKind::PredicateBoundary,
            0,
            1,
            "amount >= discount_threshold",
            RequiredDiscriminator::BoundaryValue {
                description: "amount >= discount_threshold".to_string(),
            },
            ExpectedSink::ReturnValue,
        )
    }

    #[test]
    fn extract_let_bindings_picks_up_literal_rhs_and_skips_expressions() {
        let body = "let a = 100;\nlet b: i32 = 200;\nlet mut c = 300;\nlet d = a + 1;\n";
        let bindings = extract_let_bindings(body);
        assert_eq!(bindings.get("a").map(String::as_str), Some("100"));
        assert_eq!(bindings.get("b").map(String::as_str), Some("200"));
        assert_eq!(bindings.get("c").map(String::as_str), Some("300"));
        assert!(!bindings.contains_key("d"), "non-literal RHS must not bind");
    }

    #[test]
    fn extract_module_constants_finds_const_and_static_top_level() {
        let source = "pub const A: i32 = 1;\nstatic B: i32 = 2;\n\
                      pub(crate) const C: i32 = 3;\n";
        let consts = extract_module_constants(source);
        assert_eq!(consts.get("A").map(String::as_str), Some("1"));
        assert_eq!(consts.get("B").map(String::as_str), Some("2"));
        assert_eq!(consts.get("C").map(String::as_str), Some("3"));
    }

    #[test]
    fn looks_like_literal_accepts_numbers_strings_bools_paths_and_rejects_others() {
        for ok in [
            "100",
            "-5",
            "1_000",
            "1.5",
            "\"hi\"",
            "true",
            "false",
            "None",
            "Color::Red",
            "MyError::ParseError",
        ] {
            assert!(looks_like_literal(ok), "{ok} should look like a literal");
        }
        for bad in ["amount", "make_quote()", "x + 1"] {
            assert!(
                !looks_like_literal(bad),
                "{bad} must not look like a literal"
            );
        }
    }

    #[test]
    fn unwrap_option_or_result_peels_one_level_only() {
        assert_eq!(unwrap_option_or_result("Some(100)").as_deref(), Some("100"));
        assert_eq!(unwrap_option_or_result("Ok(42)").as_deref(), Some("42"));
        assert_eq!(
            unwrap_option_or_result("Err(MyError::A)").as_deref(),
            Some("MyError::A")
        );
        assert_eq!(unwrap_option_or_result("100"), None);
    }

    #[test]
    fn resolve_option_result_constructor_keeps_unresolved_inner_opaque() {
        let seam = predicate_seam();
        let env = ValueEnv {
            seam: &seam,
            body_clean: String::new(),
            let_bindings: BTreeMap::new(),
            rstest_cases: Vec::new(),
            case_param_names: Vec::new(),
            table_bindings: BTreeMap::new(),
            module_constants: BTreeMap::new(),
        };
        assert!(
            env.resolve("Some(make_amount())").is_empty(),
            "opaque constructor payloads must not become observed values"
        );
    }

    #[test]
    fn extract_rstest_cases_preserves_string_literal_whitespace() {
        let test = TestSummary {
            name: "t".to_string(),
            file: std::path::PathBuf::from("tests/x.rs"),
            start_line: 1,
            end_line: 1,
            body: "fn t(input: &str) { check(input); }".to_string(),
            calls: Vec::new(),
            assertions: Vec::new(),
            literals: Vec::new(),
            attrs: vec!["#[rstest]".to_string(), "#[case(\"a b\")]".to_string()],
        };
        let (cases, params) = extract_rstest_cases(&test);
        assert_eq!(params, vec!["input"]);
        assert_eq!(cases, vec![vec!["\"a b\"".to_string()]]);
    }

    #[test]
    fn strip_comments_and_strings_removes_line_comments_and_string_contents() {
        let input = "let x = 1; // let x = 999;\nlet s = \"shadow = 0\";\n";
        let cleaned = strip_comments_and_strings(input);
        assert!(
            !cleaned.contains("999"),
            "comment-shadowed value must be stripped"
        );
        assert!(
            !cleaned.contains("shadow = 0"),
            "string-shadowed value must be stripped"
        );
    }

    #[test]
    fn scan_for_table_loops_extracts_named_columns() {
        let body = "for (a, b, c) in [(1, 2, 3), (4, 5, 6)] { let _ = (a, b, c); }\n";
        let captures = scan_for_table_loops(body);
        assert_eq!(captures.len(), 1);
        let cap = &captures[0];
        assert_eq!(cap.idents.len(), 3);
        assert_eq!(cap.rows.len(), 2);
        assert_eq!(cap.rows[0], vec!["1", "2", "3"]);
        assert_eq!(cap.rows[1], vec!["4", "5", "6"]);
    }

    #[test]
    fn scan_builder_calls_finds_method_chain_arguments() {
        let body = "let q = Quote::new().amount(100).threshold(200).build();\n";
        let calls = scan_builder_calls(body);
        let methods: Vec<&str> = calls.iter().map(|c| c.method.as_str()).collect();
        assert!(methods.contains(&"amount"));
        assert!(methods.contains(&"threshold"));
        assert!(methods.contains(&"build"));
    }

    #[test]
    fn builder_method_match_accepts_fixture_override_prefixes_and_rejects_unrelated_methods() {
        let allowed: std::collections::BTreeSet<String> = ["amount", "threshold"]
            .into_iter()
            .map(str::to_string)
            .collect();
        assert!(builder_method_matches_allowed("amount", &allowed));
        assert!(builder_method_matches_allowed("with_amount", &allowed));
        assert!(builder_method_matches_allowed("set_threshold", &allowed));
        assert!(builder_method_matches_allowed("amount_cents", &allowed));
        assert!(!builder_method_matches_allowed("with_seed", &allowed));
        assert!(!builder_method_matches_allowed("discount", &allowed));
    }

    #[test]
    fn allowed_builder_method_names_includes_required_discriminator_tokens() {
        // Build a minimal env; we only need the seam for this assertion.
        let seam = predicate_seam();
        let test = TestSummary {
            name: "t".to_string(),
            file: std::path::PathBuf::from("tests/x.rs"),
            start_line: 1,
            end_line: 1,
            body: String::new(),
            calls: Vec::new(),
            assertions: Vec::new(),
            literals: Vec::new(),
            attrs: Vec::new(),
        };
        let env = ValueEnv {
            seam: &seam,
            body_clean: String::new(),
            let_bindings: BTreeMap::new(),
            rstest_cases: Vec::new(),
            case_param_names: Vec::new(),
            table_bindings: BTreeMap::new(),
            module_constants: BTreeMap::new(),
        };
        // Suppress dead-code warnings by referencing the param.
        let _ = &test;
        let allowed = env.allowed_builder_method_names();
        assert!(allowed.contains("amount"));
        assert!(allowed.contains("discount_threshold"));
    }
}
