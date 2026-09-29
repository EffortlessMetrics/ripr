mod binding_predicate;
mod classify;
mod diff;
mod expectations;
mod family;
mod ids;
mod lexical;
mod repo;

use crate::analysis::diagnostic_origin::ParserByteSpan;
use crate::domain::Probe;
use std::collections::BTreeMap;

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
pub(crate) use ids::{fingerprint_probe_id, legacy_whole_line_diff_probe_id, normalize_expression};
#[cfg(test)]
pub(crate) use repo::probes_for_repo_file;
pub(crate) use repo::probes_for_repo_file_seeded;

/// One seeded probe plus optional parser origin and #3294 binding relation.
#[derive(Clone, Debug)]
pub(crate) struct SeededProbe {
    pub probe: Probe,
    pub binding_relation: Option<ChangedBindingPredicateUse>,
    pub parser_span: Option<ParserByteSpan>,
}

impl SeededProbe {
    pub(crate) fn from_probe(probe: Probe) -> Self {
        Self {
            probe,
            binding_relation: None,
            parser_span: None,
        }
    }

    pub(crate) fn with_span(probe: Probe, parser_span: ParserByteSpan) -> Self {
        Self {
            probe,
            binding_relation: None,
            parser_span: Some(parser_span),
        }
    }

    pub(crate) fn maybe_with_span(probe: Probe, parser_span: Option<ParserByteSpan>) -> Self {
        match parser_span {
            Some(span) => Self::with_span(probe, span),
            None => Self::from_probe(probe),
        }
    }

    pub(crate) fn retargeted(probe: Probe, binding_relation: ChangedBindingPredicateUse) -> Self {
        Self {
            probe,
            binding_relation: Some(binding_relation),
            parser_span: None,
        }
    }

    pub(crate) fn record_span(&self, parser_spans: &mut BTreeMap<String, ParserByteSpan>) {
        if let Some(span) = self.parser_span {
            parser_spans.insert(self.probe.id.0.clone(), span);
        }
    }
}

#[cfg(test)]
mod parameter_boundary_tests;
#[cfg(test)]
mod record_field_boundary_tests;
