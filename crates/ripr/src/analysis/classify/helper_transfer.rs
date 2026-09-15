//! Bounded helper-call transfer (#3296, P4 of #3215).
//!
//! When the changed behavior sits inside a helper that related tests
//! reach only through a short, statically resolvable call chain
//! (`test -> classify -> is_word_start`), the exact input literals the
//! tests provide never cross the helper edge: the helper-owned probe
//! reports no static path and the transitive-reach limitation can only
//! hint that a caller "may lead here".
//!
//! This module is the one helper-transfer authority. It resolves the
//! bounded chain once and is consumed by the relation (test
//! relatedness) and activation (exact input rows) stages, so every
//! surface sees the same decision.
//!
//! V1 transfers only:
//!
//! - direct same-crate calls with a **unique callee identity** (the
//!   #2971 workspace-complete uniqueness rule);
//! - **positional argument-to-parameter binding** where each bound
//!   argument is a literal or the caller's own parameter (resolved
//!   through the caller's rows);
//! - chains of at most [`MAX_HELPER_HOPS`] hops, acyclic, with a
//!   single caller and a single call site per hop.
//!
//! Everything else stops at a named edge: recursion, an ambiguous or
//! unknown callee, a computed (non-literal, non-parameter) argument,
//! multi-caller or multi-site binding, and hop exhaustion. Call reach
//! alone never establishes propagation or discrimination — the oracle
//! still has to observe the sink through the ordinary evidence stages.

use super::value_transfer::ExactInputs;
use crate::analysis::facts::{CallFact, FunctionSummary, RustIndex};

/// The configured hop bound for helper transfer. Exceeding it yields a
/// typed limitation naming the chain's stop, never a silent drop.
pub(crate) const MAX_HELPER_HOPS: usize = 3;

/// Bounded evaluation context for helper returns whose bodies call
/// other helpers (#3296 recursive row). `depth` counts helper-return
/// evaluations on the current path — the same explicit
/// [`MAX_HELPER_HOPS`] bound the caller-side chain enforces — and
/// `states` records each `(helper, bound inputs)` state already
/// evaluated: a repeated state is a true cycle (the same inputs take
/// the same arm), while distinct inputs unroll within the bound.
pub(crate) struct HelperEval<'a> {
    index: &'a RustIndex,
    workspace_complete: bool,
    depth: usize,
    states: Vec<String>,
}

impl<'a> HelperEval<'a> {
    pub(crate) fn root(index: &'a RustIndex, workspace_complete: bool) -> Self {
        HelperEval {
            index,
            workspace_complete,
            depth: 0,
            states: Vec::new(),
        }
    }

    /// Enter the evaluation of `helper` over `inputs`: refuse a state
    /// already on the path (a cycle) or a path at the hop bound, and
    /// return the advanced context for the body's evaluation.
    fn enter(&self, helper: &FunctionSummary, inputs: &ExactInputs) -> Option<HelperEval<'a>> {
        if self.depth >= MAX_HELPER_HOPS {
            return None;
        }
        let key = format!(
            "{}|{}",
            helper.name,
            inputs
                .iter()
                .map(|(parameter, value)| format!("{parameter}={value}"))
                .collect::<Vec<_>>()
                .join(",")
        );
        if self.states.iter().any(|state| state == &key) {
            return None;
        }
        let mut states = self.states.clone();
        states.push(key);
        Some(HelperEval {
            index: self.index,
            workspace_complete: self.workspace_complete,
            depth: self.depth + 1,
            states,
        })
    }
}

