use super::*;
use serde_json::json;

fn finding(class: &str, line: u64, currentness: &str) -> Value {
    json!({
        "classification": class,
        "source_currentness": currentness,
        "probe": {"file": "./src/lib.rs", "line": line},
        "related_tests_total": 1,
        "related_tests": [{"name": "t"}],
        "ripr": {"reach": {"state": "yes"}, "discriminate": {"state": "no"}},
    })
}

fn anchor() -> Anchor {
    Anchor {
        file: "src/lib.rs".to_string(),
        line: 10,
    }
}

fn repo_corpus_dir() -> PathBuf {
    crate::dogfood::repo_rooted_fixture_path(CORPUS_DIR)
}

#[test]
fn score_follows_the_truth_table_in_both_error_directions() {
    use Outcome::*;
    use TruthState::*;
    use Verdict::*;
    let table = [
        (Discriminated, Credited, Ideal),
        (Discriminated, Limited, Abstained),
        (Discriminated, Silent, Abstained),
        (Discriminated, Gap, FalseActionable),
        (NotDiscriminated, Gap, Ideal),
        (NotDiscriminated, Limited, Abstained),
        (NotDiscriminated, Credited, FalseExposed),
        (NotDiscriminated, Silent, FalseSilent),
        (PartiallyDiscriminated, Gap, Ideal),
        (PartiallyDiscriminated, Limited, Abstained),
        (PartiallyDiscriminated, Credited, FalseExposed),
        (PartiallyDiscriminated, Silent, FalseSilent),
    ];
    for (truth, observed, expected) in table {
        assert_eq!(score(truth, observed), expected, "{truth:?} x {observed:?}");
    }
}

#[test]
fn finding_verdict_mirrors_triage_classes_and_named_limits() {
    assert_eq!(
        finding_verdict(&finding("exposed", 10, "candidate_current")),
        Verdict::Credited
    );
    assert_eq!(
        finding_verdict(&finding("weakly_exposed", 10, "candidate_current")),
        Verdict::Gap
    );
    assert_eq!(
        finding_verdict(&finding("reachable_unrevealed", 10, "candidate_current")),
        Verdict::Gap
    );
    for class in [
        "no_static_path",
        "infection_unknown",
        "propagation_unknown",
        "static_unknown",
    ] {
        assert_eq!(
            finding_verdict(&finding(class, 10, "candidate_current")),
            Verdict::Limited
        );
    }
    // A producer-named limitation keeps a reachable_unrevealed class limited.
    let mut limited = finding("reachable_unrevealed", 10, "candidate_current");
    limited["static_limit_kind"] = json!("rust_integration_public_api_path_unresolved");
    assert_eq!(finding_verdict(&limited), Verdict::Limited);
    let mut null_limit = finding("reachable_unrevealed", 10, "candidate_current");
    null_limit["static_limit_kind"] = Value::Null;
    assert_eq!(finding_verdict(&null_limit), Verdict::Gap);
}

#[test]
fn anchored_findings_keep_only_candidate_current_findings_on_the_anchor() {
    let check = json!({"findings": [
        finding("exposed", 10, "candidate_current"),
        finding("reachable_unrevealed", 10, "base_deleted"),
        finding("reachable_unrevealed", 11, "candidate_current"),
    ]});
    let anchored = anchored_findings(&check, &anchor());
    assert_eq!(anchored.len(), 1);
    assert_eq!(case_verdict(&anchored), Verdict::Credited);
    let other = Anchor {
        file: "src/other.rs".to_string(),
        line: 10,
    };
    assert_eq!(
        case_verdict(&anchored_findings(&check, &other)),
        Verdict::Silent
    );
}

#[test]
fn case_verdict_ranks_gap_over_credit_over_limit() {
    let gap = finding("weakly_exposed", 10, "candidate_current");
    let credit = finding("exposed", 10, "candidate_current");
    let limit = finding("static_unknown", 10, "candidate_current");
    assert_eq!(case_verdict(&[&credit, &gap, &limit]), Verdict::Gap);
    assert_eq!(case_verdict(&[&limit, &credit]), Verdict::Credited);
    assert_eq!(case_verdict(&[&limit]), Verdict::Limited);
    assert_eq!(case_verdict(&[]), Verdict::Silent);
}

