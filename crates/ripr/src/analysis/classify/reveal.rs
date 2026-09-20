use super::super::rust_index::{
    OracleFact, OracleTextShape, TestSummary, extract_identifier_tokens, has_oracle_text_shape,
};

use super::rust_string_literals;
use crate::domain::*;

#[cfg(test)]
fn reveal_evidence(
    probe: &Probe,
    related_tests: &[(&TestSummary, RelationReason)],
) -> (StageEvidence, StageEvidence, Vec<RelatedTest>) {
    reveal_evidence_with_expression(
        probe,
        &probe.expression,
        related_tests,
        &|_, _| false,
        &|_, _| false,
    )
}

pub(in crate::analysis) fn reveal_evidence_with_expression(
    probe: &Probe,
    analysis_expression: &str,
    related_tests: &[(&TestSummary, RelationReason)],
    same_name_import_defeats: &dyn Fn(&TestSummary, &str) -> bool,
    cross_package_name_defeats: &dyn Fn(&TestSummary, &str) -> bool,
) -> (StageEvidence, StageEvidence, Vec<RelatedTest>) {
    if related_tests.is_empty() {
        return (
            StageEvidence::new(
                StageState::No,
                Confidence::Medium,
                "No reachable test oracle found",
            ),
            StageEvidence::new(
                StageState::No,
                Confidence::Medium,
                "No assertion can discriminate the changed behavior without a reachable test",
            ),
            Vec::new(),
        );
    }

    let analysis = analyze_related_assertions(
        probe,
        analysis_expression,
        related_tests,
        same_name_import_defeats,
        cross_package_name_defeats,
    );
    let related = finalize_related_tests(analysis.related);
    let observe = build_observe_evidence(analysis.matched_any);
    let mut discriminate = build_discriminate_evidence(
        &analysis.strongest,
        &analysis.strongest_kind,
        &probe.family,
        analysis.observation_unverified,
    );
    // #1746: a Yes discriminator whose discriminating oracle is not bound to
    // the changed sink is the coverage mistake, not discrimination. File
    // proximity and token coincidence observe a nearby value at best. Cap at
    // Weak so classify() emits weakly_exposed (fail closed, under-credit)
    // unless the strongest matched oracle itself carries entity-level
    // evidence (owner/helper call, assertion-target affinity, owner-named
    // test).
    if discriminate.state == StageState::Yes && !analysis.strongest_identity {
        discriminate = StageEvidence::new(
            StageState::Weak,
            Confidence::Medium,
            "Discriminator uncredited: strongest oracle reaches only through weak-token linkage without same-entity evidence (identity_unresolved)",
        );
    }

    (observe, discriminate, related)
}

struct RevealAssertionAnalysis {
    related: Vec<RelatedTest>,
    strongest: OracleStrength,
    strongest_kind: OracleKind,
    matched_any: bool,
    /// True when the DISCRIMINATING oracle — the strongest matched assertion
    /// that drives `discriminate` — is bound to the changed sink. File-level
    /// proximity (`SameTestFile`, `SameModule`) and token coincidence
    /// (`WeakTokenSubstring`) observe a nearby value at best: a same-file
    /// test asserting a different function in the file cannot discriminate
    /// this sink (#1746). Identity-conferring relations are the call-level
    /// and target-level ones (`DirectOwnerCall`, `HelperOwnerCall`,
    /// `AssertionTargetAffinity`) plus `OwnerNamedTest`, which preserves the
    /// spec'd variant-bound credit (RIPR-SPEC-0106 Part B): reach established
    /// by one test and oracle strength by another do not combine into
    /// discrimination unless the discriminating oracle itself is
    /// entity-level evidence. Ties at the top rank also confer identity.
    strongest_identity: bool,
    /// True when this probe's family requires a `token_match` to confirm that
    /// an assertion actually references the specific changed sub-expression, and
    /// no such match has fired yet.
    ///
    /// Applies to: `MatchArm`, `ReturnValue`, `FieldConstruction`, `SideEffect`,
    /// `CallDeletion`, `ErrorPath`. For each of these families, the broad
    /// `family_match` or `assertion_count == 1` matcher alone is insufficient:
    /// it tells us an oracle of the right shape exists in a reachable test, but
    /// cannot confirm it observes *this particular* changed expression (vs. a
    /// sibling, an unrelated field, or a different call site).
    ///
    /// Confirmation differs by family:
    /// - **Value** families (MatchArm, ReturnValue, FieldConstruction,
    ///   ErrorPath): the only static signal of specificity is a `token_match` —
    ///   an assertion whose text contains an identifier token from the probe
    ///   expression. For probes whose changed expression constructs an exact
    ///   error variant, `assertion_matches_probe_detail`'s
    ///   `ExactErrorVariant` fast-path returns `has_token_match=true` only when
    ///   the assertion text contains that specific variant token (RIPR-SPEC-0106,
    ///   Part B), so a genuine variant-pinning oracle clears this guard. A sibling
    ///   variant, a broad `is_err()`, or a non-variant exact-value oracle does not.
    /// - **Effect** families (SideEffect, CallDeletion): the canonical observer
    ///   is a mock/expectation/snapshot that kind-matches the seam without
    ///   sharing a token, so a genuine effect observer (`effect_observer_confirms`)
    ///   confirms in addition to `token_match`.
    ///
    /// Cleared as soon as a confirming assertion fires.
    observation_unverified: bool,
}

/// Returns true for families where an assertion must specifically reference the
/// changed sub-expression to confirm observation. For **value** families
/// (MatchArm, ReturnValue, FieldConstruction, ErrorPath) the only static
/// confirmation signal is a `token_match`. For probes whose changed expression
/// constructs an exact error variant, a genuine variant-pinning oracle
/// (`ExactErrorVariant` whose text contains that specific variant token) sets
/// `has_token_match=true` in `assertion_matches_probe_detail` (RIPR-SPEC-0106,
/// Part B), clearing this guard. A broad `is_err()` or an exact-value oracle on
/// a sibling result does not. For **effect** families (SideEffect,
/// CallDeletion) the legitimate observer is often a mock/expectation that
/// **kind-matches the seam** without sharing any probe token; for those, a
/// seam-kind match also confirms observation (see `effect_observer_confirms`).
fn needs_token_confirmation(family: &ProbeFamily) -> bool {
    matches!(
        family,
        ProbeFamily::MatchArm
            | ProbeFamily::ReturnValue
            | ProbeFamily::FieldConstruction
            | ProbeFamily::SideEffect
            | ProbeFamily::CallDeletion
            | ProbeFamily::ErrorPath
    )
}

/// Returns true for the **effect** families (SideEffect, CallDeletion) whose
/// changed behavior is a side effect or outbound call. For these, the canonical
/// observer is a mock/expectation that kind-matches the seam rather than a
/// value assertion that names a token from the changed expression. A genuine
/// effect observer therefore confirms observation even without a `token_match`.
fn is_effect_family(family: &ProbeFamily) -> bool {
    matches!(family, ProbeFamily::SideEffect | ProbeFamily::CallDeletion)
}

fn effect_target_tokens(expression: &str) -> Vec<String> {
    let target_end = expression.find(['(', '=']).unwrap_or(expression.len());
    extract_identifier_tokens(&expression[..target_end])
}

/// Returns true when `assertion` is a genuine **effect observer** that
/// kind-matches an effect seam: a mock/expectation, a snapshot, or a
/// whole-object equality capturing the resulting state. This is intentionally
/// narrower than `oracle_matches_family` for effect families — it excludes the
/// broad assertion-or-expectation text shape, so a plain non-observing assertion
/// (e.g. `assert!(result)`) does
/// **not** clear `observation_unverified`. Only a real expectation/snapshot
/// observer does.
fn effect_observer_confirms(assertion: &OracleFact) -> bool {
    matches!(
        assertion.kind,
        OracleKind::MockExpectation | OracleKind::Snapshot | OracleKind::WholeObjectEquality
    )
}

/// For a `MatchArm` probe expression, extract only the "variant" tokens —
/// the identifier segments that appear immediately after a `::` separator.
/// These are the arm-specific tokens that can confirm an assertion targets
/// this arm rather than a sibling sharing the same enum qualifier.
///
/// Example: `"Mode::Frozen => -1,"` → `["Frozen"]`.
/// Example: `"Status::Active | Status::Idle => 0,"` → `["Active", "Idle"]`.
/// Example: `"None => 0,"` → `[]` (no `::` in expression).
fn match_arm_variant_tokens(expression: &str) -> Vec<String> {
    let mut variants = Vec::new();
    let bytes = expression.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i + 1 < len {
        if bytes[i] == b':' && bytes[i + 1] == b':' {
            // skip "::"
            i += 2;
            // collect the identifier that follows
            let start = i;
            while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            if i > start {
                let variant = &expression[start..i];
                if extract_identifier_tokens(variant).contains(&variant.to_string()) {
                    variants.push(variant.to_string());
                }
            }
        } else {
            i += 1;
        }
    }
    variants
}

fn analyze_related_assertions(
    probe: &Probe,
    analysis_expression: &str,
    related_tests: &[(&TestSummary, RelationReason)],
    same_name_import_defeats: &dyn Fn(&TestSummary, &str) -> bool,
    cross_package_name_defeats: &dyn Fn(&TestSummary, &str) -> bool,
) -> RevealAssertionAnalysis {
    let probe_tokens = if is_effect_family(&probe.family) {
        effect_target_tokens(analysis_expression)
    } else {
        extract_identifier_tokens(analysis_expression)
    };
    let effect_literals = if is_effect_family(&probe.family) {
        rust_string_literals(analysis_expression)
    } else {
        Vec::new()
    };
    // For MatchArm: collect variant-only tokens (post-`::`) for the specificity
    // check. Qualifier tokens (e.g. the type name before `::`) are excluded so
    // that a sibling-arm assertion sharing the qualifier cannot spuriously
    // confirm observation of this arm.
    let match_arm_variants = if matches!(probe.family, ProbeFamily::MatchArm) {
        match_arm_variant_tokens(analysis_expression)
    } else {
        Vec::new()
    };
    // For MatchArm: string literals in the arm PATTERN (left of `=>`) are the
    // parser-owned identity of a literal arm (`"sensor"` in
    // `"sensor" => "sensor-v2",`). Result-side literals never confirm: a
    // sibling arm returning the same literal does not select this arm.
    // A sibling arm's pattern literals never overlap this arm's pattern, so
    // literal confirmation keeps the sibling rejection that variant-only
    // matching provides. No `=>` means no statically established pattern.
    let match_arm_literals = if matches!(probe.family, ProbeFamily::MatchArm) {
        match_arm_pattern_literals(analysis_expression)
    } else {
        Vec::new()
    };
    // A literal/variant match does not establish that an arm guard evaluated
    // true. Preserve the arm as an explicit unverified observation until a
    // producer-owned guard witness exists.
    let match_arm_guarded = matches!(probe.family, ProbeFamily::MatchArm)
        && match_arm_pattern_has_guard(analysis_expression);
    // For probes whose changed expression constructs an exact error variant
    // (`Err(Type::Variant)`): collect the variant-only token (the identifier
    // after the last `::`) so that a sibling-variant assertion that pins a
    // different variant of the same error type cannot confirm this probe.
    // RIPR-SPEC-0106 (Part B). The guard keys on the changed expression, not
    // the family label: a `return_value` probe on an `Err(...)` construction
    // would otherwise credit a sibling-variant oracle through the shared enum
    // qualifier token — and a `field_construction` probe whose field is an
    // `Err(...)` construction (`outcome: Err(CalcError::TooLarge)`) has the
    // same shared-qualifier exposure.
    let error_construction_variant = if matches!(
        probe.family,
        ProbeFamily::ErrorPath | ProbeFamily::ReturnValue | ProbeFamily::FieldConstruction
    ) {
        error_path_variant_token(&probe.expression)
            .or_else(|| error_path_variant_token(analysis_expression))
    } else {
        None
    };
    // #3700 (final consolidation): a wrapper error seam
    // (`callee(..).map_err(..)`) whose changed expression carries no
    // parseable variant has no statically establishable variant identity —
    // whether the wrapper faithfully carries the callee's error variant
    // through the boxed conversion is not statically resolvable. Such a seam
    // therefore NEVER confirms observation from lexical matching (every
    // confirming signal is token coincidence by construction); it stays
    // below `exposed` and carries the typed
    // `wrapper_error_binding_unresolved` limitation attached by
    // `apply_wrapper_error_binding_limit` (analysis/language/rust.rs).
    let wrapper_seam = error_construction_variant.is_none()
        && matches!(
            probe.family,
            ProbeFamily::ErrorPath | ProbeFamily::ReturnValue
        )
        && wrapper_error_seam_expression(&[probe.expression.as_str(), analysis_expression]);
    let match_context = RevealMatchContext {
        probe_tokens: &probe_tokens,
        effect_literals: &effect_literals,
        match_arm_variants: &match_arm_variants,
        match_arm_literals: &match_arm_literals,
        match_arm_guarded,
        error_construction_variant: error_construction_variant.as_deref(),
        family: &probe.family,
        wrapper_seam,
        // #3709: the owner's bare name is the segment after the symbol's
        // final `::` separator (`src/lib.rs::impl Discount::score` -> `score`,
        // `src/lib.rs::expect_response` -> `expect_response`).
        owner_callee: probe.owner.as_ref().and_then(|symbol| {
            let name = symbol.0.rsplit("::").next()?;
            (!name.is_empty()).then_some(name)
        }),
    };
    let confirm_required = needs_token_confirmation(&probe.family);
    let mut related = Vec::new();
    let mut strongest = OracleStrength::None;
    let mut strongest_kind = OracleKind::Unknown;
    let mut matched_any = false;
    let mut strongest_identity = false;
    // For families that need token confirmation: start pessimistic and clear
    // once a token_match fires.
    let mut observation_unverified = false;

    for (test, reason) in related_tests {
        let relation_reason = Some(*reason);
        let relation_confidence = Some(reason.confidence());
        if test.assertions.is_empty() {
            related.push(RelatedTest {
                name: test.name.clone(),
                file: test.file.clone(),
                line: test.start_line,
                oracle: None,
                oracle_kind: OracleKind::Unknown,
                oracle_strength: OracleStrength::None,
                relation_reason,
                relation_confidence,
            });
            continue;
        }
        // #3731 review (F11): computed once per test — whether the test's
        // own file imports the owner callee's bare name from a FOREIGN
        // path, which makes every bare-scrutinee binding in it ambiguous.
        let import_defeats_owner = match_context
            .owner_callee
            .is_some_and(|callee| same_name_import_defeats(test, callee));
        // #3731 review (G1): computed once per test — whether the test's
        // own package defines a function with the owner callee's bare name
        // while the changed owner lives in ANOTHER package, which makes the
        // bare call ambiguous across packages the same way a foreign
        // import does.
        let cross_package_defeats_owner = match_context
            .owner_callee
            .is_some_and(|callee| cross_package_name_defeats(test, callee));
        for assertion in &test.assertions {
            let (matched, has_token_match) = assertion_matches_probe_detail_with_literals(
                &match_context,
                assertion,
                test.assertions.len(),
                import_defeats_owner,
                cross_package_defeats_owner,
            );
            if matched {
                if confirm_required {
                    // Observation is confirmed when the assertion specifically
                    // references the changed sub-expression. For value families
                    // (MatchArm/ReturnValue/FieldConstruction) the only static
                    // signal is a `token_match`. For effect families
                    // (SideEffect/CallDeletion) the canonical observer is a
                    // mock/expectation/snapshot that kind-matches the seam
                    // without sharing a token, so a genuine effect observer also
                    // confirms. This prevents a real mock from being wrongly
                    // flagged `observation_unverified`, while a plain
                    // non-observing assertion (no token, no effect observer)
                    // stays unverified.
                    let observation_confirmed = has_token_match
                        || (is_effect_family(&probe.family) && effect_observer_confirms(assertion));
                    if !matched_any {
                        // First matching assertion: observation is unverified
                        // unless confirmed.
                        observation_unverified = !observation_confirmed;
                    } else if observation_confirmed {
                        // A later confirmed assertion clears the unverified flag.
                        observation_unverified = false;
                    }
                }
                matched_any = true;
                let relative_strength = probe_relative_oracle_strength(&probe.family, assertion);
                let identity = matches!(
                    reason,
                    RelationReason::DirectOwnerCall
                        | RelationReason::HelperOwnerCall
                        | RelationReason::AssertionTargetAffinity
                        | RelationReason::OwnerNamedTest
                );
                if relative_strength.rank() > strongest.rank() {
                    strongest = relative_strength.clone();
                    strongest_kind = assertion.kind.clone();
                    strongest_identity = identity;
                } else if relative_strength.rank() == strongest.rank() {
                    strongest_identity = strongest_identity || identity;
                }
                related.push(RelatedTest {
                    name: test.name.clone(),
                    file: test.file.clone(),
                    line: test.start_line,
                    oracle: Some(assertion.text.clone()),
                    oracle_kind: assertion.kind.clone(),
                    oracle_strength: relative_strength,
                    relation_reason,
                    relation_confidence,
                });
            }
        }
    }

    RevealAssertionAnalysis {
        related,
        strongest,
        strongest_kind,
        matched_any,
        strongest_identity,
        observation_unverified,
    }
}

/// Extracts the variant identifier from a changed expression that constructs
/// an exact error variant.
///
/// For `return Err(CalcError::TooLarge);` → `Some("TooLarge")`.
/// For `return Err(anyhow!("..."));` → `None` (no qualified variant).
///
/// Used by RIPR-SPEC-0106 (Part B) to restrict `ExactErrorVariant` assertion
/// matching to the changed expression's specific variant, preventing
/// sibling-variant over-credit.
fn error_path_variant_token(expression: &str) -> Option<String> {
    use super::text::exact_error_variant;
    let variant_path = exact_error_variant(expression)?;
    // Last component after the final `::`.
    let last = variant_path.rsplit("::").next()?;
    if last
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase())
    {
        Some(last.to_string())
    } else {
        None
    }
}

/// Probe-side matching inputs shared by every assertion of one probe
/// (grouped so the per-assertion matcher stays under the argument limit).
struct RevealMatchContext<'a> {
    probe_tokens: &'a [String],
    effect_literals: &'a [String],
    match_arm_variants: &'a [String],
    match_arm_literals: &'a [String],
    /// True when the parser-owned arm pattern carries a guard. This slice
    /// fails closed until the observed input can be shown to satisfy it.
    match_arm_guarded: bool,
    error_construction_variant: Option<&'a str>,
    family: &'a ProbeFamily,
    /// `true` only for #3700 wrapper error seams: the changed expression is a
    /// `map_err` conversion with no parseable variant, so the variant
    /// binding is not statically establishable and nothing may confirm
    /// observation through lexical matching.
    wrapper_seam: bool,
    /// #3709: the changed owner's bare function name, when the probe has
    /// one. A guarded Result match whose scrutinee directly calls this
    /// callee observes the owner's returned `Result` — the exact sink for
    /// the value/error families — without any changed-line token overlap.
    owner_callee: Option<&'a str>,
}

/// The bare-scrutinee convention of the synthesized guarded-Result-match
/// oracle text (extract::oracles::scan): the scrutinee path is embedded
/// directly after `match `, so a BARE one-segment scrutinee appears as
/// `match <callee>(`. A qualified scrutinee (`match helpers::parse(..)`)
/// never contains that substring — reveal cannot resolve a qualified path
/// to the probe owner's identity, so its confirmation stays unverified
/// (#3731 review; identity resolution tracked on #3727).
fn guarded_oracle_names_bare_callee(text: &str, callee: &str) -> bool {
    text.contains(&format!("match {callee}("))
}