/// Resolve one nested direct call inside a helper body (`label_of("a")`
/// as a match arm value) over the calling helper's bound inputs. The
/// decomposition, uniqueness, and splittability checks are the shared
/// direct-call authority; argument binding stays strict (a literal or
/// a parameter bound in `inputs`). A cycle or the hop bound refuses
/// (fail closed).
pub(crate) fn nested_call_value(
    call: &str,
    inputs: &ExactInputs,
    eval: &HelperEval<'_>,
) -> Option<super::value_transfer::TypedValue> {
    if !eval.workspace_complete {
        return None;
    }
    let super::activation::ResolvedOperand::Call { callee, arguments } =
        super::activation::resolve_direct_call(call, eval.index, eval.workspace_complete)?
    else {
        return None;
    };
    let mut bound = ExactInputs::new();
    let parameters = super::activation::function_parameters(&callee);
    for (index, argument) in arguments.iter().enumerate() {
        let parameter = parameters
            .get(index)
            .cloned()
            .unwrap_or_else(|| format!("arg{index}"));
        let value = strict_literal(argument).or_else(|| inputs.get(argument.trim()).cloned())?;
        bound.insert(parameter, value);
    }
    helper_return_value(&callee, &bound, eval)
}

/// One hop of a resolved helper chain: the caller, the call site, and
/// the positional argument texts at that site.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HelperHop {
    pub(crate) caller: FunctionSummary,
    pub(crate) call_text: String,
    pub(crate) arguments: Vec<String>,
}

/// The resolved chain for one helper. `stop_above` names the first
/// unsupported edge above the resolved hops (an upper bound reached, a
/// recursion, or an ambiguous grand-caller); it does not invalidate the
/// hops below it, which may still carry direct test rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HelperChain {
    pub(crate) hops: Vec<HelperHop>,
    pub(crate) stop_above: Option<String>,
}

impl HelperChain {
    /// The function a related test can call directly: the topmost
    /// resolved hop's caller.
    #[cfg(test)]
    pub(crate) fn entry_name(&self) -> Option<&str> {
        self.hops.last().map(|hop| hop.caller.name.as_str())
    }
}

/// Whether `callee_name` names exactly one function in the index (the
/// #2971 uniqueness rule; the caller must already have established
/// workspace completeness — a partial index would make a same-named
/// function in an unindexed file invisible and the name falsely
/// unique).
pub(crate) fn callee_is_unique(callee_name: &str, index: &RustIndex) -> bool {
    !callee_name.is_empty()
        && index
            .functions
            .iter()
            .filter(|function| function.name == callee_name)
            .count()
            == 1
}

/// The functions that call `callee_name` directly. Recursion
/// (`function.name == callee_name`) is excluded here and named by the
/// resolver — a self-recursive helper is a bound, not a transfer.
pub(crate) fn direct_callers<'a>(
    callee_name: &str,
    index: &'a RustIndex,
) -> Vec<&'a FunctionSummary> {
    index
        .functions
        .iter()
        .filter(|function| {
            function.name != callee_name
                && function.calls.iter().any(|call| call.name == callee_name)
        })
        .collect()
}

/// The call sites in `caller` that invoke `callee_name`, with their
/// split argument texts. `None` when a call text cannot be split.
pub(crate) fn call_sites(
    caller: &FunctionSummary,
    callee_name: &str,
) -> Option<Vec<(String, Vec<String>)>> {
    let mut sites = Vec::new();
    for call in &caller.calls {
        if call.name != callee_name {
            continue;
        }
        // #3296 review B1: a method call (`word.validate()`) or a
        // path-qualified invocation (`internal::inner(..)`) is recorded
        // under the bare callee name, and name identity is not callee
        // identity. Only a direct free-function call site may bind.
        if !is_direct_call_site(&call.text, callee_name) {
            continue;
        }
        let arguments = split_call_arguments_text(&call.text, callee_name)?;
        sites.push((call.text.clone(), arguments));
    }
    (!sites.is_empty()).then_some(sites)
}

