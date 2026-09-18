use super::super::rust_index::{FunctionSummary, TestSummary};
use super::text::{delimited_contents_at, enum_variant_values, exact_error_variant};
use crate::domain::*;

pub(in crate::analysis) fn activation_evidence(
    probe: &Probe,
    owner_fn: Option<&FunctionSummary>,
    related_tests: &[&TestSummary],
    flow_sinks: &[FlowSinkFact],
    helper_chain: Option<&super::helper_transfer::HelperChain>,
    index: &crate::analysis::rust_index::RustIndex,
    workspace_complete: bool,
) -> ActivationEvidence {
    let mut observed_values = related_tests
        .iter()
        .flat_map(|test| value_facts_for_test(test, owner_fn))
        .collect::<Vec<_>>();
    observed_values.extend(observed_discriminator_values(
        probe,
        owner_fn,
        related_tests,
        helper_chain,
        index,
        workspace_complete,
    ));
    sort_value_facts(&mut observed_values);

    let mut missing_discriminators = missing_discriminator_facts(
        probe,
        owner_fn,
        related_tests,
        flow_sinks,
        &observed_values,
        helper_chain,
        index,
        workspace_complete,
    );
    missing_discriminators.sort_by(|left, right| {
        left.value
            .cmp(&right.value)
            .then(left.reason.cmp(&right.reason))
            .then(
                left.flow_sink
                    .as_ref()
                    .map(|sink| sink.kind.as_str())
                    .cmp(&right.flow_sink.as_ref().map(|sink| sink.kind.as_str())),
            )
    });
    missing_discriminators
        .dedup_by(|left, right| left.value == right.value && left.reason == right.reason);

    ActivationEvidence {
        observed_values,
        missing_discriminators,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ParameterValue {
    parameter: String,
    value: String,
    line: usize,
    text: String,
}

fn value_facts_for_test(test: &TestSummary, owner_fn: Option<&FunctionSummary>) -> Vec<ValueFact> {
    let owner_name = owner_fn.map(|owner| owner.name.as_str()).unwrap_or("");
    let parameters = owner_fn.map(function_parameters).unwrap_or_default();
    let mut facts = Vec::new();

    for call in &test.calls {
        if !owner_name.is_empty() && call.name != owner_name {
            continue;
        }
        let Some(arguments) = call_arguments(&call.text, &call.name) else {
            continue;
        };
        for (idx, argument) in arguments.iter().enumerate() {
            for value in scalar_values(argument) {
                let value = parameters
                    .get(idx)
                    .map(|parameter| format!("{parameter} = {value}"))
                    .unwrap_or(value);
                facts.push(ValueFact {
                    line: call.line,
                    text: call.text.clone(),
                    value,
                    context: ValueContext::FunctionArgument,
                });
            }
            for value in enum_variant_values(argument) {
                facts.push(ValueFact {
                    line: call.line,
                    text: call.text.clone(),
                    value,
                    context: ValueContext::EnumVariant,
                });
            }
        }
    }

    for assertion in &test.assertions {
        let assertion_arguments = macro_arguments(&assertion.text).unwrap_or_default();
        for argument in assertion_arguments {
            if argument.contains(owner_name) && !owner_name.is_empty() {
                continue;
            }
            for value in scalar_values(&argument) {
                facts.push(ValueFact {
                    line: assertion.line,
                    text: assertion.text.clone(),
                    value,
                    context: ValueContext::AssertionArgument,
                });
            }
        }
        for value in enum_variant_values(&assertion.text) {
            facts.push(ValueFact {
                line: assertion.line,
                text: assertion.text.clone(),
                value,
                context: ValueContext::EnumVariant,
            });
        }
    }

    for (offset, line) in test.body.lines().enumerate() {
        let line_number = test.start_line + offset;
        let trimmed = line.trim();
        if looks_like_table_row(trimmed) {
            for value in scalar_values(trimmed) {
                facts.push(ValueFact {
                    line: line_number,
                    text: trimmed.to_string(),
                    value,
                    context: ValueContext::TableRow,
                });
            }
        }
        if looks_like_builder_method(trimmed) {
            for value in scalar_values(trimmed) {
                facts.push(ValueFact {
                    line: line_number,
                    text: trimmed.to_string(),
                    value,
                    context: ValueContext::BuilderMethod,
                });
            }
        }
    }

    sort_value_facts(&mut facts);
    facts
}

fn observed_discriminator_values(
    probe: &Probe,
    owner_fn: Option<&FunctionSummary>,
    related_tests: &[&TestSummary],
    helper_chain: Option<&super::helper_transfer::HelperChain>,
    index: &crate::analysis::rust_index::RustIndex,
    workspace_complete: bool,
) -> Vec<ValueFact> {
    let Some((left, right)) = comparison_operands(&probe.expression) else {
        return Vec::new();
    };
    let Some(owner) = owner_fn else {
        return Vec::new();
    };
    let parameters = function_parameters(owner);
    let call_values = call_values_for_owner(owner, &parameters, related_tests, helper_chain);
    let left_parameter = boundary_operand_parameter(owner, &parameters, &left);
    let right_parameter = boundary_operand_parameter(owner, &parameters, &right);
    // #3295: the operands resolve once per probe (initializer or
    // parameter); only the input row changes per call.
    let left_resolved = resolve_boundary_operand(
        owner,
        &left,
        probe.location.line,
        &parameters,
        index,
        workspace_complete,
    );
    let right_resolved = resolve_boundary_operand(
        owner,
        &right,
        probe.location.line,
        &parameters,
        index,
        workspace_complete,
    );
    let mut facts = Vec::new();

    for row in call_values {
        let inputs: super::value_transfer::ExactInputs = row
            .iter()
            .map(|cell| (cell.parameter.clone(), cell.value.clone()))
            .collect();
        let left_exact = left_resolved.as_ref().and_then(|resolved| {
            exact_operand_for_row(resolved, &row, &inputs, index, workspace_complete)
        });
        let right_exact = right_resolved
            .as_ref()
            .and_then(|resolved| {
                exact_operand_for_row(resolved, &row, &inputs, index, workspace_complete)
            })
            .or_else(|| {
                literal_operand_value(&right).map(|value| ExactOperand {
                    provenance: format!("literal operand {right} = {value}"),
                    value,
                })
            });
        if let (Some(left_value), Some(right_value)) = (&left_exact, &right_exact)
            && left_value.value == right_value.value
        {
            facts.push(ValueFact {
                line: row.first().map(|cell| cell.line).unwrap_or_default(),
                text: format!(
                    "{} | {}; {}",
                    row.first()
                        .map(|cell| cell.text.clone())
                        .unwrap_or_default(),
                    left_value.provenance,
                    right_value.provenance,
                ),
                value: format!("{left} == {right}"),
                context: ValueContext::FunctionArgument,
            });
            continue;
        }
        let Some(left_parameter) = left_parameter.as_deref() else {
            continue;
        };
        let Some(left_value) = parameter_value(&row, left_parameter) else {
            continue;
        };
        let right_value = right_parameter
            .as_deref()
            .and_then(|parameter| parameter_value(&row, parameter))
            .map(|value| value.value)
            .or_else(|| literal_operand_value(&right));
        if right_value
            .as_deref()
            .is_some_and(|value| comparable_value(value) == comparable_value(&left_value.value))
        {
            facts.push(ValueFact {
                line: left_value.line,
                text: left_value.text.clone(),
                value: format!("{left} == {right}"),
                context: ValueContext::FunctionArgument,
            });
        }
    }

    facts
}

/// The exact value of one comparison operand under one related-test
/// call row (#3295). A parameter resolves to the row's literal; a
/// local binding resolves through the #3294 binding relation (the
/// predicate must be a direct use in the binding's live span) and the
/// bounded value-transfer evaluator. `None` keeps the operand unknown.
/// One comparison operand resolved once per probe (#3295 review): a
/// shadowing local's initializer (evaluated per row) or the raw
/// parameter.
pub(crate) enum ResolvedOperand {
    Parameter(String),
    Local(String, String),
    /// A direct call to a unique helper (`is_word_start(input, 0)`)
    /// whose return value is evaluated over the row's bound inputs
    /// (#3296 boolean-predicate helper family).
    Call {
        callee: Box<crate::analysis::facts::FunctionSummary>,
        arguments: Vec<String>,
    },
}

fn resolve_boundary_operand(
    owner: &FunctionSummary,
    operand: &str,
    predicate_line: usize,
    parameters: &[String],
    index: &crate::analysis::rust_index::RustIndex,
    workspace_complete: bool,
) -> Option<ResolvedOperand> {
    if let Some(initializer) = live_local_initializer(owner, operand, predicate_line) {
        // #3296 scanner/helper remainder: a local whose initializer is
        // itself a direct call to a unique helper (`let final_state =
        // scan_state(input)`) jumps to the helper authority — the value
        // evaluator fails closed on call expressions, so without the
        // jump the operand stays unknown whatever the callee's body.
        if let Some(call) = resolve_direct_call(&initializer, index, workspace_complete) {
            return Some(call);
        }
        return Some(ResolvedOperand::Local(operand.to_string(), initializer));
    }
    if let Some(parameter) = parameters
        .iter()
        .find(|parameter| parameter.as_str() == operand)
    {
        return Some(ResolvedOperand::Parameter(parameter.clone()));
    }
    resolve_direct_call(operand, index, workspace_complete)
}

/// Decompose a direct-call text (`is_word_start(input, 0)`) into the
/// unique callee and its statically splittable arguments. Method or
/// path-qualified call sites, non-unique callee names, and unsplittable
/// argument lists stay unresolved (fail closed).
pub(crate) fn resolve_direct_call(
    text: &str,
    index: &crate::analysis::rust_index::RustIndex,
    workspace_complete: bool,
) -> Option<ResolvedOperand> {
    let trimmed = text.trim().trim_end_matches(';').trim();
    let callee_name = trimmed
        .split('(')
        .next()
        .filter(|name| {
            !name.is_empty()
                && name
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        })?
        .to_string();
    if !trimmed.contains('(') || !trimmed.ends_with(')') {
        return None;
    }
    if !workspace_complete || !super::helper_transfer::callee_is_unique(&callee_name, index) {
        return None;
    }
    let callee = index
        .functions
        .iter()
        .find(|function| function.name == callee_name)?;
    let arguments = super::helper_transfer::split_call_arguments_text(trimmed, &callee_name)?;
    Some(ResolvedOperand::Call {
        callee: Box::new(callee.clone()),
        arguments,
    })
}

/// Evaluate one resolved operand against one related-test call row.
fn exact_operand_for_row(
    resolved: &ResolvedOperand,
    row: &[ParameterValue],
    inputs: &super::value_transfer::ExactInputs,
    index: &crate::analysis::rust_index::RustIndex,
    workspace_complete: bool,
) -> Option<ExactOperand> {
    match resolved {
        ResolvedOperand::Local(name, initializer) => {
            match super::value_transfer::evaluate_initializer(initializer, inputs) {
                super::value_transfer::EvalOutcome::Exact { value, provenance } => Some(
                    exact_operand_from_evaluation(name, &value, &provenance, inputs),
                ),
                _ => None,
            }
        }
        ResolvedOperand::Parameter(parameter) => {
            let cell = row
                .iter()
                .find(|cell| cell.parameter == *parameter)
                .cloned()?;
            Some(ExactOperand {
                value: cell.value.clone(),
                provenance: format!("exact input {parameter} = {}", cell.value),
            })
        }
        ResolvedOperand::Call { callee, arguments } => {
            // Bind the call-site arguments against the owner's row
            // (literal or the owner's parameter), then evaluate the
            // helper's return over the bound inputs. A computed
            // argument stops the operand (no guessed value).
            let owner_parameters = function_parameters_of_row(row);
            let mut bound = super::value_transfer::ExactInputs::new();
            for (index, argument) in arguments.iter().enumerate() {
                let parameter = function_parameters(callee)
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| format!("arg{index}"));
                let value = if let Some(literal) = super::helper_transfer::strict_literal(argument)
                {
                    literal
                } else if let Some(cell) = owner_parameters
                    .iter()
                    .position(|parameter| parameter.as_str() == argument.trim())
                    .and_then(|position| row.get(position))
                {
                    cell.value.clone()
                } else {
                    return None;
                };
                bound.insert(parameter, value);
            }
            let eval = super::helper_transfer::HelperEval::root(index, workspace_complete);
            let value = super::helper_transfer::helper_return_value(callee, &bound, &eval)?;
            Some(ExactOperand {
                value: value.render(),
                provenance: format!(
                    "{} = {} via helper return of `{}` over bound inputs (1 hop)",
                    callee.name,
                    value.render(),
                    callee.name
                ),
            })
        }
    }
}

/// The parameter names implied by a row's cells, in binding order.
fn function_parameters_of_row(row: &[ParameterValue]) -> Vec<String> {
    row.iter().map(|cell| cell.parameter.clone()).collect()
}

/// Build the exact operand with its provenance chain: operation
/// families, source inputs, and chain depth (#3295 evidence contract).
fn exact_operand_from_evaluation(
    operand: &str,
    value: &super::value_transfer::TypedValue,
    provenance: &[super::value_transfer::EvalStep],
    inputs: &super::value_transfer::ExactInputs,
) -> ExactOperand {
    let chain = provenance
        .iter()
        .map(|step| step.operation.as_str())
        .collect::<Vec<_>>()
        .join(" -> ");
    let input_literals = inputs
        .iter()
        .map(|(parameter, literal)| format!("{parameter} = {literal}"))
        .collect::<Vec<_>>()
        .join(", ");
    if provenance.is_empty() {
        return ExactOperand {
            value: value.render(),
            provenance: format!("{operand} = {} (exact literal)", value.render()),
        };
    }
    ExactOperand {
        value: value.render(),
        provenance: format!(
            "{operand} = {} via {chain} over {input_literals} (chain depth {})",
            value.render(),
            provenance.len()
        ),
    }
}

/// One exact comparison operand: the rendered value plus the #3295
/// provenance chain retained on the fact text (source inputs,
/// operation families, chain depth).
struct ExactOperand {
    value: String,
    provenance: String,
}

/// The initializer of a local binding whose live span (per the #3294
/// binding relation) covers the predicate line: the predicate must be
/// one of the binding's direct uses, so the initializer provably feeds
/// the compared operand. At most one declaration generation can hold
/// the predicate in its live span; the first that does wins.
fn live_local_initializer(
    owner: &FunctionSummary,
    operand: &str,
    predicate_line: usize,
) -> Option<String> {
    let masked = crate::analysis::language::mask_rust_comments_and_strings(&owner.body);
    // Detection runs on the masked line; the initializer is taken from
    // the raw line so string literals survive for evaluation (#3295).
    for (offset, (raw_line, masked_line)) in owner.body.lines().zip(masked.lines()).enumerate() {
        let absolute = owner.start_line + offset;
        let trimmed = masked_line.trim();
        // A trailing comment (`let x = …; // note`) survives on the
        // raw line: cut the raw statement at the masked line's first
        // semicolon, which string masking keeps honest.
        // Masking preserves byte offsets, so the masked semicolon's
        // byte index cuts the raw line directly.
        let raw_statement = masked_line
            .find(';')
            .map(|cut| &raw_line[..=cut])
            .unwrap_or(raw_line);
        if trimmed.starts_with("let ")
            && trimmed.contains(';')
            && let Some((_declared, _)) = crate::analysis::language::changed_let_binding(trimmed)
            && let Some((declared, initializer)) =
                crate::analysis::language::changed_let_binding(raw_statement.trim())
            && declared == operand
        {
            let initializer = initializer.to_string();
            let resolution = crate::analysis::probes::resolve_changed_binding_uses(
                operand,
                &initializer,
                &owner.body,
                owner.start_line,
                absolute,
            );
            if let crate::analysis::probes::BindingPredicateResolution::DirectUses(uses) =
                &resolution
                && uses
                    .iter()
                    .any(|use_site| use_site.predicate_line == predicate_line)
            {
                return Some(initializer);
            }
        }
    }
    None
}

#[allow(
    clippy::too_many_arguments,
    reason = "the shared evidence inputs flow through one authority"
)]
fn missing_discriminator_facts(
    probe: &Probe,
    owner_fn: Option<&FunctionSummary>,
    related_tests: &[&TestSummary],
    flow_sinks: &[FlowSinkFact],
    observed_values: &[ValueFact],
    helper_chain: Option<&super::helper_transfer::HelperChain>,
    index: &crate::analysis::rust_index::RustIndex,
    workspace_complete: bool,
) -> Vec<MissingDiscriminatorFact> {
    let mut missing = Vec::new();
    if matches!(probe.family, ProbeFamily::Predicate)
        && let Some(fact) = missing_boundary_discriminator(
            probe,
            owner_fn,
            related_tests,
            flow_sinks,
            helper_chain,
            index,
            workspace_complete,
        )
    {
        missing.push(fact);
    }
    if (matches!(probe.family, ProbeFamily::ErrorPath)
        || flow_sinks
            .iter()
            .any(|sink| sink.kind == FlowSinkKind::ErrorVariant))
        && let Some(fact) = missing_error_variant_discriminator(probe, related_tests, flow_sinks)
    {
        missing.push(fact);
    }
    if matches!(probe.family, ProbeFamily::FieldConstruction)
        && let Some(fact) = missing_field_value_discriminator(probe, related_tests, flow_sinks)
    {
        missing.push(fact);
    }
    if missing.is_empty()
        && observed_values
            .iter()
            .any(|fact| fact.value.contains(" == "))
    {
        return Vec::new();
    }
    missing
}

