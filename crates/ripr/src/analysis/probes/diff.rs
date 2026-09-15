use super::super::classify::is_wildcard_discard_binding;
use super::super::diff::{ChangedFile, ChangedLine};
use super::super::rust_index::{
    RustIndex, SyntaxNodeFact, changed_nodes_for_lines, extract_identifier_tokens, find_file_facts,
    find_owner_function,
};
use super::binding_predicate::{
    BindingPredicateResolution, ChangedBindingPredicateUse, masked_brace_delta, masked_paren_delta,
    resolve_changed_binding_uses,
};
use super::classify::{parser_probe_shapes_for_changed_line, should_ignore_changed_line};
use super::expectations::{expected_sinks, required_oracles};
use super::family::delta_for_family;
use super::ids::{diff_probe_id, normalize_expression};
use super::lexical::classify_changed_line;
use super::subprocess::bounded_subprocess_family;
use crate::analysis::extract::mask_comments_and_strings;
use crate::analysis::facts::cfg_predicates::{attributes_require_test, split_leading_attribute};
use crate::analysis::language::changed_let_binding;
use crate::domain::{Probe, ProbeFamily, SourceLocation};
use std::path::Path;

/// One seeded probe plus the #3294 changed-binding relation when the
/// probe was retargeted from a changed `let` initializer to its
/// same-function predicate use.
pub(crate) type ProbeWithRelation = (Probe, Option<ChangedBindingPredicateUse>);

/// Test surface: the probe vec without the #3294 relations. Production
/// callers use [`probes_for_file_with_relations`].
#[cfg(test)]
pub(crate) fn probes_for_file(root: &Path, changed: &ChangedFile, index: &RustIndex) -> Vec<Probe> {
    probes_for_file_with_relations(root, changed, index)
        .into_iter()
        .map(|(probe, _)| probe)
        .collect()
}

pub(crate) fn probes_for_file_with_relations(
    root: &Path,
    changed: &ChangedFile,
    index: &RustIndex,
) -> Vec<ProbeWithRelation> {
    let mut probes = Vec::new();
    // Use `new_side_line` for all lines: for added lines this equals `line`; for
    // removed lines `new_side_line` is the new-file coordinate, which is what
    // the RustIndex (built from the new file) expects (RANK-1 fix, #1222).
    let changed_lines = changed
        .added_lines
        .iter()
        .chain(changed.removed_lines.iter())
        .map(|line| line.new_side_line)
        .collect::<Vec<_>>();
    let changed_nodes = changed_nodes_for_lines(index, &changed.path, &changed_lines);
    // Inline test-module scopes are file structure, not per-line evidence:
    // build once per file so ownerless-line checks stay linear (#3718).
    let test_module_ranges = test_module_ranges_for(index, &changed.path);
    let build_context = ProbeBuildContext {
        root,
        changed,
        index,
        changed_nodes: &changed_nodes,
    };
    let mut emitted_parser_shapes = Vec::<(usize, String)>::new();

    for added in &changed.added_lines {
        let text = added.text.trim();
        if should_ignore_changed_line(text) {
            continue;
        }
        if changed_line_is_test_evidence(
            index,
            &changed.path,
            added.new_side_line,
            &test_module_ranges,
        ) {
            continue;
        }
        // RIPR-SPEC-0178 (source authority): a bounded literal allowlisted
        // subprocess adapter is recognized ahead of the parser-shape pipeline so
        // the adapter family is not reclassified by generic syntax shapes.
        if let Some(family) =
            bounded_subprocess_family(index, &changed.path, added.new_side_line, text)
        {
            probes.push((
                build_probe(
                    &build_context,
                    added,
                    family,
                    nearby_removed_line(added.new_side_line, text, changed),
                    Some(text.to_string()),
                ),
                None,
            ));
            continue;
        }
        let parser_shapes =
            parser_probe_shapes_for_changed_line(index, &changed.path, added.new_side_line, text);
        let parser_shapes = parser_shapes
            .into_iter()
            .filter(|shape| shape.family != ProbeFamily::CallDeletion || shape.standalone_call)
            .collect::<Vec<_>>();
        let canonical_shapes = parser_shapes
            .iter()
            .filter(|shape| {
                changed
                    .added_lines
                    .iter()
                    .any(|line| line.new_side_line == shape.start_line)
            })
            .cloned()
            .collect::<Vec<_>>();
        if !canonical_shapes.is_empty() {
            for shape in canonical_shapes {
                let key = (shape.start_byte, shape.family.as_str().to_string());
                if emitted_parser_shapes.iter().any(|current| current == &key) {
                    continue;
                }
                emitted_parser_shapes.push(key);
                let canonical_text = canonical_probe_text(text, shape.text);
                let canonical_line = ChangedLine {
                    line: shape.start_line,
                    new_side_line: shape.start_line,
                    text: canonical_text.clone(),
                };
                probes.push((
                    build_probe(
                        &build_context,
                        &canonical_line,
                        shape.family,
                        nearby_removed_line(shape.start_line, &canonical_text, changed),
                        Some(canonical_text),
                    ),
                    None,
                ));
            }
            continue;
        }
        if !parser_shapes.is_empty() {
            for shape in parser_shapes {
                probes.push((
                    build_probe(
                        &build_context,
                        added,
                        shape.family,
                        nearby_removed_line(added.new_side_line, text, changed),
                        Some(text.to_string()),
                    ),
                    None,
                ));
            }
            continue;
        }
        let families = classify_changed_line(text);
        // #3294: a changed simple `let` initializer that only lands in the
        // static-unknown catch-all retargets to its same-function predicate
        // uses when the binding reaches them directly. The changed
        // initializer stays causal evidence (before/after), so the finding
        // names the actual behavioral predicate instead of a generic
        // changed-syntax limitation.
        if families == [ProbeFamily::StaticUnknown]
            && let Some(retargeted) =
                retarget_changed_binding_predicates(&build_context, added, text, &changed_lines)
        {
            probes.extend(retargeted);
            continue;
        }
        for family in families {
            probes.push((
                build_probe(
                    &build_context,
                    added,
                    family,
                    nearby_removed_line(added.new_side_line, text, changed),
                    Some(text.to_string()),
                ),
                None,
            ));
        }
    }

    for removed in &changed.removed_lines {
        let text = removed.text.trim();
        if should_ignore_changed_line(text) {
            continue;
        }
        // Use new_side_line so the owner lookup queries the new-file index at the
        // correct position (RANK-1 fix: `removed.line` is an old-side coordinate
        // and diverges from the new file when an earlier hunk shifted lines).
        if changed_line_is_test_evidence(
            index,
            &changed.path,
            removed.new_side_line,
            &test_module_ranges,
        ) {
            continue;
        }
        for family in classify_changed_line(text) {
            if has_matching_added_line(removed, &family, changed) {
                continue;
            }
            probes.push((
                build_probe(
                    &build_context,
                    removed,
                    family,
                    Some(text.to_string()),
                    None,
                ),
                None,
            ));
        }
    }

    // Post-hoc collision de-dup: if two probes got the same id, append .2, .3, …
    // to the 2nd+ occurrences (the first keeps its id as-is, i.e. ordinal 1).
    dedup_probe_ids(&mut probes);

    probes
}

