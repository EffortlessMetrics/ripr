//! Bounded fail-closed reachability authority for libtest-mimic trial
//! subjects (#3636).
//!
//! #3635 named the dead-construction over-credit: every statically
//! collected `Trial::test("name", cb)` in a registered target entered the
//! executable-test denominator, including constructors in unused helpers
//! or collections never passed to the harness's run entry point. This
//! authority answers one bounded question per trial: can the trial's
//! construction reach the registered run entry point's trial argument
//! through the resolution forms a token scanner can establish?
//!
//! ## Anchor
//!
//! The run entry point is the exact registered marker path spelled
//! qualified (`<marker>::run`) or a bare `run` bound by a top-level
//! `use` from the marker — the same import-resolution machinery the
//! trial scanner itself uses. A bare `run` with conflicting imports
//! never anchors (fail closed). Method-position `x.run(...)` never
//! anchors, and calls inside dormant `macro_rules!` templates never
//! anchor.
//!
//! ## Supported resolution forms
//!
//! From the run call's second argument (the libtest-mimic trial
//! collection position), the resolver follows exactly:
//!
//! - direct trial-construction containment: every significant token of
//!   the (container-peeled) argument lies inside an anchored trial
//!   invocation span — `vec![Trial::test(..), ..]` and array literals,
//!   including trials collected inside other macros' token trees;
//! - `&`, `&mut`, `vec![..]`, `[..]`, and `local[..]` container
//!   peeling;
//! - immutable let-bound chains in the same function body, at block
//!   depth zero, bound before the run call, with a simple identifier
//!   pattern (`let trials = ..;`). A `mut` binding, a duplicate binding
//!   of one name, or a missing binding fails closed;
//! - one level of builder-function resolution: a bare-identifier call
//!   whose name resolves to exactly one top-level function under the
//!   same fail-closed gates the callback resolver uses
//!   ([`super::resolve_helper_function`]). Inside the builder body, the
//!   same containment and let-chain forms apply under a token
//!   accountancy: every significant token must lie inside a trial span,
//!   inside a parsed `let` statement, inside the `vec!`/array literal
//!   scaffolding, or be a trailing reference to a resolved binding.
//!   Builders called from builders are not resolved (one level is the
//!   contract).
//!
//! The hop budget is bounded ([`MAX_HOPS`]); exhausting it fails closed.
//!
//! ## Fail-closed map
//!
//! - a trial whose invocation is visible inside a resolved span is
//!   [`TrialReachability::Reachable`] — admitted, no disclosure;
//! - everything the resolver cannot connect or exclude is
//!   [`TrialReachability::Unknown`] — admitted (today's behavior) and
//!   disclosed by one aggregate typed limitation
//!   (`registration_reachability_unknown`) naming the trials. Unknown is
//!   the bias: a false unreachable silently drops a real subject, which
//!   is worse than a disclosed unknown;
//! - [`TrialReachability::Unreachable`] requires proof: either no
//!   supported run entry call exists in the target at all
//!   ([`UnreachableReason::RunEntryAbsent`]), or every anchored run
//!   argument resolved completely and the trial is not in the union
//!   ([`UnreachableReason::ExcludedByResolvedArguments`]). A target that
//!   calls an unsupported entry spelling (`run_tests`) — or anchors no
//!   run call while a bare `run` import conflicts — never concludes
//!   absence; the trials stay and are disclosed as unknown.
//!
//! Unreachable trials keep their subject fact and syntactic
//! `HarnessSubjectClaim::NamedInvocation` (a named invocation exists in
//! the registered target); their executable `TestFact` is withheld and a
//! per-trial `registration_unreachable` limitation names them. There is
//! no per-subject reachability field: the unknown bucket is exactly the
//! case where per-subject attribution is not reliable, so the disclosure
//! is aggregate and typed.

use super::{
    at_path_start, enclosing_body_binds_name, inside_macro_rules, is_trivia, matching_group_close,
    matching_group_open, next_significant, previous_significant, resolve_helper_function,
    top_level_use_bindings,
};
use crate::analysis::facts::model::RustIndex;
use crate::analysis::syntax::ra::{LineIndex, slice_text};
use ra_ap_syntax::ast::{self, HasName};
use ra_ap_syntax::{AstNode, SyntaxKind, SyntaxNode, SyntaxToken, TextSize};
use std::collections::BTreeSet;
use std::path::Path;

/// Bound on let-chain and container-peel hops per resolution. Reaching
/// the budget leaves the connection unresolved (fail closed to unknown).
const MAX_HOPS: usize = 8;

/// Cap on trial names listed inside one limitation detail.
const MAX_NAMES_IN_DETAIL: usize = 12;

/// One anchored trial invocation in the token stream, in inclusive token
/// index coordinates.
pub(super) struct PendingTrialSpan {
    pub name: String,
    pub start: usize,
    pub end: usize,
}

/// The reachability verdict for one trial.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TrialReachability {
    /// The construction provably reaches a resolved run argument.
    Reachable,
    /// The bounded resolver could neither connect nor exclude the
    /// construction. Admitted, disclosed by the aggregate limitation.
    Unknown,
    /// Provably excluded from every anchored run argument.
    Unreachable(UnreachableReason),
}

/// Why a trial was provably excluded from the executable-test denominator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UnreachableReason {
    /// No supported run entry call exists in the target at all: the pure
    /// dead-construction case.
    RunEntryAbsent,
    /// Every anchored run argument resolved completely and this trial's
    /// construction is not inside any of them.
    ExcludedByResolvedArguments,
}

/// The per-trial verdicts plus the aggregate unknown disclosure payload.
pub(super) struct ReachabilityOutcome {
    /// One verdict per input trial, in input order.
    pub verdicts: Vec<TrialReachability>,
    /// Detail for the aggregate `registration_reachability_unknown`
    /// limitation, when at least one trial stayed on the unknown path.
    pub unknown_detail: Option<String>,
    /// Line the aggregate limitation is recorded at (the first anchored
    /// run call, else the target head).
    pub disclosure_line: usize,
}

/// Everything the authority reads from one registered target's scan:
/// the target identity, its source text, its parsed tree, its full
/// token stream with parallel start offsets, and its line index. The
/// parsed index is passed to classification separately so the caller's
/// mutable access ends when classification does.
pub(super) struct TargetScan<'a> {
    pub target: &'a Path,
    pub source: &'a str,
    pub file_syntax: &'a SyntaxNode,
    pub tokens: &'a [SyntaxToken],
    pub token_starts: &'a [u32],
    pub line_index: &'a LineIndex,
}

