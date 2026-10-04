//! Behaviour of the step summary the generated workflow prints.
//!
//! These replace the text pins on the retired shell step. The complete
//! fixture carries every artifact the summary reads; the empty root carries
//! none, so between them each block's at-a-glance and not-generated forms
//! render.

use super::jq::{Doc, RepoRelative};
use super::{CiSummaryInput, render_ci_summary};
use crate::agent::loop_commands::WORKFLOW_AGENT_REVIEW_SUMMARY_ARTIFACT;
use crate::app::agent_review_summary::NO_RECEIPT_BEFORE_REPAIR;
use crate::output::first_pr::{
    MANUAL_RECEIPT_LABEL, MANUAL_VERIFY_LABEL, RECEIPT_AFTER_VERIFY_LABEL,
    REPAIR_AFTER_PHASE_LABEL, REPAIR_AFTER_PHASE_STEP, VERIFY_AFTER_EDIT_LABEL,
};
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const REPAIR: &str = "ripr agent repair --root . --seam-id s1 --phase before";

/// Every heading the summary can print, in print order.
const SECTIONS: &[&str] = &[
    "## RIPR advisory summary",
    "### Start here",
    "#### First-run status",
    "### Language preview grouping",
    "### PR review summary",
    "#### PR review at a glance",
    "### Recommended next test",
    "#### Recommended next test at a glance",
    "### Top recommendation",
    "### Agent review packet",
    "### Artifact packet",
    "### Uploaded review artifacts",
    "#### Uploaded artifacts at a glance",
    "### PR evidence ledger",
    "#### PR movement at a glance",
    "### Test-oracle assistant proof",
    "#### Assistant proof at a glance",
    "### Agent proof status",
    "#### Agent proof status at a glance",
    "### Policy readiness",
    "#### Policy readiness at a glance",
    "### Policy operations",
    "#### Policy operations at a glance",
    "### Policy history",
    "#### Policy history at a glance",
    "### Policy promotion packets",
    "### Preview promotion packets",
    "### Waiver aging",
    "#### Waiver aging at a glance",
    "### Suppression health",
    "#### Suppression health at a glance",
    "### Gate decision",
    "#### Gate decision at a glance",
    "### Baseline debt delta",
    "#### Baseline debt movement",
    "### RIPR Zero status",
    "#### RIPR Zero at a glance",
    "### SARIF and badge status",
    "### PR guidance annotations",
    "### PR inline comments",
    "### Known limits",
];

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(label: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let root = std::env::temp_dir().join(format!(
            "ripr-ci-summary-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root)?;
        Ok(Self {
            root: fs::canonicalize(root)?,
        })
    }

    fn write(&self, path: &str, text: &str) -> TestResult {
        let path = self.root.join(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, text)?;
        Ok(())
    }

    fn json(&self, path: &str, value: &Value) -> TestResult {
        self.write(path, &value.to_string())
    }

    fn render(&self, languages: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
        self.render_with(languages, "true", "", "")
    }

    fn render_with(
        &self,
        languages: &[&str],
        upload_sarif: &str,
        gate_baseline: &str,
        comment_mode: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let input = CiSummaryInput {
            root: self.root.clone(),
            base_ref: "trunk".to_string(),
            upload_sarif: upload_sarif == "true",
            gate_baseline: !gate_baseline.is_empty(),
            comment_mode: comment_mode.to_string(),
            configured_languages: languages.iter().map(|name| (*name).to_string()).collect(),
        };
        Ok(String::from_utf8(render_ci_summary(&input))?)
    }

    /// Every artifact the summary reads, with a carried repair start.
    fn complete(label: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let fixture = Self::new(label)?;
        let r = "target/ripr/reports";
        fixture.json(
            &format!("{r}/start-here.json"),
            &json!({
                "status": "ready",
                "selected": {
                    "state": "top_gap",
                    "canonical_gap_id": "gap:1",
                    "language": "rust",
                    "language_status": "stable",
                    "kind": "top_gap",
                    "changed_behavior": "threshold moved",
                    "current_evidence_strength": "weak",
                    "missing_discriminator": "boundary at 100",
                    "repair": {"route": "add_boundary_test", "target_file": "tests/a.rs", "related_test": "a::b"},
                    "focused_proof_intent": "assert 100 is rejected",
                    "static_limit_kind": "macro",
                    "static_limit_detail": "opaque",
                    "verify_command": "ripr agent verify --root .",
                    "receipt_command": "ripr agent receipt --root .",
                    "receipt_path": "target/ripr/receipts/r.json",
                    "repair_command": REPAIR,
                },
                "warnings": ["w"],
            }),
        )?;
        fixture.write(&format!("{r}/start-here.md"), "# Start here\n\nbody\n")?;
        fixture.json(
            &format!("{r}/first-useful-action.json"),
            &json!({
                "status": "ready",
                "action_kind": "add_test",
                "title": "Add a boundary test",
                "why": "weak oracle",
                "selected": {"seam_id": "s1"},
                "target": {"file": "tests/a.rs", "related_test": "a::b"},
                "commands": {"repair": REPAIR, "verify": "ripr agent verify --root .", "receipt": "ripr agent receipt --root ."},
            }),
        )?;
        fixture.write(&format!("{r}/first-useful-action.md"), "# First action\n")?;
        fixture.json(
            &format!("{r}/pr-review-front-panel.json"),
            &json!({
                "status": "ready",
                "summary": {"headline": "one gap", "top_issue_state": "open"},
                "top_issue": {"path": "src/a.rs", "line": 4, "classification": "weakly_exposed", "repair_command": REPAIR, "agent_command": REPAIR},
                "policy": {"mode": "visible-only", "decision": "pass"},
            }),
        )?;
        fixture.write(&format!("{r}/pr-review-front-panel.md"), "# Front panel\n")?;
        fixture.write("target/ripr/pilot/pilot-summary.md", "# Pilot\n")?;
        fixture.json(
            WORKFLOW_AGENT_REVIEW_SUMMARY_ARTIFACT,
            &json!({"static_movement": {"state": "missing_artifact"}}),
        )?;
        fixture.json(
            &format!("{r}/index.json"),
            &json!({
                "status": "ok",
                "summary": {"entries": 3, "available": 2, "missing_expected": 1, "start_here": "target/ripr/reports/start-here.md", "gate_authority": "not_configured"},
                "missing_expected": [{"label": "gate decision"}],
                "warnings": [{"kind": "stale"}],
            }),
        )?;
        fixture.write(&format!("{r}/index.md"), "# Index\n")?;
        fixture.json(
            &format!("{r}/pr-evidence-ledger.json"),
            &json!({
                "status": "ok",
                "movement": {"count_source": "not_measured", "acknowledged": 1},
                "top_repair_route": {"path": "src/a.rs", "line": 4, "missing_discriminator": "boundary", "repair_command": REPAIR, "agent_command": "ripr agent brief"},
            }),
        )?;
        fixture.write(&format!("{r}/pr-evidence-ledger.md"), "# Ledger\n")?;
        fixture.json(
            &format!("{r}/test-oracle-assistant-proof.json"),
            &json!({"status": "ok", "seam": {"path": "src/a.rs", "line": 4}}),
        )?;
        fixture.write(&format!("{r}/test-oracle-assistant-proof.md"), "# Proof\n")?;
        fixture.json(
            &format!("{r}/assistant-loop-health.json"),
            &json!({
                "status": "ok",
                "warning_summary": [{"kind": "stale", "count": 2}],
                "repair_queue": [{"repair_kind": "add_test"}],
            }),
        )?;
        fixture.write(&format!("{r}/assistant-loop-health.md"), "# Health\n")?;
        for name in ["policy-readiness", "policy-operations", "policy-history"] {
            fixture.json(&format!("{r}/{name}.json"), &json!({"status": "ok"}))?;
            fixture.write(&format!("{r}/{name}.md"), &format!("# {name}\n"))?;
        }
        fixture.json(
            &format!("{r}/policy-promotion-acknowledgeable.json"),
            &json!({"target_mode": "acknowledgeable", "allowed_now": false, "why_or_why_not": "ceiling"}),
        )?;
        fixture.write(
            &format!("{r}/policy-promotion-acknowledgeable.md"),
            "# Promotion\n",
        )?;
        fixture.json(
            &format!("{r}/preview-promotion-typescript-boundary-gap.json"),
            &json!({"language": "typescript", "candidate_class": "boundary_gap", "allowed_now": false}),
        )?;
        fixture.write(
            &format!("{r}/preview-promotion-typescript-boundary-gap.md"),
            "# Preview\n",
        )?;
        for name in [
            "waiver-aging",
            "suppression-health",
            "baseline-debt-delta",
            "ripr-zero-status",
        ] {
            fixture.json(&format!("{r}/{name}.json"), &json!({"status": "ok"}))?;
            fixture.write(&format!("{r}/{name}.md"), &format!("# {name}\n"))?;
        }
        fixture.json(
            &format!("{r}/gate-decision.json"),
            &json!({
                "status": "fail",
                "mode": "calibrated-gate",
                "summary": {"blocking": 3},
                "inputs": {"labels": ["ripr-waive", "docs"]},
                "policy": {"acknowledgement_labels": ["ripr-waive"]},
                "decisions": [
                    {"decision": "blocking", "gate_reason": "new gap", "evidence": {"mutation_calibration": {"confidence_effect": "lowered"}}},
                    {"decision": "acknowledged", "policy": {"acknowledgement_label": "ripr-waive"}},
                    {"decision": "blocking", "gate_reason": "second"},
                    {"decision": "blocking", "gate_reason": "third", "evidence": {"mutation_calibration": {"confidence_effect": "lowered"}}},
                ],
            }),
        )?;
        fixture.write(&format!("{r}/gate-decision.md"), "# Gate\n")?;
        fixture.json(
            "target/ripr/review/comments.json",
            &json!({
                "summary": {"comments": 2, "summary_only": 1},
                "comments": [{"language": "typescript", "language_status": "preview", "classification": "weakly_exposed", "preview_actionability": {"gap_state": "actionable", "repair_packet_ready": true}}],
            }),
        )?;
        fixture.json(
            "target/ripr/review/comment-publish-plan.json",
            &json!({"status": "planned", "summary": {"publishable": 1, "safe_to_publish": true}}),
        )?;
        fixture.write("target/ripr/review/comment-publish-plan.md", "# Plan\n")?;
        for sarif in [
            "ripr-findings.sarif",
            "ripr-seams.sarif",
            "repo-ripr-badge.json",
        ] {
            fixture.write(&format!("{r}/{sarif}"), "{}")?;
        }
        Ok(fixture)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _cleanup = fs::remove_dir_all(&self.root);
    }
}

/// The block under `heading`, up to the next `### ` heading.
fn block<'a>(summary: &'a str, heading: &str) -> &'a str {
    summary
        .split(&format!("{heading}\n"))
        .nth(1)
        .and_then(|rest| rest.split("\n### ").next())
        .unwrap_or_default()
}

