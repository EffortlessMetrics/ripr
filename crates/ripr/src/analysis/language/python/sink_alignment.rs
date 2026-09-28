use super::{
    PythonOwner, PythonTest, has_identifier_boundary, import_source_module_matches_owner,
    parse_attribute_assignment, python_dict_field_segment_parts, significant_change_tokens,
    strong_test_calls_owner_method_on_bound_receiver, strong_test_imports_owner_from_module,
    top_level_python_segments,
};
use crate::domain::{OracleStrength, OwnerKind, RelatedTest};
/// The visible read-out of the sink-alignment decision. `ripr`'s value over
/// coverage is that a strong oracle credits `exposed` only when it *observes the
/// changed sink*, not merely reaches the owner. This carries which token
/// category the strongest oracle matched so a consumer can see *why* a strong
/// oracle did or did not credit `exposed`. It is a pure read-out: the boolean
/// the classifier uses is derived from `oracle_alignment` (see
/// [`SinkAlignment::observes`]), so the surfaced value can never disagree with
/// the decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SinkAlignment {
    pub(super) changed_sink: Option<String>,
    pub(super) observed_sink: Option<String>,
    /// One of `direct | alias | changed_sink_token | orthogonal | unknown`.
    pub(super) oracle_alignment: String,
    pub(super) alignment_reason: String,
}

impl SinkAlignment {
    /// The decision the classifier uses, derived from the alignment so the two
    /// can never drift. A strong oracle observes the owner when the alignment is
    /// `direct`/`alias`/`changed_sink_token`, OR in the legacy module-owner /
    /// empty-token case (alignment `unknown`, reason `module_owner_no_sink_token`)
    /// which historically returned `true` — the one place where `unknown` does
    /// not imply not-observed, preserved to keep the prior decision unchanged.
    pub(super) fn observes(&self) -> bool {
        matches!(
            self.oracle_alignment.as_str(),
            "direct" | "alias" | "changed_sink_token"
        ) || self.alignment_reason == "module_owner_no_sink_token"
    }

    /// The alignment surfaced when the classifier did not reach a strong-oracle
    /// branch (no-static-path, static-limit, heuristic-only, or weak-oracle
    /// findings never compute owner alignment). `changed_sink` is retained
    /// because it describes the changed line regardless of the test side.
    pub(super) fn unknown(changed_sink: Option<String>) -> Self {
        SinkAlignment {
            changed_sink,
            observed_sink: None,
            oracle_alignment: "unknown".to_string(),
            alignment_reason: "no_strong_oracle".to_string(),
        }
    }
}

/// Whether `token` appears in `text` as a whole Python identifier rather than a
/// substring of a larger one. Without this, a common changed-sink token like
/// `buffer` would spuriously "observe" an unrelated oracle that merely contains
/// it (e.g. `buffered_stream` from a different class), over-crediting `exposed` —
/// a confirmed false-exposed vector. Mirrors the identifier-boundary rule of
/// [`has_identifier_boundary`]; whole words such as `key` in `Invalid key` still
/// match, so genuine sink observation is preserved.
pub(super) fn oracle_text_observes_token(text: &str, token: &str) -> bool {
    if token.is_empty() {
        return false;
    }
    text.match_indices(token)
        .any(|(idx, _)| has_identifier_boundary(text, idx, token.len()))
}

/// Classify whether a strong related-test oracle actually observes the changed
/// behavior's sink — and *which* token category matched. Reach-plus-strong-oracle
/// is not enough: a strong oracle that asserts a *different* value (e.g. a
/// wrapper's return) does not discriminate the change. The three token groups
/// (owner name, import alias, changed-sink tokens) are probed in order against
/// the strongest related oracles, mirroring the prior boolean exactly so the
/// derived decision is unchanged.
#[cfg(test)]
pub(super) fn classify_sink_alignment(
    owner: &PythonOwner,
    line_text: &str,
    related: &[RelatedTest],
    all_tests: &[PythonTest],
) -> SinkAlignment {
    classify_sink_alignment_with_old(owner, line_text, None, related, all_tests)
}

