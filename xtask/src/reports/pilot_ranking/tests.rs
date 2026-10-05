use super::*;

fn repo() -> RepoEntry {
    RepoEntry {
        id: "demo".to_string(),
        url: "https://example.invalid/demo".to_string(),
        revision: "a".repeat(40),
        license: "MIT".to_string(),
        labels: "labels/demo.json".to_string(),
        labeled: LabeledCount {
            caught: 1,
            missed: 1,
        },
    }
}

fn manifest(top_k: Vec<usize>) -> Manifest {
    Manifest {
        schema_version: MANIFEST_SCHEMA_VERSION.to_string(),
        corpus_version: "test".to_string(),
        description: "d".to_string(),
        label_rule: "r".to_string(),
        limits: vec!["l".to_string()],
        max_seams: 10,
        top_k,
        repos: vec![repo()],
    }
}

fn mutant(name: &str, line: u64, column: u64, genre: &str) -> Value {
    json!({
        "name": name,
        "file": "src/a.rs",
        "genre": genre,
        "span": {"start": {"line": line, "column": column}, "end": {"line": line, "column": column + 1}},
        "function": {"span": {"start": {"line": 1}, "end": {"line": 9}}},
    })
}

fn outcome(name: &str, summary: &str) -> Value {
    json!({"scenario": {"Mutant": {"name": name}}, "summary": summary})
}

#[test]
fn committed_answer_key_is_valid_and_labels_every_pinned_repo() -> Result<(), String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(DEFAULT_MANIFEST);
    let manifest = load_manifest(&path)?;
    let dir = path.parent().ok_or("manifest has no parent")?;
    let labels = load_all_labels(&manifest, dir)?;
    assert_eq!(labels.len(), manifest.repos.len());
    for (repo, labels) in manifest.repos.iter().zip(&labels) {
        let missed = labels
            .mutants
            .iter()
            .filter(|label| label.outcome == "missed")
            .count();
        // A label set with only one outcome could not refute or confirm a pick.
        assert!(missed > 0 && missed < labels.mutants.len(), "{}", repo.id);
    }
    Ok(())
}

#[test]
fn labels_join_outcomes_by_name_and_count_unviable_and_timeout() -> Result<(), String> {
    let mutants = json!([
        mutant("src/a.rs:5:9: replace > with >=", 5, 9, "BinaryOperator"),
        mutant("src/a.rs:3:1: replace f with ()", 3, 1, "FnValue"),
        mutant("src/a.rs:4:2: replace + with -", 4, 2, "BinaryOperator"),
        mutant("src/a.rs:6:2: replace * with /", 6, 2, "BinaryOperator"),
    ]);
    let outcomes = json!({"cargo_mutants_version": "27.1.0", "outcomes": [
        {"scenario": "Baseline", "summary": "Success"},
        outcome("src/a.rs:5:9: replace > with >=", "MissedMutant"),
        outcome("src/a.rs:3:1: replace f with ()", "CaughtMutant"),
        outcome("src/a.rs:4:2: replace + with -", "Unviable"),
        outcome("src/a.rs:6:2: replace * with /", "Timeout"),
    ]});
    let labels = labels_from_mutants_out(&repo(), &mutants, &outcomes)?;
    let summary = labels
        .mutants
        .iter()
        .map(|label| (label.line, label.outcome.as_str(), label.function))
        .collect::<Vec<_>>();
    assert_eq!(
        summary,
        vec![(3, "caught", Some([1, 9])), (5, "missed", Some([1, 9]))]
    );
    assert_eq!(labels.unlabeled.get("unviable"), Some(&1));
    assert_eq!(labels.unlabeled.get("timeout"), Some(&1));
    assert_eq!(labels.cargo_mutants_version, "27.1.0");
    Ok(())
}

