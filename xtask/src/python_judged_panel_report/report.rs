//! Derivation of the deterministic report Value: one typed pass over the
//! validated inventory, replay records, and adjudication records
//! (RIPR-SPEC-0092).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::Serialize;
use serde_json::{Value, json};

use crate::branch_inventory::parse_rfc3339_epoch_seconds;
use crate::python_judged_panel::{
    KNOWN_LIMITATION_QUALITIES, PythonJudgedPanelItem, RowKind, direction_admits_error,
    load_validated_inventory, row_kind,
};
use crate::python_judged_panel_replay::{sha256_hex, stable_case_slug};

use super::adjudication::{must_not_claim_echo, read_adjudication_records, row_revision_sha256};
use super::judgment_semantics::{JudgmentSemantics, validate_judgment_semantics};
use super::publish::render_markdown;
use super::replay_records::{RecordAnchor, read_replay_records};
use super::threshold::evaluate_threshold_policy;
use super::view::{AdjudicationState, AxisValue, bool_token, derive_adjudication_view};
use super::{
    AUTHORITY_BOUNDARY, FALSE_ACTIONABLE_BOUNDARY, FALSE_EXPOSED_BOUNDARY, NOTE_NO_COMBINED_SCORE,
    NOTE_NO_INHERITED_DENOMINATOR, NOTE_RELATION_BASIS, NOTE_REPLAY_ADVISORY,
    NOTE_STALE_DEFINITION, RECORD_SCHEMA_VERSION, REPORT_KIND, REPORT_SCHEMA_VERSION, SPEC,
};

// ---------------------------------------------------------------------------
// Derivation: one typed pass over inventory + records + adjudications
// ---------------------------------------------------------------------------

/// The complete rendered report: `json` and `markdown` render from the same
/// derived Value, so the two surfaces can never disagree.
pub(crate) struct RenderedReport {
    pub(crate) json: String,
    pub(crate) markdown: String,
}

/// One error axis's rate with its full provenance. `rate` is omitted (never a
/// fake zero) when the denominator is zero.
#[derive(Default)]
pub(super) struct ErrorRate {
    pub(super) numerator: usize,
    pub(super) denominator: usize,
    pub(super) undecided: usize,
    pub(super) coverage_boundary: String,
    pub(super) denominator_case_ids: Vec<String>,
    pub(super) binary_version: Option<String>,
    pub(super) binary_sha256: Option<String>,
    /// FIX f2TMb: where the as-of identity came from — the denominator
    /// cases' own replay records, a disclosure that no common identity
    /// exists across them, or no denominator at all.
    pub(super) as_of_basis: String,
    pub(super) rate: Option<f64>,
}

/// One quality axis's flagged/assessed accounting over adjudicated rows.
#[derive(Serialize, Default)]
struct AxisCounts {
    flagged: usize,
    assessed: usize,
    unassessed: usize,
    disputed_axis: usize,
}

