use crate::analysis::facts::OracleFact;
use crate::domain::{OracleKind, OracleStrength};

use super::classify::classify_assertion;
use super::patterns::{
    contains_macro_invocation, contains_named_enum_variant, is_custom_assertion_helper,
    is_mock_expectation_line, is_side_effect_observer_assertion, is_snapshot_assertion,
    is_unwrap_err_bound_error_assertion,
};
use crate::analysis::extract::ShadowAuthority;
use crate::analysis::extract::mask_comments_and_strings;
use crate::analysis::extract::text::extract_identifier_tokens;

pub(crate) fn extract_assertions(body: &str, start_line: usize) -> Vec<OracleFact> {
    let bound_error_vars = unwrap_err_bound_variables(body);
    // #3709: the dedicated guarded-Result-match scanner owns every recognized
    // `match <direct-call> { Ok.. Err.. }` statement. Skipping its start lines
    // keeps the generic statement joiner from swallowing the whole match block
    // through the `expect_` name sniff, so both adapter paths attribute the
    // shape through one authority.
    let guarded = guarded_result_match_scan(body, start_line);
    let mut out = Vec::new();
    let mut lines = body.lines().enumerate().peekable();
    while let Some((offset, line)) = lines.next() {
        let mut trimmed = line.trim().to_string();
        if guarded.match_start_lines.contains(&(start_line + offset)) {
            continue;
        }
        if is_assertion_line(&trimmed) {
            collect_multiline_assertion(&mut trimmed, &mut lines);
            let mut classification = classify_assertion(&trimmed);
            // RIPR-SPEC-0106: upgrade exact assertions on unwrap_err-bound
            // variables to ExactErrorVariant so the ErrorVariant seam can credit
            // them as a kind-matching discriminator. Constructor-payload
            // equality is classified as WholeObjectEquality before this
            // binding-aware pass, so include it here.
            if matches!(
                classification.kind,
                OracleKind::ExactValue | OracleKind::WholeObjectEquality
            ) && is_unwrap_err_bound_error_assertion(&trimmed, &bound_error_vars)
            {
                classification.kind = OracleKind::ExactErrorVariant;
                classification.strength = OracleStrength::Strong;
            }
            let observed_tokens = extract_identifier_tokens(&trimmed);
            out.push(OracleFact {
                line: start_line + offset,
                text: trimmed,
                kind: classification.kind,
                strength: classification.strength,
                observed_tokens,
                ok_value_observed: None,
            });
        } else if trimmed.starts_with("if ") || trimmed.starts_with("if(") {
            // #3284: a terminal `if <cond> { return Err(...) }` guard is
            // the manual expansion of a message-carrying assertion. The
            // lexical path recognizes it WITHOUT consuming the block:
            // joining the whole if-body would swallow real assertions
            // inside it (review finding — a main regression in the first
            // draft), so only the condition line plus a one-line peek at
            // the first body statement participates.
            if let Some(oracle) =
                peeked_err_return_guard_oracle(&trimmed, lines.peek(), start_line + offset)
            {
                out.push(oracle);
            }
        }
    }
    out.extend(guarded.oracles);
    out.sort_by(|left, right| left.line.cmp(&right.line).then(left.text.cmp(&right.text)));
    out
}

/// Scan a function body for terminal Err-return guards and credit each
/// as its assertion twin (#3284). Used by the ra parser path, whose
/// assertion facts come from the AST — joining the guard block here
/// cannot swallow sibling assertions.
pub(crate) fn err_return_guard_oracles(body: &str, start_line: usize) -> Vec<OracleFact> {
    let mut out = Vec::new();
    let mut lines = body.lines().enumerate().peekable();
    while let Some((offset, line)) = lines.next() {
        let mut trimmed = line.trim().to_string();
        let guard_line = trimmed.starts_with("if ") || trimmed.starts_with("if(");
        if guard_line
            && let Some(oracle) =
                err_return_guard_oracle(&mut trimmed, &mut lines, start_line + offset)
        {
            out.push(oracle);
        }
    }
    out
}

/// Result of the guarded-Result-match scan (#3709).
#[derive(Debug, Default)]
pub(crate) struct GuardedResultMatchScan {
    /// Oracle facts for recognized matches whose Err guards carry a
    /// recognized discriminator and terminate. Strength carries the guard's
    /// precision: strong for an exact error-variant pin, medium for a
    /// concrete downcast type pin.
    pub(crate) oracles: Vec<OracleFact>,
    /// Start lines of every recognized `match <direct-call> { Ok(..) ..,
    /// Err(..) .. }` statement — including guards too weak to credit — so
    /// callers suppress the generic statement joiners on them and this
    /// scanner stays the single authority for the shape.
    pub(crate) match_start_lines: std::collections::BTreeSet<usize>,
}

/// Scan a test body for guarded Result matches over direct callee results
/// (#3709): `match <callee>(..) { Ok(..) => .., Err(e) => <guard> }`.
///
/// Recognition is bounded and fail-closed (#3709 supported grammar). The
/// scrutinee must be a plain path call (`path::to::callee(args)`) — no
/// method receivers, no trailing `?`, no macro, no chains — so the observed
/// result is the callee's own. The block must hold at least one `Err(` arm
/// and at least one other arm: an `Ok(` arm (classic guarded match) or a
/// catch-all arm (the guarded-routing form
/// `Err(e) if <pin> => {}, rest => <loud failure>` with no `Ok` arm, as in
/// the historical `expect_response` harness). An oracle fact is emitted
/// only when every Err arm carries a recognized discriminator AND is
/// terminal under the bounded depth-0 statement grammar (see
/// `arm_terminates`): the arm's FIRST control transfer — in depth-0
/// statement order — must be UNCONDITIONALLY diverging
/// (`panic!`/`unreachable!`/`unimplemented!`/`todo!`/`bail!` covering
/// the statement, `return Err(..)`, `process::exit(<nonzero literal>)`),
/// or the
///   body-predicate failure form (`if !matches!(.., Type::Variant ..)`,
///   `if error != Type::Variant`, `if downcast..is_none()` whose block
///   diverges), or
/// - a guarded accept body (`Err(e) if <pin> => {}`): the guard must carry
///   the pin, the body must be trivial, and every catch-all arm must fail
///   loudly, so a guard miss routes to an observed failure.
///
/// Markers merely NESTED in `if`/`match`/closure blocks, `.unwrap()`/
/// `.expect()` statements, and conditional failures never terminate.
///
/// Recognized discriminators: an exact error-variant pin in binding
/// position (the arm pattern proper, a `matches!`/`assert_matches!` guard
/// or body pattern, or a guard equality whose compared operand is rooted
/// at the arm's error binding and whose other operand is a variant path)
/// ranks strong — every Err arm's pin is collected, so two arms pinning
/// two variants carry both; a bare concrete downcast pin ranks medium.
/// Pattern pins and guard pins gate the arm's SELECTION and always
/// participate; a BODY pin counts only when it participates in the arm's
/// divergence decision (#3731 review G2: a `let`-computed pin the control
/// flow never consumes does not gate the terminal statement). Each pin is
/// capped individually in the synthesized text and the join is not
/// truncated (#3731 review G4). Everything
/// else — wildcard arms, no-op arms, opaque or message-only guards, silent
/// catch-alls — emits no oracle and keeps the shape's existing weaker
/// meaning.
///
/// A same-named local `fn` or `let` binding in the test body defeats the
/// oracle for a BARE one-segment scrutinee (shared #3714 shadow
/// authority): a shadowed name is not the resolved callee, and crediting
/// it would be exactly the token-coincidence false-`exposed` family. A
/// qualified scrutinee (`helpers::parse`) cannot be shadowed by a local
/// binding and skips the defeat; its owner confirmation stays unverified
/// downstream (#3727 tracks qualified-path identity resolution).
///
/// This entry point runs the LEXICAL authority over the masked body — the
/// fallback path of the #3727 flag law. Parser-backed callers (the
/// rust-analyzer summarizer) pass their per-function facts through
/// [`guarded_result_match_scan_with_shadow_authority`] instead, so both
/// paths keep one shadow authority
/// (`analysis::extract::shadow::ShadowAuthority`).
pub(crate) fn guarded_result_match_scan(body: &str, start_line: usize) -> GuardedResultMatchScan {
    guarded_result_match_scan_with_shadow_authority(
        body,
        start_line,
        ShadowAuthority::LexicalMaskedBody,
    )
}

/// [`guarded_result_match_scan`] with the caller's shadow authority (#3727
/// Slice A): parser-backed summarizers pass
/// [`ShadowAuthority::ParserBodyFacts`] carrying the scanned function's
/// parser-produced body facts, and the lexical fallback keeps
/// [`ShadowAuthority::LexicalMaskedBody`] — byte-identical to the
/// pre-#3727 scan.
pub(crate) fn guarded_result_match_scan_with_shadow_authority(
    body: &str,
    start_line: usize,
    shadow_authority: ShadowAuthority<'_>,
) -> GuardedResultMatchScan {
    // Comments and string contents are erased before scanning so a
    // commented-out match, or an arm body mentioning `panic!` inside a
    // diagnostic string, never becomes evidence. Masking preserves the byte
    // layout, so line attribution stays exact.
    let masked = mask_comments_and_strings(body);
    let mut scan = GuardedResultMatchScan::default();
    // Byte offsets of every line start in the masked body: the parse works
    // on the remainder of the body from a candidate line so a scrutinee and
    // its block that span lines are seen whole.
    let mut line_starts = vec![0usize];
    for (index, byte) in masked.bytes().enumerate() {
        if byte == b'\n' {
            line_starts.push(index + 1);
        }
    }
    for (offset, &line_start) in line_starts.iter().enumerate() {
        let remainder = &masked[line_start..];
        let indent = remainder.len() - remainder.trim_start().len();
        let Some(rest) = remainder[indent..].strip_prefix("match ") else {
            continue;
        };
        let line_number = start_line + offset;
        let Some(match_shape) = parse_guarded_result_match(rest) else {
            continue;
        };
        // Shape recognized: this scanner owns the statement either way.
        scan.match_start_lines.insert(line_number);
        let Some(oracle) = guarded_match_oracle_fact(&match_shape, line_number) else {
            continue;
        };
        // A shadowed callee name is not the resolved helper (#3714 authority,
        // shared via extract::shadow): without this defeat the oracle would
        // bind a local binding's result to the owner's seam. The caller's
        // #3727 authority decides: the masked body keeps the lexical check
        // string/comment-safe, and parser-backed facts were produced from
        // real syntax so they never saw comments or strings in the first
        // place. The match's own body-relative line is the use site for the
        // positional let rule.
        // The defeat applies only to a BARE one-segment scrutinee: local
        // bindings cannot shadow an explicitly qualified path
        // (`let parse = ..` never shadows `helpers::parse`), so a shadow
        // check on the final segment would drop real evidence (#3731
        // review). Qualified-path identity resolution — including
        // imported same-named callees — stays unresolvable here and is the
        // #3727 follow-up; reveal keeps those observations unverified.
        if match_shape.path == match_shape.callee
            && shadow_authority.body_shadows_callee_at_line(&masked, &match_shape.callee, offset)
        {
            continue;
        }
        scan.oracles.push(oracle);
    }
    scan
}

struct GuardedMatchShape {
    /// The scrutinee's call path (`helpers::expect_response`).
    path: String,
    /// The scrutinee's final path segment — the name the oracle binds to.
    callee: String,
    /// Whether an `Ok(`-headed arm exists (classic form vs pure routing).
    has_ok_arm: bool,
    /// Trimmed Ok-arm bodies, one per Ok arm: the observation surface the
    /// fact's `ok_value_observed` decision reads (#3731).
    ok_bodies: Vec<String>,
    /// Err-arm pattern slices, including a trailing `if <guard>` when the
    /// arm carries one.
    err_patterns: Vec<String>,
    /// Trimmed Err-arm bodies, one per Err arm.
    err_bodies: Vec<String>,
    /// Bodies of catch-all arms (neither `Ok(`- nor `Err(`-headed): the
    /// routing targets that make guarded accept arms terminal.
    catch_all_bodies: Vec<String>,
}

/// Parse the scrutinee and arms of a match statement whose source begins
/// `match ` (the prefix is already stripped). Returns `None` for every
/// shape the bounded grammar does not own.
fn parse_guarded_result_match(rest: &str) -> Option<GuardedMatchShape> {
    // Scrutinee: everything before the match block's opening brace at
    // paren/bracket depth zero.
    let mut depth = 0i32;
    let mut brace = None;
    for (index, character) in rest.char_indices() {
        match character {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            '{' if depth == 0 => {
                brace = Some(index);
                break;
            }
            _ => {}
        }
    }
    let brace = brace?;
    let scrutinee = rest[..brace].trim();
    let block = balanced_block(&rest[brace..])?;

    // The scrutinee must be a direct path call: `<path>(<args>)` with the
    // first call's closing paren the final character. Method receivers
    // (`x.callee(..)`), chains (`callee(..).method(..)`), bare `?`
    // propagation, and macros are all rejected — the observed result must be
    // the callee's own.
    let open = scrutinee.find('(')?;
    let path = scrutinee[..open].trim();
    let path_is_plain = !path.is_empty()
        && path.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == ':'
        })
        && !path.contains('!')
        && path.split("::").all(|segment| !segment.is_empty());
    if !path_is_plain {
        return None;
    }
    if !call_ends_at(scrutinee, open) {
        return None;
    }
    let callee = path.rsplit("::").next()?.to_string();

    // Arms: walk arm by arm. A block body (`=> { .. }`) ends at its
    // matching close brace — the trailing comma is optional — while an
    // expression body ends at the next depth-0 comma (string/comment
    // content is masked, and patterns, guards, and non-block bodies can
    // only contain commas inside delimiters). Walking per arm keeps every
    // body free of its neighbors' text, so a loud catch-all can never make
    // a silent Err arm look terminal.
    let bytes = block.as_bytes();
    let mut arms: Vec<(String, String)> = Vec::new();
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        while cursor < bytes.len() && (bytes[cursor].is_ascii_whitespace() || bytes[cursor] == b',')
        {
            cursor += 1;
        }
        if cursor >= bytes.len() {
            break;
        }
        let Some(arrow) = top_level_arrow(block, cursor) else {
            break;
        };
        let pattern = block[cursor..arrow].trim();
        let body_start = arrow + 2;
        let body_start = body_start
            + block[body_start..]
                .len()
                .saturating_sub(block[body_start..].trim_start().len());
        let body_end = if bytes.get(body_start) == Some(&b'{') {
            balanced_block(&block[body_start..])
                .map(|inner| body_start + inner.len() + 2)
                .unwrap_or(bytes.len())
        } else {
            top_level_comma(block, body_start).unwrap_or(bytes.len())
        };
        if !pattern.is_empty() {
            arms.push((
                pattern.to_string(),
                block[body_start..body_end].trim().to_string(),
            ));
        }
        cursor = body_end;
    }
    let ok_arms: Vec<(String, String)> = arms
        .iter()
        .filter(|(pattern, _)| pattern.starts_with("Ok("))
        .cloned()
        .collect();
    let err_arms: Vec<(String, String)> = arms
        .iter()
        .filter(|(pattern, _)| pattern.starts_with("Err("))
        .cloned()
        .collect();
    let catch_all_bodies: Vec<String> = arms
        .iter()
        .filter(|(pattern, _)| !pattern.starts_with("Ok(") && !pattern.starts_with("Err("))
        .map(|(_, body)| body.clone())
        .collect();
    if err_arms.is_empty() || (ok_arms.is_empty() && catch_all_bodies.is_empty()) {
        return None;
    }
    Some(GuardedMatchShape {
        path: path.to_string(),
        callee,
        has_ok_arm: !ok_arms.is_empty(),
        ok_bodies: ok_arms.iter().map(|(_, body)| body.clone()).collect(),
        err_patterns: err_arms
            .iter()
            .map(|(pattern, _)| pattern.clone())
            .collect(),
        err_bodies: err_arms.iter().map(|(_, body)| body.clone()).collect(),
        catch_all_bodies,
    })
}

/// The first `=>` at delimiter depth zero at or after `from`, as a byte
/// offset into `text`.
fn top_level_arrow(text: &str, from: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    for position in from..bytes.len() {
        match bytes[position] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b'=' if depth == 0 && bytes.get(position + 1) == Some(&b'>') => {
                return Some(position);
            }
            _ => {}
        }
    }
    None
}

/// The first `,` at delimiter depth zero at or after `from`, as a byte
/// offset into `text`.
fn top_level_comma(text: &str, from: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    for (position, &byte) in bytes[from..].iter().enumerate() {
        match byte {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b',' if depth == 0 => return Some(from + position),
            _ => {}
        }
    }
    None
}

fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// The closing paren of the call opening at `open` is the scrutinee's final
/// character (fail-closed against chains, `?`, and trailing operators).
fn call_ends_at(scrutinee: &str, open: usize) -> bool {
    let mut depth = 0i32;
    for (index, character) in scrutinee[open..].char_indices() {
        match character {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return open + index == scrutinee.len() - 1;
                }
            }
            _ => {}
        }
    }
    false
}

