//! Change classifier for the TypeScript preview adapter.

use super::*;

// ── Observation guard (RIPR-SPEC-0098) ───────────────────────────────────────

/// Tokenize an expression string into identifier tokens longer than 3 chars.
///
/// Only ASCII alphanumeric / underscore segments are kept. Segments adjacent
/// to a dot (`.`) or path separator (`:`) are excluded — both the qualifier
/// and the qualified member — so that shared names (e.g. `"amount"` from both
/// `props.amount * 9` and an unrelated bare `amount * 2`) do not spuriously
/// confirm: a dot-qualified name may belong to a different receiver.
fn identifier_tokens(expr: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut start: Option<usize> = None;
    for (idx, ch) in expr.char_indices() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            if start.is_none() {
                start = Some(idx);
            }
        } else if let Some(segment_start) = start.take() {
            push_identifier_token(expr, segment_start, idx, &mut tokens);
        }
    }
    if let Some(segment_start) = start {
        push_identifier_token(expr, segment_start, expr.len(), &mut tokens);
    }
    tokens
}

/// Push `expr[segment_start..segment_end]` as a confirmation token unless it
/// is dot/path-adjacent (see [`identifier_tokens`]) or too short.
fn push_identifier_token(
    expr: &str,
    segment_start: usize,
    segment_end: usize,
    tokens: &mut Vec<String>,
) {
    let preceded_by_qualifier = expr[..segment_start]
        .chars()
        .next_back()
        .is_some_and(|ch| ch == '.' || ch == ':');
    let followed_by_qualifier = expr[segment_end..]
        .chars()
        .next()
        .is_some_and(|ch| ch == '.' || ch == ':');
    if preceded_by_qualifier || followed_by_qualifier {
        return;
    }
    let token = &expr[segment_start..segment_end];
    if token.len() > 3 {
        tokens.push(token.to_string());
    }
}

/// Synthesized template words that `typescript_call_effect_discriminator`
/// interpolates into its English-sentence discriminator (`call … includes …`,
/// `call … occurs`, `mock interaction … is called`, `log contains …`). They
/// are generator vocabulary, not changed code, so they must never serve as
/// confirmation tokens for the RIPR-SPEC-0098 observation guard.
const CALL_EFFECT_TEMPLATE_WORDS: &[&str] = &[
    "includes",
    "occurs",
    "matching",
    "called",
    "interaction",
    "contains",
    "mock",
    "with",
    "call",
    "log",
];

/// Strip the synthesized prefix that `typescript_missing_discriminator_value`
/// adds so we recover the raw changed sub-expression.
fn strip_synthesized_prefix(discriminator_value: &str) -> &str {
    // Prefixes added by probe_shape.rs:
    //   "return value == "  (ReturnValue)
    //   "<lhs> == "         (Predicate/FieldConstruction)
    //   "call ... includes " (SideEffect)
    //   "log contains "     (SideEffect console.log)
    //   "call "             (SideEffect bare call name)
    //   "throws "           (ErrorPath)
    for prefix in &["return value == ", "log contains ", "call ", "throws "] {
        if let Some(rest) = discriminator_value.strip_prefix(prefix) {
            return rest;
        }
    }
    // "<lhs> == <rhs>" form from boundary/field discriminator
    if let Some(idx) = discriminator_value.find(" == ") {
        // Return the right-hand side (the changed value)
        return &discriminator_value[idx + 4..];
    }
    discriminator_value
}

