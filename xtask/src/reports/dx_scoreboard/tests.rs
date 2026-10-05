#[cfg(unix)]
use super::measure::linked_target;
use super::measure::{
    PasteVerdict, Probe, builds_ripr_from_source, check_contradictions, classify_replay,
    contradiction_outcome, extract_commands, hostile_outcome, parse_test_result, probe_result,
    repo_exposure_contradictions, rss_sample,
};
use super::*;
use crate::run::{MeasuredOutput, TimedOutput};

/// The committed config, resolved from the crate root so a test that moves
/// the working directory cannot break the lookup.
fn committed_config() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(DEFAULT_CONFIG)
}

const MINIMAL: &str = r#"
schema_version = "ripr-dx-scoreboards-v1"

[[corpus]]
id = "serde"
url = "https://github.com/serde-rs/serde"
sha = "dd18663450ec3ee38e35e3a4ee141932614e662c"

[[metric]]
id = "speed.warm_check_ms"
board = "speed"
title = "Warm check"
unit = "ms"
direction = "lower_is_better"
target = 2000
regression_pct = 25
regression_floor = 500
runner_dependent = true
source = "measured"
per_repo = true

[[metric]]
id = "ci.workflow_lines"
board = "ci"
title = "Workflow lines"
unit = "lines"
direction = "lower_is_better"
target = 150
regression_pct = 10
regression_floor = 20
runner_dependent = false
source = "measured"

[[metric]]
id = "first_run.friction_events"
board = "first_run"
title = "Friction"
unit = "events"
direction = "lower_is_better"
target = 0
regression_pct = 0
regression_floor = 0
runner_dependent = false
source = "ingest:first-run"
"#;

fn sample(metric: &str, repo: Option<&str>, outcome: SampleOutcome) -> Sample {
    Sample {
        metric: metric.to_string(),
        repo: repo.map(str::to_string),
        outcome,
        detail: String::new(),
    }
}

fn all_boards() -> Vec<String> {
    BOARDS.iter().map(|board| (*board).to_string()).collect()
}

fn context(runner: &str) -> RunContext {
    RunContext {
        revision: "abc".to_string(),
        runner_class: runner.to_string(),
        ..RunContext::default()
    }
}

fn metric<'a>(report: &'a Value, id: &str) -> Result<&'a Value, String> {
    report["metrics"]
        .as_array()
        .and_then(|rows| rows.iter().find(|row| row["id"].as_str() == Some(id)))
        .ok_or_else(|| format!("metric {id} missing from report"))
}

#[test]
fn committed_scoreboards_config_parses_and_pins_full_shas() -> Result<(), String> {
    let config = load_config(&committed_config())?;
    assert!(
        config.corpus.len() >= 3,
        "corpus should name real repositories"
    );
    for board in BOARDS {
        assert!(
            config.metric.iter().any(|metric| metric.board == board),
            "board {board} has no metric"
        );
    }
    Ok(())
}

#[test]
fn config_rejects_short_sha_unknown_source_and_misprefixed_ids() {
    let short = MINIMAL.replace("dd18663450ec3ee38e35e3a4ee141932614e662c", "dd18663");
    assert!(parse_config(&short).is_err_and(|err| err.contains("40-character")));
    let source = MINIMAL.replace("source = \"ingest:first-run\"", "source = \"guess\"");
    assert!(parse_config(&source).is_err_and(|err| err.contains("unknown source")));
    let prefix = MINIMAL.replace("id = \"ci.workflow_lines\"", "id = \"workflow_lines\"");
    assert!(parse_config(&prefix).is_err_and(|err| err.contains("prefixed")));
    let duplicate = format!(
        "{MINIMAL}\n[[corpus]]\nid = \"serde\"\nurl = \"u\"\nsha = \"dd18663450ec3ee38e35e3a4ee141932614e662c\"\n"
    );
    assert!(parse_config(&duplicate).is_err_and(|err| err.contains("duplicate corpus")));
}

#[test]
fn ingest_accepts_its_own_source_and_refuses_measured_metrics() -> Result<(), String> {
    let config = parse_config(MINIMAL)?;
    let good = json!({
        "schema_version": INPUT_SCHEMA_VERSION,
        "source": "first-run",
        "metrics": [{"id": "first_run.friction_events", "value": 3, "evidence": "log.md"}],
    });
    let samples = parse_ingest(&good, &config)?;
    assert_eq!(samples.len(), 1);
    assert_eq!(
        samples.first().map(|s| s.outcome.clone()),
        Some(SampleOutcome::Value(3.0))
    );

    let overwrite = json!({
        "schema_version": INPUT_SCHEMA_VERSION,
        "source": "first-run",
        "metrics": [{"id": "ci.workflow_lines", "value": 1}],
    });
    assert!(parse_ingest(&overwrite, &config).is_err_and(|err| err.contains("sourced from")));

    let wrong_schema = json!({"schema_version": "v0", "source": "first-run", "metrics": []});
    assert!(
        parse_ingest(&wrong_schema, &config).is_err_and(|err| err.contains(INPUT_SCHEMA_VERSION))
    );
    Ok(())
}

#[test]
fn zero_denominator_is_not_measured_not_a_perfect_rate() {
    let json = json!({"candidates": {"false_actionable": {"numerator": 0, "denominator": 0}}});
    let empty = ratio_sample("trust.x", "f.json", "candidates.false_actionable", &json);
    assert_eq!(empty.outcome, SampleOutcome::NotMeasured);

    let json = json!({"candidates": {"false_actionable": {"numerator": 1, "denominator": 4}}});
    let rate = ratio_sample("trust.x", "f.json", "candidates.false_actionable", &json);
    assert_eq!(rate.outcome, SampleOutcome::Value(0.25));
}

#[test]
fn metric_value_is_the_worst_repo_and_incomplete_never_meets_target() -> Result<(), String> {
    let config = parse_config(MINIMAL)?;
    let samples = vec![
        sample(
            "speed.warm_check_ms",
            Some("a"),
            SampleOutcome::Value(900.0),
        ),
        sample(
            "speed.warm_check_ms",
            Some("b"),
            SampleOutcome::Value(2600.0),
        ),
    ];
    let report = build_report(&config, &all_boards(), &samples, &context("r"), None, false);
    let row = metric(&report, "speed.warm_check_ms")?;
    assert_eq!(row["value"].as_f64(), Some(2600.0));
    assert_eq!(row["status"].as_str(), Some("below_target"));

    let fast_but_incomplete = vec![sample(
        "speed.warm_check_ms",
        Some("a"),
        SampleOutcome::Incomplete(10.0),
    )];
    let report = build_report(
        &config,
        &all_boards(),
        &fast_but_incomplete,
        &context("r"),
        None,
        false,
    );
    assert_eq!(
        metric(&report, "speed.warm_check_ms")?["status"].as_str(),
        Some("below_target")
    );
    Ok(())
}

#[test]
fn missing_ingest_is_not_measured_and_counted_in_rollup() -> Result<(), String> {
    let config = parse_config(MINIMAL)?;
    let samples = vec![sample(
        "ci.workflow_lines",
        None,
        SampleOutcome::Value(120.0),
    )];
    let report = build_report(&config, &all_boards(), &samples, &context("r"), None, false);
    let friction = metric(&report, "first_run.friction_events")?;
    assert_eq!(friction["status"].as_str(), Some("not_measured"));
    assert!(
        friction["reason"]
            .as_str()
            .is_some_and(|r| r.contains("--ingest"))
    );
    assert_eq!(report["rollup"]["meets_target"].as_u64(), Some(1));
    assert_eq!(report["rollup"]["not_measured"].as_u64(), Some(2));
    Ok(())
}

