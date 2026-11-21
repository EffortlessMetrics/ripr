use super::PythonOwner;
use super::related_tests::{
    PythonRelatedCandidate, line_prefix_looks_like_comment_or_string, strongest_assertion,
};
use super::source_facts::parse_module_result;
use super::static_limits::is_simple_python_identifier;
use crate::domain::{OracleStrength, OwnerKind};
use rustpython_parser::ast::{Expr, Mod, Ranged, Stmt};
use std::path::Path;
/// A changed line that carries no runtime behavior, so there is no behavior delta
/// for a test to discriminate: a blank line, a `#` comment, or a bare
/// string-literal expression statement (a docstring or standalone string). Such a
/// change is a no-op / equivalent mutant — `ripr` must not emit a behavior probe
/// for it, because crediting `exposed` would imply the tests discriminate a
/// behavior change that does not exist (#1279).
///
/// Conservative by construction: only blank/comment lines and lines that are
/// ENTIRELY a single non-f-string literal qualify. f-strings are excluded (a bare
/// f-string statement can evaluate embedded calls), multi-line docstring interiors
/// are handled from AST-backed source context in [`classify_change_with_context`],
/// and annotation-only changes are handled by the dedicated
/// `is_annotation_only_*_change` guards in
/// `classify_change_with_old` (def headers via #1294; module-scope bare variables
/// via #1289 — class-body variable annotations remain out of scope because
/// `@dataclass`/Pydantic make them runtime-meaningful).
pub(super) fn is_python_no_behavior_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty() || trimmed.starts_with('#') || is_bare_string_literal_statement(trimmed)
}

/// Whether `trimmed` (already whitespace-trimmed) is exactly one Python string
/// literal with nothing of significance after it — a docstring or standalone
/// string expression statement. An `f`/`F` prefix is rejected because a bare
/// f-string can have side effects through embedded expressions; an identity-bearing
/// prefix is only recognized when it is immediately followed by a quote (an
/// assignment like `result = "x"` has a separating space and is never matched).
fn is_bare_string_literal_statement(trimmed: &str) -> bool {
    let bytes = trimmed.as_bytes();
    let mut idx = 0;
    // Optional string prefix (at most two letters, e.g. `r`, `b`, `rb`, `br`, `u`).
    // `f`/`F` is deliberately absent so formatted strings fall through to `false`.
    while idx < 2
        && idx < bytes.len()
        && matches!(bytes[idx], b'r' | b'R' | b'b' | b'B' | b'u' | b'U')
    {
        idx += 1;
    }
    let rest = &trimmed[idx..];
    let rest_bytes = rest.as_bytes();
    let quote = match rest_bytes.first() {
        Some(&b'"') => b'"',
        Some(&b'\'') => b'\'',
        _ => return false,
    };
    let triple = rest_bytes.len() >= 3 && rest_bytes[1] == quote && rest_bytes[2] == quote;
    if triple {
        let body = &rest[3..];
        let close = [quote as char, quote as char, quote as char]
            .iter()
            .collect::<String>();
        match body.find(&close) {
            Some(pos) => body[pos + 3..].trim().is_empty(),
            None => false,
        }
    } else {
        let mut escaped = false;
        for (offset, ch) in rest.char_indices().skip(1) {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch as u32 == u32::from(quote) {
                return rest[offset + 1..].trim().is_empty();
            }
        }
        false
    }
}

/// The runtime-significant skeleton of a `def` header, used to decide whether a
/// change touches ONLY annotations (#1289). It deliberately EXCLUDES every
/// annotation (parameter and return) and INCLUDES everything that affects runtime
/// dispatch: async-ness, function name, ordered parameter names, default-value
/// source text, the positional-only / keyword-only group sizes, and the
/// `*args`/`**kwargs` names. Two headers with equal skeletons differ only in
/// annotations.
type DefSignatureSkeleton = (
    bool,                          // is_async
    String,                        // function name
    usize,                         // positional-only count
    usize,                         // keyword-only count
    Vec<(String, Option<String>)>, // ordered (param name, default-value source)
    Option<String>,                // *args name
    Option<String>,                // **kwargs name
);