/// Whether a strong related-test oracle actually observes the changed behavior's
/// sink — not merely reaches the owner. Derived from [`classify_sink_alignment`]
/// so the boolean and the surfaced `oracle_alignment` can never disagree. Now a
/// `#[cfg(test)]` regression helper: production code reads the alignment directly
/// via [`SinkAlignment::observes`], and the existing boolean tests pin that the
/// derivation stays equivalent to the prior decision.
#[cfg(test)]
pub(super) fn strong_oracle_observes_owner(
    owner: &PythonOwner,
    line_text: &str,
    related: &[RelatedTest],
    all_tests: &[PythonTest],
) -> bool {
    classify_sink_alignment(owner, line_text, related, all_tests).observes()
}

/// Whether the changed line is genuinely a control-flow construct (a predicate or
/// error-path), for the empty-token-delta fallback gate. This mirrors the
/// `Predicate` and `ErrorPath` conditions of [`classify_probe_shape`] WITHOUT its
/// default arm (which returns `Control` for any unrecognized line, e.g. a plain
/// `total = base - bonus` assignment). Keying the empty-delta operand fallback on
/// this precise shape — not the default-polluted delta kind — is the #1288 fix:
/// only a real branch / raise change can be discriminated by an outcome oracle that
/// merely matches a line token; a value-producing assignment cannot.
fn is_control_flow_change_line(line_text: &str) -> bool {
    let trimmed = line_text.trim_start();
    (trimmed.contains(" if ") && trimmed.contains(" else "))
        || trimmed.starts_with("if ")
        || trimmed.starts_with("elif ")
        || trimmed.starts_with("while ")
        || trimmed.starts_with("for ")
        || trimmed.starts_with("match ")
        || trimmed.starts_with("case ")
        || trimmed.starts_with("raise ")
        || trimmed == "raise"
        || trimmed.starts_with("try:")
        || trimmed.starts_with("except ")
        || trimmed.starts_with("except* ")
        || trimmed.starts_with("finally:")
        || (trimmed.starts_with("with ") && trimmed.contains("raises("))
}

/// Parse a dict-literal line (a `return {...}` or `lhs = {...}`) into its top-level
/// `(key, value)` pairs. Keys are unquoted; values keep their source text. Returns
/// `None` if the line has no `{...}` body or no parseable fields.
pub(super) fn parse_dict_literal_fields(line: &str) -> Option<Vec<(String, String)>> {
    let trimmed = line.trim();
    // The dict EXPRESSION must literally START with `{` (after an optional `return `),
    // not merely contain one — otherwise an f-string like `f"{value:.3f}"`, a
    // `.format(...)` call, or any line with `{` inside a string literal is mis-read as
    // a dict literal and wrongly gated (this follow-up fixes a regressed f-string
    // discriminator). A set literal `{1, 2}` is naturally excluded below since it has
    // no top-level `key: value` segments.
    let expr = trimmed
        .strip_prefix("return ")
        .map(str::trim)
        .unwrap_or(trimmed);
    let body = expr.strip_prefix('{')?.strip_suffix('}')?;
    let mut fields = Vec::new();
    for segment in top_level_python_segments(body) {
        if let Some((key, value)) = python_dict_field_segment_parts(segment) {
            fields.push((key.to_string(), value.trim().to_string()));
        }
    }
    if fields.is_empty() {
        None
    } else {
        Some(fields)
    }
}

