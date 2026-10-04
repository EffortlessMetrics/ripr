mod activation;
mod boundary_pairing;
mod context;
mod decision;
mod flow;
mod helper_transfer;
mod infection;
mod match_transfer;
mod owner_pin;
mod owner_shape;
mod propagation_witness;
mod reach;
mod related_tests;
mod reveal;
mod scanner_transfer;
mod text;
mod transitive_reach;
mod value_transfer;

pub(in crate::analysis) use activation::{
    LocalBoundary, TestValueFacts, activation_evidence_with_value_facts, literal_operand_value,
    local_boundary,
};
pub(in crate::analysis) use boundary_pairing::{
    has_same_test_boundary_oracle_pairing, same_test_pairing_missing_summary,
};
pub(in crate::analysis) use context::ProbeContext;
pub(in crate::analysis) use decision::{
    classify, confidence_score, ensure_unknown_stop_reason, missing_evidence,
    recommended_next_step, stop_reasons,
};
pub(in crate::analysis) use flow::{local_flow_sinks, propagation_evidence_with_witness};
pub(in crate::analysis) use helper_transfer::resolve_chain;
pub(in crate::analysis) use infection::infection_evidence;
pub(in crate::analysis) use owner_pin::{OwnerPinSyntax, OwnerReturnPin};
pub(in crate::analysis) use owner_shape::is_assertion_shaped_owner;
pub(in crate::analysis) use propagation_witness::{
    PropagationWitnessV1, assertion_observes_direct_collection, current_path_witness,
    direct_collection_mutation_receiver,
};
pub(in crate::analysis) use reach::{owner_may_be_reached_unseen, reach_evidence};
pub(in crate::analysis) use related_tests::{
    DependencyEdgeContext, RelatedTestCandidateIndex, body_contains_owner_call,
    find_related_tests_with_candidate_index, impl_self_type_name,
    method_call_resolves_to_impl_type, package_prefix,
};
pub(in crate::analysis) use reveal::reveal_evidence_with_expression;
pub(in crate::analysis) use reveal::wrapper_error_seam_expression;
pub(in crate::analysis) use reveal::{ASSERTION_CONTEXT_UNESTABLISHED, FileUseStatements};
pub(in crate::analysis) use reveal::{ReturnOracleAdmission, contains_as_whole_word};
// RIPR-SPEC-0106: re-export the variant parsers so test_grip_evidence.rs can
// apply variant-binding without reaching into the private `text` submodule.
pub(in crate::analysis) use text::{
    enum_variant_values, error_constructor_call_paths, error_constructor_payloads,
    error_result_payload_literal_sets, exact_error_variant, rust_string_literals,
};
// RIPR-SPEC-0114: bounded transitive-reach walk for Rust no_static_path findings.
// RIPR-SPEC-0115: the walk now returns a witness so the limitation can name the
// witnessing test (file:line) and the entry public-API symbol.
pub(in crate::analysis) use transitive_reach::{
    MACRO_WITNESS_TEST_BODY_HOST, RUST_MACRO_REACH_MESSAGE, RUST_TRANSITIVE_REACH_MESSAGE,
    TransitiveReachIndex, macro_reach_limitation_detail_lines, macro_reach_witness_pointer,
    transitive_reach_limitation_detail_lines, transitive_reach_witness_pointer,
};
