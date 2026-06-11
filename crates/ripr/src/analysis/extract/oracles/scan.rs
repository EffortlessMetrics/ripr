use crate::analysis::facts::OracleFact;

use super::classify::classify_assertion;
use super::patterns::{
    is_custom_assertion_helper, is_mock_expectation_line, is_side_effect_observer_assertion,
    is_snapshot_assertion,
};
use crate::analysis::extract::text::extract_identifier_tokens;

pub(crate) fn extract_assertions(body: &str, start_line: usize) -> Vec<OracleFact> {
    let mut out = Vec::new();
    for (offset, line) in body.lines().enumerate() {
        let trimmed = line.trim();
        if is_assertion_line(trimmed) {
            let classification = classify_assertion(trimmed);
            out.push(OracleFact {
                line: start_line + offset,
                text: trimmed.to_string(),
                kind: classification.kind,
                strength: classification.strength,
                observed_tokens: extract_identifier_tokens(trimmed),
            });
        }
    }
    out
}

pub(crate) fn extract_line_scanned_oracles(body: &str, start_line: usize) -> Vec<OracleFact> {
    let mut out = Vec::new();
    for (offset, line) in body.lines().enumerate() {
        let trimmed = line.trim();
        if !is_line_scanned_oracle(trimmed) {
            continue;
        }
        let classification = classify_assertion(trimmed);
        out.push(OracleFact {
            line: start_line + offset,
            text: trimmed.to_string(),
            kind: classification.kind,
            strength: classification.strength,
            observed_tokens: extract_identifier_tokens(trimmed),
        });
    }
    out
}

fn is_assertion_line(line: &str) -> bool {
    line.contains("assert!")
        || line.contains("assert_eq!")
        || line.contains("assert_ne!")
        || line.contains("assert_matches!")
        || line.contains("matches!")
        || is_snapshot_assertion(line)
        || is_custom_assertion_helper(line)
        || is_side_effect_observer_assertion(line)
        || line.contains("expect_")
        || line.contains(".expect(")
        || line.contains(".unwrap(")
        || line.contains("should_panic")
}

fn is_line_scanned_oracle(line: &str) -> bool {
    is_custom_assertion_helper(line)
        || is_side_effect_observer_assertion(line)
        || is_mock_expectation_line(line)
}
