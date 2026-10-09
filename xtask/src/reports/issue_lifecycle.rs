//! `cargo xtask issue-lifecycle-scorecard [--captured <corpus.json>]` (#4929,
//! RIPR-SPEC-0218): runs the committed issue-lifecycle fixture corpus through
//! the typed counting-law validator in `crate::issue_lifecycle_attempt`,
//! evaluates every committed expectation, and projects one deterministic
//! `IssueLifecycleScorecardV1` DTO to JSON and Markdown. Both projections
//! derive from the same assessed rows, so prose cannot strengthen machine
//! state. The gate fails closed when a scenario's live outcome drifts from
//! its committed expectation.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::issue_lifecycle_attempt::{
    ISSUE_LIFECYCLE_CLAIM_BOUNDARY, ISSUE_LIFECYCLE_CORPUS_SCHEMA_VERSION,
    ISSUE_LIFECYCLE_FIXTURE_CORPUS_SCHEMA_VERSION, ISSUE_LIFECYCLE_SCORECARD_SCHEMA_VERSION,
    IssueLifecycleDispositionV1, IssueLifecycleFixtureCorpusV1, IssueLifecycleRowAssessmentV1,
    assess_issue_lifecycle_attempt, issue_lifecycle_row_digest, load_issue_lifecycle_corpus,
    load_issue_lifecycle_fixture_corpus, missing_issue_lifecycle_required_scenarios,
};

const DEFAULT_CAPTURED_PATH: &str = "fixtures/issue_lifecycle_attempts/corpus.json";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum IssueLifecycleRateStateV1 {
    NotMeasured,
    Measured,
}

/// A closed rate projection: absent trustworthy data stays `not_measured`
/// and never renders as a fake zero or hundred percent.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct IssueLifecycleRateV1 {
    pub state: IssueLifecycleRateStateV1,
    pub numerator: usize,
    pub denominator: usize,
}

/// One counted real lifecycle after deduplication.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct IssueLifecycleScorecardRowV1 {
    pub lifecycle_id: String,
    pub observation_key: String,
    pub disposition: IssueLifecycleDispositionV1,
    pub implementation_success: bool,
    pub observations: usize,
}

/// Per-disposition totals over the real, deduplicated lifecycles.
/// Dispositions with zero lifecycles keep a stable zero row so negative and
/// closed dispositions never disappear from the projection.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct IssueLifecycleDispositionTotalsV1 {
    pub disposition: IssueLifecycleDispositionV1,
    pub rows: usize,
}

/// One rejected row; rejected rows stay visible and enter no denominator.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct IssueLifecycleRejectedRowV1 {
    pub lifecycle_id: String,
    pub observation_key: String,
    pub synthetic: bool,
    pub reasons: Vec<String>,
}

/// The deterministic scorecard projection. With zero real lifecycles every
/// rate stays `not_measured`: this is the honest empty report the contract
/// guarantees before any real issue is counted.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct IssueLifecycleScorecardV1 {
    pub schema_version: String,
    pub claim_boundary: String,
    pub corpus_path: String,
    pub corpus_identity: String,
    pub real_rows: Vec<IssueLifecycleScorecardRowV1>,
    pub disposition_totals: Vec<IssueLifecycleDispositionTotalsV1>,
    pub real_lifecycles: usize,
    pub implementation_successes: usize,
    pub implementation_success_rate: IssueLifecycleRateV1,
    pub synthetic_lifecycles: usize,
    pub rejected_rows: Vec<IssueLifecycleRejectedRowV1>,
}

fn workspace_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(relative)
}

/// Corpus identity: the sorted retained identities of every row, real and
/// synthetic. Reordered inputs therefore preserve the identity, and a changed
/// row changes the identity.
pub(crate) fn issue_lifecycle_corpus_identity(
    rows: &[crate::issue_lifecycle_attempt::IssueLifecycleAttemptV1],
) -> Result<String, String> {
    let mut identities = Vec::with_capacity(rows.len());
    for row in rows {
        identities.push(issue_lifecycle_row_digest(row)?);
    }
    identities.sort();
    Ok(crate::blind_journey::sha256_hex(
        identities.join("\n").as_bytes(),
    ))
}

