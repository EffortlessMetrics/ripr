//! Discriminating controls for judged-panel runtime calibration (#4795).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use super::classify::{AttemptFacts, classify_attempt};
use super::execute::{
    ExecutionPlan, OwnedProcessRunner, ProcessRunner, attempt_from_execution, bind_runner_identity,
    hash_runner,
};
use super::scorecard::{build_scorecard, scorecard_exists, write_scorecard};
use super::{
    BehaviorIdentity, CalibrationReceipt, JudgedCase, RESULT_CAUGHT, RESULT_EQUIVALENT,
    RESULT_INCONCLUSIVE, RESULT_INSTRUMENT, RESULT_NOT_RUN, RESULT_STALE, RESULT_SURVIVED,
    SCOPE_AUTHORIZED, Selector, SubjectCounts, digest_pref, join_rows,
};

struct TempOut {
    root: PathBuf,
}

impl TempOut {
    fn new(name: &str) -> Result<Self, String> {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "ripr-rust-judged-cal-{name}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        Ok(Self { root })
    }
}

impl Drop for TempOut {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn behavior(file: &str, line: u64, owner: &str, changed: &str) -> BehaviorIdentity {
    BehaviorIdentity {
        file: file.to_string(),
        line,
        owner: owner.to_string(),
        changed_behavior: changed.to_string(),
    }
}

fn selector(package: &str, target: &str, filter: &str) -> Selector {
    Selector {
        package: package.to_string(),
        target: target.to_string(),
        filter: filter.to_string(),
    }
}

fn subjects(n: u64) -> SubjectCounts {
    SubjectCounts {
        intended: n,
        discovered: n,
        selected: n,
        executed: n,
    }
}

fn judged(
    id: &str,
    expected: &str,
    terminal: &str,
    scope: &str,
    behavior: BehaviorIdentity,
    required: Option<Selector>,
) -> JudgedCase {
    JudgedCase {
        case_id: id.to_string(),
        expected_direction: expected.to_string(),
        terminal: terminal.to_string(),
        behavior_family: "predicate_boundary".to_string(),
        relation_basis: "direct_owner_call".to_string(),
        oracle_class: "exact_value".to_string(),
        witness_completeness: "aligned".to_string(),
        repository: "EffortlessMetrics/ripr-swarm".to_string(),
        base: "base".to_string(),
        head: "head".to_string(),
        tree_identity: "tree".to_string(),
        scope_authorization: scope.to_string(),
        no_focused_mutant: false,
        behavior,
        required_selector: required,
    }
}

fn receipt_for(case: &JudgedCase, result: &str, subjects: SubjectCounts) -> CalibrationReceipt {
    CalibrationReceipt {
        case_id: case.case_id.clone(),
        selection_sha256: "sha256:selection".to_string(),
        judgments_sha256: "sha256:judgments".to_string(),
        repository: case.repository.clone(),
        base: case.base.clone(),
        head: case.head.clone(),
        tree_identity: case.tree_identity.clone(),
        behavior: case.behavior.clone(),
        expected_direction: case.expected_direction.clone(),
        terminal: case.terminal.clone(),
        mutant_operator: "binop_gt_to_ge".to_string(),
        mutant_range: "src/lib.rs:4:8-4:9".to_string(),
        mutant_identity: "sha256:mutant".to_string(),
        package: case
            .required_selector
            .as_ref()
            .map(|selector| selector.package.clone())
            .unwrap_or_else(|| "pkg".to_string()),
        target: case
            .required_selector
            .as_ref()
            .map(|selector| selector.target.clone())
            .unwrap_or_else(|| "test".to_string()),
        filter: case
            .required_selector
            .as_ref()
            .map(|selector| selector.filter.clone())
            .unwrap_or_else(|| "exact_filter".to_string()),
        runner_tool: "cargo".to_string(),
        runner_version: "cargo 1.95.0".to_string(),
        runner_path: "/abs/cargo".to_string(),
        runner_sha256: "sha256:runner".to_string(),
        argv: vec!["test".to_string(), "-p".to_string(), "pkg".to_string()],
        cwd: "/abs/cwd".to_string(),
        timeout_ms: 120_000,
        environment_policy: "offline_isolated".to_string(),
        subjects,
        exit_code: Some(if result == RESULT_SURVIVED { 0 } else { 1 }),
        timed_out: false,
        compile_failed: false,
        process_failed: false,
        stdout_sha256: "sha256:stdout".to_string(),
        stderr_sha256: "sha256:stderr".to_string(),
        currentness: "current".to_string(),
        claimed_result: result.to_string(),
        limitations: Vec::new(),
        semantic_receipt_digest: format!("sha256:receipt-{}", case.case_id),
    }
}

fn quiet_gap_limit() -> (JudgedCase, JudgedCase, JudgedCase) {
    let required = selector("pkg", "test", "pricing::equality_boundary");
    (
        judged(
            "quiet-control",
            "should_stay_quiet",
            "confirmed_should_stay_quiet",
            SCOPE_AUTHORIZED,
            behavior("src/lib.rs", 4, "discounted_total", ">="),
            Some(required.clone()),
        ),
        judged(
            "gap-control",
            "should_gap",
            "confirmed_should_gap",
            SCOPE_AUTHORIZED,
            behavior("src/lib.rs", 4, "discounted_total", ">"),
            Some(required.clone()),
        ),
        judged(
            "limit-control",
            "should_limit",
            "confirmed_should_limit",
            SCOPE_AUTHORIZED,
            behavior("src/lib.rs", 8, "assert_normalized", "macro"),
            Some(required),
        ),
    )
}

fn join_ok(
    cases: &[JudgedCase],
    receipts: BTreeMap<String, CalibrationReceipt>,
) -> Result<Vec<super::JoinedRow>, String> {
    join_rows(cases, &receipts, "sha256:selection", "sha256:judgments")
}

fn report_for(rows: &[super::JoinedRow]) -> Result<super::scorecard::RenderedScorecard, String> {
    build_scorecard(
        "sha256:selection",
        "sha256:judgments",
        "sha256:rolling",
        rows,
    )
}

fn cell<'a>(value: &'a Value, direction: &str, result: &str) -> &'a Value {
    &value["static_runtime_matrix"][direction][result]
}

