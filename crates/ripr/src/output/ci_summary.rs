//! `ripr reports ci-summary`: the generated workflow's step summary.
//!
//! The `Add RIPR advisory summary` step of `ripr init --ci github` used to
//! be 1,266 lines of shell and jq that every adopting repository committed
//! and nobody could read. This renderer prints the same Markdown from the
//! same artifacts, so the step is one command. Reads only; it never runs
//! analysis, edits source, or decides pass/fail.
//!
//! Byte parity with the retired shell is the contract: field fallbacks,
//! `markdown_inline` escaping, `repo_relative` rewriting, and raw report
//! inclusion follow [`jq`]'s model of what the step printed.

mod jq;
#[cfg(test)]
mod tests;

use crate::agent::loop_commands::{
    PILOT_BEFORE_SNAPSHOT_ARTIFACT, WORKFLOW_AGENT_PACKET_ARTIFACT,
    WORKFLOW_AGENT_REVIEW_SUMMARY_ARTIFACT, WORKFLOW_AGENT_REVIEW_SUMMARY_MARKDOWN_ARTIFACT,
};
use crate::app::agent_review_summary::NO_RECEIPT_BEFORE_REPAIR;
use crate::output::first_pr::{
    MANUAL_RECEIPT_LABEL, MANUAL_VERIFY_LABEL, RECEIPT_AFTER_VERIFY_LABEL,
    REPAIR_AFTER_PHASE_LABEL, REPAIR_AFTER_PHASE_STEP, VERIFY_AFTER_EDIT_LABEL,
};
use jq::{Doc, Fail, Q, RepoRelative, captured, inline};
use serde_json::Value;
use std::fs;
use std::path::{Component, Path, PathBuf};

/// A per-object predicate that can fail the way a jq filter does.
type ObjectTest<'a> = dyn Fn(&serde_json::Map<String, Value>) -> Result<bool, Fail> + 'a;

const REPORTS: &str = "target/ripr/reports";
const START_HERE_JSON: &str = "target/ripr/reports/start-here.json";
const START_HERE_MD: &str = "target/ripr/reports/start-here.md";
const INDEX_JSON: &str = "target/ripr/reports/index.json";
const INDEX_MD: &str = "target/ripr/reports/index.md";
const FRONT_PANEL_JSON: &str = "target/ripr/reports/pr-review-front-panel.json";
const FRONT_PANEL_MD: &str = "target/ripr/reports/pr-review-front-panel.md";
const FIRST_ACTION_JSON: &str = "target/ripr/reports/first-useful-action.json";
const FIRST_ACTION_MD: &str = "target/ripr/reports/first-useful-action.md";
const PILOT_SUMMARY_MD: &str = "target/ripr/pilot/pilot-summary.md";
const LEDGER_JSON: &str = "target/ripr/reports/pr-evidence-ledger.json";
const LEDGER_MD: &str = "target/ripr/reports/pr-evidence-ledger.md";
const COMMENTS_JSON: &str = "target/ripr/review/comments.json";
const GATE_JSON: &str = "target/ripr/reports/gate-decision.json";
const GATE_MD: &str = "target/ripr/reports/gate-decision.md";
const DELTA_JSON: &str = "target/ripr/reports/baseline-debt-delta.json";
const DELTA_MD: &str = "target/ripr/reports/baseline-debt-delta.md";
const ZERO_JSON: &str = "target/ripr/reports/ripr-zero-status.json";
const ZERO_MD: &str = "target/ripr/reports/ripr-zero-status.md";
const POLICY_PROMOTION_MODES: [&str; 4] = [
    "visible-only",
    "acknowledgeable",
    "baseline-check",
    "calibrated-gate",
];

/// Everything the step summary reads besides the artifacts under `root`.
#[derive(Debug, Clone)]
pub(crate) struct CiSummaryInput {
    pub(crate) root: PathBuf,
    /// The PR base branch the missing-start-here route names.
    pub(crate) base_ref: String,
    /// `RIPR_UPLOAD_SARIF == "true"`.
    pub(crate) upload_sarif: bool,
    /// `RIPR_GATE_BASELINE` is set.
    pub(crate) gate_baseline: bool,
    /// `RIPR_COMMENT_MODE`, `off` when unset.
    pub(crate) comment_mode: String,
    /// The enabled language names `ripr doctor --json` reports.
    pub(crate) configured_languages: Vec<String>,
}

/// Render the step summary Markdown. Bytes, because full reports are
/// included verbatim.
pub(crate) fn render_ci_summary(input: &CiSummaryInput) -> Vec<u8> {
    let mut summary = Summary {
        root: input.root.clone(),
        relative: repo_relative_for(&input.root),
        out: Vec::new(),
    };
    summary.start_here();
    summary.first_run_status(&input.base_ref);
    summary.language_preview(&input.configured_languages);
    summary.front_panel();
    summary.recommended_next_test();
    summary.top_recommendation();
    summary.agent_review_packet();
    summary.artifact_packet();
    summary.uploaded_artifacts();
    summary.evidence_ledger();
    summary.assistant_proof();
    summary.agent_proof_status();
    summary.policy_readiness();
    summary.policy_operations();
    summary.policy_history();
    summary.policy_promotion();
    summary.preview_promotion();
    summary.waiver_aging();
    summary.suppression_health();
    summary.gate_decision();
    summary.baseline_delta(input.gate_baseline);
    summary.ripr_zero();
    summary.sarif_and_badges(input.upload_sarif);
    summary.guidance_annotations();
    summary.inline_comments(&input.comment_mode);
    summary.known_limits();
    summary.out
}

/// The checkout path as `pwd -P` and `$PWD` spelled it for the step.
fn repo_relative_for(root: &Path) -> RepoRelative {
    let physical = fs::canonicalize(root)
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default();
    // Like the shell, trust `$PWD` only while it still names the working
    // directory; otherwise the logical path is the physical one.
    let cwd = std::env::current_dir().ok();
    let base = std::env::var_os("PWD")
        .map(PathBuf::from)
        .filter(|path| {
            path.is_absolute()
                && fs::canonicalize(path).ok()
                    == cwd.as_ref().and_then(|cwd| fs::canonicalize(cwd).ok())
        })
        .or(cwd);
    let logical = base
        .map(|base| {
            base.join(root)
                .components()
                .filter(|component| !matches!(component, Component::CurDir))
                .collect::<PathBuf>()
                .to_string_lossy()
                .into_owned()
        })
        .unwrap_or_default();
    RepoRelative {
        physical: physical.into_bytes(),
        logical: logical.into_bytes(),
    }
}

/// Which verify and receipt labels a block uses: after a carried repair
/// start they name the manual alternative (#3906).
struct ProofLabels {
    verify: &'static str,
    receipt: &'static str,
}

impl ProofLabels {
    fn new(led_by_repair: bool) -> Self {
        if led_by_repair {
            Self {
                verify: MANUAL_VERIFY_LABEL,
                receipt: MANUAL_RECEIPT_LABEL,
            }
        } else {
            Self {
                verify: VERIFY_AFTER_EDIT_LABEL,
                receipt: RECEIPT_AFTER_VERIFY_LABEL,
            }
        }
    }
}

struct Summary {
    root: PathBuf,
    relative: RepoRelative,
    out: Vec<u8>,
}

impl Summary {
    fn line(&mut self, text: &str) {
        self.out.extend_from_slice(text.as_bytes());
        self.out.push(b'\n');
    }

    fn blank(&mut self) {
        self.out.push(b'\n');
    }

    fn is_file(&self, path: &str) -> bool {
        self.root.join(path).is_file()
    }

    fn doc(&self, path: &str) -> Option<Doc> {
        let path = self.root.join(path);
        path.is_file().then(|| Doc::read(&path))
    }

    /// A collapsed full report. `relative` rewrites the checkout path in
    /// reports that carry runnable commands.
    fn full_report(&mut self, path: &str, relative: bool) {
        self.line(&format!("<details><summary>Full report: {path}</summary>"));
        self.blank();
        let bytes = fs::read(self.root.join(path)).unwrap_or_default();
        if relative {
            let rewritten = self.relative.file(&bytes);
            self.out.extend(rewritten);
        } else {
            self.out.extend(bytes);
        }
        self.blank();
        self.line("</details>");
    }

    /// Lead with a carried repair start and its after phase (#3906).
    fn repair_start(&mut self, label: &str, command: &str) {
        self.line(&format!("- {label}: `{command}`"));
        self.line(&format!(
            "- {REPAIR_AFTER_PHASE_LABEL}: {REPAIR_AFTER_PHASE_STEP}"
        ));
    }

