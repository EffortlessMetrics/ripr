mod calls;
mod literals;
mod mask;
mod oracles;
pub(crate) mod property_macros;
mod returns;
mod shadow;
mod text;

pub(crate) use calls::extract_call_facts;
pub(crate) use literals::{extract_literal_facts, extract_literals};
pub(crate) use mask::mask_comments_and_strings;
#[cfg(test)]
pub(crate) use oracles::contains_macro_invocation;
pub(crate) use oracles::{
    OracleTextShape, assertion_oracle_text, classify_assertion, equality_assertion_arguments,
    err_return_guard_oracles, extract_assertions, extract_line_scanned_oracles,
    guarded_result_match_scan_with_shadow_authority, has_oracle_text_shape,
    is_unwrap_err_bound_error_assertion, outer_assertion_condition, unwrap_err_bound_variables,
};
pub(crate) use returns::extract_return_facts;
pub(crate) use shadow::{
    ShadowAuthority, extract_pattern_words, fact_body_defines_callee_fn, fact_body_let_shadow_line,
    test_body_defines_callee_fn, test_body_let_shadow_line,
};
pub(crate) use text::extract_identifier_tokens;
