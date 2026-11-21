//! Bounded changed-binding → predicate-operand relation (#3294, P2 of
//! #3215).
//!
//! A diff that changes a `let` initializer changes the value later
//! predicates branch on, but the lexical probe scanner cannot see that
//! link: a changed `let end = …;` line lands in the catch-all
//! static-unknown family with a generic "changed syntax is not mapped"
//! finding even when the same function contains the exact `end == start`
//! predicate the tests discriminate.
//!
//! This module resolves that link producer-side, under narrow rules:
//!
//! - one simple `let <ident> = <init>;` line (no destructuring, no
//!   pattern);
//! - uses searched only within the owning function's body (sibling
//!   functions never relate);
//! - the binding must reach the use without a re-binding (shadowing,
//!   including a destructuring re-bind) or a reassignment between the
//!   changed declaration and the use — any such event ends the live
//!   span at that point;
//! - the use must be a direct identifier operand of a comparison
//!   (`==`, `!=`, `<`, `<=`, `>`, `>=`), a direct boolean test
//!   (`if ident`, `while ident`), or a `match` scrutinee, with comment
//!   and string text masked;
//! - a use line that is a macro invocation or carries closure pipes
//!   fails closed: it is recorded as the explicit blocker, never
//!   related.
//!
//! The relation never evaluates the initializer. It names the earliest
//! unsupported initializer operation when one is present, so the finding
//! can state the exact predicate and the exact unresolved edge instead
//! of a generic syntax limitation.

use crate::analysis::language::{changed_let_binding, mask_rust_comments_and_strings};

/// Which operand position the binding occupies at the predicate use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PredicateOperandSide {
    Left,
    Right,
    /// Direct boolean test (`if ident`, `while ident`).
    Boolean,
    /// Direct `match` scrutinee.
    MatchScrutinee,
}

/// Why the relation did not reach a direct use. Retained on the typed
/// relation (and asserted in tests) so the scope decision is explicit
/// instead of a silent drop (#3294: shadowing, closure capture,
/// mutation, and macro ambiguity are named limitations).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum BindingUseScope {
    /// A later `let` re-bound the identifier before any predicate use.
    ShadowedBeforeUse,
    /// The identifier was reassigned between the changed binding and
    /// the predicate use.
    ReassignedBeforeUse,
    /// The only candidate use sits inside a closure body; ripr does not
    /// model capture or call timing.
    ClosureCaptureUnsupported,
    /// The only candidate use sits inside a macro invocation; ripr does
    /// not expand macros.
    MacroExpansionUnsupported,
    /// The changed declaration's block ended before any predicate use;
    /// a later same-named binding is a different binding.
    ScopeExitedBeforeUse,
    /// No direct predicate use of the binding exists in the function.
    NoPredicateUse,
}

/// Whether the changed initializer's operand value can be named without
/// evaluating anything. A bare literal or identifier copy is textual;
/// anything else is unresolved and names its earliest operation (the
/// leftmost operation token in the initializer).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum BindingValueResolution {
    ResolvedToText(String),
    Unresolved { earliest_operation: String },
}

/// One direct `changed_binding -> predicate_operand` use (#3294).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ChangedBindingPredicateUse {
    pub(crate) binding: String,
    /// The changed initializer text (the `…` of `let ident = …;`).
    pub(crate) initializer: String,
    /// The exact predicate line text at the use site.
    pub(crate) predicate_expression: String,
    /// Absolute (new-file) line of the predicate use.
    pub(crate) predicate_line: usize,
    pub(crate) operand_side: PredicateOperandSide,
    pub(crate) value_resolution: BindingValueResolution,
}

/// The relation outcome for one changed binding: every direct use that
/// survives the scope rules, or the first blocker when none does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum BindingPredicateResolution {
    DirectUses(Vec<ChangedBindingPredicateUse>),
    NoDirectUse { scope: BindingUseScope },
}

/// Comparison operators the relation supports. Two-character operators
/// must be checked before their one-character prefixes.
const COMPARISON_OPERATORS: [&str; 6] = ["==", "!=", "<=", ">=", "<", ">"];