    fn start_here(&mut self) {
        self.line("## RIPR advisory summary");
        self.blank();
        self.line("RIPR is advisory static evidence. It does not edit source, generate tests, or run mutation testing.");
        self.blank();
        self.line("### Start here");
        self.line("- Open `target/ripr/reports/start-here.md` first when it exists.");
        self.line(
            "- Then open `target/ripr/reports/index.md` to navigate deeper evidence artifacts.",
        );
        self.line("- Safe next action: repair one named gap, regenerate missing or malformed artifacts, refresh stale evidence, fix wrong-root setup, or stop on no-action.");
        self.line("- Recovery states: missing artifact, stale evidence, wrong root, malformed artifact, no actionable gap, and preview-limited evidence are explicit stop or regeneration states.");
        self.line("- Proof rail: the repair start, verify, receipt, and receipt path are static movement evidence only; verify and receipt run after the test edit.");
        self.line("- Preview boundary: preview-limited evidence stays syntax-first and advisory, with static limits before repair language.");
        self.line("- Gate authority: `ripr gate evaluate` remains the pass/fail source only when `RIPR_GATE_MODE` is configured.");
        if self.is_file(START_HERE_MD) {
            self.line("- Start-here artifact: `target/ripr/reports/start-here.md`");
        } else if let Some(index) = self.doc(INDEX_JSON) {
            let path =
                inline(&index.field(&["summary.start_here"], "not_available", "not_available"));
            self.line(&format!("- Start-here artifact: `{path}`"));
        } else if self.is_file(FRONT_PANEL_MD) {
            self.line("- Start-here artifact: `target/ripr/reports/pr-review-front-panel.md`");
        } else if self.is_file(PILOT_SUMMARY_MD) {
            self.line("- Start-here artifact: `target/ripr/pilot/pilot-summary.md`");
        } else {
            self.line("- Start-here artifact: not generated yet; inspect uploaded artifacts and job logs.");
        }
        self.blank();
    }

    fn first_run_status(&mut self, base_ref: &str) {
        self.line("#### First-run status");
        if let Some(doc) = self.doc(START_HERE_JSON) {
            self.start_here_status(&doc);
        } else if let Some(doc) = self.doc(FIRST_ACTION_JSON) {
            self.first_action_status(&doc);
        } else {
            self.line("- Status: `missing_start_here`");
            self.line("- State: `missing_artifact`");
            self.line(&format!(
                "- Safe next action: run `ripr first-pr --root . --base origin/{base_ref} --head HEAD --gap-ledger target/ripr/reports/gap-decision-ledger.json --first-action target/ripr/reports/first-useful-action.json --review-comments target/ripr/review/comments.json --agent-packet {WORKFLOW_AGENT_PACKET_ARTIFACT} --gate-decision target/ripr/reports/gate-decision.json --receipts-dir target/ripr/receipts --out-dir target/ripr/reports`."
            ));
            self.line("- Fallback safe next action: run `ripr first-action --root . --pr-guidance target/ripr/review/comments.json --out target/ripr/reports/first-useful-action.json --out-md target/ripr/reports/first-useful-action.md` after attaching at least one explicit input.");
            self.line("- Boundary: missing start-here packet does not fail generated CI or create gate authority.");
        }
        self.blank();
    }

    fn start_here_status(&mut self, doc: &Doc) {
        let field = |paths: &[&str], default: &str| inline(&doc.field(paths, default, "unknown"));
        let status = field(&["status"], "unknown");
        let state = field(&["selected.state"], "unknown");
        let gap = field(
            &["selected.canonical_gap_id", "selected.gap_id"],
            "not_available",
        );
        let language = inline(&doc.text(
            |value| {
                let language = jq::at(value, "selected.language")?;
                if !jq::truthy(language) {
                    return Ok(Value::from("not_available"));
                }
                let status = jq::alt(value, &["selected.language_status"], Value::from("unknown"))?;
                let opened = jq::add(language.clone(), Value::from(" ("))?;
                jq::add(jq::add(opened, status)?, Value::from(")"))
            },
            "unknown",
        ));
        let kind = field(&["selected.kind"], "none");
        let changed = field(&["selected.changed_behavior"], "not_available");
        let evidence = field(
            &["selected.current_evidence_strength", "selected.state"],
            "not_available",
        );
        let missing = field(
            &[
                "selected.missing_discriminator",
                "selected.repair.suggested_assertion",
            ],
            "not_available",
        );
        let repair = field(
            &[
                "selected.repair.route",
                "selected.repair.suggested_assertion",
            ],
            "not_available",
        );
        let focused = field(
            &[
                "selected.focused_proof_intent",
                "selected.repair.suggested_assertion",
            ],
            "not_available",
        );
        let boundary = field(
            &["selected.static_evidence_boundary"],
            "static advisory evidence only; not runtime proof, coverage adequacy, mutation confirmation, gate approval, or merge approval.",
        );
        let target = field(&["selected.repair.target_file"], "not_available");
        let related = field(&["selected.repair.related_test"], "not_available");
        let limit = inline(&doc.text(
            |value| {
                let kind = jq::at(value, "selected.static_limit_kind")?;
                if !jq::truthy(kind) {
                    return Ok(Value::from("none"));
                }
                let detail = jq::at(value, "selected.static_limit_detail")?;
                let suffix = if jq::truthy(detail) {
                    jq::add(Value::from(": "), detail.clone())?
                } else {
                    Value::from("")
                };
                jq::add(kind.clone(), suffix)
            },
            "unknown",
        ));
        let verify = field(&["selected.verify_command"], "not_available");
        let receipt = field(&["selected.receipt_command"], "not_available");
        let receipt_path = field(&["selected.receipt_path"], "not_available");
        let receipt_state = field(&["selected.receipt_state"], "receipt_missing");
        let repair_command = doc.optional(&["selected.repair_command"]);
        let next = field(
            &[
                "selected.repair_command",
                "selected.next_command",
                "selected.regeneration_command",
            ],
            "none",
        );
        let warnings = inline(&doc.length("warnings"));
        let labels = ProofLabels::new(!repair_command.is_empty());
        if !repair_command.is_empty() {
            self.repair_start("Start repair", &inline(&repair_command));
        }
        self.line(&format!("- Status: `{status}`"));
        self.line(&format!("- Selected state: `{state}`"));
        self.line(&format!("- Canonical gap: `{gap}`"));
        self.line(&format!("- Language: `{language}`"));
        self.line(&format!("- Top gap/no-action: `{kind}`"));
        self.line(&format!("- Repair: `{repair}`"));
        self.line(&format!("- Changed behavior: `{changed}`"));
        self.line(&format!("- Current evidence strength: `{evidence}`"));
        self.line(&format!("- Missing discriminator: `{missing}`"));
        self.line(&format!("- Focused proof intent: `{focused}`"));
        self.line(&format!("- Boundary: `{boundary}`"));
        self.line(&format!("- Repair target: `{target}`"));
        self.line(&format!("- Related test: `{related}`"));
        self.line(&format!("- Static limit: `{limit}`"));
        self.line(&format!("- {}: `{verify}`", labels.verify));
        self.line(&format!("- {}: `{receipt}`", labels.receipt));
        self.line(&format!("- Receipt path: `{receipt_path}`"));
        self.line(&format!("- Receipt state: `{receipt_state}`"));
        self.line(&format!("- Safe next action command: `{next}`"));
        self.line(&format!("- Warnings: `{warnings}`"));
        self.line("- Artifacts: `target/ripr/reports/start-here.json`, `target/ripr/reports/start-here.md`");
        self.line("- Boundary: start-here is advisory first-run guidance only; gate decision remains separate pass/fail authority when configured.");
        if self.is_file(START_HERE_MD) {
            self.blank();
            self.full_report(START_HERE_MD, false);
        }
    }

    fn first_action_status(&mut self, doc: &Doc) {
        let field = |paths: &[&str], default: &str| inline(&doc.field(paths, default, "unknown"));
        let status = field(&["status"], "unknown");
        let action_kind = field(&["action_kind"], "unknown");
        let title = field(&["title"], "not_available");
        let why = field(&["why"], "not_available");
        let changed = field(&["selected.changed_behavior", "why"], "not_available");
        let evidence = field(
            &[
                "selected.current_evidence_strength",
                "selected.classification",
                "status",
            ],
            "not_available",
        );
        let missing = field(
            &[
                "selected.missing_discriminator",
                "target.suggested_assertion",
            ],
            "not_available",
        );
        let proof = field(
            &[
                "selected.focused_proof_intent",
                "target.suggested_assertion",
                "title",
            ],
            "not_available",
        );
        let gap = inline(&doc.text(
            |value| {
                let selected = jq::index(value, "selected")?;
                if selected.is_null() {
                    return Ok(Value::from("none"));
                }
                let located = jq::location(selected, "path", "unknown")?;
                let named = jq::alt(
                    selected,
                    &["missing_discriminator", "classification", "seam_id"],
                    Value::from("gap"),
                )?;
                jq::add(jq::add(located, Value::from(" "))?, named)
            },
            "unknown",
        ));
        let target = inline(&doc.text(
            |value| {
                let target = jq::index(value, "target")?;
                if target.is_null() {
                    return Ok(Value::from("none"));
                }
                let mut text = jq::alt(target, &["file"], Value::from("not_available"))?;
                text = jq::add(text, prefixed(target, "related_test", " related_test=")?)?;
                jq::add(
                    text,
                    prefixed(target, "suggested_test_name", " suggested=")?,
                )
            },
            "unknown",
        ));
        let repair = field(&["commands.repair"], "not_available");
        let packet = field(&["commands.context_packet"], "not_available");
        let verify = field(&["commands.verify"], "not_available");
        let receipt = field(&["commands.receipt"], "not_available");
        let fallback = field(&["fallback.summary", "fallback.kind"], "none");
        let warnings = inline(&doc.length("warnings"));
        let led = repair != "not_available" && repair != "unknown";
        let labels = ProofLabels::new(led);
        if led {
            self.repair_start("Repair start", &repair);
        }
        self.line(&format!("- Status: `{status}`"));
        self.line(&format!("- Safe next action: `{action_kind}`"));
        self.line(&format!("- Title: `{title}`"));
        self.line(&format!("- Why: `{why}`"));
        self.line(&format!("- Changed behavior: `{changed}`"));
        self.line(&format!("- Current evidence strength: `{evidence}`"));
        self.line(&format!("- Missing discriminator: `{missing}`"));
        self.line(&format!("- Focused proof intent: `{proof}`"));
        self.line(&format!("- Gap: `{gap}`"));
        self.line(&format!("- Repair target: `{target}`"));
        self.line(&format!("- Agent packet: `{packet}`"));
        self.line(&format!("- {}: `{verify}`", labels.verify));
        self.line(&format!("- {}: `{receipt}`", labels.receipt));
        self.line(&format!("- Fallback/no-action: `{fallback}`"));
        self.line(&format!("- Warnings: `{warnings}`"));
        self.line(&format!(
            "- Artifacts: `target/ripr/reports/first-useful-action.json`, `target/ripr/reports/first-useful-action.md`, `{WORKFLOW_AGENT_PACKET_ARTIFACT}`"
        ));
        self.line("- Boundary: advisory first-run path only; gate decision remains separate pass/fail authority when configured.");
    }