/// For a dict-literal field-construction change, the keys whose value differs
/// between the old and new line (added / removed / re-valued), plus the NEW values
/// of those keys. Returns `None` when the change is not a dict-literal change on
/// both sides, or when nothing localizable changed — in which case the #1290
/// dict-element gate is a pass-through.
pub(super) fn dict_changed_keys_and_values(
    old_line: Option<&str>,
    new_line: &str,
) -> Option<(Vec<String>, Vec<String>)> {
    let old = parse_dict_literal_fields(old_line?)?;
    let new = parse_dict_literal_fields(new_line)?;
    let mut changed_keys = Vec::new();
    let mut changed_values = Vec::new();
    for (key, value) in &new {
        let old_value = old.iter().find(|(k, _)| k == key).map(|(_, v)| v);
        if old_value != Some(value) {
            changed_keys.push(key.clone());
            changed_values.push(value.clone());
        }
    }
    // A key present in the old line but removed in the new line is also a change.
    for (key, _) in &old {
        if !new.iter().any(|(k, _)| k == key) {
            changed_keys.push(key.clone());
        }
    }
    if changed_keys.is_empty() {
        None
    } else {
        Some((changed_keys, changed_values))
    }
}

/// Whether a strong oracle observes the CHANGED dict element (#1290): a subscript or
/// `.get(...)` of a changed key, the changed value literal, or a whole-collection
/// comparison. Conservative — when in doubt it returns `true` (credit stands) so a
/// genuine discriminator is never dropped; it only returns `false` for an oracle
/// that observes purely a sibling key or an aggregate.
fn oracle_observes_changed_dict_element(
    oracle: &str,
    changed_keys: &[String],
    changed_values: &[String],
) -> bool {
    // A whole-collection comparison (`== {...}` / `== [...]`) observes every element.
    if oracle.contains("=={")
        || oracle.contains("== {")
        || oracle.contains("==[")
        || oracle.contains("== [")
    {
        return true;
    }
    // Observes a changed value literal (e.g. the new `9090` / `"failure"`).
    for value in changed_values {
        let literal = value.trim().trim_matches('"').trim_matches('\'');
        if literal.len() >= 2 && oracle.contains(literal) {
            return true;
        }
    }
    // Subscripts a changed key by literal (`["port"]`, `['port']`, `.get("port")`).
    for key in changed_keys {
        if oracle.contains(&format!("[\"{key}\"]"))
            || oracle.contains(&format!("['{key}']"))
            || oracle.contains(&format!(".get(\"{key}\")"))
            || oracle.contains(&format!(".get('{key}')"))
        {
            return true;
        }
    }
    false
}

/// Parse a list-literal line (a `return [...]`) into its top-level element source
/// texts, in order. Like [`parse_dict_literal_fields`], the expression must literally
/// START with `[` (after an optional `return `) so a subscript expression such as
/// `arr[-1]` or an f-string is never mis-read as a list literal.
fn parse_list_literal_elements(line: &str) -> Option<Vec<String>> {
    let trimmed = line.trim();
    let expr = trimmed
        .strip_prefix("return ")
        .map(str::trim)
        .unwrap_or(trimmed);
    let body = expr.strip_prefix('[')?.strip_suffix(']')?;
    let elements: Vec<String> = top_level_python_segments(body)
        .into_iter()
        .map(|segment| segment.trim().to_string())
        .filter(|segment| !segment.is_empty())
        .collect();
    if elements.is_empty() {
        None
    } else {
        Some(elements)
    }
}

/// For a list-literal field-construction change, the positions (indices) whose
/// element differs between the old and new line, plus the NEW element source at
/// those positions. Returns `None` when the change is not a list-literal change on
/// both sides, when the lengths differ (a structural change is observed by `len`),
/// or when nothing changed — in which case the list-element gate is a pass-through.
fn list_changed_indices_and_values(
    old_line: Option<&str>,
    new_line: &str,
) -> Option<(Vec<usize>, Vec<String>)> {
    let old = parse_list_literal_elements(old_line?)?;
    let new = parse_list_literal_elements(new_line)?;
    // A length change is discriminated by `len(...)`, so it is NOT gated here.
    if old.len() != new.len() {
        return None;
    }
    let mut changed_indices = Vec::new();
    let mut changed_values = Vec::new();
    for (index, (old_value, new_value)) in old.iter().zip(new.iter()).enumerate() {
        if old_value != new_value {
            changed_indices.push(index);
            changed_values.push(new_value.clone());
        }
    }
    if changed_indices.is_empty() {
        None
    } else {
        Some((changed_indices, changed_values))
    }
}

