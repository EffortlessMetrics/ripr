use crate::agent::loop_commands::{
    WORKFLOW_AFTER_SNAPSHOT_ARTIFACT, WORKFLOW_AGENT_BRIEF_ARTIFACT,
    WORKFLOW_AGENT_PACKET_ARTIFACT, WORKFLOW_AGENT_RECEIPT_ARTIFACT,
    WORKFLOW_AGENT_REVIEW_SUMMARY_ARTIFACT, WORKFLOW_AGENT_REVIEW_SUMMARY_MARKDOWN_ARTIFACT,
    WORKFLOW_AGENT_STATUS_ARTIFACT, WORKFLOW_AGENT_STATUS_MARKDOWN_ARTIFACT,
    WORKFLOW_AGENT_VERIFY_ARTIFACT, WORKFLOW_ANALYSIS_OUTCOME_ARTIFACT,
    WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT, agent_brief_command, agent_packet_command,
    agent_receipt_command, agent_review_summary_command, agent_review_summary_markdown_command,
    agent_status_command, agent_status_markdown_command, agent_verify_command,
    anchored_redirect_target, bound_root, check_analysis_outcome_command,
    check_repo_exposure_command, display_path, shell_arg,
};
use crate::app::repair_attempt::{
    AfterPhaseHeadAdmission, DivergedHeadRecovery, REPAIR_ATTEMPT_DIRECTORY,
    RepairAttemptInventoryEntry, RepairAttemptManifest, RepairAttemptState,
    after_phase_head_admission, diverged_head_recovery, inventory_repair_attempts,
};
use crate::output::agent_receipt::AgentReceiptReading;
use crate::output::markdown::{COMMAND_SHELL_DISCLOSURE, PowershellForm, powershell_form};
use serde_json::Value;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) const AGENT_STATUS_SCHEMA_VERSION: &str = "0.1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AgentStatusArtifactDef {
    name: &'static str,
    label: &'static str,
    path: &'static str,
}

const ARTIFACTS: &[AgentStatusArtifactDef] = &[
    AgentStatusArtifactDef {
        name: "before_snapshot",
        label: "before snapshot",
        path: WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT,
    },
    AgentStatusArtifactDef {
        name: "after_snapshot",
        label: "after snapshot",
        path: WORKFLOW_AFTER_SNAPSHOT_ARTIFACT,
    },
    AgentStatusArtifactDef {
        name: "analysis_outcome",
        label: "analysis outcome",
        path: WORKFLOW_ANALYSIS_OUTCOME_ARTIFACT,
    },
    AgentStatusArtifactDef {
        name: "agent_brief",
        label: "agent brief",
        path: WORKFLOW_AGENT_BRIEF_ARTIFACT,
    },
    AgentStatusArtifactDef {
        name: "agent_packet",
        label: "agent packet",
        path: WORKFLOW_AGENT_PACKET_ARTIFACT,
    },
    AgentStatusArtifactDef {
        name: "agent_verify",
        label: "agent verify",
        path: WORKFLOW_AGENT_VERIFY_ARTIFACT,
    },
    AgentStatusArtifactDef {
        name: "agent_receipt",
        label: "agent receipt",
        path: WORKFLOW_AGENT_RECEIPT_ARTIFACT,
    },
];

/// Artifacts whose repository-global copies the repair-attempt authority
/// supersedes while a repair attempt is present. Per
/// `docs/REPAIR_ATTEMPT.md` ("Durable location" and "Compatibility
/// outputs"), repository-global workflow files remain compatibility
/// projections and are not repair-attempt identity: the attempt retains its
/// own digest-bound before artifacts under `target/ripr/repair-attempts/`,
/// and the manifest records that after-phase verify and receipt outputs
/// remain mirrored through the workflow compatibility paths. Status must not
/// report a projection the active loop mode does not enforce as `required`.
///
/// Shared with `app::agent_workflow`: the workflow manifest is the agent
/// loop's machine-readable contract, so its `required` flags must follow the
/// same rule instead of claiming the legacy loop is enforced while an
/// attempt is present. Both owners consume `artifact_required_by_active_loop`.
pub(crate) const REPAIR_ATTEMPT_SUPERSEDED_ARTIFACTS: &[&str] = &[
    "before_snapshot",
    "after_snapshot",
    "analysis_outcome",
    "agent_brief",
    "agent_packet",
    "agent_verify",
    "agent_receipt",
];

/// Whether the active loop mode requires this artifact: every artifact is
/// required by the legacy artifact loop when no repair attempt is present;
/// artifacts the repair-attempt authority supersedes are not required while
/// an attempt is present, because the attempt directory holds the enforced
/// identity and the global files are compatibility projections.
///
/// `pub(crate)` because `app::agent_workflow` computes the same per-artifact
/// `required` classification for the workflow manifest contract.
pub(crate) fn artifact_required_by_active_loop(name: &str, repair_attempt_present: bool) -> bool {
    !(repair_attempt_present && REPAIR_ATTEMPT_SUPERSEDED_ARTIFACTS.contains(&name))
}

const MISSING_COMMAND_ORDER: &[&str] = &[
    "before_snapshot",
    "agent_packet",
    "agent_brief",
    "after_snapshot",
    "analysis_outcome",
    "agent_verify",
    "agent_receipt",
];

/// Loop steps that need a snapshot taken after the focused test edit.
///
/// Before that edit, the after snapshot equals the before snapshot and
/// verify has no movement to compare (#3906), so human renderings label
/// these steps instead of presenting them as runnable now.
const AFTER_TEST_EDIT_STEPS: &[&str] = &[
    "after_snapshot",
    "analysis_outcome",
    "agent_verify",
    "agent_receipt",
];

/// Human note printed with a next command that belongs after the test edit.
pub(crate) const AFTER_TEST_EDIT_NOTE: &str = "Run this after the focused test edit, not before: it needs a snapshot taken after the edit. Inside a repair transaction, the `--attempt ... --phase after` command runs this step instead.";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentStatusReport {
    pub(crate) root: String,
    pub(crate) seam: Option<AgentStatusSeam>,
    pub(crate) artifacts: Vec<AgentStatusArtifact>,
    pub(crate) repair_attempts: Vec<AgentStatusRepairAttempt>,
    pub(crate) missing_commands: Vec<AgentStatusCommand>,
    /// The one command status recommends, or `None` when it cannot choose
    /// honestly (the warnings then say why). Selection order is documented in
    /// RIPR-SPEC-0011: a current awaiting repair attempt first, then a new
    /// attempt for a seam whose attempts ended without a receipt, then the
    /// legacy artifact loop, which never emits a placeholder seam or a redirect
    /// into a directory that does not exist.
    pub(crate) next_command: Option<AgentStatusCommand>,
    pub(crate) warnings: Vec<AgentStatusWarning>,
}

/// One repair attempt as status sees it. `disposition` is status's reading of
/// the manifest against the current `HEAD` and, for a finished attempt,
/// against the receipt issued for it; `command` is what would move that
/// attempt forward, if anything.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentStatusRepairAttempt {
    pub(crate) attempt_id: String,
    pub(crate) seam_id: String,
    pub(crate) state: &'static str,
    pub(crate) head_current: Option<bool>,
    pub(crate) disposition: &'static str,
    pub(crate) manifest: String,
    pub(crate) command: Option<String>,
    /// The HEAD the attempt's evidence describes (its after phase's HEAD when
    /// it has one, otherwise its before phase's).
    pub(crate) evidence_head: String,
    pub(crate) receipt: AgentStatusAttemptReceipt,
    /// The refusal the attempt's most recent after phase recorded, if any.
    pub(crate) last_after_refusal: Option<AgentStatusAfterRefusal>,
    /// For an attempt awaiting its edit whose HEAD no longer descends from
    /// its prepared head: the recovery its after phase would print when it
    /// refuses (attempt authority's narration).
    pub(crate) diverged_recovery: Option<DivergedHeadRecovery>,
}