/// Classify every pending trial against the target's run entry points.
pub(super) fn classify_trial_reachability(
    scan: &TargetScan,
    index: &RustIndex,
    marker: &str,
    trials: &[PendingTrialSpan],
) -> ReachabilityOutcome {
    let TargetScan {
        target: _,
        source,
        file_syntax,
        tokens,
        token_starts,
        line_index,
    } = *scan;
    if trials.is_empty() {
        return ReachabilityOutcome {
            verdicts: Vec::new(),
            unknown_detail: None,
            disclosure_line: 1,
        };
    }
    let mut run_calls = Vec::new();
    let mut ambiguous_bare_run = false;
    let mut unbound_imported_run_call = false;
    let mut foreign_suffix_run_call = false;
    let mut shadowed_bare_run = false;
    let run_bindings = top_level_use_bindings(file_syntax, "run");
    let aliased_run_locals = aliased_run_entry_locals(file_syntax);
    for position in 0..tokens.len() {
        let Some(matched) = match_run_path(tokens, position, marker) else {
            continue;
        };
        if matched.foreign {
            // A foreign suffix shape (`wrapper::libtest_mimic::run(`) is
            // unsupported entry evidence: it cannot anchor, and trials
            // may feed it through an unanchored real entry (#3639
            // review).
            if inside_macro_rules(tokens[position].parent_ancestors()) {
                continue;
            }
            foreign_suffix_run_call = true;
            continue;
        }
        // A `run` inside a `macro_rules!` template is dormant, exactly
        // like a trial template: it never anchors the entry point.
        if inside_macro_rules(tokens[position].parent_ancestors()) {
            continue;
        }
        let Some(close) = matching_group_close(tokens, matched.open_paren_index) else {
            continue;
        };
        if matched.qualified {
            let call = build_run_call(
                tokens,
                token_starts,
                line_index,
                position,
                matched.open_paren_index,
                close,
            );
            run_calls.push(call);
            continue;
        }
        match resolve_run_binding(&run_bindings, marker) {
            RunBindingResolution::MarkerAnchored => {
                let call = build_run_call(
                    tokens,
                    token_starts,
                    line_index,
                    position,
                    matched.open_paren_index,
                    close,
                );
                // A local binding, closure parameter, or function
                // parameter named `run` shadows the import inside its
                // scope: that call is not the registered entry point
                // (#3639 review). The call becomes unsupported entry
                // evidence instead of an anchor; a call inside a macro
                // token tree carries no scope to check and equally
                // cannot anchor.
                let shadowed = match call.scope {
                    Some(scope) => {
                        let body_text = slice_text(
                            source,
                            TextSize::from(token_starts[scope.full.0]),
                            tokens[scope.full.1].text_range().end(),
                        );
                        enclosing_body_binds_name(&body_text, "run")
                    }
                    None => true,
                };
                if shadowed {
                    shadowed_bare_run = true;
                } else {
                    run_calls.push(call);
                }
            }
            // A bare `run` the imports cannot tie to the marker may or may
            // not be the harness entry; absence of an anchored call must
            // not conclude unreachability while it exists.
            RunBindingResolution::Ambiguous => ambiguous_bare_run = true,
            // A bare `run` bound from a non-marker path (`use
            // crate::adapter::run;`) is possibly a re-export of the
            // harness entry: it cannot anchor the entry, but its presence
            // must not conclude unreachability either (#3639 review).
            RunBindingResolution::Unbound if !run_bindings.is_empty() => {
                unbound_imported_run_call = true;
            }
            RunBindingResolution::Unbound => {}
        }
    }
    // An aliased import of a `run` path (`use libtest_mimic::run as
    // execute;`) produces a call the scanner cannot anchor (#3639
    // review): its presence is unsupported entry evidence.
    let aliased_run_call_present = !aliased_run_locals.is_empty()
        && tokens.iter().enumerate().any(|(position, token)| {
            token.kind() == SyntaxKind::IDENT
                && aliased_run_locals.contains(token.text())
                && tokens
                    .get(position + 1)
                    .is_some_and(|next| next.kind() == SyntaxKind::L_PAREN)
                && !inside_macro_rules(token.parent_ancestors())
        });

    // Run-entry absence is concluded only when no supported spelling
    // anchors AND no unsupported shape is present that could be the real
    // entry point. Unsupported evidence is recorded regardless of
    // anchored calls (#3639 review): trials may feed an unanchored entry
    // beside a resolved one, so it also clears the completeness premise
    // below instead of only gating absence.
    let unsupported_entry_reason = if ambiguous_bare_run {
        Some(format!(
            "a bare `run` invocation cannot be tied to marker `{marker}` (conflicting imports bind `run` from more than one path), so the run entry point is not anchored",
            marker = marker
        ))
    } else if unbound_imported_run_call {
        Some(format!(
            "a bare `run` invocation is bound from a path that cannot be established as `{marker}::run` (possibly a re-export), so the run entry point is not anchored",
            marker = marker
        ))
    } else if aliased_run_call_present {
        Some(format!(
            "a `run` path is imported under an alias and invoked, which the scanner cannot anchor to `{marker}::run`, so the run entry point is not anchored",
            marker = marker
        ))
    } else if shadowed_bare_run {
        Some(
            "a bare `run` invocation is shadowed by a local binding in its enclosing scope, so the run entry point is not anchored"
                .to_string(),
        )
    } else if foreign_suffix_run_call {
        Some(format!(
            "a qualified call matches the `{marker}::run` suffix inside a longer foreign path, so the run entry point is not anchored",
            marker = marker
        ))
    } else if run_tests_call_present(tokens) {
        Some(format!(
            "the target calls an unsupported harness entry spelling (`run_tests`); only `{marker}::run` is resolved",
            marker = marker
        ))
    } else {
        None
    };
    let run_entry_absent = run_calls.is_empty() && unsupported_entry_reason.is_none();

    let mut reachable: BTreeSet<usize> = BTreeSet::new();
    // With unsupported entry evidence there is no complete premise to
    // reason over even when an anchored call also resolves: trials may
    // feed the unanchored entry, so every non-anchored trial stays
    // unknown, never excluded (#3639 review).
    let mut arguments_complete = !run_calls.is_empty() && unsupported_entry_reason.is_none();
    let mut first_incomplete_reason: Option<String> = unsupported_entry_reason;
    if !run_calls.is_empty() {
        let resolver = Resolver {
            scan,
            index,
            trials,
        };
        for call in &run_calls {
            let arguments = split_call_arguments(tokens, call.open, call.close);
            if arguments.len() != 2 {
                arguments_complete = false;
                if first_incomplete_reason.is_none() {
                    first_incomplete_reason = Some(format!(
                        "a `{marker}::run` call does not have the libtest-mimic two-argument shape",
                        marker = marker
                    ));
                }
                continue;
            }
            let resolution = resolver.resolve_argument(
                arguments[1].0,
                arguments[1].1,
                call.scope,
                call.open,
                0,
                0,
            );
            reachable.extend(resolution.trials);
            if !resolution.complete {
                arguments_complete = false;
                if first_incomplete_reason.is_none() {
                    first_incomplete_reason = Some(resolution.reason.unwrap_or_else(|| {
                        format!(
                            "the `{marker}::run` trial argument did not fully resolve through the supported forms",
                            marker = marker
                        )
                    }));
                }
            }
        }
    }

    let mut unknown_names = Vec::new();
    let verdicts = trials
        .iter()
        .enumerate()
        .map(|(position, trial)| {
            if reachable.contains(&position) {
                TrialReachability::Reachable
            } else if run_entry_absent {
                TrialReachability::Unreachable(UnreachableReason::RunEntryAbsent)
            } else if !arguments_complete {
                unknown_names.push(trial.name.clone());
                TrialReachability::Unknown
            } else {
                TrialReachability::Unreachable(UnreachableReason::ExcludedByResolvedArguments)
            }
        })
        .collect();
    let unknown_detail = if unknown_names.is_empty() {
        None
    } else {
        let reason = first_incomplete_reason.unwrap_or_else(|| {
            format!(
                "the `{marker}::run` trial argument did not fully resolve through the supported forms",
                marker = marker
            )
        });
        Some(format!(
            "{reason}; the trials ({names}) remain in the executable-test denominator under the syntactic claim and reachability is disclosed here rather than per subject (#3636)",
            names = name_list(&unknown_names)
        ))
    };
    ReachabilityOutcome {
        verdicts,
        unknown_detail,
        disclosure_line: run_calls.first().map_or(1, |call| call.line),
    }
}