#[test]
fn labels_refuse_a_mutant_without_an_outcome_or_an_orphan_outcome() {
    let mutants = json!([mutant("m1", 5, 9, "BinaryOperator")]);
    let missing = json!({"cargo_mutants_version": "27.1.0", "outcomes": []});
    let result = labels_from_mutants_out(&repo(), &mutants, &missing);
    assert!(
        result
            .as_ref()
            .is_err_and(|err| err.contains("has no outcome")),
        "{:?}",
        result.err()
    );

    let orphan = json!({"cargo_mutants_version": "27.1.0", "outcomes": [
        outcome("m1", "CaughtMutant"),
        outcome("m2", "MissedMutant"),
    ]});
    let result = labels_from_mutants_out(&repo(), &mutants, &orphan);
    assert!(
        result.as_ref().is_err_and(|err| err.contains("`m2`")),
        "{:?}",
        result.err()
    );
}

#[test]
fn rendered_labels_parse_back_unchanged_one_mutant_per_line() -> Result<(), String> {
    let mutants = json!([
        mutant("m1", 5, 9, "BinaryOperator"),
        mutant("m2", 7, 3, "FnValue"),
    ]);
    let outcomes = json!({"cargo_mutants_version": "27.1.0", "outcomes": [
        outcome("m1", "MissedMutant"),
        outcome("m2", "CaughtMutant"),
    ]});
    let labels = labels_from_mutants_out(&repo(), &mutants, &outcomes)?;
    let text = render_labels(&labels)?;
    let parsed: LabelFile = serde_json::from_str(&text).map_err(|err| err.to_string())?;
    assert_eq!(parsed.mutants, labels.mutants);
    assert_eq!(
        text.lines()
            .filter(|line| line.contains("\"name\""))
            .count(),
        2
    );
    validate_labels(&repo(), &parsed)
}

#[test]
fn label_validation_refuses_another_revision_unsorted_or_unknown_outcomes() {
    let label = |name: &str, line: u64, outcome: &str| Label {
        name: name.to_string(),
        genre: "BinaryOperator".to_string(),
        file: "src/a.rs".to_string(),
        line,
        column: 1,
        function: None,
        outcome: outcome.to_string(),
    };
    let file = |revision: String, mutants: Vec<Label>| LabelFile {
        schema_version: LABELS_SCHEMA_VERSION.to_string(),
        repo: "demo".to_string(),
        revision,
        cargo_mutants_version: "27.1.0".to_string(),
        cargo_mutants_args: Vec::new(),
        unlabeled: BTreeMap::new(),
        mutants,
    };
    let good = file(
        "a".repeat(40),
        vec![label("m1", 1, "caught"), label("m2", 2, "missed")],
    );
    assert_eq!(validate_labels(&repo(), &good), Ok(()));

    let stale = file("b".repeat(40), vec![label("m1", 1, "caught")]);
    assert!(validate_labels(&repo(), &stale).is_err());
    let unsorted = file(
        "a".repeat(40),
        vec![label("m2", 2, "missed"), label("m1", 1, "caught")],
    );
    assert!(validate_labels(&repo(), &unsorted).is_err());
    let timeout = file("a".repeat(40), vec![label("m1", 1, "timeout")]);
    assert!(validate_labels(&repo(), &timeout).is_err());

    // A file that lost a label still validates on its own; the manifest's
    // count is what refuses it.
    assert_eq!(check_label_count(&repo(), &good), Ok(()));
    let truncated = file("a".repeat(40), vec![label("m1", 1, "caught")]);
    assert_eq!(validate_labels(&repo(), &truncated), Ok(()));
    assert!(check_label_count(&repo(), &truncated).is_err_and(|err| err.contains("0 missed")));

    // A run narrowed to one file leaves the rest of the crate unlabeled.
    let mut subset = file(
        "a".repeat(40),
        vec![label("m1", 1, "caught"), label("m2", 2, "missed")],
    );
    subset.cargo_mutants_args = vec!["--file".to_string(), "src/a.rs".to_string()];
    assert!(
        validate_labels(&repo(), &subset).is_err_and(|err| err.contains("full cargo-mutants run"))
    );
}

