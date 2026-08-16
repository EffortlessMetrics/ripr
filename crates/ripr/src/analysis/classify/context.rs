use super::super::rust_index::{FunctionSummary, TestSummary};
use crate::domain::{Probe, RelationReason};

pub(in crate::analysis) struct ProbeContext<'a> {
    pub probe: &'a Probe,
    pub owner_fn: Option<&'a FunctionSummary>,
    /// Related tests paired with their relation reason so `reveal_evidence`
    /// can tag each emitted `RelatedTest` with `relation_reason` /
    /// `relation_confidence`.
    pub related_tests: Vec<(&'a TestSummary, RelationReason)>,
    /// RIPR-SPEC-0133: true when the changed owner is an assertion-shaped
    /// helper (an oracle), computed by `classify::owner_shape` at
    /// classification time where the full index is available. Guidance is
    /// reframed for oracle-shaped owners; the exposure class never changes.
    pub owner_assertion_shaped: bool,
}

impl<'a> ProbeContext<'a> {
    pub(in crate::analysis) fn new(
        probe: &'a Probe,
        owner_fn: Option<&'a FunctionSummary>,
        related_tests: Vec<(&'a TestSummary, RelationReason)>,
        owner_assertion_shaped: bool,
    ) -> Self {
        Self {
            probe,
            owner_fn,
            related_tests,
            owner_assertion_shaped,
        }
    }

    /// Borrow just the `TestSummary` references for callers that don't need
    /// the relation reason (reach, infection, activation).
    pub(in crate::analysis) fn related_test_summaries(&self) -> Vec<&TestSummary> {
        self.related_tests.iter().map(|(t, _)| *t).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{DeltaKind, ProbeFamily, ProbeId, SourceLocation};
    use std::path::PathBuf;

    #[test]
    fn probe_context_carries_probe_owner_and_related_tests() {
        let probe = Probe {
            id: ProbeId("probe:test".to_string()),
            location: SourceLocation::new(PathBuf::from("src/lib.rs"), 1, 1),
            owner: None,
            family: ProbeFamily::StaticUnknown,
            delta: DeltaKind::Unknown,
            before: None,
            after: Some("let value = total;".to_string()),
            expression: "let value = total;".to_string(),
            expected_sinks: Vec::new(),
            required_oracles: Vec::new(),
        };

        let context = ProbeContext::new(&probe, None, Vec::new(), false);

        assert_eq!(context.probe.id.0, "probe:test");
        assert!(context.owner_fn.is_none());
        assert!(context.related_tests.is_empty());
    }
}
