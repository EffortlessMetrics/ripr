use super::diff::ChangedFile;
use super::rust_index::{
    RustIndex, changed_nodes_for_lines, extract_identifier_tokens, find_owner_function,
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
        let families = classify_changed_line(text);
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
