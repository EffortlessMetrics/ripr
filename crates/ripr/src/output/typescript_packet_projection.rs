//! TypeScript preview → complete repair-packet projection (RIPR-SPEC-0087 §PR7).
//!
//! This module implements the **projection** side of the
//! TypeScript-actionability gate: it reads evidence lines from a
//! `preview`-language finding and — when every precondition in §1.2 holds —
//! builds a `GapRecord` that can be fed to the **shared** Rust validator
//! `validate_agent_gap_record_packet` to determine `repair_packet_ready`.
//!
//! Architectural constraints (non-negotiable):
//! - The validator lives in `output::agent_seam_packets`; this module may call
//!   it because both modules are inside `output/`.
//! - Analysis modules (`analysis/**`) must NOT import `crate::output`, so the
//!   projection CANNOT live there.
//! - No parallel TypeScript-specific completeness validator is introduced.
//!   The only flip gate is `validate_agent_gap_record_packet(..) == Ok(())`.

use crate::domain::Finding;
use crate::output::gap_decision_ledger::{
    GapAnchor, GapRecord, GapRepairRoute, ProjectionEligibility, preview_receipt_write_command,
};
use std::collections::BTreeMap;

/// The authority-boundary string carried by all TypeScript preview packets.
const TS_AUTHORITY_BOUNDARY: &str = "preview_advisory_only";

/// The target assertion shape projected for a TypeScript repair packet,
/// together with the static reachability verdict that produced it (#4105).
///
/// A complete packet must not present the observed call input as the shape for
/// the missing discriminator when that input statically cannot reach the named
/// boundary: an agent following the packet verbatim would duplicate a
/// non-discriminating assertion while the packet claims "complete and
/// delegatable".
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TargetAssertionShape {
    /// The observed oracle shape stands: the observed call input either hits
    /// the missing-discriminator boundary, or its reachability is not
    /// statically decidable from the available evidence (fail-open is allowed
    /// only where non-reach cannot be established).
    Observed { shape: String },
    /// The observed call input provably does NOT reach the named boundary.
    /// `shape` carries an explicit boundary placeholder instead of the
    /// observed input, and the packet must fail closed through the shared
    /// validator.
    Unreachable {
        shape: String,
        observed_call: String,
        discriminator: String,
    },
    /// The discriminator's boundary operand is a named constant
    /// (`amount == DISCOUNT_THRESHOLD`) that this projection cannot resolve
    /// to a concrete value, so no observed literal input can be shown to hit
    /// it (#4215). `shape` carries the same boundary placeholder as
    /// `Unreachable`, and the packet fails closed: "undecided" is not
    /// evidence that the observed input discriminates the boundary.
    UnresolvedBoundary {
        shape: String,
        observed_call: String,
        discriminator: String,
        constant: String,
    },
    /// The discriminator's boundary operand is a plain identifier
    /// (`amount == threshold`) that the observed call is not shown to bind to
    /// a concrete value (#4759): either the analysis side did not pin both
    /// sides as read-only owner parameters, or the observed arguments at
    /// those positions are not both integer literals. `shape` carries the
    /// boundary placeholder and the packet fails closed.
    UnboundOperand {
        shape: String,
        observed_call: String,
        discriminator: String,
        operand: String,
    },
    /// The observed call input does not hit the boundary (or its constant
    /// was unresolved on the discriminator text alone), but the analysis
    /// side pinned the boundary input statically (`typescript_boundary_input`
    /// evidence: the owner reads the parameter unchanged and the other operand
    /// is an integer literal or a single immutable integer module `const`).
    /// `shape` is the observed call with that argument replaced by the
    /// boundary value, so the packet stays delegatable with a concrete input
    /// that hits the missing discriminator.
    DerivedBoundary {
        shape: String,
        observed_call: String,
        boundary_call: String,
        discriminator: String,
        operand: String,
        value: i64,
    },
}

/// The analysis-side boundary input fact (`typescript_boundary_input:
/// parameter=<p>;index=<i>;operand=<o>;value=<v>`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TypeScriptBoundaryInputFact {
    parameter: String,
    index: usize,
    operand: String,
    value: i64,
}

const TYPESCRIPT_BOUNDARY_INPUT_PREFIX: &str = "typescript_boundary_input: ";

impl TypeScriptBoundaryInputFact {
    /// Parse one evidence line; any missing or malformed field is `None`.
    pub(crate) fn parse(line: &str) -> Option<Self> {
        let body = line.strip_prefix(TYPESCRIPT_BOUNDARY_INPUT_PREFIX)?;
        let mut fields = BTreeMap::new();
        for field in body.split(';') {
            let (key, value) = field.split_once('=')?;
            if fields.insert(key.trim(), value.trim()).is_some() {
                return None;
            }
        }
        if fields.len() != 4 {
            return None;
        }
        let parameter = fields.get("parameter")?.to_string();
        let operand = fields.get("operand")?.to_string();
        if !is_plain_identifier(&parameter) || operand.is_empty() {
            return None;
        }
        Some(Self {
            parameter,
            index: fields.get("index")?.parse().ok()?,
            operand,
            value: fields.get("value")?.parse().ok()?,
        })
    }
}

/// The single boundary input fact on a finding; two or more disagreeing
/// lines are ambiguous and yield `None`.
fn typescript_boundary_input_fact(finding: &Finding) -> Option<TypeScriptBoundaryInputFact> {
    single_evidence_line(finding, TYPESCRIPT_BOUNDARY_INPUT_PREFIX)
        .and_then(TypeScriptBoundaryInputFact::parse)
}

/// The analysis-side fact that both sides of the changed comparison are
/// read-only owner parameters (`typescript_boundary_parameters:
/// parameter=<p>;index=<i>;operand=<o>;operand_index=<j>`, #4759).
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TypeScriptBoundaryParametersFact {
    parameter: String,
    index: usize,
    operand: String,
    operand_index: usize,
}

const TYPESCRIPT_BOUNDARY_PARAMETERS_PREFIX: &str = "typescript_boundary_parameters: ";

impl TypeScriptBoundaryParametersFact {
    /// Parse one evidence line; any missing, duplicate, or malformed field,
    /// or two sides naming the same parameter or position, is `None`.
    pub(crate) fn parse(line: &str) -> Option<Self> {
        let body = line.strip_prefix(TYPESCRIPT_BOUNDARY_PARAMETERS_PREFIX)?;
        let mut fields = BTreeMap::new();
        for field in body.split(';') {
            let (key, value) = field.split_once('=')?;
            if fields.insert(key.trim(), value.trim()).is_some() {
                return None;
            }
        }
        if fields.len() != 4 {
            return None;
        }
        let parameter = fields.get("parameter")?.to_string();
        let operand = fields.get("operand")?.to_string();
        let index: usize = fields.get("index")?.parse().ok()?;
        let operand_index: usize = fields.get("operand_index")?.parse().ok()?;
        if !is_plain_identifier(&parameter)
            || !is_plain_identifier(&operand)
            || parameter == operand
            || index == operand_index
        {
            return None;
        }
        Some(Self {
            parameter,
            index,
            operand,
            operand_index,
        })
    }

    /// The argument positions of `(receiver, boundary)` when the fact names
    /// exactly these two parameters, in either order.
    fn positions_for(&self, receiver: &str, boundary: &str) -> Option<(usize, usize)> {
        if receiver == self.parameter && boundary == self.operand {
            Some((self.index, self.operand_index))
        } else if receiver == self.operand && boundary == self.parameter {
            Some((self.operand_index, self.index))
        } else {
            None
        }
    }
}

fn typescript_boundary_parameters_fact(
    finding: &Finding,
) -> Option<TypeScriptBoundaryParametersFact> {
    single_evidence_line(finding, TYPESCRIPT_BOUNDARY_PARAMETERS_PREFIX)
        .and_then(TypeScriptBoundaryParametersFact::parse)
}

/// The only evidence line with `prefix`; none or several is `None`.
fn single_evidence_line<'f>(finding: &'f Finding, prefix: &str) -> Option<&'f str> {
    let mut lines = finding
        .evidence
        .iter()
        .filter(|line| line.starts_with(prefix));
    let first = lines.next()?;
    if lines.next().is_some() {
        return None;
    }
    Some(first)
}

impl TargetAssertionShape {
    fn shape(&self) -> &str {
        match self {
            Self::Observed { shape } => shape,
            Self::Unreachable { shape, .. }
            | Self::UnresolvedBoundary { shape, .. }
            | Self::UnboundOperand { shape, .. }
            | Self::DerivedBoundary { shape, .. } => shape,
        }
    }

    /// Shared-validator ineligibility reason when the observed call input
    /// provably cannot reach the named boundary. `None` keeps the packet
    /// eligible through the normal complete-contract projection.
    fn packet_ineligibility_reason(&self) -> Option<String> {
        match self {
            Self::Observed { .. } | Self::DerivedBoundary { .. } => None,
            Self::Unreachable {
                observed_call,
                discriminator,
                ..
            } => Some(format!(
                "observed call input `{observed_call}` does not reach the missing \
                 discriminator `{discriminator}`; derive an input that hits the \
                 boundary before the packet is delegatable"
            )),
            Self::UnresolvedBoundary {
                observed_call,
                discriminator,
                constant,
                ..
            } => Some(format!(
                "boundary constant `{constant}` in the missing discriminator \
                 `{discriminator}` is not resolved to a concrete value, so the observed \
                 call input `{observed_call}` is not shown to hit it; derive an input \
                 equal to `{constant}` before the packet is delegatable"
            )),
            Self::UnboundOperand {
                observed_call,
                discriminator,
                operand,
                ..
            } => Some(format!(
                "boundary operand `{operand}` in the missing discriminator \
                 `{discriminator}` is not bound to a concrete value by the observed \
                 call input `{observed_call}`, so that input is not shown to hit it; \
                 derive an input where `{discriminator}` holds before the packet is \
                 delegatable"
            )),
        }
    }

    /// Stop condition forbidding reuse of the observed input when the packet
    /// fails closed on the boundary; `None` for the observed shape.
    fn boundary_stop_condition(&self) -> Option<String> {
        match self {
            Self::Observed { .. } => None,
            Self::DerivedBoundary {
                observed_call,
                boundary_call,
                discriminator,
                operand,
                value,
                ..
            } => {
                let source = if *operand == value.to_string() {
                    "the literal boundary".to_string()
                } else {
                    format!("`{operand}` = {value}")
                };
                Some(format!(
                    "The observed call `{observed_call}` is not shown to hit the missing \
                     discriminator `{discriminator}`; the boundary input `{boundary_call}` \
                     is derived from {source}. Stop if that derivation no longer holds: \
                     re-run `ripr check` before writing the assertion."
                ))
            }
            Self::Unreachable { discriminator, .. } => Some(format!(
                "Do not reuse the observed call input; it cannot reach the missing \
                 discriminator `{discriminator}`. Derive an input that hits the \
                 boundary first."
            )),
            Self::UnresolvedBoundary {
                discriminator,
                constant,
                ..
            } => Some(format!(
                "Do not reuse the observed call input; it is not shown to equal \
                 `{constant}` in the missing discriminator `{discriminator}`. Derive \
                 an input equal to `{constant}` first."
            )),
            Self::UnboundOperand {
                discriminator,
                operand,
                ..
            } => Some(format!(
                "Do not reuse the observed call input; it is not shown to bind \
                 `{operand}` so that the missing discriminator `{discriminator}` \
                 holds. Derive an input where `{discriminator}` holds first."
            )),
        }
    }
}