#[test]
fn caught_with_nonzero_subjects_is_caught() -> Result<(), String> {
    let (quiet, _, _) = quiet_gap_limit();
    let mut receipt = receipt_for(&quiet, RESULT_CAUGHT, subjects(3));
    receipt.exit_code = Some(101);
    let rows = join_ok(
        &[quiet],
        BTreeMap::from([(receipt.case_id.clone(), receipt)]),
    )?;
    assert_eq!(rows[0].runtime_result, RESULT_CAUGHT);
    assert_eq!(rows[0].case.terminal, "confirmed_should_stay_quiet");
    Ok(())
}

#[test]
fn survived_is_retained_without_automatic_false_exposed() -> Result<(), String> {
    let (quiet, _, _) = quiet_gap_limit();
    let receipt = receipt_for(&quiet, RESULT_SURVIVED, subjects(2));
    let rows = join_ok(
        &[quiet],
        BTreeMap::from([(receipt.case_id.clone(), receipt)]),
    )?;
    let report = report_for(&rows)?;
    assert_eq!(rows[0].runtime_result, RESULT_SURVIVED);
    assert_eq!(
        report.value["candidates"]["false_exposed"]["numerator"]
            .as_u64()
            .unwrap_or(0),
        1
    );
    assert_eq!(
        report.value["cases"][0]["automatic_false_exposed"],
        json!(false)
    );
    assert!(
        report
            .markdown
            .contains("without an automatic false-exposed conclusion")
    );
    Ok(())
}

#[test]
fn zero_subjects_with_exit_zero_is_not_caught_or_survived() -> Result<(), String> {
    let (quiet, _, _) = quiet_gap_limit();
    let mut receipt = receipt_for(&quiet, RESULT_SURVIVED, subjects(0));
    receipt.exit_code = Some(0);
    receipt.claimed_result = RESULT_SURVIVED.to_string();
    let rows = join_ok(
        &[quiet],
        BTreeMap::from([(receipt.case_id.clone(), receipt)]),
    )?;
    assert_eq!(rows[0].runtime_result, RESULT_INCONCLUSIVE);
    assert_eq!(
        rows[0].non_calibration_reason,
        Some("zero_executed_subjects")
    );
    assert_ne!(rows[0].runtime_result, RESULT_SURVIVED);
    assert_ne!(rows[0].runtime_result, RESULT_CAUGHT);
    Ok(())
}