    fn language_preview(&mut self, configured: &[String]) {
        let configured = if configured.is_empty() {
            vec!["rust".to_string()]
        } else {
            configured.to_vec()
        };
        let mut preview = configured
            .iter()
            .map(|language| language.trim())
            .filter(|language| matches!(*language, "typescript" | "python"))
            .map(str::to_string)
            .collect::<Vec<_>>();
        preview.sort();
        preview.dedup();
        if preview.is_empty() {
            return;
        }
        let mut grouped = preview.clone();
        if preview.iter().any(|language| language == "typescript") {
            grouped.push("javascript".to_string());
        }
        grouped.sort();
        grouped.dedup();
        self.line("### Language preview grouping");
        self.line(&format!(
            "- Configured languages: `{}`",
            inline(&configured.join(","))
        ));
        self.line(&format!(
            "- Grouped preview evidence languages: `{}`",
            inline(&grouped.join(" "))
        ));
        self.line("- Boundary: preview-language groups are advisory presentation only; `ripr gate evaluate` remains pass/fail authority when explicitly configured.");
        let inputs = self.language_inputs();
        for language in &grouped {
            let stats = LanguageStats::of(inputs.as_ref(), language);
            let name = inline(language);
            if stats.artifact_entries == "0" {
                self.line(&format!(
                    "- `{name}`: configured preview/advisory; no language findings were emitted in this run; gate_impact=`none`."
                ));
            } else {
                self.line(&format!(
                    "- `{name}`: artifact_entries=`{}`, preview_entries=`{}`, missing_preview_status=`{}`, static_limit_entries=`{}`, classifications=`{}`, static_limit_kinds=`{}`, actionability_states=`{}`, actionability_categories=`{}`, repair_packet_ready=`{}`, gate_impact=`none`",
                    stats.artifact_entries,
                    stats.preview_entries,
                    stats.missing_preview_status,
                    stats.static_limit_entries,
                    stats.class_counts,
                    stats.static_limit_kinds,
                    stats.actionability_states,
                    stats.actionability_categories,
                    stats.repair_packet_ready,
                ));
            }
        }
        self.blank();
    }

    /// The artifacts `jq -s` read for language grouping: `None` when there
    /// were none, `Some(Err)` when any one was malformed.
    fn language_inputs(&self) -> Option<Result<Vec<Value>, Fail>> {
        let exposure = [
            "target/ripr/reports/repo-exposure.json",
            PILOT_BEFORE_SNAPSHOT_ARTIFACT,
        ]
        .into_iter()
        .find(|path| self.is_file(path));
        let paths = exposure
            .into_iter()
            .chain(
                [COMMENTS_JSON, GATE_JSON, LEDGER_JSON]
                    .into_iter()
                    .filter(|path| self.is_file(path)),
            )
            .collect::<Vec<_>>();
        if paths.is_empty() {
            return None;
        }
        let mut values = Vec::new();
        for path in paths {
            let doc = Doc::read(&self.root.join(path));
            if doc.broken {
                return Some(Err(Fail));
            }
            values.extend(doc.values);
        }
        Some(Ok(values))
    }

    fn front_panel(&mut self) {
        self.line("### PR review summary");
        if !self.is_file(FRONT_PANEL_JSON) && !self.is_file(FRONT_PANEL_MD) {
            self.line("PR review summary was not generated. It runs when existing PR guidance, first-useful-action, assistant proof, health, ledger, baseline, gate, calibration, coverage/grip, or receipt artifacts are available.");
            self.line("Safe next action: run `ripr pr-review front-panel --root . --pr-guidance target/ripr/review/comments.json --out target/ripr/reports/pr-review-front-panel.json --out-md target/ripr/reports/pr-review-front-panel.md` after attaching at least one explicit input.");
            self.blank();
            return;
        }
        if let Some(doc) = self.doc(FRONT_PANEL_JSON) {
            let field =
                |paths: &[&str], default: &str| inline(&doc.field(paths, default, "unknown"));
            let count = |path: &str| inline(&doc.field(&[path], "0", "0"));
            let command = |path: &str| {
                inline(
                    &self
                        .relative
                        .value(&doc.field(&[path], "not_available", "unknown")),
                )
            };
            let status = field(&["status"], "unknown");
            let headline = field(&["summary.headline"], "not_available");
            let top_state = field(&["summary.top_issue_state"], "unknown");
            let policy_state = field(&["summary.policy_state"], "none");
            let placement = field(&["summary.placement"], "not_available");
            let movement = field(&["summary.movement_state"], "unknown");
            let coverage_grip = field(&["summary.coverage_grip_state"], "not_available");
            let new_policy_eligible = count("summary.new_policy_eligible");
            let baseline_present = count("summary.baseline_still_present");
            let baseline_resolved = count("summary.baseline_resolved");
            let acknowledged = count("summary.acknowledged");
            let suppressed = count("summary.suppressed");
            let blocking = count("summary.blocking_candidates");
            let issue = inline(&doc.text(
                |value| {
                    let issue = jq::index(value, "top_issue")?;
                    if issue.is_null() {
                        Ok(Value::from("not_available"))
                    } else {
                        jq::location(issue, "path", "unknown")
                    }
                },
                "unknown",
            ));
            let class = field(&["top_issue.classification"], "not_available");
            let missing = field(&["top_issue.missing_discriminator"], "not_available");
            let related = field(&["top_issue.related_test"], "not_available");
            let suggested = field(&["top_issue.suggested_test"], "not_available");
            let verify = command("top_issue.verify_command");
            let agent = command("top_issue.agent_command");
            let repair = command("top_issue.repair_command");
            let receipt = command("top_issue.receipt.artifact");
            let gate_mode = field(&["policy.mode"], "not_available");
            let gate_decision = field(&["policy.decision"], "not_available");
            let warnings = inline(&doc.length("warnings"));
            self.line("#### PR review at a glance");
            let led = repair != "not_available" && repair != "unknown";
            if led {
                self.repair_start("Repair start", &repair);
            }
            let verify_label = ProofLabels::new(led).verify;
            self.line(&format!("- Status: `{status}`"));
            self.line(&format!("- Headline: `{headline}`"));
            self.line(&format!("- Top issue state: `{top_state}`"));
            self.line(&format!("- Policy state: `{policy_state}`"));
            self.line(&format!("- Placement: `{placement}`"));
            self.line(&format!("- Static movement: `{movement}`"));
            self.line(&format!("- Coverage/grip: `{coverage_grip}`"));
            self.line(&format!("- Counts: new_policy_eligible=`{new_policy_eligible}`, baseline_still_present=`{baseline_present}`, baseline_resolved=`{baseline_resolved}`, acknowledged=`{acknowledged}`, suppressed=`{suppressed}`, blocking_candidates=`{blocking}`"));
            self.line(&format!("- Top issue: `{issue}` class=`{class}`"));
            self.line(&format!("- Missing discriminator: `{missing}`"));
            self.line(&format!("- Suggested focused test: `{suggested}`"));
            self.line(&format!("- Related test: `{related}`"));
            self.line(&format!("- {verify_label}: `{verify}`"));
            if agent != repair {
                self.line(&format!("- Agent handoff: `{agent}`"));
            }
            self.line(&format!("- Receipt: `{receipt}`"));
            self.line(&format!(
                "- Gate: mode=`{gate_mode}`, decision=`{gate_decision}`"
            ));
            self.line(&format!("- Warnings: `{warnings}`"));
            self.line("- Front-panel artifacts: `target/ripr/reports/pr-review-front-panel.json`, `target/ripr/reports/pr-review-front-panel.md`");
            self.line("- Pass/fail authority remains `ripr gate evaluate` when an explicit gate mode is configured.");
            self.blank();
        }
        if self.is_file(FRONT_PANEL_MD) {
            self.full_report(FRONT_PANEL_MD, true);
        }
        self.blank();
    }