/// Comma-joined trial names, capped so one sprawling target cannot
/// produce an unbounded limitation detail.
fn name_list(names: &[String]) -> String {
    if names.len() <= MAX_NAMES_IN_DETAIL {
        return names.join(", ");
    }
    let shown: Vec<&str> = names[..MAX_NAMES_IN_DETAIL]
        .iter()
        .map(String::as_str)
        .collect();
    format!(
        "{}, and {} more",
        shown.join(", "),
        names.len() - MAX_NAMES_IN_DETAIL
    )
}

/// Whether any non-dormant `run_tests(` call shape exists — an
/// unsupported entry spelling that must keep the authority from
/// concluding run-entry absence.
fn run_tests_call_present(tokens: &[SyntaxToken]) -> bool {
    for position in 0..tokens.len() {
        if tokens[position].kind() == SyntaxKind::IDENT
            && tokens[position].text() == "run_tests"
            && !inside_macro_rules(tokens[position].parent_ancestors())
            && next_significant(tokens, position + 1)
                .is_some_and(|next| tokens[next].kind() == SyntaxKind::L_PAREN)
        {
            return true;
        }
    }
    false
}

struct RunPathMatch {
    qualified: bool,
    /// The path matched `<marker>::run (` textually but began mid-path —
    /// a foreign suffix like `wrapper::libtest_mimic::run`. It can never
    /// anchor the entry, and its presence must never conclude absence
    /// (#3639 review).
    foreign: bool,
    open_paren_index: usize,
}

/// The token shapes this authority anchors: `<marker>::run (` (qualified)
/// or a path-start bare `run (`. Mirrors the trial path matcher's
/// separator tolerance (`::` as one token or two `:` tokens inside macro
/// token trees). A method-position `.run(` never anchors.
fn match_run_path(tokens: &[SyntaxToken], position: usize, marker: &str) -> Option<RunPathMatch> {
    let text = |index: usize| tokens.get(index).map(SyntaxToken::text);
    let is_ident_eq =
        |index: usize, expected: &str| text(index).is_some_and(|value| value == expected);
    // Length of the path separator starting at `index`: 1 for `::`, 2 for
    // `:` `:`, or None — the trial matcher's tolerance for macro token
    // trees, where the raw punctuation form is what the tokenizer emits.
    let separator_width = |index: usize| -> Option<usize> {
        match tokens.get(index).map(SyntaxToken::kind) {
            Some(SyntaxKind::COLON2) => Some(1),
            Some(SyntaxKind::COLON)
                if matches!(
                    tokens.get(index + 1).map(SyntaxToken::kind),
                    Some(SyntaxKind::COLON)
                ) =>
            {
                Some(2)
            }
            _ => None,
        }
    };
    for qualified in [false, true] {
        if !qualified {
            // Method-position receivers (`suite.run(..)`) are not the
            // harness entry point.
            if matches!(
                previous_significant(tokens, position).map(|previous| tokens[previous].kind()),
                Some(SyntaxKind::DOT)
            ) {
                continue;
            }
            if !at_path_start(tokens, position, false) {
                continue;
            }
        }
        let mut segments: Vec<&str> = if qualified {
            marker.split("::").collect()
        } else {
            Vec::new()
        };
        segments.push("run");
        let mut cursor = position;
        let mut matched_segments = 0usize;
        for segment in &segments {
            if !is_ident_eq(cursor, segment) {
                break;
            }
            cursor += 1;
            matched_segments += 1;
            if matched_segments < segments.len() {
                let Some(width) = separator_width(cursor) else {
                    break;
                };
                cursor += width;
            }
        }
        // Trivia between the final path segment and the call parenthesis
        // (`run /* args */ (..)`) does not change the call shape
        // (#3639 review).
        let paren = (matched_segments == segments.len()).then(|| {
            next_significant(tokens, cursor)
                .filter(|index| tokens[*index].kind() == SyntaxKind::L_PAREN)
        });
        if let Some(Some(paren)) = paren {
            if qualified && !at_path_start(tokens, position, false) {
                // The marker suffix matched but began mid-path
                // (`wrapper::libtest_mimic::run(`) — a foreign item, not
                // the registered entry: report it as foreign so the
                // caller keeps it as unsupported entry evidence instead
                // of anchoring or concluding absence (#3639 review).
                return Some(RunPathMatch {
                    qualified,
                    foreign: true,
                    open_paren_index: paren,
                });
            }
            return Some(RunPathMatch {
                qualified,
                foreign: false,
                open_paren_index: paren,
            });
        }
    }
    // Neither shape matched: a clean path-start path whose final segment
    // is `run` (e.g. `adapter::run (`) is still possibly the harness
    // entry through a re-export — record it as foreign evidence rather
    // than letting absence conclude (#3639 review). Mid-path positions
    // of legitimate calls never reach this branch, because the bare
    // arm's own path-start gate continues first and the qualified walk
    // of a real call anchored from an earlier position already returned.
    if at_path_start(tokens, position, false)
        && !matches!(
            previous_significant(tokens, position).map(|previous| tokens[previous].kind()),
            Some(SyntaxKind::DOT)
        )
        && let Some(open_paren_index) = run_path_paren(tokens, position)
    {
        return Some(RunPathMatch {
            qualified: true,
            foreign: true,
            open_paren_index,
        });
    }
    None
}

