//! `ripr reports ci-packet`: produce the generated workflow's RIPR artifact
//! packet in one command (#4696).
//!
//! The `ripr init --ci github` workflow used to carry about thirty shell
//! steps, each running one ripr command when its input artifacts existed.
//! This command runs the same commands in the same order, under the same
//! conditions, as child processes of the running ripr binary, so the
//! adopter's workflow carries one step instead of a step graph. Each stage
//! keeps its retired step's name as a log group and keeps that step's
//! failure role:
//!
//! - an advisory stage (the step had `continue-on-error: true`) logs its
//!   failure and the packet continues;
//! - the diff capture and the gate evaluation had no `continue-on-error`,
//!   and the PR guidance and SARIF renders had it only in an advisory
//!   `RIPR_GATE_MODE`, so their failure makes the command exit nonzero once
//!   the rest of the packet is written;
//! - a stage whose step had no `always()` is skipped after such a failure,
//!   as GitHub skipped the step.
//!
//! The command reads the workflow settings (`RIPR_GATE_MODE`,
//! `RIPR_GATE_BASELINE`, `RIPR_COMMENT_MODE`, `RIPR_UPLOAD_SARIF`) and the
//! GitHub Actions run (`GITHUB_EVENT_NAME`, `GITHUB_BASE_REF`,
//! `GITHUB_EVENT_PATH`, `GITHUB_REPOSITORY`, `GITHUB_ACTOR`) from its
//! environment, as the steps did. It never receives the job token: posting
//! inline comments stays a workflow step that holds it.

use crate::agent::loop_commands::{
    EDITOR_AGENT_BRIEF_ARTIFACT, EDITOR_AGENT_PACKET_ARTIFACT, PILOT_BEFORE_SNAPSHOT_ARTIFACT,
    WORKFLOW_AFTER_SNAPSHOT_ARTIFACT, WORKFLOW_AGENT_BRIEF_ARTIFACT,
    WORKFLOW_AGENT_PACKET_ARTIFACT, WORKFLOW_AGENT_RECEIPT_ARTIFACT,
    WORKFLOW_AGENT_REVIEW_SUMMARY_ARTIFACT, WORKFLOW_AGENT_REVIEW_SUMMARY_MARKDOWN_ARTIFACT,
    WORKFLOW_AGENT_SEAM_PACKETS_ARTIFACT, WORKFLOW_AGENT_STATUS_ARTIFACT,
    WORKFLOW_AGENT_STATUS_MARKDOWN_ARTIFACT, WORKFLOW_AGENT_VERIFY_ARTIFACT,
    WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT,
};
use crate::cli::suggest::unknown_argument;
use crate::output::ci_summary::annotations::render_workflow_annotations;
use crate::process_owner::OwnedProcess;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::non_empty_string_arg;

const COMMAND: &str = "reports ci-packet";

const REPORTS: &str = "target/ripr/reports";
const COMMENTS_JSON: &str = "target/ripr/review/comments.json";
const REPO_EXPOSURE: &str = "target/ripr/reports/repo-exposure.json";
const GATE_DECISION: &str = "target/ripr/reports/gate-decision.json";
const BASELINE_DELTA: &str = "target/ripr/reports/baseline-debt-delta.json";
const ZERO_STATUS: &str = "target/ripr/reports/ripr-zero-status.json";
const PR_LEDGER: &str = "target/ripr/reports/pr-evidence-ledger.json";
const LABELS_JSON: &str = "target/ci/labels.json";
const RECOMMENDATION_CALIBRATION: &str = "target/ripr/reports/recommendation-calibration.json";
const MUTATION_CALIBRATION: &str = "target/ripr/reports/mutation-calibration.json";
const COVERAGE_FRONTIER: &str = "target/ripr/reports/coverage-grip-frontier.json";
const ASSISTANT_PROOF: &str = "target/ripr/reports/test-oracle-assistant-proof.json";
const POLICY_OPERATIONS: &str = "target/ripr/reports/policy-operations.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CiPacketOptions {
    pub(super) root: PathBuf,
    /// `--step NAME`: run only these steps, each still under its condition,
    /// to reproduce one step's failure without the rest of the packet.
    pub(super) steps: Vec<String>,
}

pub(super) fn ci_packet(args: &[String]) -> Result<(), String> {
    let options = parse_ci_packet_options(args)?;
    let exe = std::env::current_exe()
        .map_err(|err| format!("{COMMAND} cannot locate the running ripr binary: {err}"))?;
    let settings = CiSettings::from_env()?;
    let mut run = PacketRun {
        root: options.root,
        exe,
        settings,
        only: options.steps,
        seen: Vec::new(),
        top_seam: None,
        failed: Vec::new(),
        recorded: None,
    };
    run.all_stages();
    let unknown = run
        .only
        .iter()
        .filter(|step| !run.seen.iter().any(|(name, _, _)| name == *step))
        .cloned()
        .collect::<Vec<_>>();
    if !unknown.is_empty() {
        return Err(format!(
            "{COMMAND}: unknown --step {}; the steps are: {}",
            unknown.join(", "),
            run.seen
                .iter()
                .map(|(name, _, _)| name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if run.failed.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{COMMAND}: required step(s) failed: {}. The rest of the packet was written; see each step's log group above.",
            run.failed.join(", ")
        ))
    }
}

pub(super) fn parse_ci_packet_options(args: &[String]) -> Result<CiPacketOptions, String> {
    let mut options = CiPacketOptions {
        root: PathBuf::from("."),
        steps: Vec::new(),
    };
    let mut i = 0usize;
    while i < args.len() {
        let flag = args[i].as_str();
        i += 1;
        match flag {
            "--root" => options.root = PathBuf::from(non_empty_string_arg(args, i, flag, COMMAND)?),
            "--step" => options
                .steps
                .push(non_empty_string_arg(args, i, flag, COMMAND)?),
            other => return Err(unknown_argument(COMMAND, other)),
        }
        i += 1;
    }
    Ok(options)
}

/// The workflow settings and GitHub run facts the retired steps read
/// through `env.*` and `github.*` expressions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CiSettings {
    pub(super) gate_mode: String,
    pub(super) gate_baseline: String,
    pub(super) comment_mode: String,
    pub(super) upload_sarif: String,
    /// `github.event_name`.
    pub(super) event_name: String,
    /// `github.base_ref`: empty outside a pull request.
    pub(super) base_ref: String,
    /// `github.event.repository.default_branch`.
    pub(super) default_branch: String,
    /// `github.event.pull_request.number`.
    pub(super) pr_number: String,
    /// `github.event.number`.
    pub(super) event_number: String,
    /// `github.event.pull_request.head.repo.full_name`.
    pub(super) head_repo: String,
    /// `github.repository`.
    pub(super) repository: String,
    /// `github.actor`.
    pub(super) actor: String,
    /// `github.event.pull_request.user.login`.
    pub(super) pr_author: String,
    /// `.pull_request.labels[]?.name` of the event, or why it could not be
    /// read.
    pub(super) labels: Result<Vec<Value>, String>,
}

impl Default for CiSettings {
    fn default() -> Self {
        Self {
            gate_mode: String::new(),
            gate_baseline: String::new(),
            comment_mode: String::new(),
            upload_sarif: String::new(),
            event_name: String::new(),
            base_ref: String::new(),
            default_branch: String::new(),
            pr_number: String::new(),
            event_number: String::new(),
            head_repo: String::new(),
            repository: String::new(),
            actor: String::new(),
            pr_author: String::new(),
            labels: Ok(Vec::new()),
        }
    }
}

impl CiSettings {
    pub(super) fn from_env() -> Result<Self, String> {
        let var = |name: &str| std::env::var(name).unwrap_or_default();
        let event_path = var("GITHUB_EVENT_PATH");
        let event = if event_path.is_empty() {
            Ok(Value::Null)
        } else {
            fs::read(&event_path)
                .map_err(|err| format!("cannot read GITHUB_EVENT_PATH {event_path}: {err}"))
                .and_then(|bytes| {
                    serde_json::from_slice::<Value>(&bytes)
                        .map_err(|err| format!("GITHUB_EVENT_PATH {event_path} is not JSON: {err}"))
                })
        };
        let mut settings = Self::from_event(event.as_ref().unwrap_or(&Value::Null));
        if let Err(err) = event {
            settings.labels = Err(err);
        }
        settings.gate_mode = var("RIPR_GATE_MODE");
        settings.gate_baseline = var("RIPR_GATE_BASELINE");
        settings.comment_mode = var("RIPR_COMMENT_MODE");
        settings.upload_sarif = var("RIPR_UPLOAD_SARIF");
        settings.event_name = var("GITHUB_EVENT_NAME");
        settings.base_ref = var("GITHUB_BASE_REF");
        settings.repository = var("GITHUB_REPOSITORY");
        settings.actor = var("GITHUB_ACTOR");
        Ok(settings)
    }

    /// The `github.event.*` facts, read the way an expression reads them:
    /// a missing field is empty.
    pub(super) fn from_event(event: &Value) -> Self {
        let text = |pointer: &str| match event.pointer(pointer) {
            Some(Value::String(text)) => text.clone(),
            Some(Value::Number(number)) => number.to_string(),
            _ => String::new(),
        };
        let labels = match event.pointer("/pull_request/labels") {
            None | Some(Value::Null) => Ok(Vec::new()),
            Some(Value::Array(items)) => items
                .iter()
                .map(|label| match label {
                    Value::Object(map) => Ok(map.get("name").cloned().unwrap_or(Value::Null)),
                    Value::Null => Ok(Value::Null),
                    _ => Err("a pull_request label is not an object".to_string()),
                })
                .collect(),
            Some(Value::Object(map)) => Ok(map
                .values()
                .map(|label| label.get("name").cloned().unwrap_or(Value::Null))
                .collect()),
            Some(_) => Ok(Vec::new()),
        };
        Self {
            default_branch: text("/repository/default_branch"),
            pr_number: text("/pull_request/number"),
            event_number: text("/number"),
            head_repo: text("/pull_request/head/repo/full_name"),
            pr_author: text("/pull_request/user/login"),
            labels,
            ..Self::default()
        }
    }

    fn pull_request(&self) -> bool {
        self.event_name == "pull_request"
    }

