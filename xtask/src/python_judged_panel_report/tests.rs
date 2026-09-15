use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use super::{
    AdjudicationRequest, RECORD_KIND, RECORD_SCHEMA_VERSION, RenderedReport, SPEC,
    acquire_record_lock, adjudicate_case_at, build_report_at, parse_replay_record_bytes,
};

const PANEL_DIR: &str = "fixtures/python-judged-pr-panel";

struct TempFixture {
    root: PathBuf,
    /// Unique name fragment embedded in every path under this fixture;
    /// used to prove volatile paths never leak into report bytes.
    marker: String,
}

impl TempFixture {
    fn new(name: &str) -> Result<Self, String> {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "ripr-py-panel-report-{name}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(root.join(format!("{PANEL_DIR}/diffs")))
            .map_err(|error| format!("create test fixture: {error}"))?;
        Ok(Self {
            marker: format!("{unique}"),
            root,
        })
    }

    fn write_diff(&self, name: &str, body: &str) -> Result<String, String> {
        let relative = format!("{PANEL_DIR}/diffs/{name}.diff");
        fs::write(self.root.join(&relative), body)
            .map_err(|error| format!("write test diff: {error}"))?;
        Ok(relative)
    }

    fn write_envelope(&self, name: &str, value: &Value) -> Result<String, String> {
        let relative = format!("{PANEL_DIR}/{name}");
        let body = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
        fs::write(self.root.join(&relative), body).map_err(|error| error.to_string())?;
        Ok(relative)
    }

    fn write_policy(&self, value: &Value) -> Result<String, String> {
        let body = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
        fs::write(self.root.join("policy.json"), body).map_err(|error| error.to_string())?;
        Ok("policy.json".to_string())
    }

    fn path(&self, name: &str) -> Result<String, String> {
        self.root
            .join(name)
            .to_str()
            .map(str::to_string)
            .ok_or_else(|| format!("non-UTF-8 temp path: `{}`", self.root.display()))
    }
}

impl Drop for TempFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

// Fully diff-proved synthetic diffs (hunks start at line 1), mirroring
// the PR B replay test bodies.
const GAP_BODY: &str = "--- a/pricing.py\n+++ b/pricing.py\n@@ -1,4 +1,4 @@\n def apply_discount(amount, threshold):\n-    if amount >= threshold:\n+    if amount > threshold:\n         return amount * 0.9\n     return amount\n";
const QUIET_BODY: &str = "--- a/pricing.py\n+++ b/pricing.py\n@@ -1,4 +1,4 @@\n def apply_discount(amount, threshold):\n     if amount >= threshold:\n-        return amount * 0.9\n+        return amount * 0.85\n     return amount\n";
const LIMIT_BODY: &str = "--- a/routes.py\n+++ b/routes.py\n@@ -1,3 +1,3 @@\n @app.route(\"/checkout\", methods=[\"POST\"])\n def checkout(order):\n-    return {\"total\": order.subtotal}\n+    return {\"total\": order.subtotal, \"tax\": order.subtotal * 0.2}\n";

fn seed_row(
    id: &str,
    repo: &str,
    direction: &str,
    diff: &str,
    target: &str,
    owner: &str,
    expected: &str,
) -> Value {
    json!({
        "id": id, "repo": repo, "diff_path": diff, "shape": ["pytest_library"],
        "expected_direction": direction,
        "anchor": {"file": target, "line": 2, "owner": owner, "boundary": "predicate equality boundary"},
        "expected_classification": expected,
        "expected_static_limit_kind": if direction == "should_limit" {
            Value::String("decorator_indirection".to_string())
        } else {
            Value::Null
        },
        "labels": {
            "top_card_useful": null, "false_actionable": null, "false_exposed": null,
            "verify_command_valid": null, "suggested_location_valid": null,
            "packet_boundaries_safe": null, "limitation_quality": null
        },
        "authority_boundary": "review_advisory_only",
        "repair_packet_ready": false,
        "must_not_claim": ["Do not treat a null label as a passing judgment."],
        "reason": "synthetic report selection reason"
    })
}

/// Three fully replayable seed rows spanning all three directions, so a
/// replay run produces one record per direction.
fn write_inventory(fixture: &TempFixture) -> Result<Vec<String>, String> {
    let gap = fixture.write_diff("report-gap", GAP_BODY)?;
    let quiet = fixture.write_diff("report-quiet", QUIET_BODY)?;
    let limit = fixture.write_diff("report-limit", LIMIT_BODY)?;
    let items = vec![
        seed_row(
            "report-gap-row",
            "report-gap-repo",
            "should_gap",
            &gap,
            "pricing.py",
            "apply_discount",
            "weakly_exposed",
        ),
        seed_row(
            "report-quiet-row",
            "report-quiet-repo",
            "should_stay_quiet",
            &quiet,
            "pricing.py",
            "apply_discount",
            "exposed",
        ),
        seed_row(
            "report-limit-row",
            "report-limit-repo",
            "should_limit",
            &limit,
            "routes.py",
            "checkout",
            "static_unknown",
        ),
    ];
    Ok(vec![fixture.write_envelope(
        "report-panel.json",
        &json!({
            "schema_version": "0.1",
            "kind": "python_judged_pr_panel_manifest",
            "spec": "RIPR-SPEC-0092",
            "tier": "B",
            "description": "Synthetic report inventory over three replayable rows.",
            "limits": ["synthetic report inventory remains advisory only"],
            "items": items
        }),
    )?])
}

fn request(
    case_id: &str,
    role: &str,
    identity: &str,
    verdict: &str,
    recorded_at: &str,
) -> AdjudicationRequest {
    AdjudicationRequest {
        case_id: case_id.to_string(),
        verdict: verdict.to_string(),
        role: role.to_string(),
        identity: identity.to_string(),
        evidence: vec!["pricing.py:2 (assertion at line 2)".to_string()],
        false_actionable: None,
        false_exposed: None,
        wrong_target: None,
        invalid_command: None,
        limitation_quality: None,
        notes: None,
        recorded_at: recorded_at.to_string(),
    }
}

fn worktree_binary() -> Result<String, String> {
    // FIX fqGSK (CodeRabbit #3681): resolve the binary cargo actually
    // built for this run — the RIPR_TEST_BINARY override first, then the
    // active CARGO_TARGET_DIR (the required rust-gates workflow points it
    // at an external directory), then the repo-local target directory —
    // selecting the profile from the test build itself (`cargo test
    // --release` puts the binary under `release`, not `debug`).
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let file_name = format!("ripr{}", std::env::consts::EXE_SUFFIX);
    let mut candidates = Vec::new();
    if let Ok(override_path) = std::env::var("RIPR_TEST_BINARY") {
        candidates.push(PathBuf::from(override_path));
    }
    if let Ok(target_root) = std::env::var("CARGO_TARGET_DIR") {
        candidates.push(Path::new(&target_root).join(profile).join(&file_name));
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("xtask manifest has no repository parent")?;
    candidates.push(root.join("target").join(profile).join(&file_name));
    let attempted = candidates
        .iter()
        .map(|candidate| candidate.display().to_string())
        .collect::<Vec<_>>()
        .join("`, `");
    for candidate in &candidates {
        if candidate.is_file() {
            return std::path::absolute(candidate)
                .map(|path| path.to_string_lossy().into_owned())
                .map_err(|error| format!("resolve worktree ripr binary: {error}"));
        }
    }
    Err(format!(
        "no built ripr binary found (looked at `{attempted}`); run `cargo build -p ripr` first (report tests resolve the binary and never spawn a nested build)"
    ))
}

fn ensure(condition: bool, message: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(format!("report test failed: {message}"))
    }
}

/// Two independent roles recorded over one case at `stamp`; `decide`
/// fills each role's assessment axes.
fn adjudicate_two_roles(
    fixture: &TempFixture,
    refs: &[&str],
    dir: &str,
    records: &str,
    case_id: &str,
    stamp: &str,
    decide: impl Fn(&mut AdjudicationRequest),
) -> Result<(), String> {
    for (role, identity) in [
        ("human_operator", "alice"),
        ("second_human_reviewer", "bob"),
    ] {
        let verdict = if case_id == "report-quiet-row" {
            "exposed"
        } else {
            "static_unknown"
        };
        let mut judged = request(case_id, role, identity, verdict, stamp);
        decide(&mut judged);
        adjudicate_case_at(fixture.root.as_path(), refs, dir, records, &judged)?;
    }
    Ok(())
}

/// The adjudication axes the shared pipeline records: the gap row is
/// decided clean on false_exposed, the quiet row flags a false_actionable
/// repair route, and the limit row grades its limitation as precise.
fn pipeline_decide(case_id: &str, judged: &mut AdjudicationRequest) {
    match case_id {
        "report-gap-row" => {
            judged.false_exposed = Some(false);
            judged.wrong_target = Some(false);
        }
        "report-quiet-row" => {
            judged.false_actionable = Some(true);
            judged.wrong_target = Some(true);
            judged.invalid_command = Some(true);
        }
        _ => {
            judged.limitation_quality = Some("precise".to_string());
        }
    }
}

