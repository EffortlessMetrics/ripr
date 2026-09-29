mod binding_predicate;
mod classify;
mod diff;
mod expectations;
mod family;
mod ids;
mod lexical;
mod repo;

#[cfg(test)]
pub(crate) use binding_predicate::PredicateOperandSide;
pub(crate) use binding_predicate::{
    BindingPredicateResolution, BindingValueResolution, ChangedBindingPredicateUse,
    resolve_changed_binding_uses,
};
pub(crate) use classify::parser_expression_for_probe;
pub(crate) use diff::probes_for_file_with_relations;
pub(crate) use diff::resolve_probe_source_currentness;
pub(crate) use expectations::{expected_sinks, required_oracles};
pub(crate) use ids::{fingerprint_probe_id, normalize_expression};
pub use repo::probes_for_repo_file;

#[cfg(test)]
mod parameter_boundary_tests;
#[cfg(test)]
mod record_field_boundary_tests;