    fn recommended_next_test(&mut self) {
        self.line("### Recommended next test");
        if !self.is_file(FIRST_ACTION_JSON) && !self.is_file(FIRST_ACTION_MD) {
            self.line("Recommended next test was not generated. It runs when existing PR guidance, assistant proof, ledger, baseline, receipt, gate, coverage/grip, or editor context artifacts are available.");
            self.line("Safe next action: run `ripr first-action --root . --pr-guidance target/ripr/review/comments.json --out target/ripr/reports/first-useful-action.json --out-md target/ripr/reports/first-useful-action.md` after attaching at least one explicit input.");
            self.blank();
            return;
        }
        if let Some(doc) = self.doc(FIRST_ACTION_JSON) {
            let field =
                |paths: &[&str], default: &str| inline(&doc.field(paths, default, "unknown"));
            let command = |path: &str| {
                inline(
                    &self
                        .relative
                        .value(&doc.field(&[path], "not_available", "unknown")),
                )
            };
            let status = field(&["status"], "unknown");
            let kind = field(&["action_kind"], "unknown");
            let title = field(&["title"], "not_available");
            let why = field(&["why"], "not_available");
            let seam = field(&["selected.seam_id"], "not_available");
            let target = inline(&doc.text(
                |value| {
                    let file = jq::alt(value, &["target.file"], Value::from("not_available"))?;
                    let target = jq::index(value, "target")?;
                    jq::add(file, prefixed(target, "related_test", " related_test=")?)
                },
                "unknown",
            ));
            let repair = command("commands.repair");
            let verify = command("commands.verify");
            let receipt = command("commands.receipt");
            let fallback = field(&["fallback.kind"], "none");
            let warnings = inline(&doc.length("warnings"));
            self.line("#### Recommended next test at a glance");
            let led = repair != "not_available" && repair != "unknown";
            if led {
                self.repair_start("Repair start", &repair);
            }
            let labels = ProofLabels::new(led);
            self.line(&format!("- Status: `{status}`"));
            self.line(&format!("- Safe next action: `{kind}`"));
            self.line(&format!("- Title: `{title}`"));
            self.line(&format!("- Why: `{why}`"));
            self.line(&format!("- Seam: `{seam}`"));
            self.line(&format!("- Target: `{target}`"));
            self.line(&format!("- {}: `{verify}`", labels.verify));
            self.line(&format!("- {}: `{receipt}`", labels.receipt));
            self.line(&format!("- Fallback: `{fallback}`"));
            self.line(&format!("- Warnings: `{warnings}`"));
            self.line("- Action artifacts: `target/ripr/reports/first-useful-action.json`, `target/ripr/reports/first-useful-action.md`");
            self.line("- Boundary: static evidence only; no runtime mutation execution.");
            self.blank();
        }
        if self.is_file(FIRST_ACTION_MD) {
            self.full_report(FIRST_ACTION_MD, true);
        }
        self.blank();
    }

    fn top_recommendation(&mut self) {
        self.line("### Top recommendation");
        if self.is_file(PILOT_SUMMARY_MD) {
            self.full_report(PILOT_SUMMARY_MD, false);
        } else {
            self.line("Pilot summary was not generated. Inspect the uploaded artifact packet and job logs.");
        }
        self.blank();
    }

    /// CI runs before any test edit, so no receipt exists yet (#3906, N5).
    /// The packet then leads with the carried repair start and its after
    /// phase instead of the low-level post-edit loop.
    fn agent_review_packet(&mut self) {
        self.line("### Agent review packet");
        let movement = self
            .doc(WORKFLOW_AGENT_REVIEW_SUMMARY_ARTIFACT)
            .map(|doc| doc.optional(&["static_movement.state"]))
            .unwrap_or_default();
        if movement == "missing_artifact" {
            self.line(&format!("- Receipt: {NO_RECEIPT_BEFORE_REPAIR}"));
            let repair = self
                .doc(START_HERE_JSON)
                .map(|doc| doc.optional(&["selected.repair_command"]))
                .unwrap_or_default();
            if repair.is_empty() {
                self.line("- No repair start is available; `ripr agent status --root .` names the next step on a local checkout.");
            } else {
                self.repair_start("Start repair", &inline(&repair));
            }
            self.line(&format!(
                "- Full packet: `{WORKFLOW_AGENT_REVIEW_SUMMARY_MARKDOWN_ARTIFACT}` (workflow artifact)."
            ));
        } else if self.is_file(WORKFLOW_AGENT_REVIEW_SUMMARY_MARKDOWN_ARTIFACT) {
            self.full_report(WORKFLOW_AGENT_REVIEW_SUMMARY_MARKDOWN_ARTIFACT, false);
        } else {
            self.line("Agent review summary was not generated. Run `ripr agent status --root .` locally or inspect uploaded workflow artifacts.");
        }
        self.blank();
    }

    fn artifact_packet(&mut self) {
        self.line("### Artifact packet");
        self.line("- Pilot reports: `target/ripr/pilot/`");
        self.line("- Agent workflow: `target/ripr/workflow/`");
        self.line("- Agent compatibility copies: `target/ripr/agent/`");
        self.line("- Repo reports, badges, SARIF, and receipts: `target/ripr/reports/`");
        self.line("- CI labels and plan inputs: `target/ci/`");
        if self.root.join("target/ripr/review").is_dir() {
            self.line("- PR test guidance report: `target/ripr/review/`");
        } else {
            self.line("- PR test guidance report: not generated yet");
        }
        self.blank();
    }

    fn uploaded_artifacts(&mut self) {
        self.line("### Uploaded review artifacts");
        if !self.is_file(INDEX_JSON) && !self.is_file(INDEX_MD) {
            self.line("Uploaded review artifacts summary was not generated. It runs when existing RIPR report, review, receipt, workflow, agent, pilot, or CI artifacts are available.");
            self.line("Regenerate command: `ripr reports index --root . --reports-dir target/ripr/reports --review-dir target/ripr/review --receipts-dir target/ripr/receipts --workflow-dir target/ripr/workflow --agent-dir target/ripr/agent --pilot-dir target/ripr/pilot --ci-dir target/ci --out target/ripr/reports/index.json --out-md target/ripr/reports/index.md`.");
            self.blank();
            return;
        }
        if let Some(doc) = self.doc(INDEX_JSON) {
            let field =
                |paths: &[&str], default: &str| inline(&doc.field(paths, default, "unknown"));
            let count = |path: &str| inline(&doc.field(&[path], "0", "0"));
            let status = field(&["status"], "unknown");
            let entries = count("summary.entries");
            let available = count("summary.available");
            let missing = count("summary.missing_expected");
            let warnings = count("summary.warnings");
            let failures = count("summary.failures");
            let start = field(&["summary.start_here"], "not_available");
            let gate = field(&["summary.gate_authority"], "not_available");
            let missing_labels = inline(&doc.text(
                |value| each_field(value, "missing_expected", "label"),
                "unknown",
            ));
            let warning_kinds =
                inline(&doc.text(|value| each_field(value, "warnings", "kind"), "unknown"));
            self.line("#### Uploaded artifacts at a glance");
            self.line(&format!("- Status: `{status}`"));
            self.line(&format!("- Entries: total=`{entries}`, available=`{available}`, missing_expected=`{missing}`, warnings=`{warnings}`, failures=`{failures}`"));
            self.line(&format!("- Start here: `{start}`"));
            self.line(&format!("- Gate authority: `{gate}`"));
            self.line(&format!("- Missing expected: `{missing_labels}`"));
            self.line(&format!("- Warning kinds: `{warning_kinds}`"));
            self.line("- Index artifacts: `target/ripr/reports/index.json`, `target/ripr/reports/index.md`");
            self.line("- Boundary: advisory artifact map only; gate-decision remains configured pass/fail authority.");
            self.blank();
        }
        if self.is_file(INDEX_MD) {
            self.full_report(INDEX_MD, false);
        }
        self.blank();
    }

    fn evidence_ledger(&mut self) {
        self.line("### PR evidence ledger");
        if let Some(doc) = self.doc(LEDGER_JSON) {
            let field =
                |paths: &[&str], default: &str| inline(&doc.field(paths, default, "unknown"));
            let count = |path: &str| inline(&doc.field(&[path], "0", "0"));
            let status = field(&["status"], "unknown");
            let gate_mode = field(&["gate.mode"], "not_evaluated");
            let gate_decision = field(&["gate.decision"], "not_evaluated");
            let new_policy_eligible = count("movement.new_policy_eligible");
            let still_present = count("movement.baseline_still_present");
            let resolved = count("movement.baseline_resolved");
            let acknowledged = count("movement.acknowledged");
            let suppressed = count("movement.suppressed");
            let blocking = count("movement.blocking_candidates");
            let visible = count("movement.visible_unresolved");
            let coverage = field(&["coverage_grip_frontier.status"], "not_available");
            let trend = field(&["history.trend"], "not_available");
            let route = inline(&doc.text(
                |value| repair_route(jq::index(value, "top_repair_route")?),
                "unknown",
            ));
            let verify = field(&["top_repair_route.verify_command"], "not_available");
            let agent = field(&["top_repair_route.agent_command"], "not_available");
            let repair = field(&["top_repair_route.repair_command"], "not_available");
            self.line("#### PR movement at a glance");
            let led = repair != "not_available" && repair != "unknown";
            if led {
                self.repair_start("Repair start", &repair);
            }
            let verify_label = ProofLabels::new(led).verify;
            self.line(&format!("- Status: `{status}`"));
            self.line(&format!(
                "- Gate: mode=`{gate_mode}`, decision=`{gate_decision}`"
            ));
            // F60-4: counts with no baseline delta or RIPR Zero status
            // behind them were never measured; do not print their zeros.
            if doc.field(&["movement.count_source"], "unknown", "unknown") == "not_measured" {
                self.line(&format!("- Counts: gap counts not measured (no baseline debt delta or RIPR Zero status); acknowledged=`{acknowledged}`, suppressed=`{suppressed}`, blocking_candidates=`{blocking}`"));
            } else {
                self.line(&format!("- Counts: new_policy_eligible=`{new_policy_eligible}`, baseline_still_present=`{still_present}`, baseline_resolved=`{resolved}`, acknowledged=`{acknowledged}`, suppressed=`{suppressed}`, blocking_candidates=`{blocking}`, visible_unresolved=`{visible}`"));
            }
            self.line(&format!("- Top repair route: `{route}`"));
            self.line(&format!("- {verify_label}: `{verify}`"));
            if agent != repair {
                self.line(&format!("- Agent command: `{agent}`"));
            }
            self.line(&format!("- Coverage/grip frontier: `{coverage}`"));
            self.line(&format!("- History trend: `{trend}`"));
            self.line("- Ledger artifacts: `target/ripr/reports/pr-evidence-ledger.json`, `target/ripr/reports/pr-evidence-ledger.md`");
            self.line("- Pass/fail authority remains `ripr gate evaluate` when an explicit gate mode is configured.");
            self.blank();
        }
        if self.is_file(LEDGER_MD) {
            self.full_report(LEDGER_MD, false);
        } else if self.is_file(COMMENTS_JSON) {
            self.line("PR evidence ledger was not generated. Inspect `target/ripr/review/comments.json` and rerun `ripr pr-ledger record` locally.");
        } else {
            self.line("PR evidence ledger was not run. It requires pull-request guidance from `target/ripr/review/comments.json`.");
        }
        self.blank();
    }

