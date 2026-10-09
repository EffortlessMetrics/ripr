use std::collections::BTreeSet;

use super::model::{
    CLAIM_BOUNDARY, GATE_KIND, GATE_SCHEMA_VERSION, GateInput, GateVerdict,
    PackageQualificationGate, RequiredRowResult, RowKey, RowStatus, STANDING_NON_CLAIMS,
    SelectionScope,
};

pub(crate) fn evaluate(input: &GateInput<'_>) -> PackageQualificationGate {
    let receipt = input.receipt;
    let mut failures = Vec::new();

    push_mismatch(
        &mut failures,
        "source.commit",
        input.expected.commit.as_deref(),
        &receipt.source.commit,
        "stale source commit",
    );
    push_mismatch(
        &mut failures,
        "source.tree",
        input.expected.tree.as_deref(),
        &receipt.source.tree,
        "stale source tree",
    );
    push_mismatch(
        &mut failures,
        "package.hash",
        input.expected.package_hash.as_deref(),
        &receipt.package.hash,
        "mismatched package hash",
    );
    push_mismatch(
        &mut failures,
        "payload.hash",
        input.expected.payload_hash.as_deref(),
        &receipt.payload.hash,
        "mismatched payload hash",
    );

    if receipt.subjects.selected == 0 || receipt.subjects.executed == 0 {
        failures.push("zero-subject receipt cannot pass".to_string());
    }
    if receipt.cleanup.status != RowStatus::Passed {
        failures.push(format!(
            "cleanup.status is {}; a pass requires cleanup passed",
            receipt.cleanup.status.as_str()
        ));
    }
    if receipt.executed_steps.is_empty() {
        failures.push("executed_steps must be nonempty for a pass".to_string());
    }

    if receipt.selection_scope == SelectionScope::DeclaredFullMatrix {
        let channels = unique_channels(&receipt.required_rows);
        let expected = full_matrix_keys(&channels, input.full_matrix_targets);
        let actual = receipt.required_rows.clone();
        if !same_row_set(&actual, &expected) {
            failures.push(
                "declared_full_matrix required_rows do not cover the complete channel/target matrix"
                    .to_string(),
            );
        }
        if input.full_matrix_targets.is_empty() {
            failures.push(
                "declared_full_matrix cannot be evaluated without an explicit target matrix"
                    .to_string(),
            );
        }
    }

    let mut required_row_results = Vec::new();
    for key in &receipt.required_rows {
        let row = receipt
            .rows
            .iter()
            .find(|row| row.channel == key.channel && row.target == key.target);
        let (status, subject_count, disposition) = match row {
            None => {
                failures.push(format!(
                    "required row {}/{} is missing",
                    key.channel.as_str(),
                    key.target
                ));
                (None, None, "missing".to_string())
            }
            Some(row) if row.status != RowStatus::Passed => {
                failures.push(format!(
                    "required row {}/{} is {}",
                    key.channel.as_str(),
                    key.target,
                    row.status.as_str()
                ));
                (
                    Some(row.status),
                    Some(row.subject_count),
                    row.status.as_str().to_string(),
                )
            }
            Some(row) if row.subject_count == 0 => {
                failures.push(format!(
                    "required row {}/{} is passed with zero subjects",
                    key.channel.as_str(),
                    key.target
                ));
                (
                    Some(row.status),
                    Some(row.subject_count),
                    "zero_subject".to_string(),
                )
            }
            Some(row) => {
                if row.executed_payload_hash.as_deref() != Some(receipt.payload.hash.as_str()) {
                    failures.push(format!(
                        "required row {}/{} executed a different payload than payload.hash",
                        key.channel.as_str(),
                        key.target
                    ));
                }
                (
                    Some(row.status),
                    Some(row.subject_count),
                    "passed".to_string(),
                )
            }
        };
        required_row_results.push(RequiredRowResult {
            channel: key.channel,
            target: key.target.clone(),
            status,
            subject_count,
            disposition,
        });
    }

    let verdict = if failures.is_empty() {
        GateVerdict::Passed
    } else {
        GateVerdict::Failed
    };

    PackageQualificationGate {
        schema_version: GATE_SCHEMA_VERSION.to_string(),
        kind: GATE_KIND.to_string(),
        verdict,
        source: receipt.source.clone(),
        package: receipt.package.clone(),
        payload: receipt.payload.clone(),
        selection_scope: receipt.selection_scope.clone(),
        subjects: receipt.subjects.clone(),
        required_row_results,
        failures,
        limitations: receipt.limitations.clone(),
        non_claims: STANDING_NON_CLAIMS
            .iter()
            .map(|item| (*item).to_string())
            .collect(),
        claim_boundary: CLAIM_BOUNDARY.to_string(),
    }
}

fn push_mismatch(
    failures: &mut Vec<String>,
    field: &str,
    expected: Option<&str>,
    actual: &str,
    label: &str,
) {
    if let Some(expected) = expected
        && expected != actual
    {
        failures.push(format!("{label} ({field})"));
    }
}

fn unique_channels(rows: &[RowKey]) -> Vec<super::model::Channel> {
    let mut seen = BTreeSet::new();
    let mut channels = Vec::new();
    for row in rows {
        if seen.insert(row.channel.as_str()) {
            channels.push(row.channel);
        }
    }
    channels
}

fn full_matrix_keys(channels: &[super::model::Channel], targets: &[String]) -> Vec<RowKey> {
    let mut keys = Vec::new();
    for channel in channels {
        for target in targets {
            keys.push(RowKey {
                channel: *channel,
                target: target.clone(),
            });
        }
    }
    keys
}

fn same_row_set(left: &[RowKey], right: &[RowKey]) -> bool {
    let pack = |rows: &[RowKey]| {
        rows.iter()
            .map(|row| format!("{}/{}", row.channel.as_str(), row.target))
            .collect::<BTreeSet<_>>()
    };
    pack(left) == pack(right)
}
