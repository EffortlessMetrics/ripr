use std::fs;
use std::path::Path;

use super::*;
use crate::rust_judged_panel::{
    RELEASE_SELECTION_PATH, check_release_selection_at, parse_json_without_duplicate_keys,
};

fn repository_root() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/.."))
}

fn canonical() -> Result<(Vec<u8>, RustJudgedPanelManifest, serde_json::Value), String> {
    let root = repository_root();
    let selection = check_release_selection_at(root)?;
    let bytes = fs::read(root.join(RELEASE_SELECTION_PATH)).map_err(|error| error.to_string())?;
    let body =
        fs::read_to_string(root.join(RELEASE_JUDGMENTS_PATH)).map_err(|error| error.to_string())?;
    let value = parse_json_without_duplicate_keys(&body).map_err(|error| error.to_string())?;
    Ok((bytes, selection, value))
}

fn violations_for(
    bytes: &[u8],
    selection: &RustJudgedPanelManifest,
    value: serde_json::Value,
) -> Result<Vec<String>, String> {
    let packet: ReleaseJudgments =
        serde_json::from_value(value).map_err(|error| error.to_string())?;
    Ok(validate_judgments(bytes, selection, &packet))
}

fn expect_violation(
    mutate: impl FnOnce(&mut serde_json::Value),
    fragment: &str,
) -> Result<(), String> {
    let (bytes, selection, mut value) = canonical()?;
    mutate(&mut value);
    let violations = violations_for(&bytes, &selection, value)?;
    if violations
        .iter()
        .any(|violation| violation.contains(fragment))
    {
        Ok(())
    } else {
        Err(format!(
            "expected a violation containing `{fragment}`, got {violations:?}"
        ))
    }
}

fn expect_clean(mutate: impl FnOnce(&mut serde_json::Value)) -> Result<(), String> {
    let (bytes, selection, mut value) = canonical()?;
    mutate(&mut value);
    let violations = violations_for(&bytes, &selection, value)?;
    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!("expected no violations, got {violations:?}"))
    }
}

fn row_mut<'a>(value: &'a mut serde_json::Value, id: &str) -> &'a mut serde_json::Value {
    // A missing id lands on an unknown field, which fails the parse
    // instead of silently mutating nothing.
    let index = value["judgments"]
        .as_array()
        .and_then(|rows| rows.iter().position(|row| row["case_id"] == id));
    match index {
        Some(index) => &mut value["judgments"][index],
        None => &mut value["missing"],
    }
}

#[test]
fn canonical_release_judgments_bind_every_frozen_row() -> Result<(), String> {
    let (bytes, selection, value) = canonical()?;
    let rows = value["judgments"].as_array().map_or(0, Vec::len);
    if rows != selection.items.len() || rows == 0 {
        return Err(format!(
            "{rows} judgments for {} rows",
            selection.items.len()
        ));
    }
    let violations = violations_for(&bytes, &selection, value)?;
    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations.join("\n"))
    }
}

#[test]
fn stale_selection_digest_rejects_before_rows() -> Result<(), String> {
    let (mut bytes, selection, value) = canonical()?;
    bytes.push(b'\n');
    let violations = violations_for(&bytes, &selection, value)?;
    match violations.as_slice() {
        [only] if only.contains("selection_sha256") => Ok(()),
        other => Err(format!("stale selection was not rejected alone: {other:?}")),
    }
}

#[test]
fn missing_duplicate_and_foreign_rows_reject() -> Result<(), String> {
    expect_violation(
        |value| {
            if let Some(rows) = value["judgments"].as_array_mut() {
                rows.pop();
            }
        },
        "has no judgment",
    )?;
    expect_violation(
        |value| {
            if let Some(rows) = value["judgments"].as_array_mut()
                && let Some(first) = rows.first().cloned()
            {
                rows.push(first);
            }
        },
        "duplicate judgment",
    )?;
    expect_violation(
        |value| {
            if let Some(rows) = value["judgments"].as_array_mut()
                && let Some(first) = rows.first_mut()
            {
                first["case_id"] = "not-a-frozen-row".into();
            }
        },
        "not a row of the frozen selection",
    )
}