/// The open-paren token index when the tokens from `position` walk a
/// path of identifier segments whose final segment is exactly `run`
/// (`adapter::run (`) — the probe behind unsupported entry evidence for
/// non-marker-prefixed and mid-path qualified run calls (#3639 review).
fn run_path_paren(tokens: &[SyntaxToken], position: usize) -> Option<usize> {
    let mut cursor = position;
    loop {
        match tokens.get(cursor).map(SyntaxToken::kind) {
            Some(SyntaxKind::IDENT) => cursor += 1,
            _ => return None,
        }
        let Some(width) = separator_width_at(tokens, cursor) else {
            break;
        };
        cursor += width;
    }
    // The previous significant token is the walked path's final segment.
    let paren = next_significant(tokens, cursor)?;
    let last_ident_is_run =
        previous_significant(tokens, paren).is_some_and(|index| tokens[index].text() == "run");
    tokens
        .get(paren)
        .filter(|token| token.kind() == SyntaxKind::L_PAREN && last_ident_is_run)
        .map(|_| paren)
}

fn separator_width_at(tokens: &[SyntaxToken], index: usize) -> Option<usize> {
    match tokens.get(index).map(SyntaxToken::kind) {
        Some(SyntaxKind::COLON2) => Some(1),
        Some(SyntaxKind::COLON)
            if matches!(
                tokens.get(index + 1).map(SyntaxToken::kind),
                Some(SyntaxKind::COLON)
            ) =>
        {
            Some(2)
        }
        _ => None,
    }
}

enum RunBindingResolution {
    MarkerAnchored,
    Ambiguous,
    Unbound,
}

fn aliased_run_entry_locals(file_syntax: &ra_ap_syntax::SyntaxNode) -> BTreeSet<String> {
    let mut locals = BTreeSet::new();
    for use_item in file_syntax.children().filter_map(ast::Use::cast) {
        let Some(use_tree) = use_item.use_tree() else {
            continue;
        };
        collect_aliased_run_locals(&use_tree, "", &mut locals);
    }
    locals
}

fn collect_aliased_run_locals(tree: &ast::UseTree, prefix: &str, out: &mut BTreeSet<String>) {
    let path_text = tree.path().map(|path| {
        path.syntax()
            .text()
            .to_string()
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>()
    });
    let child_prefix = match (&path_text, prefix.is_empty()) {
        (Some(path), false) => format!("{prefix}::{path}"),
        (Some(path), true) => path.clone(),
        (None, _) => prefix.to_string(),
    };
    if let Some(list) = tree.use_tree_list() {
        for nested in list.use_trees() {
            collect_aliased_run_locals(&nested, &child_prefix, out);
        }
        return;
    }
    let is_run_path = child_prefix.ends_with("::run") || child_prefix == "run";
    let Some(local) = tree
        .rename()
        .and_then(|rename| rename.name())
        .map(|named| named.text().to_string())
        .filter(|local| !local.is_empty())
    else {
        return;
    };
    if is_run_path {
        out.insert(local);
    }
}

/// Resolve a bare `run` spelling against the marker, mirroring the trial
/// binding resolution exactly: anchored when one import binds the name
/// from the marker path; ambiguous when conflicting bindings exist.
fn resolve_run_binding(bindings: &BTreeSet<String>, marker: &str) -> RunBindingResolution {
    let anchored_full = format!("{marker}::run");
    let alias_suffix = format!("::{anchored_full}");
    let matches_marker =
        |binding: &String| binding == &anchored_full || binding.ends_with(&alias_suffix);
    let anchored = bindings.iter().any(matches_marker);
    let conflicting = bindings
        .iter()
        .filter(|binding| !matches_marker(binding))
        .count();
    if anchored && conflicting == 0 {
        return RunBindingResolution::MarkerAnchored;
    }
    if anchored || conflicting > 1 {
        return RunBindingResolution::Ambiguous;
    }
    RunBindingResolution::Unbound
}

/// The enclosing function scope of a resolved expression: the whole
/// `fn` item range (used for the fail-closed shadow scan, which parses
/// the item) and the inner body-block range (used for depth-zero
/// let-chain lookups).
#[derive(Clone, Copy)]
struct FunctionScope {
    full: (usize, usize),
    body: (usize, usize),
}

/// One anchored `<marker>::run(..)` call in the token stream.
struct RunCall {
    open: usize,
    close: usize,
    line: usize,
    /// Enclosing function scope, when the call's tokens carry a `Fn`
    /// ancestor. `None` (a call collected inside another macro's token
    /// tree) disables let-chain resolution — containment only.
    scope: Option<FunctionScope>,
}

/// Assemble one anchored run call: line and enclosing fn-body range.
fn build_run_call(
    tokens: &[SyntaxToken],
    token_starts: &[u32],
    line_index: &LineIndex,
    path_start: usize,
    open: usize,
    close: usize,
) -> RunCall {
    let line = line_index.line(tokens[path_start].text_range().start());
    let scope = tokens[path_start]
        .parent_ancestors()
        .find_map(ast::Fn::cast)
        .and_then(|function| {
            let full = token_index_range(
                token_starts,
                u32::from(function.syntax().text_range().start()),
                u32::from(function.syntax().text_range().end()),
            )?;
            let body = fn_body_token_range(tokens, token_starts, function.syntax())?;
            Some(FunctionScope { full, body })
        });
    RunCall {
        open,
        close,
        line,
        scope,
    }
}