/// Deduplicate assessed rows by `observation_key`: one lifecycle observed
/// through several roles or imports remains one lifecycle. Observations that
/// disagree on identity, counted flag or disposition conflict and every
/// conflicting row is rejected, so a malformed duplicate cannot launder into
/// a second lifecycle.
pub(crate) fn deduplicate_assessed_lifecycles(
    assessed: &[IssueLifecycleRowAssessmentV1],
) -> (
    Vec<(IssueLifecycleRowAssessmentV1, usize)>,
    Vec<IssueLifecycleRejectedRowV1>,
) {
    let mut groups: std::collections::BTreeMap<String, Vec<&IssueLifecycleRowAssessmentV1>> =
        std::collections::BTreeMap::new();
    for row in assessed {
        groups
            .entry(row.observation_key.clone())
            .or_default()
            .push(row);
    }
    let mut counted = Vec::new();
    let mut rejected = Vec::new();
    for (_key, group) in groups {
        // Observations of one lifecycle must agree on everything that defines
        // the lifecycle: identity, counted flag and disposition. A
        // digest-corrupted duplicate therefore conflicts instead of hiding
        // behind input order, and the whole group stays visible as rejected.
        let identity = group[0].identity.clone();
        let reference = group[0];
        let conflicting = group.iter().any(|row| {
            row.identity != identity
                || row.counted != reference.counted
                || row.disposition != reference.disposition
        });
        if conflicting {
            for row in group {
                rejected.push(IssueLifecycleRejectedRowV1 {
                    lifecycle_id: row.lifecycle_id.clone(),
                    observation_key: row.observation_key.clone(),
                    synthetic: row.synthetic,
                    reasons: vec![format!(
                        "conflicting duplicate observations under observation_key `{}`",
                        row.observation_key
                    )],
                });
            }
            continue;
        }
        // The representative is the smallest lifecycle id so the projection
        // is stable regardless of input order.
        let representative = group
            .iter()
            .min_by(|left, right| left.lifecycle_id.cmp(&right.lifecycle_id))
            .map(|row| (*row).clone());
        if let Some(representative) = representative {
            counted.push((representative, group.len()));
        }
    }
    (counted, rejected)
}

/// Build the scorecard over one set of assessed rows. Synthetic rows are
/// counted separately and never enter real denominators.
pub(crate) fn build_issue_lifecycle_scorecard(
    assessed: &[IssueLifecycleRowAssessmentV1],
    corpus_identity: String,
    corpus_path: &str,
) -> IssueLifecycleScorecardV1 {
    let (deduped, mut rejected) = deduplicate_assessed_lifecycles(assessed);
    let mut real_rows: Vec<IssueLifecycleScorecardRowV1> = Vec::new();
    let mut totals: std::collections::BTreeMap<IssueLifecycleDispositionV1, usize> =
        IssueLifecycleDispositionV1::all()
            .into_iter()
            .map(|disposition| (disposition, 0usize))
            .collect();
    let mut real_lifecycles = 0usize;
    let mut implementation_successes = 0usize;
    let mut synthetic_lifecycles = 0usize;
    let mut rejected_from_dedupe = Vec::new();
    for (row, observations) in deduped {
        if !row.counted {
            rejected_from_dedupe.push(IssueLifecycleRejectedRowV1 {
                lifecycle_id: row.lifecycle_id.clone(),
                observation_key: row.observation_key.clone(),
                synthetic: row.synthetic,
                reasons: row.reasons.clone(),
            });
            continue;
        }
        let disposition = row
            .disposition
            .unwrap_or(IssueLifecycleDispositionV1::NotRun);
        if row.synthetic {
            synthetic_lifecycles += 1;
            continue;
        }
        real_lifecycles += 1;
        if disposition.is_implementation_success() {
            implementation_successes += 1;
        }
        *totals.entry(disposition).or_insert(0) += 1;
        real_rows.push(IssueLifecycleScorecardRowV1 {
            lifecycle_id: row.lifecycle_id.clone(),
            observation_key: row.observation_key.clone(),
            disposition,
            implementation_success: disposition.is_implementation_success(),
            observations,
        });
    }
    rejected.append(&mut rejected_from_dedupe);
    real_rows.sort_by(|left, right| {
        (&left.lifecycle_id, &left.observation_key)
            .cmp(&(&right.lifecycle_id, &right.observation_key))
    });
    rejected.sort_by(|left, right| {
        (
            &left.lifecycle_id,
            &left.observation_key,
            left.synthetic,
            &left.reasons,
        )
            .cmp(&(
                &right.lifecycle_id,
                &right.observation_key,
                right.synthetic,
                &right.reasons,
            ))
    });
    let implementation_success_rate = if real_lifecycles == 0 {
        IssueLifecycleRateV1 {
            state: IssueLifecycleRateStateV1::NotMeasured,
            numerator: 0,
            denominator: 0,
        }
    } else {
        IssueLifecycleRateV1 {
            state: IssueLifecycleRateStateV1::Measured,
            numerator: implementation_successes,
            denominator: real_lifecycles,
        }
    };
    IssueLifecycleScorecardV1 {
        schema_version: ISSUE_LIFECYCLE_SCORECARD_SCHEMA_VERSION.to_string(),
        claim_boundary: ISSUE_LIFECYCLE_CLAIM_BOUNDARY.to_string(),
        corpus_path: corpus_path.to_string(),
        corpus_identity,
        real_rows,
        disposition_totals: IssueLifecycleDispositionV1::all()
            .into_iter()
            .map(|disposition| IssueLifecycleDispositionTotalsV1 {
                disposition,
                rows: totals.get(&disposition).copied().unwrap_or(0),
            })
            .collect(),
        real_lifecycles,
        implementation_successes,
        implementation_success_rate,
        synthetic_lifecycles,
        rejected_rows: rejected,
    }
}