#[test]
fn complete_artifacts_render_every_section_in_order() -> TestResult {
    let fixture = Fixture::complete("sections")?;
    let summary = fixture.render(&["rust", "typescript"])?;
    let mut from = 0;
    for heading in SECTIONS {
        let found = summary
            .get(from..)
            .and_then(|rest| rest.find(&format!("{heading}\n")))
            .ok_or_else(|| format!("{heading:?} is missing or out of order:\n{summary}"))?;
        from += found + heading.len();
    }
    assert!(!summary.contains("@RIPR_"), "unsubstituted placeholder");
    assert!(summary.ends_with("- No runtime mutation execution is performed by this workflow.\n"));
    Ok(())
}

#[test]
fn empty_root_names_every_missing_artifact_and_its_regeneration() -> TestResult {
    let fixture = Fixture::new("empty")?;
    let summary = fixture.render(&[])?;
    for line in [
        "- Start-here artifact: not generated yet; inspect uploaded artifacts and job logs.",
        "- Status: `missing_start_here`",
        "- State: `missing_artifact`",
        "- Safe next action: run `ripr first-pr --root . --base origin/trunk --head HEAD --gap-ledger target/ripr/reports/gap-decision-ledger.json --first-action target/ripr/reports/first-useful-action.json --review-comments target/ripr/review/comments.json --agent-packet target/ripr/workflow/agent-packet.json --gate-decision target/ripr/reports/gate-decision.json --receipts-dir target/ripr/receipts --out-dir target/ripr/reports`.",
        "PR review summary was not generated.",
        "Safe next action: run `ripr pr-review front-panel --root . --pr-guidance target/ripr/review/comments.json --out target/ripr/reports/pr-review-front-panel.json --out-md target/ripr/reports/pr-review-front-panel.md` after attaching at least one explicit input.",
        "Recommended next test was not generated.",
        "Pilot summary was not generated.",
        "Agent review summary was not generated.",
        "- PR test guidance report: not generated yet",
        "Regenerate command: `ripr reports index --root . --reports-dir target/ripr/reports --review-dir target/ripr/review --receipts-dir target/ripr/receipts --workflow-dir target/ripr/workflow --agent-dir target/ripr/agent --pilot-dir target/ripr/pilot --ci-dir target/ci --out target/ripr/reports/index.json --out-md target/ripr/reports/index.md`.",
        "PR evidence ledger was not run.",
        "Policy readiness was not generated.",
        "Policy operations was not generated.",
        "Policy history was not generated.",
        "Policy promotion packets were not generated.",
        "Preview promotion packets were not generated.",
        "Waiver aging was not run.",
        "Suppression health was not generated.",
        "Gate decision was not run. Set `RIPR_GATE_MODE`",
        "Baseline debt delta was not run. Set `RIPR_GATE_BASELINE`",
        "RIPR Zero status was not run.",
        "- Diff SARIF: missing or skipped",
        "- Badge JSON: missing or skipped",
        "No PR test guidance report was generated.",
        "- Mode: `off`",
        "- Inline comments are disabled by default.",
    ] {
        assert!(summary.contains(line), "missing {line:?}:\n{summary}");
    }
    // Optional blocks stay out entirely when nothing feeds them.
    for heading in [
        "### Language preview grouping",
        "### Test-oracle assistant proof",
        "### Agent proof status",
    ] {
        assert!(
            !summary.contains(heading),
            "{heading} rendered with no input"
        );
    }
    Ok(())
}

