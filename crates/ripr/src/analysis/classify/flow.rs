use super::super::rust_index::FunctionSummary;
use super::text::exact_error_variant;
use crate::domain::*;

pub(in crate::analysis) fn propagation_evidence(
    probe: &Probe,
    flow_sinks: &[FlowSinkFact],
) -> StageEvidence {
    if matches!(probe.family, ProbeFamily::StaticUnknown) {
        return StageEvidence::new(
            StageState::Unknown,
            Confidence::Low,
            "No propagation model is available for this changed syntax",
        );
    }

    if let Some(sink) = flow_sinks
        .iter()
        .find(|sink| sink.kind != FlowSinkKind::Unknown)
    {
        StageEvidence::new(
            StageState::Yes,
            Confidence::Medium,
            format!(
                "Changed behavior appears to influence {}: {}",
                sink.kind.label(),
                sink.text
            ),
        )
    } else {
        StageEvidence::new(
            StageState::Unknown,
            Confidence::Low,
            "Propagation is not statically obvious from syntax-first analysis",
        )
    }
}

pub(in crate::analysis) fn local_flow_sinks(
    probe: &Probe,
    owner_fn: Option<&FunctionSummary>,
) -> Vec<FlowSinkFact> {
    let owner = owner_fn.map(|function| function.id.clone());
    let mut sinks = match probe.family {
        ProbeFamily::StaticUnknown => vec![flow_sink(
            FlowSinkKind::Unknown,
            "unknown sink",
            probe.location.line,
            owner.clone(),
        )],
        ProbeFamily::ErrorPath => vec![flow_sink(
            FlowSinkKind::ErrorVariant,
            result_error_text(&probe.expression),
            probe.location.line,
            owner.clone(),
        )],
        ProbeFamily::SideEffect | ProbeFamily::CallDeletion => {
            if probe.expression.contains("Err(") {
                vec![flow_sink(
                    FlowSinkKind::ErrorVariant,
                    result_error_text(&probe.expression),
                    probe.location.line,
                    owner.clone(),
                )]
            } else if probe.expression.starts_with("return ")
                || probe.expression.contains("Ok(")
                || probe.expression.contains("Some(")
            {
                vec![flow_sink(
                    FlowSinkKind::ReturnValue,
                    return_sink_text(&probe.expression),
                    probe.location.line,
                    owner.clone(),
                )]
            } else if value_is_swallowed(&probe.expression) {
                vec![flow_sink(
                    FlowSinkKind::Unknown,
                    "value is discarded at the call-chain tail; propagation unknown",
                    probe.location.line,
                    owner.clone(),
                )]
            } else if is_non_escaping_effect(&probe.expression, owner_fn) {
                vec![flow_sink(
                    FlowSinkKind::Unknown,
                    "effect does not escape the function scope; propagation unknown",
                    probe.location.line,
                    owner.clone(),
                )]
            } else {
                vec![flow_sink(
                    effect_sink_kind(&probe.expression),
                    call_effect_text(&probe.expression),
                    probe.location.line,
                    owner.clone(),
                )]
            }
        }
        ProbeFamily::FieldConstruction => vec![flow_sink(
            FlowSinkKind::StructField,
            field_sink_text(&probe.expression),
            probe.location.line,
            owner.clone(),
        )],
        ProbeFamily::MatchArm => vec![match_arm_sink(probe, owner.clone())],
        ProbeFamily::ReturnValue => vec![return_value_sink(probe, owner_fn, owner.clone())],
        ProbeFamily::Predicate => predicate_flow_sinks(probe, owner_fn, owner.clone()),
    };

    sinks.sort_by(|a, b| {
        a.kind
            .as_str()
            .cmp(b.kind.as_str())
            .then(a.line.cmp(&b.line))
            .then(a.text.cmp(&b.text))
    });
    sinks.dedup_by(|a, b| a.kind == b.kind && a.line == b.line && a.text == b.text);
    sinks
}

