use super::super::rust_index::{FunctionSummary, TestSummary};
use crate::domain::{Confidence, StageEvidence, StageState};

pub(in crate::analysis) fn reach_evidence(
    related_tests: &[&TestSummary],
    owner_fn: Option<&FunctionSummary>,
) -> StageEvidence {
    if related_tests.is_empty() {
        StageEvidence::new(
            StageState::No,
            Confidence::Medium,
            "No static test path found for the changed owner",
        )
    } else {
        let target = owner_fn.map(|f| f.name.as_str()).unwrap_or("changed owner");
        let names = related_tests
            .iter()
            .take(3)
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        StageEvidence::new(
            StageState::Yes,
            Confidence::Medium,
            format!("Related tests appear to reach {target}: {names}"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let related = vec![&first, &second, &third, &fourth];

        let evidence = reach_evidence(&related, Some(&owner));

        assert_eq!(evidence.state, StageState::Yes);
        assert_eq!(evidence.confidence, Confidence::Medium);
        assert_eq!(
            evidence.summary,
            "Related tests appear to reach discounted_total: below_threshold, at_threshold, above_threshold"
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
            is_test: false,
            attrs: Vec::new(),
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
        }
    }
}