/// Returns `true` when there is static evidence that a strong assertion in one
/// of the oracle-eligible related tests observes the specific changed
/// sub-expression — not just some other part of the same function body.
///
/// This is the RIPR-SPEC-0098 observation guard, scoped to **effect families**
/// (SideEffect and CallDeletion) where the false-exposed pattern is clearest:
/// a call side-effect (e.g. `console.log("audit", amount*9)`) was being promoted
/// to Exposed by value-based `toBe`/`toEqual` assertions in the same test body
/// that assert the owner's UNCHANGED return value.
///
/// ### What the guard checks
///
/// The confirmation decision keys on **`oracle_kind`** — which the live oracle
/// extractor always populates (`oracle.rs`) — rather than relying on
/// `observed_expression`, which is optional metadata. For SideEffect /
/// CallDeletion families a strong assertion CONFIRMS (returns `true`,
/// stays Exposed) only when it actually witnesses a call effect:
/// 1. It is an effect-shape oracle
///    (`MockExpectation` | `Snapshot` | `WholeObjectEquality`) — these capture
///    mock-call expectations, serialized snapshots, or persisted whole-object
///    state, all of which observe side effects directly; OR
/// 2. It carries an `observed_expression` that either shares an exact changed
///    identifier token (> 3 chars, dot/path-adjacent segments excluded, same
///    `identifier_tokens` rules on both sides) or names a side-channel (an
///    expression that does NOT name the owner — e.g. a closure-local
///    side-effect variable or a captured mock).
///
/// Value-shaped strong oracles (`ExactValue` / `ExactErrorVariant`) that observe
/// the owner's RETURN VALUE do NOT witness a `console.log`/side-effect change, so
/// they do not confirm. When the only strong oracles are value-shaped (or no
/// observed-expression metadata is available to prove a side-channel), the guard
/// **fails closed** and returns `false`, downgrading to WeaklyExposed.
///
/// ### Value families (ReturnValue / FieldConstruction)
///
/// A strong assertion confirms (returns `true`, stays Exposed) only when it
/// actually observes the changed sink:
/// 1. Its `oracle_kind` matches the seam family (the same filter
///    `strongest_family_matching_oracle` applies); AND
/// 2. Its `observed_expression` either references the owner (the owner name or
///    an owner call, e.g. `expect(applyDiscount(100, 10))`) or contains a
///    changed token from the changed sub-expression.
///
/// One-hop local aliasing: a bare-local observed_expression
/// (`t.is(result, 3)`) also confirms when the test body initializes that
/// local from an expression referencing the owner or a changed token
/// (`const result = add(1, 2)`). Without this credit the guard would
/// falsely downgrade the canonical assert-the-return-value pattern and emit
/// repair packets for tests that already observe the changed sink — the
/// exact over-correction the RIPR-SPEC-0108 `must_stay_exposed` corpus
/// controls (`ts_ava_t_is_exact_value`, `ts_tape_equal_exact_value`) pin.
///
/// An assertion on an UNRELATED expression (`expect(formatDate(now))`, or a
/// local initialized from an unrelated call) does not witness
/// `return amount - 12;`: the changed value never escapes into the observed
/// expression, so the guard fails closed and the finding downgrades to
/// WeaklyExposed. This mirrors the sink-alignment evidence the Python
/// adapter already surfaces.
///
/// For all other families the guard always returns `true` (pre-guard
/// behaviour). SideEffect / CallDeletion behaviour is pinned and unchanged.
///
/// ### Fail-closed default (RIPR-SPEC-0098 hardening, #1235)
///
/// Earlier revisions returned `true` ("confirmed") when no strong assertion
/// carried `observed_expression` metadata. That was a fail-OPEN fallback:
/// "can't tell what's observed → assume observed → exposed", which is exactly the
/// over-claim this guard exists to kill. It is removed. The decision now keys on
/// `oracle_kind` (always populated live) plus an optional `observed_expression`
/// token / side-channel check; absence of `observed_expression` no longer
/// re-promotes a swallowed side effect to Exposed.
pub(crate) fn ts_changed_value_is_observed(
    probe_shape: &TypeScriptProbeShape,
    line_text: &str,
    owner_name: &str,
    candidates: &[TypeScriptRelatedCandidate<'_>],
) -> bool {
    // Apply the guard to effect families (SideEffect / CallDeletion) and value
    // families (ReturnValue / FieldConstruction). All other families keep the
    // pre-guard (always-confirmed) behaviour.
    let value_family = matches!(
        probe_shape.family,
        ProbeFamily::ReturnValue | ProbeFamily::FieldConstruction
    );
    if !value_family
        && !matches!(
            probe_shape.family,
            ProbeFamily::SideEffect | ProbeFamily::CallDeletion
        )
    {
        return true;
    }

    // Collect changed identifier tokens from the discriminator value.
    let raw_discriminator = typescript_missing_discriminator_value(&probe_shape.family, line_text);
    let changed_tokens: Vec<String> = if let Some(ref disc) = raw_discriminator {
        let raw_expr = strip_synthesized_prefix(disc);
        identifier_tokens(raw_expr)
            .into_iter()
            .filter(|tok| !CALL_EFFECT_TEMPLATE_WORDS.contains(&tok.as_str()))
            .collect()
    } else {
        Vec::new()
    };

    // Fail-CLOSED: confirmation must be affirmatively established by at least one
    // strong assertion that actually witnesses the changed sink. We scan every
    // strong assertion in the oracle-eligible candidates and return `true` the
    // moment one of them qualifies. If none qualify, we downgrade — including
    // the case where no `observed_expression` metadata is available at all (the
    // former fail-OPEN `return true` fallback is deliberately gone: absence of
    // proof is not proof of observation).
    for candidate in candidates {
        if !candidate.relation.uses_oracle() {
            continue;
        }
        for assertion in &candidate.test.assertions {
            if assertion.oracle_strength.rank() < OracleStrength::Strong.rank() {
                continue;
            }
            if value_family {
                // Value families (ReturnValue / FieldConstruction): the strong
                // assertion must MATCH the seam family (the same filter applied
                // to `strongest_strength`) and its observed_expression must
                // reference the owner (owner name or its call) or contain a
                // changed token from the changed sub-expression. An assertion
                // on an unrelated expression does not observe the changed
                // sink; absence of observed_expression fails closed too.
                if !ts_oracle_kind_matches_seam(&assertion.oracle_kind, &probe_shape.family) {
                    continue;
                }
                let Some(ref observed) = assertion.observed_expression else {
                    continue;
                };
                if observed.contains(owner_name) {
                    return true;
                }
                if !changed_tokens.is_empty()
                    && changed_tokens
                        .iter()
                        .any(|tok| observed.contains(tok.as_str()))
                {
                    return true;
                }
                // One-hop local aliasing (RIPR-SPEC-0108 must_stay_exposed
                // controls): `const result = add(1, 2)` asserted via
                // `t.is(result, 3)` still observes the changed sink — the
                // local is initialized from the owner call. Without this
                // credit the guard falsely downgrades the canonical
                // assert-the-return-value pattern.
                if ts_observed_local_aliases_owner(
                    observed,
                    owner_name,
                    &changed_tokens,
                    &candidate.test.body_text,
                ) {
                    return true;
                }
                // This family-matching strong assertion observes an unrelated
                // expression: it does NOT witness the changed sink. Keep
                // scanning for a qualifying assertion.
                continue;
            }
            // (1) Effect-shape oracle kinds confirm unconditionally: these ARE
            // effect observers (mock call expectation, snapshot, whole-object
            // equality capturing persisted state). This decision keys ONLY on
            // `oracle_kind`, which the live extractor always populates.
            if matches!(
                assertion.oracle_kind,
                OracleKind::MockExpectation
                    | OracleKind::Snapshot
                    | OracleKind::WholeObjectEquality
            ) {
                return true;
            }
            // (2) Optional `observed_expression` checks — only when the live
            // extractor retained the `expect(<expr>)` argument text. These can
            // only ADD confirmations; their absence never re-promotes.
            if let Some(ref observed) = assertion.observed_expression {
                // Token match: tokenize the observed expression with the SAME
                // `identifier_tokens` rules and require EXACT token equality.
                // A raw substring `observed.contains(tok)` false-confirms on
                // synthesized template words (e.g. a `.includes(...)` call in
                // the test) and on tokens merely embedded in a larger
                // identifier.
                if !changed_tokens.is_empty() {
                    let observed_tokens = identifier_tokens(observed);
                    if changed_tokens
                        .iter()
                        .any(|tok| observed_tokens.contains(tok))
                    {
                        return true;
                    }
                }
                // Side-channel: the observed expression does NOT name the owner,
                // so it is asserting something other than the owner return value
                // (a closure-local side-effect variable, a captured mock result,
                // or any side-channel) — conservatively treat it as observing
                // the effect.
                if !observed.contains(owner_name) {
                    return true;
                }
            }
            // Otherwise this strong assertion is value-shaped (ExactValue /
            // ExactErrorVariant) observing the owner return value, or carries no
            // observed_expression to prove a side-channel: it does NOT witness
            // the call effect. Keep scanning for a qualifying assertion.
        }
    }

    // No strong assertion witnessed the changed sink: fail closed → downgrade.
    false
}

/// One-hop local aliasing credit for the value-family observation guard.
///
/// Test code conventionally captures the owner call in a local and asserts
/// that local (`const result = add(1, 2); t.is(result, 3)`). The bare-local
/// `observed_expression` carries no owner reference, but the changed value
/// flows into it through the initializer, so treating it as unobserved
/// would be a false downgrade that also flips `repair_packet_ready` on for
/// tests that already assert the changed value (the over-correction pinned
/// by the RIPR-SPEC-0108 `must_stay_exposed` corpus controls).
///
/// Conservatism: only a bare identifier qualifies (member or call
/// expressions were already decided by the direct owner / changed-token
/// checks), the assignment match requires whole-word identifier boundaries
/// and a single `=` (not `==` / `=>`), the initializer is read only up to
/// the next `;` or newline, and only the FIRST matching assignment in the
/// test body is considered. Any failure to resolve keeps the guard's
/// fail-closed downgrade.
fn ts_observed_local_aliases_owner(
    observed: &str,
    owner_name: &str,
    changed_tokens: &[String],
    test_body: &str,
) -> bool {
    fn is_ident_byte(b: u8) -> bool {
        b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
    }
    let ident = observed.trim();
    let ident_bytes = ident.as_bytes();
    let ident_is_bare = !ident_bytes.is_empty()
        && (ident_bytes[0].is_ascii_alphabetic()
            || ident_bytes[0] == b'_'
            || ident_bytes[0] == b'$')
        && ident_bytes.iter().all(|&b| is_ident_byte(b));
    if !ident_is_bare {
        return false;
    }
    let body = test_body.as_bytes();
    let mut search_from = 0;
    while let Some(pos) = test_body
        .get(search_from..)
        .and_then(|rest| rest.find(ident))
    {
        let abs = search_from + pos;
        let before_ok = abs == 0 || !is_ident_byte(body[abs - 1]);
        let after = abs + ident.len();
        let after_ok = after >= body.len() || !is_ident_byte(body[after]);
        if before_ok && after_ok {
            let mut i = after;
            while i < body.len() && body[i].is_ascii_whitespace() {
                i += 1;
            }
            // A single `=`: reject `==` / `===` / `=>`.
            if i < body.len() && body[i] == b'=' && body.get(i + 1) != Some(&b'=') {
                let rhs_start = i + 1;
                let rhs_end = test_body[rhs_start..]
                    .find([';', '\n'])
                    .map(|p| rhs_start + p)
                    .unwrap_or(test_body.len());
                let rhs = &test_body[rhs_start..rhs_end];
                if rhs.contains(owner_name)
                    || changed_tokens.iter().any(|tok| rhs.contains(tok.as_str()))
                {
                    return true;
                }
            }
        }
        search_from = after.max(abs + 1);
    }
    false
}

/// Build the named limitation message for the RIPR-SPEC-0098 downgrade arm.
///
/// Fires for SideEffect / CallDeletion families (effect sink swallowed by
/// value-shaped oracles) and for ReturnValue / FieldConstruction families
/// (strong assertions that observe an unrelated expression, not the changed
/// sink). Emits a `propagation_unknown` limitation.
pub(crate) fn ts_observation_guard_limitation(
    probe_shape: &TypeScriptProbeShape,
    line_text: &str,
) -> String {
    let sink_hint = typescript_missing_discriminator_value(&probe_shape.family, line_text)
        .map(|disc| {
            let raw = strip_synthesized_prefix(&disc);
            format!(" (`{raw}`)")
        })
        .unwrap_or_default();
    if matches!(
        probe_shape.family,
        ProbeFamily::ReturnValue | ProbeFamily::FieldConstruction
    ) {
        return format!(
            "propagation_unknown: changed value sinks to the owner return value or constructed object{sink_hint}; \
             all strong assertions observe an unrelated expression, not the changed sink; \
             propagation unknown"
        );
    }
    // Describe the non-escaping sink where possible.
    format!(
        "propagation_unknown: changed value sinks to a non-escaping call effect{sink_hint}; \
         all strong assertions observe the owner return value, not this call effect; \
         propagation unknown"
    )
}

// ── Predicate boundary witness (RIPR-SPEC-0027) ──────────────────────────────

/// Returns `true` when static evidence shows that a strong, family-matching
/// assertion observes an owner call that sits at the changed predicate's
/// boundary. Non-predicate families are not gated here (`true`).
///
/// A strong exact assertion somewhere in a related test does not show that the
/// changed comparison is discriminated: `shippingFee(60)` and `shippingFee(10)`
/// both behave identically under `total > 50` and `total >= 50`. Only an input
/// at the boundary (`total == 50`) tells the two apart, so `Exposed` requires
/// an assertion whose observed owner call carries that boundary:
///
/// - **One literal operand** (`total >= 50`, `status === 'paid'`): an argument
///   of the observed owner call carries the literal at a position the
///   comparison reads, standing alone as a value (#4102 guards 1 and 2) —
///   either the whole argument is the literal, or an object-literal argument
///   pins the comparison operand's field to it (`{ total: 50 }`). A literal
///   inside a larger expression (`price + 100`) leaves the effective input
///   unknown and a literal in an argument past the owner's readable parameter
///   positions (`applyDiscount(150, 100)` against a single-parameter owner) is
///   never read by the comparison.
/// - **No literal operand** (`count <= limit`): the adapter has no literal to
///   map, so it accepts the same textual witness the Rust boundary evidence
///   accepts without value resolution — two identical arguments in one
///   observed owner call (`isAllowed(5, 5)`) — or an object-literal argument
///   that names both operands with identical values (`PriceLabel({ amount:
///   100, threshold: 100 })`). Otherwise the boundary is not witnessed.
/// - **No parseable comparison** (`if (Number.isNaN(n))`, `case 'gold':`) or an
///   ambiguous fallback shape (`}`): nothing statically ties a test to the
///   changed branch, so this fails closed.
///
/// Observation keys on `observed_expression` (the `expect(<expr>)` argument).
/// When it is absent, or names a local such as `result`, the witness fails
/// closed; the finding then takes the existing weak path.
///
/// Identity and liveness guards (#4102): a receiver-qualified call
/// (`pricing.applyDiscount(100)`) witnesses only when the receiver binds to
/// the owner's own module (a namespace import of the owner file; the landed
/// receiver resolution keeps a same-named method on an unrelated receiver —
/// `other.total(50)` — from witnessing); a test body that declares its own
/// same-name function or const calls the shadow, not the owner; an expected
/// side that itself calls the owner is a tautology; and a dead expected
/// literal — one that matches neither the changed behavior's value at the
/// boundary input nor, when checkable, differs from the unchanged behavior's
/// — cannot witness. Every unresolved case fails closed.
pub(crate) fn ts_predicate_boundary_is_witnessed(
    probe_shape: &TypeScriptProbeShape,
    line_text: &str,
    owner: &TypeScriptOwner,
    candidates: &[TypeScriptRelatedCandidate<'_>],
    alias_map: Option<&TsAliasMap>,
    workspace_root: Option<&Path>,
) -> bool {
    // An ambiguous fallback shape (`}`, an unrecognised statement) never names
    // a behavior an assertion could be shown to observe.
    if !probe_shape.specific {
        return false;
    }
    if probe_shape.family != ProbeFamily::Predicate {
        return true;
    }
    let Some(boundary) = typescript_boundary_discriminator(line_text) else {
        return false;
    };
    let Some((left, right)) = boundary.split_once(" == ") else {
        return false;
    };
    let literals: Vec<&str> = [left, right]
        .into_iter()
        .filter(|operand| is_boundary_literal(operand))
        .collect();

    for candidate in candidates {
        if !candidate.relation.uses_oracle() {
            continue;
        }
        // Shadow guard (#4102): a test body that declares its own
        // `function <owner>(...)` / `const <owner> = ...` executes the local
        // declaration, not the changed owner — its assertions cannot witness
        // the owner's boundary however boundary-shaped they look.
        if local_identifier_declared_in_test_body(&candidate.test.body_text, &owner.name) {
            continue;
        }
        let owner_receivers =
            owner_namespace_receivers(candidate.test, owner, alias_map, workspace_root);
        for assertion in &candidate.test.assertions {
            if assertion.oracle_strength.rank() < OracleStrength::Strong.rank()
                || !ts_oracle_kind_matches_seam(&assertion.oracle_kind, &ProbeFamily::Predicate)
            {
                continue;
            }
            let Some(observed) = assertion.observed_expression.as_deref() else {
                continue;
            };
            for arguments in owner_call_arguments(observed, &owner.name, &owner_receivers) {
                let witnessed = if literals.is_empty() {
                    call_has_identical_arguments(&arguments)
                        || object_argument_pins_operands_equal(&arguments, left, right)
                } else {
                    literals.iter().any(|literal| {
                        boundary_literal_reaches_read_argument(
                            &arguments, literal, left, right, owner,
                        )
                    })
                };
                if !witnessed {
                    continue;
                }
                // Expected-side guards (#4102): a self-comparing expectation
                // (`toBe(applyDiscount(100))`) never discriminates, and a dead
                // expected literal (`toBe(999)` when the changed behavior
                // produces `90` at the boundary input) proves nothing — the
                // changed value never reaches a passing assertion.
                if expected_side_calls_owner(&candidate.test.body_text, observed, &owner.name) {
                    continue;
                }
                if let Some(expected) = assertion.expected_value_or_variant.as_deref()
                    && expected_side_is_live(owner, line_text, left, right, &arguments, expected)
                        == Some(false)
                {
                    continue;
                }
                return true;
            }
        }
    }
    false
}

/// Build the named limitation for a predicate whose boundary no strong
/// assertion reaches (RIPR-SPEC-0027 boundary witness).
pub(crate) fn ts_predicate_boundary_limitation(
    probe_shape: &TypeScriptProbeShape,
    line_text: &str,
    owner_name: &str,
) -> String {
    let boundary = if probe_shape.specific {
        typescript_boundary_discriminator(line_text)
    } else {
        None
    };
    match boundary {
        Some(boundary) => format!(
            "Related tests reach `{owner_name}` with a strong oracle, but no strong assertion observes a `{owner_name}(...)` call at the changed predicate boundary `{boundary}`; inputs on one side of the boundary do not discriminate the changed comparison."
        ),
        None => format!(
            "Related tests reach `{owner_name}` with a strong oracle, but the changed line has no statically comparable predicate boundary; static evidence cannot tie an assertion to the changed branch."
        ),
    }
}

fn is_boundary_literal(operand: &str) -> bool {
    let operand = operand.trim();
    numeric_literal_value(operand).is_some()
        || matches!(operand, "true" | "false" | "null" | "undefined")
        || (operand.len() >= 2
            && ['"', '\'', '`']
                .iter()
                .any(|quote| operand.starts_with(*quote) && operand.ends_with(*quote)))
}

/// Canonical decimal text of a numeric literal token (`50`, `50.0`, `1_000`,
/// `10n`), or `None` when the token is not a plain decimal literal.
fn numeric_literal_value(token: &str) -> Option<String> {
    let token = token.trim().trim_end_matches('n').replace('_', "");
    if !token.starts_with(|ch: char| ch.is_ascii_digit())
        || !token.chars().all(|ch| ch.is_ascii_digit() || ch == '.')
        || token.matches('.').count() > 1
    {
        return None;
    }
    let (integer, fraction) = token.split_once('.').unwrap_or((token.as_str(), ""));
    let integer = match integer.trim_start_matches('0') {
        "" => "0",
        digits => digits,
    };
    let fraction = fraction.trim_end_matches('0');
    Some(if fraction.is_empty() {
        integer.to_string()
    } else {
        format!("{integer}.{fraction}")
    })
}

/// Return the argument lists (one per call) of every `<owner_name>(...)` call
/// inside `observed`, split at top-level commas.
/// Receiver names in `test` that bind to the owner's own module through a
/// namespace import (`import * as pricing from "../src/pricing"`). A member
/// call on such a receiver (`pricing.applyDiscount(...)`) IS an owner call,
/// while a same-named method on any other receiver (`other.total(...)`) is
/// not — this is the receiver resolution that keeps genuine namespace-import
/// witnesses credited without crediting unrelated receivers.
fn owner_namespace_receivers(
    test: &TypeScriptTest,
    owner: &TypeScriptOwner,
    alias_map: Option<&TsAliasMap>,
    workspace_root: Option<&Path>,
) -> Vec<String> {
    test.imports_in_file
        .iter()
        .filter(|import| {
            import.namespace
                && import_source_matches_owner(import, &test.file, owner, alias_map, workspace_root)
        })
        .map(|import| import.local.clone())
        .collect()
}

/// The trailing identifier segment immediately before the final `.` of
/// `before_match` — the receiver of a member call (`expect(pricing.` →
/// `pricing`).
fn receiver_before_dot(before_match: &str) -> String {
    let trimmed = before_match.trim_end();
    let without_dot = trimmed.strip_suffix('.').unwrap_or(trimmed);
    without_dot
        .chars()
        .rev()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '$')
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

/// Return the argument lists (one per call) of every `<owner_name>(...)` call
/// inside `observed`, split at top-level commas.
///
/// A member match (`pricing.applyDiscount(...)`) is kept only when the
/// receiver is bound to the owner's own module via a namespace import
/// (`owner_receivers`); otherwise the match is a same-named method on an
/// unrelated receiver and is skipped.
fn owner_call_arguments(
    observed: &str,
    owner_name: &str,
    owner_receivers: &[String],
) -> Vec<Vec<String>> {
    let mut calls = Vec::new();
    if owner_name.is_empty() {
        return calls;
    }
    for (idx, _) in observed.match_indices(owner_name) {
        // Skip matches embedded in a longer identifier (`otherShippingFee`):
        // the match must start at a real owner call.
        let Some(before) = observed.get(..idx) else {
            continue;
        };
        let preceded_by_identifier = before
            .chars()
            .next_back()
            .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$');
        if preceded_by_identifier {
            continue;
        }
        // Member access (`receiver.owner(...)`): keep only when the receiver
        // resolves to the owner's own module (namespace import); a same-named
        // method on an unrelated receiver (`other.total(50)`) must not
        // witness the owner's boundary.
        if before.trim_end().ends_with('.')
            && !owner_receivers
                .iter()
                .any(|receiver| receiver == &receiver_before_dot(before))
        {
            continue;
        }
        let Some(inner) = observed
            .get(idx + owner_name.len()..)
            .and_then(|rest| rest.trim_start().strip_prefix('('))
        else {
            continue;
        };
        if let Some(arguments) = balanced_call_arguments(inner) {
            calls.push(arguments);
        }
    }
    calls
}

/// Split the text after a call's opening parenthesis into its top-level
/// arguments, stopping at the matching closing parenthesis.
fn balanced_call_arguments(inner: &str) -> Option<Vec<String>> {
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut current = String::new();
    let mut arguments = Vec::new();
    for ch in inner.chars() {
        if let Some(open) = quote {
            current.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == open {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' | '`' => {
                quote = Some(ch);
                current.push(ch);
            }
            '(' | '[' | '{' => {
                depth += 1;
                current.push(ch);
            }
            ')' if depth == 0 => {
                let last = current.trim();
                if !last.is_empty() {
                    arguments.push(last.to_string());
                }
                return Some(arguments);
            }
            ')' | ']' | '}' => {
                depth = depth.saturating_sub(1);
                current.push(ch);
            }
            ',' if depth == 0 => {
                arguments.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    None
}

fn call_has_identical_arguments(arguments: &[String]) -> bool {
    arguments.iter().enumerate().any(|(idx, left)| {
        !left.is_empty() && arguments.iter().skip(idx + 1).any(|right| right == left)
    })
}

/// `true` when an object-literal argument binds both comparison operands
/// (matched by their last member segment, e.g. `props.amount` → `amount`) to
/// the same non-empty value text.
fn object_argument_pins_operands_equal(arguments: &[String], left: &str, right: &str) -> bool {
    let key_of = |operand: &str| operand.rsplit('.').next().unwrap_or(operand).to_string();
    let (left_key, right_key) = (key_of(left), key_of(right));
    if left_key.is_empty() || left_key == right_key {
        return false;
    }
    arguments.iter().any(|argument| {
        let Some(body) = argument
            .trim()
            .strip_prefix('{')
            .and_then(|rest| rest.strip_suffix('}'))
        else {
            return false;
        };
        let Some(fields) = balanced_call_arguments(&format!("{body})")) else {
            return false;
        };
        let value_of = |key: &str| {
            fields.iter().find_map(|field| {
                let (name, value) = field.split_once(':')?;
                (name.trim().trim_matches(['"', '\'']) == key).then(|| value.trim().to_string())
            })
        };
        matches!(
            (value_of(&left_key), value_of(&right_key)),
            (Some(left_value), Some(right_value))
                if !left_value.is_empty() && left_value == right_value
        )
    })
}

/// owner's arity is known, and nothing is excluded when no parameter facts
/// exist (the remaining guards still apply).
enum ReadPosition {
    /// The comparison operand names the parameter at this index.
    Index(usize),
    /// The operand does not name a parameter, but the arity is known.
    Bounded(usize),
    /// No resolvable parameter facts — every position is plausible.
    Any,
}

fn comparison_read_position(operand: &str, owner: &TypeScriptOwner) -> ReadPosition {
    let segment = last_identifier_segment(operand);
    if let Some(idx) = owner.parameters.iter().position(|name| name == segment) {
        return ReadPosition::Index(idx);
    }
    match owner.arity {
        Some(arity) => ReadPosition::Bounded(arity),
        None => ReadPosition::Any,
    }
}

fn last_identifier_segment(operand: &str) -> &str {
    operand.trim().rsplit('.').next().unwrap_or(operand).trim()
}

/// `true` when the boundary literal reaches the argument position the changed
/// comparison reads (#4102 guards 1 and 2). The literal must stand alone as
/// the argument value — the whole argument tokenizes to the literal, or an
/// object-literal argument pins the comparison operand's field to it
/// (`{ total: 50 }` for `total >= 50`). Containment inside a larger
/// expression (`price + 100`) leaves the effective input unknown and never
/// credits, and an argument beyond the owner's readable positions
/// (`applyDiscount(150, 100)` against `applyDiscount(total)`) is never read.
fn boundary_literal_reaches_read_argument(
    arguments: &[String],
    literal: &str,
    left: &str,
    right: &str,
    owner: &TypeScriptOwner,
) -> bool {
    let operand = if is_boundary_literal(right) {
        left
    } else {
        right
    };
    let position = comparison_read_position(operand, owner);
    let readable = |idx: usize| match position {
        ReadPosition::Index(read) => idx == read,
        ReadPosition::Bounded(arity) => idx < arity,
        ReadPosition::Any => true,
    };
    arguments.iter().enumerate().any(|(idx, argument)| {
        readable(idx)
            && (argument_stands_alone_as_literal(argument, literal)
                || object_argument_pins_literal(argument, operand, literal))
    })
}

/// `true` when the whole argument is exactly the literal value: a plain
/// decimal (`50`, `50.0`, `1_000`, `50n`), the same quoted string, or the
/// bare token (`true` / `false` / `null` / `undefined` / an identifier
/// operand).
fn argument_stands_alone_as_literal(argument: &str, literal: &str) -> bool {
    let argument = argument.trim();
    let literal = literal.trim();
    if let Some(expected) = numeric_literal_value(literal) {
        return numeric_literal_value(argument).is_some_and(|value| value == expected);
    }
    if let Some(body) = literal
        .strip_prefix(['"', '\'', '`'])
        .and_then(|rest| rest.strip_suffix(['"', '\'', '`']))
    {
        return ['"', '\'', '`'].iter().any(|quote| {
            argument.starts_with(*quote)
                && argument.ends_with(*quote)
                && argument.len() >= 2
                && argument.get(1..argument.len() - 1) == Some(body)
        });
    }
    argument == literal
}

/// `true` when an object-literal argument pins the comparison operand (matched
/// by its last identifier segment) to exactly the boundary literal —
/// `applyDiscount({ total: 50 })` for `total >= 50`. A same-value field
/// binding keeps the effective input known, unlike containment in a larger
/// expression.
fn object_argument_pins_literal(argument: &str, operand: &str, literal: &str) -> bool {
    let key = last_identifier_segment(operand);
    if key.is_empty() {
        return false;
    }
    let Some(body) = argument
        .trim()
        .strip_prefix('{')
        .and_then(|rest| rest.strip_suffix('}'))
    else {
        return false;
    };
    let Some(fields) = balanced_call_arguments(&format!("{body})")) else {
        return false;
    };
    fields.iter().any(|field| {
        let Some((name, value)) = field.split_once(':') else {
            return false;
        };
        name.trim().trim_matches(['"', '\'']) == key
            && argument_stands_alone_as_literal(value.trim(), literal)
    })
}

/// `true` when the matcher argument of the `expect(<observed>)...` assertion
/// itself calls the owner (`expect(applyDiscount(100)).toBe(applyDiscount(
/// 100))`) — a self-comparing tautology that can never discriminate the
/// changed behavior (#4102 expected-side liveness).
fn expected_side_calls_owner(body_text: &str, observed: &str, owner_name: &str) -> bool {
    let observed = observed.trim();
    if observed.is_empty() {
        return false;
    }
    for (idx, _) in body_text.match_indices("expect(") {
        if line_prefix_looks_like_comment_or_string(body_text, idx)
            || inside_block_comment(body_text, idx)
        {
            continue;
        }
        let Some(after_open) = body_text.get(idx + "expect(".len()..) else {
            continue;
        };
        if !after_open.trim_start().starts_with(observed) {
            continue;
        }
        let Some(close) = balanced_close_offset(after_open) else {
            continue;
        };
        // Walk the `.resolves` / `.rejects` / matcher member chain after the
        // expect(...) call to the matcher's argument list.
        let mut rest = &after_open[close + 1..];
        loop {
            rest = rest.trim_start();
            let Some(after_dot) = rest.strip_prefix('.') else {
                break;
            };
            let ident_len = after_dot
                .chars()
                .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '$')
                .count();
            if ident_len == 0 {
                break;
            }
            rest = &after_dot[ident_len..];
        }
        let Some(after_paren) = rest.trim_start().strip_prefix('(') else {
            continue;
        };
        let Some(argument_close) = balanced_close_offset(after_paren) else {
            continue;
        };
        // The tautology shape is the bare owner call; receiver resolution is
        // not needed to detect the expected side re-calling the owner.
        if !owner_call_arguments(&after_paren[..argument_close], owner_name, &[]).is_empty() {
            return true;
        }
    }
    false
}

/// Byte offset of the matching close parenthesis within `inner` (which
/// excludes the opening paren), string-aware; `None` when unbalanced.
fn balanced_close_offset(inner: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (offset, ch) in inner.char_indices() {
        if let Some(open) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == open {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' | '`' => quote = Some(ch),
            '(' | '[' | '{' => depth += 1,
            ')' => {
                if depth == 0 {
                    return Some(offset);
                }
                depth -= 1;
            }
            ']' | '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    None
}

/// Expected-side liveness for a witnessed boundary assertion (#4102).
///
/// Returns `Some(true)` when the expected literal can be the CHANGED
/// behavior's value at the boundary input and differs from the UNCHANGED
/// behavior's value there; `Some(false)` when it provably cannot (a dead test
/// whose expectation is wrong under both behaviors, or one that pins the
/// unchanged value and so discriminates nothing); `None` when the check is not
/// statically determinable (no owner body, unresolvable branches or bindings,
/// non-numeric expectation) — the witness decision then stands unchanged.
///
/// The semantics are derived only from statically resolvable facts: the
/// changed comparison's operator, the observed argument values bound to the
/// comparison's parameters, and the two branch `return` expressions of the
/// changed `if` in the owner's own source. At the boundary input the changed
/// comparison takes the branch the new operator selects, and the unchanged
/// behavior took the opposite branch — that is exactly what makes the input
/// discriminating. Crate-visible so the typescript unit tests can pin the
/// `None` fail-closed contract directly.
pub(crate) fn expected_side_is_live(
    owner: &TypeScriptOwner,
    line_text: &str,
    left: &str,
    right: &str,
    arguments: &[String],
    expected: &str,
) -> Option<bool> {
    let source = owner.source_text.as_deref()?;
    let expected_value = numeric_literal_value(expected).and_then(|v| v.parse::<f64>().ok())?;
    let operator = changed_comparison_operator(line_text)?;
    let lines: Vec<&str> = source.lines().collect();
    // Locate the changed line inside the owner body. Uniqueness runs before
    // the declared offset (#4117 review): a multiline variable declarator can
    // shift the offset onto a different line carrying identical predicate
    // text, so the branches are attributed only when exactly one line
    // matches; zero or duplicated matches skip the check instead of reading
    // the wrong branch pair.
    let mut matching = lines
        .iter()
        .enumerate()
        .filter(|(_, candidate)| candidate.trim() == line_text.trim())
        .map(|(index, _)| index);
    let start = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    let (true_expr, false_expr) = branch_return_expressions(&lines, start)?;
    let lhs = comparison_operand_value(left, owner, arguments)?;
    let rhs = comparison_operand_value(right, owner, arguments)?;
    let new_takes_true = comparison_holds(&operator, lhs, rhs)?;
    let bindings = observed_argument_bindings(owner, arguments);
    let (new_expr, old_expr) = if new_takes_true {
        (true_expr.as_str(), false_expr.as_str())
    } else {
        (false_expr.as_str(), true_expr.as_str())
    };
    let new_value = fold_numeric_expression(new_expr, &bindings)?;
    let old_value = fold_numeric_expression(old_expr, &bindings)?;
    let new_epsilon = 1e-9 * new_value.abs().max(1.0);
    let old_epsilon = 1e-9 * old_value.abs().max(1.0);
    Some(
        (expected_value - new_value).abs() <= new_epsilon
            && (expected_value - old_value).abs() > old_epsilon,
    )
}

/// The operator of the changed comparison, probed in the same ordered list
/// `typescript_boundary_discriminator` uses so the operands align.
fn changed_comparison_operator(line_text: &str) -> Option<String> {
    let expression = strip_typescript_control_prefix(line_text);
    ["===", "!==", ">=", "<=", "==", "!=", ">", "<"]
        .into_iter()
        .find(|operator| expression.contains(operator))
        .map(str::to_string)
}

/// The numeric value a comparison operand evaluates to in the witnessed call:
/// the argument at that parameter's position when the operand names a
/// parameter, else the operand's own literal value.
fn comparison_operand_value(
    operand: &str,
    owner: &TypeScriptOwner,
    arguments: &[String],
) -> Option<f64> {
    let segment = last_identifier_segment(operand);
    if let Some(idx) = owner.parameters.iter().position(|name| name == segment)
        && let Some(value) = arguments
            .get(idx)
            .and_then(|argument| numeric_literal_value(argument))
            .and_then(|value| value.parse::<f64>().ok())
    {
        return Some(value);
    }
    numeric_literal_value(operand).and_then(|value| value.parse::<f64>().ok())
}

/// Parameter-name bindings for folding branch expressions: every parameter
/// whose observed argument position carries a plain numeric literal.
fn observed_argument_bindings(owner: &TypeScriptOwner, arguments: &[String]) -> Vec<(String, f64)> {
    owner
        .parameters
        .iter()
        .enumerate()
        .filter_map(|(idx, name)| {
            let value = arguments
                .get(idx)
                .and_then(|argument| numeric_literal_value(argument))
                .and_then(|value| value.parse::<f64>().ok())?;
            Some((name.clone(), value))
        })
        .collect()
}

/// The true/false-branch `return` expressions of the changed `if` at `start`
/// (0-based index into the owner source lines). Only two shapes are
/// attributed — a fall-through else-less branch and a plain `} else {` block;
/// nested branching, computed closers, quoted braces, and comments fail the
/// scan so the caller skips the check instead of attributing a wrong branch.
/// Fail closed (#4117 review): any top-level branch statement that is not
/// that branch's single `return` — notably a parameter reassignment, in
/// either branch or the fall-through — also fails the scan, because folding
/// the `return` expression with the original argument bindings would credit a
/// value the intervening statement already changed. Only closing braces at
/// the branch's own depth are recognized as branch closers.
pub(crate) fn branch_return_expressions(lines: &[&str], start: usize) -> Option<(String, String)> {
    if !lines.get(start)?.contains('{') {
        return None;
    }
    enum Phase {
        True,
        Else,
        Fallthrough,
        Done,
    }
    let mut phase = Phase::True;
    let mut depth: i64 = 1; // the changed line opened the if-body
    let mut true_return: Option<String> = None;
    let mut false_return: Option<String> = None;
    for text in lines.iter().skip(start + 1) {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }
        if ["\"", "'", "`", "//", "/*"]
            .iter()
            .any(|marker| trimmed.contains(marker))
        {
            return None; // quoted braces or comments would corrupt the scan
        }
        match phase {
            Phase::Done => break,
            Phase::True => {
                if depth == 1 && trimmed.starts_with('}') {
                    if trimmed.contains("else") {
                        // `} else {`: the false branch body opens here.
                        if trimmed.contains("else if") || !trimmed.ends_with('{') {
                            return None;
                        }
                        phase = Phase::Else;
                        depth = 1;
                    } else if trimmed == "}" {
                        phase = Phase::Fallthrough;
                        depth = 0;
                    } else {
                        return None;
                    }
                    continue;
                }
                if depth == 1 {
                    if true_return.is_some() {
                        return None; // a statement follows the return: unattributable
                    }
                    true_return = Some(return_statement_expression(trimmed)?.to_string());
                }
                depth += trimmed.matches('{').count() as i64;
                depth -= trimmed.matches('}').count() as i64;
                if depth < 1 {
                    return None; // closed without a `}`-led line: unattributable
                }
            }
            Phase::Else => {
                if depth == 1 && trimmed.starts_with('}') {
                    if trimmed == "}" {
                        phase = Phase::Done;
                        continue;
                    }
                    return None;
                }
                if depth == 1 {
                    if false_return.is_some() {
                        return None; // a statement follows the return: unattributable
                    }
                    false_return = Some(return_statement_expression(trimmed)?.to_string());
                }
                depth += trimmed.matches('{').count() as i64;
                depth -= trimmed.matches('}').count() as i64;
                if depth < 1 {
                    return None;
                }
            }
            Phase::Fallthrough => {
                if trimmed.starts_with('}') {
                    break; // enclosing function body closed
                }
                if let Some(expression) = return_statement_expression(trimmed) {
                    false_return = Some(expression.to_string());
                    break;
                }
                // Fail closed (#4117 review): a statement before the
                // fall-through `return` (a parameter reassignment, another
                // branch) makes the folded value wrong — unattributable.
                return None;
            }
        }
    }
    match (true_return, false_return) {
        (Some(true_expr), Some(false_expr)) => Some((true_expr, false_expr)),
        _ => None,
    }
}

/// The expression of a single-line `return <expr>;` statement, when `trimmed`
/// is exactly that shape.
fn return_statement_expression(trimmed: &str) -> Option<&str> {
    let expression = trimmed.strip_prefix("return ")?.strip_suffix(';')?.trim();
    (!expression.is_empty()).then_some(expression)
}

/// Evaluate the changed comparison at the bound operand values.
fn comparison_holds(operator: &str, lhs: f64, rhs: f64) -> Option<bool> {
    let epsilon = 1e-9 * lhs.abs().max(rhs.abs()).max(1.0);
    Some(match operator {
        "===" | "==" => (lhs - rhs).abs() <= epsilon,
        "!==" | "!=" => (lhs - rhs).abs() > epsilon,
        ">=" => lhs - rhs >= -epsilon,
        ">" => lhs - rhs > epsilon,
        "<=" => rhs - lhs >= -epsilon,
        "<" => rhs - lhs > epsilon,
        _ => return None,
    })
}

/// Constant-fold a numeric expression with `bindings` (identifier → value).
/// Supports decimal literals, bound identifiers, `+ - * / %`, unary minus,
/// and parentheses. Any other construct — unbound identifiers, calls, member
/// access, strings — fails the fold; the caller then skips the check rather
/// than approximating.
fn fold_numeric_expression(expression: &str, bindings: &[(String, f64)]) -> Option<f64> {
    let mut folder = Folder {
        chars: expression.chars().collect(),
        position: 0,
        bindings,
    };
    let value = folder.parse_expression()?;
    folder.skip_whitespace();
    if folder.position != folder.chars.len() {
        return None; // trailing tokens: not a pure numeric expression
    }
    Some(value)
}

struct Folder<'a> {
    chars: Vec<char>,
    position: usize,
    bindings: &'a [(String, f64)],
}

impl Folder<'_> {
    fn skip_whitespace(&mut self) {
        while self
            .chars
            .get(self.position)
            .is_some_and(|ch| ch.is_whitespace())
        {
            self.position += 1;
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.position).copied()
    }

    fn parse_expression(&mut self) -> Option<f64> {
        let mut left = self.parse_term()?;
        loop {
            self.skip_whitespace();
            match self.peek() {
                Some('+') => {
                    self.position += 1;
                    left += self.parse_term()?;
                }
                Some('-') => {
                    self.position += 1;
                    left -= self.parse_term()?;
                }
                _ => return Some(left),
            }
        }
    }

    fn parse_term(&mut self) -> Option<f64> {
        let mut left = self.parse_unary()?;
        loop {
            self.skip_whitespace();
            match self.peek() {
                Some('*') => {
                    self.position += 1;
                    left *= self.parse_unary()?;
                }
                Some('/') => {
                    self.position += 1;
                    let divisor = self.parse_unary()?;
                    if divisor == 0.0 {
                        return None;
                    }
                    left /= divisor;
                }
                Some('%') => {
                    self.position += 1;
                    let divisor = self.parse_unary()?;
                    if divisor == 0.0 {
                        return None;
                    }
                    left %= divisor;
                }
                _ => return Some(left),
            }
        }
    }

    fn parse_unary(&mut self) -> Option<f64> {
        self.skip_whitespace();
        match self.peek()? {
            '-' => {
                self.position += 1;
                Some(-self.parse_unary()?)
            }
            '+' => {
                self.position += 1;
                self.parse_unary()
            }
            '(' => {
                self.position += 1;
                let value = self.parse_expression()?;
                self.skip_whitespace();
                if self.peek() != Some(')') {
                    return None;
                }
                self.position += 1;
                Some(value)
            }
            ch if ch.is_ascii_digit() => self.parse_number(),
            ch if ch.is_ascii_alphabetic() || ch == '_' || ch == '$' => self.parse_binding(),
            _ => None,
        }
    }

    fn parse_number(&mut self) -> Option<f64> {
        let start = self.position;
        while self
            .chars
            .get(self.position)
            .is_some_and(|ch| ch.is_ascii_digit() || *ch == '.')
        {
            self.position += 1;
        }
        let text: String = self.chars[start..self.position].iter().collect();
        text.parse::<f64>().ok()
    }

    fn parse_binding(&mut self) -> Option<f64> {
        let start = self.position;
        while self
            .chars
            .get(self.position)
            .is_some_and(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '$')
        {
            self.position += 1;
        }
        let name: String = self.chars[start..self.position].iter().collect();
        self.bindings
            .iter()
            .find(|(bound, _)| *bound == name)
            .map(|(_, value)| *value)
    }
}

// ── Family↔oracle-kind matching (RIPR-SPEC-0104) ─────────────────────────────

/// Returns `true` when `oracle_kind` can observe the seam identified by
/// `probe_family`.
///
/// This is the single source of truth for the TS-adapter assertion-level
/// family↔kind filter. It mirrors `oracle_kind_matches_seam_kind` from
/// `test_grip_evidence.rs` (which operates on Rust `SeamKind`), adapted for
/// the TypeScript `ProbeFamily` vocabulary.
///
/// ### Mapping table (RIPR-SPEC-0104 §3)
///
/// | ProbeFamily | Excluded OracleKinds (Strong-rank only — the honesty fix) |
/// |---|---|
/// | `ErrorPath` | `ExactValue`, `RelationalCheck`, `MockExpectation` |
/// | `ReturnValue`, `Predicate`, `FieldConstruction`, `MatchArm` | `ExactErrorVariant`, `BroadError`, `MockExpectation` |
/// | `SideEffect`, `CallDeletion` | *(handled by RIPR-SPEC-0098 observation guard — no additional exclusion here)* |
/// | `StaticUnknown` | *(fail-open — all oracles admitted)* |
///
/// ### Design: fail-closed only at the cross-domain seam boundary
///
/// The HONESTY BUG addressed by this function is specifically the case where a
/// **Strong wrong-domain** oracle is credited to a seam whose family it cannot
/// observe:
///
/// - An `ExactErrorVariant` oracle (`.toThrow(DiscountError)`) is an error-path
///   discriminator. It MUST NOT count as a strong match for a `ReturnValue` or
///   `Predicate` seam — these seams emit a value, not an error.
/// - An `ExactValue` oracle (`.toBe(90)`) is a value discriminator. It MUST NOT
///   count as a strong match for an `ErrorPath` seam — the seam throws, not returns.
///
/// ### Composing with RIPR-SPEC-0098 (SideEffect observation guard)
///
/// For `SideEffect` and `CallDeletion` families, RIPR-SPEC-0098 already provides
/// a dedicated observation guard (`ts_changed_value_is_observed`) that correctly
/// handles the case where a value-shaped `ExactValue` assertion observes the owner
/// RETURN value (not the call effect). That guard fires in the
/// `else if strongest_strength >= Strong.rank()` arm of the classifier.
/// To preserve that path, `ExactValue` is NOT excluded from `SideEffect` /
/// `CallDeletion` by this function — SPEC-0098 handles those seams independently.
/// The two specs compose: SPEC-0104 handles cross-domain value↔error exclusion;
/// SPEC-0098 handles the same-domain but wrong-target SideEffect case.
///
/// ### Weak oracle kinds are always admitted
///
/// `SmokeOnly`, `Unknown`, `BroadError` (for non-error families),
/// `RelationalCheck` (for non-error families) are admitted for all concrete
/// families because they are weak/absent and do NOT contradict any family.
/// Their rank (Smoke / Unknown / Weak) prevents them from triggering
/// `Exposed` promotion regardless. Admitting them preserves accurate
/// per-kind messaging in `weak_oracle_missing_summary`.
pub(crate) fn ts_oracle_kind_matches_seam(
    oracle_kind: &OracleKind,
    probe_family: &ProbeFamily,
) -> bool {
    match probe_family {
        // ErrorPath seams: exclude STRONG VALUE oracles (ExactValue, RelationalCheck)
        // and MockExpectation.  BroadError, ExactErrorVariant, Snapshot all observe
        // an error-path change.  SmokeOnly/Unknown are weak and admitted.
        ProbeFamily::ErrorPath => !matches!(
            oracle_kind,
            OracleKind::ExactValue | OracleKind::RelationalCheck | OracleKind::MockExpectation
        ),
        // Value-family seams: exclude STRONG ERROR oracle (ExactErrorVariant)
        // and BroadError, MockExpectation.  ExactValue, WholeObjectEquality,
        // Snapshot, RelationalCheck, SmokeOnly, Unknown all admitted.
        // THIS IS THE PRIMARY FIX: ExactErrorVariant must not credit a ReturnValue seam.
        ProbeFamily::ReturnValue
        | ProbeFamily::Predicate
        | ProbeFamily::FieldConstruction
        | ProbeFamily::MatchArm => !matches!(
            oracle_kind,
            OracleKind::ExactErrorVariant | OracleKind::BroadError | OracleKind::MockExpectation
        ),
        // SideEffect / CallDeletion: defer to RIPR-SPEC-0098 observation guard.
        // All oracle kinds admitted here so that `strongest_strength` reflects
        // the actual oracle rank, letting the observation guard run in the
        // `else if strongest_strength >= Strong.rank()` arm.
        ProbeFamily::SideEffect | ProbeFamily::CallDeletion => true,
        // Fail-open for genuinely-unknown family: we cannot determine the
        // match — do not over-correct by blocking a potentially valid oracle.
        ProbeFamily::StaticUnknown => true,
    }
}

/// Compute the strongest oracle kind and strength that MATCHES the changed
/// seam's probe family, by iterating assertions at the assertion level across
/// oracle-eligible candidates (RIPR-SPEC-0104).
///
/// This is the family-aware replacement for the former per-test max over
/// `related: Vec<RelatedTest>` (which collapsed each test to one
/// `oracle_kind` via `strongest_assertion`). The collapsed kind may be
/// wrong-family (e.g., a test with both `.toThrow(DiscountError)` and
/// `.toBeGreaterThan(0)` collapses to `ExactErrorVariant` / Strong — but
/// `ExactErrorVariant` does NOT match a `ReturnValue` seam).
///
/// By filtering at the assertion level before taking the max, a multi-assertion
/// test can still contribute its family-matching assertion even when its
/// overall-strongest assertion is wrong-family — the anti-over-correction
/// invariant (RIPR-SPEC-0104 control 4).
///
/// Returns `(rank: u8, kind: OracleKind)` where rank is the
/// `oracle_strength.rank()` of the best matching assertion, and kind is its
/// `oracle_kind`. Returns `(0, OracleKind::Unknown)` when there are no
/// oracle-eligible candidates or no family-matching assertion.
pub(crate) fn strongest_family_matching_oracle(
    probe_family: &ProbeFamily,
    candidates: &[TypeScriptRelatedCandidate<'_>],
) -> (u8, OracleKind) {
    let mut best_rank: u8 = 0;
    let mut best_kind = OracleKind::Unknown;

    for candidate in candidates {
        if !candidate.relation.uses_oracle() {
            continue;
        }
        for assertion in &candidate.test.assertions {
            if !ts_oracle_kind_matches_seam(&assertion.oracle_kind, probe_family) {
                continue;
            }
            let rank = assertion.oracle_strength.rank();
            if rank > best_rank {
                best_rank = rank;
                best_kind = assertion.oracle_kind.clone();
            }
        }
    }

    (best_rank, best_kind)
}

/// Classify a changed TypeScript/JavaScript line and return a finding.
///
/// `workspace_root` enforces package-local ownership when `Some`: a test in
/// `packages/b/` will not be selected as an owner relation for a source file in
/// `packages/a/`.  Pass `None` to preserve the previous single-package
/// behaviour (used in unit tests).
///
/// `reexport_index` enables single-hop re-export tracing for test discovery.
/// Pass `&ReExportIndex::empty()` to disable (backward-compatible for unit tests).
// 8 parameters — all are structurally distinct context tokens required by the
// TypeScript classifier pipeline; bundling them would force a heap allocation
// per call.  The count is stable; no further parameters are planned.
/// Test-only convenience over [`classify_change_with_alias_state`] for the
/// 65 unit-test call sites that do not exercise the alias-load gap
/// (#4106-B); production always threads the typed gap through the extended
/// entry point.
#[cfg(test)]
#[allow(
    clippy::too_many_arguments,
    reason = "8 structurally-distinct context tokens; bundling forces heap allocation; count is stable"
)]
pub(crate) fn classify_change(
    file: &Path,
    line: usize,
    line_text: &str,
    owners: &[TypeScriptOwner],
    all_tests: &[TypeScriptTest],
    workspace_root: Option<&Path>,
    reexport_index: &ReExportIndex,
    alias_map: Option<&TsAliasMap>,
) -> Option<Finding> {
    classify_change_with_alias_state(
        file,
        line,
        line_text,
        owners,
        all_tests,
        workspace_root,
        reexport_index,
        alias_map,
        None,
    )
}

/// Like [`classify_change`], but carries the typed flag-ON alias-map load gap
/// (#4106-B) so the `typescript_path_alias_unresolved` advice names the real
/// fail-closed cause (missing / unparseable / JSONC / `extends` / unreadable
/// config) instead of telling the user to enable a flag that is already on.
#[allow(
    clippy::too_many_arguments,
    reason = "9 structurally-distinct context tokens; bundling forces heap allocation; count is stable"
)]
pub(crate) fn classify_change_with_alias_state(
    file: &Path,
    line: usize,
    line_text: &str,
    owners: &[TypeScriptOwner],
    all_tests: &[TypeScriptTest],
    workspace_root: Option<&Path>,
    reexport_index: &ReExportIndex,
    alias_map: Option<&TsAliasMap>,
    alias_unavailable: Option<&TsAliasMapLoadGap>,
) -> Option<Finding> {
    let changed_file = normalized_path(file);
    let owner = owners
        .iter()
        .filter(|owner| normalized_path(&owner.file) == changed_file)
        .find(|owner| line >= owner.start_line && line <= owner.end_line)?;
    let related_candidates =
        related_test_candidates(owner, all_tests, workspace_root, reexport_index, alias_map);
    let related = find_related_tests(owner, all_tests, workspace_root, reexport_index, alias_map);
    let bun_array_buffer_facts = collect_related_bun_array_buffer_facts(&related_candidates);
    let bun_bridge_hints = collect_related_bun_bridge_hints(&bun_array_buffer_facts);
    let mock_paths =
        collect_related_mock_paths(owner, all_tests, workspace_root, reexport_index, alias_map);
    let static_limit = static_limit_for_change(line_text, owner, &mock_paths);

    // Collect named TS-specific limitations (RIPR-SPEC-0085 §PR4 taxonomy).
    // These are ADDITIVE evidence lines; existing `static_limit_kind` is unchanged.
    let named_limitations_from_static: Vec<TypeScriptNamedLimitation> = static_limit
        .as_ref()
        .and_then(|limit| named_limitation_for_static_limit(limit, file, line))
        .into_iter()
        .collect();
    // Ownership-resolution limitations (RIPR-SPEC-0085 §PR6):
    // Emitted when a cross-package test references the owner by name but is
    // excluded by the package-local filter — the target is unresolvable.
    // Only active when workspace_root is supplied (i.e. in the live pipeline).
    let named_limitations_from_ownership: Vec<TypeScriptNamedLimitation> =
        if let Some(root) = workspace_root {
            named_limitations_for_unresolved_ownership(owner, all_tests, root)
        } else {
            Vec::new()
        };

    // Path-alias unresolved disclosure (RIPR-SPEC-0099 always-on honesty):
    // When a test has a non-relative, name-matched import that was NOT credited
    // as an owner relation, emit the limitation to explain the potential gap.
    // This fires regardless of `resolve_tsconfig_paths` (always-on disclosure).
    // The `related` vec is not yet computed at this point; use `related_candidates`
    // to derive which tests were actually credited.
    let credited_test_files: std::collections::HashSet<PathBuf> = related_candidates
        .iter()
        .map(|c| c.test.file.clone())
        .collect();
    let named_limitations_from_alias: Vec<TypeScriptNamedLimitation> =
        named_limitations_for_alias_unresolved(
            owner,
            all_tests,
            |test| credited_test_files.contains(&test.file),
            alias_map,
            alias_unavailable,
        );
    // Ghost relative import disclosure (#4104-C): when an uncredited test
    // name-calls the owner through a relative specifier that resolves to no
    // workspace file, the exclusion is correct but was silent — name it.
    let named_limitations_from_relative_import: Vec<TypeScriptNamedLimitation> =
        if let Some(root) = workspace_root {
            named_limitations_for_relative_import_unresolved(
                owner,
                all_tests,
                |test| credited_test_files.contains(&test.file),
                root,
            )
        } else {
            Vec::new()
        };
    // Oracle-based limitations fire from oracle-eligible candidates even when
    // there is no static_limit. We always compute them; they are empty when there
    // are no oracle-eligible candidates or no qualifying assertions.
    let named_limitations_from_oracle =
        named_limitations_for_oracle_candidates(owner, &related_candidates);
    // Oracle metadata evidence lines (RIPR-SPEC-0085 §PR5).
    // Emitted from the strongest oracle-eligible assertion across candidates.
    // ADDITIVE: does not change oracle_kind, oracle_strength, static_limit_kind,
    // or repair_packet_ready. At most one assertion's metadata is emitted
    // (the strongest, by oracle_strength rank) to avoid redundant evidence.
    let probe_shape = classify_probe_shape_detail(line_text);
    let oracle_metadata_lines: Vec<String> =
        collect_oracle_metadata_evidence_lines(&probe_shape.family, &related_candidates);
    let has_oracle_eligible_relation = related_candidates
        .iter()
        .any(|candidate| candidate.relation.uses_oracle());

    // RIPR-SPEC-0104: compute strongest_strength/strongest_kind at the
    // ASSERTION level, filtered by probe_family↔oracle_kind match.
    //
    // The former approach iterated `related: Vec<RelatedTest>` whose oracle_kind
    // was already collapsed to the test's overall-strongest assertion via
    // `strongest_assertion()`. That collapsed kind may be wrong-family:
    // e.g. a test with `.toThrow(DiscountError)` (Strong, ErrorPath-matching)
    // AND `.toBeGreaterThan(0)` (Weak, ReturnValue-matching) collapses to
    // `ExactErrorVariant/Strong`, which then falsely promoted a ReturnValue
    // seam to `Exposed / strong_oracle_observed`.
    //
    // We now iterate `related_candidates` (which carry the full `.test.assertions`
    // slice) and filter each assertion by `ts_oracle_kind_matches_seam`. This
    // lets a multi-assertion test contribute its family-matching assertion even
    // when its overall-strongest assertion is wrong-family (anti-over-correction).
    let (strongest_strength, strongest_kind) =
        strongest_family_matching_oracle(&probe_shape.family, &related_candidates);
    let mock_payload_oracle = related_mock_payload_oracle(&related);

    // Move flow_sink computation here so it is available to the observation
    // guard and the missing_discriminators gate below (RIPR-SPEC-0098).
    let flow_sink = typescript_flow_sink_for(&probe_shape, owner, line, line_text);

    // RIPR-SPEC-0098: observation guard — fires BEFORE promoting to Exposed.
    // When the strong-oracle precondition would hold, verify that at least one
    // strong assertion actually observes the changed sub-expression.  When this
    // proof fails, fall through to the downgraded WeaklyExposed arm instead.
    //
    // RIPR-SPEC-0027 boundary witness: a changed predicate is only `Exposed`
    // when a strong assertion observes an owner call at the changed boundary,
    // and an ambiguous fallback shape is never `Exposed`.
    let strong_oracle_present = strongest_strength >= OracleStrength::Strong.rank();
    let boundary_witnessed = !strong_oracle_present
        || ts_predicate_boundary_is_witnessed(
            &probe_shape,
            line_text,
            owner,
            &related_candidates,
            alias_map,
            workspace_root,
        );
    let observation_confirmed = strong_oracle_present
        && boundary_witnessed
        && ts_changed_value_is_observed(&probe_shape, line_text, &owner.name, &related_candidates);

    let (class, reach_state, observe_state, discriminate_state, mut missing) = if related.is_empty()
    {
        (
            ExposureClass::NoStaticPath,
            StageState::No,
            StageState::No,
            StageState::No,
            vec![no_static_path_missing(owner)],
        )
    } else if !has_oracle_eligible_relation {
        (
            ExposureClass::WeaklyExposed,
            StageState::Weak,
            StageState::Weak,
            StageState::Weak,
            vec![format!(
                "Only heuristic TypeScript test links were found for `{}`; verify the suggested test location or add a direct Jest/Vitest owner call with an exact-value assertion.",
                owner.name
            )],
        )
    } else if strongest_strength >= OracleStrength::Strong.rank() && observation_confirmed {
        (
            ExposureClass::Exposed,
            StageState::Yes,
            StageState::Yes,
            StageState::Yes,
            vec![format!(
                "Related test reaches `{}` with a `{}` oracle. Static evidence suggests the changed behavior is observed under an exact-value or exact-error-variant discriminator.",
                owner.name,
                strongest_kind.as_str()
            )],
        )
    } else if strongest_strength >= OracleStrength::Strong.rank() {
        // Strong oracle exists but observation guard failed: downgrade to
        // WeaklyExposed with a named limitation (RIPR-SPEC-0098), or the
        // predicate boundary is not witnessed (RIPR-SPEC-0027).
        let named_limitation = if boundary_witnessed {
            ts_observation_guard_limitation(&probe_shape, line_text)
        } else {
            ts_predicate_boundary_limitation(&probe_shape, line_text, &owner.name)
        };
        (
            ExposureClass::WeaklyExposed,
            StageState::Yes,
            StageState::Weak,
            StageState::Weak,
            vec![named_limitation],
        )
    } else {
        (
            ExposureClass::WeaklyExposed,
            StageState::Yes,
            StageState::Weak,
            StageState::Weak,
            vec![weak_oracle_missing_summary(
                &owner.name,
                &strongest_kind,
                &probe_shape.family,
                mock_payload_oracle.as_deref(),
            )],
        )
    };
    if let Some(limit) = &static_limit {
        missing.push(limit.missing.clone());
    }

    let missing_discriminators = if matches!(class, ExposureClass::WeaklyExposed)
        && has_oracle_eligible_relation
        && static_limit.is_none()
    {
        typescript_missing_discriminators(&probe_shape, line, line_text, flow_sink.as_ref())
    } else {
        Vec::new()
    };

    let id_path: String = file
        .display()
        .to_string()
        .chars()
        .map(|c| if c == '/' || c == '\\' { '_' } else { c })
        .collect();
    let family = probe_shape.family.clone();
    let delta = probe_shape.delta.clone();
    let expected_sinks = if probe_shape.specific {
        probes::expected_sinks(line_text, &family)
    } else {
        Vec::new()
    };
    let required_oracles = if probe_shape.specific {
        probes::required_oracles(line_text, &family)
    } else {
        Vec::new()
    };
    let owner_sym = owner.symbol_id();
    let ts_probe_id = fingerprint_probe_id(
        "probe",
        &id_path,
        "typescript_preview",
        owner_sym.0.as_str(),
        &normalize_expression(line_text),
        1,
    );
    let probe = Probe {
        id: ts_probe_id,
        location: SourceLocation::new(file.to_string_lossy().as_ref(), line, 1),
        owner: Some(owner_sym),
        family: family.clone(),
        delta,
        before: None,
        after: Some(line_text.to_string()),
        expression: line_text.to_string(),
        expected_sinks,
        required_oracles,
    };
    let actionability = typescript_actionability_for(
        &class,
        static_limit.as_ref(),
        has_oracle_eligible_relation,
        &missing_discriminators,
        observed_evidence_label(&related),
    );
    missing.push(actionability.missing_summary());

    let related_count = related.len();
    let reach_summary = format!(
        "{} related test(s) found for owner `{}`",
        related_count, owner.name
    );
    let reach = StageEvidence::new(reach_state.clone(), Confidence::Low, &reach_summary);
    let infect = StageEvidence::new(
        StageState::Unknown,
        Confidence::Low,
        "TypeScript preview adapter does not yet model infection.",
    );
    let propagate = StageEvidence::new(
        StageState::Unknown,
        Confidence::Low,
        "TypeScript preview adapter does not yet model propagation.",
    );
    let observe_summary = format!(
        "Strongest extracted oracle kind: `{}` (rank {})",
        strongest_kind.as_str(),
        strongest_strength
    );
    let observe = StageEvidence::new(observe_state, Confidence::Low, &observe_summary);
    let discriminate_summary = if strongest_strength >= OracleStrength::Strong.rank()
        && observation_confirmed
    {
        format!(
            "Related test uses a `{}` oracle; static evidence suggests the changed behavior is discriminated.",
            strongest_kind.as_str()
        )
    } else if strongest_strength >= OracleStrength::Strong.rank() && !boundary_witnessed {
        // RIPR-SPEC-0027: strong oracle exists but no assertion reaches the
        // changed predicate boundary.
        match missing_discriminators.first() {
            Some(discriminator) => format!(
                "TypeScript preview adapter: strong oracle found but no strong assertion observes an owner call at the changed predicate boundary; missing proof: `{}`.",
                discriminator.value
            ),
            None => "TypeScript preview adapter: strong oracle found but the changed predicate boundary is not statically witnessed by any strong assertion; discriminate unknown.".to_string(),
        }
    } else if strongest_strength >= OracleStrength::Strong.rank() {
        // RIPR-SPEC-0098: strong oracle exists but observation guard failed.
        "TypeScript preview adapter: strong oracle found but no assertion's observed_expression flows from the changed sub-expression; observation_unverified — discriminate unknown.".to_string()
    } else if let Some(discriminator) = missing_discriminators.first() {
        format!(
            "TypeScript preview adapter found no strong discriminator; missing proof: `{}`.",
            discriminator.value
        )
    } else {
        "TypeScript preview adapter found no strong discriminator; use `toBe` / `toEqual` / `toStrictEqual` to escalate. TypeScript `toThrow` forms remain broad error evidence until payload inspection lands.".to_string()
    };
    let discriminate =
        StageEvidence::new(discriminate_state, Confidence::Low, &discriminate_summary);

    let recommended = if let Some(limit) = &static_limit {
        format!(
            "TypeScript preview advisory: static limit `{}`; {}; no actionable repair packet is emitted.",
            limit.kind.as_str(),
            limit.repair_route
        )
    } else {
        match &class {
        ExposureClass::Exposed => {
            "TypeScript preview advisory: changed behavior is observed under a strong oracle; verify the assertion targets the changed boundary value.".to_string()
        }
        ExposureClass::NoStaticPath => {
            no_static_path_recommendation(owner)
        }
        _ if !has_oracle_eligible_relation => {
            "TypeScript preview advisory: related-test proximity is heuristic only; add a direct owner call before treating this as an actionable repair target.".to_string()
        }
        _ if let Some(discriminator) = missing_discriminators.first() => {
            weak_oracle_recommendation(
                &strongest_kind,
                &discriminator.value,
                mock_payload_oracle.as_deref(),
            )
        }
        _ if owner.owner_kind == OwnerKind::ModuleFunction => {
            format!(
                "TypeScript preview advisory: related module-initializer observer reaches `{}` but no safe target shape is available; add an exact value assertion for the exported value and keep the finding advisory until repair-card fields are complete.",
                owner.name
            )
        }
        _ => {
            "TypeScript preview advisory: add a test that exercises the changed behavior with an exact-value assertion (`toBe` / `toEqual` / `toStrictEqual`); no actionable repair packet is emitted until the target shape is explicit.".to_string()
        }
        }
    };
    let confidence_value = if matches!(class, ExposureClass::Exposed) {
        0.6
    } else {
        0.4
    };

    let mut evidence = vec![format!("owner: {}", owner.name)];
    if !probe_shape.specific {
        evidence.push("probe_fact: ambiguous_fallback".to_string());
    }
    for discriminator in &missing_discriminators {
        evidence.push(format!("missing_discriminator: {}", discriminator.value));
    }
    if let Some(oracle) = &mock_payload_oracle {
        evidence.push(format!("mock_payload_evidence: {oracle}"));
    }
    for fact in &bun_array_buffer_facts {
        evidence.push(fact.evidence_line());
    }
    for hint in &bun_bridge_hints {
        evidence.extend(hint.evidence_lines());
    }
    if let Some(limit) = &static_limit {
        evidence.extend(limit.evidence.iter().cloned());
    }
    // Emit additive named limitation evidence lines (RIPR-SPEC-0085 §PR4/PR6).
    // These lines are ADDITIVE — they do not change any existing field value.
    // `named_limitations_from_alias` fires on the always-on alias-gap disclosure
    // (RIPR-SPEC-0099): non-relative name-matched imports that were not credited.
    for named_limit in named_limitations_from_static
        .iter()
        .chain(named_limitations_from_oracle.iter())
        .chain(named_limitations_from_ownership.iter())
        .chain(named_limitations_from_alias.iter())
        .chain(named_limitations_from_relative_import.iter())
    {
        evidence.extend(named_limit.evidence_lines());
    }
    // Emit additive oracle metadata evidence lines (RIPR-SPEC-0085 §PR5).
    // ADDITIVE: oracle_kind, oracle_strength, static_limit_kind, and
    // repair_packet_ready are unchanged. These lines surface the structured
    // metadata needed for a future repair packet.
    evidence.extend(oracle_metadata_lines);
    evidence.extend(actionability.evidence(typescript_raw_evidence_ref(
        file,
        line,
        Some(owner),
        &probe.id.0,
    )));
    for candidate in related_candidates
        .iter()
        .filter(|candidate| candidate.relation.is_uncertain())
    {
        evidence.push(format!(
            "related_test_relation: {} ({})",
            candidate.relation.as_str(),
            candidate.test.name
        ));
        evidence.push(format!(
            "related_test_uncertain: {} ({})",
            candidate.relation.as_str(),
            candidate.test.name
        ));
    }
    // Resolved from the probe's own delta evidence (#3281) before the probe
    // moves into the finding: TypeScript probes are seeded from head-side
    // added lines.
    let source_currentness = crate::domain::SourceCurrentness::from_probe_delta(
        probe.before.as_deref(),
        probe.after.as_deref(),
    );
    Some(Finding {
        id: probe.id.0.clone(),
        canonical_gap: None,
        probe,
        class,
        ripr: RiprEvidence {
            reach,
            infect,
            propagate,
            reveal: RevealEvidence {
                observe,
                discriminate,
            },
        },
        confidence: confidence_value,
        evidence,
        missing,
        flow_sinks: flow_sink.into_iter().collect(),
        activation: ActivationEvidence {
            observed_values: Vec::new(),
            missing_discriminators,
        },
        stop_reasons: Vec::new(),
        related_tests: related,
        recommended_next_step: Some(recommended),
        language: Some(output_language_for(file)),
        language_status: Some(LanguageStatus::Preview),
        owner_kind: Some(owner.owner_kind),
        static_limit_kind: static_limit.map(|limit| limit.kind),
        changed_sink: None,
        observed_sink: None,
        oracle_alignment: None,
        alignment_reason: None,
        // Resolved above, before the probe moved into the finding (#3281).
        source_currentness,
    })
}