/// #1429: stable reason marker for a boundary discriminator whose
/// operand values the bounded evaluator cannot statically resolve.
/// The producer appends it to the fact reason; the finding builder
/// matches on it to withhold an unverifiable repair prescription.
/// Shared as a constant (like the value prefix in `domain`) so the
/// two sides cannot drift apart.
pub(in crate::analysis) const BOUNDARY_OPERAND_UNRESOLVED_MARKER: &str =
    "boundary operand value unresolved";

/// #1429: name the earliest unsupported producer edge when a boundary
/// operand is a computed local whose value the bounded evaluator
/// cannot resolve on any related-test row (e.g. `rfind(?)/len_utf8`
/// producers). Returns `None` for directly observed operands
/// (parameters, literals, or rows with exact values) so those keep
/// the honest missing listing and its satisfiable repair.
fn unresolved_local_operand_edge(
    operand: &str,
    resolved: &Option<ResolvedOperand>,
    exact_empty: bool,
    observed_empty: bool,
    call_values: &[Vec<ParameterValue>],
) -> Option<String> {
    if !exact_empty || !observed_empty {
        return None;
    }
    let ResolvedOperand::Local(_, initializer) = resolved.as_ref()? else {
        return None;
    };
    let first_row = call_values.first()?;
    let inputs: super::value_transfer::ExactInputs = first_row
        .iter()
        .map(|cell| (cell.parameter.clone(), cell.value.clone()))
        .collect();
    match super::value_transfer::evaluate_initializer(initializer, &inputs) {
        super::value_transfer::EvalOutcome::Unsupported { earliest_edge } => Some(format!(
            "{BOUNDARY_OPERAND_UNRESOLVED_MARKER}: `{operand}` flows through `{initializer}` (earliest unsupported edge: {earliest_edge})"
        )),
        _ => None,
    }
}