/// Map a byte range onto the inclusive token-index range covering it.
/// Tokens are in document order, so a binary search over the parallel
/// start offsets is exact.
fn token_index_range(token_starts: &[u32], start: u32, end: u32) -> Option<(usize, usize)> {
    if token_starts.is_empty() || end <= start {
        return None;
    }
    let first = token_starts.partition_point(|&offset| offset < start);
    let last = token_starts
        .partition_point(|&offset| offset < end)
        .checked_sub(1)?;
    (first <= last).then_some((first, last))
}

/// The inner token-index range of one function's body block. This
/// ra_ap_syntax generation's `StmtList` text still carries the
/// enclosing braces, so the brace tokens are trimmed: depth-zero scans
/// must start inside the body block.
fn fn_body_token_range(
    tokens: &[SyntaxToken],
    token_starts: &[u32],
    owner: &SyntaxNode,
) -> Option<(usize, usize)> {
    let owner_fn = ast::Fn::cast(owner.clone())?;
    let block = owner_fn.body()?;
    let range = block.syntax().text_range();
    let (first, last) = token_index_range(
        token_starts,
        u32::from(range.start()),
        u32::from(range.end()),
    )?;
    let inner_first = if tokens
        .get(first)
        .is_some_and(|token| token.kind() == SyntaxKind::L_CURLY)
    {
        first + 1
    } else {
        first
    };
    let inner_last = if tokens
        .get(last)
        .is_some_and(|token| token.kind() == SyntaxKind::R_CURLY)
    {
        last.checked_sub(1)?
    } else {
        last
    };
    (inner_first <= inner_last).then_some((inner_first, inner_last))
}

/// Split the argument spans (inclusive token-index pairs) of one call,
/// on commas at bracket depth one.
fn split_call_arguments(tokens: &[SyntaxToken], open: usize, close: usize) -> Vec<(usize, usize)> {
    let mut arguments = Vec::new();
    let mut depth: usize = 0;
    let mut current: Option<usize> = None;
    for index in open + 1..close {
        match tokens[index].kind() {
            SyntaxKind::L_PAREN | SyntaxKind::L_BRACK | SyntaxKind::L_CURLY => {
                depth += 1;
                current.get_or_insert(index);
            }
            SyntaxKind::R_PAREN | SyntaxKind::R_BRACK | SyntaxKind::R_CURLY => {
                depth = depth.saturating_sub(1);
            }
            SyntaxKind::COMMA if depth == 0 => {
                if let Some(start) = current.take()
                    && let Some(end) = previous_significant(tokens, index)
                    && end >= start
                {
                    arguments.push((start, end));
                }
            }
            _ => {
                current.get_or_insert(index);
            }
        }
    }
    if let Some(start) = current
        && let Some(end) = previous_significant(tokens, close)
        && end >= start
    {
        arguments.push((start, end));
    }
    arguments
}

/// One bounded resolution of an expression span into trial reachability.
struct ArgumentResolution {
    trials: BTreeSet<usize>,
    complete: bool,
    reason: Option<String>,
}

/// The bounded resolver. All coordinates are inclusive token indices.
struct Resolver<'a, 'b> {
    scan: &'b TargetScan<'a>,
    index: &'a RustIndex,
    trials: &'a [PendingTrialSpan],
}

impl<'a, 'b> Resolver<'a, 'b> {
    /// Resolve one expression span. `before` is the run call's open-paren
    /// index; only bindings bound strictly before it can feed the call.
    /// `builder_level` counts entered builder bodies; one level is the
    /// contract.
    fn resolve_argument(
        &self,
        start: usize,
        end: usize,
        scope: Option<FunctionScope>,
        before: usize,
        depth: usize,
        builder_level: usize,
    ) -> ArgumentResolution {
        if depth > MAX_HOPS {
            return ArgumentResolution {
                trials: BTreeSet::new(),
                complete: false,
                reason: Some("the resolution hop budget was exhausted".to_string()),
            };
        }
        let (start, end) = self.peel_containers(start, end);
        let Some((first, last)) = self.significant_bounds(start, end) else {
            // An empty trial argument resolves completely to nothing:
            // the argument-empty exclusion rule.
            return ArgumentResolution {
                trials: BTreeSet::new(),
                complete: true,
                reason: None,
            };
        };
        let contained = self.top_level_trials_within(first, last);
        // An unsupported enclosing form can discard or replace anything
        // inside it, so lexically contained trials are not credited as
        // reachable here — they resolve to unknown with the aggregate
        // disclosure instead (#3639 review).
        let deny = |reason: String| ArgumentResolution {
            trials: BTreeSet::new(),
            complete: false,
            reason: Some(reason),
        };
        if self.all_tokens_inside_trials(first, last) {
            return ArgumentResolution {
                trials: contained,
                complete: true,
                reason: None,
            };
        }
        let significant = self.significant_tokens(first, last);
        // A bare local variable reference: follow its immutable let
        // binding in the same function body.
        if significant.len() == 1 && self.scan.tokens[significant[0]].kind() == SyntaxKind::IDENT {
            let name = self.scan.tokens[significant[0]].text();
            let Some((init_start, init_end, is_mut)) = self.unique_let_binding(scope, name, before)
            else {
                let why = if scope.is_none() {
                    format!(
                        "the run call's enclosing function body could not be located, so the local `{name}` cannot be resolved"
                    )
                } else {
                    format!(
                        "the local `{name}` has no single immutable `let` binding before the run call in the same function body"
                    )
                };
                return deny(why);
            };
            if is_mut {
                return deny(format!(
                    "the local `{name}` is bound `mut`, so later re-assignments cannot be resolved"
                ));
            }
            // An immutable binding to a MUTABLE reference
            // (`let trials = &mut vec![..];`) can grow through later
            // `push` calls between the binding and the run invocation,
            // so the initializer is the whole collection only when no
            // mutation happens in between (#3639 review). A direct
            // `run(&mut vec![..])` argument has no intervening
            // statements and stays supported.
            if let Some(amp) = self.next_significant_within(init_start, init_end)
                && self.scan.tokens[amp].kind() == SyntaxKind::AMP
                && self
                    .next_significant_within(amp + 1, init_end)
                    .is_some_and(|next| self.scan.tokens[next].kind() == SyntaxKind::MUT_KW)
                && self.mutable_growth_between(init_end, before)
            {
                return deny(format!(
                    "the local `{name}` is bound to a mutable reference with later `push` mutations before the run call, so the initializer's trials are not the whole collection"
                ));
            }
            let mut resolution = self.resolve_argument(
                init_start,
                init_end,
                scope,
                before,
                depth + 1,
                builder_level,
            );
            resolution.trials.extend(contained);
            return resolution;
        }
        // A bare-identifier builder call: one level of helper resolution
        // under the same fail-closed gates the callback resolver uses.
        if significant.len() >= 2
            && self.scan.tokens[significant[0]].kind() == SyntaxKind::IDENT
            && self.scan.tokens[significant[1]].kind() == SyntaxKind::L_PAREN
            && matching_group_close(self.scan.tokens, significant[1])
                .is_some_and(|close| Some(close) == significant.last().copied())
        {
            let name = self.scan.tokens[significant[0]].text().to_string();
            if builder_level >= 1 {
                return deny(
                    "builder functions called from builder bodies are not resolved (one level is the contract)"
                        .to_string(),
                );
            }
            return self.resolve_builder_body(&name, scope, depth + 1);
        }
        deny(
            "the expression form is not one of the supported collection, binding, or builder-call forms"
                .to_string(),
        )
    }