/// Whether a strong oracle observes the CHANGED list element (#1290): a subscript of
/// a changed index, the changed element value literal, or a whole-collection
/// comparison. Conservative, mirroring [`oracle_observes_changed_dict_element`]: it
/// returns `false` only for an oracle that observes purely a sibling index or an
/// aggregate (`len(...)`).
fn oracle_observes_changed_list_element(
    oracle: &str,
    changed_indices: &[usize],
    changed_values: &[String],
) -> bool {
    if oracle.contains("=={")
        || oracle.contains("== {")
        || oracle.contains("==[")
        || oracle.contains("== [")
    {
        return true;
    }
    for value in changed_values {
        let literal = value.trim().trim_matches('"').trim_matches('\'');
        if literal.len() >= 2 && oracle.contains(literal) {
            return true;
        }
    }
    for index in changed_indices {
        if oracle.contains(&format!("[{index}]")) {
            return true;
        }
    }
    false
}

/// Split an f-string source (`f"..."`/`f'...'`, optional `r` prefix) into its
/// concatenated literal text and its ordered `{...}` interpolation substrings.
/// Returns `None` if the line (after an optional `return `) is not a single f-string.
pub(super) fn fstring_template(line: &str) -> Option<(String, Vec<String>)> {
    let trimmed = line.trim();
    let expr = trimmed
        .strip_prefix("return ")
        .map(str::trim)
        .unwrap_or(trimmed);
    // Accept f / rf / fr prefixes (case-insensitive), then a quote.
    let lower = expr.to_ascii_lowercase();
    let prefix_len = if lower.starts_with("f\"") || lower.starts_with("f'") {
        1
    } else if lower.starts_with("rf\"")
        || lower.starts_with("rf'")
        || lower.starts_with("fr\"")
        || lower.starts_with("fr'")
    {
        2
    } else {
        return None;
    };
    let rest = &expr[prefix_len..];
    let quote = rest.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let body = rest.strip_prefix(quote)?.strip_suffix(quote)?;
    // Fail open on f-string shapes this simple parser does not model precisely, so the
    // gate never downgrades on a mis-parse: escaped braces (`{{` / `}}`), a leftover
    // quote from a triple-quoted string, or nested interpolation/format-spec
    // (`{value:{width}}`). When unsupported, return `None` and the gate is a no-op.
    if body.contains("{{") || body.contains("}}") || body.starts_with(quote) {
        return None;
    }
    let mut literals = String::new();
    let mut interpolations = Vec::new();
    let mut depth = 0usize;
    let mut current = String::new();
    for ch in body.chars() {
        match ch {
            '{' => {
                depth += 1;
                if depth > 1 {
                    // Nested interpolation / format-spec — unsupported, fail open.
                    return None;
                }
                continue;
            }
            '}' if depth >= 1 => {
                depth -= 1;
                interpolations.push(current.clone());
                current.clear();
                continue;
            }
            _ => {}
        }
        if depth == 0 {
            literals.push(ch);
        } else {
            current.push(ch);
        }
    }
    if depth != 0 {
        return None;
    }
    Some((literals, interpolations))
}

/// Whether an f-string change is output-length-invariant: the interpolations are
/// identical (so the runtime-substituted text is unchanged) and the literal text has
/// the same length. When true, a `len(...)` oracle provably cannot discriminate the
/// change, so observing the owner's output only through `len` is not a discriminator.
/// A format-spec change (`:.2f` -> `:.3f`) alters an interpolation, so it is NOT
/// length-invariant and is never gated here.
pub(super) fn fstring_change_is_length_invariant(old_line: &str, new_line: &str) -> bool {
    match (fstring_template(old_line), fstring_template(new_line)) {
        (Some((old_literals, old_interps)), Some((new_literals, new_interps))) => {
            old_interps == new_interps
                && old_literals.chars().count() == new_literals.chars().count()
        }
        _ => false,
    }
}