    /// The jobs-level `continue-on-error` expression: the gate is advisory
    /// unless `RIPR_GATE_MODE` names a blocking mode.
    fn advisory(&self) -> bool {
        self.gate_mode.is_empty() || self.gate_mode == "visible-only"
    }
}

/// How a retired step's failure counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Role {
    /// `continue-on-error: true`.
    Advisory,
    /// `continue-on-error` only while the gate is advisory (#2009).
    GateCritical,
    /// No `continue-on-error`.
    Required,
}

/// Whether a retired step ran after an earlier failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum When {
    /// `if: always() && ...`.
    Always,
    /// No `always()`: skipped after a failure.
    Success,
}

struct PacketRun {
    root: PathBuf,
    exe: PathBuf,
    settings: CiSettings,
    /// `--step` names; empty runs every step.
    only: Vec<String>,
    /// Every step, in order, whether or not it ran.
    seen: Vec<(String, When, Role)>,
    top_seam: Option<String>,
    /// Names of failed steps whose failure fails the job.
    failed: Vec<String>,
    /// Test seam: when set, child commands and the diff capture are recorded
    /// as command lines instead of run, so tests can hold each step's argv.
    recorded: Option<std::cell::RefCell<Vec<String>>>,
}

type StageResult = Result<(), String>;

impl PacketRun {
    /// A regular file, as `[ -f ]` and `hashFiles()` tested: a directory at
    /// an input path does not count.
    fn exists(&self, relative: &str) -> bool {
        self.root.join(relative).is_file()
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    fn mkdir(&self, relative: &str) -> StageResult {
        fs::create_dir_all(self.path(relative))
            .map_err(|err| format!("cannot create {relative}: {err}"))
    }

    fn copy(&self, from: &str, to: &str) -> StageResult {
        fs::copy(self.path(from), self.path(to))
            .map(|_| ())
            .map_err(|err| format!("cannot copy {from} to {to}: {err}"))
    }

    fn stage(
        &mut self,
        name: &str,
        when: When,
        role: Role,
        condition: bool,
        body: impl FnOnce(&mut Self) -> StageResult,
    ) {
        self.seen.push((name.to_string(), when, role));
        if !self.only.is_empty() && !self.only.iter().any(|step| step == name) {
            return;
        }
        if !condition || (when == When::Success && !self.failed.is_empty()) {
            // A step named with --step says why it did not run, so a skip
            // is not mistaken for a pass.
            if !self.only.is_empty() {
                let reason = if condition {
                    "an earlier required step failed"
                } else {
                    "its inputs or settings are absent"
                };
                println!("RIPR step \"{name}\" skipped: {reason}.");
            }
            return;
        }
        println!("::group::{name}");
        let result = body(self);
        println!("::endgroup::");
        let Err(err) = result else {
            return;
        };
        let continues = match role {
            Role::Advisory => true,
            Role::GateCritical => self.settings.advisory(),
            Role::Required => false,
        };
        if continues {
            println!(
                "RIPR step \"{name}\" failed and is advisory; continuing: {}",
                workflow_data(&err)
            );
        } else {
            println!("::error title={name}::{}", workflow_data(&err));
            self.failed.push(name.to_string());
        }
    }

    /// `ripr ARGS` from the workspace root, its output in the job log.
    fn ripr(&self, args: &[String]) -> StageResult {
        self.spawn(args, Stdio::inherit())
    }

    /// `ripr ARGS > OUT`: the file is truncated before the command runs, as
    /// the shell redirect did.
    fn ripr_to(&self, args: &[String], out: &str) -> StageResult {
        let file =
            fs::File::create(self.path(out)).map_err(|err| format!("cannot write {out}: {err}"))?;
        if let Some(recorded) = &self.recorded {
            recorded
                .borrow_mut()
                .push(format!("ripr {} > {out}", args.join(" ")));
            return Ok(());
        }
        self.spawn(args, Stdio::from(file))
    }

    fn spawn(&self, args: &[String], stdout: Stdio) -> StageResult {
        if let Some(recorded) = &self.recorded {
            recorded
                .borrow_mut()
                .push(format!("ripr {}", args.join(" ")));
            return Ok(());
        }
        let _ = std::io::stdout().flush();
        let mut command = Command::new(&self.exe);
        command
            .args(args)
            .current_dir(&self.root)
            .stdin(Stdio::null())
            .stdout(stdout)
            .stderr(Stdio::inherit());
        let shown = format!("ripr {}", args.join(" "));
        let status = OwnedProcess::spawn(command)
            .and_then(|mut child| child.wait())
            .map_err(|err| format!("`{shown}` could not run: {err}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(match status.code() {
                Some(code) => format!("`{shown}` exited {code}"),
                None => format!("`{shown}` was terminated by a signal"),
            })
        }
    }

