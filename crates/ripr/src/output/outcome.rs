//! Render the targeted-test before/after outcome receipt.
//!
//! `ripr outcome` compares two previously rendered `repo-exposure-json`
//! artifacts. It does not run analysis or mutation testing; it only reports
//! whether static seam evidence moved after a focused test change.

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub(crate) const TARGETED_TEST_OUTCOME_SCHEMA_VERSION: &str = "0.1";
pub(crate) const AGENT_VERIFY_SCHEMA_VERSION: &str = "0.1";

const SEAM_GRIP_CLASS_ORDER: &[&str] = &[
    "strongly_gripped",
    "weakly_gripped",
    "ungripped",
    "reachable_unrevealed",
    "activation_unknown",
    "propagation_unknown",
    "observation_unknown",
    "discrimination_unknown",
    "opaque",
    "intentional",
    "suppressed",
];

const EVIDENCE_STAGES: &[&str] = &["reach", "activate", "propagate", "observe", "discriminate"];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StaticSeamRecord {
    seam_id: String,
    seam_kind: String,
    file: String,
    line: usize,
    seam_grip_class: String,
    oracle_kind: String,
    oracle_strength: String,
    observed_values: Vec<String>,
    missing_discriminators: Vec<String>,
    evidence_source: String,
    evidence_path: BTreeMap<String, StaticEvidenceStage>,
    related_tests_total: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StaticEvidenceStage {
    state: String,
    confidence: String,
    summary: String,
}

struct TargetedOutcomeEvidenceDelta<'a> {
    stage_deltas: [&'a Option<TargetedTestOutcomeStageDelta>; 5],
    observed_values_added: &'a [String],
    observed_values_removed: &'a [String],
    missing_discriminators_resolved: &'a [String],
    missing_discriminators_reopened: &'a [String],
    oracle_strength_delta: Option<&'a str>,
    related_test_delta: isize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TargetedTestOutcomeReport {
    before_path: String,
    after_path: String,
    before_counts: BTreeMap<String, usize>,
    after_counts: BTreeMap<String, usize>,
    moved: Vec<TargetedTestOutcomeMovement>,
    unchanged: Vec<TargetedTestOutcomeMovement>,
    regressed: Vec<TargetedTestOutcomeMovement>,
    new: Vec<TargetedTestOutcomeSeam>,
    removed: Vec<TargetedTestOutcomeSeam>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TargetedTestOutcomeMovement {
    seam_id: String,
    seam_kind: String,
    file: String,
    line: usize,
    before: String,
    after: String,
    direction: String,
    evidence_delta: Vec<String>,
    evidence_source: String,
    reach_delta: Option<TargetedTestOutcomeStageDelta>,
    activate_delta: Option<TargetedTestOutcomeStageDelta>,
    propagate_delta: Option<TargetedTestOutcomeStageDelta>,
    observe_delta: Option<TargetedTestOutcomeStageDelta>,
    discriminate_delta: Option<TargetedTestOutcomeStageDelta>,
    observed_values_added: Vec<String>,
    observed_values_removed: Vec<String>,
    missing_discriminators_resolved: Vec<String>,
    missing_discriminators_reopened: Vec<String>,
    oracle_strength_delta: Option<String>,
    related_test_delta: isize,
    no_movement_reason: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TargetedTestOutcomeStageDelta {
    before_state: Option<String>,
    after_state: Option<String>,
    before_confidence: Option<String>,
    after_confidence: Option<String>,
    before_summary: Option<String>,
    after_summary: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TargetedTestOutcomeSeam {
    seam_id: String,
    seam_kind: String,
    file: String,
    line: usize,
    grip_class: String,
}

pub(crate) fn targeted_test_outcome_report_from_json(
    before_json: &str,
    after_json: &str,
    before_path: String,
    after_path: String,
) -> Result<TargetedTestOutcomeReport, String> {
    let before = parse_repo_exposure_static_seams(before_json)?;
    let after = parse_repo_exposure_static_seams(after_json)?;
    build_targeted_test_outcome_report(&before, &after, before_path, after_path)
}

pub(crate) fn render_targeted_test_outcome_json(
    report: &TargetedTestOutcomeReport,
) -> Result<String, String> {
    let value = serde_json::json!({
        "schema_version": TARGETED_TEST_OUTCOME_SCHEMA_VERSION,
        "tool": "ripr",
        "status": "advisory",
        "inputs": {
            "before": report.before_path.as_str(),
            "after": report.after_path.as_str()
        },
        "before": report.before_counts,
        "after": report.after_counts,
        "summary": {
            "moved": report.moved.len(),
            "unchanged": report.unchanged.len(),
            "regressed": report.regressed.len(),
            "new": report.new.len(),
            "removed": report.removed.len()
        },
        "moved": report.moved.iter().map(targeted_test_outcome_movement_json).collect::<Vec<_>>(),
        "unchanged": report.unchanged.iter().map(targeted_test_outcome_movement_json).collect::<Vec<_>>(),
        "regressed": report.regressed.iter().map(targeted_test_outcome_movement_json).collect::<Vec<_>>(),
        "new": report.new.iter().map(targeted_test_outcome_seam_json).collect::<Vec<_>>(),
        "removed": report.removed.iter().map(targeted_test_outcome_seam_json).collect::<Vec<_>>()
    });
    super::json::render_pretty_with_newline(&value, "targeted-test outcome")
}

pub(crate) fn render_agent_verify_json(
    report: &TargetedTestOutcomeReport,
) -> Result<String, String> {
    let improved = report
        .moved
        .iter()
        .filter(|movement| movement.direction == "improved")
        .count();
    let changed = report
        .moved
        .iter()
        .filter(|movement| movement.direction != "improved")
        .count();
    let changed_seams = report
        .moved
        .iter()
        .chain(report.regressed.iter())
        .map(agent_verify_movement_json)
        .collect::<Vec<_>>();

    let value = serde_json::json!({
        "schema_version": AGENT_VERIFY_SCHEMA_VERSION,
        "tool": "ripr",
        "status": "advisory",
        "inputs": {
            "before": report.before_path.as_str(),
            "after": report.after_path.as_str()
        },
        "summary": {
            "improved": improved,
            "changed": changed,
            "regressed": report.regressed.len(),
            "unchanged": report.unchanged.len(),
            "new": report.new.len(),
            "resolved": report.removed.len()
        },
        "changed_seams": changed_seams,
        "unchanged_seams": report.unchanged.iter().map(agent_verify_movement_json).collect::<Vec<_>>(),
        "new_gaps": report.new.iter().map(|seam| agent_verify_seam_json(seam, "new")).collect::<Vec<_>>(),
        "resolved_gaps": report.removed.iter().map(|seam| agent_verify_seam_json(seam, "resolved")).collect::<Vec<_>>()
    });
    super::json::render_pretty_with_newline(&value, "agent verify")
}

pub(crate) fn render_targeted_test_outcome_md(report: &TargetedTestOutcomeReport) -> String {
    let mut out = String::new();
    out.push_str("# ripr targeted-test outcome report\n\n");
    out.push_str("Status: advisory\n\n");
    out.push_str("Inputs:\n");
    out.push_str(&format!("- before: `{}`\n", md_escape(&report.before_path)));
    out.push_str(&format!("- after: `{}`\n\n", md_escape(&report.after_path)));

    out.push_str("## Summary\n\n");
    out.push_str("| Bucket | Count |\n| --- | ---: |\n");
    out.push_str(&format!("| moved | {} |\n", report.moved.len()));
    out.push_str(&format!("| unchanged | {} |\n", report.unchanged.len()));
    out.push_str(&format!("| regressed | {} |\n", report.regressed.len()));
    out.push_str(&format!("| new | {} |\n", report.new.len()));
    out.push_str(&format!("| removed | {} |\n", report.removed.len()));

    out.push_str("\n## Grip Counts\n\n");
    out.push_str("| Class | Before | After |\n| --- | ---: | ---: |\n");
    for class in std::iter::once("seams_total").chain(SEAM_GRIP_CLASS_ORDER.iter().copied()) {
        out.push_str(&format!(
            "| {} | {} | {} |\n",
            class,
            count_for_class(&report.before_counts, class),
            count_for_class(&report.after_counts, class)
        ));
    }

    push_targeted_outcome_movements_md(&mut out, "Moved", &report.moved);
    push_targeted_outcome_movements_md(&mut out, "Unchanged", &report.unchanged);
    push_targeted_outcome_movements_md(&mut out, "Regressed", &report.regressed);
    push_targeted_outcome_seams_md(&mut out, "New", &report.new);
    push_targeted_outcome_seams_md(&mut out, "Removed", &report.removed);
    out.push_str(
        "\nThis report compares two static repo-exposure snapshots. It is advisory and does not run mutation testing.\n",
    );
    out
}

pub(crate) fn display_path(path: &Path) -> String {
    normalize_report_path(&path.display().to_string())
}

fn count_for_class(counts: &BTreeMap<String, usize>, class: &str) -> usize {
    match counts.get(class) {
        Some(count) => *count,
        None => 0,
    }
}

fn parse_repo_exposure_static_seams(json: &str) -> Result<Vec<StaticSeamRecord>, String> {
    let value: Value = serde_json::from_str(json)
        .map_err(|err| format!("failed to parse repo exposure JSON: {err}"))?;
    let seams = value
        .get("seams")
        .and_then(Value::as_array)
        .ok_or_else(|| "repo exposure JSON is missing `seams` array".to_string())?;

    let mut records = Vec::new();
    for seam in seams {
        let evidence_record = seam
            .get("evidence_record")
            .filter(|value| value.is_object());
        let location = evidence_record
            .and_then(|record| record.get("location"))
            .filter(|value| value.is_object());
        let seam_id = optional_json_string(evidence_record, "seam_id")
            .or_else(|| optional_json_string(Some(seam), "seam_id"))
            .ok_or_else(|| "repo exposure seam is missing string field `seam_id`".to_string())?;
        let seam_kind = optional_json_string(evidence_record, "seam_kind")
            .or_else(|| optional_json_string(Some(seam), "kind"))
            .ok_or_else(|| "repo exposure seam is missing string field `kind`".to_string())?;
        let file = optional_json_string(location, "file")
            .or_else(|| optional_json_string(Some(seam), "file"))
            .map(|path| normalize_report_path(&path))
            .ok_or_else(|| "repo exposure seam is missing string field `file`".to_string())?;
        let line = optional_json_usize(location, "line")
            .or_else(|| optional_json_usize(Some(seam), "line"))
            .ok_or_else(|| "repo exposure seam is missing numeric field `line`".to_string())?;
        let seam_grip_class = optional_json_string(evidence_record, "grip_class")
            .or_else(|| optional_json_string(Some(seam), "grip_class"))
            .ok_or_else(|| "repo exposure seam is missing string field `grip_class`".to_string())?;
        let oracle_source = match evidence_record {
            Some(record) if record.get("related_tests").is_some() => record,
            _ => seam,
        };
        let (oracle_kind, oracle_strength) = strongest_related_oracle(oracle_source);
        records.push(StaticSeamRecord {
            seam_id,
            seam_kind,
            file,
            line,
            seam_grip_class,
            oracle_kind,
            oracle_strength,
            observed_values: evidence_record_values_or_legacy(
                evidence_record,
                seam,
                "observed_values",
                observed_value_strings,
            ),
            missing_discriminators: evidence_record_values_or_legacy(
                evidence_record,
                seam,
                "missing_discriminators",
                missing_discriminator_strings,
            ),
            evidence_source: if evidence_record.is_some() {
                "evidence_record".to_string()
            } else {
                "legacy_fields".to_string()
            },
            evidence_path: evidence_path_stages(evidence_record),
            related_tests_total: related_tests_total(evidence_record, seam),
        });
    }
    Ok(records)
}

fn build_targeted_test_outcome_report(
    before: &[StaticSeamRecord],
    after: &[StaticSeamRecord],
    before_path: String,
    after_path: String,
) -> Result<TargetedTestOutcomeReport, String> {
    let before_by_id = targeted_outcome_seams_by_id(before, "before")?;
    let after_by_id = targeted_outcome_seams_by_id(after, "after")?;
    let mut moved = Vec::new();
    let mut unchanged = Vec::new();
    let mut regressed = Vec::new();
    let mut removed = Vec::new();

    for (seam_id, before_seam) in &before_by_id {
        match after_by_id.get(seam_id) {
            Some(after_seam) => {
                let movement = targeted_test_outcome_movement(before_seam, after_seam);
                if movement.before == movement.after {
                    unchanged.push(movement);
                } else if targeted_outcome_grip_rank(&movement.after)
                    < targeted_outcome_grip_rank(&movement.before)
                {
                    regressed.push(movement);
                } else {
                    moved.push(movement);
                }
            }
            None => removed.push(targeted_test_outcome_seam(before_seam)),
        }
    }

    let mut new = Vec::new();
    for (seam_id, after_seam) in &after_by_id {
        if !before_by_id.contains_key(seam_id) {
            new.push(targeted_test_outcome_seam(after_seam));
        }
    }

    Ok(TargetedTestOutcomeReport {
        before_path,
        after_path,
        before_counts: targeted_outcome_class_counts(before),
        after_counts: targeted_outcome_class_counts(after),
        moved,
        unchanged,
        regressed,
        new,
        removed,
    })
}

fn targeted_outcome_seams_by_id(
    seams: &[StaticSeamRecord],
    label: &str,
) -> Result<BTreeMap<String, StaticSeamRecord>, String> {
    let mut out = BTreeMap::new();
    for seam in seams {
        if out.insert(seam.seam_id.clone(), seam.clone()).is_some() {
            return Err(format!(
                "{label} repo exposure JSON contains duplicate seam_id `{}`",
                seam.seam_id
            ));
        }
    }
    Ok(out)
}

fn targeted_outcome_class_counts(seams: &[StaticSeamRecord]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    counts.insert("seams_total".to_string(), seams.len());
    for class in SEAM_GRIP_CLASS_ORDER {
        counts.insert((*class).to_string(), 0);
    }
    for seam in seams {
        *counts.entry(seam.seam_grip_class.clone()).or_insert(0) += 1;
    }
    counts
}

fn targeted_test_outcome_movement(
    before: &StaticSeamRecord,
    after: &StaticSeamRecord,
) -> TargetedTestOutcomeMovement {
    let before_rank = targeted_outcome_grip_rank(&before.seam_grip_class);
    let after_rank = targeted_outcome_grip_rank(&after.seam_grip_class);
    let direction = if before.seam_grip_class == after.seam_grip_class {
        "unchanged"
    } else if after_rank > before_rank {
        "improved"
    } else if after_rank < before_rank {
        "regressed"
    } else {
        "changed"
    };
    let evidence_source = movement_evidence_source(before, after);
    let reach_delta = stage_delta(before, after, "reach");
    let activate_delta = stage_delta(before, after, "activate");
    let propagate_delta = stage_delta(before, after, "propagate");
    let observe_delta = stage_delta(before, after, "observe");
    let discriminate_delta = stage_delta(before, after, "discriminate");
    let observed_values_added =
        string_values_added(&before.observed_values, &after.observed_values);
    let observed_values_removed =
        string_values_removed(&before.observed_values, &after.observed_values);
    let missing_discriminators_resolved = string_values_removed(
        &before.missing_discriminators,
        &after.missing_discriminators,
    );
    let missing_discriminators_reopened = string_values_added(
        &before.missing_discriminators,
        &after.missing_discriminators,
    );
    let oracle_strength_delta = oracle_strength_delta(before, after);
    let related_test_delta = related_test_delta(before, after);
    let delta_inputs = TargetedOutcomeEvidenceDelta {
        stage_deltas: [
            &reach_delta,
            &activate_delta,
            &propagate_delta,
            &observe_delta,
            &discriminate_delta,
        ],
        observed_values_added: &observed_values_added,
        observed_values_removed: &observed_values_removed,
        missing_discriminators_resolved: &missing_discriminators_resolved,
        missing_discriminators_reopened: &missing_discriminators_reopened,
        oracle_strength_delta: oracle_strength_delta.as_deref(),
        related_test_delta,
    };
    let evidence_delta = targeted_outcome_evidence_delta(before, after, &delta_inputs);
    let no_movement_reason = no_movement_reason(direction, &evidence_delta, &evidence_source);
    TargetedTestOutcomeMovement {
        seam_id: before.seam_id.clone(),
        seam_kind: before.seam_kind.clone(),
        file: before.file.clone(),
        line: before.line,
        before: before.seam_grip_class.clone(),
        after: after.seam_grip_class.clone(),
        direction: direction.to_string(),
        evidence_delta,
        evidence_source,
        reach_delta,
        activate_delta,
        propagate_delta,
        observe_delta,
        discriminate_delta,
        observed_values_added,
        observed_values_removed,
        missing_discriminators_resolved,
        missing_discriminators_reopened,
        oracle_strength_delta,
        related_test_delta,
        no_movement_reason,
    }
}

fn targeted_test_outcome_seam(seam: &StaticSeamRecord) -> TargetedTestOutcomeSeam {
    TargetedTestOutcomeSeam {
        seam_id: seam.seam_id.clone(),
        seam_kind: seam.seam_kind.clone(),
        file: seam.file.clone(),
        line: seam.line,
        grip_class: seam.seam_grip_class.clone(),
    }
}

fn targeted_outcome_grip_rank(class: &str) -> u8 {
    match class {
        "strongly_gripped" | "intentional" | "suppressed" => 7,
        "weakly_gripped" => 5,
        "reachable_unrevealed" => 4,
        "activation_unknown"
        | "propagation_unknown"
        | "observation_unknown"
        | "discrimination_unknown" => 3,
        "opaque" => 2,
        "ungripped" => 1,
        _ => 0,
    }
}

fn targeted_outcome_evidence_delta(
    before: &StaticSeamRecord,
    after: &StaticSeamRecord,
    delta: &TargetedOutcomeEvidenceDelta<'_>,
) -> Vec<String> {
    let mut deltas = Vec::new();
    if before.seam_grip_class != after.seam_grip_class {
        deltas.push(format!(
            "grip class moved from {} to {}",
            before.seam_grip_class, after.seam_grip_class
        ));
    }

    for (stage, stage_delta) in EVIDENCE_STAGES.iter().zip(delta.stage_deltas.iter()) {
        if let Some(stage_delta) = stage_delta {
            deltas.push(format!(
                "{} evidence moved from {} to {}",
                stage,
                optional_delta_value(stage_delta.before_state.as_deref()),
                optional_delta_value(stage_delta.after_state.as_deref())
            ));
        }
    }

    for value in delta.missing_discriminators_resolved {
        deltas.push(format!(
            "missing discriminator no longer reported: {}",
            md_escape(value)
        ));
    }
    for value in delta.missing_discriminators_reopened {
        deltas.push(format!(
            "new missing discriminator reported: {}",
            md_escape(value)
        ));
    }

    for value in delta.observed_values_added {
        deltas.push(format!("new observed value: {}", md_escape(value)));
    }
    for value in delta.observed_values_removed {
        deltas.push(format!(
            "previous observed value absent: {}",
            md_escape(value)
        ));
    }

    if let Some(oracle_delta) = delta.oracle_strength_delta {
        if oracle_strength_rank(&after.oracle_strength)
            > oracle_strength_rank(&before.oracle_strength)
        {
            deltas.push(format!("stronger related oracle visible: {oracle_delta}"));
        } else {
            deltas.push(format!("related oracle strength decreased: {oracle_delta}"));
        }
    }
    if before.oracle_kind != after.oracle_kind && before.oracle_strength == after.oracle_strength {
        deltas.push(format!(
            "related oracle kind changed: {} -> {}",
            before.oracle_kind, after.oracle_kind
        ));
    }
    match delta.related_test_delta.cmp(&0) {
        std::cmp::Ordering::Greater => {
            deltas.push(format!(
                "related test count increased by {}",
                delta.related_test_delta
            ));
        }
        std::cmp::Ordering::Less => {
            deltas.push(format!(
                "related test count decreased by {}",
                delta.related_test_delta.abs()
            ));
        }
        std::cmp::Ordering::Equal => {}
    }

    if deltas.is_empty() && before.seam_grip_class != after.seam_grip_class {
        deltas.push("grip class changed without rendered evidence details".to_string());
    }
    deltas
}

fn targeted_test_outcome_movement_json(movement: &TargetedTestOutcomeMovement) -> Value {
    serde_json::json!({
        "seam_id": movement.seam_id.as_str(),
        "seam_kind": movement.seam_kind.as_str(),
        "file": movement.file.as_str(),
        "line": movement.line,
        "before": movement.before.as_str(),
        "after": movement.after.as_str(),
        "direction": movement.direction.as_str(),
        "evidence_delta": movement.evidence_delta,
        "evidence_source": movement.evidence_source.as_str(),
        "reach_delta": movement.reach_delta.as_ref().map(stage_delta_json),
        "activate_delta": movement.activate_delta.as_ref().map(stage_delta_json),
        "propagate_delta": movement.propagate_delta.as_ref().map(stage_delta_json),
        "observe_delta": movement.observe_delta.as_ref().map(stage_delta_json),
        "discriminate_delta": movement.discriminate_delta.as_ref().map(stage_delta_json),
        "observed_values_added": movement.observed_values_added,
        "observed_values_removed": movement.observed_values_removed,
        "missing_discriminators_resolved": movement.missing_discriminators_resolved,
        "missing_discriminators_reopened": movement.missing_discriminators_reopened,
        "oracle_strength_delta": movement.oracle_strength_delta.as_deref(),
        "related_test_delta": movement.related_test_delta,
        "no_movement_reason": movement.no_movement_reason.as_deref()
    })
}

fn targeted_test_outcome_seam_json(seam: &TargetedTestOutcomeSeam) -> Value {
    serde_json::json!({
        "seam_id": seam.seam_id.as_str(),
        "seam_kind": seam.seam_kind.as_str(),
        "file": seam.file.as_str(),
        "line": seam.line,
        "grip_class": seam.grip_class.as_str()
    })
}

fn agent_verify_movement_json(movement: &TargetedTestOutcomeMovement) -> Value {
    serde_json::json!({
        "seam_id": movement.seam_id.as_str(),
        "seam_kind": movement.seam_kind.as_str(),
        "file": movement.file.as_str(),
        "line": movement.line,
        "before": movement.before.as_str(),
        "after": movement.after.as_str(),
        "change": movement.direction.as_str(),
        "evidence_delta": movement.evidence_delta,
        "evidence_source": movement.evidence_source.as_str(),
        "reach_delta": movement.reach_delta.as_ref().map(stage_delta_json),
        "activate_delta": movement.activate_delta.as_ref().map(stage_delta_json),
        "propagate_delta": movement.propagate_delta.as_ref().map(stage_delta_json),
        "observe_delta": movement.observe_delta.as_ref().map(stage_delta_json),
        "discriminate_delta": movement.discriminate_delta.as_ref().map(stage_delta_json),
        "observed_values_added": movement.observed_values_added,
        "observed_values_removed": movement.observed_values_removed,
        "missing_discriminators_resolved": movement.missing_discriminators_resolved,
        "missing_discriminators_reopened": movement.missing_discriminators_reopened,
        "oracle_strength_delta": movement.oracle_strength_delta.as_deref(),
        "related_test_delta": movement.related_test_delta,
        "no_movement_reason": movement.no_movement_reason.as_deref()
    })
}

fn agent_verify_seam_json(seam: &TargetedTestOutcomeSeam, change: &str) -> Value {
    serde_json::json!({
        "seam_id": seam.seam_id.as_str(),
        "seam_kind": seam.seam_kind.as_str(),
        "file": seam.file.as_str(),
        "line": seam.line,
        "grip_class": seam.grip_class.as_str(),
        "change": change
    })
}

fn push_targeted_outcome_movements_md(
    out: &mut String,
    title: &str,
    movements: &[TargetedTestOutcomeMovement],
) {
    out.push_str(&format!("\n## {title}\n\n"));
    if movements.is_empty() {
        out.push_str("None.\n");
        return;
    }
    for movement in movements {
        out.push_str(&format!(
            "- `{}` {}:{} {} -> {} ({})\n",
            md_escape(&movement.seam_id),
            md_escape(&movement.file),
            movement.line,
            movement.before,
            movement.after,
            movement.direction
        ));
        for delta in &movement.evidence_delta {
            out.push_str(&format!("  - {}\n", md_escape(delta)));
        }
        if movement.evidence_delta.is_empty()
            && let Some(reason) = &movement.no_movement_reason
        {
            out.push_str(&format!("  - no movement: {}\n", md_escape(reason)));
        }
    }
}

fn push_targeted_outcome_seams_md(
    out: &mut String,
    title: &str,
    seams: &[TargetedTestOutcomeSeam],
) {
    out.push_str(&format!("\n## {title}\n\n"));
    if seams.is_empty() {
        out.push_str("None.\n");
        return;
    }
    for seam in seams {
        out.push_str(&format!(
            "- `{}` {}:{} {} ({})\n",
            md_escape(&seam.seam_id),
            md_escape(&seam.file),
            seam.line,
            seam.grip_class,
            seam.seam_kind
        ));
    }
}

fn optional_json_string(value: Option<&Value>, key: &str) -> Option<String> {
    value?.get(key).and_then(json_scalar_as_string)
}

fn optional_json_usize(value: Option<&Value>, key: &str) -> Option<usize> {
    value?.get(key).and_then(json_scalar_as_usize)
}

fn strongest_related_oracle(seam: &Value) -> (String, String) {
    let mut best_kind = "unknown".to_string();
    let mut best_strength = "unknown".to_string();
    let mut best_rank = 0;

    if let Some(related) = seam.get("related_tests").and_then(Value::as_array) {
        for test in related {
            let strength = test
                .get("oracle_strength")
                .and_then(Value::as_str)
                .map_or("unknown", |strength| strength);
            let rank = oracle_strength_rank(strength);
            if rank > best_rank {
                best_rank = rank;
                best_strength = strength.to_string();
                best_kind = test
                    .get("oracle_kind")
                    .and_then(Value::as_str)
                    .map_or("unknown", |kind| kind)
                    .to_string();
            }
        }
    }

    (best_kind, best_strength)
}

fn oracle_strength_rank(strength: &str) -> u8 {
    match strength {
        "strong" => 5,
        "medium" => 4,
        "weak" => 3,
        "smoke" => 2,
        "none" => 1,
        _ => 0,
    }
}

fn evidence_record_values_or_legacy(
    evidence_record: Option<&Value>,
    seam: &Value,
    key: &str,
    parser: fn(&Value) -> Vec<String>,
) -> Vec<String> {
    if let Some(record) = evidence_record.filter(|record| record.get(key).is_some()) {
        parser(record)
    } else {
        parser(seam)
    }
}

fn observed_value_strings(seam: &Value) -> Vec<String> {
    match seam.get("observed_values").and_then(Value::as_array) {
        Some(items) => items
            .iter()
            .filter_map(|item| {
                json_scalar_as_string(item)
                    .or_else(|| item.get("value").and_then(json_scalar_as_string))
            })
            .collect::<Vec<_>>(),
        None => Vec::new(),
    }
}

fn missing_discriminator_strings(seam: &Value) -> Vec<String> {
    match seam.get("missing_discriminators").and_then(Value::as_array) {
        Some(items) => items
            .iter()
            .filter_map(|item| {
                if let Some(value) = json_scalar_as_string(item) {
                    return Some(value);
                }
                let value = item.get("value").and_then(json_scalar_as_string)?;
                match item.get("reason").and_then(json_scalar_as_string) {
                    Some(reason) if !reason.is_empty() => Some(format!("{value} ({reason})")),
                    _ => Some(value),
                }
            })
            .collect::<Vec<_>>(),
        None => Vec::new(),
    }
}

fn related_tests_total(evidence_record: Option<&Value>, seam: &Value) -> usize {
    let source = match evidence_record {
        Some(record) if record.get("related_tests_total").is_some() => record,
        _ => seam,
    };
    if let Some(total) = source
        .get("related_tests_total")
        .and_then(json_scalar_as_usize)
    {
        return total;
    }
    match source.get("related_tests").and_then(Value::as_array) {
        Some(related_tests) => related_tests.len(),
        None => 0,
    }
}

fn evidence_path_stages(evidence_record: Option<&Value>) -> BTreeMap<String, StaticEvidenceStage> {
    let mut stages = BTreeMap::new();
    let Some(path) = evidence_record
        .and_then(|record| record.get("evidence_path"))
        .and_then(Value::as_object)
    else {
        return stages;
    };
    for stage in EVIDENCE_STAGES {
        let Some(value) = path.get(*stage) else {
            continue;
        };
        stages.insert(
            (*stage).to_string(),
            StaticEvidenceStage {
                state: optional_json_string_or_empty(Some(value), "state"),
                confidence: optional_json_string_or_empty(Some(value), "confidence"),
                summary: optional_json_string_or_empty(Some(value), "summary"),
            },
        );
    }
    stages
}

fn optional_json_string_or_empty(value: Option<&Value>, key: &str) -> String {
    let mut text = String::new();
    if let Some(value) = optional_json_string(value, key) {
        text = value;
    }
    text
}

fn movement_evidence_source(before: &StaticSeamRecord, after: &StaticSeamRecord) -> String {
    if before.evidence_source == after.evidence_source {
        before.evidence_source.clone()
    } else {
        format!("{} -> {}", before.evidence_source, after.evidence_source)
    }
}

fn stage_delta(
    before: &StaticSeamRecord,
    after: &StaticSeamRecord,
    stage: &str,
) -> Option<TargetedTestOutcomeStageDelta> {
    let before_stage = before.evidence_path.get(stage);
    let after_stage = after.evidence_path.get(stage);
    if before_stage == after_stage {
        return None;
    }
    if before_stage.is_none() && after_stage.is_none() {
        return None;
    }
    Some(TargetedTestOutcomeStageDelta {
        before_state: before_stage.map(|stage| stage.state.clone()),
        after_state: after_stage.map(|stage| stage.state.clone()),
        before_confidence: before_stage.map(|stage| stage.confidence.clone()),
        after_confidence: after_stage.map(|stage| stage.confidence.clone()),
        before_summary: before_stage.map(|stage| stage.summary.clone()),
        after_summary: after_stage.map(|stage| stage.summary.clone()),
    })
}

fn stage_delta_json(delta: &TargetedTestOutcomeStageDelta) -> Value {
    serde_json::json!({
        "before_state": delta.before_state.as_deref(),
        "after_state": delta.after_state.as_deref(),
        "before_confidence": delta.before_confidence.as_deref(),
        "after_confidence": delta.after_confidence.as_deref(),
        "before_summary": delta.before_summary.as_deref(),
        "after_summary": delta.after_summary.as_deref(),
    })
}

fn string_values_added(before: &[String], after: &[String]) -> Vec<String> {
    let before_values = before.iter().collect::<BTreeSet<_>>();
    let after_values = after.iter().collect::<BTreeSet<_>>();
    after_values
        .difference(&before_values)
        .map(|value| (*value).clone())
        .collect()
}

fn string_values_removed(before: &[String], after: &[String]) -> Vec<String> {
    let before_values = before.iter().collect::<BTreeSet<_>>();
    let after_values = after.iter().collect::<BTreeSet<_>>();
    before_values
        .difference(&after_values)
        .map(|value| (*value).clone())
        .collect()
}

fn oracle_strength_delta(before: &StaticSeamRecord, after: &StaticSeamRecord) -> Option<String> {
    (before.oracle_strength != after.oracle_strength)
        .then(|| format!("{} -> {}", before.oracle_strength, after.oracle_strength))
}

fn related_test_delta(before: &StaticSeamRecord, after: &StaticSeamRecord) -> isize {
    match (
        isize::try_from(after.related_tests_total),
        isize::try_from(before.related_tests_total),
    ) {
        (Ok(after_total), Ok(before_total)) => after_total - before_total,
        _ => 0,
    }
}

fn no_movement_reason(
    direction: &str,
    evidence_delta: &[String],
    evidence_source: &str,
) -> Option<String> {
    (direction == "unchanged" && evidence_delta.is_empty())
        .then(|| format!("grip class and {evidence_source} evidence were unchanged"))
}

fn optional_delta_value(value: Option<&str>) -> &str {
    match value {
        Some(text) if !text.is_empty() => text,
        _ => "missing",
    }
}

fn json_scalar_as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

fn json_scalar_as_usize(value: &Value) -> Option<usize> {
    match value {
        Value::Number(number) => number
            .as_u64()
            .and_then(|value| usize::try_from(value).ok()),
        Value::String(text) => text.trim().parse::<usize>().ok(),
        _ => None,
    }
}

fn normalize_report_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    match normalized.strip_prefix("./") {
        Some(stripped) => stripped.to_string(),
        None => normalized,
    }
}

fn md_escape(value: &str) -> String {
    value.replace('`', "\\`").replace(['\r', '\n'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targeted_test_outcome_report_buckets_seam_movement() -> Result<(), String> {
        let mut before_moved = targeted_static_seam("seam-moved", "weakly_gripped");
        before_moved.missing_discriminators = vec!["threshold equality".to_string()];
        before_moved.oracle_strength = "weak".to_string();
        let before = vec![
            before_moved,
            targeted_static_seam("seam-regressed", "weakly_gripped"),
            targeted_static_seam("seam-same", "strongly_gripped"),
            targeted_static_seam("seam-removed", "ungripped"),
        ];

        let mut after_moved = targeted_static_seam("seam-moved", "strongly_gripped");
        after_moved.observed_values = vec!["50".to_string(), "100".to_string()];
        after_moved.oracle_strength = "strong".to_string();
        let after = vec![
            after_moved,
            targeted_static_seam("seam-regressed", "ungripped"),
            targeted_static_seam("seam-same", "strongly_gripped"),
            targeted_static_seam("seam-new", "weakly_gripped"),
        ];

        let report = build_targeted_test_outcome_report(
            &before,
            &after,
            "before.json".to_string(),
            "after.json".to_string(),
        )?;
        assert_eq!(report.moved.len(), 1);
        assert_eq!(report.moved[0].seam_id, "seam-moved");
        assert_eq!(report.moved[0].direction, "improved");
        assert!(
            report.moved[0]
                .evidence_delta
                .iter()
                .any(|delta| delta.contains("missing discriminator no longer reported"))
        );
        assert!(
            report.moved[0]
                .evidence_delta
                .iter()
                .any(|delta| delta.contains("stronger related oracle visible"))
        );
        assert_eq!(report.regressed.len(), 1);
        assert_eq!(report.unchanged.len(), 1);
        assert_eq!(report.new.len(), 1);
        assert_eq!(report.removed.len(), 1);
        assert_eq!(report.before_counts.get("weakly_gripped"), Some(&2));
        assert_eq!(report.after_counts.get("strongly_gripped"), Some(&2));
        Ok(())
    }

    #[test]
    fn targeted_test_outcome_json_and_markdown_are_structured() -> Result<(), String> {
        let before = vec![
            targeted_static_seam("seam-a", "weakly_gripped"),
            targeted_static_seam("seam-same", "weakly_gripped"),
        ];
        let mut after_same = targeted_static_seam("seam-same", "weakly_gripped");
        after_same.observed_values = vec!["50".to_string(), "100".to_string()];
        let after = vec![
            targeted_static_seam("seam-a", "strongly_gripped"),
            after_same,
        ];
        let report = build_targeted_test_outcome_report(
            &before,
            &after,
            "target/ripr/before.json".to_string(),
            "target/ripr/after.json".to_string(),
        )?;

        let json = render_targeted_test_outcome_json(&report)?;
        let value: Value = serde_json::from_str(&json)
            .map_err(|err| format!("targeted-test outcome JSON should parse: {err}"))?;
        assert_eq!(
            value["schema_version"],
            TARGETED_TEST_OUTCOME_SCHEMA_VERSION
        );
        assert_eq!(value["status"], "advisory");
        assert_eq!(value["summary"]["moved"], 1);

        let markdown = render_targeted_test_outcome_md(&report);
        assert!(markdown.contains("# ripr targeted-test outcome report"));
        assert!(markdown.contains("| moved | 1 |"));
        assert!(markdown.contains("## Unchanged"));
        assert!(markdown.contains("seam-same"));
        assert!(markdown.contains("new observed value: 100"));
        assert!(markdown.contains("weakly_gripped -> strongly_gripped"));
        Ok(())
    }

    #[test]
    fn agent_verify_json_maps_outcome_to_agent_status_buckets() -> Result<(), String> {
        let before = vec![
            targeted_static_seam("improved", "weakly_gripped"),
            targeted_static_seam("regressed", "weakly_gripped"),
            targeted_static_seam("unchanged", "weakly_gripped"),
            targeted_static_seam("resolved", "ungripped"),
        ];
        let after = vec![
            targeted_static_seam("improved", "strongly_gripped"),
            targeted_static_seam("regressed", "ungripped"),
            targeted_static_seam("unchanged", "weakly_gripped"),
            targeted_static_seam("new", "weakly_gripped"),
        ];
        let report = build_targeted_test_outcome_report(
            &before,
            &after,
            "before.json".to_string(),
            "after.json".to_string(),
        )?;

        let json = render_agent_verify_json(&report)?;
        let value: Value = serde_json::from_str(&json)
            .map_err(|err| format!("agent verify JSON should parse: {err}"))?;
        assert_eq!(value["schema_version"], AGENT_VERIFY_SCHEMA_VERSION);
        assert_eq!(value["status"], "advisory");
        assert_eq!(value["summary"]["improved"], 1);
        assert_eq!(value["summary"]["regressed"], 1);
        assert_eq!(value["summary"]["unchanged"], 1);
        assert_eq!(value["summary"]["new"], 1);
        assert_eq!(value["summary"]["resolved"], 1);
        assert_eq!(value["changed_seams"][0]["change"], "improved");
        assert_eq!(value["resolved_gaps"][0]["change"], "resolved");
        Ok(())
    }

    #[test]
    fn targeted_test_outcome_from_repo_exposure_json_parses_static_evidence() -> Result<(), String>
    {
        let before = r#"{
  "schema_version": "0.2",
  "scope": "repo",
  "seams": [
    {
      "seam_id": "seam-a",
      "kind": "predicate_boundary",
      "file": ".\\src\\pricing.rs",
      "line": 42,
      "grip_class": "weakly_gripped",
      "related_tests": [
        {"oracle_kind": "exact_value", "oracle_strength": "weak"}
      ],
      "observed_values": ["50"],
      "missing_discriminators": [
        {"value": "threshold equality", "reason": "not observed"}
      ]
    }
  ]
}"#;
        let after = r#"{
  "schema_version": "0.2",
  "scope": "repo",
  "seams": [
    {
      "seam_id": "seam-a",
      "kind": "predicate_boundary",
      "file": "src/pricing.rs",
      "line": 42,
      "grip_class": "strongly_gripped",
      "related_tests": [
        {"oracle_kind": "exact_value", "oracle_strength": "strong"}
      ],
      "observed_values": ["50", "100"],
      "missing_discriminators": []
    }
  ]
}"#;
        let report = targeted_test_outcome_report_from_json(
            before,
            after,
            "before.json".to_string(),
            "after.json".to_string(),
        )?;
        assert_eq!(report.moved.len(), 1);
        assert_eq!(report.moved[0].file, "src/pricing.rs");
        assert!(
            report.moved[0]
                .evidence_delta
                .iter()
                .any(|delta| delta.contains("threshold equality"))
        );
        Ok(())
    }

    #[test]
    fn targeted_test_outcome_prefers_evidence_record_movement() -> Result<(), String> {
        let before = r#"{
  "schema_version": "0.3",
  "scope": "repo",
  "seams": [
    {
      "seam_id": "seam-a",
      "kind": "legacy_kind",
      "file": "legacy.rs",
      "line": 7,
      "grip_class": "ungripped",
      "related_tests": [],
      "observed_values": ["legacy-only"],
      "missing_discriminators": ["legacy missing"],
      "evidence_record": {
        "schema_version": "0.1",
        "seam_id": "seam-a",
        "location": {"file": ".\\src\\pricing.rs", "line": 42},
        "seam_kind": "predicate_boundary",
        "grip_class": "weakly_gripped",
        "evidence_path": {
          "reach": {"state": "yes", "confidence": "high", "summary": "owner reached"},
          "activate": {"state": "yes", "confidence": "high", "summary": "above boundary covered"},
          "propagate": {"state": "yes", "confidence": "medium", "summary": "return value flows"},
          "observe": {"state": "weak", "confidence": "medium", "summary": "weak assertion"},
          "discriminate": {"state": "missing", "confidence": "high", "summary": "equality not asserted"}
        },
        "observed_values": [{"value": "50", "line": 9, "text": "discounted_total(50)", "context": "function_argument"}],
        "missing_discriminators": [{"value": "threshold equality", "reason": "not observed"}],
        "related_tests_total": 1,
        "related_tests": [{"oracle_kind": "exact_value", "oracle_strength": "weak"}]
      }
    }
  ]
}"#;
        let after = r#"{
  "schema_version": "0.3",
  "scope": "repo",
  "seams": [
    {
      "seam_id": "seam-a",
      "kind": "legacy_kind",
      "file": "legacy.rs",
      "line": 7,
      "grip_class": "ungripped",
      "related_tests": [],
      "observed_values": ["legacy-only"],
      "missing_discriminators": ["legacy missing"],
      "evidence_record": {
        "schema_version": "0.1",
        "seam_id": "seam-a",
        "location": {"file": "src/pricing.rs", "line": 42},
        "seam_kind": "predicate_boundary",
        "grip_class": "strongly_gripped",
        "evidence_path": {
          "reach": {"state": "yes", "confidence": "high", "summary": "owner reached"},
          "activate": {"state": "yes", "confidence": "high", "summary": "equality covered"},
          "propagate": {"state": "yes", "confidence": "medium", "summary": "return value flows"},
          "observe": {"state": "yes", "confidence": "high", "summary": "exact assertion"},
          "discriminate": {"state": "yes", "confidence": "high", "summary": "equality asserted"}
        },
        "observed_values": [
          {"value": "50", "line": 9, "text": "discounted_total(50)", "context": "function_argument"},
          {"value": "100", "line": 10, "text": "discounted_total(100)", "context": "function_argument"}
        ],
        "missing_discriminators": [],
        "related_tests_total": 2,
        "related_tests": [{"oracle_kind": "exact_value", "oracle_strength": "strong"}]
      }
    }
  ]
}"#;
        let report = targeted_test_outcome_report_from_json(
            before,
            after,
            "before.json".to_string(),
            "after.json".to_string(),
        )?;

        assert_eq!(report.moved.len(), 1);
        let movement = &report.moved[0];
        assert_eq!(movement.seam_kind, "predicate_boundary");
        assert_eq!(movement.file, "src/pricing.rs");
        assert_eq!(movement.line, 42);
        assert_eq!(movement.before, "weakly_gripped");
        assert_eq!(movement.after, "strongly_gripped");
        assert_eq!(movement.evidence_source, "evidence_record");
        assert_eq!(movement.observed_values_added, vec!["100".to_string()]);
        assert_eq!(
            movement.missing_discriminators_resolved,
            vec!["threshold equality (not observed)".to_string()]
        );
        assert_eq!(
            movement.oracle_strength_delta,
            Some("weak -> strong".to_string())
        );
        assert_eq!(movement.related_test_delta, 1);
        assert_eq!(
            movement
                .discriminate_delta
                .as_ref()
                .and_then(|delta| delta.before_state.as_deref()),
            Some("missing")
        );
        assert_eq!(
            movement
                .discriminate_delta
                .as_ref()
                .and_then(|delta| delta.after_state.as_deref()),
            Some("yes")
        );

        let json = render_targeted_test_outcome_json(&report)?;
        let value: Value = serde_json::from_str(&json)
            .map_err(|err| format!("targeted-test outcome JSON should parse: {err}"))?;
        assert_eq!(value["moved"][0]["evidence_source"], "evidence_record");
        assert_eq!(value["moved"][0]["observed_values_added"][0], "100");
        assert_eq!(
            value["moved"][0]["missing_discriminators_resolved"][0],
            "threshold equality (not observed)"
        );
        assert_eq!(value["moved"][0]["oracle_strength_delta"], "weak -> strong");
        assert_eq!(value["moved"][0]["related_test_delta"], 1);
        assert_eq!(
            value["moved"][0]["discriminate_delta"]["before_state"],
            "missing"
        );
        assert_eq!(
            value["moved"][0]["discriminate_delta"]["after_state"],
            "yes"
        );
        Ok(())
    }

    #[test]
    fn targeted_test_outcome_records_no_movement_reason() {
        let seam = targeted_static_seam("same", "weakly_gripped");
        let movement = targeted_test_outcome_movement(&seam, &seam);
        assert_eq!(movement.direction, "unchanged");
        assert_eq!(
            movement.no_movement_reason.as_deref(),
            Some("grip class and legacy_fields evidence were unchanged")
        );
    }

    #[test]
    fn targeted_test_outcome_rejects_duplicate_seam_ids() {
        let seam = targeted_static_seam("same", "weakly_gripped");
        let result = build_targeted_test_outcome_report(
            &[seam.clone(), seam],
            &[],
            "before.json".to_string(),
            "after.json".to_string(),
        );
        assert!(matches!(result, Err(message) if message.contains("duplicate seam_id `same`")));
    }

    #[test]
    fn targeted_test_outcome_reports_non_class_delta_branches() {
        let mut before = targeted_static_seam("same-rank", "activation_unknown");
        before.missing_discriminators = vec!["new missing later".to_string()];
        before.observed_values = vec!["old".to_string()];
        before.oracle_kind = "exact_value".to_string();
        before.oracle_strength = "strong".to_string();
        let mut after = targeted_static_seam("same-rank", "propagation_unknown");
        after.missing_discriminators = vec!["different missing now".to_string()];
        after.oracle_kind = "error_variant".to_string();
        after.oracle_strength = "weak".to_string();

        let movement = targeted_test_outcome_movement(&before, &after);
        assert_eq!(movement.direction, "changed");
        assert!(
            movement
                .evidence_delta
                .iter()
                .any(|delta| delta.contains("new missing discriminator reported"))
        );
        assert!(
            movement
                .evidence_delta
                .iter()
                .any(|delta| delta.contains("previous observed value absent"))
        );
        assert!(
            movement
                .evidence_delta
                .iter()
                .any(|delta| delta.contains("related oracle strength decreased"))
        );

        let mut before_kind = targeted_static_seam("same-kind-rank", "weakly_gripped");
        before_kind.oracle_kind = "exact_value".to_string();
        before_kind.oracle_strength = "medium".to_string();
        let mut after_kind = before_kind.clone();
        after_kind.oracle_kind = "custom_helper".to_string();
        let kind_movement = targeted_test_outcome_movement(&before_kind, &after_kind);
        assert!(
            kind_movement
                .evidence_delta
                .iter()
                .any(|delta| delta.contains("related oracle kind changed"))
        );
    }

    #[test]
    fn targeted_test_outcome_json_and_markdown_render_new_and_removed() -> Result<(), String> {
        let before = vec![targeted_static_seam("removed", "weakly_gripped")];
        let after = vec![targeted_static_seam("new", "ungripped")];
        let report = build_targeted_test_outcome_report(
            &before,
            &after,
            "before.json".to_string(),
            "after.json".to_string(),
        )?;

        let json = render_targeted_test_outcome_json(&report)?;
        assert!(json.contains(r#""removed""#));
        assert!(json.contains(r#""new""#));
        assert!(json.contains(r#""grip_class": "ungripped""#));

        let markdown = render_targeted_test_outcome_md(&report);
        assert!(markdown.contains("## New"));
        assert!(markdown.contains("`new` src/pricing.rs:42 ungripped"));
        assert!(markdown.contains("## Removed"));
        assert!(markdown.contains("`removed` src/pricing.rs:42 weakly_gripped"));
        Ok(())
    }

    #[test]
    fn targeted_test_outcome_parser_handles_scalar_fallbacks_and_empty_inputs() -> Result<(), String>
    {
        let before = r#"{
  "schema_version": "0.2",
  "scope": "repo",
  "seams": [
    {
      "seam_id": 7,
      "kind": "predicate_boundary",
      "file": "./src/pricing.rs",
      "line": "42",
      "grip_class": "weakly_gripped",
      "related_tests": [],
      "observed_values": [50, true],
      "missing_discriminators": [
        "plain missing",
        {"value": "value only", "reason": ""}
      ]
    }
  ]
}"#;
        let after = r#"{
  "schema_version": "0.2",
  "scope": "repo",
  "seams": []
}"#;
        let report = targeted_test_outcome_report_from_json(
            before,
            after,
            "before.json".to_string(),
            "after.json".to_string(),
        )?;
        assert_eq!(report.removed.len(), 1);
        assert_eq!(report.removed[0].seam_id, "7");
        assert_eq!(report.removed[0].file, "src/pricing.rs");
        Ok(())
    }

    #[test]
    fn targeted_test_outcome_rejects_missing_required_fields() {
        let result = targeted_test_outcome_report_from_json(
            r#"{"seams":[{"seam_id":"missing-kind"}]}"#,
            r#"{"seams":[]}"#,
            "before.json".to_string(),
            "after.json".to_string(),
        );
        assert!(matches!(result, Err(message) if message.contains("missing string field `kind`")));
    }

    fn targeted_static_seam(id: &str, grip_class: &str) -> StaticSeamRecord {
        StaticSeamRecord {
            seam_id: id.to_string(),
            seam_kind: "predicate_boundary".to_string(),
            file: "src/pricing.rs".to_string(),
            line: 42,
            seam_grip_class: grip_class.to_string(),
            oracle_kind: "exact_value".to_string(),
            oracle_strength: "unknown".to_string(),
            observed_values: Vec::new(),
            missing_discriminators: Vec::new(),
            evidence_source: "legacy_fields".to_string(),
            evidence_path: BTreeMap::new(),
            related_tests_total: 0,
        }
    }
}