#[test]
fn label_options_stay_on_label_and_selection_args_are_gone() -> Result<(), String> {
    let parse = |command: &str, args: &[&str]| {
        parse_options(
            command,
            &args.iter().map(ToString::to_string).collect::<Vec<_>>(),
        )
    };
    assert!(
        parse("label", &["--mutants-arg", "--file"])
            .is_err_and(|err| err.contains("unknown pilot-ranking option `--mutants-arg`"))
    );
    assert!(
        parse("score", &["--mutants-out", "demo=out"])
            .is_err_and(|err| err.contains("unknown pilot-ranking option `--mutants-out`"))
    );
    let label = parse("label", &["--mutants-out", "demo=out"])?;
    assert!(label.label_mutants_out.is_some());
    Ok(())
}

#[test]
fn manifest_validation_refuses_bad_cutoffs_pins_and_label_paths() {
    assert_eq!(validate_manifest(&manifest(vec![5, 10])), Ok(()));
    assert!(validate_manifest(&manifest(vec![10, 5])).is_err());
    assert!(validate_manifest(&manifest(vec![5, 11])).is_err());
    assert!(validate_manifest(&manifest(Vec::new())).is_err());

    let mut short_pin = manifest(vec![5]);
    short_pin.repos[0].revision = "abc".to_string();
    assert!(validate_manifest(&short_pin).is_err());
    let mut elsewhere = manifest(vec![5]);
    elsewhere.repos[0].labels = "../labels/demo.json".to_string();
    assert!(validate_manifest(&elsewhere).is_err());
}

#[test]
fn fetch_requires_explicit_network_opt_in() -> Result<(), String> {
    assert!(parse_options("fetch", &[]).is_err_and(|err| err.contains("--allow-network")));
    let options = parse_options("fetch", &["--allow-network".to_string()])?;
    assert!(options.allow_network);
    Ok(())
}

/// The labels must reach the shared judge in the shape it reads: a missed
/// operator mutant on the line confirms, and with no line mutant the
/// innermost function's whole-body mutant decides.
#[test]
fn labels_feed_the_shared_judge_line_then_owner_tier() -> Result<(), String> {
    let mutants = json!([
        mutant("src/a.rs:3:5: replace + with -", 3, 5, "BinaryOperator"),
        mutant("src/a.rs:2:1: replace f with ()", 2, 1, "FnValue"),
    ]);
    let outcomes = json!({"cargo_mutants_version": "27.1.0", "outcomes": [
        outcome("src/a.rs:3:5: replace + with -", "MissedMutant"),
        outcome("src/a.rs:2:1: replace f with ()", "CaughtMutant"),
    ]});
    let labels = labels_from_mutants_out(&repo(), &mutants, &outcomes)?;
    let (mutants, outcomes) = judge_inputs(&labels);
    let seam = |line: u64| json!({"seam_id": format!("s{line}"), "file": "src/a.rs", "line": line, "kind": "call_presence", "grip_class": "weakly_gripped"});
    let judged = pilot::judge_recommendations(
        &[seam(3), seam(4), seam(20)],
        &mutants,
        &outcomes,
        &BTreeMap::new(),
        &|_, _| None,
    );
    let verdicts = judged
        .iter()
        .map(|row| (row["verdict"].as_str(), row["tier"].as_str()))
        .collect::<Vec<_>>();
    assert_eq!(
        verdicts,
        vec![
            (Some("confirmed"), Some("line")),
            (Some("refuted"), Some("owner")),
            (Some("unscored"), Some("none")),
        ]
    );
    Ok(())
}

fn pick(verdict: &str, owner: Option<&str>, line: u64) -> Value {
    let mut row = json!({"verdict": verdict, "file": "src/a.rs", "line": line, "tier": "line"});
    if let Some(owner) = owner {
        row["owner"] = json!(owner);
    }
    row
}