fn observed_evidence_label(related: &[RelatedTest]) -> &'static str {
    if related.iter().any(|test| {
        test.oracle
            .as_deref()
            .is_some_and(|oracle| oracle.starts_with("t."))
    }) {
        "related TypeScript `t.*` evidence"
    } else {
        "related Jest/Vitest evidence"
    }
}

pub(crate) fn no_static_path_missing(owner: &TypeScriptOwner) -> String {
    match owner.owner_kind {
        OwnerKind::Method => format!(
            "No trusted TypeScript method receiver relation for `{}`. Direct `new ClassName(...)` receiver calls are supported, but factories, dependency injection, mocked modules, prototype aliases, and dynamic property access stay ambiguous in preview.",
            owner.name
        ),
        OwnerKind::ClassMethod => format!(
            "No trusted TypeScript class-method relation for `{}`. Direct same-file or imported `Class.method(...)` calls are supported, but local shadows, mocked modules, namespace chains, dynamic member access, and missing class-name context stay ambiguous in preview.",
            owner.name
        ),
        OwnerKind::ModuleFunction => format!(
            "No trusted TypeScript module-initializer observer for `{}`. Direct `expect(IMPORTED_CONST)...` and `expect(namespace.EXPORT)...` observers are supported, but helper-derived values, shadowed aliases, dynamic initialization, and non-expect references stay advisory in preview.",
            owner.name
        ),
        _ => format!(
            "No test references `{}(` — add a test that calls the changed owner.",
            owner.name
        ),
    }
}

pub(crate) fn no_static_path_recommendation(owner: &TypeScriptOwner) -> String {
    match owner.owner_kind {
        OwnerKind::Method => {
            "TypeScript preview advisory: method receiver relation is missing context; use a direct `new ClassName(...)` receiver observer when safe, and keep factories, dependency injection, mocked modules, prototype aliases, and dynamic property access advisory.".to_string()
        }
        OwnerKind::ClassMethod => {
            "TypeScript preview advisory: class-method relation is missing context; use a direct same-file or imported `Class.method(...)` observer when safe, and keep local shadows, mocked modules, namespace chains, dynamic member access, and missing class-name context advisory.".to_string()
        }
        OwnerKind::ModuleFunction => {
            "TypeScript preview advisory: module initializer observer is missing context; add a direct `expect(IMPORTED_CONST).toBe(...)` or `expect(namespace.EXPORT).toEqual(...)` observer when safe, and keep helper-derived or dynamic initialization evidence advisory.".to_string()
        }
        _ => {
            "TypeScript preview advisory: no test references the changed owner; add a test that calls the owner and asserts the changed behavior with `toBe` / `toEqual` before any repair packet is emitted.".to_string()
        }
    }
}