fn def_signature_skeleton(line: &str) -> Option<DefSignatureSkeleton> {
    let trimmed = line.trim_start();
    if !(trimmed.starts_with("def ") || trimmed.starts_with("async def ")) {
        return None;
    }
    // Synthesize a parseable statement: a `def` header alone is not a module.
    let snippet = format!("{trimmed}\n    pass\n");
    let Ok(Mod::Module(module)) =
        parse_module_result(Path::new("annotation_only_probe.py"), &snippet)
    else {
        return None;
    };
    let (is_async, name, args) = module.body.iter().find_map(|stmt| match stmt {
        Stmt::FunctionDef(f) => Some((false, f.name.to_string(), &f.args)),
        Stmt::AsyncFunctionDef(f) => Some((true, f.name.to_string(), &f.args)),
        _ => None,
    })?;
    let slice = |expr: &Expr| -> String {
        let range = expr.range();
        snippet
            .get(usize::from(range.start())..usize::from(range.end()))
            .unwrap_or_default()
            .to_string()
    };
    let mut params: Vec<(String, Option<String>)> = Vec::new();
    for arg in args
        .posonlyargs
        .iter()
        .chain(args.args.iter())
        .chain(args.kwonlyargs.iter())
    {
        let default = arg.default.as_ref().map(|expr| slice(expr));
        params.push((arg.def.arg.to_string(), default));
    }
    Some((
        is_async,
        name,
        args.posonlyargs.len(),
        args.kwonlyargs.len(),
        params,
        args.vararg.as_ref().map(|arg| arg.arg.to_string()),
        args.kwarg.as_ref().map(|arg| arg.arg.to_string()),
    ))
}

/// Whether the `def`-header change modifies ONLY type annotations, leaving the
/// callable's runtime signature unchanged. Python does not enforce annotations at
/// runtime, so such a change has no behavior delta (#1289). Fails closed: returns
/// false when either line is not a parseable `def` header, when the lines are
/// identical, or when anything beyond an annotation differs (e.g. a default-value
/// change, an added/removed/renamed/reordered parameter, a `/`/`*` marker move, or
/// an async-ness change).
pub(super) fn is_annotation_only_def_change(old_line: &str, new_line: &str) -> bool {
    if old_line.trim() == new_line.trim() {
        return false;
    }
    match (
        def_signature_skeleton(old_line),
        def_signature_skeleton(new_line),
    ) {
        (Some(old), Some(new)) => old == new,
        _ => false,
    }
}

/// The runtime-significant skeleton of a bare variable annotation line
/// (`x: int = 5` or `x: int`), used to decide whether a change touches ONLY the
/// annotation (#1289). Includes the target name and the optional value source
/// text; EXCLUDES the annotation. Two lines with equal skeletons differ only in
/// annotation, so a value/target change is NOT annotation-only. A simple-name
/// target only (`x`, not `obj.attr`); attribute annotations live inside class
/// bodies, which this suppression does not reach (it is module-scope only).
type VariableAnnotationSkeleton = (String, Option<String>); // (target name, value source)

fn variable_annotation_skeleton(line: &str) -> Option<VariableAnnotationSkeleton> {
    let trimmed = line.trim();
    // Cheap reject: must contain a `:` before any `=` (or no `=` at all) and
    // start with an identifier char. This avoids parsing plain assignments.
    let name_end = trimmed.find(':').filter(|&idx| {
        trimmed[..idx]
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_')
    })?;
    if name_end == 0 {
        return None;
    }
    // Synthesize a parseable statement: an annotated assignment is a module body.
    let snippet = format!("{trimmed}\n");
    let Ok(Mod::Module(module)) =
        parse_module_result(Path::new("annotation_only_probe.py"), &snippet)
    else {
        return None;
    };
    let stmt = module.body.first()?;
    let Stmt::AnnAssign(assign) = stmt else {
        return None;
    };
    // Simple-name target only; an attribute target (`obj.attr: int`) is not a
    // bare module-scope variable and is left to classify normally.
    let Expr::Name(target) = &*assign.target else {
        return None;
    };
    let slice = |expr: &Expr| -> String {
        let range = expr.range();
        snippet
            .get(usize::from(range.start())..usize::from(range.end()))
            .unwrap_or_default()
            .to_string()
    };
    let value = assign.value.as_deref().map(slice);
    Some((target.id.to_string(), value))
}