#[test]
fn same_named_functions_in_two_files_stay_two_functions() {
    let mut other = pick("refuted", Some("parse"), 2);
    other["file"] = json!("src/b.rs");
    let judged = vec![pick("confirmed", Some("parse"), 1), other];
    assert_eq!(Cut::of(&judged, 10).distinct_functions, 2);
}

/// A scratch git checkout under the target directory, emptied first.
fn scratch_checkout(name: &str) -> Result<PathBuf, String> {
    let dir = PathBuf::from("target/ripr/pilot-ranking/test-scratch").join(name);
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|err| format!("remove {}: {err}", dir.display()))?;
    }
    fs::create_dir_all(&dir).map_err(|err| format!("create {}: {err}", dir.display()))?;
    git(&dir, &["init", "--quiet"])?;
    Ok(dir)
}

fn commit(dir: &Path, file: &str) -> Result<(), String> {
    fs::write(dir.join(file), "x\n").map_err(|err| format!("write {file}: {err}"))?;
    git(dir, &["add", file])?;
    git(
        dir,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "-m",
            file,
        ],
    )?;
    Ok(())
}

/// Marks `dir` as a finished fetch of its current HEAD, as `materialize` does.
fn mark_fetched(dir: &Path) -> Result<(), String> {
    let head = git(dir, &["rev-parse", "HEAD"])?;
    fs::write(
        dir.join(".git").join(OWNER_MARKER),
        format!("demo\n{head}\n"),
    )
    .map_err(|err| err.to_string())
}

/// A re-fetch replaces only what holds no one's work: an interrupted fetch
/// or a clean checkout of the commit fetch recorded (an earlier pin). Edits,
/// untracked files, commits on top and amends are refused at any revision.
#[test]
fn refetch_refuses_local_work_at_any_revision() -> Result<(), String> {
    // Paths and spawned git are relative to the working directory.
    let _cwd_guard = crate::acquire_test_cwd_read_guard();
    let interrupted = scratch_checkout("interrupted")?;
    fs::write(interrupted.join(".git").join(OWNER_MARKER), "demo\n")
        .map_err(|err| err.to_string())?;
    assert_eq!(local_work(&interrupted), None);

    let clean = scratch_checkout("clean")?;
    commit(&clean, "a.rs")?;
    mark_fetched(&clean)?;
    assert_eq!(local_work(&clean), None);

    let untracked = scratch_checkout("untracked")?;
    commit(&untracked, "a.rs")?;
    mark_fetched(&untracked)?;
    fs::write(untracked.join("notes.txt"), "mine\n").map_err(|err| err.to_string())?;
    assert!(local_work(&untracked).is_some_and(|work| work.contains("untracked")));

    let edited = scratch_checkout("edited")?;
    commit(&edited, "a.rs")?;
    mark_fetched(&edited)?;
    fs::write(edited.join("a.rs"), "edited\n").map_err(|err| err.to_string())?;
    assert!(local_work(&edited).is_some_and(|work| work.contains("edits")));

    let amended = scratch_checkout("amended")?;
    commit(&amended, "a.rs")?;
    mark_fetched(&amended)?;
    git(
        &amended,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "--amend",
            "-m",
            "mine",
        ],
    )?;
    assert!(local_work(&amended).is_some_and(|work| work.contains("did not check out")));

    let committed = scratch_checkout("committed")?;
    commit(&committed, "a.rs")?;
    mark_fetched(&committed)?;
    commit(&committed, "b.rs")?;
    // The refusal reaches fetch: the marked checkout survives.
    assert!(materialize(&repo(), &committed).is_err_and(|err| err.contains("did not check out")));
    assert!(committed.join("b.rs").is_file());

    // A clone fetch did not create (no marker) is never replaced.
    let foreign = scratch_checkout("foreign")?;
    commit(&foreign, "a.rs")?;
    assert!(
        materialize(&repo(), &foreign)
            .is_err_and(|err| err.contains("not created by pilot-ranking fetch"))
    );
    assert!(foreign.join("a.rs").is_file());
    Ok(())
}

