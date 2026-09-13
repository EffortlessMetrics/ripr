use crate::domain::{OracleKind, OracleStrength, SymbolId};
use ra_ap_syntax::{
    AstNode, Edition, SourceFile, TextSize,
    ast::{self, HasAttrs, HasName},
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::super::extract::PROBE_SHAPE_UNSAFE_BOUNDARY;
use super::super::extract::ShadowAuthority;
use super::super::extract::extract_pattern_words;
use super::super::facts::FileFacts;
use super::super::facts::FunctionSourceRole;
use super::super::facts::LetBindingFact;
use super::super::facts::ModuleDeclarationFact;
use super::super::facts::ModulePathTarget;
use super::super::facts::SourceRoleProvenance;
use super::super::facts::cfg_predicates;
use super::{RaRustSyntaxAdapter, RustSyntaxAdapter, SyntaxNodeFact, TextRange};
use crate::analysis::rust_index::{
    FunctionFact, OracleFact, PROBE_SHAPE_CALL_DELETION, PROBE_SHAPE_ERROR_PATH,
    PROBE_SHAPE_FIELD_CONSTRUCTION, PROBE_SHAPE_MATCH_ARM, PROBE_SHAPE_PREDICATE,
    PROBE_SHAPE_RETURN_VALUE, PROBE_SHAPE_SIDE_EFFECT, ProbeShapeFact, TestFact,
    classify_assertion, err_return_guard_oracles, extract_call_facts, extract_identifier_tokens,
    extract_line_scanned_oracles, extract_literal_facts, extract_return_facts,
    guarded_result_match_scan_with_shadow_authority, is_unwrap_err_bound_error_assertion,
    unwrap_err_bound_variables,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RustIncludeDirective {
    pub(crate) line: usize,
    pub(crate) expression: String,
    pub(crate) literal_path: Option<PathBuf>,
    pub(crate) is_file_level: bool,
    /// True when the `include!` invocation's own attributes structurally
    /// require a test build (`#[cfg(test)] include!(...)`) — the invocation
    /// then only exists in test builds (#3533).
    pub(crate) requires_test: bool,
}

/// Extract actual `include!` macro nodes from parser-backed Rust syntax.
/// Comments, string contents, and similarly named macros never enter this
/// producer. Non-literal token trees remain explicit unsupported directives.
pub(crate) fn rust_include_directives(
    _path: &Path,
    text: &str,
    max_directives: usize,
) -> Result<Vec<RustIncludeDirective>, String> {
    let parse = SourceFile::parse(text, Edition::CURRENT);
    if !parse.errors().is_empty() {
        return Err("rust_include_parent_parse_unavailable".to_string());
    }
    let line_index = LineIndex::new(text);
    let mut directives = Vec::new();
    for macro_call in parse
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::MacroCall::cast)
    {
        // The macro-call node spans its attributes too, so the call's own
        // text starts at its path token: an attributed invocation
        // (`#[cfg(test)] include!(...)`) would otherwise carry the attribute
        // prefix into the callee check and be silently dropped.
        let call_range = macro_call.syntax().text_range();
        let call_start = macro_call
            .path()
            .map(|path| path.syntax().text_range().start())
            .unwrap_or(call_range.start());
        let expression = slice_text(text, call_start, call_range.end());
        let Some((callee, _)) = expression.split_once('!') else {
            continue;
        };
        if callee.trim() != "include" {
            continue;
        }
        let attributes = macro_call
            .attrs()
            .map(|attr| attr.syntax().text().to_string())
            .collect::<Vec<_>>();
        directives.push(RustIncludeDirective {
            line: line_index.line(call_start),
            literal_path: include_literal_path(&expression),
            is_file_level: !macro_call
                .syntax()
                .ancestors()
                .skip(1)
                .any(|node| ast::Module::can_cast(node.kind())),
            requires_test: cfg_predicates::attributes_require_test(
                attributes.iter().map(String::as_str),
            ),
            expression,
        });
        if directives.len() > max_directives {
            break;
        }
    }
    directives.sort_by(|left, right| {
        left.line
            .cmp(&right.line)
            .then(left.expression.cmp(&right.expression))
    });
    Ok(directives)
}

/// Re-extract oracles from a promoted function with the parser-backed rules
/// used by ordinary Rust tests. Comments and string contents therefore cannot
/// become assertion evidence. `None` means the function text was not
/// parser-valid; callers may use their existing lexical fallback then.
pub(crate) fn parser_oracles_for_function(
    function_text: &str,
    function_start_line: usize,
) -> Option<Vec<OracleFact>> {
    let parse = SourceFile::parse(function_text, Edition::CURRENT);
    if !parse.errors().is_empty() {
        return None;
    }
    let function = parse
        .tree()
        .syntax()
        .descendants()
        .find_map(ast::Fn::cast)?;
    let line_index = LineIndex::new(function_text);
    let mut oracles = extract_parser_oracles(&function, function_text, &line_index);
    let line_offset = function_start_line.saturating_sub(1);
    for oracle in &mut oracles {
        oracle.line += line_offset;
    }
    Some(oracles)
}

fn include_literal_path(expression: &str) -> Option<PathBuf> {
    let (_, arguments) = expression.split_once('!')?;
    let arguments = arguments.trim();
    let arguments = arguments.strip_suffix(';').unwrap_or(arguments).trim();
    let inner = match (arguments.chars().next()?, arguments.chars().last()?) {
        ('(', ')') | ('{', '}') | ('[', ']') => arguments.get(1..arguments.len() - 1)?.trim(),
        _ => return None,
    };
    parse_rust_string_literal(inner).map(PathBuf::from)
}

fn parse_rust_string_literal(literal: &str) -> Option<String> {
    if let Some(body) = literal
        .strip_prefix('"')
        .and_then(|body| body.strip_suffix('"'))
    {
        let mut decoded = String::new();
        let mut chars = body.chars();
        while let Some(ch) = chars.next() {
            if ch != '\\' {
                decoded.push(ch);
                continue;
            }
            match chars.next()? {
                '\\' => decoded.push('\\'),
                '"' => decoded.push('"'),
                'n' => decoded.push('\n'),
                'r' => decoded.push('\r'),
                't' => decoded.push('\t'),
                '0' => decoded.push('\0'),
                _ => return None,
            }
        }
        return Some(decoded);
    }

    let hash_count = literal
        .strip_prefix('r')?
        .chars()
        .take_while(|ch| *ch == '#')
        .count();
    let prefix_len = 1 + hash_count;
    let suffix = format!("\"{}", "#".repeat(hash_count));
    let body = literal.get(prefix_len..)?.strip_prefix('"')?;
    body.strip_suffix(&suffix).map(ToString::to_string)
}

impl RustSyntaxAdapter for RaRustSyntaxAdapter {
    fn summarize_file(&self, path: &Path, text: &str) -> Result<FileFacts, String> {
        summarize_file_with_parser(path, text)
    }

    fn changed_nodes(&self, facts: &FileFacts, ranges: &[TextRange]) -> Vec<SyntaxNodeFact> {
        owner_changed_nodes(facts, ranges)
    }
}

pub fn summarize_file_with_parser(path: &Path, text: &str) -> Result<FileFacts, String> {
    let parse = SourceFile::parse(text, Edition::CURRENT);
    let errors = parse.errors();
    if !errors.is_empty() {
        return Err(format!("parser reported {} syntax errors", errors.len()));
    }

    let source = parse.tree();
    let line_index = LineIndex::new(text);
    let module_declarations = module_declaration_facts(&source, &line_index);
    let mut functions = Vec::new();
    let mut tests = Vec::new();
    let mut file_calls = Vec::new();
    let mut file_returns = Vec::new();
    let mut file_literals = Vec::new();
    let mut file_probe_shapes = Vec::new();
    let path_buf = path.to_path_buf();

    for function in source.syntax().descendants().filter_map(ast::Fn::cast) {
        let Some(name) = function.name().map(|name| name.text().to_string()) else {
            continue;
        };
        let fn_start = function
            .fn_token()
            .map(|token| token.text_range().start())
            .unwrap_or_else(|| function.syntax().text_range().start());
        let fn_end = function.syntax().text_range().end();
        let start_line = line_index.line(fn_start);
        let end_line = line_index.line_for_range_end(fn_end);
        let body = slice_text(text, fn_start, fn_end);
        let calls = extract_call_facts(&body, start_line);
        let returns = extract_return_facts(&body, start_line);
        let literals = extract_literal_facts(&body, start_line);
        let probe_shapes = extract_parser_probe_shapes(&function, text, &line_index);
        // A plain helper inside an inline `#[cfg(test)]` module is test
        // infrastructure even when it has no `#[test]` attribute. Classify
        // that role at the producer boundary so diff probes, seam inventory,
        // evidence relation, and every downstream renderer consume the same
        // fact instead of re-inferring it independently. The typed role
        // (#3531) keeps the executable-test and evidence-only helper
        // meanings apart instead of collapsing both into one bit.
        let has_test_attribute = has_test_attribute(&function);
        let source_role = if has_test_attribute {
            FunctionSourceRole::TestAttribute
        } else if is_cfg_test_module_member(&function) {
            FunctionSourceRole::CfgTestModule
        } else {
            FunctionSourceRole::Production
        };
        let attrs = collect_attr_syntax(&function);
        // #3727 Slice A: parser-backed test-body shadow facts. Nested `fn`
        // item names and `let` binding facts over this function's body,
        // body-relative (relative to the `fn` keyword line, which is also
        // where the stored `body` slice starts). The lexical fallback
        // producer leaves both fields empty; on parser-backed files an
        // empty set is a real "no definition" result.
        let (nested_fn_names, let_bindings) =
            collect_body_shadow_facts(&function, &|offset| line_index.line(offset), start_line);

        file_calls.extend(calls.clone());
        file_returns.extend(returns.clone());
        file_literals.extend(literals.clone());
        file_probe_shapes.extend(probe_shapes);

        let function_fact = FunctionFact {
            id: parser_symbol_id(path, &function, &name),
            name: name.clone(),
            file: path_buf.clone(),
            start_line,
            end_line,
            body: body.clone(),
            calls: calls.clone(),
            returns: returns.clone(),
            literals: literals.clone(),
            source_role,
            attrs: attrs.clone(),
            nested_fn_names: nested_fn_names.clone(),
            let_bindings: let_bindings.clone(),
        };

        if has_test_attribute {
            tests.push(TestFact {
                name,
                file: path_buf.clone(),
                start_line,
                end_line,
                body,
                calls,
                assertions: extract_parser_oracles(&function, text, &line_index),
                literals,
                attrs,
                nested_fn_names,
                let_bindings,
            });
        }

        functions.push(function_fact);
    }

    disambiguate_duplicate_symbol_ids(&mut functions);

    file_calls.sort_by(|a, b| a.line.cmp(&b.line).then(a.name.cmp(&b.name)));
    file_calls.dedup_by(|a, b| a.line == b.line && a.name == b.name && a.text == b.text);
    file_returns.sort_by(|a, b| a.line.cmp(&b.line).then(a.text.cmp(&b.text)));
    file_returns.dedup_by(|a, b| a.line == b.line && a.text == b.text);
    file_literals.sort_by(|a, b| a.line.cmp(&b.line).then(a.value.cmp(&b.value)));
    file_literals.dedup_by(|a, b| a.line == b.line && a.value == b.value);
    file_probe_shapes.sort_by(|a, b| {
        a.start_line
            .cmp(&b.start_line)
            .then(a.end_line.cmp(&b.end_line))
            .then(a.kind.cmp(&b.kind))
            .then(a.text.cmp(&b.text))
    });
    file_probe_shapes.dedup_by(|a, b| {
        a.start_line == b.start_line
            && a.end_line == b.end_line
            && a.kind == b.kind
            && a.text == b.text
    });

    Ok(FileFacts {
        path: path_buf,
        functions,
        tests,
        calls: file_calls,
        returns: file_returns,
        literals: file_literals,
        probe_shapes: file_probe_shapes,
        used_lexical_fallback: false,
        module_declarations,
        role_provenance: SourceRoleProvenance::default(),
        source: text.to_string(),
    })
}

/// Extracts the file's top-level out-of-line `mod name;` declarations (#3533).
///
/// This is the producer side of the module composition edge: each fact carries
/// the declared name, the `#[path]` target shape, and the cfg-test requirement
/// classified once through the shared `cfg_predicates` authority (#3530).
///
/// Deliberately not captured here (typed fail-closed status quo, no composed
/// role is granted from them):
/// - inline `mod name { ... }` bodies — their members are classified by the
///   same-file cfg-test membership walk;
/// - out-of-line declarations nested inside an inline module — cross-file
///   resolution through inline nesting has no producer yet;
/// - block modules (`fn f() { mod inner; }`) — not top-level items.
///
/// The lexical fallback producer emits no module declarations at all: without
/// a parse, the facts layer cannot see declarations, and guessing from line
/// text would re-create the drifted-matcher family #3530 removed.
fn module_declaration_facts(
    source: &SourceFile,
    line_index: &LineIndex,
) -> Vec<ModuleDeclarationFact> {
    let mut declarations = Vec::new();
    for module in source.syntax().children().filter_map(ast::Module::cast) {
        // Out-of-line only: `mod name;` carries no item list.
        if module.item_list().is_some() {
            continue;
        }
        let Some(name) = module.name() else {
            continue;
        };
        let line = module
            .mod_token()
            .map(|token| line_index.line(token.text_range().start()))
            .unwrap_or_else(|| line_index.line(module.syntax().text_range().start()));
        let attributes = module
            .attrs()
            .map(|attr| attr.syntax().text().to_string())
            .collect::<Vec<_>>();
        declarations.push(ModuleDeclarationFact {
            name: name.text().to_string(),
            line,
            path_target: path_target_from_attributes(&attributes),
            requires_test: cfg_predicates::attributes_require_test(
                attributes.iter().map(String::as_str),
            ),
        });
    }
    declarations.sort_by(|left, right| left.line.cmp(&right.line).then(left.name.cmp(&right.name)));
    declarations.dedup();
    declarations
}

/// Collects the parser-backed test-body shadow facts for one function
/// (#3727 Slice A): the names of `fn` items nested inside the function's
/// body, and one [`LetBindingFact`] per whole-word name in every
/// initialized `let` binding's pattern, line-relative to the function's
/// `fn` keyword line.
///
/// The semantics are deliberately scanner-equivalent at line granularity
/// (see `analysis::extract::shadow`): a nested `fn <callee>` item is
/// hoisted and defeats the whole body, and a binding defeats uses at and
/// after its own body-relative line. No scope extents, no columns. The
/// pattern names are extracted with the shared lexical authority's
/// whole-word vocabulary (`extract_pattern_words`), so the fact-derived
/// decisions equal the lexical scanners' decisions on the same masked body
/// for realistic Rust — the equivalence battery in this module pins it.
///
/// `line_of` maps a tree offset to a 1-based line, and `fn_line` is the
/// 1-based line of the function's `fn` keyword — the two inputs that make
/// this helper reusable over the file text (the summarizer) and over a
/// standalone function text (re-extraction), each with its own
/// [`LineIndex`].
fn collect_body_shadow_facts(
    function: &ast::Fn,
    line_of: &impl Fn(TextSize) -> usize,
    fn_line: usize,
) -> (Vec<String>, Vec<LetBindingFact>) {
    let mut nested_fn_names: Vec<String> = Vec::new();
    for nested_fn in function.syntax().descendants().filter_map(ast::Fn::cast) {
        // Descendants include the function itself; text ranges uniquely
        // identify nodes within the tree, so this drops exactly the self
        // entry.
        if nested_fn.syntax().text_range() == function.syntax().text_range() {
            continue;
        }
        if let Some(name) = nested_fn.name() {
            nested_fn_names.push(name.text().to_string());
        }
    }
    nested_fn_names.sort();
    nested_fn_names.dedup();

    let mut let_bindings: Vec<LetBindingFact> = Vec::new();
    for let_stmt in function
        .syntax()
        .descendants()
        .filter_map(ast::LetStmt::cast)
    {
        // The initializer-less `;` bound (#3728 round-5): `let flag;` has
        // no initializer, produces no binding fact, and can never shadow.
        if let_stmt.initializer().is_none() {
            continue;
        }
        let Some(pattern) = let_stmt.pat() else {
            continue;
        };
        let let_line = let_stmt
            .let_token()
            .map(|token| line_of(token.text_range().start()))
            .unwrap_or_else(|| line_of(let_stmt.syntax().text_range().start()));
        let Some(body_relative_line) = let_line.checked_sub(fn_line) else {
            continue;
        };
        for name in extract_pattern_words(&pattern.syntax().text().to_string()) {
            let_bindings.push(LetBindingFact {
                line: body_relative_line,
                name,
            });
        }
    }
    let_bindings.sort_by(|left, right| left.line.cmp(&right.line).then(left.name.cmp(&right.name)));
    let_bindings.dedup();
    (nested_fn_names, let_bindings)
}

/// Parser-backed shadow facts (#3727 Slice A) for a bare body text that is
/// not necessarily a `fn` item — harness-subject invocation spans and
/// merged evidence bodies. The text is wrapped in a synthetic `fn` item
/// whose braces share the body's first line, so every wrapped fact's
/// body-relative line equals its line relative to the ORIGINAL body text
/// and needs no offset repair. Empty facts when the wrapped text does not
/// parse cleanly: the descent only ever sees real syntax, and the
/// consumers' flag law routes fallback decisions off these fields.
pub(crate) fn shadow_facts_for_body_text(body: &str) -> (Vec<String>, Vec<LetBindingFact>) {
    // The wrapper must be a COMPLETE fn item: an unclosed brace makes
    // `SourceFile::parse` report errors and the fail-closed branch below
    // return empty facts for every body — the exact producer regression
    // the non-empty-facts tests pin (#3739 review). The closing brace sits
    // on its own line after the body, so the opening line — and every
    // body-relative offset — is unchanged.
    let wrapped = format!("fn __ripr_shadow_facts__() {{ {body}\n}}");
    let parse = SourceFile::parse(&wrapped, Edition::CURRENT);
    if !parse.errors().is_empty() {
        return (Vec::new(), Vec::new());
    }
    let Some(function) = parse.tree().syntax().descendants().find_map(ast::Fn::cast) else {
        return (Vec::new(), Vec::new());
    };
    let line_index = LineIndex::new(&wrapped);
    // The `fn` keyword sits at offset 0, i.e. line 1 — exactly the base
    // that makes `line - fn_line` body-relative for the original text.
    collect_body_shadow_facts(&function, &|offset| line_index.line(offset), 1)
}

/// Classifies the `#[path]` attributes of one declaration into the bounded
/// target shape. Exactly one plain string-literal attribute resolves; absence
/// falls back to default name resolution; anything else (multiple `path`
/// attributes, macro-call arguments, concatenated targets) is a typed unknown
/// that fails closed downstream instead of resolving the wrong file.
///
/// A `path` attribute introduced conditionally by `cfg_attr` is also a typed
/// unknown (#3533): which file the compiler loads depends on the active
/// configuration, so neither the introduced target nor the default layout is
/// statically resolvable. Returning `Default` here would resolve the default
/// file that Rust does not compile under the conditional configuration and
/// could hand its functions an evidence role they did not earn.
fn path_target_from_attributes(attributes: &[String]) -> ModulePathTarget {
    if cfg_predicates::attributes_conditionally_introduce_path(
        attributes.iter().map(String::as_str),
    ) {
        return ModulePathTarget::Unknown;
    }
    let path_attributes = attributes
        .iter()
        .filter(|attribute| attribute_path_name(attribute).as_deref() == Some("path"))
        .collect::<Vec<_>>();
    match path_attributes.as_slice() {
        [] => ModulePathTarget::Default,
        [attribute] => match attribute_string_literal(attribute) {
            Some(literal) => ModulePathTarget::Literal(literal),
            None => ModulePathTarget::Unknown,
        },
        // Duplicate `#[path]` attributes are ambiguous input.
        [..] => ModulePathTarget::Unknown,
    }
}

/// Returns the attribute's path name (`path` for `#[path = "..."]`), using the
/// same whitespace-insensitive normalization as the test-attribute classifier.
fn attribute_path_name(attribute: &str) -> Option<String> {
    let body = attribute.trim().strip_prefix("#[")?;
    let body = body.strip_prefix("![").unwrap_or(body);
    let closing = body.rfind(']')?;
    let head = body.get(..closing)?.trim();
    let head = head.split('=').next()?.trim();
    let path = head
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    let path = path.trim_start_matches("::");
    (!path.is_empty()).then_some(path.to_string())
}

/// Returns the string-literal argument of `#[path = "<literal>"]`, decoding
/// plain and raw string spellings. `None` for any other shape.
fn attribute_string_literal(attribute: &str) -> Option<String> {
    let body = attribute.trim().strip_prefix("#[")?;
    let closing = body.rfind(']')?;
    let head = body.get(..closing)?;
    let (_, value) = head.split_once('=')?;
    parse_rust_string_literal(value.trim())
}

fn parser_symbol_id(path: &Path, function: &ast::Fn, name: &str) -> SymbolId {
    // The stable text projection keeps distinct raw-byte paths distinct
    // in owner identity; lossy rendering would merge same-named
    // functions across invalid-byte filenames (#3609).
    let mut segments = vec![crate::analysis::stable_path_text(path)];

    let mut modules = function
        .syntax()
        .ancestors()
        .skip(1)
        .filter_map(ast::Module::cast)
        .filter_map(|module| {
            module
                .name()
                .map(|module_name| module_name.text().to_string())
        })
        .collect::<Vec<_>>();
    modules.reverse();
    segments.extend(modules);

    if let Some(impl_block) = function
        .syntax()
        .ancestors()
        .skip(1)
        .find_map(ast::Impl::cast)
    {
        segments.push(impl_owner_segment(&impl_block));
    }

    segments.push(name.to_string());
    SymbolId(segments.join("::"))
}

fn impl_owner_segment(impl_block: &ast::Impl) -> String {
    let self_ty = match impl_block.self_ty() {
        Some(ty) => compact_syntax_text(ty.syntax().text().to_string()),
        None => "unknown".to_string(),
    };
    match impl_block.trait_() {
        Some(trait_ty) => format!(
            "impl {} for {self_ty}",
            compact_syntax_text(trait_ty.syntax().text().to_string())
        ),
        None => format!("impl {self_ty}"),
    }
}

fn compact_syntax_text(text: String) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn disambiguate_duplicate_symbol_ids(functions: &mut [FunctionFact]) {
    let mut totals = BTreeMap::new();
    for function in functions.iter() {
        let entry = totals.entry(function.id.0.clone()).or_insert(0usize);
        *entry += 1;
    }

    for function in functions.iter_mut() {
        let total = match totals.get(&function.id.0) {
            Some(total) => *total,
            None => 0,
        };
        if total > 1 {
            function.id.0 = format!("{}#L{}", function.id.0, function.start_line);
        }
    }
}

fn has_test_attribute(function: &ast::Fn) -> bool {
    function.attrs().any(|attr| {
        let compact = attr
            .syntax()
            .text()
            .to_string()
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .collect::<String>();
        compact == "#[test]"
            || compact.starts_with("#[tokio::test")
            || compact.starts_with("#[async_std::test")
            || compact == "#[rstest]"
            || compact.starts_with("#[rstest(")
    })
}

/// Returns whether a function is nested in a module gated by a test-only cfg.
///
/// The cfg-term semantics live in one shared authority
/// (`facts::cfg_predicates`, #3530) so the parser producer and the facts
/// normalizer cannot drift: nested `all(test, ...)` conjunctions, whitespace
/// and multi-line spellings, and bounded `cfg_attr` forms classify
/// identically on both sides.
fn is_cfg_test_module_member(function: &ast::Fn) -> bool {
    function
        .syntax()
        .ancestors()
        .filter_map(ast::Module::cast)
        .any(|module| {
            cfg_predicates::attributes_require_test(
                module.attrs().map(|attr| attr.syntax().text().to_string()),
            )
        })
}

fn collect_attr_syntax(function: &ast::Fn) -> Vec<String> {
    function
        .attrs()
        .map(|attr| attr.syntax().text().to_string())
        .collect()
}

fn extract_parser_probe_shapes(
    function: &ast::Fn,
    text: &str,
    line_index: &LineIndex,
) -> Vec<ProbeShapeFact> {
    let mut shapes = Vec::new();
    push_unsafe_boundary_probe_shapes(&mut shapes, function, line_index);

    for if_expr in function
        .syntax()
        .descendants()
        .filter_map(ast::IfExpr::cast)
    {
        if let Some(condition) = if_expr.condition() {
            push_probe_shape(
                &mut shapes,
                line_index,
                text,
                PROBE_SHAPE_PREDICATE,
                condition.syntax().text_range().start(),
                condition.syntax().text_range().end(),
            );
        }
    }

    for while_expr in function
        .syntax()
        .descendants()
        .filter_map(ast::WhileExpr::cast)
    {
        if let Some(condition) = while_expr.condition() {
            push_probe_shape(
                &mut shapes,
                line_index,
                text,
                PROBE_SHAPE_PREDICATE,
                condition.syntax().text_range().start(),
                condition.syntax().text_range().end(),
            );
        }
    }

    for bin_expr in function
        .syntax()
        .descendants()
        .filter_map(ast::BinExpr::cast)
    {
        if bin_expr
            .op_token()
            .is_some_and(|token| is_predicate_operator(token.text()))
        {
            push_probe_shape(
                &mut shapes,
                line_index,
                text,
                PROBE_SHAPE_PREDICATE,
                bin_expr.syntax().text_range().start(),
                bin_expr.syntax().text_range().end(),
            );
        }
    }

    for return_expr in function
        .syntax()
        .descendants()
        .filter_map(ast::ReturnExpr::cast)
    {
        let range = return_expr.syntax().text_range();
        push_probe_shape(
            &mut shapes,
            line_index,
            text,
            PROBE_SHAPE_RETURN_VALUE,
            range.start(),
            range.end(),
        );
        let return_text = slice_text(text, range.start(), range.end());
        if has_error_path_text(&return_text) {
            push_probe_shape(
                &mut shapes,
                line_index,
                text,
                PROBE_SHAPE_ERROR_PATH,
                range.start(),
                range.end(),
            );
        }
    }

    if let Some(tail_expr) = function.body().and_then(|body| body.tail_expr()) {
        let range = tail_expr.syntax().text_range();
        let tail_text = slice_text(text, range.start(), range.end());
        if is_tail_return_value_text(&tail_text) {
            push_probe_shape(
                &mut shapes,
                line_index,
                text,
                PROBE_SHAPE_RETURN_VALUE,
                range.start(),
                range.end(),
            );
            if has_error_path_text(&tail_text) {
                push_probe_shape(
                    &mut shapes,
                    line_index,
                    text,
                    PROBE_SHAPE_ERROR_PATH,
                    range.start(),
                    range.end(),
                );
            }
        }
    }

    for call_expr in function
        .syntax()
        .descendants()
        .filter_map(ast::CallExpr::cast)
    {
        let range = call_expr.syntax().text_range();
        let call_text = slice_text(text, range.start(), range.end());
        push_probe_shape(
            &mut shapes,
            line_index,
            text,
            PROBE_SHAPE_CALL_DELETION,
            range.start(),
            range.end(),
        );
        if has_return_value_text(&call_text) && !call_is_argument(&call_expr) {
            push_probe_shape(
                &mut shapes,
                line_index,
                text,
                PROBE_SHAPE_RETURN_VALUE,
                range.start(),
                range.end(),
            );
        }
        if has_error_path_text(&call_text) {
            push_probe_shape(
                &mut shapes,
                line_index,
                text,
                PROBE_SHAPE_ERROR_PATH,
                range.start(),
                range.end(),
            );
        }
    }

    for method_call in function
        .syntax()
        .descendants()
        .filter_map(ast::MethodCallExpr::cast)
    {
        let range = method_call.syntax().text_range();
        let method_text = slice_text(text, range.start(), range.end());
        push_probe_shape(
            &mut shapes,
            line_index,
            text,
            PROBE_SHAPE_CALL_DELETION,
            range.start(),
            range.end(),
        );
        if method_call
            .name_ref()
            .is_some_and(|name| is_effect_call_name(&name.syntax().text().to_string()))
            || has_effect_text(&method_text)
        {
            push_probe_shape(
                &mut shapes,
                line_index,
                text,
                PROBE_SHAPE_SIDE_EFFECT,
                range.start(),
                range.end(),
            );
        }
    }

    for field in function
        .syntax()
        .descendants()
        .filter_map(ast::RecordExprField::cast)
    {
        let range = field.syntax().text_range();
        push_probe_shape(
            &mut shapes,
            line_index,
            text,
            PROBE_SHAPE_FIELD_CONSTRUCTION,
            range.start(),
            range.end(),
        );
    }

    for match_expr in function
        .syntax()
        .descendants()
        .filter_map(ast::MatchExpr::cast)
    {
        if let Some(token) = match_expr.match_token() {
            push_probe_shape_with_text(
                &mut shapes,
                line_index,
                PROBE_SHAPE_MATCH_ARM,
                token.text_range().start(),
                token.text_range().end(),
                match_expr_probe_text(
                    text,
                    match_expr.expr().map(|expr| expr.syntax().text_range()),
                    match_expr.syntax().text_range(),
                ),
            );
        }
    }

    for arm in function
        .syntax()
        .descendants()
        .filter_map(ast::MatchArm::cast)
    {
        if let Some(token) = arm.fat_arrow_token() {
            push_probe_shape_with_text(
                &mut shapes,
                line_index,
                PROBE_SHAPE_MATCH_ARM,
                token.text_range().start(),
                token.text_range().end(),
                match_arm_probe_text(
                    text,
                    arm.syntax().text_range().start(),
                    token.text_range().start(),
                ),
            );
        }
    }

    shapes.sort_by(|a, b| {
        a.start_line
            .cmp(&b.start_line)
            .then(a.end_line.cmp(&b.end_line))
            .then(a.kind.cmp(&b.kind))
            .then(a.text.cmp(&b.text))
    });
    shapes.dedup_by(|a, b| {
        a.start_line == b.start_line
            && a.end_line == b.end_line
            && a.kind == b.kind
            && a.text == b.text
    });
    shapes
}

fn push_unsafe_boundary_probe_shapes(
    shapes: &mut Vec<ProbeShapeFact>,
    function: &ast::Fn,
    line_index: &LineIndex,
) {
    if let Some(unsafe_token) = function.unsafe_token() {
        let name = function
            .name()
            .map(|name| name.text().to_string())
            .unwrap_or_else(|| "<anonymous>".to_string());
        push_probe_shape_with_text(
            shapes,
            line_index,
            PROBE_SHAPE_UNSAFE_BOUNDARY,
            unsafe_token.text_range().start(),
            function.syntax().text_range().end(),
            format!("unsafe fn {name}"),
        );
    }

    for block in function
        .syntax()
        .descendants()
        .filter_map(ast::BlockExpr::cast)
    {
        let Some(unsafe_token) = block.unsafe_token() else {
            continue;
        };
        push_probe_shape_with_text(
            shapes,
            line_index,
            PROBE_SHAPE_UNSAFE_BOUNDARY,
            unsafe_token.text_range().start(),
            block.syntax().text_range().end(),
            "unsafe block".to_string(),
        );
    }
}

fn push_probe_shape(
    shapes: &mut Vec<ProbeShapeFact>,
    line_index: &LineIndex,
    text: &str,
    kind: &str,
    start: TextSize,
    end: TextSize,
) {
    let snippet = slice_text(text, start, end)
        .trim()
        .trim_end_matches(';')
        .to_string();
    if snippet.is_empty() {
        return;
    }
    push_probe_shape_with_text(shapes, line_index, kind, start, end, snippet);
}

fn push_probe_shape_with_text(
    shapes: &mut Vec<ProbeShapeFact>,
    line_index: &LineIndex,
    kind: &str,
    start: TextSize,
    end: TextSize,
    snippet: String,
) {
    if snippet.is_empty() {
        return;
    }
    shapes.push(ProbeShapeFact {
        start_line: line_index.line(start),
        end_line: line_index.line_for_range_end(end),
        start_byte: u32::from(start) as usize,
        kind: kind.to_string(),
        text: snippet,
    });
}

fn match_expr_probe_text(
    text: &str,
    scrutinee_range: Option<ra_ap_syntax::TextRange>,
    fallback_range: ra_ap_syntax::TextRange,
) -> String {
    if let Some(range) = scrutinee_range {
        let scrutinee = normalize_probe_shape_text(&slice_text(text, range.start(), range.end()));
        if !scrutinee.is_empty() {
            return format!("match {scrutinee}");
        }
    }

    let raw = slice_text(text, fallback_range.start(), fallback_range.end());
    let snippet = raw.trim();
    let head = snippet
        .split_once('{')
        .map(|(head, _)| head)
        .unwrap_or(snippet);
    normalize_probe_shape_text(head)
}

fn match_arm_probe_text(text: &str, start: TextSize, arrow_start: TextSize) -> String {
    let raw = slice_text(text, start, arrow_start);
    let pattern = raw.trim();
    let head = format!("{pattern} =>");
    normalize_probe_shape_text(&head)
}

fn normalize_probe_shape_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_predicate_operator(operator: &str) -> bool {
    matches!(
        operator,
        "==" | "!=" | "<=" | ">=" | "<" | ">" | "&&" | "||"
    )
}

fn has_return_value_text(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with("Ok(")
        || trimmed.starts_with("Some(")
        || trimmed.contains(" Ok(")
        || trimmed.contains(" Some(")
        || trimmed.contains("None")
}

fn call_is_argument(call: &ast::CallExpr) -> bool {
    call.syntax()
        .parent()
        .and_then(ast::ArgList::cast)
        .is_some()
}

fn is_tail_return_value_text(text: &str) -> bool {
    let trimmed = text.trim_start();
    !trimmed.is_empty()
        && !trimmed.starts_with("if ")
        && !trimmed.starts_with("match ")
        && !trimmed.starts_with("while ")
        && !trimmed.starts_with("for ")
        && !trimmed.starts_with("loop ")
}

fn has_error_path_text(text: &str) -> bool {
    text.contains("Err(")
        || text.contains("Error::")
        || text.contains("map_err")
        || text.contains("bail!")
        || text.contains("anyhow!")
}

fn has_effect_text(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    [
        ".save(",
        ".publish(",
        ".send(",
        ".write(",
        ".insert(",
        ".push(",
        ".remove(",
        ".delete(",
        ".emit(",
        ".increment(",
        "metrics.",
        "log::",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn is_effect_call_name(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "save"
            | "publish"
            | "send"
            | "write"
            | "insert"
            | "push"
            | "remove"
            | "delete"
            | "emit"
            | "increment"
    )
}

fn extract_parser_oracles(
    function: &ast::Fn,
    text: &str,
    line_index: &LineIndex,
) -> Vec<OracleFact> {
    // RIPR-SPEC-0106 (Part A): pre-scan the function body for `unwrap_err`/
    // `expect_err` variable bindings so assertions on those variables can be
    // upgraded to ExactErrorVariant.
    let function_text = function.syntax().text().to_string();
    let bound_error_vars = unwrap_err_bound_variables(&function_text);

    let mut assertions = Vec::new();
    for macro_call in function
        .syntax()
        .descendants()
        .filter_map(ast::MacroCall::cast)
    {
        let Some(path) = macro_call.path() else {
            continue;
        };
        let macro_name = path.syntax().text().to_string().replace(' ', "");
        if !is_assertion_macro(&macro_name) {
            continue;
        }
        let range = macro_call.syntax().text_range();
        let assertion_text = slice_macro_call_text(text, range.start(), range.end());
        let mut classification = classify_assertion(&assertion_text);
        // Upgrade exact assertions on unwrap_err-bound variables to
        // ExactErrorVariant when the assertion pins a specific error result
        // (RIPR-SPEC-0106, Part A). Constructor-payload equality reaches this
        // point as WholeObjectEquality.
        if matches!(
            classification.kind,
            OracleKind::ExactValue | OracleKind::WholeObjectEquality
        ) && is_unwrap_err_bound_error_assertion(&assertion_text, &bound_error_vars)
        {
            classification.kind = OracleKind::ExactErrorVariant;
            classification.strength = OracleStrength::Strong;
        }
        assertions.push(OracleFact {
            line: line_index.line(range.start()),
            kind: classification.kind,
            strength: classification.strength,
            observed_tokens: extract_identifier_tokens(&assertion_text),
            text: assertion_text,
            ok_value_observed: None,
        });
    }

    for method_call in function
        .syntax()
        .descendants()
        .filter_map(ast::MethodCallExpr::cast)
    {
        let Some(name) = method_call
            .name_ref()
            .map(|name| name.syntax().text().to_string())
        else {
            continue;
        };
        if name != "unwrap" && name != "expect" {
            continue;
        }
        let range = method_call.syntax().text_range();
        let text = slice_text(text, range.start(), range.end())
            .trim()
            .trim_end_matches(';')
            .to_string();
        assertions.push(OracleFact {
            line: line_index.line(range.start()),
            kind: OracleKind::SmokeOnly,
            strength: OracleStrength::Smoke,
            observed_tokens: extract_identifier_tokens(&text),
            text,
            ok_value_observed: None,
        });
    }

    let function_start = line_index.line(function.syntax().text_range().start());
    // #3284: terminal Err-return guards credit as their assertion twins
    // on the parser path too, so both adapters agree.
    for oracle in err_return_guard_oracles(&function_text, function_start) {
        assertions.push(oracle);
    }
    // #3727 Slice A: parser-backed shadow facts over the scanned text. The
    // guarded scan runs over `function_text` (the AST node text, which
    // includes the attribute lines) and its use-site coordinate counts
    // 0-based lines from the TOP of that text, so the facts must share
    // that base: `line_of` maps to function_text lines and the base is
    // line 1, making `LetBindingFact.line` a 0-based function_text line —
    // the same verdict the lexical scanner over the same text would reach
    // (#3739 review, coderabbit h6cfG: with a fn-keyword base, attribute
    // lines shifted the let coordinates and a binding AFTER a guarded
    // match could suppress that earlier oracle).
    let node_start = function.syntax().text_range().start();
    let local_line_index = LineIndex::new(&function_text);
    let (nested_fn_names, let_bindings) = collect_body_shadow_facts(
        function,
        &|offset| local_line_index.line(offset - node_start),
        1,
    );
    // #3709: guarded Result matches over direct callee results are extracted
    // by the dedicated scanner, which owns the whole statement — the
    // line-scanned statement joiner must not also swallow the match block
    // through the `expect_` name sniff.
    let guarded_matches = guarded_result_match_scan_with_shadow_authority(
        &function_text,
        function_start,
        ShadowAuthority::ParserBodyFacts {
            nested_fn_names: &nested_fn_names,
            let_bindings: &let_bindings,
        },
    );
    for oracle in
        extract_line_scanned_oracles(&function.syntax().text().to_string(), function_start)
            .into_iter()
            .filter(|oracle| !guarded_matches.match_start_lines.contains(&oracle.line))
    {
        assertions.push(oracle);
    }
    for oracle in guarded_matches.oracles {
        assertions.push(oracle);
    }

    assertions.sort_by(|a, b| a.line.cmp(&b.line).then(a.text.cmp(&b.text)));
    assertions.dedup_by(|a, b| a.line == b.line && a.text == b.text);
    assertions
}

/// Token-local leaf form of [`is_assertion_macro`] for scanners that see
/// one path segment at a time: the exact built-in assertion names plus the
/// supported snapshot-macro naming boundary (`*_snapshot`). The
/// source-side `contains("snapshot")` rule is deliberately NOT carried
/// over — a leaf ident like `snapshot_helper` must not classify, while
/// `assert_snapshot` / `assert_json_snapshot` do.
pub(crate) fn is_assertion_macro_leaf(name: &str) -> bool {
    matches!(
        name,
        "assert" | "assert_eq" | "assert_ne" | "assert_matches" | "matches"
    ) || name.ends_with("snapshot")
}

pub(crate) fn is_assertion_macro(macro_name: &str) -> bool {
    matches!(
        macro_name,
        "assert" | "assert_eq" | "assert_ne" | "assert_matches" | "matches"
    ) || macro_name.starts_with("insta::assert")
        || macro_name.contains("snapshot")
}

pub(crate) struct LineIndex {
    starts: Vec<usize>,
}

impl LineIndex {
    pub(crate) fn new(text: &str) -> Self {
        let mut starts = vec![0];
        for (index, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                starts.push(index + 1);
            }
        }
        Self { starts }
    }

    pub(crate) fn line(&self, offset: TextSize) -> usize {
        self.line_from_offset(text_size_to_usize(offset))
    }

    pub(crate) fn line_for_range_end(&self, offset: TextSize) -> usize {
        self.line_from_offset(text_size_to_usize(offset).saturating_sub(1))
    }

    fn line_from_offset(&self, offset: usize) -> usize {
        match self.starts.binary_search(&offset) {
            Ok(index) => index + 1,
            Err(index) => index.max(1),
        }
    }
}

fn text_size_to_usize(offset: TextSize) -> usize {
    let value: u32 = offset.into();
    value as usize
}

pub(crate) fn slice_text(text: &str, start: TextSize, end: TextSize) -> String {
    let start = text_size_to_usize(start);
    let end = text_size_to_usize(end);
    text.get(start..end).unwrap_or("").to_string()
}

fn slice_macro_call_text(text: &str, start: TextSize, end: TextSize) -> String {
    let start = text_size_to_usize(start);
    let mut end = text_size_to_usize(end);
    let bytes = text.as_bytes();
    let mut cursor = end;
    while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() && bytes[cursor] != b'\n' {
        cursor += 1;
    }
    if bytes.get(cursor) == Some(&b';') {
        end = cursor + 1;
    }
    text.get(start..end).unwrap_or("").trim().to_string()
}

fn owner_changed_nodes(
    facts: &crate::analysis::facts::FileFacts,
    ranges: &[TextRange],
) -> Vec<SyntaxNodeFact> {
    let mut nodes = Vec::new();
    for range in ranges {
        let mut owners = facts
            .functions
            .iter()
            .filter(|function| {
                ranges_overlap(
                    range.start_line,
                    range.end_line,
                    function.start_line,
                    function.end_line,
                )
            })
            .collect::<Vec<_>>();
        owners.sort_by(|left, right| {
            function_span(left)
                .cmp(&function_span(right))
                .then(right.start_line.cmp(&left.start_line))
                .then(left.id.0.cmp(&right.id.0))
        });
        if let Some(function) = owners.first() {
            nodes.push(SyntaxNodeFact {
                file: function.file.clone(),
                kind: if function.source_role.is_evidence_role() {
                    "test_function".to_string()
                } else {
                    "function".to_string()
                },
                start_line: function.start_line,
                end_line: function.end_line,
                text: function.body.clone(),
                owner: Some(function.id.clone()),
            });
        }
    }
    nodes.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then(left.start_line.cmp(&right.start_line))
            .then(left.end_line.cmp(&right.end_line))
            .then(left.kind.cmp(&right.kind))
            .then(left.owner.cmp(&right.owner))
    });
    nodes.dedup_by(|left, right| {
        left.file == right.file
            && left.start_line == right.start_line
            && left.end_line == right.end_line
            && left.kind == right.kind
            && left.owner == right.owner
    });
    nodes
}