fn predicate_flow_sinks(
    probe: &Probe,
    owner_fn: Option<&FunctionSummary>,
    owner: Option<SymbolId>,
) -> Vec<FlowSinkFact> {
    if let Some(error) = first_error_return(owner_fn, probe.location.line) {
        return vec![flow_sink(
            FlowSinkKind::ErrorVariant,
            result_error_text(&error.text),
            error.line,
            owner,
        )];
    }
    if let Some(return_fact) = nearest_return(owner_fn, probe.location.line) {
        return vec![flow_sink(
            FlowSinkKind::ReturnValue,
            return_sink_text(&return_fact.text),
            return_fact.line,
            owner,
        )];
    }
    if let Some(field) = first_field_construction(owner_fn, probe.location.line) {
        return vec![flow_sink(
            FlowSinkKind::StructField,
            field_sink_text(&field.text),
            field.line,
            owner,
        )];
    }
    if let Some(branch) = next_branch_value(owner_fn, probe.location.line) {
        return vec![flow_sink(
            FlowSinkKind::ReturnValue,
            branch.text,
            branch.line,
            owner,
        )];
    }
    Vec::new()
}

fn return_value_sink(
    probe: &Probe,
    owner_fn: Option<&FunctionSummary>,
    owner: Option<SymbolId>,
) -> FlowSinkFact {
    if probe.expression.contains("Err(") {
        return flow_sink(
            FlowSinkKind::ErrorVariant,
            result_error_text(&probe.expression),
            probe.location.line,
            owner,
        );
    }
    if let Some(return_fact) = nearest_return(owner_fn, probe.location.line) {
        return flow_sink(
            FlowSinkKind::ReturnValue,
            return_sink_text(&return_fact.text),
            return_fact.line,
            owner,
        );
    }
    if !is_obvious_return_expression(&probe.expression) {
        return flow_sink(
            FlowSinkKind::Unknown,
            "unknown sink",
            probe.location.line,
            owner,
        );
    }
    flow_sink(
        FlowSinkKind::ReturnValue,
        return_sink_text(&probe.expression),
        probe.location.line,
        owner,
    )
}