/// #3906 (F60-3, F60-14): a carried repair start leads each block with its
/// after phase, and the low-level verify and receipt become the manual
/// alternative.
#[test]
fn a_carried_repair_start_leads_every_block_that_has_one() -> TestResult {
    let fixture = Fixture::complete("repair-lead")?;
    let summary = fixture.render(&[])?;
    let after = format!("- {REPAIR_AFTER_PHASE_LABEL}: {REPAIR_AFTER_PHASE_STEP}\n");
    for (heading, label) in [
        ("#### First-run status", "Start repair"),
        ("#### PR review at a glance", "Repair start"),
        ("#### Recommended next test at a glance", "Repair start"),
        ("#### PR movement at a glance", "Repair start"),
    ] {
        let lead = format!("{heading}\n- {label}: `{REPAIR}`\n{after}");
        assert!(
            summary.contains(&lead),
            "{heading} must lead with the repair start:\n{summary}"
        );
    }
    let first_run = block(&summary, "#### First-run status");
    assert!(first_run.contains(&format!(
        "- {MANUAL_VERIFY_LABEL}: `ripr agent verify --root .`"
    )));
    assert!(first_run.contains(&format!(
        "- {MANUAL_RECEIPT_LABEL}: `ripr agent receipt --root .`"
    )));
    // The front panel's agent handoff repeats the repair start, so it is
    // not printed twice; the ledger's differs and is.
    assert!(!block(&summary, "### PR review summary").contains("- Agent handoff:"));
    assert!(
        block(&summary, "### PR evidence ledger").contains("- Agent command: `ripr agent brief`")
    );

    // Without a repair start the pair runs after the test edit.
    fixture.json(
        "target/ripr/reports/start-here.json",
        &json!({"selected": {"verify_command": "v", "receipt_command": "r"}}),
    )?;
    let summary = fixture.render(&[])?;
    let first_run = block(&summary, "#### First-run status");
    assert!(
        first_run.starts_with("- Status: `unknown`\n"),
        "{first_run}"
    );
    assert!(first_run.contains(&format!("- {VERIFY_AFTER_EDIT_LABEL}: `v`")));
    assert!(first_run.contains(&format!("- {RECEIPT_AFTER_VERIFY_LABEL}: `r`")));
    assert!(first_run.contains("- Safe next action command: `none`"));
    Ok(())
}