/// Resolve the relation for a changed `let` line against the owning
/// function body. `body_start_line` is the function's absolute start
/// line (body line offset 0 sits on it), and `changed_line` is the
/// changed declaration's absolute line: only uses after that line, with
/// no intervening shadow or reassignment, relate.
///
/// Region tracking keeps the scan inside the owning function's own
/// scope (review-hardened #3294): braces opened by a closure or a
/// nested item are no-use regions (their bodies are separate binding
/// scopes or unmodeled capture), and lines inside an unclosed foreign
/// expression (running parentheses from a previous line) are
/// continuations, not use sites.
pub(crate) fn resolve_changed_binding_uses(
    binding: &str,
    initializer: &str,
    body: &str,
    body_start_line: usize,
    changed_line: usize,
) -> BindingPredicateResolution {
    let masked = mask_rust_comments_and_strings(body);
    let mut uses = Vec::new();
    let mut live = false;
    let mut blocker: Option<BindingUseScope> = None;
    let mut brace_regions: Vec<bool> = Vec::new();
    let mut open_parens: isize = 0;
    let mut pending_item_signature = false;
    // The brace depth at the changed declaration: when the scan leaves
    // that depth, the declaration's scope has ended and the binding no
    // longer exists — a same-named outer binding's later uses belong to
    // that outer binding, never to this one (#3294 review).
    let mut declaration_depth: Option<usize> = None;
    for (offset, (raw_line, masked_line)) in body.lines().zip(masked.lines()).enumerate() {
        let absolute = body_start_line + offset;
        let masked_trimmed = masked_line.trim();
        let leading_closes = masked_trimmed.chars().take_while(|ch| *ch == '}').count();
        for _ in 0..leading_closes {
            brace_regions.pop();
        }
        if live && declaration_depth.is_some_and(|depth| brace_regions.len() < depth) {
            live = false;
            // The function's own closing brace empties the region
            // stack — that is the end of the scan, not a scope event.
            if !brace_regions.is_empty() {
                blocker.get_or_insert(BindingUseScope::ScopeExitedBeforeUse);
            }
        }
        let opens = masked_line.matches('{').count();
        let closes = masked_line.matches('}').count();
        let in_continuation = open_parens > 0;
        let inside_no_use = brace_regions.iter().any(|no_use| *no_use);

        // A nested item's multi-line signature: skip until its body
        // brace opens (a no-use region) or a `;` ends the bodyless
        // item.
        if pending_item_signature {
            let opens_here = masked_line.contains('{');
            reconcile_braces(&mut brace_regions, opens, closes, leading_closes, true);
            if opens_here || masked_trimmed.ends_with(';') {
                pending_item_signature = false;
            }
            open_parens = (open_parens + paren_delta(masked_line)).max(0);
            continue;
        }
        // Nested items (and the owner's own signature line) are never
        // use sites; a nested item's parameters and locals are a
        // separate binding scope, so its whole body is a no-use region.
        // The owner's own signature (offset 0) opens the body the scan
        // lives in, so its brace stays a plain region.
        if is_item_line(masked_trimmed) {
            let item_no_use = offset > 0;
            if !masked_line.contains('{') && !masked_trimmed.ends_with(';') {
                pending_item_signature = true;
            }
            reconcile_braces(
                &mut brace_regions,
                opens,
                closes,
                leading_closes,
                item_no_use,
            );
            open_parens = (open_parens + paren_delta(masked_line)).max(0);
            continue;
        }
        if !inside_no_use {
            if masked_trimmed.starts_with("let ") {
                if let Some((declared, _)) = changed_let_binding(masked_trimmed) {
                    if declared == binding {
                        if absolute == changed_line {
                            live = true;
                            declaration_depth = Some(brace_regions.len());
                        } else if live {
                            live = false;
                            blocker.get_or_insert(BindingUseScope::ShadowedBeforeUse);
                        }
                    } else if live && direct_predicate_operand(masked_trimmed, binding).is_some() {
                        // Another binding's `let` initializer may still
                        // carry a closure capture or macro use of ours.
                        // V1 relates no predicate inside a foreign
                        // initializer, but the closure/macro ambiguity
                        // is recorded as the explicit blocker rather
                        // than silently dropped.
                        if has_closure_pipes(masked_trimmed) {
                            blocker.get_or_insert(BindingUseScope::ClosureCaptureUnsupported);
                        } else if is_macro_invocation_line(masked_trimmed) {
                            blocker.get_or_insert(BindingUseScope::MacroExpansionUnsupported);
                        }
                    }
                    reconcile_braces(
                        &mut brace_regions,
                        opens,
                        closes,
                        leading_closes,
                        has_closure_pipes(masked_trimmed),
                    );
                    open_parens = (open_parens + paren_delta(masked_line)).max(0);
                    continue;
                }
                // Non-simple `let` (destructuring or pattern): it can
                // still re-bind the identifier, which ends the live
                // span.
                if live && pattern_rebinds_binding(masked_trimmed, binding) {
                    live = false;
                    blocker.get_or_insert(BindingUseScope::ShadowedBeforeUse);
                }
                reconcile_braces(
                    &mut brace_regions,
                    opens,
                    closes,
                    leading_closes,
                    has_closure_pipes(masked_trimmed),
                );
                open_parens = (open_parens + paren_delta(masked_line)).max(0);
                continue;
            }
            // `if let`/`while let`/`for` bind their own pattern
            // variable: a same-named binding there is a re-bind, and
            // the control line is never a predicate use of ours.
            if live && pattern_control_rebinds(masked_trimmed, binding) {
                live = false;
                blocker.get_or_insert(BindingUseScope::ShadowedBeforeUse);
            } else if live && is_reassignment(masked_trimmed, binding) {
                live = false;
                blocker.get_or_insert(BindingUseScope::ReassignedBeforeUse);
            } else if live && !in_continuation {
                let side = direct_predicate_operand(masked_trimmed, binding);
                if let Some(side) = side {
                    if is_macro_invocation_line(masked_trimmed) {
                        blocker.get_or_insert(BindingUseScope::MacroExpansionUnsupported);
                    } else if has_closure_pipes(masked_trimmed) {
                        blocker.get_or_insert(BindingUseScope::ClosureCaptureUnsupported);
                    } else {
                        uses.push(ChangedBindingPredicateUse {
                            binding: binding.to_string(),
                            initializer: initializer.to_string(),
                            predicate_expression: raw_line.trim().to_string(),
                            predicate_line: absolute,
                            operand_side: side,
                            value_resolution: value_resolution(initializer),
                        });
                    }
                }
            }
        }
        reconcile_braces(
            &mut brace_regions,
            opens,
            closes,
            leading_closes,
            has_closure_pipes(masked_line),
        );
        open_parens = (open_parens + paren_delta(masked_line)).max(0);
    }
    if uses.is_empty() {
        BindingPredicateResolution::NoDirectUse {
            scope: blocker.unwrap_or(BindingUseScope::NoPredicateUse),
        }
    } else {
        BindingPredicateResolution::DirectUses(uses)
    }
}

/// Apply a line's net brace effect to the region stack. Braces the line
/// opens become no-use regions when `no_use` is set (closure bodies,
/// item bodies); braces it closes beyond any already-popped leading
/// closes pop from the stack.
fn reconcile_braces(
    brace_regions: &mut Vec<bool>,
    opens: usize,
    closes: usize,
    leading_closes: usize,
    no_use: bool,
) {
    let net = opens as isize - closes as isize + leading_closes as isize;
    if net > 0 {
        for _ in 0..net {
            brace_regions.push(no_use);
        }
    } else {
        for _ in 0..(-net).min(brace_regions.len() as isize) {
            brace_regions.pop();
        }
    }
}