    fn assistant_proof(&mut self) {
        let json = "target/ripr/reports/test-oracle-assistant-proof.json";
        let md = "target/ripr/reports/test-oracle-assistant-proof.md";
        if !self.is_file(json) && !self.is_file(md) {
            return;
        }
        self.line("### Test-oracle assistant proof");
        if let Some(doc) = self.doc(json) {
            let field =
                |paths: &[&str], default: &str| inline(&doc.field(paths, default, "unknown"));
            let status = field(&["status"], "unknown");
            let seam = inline(&doc.text(
                |value| jq::location(jq::index(value, "seam")?, "path", "unknown"),
                "unknown",
            ));
            let missing = field(&["seam.missing_discriminator"], "not_available");
            let placement = field(&["recommendation.placement"], "not_available");
            let movement = field(&["evidence_movement.state"], "unknown");
            let receipt = field(
                &["evidence_movement.artifact", "inputs.receipt"],
                "not_available",
            );
            let gate = field(&["ci_projection.gate_decision"], "not_supplied");
            let coverage = field(&["ci_projection.coverage_frontier"], "not_supplied");
            let warnings = inline(&doc.length("warnings"));
            self.line("#### Assistant proof at a glance");
            self.line(&format!("- Status: `{status}`"));
            self.line(&format!("- Seam: `{seam}`"));
            self.line(&format!("- Missing discriminator: `{missing}`"));
            self.line(&format!("- Placement: `{placement}`"));
            self.line(&format!("- Static movement: `{movement}`"));
            self.line(&format!("- Receipt: `{receipt}`"));
            self.line(&format!("- Gate input: `{gate}`"));
            self.line(&format!("- Coverage/grip frontier input: `{coverage}`"));
            self.line(&format!("- Warnings: `{warnings}`"));
            self.line("- Proof artifacts: `target/ripr/reports/test-oracle-assistant-proof.json`, `target/ripr/reports/test-oracle-assistant-proof.md`");
            self.line("- Pass/fail authority remains `ripr gate evaluate` when an explicit gate mode is configured.");
            self.blank();
        }
        if self.is_file(md) {
            self.full_report(md, false);
        }
        self.blank();
    }

    fn agent_proof_status(&mut self) {
        let json = "target/ripr/reports/assistant-loop-health.json";
        let md = "target/ripr/reports/assistant-loop-health.md";
        if !self.is_file(json) && !self.is_file(md) {
            return;
        }
        self.line("### Agent proof status");
        if let Some(doc) = self.doc(json) {
            let count = |path: &str| inline(&doc.field(&[path], "0", "0"));
            let status = inline(&doc.field(&["status"], "unknown", "unknown"));
            let proofs = count("summary.proofs");
            let complete = count("summary.complete");
            let partial = count("summary.partial");
            let missing_required = count("summary.missing_required_input");
            let missing_optional = count("summary.missing_optional_input");
            let improved = count("summary.improved");
            let unchanged = count("summary.unchanged");
            let regressed = count("summary.regressed");
            let unknown = count("summary.unknown_movement");
            let warnings = count("summary.warnings");
            let repairs = count("summary.repair_queue");
            let top_warning = inline(&doc.text(
                |value| {
                    let mut tallies = Vec::new();
                    for item in jq::each_at(value, "warning_summary")? {
                        tallies.push(Value::from(format!(
                            "{}={}",
                            jq::interpolated(jq::index(item, "kind")?),
                            jq::interpolated(jq::index(item, "count")?)
                        )));
                    }
                    jq::join_or_none(&tallies)
                },
                "unknown",
            ));
            let top_repair = inline(&doc.text(
                |value| first_field(value, "repair_queue", "repair_kind"),
                "unknown",
            ));
            self.line("#### Agent proof status at a glance");
            self.line(&format!("- Status: `{status}`"));
            self.line(&format!("- Proof packets: total=`{proofs}`, complete=`{complete}`, partial=`{partial}`, missing_required=`{missing_required}`, missing_optional=`{missing_optional}`"));
            self.line(&format!("- Evidence movement: improved=`{improved}`, unchanged=`{unchanged}`, regressed=`{regressed}`, unknown=`{unknown}`"));
            self.line(&format!(
                "- Warnings: total=`{warnings}`, top=`{top_warning}`"
            ));
            self.line(&format!(
                "- Repair queue: total=`{repairs}`, first=`{top_repair}`"
            ));
            self.line("- Health artifacts: `target/ripr/reports/assistant-loop-health.json`, `target/ripr/reports/assistant-loop-health.md`");
            self.line("- Boundary: advisory static health over proof artifacts; gate evaluator remains pass/fail authority.");
            self.blank();
        }
        if self.is_file(md) {
            self.full_report(md, false);
        }
        self.blank();
    }

    fn policy_readiness(&mut self) {
        self.line("### Policy readiness");
        if let Some(doc) = self.doc("target/ripr/reports/policy-readiness.json") {
            let field = |path: &str| inline(&doc.field(&[path], "unknown", "unknown"));
            let status = field("status");
            let mode = field("recommended_mode");
            let blocking = field("blocking_readiness.state");
            let baseline = field("baseline_health.state");
            let waiver = field("waiver_health.state");
            let suppression = field("suppression_health.state");
            let calibration = field("calibration_health.state");
            let preview = field("preview_evidence_boundary.state");
            let warnings = inline(&doc.length("warnings"));
            let unknowns = inline(&doc.length("unknowns"));
            let next = inline(&doc.field(&["next_policy_action"], "not_available", "unknown"));
            self.line("#### Policy readiness at a glance");
            self.line(&format!("- Status: `{status}`"));
            self.line(&format!("- Recommended mode: `{mode}`"));
            self.line(&format!("- Axes: blocking=`{blocking}`, baseline=`{baseline}`, waiver=`{waiver}`, suppression=`{suppression}`, calibration=`{calibration}`, preview=`{preview}`"));
            self.line(&format!("- Warnings: `{warnings}`; unknowns: `{unknowns}`"));
            self.line(&format!("- Next policy action: `{next}`"));
            self.line("- Policy readiness artifacts: `target/ripr/reports/policy-readiness.json`, `target/ripr/reports/policy-readiness.md`");
            self.line("- Boundary: advisory readiness projection only; `ripr gate evaluate` remains pass/fail authority when configured.");
            self.blank();
        }
        self.report_or(
            "target/ripr/reports/policy-readiness.md",
            "Policy readiness was not generated. It is advisory and requires existing policy artifacts to be useful.",
        );
    }

    fn policy_operations(&mut self) {
        self.line("### Policy operations");
        if let Some(doc) = self.doc("target/ripr/reports/policy-operations.json") {
            let ceiling = inline(&doc.field(&["current_policy_ceiling"], "unknown", "unknown"));
            let next = inline(&doc.field(&["recommended_next_action"], "not_available", "unknown"));
            let safe = inline(&doc.length("safe_to_promote_to"));
            let blocked = inline(&doc.length("not_safe_to_promote_to"));
            let blockers = inline(&doc.length("promotion_blockers"));
            let top_blocker = inline(&doc.text(
                |value| first_field(value, "promotion_blockers", "repair_action"),
                "unknown",
            ));
            let warnings = inline(&doc.length("warnings"));
            let unknowns = inline(&doc.length("unknowns"));
            self.line("#### Policy operations at a glance");
            self.line(&format!("- Current ceiling: `{ceiling}`"));
            self.line(&format!("- Next safe action: `{next}`"));
            self.line(&format!(
                "- Promotion modes: allowed=`{safe}`, blocked=`{blocked}`"
            ));
            self.line(&format!(
                "- Blockers: total=`{blockers}`, first=`{top_blocker}`"
            ));
            self.line(&format!("- Warnings: `{warnings}`; unknowns: `{unknowns}`"));
            self.line("- Policy operations artifacts: `target/ripr/reports/policy-operations.json`, `target/ripr/reports/policy-operations.md`");
            self.line("- Boundary: advisory operations packet only; promotion requires manual review and separate configuration changes.");
            self.blank();
        }
        self.report_or(
            "target/ripr/reports/policy-operations.md",
            "Policy operations was not generated. It requires policy-readiness and keeps promotion advisory until packet review.",
        );
    }

