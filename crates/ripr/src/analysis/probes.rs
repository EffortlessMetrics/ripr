use super::diff::ChangedFile;
use super::rust_index::{
    PROBE_SHAPE_CALL_DELETION, PROBE_SHAPE_ERROR_PATH, PROBE_SHAPE_FIELD_CONSTRUCTION,
    PROBE_SHAPE_MATCH_ARM, PROBE_SHAPE_PREDICATE, PROBE_SHAPE_RETURN_VALUE,
    PROBE_SHAPE_SIDE_EFFECT, RustIndex, changed_nodes_for_lines, extract_identifier_tokens,
    find_owner_function,
};
use crate::domain::{DeltaKind, Probe, ProbeFamily, ProbeId, SourceLocation};
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

pub fn probes_for_repo_file(root: &Path, path: &Path, index: &RustIndex) -> Vec<Probe> {
    let mut probes = Vec::new();
    let Some(facts) = index.files.get(path) else {
        return probes;
    };

    for shape in &facts.probe_shapes {
        let Some(family) = family_for_probe_shape(&shape.kind) else {
            continue;
        };

        let owner = find_owner_function(index, path, shape.start_line).map(|f| f.id.clone());

        let id = ProbeId(format!(
            "repo-probe:{}:{}:{}",
            sanitize_path(path),
            shape.start_line,
            family.as_str()
        ));

        let expected_sinks = expected_sinks(&shape.text, &family);
        let required_oracles = required_oracles(&shape.text, &family);

        probes.push(Probe {
            id,
            location: SourceLocation::new(root.join(path), shape.start_line, 1),
            owner,
            family,
            delta: DeltaKind::Unknown,
            before: None,
            after: Some(shape.text.clone()),
            expression: shape.text.clone(),
            expected_sinks,
            required_oracles,
        });
    }

    probes
}

fn classify_changed_syntax(
    index: &RustIndex,
    file: &Path,
    line: usize,
    changed_text: &str,
) -> Option<Vec<ProbeFamily>> {
    let facts = index.files.get(file)?;
    let mut families = facts
        .probe_shapes
        .iter()
        .filter(|shape| {
            shape.start_line <= line
                && line <= shape.end_line
                && shape_contains_changed_text(&shape.text, changed_text)
        })
        .filter_map(|shape| family_for_probe_shape(&shape.kind))
        .collect::<Vec<_>>();
    if families.is_empty() {
        return None;
    }
    families.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    families.dedup_by(|a, b| a.as_str() == b.as_str());
    Some(families)
}

fn shape_contains_changed_text(shape_text: &str, changed_text: &str) -> bool {
    let changed = changed_text
        .trim()
        .trim_end_matches(';')
        .trim_end_matches(',');
    if changed.is_empty() {
        return false;
    }
    let shape = shape_text.trim();
    shape.contains(changed) || changed.contains(shape)
}

fn family_for_probe_shape(kind: &str) -> Option<ProbeFamily> {
    match kind {
        PROBE_SHAPE_PREDICATE => Some(ProbeFamily::Predicate),
        PROBE_SHAPE_RETURN_VALUE => Some(ProbeFamily::ReturnValue),
        PROBE_SHAPE_ERROR_PATH => Some(ProbeFamily::ErrorPath),
        PROBE_SHAPE_CALL_DELETION => Some(ProbeFamily::CallDeletion),
        PROBE_SHAPE_FIELD_CONSTRUCTION => Some(ProbeFamily::FieldConstruction),
        PROBE_SHAPE_SIDE_EFFECT => Some(ProbeFamily::SideEffect),
        PROBE_SHAPE_MATCH_ARM => Some(ProbeFamily::MatchArm),
        _ => None,
    }
}

fn should_ignore_changed_line(text: &str) -> bool {
    text.is_empty()
        || text.starts_with("//")
        || text.starts_with("use ")
        || text.starts_with("pub use ")
        || text.starts_with("mod ")
        || text.starts_with("#")
}

fn classify_changed_line(text: &str) -> Vec<ProbeFamily> {
    let mut out = Vec::new();
    if has_predicate_shape(text) {
        out.push(ProbeFamily::Predicate);
    }
    if has_error_shape(text) {
        out.push(ProbeFamily::ErrorPath);
    }
    if has_return_shape(text) {
        out.push(ProbeFamily::ReturnValue);
    }
    if has_effect_shape(text) {
        out.push(ProbeFamily::SideEffect);
    }
    if has_call_shape(text) {
        out.push(ProbeFamily::CallDeletion);
    }
    if has_field_shape(text) {
        out.push(ProbeFamily::FieldConstruction);
    }
    if text.starts_with("match ") || text.contains("=>") {
        out.push(ProbeFamily::MatchArm);
    }
    if out.is_empty() {
        out.push(ProbeFamily::StaticUnknown);
    }
    out.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    out.dedup_by(|a, b| a.as_str() == b.as_str());
    out
}

