//! Independent adjudication packet for the frozen 0.11 release challenge
//! (#3806).
//!
//! The packet binds one terminal structural judgment to every row of the
//! frozen selection (`release-selection.json`, #3805) by the selection's
//! exact byte digest. It never edits the selection: a judgment that
//! disagrees with a row's expected direction is recorded as such, not
//! folded back into the manifest.
//!
//! The reference-run outcome labels compare the judgment with one named
//! analyzer run used during investigation. They are not the #1609
//! candidate replay (#2769), which consumes this packet later.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::{
    RELEASE_DIRECTIONS, RELEASE_SELECTION_PATH, RustJudgedPanelManifest,
    check_release_selection_at, parse_json_without_duplicate_keys,
};

pub(crate) const RELEASE_JUDGMENTS_PATH: &str =
    "metrics/rust-judged-behavior-panel/release-judgments.json";
const JUDGMENTS_RERUN_COMMAND: &str = "cargo xtask check-release-challenge-judgments";
const JUDGMENTS_KIND: &str = "rust_release_challenge_judgments";
const JUDGMENTS_AUTHORITY: &str = "EffortlessMetrics/ripr-swarm#3806";

/// Terminal vocabulary from #3806. `confirmed_should_*` names the direction
/// the evidence supports, which may differ from the row's expected one.
const TERMINALS: [&str; 6] = [
    "confirmed_should_gap",
    "confirmed_should_stay_quiet",
    "confirmed_should_limit",
    "inconclusive_missing_evidence",
    "inconclusive_disagreement",
    "invalid_case_identity",
];
const CONFIDENCE: [&str; 3] = ["high", "medium", "low"];
/// Review verdicts and the confirmed direction each one supports. Reviews
/// that support different directions, or a direction other than the
/// confirmed terminal, are a disagreement and must be recorded as one.
const REVIEW_JUDGMENTS: [(&str, &str); 5] = [
    ("discriminated", "should_stay_quiet"),
    ("no_production_behavior", "should_stay_quiet"),
    ("weakly_discriminated", "should_gap"),
    ("not_discriminated", "should_gap"),
    ("limited", "should_limit"),
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReleaseJudgments {
    schema_version: String,
    kind: String,
    authority: String,
    selection_path: String,
    selection_sha256: String,
    reference_run: ReferenceRun,
    roles: Vec<ReviewRole>,
    limits: Vec<String>,
    judgments: Vec<RowJudgment>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReferenceRun {
    analyzer: String,
    source: String,
    argv: Vec<String>,
    scope: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewRole {
    id: String,
    method: String,
    blind_to: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RowJudgment {
    case_id: String,
    expected_direction: String,
    terminal: String,
    structural: Structural,
    reviews: Vec<Review>,
    disagreement: Option<Disagreement>,
    reference_outcome: ReferenceOutcome,
    non_claims: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Structural {
    owner: String,
    activation: String,
    propagation: String,
    observer: String,
    oracle: String,
    target: String,
    limitation: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Review {
    role: String,
    judgment: String,
    confidence: String,
    evidence: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Disagreement {
    summary: String,
    resolution: Option<String>,
}

/// Outcome labels against the reference run. `None` is "not established",
/// never false.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReferenceOutcome {
    observation: String,
    false_actionable: Option<bool>,
    false_exposed: Option<bool>,
    under_credit: Option<bool>,
    limitation_correct: Option<bool>,
}

pub(crate) fn check_release_judgments() -> Result<(), String> {
    let (_, judgments) = check_release_judgments_at(Path::new("."))?;
    let summary = summarize(&judgments);
    crate::write_report("release-judgments.md", &summary)?;
    print!("{summary}");
    Ok(())
}

pub(crate) fn check_release_judgments_at(
    root: &Path,
) -> Result<(RustJudgedPanelManifest, ReleaseJudgments), String> {
    let selection = check_release_selection_at(root)?;
    let selection_bytes = fs::read(root.join(RELEASE_SELECTION_PATH))
        .map_err(|error| format!("read `{RELEASE_SELECTION_PATH}`: {error}"))?;
    let body = fs::read_to_string(root.join(RELEASE_JUDGMENTS_PATH))
        .map_err(|error| format!("read `{RELEASE_JUDGMENTS_PATH}`: {error}"))?;
    let judgments = parse_bound_judgments(&selection_bytes, &body)?;
    let mut violations = validate_judgments(&selection_bytes, &selection, &judgments);
    violations.sort();
    violations.dedup();
    if violations.is_empty() {
        Ok((selection, judgments))
    } else {
        Err(format!(
            "Release-challenge judgments `{RELEASE_JUDGMENTS_PATH}` have {} violation(s):\n- {}\nrerun: {JUDGMENTS_RERUN_COMMAND}",
            violations.len(),
            violations.join("\n- ")
        ))
    }
}

fn selection_digest(selection_bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(selection_bytes))
}

fn stale_selection(bound: &str, actual: &str) -> String {
    format!(
        "selection_sha256: packet binds `{bound}` but `{RELEASE_SELECTION_PATH}` is `{actual}`; re-adjudicate against the current selection"
    )
}

/// Reads the selection binding before the typed rows, so a stale packet
/// reports the stale selection even when its rows no longer parse.
fn parse_bound_judgments(selection_bytes: &[u8], body: &str) -> Result<ReleaseJudgments, String> {
    let value = parse_json_without_duplicate_keys(body)
        .map_err(|error| format!("parse `{RELEASE_JUDGMENTS_PATH}`: {error}"))?;
    let actual = selection_digest(selection_bytes);
    let bound = value
        .get("selection_sha256")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("<missing>");
    if bound != actual {
        return Err(format!(
            "Release-challenge judgments `{RELEASE_JUDGMENTS_PATH}`: {}\nrerun: {JUDGMENTS_RERUN_COMMAND}",
            stale_selection(bound, &actual)
        ));
    }
    serde_json::from_value(value)
        .map_err(|error| format!("parse `{RELEASE_JUDGMENTS_PATH}`: {error}"))
}

fn validate_judgments(
    selection_bytes: &[u8],
    selection: &RustJudgedPanelManifest,
    packet: &ReleaseJudgments,
) -> Vec<String> {
    let mut violations = Vec::new();
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
    // Stale or edited selection bytes reject before any row is read: a
    // judgment is only meaningful for the exact rows it was made against.
    let actual = selection_digest(selection_bytes);
    if packet.selection_sha256 != actual {
        violations.push(stale_selection(&packet.selection_sha256, &actual));
        return violations;
    }
    let run = &packet.reference_run;
    for (field, value) in [
        ("reference_run.analyzer", &run.analyzer),
        ("reference_run.source", &run.source),
        ("reference_run.scope", &run.scope),
    ] {
        if value.trim().is_empty() {
            violations.push(format!("{field}: must be non-empty"));
        }
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

    let expected: BTreeMap<&str, &str> = selection
        .items
        .iter()
        .map(|item| (item.id.as_str(), item.expected_direction.as_str()))
        .collect();
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
        if value.trim().is_empty() {
            violations.push(format!("{subject}.structural.{field}: must cite evidence"));
        }
    }
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
        if review.evidence.is_empty() {
            violations.push(format!(
                "{subject}.reviews.{}: cited evidence is required",
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
    // Reviews that support different directions, or not the confirmed one,
    // are a disagreement; it must be recorded rather than silently certified.
    let confirmed = row.terminal.strip_prefix("confirmed_");
    let reviews_disagree = review_directions.len() > 1
        || confirmed.is_some_and(|direction| {
            review_directions
                .iter()
                .any(|supported| *supported != direction)
        });
    if reviews_disagree && row.disagreement.is_none() {
        violations.push(format!(
            "{subject}.disagreement: reviews support {:?} but no disagreement is recorded",
            review_directions
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
    if needs_two && row_roles.len() < 2 {
        violations.push(format!(
            "{subject}.reviews: limit, gap, disputed, or departing rows need two independent roles; found {}",
            row_roles.len()
        ));
    }
    // An empty summary does not record the disagreement, whatever the terminal.
    if let Some(disagreement) = &row.disagreement
        && disagreement.summary.trim().is_empty()
    {
        violations.push(format!("{subject}.disagreement.summary: must be non-empty"));
    }
    match (&row.disagreement, row.terminal.as_str()) {
        (Some(disagreement), "inconclusive_disagreement") if disagreement.resolution.is_some() => {
            violations.push(format!(
                "{subject}.disagreement: a resolved disagreement cannot stay inconclusive_disagreement"
            ));
        }
        (None, "inconclusive_disagreement") => violations.push(format!(
            "{subject}.disagreement: inconclusive_disagreement must record the disagreement"
        )),
        (Some(disagreement), terminal)
            if terminal.starts_with("confirmed_")
                && disagreement
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
    let outcome = &row.reference_outcome;
    if outcome.observation.trim().is_empty() {
        violations.push(format!(
            "{subject}.reference_outcome.observation: must describe the reference output"
        ));
    }
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
    if !row.terminal.starts_with("confirmed_")
        && [
            outcome.false_actionable,
            outcome.false_exposed,
            outcome.under_credit,
            outcome.limitation_correct,
        ]
        .iter()
        .any(Option::is_some)
    {
        violations.push(format!(
            "{subject}.reference_outcome: inconclusive or invalid rows carry no outcome labels"
        ));
    }
    if row.non_claims.is_empty() {
        violations.push(format!("{subject}.non_claims: must be non-empty"));
    }
}

fn summarize(packet: &ReleaseJudgments) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn repository_root() -> &'static Path {
        Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/.."))
    }

    fn canonical() -> Result<(Vec<u8>, RustJudgedPanelManifest, serde_json::Value), String> {
        let root = repository_root();
        let selection = check_release_selection_at(root)?;
        let bytes =
            fs::read(root.join(RELEASE_SELECTION_PATH)).map_err(|error| error.to_string())?;
        let body = fs::read_to_string(root.join(RELEASE_JUDGMENTS_PATH))
            .map_err(|error| error.to_string())?;
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
            |value| {
                row_mut(value, "p1744-quiet-test-only")["expected_direction"] = "should_gap".into()
            },
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
}