/// #3906 (N5): before any test edit the review packet names the missing
/// receipt as expected and leads with the repair start, not the post-edit
/// loop; once a receipt can exist the full packet is collapsed instead.
#[test]
fn the_agent_review_packet_leads_with_the_repair_before_any_edit() -> TestResult {
    let fixture = Fixture::complete("review-packet")?;
    let summary = fixture.render(&[])?;
    let packet = block(&summary, "### Agent review packet");
    assert!(
        packet.starts_with(&format!(
            "- Receipt: {NO_RECEIPT_BEFORE_REPAIR}\n- Start repair: `{REPAIR}`\n- {REPAIR_AFTER_PHASE_LABEL}: {REPAIR_AFTER_PHASE_STEP}\n- Full packet: `target/ripr/workflow/agent-review-summary.md` (workflow artifact).\n"
        )),
        "{packet}"
    );

    fixture.json(
        WORKFLOW_AGENT_REVIEW_SUMMARY_ARTIFACT,
        &json!({"static_movement": {"state": "improved"}}),
    )?;
    fixture.write("target/ripr/workflow/agent-review-summary.md", "# Review\n")?;
    let summary = fixture.render(&[])?;
    assert!(block(&summary, "### Agent review packet").starts_with(
        "<details><summary>Full report: target/ripr/workflow/agent-review-summary.md</summary>\n\n# Review\n\n</details>\n"
    ));
    Ok(())
}