#[test]
fn contradictions_flag_each_internal_inconsistency_and_pass_a_clean_finding() {
    let clean = finding("reachable_unrevealed", 10, "candidate_current");
    assert!(finding_contradictions(&clean).is_empty());

    let mut reach = clean.clone();
    reach["related_tests_total"] = json!(0);
    reach["related_tests"] = json!([]);
    assert_eq!(
        finding_contradictions(&reach),
        vec!["reach_yes_without_related_tests"]
    );

    let mut no_path = clean.clone();
    no_path["classification"] = json!("no_static_path");
    no_path["ripr"]["reach"]["state"] = json!("no");
    no_path["related_tests_total"] = json!(81);
    assert_eq!(
        finding_contradictions(&no_path),
        vec!["no_static_path_with_related_tests"]
    );

    let mut exposed = clean.clone();
    exposed["classification"] = json!("exposed");
    assert_eq!(
        finding_contradictions(&exposed),
        vec!["exposed_without_discriminator"]
    );
    exposed["ripr"]["discriminate"]["state"] = json!("yes");
    assert!(finding_contradictions(&exposed).is_empty());

    let mut listed = clean;
    listed["related_tests"] = json!([{"name": "a"}, {"name": "b"}]);
    assert_eq!(
        finding_contradictions(&listed),
        vec!["related_tests_listed_exceed_total"]
    );
}

#[test]
fn summary_contradictions_compare_counts_with_the_findings_list() {
    let consistent = json!({
        "summary": {"findings": 1, "exposed": 0, "reachable_unrevealed": 1},
        "findings": [finding("reachable_unrevealed", 10, "candidate_current")],
    });
    assert!(summary_contradictions(&consistent).is_empty());
    let drifted = json!({
        "summary": {"findings": 2, "exposed": 1, "reachable_unrevealed": 1},
        "findings": [finding("reachable_unrevealed", 10, "candidate_current")],
    });
    assert_eq!(
        summary_contradictions(&drifted),
        vec![
            "summary_findings_count_mismatch".to_string(),
            "summary_exposed_count_mismatch".to_string()
        ]
    );
}

#[test]
fn ratio_text_is_fixed_precision_and_names_an_empty_denominator() {
    assert_eq!(ratio(3, 21).rate, "0.1429");
    assert_eq!(ratio(1, 3).rate, "0.3333");
    assert_eq!(ratio(2, 3).rate, "0.6667");
    assert_eq!(ratio(1, 1).rate, "1.0000");
    assert_eq!(ratio(0, 5).rate, "0.0000");
    assert_eq!(ratio(0, 0).rate, "n/a");
}

const PATCH: &str = "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -2,3 +2,3 @@ fn f() {\n one\n-    if n >= 100 {\n+    if n > 99 {\n three\n";

#[test]
fn apply_patch_rewrites_the_anchored_line_and_reports_it_as_added() -> Result<(), String> {
    let patches = parse_patch(PATCH)?;
    assert_eq!(patches.len(), 1);
    assert_eq!(patches[0].path, "src/lib.rs");
    assert_eq!(patches[0].added_lines(), BTreeSet::from([3]));
    let original = "zero\none\n    if n >= 100 {\nthree\nfour\n";
    let patched = apply_patch(original, &patches[0])?;
    assert_eq!(patched, "zero\none\n    if n > 99 {\nthree\nfour\n");
    Ok(())
}

#[test]
fn apply_patch_refuses_drifted_context() -> Result<(), String> {
    let patches = parse_patch(PATCH)?;
    let drifted = "zero\none\n    if n >= 101 {\nthree\n";
    let error = apply_patch(drifted, &patches[0]).err().unwrap_or_default();
    assert!(
        error.contains("does not match the patch context"),
        "{error}"
    );
    Ok(())
}

#[test]
fn parse_patch_refuses_renames_and_empty_input() {
    let rename = "--- a/src/a.rs\n+++ b/src/b.rs\n@@ -1 +1 @@\n-a\n+b\n";
    let rename_error = parse_patch(rename).err().unwrap_or_default();
    assert!(rename_error.contains("rename"), "{rename_error}");
    let empty_error = parse_patch("").err().unwrap_or_default();
    assert!(
        empty_error.contains("no `---`/`+++` file patches"),
        "{empty_error}"
    );
}