/// Build the assertion the repair should add, from the borrowed call shape,
/// gated by a static reachability verdict against the named discriminator
/// (issue #4105).
///
/// The borrowed assertion supplies only the observed call (`applyDiscount(100,
/// 100)`), which is what makes the target concrete. Its expected literal and
/// matcher belong to a different, weaker check: the projection only runs for
/// weakly-exposed findings, so by construction the borrowed assertion does not
/// pin the changed behavior. Re-using its literal under `toBe` fabricated
/// assertions such as `expect(result).toBe(50)` from `toBeGreaterThan(50)`.
/// Static evidence cannot know the right expected value, so the shape uses the
/// same `expected` placeholder as the Rust assertion shapes.
///
/// Reachability (#4105):
/// - No parseable discriminator comparison, or no statically decidable
///   verdict → the observed shape stands unchanged.
/// - The observed call input hits the boundary → the observed shape stands
///   (landed behavior for boundary-hitting fixtures).
/// - The observed call input provably does NOT hit the boundary → a
///   placeholder shape naming the boundary (repo convention
///   `/* boundary input for <discriminator> */`) replaces the observed input;
///   the caller fails the packet closed via the shared validator.
/// - The boundary operand is an unresolved named constant
///   (`amount == DISCOUNT_THRESHOLD`) and no argument is that constant → the
///   same placeholder, failed closed (#4215). A constant is never bound by a
///   call argument, so unlike a parameter-named boundary (`amount >=
///   threshold`) "undecided" here means the observed input is unrelated to
///   the boundary, not that it might bind to it.
#[cfg(test)]
pub(crate) fn typescript_target_assertion_shape(
    family: &crate::domain::ProbeFamily,
    observed: &str,
    missing_discriminator: Option<&str>,
    owner_name: Option<&str>,
) -> TargetAssertionShape {
    typescript_target_assertion_shape_with_boundary_input(
        family,
        observed,
        missing_discriminator,
        owner_name,
        None,
    )
}

/// [`typescript_target_assertion_shape`] with the analysis-side boundary
/// input fact and no parameter-pair fact.
#[cfg(test)]
pub(crate) fn typescript_target_assertion_shape_with_boundary_input(
    family: &crate::domain::ProbeFamily,
    observed: &str,
    missing_discriminator: Option<&str>,
    owner_name: Option<&str>,
    boundary_input: Option<&TypeScriptBoundaryInputFact>,
) -> TargetAssertionShape {
    typescript_target_assertion_shape_with_boundary_facts(
        family,
        observed,
        missing_discriminator,
        owner_name,
        boundary_input,
        None,
    )
}

/// [`typescript_target_assertion_shape`] with the analysis-side boundary
/// facts. The input fact only ever replaces a non-delegatable verdict
/// (`Misses`, an unresolved constant, or an undecided binding) with a
/// concrete boundary input; it never overrides an observed input that hits
/// the boundary, and it applies only when it names the same comparison: the
/// discriminator's receiver is the fact's parameter, its boundary operand is
/// the fact's operand (text-equal constant or equal integer literal), the
/// operator is equality, and the observed call has an argument at the
/// fact's position.
///
/// The parameter-pair fact (#4759) is the only way a plain-identifier
/// boundary (`amount == threshold`) is judged: it binds both sides to call
/// positions, the observed integer-literal arguments there decide Hits or
/// Misses, and a miss is re-derived by giving the receiver the boundary's
/// argument. Without the fact, or without literal arguments, the packet
/// fails closed.
pub(crate) fn typescript_target_assertion_shape_with_boundary_facts(
    family: &crate::domain::ProbeFamily,
    observed: &str,
    missing_discriminator: Option<&str>,
    owner_name: Option<&str>,
    boundary_input: Option<&TypeScriptBoundaryInputFact>,
    boundary_parameters: Option<&TypeScriptBoundaryParametersFact>,
) -> TargetAssertionShape {
    let expected_clause = match family {
        crate::domain::ProbeFamily::ErrorPath => ".toThrow(expected)",
        _ => ".toBe(expected)",
    };
    let observed_shape = TargetAssertionShape::Observed {
        shape: format!("expect({observed}){expected_clause}"),
    };
    let Some(discriminator) = missing_discriminator
        .map(str::trim)
        .filter(|d| !d.is_empty())
    else {
        return observed_shape;
    };
    let Some(call) = parse_static_call_expression(observed) else {
        return observed_shape;
    };
    if !call_callee_is_owner(&call.callee, owner_name) {
        return observed_shape;
    }
    let Some(comparison) = parse_static_comparison(discriminator, boundary_parameters) else {
        return observed_shape;
    };
    let placeholder_shape = || {
        format!(
            "expect({}(/* boundary input for {discriminator} */)){expected_clause}",
            call.callee
        )
    };
    let reach = static_argument_reaches_boundary(&call, &comparison, boundary_parameters);
    if matches!(reach, BoundaryReach::Misses)
        && let Some((boundary_call, value)) =
            derived_parameter_boundary_call(&call, &comparison, boundary_parameters)
        && let StaticBoundary::Parameter(operand) = &comparison.boundary
    {
        return TargetAssertionShape::DerivedBoundary {
            shape: format!("expect({boundary_call}){expected_clause}"),
            observed_call: observed.to_string(),
            boundary_call,
            discriminator: discriminator.to_string(),
            operand: operand.clone(),
            value,
        };
    }
    if !matches!(reach, BoundaryReach::Hits)
        && let Some(fact) = boundary_input
        && let Some(boundary_call) = derived_boundary_call(&call, &comparison, fact)
    {
        return TargetAssertionShape::DerivedBoundary {
            shape: format!("expect({boundary_call}){expected_clause}"),
            observed_call: observed.to_string(),
            boundary_call,
            discriminator: discriminator.to_string(),
            operand: fact.operand.clone(),
            value: fact.value,
        };
    }
    match reach {
        BoundaryReach::Hits | BoundaryReach::Undecided => observed_shape,
        BoundaryReach::Misses => TargetAssertionShape::Unreachable {
            shape: placeholder_shape(),
            observed_call: observed.to_string(),
            discriminator: discriminator.to_string(),
        },
        BoundaryReach::UnresolvedConstant(constant) => TargetAssertionShape::UnresolvedBoundary {
            shape: placeholder_shape(),
            observed_call: observed.to_string(),
            discriminator: discriminator.to_string(),
            constant,
        },
        BoundaryReach::UnboundOperand(operand) => TargetAssertionShape::UnboundOperand {
            shape: placeholder_shape(),
            observed_call: observed.to_string(),
            discriminator: discriminator.to_string(),
            operand,
        },
    }
}

/// A statically parseable call expression: `callee(arg, ...)`.
struct StaticCall {
    callee: String,
    args: Vec<String>,
}

/// Parse a plain call expression `name(a, b)` (dotted callee allowed).
///
/// Returns `None` for anything else — variables (`result`), member tails
/// (`login('alice').length`), or wrapped calls (`wrap(login('alice'))`) — so
/// reachability stays undecided instead of binding the wrong expression.
fn parse_static_call_expression(expr: &str) -> Option<StaticCall> {
    let expr = expr.trim();
    let open = expr.find('(')?;
    // Quote- and escape-aware scan for the first `(`'s matching close; the
    // expression only counts as one static call when that close is the final
    // character (rejects compounds like `login('a') + login('b')` and
    // `login('ab')('c')`).
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut matching_close = None;
    for (i, ch) in expr[open..].char_indices() {
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == q {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' | '`' => quote = Some(ch),
            '(' => depth += 1,
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    matching_close = Some(open + i);
                    break;
                }
            }
            _ => {}
        }
    }
    let close = matching_close?;
    if close != expr.len() - 1 {
        return None;
    }
    let callee = expr[..open].trim();
    let mut callee_chars = callee.chars();
    let first_ok = callee_chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$');
    if !first_ok || !callee_chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '$'))
    {
        return None;
    }
    let inner = &expr[open + 1..close];
    let args = if inner.trim().is_empty() {
        Vec::new()
    } else {
        split_top_level_args(inner)
    };
    Some(StaticCall {
        callee: callee.to_string(),
        args,
    })
}

/// Split call arguments on top-level commas, respecting nesting and quotes.
fn split_top_level_args(inner: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    let mut current = String::new();
    let mut escaped = false;
    for ch in inner.chars() {
        match quote {
            Some(q) => {
                // `\"` inside a string neither closes it nor splits on a comma.
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == q {
                    quote = None;
                }
                current.push(ch);
            }
            None => match ch {
                '\'' | '"' | '`' => {
                    quote = Some(ch);
                    current.push(ch);
                }
                '(' | '[' | '{' => {
                    depth += 1;
                    current.push(ch);
                }
                ')' | ']' | '}' => {
                    depth = depth.saturating_sub(1);
                    current.push(ch);
                }
                ',' if depth == 0 => {
                    args.push(current.trim().to_string());
                    current.clear();
                }
                _ => current.push(ch),
            },
        }
    }
    if !current.trim().is_empty() {
        args.push(current.trim().to_string());
    }
    args
}