    /// Resolve one builder function by tracing its returned expression
    /// (#3639 review): a straight-line body's value is its tail
    /// expression, so only trials reachable from the tail (direct
    /// collection elements and the immutable let bindings they consume)
    /// are admitted; trials in unused bindings or dead statements are
    /// not part of the run argument. `return`, branches, or loops at
    /// body depth zero fail closed to unknown.
    fn resolve_builder_body(
        &self,
        name: &str,
        caller_scope: Option<FunctionScope>,
        depth: usize,
    ) -> ArgumentResolution {
        if depth > MAX_HOPS {
            return ArgumentResolution {
                trials: BTreeSet::new(),
                complete: false,
                reason: Some("the resolution hop budget was exhausted".to_string()),
            };
        }
        // The same fail-closed gates as the callback resolver: shadowing
        // of any kind, ambiguity, or a non-top-level target rejects the
        // builder entirely.
        // The shadow gate parses the whole `fn` item, exactly like the
        // existing callback resolver's enclosing-scope text.
        let enclosing_body_text = caller_scope.map(|scope| {
            slice_text(
                self.scan.source,
                TextSize::from(self.scan.token_starts[scope.full.0]),
                self.scan.tokens[scope.full.1].text_range().end(),
            )
        });
        let Some(function) = resolve_helper_function(
            self.index,
            self.scan.target,
            self.scan.file_syntax,
            enclosing_body_text.as_deref(),
            name,
        ) else {
            return ArgumentResolution {
                trials: BTreeSet::new(),
                complete: false,
                reason: Some(format!(
                    "the builder function `{name}` could not be resolved to exactly one shadow-free top-level function"
                )),
            };
        };
        let Some((fn_first, fn_last)) = self.function_token_range(function.start_line, name) else {
            return ArgumentResolution {
                trials: BTreeSet::new(),
                complete: false,
                reason: Some(format!(
                    "the builder function `{name}` body could not be located"
                )),
            };
        };
        let Some(body_range) = self.function_body_range(function.start_line, name) else {
            return ArgumentResolution {
                trials: BTreeSet::new(),
                complete: false,
                reason: Some(format!(
                    "the builder function `{name}` body could not be located"
                )),
            };
        };
        let builder_scope = FunctionScope {
            full: (fn_first, fn_last),
            body: body_range,
        };
        let (body_first, body_last) = body_range;
        // Trace the builder's RETURNED expression, not every construction
        // in the body (#3639 review): a straight-line body's value is its
        // tail expression, so trials in unused bindings or dead
        // statements are not part of the run argument. Unsupported
        // control flow (`return`, branches, loops) fails closed to
        // unknown — never to reachable.
        let Some((tail_first, tail_last)) = self.builder_tail_expression(body_first, body_last)
        else {
            return ArgumentResolution {
                trials: BTreeSet::new(),
                complete: false,
                reason: Some(format!(
                    "the builder function `{name}` body has no statically-resolvable tail expression (unsupported control flow such as `return` or a branch/loop, or no tail at all), so the returned trials cannot be resolved"
                )),
            };
        };
        let tail_resolution = self.resolve_argument(
            tail_first,
            tail_last,
            Some(builder_scope),
            usize::MAX,
            depth + 1,
            1,
        );
        ArgumentResolution {
            trials: tail_resolution.trials,
            complete: tail_resolution.complete,
            reason: tail_resolution.reason,
        }
    }

    /// The tail expression of a builder body: the significant tokens
    /// after the last depth-zero `;`. `None` when the body contains
    /// unsupported control flow at depth zero — a `return`, branch, or
    /// loop keyword means the returned value is not statically the tail.
    fn builder_tail_expression(
        &self,
        body_first: usize,
        body_last: usize,
    ) -> Option<(usize, usize)> {
        let mut last_semicolon: Option<usize> = None;
        let mut depth: usize = 0;
        for index in body_first..=body_last {
            match self.scan.tokens[index].kind() {
                SyntaxKind::L_PAREN | SyntaxKind::L_BRACK | SyntaxKind::L_CURLY => {
                    depth += 1;
                }
                SyntaxKind::R_PAREN | SyntaxKind::R_BRACK | SyntaxKind::R_CURLY => {
                    depth = depth.saturating_sub(1);
                }
                SyntaxKind::SEMICOLON if depth == 0 => {
                    last_semicolon = Some(index);
                }
                SyntaxKind::RETURN_KW
                | SyntaxKind::IF_KW
                | SyntaxKind::MATCH_KW
                | SyntaxKind::LOOP_KW
                | SyntaxKind::WHILE_KW
                | SyntaxKind::FOR_KW
                    if depth == 0 =>
                {
                    return None;
                }
                _ => {}
            }
        }
        let tail_first = match last_semicolon {
            Some(semicolon) => self
                .next_significant_within(semicolon + 1, body_last)
                .unwrap_or(body_last + 1),
            None => self
                .next_significant_within(body_first, body_last)
                .unwrap_or(body_last + 1),
        };
        if tail_first > body_last {
            // No tail expression: an empty body or one that ends in `;`.
            // The returned value cannot be established (a unit return
            // feeds no run argument, but proving the builder is the
            // argument's value means it returned something) — fail
            // closed via the unsupported-control-flow path.
            return None;
        }
        Some((tail_first, body_last))
    }

    /// Locate the top-level function node matching a resolved builder
    /// fact (name plus start line) and return its whole-item token
    /// range.
    fn function_token_range(&self, start_line: usize, name: &str) -> Option<(usize, usize)> {
        let function = self.top_level_fn_node(start_line, name)?;
        token_index_range(
            self.scan.token_starts,
            u32::from(function.syntax().text_range().start()),
            u32::from(function.syntax().text_range().end()),
        )
    }