#[test]
fn wrong_selector_is_rejected_as_stale_not_caught() -> Result<(), String> {
    let (quiet, _, _) = quiet_gap_limit();
    let mut receipt = receipt_for(&quiet, RESULT_CAUGHT, subjects(1));
    receipt.filter = "pricing::equality".to_string();
    let rows = join_ok(
        &[quiet],
        BTreeMap::from([(receipt.case_id.clone(), receipt)]),
    )?;
    assert_eq!(rows[0].runtime_result, RESULT_STALE);
    assert_eq!(rows[0].non_calibration_reason, Some("wrong_selector"));
    Ok(())
}

#[test]
fn neighboring_same_name_filter_cannot_substitute() -> Result<(), String> {
    let required = selector("pkg", "test", "owner::behavior");
    let case = judged(
        "named",
        "should_stay_quiet",
        "confirmed_should_stay_quiet",
        SCOPE_AUTHORIZED,
        behavior("src/lib.rs", 1, "owner", "token"),
        Some(required),
    );
    let mut neighbor = receipt_for(&case, RESULT_CAUGHT, subjects(4));
    neighbor.filter = "other_owner::behavior".to_string();
    let rows = join_ok(
        &[case],
        BTreeMap::from([(neighbor.case_id.clone(), neighbor)]),
    )?;
    assert_eq!(rows[0].runtime_result, RESULT_STALE);
    Ok(())
}

#[test]
fn timeout_compile_and_process_failure_remain_separate() {
    let timeout = classify_attempt(&AttemptFacts {
        identity_ok: true,
        selector_ok: true,
        equivalent: false,
        timed_out: true,
        compile_failed: true,
        process_failed: true,
        exit_code: None,
        intended: 1,
        discovered: 1,
        selected: 1,
        executed: 1,
    });
    let compile = classify_attempt(&AttemptFacts {
        identity_ok: true,
        selector_ok: true,
        equivalent: false,
        timed_out: false,
        compile_failed: true,
        process_failed: false,
        exit_code: Some(1),
        intended: 1,
        discovered: 1,
        selected: 1,
        executed: 1,
    });
    let process = classify_attempt(&AttemptFacts {
        identity_ok: true,
        selector_ok: true,
        equivalent: false,
        timed_out: false,
        compile_failed: false,
        process_failed: true,
        exit_code: None,
        intended: 1,
        discovered: 1,
        selected: 1,
        executed: 1,
    });
    assert_eq!(timeout.runtime_result, RESULT_INSTRUMENT);
    assert_eq!(timeout.instrument_kind, Some(super::INSTRUMENT_TIMEOUT));
    assert_eq!(compile.runtime_result, RESULT_INSTRUMENT);
    assert_eq!(compile.instrument_kind, Some(super::INSTRUMENT_COMPILE));
    assert_eq!(process.runtime_result, RESULT_INSTRUMENT);
    assert_eq!(process.instrument_kind, Some(super::INSTRUMENT_PROCESS));
    assert_ne!(timeout.instrument_kind, compile.instrument_kind);
    assert_ne!(compile.instrument_kind, process.instrument_kind);
}

#[test]
fn equivalent_mutant_is_excluded_from_caught_survived_rates() -> Result<(), String> {
    let (quiet, gap, _) = quiet_gap_limit();
    let equivalent = {
        let mut receipt = receipt_for(&quiet, RESULT_EQUIVALENT, subjects(1));
        receipt.exit_code = Some(0);
        receipt
    };
    let caught = receipt_for(&gap, RESULT_CAUGHT, subjects(1));
    let rows = join_ok(
        &[quiet, gap],
        BTreeMap::from([
            (equivalent.case_id.clone(), equivalent),
            (caught.case_id.clone(), caught),
        ]),
    )?;
    let report = report_for(&rows)?;
    assert_eq!(rows[0].runtime_result, RESULT_EQUIVALENT);
    assert_eq!(
        report.value["equivalent_or_unusable_case_ids"][0],
        json!("quiet-control")
    );
    assert_eq!(report.value["counts"]["completed"].as_u64(), Some(1));
    assert!(
        report.value["candidates"]["false_exposed"]["excluded_case_ids"]
            .as_array()
            .is_some_and(|ids| ids.iter().any(|id| id == "quiet-control"))
    );
    Ok(())
}