/// Run one parsed fixture corpus through the live validator and evaluate
/// every committed expectation. The returned scorecard is `Ok` even on
/// expectation drift; `expectation_failures` carries the drift so callers
/// decide, mirroring the blind-journey contract gate.
pub(crate) fn assess_issue_lifecycle_fixture_corpus(
    corpus: &IssueLifecycleFixtureCorpusV1,
    corpus_path: &str,
) -> (IssueLifecycleScorecardV1, Vec<String>) {
    let rows: Vec<crate::issue_lifecycle_attempt::IssueLifecycleAttemptV1> = corpus
        .scenarios
        .iter()
        .map(|scenario| scenario.attempt.clone())
        .collect();
    let corpus_identity = match issue_lifecycle_corpus_identity(&rows) {
        Ok(identity) => identity,
        Err(error) => format!("corpus_identity_error:{error}"),
    };
    let assessed: Vec<IssueLifecycleRowAssessmentV1> = corpus
        .scenarios
        .iter()
        .map(|scenario| assess_issue_lifecycle_attempt(&scenario.attempt))
        .collect();
    let mut failures = Vec::new();
    for (scenario, assessment) in corpus.scenarios.iter().zip(assessed.iter()) {
        if assessment.counted != scenario.expected.countable {
            failures.push(format!(
                "scenario `{}` drifted: expected countable={}, got counted={} reasons={:?}",
                scenario.id, scenario.expected.countable, assessment.counted, assessment.reasons
            ));
        }
        if let Some(expected) = scenario.expected.disposition
            && assessment.disposition != Some(expected)
        {
            failures.push(format!(
                "scenario `{}` drifted: expected disposition {expected:?}, got {:?} reasons={:?}",
                scenario.id, assessment.disposition, assessment.reasons
            ));
        }
        for needle in &scenario.expected.reason_contains {
            if !assessment
                .reasons
                .iter()
                .any(|reason| reason.contains(needle.as_str()))
            {
                failures.push(format!(
                    "scenario `{}` drifted: expected a reason containing `{needle}`, got {:?}",
                    scenario.id, assessment.reasons
                ));
            }
        }
    }
    let scorecard = build_issue_lifecycle_scorecard(&assessed, corpus_identity, corpus_path);
    (scorecard, failures)
}

fn parse_captured_arg(args: &[String]) -> Result<String, String> {
    const USAGE: &str = "usage: cargo xtask issue-lifecycle-scorecard [--captured <corpus.json>]";
    let mut captured = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--captured" | "--corpus" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| format!("missing value for {}\n{USAGE}", args[index]))?;
                if captured.replace(value.clone()).is_some() {
                    return Err(format!("duplicate corpus path\n{USAGE}"));
                }
                index += 2;
            }
            "--help" | "-h" => return Err(USAGE.to_string()),
            other => return Err(format!("unknown argument `{other}`\n{USAGE}")),
        }
    }
    Ok(captured.unwrap_or_else(|| DEFAULT_CAPTURED_PATH.to_string()))
}