fn missing_boundary_discriminator(
    probe: &Probe,
    owner_fn: Option<&FunctionSummary>,
    related_tests: &[&TestSummary],
    flow_sinks: &[FlowSinkFact],
    helper_chain: Option<&super::helper_transfer::HelperChain>,
    index: &crate::analysis::rust_index::RustIndex,
    workspace_complete: bool,
) -> Option<MissingDiscriminatorFact> {
    let (left, right) = comparison_operands(&probe.expression)?;
    let owner = owner_fn?;
    let parameters = function_parameters(owner);
    let call_values = call_values_for_owner(owner, &parameters, related_tests, helper_chain);
    if call_values.is_empty() {
        return None;
    }
    let left_parameter = boundary_operand_parameter(owner, &parameters, &left);
    let right_parameter = boundary_operand_parameter(owner, &parameters, &right);

    // #3295 exact path: when either operand is a computed local, the
    // bounded evaluator resolves its value from the row's exact
    // inputs; the boundary is observed when both sides compare equal
    // under any row.
    let left_resolved = resolve_boundary_operand(
        owner,
        &left,
        probe.location.line,
        &parameters,
        index,
        workspace_complete,
    );
    let right_resolved = resolve_boundary_operand(
        owner,
        &right,
        probe.location.line,
        &parameters,
        index,
        workspace_complete,
    );
    let exact_rows: Vec<(Vec<ExactOperand>, Vec<ExactOperand>)> = call_values
        .iter()
        .map(|row| {
            let inputs: super::value_transfer::ExactInputs = row
                .iter()
                .map(|cell| (cell.parameter.clone(), cell.value.clone()))
                .collect();
            let lefts = left_resolved
                .as_ref()
                .and_then(|resolved| {
                    exact_operand_for_row(resolved, row, &inputs, index, workspace_complete)
                })
                .into_iter()
                .collect::<Vec<_>>();
            let rights = right_resolved
                .as_ref()
                .and_then(|resolved| {
                    exact_operand_for_row(resolved, row, &inputs, index, workspace_complete)
                })
                .or_else(|| {
                    literal_operand_value(&right).map(|value| ExactOperand {
                        provenance: format!("literal operand {right} = {value}"),
                        value,
                    })
                })
                .into_iter()
                .collect::<Vec<_>>();
            (lefts, rights)
        })
        .collect();
    // Exact operands compare by their canonical renderings directly:
    // the digit-oriented literal normalization (underscore/quote
    // stripping) would equate distinct strings (#3295 review).
    let exact_equality_observed = exact_rows.iter().any(|(lefts, rights)| {
        lefts.iter().any(|left_value| {
            rights
                .iter()
                .any(|right_value| right_value.value == left_value.value)
        })
    });

    let equality_observed = exact_equality_observed
        || left_parameter.as_deref().is_some_and(|left_parameter| {
            call_values.iter().any(|row| {
                let Some(left_value) = parameter_value(row, left_parameter) else {
                    return false;
                };
                let right_value = right_parameter
                    .as_deref()
                    .and_then(|parameter| parameter_value(row, parameter))
                    .map(|value| value.value)
                    .or_else(|| literal_operand_value(&right));
                right_value.as_deref().is_some_and(|value| {
                    comparable_value(value) == comparable_value(&left_value.value)
                })
            })
        });
    if equality_observed {
        return None;
    }

    let mut left_values = left_parameter
        .as_deref()
        .map(|parameter| observed_parameter_values(&call_values, parameter))
        .unwrap_or_default();
    // Exact evaluated lefts join the observed listing so the reason
    // names real values instead of `unknown` (#3295).
    let exact_lefts: Vec<String> = exact_rows
        .iter()
        .flat_map(|(lefts, _)| lefts.iter().map(|operand| operand.value.clone()))
        .filter(|value| !left_values.contains(value))
        .collect();
    left_values.extend(exact_lefts);
    left_values.sort();
    left_values.dedup();
    let right_parameter_values = right_parameter
        .as_deref()
        .and_then(|parameter| parameter_value_set(&call_values, parameter));
    let right_literal = literal_operand_value(&right);
    // #1429: an `unknown` listing for a computed local the evaluator
    // cannot resolve records analyzer inability, not test absence.
    let mut unresolved_notes = Vec::new();
    if let Some(note) = unresolved_local_operand_edge(
        &left,
        &left_resolved,
        exact_rows.iter().all(|(lefts, _)| lefts.is_empty()),
        left_values.is_empty(),
        &call_values,
    ) {
        unresolved_notes.push(note);
    }
    if let Some(note) = unresolved_local_operand_edge(
        &right,
        &right_resolved,
        exact_rows.iter().all(|(_, rights)| rights.is_empty()),
        right_parameter_values.is_none() && right_literal.is_none(),
        &call_values,
    ) {
        unresolved_notes.push(note);
    }
    let mut reason = if let Some(right_values) = right_parameter_values {
        format!(
            "No related test call uses {left} equal to {right}; observed {left} values: {}; observed {right} values: {}",
            list_or_unknown(&left_values),
            list_or_unknown(&right_values)
        )
    } else if let Some(right_value) = right_literal {
        format!(
            "No related test call uses {left} equal to {right}; observed {left} values: {}; target {right} value: {right_value}",
            list_or_unknown(&left_values)
        )
    } else {
        format!(
            "No related test call uses {left} equal to {right}; observed {left} values: {}",
            list_or_unknown(&left_values)
        )
    };
    for note in &unresolved_notes {
        reason.push_str("; ");
        reason.push_str(note);
    }

    Some(MissingDiscriminatorFact {
        value: format!("{left} == {right}"),
        reason,
        flow_sink: first_visible_flow_sink(flow_sinks).cloned(),
    })
}

fn missing_error_variant_discriminator(
    probe: &Probe,
    related_tests: &[&TestSummary],
    flow_sinks: &[FlowSinkFact],
) -> Option<MissingDiscriminatorFact> {
    let variant = exact_error_variant(&probe.expression).or_else(|| {
        flow_sinks
            .iter()
            .find_map(|sink| exact_error_variant(&sink.text))
    })?;
    let exact_assertion_found = related_tests.iter().any(|test| {
        test.assertions.iter().any(|assertion| {
            assertion.kind == OracleKind::ExactErrorVariant && assertion.text.contains(&variant)
        })
    });
    if exact_assertion_found {
        return None;
    }

    Some(MissingDiscriminatorFact {
        value: variant.clone(),
        reason: format!("No exact error variant assertion for {variant}"),
        flow_sink: flow_sinks
            .iter()
            .find(|sink| sink.kind == FlowSinkKind::ErrorVariant)
            .or_else(|| first_visible_flow_sink(flow_sinks))
            .cloned(),
    })
}

/// Produce a missing-discriminator fact for a `FieldConstruction` seam whose
/// `RequiredDiscriminator::FieldValue { field }` has no matching producer-owned
/// discriminator in the test evidence.
///
/// Mirrors the Predicate (`missing_boundary_discriminator`) and ErrorPath
/// (`missing_error_variant_discriminator`) arms. The probe expression is the
/// field value the readiness authority (`repair_route.rs`) compares via
/// `exact_key("field_value", field)`. When a related test already asserts the
/// field via an ExactValue / WholeObjectEquality / RelationalCheck / Snapshot
/// oracle, the discriminator is NOT missing and this returns `None`.
fn missing_field_value_discriminator(
    probe: &Probe,
    related_tests: &[&TestSummary],
    flow_sinks: &[FlowSinkFact],
) -> Option<MissingDiscriminatorFact> {
    // Only meaningful when there is a StructField flow sink.
    let has_struct_field_sink = flow_sinks
        .iter()
        .any(|sink| sink.kind == FlowSinkKind::StructField);
    if !has_struct_field_sink {
        return None;
    }

    // If any related test observes this field via an accepted oracle kind
    // (ExactValue, WholeObjectEquality, RelationalCheck, Snapshot), the
    // discriminator is already covered — do not emit a missing fact.
    //
    // The match uses word-boundary semantics via `contains_as_whole_word` to
    // avoid token coincidence (e.g. `id` matching inside `provider`), the
    // recurring false-observation family. See reveal.rs:413 for the same guard.
    let field_already_observed = related_tests.iter().any(|test| {
        test.assertions.iter().any(|assertion| {
            matches!(
                assertion.kind,
                OracleKind::ExactValue
                    | OracleKind::WholeObjectEquality
                    | OracleKind::RelationalCheck
                    | OracleKind::Snapshot
            ) && super::reveal::contains_as_whole_word(&assertion.text, &probe.expression)
        })
    });
    if field_already_observed {
        return None;
    }

    Some(MissingDiscriminatorFact {
        value: probe.expression.clone(),
        reason: format!(
            "No field-value assertion observes the constructed field: {}",
            probe.expression
        ),
        flow_sink: flow_sinks
            .iter()
            .find(|sink| sink.kind == FlowSinkKind::StructField)
            .cloned(),
    })
}

