//! Packet and per-row contracts for the frozen release-challenge judgments.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    CONFIDENCE, JUDGMENTS_AUTHORITY, JUDGMENTS_KIND, REVIEW_JUDGMENTS, ReferenceOutcome,
    ReleaseJudgments, RowJudgment, TERMINALS, selection_digest, stale_selection,
};
use crate::rust_judged_panel::{RELEASE_SELECTION_PATH, RustJudgedPanelManifest};

pub(super) fn validate_judgments(
    selection_bytes: &[u8],
    selection: &RustJudgedPanelManifest,
    packet: &ReleaseJudgments,
) -> Vec<String> {
    let mut violations = Vec::new();
    validate_header(packet, &mut violations);
    // Stale or edited selection bytes reject before any row is read: a
    // judgment is only meaningful for the exact rows it was made against.
    let actual = selection_digest(selection_bytes);
    if packet.selection_sha256 != actual {
        violations.push(stale_selection(&packet.selection_sha256, &actual));
        return violations;
    }
    validate_reference_run_and_roles(packet, &mut violations);

    let expected: BTreeMap<&str, &str> = selection
        .items
        .iter()
        .map(|item| (item.id.as_str(), item.expected_direction.as_str()))
        .collect();
    let roles: BTreeSet<&str> = packet.roles.iter().map(|role| role.id.as_str()).collect();
    let mut seen = BTreeSet::new();
    for row in &packet.judgments {
        let subject = format!("judgments.{}", row.case_id);
        if !seen.insert(row.case_id.as_str()) {
            violations.push(format!("{subject}: duplicate judgment"));
            continue;
        }
        let Some(direction) = expected.get(row.case_id.as_str()) else {
            violations.push(format!("{subject}: not a row of the frozen selection"));
            continue;
        };
        if row.expected_direction != *direction {
            violations.push(format!(
                "{subject}.expected_direction: selection says `{direction}`, packet says `{}`",
                row.expected_direction
            ));
        }
        validate_row(row, &subject, &roles, &mut violations);
    }
    for id in expected.keys() {
        if !seen.contains(id) {
            violations.push(format!(
                "judgments: frozen row `{id}` has no judgment; unjudged rows stay visible as inconclusive, never absent"
            ));
        }
    }
    violations
}

fn validate_header(packet: &ReleaseJudgments, violations: &mut Vec<String>) {
    if packet.schema_version != "0.1" {
        violations.push(format!(
            "schema_version: expected `0.1`, found `{}`",
            packet.schema_version
        ));
    }
    if packet.kind != JUDGMENTS_KIND {
        violations.push(format!("kind: expected `{JUDGMENTS_KIND}`"));
    }
    if packet.authority != JUDGMENTS_AUTHORITY {
        violations.push(format!("authority: expected `{JUDGMENTS_AUTHORITY}`"));
    }
    if packet.selection_path != RELEASE_SELECTION_PATH {
        violations.push(format!(
            "selection_path: expected `{RELEASE_SELECTION_PATH}`"
        ));
    }
}

fn validate_reference_run_and_roles(packet: &ReleaseJudgments, violations: &mut Vec<String>) {
    let run = &packet.reference_run;
    for (field, value) in [
        ("reference_run.analyzer", &run.analyzer),
        ("reference_run.source", &run.source),
        ("reference_run.scope", &run.scope),
    ] {
        reject_blank(violations, field, value, "must be non-empty");
    }
    if run.argv.is_empty() {
        violations.push("reference_run.argv: must be non-empty".to_string());
    }
    if packet.limits.is_empty() {
        violations.push("limits: must state the packet's non-claims".to_string());
    }

    let mut roles = BTreeSet::new();
    for role in &packet.roles {
        if !roles.insert(role.id.as_str()) {
            violations.push(format!("roles: duplicate role `{}`", role.id));
        }
        if role.method.trim().is_empty() || role.blind_to.is_empty() {
            violations.push(format!(
                "roles.{}: method and blind_to must be non-empty",
                role.id
            ));
        }
    }
}