/// Retarget a changed simple `let` line to the same-function predicate
/// uses of its binding (#3294). Fails closed to `None` — keeping the
/// generic static-unknown path — unless at least one direct use
/// survives the scope rules at a line the diff itself does not change
/// (a directly changed predicate already carries its own probe).
fn retarget_changed_binding_predicates(
    context: &ProbeBuildContext<'_>,
    added: &ChangedLine,
    text: &str,
    changed_lines: &[usize],
) -> Option<Vec<ProbeWithRelation>> {
    let (binding, initializer) = changed_let_binding(text)?;
    // Only a complete single-line declaration retargets: a multi-line
    // initializer's first added line would otherwise retarget with a
    // truncated initializer (and a false plain-value resolution), so it
    // fails closed to the generic per-line probes (#3294 review).
    if !text.ends_with(';') || masked_paren_delta(text) != 0 || masked_brace_delta(text) != 0 {
        return None;
    }
    let owner = find_owner_function(context.index, &context.changed.path, added.new_side_line)?;
    let resolution = resolve_changed_binding_uses(
        binding,
        initializer,
        &owner.body,
        owner.start_line,
        added.new_side_line,
    );
    let uses = match resolution {
        BindingPredicateResolution::DirectUses(uses) if !uses.is_empty() => uses,
        _ => return None,
    };
    // The removed counterpart of this replacement, when it is the same
    // binding's old initializer, becomes the probe's `before` evidence.
    let before_initializer = nearby_removed_line(added.new_side_line, text, context.changed)
        .and_then(|removed_text| {
            changed_let_binding(&removed_text).map(|(removed_binding, removed_initializer)| {
                (removed_binding.to_string(), removed_initializer.to_string())
            })
        })
        .and_then(|(removed_binding, removed_initializer)| {
            (removed_binding == binding).then_some(removed_initializer)
        });
    let mut retargeted = Vec::new();
    for use_site in uses {
        if changed_lines.contains(&use_site.predicate_line) {
            continue;
        }
        let predicate_line = ChangedLine {
            line: use_site.predicate_line,
            new_side_line: use_site.predicate_line,
            text: use_site.predicate_expression.clone(),
        };
        retargeted.push((
            build_probe(
                context,
                &predicate_line,
                ProbeFamily::Predicate,
                before_initializer.clone(),
                Some(use_site.initializer.clone()),
            ),
            Some(use_site),
        ));
    }
    (!retargeted.is_empty()).then_some(retargeted)
}

fn canonical_probe_text(changed_head: &str, parser_expression: &str) -> String {
    let changed_head = changed_head.trim();
    if is_wildcard_discard_binding(changed_head) {
        changed_head.to_string()
    } else {
        parser_expression.to_string()
    }
}

/// Scan `probes` in order; for any id that appears more than once, rewrite the
/// 2nd+ occurrences to append `.2`, `.3`, … (ordinal-based collision suffix).
fn dedup_probe_ids(probes: &mut [ProbeWithRelation]) {
    use std::collections::HashMap;
    let mut seen: HashMap<String, u32> = HashMap::new();
    for (probe, _) in probes.iter_mut() {
        let count = seen.entry(probe.id.0.clone()).or_insert(0);
        *count += 1;
        if *count > 1 {
            probe.id.0 = format!("{}.{}", probe.id.0, count);
        }
    }
}

/// Tests are the instrument, not the surface under test: a probe on a line
/// inside a `#[test]` function (e.g. the error path of a `?` in the test body)
/// is unactionable, because the test failing *is* the discrimination (#1055).
fn changed_line_is_test_evidence(
    index: &RustIndex,
    path: &Path,
    line: usize,
    test_module_ranges: &[InlineModuleRange],
) -> bool {
    if let Some(function) = find_owner_function(index, path, line) {
        return function.source_role.is_evidence_role();
    }
    // Lines outside functions but inside an inline `#[cfg(test)]` module
    // (fixture constants, raw-string data) are test evidence even when the
    // file itself is production (#3718). Unknown module structure fails
    // closed to production-eligible below.
    if line_in_module_ranges(test_module_ranges, line) {
        return true;
    }
    let Some(facts) = find_file_facts(index, path) else {
        return false;
    };
    // Declarations outside functions consume the composer's existing evidence
    // authority (#3695), without guessing from filenames or parsing cfg again.
    // Missing or unresolved provenance cannot remove production eligibility.
    !facts.used_lexical_fallback
        && facts.role_provenance.earliest_unresolved_reason.is_none()
        && facts
            .role_provenance
            .edges
            .iter()
            .any(|edge| edge.requires_test)
}

/// Cap on the upward walk for a module item's attached attribute lines.
/// Attributes attach directly; anything further away is not this item's
/// gate, and the walk must stay bounded on pathological files.
const MAX_ATTRIBUTE_LOOKBEHIND_LINES: usize = 32;

/// One inline module scope: 1-based lines strictly inside the braces.
/// `close_line` is the closing-brace line, or [`usize::MAX`] when the
/// block never closes. The opening and closing lines themselves are
/// never members (fail-closed: bare syntax lines stay eligible).
struct InlineModuleRange {
    open_line: usize,
    close_line: usize,
    requires_test: bool,
}

/// Inline test-module scopes for one file, built once per file (the
/// per-line check is then a membership test, not a rescan).
fn test_module_ranges_for(index: &RustIndex, path: &Path) -> Vec<InlineModuleRange> {
    find_file_facts(index, path)
        .map(|facts| inline_test_module_ranges(&facts.source))
        .unwrap_or_default()
}

/// Whether `line` (1-based) sits inside a `requires_test` inline-module
/// range (#3718).
fn line_in_module_ranges(ranges: &[InlineModuleRange], line: usize) -> bool {
    ranges
        .iter()
        .any(|range| range.requires_test && range.open_line < line && line < range.close_line)
}

/// Whether `line` (1-based) sits inside an inline `mod` block whose own
/// attributes — or any enclosing inline `mod`'s — require a test build
/// (#3718). Unit-test entry point; production calls precompute ranges
/// once per file with [`test_module_ranges_for`].
#[cfg(test)]
fn line_in_cfg_test_module(source: &str, line: usize) -> bool {
    line_in_module_ranges(&inline_test_module_ranges(source), line)
}

/// Collect every inline module scope in `source` as line ranges.
/// Each open entry binds to its module's own opening brace: a block
/// already closed on its opening line (`mod tests {}`) contributes no
/// range, and a same-line nested opener never shifts its parent's
/// entry — only the parent scope (already on the stack) covers the
/// lines beneath.
fn inline_test_module_ranges(source: &str) -> Vec<InlineModuleRange> {
    let masked = mask_comments_and_strings(source);
    let masked_lines: Vec<&str> = masked.lines().collect();
    let original_lines: Vec<&str> = source.lines().collect();
    let mut ranges = Vec::new();
    // Open entries as (entry depth bound to the module's own brace,
    // requires_test, 1-based opening line).
    let mut stack: Vec<(usize, bool, usize)> = Vec::new();
    let mut depth = 0usize;
    for (index, masked_line) in masked_lines.iter().enumerate() {
        let line_number = index + 1;
        if let Some((requires_test, Some(brace))) =
            parse_inline_module(&original_lines, &masked_lines, index)
        {
            // Only a block continuing past this line opens a scope.
            // Out-of-line `mod name;` (no brace index) and self-closed
            // blocks contribute no scope; the composer owns cross-file
            // roles and bare syntax lines stay eligible.
            let (opens, closes) = brace_delta(&masked_line[brace..]);
            if opens > closes {
                stack.push((depth + 1, requires_test, line_number));
            }
        }
        let (opens, closes) = brace_delta(masked_line);
        depth = depth.saturating_add(opens).saturating_sub(closes);
        let mut still_open = Vec::new();
        for (entry_depth, requires_test, open_line) in stack.drain(..) {
            if entry_depth > depth {
                ranges.push(InlineModuleRange {
                    open_line,
                    close_line: line_number,
                    requires_test,
                });
            } else {
                still_open.push((entry_depth, requires_test, open_line));
            }
        }
        stack = still_open;
    }
    for (_, requires_test, open_line) in stack {
        ranges.push(InlineModuleRange {
            open_line,
            close_line: usize::MAX,
            requires_test,
        });
    }
    ranges
}

/// Count `{` and `}` in one pass.
fn brace_delta(line: &str) -> (usize, usize) {
    line.bytes()
        .fold((0, 0), |(opens, closes), byte| match byte {
            b'{' => (opens + 1, closes),
            b'}' => (opens, closes + 1),
            _ => (opens, closes),
        })
}

