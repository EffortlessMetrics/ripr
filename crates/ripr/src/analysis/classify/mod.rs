mod activation;
mod context;
mod decision;
mod flow;
mod helper_transfer;
mod infection;
mod match_transfer;
mod owner_shape;
mod propagation_witness;
mod reach;
mod related_tests;
mod reveal;
mod scanner_transfer;
mod text;
mod transitive_reach;
mod value_transfer;

pub(in crate::analysis) use activation::{BOUNDARY_OPERAND_UNRESOLVED_MARKER, activation_evidence};
pub(in crate::analysis) use context::ProbeContext;
pub(in crate::analysis) use decision::{
    classify, confidence_score, ensure_unknown_stop_reason, missing_evidence,
    recommended_next_step, stop_reasons,
};
pub(in crate::analysis) use flow::{local_flow_sinks, propagation_evidence_with_witness};
pub(in crate::analysis) use helper_transfer::resolve_chain;
pub(in crate::analysis) use infection::infection_evidence;
pub(in crate::analysis) use owner_shape::is_assertion_shaped_owner;
pub(in crate::analysis) use propagation_witness::{PropagationWitnessV1, current_path_witness};
pub(in crate::analysis) use reach::reach_evidence;
pub(in crate::analysis) use related_tests::{
    DependencyEdgeContext, body_contains_owner_call, find_related_tests, package_prefix,
};
pub(in crate::analysis) use reveal::file_imports_foreign_callee_name;
pub(in crate::analysis) use reveal::reveal_evidence_with_expression;
pub(in crate::analysis) use reveal::wrapper_error_seam_expression;
// RIPR-SPEC-0106: re-export the variant parsers so test_grip_evidence.rs can
// apply variant-binding without reaching into the private `text` submodule.
pub(in crate::analysis) use text::{
    enum_variant_values, error_constructor_call_paths, error_constructor_payloads,
    error_result_payload_literal_sets, exact_error_variant, is_wildcard_discard_binding,
    rust_string_literals,
};
// RIPR-SPEC-0114: bounded transitive-reach walk for Rust no_static_path findings.
// RIPR-SPEC-0115: the walk now returns a witness so the limitation can name the
// witnessing test (file:line) and the entry public-API symbol.
pub(in crate::analysis) use transitive_reach::{
    MACRO_WITNESS_TEST_BODY_HOST, RUST_MACRO_REACH_MESSAGE, RUST_TRANSITIVE_REACH_MESSAGE,
    find_macro_reach_witness, find_transitive_witness, macro_reach_limitation_detail_lines,
    macro_reach_witness_pointer, transitive_reach_limitation_detail_lines,
    transitive_reach_witness_pointer,
};