/// The balanced `{..}` block starting at `text[0]`, or `None`.
fn balanced_block(text: &str) -> Option<&str> {
    let mut depth = 0i32;
    for (index, character) in text.char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&text[1..index]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Build the oracle fact for a recognized guarded Result match, or `None`
/// when no Err arm carries a recognized, terminating discriminator.
fn guarded_match_oracle_fact(shape: &GuardedMatchShape, line: usize) -> Option<OracleFact> {
    // Every Err arm must fail loudly on its own text. A guarded accept arm
    // (`Err(e) if <pin> => {}`) is terminal only when it can route: it has
    // a guard, its body is trivial, and every catch-all arm fails loudly.
    // Bodies are sliced per arm segment, so a loud catch-all can never
    // make a silent Err arm look terminal.
    let routed = !shape.catch_all_bodies.is_empty()
        && shape
            .catch_all_bodies
            .iter()
            .all(|body| arm_terminates(body));
    let all_terminal =
        shape
            .err_patterns
            .iter()
            .zip(shape.err_bodies.iter())
            .all(|(pattern, body)| {
                arm_terminates(body)
                    || (split_pattern_guard(pattern).1.is_some()
                        && arm_body_is_trivial(body)
                        && routed)
            });
    if !all_terminal {
        return None;
    }
    // Exact variant pins, one per Err arm, ALL collected (#3731 review: a
    // later Err arm's pin must not disappear behind the first arm's): the
    // arm pattern proper, a `matches!`/`assert_matches!` pattern in the
    // arm's body or its guard, or a guard equality (`==`/`!=`) whose
    // compared operand is rooted at the arm's error binding and whose other
    // operand is a variant path. Arbitrary path tokens elsewhere in a guard
    // never pin — exactness is not inferred from names. Pattern pins and
    // guard pins gate the arm's SELECTION, so they always participate; a
    // body pin counts only when it participates in the arm's divergence
    // decision (#3731 review G2: a `let`-computed pin the control flow
    // never consumes does not gate the terminal statement). EVERY Err arm
    // must carry a pin: one pinned arm beside an unpinned escape hatch is
    // not an exact result identity, and the match stays unrecognized
    // (fail-closed).
    let mut variant_pins: Vec<String> = Vec::new();
    let mut all_pinned = true;
    for (pattern_slice, body) in shape.err_patterns.iter().zip(shape.err_bodies.iter()) {
        let (pattern_proper, guard) = split_pattern_guard(pattern_slice);
        let binding = err_arm_binding_identifier(pattern_proper);
        let mut pinned = false;
        if contains_named_enum_variant(pattern_proper) {
            let pin = compact_whitespace(pattern_proper);
            if !variant_pins.contains(&pin) {
                variant_pins.push(pin);
            }
            pinned = true;
        }
        // Body candidates first, then guard candidates — the merged order
        // keeps pin selection stable — with only the body candidates gated
        // on divergence participation.
        let mut candidates = matches_guard_invocations(body)
            .into_iter()
            .map(|(at, pattern)| (Some(at), pattern))
            .collect::<Vec<_>>();
        if let Some(guard_text) = guard {
            candidates.extend(
                matches_guard_patterns(guard_text)
                    .into_iter()
                    .map(|pattern| (None, pattern)),
            );
        }
        for (body_at, candidate) in candidates {
            if !contains_named_enum_variant(&candidate) {
                continue;
            }
            if let Some(at) = body_at
                && !body_pin_participates(body, at, &candidate)
            {
                continue;
            }
            let pin = compact_whitespace(&candidate);
            if !variant_pins.contains(&pin) {
                variant_pins.push(pin);
            }
            pinned = true;
            break;
        }
        if let Some(guard) = guard
            && let Some(token) = guard_equality_variant_pin(guard, binding)
        {
            if !variant_pins.contains(&token) {
                variant_pins.push(token);
            }
            pinned = true;
        }
        if participating_downcast_invocation(body).is_some() {
            pinned = true;
        }
        if !pinned {
            all_pinned = false;
        }
    }
    if !all_pinned {
        return None;
    }
    // Concrete type pin: a downcast to a named error type without a variant.
    // Every downcast invocation in the body participates; the first OBSERVED,
    // divergence-participating one supplies the pin text (#3731 review
    // round 4, participation per G2).
    let downcast_pin = shape
        .err_bodies
        .iter()
        .find_map(|body| participating_downcast_invocation(body).map(|(_, invocation)| invocation));
    let (pin_text, strength) = if !variant_pins.is_empty() {
        // Every collected pin joins the synthesized text, so a downstream
        // seam whose changed variant equals ANY collected pin confirms
        // (reveal reads whole-word containment; repo grading re-parses the
        // pin list out of this same text). Each pin is capped individually
        // and the JOIN IS NOT TRUNCATED (#3731 review G4): an overall cap
        // dropped later variants from the synthesized text — and with them
        // from reveal/repo parsing — so a two-pin harness lost its second
        // discriminator. Long single pins still cap (see
        // `PIN_TEXT_MAX_CHARS`).
        (
            variant_pins
                .iter()
                .map(|pin| truncate_chars(pin, PIN_TEXT_MAX_CHARS))
                .collect::<Vec<_>>()
                .join(" | "),
            OracleStrength::Strong,
        )
    } else if let Some(pin) = downcast_pin {
        (
            truncate_chars(&pin, PIN_TEXT_MAX_CHARS),
            OracleStrength::Medium,
        )
    } else {
        // Wildcard arms, opaque predicates, and message-only diagnostics
        // stay unrecognized: exactness is never inferred from names or
        // payload text (#3709 fail-closed).
        return None;
    };
    let text = if shape.has_ok_arm {
        format!(
            "match {}(..) {{ Ok(..) => .., Err(..) => {pin_text} }}",
            shape.path
        )
    } else {
        // Guarded-routing form: the accept arm routes to a loud catch-all,
        // so the catch-all — not an `Ok` arm — closes the match.
        format!(
            "match {}(..) {{ Err(..) => {pin_text}, _ => .. }}",
            shape.path
        )
    };
    Some(OracleFact {
        line,
        observed_tokens: extract_identifier_tokens(&text),
        kind: OracleKind::GuardedResultMatch,
        strength,
        text,
        ok_value_observed: Some(ok_arms_observe_value(&shape.ok_bodies)),
    })
}

/// Whether any Ok arm of a guarded Result match observes the unwrapped
/// success value, under the bounded containment grammar (#3731 observation
/// authority, RIPR-SPEC-0175): an assertion form (`assert`, covering
/// `assert!`/`assert_eq!`/`assert_ne!`/`assert_matches!`), a `matches!`
/// invocation, an equality/inequality, or an unwrap-family inspection
/// (`.is_ok()`, `.unwrap(`, `.expect(`) anywhere in an Ok-arm body counts.
/// The bodies come from the masked source, so a string- or comment-embedded
/// marker never satisfies the rule. Bounded residuals, documented as
/// under-credit/over-credit edges of the lexical rule: (a) a payload
/// observed only OUTSIDE the arm — the match expression's own `let` binding
/// asserted in a later statement — is invisible here, so the fact reports
/// unobserved and the confirmation fails closed (parser-backed arm
/// observation rides #3727); (b) an equality on a value OTHER than the
/// unwrapped payload can satisfy the containment rule, because operand
/// resolution is exactly what the lexical view cannot do — the guard is
/// deliberately coarse and a binding-flow refinement is future work.
fn ok_arms_observe_value(ok_bodies: &[String]) -> bool {
    const OBSERVER_MARKERS: [&str; 7] = [
        "assert",
        "matches!(",
        "==",
        "!=",
        ".is_ok()",
        ".unwrap(",
        ".expect(",
    ];
    ok_bodies
        .iter()
        .any(|body| OBSERVER_MARKERS.iter().any(|marker| body.contains(marker)))
}

/// Split an arm's pattern slice into `(pattern proper, guard)`: the first
/// whole-word `if` at delimiter depth zero starts the guard (patterns
/// cannot contain `if`, and string/comment content is already masked).
fn split_pattern_guard(pattern_slice: &str) -> (&str, Option<&str>) {
    let bytes = pattern_slice.as_bytes();
    let mut depth = 0i32;
    for position in 0..bytes.len() {
        match bytes[position] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b'i' if depth == 0
                && bytes.get(position + 1) == Some(&b'f')
                && (position == 0 || !is_ident_byte(bytes[position - 1]))
                && bytes
                    .get(position + 2)
                    .is_none_or(|next| !is_ident_byte(*next)) =>
            {
                return (
                    pattern_slice[..position].trim_end(),
                    Some(pattern_slice[position + 2..].trim_start()),
                );
            }
            _ => {}
        }
    }
    (pattern_slice, None)
}

/// The error binding identifier an Err arm's pattern proper introduces
/// (`Err(e)` -> `e`): the name a guard equality must reference to compare
/// THIS arm's error. Only a bare identifier (optionally `mut`/`ref`
/// prefixed) binds one — `Err(_)`, struct/tuple/`@` patterns, and variant
/// patterns introduce no binding name, and a guard equality in those arms
/// cannot pin through the binding (fail-closed).
fn err_arm_binding_identifier(pattern_proper: &str) -> Option<&str> {
    let inner = pattern_proper.trim().strip_prefix("Err(")?;
    let inner = inner.strip_suffix(')')?.trim();
    let mut tokens = inner.split_whitespace();
    let mut candidate = tokens.next()?;
    if candidate == "mut" || candidate == "ref" {
        candidate = tokens.next()?;
    }
    if tokens.next().is_some() || candidate.is_empty() || candidate == "_" {
        return None;
    }
    let is_binding = candidate
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_')
        && candidate
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_lowercase() || character == '_');
    is_binding.then_some(candidate)
}

/// A guard equality against an error-variant path where the compared
/// operand is rooted at the arm's error binding (#3731 review F19: an
/// equality on an UNRELATED value — `config.mode == ParseError::Bad` — is
/// not a pin of the matched error). For each depth-0 `==`/`!=` site, one
/// operand must be a `Path::Variant` token and the OTHER must start with
/// the arm's binding identifier (`e`, `e.kind()`, `e.inner.field` — a
/// field/method chain rooted at the binding), in either order
/// (`e == ParseError::Bad`, `ParseError::Bad == e`). The variant path is
/// the pin; arbitrary operands and message strings never qualify.
fn guard_equality_variant_pin(guard: &str, binding: Option<&str>) -> Option<String> {
    let binding = binding?;
    let bytes = guard.as_bytes();
    let mut depth = 0i32;
    // Start of the operand that contains the next operator: moved past each
    // top-level separator (`&&`, `||`, `,`, `;`), so the left operand of a
    // later equality does not swallow an earlier conjunct.
    let mut operand_start = 0usize;
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b',' | b';' | b'&' | b'|' if depth == 0 => {
                // `&&`/`||` skip both operator bytes; single `&`/`|` (never
                // part of a comparison operand) skip one.
                operand_start = index
                    + if bytes.get(index + 1) == Some(&bytes[index]) {
                        2
                    } else {
                        1
                    };
            }
            b'=' | b'!' if depth == 0 && bytes.get(index + 1) == Some(&b'=') => {
                let lhs = guard[operand_start..index].trim();
                let rhs_rest = guard[index + 2..].trim_start();
                let rhs_token: String = rhs_rest
                    .chars()
                    .take_while(|character| {
                        character.is_ascii_alphanumeric() || *character == '_' || *character == ':'
                    })
                    .collect();
                let lhs_token: String = lhs
                    .chars()
                    .take_while(|character| {
                        character.is_ascii_alphanumeric() || *character == '_' || *character == ':'
                    })
                    .collect();
                // The variant path on the right, binding-rooted operand on
                // the left (`e.kind() == io::ErrorKind::InvalidData`).
                if is_variant_path(&rhs_token) && operand_rooted_at(lhs, binding) {
                    return Some(rhs_token);
                }
                // Mirrored order: variant path on the left
                // (`ParseError::Bad == e`).
                if is_variant_path(&lhs_token) && operand_rooted_at(rhs_rest, binding) {
                    return Some(lhs_token);
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

/// Whether a comparison operand is a chain rooted at `binding`: the operand
/// starts with the binding identifier and continues only into a field or
/// method chain (`e`, `e.kind()`, `e.inner.field`) — never an unrelated
/// root (`config.mode`, `error_code`).
fn operand_rooted_at(operand: &str, binding: &str) -> bool {
    let Some(rest) = operand.trim_start().strip_prefix(binding) else {
        return false;
    };
    rest.chars()
        .next()
        .is_none_or(|character| !(character.is_ascii_alphanumeric() || character == '_'))
}

/// A plain path token whose final segment names an enum variant
/// (`ParseError::InvalidData`, `io::ErrorKind::InvalidData`).
fn is_variant_path(token: &str) -> bool {
    !token.is_empty()
        && token.contains("::")
        && token.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == ':'
        })
        && token.split("::").all(|segment| !segment.is_empty())
        && token.rsplit("::").next().is_some_and(|segment| {
            segment
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_uppercase())
        })
}

/// Whether a guarded accept body is trivial (`{}`): an empty or unit arm
/// routes via its guard instead of acting.
fn arm_body_is_trivial(body: &str) -> bool {
    matches!(body.trim(), "" | "{}")
}

/// Whether a (masked) Err-arm or catch-all body terminates the failure
/// loudly (#3731 review: bounded depth-0 statement grammar, replacing
/// substring containment). The body is split into top-level statements
/// (balanced delimiters; string/comment content is already masked), and
/// the statements are scanned IN ORDER: the arm's outcome is decided by the
/// FIRST control transfer — the first statement that is a `return`/`
/// process::exit` form, or that unconditionally diverges (#3731 review F21:
/// an earlier successful return swallows everything after it, so a
/// `return Ok(..)` followed by a `panic!` is NOT terminal). The arm
/// terminates only when that first transfer diverges:
/// - a statement whose whole form is a `panic!`/`unreachable!`/
///   `unimplemented!`/`todo!`/`bail!` invocation (optionally
///   `;`-terminated),
/// - a `return Err(..)` statement — in a Result-returning test the Err
///   return IS the test failing; a bare `return`, a successful
///   `return Ok(..)` / `return ()`, and any other returned value are NOT
///   terminal (#3731 review: successful exits are not loud failures),
/// - a `process::exit(..)`/`std::process::exit(..)` statement whose
///   argument is a NONZERO integer literal (`exit(0)` and non-literal
///   arguments are not terminal), or
/// - the body-predicate failure form: a depth-0 `if <cond> { .. }`
///   statement whose condition carries the NEGATED changed-error pin and
///   whose every branch terminates (recursively under the same grammar) —
///   every depth-0 statement of the then-block and else-block diverges and
///   no successful `return` sits anywhere inside them (#3731 review G3: an
///   escape path swallows the matched error) — see
///   `condition_pins_changed_error`.
///
/// Explicitly NOT accepted: failure markers nested inside `if`/`match`/
/// closure blocks of an unpinned statement (depth > 0 — a conditional
/// `panic!` fires only when its unrelated condition holds), `.unwrap()`/
/// `.expect()` statements (the unwrapped value may be unrelated to the
/// matched error), and `assert!` forms (condition-dependent divergence).
/// Those shapes keep the match unrecognized: an arm that can return
/// normally swallows the error, and crediting it would fabricate a strong
/// discriminator (fail-closed under-credit).
fn arm_terminates(body: &str) -> bool {
    first_control_transfer_diverges(body) == Some(true)
}

/// The in-order walk behind [`arm_terminates`]: `Some(diverges)` is the
/// disposition of the body's FIRST control transfer; `None` when control
/// can reach the arm's end (no transfer at all — non-terminal).
fn first_control_transfer_diverges(body: &str) -> Option<bool> {
    first_control_transfer_statement(body).map(statement_diverges)
}

/// The body's FIRST control transfer in depth-0 statement order — the
/// statement that decides the body's outcome (`return`/`process::exit`
/// form, unconditionally diverging statement, or body-predicate `if`).
/// Used by [`first_control_transfer_diverges`] and by the pin-participation
/// gate, which must know whether the decisive statement consumes a
/// computed pin (#3731 review G2).
fn first_control_transfer_statement(body: &str) -> Option<&str> {
    for statement in top_level_statements(body) {
        let statement = statement.trim();
        if statement.is_empty() {
            continue;
        }
        // A segment wrapped in one balanced brace pair — a bare nested
        // block statement — transfers control exactly when one of its own
        // top-level statements does, decided by the same in-order rule.
        // (`balanced_block` treats the first byte as the opener, so the
        // leading `{` is required here; blocks that do not span the whole
        // segment, like an `if` statement's own block, fall through to the
        // forms below.)
        if statement.starts_with('{')
            && let Some(inner) = balanced_block(statement)
            && statement.len() == inner.len() + 2
        {
            if let Some(transfer) = first_control_transfer_statement(inner) {
                return Some(transfer);
            }
            continue;
        }
        if is_control_transfer_statement(statement) || statement_diverges(statement) {
            return Some(statement);
        }
    }
    None
}

/// Whether one (masked) statement is a control-transfer FORM: a `return`
/// expression (any returned value — the divergence check decides whether
/// the transfer is loud) or a `process::exit(..)` invocation covering the
/// statement. Whole-statement diverging macros are handled by
/// [`statement_diverges`] directly.
fn is_control_transfer_statement(statement: &str) -> bool {
    let bare = statement.strip_suffix(';').unwrap_or(statement).trim_end();
    if bare == "return" || bare.starts_with("return ") || bare.starts_with("return\t") {
        return true;
    }
    [
        "::std::process::exit",
        "std::process::exit",
        "process::exit",
    ]
    .iter()
    .any(|name| process_exit_covers_statement(bare, name))
}