    /// The inner body-block range of the same top-level function node.
    fn function_body_range(&self, start_line: usize, name: &str) -> Option<(usize, usize)> {
        let function = self.top_level_fn_node(start_line, name)?;
        fn_body_token_range(self.scan.tokens, self.scan.token_starts, function.syntax())
    }

    /// The top-level function node matching a resolved builder fact
    /// (name plus start line).
    fn top_level_fn_node(&self, start_line: usize, name: &str) -> Option<ast::Fn> {
        self.scan
            .file_syntax
            .children()
            .filter_map(ast::Fn::cast)
            .find(|function| {
                function
                    .name()
                    .map(|item| item.text() == name)
                    .unwrap_or(false)
                    && function
                        .fn_token()
                        .map(|token| self.scan.line_index.line(token.text_range().start()))
                        == Some(start_line)
            })
    }

    /// Whether every significant token in a builder body is accounted
    /// for: inside a trial span or parsed `let` statement, inside the
    /// `vec!`/array literal scaffolding around them, or a trailing
    /// reference to a resolved binding.
    /// The one immutable `let` binding of `name` at block depth zero in
    /// `body`, bound before `before`. Zero or multiple candidates fail.
    fn unique_let_binding(
        &self,
        scope: Option<FunctionScope>,
        name: &str,
        before: usize,
    ) -> Option<(usize, usize, bool)> {
        let (first, last) = scope.map(|function| function.body)?;
        let candidates: Vec<LetBinding> = self
            .depth_zero_let_bindings(first, last, before)
            .into_iter()
            .filter(|binding| binding.name == name)
            .collect();
        if candidates.len() != 1 {
            return None;
        }
        let binding = &candidates[0];
        Some((binding.init_start, binding.init_end, binding.is_mut))
    }