/// String literals in a match-arm pattern (left of the arm separator `=>`).
/// Empty when the expression has no arm separator: without a statically
/// established pattern there is no literal arm identity to confirm.
///
/// The separator is the first `=>` outside string/character literals and
/// comments (a `=>` inside literal content such as `"sensor=>legacy"` is
/// not an arm separator). Values are decoded literal values, so a raw
/// spelling and a cooked spelling with different decoded values never
/// equate (`r"sensor\n"` is not `"sensor\n"`).
fn match_arm_pattern_literals(expression: &str) -> Vec<String> {
    match find_fat_arrow(expression) {
        Some(separator) => match_arm_string_values(&expression[..separator]),
        None => Vec::new(),
    }
}

/// Whether the parser-owned match-arm pattern includes a guard before `=>`.
///
/// The `if` keyword must sit outside string/character literals and comments,
/// so `"if" =>` and comments do not create a guard. This slice deliberately
/// fails closed for guarded arms: matching the pattern literal alone does
/// not establish guard satisfaction.
fn match_arm_pattern_has_guard(expression: &str) -> bool {
    let Some(separator) = find_fat_arrow(expression) else {
        return false;
    };
    contains_guard_if(&expression[..separator])
}

/// Byte index of the `=` in the first `=>` outside string/character literals
/// and comments. A fat arrow inside literal content or a comment never
/// separates a match arm from its body.
fn find_fat_arrow(text: &str) -> Option<usize> {
    let opaque = lex_opaque_ranges(text);
    let bytes = text.as_bytes();
    let mut index = 0usize;
    while index + 1 < bytes.len() {
        if opaque
            .iter()
            .any(|(start, end)| index >= *start && index < *end)
        {
            index += 1;
            continue;
        }
        if bytes[index] == b'=' && bytes[index + 1] == b'>' {
            return Some(index);
        }
        index += 1;
    }
    None
}

/// True when a whole-word `if` occurs outside string/character literals and
/// comments. Raw-string contents (including `r#"if"#`) never count.
fn contains_guard_if(pattern: &str) -> bool {
    let opaque = lex_opaque_ranges(pattern);
    let bytes = pattern.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if opaque
            .iter()
            .any(|(start, end)| index >= *start && index < *end)
        {
            index += 1;
            continue;
        }
        let is_if = bytes[index] == b'i'
            && rest_at(pattern, index).starts_with("if")
            && !is_rust_word_continue_at(pattern, index + 2);
        if is_if {
            let before = if index == 0 {
                None
            } else {
                pattern
                    .get(..index)
                    .and_then(|before| before.chars().next_back())
            };
            if before.is_none_or(|ch| !is_rust_word_char(ch)) {
                return true;
            }
        }
        index += 1;
    }
    false
}

/// Suffix of `text` at byte `index`, or empty when `index` is past the end
/// or inside a multibyte character. Structural scans use this so
/// non-ASCII literal contents can never panic byte slicing.
fn rest_at(text: &str, index: usize) -> &str {
    text.get(index..).unwrap_or("")
}

fn is_rust_word_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn is_rust_word_continue_at(text: &str, index: usize) -> bool {
    text.get(index..)
        .is_some_and(|rest| rest.chars().next().is_some_and(is_rust_word_char))
}

/// Byte ranges (prefix and quotes included) of the string-literal spans in
/// `text`, covering cooked (`"..."`, `b"..."`) and raw (`r"..."`,
/// `r#"..."#`, `br...`) spellings. Comments and character literals never
/// produce spans.
fn string_span_ranges(text: &str) -> Vec<(usize, usize)> {
    lex_strings(text)
        .into_iter()
        .map(|(start, end, _)| (start, end))
        .collect()
}

/// True when `text` holds a string literal that never terminates. The lexer
/// extends such a span to the end of the text, so structural scans fail
/// closed instead of reading past it.
fn has_unterminated_string(text: &str) -> bool {
    // Only an unterminated literal reaches the end of the text without a
    // decoded value; an invalid escape keeps its closing quote and span.
    lex_strings(text)
        .into_iter()
        .any(|(_, end, value)| value.is_none() && end == text.len() && !text.is_empty())
}