/// The top-level (depth-0) statements of a (masked) body: split on `;` at
/// delimiter depth zero. Masked strings and comments cannot contribute
/// delimiters, and balanced braces keep block-internal `;` below depth
/// zero, so each slice is one statement (possibly empty).
fn top_level_statements(body: &str) -> Vec<&str> {
    let mut statements = Vec::new();
    let bytes = body.as_bytes();
    let mut depth = 0i32;
    let mut start = 0usize;
    for (index, &byte) in bytes.iter().enumerate() {
        match byte {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b';' if depth == 0 => {
                statements.push(&body[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    let tail = &body[start..];
    if !tail.trim().is_empty() {
        statements.push(tail);
    }
    statements
}

/// Whether one (masked) top-level statement is UNCONDITIONALLY diverging
/// under the bounded grammar documented on [`arm_terminates`].
fn statement_diverges(statement: &str) -> bool {
    let statement = statement.trim();
    // A segment wrapped in one balanced brace pair — the brace-wrapped arm
    // body block, or a bare nested block statement — diverges exactly when
    // one of its own top-level statements does. (`balanced_block` treats
    // the first byte as the opener, so the leading `{` is required here;
    // blocks that do not span the whole segment, like an `if` statement's
    // own block, fall through to the forms below.)
    if statement.starts_with('{')
        && let Some(inner) = balanced_block(statement)
        && statement.len() == inner.len() + 2
    {
        return top_level_statements(inner)
            .iter()
            .any(|nested| statement_diverges(nested));
    }
    let statement = statement.strip_suffix(';').unwrap_or(statement).trim_end();
    if statement.is_empty() {
        return false;
    }
    // An unconditional diverging macro covering the whole statement.
    if ["panic!", "unreachable!", "unimplemented!", "todo!", "bail!"]
        .iter()
        .any(|name| invocation_covers_statement(statement, name))
    {
        return true;
    }
    // #3731 review: only `return Err(..)` is a loud failure — in a
    // Result-returning test it IS the test failing. A bare `return`, a
    // successful `return Ok(..)` / `return ()`, and any other returned
    // value end the arm normally and swallow the matched error, so they
    // are not terminal (a trailing `;` was already stripped; the whole-
    // word boundary keeps `returned_x` from qualifying).
    if statement == "return"
        || statement.starts_with("return ")
        || statement.starts_with("return\t")
    {
        let value = statement["return".len()..].trim_start();
        return value.starts_with("Err(") && invocation_covers_statement(value, "Err");
    }
    // #3731 review: `process::exit(..)` is a loud failure only when the
    // exit code is a NONZERO integer literal — `exit(0)` reports success,
    // and a non-literal argument is not statically a failure (fail-closed).
    if [
        "::std::process::exit",
        "std::process::exit",
        "process::exit",
    ]
    .iter()
    .any(|name| nonzero_process_exit_covers_statement(statement, name))
    {
        return true;
    }
    // The body-predicate failure form: the if statement IS the
    // discriminator (diverge exactly when the error misses the pin).
    if_statement_diverges(statement)
}

/// Whether `statement` is exactly `<name>(<nonzero integer literal>)` —
/// the invocation covers the statement and its exit code is a nonzero
/// integer literal (`1`, `2`, `-1`). `exit(0)` reports success and a
/// non-literal argument is not statically a failure; both fail closed to
/// non-diverging (#3731 review).
fn nonzero_process_exit_covers_statement(statement: &str, name: &str) -> bool {
    process_exit_covers_statement(statement, name)
        && process_exit_argument(statement, name).is_some_and(|argument| {
            let digits = argument.strip_prefix('-').unwrap_or(argument);
            !digits.is_empty() && digits != "0" && digits.bytes().all(|byte| byte.is_ascii_digit())
        })
}

/// Whether `statement` is exactly `<name>(..)` — the invocation opens the
/// statement and its balanced close paren is the final character — and the
/// call's argument text when it is.
fn process_exit_covers_statement(statement: &str, name: &str) -> bool {
    process_exit_argument(statement, name).is_some()
}

fn process_exit_argument<'a>(statement: &'a str, name: &str) -> Option<&'a str> {
    let rest = statement.strip_prefix(name)?;
    let rest = rest.trim_start();
    let after_open = rest.strip_prefix('(')?;
    let mut depth = 1i32;
    for (index, byte) in after_open.bytes().enumerate() {
        match byte {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                depth -= 1;
                if depth == 0 {
                    if !after_open[index + 1..].trim().is_empty() {
                        return None;
                    }
                    return Some(after_open[..index].trim());
                }
            }
            _ => {}
        }
    }
    None
}

fn invocation_covers_statement(statement: &str, name: &str) -> bool {
    let Some(rest) = statement.strip_prefix(name) else {
        return false;
    };
    let rest = rest.trim_start();
    let Some(after_open) = rest.strip_prefix('(') else {
        return false;
    };
    let mut depth = 1i32;
    for (index, byte) in after_open.bytes().enumerate() {
        match byte {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                depth -= 1;
                if depth == 0 {
                    return after_open[index + 1..].trim().is_empty();
                }
            }
            _ => {}
        }
    }
    false
}

/// The parts of a depth-0 `if <cond> { .. } [else ..]` statement: the
/// condition, the then-block's inner text, and the `else` tail when one is
/// present. `None` for every other statement form.
struct IfParts<'a> {
    condition: &'a str,
    block: &'a str,
    else_tail: Option<&'a str>,
}

/// Split a depth-0 `if` statement into its condition, then-block, and
/// optional `else` tail (the text after the then-block's closing brace).
fn depth_zero_if_parts(statement: &str) -> Option<IfParts<'_>> {
    let rest = statement.strip_prefix("if")?;
    if rest
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return None;
    }
    let rest = rest.trim_start();
    let mut depth = 0i32;
    let mut brace = None;
    for (index, character) in rest.char_indices() {
        match character {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            '{' if depth == 0 => {
                brace = Some(index);
                break;
            }
            _ => {}
        }
    }
    let brace = brace?;
    let condition = rest[..brace].trim();
    let after = &rest[brace..];
    let block = balanced_block(after)?;
    let after_block = after[block.len() + 2..].trim_start();
    let else_tail = after_block.strip_prefix("else").filter(|tail| {
        tail.chars()
            .next()
            .is_none_or(|character| !(character.is_ascii_alphanumeric() || character == '_'))
    });
    Some(IfParts {
        condition,
        block,
        else_tail,
    })
}

/// The depth-0 `if <cond> { .. }` form of the body-predicate failure
/// grammar: the condition carries the NEGATED changed-error pin and every
/// branch terminates under the bounded grammar, so a guard miss is observed.
/// See [`if_blocks_terminate`] for the branch rule (#3731 review G3: a
/// successful return beside the panic is an escape path that swallows the
/// matched error and disqualifies the form).
fn if_statement_diverges(statement: &str) -> bool {
    let Some(parts) = depth_zero_if_parts(statement) else {
        return false;
    };
    condition_pins_changed_error(parts.condition)
        && if_blocks_terminate(parts.block, parts.else_tail)
}

/// Whether an if-form's branches terminate (#3731 review G3): EVERY depth-0
/// statement of the then-block — and of the else-block when present — must
/// diverge under [`statement_diverges`] (a log-then-panic block still ends
/// in the panic, but a successful transfer beside it does not), no
/// successful `return` may sit anywhere inside either block at any depth
/// (an escape path ends the arm without failing the test), and an
/// `else if` chain must itself terminate under the same rule — a chain that
/// can fall through ends the arm normally. An absent else does not
/// disqualify: the pin-miss branch is the observed route and the arm's own
/// first-transfer walk handles what follows the `if`.
fn if_blocks_terminate(block: &str, else_tail: Option<&str>) -> bool {
    let all_diverging = |text: &str| {
        top_level_statements(text).iter().all(|statement| {
            let statement = statement.trim();
            statement.is_empty() || statement_diverges(statement)
        })
    };
    if !all_diverging(block) || contains_successful_return(block) {
        return false;
    }
    let Some(tail) = else_tail else {
        return true;
    };
    let tail = tail.trim_start();
    if let Some(chained) = tail.strip_prefix("if")
        && chained
            .chars()
            .next()
            .is_none_or(|character| !(character.is_ascii_alphanumeric() || character == '_'))
    {
        // `else if` chain: the chained form must itself terminate under the
        // same grammar (its own condition must pin and its own branches
        // must hold the same rule) — fail closed otherwise.
        return if_statement_diverges(chained.trim_start());
    }
    if tail.starts_with('{')
        && let Some(else_block) = balanced_block(tail)
    {
        return all_diverging(else_block) && !contains_successful_return(else_block);
    }
    false
}

/// Whether a successful `return` — any returned value that is not `Err(..)` —
/// appears anywhere in the (masked) text at any depth (#3731 review G3):
/// inside an if-form's blocks such a return is an escape path that ends the
/// arm without failing the test. `return Err(..)` is the loud failure form
/// and does not trip the sweep. Closures are not distinguished — a
/// closure-internal `return` under-credits here, the same documented
/// residual as closure-nested failure markers.
fn contains_successful_return(text: &str) -> bool {
    const NEEDLE: &str = "return";
    let bytes = text.as_bytes();
    let mut from = 0usize;
    while let Some(relative) = text[from..].find(NEEDLE) {
        let at = from + relative;
        let after = at + NEEDLE.len();
        let whole_word = (at == 0 || !is_ident_byte(bytes[at - 1]))
            && bytes
                .get(after)
                .copied()
                .is_none_or(|byte| !is_ident_byte(byte));
        if whole_word {
            let value = text[after..].trim_start();
            if !value.starts_with("Err(") {
                return true;
            }
        }
        from = after;
    }
    false
}

/// Whether a depth-0 `if` condition carries the NEGATED changed-error pin:
/// the statement diverges exactly when the matched result's error is NOT
/// the pinned identity. Recognized forms:
/// - `!matches!(.., Type::Variant ..)` / `!assert_matches!(..)`,
/// - a depth-0 `!=` whose right-hand side is a variant path,
/// - a `.downcast[_ref|_mut]::<T>()` invocation tested with `.is_none()`.
///
/// A positive-form (`if matches!(.., V) { panic!() }`) or opaque
/// condition (`if diagnostics_enabled() { panic!() }`) never qualifies:
/// the divergence would key on an unrelated switch or fire on the pin
/// itself, not on a miss of the changed error (fail-closed under-credit).
fn condition_pins_changed_error(condition: &str) -> bool {
    for prefix in ["assert_matches!(", "matches!("] {
        let mut from = 0usize;
        while let Some(relative) = condition[from..].find(prefix) {
            let name_start = from + relative;
            let open = name_start + prefix.len() - 1;
            let negated = condition[..name_start].trim_end().ends_with('!');
            if negated
                && let Some(pattern) = slice_after_first_top_level_comma(&condition[open..])
                && contains_named_enum_variant(&pattern)
            {
                return true;
            }
            from = open + 1;
        }
    }
    let bytes = condition.as_bytes();
    let mut depth = 0i32;
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b'!' if depth == 0 && bytes.get(index + 1) == Some(&b'=') => {
                let rhs = condition[index + 2..].trim_start();
                let token: String = rhs
                    .chars()
                    .take_while(|character| {
                        character.is_ascii_alphanumeric() || *character == '_' || *character == ':'
                    })
                    .collect();
                if is_variant_path(&token) {
                    return true;
                }
            }
            _ => {}
        }
        index += 1;
    }
    downcast_invocation(condition).is_some() && condition.contains(".is_none()")
}

/// The pattern arguments of every `matches!`/`assert_matches!` guard in a
/// (masked) arm body: the slice after the first top-level comma.
fn matches_guard_patterns(body: &str) -> Vec<String> {
    let mut patterns = Vec::new();
    for prefix in ["matches!(", "assert_matches!("] {
        let mut from = 0usize;
        while let Some(relative) = body[from..].find(prefix) {
            let open = from + relative + prefix.len() - 1;
            if let Some(pattern) = slice_after_first_top_level_comma(&body[open..]) {
                patterns.push(pattern);
            }
            from = open + 1;
        }
    }
    patterns
}

/// The `matches!`/`assert_matches!` invocations in a (masked) arm body with
/// their byte offsets and pattern arguments, in source order. Positions let
/// the pin collector bind each body candidate to its containing statement,
/// so a computed-but-unconsumed pin can be told apart from a pin that
/// participates in the arm's divergence decision (#3731 review G2). The
/// `matches!(` substring inside `assert_matches!(` is scanned too — the
/// same double-scan [`matches_guard_patterns`] performs — so both
/// occurrences bind to the same statement and the outcome is unchanged.
fn matches_guard_invocations(body: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for prefix in ["matches!(", "assert_matches!("] {
        let mut from = 0usize;
        while let Some(relative) = body[from..].find(prefix) {
            let name_start = from + relative;
            let open = name_start + prefix.len() - 1;
            if let Some(pattern) = slice_after_first_top_level_comma(&body[open..]) {
                out.push((name_start, pattern));
            }
            from = open + 1;
        }
    }
    out.sort_by_key(|(at, _)| *at);
    out
}

/// The depth-0 statement containing byte offset `at`, as `(statement start,
/// statement)`: [`top_level_statements`] slices tile the body exactly, so
/// the offset falls in exactly one slice.
fn containing_top_level_statement(body: &str, at: usize) -> Option<(usize, &str)> {
    let mut cursor = 0usize;
    for statement in top_level_statements(body) {
        let end = cursor + statement.len();
        if at < end {
            return Some((cursor, statement));
        }
        cursor = end;
    }
    None
}

/// Whether `statement` is an `if`-headed statement (whole-word `if`).
fn is_if_statement(statement: &str) -> bool {
    statement.starts_with("if(")
        || statement.starts_with("if ")
        || statement
            .strip_prefix("if")
            .is_some_and(|rest| rest.starts_with('{'))
}

/// The variable a `let` binding head computes a pin into: the head before
/// a pin invocation contains a whole-word `let` whose binder runs to the
/// first `=`/`:` after the keyword (`let x = <pin>..`, `let ok = r.cast()..`,
/// `let typed: T = <pin>..`). `let _ =` discards and binds nothing; a head
/// with no `let` (an `if` condition, an assertion wrapper) binds nothing.
/// The LAST whole-word `let` wins, so earlier statements inside the same
/// statement window cannot shadow the binding the invocation actually
/// feeds.
fn let_binding_name(head: &str) -> Option<String> {
    const KEYWORD: &str = "let";
    let bytes = head.as_bytes();
    let mut last = None;
    let mut from = 0usize;
    while let Some(relative) = head[from..].find(KEYWORD) {
        let at = from + relative;
        let after = at + KEYWORD.len();
        let whole_word = (at == 0 || !is_ident_byte(bytes[at - 1]))
            && bytes
                .get(after)
                .is_some_and(|byte| byte.is_ascii_whitespace());
        if whole_word {
            last = Some(after);
        }
        from = at + KEYWORD.len();
    }
    let after = last?;
    let binder = &head[after..];
    let binder_end = binder.find(['=', ':']).unwrap_or(binder.len());
    let name = binder[..binder_end].trim();
    let is_name = !name.is_empty()
        && name != "_"
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
        && name
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_lowercase() || character == '_');
    is_name.then(|| name.to_string())
}

/// Whether a body-computed pin participates in the arm's divergence decision
/// (#3731 review G2). `at` is the invocation's byte offset in the (masked)
/// body and `pin_text` the candidate pattern/invocation text used for the
/// condition-membership check.
///
/// A pin whose statement computes it into a variable (`let x = matches!(..);`,
/// `let ok = cast.is_ok();`) is dead computation unless it participates:
/// either (a) the pin appears inside the condition of a depth-0 `if` that
/// guards the diverging statement (the body-predicate failure form), or (b)
/// the arm's FIRST control transfer — the statement that decides the arm's
/// outcome — references the pin's binding variable (whole-word). An
/// unconsumed `let` pin does not gate the terminal statement, so a changed
/// variant cannot affect the outcome and the pin must not credit.
///
/// A pin whose statement IS the observation (an `if` condition or an
/// asserting statement) participates by construction — except when its
/// `if`-form swallows the matched error through a successful return
/// (#3731 review G3): an escape path ends the arm before any guard miss is
/// observed, so the pin does not credit.
/// The statement body behind an arm body: a brace-wrapped arm body
/// (`{ stmt; .. }` — arm bodies keep their braces through the arm split)
/// unwraps to its inner text; an expression body is its own text. The
/// participation rule walks STATEMENTS, so the wrapper must go.
fn arm_statement_body(arm_body: &str) -> &str {
    if arm_body.starts_with('{')
        && let Some(inner) = balanced_block(arm_body)
        && arm_body.len() == inner.len() + 2
    {
        return inner;
    }
    arm_body
}

fn body_pin_participates(body: &str, at: usize, pin_text: &str) -> bool {
    let Some((statement_start, statement)) = containing_top_level_statement(body, at) else {
        return true;
    };
    let head = &body[statement_start..at];
    let Some(binding) = let_binding_name(head) else {
        let statement = arm_statement_body(statement.trim());
        return !(is_if_statement(statement) && contains_successful_return(statement));
    };
    bound_pin_participates_in_divergence(body, &binding, pin_text)
}

