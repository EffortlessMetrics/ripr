//! Owner reuse and rival detection. The ledger records references; it never
//! mutates GitHub.

use std::collections::BTreeMap;

use super::schema::{FeedbackRow, is_analyzer_defect};

pub(crate) fn validate_row(row: &FeedbackRow) -> Vec<String> {
    let mut violations = Vec::new();
    if row.owner.search_receipt.trim().is_empty() {
        violations.push(format!("{}: owner search receipt is required", row.case_id));
    }
    if row.owner.designated.trim().is_empty() {
        violations.push(format!("{}: designated owner is required", row.case_id));
    }
    if row
        .owner
        .competing
        .iter()
        .any(|item| item == &row.owner.designated)
    {
        violations.push(format!(
            "{}: competing owners cannot include the designated owner",
            row.case_id
        ));
    }
    if is_analyzer_defect(&row.failure_direction)
        && row.owner.designated == "unowned_no_github_mutation"
        && row.owner.existing.is_some()
    {
        violations.push(format!(
            "{}: existing owner must be reused rather than left unowned",
            row.case_id
        ));
    }
    if is_analyzer_defect(&row.failure_direction) && row.owner.designated == "none" {
        violations.push(format!(
            "{}: confirmed analyzer defect requires an owner or an explicit `unowned_no_github_mutation` designation; `none` hides it from the ownership counts",
            row.case_id
        ));
    }
    if let Some(existing) = &row.owner.existing
        && is_analyzer_defect(&row.failure_direction)
        && row.owner.designated != existing.as_str()
        && row.owner.designated != "unowned_no_github_mutation"
    {
        violations.push(format!(
            "{}: existing focused owner `{existing}` must remain designated (found `{}`); listing it under `competing` does not record a supersession",
            row.case_id, row.owner.designated
        ));
    }
    violations
}

pub(crate) fn validate_duplicates(rows: &[FeedbackRow]) -> Vec<String> {
    let mut by_mechanism: BTreeMap<&str, Vec<&FeedbackRow>> = BTreeMap::new();
    for row in rows {
        if is_analyzer_defect(&row.failure_direction) {
            by_mechanism
                .entry(row.mechanism.as_str())
                .or_default()
                .push(row);
        }
    }
    let mut violations = Vec::new();
    for (mechanism, group) in by_mechanism {
        let mut owners = group
            .iter()
            .map(|row| row.owner.designated.as_str())
            .collect::<Vec<_>>();
        owners.sort_unstable();
        owners.dedup();
        if owners.len() > 1
            && owners
                .iter()
                .any(|owner| *owner != "unowned_no_github_mutation")
        {
            violations.push(format!(
                "mechanism `{mechanism}` has rival designated owners ({}); reuse one focused owner",
                owners.join(", ")
            ));
        }
    }
    violations
}