fn ranges_overlap(start1: usize, end1: usize, start2: usize, end2: usize) -> bool {
    start1 <= end2 && start2 <= end1
}

fn function_span(function: &FunctionFact) -> usize {
    function.end_line - function.start_line
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    #[cfg(unix)]
    fn parser_symbol_id_keeps_raw_byte_files_distinct() -> Result<(), Box<dyn Error>> {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        let source = "fn bounds_check() {}\n";
        let parse_a = SourceFile::parse(source, Edition::CURRENT);
        let parse_b = SourceFile::parse(source, Edition::CURRENT);
        let function = |tree: &SourceFile| {
            tree.syntax()
                .descendants()
                .find_map(ast::Fn::cast)
                .ok_or("function present")
        };
        let function_a = function(&parse_a.tree())?;
        let function_b = function(&parse_b.tree())?;
        let raw_a = Path::new(OsStr::from_bytes(b"pricing_\xff.rs"));
        let raw_b = Path::new(OsStr::from_bytes(b"pricing_\xfe.rs"));
        let id_a = parser_symbol_id(raw_a, &function_a, "bounds_check");
        let id_b = parser_symbol_id(raw_b, &function_b, "bounds_check");
        assert_ne!(
            id_a, id_b,
            "distinct raw-byte files must own distinct symbols"
        );
        Ok(())
    }

    fn temp_dir(name: &str) -> Result<PathBuf, Box<dyn Error>> {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let dir = std::env::temp_dir().join(format!("ripr-{name}-{stamp}"));
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    fn write_manifest(root: &Path) -> Result<(), Box<dyn Error>> {
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='test'\nversion='0.1.0'\nedition='2024'\n",
        )?;
        Ok(())
    }

    #[test]
    fn ra_adapter_parses_valid_rust_source() -> Result<(), Box<dyn Error>> {
        let root = temp_dir("ra_parser")?;
        fs::create_dir_all(root.join("src"))?;
        write_manifest(&root)?;
        fs::write(
            root.join("src/lib.rs"),
            r#"
pub fn calculate(x: i32, y: i32) -> i32 {
    if x > y { x } else { y }
}

#[test]
fn test_calculate() {
    assert_eq!(calculate(5, 3), 5);
}
"#,
        )?;

        let adapter = RaRustSyntaxAdapter;
        let text = fs::read_to_string(root.join("src/lib.rs"))?;
        let facts = adapter.summarize_file(&root.join("src/lib.rs"), &text)?;

        assert!(!facts.functions.is_empty());
        assert!(!facts.tests.is_empty());
        assert!(!facts.probe_shapes.is_empty());
        Ok(())
    }

    #[test]
    fn ra_adapter_ignores_unannotated_helpers_in_test_files_and_keeps_rstest()
    -> Result<(), Box<dyn Error>> {
        let root = temp_dir("ra_test_helpers")?;
        fs::create_dir_all(root.join("tests"))?;
        write_manifest(&root)?;
        fs::write(
            root.join("tests/pipeline.rs"),
            r#"
fn helper() {
    run_pipeline();
}

#[rstest]
#[case("alpha")]
fn parameterized_case(input: &str) {
    helper();
    assert_eq!(input, "alpha");
}

#[test]
fn integration_smoke() {
    helper();
}
"#,
        )?;

        let adapter = RaRustSyntaxAdapter;
        let text = fs::read_to_string(root.join("tests/pipeline.rs"))?;
        let facts = adapter.summarize_file(&root.join("tests/pipeline.rs"), &text)?;
        let test_names = facts
            .tests
            .iter()
            .map(|test| test.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(facts.functions.len(), 3);
        assert_eq!(test_names, vec!["parameterized_case", "integration_smoke"]);
        assert!(
            facts
                .tests
                .iter()
                .find(|test| test.name == "parameterized_case")
                .is_some_and(|test| test.attrs.iter().any(|attr| attr.contains("rstest"))),
            "rstest attrs should remain available for value resolution"
        );
        Ok(())
    }

    #[test]
    fn ra_adapter_marks_inline_cfg_test_helpers_as_test_role() -> Result<(), Box<dyn Error>> {
        let root = temp_dir("ra_cfg_test_helpers")?;
        fs::create_dir_all(root.join("src"))?;
        write_manifest(&root)?;
        fs::write(
            root.join("src/lib.rs"),
            r#"
pub fn production_control(value: i32) -> Result<i32, String> {
    if value < 0 { return Err("negative".to_string()); }
    Ok(value)
}

#[cfg(test)]
mod tests {
    fn helper_returns_result(value: i32) -> Result<(), String> {
        if value < 0 { return Err("negative".to_string()); }
        Ok(())
    }

    #[test]
    fn helper_is_evidence() {
        helper_returns_result(1).expect("fixture should pass");
    }
}
"#,
        )?;

        let adapter = RaRustSyntaxAdapter;
        let text = fs::read_to_string(root.join("src/lib.rs"))?;
        let facts = adapter.summarize_file(&root.join("src/lib.rs"), &text)?;
        let helper = facts
            .functions
            .iter()
            .find(|function| function.name == "helper_returns_result")
            .ok_or("missing cfg(test) helper fact")?;

        assert!(
            helper.source_role == FunctionSourceRole::CfgTestModule,
            "cfg(test) helper must carry the producer-owned evidence-only role"
        );
        assert!(
            facts
                .tests
                .iter()
                .any(|test| test.name == "helper_is_evidence"),
            "actual test function remains available as evidence input"
        );
        assert!(
            facts
                .tests
                .iter()
                .all(|test| test.name != "helper_returns_result"),
            "cfg(test) helper must not be promoted to a test fact"
        );
        assert!(
            facts
                .functions
                .iter()
                .find(|function| function.name == "production_control")
                .is_some_and(|function| function.source_role == FunctionSourceRole::Production),
            "production control must remain production role"
        );
        Ok(())
    }

    #[test]
    fn ra_adapter_extracts_probe_shapes() -> Result<(), Box<dyn Error>> {
        let root = temp_dir("ra_probe_shapes")?;
        fs::create_dir_all(root.join("src"))?;
        write_manifest(&root)?;
        fs::write(
            root.join("src/lib.rs"),
            r#"
pub fn validate(value: i32) -> Result<i32, String> {
    if value < 0 {
        Err("negative".to_string())
    } else {
        Ok(value * 2)
    }
}
"#,
        )?;

        let adapter = RaRustSyntaxAdapter;
        let text = fs::read_to_string(root.join("src/lib.rs"))?;
        let facts = adapter.summarize_file(&root.join("src/lib.rs"), &text)?;

        assert!(
            facts
                .probe_shapes
                .iter()
                .any(|p| p.kind == PROBE_SHAPE_PREDICATE),
            "Should extract predicate probe shapes"
        );
        assert!(
            facts
                .probe_shapes
                .iter()
                .any(|p| p.kind == PROBE_SHAPE_ERROR_PATH),
            "Should extract error_path probe shapes"
        );
        Ok(())
    }

    #[test]
    fn ra_adapter_extracts_unsafe_execution_boundaries_without_lexical_false_positives()
    -> Result<(), Box<dyn Error>> {
        let root = temp_dir("ra_unsafe_boundaries")?;
        fs::create_dir_all(root.join("src"))?;
        write_manifest(&root)?;
        fs::write(
            root.join("src/lib.rs"),
            r#"pub unsafe fn read_raw_unchecked(ptr: *const u8) -> u8 {
    ptr.read()
}

pub fn read_raw(ptr: *const u8) -> u8 {
    let marker = "unsafe { not syntax }";
    // unsafe fn fake() {}
    unsafe {
        ptr.read()
    }
}
"#,
        )?;

        let adapter = RaRustSyntaxAdapter;
        let text = fs::read_to_string(root.join("src/lib.rs"))?;
        let facts = adapter.summarize_file(&root.join("src/lib.rs"), &text)?;
        let boundaries = facts
            .probe_shapes
            .iter()
            .filter(|shape| shape.kind == PROBE_SHAPE_UNSAFE_BOUNDARY)
            .map(|shape| (shape.text.clone(), shape.start_line, shape.end_line))
            .collect::<Vec<_>>();

        assert_eq!(
            boundaries,
            vec![
                ("unsafe fn read_raw_unchecked".to_string(), 1, 3),
                ("unsafe block".to_string(), 8, 10),
            ]
        );
        Ok(())
    }

    #[test]
    fn ra_adapter_does_not_promote_nested_constructor_arguments_to_returns()
    -> Result<(), Box<dyn Error>> {
        let root = temp_dir("ra_nested_constructor_returns")?;
        fs::create_dir_all(root.join("src"))?;
        write_manifest(&root)?;
        fs::write(
            root.join("src/lib.rs"),
            r#"
fn consume(_: Option<u64>) {}

pub fn wrap(value: u64) -> Result<Option<u64>, ()> {
    consume(Some(value));
    Ok(Some(value))
}
"#,
        )?;

        let adapter = RaRustSyntaxAdapter;
        let text = fs::read_to_string(root.join("src/lib.rs"))?;
        let facts = adapter.summarize_file(&root.join("src/lib.rs"), &text)?;
        let return_shapes = facts
            .probe_shapes
            .iter()
            .filter(|shape| shape.kind == PROBE_SHAPE_RETURN_VALUE)
            .map(|shape| shape.text.as_str())
            .collect::<Vec<_>>();

        if return_shapes != ["Ok(Some(value))"] {
            return Err(format!("unexpected return probe shapes: {return_shapes:?}").into());
        }
        Ok(())
    }

    #[test]
    fn ra_adapter_handles_parser_errors() -> Result<(), Box<dyn Error>> {
        let root = temp_dir("ra_parser_error")?;
        let adapter = RaRustSyntaxAdapter;
        let invalid_rust = "pub fn broken( { invalid rust";
        let result = adapter.summarize_file(&root.join("invalid.rs"), invalid_rust);

        assert!(matches!(result, Err(ref err) if err.contains("syntax errors")));
        Ok(())
    }

    #[test]
    fn ra_adapter_changed_nodes_returns_empty_for_missing_file() {
        let _index = crate::analysis::facts::RustIndex::default();
        let adapter = RaRustSyntaxAdapter;
        let ranges = vec![TextRange {
            start_line: 1,
            start_column: 1,
            end_line: 10,
            end_column: 80,
        }];

        let nodes = adapter.changed_nodes(
            &crate::analysis::facts::FileFacts {
                path: std::path::PathBuf::from("nonexistent.rs"),
                functions: vec![],
                tests: vec![],
                calls: vec![],
                returns: vec![],
                literals: vec![],
                probe_shapes: vec![],
                used_lexical_fallback: false,
                module_declarations: Vec::new(),
                role_provenance: Default::default(),
                source: String::new(),
            },
            &ranges,
        );

        assert!(nodes.is_empty());
    }
}

