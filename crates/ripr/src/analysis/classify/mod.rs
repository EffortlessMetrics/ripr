mod activation;
mod context;
mod decision;
mod flow;
mod infection;
mod reach;
mod related_tests;
mod reveal;
mod text;
mod transitive_reach;

pub(in crate::analysis) use activation::activation_evidence;
pub(in crate::analysis) use context::ProbeContext;
pub(in crate::analysis) use decision::{
    classify, confidence_score, ensure_unknown_stop_reason, missing_evidence,
    recommended_next_step, stop_reasons,
};
pub(in crate::analysis) use flow::{local_flow_sinks, propagation_evidence};
pub(in crate::analysis) use infection::infection_evidence;
pub(in crate::analysis) use reach::reach_evidence;
pub(in crate::analysis) use related_tests::find_related_tests;
pub(in crate::analysis) use reveal::reveal_evidence;
// RIPR-SPEC-0106: re-export the variant parsers so test_grip_evidence.rs can
// apply variant-binding without reaching into the private `text` submodule.
pub(in crate::analysis) use text::{enum_variant_values, exact_error_variant};
// RIPR-SPEC-0114: bounded transitive-reach walk for Rust no_static_path findings.
// RIPR-SPEC-0115: the walk now returns a witness so the limitation can name the
// witnessing test (file:line) and the entry public-API symbol.
pub(in crate::analysis) use transitive_reach::{
    RUST_TRANSITIVE_REACH_MESSAGE, find_transitive_witness, transitive_reach_witness_pointer,
};
