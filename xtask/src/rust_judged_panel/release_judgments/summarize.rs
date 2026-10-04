//! Human scorecard derived from a validated judgment packet.

use std::collections::BTreeMap;

use super::{RELEASE_JUDGMENTS_PATH, ReleaseJudgments, TERMINALS};
use crate::rust_judged_panel::RELEASE_DIRECTIONS;

pub(super) fn summarize(packet: &ReleaseJudgments) -> String {
    let mut terminals: BTreeMap<&str, usize> = BTreeMap::new();
    let mut departing = Vec::new();
    let mut counts = [0usize; 4];
    let mut established = [0usize; 4];
    for row in &packet.judgments {
        *terminals.entry(row.terminal.as_str()).or_insert(0) += 1;
        if let Some(direction) = row.terminal.strip_prefix("confirmed_")
            && direction != row.expected_direction
        {
            departing.push(format!(
                "{} (expected {}, judged {direction})",
                row.case_id, row.expected_direction
            ));
        }
        let outcome = &row.reference_outcome;
        for (index, label) in [
            outcome.false_exposed,
            outcome.false_actionable,
            outcome.under_credit,
            outcome.limitation_correct,
        ]
        .into_iter()
        .enumerate()
        {
            if let Some(value) = label {
                established[index] += 1;
                if value {
                    counts[index] += 1;
                }
            }
        }
    }
    let mut body = format!(
        "# release-challenge judgments\n\npacket: {RELEASE_JUDGMENTS_PATH}\nselection: {} ({})\nrows: {}\n",
        packet.selection_path,
        packet.selection_sha256,
        packet.judgments.len()
    );
    for terminal in TERMINALS {
        body.push_str(&format!(
            "terminal {terminal}: {}\n",
            terminals.get(terminal).copied().unwrap_or(0)
        ));
    }
    for direction in RELEASE_DIRECTIONS {
        let judged = packet
            .judgments
            .iter()
            .filter(|row| row.expected_direction == direction)
            .count();
        body.push_str(&format!("expected {direction}: {judged}\n"));
    }
    for (index, name) in [
        "false_exposed",
        "false_actionable",
        "under_credit",
        "limitation_correct",
    ]
    .iter()
    .enumerate()
    {
        body.push_str(&format!(
            "reference {name}: {} true of {} established, {} rows\n",
            counts[index],
            established[index],
            packet.judgments.len()
        ));
    }
    for row in &departing {
        body.push_str(&format!("departs from expected: {row}\n"));
    }
    body.push_str(&format!(
        "reference run: {} ({})\n",
        packet.reference_run.analyzer, packet.reference_run.source
    ));
    body
}