/// Whether a bare variable annotation change modifies ONLY the annotation,
/// leaving the runtime binding (target name and value) unchanged. Python does
/// not enforce annotations at runtime at module scope, so such a change has no
/// behavior delta (#1289). Fails closed: returns false when either line is not
/// a parseable bare-variable annotation, when the lines are identical, or when
/// anything beyond the annotation differs (a value change, a target rename, an
/// added/removed value, or an attribute target).
pub(super) fn is_annotation_only_var_change(old_line: &str, new_line: &str) -> bool {
    if old_line.trim() == new_line.trim() {
        return false;
    }
    match (
        variable_annotation_skeleton(old_line),
        variable_annotation_skeleton(new_line),
    ) {
        (Some(old), Some(new)) => old == new,
        _ => false,
    }
}

/// A parameter whose default VALUE changed in a `def`-header diff, with the
/// position metadata needed to decide whether a call binds it.
pub(super) struct ChangedDefaultParam {
    pub(super) name: String,
    /// 0-based index in the full ordered parameter list (posonly ++ args ++ kwonly).
    pub(super) index: usize,
    /// Whether a positional argument at `index` can bind this parameter. False for
    /// a keyword-only parameter, which a positional argument can never reach.
    pub(super) positionally_bindable: bool,
}

/// The parameters whose default VALUE changed between two `def` headers, when the
/// change is a PURE default-value change (value -> different value) and nothing
/// else about the runtime signature differs. Returns None — leaving classification
/// untouched — for a non-`def` line, an added/removed default (which changes
/// requiredness, not just a value), a renamed/reordered/added parameter, an
/// async-ness change, or a `*args`/`**kwargs` change, or when no default value
/// actually changed. Fails closed: anything it cannot prove is a pure
/// default-value change yields None.
pub(super) fn changed_default_value_params(
    old_line: &str,
    new_line: &str,
) -> Option<Vec<ChangedDefaultParam>> {
    let (old_async, old_name, old_pos, old_kw, old_params, old_va, old_kwa) =
        def_signature_skeleton(old_line)?;
    let (new_async, new_name, new_pos, new_kw, new_params, new_va, new_kwa) =
        def_signature_skeleton(new_line)?;
    if old_async != new_async
        || old_name != new_name
        || old_pos != new_pos
        || old_kw != new_kw
        || old_va != new_va
        || old_kwa != new_kwa
        || old_params.len() != new_params.len()
    {
        return None;
    }
    let positional_capacity = new_params.len().saturating_sub(new_kw);
    let mut changed = Vec::new();
    for (index, (old_param, new_param)) in old_params.iter().zip(new_params.iter()).enumerate() {
        if old_param.0 != new_param.0 {
            return None; // renamed / reordered parameter
        }
        match (&old_param.1, &new_param.1) {
            (Some(old_default), Some(new_default)) if old_default != new_default => {
                changed.push(ChangedDefaultParam {
                    name: new_param.0.clone(),
                    index,
                    positionally_bindable: index < positional_capacity,
                });
            }
            (Some(_), Some(_)) | (None, None) => {}
            // Added or removed default changes requiredness, not just a value.
            (Some(_), None) | (None, Some(_)) => return None,
        }
    }
    (!changed.is_empty()).then_some(changed)
}

/// The argument shape of a single call: how many positional arguments precede any
/// keyword arguments, and the set of keyword-argument names.
pub(super) struct CallArgShape {
    pub(super) positional_count: usize,
    pub(super) keywords: Vec<String>,
}

