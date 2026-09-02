use super::super::classify::is_wildcard_discard_binding;
use super::super::diff::{ChangedFile, ChangedLine};
use super::super::rust_index::{
    RustIndex, SyntaxNodeFact, changed_nodes_for_lines, extract_identifier_tokens,
    find_owner_function,
};
use super::classify::{parser_probe_shapes_for_changed_line, should_ignore_changed_line};
use super::expectations::{expected_sinks, required_oracles};
use super::family::delta_for_family;
use super::ids::{diff_probe_id, normalize_expression};
use super::lexical::classify_changed_line;
use super::subprocess::bounded_subprocess_family;
use crate::domain::{Probe, ProbeFamily, SourceLocation};
use std::path::Path;

pub fn probes_for_file(root: &Path, changed: &ChangedFile, index: &RustIndex) -> Vec<Probe> {
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
        if changed_line_owned_by_test(index, &changed.path, added.new_side_line) {
            continue;
        }
        // RIPR-SPEC-0151 (source authority): a bounded literal allowlisted
        // subprocess adapter is recognized ahead of the parser-shape pipeline so
        // the adapter family is not reclassified by generic syntax shapes.
        if let Some(family) =
            bounded_subprocess_family(index, &changed.path, added.new_side_line, text)
        {
            probes.push(build_probe(
                &build_context,
                added,
                family,
                nearby_removed_line(added.new_side_line, text, changed),
                Some(text.to_string()),
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
                probes.push(build_probe(
                    &build_context,
                    &canonical_line,
                    shape.family,
                    nearby_removed_line(shape.start_line, &canonical_text, changed),
                    Some(canonical_text),
                ));
            }
            continue;
        }
        if !parser_shapes.is_empty() {
            for shape in parser_shapes {
                probes.push(build_probe(
                    &build_context,
                    added,
                    shape.family,
                    nearby_removed_line(added.new_side_line, text, changed),
                    Some(text.to_string()),
                ));
            }
            continue;
        }
        for family in classify_changed_line(text) {
            probes.push(build_probe(
                &build_context,
                added,
                family,
                nearby_removed_line(added.new_side_line, text, changed),
                Some(text.to_string()),
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
        if changed_line_owned_by_test(index, &changed.path, removed.new_side_line) {
            continue;
        }
        for family in classify_changed_line(text) {
            if has_matching_added_line(removed, &family, changed) {
                continue;
            }
            probes.push(build_probe(
                &build_context,
                removed,
                family,
                Some(text.to_string()),
                None,
            ));
        }
    }

    // Post-hoc collision de-dup: if two probes got the same id, append .2, .3, …
    // to the 2nd+ occurrences (the first keeps its id as-is, i.e. ordinal 1).
    dedup_probe_ids(&mut probes);

    probes
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
fn dedup_probe_ids(probes: &mut [Probe]) {
    use std::collections::HashMap;
    let mut seen: HashMap<String, u32> = HashMap::new();
    for probe in probes.iter_mut() {
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
fn changed_line_owned_by_test(index: &RustIndex, path: &Path, line: usize) -> bool {
    find_owner_function(index, path, line).is_some_and(|function| function.is_test)
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
    // coordinate, which is what the RustIndex (built from the new file) and any
    // IDE navigation into the new file require (RANK-1 fix, #1222).
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
                        is_test: false,
                        attrs: vec![],
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
                        is_test: false,
                        attrs: vec![],
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
        let index_with = |is_test: bool| RustIndex {
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
                        is_test,
                        attrs: vec![],
                    }],
                    ..FileFacts::default()
                },
            )]),
            ..RustIndex::default()
        };

        // Control: a production owner still probes the error path.
        let production = probes_for_file(Path::new("workspace"), &changed, &index_with(false));
        assert!(
            !production.is_empty(),
            "a non-test error path should still generate a probe"
        );

        // #1055: the same line owned by a `#[test]` function generates nothing —
        // the test is the instrument, not the surface under test.
        let in_test = probes_for_file(Path::new("workspace"), &changed, &index_with(true));
        assert!(
            in_test.is_empty(),
            "a line owned by a test function must not generate probes, got {in_test:?}"
        );
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
                        is_test: false,
                        attrs: vec![],
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
}