/// The observed oracle expression counts as owner evidence only when its
/// callee resolves to the finding's owner short name (dotted callees compare
/// by their last segment). Without an owner identity the reachability verdict
/// is refused — never judged.
fn call_callee_is_owner(callee: &str, owner_name: Option<&str>) -> bool {
    let Some(owner) = owner_name.map(str::trim).filter(|o| !o.is_empty()) else {
        return false;
    };
    callee
        .rsplit('.')
        .next()
        .is_some_and(|short| short == owner)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StaticComparisonOp {
    Equal,        // == / ===
    NotEqual,     // != / !==
    GreaterEqual, // >=
    LessEqual,    // <=
    Greater,      // >
    Less,         // <
}

impl StaticComparisonOp {
    /// The operator with its operands swapped: `a >= b` is `b <= a`.
    fn mirrored(self) -> Self {
        match self {
            Self::Equal | Self::NotEqual => self,
            Self::GreaterEqual => Self::LessEqual,
            Self::LessEqual => Self::GreaterEqual,
            Self::Greater => Self::Less,
            Self::Less => Self::Greater,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum StaticBoundary {
    Int(i64),
    Str(String),
    /// A constant-shaped operand (`DISCOUNT_THRESHOLD`) with no value in the
    /// finding evidence.
    Constant(String),
    /// A plain lowercase identifier (`threshold`): possibly an owner
    /// parameter the call binds, possibly a module value (#4759).
    Parameter(String),
}

struct StaticComparison {
    receiver: String,
    receiver_is_length: bool,
    op: StaticComparisonOp,
    boundary: StaticBoundary,
}

/// Static verdict of the observed call input against the discriminator.
#[derive(Debug, Eq, PartialEq)]
enum BoundaryReach {
    Hits,
    Misses,
    Undecided,
    /// The boundary is a named constant that no argument names.
    UnresolvedConstant(String),
    /// The boundary is a plain identifier the observed call is not shown to
    /// bind to a concrete value.
    UnboundOperand(String),
}

/// Parse a discriminator like `user.length == 3` into a static comparison.
///
/// Conservative: only `<ident>` / `<ident>.length` receivers and int, plain
/// string, constant-shaped (`UPPER_CASE`), or plain identifier boundaries are
/// recognized; anything else stays undecided. A literal or constant side is
/// preferred as the boundary in either operand order, so `LIMIT == amount`
/// never reads `amount` as the boundary.
fn parse_static_comparison(
    discriminator: &str,
    boundary_parameters: Option<&TypeScriptBoundaryParametersFact>,
) -> Option<StaticComparison> {
    const OPS: [(&str, StaticComparisonOp); 8] = [
        ("===", StaticComparisonOp::Equal),
        ("!==", StaticComparisonOp::NotEqual),
        ("==", StaticComparisonOp::Equal),
        ("!=", StaticComparisonOp::NotEqual),
        (">=", StaticComparisonOp::GreaterEqual),
        ("<=", StaticComparisonOp::LessEqual),
        (">", StaticComparisonOp::Greater),
        ("<", StaticComparisonOp::Less),
    ];
    let mut best: Option<(usize, usize, StaticComparisonOp)> = None;
    for (symbol, op) in OPS {
        if let Some(position) = discriminator.find(symbol) {
            let better = match best {
                None => true,
                Some((best_position, best_len, _)) => {
                    position < best_position
                        || (position == best_position && symbol.len() > best_len)
                }
            };
            if better {
                best = Some((position, symbol.len(), op));
            }
        }
    }
    let (position, len, op) = best?;
    let left = discriminator[..position].trim();
    let right = discriminator[position + len..].trim();
    // The analysis side keeps operand order (`DISCOUNT_THRESHOLD <= amount`
    // becomes `DISCOUNT_THRESHOLD == amount`), so a boundary written first
    // is read with the operator mirrored (#4215 review).
    // A parameter-pair fact naming both sides is the analysis side's own
    // reading that both are read-only owner parameters (#4759 review): a
    // CONSTANT_CASE parameter would otherwise parse as a module constant
    // and close the packet the emitted fact is evidence for. The fact's own
    // module rules (plain positional bindings, read-only, changed line runs
    // on every call) already held when it was emitted, and the packet still
    // judges only through integer-literal arguments at the fact's positions.
    if boundary_parameters.is_some_and(|fact| {
        (fact.parameter == left && fact.operand == right)
            || (fact.parameter == right && fact.operand == left)
    }) {
        return static_parameter_pair_comparison(left, op, right);
    }
    static_comparison_from(left, op, right, false)
        .or_else(|| static_comparison_from(right, op.mirrored(), left, false))
        .or_else(|| static_comparison_from(left, op, right, true))
}

/// Both comparison sides are the parameter-pair fact's own parameters, so
/// both are plain identifiers by the fact's parse; read them as parameters
/// regardless of name shape (#4759 review).
fn static_parameter_pair_comparison(
    receiver: &str,
    op: StaticComparisonOp,
    boundary_raw: &str,
) -> Option<StaticComparison> {
    let receiver_is_length = parse_static_receiver(receiver)?;
    is_plain_identifier(boundary_raw).then(|| StaticComparison {
        receiver: receiver.trim().to_string(),
        receiver_is_length,
        op,
        boundary: StaticBoundary::Parameter(boundary_raw.to_string()),
    })
}

fn static_comparison_from(
    receiver: &str,
    op: StaticComparisonOp,
    boundary_raw: &str,
    identifier_boundary: bool,
) -> Option<StaticComparison> {
    let receiver_is_length = parse_static_receiver(receiver)?;
    let boundary = if identifier_boundary {
        is_parameter_shaped_operand(boundary_raw)
            .then(|| StaticBoundary::Parameter(boundary_raw.to_string()))
    } else {
        parse_static_literal(boundary_raw).or_else(|| {
            is_constant_shaped_operand(boundary_raw)
                .then(|| StaticBoundary::Constant(boundary_raw.to_string()))
        })
    }?;
    Some(StaticComparison {
        receiver: receiver.trim().to_string(),
        receiver_is_length,
        op,
        boundary,
    })
}

/// Recognize `<ident>` or `<ident>.length` receivers; other shapes (member
/// chains, index expressions) are not statically decidable. Returns whether
/// the receiver is a `.length` probe.
fn parse_static_receiver(receiver: &str) -> Option<bool> {
    if is_plain_identifier(receiver) {
        return Some(false);
    }
    let stem = receiver.strip_suffix(".length")?;
    if is_plain_identifier(stem) {
        Some(true)
    } else {
        None
    }
}

fn is_plain_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let first_ok = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$');
    first_ok && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

/// A quoted string literal without escape sequences. Escapes make the static
/// length and byte-for-byte equality unreliable, so they fail open instead.
fn parse_plain_string_literal(raw: &str) -> Option<String> {
    let quote = raw.chars().next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }
    if raw.chars().count() < 2 || !raw.ends_with(quote) {
        return None;
    }
    let inner = &raw[1..raw.len() - 1];
    // Escapes and an inner matching quote make the static length and
    // byte-for-byte equality unreliable, so they fail open instead.
    if inner.contains('\\') || inner.contains(quote) {
        return None;
    }
    Some(inner.to_string())
}

fn parse_static_literal(raw: &str) -> Option<StaticBoundary> {
    if let Some(value) = parse_integer_literal(raw) {
        return Some(StaticBoundary::Int(value));
    }
    parse_plain_string_literal(raw).map(StaticBoundary::Str)
}

/// An integer literal as `i64`, reading JavaScript numeric separators
/// (`5_000`) the way the analysis-side boundary input does. A separator
/// must sit between two digits.
fn parse_integer_literal(raw: &str) -> Option<i64> {
    if !raw.contains('_') {
        return raw.parse::<i64>().ok();
    }
    // JavaScript rejects separators in a legacy leading-zero literal.
    if raw.trim_start_matches(['-', '+']).starts_with('0') {
        return None;
    }
    let bytes = raw.as_bytes();
    let separators_between_digits = bytes.iter().enumerate().all(|(index, byte)| {
        *byte != b'_'
            || (index > 0
                && bytes[index - 1].is_ascii_digit()
                && bytes.get(index + 1).is_some_and(u8::is_ascii_digit))
    });
    if !separators_between_digits {
        return None;
    }
    raw.replace('_', "").parse::<i64>().ok()
}

/// A constant-shaped operand: leading ASCII uppercase, then uppercase, digits,
/// or `_` only. Mirrors the analysis-side `is_constant_shaped_operand`
/// (typescript classifier, #4104-E3). A lowercase identifier is NOT treated
/// as a constant: it may name a parameter the observed arguments bind to.
fn is_constant_shaped_operand(operand: &str) -> bool {
    operand.starts_with(|ch: char| ch.is_ascii_uppercase())
        && operand
            .chars()
            .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
}

/// A plain identifier that could name a parameter: not constant-shaped and
/// not a value keyword (`true`, `null`, `undefined`, `NaN`, `Infinity`),
/// whose comparisons this projection leaves undecided as before.
fn is_parameter_shaped_operand(operand: &str) -> bool {
    const VALUE_KEYWORDS: [&str; 7] = [
        "true",
        "false",
        "null",
        "undefined",
        "NaN",
        "Infinity",
        "this",
    ];
    is_plain_identifier(operand)
        && !is_constant_shaped_operand(operand)
        && !VALUE_KEYWORDS.contains(&operand)
}

fn apply_static_comparison(op: StaticComparisonOp, lhs: i64, rhs: i64) -> bool {
    match op {
        StaticComparisonOp::Equal => lhs == rhs,
        StaticComparisonOp::NotEqual => lhs != rhs,
        StaticComparisonOp::GreaterEqual => lhs >= rhs,
        StaticComparisonOp::LessEqual => lhs <= rhs,
        StaticComparisonOp::Greater => lhs > rhs,
        StaticComparisonOp::Less => lhs < rhs,
    }
}

/// Statically decide whether the observed call's argument satisfies the
/// discriminator comparison.
///
/// The discriminator receiver binds to the observed argument only when the
/// call has exactly one argument: without signature evidence a multi-argument
/// binding would be a guess, and a wrong "cannot reach" verdict is worse than
/// no verdict. An unresolved constant boundary is judged before that binding:
/// it fails the packet closed at any arity unless some argument IS the
/// constant, because a wrong actionable packet is worse than a missed one.
///
/// A plain-identifier boundary is decided only through the parameter-pair
/// fact (#4759); otherwise it fails closed as unbound at any arity.
fn static_argument_reaches_boundary(
    call: &StaticCall,
    comparison: &StaticComparison,
    boundary_parameters: Option<&TypeScriptBoundaryParametersFact>,
) -> BoundaryReach {
    if let StaticBoundary::Constant(name) = &comparison.boundary {
        if call.args.iter().any(|argument| argument.trim() == name) {
            return BoundaryReach::Undecided;
        }
        return BoundaryReach::UnresolvedConstant(name.clone());
    }
    if let StaticBoundary::Parameter(name) = &comparison.boundary {
        return match parameter_argument_values(call, comparison, boundary_parameters) {
            Some((_, receiver, boundary)) => {
                if apply_static_comparison(comparison.op, receiver, boundary) {
                    BoundaryReach::Hits
                } else {
                    BoundaryReach::Misses
                }
            }
            None => BoundaryReach::UnboundOperand(name.clone()),
        };
    }
    match static_literal_argument_reaches_boundary(call, comparison) {
        Some(true) => BoundaryReach::Hits,
        Some(false) => BoundaryReach::Misses,
        None => BoundaryReach::Undecided,
    }
}

/// The observed call with the fact's argument replaced by the boundary
/// value, when the fact names exactly this comparison. `None` keeps the
/// existing verdict.
fn derived_boundary_call(
    call: &StaticCall,
    comparison: &StaticComparison,
    fact: &TypeScriptBoundaryInputFact,
) -> Option<String> {
    if comparison.receiver_is_length
        || comparison.op != StaticComparisonOp::Equal
        || comparison.receiver != fact.parameter
    {
        return None;
    }
    let operand_matches = match &comparison.boundary {
        StaticBoundary::Constant(name) => *name == fact.operand,
        StaticBoundary::Int(value) => {
            *value == fact.value && parse_integer_literal(&fact.operand) == Some(*value)
        }
        StaticBoundary::Str(_) | StaticBoundary::Parameter(_) => false,
    };
    if !operand_matches || fact.index >= call.args.len() || !call_positions_are_stable(call) {
        return None;
    }
    let args: Vec<String> = call
        .args
        .iter()
        .enumerate()
        .map(|(index, argument)| {
            if index == fact.index {
                fact.value.to_string()
            } else {
                argument.clone()
            }
        })
        .collect();
    Some(format!("{}({})", call.callee, args.join(", ")))
}

/// A spread argument shifts every later position; refuse to bind. An escape
/// anywhere in the arguments makes the split uncertain, and a rebuilt call
/// copies the other arguments verbatim, so refuse that too.
fn call_positions_are_stable(call: &StaticCall) -> bool {
    !call
        .args
        .iter()
        .any(|argument| argument.trim_start().starts_with("...") || argument.contains('\\'))
}

/// The receiver's argument position and the integer-literal arguments bound
/// to `(receiver, boundary)` when the parameter-pair fact names exactly this
/// comparison and both observed arguments are integer literals. `None` means
/// the binding is not shown.
fn parameter_argument_values(
    call: &StaticCall,
    comparison: &StaticComparison,
    fact: Option<&TypeScriptBoundaryParametersFact>,
) -> Option<(usize, i64, i64)> {
    let StaticBoundary::Parameter(boundary) = &comparison.boundary else {
        return None;
    };
    if comparison.receiver_is_length || !call_positions_are_stable(call) {
        return None;
    }
    let (receiver_index, boundary_index) = fact?.positions_for(&comparison.receiver, boundary)?;
    let receiver = parse_integer_literal(call.args.get(receiver_index)?.trim())?;
    let boundary = parse_integer_literal(call.args.get(boundary_index)?.trim())?;
    Some((receiver_index, receiver, boundary))
}

/// The observed call with the receiver's argument replaced by the
/// boundary's value, so an equality discriminator holds (#4759). Only for
/// an equality comparison the parameter-pair fact binds to literals.
fn derived_parameter_boundary_call(
    call: &StaticCall,
    comparison: &StaticComparison,
    fact: Option<&TypeScriptBoundaryParametersFact>,
) -> Option<(String, i64)> {
    if comparison.op != StaticComparisonOp::Equal {
        return None;
    }
    let (receiver_index, _, boundary) = parameter_argument_values(call, comparison, fact)?;
    let args: Vec<String> = call
        .args
        .iter()
        .enumerate()
        .map(|(index, argument)| {
            if index == receiver_index {
                boundary.to_string()
            } else {
                argument.clone()
            }
        })
        .collect();
    Some((format!("{}({})", call.callee, args.join(", ")), boundary))
}

/// `Some(true)` reaches a literal boundary, `Some(false)` provably does not,
/// `None` is undecided.
fn static_literal_argument_reaches_boundary(
    call: &StaticCall,
    comparison: &StaticComparison,
) -> Option<bool> {
    if call.args.len() != 1 {
        return None;
    }
    let argument = call.args[0].trim();
    if comparison.receiver_is_length {
        let text = parse_plain_string_literal(argument)?;
        let StaticBoundary::Int(boundary) = comparison.boundary else {
            return None;
        };
        // JavaScript `String.prototype.length` counts UTF-16 code units, not
        // Unicode scalar values; the two differ outside the BMP (e.g. emoji).
        let length = text.encode_utf16().count() as i64;
        return Some(apply_static_comparison(comparison.op, length, boundary));
    }
    match &comparison.boundary {
        StaticBoundary::Int(boundary) => {
            let value = parse_integer_literal(argument)?;
            Some(apply_static_comparison(comparison.op, value, *boundary))
        }
        StaticBoundary::Constant(_) | StaticBoundary::Parameter(_) => None,
        StaticBoundary::Str(boundary) => {
            let value = parse_plain_string_literal(argument)?;
            match comparison.op {
                StaticComparisonOp::Equal => Some(value == *boundary),
                StaticComparisonOp::NotEqual => Some(value != *boundary),
                // Relational comparisons over strings are not statically
                // decided here.
                _ => None,
            }
        }
    }
}

/// Project a TypeScript preview finding into a `GapRecord` for validator
/// consumption, applying all §1.2 preconditions (G-A through G-G).
///
/// Returns `None` whenever ANY precondition fails — the false case requires
/// no positive proof. Only `Ok(())` from the shared validator leads to a flip.
///
/// # Preconditions checked (fail-closed)
/// - G-A: `actionability_category == "incomplete_repair_packet"`
/// - G-C: no named TypeScript limitation, plus a non-dynamic oracle with
///   `expected_value_or_variant` present (`typescript_oracle_expected` evidence)
/// - G-D: oracle-eligible relation (not `ambiguous_related_test`; verified by G-A)
/// - G-E: non-empty `missing_discriminators` list (a target shape exists)
/// - G-F: no cross-language bridge limitation evidence present
/// - G-G (#4105): the observed oracle call input must reach — or be statically
///   undecidable against — the named discriminator boundary. A provably
///   non-reaching input keeps the record projected with a boundary placeholder
///   shape but fails the packet closed (`agent_packet` ineligible). An
///   unresolved named-constant boundary fails closed the same way (#4215).
pub(crate) fn typescript_gap_record_for(finding: &Finding) -> Option<GapRecord> {
    // G-A: only the terminal `incomplete_repair_packet` branch is eligible.
    let category = evidence_value(finding, "actionability_category: ")?;
    if category != "incomplete_repair_packet" {
        return None;
    }

    if has_named_typescript_limitation(finding) {
        return None;
    }

    // G-C: non-dynamic oracle: `typescript_oracle_expected` must be present.
    let oracle_expected = evidence_value(finding, "typescript_oracle_expected: ")?;
    if oracle_expected.is_empty() {
        return None;
    }
    // Fail-closed: if any dynamic-assertion limitation was emitted, stay preview.
    if finding
        .evidence
        .iter()
        .any(|line| line == "typescript_limitation: typescript_dynamic_assertion_unresolved")
    {
        return None;
    }

    // G-D: oracle-eligible relation is guaranteed by G-A (ambiguous_related_test
    // is a different category and is excluded above). Verify also that there is
    // actually a related test with an oracle-eligible file we can use.
    let related_test = finding
        .related_tests
        .iter()
        .max_by_key(|t| t.oracle_strength.rank())?;
    let test_file = related_test.file.display().to_string().replace('\\', "/");
    if test_file.is_empty() {
        return None;
    }

    // G-E: the finding must have at least one named missing discriminator
    // (the `missing_target_shape` guard in actionability.rs already passed,
    // but we double-check here so the projection is self-contained).
    if finding.activation.missing_discriminators.is_empty() {
        return None;
    }

    // G-F: no unresolved cross-language oracle visibility limitation.
    if finding.evidence.iter().any(|line| {
        line.starts_with("route_cross_language_oracle_visibility_limitation:")
            || line.starts_with("typescript_bun_bridge_verdict:")
            || line
                .starts_with("typescript_limitation: typescript_cross_language_bridge_unresolved")
    }) {
        return None;
    }

    // Derive `canonical_gap_id` from the finding id (§3.2).
    // The finding id is already content-addressed with path normalized \→/ in
    // `fingerprint_probe_id` — we reuse the last two segments (family:fp8)
    // and prepend `gap:typescript:`.
    // F13: normalize before hashing (the finding id already has / separators,
    // but we defensively strip any remaining backslashes from the id string).
    let canonical_gap_id = typescript_canonical_gap_id(&finding.id);

    // Build verify command from evidence (§3.1 — existing producer).
    let verify_command = evidence_value(finding, "typescript_verify_command: ")?;
    if verify_command.is_empty() {
        return None;
    }

    // Build repair_route from the finding (§3.1 — test file from related test).
    // The route_kind is derived from the probe family / missing discriminator.
    let missing_discriminator = finding
        .activation
        .missing_discriminators
        .first()
        .map(|d| d.value.clone());
    let owner_name = finding
        .probe
        .owner
        .as_ref()
        .and_then(|sym| sym.0.rsplit("::").next())
        .or_else(|| evidence_value(finding, "owner: "))
        .map(ToString::to_string);
    let route_kind = typescript_route_kind_for(&finding.probe.family);
    // Target shape + static reachability of the observed call input against
    // the named discriminator boundary (#4105): a packet must not present a
    // provably non-reaching observed input as the shape for the boundary —
    // an agent following it verbatim would duplicate a non-discriminating
    // assertion.
    let boundary_input = typescript_boundary_input_fact(finding);
    let boundary_parameters = typescript_boundary_parameters_fact(finding);
    let target_shape = evidence_value(finding, "typescript_oracle_observed: ").map(|observed| {
        typescript_target_assertion_shape_with_boundary_facts(
            &finding.probe.family,
            observed,
            missing_discriminator.as_deref(),
            owner_name.as_deref(),
            boundary_input.as_ref(),
            boundary_parameters.as_ref(),
        )
    });
    let assertion_shape = target_shape.as_ref().map(|shape| shape.shape().to_string());
    let mut stop_conditions = vec![
        "Stop if the gap record is no longer present or loses agent-packet eligibility."
            .to_string(),
        "Stop if the verification command cannot run from this workspace.".to_string(),
    ];
    if let Some(boundary_stop) = target_shape
        .as_ref()
        .and_then(TargetAssertionShape::boundary_stop_condition)
    {
        stop_conditions.push(boundary_stop);
    }

    let repair_route = GapRepairRoute {
        route_kind: route_kind.to_string(),
        target_file: Some(test_file.clone()),
        related_test: Some(format!("{test_file}::{}", related_test.name)),
        assertion_shape,
        missing_discriminator,
        changed_behavior: None,
        target_line: if related_test.line > 0 {
            Some(related_test.line as u64)
        } else {
            None
        },
        inspection_command: None,
        stop_conditions,
    };

    // Anchor: probe location + owner from the finding.
    let probe_file = finding
        .probe
        .location
        .file
        .display()
        .to_string()
        .replace('\\', "/");
    let probe_line = finding.probe.location.line as u64;
    let anchor = GapAnchor {
        file: Some(probe_file),
        line: Some(probe_line),
        owner: owner_name,
        dedupe_fingerprint: Some(finding.id.clone()),
    };

    // Projection eligibility: agent_packet eligible through the normal
    // complete-contract path, unless the observed call input provably does
    // not reach the named discriminator boundary (#4105). Then the shared
    // validator fails the packet closed while the placeholder shape still
    // names the boundary — a complete packet must not instruct a duplicate
    // of a non-discriminating assertion.
    let boundary_ineligibility = target_shape
        .as_ref()
        .and_then(TargetAssertionShape::packet_ineligibility_reason);
    let mut projection_eligibility = BTreeMap::new();
    projection_eligibility.insert(
        "agent_packet".to_string(),
        ProjectionEligibility {
            eligible: boundary_ineligibility.is_none(),
            reason: boundary_ineligibility.unwrap_or_else(|| {
                "TypeScript preview complete-contract projection (RIPR-SPEC-0087)".to_string()
            }),
        },
    );

    // Evidence IDs: the finding's own id.
    let evidence_ids = vec![finding.id.clone()];

    let mut record = GapRecord {
        source_currentness: Some(finding.source_currentness.as_str().to_string()),
        gap_id: finding.id.clone(),
        canonical_gap_id,
        seam_id: None,
        kind: "typescript_preview_boundary".to_string(),
        language: "typescript".to_string(),
        language_status: "preview".to_string(),
        scope: "diff".to_string(),
        evidence_class: "weakly_exposed".to_string(),
        gap_state: "advisory".to_string(),
        policy_state: "preview".to_string(),
        repairability: "repairable".to_string(),
        repair_route: Some(repair_route),
        static_limit_kind: None,
        static_limit_detail: None,
        static_limits: Vec::new(),
        anchor: Some(anchor),
        evidence_ids,
        projection_eligibility,
        verification_commands: vec![verify_command.to_string()],
        command_specs: None,
        receipt_command: None,
        regeneration_commands: Vec::new(),
        receipt: None,
        safe_gate_predicate: None,
        authority_boundary: TS_AUTHORITY_BOUNDARY.to_string(),
    };
    // Receipt command (RIPR-SPEC-0079 `canonical_receipt_command` field rule):
    // the canonical `ripr receipt write …` form built by the shared receipt-write
    // owner from this record's canonical gap id, verify command, and default
    // preview receipt path. The gap ledger synthesizes the same string for the
    // same record, so the packet and the ledger agree. `ripr outcome` is a
    // movement command and must not appear here.
    record.receipt_command = Some(preview_receipt_write_command(&record));
    Some(record)
}

/// Derive the content-addressed `gap:typescript:<probe_family>:<fp8>` canonical
/// gap id from the finding's existing content-addressed id (§3.2 / F13).
///
/// The finding id format is `probe:<path>:<family>:<fp8>`. We extract the
/// `<family>` and `<fp8>` segments and build `gap:typescript:<family>:<fp8>`.
/// This avoids introducing a new hash domain and reuses the existing SHA-256 fp8.
pub(crate) fn typescript_canonical_gap_id(finding_id: &str) -> String {
    // finding_id: "probe:src_discount.ts:typescript_preview:1a2b3c4d"
    // Normalize backslashes (defensive, finding ids should already use /).
    let normalized = finding_id.replace('\\', "/");
    // Split on `:` and pick the last two segments as family:fp8.
    let segments: Vec<&str> = normalized.splitn(4, ':').collect();
    if segments.len() == 4 {
        // segments[0]=probe, [1]=path, [2]=family, [3]=fp8
        let family = segments[2];
        let fp8 = segments[3];
        format!("gap:typescript:{family}:{fp8}")
    } else {
        // Fallback: use the whole normalized id as a slug.
        format!("gap:typescript:{normalized}")
    }
}

/// Map the probe family to a `GapRepairRoute` route_kind (§3.2).
///
/// Routes stay consistent with the Rust taxonomy — no new TS-only route kind.
fn typescript_route_kind_for(family: &crate::domain::ProbeFamily) -> &'static str {
    use crate::domain::ProbeFamily;
    match family {
        ProbeFamily::Predicate => "AddBoundaryAssertion",
        ProbeFamily::ReturnValue => "AddValueAssertion",
        ProbeFamily::ErrorPath => "AddErrorDiscriminator",
        ProbeFamily::FieldConstruction => "AddValueAssertion",
        ProbeFamily::SideEffect | ProbeFamily::CallDeletion => "AddBoundaryAssertion",
        ProbeFamily::MatchArm | ProbeFamily::StaticUnknown => "AddBoundaryAssertion",
    }
}

/// Extract a value from the finding's evidence vec by prefix.
fn evidence_value<'a>(finding: &'a Finding, prefix: &str) -> Option<&'a str> {
    finding
        .evidence
        .iter()
        .find_map(|line| line.strip_prefix(prefix))
        .map(str::trim)
        .filter(|v| !v.is_empty())
}

