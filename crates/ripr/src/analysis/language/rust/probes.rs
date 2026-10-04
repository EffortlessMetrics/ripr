//! Probe-family shaping and lexical changed-behavior extraction for the Rust adapter.
//!
//! This module owns the Rust-adapter-local lexical helpers that name a changed
//! `let` binding, mask comments/strings for token-safe scans, and attach
//! value-propagation / changed-binding-predicate evidence. Shared probe
//! production remains in `analysis::probes`; this file does not reimplement
//! family classification or index construction.
//!
//! Mixed reach, index, and diff-pipeline helpers stay in `rust/mod.rs` until
//! later RA slices.

use crate::analysis::facts::RustIndex;
use crate::analysis::probes::{BindingValueResolution, ChangedBindingPredicateUse};
use crate::domain::{ExposureClass, Finding, Probe, StaticLimitKind, StopReason};

/// Name the bounded value-propagation limitation from #3215 without
/// pretending that syntax-first analysis proved the equality boundary.
///
/// This deliberately recognizes only a changed `let` binding whose value is
/// produced by `find`/`rfind` or `len_utf8`, normalized through `map_or`, and
/// whose same-owner body later compares that binding. Other helper, loop,
/// coercion, and data-flow shapes remain unchanged and fail closed as before.
pub(super) fn apply_rust_value_propagation_limit(
    finding: &mut Finding,
    probe: &Probe,
    index: &RustIndex,
) {
    if finding.class != ExposureClass::StaticUnknown
        || finding.static_limit_kind.is_some()
        || finding.related_tests.is_empty()
    {
        return;
    }
    let Some((binding, rhs)) = changed_let_binding(&probe.expression) else {
        return;
    };
    let masked_rhs = mask_rust_comments_and_strings(rhs);
    if !masked_rhs.contains(".map_or(")
        || !(masked_rhs.contains(".find(")
            || masked_rhs.contains(".rfind(")
            || masked_rhs.contains(".len_utf8("))
    {
        return;
    }
    let Some(owner_id) = probe.owner.as_ref() else {
        return;
    };
    let Some(owner) = index
        .functions()
        .iter()
        .find(|function| &function.id == owner_id)
    else {
        return;
    };
    let Some(predicate) = find_value_propagation_predicate(&owner.body, binding) else {
        return;
    };

    finding.static_limit_kind = Some(StaticLimitKind::RustValuePropagationUnresolved);
    finding
        .stop_reasons
        .push(StopReason::PropagationEvidenceUnknown);
    finding.evidence.push(format!(
        "limitation_last_established_edge: changed binding `{binding}` uses `{rhs}`"
    ));
    finding.evidence.push(format!(
        "limitation_first_unresolved_edge: `{binding}` value propagation into equality predicate `{}`",
        predicate.trim()
    ));
    finding
        .evidence
        .push("limitation_analyzer_route: analysis/rust-value-propagation".to_string());
    finding.evidence.push(
        "limitation_non_claim: named analyzer limitation only; ripr does not confirm coverage or prescribe a repair test"
            .to_string(),
    );
}

/// Disclose a changed-binding predicate relation on the probe's finding.
/// The probe is predicate-shaped and classifies through the normal
/// predicate path; this only discloses the causal link (which binding and
/// initializer fed the predicate) and the operand-value limitation. It
/// never changes the class, adds a stop reason, or prescribes a repair —
/// the operand values stay unresolved until a later slice evaluates them.
pub(super) fn attach_changed_binding_predicate_evidence(
    finding: &mut Finding,
    relation: Option<&ChangedBindingPredicateUse>,
) {
    let Some(relation) = relation else {
        return;
    };
    // The relation names both causal values when the diff carries the
    // old initializer; the probe's before/after already hold them, and
    // the relation line states them together.
    let initializer_range = match finding.probe.before.as_deref() {
        Some(before) if before != relation.initializer => {
            format!("`{before}` -> `{}`", relation.initializer)
        }
        _ => format!("`{}`", relation.initializer),
    };
    finding.evidence.push(format!(
        "binding_predicate_relation: changed binding `{}` initializer {initializer_range} flows into predicate operand at line {}",
        relation.binding, relation.predicate_line
    ));
    if let BindingValueResolution::Unresolved { earliest_operation } = &relation.value_resolution {
        // Neutral prefixes: this is a value disclosure on a
        // predicate-shaped finding, not a `static_limit_kind` record,
        // so it deliberately stays outside the structured
        // `limitation_*` evidence contract (#3294 review).
        finding.evidence.push(format!(
            "binding_predicate_value_unresolved: operand value of `{}` unresolved at earliest initializer operation `{}`",
            relation.binding, earliest_operation
        ));
        finding.evidence.push(
            "binding_predicate_non_claim: named analyzer limitation only; ripr does not confirm coverage or prescribe a repair test"
                .to_string(),
        );
    }
}