#[test]
fn expected_direction_must_match_the_selection() -> Result<(), String> {
    expect_violation(
        |value| row_mut(value, "p1744-quiet-test-only")["expected_direction"] = "should_gap".into(),
        "selection says `should_stay_quiet`",
    )
}

#[test]
fn false_actionable_and_false_exposed_are_exclusive() -> Result<(), String> {
    expect_violation(
        |value| {
            let row = row_mut(value, "p1741-pair-rows-gap");
            row["reference_outcome"]["false_actionable"] = true.into();
            row["reference_outcome"]["false_exposed"] = true.into();
        },
        "mutually exclusive",
    )
}

#[test]
fn outcome_labels_must_fit_the_confirmed_direction() -> Result<(), String> {
    expect_violation(
        |value| {
            row_mut(value, "p1745-wedge-rows-quiet")["reference_outcome"]["false_exposed"] =
                true.into();
        },
        "only a confirmed gap or limit row can be over-credited",
    )?;
    expect_violation(
        |value| {
            row_mut(value, "p1706-wiring-rows-gap")["reference_outcome"] = serde_json::json!({
                "observation": "x", "false_actionable": true, "false_exposed": null,
                "under_credit": null, "limitation_correct": null
            });
        },
        "only a confirmed quiet or limit row can carry a false repair",
    )
}

#[test]
fn contradicting_reviews_need_a_recorded_disagreement() -> Result<(), String> {
    // One review now supports a gap on a confirmed limit row.
    expect_violation(
        |value| {
            row_mut(value, "s3866-doctor-packet-subprocess-limit")["reviews"][1]["judgment"] =
                "not_discriminated".into();
        },
        "no disagreement is recorded",
    )?;
    expect_violation(
        |value| {
            row_mut(value, "p1744-quiet-test-only")["reviews"][0]["judgment"] = "fine".into();
        },
        "is not a review verdict",
    )
}

#[test]
fn stale_selection_is_reported_before_malformed_rows() -> Result<(), String> {
    let (mut bytes, _, mut value) = canonical()?;
    bytes.push(b'\n');
    row_mut(&mut value, "p1744-quiet-test-only")["reviews"] = "not a list".into();
    match parse_bound_judgments(&bytes, &value.to_string()) {
        Err(error) if error.contains("selection_sha256") => Ok(()),
        Err(error) => Err(format!("stale selection lost its reason: {error}")),
        Ok(_) => Err("stale selection with a malformed row parsed".to_string()),
    }
}

#[test]
fn limit_and_gap_rows_need_two_roles() -> Result<(), String> {
    for id in [
        "s3866-doctor-packet-subprocess-limit",
        "p1706-wiring-rows-gap",
    ] {
        expect_violation(
            |value| {
                let row = row_mut(value, id);
                if let Some(reviews) = row["reviews"].as_array_mut() {
                    reviews.truncate(1);
                }
            },
            "need two independent roles",
        )?;
    }
    Ok(())
}

#[test]
fn inconclusive_rows_carry_no_outcome_labels() -> Result<(), String> {
    expect_violation(
        |value| {
            let row = row_mut(value, "p1744-quiet-test-only");
            row["terminal"] = "inconclusive_missing_evidence".into();
            row["reference_outcome"]["false_exposed"] = false.into();
        },
        "carry no outcome labels",
    )
}

#[test]
fn disagreement_must_stay_visible_until_resolved() -> Result<(), String> {
    expect_violation(
        |value| {
            let row = row_mut(value, "p1744-quiet-test-only");
            row["terminal"] = "inconclusive_disagreement".into();
            row["disagreement"] = serde_json::Value::Null;
            row["reference_outcome"] = serde_json::json!({
                "observation": "x", "false_actionable": null, "false_exposed": null,
                "under_credit": null, "limitation_correct": null
            });
        },
        "must record the disagreement",
    )?;
    expect_violation(
        |value| {
            let row = row_mut(value, "p1744-quiet-test-only");
            row["disagreement"] =
                serde_json::json!({ "summary": "roles differ", "resolution": null });
        },
        "needs the cited resolution",
    )?;
    expect_violation(
        |value| {
            let row = row_mut(value, "p1744-quiet-test-only");
            row["terminal"] = "inconclusive_disagreement".into();
            row["disagreement"] = serde_json::json!({ "summary": " ", "resolution": null });
            row["reference_outcome"] = serde_json::json!({
                "observation": "x", "false_actionable": null, "false_exposed": null,
                "under_credit": null, "limitation_correct": null
            });
        },
        "disagreement.summary: must be non-empty",
    )
}

