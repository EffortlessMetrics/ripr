use crate::analysis::facts::OracleFact;
use crate::domain::{OracleKind, OracleStrength};

use super::classify::classify_assertion;
use super::patterns::{
    is_custom_assertion_helper, is_mock_expectation_line, is_side_effect_observer_assertion,
    is_snapshot_assertion, is_unwrap_err_bound_error_assertion,
};
use crate::analysis::extract::text::extract_identifier_tokens;

pub(crate) fn extract_assertions(body: &str, start_line: usize) -> Vec<OracleFact> {
    let bound_error_vars = unwrap_err_bound_variables(body);
    let mut out = Vec::new();
    for (offset, line) in body.lines().enumerate() {
        let trimmed = line.trim();
        if is_assertion_line(trimmed) {
            let mut classification = classify_assertion(trimmed);
            // RIPR-SPEC-0106: upgrade ExactValue assertions on unwrap_err-bound
            // variables to ExactErrorVariant so the ErrorVariant seam can credit
            // them as a kind-matching discriminator.
            if classification.kind == OracleKind::ExactValue
                && is_unwrap_err_bound_error_assertion(trimmed, &bound_error_vars)
            {
                classification.kind = OracleKind::ExactErrorVariant;
                classification.strength = OracleStrength::Strong;
            }
            out.push(OracleFact {
                line: start_line + offset,
                text: trimmed.to_string(),
                kind: classification.kind,
                strength: classification.strength,
                observed_tokens: extract_identifier_tokens(trimmed),
            });
        }
    }
    out
}

/// Scan a test body for `let <var> = <expr>.unwrap_err()` and
/// `let <var> = <expr>.expect_err(...)` bindings.
///
/// Returns the set of variable names bound to unwrap_err results so
/// subsequent assertion lines can be recognized as error-variant oracles
/// (RIPR-SPEC-0106, Part A).
pub(crate) fn unwrap_err_bound_variables(body: &str) -> std::collections::BTreeSet<String> {
    let mut vars = std::collections::BTreeSet::new();
    // Split into statement-sized chunks on `;`/`{`/`}`/newline so a
    // `let <ident> = <expr>.unwrap_err()` binding is recognized regardless of
    // source formatting — not only when `let` begins a trimmed source line.
    // Without this, a single-line test body (`fn t() { let e = f().unwrap_err();
    // assert_eq!(e, E::V); }`, e.g. un-rustfmt'd) hides the binding, the
    // assertion is never upgraded to ExactErrorVariant, and the seam carries a
    // contradictory `missing_discriminators` line despite being discriminated.
    for stmt in body.split([';', '{', '}', '\n']) {
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
    for (offset, line) in body.lines().enumerate() {
        let trimmed = line.trim();
        if !is_line_scanned_oracle(trimmed) {
            continue;
        }
        let classification = classify_assertion(trimmed);
        out.push(OracleFact {
            line: start_line + offset,
            text: trimmed.to_string(),
            kind: classification.kind,
            strength: classification.strength,
            observed_tokens: extract_identifier_tokens(trimmed),
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
}

fn is_line_scanned_oracle(line: &str) -> bool {
    is_custom_assertion_helper(line)
        || is_side_effect_observer_assertion(line)
        || is_mock_expectation_line(line)
}

#[cfg(test)]
mod spec_0106_scan_tests {
    use super::*;
    use crate::domain::OracleKind;

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