#[test]
fn stale_identity_rejects_caught_promotion() -> Result<(), String> {
    let (quiet, _, _) = quiet_gap_limit();
    let mut receipt = receipt_for(&quiet, RESULT_CAUGHT, subjects(1));
    receipt.head = "other-head".to_string();
    let rows = join_ok(
        &[quiet],
        BTreeMap::from([(receipt.case_id.clone(), receipt)]),
    )?;
    assert_eq!(rows[0].runtime_result, RESULT_STALE);
    assert_eq!(rows[0].non_calibration_reason, Some("stale_identity"));
    Ok(())
}

#[test]
fn reordered_cases_are_byte_stable() -> Result<(), String> {
    let (quiet, gap, limit) = quiet_gap_limit();
    let receipts = BTreeMap::from([
        (
            quiet.case_id.clone(),
            receipt_for(&quiet, RESULT_CAUGHT, subjects(1)),
        ),
        (
            gap.case_id.clone(),
            receipt_for(&gap, RESULT_SURVIVED, subjects(1)),
        ),
        (
            limit.case_id.clone(),
            receipt_for(&limit, RESULT_CAUGHT, subjects(1)),
        ),
    ]);
    let forward = join_ok(
        &[quiet.clone(), gap.clone(), limit.clone()],
        receipts.clone(),
    )?;
    let reverse = join_ok(&[limit, gap, quiet], receipts)?;
    let left = report_for(&forward)?;
    let right = report_for(&reverse)?;
    assert_eq!(left.json, right.json);
    assert_eq!(left.markdown, right.markdown);
    assert_eq!(left.value["digest"], right.value["digest"]);
    Ok(())
}

#[test]
fn runtime_join_cannot_mutate_structural_judgment_bytes() -> Result<(), String> {
    let (quiet, _, _) = quiet_gap_limit();
    let terminal_before = quiet.terminal.clone();
    let receipt = receipt_for(&quiet, RESULT_SURVIVED, subjects(1));
    let rows = join_ok(
        std::slice::from_ref(&quiet),
        BTreeMap::from([(receipt.case_id.clone(), receipt)]),
    )?;
    assert_eq!(quiet.terminal, terminal_before);
    assert_eq!(rows[0].case.terminal, terminal_before);
    assert_eq!(rows[0].runtime_result, RESULT_SURVIVED);
    assert_ne!(rows[0].runtime_result, rows[0].case.terminal);
    Ok(())
}

#[test]
fn removing_one_calibrated_row_changes_denominator_and_digest() -> Result<(), String> {
    let (quiet, gap, _) = quiet_gap_limit();
    let receipts = BTreeMap::from([
        (
            quiet.case_id.clone(),
            receipt_for(&quiet, RESULT_CAUGHT, subjects(1)),
        ),
        (
            gap.case_id.clone(),
            receipt_for(&gap, RESULT_CAUGHT, subjects(1)),
        ),
    ]);
    let two = report_for(&join_ok(&[quiet.clone(), gap.clone()], receipts.clone())?)?;
    let one = report_for(&join_ok(
        &[quiet],
        BTreeMap::from([(
            "quiet-control".to_string(),
            receipts
                .get("quiet-control")
                .cloned()
                .ok_or_else(|| "missing quiet receipt".to_string())?,
        )]),
    )?)?;
    assert_eq!(two.value["counts"]["judged"].as_u64(), Some(2));
    assert_eq!(one.value["counts"]["judged"].as_u64(), Some(1));
    assert_ne!(two.value["digest"], one.value["digest"]);
    assert_ne!(two.json, one.json);
    Ok(())
}