/// Find an equality predicate that refers to the established binding after
/// its declaration. Comments, strings, member names, and later shadowing are
/// intentionally excluded so this limitation remains fail-closed.
pub(super) fn find_value_propagation_predicate<'a>(
    body: &'a str,
    binding: &str,
) -> Option<&'a str> {
    let masked = mask_rust_comments_and_strings(body);
    let mut established = false;
    let mut shadowed = false;
    for (line, masked_line) in body.lines().zip(masked.lines()) {
        let trimmed = masked_line.trim();
        if trimmed.starts_with("let ") {
            let is_binding_declaration = trimmed
                .split_once('=')
                .is_some_and(|(lhs, _)| contains_identifier_token(lhs, binding));
            if is_binding_declaration && established {
                shadowed = true;
            }
            established = is_binding_declaration || established;
            continue;
        }
        if shadowed || !established || !binding_equality_predicate(trimmed, binding) {
            continue;
        }
        let mut search = 0;
        while let Some(offset) = trimmed[search..].find(binding) {
            let start = search + offset;
            let before = trimmed[..start].chars().next_back();
            let after = trimmed[start + binding.len()..].chars().next();
            if !before.is_some_and(|ch| ch == '_' || ch.is_ascii_alphanumeric())
                && !after.is_some_and(|ch| ch == '_' || ch.is_ascii_alphanumeric())
                && !trimmed[..start].trim_end().ends_with('.')
            {
                return Some(line);
            }
            search = start.saturating_add(binding.len());
        }
    }
    None
}

fn binding_equality_predicate(line: &str, binding: &str) -> bool {
    if line.contains("!=") {
        return false;
    }
    let Some((left, right)) = line.split_once("==") else {
        return false;
    };
    let left = left
        .rsplit_once("&&")
        .or_else(|| left.rsplit_once("||"))
        .map_or(left, |(_, operand)| operand);
    let right = right
        .split_once("&&")
        .or_else(|| right.split_once("||"))
        .map_or(right, |(operand, _)| operand);
    [left, right].into_iter().any(|side| {
        let Some(start) = side.find(binding) else {
            return false;
        };
        let before = side[..start].chars().next_back();
        let after = side[start + binding.len()..].chars().next();
        !before.is_some_and(|ch| ch == '_' || ch.is_ascii_alphanumeric() || ch == '.')
            && !after.is_some_and(|ch| ch == '_' || ch.is_ascii_alphanumeric())
    })
}

/// A simple `let <ident> = <rhs>;` line. Shared by the #3271 limitation
/// and the #3294 binding-predicate relation so both agree on what a
/// changed binding is.
pub(crate) fn changed_let_binding(expression: &str) -> Option<(&str, &str)> {
    let text = expression.trim().trim_end_matches(';').trim();
    let rest = text.strip_prefix("let ")?;
    let (lhs, rhs) = rest.split_once('=')?;
    let binding = lhs.trim().strip_prefix("mut ").unwrap_or(lhs.trim());
    if binding.is_empty()
        || !binding
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return None;
    }
    Some((binding, rhs.trim()))
}

fn contains_identifier_token(text: &str, ident: &str) -> bool {
    text.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .any(|token| token == ident)
}