impl CallArgShape {
    fn binds(&self, param: &ChangedDefaultParam) -> bool {
        if self.keywords.iter().any(|name| name == &param.name) {
            return true;
        }
        param.positionally_bindable && param.index < self.positional_count
    }
}

/// Splits a call's argument-list text into top-level argument segments, respecting
/// quotes and nested brackets: `a, g(b, c), d=1` -> `["a", " g(b, c)", " d=1"]`.
fn split_top_level_args(args: &str) -> Vec<&str> {
    let mut segments = Vec::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut depth = 0usize;
    let mut start = 0usize;
    for (idx, ch) in args.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if let Some(active) = quote {
            if ch == active {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                segments.push(&args[start..idx]);
                start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }
    segments.push(&args[start..]);
    segments
}

/// The keyword-argument name of a single call-argument segment (`rate=0.2` ->
/// `Some("rate")`), or None when the segment is positional. Guards against
/// comparison operators (`x == 1`, `a != b`, `n <= 3`) so a positional boolean
/// expression is not misread as a keyword binding.
fn call_segment_keyword_name(segment: &str) -> Option<&str> {
    let chars: Vec<(usize, char)> = segment.char_indices().collect();
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut depth = 0usize;
    for (position, (idx, ch)) in chars.iter().copied().enumerate() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if let Some(active) = quote {
            if ch == active {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            '=' if depth == 0 => {
                let prev = position.checked_sub(1).map(|p| chars[p].1);
                let next = chars.get(position + 1).map(|(_, c)| *c);
                if matches!(prev, Some('=' | '!' | '<' | '>')) || next == Some('=') {
                    continue; // part of ==, !=, <=, >=
                }
                let field = segment[..idx].trim();
                return is_simple_python_identifier(field).then_some(field);
            }
            _ => {}
        }
    }
    None
}

/// Parses a call's argument-list text into a positional/keyword shape. Returns
/// None for any shape this conservative parser cannot fully account for — an
/// `*args` / `**kwargs` unpacking — so the caller fails open (keeps the existing
/// classification) rather than guessing a binding.
pub(super) fn analyze_call_args(args: &str) -> Option<CallArgShape> {
    let mut positional_count = 0usize;
    let mut keywords = Vec::new();
    for segment in split_top_level_args(args) {
        let trimmed = segment.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('*') {
            return None; // *args / **kwargs unpack: binding is undecidable
        }
        // A `#` inside an argument segment is an inline comment (legal in a
        // multi-line call). A comment can hide a `)` that `split_top_level_args`
        // already mis-counted, or carry text that inflates `positional_count`,
        // so the binding is ambiguous — fail open rather than risk a false-clean.
        if trimmed.contains('#') {
            return None;
        }
        match call_segment_keyword_name(trimmed) {
            Some(name) => keywords.push(name.to_string()),
            None => positional_count += 1,
        }
    }
    Some(CallArgShape {
        positional_count,
        keywords,
    })
}

/// The byte index of the `)` that closes the `(` at `open_idx`, respecting quotes
/// and nesting. None if unbalanced.
fn matching_call_paren(text: &str, open_idx: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (offset, ch) in text[open_idx..].char_indices() {
        let idx = open_idx + offset;
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if let Some(active) = quote {
            if ch == active {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            '(' | '[' | '{' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(idx);
                }
            }
            ']' | '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    None
}

/// The argument-list text of every direct free-function call to `name` in `body`
/// (`render(...)` but not `obj.render(...)` and not `renderer(...)`). Balanced-paren
/// aware; skips calls whose parentheses are unbalanced in the captured body text.
pub(super) fn free_function_call_arglists<'a>(body: &'a str, name: &str) -> Vec<&'a str> {
    let mut arglists = Vec::new();
    if name.is_empty() {
        return arglists;
    }
    let mut search_from = 0usize;
    while let Some(rel) = body[search_from..].find(name) {
        let name_start = search_from + rel;
        let name_end = name_start + name.len();
        search_from = name_end;
        // Word boundary before the name: not part of a longer identifier, and not a
        // method/attribute access (`obj.render`).
        if let Some(prev) = body[..name_start].chars().next_back()
            && (prev == '_' || prev == '.' || prev.is_alphanumeric())
        {
            continue;
        }
        // The match must be live code, not a mention inside a comment or a
        // string literal. A `# comment` or an unclosed quote on the same line
        // before the name means this occurrence is not an executable call; a
        // comment containing `)` would otherwise break `matching_call_paren` and
        // a string mention would invent a call that does not run.
        if line_prefix_looks_like_comment_or_string(body, name_start) {
            continue;
        }
        let rest = &body[name_end..];
        // Not part of a longer identifier after the name (`renderer`).
        if let Some(next) = rest.chars().next()
            && (next == '_' || next.is_alphanumeric())
        {
            continue;
        }
        let trimmed = rest.trim_start();
        if !trimmed.starts_with('(') {
            continue;
        }
        let open_idx = name_end + (rest.len() - trimmed.len());
        let Some(close_idx) = matching_call_paren(body, open_idx) else {
            continue;
        };
        arglists.push(&body[open_idx + 1..close_idx]);
        search_from = close_idx + 1;
    }
    arglists
}