fn paren_delta(line: &str) -> isize {
    line.matches('(').count() as isize - line.matches(')').count() as isize
}

/// Net parenthesis balance of a changed `let` line with comment/string
/// text masked, so a literal `"("` cannot fake balance. A non-zero
/// delta means the declaration continues on the next line.
pub(crate) fn masked_paren_delta(line: &str) -> isize {
    paren_delta(&mask_rust_comments_and_strings(line))
}

/// Net brace balance of a changed `let` line with comment/string text
/// masked. A non-zero delta means the initializer's block continues on
/// the next line.
pub(crate) fn masked_brace_delta(line: &str) -> isize {
    let masked = mask_rust_comments_and_strings(line);
    masked.matches('{').count() as isize - masked.matches('}').count() as isize
}

/// A nested (or the owner's own) item line: separate binding scope,
/// never a use site.
fn is_item_line(line: &str) -> bool {
    [
        "fn ",
        "pub ",
        "pub(",
        "async ",
        "impl ",
        "struct ",
        "enum ",
        "trait ",
        "mod ",
        "const ",
        "static ",
        "macro_rules!",
        "unsafe ",
        "extern ",
    ]
    .iter()
    .any(|prefix| line.starts_with(prefix))
}

/// A non-simple `let` line whose binding pattern mentions the
/// identifier (e.g. `let (end, other) = pair;`) re-binds it.
fn pattern_rebinds_binding(let_line: &str, binding: &str) -> bool {
    let Some((lhs, _)) = let_line.split_once('=') else {
        return false;
    };
    lhs.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .any(|token| token == binding)
}

/// An `if let`/`while let`/`for` line whose pattern binds the
/// identifier (e.g. `if let Some(end) = opt {`, `for end in items {`)
/// re-binds it; the control line is a pattern, never a predicate use.
fn pattern_control_rebinds(line: &str, binding: &str) -> bool {
    // Normalize the rustfmt shape `} else if let …` before matching.
    let line = strip_leading_closing_and_else(line);
    let pattern = if let Some(rest) = line
        .strip_prefix("if let ")
        .or_else(|| line.strip_prefix("while let "))
    {
        rest.split_once('=').map_or(rest, |(pattern, _)| pattern)
    } else if let Some(rest) = line.strip_prefix("for ") {
        rest.split_once(" in ").map_or(rest, |(pattern, _)| pattern)
    } else {
        return false;
    };
    pattern
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .any(|token| token == binding)
}

/// `<ident> = …` / `<ident> += …` (and friends) on a live binding —
/// but never `==`, `!=`, `<=`, `>=`, or a `let`.
fn is_reassignment(line: &str, binding: &str) -> bool {
    for operator in ["+=", "-=", "*=", "/=", "%=", "|=", "&=", "^=", "<<=", ">>="] {
        if let Some((lhs, _)) = line.split_once(operator)
            && binding_is_plain_operand(lhs, binding)
        {
            return true;
        }
    }
    if let Some((lhs, rhs)) = line.split_once('=') {
        return binding_is_plain_operand(lhs, binding) && !rhs.trim_start().starts_with('=');
    }
    false
}

/// Whether `text` ends with `binding` as a standalone identifier (not a
/// field path like `self.end`, not a longer identifier).
fn binding_is_plain_operand(text: &str, binding: &str) -> bool {
    let trimmed = text.trim_end();
    let Some(rest) = trimmed.strip_suffix(binding) else {
        return false;
    };
    match rest.chars().next_back() {
        None => true,
        Some(ch) => !(ch == '_' || ch.is_ascii_alphanumeric() || ch == '.'),
    }
}

/// Whether `binding` is a **direct identifier operand** of a supported
/// comparison, boolean test, or `match` scrutinee on this (masked)
/// line. Direct means the operand position holds exactly the binding,
/// optionally behind `&`/`*` — never a method receiver
/// (`end.is_empty()`), a call or index interior (`map.get(&end)`), a
/// composite expression (`end + 1`), a field path (`self.end`), or a
/// longer identifier (`endpoint`).
fn direct_predicate_operand(line: &str, binding: &str) -> Option<PredicateOperandSide> {
    let base = strip_leading_closing_and_else(line);
    let is_boolean_control = ["if ", "while "]
        .iter()
        .any(|prefix| base.starts_with(prefix));
    let is_match_control = base.starts_with("match ");
    let head = strip_control_keywords(base);
    for operator in COMPARISON_OPERATORS {
        let mut search = 0;
        while let Some(offset) = head[search..].find(operator) {
            let at = search + offset;
            // A `<`/`>` that is half of a shift operator is not a
            // comparison: `sink(flags << end)` must never read as a
            // predicate use of `end` (#3294 review).
            if is_shift_operator_half(head, at, operator) {
                search = at + operator.len();
                continue;
            }
            // An operator inside unclosed parentheses/brackets sits in
            // a call or index argument, not at statement level.
            if has_unclosed_group_before(&head[..at]) {
                search = at + operator.len();
                continue;
            }
            let left = operand_before(&head[..at]);
            let right = operand_after(&head[at + operator.len()..]);
            if operand_is_binding(left, binding) {
                return Some(PredicateOperandSide::Left);
            }
            if operand_is_binding(right, binding) {
                return Some(PredicateOperandSide::Right);
            }
            search = at + operator.len();
        }
    }
    if is_boolean_control {
        let condition_text = head.split('{').next().unwrap_or(head);
        // `if let`/`while let` conditions bind a pattern, they do
        // not test our binding (pattern re-binds are handled by
        // `pattern_control_rebinds`); otherwise the binding must be
        // a whole `&&`/`||` term of the condition.
        if !condition_text.trim_start().starts_with("let ")
            && condition_terms(condition_text).any(|term| operand_is_binding(Some(term), binding))
        {
            return Some(PredicateOperandSide::Boolean);
        }
    }
    if is_match_control {
        let scrutinee = head.split('{').next().unwrap_or(head);
        if operand_is_binding(Some(scrutinee), binding) {
            return Some(PredicateOperandSide::MatchScrutinee);
        }
    }
    None
}