fn owner_call_parameter_values(
    related_tests: &[&TestSummary],
    owner_name: &str,
    parameters: &[String],
) -> Vec<Vec<ParameterValue>> {
    let mut rows = Vec::new();
    if owner_name.is_empty() || parameters.is_empty() {
        return rows;
    }
    for test in related_tests {
        for call in &test.calls {
            if call.name != owner_name {
                continue;
            }
            let Some(arguments) = call_arguments(&call.text, &call.name) else {
                continue;
            };
            let row = arguments
                .iter()
                .enumerate()
                .filter_map(|(idx, argument)| {
                    let parameter = parameters.get(idx)?;
                    let value = scalar_values(argument).into_iter().next()?;
                    Some(ParameterValue {
                        parameter: parameter.clone(),
                        value,
                        line: call.line,
                        text: call.text.clone(),
                    })
                })
                .collect::<Vec<_>>();
            if !row.is_empty() {
                rows.push(row);
            }
        }
    }
    rows
}

/// #3296: the owner's exact input rows. Direct test-call rows first
/// (the pre-existing source); when the owner has none and the bounded
/// helper chain resolves, the entry function's rows bind down the
/// chain to the owner's parameters. A chain that stops, or a computed
/// argument at any hop, yields no rows — the stop edge stays with the
/// chain the classifier already recorded.
fn call_values_for_owner(
    owner: &FunctionSummary,
    parameters: &[String],
    related_tests: &[&TestSummary],
    helper_chain: Option<&super::helper_transfer::HelperChain>,
) -> Vec<Vec<ParameterValue>> {
    let parameters = if parameters.is_empty() {
        function_parameters(owner)
    } else {
        parameters.to_vec()
    };
    let direct = owner_call_parameter_values(related_tests, &owner.name, &parameters);
    if !direct.is_empty() {
        return direct;
    }
    let Some(chain) = helper_chain else {
        return direct;
    };
    helper_transferred_rows(&parameters, chain, related_tests)
}

/// Bind the entry function's direct test rows down the resolved chain
/// to the owner's parameters. Each hop's call-site arguments bind
/// positionally: a literal binds directly, the caller's own parameter
/// resolves through that caller's row, and anything else stops the row.
fn helper_transferred_rows(
    owner_parameters: &[String],
    chain: &super::helper_transfer::HelperChain,
    related_tests: &[&TestSummary],
) -> Vec<Vec<ParameterValue>> {
    let Some(entry) = chain.hops.last() else {
        return Vec::new();
    };
    let entry_parameters = function_parameters(&entry.caller);
    let mut rows =
        owner_call_parameter_values(related_tests, &entry.caller.name, &entry_parameters);
    if rows.is_empty() {
        return Vec::new();
    }
    // hops[0] is the owner's direct caller; hops[len-1] is the entry a
    // test can call. Bind from the entry downward.
    for step in (0..chain.hops.len()).rev() {
        let hop = &chain.hops[step];
        let target_parameters: Vec<String> = if step == 0 {
            owner_parameters.to_vec()
        } else {
            function_parameters(&chain.hops[step - 1].caller)
        };
        // The hop's call site lives in the hop's own caller: its
        // arguments reference THAT function's parameters (#3296 review
        // M3 — the previous off-by-one silently dropped every row when
        // parameter names differed between hops).
        let caller_parameters = function_parameters(&hop.caller);
        let mut bound_rows = Vec::new();
        for row in &rows {
            let mut bound = Vec::new();
            for (index, argument) in hop.arguments.iter().enumerate() {
                let Some(parameter) = target_parameters.get(index) else {
                    return Vec::new();
                };
                let Some(value) = bind_helper_argument(argument, &caller_parameters, row) else {
                    return Vec::new();
                };
                bound.push(ParameterValue {
                    parameter: parameter.clone(),
                    value,
                    line: row.first().map(|cell| cell.line).unwrap_or_default(),
                    text: row
                        .first()
                        .map(|cell| cell.text.clone())
                        .unwrap_or_default(),
                });
            }
            bound_rows.push(bound);
        }
        rows = bound_rows;
    }
    rows
}

/// One bound argument: a literal, or the caller's own parameter
/// resolved through the caller's row. A computed argument stops the
/// transfer for that chain (#3296: named edge, no guessed value).
fn bind_helper_argument(
    argument: &str,
    caller_parameters: &[String],
    row: &[ParameterValue],
) -> Option<String> {
    // #3296 review B2: only a strict whole-token literal binds; the
    // substring scanner must never fabricate a value from an
    // identifier like `a2`.
    if let Some(literal) = super::helper_transfer::strict_literal(argument) {
        return Some(literal);
    }
    let trimmed = argument.trim();
    caller_parameters
        .iter()
        .find(|parameter| parameter.as_str() == trimmed)
        .and_then(|parameter| {
            row.iter()
                .find(|cell| cell.parameter == *parameter)
                .map(|cell| cell.value.clone())
        })
}

fn parameter_value(row: &[ParameterValue], parameter: &str) -> Option<ParameterValue> {
    row.iter()
        .find(|value| value.parameter == parameter)
        .cloned()
}

fn parameter_value_set(rows: &[Vec<ParameterValue>], parameter: &str) -> Option<Vec<String>> {
    let mut values = observed_parameter_values(rows, parameter);
    if values.is_empty() {
        None
    } else {
        values.sort();
        values.dedup();
        Some(values)
    }
}

fn observed_parameter_values(rows: &[Vec<ParameterValue>], parameter: &str) -> Vec<String> {
    let mut values = rows
        .iter()
        .flat_map(|row| {
            row.iter()
                .filter(|value| value.parameter == parameter)
                .map(|value| value.value.clone())
        })
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

pub(crate) fn function_parameters(function: &FunctionSummary) -> Vec<String> {
    let signature = function
        .body
        .lines()
        .next()
        .unwrap_or(function.body.as_str());
    let Some(arguments) = delimited_contents_after(signature, '(') else {
        return Vec::new();
    };
    split_top_level_args(&arguments)
        .into_iter()
        .filter_map(|argument| {
            argument
                .split_once(':')
                .map(|(name, _)| name.trim().to_string())
        })
        .filter(|name| !name.is_empty() && name != "self" && name != "&self" && name != "mut self")
        .collect()
}

fn boundary_operand_parameter(
    function: &FunctionSummary,
    parameters: &[String],
    operand: &str,
) -> Option<String> {
    parameters
        .iter()
        .find(|parameter| parameter.as_str() == operand)
        .cloned()
        .or_else(|| boundary_local_operand_parameter(function, parameters, operand))
}

fn boundary_local_operand_parameter(
    function: &FunctionSummary,
    parameters: &[String],
    operand: &str,
) -> Option<String> {
    if operand.is_empty() {
        return None;
    }
    for parameter in parameters {
        if body_contains_wrapped_local_alias(&function.body, "Some", operand, parameter)
            || body_contains_wrapped_local_alias(&function.body, "Ok", operand, parameter)
            || body_contains_direct_local_alias(&function.body, operand, parameter)
        {
            return Some(parameter.clone());
        }
    }
    None
}

fn body_contains_wrapped_local_alias(
    body: &str,
    wrapper: &str,
    operand: &str,
    parameter: &str,
) -> bool {
    body.lines().any(|line| {
        let line = code_line_before_comment(line);
        let prefix = format!("if let {wrapper}({operand}) = ");
        line.strip_prefix(&prefix)
            .is_some_and(|rest| starts_with_identifier_token(rest, parameter))
    }) || (body_contains_match_parameter(body, parameter)
        && body_contains_wrapper_pattern(body, wrapper, operand))
}

fn body_contains_match_parameter(body: &str, parameter: &str) -> bool {
    body.lines().any(|line| {
        let line = code_line_before_comment(line);
        if is_comment_line(line) {
            return false;
        }
        line.find("match ")
            .map(|index| &line[index + "match ".len()..])
            .is_some_and(|rest| starts_with_identifier_token(rest, parameter))
    })
}

fn body_contains_wrapper_pattern(body: &str, wrapper: &str, operand: &str) -> bool {
    let pattern = format!("{wrapper}({operand})");
    body.lines().any(|line| {
        let line = code_line_before_comment(line);
        !is_comment_line(line) && line.contains(&pattern)
    })
}

fn code_line_before_comment(line: &str) -> &str {
    let line = line.trim();
    let line = line.split_once("//").map_or(line, |(code, _comment)| code);
    line.split_once("/*")
        .map_or(line, |(code, _comment)| code)
        .trim()
}

fn is_comment_line(line: &str) -> bool {
    line.starts_with("//") || line.starts_with("/*") || line.starts_with('*')
}

fn starts_with_identifier_token(text: &str, token: &str) -> bool {
    let text = text.trim_start();
    let end = text
        .find(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .unwrap_or(text.len());
    end > 0 && &text[..end] == token
}

fn body_contains_direct_local_alias(body: &str, operand: &str, parameter: &str) -> bool {
    body.lines().any(|line| {
        let line = line.trim().trim_end_matches(';').trim();
        let Some(binding) = line.strip_prefix("let ") else {
            return false;
        };
        let Some((left, right)) = binding.split_once('=') else {
            return false;
        };
        let local_name = left.split_once(':').map(|(name, _)| name).unwrap_or(left);
        local_name.trim() == operand && right.trim() == parameter
    })
}

fn comparison_operands(expression: &str) -> Option<(String, String)> {
    for operator in [">=", "<=", "==", "!=", ">", "<"] {
        if let Some((left, right)) = expression.split_once(operator) {
            let left = clean_operand(left);
            let right = clean_operand(right);
            if !left.is_empty() && !right.is_empty() {
                return Some((left, right));
            }
        }
    }
    None
}

fn clean_operand(operand: &str) -> String {
    let cleaned = operand
        .trim()
        .trim_start_matches("if ")
        .trim_end_matches('{')
        .trim_end_matches(';')
        .trim();
    let cleaned = cleaned
        .split_once('{')
        .map(|(before, _)| before.trim())
        .unwrap_or(cleaned);
    cleaned.to_string()
}

fn literal_operand_value(operand: &str) -> Option<String> {
    scalar_values(operand).into_iter().next()
}

fn comparable_value(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .chars()
        .filter(|ch| *ch != '_')
        .collect()
}

fn first_visible_flow_sink(flow_sinks: &[FlowSinkFact]) -> Option<&FlowSinkFact> {
    flow_sinks
        .iter()
        .find(|sink| sink.kind != FlowSinkKind::Unknown)
}

fn list_or_unknown(values: &[String]) -> String {
    if values.is_empty() {
        "unknown".to_string()
    } else {
        values.join(", ")
    }
}

pub(in crate::analysis) fn has_observed_boundary_equality(activation: &ActivationEvidence) -> bool {
    activation
        .observed_values
        .iter()
        .any(|fact| fact.value.contains(" == "))
}

fn sort_value_facts(facts: &mut Vec<ValueFact>) {
    facts.sort_by(|left, right| {
        left.line
            .cmp(&right.line)
            .then(left.context.as_str().cmp(right.context.as_str()))
            .then(left.value.cmp(&right.value))
            .then(left.text.cmp(&right.text))
    });
    facts.dedup_by(|left, right| {
        left.line == right.line
            && left.text == right.text
            && left.value == right.value
            && left.context == right.context
    });
}

fn call_arguments(text: &str, name: &str) -> Option<Vec<String>> {
    let needle = format!("{name}(");
    let start = text.find(&needle)? + name.len();
    let contents = delimited_contents_at(text, start)?;
    Some(split_top_level_args(&contents))
}

fn macro_arguments(text: &str) -> Option<Vec<String>> {
    let start = text.find("!(")? + 1;
    let contents = delimited_contents_at(text, start)?;
    Some(split_top_level_args(&contents))
}

fn delimited_contents_after(text: &str, delimiter: char) -> Option<String> {
    let start = text.find(delimiter)?;
    delimited_contents_at(text, start)
}

fn split_top_level_args(text: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut start = 0usize;
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for (idx, ch) in text.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                if let Some(arg) = text.get(start..idx).map(str::trim)
                    && !arg.is_empty()
                {
                    args.push(arg.to_string());
                }
                start = idx + 1;
            }
            _ => {}
        }
    }
    if let Some(arg) = text.get(start..).map(str::trim)
        && !arg.is_empty()
    {
        args.push(arg.to_string());
    }
    args
}