    fn all_stages(&mut self) {
        let pr = self.settings.pull_request();

        self.stage(
            "Generate RIPR pilot packet",
            When::Success,
            Role::Advisory,
            true,
            |run| {
                run.ripr(&args(&[
                    "pilot",
                    "--root",
                    ".",
                    "--out",
                    "target/ripr/pilot",
                    "--mode",
                    "ready",
                    "--max-seams",
                    "5",
                ]))
            },
        );

        self.stage(
            "Prepare RIPR editor-agent artifacts",
            When::Always,
            Role::Advisory,
            true,
            Self::prepare_editor_agent_artifacts,
        );

        // Run alone with --step, the agent loop reads the seam the prepare
        // step would have set; a full run keeps the prepare step's answer.
        let top_seam = if self.only.is_empty() {
            self.top_seam.clone()
        } else {
            fs::read(self.path("target/ripr/pilot/pilot-summary.json"))
                .ok()
                .and_then(|bytes| top_seam_id(&bytes))
        };
        self.stage(
            "Generate RIPR agent loop artifacts",
            When::Always,
            Role::Advisory,
            top_seam.is_some(),
            |run| run.agent_loop(top_seam.as_deref().unwrap_or_default()),
        );

        self.stage(
            "Render RIPR gap decision ledger",
            When::Always,
            Role::Advisory,
            self.exists(REPO_EXPOSURE),
            |run| {
                run.mkdir(REPORTS)?;
                run.ripr(&args(&[
                    "reports",
                    "gap-ledger",
                    "--root",
                    ".",
                    "--repo-exposure",
                    REPO_EXPOSURE,
                    "--out",
                    "target/ripr/reports/gap-decision-ledger.json",
                    "--out-md",
                    "target/ripr/reports/gap-decision-ledger.md",
                ]))
            },
        );

        self.stage(
            "Capture pull request diff",
            When::Success,
            Role::Required,
            pr,
            Self::capture_pull_request_diff,
        );

        self.stage(
            "Run RIPR PR guidance report",
            When::Success,
            Role::GateCritical,
            pr,
            Self::pr_guidance,
        );

        self.stage(
            "Plan RIPR inline comments",
            When::Always,
            Role::Advisory,
            pr && self.settings.comment_mode != "off" && self.exists(COMMENTS_JSON),
            Self::plan_inline_comments,
        );

        self.stage(
            "Capture RIPR gate labels",
            When::Always,
            Role::Advisory,
            pr,
            Self::capture_labels,
        );

        let sarif = self.settings.upload_sarif == "true";
        self.stage(
            "Render RIPR diff SARIF",
            When::Success,
            Role::GateCritical,
            sarif && pr,
            |run| {
                run.ripr_to(
                    &args(&[
                        "check",
                        "--root",
                        ".",
                        "--diff",
                        "target/ripr/reports/pr.diff",
                        "--format",
                        "sarif",
                    ]),
                    "target/ripr/reports/ripr-findings.sarif",
                )
            },
        );

        self.stage(
            "Render RIPR repo seam SARIF",
            When::Success,
            Role::GateCritical,
            sarif,
            |run| {
                run.mkdir(REPORTS)?;
                run.ripr_to(
                    &args(&[
                        "check",
                        "--root",
                        ".",
                        "--mode",
                        "ready",
                        "--format",
                        "repo-sarif",
                    ]),
                    "target/ripr/reports/ripr-seams.sarif",
                )
            },
        );

        // These files are uploaded with the run; they do not update a README
        // badge endpoint. Publishing a badge is a separate reviewed workflow
        // (docs/BADGE_ADOPTION.md).
        self.stage(
            "Render RIPR repo badge artifacts",
            When::Success,
            Role::Advisory,
            true,
            |run| {
                run.mkdir(REPORTS)?;
                run.ripr_to(
                    &args(&[
                        "check",
                        "--root",
                        ".",
                        "--mode",
                        "ready",
                        "--format",
                        "repo-badge-json",
                    ]),
                    "target/ripr/reports/repo-ripr-badge.json",
                )?;
                run.ripr_to(
                    &args(&[
                        "check",
                        "--root",
                        ".",
                        "--mode",
                        "ready",
                        "--format",
                        "repo-badge-shields",
                    ]),
                    "target/ripr/reports/repo-ripr-badge-shields.json",
                )
            },
        );

        self.stage(
            "Evaluate RIPR gate decision",
            When::Always,
            Role::Required,
            !self.settings.gate_mode.is_empty() && self.exists(COMMENTS_JSON),
            Self::evaluate_gate,
        );

        self.stage(
            "Render RIPR baseline debt delta",
            When::Always,
            Role::Advisory,
            !self.settings.gate_baseline.is_empty() && self.exists(GATE_DECISION),
            |run| {
                run.mkdir(REPORTS)?;
                let baseline = run.settings.gate_baseline.clone();
                run.ripr(&args(&[
                    "baseline",
                    "diff",
                    "--baseline",
                    &baseline,
                    "--current",
                    GATE_DECISION,
                    "--out",
                    BASELINE_DELTA,
                    "--out-md",
                    "target/ripr/reports/baseline-debt-delta.md",
                ]))
            },
        );

        self.stage(
            "Render RIPR Zero status",
            When::Always,
            Role::Advisory,
            self.exists(BASELINE_DELTA),
            |run| {
                run.mkdir(REPORTS)?;
                let mut argv = args(&[
                    "zero",
                    "status",
                    "--delta",
                    BASELINE_DELTA,
                    "--out",
                    ZERO_STATUS,
                    "--out-md",
                    "target/ripr/reports/ripr-zero-status.md",
                ]);
                if !run.settings.gate_baseline.is_empty() {
                    argv.extend(args(&["--baseline", &run.settings.gate_baseline]));
                }
                run.push_present(
                    &mut argv,
                    &[
                        ("--gate", GATE_DECISION),
                        ("--pr-guidance", COMMENTS_JSON),
                        ("--recommendation-calibration", RECOMMENDATION_CALIBRATION),
                    ],
                );
                run.ripr(&argv)
            },
        );

        self.stage(
            "Render RIPR PR evidence ledger",
            When::Always,
            Role::Advisory,
            pr && self.exists(COMMENTS_JSON),
            Self::pr_evidence_ledger,
        );

        self.stage(
            "Render RIPR waiver aging",
            When::Always,
            Role::Advisory,
            self.exists(PR_LEDGER),
            |run| {
                run.mkdir(REPORTS)?;
                let mut argv = args(&[
                    "policy",
                    "waiver-aging",
                    "--root",
                    ".",
                    "--ledger",
                    PR_LEDGER,
                    "--out",
                    "target/ripr/reports/waiver-aging.json",
                    "--out-md",
                    "target/ripr/reports/waiver-aging.md",
                ]);
                run.push_present(
                    &mut argv,
                    &[("--history", ".ripr/pr-evidence-ledger.jsonl")],
                );
                run.ripr(&argv)
            },
        );

        self.stage(
            "Render RIPR suppression health",
            When::Always,
            Role::Advisory,
            true,
            |run| {
                run.mkdir(REPORTS)?;
                run.ripr(&args(&[
                    "policy",
                    "suppression-health",
                    "--root",
                    ".",
                    "--out",
                    "target/ripr/reports/suppression-health.json",
                    "--out-md",
                    "target/ripr/reports/suppression-health.md",
                ]))
            },
        );

        self.stage(
            "Render RIPR policy readiness",
            When::Always,
            Role::Advisory,
            true,
            |run| {
                run.mkdir(REPORTS)?;
                let mut argv = args(&[
                    "policy",
                    "readiness",
                    "--root",
                    ".",
                    "--out",
                    "target/ripr/reports/policy-readiness.json",
                    "--out-md",
                    "target/ripr/reports/policy-readiness.md",
                ]);
                run.push_present(
                    &mut argv,
                    &[
                        ("--gate-decision", GATE_DECISION),
                        ("--baseline-delta", BASELINE_DELTA),
                        ("--recommendation-calibration", RECOMMENDATION_CALIBRATION),
                        ("--mutation-calibration", MUTATION_CALIBRATION),
                        ("--waiver-aging", "target/ripr/reports/waiver-aging.json"),
                        (
                            "--suppression-health",
                            "target/ripr/reports/suppression-health.json",
                        ),
                    ],
                );
                run.ripr(&argv)
            },
        );

        self.stage(
            "Render RIPR policy operations",
            When::Always,
            Role::Advisory,
            self.exists("target/ripr/reports/policy-readiness.json"),
            |run| {
                run.mkdir(REPORTS)?;
                let mut argv = args(&[
                    "policy",
                    "operations",
                    "--root",
                    ".",
                    "--policy-readiness",
                    "target/ripr/reports/policy-readiness.json",
                    "--out",
                    POLICY_OPERATIONS,
                    "--out-md",
                    "target/ripr/reports/policy-operations.md",
                ]);
                run.push_present(
                    &mut argv,
                    &[
                        ("--waiver-aging", "target/ripr/reports/waiver-aging.json"),
                        (
                            "--suppression-health",
                            "target/ripr/reports/suppression-health.json",
                        ),
                        ("--baseline-delta", BASELINE_DELTA),
                        ("--gate-decision", GATE_DECISION),
                        ("--recommendation-calibration", RECOMMENDATION_CALIBRATION),
                        ("--mutation-calibration", MUTATION_CALIBRATION),
                        ("--preview-boundary", REPO_EXPOSURE),
                    ],
                );
                run.ripr(&argv)
            },
        );

        self.stage(
            "Render RIPR policy history",
            When::Always,
            Role::Advisory,
            self.exists(POLICY_OPERATIONS),
            |run| {
                run.mkdir(REPORTS)?;
                // Record the analyzed commit. On a PR, GITHUB_SHA names the
                // merge commit, which the workflow does not check out.
                let commit = if run.recorded.is_some() {
                    "HEAD".to_string()
                } else {
                    crate::git::run_git(&run.root, &["rev-parse", "HEAD"])?
                };
                let mut argv = args(&[
                    "policy",
                    "history",
                    "--root",
                    ".",
                    "--current",
                    POLICY_OPERATIONS,
                    "--commit",
                    &commit,
                    "--out",
                    "target/ripr/reports/policy-history.json",
                    "--out-md",
                    "target/ripr/reports/policy-history.md",
                ]);
                run.push_present(&mut argv, &[("--history", ".ripr/policy-history.jsonl")]);
                if run.settings.pull_request() {
                    argv.extend(args(&["--pr-number", &run.settings.event_number]));
                }
                run.ripr(&argv)
            },
        );

        self.stage(
            "Render RIPR policy promotion packets",
            When::Always,
            Role::Advisory,
            self.exists(POLICY_OPERATIONS),
            |run| {
                run.mkdir(REPORTS)?;
                for target_mode in [
                    "visible-only",
                    "acknowledgeable",
                    "baseline-check",
                    "calibrated-gate",
                ] {
                    let mut argv = args(&[
                        "policy",
                        "promote",
                        "--to",
                        target_mode,
                        "--operations",
                        POLICY_OPERATIONS,
                        "--out",
                        &format!("target/ripr/reports/policy-promotion-{target_mode}.json"),
                        "--out-md",
                        &format!("target/ripr/reports/policy-promotion-{target_mode}.md"),
                    ]);
                    run.push_present(
                        &mut argv,
                        &[("--history", "target/ripr/reports/policy-history.json")],
                    );
                    run.ripr(&argv)?;
                }
                Ok(())
            },
        );

        self.stage(
            "Render RIPR preview promotion packets",
            When::Always,
            Role::Advisory,
            true,
            Self::preview_promotion_packets,
        );

        self.stage(
            "Render RIPR test-oracle assistant proof",
            When::Always,
            Role::Advisory,
            [
                COMMENTS_JSON,
                WORKFLOW_AGENT_BRIEF_ARTIFACT,
                WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT,
                WORKFLOW_AFTER_SNAPSHOT_ARTIFACT,
                WORKFLOW_AGENT_RECEIPT_ARTIFACT,
                PR_LEDGER,
            ]
            .iter()
            .all(|path| self.exists(path)),
            |run| {
                run.mkdir(REPORTS)?;
                let mut argv = args(&[
                    "assistant-loop",
                    "proof",
                    "--root",
                    ".",
                    "--pr-guidance",
                    COMMENTS_JSON,
                    "--agent-packet",
                    WORKFLOW_AGENT_BRIEF_ARTIFACT,
                    "--before",
                    WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT,
                    "--after",
                    WORKFLOW_AFTER_SNAPSHOT_ARTIFACT,
                    "--receipt",
                    WORKFLOW_AGENT_RECEIPT_ARTIFACT,
                    "--ledger",
                    PR_LEDGER,
                    "--out",
                    ASSISTANT_PROOF,
                    "--out-md",
                    "target/ripr/reports/test-oracle-assistant-proof.md",
                ]);
                run.push_present(
                    &mut argv,
                    &[
                        ("--coverage-frontier", COVERAGE_FRONTIER),
                        ("--gate-decision", GATE_DECISION),
                    ],
                );
                run.ripr(&argv)
            },
        );

        self.stage(
            "Render RIPR assistant loop health",
            When::Always,
            Role::Advisory,
            self.exists(ASSISTANT_PROOF),
            |run| {
                run.mkdir(REPORTS)?;
                run.ripr(&args(&[
                    "assistant-loop",
                    "health",
                    "--root",
                    ".",
                    "--proof",
                    ASSISTANT_PROOF,
                    "--out",
                    "target/ripr/reports/assistant-loop-health.json",
                    "--out-md",
                    "target/ripr/reports/assistant-loop-health.md",
                ]))
            },
        );

        self.stage(
            "Render RIPR first useful action",
            When::Always,
            Role::Advisory,
            true,
            Self::first_useful_action,
        );

        self.stage(
            "Render RIPR PR review front panel",
            When::Always,
            Role::Advisory,
            true,
            Self::pr_review_front_panel,
        );

        // first-pr checks its base resolves and that the review cards were
        // built for the same base. Without --base it resolves the default
        // branch, which is not the base of a PR into another branch, so pass
        // the PR base. A manual run has no PR base; use the default branch.
        self.stage(
            "Render RIPR first-pr start-here",
            When::Always,
            Role::Advisory,
            true,
            |run| {
                run.mkdir(REPORTS)?;
                let base = if run.settings.base_ref.is_empty() {
                    run.settings.default_branch.clone()
                } else {
                    run.settings.base_ref.clone()
                };
                run.ripr(&args(&[
                    "first-pr",
                    "--root",
                    ".",
                    "--base",
                    &format!("origin/{base}"),
                    "--head",
                    "HEAD",
                    "--gap-ledger",
                    "target/ripr/reports/gap-decision-ledger.json",
                    "--first-action",
                    "target/ripr/reports/first-useful-action.json",
                    "--review-comments",
                    COMMENTS_JSON,
                    "--agent-packet",
                    WORKFLOW_AGENT_PACKET_ARTIFACT,
                    "--gate-decision",
                    GATE_DECISION,
                    "--receipts-dir",
                    "target/ripr/receipts",
                    "--out-dir",
                    REPORTS,
                ]))
            },
        );

        self.stage(
            "Render RIPR report packet index",
            When::Always,
            Role::Advisory,
            true,
            Self::report_packet_index,
        );

        self.stage(
            "Render RIPR LLM work-loop summaries",
            When::Always,
            Role::Advisory,
            true,
            |run| {
                run.mkdir("target/ripr/workflow")?;
                run.ripr_to(
                    &args(&["agent", "status", "--root", ".", "--json"]),
                    WORKFLOW_AGENT_STATUS_ARTIFACT,
                )?;
                run.ripr_to(
                    &args(&["agent", "status", "--root", "."]),
                    WORKFLOW_AGENT_STATUS_MARKDOWN_ARTIFACT,
                )?;
                run.ripr_to(
                    &args(&["agent", "review-summary", "--root", ".", "--json"]),
                    WORKFLOW_AGENT_REVIEW_SUMMARY_ARTIFACT,
                )?;
                run.ripr_to(
                    &args(&["agent", "review-summary", "--root", "."]),
                    WORKFLOW_AGENT_REVIEW_SUMMARY_MARKDOWN_ARTIFACT,
                )
            },
        );

        // Green-with-missing-artifacts is a real failure mode (#2009): report
        // it visibly without failing the advisory packet. This runs before
        // the annotations so GitHub's per-step annotation cap cannot drop it.
        self.stage(
            "Check RIPR advisory artifacts",
            When::Always,
            Role::Advisory,
            true,
            |run| {
                run.check_advisory_artifacts();
                Ok(())
            },
        );

        self.stage(
            "Emit RIPR PR guidance annotations",
            When::Always,
            Role::Advisory,
            self.exists(COMMENTS_JSON),
            Self::emit_annotations,
        );
    }