/// Byte index of the `(` that opens a direct free-function call of
/// `callee_name`: the first `callee(` occurrence at a token boundary that
/// is preceded by neither a receiver (`.`), a path qualifier (`:`), nor an
/// identifier character. Later occurrences are examined when an earlier one
/// is shadowed (e.g. `my_inner(2); inner(1)` still resolves the direct
/// `inner(1)` site). `None` when no occurrence qualifies.
fn direct_call_paren(text: &str, callee_name: &str) -> Option<usize> {
    let needle = format!("{callee_name}(");
    let mut search = 0usize;
    while let Some(relative) = text[search..].find(&needle) {
        let at = search + relative;
        let direct = match text[..at].chars().next_back() {
            None => true,
            Some(before) => {
                !before.is_ascii_alphanumeric() && before != '_' && before != '.' && before != ':'
            }
        };
        if direct {
            return Some(at);
        }
        search = at + 1;
    }
    None
}

/// Whether `text` invokes `callee_name` as a direct free-function call:
/// the callee occurrence is at a token boundary and is preceded by
/// neither a receiver (`.`) nor a path qualifier (`::`).
pub(crate) fn is_direct_call_site(text: &str, callee_name: &str) -> bool {
    direct_call_paren(text, callee_name).is_some()
}

/// Split a call's argument texts, quote- and nesting-aware.
pub(crate) fn split_call_arguments_text(text: &str, callee_name: &str) -> Option<Vec<String>> {
    let open = direct_call_paren(text, callee_name)?;
    let after = &text[open + callee_name.len() + 1..];
    let mut arguments = Vec::new();
    let mut depth = 0usize;
    let mut literal: Option<char> = None;
    let mut escaped = false;
    let mut start = 0usize;
    for (at, character) in after.char_indices() {
        if let Some(quote) = literal {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == quote {
                literal = None;
            }
            continue;
        }
        match character {
            '"' | '\'' => literal = Some(character),
            '(' | '[' => depth += 1,
            ')' | ']' => {
                if depth == 0 {
                    let last = after[start..at].trim();
                    if !last.is_empty() {
                        arguments.push(last.to_string());
                    }
                    return Some(arguments);
                }
                depth -= 1;
            }
            ',' if depth == 0 => {
                let argument = after[start..at].trim();
                if !argument.is_empty() {
                    arguments.push(argument.to_string());
                }
                start = at + 1;
            }
            _ => {}
        }
    }
    None
}

/// Resolve the helper chain above `callee_name`, one hop at a time.
/// `visited` carries the chain's names so a cycle stops with the
/// recursion edge named. The result always names either the resolved
/// hops or the exact stop edge.
pub(crate) fn resolve_chain(
    callee_name: &str,
    index: &RustIndex,
    workspace_complete: bool,
    visited: &[String],
) -> HelperChain {
    let stopped = |edge: String| HelperChain {
        hops: Vec::new(),
        stop_above: Some(edge),
    };
    if !workspace_complete {
        return stopped(
            "helper chain requires a workspace-complete index (the analysis mode indexed a subset)"
                .to_string(),
        );
    }
    if visited.len() >= MAX_HELPER_HOPS {
        return stopped(format!(
            "helper chain from `{callee_name}` exceeds the {MAX_HELPER_HOPS}-hop bound"
        ));
    }
    if !callee_is_unique(callee_name, index) {
        return stopped(format!(
            "callee `{callee_name}` is not a unique function in the workspace"
        ));
    }
    if visited.contains(&callee_name.to_string()) {
        return stopped(format!(
            "recursion through helper `{callee_name}` at the transfer bound"
        ));
    }
    let callers = direct_callers(callee_name, index);
    if callers.is_empty() {
        return stopped(format!("no static caller found for helper `{callee_name}`"));
    }
    if callers.len() > 1 {
        return stopped(format!(
            "callee `{callee_name}` has {} callers; multi-caller binding is not transferred at this bound",
            callers.len()
        ));
    }
    let caller = callers[0];
    let Some(sites) = call_sites(caller, callee_name) else {
        return stopped(format!(
            "call site for `{callee_name}` in `{}` is not statically splittable",
            caller.name
        ));
    };
    if sites.len() > 1 {
        return stopped(format!(
            "helper `{callee_name}` is called {} times in `{}`; multi-site binding is not transferred at this bound",
            sites.len(),
            caller.name
        ));
    }
    let (call_text, arguments) = sites[0].clone();
    let hop = HelperHop {
        caller: caller.clone(),
        call_text,
        arguments,
    };
    let mut next_visited = visited.to_vec();
    next_visited.push(callee_name.to_string());
    let HelperChain {
        mut hops,
        stop_above,
    } = resolve_chain(&caller.name, index, workspace_complete, &next_visited);
    // #3296 review M2: tests are ordinary functions in the index, so
    // the chain must never climb into a test body — the entry a test
    // can call directly is the topmost production caller.
    while hops
        .last()
        .is_some_and(|top| top.caller.source_role.is_evidence_role())
    {
        hops.pop();
    }
    let mut resolved = vec![hop];
    resolved.append(&mut hops);
    HelperChain {
        hops: resolved,
        stop_above,
    }
}