#[test]
fn gate_uses_the_larger_margin_and_skips_other_runner_classes() -> Result<(), String> {
    let config = parse_config(MINIMAL)?;
    let baseline_samples = vec![
        sample(
            "speed.warm_check_ms",
            Some("a"),
            SampleOutcome::Value(4000.0),
        ),
        sample("ci.workflow_lines", None, SampleOutcome::Value(100.0)),
    ];
    let baseline = build_report(
        &config,
        &all_boards(),
        &baseline_samples,
        &context("runner-a"),
        None,
        false,
    );

    // 4000 -> 4900 is within 25% (1000); 100 -> 119 is within the 20-line floor.
    let within = vec![
        sample(
            "speed.warm_check_ms",
            Some("a"),
            SampleOutcome::Value(4900.0),
        ),
        sample("ci.workflow_lines", None, SampleOutcome::Value(119.0)),
    ];
    let report = build_report(
        &config,
        &all_boards(),
        &within,
        &context("runner-a"),
        Some(&baseline),
        true,
    );
    assert_eq!(report["gate"]["status"].as_str(), Some("pass"));

    // 4000 -> 5100 exceeds 25%; 100 -> 121 exceeds the floor.
    let worse = vec![
        sample(
            "speed.warm_check_ms",
            Some("a"),
            SampleOutcome::Value(5100.0),
        ),
        sample("ci.workflow_lines", None, SampleOutcome::Value(121.0)),
    ];
    let report = build_report(
        &config,
        &all_boards(),
        &worse,
        &context("runner-a"),
        Some(&baseline),
        true,
    );
    assert_eq!(report["gate"]["status"].as_str(), Some("fail"));
    assert_eq!(
        report["gate"]["regressions"].as_array().map(Vec::len),
        Some(2)
    );

    // On another runner class only the runner-independent metric is compared.
    let report = build_report(
        &config,
        &all_boards(),
        &worse,
        &context("runner-b"),
        Some(&baseline),
        true,
    );
    assert_eq!(
        report["gate"]["regressions"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(
        metric(&report, "speed.warm_check_ms")?["baseline"]["comparable"].as_bool(),
        Some(false)
    );
    Ok(())
}

#[test]
fn gate_fails_on_a_broken_instrument_even_without_baseline() -> Result<(), String> {
    let config = parse_config(MINIMAL)?;
    let samples = vec![sample("ci.workflow_lines", None, SampleOutcome::Failed)];
    let report = build_report(&config, &all_boards(), &samples, &context("r"), None, true);
    assert_eq!(report["gate"]["status"].as_str(), Some("fail"));
    let clean = vec![sample(
        "ci.workflow_lines",
        None,
        SampleOutcome::Value(10.0),
    )];
    let report = build_report(&config, &all_boards(), &clean, &context("r"), None, true);
    assert_eq!(report["gate"]["status"].as_str(), Some("no_baseline"));
    Ok(())
}

#[test]
fn repo_view_groups_per_repo_samples() -> Result<(), String> {
    let config = parse_config(MINIMAL)?;
    let samples = vec![sample(
        "speed.warm_check_ms",
        Some("serde"),
        SampleOutcome::Value(700.0),
    )];
    let report = build_report(&config, &all_boards(), &samples, &context("r"), None, false);
    let serde = report["repos"]
        .as_array()
        .and_then(|repos| repos.first())
        .ok_or("repo row missing")?;
    assert_eq!(serde["id"].as_str(), Some("serde"));
    assert_eq!(serde["status"].as_str(), Some("meets_target"));
    assert!(render_markdown(&report).contains("## By repository"));
    Ok(())
}

#[test]
fn extract_commands_reads_spans_labels_and_bash_fences_but_not_powershell() {
    let text = "\
ripr progress: analyzing [diff]
  ripr explain --root '/tmp/a b' --base 'HEAD~1' probe:x
  repair this seam: ripr agent repair --root '/r' --seam-id 1 --phase before
Regeneration command: `ripr check --root '/r' --format json > '/r/out.json'`
Regeneration command (PowerShell): `ripr check --root '/r''s' --format json`
- Cache size: 8 KB (run `ripr cache status` for details)
```bash
ripr agent repair --root '/fence' --phase before
```
```powershell
ripr agent repair --root '/ps''s' --phase before
```
";
    let commands = extract_commands(text);
    assert_eq!(
        commands,
        vec![
            "ripr explain --root '/tmp/a b' --base 'HEAD~1' probe:x".to_string(),
            "ripr agent repair --root '/r' --seam-id 1 --phase before".to_string(),
            "ripr check --root '/r' --format json > '/r/out.json'".to_string(),
            "ripr agent repair --root '/fence' --phase before".to_string(),
        ]
    );
}

#[test]
fn classify_replay_separates_bound_unbound_and_split_roots() {
    let root = "/tmp/x/dx paste $(touch PWNED) it's \"q\"";
    let bound = format!("explain\0--root\0{root}\0probe:x\0");
    assert_eq!(
        classify_replay(true, Some(bound.as_bytes()), Some("/tmp/caller"), root).0,
        PasteVerdict::Bound
    );
    let via_cd = b"check\0--format\0json\0";
    assert_eq!(
        classify_replay(true, Some(via_cd), Some(root), root).0,
        PasteVerdict::Bound
    );
    assert_eq!(
        classify_replay(true, Some(via_cd), Some("/tmp/caller"), root).0,
        PasteVerdict::Unbound
    );
    let split = b"check\0--root\0/tmp/x/dx\0paste\0";
    let split_root = b"check\0--root\0/tmp/x/dx paste its q\0";
    assert_eq!(
        classify_replay(true, Some(split), Some("/tmp/caller"), root).0,
        PasteVerdict::Unsafe
    );
    assert_eq!(
        classify_replay(true, Some(split_root), Some("/tmp/caller"), root).0,
        PasteVerdict::Unsafe
    );
    assert_eq!(
        classify_replay(false, None, None, root).0,
        PasteVerdict::Unsafe
    );
    // Reaching ripr with the root intact is not enough if the line then fails.
    assert_eq!(
        classify_replay(false, Some(bound.as_bytes()), Some("/tmp/caller"), root).0,
        PasteVerdict::Unbound
    );
}

#[test]
fn contradiction_rules_fire_only_on_self_inconsistent_rows() {
    let exposure = json!({"seams": [
        {"file": "a.rs", "line": 1, "evidence": {"reach": "no"}, "related_tests_total": 81},
        {"file": "b.rs", "line": 2, "evidence": {"reach": "no"}, "related_tests_total": 0},
        {"file": "c.rs", "line": 3, "evidence": {"reach": "yes"}, "related_tests_total": 4},
    ]});
    assert_eq!(repo_exposure_contradictions(&exposure).0, 1);

    let check = json!({"findings": [
        {"id": "p1", "classification": "no_static_path", "related_tests_total": 2, "evidence": []},
        {"id": "p2", "classification": "static_unknown", "related_tests_total": 0,
         "evidence": ["Related tests were found, but no assertion appears to observe the changed value"]},
        {"id": "p3", "classification": "weakly_exposed", "related_tests_total": 3,
         "evidence": ["Related tests were found, but no assertion appears to observe the changed value"]},
    ]});
    let (count, examples) = check_contradictions(&check);
    assert_eq!(count, 2);
    assert!(examples.iter().any(|e| e.starts_with("R2 p1")));
    assert!(examples.iter().any(|e| e.starts_with("R3 p2")));
}

#[test]
fn source_build_detection_ignores_comments() {
    assert!(builds_ripr_from_source(
        "    run: cargo install ripr --version 0.11.0 --locked\n"
    ));
    assert!(!builds_ripr_from_source(
        "      # an uncached `cargo install ripr` recompiles for minutes\n"
    ));
    assert!(!builds_ripr_from_source(
        "  url=\"https://github.com/o/ripr/releases/download/v1/a.tar.gz\"\n  cargo install ripr --version 1 --locked\n"
    ));
}

#[test]
fn vm_hwm_parses_kilobytes() {
    let status = "Name:\tripr\nVmPeak:\t  900 kB\nVmHWM:\t   2048 kB\nVmRSS:\t 1000 kB\n";
    assert_eq!(crate::run::parse_vm_hwm_bytes(status), Some(2048 * 1024));
    assert_eq!(crate::run::parse_vm_hwm_bytes("Name:\tx\n"), None);
}

#[test]
fn parse_options_rejects_unknown_boards() {
    assert!(
        parse_options(&["--boards".to_string(), "speed,vibes".to_string()])
            .is_err_and(|err| err.contains("unknown board"))
    );
    let parsed = parse_options(&["--boards".to_string(), "ci,paste".to_string()]);
    assert!(parsed.is_ok_and(|options| options.boards == vec!["ci", "paste"]));
}

fn first_run_receipt(with_install: bool) -> Value {
    let mut setup = vec![json!({"step": "fetch_sources", "secs": 0.2, "exit": 0, "friction": []})];
    if with_install {
        setup.push(json!({"step": "install_published", "secs": 128.0, "exit": 0, "friction": []}));
    }
    json!({
        "schema_version": "first_run.v1",
        "ripr": "ripr 0.11.0",
        "setup": setup,
        "cases": [
            {"case": "semver", "verdict": "infection_unknown", "steps": [
                {"step": "doctor", "secs": 0.5, "exit": 0, "friction": []},
                {"step": "check", "secs": 1.5, "exit": 0, "friction": []},
                {"step": "init_ci", "secs": 0.1, "exit": 0, "friction": ["workflow is 2374 lines"]},
            ]},
            {"case": "bytesize", "verdict": "weakly_exposed", "steps": [
                {"step": "check", "secs": 2.0, "exit": 2, "friction": ["check exited 2"]},
            ]},
        ],
    })
}

#[test]
fn first_run_receipt_counts_install_through_first_successful_check() -> Result<(), String> {
    let config = parse_config(&MINIMAL.replace(
        "[[metric]]\nid = \"first_run.friction_events\"",
        "[[metric]]\nid = \"first_run.install_seconds\"\nboard = \"first_run\"\ntitle = \"i\"\nunit = \"s\"\ndirection = \"lower_is_better\"\ntarget = 120\nregression_pct = 25\nregression_floor = 30\nrunner_dependent = true\nsource = \"ingest:first-run\"\n\n[[metric]]\nid = \"first_run.time_to_first_useful_result_s\"\nboard = \"first_run\"\ntitle = \"t\"\nunit = \"s\"\ndirection = \"lower_is_better\"\ntarget = 300\nregression_pct = 25\nregression_floor = 60\nrunner_dependent = true\nsource = \"ingest:first-run\"\n\n[[metric]]\nid = \"first_run.unknown_verdicts\"\nboard = \"first_run\"\ntitle = \"u\"\nunit = \"cases\"\ndirection = \"lower_is_better\"\ntarget = 0\nregression_pct = 0\nregression_floor = 0\nrunner_dependent = false\nsource = \"ingest:first-run\"\n\n[[metric]]\nid = \"first_run.friction_events\"",
    ))?;
    let samples = parse_ingest(&first_run_receipt(true), &config)?;
    let find = |metric: &str, repo: Option<&str>| {
        samples
            .iter()
            .find(|s| s.metric == metric && s.repo.as_deref() == repo)
            .map(|s| s.outcome.clone())
    };
    assert_eq!(
        find("first_run.time_to_first_useful_result_s", Some("semver")),
        Some(SampleOutcome::Value(130.0))
    );
    assert_eq!(
        find("first_run.time_to_first_useful_result_s", Some("bytesize")),
        Some(SampleOutcome::Incomplete(130.0))
    );
    assert_eq!(
        find("first_run.install_seconds", None),
        Some(SampleOutcome::Value(128.0))
    );
    assert_eq!(
        find("first_run.friction_events", None),
        Some(SampleOutcome::Value(2.0))
    );
    assert_eq!(
        find("first_run.unknown_verdicts", None),
        Some(SampleOutcome::Value(1.0))
    );

    // An install step with no duration must not pass as a faster install.
    let mut partial = first_run_receipt(true);
    if let Some(setup) = partial["setup"].as_array_mut() {
        setup.push(json!({"step": "install_extra", "exit": 0, "friction": []}));
    }
    let samples = parse_ingest(&partial, &config)?;
    assert_eq!(
        samples
            .iter()
            .find(|s| s.metric == "first_run.install_seconds")
            .map(|s| s.outcome.clone()),
        Some(SampleOutcome::Incomplete(128.0))
    );

    // Without a timed install the journey metric stays unmeasured.
    let samples = parse_ingest(&first_run_receipt(false), &config)?;
    assert!(
        !samples
            .iter()
            .any(|s| s.metric == "first_run.time_to_first_useful_result_s"
                || s.metric == "first_run.install_seconds")
    );

    // An install step with no duration at all is an incomplete sample, not a
    // missing one, so a previously timed install cannot drop out unnoticed.
    let mut untimed = first_run_receipt(false);
    if let Some(setup) = untimed["setup"].as_array_mut() {
        setup.push(json!({"step": "install_published", "exit": 0, "friction": []}));
    }
    let samples = parse_ingest(&untimed, &config)?;
    assert_eq!(
        samples
            .iter()
            .find(|s| s.metric == "first_run.install_seconds")
            .map(|s| s.outcome.clone()),
        Some(SampleOutcome::Incomplete(0.0))
    );

    // A failed install writes no cases; its timing still becomes an
    // incomplete install sample and no case-dependent metric appears.
    let failed = json!({
        "schema_version": "first_run.v1",
        "ripr": "ripr 0.11.0",
        "setup": [{"step": "install_published", "secs": 20.0, "exit": 101, "friction": []}],
        "cases": [],
    });
    let samples = parse_ingest(&failed, &config)?;
    assert_eq!(samples.len(), 1);
    assert_eq!(
        samples
            .first()
            .map(|s| (s.metric.as_str(), s.outcome.clone())),
        Some(("first_run.install_seconds", SampleOutcome::Incomplete(20.0)))
    );

    // A receipt with an install step but no `cases` array is malformed, not
    // an install-failed walk.
    let no_cases = json!({
        "schema_version": "first_run.v1",
        "ripr": "r",
        "setup": [{"step": "install_published", "secs": 20.0, "exit": 0, "friction": []}],
    });
    let rejected = parse_ingest(&no_cases, &config).err();
    assert!(rejected.is_some_and(|e| e.contains("cases array")));

    // No install step and no cases is still rejected.
    let empty = json!({"schema_version": "first_run.v1", "ripr": "r", "setup": [], "cases": []});
    let rejected = parse_ingest(&empty, &config).err();
    assert!(rejected.is_some_and(|e| e.contains("non-empty cases")));
    Ok(())
}

#[test]
fn shared_corpus_manifest_maps_fast_tier_to_default_runs() -> Result<(), String> {
    let manifest = json!({
        "kind": "ripr_rust_corpus_manifest",
        "corpus_version": "2026-10-04.1",
        "repos": [
            {"id": "serde", "url": "u", "tier": "fast",
             "sha": "9d3410e3f4e38f9ea1a798e7ae9fab71577ab31b",
             "base_sha": "b7dbf7e3cb53bc9b9442047229e4f125bb07783e"},
            {"id": "tokio", "url": "u", "tier": "full",
             "sha": "9d3410e3f4e38f9ea1a798e7ae9fab71577ab31b"},
        ],
    });
    let (version, corpus) = corpus_from_manifest(&manifest)?;
    assert_eq!(version, "2026-10-04.1");
    assert_eq!(corpus.len(), 2);
    assert!(
        corpus
            .first()
            .is_some_and(|serde| !serde.heavy && serde.base_sha.is_some())
    );
    assert!(
        corpus
            .get(1)
            .is_some_and(|tokio| tokio.heavy && tokio.base_sha.is_none())
    );
    assert!(corpus_from_manifest(&json!({"kind": "other"})).is_err_and(|e| e.contains("kind")));
    Ok(())
}

#[test]
fn mutation_spot_check_receipt_maps_agreement_and_join_coverage() -> Result<(), String> {
    let receipt = json!({
        "schema_version": "ripr-mutation-spot-check-v1",
        "scored_families": {
            "claims_discriminator": {"agreement_rate": 1.0, "mutants_scored": 14},
            "claims_no_discriminator": {"agreement_rate": 0.043, "mutants_scored": 23},
        },
        "repos": [
            {"pairings": {"seam_precise": 2}, "calibration_metrics": {"mutants_total": 86}},
            {"pairings": {"seam_precise": 170}, "calibration_metrics": {"mutants_total": 1659}},
        ],
        "pilot_top_recommendations": {"scored": 39, "precision": 0.385, "by_tier": {"seam": {"confirmed": 2, "refuted": 5}, "owner": {"confirmed": 13, "refuted": 19}}, "repos": [{"name": "semver"}, {"name": "humantime"}]},
    });
    let input = mutation_spot_check_to_input(&receipt)?;
    let value = |id: &str| {
        input["metrics"]
            .as_array()
            .and_then(|rows| rows.iter().find(|row| row["id"].as_str() == Some(id)))
            .and_then(|row| row["value"].as_f64())
    };
    assert_eq!(value("trust.discriminator_claim_agreement"), Some(1.0));
    assert_eq!(value("trust.gap_claim_agreement"), Some(0.043));
    assert!(
        value("trust.mutation_join_coverage").is_some_and(|v| (v - 172.0 / 1745.0).abs() < 1e-9)
    );
    assert_eq!(
        value("trust.pilot_top_recommendation_precision"),
        Some(0.385)
    );
    let pilot_evidence = input["metrics"].as_array().and_then(|rows| {
        rows.iter()
            .find(|row| row["id"] == "trust.pilot_top_recommendation_precision")
            .and_then(|row| row["evidence"].as_str())
    });
    assert_eq!(
        pilot_evidence,
        Some(
            "39 pilot recommendations scored (seam 2/7, line 0/0, owner 13/32) over semver, humantime"
        )
    );

    let evidence = |input: &Value| input["evidence"].as_str().unwrap_or_default().to_string();
    assert!(
        !evidence(&input).contains("cargo-mutants arguments"),
        "{}",
        evidence(&input)
    );

    let config = load_config(&committed_config())?;
    let samples = parse_ingest(&receipt, &config)?;
    assert_eq!(samples.len(), 4);

    let mut empty_args = receipt.clone();
    empty_args["repos"][0]["cargo_mutants_args"] = json!([]);
    let input = mutation_spot_check_to_input(&empty_args)?;
    assert!(
        !evidence(&input).contains("cargo-mutants arguments"),
        "{}",
        evidence(&input)
    );

    let mut sampled = receipt.clone();
    sampled["repos"][0]["cargo_mutants_args"] = json!(["--re=decode"]);
    let caveat = "1 of 2 repositories ran with cargo-mutants arguments";
    let input = mutation_spot_check_to_input(&sampled)?;
    assert!(evidence(&input).contains(caveat), "{}", evidence(&input));
    // Ingest publishes each row's own evidence, so the caveat must reach
    // every sample, not only the top-level evidence.
    let samples = parse_ingest(&sampled, &config)?;
    assert_eq!(samples.len(), 4);
    for sample in &samples {
        assert!(sample.detail.contains(caveat), "{}", sample.detail);
    }
    let unsampled = parse_ingest(&receipt, &config)?;
    assert!(
        unsampled
            .iter()
            .all(|sample| !sample.detail.contains("cargo-mutants arguments"))
    );

    // A receipt from before the pilot section, or with nothing scored, adds
    // no pilot row rather than a misleading zero.
    // A run with a repository's pilot unavailable measured a smaller
    // population, so it publishes no pilot row either.
    for section in [
        None,
        Some(json!({"scored": 0, "precision": null})),
        Some(json!({"scored": 39, "precision": 0.4, "unavailable_repos": 1})),
    ] {
        let mut older = receipt.clone();
        match section {
            Some(section) => older["pilot_top_recommendations"] = section,
            None => {
                older
                    .as_object_mut()
                    .map(|map| map.remove("pilot_top_recommendations"));
            }
        }
        let rows = mutation_spot_check_to_input(&older)?;
        assert!(rows["metrics"].as_array().is_some_and(|rows| {
            rows.iter()
                .all(|row| row["id"] != "trust.pilot_top_recommendation_precision")
        }));
    }
    // A present section with an unreadable count is a malformed receipt.
    let mut malformed = receipt.clone();
    malformed["pilot_top_recommendations"]["unavailable_repos"] = json!("1");
    if mutation_spot_check_to_input(&malformed).is_ok() {
        return Err("accepted a string unavailable_repos".to_string());
    }
    for scored in [json!(null), json!("39"), json!(-1)] {
        let mut malformed = receipt.clone();
        malformed["pilot_top_recommendations"]["scored"] = scored.clone();
        if mutation_spot_check_to_input(&malformed).is_ok() {
            return Err(format!("accepted pilot scored {scored}"));
        }
    }
    Ok(())
}

#[test]
fn first_run_rows_map_to_gates_and_list_verdicts() {
    let text = [
        r#"{"schema":"first_run_row.v1","ripr":"ripr 0.11.0","case":"_setup","step":"install_published","metric":"secs","value":40.0,"budget":null,"better":"lower"}"#,
        r#"{"schema":"first_run_row.v1","ripr":"ripr 0.11.0","case":"_setup","step":"install_published","metric":"exit","value":0,"budget":0,"better":"equal"}"#,
        r#"{"schema":"first_run_row.v1","ripr":"ripr 0.11.0","case":"a","step":"doctor","metric":"secs","value":0.5,"budget":5,"better":"lower"}"#,
        r#"{"schema":"first_run_row.v1","ripr":"ripr 0.11.0","case":"a","step":"doctor","metric":"exit","value":0,"budget":0,"better":"equal"}"#,
        r#"{"schema":"first_run_row.v1","ripr":"ripr 0.11.0","case":"a","step":"check","metric":"secs","value":12.0,"budget":10,"better":"lower"}"#,
        r#"{"schema":"first_run_row.v1","ripr":"ripr 0.11.0","case":"a","step":"check","metric":"exit","value":0,"budget":0,"better":"equal"}"#,
        r#"{"schema":"first_run_row.v1","ripr":"ripr 0.11.0","case":"a","step":"check","metric":"verdict","value":"infection_unknown","budget":null,"better":"review_on_change"}"#,
        r#"{"schema":"first_run_row.v1","ripr":"ripr 0.11.0","case":"a","step":"pilot","metric":"secs","value":1.0,"budget":30,"better":"lower"}"#,
        r#"{"schema":"first_run_row.v1","ripr":"ripr 0.11.0","case":"a","step":"pilot","metric":"exit","value":2,"budget":0,"better":"equal"}"#,
        r#"{"schema":"first_run_row.v1","ripr":"ripr 0.11.0","case":"a","step":"init_ci","metric":"workflow_lines","value":2374,"budget":1500,"better":"lower"}"#,
        r#"{"schema":"first_run_row.v1","ripr":"ripr 0.11.0","case":"a","step":"init_ci","metric":"friction_count","value":2,"budget":0,"better":"lower"}"#,
        r#"{"schema":"first_run_row.v1","ripr":"ripr 0.11.0","case":"a","step":"init_ci","metric":"exit","value":0,"budget":0,"better":"equal"}"#,
    ]
    .join("\n");
    let value = parse_ingest_text(&text).unwrap_or_default();
    let converted = first_run_rows_to_input(&value).unwrap_or_default();
    let get = |id: &str| -> Vec<Value> {
        converted["metrics"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|row| row["id"].as_str() == Some(id))
            .cloned()
            .collect()
    };
    assert_eq!(get("first_run.failed_steps")[0]["value"], json!(1));
    assert_eq!(get("first_run.over_budget_steps")[0]["value"], json!(2));
    assert_eq!(get("first_run.friction_events")[0]["value"], json!(2.0));
    assert_eq!(get("first_run.unknown_verdicts")[0]["value"], json!(1));
    assert_eq!(
        get("first_run.unknown_verdicts")[0]["evidence"],
        json!("verdicts: a=infection_unknown")
    );
    assert_eq!(get("first_run.walk_secs")[0]["value"], json!(13.5));
    // install 40 + doctor 0.5 + check 12 = first useful result at 52.5 s
    let first = &get("first_run.time_to_first_useful_result_s")[0];
    assert_eq!(first["value"], json!(52.5));
    assert_eq!(first["completed"], json!(true));
    // The install alone is its own row, so a slower compile regresses it even
    // when the walk after it stays fast.
    let install = &get("first_run.install_seconds")[0];
    assert_eq!(install["value"], json!(40.0));
    assert_eq!(install["completed"], json!(true));
    assert!(
        parse_ingest_text("{\"schema\":\"other\"}\nnot json")
            .is_err_and(|err| err.contains("line 1"))
    );
}

#[test]
fn losing_completion_regresses_on_any_runner_class() -> Result<(), String> {
    let config = parse_config(MINIMAL)?;
    let base = vec![sample(
        "speed.warm_check_ms",
        Some("a"),
        SampleOutcome::Value(1000.0),
    )];
    let baseline = build_report(
        &config,
        &all_boards(),
        &base,
        &context("runner-a"),
        None,
        false,
    );
    // A check that now exits early looks faster than the baseline.
    let broken = vec![sample(
        "speed.warm_check_ms",
        Some("a"),
        SampleOutcome::Incomplete(50.0),
    )];
    for runner in ["runner-a", "runner-b"] {
        let report = build_report(
            &config,
            &all_boards(),
            &broken,
            &context(runner),
            Some(&baseline),
            true,
        );
        assert_eq!(report["gate"]["status"].as_str(), Some("fail"), "{runner}");
    }
    Ok(())
}

#[test]
fn losing_completion_is_judged_per_repo() -> Result<(), String> {
    let config = parse_config(MINIMAL)?;
    // Repo `a` was already incomplete in the baseline; `b` completed.
    let base = vec![
        sample(
            "speed.warm_check_ms",
            Some("a"),
            SampleOutcome::Incomplete(50.0),
        ),
        sample(
            "speed.warm_check_ms",
            Some("b"),
            SampleOutcome::Value(1000.0),
        ),
    ];
    let baseline = build_report(
        &config,
        &all_boards(),
        &base,
        &context("runner-a"),
        None,
        false,
    );
    let current = vec![
        sample(
            "speed.warm_check_ms",
            Some("a"),
            SampleOutcome::Incomplete(50.0),
        ),
        sample(
            "speed.warm_check_ms",
            Some("b"),
            SampleOutcome::Incomplete(40.0),
        ),
    ];
    for runner in ["runner-a", "runner-b"] {
        let report = build_report(
            &config,
            &all_boards(),
            &current,
            &context(runner),
            Some(&baseline),
            true,
        );
        assert_eq!(report["gate"]["status"].as_str(), Some("fail"), "{runner}");
        let repos = report["gate"]["regressions"][0]["regressed_repos"]
            .as_array()
            .ok_or("regressed_repos missing")?;
        assert_eq!(repos.len(), 1, "{runner}");
        assert_eq!(repos[0]["repo"], json!("b"), "{runner}");
        assert!(gate_failure_message(&report).contains("b: did not complete"));
    }
    Ok(())
}

#[test]
fn a_repo_new_to_the_corpus_is_listed_not_gated_unless_it_stops_short() -> Result<(), String> {
    let config = parse_config(MINIMAL)?;
    let base = vec![sample(
        "speed.warm_check_ms",
        Some("a"),
        SampleOutcome::Value(1000.0),
    )];
    let baseline = build_report(&config, &all_boards(), &base, &context("r"), None, false);
    let run = |new: SampleOutcome| {
        let current = vec![
            sample(
                "speed.warm_check_ms",
                Some("a"),
                SampleOutcome::Value(1000.0),
            ),
            sample("speed.warm_check_ms", Some("c"), new),
        ];
        build_report(
            &config,
            &all_boards(),
            &current,
            &context("r"),
            Some(&baseline),
            true,
        )
    };
    // A heavier new repository is slower than the old worst but regressed nothing.
    let slower = run(SampleOutcome::Value(9000.0));
    assert_eq!(slower["gate"]["status"].as_str(), Some("pass"));
    let row = slower["metrics"]
        .as_array()
        .and_then(|rows| rows.iter().find(|r| r["id"] == "speed.warm_check_ms"))
        .ok_or("warm check row missing")?;
    assert_eq!(row["baseline"]["new_repos"], json!(["c"]));
    assert_eq!(row["baseline"]["delta"], json!(0.0));
    assert!(render_markdown(&slower).contains("not in baseline: c"));
    // A new repository that stops short still fails, and says why.
    let broken = run(SampleOutcome::Incomplete(40.0));
    assert_eq!(broken["gate"]["status"].as_str(), Some("fail"));
    assert!(gate_failure_message(&broken).contains("c: new in this run and did not complete"));
    Ok(())
}

#[test]
fn gate_lists_baseline_metrics_it_could_not_compare() -> Result<(), String> {
    let config = parse_config(MINIMAL)?;
    let base = vec![
        sample("ci.workflow_lines", None, SampleOutcome::Value(100.0)),
        sample(
            "speed.warm_check_ms",
            Some("a"),
            SampleOutcome::Value(1000.0),
        ),
    ];
    let baseline = build_report(&config, &all_boards(), &base, &context("r"), None, false);
    let current = vec![sample(
        "ci.workflow_lines",
        None,
        SampleOutcome::Value(100.0),
    )];
    let report = build_report(
        &config,
        &all_boards(),
        &current,
        &context("r"),
        Some(&baseline),
        true,
    );
    assert_eq!(report["gate"]["status"].as_str(), Some("pass"));
    let uncompared = report["gate"]["uncompared"]
        .as_array()
        .ok_or("uncompared list missing")?;
    assert!(
        uncompared
            .iter()
            .any(|item| item["metric"] == "speed.warm_check_ms")
    );
    assert!(render_markdown(&report).contains("Not compared with the baseline"));
    Ok(())
}

#[test]
fn manifest_corpus_entries_must_pin_full_shas_and_unique_ids() {
    let entry = |id: &str, sha: &str| CorpusEntry {
        id: id.to_string(),
        url: "u".to_string(),
        sha: sha.to_string(),
        base_sha: None,
        note: String::new(),
        heavy: false,
    };
    let full = "9d3410e3f4e38f9ea1a798e7ae9fab71577ab31b";
    assert_eq!(validate_corpus(&[entry("a", full)]), Ok(()));
    assert!(validate_corpus(&[entry("a", "main")]).is_err_and(|e| e.contains("40-character")));
    assert!(
        validate_corpus(&[entry("a", full), entry("a", full)])
            .is_err_and(|e| e.contains("duplicate"))
    );
}

#[test]
fn first_run_rows_fail_closed_on_malformed_or_cut_off_input() {
    let row = |body: &str| format!(r#"{{"schema":"first_run_row.v1",{body}}}"#);
    let convert =
        |text: String| parse_ingest_text(&text).and_then(|value| first_run_rows_to_input(&value));
    // A bare row and a non-numeric exit reject the file instead of counting 0.
    assert!(convert(row(r#""ripr":"r""#)).is_err_and(|e| e.contains("non-empty `case`")));
    assert!(
        convert(row(
            r#""case":"a","step":"check","metric":"exit","value":"2""#
        ))
        .is_err_and(|e| e.contains("value must be a number"))
    );
    // Only setup rows: nothing was walked.
    assert!(
        convert(row(
            r#""case":"_setup","step":"fetch","metric":"exit","value":0"#
        ))
        .is_err_and(|e| e.contains("no case rows"))
    );
    // A failed install writes only setup rows; its timing still lands as an
    // incomplete install sample, and no per-case metric appears.
    let failed_install_text = [
        row(
            r#""ripr":"r","case":"_setup","step":"install_published","metric":"secs","value":20.0"#,
        ),
        row(r#""ripr":"r","case":"_setup","step":"install_published","metric":"exit","value":101"#),
    ]
    .join("\n");
    let install_rows = convert(failed_install_text.clone()).map(|input| {
        input["metrics"]
            .as_array()
            .map(|rows| {
                rows.iter()
                    .filter(|r| r["id"].as_str() == Some("first_run.install_seconds"))
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    });
    let measured_nothing = convert(failed_install_text.clone()).map(|input| {
        input["metrics"].as_array().is_some_and(|rows| {
            rows.iter().all(|r| {
                !matches!(
                    r["id"].as_str(),
                    Some("first_run.friction_events" | "first_run.unknown_verdicts")
                )
            })
        })
    });
    assert_eq!(measured_nothing, Ok(true));
    assert!(install_rows.is_ok_and(|rows| rows.len() == 1
        && rows[0]["value"] == json!(20.0)
        && rows[0]["completed"] == json!(false)));
    // A step cut off before its exit row counts as failed.
    let cut = [
        row(r#""case":"a","step":"check","metric":"exit","value":0"#),
        row(r#""case":"a","step":"pilot","metric":"secs","value":3.0"#),
    ]
    .join("\n");
    let failed = convert(cut).map(|input| {
        input["metrics"]
            .as_array()
            .and_then(|rows| {
                rows.iter()
                    .find(|r| r["id"] == "first_run.failed_steps")
                    .cloned()
            })
            .unwrap_or_default()
    });
    assert!(failed.is_ok_and(
        |row| row["value"] == json!(1) && row["evidence"] == json!("a/pilot no exit recorded")
    ));
}

#[test]
fn first_useful_result_stops_at_the_first_check_that_exits_zero() -> Result<(), String> {
    let row = |body: &str| format!(r#"{{"schema":"first_run_row.v1",{body}}}"#);
    // Summary rows first, as the walk's export writes them.
    let text = [
        row(r#""case":"a","step":"check","metric":"verdict","value":"infection_unknown""#),
        row(r#""case":"a","step":"init_ci","metric":"workflow_lines","value":1154,"budget":1500"#),
        row(r#""case":"_setup","step":"install","metric":"secs","value":10.0"#),
        row(r#""case":"_setup","step":"install","metric":"exit","value":0"#),
        row(r#""case":"a","step":"check","metric":"secs","value":1.0"#),
        row(r#""case":"a","step":"check","metric":"exit","value":1"#),
        row(r#""case":"a","step":"fix","metric":"secs","value":2.0"#),
        row(r#""case":"a","step":"fix","metric":"exit","value":0"#),
        row(r#""case":"a","step":"check","metric":"secs","value":4.0"#),
        row(r#""case":"a","step":"check","metric":"exit","value":0"#),
    ]
    .join("\n");
    let input = first_run_rows_to_input(&parse_ingest_text(&text)?)?;
    let first = input["metrics"]
        .as_array()
        .and_then(|rows| {
            rows.iter()
                .find(|r| r["id"] == "first_run.time_to_first_useful_result_s")
        })
        .ok_or("time to first useful result missing")?;
    assert_eq!(first["value"], json!(17.0));
    assert_eq!(first["completed"], json!(true));
    Ok(())
}

#[test]
fn repeated_step_rows_pair_in_order_when_exits_come_last() -> Result<(), String> {
    let row = |body: &str| format!(r#"{{"schema":"first_run_row.v1",{body}}}"#);
    // Both timings of a repeated `check` arrive before both exits.
    let text = [
        row(r#""case":"_setup","step":"install","metric":"secs","value":10.0"#),
        row(r#""case":"_setup","step":"install","metric":"exit","value":0"#),
        row(r#""case":"a","step":"check","metric":"secs","value":1.0"#),
        row(r#""case":"a","step":"check","metric":"secs","value":4.0"#),
        row(r#""case":"a","step":"check","metric":"exit","value":1"#),
        row(r#""case":"a","step":"check","metric":"exit","value":0"#),
    ]
    .join("\n");
    let input = first_run_rows_to_input(&parse_ingest_text(&text)?)?;
    let get = |id: &str| {
        input["metrics"]
            .as_array()
            .and_then(|rows| rows.iter().find(|r| r["id"] == id).cloned())
            .unwrap_or_default()
    };
    // Only the first run exited nonzero; neither run is left without an exit.
    assert_eq!(get("first_run.failed_steps")["value"], json!(1));
    assert_eq!(
        get("first_run.failed_steps")["evidence"],
        json!("a/check exit 1")
    );
    assert_eq!(
        get("first_run.time_to_first_useful_result_s")["value"],
        json!(15.0)
    );
    Ok(())
}

#[test]
fn gate_compares_every_repository_not_just_the_worst() -> Result<(), String> {
    let config = parse_config(MINIMAL)?;
    let samples = |a: f64, b: f64| {
        vec![
            sample("speed.warm_check_ms", Some("a"), SampleOutcome::Value(a)),
            sample("speed.warm_check_ms", Some("b"), SampleOutcome::Value(b)),
        ]
    };
    let baseline = build_report(
        &config,
        &all_boards(),
        &samples(4000.0, 100.0),
        &context("r"),
        None,
        false,
    );
    // The worst repository is unchanged; `b` grew 30x.
    let report = build_report(
        &config,
        &all_boards(),
        &samples(4000.0, 3000.0),
        &context("r"),
        Some(&baseline),
        true,
    );
    assert_eq!(report["gate"]["status"].as_str(), Some("fail"));
    let repos = &metric(&report, "speed.warm_check_ms")?["baseline"]["regressed_repos"];
    assert_eq!(repos[0]["repo"], json!("b"));
    assert!(gate_failure_message(&report).contains("b: baseline 100"));
    Ok(())
}

#[test]
fn additive_margin_allows_pct_plus_floor() -> Result<(), String> {
    let mut config = parse_config(MINIMAL)?;
    let def = config
        .metric
        .iter_mut()
        .find(|m| m.id == "speed.warm_check_ms")
        .ok_or("metric missing")?;
    def.regression_pct = 50.0;
    def.regression_floor = 0.5;
    def.regression_additive = true;
    // 1.5x plus 0.5 of a 2.0 baseline is 3.5.
    assert!(allowed_worsening(def, 2.0) > 1.49 && allowed_worsening(def, 2.0) < 1.51);
    def.regression_additive = false;
    assert!(allowed_worsening(def, 2.0) > 0.99 && allowed_worsening(def, 2.0) < 1.01);
    Ok(())
}

#[test]
fn baseline_must_be_a_scoreboard_report() {
    assert!(check_baseline(json!({"metrics": []})).is_err_and(|e| e.contains("schema_version")));
    assert!(
        check_baseline(json!({"schema_version": "ripr-dx-scoreboard-v1"}))
            .is_err_and(|e| e.contains("metrics"))
    );
}

#[test]
fn malformed_mutation_and_generic_receipts_are_rejected() {
    assert!(
        mutation_spot_check_to_input(&json!({"repos": []}))
            .is_err_and(|e| e.contains("scored_families"))
    );
    let receipt = json!({
        "scored_families": {
            "claims_discriminator": {"agreement_rate": 1.0, "mutants_scored": 1},
            "claims_no_discriminator": {"agreement_rate": 0.5, "mutants_scored": 1},
        },
        "repos": [{"pairings": {"seam_precise": 5}}],
    });
    assert!(mutation_spot_check_to_input(&receipt).is_err_and(|e| e.contains("mutants_total")));
    assert!(
        first_run_to_input(&json!({"schema_version": "first_run.v1", "cases": []}))
            .is_err_and(|e| e.contains("non-empty"))
    );
}

#[test]
fn hostile_repo_counts_come_from_the_libtest_summary() {
    let ok = "running 15 tests\n...\ntest result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.1s\n";
    assert_eq!(parse_test_result(ok), Some((15, 0)));
    let bad = "test result: FAILED. 13 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out\n";
    assert_eq!(parse_test_result(bad), Some((13, 2)));
    // A build failure prints no summary: not "zero failures".
    assert_eq!(parse_test_result("error[E0061]: oops\n"), None);
}

#[test]
fn a_step_without_a_duration_leaves_the_walk_incomplete() -> Result<(), String> {
    let row = |body: &str| format!(r#"{{"schema":"first_run_row.v1",{body}}}"#);
    let text = [
        row(r#""case":"_setup","step":"install","metric":"secs","value":10.0"#),
        row(r#""case":"_setup","step":"install","metric":"exit","value":0"#),
        row(r#""case":"a","step":"doctor","metric":"secs","value":1.0"#),
        row(r#""case":"a","step":"doctor","metric":"exit","value":0"#),
        // `check` exited 0 but its secs row was lost.
        row(r#""case":"a","step":"check","metric":"exit","value":0"#),
    ]
    .join("\n");
    let input = first_run_rows_to_input(&parse_ingest_text(&text)?)?;
    let get = |id: &str| {
        input["metrics"]
            .as_array()
            .and_then(|rows| rows.iter().find(|r| r["id"] == id).cloned())
            .unwrap_or_default()
    };
    assert_eq!(get("first_run.walk_secs")["completed"], json!(false));
    assert_eq!(
        get("first_run.walk_secs")["evidence"],
        json!("no secs row for: check")
    );
    assert_eq!(
        get("first_run.time_to_first_useful_result_s")["completed"],
        json!(false)
    );
    Ok(())
}

#[test]
fn a_baseline_repo_missing_from_the_run_is_listed_not_passed() -> Result<(), String> {
    let config = parse_config(MINIMAL)?;
    let base = vec![
        sample(
            "speed.warm_check_ms",
            Some("a"),
            SampleOutcome::Value(1000.0),
        ),
        sample(
            "speed.warm_check_ms",
            Some("b"),
            SampleOutcome::Value(1000.0),
        ),
    ];
    let baseline = build_report(&config, &all_boards(), &base, &context("r"), None, false);
    let current = vec![sample(
        "speed.warm_check_ms",
        Some("a"),
        SampleOutcome::Value(1000.0),
    )];
    let report = build_report(
        &config,
        &all_boards(),
        &current,
        &context("r"),
        Some(&baseline),
        true,
    );
    let uncompared = report["gate"]["uncompared"]
        .as_array()
        .ok_or("uncompared list missing")?;
    assert!(uncompared.iter().any(|item| {
        item["metric"] == "speed.warm_check_ms"
            && item["reason"]
                .as_str()
                .is_some_and(|reason| reason.ends_with("not measured this run: b"))
    }));
    Ok(())
}

fn measured(timed_out: bool, peak_rss_bytes: Option<u64>) -> MeasuredOutput {
    MeasuredOutput {
        output: TimedOutput {
            status: None,
            stdout: String::new(),
            stderr: String::new(),
            duration: std::time::Duration::from_millis(5),
            timed_out,
        },
        peak_rss_bytes,
    }
}

#[test]
fn the_peak_of_an_incomplete_run_is_not_a_value() {
    let sample = |metric: &str, outcome: SampleOutcome, detail: String| Sample {
        metric: metric.to_string(),
        repo: Some("r".to_string()),
        outcome,
        detail,
    };
    let run = measured(false, Some(64 * 1024 * 1024));
    let done = rss_sample(&sample, "speed.warm_check_peak_rss_mb", &run, true);
    assert!(matches!(done.outcome, SampleOutcome::Value(mb) if (mb - 64.0).abs() < 1e-9));
    let cut = rss_sample(&sample, "speed.warm_check_peak_rss_mb", &run, false);
    assert!(matches!(cut.outcome, SampleOutcome::Incomplete(mb) if (mb - 64.0).abs() < 1e-9));
    assert!(cut.detail.contains("did not complete"));
}

#[test]
fn contradictions_from_one_source_are_incomplete() {
    let (outcome, detail) = contradiction_outcome(Some((0, Vec::new())), &["warm check JSON"]);
    assert!(matches!(outcome, SampleOutcome::Incomplete(n) if n.abs() < 1e-9));
    assert_eq!(detail, "not scanned: warm check JSON");
    let (outcome, _) = contradiction_outcome(Some((2, vec!["x".to_string()])), &[]);
    assert!(matches!(outcome, SampleOutcome::Value(n) if (n - 2.0).abs() < 1e-9));
    let (outcome, _) = contradiction_outcome(None, &["pilot repo-exposure.json"]);
    assert!(matches!(outcome, SampleOutcome::NotMeasured));
}

#[test]
fn a_hung_bad_input_probe_is_not_a_refusal() {
    assert_eq!(probe_result(&measured(true, None)), Probe::TimedOut);
    // No exit status and no timeout: ended by a signal, which is a refusal.
    assert_eq!(probe_result(&measured(false, None)), Probe::Refused);
}

#[cfg(unix)]
#[test]
fn a_symlinked_target_is_never_cleared() -> Result<(), String> {
    let root = std::env::temp_dir().join(format!("dx-linked-target-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let checkout = root.join("checkout");
    let outside = root.join("outside");
    std::fs::create_dir_all(&checkout).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&outside).map_err(|e| e.to_string())?;
    assert_eq!(linked_target(&checkout), None);
    std::fs::create_dir_all(checkout.join("target")).map_err(|e| e.to_string())?;
    assert_eq!(linked_target(&checkout), None);
    std::os::unix::fs::symlink(&outside, checkout.join("target/ripr"))
        .map_err(|e| e.to_string())?;
    let reason = linked_target(&checkout);
    // A symlinked `target` itself is refused too, not only `target/ripr`.
    let other = root.join("other");
    std::fs::create_dir_all(&other).map_err(|e| e.to_string())?;
    std::os::unix::fs::symlink(&outside, other.join("target")).map_err(|e| e.to_string())?;
    let linked_parent = linked_target(&other);
    let _ = std::fs::remove_dir_all(&root);
    assert!(linked_parent.is_some());
    assert!(
        reason.is_some_and(|r| r.contains("refusing to clear") && r.contains("remove the link"))
    );
    Ok(())
}

fn smoke_receipt(rows: &[(&str, &str, u64)]) -> Value {
    json!({
        "schema_version": "ripr-rust-corpus-smoke-v1",
        "corpus_version": "2026-10-04.5",
        "tier": "fast",
        "repos": rows
            .iter()
            .map(|(id, status, ms)| {
                let mut row = json!({"id": id, "status": status, "duration_ms": ms});
                if *status == "not_fetched" {
                    row["reason"] = json!("missing checkout");
                } else {
                    row["findings"] = json!(2);
                }
                row
            })
            .collect::<Vec<_>>(),
    })
}

fn corpus_gate(base: &Value, current: &Value) -> Result<Value, String> {
    let config = load_config(&committed_config())?;
    let boards = vec!["corpus".to_string()];
    let base = parse_ingest(base, &config)?;
    let baseline = build_report(&config, &boards, &base, &context("r"), None, false);
    let current = parse_ingest(current, &config)?;
    Ok(build_report(
        &config,
        &boards,
        &current,
        &context("r"),
        Some(&baseline),
        true,
    ))
}

#[test]
fn a_corpus_repo_that_stops_analyzing_fails_the_gate() -> Result<(), String> {
    // `c` failed closed in the baseline too, so it is not a new failure.
    let base = smoke_receipt(&[
        ("a", "analyzed", 900),
        ("b", "analyzed", 1200),
        ("c", "diff_scope_oversized", 40),
    ]);
    let same = corpus_gate(&base, &base)?;
    assert_eq!(same["gate"]["status"].as_str(), Some("pass"));

    let current = smoke_receipt(&[
        ("a", "analyzed", 900),
        ("b", "timed_out", 600_000),
        ("c", "diff_scope_oversized", 40),
    ]);
    let report = corpus_gate(&base, &current)?;
    assert_eq!(report["gate"]["status"].as_str(), Some("fail"));
    let regressions = report["gate"]["regressions"]
        .as_array()
        .ok_or("regressions missing")?;
    assert!(
        regressions
            .iter()
            .any(|r| r["metric"] == "corpus.not_analyzed"),
        "{regressions:?}"
    );
    let text = regressions.iter().map(Value::to_string).collect::<String>();
    assert!(text.contains("\"b\""), "{text}");
    assert!(!text.contains("\"c\""), "{text}");
    Ok(())
}

#[test]
fn a_negative_ingested_value_is_refused() -> Result<(), String> {
    let config = load_config(&committed_config())?;
    let input = json!({
        "schema_version": INPUT_SCHEMA_VERSION,
        "source": "install",
        "metrics": [{"id": "ci.install_seconds", "value": -0.5}],
    });
    assert!(parse_ingest(&input, &config).is_err_and(|e| e.contains("negative")));
    let fine = json!({
        "schema_version": INPUT_SCHEMA_VERSION,
        "source": "install",
        "metrics": [{"id": "ci.install_seconds", "value": 0.97}],
    });
    assert_eq!(parse_ingest(&fine, &config)?.len(), 1);
    Ok(())
}

#[test]
fn an_unknown_ingested_metric_is_refused_with_where_to_declare_it() -> Result<(), String> {
    let config = load_config(&committed_config())?;
    let input = json!({
        "schema_version": INPUT_SCHEMA_VERSION,
        "source": "agent-as-user",
        "metrics": [{"id": "agent.not_declared", "value": 1}],
    });
    assert!(parse_ingest(&input, &config).is_err_and(|e| e.contains("scoreboards.toml")));
    // The agent-as-user stub-route counts are declared.
    let stub_route = json!({
        "schema_version": INPUT_SCHEMA_VERSION,
        "source": "agent-as-user",
        "metrics": [
            {"id": "agent.stub_calls", "value": 5},
            {"id": "agent.stub_calls_producing_stub", "value": 0},
        ],
    });
    assert_eq!(parse_ingest(&stub_route, &config)?.len(), 2);
    Ok(())
}

#[test]
fn a_review_only_count_that_moves_with_unchanged_evidence_is_listed() -> Result<(), String> {
    let config = load_config(&committed_config())?;
    let boards = vec!["agent".to_string()];
    let receipt = |calls: u64| {
        json!({
            "schema_version": INPUT_SCHEMA_VERSION,
            "source": "agent-as-user",
            "metrics": [{"id": "agent.stub_calls", "value": calls}],
        })
    };
    let base = parse_ingest(&receipt(5), &config)?;
    let baseline = build_report(&config, &boards, &base, &context("r"), None, false);
    let current = parse_ingest(&receipt(8), &config)?;
    let report = build_report(
        &config,
        &boards,
        &current,
        &context("r"),
        Some(&baseline),
        true,
    );
    assert_eq!(
        report["gate"]["status"].as_str(),
        Some("pass"),
        "{}",
        report["gate"]
    );
    let review = report["gate"]["review"].to_string();
    assert!(review.contains("agent.stub_calls"), "{review}");
    let rendered = render_markdown(&report);
    assert!(
        rendered.contains("`agent.stub_calls` changed: baseline [5.0] → current [8.0]"),
        "{rendered}"
    );
    Ok(())
}

#[test]
fn a_status_flip_is_listed_next_to_a_lost_completion() -> Result<(), String> {
    let base = smoke_receipt(&[("a", "analyzed", 900), ("b", "analyzed", 1200)]);
    let current = smoke_receipt(&[("a", "timed_out", 600_000), ("b", "not_fetched", 0)]);
    let report = corpus_gate(&base, &current)?;
    let regression = report["gate"]["regressions"]
        .as_array()
        .and_then(|all| all.iter().find(|r| r["metric"] == "corpus.not_analyzed"))
        .ok_or("corpus.not_analyzed regression missing")?;
    let repos: Vec<&str> = regression["regressed_repos"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| r["repo"].as_str())
        .collect();
    assert_eq!(repos, ["b", "a"], "{regression}");
    Ok(())
}

#[test]
fn a_corpus_repo_that_was_not_fetched_is_lost_completion_not_a_pass() -> Result<(), String> {
    let base = smoke_receipt(&[("a", "analyzed", 900), ("b", "analyzed", 1200)]);
    let current = smoke_receipt(&[("a", "analyzed", 900), ("b", "not_fetched", 0)]);
    let input = rust_corpus_smoke_to_input(&current)?;
    let rows = input["metrics"].as_array().ok_or("metrics missing")?;
    assert!(rows.iter().filter(|row| row["repo"] == "b").all(|row| {
        row["completed"] == false
            && row["evidence"]
                .as_str()
                .is_some_and(|e| e.contains("missing checkout"))
    }));
    let report = corpus_gate(&base, &current)?;
    assert_eq!(report["gate"]["status"].as_str(), Some("fail"));
    Ok(())
}

#[test]
fn a_smoke_receipt_without_repos_is_refused() {
    let empty = json!({"schema_version": "ripr-rust-corpus-smoke-v1", "repos": []});
    assert!(rust_corpus_smoke_to_input(&empty).is_err_and(|e| e.contains("repos")));
    let unnamed =
        json!({"schema_version": "ripr-rust-corpus-smoke-v1", "repos": [{"status": "analyzed"}]});
    assert!(rust_corpus_smoke_to_input(&unnamed).is_err_and(|e| e.contains("needs an id")));
}

#[test]
fn libtest_summary_counts_are_read_by_label_not_position() {
    let line = "test result: FAILED. 123 passed; 45 failed; 6 ignored; 7 measured; 890 filtered out; finished in 1.0s\n";
    assert_eq!(parse_test_result(line), Some((123, 45)));
}

#[cfg(unix)]
#[test]
fn a_probe_ended_by_a_signal_is_a_refusal_not_a_clean_exit() {
    use std::os::unix::process::ExitStatusExt;
    let mut run = measured(false, None);
    run.output.status = Some(std::process::ExitStatus::from_raw(9));
    assert_eq!(probe_result(&run), Probe::Refused);
    run.output.status = Some(std::process::ExitStatus::from_raw(0));
    assert_eq!(probe_result(&run), Probe::ExitedZero);
}

#[test]
fn a_negative_first_run_duration_is_refused() {
    let row = |body: &str| format!(r#"{{"schema":"first_run_row.v1",{body}}}"#);
    let text = [
        row(r#""case":"a","step":"check","metric":"secs","value":-100.0"#),
        row(r#""case":"a","step":"check","metric":"exit","value":0"#),
    ]
    .join("\n");
    let parsed = parse_ingest_text(&text);
    let converted = parsed.and_then(|value| first_run_rows_to_input(&value));
    assert!(converted.is_err_and(|e| e.contains("negative")));
}

#[test]
fn a_hostile_run_with_no_tests_is_not_zero_failures() {
    let none = "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 15 filtered out\n";
    let (outcome, detail) = hostile_outcome(parse_test_result(none), "");
    assert!(matches!(outcome, SampleOutcome::Failed), "{detail}");
    let (outcome, _) = hostile_outcome(Some((13, 2)), "");
    assert!(matches!(outcome, SampleOutcome::Value(n) if (n - 2.0).abs() < 1e-9));
}

#[test]
fn the_gate_message_reports_the_compared_value_not_a_new_repos_worst() -> Result<(), String> {
    let config = parse_config(MINIMAL)?;
    let base = vec![sample(
        "speed.warm_check_ms",
        Some("a"),
        SampleOutcome::Value(1000.0),
    )];
    let baseline = build_report(&config, &all_boards(), &base, &context("r"), None, false);
    let current = vec![
        sample(
            "speed.warm_check_ms",
            Some("a"),
            SampleOutcome::Value(3000.0),
        ),
        sample(
            "speed.warm_check_ms",
            Some("new"),
            SampleOutcome::Value(9000.0),
        ),
    ];
    let report = build_report(
        &config,
        &all_boards(),
        &current,
        &context("r"),
        Some(&baseline),
        true,
    );
    let regression = report["gate"]["regressions"]
        .as_array()
        .and_then(|all| all.first())
        .ok_or("regression missing")?;
    assert_eq!(regression["current"].as_f64(), Some(3000.0), "{regression}");
    Ok(())
}

#[test]
fn a_slower_fail_closed_repo_is_not_a_time_regression() -> Result<(), String> {
    let base = smoke_receipt(&[("a", "analyzed", 900), ("c", "diff_scope_oversized", 40)]);
    let current = smoke_receipt(&[("a", "analyzed", 900), ("c", "diff_scope_oversized", 9000)]);
    let report = corpus_gate(&base, &current)?;
    assert_eq!(
        report["gate"]["status"].as_str(),
        Some("pass"),
        "{}",
        report["gate"]
    );
    Ok(())
}

#[test]
fn a_repo_that_starts_completing_again_is_not_a_time_regression() -> Result<(), String> {
    // The baseline's fail-closed sample records 0 ms; recovery must not be
    // compared against that placeholder.
    let base = smoke_receipt(&[("a", "analyzed", 900), ("c", "diff_scope_oversized", 40)]);
    // c recovers slower than a, so it would also be the compared worst.
    let current = smoke_receipt(&[("a", "analyzed", 900), ("c", "analyzed", 4000)]);
    let report = corpus_gate(&base, &current)?;
    assert_eq!(
        report["gate"]["status"].as_str(),
        Some("pass"),
        "{}",
        report["gate"]
    );
    let row = report["metrics"]
        .as_array()
        .and_then(|rows| rows.iter().find(|r| r["id"] == "corpus.check_ms"))
        .ok_or("corpus.check_ms row missing")?;
    assert_eq!(
        row["baseline"]["recovered_repos"],
        json!(["c"]),
        "{}",
        row["baseline"]
    );
    // A repository the baseline never sampled is new, not recovered.
    let grown = smoke_receipt(&[
        ("a", "analyzed", 900),
        ("c", "analyzed", 800),
        ("d", "analyzed", 700),
    ]);
    let report = corpus_gate(&base, &grown)?;
    let row = report["metrics"]
        .as_array()
        .and_then(|rows| rows.iter().find(|r| r["id"] == "corpus.check_ms"))
        .ok_or("corpus.check_ms row missing")?;
    assert_eq!(row["baseline"]["recovered_repos"], json!(["c"]));
    assert_eq!(row["baseline"]["new_repos"], json!(["d"]));
    let rendered = render_markdown(&report);
    assert!(
        rendered.contains("not in baseline: d; completed again: c"),
        "{rendered}"
    );
    // A repository that completed in both runs still regresses on time.
    let slower = smoke_receipt(&[("a", "analyzed", 9000), ("c", "analyzed", 4000)]);
    let report = corpus_gate(&base, &slower)?;
    assert_eq!(
        report["gate"]["status"].as_str(),
        Some("fail"),
        "{}",
        report["gate"]
    );
    Ok(())
}

#[test]
fn a_recovery_is_listed_next_to_a_lost_completion_and_when_nothing_compares() -> Result<(), String>
{
    let row_of = |report: &Value| -> Result<Value, String> {
        report["metrics"]
            .as_array()
            .and_then(|rows| rows.iter().find(|r| r["id"] == "corpus.check_ms"))
            .cloned()
            .ok_or_else(|| "corpus.check_ms row missing".to_string())
    };
    // a stops completing while c starts again: the gate fails for a and still
    // lists c.
    let base = smoke_receipt(&[("a", "analyzed", 900), ("c", "diff_scope_oversized", 40)]);
    let swapped = smoke_receipt(&[("a", "diff_scope_oversized", 40), ("c", "analyzed", 900)]);
    let report = corpus_gate(&base, &swapped)?;
    assert_eq!(report["gate"]["status"].as_str(), Some("fail"));
    assert_eq!(
        row_of(&report)?["baseline"]["recovered_repos"],
        json!(["c"])
    );
    // Only c, which recovered, completed: nothing compares, and the Markdown
    // cell still names the recovery.
    let only_c = smoke_receipt(&[("c", "diff_scope_oversized", 40)]);
    let report = corpus_gate(&only_c, &smoke_receipt(&[("c", "analyzed", 900)]))?;
    let row = row_of(&report)?;
    assert_eq!(
        row["baseline"]["comparable"],
        json!(false),
        "{}",
        row["baseline"]
    );
    let rendered = render_markdown(&report);
    assert!(rendered.contains("completed again: c"), "{rendered}");
    Ok(())
}

#[test]
fn an_analyzed_smoke_row_without_a_duration_is_refused() -> Result<(), String> {
    let receipt = json!({
        "schema_version": "ripr-rust-corpus-smoke-v1",
        "repos": [{"id": "a", "status": "analyzed", "findings": 1}],
    });
    assert!(rust_corpus_smoke_to_input(&receipt).is_err_and(|e| e.contains("duration_ms")));
    let negative = json!({
        "schema_version": "ripr-rust-corpus-smoke-v1",
        "repos": [{"id": "a", "status": "analyzed", "duration_ms": -5, "findings": 1}],
    });
    assert!(rust_corpus_smoke_to_input(&negative).is_err_and(|e| e.contains("duration_ms")));
    // A fail-closed row has no time to compare, so a missing one is fine.
    let closed = json!({
        "schema_version": "ripr-rust-corpus-smoke-v1",
        "repos": [{"id": "c", "status": "diff_scope_oversized"}],
    });
    rust_corpus_smoke_to_input(&closed)?;
    Ok(())
}

#[test]
fn pilot_ranking_receipt_maps_pooled_cuts_and_marks_a_lost_crate_incomplete() -> Result<(), String>
{
    let cut = |picks: u64, confirmed: u64, refuted: u64, distinct: u64| {
        json!({
            "picks": picks,
            "confirmed": confirmed,
            "refuted": refuted,
            "unscored": picks - confirmed - refuted,
            "precision": confirmed as f64 / (confirmed + refuted) as f64,
            "scored_share": (confirmed + refuted) as f64 / picks as f64,
            "distinct_functions": distinct,
            "distinct_function_share": distinct as f64 / picks as f64,
        })
    };
    let receipt = json!({
        "schema_version": "ripr-pilot-ranking-v1",
        "corpus_version": "2026-10-04.1",
        "status": "complete",
        "repos_total": 5,
        "unavailable_repos": 0,
        "pooled": {"top5": cut(25, 8, 12, 20), "top10": cut(50, 13, 22, 40)},
    });
    let config = load_config(&committed_config())?;
    let samples = parse_ingest(&receipt, &config)?;
    let value = |id: &str| {
        samples
            .iter()
            .find(|sample| sample.metric == id)
            .map(|sample| sample.outcome.clone())
    };
    assert_eq!(
        value("ranking.pilot_precision_top5"),
        Some(SampleOutcome::Value(0.4))
    );
    assert_eq!(
        value("ranking.pilot_precision_top10"),
        Some(SampleOutcome::Value(13.0 / 35.0))
    );
    assert_eq!(
        value("ranking.pilot_scored_share_top10"),
        Some(SampleOutcome::Value(0.7))
    );
    assert_eq!(
        value("ranking.pilot_distinct_function_share_top10"),
        Some(SampleOutcome::Value(0.8))
    );
    assert_eq!(
        value("ranking.pilot_picks_top10"),
        Some(SampleOutcome::Value(50.0))
    );
    assert_eq!(
        value("ranking.pilot_refuted_top10"),
        Some(SampleOutcome::Value(22.0))
    );
    assert_eq!(
        value("ranking.pilot_confirmed_top5"),
        Some(SampleOutcome::Value(8.0))
    );
    assert_eq!(
        value("ranking.pilot_refuted_top5"),
        Some(SampleOutcome::Value(12.0))
    );
    assert_eq!(
        value("ranking.pilot_confirmed_top10"),
        Some(SampleOutcome::Value(13.0))
    );
    assert!(
        samples
            .iter()
            .all(|sample| sample.repo.as_deref() == Some("pilot-ranking corpus 2026-10-04.1"))
    );
    assert!(
        samples
            .iter()
            .all(|sample| sample.detail.contains("over 5 of 5 repositories"))
    );
    assert!(samples.iter().any(|sample| sample.metric
        == "ranking.pilot_distinct_function_share_top10"
        && sample.detail.contains("40 distinct functions in 50 picks")));

    // Each precision row shows its own cut's tier split, and a split that
    // does not account for exactly the cut's judged picks is refused.
    let mut tiered = receipt.clone();
    tiered["pooled"]["top5"]["by_tier"] = json!({
        "seam": {"confirmed": 1, "refuted": 1},
        "line": {"confirmed": 5, "refuted": 4},
        "owner": {"confirmed": 2, "refuted": 7},
    });
    let samples = parse_ingest(&tiered, &config)?;
    assert!(samples.iter().any(|sample| {
        sample.metric == "ranking.pilot_precision_top5"
            && sample
                .detail
                .contains("(by tier: seam 1/2, line 5/9, owner 2/9)")
    }));
    tiered["pooled"]["top5"]["by_tier"]["owner"]["refuted"] = json!(8);
    assert!(parse_ingest(&tiered, &config).is_err_and(|err| err.contains("judges 21 picks")));

    // A crate that could not be fetched or scored changes the population, so
    // every row is incomplete instead of a rate the gate compares as like for
    // like.
    let mut lost = receipt.clone();
    lost["status"] = json!("incomplete");
    lost["unavailable_repos"] = json!(1);
    let samples = parse_ingest(&lost, &config)?;
    assert_eq!(samples.len(), 9);
    assert!(
        samples
            .iter()
            .all(|sample| matches!(sample.outcome, SampleOutcome::Incomplete(_))),
    );

    // Nothing scored is no precision, not a perfect or zero one.
    let mut unscored = receipt.clone();
    unscored["pooled"]["top5"] = json!({
        "picks": 25, "confirmed": 0, "refuted": 0, "unscored": 25,
        "precision": null, "scored_share": 0.0,
        "distinct_functions": 20, "distinct_function_share": 0.8,
    });
    let samples = parse_ingest(&unscored, &config)?;
    assert!(
        samples
            .iter()
            .any(|sample| sample.metric == "ranking.pilot_precision_top5"
                && matches!(sample.outcome, SampleOutcome::Incomplete(_)))
    );

    let mut malformed = receipt.clone();
    malformed["pooled"]["top10"]["precision"] = json!(1.5);
    assert!(parse_ingest(&malformed, &config).is_err_and(|err| err.contains("its counts give")));
    // In range but not what the counts say: 13 confirmed of 35 is not 1.0.
    let mut inconsistent = receipt.clone();
    inconsistent["pooled"]["top10"]["precision"] = json!(1.0);
    assert!(parse_ingest(&inconsistent, &config).is_err_and(|err| err.contains("its counts give")));
    let mut hidden = receipt.clone();
    hidden["pooled"]["top10"]["precision"] = Value::Null;
    assert!(parse_ingest(&hidden, &config).is_err_and(|err| err.contains("its counts give")));
    let mut overcounted = receipt;
    overcounted["pooled"]["top10"]["confirmed"] = json!(40);
    assert!(parse_ingest(&overcounted, &config).is_err_and(|err| err.contains("more confirmed")));
    Ok(())
}

/// The committed floors must fail a single pick moving the wrong way, even
/// in the tightest case where every top-10 pick is judged (one flip moves
/// precision by exactly 0.02), and any lost pick.
#[test]
fn ranking_gate_fails_one_flipped_pick_and_one_lost_pick() -> Result<(), String> {
    let receipt_on = |corpus: &str, picks: u64, confirmed: u64, scored: u64| {
        let cut = json!({
            "picks": picks,
            "confirmed": confirmed,
            "refuted": scored - confirmed,
            "unscored": picks - scored,
            "precision": confirmed as f64 / scored as f64,
            "scored_share": scored as f64 / picks as f64,
            "distinct_functions": picks,
            "distinct_function_share": 1.0,
        });
        json!({
            "schema_version": "ripr-pilot-ranking-v1",
            "corpus_version": corpus,
            "status": "complete",
            "repos_total": 5,
            "unavailable_repos": 0,
            "pooled": {"top5": cut.clone(), "top10": cut},
        })
    };
    let receipt =
        |picks: u64, confirmed: u64, scored: u64| receipt_on("test", picks, confirmed, scored);
    let config = load_config(&committed_config())?;
    let boards = vec!["ranking".to_string()];
    let report = |value: &Value, baseline: Option<&Value>| -> Result<Value, String> {
        let samples = parse_ingest(value, &config)?;
        Ok(build_report(
            &config,
            &boards,
            &samples,
            &context("r"),
            baseline,
            true,
        ))
    };
    let regressed = |report: &Value| {
        report["gate"]["regressions"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|row| row["metric"].as_str().map(str::to_string))
            .collect::<Vec<_>>()
    };
    // 3 -> 2 confirmed of 50 judged: the case a 0.02 floor let through.
    let baseline = report(&receipt(50, 3, 50), None)?;
    assert!(regressed(&report(&receipt(50, 3, 50), Some(&baseline))?).is_empty());
    let flipped = regressed(&report(&receipt(50, 2, 50), Some(&baseline))?);
    assert!(
        flipped.contains(&"ranking.pilot_precision_top10".to_string()),
        "{flipped:?}"
    );
    let lost = regressed(&report(&receipt(49, 3, 49), Some(&baseline))?);
    assert!(
        lost.contains(&"ranking.pilot_picks_top10".to_string()),
        "{lost:?}"
    );

    // An unscored pick turning refuted moves precision by less than one
    // pick's step (13/35 to 13/36); the refuted count still fails it.
    let base = report(&receipt(50, 13, 35), None)?;
    let worse = regressed(&report(&receipt(50, 13, 36), Some(&base))?);
    assert!(
        !worse.contains(&"ranking.pilot_precision_top10".to_string())
            && worse.contains(&"ranking.pilot_refuted_top10".to_string()),
        "{worse:?}"
    );

    // A baseline from another answer key is reported uncompared, never
    // compared as the same population.
    // The run is worse on every axis, so any compared row would regress.
    let other = report(&receipt_on("other", 50, 3, 50), None)?;
    let crossed = report(&receipt(40, 1, 40), Some(&other))?;
    assert!(regressed(&crossed).is_empty(), "{}", crossed["gate"]);
    let uncompared: std::collections::BTreeSet<&str> = crossed["gate"]["uncompared"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|row| row["metric"].as_str())
        .filter(|metric| metric.starts_with("ranking."))
        .collect();
    assert_eq!(uncompared.len(), 9, "{}", crossed["gate"]);
    Ok(())
}

#[test]
fn the_step_summary_keeps_earlier_steps_and_gains_the_scoreboard() -> Result<(), String> {
    let root = std::env::temp_dir().join(format!("dx-step-summary-{}", std::process::id()));
    fs::create_dir_all(&root).map_err(|err| err.to_string())?;
    let summary = root.join("summary.md");
    fs::write(&summary, "earlier step\n").map_err(|err| err.to_string())?;
    append_step_summary(&summary, "# DX scoreboard\n")?;
    let text = fs::read_to_string(&summary).map_err(|err| err.to_string())?;
    let _ = fs::remove_dir_all(&root);
    assert_eq!(text, "earlier step\n# DX scoreboard\n");
    Ok(())
}