    /// Append `FLAG PATH` for each path that exists.
    fn push_present(&self, argv: &mut Vec<String>, optional: &[(&str, &str)]) {
        for (flag, path) in optional {
            if self.exists(path) {
                argv.push((*flag).to_string());
                argv.push((*path).to_string());
            }
        }
    }

    /// Append `FLAG PATH` for each path that exists, and say whether any did.
    fn push_inputs(&self, argv: &mut Vec<String>, optional: &[(&str, &str)]) -> bool {
        let before = argv.len();
        self.push_present(argv, optional);
        argv.len() > before
    }

    fn prepare_editor_agent_artifacts(&mut self) -> StageResult {
        self.mkdir(REPORTS)?;
        self.mkdir("target/ripr/agent")?;
        self.mkdir("target/ripr/workflow")?;
        if self.exists(PILOT_BEFORE_SNAPSHOT_ARTIFACT) {
            self.copy(PILOT_BEFORE_SNAPSHOT_ARTIFACT, REPO_EXPOSURE)?;
            self.copy(
                PILOT_BEFORE_SNAPSHOT_ARTIFACT,
                WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT,
            )?;
        }
        if self.exists("target/ripr/pilot/agent-seam-packets.json") {
            self.copy(
                "target/ripr/pilot/agent-seam-packets.json",
                WORKFLOW_AGENT_SEAM_PACKETS_ARTIFACT,
            )?;
        }
        if let Ok(bytes) = fs::read(self.path("target/ripr/pilot/pilot-summary.json")) {
            self.top_seam = top_seam_id(&bytes);
        }
        Ok(())
    }

    /// CI writes the before side of the repair loop only: the workflow
    /// manifest, brief, and packet the focused-test edit starts from. The
    /// after snapshot, verify, and receipt need that edit between the
    /// snapshots, so the repair's `--attempt ... --phase after` command
    /// produces them where the edit happens (#3906). The packet lands through
    /// a temporary file so a failed render never leaves an empty JSON
    /// artifact for later stages or the upload.
    fn agent_loop(&mut self, seam: &str) -> StageResult {
        // `--step` can run this stage without the preparation stage, so it
        // creates both output directories itself.
        self.mkdir("target/ripr/workflow")?;
        self.mkdir("target/ripr/agent")?;
        self.ripr(&args(&[
            "agent",
            "start",
            "--root",
            ".",
            "--seam-id",
            seam,
            "--out",
            "target/ripr/workflow",
        ]))?;
        let pending = format!("{WORKFLOW_AGENT_PACKET_ARTIFACT}.partial");
        let rendered = self.ripr_to(
            &args(&[
                "agent",
                "packet",
                "--root",
                ".",
                "--seam-id",
                seam,
                "--json",
            ]),
            &pending,
        );
        if let Err(err) = rendered {
            let _ = fs::remove_file(self.path(&pending));
            return Err(err);
        }
        fs::rename(
            self.path(&pending),
            self.path(WORKFLOW_AGENT_PACKET_ARTIFACT),
        )
        .map_err(|err| format!("cannot move the agent packet into place: {err}"))?;
        self.copy(WORKFLOW_AGENT_PACKET_ARTIFACT, EDITOR_AGENT_PACKET_ARTIFACT)?;
        self.copy(WORKFLOW_AGENT_BRIEF_ARTIFACT, EDITOR_AGENT_BRIEF_ARTIFACT)
    }

    /// Pinned diff contract (#4005): the same presentation pins as the
    /// production loaders. Ambient external-diff, textconv, color, context,
    /// path-quoting, and side-prefix configuration must not change the bytes
    /// RIPR analyzes.
    fn capture_pull_request_diff(&mut self) -> StageResult {
        self.mkdir(REPORTS)?;
        let base_ref = format!("origin/{}", self.settings.base_ref);
        if let Some(recorded) = &self.recorded {
            recorded.borrow_mut().push(format!(
                "capture {base_ref}...HEAD > target/ripr/reports/pr.diff"
            ));
            return Ok(());
        }
        let base_sha = crate::git::run_git(
            &self.root,
            &["rev-parse", "--verify", &format!("{base_ref}^{{commit}}")],
        )
        .map_err(|err| format!("ripr: cannot resolve base ref {base_ref}: {err}"))?;
        let head_sha = crate::git::run_git(&self.root, &["rev-parse", "--verify", "HEAD^{commit}"])
            .map_err(|err| format!("ripr: cannot resolve HEAD: {err}"))?;
        let range = format!("{base_sha}...{head_sha}");
        let diff = git_bytes(
            &self.root,
            &[
                "-c",
                "core.quotePath=true",
                "diff",
                "--binary",
                "--no-ext-diff",
                "--no-textconv",
                "--no-color",
                "--src-prefix=a/",
                "--dst-prefix=b/",
                "--unified=3",
                "--inter-hunk-context=0",
                &range,
            ],
        )
        .map_err(|err| format!("ripr: git diff failed for {range}: {err}"))?;
        fs::write(self.path("target/ripr/reports/pr.diff"), &diff)
            .map_err(|err| format!("cannot write target/ripr/reports/pr.diff: {err}"))?;
        let receipt = DiffReceipt {
            tool: "ripr",
            kind: "pr-diff-receipt",
            base_ref: &base_ref,
            base_sha: &base_sha,
            head_sha: &head_sha,
            byte_count: diff.len(),
            sha256: &hex(&Sha256::digest(&diff)),
        };
        let mut json = serde_json::to_string_pretty(&receipt)
            .map_err(|err| format!("cannot render the diff receipt: {err}"))?;
        json.push('\n');
        fs::write(self.path("target/ripr/reports/pr-diff.receipt.json"), json).map_err(|err| {
            format!("cannot write target/ripr/reports/pr-diff.receipt.json: {err}")
        })?;
        if diff.is_empty() {
            let names = git_bytes(
                &self.root,
                &[
                    "-c",
                    "core.quotePath=true",
                    "diff",
                    "--name-only",
                    "-z",
                    &range,
                ],
            )
            .map_err(|err| format!("ripr: git diff --name-only failed for {range}: {err}"))?;
            let changed = names.iter().filter(|byte| **byte == 0).count();
            if changed != 0 {
                return Err(format!(
                    "ripr: empty patch but {changed} changed path(s); refusing an absent result"
                ));
            }
        }
        Ok(())
    }

    /// Gate-critical producer (#2009): advisory by default, but a blocking
    /// `RIPR_GATE_MODE` must not green-on-error past the gate's own input.
    fn pr_guidance(&mut self) -> StageResult {
        self.mkdir("target/ripr/pr")?;
        self.mkdir("target/ripr/review")?;
        let base = format!("origin/{}", self.settings.base_ref);
        if let Err(err) = self.ripr_to(
            &args(&["check", "--root", ".", "--base", &base, "--format", "json"]),
            "target/ripr/pr/check.json",
        ) {
            println!(
                "RIPR check did not produce a complete result ({}); review-comments will fail closed on the named artifact.",
                workflow_data(&err)
            );
        }
        self.ripr(&args(&[
            "review-comments",
            "--root",
            ".",
            "--base",
            &base,
            "--head",
            "HEAD",
            "--check-output",
            "target/ripr/pr/check.json",
            "--out",
            COMMENTS_JSON,
        ]))
    }

    fn plan_inline_comments(&mut self) -> StageResult {
        self.mkdir("target/ripr/review")?;
        let settings = &self.settings;
        let mut argv = args(&[
            "pr-comments",
            "plan",
            "--root",
            ".",
            "--pr-guidance",
            COMMENTS_JSON,
            "--mode",
            &settings.comment_mode,
            "--event-name",
            &settings.event_name,
            "--pull-request",
            &settings.pr_number,
            "--head-repo",
            &settings.head_repo,
            "--base-repo",
            &settings.repository,
            "--out",
            "target/ripr/review/comment-publish-plan.json",
            "--out-md",
            "target/ripr/review/comment-publish-plan.md",
        ]);
        self.push_present(
            &mut argv,
            &[(
                "--existing-comments",
                "target/ripr/review/existing-comments.json",
            )],
        );
        // The publish step holds the job token (`github.token`), which GitHub
        // always provides; this command never sees the token itself.
        argv.push("--token-available".to_string());
        // GitHub gives Dependabot runs a read-only token whatever the
        // permissions block says, so the plan must not claim write. Check
        // the PR author too: a maintainer who reopens a Dependabot PR is the
        // event actor, and the run can still carry the read-only token.
        if settings.actor == "dependabot[bot]" || settings.pr_author == "dependabot[bot]" {
            argv.push("--no-write-permission".to_string());
        } else {
            argv.push("--write-permission".to_string());
        }
        self.ripr(&argv)
    }

    fn capture_labels(&mut self) -> StageResult {
        self.mkdir("target/ci")?;
        let labels = self.settings.labels.clone()?;
        let mut json = serde_json::to_string(&serde_json::json!({ "labels": labels }))
            .map_err(|err| format!("cannot render the labels: {err}"))?;
        json.push('\n');
        fs::write(self.path(LABELS_JSON), json)
            .map_err(|err| format!("cannot write {LABELS_JSON}: {err}"))
    }