    /// Parse the depth-zero `let` statements of a body range with simple
    /// identifier patterns (`let [mut] name [: type] = init;`).
    fn depth_zero_let_bindings(
        &self,
        first: usize,
        last: usize,
        before: usize,
    ) -> Vec<LetBinding<'a>> {
        let mut bindings = Vec::new();
        let mut depth: usize = 0;
        let mut index = first;
        while index <= last {
            match self.scan.tokens[index].kind() {
                SyntaxKind::L_PAREN | SyntaxKind::L_BRACK | SyntaxKind::L_CURLY => depth += 1,
                SyntaxKind::R_PAREN | SyntaxKind::R_BRACK | SyntaxKind::R_CURLY => {
                    depth = depth.saturating_sub(1);
                }
                SyntaxKind::LET_KW if depth == 0 => {
                    // `if let` / `while let` are not binding statements.
                    let binds = !matches!(
                        previous_significant(self.scan.tokens, index)
                            .map(|previous| self.scan.tokens[previous].kind()),
                        Some(SyntaxKind::IF_KW) | Some(SyntaxKind::WHILE_KW)
                    );
                    if binds && let Some(binding) = self.parse_let_binding(index, last, before) {
                        index = binding.statement_end;
                        bindings.push(binding);
                    }
                }
                _ => {}
            }
            index += 1;
        }
        bindings
    }

    /// Parse one simple `let` binding starting at the `let` keyword, or
    /// `None` for any other pattern shape.
    fn parse_let_binding(
        &self,
        let_index: usize,
        last: usize,
        before: usize,
    ) -> Option<LetBinding<'a>> {
        let mut cursor = next_significant(self.scan.tokens, let_index + 1)?;
        let mut is_mut = false;
        if self.scan.tokens[cursor].kind() == SyntaxKind::MUT_KW {
            is_mut = true;
            cursor = next_significant(self.scan.tokens, cursor + 1)?;
        }
        if self.scan.tokens[cursor].kind() != SyntaxKind::IDENT {
            return None;
        }
        let name = self.scan.tokens[cursor].text();
        cursor = next_significant(self.scan.tokens, cursor + 1)?;
        if self.scan.tokens[cursor].kind() == SyntaxKind::COLON {
            // Skip the type annotation up to the `=` at bracket depth 0.
            let mut depth: usize = 0;
            loop {
                match self.scan.tokens[cursor].kind() {
                    SyntaxKind::L_PAREN
                    | SyntaxKind::L_BRACK
                    | SyntaxKind::L_CURLY
                    | SyntaxKind::L_ANGLE => depth += 1,
                    SyntaxKind::R_PAREN
                    | SyntaxKind::R_BRACK
                    | SyntaxKind::R_CURLY
                    | SyntaxKind::R_ANGLE => depth = depth.saturating_sub(1),
                    SyntaxKind::EQ if depth == 0 => break,
                    SyntaxKind::SEMICOLON => return None,
                    _ => {}
                }
                cursor = next_significant(self.scan.tokens, cursor + 1)?;
                if cursor > last {
                    return None;
                }
            }
        }
        if self.scan.tokens[cursor].kind() != SyntaxKind::EQ {
            return None;
        }
        let init_start = next_significant(self.scan.tokens, cursor + 1)?;
        let mut depth: usize = 0;
        let mut statement_end = None;
        let mut scan = init_start;
        while scan <= last {
            match self.scan.tokens[scan].kind() {
                SyntaxKind::L_PAREN | SyntaxKind::L_BRACK | SyntaxKind::L_CURLY => depth += 1,
                SyntaxKind::R_PAREN | SyntaxKind::R_BRACK | SyntaxKind::R_CURLY => {
                    depth = depth.saturating_sub(1);
                }
                SyntaxKind::SEMICOLON if depth == 0 => {
                    statement_end = Some(scan);
                    break;
                }
                _ => {}
            }
            scan += 1;
        }
        let statement_end = statement_end?;
        if statement_end >= before {
            return None;
        }
        let init_end = previous_significant(self.scan.tokens, statement_end)?;
        if init_end < init_start {
            return None;
        }
        Some(LetBinding {
            name,
            is_mut,
            init_start,
            init_end,
            statement_end,
        })
    }

    /// Strip supported collection containers from an expression span:
    /// leading `&`/`&mut`, a whole-span `vec![..]`, a whole-span `[..]`,
    /// and a `local[..]` range-full index suffix.
    /// Whether a `push(`-shaped mutation of a mutable collection occurs
    /// in the token span between a `&mut` binding's initializer and the
    /// run call (#3639 review).
    fn mutable_growth_between(&self, init_end: usize, before: usize) -> bool {
        (init_end..=before).any(|index| {
            self.scan.tokens[index].kind() == SyntaxKind::IDENT
                && self.scan.tokens[index].text() == "push"
                && self
                    .next_significant_within(index + 1, before)
                    .is_some_and(|next| self.scan.tokens[next].kind() == SyntaxKind::L_PAREN)
        })
    }

    fn peel_containers(&self, start: usize, end: usize) -> (usize, usize) {
        let (mut start, mut end) = (start, end);
        for _ in 0..MAX_HOPS {
            let Some(first) = self.next_significant_within(start, end) else {
                return (start, end);
            };
            let Some(last) = self.previous_significant_within(first, end) else {
                return (start, end);
            };
            if self.scan.tokens[first].kind() == SyntaxKind::AMP {
                start = first + 1;
                if let Some(next) = self.next_significant_within(start, end)
                    && self.scan.tokens[next].kind() == SyntaxKind::MUT_KW
                {
                    start = next + 1;
                }
                continue;
            }
            if self.scan.tokens[first].kind() == SyntaxKind::IDENT
                && self.scan.tokens[first].text() == "vec"
                && let Some(bang) = self.next_significant_within(first + 1, last)
                && self.scan.tokens[bang].kind() == SyntaxKind::BANG
                && let Some(bracket) = self.next_significant_within(bang + 1, last)
                && self.scan.tokens[bracket].kind() == SyntaxKind::L_BRACK
                && matching_group_close(self.scan.tokens, bracket) == Some(last)
            {
                start = bracket + 1;
                end = last - 1;
                continue;
            }
            if self.scan.tokens[first].kind() == SyntaxKind::L_BRACK
                && matching_group_close(self.scan.tokens, first) == Some(last)
            {
                start = first + 1;
                end = last - 1;
                continue;
            }
            // `trials[..]`: the identifier followed by a bracket group
            // whose only content is `..` (a range-full index).
            if self.scan.tokens[last].kind() == SyntaxKind::R_BRACK
                && let Some(open) = matching_group_open(self.scan.tokens, last)
                && open > first
                && previous_significant(self.scan.tokens, open) == Some(first)
                && self.group_is_range_full(open, last)
            {
                end = open - 1;
                continue;
            }
            break;
        }
        (start, end)
    }

    /// Whether the bracket group from `open` to its matching close holds
    /// exactly one `..` token.
    fn group_is_range_full(&self, open: usize, close: usize) -> bool {
        let inner: Vec<usize> = (open + 1..close)
            .filter(|index| !is_trivia(self.scan.tokens[*index].kind()))
            .collect();
        inner.len() == 1 && self.scan.tokens[inner[0]].kind() == SyntaxKind::DOT2
    }

    fn next_significant_within(&self, start: usize, end: usize) -> Option<usize> {
        (start..=end).find(|index| !is_trivia(self.scan.tokens[*index].kind()))
    }

    fn previous_significant_within(&self, start: usize, end: usize) -> Option<usize> {
        (start..=end)
            .rev()
            .find(|index| !is_trivia(self.scan.tokens[*index].kind()))
    }

    fn significant_bounds(&self, start: usize, end: usize) -> Option<(usize, usize)> {
        let first = self.next_significant_within(start, end)?;
        let last = self.previous_significant_within(first, end)?;
        Some((first, last))
    }

    fn significant_tokens(&self, start: usize, end: usize) -> Vec<usize> {
        (start..=end)
            .filter(|index| !is_trivia(self.scan.tokens[*index].kind()))
            .collect()
    }

    /// Anchored trial invocations fully contained in the span.
    fn trials_within(&self, start: usize, end: usize) -> BTreeSet<usize> {
        self.trials
            .iter()
            .enumerate()
            .filter(|(_, trial)| trial.start >= start && trial.end <= end)
            .map(|(position, _)| position)
            .collect()
    }

    /// Anchored trial invocations contained in the span that are not
    /// nested inside another trial invocation (#3639 review): a
    /// `Trial::test` constructed inside another trial's callback is
    /// textually contained by the collection span but is not itself an
    /// element registered with the harness, so it must not be credited
    /// as reachable through direct containment.
    fn top_level_trials_within(&self, start: usize, end: usize) -> BTreeSet<usize> {
        // Nesting counts only containers INSIDE the resolved span: a
        // trial built by a nested run call deeper in the target has its
        // containing trial outside this span and stays a top-level
        // element of its own argument (#3639 review).
        let within = self.trials_within(start, end);
        within
            .iter()
            .filter(|&&position| {
                let trial = &self.trials[position];
                !within.iter().any(|&other| {
                    other != position
                        && self.trials[other].start <= trial.start
                        && trial.end <= self.trials[other].end
                })
            })
            .copied()
            .collect()
    }

    /// Whether every significant token in the span is accounted for by a
    /// top-level trial element or literal scaffolding: direct trial
    /// collections in every literal shape, including the commas that
    /// separate multiple elements (#3639 review) and `mut` in
    /// `&mut`/`mut` receiver positions.
    fn all_tokens_inside_trials(&self, start: usize, end: usize) -> bool {
        (start..=end)
            .filter(|index| !is_trivia(self.scan.tokens[*index].kind()))
            .all(|index| {
                is_literal_scaffolding(&self.scan.tokens[index])
                    || self
                        .trials
                        .iter()
                        .any(|trial| index >= trial.start && index <= trial.end)
            })
    }
}

/// One parsed simple `let` binding.
struct LetBinding<'a> {
    name: &'a str,
    is_mut: bool,
    init_start: usize,
    init_end: usize,
    statement_end: usize,
}

/// Tokens that only ever scaffold a trial collection literal and can
/// never introduce a construction by themselves. `MUT_KW` covers the
/// `&mut`/`mut` receiver positions (`run(&mut vec![..])`, mutable slice
/// literals) that never change which trials the argument contains
/// (#3639 review).
fn is_literal_scaffolding(token: &SyntaxToken) -> bool {
    matches!(
        token.kind(),
        SyntaxKind::L_BRACK
            | SyntaxKind::R_BRACK
            | SyntaxKind::COMMA
            | SyntaxKind::SEMICOLON
            | SyntaxKind::AMP
            | SyntaxKind::MUT_KW
            | SyntaxKind::DOT2
            | SyntaxKind::BANG
    ) || (token.kind() == SyntaxKind::IDENT && token.text() == "vec")
}