#[test]
fn failed_calibration_does_not_write_a_successful_empty_scorecard() -> Result<(), String> {
    let out = TempOut::new("empty-fail")?;
    let error = build_scorecard(
        "sha256:selection",
        "sha256:judgments",
        "sha256:rolling",
        &[],
    )
    .err()
    .ok_or_else(|| "empty judged set produced a scorecard".to_string())?;
    assert!(error.contains("empty judged set"));
    assert!(!scorecard_exists(&out.root));
    let unknown = judged(
        "known",
        "should_gap",
        "confirmed_should_gap",
        SCOPE_AUTHORIZED,
        behavior("a.rs", 1, "o", "x"),
        None,
    );
    let mut stray = receipt_for(&unknown, RESULT_CAUGHT, subjects(1));
    stray.case_id = "not-in-panel".to_string();
    let join_error = join_ok(
        &[unknown],
        BTreeMap::from([("not-in-panel".to_string(), stray)]),
    )
    .err()
    .ok_or_else(|| "unknown receipt was accepted".to_string())?;
    assert!(join_error.contains("unknown case"));
    assert!(!scorecard_exists(&out.root));
    Ok(())
}

#[test]
fn json_and_markdown_agree_on_every_count_and_row() -> Result<(), String> {
    let (quiet, gap, limit) = quiet_gap_limit();
    let unauthorized = judged(
        "unauth",
        "should_stay_quiet",
        "confirmed_should_stay_quiet",
        "proposed_unauthorized",
        behavior("src/lib.rs", 9, "helper", "token"),
        None,
    );
    let rows = join_ok(
        &[quiet.clone(), gap.clone(), limit.clone(), unauthorized],
        BTreeMap::from([
            (
                quiet.case_id.clone(),
                receipt_for(&quiet, RESULT_CAUGHT, subjects(1)),
            ),
            (
                gap.case_id.clone(),
                receipt_for(&gap, RESULT_SURVIVED, subjects(1)),
            ),
            (
                limit.case_id.clone(),
                receipt_for(&limit, RESULT_CAUGHT, subjects(2)),
            ),
        ]),
    )?;
    let report = report_for(&rows)?;
    assert_eq!(
        cell(&report.value, "should_stay_quiet", RESULT_CAUGHT)["count"].as_u64(),
        Some(1)
    );
    assert_eq!(
        cell(&report.value, "should_gap", RESULT_SURVIVED)["count"].as_u64(),
        Some(1)
    );
    assert_eq!(
        cell(&report.value, "should_limit", RESULT_CAUGHT)["count"].as_u64(),
        Some(1)
    );
    assert_eq!(
        cell(&report.value, "should_stay_quiet", RESULT_NOT_RUN)["count"].as_u64(),
        Some(1)
    );
    for case in report.value["cases"]
        .as_array()
        .ok_or_else(|| "scorecard cases missing".to_string())?
    {
        let id = case["case_id"]
            .as_str()
            .ok_or_else(|| "case_id missing".to_string())?;
        assert!(report.markdown.contains(&format!("`{id}`")), "{id}");
        assert!(report.json.contains(id), "{id}");
    }
    for key in [
        "selected",
        "judged",
        "calibration_eligible",
        "attempted",
        "completed",
        "inconclusive",
    ] {
        let number = report.value["counts"][key]
            .as_u64()
            .ok_or_else(|| format!("{key} missing"))?;
        assert!(
            report.markdown.contains(&format!("{key}: {number}")),
            "{key}"
        );
    }
    assert_eq!(
        report.value["candidates"]["false_exposed"]["denominator"].as_u64(),
        Some(3)
    );
    assert_eq!(
        report.value["candidates"]["false_exposed"]["rate"].as_str(),
        Some("0/3")
    );
    assert!(report.markdown.ends_with('\n'));
    assert!(
        !report.markdown.ends_with("\n\n"),
        "markdown must not add a trailing blank line"
    );
    Ok(())
}