fn validate_row(
    row: &RowJudgment,
    subject: &str,
    roles: &BTreeSet<&str>,
    violations: &mut Vec<String>,
) {
    if !TERMINALS.contains(&row.terminal.as_str()) {
        violations.push(format!(
            "{subject}.terminal: `{}` is not one of {}",
            row.terminal,
            TERMINALS.join(", ")
        ));
    }
    validate_structural(row, subject, violations);
    let scan = scan_reviews(row, subject, roles, violations);
    let confirmed = row.terminal.strip_prefix("confirmed_");
    validate_independence(row, subject, confirmed, &scan, violations);
    validate_disagreement(row, subject, confirmed, violations);
    validate_outcome(row, subject, confirmed, violations);
}

fn validate_structural(row: &RowJudgment, subject: &str, violations: &mut Vec<String>) {
    let s = &row.structural;
    for (field, value) in [
        ("owner", &s.owner),
        ("activation", &s.activation),
        ("propagation", &s.propagation),
        ("observer", &s.observer),
        ("oracle", &s.oracle),
        ("target", &s.target),
        ("limitation", &s.limitation),
    ] {
        reject_blank(
            violations,
            &format!("{subject}.structural.{field}"),
            value,
            "must cite evidence",
        );
    }
}

struct ReviewScan<'a> {
    row_roles: BTreeSet<&'a str>,
    review_directions: BTreeSet<&'a str>,
}

fn scan_reviews<'a>(
    row: &'a RowJudgment,
    subject: &str,
    roles: &BTreeSet<&str>,
    violations: &mut Vec<String>,
) -> ReviewScan<'a> {
    if row.reviews.is_empty() {
        violations.push(format!("{subject}.reviews: at least one recorded review"));
    }
    let mut row_roles = BTreeSet::new();
    let mut review_directions = BTreeSet::new();
    for review in &row.reviews {
        // Only declared roles count toward the two-role floor.
        if roles.contains(review.role.as_str()) {
            row_roles.insert(review.role.as_str());
        } else {
            violations.push(format!(
                "{subject}.reviews: role `{}` is not declared",
                review.role
            ));
        }
        if !CONFIDENCE.contains(&review.confidence.as_str()) {
            violations.push(format!(
                "{subject}.reviews.{}.confidence: `{}` is not high/medium/low",
                review.role, review.confidence
            ));
        }
        if review.evidence.is_empty()
            || review
                .evidence
                .iter()
                .any(|citation| citation.trim().is_empty())
        {
            violations.push(format!(
                "{subject}.reviews.{}: cited evidence is required and must be non-blank",
                review.role
            ));
        }
        match REVIEW_JUDGMENTS
            .iter()
            .find(|(judgment, _)| *judgment == review.judgment)
        {
            Some((_, direction)) => {
                review_directions.insert(*direction);
            }
            None => violations.push(format!(
                "{subject}.reviews.{}.judgment: `{}` is not a review verdict",
                review.role, review.judgment
            )),
        }
    }
    ReviewScan {
        row_roles,
        review_directions,
    }
}

fn validate_independence(
    row: &RowJudgment,
    subject: &str,
    confirmed: Option<&str>,
    scan: &ReviewScan<'_>,
    violations: &mut Vec<String>,
) {
    // Reviews that support different directions, or not the confirmed one,
    // are a disagreement; it must be recorded rather than silently certified.
    let reviews_disagree = scan.review_directions.len() > 1
        || confirmed.is_some_and(|direction| {
            scan.review_directions
                .iter()
                .any(|supported| *supported != direction)
        });
    if reviews_disagree && row.disagreement.is_none() {
        violations.push(format!(
            "{subject}.disagreement: reviews support {:?} but no disagreement is recorded",
            scan.review_directions
        ));
    }
    // #3806: two independent roles for every limit, disputed, or
    // release-blocking row. A should_gap row can block release (a
    // false-exposed result), and a terminal that departs from the expected
    // direction is disputed by definition.
    let departs = confirmed.is_some_and(|direction| direction != row.expected_direction);
    let needs_two = row.expected_direction == "should_limit"
        || row.expected_direction == "should_gap"
        || row.terminal == "confirmed_should_limit"
        || departs
        || row.disagreement.is_some();
    if needs_two && scan.row_roles.len() < 2 {
        violations.push(format!(
            "{subject}.reviews: limit, gap, disputed, or departing rows need two independent roles; found {}",
            scan.row_roles.len()
        ));
    }
}