#[test]
fn cuts_count_precision_and_distinct_functions_within_k() {
    let judged = vec![
        pick("confirmed", Some("src/a.rs::f"), 1),
        pick("refuted", Some("src/a.rs::f"), 2),
        pick("unscored", None, 3),
        pick("confirmed", Some("src/a.rs::g"), 4),
        pick("refuted", Some("src/a.rs::h"), 5),
    ];
    let top3 = Cut::of(&judged, 3);
    assert_eq!(
        top3,
        Cut {
            picks: 3,
            confirmed: 1,
            refuted: 1,
            unscored: 1,
            distinct_functions: 2,
            tiers: [[0, 0], [1, 1], [0, 0]],
        }
    );
    let json = Cut::of(&judged, 10).to_json();
    assert_eq!(json["picks"], 5);
    assert_eq!(json["precision"], json!(0.5));
    assert_eq!(json["scored_share"], json!(0.8));
    assert_eq!(json["distinct_functions"], 4);
    // Nothing scored is no precision, not a perfect one.
    assert_eq!(
        Cut::of(&[pick("unscored", None, 1)], 5).to_json()["precision"],
        Value::Null
    );
}

#[test]
fn report_pools_scored_repos_and_marks_an_unavailable_one_incomplete() {
    let manifest = manifest(vec![1, 2]);
    let repos = vec![
        RepoScore {
            id: "a".to_string(),
            revision: "r".to_string(),
            judged: Ok(vec![
                pick("confirmed", Some("f"), 1),
                pick("refuted", Some("g"), 2),
            ]),
        },
        RepoScore {
            id: "b".to_string(),
            revision: "r".to_string(),
            judged: Ok(vec![pick("refuted", Some("f"), 1)]),
        },
    ];
    let report = build_report(&manifest, Path::new("ripr"), &repos);
    assert_eq!(report["status"], "complete");
    assert_eq!(report["pooled"]["top1"]["confirmed"], 1);
    assert_eq!(report["pooled"]["top1"]["refuted"], 1);
    assert_eq!(report["pooled"]["top2"]["picks"], 3);
    // Distinct functions are counted per repository, then summed.
    assert_eq!(report["pooled"]["top2"]["distinct_functions"], 3);
    assert!(markdown(&report).contains("| all, top2 | 3 | 1 | 2 | 0 | 33.3% |"));

    let mut with_gap = repos;
    with_gap.push(RepoScore {
        id: "c".to_string(),
        revision: "r".to_string(),
        judged: Err("checkout missing".to_string()),
    });
    let report = build_report(&manifest, Path::new("ripr"), &with_gap);
    assert_eq!(report["status"], "incomplete");
    assert_eq!(report["unavailable_repos"], 1);
    assert!(markdown(&report).contains("| c | unavailable: checkout missing |"));
}

/// The seam tier reads each label's name (original operator) and column, so
/// a conversion that dropped or zeroed either would grade this pick by the
/// coarser line tier instead.
#[test]
fn labels_keep_the_name_and_column_the_seam_tier_reads() -> Result<(), String> {
    let mutants = json!([
        // `>` inside `a > b`, the seam's expression: missed.
        mutant(
            "src/a.rs:3:10: replace > with >= in f",
            3,
            10,
            "BinaryOperator"
        ),
        // `&&` elsewhere on the line: caught, and must not decide the seam.
        mutant(
            "src/a.rs:3:16: replace && with || in f",
            3,
            16,
            "BinaryOperator"
        ),
    ]);
    let outcomes = json!({"cargo_mutants_version": "27.1.0", "outcomes": [
        outcome("src/a.rs:3:10: replace > with >= in f", "MissedMutant"),
        outcome("src/a.rs:3:16: replace && with || in f", "CaughtMutant"),
    ]});
    let labels = labels_from_mutants_out(&repo(), &mutants, &outcomes)?;
    let (mutants, outcomes) = judge_inputs(&labels);
    let seam = json!({"seam_id": "s3", "file": "src/a.rs", "line": 3, "kind": "predicate_boundary", "grip_class": "weakly_gripped"});
    let expressions = BTreeMap::from([("s3", "a > b")]);
    let judged =
        pilot::judge_recommendations(&[seam], &mutants, &outcomes, &expressions, &|_, _| {
            Some("    if a > b && c {".to_string())
        });
    assert_eq!(judged[0]["tier"], "seam");
    assert_eq!(judged[0]["verdict"], "confirmed");
    assert_eq!(
        (judged[0]["caught"].as_u64(), judged[0]["missed"].as_u64()),
        (Some(0), Some(1))
    );
    Ok(())
}