#[test]
fn unauthorized_row_stays_not_run_and_cannot_be_caught() -> Result<(), String> {
    let case = judged(
        "locked",
        "should_gap",
        "confirmed_should_gap",
        "proposed_unauthorized",
        behavior("src/lib.rs", 2, "owner", "token"),
        Some(selector("pkg", "test", "t")),
    );
    let receipt = receipt_for(&case, RESULT_CAUGHT, subjects(9));
    let rows = join_ok(
        &[case],
        BTreeMap::from([(receipt.case_id.clone(), receipt)]),
    )?;
    assert_eq!(
        rows[0].eligibility,
        super::eligibility::INELIGIBLE_UNAUTHORIZED
    );
    assert_eq!(rows[0].runtime_result, RESULT_NOT_RUN);
    assert_ne!(rows[0].runtime_result, RESULT_CAUGHT);
    assert_ne!(rows[0].runtime_result, RESULT_STALE);
    let report = report_for(&rows)?;
    assert_eq!(report.value["counts"]["attempted"].as_u64(), Some(0));
    assert_eq!(report.value["counts"]["completed"].as_u64(), Some(0));
    Ok(())
}

#[test]
fn path_or_command_name_is_not_runner_authority() -> Result<(), String> {
    let plan = ExecutionPlan {
        runner_path: PathBuf::from("cargo"),
        runner_sha256: "sha256:anything".to_string(),
        argv: vec!["test".to_string()],
        cwd: PathBuf::from("/tmp"),
        timeout: Duration::from_secs(1),
        selector: selector("pkg", "test", "t"),
        subjects: subjects(1),
        equivalent: false,
    };
    let error = bind_runner_identity(&plan)
        .err()
        .ok_or_else(|| "relative runner must fail".to_string())?;
    assert!(error.contains("PATH") || error.contains("absolute"));
    Ok(())
}

#[test]
fn owned_runner_binds_hashed_absolute_identity() -> Result<(), String> {
    let runner = Path::new("/bin/true");
    if !runner.is_file() {
        return Ok(());
    }
    let digest = hash_runner(runner)?;
    let plan = ExecutionPlan {
        runner_path: runner.to_path_buf(),
        runner_sha256: digest,
        argv: Vec::new(),
        cwd: std::env::temp_dir(),
        timeout: Duration::from_secs(5),
        selector: selector("pkg", "test", "t"),
        subjects: subjects(1),
        equivalent: false,
    };
    let raw = OwnedProcessRunner.run(&plan)?;
    let facts = attempt_from_execution(&plan, &raw, true, true);
    let classified = classify_attempt(&facts);
    assert!(!raw.timed_out);
    assert_eq!(classified.runtime_result, RESULT_SURVIVED);
    Ok(())
}

#[test]
fn writing_scorecard_then_removing_a_row_is_visible_on_disk() -> Result<(), String> {
    let out = TempOut::new("digest-disk")?;
    let (quiet, gap, _) = quiet_gap_limit();
    let two = report_for(&join_ok(
        &[quiet.clone(), gap.clone()],
        BTreeMap::from([
            (
                quiet.case_id.clone(),
                receipt_for(&quiet, RESULT_CAUGHT, subjects(1)),
            ),
            (
                gap.case_id.clone(),
                receipt_for(&gap, RESULT_SURVIVED, subjects(1)),
            ),
        ]),
    )?)?;
    write_scorecard(
        &out.root.join("scorecard.json"),
        &out.root.join("scorecard.md"),
        &two,
    )?;
    let first = fs::read_to_string(out.root.join("scorecard.json")).map_err(|e| e.to_string())?;
    let one = report_for(&join_ok(
        std::slice::from_ref(&quiet),
        BTreeMap::from([(
            quiet.case_id.clone(),
            receipt_for(&quiet, RESULT_CAUGHT, subjects(1)),
        )]),
    )?)?;
    write_scorecard(
        &out.root.join("scorecard.json"),
        &out.root.join("scorecard.md"),
        &one,
    )?;
    let second = fs::read_to_string(out.root.join("scorecard.json")).map_err(|e| e.to_string())?;
    assert_ne!(first, second);
    assert!(digest_pref(first.as_bytes()) != digest_pref(second.as_bytes()));
    Ok(())
}