fn scalar_values(text: &str) -> Vec<String> {
    let mut values = Vec::new();
    let chars = text.char_indices().collect::<Vec<_>>();
    let mut idx = 0usize;
    while idx < chars.len() {
        let (byte_idx, ch) = chars[idx];
        if ch == '"' {
            let mut end = byte_idx + ch.len_utf8();
            let mut cursor = idx + 1;
            let mut escaped = false;
            while cursor < chars.len() {
                let (next_byte, next_ch) = chars[cursor];
                end = next_byte + next_ch.len_utf8();
                if escaped {
                    escaped = false;
                } else if next_ch == '\\' {
                    escaped = true;
                } else if next_ch == '"' {
                    break;
                }
                cursor += 1;
            }
            if let Some(value) = text.get(byte_idx..end) {
                values.push(value.to_string());
            }
            idx = cursor.saturating_add(1);
            continue;
        }
        if ch == '\'' {
            // A char literal `'x'` / `'\n'` / `'\''`; a lifetime
            // (`'a`, never closed by a quote on its own) is not a
            // value. #3295: char arguments are exact inputs.
            let closing = if chars
                .get(idx + 1)
                .is_some_and(|(_, next_ch)| *next_ch == '\\')
            {
                chars.get(idx + 3)
            } else {
                chars.get(idx + 2)
            };
            if let Some((end_byte, end_ch)) = closing
                && *end_ch == '\''
                && let Some(value) = text.get(byte_idx..end_byte + end_ch.len_utf8())
            {
                values.push(value.to_string());
                idx = chars
                    .iter()
                    .position(|(scan_byte, _)| *scan_byte == *end_byte)
                    .map_or(idx + 1, |position| position.saturating_add(1));
                continue;
            }
        }
        // Boolean literals are exact inputs too (#3295). Both edges
        // must be identifier boundaries: `is_true`/`true_flag` are
        // identifiers, not booleans (#3295 review).
        for literal in ["true", "false"] {
            if text[byte_idx..].starts_with(literal) {
                let before = text[..byte_idx].chars().next_back();
                let after = text[byte_idx + literal.len()..].chars().next();
                let before_ok = !before
                    .is_some_and(|prev_ch: char| prev_ch.is_ascii_alphanumeric() || prev_ch == '_');
                let after_ok = !after
                    .is_some_and(|next_ch: char| next_ch.is_ascii_alphanumeric() || next_ch == '_');
                if before_ok && after_ok {
                    values.push(literal.to_string());
                    idx += 1;
                    break;
                }
            }
        }
        if ch.is_ascii_digit()
            || (ch == '-'
                && chars
                    .get(idx + 1)
                    .is_some_and(|(_, next_ch)| next_ch.is_ascii_digit()))
        {
            let mut end = byte_idx + ch.len_utf8();
            let mut cursor = idx + 1;
            while cursor < chars.len() {
                let (next_byte, next_ch) = chars[cursor];
                if next_ch.is_ascii_digit() || next_ch == '_' {
                    end = next_byte + next_ch.len_utf8();
                    cursor += 1;
                } else {
                    break;
                }
            }
            if let Some(value) = text.get(byte_idx..end) {
                values.push(value.to_string());
            }
            idx = cursor;
            continue;
        }
        idx += 1;
    }
    values.sort();
    values.dedup();
    values
}

fn looks_like_table_row(line: &str) -> bool {
    (line.starts_with('(') || line.starts_with('[') || line.contains("[(")) && line.contains(',')
}

