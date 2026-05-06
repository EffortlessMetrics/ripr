mod calls;
mod literals;
mod oracles;
mod probe_shapes;
mod returns;
mod text;

pub(crate) use calls::extract_call_facts;
pub(crate) use literals::{extract_literal_facts, extract_literals};
#[cfg(test)]
pub(crate) use oracles::contains_macro_invocation;
pub(crate) use oracles::{classify_assertion, extract_assertions, extract_line_scanned_oracles};
pub(crate) use probe_shapes::{
    PROBE_SHAPE_CALL_DELETION, PROBE_SHAPE_ERROR_PATH, PROBE_SHAPE_FIELD_CONSTRUCTION,
    PROBE_SHAPE_MATCH_ARM, PROBE_SHAPE_PREDICATE, PROBE_SHAPE_RETURN_VALUE,
    PROBE_SHAPE_SIDE_EFFECT,
};
pub(crate) use returns::extract_return_facts;
pub(crate) use text::extract_identifier_tokens;