/// Whether a changed default VALUE in a `def` header is left UN-exercised by every
/// strong related oracle. When the change is a pure default-value change and every
/// strong related test that calls the owner binds the changed parameter(s)
/// explicitly (keyword or positional), the changed default is never reached, so a
/// strong observing oracle cannot discriminate it (#1289 trap 45) — returns
/// Some(changed-param names) naming what to exercise by omission. Returns None (no
/// block) when the change is not a pure default-value change, when no owner call
/// can be analyzed, or when at least one strong call omits a changed parameter.
/// Fails open: any untracked shape yields None so a genuine exposure is never
/// suppressed. Scoped to free-function owners — a method/classmethod has an
/// implicit `self`/`cls` that shifts positional binding, so those fail open.
pub(super) fn changed_default_overridden_params(
    old_line_text: Option<&str>,
    new_line_text: &str,
    owner: &PythonOwner,
    related_candidates: &[PythonRelatedCandidate<'_>],
) -> Option<Vec<String>> {
    let old_line = old_line_text?;
    if matches!(
        owner.owner_kind,
        Some(OwnerKind::Method | OwnerKind::ClassMethod)
    ) {
        return None;
    }
    let changed = changed_default_value_params(old_line, new_line_text)?;
    let mut saw_strong = false;
    for candidate in related_candidates {
        if !candidate.relation.uses_oracle() {
            continue;
        }
        let is_strong = strongest_assertion(&candidate.test.assertions).is_some_and(|assertion| {
            assertion.oracle_strength.rank() >= OracleStrength::Strong.rank()
        });
        if !is_strong {
            continue;
        }
        saw_strong = true;
        let arglists = free_function_call_arglists(&candidate.test.body_text, &owner.name);
        if arglists.is_empty() {
            // A strong related test that reaches the owner without a direct
            // `owner(...)` call (an alias, wrapper, or indirection this scanner does
            // not resolve) might exercise the default. Fail open so a genuine
            // exposure is never suppressed.
            return None;
        }
        for arglist in arglists {
            let Some(shape) = analyze_call_args(arglist) else {
                return None; // unanalyzable call -> fail open
            };
            if changed.iter().any(|param| !shape.binds(param)) {
                return None; // some changed default is omitted -> exercised
            }
        }
    }
    if !saw_strong {
        return None; // no strong oracle -> the exposed branch is unreachable anyway
    }
    Some(changed.into_iter().map(|param| param.name).collect())
}

/// Backtick-quotes and comma-joins parameter names for a `missing` message.
pub(super) fn format_param_name_list(params: &[String]) -> String {
    params
        .iter()
        .map(|name| format!("`{name}`"))
        .collect::<Vec<_>>()
        .join(", ")
}
