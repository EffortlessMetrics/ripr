//! Render repo-local review guidance outcome receipts.

use serde_json::json;

pub(crate) const REVIEW_FEEDBACK_SCHEMA_VERSION: &str = "0.1";
pub(crate) const REVIEW_FEEDBACK_LIMITS_NOTE: &str = "Repo-local review feedback only; no telemetry, generated tests, source edits, mutation execution, or CI blocking.";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ReviewFeedbackOutcome {
    Useful,
    Noisy,
    WrongLine,
    AlreadyCovered,
    WrongTarget,
    SummaryOnlyCorrect,
    SuppressedCorrectly,
    MissingRecommendation,
}

impl ReviewFeedbackOutcome {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "useful" => Ok(Self::Useful),
            "noisy" => Ok(Self::Noisy),
            "wrong_line" => Ok(Self::WrongLine),
            "already_covered" => Ok(Self::AlreadyCovered),
            "wrong_target" => Ok(Self::WrongTarget),
            "summary_only_correct" => Ok(Self::SummaryOnlyCorrect),
            "suppressed_correctly" => Ok(Self::SuppressedCorrectly),
            "missing_recommendation" => Ok(Self::MissingRecommendation),
            _ => Err(format!(
                "unknown review-feedback outcome {value:?}; expected {}",
                Self::supported_values().join(", ")
            )),
        }
    }

    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Useful => "useful",
            Self::Noisy => "noisy",
            Self::WrongLine => "wrong_line",
            Self::AlreadyCovered => "already_covered",
            Self::WrongTarget => "wrong_target",
            Self::SummaryOnlyCorrect => "summary_only_correct",
            Self::SuppressedCorrectly => "suppressed_correctly",
            Self::MissingRecommendation => "missing_recommendation",
        }
    }

    pub(crate) fn supported_values() -> &'static [&'static str] {
        &[
            "useful",
            "noisy",
            "wrong_line",
            "already_covered",
            "wrong_target",
            "summary_only_correct",
            "suppressed_correctly",
            "missing_recommendation",
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ReviewFeedbackSource {
    HumanReview,
    AgentReview,
    FixtureExpectation,
    MaintainerImport,
}

impl ReviewFeedbackSource {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "human_review" => Ok(Self::HumanReview),
            "agent_review" => Ok(Self::AgentReview),
            "fixture_expectation" => Ok(Self::FixtureExpectation),
            "maintainer_import" => Ok(Self::MaintainerImport),
            _ => Err(format!(
                "unknown review-feedback source {value:?}; expected human_review, agent_review, fixture_expectation, or maintainer_import"
            )),
        }
    }

    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::HumanReview => "human_review",
            Self::AgentReview => "agent_review",
            Self::FixtureExpectation => "fixture_expectation",
            Self::MaintainerImport => "maintainer_import",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ReviewFeedbackReceiptInput {
    pub(crate) root: String,
    pub(crate) source: ReviewFeedbackSource,
    pub(crate) outcome: ReviewFeedbackOutcome,
    pub(crate) recommendation_id: Option<String>,
    pub(crate) comment_id: Option<String>,
    pub(crate) seam_id: Option<String>,
    pub(crate) expected_test_file: Option<String>,
    pub(crate) actual_test_file: Option<String>,
    pub(crate) reason: Option<String>,
    pub(crate) recorded_unix_ms: Option<u64>,
}

pub(crate) fn render_review_feedback_receipt_json(
    input: &ReviewFeedbackReceiptInput,
) -> Result<String, String> {
    let value = json!({
        "schema_version": REVIEW_FEEDBACK_SCHEMA_VERSION,
        "tool": "ripr",
        "status": "advisory",
        "root": input.root.as_str(),
        "source": input.source.as_str(),
        "recommendation_id": input.recommendation_id.as_deref(),
        "comment_id": input.comment_id.as_deref(),
        "seam_id": input.seam_id.as_deref(),
        "outcome": input.outcome.as_str(),
        "expected": {
            "test_file": input.expected_test_file.as_deref(),
        },
        "actual": {
            "test_file": input.actual_test_file.as_deref(),
        },
        "reason": input.reason.as_deref(),
        "recorded_unix_ms": input.recorded_unix_ms,
        "limits_note": REVIEW_FEEDBACK_LIMITS_NOTE,
    });

    serde_json::to_string_pretty(&value)
        .map(|mut rendered| {
            rendered.push('\n');
            rendered
        })
        .map_err(|err| format!("failed to render review feedback receipt JSON: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt_input(outcome: ReviewFeedbackOutcome) -> ReviewFeedbackReceiptInput {
        ReviewFeedbackReceiptInput {
            root: ".".to_string(),
            source: ReviewFeedbackSource::HumanReview,
            outcome,
            recommendation_id: Some("ripr-review-67fc764ba37d77bd".to_string()),
            comment_id: Some("comment-1".to_string()),
            seam_id: Some("67fc764ba37d77bd".to_string()),
            expected_test_file: Some("tests/pricing.rs".to_string()),
            actual_test_file: Some("tests/pricing.rs".to_string()),
            reason: Some("Reviewer accepted the focused test request.".to_string()),
            recorded_unix_ms: Some(1_778_240_000_000),
        }
    }

    #[test]
    fn review_feedback_outcome_parses_supported_values() -> Result<(), String> {
        for value in ReviewFeedbackOutcome::supported_values() {
            let parsed = ReviewFeedbackOutcome::parse(value)
                .map_err(|err| format!("parse {value}: {err}"))?;
            assert_eq!(parsed.as_str(), *value);
        }
        Ok(())
    }

    #[test]
    fn review_feedback_outcome_rejects_unknown_values() {
        assert_eq!(
            ReviewFeedbackOutcome::parse("blocked"),
            Err("unknown review-feedback outcome \"blocked\"; expected useful, noisy, wrong_line, already_covered, wrong_target, summary_only_correct, suppressed_correctly, missing_recommendation".to_string())
        );
    }

    #[test]
    fn review_feedback_receipt_json_is_advisory_and_structured() -> Result<(), String> {
        let rendered =
            render_review_feedback_receipt_json(&receipt_input(ReviewFeedbackOutcome::Useful))?;
        let value: serde_json::Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("receipt JSON should parse: {err}"))?;

        assert_eq!(value["schema_version"], REVIEW_FEEDBACK_SCHEMA_VERSION);
        assert_eq!(value["tool"], "ripr");
        assert_eq!(value["status"], "advisory");
        assert_eq!(value["source"], "human_review");
        assert_eq!(value["outcome"], "useful");
        assert_eq!(value["recommendation_id"], "ripr-review-67fc764ba37d77bd");
        assert_eq!(value["seam_id"], "67fc764ba37d77bd");
        assert_eq!(value["expected"]["test_file"], "tests/pricing.rs");
        assert_eq!(value["actual"]["test_file"], "tests/pricing.rs");
        assert_eq!(value["recorded_unix_ms"], 1_778_240_000_000_u64);
        assert!(
            value["limits_note"]
                .as_str()
                .unwrap_or_default()
                .contains("no telemetry")
        );
        Ok(())
    }

    #[test]
    fn review_feedback_receipt_json_allows_missing_recommendation() -> Result<(), String> {
        let mut input = receipt_input(ReviewFeedbackOutcome::MissingRecommendation);
        input.recommendation_id = None;
        input.comment_id = None;
        input.reason = Some("Expected PR guidance did not appear for this seam.".to_string());

        let rendered = render_review_feedback_receipt_json(&input)?;
        let value: serde_json::Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("receipt JSON should parse: {err}"))?;
        assert_eq!(value["outcome"], "missing_recommendation");
        assert!(value["recommendation_id"].is_null());
        assert_eq!(value["seam_id"], "67fc764ba37d77bd");
        Ok(())
    }
}