/// The `(a)`/`(b)` participation rule for a `let`-computed pin: see
/// [`body_pin_participates`].
fn bound_pin_participates_in_divergence(body: &str, binding: &str, pin_text: &str) -> bool {
    let body = arm_statement_body(body);
    // (a) the pin appears inside the condition of a depth-0 `if` that guards
    // the diverging statement (the body-predicate failure form).
    for statement in top_level_statements(body) {
        let statement = statement.trim();
        if let Some(parts) = depth_zero_if_parts(statement)
            && if_statement_diverges(statement)
            && parts.condition.contains(pin_text)
        {
            return true;
        }
    }
    // (b) the arm's decisive statement references the pin's binding.
    if let Some(transfer) = first_control_transfer_statement(body)
        && references_whole_word(transfer, binding)
    {
        return true;
    }
    false
}

/// Whether `text` contains `token` delimited by identifier boundaries on
/// both sides (local whole-word authority for the pin-participation gate,
/// kept beside the lexical grammar it serves).
fn references_whole_word(text: &str, token: &str) -> bool {
    if token.is_empty() {
        return false;
    }
    let bytes = text.as_bytes();
    let mut from = 0usize;
    while let Some(relative) = text[from..].find(token) {
        let at = from + relative;
        let after = at + token.len();
        let whole_word = (at == 0 || !is_ident_byte(bytes[at - 1]))
            && bytes
                .get(after)
                .copied()
                .is_none_or(|byte| !is_ident_byte(byte));
        if whole_word {
            return true;
        }
        from = at + 1;
    }
    false
}

/// Inside `text` (which starts at an opening paren), the slice after the
/// first comma at depth 1, up to the matching close paren.
fn slice_after_first_top_level_comma(text: &str) -> Option<String> {
    let mut depth = 0i32;
    let mut comma = None;
    let mut close = None;
    for (index, character) in text.char_indices() {
        match character {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(index);
                    break;
                }
            }
            // The FIRST top-level comma ends the scrutinee expression; a
            // later one (e.g. `assert_matches!(expr, Pattern, "message")`)
            // belongs to a further argument, and slicing after it would
            // return the message instead of the pattern (#3731 review).
            ',' if depth == 1 && comma.is_none() => comma = Some(index),
            _ => {}
        }
    }
    let comma = comma?;
    let close = close?;
    if comma >= close {
        return None;
    }
    Some(text[comma + 1..close].trim().to_string())
}

/// A downcast counts as a pin only when its own statement OBSERVES THE
/// CAST RESULT — the observer must bind to the invocation, not to any
/// observer token anywhere in the statement (#3731 review round 4: a
/// statement that observes ANOTHER value while the downcast result is
/// discarded no longer pins). `start` is the invocation's byte offset in
/// the (masked) body, so a repeated invocation text binds to its own
/// occurrence. Recognized observation:
/// - the text immediately after the invocation's call-closing paren
///   (whitespace skipped) starts a boolean inspection (`.is_ok()`,
///   `.is_err()`, `.is_some()`, `.is_none()`) or an observing unwrap
///   (`.expect(`, `.unwrap(` — both panic on the wrong type, so they
///   observe by construction);
/// - or the statement wraps the invocation in a whole-word
///   `matches!(`/`assert!(`/`assert_eq!(`/`assert_ne!(` macro.
///
/// `.map(`/`.map_err(` deliberately do NOT observe: they convert the value
/// without inspecting it. A discard binding (`let _ =`/
/// `let _: Type =` before the invocation) observes nothing by
/// construction. A cast whose result flows into a variable that a LATER
/// statement observes is under-credit (documented fail-closed residual;
/// parser-backed observation rides #3727).
fn downcast_statement_is_observed(body: &str, start: usize, invocation: &str) -> bool {
    let statement_start = statement_window_start(body, start);
    if is_discard_binding_head(&body[statement_start..start]) {
        return false;
    }
    if invocation_result_observed(body, start + invocation.len()) {
        return true;
    }
    ["matches!(", "assert!(", "assert_eq!(", "assert_ne!("]
        .iter()
        .any(|wrapper| statement_wraps_invocation(&body[statement_start..start], wrapper))
}

/// The start of the statement containing `start`: the position after the
/// last `;` or `}` at bracket depth zero before it. A FORWARD scan tracks
/// the real nesting depth, so brackets or nested calls before the
/// invocation cannot mis-slice the window (the reverse scan this replaced
/// compared depth zero against a nested position and could swallow
/// preceding statements — their observers must not leak into this
/// statement's window).
fn statement_window_start(body: &str, start: usize) -> usize {
    let mut window_start = 0usize;
    let mut depth = 0i32;
    for (index, character) in body[..start].char_indices() {
        match character {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' => depth -= 1,
            '}' => {
                depth -= 1;
                // A closer that lands back on depth zero ends a complete
                // block statement: the next statement starts after it.
                if depth == 0 {
                    window_start = index + 1;
                }
            }
            ';' if depth == 0 => window_start = index + 1,
            _ => {}
        }
    }
    window_start
}

/// Whether the text before an invocation is a discard binding head:
/// `let _ =` or `let _: Type =`. A discard pattern drops the value, so
/// whatever follows the invocation is never observed.
fn is_discard_binding_head(head: &str) -> bool {
    let Some(rest) = head.trim_start().strip_prefix("let ") else {
        return false;
    };
    let Some(after_pattern) = rest.strip_prefix('_') else {
        return false;
    };
    matches!(
        after_pattern.trim_start().chars().next(),
        Some('=') | Some(':')
    )
}

/// Whether the invocation's own call result is inspected: the text right
/// after the call's closing paren (whitespace skipped) must start a
/// boolean inspection or an observing unwrap. The call parens are located
/// after the turbofish's closing `>`; a shape without call parens fails
/// closed.
fn invocation_result_observed(body: &str, after_invocation: usize) -> bool {
    let rest = body[after_invocation..].trim_start();
    let Some(after_open) = rest.strip_prefix('(') else {
        return false;
    };
    let mut depth = 1i32;
    let mut close = None;
    for (index, byte) in after_open.bytes().enumerate() {
        match byte {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(index);
                    break;
                }
            }
            _ => {}
        }
    }
    let Some(close) = close else {
        return false;
    };
    let after_call = after_open[close + 1..].trim_start();
    [
        ".is_ok()",
        ".is_err()",
        ".is_some()",
        ".is_none()",
        ".expect(",
        ".unwrap(",
    ]
    .iter()
    .any(|observer| after_call.starts_with(observer))
}

/// Whether `wrapper` occurs as a whole word inside the statement window
/// before the invocation: the character before it must not continue an
/// identifier, so `debug_assert!(` (an assertion about ANOTHER value) does
/// not read as `assert!(` and `assert_matches!(` does not read as
/// `matches!(`.
fn statement_wraps_invocation(window: &str, wrapper: &str) -> bool {
    let mut from = 0usize;
    while let Some(relative) = window[from..].find(wrapper) {
        let at = from + relative;
        if at == 0 || !is_ident_byte(window.as_bytes()[at - 1]) {
            return true;
        }
        from = at + 1;
    }
    false
}

/// Every `.downcast[_ref|_mut]::<Type>()` invocation in a (masked) body,
/// in source order: the position of the leading `.` plus the invocation
/// text through the turbofish's closing `>`. ALL invocations participate
/// in pinning (#3731 review: a discarded first cast must not hide a later
/// observed one).
fn downcast_invocations(body: &str) -> Vec<(usize, String)> {
    const NEEDLE: &str = ".downcast";
    let mut out = Vec::new();
    let mut from = 0usize;
    while let Some(relative) = body[from..].find(NEEDLE) {
        let start = from + relative;
        match downcast_invocation_at(body, start) {
            Some((position, text)) => {
                out.push((position, text));
                from = position + 1;
            }
            None => {
                from = start + NEEDLE.len();
            }
        }
    }
    out
}

/// The invocation starting exactly at `start` (the leading `.`), or `None`
/// when the text there is not a recognized turbofish downcast form.
fn downcast_invocation_at(body: &str, start: usize) -> Option<(usize, String)> {
    for prefix in [".downcast_ref::<", ".downcast_mut::<", ".downcast::<"] {
        if body[start..].starts_with(prefix) {
            let turbofish_open = start + prefix.len() - 1;
            let mut depth = 0i32;
            for (index, character) in body[turbofish_open..].char_indices() {
                match character {
                    '<' => depth += 1,
                    '>' => {
                        depth -= 1;
                        if depth == 0 {
                            let end = turbofish_open + index + 1;
                            return Some((start, body[start..end].to_string()));
                        }
                    }
                    _ => {}
                }
            }
            return None;
        }
    }
    None
}

/// The first `.downcast[_ref|_mut]::<Type>()` invocation text in the body.
/// Used by the condition gate, which only asks whether a downcast appears
/// at all; pinning goes through [`observed_downcast_invocation`].
fn downcast_invocation(body: &str) -> Option<String> {
    downcast_invocations(body)
        .into_iter()
        .next()
        .map(|(_, text)| text)
}

/// The first OBSERVED, divergence-participating downcast invocation in a
/// (masked) body, as `(invocation offset, invocation text)`: every
/// invocation is considered (#3731 review — a discarded first cast no
/// longer hides a later observed one), `downcast_statement_is_observed`
/// decides observation, and a cast computed into an UNCONSUMED `let`
/// binding is skipped — it is dead computation, not a pin (#3731 review
/// G2). A cast observed directly in an `if` condition participates unless
/// that if-form swallows the matched error through a successful return
/// (#3731 review G3).
fn participating_downcast_invocation(body: &str) -> Option<(usize, String)> {
    downcast_invocations(body)
        .into_iter()
        .find(|(start, invocation)| {
            downcast_statement_is_observed(body, *start, invocation)
                && downcast_pin_participates(body, *start, invocation)
        })
}

/// The participation gate for one observed downcast invocation: see
/// [`participating_downcast_invocation`].
fn downcast_pin_participates(body: &str, start: usize, invocation: &str) -> bool {
    let Some((statement_start, statement)) = containing_top_level_statement(body, start) else {
        return true;
    };
    let head = &body[statement_start..start];
    match let_binding_name(head) {
        Some(binding) => bound_pin_participates_in_divergence(body, &binding, invocation),
        None => {
            let statement = arm_statement_body(statement.trim());
            !(is_if_statement(statement) && contains_successful_return(statement))
        }
    }
}

/// Collapse whitespace runs to single spaces (stable fact text for
/// multi-line guards).
fn compact_whitespace(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut pending_space = false;
    for character in text.chars() {
        if character.is_whitespace() {
            pending_space = !out.is_empty();
            continue;
        }
        if pending_space {
            out.push(' ');
            pending_space = false;
        }
        out.push(character);
    }
    out
}

/// The per-pin character cap for the synthesized guarded-match pin text
/// (#3731 review G4): each variant or downcast pin is truncated
/// individually at this boundary, and the joined pin list carries NO
/// overall truncation, so a multi-pin harness keeps every later variant in
/// the fact text that reveal and repo grading parse. 80 characters covers
/// the grammar's realistic variant paths while bounding one pathological
/// pin.
const PIN_TEXT_MAX_CHARS: usize = 80;

/// Truncate to at most `max` characters without splitting one.
fn truncate_chars(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let cut = text
        .char_indices()
        .nth(max)
        .map(|(index, _)| index)
        .unwrap_or(text.len());
    format!("{}...", &text[..cut])
}

/// The lexical-path guard oracle: recognize from the condition line plus
/// a single peeked body line, consuming nothing, so assertions inside the
/// guard body stay visible to the outer loop (review finding: the first
/// draft joined the whole block and swallowed them).
fn peeked_err_return_guard_oracle(
    line: &str,
    next: Option<&(usize, &str)>,
    line_number: usize,
) -> Option<OracleFact> {
    let brace = line.find('{')?;
    let head = line[..brace].trim();
    let condition = head.strip_prefix("if")?.trim();
    if condition.is_empty() {
        return None;
    }
    // Same fail-closed gate as the parser path: the Err return must be
    // the body's first statement, so a commented-out or string-embedded
    // `return Err(` never credits.
    let returns_on_line = line[brace..].replace(' ', "").starts_with("{returnErr(");
    let returns_on_next = next.is_some_and(|(_, next)| {
        let compact = next.replace(' ', "");
        compact.starts_with("returnErr(")
    });
    if !returns_on_line && !returns_on_next {
        return None;
    }
    let twin = err_return_guard_assertion(&format!("if {condition} {{ return Err(()) }}"))?;
    let classification = classify_assertion(&twin);
    let observed_tokens = extract_identifier_tokens(condition);
    Some(OracleFact {
        line: line_number,
        text: format!("if {condition} {{ return Err(..) }}"),
        kind: classification.kind,
        strength: classification.strength,
        observed_tokens,
        ok_value_observed: None,
    })
}