#[test]
fn parse_patch_holds_hunks_to_their_declared_counts_and_starts() {
    // A new start that does not follow from the old start would move the
    // anchor off the line the edit really adds.
    let shifted = PATCH.replace("+2,3 @@", "+52,3 @@");
    let shifted_error = parse_patch(&shifted).err().unwrap_or_default();
    assert!(
        shifted_error.contains("does not follow from old start 2"),
        "{shifted_error}"
    );
    // A truncated hunk is refused rather than applied partially.
    let truncated = PATCH.strip_suffix(" three\n").unwrap_or_default();
    let truncated_error = parse_patch(truncated).err().unwrap_or_default();
    assert!(
        truncated_error.contains("hunk ends early"),
        "{truncated_error}"
    );
    // Pure insertions and deletions cannot pin the anchor, so they are refused.
    let insertion = "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -769,0 +820,1 @@\n+x\n";
    let insertion_error = parse_patch(insertion).err().unwrap_or_default();
    assert!(
        insertion_error.contains("only inserts or only deletes"),
        "{insertion_error}"
    );
    // A second file header is never swallowed as a removed line.
    let two_files = format!(
        "{}--- a/src/two.rs\n+++ b/src/two.rs\n@@ -1 +1 @@\n-x\n+y\n",
        PATCH
    );
    let patches = parse_patch(&two_files).unwrap_or_default();
    assert_eq!(
        patches.iter().map(|p| p.path.as_str()).collect::<Vec<_>>(),
        vec!["src/lib.rs", "src/two.rs"]
    );
}

#[test]
fn summary_contradictions_account_for_suppressed_findings() {
    let suppressed = json!({
        "summary": {"findings": 2, "reachable_unrevealed": 1, "exposed": 0, "suppressed_by_policy": 1},
        "findings": [
            finding("reachable_unrevealed", 10, "candidate_current"),
            finding("reachable_unrevealed", 11, "candidate_current"),
        ],
    });
    assert!(
        summary_contradictions(&suppressed).is_empty(),
        "{:?}",
        summary_contradictions(&suppressed)
    );
    let mut drifted = suppressed.clone();
    drifted["summary"]["suppressed_by_policy"] = json!(0);
    assert_eq!(
        summary_contradictions(&drifted),
        vec!["summary_suppression_count_mismatch".to_string()]
    );
}

#[test]
fn committed_corpus_is_valid_and_measures_both_error_directions() -> Result<(), String> {
    let dir = repo_corpus_dir();
    let corpus = load_corpus(&dir)?;
    let violations = validate(&corpus, &dir);
    assert!(violations.is_empty(), "{violations:#?}");
    assert!(
        corpus.cases.len() >= 20,
        "corpus shrank to {}",
        corpus.cases.len()
    );
    let subjects: BTreeSet<&str> = corpus
        .subjects
        .iter()
        .map(|s| s.subject_id.as_str())
        .collect();
    assert!(subjects.len() >= 5, "{subjects:?}");
    assert!(corpus.cases.iter().any(|c| c.hard_case.is_some()));
    // The serde 256-value loop is the named hard case this corpus exists to keep.
    assert!(
        corpus
            .cases
            .iter()
            .any(|c| c.case_id == "serde-format-u8-hundreds"
                && c.truth.state == TruthState::Discriminated)
    );
    Ok(())
}

fn tampered(edit: impl Fn(&mut Value)) -> Result<Vec<String>, String> {
    let dir = repo_corpus_dir();
    let mut raw: Value =
        serde_json::from_str(&read(&dir.join("corpus.json"))?).map_err(|err| err.to_string())?;
    edit(&mut raw);
    let corpus: Corpus = serde_json::from_value(raw).map_err(|err| err.to_string())?;
    Ok(validate(&corpus, &dir))
}

#[test]
fn validator_rejects_a_label_that_contradicts_its_mutant_outcomes() -> Result<(), String> {
    let violations = tampered(|raw| {
        raw["cases"][0]["truth"]["mutants"][0]["outcome"] = json!("tests_passed");
        raw["cases"][0]["truth"]["mutants"][0]["failing_test"] = Value::Null;
    })?;
    assert!(
        violations
            .iter()
            .any(|v| v.contains("does not follow from")),
        "{violations:#?}"
    );
    Ok(())
}

#[test]
fn validator_rejects_an_expected_verdict_outside_the_truth_table() -> Result<(), String> {
    let violations = tampered(|raw| {
        raw["cases"][0]["expected"]["ideal_verdict"] = json!("gap");
        raw["cases"][0]["expected"]["acceptable_verdicts"] = json!(["gap", "credited"]);
    })?;
    assert!(
        violations.iter().any(|v| v.contains("contradicts truth")),
        "{violations:#?}"
    );
    assert!(
        violations
            .iter()
            .any(|v| v.contains("drift from the truth table")),
        "{violations:#?}"
    );
    Ok(())
}