fn match_arm_sink(probe: &Probe, owner: Option<SymbolId>) -> FlowSinkFact {
    let arm_result = probe
        .expression
        .split_once("=>")
        .map(|(_, result)| result.trim().trim_end_matches(',').to_string())
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| probe.expression.clone());

    if arm_result.contains("Err(") {
        flow_sink(
            FlowSinkKind::ErrorVariant,
            result_error_text(&arm_result),
            probe.location.line,
            owner,
        )
    } else {
        flow_sink(
            FlowSinkKind::MatchArm,
            arm_result,
            probe.location.line,
            owner,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LocalTextFact {
    line: usize,
    text: String,
}

fn first_error_return(
    owner_fn: Option<&FunctionSummary>,
    probe_line: usize,
) -> Option<LocalTextFact> {
    owner_fn.and_then(|function| {
        function
            .returns
            .iter()
            .find(|return_fact| return_fact.line >= probe_line && return_fact.text.contains("Err("))
            .map(|return_fact| LocalTextFact {
                line: return_fact.line,
                text: return_fact.text.clone(),
            })
    })
}

fn nearest_return(owner_fn: Option<&FunctionSummary>, probe_line: usize) -> Option<LocalTextFact> {
    owner_fn.and_then(|function| {
        function
            .returns
            .iter()
            .filter(|return_fact| return_fact.line >= probe_line)
            .min_by_key(|return_fact| return_fact.line - probe_line)
            .map(|return_fact| LocalTextFact {
                line: return_fact.line,
                text: return_fact.text.clone(),
            })
    })
}

fn next_branch_value(
    owner_fn: Option<&FunctionSummary>,
    probe_line: usize,
) -> Option<LocalTextFact> {
    let function = owner_fn?;
    let start_index = probe_line.saturating_sub(function.start_line);
    function
        .body
        .lines()
        .enumerate()
        .skip(start_index + 1)
        .find_map(|(offset, line)| {
            let text = line.trim().trim_end_matches(',').to_string();
            if !looks_like_branch_tail_expression(&text) {
                return None;
            }
            Some(LocalTextFact {
                line: function.start_line + offset,
                text,
            })
        })
}

fn first_field_construction(
    owner_fn: Option<&FunctionSummary>,
    probe_line: usize,
) -> Option<LocalTextFact> {
    owner_fn.and_then(|function| {
        function
            .body
            .lines()
            .enumerate()
            .skip(probe_line.saturating_sub(function.start_line))
            .find_map(|(offset, line)| {
                let text = line.trim().trim_end_matches(',').to_string();
                if looks_like_field_assignment(&text) {
                    Some(LocalTextFact {
                        line: function.start_line + offset,
                        text,
                    })
                } else {
                    None
                }
            })
    })
}

fn flow_sink(
    kind: FlowSinkKind,
    text: impl Into<String>,
    line: usize,
    owner: Option<SymbolId>,
) -> FlowSinkFact {
    FlowSinkFact {
        kind,
        text: text.into(),
        line,
        owner,
    }
}

/// Returns true when the effect cannot escape the function's observable boundary.
///
/// Three categories (conservative — only provably-local cases flagged):
/// 1. `println!` / `eprintln!` — stdout macros that produce no capturable
///    artifact from a static-analysis perspective.  `log::` / `tracing::` are
///    KEPT as LogMessage (they go to a capturable sink).
/// 2. `.push(..)` / `.insert(..)` / `.write(..)` on a provably function-local
///    receiver — scanned from `owner_fn.body` for a `let [mut] <recv> = …`
///    binding with no subsequent `return <recv>` or field-store of `<recv>`.
/// 3. Trait-object receiver (`&dyn` / `Box<dyn`) — we cannot statically prove
///    where dispatch resolves.
fn is_non_escaping_effect(text: &str, owner_fn: Option<&FunctionSummary>) -> bool {
    let trimmed = text.trim();
    // Category 1: stdout macros (but NOT log:: / tracing:: which are capturable)
    if is_stdout_macro(trimmed) {
        return true;
    }
    // Category 3: trait-object receiver  (&dyn Trait::m or Box<dyn Trait>::m)
    if has_trait_object_receiver(trimmed) {
        return true;
    }
    // Category 2: local-dropped collection receiver
    if let Some(receiver) = collection_receiver(trimmed)
        && is_function_local_dropped_receiver(&receiver, owner_fn)
    {
        return true;
    }
    false
}

/// True for `println!(…)` / `eprintln!(…)` but NOT `log::`, `tracing::`,
/// `info!(`, `warn!(`, etc.  Those go to a capturable sink and stay as
/// `LogMessage`.
fn is_stdout_macro(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    // Must start with the macro name to avoid matching a call like
    // `do_println(x)`.
    lower.trim_start().starts_with("println!(") || lower.trim_start().starts_with("eprintln!(")
}

/// True when the receiver is an opaque trait object.
/// We look for `(&dyn` / `(Box<dyn` in the call chain.
fn has_trait_object_receiver(text: &str) -> bool {
    text.contains("&dyn ") || text.contains("Box<dyn ")
}

/// Extract the receiver name from `.push(` / `.insert(` / `.write(` calls,
/// e.g. `vec.push(x)` → `Some("vec")`, `self.push(x)` → `None` (self escapes).
fn collection_receiver(text: &str) -> Option<String> {
    for method in &[".push(", ".insert(", ".write("] {
        if let Some(dot_pos) = text.find(method) {
            // Walk backwards to find the receiver name
            let before = &text[..dot_pos];
            let receiver = before
                .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
                .next_back()
                .unwrap_or("")
                .to_string();
            if !receiver.is_empty() && receiver != "self" {
                return Some(receiver);
            }
            // receiver is `self` or empty — cannot determine, leave as Yes
        }
    }
    None
}

/// Returns true when `receiver` is provably declared and dropped inside
/// `owner_fn` — i.e. there is a `let [mut] <receiver> = …` in the body and
/// no `return <receiver>` / `self.<field> = <receiver>` after it.
fn is_function_local_dropped_receiver(receiver: &str, owner_fn: Option<&FunctionSummary>) -> bool {
    let Some(function) = owner_fn else {
        return false;
    };
    let body = &function.body;
    // Check for a let binding of this receiver
    let let_pat_plain = format!("let {receiver} =");
    let let_pat_mut = format!("let mut {receiver} =");
    let has_local_binding = body.contains(&let_pat_plain) || body.contains(&let_pat_mut);
    if !has_local_binding {
        return false;
    }
    // Check it does NOT escape via return or field-store
    let escapes = body.contains(&format!("return {receiver}"))
        || body.contains(&format!("return {receiver};"))
        || body.contains(&format!("self. = {receiver}")) // self.<any_field> = recv
        || body
            .lines()
            .any(|line| looks_like_field_store_of(line, receiver));
    !escapes
}

/// Heuristic: `self.<field> = <receiver>` lines escape the receiver.
fn looks_like_field_store_of(line: &str, receiver: &str) -> bool {
    let trimmed = line.trim();
    // Pattern: `self.<ident> = <receiver>` or `self.<ident> = <receiver>;`
    if !trimmed.starts_with("self.") {
        return false;
    }
    let suffix = &trimmed["self.".len()..];
    // There should be ` = <receiver>` somewhere after the field name
    suffix.contains(&format!("= {receiver}")) || suffix.contains(&format!("= {receiver};"))
}

/// Returns true when the call-chain tail provably discards the returned value,
/// meaning the changed call cannot propagate through a return/error/field path.
///
/// Conservative: only matches exact tail patterns so `x.ok().map(f)` is NOT
/// flagged (the value continues to flow).
fn value_is_swallowed(text: &str) -> bool {
    let trimmed = text.trim();
    // Pattern 1: `let _ = <expr>;`  — wildcard-discard binding
    if trimmed.starts_with("let _ =") {
        return true;
    }
    // Pattern 2: trailing `.ok();`  — result converted to Option and dropped
    // We strip the trailing `;` first, then check if the trimmed tail is ".ok()"
    let without_semi = trimmed.trim_end_matches(';').trim_end();
    if without_semi.ends_with(".ok()") {
        return true;
    }
    // Pattern 3: `drop(<expr>)` wrapper — explicit discard
    if trimmed.starts_with("drop(") {
        return true;
    }
    // Pattern 4: `= ();` — assignment of unit value (explicit unit discard)
    if trimmed.ends_with("= ();") || trimmed == "= ()" {
        return true;
    }
    false
}

fn effect_sink_kind(text: &str) -> FlowSinkKind {
    let normalized = text.to_ascii_lowercase();
    if looks_like_log_effect(&normalized) {
        FlowSinkKind::LogMessage
    } else if looks_like_config_effect(&normalized) {
        FlowSinkKind::ConfigChange
    } else if looks_like_persistence_effect(&normalized) {
        FlowSinkKind::Persistence
    } else if looks_like_event_call_effect(&normalized) {
        FlowSinkKind::EventCall
    } else if looks_like_state_write_effect(&normalized) {
        FlowSinkKind::StateWrite
    } else {
        FlowSinkKind::CallEffect
    }
}

fn looks_like_event_call_effect(text: &str) -> bool {
    [
        ".publish(",
        ".emit(",
        ".send(",
        ".dispatch(",
        ".notify(",
        ".enqueue(",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

fn looks_like_state_write_effect(text: &str) -> bool {
    [
        ".write(",
        ".insert(",
        ".push(",
        ".remove(",
        ".delete(",
        ".increment(",
        ".replace(",
        ".clear(",
        ".extend(",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

fn looks_like_persistence_effect(text: &str) -> bool {
    [".save(", ".persist(", ".store(", ".commit(", ".upsert("]
        .iter()
        .any(|needle| text.contains(needle))
}

fn looks_like_log_effect(text: &str) -> bool {
    text.contains("log::")
        || text.contains("tracing::")
        || [
            "println!(",
            "eprintln!(",
            "trace!(",
            "debug!(",
            "info!(",
            "warn!(",
            "error!(",
        ]
        .iter()
        .any(|needle| text.contains(needle))
}

fn looks_like_config_effect(text: &str) -> bool {
    text.contains("config.")
        || text.contains("settings.")
        || [
            ".set_config(",
            ".configure(",
            ".set_option(",
            ".set_default(",
            ".set_var(",
        ]
        .iter()
        .any(|needle| text.contains(needle))
}

fn result_error_text(text: &str) -> String {
    if let Some(variant) = exact_error_variant(text) {
        return format!("Result::Err({variant})");
    }
    if let Some(start) = text.find("Err(") {
        let error = text[start..]
            .trim()
            .trim_start_matches("return ")
            .trim_end_matches(';')
            .trim_end_matches(',')
            .to_string();
        return format!("Result::{error}");
    }
    return_sink_text(text)
}

fn return_sink_text(text: &str) -> String {
    text.trim()
        .trim_start_matches("return ")
        .trim_end_matches(';')
        .trim_end_matches(',')
        .trim()
        .to_string()
}

fn call_effect_text(text: &str) -> String {
    return_sink_text(text)
}

fn field_sink_text(text: &str) -> String {
    return_sink_text(text)
}

fn looks_like_field_assignment(text: &str) -> bool {
    let Some((field, _)) = text.split_once(':') else {
        return false;
    };
    if text.contains("::") {
        return false;
    }
    let field = field.trim();
    !field.is_empty()
        && field
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        && field
            .chars()
            .next()
            .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
}

fn looks_like_branch_tail_expression(text: &str) -> bool {
    if text.is_empty()
        || text == "{"
        || text == "}"
        || text.starts_with("else")
        || text.starts_with("//")
        || text.starts_with("let ")
        || text.ends_with(';')
    {
        return false;
    }
    if text.contains(" = ")
        || text.contains(" += ")
        || text.contains(" -= ")
        || text.contains(" *= ")
        || text.contains(" /= ")
    {
        return false;
    }
    is_obvious_return_expression(text)
}

fn is_obvious_return_expression(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.starts_with("return ")
        || trimmed.starts_with("Ok(")
        || trimmed.starts_with("Some(")
        || trimmed.contains("Err(")
        || trimmed.contains('(')
        || trimmed.contains('"')
        || trimmed.chars().any(|ch| ch.is_ascii_digit())
        || [" + ", " - ", " * ", " / ", " % "]
            .iter()
            .any(|operator| trimmed.contains(operator))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::rust_index::ReturnFact;
    use std::path::PathBuf;

    #[test]
    fn predicate_flow_uses_nearest_return_after_changed_line() {
        let owner = function(
            "pub fn score(amount: i32) -> i32 {\n    if amount > 10 {\n        amount - 1\n    }\n}",
        );
        let probe = probe(ProbeFamily::Predicate, "amount > 10", 2);

        let sinks = local_flow_sinks(&probe, Some(&owner));

        assert_eq!(sinks.len(), 1);
        assert_eq!(sinks[0].kind, FlowSinkKind::ReturnValue);
        assert_eq!(sinks[0].text, "amount - 1");
        assert_eq!(sinks[0].line, 3);
    }

    #[test]
    fn error_path_flow_uses_exact_error_variant_text() {
        let probe = probe(
            ProbeFamily::ErrorPath,
            "return Err(AuthError::RevokedToken);",
            2,
        );

        let sinks = local_flow_sinks(&probe, None);

        assert_eq!(sinks.len(), 1);
        assert_eq!(sinks[0].kind, FlowSinkKind::ErrorVariant);
        assert_eq!(sinks[0].text, "Result::Err(AuthError::RevokedToken)");
    }

    #[test]
    fn propagation_names_first_visible_sink() {
        let probe = probe(ProbeFamily::Predicate, "amount > 10", 2);
        let sinks = vec![FlowSinkFact {
            kind: FlowSinkKind::ReturnValue,
            text: "amount - 1".to_string(),
            line: 3,
            owner: None,
        }];

        let evidence = propagation_evidence(&probe, &sinks);

        assert_eq!(evidence.state, StageState::Yes);
        assert_eq!(
            evidence.summary,
            "Changed behavior appears to influence returned value: amount - 1"
        );
    }

    #[test]
    fn propagation_is_unknown_for_static_unknown_probe() {
        let probe = probe(ProbeFamily::StaticUnknown, "let value = total;", 2);

        let evidence = propagation_evidence(&probe, &[]);

        assert_eq!(evidence.state, StageState::Unknown);
        assert_eq!(
            evidence.summary,
            "No propagation model is available for this changed syntax"
        );
    }

    #[test]
    fn propagation_is_unknown_when_only_unknown_flow_sink_exists() {
        let probe = probe(ProbeFamily::ReturnValue, "opaque_value", 2);
        let sinks = vec![FlowSinkFact {
            kind: FlowSinkKind::Unknown,
            text: "unknown sink".to_string(),
            line: 2,
            owner: None,
        }];

        let evidence = propagation_evidence(&probe, &sinks);

        assert_eq!(evidence.state, StageState::Unknown);
        assert_eq!(
            evidence.summary,
            "Propagation is not statically obvious from syntax-first analysis"
        );
    }

    #[test]
    fn static_unknown_flow_returns_unknown_sink() {
        let probe = probe(ProbeFamily::StaticUnknown, "let value = total;", 2);

        let sinks = local_flow_sinks(&probe, None);

        assert_eq!(sinks.len(), 1);
        assert_eq!(sinks[0].kind, FlowSinkKind::Unknown);
        assert_eq!(sinks[0].text, "unknown sink");
    }

    #[test]
    fn side_effect_flow_distinguishes_error_return_and_call_effect() {
        let error_probe = probe(
            ProbeFamily::SideEffect,
            "return Err(AuthError::ExpiredToken);",
            2,
        );
        let call_probe = probe(ProbeFamily::SideEffect, "adapter.flush();", 2);

        let error_sinks = local_flow_sinks(&error_probe, None);
        let call_sinks = local_flow_sinks(&call_probe, None);

        assert_eq!(error_sinks[0].kind, FlowSinkKind::ErrorVariant);
        assert_eq!(error_sinks[0].text, "Result::Err(AuthError::ExpiredToken)");
        assert_eq!(call_sinks[0].kind, FlowSinkKind::CallEffect);
        assert_eq!(call_sinks[0].text, "adapter.flush()");
    }

    #[test]
    fn side_effect_flow_names_event_state_persistence_log_and_config_sinks() {
        let cases = [
            ("events.publish(score);", FlowSinkKind::EventCall),
            ("cache.insert(key, value);", FlowSinkKind::StateWrite),
            ("repository.save(invoice);", FlowSinkKind::Persistence),
            ("log::info!(\"saved\");", FlowSinkKind::LogMessage),
            (
                "config.set_option(\"mode\", mode);",
                FlowSinkKind::ConfigChange,
            ),
        ];

        for (expression, expected_kind) in cases {
            let probe = probe(ProbeFamily::SideEffect, expression, 2);
            let sinks = local_flow_sinks(&probe, None);

            assert_eq!(sinks.len(), 1, "{expression}");
            assert_eq!(sinks[0].kind, expected_kind, "{expression}");
            assert_eq!(
                sinks[0].text,
                expression.trim_end_matches(';'),
                "{expression}"
            );
        }
    }

    #[test]
    fn call_deletion_flow_distinguishes_return_value() {
        let probe = probe(ProbeFamily::CallDeletion, "return Ok(total);", 2);

        let sinks = local_flow_sinks(&probe, None);

        assert_eq!(sinks.len(), 1);
        assert_eq!(sinks[0].kind, FlowSinkKind::ReturnValue);
        assert_eq!(sinks[0].text, "Ok(total)");
    }

    #[test]
    fn field_construction_flow_reports_struct_field() {
        let probe = probe(ProbeFamily::FieldConstruction, "status: Status::Ready", 2);

        let sinks = local_flow_sinks(&probe, None);

        assert_eq!(sinks.len(), 1);
        assert_eq!(sinks[0].kind, FlowSinkKind::StructField);
        assert_eq!(sinks[0].text, "status: Status::Ready");
    }

    #[test]
    fn match_arm_flow_distinguishes_error_variant_and_match_result() {
        let error_probe = probe(
            ProbeFamily::MatchArm,
            "State::Bad => Err(AuthError::Bad),",
            2,
        );
        let value_probe = probe(ProbeFamily::MatchArm, "State::Good => total + 1,", 2);

        let error_sinks = local_flow_sinks(&error_probe, None);
        let value_sinks = local_flow_sinks(&value_probe, None);

        assert_eq!(error_sinks[0].kind, FlowSinkKind::ErrorVariant);
        assert_eq!(error_sinks[0].text, "Result::Err(AuthError::Bad)");
        assert_eq!(value_sinks[0].kind, FlowSinkKind::MatchArm);
        assert_eq!(value_sinks[0].text, "total + 1");
    }

    #[test]
    fn return_value_flow_distinguishes_unknown_and_obvious_expression() {
        let unknown_probe = probe(ProbeFamily::ReturnValue, "opaque_value", 2);
        let value_probe = probe(ProbeFamily::ReturnValue, "total + 1", 2);

        let unknown_sinks = local_flow_sinks(&unknown_probe, None);
        let value_sinks = local_flow_sinks(&value_probe, None);

        assert_eq!(unknown_sinks[0].kind, FlowSinkKind::Unknown);
        assert_eq!(unknown_sinks[0].text, "unknown sink");
        assert_eq!(value_sinks[0].kind, FlowSinkKind::ReturnValue);
        assert_eq!(value_sinks[0].text, "total + 1");
    }

    #[test]
    fn predicate_flow_uses_field_construction_when_no_return_is_available() {
        let owner = FunctionSummary {
            id: SymbolId("src/lib.rs::score".to_string()),
            name: "score".to_string(),
            file: PathBuf::from("src/lib.rs"),
            start_line: 1,
            end_line: 5,
            body: "pub fn score(amount: i32) -> Response {\n    if amount > 10 {\n        status: ready,\n    }\n}"
                .to_string(),
            calls: Vec::new(),
            returns: Vec::new(),
            literals: Vec::new(),
            is_test: false,
            attrs: Vec::new(),
        };
        let probe = probe(ProbeFamily::Predicate, "amount > 10", 2);

        let sinks = local_flow_sinks(&probe, Some(&owner));

        assert_eq!(sinks.len(), 1);
        assert_eq!(sinks[0].kind, FlowSinkKind::StructField);
        assert_eq!(sinks[0].text, "status: ready");
    }

    fn function(body: &str) -> FunctionSummary {
        FunctionSummary {
            id: SymbolId("src/lib.rs::score".to_string()),
            name: "score".to_string(),
            file: PathBuf::from("src/lib.rs"),
            start_line: 1,
            end_line: body.lines().count(),
            body: body.to_string(),
            calls: Vec::new(),
            returns: vec![ReturnFact {
                line: 3,
                text: "amount - 1".to_string(),
            }],
            literals: Vec::new(),
            is_test: false,
            attrs: Vec::new(),
        }
    }

    // ── Fix B: value_is_swallowed ─────────────────────────────────────────

    #[test]
    fn swallowed_ok_tail_yields_unknown_sink() {
        let probe = probe(ProbeFamily::SideEffect, "self.persist(amount * 9).ok();", 2);
        let sinks = local_flow_sinks(&probe, None);
        assert_eq!(sinks.len(), 1);
        assert_eq!(sinks[0].kind, FlowSinkKind::Unknown);
        assert!(sinks[0].text.contains("discarded"));
    }

    #[test]
    fn wildcard_discard_binding_yields_unknown_sink() {
        let probe = probe(ProbeFamily::SideEffect, "let _ = compute(x);", 2);
        let sinks = local_flow_sinks(&probe, None);
        assert_eq!(sinks.len(), 1);
        assert_eq!(sinks[0].kind, FlowSinkKind::Unknown);
    }

    #[test]
    fn drop_wrapper_yields_unknown_sink() {
        let probe = probe(ProbeFamily::SideEffect, "drop(compute(x));", 2);
        let sinks = local_flow_sinks(&probe, None);
        assert_eq!(sinks.len(), 1);
        assert_eq!(sinks[0].kind, FlowSinkKind::Unknown);
    }

    #[test]
    fn returned_call_is_not_swallowed_stays_return_value() {
        // Control: `return self.persist(amount*9)` is NOT swallowed — stays exposed
        let probe = probe(
            ProbeFamily::SideEffect,
            "return self.persist(amount * 9);",
            2,
        );
        let sinks = local_flow_sinks(&probe, None);
        assert_eq!(sinks.len(), 1);
        assert_eq!(sinks[0].kind, FlowSinkKind::ReturnValue);
    }

    #[test]
    fn chained_ok_map_is_not_swallowed() {
        // `x.ok().map(f)` is NOT a tail-discard: the value continues to flow
        let probe = probe(
            ProbeFamily::SideEffect,
            "self.compute().ok().map(transform)",
            2,
        );
        let sinks = local_flow_sinks(&probe, None);
        assert_eq!(sinks.len(), 1);
        // Should NOT be Unknown — the value is not swallowed
        assert_ne!(sinks[0].kind, FlowSinkKind::Unknown);
    }

    // ── Fix C: is_non_escaping_effect ────────────────────────────────────

    #[test]
    fn println_macro_yields_unknown_sink() {
        let probe = probe(
            ProbeFamily::SideEffect,
            "println!(\"amount is {}\", amount * 9);",
            2,
        );
        let sinks = local_flow_sinks(&probe, None);
        assert_eq!(sinks.len(), 1);
        assert_eq!(sinks[0].kind, FlowSinkKind::Unknown);
    }

    #[test]
    fn eprintln_macro_yields_unknown_sink() {
        let probe = probe(ProbeFamily::SideEffect, "eprintln!(\"err {}\", msg);", 2);
        let sinks = local_flow_sinks(&probe, None);
        assert_eq!(sinks.len(), 1);
        assert_eq!(sinks[0].kind, FlowSinkKind::Unknown);
    }

    #[test]
    fn log_info_macro_stays_log_message_not_downgraded() {
        // Control: `log::info!` is a capturable sink — must NOT be downgraded
        let probe = probe(
            ProbeFamily::SideEffect,
            "log::info!(\"amount is {}\", amount * 9);",
            2,
        );
        let sinks = local_flow_sinks(&probe, None);
        assert_eq!(sinks.len(), 1);
        assert_eq!(sinks[0].kind, FlowSinkKind::LogMessage);
    }

    #[test]
    fn local_vec_push_yields_unknown_sink() {
        // A provably-local Vec receiver: `let mut items = Vec::new(); items.push(x)`
        let owner = FunctionSummary {
            id: SymbolId("src/lib.rs::collect".to_string()),
            name: "collect".to_string(),
            file: PathBuf::from("src/lib.rs"),
            start_line: 1,
            end_line: 5,
            body:
                "pub fn collect(x: i32) {\n    let mut items = Vec::new();\n    items.push(x);\n}"
                    .to_string(),
            calls: Vec::new(),
            returns: Vec::new(),
            literals: Vec::new(),
            is_test: false,
            attrs: Vec::new(),
        };
        let probe = probe(ProbeFamily::SideEffect, "items.push(x * 9);", 3);
        let sinks = local_flow_sinks(&probe, Some(&owner));
        assert_eq!(sinks.len(), 1);
        assert_eq!(sinks[0].kind, FlowSinkKind::Unknown);
    }

    #[test]
    fn self_field_push_stays_state_write_not_downgraded() {
        // Control: `self.items.push(x)` — `self` is not a local binding, must stay Yes
        let probe = probe(ProbeFamily::SideEffect, "self.items.push(x * 9);", 2);
        let sinks = local_flow_sinks(&probe, None);
        assert_eq!(sinks.len(), 1);
        // `self` receiver is NOT considered local-dropped — stays StateWrite
        assert_eq!(sinks[0].kind, FlowSinkKind::StateWrite);
    }

    fn probe(family: ProbeFamily, expression: &str, line: usize) -> Probe {
        Probe {
            id: ProbeId("probe:test".to_string()),
            location: SourceLocation::new("src/lib.rs", line, 1),
            owner: Some(SymbolId("src/lib.rs::score".to_string())),
            family,
            delta: DeltaKind::Control,
            before: None,
            after: Some(expression.to_string()),
            expression: expression.to_string(),
            expected_sinks: Vec::new(),
            required_oracles: Vec::new(),
        }
    }
}