/// Strip leading closing braces and `else` so `} else if …` shapes read
/// as their control form.
fn strip_leading_closing_and_else(line: &str) -> &str {
    let mut base = line.trim_start();
    loop {
        let next = base
            .strip_prefix('}')
            .map(str::trim_start)
            .or_else(|| base.strip_prefix("else ").map(str::trim_start));
        match next {
            Some(stripped) if stripped != base => base = stripped,
            _ => return base,
        }
    }
}

/// Strip a leading control keyword (`if `, `while `, `match `) so the
/// operand scan sees the statement's expression head.
fn strip_control_keywords(base: &str) -> &str {
    ["if ", "while ", "match "]
        .iter()
        .find_map(|prefix| base.strip_prefix(prefix))
        .unwrap_or(base)
}

/// Whether the text before an operator leaves a `(`/`[` unclosed at the
/// operator itself: the operator sits inside a call or index argument.
/// A fully closed group before the operator is fine — the operator is
/// still at statement level.
fn has_unclosed_group_before(text: &str) -> bool {
    let mut depth = 0isize;
    for character in text.chars() {
        match character {
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            _ => {}
        }
    }
    depth > 0
}

/// The operand text immediately left of an operator: the segment since
/// the nearest `(`/`[`/`{`/`,` boundary. `None` when that boundary was
/// a group opener or the segment still holds a group character — both
/// mean the operand position is a call/index interior or a composite.
fn operand_before(text: &str) -> Option<&str> {
    let after_and = text.rsplit("&&").next().unwrap_or(text);
    let after_or = after_and.rsplit("||").next().unwrap_or(after_and);
    let boundary = after_or
        .rfind(['(', '[', '{', ','])
        .map(|at| (at, after_or.as_bytes()[at] as char));
    let segment = match boundary {
        Some((_, '(' | '[')) => return None,
        Some((at, _)) => &after_or[at + 1..],
        None => after_or,
    };
    if segment.contains(['(', ')', '[', ']', ',']) {
        return None;
    }
    Some(segment)
}

/// The operand text immediately right of an operator: the segment up to
/// the nearest `(`/`[`/`)`/`]`/`{`/`,` boundary. `None` when the segment
/// holds a group character — the operand is a call/index or composite.
fn operand_after(text: &str) -> Option<&str> {
    let before_and = text.split("&&").next().unwrap_or(text);
    let before_or = before_and.split("||").next().unwrap_or(before_and);
    let cut = before_or
        .find(['(', '[', ')', ']', '{', ','])
        .unwrap_or(before_or.len());
    let segment = &before_or[..cut];
    if segment.contains(['(', ')', '[', ']', ',']) {
        return None;
    }
    Some(segment)
}

/// The `&&`/`||`-separated terms of a boolean condition.
fn condition_terms(condition: &str) -> impl Iterator<Item = &str> {
    condition
        .split("&&")
        .flat_map(|part| part.split("||"))
        .map(str::trim)
}

/// Whether an operand segment is exactly `binding`, behind any number
/// of `&`/`*` references and dereferences.
fn operand_is_binding(operand: Option<&str>, binding: &str) -> bool {
    let Some(operand) = operand else {
        return false;
    };
    let mut operand = operand.trim();
    loop {
        let stripped = operand
            .strip_prefix('&')
            .or_else(|| operand.strip_prefix('*'))
            .map(str::trim_start);
        match stripped {
            Some(next) => operand = next,
            None => break,
        }
    }
    operand == binding
}

/// Whether the `<`/`>` at `at` is one character of a `<<`/`>>` shift
/// operator rather than a comparison.
fn is_shift_operator_half(line: &str, at: usize, operator: &str) -> bool {
    if operator != "<" && operator != ">" {
        return false;
    }
    let before = line[..at].chars().next_back();
    let after = line[at + 1..].chars().next();
    matches!(before, Some('<') | Some('>')) || matches!(after, Some('<') | Some('>'))
}

/// A use line that invokes a macro (`ensure!(…)`, `debug_assert!(…)`):
/// ripr does not expand macros, so the use fails closed. `!(`
/// negation is not a macro — the `!` must follow an identifier.
fn is_macro_invocation_line(line: &str) -> bool {
    line.char_indices().any(|(at, character)| {
        character == '!'
            && line[at + 1..].starts_with('(')
            && line[..at]
                .chars()
                .next_back()
                .is_some_and(|prev| prev.is_ascii_alphanumeric() || prev == '_')
    })
}

/// A use line carrying closure pipes (`|x| …`, `move |x|`): the capture
/// and call timing are not modeled, so the use fails closed. `||` in a
/// boolean condition is explicitly not a closure pipe, and a single `|`
/// between operands is a bitwise OR, not a pipe — a pipe counts only in
/// an argument position (line start, or after `=`, `(`, `,`, `;`, `{`).
fn has_closure_pipes(line: &str) -> bool {
    let without_boolean_ops = line.replace("||", "");
    without_boolean_ops.char_indices().any(|(at, character)| {
        if character != '|' {
            return false;
        }
        let before = without_boolean_ops[..at].trim_end().chars().next_back();
        matches!(
            before,
            None | Some('=') | Some('(') | Some(',') | Some(';') | Some('{')
        )
    })
}