/// Whether a test reaches `callee_name` through a bounded helper chain
/// (the relation stage's question): the test calls some resolved hop's
/// caller directly. The relation stage resolves the chain once per
/// probe and checks the calls itself (#3296 review M1); this query
/// stays as the unit-test surface for the reach question.
#[cfg_attr(not(test), allow(dead_code, reason = "unit-test query surface"))]
pub(crate) fn test_reaches_through_chain(
    test_calls: &[CallFact],
    callee_name: &str,
    index: &RustIndex,
    workspace_complete: bool,
) -> Option<HelperChain> {
    let chain = resolve_chain(callee_name, index, workspace_complete, &[]);
    if chain.hops.is_empty() {
        return None;
    }
    let reached = test_calls
        .iter()
        .any(|call| chain.hops.iter().any(|hop| hop.caller.name == call.name));
    reached.then_some(chain)
}

/// Evaluate a helper's return value over bound exact inputs (#3296,
/// the boolean/return predicate family). V1 supports a body whose
/// final expression is a bare binding established by simple single-line
/// `let` statements: the binding's initializer evaluates through the
/// #3295 families. A body with loops, inner calls, early returns, or
/// any other shape fails closed to `None`.
pub(crate) fn helper_return_value(
    helper: &FunctionSummary,
    inputs: &ExactInputs,
    eval: &HelperEval<'_>,
) -> Option<super::value_transfer::TypedValue> {
    // The bounded context refuses a repeated (helper, inputs) state (a
    // true cycle) and a path at the hop bound before any body shape is
    // considered.
    let eval = eval.enter(helper, inputs)?;
    // #3296: the scanner authority owns the literal-driven state-loop
    // shape first; the literal match-arm authority owns the string
    // `match` tail expression (and may resolve nested direct calls in
    // arm values through the same context); the let-chain evaluator
    // below keeps every other body (and stays the authority for
    // non-scanner tails).
    if let Some(value) = super::scanner_transfer::scanner_return_value(helper, inputs) {
        return Some(value);
    }
    if let Some(value) = super::match_transfer::match_return_value(helper, inputs, &eval) {
        return Some(value);
    }
    let masked = crate::analysis::language::mask_rust_comments_and_strings(&helper.body);
    let raw_lines: Vec<&str> = helper.body.lines().collect();
    let masked_lines: Vec<&str> = masked.lines().collect();
    if raw_lines.len() != masked_lines.len() {
        return None;
    }
    let mut bindings: Vec<(String, String)> = Vec::new();
    let mut tail: Option<String> = None;
    for (raw_line, masked_line) in raw_lines.iter().zip(masked_lines.iter()) {
        let trimmed = masked_line.trim();
        if trimmed.starts_with("let ") {
            if !trimmed.ends_with(';') {
                return None;
            }
            let (declared, _) = crate::analysis::language::changed_let_binding(trimmed)?;
            let (declared_raw, initializer) =
                crate::analysis::language::changed_let_binding(raw_line.trim())?;
            if declared != declared_raw {
                return None;
            }
            bindings.push((declared.to_string(), initializer.to_string()));
            continue;
        }
        let cleaned = trimmed.trim_end_matches(';').trim();
        if cleaned.is_empty()
            || cleaned.starts_with("//")
            || cleaned.starts_with('#')
            || cleaned.starts_with("pub ")
            || cleaned.starts_with("pub(")
            || cleaned.starts_with("fn ")
            || cleaned.starts_with("async ")
            || cleaned.starts_with("const ")
            || cleaned.starts_with("unsafe ")
            || cleaned.starts_with('}')
            || cleaned.starts_with("return ")
        {
            continue;
        }
        if tail.is_some() {
            return None;
        }
        tail = Some(raw_line.trim().trim_end_matches(';').trim().to_string());
    }
    let tail = tail?;
    let is_identifier = !tail.is_empty()
        && tail
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
        && tail
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_');
    if !is_identifier {
        return None;
    }
    let initializer = bindings
        .iter()
        .rev()
        .find(|(name, _)| *name == tail)
        .map(|(_, initializer)| initializer.clone())?;
    match super::value_transfer::evaluate_initializer(&initializer, inputs) {
        super::value_transfer::EvalOutcome::Exact { value, .. } => Some(value),
        _ => None,
    }
}