/// Decoded string-literal values in `text`, sorted and deduplicated.
/// A raw spelling contributes its verbatim content while a cooked spelling
/// contributes its unescaped value, so spellings with different decoded
/// values never equate. An undecodable or unterminated literal contributes
/// nothing (fail closed).
fn match_arm_string_values(text: &str) -> Vec<String> {
    let mut values = lex_strings(text)
        .into_iter()
        .filter_map(|(_, _, value)| value)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

/// Byte ranges of every lexically opaque region in `text`: string literals
/// (cooked and raw), character literals, line comments, and (nesting-aware)
/// block comments. Structural scans skip these ranges so literal contents
/// never read as syntax.
fn lex_opaque_ranges(text: &str) -> Vec<(usize, usize)> {
    let mut opaque = Vec::new();
    for (start, end, _) in lex_strings(text) {
        opaque.push((start, end));
    }
    let mut index = 0usize;
    let bytes = text.as_bytes();
    while index < bytes.len() {
        if opaque
            .iter()
            .any(|(start, end)| index >= *start && index < *end)
        {
            index += 1;
            continue;
        }
        let rest = rest_at(text, index);
        if rest.starts_with("//") {
            let end = rest.find('\n').map_or(text.len(), |offset| index + offset);
            opaque.push((index, end));
            index = end;
            continue;
        }
        if rest.starts_with("/*") {
            let end = block_comment_end(text, index);
            opaque.push((index, end));
            index = end;
            continue;
        }
        if rest.starts_with('\'')
            && let Some(end) = char_literal_end(text, index)
        {
            opaque.push((index, end));
            index = end;
            continue;
        }
        index += 1;
    }
    opaque
}

/// End byte index of the nesting-aware block comment opening at `start`.
/// An unterminated comment extends to the end of the text so structural
/// scans fail closed instead of reading past it.
fn block_comment_end(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut cursor = start;
    while cursor < bytes.len() {
        if rest_at(text, cursor).starts_with("/*") {
            depth += 1;
            cursor += 2;
        } else if rest_at(text, cursor).starts_with("*/") {
            depth -= 1;
            cursor += 2;
            if depth == 0 {
                break;
            }
        } else {
            cursor += 1;
        }
    }
    cursor
}

/// String literals as `(span_start, span_end, decoded_value)`. Cooked
/// literals decode standard escapes; raw literals contribute verbatim
/// content. Line and (nesting-aware) block comments are skipped, so quoted
/// comment text never contributes a value while genuine strings containing
/// comment delimiters still lex as strings. An undecodable literal keeps
/// its span with no value, and an unterminated literal extends to the end
/// of the text with no value, so downstream structural scans fail closed
/// instead of reading past it.
fn lex_strings(text: &str) -> Vec<(usize, usize, Option<String>)> {
    let mut literals = Vec::new();
    let mut index = 0usize;
    let bytes = text.as_bytes();
    while index < bytes.len() {
        let rest = rest_at(text, index);
        if rest.starts_with("//") {
            index = rest.find('\n').map_or(text.len(), |offset| index + offset);
            continue;
        }
        if rest.starts_with("/*") {
            index = block_comment_end(text, index);
            continue;
        }
        if let Some((end, value)) = raw_string_at(text, index) {
            literals.push((index, end, Some(value)));
            index = end;
            continue;
        }
        if rest.starts_with('\'')
            && let Some(end) = char_literal_end(text, index)
        {
            index = end;
            continue;
        }
        let cooked_start = if rest.starts_with('"') || rest.starts_with("b\"") {
            Some(index)
        } else {
            None
        };
        if let Some(start) = cooked_start {
            let quote = start + usize::from(text[start..].starts_with('b'));
            match cooked_string_at(text, quote) {
                Some((end, value)) => {
                    literals.push((start, end, value));
                    index = end;
                }
                None => {
                    literals.push((start, text.len(), None));
                    break;
                }
            }
            continue;
        }
        index += 1;
    }
    literals
}

/// Raw string at `start` (`r"..."`, `r#"..."#`, `br...`), returning the end
/// byte index and the verbatim content. `None` when no raw string opens
/// here or the terminator is missing (the caller then advances one byte).
fn raw_string_at(text: &str, start: usize) -> Option<(usize, String)> {
    let mut cursor = start;
    if rest_at(text, cursor).starts_with('b') {
        cursor += 1;
    }
    if !rest_at(text, cursor).starts_with('r') {
        return None;
    }
    cursor += 1;
    let mut hashes = 0usize;
    while rest_at(text, cursor).starts_with('#') {
        hashes += 1;
        cursor += 1;
    }
    if !rest_at(text, cursor).starts_with('"') {
        return None;
    }
    cursor += 1;
    let content_start = cursor;
    let bytes = text.as_bytes();
    while cursor < bytes.len() {
        if bytes[cursor] == b'"' {
            let mut tail = cursor + 1;
            let mut seen = 0usize;
            while seen < hashes && tail < bytes.len() && bytes[tail] == b'#' {
                seen += 1;
                tail += 1;
            }
            if seen == hashes {
                return Some((tail, text[content_start..cursor].to_string()));
            }
        }
        cursor += 1;
    }
    None
}

/// Cooked string starting at the opening `"` byte index `quote`, returning
/// the end byte index and the decoded value (`None` value on an invalid
/// escape). `None` when the string never terminates.
fn cooked_string_at(text: &str, quote: usize) -> Option<(usize, Option<String>)> {
    let mut value = String::new();
    let mut valid = true;
    let mut index = quote + 1;
    let bytes = text.as_bytes();
    while index < bytes.len() {
        let ch = rest_at(text, index).chars().next()?;
        if ch == '"' {
            return Some((index + 1, valid.then(|| value.clone())));
        }
        if ch != '\\' {
            value.push(ch);
            index += ch.len_utf8();
            continue;
        }
        match decode_cooked_escape(text, index) {
            Some((consumed, decoded)) => {
                value.push_str(&decoded);
                index += consumed;
            }
            None => {
                valid = false;
                index += 1;
            }
        }
    }
    None
}

/// Decode one cooked escape at the `\` byte index `start`, returning the
/// consumed byte count and the decoded text. Covers `\\`, `\"`, `\'`,
/// `\n`, `\r`, `\t`, `\0`, `\xNN`, `\u{...}`, and the
/// backslash-newline line continuation.
fn decode_cooked_escape(text: &str, start: usize) -> Option<(usize, String)> {
    let rest = &text[start..];
    let mut chars = rest.chars();
    if chars.next() != Some('\\') {
        return None;
    }
    match chars.next()? {
        '\\' => Some((2, "\\".to_string())),
        '"' => Some((2, "\"".to_string())),
        '\'' => Some((2, "'".to_string())),
        'n' => Some((2, "\n".to_string())),
        'r' => Some((2, "\r".to_string())),
        't' => Some((2, "\t".to_string())),
        '0' => Some((2, "\0".to_string())),
        'x' => {
            let digits = rest.get(2..4)?;
            let byte = u8::from_str_radix(digits, 16).ok()?;
            Some((4, (byte as char).to_string()))
        }
        'u' => {
            let braced = rest.strip_prefix("\\u{")?;
            let end = braced.find('}')?;
            let scalar = u32::from_str_radix(&braced[..end], 16).ok()?;
            let decoded = char::from_u32(scalar)?;
            Some((3 + end + 1, decoded.to_string()))
        }
        '\n' => {
            let mut consumed = 2usize;
            for ch in rest[2..].chars() {
                if ch.is_whitespace() {
                    consumed += ch.len_utf8();
                } else {
                    break;
                }
            }
            Some((consumed, String::new()))
        }
        _ => None,
    }
}

/// End byte index of the character literal opening at `start`, or `None`
/// when the `'` begins a lifetime rather than a character literal.
fn char_literal_end(text: &str, start: usize) -> Option<usize> {
    let rest = &text[start + 1..];
    let mut chars = rest.char_indices().peekable();
    let (_, first) = chars.next()?;
    if first == '\n' {
        return None;
    }
    if first == '\\' {
        let mut cursor = 1usize;
        let mut found = false;
        for (offset, ch) in chars {
            cursor = offset + ch.len_utf8();
            if ch == '\'' {
                found = true;
                break;
            }
            if ch == '\n' || cursor > 10 {
                return None;
            }
        }
        if !found {
            return None;
        }
        return Some(start + 1 + cursor);
    }
    let (offset, _) = chars.next()?;
    if rest[offset..].starts_with('\'') {
        return Some(start + 1 + offset + 1);
    }
    None
}

/// Direct string-literal inputs supplied by the observed owner call.
///
/// Match-arm confirmation is intentionally narrower than arbitrary literal
/// occurrence. Only the two compared operands of `assert_eq!` / `assert_ne!`
/// are observed. Diagnostic arguments are ignored. Within a compared operand,
/// the complete expression must be a syntactically bare owner call with one
/// direct string-literal argument. Qualified paths, methods, wrappers,
/// conditionals, blocks, variables, and transformed/nested inputs fail closed.
fn split_top_level_arguments(text: &str) -> Option<Vec<&str>> {
    let opaque = lex_opaque_ranges(text);
    let mut arguments = Vec::new();
    let mut start = 0usize;
    let mut stack = Vec::new();

    for (index, ch) in text.char_indices() {
        if opaque
            .iter()
            .any(|(range_start, range_end)| index >= *range_start && index < *range_end)
        {
            continue;
        }

        match ch {
            '(' | '[' | '{' => stack.push(ch),
            ')' if stack.pop() != Some('(') => return None,
            ']' if stack.pop() != Some('[') => return None,
            '}' if stack.pop() != Some('{') => return None,
            ',' if stack.is_empty() => {
                arguments.push(text[start..index].trim());
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }

    if !stack.is_empty() {
        return None;
    }
    if has_unterminated_string(text) {
        return None;
    }
    arguments.push(text[start..].trim());
    Some(arguments)
}

fn matching_parenthesis(text: &str, opening: usize) -> Option<usize> {
    let opaque = lex_opaque_ranges(text);
    let mut depth = 0usize;

    for (relative, ch) in text[opening..].char_indices() {
        let index = opening + relative;
        if opaque
            .iter()
            .any(|(range_start, range_end)| index >= *range_start && index < *range_end)
        {
            continue;
        }

        match ch {
            '(' => depth += 1,
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn assertion_comparison_operands(text: &str) -> Option<[&str; 2]> {
    let spans = string_span_ranges(text);
    for macro_name in ["assert_eq!", "assert_ne!"] {
        let mut search_from = 0usize;
        while let Some(relative) = text[search_from..].find(macro_name) {
            let start = search_from + relative;
            if spans
                .iter()
                .any(|(span_start, span_end)| start >= *span_start && start < *span_end)
            {
                search_from = start + macro_name.len();
                continue;
            }

            let mut opening = start + macro_name.len();
            while text[opening..]
                .chars()
                .next()
                .is_some_and(char::is_whitespace)
            {
                opening += text[opening..].chars().next()?.len_utf8();
            }
            if text[opening..].chars().next()? != '(' {
                search_from = start + macro_name.len();
                continue;
            }
            let closing = matching_parenthesis(text, opening)?;
            let arguments = split_top_level_arguments(&text[opening + 1..closing])?;
            if arguments.len() < 2 {
                return None;
            }
            return Some([arguments[0], arguments[1]]);
        }
    }
    None
}

fn direct_owner_string_input(expression: &str, owner: &str) -> Option<String> {
    let expression = expression.trim();
    let rest = expression.strip_prefix(owner)?;
    if rest
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return None;
    }

    let rest = rest.trim_start();
    if !rest.starts_with('(') {
        return None;
    }
    let opening = expression.len() - rest.len();
    let closing = matching_parenthesis(expression, opening)?;
    if !expression[closing + 1..].trim().is_empty() {
        return None;
    }

    let arguments = split_top_level_arguments(&expression[opening + 1..closing])?;
    if arguments.len() != 1 {
        return None;
    }
    let argument = arguments[0].trim();
    let spans = string_span_ranges(argument);
    if spans.len() != 1 || spans[0] != (0, argument.len()) {
        return None;
    }
    let mut literals = match_arm_string_values(argument);
    (literals.len() == 1).then(|| literals.remove(0))
}

fn owner_call_literals(text: &str, owner: &str) -> Vec<String> {
    let Some(operands) = assertion_comparison_operands(text) else {
        return Vec::new();
    };
    let mut literals = operands
        .into_iter()
        .filter_map(|operand| direct_owner_string_input(operand, owner))
        .collect::<Vec<_>>();
    literals.sort();
    literals.dedup();
    literals
}

/// Returns `(matched, has_token_match)`.
///
/// `matched` is true when the assertion should be associated with this probe
/// (via token text, family kind, or single-assertion escape hatch).
/// `has_token_match` is true when the assertion text contains an identifier
/// token from the probe expression that is specific enough to confirm this
/// particular sub-expression is being observed.
///
/// For `MatchArm` probes, `has_token_match` uses only the **variant** tokens
/// (`match_arm_variants`, the identifiers immediately after `::` in the probe
/// expression). This prevents a sibling-arm assertion like `Mode::Warm` from
/// clearing `observation_unverified` for a probe on `Mode::Frozen`, because
/// the shared qualifier token `Mode` is excluded from the confirmation set.
/// When the expression contains no `::` (e.g. `None => 0,`), the variant
/// token list is empty and variant matching contributes nothing. String
/// literals in the arm PATTERN additionally confirm, but only when supplied
/// as inputs to the changed owner (`route("sensor")`): result-side literals
/// never confirm (a sibling returning the same literal does not select this
/// arm), and diagnostic/message literals never confirm. The owner's bare
/// callee name scopes the match; without a known owner nothing confirms.
///
/// For probes whose changed expression constructs an exact error variant,
/// with `ExactErrorVariant` assertions (RIPR-SPEC-0106, Part B): when
/// `error_construction_variant` is `Some`, the oracle is gated on the changed
/// expression's specific variant token:
/// - `ErrorPath` probes: the assertion must pin that variant to match at all.
///   A sibling-variant assertion (`CalcError::Negative`) does not associate
///   with a `CalcError::TooLarge` probe — both share the `CalcError`
///   qualifier token, but only the variant token (`TooLarge`) is specific.
/// - Other direct families (e.g. a `return_value` or `field_construction`
///   probe on an `Err(...)` construction): the assertion stays associated
///   through the standard match rules, but only a variant-pinned text sets
///   `has_token_match`, so a sibling-variant oracle leaves observation
///   unverified instead of crediting discrimination for an unrelated seam.
///
/// Without `error_construction_variant` (probe has no parseable variant),
/// falls back to the standard `token_match` behavior — except for #3700
/// wrapper error seams (`context.wrapper_seam` is `true`), where lexical
/// matching can never confirm observation: the variant binding of a
/// `map_err` conversion is not statically establishable, so the seam stays
/// below `exposed` and carries the typed
/// `wrapper_error_binding_unresolved` limitation.
///
/// A `GuardedResultMatch` assertion confirms a probe only through the
/// producer-owned binding, under five #3731 fail-closed gates: the
/// synthesized text must embed a BARE one-segment scrutinee
/// (`match <owner>(..)` — a qualified path's identity is unresolvable
/// here, #3727), when the changed expression constructs an exact
/// error variant the guarded pin must name that exact variant, the
/// related test's file must not import the owner callee's bare name from
/// a FOREIGN path (a same-name import makes the bare binding ambiguous —
/// see `file_imports_foreign_callee_name`), the test's own package
/// must not define a same-named function while the changed owner lives in
/// another package (a bare call may bind the test package's own function —
/// the cross-package ambiguity gate), and — for a return-value probe whose
/// changed value is the SUCCESS payload — the match's Ok arm must observe
/// the unwrapped value (RIPR-SPEC-0175; the fact's extraction-time
/// `ok_value_observed` decision): a routing form with no Ok arm and a
/// payload-ignoring `Ok(_) => ..` arm never observe a changed Ok value, so
/// those confirmations are refused (fail closed, under-credit). For a
/// variant-carrying probe
/// the qualifier token is not a specificity signal, so the guarded
/// oracle's `has_token_match` is exactly the variant-gated owner binding.
fn assertion_matches_probe_detail_with_literals(
    context: &RevealMatchContext,
    assertion: &OracleFact,
    assertion_count: usize,
    import_defeats_owner: bool,
    cross_package_defeats_owner: bool,
) -> (bool, bool) {
    let RevealMatchContext {
        probe_tokens,
        effect_literals,
        match_arm_variants,
        match_arm_literals,
        match_arm_guarded,
        error_construction_variant,
        family,
        wrapper_seam,
        owner_callee,
    } = *context;
    let token_match = probe_tokens
        .iter()
        .any(|token| contains_as_whole_word(&assertion.text, token));
    let effect_literal_match = !effect_literals.is_empty()
        && rust_string_literals(&assertion.text)
            .iter()
            .any(|literal| effect_literals.contains(literal));
    // #3709: a guarded Result match whose scrutinee directly calls the
    // probe's owner observes the owner's returned `Result` — the sink every
    // value/error behavior in the owner flows through — regardless of the
    // changed line's tokens. The oracle's text embeds the scrutinee callee,
    // so the binding is same-entity by name, not token coincidence; a
    // shadowed callee never produces the oracle (extraction-side defeat),
    // and only the result-defined families (ErrorPath, ReturnValue) credit:
    // a changed effect or call inside the owner need not flow through the
    // matched result, so those families keep their existing observers.
    //
    // #3731 review, four fail-closed gates on the owner shortcut:
    // - BARE scrutinee only. The synthesized text embeds the scrutinee path
    //   after `match `, so a one-segment scrutinee reads `match <callee>(`;
    //   a qualified path (`match helpers::parse(..)`) names an entity this
    //   lexical view cannot resolve to the probe owner (an imported or
    //   re-exported same-named callee is exactly the token-coincidence
    //   family), so its observation stays unverified. Qualified-path
    //   identity resolution is the #3727 follow-up.
    // - Exact variant when the changed expression constructs one. A probe
    //   with an `error_construction_variant` is confirmed only when the
    //   guarded oracle's pin names that exact variant; a sibling-variant
    //   (or type-only) guard does not observe the changed error
    //   (RIPR-SPEC-0106 Part B, mirrored from the ExactErrorVariant gate).
    // - No foreign same-name import in the related test's file (F11). An
    //   import of the callee's bare name from a path outside the analyzed
    //   crate makes the bare binding ambiguous between the owner and the
    //   imported callee, so the confirmation is refused (fail-closed
    //   under-credit). An own-crate import (`use this_crate::callee;` — the
    //   normal integration-test binding) is the owner's own export and does
    //   not defeat.
    // - No same-named function in the test's OWN package when the owner
    //   lives in another package (G1). The RustIndex knows the test
    //   package's own functions; a bare `match <callee>(..)` scrutinee in
    //   that test may bind the local definition instead of the owner, so
    //   the confirmation is refused (fail-closed under-credit).
    // - Ok-arm observation for success-payload return-value probes
    //   (RIPR-SPEC-0175). A return-value probe whose changed expression
    //   does NOT construct an exact error variant is a probe on the
    //   owner's returned success value: it is discriminated only when the
    //   match's Ok arm observes the unwrapped value. The routing form (no
    //   Ok arm — the success value flows into a trivial catch-all) and a
    //   payload-ignoring `Ok(_) => ..` arm never observe a changed Ok
    //   value, so the confirmation is refused (fail closed, under-credit;
    //   the extraction-time `ok_value_observed` decision rides the fact).
    //   A return-value probe on an exact Err construction keeps the
    //   Err-guard discriminator — the pin gate above names the changed
    //   variant — the same principle that leaves ErrorPath probes
    //   unchanged here.
    let producer_owned_result = owner_callee.is_some_and(|owner| {
        matches!(assertion.kind, OracleKind::GuardedResultMatch)
            && matches!(family, ProbeFamily::ErrorPath | ProbeFamily::ReturnValue)
            && !import_defeats_owner
            && !cross_package_defeats_owner
            && guarded_oracle_names_bare_callee(&assertion.text, owner)
            && error_construction_variant
                .is_none_or(|variant| contains_as_whole_word(&assertion.text, variant))
            && (matches!(family, ProbeFamily::ErrorPath)
                || error_construction_variant.is_some()
                || assertion.ok_value_observed == Some(true))
    });
    // For MatchArm probes, restrict the confirmation check to variant-only
    // tokens (post-`::`). The qualifier ("Mode" in "Mode::Frozen") is shared
    // across all arms and therefore cannot confirm this specific arm.
    // For #3700 wrapper error seams there is no confirmation signal at all:
    // every lexical overlap between the seam expression and a witness text
    // (parameter names, the callee name, `Into::into`, a message string) is
    // token coincidence by construction, so observation stays unverified and
    // the seam cannot read `exposed` from lexical heuristics.
    let has_token_match = if matches!(family, ProbeFamily::MatchArm) {
        !match_arm_guarded
            && (match_arm_variants
                .iter()
                .any(|v| contains_as_whole_word(&assertion.text, v))
                || !match_arm_literals.is_empty()
                    && !import_defeats_owner
                    && !cross_package_defeats_owner
                    && owner_callee.is_some_and(|owner| {
                        owner_call_literals(&assertion.text, owner)
                            .iter()
                            .any(|literal| match_arm_literals.contains(literal))
                    }))
    } else if wrapper_seam {
        // A #3700 wrapper error seam stays unconfirmable: see above.
        false
    } else if matches!(assertion.kind, OracleKind::GuardedResultMatch)
        && error_construction_variant.is_some()
    {
        // Mirror of the ExactErrorVariant gate (#3731 review): for a
        // variant-carrying probe, a guarded Result match confirms only
        // through the variant-gated owner binding above. The shared
        // enum-qualifier token (`ParseError` in `Err(ParseError::..)`) is
        // not a specificity signal — a guard pinning a sibling variant of
        // the same error type would otherwise clear the unverified flag.
        producer_owned_result
    } else {
        token_match || effect_literal_match || producer_owned_result
    };
    // Fail-closed: if error_construction_variant is None (no parseable variant
    // in the probe), fall through to the standard token_match + family_match
    // check below.
    if matches!(assertion.kind, OracleKind::ExactErrorVariant)
        && let Some(variant) = error_construction_variant
    {
        let variant_matches = contains_as_whole_word(&assertion.text, variant);
        if matches!(family, ProbeFamily::ErrorPath) {
            return (variant_matches, variant_matches);
        }
        return (token_match || effect_literal_match, variant_matches);
    }
    let family_match = oracle_matches_family(family, assertion);
    let matched = token_match
        || effect_literal_match
        || family_match
        || producer_owned_result
        || assertion_count == 1;
    (matched, has_token_match)
}

#[cfg(test)]
#[allow(
    clippy::too_many_arguments,
    reason = "test-only mirror of the grouped RevealMatchContext inputs"
)]
fn assertion_matches_probe_detail(
    probe_tokens: &[String],
    match_arm_variants: &[String],
    match_arm_literals: &[String],
    error_construction_variant: Option<&str>,
    family: &ProbeFamily,
    assertion: &OracleFact,
    assertion_count: usize,
    owner_callee: Option<&str>,
) -> (bool, bool) {
    assertion_matches_probe_detail_with_literals(
        &RevealMatchContext {
            probe_tokens,
            effect_literals: &[],
            match_arm_variants,
            match_arm_literals,
            match_arm_guarded: false,
            error_construction_variant,
            family,
            wrapper_seam: false,
            owner_callee,
        },
        assertion,
        assertion_count,
        false,
        false,
    )
}

/// #3731 review (F11, F22): whether the related test's file imports the
/// owner callee's bare name FROM A FOREIGN PATH — a `use` binding whose
/// first path segment is neither `crate`/`self`/`super` nor one of the
/// analyzed workspace's own package names (`crate_names`). Such an import
/// makes a bare `match <callee>(..)` scrutinee ambiguous between the
/// changed owner and the imported same-named callee, so the reveal-side
/// owner binding must not confirm (fail closed, under-credit). An
/// own-crate import (`use this_crate::callee;` — the normal
/// integration-test binding of the changed owner) binds the owner itself
/// and does not defeat.
///
/// Bounded lexical scan over the masked file source covering ALL `use`
/// declarations (#3731 review F22): file-level items, module-nested `use`s
/// (`mod tests { use other::expect_response; .. }` — the historical
/// harness shape), and function-local imports. A binding is the terminal
/// `::` segment of an import item — a simple path or a (nested) brace-list
/// item; a `callee as alias` rename binds the alias, not the name. Glob
/// (`use p::*;`) imports prove nothing and are not detected, and a
/// brace-rooted `use {..};` with no path prefix counts as foreign (its
/// binding target is not statically the owner's own export) — bounded-scan
/// residuals; parser-backed import resolution is #3727. Scanning past
/// module boundaries can defeat a confirmation for a test the nested
/// import is not visible to — a documented under-credit residual, since
/// lexical scope resolution is exactly what this scan cannot do.
pub(in crate::analysis) fn file_imports_foreign_callee_name(
    source: &str,
    callee: &str,
    crate_names: &std::collections::BTreeSet<String>,
) -> bool {
    if callee.is_empty() {
        return false;
    }
    let masked = crate::analysis::extract::mask_comments_and_strings(source);
    for statement in all_use_statements(&masked) {
        let statement = statement.trim();
        let statement = statement.strip_suffix(';').unwrap_or(statement).trim_end();
        let Some(first_segment) = use_statement_first_segment(statement) else {
            continue;
        };
        // Both sides compare in crate-identifier form (#3731 review F23):
        // a hyphenated package name (`foo-bar`) is imported through its
        // underscore identifier (`foo_bar`), so every stored crate name
        // admits both spellings.
        let own = crate_names
            .iter()
            .any(|name| name == first_segment || crate_identifier(name) == first_segment);
        let foreign =
            first_segment != "crate" && first_segment != "self" && first_segment != "super" && !own;
        if foreign && use_statement_binds_name(statement, callee) {
            return true;
        }
    }
    false
}

/// The crate-identifier form of a manifest name: hyphens normalize to
/// underscores in crate identifiers, so a package named `foo-bar` is
/// imported as `foo_bar` and an import gate must treat the two spellings
/// as the same crate (#3731 review F23).
fn crate_identifier(name: &str) -> String {
    name.replace('-', "_")
}

/// Every `use` declaration in a (masked) source, at any brace depth: the
/// statement runs from a whole-word `use` keyword to its terminating `;`
/// (a `use` path or brace list cannot contain `;`, and comments/strings
/// are already masked, so a masked-out `use` inside a string or comment
/// never appears). Consuming each statement whole keeps a later
/// same-tuned text inside one import from re-matching.
fn all_use_statements(masked: &str) -> Vec<String> {
    let bytes = masked.as_bytes();
    let mut out = Vec::new();
    let mut index = 0usize;
    while index < bytes.len() {
        let use_starts = bytes[index] == b'u'
            && bytes.get(index + 1) == Some(&b's')
            && bytes.get(index + 2) == Some(&b'e')
            && (index == 0 || !is_ident_byte(bytes[index - 1]))
            && bytes[index + 3..]
                .first()
                .is_some_and(|byte| byte.is_ascii_whitespace());
        if use_starts {
            let mut end = index;
            while end < bytes.len() && bytes[end] != b';' {
                end += 1;
            }
            if end < bytes.len() {
                out.push(masked[index..=end].to_string());
                index = end + 1;
                continue;
            }
        }
        index += 1;
    }
    out
}

fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// The first path segment of a `use` statement (the keyword is still
/// present): `use crate::x::y;` -> `crate`, `use a::b::{c};` -> `a`. An
/// empty segment (a brace-rooted `use {..};`) signals no path prefix.
fn use_statement_first_segment(statement: &str) -> Option<&str> {
    let rest = statement.trim_start().strip_prefix("use")?;
    let rest = rest.trim_start();
    let end = rest
        .find(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        .unwrap_or(rest.len());
    Some(&rest[..end])
}

/// Whether the `use` statement binds `callee` as an imported item: the
/// terminal `::` segment of a brace-less path, or any brace-list item
/// (nested brace lists recurse). `as` renames bind the alias; `*` and
/// `self` bind nothing nameable here.
fn use_statement_binds_name(statement: &str, callee: &str) -> bool {
    let Some(rest) = statement.trim_start().strip_prefix("use") else {
        return false;
    };
    use_items_bind(rest.trim_start(), callee)
}

/// Whether one comma-separated `use` item group binds `callee`. An item is
/// a `::`-separated path that may end in a brace list; the binding of a
/// brace-less item is its terminal segment (respecting `as` renames), and
/// brace-list items recurse.
fn use_items_bind(items: &str, callee: &str) -> bool {
    for item in split_top_level_commas(items) {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        match item.find('{') {
            None => {
                if braceless_item_binds(item, callee) {
                    return true;
                }
            }
            Some(open) => {
                if let Some(close) = matching_brace_close(item, open)
                    && use_items_bind(&item[open + 1..close], callee)
                {
                    return true;
                }
            }
        }
    }
    false
}

/// The binding name of a brace-less import item: its terminal `::`
/// segment, with `callee as alias` renames resolving to the alias.
fn braceless_item_binds(item: &str, callee: &str) -> bool {
    let terminal = item.rsplit("::").next().unwrap_or(item).trim();
    let mut parts = terminal.split_whitespace();
    let name = parts.next().unwrap_or("");
    if parts.next() == Some("as") {
        return parts.next() == Some(callee);
    }
    name == callee && name != "*" && name != "self"
}

/// The comma-separated top-level (depth-0) slices of an item list.
fn split_top_level_commas(items: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    for (index, character) in items.char_indices() {
        match character {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                out.push(&items[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    out.push(&items[start..]);
    out
}

/// The byte offset of the `}` closing the `{` at `open`, or `None`.
fn matching_brace_close(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0i32;
    for (index, character) in text[open..].char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open + index);
                }
            }
            _ => {}
        }
    }
    None
}

/// Whether the changed expression is a wrapper error seam: a `map_err`
/// conversion over a callee result whose error identity the seam expression
/// itself does not spell out (no `Err(..)` construction). This is the #3700
/// boxed-wrapper shape — `try_parse_summary(raw).map_err(Into::into)` — where
/// the propagated variant lives in the callee, not in the changed line.
pub(in crate::analysis) fn wrapper_error_seam_expression(expressions: &[&str]) -> bool {
    expressions
        .iter()
        .any(|expression| last_top_level_map_err_dot(expression).is_some())
}

/// Strips grouping that wraps the ENTIRE expression — balanced `(..)` and
/// `{..}` pairs whose opener is the first character and whose closer is the
/// last — repeatedly (`(try_x(raw).map_err(Into::into));` -> the inner
/// conversion, #3714 round-2 review, devin hGdAZ). Grouping that does NOT
/// span the whole expression (`(a) + (b.map_err(f))`) is left in place:
/// those shapes fail closed instead of crediting an ambiguous conversion.
pub(in crate::analysis) fn without_harmless_outer_groups(expression: &str) -> &str {
    // A trailing statement semicolon must not defeat the group-span check.
    let mut working = expression.trim();
    if let Some(stripped) = working.strip_suffix(';') {
        working = stripped.trim_end();
    }
    loop {
        let bytes = working.as_bytes();
        if bytes.first() != Some(&b'(') && bytes.first() != Some(&b'{') {
            return working;
        }
        let opener = bytes[0];
        let closer = if opener == b'(' { b')' } else { b'}' };
        let mut depth = 0isize;
        let mut matched = false;
        for (index, byte) in bytes.iter().enumerate() {
            if *byte == opener {
                depth += 1;
            } else if *byte == closer {
                depth -= 1;
                if depth == 0 {
                    matched = index == bytes.len() - 1;
                    break;
                }
            }
        }
        if !matched {
            return working;
        }
        working = &working[1..working.len() - 1];
        working = working.trim();
    }
}

/// The byte index of the `.` opening the LAST top-level `.map_err(..)`
/// conversion in `expression`. Top-level means bracket depth zero, with
/// string literals, char literals, and lifetimes skipped (a `'` that does
/// not close as a character literal is a lifetime or loop label, not a
/// literal — #3714 round-2 review, devin hDRNH). Single shared authority
/// for wrapper-seam detection (decision.rs, the limitation limiter, and
/// the #3714 related-test attribution) so syntax fixes cannot make the
/// paths disagree (#3714 round-2 review, devin hDRP-). `None` when no
/// `.map_err(..)` conversion opens at depth zero.
pub(in crate::analysis) fn last_top_level_map_err_dot(expression: &str) -> Option<usize> {
    let expression = without_harmless_outer_groups(expression);
    let bytes = expression.as_bytes();
    let mut depth = 0isize;
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;
    let mut last = None;
    let mut index = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            index += 1;
            continue;
        }
        if in_char {
            if escaped {
                escaped = false;
            } else if byte == b'\'' {
                in_char = false;
            }
            index += 1;
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'\'' => {
                // A quote opens a char literal only in the `'x'` and
                // `'\x'` forms; lifetimes (`'_`, `'a`, `'ctx`) and loop
                // labels have no closing quote and are skipped.
                let tail = &bytes[index + 1..];
                let opens_char = (tail.first() == Some(&b'\\') && tail.get(2) == Some(&b'\''))
                    || tail.first() == Some(&b'\'')
                    || (tail
                        .first()
                        .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_')
                        && tail.get(1) == Some(&b'\''));
                if opens_char {
                    in_char = true;
                }
                index += 1;
            }
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b'.' if depth == 0 => {
                let rest = &expression[index + 1..];
                let name = rest.trim_start();
                if let Some(after_name) = name.strip_prefix("map_err") {
                    let token_is_whole = after_name
                        .chars()
                        .next()
                        .is_none_or(|ch| !(ch.is_ascii_alphanumeric() || ch == '_'));
                    let followed_by_call = after_name.trim_start().starts_with('(');
                    if token_is_whole && followed_by_call {
                        last = Some(index);
                    }
                }
            }
            _ => {}
        }
        index += 1;
    }
    last
}

/// Establish the wrapper-to-variant binding for a wrapper error seam.
///
/// The seam's changed expression carries no parseable variant, so the
/// propagated variant identity must come from a witness whose exact-variant
/// pin demonstrably constrains the callee's own result: a related test that
/// calls the seam's callee, where the `matches!` scrutinee either calls the
/// callee directly or names a variable bound from a callee call in the test
/// body. The stored identity is the qualified `Enum::Variant` path, so equal
/// terminal variant names from different enums cannot align. A witness that
/// only calls the wrapper, only names a variant in message text, or pins a
/// variant against another call establishes nothing (#3700).
/// Check whether `text` contains `token` as a whole word — delimited by
/// non-identifier characters (or string boundaries) on both sides. This
/// replaces the old `token.len() > 3` gate, which filtered out short tokens
/// like `id`, `key`, `sum` entirely. With word-boundary matching, a short
/// token matches `result.id` or `id == 42` but NOT `provider` or `middle`
/// (#2397).
///
/// Uses `is_ident_char` (alphanumeric + underscore) for boundary checks,
/// matching Rust identifier rules: `_` is part of an identifier, so `err`
/// does NOT match inside `is_err`.
pub(in crate::analysis) fn contains_as_whole_word(text: &str, token: &str) -> bool {
    fn is_ident_char(byte: u8) -> bool {
        byte.is_ascii_alphanumeric() || byte == b'_'
    }
    if token.is_empty() {
        return false;
    }
    let mut start = 0;
    while let Some(pos) = text[start..].find(token) {
        let abs_pos = start + pos;
        let end_pos = abs_pos + token.len();
        let before_ok = abs_pos == 0 || !is_ident_char(text.as_bytes()[abs_pos - 1]);
        let after_ok = end_pos >= text.len() || !is_ident_char(text.as_bytes()[end_pos]);
        if before_ok && after_ok {
            return true;
        }
        start = abs_pos + 1;
    }
    false
}

fn finalize_related_tests(mut related: Vec<RelatedTest>) -> Vec<RelatedTest> {
    related.sort_by(|a, b| a.name.cmp(&b.name).then(a.line.cmp(&b.line)));
    related.dedup_by(|a, b| a.name == b.name && a.oracle == b.oracle);
    related
}

fn build_observe_evidence(matched_any: bool) -> StageEvidence {
    if matched_any {
        StageEvidence::new(
            StageState::Yes,
            Confidence::Medium,
            "A related test observes a value or effect near the changed behavior",
        )
    } else {
        StageEvidence::new(
            StageState::No,
            Confidence::Medium,
            "Related tests were found, but no assertion appears to observe the changed value, error, field, or effect",
        )
    }
}

fn build_discriminate_evidence(
    strongest: &OracleStrength,
    strongest_kind: &OracleKind,
    family: &ProbeFamily,
    observation_unverified: bool,
) -> StageEvidence {
    // For families that require token confirmation (MatchArm, ReturnValue,
    // FieldConstruction, SideEffect, CallDeletion), a family_match or
    // assertion_count==1 alone cannot confirm that an assertion observes *this*
    // specific changed sub-expression. Without a token_match, downgrade to Weak
    // so classify() emits weakly_exposed (observation_unverified). A probe
    // with a token_match stays exposed.
    if observation_unverified {
        return StageEvidence::new(
            StageState::Weak,
            Confidence::Medium,
            "Discriminator unconfirmed: no assertion text references this probe's changed expression (observation_unverified)",
        );
    }
    match strongest {
        OracleStrength::Strong => StageEvidence::new(
            StageState::Yes,
            Confidence::Medium,
            match strongest_kind {
                OracleKind::ExactErrorVariant => {
                    "Strong oracle found: exact error variant assertion"
                }
                OracleKind::GuardedResultMatch => {
                    "Strong oracle found: guarded Result match over the changed owner's result"
                }
                OracleKind::WholeObjectEquality => {
                    "Strong oracle found: whole-object equality assertion"
                }
                _ => "Strong oracle found: exact value or pattern assertion",
            },
        ),
        OracleStrength::Medium => StageEvidence::new(
            StageState::Weak,
            Confidence::Medium,
            match strongest_kind {
                OracleKind::Snapshot => {
                    "Medium oracle found: snapshot assertion observes the changed behavior"
                }
                OracleKind::MockExpectation => {
                    "Medium oracle found: mock or expectation observes the changed behavior"
                }
                OracleKind::GuardedResultMatch => {
                    "Medium oracle found: guarded Result match pins the error type, not an exact variant"
                }
                _ => "Medium oracle found: property or partial structural assertion",
            },
        ),
        OracleStrength::Weak => StageEvidence::new(
            StageState::Weak,
            Confidence::High,
            match (strongest_kind, family) {
                (OracleKind::BroadError, ProbeFamily::ErrorPath) => {
                    "Only broad error oracle found; is_err() does not discriminate exact error variants"
                }
                (OracleKind::BroadError, _) => {
                    "Only broad error oracle found; it may not discriminate the changed behavior exactly"
                }
                (OracleKind::RelationalCheck, _) => {
                    "Only relational oracle found; it may not discriminate the changed value exactly"
                }
                _ => {
                    "Only weak oracle found, such as a broad relational assertion or non-empty check"
                }
            },
        ),
        OracleStrength::Smoke => StageEvidence::new(
            StageState::Weak,
            Confidence::High,
            "Only smoke oracle found, such as unwrap/expect or execution without a discriminator",
        ),
        OracleStrength::None => StageEvidence::new(
            StageState::No,
            Confidence::Medium,
            "No assertion found on related tests",
        ),
        OracleStrength::Unknown => StageEvidence::new(
            StageState::Unknown,
            Confidence::Low,
            "Assertions exist, but oracle strength is unknown",
        ),
    }
}

/// Typed family/kind relationships are kept in data so the supported oracle
/// shapes can be audited without following nested family and kind matches.
/// Text heuristics remain below because they are intentionally conservative
/// fallback signals, not typed oracle classifications. `ProbeFamily::StaticUnknown`
/// has no typed relationships.
const ORACLE_FAMILY_MATCHES: &[(ProbeFamily, OracleKind)] = &[
    (ProbeFamily::ErrorPath, OracleKind::ExactErrorVariant),
    (ProbeFamily::ErrorPath, OracleKind::BroadError),
    (ProbeFamily::SideEffect, OracleKind::MockExpectation),
    (ProbeFamily::FieldConstruction, OracleKind::ExactValue),
    (
        ProbeFamily::FieldConstruction,
        OracleKind::WholeObjectEquality,
    ),
    (ProbeFamily::FieldConstruction, OracleKind::RelationalCheck),
    (ProbeFamily::FieldConstruction, OracleKind::Snapshot),
    (ProbeFamily::Predicate, OracleKind::ExactValue),
    (ProbeFamily::Predicate, OracleKind::RelationalCheck),
    (ProbeFamily::Predicate, OracleKind::ExactErrorVariant),
    (ProbeFamily::Predicate, OracleKind::Snapshot),
    (ProbeFamily::ReturnValue, OracleKind::ExactValue),
    (ProbeFamily::ReturnValue, OracleKind::WholeObjectEquality),
    (ProbeFamily::ReturnValue, OracleKind::RelationalCheck),
    (ProbeFamily::ReturnValue, OracleKind::Snapshot),
    (ProbeFamily::ReturnValue, OracleKind::SmokeOnly),
    (ProbeFamily::CallDeletion, OracleKind::MockExpectation),
    (ProbeFamily::CallDeletion, OracleKind::ExactValue),
    (ProbeFamily::CallDeletion, OracleKind::RelationalCheck),
    (ProbeFamily::CallDeletion, OracleKind::SmokeOnly),
    (ProbeFamily::MatchArm, OracleKind::ExactErrorVariant),
    (ProbeFamily::MatchArm, OracleKind::ExactValue),
    (ProbeFamily::MatchArm, OracleKind::RelationalCheck),
    (ProbeFamily::MatchArm, OracleKind::Snapshot),
];

/// Family-specific text fallbacks are typed separately from parsed
/// [`OracleKind`] relationships. They preserve support for conservative custom
/// assertion helpers while keeping text recognition in the assertion-pattern
/// owner rather than in reveal classification.
const ORACLE_FAMILY_TEXT_SHAPES: &[(ProbeFamily, OracleTextShape)] = &[
    (ProbeFamily::ErrorPath, OracleTextShape::ErrorPath),
    (ProbeFamily::SideEffect, OracleTextShape::SideEffect),
    (
        ProbeFamily::FieldConstruction,
        OracleTextShape::MemberAccess,
    ),
    (
        ProbeFamily::CallDeletion,
        OracleTextShape::AssertionOrExpectation,
    ),
];

/// Family-specific strength overrides. A missing entry preserves the
/// classifier's parsed assertion strength; entries are only for kinds whose
/// relative strength is fixed by the probe family.
const ORACLE_FAMILY_STRENGTH_OVERRIDES: &[(ProbeFamily, OracleKind, OracleStrength)] = &[
    (
        ProbeFamily::ErrorPath,
        OracleKind::ExactErrorVariant,
        OracleStrength::Strong,
    ),
    (
        ProbeFamily::ErrorPath,
        OracleKind::SmokeOnly,
        OracleStrength::Smoke,
    ),
    (
        ProbeFamily::ReturnValue,
        OracleKind::ExactValue,
        OracleStrength::Strong,
    ),
    (
        ProbeFamily::ReturnValue,
        OracleKind::ExactErrorVariant,
        OracleStrength::Strong,
    ),
    (
        ProbeFamily::ReturnValue,
        OracleKind::WholeObjectEquality,
        OracleStrength::Strong,
    ),
    (
        ProbeFamily::ReturnValue,
        OracleKind::SmokeOnly,
        OracleStrength::Smoke,
    ),
    (
        ProbeFamily::ReturnValue,
        OracleKind::Unknown,
        OracleStrength::Unknown,
    ),
    (
        ProbeFamily::Predicate,
        OracleKind::ExactValue,
        OracleStrength::Strong,
    ),
    (
        ProbeFamily::Predicate,
        OracleKind::ExactErrorVariant,
        OracleStrength::Strong,
    ),
    (
        ProbeFamily::Predicate,
        OracleKind::WholeObjectEquality,
        OracleStrength::Strong,
    ),
    (
        ProbeFamily::Predicate,
        OracleKind::SmokeOnly,
        OracleStrength::Smoke,
    ),
    (
        ProbeFamily::Predicate,
        OracleKind::Unknown,
        OracleStrength::Unknown,
    ),
    (
        ProbeFamily::FieldConstruction,
        OracleKind::ExactValue,
        OracleStrength::Strong,
    ),
    (
        ProbeFamily::FieldConstruction,
        OracleKind::ExactErrorVariant,
        OracleStrength::Strong,
    ),
    (
        ProbeFamily::FieldConstruction,
        OracleKind::WholeObjectEquality,
        OracleStrength::Strong,
    ),
    (
        ProbeFamily::FieldConstruction,
        OracleKind::SmokeOnly,
        OracleStrength::Smoke,
    ),
    (
        ProbeFamily::FieldConstruction,
        OracleKind::Unknown,
        OracleStrength::Unknown,
    ),
    (
        ProbeFamily::MatchArm,
        OracleKind::ExactValue,
        OracleStrength::Strong,
    ),
    (
        ProbeFamily::MatchArm,
        OracleKind::ExactErrorVariant,
        OracleStrength::Strong,
    ),
    (
        ProbeFamily::MatchArm,
        OracleKind::WholeObjectEquality,
        OracleStrength::Strong,
    ),
    (
        ProbeFamily::MatchArm,
        OracleKind::SmokeOnly,
        OracleStrength::Smoke,
    ),
    (
        ProbeFamily::MatchArm,
        OracleKind::Unknown,
        OracleStrength::Unknown,
    ),
    (
        ProbeFamily::SideEffect,
        OracleKind::ExactValue,
        OracleStrength::Strong,
    ),
    (
        ProbeFamily::SideEffect,
        OracleKind::WholeObjectEquality,
        OracleStrength::Strong,
    ),
    (
        ProbeFamily::SideEffect,
        OracleKind::ExactErrorVariant,
        OracleStrength::Medium,
    ),
    (
        ProbeFamily::SideEffect,
        OracleKind::SmokeOnly,
        OracleStrength::Smoke,
    ),
    (
        ProbeFamily::SideEffect,
        OracleKind::Unknown,
        OracleStrength::Unknown,
    ),
    (
        ProbeFamily::CallDeletion,
        OracleKind::ExactValue,
        OracleStrength::Strong,
    ),
    (
        ProbeFamily::CallDeletion,
        OracleKind::WholeObjectEquality,
        OracleStrength::Strong,
    ),
    (
        ProbeFamily::CallDeletion,
        OracleKind::ExactErrorVariant,
        OracleStrength::Medium,
    ),
    (
        ProbeFamily::CallDeletion,
        OracleKind::SmokeOnly,
        OracleStrength::Smoke,
    ),
    (
        ProbeFamily::CallDeletion,
        OracleKind::Unknown,
        OracleStrength::Unknown,
    ),
    (
        ProbeFamily::StaticUnknown,
        OracleKind::Unknown,
        OracleStrength::Unknown,
    ),
];

fn oracle_matches_family(family: &ProbeFamily, assertion: &OracleFact) -> bool {
    let typed_match = ORACLE_FAMILY_MATCHES
        .iter()
        .any(|(rule_family, rule_kind)| rule_family == family && rule_kind == &assertion.kind);
    let text_shape_match = ORACLE_FAMILY_TEXT_SHAPES
        .iter()
        .any(|(rule_family, shape)| {
            rule_family == family && has_oracle_text_shape(&assertion.text, *shape)
        });
    typed_match || text_shape_match
}

fn probe_relative_oracle_strength(family: &ProbeFamily, assertion: &OracleFact) -> OracleStrength {
    if matches!(family, ProbeFamily::StaticUnknown) {
        return OracleStrength::Unknown;
    }
    ORACLE_FAMILY_STRENGTH_OVERRIDES
        .iter()
        .find(|(rule_family, rule_kind, _)| rule_family == family && rule_kind == &assertion.kind)
        .map_or_else(
            || assertion.strength.clone(),
            |(_, _, strength)| strength.clone(),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn reveal_evidence_keeps_assertionless_related_test_without_observe_signal() {
        let probe = probe(ProbeFamily::ReturnValue, "score");
        let test = test_with_assertions("score_returns_value", Vec::new());
        let (observe, discriminate, related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(observe.state, StageState::No);
        assert_eq!(discriminate.state, StageState::No);
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].name, "score_returns_value");
        assert_eq!(related[0].oracle, None);
    }

    #[test]
    fn reveal_evidence_records_matching_assertions_and_sorts_related_tests() {
        let probe = probe(
            ProbeFamily::ErrorPath,
            "return Err(AuthError::RevokedToken);",
        );
        let late = test_with_assertions(
            "z_error_path",
            vec![oracle(
                "assert!(score(\"\").is_err());",
                OracleKind::BroadError,
                OracleStrength::Weak,
            )],
        );
        let early = test_with_assertions(
            "a_error_path",
            vec![oracle(
                "assert_matches!(score(\"\"), Err(AuthError::RevokedToken));",
                OracleKind::ExactErrorVariant,
                OracleStrength::Strong,
            )],
        );
        let (observe, discriminate, related) = reveal_evidence(
            &probe,
            &[
                (&late, RelationReason::DirectOwnerCall),
                (&early, RelationReason::DirectOwnerCall),
            ],
        );

        assert_eq!(observe.state, StageState::Yes);
        assert_eq!(discriminate.state, StageState::Yes);
        assert_eq!(related.len(), 2);
        assert_eq!(related[0].name, "a_error_path");
        assert_eq!(related[0].oracle_strength, OracleStrength::Strong);
        assert_eq!(related[1].name, "z_error_path");
    }

    #[test]
    fn reveal_evidence_ignores_unmatched_assertions() {
        let probe = probe(ProbeFamily::StaticUnknown, "opaque_changed_expr");
        let test = test_with_assertions(
            "opaque_behavior",
            vec![
                oracle(
                    "assert_eq!(unrelated, 3);",
                    OracleKind::Unknown,
                    OracleStrength::Unknown,
                ),
                oracle(
                    "assert!(other_value);",
                    OracleKind::Unknown,
                    OracleStrength::Unknown,
                ),
            ],
        );
        let (observe, discriminate, related) =
            reveal_evidence(&probe, &[(&test, RelationReason::WeakTokenSubstring)]);

        assert_eq!(observe.state, StageState::No);
        assert_eq!(discriminate.state, StageState::No);
        assert!(related.is_empty());
    }

    #[test]
    fn reveal_evidence_withholds_yes_discriminator_for_weak_token_only_linkage() {
        // #1746: a strong oracle in a test that merely shares a token with
        // the probe observes a nearby value, not necessarily the changed
        // sink. Reach plus oracle strength from token coincidence must not
        // combine into discrimination.
        let probe = probe(ProbeFamily::Predicate, "current.status == recorded.status");
        let coincidental = test_with_assertions(
            "artifact_terminal_status_is_observed",
            vec![oracle(
                "assert_eq!(record.status, \"terminal\");",
                OracleKind::ExactValue,
                OracleStrength::Strong,
            )],
        );
        let (_, discriminate, _) = reveal_evidence(
            &probe,
            &[(&coincidental, RelationReason::WeakTokenSubstring)],
        );

        assert_eq!(discriminate.state, StageState::Weak);
    }

    #[test]
    fn reveal_evidence_withholds_yes_discriminator_for_same_file_other_sink() {
        // #1746: file proximity is not sink identity. A strong oracle in a
        // same-file test asserting a different function cannot discriminate
        // this sink, even though the relation outranks token coincidence.
        let probe = probe(ProbeFamily::Predicate, "current.status == recorded.status");
        let neighbor = test_with_assertions(
            "sanitize_path_trims_underscores",
            vec![oracle(
                "assert_eq!(sanitize_path(\"a:b\"), \"a_b\");",
                OracleKind::ExactValue,
                OracleStrength::Strong,
            )],
        );
        let (_, discriminate, _) =
            reveal_evidence(&probe, &[(&neighbor, RelationReason::SameTestFile)]);

        assert_eq!(discriminate.state, StageState::Weak);
    }

    #[test]
    fn reveal_evidence_keeps_yes_discriminator_for_same_entity_oracle() {
        // Same oracle shape as above, but the test calls the changed owner:
        // the discriminating oracle is same-entity evidence, so Yes stands.
        let probe = probe(ProbeFamily::Predicate, "current.status == recorded.status");
        let aligned = test_with_assertions(
            "binding_status_change_refuses",
            vec![oracle(
                "assert_eq!(record.status, \"terminal\");",
                OracleKind::ExactValue,
                OracleStrength::Strong,
            )],
        );
        let (_, discriminate, _) =
            reveal_evidence(&probe, &[(&aligned, RelationReason::DirectOwnerCall)]);

        assert_eq!(discriminate.state, StageState::Yes);
    }

    #[test]
    fn assertion_matching_accepts_token_family_and_single_assertion_fallbacks() {
        let token_assertion = oracle(
            "assert_eq!(score, 3);",
            OracleKind::Unknown,
            OracleStrength::Unknown,
        );
        let (matched, has_token) = assertion_matches_probe_detail(
            &["score".to_string()],
            &[],
            &[],
            None,
            &ProbeFamily::StaticUnknown,
            &token_assertion,
            2,
            None,
        );
        assert!(matched, "token match must fire");
        assert!(has_token, "token match must set has_token_match");

        let family_assertion = oracle(
            "assert!(result.is_err());",
            OracleKind::BroadError,
            OracleStrength::Weak,
        );
        let (matched, has_token) = assertion_matches_probe_detail(
            &["err".to_string()],
            &[],
            &[],
            None,
            &ProbeFamily::ErrorPath,
            &family_assertion,
            2,
            None,
        );
        assert!(matched, "family match must fire");
        assert!(!has_token, "family-only match must not set has_token_match");

        let fallback_assertion = oracle(
            "assert!(ran);",
            OracleKind::Unknown,
            OracleStrength::Unknown,
        );
        let (matched, has_token) = assertion_matches_probe_detail(
            &["run".to_string()],
            &[],
            &[],
            None,
            &ProbeFamily::StaticUnknown,
            &fallback_assertion,
            1,
            None,
        );
        assert!(matched, "single-assertion fallback must fire");
        assert!(
            !has_token,
            "escape-hatch-only match must not set has_token_match"
        );

        let (matched, _) = assertion_matches_probe_detail(
            &["run".to_string()],
            &[],
            &[],
            None,
            &ProbeFamily::StaticUnknown,
            &fallback_assertion,
            2,
            None,
        );
        assert!(!matched, "fallback must not fire for assertion_count > 1");
    }

    // Literal-arm identity (#1714): an assertion supplying the arm's string
    // literal confirms a MatchArm probe with no `::` variant tokens.
    #[test]
    fn literal_assertion_confirms_literal_match_arm() {
        let aligned = oracle(
            "assert_eq!(route(\"sensor\"), \"sensor-v2\");",
            OracleKind::ExactValue,
            OracleStrength::Strong,
        );
        let (matched, has_token) = assertion_matches_probe_detail(
            &[],
            &[],
            &["sensor".to_string()],
            None,
            &ProbeFamily::MatchArm,
            &aligned,
            2,
            Some("route"),
        );
        assert!(matched, "literal overlap must associate");
        assert!(has_token, "owner-supplied pattern literal must confirm");
    }

    // Sibling rejection (#1714): a sibling arm's literals never overlap this
    // arm's expression, so the sibling oracle leaves it unverified.
    #[test]
    fn sibling_literal_assertion_does_not_confirm_changed_arm() {
        let sibling = oracle(
            "assert_eq!(route(\"focused-test\"), \"proof\");",
            OracleKind::ExactValue,
            OracleStrength::Strong,
        );
        let (matched, has_token) = assertion_matches_probe_detail(
            &[],
            &[],
            &["sensor".to_string()],
            None,
            &ProbeFamily::MatchArm,
            &sibling,
            2,
            Some("route"),
        );
        assert!(
            !has_token,
            "sibling literals must not confirm the changed arm"
        );
        let _ = matched;
    }

    // Shared result (#1714): a sibling returning the same result literal does
    // not select the changed arm, even though the literal overlaps.
    #[test]
    fn shared_result_literal_does_not_confirm_sibling_arm() {
        let shared = oracle(
            "assert_eq!(route(\"focused-test\"), \"sensor-v2\");",
            OracleKind::ExactValue,
            OracleStrength::Strong,
        );
        let (_, has_token) = assertion_matches_probe_detail(
            &[],
            &[],
            &["sensor".to_string()],
            None,
            &ProbeFamily::MatchArm,
            &shared,
            2,
            Some("route"),
        );
        assert!(
            !has_token,
            "result-side overlap must not confirm the changed arm"
        );
    }

    // Only a direct bare owner call in one of the two compared operands
    // supplies an observable literal input. Every other shape fails closed.
    #[test]
    fn owner_call_literal_scope_rejects_longer_callee_names() {
        for assertion in [
            "assert_eq!(reroute(\"sensor\"), 1);",
            "assert_eq!(other::route(\"sensor\"), 1);",
            "assert_eq!(other :: route(\"sensor\"), 1);",
            "assert_eq!(router.route(\"sensor\"), 1);",
            "assert_eq!(router . route(\"sensor\"), 1);",
            "assert_eq!(route(wrap(\"sensor\")), 1);",
            "assert_eq!(route(if false { \"sensor\" } else { \"focused-test\" }), 1);",
        ] {
            assert!(
                owner_call_literals(assertion, "route").is_empty(),
                "unsupported owner-call shape unexpectedly supplied an input: {assertion}"
            );
        }
        assert_eq!(
            owner_call_literals(
                "assert_eq!(route(\"focused-test\"), \"proof\", \"{}\", route(\"sensor\"));",
                "route"
            ),
            vec!["focused-test".to_string()]
        );
        assert_eq!(
            owner_call_literals("assert_eq!(route(\"sensor\"), \"v\");", "route"),
            vec!["sensor".to_string()]
        );
        assert_eq!(
            owner_call_literals("assert_eq!(\"v\", route(\"sensor\"));", "route"),
            vec!["sensor".to_string()]
        );
    }

    #[test]
    fn match_arm_guard_detection_masks_literals_and_comments() {
        assert!(match_arm_pattern_has_guard(
            "\"sensor\" if kind.len() > 10 => \"sensor-v2\""
        ));
        assert!(!match_arm_pattern_has_guard("\"if\" => \"literal\""));
        assert!(!match_arm_pattern_has_guard(
            "/* if */ \"sensor\" => \"sensor-v2\""
        ));
        assert!(!match_arm_pattern_has_guard("Mode::If => 1"));
    }

    #[test]
    fn match_arm_pattern_literals_ignore_comment_text() {
        assert_eq!(
            match_arm_pattern_literals("\"sensor\" /* \"focused-test\" */ => \"sensor-v2\""),
            vec!["sensor".to_string()]
        );
        assert_eq!(
            match_arm_pattern_literals(
                "\"sensor\" /* outer /* r#\"focused-test\"# */ tail */ => \"sensor-v2\""
            ),
            vec!["sensor".to_string()]
        );
        assert_eq!(
            match_arm_pattern_literals("\"sensor\" // \"focused-test\"\n => \"sensor-v2\""),
            vec!["sensor".to_string()]
        );
        assert_eq!(
            match_arm_pattern_literals("/* \"focused-test\" */ \"sensor\" => \"sensor-v2\""),
            vec!["sensor".to_string()]
        );
        assert_eq!(
            match_arm_pattern_literals("// \"focused-test\"\n\"sensor\" => \"sensor-v2\""),
            vec!["sensor".to_string()]
        );
        assert_eq!(
            match_arm_pattern_literals("\"/*\" => \"comment-like\""),
            vec!["/*".to_string()]
        );
        assert_eq!(
            match_arm_pattern_literals("r#\"//\"# => \"comment-like\""),
            vec!["//".to_string()]
        );
        // An unterminated comment swallows the separator, so no arm
        // identity is established at all (fail closed, not a partial
        // literal).
        assert_eq!(
            match_arm_pattern_literals("\"sensor\" /* unterminated => \"sensor-v2\""),
            Vec::<String>::new()
        );
    }

    #[test]
    fn match_arm_literal_confirmation_respects_owner_ambiguity_and_guards() {
        let aligned = oracle(
            "assert_eq!(route(\"sensor\"), \"sensor-v2\");",
            OracleKind::ExactValue,
            OracleStrength::Strong,
        );
        let empty = Vec::<String>::new();
        let pattern_literals = vec!["sensor".to_string()];
        let family = ProbeFamily::MatchArm;

        for (guarded, import_defeats_owner, cross_package_defeats_owner, expected) in [
            (false, false, false, true),
            (true, false, false, false),
            (false, true, false, false),
            (false, false, true, false),
            (false, true, true, false),
        ] {
            let context = RevealMatchContext {
                probe_tokens: &empty,
                effect_literals: &empty,
                match_arm_variants: &empty,
                match_arm_literals: &pattern_literals,
                match_arm_guarded: guarded,
                error_construction_variant: None,
                family: &family,
                wrapper_seam: false,
                owner_callee: Some("route"),
            };
            let (_, has_token) = assertion_matches_probe_detail_with_literals(
                &context,
                &aligned,
                2,
                import_defeats_owner,
                cross_package_defeats_owner,
            );
            assert_eq!(
                has_token, expected,
                "wrong confirmation: guarded={guarded} import={import_defeats_owner} cross_package={cross_package_defeats_owner}"
            );
        }
    }

    // A diagnostic message that spells the owner call binds no input: the
    // callee occurrence inside the message string is not a call.
    #[test]
    fn owner_call_literal_scope_rejects_call_text_inside_message() {
        assert!(
            owner_call_literals(
                "assert!(value == 1, \"route(\\\"sensor\\\") should hold\");",
                "route"
            )
            .is_empty()
        );
    }

    // Variable-bound inputs are not statically visible: `route(input)` with
    // no literal argument supplies nothing, so confirmation fails closed.
    #[test]
    fn owner_call_literal_scope_rejects_variable_bound_inputs() {
        assert!(
            owner_call_literals("assert_eq!(route(input), \"sensor-v2\");", "route").is_empty()
        );
    }

    // Diagnostic text (#1714): a pattern literal appearing only as assertion
    // message text is not an owner-supplied input.
    #[test]
    fn diagnostic_literal_does_not_confirm_changed_arm() {
        let diagnostic = oracle(
            "assert_eq!(route(\"focused-test\"), \"proof\", \"sensor\");",
            OracleKind::ExactValue,
            OracleStrength::Strong,
        );
        let (_, has_token) = assertion_matches_probe_detail(
            &[],
            &[],
            &["sensor".to_string()],
            None,
            &ProbeFamily::MatchArm,
            &diagnostic,
            2,
            Some("route"),
        );
        assert!(
            !has_token,
            "message-only literal must not confirm the changed arm"
        );
    }

    // RIPR-SPEC-0106 Control 2 (SIBLING-VARIANT): a Negative-pinning assertion
    // must not match a TooLarge error_path probe.
    #[test]
    fn sibling_variant_assertion_does_not_match_too_large_probe() {
        let sibling_assertion = oracle(
            "assert_eq!(err, CalcError::Negative);",
            OracleKind::ExactErrorVariant,
            OracleStrength::Strong,
        );
        // Probe expression names TooLarge; error_path_variant = Some("TooLarge").
        let (matched, has_token) = assertion_matches_probe_detail(
            &["CalcError".to_string(), "TooLarge".to_string()],
            &[],
            &[],
            Some("TooLarge"),
            &ProbeFamily::ErrorPath,
            &sibling_assertion,
            2,
            None,
        );
        assert!(
            !matched,
            "sibling-variant Negative assertion must not match TooLarge probe"
        );
        assert!(
            !has_token,
            "sibling-variant assertion must not set has_token_match for TooLarge probe"
        );
    }

    // #3700 (final consolidation): a wrapper error seam never confirms
    // observation from lexical matching — not from a callee-side exact Err
    // pin, not from a wrapper-invoking downcast pin, not from message text —
    // so the seam stays `weakly_exposed` (unverified observation) and the
    // typed `wrapper_error_binding_unresolved` limitation (attached by
    // `apply_wrapper_error_binding_limit`) is the honest outcome instead of
    // an escalating lexical binding heuristic.
    #[test]
    fn wrapper_seam_never_confirms_and_stays_weak_under_strongest_witnesses() {
        let wrapper_probe = probe(
            ProbeFamily::ErrorPath,
            "try_parse_summary(raw).map_err(Into::into)",
        );
        let binder = test_with_body_assertions(
            "try_parse_summary_pins_malformed_source",
            "let result = try_parse_summary(\"@bad;\");",
            vec![oracle(
                "if !matches!(result, Err(ParseSummaryError::MalformedSource)) {
return Err(\"callee pin\".into());
}",
                OracleKind::ExactErrorVariant,
                OracleStrength::Strong,
            )],
        );
        let observer = test_with_body_assertions(
            "parse_summary_boxed_variant_propagates_malformed_source",
            "let error = parse_summary(\"@bad;\").err().ok_or(\"expected error\")?;",
            vec![oracle(
                "if !matches!(error.downcast_ref::<ParseSummaryError>(), Some(ParseSummaryError::MalformedSource)) {
return Err(\"boxed identity should survive\".into());
}",
                OracleKind::ExactValue,
                OracleStrength::Strong,
            )],
        );
        let (observe, discriminate, _) = reveal_evidence(
            &wrapper_probe,
            &[
                (&binder, RelationReason::OwnerNamedTest),
                (&observer, RelationReason::DirectOwnerCall),
            ],
        );
        assert_eq!(observe.state, StageState::Yes);
        assert_eq!(
            discriminate.state,
            StageState::Weak,
            "a wrapper seam without a parseable variant must never read exposed from lexical heuristics"
        );
    }

    // #3700 removal-fails control for the typed path: the parseable-variant
    // site (`return Err(ParseSummaryError::MalformedSource);`) credits only
    // while the witness pins the exact variant against the callee's own
    // result. Removing the variant pin (broad is_err) drops the seam to
    // weakly_exposed — the pre-existing main behavior, no wrapper heuristics
    // involved.
    #[test]
    fn typed_variant_site_loses_credit_when_witness_pin_removed() {
        let typed_probe = probe(
            ProbeFamily::ErrorPath,
            "return Err(ParseSummaryError::MalformedSource);",
        );
        let exact_witness = test_with_body_assertions(
            "try_parse_summary_fails_closed_on_malformed_source",
            "let result = try_parse_summary(\"@bad;\");",
            vec![oracle(
                "if !matches!(result, Err(ParseSummaryError::MalformedSource)) {
return Err(\"typed pin\".into());
}",
                OracleKind::ExactErrorVariant,
                OracleStrength::Strong,
            )],
        );
        let (observe, discriminate, _) = reveal_evidence(
            &typed_probe,
            &[(&exact_witness, RelationReason::OwnerNamedTest)],
        );
        assert_eq!(observe.state, StageState::Yes);
        assert_eq!(
            discriminate.state,
            StageState::Yes,
            "the parseable-variant path keeps the pre-existing variant-bound credit"
        );

        let broad_witness = test_with_body_assertions(
            "try_parse_summary_fails_closed_on_malformed_source",
            "let result = try_parse_summary(\"@bad;\");",
            vec![oracle(
                "assert!(result.is_err());",
                OracleKind::BroadError,
                OracleStrength::Weak,
            )],
        );
        let (_, degraded, _) = reveal_evidence(
            &typed_probe,
            &[(&broad_witness, RelationReason::OwnerNamedTest)],
        );
        assert_eq!(
            degraded.state,
            StageState::Weak,
            "removing the variant pin must fail the typed path's credit"
        );
    }

    // RIPR-SPEC-0106 Control 1 (POSITIVE): exact variant assertion DOES match
    // the probe when the specific variant token is present.
    #[test]
    fn exact_variant_assertion_matches_matching_probe() {
        let exact_assertion = oracle(
            "assert_eq!(err, CalcError::Negative);",
            OracleKind::ExactErrorVariant,
            OracleStrength::Strong,
        );
        let (matched, has_token) = assertion_matches_probe_detail(
            &["CalcError".to_string(), "Negative".to_string()],
            &[],
            &[],
            Some("Negative"),
            &ProbeFamily::ErrorPath,
            &exact_assertion,
            2,
            None,
        );
        assert!(
            matched,
            "exact variant assertion must match the probe when variant token matches"
        );
        assert!(
            has_token,
            "matching variant assertion must set has_token_match"
        );
    }

    // #3700 callee-extraction pins: the converted callee is the final
    // top-level call segment before `.map_err(..)` — receiver qualification,
    // chaining, and turbofish must not misattribute it, and non-call shapes
    // fail closed to `None`.

    #[test]
    fn sibling_variant_assertion_does_not_confirm_return_value_error_construction() {
        let sibling_assertion = oracle(
            "assert_eq!(err, CalcError::Negative);",
            OracleKind::ExactErrorVariant,
            OracleStrength::Strong,
        );
        let (matched, has_token) = assertion_matches_probe_detail(
            &[
                "return".to_string(),
                "Err".to_string(),
                "CalcError".to_string(),
                "TooLarge".to_string(),
            ],
            &[],
            &[],
            Some("TooLarge"),
            &ProbeFamily::ReturnValue,
            &sibling_assertion,
            2,
            None,
        );
        assert!(
            matched,
            "the sibling test still observes near the changed behavior and stays associated"
        );
        assert!(
            !has_token,
            "a Negative pin must not confirm observation of a TooLarge construction"
        );
    }

    #[test]
    fn aligned_variant_assertion_confirms_return_value_error_construction() {
        let aligned_assertion = oracle(
            "assert_eq!(result.unwrap_err(), CalcError::TooLarge);",
            OracleKind::ExactErrorVariant,
            OracleStrength::Strong,
        );
        let (matched, has_token) = assertion_matches_probe_detail(
            &[
                "return".to_string(),
                "Err".to_string(),
                "CalcError".to_string(),
                "TooLarge".to_string(),
            ],
            &[],
            &[],
            Some("TooLarge"),
            &ProbeFamily::ReturnValue,
            &aligned_assertion,
            2,
            None,
        );
        assert!(matched);
        assert!(
            has_token,
            "an oracle pinning the constructed variant confirms observation"
        );
    }

    // XQFf: the error-construction variant guard must also cover
    // `FieldConstruction` probes — a probe whose field is an `Err(...)`
    // construction (`outcome: Err(CalcError::TooLarge)`) has no separate
    // `error_construction_variant` source, so without the guard a
    // sibling-variant oracle (`CalcError::Negative`) confirmed observation
    // through the shared `CalcError` qualifier token.
    #[test]
    fn sibling_variant_assertion_does_not_confirm_field_construction_error_construction() {
        let probe = probe(
            ProbeFamily::FieldConstruction,
            "outcome: Err(CalcError::TooLarge)",
        );
        let test = test_with_assertions(
            "negative_outcome_asserted",
            vec![oracle(
                "assert_eq!(err, CalcError::Negative);",
                OracleKind::ExactErrorVariant,
                OracleStrength::Strong,
            )],
        );
        let (observe, discriminate, related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(observe.state, StageState::Yes);
        assert_eq!(
            discriminate.state,
            StageState::Weak,
            "the sibling Negative pin must not confirm the TooLarge field construction"
        );
        assert!(
            discriminate.summary.contains("observation_unverified"),
            "sibling-variant oracle must leave field construction observation unverified: got `{}`",
            discriminate.summary
        );
        assert_eq!(related.len(), 1);
    }

    #[test]
    fn aligned_variant_assertion_confirms_field_construction_error_construction() {
        let probe = probe(
            ProbeFamily::FieldConstruction,
            "outcome: Err(CalcError::TooLarge)",
        );
        let test = test_with_assertions(
            "too_large_outcome_asserted",
            vec![oracle(
                "assert_eq!(err, CalcError::TooLarge);",
                OracleKind::ExactErrorVariant,
                OracleStrength::Strong,
            )],
        );
        let (_observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(
            discriminate.state,
            StageState::Yes,
            "an oracle pinning the constructed variant must keep the field seam exposed"
        );
    }

    // End-to-end: the sibling fixture shape (RIPR-SPEC-0106) keeps the
    // return_value finding's discrimination unconfirmed instead of crediting
    // the sibling oracle as a strong discriminator.
    #[test]
    fn sibling_variant_oracle_leaves_return_value_discrimination_unconfirmed() {
        let probe = probe(ProbeFamily::ReturnValue, "return Err(CalcError::TooLarge);");
        let test = test_with_assertions(
            "negative_input_rejects_with_negative_error",
            vec![oracle(
                "assert_eq!(err, CalcError::Negative);",
                OracleKind::ExactErrorVariant,
                OracleStrength::Strong,
            )],
        );
        let (observe, discriminate, related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(observe.state, StageState::Yes);
        assert_eq!(discriminate.state, StageState::Weak);
        assert!(
            discriminate
                .summary
                .contains("Discriminator unconfirmed: no assertion text references this probe's changed expression (observation_unverified)"),
            "sibling oracle must not credit strong discrimination; got: {}",
            discriminate.summary
        );
        assert_eq!(related.len(), 1);
    }

    #[test]
    fn discriminate_evidence_names_strength_and_oracle_kind() {
        let cases = [
            (
                OracleStrength::Strong,
                OracleKind::ExactErrorVariant,
                ProbeFamily::ErrorPath,
                StageState::Yes,
                "Strong oracle found: exact error variant assertion",
            ),
            (
                OracleStrength::Strong,
                OracleKind::WholeObjectEquality,
                ProbeFamily::ReturnValue,
                StageState::Yes,
                "Strong oracle found: whole-object equality assertion",
            ),
            (
                OracleStrength::Strong,
                OracleKind::ExactValue,
                ProbeFamily::ReturnValue,
                StageState::Yes,
                "Strong oracle found: exact value or pattern assertion",
            ),
            (
                OracleStrength::Medium,
                OracleKind::Snapshot,
                ProbeFamily::ReturnValue,
                StageState::Weak,
                "Medium oracle found: snapshot assertion observes the changed behavior",
            ),
            (
                OracleStrength::Medium,
                OracleKind::MockExpectation,
                ProbeFamily::SideEffect,
                StageState::Weak,
                "Medium oracle found: mock or expectation observes the changed behavior",
            ),
            (
                OracleStrength::Medium,
                OracleKind::ExactValue,
                ProbeFamily::ReturnValue,
                StageState::Weak,
                "Medium oracle found: property or partial structural assertion",
            ),
            (
                OracleStrength::Weak,
                OracleKind::BroadError,
                ProbeFamily::ErrorPath,
                StageState::Weak,
                "Only broad error oracle found; is_err() does not discriminate exact error variants",
            ),
            (
                OracleStrength::Weak,
                OracleKind::BroadError,
                ProbeFamily::ReturnValue,
                StageState::Weak,
                "Only broad error oracle found; it may not discriminate the changed behavior exactly",
            ),
            (
                OracleStrength::Weak,
                OracleKind::RelationalCheck,
                ProbeFamily::Predicate,
                StageState::Weak,
                "Only relational oracle found; it may not discriminate the changed value exactly",
            ),
            (
                OracleStrength::Weak,
                OracleKind::ExactValue,
                ProbeFamily::ReturnValue,
                StageState::Weak,
                "Only weak oracle found, such as a broad relational assertion or non-empty check",
            ),
            (
                OracleStrength::Smoke,
                OracleKind::SmokeOnly,
                ProbeFamily::ReturnValue,
                StageState::Weak,
                "Only smoke oracle found, such as unwrap/expect or execution without a discriminator",
            ),
            (
                OracleStrength::None,
                OracleKind::Unknown,
                ProbeFamily::ReturnValue,
                StageState::No,
                "No assertion found on related tests",
            ),
            (
                OracleStrength::Unknown,
                OracleKind::Unknown,
                ProbeFamily::ReturnValue,
                StageState::Unknown,
                "Assertions exist, but oracle strength is unknown",
            ),
        ];

        for (strength, kind, family, state, summary) in cases {
            let evidence = build_discriminate_evidence(&strength, &kind, &family, false);
            assert_eq!(evidence.state, state);
            assert_eq!(evidence.summary, summary);
        }
    }

    #[test]
    fn oracle_family_matching_covers_family_specific_shapes() {
        assert!(oracle_matches_family(
            &ProbeFamily::ErrorPath,
            &oracle(
                "assert_matches!(result, Err(AuthError::RevokedToken));",
                OracleKind::ExactErrorVariant,
                OracleStrength::Strong,
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::ErrorPath,
            &oracle(
                "assert!(result.is_err());",
                OracleKind::BroadError,
                OracleStrength::Weak
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::ErrorPath,
            &oracle(
                "assert!(matches!(result, Err(_)));",
                OracleKind::Unknown,
                OracleStrength::Unknown
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::ErrorPath,
            &oracle(
                "assert_eq!(kind, Error::Denied);",
                OracleKind::Unknown,
                OracleStrength::Unknown
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::SideEffect,
            &oracle(
                "mock.expect_send();",
                OracleKind::MockExpectation,
                OracleStrength::Medium
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::SideEffect,
            &oracle(
                "assert!(event.saved);",
                OracleKind::Unknown,
                OracleStrength::Unknown
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::SideEffect,
            &oracle(
                "assert!(event.published);",
                OracleKind::Unknown,
                OracleStrength::Unknown
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::FieldConstruction,
            &oracle(
                "assert_eq!(item.id, 3);",
                OracleKind::Unknown,
                OracleStrength::Unknown
            )
        ));
        assert!(!oracle_matches_family(
            &ProbeFamily::FieldConstruction,
            &oracle(
                "assert!(3.14_f64 > 0.0_f64);",
                OracleKind::Unknown,
                OracleStrength::Unknown
            )
        ));
        // #2904: a dot inside a string literal is not a field access.
        assert!(!oracle_matches_family(
            &ProbeFamily::FieldConstruction,
            &oracle(
                "assert_eq!(msg, \"error.timeout\");",
                OracleKind::Unknown,
                OracleStrength::Unknown
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::FieldConstruction,
            &oracle(
                "assert_debug_snapshot!(item);",
                OracleKind::Snapshot,
                OracleStrength::Medium
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::Predicate,
            &oracle(
                "assert!(value >= 3);",
                OracleKind::RelationalCheck,
                OracleStrength::Weak
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::ReturnValue,
            &oracle(
                "assert_eq!(score(), 3);",
                OracleKind::ExactValue,
                OracleStrength::Strong
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::ReturnValue,
            &oracle(
                "score().unwrap();",
                OracleKind::SmokeOnly,
                OracleStrength::Smoke
            )
        ));
        assert!(!oracle_matches_family(
            &ProbeFamily::ReturnValue,
            &oracle(
                "assert_custom(score());",
                OracleKind::Unknown,
                OracleStrength::Unknown
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::CallDeletion,
            &oracle(
                "assert!(sent);",
                OracleKind::Unknown,
                OracleStrength::Unknown
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::CallDeletion,
            &oracle(
                "expect_send_called();",
                OracleKind::Unknown,
                OracleStrength::Unknown
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::CallDeletion,
            &oracle(
                "mock.expect_send();",
                OracleKind::MockExpectation,
                OracleStrength::Medium
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::MatchArm,
            &oracle(
                "assert_matches!(kind, Ready);",
                OracleKind::ExactErrorVariant,
                OracleStrength::Strong
            )
        ));
        assert!(oracle_matches_family(
            &ProbeFamily::MatchArm,
            &oracle(
                "assert_eq!(kind, Ready);",
                OracleKind::ExactValue,
                OracleStrength::Strong
            )
        ));
        assert!(!oracle_matches_family(
            &ProbeFamily::StaticUnknown,
            &oracle(
                "assert_eq!(value, 3);",
                OracleKind::ExactValue,
                OracleStrength::Strong
            )
        ));
    }

    #[test]
    fn probe_relative_oracle_strength_preserves_family_overrides() {
        let cases = [
            (
                ProbeFamily::ErrorPath,
                oracle("exact", OracleKind::ExactErrorVariant, OracleStrength::Weak),
                OracleStrength::Strong,
            ),
            (
                ProbeFamily::ErrorPath,
                oracle("broad", OracleKind::BroadError, OracleStrength::Weak),
                OracleStrength::Weak,
            ),
            (
                ProbeFamily::ErrorPath,
                oracle("smoke", OracleKind::SmokeOnly, OracleStrength::Strong),
                OracleStrength::Smoke,
            ),
            (
                ProbeFamily::ErrorPath,
                oracle("snapshot", OracleKind::Snapshot, OracleStrength::Medium),
                OracleStrength::Medium,
            ),
            (
                ProbeFamily::ReturnValue,
                oracle("exact", OracleKind::ExactValue, OracleStrength::Weak),
                OracleStrength::Strong,
            ),
            (
                ProbeFamily::Predicate,
                oracle("snapshot", OracleKind::Snapshot, OracleStrength::Medium),
                OracleStrength::Medium,
            ),
            (
                ProbeFamily::FieldConstruction,
                oracle("smoke", OracleKind::SmokeOnly, OracleStrength::Strong),
                OracleStrength::Smoke,
            ),
            (
                ProbeFamily::MatchArm,
                oracle("unknown", OracleKind::Unknown, OracleStrength::Strong),
                OracleStrength::Unknown,
            ),
            (
                ProbeFamily::SideEffect,
                oracle("mock", OracleKind::MockExpectation, OracleStrength::Medium),
                OracleStrength::Medium,
            ),
            (
                ProbeFamily::SideEffect,
                oracle(
                    "exact",
                    OracleKind::WholeObjectEquality,
                    OracleStrength::Weak,
                ),
                OracleStrength::Strong,
            ),
            (
                ProbeFamily::CallDeletion,
                oracle("rel", OracleKind::RelationalCheck, OracleStrength::Weak),
                OracleStrength::Weak,
            ),
            (
                ProbeFamily::CallDeletion,
                oracle("smoke", OracleKind::SmokeOnly, OracleStrength::Strong),
                OracleStrength::Smoke,
            ),
            (
                ProbeFamily::SideEffect,
                oracle(
                    "exact_error",
                    OracleKind::ExactErrorVariant,
                    OracleStrength::Strong,
                ),
                OracleStrength::Medium,
            ),
            (
                ProbeFamily::SideEffect,
                oracle("snapshot", OracleKind::Snapshot, OracleStrength::Weak),
                OracleStrength::Weak,
            ),
            (
                ProbeFamily::SideEffect,
                oracle("unknown", OracleKind::Unknown, OracleStrength::Strong),
                OracleStrength::Unknown,
            ),
            (
                ProbeFamily::StaticUnknown,
                oracle("exact", OracleKind::ExactValue, OracleStrength::Strong),
                OracleStrength::Unknown,
            ),
        ];

        for (family, assertion, expected) in cases {
            assert_eq!(
                probe_relative_oracle_strength(&family, &assertion),
                expected
            );
        }
    }

    fn owned_probe(family: ProbeFamily, expression: &str, owner: &str) -> Probe {
        Probe {
            owner: Some(SymbolId(format!("src/lib.rs::{owner}"))),
            ..probe(family, expression)
        }
    }

    fn probe(family: ProbeFamily, expression: &str) -> Probe {
        Probe {
            id: ProbeId("probe:test".to_string()),
            location: SourceLocation::new("src/lib.rs", 1, 1),
            owner: None,
            family,
            delta: DeltaKind::Value,
            before: None,
            after: None,
            expression: expression.to_string(),
            expected_sinks: Vec::new(),
            required_oracles: Vec::new(),
        }
    }

    fn test_with_assertions(name: &str, assertions: Vec<OracleFact>) -> TestSummary {
        TestSummary {
            name: name.to_string(),
            file: PathBuf::from("tests/value.rs"),
            start_line: 1,
            end_line: 3,
            body: "score();".to_string(),
            calls: Vec::new(),
            assertions,
            literals: Vec::new(),
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        }
    }

    /// #3700: a witness whose body binds the seam's callee (the shape the
    /// wrapper-establishment rule keys on), with no captured call facts — the
    /// lexical fallback in `body_contains_owner_call` must carry the binding.
    fn test_with_body_assertions(
        name: &str,
        body: &str,
        assertions: Vec<OracleFact>,
    ) -> TestSummary {
        TestSummary {
            body: body.to_string(),
            ..test_with_assertions(name, assertions)
        }
    }

    fn oracle(text: &str, kind: OracleKind, strength: OracleStrength) -> OracleFact {
        OracleFact {
            line: 2,
            text: text.to_string(),
            kind,
            strength,
            observed_tokens: extract_identifier_tokens(text),
            ok_value_observed: None,
        }
    }

    /// A guarded Result-match oracle fact with an explicit Ok-arm
    /// observation decision, mirroring what the extraction-side scanner
    /// threads into the fact (#3731 observation authority).
    fn guarded_oracle(text: &str, strength: OracleStrength, ok_value_observed: bool) -> OracleFact {
        OracleFact {
            line: 2,
            text: text.to_string(),
            kind: OracleKind::GuardedResultMatch,
            strength,
            observed_tokens: extract_identifier_tokens(text),
            ok_value_observed: Some(ok_value_observed),
        }
    }

    // --- #3709 producer-owned guarded Result match ---

    /// A guarded Result match whose scrutinee directly calls the probe's
    /// owner confirms observation even with zero changed-line token overlap.
    #[test]
    fn guarded_result_match_on_owner_confirms_observation_without_token_overlap() {
        let probe = owned_probe(
            ProbeFamily::ReturnValue,
            "if trimmed != Some(expected_id.trim()).as_str() {",
            "expect_response",
        );
        let test = test_with_assertions(
            "validates_ready_response",
            vec![guarded_oracle(
                "match expect_response(..) { Ok(..) => .., Err(..) => Some(ParseError::InvalidData { .. }) }",
                OracleStrength::Strong,
                true,
            )],
        );
        let (observe, discriminate, related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(observe.state, StageState::Yes);
        assert_eq!(discriminate.state, StageState::Yes, "{discriminate:?}");
        assert!(
            discriminate
                .summary
                .contains("guarded Result match over the changed owner's result"),
            "{discriminate:?}"
        );
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].oracle_kind, OracleKind::GuardedResultMatch);
    }

    /// The same oracle against a probe owned by a different function never
    /// confirms observation: the binding is same-entity by name.
    #[test]
    fn guarded_result_match_on_wrong_owner_stays_unverified() {
        let probe = owned_probe(
            ProbeFamily::ReturnValue,
            "if trimmed != Some(expected_id.trim()).as_str() {",
            "expect_response",
        );
        let test = test_with_assertions(
            "guards_a_different_helper",
            vec![guarded_oracle(
                "match other_helper(..) { Ok(..) => .., Err(..) => Some(ParseError::InvalidData { .. }) }",
                OracleStrength::Strong,
                true,
            )],
        );
        let (_, discriminate, related) =
            reveal_evidence(&probe, &[(&test, RelationReason::SameTestFile)]);

        assert_eq!(discriminate.state, StageState::Weak, "{discriminate:?}");
        assert!(
            discriminate.summary.contains("observation_unverified"),
            "wrong owner must not confirm: {discriminate:?}"
        );
        assert_eq!(related.len(), 1, "association is via single-assertion only");
    }

    /// A medium (type-pin) guarded match confirms observation but keeps the
    /// seam below exposed: the error type is pinned, the variant is not.
    #[test]
    fn guarded_result_match_type_pin_keeps_discriminate_weak() {
        let probe = owned_probe(
            ProbeFamily::ErrorPath,
            "return Err(Box::new(ParseError::InvalidData { .. }));",
            "expect_response",
        );
        let test = test_with_assertions(
            "checks_error_type",
            vec![guarded_oracle(
                "match expect_response(..) { Ok(..) => .., Err(..) => .downcast_ref::<ParseError> }",
                OracleStrength::Medium,
                true,
            )],
        );
        let (observe, discriminate, _) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(observe.state, StageState::Yes);
        assert_eq!(discriminate.state, StageState::Weak, "{discriminate:?}");
        assert!(
            !matches!(discriminate.state, StageState::Yes),
            "a type-only pin must not read exposed"
        );
    }

    /// Effect families never take the producer-owned path: a changed call or
    /// effect inside the owner need not flow through the matched result.
    #[test]
    fn guarded_result_match_does_not_credit_effect_families() {
        let probe = owned_probe(
            ProbeFamily::SideEffect,
            "audit_log.append(event);",
            "expect_response",
        );
        let test = test_with_assertions(
            "guards_result",
            vec![guarded_oracle(
                "match expect_response(..) { Ok(..) => .., Err(..) => Some(ParseError::InvalidData { .. }) }",
                OracleStrength::Strong,
                true,
            )],
        );
        let (_, discriminate, _) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_ne!(
            discriminate.state,
            StageState::Yes,
            "effect families keep their own observers: {discriminate:?}"
        );
    }

    /// #3731 review: a guarded match pinning a SIBLING variant does not
    /// confirm a probe whose changed expression constructs the exact
    /// variant — the shared enum-qualifier token is not a specificity
    /// signal, so the seam stays weakly exposed with an unverified
    /// observation.
    #[test]
    fn guarded_result_match_sibling_variant_does_not_confirm() {
        let probe = owned_probe(
            ProbeFamily::ErrorPath,
            "return Err(ParseError::InvalidData);",
            "expect_response",
        );
        let test = test_with_assertions(
            "pins_a_sibling_variant",
            vec![guarded_oracle(
                "match expect_response(..) { Ok(..) => .., Err(..) => ParseError::UnexpectedEof }",
                OracleStrength::Strong,
                true,
            )],
        );
        let (observe, discriminate, related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(observe.state, StageState::Yes, "the guard still observes");
        assert_eq!(
            discriminate.state,
            StageState::Weak,
            "a sibling-variant guard must not read exposed: {discriminate:?}"
        );
        assert!(
            discriminate.summary.contains("observation_unverified"),
            "the sibling variant leaves observation unverified: {discriminate:?}"
        );
        assert_eq!(
            related.len(),
            1,
            "association survives; confirmation does not"
        );
    }

    /// Positive control for the sibling gate: the SAME guarded match
    /// pinning the exact changed variant does confirm, through the
    /// variant-gated owner binding.
    #[test]
    fn guarded_result_match_exact_variant_on_owner_confirms() {
        let probe = owned_probe(
            ProbeFamily::ErrorPath,
            "return Err(ParseError::InvalidData);",
            "expect_response",
        );
        let test = test_with_assertions(
            "pins_the_exact_variant",
            vec![guarded_oracle(
                "match expect_response(..) { Ok(..) => .., Err(..) => ParseError::InvalidData }",
                OracleStrength::Strong,
                true,
            )],
        );
        let (_, discriminate, _) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(
            discriminate.state,
            StageState::Yes,
            "the exact-variant guard must confirm: {discriminate:?}"
        );
    }

    /// #3731 review: a qualified scrutinee sharing the owner's bare name
    /// (`other_crate::expect_response`) never confirms the local owner —
    /// reveal cannot resolve the qualified path's identity, so the
    /// observation stays unverified.
    #[test]
    fn guarded_result_match_on_qualified_scrutinee_stays_unverified() {
        let probe = owned_probe(
            ProbeFamily::ReturnValue,
            "if trimmed != Some(expected_id.trim()).as_str() {",
            "expect_response",
        );
        let test = test_with_assertions(
            "guards_a_qualified_same_named_callee",
            vec![guarded_oracle(
                "match other_crate::expect_response(..) { Ok(..) => .., Err(..) => Some(ParseError::InvalidData { .. }) }",
                OracleStrength::Strong,
                true,
            )],
        );
        let (_, discriminate, _) =
            reveal_evidence(&probe, &[(&test, RelationReason::SameTestFile)]);

        assert_eq!(
            discriminate.state,
            StageState::Weak,
            "a qualified scrutinee must not bind the owner: {discriminate:?}"
        );
        assert!(
            discriminate.summary.contains("observation_unverified"),
            "the qualified-path observation stays unverified: {discriminate:?}"
        );
    }

    // --- #3731 observation authority (RIPR-SPEC-0175): Ok-arm observation ---

    /// A return-value probe whose changed value is the SUCCESS payload is
    /// not confirmed by a guarded-routing match with no Ok arm: the success
    /// value flows into a trivial catch-all, so the test never observes a
    /// change to it and the fact reports `ok_value_observed: Some(false)`.
    #[test]
    fn return_value_probe_routing_form_without_ok_arm_stays_unconfirmed() {
        let probe = owned_probe(
            ProbeFamily::ReturnValue,
            "Ok(build_response(expected_id, trimmed.trim()))",
            "expect_response",
        );
        let test = test_with_assertions(
            "routes_the_result",
            vec![guarded_oracle(
                "match expect_response(..) { Err(..) => ParseError::InvalidData, _ => .. }",
                OracleStrength::Strong,
                false,
            )],
        );
        let (_, discriminate, related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(
            discriminate.state,
            StageState::Weak,
            "a routing form never observes the Ok payload: {discriminate:?}"
        );
        assert!(
            discriminate.summary.contains("observation_unverified"),
            "the unobserved success value stays unverified: {discriminate:?}"
        );
        assert_eq!(
            related.len(),
            1,
            "association survives; confirmation does not"
        );
    }

    /// A payload-ignoring Ok arm (`Ok(_) => {}`) does not confirm a
    /// success-payload return-value probe either: the synthesized text
    /// keeps its `Ok(..) => ..` template (output-contract stability), so
    /// the unobserving decision rides the fact's `ok_value_observed`.
    #[test]
    fn return_value_probe_payload_ignoring_ok_arm_stays_unconfirmed() {
        let probe = owned_probe(
            ProbeFamily::ReturnValue,
            "Ok(build_response(expected_id, trimmed.trim()))",
            "expect_response",
        );
        let test = test_with_assertions(
            "ignores_the_payload",
            vec![guarded_oracle(
                "match expect_response(..) { Ok(..) => .., Err(..) => ParseError::InvalidData }",
                OracleStrength::Strong,
                false,
            )],
        );
        let (_, discriminate, _) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(
            discriminate.state,
            StageState::Weak,
            "an Ok arm that ignores the payload never discriminates it: {discriminate:?}"
        );
        assert!(
            discriminate.summary.contains("observation_unverified"),
            "the ignored payload stays unverified: {discriminate:?}"
        );
    }

    /// Positive control: the same probe against an OBSERVING Ok arm
    /// (`Ok(v) => assert_eq!(v, 3)`) confirms through the producer-owned
    /// binding — the fact reports `ok_value_observed: Some(true)`.
    #[test]
    fn return_value_probe_observing_ok_arm_confirms() {
        let probe = owned_probe(
            ProbeFamily::ReturnValue,
            "Ok(build_response(expected_id, trimmed.trim()))",
            "expect_response",
        );
        let test = test_with_assertions(
            "asserts_the_payload",
            vec![guarded_oracle(
                "match expect_response(..) { Ok(..) => .., Err(..) => ParseError::InvalidData }",
                OracleStrength::Strong,
                true,
            )],
        );
        let (_, discriminate, _) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(
            discriminate.state,
            StageState::Yes,
            "an observing Ok arm discriminates the success payload: {discriminate:?}"
        );
    }

    /// ErrorPath probes keep the existing behavior: the Err guard is the
    /// discriminator there, so a non-observing Ok arm never blocks the
    /// variant-gated confirmation.
    #[test]
    fn error_path_probe_is_independent_of_ok_arm_observation() {
        let probe = owned_probe(
            ProbeFamily::ErrorPath,
            "return Err(ParseError::InvalidData);",
            "expect_response",
        );
        let test = test_with_assertions(
            "pins_the_error_variant",
            vec![guarded_oracle(
                "match expect_response(..) { Ok(..) => .., Err(..) => ParseError::InvalidData }",
                OracleStrength::Strong,
                false,
            )],
        );
        let (_, discriminate, _) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(
            discriminate.state,
            StageState::Yes,
            "the Err guard discriminates regardless of the Ok arm: {discriminate:?}"
        );
    }

    /// A return-value probe on an exact Err CONSTRUCTION also keeps the
    /// Err-guard discriminator (the pin names the changed variant): the
    /// Ok-arm observation requirement applies only to success-payload
    /// probes — the shape the guarded-routing positive fixture pins.
    #[test]
    fn return_value_probe_on_err_construction_is_independent_of_ok_arm_observation() {
        let probe = owned_probe(
            ProbeFamily::ReturnValue,
            "return Err(ParseError::InvalidData);",
            "expect_response",
        );
        let test = test_with_assertions(
            "routes_the_error",
            vec![guarded_oracle(
                "match expect_response(..) { Err(..) => ParseError::InvalidData, _ => .. }",
                OracleStrength::Strong,
                false,
            )],
        );
        let (_, discriminate, _) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(
            discriminate.state,
            StageState::Yes,
            "the exact-variant pin discriminates the Err construction: {discriminate:?}"
        );
    }

    // --- #3731 review round 4 (F11): foreign same-name import defeat ---

    /// The related test's file importing the owner callee's bare name from
    /// a FOREIGN path (`use other_crate::expect_response;`) makes the bare
    /// scrutinee binding ambiguous — the owner confirmation is refused and
    /// the observation stays unverified.
    #[test]
    fn foreign_same_name_import_defeats_owner_confirmation() {
        let probe = owned_probe(
            ProbeFamily::ReturnValue,
            "if trimmed != Some(expected_id.trim()).as_str() {",
            "expect_response",
        );
        let test = test_with_assertions(
            "guards_an_imported_same_named_callee",
            vec![guarded_oracle(
                "match expect_response(..) { Ok(..) => .., Err(..) => Some(ParseError::InvalidData { .. }) }",
                OracleStrength::Strong,
                true,
            )],
        );
        let test_source = "use other_crate::expect_response;\n";
        let crate_names = std::collections::BTreeSet::new();
        let (_, discriminate, _) = reveal_evidence_with_expression(
            &probe,
            &probe.expression,
            &[(&test, RelationReason::DirectOwnerCall)],
            &|_test, callee| file_imports_foreign_callee_name(test_source, callee, &crate_names),
            &|_, _| false,
        );

        assert_eq!(
            discriminate.state,
            StageState::Weak,
            "a foreign same-name import must defeat the owner binding: {discriminate:?}"
        );
        assert!(
            discriminate.summary.contains("observation_unverified"),
            "the ambiguous binding leaves observation unverified: {discriminate:?}"
        );
    }

    /// Positive control: the same harness WITHOUT the import confirms —
    /// and an OWN-CRATE import (the normal integration-test binding) does
    /// not defeat, because it binds the owner's own export.
    #[test]
    fn no_import_or_own_crate_import_keeps_owner_confirmation() {
        let probe = owned_probe(
            ProbeFamily::ReturnValue,
            "if trimmed != Some(expected_id.trim()).as_str() {",
            "expect_response",
        );
        let test = test_with_assertions(
            "guards_the_owner_directly",
            vec![guarded_oracle(
                "match expect_response(..) { Ok(..) => .., Err(..) => Some(ParseError::InvalidData { .. }) }",
                OracleStrength::Strong,
                true,
            )],
        );
        let without_import = "";
        let own_crate_import = "use guarded_result_match::{ParseError, expect_response};\n";
        let own_crate_names: std::collections::BTreeSet<String> = ["guarded_result_match"]
            .into_iter()
            .map(str::to_string)
            .collect();

        let (_, discriminate, _) = reveal_evidence_with_expression(
            &probe,
            &probe.expression,
            &[(&test, RelationReason::DirectOwnerCall)],
            &|_, callee| file_imports_foreign_callee_name(without_import, callee, &own_crate_names),
            &|_, _| false,
        );
        assert_eq!(
            discriminate.state,
            StageState::Yes,
            "no import means no ambiguity: {discriminate:?}"
        );

        let (_, own_crate, _) = reveal_evidence_with_expression(
            &probe,
            &probe.expression,
            &[(&test, RelationReason::DirectOwnerCall)],
            &|_, callee| {
                file_imports_foreign_callee_name(own_crate_import, callee, &own_crate_names)
            },
            &|_, _| false,
        );
        assert_eq!(
            own_crate.state,
            StageState::Yes,
            "an own-crate import binds the owner's own export and must not defeat: {own_crate:?}"
        );
    }

    /// An aliased foreign import (`use other_crate::expect_response as
    /// respond;`) binds the ALIAS, not the bare name, so the bare
    /// scrutinee is unambiguous and the confirmation stands.
    #[test]
    fn aliased_foreign_import_does_not_defeat_owner_confirmation() {
        let probe = owned_probe(
            ProbeFamily::ReturnValue,
            "if trimmed != Some(expected_id.trim()).as_str() {",
            "expect_response",
        );
        let test = test_with_assertions(
            "guards_the_owner_with_an_unrelated_alias_import",
            vec![guarded_oracle(
                "match expect_response(..) { Ok(..) => .., Err(..) => Some(ParseError::InvalidData { .. }) }",
                OracleStrength::Strong,
                true,
            )],
        );
        let aliased_import = "use other_crate::expect_response as respond;\n";
        let crate_names = std::collections::BTreeSet::new();
        let (_, discriminate, _) = reveal_evidence_with_expression(
            &probe,
            &probe.expression,
            &[(&test, RelationReason::DirectOwnerCall)],
            &|_, callee| file_imports_foreign_callee_name(aliased_import, callee, &crate_names),
            &|_, _| false,
        );
        assert_eq!(
            discriminate.state,
            StageState::Yes,
            "an aliased import binds the alias, not the bare name: {discriminate:?}"
        );
    }

    /// F22 (#3731 review): a foreign import NESTED inside a test module —
    /// the historical `mod tests { use other_crate::expect_response; .. }`
    /// harness shape — defeats the owner confirmation too. Pre-fix the
    /// scan read file-level `use` statements only, so the nested import
    /// bypassed the defeat.
    #[test]
    fn nested_module_foreign_import_defeats_owner_confirmation() {
        let probe = owned_probe(
            ProbeFamily::ReturnValue,
            "if trimmed != Some(expected_id.trim()).as_str() {",
            "expect_response",
        );
        let test = test_with_assertions(
            "guards_an_imported_same_named_callee_from_a_test_module",
            vec![guarded_oracle(
                "match expect_response(..) { Ok(..) => .., Err(..) => Some(ParseError::InvalidData { .. }) }",
                OracleStrength::Strong,
                true,
            )],
        );
        let test_source = "mod tests {\n    use other_crate::expect_response;\n\n    #[test]\n    fn guards_the_result() {}\n}\n";
        let crate_names = std::collections::BTreeSet::new();
        let (_, discriminate, _) = reveal_evidence_with_expression(
            &probe,
            &probe.expression,
            &[(&test, RelationReason::DirectOwnerCall)],
            &|_test, callee| file_imports_foreign_callee_name(test_source, callee, &crate_names),
            &|_, _| false,
        );

        assert_eq!(
            discriminate.state,
            StageState::Weak,
            "a module-nested foreign same-name import must defeat the owner binding: {discriminate:?}"
        );
        assert!(
            discriminate.summary.contains("observation_unverified"),
            "the ambiguous binding leaves observation unverified: {discriminate:?}"
        );
    }

    /// F22 direct-scan controls on the same function: a nested import is
    /// found at any depth, a function-local import counts, and text
    /// without the keyword does not.
    #[test]
    fn foreign_callee_import_scan_covers_nested_and_local_use_declarations() {
        let crate_names = std::collections::BTreeSet::new();
        let nested = "mod tests {\n    use other_crate::expect_response;\n}\n";
        assert!(
            file_imports_foreign_callee_name(nested, "expect_response", &crate_names),
            "a module-nested foreign import must defeat"
        );
        let brace_list = "mod tests {\n    use other_crate::{setup, expect_response};\n}\n";
        assert!(
            file_imports_foreign_callee_name(brace_list, "expect_response", &crate_names),
            "a module-nested brace-list import must defeat"
        );
        let function_local = "fn t() {\n    use other_crate::expect_response;\n}\n";
        assert!(
            file_imports_foreign_callee_name(function_local, "expect_response", &crate_names),
            "a function-local foreign import must defeat"
        );
        let own_path = "mod tests {\n    use crate::expect_response;\n}\n";
        assert!(
            !file_imports_foreign_callee_name(own_path, "expect_response", &crate_names),
            "an own-crate nested import binds the owner and must not defeat"
        );
    }

    /// F23 (#3731 review): the analyzed crate's own names include the
    /// `[lib]` target name, and hyphenated package names normalize to
    /// underscores in crate identifiers — an import through the
    /// underscore form of a hyphenated package name binds the owner's own
    /// export and must NOT defeat, while a foreign first segment still
    /// does.
    #[test]
    fn own_lib_target_and_hyphen_normalized_names_do_not_defeat() {
        // Package `foo-bar` (hyphenated) whose lib target is `foo_bar`:
        // the integration-test binding `use foo_bar::expect_response;` is
        // the owner's own export on both spellings.
        let hyphenated_and_lib_names: std::collections::BTreeSet<String> = ["foo-bar", "foo_bar"]
            .into_iter()
            .map(str::to_string)
            .collect();
        let import = "use foo_bar::expect_response;\n";
        assert!(
            !file_imports_foreign_callee_name(import, "expect_response", &hyphenated_and_lib_names),
            "an import through the crate's own lib-target name must not defeat"
        );
        // The normalization direction too: a raw hyphenated manifest name
        // admits its underscore crate identifier.
        let hyphenated_only: std::collections::BTreeSet<String> =
            ["foo-bar"].into_iter().map(str::to_string).collect();
        assert!(
            !file_imports_foreign_callee_name(import, "expect_response", &hyphenated_only),
            "a hyphenated own package name must admit its underscore identifier"
        );
        // A foreign first segment still defeats.
        let foreign_names: std::collections::BTreeSet<String> = ["unrelated_crate"]
            .into_iter()
            .map(str::to_string)
            .collect();
        assert!(
            file_imports_foreign_callee_name(import, "expect_response", &foreign_names),
            "a foreign first segment must still defeat"
        );
    }

    /// #3731 review (G1): the cross-package same-name defeat threads
    /// through the same per-test path as the import defeat — when the
    /// test's own package defines the callee's name while the changed
    /// owner lives in another package, the bare binding is ambiguous and
    /// the confirmation is refused; without the defeat it stands.
    #[test]
    fn cross_package_same_name_defeat_blocks_owner_confirmation() {
        let probe = owned_probe(
            ProbeFamily::ReturnValue,
            "if trimmed != Some(expected_id.trim()).as_str() {",
            "expect_response",
        );
        let test = test_with_assertions(
            "guards_a_same_named_local_function",
            vec![guarded_oracle(
                "match expect_response(..) { Ok(..) => .., Err(..) => Some(ParseError::InvalidData { .. }) }",
                OracleStrength::Strong,
                true,
            )],
        );
        let (_, defeated, _) = reveal_evidence_with_expression(
            &probe,
            &probe.expression,
            &[(&test, RelationReason::DirectOwnerCall)],
            &|_, _| false,
            &|_, _| true,
        );
        assert_eq!(
            defeated.state,
            StageState::Weak,
            "a same-named function in the test's own package must defeat the \
             bare binding: {defeated:?}"
        );
        assert!(
            defeated.summary.contains("observation_unverified"),
            "the ambiguous binding leaves observation unverified: {defeated:?}"
        );

        let (_, confirmed, _) = reveal_evidence_with_expression(
            &probe,
            &probe.expression,
            &[(&test, RelationReason::DirectOwnerCall)],
            &|_, _| false,
            &|_, _| false,
        );
        assert_eq!(
            confirmed.state,
            StageState::Yes,
            "without a same-named local definition the confirmation stands: {confirmed:?}"
        );
    }

    // --- RIPR-SPEC-0093 arm-blind downgrade ---

    /// A MatchArm probe whose expression has no extractable tokens (e.g. `None`
    /// is filtered) and whose single related test has an ExactValue oracle for a
    /// DIFFERENT arm must emit weakly_exposed with observation_unverified.
    #[test]
    fn match_arm_probe_without_token_match_downgrades_discriminate_to_weak() {
        // probe expression "None => 0," — None is filtered, 0 is not alpha
        let probe = probe(ProbeFamily::MatchArm, "None => 0,");
        let test = test_with_assertions(
            "some_arm_returns_incremented_value",
            vec![oracle(
                "assert_eq!(reason(Some(5)), 6);",
                OracleKind::ExactValue,
                OracleStrength::Strong,
            )],
        );
        let (observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(observe.state, StageState::Yes, "observe must still fire");
        assert_eq!(
            discriminate.state,
            StageState::Weak,
            "discriminate must be downgraded to Weak (observation_unverified)"
        );
        assert!(
            discriminate.summary.contains("observation_unverified"),
            "summary must name the reason: got `{}`",
            discriminate.summary
        );
    }

    /// A MatchArm probe whose expression DOES have a token that appears in the
    /// assertion text (token_match) must stay exposed (StageState::Yes).
    #[test]
    fn match_arm_probe_with_token_match_keeps_discriminate_yes() {
        // probe expression "Status::Idle => 0," — Idle and Status are extractable
        let probe = probe(ProbeFamily::MatchArm, "Status::Idle => 0,");
        let test = test_with_assertions(
            "idle_arm_returns_zero",
            vec![oracle(
                "assert_eq!(classify(Status::Idle), 0);",
                OracleKind::ExactValue,
                OracleStrength::Strong,
            )],
        );
        let (observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(observe.state, StageState::Yes);
        assert_eq!(
            discriminate.state,
            StageState::Yes,
            "token_match on Idle must keep discriminate Yes (no over-correction)"
        );
    }

    /// A ReturnValue probe whose single related test has a family-matching assertion
    /// but NO token referencing the changed expression must downgrade to Weak.
    /// This inverts the former bug-locking test
    /// `non_match_arm_probe_family_match_only_keeps_discriminate_yes`.
    #[test]
    fn return_value_family_match_only_without_token_downgrades_discriminate_to_weak() {
        // "value + 1" — tokens: ["value"] (len 5). Assertion "assert_eq!(compute(), 42);"
        // has no occurrence of "value", so has_token_match=false.
        let probe = probe(ProbeFamily::ReturnValue, "value + 1");
        let test = test_with_assertions(
            "returns_incremented",
            vec![oracle(
                "assert_eq!(compute(), 42);",
                OracleKind::ExactValue,
                OracleStrength::Strong,
            )],
        );
        let (_observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(
            discriminate.state,
            StageState::Weak,
            "ReturnValue probe with no token_match must downgrade to Weak (observation_unverified)"
        );
        assert!(
            discriminate.summary.contains("observation_unverified"),
            "summary must name the reason: got `{}`",
            discriminate.summary
        );
    }

    /// A ReturnValue probe whose assertion text CONTAINS a token from the probe
    /// expression must stay exposed (StageState::Yes) — no over-correction.
    #[test]
    fn return_value_with_token_match_keeps_discriminate_yes() {
        // "score + 1" — tokens: ["score"] (len 5). Assertion contains "score".
        let probe = probe(ProbeFamily::ReturnValue, "score + 1");
        let test = test_with_assertions(
            "score_incremented",
            vec![oracle(
                "assert_eq!(score(), 6);",
                OracleKind::ExactValue,
                OracleStrength::Strong,
            )],
        );
        let (_observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(
            discriminate.state,
            StageState::Yes,
            "ReturnValue probe with token_match must stay Yes (no over-correction)"
        );
    }

    /// A FieldConstruction probe whose single assertion has a dot (family_match)
    /// but no token referencing the specific changed field must downgrade to Weak.
    #[test]
    fn field_construction_family_match_only_without_token_downgrades_to_weak() {
        // "priority: 3" — tokens: ["priority"] (len 8). Assertion "assert_eq!(item.id, 3);"
        // does not contain "priority".
        let probe = probe(ProbeFamily::FieldConstruction, "priority: 3");
        let test = test_with_assertions(
            "item_has_id",
            vec![oracle(
                "assert_eq!(item.id, 3);",
                OracleKind::ExactValue,
                OracleStrength::Strong,
            )],
        );
        let (_observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(
            discriminate.state,
            StageState::Weak,
            "FieldConstruction probe with no token_match must downgrade to Weak"
        );
    }

    /// A FieldConstruction probe whose assertion references the exact changed field
    /// must stay exposed (StageState::Yes).
    #[test]
    fn field_construction_with_token_match_keeps_discriminate_yes() {
        // "priority: 3" — tokens: ["priority"]. Assertion "assert_eq!(item.priority, 3);"
        // contains "priority".
        let probe = probe(ProbeFamily::FieldConstruction, "priority: 3");
        let test = test_with_assertions(
            "item_has_priority",
            vec![oracle(
                "assert_eq!(item.priority, 3);",
                OracleKind::ExactValue,
                OracleStrength::Strong,
            )],
        );
        let (_observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(
            discriminate.state,
            StageState::Yes,
            "FieldConstruction probe with token_match on priority must stay Yes"
        );
    }

    /// A SideEffect probe whose single PLAIN assertion has neither a token
    /// referencing the changed effect NOR an effect-observer kind (no mock,
    /// snapshot, or whole-object) must emit observation_unverified. This is the
    /// genuinely-blind case: the assertion only fired via the single-assertion
    /// escape hatch.
    #[test]
    fn side_effect_plain_assertion_without_token_or_observer_emits_observation_unverified() {
        // "send_notification(user_id)" — tokens: ["send", "notification", "user"].
        // Assertion "assert!(ran);" contains none of those, and is not a mock,
        // snapshot, or whole-object observer.
        let probe = probe(ProbeFamily::SideEffect, "send_notification(user_id)");
        let test = test_with_assertions(
            "ran",
            vec![oracle(
                "assert!(ran);",
                OracleKind::Unknown,
                OracleStrength::Unknown,
            )],
        );
        let (_observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(
            discriminate.state,
            StageState::Weak,
            "SideEffect probe with no token and no effect observer must downgrade to Weak"
        );
        assert!(
            discriminate.summary.contains("observation_unverified"),
            "plain non-observing assertion must emit observation_unverified: got `{}`",
            discriminate.summary
        );
    }

    /// REGRESSION LOCK (#1216 second-pass): a SideEffect probe whose single
    /// matched assertion is a genuine MOCK EXPECTATION that kind-matches the seam
    /// but shares NO token with the probe expression must NOT emit
    /// observation_unverified. The mock observes the effect; downgrading it to
    /// observation_unverified would be a false weakening. (It may still be Weak
    /// via the Medium-strength path, but never via observation_unverified.)
    #[test]
    fn side_effect_mock_observer_without_token_clears_observation_unverified() {
        // "send_notification(user_id)" — tokens: ["send", "notification", "user"].
        // Assertion "mock.verify();" shares no token but is a MockExpectation,
        // i.e. a genuine effect observer.
        let probe = probe(ProbeFamily::SideEffect, "send_notification(user_id)");
        let test = test_with_assertions(
            "notification_checked",
            vec![oracle(
                "mock.verify();",
                OracleKind::MockExpectation,
                OracleStrength::Medium,
            )],
        );
        let (_observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert!(
            !discriminate.summary.contains("observation_unverified"),
            "a genuine mock observer must clear observation_unverified even without a token: got `{}`",
            discriminate.summary
        );
    }

    /// REGRESSION LOCK (#1216 second-pass): a CallDeletion probe whose single
    /// matched assertion is a whole-object equality (a genuine effect observer)
    /// sharing no token must NOT emit observation_unverified. Whole-object
    /// equality captures the resulting persisted state, so it observes the
    /// effect even without naming the changed call token.
    #[test]
    fn call_deletion_whole_object_observer_without_token_clears_observation_unverified() {
        // "persist_audit(record)" — tokens: ["persist", "audit", "record"].
        // Assertion "assert_eq!(store, expected);" shares no token but is a
        // WholeObjectEquality effect observer (Strong).
        let probe = probe(ProbeFamily::CallDeletion, "persist_audit(record)");
        let test = test_with_assertions(
            "store_matches_expected",
            vec![oracle(
                "assert_eq!(store, expected);",
                OracleKind::WholeObjectEquality,
                OracleStrength::Strong,
            )],
        );
        let (_observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert!(
            !discriminate.summary.contains("observation_unverified"),
            "a whole-object effect observer must clear observation_unverified even without a token: got `{}`",
            discriminate.summary
        );
    }

    /// A VALUE family (ReturnValue) must NOT treat a mock/whole-object as an
    /// observation confirmation — only a token_match confirms value families.
    /// This guards against the effect-family relaxation leaking into value
    /// families (the point of #1200/#1216: an ExactValue/whole-object oracle
    /// does not kind-match a value seam's specific sub-expression).
    #[test]
    fn return_value_whole_object_without_token_still_emits_observation_unverified() {
        // "base * SCALE" — tokens: ["base", "SCALE"]. Assertion
        // "assert_eq!(result, expected);" is WholeObjectEquality but shares no
        // token; for a VALUE family this must NOT clear observation_unverified.
        let probe = probe(ProbeFamily::ReturnValue, "base * SCALE");
        let test = test_with_assertions(
            "result_matches_expected",
            vec![oracle(
                "assert_eq!(result, expected);",
                OracleKind::WholeObjectEquality,
                OracleStrength::Strong,
            )],
        );
        let (_observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(
            discriminate.state,
            StageState::Weak,
            "ReturnValue (value family) with no token must stay observation_unverified"
        );
        assert!(
            discriminate.summary.contains("observation_unverified"),
            "value family must not be cleared by an effect-observer kind: got `{}`",
            discriminate.summary
        );
    }

    /// A SideEffect probe whose assertion references a specific token from the
    /// changed expression must not be downgraded by observation_unverified.
    #[test]
    fn side_effect_with_token_match_keeps_discriminate_not_unverified() {
        // "emit_payment_event(tx)" — tokens: ["emit_payment_event", "tx"] but
        // only "emit_payment_event" (len > 3, well, len 17) would match.
        // Assertion "mock.expect_emit_payment_event();" contains "emit_payment_event".
        let probe = probe(ProbeFamily::SideEffect, "emit_payment_event(tx)");
        let test = test_with_assertions(
            "payment_event_emitted",
            vec![oracle(
                "mock.expect_emit_payment_event();",
                OracleKind::MockExpectation,
                OracleStrength::Medium,
            )],
        );
        let (_observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        // MockExpectation yields OracleStrength::Medium → StageState::Weak via
        // the existing Medium oracle path, NOT via observation_unverified.
        // The key invariant: observation_unverified must NOT fire when there IS
        // a token_match — so the summary must not contain "observation_unverified".
        assert!(
            !discriminate.summary.contains("observation_unverified"),
            "token-matched SideEffect must not emit observation_unverified: got `{}`",
            discriminate.summary
        );
    }

    /// A CallDeletion probe whose single assertion fires family_match via
    /// `text.contains("assert")` but contains no token from the changed call
    /// expression must downgrade to Weak.
    #[test]
    fn call_deletion_family_match_only_without_token_downgrades_to_weak() {
        // "log_audit_event(record)" — tokens: ["audit", "event", "record"] (all len > 3).
        // Assertion "assert!(result.is_ok());" does not contain any of those.
        let probe = probe(ProbeFamily::CallDeletion, "log_audit_event(record)");
        let test = test_with_assertions(
            "result_ok",
            vec![oracle(
                "assert!(result.is_ok());",
                OracleKind::BroadError,
                OracleStrength::Weak,
            )],
        );
        let (_observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(
            discriminate.state,
            StageState::Weak,
            "CallDeletion probe with no token_match must downgrade to Weak"
        );
    }

    /// A CallDeletion probe whose assertion text contains the full call token
    /// must not be downgraded by observation_unverified.
    #[test]
    fn call_deletion_with_token_match_does_not_emit_observation_unverified() {
        // "log_audit_event(record)" — tokens: ["log_audit_event", "record"].
        // Assertion "assert!(log_audit_event());" contains
        // "log_audit_event" as a whole-word match (#2397: the word-boundary
        // check correctly distinguishes `log_audit_event` from a different
        // identifier like `log_audit_event_was_called`).
        let probe = probe(ProbeFamily::CallDeletion, "log_audit_event(record)");
        let test = test_with_assertions(
            "audit_event_logged",
            vec![oracle(
                "assert!(log_audit_event());",
                OracleKind::ExactValue,
                OracleStrength::Strong,
            )],
        );
        let (_observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert!(
            !discriminate.summary.contains("observation_unverified"),
            "token-matched CallDeletion must not emit observation_unverified: got `{}`",
            discriminate.summary
        );
    }

    /// MatchArm: type-blind token match — `Mode::Warm` assertion must NOT clear
    /// observation_unverified for a `Mode::Frozen` probe (Part B regression lock).
    #[test]
    fn match_arm_sibling_qualifier_does_not_clear_observation_unverified() {
        // probe expression "Mode::Frozen => -1," — tokens: ["Mode", "Frozen"].
        // Assertion "assert_eq!(classify(Mode::Warm), 1);" contains "Mode" (len 4)
        // but NOT "Frozen" (the variant token). With variant-scoped token_match,
        // "Mode" alone must not clear observation_unverified.
        let probe = probe(ProbeFamily::MatchArm, "Mode::Frozen => -1,");
        let test = test_with_assertions(
            "warm_arm_returns_one",
            vec![oracle(
                "assert_eq!(classify(Mode::Warm), 1);",
                OracleKind::ExactValue,
                OracleStrength::Strong,
            )],
        );
        let (_observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(
            discriminate.state,
            StageState::Weak,
            "Mode::Warm assertion must not confirm Mode::Frozen arm (sibling-qualifier hole)"
        );
        assert!(
            discriminate.summary.contains("observation_unverified"),
            "summary must name the reason: got `{}`",
            discriminate.summary
        );
    }

    /// MatchArm: assertion containing the specific VARIANT token confirms the arm.
    #[test]
    fn match_arm_variant_token_match_keeps_discriminate_yes() {
        // probe expression "Mode::Frozen => -1," — variant "Frozen" appears in assertion.
        let probe = probe(ProbeFamily::MatchArm, "Mode::Frozen => -1,");
        let test = test_with_assertions(
            "frozen_arm_returns_minus_one",
            vec![oracle(
                "assert_eq!(classify(Mode::Frozen), -1);",
                OracleKind::ExactValue,
                OracleStrength::Strong,
            )],
        );
        let (_observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(
            discriminate.state,
            StageState::Yes,
            "Frozen variant in assertion must confirm observation (no over-correction)"
        );
    }

    // --- Predicate must NOT be affected; ErrorPath now requires token/variant confirmation ---

    /// Predicate probes do not require token confirmation and must not be
    /// affected by the observation_unverified logic.
    #[test]
    fn predicate_probe_family_match_only_keeps_discriminate_yes() {
        let probe = probe(ProbeFamily::Predicate, "x > 0");
        let test = test_with_assertions(
            "check_positive",
            vec![oracle(
                "assert!(value >= 3);",
                OracleKind::RelationalCheck,
                OracleStrength::Weak,
            )],
        );
        let (_observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        // RelationalCheck → OracleStrength::Weak → StageState::Weak, but NOT
        // via observation_unverified.
        assert!(
            !discriminate.summary.contains("observation_unverified"),
            "Predicate probe must not emit observation_unverified: got `{}`",
            discriminate.summary
        );
    }

    // --- RIPR-SPEC-0107: ErrorPath now requires variant/token confirmation ---

    /// RIPR-SPEC-0107 Control A (REPRO): an ErrorPath probe with ONLY a broad
    /// `is_err()` oracle and a sibling `ExactValue` result (no variant-pinning
    /// oracle) must downgrade to `weakly_exposed` via `observation_unverified`.
    /// This is the fake-clean being fixed: the sibling oracle cannot confirm the
    /// changed error variant is specifically observed.
    #[test]
    fn error_path_broad_oracle_only_downgrades_discriminate_to_weak() {
        let probe = probe(ProbeFamily::ErrorPath, "Err(AuthError::RevokedToken)");
        let test = test_with_assertions(
            "revoked_token_fails",
            vec![oracle(
                "assert!(result.is_err());",
                OracleKind::BroadError,
                OracleStrength::Weak,
            )],
        );
        let (_observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(
            discriminate.state,
            StageState::Weak,
            "ErrorPath probe with only a broad is_err() oracle must downgrade to Weak (observation_unverified)"
        );
        assert!(
            discriminate.summary.contains("observation_unverified"),
            "broad is_err() must emit observation_unverified for ErrorPath: got `{}`",
            discriminate.summary
        );
    }

    /// RIPR-SPEC-0107 Control A continued: an ErrorPath probe with a sibling
    /// `ExactValue` oracle (no variant token in assertion) must also downgrade.
    /// An `assert_eq!(validate_or_default(""), "guest")` oracle credits the
    /// happy-path return value, not the error variant — it must NOT promote
    /// the error_path seam to `exposed`.
    #[test]
    fn error_path_sibling_exact_value_oracle_downgrades_discriminate_to_weak() {
        let probe = probe(ProbeFamily::ErrorPath, "Err(ParseError::TooLong(len))");
        let test = test_with_assertions(
            "default_value_returned",
            vec![oracle(
                "assert_eq!(validate_or_default(\"\"), \"guest\");",
                OracleKind::ExactValue,
                OracleStrength::Strong,
            )],
        );
        let (_observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(
            discriminate.state,
            StageState::Weak,
            "ErrorPath probe with a sibling ExactValue oracle (no variant token) must downgrade to Weak"
        );
        assert!(
            discriminate.summary.contains("observation_unverified"),
            "sibling ExactValue oracle must emit observation_unverified for ErrorPath: got `{}`",
            discriminate.summary
        );
    }

    /// RIPR-SPEC-0107 Control B (MUST-NOT-OVER-CORRECT): an ErrorPath probe
    /// backed by a real variant-pinning oracle (`assert_eq!(err, ParseError::TooLong(12))`)
    /// must STAY `exposed`. The RIPR-SPEC-0106/#1252 variant-credit path sets
    /// `has_token_match=true` for a genuine `ExactErrorVariant` oracle whose
    /// text contains the probe's specific variant token, clearing
    /// `observation_unverified`.
    #[test]
    fn error_path_exact_variant_oracle_keeps_discriminate_yes() {
        // Probe: Err(ParseError::TooLong(len)) — variant token "TooLong".
        // Assertion: assert_eq!(err, ParseError::TooLong(12)) — contains "TooLong".
        let probe = probe(ProbeFamily::ErrorPath, "Err(ParseError::TooLong(len))");
        let test = test_with_assertions(
            "too_long_error_pinned",
            vec![oracle(
                "assert_eq!(err, ParseError::TooLong(12));",
                OracleKind::ExactErrorVariant,
                OracleStrength::Strong,
            )],
        );
        let (_observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(
            discriminate.state,
            StageState::Yes,
            "ErrorPath probe with a genuine variant-pinning ExactErrorVariant oracle must stay exposed (no over-correction)"
        );
        assert!(
            !discriminate.summary.contains("observation_unverified"),
            "variant-confirmed oracle must NOT emit observation_unverified: got `{}`",
            discriminate.summary
        );
    }

    #[test]
    fn error_path_parser_context_cannot_replace_emitted_probe_variant() {
        let probe = probe(ProbeFamily::ErrorPath, "Err(ParseError::TooLong(len))");
        let test = test_with_assertions(
            "too_long_is_exact",
            vec![oracle(
                "assert_eq!(err, ParseError::TooLong(12));",
                OracleKind::ExactErrorVariant,
                OracleStrength::Strong,
            )],
        );

        let (_observe, discriminate, _related) = reveal_evidence_with_expression(
            &probe,
            "Err(ParseError::SiblingVariant)",
            &[(&test, RelationReason::DirectOwnerCall)],
            &|_, _| false,
            &|_, _| false,
        );

        assert_eq!(
            discriminate.state,
            StageState::Yes,
            "the emitted exact variant remains authoritative when parser context resolves a sibling"
        );
        assert!(
            !discriminate.summary.contains("observation_unverified"),
            "variant fallback must preserve exact-error confirmation: got `{}`",
            discriminate.summary
        );
    }

    /// RIPR-SPEC-0107 Control B continued: `matches!(err, ParseError::TooLong(_))`
    /// also pins the variant token and must keep the seam `exposed`.
    #[test]
    fn error_path_matches_variant_oracle_keeps_discriminate_yes() {
        let probe = probe(ProbeFamily::ErrorPath, "Err(ParseError::TooLong(len))");
        let test = test_with_assertions(
            "too_long_error_matches",
            vec![oracle(
                "assert!(matches!(err, ParseError::TooLong(_)));",
                OracleKind::ExactErrorVariant,
                OracleStrength::Strong,
            )],
        );
        let (_observe, discriminate, _related) =
            reveal_evidence(&probe, &[(&test, RelationReason::DirectOwnerCall)]);

        assert_eq!(
            discriminate.state,
            StageState::Yes,
            "ErrorPath probe with a matches! variant oracle must stay exposed"
        );
    }

    /// RIPR-SPEC-0107 Control C (CROSS-SURFACE / IS-EFFECT-FAMILY guard):
    /// `is_effect_family` must return false for `ErrorPath`, ensuring that a
    /// mock/snapshot cannot clear `observation_unverified` for an error seam.
    /// Only a genuine variant-pinning oracle may confirm it.
    #[test]
    fn error_path_is_not_effect_family() {
        assert!(
            !is_effect_family(&ProbeFamily::ErrorPath),
            "ErrorPath must not be classified as an effect family (mocks must not clear observation_unverified)"
        );
    }

    // ── Short-token word-boundary matching (#2397) ───────────────────────────

    #[test]
    fn whole_word_match_accepts_short_token_in_field_access() {
        assert!(contains_as_whole_word("assert_eq!(result.id, 42)", "id"));
    }

    #[test]
    fn whole_word_match_accepts_short_token_in_equality() {
        assert!(contains_as_whole_word("assert!(id == 42)", "id"));
    }

    #[test]
    fn whole_word_match_rejects_short_token_as_substring() {
        // The anti-substring property: `id` must NOT match inside `provider`
        // or `middle`. This is what the old `> 3` gate tried to protect
        // against — word-boundary matching achieves it without filtering by
        // length.
        assert!(!contains_as_whole_word("provider", "id"));
        assert!(!contains_as_whole_word("middle", "id"));
        assert!(!contains_as_whole_word("candidate", "id"));
    }

    #[test]
    fn whole_word_match_accepts_long_token() {
        // Long tokens that matched under the old gate still match.
        assert!(contains_as_whole_word(
            "assert_eq!(priority, high)",
            "priority"
        ));
    }

    #[test]
    fn whole_word_match_rejects_empty_token() {
        assert!(!contains_as_whole_word("anything", ""));
    }
}