    fn policy_history(&mut self) {
        self.line("### Policy history");
        if let Some(doc) = self.doc("target/ripr/reports/policy-history.json") {
            let direction = |path: &str| inline(&doc.field(&[path], "unknown", "unknown"));
            let ceiling = direction("current.current_policy_ceiling");
            let entries = inline(&doc.field(&["history_summary.entries"], "0", "0"));
            let readiness = direction("trend.ceiling.direction");
            let waiver = direction("trend.waiver_count.direction");
            let suppression = direction("trend.stale_suppression_count.direction");
            let baseline_present = direction("trend.baseline_still_present.direction");
            let baseline_resolved = direction("trend.baseline_resolved.direction");
            let preview = direction("trend.preview_boundary_state.direction");
            let warnings = inline(&doc.length("warnings"));
            let unknowns = inline(&doc.length("unknowns"));
            self.line("#### Policy history at a glance");
            self.line(&format!(
                "- Current ceiling: `{ceiling}`; history entries: `{entries}`"
            ));
            self.line(&format!("- Trends: readiness=`{readiness}`, waiver_pressure=`{waiver}`, suppression_health=`{suppression}`, baseline_still_present=`{baseline_present}`, baseline_resolved=`{baseline_resolved}`, preview_boundary=`{preview}`"));
            self.line(&format!("- Warnings: `{warnings}`; unknowns: `{unknowns}`"));
            self.line("- Policy history artifacts: `target/ripr/reports/policy-history.json`, `target/ripr/reports/policy-history.md`");
            self.line("- Boundary: history is read-only and never appends to `.ripr/policy-history.jsonl` automatically.");
            self.blank();
        }
        self.report_or(
            "target/ripr/reports/policy-history.md",
            "Policy history was not generated. It requires policy-operations and never writes history automatically.",
        );
    }

    fn policy_promotion(&mut self) {
        self.line("### Policy promotion packets");
        let mut found = false;
        for mode in POLICY_PROMOTION_MODES {
            let Some(doc) = self.doc(&format!("{REPORTS}/policy-promotion-{mode}.json")) else {
                continue;
            };
            found = true;
            let target = inline(&doc.field(&["target_mode"], "unknown", "unknown"));
            let allowed = inline(&doc.field(&["allowed_now"], "false", "false"));
            let repairs = inline(&doc.length("required_repairs"));
            let receipts = inline(&doc.length("required_receipts"));
            let warnings = inline(&doc.length("warnings"));
            let unknowns = inline(&doc.length("unknowns"));
            let reason = inline(&doc.field(&["why_or_why_not"], "not_available", "unknown"));
            self.line(&format!("- `{target}`: allowed_now=`{allowed}`, repairs=`{repairs}`, receipts=`{receipts}`, warnings=`{warnings}`, unknowns=`{unknowns}`, why=`{reason}`"));
        }
        if found {
            self.line("- Promotion packet artifacts: `target/ripr/reports/policy-promotion-*.json`, `target/ripr/reports/policy-promotion-*.md`");
            self.line("- Boundary: packets do not edit `ripr.toml`, baselines, suppressions, workflows, branch protection, CI defaults, or preview eligibility.");
        } else {
            self.line("Policy promotion packets were not generated. They require policy-operations and remain read-only manual review packets.");
        }
        for mode in POLICY_PROMOTION_MODES {
            let md = format!("{REPORTS}/policy-promotion-{mode}.md");
            if self.is_file(&md) {
                self.blank();
                self.full_report(&md, false);
            }
        }
        self.blank();
    }

    fn preview_promotion(&mut self) {
        self.line("### Preview promotion packets");
        let jsons = self.preview_promotion_files(".json");
        for json in &jsons {
            let Some(doc) = self.doc(json) else {
                continue;
            };
            let language = inline(&doc.field(&["language"], "unknown", "unknown"));
            let class = inline(&doc.field(&["candidate_class"], "unknown", "unknown"));
            let allowed = inline(&doc.field(&["allowed_now"], "false", "false"));
            let missing = inline(&doc.length("missing_evidence"));
            let supplied = inline(&doc.length("supplied_evidence"));
            let warnings = inline(&doc.length("warnings"));
            let unknowns = inline(&doc.length("unknowns"));
            self.line(&format!("- `{language}`/`{class}`: allowed_now=`{allowed}`, supplied_evidence=`{supplied}`, missing_evidence=`{missing}`, warnings=`{warnings}`, unknowns=`{unknowns}`"));
        }
        if jsons.iter().any(|json| self.is_file(json)) {
            self.line("- Preview promotion artifacts: `target/ripr/reports/preview-promotion-*.json`, `target/ripr/reports/preview-promotion-*.md`");
            self.line("- Boundary: preview evidence remains visible and non-gating unless a later explicit promotion policy is reviewed.");
        } else {
            self.line("Preview promotion packets were not generated. They are only surfaced when TypeScript or Python preview adapters are configured.");
        }
        for md in self.preview_promotion_files(".md") {
            if self.is_file(&md) {
                self.blank();
                self.full_report(&md, false);
            }
        }
        self.blank();
    }