fn has_predicate_shape(text: &str) -> bool {
    text.contains(" if ")
        || text.starts_with("if ")
        || text.starts_with("while ")
        || text.contains(" >= ")
        || text.contains(" <= ")
        || text.contains(" > ")
        || text.contains(" < ")
        || text.contains(" == ")
        || text.contains(" != ")
        || text.contains("&&")
        || text.contains("||")
}

fn has_return_shape(text: &str) -> bool {
    text.starts_with("return ")
        || text.contains(" Ok(")
        || text.starts_with("Ok(")
        || text.contains(" Some(")
        || text.starts_with("Some(")
        || text.contains("None")
        || text.contains("return")
}

fn has_error_shape(text: &str) -> bool {
    text.contains("Err(")
        || text.contains("Error::")
        || text.contains("map_err")
        || text.contains("bail!")
        || text.contains("anyhow!")
        || text.contains("?") && text.contains("Err")
}

fn has_effect_shape(text: &str) -> bool {
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

fn has_call_shape(text: &str) -> bool {
    text.contains('(')
        && text.contains(')')
        && !text.starts_with("fn ")
        && !text.starts_with("pub fn ")
        && !text.contains("assert")
}

fn has_field_shape(text: &str) -> bool {
    text.contains(':') && !text.contains("::") && !text.starts_with("fn ")
}

fn delta_for_family(family: &ProbeFamily) -> DeltaKind {
    match family {
        ProbeFamily::Predicate | ProbeFamily::MatchArm => DeltaKind::Control,
        ProbeFamily::SideEffect | ProbeFamily::CallDeletion => DeltaKind::Effect,
        ProbeFamily::ReturnValue | ProbeFamily::ErrorPath | ProbeFamily::FieldConstruction => {
            DeltaKind::Value
        }
        ProbeFamily::StaticUnknown => DeltaKind::Unknown,
    }
}

fn expected_sinks(text: &str, family: &ProbeFamily) -> Vec<String> {
    let mut sinks = Vec::new();
    match family {
        ProbeFamily::Predicate => {
            sinks.extend(["branch result".to_string(), "returned value".to_string()])
        }
        ProbeFamily::ReturnValue => {
            sinks.extend(["return value".to_string(), "assigned field".to_string()])
        }
        ProbeFamily::ErrorPath => sinks.extend(["error variant".to_string(), "Result".to_string()]),
        ProbeFamily::CallDeletion => {
            sinks.extend(["call effect".to_string(), "returned value".to_string()])
        }
        ProbeFamily::FieldConstruction => sinks.extend(
            extract_identifier_tokens(text)
                .into_iter()
                .take(4)
                .map(|t| format!("field:{t}")),
        ),
        ProbeFamily::SideEffect => sinks.extend([
            "published event".to_string(),
            "persisted state".to_string(),
            "mock expectation".to_string(),
        ]),
        ProbeFamily::MatchArm => {
            sinks.extend(["selected variant".to_string(), "arm result".to_string()])
        }
        ProbeFamily::StaticUnknown => sinks.push("unknown sink".to_string()),
    }
    sinks.sort();
    sinks.dedup();
    sinks
}

fn required_oracles(text: &str, family: &ProbeFamily) -> Vec<String> {
    let mut out = Vec::new();
    match family {
        ProbeFamily::Predicate => {
            out.push("boundary input".to_string());
            out.push("exact assertion on branch output".to_string());
        }
        ProbeFamily::ReturnValue => {
            out.push("exact or property assertion on returned value".to_string())
        }
        ProbeFamily::ErrorPath => out.push("exact error variant assertion".to_string()),
        ProbeFamily::CallDeletion => {
            out.push("assertion that notices removed call behavior".to_string())
        }
        ProbeFamily::FieldConstruction => out.push("field or whole-struct assertion".to_string()),
        ProbeFamily::SideEffect => {
            out.push("mock, event, persisted-state, or metric assertion".to_string())
        }
        ProbeFamily::MatchArm => {
            out.push("input selecting changed match arm and exact assertion".to_string())
        }
        ProbeFamily::StaticUnknown => out.push("manual review or real mutation".to_string()),
    }
    for token in extract_identifier_tokens(text).into_iter().take(3) {
        if token.chars().any(|c| c.is_uppercase()) {
            out.push(format!("assertion mentioning {token}"));
        }
    }
    out.sort();
    out.dedup();
    out
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

fn sanitize_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace(['/', '\\', ':'], "_")
        .trim_matches('_')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::diff::ChangedLine;
    use crate::analysis::rust_index::{RaRustSyntaxAdapter, RustSyntaxAdapter};
    use std::path::PathBuf;

    #[test]
    fn syntax_probe_shapes_classify_multiline_side_effect_calls() -> Result<(), String> {
        let source = r#"
pub fn publish(service: &mut Service, event: Event) {
    service
        .publish(
            event,
        );
}
"#;
        let adapter = RaRustSyntaxAdapter;
        let facts = adapter.summarize_file(Path::new("src/lib.rs"), source)?;
        let changed_line = line_containing(source, "event,")?;
        let mut index = RustIndex::default();
        index.files.insert(PathBuf::from("src/lib.rs"), facts);
        let changed = ChangedFile {
            path: PathBuf::from("src/lib.rs"),
            added_lines: vec![ChangedLine {
                line: changed_line,
                text: "            event,".to_string(),
            }],
            removed_lines: Vec::new(),
        };

        let probes = probes_for_file(Path::new("."), &changed, &index);
        let families = probes
            .iter()
            .map(|probe| probe.family.as_str())
            .collect::<Vec<_>>();

        assert!(families.contains(&"call_deletion"));
        assert!(families.contains(&"side_effect"));
        assert!(!families.contains(&"static_unknown"));
        Ok(())
    }

    #[test]
    fn syntax_probe_shapes_classify_multiline_return_calls() -> Result<(), String> {
        let source = r#"
pub fn parse(value: i32) -> Result<i32, Error> {
    return Ok(
        value,
    );
}
"#;
        let adapter = RaRustSyntaxAdapter;
        let facts = adapter.summarize_file(Path::new("src/lib.rs"), source)?;
        let changed_line = line_containing(source, "value,")?;
        let mut index = RustIndex::default();
        index.files.insert(PathBuf::from("src/lib.rs"), facts);
        let changed = ChangedFile {
            path: PathBuf::from("src/lib.rs"),
            added_lines: vec![ChangedLine {
                line: changed_line,
                text: "        value,".to_string(),
            }],
            removed_lines: Vec::new(),
        };

        let probes = probes_for_file(Path::new("."), &changed, &index);
        let families = probes
            .iter()
            .map(|probe| probe.family.as_str())
            .collect::<Vec<_>>();

        assert!(families.contains(&"call_deletion"));
        assert!(families.contains(&"return_value"));
        assert!(!families.contains(&"static_unknown"));
        Ok(())
    }

    #[test]
    fn syntax_probe_shapes_classify_tail_expression_returns() -> Result<(), String> {
        let source = r#"
pub fn total(amount: i32, fee: i32) -> i32 {
    amount + fee
}
"#;
        let adapter = RaRustSyntaxAdapter;
        let facts = adapter.summarize_file(Path::new("src/lib.rs"), source)?;
        let changed_line = line_containing(source, "amount + fee")?;
        let mut index = RustIndex::default();
        index.files.insert(PathBuf::from("src/lib.rs"), facts);
        let changed = ChangedFile {
            path: PathBuf::from("src/lib.rs"),
            added_lines: vec![ChangedLine {
                line: changed_line,
                text: "    amount + fee".to_string(),
            }],
            removed_lines: Vec::new(),
        };

        let probes = probes_for_file(Path::new("."), &changed, &index);
        let families = probes
            .iter()
            .map(|probe| probe.family.as_str())
            .collect::<Vec<_>>();

        assert_eq!(families, vec!["return_value"]);
        Ok(())
    }

    fn line_containing(source: &str, needle: &str) -> Result<usize, String> {
        match source.lines().position(|line| line.contains(needle)) {
            Some(index) => Ok(index + 1),
            None => Err(format!("missing line containing {needle}")),
        }
    }

    #[test]
    fn probes_for_repo_file_uses_repo_probe_id_prefix() -> Result<(), String> {
        let source = r#"
pub fn check(x: i32) -> bool {
    x > 5
}
"#;
        let adapter = RaRustSyntaxAdapter;
        let facts = adapter.summarize_file(std::path::Path::new("src/lib.rs"), source)?;
        let mut index = RustIndex::default();
        index.files.insert(PathBuf::from("src/lib.rs"), facts);

        let probes = probes_for_repo_file(
            std::path::Path::new("."),
            std::path::Path::new("src/lib.rs"),
            &index,
        );

        if probes.is_empty() {
            return Err("expected at least one probe from probes_for_repo_file".to_string());
        }
        for probe in &probes {
            if !probe.id.0.starts_with("repo-probe:") {
                return Err(format!(
                    "expected probe id to start with 'repo-probe:' but got {}",
                    probe.id.0
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn probes_for_repo_file_returns_empty_for_unknown_path() -> Result<(), String> {
        let index = RustIndex::default();
        let probes = probes_for_repo_file(
            std::path::Path::new("."),
            std::path::Path::new("src/unknown.rs"),
            &index,
        );
        if !probes.is_empty() {
            return Err("expected empty vec for unknown path, but got probes".to_string());
        }
        Ok(())
    }
}
