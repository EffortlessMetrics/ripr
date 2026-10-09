//! Repair-lifecycle rules. A fixture passing on a branch is not closure.

use super::schema::{FeedbackRow, is_analyzer_defect};

pub(crate) fn validate_row(row: &FeedbackRow) -> Vec<String> {
    let mut violations = Vec::new();
    if !is_analyzer_defect(&row.failure_direction) {
        return violations;
    }
    let merged = row
        .repair
        .merged_implementation
        .as_deref()
        .unwrap_or("")
        .trim();
    let replay_ids = row.repair.replay_identities.as_deref().unwrap_or("").trim();
    match row.status.as_str() {
        "closed_with_replay" => {
            if merged.is_empty() {
                violations.push(format!(
                    "{}: closed_with_replay requires a merged implementation identity",
                    row.case_id
                ));
            }
            if !row.repair.original_case_replay || replay_ids.is_empty() {
                violations.push(format!(
                    "{}: closed_with_replay requires original-case replay identities; a fixture pass without that replay stays repaired_pending_replay",
                    row.case_id
                ));
            }
            if row.failure_direction == "wrong_target" {
                match row.target_identity.as_deref() {
                    Some(target) if replay_ids == target => {}
                    _ => violations.push(format!(
                        "{}: original-case replay identity is not the exact target",
                        row.case_id
                    )),
                }
            }
        }
        "repaired_pending_replay" => {
            if merged.is_empty() {
                violations.push(format!(
                    "{}: repaired_pending_replay requires a merged implementation identity",
                    row.case_id
                ));
            }
            if row.repair.original_case_replay {
                violations.push(format!(
                    "{}: original-case replay is present; status cannot remain repaired_pending_replay",
                    row.case_id
                ));
            }
        }
        "open" | "candidate_in_review"
            if !merged.is_empty() && !row.repair.original_case_replay =>
        {
            violations.push(format!(
                "{}: merged repair without original-case replay must be repaired_pending_replay",
                row.case_id
            ));
        }
        _ => {}
    }
    violations
}