#[cfg(test)]
mod cfg_all_test_tests {
    use super::{RaRustSyntaxAdapter, RustSyntaxAdapter};
    use crate::analysis::facts::FunctionSourceRole;
    use crate::analysis::facts::cfg_predicates::{
        CfgTestRequirement, attribute_requires_test, classify_attribute,
    };
    use std::fs;
    use std::path::PathBuf;

    fn temp_dir(name: &str) -> Result<PathBuf, String> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("ripr-{name}-{stamp}"));
        std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
        Ok(dir)
    }

    fn write_manifest(root: &std::path::Path) -> Result<(), String> {
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]
name='test'
version='0.1.0'
edition='2024'
",
        )
        .map_err(|error| error.to_string())
    }

    #[test]
    fn cfg_test_predicate_is_order_insensitive_and_token_safe() {
        // The producer no longer carries its own token matching; this table
        // pins the shared cfg-predicate authority (#3530) at the spellings
        // this producer's modules present. Reverting the producer to a
        // drifted local matcher fails the nested-conjunct integration test
        // below.
        let requires_test = [
            "#[cfg(test)]",
            "#[ cfg ( test ) ]",
            "#[cfg(all(test, feature = \"slow\"))]",
            "#[cfg(all(feature = \"slow\", test))]",
            "#[cfg(all(unix, test, feature = r#\"slow,test\"#))]",
            "#[cfg(all(unix, all(test, feature = \"slow\")))]",
        ];
        for spelling in requires_test {
            assert!(
                attribute_requires_test(spelling),
                "top-level or nested test conjunct must require test cfg: {spelling}"
            );
        }

        let not_requires_test = [
            (
                "#[cfg(any(test, feature = \"slow\"))]",
                CfgTestRequirement::MayIncludeTest,
            ),
            ("#[cfg(not(test))]", CfgTestRequirement::IndependentOfTest),
            (
                "#[cfg(all(any(test, feature = \"slow\"), unix))]",
                CfgTestRequirement::MayIncludeTest,
            ),
            (
                "#[cfg(all(feature = \"slow,test\", unix))]",
                CfgTestRequirement::IndependentOfTest,
            ),
            (
                "#[cfg(target_os = \"test\")]",
                CfgTestRequirement::IndependentOfTest,
            ),
        ];
        for (spelling, expected) in not_requires_test {
            assert_eq!(
                classify_attribute(spelling),
                expected,
                "alternative, negated, nested, or literal test token must stay production: {spelling}"
            );
        }
    }

    #[test]
    fn cfg_all_test_module_members_carry_evidence_role() -> Result<(), String> {
        // A direct `test` conjunct makes the module harness-only regardless of
        // conjunct order. Trivia and literals cannot manufacture a conjunct;
        // alternatives, negation, and nested alternatives stay production.
        let root = temp_dir("ra_cfg_all_test")?;
        fs::create_dir_all(root.join("src")).map_err(|error| error.to_string())?;
        write_manifest(&root)?;
        fs::write(
            root.join("src/lib.rs"),
            "pub fn production_control() -> i32 { 1 }\n\n#[cfg(all(test, feature = \"slow\"))]\nmod test_first {\n    fn helper_test_first() -> i32 { production_control() }\n}\n\n#[cfg(all(feature = \"slow\", test))]\nmod test_second {\n    fn helper_test_second() -> i32 { production_control() }\n}\n\n#[cfg(all(feature = \"slow\" /* fake ,test, */, test))]\nmod commented_test {\n    fn helper_commented_test() -> i32 { production_control() }\n}\n\n#[cfg(any(test, feature = \"slow\"))]\nmod any_control {\n    pub fn any_shape() -> i32 { 2 }\n}\n\n#[cfg(all(any(test, feature = \"slow\"), unix))]\nmod nested_any_control {\n    pub fn nested_any_shape() -> i32 { 3 }\n}\n\n#[cfg(all(feature = \"slow,test\", unix))]\nmod string_control {\n    pub fn string_content_shape() -> i32 { 4 }\n}\n\n#[cfg(all(feature = r#\"slow,test\"#, unix))]\nmod raw_string_control {\n    pub fn raw_string_content_shape() -> i32 { 5 }\n}\n\n#[cfg(all(feature = \"slow\" /*,test,*/, unix))]\nmod comment_control {\n    pub fn comment_content_shape() -> i32 { 6 }\n}\n\n#[cfg(not(test))]\nmod prod_only {\n    pub fn production_shape() -> i32 { 7 }\n}\n",
        )
        .map_err(|error| error.to_string())?;
        let facts = RaRustSyntaxAdapter
            .summarize_file(
                &root.join("src/lib.rs"),
                &fs::read_to_string(root.join("src/lib.rs")).map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;

        for name in [
            "helper_test_first",
            "helper_test_second",
            "helper_commented_test",
        ] {
            let function = facts
                .functions
                .iter()
                .find(|function| function.name == name)
                .ok_or_else(|| format!("{name} missing from facts"))?;
            assert!(
                function.source_role == FunctionSourceRole::CfgTestModule,
                "{name} must carry the evidence-only cfg-test-module role"
            );
        }

        for name in [
            "any_shape",
            "nested_any_shape",
            "string_content_shape",
            "raw_string_content_shape",
            "comment_content_shape",
            "production_shape",
        ] {
            let function = facts
                .functions
                .iter()
                .find(|function| function.name == name)
                .ok_or_else(|| format!("{name} missing from facts"))?;
            assert!(
                !function.source_role.is_evidence_role(),
                "{name} must stay production role"
            );
        }
        Ok(())
    }

    #[test]
    fn cfg_nested_all_and_cfg_attr_predicates_follow_the_shared_authority() -> Result<(), String> {
        // #3530: the producer consumes `facts::cfg_predicates`, so a nested
        // `all(test, ...)` conjunction earns the evidence role (a drifted
        // local matcher without structural nesting would fail this), while
        // `cfg_attr` introductions never promote an item to test-only.
        let root = temp_dir("ra_cfg_predicates_authority")?;
        fs::create_dir_all(root.join("src")).map_err(|error| error.to_string())?;
        write_manifest(&root)?;
        let source = "\
pub fn production_control() -> i32 { 1 }

#[cfg(all(unix, all(test, feature = \"slow\")))]
mod nested_gate {
    fn nested_all_helper() -> i32 { production_control() }
}

#[cfg_attr(feature = \"internal-tests\", cfg(test))]
mod conditional_gate {
    fn cfg_attr_conditional_helper() -> i32 { production_control() }
}

#[cfg_attr(test, allow(dead_code))]
fn production_with_conditional_lint() -> i32 { 2 }
";
        fs::write(root.join("src/lib.rs"), source).map_err(|error| error.to_string())?;
        let facts = RaRustSyntaxAdapter
            .summarize_file(
                &root.join("src/lib.rs"),
                &fs::read_to_string(root.join("src/lib.rs")).map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;

        let nested_all_helper = facts
            .functions
            .iter()
            .find(|function| function.name == "nested_all_helper")
            .ok_or("nested_all_helper missing from facts")?;
        assert!(
            nested_all_helper.source_role == FunctionSourceRole::CfgTestModule,
            "a nested all(test, ...) conjunction must carry evidence role through the shared authority"
        );

        for name in [
            "cfg_attr_conditional_helper",
            "production_with_conditional_lint",
        ] {
            let function = facts
                .functions
                .iter()
                .find(|function| function.name == name)
                .ok_or_else(|| format!("{name} missing from facts"))?;
            assert!(
                !function.source_role.is_evidence_role(),
                "cfg_attr introductions must never promote {name} to test-only"
            );
        }
        assert!(
            facts.tests.is_empty(),
            "cfg-gated helpers stay evidence-role functions without executable TestFacts"
        );
        Ok(())
    }
}

#[cfg(test)]
mod guard_pipeline_debug_tests {
    use super::RaRustSyntaxAdapter;
    use super::RustSyntaxAdapter;

    #[test]
    fn parser_path_credits_err_guard_in_test_facts() -> Result<(), String> {
        let source = "use parity_err_guard::discounted_total;\n\n#[test]\nfn boundary_matches_expected() -> Result<(), String> {\n    let actual = discounted_total(100, 100);\n    let expected = 90;\n    if actual != expected {\n        return Err(format!(\"actual={actual:?}\"));\n    }\n    Ok(())\n}\n";
        let facts = RaRustSyntaxAdapter
            .summarize_file(std::path::Path::new("tests/pricing.rs"), source)
            .map_err(|error| error.to_string())?;
        let test = facts
            .tests
            .iter()
            .find(|test| test.name == "boundary_matches_expected")
            .ok_or_else(|| format!("test missing: {:?}", facts.tests))?;
        assert!(
            test.assertions
                .iter()
                .any(|oracle| oracle.kind == crate::domain::OracleKind::RelationalCheck),
            "guard must credit through the parser path: {:?}",
            test.assertions
        );
        Ok(())
    }

    #[test]
    fn parser_path_credits_guarded_routing_match_in_test_facts() -> Result<(), String> {
        // #3709: the guarded-routing form over a direct callee result —
        // guarded accept arm plus loud catch-all, no `Ok` arm, as in the
        // historical `expect_response` harness — must credit through the
        // parser path, and the line-scanned joiner must not also swallow
        // the same statement into a duplicate/mock-expectation fact.
        let source = concat!(
            "use routes_fixture::expect_ready;\n",
            "\n",
            "#[test]\n",
            "fn rejects_unready_kind() {\n",
            "    match expect_ready(\"busy\", 12) {\n",
            "        Err(error) if error.kind() == io::ErrorKind::InvalidData => {}\n",
            "        result => bail!(\"accepted: {result:?}\"),\n",
            "    }\n",
            "}\n",
        );
        let facts = RaRustSyntaxAdapter
            .summarize_file(std::path::Path::new("tests/routes.rs"), source)
            .map_err(|error| error.to_string())?;
        let test = facts
            .tests
            .iter()
            .find(|test| test.name == "rejects_unready_kind")
            .ok_or_else(|| format!("test missing: {:?}", facts.tests))?;
        let guarded: Vec<_> = test
            .assertions
            .iter()
            .filter(|oracle| oracle.kind == crate::domain::OracleKind::GuardedResultMatch)
            .collect();
        assert_eq!(
            guarded.len(),
            1,
            "exactly one guarded Result match oracle: {:?}",
            test.assertions
        );
        assert!(
            guarded[0].text.contains("match expect_ready(..)"),
            "the scrutinee callee is the binding token: {}",
            guarded[0].text
        );
        assert!(
            !test
                .assertions
                .iter()
                .any(|oracle| oracle.kind == crate::domain::OracleKind::MockExpectation),
            "the expect_-sniff joiner must not swallow the owned match: {:?}",
            test.assertions
        );
        Ok(())
    }

    /// #3727 Slice A: on the parser path the guarded-match shadow defeat
    /// derives from the parser-produced body facts. The `let` shadow sits
    /// BEFORE the match, so the positional rule defeats under either
    /// coordinate base; the base itself is pinned by
    /// `parser_path_shadow_facts_share_the_scan_text_coordinate_base`,
    /// which puts the binding after a compact match under the same
    /// attribute lines. The clean control above proves the same shape
    /// credits when no shadow exists.
    #[test]
    fn parser_path_shadow_defeats_guarded_match_through_facts() -> Result<(), String> {
        let source = concat!(
            "use routes_fixture::expect_ready;\n",
            "\n",
            "#[test]\n",
            "fn rejects_unready_kind() {\n",
            "    let expect_ready = build();\n",
            "    match expect_ready(\"busy\", 12) {\n",
            "        Err(error) if error.kind() == io::ErrorKind::InvalidData => {}\n",
            "        result => bail!(\"accepted: {result:?}\"),\n",
            "    }\n",
            "}\n",
        );
        let facts = RaRustSyntaxAdapter
            .summarize_file(std::path::Path::new("tests/routes.rs"), source)
            .map_err(|error| error.to_string())?;
        let test = facts
            .tests
            .iter()
            .find(|test| test.name == "rejects_unready_kind")
            .ok_or_else(|| format!("test missing: {:?}", facts.tests))?;
        assert!(
            test.let_bindings
                .iter()
                .any(|binding| binding.name == "expect_ready" && binding.line == 1),
            "the shadow binding fact is body-relative line 1: {:?}",
            test.let_bindings
        );
        assert!(
            !test
                .assertions
                .iter()
                .any(|oracle| oracle.kind == crate::domain::OracleKind::GuardedResultMatch),
            "a fact-shadowed bare scrutinee must not credit through the parser path: {:?}",
            test.assertions
        );
        Ok(())
    }

    /// #3739 review (coderabbit h6cfG): the guarded scan's use-site
    /// coordinate counts 0-based lines from the TOP of `function_text`
    /// (attributes included), so the parser facts fed to the shadow
    /// authority must share that base. The pre-fix fn-keyword base made a
    /// `let` AFTER a guarded match defeat that earlier oracle whenever
    /// attribute lines sat above the `fn` — one attribute line suffices
    /// when the match is compact enough for the binding to follow within
    /// the attribute count. The positional rule must keep crediting the
    /// match here; the sibling test above pins the let-BEFORE defeat.
    #[test]
    fn parser_path_shadow_facts_share_the_scan_text_coordinate_base() -> Result<(), String> {
        let source = concat!(
            "use routes_fixture::expect_ready;\n",
            "\n",
            "#[test]\n",
            "fn rejects_unready_kind() {\n",
            "    match expect_ready(\"busy\", 12) { Err(error) if error.kind() == io::ErrorKind::InvalidData => {} result => bail!(\"accepted: {result:?}\") }\n",
            "    let expect_ready = build();\n",
            "}\n",
        );
        let facts = RaRustSyntaxAdapter
            .summarize_file(std::path::Path::new("tests/routes.rs"), source)
            .map_err(|error| error.to_string())?;
        let test = facts
            .tests
            .iter()
            .find(|test| test.name == "rejects_unready_kind")
            .ok_or_else(|| format!("test missing: {:?}", facts.tests))?;
        assert!(
            test.let_bindings
                .iter()
                .any(|binding| binding.name == "expect_ready" && binding.line == 2),
            "the summarizer's binding fact is fn-relative line 2: {:?}",
            test.let_bindings
        );
        let guarded: Vec<_> = test
            .assertions
            .iter()
            .filter(|oracle| oracle.kind == crate::domain::OracleKind::GuardedResultMatch)
            .collect();
        assert_eq!(
            guarded.len(),
            1,
            "a binding AFTER the match must never suppress the earlier oracle \
             (shadow coordinates share the scan text's base): {:?}",
            test.assertions
        );
        Ok(())
    }
}

#[cfg(test)]
mod include_directive_tests {
    use super::rust_include_directives;
    use std::path::{Path, PathBuf};

    #[test]
    fn parser_extracts_only_real_include_macros_and_decodes_literals() -> Result<(), String> {
        let source = r###"
// include!("comment.rs");
const SAMPLE: &str = "include!(\"string.rs\")";
include!("plain.rs");
include!(r#"raw.rs"#);
include!("escaped\\path.rs");
include!("quoted\"file.rs");
include!(concat!(env!("OUT_DIR"), "/generated.rs"));
mod nested { include!("nested.rs"); }
"###;

        let directives = rust_include_directives(Path::new("src/lib.rs"), source, 16)?;
        let literals = directives
            .iter()
            .filter_map(|directive| directive.literal_path.clone())
            .collect::<Vec<_>>();

        assert_eq!(
            literals,
            vec![
                PathBuf::from("plain.rs"),
                PathBuf::from("raw.rs"),
                PathBuf::from("escaped\\path.rs"),
                PathBuf::from("quoted\"file.rs"),
                PathBuf::from("nested.rs")
            ]
        );
        assert_eq!(directives.len(), 6);
        assert!(directives[4].literal_path.is_none());
        assert!(!directives[5].is_file_level);
        Ok(())
    }
}

#[cfg(test)]
mod shadow_fact_equivalence_tests {
    //! #3727 Slice A: the parser-produced body facts must decide the shared
    //! shadow authority EXACTLY like the lexical scanners decide the same
    //! masked body, so switching consumers between the two authorities
    //! cannot move a verdict. The battery pins scanner equivalence over the
    //! #3722/#3728 pin families plus the positional and same-line rules.

    use super::*;
    use crate::analysis::extract::{
        ShadowAuthority, fact_body_defines_callee_fn, fact_body_let_shadow_line,
        mask_comments_and_strings, test_body_defines_callee_fn, test_body_let_shadow_line,
    };

    /// The callees every battery body is probed against: shadowing names,
    /// prefix-coincidence names, and absent names.
    const CALLEES: [&str; 5] = [
        "try_parse_summary",
        "expect_response",
        "parse_summary",
        "try_parse_summary_result",
        "score",
    ];

    fn parser_facts(source: &str) -> Result<(Vec<String>, Vec<LetBindingFact>), String> {
        let parse = SourceFile::parse(source, Edition::CURRENT);
        assert!(
            parse.errors().is_empty(),
            "battery source must parse cleanly: {source}"
        );
        let function = parse
            .tree()
            .syntax()
            .descendants()
            .find_map(ast::Fn::cast)
            .ok_or_else(|| format!("battery source carries no fn item: {source}"))?;
        let line_index = LineIndex::new(source);
        let fn_token = function
            .fn_token()
            .ok_or_else(|| format!("battery fn carries no fn token: {source}"))?;
        let fn_line = line_index.line(fn_token.text_range().start());
        Ok(collect_body_shadow_facts(
            &function,
            &|offset| line_index.line(offset),
            fn_line,
        ))
    }

    fn assert_scanner_equivalent(source: &str) -> Result<(), String> {
        let (nested_fn_names, let_bindings) = parser_facts(source)?;
        let masked = mask_comments_and_strings(source);
        for callee in CALLEES {
            assert_eq!(
                fact_body_defines_callee_fn(&nested_fn_names, callee),
                test_body_defines_callee_fn(&masked, callee),
                "fn-defeat verdicts must agree for `{callee}` in:\n{source}"
            );
            assert_eq!(
                fact_body_let_shadow_line(&let_bindings, callee),
                test_body_let_shadow_line(&masked, callee),
                "let-shadow verdicts must agree for `{callee}` in:\n{source}"
            );
        }
        Ok(())
    }

    #[test]
    fn battery_plain_fn_and_let_shadows_are_equivalent() -> Result<(), String> {
        assert_scanner_equivalent(
            "fn probe() {\n    fn try_parse_summary(raw: &str) -> usize { raw.len() }\n    let r = try_parse_summary(\"x\");\n}\n",
        )?;
        assert_scanner_equivalent(
            "fn probe() {\n    let result = parse_summary(\"@bad\");\n    let try_parse_summary = build();\n    let r = try_parse_summary;\n}\n",
        )?;
        assert_scanner_equivalent("fn probe() {\n    score(1);\n}\n")?;
        Ok(())
    }

    #[test]
    fn battery_let_mut_ref_typed_and_destructuring_are_equivalent() -> Result<(), String> {
        assert_scanner_equivalent(
            "fn probe() {\n    let mut expect_response = mock();\n    expect_response();\n}\n",
        )?;
        assert_scanner_equivalent(
            "fn probe() {\n    let ref parse_summary = owner();\n    let _ = parse_summary;\n}\n",
        )?;
        assert_scanner_equivalent(
            "fn probe() {\n    let try_parse_summary: Vec<u8> = build();\n    drop(try_parse_summary);\n}\n",
        )?;
        assert_scanner_equivalent(
            "fn probe() {\n    let (a, try_parse_summary) = pair();\n    drop((a, try_parse_summary));\n}\n",
        )?;
        assert_scanner_equivalent(
            "fn probe() {\n    let Summary { name, expect_response } = make();\n    drop((name, expect_response));\n}\n",
        )?;
        assert_scanner_equivalent(
            "fn probe() {\n    let Some(score) = opt() else { return };\n    drop(score);\n}\n",
        )?;
        Ok(())
    }

    #[test]
    fn battery_initializer_less_binding_is_equivalent() -> Result<(), String> {
        // #3728 round-5: `let flag;` has no initializer and is not a
        // shadow — the parser facts must skip it exactly like the lexical
        // `;` bound, and must NOT borrow a LATER binding's `=`.
        assert_scanner_equivalent(
            "fn probe() {\n    let flag;\n    flag = true;\n    let try_parse_summary = build();\n    drop(try_parse_summary);\n}\n",
        )?;
        assert_scanner_equivalent(
            "fn probe() {\n    let typed: expect_response;\n    drop(typed);\n}\n",
        )?;
        Ok(())
    }

    #[test]
    fn battery_string_comment_char_and_multiline_shapes_are_equivalent() -> Result<(), String> {
        // #3722/#3728 pins: shadow-shaped text inside strings, comments,
        // char literals, and multiline strings never defeats — the parser
        // facts never see those bytes as syntax at all.
        assert_scanner_equivalent(
            "fn probe() {\n    let text = \"let try_parse_summary = 1;\";\n    let note = \"fn expect_response() {}\";\n    real();\n}\n",
        )?;
        assert_scanner_equivalent(
            "fn probe() {\n    // fn try_parse_summary(raw: &str) {}\n    // let expect_response = build();\n    real();\n}\n",
        )?;
        assert_scanner_equivalent(
            "fn probe() {\n    /* let parse_summary = 1; */\n    real();\n}\n",
        )?;
        assert_scanner_equivalent(
            "fn probe() {\n    let quote = '\"';\n    fn try_parse_summary() {}\n    try_parse_summary();\n}\n",
        )?;
        assert_scanner_equivalent(
            "fn probe() {\n    let doc = \"let score = 1;\nfn parse_summary() {}\";\n    real();\n}\n",
        )?;
        Ok(())
    }

    #[test]
    fn battery_positional_and_same_line_rules_are_equivalent() -> Result<(), String> {
        // A call BEFORE the binding precedes the shadow: no defeat in
        // either authority.
        assert_scanner_equivalent(
            "fn probe() {\n    let r = try_parse_summary(\"x\");\n    let try_parse_summary = build();\n    drop(r);\n}\n",
        )?;
        // Same-line conservative defeat: the binding and the use share a
        // line, and both authorities defeat.
        assert_scanner_equivalent(
            "fn probe() {\n    let try_parse_summary = build(); let r = try_parse_summary();\n}\n",
        )?;
        // A use BEFORE the binding line is not defeated; one at or after
        // it is.
        assert_scanner_equivalent(
            "fn probe() {\n    let r = expect_response(&mut cursor, 12);\n    let expect_response = build();\n    drop((r, expect_response));\n}\n",
        )?;
        Ok(())
    }

    #[test]
    fn battery_nested_fns_and_nested_blocks_are_equivalent() -> Result<(), String> {
        assert_scanner_equivalent(
            "fn probe() {\n    fn outer() {\n        fn try_parse_summary() {}\n    }\n    try_parse_summary();\n}\n",
        )?;
        assert_scanner_equivalent(
            "fn probe() {\n    if ready {\n        let expect_response = build();\n        expect_response();\n    }\n}\n",
        )?;
        assert_scanner_equivalent(
            "fn probe() {\n    let closure = |raw: &str| {\n        let parse_summary = raw.len();\n        parse_summary\n    };\n    drop(closure);\n}\n",
        )?;
        Ok(())
    }

    #[test]
    fn battery_prefix_coincidence_names_are_equivalent() -> Result<(), String> {
        // `let try_parse_summary_result = ..` is a DIFFERENT binding; a
        // `fn try_parse_summary_result` is a DIFFERENT item. Neither
        // defeats a `try_parse_summary` callee in either authority.
        assert_scanner_equivalent(
            "fn probe() {\n    let try_parse_summary_result = build();\n    let r = real();\n    drop((try_parse_summary_result, r));\n}\n",
        )?;
        assert_scanner_equivalent(
            "fn probe() {\n    fn try_parse_summary_result() {}\n    real();\n}\n",
        )?;
        assert_scanner_equivalent(
            "fn probe() {\n    fn score_board() {}\n    let scoring = 1;\n    real(scoring);\n}\n",
        )?;
        Ok(())
    }

    /// Removal control at the producer boundary (#3727 acceptance): a
    /// shadow-bearing body MUST produce non-empty facts on the parser
    /// path. A producer regression that silently stops populating the
    /// fields fails here instead of quietly letting parser-backed
    /// consumers read "no shadow" and over-credit.
    #[test]
    fn producer_populates_shadow_facts_for_shadow_bearing_bodies() -> Result<(), String> {
        let (nested, bindings) = parser_facts(
            "fn probe() {\n    fn try_parse_summary() {}\n    let expect_response = build();\n    drop(expect_response);\n}\n",
        )?;
        assert!(nested.contains(&"try_parse_summary".to_string()));
        assert!(
            bindings
                .iter()
                .any(|binding| binding.name == "expect_response" && binding.line == 2)
        );
        Ok(())
    }

    /// #3739 review (devin h0fZ1, codex h6Z9y, gemini h6ZRp, coderabbit
    /// h6cfM): the synthetic wrapper in [`shadow_facts_for_body_text`] must
    /// CLOSE the fn item it opens. The pre-fix wrapper left the brace
    /// unterminated, `SourceFile::parse` reported errors on every input,
    /// and the fail-closed branch returned empty facts for every harness
    /// body — parser-backed harness subjects then read "no shadow" and a
    /// closure-local binding could impersonate the wrapper seam's callee.
    /// These controls fail empty: a harness-style closure body with a
    /// shadow-bearing `let` and a nested `fn` must produce the non-empty
    /// facts its consumers' flag law reads.
    #[test]
    fn body_text_wrapper_produces_non_empty_facts_for_harness_bodies() {
        let (nested, bindings) = shadow_facts_for_body_text(
            "|| {\n    let try_parse_summary = fake;\n    try_parse_summary(input)\n}",
        );
        assert!(
            bindings
                .iter()
                .any(|binding| binding.name == "try_parse_summary" && binding.line == 1),
            "the harness body's shadow-bearing let must become a fact: {bindings:?}"
        );
        assert!(nested.is_empty(), "a closure body defines no fn items");

        let (nested, bindings) = shadow_facts_for_body_text("{\n    fn fake_helper() {}\n}");
        assert!(
            nested.contains(&"fake_helper".to_string()),
            "a nested fn item in a harness body must become a fact: {nested:?}"
        );
        assert!(bindings.is_empty(), "no let bindings in this body");
    }

    /// The stored `TestFact` fields are body-relative to the `fn` keyword
    /// line — the same base `test.body` gives the lexical scanners — so a
    /// parser-backed summary and a lexical scan of the same body agree
    /// through the real summarizer, not just the descent helper.
    #[test]
    fn summarizer_facts_decide_like_the_lexical_scanners_on_test_body() -> Result<(), String> {
        let source = concat!(
            "use fixture::try_parse_summary;\n",
            "\n",
            "#[test]\n",
            "fn misc_edge_case() {\n",
            "    let try_parse_summary = build();\n",
            "    let r = try_parse_summary;\n",
            "}\n",
        );
        let facts = summarize_file_with_parser(std::path::Path::new("tests/fixture.rs"), source)
            .map_err(|error| error.to_string())?;
        let test = facts
            .tests
            .iter()
            .find(|test| test.name == "misc_edge_case")
            .ok_or_else(|| format!("test fact missing: {:?}", facts.tests))?;
        let masked = mask_comments_and_strings(&test.body);
        for callee in CALLEES {
            assert_eq!(
                fact_body_defines_callee_fn(&test.nested_fn_names, callee),
                test_body_defines_callee_fn(&masked, callee),
                "stored facts must equal the lexical scan of `test.body` for `{callee}`"
            );
            assert_eq!(
                fact_body_let_shadow_line(&test.let_bindings, callee),
                test_body_let_shadow_line(&masked, callee),
                "stored let facts must equal the lexical scan of `test.body` for `{callee}`"
            );
        }
        assert!(
            test.let_bindings
                .iter()
                .any(|binding| binding.name == "try_parse_summary" && binding.line == 1),
            "the binding sits on body-relative line 1: {:?}",
            test.let_bindings
        );
        Ok(())
    }

    /// Consumer-side equivalence for the guarded-match scanner: driving
    /// `guarded_result_match_scan_with_shadow_authority` with the parser
    /// facts of a body must emit exactly what the lexical entry point
    /// emits on the same body.
    #[test]
    fn guarded_match_scan_with_parser_facts_matches_lexical_scan() -> Result<(), String> {
        let bodies = [
            // Clean guarded match: credited through both authorities.
            "fn probe() {\n    match expect_response(\"busy\", 12) {\n        Ok(v) => assert_eq!(v, 12),\n        Err(e) => panic!(\"{e}\"),\n    }\n}\n",
            // Let-shadowed callee: defeated through both authorities.
            "fn probe() {\n    let expect_response = build();\n    match expect_response(\"busy\", 12) {\n        Ok(v) => assert_eq!(v, 12),\n        Err(e) => panic!(\"{e}\"),\n    }\n}\n",
            // fn-shadowed callee: defeated through both authorities.
            "fn probe() {\n    fn expect_response(a: &str, b: u8) -> Result<u8, String> { Ok(b) }\n    match expect_response(\"busy\", 12) {\n        Ok(v) => assert_eq!(v, 12),\n        Err(e) => panic!(\"{e}\"),\n    }\n}\n",
            // Shadow-shaped text in a string never defeats in either.
            "fn probe() {\n    let note = \"let expect_response = build();\";\n    match expect_response(\"busy\", 12) {\n        Ok(v) => assert_eq!(v, 12),\n        Err(e) => panic!(\"{e}\"),\n    }\n}\n",
        ];
        for body in bodies {
            let (nested, bindings) = parser_facts(body)?;
            let lexical = guarded_result_match_scan_with_shadow_authority(
                body,
                1,
                ShadowAuthority::LexicalMaskedBody,
            );
            let parser_backed = guarded_result_match_scan_with_shadow_authority(
                body,
                1,
                ShadowAuthority::ParserBodyFacts {
                    nested_fn_names: &nested,
                    let_bindings: &bindings,
                },
            );
            assert_eq!(
                lexical.oracles.len(),
                parser_backed.oracles.len(),
                "oracle counts must agree for:\n{body}"
            );
            for (left, right) in lexical.oracles.iter().zip(parser_backed.oracles.iter()) {
                assert_eq!(left.line, right.line);
                assert_eq!(left.text, right.text);
                assert_eq!(left.kind, right.kind);
            }
            assert_eq!(
                lexical.match_start_lines, parser_backed.match_start_lines,
                "owned-statement lines must agree for:\n{body}"
            );
        }
        Ok(())
    }
}