/// Field values cannot break out of their code span or line.
#[test]
fn field_values_are_escaped_into_one_inline_code_span() -> TestResult {
    let fixture = Fixture::new("inline")?;
    fixture.json(
        "target/ripr/reports/first-useful-action.json",
        &json!({"title": "a `b`\nc\r\n", "why": 7, "action_kind": ["x"]}),
    )?;
    let summary = fixture.render(&[])?;
    let first_run = block(&summary, "#### First-run status");
    // Trailing newlines drop, inner ones become spaces, backticks escape.
    assert!(first_run.contains("- Title: `a \\`b\\` c `"), "{first_run}");
    assert!(first_run.contains("- Why: `7`"));
    assert!(
        first_run.contains("- Safe next action: `[   \"x\" ]`"),
        "{first_run}"
    );
    Ok(())
}

/// A malformed artifact, a whitespace-only one, and a field whose type
/// breaks the read each keep the fallback the shell step printed.
#[test]
fn malformed_and_mistyped_artifacts_keep_their_fallbacks() -> TestResult {
    let fixture = Fixture::new("fallbacks")?;
    fixture.write("target/ripr/reports/gate-decision.json", "{not json")?;
    fixture.write("target/ripr/reports/waiver-aging.json", " \n")?;
    fixture.json(
        "target/ripr/reports/ripr-zero-status.json",
        &json!({"ripr_zero": "flat", "top_debt_areas": {"area": "x"}, "repair_routes": []}),
    )?;
    fixture.json(
        "target/ripr/reports/index.json",
        &json!({"missing_expected": "none", "warnings": [{"kind": "a"}, 3]}),
    )?;
    let summary = fixture.render(&[])?;
    let gate = block(&summary, "### Gate decision");
    assert!(gate.contains("- Mode: `unknown`"));
    assert!(gate.contains("- Counts: blocking=`0`, acknowledged=`0`"));
    assert!(gate.contains("- Active PR labels: `unknown`"));
    // jq printed nothing for an empty input and succeeded.
    assert!(block(&summary, "### Waiver aging").contains("- Status: ``"));
    let zero = block(&summary, "### RIPR Zero status");
    assert!(zero.contains("- State: `unknown`"), "{zero}");
    assert!(zero.contains("- Visible unresolved: `0`"));
    assert!(zero.contains("- Top debt area: `unknown`"));
    assert!(zero.contains("- Top repair route: `none`"));
    let index = block(&summary, "### Uploaded review artifacts");
    assert!(index.contains("- Missing expected: `none`"), "{index}");
    assert!(index.contains("- Warning kinds: `unknown`"), "{index}");
    Ok(())
}

#[test]
fn the_gate_block_summarises_labels_waivers_effects_and_reasons() -> TestResult {
    let fixture = Fixture::complete("gate")?;
    let summary = fixture.render(&[])?;
    let gate = block(&summary, "### Gate decision");
    for line in [
        "- Mode: `calibrated-gate`",
        "- Counts: blocking=`3`, acknowledged=`0`, advisory=`0`, suppressed=`0`, not_applicable=`0`, unknown_confidence=`0`",
        "- Active PR labels: `ripr-waive, docs`",
        "- Acknowledgement labels: `ripr-waive`",
        "- Applied waiver label: `ripr-waive`",
        "- Baseline artifact: `not supplied`",
        "- Recommendation calibration: `not supplied` (effects: none)",
        "- Mutation calibration: `not supplied` (effects: lowered)",
        "- Blocking reason (`3`): `new gap (+2 more, see gate-decision.md)`",
    ] {
        assert!(gate.contains(line), "missing {line:?}:\n{gate}");
    }
    Ok(())
}

