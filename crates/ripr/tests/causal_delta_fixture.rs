use ripr::domain::{CanonicalEvidenceState, DeltaAttribution, compare_fixture_delta};

#[derive(serde::Deserialize)]
struct FixtureCase {
    name: String,
    base_available: bool,
    head_available: bool,
    base_state: Option<CanonicalEvidenceState>,
    head_state: Option<CanonicalEvidenceState>,
    expected: DeltaAttribution,
}

#[test]
fn comparison_rules_match_the_pr_a_fixture_corpus() -> Result<(), String> {
    let cases: Vec<FixtureCase> = serde_json::from_str(include_str!(
        "fixtures/causal-attribution/pr-a-comparison-rules.json"
    ))
    .map_err(|error| format!("parse PR A comparison fixture: {error}"))?;
    if cases.is_empty() {
        return Err("PR A comparison fixture is empty".to_string());
    }
    for case in cases {
        let delta = compare_fixture_delta(
            format!("gap:{}", case.name),
            case.base_available,
            case.head_available,
            case.base_state,
            case.head_state,
        );
        if delta.delta_attribution != case.expected {
            return Err(format!(
                "fixture {} expected {:?}, got {:?}",
                case.name, case.expected, delta.delta_attribution
            ));
        }
    }
    Ok(())
}