/// The gate compares the receipt with this committed baseline; a ranking
/// metric missing from it, or tagged with another corpus version, would be
/// listed as not compared and pass. A corpus bump must refresh it.
#[test]
fn committed_baseline_carries_every_ranking_metric_completed() -> Result<(), String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../metrics/dx-scoreboard/pilot-ranking-baseline.json");
    let text = fs::read_to_string(&path).map_err(|err| err.to_string())?;
    let baseline: Value = serde_json::from_str(&text).map_err(|err| err.to_string())?;
    let manifest = load_manifest(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(DEFAULT_MANIFEST),
    )?;
    let corpus = format!("pilot-ranking corpus {}", manifest.corpus_version);
    for id in [
        "ranking.pilot_precision_top5",
        "ranking.pilot_precision_top10",
        "ranking.pilot_scored_share_top10",
        "ranking.pilot_distinct_function_share_top10",
        "ranking.pilot_picks_top10",
        "ranking.pilot_confirmed_top5",
        "ranking.pilot_refuted_top5",
        "ranking.pilot_confirmed_top10",
        "ranking.pilot_refuted_top10",
    ] {
        let metric = baseline["metrics"]
            .as_array()
            .and_then(|metrics| metrics.iter().find(|metric| metric["id"] == id))
            .ok_or_else(|| format!("baseline has no `{id}`"))?;
        assert!(metric["value"].is_number(), "{id}: {metric}");
        assert_eq!(metric["partial"], false, "{id}");
        let samples = metric["samples"].as_array().map_or(&[][..], Vec::as_slice);
        assert!(!samples.is_empty(), "{id}");
        for sample in samples {
            assert_eq!(sample["repo"].as_str(), Some(corpus.as_str()), "{id}");
        }
    }
    Ok(())
}

#[test]
fn default_binary_follows_the_target_directory_cargo_reports() -> Result<(), String> {
    let binary = release_binary(r#"{"target_directory": "/ci/scratch/target"}"#)?;
    assert_eq!(
        binary,
        Path::new("/ci/scratch/target")
            .join("release")
            .join(format!("ripr{}", std::env::consts::EXE_SUFFIX))
    );
    assert!(release_binary("{}").is_err_and(|err| err.contains("target_directory")));
    Ok(())
}

#[test]
fn label_refuses_a_run_narrower_than_the_pin_lists() {
    let named = |names: &[&str]| -> Value {
        Value::Array(names.iter().map(|name| json!({ "name": name })).collect())
    };
    let listed = named(&["a", "b", "c"]);
    assert_eq!(
        require_full_run("demo", &listed, &named(&["c", "a", "b"])),
        Ok(())
    );
    // `--file` dropped `c`; the run's own files agree with each other but
    // not with the unfiltered inventory.
    assert!(
        require_full_run("demo", &listed, &named(&["a", "b"]))
            .is_err_and(|err| err.contains("1 missing from the run"))
    );
    assert!(
        require_full_run("demo", &listed, &named(&["a", "b", "c", "d"]))
            .is_err_and(|err| err.contains("1 not listed"))
    );
    assert!(require_full_run("demo", &json!({}), &listed).is_err());
}