    fn evaluate_gate(&mut self) -> StageResult {
        self.mkdir(REPORTS)?;
        let mut argv = args(&[
            "gate",
            "evaluate",
            "--root",
            ".",
            "--pr-guidance",
            COMMENTS_JSON,
            "--mode",
            &self.settings.gate_mode,
            "--out",
            GATE_DECISION,
            "--out-md",
            "target/ripr/reports/gate-decision.md",
        ]);
        self.push_present(
            &mut argv,
            &[
                ("--repo-exposure", REPO_EXPOSURE),
                ("--labels-json", LABELS_JSON),
                ("--sarif-policy", "target/ripr/reports/sarif-policy.json"),
                ("--agent-verify", WORKFLOW_AGENT_VERIFY_ARTIFACT),
                ("--agent-receipt", WORKFLOW_AGENT_RECEIPT_ARTIFACT),
                ("--recommendation-calibration", RECOMMENDATION_CALIBRATION),
                ("--mutation-calibration", MUTATION_CALIBRATION),
            ],
        );
        if !self.settings.gate_baseline.is_empty() {
            argv.extend(args(&["--baseline", &self.settings.gate_baseline]));
        }
        self.ripr(&argv)
    }

    fn pr_evidence_ledger(&mut self) -> StageResult {
        self.mkdir(REPORTS)?;
        let mut argv = args(&[
            "pr-ledger",
            "record",
            "--pr-number",
            &self.settings.pr_number,
            "--base",
            &format!("origin/{}", self.settings.base_ref),
            "--head",
            "HEAD",
            "--pr-guidance",
            COMMENTS_JSON,
            "--out",
            PR_LEDGER,
            "--out-md",
            "target/ripr/reports/pr-evidence-ledger.md",
        ]);
        self.push_present(
            &mut argv,
            &[
                ("--gate", GATE_DECISION),
                ("--baseline-delta", BASELINE_DELTA),
                ("--zero-status", ZERO_STATUS),
                ("--recommendation-calibration", RECOMMENDATION_CALIBRATION),
                ("--agent-receipt", WORKFLOW_AGENT_RECEIPT_ARTIFACT),
                ("--coverage", "target/ripr/reports/coverage-summary.json"),
                ("--history", ".ripr/pr-evidence-ledger.jsonl"),
            ],
        );
        for label in recorded_labels(&self.path(LABELS_JSON)) {
            argv.push("--label".to_string());
            argv.push(label);
        }
        self.ripr(&argv)
    }

    fn preview_promotion_packets(&mut self) -> StageResult {
        self.mkdir(REPORTS)?;
        let languages = match crate::config::load_for_root(&self.root) {
            Ok(config) => config
                .languages()
                .enabled()
                .iter()
                .map(|language| language.as_str().to_string())
                .filter(|language| language == "typescript" || language == "python")
                .collect::<std::collections::BTreeSet<_>>(),
            Err(err) => {
                println!(
                    "Reading the language configuration failed ({err}); preview promotion packets were not generated. Run `ripr doctor --root .` locally for the underlying error."
                );
                return Ok(());
            }
        };
        if languages.is_empty() {
            println!(
                "No TypeScript or Python preview languages are configured; preview promotion packets were not generated."
            );
            return Ok(());
        }
        for language in languages {
            let mut argv = args(&[
                "policy",
                "preview-promote",
                "--language",
                &language,
                "--class",
                "boundary_gap",
                "--out",
                &format!("target/ripr/reports/preview-promotion-{language}-boundary-gap.json"),
                "--out-md",
                &format!("target/ripr/reports/preview-promotion-{language}-boundary-gap.md"),
            ]);
            self.push_present(
                &mut argv,
                &[(
                    "--evidence",
                    "target/ripr/reports/preview-promotion-evidence.json",
                )],
            );
            self.ripr(&argv)?;
        }
        Ok(())
    }

    fn first_useful_action(&mut self) -> StageResult {
        self.mkdir(REPORTS)?;
        let mut argv = args(&[
            "first-action",
            "--root",
            ".",
            "--out",
            "target/ripr/reports/first-useful-action.json",
            "--out-md",
            "target/ripr/reports/first-useful-action.md",
        ]);
        let has_input = self.push_inputs(
            &mut argv,
            &[
                ("--pr-guidance", COMMENTS_JSON),
                ("--assistant-proof", ASSISTANT_PROOF),
                ("--ledger", PR_LEDGER),
                ("--baseline-delta", BASELINE_DELTA),
                ("--receipt", WORKFLOW_AGENT_RECEIPT_ARTIFACT),
                ("--gate-decision", GATE_DECISION),
                ("--coverage-frontier", COVERAGE_FRONTIER),
                (
                    "--editor-context",
                    "target/ripr/workflow/evidence-context.json",
                ),
            ],
        );
        if has_input {
            self.ripr(&argv)
        } else {
            println!("No RIPR first-useful-action inputs were available.");
            println!(
                "Safe next action: run `ripr first-action --root . --pr-guidance target/ripr/review/comments.json --out target/ripr/reports/first-useful-action.json --out-md target/ripr/reports/first-useful-action.md` after attaching at least one explicit input."
            );
            Ok(())
        }
    }

    fn pr_review_front_panel(&mut self) -> StageResult {
        self.mkdir(REPORTS)?;
        let mut argv = args(&[
            "pr-review",
            "front-panel",
            "--root",
            ".",
            "--out",
            "target/ripr/reports/pr-review-front-panel.json",
            "--out-md",
            "target/ripr/reports/pr-review-front-panel.md",
        ]);
        let has_input = self.push_inputs(
            &mut argv,
            &[
                ("--pr-guidance", COMMENTS_JSON),
                (
                    "--first-action",
                    "target/ripr/reports/first-useful-action.json",
                ),
                ("--assistant-proof", ASSISTANT_PROOF),
                (
                    "--assistant-health",
                    "target/ripr/reports/assistant-loop-health.json",
                ),
                ("--ledger", PR_LEDGER),
                ("--baseline-delta", BASELINE_DELTA),
                ("--zero-status", ZERO_STATUS),
                ("--gate-decision", GATE_DECISION),
                ("--recommendation-calibration", RECOMMENDATION_CALIBRATION),
                ("--mutation-calibration", MUTATION_CALIBRATION),
                ("--coverage-frontier", COVERAGE_FRONTIER),
                ("--receipt", WORKFLOW_AGENT_RECEIPT_ARTIFACT),
            ],
        );
        if has_input {
            self.ripr(&argv)
        } else {
            println!("No RIPR PR review front-panel inputs were available.");
            println!(
                "Safe next action: run `ripr pr-review front-panel --root . --pr-guidance target/ripr/review/comments.json --out target/ripr/reports/pr-review-front-panel.json --out-md target/ripr/reports/pr-review-front-panel.md` after attaching at least one explicit input."
            );
            Ok(())
        }
    }

    fn report_packet_index(&mut self) -> StageResult {
        self.mkdir(REPORTS)?;
        let index_inputs = [
            "target/ripr/reports/start-here.md",
            "target/ripr/reports/pr-review-front-panel.md",
            "target/ripr/reports/first-useful-action.md",
            "target/ripr/review/comments.md",
            COMMENTS_JSON,
            "target/ripr/review/comment-publish-plan.md",
            "target/ripr/reports/test-oracle-assistant-proof.md",
            "target/ripr/reports/assistant-loop-health.md",
            "target/ripr/reports/pr-evidence-ledger.md",
            "target/ripr/reports/waiver-aging.md",
            "target/ripr/reports/suppression-health.md",
            "target/ripr/reports/policy-readiness.md",
            "target/ripr/reports/policy-operations.md",
            "target/ripr/reports/policy-history.md",
            "target/ripr/reports/policy-promotion-visible-only.md",
            "target/ripr/reports/policy-promotion-acknowledgeable.md",
            "target/ripr/reports/policy-promotion-baseline-check.md",
            "target/ripr/reports/policy-promotion-calibrated-gate.md",
            "target/ripr/reports/preview-promotion-typescript-boundary-gap.md",
            "target/ripr/reports/preview-promotion-python-boundary-gap.md",
            "target/ripr/reports/baseline-debt-delta.md",
            "target/ripr/reports/ripr-zero-status.md",
            "target/ripr/reports/gate-decision.md",
            "target/ripr/reports/recommendation-calibration.md",
            "target/ripr/reports/mutation-calibration.md",
            "target/ripr/reports/coverage-grip-frontier.md",
            WORKFLOW_AGENT_RECEIPT_ARTIFACT,
            "target/ripr/reports/pr-summary.md",
            "target/ripr/reports/check-pr.md",
            "target/ripr/reports/ripr.sarif.json",
            "target/ripr/reports/ripr-badge.json",
        ];
        let regenerate = args(&[
            "reports",
            "index",
            "--root",
            ".",
            "--reports-dir",
            REPORTS,
            "--review-dir",
            "target/ripr/review",
            "--receipts-dir",
            "target/ripr/receipts",
            "--workflow-dir",
            "target/ripr/workflow",
            "--agent-dir",
            "target/ripr/agent",
            "--pilot-dir",
            "target/ripr/pilot",
            "--ci-dir",
            "target/ci",
            "--out",
            "target/ripr/reports/index.json",
            "--out-md",
            "target/ripr/reports/index.md",
        ]);
        if index_inputs.iter().any(|path| self.path(path).is_file()) {
            self.ripr(&regenerate)
        } else {
            println!("No RIPR report-packet index inputs were available.");
            println!("Regenerate command: `ripr {}`.", regenerate.join(" "));
            Ok(())
        }
    }

    fn check_advisory_artifacts(&self) {
        let mut missing = [
            "target/ripr/reports/start-here.md",
            "target/ripr/reports/index.json",
        ]
        .into_iter()
        .filter(|artifact| !self.path(artifact).is_file())
        .map(str::to_string)
        .collect::<Vec<_>>();
        if !self.settings.gate_mode.is_empty()
            && !self.path(GATE_DECISION).is_file()
            && self.path(COMMENTS_JSON).is_file()
        {
            missing.push(format!("{GATE_DECISION} (RIPR_GATE_MODE is set)"));
        }
        if !missing.is_empty() {
            println!(
                "::warning::Some RIPR advisory artifacts are missing (upstream step failed softly):"
            );
            for artifact in missing {
                println!("  - {artifact}");
            }
        }
    }