pub(crate) fn issue_lifecycle_scorecard_report(args: &[String]) -> Result<(), String> {
    let captured = parse_captured_arg(args)?;
    let body = fs::read_to_string(workspace_path(&captured))
        .map_err(|error| format!("read issue lifecycle corpus {captured}: {error}"))?;
    let parsed: serde_json::Value = serde_json::from_str(&body)
        .map_err(|error| format!("parse issue lifecycle corpus {captured}: {error}"))?;
    let schema_version = parsed
        .get("schema_version")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("issue lifecycle corpus {captured} is missing `schema_version`"))?;
    let (scorecard, failures) = match schema_version {
        ISSUE_LIFECYCLE_FIXTURE_CORPUS_SCHEMA_VERSION => {
            let corpus = load_issue_lifecycle_fixture_corpus(&body)?;
            let missing = missing_issue_lifecycle_required_scenarios(
                corpus.scenarios.iter().map(|scenario| scenario.id.as_str()),
            );
            if !missing.is_empty() {
                return Err(format!(
                    "issue lifecycle fixture corpus is missing required scenarios: {missing:?}"
                ));
            }
            assess_issue_lifecycle_fixture_corpus(&corpus, &captured)
        }
        ISSUE_LIFECYCLE_CORPUS_SCHEMA_VERSION => {
            let corpus = load_issue_lifecycle_corpus(&body)?;
            let corpus_identity = issue_lifecycle_corpus_identity(&corpus.rows)?;
            let assessed: Vec<IssueLifecycleRowAssessmentV1> = corpus
                .rows
                .iter()
                .map(assess_issue_lifecycle_attempt)
                .collect();
            (
                build_issue_lifecycle_scorecard(&assessed, corpus_identity, &captured),
                Vec::new(),
            )
        }
        other => {
            return Err(format!(
                "unsupported issue lifecycle corpus schema `{other}` in {captured}"
            ));
        }
    };
    if !failures.is_empty() {
        return Err(format!(
            "issue lifecycle fixture corpus drifted: {failures:?}"
        ));
    }
    let json_body = issue_lifecycle_scorecard_json(&scorecard)?;
    crate::write_report("issue-lifecycle-scorecard.json", &json_body)?;
    crate::write_report(
        "issue-lifecycle-scorecard.md",
        &issue_lifecycle_scorecard_markdown(&scorecard),
    )?;
    println!("{json_body}");
    Ok(())
}

pub(crate) fn issue_lifecycle_scorecard_json(
    scorecard: &IssueLifecycleScorecardV1,
) -> Result<String, String> {
    let body = serde_json::to_string_pretty(scorecard)
        .map_err(|error| format!("serialize issue lifecycle scorecard: {error}"))?;
    Ok(format!("{body}\n"))
}