/// Parse an inline or out-of-line module item starting at line `index`.
/// Structure (the `mod` keyword, name, and opening brace) is read from
/// the masked lines; attribute text is read from the original lines so
/// string-bearing predicates (`feature = "slow"`) keep their literals.
/// Returns (requires_test, byte index of the module's opening `{` in the
/// masked line) — `None` for out-of-line items and non-module lines.
fn parse_inline_module(
    original_lines: &[&str],
    masked_lines: &[&str],
    index: usize,
) -> Option<(bool, Option<usize>)> {
    let mut attributes: Vec<String> = Vec::new();
    let mut masked_rest = masked_lines[index].trim();
    let mut original_rest = original_lines[index].trim();
    // Same-line attributes, in lockstep: structure from masked text, text
    // from the original. Divergence fails closed (fewer attributes).
    while let (Some((_, masked_remainder)), Some((attribute, original_remainder))) = (
        split_leading_attribute(masked_rest),
        split_leading_attribute(original_rest),
    ) {
        attributes.push(attribute.to_string());
        masked_rest = masked_remainder.trim();
        original_rest = original_remainder.trim();
    }
    let masked_rest = strip_visibility(masked_rest);
    let after_mod = masked_rest.strip_prefix("mod")?;
    if after_mod
        .chars()
        .next()
        .is_some_and(|next| next.is_ascii_alphanumeric() || next == '_')
    {
        return None;
    }
    let name_start = masked_rest.len() - after_mod.trim_start().len();
    let name = after_mod.trim_start();
    // Module name: one identifier; anything else is not an item we scope.
    let end = name
        .find(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        .unwrap_or(name.len());
    if end == 0 {
        return None;
    }
    let after_name = &masked_rest[name_start + end..];
    let brace_offset = after_name.find('{')?;
    // A `;` before any `{` is an out-of-line declaration: no scope here.
    if after_name.find(';').is_some_and(|semi| semi < brace_offset) {
        return None;
    }
    // Attached attribute lines directly above: blank masked lines
    // (comments included) are skipped; text comes from the original
    // lines; anything else stops the walk.
    let mut cursor = index;
    let mut skipped = 0usize;
    while cursor > 0 && skipped < MAX_ATTRIBUTE_LOOKBEHIND_LINES {
        cursor -= 1;
        skipped += 1;
        if !masked_lines[cursor].trim().is_empty() {
            let mut pending = original_lines[cursor].trim();
            let mut found = false;
            while let Some((attribute, remainder)) = split_leading_attribute(pending) {
                attributes.push(attribute.to_string());
                pending = remainder.trim();
                found = true;
            }
            if !(found && pending.is_empty()) {
                break;
            }
        }
    }
    Some((
        attributes_require_test(&attributes),
        Some(name_start + end + brace_offset),
    ))
}

/// Strip a leading `pub`, `pub(...)`, or `pub(crate)` visibility qualifier
/// (whitespace between `pub` and the qualifier is legal Rust).
/// Text without a qualifier passes through unchanged; unrecognized shapes
/// fall through so the module parse fails closed below.
fn strip_visibility(text: &str) -> &str {
    let Some(after_pub) = text.strip_prefix("pub") else {
        return text;
    };
    // `pub` followed by an identifier character is another token
    // (`publish`); there is no visibility qualifier here.
    if after_pub
        .chars()
        .next()
        .is_some_and(|next| next.is_ascii_alphanumeric() || next == '_')
    {
        return text;
    }
    let after_pub = after_pub.trim_start();
    match after_pub.strip_prefix('(') {
        Some(rest) => match rest.find(')') {
            Some(close) => rest[close + 1..].trim_start(),
            None => text,
        },
        None => after_pub,
    }
}

struct ProbeBuildContext<'a> {
    root: &'a Path,
    changed: &'a ChangedFile,
    index: &'a RustIndex,
    changed_nodes: &'a [SyntaxNodeFact],
}

fn build_probe(
    context: &ProbeBuildContext<'_>,
    changed_line: &ChangedLine,
    family: ProbeFamily,
    before: Option<String>,
    after: Option<String>,
) -> Probe {
    let text = changed_line.text.trim();
    let delta = delta_for_family(&family);
    // Use `new_side_line` for all index lookups and the SourceLocation: for
    // added lines this equals `line`; for removed lines it is the new-file
    // coordinate, which is what the RustIndex (built from the new file),
    // the flow/value classifiers, and any IDE navigation into the new file
    // require (RANK-1 fix, #1222). #3280 keeps this coordinate in the
    // producer slice — a removed-only probe's revision semantics are stated
    // by its `source_currentness` disposition, and re-coordinating
    // deleted-side evidence is the #3212 projection slice's consumer work.
    let new_line = changed_line.new_side_line;
    let owner = context
        .changed_nodes
        .iter()
        .find(|node| node.start_line <= new_line && new_line <= node.end_line)
        .and_then(|node| node.owner.clone())
        .or_else(|| {
            find_owner_function(context.index, &context.changed.path, new_line)
                .map(|function| function.id.clone())
        });
    let norm_expr = normalize_expression(text);
    // Ordinal 1 here; post-hoc dedup in probes_for_file handles collisions.
    let id = diff_probe_id(
        &context.changed.path,
        &family,
        owner.as_ref(),
        &norm_expr,
        1,
    );
    let expected_sinks = expected_sinks(text, &family);
    let required_oracles = required_oracles(text, &family);

    Probe {
        id,
        location: SourceLocation::new(context.root.join(&context.changed.path), new_line, 1),
        owner,
        family,
        delta,
        before,
        after,
        expression: text.to_string(),
        expected_sinks,
        required_oracles,
    }
}

/// Producer-owned source-currentness resolution for one diff probe
/// (#3212 / #3280).
///
/// `after`-carrying probes are candidate-side by construction: the probe's
/// expression is head-side code. Removed-only probes (`before` set, `after`
/// absent) are base-side evidence: when the same trimmed expression
/// re-appears among the file's added lines, movement evidence exists but the
/// exact candidate identity of the source cannot be established, so the probe is
/// `MovedOrRenamed`; otherwise the source was deleted from the candidate and
/// the probe is `BaseDeleted`. The defensive no-evidence shape stays the
/// explicit unknown rather than guessing.
pub(crate) fn resolve_probe_source_currentness(
    changed: &ChangedFile,
    probe: &Probe,
) -> crate::domain::SourceCurrentness {
    use crate::domain::SourceCurrentness;
    match (&probe.after, &probe.before) {
        (Some(_), _) => SourceCurrentness::CandidateCurrent,
        (None, Some(before)) => {
            let expression_moved = changed
                .added_lines
                .iter()
                .any(|added| added.text.trim() == before.trim());
            if expression_moved {
                SourceCurrentness::MovedOrRenamed
            } else {
                SourceCurrentness::BaseDeleted
            }
        }
        (None, None) => SourceCurrentness::UnresolvedSubject,
    }
}

// A removed line and an added line are considered a plausible pairing only
// when their new-file coordinates are adjacent. Compare `new_side_line` (not
// `line`, the old-side coordinate) on both sides: `new_side_line` is the
// coordinate that stays consistent for removed and added lines alike even
// when an earlier hunk in the same file shifted the old/new line counters
// apart from each other.
//
// A single `abs_diff <= 1` check is not enough for a multi-line replacement
// block (`-a\n-b\n-c\n+x\n+y\n+z`): the diff parser assigns every removed
// line in a contiguous run the SAME `new_side_line` (it only advances on `+`
// or context lines), while the added lines in the paired run advance one per
// line. So `z` (the third added line) sits 2 away from the shared removed
// coordinate even though it belongs to the same replacement. Widen the
// window to the start of the added line's contiguous run so every removed
// line in a same-shaped run is still considered adjacent to every added line
// in the paired run.
fn lines_are_adjacent(removed_new_side_line: usize, added_new_side_line: usize) -> bool {
    removed_new_side_line.abs_diff(added_new_side_line) <= 1
}