#[test]
fn unknown_terminal_and_undeclared_role_reject() -> Result<(), String> {
    expect_violation(
        |value| row_mut(value, "p1744-quiet-test-only")["terminal"] = "pass".into(),
        "is not one of",
    )?;
    expect_violation(
        |value| {
            row_mut(value, "p1744-quiet-test-only")["reviews"][0]["role"] = "ghost".into();
        },
        "is not declared",
    )?;
    // An undeclared role must not count toward the two-role floor.
    expect_violation(
        |value| {
            row_mut(value, "p1706-wiring-rows-gap")["reviews"][1]["role"] = "ghost".into();
        },
        "need two independent roles",
    )
}

#[test]
fn blank_structural_citation_is_not_evidence() -> Result<(), String> {
    expect_violation(
        |value| {
            row_mut(value, "p1744-quiet-test-only")["structural"]["owner"] = "  ".into();
        },
        "structural.owner: must cite evidence",
    )
}

#[test]
fn blank_or_empty_review_evidence_is_not_a_citation() -> Result<(), String> {
    expect_violation(
        |value| {
            row_mut(value, "p1744-quiet-test-only")["reviews"][0]["evidence"] =
                serde_json::json!([]);
        },
        "cited evidence is required",
    )?;
    expect_violation(
        |value| {
            row_mut(value, "p1744-quiet-test-only")["reviews"][0]["evidence"] =
                serde_json::json!(["  "]);
        },
        "must be non-blank",
    )?;
    // A real citation next to a blank one must not launder the blank.
    expect_violation(
        |value| {
            row_mut(value, "p1744-quiet-test-only")["reviews"][0]["evidence"] =
                serde_json::json!(["doctor.rs:563 #[cfg(test)]", " "]);
        },
        "must be non-blank",
    )
}

#[test]
fn empty_reviews_confidence_observation_and_non_claims_reject() -> Result<(), String> {
    expect_violation(
        |value| {
            row_mut(value, "p1744-quiet-test-only")["reviews"] = serde_json::json!([]);
        },
        "at least one recorded review",
    )?;
    expect_violation(
        |value| {
            row_mut(value, "p1744-quiet-test-only")["reviews"][0]["confidence"] = "sure".into();
        },
        "is not high/medium/low",
    )?;
    expect_violation(
        |value| {
            row_mut(value, "p1744-quiet-test-only")["reference_outcome"]["observation"] =
                " \t".into();
        },
        "must describe the reference output",
    )?;
    expect_violation(
        |value| {
            row_mut(value, "p1744-quiet-test-only")["non_claims"] = serde_json::json!([]);
        },
        "non_claims: must be non-empty",
    )
}

#[test]
fn packet_header_identity_and_roles_reject_emptiness_and_duplicates() -> Result<(), String> {
    expect_violation(
        |value| value["kind"] = "rust_judged_panel_seed".into(),
        "kind: expected `rust_release_challenge_judgments`",
    )?;
    expect_violation(
        |value| value["authority"] = "EffortlessMetrics/ripr-swarm#3164".into(),
        "authority: expected `EffortlessMetrics/ripr-swarm#3806`",
    )?;
    expect_violation(
        |value| value["limits"] = serde_json::json!([]),
        "limits: must state the packet's non-claims",
    )?;
    expect_violation(
        |value| value["reference_run"]["argv"] = serde_json::json!([]),
        "reference_run.argv: must be non-empty",
    )?;
    expect_violation(
        |value| {
            if let Some(roles) = value["roles"].as_array_mut()
                && let Some(first) = roles.first().cloned()
            {
                roles.push(first);
            }
        },
        "duplicate role",
    )?;
    expect_violation(
        |value| {
            value["roles"][0]["method"] = " ".into();
            value["roles"][0]["blind_to"] = serde_json::json!([]);
        },
        "method and blind_to must be non-empty",
    )
}

