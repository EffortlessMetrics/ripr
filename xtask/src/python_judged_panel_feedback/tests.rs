//! Feedback staging tests: a self-contained fixture replays two rows and
//! adjudicates them, then exercises the confirmed-over-credit detection,
//! staging determinism, `--check`, and the never-writes-the-corpus contract.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::{derive_feedback, verify_staged, write_staging};
use crate::python_judged_panel_replay::replay_inventory_at;
use crate::python_judged_panel_report::{AdjudicationRequest, adjudicate_case_at};

const PANEL_DIR: &str = "fixtures/python-judged-pr-panel";

const GAP_BODY: &str = "--- a/pricing.py\n+++ b/pricing.py\n@@ -1,4 +1,4 @@\n def apply_discount(amount, threshold):\n-    if amount >= threshold:\n+    if amount > threshold:\n         return amount * 0.9\n     return amount\n";
const QUIET_BODY: &str = "--- a/pricing.py\n+++ b/pricing.py\n@@ -1,4 +1,4 @@\n def apply_discount(amount, threshold):\n     if amount >= threshold:\n-        return amount * 0.9\n+        return amount * 0.85\n     return amount\n";
const LIMIT_BODY: &str = "--- a/routes.py\n+++ b/routes.py\n@@ -1,3 +1,3 @@\n @app.route(\"/checkout\", methods=[\"POST\"])\n def checkout(order):\n-    return {\"total\": order.subtotal}\n+    return {\"total\": order.subtotal, \"tax\": order.subtotal * 0.2}\n";

struct TempFixture {
    root: PathBuf,
}

impl Drop for TempFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

impl TempFixture {
    fn new(name: &str) -> Result<Self, String> {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "ripr-py-panel-feedback-{name}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(root.join(format!("{PANEL_DIR}/diffs")))
            .map_err(|error| format!("create test fixture: {error}"))?;
        Ok(Self { root })
    }

    fn write_diff(&self, name: &str, body: &str) -> Result<String, String> {
        let relative = format!("{PANEL_DIR}/diffs/{name}.diff");
        fs::write(self.root.join(&relative), body)
            .map_err(|error| format!("write test diff: {error}"))?;
        Ok(relative)
    }
}