/// Whether an oracle observes the owner's output ONLY through a `len(...)` aggregate
/// — i.e. it calls `len(` and does NOT also observe the output exactly. An oracle
/// that compares against a string literal, or that contains the changed f-string
/// literal text, observes the changed output and is therefore NOT a pure aggregate
/// (so the #1290 1b gate must not downgrade it). Conservative by design: anything
/// other than a clear length-only observation keeps the credit.
pub(super) fn oracle_is_pure_len_aggregate(oracle: &str, changed_literals: &str) -> bool {
    if !oracle.contains("len(") {
        return false;
    }
    // An exact string-equality comparison observes the produced string, not just its
    // length.
    if oracle.contains("== \"")
        || oracle.contains("== '")
        || oracle.contains("==\"")
        || oracle.contains("=='")
    {
        return false;
    }
    // The changed literal text appearing in the oracle means it observes the change.
    let trimmed = changed_literals.trim();
    if trimmed.len() >= 2 && oracle.contains(trimmed) {
        return false;
    }
    true
}

pub(super) fn classify_sink_alignment_with_old(
    owner: &PythonOwner,
    line_text: &str,
    old_line_text: Option<&str>,
    related: &[RelatedTest],
    all_tests: &[PythonTest],
) -> SinkAlignment {
    // Whether the changed line is genuinely a control-flow construct gates the
    // empty-token-delta fallback below: only a control-flow change may credit
    // `changed_sink_token` from bare line operands when the token delta is empty
    // (see the fallback comment). This is a PRECISE shape check (#1288) — not
    // `classify_probe_shape(..).1 == Control`, whose default arm also returns
    // `Control` for unrecognized lines such as plain local assignments
    // (`total = base - bonus`), which would wrongly keep the operand fallback and
    // re-introduce the false-`exposed`. Value/effect changes are observed via the
    // owner call, not an input operand.
    let changed_line_is_control_flow = is_control_flow_change_line(line_text);
    // `changed_sink` describes the changed line; deduped, joined for display.
    let change_tokens = significant_change_tokens(line_text);
    let mut change_display: Vec<String> = Vec::new();
    for token in &change_tokens {
        if !change_display.contains(token) {
            change_display.push(token.clone());
        }
    }
    let changed_sink = if change_display.is_empty() {
        None
    } else {
        Some(change_display.join(", "))
    };

    // The strongest strong-rank related oracle is the one the decision inspects.
    let strong_tests: Vec<&RelatedTest> = related
        .iter()
        .filter(|test| test.oracle_strength.rank() >= OracleStrength::Strong.rank())
        .collect();
    if strong_tests.is_empty() {
        return SinkAlignment::unknown(changed_sink);
    }
    let observed_sink = strong_tests
        .iter()
        .max_by_key(|test| test.oracle_strength.rank())
        .and_then(|test| test.oracle.clone());

    // Owner-name tokens, split for identity-aware crediting. For a method /
    // classmethod owner the bare method name is collision-prone: a same-named
    // method on an unrelated class (`PaymentProcessor.validate` vs owner
    // `TokenValidator.validate`) shares the token `validate` yet is a different
    // entity. Crediting `direct` from the bare method name alone is a silent
    // false-`exposed`, so it requires owner-class identity (the class token is
    // observed, or a strong observing test imports the owner's class). Class
    // tokens and free-function names are unambiguous and credit directly.
    let is_method_owner = matches!(
        owner.owner_kind,
        Some(OwnerKind::Method | OwnerKind::ClassMethod)
    );
    let owner_class_token: Option<String> = if is_method_owner {
        owner
            .qualified_name
            .rsplit_once('.')
            .map(|(class, _)| class)
            .filter(|class| !class.is_empty() && *class != "<module>")
            .map(str::to_string)
    } else {
        None
    };
    // Unambiguous identity tokens: every qualified-name segment except the bare
    // method name of a method owner. For non-method owners this is the full set
    // (a free-function name is its own identity).
    let mut identity_tokens: Vec<String> = owner
        .qualified_name
        .split('.')
        .filter(|token| !token.is_empty() && *token != "<module>")
        .filter(|token| !(is_method_owner && *token == owner.name))
        .map(str::to_string)
        .collect();
    if !is_method_owner && !owner.name.is_empty() && owner.name != "<module>" {
        identity_tokens.push(owner.name.clone());
    }
    // The collision-prone bare method name of a method owner, gated on identity.
    let method_name_token: Option<String> =
        if is_method_owner && !owner.name.is_empty() && owner.name != "<module>" {
            Some(owner.name.clone())
        } else {
            None
        };
    // Import aliases of the owner: `from m import owner as alias` makes the test
    // assert on `alias(...)`, which still observes the owner's output.
    let owner_simple = owner
        .qualified_name
        .rsplit('.')
        .next()
        .unwrap_or(owner.name.as_str());
    let mut alias_tokens: Vec<String> = Vec::new();
    for test in all_tests {
        for import in &test.imports {
            // For a method/classmethod owner the bare method name is not directly
            // importable, so only the owner's CLASS alias is identity-bearing.
            // Matching the bare method name here would credit any same-named free
            // function aliased in an unrelated module (a false-`exposed`).
            let imported_matches = if is_method_owner {
                owner_class_token.as_deref() == Some(import.imported.as_str())
            } else {
                // Free-function alias: require module identity, else a same-named
                // function aliased from an unrelated module credits a false-`exposed`.
                (import.imported == owner_simple || import.imported == owner.name)
                    && import_source_module_matches_owner(import, owner)
            };
            if imported_matches && !import.alias.is_empty() {
                alias_tokens.push(import.alias.clone());
            }
        }
    }
    let mut change_only = change_tokens.clone();
    identity_tokens.retain(|token| token.len() >= 2);
    let method_name_token = method_name_token.filter(|token| token.len() >= 2);
    alias_tokens.retain(|token| token.len() >= 2);
    change_only.retain(|token| token.len() >= 2);
    // Delta tokens: the changed-sink-token credit must reflect what actually
    // CHANGED on the line, not every operand on it. A token that is unchanged
    // between the old and new line (e.g. `valid_tokens` in `token in valid_tokens`
    // -> `token.strip() in valid_tokens`, or `_balance` in a `max(0, ...)` wrap) is
    // not the behavior delta; an oracle observing only such an operand does not
    // discriminate the change. When the old line is unavailable (a pure addition),
    // every token is part of the delta.
    let delta_tokens: Vec<String> = match old_line_text {
        Some(old) => {
            let old_tokens: std::collections::BTreeSet<String> =
                significant_change_tokens(old).into_iter().collect();
            let delta: Vec<String> = change_only
                .iter()
                .filter(|token| !old_tokens.contains(*token))
                .cloned()
                .collect();
            // An empty token delta means the change is in non-tokenized syntax (an
            // operator like `<=` -> `<`, punctuation, ordering) that token extraction
            // does not capture. Falling back to the full changed-line tokens is only
            // sound for a CONTROL-FLOW change (#1278): for a predicate / error-path
            // operator change the discriminated outcome (a taken branch, a raised
            // exception) can be observed by an outcome oracle that matches a line
            // token — as in `python_cross_file_construct_call`'s `pytest.raises`. For
            // a VALUE/EFFECT change (a `return` or assignment operator edit), every
            // line token is an unchanged INPUT operand; the changed sink is the
            // produced value, which a discriminating test observes via the owner call
            // (the `direct`/`alias` paths), not via an input token. Crediting
            // `changed_sink_token` on an input operand there is the false-`exposed`
            // (e.g. `return count + 1` -> `count - 1` with `assert count == 5`), so
            // for non-control empty deltas we credit nothing rather than the operands.
            if delta.is_empty() {
                if changed_line_is_control_flow {
                    change_only.clone()
                } else {
                    Vec::new()
                }
            } else {
                delta
            }
        }
        None => change_only.clone(),
    };

    // Module owner / no usable token: the prior boolean returned `true` here, so
    // the decision must stay `observes`. Map to `unknown` with the reason that
    // `observes()` special-cases back to true.
    if identity_tokens.is_empty()
        && method_name_token.is_none()
        && alias_tokens.is_empty()
        && change_only.is_empty()
    {
        return SinkAlignment {
            changed_sink,
            observed_sink,
            oracle_alignment: "unknown".to_string(),
            alignment_reason: "module_owner_no_sink_token".to_string(),
        };
    }

    let any_strong_observes = |group: &[String]| -> bool {
        strong_tests.iter().any(|test| {
            test.oracle.as_deref().is_some_and(|text| {
                group
                    .iter()
                    .any(|token| oracle_text_observes_token(text, token))
            })
        })
    };
    // Receiver identity for a method owner: a strong observing test must call the
    // owner's method on a receiver statically bound to the owner class (inline
    // construct, local binding, or a classmethod/direct call on the class). A bare
    // method-name match — even with the owner class imported and mentioned — is not
    // identity-bearing, because the asserted `.method(` may run on an unrelated
    // receiver while the class is referenced (or merely named) elsewhere. This is
    // the false-`exposed` guard at the relation layer.
    let strong_test_binds_method_receiver = strong_test_calls_owner_method_on_bound_receiver(
        owner_class_token.as_ref(),
        method_name_token.as_ref(),
        &strong_tests,
        all_tests,
    );
    let method_name_observed = method_name_token
        .as_ref()
        .is_some_and(|token| any_strong_observes(std::slice::from_ref(token)));
    // Free-function module identity: a non-method owner's bare function-name token
    // credits `direct` only when a strong observing test imports it from the
    // owner's module. A same-named free function imported from a different module
    // (`from src.checker import validate` for owner `src.handler.validate`) is not
    // identity-bearing — the false-`exposed` guard for free functions.
    let free_fn_module_identity =
        !is_method_owner && strong_test_imports_owner_from_module(&strong_tests, all_tests, owner);
    // Receiver/value identity for an attribute-assignment changed sink. The bare
    // attribute token (`status`) is collision-prone: a same-named field on an
    // unrelated receiver (`session.status` changed, oracle `conn.status == ...`)
    // would otherwise credit `changed_sink_token` on token coincidence. For an
    // attribute write `recv.attr = value`, credit only when a strong oracle
    // observes the receiver-qualified `recv.attr`, OR observes the assigned VALUE
    // together with the attribute name (co-observation defeats a common-literal
    // value coinciding in an unrelated oracle, while keeping legitimate
    // `obj.attr == value` field assertions). Non-attribute changed lines (returns,
    // method calls, comparisons) are not gated and keep prior behavior.
    let change_only_credit_ok = match parse_attribute_assignment(line_text) {
        None => true,
        Some((receiver, attr, rhs)) => {
            let qualified = [format!("{receiver}.{attr}")];
            let mut value_tokens = significant_change_tokens(rhs);
            value_tokens.retain(|token| token.len() >= 2);
            let attr_token = [attr.to_string()];
            any_strong_observes(&qualified)
                || (!value_tokens.is_empty()
                    && any_strong_observes(&value_tokens)
                    && any_strong_observes(&attr_token))
        }
    };
    // Changed-element identity for a dict-literal field-construction change (#1290).
    // A dict-literal change is localized to specific key(s) (`{"port": 8080}` ->
    // `9090` changes only `port`), but a strong oracle that merely calls the owner
    // and observes a SIBLING key (`build_config()["host"]`) or an aggregate
    // (`len(...)`) does not discriminate the change. Credit only when a strong
    // oracle observes the CHANGED element: its changed value, a subscript of the
    // changed key, or a whole-collection comparison. Non-dict changes (and changes
    // whose changed keys cannot be localized from the paired old line) are not gated
    // and keep prior behavior (pass-through `true`).
    let field_construction_credit_ok = if let Some((changed_keys, changed_values)) =
        dict_changed_keys_and_values(old_line_text, line_text)
    {
        strong_tests.iter().any(|test| {
            test.oracle.as_deref().is_some_and(|text| {
                oracle_observes_changed_dict_element(text, &changed_keys, &changed_values)
            })
        })
    } else if let Some((changed_indices, changed_values)) =
        list_changed_indices_and_values(old_line_text, line_text)
    {
        strong_tests.iter().any(|test| {
            test.oracle.as_deref().is_some_and(|text| {
                oracle_observes_changed_list_element(text, &changed_indices, &changed_values)
            })
        })
    } else {
        true
    };
    // F-string aggregate gate (#1290 1b): a length-invariant f-string change (only
    // literal text changed, interpolations unchanged) observed SOLELY through a
    // `len(...)` aggregate is not discriminated — the output length is identical, so
    // `len` cannot notice the change. Downgrade only when every strong oracle is such
    // a length aggregate; a string-equality oracle (`== "..."`) or a format-spec
    // change (which alters an interpolation, so it is not length-invariant) keeps the
    // credit. Pass-through `true` for any non-f-string change.
    let fstring_credit_ok = match old_line_text {
        Some(old) if fstring_change_is_length_invariant(old, line_text) => {
            // Credit stands unless EVERY strong oracle is a PURE `len(...)` aggregate,
            // which cannot discriminate a length-invariant f-string change. An oracle
            // that ALSO observes the output exactly (a string-equality comparison, or
            // the changed literal text) keeps the credit — fail open so this narrow
            // false-`exposed` fix never introduces a false negative (e.g.
            // `assert len(f(x)) == 4 and f(x) == "NO:7"`). (`strong_tests` is non-empty
            // here — the empty case returned `unknown` above.)
            let new_literals = fstring_template(line_text)
                .map(|(literals, _)| literals)
                .unwrap_or_default();
            !strong_tests.iter().all(|test| {
                test.oracle
                    .as_deref()
                    .is_some_and(|text| oracle_is_pure_len_aggregate(text, &new_literals))
            })
        }
        _ => true,
    };
    // The literal-element and f-string gates (#1290) apply to EVERY credit branch —
    // like the #1249 every-branch lesson — so a sibling-key / aggregate-only oracle
    // cannot sneak `exposed` through the direct/alias/changed_sink_token path of a
    // localized literal change. Both are pass-through `true` for unrelated changes.
    let (oracle_alignment, alignment_reason) = if any_strong_observes(&identity_tokens)
        && (is_method_owner || free_fn_module_identity)
        && field_construction_credit_ok
        && fstring_credit_ok
    {
        ("direct", "strong_oracle_observes_owner_name")
    } else if method_name_observed
        && strong_test_binds_method_receiver
        && field_construction_credit_ok
        && fstring_credit_ok
    {
        (
            "direct",
            "strong_oracle_observes_owner_method_on_bound_receiver",
        )
    } else if any_strong_observes(&alias_tokens)
        && field_construction_credit_ok
        && fstring_credit_ok
    {
        ("alias", "strong_oracle_observes_import_alias")
    } else if any_strong_observes(&delta_tokens)
        && change_only_credit_ok
        && field_construction_credit_ok
        && fstring_credit_ok
        && (is_method_owner || free_fn_module_identity)
    {
        // Gate the changed-sink-token path with the same free-function module
        // identity as the direct/alias paths: a same-named free function from a
        // different module must not credit `exposed` via this sibling branch
        // either (the #1249 every-branch lesson). Method owners are unaffected.
        (
            "changed_sink_token",
            "strong_oracle_observes_changed_sink_token",
        )
    } else {
        ("orthogonal", "strong_oracle_observes_different_sink")
    };
    SinkAlignment {
        changed_sink,
        observed_sink,
        oracle_alignment: oracle_alignment.to_string(),
        alignment_reason: alignment_reason.to_string(),
    }
}