fn validate_disagreement(
    row: &RowJudgment,
    subject: &str,
    confirmed: Option<&str>,
    violations: &mut Vec<String>,
) {
    // An empty summary does not record the disagreement, whatever the terminal.
    if let Some(disagreement) = &row.disagreement
        && disagreement.summary.trim().is_empty()
    {
        violations.push(format!("{subject}.disagreement.summary: must be non-empty"));
    }
    match (&row.disagreement, row.terminal.as_str(), confirmed) {
        (Some(disagreement), "inconclusive_disagreement", _)
            if disagreement.resolution.is_some() =>
        {
            violations.push(format!(
                "{subject}.disagreement: a resolved disagreement cannot stay inconclusive_disagreement"
            ));
        }
        (None, "inconclusive_disagreement", _) => violations.push(format!(
            "{subject}.disagreement: inconclusive_disagreement must record the disagreement"
        )),
        (Some(disagreement), _, Some(_))
            if disagreement
                .resolution
                .as_deref()
                .is_none_or(|resolution| resolution.trim().is_empty()) =>
        {
            violations.push(format!(
                "{subject}.disagreement: a confirmed terminal needs the cited resolution"
            ));
        }
        _ => {}
    }
}

fn validate_outcome(
    row: &RowJudgment,
    subject: &str,
    confirmed: Option<&str>,
    violations: &mut Vec<String>,
) {
    let outcome = &row.reference_outcome;
    reject_blank(
        violations,
        &format!("{subject}.reference_outcome.observation"),
        &outcome.observation,
        "must describe the reference output",
    );
    // The two error directions are exclusive for one terminal row.
    if outcome.false_actionable == Some(true) && outcome.false_exposed == Some(true) {
        violations.push(format!(
            "{subject}.reference_outcome: false_actionable and false_exposed are mutually exclusive"
        ));
    }
    // Each error label only exists for the directions that admit it:
    // over-credit needs a missing discriminator, a false repair needs one
    // that is already present.
    if outcome.false_exposed == Some(true)
        && !matches!(confirmed, Some("should_gap" | "should_limit"))
    {
        violations.push(format!(
            "{subject}.reference_outcome.false_exposed: only a confirmed gap or limit row can be over-credited"
        ));
    }
    if outcome.false_actionable == Some(true)
        && !matches!(confirmed, Some("should_stay_quiet" | "should_limit"))
    {
        violations.push(format!(
            "{subject}.reference_outcome.false_actionable: only a confirmed quiet or limit row can carry a false repair"
        ));
    }
    // Missing evidence is never forced into a direction.
    if confirmed.is_none() && outcome_labels_present(outcome) {
        violations.push(format!(
            "{subject}.reference_outcome: inconclusive or invalid rows carry no outcome labels"
        ));
    }
    if row.non_claims.is_empty() {
        violations.push(format!("{subject}.non_claims: must be non-empty"));
    }
}

fn outcome_labels_present(outcome: &ReferenceOutcome) -> bool {
    [
        outcome.false_actionable,
        outcome.false_exposed,
        outcome.under_credit,
        outcome.limitation_correct,
    ]
    .iter()
    .any(Option::is_some)
}

fn reject_blank(violations: &mut Vec<String>, field: &str, value: &str, message: &str) {
    if value.trim().is_empty() {
        violations.push(format!("{field}: {message}"));
    }
}