pub(crate) fn mask_rust_comments_and_strings(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut masked = bytes.to_vec();
    let mut index = 0usize;
    let mut block_depth = 0usize;

    while index < bytes.len() {
        if block_depth > 0 {
            if starts_with_bytes(bytes, index, b"/*") {
                mask_non_newline_bytes(&mut masked, index, index.saturating_add(2));
                block_depth = block_depth.saturating_add(1);
                index = index.saturating_add(2);
            } else if starts_with_bytes(bytes, index, b"*/") {
                mask_non_newline_bytes(&mut masked, index, index.saturating_add(2));
                block_depth = block_depth.saturating_sub(1);
                index = index.saturating_add(2);
            } else {
                mask_non_newline_bytes(&mut masked, index, index.saturating_add(1));
                index = index.saturating_add(1);
            }
            continue;
        }

        if starts_with_bytes(bytes, index, b"//") {
            let end = bytes[index..]
                .iter()
                .position(|byte| *byte == b'\n')
                .map_or(bytes.len(), |offset| index + offset);
            mask_non_newline_bytes(&mut masked, index, end);
            index = end;
            continue;
        }

        if starts_with_bytes(bytes, index, b"/*") {
            mask_non_newline_bytes(&mut masked, index, index.saturating_add(2));
            block_depth = 1;
            index = index.saturating_add(2);
            continue;
        }

        if let Some(end) = rust_raw_string_literal_end(bytes, index) {
            mask_non_newline_bytes(&mut masked, index, end);
            index = end;
            continue;
        }

        if bytes[index] == b'"' {
            let end = rust_string_literal_end(bytes, index);
            mask_non_newline_bytes(&mut masked, index, end);
            index = end;
            continue;
        }

        index = index.saturating_add(1);
    }

    match String::from_utf8(masked) {
        Ok(value) => value,
        Err(_) => text.to_string(),
    }
}

fn starts_with_bytes(bytes: &[u8], index: usize, needle: &[u8]) -> bool {
    bytes
        .get(index..index.saturating_add(needle.len()))
        .is_some_and(|candidate| candidate == needle)
}

fn mask_non_newline_bytes(bytes: &mut [u8], start: usize, end: usize) {
    let bounded_end = end.min(bytes.len());
    for byte in bytes.iter_mut().take(bounded_end).skip(start) {
        if *byte != b'\n' {
            *byte = b' ';
        }
    }
}

fn rust_string_literal_end(bytes: &[u8], start: usize) -> usize {
    let mut index = start.saturating_add(1);
    let mut escaped = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if escaped {
            escaped = false;
        } else if byte == b'\\' {
            escaped = true;
        } else if byte == b'"' {
            return index.saturating_add(1);
        }
        index = index.saturating_add(1);
    }
    bytes.len()
}

fn rust_raw_string_literal_end(bytes: &[u8], start: usize) -> Option<usize> {
    let prefix_len = if bytes.get(start) == Some(&b'r') {
        1
    } else if bytes.get(start) == Some(&b'b') && bytes.get(start.saturating_add(1)) == Some(&b'r') {
        2
    } else {
        return None;
    };

    let mut delimiter = start.saturating_add(prefix_len);
    let mut hashes = 0usize;
    while bytes.get(delimiter) == Some(&b'#') {
        hashes = hashes.saturating_add(1);
        delimiter = delimiter.saturating_add(1);
    }
    if bytes.get(delimiter) != Some(&b'"') {
        return None;
    }

    let mut index = delimiter.saturating_add(1);
    while index < bytes.len() {
        if bytes[index] == b'"' {
            let suffix_start = index.saturating_add(1);
            let suffix_end = suffix_start.saturating_add(hashes);
            if suffix_end <= bytes.len()
                && bytes[suffix_start..suffix_end]
                    .iter()
                    .all(|byte| *byte == b'#')
            {
                return Some(suffix_end);
            }
        }
        index = index.saturating_add(1);
    }

    Some(bytes.len())
}