/// `records_display` / `adjudications_display` are the path strings echoed in
/// the report (the CLI passes the same string it resolves); tests pass a
/// stable display while pointing at different physical directories so the
/// determinism proof can compare two independent replay runs.
pub(crate) fn build_report_at(
    root: &Path,
    displays: &[&str],
    records_dir: &Path,
    records_display: &str,
    adjudications_dir: &Path,
    adjudications_display: &str,
    policy_path: Option<&str>,
) -> Result<RenderedReport, String> {
    let loaded = load_validated_inventory(root, displays)?;

    let mut inventory_identity = Vec::new();
    for display in displays {
        let bytes = fs::read(root.join(display))
            .map_err(|error| format!("read panel envelope `{display}` for identity: {error}"))?;
        inventory_identity.push(json!({"path": display, "sha256": sha256_hex(&bytes)}));
    }
    inventory_identity.sort_by(|left, right| {
        left["path"]
            .as_str()
            .unwrap_or_default()
            .cmp(right["path"].as_str().unwrap_or_default())
    });
    let panel_digest = {
        let mut digest_source = String::new();
        for identity in &inventory_identity {
            digest_source.push_str(identity["path"].as_str().unwrap_or_default());
            digest_source.push('\u{0}');
            digest_source.push_str(identity["sha256"].as_str().unwrap_or_default());
            digest_source.push('\n');
        }
        sha256_hex(digest_source.as_bytes())
    };

    let (records, binary_identity) = read_replay_records(records_dir)?;
    let adjudications = read_adjudication_records(adjudications_dir)?;

    // Per-case views, in stable case-id order.
    let mut rows: Vec<(&str, &PythonJudgedPanelItem)> = Vec::new();
    for file in &loaded {
        for item in &file.envelope.items {
            rows.push((file.display.as_str(), item));
        }
    }
    rows.sort_by(|left, right| left.1.id.cmp(&right.1.id));

    // A record naming a case outside the validated inventory is stale data,
    // not report input: fail closed so the operator re-runs replay.
    for (case_id, view) in &records {
        if !rows.iter().any(|(_, item)| &item.id == case_id) {
            return Err(format!(
                "replay record `{}` names case `{case_id}` which is not in the validated inventory; re-run `cargo xtask python-judged-panel replay` to refresh the record set",
                view.file_name
            ));
        }
    }
    for case_id in adjudications.keys() {
        if !rows.iter().any(|(_, item)| item.id == *case_id) {
            return Err(format!(
                "adjudication record names case `{case_id}` which is not in the validated inventory; re-record or remove it"
            ));
        }
    }

    let mut counts = BTreeMap::from([
        ("selected", rows.len()),
        ("replayed", 0),
        ("not_run", 0),
        ("adjudicated", 0),
        ("pending_second_role", 0),
        ("disputed", 0),
        ("inconclusive", 0),
        ("stale_row", 0),
        ("stale", 0),
        ("anchor_stale", 0),
        ("mismatched", 0),
        ("comparison_unavailable", 0),
        ("no_replay_record", 0),
    ]);
    let cover =
        |map: &mut BTreeMap<String, [usize; 3]>, key: &str, replayed: bool, adjudicated: bool| {
            let cell = map.entry(key.to_string()).or_insert([0, 0, 0]);
            cell[0] += 1;
            cell[1] += usize::from(replayed);
            cell[2] += usize::from(adjudicated);
        };
    let (mut by_direction, mut by_repository, mut by_family) =
        (BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
    let (mut by_oracle, mut by_limit) = (BTreeMap::new(), BTreeMap::new());
    let mut cases = Vec::new();
    let mut false_actionable = ErrorRate {
        coverage_boundary: FALSE_ACTIONABLE_BOUNDARY.to_string(),
        ..ErrorRate::default()
    };
    let mut false_exposed = ErrorRate {
        coverage_boundary: FALSE_EXPOSED_BOUNDARY.to_string(),
        ..ErrorRate::default()
    };
    let mut wrong_target = AxisCounts::default();
    let mut invalid_command = AxisCounts::default();
    let mut limitation_correctness: BTreeMap<String, usize> = BTreeMap::new();
    // FIX fqNy (devin round 4): per case, the rate as-of identity this run
    // may cite — `None` when the case has no replay record or its record is
    // not current against the present diff/row kind, so a stale replay can
    // never label current adjudications with a binary that never replayed
    // this revision.
    let mut rate_identity: BTreeMap<String, Option<(String, String)>> = BTreeMap::new();
    for key in KNOWN_LIMITATION_QUALITIES
        .iter()
        .map(|quality| quality.to_string())
        .chain([
            "undecided".to_string(),
            "disputed_axis".to_string(),
            "not_adjudicated".to_string(),
        ])
    {
        limitation_correctness.insert(key, 0);
    }

    for (source_envelope, item) in &rows {
        let case_id = &item.id;
        let row_revision = row_revision_sha256(root, item)?;
        let replay_view = records.get(case_id);
        // FIX f2TIz: carryover rows (null expected_classification,
        // robustness-only) are excluded defensively — no stored adjudication
        // can enter their counts, denominators, quality tallies, or
        // thresholds, and `adjudicate` refuses to create one.
        let is_carryover = row_kind(item) == RowKind::Carryover;
        let adjudication_record = if is_carryover {
            None
        } else {
            adjudications.get(case_id)
        };
        let adjudication_view = adjudication_record.map(|record| {
            // FIX f2XZU: every stored judgment must satisfy the same
            // semantic rules the CLI enforces; a violation fails the report
            // named per case and judgment, it is never silently excluded.
            let file_name = format!("{}.json", stable_case_slug(case_id));
            // FIX fqNa (devin round 4): the stored provenance echoes must
            // match the current validated row — a hand-edited or drifted
            // echo is a contradiction, never preservable report input.
            if record.source_envelope != *source_envelope
                || record.expected_direction != item.expected_direction
                || record.must_not_claim != must_not_claim_echo(item)
            {
                return Err(format!(
                    "adjudication record `{file_name}` case `{case_id}`: stored provenance contradicts the current validated row (envelope `{}` vs `{}`, direction `{}` vs `{}`); re-record the adjudication against the current row",
                    record.source_envelope, source_envelope,
                    record.expected_direction, item.expected_direction
                ));
            }
            for judgment in &record.judgments {
                // FIX fqNa: stored provenance must carry a real instant.
                if parse_rfc3339_epoch_seconds(&judgment.recorded_at).is_err() {
                    return Err(format!(
                        "adjudication record `{file_name}` case `{case_id}` judgment by role `{}`: recorded_at `{}` is not a parseable RFC 3339 timestamp",
                        judgment.reviewer_role, judgment.recorded_at
                    ));
                }
                if let Err(violation) = validate_judgment_semantics(JudgmentSemantics {
                    role: &judgment.reviewer_role,
                    identity: &judgment.reviewer_identity,
                    verdict: &judgment.verdict,
                    false_actionable: judgment.false_actionable,
                    false_exposed: judgment.false_exposed,
                    evidence_references: &judgment.evidence_references,
                    limitation_quality: judgment.limitation_quality.as_deref(),
                    direction: &item.expected_direction,
                    carryover: false,
                }) {
                    return Err(format!(
                        "adjudication record `{file_name}` case `{case_id}` judgment by role `{}`: {violation}",
                        judgment.reviewer_role
                    ));
                }
            }
            let mut seen_pairs = BTreeSet::new();
            let mut seen_roles = BTreeSet::new();
            let mut seen_identities = BTreeSet::new();
            for judgment in &record.judgments {
                if !seen_pairs.insert((
                    judgment.reviewer_role.clone(),
                    judgment.reviewer_identity.clone(),
                )) {
                    return Err(format!(
                        "adjudication record `{file_name}` case `{case_id}`: duplicate judgment for role `{}` identity `{}`; each recorded role/identity pair may appear once",
                        judgment.reviewer_role, judgment.reviewer_identity
                    ));
                }
                seen_roles.insert(judgment.reviewer_role.as_str());
                seen_identities.insert(judgment.reviewer_identity.as_str());
            }
            // FIX fqGSD (CodeRabbit #3681): independence needs a one-to-one
            // role-to-identity mapping — one identity occupying two roles (or
            // one role carrying two identities) can satisfy both count checks
            // while the "independent roles" claim is false. With duplicate
            // pairs already rejected above, the mapping is one-to-one exactly
            // when every set has the pair count (simplification credited to
            // the gemini review), so a hand-edited record can never reach
            // `Adjudicated` with a shared identity.
            if seen_pairs.len() != seen_roles.len() || seen_pairs.len() != seen_identities.len() {
                return Err(format!(
                    "adjudication record `{file_name}` case `{case_id}`: role-to-identity mapping is not one-to-one; one identity must never occupy two roles and one role must never carry two identities — independence requires it"
                ));
            }
            Ok(derive_adjudication_view(record, item, &row_revision))
        });
        let adjudication_view = adjudication_view.transpose()?;

        // Replay side. Mismatch and comparison-unavailable counts follow the
        // replay summary's semantics: attempted runs only — a not_run record
        // has no comparison by definition.
        let diff_matches = replay_view
            .map(|view| view.diff_sha256 == sha256_file_or_blank(&root.join(&item.diff_path)))
            .unwrap_or(false);
        // FIX #3677: the replayed subject is only current when the row's
        // anchor still matches the record's echo. A moved anchor (or a record
        // written before the echo existed) means the replay evaluated a
        // different subject, so it can never be silently current: the row
        // lands in the `stale` aggregate, the `anchor_stale` disclosure names
        // the reason, and the rate as-of identity excludes it. Rows without a
        // declared anchor (carryover/historical) have no anchor identity to
        // bind — anchor currency does not apply to them (round-2 review).
        let row_has_anchor = item.anchor.file.non_blank_value().is_some();
        let current_anchor = RecordAnchor {
            file: item
                .anchor
                .file
                .non_blank_value()
                .unwrap_or_default()
                .to_string(),
            line: item.anchor.line.value().copied(),
            owner: item.anchor.owner.clone(),
        };
        let anchor_state = if !row_has_anchor {
            None
        } else {
            match replay_view.and_then(|view| view.anchor.clone()) {
                // FIX (round-3 review): no record means nothing to be stale —
                // `no_replay_record` already discloses that state.
                None => replay_view.map(|_| "missing_echo".to_string()),
                Some(echo) if echo != current_anchor => Some("anchor_moved".to_string()),
                Some(_) => None,
            }
        };
        let identity_current = replay_view
            .map(|view| {
                diff_matches
                    && !view.prior_actual_stale
                    && view.row_kind == row_kind_name(row_kind(item))
                    && anchor_state.is_none()
            })
            .unwrap_or(false);
        let replayed = replay_view.is_some_and(|view| view.outcome_kind != "not_run");
        if replayed {
            *counts.entry("replayed").or_insert(0) += 1;
            if replay_view.is_some_and(|view| !view.mismatch_kinds.is_empty()) {
                *counts.entry("mismatched").or_insert(0) += 1;
            }
            if replay_view.is_some_and(|view| view.comparison_unavailable) {
                *counts.entry("comparison_unavailable").or_insert(0) += 1;
            }
        } else {
            *counts.entry("not_run").or_insert(0) += 1;
            if replay_view.is_none() {
                *counts.entry("no_replay_record").or_insert(0) += 1;
            }
        }
        if replay_view.is_some() && !identity_current {
            *counts.entry("stale").or_insert(0) += 1;
        }
        if anchor_state.is_some() {
            *counts.entry("anchor_stale").or_insert(0) += 1;
        }
        rate_identity.insert(
            case_id.clone(),
            replay_view
                .filter(|_| identity_current)
                .map(|view| (view.binary_version.clone(), view.binary_sha256.clone())),
        );

        // Adjudication side.
        let is_adjudicated = adjudication_view
            .as_ref()
            .is_some_and(|view| view.state == AdjudicationState::Adjudicated);
        if let Some(view) = &adjudication_view {
            *counts.entry(view.state.as_str()).or_insert(0) += 1;
        }

        // Coverage cells. A row counts under each of its behavior-family
        // shapes, so the family table may sum above `selected`; the table
        // title discloses that.
        cover(
            &mut by_direction,
            &item.expected_direction,
            replayed,
            is_adjudicated,
        );
        cover(&mut by_repository, &item.repo, replayed, is_adjudicated);
        for shape in &item.shape {
            cover(&mut by_family, shape, replayed, is_adjudicated);
        }
        cover(
            &mut by_oracle,
            item.actual_oracle_alignment
                .non_blank_value()
                .unwrap_or("unrecorded"),
            replayed,
            is_adjudicated,
        );
        cover(
            &mut by_limit,
            item.expected_static_limit_kind
                .non_blank_value()
                .unwrap_or("none"),
            replayed,
            is_adjudicated,
        );

        // Rates: separate error denominators over adjudicated rows only.
        // Replay mismatch data is never consulted here.
        if let (Some(view), true) = (&adjudication_view, is_adjudicated) {
            for (admitted, axis, rate) in [
                (
                    direction_admits_error(&item.expected_direction, "false_actionable"),
                    &view.false_actionable,
                    &mut false_actionable,
                ),
                (
                    direction_admits_error(&item.expected_direction, "false_exposed"),
                    &view.false_exposed,
                    &mut false_exposed,
                ),
            ] {
                if !admitted {
                    continue;
                }
                match axis.agreed_decided() {
                    Some(decided) => {
                        rate.denominator += 1;
                        rate.denominator_case_ids.push(case_id.clone());
                        if decided {
                            rate.numerator += 1;
                        }
                    }
                    None => rate.undecided += 1,
                }
            }
            for (axis, target) in [
                (&view.wrong_target, &mut wrong_target),
                (&view.invalid_command, &mut invalid_command),
            ] {
                match axis {
                    AxisValue::Agreed(Some(flagged)) => {
                        target.flagged += usize::from(*flagged);
                        target.assessed += 1;
                    }
                    AxisValue::Agreed(None) => target.unassessed += 1,
                    AxisValue::Disputed => target.disputed_axis += 1,
                    AxisValue::NotIndependent => {}
                }
            }
        }
        if item.expected_direction == "should_limit" {
            let key = match (&adjudication_view, is_adjudicated) {
                (Some(view), true) => match &view.limitation_quality {
                    AxisValue::Agreed(Some(quality)) => quality.clone(),
                    AxisValue::Agreed(None) => "undecided".to_string(),
                    AxisValue::Disputed => "disputed_axis".to_string(),
                    AxisValue::NotIndependent => "not_adjudicated".to_string(),
                },
                _ => "not_adjudicated".to_string(),
            };
            *limitation_correctness.entry(key).or_insert(0) += 1;
        }

        cases.push(json!({
            "case_id": case_id,
            "source_envelope": source_envelope,
            "repo": item.repo.clone(),
            "diff_path": item.diff_path.clone(),
            "expected_direction": item.expected_direction.clone(),
            "row_kind": row_kind_name(row_kind(item)),
            "behavior_family": item.shape.clone(),
            "must_not_claim": must_not_claim_echo(item),
            "replay": replay_view.map(|view| json!({
                "record": view.file_name.clone(),
                "outcome": view.outcome_kind.clone(),
                "candidate_classification": view.candidate.clone(),
                "diff_sha256": view.diff_sha256.clone(),
                "identity_current": identity_current,
                "anchor_stale_reason": anchor_state,
                "mismatched": !view.mismatch_kinds.is_empty(),
                "mismatch_kinds": view.mismatch_kinds.clone(),
                "comparison_unavailable": view.comparison_unavailable,
            })),
            "adjudication": adjudication_view.map(|view| json!({
                "record": format!("{}.json", stable_case_slug(case_id)),
                "state": view.state.as_str(),
                "roles": view.roles.clone(),
                "verdict": view.verdict_agreed.clone(),
                "false_actionable": bool_token(&view.false_actionable),
                "false_exposed": bool_token(&view.false_exposed),
                "wrong_target": bool_token(&view.wrong_target),
                "invalid_command": bool_token(&view.invalid_command),
                "limitation_quality": match &view.limitation_quality {
                    AxisValue::Agreed(Some(quality)) => Value::String(quality.clone()),
                    AxisValue::Agreed(None) => json!("undecided"),
                    AxisValue::Disputed => json!("disputed"),
                    AxisValue::NotIndependent => json!("not_independent"),
                },
                "row_revision": {
                    "stored": view.row_revision_stored,
                    "current": view.row_revision_current,
                },
            })),
        }));
    }

    counts.insert(
        "unjudged",
        rows.len() - counts["adjudicated"] - counts["inconclusive"],
    );
    for rate in [&mut false_actionable, &mut false_exposed] {
        rate.denominator_case_ids.sort();
        if rate.denominator > 0 {
            rate.rate = Some(rate.numerator as f64 / rate.denominator as f64);
        }
        // FIX f2TMb + FIX fqNy: the as-of identity comes only from the
        // denominator cases' own *current* replay records — every denominator
        // case must bind a record with one shared binary identity, else the
        // rate discloses `no_common_binary_identity` instead of citing the
        // directory-wide identity.
        let mut shared: Option<(String, String)> = None;
        let mut common = rate.denominator > 0;
        for id in &rate.denominator_case_ids {
            match rate_identity.get(id).and_then(|bound| bound.clone()) {
                Some(identity) => match &shared {
                    None => shared = Some(identity),
                    Some(previous) if *previous != identity => common = false,
                    Some(_) => {}
                },
                None => common = false,
            }
        }
        match (rate.denominator > 0, common, shared) {
            (true, true, Some((version, sha256))) => {
                rate.binary_version = Some(version);
                rate.binary_sha256 = Some(sha256);
                rate.as_of_basis = "denominator_case_records".to_string();
            }
            (true, _, _) => {
                rate.as_of_basis = "no_common_binary_identity".to_string();
            }
            (false, _, _) => {
                rate.as_of_basis = "no_denominator".to_string();
            }
        }
    }

    let thresholds = policy_path
        .map(|path| {
            evaluate_threshold_policy(
                root,
                path,
                rows.len(),
                counts["adjudicated"],
                &false_actionable,
                &false_exposed,
            )
        })
        .transpose()?;

    let mut report = json!({
        "schema_version": REPORT_SCHEMA_VERSION,
        "kind": REPORT_KIND,
        "spec": SPEC,
        "authority_boundary": AUTHORITY_BOUNDARY,
        "inputs": {
            "inventory": inventory_identity,
            "records_dir": records_display,
            "adjudications_dir": adjudications_display,
            "threshold_policy": policy_path,
        },
        "as_of": {
            "panel_digest": panel_digest,
            "replay_binary": binary_identity
                .as_ref()
                .map(|(version, sha256)| json!({"version": version, "sha256": sha256})),
            "replay_record_schema_version": if records.is_empty() {
                Value::Null
            } else {
                json!(RECORD_SCHEMA_VERSION)
            },
        },
        "counts": counts,
        "coverage": {
            "by_direction": by_direction,
            "by_repository": by_repository,
            "by_behavior_family": by_family,
            "by_oracle_alignment": by_oracle,
            "by_limitation_kind": by_limit,
            "relation_basis": {
                "available": false,
                "reason": "the retained panel schema carries no typed relation-basis field; coverage by relation basis is disclosed unavailable rather than invented",
            },
        },
        "rates": {
            "false_actionable": rate_value(&false_actionable),
            "false_exposed": rate_value(&false_exposed),
            "wrong_target": wrong_target,
            "invalid_command": invalid_command,
            "limitation_correctness": limitation_correctness,
        },
        "thresholds": thresholds.clone().unwrap_or(Value::Null),
        "cases": cases,
        "notes": [
            NOTE_REPLAY_ADVISORY,
            NOTE_NO_INHERITED_DENOMINATOR,
            NOTE_NO_COMBINED_SCORE,
            NOTE_RELATION_BASIS,
            NOTE_STALE_DEFINITION,
        ],
    });
    if thresholds.is_none() {
        report
            .as_object_mut()
            .ok_or("panel report must be a JSON object")?
            .remove("thresholds");
    }
    let json = format!(
        "{}\n",
        serde_json::to_string_pretty(&report)
            .map_err(|error| format!("serialize panel report: {error}"))?
    );
    let markdown = render_markdown(&report);
    Ok(RenderedReport { json, markdown })
}

fn rate_value(rate: &ErrorRate) -> Value {
    let mut value = json!({
        "numerator": rate.numerator,
        "denominator": rate.denominator,
        "undecided": rate.undecided,
        "coverage_boundary": rate.coverage_boundary,
        "denominator_case_ids": rate.denominator_case_ids,
        "as_of_basis": rate.as_of_basis,
        "as_of": {
            "binary_version": rate.binary_version,
            "binary_sha256": rate.binary_sha256,
        },
    });
    if let Some(measured) = rate.rate {
        value["rate"] = json!(measured);
    }
    value
}

fn row_kind_name(kind: RowKind) -> &'static str {
    match kind {
        RowKind::Seed => "seed",
        RowKind::Judged => "judged",
        RowKind::Carryover => "carryover",
    }
}

pub(super) fn sha256_file_or_blank(path: &Path) -> String {
    fs::read(path)
        .map(|bytes| sha256_hex(&bytes))
        .unwrap_or_default()
}