/// Join a guard line's continuation and recognize the assertion twin.
fn err_return_guard_oracle<'a, I>(
    trimmed: &mut String,
    lines: &mut std::iter::Peekable<I>,
    line: usize,
) -> Option<OracleFact>
where
    I: Iterator<Item = (usize, &'a str)>,
{
    collect_multiline_assertion(trimmed, lines);
    let equivalent_assertion = err_return_guard_assertion(trimmed)?;
    let classification = classify_assertion(&equivalent_assertion);
    let observed_tokens = extract_identifier_tokens(trimmed);
    Some(OracleFact {
        line,
        text: trimmed.clone(),
        kind: classification.kind,
        strength: classification.strength,
        observed_tokens,
        ok_value_observed: None,
    })
}

/// The assertion twin of a terminal Err-return guard, when the guard's
/// condition can be structurally negated (#3284).
///
/// `if <lhs> != <rhs> { return Err(...) }` is equivalent to
/// `assert!(<lhs> == <rhs>, ...)`; `if !<expr> { ... }` is equivalent to
/// `assert!(<expr>)`; a top-level `==` condition negates to `!=`. Any
/// other condition returns `None` — exactness is never inferred from
/// messages or names.
///
/// Two fail-closed gates: the Err return must be the guard body's first
/// statement (`{returnErr(` after whitespace compaction — a commented-out
/// or string-embedded `return Err(` never credits), and the condition
/// must not carry a top-level `&&`/`||` (a compound's correct negation is
/// not a single assert twin, so it stays unrecognized rather than
/// mis-twinned).
fn err_return_guard_assertion(line: &str) -> Option<String> {
    let compact: String = line.chars().filter(|ch| !ch.is_whitespace()).collect();
    if !compact.starts_with("if") || !compact.contains("{returnErr(") {
        return None;
    }
    let brace = compact.find('{')?;
    let condition = &compact[2..brace];
    if condition.is_empty() || has_top_level_boolean_operator(condition) {
        return None;
    }
    if let Some(inner) = condition
        .strip_prefix("!(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        return Some(format!("assert!({inner})"));
    }
    if let Some(inner) = condition.strip_prefix('!') {
        if !inner.starts_with('=') {
            return Some(format!("assert!({inner})"));
        }
        return None;
    }
    // Top-level comparison: split on `==`/`!=` outside any nesting. The
    // condition has no braces here (they end the condition), so a flat
    // scan at depth zero suffices. char_indices keeps every slice on a
    // char boundary — a multibyte comparison operand (`✓`) must not
    // panic the byte-index arithmetic.
    let mut depth = 0usize;
    let mut split = None;
    for (index, character) in condition.char_indices() {
        let top_level_comparison = depth == 0
            && (condition[index..].starts_with("==") || condition[index..].starts_with("!="));
        match character {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            _ if top_level_comparison => {
                split = Some(index);
                break;
            }
            _ => {}
        }
    }
    let at = split?;
    let (operator, rest) = if condition[at..].starts_with("!=") {
        ("!=", &condition[at + 2..])
    } else {
        ("==", &condition[at + 2..])
    };
    let lhs = &condition[..at];
    let rhs = rest;
    if lhs.is_empty() || rhs.is_empty() {
        return None;
    }
    let negated = if operator == "!=" { "==" } else { "!=" };
    Some(format!("assert!({lhs} {negated} {rhs})"))
}

/// Whether a compacted condition carries a top-level `&&`/`||`: its
/// correct negation is not a single `assert!` twin (#3284 fail-closed).
fn has_top_level_boolean_operator(condition: &str) -> bool {
    let mut depth = 0usize;
    for (index, character) in condition.char_indices() {
        let top_level_operator = depth == 0
            && (condition[index..].starts_with("&&") || condition[index..].starts_with("||"));
        match character {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            _ if top_level_operator => return true,
            _ => {}
        }
    }
    false
}

fn collect_multiline_assertion<'a, I>(statement: &mut String, lines: &mut std::iter::Peekable<I>)
where
    I: Iterator<Item = (usize, &'a str)>,
{
    while delimiter_depth(statement) > 0 {
        let Some((_, next_line)) = lines.peek() else {
            return;
        };
        statement.push('\n');
        statement.push_str(next_line.trim());
        let _ = lines.next();
    }
}

fn delimiter_depth(text: &str) -> i32 {
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if in_line_comment {
            if ch == '\n' {
                in_line_comment = false;
            }
            continue;
        }
        if in_block_comment {
            if ch == '*'
                && let Some('/') = chars.peek().copied()
            {
                let _ = chars.next();
                in_block_comment = false;
            }
            continue;
        }
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
            '/' => match chars.peek().copied() {
                Some('/') => {
                    let _ = chars.next();
                    in_line_comment = true;
                }
                Some('*') => {
                    let _ = chars.next();
                    in_block_comment = true;
                }
                _ => {}
            },
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    depth
}

/// Scan a test body for `let <var> = <expr>.unwrap_err()` and
/// `let <var> = <expr>.expect_err(...)` bindings.
///
/// Returns the set of variable names bound to unwrap_err results so
/// subsequent assertion lines can be recognized as error-variant oracles
/// (RIPR-SPEC-0106, Part A).
pub(crate) fn unwrap_err_bound_variables(body: &str) -> std::collections::BTreeSet<String> {
    let mut vars = std::collections::BTreeSet::new();
    // Split into statement-sized chunks on `;`/`{`/`}` so a
    // `let <ident> = <expr>.unwrap_err()` binding is recognized regardless of
    // source formatting — not only when `let` begins a trimmed source line.
    // Without this, a single-line test body (`fn t() { let e = f().unwrap_err();
    // assert_eq!(e, E::V); }`, e.g. un-rustfmt'd) hides the binding, the
    // assertion is never upgraded to ExactErrorVariant, and the seam carries a
    // contradictory `missing_discriminators` line despite being discriminated.
    for stmt in body.split([';', '{', '}']) {
        if let Some(binding) = let_binding_substring(stmt)
            && let Some(var) = extract_unwrap_err_binding(binding)
        {
            vars.insert(var);
        }
    }
    vars
}

/// Return the slice of `stmt` starting at a `let ` token boundary (preceded by
/// start-of-chunk or a non-identifier char so `let` inside a longer identifier
/// is skipped), or `None` when the chunk has no `let` binding.
fn let_binding_substring(stmt: &str) -> Option<&str> {
    let mut search_from = 0;
    while let Some(rel) = stmt[search_from..].find("let ") {
        let idx = search_from + rel;
        let prev_ok = idx == 0
            || !stmt[..idx]
                .chars()
                .next_back()
                .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_');
        if prev_ok {
            return Some(stmt[idx..].trim_start());
        }
        search_from = idx + 1;
    }
    None
}

/// If `line` is of the form `let <ident> = <expr>.unwrap_err()` or
/// `let <ident> = <expr>.expect_err(...)`, returns `Some(<ident>)`.
/// Otherwise returns `None`.
fn extract_unwrap_err_binding(line: &str) -> Option<String> {
    // Must start with `let `
    let rest = line.strip_prefix("let ")?.trim_start();
    // Grab the identifier (variable name)
    let ident_end = rest
        .char_indices()
        .find(|(_, ch)| !ch.is_ascii_alphanumeric() && *ch != '_')
        .map(|(i, _)| i)
        .unwrap_or(rest.len());
    if ident_end == 0 {
        return None;
    }
    let var = &rest[..ident_end];
    let after_var = rest[ident_end..].trim_start();
    // Allow optional type annotation: `let err: MyError = ...`
    let after_colon = if after_var.starts_with(':') {
        // Skip the type annotation up to the `=`
        after_var.find('=').map(|i| &after_var[i..])?
    } else {
        after_var
    };
    let after_eq = after_colon.strip_prefix('=')?.trim_start();
    // The expression must end in `.unwrap_err()` or `.expect_err(<anything>)`
    let expr = after_eq.trim_end_matches(';');
    if ends_with_unwrap_err(expr) || ends_with_expect_err(expr) {
        Some(var.to_string())
    } else {
        None
    }
}

fn ends_with_unwrap_err(expr: &str) -> bool {
    expr.trim_end().ends_with(".unwrap_err()")
}

fn ends_with_expect_err(expr: &str) -> bool {
    // .expect_err("...") — ends with `)` after some `.expect_err(`
    let expr = expr.trim_end();
    if !expr.ends_with(')') {
        return false;
    }
    // Find last `.expect_err(`
    expr.contains(".expect_err(")
}

pub(crate) fn extract_line_scanned_oracles(body: &str, start_line: usize) -> Vec<OracleFact> {
    let mut out = Vec::new();
    let mut lines = body.lines().enumerate().peekable();
    while let Some((offset, line)) = lines.next() {
        let mut statement = line.trim().to_string();
        if !is_line_scanned_oracle(&statement) {
            continue;
        }
        collect_multiline_assertion(&mut statement, &mut lines);
        let classification = classify_assertion(&statement);
        out.push(OracleFact {
            line: start_line + offset,
            text: statement.clone(),
            kind: classification.kind,
            strength: classification.strength,
            observed_tokens: extract_identifier_tokens(&statement),
            ok_value_observed: None,
        });
    }
    out
}

fn is_assertion_line(line: &str) -> bool {
    line.contains("assert!")
        || line.contains("assert_eq!")
        || line.contains("assert_ne!")
        || line.contains("assert_matches!")
        || line.contains("matches!")
        || is_snapshot_assertion(line)
        || is_custom_assertion_helper(line)
        || is_side_effect_observer_assertion(line)
        || line.contains("expect_")
        || line.contains(".expect(")
        || line.contains(".unwrap(")
        || line.contains("should_panic")
        || contains_macro_invocation(line, "ensure!")
}

fn is_line_scanned_oracle(line: &str) -> bool {
    !is_function_signature(line)
        && (is_custom_assertion_helper(line)
            || is_side_effect_observer_assertion(line)
            || is_mock_expectation_line(line)
            || contains_macro_invocation(line, "ensure!"))
}

fn is_function_signature(line: &str) -> bool {
    line.contains('(') && line.contains('{') && line.split_whitespace().any(|token| token == "fn")
}

#[cfg(test)]
mod spec_0106_scan_tests {
    use super::*;
    use crate::domain::OracleKind;

    #[test]
    fn line_scanner_collects_multiline_exact_fallible_oracle() -> Result<(), String> {
        let facts = extract_line_scanned_oracles(
            r#"
                ensure!(
                    state == TerminalState::Pass,
                    "terminal state must match"
                );
            "#,
            10,
        );
        let fact = facts
            .first()
            .ok_or_else(|| "expected multiline ensure oracle".to_string())?;
        if facts.len() != 1
            || fact.kind != OracleKind::ExactValue
            || fact.strength != crate::domain::OracleStrength::Strong
        {
            return Err(format!("unexpected multiline ensure oracle: {facts:?}"));
        }
        Ok(())
    }

    #[test]
    fn line_scanner_does_not_consume_expect_named_function_body() -> Result<(), String> {
        let facts = extract_line_scanned_oracles(
            r#"fn expect_metric_recorded() {
    ensure!(
        state == TerminalState::Pass,
        "terminal state must match"
    );
}"#,
            10,
        );
        let fact = facts
            .first()
            .ok_or_else(|| "expected nested ensure oracle".to_string())?;
        if facts.len() != 1
            || fact.line != 11
            || fact.kind != OracleKind::ExactValue
            || fact.strength != crate::domain::OracleStrength::Strong
            || fact.text.contains("fn expect_metric_recorded")
        {
            return Err(format!("unexpected expect-named scan result: {facts:?}"));
        }
        Ok(())
    }

    // Control 1 (POSITIVE): unwrap_err_bound_variables collects the variable name.
    #[test]
    fn unwrap_err_binding_recognized_and_variable_collected() {
        let body = r"
            let err = compute(-1).unwrap_err();
            assert_eq!(err, CalcError::Negative);
        ";
        let vars = unwrap_err_bound_variables(body);
        assert!(
            vars.contains("err"),
            "bound variable 'err' must be collected from unwrap_err() binding"
        );
    }

    #[test]
    fn expect_err_binding_also_recognized() {
        let body = r#"
            let err = compute(-1).expect_err("should fail");
            assert_eq!(err, CalcError::Negative);
        "#;
        let vars = unwrap_err_bound_variables(body);
        assert!(
            vars.contains("err"),
            "bound variable from expect_err() must also be collected"
        );
    }

    // Control 1 end-to-end: extract_assertions upgrades the oracle kind to ExactErrorVariant.
    #[test]
    fn extract_assertions_upgrades_unwrap_err_variant_to_exact_error_variant() {
        let body = r"
            let err = compute(-1).unwrap_err();
            assert_eq!(err, CalcError::Negative);
        ";
        let facts = extract_assertions(body, 1);
        let got = facts.iter().find(|f| f.text.contains("assert_eq!"));
        assert!(got.is_some(), "must find the assert_eq! fact");
        assert_eq!(
            got.map(|f| f.kind.clone()),
            Some(OracleKind::ExactErrorVariant),
            "assert_eq!(err, CalcError::Negative) on unwrap_err binding must be ExactErrorVariant"
        );
    }

    #[test]
    fn extract_assertions_upgrades_expect_err_constructor_payload_equality_to_exact_error_variant()
    -> Result<(), String> {
        let body = r#"
            let duplicate_id = "duplicate";
            let err = validate(duplicate_id)
                .expect_err("duplicate should fail");
            assert_eq!(
                err,
                CargoAllowError::new(format!("duplicate allow id `{}`", duplicate_id))
            );
        "#;
        let facts = extract_assertions(body, 1);
        let got = facts
            .iter()
            .find(|fact| fact.text.contains("CargoAllowError::new"))
            .ok_or_else(|| "must find constructor-payload equality assertion".to_string())?;
        if got.kind != OracleKind::ExactErrorVariant {
            return Err(format!(
                "constructor-payload equality on expect_err binding must be ExactErrorVariant, got {:?}",
                got.kind
            ));
        }
        Ok(())
    }

    #[test]
    fn extract_assertions_does_not_collect_after_line_comment_delimiter() -> Result<(), String> {
        let body = r#"
            assert!(first_observed); // (
            assert!(second_observed);
        "#;
        let facts = extract_assertions(body, 1);
        if facts.len() != 2 {
            return Err(format!(
                "line comment delimiter should not join assertions: {facts:?}"
            ));
        }
        if facts[0].text.contains("second_observed") {
            return Err(format!(
                "first assertion should not consume the second assertion: {:?}",
                facts[0]
            ));
        }
        Ok(())
    }

    // Formatting robustness: a single-line test body (`let` not at the start of
    // a source line, sitting mid-line after `{`) must still expose the binding.
    // Without statement-wise splitting the binding is hidden, the assertion is
    // never upgraded, and a discriminated error_path seam carries a
    // contradictory `missing_discriminators` line.
    #[test]
    fn unwrap_err_binding_detected_in_single_line_body() {
        let body =
            "fn t() { let err = compute(-1).unwrap_err(); assert_eq!(err, CalcError::Negative); }";
        let vars = unwrap_err_bound_variables(body);
        assert!(
            vars.contains("err"),
            "binding in a single-line body must be collected, not only `let`-at-line-start"
        );
    }

    // End-to-end (lexical path): a brace-prefixed binding (`let` sharing the
    // opening line of the fn body) is now detected, so the own-line assertion
    // upgrades to ExactErrorVariant. Isolates the binding-detection fix from the
    // separate line-classifier `{`-confusion that a fully single-line body trips.
    #[test]
    fn extract_assertions_upgrades_brace_prefixed_unwrap_err_variant() {
        let body = "fn t() { let err = compute(-1).unwrap_err();\n            assert_eq!(err, CalcError::Negative);\n}";
        let facts = extract_assertions(body, 1);
        let got = facts.iter().find(|f| f.text.contains("assert_eq!"));
        assert_eq!(
            got.map(|f| f.kind.clone()),
            Some(OracleKind::ExactErrorVariant),
            "brace-prefixed unwrap_err variant assertion must upgrade to ExactErrorVariant"
        );
    }

    #[test]
    fn multiple_bindings_separated_by_braces_all_collected() {
        let body = "fn t() { if c { let a = f().unwrap_err(); } let b = g().expect_err(\"x\"); }";
        let vars = unwrap_err_bound_variables(body);
        assert!(vars.contains("a") && vars.contains("b"), "got {vars:?}");
    }

    // Control 3 (GENERIC): generic assertion stays at its original classification.
    #[test]
    fn generic_assertion_on_unwrap_err_var_not_upgraded() {
        let body = r#"
            let err = compute(-1).unwrap_err();
            assert!(err.to_string().contains("error"));
        "#;
        let facts = extract_assertions(body, 1);
        let got = facts.iter().find(|f| f.text.contains("assert!"));
        assert!(got.is_some(), "must find the assert! fact");
        assert_ne!(
            got.map(|f| f.kind.clone()),
            Some(OracleKind::ExactErrorVariant),
            "generic assertion without variant token must not be upgraded to ExactErrorVariant"
        );
    }
}

#[cfg(test)]
mod guarded_result_match_tests {
    use super::{
        downcast_invocations, downcast_statement_is_observed, guarded_result_match_scan,
        mask_comments_and_strings, matches_guard_patterns, statement_window_start,
    };
    use crate::domain::{OracleKind, OracleStrength};

    /// The byte offset of the first downcast invocation in a (masked)
    /// body, the way the scanner locates one, so the unit tests below bind
    /// through the same position-aware rule the production path uses.
    fn first_downcast(body: &str) -> Result<(usize, String), String> {
        downcast_invocations(body)
            .into_iter()
            .next()
            .ok_or_else(|| "test body must contain a downcast invocation".to_string())
    }

    const POSITIVE: &str = r#"
#[test]
fn validates_ready_response() {
    let mut cursor = std::io::Cursor::new("ready");
    match expect_response(&mut cursor, "ready") {
        Ok(response) => {
            assert_eq!(response.id, "ready");
            assert_eq!(response.body, "payload");
        }
        Err(error) => {
            if !matches!(
                error.downcast_ref::<ParseError>(),
                Some(ParseError::InvalidData { .. })
            ) {
                panic!("unexpected error variant: {error}");
            }
        }
    }
}
"#;

    #[test]
    fn positive_shape_credits_strong_variant_pin_with_callee_binding() {
        let scan = guarded_result_match_scan(POSITIVE, 1);
        assert_eq!(scan.oracles.len(), 1, "got {:?}", scan.oracles);
        let fact = &scan.oracles[0];
        assert_eq!(fact.kind, OracleKind::GuardedResultMatch);
        assert_eq!(fact.strength, OracleStrength::Strong);
        assert_eq!(fact.line, 5, "the match statement's own line");
        assert!(
            fact.text.contains("match expect_response(..)"),
            "the scrutinee callee is the binding token: {}",
            fact.text
        );
        assert!(
            fact.text.contains("Some(ParseError::InvalidData{..})")
                || fact.text.contains("Some(ParseError::InvalidData { .. })"),
            "the Err-arm variant pin is named: {}",
            fact.text
        );
        assert!(
            scan.match_start_lines.contains(&5),
            "the match start line is suppressed for the generic joiners"
        );
        // RIPR-SPEC-0175: the Ok arm asserts the unwrapped response, so the
        // fact reports the success value as observed.
        assert_eq!(fact.ok_value_observed, Some(true));
    }

    #[test]
    fn downcast_without_variant_credits_medium_type_pin() {
        let body = r#"
#[test]
fn checks_error_type() {
    match parse(input) {
        Ok(value) => { assert_eq!(value, 1); }
        Err(error) => {
            if error.downcast_ref::<ParseError>().is_none() {
                panic!("wrong error type: {error}");
            }
        }
    }
}
"#;
        let scan = guarded_result_match_scan(body, 1);
        assert_eq!(scan.oracles.len(), 1, "got {:?}", scan.oracles);
        assert_eq!(scan.oracles[0].kind, OracleKind::GuardedResultMatch);
        assert_eq!(scan.oracles[0].strength, OracleStrength::Medium);
        assert!(scan.oracles[0].text.contains(".downcast_ref::<ParseError>"));
    }

    // #3731 review round 3 (devin): a downcast whose result is discarded
    // computes without discriminating — the arm must OBSERVE the cast for
    // it to pin. Pre-fix, `let _ = ..downcast::<T>()..;` granted the pin
    // and the arm's terminating panic produced a Medium oracle.
    #[test]
    fn discarded_downcast_does_not_pin() {
        let body = r#"
#[test]
fn swallows_with_discarded_cast() {
    match parse(input) {
        Ok(value) => { assert_eq!(value, 1); }
        Err(error) => {
            let _ = error.downcast_ref::<ParseError>();
            panic!("any error surfaced: {error}");
        }
    }
}
"#;
        let scan = guarded_result_match_scan(body, 1);
        assert!(
            scan.oracles.is_empty(),
            "a discarded downcast must not pin: {:?}",
            scan.oracles
        );
    }

    #[test]
    fn err_pattern_pin_credits_without_matches_macro() {
        let body = r#"
#[test]
fn pattern_pin() {
    match parse(input) {
        Ok(value) => assert_eq!(value, 1),
        Err(ParseError::InvalidData { .. }) => panic!("invalid data"),
    }
}
"#;
        let scan = guarded_result_match_scan(body, 1);
        assert_eq!(scan.oracles.len(), 1, "got {:?}", scan.oracles);
        assert_eq!(scan.oracles[0].strength, OracleStrength::Strong);
    }

    #[test]
    fn wildcard_or_noop_err_arms_never_credit() {
        let wildcard = r#"
#[test]
fn swallows_wildcard() {
    match parse(input) {
        Ok(value) => assert_eq!(value, 1),
        Err(_) => panic!("failed"),
    }
}
"#;
        let scan = guarded_result_match_scan(wildcard, 1);
        assert!(
            scan.oracles.is_empty(),
            "a wildcard Err arm has no discriminator: {:?}",
            scan.oracles
        );
        let noop = r#"
#[test]
fn swallows_error() {
    match parse(input) {
        Ok(value) => assert_eq!(value, 1),
        Err(error) => {
            let _typed = error.downcast_ref::<ParseError>();
        }
    }
}
"#;
        let scan = guarded_result_match_scan(noop, 1);
        assert!(
            scan.oracles.is_empty(),
            "a non-terminal Err arm never credits: {:?}",
            scan.oracles
        );
        // The shape is still owned by the scanner (suppressed), even without a fact.
        assert!(!scan.match_start_lines.is_empty());
    }

    #[test]
    fn wrong_error_predicate_and_message_only_diagnostics_do_not_pin() {
        let opaque = r#"
#[test]
fn opaque_predicate() {
    match parse(input) {
        Ok(value) => assert_eq!(value, 1),
        Err(error) => {
            if !error.to_string().contains("ParseError::InvalidData") {
                panic!("bad error");
            }
        }
    }
}
"#;
        let scan = guarded_result_match_scan(opaque, 1);
        assert!(
            scan.oracles.is_empty(),
            "a message-only variant mention never pins: {:?}",
            scan.oracles
        );
    }

    #[test]
    fn non_direct_scrutinees_are_not_guarded_result_matches() {
        let let_binding = r#"
#[test]
fn different_result_binding() {
    let result = expect_response(&mut cursor, "ready");
    match result {
        Ok(value) => assert_eq!(value, 1),
        Err(error) => panic!("bad error"),
    }
}
"#;
        let scan = guarded_result_match_scan(let_binding, 1);
        assert!(
            scan.oracles.is_empty() && scan.match_start_lines.is_empty(),
            "a variable scrutinee is not a direct callee result: {:?}",
            scan.oracles
        );
        let method_call = r#"
#[test]
fn method_scrutinee() {
    match client.expect_response(&mut cursor) {
        Ok(value) => assert_eq!(value, 1),
        Err(error) => panic!("bad error"),
    }
}
"#;
        let scan = guarded_result_match_scan(method_call, 1);
        assert!(scan.oracles.is_empty(), "{:?}", scan.oracles);
        let bare_question = r#"
#[test]
fn propagation_scrutinee() -> Result<(), Box<dyn std::error::Error>> {
    match expect_response(&mut cursor, "ready")? {
        Ok(value) => assert_eq!(value, 1),
        Err(error) => panic!("bad error"),
    }
}
"#;
        let scan = guarded_result_match_scan(bare_question, 1);
        assert!(scan.oracles.is_empty(), "{:?}", scan.oracles);
        let chained = r#"
#[test]
fn chained_scrutinee() {
    match expect_response(&mut cursor).map(into_response) {
        Ok(value) => assert_eq!(value, 1),
        Err(error) => panic!("bad error"),
    }
}
"#;
        let scan = guarded_result_match_scan(chained, 1);
        assert!(scan.oracles.is_empty(), "{:?}", scan.oracles);
    }

    #[test]
    fn shadowed_callee_does_not_credit() {
        let body = r#"
#[test]
fn local_shadow() {
    fn expect_response(reader: &mut Cursor, id: &str) -> Result<u32, ParseError> {
        Ok(1)
    }
    match expect_response(&mut cursor, "ready") {
        Ok(value) => assert_eq!(value, 1),
        Err(error) => panic!("{}", error),
    }
}
"#;
        let scan = guarded_result_match_scan(body, 1);
        assert!(
            scan.oracles.is_empty(),
            "a local fn shadowing the callee must not credit: {:?}",
            scan.oracles
        );
        let let_shadow = r#"
#[test]
fn let_shadow() {
    let expect_response = read_only_expect_response;
    match expect_response(&mut cursor, "ready") {
        Ok(value) => assert_eq!(value, 1),
        Err(error) => panic!("{}", error),
    }
}
"#;
        let scan = guarded_result_match_scan(let_shadow, 1);
        assert!(
            scan.oracles.is_empty(),
            "a let binding shadowing the callee must not credit: {:?}",
            scan.oracles
        );
    }

    #[test]
    fn option_and_single_arm_matches_are_not_guarded_result_matches() {
        let option_match = r#"
#[test]
fn option_match() {
    match parse(input) {
        Some(value) => assert_eq!(value, 1),
        None => panic!("none"),
    }
}
"#;
        let scan = guarded_result_match_scan(option_match, 1);
        assert!(scan.oracles.is_empty() && scan.match_start_lines.is_empty());
    }

    #[test]
    fn commented_out_match_and_string_mentions_never_credit() {
        let commented = r#"
#[test]
fn commented_out() {
    // match expect_response(&mut cursor) {
    //     Ok(v) => assert_eq!(v, 1),
    //     Err(e) => panic!("{}", e),
    // }
    let note = "match expect_response(x) { Ok(..) => .., Err(..) => panic!(x) }";
    assert_eq!(parse(input), 1);
}
"#;
        let scan = guarded_result_match_scan(commented, 1);
        assert!(
            scan.oracles.is_empty(),
            "comment/string mentions never become evidence: {:?}",
            scan.oracles
        );
    }

    #[test]
    fn qualified_path_scrutinee_binds_the_final_segment() {
        let body = r#"
#[test]
fn qualified_path() {
    match helpers::expect_response(&mut cursor, "ready") {
        Ok(value) => assert_eq!(value, 1),
        Err(ParseError::InvalidData) => panic!("invalid data"),
    }
}
"#;
        let scan = guarded_result_match_scan(body, 1);
        assert_eq!(scan.oracles.len(), 1, "{:?}", scan.oracles);
        assert!(
            scan.oracles[0]
                .text
                .contains("match helpers::expect_response(..)")
        );
    }

    // The historical `expect_response` harness shape (#13162/#3709): a
    // guarded accept arm routes to a loud catch-all; there is no `Ok` arm.
    #[test]
    fn guarded_routing_form_without_ok_arm_credits_strong() {
        let body = r#"
#[test]
fn fixture_requires_exact_response_identity() {
    match expect_response(&invalid, &id, "workspace/configuration") {
        Err(error)
            if error.kind() == io::ErrorKind::InvalidData
                && error.to_string().contains("workspace/configuration") => {}
        result => bail!(
            "fixture accepted an invalid response or lost method context: {result:?}"
        ),
    }
}
"#;
        let scan = guarded_result_match_scan(body, 1);
        assert_eq!(scan.oracles.len(), 1, "{:?}", scan.oracles);
        let fact = &scan.oracles[0];
        assert_eq!(fact.kind, OracleKind::GuardedResultMatch);
        assert_eq!(fact.strength, OracleStrength::Strong);
        assert!(
            fact.text.contains("io::ErrorKind::InvalidData"),
            "the guard equality pin is named: {}",
            fact.text
        );
        assert!(
            fact.text.contains("_ => .."),
            "the routing form is disclosed without a fabricated Ok arm: {}",
            fact.text
        );
        assert!(
            scan.match_start_lines.contains(&4),
            "the routing match start line is suppressed for the generic joiners"
        );
        // RIPR-SPEC-0175 observation authority: with no Ok arm the success
        // value flows into the catch-all unobserved, so the fact reports
        // the decision the reveal/repo gates read for return-value seams.
        assert_eq!(
            fact.ok_value_observed,
            Some(false),
            "a routing form never observes the success value"
        );
    }

    // --- #3731 observation authority (RIPR-SPEC-0175): Ok-arm observation ---

    /// A payload-ignoring Ok arm (`Ok(_) => {}`) still lets the oracle
    /// exist (the Err guard is the error-side discriminator), but the fact
    /// reports the success value as unobserved: a test that never looks at
    /// the Ok payload cannot discriminate a change to it.
    #[test]
    fn payload_ignoring_ok_arm_reports_unobserved_ok_value() {
        let body = r#"
#[test]
fn accepts_any_payload() {
    match parse(input) {
        Ok(_) => {}
        Err(ParseError::InvalidData { .. }) => panic!("invalid data"),
    }
}
"#;
        let scan = guarded_result_match_scan(body, 1);
        assert_eq!(scan.oracles.len(), 1, "got {:?}", scan.oracles);
        assert_eq!(scan.oracles[0].kind, OracleKind::GuardedResultMatch);
        assert_eq!(
            scan.oracles[0].ok_value_observed,
            Some(false),
            "an Ok arm that ignores the payload observes nothing"
        );
    }

    /// An Ok arm asserting the unwrapped value reports the observation.
    #[test]
    fn observing_ok_arm_reports_observed_ok_value() {
        let body = r#"
#[test]
fn asserts_the_payload() {
    match parse(input) {
        Ok(value) => assert_eq!(value, 3),
        Err(ParseError::InvalidData { .. }) => panic!("invalid data"),
    }
}
"#;
        let scan = guarded_result_match_scan(body, 1);
        assert_eq!(scan.oracles.len(), 1, "got {:?}", scan.oracles);
        assert_eq!(scan.oracles[0].ok_value_observed, Some(true));
    }

    /// The observation decision reads the MASKED arm bodies: an assertion
    /// marker inside a string or comment never counts as observing.
    #[test]
    fn string_or_comment_markers_never_observe_the_ok_value() {
        let string_marker = r#"
#[test]
fn mentions_but_never_observes() {
    match parse(input) {
        Ok(value) => {
            let label = "assert nothing";
            let _ = (value, label);
        }
        Err(ParseError::InvalidData { .. }) => panic!("invalid data"),
    }
}
"#;
        let scan = guarded_result_match_scan(string_marker, 1);
        assert_eq!(scan.oracles.len(), 1, "got {:?}", scan.oracles);
        assert_eq!(
            scan.oracles[0].ok_value_observed,
            Some(false),
            "a string-embedded marker is not an observation"
        );
    }

    /// Bounded-grammar residual, pinned so the boundary stays visible: the
    /// decision reads ONLY the Ok-arm bodies, so a payload that flows out
    /// through an assignment and is asserted AFTER the match is not
    /// credited here (under-credit residual; parser-backed arm observation
    /// rides #3727).
    #[test]
    fn observation_after_the_match_is_outside_the_bounded_grammar() {
        let body = r#"
#[test]
fn observes_outside_the_arm() {
    let parsed;
    match parse(input) {
        Ok(value) => parsed = value,
        Err(ParseError::InvalidData { .. }) => panic!("invalid data"),
    }
    assert_eq!(parsed, 3);
}
"#;
        let scan = guarded_result_match_scan(body, 1);
        assert_eq!(scan.oracles.len(), 1, "got {:?}", scan.oracles);
        assert_eq!(
            scan.oracles[0].ok_value_observed,
            Some(false),
            "an assertion after the match is invisible to the arm-body rule"
        );
    }

    #[test]
    fn routing_form_falsifiers_stay_non_crediting() {
        // A silent catch-all cannot observe a guard miss: routing to it
        // swallows the failure.
        let silent_catch_all = r#"
#[test]
fn routes_to_silence() {
    match expect_response(&invalid, &id, "m") {
        Err(error) if error.kind() == io::ErrorKind::InvalidData => {}
        result => {
            eprintln!("ignored: {result:?}");
        }
    }
}
"#;
        let scan = guarded_result_match_scan(silent_catch_all, 1);
        assert!(
            scan.oracles.is_empty(),
            "a silent catch-all routing target never credits: {:?}",
            scan.oracles
        );
        // A trivial Err body without a guard does not route (the arm ends
        // the match) — the swallowed error is never observed.
        let unguarded_trivial = r#"
#[test]
fn unguarded_trivial() {
    match expect_response(&invalid, &id, "m") {
        Err(error) => {}
        result => bail!("unexpected result: {result:?}"),
    }
}
"#;
        let scan = guarded_result_match_scan(unguarded_trivial, 1);
        assert!(
            scan.oracles.is_empty(),
            "an unguarded trivial Err arm never credits: {:?}",
            scan.oracles
        );
        // A guard equality against a message token (masked) or a plain
        // binding never pins, even when the shape is otherwise loud.
        let message_equality = r#"
#[test]
fn message_only_equality() {
    match expect_response(&invalid, &id, "m") {
        Err(error) if error.to_string() == "ParseError::InvalidData" => {}
        result => bail!("unexpected result: {result:?}"),
    }
}
"#;
        let scan = guarded_result_match_scan(message_equality, 1);
        assert!(
            scan.oracles.is_empty(),
            "an equality against a string literal never pins: {:?}",
            scan.oracles
        );
    }

    #[test]
    fn loud_body_err_arm_with_neighbor_pollution_stays_honest() {
        // The loud catch-all text belongs to the catch-all arm only: a
        // silent Err arm must not inherit its `bail!` as a failure action.
        let silent_err_loud_neighbor = r#"
#[test]
fn silent_err_loud_neighbor() {
    match expect_response(&invalid, &id, "m") {
        Err(error) => {
            log_failure(error);
        }
        result => bail!("unexpected result: {result:?}"),
    }
}
"#;
        let scan = guarded_result_match_scan(silent_err_loud_neighbor, 1);
        assert!(
            scan.oracles.is_empty(),
            "a silent Err arm next to a loud catch-all never credits: {:?}",
            scan.oracles
        );
    }

    #[test]
    fn multi_err_arm_requires_every_arm_pinned_and_terminal() {
        let mixed = r#"
#[test]
fn one_pin_one_wildcard() {
    match parse(input) {
        Ok(value) => assert_eq!(value, 1),
        Err(ParseError::InvalidData) => panic!("invalid data"),
        Err(other) => panic!("other: {other}"),
    }
}
"#;
        // The wildcard arm is loud but unpinned: the shape stays
        // unrecognized (every Err arm must pin), fail-closed.
        let scan = guarded_result_match_scan(mixed, 1);
        assert!(
            scan.oracles.is_empty(),
            "an unpinned (wildcard-headed) Err arm blocks the oracle: {:?}",
            scan.oracles
        );
    }

    // --- #3731 review: first top-level comma ends the matches! scrutinee ---

    /// F1: `assert_matches!(expr, Pattern, "message")` — the pattern slice
    /// must start at the FIRST post-comma argument, not the message. (The
    /// scanner double-scans the `matches!(` substring inside
    /// `assert_matches!(`, so both candidates carry the pattern; every
    /// slice must start at the pattern, never at the message.)
    #[test]
    fn assert_matches_pattern_slice_starts_after_the_first_top_level_comma() {
        let body = r#"assert_matches!(expr, ParseError::InvalidData, "custom message")"#;
        let masked = mask_comments_and_strings(body);
        let patterns = matches_guard_patterns(&masked);
        assert!(!patterns.is_empty(), "{patterns:?}");
        for pattern in &patterns {
            assert!(
                pattern.starts_with("ParseError::InvalidData"),
                "the pattern slice must start at the pattern, not the message: {patterns:?}"
            );
        }
    }

    /// F1 end-to-end: a three-argument `assert_matches!` in the Err body
    /// still pins the variant and credits the strong oracle.
    #[test]
    fn three_argument_assert_matches_body_still_pins_the_variant() {
        let body = r#"
#[test]
fn three_arg_body() {
    match parse(input) {
        Ok(value) => assert_eq!(value, 1),
        Err(error) => {
            if !assert_matches!(
                error.downcast_ref::<ParseError>(),
                Some(ParseError::InvalidData),
                "custom message"
            ) {
                panic!("invalid data");
            }
        }
    }
}
"#;
        let scan = guarded_result_match_scan(body, 1);
        assert_eq!(scan.oracles.len(), 1, "{:?}", scan.oracles);
        assert_eq!(scan.oracles[0].strength, OracleStrength::Strong);
        assert!(
            scan.oracles[0].text.contains("ParseError::InvalidData"),
            "the variant pin survives the extra message argument: {}",
            scan.oracles[0].text
        );
    }

    // --- #3731 review: bounded depth-0 terminal-statement grammar ---

    /// F4: a `panic!` nested behind an unrelated condition never makes the
    /// arm terminal — even a pinned arm stays unrecognized (fail-closed).
    #[test]
    fn conditional_panic_err_arm_is_not_terminal() {
        let body = r#"
#[test]
fn conditional_panic() {
    match parse(input) {
        Ok(value) => assert_eq!(value, 1),
        Err(ParseError::InvalidData) => {
            if diagnostics_enabled() {
                panic!("debug");
            }
        }
    }
}
"#;
        let scan = guarded_result_match_scan(body, 1);
        assert!(
            scan.oracles.is_empty(),
            "a conditional panic swallows the error when diagnostics are off: {:?}",
            scan.oracles
        );
    }

    /// F4: a conditional `return` is equally non-terminal.
    #[test]
    fn conditional_return_err_arm_is_not_terminal() {
        let body = r#"
#[test]
fn conditional_return() {
    match parse(input) {
        Ok(value) => assert_eq!(value, 1),
        Err(ParseError::InvalidData) => {
            if retriable() {
                return;
            }
        }
    }
}
"#;
        let scan = guarded_result_match_scan(body, 1);
        assert!(
            scan.oracles.is_empty(),
            "a conditional return lets the arm fall through: {:?}",
            scan.oracles
        );
    }

    /// F4: an `.unwrap()` on an unrelated value is not a failure action.
    #[test]
    fn unrelated_unwrap_err_arm_is_not_terminal() {
        let body = r#"
#[test]
fn unrelated_unwrap() {
    match parse(input) {
        Ok(value) => assert_eq!(value, 1),
        Err(ParseError::InvalidData) => {
            config.unwrap();
        }
    }
}
"#;
        let scan = guarded_result_match_scan(body, 1);
        assert!(
            scan.oracles.is_empty(),
            "an unrelated unwrap must not read as a loud failure: {:?}",
            scan.oracles
        );
    }

    /// F4: a `panic!` inside a nested closure block is not a depth-0
    /// failure action.
    #[test]
    fn nested_closure_panic_err_arm_is_not_terminal() {
        let body = r#"
#[test]
fn nested_closure() {
    match parse(input) {
        Ok(value) => assert_eq!(value, 1),
        Err(ParseError::InvalidData) => {
            let loud = || panic!("x");
            loud();
        }
    }
}
"#;
        let scan = guarded_result_match_scan(body, 1);
        assert!(
            scan.oracles.is_empty(),
            "closure-nested panic markers never terminate the arm: {:?}",
            scan.oracles
        );
    }

    /// F4: an expression statement that IS the diverging macro terminates;
    /// a trailing `;` and sibling statements change nothing.
    #[test]
    fn unconditional_failure_statements_terminate() {
        let bare_panic = r#"
#[test]
fn bare_panic() {
    match parse(input) {
        Ok(value) => assert_eq!(value, 1),
        Err(ParseError::InvalidData) => panic!("invalid data"),
    }
}
"#;
        let scan = guarded_result_match_scan(bare_panic, 1);
        assert_eq!(scan.oracles.len(), 1, "{:?}", scan.oracles);
        let semicolon_panic = r#"
#[test]
fn semicolon_panic() {
    match parse(input) {
        Ok(value) => assert_eq!(value, 1),
        Err(ParseError::InvalidData) => panic!("invalid data");
    }
}
"#;
        let scan = guarded_result_match_scan(semicolon_panic, 1);
        assert_eq!(scan.oracles.len(), 1, "{:?}", scan.oracles);
        let bail_after_log = r#"
#[test]
fn bail_after_log() {
    match parse(input) {
        Ok(value) => assert_eq!(value, 1),
        Err(ParseError::InvalidData) => {
            log(err);
            bail!("bad")
        }
    }
}
"#;
        let scan = guarded_result_match_scan(bail_after_log, 1);
        assert_eq!(scan.oracles.len(), 1, "{:?}", scan.oracles);
        let process_exit = r#"
#[test]
fn exits_process() {
    match parse(input) {
        Ok(value) => assert_eq!(value, 1),
        Err(ParseError::InvalidData) => std::process::exit(1),
    }
}
"#;
        let scan = guarded_result_match_scan(process_exit, 1);
        assert_eq!(scan.oracles.len(), 1, "{:?}", scan.oracles);
    }

    /// F4: the body-predicate failure form with a `!matches!` condition
    /// stays terminal — the negated pin condition IS the discriminator
    /// (the fixture-positive shape, also credited through the body
    /// `matches!` pin).
    #[test]
    fn negated_matches_condition_with_panic_body_stays_terminal() {
        let body = r#"
#[test]
fn pin_conditioned_panic() {
    match parse(input) {
        Ok(value) => assert_eq!(value, 1),
        Err(error) => {
            if !matches!(
                error.downcast_ref::<ParseError>(),
                Some(ParseError::InvalidData)
            ) {
                panic!("unexpected error: {error}");
            }
        }
    }
}
"#;
        let scan = guarded_result_match_scan(body, 1);
        assert_eq!(scan.oracles.len(), 1, "{:?}", scan.oracles);
        assert_eq!(scan.oracles[0].strength, OracleStrength::Strong);
    }

    /// F4 residual (fail-closed under-credit, documented): a bare
    /// `if error != Type::Variant { panic!(..) }` body terminates under
    /// the depth-0 grammar, but the pin authority reads only the arm
    /// pattern, `matches!`-family patterns, arm-guard equalities, and
    /// downcasts as exact pins — a body inequality is not one, so the
    /// shape stays unrecognized rather than guessing an exact variant
    /// from an inequality. Extending the pin authority to body
    /// inequality forms is future work, not part of this fix.
    #[test]
    fn bare_inequality_condition_terminates_but_does_not_pin() {
        let body = r#"
#[test]
fn inequality_conditioned_panic() {
    match parse(input) {
        Ok(value) => assert_eq!(value, 1),
        Err(error) => {
            if error.kind() != io::ErrorKind::InvalidData {
                panic!("unexpected kind: {error}");
            }
        }
    }
}
"#;
        let scan = guarded_result_match_scan(body, 1);
        assert!(
            scan.oracles.is_empty(),
            "a body inequality never establishes an exact variant pin: {:?}",
            scan.oracles
        );
        assert!(
            !scan.match_start_lines.is_empty(),
            "the shape is still owned (suppressed) by the scanner"
        );
    }

    // --- #3731 review: the shadow defeat applies only to bare scrutinees ---

    /// F7: a qualified scrutinee is not shadowed by a same-named local
    /// binding — `let parse = ..` never shadows `real_helpers::parse`.
    #[test]
    fn qualified_scrutinee_with_same_named_local_binding_still_credits() {
        let body = r#"
#[test]
fn qualified_not_shadowed() {
    let parse = fake_parse;
    match real_helpers::parse(input) {
        Ok(value) => assert_eq!(value, 1),
        Err(ParseError::InvalidData) => panic!("invalid data"),
    }
}
"#;
        let scan = guarded_result_match_scan(body, 1);
        assert_eq!(
            scan.oracles.len(),
            1,
            "a local binding cannot shadow a qualified path: {:?}",
            scan.oracles
        );
        assert!(
            scan.oracles[0]
                .text
                .contains("match real_helpers::parse(..)")
        );
    }

    /// F7 control: the same local binding still defeats a BARE scrutinee.
    #[test]
    fn bare_scrutinee_with_same_named_local_binding_is_defeated() {
        let body = r#"
#[test]
fn bare_shadowed() {
    let parse = fake_parse;
    match parse(input) {
        Ok(value) => assert_eq!(value, 1),
        Err(ParseError::InvalidData) => panic!("invalid data"),
    }
}
"#;
        let scan = guarded_result_match_scan(body, 1);
        assert!(
            scan.oracles.is_empty(),
            "a bare scrutinee named by a local binding stays defeated: {:?}",
            scan.oracles
        );
    }

    // --- #3731 review round 4: successful exits are not loud failures ---

    /// F9: a bare `return`, a successful `return Ok(())`, and
    /// `process::exit(0)` end the arm without failing the test, so a
    /// pinned arm built on them stays unrecognized. Pre-fix these shapes
    /// credited Strong.
    #[test]
    fn successful_exits_are_not_terminal() {
        let cases = [
            "return Ok(())",
            "return",
            "return ()",
            "std::process::exit(0)",
            "::std::process::exit(0)",
            "process::exit(0)",
            "std::process::exit(code)",
        ];
        for failure_action in cases {
            let body = format!(
                r#"
#[test]
fn swallows_through_successful_exit() {{
    match parse(input) {{
        Ok(value) => assert_eq!(value, 1),
        Err(ParseError::InvalidData) => {failure_action},
    }}
}}
"#
            );
            let scan = guarded_result_match_scan(&body, 1);
            assert!(
                scan.oracles.is_empty(),
                "a successful exit is not a loud failure: {failure_action:?} -> {:?}",
                scan.oracles
            );
        }
    }

    /// F9 positive controls: `return Err(..)` fails a Result-returning
    /// test and a nonzero `process::exit` literal is a loud failure —
    /// both keep crediting the pinned arm.
    #[test]
    fn failure_returns_and_nonzero_exits_stay_terminal() {
        let return_err = r#"
#[test]
fn fails_the_result_test() {
    match parse(input) {
        Ok(value) => assert_eq!(value, 1),
        Err(ParseError::InvalidData) => return Err(anyhow!("mismatch")),
    }
}
"#;
        let scan = guarded_result_match_scan(return_err, 1);
        assert_eq!(scan.oracles.len(), 1, "{:?}", scan.oracles);
        assert_eq!(scan.oracles[0].strength, OracleStrength::Strong);
        let exit_two = r#"
#[test]
fn exits_failing() {
    match parse(input) {
        Ok(value) => assert_eq!(value, 1),
        Err(ParseError::InvalidData) => std::process::exit(2),
    }
}
"#;
        let scan = guarded_result_match_scan(exit_two, 1);
        assert_eq!(scan.oracles.len(), 1, "{:?}", scan.oracles);
        assert_eq!(scan.oracles[0].strength, OracleStrength::Strong);
    }

    /// F21 (#3731 review): the arm's outcome is decided by its FIRST
    /// control transfer in depth-0 statement order. A successful
    /// `return Ok(())` ahead of a later `panic!` ends the arm without
    /// failing the test, so the arm does not terminate even though a
    /// diverging statement follows it.
    #[test]
    fn successful_return_ahead_of_a_later_panic_is_not_terminal() {
        let body = r#"
#[test]
fn returns_ok_then_panics() {
    match parse(input) {
        Ok(value) => assert_eq!(value, 1),
        Err(ParseError::InvalidData) => {
            return Ok(());
            panic!("unreachable after the successful return");
        }
    }
}
"#;
        let scan = guarded_result_match_scan(body, 1);
        assert!(
            scan.oracles.is_empty(),
            "a successful first transfer swallows the later panic: {:?}",
            scan.oracles
        );
    }

    /// F21 ordering control: when the diverging statement comes FIRST, the
    /// arm still terminates (the first transfer diverges).
    #[test]
    fn panic_ahead_of_a_later_return_stays_terminal() {
        let body = r#"
#[test]
fn panics_then_returns() {
    match parse(input) {
        Ok(value) => assert_eq!(value, 1),
        Err(ParseError::InvalidData) => {
            panic!("invalid data");
            return Ok(());
        }
    }
}
"#;
        let scan = guarded_result_match_scan(body, 1);
        assert_eq!(
            scan.oracles.len(),
            1,
            "the first transfer diverges, so the arm terminates: {:?}",
            scan.oracles
        );
    }

    // --- #3731 review (F19): guard equalities must reference the binding ---

    /// F19: a guard equality on an UNRELATED value
    /// (`config.mode == ParseError::Bad`) never pins the matched error —
    /// neither operand is rooted at the arm's error binding, so the
    /// otherwise-loud routing shape stays unrecognized.
    #[test]
    fn guard_equality_on_an_unrelated_value_does_not_pin() {
        let body = r#"
#[test]
fn unrelated_guard_equality() {
    match parse(input) {
        Err(other) if config.mode == ParseError::Bad => {}
        result => bail!("unexpected result: {result:?}"),
    }
}
"#;
        let scan = guarded_result_match_scan(body, 1);
        assert!(
            scan.oracles.is_empty(),
            "an equality against an unrelated value must not pin: {:?}",
            scan.oracles
        );
    }

    /// F19 positive: the guard equality compares a value rooted at the
    /// arm's error binding (`e`) with the variant path — an exact pin.
    #[test]
    fn guard_equality_on_the_error_binding_pins() {
        let direct = r#"
#[test]
fn binding_equality() {
    match parse(input) {
        Ok(value) => assert_eq!(value, 1),
        Err(e) if e == ParseError::InvalidData => panic!("invalid data"),
    }
}
"#;
        let scan = guarded_result_match_scan(direct, 1);
        assert_eq!(scan.oracles.len(), 1, "{:?}", scan.oracles);
        assert_eq!(scan.oracles[0].strength, OracleStrength::Strong);
        let mirrored = r#"
#[test]
fn mirrored_binding_equality() {
    match parse(input) {
        Ok(value) => assert_eq!(value, 1),
        Err(e) if ParseError::InvalidData == e => panic!("invalid data"),
    }
}
"#;
        let scan = guarded_result_match_scan(mirrored, 1);
        assert_eq!(scan.oracles.len(), 1, "{:?}", scan.oracles);
        assert_eq!(scan.oracles[0].strength, OracleStrength::Strong);
    }

    // --- #3731 review (F24): every Err arm's pin is collected ---

    /// F24: two Err arms pinning two variants carry BOTH pins in the
    /// synthesized text, so either variant's seam can confirm. Pre-fix the
    /// second arm's pin disappeared behind the first arm's.
    #[test]
    fn every_err_arm_pin_is_collected() {
        let body = r#"
#[test]
fn two_variant_arms() {
    match parse(input) {
        Ok(value) => assert_eq!(value, 1),
        Err(ParseError::InvalidData) => panic!("invalid data"),
        Err(ParseError::UnexpectedEof) => panic!("unexpected eof"),
    }
}
"#;
        let scan = guarded_result_match_scan(body, 1);
        assert_eq!(scan.oracles.len(), 1, "{:?}", scan.oracles);
        assert_eq!(scan.oracles[0].strength, OracleStrength::Strong);
        assert!(
            scan.oracles[0].text.contains("ParseError::InvalidData"),
            "the first arm's pin is named: {}",
            scan.oracles[0].text
        );
        assert!(
            scan.oracles[0].text.contains("ParseError::UnexpectedEof"),
            "the later arm's pin must not disappear: {}",
            scan.oracles[0].text
        );
    }

    /// F24 control: an unpinned Err arm beside a pinned one still blocks
    /// the oracle (every arm must pin) — collection does not weaken the
    /// fail-closed gate. (`multi_err_arm_requires_every_arm_pinned_and_
    /// terminal` pins the classic form; this pins the routing form.)
    #[test]
    fn unpinned_second_arm_still_blocks_collection() {
        let body = r#"
#[test]
fn pinned_then_wildcard() {
    match parse(input) {
        Err(ParseError::InvalidData) => panic!("invalid data"),
        Err(other) => bail!("other: {other}"),
    }
}
"#;
        let scan = guarded_result_match_scan(body, 1);
        assert!(
            scan.oracles.is_empty(),
            "an unpinned Err arm blocks the oracle regardless of collection: {:?}",
            scan.oracles
        );
    }

    // --- #3731 review round 4: every downcast invocation participates ---

    /// F12: a discarded first cast must not hide a later OBSERVED one —
    /// the match still pins Medium, with the observed cast's type text.
    #[test]
    fn later_observed_downcast_pins_after_a_discarded_first_cast() {
        let body = r#"
#[test]
fn discards_then_observes() {
    match parse(input) {
        Ok(value) => { assert_eq!(value, 1); }
        Err(error) => {
            let _ = error.downcast_ref::<Alpha>();
            if error.downcast_ref::<Beta>().is_none() {
                panic!("wrong error type: {error}");
            }
        }
    }
}
"#;
        let scan = guarded_result_match_scan(body, 1);
        assert_eq!(scan.oracles.len(), 1, "{:?}", scan.oracles);
        assert_eq!(scan.oracles[0].strength, OracleStrength::Medium);
        assert!(
            scan.oracles[0].text.contains(".downcast_ref::<Beta>"),
            "the pin text is the observed cast, not the discarded one: {}",
            scan.oracles[0].text
        );
        assert!(
            !scan.oracles[0].text.contains("Alpha"),
            "the discarded cast contributes no pin text: {}",
            scan.oracles[0].text
        );
    }

    // --- #3731 review round 4: expect/unwrap observe the cast ---

    /// F13: `.expect(` on the downcast panics on the wrong type, so it
    /// observes the cast and pins Medium. The terminal grammar is
    /// unchanged: the arm terminates through the negated `is_none`
    /// predicate, not through the expect.
    #[test]
    fn expected_downcast_observes_and_pins_medium() {
        let body = r#"
#[test]
fn expects_the_parse_error_type() {
    match parse(input) {
        Ok(value) => assert_eq!(value, 1),
        Err(error) => {
            if error.downcast_ref::<ParseError>().is_none() {
                panic!("expected a parse error: {error}");
            }
            let parse_error = error.downcast_ref::<ParseError>().expect("parse error");
            assert_eq!(parse_error.kind, 1);
        }
    }
}
"#;
        let scan = guarded_result_match_scan(body, 1);
        assert_eq!(scan.oracles.len(), 1, "{:?}", scan.oracles);
        assert_eq!(scan.oracles[0].strength, OracleStrength::Medium);
        assert!(
            scan.oracles[0].text.contains(".downcast_ref::<ParseError>"),
            "{}",
            scan.oracles[0].text
        );
    }

    // --- #3731 review round 4: the observer binds to the invocation ---

    /// F8: an observer for ANOTHER value does not pin the cast. The
    /// `debug_assert!` records `other`, while the downcast result is only
    /// formatted into the message and discarded.
    #[test]
    fn observer_for_another_value_does_not_pin_the_cast() -> Result<(), String> {
        let body = r#"debug_assert!(other.is_ok(), "{:?}", e.downcast::<T>());"#;
        let masked = mask_comments_and_strings(body);
        let (start, invocation) = first_downcast(&masked)?;
        assert!(
            !downcast_statement_is_observed(&masked, start, &invocation),
            "an observer of another value must not pin this cast"
        );
        Ok(())
    }

    /// F8 positive control: the same shape observing the CAST's own result
    /// does pin.
    #[test]
    fn observer_of_the_cast_result_pins() -> Result<(), String> {
        let body = r#"assert!(e.downcast::<T>().is_ok(), "wrong type");"#;
        let masked = mask_comments_and_strings(body);
        let (start, invocation) = first_downcast(&masked)?;
        assert!(downcast_statement_is_observed(&masked, start, &invocation));
        Ok(())
    }

    /// F8: `matches!`/`assert*!` wrappers observe only as whole words —
    /// `debug_assert!(` is not `assert!(`, and `assert_matches!(` is not
    /// `matches!(`.
    #[test]
    fn wrapper_prefix_names_do_not_count_as_wrappers() -> Result<(), String> {
        let debug_assert = r#"debug_assert!(ready, "{:?}", e.downcast::<T>());"#;
        let masked = mask_comments_and_strings(debug_assert);
        let (start, invocation) = first_downcast(&masked)?;
        assert!(
            !downcast_statement_is_observed(&masked, start, &invocation),
            "debug_assert! must not read as an assert! wrapper"
        );
        let assert_matches = r#"if !assert_matches!(e.downcast::<T>(), Ok(_)) { }"#;
        let masked = mask_comments_and_strings(assert_matches);
        let (start, invocation) = first_downcast(&masked)?;
        assert!(
            !downcast_statement_is_observed(&masked, start, &invocation),
            "assert_matches! is not a whole-word matches! wrapper (its pattern \
             pinning goes through the matches-guard authority instead)"
        );
        Ok(())
    }

    /// F8: `.map(`/`.map_err(` convert the cast result without inspecting
    /// it, so they do not observe; a discard binding observes nothing even
    /// when a boolean inspection follows the invocation.
    #[test]
    fn conversions_and_discard_bindings_do_not_observe() -> Result<(), String> {
        let map_err = r#"let _ = e.downcast::<T>().map_err(|x| x.to_string())?;"#;
        let masked = mask_comments_and_strings(map_err);
        let (start, invocation) = first_downcast(&masked)?;
        assert!(
            !downcast_statement_is_observed(&masked, start, &invocation),
            "map_err converts, it does not observe"
        );
        let map = r#"let width = e.downcast::<T>().map(|x| x.len()).unwrap_or(0);"#;
        let masked = mask_comments_and_strings(map);
        let (start, invocation) = first_downcast(&masked)?;
        assert!(
            !downcast_statement_is_observed(&masked, start, &invocation),
            "map converts the cast; the later unwrap_or observes a WIDTH, not \
             the cast result"
        );
        let discarded_boolean = r#"let _ = e.downcast::<T>().is_ok();"#;
        let masked = mask_comments_and_strings(discarded_boolean);
        let (start, invocation) = first_downcast(&masked)?;
        assert!(
            !downcast_statement_is_observed(&masked, start, &invocation),
            "a discard binding observes nothing, even with .is_ok() appended"
        );
        Ok(())
    }

    /// F14: a downcast nested inside a closure argument keeps its own
    /// statement window — the observer after the call close still pins,
    /// and an unobserved closure result does not.
    #[test]
    fn closure_argument_windows_do_not_mis_slice() -> Result<(), String> {
        let observed = r#"with_default(|| e.downcast::<T>().is_ok(), fallback);"#;
        let masked = mask_comments_and_strings(observed);
        let (start, invocation) = first_downcast(&masked)?;
        assert!(downcast_statement_is_observed(&masked, start, &invocation));
        let unobserved = r#"with_default(|| e.downcast::<T>(), fallback);"#;
        let masked = mask_comments_and_strings(unobserved);
        let (start, invocation) = first_downcast(&masked)?;
        assert!(
            !downcast_statement_is_observed(&masked, start, &invocation),
            "the closure result is passed on, not inspected"
        );
        Ok(())
    }

    /// F14: brackets and nested calls before the invocation must not
    /// mis-slice the window — and a PRECEDING statement's observer must
    /// not leak into this statement's.
    #[test]
    fn nested_brackets_before_the_invocation_keep_the_window_honest() -> Result<(), String> {
        let nested = r#"results[config.index].push(format!("{}", e.downcast::<T>().is_ok()));"#;
        let masked = mask_comments_and_strings(nested);
        let (start, invocation) = first_downcast(&masked)?;
        assert!(downcast_statement_is_observed(&masked, start, &invocation));
        // The statement window starts after the previous statement's `;`,
        // so the earlier `assert!(` cannot wrap this invocation.
        let leak = r#"assert!(ready); sink(e.downcast::<T>());"#;
        let masked = mask_comments_and_strings(leak);
        let (start, invocation) = first_downcast(&masked)?;
        let window_start = statement_window_start(&masked, start);
        assert!(
            !masked[window_start..start].contains("assert!("),
            "the previous statement's observer must be outside the window"
        );
        assert!(
            !downcast_statement_is_observed(&masked, start, &invocation),
            "a preceding statement's assert! must not wrap this invocation"
        );
        Ok(())
    }

    /// F14: a string literal containing `; .is_ok()` hides no real
    /// statement boundary — masked string content cannot mis-slice the
    /// window, and the cast's own observer still pins.
    #[test]
    fn string_literal_observer_text_before_the_cast_does_not_mis_slice() -> Result<(), String> {
        let body = r#"log("x; .is_ok()"); e.downcast::<T>().is_ok();"#;
        let masked = mask_comments_and_strings(body);
        let (start, invocation) = first_downcast(&masked)?;
        let window_start = statement_window_start(&masked, start);
        assert!(
            !masked[window_start..start].contains(".is_ok()"),
            "the masked string's observer text must not leak into the window: {}",
            &masked[window_start..start]
        );
        assert!(downcast_statement_is_observed(&masked, start, &invocation));
        Ok(())
    }

    // --- #3731 review (G2): a body pin must participate in the divergence ---

    /// G2 falsifier: a `matches!` pin computed into an unconsumed `let`
    /// binding does not gate the unconditional panic — the arm diverges
    /// regardless of the changed variant, so the pin must not credit.
    /// Pre-fix this shape credited Strong.
    #[test]
    fn computed_but_unconsumed_matches_pin_does_not_pin() {
        let body = r#"
#[test]
fn computed_then_any_panic() {
    match parse(input) {
        Ok(value) => assert_eq!(value, 1),
        Err(error) => {
            let matched = matches!(error, ParseError::InvalidData);
            panic!("any error surfaced: {error}");
        }
    }
}
"#;
        let scan = guarded_result_match_scan(body, 1);
        assert!(
            scan.oracles.is_empty(),
            "a computed-but-unconsumed pin must not credit: {:?}",
            scan.oracles
        );
    }

    /// G2 falsifier (downcast family): an observed cast computed into an
    /// unconsumed binding is dead computation, not a pin. Pre-fix this
    /// shape credited Medium.
    #[test]
    fn computed_but_unconsumed_downcast_does_not_pin() {
        let body = r#"
#[test]
fn observed_then_discarded_binding() {
    match parse(input) {
        Ok(value) => assert_eq!(value, 1),
        Err(error) => {
            let is_parse = error.downcast_ref::<ParseError>().is_ok();
            panic!("any error surfaced: {error}");
        }
    }
}
"#;
        let scan = guarded_result_match_scan(body, 1);
        assert!(
            scan.oracles.is_empty(),
            "an unconsumed observed cast must not pin: {:?}",
            scan.oracles
        );
    }

    /// G2 clause (b) control: when the arm's FIRST control transfer — the
    /// statement deciding the arm's outcome — references the pin's binding
    /// variable in its own (non-string) code, the computed pin
    /// participates. The reference must sit outside a string literal:
    /// masked string content is erased before the scan.
    #[test]
    fn computed_pin_consumed_by_the_decisive_statement_pins() {
        let body = r#"
#[test]
fn computed_and_consumed() {
    match parse(input) {
        Ok(value) => assert_eq!(value, 1),
        Err(error) => {
            let matched = matches!(error, ParseError::InvalidData);
            panic!("{}", matched);
        }
    }
}
"#;
        let scan = guarded_result_match_scan(body, 1);
        assert_eq!(
            scan.oracles.len(),
            1,
            "a pin the decisive statement consumes participates: {:?}",
            scan.oracles
        );
        assert_eq!(scan.oracles[0].strength, OracleStrength::Strong);
    }

    /// G2 positive control (clause a): the computed pin feeding a
    /// body-predicate `if` condition through the same pattern stays
    /// credited — the diverging if's condition carries the pin.
    #[test]
    fn computed_pin_also_in_a_diverging_if_condition_pins() {
        let body = r#"
#[test]
fn computed_and_conditioned() {
    match parse(input) {
        Ok(value) => assert_eq!(value, 1),
        Err(error) => {
            let matched = matches!(error, ParseError::InvalidData);
            if !matches!(error, ParseError::InvalidData) {
                panic!("unexpected variant: {error}");
            }
        }
    }
}
"#;
        let scan = guarded_result_match_scan(body, 1);
        assert_eq!(scan.oracles.len(), 1, "{:?}", scan.oracles);
        assert_eq!(scan.oracles[0].strength, OracleStrength::Strong);
    }

    // --- #3731 review (G3): successful returns inside the if-form ---

    /// G3 falsifier (negated form): a successful `return Ok(())` beside the
    /// panic inside the body-predicate `if` is an escape path — the first
    /// depth-0 statement does not diverge, so the form is not terminal.
    /// Pre-fix the block's `.any()` rule credited the later panic.
    #[test]
    fn successful_return_beside_the_panic_disqualifies_the_if_form() {
        let body = r#"
#[test]
fn escapes_through_the_branch() {
    match parse(input) {
        Ok(value) => assert_eq!(value, 1),
        Err(error) => {
            if !matches!(error, ParseError::InvalidData) {
                return Ok(());
                panic!("unreachable after the successful return");
            }
        }
    }
}
"#;
        let scan = guarded_result_match_scan(body, 1);
        assert!(
            scan.oracles.is_empty(),
            "a successful return inside the if-form swallows the error: {:?}",
            scan.oracles
        );
    }

    /// G3 falsifier (the finding's shape): a pin-conditioned `if` whose
    /// branch returns successfully swallows the matched variant; the
    /// condition's pin must not credit even though a trailing unconditional
    /// panic terminates the arm.
    #[test]
    fn successful_return_in_branch_blocks_the_condition_pin() {
        let body = r#"
#[test]
fn branch_returns_ok_then_any_panic() {
    match parse(input) {
        Ok(value) => assert_eq!(value, 1),
        Err(error) => {
            if matches!(error, ParseError::InvalidData) {
                return Ok(());
            }
            panic!("any error surfaced: {error}");
        }
    }
}
"#;
        let scan = guarded_result_match_scan(body, 1);
        assert!(
            scan.oracles.is_empty(),
            "a branch that returns successfully must not credit its pin: {:?}",
            scan.oracles
        );
    }

    /// G3 positive control: a negated body-predicate form whose block holds
    /// only diverging statements stays terminal.
    #[test]
    fn all_diverging_branch_keeps_the_if_form_terminal() {
        let body = r#"
#[test]
fn loud_branch() {
    match parse(input) {
        Ok(value) => assert_eq!(value, 1),
        Err(error) => {
            if !matches!(error, ParseError::InvalidData) {
                panic!("wrong variant");
                unreachable!("never reached");
            }
        }
    }
}
"#;
        let scan = guarded_result_match_scan(body, 1);
        assert_eq!(
            scan.oracles.len(),
            1,
            "an all-diverging branch terminates: {:?}",
            scan.oracles
        );
        assert_eq!(scan.oracles[0].strength, OracleStrength::Strong);
    }

    // --- #3731 review (G4): per-pin truncation ---

    /// G4 falsifier: two long variant pins whose JOIN exceeds the pre-fix
    /// 120-character overall cap both survive into the synthesized text
    /// whole — each pin is capped individually and the join is not
    /// truncated — so a changed seam matching either variant still finds
    /// its pin in the text reveal and repo grading parse. (Confirmation
    /// through any collected pin is pinned by
    /// `every_err_arm_pin_is_collected` on the reveal side.)
    #[test]
    fn two_long_pins_both_survive_into_the_oracle_text() {
        let body = r#"
#[test]
fn two_long_variant_arms() {
    match parse(input) {
        Ok(value) => assert_eq!(value, 1),
        Err(ParseError::InvalidPayloadChecksumDetectedInResponseBodyPayload) => {
            panic!("checksum")
        }
        Err(ParseError::UnexpectedEofDetectedInResponseBodyPayloadSection) => {
            panic!("eof")
        }
    }
}
"#;
        let scan = guarded_result_match_scan(body, 1);
        assert_eq!(scan.oracles.len(), 1, "{:?}", scan.oracles);
        let text = &scan.oracles[0].text;
        assert!(
            text.contains("Err(ParseError::InvalidPayloadChecksumDetectedInResponseBodyPayload)"),
            "the first long pin survives whole: {text}"
        );
        assert!(
            text.contains("Err(ParseError::UnexpectedEofDetectedInResponseBodyPayloadSection)"),
            "the later long pin must not disappear behind an overall cap: {text}"
        );
        assert!(!text.contains("..."), "no overall truncation: {text}");
    }

    /// G4 control: a single pin over the documented per-pin cap is
    /// truncated at that boundary, so one pathological pin stays bounded.
    #[test]
    fn a_single_pin_over_the_per_pin_cap_truncates_alone() {
        let body = r#"
#[test]
fn one_pathological_variant_arm() {
    match parse(input) {
        Ok(value) => assert_eq!(value, 1),
        Err(ParseError::InvalidPayloadChecksumDetectedInResponseBodyPayloadSectionMarker) => {
            panic!("checksum")
        }
    }
}
"#;
        let scan = guarded_result_match_scan(body, 1);
        assert_eq!(scan.oracles.len(), 1, "{:?}", scan.oracles);
        let text = &scan.oracles[0].text;
        // The pin (81 chars) caps at the 80-character per-pin boundary: the
        // truncated slice plus the marker appears, the closed whole pin
        // does not.
        assert!(
            text.contains("Err(ParseError::InvalidPayloadChecksumDetectedInResponseBodyPayloadSectionMarker..."),
            "the pin is truncated at the per-pin cap: {text}"
        );
        assert!(
            !text.contains("SectionMarker)"),
            "the over-cap pin carries the truncation marker, not its closed \
             form: {text}"
        );
    }
}

#[cfg(test)]
mod err_guard_parity_tests {
    use super::extract_assertions;
    use crate::domain::{OracleKind, OracleStrength};

    #[test]
    fn err_return_guard_is_an_oracle_equal_to_its_assert_twin() {
        // #3284: `if actual != expected { return Err(...) }` in a test body
        // is the manual expansion of a message-carrying assertion; leaving
        // it unrecognized made equivalent harness forms change the
        // credited oracle and, through it, the production gap accounting.
        let guard = extract_assertions(
            "if actual != expected {\n    return Err(anyhow!(\"actual={actual:?}\"));\n}\nOk(())\n",
            3,
        );
        assert!(
            guard
                .iter()
                .any(|fact| fact.kind == OracleKind::RelationalCheck),
            "the Err-return guard must be recognized: {guard:?}"
        );
        let twin = extract_assertions("assert!(actual == expected, \"actual={actual:?}\");\n", 3);
        let guard_best = guard
            .iter()
            .find(|fact| fact.kind == OracleKind::RelationalCheck)
            .map(|fact| (fact.kind.clone(), fact.strength.clone()));
        let twin_best = twin
            .iter()
            .find(|fact| fact.kind == OracleKind::RelationalCheck)
            .map(|fact| (fact.kind.clone(), fact.strength.clone()));
        assert_eq!(
            guard_best, twin_best,
            "the guard and its assert! twin must carry identical oracle meaning"
        );
        assert_eq!(
            guard_best.map(|(_, strength)| strength),
            Some(OracleStrength::Weak)
        );
    }

    #[test]
    fn comment_only_or_compound_guards_never_credit() {
        // #3284 fail-closed gates: an Err return that is not the body's
        // first statement (commented out or in a string), and compound
        // conditions whose negation is not a single assert twin, produce
        // no oracle.
        let commented = extract_assertions(
            "if actual != expected { /* previously: return Err(\"m\") */ eprintln!(\"m\"); }
",
            3,
        );
        assert!(
            commented.is_empty(),
            "commented-out Err must not credit: {commented:?}"
        );
        let compound = extract_assertions(
            "if a != b || c != d { return Err(\"x\"); }
",
            3,
        );
        assert!(
            compound.is_empty(),
            "compound guard must stay unrecognized: {compound:?}"
        );
    }

    #[test]
    fn guard_conditions_without_structural_equivalence_stay_unrecognized() {
        // No inference from messages or opaque conditions: a guard whose
        // condition cannot be structurally negated into an assertion
        // contributes no oracle fact.
        let opaque = extract_assertions(
            "if !matches!(result, Expected::Good(_)) {\n    return Err(anyhow!(\"bad\"));\n}\n",
            3,
        );
        assert!(
            !opaque
                .iter()
                .any(|fact| fact.kind == OracleKind::RelationalCheck
                    || fact.kind == OracleKind::ExactValue),
            "opaque guard conditions must not be guessed into oracles: {opaque:?}"
        );
    }
}

#[cfg(test)]
mod multibyte_guard_tests {
    use super::extract_assertions;
    use crate::domain::OracleKind;

    #[test]
    fn multibyte_comparison_operands_do_not_panic() {
        // #3284: the Linux CI panic — a guard comparing a multibyte
        // operand must classify (or reject) without byte-slicing inside
        // a character boundary.
        let guard = extract_assertions(
            "if actual != \"\u{2713}\" {\n    return Err(\"mismatch\");\n}\n",
            3,
        );
        assert!(
            guard
                .iter()
                .any(|fact| fact.kind == OracleKind::RelationalCheck),
            "multibyte guard must credit: {guard:?}"
        );
    }
}
