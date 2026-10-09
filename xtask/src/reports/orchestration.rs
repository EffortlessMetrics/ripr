//! `cargo xtask orchestration-scorecard [--captured <path>]` (#4925,
//! RIPR-SPEC-0213): runs the committed orchestration fixture corpus through
//! the typed counting-law validator in `crate::orchestration_attempt`,
//! evaluates every committed expectation, and projects one deterministic
//! `OrchestrationScorecardV1` DTO to JSON and Markdown. Both projections
//! derive from the same assessed rows, so prose cannot strengthen machine
//! state. The gate fails closed when a scenario's live outcome drifts from
//! its committed expectation.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::orchestration_attempt::{
    AttemptDispositionV1, AttemptStrategyV1, ORCHESTRATION_CLAIM_BOUNDARY,
    OrchestrationFixtureCorpusV1, OrchestrationRowAssessmentV1, assess_orchestration_attempt,
    load_orchestration_fixture_corpus, missing_orchestration_required_scenarios,
    orchestration_portable_identity,
};

const DEFAULT_CAPTURED_PATH: &str = "fixtures/orchestration_attempt_receipts/corpus.json";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OrchestrationRateStateV1 {
    NotMeasured,
    Measured,
}

/// A closed rate projection: absent trustworthy data stays `not_measured`
/// and never renders as a fake zero or hundred percent.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct OrchestrationRateV1 {
    pub state: OrchestrationRateStateV1,
    pub numerator: usize,
    pub denominator: usize,
}

/// One counted real attempt after deduplication.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct OrchestrationScorecardRowV1 {
    pub attempt_id: String,
    pub observation_key: String,
    pub strategy: AttemptStrategyV1,
    pub disposition: AttemptDispositionV1,
    pub observations: usize,
    pub portable_identity: String,
}

/// One rejected row; rejected rows stay visible and enter no denominator.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct OrchestrationRejectedRowV1 {
    pub attempt_id: String,
    pub observation_key: String,
    pub synthetic: bool,
    pub reasons: Vec<String>,
}

/// Per-strategy totals over the real, deduplicated attempts. Strategies with
/// zero attempts keep a stable zero row so single-agent preference and
/// rejected fan-out never disappear from the projection.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct OrchestrationStrategyTotalsV1 {
    pub strategy: AttemptStrategyV1,
    pub attempts: usize,
    pub completed: usize,
    pub visible_non_completed: usize,
}

/// The deterministic scorecard projection. With zero real attempts every
/// rate stays `not_measured`: this is the honest empty report the contract
/// guarantees before any dogfood begins.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct OrchestrationScorecardV1 {
    pub schema_version: String,
    pub claim_boundary: String,
    pub corpus_path: String,
    pub corpus_identity: String,
    pub real_rows: Vec<OrchestrationScorecardRowV1>,
    pub synthetic_attempts: usize,
    pub rejected_rows: Vec<OrchestrationRejectedRowV1>,
    pub real_attempts: usize,
    pub completed: usize,
    pub completion_rate: OrchestrationRateV1,
    pub strategy_totals: Vec<OrchestrationStrategyTotalsV1>,
}

pub(crate) const ORCHESTRATION_SCORECARD_SCHEMA_VERSION: &str = "orchestration_scorecard.v1";

fn workspace_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(relative)
}

/// Corpus identity: the sorted portable identities of every row, real and
/// synthetic. Reordered inputs therefore preserve the identity, and a changed
/// row changes the identity.
pub(crate) fn orchestration_corpus_identity(
    attempts: &[crate::orchestration_attempt::OrchestrationAttemptV1],
) -> Result<String, String> {
    let mut identities = Vec::with_capacity(attempts.len());
    for attempt in attempts {
        identities.push(orchestration_portable_identity(attempt)?);
    }
    identities.sort();
    Ok(crate::blind_journey::sha256_hex(
        identities.join("\n").as_bytes(),
    ))
}