    /// `target/ripr/reports/preview-promotion-*-*<suffix>`, in the shell's
    /// glob order.
    fn preview_promotion_files(&self, suffix: &str) -> Vec<String> {
        let Ok(entries) = fs::read_dir(self.root.join(REPORTS)) else {
            return Vec::new();
        };
        let mut names = entries
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|name| {
                name.strip_prefix("preview-promotion-")
                    .and_then(|rest| rest.strip_suffix(suffix))
                    .is_some_and(|middle| middle.contains('-'))
            })
            .collect::<Vec<_>>();
        names.sort();
        names
            .into_iter()
            .map(|name| format!("{REPORTS}/{name}"))
            .collect()
    }

    fn waiver_aging(&mut self) {
        self.line("### Waiver aging");
        if let Some(doc) = self.doc("target/ripr/reports/waiver-aging.json") {
            let count = |path: &str| inline(&doc.field(&[path], "0", "0"));
            let status = inline(&doc.field(&["status"], "unknown", "unknown"));
            let waivers = count("summary.waiver_count");
            let identities = count("summary.identity_count");
            let repeated_seams = count("summary.repeated_seam_count");
            let repeated_files = count("summary.repeated_file_count");
            let focused = count("summary.focused_test_candidates");
            let suppression = count("summary.durable_suppression_candidates");
            let warnings = count("summary.warnings");
            self.line("#### Waiver aging at a glance");
            self.line(&format!("- Status: `{status}`"));
            self.line(&format!("- Counts: waivers=`{waivers}`, identities=`{identities}`, repeated_seams=`{repeated_seams}`, repeated_files=`{repeated_files}`"));
            self.line(&format!("- Review signals: focused_test_candidates=`{focused}`, durable_suppression_candidates=`{suppression}`, warnings=`{warnings}`"));
            self.line("- Waiver-aging artifacts: `target/ripr/reports/waiver-aging.json`, `target/ripr/reports/waiver-aging.md`");
            self.line("- Boundary: repeated waiver is a visible signal, not a failure or durable suppression.");
            self.blank();
        }
        if self.is_file("target/ripr/reports/waiver-aging.md") {
            self.full_report("target/ripr/reports/waiver-aging.md", false);
        } else if self.is_file(LEDGER_JSON) {
            self.line("Waiver aging was not generated. Inspect `target/ripr/reports/pr-evidence-ledger.json` and rerun `ripr policy waiver-aging` locally.");
        } else {
            self.line("Waiver aging was not run. It requires a PR evidence ledger.");
        }
        self.blank();
    }

    fn suppression_health(&mut self) {
        self.line("### Suppression health");
        if let Some(doc) = self.doc("target/ripr/reports/suppression-health.json") {
            let count = |path: &str| inline(&doc.field(&[path], "0", "0"));
            let status = inline(&doc.field(&["status"], "unknown", "unknown"));
            let total = count("summary.suppressions");
            let healthy = count("summary.healthy");
            let missing_owner = count("summary.missing_owner");
            let missing_reason = count("summary.missing_reason");
            let stale = count("summary.stale");
            let overbroad = count("summary.overbroad_scope");
            let unknown_selector = count("summary.unknown_selector");
            let preview_gap = count("summary.preview_without_preview_label");
            let warnings = count("summary.warnings");
            let config_errors = count("summary.config_errors");
            self.line("#### Suppression health at a glance");
            self.line(&format!("- Status: `{status}`"));
            self.line(&format!("- Counts: suppressions=`{total}`, healthy=`{healthy}`, missing_owner=`{missing_owner}`, missing_reason=`{missing_reason}`"));
            self.line(&format!("- Review signals: stale=`{stale}`, overbroad_scope=`{overbroad}`, unknown_selector=`{unknown_selector}`, preview_without_preview_label=`{preview_gap}`"));
            self.line(&format!(
                "- Warnings: `{warnings}`; config_errors: `{config_errors}`"
            ));
            self.line("- Suppression-health artifacts: `target/ripr/reports/suppression-health.json`, `target/ripr/reports/suppression-health.md`");
            self.line("- Boundary: suppressions remain visible durable exceptions; this report never applies or gates on suppressions.");
            self.blank();
        }
        self.report_or(
            "target/ripr/reports/suppression-health.md",
            "Suppression health was not generated. It is advisory and reads the durable suppression manifest when present.",
        );
    }

    fn gate_decision(&mut self) {
        self.line("### Gate decision");
        if let Some(doc) = self.doc(GATE_JSON) {
            let count = |path: &str| inline(&doc.field(&[path], "0", "0"));
            let supplied = |path: &str| inline(&doc.field(&[path], "not supplied", "unknown"));
            let status = inline(&doc.field(&["status"], "unknown", "unknown"));
            let mode = inline(&doc.field(&["mode"], "unknown", "unknown"));
            let blocking = count("summary.blocking");
            let acknowledged = count("summary.acknowledged");
            let advisory = count("summary.advisory");
            let suppressed = count("summary.suppressed");
            let not_applicable = count("summary.not_applicable");
            let unknown_confidence = count("summary.unknown_confidence");
            let labels = |path: &'static str| {
                inline(&doc.text(
                    move |value| {
                        let labels = jq::alt(value, &[path], Value::Array(Vec::new()))?;
                        if jq::length(&labels)?.as_f64() == Some(0.0) {
                            return Ok(Value::from("none"));
                        }
                        match &labels {
                            Value::Array(items) => jq::join(items, ", ").map(Value::from),
                            Value::Object(map) => {
                                jq::join(&map.values().cloned().collect::<Vec<_>>(), ", ")
                                    .map(Value::from)
                            }
                            _ => Err(Fail),
                        }
                    },
                    "unknown",
                ))
            };
            let active_labels = labels("inputs.labels");
            let acknowledgement_labels = labels("policy.acknowledgement_labels");
            let applied_waiver = inline(&doc.text(
                |value| {
                    let mut labels = Vec::new();
                    for decision in jq::each_at(value, "decisions")? {
                        if jq::index(decision, "decision")? == &Value::from("acknowledged") {
                            let label = jq::at(decision, "policy.acknowledgement_label")?;
                            if !label.is_null() {
                                labels.push(label.clone());
                            }
                        }
                    }
                    first_or_none(labels)
                },
                "unknown",
            ));
            let baseline = supplied("inputs.baseline");
            let recommendation = supplied("inputs.recommendation_calibration");
            let mutation = supplied("inputs.mutation_calibration");
            let effects = |calibration: &'static str| {
                inline(&doc.text(
                    move |value| {
                        let mut effects = Vec::new();
                        for decision in jq::each_at(value, "decisions")? {
                            let effect = jq::at(
                                decision,
                                &format!("evidence.{calibration}.confidence_effect"),
                            )?;
                            if !effect.is_null() {
                                effects.push(effect.clone());
                            }
                        }
                        jq::join_or_none(&jq::unique(effects))
                    },
                    "unknown",
                ))
            };
            let recommendation_effects = effects("recommendation_calibration");
            let mutation_effects = effects("mutation_calibration");
            let blocking_reason = inline(&doc.text(
                |value| {
                    let mut reasons = Vec::new();
                    for decision in jq::each_at(value, "decisions")? {
                        if jq::index(decision, "decision")? == &Value::from("blocking") {
                            reasons.push(jq::index(decision, "gate_reason")?.clone());
                        }
                    }
                    Ok(match reasons.as_slice() {
                        [] => Value::from("none"),
                        [only] => only.clone(),
                        [first, rest @ ..] => Value::from(format!(
                            "{} (+{} more, see gate-decision.md)",
                            jq::interpolated(first),
                            rest.len()
                        )),
                    })
                },
                "unknown",
            ));
            self.line("#### Gate decision at a glance");
            self.line(&format!("- Mode: `{mode}`"));
            self.line(&format!("- Status: `{status}`"));
            self.line(&format!("- Counts: blocking=`{blocking}`, acknowledged=`{acknowledged}`, advisory=`{advisory}`, suppressed=`{suppressed}`, not_applicable=`{not_applicable}`, unknown_confidence=`{unknown_confidence}`"));
            self.line(&format!("- Active PR labels: `{active_labels}`"));
            self.line(&format!(
                "- Acknowledgement labels: `{acknowledgement_labels}`"
            ));
            self.line(&format!("- Applied waiver label: `{applied_waiver}`"));
            self.line(&format!("- Baseline artifact: `{baseline}`"));
            self.line(&format!("- Recommendation calibration: `{recommendation}` (effects: {recommendation_effects})"));
            self.line(&format!(
                "- Mutation calibration: `{mutation}` (effects: {mutation_effects})"
            ));
            self.line(&format!(
                "- Blocking reason (`{blocking}`): `{blocking_reason}`"
            ));
            self.line("- Gate artifacts: `target/ripr/reports/gate-decision.json`, `target/ripr/reports/gate-decision.md`");
            self.line(
                "- Related inputs: `target/ripr/review/comments.json`, `target/ci/labels.json`",
            );
            self.blank();
        }
        self.report_or(
            GATE_MD,
            "Gate decision was not run. Set `RIPR_GATE_MODE` to `visible-only`, `acknowledgeable`, `baseline-check`, or `calibrated-gate` to opt in.",
        );
    }

    fn baseline_delta(&mut self, gate_baseline: bool) {
        self.line("### Baseline debt delta");
        if let Some(doc) = self.doc(DELTA_JSON) {
            let count = |path: &str| inline(&doc.field(&[path], "0", "0"));
            let baseline =
                inline(&doc.field(&["baseline.path", "inputs.baseline"], "unknown", "unknown"));
            let still_present = count("delta.still_present");
            let resolved = count("delta.resolved");
            let new_policy_eligible = count("delta.new_policy_eligible");
            let acknowledged = count("delta.acknowledged");
            let suppressed = count("delta.suppressed");
            let stale = count("delta.stale_baseline_entry");
            let invalid = count("delta.invalid_baseline_entry");
            let missing_current = count("delta.missing_current_input");
            let legacy = count("delta.legacy_fallback_match");
            let limits = inline(&doc.field(
                &["limits_note"],
                "Advisory baseline debt movement; gate decision owns pass or fail.",
                "unknown",
            ));
            self.line("#### Baseline debt movement");
            self.line(&format!("- Baseline: `{baseline}`"));
            self.line(&format!("- Counts: still_present=`{still_present}`, resolved=`{resolved}`, new_policy_eligible=`{new_policy_eligible}`, acknowledged=`{acknowledged}`, suppressed=`{suppressed}`, stale=`{stale}`, invalid=`{invalid}`, missing_current_input=`{missing_current}`, legacy_fallback=`{legacy}`"));
            self.line(&format!("- Boundary: {limits}"));
            self.line("- Baseline delta artifacts: `target/ripr/reports/baseline-debt-delta.json`, `target/ripr/reports/baseline-debt-delta.md`");
            self.blank();
        }
        if self.is_file(DELTA_MD) {
            self.full_report(DELTA_MD, false);
        } else if gate_baseline {
            self.line("Baseline debt delta was not generated. Check that `RIPR_GATE_MODE` produced `target/ripr/reports/gate-decision.json` and that `RIPR_GATE_BASELINE` points at a readable baseline.");
        } else {
            self.line("Baseline debt delta was not run. Set `RIPR_GATE_BASELINE` with an explicit gate mode to compare current evidence against reviewed baseline debt.");
        }
        self.blank();
    }

    fn ripr_zero(&mut self) {
        self.line("### RIPR Zero status");
        if let Some(doc) = self.doc(ZERO_JSON) {
            let count = |path: &str| inline(&doc.field(&[path], "0", "0"));
            let state = inline(&doc.field(&["ripr_zero.state"], "unknown", "unknown"));
            let visible = count("ripr_zero.visible_unresolved");
            let new_policy_eligible = count("ripr_zero.new_policy_eligible");
            let blocking = count("ripr_zero.blocking_candidates");
            let acknowledged = count("ripr_zero.acknowledged");
            let suppressed = count("ripr_zero.suppressed");
            let still_present = count("baseline.still_present");
            let resolved = count("baseline.resolved");
            let metadata_stale = count("baseline.metadata.stale");
            let metadata_missing = count("baseline.metadata.missing_metadata");
            let top_area = inline(&doc.text(
                |value| {
                    let first = first_item(jq::index(value, "top_debt_areas")?)?;
                    jq::alt(first, &["area"], Value::from("none"))
                },
                "unknown",
            ));
            let top_route = inline(&doc.text(
                |value| repair_route(first_item(jq::index(value, "repair_routes")?)?),
                "unknown",
            ));
            let trend = inline(&doc.field(&["trend.source"], "not_available", "unknown"));
            self.line("#### RIPR Zero at a glance");
            self.line(&format!("- State: `{state}`"));
            self.line(&format!("- Visible unresolved: `{visible}`"));
            self.line(&format!("- New policy-eligible: `{new_policy_eligible}`"));
            self.line(&format!("- Blocking candidates: `{blocking}`"));
            self.line(&format!("- Acknowledged: `{acknowledged}`"));
            self.line(&format!("- Suppressed: `{suppressed}`"));
            self.line(&format!("- Baseline still present: `{still_present}`"));
            self.line(&format!("- Baseline resolved: `{resolved}`"));
            self.line(&format!(
                "- Baseline metadata: stale=`{metadata_stale}`, missing=`{metadata_missing}`"
            ));
            self.line(&format!("- Top debt area: `{top_area}`"));
            self.line(&format!("- Top repair route: `{top_route}`"));
            self.line(&format!("- Trend source: `{trend}`"));
            self.line("- RIPR Zero artifacts: `target/ripr/reports/ripr-zero-status.json`, `target/ripr/reports/ripr-zero-status.md`");
            self.blank();
        }
        if self.is_file(ZERO_MD) {
            self.full_report(ZERO_MD, false);
        } else if self.is_file(DELTA_JSON) {
            self.line("RIPR Zero status was not generated. Inspect `target/ripr/reports/baseline-debt-delta.json` and rerun `ripr zero status` locally.");
        } else {
            self.line("RIPR Zero status was not run. It requires `baseline-debt-delta.json`, which is produced only after an explicit gate mode and reviewed baseline are configured.");
        }
        self.blank();
    }

    fn sarif_and_badges(&mut self, upload_sarif: bool) {
        self.line("### SARIF and badge status");
        if upload_sarif {
            self.generated("- Diff SARIF", "target/ripr/reports/ripr-findings.sarif");
            self.generated("- Repo seam SARIF", "target/ripr/reports/ripr-seams.sarif");
        } else {
            self.line("- SARIF upload: disabled by `RIPR_UPLOAD_SARIF`");
        }
        self.generated("- Badge JSON", "target/ripr/reports/repo-ripr-badge.json");
        self.generated(
            "- Badge Shields JSON",
            "target/ripr/reports/repo-ripr-badge-shields.json",
        );
        self.blank();
    }

    fn generated(&mut self, label: &str, path: &str) {
        let state = if self.is_file(path) {
            "generated"
        } else {
            "missing or skipped"
        };
        self.line(&format!("{label}: {state}"));
    }

    /// These counts print bare, without `markdown_inline`, as the shell
    /// step did.
    fn guidance_annotations(&mut self) {
        self.line("### PR guidance annotations");
        if let Some(doc) = self.doc(COMMENTS_JSON) {
            let count = |path: &str| doc.field(&[path], "0", "0");
            let comments = count("summary.comments");
            let summary_only = count("summary.summary_only");
            let suppressed = count("summary.suppressed");
            self.line(&format!("- Changed-line annotations emitted: {comments}"));
            self.line(&format!("- Summary-only recommendations: {summary_only}"));
            self.line(&format!("- Suppressed recommendations: {suppressed}"));
        } else {
            self.line("No PR test guidance report was generated. When `ripr review-comments` writes `target/ripr/review/comments.json`, this workflow emits changed-line check annotations by default.");
        }
        self.blank();
    }

    fn inline_comments(&mut self, comment_mode: &str) {
        self.line("### PR inline comments");
        let mode = if comment_mode.is_empty() {
            "off"
        } else {
            comment_mode
        };
        self.line(&format!("- Mode: `{}`", inline(mode)));
        if let Some(doc) = self.doc("target/ripr/review/comment-publish-plan.json") {
            let count = |path: &str| inline(&doc.field(&[path], "0", "0"));
            let status = inline(&doc.field(&["status"], "unknown", "unknown"));
            let publishable = count("summary.publishable");
            let skipped = count("summary.skipped");
            let blocked = count("summary.blocked");
            let safe = inline(&doc.field(&["summary.safe_to_publish"], "false", "false"));
            self.line(&format!("- Status: `{status}`"));
            self.line(&format!(
                "- Counts: publishable=`{publishable}`, skipped=`{skipped}`, blocked=`{blocked}`"
            ));
            self.line(&format!("- Safe to publish: `{safe}`"));
            self.line("- Plan artifacts: `target/ripr/review/comment-publish-plan.json`, `target/ripr/review/comment-publish-plan.md`");
            self.line("- Boundary: inline comments remain opt-in; gate decisions remain separate pass/fail authority.");
            self.blank();
            if self.is_file("target/ripr/review/comment-publish-plan.md") {
                self.full_report("target/ripr/review/comment-publish-plan.md", false);
            }
        } else {
            self.line("- Inline comments are disabled by default. Set `RIPR_COMMENT_MODE` to `plan` to inspect a publish plan or `inline` to publish same-repo changed-line comments when permissions are safe.");
        }
        self.blank();
    }

    fn known_limits(&mut self) {
        self.line("### Known limits");
        self.line(
            "- Advisory static evidence only; review the named seam and write one focused test.",
        );
        self.line("- No automatic source edits or generated tests.");
        self.line("- No runtime mutation execution is performed by this workflow.");
    }

    /// The full report when it exists, otherwise the not-generated line.
    fn report_or(&mut self, path: &str, missing: &str) {
        if self.is_file(path) {
            self.full_report(path, false);
        } else {
            self.line(missing);
        }
        self.blank();
    }
}