#[test]
fn same_declared_role_twice_does_not_satisfy_the_two_role_floor() -> Result<(), String> {
    expect_violation(
        |value| {
            let row = row_mut(value, "p1706-wiring-rows-gap");
            let first_role = row["reviews"][0]["role"].clone();
            row["reviews"][1]["role"] = first_role;
        },
        "need two independent roles",
    )
}

#[test]
fn departing_quiet_to_gap_row_still_needs_two_roles() -> Result<(), String> {
    // expected should_stay_quiet, terminal confirmed_should_gap.
    expect_violation(
        |value| {
            let row = row_mut(value, "p1741-helper-rows-quiet");
            if let Some(reviews) = row["reviews"].as_array_mut() {
                reviews.truncate(1);
            }
        },
        "need two independent roles",
    )
}

#[test]
fn quiet_control_may_carry_one_declared_role() -> Result<(), String> {
    expect_clean(|value| {
        let row = row_mut(value, "p1744-quiet-test-only");
        if let Some(reviews) = row["reviews"].as_array_mut() {
            reviews.truncate(1);
        }
    })
}

#[test]
fn resolved_disagreement_cannot_stay_inconclusive() -> Result<(), String> {
    expect_violation(
        |value| {
            let row = row_mut(value, "p1741-helper-rows-quiet");
            row["terminal"] = "inconclusive_disagreement".into();
            row["reference_outcome"] = serde_json::json!({
                "observation": "x", "false_actionable": null, "false_exposed": null,
                "under_credit": null, "limitation_correct": null
            });
        },
        "a resolved disagreement cannot stay inconclusive_disagreement",
    )
}

#[test]
fn whitespace_only_resolution_does_not_confirm_a_disagreement() -> Result<(), String> {
    expect_violation(
        |value| {
            row_mut(value, "p1741-helper-rows-quiet")["disagreement"]["resolution"] = "  ".into();
        },
        "a confirmed terminal needs the cited resolution",
    )
}

#[test]
fn invalid_identity_cannot_carry_outcome_labels() -> Result<(), String> {
    expect_violation(
        |value| {
            row_mut(value, "p1744-quiet-test-only")["terminal"] = "invalid_case_identity".into();
        },
        "carry no outcome labels",
    )
}

#[test]
fn canonical_packet_retains_silent_over_credit_and_quiet_control() -> Result<(), String> {
    let (_, _, value) = canonical()?;
    let rows = value["judgments"].as_array().ok_or("judgments missing")?;
    let quiet_control = rows
        .iter()
        .find(|row| row["case_id"] == "p1744-quiet-test-only");
    let over_credit = rows.iter().find(|row| {
        row["terminal"] == "confirmed_should_gap"
            && row["reference_outcome"]["false_exposed"] == true
    });
    match (quiet_control, over_credit) {
        (Some(quiet), Some(gap))
            if quiet["terminal"] == "confirmed_should_stay_quiet"
                && quiet["reference_outcome"]["false_exposed"] == false
                && gap["case_id"] != "p1744-quiet-test-only" =>
        {
            Ok(())
        }
        _ => Err(
            "canonical packet lost the quiet control or the silent over-credit counterexample"
                .to_string(),
        ),
    }
}

#[test]
fn production_check_and_summary_consume_the_validated_packet() -> Result<(), String> {
    let root = repository_root();
    let (selection, packet) = check_release_judgments_at(root)?;
    if selection.items.len() != packet.judgments.len() || packet.judgments.is_empty() {
        return Err(format!(
            "check joined {} selection rows to {} judgments",
            selection.items.len(),
            packet.judgments.len()
        ));
    }
    let false_exposed = packet
        .judgments
        .iter()
        .filter(|row| row.reference_outcome.false_exposed == Some(true))
        .count();
    let summary = summarize(&packet);
    if false_exposed == 0
        || !summary.contains(&format!("reference false_exposed: {false_exposed} true of"))
        || !summary.contains("departs from expected: p1741-helper-rows-quiet")
        || !summary.contains(&format!("rows: {}", packet.judgments.len()))
    {
        return Err(format!(
            "summary did not consume the validated packet: {summary}"
        ));
    }
    Ok(())
}