/// Deduplicate assessed rows by `observation_key`: one work item observed
/// through several roles remains one attempt. Observations that disagree on
/// portable identity conflict and every conflicting row is rejected, so a
/// malformed duplicate cannot launder into a second attempt.
pub(crate) fn deduplicate_assessed_rows(
    assessed: &[OrchestrationRowAssessmentV1],
) -> (
    Vec<(OrchestrationRowAssessmentV1, usize)>,
    Vec<OrchestrationRejectedRowV1>,
) {
    let mut groups: std::collections::BTreeMap<String, Vec<&OrchestrationRowAssessmentV1>> =
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
        // Observations of one attempt must agree on everything that defines
        // the attempt: portable identity, counted flag and disposition. A
        // digest-corrupted duplicate therefore conflicts instead of hiding
        // behind input order, and the whole group stays visible as rejected.
        let identity = group[0].portable_identity.clone();
        let reference = group[0];
        let conflicting = group.iter().any(|row| {
            row.portable_identity != identity
                || row.counted != reference.counted
                || row.disposition != reference.disposition
        });
        if conflicting {
            for row in group {
                rejected.push(OrchestrationRejectedRowV1 {
                    attempt_id: row.attempt_id.clone(),
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
        // The representative is the smallest attempt id so the projection is
        // stable regardless of input order.
        let representative = group
            .iter()
            .min_by(|left, right| left.attempt_id.cmp(&right.attempt_id))
            .map(|row| (*row).clone());
        if let Some(representative) = representative {
            counted.push((representative, group.len()));
        }
    }
    (counted, rejected)
}

/// Build the scorecard over one set of assessed rows. Synthetic rows are
/// counted separately and never enter real denominators.
pub(crate) fn build_orchestration_scorecard(
    assessed: &[OrchestrationRowAssessmentV1],
    corpus_identity: String,
    corpus_path: &str,
) -> OrchestrationScorecardV1 {
    let (deduped, mut rejected) = deduplicate_assessed_rows(assessed);
    let mut real_rows: Vec<OrchestrationScorecardRowV1> = Vec::new();
    let mut synthetic_attempts = 0usize;
    let mut real_attempts = 0usize;
    let mut completed = 0usize;
    let mut totals: std::collections::BTreeMap<AttemptStrategyV1, (usize, usize, usize)> =
        AttemptStrategyV1::all()
            .into_iter()
            .map(|strategy| (strategy, (0usize, 0usize, 0usize)))
            .collect();
    let mut rejected_from_dedupe = Vec::new();
    for (row, observations) in deduped {
        if !row.counted {
            rejected_from_dedupe.push(OrchestrationRejectedRowV1 {
                attempt_id: row.attempt_id.clone(),
                observation_key: row.observation_key.clone(),
                synthetic: row.synthetic,
                reasons: row.reasons.clone(),
            });
            continue;
        }
        let disposition = row.disposition.unwrap_or(AttemptDispositionV1::NotRun);
        if row.synthetic {
            synthetic_attempts += 1;
            continue;
        }
        real_attempts += 1;
        if disposition == AttemptDispositionV1::Completed {
            completed += 1;
        }
        let entry = totals
            .entry(row.strategy)
            .or_insert((0usize, 0usize, 0usize));
        entry.0 += 1;
        if disposition == AttemptDispositionV1::Completed {
            entry.1 += 1;
        } else {
            entry.2 += 1;
        }
        real_rows.push(OrchestrationScorecardRowV1 {
            attempt_id: row.attempt_id.clone(),
            observation_key: row.observation_key.clone(),
            strategy: row.strategy,
            disposition,
            observations,
            portable_identity: row.portable_identity.clone(),
        });
    }
    rejected.append(&mut rejected_from_dedupe);
    real_rows.sort_by(|left, right| left.attempt_id.cmp(&right.attempt_id));
    rejected.sort_by(|left, right| left.attempt_id.cmp(&right.attempt_id));
    let completion_rate = if real_attempts == 0 {
        OrchestrationRateV1 {
            state: OrchestrationRateStateV1::NotMeasured,
            numerator: 0,
            denominator: 0,
        }
    } else {
        OrchestrationRateV1 {
            state: OrchestrationRateStateV1::Measured,
            numerator: completed,
            denominator: real_attempts,
        }
    };
    OrchestrationScorecardV1 {
        schema_version: ORCHESTRATION_SCORECARD_SCHEMA_VERSION.to_string(),
        claim_boundary: ORCHESTRATION_CLAIM_BOUNDARY.to_string(),
        corpus_path: corpus_path.to_string(),
        corpus_identity,
        real_rows,
        synthetic_attempts,
        rejected_rows: rejected,
        real_attempts,
        completed,
        completion_rate,
        strategy_totals: AttemptStrategyV1::all()
            .into_iter()
            .map(|strategy| {
                let (attempts, strategy_completed, visible_non_completed) =
                    totals.get(&strategy).copied().unwrap_or((0, 0, 0));
                OrchestrationStrategyTotalsV1 {
                    strategy,
                    attempts,
                    completed: strategy_completed,
                    visible_non_completed,
                }
            })
            .collect(),
    }
}

/// Run one parsed fixture corpus through the live validator and evaluate
/// every committed expectation. The returned scorecard is `Ok` even on
/// expectation drift; `expectation_failures` carries the drift so callers
/// decide, mirroring the blind-journey contract gate.
pub(crate) fn assess_orchestration_fixture_corpus(
    corpus: &OrchestrationFixtureCorpusV1,
    corpus_path: &str,
) -> (OrchestrationScorecardV1, Vec<String>) {
    let attempts: Vec<crate::orchestration_attempt::OrchestrationAttemptV1> = corpus
        .scenarios
        .iter()
        .map(|scenario| scenario.attempt.clone())
        .collect();
    let corpus_identity = match orchestration_corpus_identity(&attempts) {
        Ok(identity) => identity,
        Err(error) => format!("corpus_identity_error:{error}"),
    };
    let mut assessed = Vec::with_capacity(corpus.scenarios.len());
    let mut identities: std::collections::BTreeMap<String, String> =
        std::collections::BTreeMap::new();
    for scenario in &corpus.scenarios {
        let assessment = assess_orchestration_attempt(&scenario.attempt);
        identities.insert(scenario.id.clone(), assessment.portable_identity.clone());
        assessed.push(assessment);
    }
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
        if let Some(other) = &scenario.expected.same_portable_identity_as {
            let matches = identities
                .get(other)
                .is_some_and(|identity| identity == &assessment.portable_identity);
            if !matches {
                failures.push(format!(
                    "scenario `{}` drifted: portable identity does not match `{other}`",
                    scenario.id
                ));
            }
        }
    }
    let scorecard = build_orchestration_scorecard(&assessed, corpus_identity, corpus_path);
    (scorecard, failures)
}

fn parse_captured_arg(args: &[String]) -> Result<String, String> {
    const USAGE: &str = "usage: cargo xtask orchestration-scorecard [--captured <corpus.json>]";
    let mut captured = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--captured" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| format!("missing value for --captured\n{USAGE}"))?;
                if captured.replace(value.clone()).is_some() {
                    return Err(format!("duplicate --captured\n{USAGE}"));
                }
                index += 2;
            }
            "--help" | "-h" => return Err(USAGE.to_string()),
            other => return Err(format!("unknown argument `{other}`\n{USAGE}")),
        }
    }
    Ok(captured.unwrap_or_else(|| DEFAULT_CAPTURED_PATH.to_string()))
}