    fn emit_annotations(&mut self) -> StageResult {
        let bytes = fs::read(self.path(COMMENTS_JSON))
            .map_err(|err| format!("cannot read {COMMENTS_JSON}: {err}"))?;
        let document = serde_json::from_slice::<Value>(&bytes)
            .map_err(|err| format!("{COMMENTS_JSON} is not JSON: {err}"))?;
        let (printed, result) = match render_workflow_annotations(&document) {
            Ok(lines) => (lines, Ok(())),
            Err((lines, err)) => (lines, Err(err)),
        };
        print!("{printed}");
        let _ = std::io::stdout().flush();
        result
    }
}

#[derive(Serialize)]
struct DiffReceipt<'a> {
    tool: &'a str,
    kind: &'a str,
    base_ref: &'a str,
    base_sha: &'a str,
    head_sha: &'a str,
    byte_count: usize,
    sha256: &'a str,
}

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Raw stdout of a successful git invocation; the diff is bytes, not text.
fn git_bytes(root: &Path, git_args: &[&str]) -> Result<Vec<u8>, String> {
    let output = crate::git::run_git_output_with_deadline(root, git_args, None)?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/// `.top_actionable_seams[0].seam_id // empty` of the pilot summary, minus
/// the literal "null"; an unreadable summary names no seam.
pub(super) fn top_seam_id(bytes: &[u8]) -> Option<String> {
    let summary = serde_json::from_slice::<Value>(bytes).ok()?;
    let seam = summary.pointer("/top_actionable_seams/0/seam_id")?;
    let seam = match seam {
        Value::String(text) => text.clone(),
        Value::Null | Value::Bool(false) => return None,
        other => other.to_string(),
    };
    (!seam.is_empty() && seam != "null").then_some(seam)
}

/// `.labels[]? // empty` of the recorded labels, minus empty and "null".
fn recorded_labels(path: &Path) -> Vec<String> {
    let Ok(bytes) = fs::read(path) else {
        return Vec::new();
    };
    let Ok(document) = serde_json::from_slice::<Value>(&bytes) else {
        return Vec::new();
    };
    let labels = match document.get("labels") {
        Some(Value::Array(items)) => items.clone(),
        Some(Value::Object(map)) => map.values().cloned().collect(),
        _ => Vec::new(),
    };
    labels
        .into_iter()
        .filter_map(|label| match label {
            Value::String(text) => Some(text),
            Value::Null | Value::Bool(false) => None,
            other => Some(other.to_string()),
        })
        .filter(|label| !label.is_empty() && label != "null")
        .collect()
}

/// Workflow-command data encoding for an `::error` message.
fn workflow_data(text: &str) -> String {
    text.replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

/// The command lines the packet would run for `settings` over the
/// artifacts present under `root`, with the steps that failed. Only the
/// child `ripr` commands and the diff capture are recorded; file copies and
/// reads run for real.
#[cfg(test)]
pub(super) fn recorded_commands(root: &Path, settings: CiSettings) -> (Vec<String>, Vec<String>) {
    let mut run = PacketRun {
        root: root.to_path_buf(),
        exe: PathBuf::from("ripr"),
        settings,
        only: Vec::new(),
        seen: Vec::new(),
        top_seam: None,
        failed: Vec::new(),
        recorded: Some(std::cell::RefCell::new(Vec::new())),
    };
    run.all_stages();
    let commands = run
        .recorded
        .take()
        .map(std::cell::RefCell::into_inner)
        .unwrap_or_default();
    (commands, run.failed)
}

/// Every step the packet declares, in order, with how its failure counts
/// and whether it runs after one.
#[cfg(test)]
pub(super) fn stage_table() -> Vec<(String, When, Role)> {
    let mut run = PacketRun {
        root: std::env::temp_dir().join("ripr-ci-packet-stage-table-absent"),
        exe: PathBuf::from("ripr"),
        settings: CiSettings::default(),
        only: vec![String::new()],
        seen: Vec::new(),
        top_seam: None,
        failed: Vec::new(),
        recorded: Some(std::cell::RefCell::new(Vec::new())),
    };
    run.all_stages();
    run.seen
}

/// Every optional input the steps look for, so a recorded packet shows
/// every optional flag.
#[cfg(test)]
pub(super) const EVERY_OPTIONAL_INPUT: &[&str] = &[
    PILOT_BEFORE_SNAPSHOT_ARTIFACT,
    "target/ripr/pilot/agent-seam-packets.json",
    WORKFLOW_AGENT_BRIEF_ARTIFACT,
    COMMENTS_JSON,
    "target/ripr/review/existing-comments.json",
    GATE_DECISION,
    BASELINE_DELTA,
    ZERO_STATUS,
    PR_LEDGER,
    LABELS_JSON,
    "target/ripr/reports/sarif-policy.json",
    WORKFLOW_AGENT_VERIFY_ARTIFACT,
    WORKFLOW_AGENT_RECEIPT_ARTIFACT,
    RECOMMENDATION_CALIBRATION,
    MUTATION_CALIBRATION,
    "target/ripr/reports/coverage-summary.json",
    ".ripr/pr-evidence-ledger.jsonl",
    ".ripr/policy-history.jsonl",
    "target/ripr/reports/waiver-aging.json",
    "target/ripr/reports/suppression-health.json",
    "target/ripr/reports/policy-readiness.json",
    POLICY_OPERATIONS,
    "target/ripr/reports/policy-history.json",
    "target/ripr/reports/preview-promotion-evidence.json",
    WORKFLOW_AFTER_SNAPSHOT_ARTIFACT,
    ASSISTANT_PROOF,
    COVERAGE_FRONTIER,
    "target/ripr/reports/assistant-loop-health.json",
    "target/ripr/reports/first-useful-action.json",
    "target/ripr/workflow/evidence-context.json",
];

/// A pull request run with every setting on, as recorded command lines
/// joined by newlines, over a root holding [`EVERY_OPTIONAL_INPUT`] and a
/// pilot summary naming `seam-1`.
#[cfg(test)]
pub(super) fn recorded_full_packet() -> Result<(String, Vec<String>), String> {
    recorded_packet_with(full_packet_settings())
}

/// The settings [`recorded_full_packet`] records under.
#[cfg(test)]
fn full_packet_settings() -> CiSettings {
    CiSettings {
        gate_mode: "acknowledgeable".to_string(),
        gate_baseline: ".ripr/gate-baseline.json".to_string(),
        comment_mode: "inline".to_string(),
        upload_sarif: "true".to_string(),
        event_name: "pull_request".to_string(),
        base_ref: "main".to_string(),
        default_branch: "main".to_string(),
        pr_number: "7".to_string(),
        event_number: "7".to_string(),
        head_repo: "owner/repo".to_string(),
        repository: "owner/repo".to_string(),
        actor: "octocat".to_string(),
        pr_author: "octocat".to_string(),
        labels: Ok(vec![Value::from("ripr-waive")]),
    }
}

/// [`recorded_full_packet`]'s inputs, recorded under `settings`.
#[cfg(test)]
fn recorded_packet_with(settings: CiSettings) -> Result<(String, Vec<String>), String> {
    let root = std::env::temp_dir().join(format!(
        "ripr-ci-packet-recorded-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|err| err.to_string())?
            .as_nanos()
    ));
    for input in EVERY_OPTIONAL_INPUT {
        let path = root.join(input);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        fs::write(&path, "{\"comments\":[]}").map_err(|err| err.to_string())?;
    }
    fs::write(
        root.join("target/ripr/pilot/pilot-summary.json"),
        r#"{"top_actionable_seams":[{"seam_id":"seam-1"}]}"#,
    )
    .map_err(|err| err.to_string())?;
    let (commands, failed) = recorded_commands(&root, settings);
    let _ = fs::remove_dir_all(&root);
    Ok((commands.join("\n"), failed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_default_to_the_current_directory() {
        assert_eq!(
            parse_ci_packet_options(&[]),
            Ok(CiPacketOptions {
                root: PathBuf::from("."),
                steps: Vec::new(),
            })
        );
        assert_eq!(
            parse_ci_packet_options(&args(&["--root", "repo"])).map(|options| options.root),
            Ok(PathBuf::from("repo"))
        );
        assert_eq!(
            parse_ci_packet_options(&args(&[
                "--step",
                "Capture pull request diff",
                "--step",
                "Run RIPR PR guidance report"
            ]))
            .map(|options| options.steps),
            Ok(args(&[
                "Capture pull request diff",
                "Run RIPR PR guidance report"
            ]))
        );
        assert!(matches!(
            parse_ci_packet_options(&args(&["--base", "main"])),
            Err(err) if err.contains("--base")
        ));
        assert!(matches!(
            parse_ci_packet_options(&args(&["--root", " "])),
            Err(err) if err.contains("--root")
        ));
    }

    #[test]
    fn event_facts_read_like_workflow_expressions() {
        let event = serde_json::json!({
            "number": 7,
            "repository": {"default_branch": "trunk"},
            "pull_request": {
                "number": 7,
                "head": {"repo": {"full_name": "fork/repo"}},
                "user": {"login": "dependabot[bot]"},
                "labels": [{"name": "ripr-waive"}, {"color": "red"}]
            }
        });
        let settings = CiSettings::from_event(&event);
        assert_eq!(settings.pr_number, "7");
        assert_eq!(settings.event_number, "7");
        assert_eq!(settings.default_branch, "trunk");
        assert_eq!(settings.head_repo, "fork/repo");
        assert_eq!(settings.pr_author, "dependabot[bot]");
        assert_eq!(
            settings.labels,
            Ok(vec![Value::from("ripr-waive"), Value::Null])
        );
        let manual = CiSettings::from_event(&serde_json::json!({"ref": "refs/heads/main"}));
        assert_eq!(manual.pr_number, "");
        assert_eq!(manual.labels, Ok(Vec::new()));
        let malformed =
            CiSettings::from_event(&serde_json::json!({"pull_request": {"labels": ["x"]}}));
        assert!(matches!(&malformed.labels, Err(err) if !err.is_empty()));
    }

    #[test]
    fn gate_mode_decides_whether_gate_critical_stages_are_advisory() {
        let mut settings = CiSettings::default();
        assert!(settings.advisory());
        settings.gate_mode = "visible-only".to_string();
        assert!(settings.advisory());
        settings.gate_mode = "baseline-check".to_string();
        assert!(!settings.advisory());
    }

    #[test]
    fn top_seam_follows_the_retired_jq_query() {
        let seam = |text: &str| top_seam_id(text.as_bytes());
        assert_eq!(
            seam(r#"{"top_actionable_seams":[{"seam_id":"s1"},{"seam_id":"s2"}]}"#),
            Some("s1".to_string())
        );
        assert_eq!(seam(r#"{"top_actionable_seams":[]}"#), None);
        assert_eq!(seam(r#"{"top_actionable_seams":[{"seam_id":null}]}"#), None);
        assert_eq!(
            seam(r#"{"top_actionable_seams":[{"seam_id":"null"}]}"#),
            None
        );
        assert_eq!(seam(r#"{"top_actionable_seams":[{"seam_id":""}]}"#), None);
        assert_eq!(seam("not json"), None);
    }

    #[test]
    fn recorded_labels_skip_empty_and_null() -> Result<(), String> {
        let dir = std::env::temp_dir().join(format!(
            "ripr-ci-packet-labels-{}-{}",
            std::process::id(),
            line!()
        ));
        fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
        let path = dir.join("labels.json");
        fs::write(&path, r#"{"labels":["ripr-waive",null,"","null","bug"]}"#)
            .map_err(|err| err.to_string())?;
        let labels = recorded_labels(&path);
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(labels, vec!["ripr-waive".to_string(), "bug".to_string()]);
        assert!(recorded_labels(&dir.join("absent.json")).is_empty());
        Ok(())
    }

    #[test]
    fn recorded_packet_runs_each_step_with_its_present_inputs() -> Result<(), String> {
        let (packet, failed) = recorded_full_packet()?;
        assert!(failed.is_empty(), "{failed:?}");
        // Every command line, in the retired workflow's step order, with
        // every optional flag its input turns on.
        let expected = [
            "ripr pilot --root . --out target/ripr/pilot --mode ready --max-seams 5",
            "ripr agent start --root . --seam-id seam-1 --out target/ripr/workflow",
            "ripr agent packet --root . --seam-id seam-1 --json > target/ripr/workflow/agent-packet.json.partial",
            "ripr reports gap-ledger --root . --repo-exposure target/ripr/reports/repo-exposure.json --out target/ripr/reports/gap-decision-ledger.json --out-md target/ripr/reports/gap-decision-ledger.md",
            "capture origin/main...HEAD > target/ripr/reports/pr.diff",
            "ripr check --root . --base origin/main --format json > target/ripr/pr/check.json",
            "ripr review-comments --root . --base origin/main --head HEAD --check-output target/ripr/pr/check.json --out target/ripr/review/comments.json",
            "ripr pr-comments plan --root . --pr-guidance target/ripr/review/comments.json --mode inline --event-name pull_request --pull-request 7 --head-repo owner/repo --base-repo owner/repo --out target/ripr/review/comment-publish-plan.json --out-md target/ripr/review/comment-publish-plan.md --existing-comments target/ripr/review/existing-comments.json --token-available --write-permission",
            "ripr check --root . --diff target/ripr/reports/pr.diff --format sarif > target/ripr/reports/ripr-findings.sarif",
            "ripr check --root . --mode ready --format repo-sarif > target/ripr/reports/ripr-seams.sarif",
            "ripr check --root . --mode ready --format repo-badge-json > target/ripr/reports/repo-ripr-badge.json",
            "ripr check --root . --mode ready --format repo-badge-shields > target/ripr/reports/repo-ripr-badge-shields.json",
            "ripr gate evaluate --root . --pr-guidance target/ripr/review/comments.json --mode acknowledgeable --out target/ripr/reports/gate-decision.json --out-md target/ripr/reports/gate-decision.md --repo-exposure target/ripr/reports/repo-exposure.json --labels-json target/ci/labels.json --sarif-policy target/ripr/reports/sarif-policy.json --agent-verify target/ripr/workflow/agent-verify.json --agent-receipt target/ripr/reports/agent-receipt.json --recommendation-calibration target/ripr/reports/recommendation-calibration.json --mutation-calibration target/ripr/reports/mutation-calibration.json --baseline .ripr/gate-baseline.json",
            "ripr baseline diff --baseline .ripr/gate-baseline.json --current target/ripr/reports/gate-decision.json --out target/ripr/reports/baseline-debt-delta.json --out-md target/ripr/reports/baseline-debt-delta.md",
            "ripr zero status --delta target/ripr/reports/baseline-debt-delta.json --out target/ripr/reports/ripr-zero-status.json --out-md target/ripr/reports/ripr-zero-status.md --baseline .ripr/gate-baseline.json --gate target/ripr/reports/gate-decision.json --pr-guidance target/ripr/review/comments.json --recommendation-calibration target/ripr/reports/recommendation-calibration.json",
            "ripr pr-ledger record --pr-number 7 --base origin/main --head HEAD --pr-guidance target/ripr/review/comments.json --out target/ripr/reports/pr-evidence-ledger.json --out-md target/ripr/reports/pr-evidence-ledger.md --gate target/ripr/reports/gate-decision.json --baseline-delta target/ripr/reports/baseline-debt-delta.json --zero-status target/ripr/reports/ripr-zero-status.json --recommendation-calibration target/ripr/reports/recommendation-calibration.json --agent-receipt target/ripr/reports/agent-receipt.json --coverage target/ripr/reports/coverage-summary.json --history .ripr/pr-evidence-ledger.jsonl --label ripr-waive",
            "ripr policy waiver-aging --root . --ledger target/ripr/reports/pr-evidence-ledger.json --out target/ripr/reports/waiver-aging.json --out-md target/ripr/reports/waiver-aging.md --history .ripr/pr-evidence-ledger.jsonl",
            "ripr policy suppression-health --root . --out target/ripr/reports/suppression-health.json --out-md target/ripr/reports/suppression-health.md",
            "ripr policy readiness --root . --out target/ripr/reports/policy-readiness.json --out-md target/ripr/reports/policy-readiness.md --gate-decision target/ripr/reports/gate-decision.json --baseline-delta target/ripr/reports/baseline-debt-delta.json --recommendation-calibration target/ripr/reports/recommendation-calibration.json --mutation-calibration target/ripr/reports/mutation-calibration.json --waiver-aging target/ripr/reports/waiver-aging.json --suppression-health target/ripr/reports/suppression-health.json",
            "ripr policy operations --root . --policy-readiness target/ripr/reports/policy-readiness.json --out target/ripr/reports/policy-operations.json --out-md target/ripr/reports/policy-operations.md --waiver-aging target/ripr/reports/waiver-aging.json --suppression-health target/ripr/reports/suppression-health.json --baseline-delta target/ripr/reports/baseline-debt-delta.json --gate-decision target/ripr/reports/gate-decision.json --recommendation-calibration target/ripr/reports/recommendation-calibration.json --mutation-calibration target/ripr/reports/mutation-calibration.json --preview-boundary target/ripr/reports/repo-exposure.json",
            "ripr policy history --root . --current target/ripr/reports/policy-operations.json --commit HEAD --out target/ripr/reports/policy-history.json --out-md target/ripr/reports/policy-history.md --history .ripr/policy-history.jsonl --pr-number 7",
            "ripr policy promote --to visible-only --operations target/ripr/reports/policy-operations.json --out target/ripr/reports/policy-promotion-visible-only.json --out-md target/ripr/reports/policy-promotion-visible-only.md --history target/ripr/reports/policy-history.json",
            "ripr policy promote --to acknowledgeable --operations target/ripr/reports/policy-operations.json --out target/ripr/reports/policy-promotion-acknowledgeable.json --out-md target/ripr/reports/policy-promotion-acknowledgeable.md --history target/ripr/reports/policy-history.json",
            "ripr policy promote --to baseline-check --operations target/ripr/reports/policy-operations.json --out target/ripr/reports/policy-promotion-baseline-check.json --out-md target/ripr/reports/policy-promotion-baseline-check.md --history target/ripr/reports/policy-history.json",
            "ripr policy promote --to calibrated-gate --operations target/ripr/reports/policy-operations.json --out target/ripr/reports/policy-promotion-calibrated-gate.json --out-md target/ripr/reports/policy-promotion-calibrated-gate.md --history target/ripr/reports/policy-history.json",
            "ripr assistant-loop proof --root . --pr-guidance target/ripr/review/comments.json --agent-packet target/ripr/workflow/agent-brief.json --before target/ripr/workflow/before.repo-exposure.json --after target/ripr/workflow/after.repo-exposure.json --receipt target/ripr/reports/agent-receipt.json --ledger target/ripr/reports/pr-evidence-ledger.json --out target/ripr/reports/test-oracle-assistant-proof.json --out-md target/ripr/reports/test-oracle-assistant-proof.md --coverage-frontier target/ripr/reports/coverage-grip-frontier.json --gate-decision target/ripr/reports/gate-decision.json",
            "ripr assistant-loop health --root . --proof target/ripr/reports/test-oracle-assistant-proof.json --out target/ripr/reports/assistant-loop-health.json --out-md target/ripr/reports/assistant-loop-health.md",
            "ripr first-action --root . --out target/ripr/reports/first-useful-action.json --out-md target/ripr/reports/first-useful-action.md --pr-guidance target/ripr/review/comments.json --assistant-proof target/ripr/reports/test-oracle-assistant-proof.json --ledger target/ripr/reports/pr-evidence-ledger.json --baseline-delta target/ripr/reports/baseline-debt-delta.json --receipt target/ripr/reports/agent-receipt.json --gate-decision target/ripr/reports/gate-decision.json --coverage-frontier target/ripr/reports/coverage-grip-frontier.json --editor-context target/ripr/workflow/evidence-context.json",
            "ripr pr-review front-panel --root . --out target/ripr/reports/pr-review-front-panel.json --out-md target/ripr/reports/pr-review-front-panel.md --pr-guidance target/ripr/review/comments.json --first-action target/ripr/reports/first-useful-action.json --assistant-proof target/ripr/reports/test-oracle-assistant-proof.json --assistant-health target/ripr/reports/assistant-loop-health.json --ledger target/ripr/reports/pr-evidence-ledger.json --baseline-delta target/ripr/reports/baseline-debt-delta.json --zero-status target/ripr/reports/ripr-zero-status.json --gate-decision target/ripr/reports/gate-decision.json --recommendation-calibration target/ripr/reports/recommendation-calibration.json --mutation-calibration target/ripr/reports/mutation-calibration.json --coverage-frontier target/ripr/reports/coverage-grip-frontier.json --receipt target/ripr/reports/agent-receipt.json",
            "ripr first-pr --root . --base origin/main --head HEAD --gap-ledger target/ripr/reports/gap-decision-ledger.json --first-action target/ripr/reports/first-useful-action.json --review-comments target/ripr/review/comments.json --agent-packet target/ripr/workflow/agent-packet.json --gate-decision target/ripr/reports/gate-decision.json --receipts-dir target/ripr/receipts --out-dir target/ripr/reports",
            "ripr reports index --root . --reports-dir target/ripr/reports --review-dir target/ripr/review --receipts-dir target/ripr/receipts --workflow-dir target/ripr/workflow --agent-dir target/ripr/agent --pilot-dir target/ripr/pilot --ci-dir target/ci --out target/ripr/reports/index.json --out-md target/ripr/reports/index.md",
            "ripr agent status --root . --json > target/ripr/workflow/agent-status.json",
            "ripr agent status --root . > target/ripr/workflow/agent-status.md",
            "ripr agent review-summary --root . --json > target/ripr/workflow/agent-review-summary.json",
            "ripr agent review-summary --root . > target/ripr/workflow/agent-review-summary.md",
        ];
        assert_eq!(packet.lines().collect::<Vec<_>>(), expected);
        Ok(())
    }

    /// Each expression the retired steps read stays bound to its own field:
    /// the PR base and number for the guidance and PR ledger, `event.number`
    /// for policy history, and the default branch for a manual run's
    /// first-pr base (the full packet uses one value for each pair).
    #[test]
    fn each_step_reads_its_own_event_field() -> Result<(), String> {
        let line = |packet: &str, prefix: &str| -> String {
            packet
                .lines()
                .find(|line| line.starts_with(prefix))
                .unwrap_or_default()
                .to_string()
        };
        let (pr, failed) = recorded_packet_with(CiSettings {
            base_ref: "release".to_string(),
            default_branch: "trunk".to_string(),
            pr_number: "7".to_string(),
            event_number: "8".to_string(),
            ..full_packet_settings()
        })?;
        assert!(failed.is_empty(), "{failed:?}");
        assert!(line(&pr, "ripr review-comments ").contains(" --base origin/release "));
        assert!(line(&pr, "ripr first-pr ").contains(" --base origin/release "));
        assert!(line(&pr, "ripr pr-ledger record ").contains(" --pr-number 7 "));
        assert!(line(&pr, "ripr pr-comments plan ").contains(" --pull-request 7 "));
        assert!(line(&pr, "ripr policy history ").ends_with(" --pr-number 8"));

        let (manual, failed) = recorded_packet_with(CiSettings {
            event_name: "workflow_dispatch".to_string(),
            base_ref: String::new(),
            default_branch: "trunk".to_string(),
            pr_number: String::new(),
            event_number: String::new(),
            ..full_packet_settings()
        })?;
        assert!(failed.is_empty(), "{failed:?}");
        assert!(line(&manual, "ripr first-pr ").contains(" --base origin/trunk "));
        assert!(!manual.contains("ripr review-comments "), "{manual}");
        assert!(!line(&manual, "ripr policy history ").contains("--pr-number"));
        Ok(())
    }

    /// The YAML steps' `continue-on-error` and `always()`, as the packet
    /// keeps them (#2009: SARIF and guidance producers block only when the
    /// operator chose a blocking gate).
    #[test]
    fn stage_table_keeps_the_retired_step_semantics() {
        use Role::{Advisory, GateCritical, Required};
        use When::{Always, Success};
        let expected = [
            ("Generate RIPR pilot packet", Success, Advisory),
            ("Prepare RIPR editor-agent artifacts", Always, Advisory),
            ("Generate RIPR agent loop artifacts", Always, Advisory),
            ("Render RIPR gap decision ledger", Always, Advisory),
            ("Capture pull request diff", Success, Required),
            ("Run RIPR PR guidance report", Success, GateCritical),
            ("Plan RIPR inline comments", Always, Advisory),
            ("Capture RIPR gate labels", Always, Advisory),
            ("Render RIPR diff SARIF", Success, GateCritical),
            ("Render RIPR repo seam SARIF", Success, GateCritical),
            ("Render RIPR repo badge artifacts", Success, Advisory),
            ("Evaluate RIPR gate decision", Always, Required),
            ("Render RIPR baseline debt delta", Always, Advisory),
            ("Render RIPR Zero status", Always, Advisory),
            ("Render RIPR PR evidence ledger", Always, Advisory),
            ("Render RIPR waiver aging", Always, Advisory),
            ("Render RIPR suppression health", Always, Advisory),
            ("Render RIPR policy readiness", Always, Advisory),
            ("Render RIPR policy operations", Always, Advisory),
            ("Render RIPR policy history", Always, Advisory),
            ("Render RIPR policy promotion packets", Always, Advisory),
            ("Render RIPR preview promotion packets", Always, Advisory),
            ("Render RIPR test-oracle assistant proof", Always, Advisory),
            ("Render RIPR assistant loop health", Always, Advisory),
            ("Render RIPR first useful action", Always, Advisory),
            ("Render RIPR PR review front panel", Always, Advisory),
            ("Render RIPR first-pr start-here", Always, Advisory),
            ("Render RIPR report packet index", Always, Advisory),
            ("Render RIPR LLM work-loop summaries", Always, Advisory),
            ("Check RIPR advisory artifacts", Always, Advisory),
            ("Emit RIPR PR guidance annotations", Always, Advisory),
        ];
        let table = stage_table();
        let table: Vec<(&str, When, Role)> = table
            .iter()
            .map(|(name, when, role)| (name.as_str(), *when, *role))
            .collect();
        assert_eq!(table, expected);
    }

    /// `--step` alone runs the agent loop from the pilot summary's seam,
    /// without the prepare step that sets it in a full run.
    #[test]
    fn the_agent_loop_runs_alone_with_step() -> Result<(), String> {
        let root = std::env::temp_dir().join(format!(
            "ripr-ci-packet-agent-loop-{}-{}",
            std::process::id(),
            line!()
        ));
        fs::create_dir_all(root.join("target/ripr/pilot")).map_err(|err| err.to_string())?;
        fs::write(
            root.join("target/ripr/pilot/pilot-summary.json"),
            r#"{"top_actionable_seams":[{"seam_id":"seam-9"}]}"#,
        )
        .map_err(|err| err.to_string())?;
        // The recorded `agent start` writes nothing, so seed the brief it
        // would have written; the stage copies it for the editor.
        fs::create_dir_all(root.join("target/ripr/workflow")).map_err(|err| err.to_string())?;
        fs::write(root.join(WORKFLOW_AGENT_BRIEF_ARTIFACT), "brief")
            .map_err(|err| err.to_string())?;
        let mut run = PacketRun {
            root: root.clone(),
            exe: PathBuf::from("ripr"),
            settings: CiSettings::default(),
            only: args(&["Generate RIPR agent loop artifacts"]),
            seen: Vec::new(),
            top_seam: None,
            failed: Vec::new(),
            recorded: Some(std::cell::RefCell::new(Vec::new())),
        };
        run.all_stages();
        let packet_copied = root.join(EDITOR_AGENT_PACKET_ARTIFACT).is_file();
        let brief_copied = root.join(EDITOR_AGENT_BRIEF_ARTIFACT).is_file();
        let _ = fs::remove_dir_all(&root);
        let commands = run
            .recorded
            .take()
            .map(std::cell::RefCell::into_inner)
            .unwrap_or_default();
        assert!(packet_copied && brief_copied, "{commands:?}");
        assert!(
            commands.iter().any(|line| line
                == "ripr agent start --root . --seam-id seam-9 --out target/ripr/workflow"),
            "{commands:?}"
        );
        assert!(
            !commands.iter().any(|line| line.starts_with("ripr pilot")),
            "{commands:?}"
        );
        Ok(())
    }

    /// A step without `always()` is skipped after a required step fails,
    /// as GitHub skipped it; `always()` steps still run.
    #[test]
    fn a_required_failure_skips_only_the_success_steps() -> Result<(), String> {
        let root = std::env::temp_dir().join(format!(
            "ripr-ci-packet-skip-{}-{}",
            std::process::id(),
            line!()
        ));
        fs::create_dir_all(&root).map_err(|err| err.to_string())?;
        let mut run = PacketRun {
            root: root.clone(),
            exe: PathBuf::from("ripr"),
            settings: CiSettings {
                upload_sarif: "true".to_string(),
                ..CiSettings::default()
            },
            only: Vec::new(),
            seen: Vec::new(),
            top_seam: None,
            failed: Vec::new(),
            recorded: Some(std::cell::RefCell::new(Vec::new())),
        };
        run.stage("Required", When::Always, Role::Required, true, |_| {
            Err("boom".to_string())
        });
        let mut ran = Vec::new();
        run.stage(
            "Advisory after",
            When::Success,
            Role::Advisory,
            true,
            |_| {
                ran.push("success");
                Ok(())
            },
        );
        run.stage("Always after", When::Always, Role::Advisory, true, |_| {
            ran.push("always");
            Ok(())
        });
        run.stage(
            "Advisory failure",
            When::Always,
            Role::Advisory,
            true,
            |_| Err("soft".to_string()),
        );
        run.stage("Gate input", When::Always, Role::GateCritical, true, |_| {
            Err("gate input".to_string())
        });
        let _ = fs::remove_dir_all(&root);
        assert_eq!(ran, vec!["always"]);
        // Advisory gate mode: the gate input failure is advisory too.
        assert_eq!(run.failed, vec!["Required".to_string()]);
        run.settings.gate_mode = "baseline-check".to_string();
        run.stage("Gate input", When::Always, Role::GateCritical, true, |_| {
            Err("gate input".to_string())
        });
        assert_eq!(
            run.failed,
            vec!["Required".to_string(), "Gate input".to_string()]
        );
        Ok(())
    }
}
