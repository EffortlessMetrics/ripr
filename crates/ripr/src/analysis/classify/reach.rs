use super::super::rust_index::{FunctionSummary, TestSummary};
use crate::domain::{Confidence, RelationReason, StageEvidence, StageState};

pub(in crate::analysis) fn reach_evidence(
    related_tests: &[(&TestSummary, RelationReason)],
    owner_fn: Option<&FunctionSummary>,
) -> StageEvidence {
    if related_tests.is_empty() {
        return StageEvidence::new(
            StageState::No,
            Confidence::Medium,
            "No static test path found for the changed owner",
        );
    }
    // #3714 round-2 review (devin hDRL2): a `SeamCalleeCall` relation means
    // the test exercises the seam's converted callee — it never invokes the
    // changed owner or its conversion, so the reach summary must not claim
    // owner reach for it. Owner-anchored relations keep the established
    // phrasing; callee-only relations carry their own honest summary (the
    // exposure class stays `weakly_exposed` — the conversion's variant
    // binding remains the typed `wrapper_error_binding_unresolved`
    // limitation per #3700).
    let target = owner_fn.map(|f| f.name.as_str()).unwrap_or("changed owner");
    let owner_anchored: Vec<&TestSummary> = related_tests
        .iter()
        .filter(|(_, reason)| *reason != RelationReason::SeamCalleeCall)
        .map(|(test, _)| *test)
        .collect();
    let callee_only: Vec<&TestSummary> = related_tests
        .iter()
        .filter(|(_, reason)| *reason == RelationReason::SeamCalleeCall)
        .map(|(test, _)| *test)
        .collect();
    let summary = if owner_anchored.is_empty() {
        let names = callee_only
            .iter()
            .take(3)
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "Related tests exercise the wrapper seam's converted callee (the changed owner is not invoked by them): {names}"
        )
    } else {
        let names = owner_anchored
            .iter()
            .take(3)
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        format!("Related tests appear to reach {target}: {names}")
    };
    StageEvidence::new(StageState::Yes, Confidence::Medium, summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::facts::FunctionSourceRole;
    use crate::domain::SymbolId;
    use std::path::PathBuf;

    #[test]
    fn given_no_related_tests_when_building_reach_evidence_then_stage_is_no() {
        let evidence = reach_evidence(&[], None);

        assert_eq!(evidence.state, StageState::No);
        assert_eq!(evidence.confidence, Confidence::Medium);
        assert_eq!(
            evidence.summary,
            "No static test path found for the changed owner"
        );
    }

    #[test]
    fn given_related_tests_when_building_reach_evidence_then_names_owner_and_tests() {
        let owner = function("discounted_total");
        let first = test("below_threshold");
        let second = test("at_threshold");
        let third = test("above_threshold");
        let fourth = test("large_amount");
        let related = vec![
            (&first, RelationReason::DirectOwnerCall),
            (&second, RelationReason::DirectOwnerCall),
            (&third, RelationReason::DirectOwnerCall),
            (&fourth, RelationReason::DirectOwnerCall),
        ];

        let evidence = reach_evidence(&related, Some(&owner));

        assert_eq!(evidence.state, StageState::Yes);
        assert_eq!(evidence.confidence, Confidence::Medium);
        assert_eq!(
            evidence.summary,
            "Related tests appear to reach discounted_total: below_threshold, at_threshold, above_threshold"
        );
    }

    // #3714 round-2 review (devin hDRL2): callee-only relations must not
    // claim owner reach in the summary.
    #[test]
    fn given_callee_only_relations_when_building_reach_evidence_then_summary_names_callee_affinity()
    {
        let owner = function("parse_summary");
        let first = test("observes_callee_outcome");
        let second = test("other_callee_probe");
        let related = vec![
            (&first, RelationReason::SeamCalleeCall),
            (&second, RelationReason::SeamCalleeCall),
        ];

        let evidence = reach_evidence(&related, Some(&owner));

        assert_eq!(evidence.state, StageState::Yes);
        assert_eq!(
            evidence.summary,
            "Related tests exercise the wrapper seam's converted callee (the changed owner is not invoked by them): observes_callee_outcome, other_callee_probe"
        );
    }

    // #3714 round-2 review (devin hDRL2): mixed relations keep the
    // established owner-reach phrasing for the owner-anchored tests.
    #[test]
    fn given_mixed_relations_when_building_reach_evidence_then_owner_reach_is_named() {
        let owner = function("parse_summary");
        let callee_only = test("observes_callee_outcome");
        let anchored = test("parse_summary_fails_closed");
        let related = vec![
            (&callee_only, RelationReason::SeamCalleeCall),
            (&anchored, RelationReason::OwnerNamedTest),
        ];

        let evidence = reach_evidence(&related, Some(&owner));

        assert_eq!(
            evidence.summary,
            "Related tests appear to reach parse_summary: parse_summary_fails_closed"
        );
    }

    fn function(name: &str) -> FunctionSummary {
        FunctionSummary {
            id: SymbolId(format!("src/lib.rs::{name}")),
            name: name.to_string(),
            file: PathBuf::from("src/lib.rs"),
            start_line: 1,
            end_line: 3,
            body: String::new(),
            calls: Vec::new(),
            returns: Vec::new(),
            literals: Vec::new(),
            source_role: FunctionSourceRole::Production,
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        }
    }

    fn test(name: &str) -> TestSummary {
        TestSummary {
            name: name.to_string(),
            file: PathBuf::from("tests/pricing.rs"),
            start_line: 1,
            end_line: 3,
            body: String::new(),
            calls: Vec::new(),
            assertions: Vec::new(),
            literals: Vec::new(),
            attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        }
    }
}