/// The workflow receipt read against one attempt. Only a `ready_to_finish`
/// attempt can have a receipt issued for it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AgentStatusAttemptReceipt {
    /// The attempt has no compliant, current after verdict to issue against.
    NotApplicable,
    /// No receipt at the workflow receipt path is bound to this attempt's
    /// after verdict (the file is absent, or it parses but belongs to other
    /// work). A receipt whose JSON cannot be read at all is `Unreadable`,
    /// not `NotIssued`: conflating them would tell an orchestrator the
    /// receipt was never issued.
    NotIssued,
    /// The workflow receipt is bound to a different repair attempt. The
    /// workflow keeps one receipt, so a later attempt's after phase replaced
    /// the receipt this attempt's after phase wrote; its outcome can no
    /// longer be read from it.
    Superseded { by_attempt_id: String },
    /// The receipt file at the workflow receipt path exists but is not
    /// parseable JSON, so status cannot tell whether it was issued for this
    /// attempt's after verdict. Distinct from `NotIssued`, which means no
    /// receipt file is there (or the readable file belongs to other work).
    Unreadable,
    /// The receipt bound to this attempt's after verdict, read through the
    /// receipt owner.
    Issued(AgentReceiptReading),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentStatusAfterRefusal {
    pub(crate) reason: String,
    pub(crate) recorded_unix_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentStatusSeam {
    pub(crate) seam_id: String,
    pub(crate) source: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentStatusArtifact {
    pub(crate) name: String,
    pub(crate) label: String,
    pub(crate) path: String,
    pub(crate) present: bool,
    pub(crate) bytes: Option<u64>,
    pub(crate) modified: Option<SystemTime>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentStatusCommand {
    pub(crate) step: String,
    pub(crate) artifact: String,
    pub(crate) reason: String,
    pub(crate) command: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentStatusWarning {
    pub(crate) kind: String,
    pub(crate) artifact: String,
    pub(crate) message: String,
}

impl AgentStatusCommand {
    /// Whether this step only makes sense after the focused test edit.
    pub(crate) fn runs_after_test_edit(&self) -> bool {
        AFTER_TEST_EDIT_STEPS.contains(&self.step.as_str())
    }
}

impl AgentStatusReport {
    /// The first issued receipt that records no test run. A cold agent reads
    /// `status: complete` plus movement `improved` as a pass, even when the
    /// focused test fails; this names the missing test run explicitly.
    pub(crate) fn unrun_test_receipt(&self) -> Option<&AgentReceiptReading> {
        self.repair_attempts
            .iter()
            .find_map(|attempt| match &attempt.receipt {
                AgentStatusAttemptReceipt::Issued(reading) if reading.test_not_run() => {
                    Some(reading)
                }
                _ => None,
            })
    }

    pub(crate) fn status(&self) -> &'static str {
        if self.next_command.is_some() || self.artifacts.iter().any(|artifact| !artifact.present) {
            "incomplete"
        } else if self.warnings.is_empty() {
            "complete"
        } else {
            "warning"
        }
    }
}

pub(crate) fn build_agent_status_report(root: &Path, root_argument: &Path) -> AgentStatusReport {
    let root_display = display_path(root_argument);
    // #3999: every next command binds the selected root once, here; the
    // report's `root` field keeps the invocation spelling.
    let command_root = bound_root(&root_display);
    keep_follow_up_templates_reachable(&command_root);
    let artifacts = ARTIFACTS
        .iter()
        .map(|artifact| inspect_artifact(root, artifact))
        .collect::<Vec<_>>();
    let mut warnings = Vec::new();
    let seam = recover_seam_id(root, &artifacts, &mut warnings);
    warnings.extend(stale_warnings(&artifacts));
    let missing_commands = missing_commands(root_argument, seam.as_ref(), &artifacts);
    let receipt = read_workflow_receipt(root);
    let repair_attempts = inspect_repair_attempts(root, &command_root, &receipt, &mut warnings);
    let next_command = select_next_command(
        root,
        &command_root,
        seam.as_ref(),
        repair_attempts.as_ref(),
        &missing_commands,
        &mut warnings,
    );

    AgentStatusReport {
        root: root_display,
        seam,
        artifacts,
        repair_attempts: repair_attempts.unwrap_or_default(),
        missing_commands,
        next_command,
        warnings,
    }
}

/// Reads every repair attempt through the attempt authority. `None` means the
/// inventory could not be trusted (an unreadable directory or manifest), in
/// which case a warning says so and status selects no command.
fn inspect_repair_attempts(
    root: &Path,
    root_display: &str,
    receipt: &WorkflowReceiptRead,
    warnings: &mut Vec<AgentStatusWarning>,
) -> Option<Vec<AgentStatusRepairAttempt>> {
    let entries = match inventory_repair_attempts(root) {
        Ok(entries) => entries,
        Err(error) => {
            warnings.push(AgentStatusWarning {
                kind: "repair_attempt_unreadable".to_string(),
                artifact: REPAIR_ATTEMPT_DIRECTORY.to_string(),
                message: format!("could not list repair attempts: {error}"),
            });
            return None;
        }
    };
    if entries.is_empty() {
        return Some(Vec::new());
    }
    let current_head = crate::agent::artifact::current_git_head(root).ok();
    let mut attempts = Vec::new();
    let mut trusted = true;
    for entry in entries {
        match entry {
            RepairAttemptInventoryEntry::Valid(manifest) => attempts.push(status_repair_attempt(
                root,
                root_display,
                &manifest,
                current_head.as_deref(),
                receipt,
            )),
            RepairAttemptInventoryEntry::Invalid { directory, error } => {
                trusted = false;
                warnings.push(AgentStatusWarning {
                    kind: "repair_attempt_unreadable".to_string(),
                    artifact: format!("{REPAIR_ATTEMPT_DIRECTORY}/{directory}/attempt.json"),
                    message: format!(
                        "repair attempt `{directory}` was refused ({error}); status selects no next command until it is repaired or removed"
                    ),
                });
            }
        }
    }
    if trusted {
        warnings.extend(finished_attempt_warnings(
            root_display,
            &attempts,
            current_head.as_deref(),
        ));
    }
    trusted.then_some(attempts)
}

/// The workflow receipt read against one attempt's after verdict. Status
/// reads the receipt only to match it against a finished attempt's after
/// verdict; a receipt whose JSON cannot be parsed keeps its own state so a
/// malformed file is never reported as "never issued".
#[derive(Clone, Debug, Eq, PartialEq)]
enum WorkflowReceiptRead {
    /// No receipt file exists at the workflow receipt path.
    Missing,
    /// The receipt file exists but is not parseable JSON.
    Unreadable,
    Parsed(Value),
}

/// The workflow receipt, when it exists and parses. Status reads it only to
/// match it against a finished attempt's after verdict.
fn read_workflow_receipt(root: &Path) -> WorkflowReceiptRead {
    let text = match std::fs::read_to_string(root.join(WORKFLOW_AGENT_RECEIPT_ARTIFACT)) {
        Ok(text) => text,
        Err(error) => {
            return if error.kind() == std::io::ErrorKind::NotFound {
                WorkflowReceiptRead::Missing
            } else {
                // The file exists but cannot be read (permissions, race with
                // a rewrite, ...): that is an unreadable receipt, never a
                // missing one — reporting it as Missing would claim the
                // receipt was never issued.
                WorkflowReceiptRead::Unreadable
            };
        }
    };
    match serde_json::from_str(&text) {
        Ok(value) => WorkflowReceiptRead::Parsed(value),
        Err(_) => WorkflowReceiptRead::Unreadable,
    }
}

/// Whether the workflow receipt was issued for exactly this attempt's after
/// verdict: the attempt-bound receipt records the attempt, its after HEAD, and
/// the delta and packet digests the finish measured. A receipt bound to
/// another attempt superseded this attempt's receipt (the workflow keeps one
/// receipt); a legacy unbound receipt, or one issued before a later finish of
/// the same attempt, does not match.
fn attempt_receipt(
    manifest: &RepairAttemptManifest,
    receipt: &WorkflowReceiptRead,
) -> AgentStatusAttemptReceipt {
    let Some(after) = manifest
        .after
        .as_ref()
        .filter(|_| manifest.state == RepairAttemptState::ReadyToFinish)
    else {
        return AgentStatusAttemptReceipt::NotApplicable;
    };
    let receipt = match receipt {
        WorkflowReceiptRead::Missing => return AgentStatusAttemptReceipt::NotIssued,
        WorkflowReceiptRead::Unreadable => return AgentStatusAttemptReceipt::Unreadable,
        WorkflowReceiptRead::Parsed(value) => value,
    };
    let bound = |pointer: &str, expected: &str| {
        receipt.pointer(pointer).and_then(Value::as_str) == Some(expected)
    };
    if bound("/repair_attempt/attempt_id", after.attempt_id.as_str())
        && bound("/repair_attempt/after_head", &after.repository_head)
        && bound("/repair_attempt/delta_sha256", &after.delta_sha256)
        && bound("/repair_attempt/packet_sha256", &after.packet_sha256)
    {
        AgentStatusAttemptReceipt::Issued(AgentReceiptReading::from_value(receipt))
    } else if let Some(other) = receipt
        .pointer("/repair_attempt/attempt_id")
        .and_then(Value::as_str)
        .filter(|other| *other != after.attempt_id.as_str())
    {
        AgentStatusAttemptReceipt::Superseded {
            by_attempt_id: other.to_string(),
        }
    } else {
        AgentStatusAttemptReceipt::NotIssued
    }
}

/// Status's reading of one attempt. `head_current` asks the question the
/// attempt's next step depends on: for an attempt awaiting its edit, whether
/// its after phase would evaluate the current HEAD as the attempt's (the
/// attempt authority's lineage rule: the prepared head, or for an ordinary
/// attempt a commit that descends from it); for an attempt with an after
/// verdict, whether HEAD is still the head that verdict recorded.
fn status_repair_attempt(
    root: &Path,
    root_display: &str,
    manifest: &RepairAttemptManifest,
    current_head: Option<&str>,
    receipt: &WorkflowReceiptRead,
) -> AgentStatusRepairAttempt {
    let restart = Some(new_repair_attempt_command(root_display, &manifest.seam_id));
    let receipt = attempt_receipt(manifest, receipt);
    let evidence_head = manifest.after.as_ref().map_or_else(
        || manifest.repository_head.clone(),
        |after| after.repository_head.clone(),
    );
    let mut diverged_recovery = None;
    let (head_current, (state, disposition, command)) = match manifest.state {
        RepairAttemptState::AwaitingEdit => {
            match current_head.map(|_| after_phase_head_admission(root, manifest)) {
                Some(Ok(AfterPhaseHeadAdmission::Current { .. })) => (
                    Some(true),
                    (
                        "awaiting_edit",
                        "resumable",
                        Some(manifest.next_command.clone()),
                    ),
                ),
                Some(Ok(AfterPhaseHeadAdmission::FinishesStale { .. })) => (
                    Some(false),
                    ("awaiting_edit", "prepared_at_other_head", restart),
                ),
                Some(Ok(AfterPhaseHeadAdmission::RefusedDiverged { current_head })) => {
                    diverged_recovery = Some(diverged_head_recovery(
                        root_display,
                        manifest.repair_attempt_id.as_str(),
                        &manifest.seam_id,
                        &manifest.repository_head,
                        &current_head,
                    ));
                    (
                        Some(false),
                        ("awaiting_edit", "prepared_at_other_head", restart),
                    )
                }
                // HEAD unreadable, or its lineage to the prepared head could not
                // be established: status cannot tell what the after phase would do.
                None | Some(Err(_)) => (None, ("awaiting_edit", "head_unknown", None)),
            }
        }
        _ => (
            current_head.map(|head| head == evidence_head),
            status_after_disposition(manifest, &receipt, restart),
        ),
    };
    AgentStatusRepairAttempt {
        attempt_id: manifest.repair_attempt_id.as_str().to_string(),
        seam_id: manifest.seam_id.clone(),
        state,
        head_current,
        disposition,
        manifest: format!(
            "{REPAIR_ATTEMPT_DIRECTORY}/{}/attempt.json",
            manifest.repair_attempt_id.as_str()
        ),
        command,
        evidence_head,
        receipt,
        last_after_refusal: manifest.last_after_refusal.as_ref().map(|refusal| {
            AgentStatusAfterRefusal {
                reason: refusal.reason.clone(),
                recorded_unix_ms: refusal.recorded_unix_ms,
            }
        }),
        diverged_recovery,
    }
}

/// State, disposition, and command for an attempt that is not awaiting its
/// edit.
fn status_after_disposition(
    manifest: &RepairAttemptManifest,
    receipt: &AgentStatusAttemptReceipt,
    restart: Option<String>,
) -> (&'static str, &'static str, Option<String>) {
    match manifest.state {
        RepairAttemptState::AwaitingEdit => ("awaiting_edit", "head_unknown", None),
        RepairAttemptState::Prepared => ("prepared", "not_published", restart),
        // `ready_to_finish` only says the edit cage admitted the edit. The
        // attempt reads `finished` only when the receipt issued for this
        // attempt is advisory with static grip improved (receipt owner). A
        // receipt whose grip did not rise leaves the gap open, so the seam is
        // restarted; any other reading is reported, never called finished.
        RepairAttemptState::ReadyToFinish => match receipt {
            AgentStatusAttemptReceipt::Issued(reading) if reading.shows_gap_closed() => {
                ("ready_to_finish", "finished", None)
            }
            AgentStatusAttemptReceipt::Issued(reading) if reading.leaves_gap_open() => {
                ("ready_to_finish", "gap_open", restart)
            }
            _ => ("ready_to_finish", "unconfirmed", None),
        },
        RepairAttemptState::Stale => ("stale", "ended", restart),
        RepairAttemptState::Incomparable => ("incomparable", "ended", restart),
        RepairAttemptState::Failed => ("failed", "ended", restart),
    }
}

/// Warnings for finished attempts whose receipt does not let status call the
/// loop complete. A seam with a new attempt awaiting its edit, or with a
/// `finished` attempt (advisory receipt, static grip improved) still at the
/// HEAD its after phase recorded, needs no warning. That after head may
/// itself descend from the prepared head (a committed focused test). Otherwise
/// each finished attempt whose evidence describes another HEAD is stale, and
/// each attempt whose receipt does not report improved grip says why.
fn finished_attempt_warnings(
    root_display: &str,
    attempts: &[AgentStatusRepairAttempt],
    current_head: Option<&str>,
) -> Vec<AgentStatusWarning> {
    let settled_seams = attempts
        .iter()
        .filter(|attempt| {
            attempt.disposition == "resumable"
                || (attempt.disposition == "finished" && attempt.head_current == Some(true))
        })
        .map(|attempt| attempt.seam_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let mut warnings = Vec::new();
    for attempt in attempts {
        if settled_seams.contains(attempt.seam_id.as_str()) {
            continue;
        }
        match attempt.disposition {
            "finished" => warnings.push(AgentStatusWarning {
                kind: "repair_receipt_stale".to_string(),
                artifact: WORKFLOW_AGENT_RECEIPT_ARTIFACT.to_string(),
                message: format!(
                    "the receipt for repair attempt `{}` (seam `{}`) reports static grip improved (receipt advisory) at HEAD `{}`, the head its after phase recorded, but {}; its static evidence is stale against HEAD, so status does not report the loop complete. Re-read the current HEAD with `ripr check --root {}` before relying on it",
                    attempt.attempt_id,
                    attempt.seam_id,
                    attempt.evidence_head,
                    match current_head {
                        Some(head) => format!("the repository HEAD is now `{head}`"),
                        None => "the current Git HEAD could not be read".to_string(),
                    },
                    shell_arg(root_display)
                ),
            }),
            "unconfirmed" => warnings.push(AgentStatusWarning {
                kind: "repair_receipt_unconfirmed".to_string(),
                artifact: WORKFLOW_AGENT_RECEIPT_ARTIFACT.to_string(),
                message: format!(
                    "repair attempt `{}` (seam `{}`) passed the edit cage, but {}; status does not report the loop complete{}{}",
                    attempt.attempt_id,
                    attempt.seam_id,
                    unconfirmed_receipt_reason(attempt),
                    match &attempt.receipt {
                        AgentStatusAttemptReceipt::Superseded { .. } => format!(
                            ". If seam `{}` still has its gap open, start a new attempt with `{}`",
                            attempt.seam_id,
                            new_repair_attempt_command(root_display, &attempt.seam_id)
                        ),
                        _ => String::new(),
                    },
                    match &attempt.last_after_refusal {
                        Some(refusal) => format!(
                            ". Its last after phase was refused: {}",
                            refusal.reason
                        ),
                        None => String::new(),
                    }
                ),
            }),
            _ => {}
        }
    }
    warnings
}

fn unconfirmed_receipt_reason(attempt: &AgentStatusRepairAttempt) -> String {
    match &attempt.receipt {
        AgentStatusAttemptReceipt::Issued(reading) if !reading.is_advisory() => format!(
            "its receipt is `{}`{} and an invalid or incomplete receipt does not show the gap closed (movement `{}`)",
            reading.status.as_deref().unwrap_or("unknown"),
            reading
                .analysis_outcome_error
                .as_deref()
                .map(|error| format!(" ({})", error.trim_end_matches('.')))
                .unwrap_or_default(),
            reading.movement.as_deref().unwrap_or("unknown")
        ),
        AgentStatusAttemptReceipt::Issued(reading) => format!(
            "its receipt reports movement `{}`, which does not show the gap closed{}",
            reading.movement.as_deref().unwrap_or("unknown"),
            reading
                .recommended_action
                .as_deref()
                .map(|action| format!(
                    "; the receipt's next action: {}",
                    action.trim_end_matches('.')
                ))
                .unwrap_or_default()
        ),
        AgentStatusAttemptReceipt::Superseded { by_attempt_id } => format!(
            "the receipt its after phase wrote to `{WORKFLOW_AGENT_RECEIPT_ARTIFACT}` was superseded by the receipt for repair attempt `{by_attempt_id}` (the workflow keeps one receipt), so status can no longer read this attempt's outcome"
        ),
        AgentStatusAttemptReceipt::Unreadable => format!(
            "the receipt at `{WORKFLOW_AGENT_RECEIPT_ARTIFACT}` exists but could not be parsed as JSON, so status cannot tell whether it was issued for this attempt's after verdict"
        ),
        _ => format!(
            "no receipt at `{WORKFLOW_AGENT_RECEIPT_ARTIFACT}` was issued for its after verdict"
        ),
    }
}

/// The documented start of the repair transaction. It creates every artifact
/// it needs, so it never depends on a workflow directory already existing.
fn new_repair_attempt_command(root_display: &str, seam_id: &str) -> String {
    format!(
        "ripr agent repair --root {} --seam-id {} --phase before",
        shell_arg(root_display),
        shell_arg(seam_id)
    )
}

fn select_next_command(
    root: &Path,
    root_display: &str,
    seam: Option<&AgentStatusSeam>,
    repair_attempts: Option<&Vec<AgentStatusRepairAttempt>>,
    missing_commands: &[AgentStatusCommand],
    warnings: &mut Vec<AgentStatusWarning>,
) -> Option<AgentStatusCommand> {
    // An inventory status could not read is not an empty one: choosing a
    // command past it could resume or restart the wrong transaction.
    let attempts = repair_attempts?;

    let resumable = attempts
        .iter()
        .filter(|attempt| attempt.disposition == "resumable")
        .collect::<Vec<_>>();
    match resumable.as_slice() {
        [attempt] => {
            let reason = match &attempt.last_after_refusal {
                // The same command is still the only way forward for this
                // attempt, but status must not present it as if it had never
                // run: it names the refusal the attempt recorded.
                Some(refusal) => format!(
                    "the last after phase of repair attempt `{}` for seam `{}` was refused: {}. The attempt still awaits the focused test edit; the command below repeats that after phase and refuses again until the cause the refusal names is resolved",
                    attempt.attempt_id,
                    attempt.seam_id,
                    refusal.reason.trim_end_matches('.')
                ),
                None => format!(
                    "repair attempt `{}` for seam `{}` is awaiting the focused test edit; once the test is in place, run the after phase its before phase recorded",
                    attempt.attempt_id, attempt.seam_id
                ),
            };
            return Some(AgentStatusCommand {
                step: "repair_attempt_after".to_string(),
                artifact: attempt.manifest.clone(),
                reason,
                command: attempt.command.clone().unwrap_or_default(),
            });
        }
        [] => {}
        several => {
            warnings.push(AgentStatusWarning {
                kind: "ambiguous_repair_attempts".to_string(),
                artifact: REPAIR_ATTEMPT_DIRECTORY.to_string(),
                message: format!(
                    "{} repair attempts are awaiting an edit at the current HEAD; status does not choose between them. Run the after phase of the attempt you edited: {}",
                    several.len(),
                    attempt_command_list(several)
                ),
            });
            return None;
        }
    }

    let head_unknown = attempts
        .iter()
        .filter(|attempt| attempt.disposition == "head_unknown")
        .collect::<Vec<_>>();
    if !head_unknown.is_empty() {
        warnings.push(AgentStatusWarning {
            kind: "repair_attempt_head_unknown".to_string(),
            artifact: REPAIR_ATTEMPT_DIRECTORY.to_string(),
            message: format!(
                "the current Git HEAD could not be read or related to the head its attempt was prepared at, so status cannot tell whether {} awaiting repair attempt(s) are still current",
                head_unknown.len()
            ),
        });
        return None;
    }

    // A seam is open when none of its attempts finished and at least one ended,
    // went stale, or finished with a receipt that leaves the gap open.
    // Grouping by seam rather than picking "the latest" attempt keeps status
    // from reading creation order into the inventory. A finished attempt whose
    // receipt status cannot read as closed or open is not restarted: the
    // warnings say why instead.
    let finished_seams = attempts
        .iter()
        .filter(|attempt| matches!(attempt.disposition, "finished" | "unconfirmed"))
        .map(|attempt| attempt.seam_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let mut open = std::collections::BTreeMap::<&str, Vec<&AgentStatusRepairAttempt>>::new();
    for attempt in attempts {
        if attempt.command.is_some() && !finished_seams.contains(attempt.seam_id.as_str()) {
            open.entry(attempt.seam_id.as_str())
                .or_default()
                .push(attempt);
        }
    }
    let mut open_seams = open.into_iter();
    match (open_seams.next(), open_seams.next()) {
        (Some((seam_id, ended)), None) => {
            let restart = if ended
                .iter()
                .any(|attempt| attempt.disposition == "gap_open")
            {
                "the gap is still open, so start a new attempt and strengthen the focused test before its after phase"
            } else if ended
                .iter()
                .any(|attempt| attempt.diverged_recovery.is_some())
            {
                "restore the prepared head as described to continue that attempt, or start a new attempt while the gap still exists"
            } else {
                "start a new attempt"
            };
            return Some(AgentStatusCommand {
                step: "repair_attempt_before".to_string(),
                artifact: REPAIR_ATTEMPT_DIRECTORY.to_string(),
                reason: format!(
                    "no repair attempt for seam `{seam_id}` can continue at the current HEAD ({}); {restart}",
                    ended
                        .iter()
                        .map(|attempt| format!(
                            "`{}` is {}",
                            attempt.attempt_id,
                            attempt_condition(attempt)
                        ))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                command: new_repair_attempt_command(root_display, seam_id),
            });
        }
        (Some((first, _)), Some((second, _))) => {
            let mut seams = vec![first, second];
            seams.extend(open_seams.map(|(seam_id, _)| seam_id));
            warnings.push(AgentStatusWarning {
                kind: "multiple_open_repair_seams".to_string(),
                artifact: REPAIR_ATTEMPT_DIRECTORY.to_string(),
                message: format!(
                    "repair attempts for {} seams ended without a receipt; status does not choose between them. Start a new attempt for the seam you mean: {}",
                    seams.len(),
                    seams
                        .iter()
                        .map(|seam_id| format!("`{}`", new_repair_attempt_command(root_display, seam_id)))
                        .collect::<Vec<_>>()
                        .join("; ")
                ),
            });
            return None;
        }
        (None, _) => {}
    }

    legacy_next_command(root, root_display, seam, missing_commands, warnings)
}

/// Where `ripr pilot` writes its summary by default, relative to the root.
const PILOT_SUMMARY_ARTIFACT: &str = "target/ripr/pilot/pilot-summary.json";

/// The seam-selection route status offers before any seam is known. `ripr
/// pilot` resolves a relative `--out` against the working directory, not
/// `--root`, so the command names the pilot directory under the selected root
/// explicitly; pasted from any directory it writes the summary status reads
/// next (#4000).
///
/// The root is bound here, once (#4287): a caller may pass the raw `--root`
/// or an already bound one. Binding is idempotent for an absolute root, so
/// `--root` and `--out` always name the same bound directory and the pilot
/// directory is never anchored twice.
pub(crate) fn pilot_select_command(root: &str) -> String {
    let root = bound_root(root);
    format!(
        "ripr pilot --root {} --out {}",
        shell_arg(&root),
        shell_arg(&anchored_redirect_target(&root, "target/ripr/pilot"))
    )
}

/// The repair start `ripr pilot` recorded for its top seam (#3906), carried
/// verbatim. Pilot fills `next.repair_command` only past the repair-packet
/// flip, so status repeats that decision instead of re-deriving it. A missing,
/// unreadable, `null`, or non-repair value leaves status on `select_seam`.
fn pilot_repair_command(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join(PILOT_SUMMARY_ARTIFACT)).ok()?;
    let summary = serde_json::from_str::<Value>(&text).ok()?;
    summary
        .pointer("/next/repair_command")
        .and_then(Value::as_str)
        .filter(|command| command.starts_with("ripr agent repair "))
        .map(str::to_string)
}

/// Whether a complete `ripr pilot` run ranked a top seam and recorded no
/// repair start for it (`next.repair_command` is an explicit `null`). Sending the
/// user back to pilot then only ranks the same seam again (#4216 row 3).
/// A missing, unreadable, timed-out, or non-null summary is not this fact.
fn pilot_found_no_repair_target(root: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(root.join(PILOT_SUMMARY_ARTIFACT)) else {
        return false;
    };
    let Ok(summary) = serde_json::from_str::<Value>(&text) else {
        return false;
    };
    summary.pointer("/status").and_then(Value::as_str) == Some("complete")
        && summary
            .pointer("/top_actionable_seams")
            .and_then(Value::as_array)
            .is_some_and(|seams| !seams.is_empty())
        && summary
            .pointer("/next/repair_command")
            .is_some_and(Value::is_null)
}

/// Python first-use statuses that record "pilot produced no repair card"
/// (`output::pilot::types::PilotPythonFirstUseStatus`). `analysis_unavailable`
/// is a failed analysis, not that fact, and `ready` has repair cards.
const PILOT_NO_REPAIR_CARD_PYTHON_STATUSES: &[&str] = &["no_python_findings", "no_repair_cards"];

/// The diff-first routes a complete `ripr pilot` run recorded when it ranked
/// no seam, produced no repair card, and recorded no repair start.
struct PilotCheckRoutes {
    /// Languages whose route is enabled in `[languages]`, with their
    /// distinct recorded check commands.
    enabled_languages: Vec<String>,
    commands: Vec<String>,
    /// Routed languages the effective config does not enable, as the
    /// `[languages] enabled` entry that turns each on.
    disabled_config_languages: Vec<String>,
}

/// Reads the retained pilot summary (#4216, Python and TypeScript
/// re-walks): `status: complete`, empty `top_actionable_seams`, an explicit
/// null `next.repair_command`, `language_routes.state: required` (pilot
/// itself sends the code to `ripr check`, which offers no `ripr agent
/// repair` start), and no repair card (`python_first_use` absent, null, or
/// a no-repair-card status). Sending the user back to pilot then only routes
/// them to `ripr check` again, for any routed language. A missing,
/// unreadable, timed-out, seam-ranking, repair-card-bearing,
/// analysis-unavailable, or command-less summary is not this fact.
fn pilot_routed_changed_code_to_check(root: &Path) -> Option<PilotCheckRoutes> {
    let text = std::fs::read_to_string(root.join(PILOT_SUMMARY_ARTIFACT)).ok()?;
    let summary = serde_json::from_str::<Value>(&text).ok()?;
    let complete = summary.pointer("/status").and_then(Value::as_str) == Some("complete");
    let no_seams = summary
        .pointer("/top_actionable_seams")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty);
    let no_repair_start = summary
        .pointer("/next/repair_command")
        .is_some_and(Value::is_null);
    let routes_required = summary
        .pointer("/language_routes/state")
        .and_then(Value::as_str)
        == Some("required");
    let no_repair_cards = summary
        .pointer("/python_first_use")
        .filter(|first_use| !first_use.is_null())
        .is_none_or(|first_use| {
            first_use
                .get("status")
                .and_then(Value::as_str)
                .is_some_and(|status| PILOT_NO_REPAIR_CARD_PYTHON_STATUSES.contains(&status))
        });
    if !(complete && no_seams && no_repair_start && routes_required && no_repair_cards) {
        return None;
    }
    let mut routes = PilotCheckRoutes {
        enabled_languages: Vec::new(),
        commands: Vec::new(),
        disabled_config_languages: Vec::new(),
    };
    for route in summary
        .pointer("/language_routes/routes")
        .and_then(Value::as_array)?
    {
        let Some(command) = route.get("command").and_then(Value::as_str) else {
            continue;
        };
        let language = route.get("language").and_then(Value::as_str);
        if route.get("enabled").and_then(Value::as_bool) == Some(true) {
            if let Some(language) = language {
                routes.enabled_languages.push(language.to_string());
            }
            if !routes.commands.iter().any(|known| known == command) {
                routes.commands.push(command.to_string());
            }
        } else if let Some(language) = language {
            // JavaScript runs through the TypeScript-family adapter, which
            // the `typescript` entry turns on (pilot `language_routes`).
            let config_language = if language == "javascript" {
                "typescript"
            } else {
                language
            };
            if !routes
                .disabled_config_languages
                .iter()
                .any(|known| known == config_language)
            {
                routes
                    .disabled_config_languages
                    .push(config_language.to_string());
            }
        }
    }
    (!routes.commands.is_empty() || !routes.disabled_config_languages.is_empty()).then_some(routes)
}

/// "the python, typescript code", or "the code" for an empty list.
fn routed_code_phrase(languages: &[String]) -> String {
    if languages.is_empty() {
        "the code".to_string()
    } else {
        format!("the {} code", languages.join(", "))
    }
}

fn pilot_routed_to_check_message(routes: &PilotCheckRoutes, root_display: &str) -> String {
    let mut message =
        "the last complete `ripr pilot` run ranked no seam and produced no repair card".to_string();
    if !routes.commands.is_empty() {
        let commands = routes
            .commands
            .iter()
            .map(|command| format!("`{command}`"))
            .collect::<Vec<_>>()
            .join(", ");
        message.push_str(&format!(
            ": it routed {} to {commands}, which reviews changes diff-first and offers no `ripr agent repair` start, and running pilot again unchanged routes there again. Read the findings from {commands}, add or strengthen a test for the changed behavior by hand, then rerun {commands} to see whether static evidence now finds a discriminator",
            routed_code_phrase(&routes.enabled_languages)
        ));
    }
    if !routes.disabled_config_languages.is_empty() {
        let entries = routes
            .disabled_config_languages
            .iter()
            .map(|language| format!("\"{language}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let lead = if routes.commands.is_empty() {
            ": "
        } else {
            ". Also, "
        };
        message.push_str(&format!(
            "{lead}{} is not enabled in ripr.toml [languages], so `ripr check` does not analyze it yet: add {entries} to `[languages] enabled` in ripr.toml, then run `ripr check --root {}`",
            routed_code_phrase(&routes.disabled_config_languages),
            shell_arg(root_display)
        ));
    }
    message.push_str(&format!(
        ". If the workspace changed since that run, rerun `{}`",
        pilot_select_command(root_display)
    ));
    message
}

/// The legacy seven-artifact loop, kept for `agent start` and manual users,
/// with three refusals: it never recommends a command that needs a seam status
/// does not know, never a redirect into a directory that does not exist, and
/// never `ripr pilot` again after a complete pilot run found no repair start.
fn legacy_next_command(
    root: &Path,
    root_display: &str,
    seam: Option<&AgentStatusSeam>,
    missing_commands: &[AgentStatusCommand],
    warnings: &mut Vec<AgentStatusWarning>,
) -> Option<AgentStatusCommand> {
    let first = missing_commands.first()?;
    let Some(seam) = seam else {
        if let Some(command) = pilot_repair_command(root) {
            return Some(AgentStatusCommand {
                step: "repair_attempt_before".to_string(),
                artifact: PILOT_SUMMARY_ARTIFACT.to_string(),
                reason: "`ripr pilot` selected a seam the repair transaction can target; start its repair attempt".to_string(),
                command,
            });
        }
        if pilot_found_no_repair_target(root) {
            warnings.push(AgentStatusWarning {
                kind: "pilot_found_no_repair_target".to_string(),
                artifact: PILOT_SUMMARY_ARTIFACT.to_string(),
                message: "the last complete `ripr pilot` run offered no repair attempt: its top seam is not eligible for `ripr agent repair`, and running pilot again unchanged ranks the same seams. Read `target/ripr/pilot/pilot-summary.md` for that seam, add a test for it by hand in the crate that owns it, then rerun `ripr pilot` to rank the seams against that test".to_string(),
            });
            return None;
        }
        if let Some(routes) = pilot_routed_changed_code_to_check(root) {
            warnings.push(AgentStatusWarning {
                kind: "pilot_routed_to_check_no_repair_target".to_string(),
                artifact: PILOT_SUMMARY_ARTIFACT.to_string(),
                message: pilot_routed_to_check_message(&routes, root_display),
            });
            return None;
        }
        return Some(AgentStatusCommand {
            step: "select_seam".to_string(),
            artifact: "target/ripr/pilot".to_string(),
            reason: "no repair seam is known yet; `ripr pilot` inspects the workspace and selects the seam to repair".to_string(),
            command: pilot_select_command(root_display),
        });
    };
    let target_directory_exists = Path::new(&first.artifact)
        .parent()
        .is_none_or(|parent| root.join(parent).is_dir());
    if !target_directory_exists {
        return Some(AgentStatusCommand {
            step: "repair_attempt_before".to_string(),
            artifact: REPAIR_ATTEMPT_DIRECTORY.to_string(),
            reason: format!(
                "{}; start a repair attempt for seam `{}`, which writes the workflow artifacts itself",
                first.reason, seam.seam_id
            ),
            command: new_repair_attempt_command(root_display, &seam.seam_id),
        });
    }
    Some(first.clone())
}

fn attempt_condition(attempt: &AgentStatusRepairAttempt) -> String {
    match attempt.disposition {
        "prepared_at_other_head" => match &attempt.diverged_recovery {
            Some(recovery) => format!(
                "awaiting an edit, but its after phase would refuse: {} {}",
                recovery.cause,
                recovery.reset.trim_end_matches('.')
            ),
            None => "awaiting an edit but was prepared at a different HEAD".to_string(),
        },
        "not_published" => "prepared but never published".to_string(),
        "gap_open" => {
            let AgentStatusAttemptReceipt::Issued(reading) = &attempt.receipt else {
                return attempt.state.to_string();
            };
            let mut condition = format!(
                "finished, but its receipt (status `{}`, movement `{}`) does not show the gap closed",
                reading.status.as_deref().unwrap_or("unknown"),
                reading.movement.as_deref().unwrap_or("unknown")
            );
            if attempt.head_current != Some(true) {
                condition.push_str(&format!(
                    ", and its evidence is stale against HEAD (recorded at `{}`)",
                    attempt.evidence_head
                ));
            }
            if let Some(action) = &reading.recommended_action {
                condition.push_str(&format!(
                    "; the receipt's next action: {}",
                    action.trim_end_matches('.')
                ));
            }
            condition
        }
        _ => attempt.state.to_string(),
    }
}

/// One-cell summary of what an attempt produced, for the Markdown table.
fn attempt_outcome(attempt: &AgentStatusRepairAttempt) -> String {
    let mut parts = Vec::new();
    match &attempt.receipt {
        AgentStatusAttemptReceipt::Issued(reading) => parts.push(format!(
            "receipt `{}`, movement `{}`",
            reading.status.as_deref().unwrap_or("unknown"),
            reading.movement.as_deref().unwrap_or("unknown")
        )),
        AgentStatusAttemptReceipt::NotIssued => {
            parts.push("no receipt issued for this attempt".to_string());
        }
        AgentStatusAttemptReceipt::Superseded { by_attempt_id } => {
            parts.push(format!("receipt superseded by attempt `{by_attempt_id}`"));
        }
        AgentStatusAttemptReceipt::Unreadable => {
            parts.push("receipt present but unreadable".to_string());
        }
        AgentStatusAttemptReceipt::NotApplicable => {}
    }
    match attempt.disposition {
        "finished" => parts.push("static grip improved (receipt advisory)".to_string()),
        "gap_open" => parts.push("gap still open".to_string()),
        "unconfirmed" => parts.push("gap closure not confirmed".to_string()),
        _ => {}
    }
    if attempt.receipt != AgentStatusAttemptReceipt::NotApplicable {
        match attempt.head_current {
            Some(true) => {}
            Some(false) => parts.push("evidence stale against HEAD".to_string()),
            None => parts.push("HEAD unknown".to_string()),
        }
    }
    if attempt.last_after_refusal.is_some() {
        parts.push("last after phase refused".to_string());
    }
    if parts.is_empty() {
        "-".to_string()
    } else {
        parts.join("; ")
    }
}

fn attempt_command_list(attempts: &[&AgentStatusRepairAttempt]) -> String {
    attempts
        .iter()
        .map(|attempt| {
            format!(
                "`{}` (seam `{}`): `{}`",
                attempt.attempt_id,
                attempt.seam_id,
                attempt.command.as_deref().unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn keep_follow_up_templates_reachable(root: &str) {
    drop((
        agent_status_command(root, Some(WORKFLOW_AGENT_STATUS_ARTIFACT)),
        agent_status_markdown_command(root, Some(WORKFLOW_AGENT_STATUS_MARKDOWN_ARTIFACT)),
        agent_review_summary_command(root, Some(WORKFLOW_AGENT_REVIEW_SUMMARY_ARTIFACT)),
        agent_review_summary_markdown_command(
            root,
            Some(WORKFLOW_AGENT_REVIEW_SUMMARY_MARKDOWN_ARTIFACT),
        ),
    ));
}

pub(crate) fn render_agent_status_json(report: &AgentStatusReport) -> Result<String, String> {
    let next_command = report.next_command.as_ref().map(agent_status_command_json);
    let repair_attempt_present = !report.repair_attempts.is_empty();
    let artifacts = report
        .artifacts
        .iter()
        .map(|artifact| agent_status_artifact_json(artifact, repair_attempt_present))
        .collect::<Vec<_>>();
    let value = serde_json::json!({
        "schema_version": AGENT_STATUS_SCHEMA_VERSION,
        "tool": "ripr",
        "status": report.status(),
        "root": report.root,
        "seam": report.seam.as_ref().map(agent_status_seam_json),
        "artifacts": artifacts,
        "repair_attempts": report.repair_attempts.iter().map(agent_status_repair_attempt_json).collect::<Vec<_>>(),
        "missing_commands": report.missing_commands.iter().map(agent_status_command_json).collect::<Vec<_>>(),
        "next_command": next_command,
        "test_run": report.unrun_test_receipt().map(test_not_run_json),
        "warnings": report.warnings.iter().map(agent_status_warning_json).collect::<Vec<_>>()
    });
    serde_json::to_string_pretty(&value)
        .map(|mut rendered| {
            rendered.push('\n');
            rendered
        })
        .map_err(|err| format!("failed to render agent status JSON: {err}"))
}

pub(crate) fn render_agent_status_markdown(report: &AgentStatusReport) -> String {
    let mut rendered = String::new();
    rendered.push_str("# RIPR Agent Status\n\n");
    rendered.push_str(&format!("Status: {}\n", report.status()));
    rendered.push_str(&format!("Root: {}\n", report.root));
    match &report.seam {
        Some(seam) => rendered.push_str(&format!("Seam: {} ({})\n", seam.seam_id, seam.source)),
        None => rendered.push_str("Seam: unknown\n"),
    }
    if let Some(reading) = report.unrun_test_receipt() {
        rendered.push_str(&format!(
            "Test run: none recorded. {}\n",
            test_not_run_next_step(reading)
        ));
    }

    rendered.push_str("\n## Artifacts\n\n");
    rendered.push_str("| Artifact | State | Path |\n");
    rendered.push_str("| --- | --- | --- |\n");
    for artifact in &report.artifacts {
        let state = if artifact.present {
            "present"
        } else {
            "missing"
        };
        rendered.push_str(&format!(
            "| {} | {} | `{}` |\n",
            artifact.label, state, artifact.path
        ));
    }

    if !report.repair_attempts.is_empty() {
        rendered.push_str("\n## Repair Attempts\n\n");
        rendered.push_str("| Attempt | Seam | State | Status reading | Outcome |\n");
        rendered.push_str("| --- | --- | --- | --- | --- |\n");
        for attempt in &report.repair_attempts {
            rendered.push_str(&format!(
                "| `{}` | `{}` | {} | {} | {} |\n",
                attempt.attempt_id,
                attempt.seam_id,
                attempt.state,
                attempt.disposition,
                attempt_outcome(attempt)
            ));
        }
    }

    if let Some(next) = &report.next_command {
        rendered.push_str("\n## Next Command\n\n");
        rendered.push_str(&format!("{}\n\n", next.reason));
        if next.runs_after_test_edit() {
            rendered.push_str(&format!("{AFTER_TEST_EDIT_NOTE}\n\n"));
        }
        rendered.push_str(COMMAND_SHELL_DISCLOSURE);
        rendered.push_str("```bash\n");
        rendered.push_str(&next.command);
        rendered.push_str("\n```\n");
        match powershell_form(&next.command) {
            PowershellForm::Translated(line) => {
                rendered.push_str("\n```powershell\n");
                rendered.push_str(&line);
                rendered.push_str("\n```\n");
            }
            PowershellForm::SameAsBash => {}
            PowershellForm::Unavailable => rendered.push_str(&format!(
                "{}: `{}`\n",
                crate::output::markdown::POWERSHELL_UNAVAILABLE_DISCLOSURE,
                next.command
            )),
        }
    } else if report.missing_commands.is_empty() && report.warnings.is_empty() {
        rendered.push_str("\nNo missing agent-loop artifacts were detected.\n");
    } else if report.missing_commands.is_empty() {
        rendered.push_str("\nNo missing agent-loop artifacts were detected, but status does not report the loop complete; the warnings below say why.\n");
    } else {
        rendered.push_str("\n## Next Command\n\nStatus selects no next command; the warnings below say why and list the choices.\n");
    }

    if !report.warnings.is_empty() {
        rendered.push_str("\n## Warnings\n\n");
        for warning in &report.warnings {
            rendered.push_str(&format!(
                "- {}: {} (`{}`)\n",
                warning.kind, warning.message, warning.artifact
            ));
        }
    }

    rendered.push_str("\n## Limits\n\n");
    rendered.push_str("- Reads existing artifacts only.\n");
    rendered.push_str("- No repo analysis is run by this command.\n");
    rendered.push_str("- No runtime mutation execution.\n");
    rendered.push_str("- No automatic source edits.\n");
    rendered.push_str("- No generated tests.\n");
    rendered
}

fn agent_status_seam_json(seam: &AgentStatusSeam) -> Value {
    serde_json::json!({
        "seam_id": seam.seam_id,
        "source": seam.source
    })
}

fn agent_status_artifact_json(
    artifact: &AgentStatusArtifact,
    repair_attempt_present: bool,
) -> Value {
    serde_json::json!({
        "name": artifact.name,
        "label": artifact.label,
        "path": artifact.path,
        "required": artifact_required_by_active_loop(&artifact.name, repair_attempt_present),
        "state": if artifact.present { "present" } else { "missing" },
        "bytes": artifact.bytes,
        "modified_unix_ms": modified_unix_ms(artifact.modified)
    })
}

fn agent_status_repair_attempt_json(attempt: &AgentStatusRepairAttempt) -> Value {
    serde_json::json!({
        "attempt_id": attempt.attempt_id,
        "seam_id": attempt.seam_id,
        "state": attempt.state,
        "head_current": attempt.head_current,
        "disposition": attempt.disposition,
        "manifest": attempt.manifest,
        "command": attempt.command,
        "receipt": attempt_receipt_json(&attempt.receipt),
        "last_after_refusal": attempt.last_after_refusal.as_ref().map(|refusal| serde_json::json!({
            "reason": refusal.reason,
            "recorded_unix_ms": refusal.recorded_unix_ms
        }))
    })
}

fn attempt_receipt_json(receipt: &AgentStatusAttemptReceipt) -> Value {
    let (reading, superseded_by, unreadable) = match receipt {
        AgentStatusAttemptReceipt::NotApplicable => return Value::Null,
        AgentStatusAttemptReceipt::NotIssued => (None, None, false),
        AgentStatusAttemptReceipt::Superseded { by_attempt_id } => {
            (None, Some(by_attempt_id.as_str()), false)
        }
        AgentStatusAttemptReceipt::Unreadable => (None, None, true),
        AgentStatusAttemptReceipt::Issued(reading) => (Some(reading), None, false),
    };
    serde_json::json!({
        "path": WORKFLOW_AGENT_RECEIPT_ARTIFACT,
        "issued_for_attempt": reading.is_some(),
        "unreadable": unreadable,
        "superseded_by": superseded_by,
        "status": reading.and_then(|reading| reading.status.as_deref()),
        "movement": reading.and_then(|reading| reading.movement.as_deref()),
        "receipt_state": reading.map(|reading| reading.receipt_state.as_str()),
        "shows_gap_closed": reading.is_some_and(AgentReceiptReading::shows_gap_closed),
        "recommended_action": reading.and_then(|reading| reading.recommended_action.as_deref()),
        "verification_status": reading.and_then(|reading| reading.verification_status.as_deref()),
        "analysis_outcome_error": reading.and_then(|reading| reading.analysis_outcome_error.as_deref())
    })
}

/// One sentence for a repair receipt that records no test run: what the
/// receipt does not establish and the step that still decides whether the
/// test is kept. It speaks for the receipt only; a separate verification
/// receipt (the trust-bound `--phase verify` route) is not read here.
fn test_not_run_next_step(reading: &AgentReceiptReading) -> String {
    let target = reading
        .test_changed
        .as_deref()
        // The receipt's `test_changed` is whatever `--test` named: a path or a
        // test identifier, so it is quoted, not presented as a file.
        .map(|test| format!("the focused test (`{test}`)"))
        .unwrap_or_else(|| "the focused test".to_string());
    format!(
        "The repair receipt compares static evidence only and records no run of {target}; run it with the project's test command and keep it only if it passes. A failing test can still show movement `improved`."
    )
}

fn test_not_run_json(reading: &AgentReceiptReading) -> Value {
    serde_json::json!({
        "status": "not_recorded",
        "test_changed": reading.test_changed,
        "next_step": test_not_run_next_step(reading)
    })
}

fn agent_status_command_json(command: &AgentStatusCommand) -> Value {
    serde_json::json!({
        "step": command.step,
        "artifact": command.artifact,
        "reason": command.reason,
        "command": command.command
    })
}

fn agent_status_warning_json(warning: &AgentStatusWarning) -> Value {
    serde_json::json!({
        "kind": warning.kind,
        "artifact": warning.artifact,
        "message": warning.message
    })
}

fn inspect_artifact(root: &Path, artifact: &AgentStatusArtifactDef) -> AgentStatusArtifact {
    let path = root.join(artifact.path);
    match std::fs::metadata(&path) {
        Ok(metadata) if metadata.is_file() => AgentStatusArtifact {
            name: artifact.name.to_string(),
            label: artifact.label.to_string(),
            path: artifact.path.to_string(),
            present: true,
            bytes: Some(metadata.len()),
            modified: metadata.modified().ok(),
        },
        _ => AgentStatusArtifact {
            name: artifact.name.to_string(),
            label: artifact.label.to_string(),
            path: artifact.path.to_string(),
            present: false,
            bytes: None,
            modified: None,
        },
    }
}

fn recover_seam_id(
    root: &Path,
    artifacts: &[AgentStatusArtifact],
    warnings: &mut Vec<AgentStatusWarning>,
) -> Option<AgentStatusSeam> {
    for (artifact_name, source) in [
        ("agent_receipt", "agent_receipt"),
        ("agent_verify", "agent_verify"),
        ("agent_packet", "agent_packet"),
        ("agent_brief", "agent_brief"),
    ] {
        let Some(artifact) = artifact_by_name(artifacts, artifact_name).filter(|a| a.present)
        else {
            continue;
        };
        let path = root.join(&artifact.path);
        let Some(value) = read_json_artifact(&path, artifact, warnings) else {
            continue;
        };
        if let Some(seam_id) = seam_id_from_source(&value, source) {
            return Some(AgentStatusSeam {
                seam_id,
                source: source.to_string(),
            });
        }
    }
    None
}

fn read_json_artifact(
    path: &Path,
    artifact: &AgentStatusArtifact,
    warnings: &mut Vec<AgentStatusWarning>,
) -> Option<Value> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => {
            warnings.push(AgentStatusWarning {
                kind: "artifact_json_unreadable".to_string(),
                artifact: artifact.name.clone(),
                message: format!("could not read {}: {err}", artifact.path),
            });
            return None;
        }
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(value) => Some(value),
        Err(err) => {
            warnings.push(AgentStatusWarning {
                kind: "artifact_json_unreadable".to_string(),
                artifact: artifact.name.clone(),
                message: format!("could not parse {} as JSON: {err}", artifact.path),
            });
            None
        }
    }
}

fn seam_id_from_source(value: &Value, source: &str) -> Option<String> {
    match source {
        "agent_receipt" => value
            .get("seam")
            .and_then(|seam| string_field(seam, "seam_id")),
        "agent_verify" => seam_id_from_verify(value),
        "agent_packet" => value
            .get("packets")
            .and_then(Value::as_array)
            .and_then(|packets| packets.first())
            .and_then(|packet| string_field(packet, "seam_id")),
        "agent_brief" => value
            .get("working_set")
            .and_then(|working_set| string_field(working_set, "seam_id"))
            .or_else(|| {
                value
                    .get("top_seams")
                    .and_then(Value::as_array)
                    .and_then(|seams| seams.first())
                    .and_then(|seam| string_field(seam, "seam_id"))
            }),
        _ => None,
    }
}

fn seam_id_from_verify(value: &Value) -> Option<String> {
    for bucket in [
        "changed_seams",
        "unchanged_seams",
        "new_gaps",
        "resolved_gaps",
    ] {
        let Some(seam_id) = value
            .get(bucket)
            .and_then(Value::as_array)
            .and_then(|seams| seams.first())
            .and_then(|seam| string_field(seam, "seam_id"))
        else {
            continue;
        };
        return Some(seam_id);
    }
    None
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
}

fn stale_warnings(artifacts: &[AgentStatusArtifact]) -> Vec<AgentStatusWarning> {
    let mut warnings = Vec::new();
    push_snapshot_order_warning(artifacts, &mut warnings);
    push_stale_warning(
        artifacts,
        "agent_verify",
        "before_snapshot",
        "agent verify is older than the before snapshot; rerun verify after refreshing snapshots",
        &mut warnings,
    );
    push_stale_warning(
        artifacts,
        "agent_verify",
        "after_snapshot",
        "agent verify is older than the after snapshot; rerun verify after refreshing snapshots",
        &mut warnings,
    );
    push_stale_warning(
        artifacts,
        "agent_receipt",
        "agent_verify",
        "agent receipt is older than agent verify; rerun receipt after refreshing verify",
        &mut warnings,
    );
    warnings
}

fn push_snapshot_order_warning(
    artifacts: &[AgentStatusArtifact],
    warnings: &mut Vec<AgentStatusWarning>,
) {
    let Some(before) = artifact_by_name(artifacts, "before_snapshot").filter(|a| a.present) else {
        return;
    };
    let Some(after) = artifact_by_name(artifacts, "after_snapshot").filter(|a| a.present) else {
        return;
    };
    let (Some(before_modified), Some(after_modified)) = (before.modified, after.modified) else {
        return;
    };
    if before_modified > after_modified {
        warnings.push(AgentStatusWarning {
            kind: "stale_artifact".to_string(),
            artifact: before.name.clone(),
            message: "before snapshot is newer than after snapshot; the before artifact may have been overwritten after editing".to_string(),
        });
    }
}

fn push_stale_warning(
    artifacts: &[AgentStatusArtifact],
    stale_candidate: &str,
    newer_input: &str,
    message: &str,
    warnings: &mut Vec<AgentStatusWarning>,
) {
    let Some(candidate) = artifact_by_name(artifacts, stale_candidate).filter(|a| a.present) else {
        return;
    };
    let Some(input) = artifact_by_name(artifacts, newer_input).filter(|a| a.present) else {
        return;
    };
    let (Some(candidate_modified), Some(input_modified)) = (candidate.modified, input.modified)
    else {
        return;
    };
    if candidate_modified.duration_since(input_modified).is_err() {
        warnings.push(AgentStatusWarning {
            kind: "stale_artifact".to_string(),
            artifact: candidate.name.clone(),
            message: message.to_string(),
        });
    }
}

fn missing_commands(
    root_argument: &Path,
    seam: Option<&AgentStatusSeam>,
    artifacts: &[AgentStatusArtifact],
) -> Vec<AgentStatusCommand> {
    let mut commands = Vec::new();
    for artifact_name in MISSING_COMMAND_ORDER {
        let Some(artifact) = artifact_by_name(artifacts, artifact_name) else {
            continue;
        };
        if artifact.present {
            continue;
        }
        commands.push(AgentStatusCommand {
            step: artifact.name.clone(),
            artifact: artifact.path.clone(),
            reason: format!("{} artifact is missing", artifact.label),
            command: command_for_missing_artifact(root_argument, seam, artifact),
        });
    }
    commands
}

fn command_for_missing_artifact(
    root_argument: &Path,
    seam: Option<&AgentStatusSeam>,
    artifact: &AgentStatusArtifact,
) -> String {
    let root = bound_root(&display_path(root_argument));
    let seam_id = seam
        .map(|seam| seam.seam_id.as_str())
        .unwrap_or("<seam-id>");
    match artifact.name.as_str() {
        "before_snapshot" => {
            check_repo_exposure_command(&root, "draft", WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT)
        }
        "after_snapshot" => {
            check_repo_exposure_command(&root, "draft", WORKFLOW_AFTER_SNAPSHOT_ARTIFACT)
        }
        "analysis_outcome" => {
            check_analysis_outcome_command(&root, "draft", WORKFLOW_ANALYSIS_OUTCOME_ARTIFACT)
        }
        "agent_packet" => agent_packet_command(&root, seam_id, WORKFLOW_AGENT_PACKET_ARTIFACT),
        "agent_brief" => agent_brief_command(&root, seam_id, WORKFLOW_AGENT_BRIEF_ARTIFACT),
        "agent_verify" => agent_verify_command(
            &root,
            WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT,
            WORKFLOW_AFTER_SNAPSHOT_ARTIFACT,
            Some(WORKFLOW_AGENT_VERIFY_ARTIFACT),
        ),
        "agent_receipt" => agent_receipt_command(
            &root,
            WORKFLOW_AGENT_VERIFY_ARTIFACT,
            seam_id,
            Some(WORKFLOW_AGENT_RECEIPT_ARTIFACT),
        ),
        _ => String::new(),
    }
}

fn artifact_by_name<'a>(
    artifacts: &'a [AgentStatusArtifact],
    name: &str,
) -> Option<&'a AgentStatusArtifact> {
    artifacts.iter().find(|artifact| artifact.name == name)
}

fn modified_unix_ms(time: Option<SystemTime>) -> Option<u64> {
    let millis = time?.duration_since(UNIX_EPOCH).ok()?.as_millis();
    u64::try_from(millis).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Duration;

    fn unique_agent_status_test_dir(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "ripr-agent-status-{label}-{}-{stamp}",
            std::process::id()
        ))
    }

    fn write_file(path: &Path, text: &str) -> Result<(), String> {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).map_err(|err| format!("create parent: {err}"))?;
        }
        std::fs::write(path, text).map_err(|err| format!("write {}: {err}", path.display()))
    }

    fn artifact(name: &str, present: bool, modified: Option<SystemTime>) -> AgentStatusArtifact {
        AgentStatusArtifact {
            name: name.to_string(),
            label: name.replace('_', " "),
            path: format!("target/ripr/{name}.json"),
            present,
            bytes: Some(1),
            modified,
        }
    }

    /// #4287: `pilot_select_command` binds its root once, so a raw root and an
    /// already bound root render the same command and the pilot directory is
    /// anchored exactly once under the bound root.
    #[test]
    fn pilot_select_command_binds_raw_and_bound_roots_once() {
        let bound = bound_root(".");
        let command = pilot_select_command(&bound);
        assert_eq!(pilot_select_command("."), command);
        assert_eq!(pilot_select_command(&bound_root(&bound)), command);
        assert_eq!(
            command,
            format!(
                "ripr pilot --root {} --out {}",
                shell_arg(&bound),
                shell_arg(&format!("{bound}/target/ripr/pilot"))
            )
        );
        assert_eq!(command.matches("target/ripr/pilot").count(), 1, "{command}");
    }

    #[test]
    fn agent_status_reports_missing_artifacts_and_next_commands() -> Result<(), String> {
        let root = unique_agent_status_test_dir("missing");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;

        let report = build_agent_status_report(&root, Path::new("."));
        let rendered = render_agent_status_json(&report)?;
        let value: Value =
            serde_json::from_str(&rendered).map_err(|err| format!("parse status JSON: {err}"))?;

        assert_eq!(value["schema_version"], AGENT_STATUS_SCHEMA_VERSION);
        assert_eq!(value["status"], "incomplete");
        assert_eq!(value["seam"], Value::Null);
        assert_eq!(value["artifacts"].as_array().map(Vec::len), Some(7));
        // No repair attempt is present, so the legacy artifact loop is the
        // active mode and every workflow artifact is required
        // (docs/LEARNINGS.md, 2026-07-25 false-confidence gates).
        let artifacts = value["artifacts"]
            .as_array()
            .ok_or_else(|| "status JSON must carry an artifacts array".to_string())?;
        for artifact in artifacts {
            assert_eq!(
                artifact["required"], true,
                "legacy loop must require `{}`",
                artifact["name"]
            );
        }
        assert_eq!(value["missing_commands"].as_array().map(Vec::len), Some(7));
        assert_eq!(value["repair_attempts"], serde_json::json!([]));
        // A fresh workspace knows no seam and has no workflow directory, so the
        // next command is the product route that selects a seam, not a
        // redirect into `target/ripr/workflow/` (#3906).
        assert_eq!(value["next_command"]["step"], "select_seam");
        assert_eq!(
            value["next_command"]["command"],
            pilot_select_command(&bound_root("."))
        );

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    /// The artifact `required` flags must claim no more than the active loop
    /// mode enforces (docs/LEARNINGS.md, 2026-07-25 false-confidence gates).
    /// With no repair attempt present the legacy artifact loop is active and
    /// requires every workflow artifact; with a trusted repair attempt
    /// present the attempt authority supersedes the repository-global
    /// projections (docs/REPAIR_ATTEMPT.md, "Durable location" and
    /// "Compatibility outputs"), so none of them is required.
    #[test]
    fn agent_status_artifact_required_flags_follow_the_enforced_loop_mode() -> Result<(), String> {
        let root = unique_agent_status_test_dir("required-flags-repair");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        run_git(&root, &["init"])?;
        run_git(
            &root,
            &["config", "user.email", "ripr-test@example.invalid"],
        )?;
        run_git(&root, &["config", "user.name", "RIPR Test"])?;
        write_file(&root.join("README.md"), "# test\n")?;
        run_git(&root, &["add", "."])?;
        run_git(&root, &["commit", "--no-gpg-sign", "-m", "initial"])?;
        prepare_attempt_fixture(&root, "seam:status-honesty")?;

        let report = build_agent_status_report(&root, &root);
        assert_eq!(
            report.repair_attempts.len(),
            1,
            "the prepared attempt must be the active loop mode"
        );
        let rendered = render_agent_status_json(&report)?;
        let value: Value =
            serde_json::from_str(&rendered).map_err(|err| format!("parse status JSON: {err}"))?;
        let artifacts = value["artifacts"]
            .as_array()
            .ok_or_else(|| "status JSON must carry an artifacts array".to_string())?;
        for artifact in artifacts {
            assert_eq!(
                artifact["required"], false,
                "repair loop must not require the superseded projection `{}`",
                artifact["name"]
            );
        }

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    /// Every artifact status reports on must carry an explicit classification
    /// for the repair-attempt loop mode, so a newly added artifact cannot
    /// silently inherit an all-`true` or all-`false` claim.
    #[test]
    fn agent_status_every_artifact_has_a_loop_mode_classification() {
        let reported = ARTIFACTS
            .iter()
            .map(|artifact| artifact.name)
            .collect::<Vec<_>>();
        assert_eq!(
            REPAIR_ATTEMPT_SUPERSEDED_ARTIFACTS, reported,
            "each reported artifact needs an explicit active-loop classification"
        );
    }

    /// Reading the workflow receipt must distinguish a missing file from a
    /// malformed one: they are different facts, and only the second makes the
    /// receipt's content unreachable.
    #[test]
    fn agent_status_receipt_read_distinguishes_missing_from_malformed() -> Result<(), String> {
        let root = unique_agent_status_test_dir("receipt-read");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;

        assert_eq!(
            read_workflow_receipt(&root),
            WorkflowReceiptRead::Missing,
            "no receipt file must read as missing"
        );

        write_file(&root.join(WORKFLOW_AGENT_RECEIPT_ARTIFACT), "{not json")?;
        assert_eq!(
            read_workflow_receipt(&root),
            WorkflowReceiptRead::Unreadable,
            "a malformed receipt file must read as unreadable, not missing"
        );

        write_file(&root.join(WORKFLOW_AGENT_RECEIPT_ARTIFACT), "{}")?;
        match read_workflow_receipt(&root) {
            WorkflowReceiptRead::Parsed(_) => {}
            other => return Err(format!("a valid receipt must parse, got {other:?}")),
        }

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    /// A read failure that is not simple absence (e.g. the receipt path is a
    /// directory, or permissions deny the read) must read as `Unreadable`,
    /// never as `Missing` — `Missing` claims the receipt was never issued.
    #[test]
    fn agent_status_receipt_read_treats_io_errors_as_unreadable() -> Result<(), String> {
        let root = unique_agent_status_test_dir("receipt-read-io-error");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        // A directory at the receipt path makes read_to_string fail with a
        // non-NotFound error on every supported platform.
        std::fs::create_dir_all(root.join(WORKFLOW_AGENT_RECEIPT_ARTIFACT))
            .map_err(|err| format!("create receipt-dir: {err}"))?;
        assert_eq!(
            read_workflow_receipt(&root),
            WorkflowReceiptRead::Unreadable,
            "an unreadable receipt path must not be reported as missing"
        );
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    /// A `ready_to_finish` attempt whose workflow receipt file is malformed
    /// must report the typed receipt as `Unreadable`, not `NotIssued`: an
    /// orchestrator's next-command decision needs "a receipt exists but
    /// cannot be read", not "no receipt was ever issued".
    #[test]
    fn agent_status_ready_to_finish_attempt_reports_unreadable_receipt() -> Result<(), String> {
        let manifest = ready_to_finish_manifest()?;

        assert_eq!(
            attempt_receipt(&manifest, &WorkflowReceiptRead::Unreadable),
            AgentStatusAttemptReceipt::Unreadable
        );
        assert_eq!(
            attempt_receipt(&manifest, &WorkflowReceiptRead::Missing),
            AgentStatusAttemptReceipt::NotIssued,
            "a missing receipt file stays not issued"
        );
        let unbound = serde_json::json!({});
        assert_eq!(
            attempt_receipt(&manifest, &WorkflowReceiptRead::Parsed(unbound)),
            AgentStatusAttemptReceipt::NotIssued,
            "a readable receipt bound to other work stays not issued"
        );

        // The JSON rendering exposes the distinction machine-readably.
        let unreadable = attempt_receipt_json(&AgentStatusAttemptReceipt::Unreadable);
        assert_eq!(unreadable["unreadable"], true);
        assert_eq!(unreadable["issued_for_attempt"], false);
        let not_issued = attempt_receipt_json(&AgentStatusAttemptReceipt::NotIssued);
        assert_eq!(not_issued["unreadable"], false);
        Ok(())
    }

    fn ready_to_finish_manifest() -> Result<RepairAttemptManifest, String> {
        use crate::app::repair_attempt::{RepairAttemptAfter, RepairAttemptId};
        use crate::edit_cage::{EditCageVerdict, EditCageVerdictStatus};
        let attempt_id = RepairAttemptId::parse("repair-attempt-0123456789abcdef01234567")
            .map_err(|err| format!("parse attempt id: {err}"))?;
        let schema_version = crate::app::repair_attempt::REPAIR_ATTEMPT_SCHEMA_VERSION;
        let head = "0123456789abcdef0123456789abcdef01234567".to_string();
        Ok(RepairAttemptManifest {
            schema_version: schema_version.to_string(),
            kind: "repair_attempt".to_string(),
            repair_attempt_id: attempt_id.clone(),
            state: RepairAttemptState::ReadyToFinish,
            root: ".".to_string(),
            repository_head: head.clone(),
            producer_version: "test".to_string(),
            seam_id: "seam-a".to_string(),
            created_unix_ms: 0,
            artifacts: Vec::new(),
            next_command: "ripr agent repair --root . --seam-id seam-a --phase after".to_string(),
            limitations: Vec::new(),
            non_claims: Vec::new(),
            after: Some(RepairAttemptAfter {
                attempt_id,
                repository_head: head,
                delta_sha256: "sha256:delta".to_string(),
                packet_sha256: "sha256:packet".to_string(),
                current: true,
                verdict: EditCageVerdict {
                    status: EditCageVerdictStatus::Compliant,
                    changed_paths: vec!["tests/target.rs".to_string()],
                    violations: Vec::new(),
                },
            }),
            last_after_refusal: None,
        })
    }

    fn run_git(root: &Path, args: &[&str]) -> Result<(), String> {
        crate::testing::fixture_git::fixture_git_ok(root, args)
    }

    /// Publishes one real repair attempt the way the before phase does, so
    /// status reads a trusted attempt directory rather than a synthetic one.
    fn prepare_attempt_fixture(root: &Path, seam_id: &str) -> Result<(), String> {
        use crate::app::repair_attempt::{
            BeforeArtifactSource, BeginRepairAttemptOptions, begin_repair_attempt_with,
            edit_cage_policy_from_packet, write_edit_cage_baseline,
        };
        let workflow = root.join("target/ripr/workflow");
        std::fs::create_dir_all(&workflow)
            .map_err(|err| format!("create {}: {err}", workflow.display()))?;
        let before = workflow.join("before-status-honesty.json");
        let packet = workflow.join("packet-status-honesty.json");
        let baseline = workflow.join("baseline-status-honesty.json");
        write_file(&before, "{}")?;
        let packet_text = serde_json::json!({
            "seam_id": seam_id,
            "allowed_edit_surface": ["tests/target.rs"],
            "forbidden_files": []
        })
        .to_string();
        write_file(&packet, &packet_text)?;
        let policy = edit_cage_policy_from_packet(&packet_text, seam_id)?;
        write_edit_cage_baseline(root, &baseline, &policy)?;
        begin_repair_attempt_with(BeginRepairAttemptOptions {
            root,
            root_argument: root,
            seam_id,
            sources: &[
                BeforeArtifactSource {
                    role: "before_snapshot",
                    path: &before,
                },
                BeforeArtifactSource {
                    role: "agent_packet",
                    path: &packet,
                },
                BeforeArtifactSource {
                    role: "edit_cage_baseline",
                    path: &baseline,
                },
            ],
            expected_repository_head: None,
            next_command_suffix: None,
        })?;
        Ok(())
    }

    #[test]
    fn agent_status_reports_complete_when_all_artifacts_are_present() -> Result<(), String> {
        let root = unique_agent_status_test_dir("complete");
        write_file(&root.join(WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT), "{}")?;
        write_file(&root.join(WORKFLOW_AFTER_SNAPSHOT_ARTIFACT), "{}")?;
        write_file(&root.join(WORKFLOW_ANALYSIS_OUTCOME_ARTIFACT), "{}")?;
        write_file(&root.join(WORKFLOW_AGENT_BRIEF_ARTIFACT), "{}")?;
        write_file(&root.join(WORKFLOW_AGENT_PACKET_ARTIFACT), "{}")?;
        write_file(
            &root.join(WORKFLOW_AGENT_VERIFY_ARTIFACT),
            r#"{
  "changed_seams": [],
  "unchanged_seams": [{"seam_id": "from-verify"}],
  "new_gaps": [],
  "resolved_gaps": []
}"#,
        )?;
        write_file(
            &root.join(WORKFLOW_AGENT_RECEIPT_ARTIFACT),
            r#"{"seam":{"seam_id":"from-receipt"}}"#,
        )?;

        let report = build_agent_status_report(&root, Path::new("."));
        let rendered = render_agent_status_json(&report)?;
        let value: Value =
            serde_json::from_str(&rendered).map_err(|err| format!("parse status JSON: {err}"))?;

        assert_eq!(value["status"], "complete");
        assert_eq!(value["seam"]["seam_id"], "from-receipt");
        assert_eq!(value["seam"]["source"], "agent_receipt");
        assert_eq!(value["missing_commands"].as_array().map(Vec::len), Some(0));
        assert_eq!(value["next_command"], Value::Null);
        assert_eq!(
            value["artifacts"][0]["state"],
            serde_json::Value::String("present".to_string())
        );
        assert!(value["artifacts"][0]["bytes"].as_u64().is_some());
        assert!(value["artifacts"][0]["modified_unix_ms"].as_u64().is_some());

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_status_markdown_names_next_command_and_limits() -> Result<(), String> {
        let root = unique_agent_status_test_dir("markdown");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;

        let report = build_agent_status_report(&root, Path::new("."));
        let rendered = render_agent_status_markdown(&report);

        assert!(rendered.contains("# RIPR Agent Status"));
        assert!(rendered.contains("Status: incomplete"));
        assert!(rendered.contains("| before snapshot | missing |"));
        assert!(rendered.contains("## Next Command"));
        assert!(rendered.contains(&pilot_select_command(&bound_root("."))));
        assert!(rendered.contains("No runtime mutation execution."));
        assert!(rendered.contains("No generated tests."));

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    /// #3906 (F60-2): a next command that needs a snapshot taken after the
    /// focused test edit carries the post-edit note; any other command does
    /// not. The labeller is checked over every loop step on constructed
    /// commands, so the contract does not depend on which step status routes
    /// a given artifact set to.
    #[test]
    fn agent_status_command_labels_only_post_edit_steps() {
        let command = |step: &str| AgentStatusCommand {
            step: step.to_string(),
            artifact: "target/ripr/workflow/x.json".to_string(),
            reason: format!("{step} is missing"),
            command: format!("ripr {step}"),
        };
        for step in AFTER_TEST_EDIT_STEPS {
            assert!(command(step).runs_after_test_edit(), "{step}");
        }
        // Pre-edit loop steps, and the routes a status that reads repair
        // attempts or pilot may select instead (#3931): none is post-edit.
        for step in [
            "before_snapshot",
            "agent_start",
            "agent_brief",
            "agent_packet",
            "select_seam",
            "repair_attempt_before",
            "repair_attempt_after",
        ] {
            assert!(!command(step).runs_after_test_edit(), "{step}");
        }
    }

    /// #3906 (F60-2): the rendered Next Command places the post-edit note
    /// between the reason and the command when, and only when, that command
    /// runs after the test edit. The before-side fixture names its seam in
    /// the packet and keeps the workflow directory, so the after snapshot is
    /// the next step whether status reads only the artifact loop or also
    /// repair attempts and pilot (#3931); an empty root's next command is a
    /// pre-edit step in both.
    #[test]
    fn agent_status_markdown_labels_post_edit_next_commands() -> Result<(), String> {
        let empty = unique_agent_status_test_dir("markdown-before-side-missing");
        std::fs::create_dir_all(&empty).map_err(|err| format!("create root: {err}"))?;
        let report = build_agent_status_report(&empty, Path::new("."));
        let rendered = render_agent_status_markdown(&report);
        assert!(rendered.contains("## Next Command"), "{rendered}");
        assert!(!rendered.contains(AFTER_TEST_EDIT_NOTE), "{rendered}");
        std::fs::remove_dir_all(&empty).map_err(|err| format!("remove root: {err}"))?;

        let root = unique_agent_status_test_dir("markdown-before-side-present");
        write_file(&root.join(WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT), "{}")?;
        write_file(
            &root.join(WORKFLOW_AGENT_PACKET_ARTIFACT),
            r#"{"packets":[{"seam_id":"seam-a"}]}"#,
        )?;
        write_file(&root.join(WORKFLOW_AGENT_BRIEF_ARTIFACT), "{}")?;
        let report = build_agent_status_report(&root, Path::new("."));
        let steps = report
            .missing_commands
            .iter()
            .map(|command| (command.step.as_str(), command.runs_after_test_edit()))
            .collect::<Vec<_>>();
        assert_eq!(
            steps,
            vec![
                ("after_snapshot", true),
                ("analysis_outcome", true),
                ("agent_verify", true),
                ("agent_receipt", true),
            ]
        );
        let rendered = render_agent_status_markdown(&report);
        let reason = rendered
            .find("after snapshot artifact is missing")
            .ok_or_else(|| format!("next-command reason missing:\n{rendered}"))?;
        let note = rendered
            .find(AFTER_TEST_EDIT_NOTE)
            .ok_or_else(|| format!("post-edit note missing:\n{rendered}"))?;
        let fence = rendered
            .find(&format!(
                "```bash\n{}\n```",
                check_repo_exposure_command(
                    &bound_root("."),
                    "draft",
                    WORKFLOW_AFTER_SNAPSHOT_ARTIFACT
                )
            ))
            .ok_or_else(|| format!("after-snapshot command missing:\n{rendered}"))?;
        assert!(reason < note && note < fence, "{rendered}");

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    /// The agent-status Next Command block must offer both shells (#2628): the
    /// bash fence stays byte-identical, the PowerShell fence derives through
    /// the shared `powershell_command` translation (redirect becomes a UTF-8
    /// .NET write; the quoted root survives as a single-quoted literal), and
    /// the cmd.exe boundary is stated.
    #[test]
    fn agent_status_markdown_next_command_offers_powershell_variant() -> Result<(), String> {
        let root = unique_agent_status_test_dir("markdown-powershell");
        // A known seam and an existing workflow directory keep the legacy
        // redirect route selected, which is the translation this pins.
        write_file(
            &root.join(WORKFLOW_AGENT_PACKET_ARTIFACT),
            r#"{"packets":[{"seam_id":"seam-a"}]}"#,
        )?;

        let report = build_agent_status_report(&root, Path::new("repo root"));
        let rendered = render_agent_status_markdown(&report);

        // Issue #3872: the next-command redirect anchors at the resolved
        // --root, so both presented forms build from the same builder output
        // (the anchor math itself is pinned in loop_commands tests).
        let next = check_repo_exposure_command(
            &bound_root("repo root"),
            "draft",
            WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT,
        );
        let bash_form = format!("```bash\n{next}\n```\n");
        assert!(
            rendered.contains(bash_form.as_str()),
            "bash next command drifted:\n{rendered}"
        );
        let powershell_form = format!(
            "```powershell\n{}\n```\n",
            crate::output::markdown::powershell_command(&next)
                .ok_or_else(|| "redirect commands gain a powershell variant".to_string())?
        );
        assert!(
            rendered.contains(powershell_form.as_str()),
            "powershell next command missing or drifted:\n{rendered}"
        );
        let bash_fence = rendered
            .find(bash_form.as_str())
            .ok_or_else(|| format!("bash fence must exist: {rendered}"))?;
        let powershell_fence = rendered
            .find(powershell_form.as_str())
            .ok_or_else(|| format!("powershell fence must exist: {rendered}"))?;
        assert!(
            bash_fence < powershell_fence,
            "bash form must be presented before the PowerShell variant"
        );
        assert!(
            rendered.contains("cmd.exe is not supported."),
            "next command presentation must state the cmd.exe boundary:\n{rendered}"
        );

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_status_recovers_seam_id_from_receipt() -> Result<(), String> {
        let root = unique_agent_status_test_dir("receipt");
        write_file(
            &root.join(WORKFLOW_AGENT_RECEIPT_ARTIFACT),
            r#"{"seam":{"seam_id":"67fc764ba37d77bd"}}"#,
        )?;

        let report = build_agent_status_report(&root, Path::new("repo root"));
        let seam = report
            .seam
            .as_ref()
            .ok_or_else(|| "expected recovered seam".to_string())?;

        assert_eq!(seam.seam_id, "67fc764ba37d77bd");
        assert_eq!(seam.source, "agent_receipt");
        assert!(report.missing_commands.iter().any(|command| {
            command.step == "agent_packet"
                && command.command
                    == agent_packet_command(
                        &bound_root("repo root"),
                        "67fc764ba37d77bd",
                        WORKFLOW_AGENT_PACKET_ARTIFACT,
                    )
        }));

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_status_warns_for_malformed_present_json() -> Result<(), String> {
        let root = unique_agent_status_test_dir("malformed");
        write_file(&root.join(WORKFLOW_AGENT_RECEIPT_ARTIFACT), "{not json")?;

        let report = build_agent_status_report(&root, Path::new("."));

        assert!(report.seam.is_none());
        assert!(report.warnings.iter().any(|warning| {
            warning.kind == "artifact_json_unreadable" && warning.artifact == "agent_receipt"
        }));

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_status_recovers_seam_id_from_verify_packet_or_brief() {
        let verify: Value = serde_json::json!({
            "changed_seams": [],
            "unchanged_seams": [{"seam_id": "from-verify"}],
            "new_gaps": [],
            "resolved_gaps": []
        });
        let packet: Value = serde_json::json!({
            "packets": [{"seam_id": "from-packet"}]
        });
        let brief: Value = serde_json::json!({
            "working_set": {"seam_id": "from-brief"},
            "top_seams": [{"seam_id": "from-top-seam"}]
        });

        assert_eq!(
            seam_id_from_source(&verify, "agent_verify"),
            Some("from-verify".to_string())
        );
        assert_eq!(
            seam_id_from_source(&packet, "agent_packet"),
            Some("from-packet".to_string())
        );
        assert_eq!(
            seam_id_from_source(&brief, "agent_brief"),
            Some("from-brief".to_string())
        );
    }

    #[test]
    fn agent_status_recovers_brief_top_seam_without_working_set_seam() {
        let brief: Value = serde_json::json!({
            "working_set": {"seam_id": null},
            "top_seams": [{"seam_id": "from-top-seam"}]
        });

        assert_eq!(
            seam_id_from_source(&brief, "agent_brief"),
            Some("from-top-seam".to_string())
        );
    }

    #[test]
    fn agent_status_warns_when_verify_or_receipt_look_stale() {
        let early = UNIX_EPOCH + Duration::from_secs(1);
        let middle = UNIX_EPOCH + Duration::from_secs(2);
        let late = UNIX_EPOCH + Duration::from_secs(3);
        let warnings = stale_warnings(&[
            artifact("before_snapshot", true, Some(late)),
            artifact("after_snapshot", true, Some(late)),
            artifact("agent_verify", true, Some(middle)),
            artifact("agent_receipt", true, Some(early)),
        ]);

        assert_eq!(warnings.len(), 3);
        assert!(warnings.iter().any(|warning| {
            warning.artifact == "agent_verify"
                && warning.message.contains("older than the before snapshot")
        }));
        assert!(warnings.iter().any(|warning| {
            warning.artifact == "agent_receipt"
                && warning.message.contains("older than agent verify")
        }));
    }

    #[test]
    fn agent_status_warns_when_before_snapshot_is_newer_than_after() {
        let after = UNIX_EPOCH + Duration::from_secs(2);
        let before = UNIX_EPOCH + Duration::from_secs(3);
        let completed = UNIX_EPOCH + Duration::from_secs(4);
        let warnings = stale_warnings(&[
            artifact("before_snapshot", true, Some(before)),
            artifact("after_snapshot", true, Some(after)),
            artifact("agent_verify", true, Some(completed)),
            artifact("agent_receipt", true, Some(completed)),
        ]);

        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].artifact, "before_snapshot");
        assert!(
            warnings[0]
                .message
                .contains("before snapshot is newer than after snapshot")
        );
    }

    fn synthetic_attempt(
        id: &str,
        seam: &str,
        disposition: &'static str,
        command: Option<&str>,
    ) -> AgentStatusRepairAttempt {
        AgentStatusRepairAttempt {
            attempt_id: id.to_string(),
            seam_id: seam.to_string(),
            state: "failed",
            head_current: Some(true),
            disposition,
            manifest: format!("{REPAIR_ATTEMPT_DIRECTORY}/{id}/attempt.json"),
            command: command.map(str::to_string),
            evidence_head: "0123456789abcdef0123456789abcdef01234567".to_string(),
            receipt: AgentStatusAttemptReceipt::NotApplicable,
            last_after_refusal: None,
            diverged_recovery: None,
        }
    }

    #[test]
    fn agent_status_refuses_to_choose_between_open_seams() -> Result<(), String> {
        let root = unique_agent_status_test_dir("open-seams");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        let attempts = vec![
            synthetic_attempt("a", "seam-a", "ended", Some("restart a")),
            synthetic_attempt("b", "seam-b", "ended", Some("restart b")),
        ];
        let mut warnings = Vec::new();
        let next = select_next_command(&root, ".", None, Some(&attempts), &[], &mut warnings);
        assert_eq!(next, None);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].kind, "multiple_open_repair_seams");
        assert!(warnings[0].message.contains("--seam-id seam-a"));
        assert!(warnings[0].message.contains("--seam-id seam-b"));
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_status_does_not_restart_a_seam_that_finished() -> Result<(), String> {
        let root = unique_agent_status_test_dir("finished-seam");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        let attempts = vec![
            synthetic_attempt("a", "seam-a", "ended", Some("restart a")),
            synthetic_attempt("b", "seam-a", "finished", None),
            synthetic_attempt("c", "seam-b", "ended", Some("restart c")),
        ];
        let mut warnings = Vec::new();
        let next = select_next_command(&root, ".", None, Some(&attempts), &[], &mut warnings)
            .ok_or_else(|| "expected a restart for the one open seam".to_string())?;
        assert_eq!(next.step, "repair_attempt_before");
        assert_eq!(
            next.command,
            "ripr agent repair --root . --seam-id seam-b --phase before"
        );
        assert!(warnings.is_empty(), "{warnings:?}");
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    /// After `ripr pilot`, status continues with the repair start pilot
    /// recorded rather than sending the user back to pilot. A summary whose
    /// top seam did not pass the flip (`null`) leaves status on `select_seam`.
    #[test]
    fn agent_status_continues_from_the_pilot_repair_start() -> Result<(), String> {
        let root = unique_agent_status_test_dir("after-pilot");
        let command = "ripr agent repair --root . --seam-id 8f7fa8644fd12280 --phase before";
        write_file(
            &root.join(PILOT_SUMMARY_ARTIFACT),
            &serde_json::json!({"next": {"repair_command": command}}).to_string(),
        )?;
        let report = build_agent_status_report(&root, Path::new("."));
        let next = report
            .next_command
            .as_ref()
            .ok_or_else(|| "expected a next command".to_string())?;
        assert_eq!(next.step, "repair_attempt_before");
        assert_eq!(next.command, command);
        assert_eq!(next.artifact, PILOT_SUMMARY_ARTIFACT);

        write_file(
            &root.join(PILOT_SUMMARY_ARTIFACT),
            r#"{"next": {"repair_command": null}}"#,
        )?;
        let report = build_agent_status_report(&root, Path::new("."));
        let next = report
            .next_command
            .as_ref()
            .ok_or_else(|| "expected a next command".to_string())?;
        assert_eq!(next.step, "select_seam");
        assert_eq!(next.command, pilot_select_command(&bound_root(".")));

        // #4216 row 3: a complete pilot run that ranked a top seam and
        // recorded no repair start is terminal for status. Sending the user
        // back to `ripr pilot` would only rank the same seam again.
        write_file(
            &root.join(PILOT_SUMMARY_ARTIFACT),
            r#"{"status": "complete", "top_actionable_seams": [{"seam_id": "601d0f60f676a636"}], "next": {"repair_command": null}}"#,
        )?;
        let report = build_agent_status_report(&root, Path::new("."));
        assert_eq!(report.next_command, None, "{:?}", report.next_command);
        let warning = report
            .warnings
            .iter()
            .find(|warning| warning.kind == "pilot_found_no_repair_target")
            .ok_or_else(|| format!("expected a no-repair-target warning: {:?}", report.warnings))?;
        assert_eq!(warning.artifact, PILOT_SUMMARY_ARTIFACT);
        assert!(
            warning
                .message
                .contains("add a test for it by hand in the crate that owns it"),
            "{}",
            warning.message
        );
        let rendered = render_agent_status_markdown(&report);
        assert!(!rendered.contains("ripr pilot --root"), "{rendered}");

        // A pilot run that timed out is not that fact: rerunning pilot is
        // still the way forward.
        write_file(
            &root.join(PILOT_SUMMARY_ARTIFACT),
            r#"{"status": "timed_out", "top_actionable_seams": [{"seam_id": "601d0f60f676a636"}], "next": {"repair_command": null}}"#,
        )?;
        let report = build_agent_status_report(&root, Path::new("."));
        let next = report
            .next_command
            .as_ref()
            .ok_or_else(|| "expected a next command".to_string())?;
        assert_eq!(next.step, "select_seam");

        // Status repeats the pilot value as its next command, so only a repair
        // start may pass. Any other string, even another ripr command, leaves
        // status on `select_seam`, exactly as `null` does. `ripr pilot` never
        // writes such a value, so no end-to-end run reaches this case.
        write_file(
            &root.join(PILOT_SUMMARY_ARTIFACT),
            r#"{"next": {"repair_command": "ripr check --root ."}}"#,
        )?;
        let report = build_agent_status_report(&root, Path::new("."));
        let next = report
            .next_command
            .as_ref()
            .ok_or_else(|| "expected a next command".to_string())?;
        assert_eq!(next.step, "select_seam");
        assert_eq!(next.command, pilot_select_command(&bound_root(".")));

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    /// #4216 (Python and TypeScript re-walks): a complete pilot run that
    /// ranked no seam, produced no repair card, and routed the code to
    /// `ripr check` is terminal for status, whatever the language. Status
    /// names the hand step and the recorded check command instead of looping
    /// back to pilot; a route not enabled in `[languages]` names the enable
    /// step instead of promising a discriminator. Every other summary still
    /// routes to pilot.
    #[test]
    fn agent_status_stops_when_pilot_routed_changed_code_to_check() -> Result<(), String> {
        let root = unique_agent_status_test_dir("pilot-routed-to-check");
        let enabled_routes = r#"[{"language": "typescript", "enabled": true, "command": "ripr check --root ."}, {"language": "python", "enabled": true, "command": "ripr check --root ."}]"#;
        let summary = |status: &str, state: &str, first_use: &str, routes: &str, repair: &str| {
            format!(
                r#"{{"status": "{status}", "top_actionable_seams": [], {first_use} "language_routes": {{"state": "{state}", "routes": {routes}}}, "next": {{"repair_command": {repair}}}}}"#
            )
        };
        let stopped = |text: &str| -> Result<String, String> {
            write_file(&root.join(PILOT_SUMMARY_ARTIFACT), text)?;
            let report = build_agent_status_report(&root, Path::new("."));
            if report.next_command.is_some() {
                return Err(format!(
                    "expected no next command for {text}: {:?}",
                    report.next_command
                ));
            }
            let warning = report
                .warnings
                .iter()
                .find(|warning| warning.kind == "pilot_routed_to_check_no_repair_target")
                .ok_or_else(|| {
                    format!("expected a routed-to-check warning: {:?}", report.warnings)
                })?;
            assert_eq!(warning.artifact, PILOT_SUMMARY_ARTIFACT);
            let rendered = render_agent_status_markdown(&report);
            assert!(
                !rendered.contains("```bash\nripr pilot --root"),
                "{rendered}"
            );
            assert!(!warning.message.contains("  "), "{}", warning.message);
            Ok(warning.message.clone())
        };

        // Positive: python_first_use null, missing key, or a no-repair-card
        // status; the enabled routes name the check command and hand step.
        for first_use in [
            r#""python_first_use": null,"#,
            "",
            r#""python_first_use": {"status": "no_repair_cards", "repair_cards_total": 0},"#,
            r#""python_first_use": {"status": "no_python_findings", "repair_cards_total": 0},"#,
        ] {
            let message = stopped(&summary(
                "complete",
                "required",
                first_use,
                enabled_routes,
                "null",
            ))?;
            // The carried route keeps the pilot artifact's text; the rerun
            // hint status generates binds the selected root (#4000).
            for expected in [
                "routed the typescript, python code to `ripr check --root .`".to_string(),
                "add or strengthen a test for the changed behavior by hand, then rerun `ripr check --root .`".to_string(),
                format!(
                    "If the workspace changed since that run, rerun `{}`",
                    pilot_select_command(&bound_root("."))
                ),
            ] {
                assert!(message.contains(&expected), "{message}");
            }
        }

        // A route not enabled in `[languages]` names the enable step and
        // promises no discriminator.
        let message = stopped(&summary(
            "complete",
            "required",
            "",
            r#"[{"language": "javascript", "enabled": false, "command": "ripr check --root ."}]"#,
            "null",
        ))?;
        assert!(
            message.contains("the typescript code is not enabled in ripr.toml [languages]"),
            "{message}"
        );
        assert!(
            message.contains(r#"add "typescript" to `[languages] enabled` in ripr.toml"#),
            "{message}"
        );
        assert!(!message.contains("discriminator"), "{message}");

        // A route without a language renders without a double space.
        let message = stopped(&summary(
            "complete",
            "required",
            "",
            r#"[{"enabled": true, "command": "ripr check --root ."}]"#,
            "null",
        ))?;
        assert!(
            message.contains("routed the code to `ripr check --root .`"),
            "{message}"
        );

        // Controls: every other summary keeps status on `select_seam`.
        for control in [
            summary("timed_out", "required", "", enabled_routes, "null"),
            summary("complete", "supplementary", "", enabled_routes, "null"),
            summary("complete", "not_detected", "", enabled_routes, "null"),
            summary(
                "complete",
                "required",
                r#""python_first_use": {"status": "ready", "repair_cards_total": 1},"#,
                enabled_routes,
                "null",
            ),
            summary(
                "complete",
                "required",
                r#""python_first_use": {"status": "analysis_unavailable", "repair_cards_total": 0},"#,
                enabled_routes,
                "null",
            ),
            summary(
                "complete",
                "required",
                "",
                enabled_routes,
                r#""ripr check --root .""#,
            ),
            summary(
                "complete",
                "required",
                "",
                r#"[{"language": "perl", "enabled": false, "command": null}]"#,
                "null",
            ),
            "{ not json".to_string(),
        ] {
            write_file(&root.join(PILOT_SUMMARY_ARTIFACT), &control)?;
            let report = build_agent_status_report(&root, Path::new("."));
            let next = report
                .next_command
                .as_ref()
                .ok_or_else(|| format!("expected pilot for control {control}"))?;
            assert_eq!(next.step, "select_seam", "{control}");
            assert_eq!(
                next.command,
                pilot_select_command(&bound_root(".")),
                "{control}"
            );
        }
        std::fs::remove_file(root.join(PILOT_SUMMARY_ARTIFACT))
            .map_err(|err| format!("remove summary: {err}"))?;
        let report = build_agent_status_report(&root, Path::new("."));
        assert_eq!(
            report.next_command.as_ref().map(|next| next.step.as_str()),
            Some("select_seam")
        );

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    /// A seam known only from the receipt, with no workflow directory: the
    /// first missing artifact would be a redirect into that missing directory,
    /// so status starts a repair attempt instead.
    #[test]
    fn agent_status_never_redirects_into_a_missing_workflow_directory() -> Result<(), String> {
        let root = unique_agent_status_test_dir("receipt-only");
        write_file(
            &root.join(WORKFLOW_AGENT_RECEIPT_ARTIFACT),
            r#"{"seam":{"seam_id":"from-receipt"}}"#,
        )?;

        let report = build_agent_status_report(&root, Path::new("repo root"));
        let next = report
            .next_command
            .as_ref()
            .ok_or_else(|| "expected a next command".to_string())?;
        assert_eq!(report.missing_commands[0].step, "before_snapshot");
        assert_eq!(next.step, "repair_attempt_before");
        assert_eq!(
            next.command,
            format!(
                "ripr agent repair --root {} --seam-id from-receipt --phase before",
                shell_arg(&bound_root("repo root"))
            )
        );
        assert!(!next.command.contains('>'));

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_status_emits_all_missing_command_templates() {
        let artifacts = ARTIFACTS
            .iter()
            .map(|def| AgentStatusArtifact {
                name: def.name.to_string(),
                label: def.label.to_string(),
                path: def.path.to_string(),
                present: false,
                bytes: None,
                modified: None,
            })
            .collect::<Vec<_>>();
        let seam = AgentStatusSeam {
            seam_id: "seam-a".to_string(),
            source: "agent_verify".to_string(),
        };
        let commands = missing_commands(Path::new("."), Some(&seam), &artifacts);

        assert_eq!(commands.len(), 7);
        assert!(commands.iter().any(|command| {
            command.step == "after_snapshot"
                && command.command
                    == check_repo_exposure_command(
                        &bound_root("."),
                        "draft",
                        WORKFLOW_AFTER_SNAPSHOT_ARTIFACT,
                    )
        }));
        assert!(commands.iter().any(|command| {
            command.step == "analysis_outcome"
                && command.command
                    == check_analysis_outcome_command(
                        &bound_root("."),
                        "draft",
                        WORKFLOW_ANALYSIS_OUTCOME_ARTIFACT,
                    )
        }));
        assert!(commands.iter().any(|command| {
            command.step == "agent_brief"
                && command.command
                    == agent_brief_command(
                        &bound_root("."),
                        "seam-a",
                        WORKFLOW_AGENT_BRIEF_ARTIFACT,
                    )
        }));
        assert!(commands.iter().any(|command| {
            command.step == "agent_verify"
                && command.command
                    == agent_verify_command(
                        &bound_root("."),
                        WORKFLOW_BEFORE_SNAPSHOT_ARTIFACT,
                        WORKFLOW_AFTER_SNAPSHOT_ARTIFACT,
                        Some(WORKFLOW_AGENT_VERIFY_ARTIFACT),
                    )
        }));
        assert!(commands.iter().any(|command| {
            command.step == "agent_receipt"
                && command.command
                    == format!(
                        "ripr agent receipt --root {} --verify-json target/ripr/workflow/agent-verify.json --seam-id seam-a --json --out target/ripr/reports/agent-receipt.json",
                        shell_arg(&bound_root("."))
                    )
        }));
    }

    #[test]
    fn agent_status_quotes_paths_with_spaces() {
        // Issue #3872: the redirect anchors at the resolved root, so the
        // quoted expectation names the anchored absolute target: the quoting
        // under test is the single-quote shell encoding around both values.
        let command = agent_packet_command("repo root", "seam-a", WORKFLOW_AGENT_PACKET_ARTIFACT);
        assert!(
            command.starts_with("ripr agent packet --root 'repo root' --seam-id seam-a --json > '"),
            "root and target must stay single-quoted: {command}"
        );
        assert!(
            command.ends_with("/repo root/target/ripr/workflow/agent-packet.json'"),
            "redirect must anchor under the resolved root: {command}"
        );
    }
}