pub(crate) fn orchestration_scorecard_report(args: &[String]) -> Result<(), String> {
    let captured = parse_captured_arg(args)?;
    let body = fs::read_to_string(workspace_path(&captured))
        .map_err(|error| format!("read orchestration corpus {captured}: {error}"))?;
    let parsed: serde_json::Value = serde_json::from_str(&body)
        .map_err(|error| format!("parse orchestration corpus {captured}: {error}"))?;
    let schema_version = parsed
        .get("schema_version")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("orchestration corpus {captured} is missing `schema_version`"))?;
    let (scorecard, failures) = match schema_version {
        crate::orchestration_attempt::ORCHESTRATION_FIXTURE_CORPUS_SCHEMA_VERSION => {
            let corpus = load_orchestration_fixture_corpus(&body)?;
            let missing = missing_orchestration_required_scenarios(
                corpus.scenarios.iter().map(|scenario| scenario.id.as_str()),
            );
            if !missing.is_empty() {
                return Err(format!(
                    "orchestration fixture corpus is missing required scenarios: {missing:?}"
                ));
            }
            assess_orchestration_fixture_corpus(&corpus, &captured)
        }
        crate::orchestration_attempt::ORCHESTRATION_CORPUS_SCHEMA_VERSION => {
            let corpus = crate::orchestration_attempt::load_orchestration_corpus(&body)?;
            let corpus_identity = orchestration_corpus_identity(&corpus.rows)?;
            let assessed: Vec<OrchestrationRowAssessmentV1> = corpus
                .rows
                .iter()
                .map(assess_orchestration_attempt)
                .collect();
            (
                build_orchestration_scorecard(&assessed, corpus_identity, &captured),
                Vec::new(),
            )
        }
        other => {
            return Err(format!(
                "unsupported orchestration corpus schema `{other}` in {captured}"
            ));
        }
    };
    if !failures.is_empty() {
        return Err(format!(
            "orchestration fixture corpus drifted: {failures:?}"
        ));
    }
    let json_body = orchestration_scorecard_json(&scorecard)?;
    crate::write_report("orchestration-scorecard.json", &json_body)?;
    crate::write_report(
        "orchestration-scorecard.md",
        &orchestration_scorecard_markdown(&scorecard),
    )?;
    println!("{json_body}");
    Ok(())
}