/// The single-quote character, named so the strict literal parser does
/// not need an escaped char constant.
const CHAR_QUOTE: char = '\'';

fn char_quote() -> char {
    CHAR_QUOTE
}

/// Parse a call-site argument as a strict literal (#3296 review B2):
/// only a whole-token string, char, integer, or boolean literal counts.
/// An identifier like `a2` is a parameter reference, never the literal
/// `2` — the substring scanner must not fabricate values.
pub(crate) fn strict_literal(argument: &str) -> Option<String> {
    let trimmed = argument.trim();
    if trimmed.is_empty() {
        return None;
    }
    let is_string = trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2;
    let is_char =
        trimmed.starts_with(char_quote()) && trimmed.ends_with(char_quote()) && trimmed.len() >= 3;
    let is_integer = trimmed.chars().all(|ch| ch.is_ascii_digit());
    let is_boolean = trimmed == "true" || trimmed == "false";
    if !(is_string || is_char || is_integer || is_boolean) {
        return None;
    }
    Some(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::facts::FunctionSourceRole;
    use crate::analysis::facts::{CallFact, FunctionSummary};
    use crate::domain::SymbolId;
    use std::path::PathBuf;

    fn function(file: &str, name: &str, calls: &[(&str, &str)]) -> FunctionSummary {
        FunctionSummary {
            id: SymbolId(format!("{file}::{name}")),
            name: name.to_string(),
            file: PathBuf::from(file),
            start_line: 1,
            end_line: 3,
            body: format!("pub fn {name}(input: &str) -> bool {{ true }}"),
            calls: calls
                .iter()
                .map(|(callee, text)| CallFact {
                    line: 2,
                    name: callee.to_string(),
                    text: text.to_string(),
                })
                .collect(),
            returns: Vec::new(),
            literals: Vec::new(),
            source_role: FunctionSourceRole::Production,
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        }
    }

    fn index(functions: Vec<FunctionSummary>) -> RustIndex {
        RustIndex {
            functions,
            ..RustIndex::default()
        }
    }

    #[test]
    fn one_hop_chain_resolves_with_arguments() -> Result<(), String> {
        let owner = function("src/lib.rs", "is_word_start", &[]);
        let caller = function(
            "src/lib.rs",
            "classify",
            &[("is_word_start", "is_word_start(input, 0)")],
        );
        let idx = index(vec![owner, caller]);
        let chain = resolve_chain("is_word_start", &idx, true, &[]);
        if chain.hops.is_empty() {
            return Err(format!(
                "expected a resolved hop, stopped: {:?}",
                chain.stop_above
            ));
        }
        assert_eq!(chain.hops[0].caller.name, "classify");
        assert_eq!(
            chain.hops[0].arguments,
            vec!["input".to_string(), "0".to_string()]
        );
        assert_eq!(chain.entry_name(), Some("classify"));
        Ok(())
    }

    #[test]
    fn non_unique_callee_stops_by_name() -> Result<(), String> {
        let a = function("src/a.rs", "helper", &[]);
        let b = function("src/b.rs", "helper", &[]);
        let idx = index(vec![a, b]);
        let chain = resolve_chain("helper", &idx, true, &[]);
        assert!(chain.hops.is_empty());
        assert!(
            chain
                .stop_above
                .as_ref()
                .is_some_and(|edge| edge.contains("not a unique function"))
        );
        Ok(())
    }

    #[test]
    fn incomplete_workspace_stops_by_name() -> Result<(), String> {
        let owner = function("src/lib.rs", "helper", &[]);
        let idx = index(vec![owner]);
        let chain = resolve_chain("helper", &idx, false, &[]);
        assert!(chain.hops.is_empty());
        assert!(
            chain
                .stop_above
                .as_ref()
                .is_some_and(|edge| edge.contains("workspace-complete"))
        );
        Ok(())
    }

    #[test]
    fn recursion_stops_at_the_bound() -> Result<(), String> {
        let a = function("src/a.rs", "a", &[("b", "b()")]);
        let b = function("src/b.rs", "b", &[("a", "a()")]);
        let idx = index(vec![a, b]);
        let chain = resolve_chain("a", &idx, true, &[]);
        assert!(
            chain
                .stop_above
                .as_ref()
                .is_some_and(|edge| edge.contains("recursion"))
        );
        Ok(())
    }

    #[test]
    fn multiple_callers_stop_the_binding() -> Result<(), String> {
        let owner = function("src/lib.rs", "helper", &[]);
        let first = function("src/lib.rs", "first", &[("helper", "helper()")]);
        let second = function("src/lib.rs", "second", &[("helper", "helper()")]);
        let idx = index(vec![owner, first, second]);
        let chain = resolve_chain("helper", &idx, true, &[]);
        assert!(chain.hops.is_empty());
        assert!(
            chain
                .stop_above
                .as_ref()
                .is_some_and(|edge| edge.contains("2 callers"))
        );
        Ok(())
    }

    // #3296 review B1: a method call site (`word.validate()`) shares
    // the callee's bare name but is not callee identity — it must
    // never bind a hop.
    #[test]
    fn method_call_sites_never_bind() -> Result<(), String> {
        assert!(!is_direct_call_site("word.validate()", "validate"));
        assert!(!is_direct_call_site("internal::inner(a, b)", "inner"));
        // #3713 review (splitter-occurrence thread): the shared
        // occurrence rule looks past the shadowed `outer_inner(` prefix to
        // the direct `inner(2)` site.
        assert!(is_direct_call_site("outer_inner(inner(2))", "inner"));
        assert!(is_direct_call_site(
            "is_word_start(input, 0)",
            "is_word_start"
        ));
        assert!(is_direct_call_site("let x = helper();", "helper"));
        Ok(())
    }

    // #3296 review B2: only whole-token literals bind; an identifier
    // like `a2` is never the literal `2`.
    #[test]
    fn strict_literals_reject_identifiers() -> Result<(), String> {
        assert_eq!(strict_literal("\"ab\"").as_deref(), Some("\"ab\""));
        assert_eq!(strict_literal("'x'").as_deref(), Some("'x'"));
        assert_eq!(strict_literal("42").as_deref(), Some("42"));
        assert_eq!(strict_literal("true").as_deref(), Some("true"));
        assert_eq!(strict_literal("a2"), None);
        assert_eq!(strict_literal("input"), None);
        assert_eq!(strict_literal("input.trim()"), None);
        Ok(())
    }

    #[test]
    fn test_reaches_through_the_entry_hop() -> Result<(), String> {
        let owner = function("src/lib.rs", "is_word_start", &[]);
        let caller = function(
            "src/lib.rs",
            "classify",
            &[("is_word_start", "is_word_start(input, 0)")],
        );
        let idx = index(vec![owner.clone(), caller]);
        let test_calls = vec![CallFact {
            line: 1,
            name: "classify".to_string(),
            text: "classify(\" x\")".to_string(),
        }];
        assert!(test_reaches_through_chain(&test_calls, "is_word_start", &idx, true).is_some());
        let unrelated = vec![CallFact {
            line: 1,
            name: "other".to_string(),
            text: "other()".to_string(),
        }];
        assert!(test_reaches_through_chain(&unrelated, "is_word_start", &idx, true).is_none());
        Ok(())
    }

    #[test]
    fn helper_return_value_evaluates_identity_tails_and_fails_closed_otherwise()
    -> Result<(), String> {
        let mut inputs = super::super::value_transfer::ExactInputs::new();
        inputs.insert("input".to_string(), "\" x\"".to_string());
        // An identity closure evaluates through the #3295 families.
        let evaluated = FunctionSummary {
            body: "pub fn first_char(input: &str) -> char {
    let prev = input.chars().next().map_or('?', |c| c);
    prev
}
"
            .to_string(),
            start_line: 1,
            ..function("src/lib.rs", "first_char", &[])
        };
        let empty = RustIndex::default();
        let eval = HelperEval::root(&empty, true);
        match helper_return_value(&evaluated, &inputs, &eval) {
            Some(value) => assert_eq!(value.render(), "' '"),
            None => return Err("expected the identity tail to evaluate".to_string()),
        }
        // A non-identity closure is an unsupported edge: the return
        // fails closed instead of guessing a boolean.
        let closed = FunctionSummary {
            body: "pub fn is_word_start(input: &str) -> bool {
    let prev = input.chars().next().map_or(true, |c| c == ' ');
    prev
}
"
            .to_string(),
            start_line: 1,
            ..function("src/lib.rs", "is_word_start", &[])
        };
        assert!(helper_return_value(&closed, &inputs, &eval).is_none());
        Ok(())
    }

    #[test]
    fn shadowed_first_occurrence_still_resolves_direct_site() -> Result<(), String> {
        // #3713 review: the matcher and the splitter share one
        // occurrence-resolution rule, so a shadowed first occurrence no
        // longer hides a direct call later on the same line.
        if !is_direct_call_site("my_inner(2); inner(1)", "inner") {
            return Err("expected the second occurrence to qualify".to_string());
        }
        match split_call_arguments_text("my_inner(2); inner(1)", "inner") {
            Some(arguments) if arguments == vec!["1".to_string()] => Ok(()),
            other => Err(format!("expected [\"1\"], got {other:?}")),
        }
    }

    #[test]
    fn nested_call_splits_inner_arguments() -> Result<(), String> {
        match split_call_arguments_text("foo(inner(2))", "inner") {
            Some(arguments) if arguments == vec!["2".to_string()] => Ok(()),
            other => Err(format!("expected [\"2\"], got {other:?}")),
        }
    }

    #[test]
    fn receiver_path_and_spaced_calls_stay_rejected() -> Result<(), String> {
        for (text, callee) in [
            ("word.validate()", "validate"),
            ("internal::inner(..)", "inner"),
            ("inner (2)", "inner"),
            ("my_inner(2)", "inner"),
        ] {
            if is_direct_call_site(text, callee) {
                return Err(format!("expected no direct site in {text:?}"));
            }
            if split_call_arguments_text(text, callee).is_some() {
                return Err(format!("expected no split in {text:?}"));
            }
        }
        Ok(())
    }
}