#[test]
fn validator_rejects_a_retained_file_whose_digest_moved() -> Result<(), String> {
    let violations = tampered(|raw| {
        raw["subjects"][0]["retained_files"][0]["sha256"] = json!("0".repeat(64));
    })?;
    assert!(
        violations
            .iter()
            .any(|v| v.contains("does not match its pinned sha256")),
        "{violations:#?}"
    );
    Ok(())
}

#[test]
fn validator_rejects_an_anchor_the_diff_does_not_add() -> Result<(), String> {
    let violations = tampered(|raw| {
        let line = raw["cases"][0]["anchor"]["line"].as_u64().unwrap_or(0);
        raw["cases"][0]["anchor"]["line"] = json!(line + 1);
    })?;
    assert!(
        violations
            .iter()
            .any(|v| v.contains("diff does not add line")),
        "{violations:#?}"
    );
    Ok(())
}

#[test]
fn validator_rejects_ids_that_are_not_one_safe_path_segment() -> Result<(), String> {
    let violations = tampered(|raw| {
        raw["cases"][0]["case_id"] = json!("../escape");
        raw["subjects"][0]["subject_id"] = json!("a/b");
    })?;
    assert!(
        violations
            .iter()
            .any(|v| v.contains("case id `../escape` is not a single safe path segment")),
        "{violations:#?}"
    );
    assert!(
        violations
            .iter()
            .any(|v| v.contains("subject id `a/b` is not a single safe path segment")),
        "{violations:#?}"
    );
    Ok(())
}