/// Three anchored seed rows spanning all three directions (the loader
/// requires full directional coverage), so a replay run produces one record
/// per direction and the gap row is the over-credit candidate.
fn write_inventory(fixture: &TempFixture) -> Result<Vec<String>, String> {
    let gap = fixture.write_diff("report-gap", GAP_BODY)?;
    let quiet = fixture.write_diff("report-quiet", QUIET_BODY)?;
    let limit = fixture.write_diff("report-limit", LIMIT_BODY)?;
    let seed = |id: &str, repo: &str, direction: &str, diff: &str, owner: &str, target: &str| {
        json!({
            "id": id, "repo": repo, "diff_path": diff, "shape": ["pytest_library"],
            "expected_direction": direction,
            "anchor": {"file": target, "line": 2, "owner": owner, "boundary": "predicate equality boundary"},
            "expected_classification": match direction {
                "should_gap" => "weakly_exposed",
                "should_limit" => "static_unknown",
                _ => "exposed",
            },
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
            "reason": "synthetic feedback selection reason"
        })
    };
    let items = vec![
        seed(
            "feedback-gap-row",
            "feedback-gap-repo",
            "should_gap",
            &gap,
            "apply_discount",
            "pricing.py",
        ),
        seed(
            "feedback-quiet-row",
            "feedback-quiet-repo",
            "should_stay_quiet",
            &quiet,
            "apply_discount",
            "pricing.py",
        ),
        seed(
            "feedback-limit-row",
            "feedback-limit-repo",
            "should_limit",
            &limit,
            "checkout",
            "routes.py",
        ),
    ];
    let envelope = json!({
        "schema_version": "0.1",
        "kind": "python_judged_pr_panel_manifest",
        "spec": "RIPR-SPEC-0092",
        "tier": "B",
        "description": "Synthetic feedback inventory over two replayable rows.",
        "limits": ["synthetic feedback inventory remains advisory only"],
        "items": items
    });
    let relative = format!("{PANEL_DIR}/feedback-panel.json");
    fs::write(
        fixture.root.join(&relative),
        serde_json::to_string_pretty(&envelope).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    Ok(vec![relative])
}

fn worktree_binary() -> Result<String, String> {
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
    for candidate in &candidates {
        if candidate.is_file() {
            return std::path::absolute(candidate)
                .map(|path| path.to_string_lossy().into_owned())
                .map_err(|error| format!("resolve worktree ripr binary: {error}"));
        }
    }
    Err(format!(
        "no built ripr binary found (looked at {}); run `cargo build -p ripr` first (feedback tests resolve the binary and never spawn a nested build)",
        candidates
            .iter()
            .map(|candidate| candidate.display().to_string())
            .collect::<Vec<_>>()
            .join("`, `")
    ))
}

fn judge_request(
    case_id: &str,
    role: &str,
    identity: &str,
    verdict: &str,
    false_exposed: Option<bool>,
    false_actionable: Option<bool>,
) -> AdjudicationRequest {
    AdjudicationRequest {
        case_id: case_id.to_string(),
        verdict: verdict.to_string(),
        role: role.to_string(),
        identity: identity.to_string(),
        evidence: vec!["pricing.py:2 (assertion at line 2)".to_string()],
        false_actionable,
        false_exposed,
        wrong_target: None,
        invalid_command: None,
        limitation_quality: None,
        notes: None,
        recorded_at: "2026-09-08T00:00:00Z".to_string(),
    }
}

/// Replays both rows and adjudicates the gap row as a confirmed over-credit
/// (two roles agreeing `exposed` with `false_exposed` decided true).
fn seeded_pipeline(name: &str) -> Result<(TempFixture, Vec<String>, String, String), String> {
    let fixture = TempFixture::new(name)?;
    let refs = write_inventory(&fixture)?;
    let ref_strs = refs.iter().map(String::as_str).collect::<Vec<_>>();
    let records = fixture.root.join("records");
    replay_inventory_at(
        &fixture.root,
        &ref_strs,
        &records,
        None,
        worktree_binary()?.as_str(),
    )?;
    let adjudications = fixture.root.join("adjudications");
    for (role, identity) in [
        ("human_operator", "alice"),
        ("second_human_reviewer", "bob"),
    ] {
        adjudicate_case_at(
            fixture.root.as_path(),
            &ref_strs,
            adjudications.to_str().ok_or("adjudications utf-8")?,
            records.to_str().ok_or("records utf-8")?,
            &judge_request(
                "feedback-gap-row",
                role,
                identity,
                "exposed",
                Some(true),
                None,
            ),
        )?;
    }
    Ok((
        fixture,
        refs,
        adjudications
            .to_str()
            .ok_or("adjudications utf-8")?
            .to_string(),
        records.to_str().ok_or("records utf-8")?.to_string(),
    ))
}

fn corpus_digest() -> Result<String, String> {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("xtask manifest has no repository parent")?;
    let bytes = fs::read(
        repo_root
            .join("fixtures")
            .join("evidence-promotion-honesty-corpus")
            .join("corpus.json"),
    )
    .map_err(|error| error.to_string())?;
    Ok(format!("{:x}", md5_like(&bytes)))
}

/// Stable content fingerprint without pulling an md5 dependency: FNV-1a.
fn md5_like(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// FIX #3680 acceptance: no confirmed over-credits stage an explicit empty
/// set — zero is a denominator, not silence.
#[test]
fn feedback_with_no_adjudications_stages_an_explicit_empty_set() -> Result<(), String> {
    let fixture = TempFixture::new("feedback-empty")?;
    let refs = write_inventory(&fixture)?;
    let ref_strs = refs.iter().map(String::as_str).collect::<Vec<_>>();
    let records = fixture.root.join("records");
    replay_inventory_at(
        &fixture.root,
        &ref_strs,
        &records,
        None,
        worktree_binary()?.as_str(),
    )?;
    let out = fixture.root.join("feedback");
    let staged = derive_feedback(
        fixture.root.as_path(),
        &ref_strs,
        &records,
        &fixture.root.join("no-such-adjudications"),
        "no-such-adjudications",
    )?;
    write_staging(&out, &staged)?;
    let index = serde_json::from_str::<Value>(
        &fs::read_to_string(out.join("index.json")).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    ensure(
        index["confirmed_over_credits"].as_u64() == Some(0)
            && index["proposal_files"]
                .as_array()
                .is_some_and(Vec::is_empty),
        "an empty confirmed set must stage an explicit empty index",
    )?;
    Ok(())
}

/// FIX #3680 acceptance: a confirmed over-credit stages one proposal with the
/// full provenance chain and the promotion recipe — and never touches the
/// corpus.
#[test]
fn feedback_stages_a_proposal_for_a_confirmed_over_credit() -> Result<(), String> {
    let corpus_before = corpus_digest()?;
    let (fixture, refs, adjudications, records) = seeded_pipeline("feedback-seeded")?;
    let out = fixture.root.join("feedback");
    let ref_strs = refs.iter().map(String::as_str).collect::<Vec<_>>();
    let staged = derive_feedback(
        fixture.root.as_path(),
        &ref_strs,
        Path::new(&records),
        Path::new(&adjudications),
        &adjudications,
    )?;
    write_staging(&out, &staged)?;
    let index = serde_json::from_str::<Value>(
        &fs::read_to_string(out.join("index.json")).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    ensure(
        index["confirmed_over_credits"].as_u64() == Some(1)
            && index["proposal_files"].as_array().is_some_and(|files| {
                files.len() == 1 && files[0] == json!("feedback-gap-row.proposal.json")
            }),
        "the confirmed over-credit must stage exactly one named proposal",
    )?;
    let proposal = serde_json::from_str::<Value>(
        &fs::read_to_string(out.join("feedback-gap-row.proposal.json"))
            .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    ensure(
        proposal["confirmed_over_credit"]["verdict"] == "exposed"
            && proposal["confirmed_over_credit"]["false_exposed"] == "true"
            && proposal["confirmed_over_credit"]["expected_direction"] == "should_gap"
            && proposal["confirmed_over_credit"]["roles"]
                .as_array()
                .is_some_and(|roles| roles.len() == 2),
        "the proposal must carry the confirmed over-credit facts",
    )?;
    ensure(
        proposal["provenance"]["panel_digest"].as_str().is_some()
            && proposal["provenance"]["adjudication_record"] == "feedback-gap-row.json"
            && proposal["provenance"]["replay_record"] == "feedback-gap-row.json"
            && proposal["provenance"]["diff_path"].as_str().is_some(),
        "the proposal must carry the full provenance chain",
    )?;
    ensure(
        proposal["promotion_recipe"]["step_1"]
            .as_str()
            .is_some_and(|step| step.contains("REGULAR fixture"))
            && proposal["promotion_recipe"]["step_2"]
                .as_str()
                .is_some_and(|step| step.contains("must_not_promote")),
        "the proposal must carry the promotion recipe",
    )?;
    ensure(
        corpus_digest()? == corpus_before,
        "the corpus must be untouched by feedback staging",
    )?;

    // An incomplete replay (round-1 review) can never confirm anything: the
    // same case downgraded to not_run drops out of feedback entirely.
    let path = Path::new(&records).join("feedback-gap-row.json");
    let mut value = serde_json::from_str::<Value>(
        &fs::read_to_string(&path).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    value["outcome"]["kind"] = json!("not_run");
    value["comparison"] = json!({"kind": "comparison_unavailable", "outcome": "not_run"});
    fs::write(
        &path,
        serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let staged = derive_feedback(
        fixture.root.as_path(),
        &ref_strs,
        Path::new(&records),
        Path::new(&adjudications),
        &adjudications,
    )?;
    ensure(
        staged.confirmed_over_credits == 0,
        "an incomplete replay must not confirm an over-credit",
    )?;
    Ok(())
}

/// FIX #3680 acceptance: staging is deterministic — a second run's `--check`
/// verifies byte-identity (modulo the disclosed generated_at field), and a
/// mutated staging file fails `--check` with a named error.
#[test]
fn feedback_check_verifies_staging_and_detects_drift() -> Result<(), String> {
    let (fixture, refs, adjudications, records) = seeded_pipeline("feedback-check")?;
    let out = fixture.root.join("feedback");
    let ref_strs = refs.iter().map(String::as_str).collect::<Vec<_>>();
    let derive = || {
        derive_feedback(
            fixture.root.as_path(),
            &ref_strs,
            Path::new(&records),
            Path::new(&adjudications),
            &adjudications,
        )
    };
    let staged = derive()?;
    write_staging(&out, &staged)?;
    // A second derivation matches the staging byte-for-byte (modulo the
    // disclosed generated_at field, which verify_staged strips).
    verify_staged(&out, &derive()?)?;
    // Drift is detected: rewriting a proposal breaks the byte comparison.
    let proposal_path = out.join("feedback-gap-row.proposal.json");
    let prior = fs::read_to_string(&proposal_path).map_err(|error| error.to_string())?;
    fs::write(
        &proposal_path,
        prior.replace("should_gap", "should_stay_quiet"),
    )
    .map_err(|error| error.to_string())?;
    let drifted = verify_staged(&out, &derive()?)
        .err()
        .ok_or("feedback test failed: drifted staging must fail --check")?;
    ensure(
        drifted.contains("does not match a fresh derivation"),
        &format!("the drift must be named, got: {drifted}"),
    )?;
    // A stale proposal whose confirmation disappeared must also fail: the
    // entry set on disk is compared against the fresh derivation.
    let stale_proposal = out.join("feedback-quiet-row.proposal.json");
    fs::write(&stale_proposal, "{}").map_err(|error| error.to_string())?;
    let extra = verify_staged(&out, &derive()?)
        .err()
        .ok_or("feedback test failed: an extra staging file must fail --check")?;
    ensure(
        extra.contains("unexpected file set"),
        &format!("the extra file must be named, got: {extra}"),
    )?;
    Ok(())
}

/// Non-over-credit confirmations stay out of feedback: a quiet-row
/// false_actionable confirmation is a different error family, and a
/// not-current replay can never be staged as current evidence.
#[test]
fn feedback_ignores_non_over_credit_confirmations() -> Result<(), String> {
    let (fixture, refs, adjudications, records) = seeded_pipeline("feedback-quiet")?;
    // The quiet row judged `exposed` is only reachable with false_actionable
    // decided — that family feeds no proposal.
    let ref_strs = refs.iter().map(String::as_str).collect::<Vec<_>>();
    for (role, identity) in [
        ("human_operator", "alice"),
        ("second_human_reviewer", "bob"),
    ] {
        adjudicate_case_at(
            fixture.root.as_path(),
            &ref_strs,
            &adjudications,
            &records,
            &judge_request(
                "feedback-quiet-row",
                role,
                identity,
                "exposed",
                None,
                Some(true),
            ),
        )?;
    }
    let out = fixture.root.join("feedback");
    let staged = derive_feedback(
        fixture.root.as_path(),
        &ref_strs,
        Path::new(&records),
        Path::new(&adjudications),
        &adjudications,
    )?;
    write_staging(&out, &staged)?;
    let index = serde_json::from_str::<Value>(
        &fs::read_to_string(out.join("index.json")).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    ensure(
        index["confirmed_over_credits"].as_u64() == Some(1),
        "the quiet-row false_actionable confirmation must not stage a proposal",
    )?;
    let gap_proposal = out.join("feedback-gap-row.proposal.json");
    ensure(
        gap_proposal.is_file() && !out.join("feedback-quiet-row.proposal.json").exists(),
        "the fully-adjudicated quiet-row false_actionable case must stay excluded while the gap over-credit still proposes",
    )?;
    Ok(())
}

fn ensure(condition: bool, message: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(format!("feedback test failed: {message}"))
    }
}