/// The earliest (leftmost) unresolved operation in the initializer, or
/// `None` when the initializer is a bare literal/identifier copy. Any
/// call or arithmetic is unresolved — ripr never pretends to know an
/// operand value it did not evaluate. Operation characters inside a
/// string literal never count: the scan runs over masked text.
fn value_resolution(initializer: &str) -> BindingValueResolution {
    let trimmed = initializer.trim();
    let masked = mask_rust_comments_and_strings(trimmed);
    match earliest_operation(&masked) {
        Some(earliest_operation) => BindingValueResolution::Unresolved { earliest_operation },
        None => BindingValueResolution::ResolvedToText(trimmed.to_string()),
    }
}

fn earliest_operation(trimmed: &str) -> Option<String> {
    const STD_OPERATION_TOKENS: [&str; 5] =
        [".find(", ".rfind(", ".len_utf8(", ".chars(", ".map_or("];
    let mut best: Option<(usize, String)> = None;
    let mut consider = |at: usize, operation: String| {
        if best.as_ref().is_none_or(|(pos, _)| at < *pos) {
            best = Some((at, operation));
        }
    };
    for token in STD_OPERATION_TOKENS {
        if let Some(at) = trimmed.find(token) {
            consider(at, token.to_string());
        }
    }
    for token in ["<<", ">>"] {
        if let Some(at) = trimmed.find(token) {
            consider(at, token.to_string());
        }
    }
    if let Some(at) = trimmed.find('(') {
        consider(at, trimmed[..=at].to_string());
    }
    for (at, character) in trimmed.char_indices() {
        match character {
            '+' | '*' | '/' | '%' | '&' | '|' | '^' | '<' | '>' => {
                consider(at, character.to_string());
            }
            // A `-` is an operation only between operands; a leading or
            // post-operator `-` is a literal sign, not an operation.
            '-' if at > 0
                && trimmed[..at]
                    .trim_end()
                    .chars()
                    .next_back()
                    .is_some_and(|prev| {
                        prev.is_ascii_alphanumeric() || prev == '_' || prev == ')' || prev == ']'
                    }) =>
            {
                consider(at, character.to_string());
            }
            _ => {}
        }
    }
    best.map(|(_, operation)| operation)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BODY_START: usize = 1;

    fn resolve_end(body: &str, changed_line: usize) -> BindingPredicateResolution {
        resolve_changed_binding_uses(
            "end",
            "input.rfind(delim).map_or(0, |idx| idx)",
            body,
            BODY_START,
            changed_line,
        )
    }

    fn blocked_scope(resolution: &BindingPredicateResolution) -> Result<BindingUseScope, String> {
        match resolution {
            BindingPredicateResolution::NoDirectUse { scope } => Ok(scope.clone()),
            BindingPredicateResolution::DirectUses(uses) => {
                Err(format!("expected no direct use, got {uses:?}"))
            }
        }
    }

    fn direct_uses(
        resolution: &BindingPredicateResolution,
    ) -> Result<&[ChangedBindingPredicateUse], String> {
        match resolution {
            BindingPredicateResolution::DirectUses(uses) => Ok(uses),
            BindingPredicateResolution::NoDirectUse { scope } => {
                Err(format!("expected direct uses, blocked by {scope:?}"))
            }
        }
    }

    #[test]
    fn resolves_changed_binding_to_equality_predicate() -> Result<(), String> {
        let body = "pub fn split(input: &str, delim: char) -> &str {\n    let end = input.rfind(delim).map_or(0, |idx| idx);\n    let start = delim.chars().next().map_or(0, |c| c.len_utf8());\n    if end == start {\n        &input[..end]\n    } else {\n        input\n    }\n}\n";
        let resolution = resolve_end(body, 2);
        let uses = direct_uses(&resolution)?;
        assert_eq!(uses.len(), 1);
        assert_eq!(uses[0].predicate_line, 4);
        assert_eq!(uses[0].predicate_expression, "if end == start {");
        assert_eq!(uses[0].operand_side, PredicateOperandSide::Left);
        assert_eq!(
            uses[0].value_resolution,
            BindingValueResolution::Unresolved {
                earliest_operation: ".rfind(".to_string()
            }
        );
        Ok(())
    }

    #[test]
    fn literal_initializer_resolves_to_text() -> Result<(), String> {
        let body = "pub fn f() -> bool {\n    let limit = 10;\n    limit > 2\n}\n";
        let resolution = resolve_changed_binding_uses("limit", "10", body, BODY_START, 2);
        let uses = direct_uses(&resolution)?;
        assert_eq!(
            uses[0].value_resolution,
            BindingValueResolution::ResolvedToText("10".to_string())
        );
        Ok(())
    }

    #[test]
    fn earliest_operation_is_leftmost_not_first_listed() {
        // `.rfind(` is listed before `+`, but `+` is the leftmost
        // operation in the initializer, so it is the named edge.
        let resolution = value_resolution("a + b.rfind(c)");
        assert_eq!(
            resolution,
            BindingValueResolution::Unresolved {
                earliest_operation: "+".to_string()
            }
        );
    }

    #[test]
    fn unsupported_calls_and_arithmetic_stay_unresolved() {
        let cases = [
            ("input.len()", "input.len("),
            ("compute(x)", "compute("),
            ("x - 1", "-"),
            ("a * b", "*"),
        ];
        for (initializer, operation) in cases {
            assert_eq!(
                value_resolution(initializer),
                BindingValueResolution::Unresolved {
                    earliest_operation: operation.to_string()
                },
                "initializer `{initializer}` must stay unresolved"
            );
        }
    }

    #[test]
    fn literals_and_identifier_copies_resolve_to_text() {
        for initializer in ["10", "-1", "\"done\"", "\"50%\"", "\"a/b\"", "copied"] {
            assert_eq!(
                value_resolution(initializer),
                BindingValueResolution::ResolvedToText(initializer.to_string()),
                "initializer `{initializer}` is a bare value copy"
            );
        }
    }

    #[test]
    fn supports_comparison_boolean_and_match_positions() {
        let cases = [
            ("if end == start {", PredicateOperandSide::Left),
            ("if start == end {", PredicateOperandSide::Right),
            ("if end != start {", PredicateOperandSide::Left),
            ("if start >= end {", PredicateOperandSide::Right),
            ("while end > limit {", PredicateOperandSide::Left),
            ("if end {", PredicateOperandSide::Boolean),
            ("else if end {", PredicateOperandSide::Boolean),
            ("match end {", PredicateOperandSide::MatchScrutinee),
            ("if other && end == start {", PredicateOperandSide::Left),
            (
                "if start == end || unrelated {",
                PredicateOperandSide::Right,
            ),
        ];
        for (line, expected) in cases {
            assert_eq!(
                direct_predicate_operand(line, "end"),
                Some(expected),
                "operand position mismatch for `{line}`"
            );
        }
    }

    #[test]
    fn field_paths_and_longer_identifiers_never_relate() {
        assert_eq!(
            direct_predicate_operand("if self.end == start {", "end"),
            None
        );
        assert_eq!(
            direct_predicate_operand("if endpoint == start {", "end"),
            None
        );
        assert_eq!(
            direct_predicate_operand("if end_two == start {", "end"),
            None
        );
        assert_eq!(direct_predicate_operand("if start == rend {", "end"), None);
    }

    // Strict direct-operand semantics (#3294 review): a method
    // receiver, a composite expression operand, and a call or index
    // interior are never predicate uses.
    #[test]
    fn receivers_composites_and_call_interiors_never_relate() {
        assert_eq!(direct_predicate_operand("if end.is_empty() {", "end"), None);
        assert_eq!(direct_predicate_operand("match end.kind() {", "end"), None);
        assert_eq!(direct_predicate_operand("if end + 1 == 2 {", "end"), None);
        assert_eq!(
            direct_predicate_operand("if map.get(&end) < limit {", "end"),
            None
        );
        assert_eq!(direct_predicate_operand("if f(a == end, b) {", "end"), None);
        // A reference or dereference of the binding is still direct.
        assert_eq!(
            direct_predicate_operand("if *end == 2 {", "end"),
            Some(PredicateOperandSide::Left)
        );
        assert_eq!(
            direct_predicate_operand("if ready && end {", "end"),
            Some(PredicateOperandSide::Boolean)
        );
    }

    #[test]
    fn sibling_function_use_never_relates() -> Result<(), String> {
        // The wiring scopes the resolver to the owner function of the
        // changed line (`find_owner_function`), so a same-named binding
        // in a sibling function is never even scanned. This pins the
        // owner-scoped body's outcome; the end-to-end sibling control
        // lives in the #3294 fixtures.
        let body = "pub fn first(input: &str) -> &str {\n    let end = 1;\n    input\n}\n";
        let resolution = resolve_end(body, 2);
        assert_eq!(blocked_scope(&resolution)?, BindingUseScope::NoPredicateUse);
        Ok(())
    }

    #[test]
    fn inner_scope_shadowing_blocks_later_uses() -> Result<(), String> {
        let body = "pub fn f() -> bool {\n    let end = 1;\n    {\n        let end = 2;\n        end == 3\n    }\n}\n";
        let resolution = resolve_end(body, 2);
        assert_eq!(
            blocked_scope(&resolution)?,
            BindingUseScope::ShadowedBeforeUse
        );
        Ok(())
    }

    #[test]
    fn destructuring_rebind_blocks_later_uses() -> Result<(), String> {
        let body = "pub fn f(pair: (usize, usize)) -> bool {\n    let end = 1;\n    let (end, other) = pair;\n    end == other\n}\n";
        let resolution = resolve_end(body, 2);
        assert_eq!(
            blocked_scope(&resolution)?,
            BindingUseScope::ShadowedBeforeUse
        );
        Ok(())
    }

    #[test]
    fn reassignment_blocks_later_uses() -> Result<(), String> {
        let body =
            "pub fn f() -> bool {\n    let mut end = 1;\n    end = compute();\n    end == 3\n}\n";
        let resolution = resolve_end(body, 2);
        assert_eq!(
            blocked_scope(&resolution)?,
            BindingUseScope::ReassignedBeforeUse
        );
        Ok(())
    }

    #[test]
    fn compound_reassignment_blocks_later_uses() -> Result<(), String> {
        let body = "pub fn f() -> bool {\n    let mut end = 1;\n    end += 1;\n    end == 3\n}\n";
        let resolution = resolve_end(body, 2);
        assert_eq!(
            blocked_scope(&resolution)?,
            BindingUseScope::ReassignedBeforeUse
        );
        Ok(())
    }

    #[test]
    fn use_before_reassignment_still_relates() -> Result<(), String> {
        let body = "pub fn f() -> bool {\n    let mut end = 1;\n    if end == 2 { }\n    end = 5;\n    end == 9\n}\n";
        let resolution = resolve_end(body, 2);
        let uses = direct_uses(&resolution)?;
        assert_eq!(uses.len(), 1, "only the pre-reassignment use relates");
        assert_eq!(uses[0].predicate_line, 3);
        Ok(())
    }

    #[test]
    fn closure_use_fails_closed() -> Result<(), String> {
        // The comparison inside the foreign closure initializer is not
        // a direct use, and pipes on the line block any candidate.
        let body = "pub fn f() -> bool {\n    let end = 1;\n    let check = |other: usize| end == other;\n    check(2)\n}\n";
        let resolution = resolve_end(body, 2);
        assert_eq!(blocked_scope(&resolution)?, BindingUseScope::NoPredicateUse);
        Ok(())
    }

    #[test]
    fn closure_pipes_on_the_use_line_block_it() -> Result<(), String> {
        let body = "pub fn f() -> bool {\n    let end = 1;\n    if end == 2 { sink(|c| c); }\n    true\n}\n";
        let resolution = resolve_end(body, 2);
        assert_eq!(
            blocked_scope(&resolution)?,
            BindingUseScope::ClosureCaptureUnsupported
        );
        Ok(())
    }

    #[test]
    fn macro_use_fails_closed() -> Result<(), String> {
        // A comparison inside macro parentheses is not a direct use;
        // the macro blocker fires for a statement-level use on a line
        // that also invokes a macro.
        let body = "pub fn f() -> bool {\n    let end = 1;\n    if end == 2 { ensure!(true); }\n    true\n}\n";
        let resolution = resolve_end(body, 2);
        assert_eq!(
            blocked_scope(&resolution)?,
            BindingUseScope::MacroExpansionUnsupported
        );
        Ok(())
    }

    // `!(` negation is not a macro invocation; the negated (grouped)
    // comparison is simply not a direct operand and fails closed
    // without the wrong blocker.
    #[test]
    fn negated_grouped_comparison_is_not_a_macro() -> Result<(), String> {
        assert!(!is_macro_invocation_line("if !(end == start) {"));
        let body = "pub fn f() -> bool {\n    let end = 1;\n    if !(end == 2) {\n        true\n    } else {\n        false\n    }\n}\n";
        let resolution = resolve_end(body, 2);
        assert_eq!(blocked_scope(&resolution)?, BindingUseScope::NoPredicateUse);
        Ok(())
    }

    // A changed declaration inside an inner block ends with the block:
    // a later same-named outer binding's use never relates to it.
    #[test]
    fn block_scope_exit_ends_the_live_span() -> Result<(), String> {
        let body = "pub fn f(flag: bool) -> bool {\n    let end = 1;\n    {\n        let end = 2;\n        flag\n    }\n    end == 4\n}\n";
        // The changed declaration is the inner `let end = 2;` (line 4);
        // the later `end == 4` belongs to the outer binding.
        let resolution = resolve_end(body, 4);
        assert_eq!(
            blocked_scope(&resolution)?,
            BindingUseScope::ScopeExitedBeforeUse
        );
        Ok(())
    }

    #[test]
    fn comment_and_string_mentions_never_relate() -> Result<(), String> {
        let body = "// let end = 1;\nlet end = 1;\n// if end == start { }\nlet label = \"if end == start\";\n";
        let resolution = resolve_end(body, 2);
        assert_eq!(blocked_scope(&resolution)?, BindingUseScope::NoPredicateUse);
        Ok(())
    }

    #[test]
    fn if_let_pattern_rebind_blocks_later_uses() -> Result<(), String> {
        // `if let Some(end) = opt` binds its own `end`: it is a re-bind,
        // never a boolean use of the changed binding.
        let body = "pub fn f(opt: Option<usize>) -> bool {\n    let end = 1;\n    if let Some(end) = opt {\n        end == 3\n    } else {\n        false\n    }\n}\n";
        let resolution = resolve_end(body, 2);
        assert_eq!(
            blocked_scope(&resolution)?,
            BindingUseScope::ShadowedBeforeUse
        );
        Ok(())
    }

    // The rustfmt shape `} else if let Some(end) = opt {` re-binds
    // its pattern variable: the leading `}` must not hide the pattern.
    #[test]
    fn else_if_let_pattern_rebind_blocks_later_uses() -> Result<(), String> {
        let body = "pub fn f(opt: Option<usize>) -> bool {\n    let end = 1;\n    if opt.is_none() {\n        false\n    } else if let Some(end) = opt {\n        end == 3\n    } else {\n        false\n    }\n}\n";
        let resolution = resolve_end(body, 2);
        assert_eq!(
            blocked_scope(&resolution)?,
            BindingUseScope::ShadowedBeforeUse
        );
        Ok(())
    }

    #[test]
    fn for_loop_pattern_rebind_blocks_later_uses() -> Result<(), String> {
        let body = "pub fn f(items: &[usize]) -> bool {\n    let end = 1;\n    for end in items {\n        end == 3\n    }\n    false\n}\n";
        let resolution = resolve_end(body, 2);
        assert_eq!(
            blocked_scope(&resolution)?,
            BindingUseScope::ShadowedBeforeUse
        );
        Ok(())
    }

    #[test]
    fn foreign_if_let_keeps_outer_binding_live() -> Result<(), String> {
        // `if let Some(other)` re-binds `other`, not `end`: the later
        // use of `end` still relates.
        let body = "pub fn f(opt: Option<usize>) -> bool {\n    let end = 1;\n    if let Some(other) = opt {\n        other == 3\n    }\n    end == 4\n}\n";
        let resolution = resolve_end(body, 2);
        let uses = direct_uses(&resolution)?;
        assert_eq!(uses.len(), 1);
        assert_eq!(uses[0].predicate_line, 6);
        Ok(())
    }

    // #3294 review finding 1 (blocker): shift operators are not
    // comparisons — `flags << end` on a call or assignment line must
    // never read as a predicate use of the binding.
    #[test]
    fn shift_operands_never_relate() -> Result<(), String> {
        let shapes = [
            "pub fn f(flags: u32, sink: &mut u32) {\n    let end = 1;\n    sink(flags << end);\n}\n",
            "pub fn f(flags: u32, sum: &mut u32) {\n    let end = 1;\n    *sum += flags << end;\n}\n",
        ];
        for body in shapes {
            let resolution = resolve_end(body, 2);
            assert_eq!(
                blocked_scope(&resolution)?,
                BindingUseScope::NoPredicateUse,
                "shift operand must not relate: {body}"
            );
        }
        // A shift inside a real comparison whose operand is the
        // composite `(1 << end)` is not a direct identifier operand:
        // strict semantics fail closed.
        assert_eq!(direct_predicate_operand("if (1 << end) > 2 {", "end"), None);
        Ok(())
    }

    // #3294 review finding 2: a closure body opened on a previous line
    // is a no-use region; the captured use never relates.
    #[test]
    fn multiline_closure_body_never_relates() -> Result<(), String> {
        let body = "pub fn f() -> bool {\n    let end = 1;\n    let check = |other: usize| {\n        end == other\n    };\n    check(2)\n}\n";
        let resolution = resolve_end(body, 2);
        assert_eq!(
            blocked_scope(&resolution)?,
            BindingUseScope::NoPredicateUse,
            "a use inside the closure body must not relate"
        );
        Ok(())
    }

    #[test]
    fn closure_local_shadow_does_not_end_outer_liveness() -> Result<(), String> {
        let body = "pub fn f() -> bool {\n    let end = 1;\n    let check = |x: usize| {\n        let end = 2;\n        end == 2\n    };\n    end == 3\n}\n";
        let resolution = resolve_end(body, 2);
        let uses = direct_uses(&resolution)?;
        assert_eq!(uses.len(), 1, "only the post-closure outer use relates");
        assert_eq!(uses[0].predicate_line, 7);
        Ok(())
    }

    // #3294 review finding 3: a nested item is a separate binding
    // scope; its parameters and locals never relate.
    #[test]
    fn nested_function_body_never_relates() -> Result<(), String> {
        let body = "pub fn f() -> bool {\n    let end = 1;\n    fn inner(end: usize) -> bool {\n        end == 3\n    }\n    inner(end)\n}\n";
        let resolution = resolve_end(body, 2);
        assert_eq!(
            blocked_scope(&resolution)?,
            BindingUseScope::NoPredicateUse,
            "nested item parameters are a different binding"
        );
        Ok(())
    }

    // #3294 review finding 5: a comparison inside a foreign `let`'s
    // multi-line initializer is a continuation line, not a use.
    #[test]
    fn foreign_let_multiline_initializer_never_relates() -> Result<(), String> {
        let body = "pub fn f(wrap: fn(bool) -> bool) -> bool {\n    let end = 1;\n    let other = wrap(\n        end == 3\n    );\n    other\n}\n";
        let resolution = resolve_end(body, 2);
        assert_eq!(
            blocked_scope(&resolution)?,
            BindingUseScope::NoPredicateUse,
            "continuation lines inside foreign parens are not use sites"
        );
        Ok(())
    }

    // #3294 review finding 6: bit operations in the initializer are
    // unresolved operations, not resolved text.
    #[test]
    fn bit_operation_initializers_stay_unresolved() {
        let cases = [("flags & mask", "&"), ("1 << shift", "<<"), ("x | y", "|")];
        for (initializer, operation) in cases {
            assert_eq!(
                value_resolution(initializer),
                BindingValueResolution::Unresolved {
                    earliest_operation: operation.to_string()
                },
                "initializer `{initializer}` must stay unresolved"
            );
        }
    }

    // A single `|` between operands is a bitwise OR, not a closure
    // pipe — but the composite `a | end` is not a direct comparison
    // operand either, so the shape fails closed on both readings.
    #[test]
    fn bitwise_or_in_condition_is_not_a_closure() -> Result<(), String> {
        assert_eq!(direct_predicate_operand("if a | end == b {", "end"), None);
        let body = "pub fn f(a: u8, b: u8) -> bool {\n    let end = 1;\n    if a | end == b {\n        true\n    } else {\n        false\n    }\n}\n";
        let resolution = resolve_end(body, 2);
        assert_eq!(blocked_scope(&resolution)?, BindingUseScope::NoPredicateUse);
        Ok(())
    }

    #[test]
    fn modulo_reassignment_ends_liveness() -> Result<(), String> {
        let body = "pub fn f() -> bool {\n    let mut end = 7;\n    end %= 2;\n    end == 3\n}\n";
        let resolution = resolve_end(body, 2);
        assert_eq!(
            blocked_scope(&resolution)?,
            BindingUseScope::ReassignedBeforeUse
        );
        Ok(())
    }

    #[test]
    fn brace_leading_else_if_still_relates() {
        assert_eq!(
            direct_predicate_operand("} else if end {", "end"),
            Some(PredicateOperandSide::Boolean)
        );
    }

    // The unit of relation is the predicate use line: the same
    // binding appearing twice in one predicate statement yields one
    // relation naming that statement, not two occurrences of the same
    // line and text (#3294 review: deterministic by design).
    #[test]
    fn duplicated_operand_on_one_line_yields_one_relation() -> Result<(), String> {
        let body = "pub fn f(start: usize) -> bool {\n    let end = 1;\n    if end == start || end == start {\n        true\n    } else {\n        false\n    }\n}\n";
        let resolution = resolve_end(body, 2);
        let uses = direct_uses(&resolution)?;
        assert_eq!(uses.len(), 1);
        assert_eq!(
            uses[0].predicate_expression,
            "if end == start || end == start {"
        );
        Ok(())
    }

    #[test]
    fn two_predicate_uses_stay_separately_identifiable() -> Result<(), String> {
        let body = "pub fn f() -> usize {\n    let end = 1;\n    if end == 2 { }\n    if end != 3 { }\n    end\n}\n";
        let resolution = resolve_end(body, 2);
        let uses = direct_uses(&resolution)?;
        assert_eq!(uses.len(), 2);
        assert_eq!(uses[0].predicate_line, 3);
        assert_eq!(uses[0].predicate_expression, "if end == 2 { }");
        assert_eq!(uses[1].predicate_line, 4);
        assert_eq!(uses[1].predicate_expression, "if end != 3 { }");
        Ok(())
    }
}