#[test]
fn matrix_keeps_required_static_runtime_pairs() -> Result<(), String> {
    let (quiet, gap, limit) = quiet_gap_limit();
    let quiet_survived = judged(
        "quiet-survived",
        "should_stay_quiet",
        "confirmed_should_stay_quiet",
        SCOPE_AUTHORIZED,
        behavior("src/lib.rs", 5, "other", ">="),
        Some(selector("pkg", "test", "pricing::equality_boundary")),
    );
    let gap_caught = judged(
        "gap-caught",
        "should_gap",
        "confirmed_should_gap",
        SCOPE_AUTHORIZED,
        behavior("src/lib.rs", 6, "other", ">"),
        Some(selector("pkg", "test", "pricing::equality_boundary")),
    );
    let rows = join_ok(
        &[
            quiet.clone(),
            quiet_survived.clone(),
            gap.clone(),
            gap_caught.clone(),
            limit.clone(),
        ],
        BTreeMap::from([
            (
                quiet.case_id.clone(),
                receipt_for(&quiet, RESULT_CAUGHT, subjects(1)),
            ),
            (
                quiet_survived.case_id.clone(),
                receipt_for(&quiet_survived, RESULT_SURVIVED, subjects(1)),
            ),
            (
                gap.case_id.clone(),
                receipt_for(&gap, RESULT_SURVIVED, subjects(1)),
            ),
            (
                gap_caught.case_id.clone(),
                receipt_for(&gap_caught, RESULT_CAUGHT, subjects(1)),
            ),
            (
                limit.case_id.clone(),
                receipt_for(&limit, RESULT_SURVIVED, subjects(1)),
            ),
        ]),
    )?;
    let report = report_for(&rows)?;
    assert_eq!(
        cell(&report.value, "should_stay_quiet", RESULT_CAUGHT)["count"].as_u64(),
        Some(1)
    );
    assert_eq!(
        cell(&report.value, "should_stay_quiet", RESULT_SURVIVED)["count"].as_u64(),
        Some(1)
    );
    assert_eq!(
        cell(&report.value, "should_gap", RESULT_CAUGHT)["count"].as_u64(),
        Some(1)
    );
    assert_eq!(
        cell(&report.value, "should_gap", RESULT_SURVIVED)["count"].as_u64(),
        Some(1)
    );
    assert_eq!(
        cell(&report.value, "should_limit", RESULT_SURVIVED)["count"].as_u64(),
        Some(1)
    );
    Ok(())
}

#[test]
fn no_focused_mutant_stays_ineligible_not_caught() -> Result<(), String> {
    let mut case = judged(
        "no-mutant",
        "should_gap",
        "confirmed_should_gap",
        SCOPE_AUTHORIZED,
        behavior("src/lib.rs", 1, "owner", "token"),
        None,
    );
    case.no_focused_mutant = true;
    let receipt = receipt_for(&case, RESULT_CAUGHT, subjects(4));
    let rows = join_ok(
        &[case],
        BTreeMap::from([(receipt.case_id.clone(), receipt)]),
    )?;
    assert_eq!(
        rows[0].eligibility,
        super::eligibility::INELIGIBLE_NO_FOCUSED_MUTANT
    );
    assert_eq!(rows[0].runtime_result, RESULT_NOT_RUN);
    assert_ne!(rows[0].runtime_result, RESULT_CAUGHT);
    assert!(super::eligibility::ELIGIBILITY.contains(&rows[0].eligibility));
    Ok(())
}

#[test]
fn no_denominator_is_not_measurable_not_zero_percent() -> Result<(), String> {
    let (quiet, _, _) = quiet_gap_limit();
    let report = report_for(&join_ok(&[quiet], BTreeMap::new())?)?;
    assert_eq!(report.value["counts"]["completed"].as_u64(), Some(0));
    assert_eq!(
        report.value["candidates"]["false_exposed"]["rate"].as_str(),
        Some("not_measurable")
    );
    assert_ne!(
        report.value["candidates"]["false_exposed"]["rate"].as_str(),
        Some("0%")
    );
    assert_ne!(
        report.value["candidates"]["false_exposed"]["rate"].as_str(),
        Some("0/0")
    );
    assert!(report.markdown.contains("false_exposed: not_measurable"));
    Ok(())
}

#[test]
fn canonical_retained_scorecard_matches_fresh_derivation() -> Result<(), String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| "xtask manifest has no repository parent".to_string())?;
    super::validate_retained_at(root)
}