/// One shared end-to-end pipeline: two independent replay runs (their
/// records embed different temp workspace paths and command lines) plus
/// two two-role adjudication sets stamped at different times, and the
/// empty-state render.
struct Pipeline {
    fixture: TempFixture,
    refs: Vec<String>,
    report_empty: RenderedReport,
    report_a: RenderedReport,
    report_b: RenderedReport,
    records_a: String,
    adjudications: String,
}

/// Copy the replay binary into the test fixture so a concurrent
/// `cargo build -p ripr` from a sibling test cannot relink the shared
/// target binary between the two replay runs. The suite build unifies
/// dev-dependency features, giving it different bytes than a bare
/// build, so a mid-test relink changes the recorded binary_sha256 (and
/// can ETXTBSY an exec). The pinned copy keeps the producer's bytes
/// and identity while making them immutable for the test.
fn pin_test_binary(fixture: &TempFixture, binary: &str) -> Result<String, String> {
    let pinned = fixture
        .root
        .join(format!("replay-binary{}", std::env::consts::EXE_SUFFIX));
    for _ in 0..3 {
        std::fs::copy(binary, &pinned)
            .map_err(|error| format!("copy replay binary for pinning: {error}"))?;
        let probe = std::process::Command::new(&pinned)
            .arg("--version")
            .output();
        if probe.is_ok_and(|output| output.status.success()) {
            return pinned
                .to_str()
                .ok_or_else(|| "utf-8 pinned binary".to_string())
                .map(str::to_string);
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
    Err("pinned replay binary copy does not execute; refusing a partial relink".to_string())
}

fn pipeline(name: &str) -> Result<Pipeline, String> {
    let fixture = TempFixture::new(name)?;
    let refs = write_inventory(&fixture)?;
    let ref_strs = refs.iter().map(String::as_str).collect::<Vec<_>>();
    let binary = worktree_binary()?;
    let binary = pin_test_binary(&fixture, &binary)?;
    let records_a = fixture.root.join("records-a");
    let records_b = fixture.root.join("records-b");
    for records in [&records_a, &records_b] {
        let summary = crate::python_judged_panel_replay::replay_inventory_at(
            &fixture.root,
            &ref_strs,
            records,
            None,
            binary.as_str(),
        )?;
        ensure(
            summary.replayed == 3,
            "every replay run must replay all three rows",
        )?;
    }
    for (dir, stamp) in [
        (fixture.root.join("adjudications-a"), "2026-09-04T00:00:00Z"),
        (fixture.root.join("adjudications-b"), "2027-12-31T23:59:59Z"),
    ] {
        for case_id in ["report-gap-row", "report-quiet-row", "report-limit-row"] {
            adjudicate_two_roles(
                &fixture,
                &ref_strs,
                dir.to_str().ok_or("utf-8 adjudications")?,
                records_a.to_str().ok_or("utf-8 records")?,
                case_id,
                stamp,
                |judged| pipeline_decide(case_id, judged),
            )?;
        }
    }
    let empty = build_report_at(
        fixture.root.as_path(),
        &ref_strs,
        Path::new("no-such-records"),
        "no-such-records",
        Path::new("no-such-adjudications"),
        "no-such-adjudications",
        None,
    )?;
    let render = |records: &Path, adjudications: &Path| {
        build_report_at(
            fixture.root.as_path(),
            &ref_strs,
            records,
            "records",
            adjudications,
            "adjudications",
            None,
        )
    };
    let report_a = render(&records_a, &fixture.root.join("adjudications-a"))?;
    let report_b = render(&records_b, &fixture.root.join("adjudications-b"))?;
    let adjudications = fixture.path("adjudications-a")?;
    let records_a_display = fixture.path("records-a")?;
    Ok(Pipeline {
        fixture,
        refs,
        report_empty: empty,
        report_a,
        report_b,
        records_a: records_a_display,
        adjudications,
    })
}

/// The byte-stability pin plus the shared-model pin: two independent
/// replay runs (volatile record content differs) and two adjudication
/// sets stamped at different times must render byte-identical JSON and
/// Markdown, with no volatile path leaking; both surfaces state the same
/// counts and keep the honesty notes visible.
#[test]
fn report_bytes_are_stable_across_independent_runs() -> Result<(), String> {
    let pipeline = pipeline("determinism")?;
    let (a, b) = (&pipeline.report_a, &pipeline.report_b);
    ensure(
        a.json == b.json && a.markdown == b.markdown,
        "two independent replay runs over identical inputs must render byte-identical reports",
    )?;
    ensure(
        !a.json.contains(&pipeline.fixture.marker),
        "the report must not echo volatile record content (fixture-root paths leak through the records' command field)",
    )?;
    // The adjudicated rows must be visible with both roles; the limit row
    // stays inconclusive (a real recorded state, never a pass).
    ensure(
        a.json.contains("\"adjudicated\": 2")
            && a.json.contains("\"inconclusive\": 1")
            && a.json.contains("\"selected\": 3"),
        "the two-role decided rows count as adjudicated; the undecided limit row is inconclusive",
    )?;
    ensure(
        a.json.contains("human_operator") && a.json.contains("second_human_reviewer"),
        "case-level roles must be disclosed",
    )?;
    // Markdown and JSON must describe the same model.
    ensure(
        a.markdown.contains("selected: 3") && a.json.contains("\"selected\": 3"),
        "both surfaces must state the same selected count",
    )?;
    ensure(
        a.markdown.contains("no combined score")
            && a.markdown.contains("relation basis: unavailable"),
        "markdown must state that no combined quality score exists and disclose the unavailable relation-basis dimension",
    )?;
    Ok(())
}

/// Separate error denominators: a decided false_exposed on a should_gap
/// row feeds only the false_exposed rate; false_actionable keeps its own
/// denominator; no denominator means no rate anywhere.
#[test]
fn report_derives_separate_error_denominators_and_rates() -> Result<(), String> {
    let pipeline = pipeline("rates")?;
    let empty = &pipeline.report_empty;
    for fragment in [
        "\"selected\": 3",
        "\"replayed\": 0",
        "\"not_run\": 3",
        "\"no_replay_record\": 3",
        "\"adjudicated\": 0",
        "\"unjudged\": 3",
    ] {
        ensure(
            empty.json.contains(fragment),
            &format!("the empty state must disclose its achieved denominator ({fragment})"),
        )?;
    }
    ensure(
        !empty.json.contains("\"rate\":"),
        "absent rates must be omitted from the JSON, not faked as zero",
    )?;
    ensure(
        empty
            .markdown
            .contains("rate not disclosed: no denominator"),
        "the markdown must keep the no-denominator-no-rate rule visible",
    )?;

    let report = &pipeline.report_a;
    let value = serde_json::from_str::<Value>(&report.json).map_err(|error| error.to_string())?;
    let rate = |key: &str, field: &str| value["rates"][key][field].as_u64().unwrap_or(u64::MAX);
    ensure(
        rate("false_exposed", "numerator") == 0 && rate("false_exposed", "denominator") == 1,
        "the false_exposed denominator must cover exactly the decided should_gap row",
    )?;
    ensure(
        rate("false_actionable", "numerator") == 1 && rate("false_actionable", "denominator") == 1,
        "the false_actionable denominator must cover exactly the decided should_stay_quiet row",
    )?;
    ensure(
        rate("wrong_target", "flagged") == 1
            && rate("wrong_target", "assessed") == 2
            && rate("invalid_command", "flagged") == 1,
        "wrong-target and invalid-command counts must come from the adjudications",
    )?;
    ensure(
        value["rates"]["limitation_correctness"]["not_adjudicated"].as_u64() == Some(1)
            && value["rates"]["limitation_correctness"]["precise"].as_u64() == Some(0),
        "an inconclusive should_limit row must not enter the limitation-correctness counts",
    )?;
    ensure(
        value["rates"]["false_actionable"]["denominator_case_ids"]
            .as_array()
            .is_some_and(|ids| {
                ids.iter()
                    .map(|id| id.as_str().unwrap_or("?"))
                    .collect::<Vec<_>>()
                    == ["report-quiet-row"]
            }),
        "the rate must name its exact denominator cases",
    )?;
    ensure(
        report
            .markdown
            .contains("false_actionable: numerator 1 / denominator 1"),
        "the markdown must carry the exact numerator and denominator",
    )?;
    ensure(
        report.json.contains("denominator_case_ids") && report.json.contains("coverage_boundary"),
        "every rate must carry its denominator case ids and coverage boundary",
    )?;
    Ok(())
}

/// Threshold evaluation is per-threshold pass/fail/not_evaluable, echoes
/// the policy rationale, and never writes a tier claim.
#[test]
fn threshold_evaluation_is_explicit_per_threshold_and_non_authoritative() -> Result<(), String> {
    let pipeline = pipeline("thresholds")?;
    let refs = pipeline.refs.iter().map(String::as_str).collect::<Vec<_>>();
    let policy = pipeline.fixture.write_policy(&json!({
        "schema_version": "0.1",
        "kind": "python_judged_panel_threshold_policy",
        "spec": "RIPR-SPEC-0092",
        "rationale": "candidate for discussion only: zero tolerance on both error axes once two rows are adjudicated",
        "authority": "panel working group (candidate, not accepted)",
        "thresholds": [
            {"metric": "false_actionable_rate", "operator": "max", "value": 0.0},
            {"metric": "false_exposed_rate", "operator": "max", "value": 0.0},
            {"metric": "adjudicated_count", "operator": "min", "value": 2}
        ]
    }))?;
    let before = build_report_at(
        pipeline.fixture.root.as_path(),
        &refs,
        Path::new("no-such-records"),
        "no-such-records",
        Path::new("no-such-adjudications"),
        "no-such-adjudications",
        Some(&policy),
    )?;
    // With zero adjudications: both rate thresholds are not_evaluable and
    // the count threshold fails — never silently passes.
    ensure(
        before.json.matches("\"result\": \"not_evaluable\"").count() == 2
            && before.json.contains("\"result\": \"fail\""),
        "rate thresholds must be not_evaluable without denominators and the count threshold must fail below its minimum",
    )?;
    ensure(
        before.json.contains("candidate for discussion only")
            && before.json.contains("never promotes support")
            && before.json.contains("no operator tier ruling"),
        "the policy rationale must be echoed and the non-authority note present",
    )?;

    // Over the pipeline's two-role adjudications (one flagged
    // false_actionable): that rate fails its zero threshold while the
    // others pass.
    let after = build_report_at(
        pipeline.fixture.root.as_path(),
        &refs,
        Path::new("no-such-records"),
        "no-such-records",
        Path::new(&pipeline.adjudications),
        "adjudications",
        Some(&policy),
    )?;
    ensure(
        after.json.contains("\"result\": \"pass\"") && after.json.contains("\"result\": \"fail\""),
        "a failing measured rate must be reported next to passing ones",
    )?;
    // FIX #3686: the threshold table renders count metrics without decimals
    // — matching the threshold column — while rates keep three decimals even
    // when they land exactly on an integer (the zero-tolerance case here
    // measures 1.0 and must render 1.000, not 1).
    ensure(
        after
            .markdown
            .contains("| adjudicated_count | min | 2 | 2 |")
            && after
                .markdown
                .contains("| false_actionable_rate | max | 0 | 1.000 |")
            && !after.markdown.contains("2.000"),
        "count metrics render bare, rates keep fixed three-decimal precision",
    )?;
    Ok(())
}

/// The adjudication workflow's honesty rules: reviewer identity and own
/// evidence citations are required, the verdict must use the conservative
/// vocabulary, the direction lattice holds, and independence requires two
/// distinct roles.
#[test]
fn adjudicate_rejects_unattributed_lattice_and_vocabulary_drift() -> Result<(), String> {
    let fixture = TempFixture::new("adjudicate-rejects")?;
    let refs = write_inventory(&fixture)?;
    let ref_strs = refs.iter().map(String::as_str).collect::<Vec<_>>();
    let dir = fixture.path("adjudications")?;
    let judge = |judge_request: AdjudicationRequest| {
        adjudicate_case_at(
            fixture.root.as_path(),
            &ref_strs,
            &dir,
            "records",
            &judge_request,
        )
    };
    let rejection = |judge_request: AdjudicationRequest, what: &str| {
        judge(judge_request)
            .err()
            .ok_or_else(|| format!("report test failed: {what}: expected rejection"))
    };

    let error = rejection(
        request("no-such-case", "human_operator", "alice", "exposed", "t0"),
        "case",
    )?;
    ensure(
        error.contains("unknown case id `no-such-case`"),
        "an unknown case id must be rejected",
    )?;
    let mut no_reviewer = request("report-gap-row", "human_operator", "", "exposed", "t0");
    no_reviewer.evidence.clear();
    let error = rejection(no_reviewer, "identity")?;
    ensure(
        error.contains("reviewer identity is required") && error.contains("RIPR_PANEL_ADJUDICATOR"),
        "identity is required and the error must name the env fallback",
    )?;
    let mut no_evidence = request("report-gap-row", "human_operator", "alice", "exposed", "t0");
    no_evidence.evidence.clear();
    let error = rejection(no_evidence, "evidence")?;
    ensure(
        error.contains("evidence") && error.contains("do not copy"),
        "the evidence requirement must warn against copying the candidate classification",
    )?;
    let error = rejection(
        request(
            "report-gap-row",
            "human_operator",
            "alice",
            "proven_correct",
            "t0",
        ),
        "verdict",
    )?;
    ensure(
        error.contains("conservative static vocabulary"),
        "the verdict must stay in vocabulary",
    )?;
    let mut both_true = request("report-gap-row", "human_operator", "alice", "exposed", "t0");
    both_true.false_actionable = Some(true);
    both_true.false_exposed = Some(true);
    let error = rejection(both_true, "lattice")?;
    ensure(
        error.contains("cannot both be true"),
        "the lattice must hold",
    )?;
    let mut inadmissible = request(
        "report-gap-row",
        "human_operator",
        "alice",
        "weakly_exposed",
        "t0",
    );
    inadmissible.false_actionable = Some(true);
    let error = rejection(inadmissible, "direction")?;
    ensure(
        error.contains("not admitted by direction `should_gap`"),
        "the direction lattice must gate the true labels",
    )?;
    let mut limit_quality_on_gap = request(
        "report-gap-row",
        "human_operator",
        "alice",
        "weakly_exposed",
        "t0",
    );
    limit_quality_on_gap.limitation_quality = Some("precise".to_string());
    let error = rejection(limit_quality_on_gap, "limitation")?;
    ensure(
        error.contains("only to `should_limit`"),
        "limitation grading stays limit-scoped",
    )?;

    // A single role never counts as adjudicated; the same role under a
    // second identity changes nothing; a second independent role does.
    let first = judge(request(
        "report-gap-row",
        "human_operator",
        "alice",
        "weakly_exposed",
        "t0",
    ))?;
    ensure(
        first.contains("pending_second_role"),
        "one role must stay pending",
    )?;
    let again = judge(request(
        "report-gap-row",
        "human_operator",
        "alice_recheck",
        "weakly_exposed",
        "t1",
    ))?;
    ensure(
        again.contains("pending_second_role"),
        "two identities under one role are still not independent",
    )?;
    let second = judge(request(
        "report-gap-row",
        "second_human_reviewer",
        "bob",
        "weakly_exposed",
        "t2",
    ))?;
    ensure(
        second.contains("inconclusive") && second.contains("not a pass"),
        "an agreeing judgment with no decided axis must be inconclusive, not a pass",
    )?;

    // Disagreement is a named state with a recorded disposition required.
    judge(request(
        "report-quiet-row",
        "human_operator",
        "alice",
        "exposed",
        "t0",
    ))?;
    let disputed = judge(request(
        "report-quiet-row",
        "second_human_reviewer",
        "bob",
        "weakly_exposed",
        "t1",
    ))?;
    ensure(
        disputed.contains("disputed"),
        "role disagreement must surface as disputed",
    )?;
    Ok(())
}

/// The report reader fails closed on record-set rot against the real
/// pipeline record set: unknown case ids, mixed as-of identity, and
/// foreign record kinds are rejected instead of silently folded into the
/// counts.
#[test]
fn report_fails_closed_on_record_set_rot() -> Result<(), String> {
    let pipeline = pipeline("record-rot")?;
    let records = Path::new(&pipeline.records_a);
    let record_path = |name: &str| records.join(name);
    let render = || {
        build_report_at(
            pipeline.fixture.root.as_path(),
            &pipeline.refs.iter().map(String::as_str).collect::<Vec<_>>(),
            records,
            "records",
            Path::new("adjudications"),
            "adjudications",
            None,
        )
    };
    let failure = |what: &str| -> Result<String, String> {
        render()
            .err()
            .ok_or_else(|| format!("report test failed: {what}: expected rejection"))
    };
    let rewrite = |name: &str, mutate: &dyn Fn(&mut Value)| -> Result<(), String> {
        let mut value = serde_json::from_str::<Value>(
            &fs::read_to_string(record_path(name)).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        mutate(&mut value);
        fs::write(
            record_path(name),
            serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())
    };
    // The real records share one binary identity; rot stages must keep it
    // consistent so each stage exercises exactly one rejection.
    let shared_binary = serde_json::from_str::<Value>(
        &fs::read_to_string(record_path("report-quiet-row.json"))
            .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?["binary"]
        .clone();

    // Stage 1: a record naming a case outside the inventory is rejected
    // (built on the shared identity so the case check fires first).
    let ghost = json!({
        "schema_version": "0.1",
        "kind": "python_judged_panel_replay_record",
        "spec": "RIPR-SPEC-0092",
        "case_id": "ghost-case",
        "binary": shared_binary,
        "diff": {"sha256": "b".repeat(64)},
        "outcome": {"kind": "not_run"},
        "comparison": {"kind": "comparison_unavailable"}
    });
    fs::write(
        record_path("ghost-case.json"),
        serde_json::to_string_pretty(&ghost).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    ensure(
        failure("rot")?.contains("ghost-case"),
        "the foreign case must be named",
    )?;
    fs::remove_file(record_path("ghost-case.json")).map_err(|error| error.to_string())?;

    // Stage 2: mixed binary identity across two records is rejected.
    rewrite("report-gap-row.json", &|value: &mut Value| {
        value["binary"]["sha256"] = json!("c".repeat(64));
    })?;
    ensure(
        failure("rot")?.contains("mixed binary identity"),
        "mixed identity must be named",
    )?;

    // Stage 2b (FIX #3686): a blank or malformed diff digest must fail the
    // read — a blank-to-blank currency match at report time would otherwise
    // let a hand-edited record report a stale replay as current.
    rewrite("report-gap-row.json", &|value: &mut Value| {
        value["binary"] = shared_binary.clone();
        value["diff"]["sha256"] = json!("");
    })?;
    ensure(
        failure("rot")?.contains("malformed diff sha256"),
        "the blank digest must be named",
    )?;
    rewrite("report-gap-row.json", &|value: &mut Value| {
        value["diff"]["sha256"] = json!("xyz-not-hex");
    })?;
    ensure(
        failure("rot")?.contains("malformed diff sha256"),
        "the non-hex digest must be named",
    )?;
    // FIX (round-2 review): uppercase hex passes a naive shape check but can
    // never match the producer's lowercase digests — reject it as malformed.
    rewrite("report-gap-row.json", &|value: &mut Value| {
        value["diff"]["sha256"] = json!("B".repeat(64));
    })?;
    ensure(
        failure("rot")?.contains("malformed diff sha256"),
        "the uppercase digest must be named",
    )?;
    // Restore a valid digest so stage 3 exercises the kind check alone.
    rewrite("report-gap-row.json", &|value: &mut Value| {
        value["diff"]["sha256"] = json!("b".repeat(64));
    })?;

    // Stage 3: a foreign kind is rejected even with a consistent identity
    // (the record is rewritten back onto the shared identity so the kind
    // check — not the identity check — fires).
    rewrite("report-gap-row.json", &|value: &mut Value| {
        value["kind"] = json!("some_other_record");
        value["binary"] = shared_binary.clone();
    })?;
    ensure(
        failure("rot")?.contains("unknown identity"),
        "foreign kinds must be named",
    )?;

    // Stage 4 (FIX #3677): a malformed anchor echo is a named read error.
    rewrite("report-gap-row.json", &|value: &mut Value| {
        value["kind"] = json!("python_judged_panel_replay_record");
        value["anchor"] = json!({"line": 3});
    })?;
    ensure(
        failure("rot")?.contains("anchor echo without a file"),
        "the malformed anchor echo must be named",
    )?;
    Ok(())
}

/// FIX fqoP (devin round 2): a replay record file is addressable only by
/// its case's stable slug — a renamed record is rejected instead of
/// silently folding its evidence into the counts under filename order.
#[test]
fn report_replay_records_reject_a_slug_mismatch() -> Result<(), String> {
    let pipeline = pipeline("record-slug-binding")?;
    let records = Path::new(&pipeline.records_a);
    let body = fs::read_to_string(records.join("report-quiet-row.json"))
        .map_err(|error| error.to_string())?;
    fs::write(records.join("misnamed.json"), body).map_err(|error| error.to_string())?;
    let failure = build_report_at(
        pipeline.fixture.root.as_path(),
        &pipeline.refs.iter().map(String::as_str).collect::<Vec<_>>(),
        records,
        "records",
        Path::new("adjudications"),
        "adjudications",
        None,
    )
    .err()
    .ok_or("report test failed: expected slug-mismatch rejection")?;
    ensure(
        failure.contains("is not named"),
        &format!("the slug contract must be named, got: {failure}"),
    )?;
    Ok(())
}

/// FIX fqqq (devin round 2): the replay reader deliberately tolerates
/// unknown producer fields — forward compatibility, so an older report
/// reader keeps reading records from a newer replay producer. This test
/// pins that contract: unknown fields at every level parse, and the
/// projected view stays limited to the documented keys so a future
/// producer fact can never silently leak into report bytes.
#[test]
fn replay_record_input_pins_the_compatibility_contract() -> Result<(), String> {
    let body = json!({
        "schema_version": RECORD_SCHEMA_VERSION,
        "kind": RECORD_KIND,
        "spec": SPEC,
        "case_id": "compat-case",
        "row_kind": "gap",
        "binary": {"version": "0.0.0-test", "sha256": "a".repeat(64), "future_binary_fact": 1},
        "diff": {"sha256": "b".repeat(64), "path": "case.diff", "future_diff_fact": true},
        "outcome": {"kind": "not_run", "future_outcome_fact": []},
        "comparison": {"kind": "comparison_unavailable", "future_comparison_fact": "x"},
        "anchor": {"file": "case.py", "line": 7, "owner": "case_owner", "future_anchor_fact": true},
        "future_top_level_fact": {"nested": [1, 2, 3]}
    });
    let serialized = serde_json::to_string(&body).map_err(|error| error.to_string())?;
    let record = parse_replay_record_bytes(&serialized, "compat-test")?;
    ensure(
        record.case_id == "compat-case" && record.row_kind == "gap",
        "the documented consumed fields must still project",
    )?;
    ensure(
        record.binary.is_some() && record.diff.is_some(),
        "the documented identity fields must still project",
    )?;
    ensure(
        record.anchor.is_some(),
        "the anchor echo projects as a documented field",
    )?;
    Ok(())
}

/// FIX fqlm (devin round 2): adjudicating one case is a read-modify-write
/// cycle, so a second concurrent adjudication while the record lock is
/// held must fail closed with a named error instead of silently
/// discarding one judgment.
#[test]
fn concurrent_adjudication_is_refused_while_a_record_lock_is_held() -> Result<(), String> {
    let fixture = TempFixture::new("adjudicate-lock")?;
    let record_path = fixture.root.join("adjudications").join("some-case.json");
    let parent_dir = record_path
        .parent()
        .ok_or("lock test failed: record path has no parent")?;
    fs::create_dir_all(parent_dir).map_err(|error| error.to_string())?;
    let _held = acquire_record_lock(&record_path)?;
    let second = acquire_record_lock(&record_path)
        .err()
        .ok_or("lock test failed: expected the second acquisition to be refused")?;
    ensure(
        second.contains("is locked"),
        &format!("the lock contract must be named, got: {second}"),
    )?;
    drop(_held);
    acquire_record_lock(&record_path)
        .map_err(|error| format!("the lock must release on drop: {error}"))?;
    Ok(())
}
/// FIX f2THL/f2XZZ: the record replacement is atomic and read-failure
/// safe — an injected rename failure fails named with the obstruction
/// untouched and leaves no temp residue, and invalid UTF-8 at the record
/// path is a named error instead of a silent overwrite.
#[test]
fn adjudication_writes_are_atomic_and_read_failures_are_refused() -> Result<(), String> {
    let fixture = TempFixture::new("atomic-writes")?;
    let refs = write_inventory(&fixture)?;
    let ref_strs = refs.iter().map(String::as_str).collect::<Vec<_>>();
    let dir = fixture.path("adjudications")?;
    let dest = Path::new(&dir).join("report-gap-row.json");
    let judge = |judge_request: AdjudicationRequest| {
        adjudicate_case_at(
            fixture.root.as_path(),
            &ref_strs,
            &dir,
            "records",
            &judge_request,
        )
    };

    // A prior record exists from the first role.
    judge(request(
        "report-gap-row",
        "human_operator",
        "alice",
        "weakly_exposed",
        "t0",
    ))?;
    let prior = fs::read(&dest).map_err(|error| error.to_string())?;

    // Injected write failure (FIX fqGSQ, CodeRabbit #3681). Each platform
    // injects the failure where it actually can, and the assertion names
    // the mechanism that really fired — the old single loose assertion
    // accepted a lock-step failure on Unix while claiming rename coverage:
    // - Windows: a read-only record file is still readable (the read and
    //   lock steps succeed) but the staged rename cannot replace it, so
    //   the rename-failure path is genuinely exercised.
    // - Unix: a parent directory unwritable for the lock fails the lock
    //   step first, and a record path that is a directory fails the read;
    //   the latter is asserted here because it is reachable regardless of
    //   the effective uid (root bypasses permission bits).
    #[cfg(windows)]
    {
        let mut permissions = fs::metadata(&dest)
            .map_err(|error| error.to_string())?
            .permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&dest, permissions).map_err(|error| error.to_string())?;
    }
    #[cfg(not(windows))]
    {
        fs::remove_file(&dest).map_err(|error| error.to_string())?;
        fs::create_dir(&dest).map_err(|error| error.to_string())?;
    }
    let failure = judge(request(
        "report-gap-row",
        "second_human_reviewer",
        "bob",
        "weakly_exposed",
        "t1",
    ))
    .err()
    .ok_or("report test failed: injected write failure must be a named error")?;
    ensure(
        failure.contains("adjudication record"),
        "the write failure must name the record",
    )?;
    #[cfg(windows)]
    ensure(
        failure.contains("rename failed"),
        &format!("the rename failure must be named, got: {failure}"),
    )?;
    #[cfg(not(windows))]
    ensure(
        failure.contains("refusing to overwrite an unreadable record"),
        &format!("the read refusal must be named, got: {failure}"),
    )?;

    // Restore the record path and the prior bytes; the second role then
    // records normally and no staged temp file survives in the directory.
    #[cfg(windows)]
    {
        // Clearing FILE_ATTRIBUTE_READONLY is the only way to undo the
        // injected failure on Windows; the Unix-mode lint does not apply
        // to this cfg-gated branch.
        #[expect(
            clippy::permissions_set_readonly_false,
            reason = "restoring the Windows file attribute after the injected write failure"
        )]
        {
            let mut permissions = fs::metadata(&dest)
                .map_err(|error| error.to_string())?
                .permissions();
            permissions.set_readonly(false);
            fs::set_permissions(&dest, permissions).map_err(|error| error.to_string())?;
        }
        ensure(
            fs::read(&dest).map_err(|error| error.to_string())? == prior,
            "the prior record must survive an injected write failure",
        )?;
    }
    #[cfg(not(windows))]
    {
        ensure(
            fs::metadata(&dest)
                .map_err(|error| error.to_string())?
                .is_dir(),
            "the writer must leave the injected obstruction untouched",
        )?;
        fs::remove_dir(&dest).map_err(|error| error.to_string())?;
        fs::write(&dest, &prior).map_err(|error| error.to_string())?;
    }

    // After restoring, the second role records normally and no staged
    // temp file survives in the record directory.
    judge(request(
        "report-gap-row",
        "second_human_reviewer",
        "bob",
        "weakly_exposed",
        "t1",
    ))?;
    for entry in fs::read_dir(&dir).map_err(|error| error.to_string())? {
        let name = entry
            .map_err(|error| error.to_string())?
            .file_name()
            .to_string_lossy()
            .to_string();
        ensure(!name.contains(".tmp-"), "no temp residue may survive")?;
    }

    // Invalid UTF-8 at the record path is a named error, never a
    // truncated or replaced record.
    fs::write(&dest, [0xFF_u8, 0xFE, 0x00]).map_err(|error| error.to_string())?;
    let corrupted = fs::read(&dest).map_err(|error| error.to_string())?;
    let error = judge(request(
        "report-gap-row",
        "human_operator",
        "alice",
        "weakly_exposed",
        "t2",
    ))
    .err()
    .ok_or("report test failed: unreadable record must be refused")?;
    ensure(
        error.contains("read existing adjudication record") && error.contains("report-gap-row"),
        "the read failure must name the unreadable record path",
    )?;
    ensure(
        fs::read(&dest).map_err(|error| error.to_string())? == corrupted,
        "the unreadable record must not be overwritten",
    )?;
    Ok(())
}

/// A minimal stored adjudication record for report-side validation tests;
/// the arguments carry the pieces under test.
fn stored_record(
    case_id: &str,
    direction: &str,
    verdict: &str,
    evidence: Value,
    false_actionable: Value,
    false_exposed: Value,
) -> Value {
    json!({
        "schema_version": "0.1",
        "kind": "python_judged_panel_adjudication_record",
        "spec": "RIPR-SPEC-0092",
        "case_id": case_id,
        "source_envelope": "fixtures/python-judged-pr-panel/report-panel.json",
        "expected_direction": direction,
        "must_not_claim": ["Do not treat a null label as a passing judgment."],
        "judgments": [{
            "reviewer_role": "human_operator",
            "reviewer_identity": "alice",
            "recorded_at": "2026-09-04T00:00:00Z",
            "verdict": verdict,
            "false_actionable": false_actionable,
            "false_exposed": false_exposed,
            "wrong_target": null,
            "invalid_command": null,
            "limitation_quality": null,
            "evidence_references": evidence,
            "notes": null
        }],
        "authority_boundary": "review_advisory_only"
    })
}

fn write_stored(fixture: &TempFixture, value: &Value) -> Result<String, String> {
    let dir = fixture.path("adjudications")?;
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let case_id = value["case_id"].as_str().unwrap_or("?").to_string();
    let slug = crate::python_judged_panel_replay::stable_case_slug(&case_id);
    fs::write(
        Path::new(&dir).join(format!("{slug}.json")),
        serde_json::to_string_pretty(value).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    Ok(dir)
}

/// FIX fqGSD (CodeRabbit #3681): independence requires a one-to-one
/// role-to-identity mapping. One identity occupying two roles is refused
/// at write time, one role carrying two identities is replaced by a
/// re-adjudication, and a hand-edited record that slips a shared identity
/// past both count checks fails the report named.
#[test]
fn role_to_identity_must_be_one_to_one() -> Result<(), String> {
    let fixture = TempFixture::new("identity-bijection")?;
    let refs = write_inventory(&fixture)?;
    let ref_strs = refs.iter().map(String::as_str).collect::<Vec<_>>();
    let dir = fixture.path("adjudications")?;
    let judge = |role: &str, identity: &str| {
        adjudicate_case_at(
            fixture.root.as_path(),
            &ref_strs,
            &dir,
            "records",
            &request(
                "report-gap-row",
                role,
                identity,
                "weakly_exposed",
                "2026-09-04T00:00:00Z",
            ),
        )
    };
    judge("human_operator", "alice")?;

    // alice cannot take a second role on the same case.
    let refusal = judge("second_human_reviewer", "alice")
        .err()
        .ok_or("bijection test failed: the shared identity must be refused")?;
    ensure(
        refusal.contains("already recorded under a different role"),
        &format!("the shared-identity refusal must be named, got: {refusal}"),
    )?;

    // FIX fA8C (devin #3682): a padded identity is rejected outright — it
    // must neither borrow a clean identity's record nor dodge its
    // independence conflict through a string-comparison gap.
    let padded = judge("second_human_reviewer", " alice ")
        .err()
        .ok_or("bijection test failed: the padded identity must be refused")?;
    ensure(
        padded.contains("must not carry leading or trailing whitespace"),
        &format!("the padded-identity refusal must be named, got: {padded}"),
    )?;

    // A role re-recording under a changed identity replaces its prior
    // judgment instead of accumulating two identities on one role.
    judge("human_operator", "carol")?;
    let record_path = Path::new(&dir).join("report-gap-row.json");
    let value = serde_json::from_str::<Value>(
        &fs::read_to_string(&record_path).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    ensure(
        value["judgments"]
            .as_array()
            .is_some_and(|judgments| judgments.len() == 1)
            && value["judgments"][0]["reviewer_identity"] == "carol",
        "the role re-record must replace its prior judgment",
    )?;

    // A hand-edited record that satisfies both count checks with a shared
    // identity still fails the report named.
    let mut forged = value.clone();
    forged["judgments"]
        .as_array_mut()
        .ok_or("judgments array")?
        .push(json!({
            "reviewer_role": "second_human_reviewer",
            "reviewer_identity": "carol",
            "recorded_at": "2026-09-04T00:00:00Z",
            "verdict": "weakly_exposed",
            "false_actionable": null,
            "false_exposed": null,
            "wrong_target": null,
            "invalid_command": null,
            "limitation_quality": null,
            "evidence_references": ["pricing.py:2"],
            "notes": null
        }));
    fs::write(
        &record_path,
        serde_json::to_string_pretty(&forged).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let report_error = build_report_at(
        fixture.root.as_path(),
        &ref_strs,
        Path::new("records"),
        "records",
        Path::new(&dir),
        "adjudications",
        None,
    )
    .err()
    .ok_or("bijection test failed: the forged record must fail the report")?;
    ensure(
        report_error.contains("not one-to-one"),
        &format!("the bijection violation must be named, got: {report_error}"),
    )?;
    Ok(())
}

/// FIX f2XZU: every stored judgment runs through the same semantic rules
/// the CLI enforces — a malformed stored record fails the report with a
/// violation named per case, it is never silently excluded.
#[test]
fn report_fails_on_semantically_invalid_stored_judgments() -> Result<(), String> {
    let fixture = TempFixture::new("stored-semantics")?;
    let refs = write_inventory(&fixture)?;
    let ref_strs = refs.iter().map(String::as_str).collect::<Vec<_>>();
    for (name, record, fragment) in [
        (
            "empty evidence",
            stored_record(
                "report-gap-row",
                "should_gap",
                "weakly_exposed",
                json!([]),
                json!(null),
                json!(null),
            ),
            "evidence is required",
        ),
        (
            "unknown verdict",
            stored_record(
                "report-gap-row",
                "should_gap",
                "proven_correct",
                json!(["pricing.py:2"]),
                json!(null),
                json!(null),
            ),
            "conservative static vocabulary",
        ),
        (
            "both error flags",
            stored_record(
                "report-gap-row",
                "should_gap",
                "weakly_exposed",
                json!(["pricing.py:2"]),
                json!(true),
                json!(true),
            ),
            "cannot both be true",
        ),
    ] {
        let dir = write_stored(&fixture, &record)?;
        let error = build_report_at(
            fixture.root.as_path(),
            &ref_strs,
            Path::new("records"),
            "records",
            Path::new(&dir),
            "adjudications",
            None,
        )
        .err()
        .ok_or_else(|| format!("report test failed: {name}: expected rejection"))?;
        ensure(
            error.contains("report-gap-row") && error.contains(fragment),
            &format!(
                "report test failed: {name}: violation must name the case and reason, found: {error}"
            ),
        )?;
        fs::remove_dir_all(&dir).map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// FIX f2TIz: carryover rows (null expected_classification) cannot be
/// adjudicated and never enter adjudicated counts, error denominators,
/// quality counts, or thresholds — an injected record for one is
/// excluded defensively.
#[test]
fn carryover_rows_are_never_adjudicated_or_counted() -> Result<(), String> {
    let fixture = TempFixture::new("carryover")?;
    let refs = write_inventory(&fixture)?;
    // The retained sqlalchemy-style carryover: robustness-only row with
    // null expected_classification, no anchor, and the grandfathered
    // timeout limit kind.
    let carryover_diff = fixture.write_diff("report-carryover", LIMIT_BODY)?;
    let carryover = json!({
        "id": "report-carryover-row",
        "repo": "report-carryover-repo",
        "diff_path": carryover_diff,
        "shape": ["pytest_library"],
        "expected_direction": "should_limit",
        "anchor": {"file": null, "line": null, "owner": "carried_owner", "boundary": "carried boundary"},
        "expected_classification": null,
        "expected_static_limit_kind": "timeout",
        "actual_classification": null,
        "actual_oracle_alignment": null,
        "labels": {
            "top_card_useful": null, "false_actionable": null, "false_exposed": null,
            "verify_command_valid": null, "suggested_location_valid": null,
            "packet_boundaries_safe": null, "limitation_quality": "imprecise"
        },
        "judgment_source": "manual_review",
        "judged_at": "2026-06-13",
        "judged_by": "campaign",
        "authority_boundary": "review_advisory_only",
        "repair_packet_ready": false,
        "must_not_claim": ["Robustness carryover; not a judged denominator row."],
        "reason": "retained robustness sweep carryover row"
    });
    let envelope_path = format!("{PANEL_DIR}/report-panel.json");
    let mut envelope = serde_json::from_str::<Value>(
        &fs::read_to_string(fixture.root.join(&envelope_path))
            .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    envelope["items"]
        .as_array_mut()
        .ok_or("items must be an array")?
        .push(carryover);
    fs::write(
        fixture.root.join(&envelope_path),
        serde_json::to_string_pretty(&envelope).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let ref_strs = refs.iter().map(String::as_str).collect::<Vec<_>>();

    // The CLI refuses to adjudicate a carryover row, with a named reason.
    let error = adjudicate_case_at(
        fixture.root.as_path(),
        &ref_strs,
        &fixture.path("adjudications")?,
        "records",
        &request(
            "report-carryover-row",
            "human_operator",
            "alice",
            "static_unknown",
            "t0",
        ),
    )
    .err()
    .ok_or("report test failed: carryover rows must refuse adjudication")?;
    ensure(
        error.contains("cannot be adjudicated"),
        "the refusal must name the carryover rule",
    )?;

    // An injected record for the carryover row is excluded defensively:
    // it never enters the adjudicated count, and the case entry carries
    // no adjudication.
    let mut injected = stored_record(
        "report-carryover-row",
        "should_limit",
        "static_unknown",
        json!(["routes.py:3"]),
        json!(false),
        json!(false),
    );
    injected["judgments"]
        .as_array_mut()
        .ok_or("judgments must be an array")?
        .push(json!({
            "reviewer_role": "second_human_reviewer",
            "reviewer_identity": "bob",
            "recorded_at": "2026-09-04T00:00:00Z",
            "verdict": "static_unknown",
            "false_actionable": false,
            "false_exposed": false,
            "wrong_target": null,
            "invalid_command": null,
            "limitation_quality": null,
            "evidence_references": ["routes.py:3"],
            "notes": null
        }));
    let dir = write_stored(&fixture, &injected)?;
    let report = build_report_at(
        fixture.root.as_path(),
        &ref_strs,
        Path::new("records"),
        "records",
        Path::new(&dir),
        "adjudications",
        None,
    )?;
    ensure(
        report.json.contains("\"adjudicated\": 0") && report.json.contains("\"stale_row\": 0"),
        "an injected carryover record must stay out of every adjudicated count",
    )?;
    ensure(
        report.json.contains("\"adjudication\": null"),
        "the carryover case entry must disclose no adjudication",
    )?;
    Ok(())
}

/// FIX f2TIA: verdict-to-error coherence mirrors the retained-panel
/// validator's outcome table — `exposed` on a should_gap/should_limit row
/// is an over-credit and requires false_exposed true, on the CLI and in
/// every stored judgment.
#[test]
fn verdict_error_coherence_follows_the_direction_lattice() -> Result<(), String> {
    let fixture = TempFixture::new("verdict-coherence")?;
    let refs = write_inventory(&fixture)?;
    let ref_strs = refs.iter().map(String::as_str).collect::<Vec<_>>();
    let dir = fixture.path("adjudications")?;

    // CLI: an exposed verdict on a should_gap row without the
    // false_exposed=true label is rejected.
    let error = adjudicate_case_at(
        fixture.root.as_path(),
        &ref_strs,
        &dir,
        "records",
        &request("report-gap-row", "human_operator", "alice", "exposed", "t0"),
    )
    .err()
    .ok_or("report test failed: incoherent verdict must be rejected")?;
    ensure(
        error.contains("over-credit") && error.contains("should_gap"),
        "the coherence violation must name the over-credit rule",
    )?;

    // Two roles recording the over-credit coherently: verdict exposed
    // plus false_exposed true is admitted and lands in the rate.
    for (role, identity) in [
        ("human_operator", "alice"),
        ("second_human_reviewer", "bob"),
    ] {
        let mut judged = request(
            "report-gap-row",
            role,
            identity,
            "exposed",
            "2026-09-04T00:00:00Z",
        );
        judged.false_exposed = Some(true);
        adjudicate_case_at(fixture.root.as_path(), &ref_strs, &dir, "records", &judged)?;
    }
    let report = build_report_at(
        fixture.root.as_path(),
        &ref_strs,
        Path::new("records"),
        "records",
        Path::new(&dir),
        "adjudications",
        None,
    )?;
    let value = serde_json::from_str::<Value>(&report.json).map_err(|error| error.to_string())?;
    ensure(
        value["rates"]["false_exposed"]["numerator"].as_u64() == Some(1)
            && value["counts"]["adjudicated"].as_u64() == Some(1),
        "the coherent over-credit judgment must count as adjudicated and feed the false_exposed numerator",
    )?;

    // Stored incoherence fails the report named per case.
    let incoherent = stored_record(
        "report-gap-row",
        "should_gap",
        "exposed",
        json!(["pricing.py:2"]),
        json!(null),
        json!(null),
    );
    let incoherent_dir = write_stored(&fixture, &incoherent)?;
    let error = build_report_at(
        fixture.root.as_path(),
        &ref_strs,
        Path::new("records"),
        "records",
        Path::new(&incoherent_dir),
        "adjudications",
        None,
    )
    .err()
    .ok_or("report test failed: stored incoherence must fail the report")?;
    ensure(
        error.contains("report-gap-row") && error.contains("over-credit"),
        "the stored incoherence must name the case and rule",
    )?;
    Ok(())
}

/// FIX f2TMb: a rate's as-of identity derives only from the denominator
/// cases' own replay records; a denominator case without a record forces
/// the `no_common_binary_identity` disclosure instead of a fabricated
/// directory-wide identity.
#[test]
fn rate_as_of_identity_binds_the_denominator_cases_records() -> Result<(), String> {
    let fixture = TempFixture::new("rate-as-of")?;
    let refs = write_inventory(&fixture)?;
    let ref_strs = refs.iter().map(String::as_str).collect::<Vec<_>>();
    let records = fixture.root.join("records");
    crate::python_judged_panel_replay::replay_inventory_at(
        &fixture.root,
        &ref_strs,
        &records,
        None,
        worktree_binary()?.as_str(),
    )?;
    let adjudications = fixture.path("adjudications")?;
    let records_display = records.to_str().ok_or("records utf-8")?;
    for (case_id, decided_axis) in [
        ("report-gap-row", "false_exposed"),
        ("report-quiet-row", "false_actionable"),
    ] {
        for (role, identity) in [
            ("human_operator", "alice"),
            ("second_human_reviewer", "bob"),
        ] {
            let mut judged = request(
                case_id,
                role,
                identity,
                if case_id == "report-quiet-row" {
                    "exposed"
                } else {
                    "weakly_exposed"
                },
                "2026-09-04T00:00:00Z",
            );
            if decided_axis == "false_exposed" {
                judged.false_exposed = Some(false);
            } else {
                judged.false_actionable = Some(true);
            }
            adjudicate_case_at(
                fixture.root.as_path(),
                &ref_strs,
                &adjudications,
                records_display,
                &judged,
            )?;
        }
    }
    let render =
        |records_dir: &Path, adjudications_dir: &str| -> Result<super::RenderedReport, String> {
            build_report_at(
                fixture.root.as_path(),
                &ref_strs,
                records_dir,
                "records",
                Path::new(adjudications_dir),
                "adjudications",
                None,
            )
        };
    let full = render(&records, &adjudications)?;
    let value = serde_json::from_str::<Value>(&full.json).map_err(|error| error.to_string())?;
    ensure(
        value["rates"]["false_exposed"]["as_of_basis"] == "denominator_case_records"
            && value["rates"]["false_exposed"]["as_of"]["binary_version"]
                .as_str()
                .is_some(),
        "a denominator case with its own record binds the rate as-of identity",
    )?;

    // Drop the quiet row's record: the false_actionable denominator now
    // has no record to bind, so the rate discloses instead of citing.
    let partial = fixture.root.join("records-partial");
    fs::create_dir_all(&partial).map_err(|error| error.to_string())?;
    for entry in fs::read_dir(&records).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path.file_name().and_then(|name| name.to_str()) == Some("report-quiet-row.json") {
            continue;
        }
        fs::copy(&path, partial.join(path.file_name().ok_or("file name")?))
            .map_err(|error| error.to_string())?;
    }
    let partial_report = render(&partial, &adjudications)?;
    let value =
        serde_json::from_str::<Value>(&partial_report.json).map_err(|error| error.to_string())?;
    ensure(
        value["rates"]["false_actionable"]["as_of_basis"] == "no_common_binary_identity"
            && value["rates"]["false_actionable"]["as_of"]["binary_version"].is_null(),
        "a denominator case without a record must disclose no_common_binary_identity",
    )?;
    ensure(
        value["rates"]["false_exposed"]["as_of_basis"] == "denominator_case_records",
        "denominator cases that do bind records still cite their identity",
    )?;

    // FIX fqNy: a denominator record whose bound diff no longer matches
    // the case's current diff is stale; its binary identity must not
    // label the rate either.
    let stale_dir = fixture.root.join("records-stale");
    fs::create_dir_all(&stale_dir).map_err(|error| error.to_string())?;
    for entry in fs::read_dir(&records).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        let name = path
            .file_name()
            .ok_or("record file name")?
            .to_string_lossy()
            .to_string();
        let body = fs::read_to_string(&path).map_err(|error| error.to_string())?;
        let mut value = serde_json::from_str::<Value>(&body).map_err(|error| error.to_string())?;
        if name == "report-quiet-row.json" {
            value["diff"]["sha256"] = json!("f".repeat(64));
        }
        fs::write(
            stale_dir.join(&name),
            serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
    }
    let stale_report = render(&stale_dir, &adjudications)?;
    let value =
        serde_json::from_str::<Value>(&stale_report.json).map_err(|error| error.to_string())?;
    ensure(
        value["rates"]["false_actionable"]["as_of_basis"] == "no_common_binary_identity",
        "a stale denominator record must not label the rate with its identity",
    )?;
    Ok(())
}

/// FIX fqNa (devin round 4): stored provenance echoes (envelope,
/// direction, non-claims) and the stored recorded_at must stay consistent
/// with the validated row; a drifted echo fails the report and refuses
/// re-adjudication instead of being preserved.
#[test]
fn stored_provenance_echoes_must_match_the_validated_row() -> Result<(), String> {
    let fixture = TempFixture::new("provenance-echo")?;
    let refs = write_inventory(&fixture)?;
    let ref_strs = refs.iter().map(String::as_str).collect::<Vec<_>>();
    let dir = fixture.path("adjudications")?;
    adjudicate_case_at(
        fixture.root.as_path(),
        &ref_strs,
        &dir,
        "records",
        &request(
            "report-gap-row",
            "human_operator",
            "alice",
            "weakly_exposed",
            "2026-09-04T00:00:00Z",
        ),
    )?;
    let record_path = Path::new(&dir).join("report-gap-row.json");
    let mutate = |edit: &dyn Fn(&mut Value)| -> Result<(), String> {
        let mut value = serde_json::from_str::<Value>(
            &fs::read_to_string(&record_path).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        edit(&mut value);
        fs::write(
            &record_path,
            serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())
    };
    let report_error = || -> Result<String, String> {
        build_report_at(
            fixture.root.as_path(),
            &ref_strs,
            Path::new("records"),
            "records",
            Path::new(&dir),
            "adjudications",
            None,
        )
        .err()
        .ok_or("report test failed: drifted provenance must fail the report".to_string())
    };

    // Direction echo drift fails the report and refuses re-adjudication.
    mutate(&|value: &mut Value| value["expected_direction"] = json!("should_limit"))?;
    ensure(
        report_error()?.contains("stored provenance contradicts"),
        "a drifted direction echo must fail the report",
    )?;
    let refusal = adjudicate_case_at(
        fixture.root.as_path(),
        &ref_strs,
        &dir,
        "records",
        &request(
            "report-gap-row",
            "second_human_reviewer",
            "bob",
            "weakly_exposed",
            "2026-09-04T00:00:00Z",
        ),
    )
    .err()
    .ok_or("report test failed: drifted provenance must refuse re-adjudication")?;
    ensure(
        refusal.contains("contradicts the current validated row"),
        "re-adjudication must refuse a drifted record",
    )?;
    mutate(&|value: &mut Value| value["expected_direction"] = json!("should_gap"))?;

    // Non-claims echo drift fails the report too.
    mutate(&|value: &mut Value| value["must_not_claim"] = json!(["never claim X"]))?;
    ensure(
        report_error()?.contains("stored provenance contradicts"),
        "a drifted non-claims echo must fail the report",
    )?;
    mutate(&|value: &mut Value| {
        value["must_not_claim"] = json!(["Do not treat a null label as a passing judgment."])
    })?;

    // A stored timestamp that is not a parseable RFC 3339 instant is
    // rejected provenance.
    mutate(&|value: &mut Value| value["judgments"][0]["recorded_at"] = json!("not-a-time"))?;
    ensure(
        report_error()?.contains("parseable RFC 3339"),
        "a fabricated timestamp must fail the report",
    )?;
    Ok(())
}

/// FIX fqNz (devin round 4): cite_replay_record cites a record at the
/// expected name only when it declares the requested case; foreign
/// content under the right file name is never attributed as the case's
/// advisory evidence.
#[test]
fn cite_replay_record_refuses_foreign_case_content() -> Result<(), String> {
    let fixture = TempFixture::new("cite-foreign")?;
    let records = fixture.root.join("records");
    fs::create_dir_all(&records).map_err(|error| error.to_string())?;
    let write_record = |case_id: &str| -> Result<(), String> {
        let body = json!({
            "schema_version": RECORD_SCHEMA_VERSION,
            "kind": RECORD_KIND,
            "spec": SPEC,
            "case_id": case_id,
            "binary": {"version": "0.0.0-test", "sha256": "a".repeat(64)},
            "diff": {"sha256": "b".repeat(64)},
            "outcome": {"kind": "not_run"},
            "comparison": {"kind": "comparison_unavailable"}
        });
        fs::write(
            records.join("report-gap-row.json"),
            serde_json::to_string(&body).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())
    };
    let records_display = records.to_str().ok_or("records utf-8")?;
    write_record("some-other-case")?;
    ensure(
        super::cite_replay_record(records_display, "report-gap-row").is_none(),
        "foreign case content under the expected name must not be cited",
    )?;
    write_record("report-gap-row")?;
    ensure(
        super::cite_replay_record(records_display, "report-gap-row").is_some(),
        "the honest record is cited",
    )?;
    Ok(())
}

/// FIX fqNv (devin round 4): report generations are serialized per output
/// directory; a held generation lock refuses a second publisher with a
/// named error instead of letting two renames interleave into a mixed
/// json/markdown pair.
#[test]
fn report_generations_are_serialized_by_the_output_lock() -> Result<(), String> {
    let fixture = TempFixture::new("report-lock")?;
    let out = fixture.root.join("out");
    fs::create_dir_all(&out).map_err(|error| error.to_string())?;
    let _held = super::acquire_path_lock(
        out.join(".report-generation.lock"),
        "report generation",
        |_| "held".to_string(),
    )?;
    let failure = super::write_report_generation(
        &out.join("report.json"),
        &out.join("report.md"),
        "{\"v\":1}\n",
        "# v1\n",
    )
    .err()
    .ok_or("report test failed: the held lock must refuse a second publisher")?;
    ensure(
        failure.contains("another report generation"),
        "the lock contract must be named, got: {failure}",
    )?;
    Ok(())
}

/// FIX #3677: a replay whose echoed anchor no longer matches the row (or
/// that predates the echo) is anchor-stale — disclosed with a named
/// reason, never silently current, and excluded from the rate as-of
/// identity.
#[test]
fn anchor_drift_marks_replay_stale() -> Result<(), String> {
    let pipeline = pipeline("anchor-drift")?;
    let refs = pipeline.refs.iter().map(String::as_str).collect::<Vec<_>>();
    let records = Path::new(&pipeline.records_a);
    let render = || {
        build_report_at(
            pipeline.fixture.root.as_path(),
            &refs,
            records,
            "records",
            Path::new("no-such-adjudications"),
            "adjudications",
            None,
        )
    };
    // The producer echoes the row anchor into every record.
    let fresh = render()?;
    let value = serde_json::from_str::<Value>(&fresh.json).map_err(|error| error.to_string())?;
    ensure(
        value["counts"]["anchor_stale"].as_u64() == Some(0)
            && value["counts"]["stale"].as_u64() == Some(0),
        "a fresh record set has no anchor staleness",
    )?;

    // An anchor move (same diff, same row kind) marks the replay stale
    // with the named reason and drops the case from the rate identity.
    let mutate = |edit: &dyn Fn(&mut Value)| -> Result<(), String> {
        let path = records.join("report-gap-row.json");
        let mut value = serde_json::from_str::<Value>(
            &fs::read_to_string(&path).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        edit(&mut value);
        fs::write(
            &path,
            serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())
    };
    mutate(&|value: &mut Value| {
        value["anchor"]["owner"] = json!("moved_owner");
    })?;
    let drifted = render()?;
    let value = serde_json::from_str::<Value>(&drifted.json).map_err(|error| error.to_string())?;
    ensure(
        value["counts"]["anchor_stale"].as_u64() == Some(1),
        "the anchor-moved record must land in the anchor_stale disclosure",
    )?;
    let gap_case = value["cases"]
        .as_array()
        .and_then(|cases| {
            cases
                .iter()
                .find(|case| case["case_id"] == "report-gap-row")
        })
        .ok_or("the gap case must be present")?;
    ensure(
        gap_case["replay"]["anchor_stale_reason"] == "anchor_moved"
            && gap_case["replay"]["identity_current"] == false,
        "the moved anchor must be named per case and never count as current",
    )?;
    // (The rate as-of exclusion follows from identity_current == false via
    // the fqNy rate-identity binding, pinned by the rate as-of test.)

    // A record written before the echo existed is anchor-stale too.
    mutate(&|value: &mut Value| {
        value["anchor"] = Value::Null;
    })?;
    let legacy = render()?;
    let value = serde_json::from_str::<Value>(&legacy.json).map_err(|error| error.to_string())?;
    ensure(
        value["counts"]["anchor_stale"].as_u64() == Some(1),
        "the echo-less record must land in the anchor_stale disclosure",
    )?;
    let gap_case = value["cases"]
        .as_array()
        .and_then(|cases| {
            cases
                .iter()
                .find(|case| case["case_id"] == "report-gap-row")
        })
        .ok_or("the gap case must be present")?;
    ensure(
        gap_case["replay"]["anchor_stale_reason"] == "missing_echo",
        "a missing echo must be named as such",
    )?;
    Ok(())
}

/// FIX f2XZN: an adjudication is bound to the full row revision; a row
/// or diff change after adjudication makes the record `stale_row` —
/// excluded from every denominator and disclosed with stored-vs-current
/// digests.
#[test]
fn adjudications_stale_against_a_changed_row_revision() -> Result<(), String> {
    let adjudicate_gap =
        |fixture: &TempFixture, refs: &[String], dir: &str| -> Result<(), String> {
            let ref_strs = refs.iter().map(String::as_str).collect::<Vec<_>>();
            for (role, identity) in [
                ("human_operator", "alice"),
                ("second_human_reviewer", "bob"),
            ] {
                let mut judged = request(
                    "report-gap-row",
                    role,
                    identity,
                    "weakly_exposed",
                    "2026-09-04T00:00:00Z",
                );
                judged.false_exposed = Some(false);
                adjudicate_case_at(fixture.root.as_path(), &ref_strs, dir, "records", &judged)?;
            }
            Ok(())
        };
    let render = |fixture: &TempFixture, refs: &[String], dir: &str| -> Result<Value, String> {
        let ref_strs = refs.iter().map(String::as_str).collect::<Vec<_>>();
        let report = build_report_at(
            fixture.root.as_path(),
            &ref_strs,
            Path::new("records"),
            "records",
            Path::new(dir),
            "adjudications",
            None,
        )?;
        serde_json::from_str::<Value>(&report.json).map_err(|error| error.to_string())
    };

    // Sub-case 1: the row's own content changes after adjudication.
    let fixture = TempFixture::new("row-revision")?;
    let refs = write_inventory(&fixture)?;
    let dir = fixture.path("adjudications")?;
    adjudicate_gap(&fixture, &refs, &dir)?;
    let before = render(&fixture, &refs, &dir)?;
    let entry = &before["cases"][0];
    ensure(
        entry["case_id"] == "report-gap-row"
            && entry["adjudication"]["row_revision"]["stored"]
                == entry["adjudication"]["row_revision"]["current"],
        "a fresh adjudication binds the current row revision",
    )?;
    let envelope_path = format!("{PANEL_DIR}/report-panel.json");
    let mut envelope = serde_json::from_str::<Value>(
        &fs::read_to_string(fixture.root.join(&envelope_path))
            .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    envelope["items"][0]["reason"] = json!("reason changed after adjudication");
    fs::write(
        fixture.root.join(&envelope_path),
        serde_json::to_string_pretty(&envelope).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let after = render(&fixture, &refs, &dir)?;
    ensure(
        after["counts"]["stale_row"].as_u64() == Some(1)
            && after["counts"]["adjudicated"].as_u64() == Some(0),
        "a changed row makes the adjudication stale and drops it from the denominator",
    )?;
    let entry = &after["cases"][0];
    ensure(
        entry["adjudication"]["row_revision"]["stored"]
            != entry["adjudication"]["row_revision"]["current"],
        "the stale entry must disclose stored-vs-current digests",
    )?;

    // Sub-case 2: only the diff content changes after adjudication.
    let fixture = TempFixture::new("row-revision-diff")?;
    let refs = write_inventory(&fixture)?;
    let dir = fixture.path("adjudications")?;
    adjudicate_gap(&fixture, &refs, &dir)?;
    let diff_path = format!("{PANEL_DIR}/diffs/report-gap.diff");
    let diff =
        fs::read_to_string(fixture.root.join(&diff_path)).map_err(|error| error.to_string())?;
    fs::write(
        fixture.root.join(&diff_path),
        diff.replace("return amount * 0.9", "return amount * 0.95"),
    )
    .map_err(|error| error.to_string())?;
    let after = render(&fixture, &refs, &dir)?;
    ensure(
        after["counts"]["stale_row"].as_u64() == Some(1),
        "a changed diff breaks the row-revision binding too",
    )?;
    Ok(())
}

/// FIX f2TNz: report.json and report.md publish as one generation — a
/// failure between the two publications is rolled back to the prior pair.
#[test]
fn report_publication_is_one_generation() -> Result<(), String> {
    let fixture = TempFixture::new("one-generation")?;
    let out = fixture.root.join("out");
    let json_path = out.join("report.json");
    let markdown_path = out.join("report.md");
    super::write_report_generation(&json_path, &markdown_path, "{\"v\":1}\n", "# v1\n")?;
    let prior_json = fs::read(&json_path).map_err(|e| format!("read prior json: {e}"))?;
    let prior_markdown = fs::read(&markdown_path).map_err(|e| format!("read prior md: {e}"))?;

    // Inject a failure between the two publications: the markdown path
    // is occupied by a directory, so its rename fails after report.json
    // was already replaced.
    fs::remove_file(&markdown_path).map_err(|error| format!("remove prior markdown: {error}"))?;
    fs::create_dir(&markdown_path).map_err(|error| format!("create md dir: {error}"))?;
    let error = super::write_report_generation(&json_path, &markdown_path, "{\"v\":2}\n", "# v2\n")
        .err()
        .ok_or("report test failed: the injected second-publication failure must surface")?;
    ensure(
        error.contains("prior report.json generation was restored"),
        "the failure must disclose the rollback",
    )?;
    ensure(
        fs::read(&json_path).map_err(|e| format!("read rolled-back json: {e}"))? == prior_json,
        "report.json must be rolled back to the prior generation",
    )?;
    ensure(
        markdown_path.is_dir(),
        "the prior markdown must be untouched (still the injected obstruction)",
    )?;
    let _ = prior_markdown;
    for entry in fs::read_dir(&out).map_err(|error| error.to_string())? {
        let name = entry
            .map_err(|error| error.to_string())?
            .file_name()
            .to_string_lossy()
            .to_string();
        ensure(!name.contains(".tmp-"), "no staged temp may survive")?;
    }

    // Removing the obstruction lets the next generation publish fully.
    fs::remove_dir(&markdown_path).map_err(|error| format!("remove md dir: {error}"))?;
    super::write_report_generation(&json_path, &markdown_path, "{\"v\":2}\n", "# v2\n")?;
    ensure(
        fs::read(&json_path).map_err(|error| error.to_string())? == b"{\"v\":2}\n"
            && fs::read(&markdown_path).map_err(|error| error.to_string())? == b"# v2\n",
        "the next generation publishes both files",
    )?;
    Ok(())
}