fn has_named_typescript_limitation(finding: &Finding) -> bool {
    finding
        .evidence
        .iter()
        .any(|line| line.starts_with("typescript_limitation: "))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ──────────────────────────────────────────────────────────────────────
    // §7.4 Validator-parity test: proves the flip is driven by the shared
    // validator and not a parallel TypeScript-specific path.
    // ──────────────────────────────────────────────────────────────────────

    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, Finding, LanguageId,
        LanguageStatus, MissingDiscriminatorFact, OracleKind, OracleStrength, OwnerKind, Probe,
        ProbeFamily, ProbeId, RelatedTest, RevealEvidence, RiprEvidence, SourceLocation,
        StageEvidence, StageState, SymbolId,
    };
    use crate::output::agent_seam_packets::{
        gap_record_packet_do_not_do, validate_agent_gap_record_packet,
    };
    use std::path::PathBuf;

    fn complete_finding() -> Finding {
        Finding {
            id: "probe:src_discount.ts:typescript_preview:a1b2c3d4".to_string(),
            canonical_gap: None,
            probe: Probe {
                id: ProbeId("probe:src_discount.ts:typescript_preview:a1b2c3d4".to_string()),
                location: SourceLocation::new("src/discount.ts", 3, 1),
                owner: Some(SymbolId(
                    "typescript:src/discount.ts::applyDiscount".to_string(),
                )),
                family: ProbeFamily::Predicate,
                delta: DeltaKind::Control,
                before: None,
                after: Some("  if (amount >= threshold) {".to_string()),
                expression: "  if (amount >= threshold) {".to_string(),
                expected_sinks: Vec::new(),
                required_oracles: Vec::new(),
            },
            class: ExposureClass::WeaklyExposed,
            ripr: RiprEvidence {
                reach: StageEvidence::new(StageState::Yes, Confidence::Low, "1 related test"),
                infect: StageEvidence::new(
                    StageState::Unknown,
                    Confidence::Low,
                    "TypeScript preview adapter does not yet model infection.",
                ),
                propagate: StageEvidence::new(
                    StageState::Unknown,
                    Confidence::Low,
                    "TypeScript preview adapter does not yet model propagation.",
                ),
                reveal: RevealEvidence {
                    observe: StageEvidence::new(StageState::Weak, Confidence::Low, "weak oracle"),
                    discriminate: StageEvidence::new(
                        StageState::Weak,
                        Confidence::Low,
                        "weak discriminator",
                    ),
                },
            },
            confidence: 0.4,
            evidence: vec![
                "owner: applyDiscount".to_string(),
                "gap_state: advisory".to_string(),
                "actionability_category: incomplete_repair_packet".to_string(),
                "why_not_actionable: TypeScript preview has owner, related-test, oracle, and probe evidence but lacks a complete repair packet contract".to_string(),
                "repair_route: project canonical TypeScript repair packet fields only after verify, receipt, evidence refs, and edit boundaries are available".to_string(),
                "evidence_needed_to_promote: canonical gap identity, repair kind, target test shape, related observer, verify command, receipt command, raw evidence refs, and edit constraints".to_string(),
                "raw_evidence_ref: leg=rust_seam;file=src/discount.ts;line=3;kind=typescript_preview_probe;source_id=probe:src_discount.ts:typescript_preview:a1b2c3d4;owner=applyDiscount".to_string(),
                "typescript_package_root: .".to_string(),
                "typescript_workspace_root: .".to_string(),
                "typescript_framework_hint: jest".to_string(),
                "typescript_runner_hint: npm".to_string(),
                "typescript_package_confidence: high".to_string(),
                "typescript_verify_command: jest tests/discount.test.ts".to_string(),
                "typescript_oracle_observed: applyDiscount(100, 100)".to_string(),
                "typescript_oracle_expected: 90".to_string(),
                "typescript_oracle_confidence: high".to_string(),
                "typescript_oracle_evidence_ref: tests/discount.test.ts:5".to_string(),
                "missing_discriminator: amount >= threshold".to_string(),
                // The analysis side pinned both sides as read-only owner
                // parameters, so `applyDiscount(100, 100)` is shown to hit
                // the boundary (#4759).
                "typescript_boundary_parameters: parameter=amount;index=0;operand=threshold;operand_index=1".to_string(),
            ],
            missing: Vec::new(),
            flow_sinks: Vec::new(),
            activation: ActivationEvidence {
                observed_values: Vec::new(),
                missing_discriminators: vec![MissingDiscriminatorFact {
                    value: "amount >= threshold".to_string(),
                    reason: "changed TypeScript equality-boundary at line 3 lacks a concrete preview discriminator".to_string(),
                    flow_sink: None,
                }],
            },
            stop_reasons: Vec::new(),
            related_tests: vec![RelatedTest {
                name: "applyDiscount applies discount".to_string(),
                file: PathBuf::from("tests/discount.test.ts"),
                line: 4,
                oracle_strength: OracleStrength::Weak,
                oracle_kind: OracleKind::ExactValue,
                oracle: Some("expect(...).toBe(...)".to_string()),
                relation_reason: None,
                relation_confidence: None,
            }],
            recommended_next_step: None,
            language: Some(LanguageId::TypeScript),
            language_status: Some(LanguageStatus::Preview),
            owner_kind: Some(OwnerKind::Function),
            static_limit_kind: None,
            changed_sink: None,
            observed_sink: None,
            oracle_alignment: None,
            alignment_reason: None,
            source_currentness: crate::domain::SourceCurrentness::CandidateCurrent,
        }
    }

    /// §7.4 Validator-parity test (architectural assertion):
    /// The GapRecord produced by `typescript_gap_record_for` for the complete
    /// fixture MUST pass `validate_agent_gap_record_packet`. Any future fork
    /// of the logic would break this test.
    #[test]
    fn validator_parity_complete_finding_passes_shared_validator() {
        let finding = complete_finding();
        let maybe_record = typescript_gap_record_for(&finding);
        assert!(
            maybe_record.is_some(),
            "complete finding must produce a GapRecord"
        );
        let record = match maybe_record {
            Some(r) => r,
            None => return,
        };
        let result = validate_agent_gap_record_packet(&record);
        assert!(
            result.is_ok(),
            "shared validator must accept complete TS GapRecord, got: {:?}",
            result
        );
    }

    /// The target shape keeps the borrowed call but never the borrowed literal:
    /// the borrowed assertion belongs to a weaker check that does not pin the
    /// change, so `toBe(<its literal>)` would be a fabricated expectation.
    #[test]
    fn assertion_shape_keeps_the_call_and_drops_the_borrowed_literal() -> Result<(), String> {
        let finding = complete_finding();
        let record = typescript_gap_record_for(&finding)
            .ok_or_else(|| "complete finding must produce a GapRecord".to_string())?;
        let shape = record
            .repair_route
            .and_then(|route| route.assertion_shape)
            .ok_or_else(|| "complete packet must carry an assertion shape".to_string())?;
        assert_eq!(shape, "expect(applyDiscount(100, 100)).toBe(expected)");
        Ok(())
    }

    /// §7.4: Missing verify command → validator fails (cond. 3), stays preview.
    #[test]
    fn validator_parity_missing_verify_command_returns_none() {
        let mut finding = complete_finding();
        finding
            .evidence
            .retain(|l| !l.starts_with("typescript_verify_command:"));
        let record = typescript_gap_record_for(&finding);
        assert!(
            record.is_none(),
            "missing verify command must return None (stay preview)"
        );
    }

    /// §7.4: Missing oracle expected value → G-C fails, returns None.
    #[test]
    fn validator_parity_missing_oracle_expected_returns_none() {
        let mut finding = complete_finding();
        finding
            .evidence
            .retain(|l| !l.starts_with("typescript_oracle_expected:"));
        let record = typescript_gap_record_for(&finding);
        assert!(
            record.is_none(),
            "missing oracle expected value must return None (G-C)"
        );
    }

    /// §7.4: Dynamic oracle limitation → G-C fails, returns None.
    #[test]
    fn validator_parity_dynamic_oracle_returns_none() {
        let mut finding = complete_finding();
        finding
            .evidence
            .push("typescript_limitation: typescript_dynamic_assertion_unresolved".to_string());
        let record = typescript_gap_record_for(&finding);
        assert!(
            record.is_none(),
            "dynamic oracle limitation must return None (G-C)"
        );
    }

    /// Named TypeScript limitations fail closed before packet validation.
    #[test]
    fn validator_parity_named_typescript_limitation_returns_none() {
        let mut finding = complete_finding();
        finding
            .evidence
            .push("typescript_limitation: typescript_custom_matcher_unresolved".to_string());
        let record = typescript_gap_record_for(&finding);
        assert!(
            record.is_none(),
            "named TypeScript limitation must return None before packet validation"
        );
    }

    /// §7.4: Wrong category → G-A fails, returns None.
    #[test]
    fn validator_parity_wrong_category_returns_none() {
        let mut finding = complete_finding();
        for line in finding.evidence.iter_mut() {
            if line.starts_with("actionability_category: ") {
                *line = "actionability_category: strong_oracle_observed".to_string();
            }
        }
        let record = typescript_gap_record_for(&finding);
        assert!(
            record.is_none(),
            "non-incomplete-repair-packet category must return None (G-A)"
        );
    }

    /// §7.4: No related tests → projection fails, returns None.
    #[test]
    fn validator_parity_no_related_tests_returns_none() {
        let mut finding = complete_finding();
        finding.related_tests.clear();
        let record = typescript_gap_record_for(&finding);
        assert!(record.is_none(), "no related tests must return None (G-D)");
    }

    /// §7.4: Empty missing_discriminators → G-E fails, returns None.
    #[test]
    fn validator_parity_no_missing_discriminators_returns_none() {
        let mut finding = complete_finding();
        finding.activation.missing_discriminators.clear();
        let record = typescript_gap_record_for(&finding);
        assert!(
            record.is_none(),
            "empty missing_discriminators must return None (G-E)"
        );
    }

    /// §7.4: Cross-language bridge limitation → G-F fails, returns None.
    #[test]
    fn validator_parity_cross_language_bridge_returns_none() {
        let mut finding = complete_finding();
        finding
            .evidence
            .push("typescript_limitation: typescript_cross_language_bridge_unresolved".to_string());
        let record = typescript_gap_record_for(&finding);
        assert!(
            record.is_none(),
            "cross-language bridge limitation must return None (G-F)"
        );
    }

    #[test]
    fn canonical_gap_id_derives_from_finding_id() {
        let id = "probe:src_discount.ts:typescript_preview:a1b2c3d4";
        let gap_id = typescript_canonical_gap_id(id);
        assert_eq!(gap_id, "gap:typescript:typescript_preview:a1b2c3d4");
    }

    #[test]
    fn canonical_gap_id_normalizes_backslashes() {
        let id = r"probe:src\discount.ts:typescript_preview:a1b2c3d4";
        let gap_id = typescript_canonical_gap_id(id);
        // After backslash normalization, result still starts with gap:typescript:
        assert!(gap_id.starts_with("gap:typescript:"));
    }

    /// RIPR-SPEC-0079 `canonical_receipt_command` field rule: the projected
    /// record's `receipt_command` is the canonical `ripr receipt write …`
    /// form carrying the record's canonical gap id, its verify command, and
    /// the ledger's default preview receipt path — never `ripr outcome`.
    #[test]
    fn projected_receipt_command_is_canonical_receipt_write() -> Result<(), String> {
        let record = typescript_gap_record_for(&complete_finding())
            .ok_or("complete finding must produce a GapRecord")?;
        let cmd = record
            .receipt_command
            .as_deref()
            .ok_or("projected record must carry a receipt command")?;
        assert_eq!(
            cmd,
            "ripr receipt write --gap gap:typescript:typescript_preview:a1b2c3d4 \
             --verify-command 'jest tests/discount.test.ts' --status not_run \
             --out target/ripr/receipts/gap-typescript-typescript_preview-a1b2c3d4.json"
        );
        assert!(
            !cmd.contains("ripr outcome"),
            "receipt_command must not be a movement command: {cmd}"
        );
        assert!(!cmd.contains("curl"), "F7: must not contain curl");
        assert!(!cmd.contains("http"), "F7: must not contain http");
        Ok(())
    }

    /// §3.2 / F14: The shared `gap_record_packet_do_not_do` function must include
    /// the preview-language clause when `language_status == "preview"`.
    /// This proves the TS packet reuses the shared boundary list (not a fork).
    #[test]
    fn must_not_change_includes_preview_clause_via_shared_function() {
        let finding = complete_finding();
        let maybe_record = typescript_gap_record_for(&finding);
        assert!(
            maybe_record.is_some(),
            "complete finding must produce a GapRecord for do_not_do test"
        );
        let record = match maybe_record {
            Some(r) => r,
            None => return,
        };
        let do_not_do = gap_record_packet_do_not_do(&record);
        let has_preview_clause = do_not_do
            .iter()
            .any(|s| s.contains("preview-language evidence"));
        assert!(
            has_preview_clause,
            "F14: gap_record_packet_do_not_do must include preview clause for language_status=preview; got: {do_not_do:?}"
        );
    }

    // ──────────────────────────────────────────────────────────────────────
    // #4105: observed-input vs discriminator-boundary reachability.
    // A complete packet must not present an observed call input that provably
    // cannot reach the named boundary as the shape for that boundary.
    // ──────────────────────────────────────────────────────────────────────

    /// A complete finding shaped like the #4105 repro: owner `login` with the
    /// changed boundary `user.length >= 3`, a related test asserting on
    /// `login(<observed>)`, and the missing discriminator naming the boundary.
    fn boundary_finding(observed: &str, expected: &str, discriminator: &str) -> Finding {
        let mut finding = complete_finding();
        finding.id = "probe:src_auth.ts:typescript_preview:23e9836b".to_string();
        finding
            .evidence
            .retain(|line| !line.starts_with("typescript_boundary_parameters: "));
        finding.probe.owner = Some(SymbolId("typescript:src/auth.ts::login".to_string()));
        for line in finding.evidence.iter_mut() {
            if line.starts_with("typescript_oracle_observed: ") {
                *line = format!("typescript_oracle_observed: {observed}");
            } else if line.starts_with("typescript_oracle_expected: ") {
                *line = format!("typescript_oracle_expected: {expected}");
            } else if line.starts_with("missing_discriminator: ") {
                *line = format!("missing_discriminator: {discriminator}");
            }
        }
        finding.activation.missing_discriminators[0].value = discriminator.to_string();
        finding
    }

    /// Wrongfam: the observed input `'alice'` has length 5, so it provably
    /// does NOT reach the `user.length == 3` boundary. The shape must become
    /// an explicit boundary placeholder and the packet must fail closed.
    #[test]
    fn observed_input_not_reaching_boundary_downgrades_to_placeholder_and_fails_closed()
    -> Result<(), String> {
        let finding = boundary_finding("login('alice')", "'session-for-alice'", "user.length == 3");
        let record = typescript_gap_record_for(&finding)
            .ok_or_else(|| "record must still project for the wrongfam finding".to_string())?;
        let route = record
            .repair_route
            .as_ref()
            .ok_or_else(|| "repair route must be present".to_string())?;
        let shape = route
            .assertion_shape
            .as_deref()
            .ok_or_else(|| "shape must be present".to_string())?;
        assert_eq!(
            shape, "expect(login(/* boundary input for user.length == 3 */)).toBe(expected)",
            "shape must name the boundary, not the observed input"
        );
        assert!(
            !shape.contains("'alice'"),
            "placeholder shape must not reuse the observed input: {shape}"
        );
        assert!(
            route.stop_conditions.iter().any(|stop| {
                stop.contains("Do not reuse the observed call input")
                    && stop.contains("user.length == 3")
            }),
            "a stop condition must forbid reusing the observed input: {:?}",
            route.stop_conditions
        );
        let error = match validate_agent_gap_record_packet(&record) {
            Err(error) => error,
            Ok(()) => {
                return Err(
                    "packet must fail closed when the observed input cannot reach the boundary"
                        .to_string(),
                );
            }
        };
        assert!(
            error.contains("user.length == 3") && error.contains("login('alice')"),
            "validator reason must name the boundary and the observed input: {error}"
        );
        Ok(())
    }

    /// Hon-style control: an observed input that DOES hit the boundary
    /// (`'abc'` has length 3) keeps today's derived shape and the packet
    /// stays delegatable through the shared validator.
    #[test]
    fn observed_input_hitting_boundary_keeps_observed_shape_and_validator_pass()
    -> Result<(), String> {
        let finding = boundary_finding("login('abc')", "'session-for-alice'", "user.length == 3");
        let record =
            typescript_gap_record_for(&finding).ok_or_else(|| "record must project".to_string())?;
        let route = record
            .repair_route
            .as_ref()
            .ok_or_else(|| "repair route must be present".to_string())?;
        assert_eq!(
            route.assertion_shape.as_deref(),
            Some("expect(login('abc')).toBe(expected)"),
            "boundary-hitting observed input keeps the derived shape"
        );
        if let Err(error) = validate_agent_gap_record_packet(&record) {
            return Err(format!(
                "boundary-hitting input keeps the packet delegatable; got: {error}"
            ));
        }
        Ok(())
    }

    /// Multi-argument call: without signature evidence the argument binding
    /// would be a guess, so the observed shape stands (undecided, fail-open).
    #[test]
    fn undecidable_argument_binding_keeps_observed_shape_and_validator_pass() -> Result<(), String>
    {
        let finding = boundary_finding(
            "login('alice', true)",
            "'session-for-alice'",
            "user.length == 3",
        );
        let record =
            typescript_gap_record_for(&finding).ok_or_else(|| "record must project".to_string())?;
        let route = record
            .repair_route
            .as_ref()
            .ok_or_else(|| "repair route must be present".to_string())?;
        assert_eq!(
            route.assertion_shape.as_deref(),
            Some("expect(login('alice', true)).toBe(expected)")
        );
        if let Err(error) = validate_agent_gap_record_packet(&record) {
            return Err(format!(
                "undecided reachability must not block the packet; got: {error}"
            ));
        }
        Ok(())
    }

    /// A callee that does not resolve to the owner is never judged.
    #[test]
    fn non_owner_callee_is_not_judged_for_reachability() -> Result<(), String> {
        let finding =
            boundary_finding("helper('alice')", "'session-for-alice'", "user.length == 3");
        let record =
            typescript_gap_record_for(&finding).ok_or_else(|| "record must project".to_string())?;
        let route = record
            .repair_route
            .as_ref()
            .ok_or_else(|| "repair route must be present".to_string())?;
        assert_eq!(
            route.assertion_shape.as_deref(),
            Some("expect(helper('alice')).toBe(expected)")
        );
        if let Err(error) = validate_agent_gap_record_packet(&record) {
            return Err(format!(
                "a non-owner callee must not be judged for reachability; got: {error}"
            ));
        }
        Ok(())
    }

    #[test]
    fn target_shape_numeric_boundary_miss_downgrades_to_placeholder() {
        let shape = typescript_target_assertion_shape(
            &ProbeFamily::Predicate,
            "limit(5)",
            Some("amount == 3"),
            Some("limit"),
        );
        assert_eq!(
            shape,
            TargetAssertionShape::Unreachable {
                shape: "expect(limit(/* boundary input for amount == 3 */)).toBe(expected)"
                    .to_string(),
                observed_call: "limit(5)".to_string(),
                discriminator: "amount == 3".to_string(),
            }
        );
    }

    #[test]
    fn target_shape_numeric_boundary_hit_keeps_observed() {
        let shape = typescript_target_assertion_shape(
            &ProbeFamily::Predicate,
            "limit(3)",
            Some("amount == 3"),
            Some("limit"),
        );
        assert_eq!(
            shape,
            TargetAssertionShape::Observed {
                shape: "expect(limit(3)).toBe(expected)".to_string(),
            }
        );
    }

    #[test]
    fn target_shape_string_equality_boundary_respects_operator() {
        let miss = typescript_target_assertion_shape(
            &ProbeFamily::Predicate,
            "greet('bob')",
            Some("name == 'admin'"),
            Some("greet"),
        );
        assert!(matches!(miss, TargetAssertionShape::Unreachable { .. }));
        let hit = typescript_target_assertion_shape(
            &ProbeFamily::Predicate,
            "greet('admin')",
            Some("name == 'admin'"),
            Some("greet"),
        );
        assert!(matches!(hit, TargetAssertionShape::Observed { .. }));
        // `!=` flips both verdicts.
        let ne_miss = typescript_target_assertion_shape(
            &ProbeFamily::Predicate,
            "greet('admin')",
            Some("name != 'admin'"),
            Some("greet"),
        );
        assert!(matches!(ne_miss, TargetAssertionShape::Unreachable { .. }));
        let ne_hit = typescript_target_assertion_shape(
            &ProbeFamily::Predicate,
            "greet('bob')",
            Some("name != 'admin'"),
            Some("greet"),
        );
        assert!(matches!(ne_hit, TargetAssertionShape::Observed { .. }));
    }

    #[test]
    fn target_shape_length_comparison_operators_are_evaluated() {
        // `'ab'.length == 3` is false → downgrade.
        let miss = typescript_target_assertion_shape(
            &ProbeFamily::Predicate,
            "pad('ab')",
            Some("user.length == 3"),
            Some("pad"),
        );
        assert!(matches!(miss, TargetAssertionShape::Unreachable { .. }));
        // `'abcd'.length < 3` is false → downgrade.
        let miss_lt = typescript_target_assertion_shape(
            &ProbeFamily::Predicate,
            "pad('abcd')",
            Some("user.length < 3"),
            Some("pad"),
        );
        assert!(matches!(miss_lt, TargetAssertionShape::Unreachable { .. }));
        // `'ab'.length < 3` is true → keep the observed shape.
        let hit_lt = typescript_target_assertion_shape(
            &ProbeFamily::Predicate,
            "pad('ab')",
            Some("user.length < 3"),
            Some("pad"),
        );
        assert!(matches!(hit_lt, TargetAssertionShape::Observed { .. }));
    }

    #[test]
    fn target_shape_escaped_literal_is_undecided() {
        // Escape sequences make the static length unreliable — fail open.
        let shape = typescript_target_assertion_shape(
            &ProbeFamily::Predicate,
            "login('a\\t')",
            Some("user.length == 3"),
            Some("login"),
        );
        assert!(
            matches!(shape, TargetAssertionShape::Observed { .. }),
            "escaped literals must stay undecided: {shape:?}"
        );
    }

    #[test]
    fn target_shape_without_discriminator_or_owner_keeps_observed() {
        let no_discriminator = typescript_target_assertion_shape(
            &ProbeFamily::Predicate,
            "login('alice')",
            None,
            Some("login"),
        );
        assert!(matches!(
            no_discriminator,
            TargetAssertionShape::Observed { .. }
        ));
        let no_owner = typescript_target_assertion_shape(
            &ProbeFamily::Predicate,
            "login('alice')",
            Some("user.length == 3"),
            None,
        );
        assert!(matches!(no_owner, TargetAssertionShape::Observed { .. }));
        // A plain-identifier boundary (`amount >= threshold`) without the
        // parameter-pair fact is not shown to be bound by the observed
        // arguments, so it fails closed instead of standing (#4759).
        let non_literal = typescript_target_assertion_shape(
            &ProbeFamily::Predicate,
            "applyDiscount(100, 100)",
            Some("amount >= threshold"),
            Some("applyDiscount"),
        );
        assert!(matches!(
            non_literal,
            TargetAssertionShape::UnboundOperand { .. }
        ));
    }

    #[test]
    fn compound_expression_is_not_one_static_call() {
        // `login('a') + login('b')` and `login('ab')('c')` must not parse as a
        // single static call, or reachability would be judged on fabricated
        // arguments (review: reject compounds before static reachability).
        for expr in ["login('a') + login('b')", "login('ab')('c')"] {
            assert!(
                parse_static_call_expression(expr).is_none(),
                "compound expression must be refused: {expr}"
            );
        }
    }

    #[test]
    fn string_literal_with_inner_matching_quote_is_undecided() {
        // `login('ab' + 'c')`: the argument is not a plain string literal, so
        // its length must not be judged statically.
        let shape = typescript_target_assertion_shape(
            &ProbeFamily::Predicate,
            "login('ab' + 'c')",
            Some("user.length == 3"),
            Some("login"),
        );
        assert!(
            matches!(shape, TargetAssertionShape::Observed { .. }),
            "non-literal argument must stay undecided: {shape:?}"
        );
    }

    #[test]
    fn string_length_uses_utf16_code_units() {
        // JavaScript `String.prototype.length` counts UTF-16 code units: an
        // astral character (e.g. emoji) counts as 2, so `login('a😀')` has
        // length 3 and hits a `user.length == 3` boundary.
        let shape = typescript_target_assertion_shape(
            &ProbeFamily::Predicate,
            "login('a😀')",
            Some("user.length == 3"),
            Some("login"),
        );
        assert!(
            matches!(shape, TargetAssertionShape::Observed { .. }),
            "UTF-16 length 3 must reach the boundary: {shape:?}"
        );
    }

    // ──────────────────────────────────────────────────────────────────────
    // #4215: an unresolved named-constant boundary is not evidence that the
    // observed input discriminates it; the packet fails closed.
    // ──────────────────────────────────────────────────────────────────────

    /// The onboarding `tsapp` repro: `amount >= DISCOUNT_THRESHOLD` changed,
    /// the only related assertion observes `discountedTotal(20000)`, and the
    /// missing discriminator names the constant. The packet must not be
    /// delegatable, and its shape must name the boundary, not `20000`.
    #[test]
    fn unresolved_constant_boundary_fails_closed_with_boundary_placeholder() -> Result<(), String> {
        let mut finding = boundary_finding(
            "discountedTotal(20000)",
            "18000",
            "amount == DISCOUNT_THRESHOLD",
        );
        finding.probe.owner = Some(SymbolId(
            "typescript:src/pricing.ts::discountedTotal".to_string(),
        ));
        let record = typescript_gap_record_for(&finding).ok_or_else(|| {
            "record must still project for the named-constant finding".to_string()
        })?;
        let route = record
            .repair_route
            .as_ref()
            .ok_or_else(|| "repair route must be present".to_string())?;
        assert_eq!(
            route.assertion_shape.as_deref(),
            Some(
                "expect(discountedTotal(/* boundary input for amount == DISCOUNT_THRESHOLD */)).toBe(expected)"
            ),
            "shape must name the boundary, not the observed input"
        );
        assert!(
            route.stop_conditions.iter().any(|stop| {
                stop.contains("Do not reuse the observed call input")
                    && stop.contains("DISCOUNT_THRESHOLD")
            }),
            "a stop condition must forbid reusing the observed input: {:?}",
            route.stop_conditions
        );
        let error = match validate_agent_gap_record_packet(&record) {
            Err(error) => error,
            Ok(()) => {
                return Err(
                    "packet must fail closed when the boundary constant is unresolved".to_string(),
                );
            }
        };
        assert!(
            error.contains("DISCOUNT_THRESHOLD") && error.contains("discountedTotal(20000)"),
            "validator reason must name the constant and the observed input: {error}"
        );
        Ok(())
    }

    /// The same finding with a literal threshold that the observed input
    /// hits stays complete: the fail-closed arm is specific to unresolved
    /// constants, not to every `>=`/`==` boundary.
    #[test]
    fn literal_boundary_hit_by_observed_input_stays_delegatable() -> Result<(), String> {
        let mut finding = boundary_finding("discountedTotal(10000)", "9000", "amount == 10000");
        finding.probe.owner = Some(SymbolId(
            "typescript:src/pricing.ts::discountedTotal".to_string(),
        ));
        let record =
            typescript_gap_record_for(&finding).ok_or_else(|| "record must project".to_string())?;
        let route = record
            .repair_route
            .as_ref()
            .ok_or_else(|| "repair route must be present".to_string())?;
        assert_eq!(
            route.assertion_shape.as_deref(),
            Some("expect(discountedTotal(10000)).toBe(expected)")
        );
        validate_agent_gap_record_packet(&record)
            .map_err(|error| format!("literal boundary hit must stay delegatable; got: {error}"))
    }

    /// A literal written first is read with the operator mirrored:
    /// `3 < amount` is `amount > 3`.
    #[test]
    fn target_shape_literal_first_mirrors_the_operator() {
        let hit = typescript_target_assertion_shape(
            &ProbeFamily::Predicate,
            "limit(5)",
            Some("3 < amount"),
            Some("limit"),
        );
        assert!(
            matches!(hit, TargetAssertionShape::Observed { .. }),
            "{hit:?}"
        );
        let miss = typescript_target_assertion_shape(
            &ProbeFamily::Predicate,
            "limit(2)",
            Some("3 < amount"),
            Some("limit"),
        );
        assert!(
            matches!(miss, TargetAssertionShape::Unreachable { .. }),
            "{miss:?}"
        );
    }

    #[test]
    fn target_shape_constant_boundary_is_judged_at_any_arity_unless_an_argument_names_it() {
        for (observed, discriminator) in [
            ("discountedTotal(20000)", "amount == DISCOUNT_THRESHOLD"),
            ("discountedTotal(20000)", "amount >= DISCOUNT_THRESHOLD"),
            (
                "discountedTotal(20000, true)",
                "amount == DISCOUNT_THRESHOLD",
            ),
            ("pad('abc')", "user.length == MIN_LEN"),
        ] {
            let owner = observed.split('(').next();
            let shape = typescript_target_assertion_shape(
                &ProbeFamily::Predicate,
                observed,
                Some(discriminator),
                owner,
            );
            assert!(
                matches!(shape, TargetAssertionShape::UnresolvedBoundary { .. }),
                "`{observed}` vs `{discriminator}` must fail closed: {shape:?}"
            );
        }
        // A constant written first is still the boundary (the analysis side
        // keeps operand order: `DISCOUNT_THRESHOLD <= amount` becomes
        // `DISCOUNT_THRESHOLD == amount`).
        let constant_first = typescript_target_assertion_shape(
            &ProbeFamily::Predicate,
            "discountedTotal(20000)",
            Some("DISCOUNT_THRESHOLD == amount"),
            Some("discountedTotal"),
        );
        assert!(
            matches!(
                constant_first,
                TargetAssertionShape::UnresolvedBoundary { ref constant, .. }
                    if constant == "DISCOUNT_THRESHOLD"
            ),
            "a leading constant boundary must fail closed: {constant_first:?}"
        );
        // An argument that IS the constant may hit the boundary: undecided.
        let names_constant = typescript_target_assertion_shape(
            &ProbeFamily::Predicate,
            "discountedTotal(DISCOUNT_THRESHOLD)",
            Some("amount == DISCOUNT_THRESHOLD"),
            Some("discountedTotal"),
        );
        assert!(
            matches!(names_constant, TargetAssertionShape::Observed { .. }),
            "an argument naming the constant stays undecided: {names_constant:?}"
        );
    }

    fn boundary_input_fact(parameter: &str, index: usize, operand: &str, value: i64) -> String {
        format!(
            "typescript_boundary_input: parameter={parameter};index={index};operand={operand};value={value}"
        )
    }

    fn parsed_fact(line: &str) -> Result<TypeScriptBoundaryInputFact, String> {
        TypeScriptBoundaryInputFact::parse(line).ok_or_else(|| format!("fact must parse: {line}"))
    }

    /// #4410 / RC rehearsal: a literal boundary the observed input misses
    /// (`shipping(1000)` vs `amount == 5000`) projects the derived boundary
    /// input and stays delegatable through the shared validator; the observed
    /// input stays as context in a stop condition.
    #[test]
    fn missed_literal_boundary_with_input_fact_derives_the_boundary_call() -> Result<(), String> {
        let mut finding = boundary_finding("shipping(1000)", "500", "amount == 5000");
        finding.probe.owner = Some(SymbolId(
            "typescript:src/shipping.mts::shipping".to_string(),
        ));
        // Control: without the fact the packet fails closed (#4105).
        let closed =
            typescript_gap_record_for(&finding).ok_or_else(|| "record must project".to_string())?;
        assert!(validate_agent_gap_record_packet(&closed).is_err());

        finding
            .evidence
            .push(boundary_input_fact("amount", 0, "5000", 5000));
        let record =
            typescript_gap_record_for(&finding).ok_or_else(|| "record must project".to_string())?;
        let route = record
            .repair_route
            .as_ref()
            .ok_or_else(|| "repair route must be present".to_string())?;
        assert_eq!(
            route.assertion_shape.as_deref(),
            Some("expect(shipping(5000)).toBe(expected)")
        );
        assert!(
            route.stop_conditions.iter().any(|stop| stop
                .contains("The observed call `shipping(1000)` is not shown to hit")
                && stop.contains("`shipping(5000)` is derived from the literal boundary")),
            "{:?}",
            route.stop_conditions
        );
        validate_agent_gap_record_packet(&record)
            .map_err(|error| format!("derived literal boundary must be delegatable: {error}"))
    }

    /// The onboarding `tsapp` shape: the constant resolved on the analysis
    /// side turns the #4215 fail-closed packet into a delegatable one.
    #[test]
    fn resolved_constant_boundary_with_input_fact_is_delegatable() -> Result<(), String> {
        let mut finding = boundary_finding(
            "discountedTotal(20000)",
            "18000",
            "amount == DISCOUNT_THRESHOLD",
        );
        finding.probe.owner = Some(SymbolId(
            "typescript:src/pricing.ts::discountedTotal".to_string(),
        ));
        finding.evidence.push(boundary_input_fact(
            "amount",
            0,
            "DISCOUNT_THRESHOLD",
            10000,
        ));
        let record =
            typescript_gap_record_for(&finding).ok_or_else(|| "record must project".to_string())?;
        let route = record
            .repair_route
            .as_ref()
            .ok_or_else(|| "repair route must be present".to_string())?;
        assert_eq!(
            route.assertion_shape.as_deref(),
            Some("expect(discountedTotal(10000)).toBe(expected)")
        );
        assert!(
            route
                .stop_conditions
                .iter()
                .any(|stop| stop.contains("derived from `DISCOUNT_THRESHOLD` = 10000")),
            "{:?}",
            route.stop_conditions
        );
        validate_agent_gap_record_packet(&record)
            .map_err(|error| format!("resolved constant must be delegatable: {error}"))
    }

    #[test]
    fn input_fact_binds_its_argument_position_and_both_operand_orders() -> Result<(), String> {
        let rate_fact = parsed_fact(&boundary_input_fact("amount", 1, "100", 100))?;
        let shape = typescript_target_assertion_shape_with_boundary_input(
            &ProbeFamily::Predicate,
            "applyRate(0.1, 50)",
            Some("amount == 100"),
            Some("applyRate"),
            Some(&rate_fact),
        );
        assert_eq!(shape.shape(), "expect(applyRate(0.1, 100)).toBe(expected)");
        assert!(shape.packet_ineligibility_reason().is_none());

        let constant_fact = parsed_fact(&boundary_input_fact("amount", 0, "LIMIT", 7))?;
        let first = typescript_target_assertion_shape_with_boundary_input(
            &ProbeFamily::Predicate,
            "check(1)",
            Some("LIMIT == amount"),
            Some("check"),
            Some(&constant_fact),
        );
        assert_eq!(first.shape(), "expect(check(7)).toBe(expected)");
        Ok(())
    }

    /// Codex on #4429: the analysis side reads `5_000` as 5000, so the
    /// projection must too, or the literal route never becomes delegatable.
    #[test]
    fn separator_literal_boundary_derives_the_boundary_call() -> Result<(), String> {
        let fact = parsed_fact(&boundary_input_fact("amount", 0, "5_000", 5000))?;
        let shape = typescript_target_assertion_shape_with_boundary_input(
            &ProbeFamily::Predicate,
            "shipping(1_000)",
            Some("amount == 5_000"),
            Some("shipping"),
            Some(&fact),
        );
        assert_eq!(shape.shape(), "expect(shipping(5000)).toBe(expected)");
        assert!(shape.packet_ineligibility_reason().is_none());
        for malformed in ["5__000", "5000_", "_5000", "0_5"] {
            assert_eq!(parse_integer_literal(malformed), None, "{malformed}");
        }
        Ok(())
    }

    /// An escaped quote no longer ends the string when splitting arguments.
    #[test]
    fn argument_split_keeps_escaped_quotes_inside_their_string() {
        assert_eq!(
            split_top_level_args(r#""a\",b", 50"#),
            vec![r#""a\",b""#.to_string(), "50".to_string()]
        );
    }

    /// An observed input that already hits the boundary keeps its own shape.
    #[test]
    fn input_fact_never_overrides_an_observed_input_that_hits() -> Result<(), String> {
        let fact = parsed_fact(&boundary_input_fact("amount", 0, "5000", 5000))?;
        let shape = typescript_target_assertion_shape_with_boundary_input(
            &ProbeFamily::Predicate,
            "shipping(5000)",
            Some("amount == 5000"),
            Some("shipping"),
            Some(&fact),
        );
        assert!(
            matches!(shape, TargetAssertionShape::Observed { .. }),
            "{shape:?}"
        );
        Ok(())
    }

    /// A fact that does not name exactly this comparison, or cannot bind the
    /// observed call, leaves the fail-closed verdict in place.
    #[test]
    fn mismatched_input_facts_keep_the_packet_closed() -> Result<(), String> {
        for (label, observed, discriminator, fact) in [
            (
                "other parameter",
                "shipping(1000)",
                "amount == 5000",
                boundary_input_fact("total", 0, "5000", 5000),
            ),
            (
                "other literal value",
                "shipping(1000)",
                "amount == 5000",
                boundary_input_fact("amount", 0, "4000", 4000),
            ),
            (
                "operand text differs from value",
                "shipping(1000)",
                "amount == 5000",
                boundary_input_fact("amount", 0, "LIMIT", 5000),
            ),
            (
                "other constant",
                "discountedTotal(20000)",
                "amount == DISCOUNT_THRESHOLD",
                boundary_input_fact("amount", 0, "OTHER_LIMIT", 10000),
            ),
            (
                "position beyond the observed arguments",
                "shipping(1000)",
                "amount == 5000",
                boundary_input_fact("amount", 1, "5000", 5000),
            ),
            (
                "spread argument",
                "shipping(...orders)",
                "amount == 5000",
                boundary_input_fact("amount", 0, "5000", 5000),
            ),
            (
                "length receiver",
                "login('alice')",
                "user.length == 3",
                boundary_input_fact("user", 0, "3", 3),
            ),
            (
                "escaped quote in another argument",
                r#"apply("a\",b", 50)"#,
                "amount == 100",
                boundary_input_fact("amount", 1, "100", 100),
            ),
        ] {
            let fact = parsed_fact(&fact)?;
            let owner = observed.split('(').next();
            let shape = typescript_target_assertion_shape_with_boundary_input(
                &ProbeFamily::Predicate,
                observed,
                Some(discriminator),
                owner,
                Some(&fact),
            );
            assert!(
                !matches!(shape, TargetAssertionShape::DerivedBoundary { .. }),
                "{label}: {shape:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn malformed_or_duplicate_input_facts_are_ignored() {
        for line in [
            "typescript_boundary_input: parameter=amount;index=0;operand=5000",
            "typescript_boundary_input: parameter=amount;index=x;operand=5000;value=5000",
            "typescript_boundary_input: parameter=amount;index=0;operand=5000;value=5.5",
            "typescript_boundary_input: parameter=a b;index=0;operand=5000;value=5000",
            "typescript_boundary_input: parameter=amount;index=0;operand=5000;value=5000;extra=1",
            "typescript_boundary_input: parameter=amount;parameter=total;index=0;operand=5000",
        ] {
            assert!(TypeScriptBoundaryInputFact::parse(line).is_none(), "{line}");
        }
        let mut finding = boundary_finding("shipping(1000)", "500", "amount == 5000");
        finding
            .evidence
            .push(boundary_input_fact("amount", 0, "5000", 5000));
        finding
            .evidence
            .push(boundary_input_fact("amount", 0, "5000", 5000));
        assert!(typescript_boundary_input_fact(&finding).is_none());
    }

    fn boundary_parameters_fact(
        parameter: &str,
        index: usize,
        operand: &str,
        operand_index: usize,
    ) -> String {
        format!(
            "typescript_boundary_parameters: parameter={parameter};index={index};operand={operand};operand_index={operand_index}"
        )
    }

    fn parsed_parameters_fact(line: &str) -> Result<TypeScriptBoundaryParametersFact, String> {
        TypeScriptBoundaryParametersFact::parse(line)
            .ok_or_else(|| format!("parameters fact must parse: {line}"))
    }

    fn parameter_pair_shape(
        observed: &str,
        discriminator: &str,
        fact: Option<&TypeScriptBoundaryParametersFact>,
    ) -> TargetAssertionShape {
        typescript_target_assertion_shape_with_boundary_facts(
            &ProbeFamily::Predicate,
            observed,
            Some(discriminator),
            Some("discount"),
            None,
            fact,
        )
    }

    /// #4759 repro: `discount(50, 100)` cannot tell `>` from `>=` at
    /// `amount == threshold`. Without the analysis-side parameter-pair fact
    /// the packet fails closed; with it, the observed literals decide Misses
    /// and the receiver takes the boundary's argument.
    #[test]
    fn missed_parameter_pair_boundary_derives_the_boundary_call() -> Result<(), String> {
        let mut finding = boundary_finding("discount(50, 100)", "0", "amount == threshold");
        finding.probe.owner = Some(SymbolId("typescript:src/pricing.ts::discount".to_string()));
        let closed =
            typescript_gap_record_for(&finding).ok_or_else(|| "record must project".to_string())?;
        let closed_route = closed
            .repair_route
            .as_ref()
            .ok_or_else(|| "repair route must be present".to_string())?;
        assert_eq!(
            closed_route.assertion_shape.as_deref(),
            Some("expect(discount(/* boundary input for amount == threshold */)).toBe(expected)")
        );
        assert!(
            closed_route
                .stop_conditions
                .iter()
                .any(|stop| stop.starts_with("Do not reuse the observed call input")),
            "{:?}",
            closed_route.stop_conditions
        );
        assert!(validate_agent_gap_record_packet(&closed).is_err());

        finding
            .evidence
            .push(boundary_parameters_fact("amount", 0, "threshold", 1));
        let record =
            typescript_gap_record_for(&finding).ok_or_else(|| "record must project".to_string())?;
        let route = record
            .repair_route
            .as_ref()
            .ok_or_else(|| "repair route must be present".to_string())?;
        assert_eq!(
            route.assertion_shape.as_deref(),
            Some("expect(discount(100, 100)).toBe(expected)")
        );
        assert!(
            route.stop_conditions.iter().any(|stop| stop
                .contains("The observed call `discount(50, 100)` is not shown to hit")
                && stop.contains("derived from `threshold` = 100")),
            "{:?}",
            route.stop_conditions
        );
        validate_agent_gap_record_packet(&record)
            .map_err(|error| format!("derived parameter boundary must be delegatable: {error}"))
    }

    #[test]
    fn parameter_pair_fact_decides_hits_and_binds_either_operand_order() -> Result<(), String> {
        let fact = parsed_parameters_fact(&boundary_parameters_fact("amount", 0, "threshold", 1))?;
        let hits = parameter_pair_shape("discount(100, 100)", "amount == threshold", Some(&fact));
        assert_eq!(
            hits,
            TargetAssertionShape::Observed {
                shape: "expect(discount(100, 100)).toBe(expected)".to_string()
            }
        );
        let separators =
            parameter_pair_shape("discount(1_000, 1000)", "amount == threshold", Some(&fact));
        assert!(
            matches!(separators, TargetAssertionShape::Observed { .. }),
            "{separators:?}"
        );
        // `threshold == amount` reads `threshold` as the receiver, so the
        // derivation gives it the value bound to `amount`.
        let reversed =
            parameter_pair_shape("discount(50, 100)", "threshold == amount", Some(&fact));
        assert_eq!(reversed.shape(), "expect(discount(50, 50)).toBe(expected)");
        assert!(reversed.packet_ineligibility_reason().is_none());
        Ok(())
    }

    /// A parameter-pair fact that does not name this comparison, or observed
    /// arguments that are not both integer literals, keep the packet closed.
    #[test]
    fn unbound_parameter_pair_boundaries_keep_the_packet_closed() -> Result<(), String> {
        let fact = parsed_parameters_fact(&boundary_parameters_fact("amount", 0, "threshold", 1))?;
        let other = parsed_parameters_fact(&boundary_parameters_fact("amount", 0, "limit", 1))?;
        for (label, observed, discriminator, fact) in [
            ("no fact", "discount(50, 100)", "amount == threshold", None),
            (
                "other parameter",
                "discount(50, 100)",
                "amount == threshold",
                Some(&other),
            ),
            (
                "variable argument",
                "discount(amount, 100)",
                "amount == threshold",
                Some(&fact),
            ),
            (
                "float argument",
                "discount(50.5, 100)",
                "amount == threshold",
                Some(&fact),
            ),
            (
                "missing argument",
                "discount(50)",
                "amount == threshold",
                Some(&fact),
            ),
            (
                "spread argument",
                "discount(...pair)",
                "amount == threshold",
                Some(&fact),
            ),
            (
                "length receiver",
                "discount(50, 100)",
                "amount.length == threshold",
                Some(&fact),
            ),
        ] {
            let shape = parameter_pair_shape(observed, discriminator, fact);
            assert!(
                matches!(shape, TargetAssertionShape::UnboundOperand { .. }),
                "{label}: {shape:?}"
            );
            assert!(
                shape.packet_ineligibility_reason().is_some(),
                "{label}: {shape:?}"
            );
            assert!(shape.shape().contains("/* boundary input for "), "{label}");
        }
        Ok(())
    }

    /// #4759 review: a CONSTANT_CASE owner parameter (`LIMIT`) carries the
    /// parameter-pair fact on the analysis side, but the constant-shaped
    /// parse fallback closed the packet. When the fact names both comparison
    /// sides, they are read as parameters; without a matching fact the
    /// constant reading stays.
    #[test]
    fn parameter_pair_fact_reads_a_constant_shaped_owner_parameter() -> Result<(), String> {
        let fact = parsed_parameters_fact(&boundary_parameters_fact("amount", 0, "LIMIT", 1))?;
        // Without the fact, `LIMIT` parses as a constant and the packet
        // fails closed on the unresolved constant.
        let without = parameter_pair_shape("discount(50, 100)", "amount == LIMIT", None);
        assert!(
            matches!(without, TargetAssertionShape::UnresolvedBoundary { .. }),
            "{without:?}"
        );
        // With the fact, both sides are parameters and the observed integer
        // literals decide; a miss derives the boundary call.
        let derived = parameter_pair_shape("discount(50, 100)", "amount == LIMIT", Some(&fact));
        assert_eq!(derived.shape(), "expect(discount(100, 100)).toBe(expected)");
        assert!(
            derived.packet_ineligibility_reason().is_none(),
            "{derived:?}"
        );
        let pair_parse = parse_static_comparison("amount == LIMIT", Some(&fact));
        assert!(matches!(
            pair_parse.map(|c| (c.receiver, c.boundary)),
            Some((receiver, StaticBoundary::Parameter(name)))
                if receiver == "amount" && name == "LIMIT"
        ));
        // A hit keeps the observed call.
        let hits = parameter_pair_shape("discount(100, 100)", "amount == LIMIT", Some(&fact));
        assert!(
            matches!(hits, TargetAssertionShape::Observed { .. }),
            "{hits:?}"
        );
        // The written order `LIMIT == amount` binds the same two positions.
        let reversed = parameter_pair_shape("discount(50, 100)", "LIMIT == amount", Some(&fact));
        assert_eq!(reversed.shape(), "expect(discount(50, 50)).toBe(expected)");
        // A fact naming other parameters leaves the constant reading alone.
        let other = parsed_parameters_fact(&boundary_parameters_fact("amount", 0, "ceiling", 1))?;
        let mismatch = parameter_pair_shape("discount(50, 100)", "amount == LIMIT", Some(&other));
        assert!(
            matches!(mismatch, TargetAssertionShape::UnresolvedBoundary { .. }),
            "{mismatch:?}"
        );
        Ok(())
    }

    /// Literal and constant sides keep their #4105/#4215 reading in either
    /// order, and value keywords stay undecided, so the identifier reading
    /// only takes comparisons the earlier rules left unparsed.
    #[test]
    fn identifier_boundary_reading_leaves_earlier_comparisons_alone() {
        let constant_first = parse_static_comparison("LIMIT == amount", None);
        assert!(matches!(
            constant_first.map(|c| (c.receiver, c.boundary)),
            Some((receiver, StaticBoundary::Constant(name))) if receiver == "amount" && name == "LIMIT"
        ));
        let literal_first = parse_static_comparison("5 == amount", None);
        assert!(matches!(
            literal_first.map(|c| c.boundary),
            Some(StaticBoundary::Int(5))
        ));
        for keyword in [
            "flag == true",
            "value == null",
            "value === undefined",
            "ratio == NaN",
        ] {
            assert!(
                parse_static_comparison(keyword, None).is_none(),
                "{keyword}"
            );
            let shape = typescript_target_assertion_shape(
                &ProbeFamily::Predicate,
                "discount(1)",
                Some(keyword),
                Some("discount"),
            );
            assert!(
                matches!(shape, TargetAssertionShape::Observed { .. }),
                "{keyword}"
            );
        }
    }

    #[test]
    fn malformed_parameter_pair_facts_do_not_parse() {
        for line in [
            "typescript_boundary_parameters: parameter=amount;index=0;operand=threshold",
            "typescript_boundary_parameters: parameter=amount;index=0;operand=amount;operand_index=1",
            "typescript_boundary_parameters: parameter=amount;index=1;operand=threshold;operand_index=1",
            "typescript_boundary_parameters: parameter=amount;index=x;operand=threshold;operand_index=1",
            "typescript_boundary_parameters: parameter=a b;index=0;operand=threshold;operand_index=1",
            "typescript_boundary_parameters: parameter=amount;index=0;operand=100;operand_index=1",
            "typescript_boundary_parameters: parameter=amount;index=0;index=0;operand=threshold",
        ] {
            assert!(
                TypeScriptBoundaryParametersFact::parse(line).is_none(),
                "{line}"
            );
        }
        let mut finding = boundary_finding("discount(50, 100)", "0", "amount == threshold");
        finding
            .evidence
            .push(boundary_parameters_fact("amount", 0, "threshold", 1));
        finding
            .evidence
            .push(boundary_parameters_fact("amount", 0, "threshold", 1));
        assert!(typescript_boundary_parameters_fact(&finding).is_none());
    }
}
