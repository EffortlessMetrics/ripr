use super::super::diff::{ChangedFile, ChangedLine};
use super::super::rust_index::{
    RustIndex, SyntaxNodeFact, changed_nodes_for_lines, extract_identifier_tokens,
    find_owner_function,
};
use super::classify::{classify_changed_syntax, should_ignore_changed_line};
use super::expectations::{expected_sinks, required_oracles};
use super::family::delta_for_family;
use super::ids::diff_probe_id;
use super::lexical::classify_changed_line;
use crate::domain::{Probe, ProbeFamily, SourceLocation};
use std::path::Path;

pub fn probes_for_file(root: &Path, changed: &ChangedFile, index: &RustIndex) -> Vec<Probe> {
    let mut probes = Vec::new();
    let changed_lines = changed
        .added_lines
        .iter()
        .chain(changed.removed_lines.iter())
        .map(|line| line.line)
        .collect::<Vec<_>>();
    let changed_nodes = changed_nodes_for_lines(index, &changed.path, &changed_lines);
    let build_context = ProbeBuildContext {
        root,
        changed,
        index,
        changed_nodes: &changed_nodes,
    };

    for added in &changed.added_lines {
        let text = added.text.trim();
        if should_ignore_changed_line(text) {
            continue;
        }
        let families = classify_changed_syntax(index, &changed.path, added.line, text)
            .unwrap_or_else(|| classify_changed_line(text));
        for family in families {
            probes.push(build_probe(
                &build_context,
                added,
                family,
                nearby_removed_line(text, changed),
                Some(text.to_string()),
            ));
        }
    }

    for removed in &changed.removed_lines {
        let text = removed.text.trim();
        if should_ignore_changed_line(text) {
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

    probes
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
    let owner = context
        .changed_nodes
        .iter()
        .find(|node| node.start_line <= changed_line.line && changed_line.line <= node.end_line)
        .and_then(|node| node.owner.clone())
        .or_else(|| {
            find_owner_function(context.index, &context.changed.path, changed_line.line)
                .map(|function| function.id.clone())
        });
    let id = diff_probe_id(&context.changed.path, changed_line.line, &family);
    let expected_sinks = expected_sinks(text, &family);
    let required_oracles = required_oracles(text, &family);

    Probe {
        id,
        location: SourceLocation::new(
            context.root.join(&context.changed.path),
            changed_line.line,
            1,
        ),
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

fn has_matching_added_line(
    removed_line: &ChangedLine,
    removed_family: &ProbeFamily,
    changed: &ChangedFile,
) -> bool {
    let removed_tokens = extract_identifier_tokens(&removed_line.text);
    !removed_tokens.is_empty()
        && changed.added_lines.iter().any(|line| {
            if removed_line.line.abs_diff(line.line) > 1 {
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

fn nearby_removed_line(added: &str, changed: &ChangedFile) -> Option<String> {
    let added_tokens = extract_identifier_tokens(added);
    changed
        .removed_lines
        .iter()
        .find(|line| {
            let removed_tokens = extract_identifier_tokens(&line.text);
            !added_tokens.is_empty()
                && added_tokens
                    .iter()
                    .any(|token| removed_tokens.iter().any(|other| other == token))
        })
        .map(|line| line.text.trim().to_string())
        .or_else(|| {
            changed
                .removed_lines
                .first()
                .map(|line| line.text.trim().to_string())
        })
}

#[cfg(test)]
mod tests {
    use super::super::super::diff::ChangedLine;
    use super::super::super::rust_index::{
        FileFacts, FunctionFact, PROBE_SHAPE_PREDICATE, ProbeShapeFact, RustIndex,
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
                text: "if amount >= threshold {".to_string(),
            }],
            removed_lines: vec![ChangedLine {
                line: 3,
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
        assert_eq!(probe.id.0, "probe:src_lib.rs:3:predicate");
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
    fn probes_for_file_falls_back_to_static_unknown_without_syntax_shape() {
        let changed = ChangedFile {
            path: PathBuf::from("src/lib.rs"),
            added_lines: vec![ChangedLine {
                line: 10,
                text: "let total = discounted;".to_string(),
            }],
            removed_lines: vec![],
        };

        let probes = probes_for_file(Path::new("workspace"), &changed, &RustIndex::default());

        assert_eq!(probes.len(), 1);
        assert_eq!(probes[0].id.0, "probe:src_lib.rs:10:static_unknown");
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
        assert_eq!(side_effect.id.0, "probe:src_lib.rs:4:side_effect");
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
                text: "if amount >= threshold {".to_string(),
            }],
            removed_lines: vec![ChangedLine {
                line: 3,
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

    #[test]
    fn probes_for_file_ignores_non_behavior_lines() {
        let changed = ChangedFile {
            path: PathBuf::from("src/lib.rs"),
            added_lines: vec![
                ChangedLine {
                    line: 1,
                    text: "use crate::pricing;".to_string(),
                },
                ChangedLine {
                    line: 2,
                    text: "// comment".to_string(),
                },
            ],
            removed_lines: vec![],
        };

        let probes = probes_for_file(Path::new("workspace"), &changed, &RustIndex::default());
        assert!(probes.is_empty());
    }
}
