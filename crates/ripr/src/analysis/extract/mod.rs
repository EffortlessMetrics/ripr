mod calls;
mod literals;
mod mask;
mod oracles;
mod probe_shapes;
mod returns;
mod shadow;
mod text;

pub(crate) use calls::extract_call_facts;
pub(crate) use literals::{extract_literal_facts, extract_literals};
pub(crate) use mask::mask_comments_and_strings;
#[cfg(test)]
pub(crate) use oracles::contains_macro_invocation;
pub(crate) use oracles::{
    OracleTextShape, classify_assertion, equality_assertion_arguments, err_return_guard_oracles,
    extract_assertions, extract_line_scanned_oracles,
    guarded_result_match_scan_with_shadow_authority, has_oracle_text_shape,
    is_unwrap_err_bound_error_assertion, unwrap_err_bound_variables,
};
pub(crate) use probe_shapes::{
    PROBE_SHAPE_CALL_DELETION, PROBE_SHAPE_ERROR_PATH, PROBE_SHAPE_FIELD_CONSTRUCTION,
    PROBE_SHAPE_MATCH_ARM, PROBE_SHAPE_PREDICATE, PROBE_SHAPE_RETURN_VALUE,
    PROBE_SHAPE_SIDE_EFFECT, PROBE_SHAPE_UNSAFE_BOUNDARY, is_known_probe_shape,
};
pub(crate) use returns::extract_return_facts;
pub(crate) use shadow::{
    ShadowAuthority, extract_pattern_words, fact_body_defines_callee_fn, fact_body_let_shadow_line,
    test_body_defines_callee_fn, test_body_let_shadow_line,
};
pub(crate) use text::extract_identifier_tokens;
