use super::super::rust_index::{FunctionSummary, TestSummary};
use crate::domain::Probe;

pub(in crate::analysis) struct ProbeContext<'a> {
    pub probe: &'a Probe,
    pub owner_fn: Option<&'a FunctionSummary>,
    pub related_tests: Vec<&'a TestSummary>,
}

impl<'a> ProbeContext<'a> {
    pub(in crate::analysis) fn new(
        probe: &'a Probe,
        owner_fn: Option<&'a FunctionSummary>,
        related_tests: Vec<&'a TestSummary>,
    ) -> Self {
        Self {
            probe,
            owner_fn,
            related_tests,
        }
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

        let context = ProbeContext::new(&probe, None, Vec::new());

        assert_eq!(context.probe.id.0, "probe:test");
        assert!(context.owner_fn.is_none());
        assert!(context.related_tests.is_empty());
    }
}