fn looks_like_builder_method(line: &str) -> bool {
    line.contains('.')
        && line.contains('(')
        && (line.contains("builder")
            || line.contains("with_")
            || line.contains(".amount(")
            || line.contains(".token(")
            || line.contains(".threshold("))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::facts::FunctionSourceRole;
    use crate::analysis::rust_index::{CallFact, OracleFact};
    use std::path::PathBuf;

    #[test]
    fn activation_evidence_records_observed_boundary_equality() {
        let owner = function(
            "pub fn score(amount: i32, threshold: i32) -> bool {\n    amount >= threshold\n}",
        );
        let test = test_with_call("score_uses_boundary", "score(100, 100);");
        let probe = probe(ProbeFamily::Predicate, "amount >= threshold");

        let activation = activation_evidence(
            &probe,
            Some(&owner),
            &[&test],
            &[],
            None,
            &crate::analysis::rust_index::RustIndex::default(),
            false,
        );

        assert!(has_observed_boundary_equality(&activation));
        assert!(activation.missing_discriminators.is_empty());
        assert!(activation.observed_values.iter().any(|fact| {
            fact.context == ValueContext::FunctionArgument && fact.value == "amount == threshold"
        }));
    }

    #[test]
    fn activation_evidence_resolves_direct_local_boundary_operand_alias() {
        let owner = function(
            "pub fn score(raw_amount: i32, threshold: i32) -> bool {\n    let amount = raw_amount;\n    amount >= threshold\n}",
        );
        let test = test_with_call("score_uses_boundary", "score(100, 100);");
        let probe = probe(ProbeFamily::Predicate, "amount >= threshold");

        let activation = activation_evidence(
            &probe,
            Some(&owner),
            &[&test],
            &[],
            None,
            &crate::analysis::rust_index::RustIndex::default(),
            false,
        );

        assert!(has_observed_boundary_equality(&activation));
        assert!(activation.missing_discriminators.is_empty());
        assert!(activation.observed_values.iter().any(|fact| {
            fact.context == ValueContext::FunctionArgument && fact.value == "amount == threshold"
        }));
    }

    // #3295 review F2: an identifier argument like `is_true` never
    // yields a boolean exact input.
    #[test]
    fn boolean_extraction_requires_identifier_boundaries() {
        assert!(scalar_values("check(is_true)").is_empty());
        assert!(scalar_values("run(x_false)").is_empty());
        assert_eq!(
            scalar_values("f(true_flag)"),
            Vec::<String>::new(),
            "trailing identifier chars reject the token"
        );
        assert_eq!(scalar_values("check(true)"), vec!["true".to_string()]);
        assert_eq!(scalar_values("check(Some(true))"), vec!["true".to_string()]);
    }

    // #3295 review N4: a local that re-binds a parameter name wins over
    // the raw call argument.
    #[test]
    fn shadowing_local_wins_over_the_parameter_argument() -> Result<(), String> {
        let owner = function(
            "pub fn split_after(input: &str) -> bool {
    let input = input.strip_prefix(\"x\").map_or(\"none\", |s| s);
    input == \"y\"
}",
        );
        let test = test_with_call("boundary", "score(\"xy\");");
        let mut probe = probe(ProbeFamily::Predicate, "input == \"y\"");
        probe.location = SourceLocation::new("src/lib.rs", 3, 1);

        let activation = activation_evidence(
            &probe,
            Some(&owner),
            &[&test],
            &[],
            None,
            &crate::analysis::rust_index::RustIndex::default(),
            false,
        );

        // strip_prefix("x") over "xy" yields exactly "y": the boundary
        // is observed through the shadowing local, not the raw "xy".
        assert!(
            has_observed_boundary_equality(&activation),
            "observed: {:?}",
            activation.observed_values
        );
        let Some(fact) = activation
            .observed_values
            .iter()
            .find(|fact| fact.value == "input == \"y\"")
        else {
            return Err("boundary fact missing".to_string());
        };
        assert!(
            fact.text.contains("strip_prefix -> map_or"),
            "provenance names the evaluated chain: {}",
            fact.text
        );
        Ok(())
    }

    // #3295: computed local operands resolve through the bounded
    // evaluator over the exact test inputs, so the #3215 equality
    // boundary is observed instead of `unknown`.
    #[test]
    fn activation_evidence_resolves_computed_local_boundary_operands() {
        let owner = FunctionSummary {
            id: SymbolId("src/lib.rs::split_after".to_string()),
            name: "split_after".to_string(),
            file: PathBuf::from("src/lib.rs"),
            start_line: 1,
            end_line: 9,
            body: "pub fn split_after(input: &str, delim: char) -> &str {\n    let end = input.rfind(delim).map_or(1, |idx| idx);\n    let start = delim.len_utf8();\n    if end == start {\n        &input[..end]\n    } else {\n        input\n    }\n}".to_string(),
            calls: Vec::new(),
            returns: Vec::new(),
            literals: Vec::new(),
            source_role: FunctionSourceRole::Production,
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        };
        let test = TestSummary {
            name: "absent_delimiter_boundary".to_string(),
            file: PathBuf::from("tests/split.rs"),
            start_line: 4,
            end_line: 6,
            body: "split_after(\"ab\", 'x');".to_string(),
            calls: vec![CallFact {
                name: "split_after".to_string(),
                line: 5,
                text: "split_after(\"ab\", 'x');".to_string(),
            }],
            assertions: Vec::new(),
            literals: Vec::new(),
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib.rs:predicate:eval".to_string()),
            location: SourceLocation::new("src/lib.rs", 4, 1),
            owner: Some(SymbolId("src/lib.rs::split_after".to_string())),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: None,
            after: Some("if end == start {".to_string()),
            expression: "if end == start {".to_string(),
            expected_sinks: Vec::new(),
            required_oracles: Vec::new(),
        };

        let activation = activation_evidence(
            &probe,
            Some(&owner),
            &[&test],
            &[],
            None,
            &crate::analysis::rust_index::RustIndex::default(),
            false,
        );

        assert!(
            has_observed_boundary_equality(&activation),
            "the exact inputs end=1, start=1 must observe the boundary: {:?}",
            activation.observed_values
        );
        assert!(activation.missing_discriminators.is_empty());
        assert!(activation.observed_values.iter().any(|fact| {
            fact.context == ValueContext::FunctionArgument && fact.value == "end == start"
        }));
    }

    // #1429: the historical `rfind(?)/len_utf8` equality shape. The
    // equality test is present, but the bounded evaluator cannot
    // resolve the computed locals, so the missing fact must carry the
    // unresolved-operand marker naming the earliest unsupported edge
    // instead of a plain absence claim.
    #[test]
    fn activation_evidence_marks_unresolved_boundary_operand() {
        let owner = FunctionSummary {
            id: SymbolId("src/lib.rs::quote_body".to_string()),
            name: "quote_body".to_string(),
            file: PathBuf::from("src/lib.rs"),
            start_line: 1,
            end_line: 11,
            body: "pub fn quote_body(rest: &str, open: char, close: char) -> Option<&str> {\n    let start = open.len_utf8();\n    let end = rest.rfind(close)?;\n    if end == start {\n        Some(\"\")\n    } else if end > start {\n        Some(&rest[start..end])\n    } else {\n        None\n    }\n}".to_string(),
            calls: Vec::new(),
            returns: Vec::new(),
            literals: Vec::new(),
            source_role: FunctionSourceRole::Production,
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        };
        let test = TestSummary {
            name: "empty_body_equality_case".to_string(),
            file: PathBuf::from("tests/q.rs"),
            start_line: 4,
            end_line: 6,
            body: "assert_eq!(quote_body(\"[]\", '[', ']'), Some(\"\"));".to_string(),
            calls: vec![CallFact {
                name: "quote_body".to_string(),
                line: 5,
                text: "quote_body(\"[]\", '[', ']')".to_string(),
            }],
            assertions: Vec::new(),
            literals: Vec::new(),
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib.rs:predicate:unresolved".to_string()),
            location: SourceLocation::new("src/lib.rs", 4, 1),
            owner: Some(SymbolId("src/lib.rs::quote_body".to_string())),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: None,
            after: Some("if end == start {".to_string()),
            expression: "if end == start {".to_string(),
            expected_sinks: Vec::new(),
            required_oracles: Vec::new(),
        };

        let activation = activation_evidence(
            &probe,
            Some(&owner),
            &[&test],
            &[],
            None,
            &crate::analysis::rust_index::RustIndex::default(),
            false,
        );

        let reason = activation
            .missing_discriminators
            .iter()
            .find(|fact| fact.value == "end == start")
            .map(|fact| fact.reason.as_str())
            .unwrap_or("");
        assert!(
            !reason.is_empty(),
            "expected an unconfirmed end == start discriminator, got {:?}",
            activation.missing_discriminators
        );
        assert!(
            reason.contains(BOUNDARY_OPERAND_UNRESOLVED_MARKER),
            "reason must carry the unresolved marker: {reason}"
        );
        assert!(
            reason.contains("rfind"),
            "reason must name the earliest unresolved producer edge: {reason}"
        );
    }

    // #1429 paired control: a predicate over direct parameters with a
    // genuinely missing equality row keeps the plain absence claim
    // (no unresolved marker) so its bounded repair stays prescribable.
    #[test]
    fn activation_evidence_keeps_plain_claim_for_parameter_boundary() {
        let owner = FunctionSummary {
            id: SymbolId("src/lib.rs::discounted_total".to_string()),
            name: "discounted_total".to_string(),
            file: PathBuf::from("src/lib.rs"),
            start_line: 1,
            end_line: 7,
            body: "pub fn discounted_total(amount: i32, discount_threshold: i32) -> i32 {\n    if amount >= discount_threshold {\n        amount - 10\n    } else {\n        amount\n    }\n}".to_string(),
            calls: Vec::new(),
            returns: Vec::new(),
            literals: Vec::new(),
            source_role: FunctionSourceRole::Production,
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        };
        let above = TestSummary {
            name: "above_threshold".to_string(),
            file: PathBuf::from("tests/d.rs"),
            start_line: 4,
            end_line: 6,
            body: "assert_eq!(discounted_total(150, 100), 140);".to_string(),
            calls: vec![CallFact {
                name: "discounted_total".to_string(),
                line: 5,
                text: "discounted_total(150, 100)".to_string(),
            }],
            assertions: Vec::new(),
            literals: Vec::new(),
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        };
        let below = TestSummary {
            name: "below_threshold".to_string(),
            file: PathBuf::from("tests/d.rs"),
            start_line: 8,
            end_line: 10,
            body: "assert_eq!(discounted_total(50, 100), 50);".to_string(),
            calls: vec![CallFact {
                name: "discounted_total".to_string(),
                line: 9,
                text: "discounted_total(50, 100)".to_string(),
            }],
            assertions: Vec::new(),
            literals: Vec::new(),
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        };
        let probe = Probe {
            id: ProbeId("probe:src_lib.rs:predicate:plain".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 1),
            owner: Some(SymbolId("src/lib.rs::discounted_total".to_string())),
            family: ProbeFamily::Predicate,
            delta: DeltaKind::Control,
            before: None,
            after: Some("if amount >= discount_threshold {".to_string()),
            expression: "if amount >= discount_threshold {".to_string(),
            expected_sinks: Vec::new(),
            required_oracles: Vec::new(),
        };

        let activation = activation_evidence(
            &probe,
            Some(&owner),
            &[&above, &below],
            &[],
            None,
            &crate::analysis::rust_index::RustIndex::default(),
            false,
        );

        let reason = activation
            .missing_discriminators
            .iter()
            .find(|fact| fact.value == "amount == discount_threshold")
            .map(|fact| fact.reason.as_str())
            .unwrap_or("");
        assert!(
            !reason.is_empty(),
            "expected a genuinely missing equality discriminator, got {:?}",
            activation.missing_discriminators
        );
        assert!(
            !reason.contains(BOUNDARY_OPERAND_UNRESOLVED_MARKER),
            "parameter operands are directly observed; no unresolved marker: {reason}"
        );
    }

    #[test]
    fn same_file_method_chain_owner_call_tracing_covers_activation_alias_helpers() {
        let owner = function(
            "pub fn score(raw_amount: Option<i32>, threshold: i32) -> bool {\n    if let Some(amount) = raw_amount { amount >= threshold } else { false }\n}",
        );
        let parameters = function_parameters(&owner);

        assert_eq!(
            boundary_local_operand_parameter(&owner, &parameters, "amount"),
            Some("raw_amount".to_string())
        );
        assert_eq!(
            boundary_local_operand_parameter(&owner, &parameters, ""),
            None
        );
        assert!(body_contains_wrapped_local_alias(
            &owner.body,
            "Some",
            "amount",
            "raw_amount"
        ));
        let match_body =
            "match raw_amount {\n    Some(amount) => amount >= threshold,\n    None => false,\n}";
        assert!(body_contains_wrapped_local_alias(
            match_body,
            "Some",
            "amount",
            "raw_amount"
        ));
        assert!(body_contains_match_parameter(match_body, "raw_amount"));
        assert!(body_contains_wrapper_pattern(match_body, "Some", "amount"));
        assert!(body_contains_direct_local_alias(
            "let amount = raw_amount;\namount >= threshold",
            "amount",
            "raw_amount"
        ));
        assert!(!body_contains_match_parameter(
            "// match raw_amount { Some(amount) => amount >= threshold }",
            "raw_amount"
        ));
        assert!(!body_contains_wrapper_pattern(
            "// Some(amount) => amount >= threshold",
            "Some",
            "amount"
        ));
        assert!(!body_contains_direct_local_alias(
            "let amount = raw_amount_extra;",
            "amount",
            "raw_amount"
        ));
        assert!(starts_with_identifier_token(
            " raw_amount.required_discriminator()",
            "raw_amount"
        ));
        assert!(!starts_with_identifier_token(
            " raw_amount_extra.required_discriminator()",
            "raw_amount"
        ));
    }

    #[test]
    fn activation_evidence_uses_exact_if_let_parameter_name_for_boundary_operand_alias() {
        let owner = function(
            "pub fn score(raw_amount: Option<i32>, raw_amount_extra: Option<i32>, threshold: i32) -> bool {\n    if let Some(amount) = raw_amount_extra { amount >= threshold } else { false }\n}",
        );
        let test = test_with_call("score_uses_boundary", "score(Some(100), Some(101), 100);");
        let probe = probe(ProbeFamily::Predicate, "amount >= threshold");

        let activation = activation_evidence(
            &probe,
            Some(&owner),
            &[&test],
            &[],
            None,
            &crate::analysis::rust_index::RustIndex::default(),
            false,
        );

        assert!(!has_observed_boundary_equality(&activation));
        assert_eq!(activation.missing_discriminators.len(), 1);
        assert!(
            activation.missing_discriminators[0]
                .reason
                .contains("observed amount values: 101"),
            "prefix parameter matches must not make raw_amount look like amount; got {:?}",
            activation.missing_discriminators
        );
    }

    #[test]
    fn activation_evidence_ignores_commented_match_boundary_operand_alias() {
        let owner = function(
            "pub fn score(raw_amount: Option<i32>, threshold: i32) -> bool {\n    // match raw_amount { Some(amount) => amount >= threshold, _ => false }\n    let amount = 1;\n    amount >= threshold\n}",
        );
        let test = test_with_call("score_uses_boundary", "score(Some(100), 100);");
        let probe = probe(ProbeFamily::Predicate, "amount >= threshold");

        let activation = activation_evidence(
            &probe,
            Some(&owner),
            &[&test],
            &[],
            None,
            &crate::analysis::rust_index::RustIndex::default(),
            false,
        );

        assert!(!has_observed_boundary_equality(&activation));
        assert_eq!(activation.missing_discriminators.len(), 1);
        assert!(
            activation.missing_discriminators[0]
                .reason
                .contains("observed amount values: unknown"),
            "commented match aliases must not resolve boundary operands; got {:?}",
            activation.missing_discriminators
        );
    }

    #[test]
    fn activation_evidence_ignores_inline_commented_match_boundary_operand_alias() {
        let owner = function(
            "pub fn score(raw_amount: Option<i32>, threshold: i32) -> bool {\n    let _note = 0; // match raw_amount { Some(amount) => amount >= threshold, _ => false }\n    let amount = 1;\n    amount >= threshold\n}",
        );
        let test = test_with_call("score_uses_boundary", "score(Some(100), 100);");
        let probe = probe(ProbeFamily::Predicate, "amount >= threshold");

        let activation = activation_evidence(
            &probe,
            Some(&owner),
            &[&test],
            &[],
            None,
            &crate::analysis::rust_index::RustIndex::default(),
            false,
        );

        assert!(!has_observed_boundary_equality(&activation));
        assert_eq!(activation.missing_discriminators.len(), 1);
        assert!(
            activation.missing_discriminators[0]
                .reason
                .contains("observed amount values: unknown"),
            "inline commented match aliases must not resolve boundary operands; got {:?}",
            activation.missing_discriminators
        );
    }

    #[test]
    fn activation_evidence_ignores_commented_match_wrapper_pattern() {
        let owner = function(
            "pub fn score(raw_amount: Option<i32>, threshold: i32) -> bool {\n    let _seen = match raw_amount { _ => false };\n    // Some(amount)\n    let amount = 1;\n    amount >= threshold\n}",
        );
        let test = test_with_call("score_uses_boundary", "score(Some(100), 100);");
        let probe = probe(ProbeFamily::Predicate, "amount >= threshold");

        let activation = activation_evidence(
            &probe,
            Some(&owner),
            &[&test],
            &[],
            None,
            &crate::analysis::rust_index::RustIndex::default(),
            false,
        );

        assert!(!has_observed_boundary_equality(&activation));
        assert_eq!(activation.missing_discriminators.len(), 1);
        assert!(
            activation.missing_discriminators[0]
                .reason
                .contains("observed amount values: unknown"),
            "commented wrapper patterns must not resolve boundary operands; got {:?}",
            activation.missing_discriminators
        );
    }

    #[test]
    fn activation_evidence_ignores_inline_commented_match_wrapper_pattern() {
        let owner = function(
            "pub fn score(raw_amount: Option<i32>, threshold: i32) -> bool {\n    let _seen = match raw_amount { _ => false }; // Some(amount)\n    let amount = 1;\n    amount >= threshold\n}",
        );
        let test = test_with_call("score_uses_boundary", "score(Some(100), 100);");
        let probe = probe(ProbeFamily::Predicate, "amount >= threshold");

        let activation = activation_evidence(
            &probe,
            Some(&owner),
            &[&test],
            &[],
            None,
            &crate::analysis::rust_index::RustIndex::default(),
            false,
        );

        assert!(!has_observed_boundary_equality(&activation));
        assert_eq!(activation.missing_discriminators.len(), 1);
        assert!(
            activation.missing_discriminators[0]
                .reason
                .contains("observed amount values: unknown"),
            "inline commented wrapper patterns must not resolve boundary operands; got {:?}",
            activation.missing_discriminators
        );
    }

    #[test]
    fn activation_evidence_keeps_computed_local_boundary_operand_unresolved() {
        let owner = function(
            "pub fn score(raw_amount: i32, threshold: i32) -> bool {\n    let amount = raw_amount + 1;\n    amount >= threshold\n}",
        );
        let test = test_with_call("score_uses_boundary", "score(100, 100);");
        let probe = probe(ProbeFamily::Predicate, "amount >= threshold");

        let activation = activation_evidence(
            &probe,
            Some(&owner),
            &[&test],
            &[],
            None,
            &crate::analysis::rust_index::RustIndex::default(),
            false,
        );

        assert!(!has_observed_boundary_equality(&activation));
        assert_eq!(activation.missing_discriminators.len(), 1);
        assert_eq!(
            activation.missing_discriminators[0].value,
            "amount == threshold"
        );
        assert!(
            activation.missing_discriminators[0]
                .reason
                .contains("observed amount values: unknown")
        );
    }

    #[test]
    fn activation_evidence_reports_missing_boundary_discriminator() {
        let owner = function("pub fn score(amount: i32) -> bool {\n    amount > 10\n}");
        let test = test_with_call("score_uses_adjacent_value", "score(9);");
        let probe = probe(ProbeFamily::Predicate, "amount > 10");
        let flow_sinks = vec![FlowSinkFact {
            kind: FlowSinkKind::ReturnValue,
            text: "amount > 10".to_string(),
            line: 2,
            owner: None,
        }];

        let activation = activation_evidence(
            &probe,
            Some(&owner),
            &[&test],
            &flow_sinks,
            None,
            &crate::analysis::rust_index::RustIndex::default(),
            false,
        );

        assert_eq!(activation.missing_discriminators.len(), 1);
        assert_eq!(activation.missing_discriminators[0].value, "amount == 10");
        assert!(
            activation.missing_discriminators[0]
                .reason
                .contains("observed amount values: 9")
        );
        assert_eq!(
            activation.missing_discriminators[0]
                .flow_sink
                .as_ref()
                .map(|sink| &sink.kind),
            Some(&FlowSinkKind::ReturnValue)
        );
    }

    #[test]
    fn activation_evidence_omits_missing_error_variant_when_exact_assertion_exists() {
        let test = test_with_assertion(
            "rejects_revoked",
            "assert_eq!(err, AuthError::RevokedToken);",
            OracleKind::ExactErrorVariant,
        );
        let probe = probe(
            ProbeFamily::ErrorPath,
            "return Err(AuthError::RevokedToken);",
        );
        let flow_sinks = vec![FlowSinkFact {
            kind: FlowSinkKind::ErrorVariant,
            text: "Result::Err(AuthError::RevokedToken)".to_string(),
            line: 2,
            owner: None,
        }];

        let activation = activation_evidence(
            &probe,
            None,
            &[&test],
            &flow_sinks,
            None,
            &crate::analysis::rust_index::RustIndex::default(),
            false,
        );

        assert!(activation.missing_discriminators.is_empty());
    }

    #[test]
    fn activation_evidence_sorts_multiple_missing_discriminators() {
        let owner = function(
            "pub fn score(amount: i32) -> Result<bool, AuthError> {\n    if amount > 10 { return Err(AuthError::Bad); }\n    Ok(true)\n}",
        );
        let test = test_with_call("score_uses_adjacent_value", "score(9);");
        let probe = probe(ProbeFamily::Predicate, "amount > 10");
        let flow_sinks = vec![FlowSinkFact {
            kind: FlowSinkKind::ErrorVariant,
            text: "Result::Err(AuthError::Bad)".to_string(),
            line: 2,
            owner: None,
        }];

        let activation = activation_evidence(
            &probe,
            Some(&owner),
            &[&test],
            &flow_sinks,
            None,
            &crate::analysis::rust_index::RustIndex::default(),
            false,
        );
        let values = activation
            .missing_discriminators
            .iter()
            .map(|fact| fact.value.as_str())
            .collect::<Vec<_>>();

        assert_eq!(values, vec!["AuthError::Bad", "amount == 10"]);
    }

    #[test]
    fn value_facts_for_test_preserves_table_builder_and_assertion_contexts() {
        let test = TestSummary {
            name: "table_and_builder".to_string(),
            file: PathBuf::from("tests/value.rs"),
            start_line: 10,
            end_line: 16,
            body: r#"let rows = [(99, 100), (100, 100)];
let input = Request::builder().amount(100).token("abc").build();
assert_eq!(input.amount, 100);"#
                .to_string(),
            calls: Vec::new(),
            assertions: vec![oracle_fact(
                "assert_eq!(input.amount, 100);",
                OracleKind::ExactValue,
            )],
            literals: Vec::new(),
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        };

        let facts = value_facts_for_test(&test, None);

        assert!(
            facts
                .iter()
                .any(|fact| fact.context == ValueContext::TableRow && fact.value == "99")
        );
        assert!(
            facts
                .iter()
                .any(|fact| fact.context == ValueContext::BuilderMethod && fact.value == "100")
        );
        assert!(
            facts
                .iter()
                .any(|fact| fact.context == ValueContext::BuilderMethod && fact.value == "\"abc\"")
        );
        assert!(
            facts
                .iter()
                .any(|fact| fact.context == ValueContext::AssertionArgument && fact.value == "100")
        );
    }

    #[test]
    fn value_facts_for_test_filters_non_owner_calls_and_reads_enum_call_arguments() {
        let owner = function(
            "pub fn score(error: AuthError) -> Result<(), AuthError> {\n    Err(error)\n}",
        );
        let test = TestSummary {
            name: "enum_call".to_string(),
            file: PathBuf::from("tests/value.rs"),
            start_line: 10,
            end_line: 12,
            body: "other(AuthError::Ignored);\nscore(AuthError::RevokedToken);".to_string(),
            calls: vec![
                CallFact {
                    line: 11,
                    name: "other".to_string(),
                    text: "other(AuthError::Ignored);".to_string(),
                },
                CallFact {
                    line: 12,
                    name: "score".to_string(),
                    text: "score(AuthError::RevokedToken);".to_string(),
                },
                CallFact {
                    line: 13,
                    name: "score".to_string(),
                    text: "score;".to_string(),
                },
            ],
            assertions: Vec::new(),
            literals: Vec::new(),
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        };

        let facts = value_facts_for_test(&test, Some(&owner));

        assert!(
            facts
                .iter()
                .any(|fact| fact.value == "AuthError::RevokedToken")
        );
        assert!(!facts.iter().any(|fact| fact.value == "AuthError::Ignored"));
    }

    #[test]
    fn missing_boundary_handles_missing_left_and_nonliteral_target() {
        let owner = function("pub fn score(amount: i32) -> bool {\n    amount > 10\n}");
        let test = test_with_call("score_uses_other_value", "score(9);");
        let probe = probe(ProbeFamily::Predicate, "threshold > limit");

        let activation = activation_evidence(
            &probe,
            Some(&owner),
            &[&test],
            &[],
            None,
            &crate::analysis::rust_index::RustIndex::default(),
            false,
        );

        assert_eq!(activation.missing_discriminators.len(), 1);
        assert_eq!(
            activation.missing_discriminators[0].value,
            "threshold == limit"
        );
        assert!(
            activation.missing_discriminators[0]
                .reason
                .contains("observed threshold values: unknown")
        );
    }

    #[test]
    fn owner_call_parameter_values_handles_empty_inputs_and_skips_other_calls() {
        let test = TestSummary {
            name: "mixed_calls".to_string(),
            file: PathBuf::from("tests/value.rs"),
            start_line: 10,
            end_line: 12,
            body: "other(1);\nscore(2);".to_string(),
            calls: vec![
                CallFact {
                    line: 11,
                    name: "other".to_string(),
                    text: "other(1);".to_string(),
                },
                CallFact {
                    line: 12,
                    name: "score".to_string(),
                    text: "score(2);".to_string(),
                },
                CallFact {
                    line: 13,
                    name: "score".to_string(),
                    text: "score;".to_string(),
                },
            ],
            assertions: Vec::new(),
            literals: Vec::new(),
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        };

        assert!(owner_call_parameter_values(&[&test], "", &["amount".to_string()]).is_empty());
        assert!(owner_call_parameter_values(&[&test], "score", &[]).is_empty());

        let rows = owner_call_parameter_values(&[&test], "score", &["amount".to_string()]);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][0].value, "2");
    }

    #[test]
    fn text_helpers_handle_braces_escapes_negative_numbers_and_dedup_contexts() {
        let owner = function("pub fn score(amount: i32) -> bool {\n    amount > 10\n}");
        let non_comparison_probe = probe(ProbeFamily::ReturnValue, "amount");
        assert!(
            observed_discriminator_values(
                &non_comparison_probe,
                Some(&owner),
                &[],
                None,
                &crate::analysis::rust_index::RustIndex::default(),
                false,
            )
            .is_empty()
        );
        assert!(
            observed_discriminator_values(
                &probe(ProbeFamily::Predicate, "amount > 10"),
                None,
                &[],
                None,
                &crate::analysis::rust_index::RustIndex::default(),
                false,
            )
            .is_empty()
        );
        assert_eq!(
            comparison_operands("if amount > 10 {"),
            Some(("amount".to_string(), "10".to_string()))
        );
        assert_eq!(comparison_operands("> 10"), None);
        assert_eq!(
            call_arguments(r#"score("a\",b", -12)"#, "score"),
            Some(vec![r#""a\",b""#.to_string(), "-12".to_string()])
        );
        assert_eq!(
            scalar_values(r#""a\"b" -12"#),
            vec!["\"a\\\"b\"".to_string(), "-12".to_string()]
        );

        let mut facts = vec![
            value_fact(1, "score(1)", "1", ValueContext::FunctionArgument),
            value_fact(1, "score(1)", "1", ValueContext::AssertionArgument),
        ];

        sort_value_facts(&mut facts);

        assert_eq!(facts.len(), 2);
    }

    fn probe(family: ProbeFamily, expression: &str) -> Probe {
        Probe {
            id: ProbeId("probe:src/lib.rs:2:score".to_string()),
            location: SourceLocation::new("src/lib.rs", 2, 5),
            owner: None,
            family,
            delta: DeltaKind::Control,
            before: None,
            after: Some(expression.to_string()),
            expression: expression.to_string(),
            expected_sinks: Vec::new(),
            required_oracles: Vec::new(),
        }
    }

    fn function(body: &str) -> FunctionSummary {
        FunctionSummary {
            id: SymbolId("src/lib.rs::score".to_string()),
            name: "score".to_string(),
            file: PathBuf::from("src/lib.rs"),
            start_line: 1,
            end_line: 3,
            body: body.to_string(),
            calls: Vec::new(),
            returns: Vec::new(),
            literals: Vec::new(),
            source_role: FunctionSourceRole::Production,
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        }
    }

    fn test_with_call(name: &str, call: &str) -> TestSummary {
        TestSummary {
            name: name.to_string(),
            file: PathBuf::from("tests/score.rs"),
            start_line: 10,
            end_line: 12,
            body: call.to_string(),
            calls: vec![CallFact {
                name: "score".to_string(),
                line: 11,
                text: call.to_string(),
            }],
            assertions: Vec::new(),
            literals: Vec::new(),
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        }
    }

    fn test_with_assertion(name: &str, assertion: &str, kind: OracleKind) -> TestSummary {
        TestSummary {
            name: name.to_string(),
            file: PathBuf::from("tests/score.rs"),
            start_line: 10,
            end_line: 12,
            body: assertion.to_string(),
            calls: Vec::new(),
            assertions: vec![oracle_fact(assertion, kind)],
            literals: Vec::new(),
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        }
    }

    fn oracle_fact(assertion: &str, kind: OracleKind) -> OracleFact {
        OracleFact {
            kind,
            strength: OracleStrength::Strong,
            line: 11,
            text: assertion.to_string(),
            observed_tokens: Vec::new(),
            ok_value_observed: None,
        }
    }

    fn value_fact(line: usize, text: &str, value: &str, context: ValueContext) -> ValueFact {
        ValueFact {
            line,
            text: text.to_string(),
            value: value.to_string(),
            context,
        }
    }
}
