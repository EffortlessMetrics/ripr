use super::super::diff::ChangedFile;
use super::super::rust_index::{
    RustIndex, changed_nodes_for_lines, extract_identifier_tokens, find_owner_function,
};
use super::classify::{classify_changed_line, classify_changed_syntax, should_ignore_changed_line};
use super::config::{delta_for_family, expected_sinks, required_oracles};
use super::sanitize_path;
use crate::domain::{Probe, ProbeId, SourceLocation};
use std::path::Path;

pub fn probes_for_file(root: &Path, changed: &ChangedFile, index: &RustIndex) -> Vec<Probe> {
    let mut probes = Vec::new();
    let changed_lines = changed
        .added_lines
        .iter()
        .map(|line| line.line)
        .collect::<Vec<_>>();
    let changed_nodes = changed_nodes_for_lines(index, &changed.path, &changed_lines);
    for added in &changed.added_lines {
        let text = added.text.trim();
        if should_ignore_changed_line(text) {
            continue;
        }
        let families = classify_changed_syntax(index, &changed.path, added.line, text)
            .unwrap_or_else(|| classify_changed_line(text));
        for family in families {
            let delta = delta_for_family(&family);
            let owner = changed_nodes
                .iter()
                .find(|node| node.start_line <= added.line && added.line <= node.end_line)
                .and_then(|node| node.owner.clone())
                .or_else(|| {
                    find_owner_function(index, &changed.path, added.line).map(|f| f.id.clone())
                });
            let id = ProbeId(format!(
                "probe:{}:{}:{}",
                sanitize_path(&changed.path),
                added.line,
                family.as_str()
            ));
            let expected_sinks = expected_sinks(text, &family);
            let required_oracles = required_oracles(text, &family);
            probes.push(Probe {
                id,
                location: SourceLocation::new(root.join(&changed.path), added.line, 1),
                owner,
                family,
                delta,
                before: nearby_removed_line(text, changed),
                after: Some(text.to_string()),
                expression: text.to_string(),
                expected_sinks,
                required_oracles,
            });
        }
    }
    probes
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
    use crate::domain::{ProbeFamily, SymbolId};
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