pub(crate) fn orchestration_scorecard_json(
    scorecard: &OrchestrationScorecardV1,
) -> Result<String, String> {
    let body = serde_json::to_string_pretty(scorecard)
        .map_err(|error| format!("serialize orchestration scorecard: {error}"))?;
    Ok(format!("{body}\n"))
}

pub(crate) fn orchestration_scorecard_markdown(scorecard: &OrchestrationScorecardV1) -> String {
    let mut body = String::new();
    body.push_str("# Orchestration attempt scorecard\n\n");
    body.push_str(&format!("Claim boundary: {}\n\n", scorecard.claim_boundary));
    let rate = match scorecard.completion_rate.state {
        OrchestrationRateStateV1::Measured => format!(
            "{}/{}",
            scorecard.completion_rate.numerator, scorecard.completion_rate.denominator
        ),
        OrchestrationRateStateV1::NotMeasured => {
            "not measured (zero real attempts; no rate is fabricated)".to_string()
        }
    };
    body.push_str(&format!(
        "- corpus: `{}`\n- corpus identity: `{}`\n- real attempts: {}\n- completed: {}\n- completion rate: {}\n- synthetic mechanics attempts (outside every real denominator): {}\n- rejected rows (visible, counted nowhere): {}\n\n",
        scorecard.corpus_path,
        scorecard.corpus_identity,
        scorecard.real_attempts,
        scorecard.completed,
        rate,
        scorecard.synthetic_attempts,
        scorecard.rejected_rows.len()
    ));
    body.push_str("| strategy | attempts | completed | visible non-completed |\n");
    body.push_str("| --- | ---: | ---: | ---: |\n");
    for totals in &scorecard.strategy_totals {
        body.push_str(&format!(
            "| {:?} | {} | {} | {} |\n",
            totals.strategy, totals.attempts, totals.completed, totals.visible_non_completed
        ));
    }
    if !scorecard.real_rows.is_empty() {
        body.push_str("\n| attempt | strategy | disposition | observations |\n");
        body.push_str("| --- | --- | --- | ---: |\n");
        for row in &scorecard.real_rows {
            body.push_str(&format!(
                "| {} | {:?} | {:?} | {} |\n",
                row.attempt_id, row.strategy, row.disposition, row.observations
            ));
        }
    }
    for row in &scorecard.rejected_rows {
        body.push_str(&format!(
            "\n## rejected: {} (synthetic: {})\n\n",
            row.attempt_id, row.synthetic
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
    use crate::orchestration_attempt::{
        AttemptComparisonV1, ORCHESTRATION_ATTEMPT_SCHEMA_VERSION, OrchestrationAttemptV1,
        OrchestrationBoundaryStatusV1, OrchestrationChangedPathV1, OrchestrationClaimRefV1,
        OrchestrationClaimStateV1, OrchestrationCleanupV1, OrchestrationClientRefV1,
        OrchestrationCommandDenominatorV1, OrchestrationEvidenceRefV1, OrchestrationOverflowRefV1,
        OrchestrationRoleV1, OrchestrationVerificationV1, OrchestrationWorkRefV1,
        load_orchestration_corpus, orchestration_row_digest,
    };

    fn hex64(fill: char) -> String {
        fill.to_string().repeat(64)
    }

    fn sample_attempt(
        attempt_id: &str,
        observation_key: &str,
        synthetic: bool,
    ) -> Result<OrchestrationAttemptV1, String> {
        let result_identity = format!("result:sha256:{}", hex64('b'));
        let mut row = OrchestrationAttemptV1 {
            schema_version: ORCHESTRATION_ATTEMPT_SCHEMA_VERSION.to_string(),
            attempt_id: attempt_id.to_string(),
            work: OrchestrationWorkRefV1 {
                repository: "https://example.invalid/operator/target".to_string(),
                source: "source-sample".to_string(),
                selected_work: "issue-sample".to_string(),
                portfolio: "campaign-sample".to_string(),
                base: "base-sha".to_string(),
                head: "head-sha".to_string(),
            },
            task_family: "narrow_bug".to_string(),
            accepted_contract: "accepted-contract-sample".to_string(),
            client: OrchestrationClientRefV1 {
                agent_client: "agent-cli-1.0".to_string(),
                role_configuration: "single_builder".to_string(),
            },
            strategy: AttemptStrategyV1::SingleAgent,
            disposition: AttemptDispositionV1::Completed,
            planned_waves: 1,
            actual_waves: 1,
            packet: OrchestrationEvidenceRefV1 {
                identity: format!("packet:sha256:{}", hex64('a')),
                bytes: 2048,
            },
            result: OrchestrationEvidenceRefV1 {
                identity: result_identity.clone(),
                bytes: 4096,
            },
            synthesis: Some(OrchestrationEvidenceRefV1 {
                identity: format!("synthesis:sha256:{}", hex64('c')),
                bytes: 512,
            }),
            overflow: vec![OrchestrationOverflowRefV1 {
                required: true,
                evidence: Some(OrchestrationEvidenceRefV1 {
                    identity: format!("overflow:sha256:{}", hex64('d')),
                    bytes: 256,
                }),
            }],
            claims: vec![OrchestrationClaimRefV1 {
                claim_id: format!("claim-{attempt_id}"),
                role: OrchestrationRoleV1::Builder,
                state: OrchestrationClaimStateV1::Open,
                worktree_root: "/srv/orchestration/worktree-sample".to_string(),
                edit_cage: vec!["src/lib.rs".to_string()],
                resources: vec!["cpu:1".to_string()],
            }],
            commands: vec![OrchestrationCommandDenominatorV1 {
                command: "cargo test -p sample".to_string(),
                subject_count: 3,
                passed: true,
            }],
            independent_verification: Some(OrchestrationVerificationV1 {
                verification_id: format!("verification-{attempt_id}"),
                bound_base: "base-sha".to_string(),
                bound_head: "head-sha".to_string(),
                bound_result_identity: result_identity,
                commands: vec![OrchestrationCommandDenominatorV1 {
                    command: "cargo test -p sample".to_string(),
                    subject_count: 3,
                    passed: true,
                }],
                independent_receipt: true,
                comparison: AttemptComparisonV1::Matched,
            }),
            contradictions: Vec::new(),
            rejected_claims: Vec::new(),
            changed_paths: vec![OrchestrationChangedPathV1 {
                path: "src/lib.rs".to_string(),
                boundary_status: OrchestrationBoundaryStatusV1::WithinCage,
            }],
            delivery: None,
            cleanup: OrchestrationCleanupV1 {
                cleaned: true,
                residue: Vec::new(),
            },
            limitations: vec!["sample limitation".to_string()],
            non_claims: vec!["sample non-claim".to_string()],
            synthetic,
            observation_key: observation_key.to_string(),
            row_digest: String::new(),
        };
        row.row_digest = orchestration_row_digest(&row)?;
        Ok(row)
    }

    fn assessed(row: OrchestrationAttemptV1) -> OrchestrationRowAssessmentV1 {
        assess_orchestration_attempt(&row)
    }

    #[test]
    fn empty_corpus_reports_no_success_rate() -> Result<(), String> {
        let corpus =
            load_orchestration_corpus(r#"{"schema_version":"orchestration_corpus.v1","rows":[]}"#)?;
        if !corpus.rows.is_empty() {
            return Err("the empty corpus fixture must parse to zero rows".to_string());
        }
        let scorecard =
            build_orchestration_scorecard(&[], "empty".to_string(), "fixtures/empty.json");
        if scorecard.real_attempts != 0 || scorecard.completed != 0 {
            return Err("an empty corpus must keep zero real attempts".to_string());
        }
        if scorecard.completion_rate.state != OrchestrationRateStateV1::NotMeasured {
            return Err("an empty corpus must keep the completion rate not measured".to_string());
        }
        let markdown = orchestration_scorecard_markdown(&scorecard);
        if !markdown.contains("not measured") {
            return Err("the honest empty report must say the rate is not measured".to_string());
        }
        if !scorecard
            .strategy_totals
            .iter()
            .all(|totals| totals.attempts == 0)
        {
            return Err("every strategy row must stay visible with zero attempts".to_string());
        }
        Ok(())
    }

    #[test]
    fn synthetic_rows_never_enter_real_denominators() -> Result<(), String> {
        let row = sample_attempt("attempt-synthetic", "observation-synthetic", true)?;
        let scorecard = build_orchestration_scorecard(
            &[assessed(row)],
            "synthetic".to_string(),
            "fixtures/synthetic.json",
        );
        if scorecard.real_attempts != 0
            || scorecard.completion_rate.state != OrchestrationRateStateV1::NotMeasured
        {
            return Err(format!(
                "synthetic rows leaked into real denominators: real_attempts={} rate={:?}",
                scorecard.real_attempts, scorecard.completion_rate
            ));
        }
        if scorecard.synthetic_attempts != 1 {
            return Err(format!(
                "synthetic attempts must be counted separately, got {}",
                scorecard.synthetic_attempts
            ));
        }
        Ok(())
    }

    #[test]
    fn real_completed_row_enters_the_completion_rate() -> Result<(), String> {
        let row = sample_attempt("attempt-real", "observation-real", false)?;
        let scorecard = build_orchestration_scorecard(
            &[assessed(row)],
            "real".to_string(),
            "fixtures/real.json",
        );
        if scorecard.real_attempts != 1 || scorecard.completed != 1 {
            return Err(format!(
                "one valid real row must count once as completed, got real={} completed={}",
                scorecard.real_attempts, scorecard.completed
            ));
        }
        if scorecard.completion_rate
            != (OrchestrationRateV1 {
                state: OrchestrationRateStateV1::Measured,
                numerator: 1,
                denominator: 1,
            })
        {
            return Err("the completion rate must be measured 1/1".to_string());
        }
        Ok(())
    }

    #[test]
    fn duplicate_observations_of_one_attempt_deduplicate() -> Result<(), String> {
        let first = sample_attempt("attempt-duplicate", "observation-duplicate", false)?;
        let second = sample_attempt("attempt-duplicate", "observation-duplicate", false)?;
        let scorecard = build_orchestration_scorecard(
            &[assessed(first), assessed(second)],
            "duplicates".to_string(),
            "fixtures/duplicates.json",
        );
        if scorecard.real_attempts != 1 {
            return Err(format!(
                "duplicate observations must remain one attempt, got {}",
                scorecard.real_attempts
            ));
        }
        let row = scorecard
            .real_rows
            .first()
            .ok_or_else(|| "the deduplicated row must be present".to_string())?;
        if row.observations != 2 || row.attempt_id != "attempt-duplicate" {
            return Err(format!(
                "the representative must keep one identity with two observations, got {:?}",
                row
            ));
        }
        Ok(())
    }

    #[test]
    fn conflicting_duplicate_observations_reject_both() -> Result<(), String> {
        let mut first = sample_attempt("attempt-conflict-a", "observation-conflict", false)?;
        let mut second = sample_attempt("attempt-conflict-b", "observation-conflict", false)?;
        second.disposition = AttemptDispositionV1::Blocked;
        second.independent_verification = None;
        first.row_digest = orchestration_row_digest(&first)?;
        second.row_digest = orchestration_row_digest(&second)?;
        let scorecard = build_orchestration_scorecard(
            &[assessed(first), assessed(second)],
            "conflicts".to_string(),
            "fixtures/conflicts.json",
        );
        if scorecard.real_attempts != 0 {
            return Err("conflicting observations must not count as an attempt".to_string());
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
    fn rejected_rows_stay_visible_without_denominators() -> Result<(), String> {
        let mut row = sample_attempt("attempt-over-budget", "observation-over-budget", false)?;
        row.result.bytes = crate::orchestration_attempt::ORCHESTRATION_RESULT_BYTE_BUDGET + 1;
        row.row_digest = orchestration_row_digest(&row)?;
        let scorecard = build_orchestration_scorecard(
            &[assessed(row)],
            "rejected".to_string(),
            "fixtures/rejected.json",
        );
        if scorecard.real_attempts != 0
            || scorecard.completion_rate.state != OrchestrationRateStateV1::NotMeasured
        {
            return Err("a rejected row must not enter real denominators".to_string());
        }
        if scorecard.rejected_rows.is_empty() {
            return Err("a rejected row must stay visible".to_string());
        }
        Ok(())
    }

    #[test]
    fn reordered_inputs_preserve_the_corpus_identity() -> Result<(), String> {
        let first = sample_attempt("attempt-order-a", "observation-order-a", false)?;
        let second = sample_attempt("attempt-order-b", "observation-order-b", true)?;
        let identity_ab = orchestration_corpus_identity(&[first.clone(), second.clone()])?;
        let identity_ba = orchestration_corpus_identity(&[second, first])?;
        if identity_ab != identity_ba {
            return Err("reordered inputs must preserve the portable corpus digest".to_string());
        }
        Ok(())
    }

    #[test]
    fn equivalent_roots_preserve_the_portable_corpus_digest() -> Result<(), String> {
        let mut other = sample_attempt("attempt-roots", "observation-roots", false)?;
        other.claims[0].worktree_root = "/var/orchestration/worktree-sample".to_string();
        other.row_digest = orchestration_row_digest(&other)?;
        let first = sample_attempt("attempt-roots", "observation-roots", false)?;
        let identity_first = orchestration_corpus_identity(&[first])?;
        let identity_second = orchestration_corpus_identity(&[other])?;
        if identity_first != identity_second {
            return Err("equivalent roots must preserve the portable corpus digest".to_string());
        }
        Ok(())
    }

    #[test]
    fn json_and_markdown_derive_from_one_dto_deterministically() -> Result<(), String> {
        let first = sample_attempt("attempt-projection", "observation-projection", false)?;
        let scorecard = build_orchestration_scorecard(
            &[assessed(first)],
            "projection".to_string(),
            "fixtures/projection.json",
        );
        let json_first = orchestration_scorecard_json(&scorecard)?;
        let json_second = orchestration_scorecard_json(&scorecard)?;
        if json_first != json_second {
            return Err("the JSON projection must be byte-identical across runs".to_string());
        }
        if !json_first.ends_with('\n') || json_first.ends_with("\n\n") {
            return Err("the JSON projection must end in exactly one newline".to_string());
        }
        let markdown = orchestration_scorecard_markdown(&scorecard);
        if !markdown.contains("attempt-projection") || !markdown.contains("SingleAgent") {
            return Err(
                "the Markdown projection must name the attempt and the strategy".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn digest_corrupted_duplicate_conflicts_in_either_input_order() -> Result<(), String> {
        let valid = sample_attempt("attempt-digest-dup", "observation-digest-dup", false)?;
        let mut corrupted = sample_attempt("attempt-digest-dup", "observation-digest-dup", false)?;
        corrupted.row_digest = String::new();
        let build = |rows: Vec<OrchestrationRowAssessmentV1>| {
            build_orchestration_scorecard(&rows, "digest-dup".to_string(), "fixtures/digest.json")
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
        if forward.real_attempts != 0 || forward.rejected_rows.len() != 2 {
            return Err(format!(
                "a digest-corrupted duplicate must reject the whole group and stay visible, got real={} rejected={}",
                forward.real_attempts,
                forward.rejected_rows.len()
            ));
        }
        Ok(())
    }

    #[test]
    fn parse_captured_accepts_at_most_one_path() -> Result<(), String> {
        let parsed = parse_captured_arg(&["--captured".to_string(), "some.json".to_string()])?;
        if parsed != "some.json" {
            return Err(format!("expected `some.json`, got {parsed}"));
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
                "--captured".to_string(),
                "b.json".to_string(),
            ],
        ] {
            if parse_captured_arg(&args).is_ok() {
                return Err(format!("malformed arguments were accepted: {args:?}"));
            }
        }
        Ok(())
    }
}