fn added_run_start(added_new_side_line: usize, changed: &ChangedFile) -> usize {
    let mut start = added_new_side_line;
    while start > 0
        && changed
            .added_lines
            .iter()
            .any(|line| line.new_side_line == start - 1)
    {
        start -= 1;
    }
    start
}

fn has_matching_added_line(
    removed_line: &ChangedLine,
    removed_family: &ProbeFamily,
    changed: &ChangedFile,
) -> bool {
    let removed_tokens = extract_identifier_tokens(&removed_line.text);
    !removed_tokens.is_empty()
        && changed.added_lines.iter().any(|line| {
            let run_start = added_run_start(line.new_side_line, changed);
            if !lines_are_adjacent(removed_line.new_side_line, line.new_side_line)
                && !lines_are_adjacent(removed_line.new_side_line, run_start)
            {
                return false;
            }
            let added_families = classify_changed_line(line.text.trim());
            if !added_families.iter().any(|family| family == removed_family) {
                return false;
            }
            let added_tokens = extract_identifier_tokens(&line.text);
            added_tokens
                .iter()
                .any(|token| removed_tokens.iter().any(|other| other == token))
        })
}

fn nearby_removed_line(
    added_new_side_line: usize,
    added: &str,
    changed: &ChangedFile,
) -> Option<String> {
    let added_tokens = extract_identifier_tokens(added);
    // Only consider removed lines adjacent to this added line, or to the
    // start of the added line's contiguous run (see `added_run_start`) so a
    // multi-line replacement block still pairs its later added lines with
    // the removed lines it replaced. Falling back to *any* removed line in
    // the file (regardless of hunk or position) previously produced a
    // `before` value taken from an unrelated hunk whenever no removed line
    // shared a token with the added line.
    let run_start = added_run_start(added_new_side_line, changed);
    let nearby = changed
        .removed_lines
        .iter()
        .filter(|line| {
            lines_are_adjacent(line.new_side_line, added_new_side_line)
                || lines_are_adjacent(line.new_side_line, run_start)
        })
        .collect::<Vec<_>>();
    nearby
        .iter()
        .find(|line| {
            let removed_tokens = extract_identifier_tokens(&line.text);
            !added_tokens.is_empty()
                && added_tokens
                    .iter()
                    .any(|token| removed_tokens.iter().any(|other| other == token))
        })
        .or_else(|| nearby.first())
        .map(|line| line.text.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::super::super::diff::ChangedLine;
    use super::super::super::rust_index::{
        FileFacts, FunctionFact, PROBE_SHAPE_CALL_DELETION, PROBE_SHAPE_PREDICATE,
        PROBE_SHAPE_SIDE_EFFECT, ProbeShapeFact, RustIndex,
    };
    use super::*;
    use crate::analysis::facts::{
        FunctionSourceRole, SourceRoleProvenance, SourceRoleProvenanceEdge,
        SourceRoleProvenanceEdgeKind,
    };
    use crate::domain::SymbolId;
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    #[test]
    fn probes_for_file_uses_syntax_shape_owner_and_removed_context() {
        let path = PathBuf::from("src/lib.rs");
        let changed = ChangedFile {
            path: path.clone(),
            added_lines: vec![ChangedLine {
                line: 3,
                new_side_line: 3,
                text: "if amount >= threshold {".to_string(),
            }],
            removed_lines: vec![ChangedLine {
                line: 3,
                new_side_line: 3,
                text: "if amount > threshold {".to_string(),
            }],
        };
        let index = RustIndex {
            files: BTreeMap::from([(
                path.clone(),
                FileFacts {
                    path: path.clone(),
                    functions: vec![FunctionFact {
                        id: SymbolId("pricing::discounted_total".to_string()),
                        name: "discounted_total".to_string(),
                        file: path.clone(),
                        start_line: 1,
                        end_line: 5,
                        body: "fn discounted_total() { if amount >= threshold {} }".to_string(),
                        calls: vec![],
                        returns: vec![],
                        literals: vec![],
                        source_role: FunctionSourceRole::Production,
                        attrs: vec![],
                        nested_fn_names: Vec::new(),
                        let_bindings: Vec::new(),
                    }],
                    probe_shapes: vec![ProbeShapeFact {
                        start_line: 3,
                        end_line: 3,
                        start_byte: 20,
                        kind: PROBE_SHAPE_PREDICATE.to_string(),
                        text: "if amount >= threshold {".to_string(),
                    }],
                    ..FileFacts::default()
                },
            )]),
            ..RustIndex::default()
        };

        let probes = probes_for_file(Path::new("workspace"), &changed, &index);

        assert_eq!(probes.len(), 1);
        let probe = &probes[0];
        assert_eq!(probe.id.0, "probe:src_lib.rs:predicate:b6638ef3");
        assert_eq!(probe.family, ProbeFamily::Predicate);
        assert_eq!(
            probe.owner,
            Some(SymbolId("pricing::discounted_total".to_string()))
        );
        assert_eq!(probe.before, Some("if amount > threshold {".to_string()));
        assert_eq!(probe.after, Some("if amount >= threshold {".to_string()));
        assert!(
            probe
                .expected_sinks
                .iter()
                .any(|sink| sink == "branch result")
        );
    }

    #[test]
    fn probes_for_file_emits_multiline_parser_shape_once() -> Result<(), String> {
        let path = PathBuf::from("src/gate_watchdog.rs");
        let expression = "watchdog_reason(\n    \"run-missing\",\n    receipt,\n)";
        let changed = ChangedFile {
            path: path.clone(),
            added_lines: vec![
                ChangedLine {
                    line: 10,
                    new_side_line: 10,
                    text: "watchdog_reason(".to_string(),
                },
                ChangedLine {
                    line: 11,
                    new_side_line: 11,
                    text: "\"run-missing\",".to_string(),
                },
                ChangedLine {
                    line: 12,
                    new_side_line: 12,
                    text: "receipt,".to_string(),
                },
                ChangedLine {
                    line: 13,
                    new_side_line: 13,
                    text: ")".to_string(),
                },
            ],
            removed_lines: vec![],
        };
        let index = RustIndex {
            files: BTreeMap::from([(
                path.clone(),
                FileFacts {
                    path: path.clone(),
                    source: format!("{expression};"),
                    functions: vec![FunctionFact {
                        id: SymbolId("gate_watchdog::classify".to_string()),
                        name: "classify".to_string(),
                        file: path.clone(),
                        start_line: 1,
                        end_line: 20,
                        body: expression.to_string(),
                        calls: vec![],
                        returns: vec![],
                        literals: vec![],
                        source_role: FunctionSourceRole::Production,
                        attrs: vec![],
                        nested_fn_names: Vec::new(),
                        let_bindings: Vec::new(),
                    }],
                    probe_shapes: vec![ProbeShapeFact {
                        start_line: 10,
                        end_line: 13,
                        start_byte: 0,
                        kind: PROBE_SHAPE_CALL_DELETION.to_string(),
                        text: expression.to_string(),
                    }],
                    ..FileFacts::default()
                },
            )]),
            ..RustIndex::default()
        };

        let probes = probes_for_file(Path::new("workspace"), &changed, &index);
        if probes.len() != 1 {
            return Err(format!(
                "expected one semantic probe for multiline call, got {probes:?}"
            ));
        }
        let Some(probe) = probes.first() else {
            return Err("missing semantic probe".to_string());
        };
        if probe.expression != expression || probe.location.line != 10 {
            return Err(format!("unexpected semantic probe: {probe:?}"));
        }
        Ok(())
    }

    #[test]
    fn probes_for_file_does_not_borrow_unchanged_outer_shape() -> Result<(), String> {
        let path = PathBuf::from("src/hir.rs");
        let changed = ChangedFile {
            path: path.clone(),
            added_lines: vec![ChangedLine {
                line: 12,
                new_side_line: 12,
                text: "storage,".to_string(),
            }],
            removed_lines: vec![],
        };
        let index = RustIndex {
            files: BTreeMap::from([(
                path.clone(),
                FileFacts {
                    path,
                    probe_shapes: vec![ProbeShapeFact {
                        start_line: 10,
                        end_line: 13,
                        start_byte: 100,
                        kind: crate::analysis::rust_index::PROBE_SHAPE_RETURN_VALUE.to_string(),
                        text: "HirLet {\n    name,\n    storage,\n}".to_string(),
                    }],
                    ..FileFacts::default()
                },
            )]),
            ..RustIndex::default()
        };

        let probes = probes_for_file(Path::new("workspace"), &changed, &index);
        let Some(probe) = probes.first() else {
            return Err("expected physical-line fallback probe".to_string());
        };
        if probe.expression != "storage," || probe.family != ProbeFamily::ReturnValue {
            return Err(format!(
                "isolated field edit borrowed unchanged outer shape: {probe:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn probes_for_file_preserves_wildcard_discard_head() -> Result<(), String> {
        for changed_text in [
            "let _ = compute_fee(amount * 9);",
            "let _ : u32 = compute_fee(amount * 9);",
            "let _= compute_fee(amount * 9);",
        ] {
            let path = PathBuf::from("src/lib.rs");
            let changed = ChangedFile {
                path: path.clone(),
                added_lines: vec![ChangedLine {
                    line: 4,
                    new_side_line: 4,
                    text: changed_text.to_string(),
                }],
                removed_lines: vec![],
            };
            let index = RustIndex {
                files: BTreeMap::from([(
                    path.clone(),
                    FileFacts {
                        path,
                        probe_shapes: vec![ProbeShapeFact {
                            start_line: 4,
                            end_line: 4,
                            start_byte: 40,
                            kind: PROBE_SHAPE_SIDE_EFFECT.to_string(),
                            text: "compute_fee(amount * 9)".to_string(),
                        }],
                        ..FileFacts::default()
                    },
                )]),
                ..RustIndex::default()
            };

            let probes = probes_for_file(Path::new("workspace"), &changed, &index);
            let Some(probe) = probes.first() else {
                return Err("expected wildcard-discard call probe".to_string());
            };
            if probe.expression != changed_text {
                return Err(format!("wildcard discard context was erased: {probe:?}"));
            }
            let sinks = crate::analysis::classify::local_flow_sinks(probe, None);
            if !sinks
                .first()
                .is_some_and(|sink| sink.kind == crate::domain::FlowSinkKind::Unknown)
            {
                return Err(format!(
                    "wildcard discard did not produce an unknown sink: {probe:?}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn probes_for_file_skips_lines_owned_by_test_functions() {
        let path = PathBuf::from("src/config.rs");
        let changed = ChangedFile {
            path: path.clone(),
            added_lines: vec![ChangedLine {
                line: 3,
                new_side_line: 3,
                text: "let config = toml::from_str(text)?;".to_string(),
            }],
            removed_lines: vec![],
        };
        let index_with = |source_role: FunctionSourceRole| RustIndex {
            files: BTreeMap::from([(
                path.clone(),
                FileFacts {
                    path: path.clone(),
                    functions: vec![FunctionFact {
                        id: SymbolId("config::tests::parses".to_string()),
                        name: "parses".to_string(),
                        file: path.clone(),
                        start_line: 1,
                        end_line: 5,
                        body: "fn parses() { let config = toml::from_str(text)?; }".to_string(),
                        calls: vec![],
                        returns: vec![],
                        literals: vec![],
                        source_role,
                        attrs: vec![],
                        nested_fn_names: Vec::new(),
                        let_bindings: Vec::new(),
                    }],
                    ..FileFacts::default()
                },
            )]),
            ..RustIndex::default()
        };

        // Control: a production owner still probes the error path.
        let production = probes_for_file(
            Path::new("workspace"),
            &changed,
            &index_with(FunctionSourceRole::Production),
        );
        assert!(
            !production.is_empty(),
            "a non-test error path should still generate a probe"
        );

        // #1055: the same line owned by a `#[test]` function generates nothing —
        // the test is the instrument, not the surface under test.
        let in_test = probes_for_file(
            Path::new("workspace"),
            &changed,
            &index_with(FunctionSourceRole::TestAttribute),
        );
        assert!(
            in_test.is_empty(),
            "a line owned by a test function must not generate probes, got {in_test:?}"
        );
    }

    #[test]
    fn ownerless_fields_consume_only_resolved_test_context() -> Result<(), String> {
        let test_path = PathBuf::from("src/support.rs");
        let production_path = PathBuf::from("src/model.rs");
        let changed = |path: PathBuf| ChangedFile {
            path,
            added_lines: vec![ChangedLine {
                line: 2,
                new_side_line: 2,
                text: "    allowed: usize,".to_string(),
            }],
            removed_lines: vec![],
        };
        let test_provenance = SourceRoleProvenance {
            edges: vec![SourceRoleProvenanceEdge {
                kind: SourceRoleProvenanceEdgeKind::Module,
                parent: PathBuf::from("src/lib.rs"),
                child: test_path.clone(),
                declaration: "mod support;".to_string(),
                line: 2,
                requires_test: true,
            }],
            earliest_unresolved_reason: None,
        };
        let index = RustIndex {
            files: BTreeMap::from([
                (
                    test_path.clone(),
                    FileFacts {
                        path: test_path.clone(),
                        role_provenance: test_provenance,
                        ..FileFacts::default()
                    },
                ),
                (production_path.clone(), FileFacts::default()),
            ]),
            ..RustIndex::default()
        };

        let test_probes = probes_for_file(Path::new("workspace"), &changed(test_path), &index);
        if !test_probes.is_empty() {
            return Err(format!("resolved test field emitted {test_probes:?}"));
        }
        let production_probes =
            probes_for_file(Path::new("workspace"), &changed(production_path), &index);
        if production_probes.len() != 1
            || production_probes.first().map(|probe| &probe.family)
                != Some(&ProbeFamily::FieldConstruction)
        {
            return Err(format!(
                "production field was hidden: {production_probes:?}"
            ));
        }

        for provenance in [
            SourceRoleProvenance::default(),
            SourceRoleProvenance {
                edges: vec![SourceRoleProvenanceEdge {
                    kind: SourceRoleProvenanceEdgeKind::Module,
                    parent: PathBuf::from("src/lib.rs"),
                    child: PathBuf::from("src/support.rs"),
                    declaration: "mod support;".to_string(),
                    line: 2,
                    requires_test: true,
                }],
                earliest_unresolved_reason: Some("rust_module_context_conflict".to_string()),
            },
            SourceRoleProvenance {
                edges: vec![SourceRoleProvenanceEdge {
                    kind: SourceRoleProvenanceEdgeKind::Module,
                    parent: PathBuf::from("src/lib.rs"),
                    child: PathBuf::from("src/support.rs"),
                    declaration: "mod support;".to_string(),
                    line: 2,
                    requires_test: false,
                }],
                earliest_unresolved_reason: None,
            },
        ] {
            let mut conservative_index = index.clone();
            let facts = conservative_index
                .files
                .get_mut(Path::new("src/support.rs"))
                .ok_or("fixture support file is missing")?;
            facts.role_provenance = provenance;
            let probes = probes_for_file(
                Path::new("workspace"),
                &changed(PathBuf::from("src/support.rs")),
                &conservative_index,
            );
            if probes.len() != 1 {
                return Err(format!(
                    "conservative context was incorrectly hidden: {probes:?}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn probes_for_file_falls_back_to_static_unknown_without_syntax_shape() {
        let changed = ChangedFile {
            path: PathBuf::from("src/lib.rs"),
            added_lines: vec![ChangedLine {
                line: 10,
                new_side_line: 10,
                text: "let total = discounted;".to_string(),
            }],
            removed_lines: vec![],
        };

        let probes = probes_for_file(Path::new("workspace"), &changed, &RustIndex::default());

        assert_eq!(probes.len(), 1);
        assert_eq!(probes[0].id.0, "probe:src_lib.rs:static_unknown:1e078e9a");
        assert_eq!(probes[0].family, ProbeFamily::StaticUnknown);
        assert_eq!(probes[0].before, None);
    }

    #[test]
    fn probes_for_file_keeps_removed_only_behavior_changes() {
        let path = PathBuf::from("src/lib.rs");
        let changed = ChangedFile {
            path: path.clone(),
            added_lines: vec![],
            removed_lines: vec![ChangedLine {
                line: 4,
                new_side_line: 4,
                text: "events.publish(invoice);".to_string(),
            }],
        };
        let index = RustIndex {
            files: BTreeMap::from([(
                path.clone(),
                FileFacts {
                    path: path.clone(),
                    functions: vec![FunctionFact {
                        id: SymbolId("billing::record_invoice".to_string()),
                        name: "record_invoice".to_string(),
                        file: path.clone(),
                        start_line: 1,
                        end_line: 6,
                        body: "fn record_invoice() { }".to_string(),
                        calls: vec![],
                        returns: vec![],
                        literals: vec![],
                        source_role: FunctionSourceRole::Production,
                        attrs: vec![],
                        nested_fn_names: Vec::new(),
                        let_bindings: Vec::new(),
                    }],
                    ..FileFacts::default()
                },
            )]),
            ..RustIndex::default()
        };

        let probes = probes_for_file(Path::new("workspace"), &changed, &index);

        assert_eq!(probes.len(), 2);
        let side_effect_position = probes
            .iter()
            .position(|probe| probe.family == ProbeFamily::SideEffect);
        assert_ne!(
            side_effect_position, None,
            "removed side effect should stay visible as a probe"
        );
        let Some(side_effect_position) = side_effect_position else {
            return;
        };
        let side_effect = &probes[side_effect_position];
        assert_eq!(side_effect.id.0, "probe:src_lib.rs:side_effect:682b613e");
        assert_eq!(
            side_effect.before,
            Some("events.publish(invoice);".to_string())
        );
        assert_eq!(side_effect.after, None);
        assert_eq!(side_effect.expression, "events.publish(invoice);");
        assert_eq!(
            side_effect.owner,
            Some(SymbolId("billing::record_invoice".to_string()))
        );
    }

    #[test]
    fn probes_for_file_does_not_duplicate_replacements_as_removed_only_changes() {
        let changed = ChangedFile {
            path: PathBuf::from("src/lib.rs"),
            added_lines: vec![ChangedLine {
                line: 3,
                new_side_line: 3,
                text: "if amount >= threshold {".to_string(),
            }],
            removed_lines: vec![ChangedLine {
                line: 3,
                new_side_line: 3,
                text: "if amount > threshold {".to_string(),
            }],
        };

        let probes = probes_for_file(Path::new("workspace"), &changed, &RustIndex::default());

        assert_eq!(probes.len(), 1);
        assert_eq!(
            probes[0].before,
            Some("if amount > threshold {".to_string())
        );
        assert_eq!(
            probes[0].after,
            Some("if amount >= threshold {".to_string())
        );
    }

    // Regression: an added line adjacent to a removed line that shares no
    // identifier token must still fall back to that *nearby* removed line
    // (not `None`, and not an unrelated line from elsewhere in the file).
    #[test]
    fn probes_for_file_falls_back_to_nearby_removed_line_without_token_match() {
        let changed = ChangedFile {
            path: PathBuf::from("src/lib.rs"),
            added_lines: vec![ChangedLine {
                line: 3,
                new_side_line: 3,
                text: "if enabled {".to_string(),
            }],
            removed_lines: vec![ChangedLine {
                line: 3,
                new_side_line: 3,
                text: "if legacy_flag {".to_string(),
            }],
        };

        let probes = probes_for_file(Path::new("workspace"), &changed, &RustIndex::default());

        // No shared token means the removed line isn't deduped as this added
        // line's replacement (see `has_matching_added_line`), so it also
        // surfaces as its own probe; both are expected here.
        assert_eq!(probes.len(), 2);
        let inserted = probes
            .iter()
            .find(|probe| probe.expression == "if enabled {");
        assert!(
            inserted.is_some(),
            "expected a probe for the added line, got {probes:?}"
        );
        if let Some(inserted) = inserted {
            assert_eq!(inserted.before, Some("if legacy_flag {".to_string()));
        }
    }

    // Regression: in a multi-line replacement block (`-a -b -c +x +y +z`),
    // every removed line in the run shares the SAME `new_side_line` (the
    // diff parser only advances it on `+`/context lines), while the added
    // lines advance one per line. A naive `abs_diff <= 1` window around a
    // single added line therefore loses the removed context entirely for
    // the later added lines in the run (e.g. the third). The later added
    // lines must still recover `before` context from the same block instead
    // of getting `None`.
    #[test]
    fn probes_for_file_pairs_third_added_line_of_multiline_replacement() {
        let changed = ChangedFile {
            path: PathBuf::from("src/lib.rs"),
            added_lines: vec![
                ChangedLine {
                    line: 10,
                    new_side_line: 10,
                    text: "if flag_one_updated {".to_string(),
                },
                ChangedLine {
                    line: 11,
                    new_side_line: 11,
                    text: "if flag_two_updated {".to_string(),
                },
                ChangedLine {
                    line: 12,
                    new_side_line: 12,
                    text: "if flag_three_updated {".to_string(),
                },
            ],
            removed_lines: vec![
                ChangedLine {
                    line: 10,
                    new_side_line: 10,
                    text: "if flag_one {".to_string(),
                },
                ChangedLine {
                    line: 11,
                    new_side_line: 10,
                    text: "if flag_two {".to_string(),
                },
                ChangedLine {
                    line: 12,
                    new_side_line: 10,
                    text: "if flag_three {".to_string(),
                },
            ],
        };

        let probes = probes_for_file(Path::new("workspace"), &changed, &RustIndex::default());

        let third = probes
            .iter()
            .find(|probe| probe.expression == "if flag_three_updated {");
        assert!(
            third.is_some(),
            "expected a probe for the third added line, got {probes:?}"
        );
        if let Some(third) = third {
            assert!(
                third.before.is_some(),
                "the third added line in a multi-line replacement must recover \
                 `before` context from its own block, not None"
            );
        }
    }

    // Regression: an added line with no removed counterpart nearby must not
    // borrow `before` from an unrelated removed line elsewhere in the file.
    #[test]
    fn probes_for_file_does_not_attribute_unrelated_hunk_as_before_context() {
        let path = PathBuf::from("src/lib.rs");
        let changed = ChangedFile {
            path: path.clone(),
            added_lines: vec![
                ChangedLine {
                    line: 3,
                    new_side_line: 3,
                    text: "if amount >= threshold {".to_string(),
                },
                ChangedLine {
                    line: 50,
                    new_side_line: 50,
                    text: "events.notify(user_id);".to_string(),
                },
            ],
            removed_lines: vec![ChangedLine {
                line: 3,
                new_side_line: 3,
                text: "if amount > threshold {".to_string(),
            }],
        };

        let probes = probes_for_file(Path::new("workspace"), &changed, &RustIndex::default());

        let inserted = probes
            .iter()
            .find(|probe| probe.expression == "events.notify(user_id);");
        assert!(
            inserted.is_some(),
            "expected a probe for the pure insertion, got {probes:?}"
        );
        if let Some(inserted) = inserted {
            assert_eq!(
                inserted.before, None,
                "a pure insertion far from any removed line must not borrow \
                 an unrelated hunk's removed text as `before`"
            );
        }
    }

    #[test]
    fn probes_for_file_ignores_non_behavior_lines() {
        let changed = ChangedFile {
            path: PathBuf::from("src/lib.rs"),
            added_lines: vec![
                ChangedLine {
                    line: 1,
                    new_side_line: 1,
                    text: "use crate::pricing;".to_string(),
                },
                ChangedLine {
                    line: 2,
                    new_side_line: 2,
                    text: "// comment".to_string(),
                },
            ],
            removed_lines: vec![],
        };

        let probes = probes_for_file(Path::new("workspace"), &changed, &RustIndex::default());
        assert!(probes.is_empty());
    }

    // #3294: the changed `let` initializer retargets to its same-function
    // predicate use. The probe is predicate-shaped at the predicate line,
    // the initializers stay causal before/after evidence, and the typed
    // relation rides along with the probe.
    #[test]
    fn changed_binding_initializer_retargets_to_predicate_use() -> Result<(), String> {
        let path = PathBuf::from("src/split.rs");
        let body = "pub fn split_first(input: &str, delim: char) -> &str {\n    let end = input.rfind(delim).map_or(0, |idx| idx);\n    let start = delim.chars().next().map_or(0, |c| c.len_utf8());\n    if end == start {\n        &input[..end]\n    } else {\n        input\n    }\n}";
        let changed = ChangedFile {
            path: path.clone(),
            added_lines: vec![ChangedLine {
                line: 2,
                new_side_line: 2,
                text: "let end = input.rfind(delim).map_or(0, |idx| idx);".to_string(),
            }],
            removed_lines: vec![ChangedLine {
                line: 2,
                new_side_line: 2,
                text: "let end = input.rfind(delim).map_or(0, |idx| idx + 1);".to_string(),
            }],
        };
        let index = RustIndex {
            files: BTreeMap::from([(
                path.clone(),
                FileFacts {
                    path: path.clone(),
                    functions: vec![FunctionFact {
                        id: SymbolId("src/split.rs::split_first".to_string()),
                        name: "split_first".to_string(),
                        file: path.clone(),
                        start_line: 1,
                        end_line: 9,
                        body: body.to_string(),
                        calls: vec![],
                        returns: vec![],
                        literals: vec![],
                        source_role: FunctionSourceRole::Production,
                        attrs: vec![],
                        nested_fn_names: Vec::new(),
                        let_bindings: Vec::new(),
                    }],
                    ..FileFacts::default()
                },
            )]),
            ..RustIndex::default()
        };

        let probes = probes_for_file_with_relations(Path::new("workspace"), &changed, &index);

        let [(probe, relation)] = probes.as_slice() else {
            return Err(format!("expected one retargeted probe, got {probes:?}"));
        };
        if probe.family != ProbeFamily::Predicate {
            return Err(format!("expected predicate family, got {probe:?}"));
        }
        if probe.expression != "if end == start {" || probe.location.line != 4 {
            return Err(format!(
                "probe must sit on the predicate use, got {probe:?}"
            ));
        }
        if probe.before.as_deref() != Some("input.rfind(delim).map_or(0, |idx| idx + 1)") {
            return Err(format!("old initializer is causal evidence: {probe:?}"));
        }
        if probe.after.as_deref() != Some("input.rfind(delim).map_or(0, |idx| idx)") {
            return Err(format!("new initializer is causal evidence: {probe:?}"));
        }
        let relation = relation
            .as_ref()
            .ok_or_else(|| "retargeted probe must carry the relation".to_string())?;
        if relation.binding != "end" || relation.predicate_line != 4 {
            return Err(format!(
                "relation must name the binding and use: {relation:?}"
            ));
        }
        Ok(())
    }

    // #3294 control: when the binding never reaches a predicate directly
    // (inner-scope shadowing), the generic static-unknown path stays.
    #[test]
    fn shadowed_binding_initializer_keeps_static_unknown() -> Result<(), String> {
        let path = PathBuf::from("src/lib.rs");
        let body = "pub fn f() -> bool {\n    let end = 1;\n    {\n        let end = 2;\n        end == 3\n    }\n}";
        let changed = ChangedFile {
            path: path.clone(),
            added_lines: vec![ChangedLine {
                line: 2,
                new_side_line: 2,
                text: "let end = 1;".to_string(),
            }],
            removed_lines: vec![],
        };
        let index = RustIndex {
            files: BTreeMap::from([(
                path.clone(),
                FileFacts {
                    path,
                    functions: vec![FunctionFact {
                        id: SymbolId("src/lib.rs::f".to_string()),
                        name: "f".to_string(),
                        file: PathBuf::from("src/lib.rs"),
                        start_line: 1,
                        end_line: 8,
                        body: body.to_string(),
                        calls: vec![],
                        returns: vec![],
                        literals: vec![],
                        source_role: FunctionSourceRole::Production,
                        attrs: vec![],
                        nested_fn_names: Vec::new(),
                        let_bindings: Vec::new(),
                    }],
                    ..FileFacts::default()
                },
            )]),
            ..RustIndex::default()
        };

        let probes = probes_for_file_with_relations(Path::new("workspace"), &changed, &index);

        let [(probe, relation)] = probes.as_slice() else {
            return Err(format!("expected the generic probe only, got {probes:?}"));
        };
        if probe.family != ProbeFamily::StaticUnknown || relation.is_some() {
            return Err(format!(
                "shadowed binding must keep the generic path: {probes:?}"
            ));
        }
        Ok(())
    }

    // #3294 control: a predicate line the diff itself changes keeps its
    // own direct probe; the changed binding does not retarget onto it.
    #[test]
    fn retarget_skips_predicate_lines_that_are_changed() -> Result<(), String> {
        let path = PathBuf::from("src/lib.rs");
        let body = "pub fn f(flag: bool) -> bool {\n    let end = 1;\n    if end == 2 {\n        true\n    } else {\n        false\n    }\n}";
        let changed = ChangedFile {
            path: path.clone(),
            added_lines: vec![
                ChangedLine {
                    line: 2,
                    new_side_line: 2,
                    text: "let end = 1;".to_string(),
                },
                ChangedLine {
                    line: 3,
                    new_side_line: 3,
                    text: "if end == 2 {".to_string(),
                },
            ],
            removed_lines: vec![],
        };
        let index = RustIndex {
            files: BTreeMap::from([(
                path.clone(),
                FileFacts {
                    path,
                    functions: vec![FunctionFact {
                        id: SymbolId("src/lib.rs::f".to_string()),
                        name: "f".to_string(),
                        file: PathBuf::from("src/lib.rs"),
                        start_line: 1,
                        end_line: 8,
                        body: body.to_string(),
                        calls: vec![],
                        returns: vec![],
                        literals: vec![],
                        source_role: FunctionSourceRole::Production,
                        attrs: vec![],
                        nested_fn_names: Vec::new(),
                        let_bindings: Vec::new(),
                    }],
                    ..FileFacts::default()
                },
            )]),
            ..RustIndex::default()
        };

        let probes = probes_for_file_with_relations(Path::new("workspace"), &changed, &index);

        // The changed let stays generic; the changed predicate carries its
        // own probe; neither is a retarget.
        if probes.len() != 2 {
            return Err(format!("expected two direct probes, got {probes:?}"));
        }
        if probes.iter().any(|(_, relation)| relation.is_some()) {
            return Err(format!(
                "no retarget may attach to a changed predicate: {probes:?}"
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod source_currentness_tests {
    use super::super::super::diff::ChangedLine;
    use super::super::super::rust_index::RustIndex;
    use super::*;
    use crate::domain::{ProbeFamily, SourceCurrentness, SymbolId};
    use std::path::{Path, PathBuf};

    fn removed_only_changed(line: usize, new_side_line: usize, text: &str) -> ChangedFile {
        ChangedFile {
            path: PathBuf::from("src/lib.rs"),
            added_lines: vec![],
            removed_lines: vec![ChangedLine {
                line,
                new_side_line,
                text: text.to_string(),
            }],
        }
    }

    #[test]
    fn removed_only_probe_resolves_base_deleted_and_keeps_new_side_coordinate() -> Result<(), String>
    {
        // The #3212 incident shape: a base expression whose candidate file
        // no longer contains it. In the producer slice the probe's recorded
        // coordinate stays the projected new-side position (#1222 RANK-1:
        // the index, flow classifiers, and IDE navigation all read the new
        // file); the revision semantics are carried by the disposition, and
        // consumer re-coordination is the #3212 projection slice.
        let changed = removed_only_changed(29, 13, "events.publish(invoice);");
        let index = RustIndex::default();
        let probes = probes_for_file(Path::new("workspace"), &changed, &index);
        let Some(probe) = probes
            .iter()
            .find(|probe| probe.after.is_none() && probe.family == ProbeFamily::SideEffect)
        else {
            return Err("side-effect probe expected".to_string());
        };
        assert_eq!(
            probe.location.line, 13,
            "producer slice keeps the projected new-side coordinate"
        );
        assert_eq!(
            resolve_probe_source_currentness(&changed, probe),
            SourceCurrentness::BaseDeleted
        );
        Ok(())
    }

    #[test]
    fn moved_expression_resolves_moved_or_renamed() -> Result<(), String> {
        let mut changed = removed_only_changed(4, 4, "events.publish(invoice);");
        // The same expression re-appears, non-adjacent, further down the
        // candidate file: movement evidence exists, exact identity does not.
        changed.added_lines.push(ChangedLine {
            line: 50,
            new_side_line: 50,
            text: "events.publish(invoice);".to_string(),
        });
        let index = RustIndex::default();
        let probes = probes_for_file(Path::new("workspace"), &changed, &index);
        let Some(probe) = probes
            .iter()
            .find(|probe| probe.after.is_none() && probe.family == ProbeFamily::SideEffect)
        else {
            return Err("side-effect probe expected".to_string());
        };
        assert_eq!(
            resolve_probe_source_currentness(&changed, probe),
            SourceCurrentness::MovedOrRenamed
        );
        Ok(())
    }

    #[test]
    fn added_probe_keeps_new_side_line_and_resolves_candidate_current() -> Result<(), String> {
        let changed = ChangedFile {
            path: PathBuf::from("src/lib.rs"),
            added_lines: vec![ChangedLine {
                line: 7,
                new_side_line: 7,
                text: "events.publish(invoice);".to_string(),
            }],
            removed_lines: vec![],
        };
        // An index without the file keeps the added-line path on the lexical
        // classifier, which is what seeds the side-effect family here.
        let index = RustIndex::default();
        let probes = probes_for_file(Path::new("workspace"), &changed, &index);
        let Some(probe) = probes
            .iter()
            .find(|probe| probe.family == ProbeFamily::SideEffect)
        else {
            return Err("side-effect probe expected".to_string());
        };
        assert_eq!(probe.location.line, 7);
        assert_eq!(
            resolve_probe_source_currentness(&changed, probe),
            SourceCurrentness::CandidateCurrent
        );
        // Reused line number with a different expression must not inherit
        // base identity: ids are content-addressed over the expression.
        let other = ChangedFile {
            path: PathBuf::from("src/lib.rs"),
            added_lines: vec![ChangedLine {
                line: 7,
                new_side_line: 7,
                text: "ledger.persist(invoice);".to_string(),
            }],
            removed_lines: vec![],
        };
        let other_probes = probes_for_file(Path::new("workspace"), &other, &index);
        let Some(other_probe) = other_probes
            .iter()
            .find(|probe| probe.family == ProbeFamily::SideEffect)
        else {
            return Err("side-effect probe expected".to_string());
        };
        assert_ne!(probe.id.0, other_probe.id.0);
        Ok(())
    }

    #[test]
    fn resolver_stays_unresolved_without_diff_evidence() {
        let changed = ChangedFile {
            path: PathBuf::from("src/lib.rs"),
            added_lines: vec![],
            removed_lines: vec![],
        };
        let bare = Probe {
            id: diff_probe_id(
                &changed.path,
                &ProbeFamily::SideEffect,
                Some(&SymbolId("owner".to_string())),
                "x()",
                1,
            ),
            location: SourceLocation::new(PathBuf::from("src/lib.rs"), 1, 1),
            owner: None,
            family: ProbeFamily::SideEffect,
            delta: super::super::family::delta_for_family(&ProbeFamily::SideEffect),
            before: None,
            after: None,
            expression: "x()".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };
        assert_eq!(
            resolve_probe_source_currentness(&changed, &bare),
            SourceCurrentness::UnresolvedSubject
        );
    }

    /// #3718: lines inside an inline `#[cfg(test)]` module are test
    /// evidence even when no function owns them; anything else fails
    /// closed to production-eligible (false).
    #[test]
    fn cfg_test_module_scope_grants_evidence_without_function_owner() {
        let source = "pub fn triage(input: &str) -> bool {\n\
             \x20   !input.is_empty()\n\
             }\n\
             \n\
             #[cfg(test)]\n\
             mod tests {\n\
             \x20   const FIXTURE_JSON: &str = r#\"{\n\
             \x20       \"producer_id\": \"abc\",\n\
             \x20   }\"#;\n\
             }\n";
        // The raw-string data line inside the cfg(test) module.
        assert!(line_in_cfg_test_module(source, 8));
        // The module opener and production lines are not evidence.
        assert!(!line_in_cfg_test_module(source, 6));
        assert!(!line_in_cfg_test_module(source, 2));
        // A bare `mod tests` without cfg(test) grants nothing.
        let bare = source.replace("#[cfg(test)]\n", "");
        assert!(!line_in_cfg_test_module(&bare, 7));
        // Out-of-line declarations contribute no scope here.
        assert!(!line_in_cfg_test_module("mod tests;\n", 1));
        // Degenerate inputs fail closed.
        assert!(!line_in_cfg_test_module(source, 0));
        assert!(!line_in_cfg_test_module(source, 99));
    }

    /// #3718: comments between `pub` and `const`-style items and nested
    /// modules resolve through the same masked scan.
    #[test]
    fn cfg_test_module_scope_handles_comments_and_nesting() {
        let source = "#[cfg(test)]\n\
             mod outer {\n\
             \x20   // a comment with { brace\n\
             \x20   mod inner {\n\
             \x20       const DEEP: u32 = 1;\n\
             \x20   }\n\
             }\n\
             pub fn live() {}\n";
        assert!(line_in_cfg_test_module(source, 5));
        assert!(!line_in_cfg_test_module(source, 8));
    }

    /// Review (#3721): string-bearing predicates, one-line modules,
    /// same-line nested openers, and spaced visibility all resolve.
    #[test]
    fn cfg_test_module_scope_covers_predicates_and_brace_edges() {
        // `feature = "slow"` keeps its literal for the shared classifier.
        let gated = "#[cfg(all(test, feature = \"slow\"))]\n\
             mod tests {\n\
             \x20   const JSON: &str = \"{}\";\n\
             }\n\
             pub fn live() {}\n";
        assert!(line_in_cfg_test_module(gated, 3));
        assert!(!line_in_cfg_test_module(gated, 5));
        // A one-line module contributes no scope to later lines.
        let one_liner = "#[cfg(test)] mod tests {}\n\
             const LIMIT: usize = 2;\n";
        assert!(!line_in_cfg_test_module(one_liner, 2));
        // A same-line nested opener never shifts the parent entry: one
        // `}` closes the nested block while the outer test scope stays
        // open; only the second `}` ends it.
        let nested_same_line = "#[cfg(test)] mod tests { mod nested {\n\
             \x20   const DEEP: u32 = 1;\n\
             }\n\
             }\n\
             const LIMIT: usize = 2;\n";
        assert!(line_in_cfg_test_module(nested_same_line, 2));
        assert!(!line_in_cfg_test_module(nested_same_line, 5));
        // Spaced visibility is legal Rust.
        let spaced = "#[cfg(test)]\n\
             pub (crate) mod tests {\n\
             \x20   const VALUE: u32 = 3;\n\
             }\n";
        assert!(line_in_cfg_test_module(spaced, 3));
    }
}