/// `if .key then " label=" + .key else "" end`.
fn prefixed(value: &Value, key: &str, label: &str) -> Q {
    let found = jq::index(value, key)?;
    if jq::truthy(found) {
        jq::add(Value::from(label), found.clone())
    } else {
        Ok(Value::from(""))
    }
}

/// `[.list[]?.key] | if length == 0 then "none" else join(", ") end`.
fn each_field(value: &Value, list: &str, key: &str) -> Q {
    let mut found = Vec::new();
    for item in jq::each_at(value, list)? {
        found.push(jq::index(item, key)?.clone());
    }
    jq::join_or_none(&found)
}

/// `([.list[]?.key] | first) // "none"`.
fn first_field(value: &Value, list: &str, key: &str) -> Q {
    let mut found = Vec::new();
    for item in jq::each_at(value, list)? {
        found.push(jq::index(item, key)?.clone());
    }
    first_or_none(found)
}

fn first_or_none(values: Vec<Value>) -> Q {
    Ok(values
        .into_iter()
        .next()
        .filter(jq::truthy)
        .unwrap_or_else(|| Value::from("none")))
}

/// `.list[0]` on whatever the list is: null passes through, an array
/// yields its first item, anything else is a type error.
fn first_item(value: &Value) -> Result<&Value, Fail> {
    match value {
        Value::Null => Ok(&Value::Null),
        Value::Array(items) => Ok(items.first().unwrap_or(&Value::Null)),
        _ => Err(Fail),
    }
}

/// A top repair route as `path:line missing-discriminator`, or `none`.
fn repair_route(route: &Value) -> Q {
    if route.is_null() {
        return Ok(Value::from("none"));
    }
    let located = jq::location(route, "path", "unknown")?;
    let missing = jq::alt(
        route,
        &["missing_discriminator"],
        Value::from("missing discriminator unavailable"),
    )?;
    jq::add(jq::add(located, Value::from(" "))?, missing)
}

/// The per-language counts of the Language preview grouping block.
struct LanguageStats {
    artifact_entries: String,
    preview_entries: String,
    missing_preview_status: String,
    static_limit_entries: String,
    class_counts: String,
    static_limit_kinds: String,
    actionability_states: String,
    actionability_categories: String,
    repair_packet_ready: String,
}

impl LanguageStats {
    fn of(inputs: Option<&Result<Vec<Value>, Fail>>, language: &str) -> Self {
        let Some(inputs) = inputs else {
            return Self {
                artifact_entries: "0".to_string(),
                preview_entries: "0".to_string(),
                missing_preview_status: "0".to_string(),
                static_limit_entries: "0".to_string(),
                class_counts: "none".to_string(),
                static_limit_kinds: "none".to_string(),
                actionability_states: "none".to_string(),
                actionability_categories: "none".to_string(),
                repair_packet_ready: "0".to_string(),
            };
        };
        let mut objects = Vec::new();
        if let Ok(values) = inputs {
            for value in values {
                jq::objects(value, &mut objects);
            }
        }
        let wanted = Value::from(language);
        let matching = objects
            .into_iter()
            .filter(|object| object.get("language") == Some(&wanted))
            .collect::<Vec<_>>();
        let field = |object: &serde_json::Map<String, Value>, key: &str| {
            object.get(key).cloned().unwrap_or(Value::Null)
        };
        let query = |run: &dyn Fn() -> Q, fail: &str| match inputs {
            Ok(_) => run()
                .map(|value| inline(&captured(&jq::raw(&value))))
                .unwrap_or_else(|Fail| fail.to_string()),
            Err(Fail) => fail.to_string(),
        };
        let counted = |keep: &ObjectTest<'_>| {
            query(
                &|| {
                    let mut count = 0_usize;
                    for object in &matching {
                        if keep(object)? {
                            count += 1;
                        }
                    }
                    Ok(Value::from(count))
                },
                "0",
            )
        };
        let preview = Value::from("preview");
        let tallied = |key: &str| {
            query(
                &|| {
                    let mut values = Vec::new();
                    for object in &matching {
                        let nested =
                            jq::index(&field(object, "preview_actionability"), key)?.clone();
                        let value = if jq::truthy(&nested) {
                            nested
                        } else {
                            field(object, key)
                        };
                        if !value.is_null() {
                            values.push(value);
                        }
                    }
                    Ok(jq::tally(values))
                },
                "none",
            )
        };
        Self {
            artifact_entries: counted(&|_| Ok(true)),
            preview_entries: counted(&|object| Ok(field(object, "language_status") == preview)),
            missing_preview_status: counted(&|object| {
                Ok(field(object, "language_status") != preview)
            }),
            static_limit_entries: counted(&|object| {
                Ok(!field(object, "static_limit_kind").is_null())
            }),
            class_counts: query(
                &|| {
                    Ok(jq::tally(
                        matching
                            .iter()
                            .map(|object| field(object, "classification"))
                            .filter(|value| !value.is_null())
                            .collect(),
                    ))
                },
                "none",
            ),
            static_limit_kinds: query(
                &|| {
                    jq::join_or_none(&jq::unique(
                        matching
                            .iter()
                            .map(|object| field(object, "static_limit_kind"))
                            .filter(|value| !value.is_null())
                            .collect(),
                    ))
                },
                "none",
            ),
            actionability_states: tallied("gap_state"),
            actionability_categories: tallied("actionability_category"),
            repair_packet_ready: counted(&|object| {
                Ok(jq::index(
                    &field(object, "preview_actionability"),
                    "repair_packet_ready",
                )? == &Value::Bool(true))
            }),
        }
    }
}