/// F60-4: counts with no baseline delta or RIPR Zero status behind them
/// were never measured, so their zeros are not printed.
#[test]
fn unmeasured_ledger_counts_are_not_printed_as_zeros() -> TestResult {
    let fixture = Fixture::complete("ledger")?;
    let ledger = fixture.render(&[])?;
    let ledger = block(&ledger, "### PR evidence ledger");
    assert!(ledger.contains("- Counts: gap counts not measured (no baseline debt delta or RIPR Zero status); acknowledged=`1`, suppressed=`0`, blocking_candidates=`0`"));
    assert!(ledger.contains("- Top repair route: `src/a.rs:4 boundary`"));

    fixture.json(
        "target/ripr/reports/pr-evidence-ledger.json",
        &json!({"movement": {"visible_unresolved": 2}}),
    )?;
    let ledger = fixture.render(&[])?;
    assert!(block(&ledger, "### PR evidence ledger").contains("visible_unresolved=`2`"));
    Ok(())
}

/// #3999: commands the summary prints for another machine name the
/// repository root, not this runner's checkout, wherever the checkout is a
/// whole path token; reports that are included raw stay as written.
#[test]
fn checkout_paths_become_the_repository_root_in_copied_commands() -> TestResult {
    let fixture = Fixture::new("relative")?;
    let checkout = fixture.root.to_string_lossy().into_owned();
    let verify =
        format!("ripr agent verify --root '{checkout}' --json > '{checkout}/target/v.json'");
    let sibling = format!("{checkout}-other/notes.md");
    fixture.json(
        "target/ripr/reports/pr-review-front-panel.json",
        &json!({"top_issue": {"verify_command": verify}}),
    )?;
    fixture.write(
        "target/ripr/reports/pr-review-front-panel.md",
        &format!("- Verify: `{verify}`\n- Notes: `{sibling}`"),
    )?;
    fixture.write(
        "target/ripr/pilot/pilot-summary.md",
        &format!("`{verify}`\n"),
    )?;
    let summary = fixture.render(&[])?;
    let relative = "ripr agent verify --root '.' --json > './target/v.json'";
    let panel = block(&summary, "### PR review summary");
    assert_eq!(panel.matches(relative).count(), 2, "{panel}");
    // A sibling that only shares the prefix stays, and the report's last
    // line gains the newline the shell's line rewrite gave it.
    assert!(
        panel.contains(&format!("- Notes: `{sibling}`\n\n</details>")),
        "{panel}"
    );
    assert!(block(&summary, "### Top recommendation").contains(&verify));
    Ok(())
}

#[test]
fn repo_relative_rewrites_only_whole_path_tokens() {
    let relative = RepoRelative {
        physical: b"/w/repo".to_vec(),
        logical: b"/link/repo".to_vec(),
    };
    assert_eq!(
        relative.value("cd /w/repo && ls /w/repo/src"),
        "cd . && ls ./src"
    );
    assert_eq!(relative.value("x=/link/repo:(/w/repo)"), "x=.:(.)");
    assert_eq!(
        relative.value("/w/repository /w/repo2 a/w/repo"),
        "/w/repository /w/repo2 a/w/repo"
    );
    assert_eq!(relative.value("one /w/repo\ntwo\n\n"), "one .\ntwo");
    assert_eq!(relative.file(b""), b"");
    assert_eq!(relative.file(b"\n"), b"\n");
    assert_eq!(relative.file(b"a /w/repo\nb"), b"a .\nb\n");
}

#[test]
fn doc_reads_follow_jq_exit_status() {
    let query = |value: &Value| super::jq::alt(value, &["a"], Value::from("d"));
    assert_eq!(Doc::parse(b"").text(query, "fail"), "");
    assert_eq!(Doc::parse(b"{\"a\":1}").text(query, "fail"), "1");
    assert_eq!(Doc::parse(b"{bad").text(query, "fail"), "fail");
    // jq keeps going after a failed input and exits on the last one.
    assert_eq!(Doc::parse(b"\"s\" {\"a\":1}").text(query, "fail"), "1");
    assert_eq!(
        Doc::parse(b"{\"a\":1} \"s\"").text(query, "fail"),
        "1\nfail"
    );
    assert_eq!(Doc::parse(b"{\"a\":1} {bad").text(query, "fail"), "1\nfail");
    assert_eq!(Doc::parse(b"\"s\"").text(query, ""), "");
}