pub(crate) fn issue_lifecycle_scorecard_markdown(scorecard: &IssueLifecycleScorecardV1) -> String {
    let mut body = String::new();
    body.push_str("# Issue lifecycle scorecard\n\n");
    body.push_str(&format!("Claim boundary: {}\n\n", scorecard.claim_boundary));
    let rate = match scorecard.implementation_success_rate.state {
        IssueLifecycleRateStateV1::Measured => format!(
            "{}/{}",
            scorecard.implementation_success_rate.numerator,
            scorecard.implementation_success_rate.denominator
        ),
        IssueLifecycleRateStateV1::NotMeasured => {
            "not measured (zero real lifecycles; no rate is fabricated)".to_string()
        }
    };
    body.push_str(&format!(
        "- corpus: `{}`\n- corpus identity: `{}`\n- real lifecycles: {}\n- implementation successes: {}\n- implementation success rate: {}\n- synthetic mechanics lifecycles (outside every real denominator): {}\n- rejected rows (visible, counted nowhere): {}\n\n",
        scorecard.corpus_path,
        scorecard.corpus_identity,
        scorecard.real_lifecycles,
        scorecard.implementation_successes,
        rate,
        scorecard.synthetic_lifecycles,
        scorecard.rejected_rows.len()
    ));
    body.push_str("| disposition | rows |\n");
    body.push_str("| --- | ---: |\n");
    for totals in &scorecard.disposition_totals {
        body.push_str(&format!("| {:?} | {} |\n", totals.disposition, totals.rows));
    }
    if !scorecard.real_rows.is_empty() {
        body.push_str("\n| lifecycle | disposition | implementation success | observations |\n");
        body.push_str("| --- | --- | --- | ---: |\n");
        for row in &scorecard.real_rows {
            body.push_str(&format!(
                "| {} | {:?} | {} | {} |\n",
                row.lifecycle_id, row.disposition, row.implementation_success, row.observations
            ));
        }
    }
    for row in &scorecard.rejected_rows {
        body.push_str(&format!(
            "\n## rejected: {} (synthetic: {})\n\n",
            row.lifecycle_id, row.synthetic
        ));
        for reason in &row.reasons {
            body.push_str(&format!("- {reason}\n"));
        }
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::issue_lifecycle_attempt::{
        ISSUE_LIFECYCLE_ATTEMPT_SCHEMA_VERSION, IssueLifecycleAttemptV1,
        IssueLifecycleBaseAttemptRefV1, IssueLifecycleBurnDownStateV1, IssueLifecycleBurnDownV1,
        IssueLifecycleClaimEventKindV1, IssueLifecycleClaimEventV1, IssueLifecycleCloseoutStateV1,
        IssueLifecycleCloseoutV1, IssueLifecycleContextV1, IssueLifecycleContractArtifactsV1,
        IssueLifecycleContractDecisionV1, IssueLifecycleEvidenceRefV1,
        IssueLifecycleExecutionRefsV1, IssueLifecycleInformationCompletenessV1,
        IssueLifecycleInitialV1, IssueLifecycleIntakeV1, IssueLifecycleIssueSnapshotV1,
        IssueLifecyclePlanV1, IssueLifecycleProgressV1,
    };

    fn hex64(fill: char) -> String {
        fill.to_string().repeat(64)
    }

    fn sample_lifecycle(
        lifecycle_id: &str,
        observation_key: &str,
        synthetic: bool,
        disposition: IssueLifecycleDispositionV1,
    ) -> Result<IssueLifecycleAttemptV1, String> {
        let mut row = IssueLifecycleAttemptV1 {
            schema_version: ISSUE_LIFECYCLE_ATTEMPT_SCHEMA_VERSION.to_string(),
            lifecycle_id: lifecycle_id.to_string(),
            observation_key: observation_key.to_string(),
            synthetic,
            disposition,
            base_attempts: vec![IssueLifecycleBaseAttemptRefV1 {
                attempt_id: format!("attempt-{lifecycle_id}"),
                shared_facts: vec!["context".to_string(), "execution".to_string()],
            }],
            issue: IssueLifecycleIssueSnapshotV1 {
                snapshot_id: format!("issue-snapshot:sha256:{}", hex64('a')),
                issue_ref: "operator/target#1234".to_string(),
                comments_ref: format!("issue-comments:sha256:{}", hex64('b')),
                labels_ref: format!("issue-labels:sha256:{}", hex64('c')),
                assignees_ref: None,
                milestone_ref: None,
            },
            initial: IssueLifecycleInitialV1 {
                information_completeness: IssueLifecycleInformationCompletenessV1::Complete,
                issue_family: "narrow_bug".to_string(),
            },
            context: IssueLifecycleContextV1 {
                current_main: "main-sha".to_string(),
                relevant_prs: vec![format!("pr-{lifecycle_id}")],
                portfolio: "campaign-sample".to_string(),
                selected_work: "issue-sample".to_string(),
                claim_ids: vec![format!("claim-{lifecycle_id}")],
            },
            intake: IssueLifecycleIntakeV1 {
                evidence: vec![IssueLifecycleEvidenceRefV1 {
                    identity: format!("intake-packet:sha256:{}", hex64('e')),
                    bytes: 1024,
                }],
                missing_evidence_questions: Vec::new(),
            },
            contract_decision: IssueLifecycleContractDecisionV1 {
                spec_required: false,
                decision_rationale: "narrow accepted-contract bug; no new spec".to_string(),
                root_disposition: Some("root-accepted".to_string()),
            },
            contract_artifacts: IssueLifecycleContractArtifactsV1 {
                proposal: None,
                spec: None,
                adr: None,
                challenge: None,
                amendments: Vec::new(),
                acceptance: Some(format!("contract-acceptance:sha256:{}", hex64('f'))),
            },
            plan: IssueLifecyclePlanV1 {
                plan_id: format!("plan-{lifecycle_id}"),
                work_items: vec![format!("work-item-{lifecycle_id}")],
                dependencies: Vec::new(),
                acceptance_coverage: vec!["acceptance-row-sample".to_string()],
            },
            claim_events: vec![IssueLifecycleClaimEventV1 {
                claim_id: format!("claim-{lifecycle_id}"),
                event: IssueLifecycleClaimEventKindV1::Claimed,
                detail: "durable exclusive writer claim".to_string(),
            }],
            execution_refs: IssueLifecycleExecutionRefsV1 {
                pr: Some(format!("pr-{lifecycle_id}")),
                review: Some(format!("review-{lifecycle_id}")),
                checks: vec![format!("ci-{lifecycle_id}")],
                verification: Some(format!("verification-{lifecycle_id}")),
                merge: Some(format!("merge-{lifecycle_id}")),
                current_main: "main-sha".to_string(),
            },
            progress: IssueLifecycleProgressV1 {
                mutation_plan: None,
                before_digest: None,
                after_digest: None,
                suppressed_unchanged: false,
            },
            burn_down: IssueLifecycleBurnDownV1 {
                state: IssueLifecycleBurnDownStateV1::Closed,
                uncovered_rows: Vec::new(),
                contradicted_rows: Vec::new(),
                deferred_rows: Vec::new(),
            },
            closeout: IssueLifecycleCloseoutV1 {
                state: IssueLifecycleCloseoutStateV1::Completed,
                reason: Some("acceptance covered at current main".to_string()),
                remaining_limitations: vec!["sample limitation".to_string()],
                current_head_verification: Some(format!(
                    "current-head-verification:sha256:{}",
                    hex64('1')
                )),
            },
            limitations: vec!["sample limitation".to_string()],
            non_claims: vec!["sample non-claim".to_string()],
            row_digest: String::new(),
        };
        row.row_digest = issue_lifecycle_row_digest(&row)?;
        Ok(row)
    }

    fn qualified_row(
        lifecycle_id: &str,
        observation_key: &str,
        synthetic: bool,
    ) -> Result<IssueLifecycleAttemptV1, String> {
        let mut row = sample_lifecycle(
            lifecycle_id,
            observation_key,
            synthetic,
            IssueLifecycleDispositionV1::QualifiedOnePr,
        )?;
        row.closeout.state = IssueLifecycleCloseoutStateV1::NotStarted;
        row.closeout.reason = None;
        row.closeout.current_head_verification = None;
        row.contract_artifacts.acceptance = None;
        row.execution_refs.merge = None;
        row.execution_refs.pr = None;
        row.execution_refs.review = None;
        row.execution_refs.verification = None;
        row.execution_refs.checks = Vec::new();
        row.row_digest = issue_lifecycle_row_digest(&row)?;
        Ok(row)
    }

    fn assessed(row: IssueLifecycleAttemptV1) -> IssueLifecycleRowAssessmentV1 {
        assess_issue_lifecycle_attempt(&row)
    }

    #[test]
    fn empty_corpus_reports_no_success_rate() -> Result<(), String> {
        let corpus = load_issue_lifecycle_corpus(
            r#"{"schema_version":"issue_lifecycle_corpus.v1","rows":[]}"#,
        )?;
        if !corpus.rows.is_empty() {
            return Err("the empty corpus fixture must parse to zero rows".to_string());
        }
        let scorecard =
            build_issue_lifecycle_scorecard(&[], "empty".to_string(), "fixtures/empty.json");
        if scorecard.real_lifecycles != 0 || scorecard.implementation_successes != 0 {
            return Err("an empty corpus must keep zero real lifecycles".to_string());
        }
        if scorecard.implementation_success_rate.state != IssueLifecycleRateStateV1::NotMeasured {
            return Err(
                "an empty corpus must keep the implementation success rate not measured"
                    .to_string(),
            );
        }
        let markdown = issue_lifecycle_scorecard_markdown(&scorecard);
        if !markdown.contains("not measured") {
            return Err("the honest empty report must say the rate is not measured".to_string());
        }
        if !scorecard
            .disposition_totals
            .iter()
            .all(|totals| totals.rows == 0)
        {
            return Err("every disposition row must stay visible with zero rows".to_string());
        }
        Ok(())
    }

    #[test]
    fn synthetic_rows_never_enter_real_denominators() -> Result<(), String> {
        let row = sample_lifecycle(
            "lifecycle-synthetic",
            "observation-synthetic",
            true,
            IssueLifecycleDispositionV1::Completed,
        )?;
        let scorecard = build_issue_lifecycle_scorecard(
            &[assessed(row)],
            "synthetic".to_string(),
            "fixtures/synthetic.json",
        );
        if scorecard.real_lifecycles != 0
            || scorecard.implementation_success_rate.state != IssueLifecycleRateStateV1::NotMeasured
        {
            return Err(format!(
                "synthetic rows leaked into real denominators: real={} rate={:?}",
                scorecard.real_lifecycles, scorecard.implementation_success_rate
            ));
        }
        if scorecard.synthetic_lifecycles != 1 {
            return Err(format!(
                "synthetic lifecycles must be counted separately, got {}",
                scorecard.synthetic_lifecycles
            ));
        }
        Ok(())
    }

    #[test]
    fn completed_row_enters_the_implementation_success_rate() -> Result<(), String> {
        let row = sample_lifecycle(
            "lifecycle-real",
            "observation-real",
            false,
            IssueLifecycleDispositionV1::Completed,
        )?;
        let scorecard = build_issue_lifecycle_scorecard(
            &[assessed(row)],
            "real".to_string(),
            "fixtures/real.json",
        );
        if scorecard.real_lifecycles != 1 || scorecard.implementation_successes != 1 {
            return Err(format!(
                "one valid real row must count once as implementation success, got real={} successes={}",
                scorecard.real_lifecycles, scorecard.implementation_successes
            ));
        }
        if scorecard.implementation_success_rate
            != (IssueLifecycleRateV1 {
                state: IssueLifecycleRateStateV1::Measured,
                numerator: 1,
                denominator: 1,
            })
        {
            return Err("the implementation success rate must be measured 1/1".to_string());
        }
        Ok(())
    }

    #[test]
    fn negative_dispositions_never_count_as_implementation_success() -> Result<(), String> {
        let mut assessed_rows = Vec::new();
        for (index, disposition) in [
            IssueLifecycleDispositionV1::Duplicate,
            IssueLifecycleDispositionV1::AlreadySatisfied,
            IssueLifecycleDispositionV1::ClosedNotPlanned,
            IssueLifecycleDispositionV1::Blocked,
            IssueLifecycleDispositionV1::NeedsEvidence,
            IssueLifecycleDispositionV1::PartiallyLanded,
            IssueLifecycleDispositionV1::MergedPendingCloseout,
            IssueLifecycleDispositionV1::VerificationFailed,
        ]
        .into_iter()
        .enumerate()
        {
            let row = qualified_row(
                &format!("lifecycle-negative-{index}"),
                &format!("observation-negative-{index}"),
                false,
            )?;
            let mut row = row;
            row.disposition = disposition;
            row.row_digest = issue_lifecycle_row_digest(&row)?;
            assessed_rows.push(assessed(row));
        }
        let scorecard = build_issue_lifecycle_scorecard(
            &assessed_rows,
            "negatives".to_string(),
            "fixtures/negatives.json",
        );
        if scorecard.real_lifecycles != assessed_rows.len() {
            return Err(format!(
                "every negative row must stay counted and visible, got {}",
                scorecard.real_lifecycles
            ));
        }
        if scorecard.implementation_successes != 0
            || scorecard.implementation_success_rate.state != IssueLifecycleRateStateV1::Measured
            || scorecard.implementation_success_rate.numerator != 0
            || scorecard.implementation_success_rate.denominator != assessed_rows.len()
        {
            return Err(format!(
                "negative dispositions must not count as implementation success, got successes={} rate={:?}",
                scorecard.implementation_successes, scorecard.implementation_success_rate
            ));
        }
        let totals: std::collections::BTreeMap<IssueLifecycleDispositionV1, usize> = scorecard
            .disposition_totals
            .iter()
            .map(|totals| (totals.disposition, totals.rows))
            .collect();
        if totals.get(&IssueLifecycleDispositionV1::Duplicate) != Some(&1)
            || totals.get(&IssueLifecycleDispositionV1::Completed) != Some(&0)
        {
            return Err("per-disposition totals must stay stable and exact".to_string());
        }
        Ok(())
    }

    #[test]
    fn duplicate_observations_of_one_lifecycle_deduplicate() -> Result<(), String> {
        let first = sample_lifecycle(
            "lifecycle-duplicate",
            "observation-duplicate",
            false,
            IssueLifecycleDispositionV1::Completed,
        )?;
        let second = sample_lifecycle(
            "lifecycle-duplicate",
            "observation-duplicate",
            false,
            IssueLifecycleDispositionV1::Completed,
        )?;
        let scorecard = build_issue_lifecycle_scorecard(
            &[assessed(first), assessed(second)],
            "duplicates".to_string(),
            "fixtures/duplicates.json",
        );
        if scorecard.real_lifecycles != 1 {
            return Err(format!(
                "duplicate observations must remain one lifecycle, got {}",
                scorecard.real_lifecycles
            ));
        }
        let row = scorecard
            .real_rows
            .first()
            .ok_or_else(|| "the deduplicated row must be present".to_string())?;
        if row.observations != 2 || row.lifecycle_id != "lifecycle-duplicate" {
            return Err(format!(
                "the representative must keep one identity with two observations, got {:?}",
                row
            ));
        }
        Ok(())
    }

    #[test]
    fn conflicting_duplicate_observations_reject_both() -> Result<(), String> {
        let first = sample_lifecycle(
            "lifecycle-conflict-a",
            "observation-conflict",
            false,
            IssueLifecycleDispositionV1::Completed,
        )?;
        let mut second = sample_lifecycle(
            "lifecycle-conflict-b",
            "observation-conflict",
            false,
            IssueLifecycleDispositionV1::Completed,
        )?;
        second.disposition = IssueLifecycleDispositionV1::Blocked;
        second.closeout.state = IssueLifecycleCloseoutStateV1::NotStarted;
        second.closeout.reason = None;
        second.closeout.current_head_verification = None;
        second.contract_artifacts.acceptance = None;
        second.execution_refs.merge = None;
        second.row_digest = issue_lifecycle_row_digest(&second)?;
        let scorecard = build_issue_lifecycle_scorecard(
            &[assessed(first), assessed(second)],
            "conflicts".to_string(),
            "fixtures/conflicts.json",
        );
        if scorecard.real_lifecycles != 0 {
            return Err("conflicting observations must not count as a lifecycle".to_string());
        }
        if scorecard.rejected_rows.len() != 2 {
            return Err(format!(
                "both conflicting observations must stay visible as rejected rows, got {}",
                scorecard.rejected_rows.len()
            ));
        }
        Ok(())
    }

    #[test]
    fn reordered_inputs_preserve_the_corpus_identity() -> Result<(), String> {
        let first = sample_lifecycle(
            "lifecycle-order-a",
            "observation-order-a",
            false,
            IssueLifecycleDispositionV1::Completed,
        )?;
        let second = sample_lifecycle(
            "lifecycle-order-b",
            "observation-order-b",
            true,
            IssueLifecycleDispositionV1::Completed,
        )?;
        let identity_ab = issue_lifecycle_corpus_identity(&[first.clone(), second.clone()])?;
        let identity_ba = issue_lifecycle_corpus_identity(&[second, first])?;
        if identity_ab != identity_ba {
            return Err("reordered inputs must preserve the corpus digest".to_string());
        }
        Ok(())
    }

    #[test]
    fn json_and_markdown_derive_from_one_dto_deterministically() -> Result<(), String> {
        let row = sample_lifecycle(
            "lifecycle-projection",
            "observation-projection",
            false,
            IssueLifecycleDispositionV1::Completed,
        )?;
        let scorecard = build_issue_lifecycle_scorecard(
            &[assessed(row)],
            "projection".to_string(),
            "fixtures/projection.json",
        );
        let json_first = issue_lifecycle_scorecard_json(&scorecard)?;
        let json_second = issue_lifecycle_scorecard_json(&scorecard)?;
        if json_first != json_second {
            return Err("the JSON projection must be byte-identical across runs".to_string());
        }
        if !json_first.ends_with('\n') || json_first.ends_with("\n\n") {
            return Err("the JSON projection must end in exactly one newline".to_string());
        }
        let markdown = issue_lifecycle_scorecard_markdown(&scorecard);
        if !markdown.contains("lifecycle-projection") || !markdown.contains("Completed") {
            return Err(
                "the Markdown projection must name the lifecycle and the disposition".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn parse_captured_accepts_at_most_one_path() -> Result<(), String> {
        let parsed = parse_captured_arg(&["--captured".to_string(), "some.json".to_string()])?;
        if parsed != "some.json" {
            return Err(format!("expected `some.json`, got {parsed}"));
        }
        let aliased = parse_captured_arg(&["--corpus".to_string(), "other.json".to_string()])?;
        if aliased != "other.json" {
            return Err(format!("expected `other.json`, got {aliased}"));
        }
        let default = parse_captured_arg(&[])?;
        if default != DEFAULT_CAPTURED_PATH {
            return Err(format!("expected the default captured path, got {default}"));
        }
        for args in [
            vec!["--captured".to_string()],
            vec!["--other".to_string()],
            vec![
                "--captured".to_string(),
                "a.json".to_string(),
                "--corpus".to_string(),
                "b.json".to_string(),
            ],
        ] {
            match parse_captured_arg(&args) {
                Err(_error) => {}
                Ok(path) => {
                    return Err(format!(
                        "malformed arguments were accepted as {path}: {args:?}"
                    ));
                }
            }
        }
        Ok(())
    }

    #[test]
    fn digest_corrupted_duplicate_conflicts_in_either_input_order() -> Result<(), String> {
        let valid = sample_lifecycle(
            "lifecycle-digest-dup",
            "observation-digest-dup",
            false,
            IssueLifecycleDispositionV1::Completed,
        )?;
        let mut corrupted = sample_lifecycle(
            "lifecycle-digest-dup",
            "observation-digest-dup",
            false,
            IssueLifecycleDispositionV1::Completed,
        )?;
        corrupted.row_digest = String::new();
        let build = |rows: Vec<IssueLifecycleRowAssessmentV1>| {
            build_issue_lifecycle_scorecard(&rows, "digest-dup".to_string(), "fixtures/digest.json")
        };
        let forward = build(vec![assessed(valid.clone()), assessed(corrupted.clone())]);
        let backward = build(vec![assessed(corrupted), assessed(valid)]);
        let left = serde_json::to_string(&forward).map_err(|error| error.to_string())?;
        let right = serde_json::to_string(&backward).map_err(|error| error.to_string())?;
        if left != right {
            return Err(
                "reordered valid-plus-corrupted duplicates must give one scorecard".to_string(),
            );
        }
        if forward.real_lifecycles != 0 || forward.rejected_rows.len() != 2 {
            return Err(format!(
                "a digest-corrupted duplicate must reject the whole group and stay visible, got real={} rejected={}",
                forward.real_lifecycles,
                forward.rejected_rows.len()
            ));
        }
        Ok(())
    }
}