#[cfg(test)]
mod tests {
    use super::{
        apply_rust_value_propagation_limit, attach_changed_binding_predicate_evidence,
        changed_let_binding, find_value_propagation_predicate, mask_rust_comments_and_strings,
    };
    use crate::analysis::facts::{FunctionSourceRole, FunctionSummary, RustIndex};
    use crate::analysis::probes::{
        BindingValueResolution, ChangedBindingPredicateUse, PredicateOperandSide,
    };
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, Finding, OracleKind,
        OracleStrength, Probe, ProbeFamily, ProbeId, RelatedTest, RevealEvidence, RiprEvidence,
        SourceLocation, StageEvidence, StageState, StaticLimitKind, StopReason, SymbolId,
    };
    use std::path::PathBuf;

    #[test]
    fn changed_let_binding_accepts_simple_mut_and_trailing_semicolon() {
        assert_eq!(
            changed_let_binding("    let end = input.rfind(delim).map_or(0, |idx| idx);"),
            Some(("end", "input.rfind(delim).map_or(0, |idx| idx)"))
        );
        assert_eq!(
            changed_let_binding("let mut start = delim.len_utf8();"),
            Some(("start", "delim.len_utf8()"))
        );
    }

    #[test]
    fn changed_let_binding_rejects_patterns_types_and_non_let_forms() {
        assert_eq!(changed_let_binding("let Foo { end } = value;"), None);
        assert_eq!(changed_let_binding("let end: usize = 1;"), None);
        assert_eq!(changed_let_binding("let (end, start) = pair;"), None);
        assert_eq!(changed_let_binding("end = input.len();"), None);
        assert_eq!(changed_let_binding("let  = missing;"), None);
        assert_eq!(changed_let_binding(""), None);
    }

    #[test]
    fn mask_erases_comments_and_strings_but_keeps_newlines_and_code() {
        let source = "call(x); // assert_result!(x, 1)\n/* assert_block!(x) */ keep();\n";
        let masked = mask_rust_comments_and_strings(source);
        assert!(masked.contains("call(x);"));
        assert!(masked.contains("keep();"));
        assert!(!masked.contains("assert_result"));
        assert!(!masked.contains("assert_block"));
        assert_eq!(masked.matches('\n').count(), source.matches('\n').count());
        assert_eq!(masked.len(), source.len());
    }

    #[test]
    fn mask_nested_block_comments_and_hashed_raw_strings() {
        let source = r##"keep(/* /* nested */ still */ done()); let raw = r#"assert_raw!(x)"#;"##;
        let masked = mask_rust_comments_and_strings(source);
        assert!(masked.contains("keep("));
        assert!(masked.contains("done())"));
        assert!(!masked.contains("nested"));
        assert!(!masked.contains("assert_raw"));
    }

    #[test]
    fn mask_does_not_special_case_char_literal_quotes_unlike_extract_mask() {
        // Discriminator against silently switching to extract::mask_comments_and_strings,
        // which treats character literals as code. The adapter mask starts a string at
        // the inner quote and therefore blanks the remainder of the line.
        let source = "let quote = '\"'; keep_alive(x);";
        let masked = mask_rust_comments_and_strings(source);
        assert!(
            !masked.contains("keep_alive"),
            "adapter mask must keep treating the quote inside a char literal as a string opener: {masked:?}"
        );
        assert!(masked.contains("let quote"));
    }

    #[test]
    fn mask_byte_raw_string_and_escaped_plain_string() {
        let source = r##"let start = br#"assert_br!(x)"#; let note = "assert_str!(x)"; call();"##;
        let masked = mask_rust_comments_and_strings(source);
        assert!(masked.contains("let start"));
        assert!(masked.contains("let note"));
        assert!(masked.contains("call();"));
        assert!(!masked.contains("assert_br"));
        assert!(!masked.contains("assert_str"));
    }

    #[test]
    fn find_value_propagation_predicate_accepts_and_or_equality_operands() {
        // The matcher splits on the first `==` only, then keeps the first
        // `&&`/`||` operand of each side. `end` on the left of that first
        // equality is accepted; `end == start` after a preceding `&&`
        // equality stays fail-closed.
        let first_equality = concat!(
            "    let end = input.find(d).map_or(0, |idx| idx);\n",
            "    if end == start || other == marker { return 1; }\n",
        );
        assert_eq!(
            find_value_propagation_predicate(first_equality, "end").map(str::trim),
            Some("if end == start || other == marker { return 1; }")
        );
        let later_equality = concat!(
            "    let end = input.find(d).map_or(0, |idx| idx);\n",
            "    if other == marker && end == start { return 1; }\n",
        );
        assert!(
            find_value_propagation_predicate(later_equality, "end").is_none(),
            "a later &&-clause equality must not be credited as the first == operand"
        );
    }

    #[test]
    fn value_propagation_predicate_ignores_non_entity_text_and_shadowing() {
        let body = r#"
    let end = input.rfind(delim).map_or(0, |idx| idx);
    let copied = end;
    // end == start is only documentation.
    let text = "end == start";
    if other.end == start { return 0; }
    let end = 1;
    if end == start { return 1; }
"#;
        assert!(find_value_propagation_predicate(body, "end").is_none());
    }

    #[test]
    fn value_propagation_predicate_rejects_mixed_operator_line() {
        let body = concat!(
            "    let end = input.rfind(delim).map_or(0, |idx| idx);\n",
            "    if end != start && other == marker { return 0; }\n",
        );
        assert!(find_value_propagation_predicate(body, "end").is_none());
    }

    #[test]
    fn value_propagation_predicate_rejects_binding_outside_equality_operand() {
        let body = concat!(
            "    let end = input.rfind(delim).map_or(0, |idx| idx);\n",
            "    if other == marker && end > 0 { return 0; }\n",
        );
        assert!(find_value_propagation_predicate(body, "end").is_none());
    }

    #[test]
    fn value_propagation_predicate_rejects_map_or_else_shape() {
        let rhs = mask_rust_comments_and_strings("input.find(delim).map_or_else(|| 0, |idx| idx)");
        assert!(!rhs.contains(".map_or("));
    }

    fn stage(state: StageState) -> StageEvidence {
        StageEvidence::new(state, Confidence::Medium, "stage")
    }

    fn static_unknown_finding(expression: &str, owner: &str) -> Finding {
        Finding {
            id: "probe:src_lib.rs:binding:test".to_string(),
            canonical_gap: None,
            probe: Probe {
                id: ProbeId("probe:src_lib.rs:binding:test".to_string()),
                location: SourceLocation::new("src/lib.rs", 2, 1),
                owner: Some(SymbolId(owner.to_string())),
                family: ProbeFamily::Predicate,
                delta: DeltaKind::Value,
                before: None,
                after: Some(expression.to_string()),
                expression: expression.to_string(),
                expected_sinks: Vec::new(),
                required_oracles: Vec::new(),
            },
            class: ExposureClass::StaticUnknown,
            ripr: RiprEvidence {
                reach: stage(StageState::Yes),
                infect: stage(StageState::Unknown),
                propagate: stage(StageState::Unknown),
                reveal: RevealEvidence {
                    observe: stage(StageState::No),
                    discriminate: stage(StageState::No),
                },
            },
            confidence: 0.4,
            evidence: Vec::new(),
            missing: Vec::new(),
            flow_sinks: Vec::new(),
            activation: ActivationEvidence::default(),
            stop_reasons: Vec::new(),
            related_tests_matched_total: None,
            related_tests: vec![RelatedTest {
                name: "covers_split".to_string(),
                file: PathBuf::from("tests/it.rs"),
                line: 4,
                oracle: None,
                oracle_kind: OracleKind::Unknown,
                oracle_strength: OracleStrength::None,
                relation_reason: None,
                relation_confidence: None,
            }],
            recommended_next_step: None,
            language: None,
            language_status: None,
            owner_kind: None,
            static_limit_kind: None,
            changed_sink: None,
            observed_sink: None,
            oracle_alignment: None,
            alignment_reason: None,
            source_currentness: crate::domain::SourceCurrentness::CandidateCurrent,
        }
    }

    fn owner_function(id: &str, name: &str, body: &str) -> FunctionSummary {
        FunctionSummary {
            id: SymbolId(id.to_string()),
            name: name.to_string(),
            file: PathBuf::from("src/lib.rs"),
            start_line: 1,
            end_line: 8,
            body: body.to_string(),
            calls: vec![],
            returns: vec![],
            literals: vec![],
            source_role: FunctionSourceRole::Production,
            attrs: vec![],
            impl_attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
            impl_context: Default::default(),
            item: Default::default(),
        }
    }

    #[test]
    fn value_propagation_limit_names_map_or_find_equality_and_does_not_fire_on_map_or_else() {
        let expression = "let end = input.rfind(delim).map_or(0, |idx| idx);";
        let body = concat!(
            "    let end = input.rfind(delim).map_or(0, |idx| idx);\n",
            "    if end == start { return 1; }\n",
        );
        let index = RustIndex::from_owned(crate::analysis::facts::OwnedRustIndex {
            functions: vec![owner_function("src/lib.rs::split", "split", body)],
            ..Default::default()
        });

        let mut named = static_unknown_finding(expression, "src/lib.rs::split");
        let probe = named.probe.clone();
        apply_rust_value_propagation_limit(&mut named, &probe, &index);
        assert_eq!(
            named.static_limit_kind,
            Some(StaticLimitKind::RustValuePropagationUnresolved)
        );
        assert!(
            named
                .stop_reasons
                .contains(&StopReason::PropagationEvidenceUnknown)
        );
        assert!(named.evidence.iter().any(|line| {
            line.contains("limitation_last_established_edge: changed binding `end`")
        }));
        assert_eq!(named.class, ExposureClass::StaticUnknown);

        let mut map_or_else = static_unknown_finding(
            "let end = input.find(delim).map_or_else(|| 0, |idx| idx);",
            "src/lib.rs::split",
        );
        let probe = map_or_else.probe.clone();
        apply_rust_value_propagation_limit(&mut map_or_else, &probe, &index);
        assert_eq!(map_or_else.static_limit_kind, None);
        assert!(map_or_else.evidence.is_empty());
    }

    #[test]
    fn value_propagation_limit_stays_fail_closed_without_tests_or_when_already_named() {
        let expression = "let end = input.rfind(delim).map_or(0, |idx| idx);";
        let body = concat!(
            "    let end = input.rfind(delim).map_or(0, |idx| idx);\n",
            "    if end == start { return 1; }\n",
        );
        let index = RustIndex::from_owned(crate::analysis::facts::OwnedRustIndex {
            functions: vec![owner_function("src/lib.rs::split", "split", body)],
            ..Default::default()
        });

        let mut no_tests = static_unknown_finding(expression, "src/lib.rs::split");
        no_tests.related_tests.clear();
        let probe = no_tests.probe.clone();
        apply_rust_value_propagation_limit(&mut no_tests, &probe, &index);
        assert_eq!(no_tests.static_limit_kind, None);

        let mut already_named = static_unknown_finding(expression, "src/lib.rs::split");
        already_named.static_limit_kind =
            Some(StaticLimitKind::RustMacroWrappedAssertionUnresolved);
        let probe = already_named.probe.clone();
        apply_rust_value_propagation_limit(&mut already_named, &probe, &index);
        assert_eq!(
            already_named.static_limit_kind,
            Some(StaticLimitKind::RustMacroWrappedAssertionUnresolved)
        );
        assert!(already_named.evidence.is_empty());
    }

    #[test]
    fn changed_binding_predicate_evidence_is_disclosure_only() {
        let mut finding = static_unknown_finding(
            "let end = input.rfind(delim).map_or(0, |idx| idx);",
            "src/lib.rs::split",
        );
        finding.probe.before = Some("input.find(delim).map_or(0, |idx| idx)".to_string());
        finding.class = ExposureClass::WeaklyExposed;
        attach_changed_binding_predicate_evidence(&mut finding, None);
        assert!(finding.evidence.is_empty());
        assert_eq!(finding.class, ExposureClass::WeaklyExposed);
        assert_eq!(finding.static_limit_kind, None);

        let relation = ChangedBindingPredicateUse {
            binding: "end".to_string(),
            initializer: "input.rfind(delim).map_or(0, |idx| idx)".to_string(),
            predicate_expression: "end == start".to_string(),
            predicate_line: 4,
            operand_side: PredicateOperandSide::Left,
            value_resolution: BindingValueResolution::Unresolved {
                earliest_operation: "rfind".to_string(),
            },
        };
        attach_changed_binding_predicate_evidence(&mut finding, Some(&relation));
        assert_eq!(finding.class, ExposureClass::WeaklyExposed);
        assert_eq!(finding.static_limit_kind, None);
        assert!(finding.stop_reasons.is_empty());
        assert!(finding.evidence.iter().any(|line| {
            line == "binding_predicate_relation: changed binding `end` initializer `input.find(delim).map_or(0, |idx| idx)` -> `input.rfind(delim).map_or(0, |idx| idx)` flows into predicate operand at line 4"
        }));
        assert!(finding.evidence.iter().any(|line| {
            line.contains("binding_predicate_value_unresolved: operand value of `end` unresolved at earliest initializer operation `rfind`")
        }));
        assert!(finding.evidence.iter().any(|line| {
            line == "binding_predicate_non_claim: named analyzer limitation only; ripr does not confirm coverage or prescribe a repair test"
        }));
    }
}
