use super::super::rust_index::{FunctionSummary, RustIndex, TestSummary};
use super::helper_transfer::HelperChain;
use super::reveal::FileUseStatements;
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
    /// #3296: the bounded helper-transfer chain above the owner when the
    /// owner is reached through statically resolvable callers, computed
    /// once at classification time so the relation and activation stages
    /// consume the same authority.
    pub helper_chain: Option<HelperChain>,
    /// #3296: the workspace-complete index for helper-transfer
    /// resolution inside the evidence stages.
    pub index: &'a RustIndex,
    pub workspace_complete: bool,
    /// Run-scoped per-file `use` scan, attached by the classifier; `None`
    /// (unit-test contexts) scans on every query.
    pub file_use_statements: Option<&'a FileUseStatements>,
}

impl<'a> ProbeContext<'a> {
    pub(in crate::analysis) fn new(
        probe: &'a Probe,
        owner_fn: Option<&'a FunctionSummary>,
        related_tests: Vec<(&'a TestSummary, RelationReason)>,
        owner_assertion_shaped: bool,
        index: &'a RustIndex,
        workspace_complete: bool,
    ) -> Self {
        Self {
            probe,
            owner_fn,
            related_tests,
            owner_assertion_shaped,
            helper_chain: None,
            index,
            workspace_complete,
            file_use_statements: None,
        }
    }

    /// Share one classification run's per-file `use` scan across probes.
    pub(in crate::analysis) fn with_file_use_statements(
        mut self,
        file_use_statements: &'a FileUseStatements,
    ) -> Self {
        self.file_use_statements = Some(file_use_statements);
        self
    }

    /// Attach the #3296 helper-transfer chain (computed once by the
    /// classifier, where the index is available).
    pub(in crate::analysis) fn with_helper_chain(
        mut self,
        helper_chain: Option<HelperChain>,
    ) -> Self {
        self.helper_chain = helper_chain;
        self
    }

    /// Whether the related test file `file` (indexed source `source`)
    /// imports `callee` from a foreign path; see
    /// `use_statements_import_foreign_callee_name`. Uses the run-scoped scan when
    /// the classifier attached one.
    pub(in crate::analysis) fn test_file_imports_foreign_callee_name(
        &self,
        file: &std::path::Path,
        source: &str,
        callee: &str,
    ) -> bool {
        let names = &self.index.package_names;
        match self.file_use_statements {
            Some(memo) => memo.imports_foreign_callee_name(file, source, callee, names),
            None => FileUseStatements::default()
                .imports_foreign_callee_name(file, source, callee, names),
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

        let index = RustIndex::default();
        let context = ProbeContext::new(&probe, None, Vec::new(), false, &index, false);

        assert_eq!(context.probe.id.0, "probe:test");
        assert!(context.owner_fn.is_none());
        assert!(context.related_tests.is_empty());
    }
}