#[test]
fn preview_languages_group_only_when_configured() -> TestResult {
    let fixture = Fixture::complete("languages")?;
    let rust_only = fixture.render(&["rust"])?;
    assert!(!rust_only.contains("### Language preview grouping"));

    let summary = fixture.render(&["rust", "typescript"])?;
    let grouping = summary
        .find("### Language preview grouping\n")
        .ok_or("no grouping block")?;
    let review = summary
        .find("### PR review summary\n")
        .ok_or("no review block")?;
    assert!(
        grouping < review,
        "grouping must precede the PR review summary"
    );
    let grouping = block(&summary, "### Language preview grouping");
    assert!(grouping.contains("- Configured languages: `rust,typescript`"));
    assert!(grouping.contains("- Grouped preview evidence languages: `javascript typescript`"));
    assert!(grouping.contains("- `javascript`: configured preview/advisory; no language findings were emitted in this run; gate_impact=`none`."));
    assert!(grouping.contains("- `typescript`: artifact_entries=`1`, preview_entries=`1`, missing_preview_status=`0`, static_limit_entries=`0`, classifications=`weakly_exposed=1`, static_limit_kinds=`none`, actionability_states=`actionable=1`, actionability_categories=`none`, repair_packet_ready=`1`, gate_impact=`none`"), "{grouping}");

    // A malformed input fails every count, as `jq -s` did.
    fixture.write("target/ripr/reports/gate-decision.json", "{bad")?;
    let summary = fixture.render(&["python"])?;
    let grouping = block(&summary, "### Language preview grouping");
    assert!(
        grouping.contains("- `python`: configured preview/advisory"),
        "{grouping}"
    );
    Ok(())
}

#[test]
fn workflow_settings_choose_the_sarif_baseline_and_comment_lines() -> TestResult {
    let fixture = Fixture::complete("settings")?;
    let enabled = fixture.render_with(&[], "true", "", "")?;
    let sarif = block(&enabled, "### SARIF and badge status");
    assert!(sarif.starts_with("- Diff SARIF: generated\n- Repo seam SARIF: generated\n- Badge JSON: generated\n- Badge Shields JSON: missing or skipped\n"), "{sarif}");
    let disabled = fixture.render_with(&[], "false", "", "plan`x")?;
    assert!(
        block(&disabled, "### SARIF and badge status")
            .starts_with("- SARIF upload: disabled by `RIPR_UPLOAD_SARIF`\n")
    );
    let comments = block(&disabled, "### PR inline comments");
    assert!(comments.starts_with("- Mode: `plan\\`x`\n- Status: `planned`\n- Counts: publishable=`1`, skipped=`0`, blocked=`0`\n- Safe to publish: `true`\n"), "{comments}");

    let empty = Fixture::new("settings-empty")?;
    let without = empty.render_with(&[], "", "", "")?;
    assert!(without.contains("Baseline debt delta was not run."));
    let with = empty.render_with(&[], "", ".ripr/gate-baseline.json", "")?;
    assert!(
        with.contains(
            "Baseline debt delta was not generated. Check that `RIPR_GATE_MODE` produced"
        )
    );
    Ok(())
}

#[test]
fn preview_promotion_packets_follow_the_shell_glob() -> TestResult {
    let fixture = Fixture::new("preview-glob")?;
    let r = "target/ripr/reports";
    fixture.json(
        &format!("{r}/preview-promotion-python-b.json"),
        &json!({"language": "python", "candidate_class": "b"}),
    )?;
    fixture.json(
        &format!("{r}/preview-promotion-python-a.json"),
        &json!({"language": "python", "candidate_class": "a"}),
    )?;
    // No second dash after the prefix: outside `preview-promotion-*-*.json`.
    fixture.json(
        &format!("{r}/preview-promotion-python.json"),
        &json!({"language": "skip"}),
    )?;
    fixture.write(&format!("{r}/preview-promotion-python-a.md"), "# A")?;
    let summary = fixture.render(&[])?;
    let packets = block(&summary, "### Preview promotion packets");
    let a = packets.find("- `python`/`a`").ok_or("no a")?;
    let b = packets.find("- `python`/`b`").ok_or("no b")?;
    assert!(a < b, "{packets}");
    assert!(!packets.contains("skip"));
    assert!(packets.contains("<details><summary>Full report: target/ripr/reports/preview-promotion-python-a.md</summary>\n\n# A\n</details>"), "{packets}");
    Ok(())
}