#[test]
fn validator_rejects_a_diff_that_patches_an_unretained_path() -> Result<(), String> {
    let dir = repo_corpus_dir();
    let mut raw: Value =
        serde_json::from_str(&read(&dir.join("corpus.json"))?).map_err(|err| err.to_string())?;
    let case_diff = raw["cases"][0]["diff"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let original = read(&dir.join(&case_diff))?;
    let escape = "diff --git a/../../Cargo.toml b/../../Cargo.toml\n--- a/../../Cargo.toml\n+++ b/../../Cargo.toml\n@@ -1,1 +1,1 @@\n-[workspace]\n+[package]\n";
    let scratch =
        std::env::temp_dir().join(format!("verdict-corpus-escape-{}", std::process::id()));
    fs::create_dir_all(scratch.join("cases")).map_err(|err| err.to_string())?;
    fs::write(
        scratch.join("cases/escape.diff"),
        format!("{original}{escape}"),
    )
    .map_err(|err| err.to_string())?;
    raw["cases"][0]["diff"] = json!("cases/escape.diff");
    let corpus: Corpus = serde_json::from_value(raw).map_err(|err| err.to_string())?;
    let case = &corpus.cases[0];
    let subject = corpus
        .subjects
        .iter()
        .find(|s| s.subject_id == case.subject_id)
        .ok_or("subject")?;
    let violations = case_violations(case, subject, &scratch);
    let _ = fs::remove_dir_all(&scratch);
    assert!(
        violations
            .iter()
            .any(|v| v.contains("diff patches `../../Cargo.toml`, which is unsafe or not retained")),
        "{violations:#?}"
    );
    Ok(())
}

#[test]
fn validator_requires_both_truth_directions() -> Result<(), String> {
    let violations = tampered(|raw| {
        if let Some(cases) = raw["cases"].as_array_mut() {
            cases.retain(|c| c["truth"]["state"] != json!("not_discriminated"));
        }
    })?;
    assert!(
        violations
            .iter()
            .any(|v| v.contains("no `not_discriminated` case")),
        "{violations:#?}"
    );
    Ok(())
}

#[test]
fn expected_report_rows_agree_with_corpus_labels() -> Result<(), String> {
    let dir = repo_corpus_dir();
    let corpus = load_corpus(&dir)?;
    let report: Value = serde_json::from_str(&read(&dir.join("expected/report.json"))?)
        .map_err(|err| err.to_string())?;
    let rows = report["rows"].as_array().cloned().unwrap_or_default();
    assert_eq!(rows.len(), corpus.cases.len());
    for (row, case) in rows.iter().zip(&corpus.cases) {
        assert_eq!(row["case_id"], json!(case.case_id));
        assert_eq!(row["truth"], json!(case.truth.state.as_str()));
        let observed: Verdict =
            serde_json::from_value(row["observed_verdict"].clone()).map_err(|e| e.to_string())?;
        assert_eq!(
            row["outcome"],
            json!(score(case.truth.state, observed).as_str()),
            "{}",
            case.case_id
        );
    }
    Ok(())
}

#[test]
fn build_report_counts_rates_over_the_right_denominators() -> Result<(), String> {
    let dir = repo_corpus_dir();
    let corpus = load_corpus(&dir)?;
    // Every case observed as a gap: discriminated cases become false
    // actionable, the rest are ideal.
    let checks: Vec<(String, Value)> = corpus
        .cases
        .iter()
        .map(|case| {
            let mut f = finding(
                "weakly_exposed",
                case.anchor.line as u64,
                "candidate_current",
            );
            f["probe"]["file"] = json!(case.anchor.file);
            (case.case_id.clone(), json!({"findings": [f]}))
        })
        .collect();
    let report = build_report(&corpus, &checks)?;
    let discriminated = corpus
        .cases
        .iter()
        .filter(|c| c.truth.state == TruthState::Discriminated)
        .count();
    assert_eq!(report.false_actionable_rate.numerator, discriminated);
    assert_eq!(report.false_actionable_rate.denominator, discriminated);
    assert_eq!(report.false_exposed_rate.numerator, 0);
    assert_eq!(report.false_verdict_rate.numerator, discriminated);
    assert_eq!(
        report.ideal_rate.numerator,
        corpus.cases.len() - discriminated
    );
    let missing = build_report(&corpus, &checks[1..])
        .err()
        .unwrap_or_default();
    assert!(missing.contains("no ripr result for case"), "{missing}");
    Ok(())
}

#[test]
fn relativize_probe_files_strips_only_the_run_root() {
    let mut check = json!({"findings": [
        {"source_currentness": "candidate_current", "probe": {"file": "/work/case/src/lib.rs", "line": 10}},
        {"source_currentness": "candidate_current", "probe": {"file": "./src/lib.rs", "line": 10}},
        {"source_currentness": "candidate_current", "probe": {"file": "/elsewhere/src/lib.rs", "line": 10}},
    ]});
    relativize_probe_files(&mut check, Path::new("/work/case"));
    assert_eq!(check["findings"][0]["probe"]["file"], json!("src/lib.rs"));
    assert_eq!(check["findings"][1]["probe"]["file"], json!("./src/lib.rs"));
    assert_eq!(
        check["findings"][2]["probe"]["file"],
        json!("/elsewhere/src/lib.rs")
    );
    // `./src/lib.rs` already reads as root-relative; the foreign root does not.
    assert_eq!(anchored_findings(&check, &anchor()).len(), 2);
}

#[test]
fn contradiction_counts_use_one_per_finding_unit() -> Result<(), String> {
    let dir = repo_corpus_dir();
    let corpus = load_corpus(&dir)?;
    let mut contradicted = finding("reachable_unrevealed", 10, "candidate_current");
    contradicted["related_tests_total"] = json!(0);
    contradicted["related_tests"] = json!([]);
    let mut other_line = contradicted.clone();
    other_line["probe"]["line"] = json!(99);
    let checks: Vec<(String, Value)> = corpus
        .cases
        .iter()
        .enumerate()
        .map(|(i, case)| {
            let findings = if i == 0 {
                json!([contradicted, other_line])
            } else {
                json!([])
            };
            (case.case_id.clone(), json!({"findings": findings}))
        })
        .collect();
    let report = build_report(&corpus, &checks)?;
    // Two contradicted findings in one case: the rate and the per-code count
    // agree, while the row lists the code once.
    assert_eq!(report.contradiction_rate.numerator, 2);
    assert_eq!(report.contradiction_rate.denominator, 2);
    assert_eq!(
        report
            .contradictions_by_code
            .get("reach_yes_without_related_tests"),
        Some(&2)
    );
    assert_eq!(
        report.rows[0].contradictions,
        vec!["reach_yes_without_related_tests".to_string()]
    );
    Ok(())
}

#[test]
fn stored_paths_keep_vendored_rust_out_of_the_workspace() {
    assert_eq!(stored_path("src/lib.rs"), "src/lib.rs.txt");
    assert_eq!(stored_path("Cargo.toml"), "Cargo.toml");
    assert_eq!(
        logical_path("src/lib.rs.txt").as_deref(),
        Some("src/lib.rs")
    );
    assert_eq!(logical_path("LICENSE-MIT").as_deref(), Some("LICENSE-MIT"));
    // A bare `.rs` file under subjects is refused, not silently copied.
    assert_eq!(logical_path("src/lib.rs"), None);
}
