//! Typed workflow catalog and bounded `help workflow` discovery.
//!
//! RIPR-SPEC-0189 / issue #4824 (command-discovery C3). The C1 catalog
//! ([`crate::cli::command_catalog`]) owns command identity and the C2 metadata
//! table ([`crate::cli::command_metadata`]) owns per-command cost, side
//! effects, and artifacts; this module owns workflow identity: ordered command
//! relationships, result families, stop/recovery boundaries, and the human
//! workflow renderer. Workflows summarize command facts from the C2 table and
//! can never override or strengthen them (required control 5 enforces this at
//! catalog-check time).
//!
//! Out of scope for this slice: `help --json` (RIPR-SPEC-0189 / #4825), command
//! execution, and any change to underlying command behavior. The repair
//! workflow consumes the durable-attempt contract; it never re-implements the
//! #2927 lifecycle and never implies ripr performs the user edit, runs tests
//! without explicit verify authorization, or closes an attempt.

use crate::cli::command_catalog::{CommandCatalogEntry, CommandClass, catalog};
use crate::cli::command_metadata::{
    CommandCost, CommandMetadata, CommandOperation, WORKFLOW_TAGS, metadata, metadata_for,
};

/// One typed workflow row. Every fact is static data; nothing here inspects a
/// repository, so the JSON-discovery child (#4825) can serialize these rows
/// without parsing Markdown.
#[derive(Clone, Debug)]
pub(crate) struct WorkflowCatalogEntry {
    /// Stable kebab-case identity, e.g. `"inspect-change"`.
    pub(crate) id: &'static str,
    /// Alternate spellings accepted by `help workflow <name>`; unique across
    /// the catalog and never equal to another workflow's id.
    pub(crate) aliases: &'static [&'static str],
    /// The [`WORKFLOW_TAGS`] value this workflow promotes into typed identity.
    /// Claimed by exactly one workflow and must be used by at least one
    /// described command row, so a workflow cannot exist without members.
    pub(crate) command_tag: &'static str,
    /// One-line purpose.
    pub(crate) purpose: &'static str,
    /// One-line applicability statement.
    pub(crate) applicability: &'static str,
    /// Required inputs and prerequisites a reader must have before step one.
    pub(crate) prerequisites: &'static [&'static str],
    /// The ordinary public entry command; must equal `steps[0].command`.
    pub(crate) first_command: &'static str,
    /// Ordered required command roles. Order is positional and rendered as
    /// numbered steps; the catalog check proves the role graph is connected
    /// and acyclic.
    pub(crate) steps: &'static [WorkflowStep],
    /// Optional command roles; rendered unnumbered after the required steps.
    pub(crate) optional_steps: &'static [WorkflowStep],
    /// Expected result/state families and the typed next route for each.
    pub(crate) result_families: &'static [WorkflowResultFamily],
    /// Artifact families the workflow as a whole reads.
    pub(crate) artifacts_read: &'static [&'static str],
    /// Artifact families the workflow as a whole writes.
    pub(crate) artifacts_written: &'static [&'static str],
    /// Recovery routes from named failure or refusal states.
    pub(crate) recovery: &'static [WorkflowRecoveryRoute],
    /// Explicit stop or terminal states.
    pub(crate) stop_conditions: &'static [&'static str],
    /// Advanced/control command alternatives (C1 `Advanced` class only).
    pub(crate) advanced_alternatives: &'static [&'static str],
    /// Advisory limitations and non-claims; never empty.
    pub(crate) limitations: &'static str,
}

/// A required or optional command role. Cost, operation, side-effect flags,
/// and artifact claims mirror the C2 command metadata row and are validated to
/// equal it, so a command-side change that contradicts this workflow fails the
/// catalog check visibly (issue #4824 required control 5).
#[derive(Clone, Copy, Debug)]
pub(crate) struct WorkflowStep {
    /// C1/C2 command identity, e.g. `"cmd:check"`.
    pub(crate) command: &'static str,
    /// What this step does inside this workflow; one line.
    pub(crate) role: &'static str,
    /// Mirror of the command row's cost class.
    pub(crate) cost: CommandCost,
    /// Mirror of the command row's operation class.
    pub(crate) operation: CommandOperation,
    /// Mirror of the command row's `may_compile` flag.
    pub(crate) may_compile: bool,
    /// Mirror of the command row's `may_run_tests` flag.
    pub(crate) may_run_tests: bool,
    /// Mirror of the command row's `may_start_child_process` flag.
    pub(crate) may_start_child_process: bool,
    /// Artifact claims; each must appear in the command row's declared output
    /// roles (default or optional) as a substring.
    pub(crate) writes: &'static [&'static str],
}

/// One expected result/state family and its typed next route. Every named
/// family routes onward, stops, or names an explicit limitation (required
/// control 4), and names the step command that produces it, so removing one
/// family cannot silently reassign later outcomes to earlier steps.
#[derive(Clone, Copy, Debug)]
pub(crate) struct WorkflowResultFamily {
    /// The producing step command; must be one of this workflow's required or
    /// optional steps.
    pub(crate) from: &'static str,
    pub(crate) family: &'static str,
    pub(crate) next: WorkflowNext,
}

/// The next route for one result family.
#[derive(Clone, Copy, Debug)]
pub(crate) enum WorkflowNext {
    /// Route to a registered public/compatibility command (C1 identity).
    Command(&'static str),
    /// Terminal stop; the string is the stop statement.
    Terminal(&'static str),
    /// Explicit limitation; the string is the limitation statement.
    Limitation(&'static str),
}

/// A recovery route from a named state back to a registered command.
#[derive(Clone, Copy, Debug)]
pub(crate) struct WorkflowRecoveryRoute {
    pub(crate) when: &'static str,
    pub(crate) route: &'static str,
}

const fn step(
    command: &'static str,
    role: &'static str,
    cost: CommandCost,
    operation: CommandOperation,
    may_start_child_process: bool,
    writes: &'static [&'static str],
) -> WorkflowStep {
    WorkflowStep {
        command,
        role,
        cost,
        operation,
        may_compile: false,
        may_run_tests: false,
        may_start_child_process,
        writes,
    }
}

/// The governed workflow table. One row per reviewed workflow; new workflows
/// need explicit command/task authority in the claim that adds them (issue
/// #4824 initial denominator).
const WORKFLOWS: &[WorkflowCatalogEntry] = &[
    WorkflowCatalogEntry {
        id: "inspect-change",
        aliases: &["review-change"],
        command_tag: "inspect-change",
        purpose: "Trace one change from diff to a named, actionable test gap.",
        applicability: "A local edit or committed diff needs inspection before review or repair.",
        prerequisites: &[
            "a git repository with a resolvable diff (--base, --worktree, or --diff PATH)",
        ],
        first_command: "cmd:check",
        steps: &[
            step(
                "cmd:check",
                "name the top actionable gap for the current diff",
                CommandCost::Analysis,
                CommandOperation::WritesArtifacts,
                true,
                &["--write-artifact PATH reusable findings artifact"],
            ),
            step(
                "cmd:explain",
                "explain one finding against the same diff",
                CommandCost::Analysis,
                CommandOperation::ReadOnly,
                true,
                &[],
            ),
            step(
                "cmd:context",
                "collect the tests and seams related to the finding",
                CommandCost::Analysis,
                CommandOperation::ReadOnly,
                true,
                &[],
            ),
        ],
        optional_steps: &[
            step(
                "cmd:diff",
                "deliver the raw diff-scoped JSON report when a machine-readable view helps",
                CommandCost::Analysis,
                CommandOperation::ReadOnly,
                true,
                &[],
            ),
            step(
                "cmd:pilot",
                "select the seam ID for the repair handoff from the pilot packet's top actionable seams; a probe: finding ID is not a seam ID",
                CommandCost::Workspace,
                CommandOperation::WritesArtifacts,
                true,
                &["target/ripr/pilot packet (--out overrides)"],
            ),
        ],
        result_families: &[
            WorkflowResultFamily {
                from: "cmd:check",
                family: "top actionable gap named",
                next: WorkflowNext::Command("cmd:explain"),
            },
            WorkflowResultFamily {
                from: "cmd:explain",
                family: "finding explained",
                next: WorkflowNext::Command("cmd:context"),
            },
            WorkflowResultFamily {
                from: "cmd:context",
                family: "handoff context collected",
                next: WorkflowNext::Command("cmd:pilot"),
            },
            WorkflowResultFamily {
                from: "cmd:pilot",
                family: "seam selected for repair",
                next: WorkflowNext::Command("cmd:agent.repair"),
            },
            WorkflowResultFamily {
                from: "cmd:check",
                family: "no actionable gap",
                next: WorkflowNext::Terminal(
                    "stop: the diff is already covered; there is nothing to route onward",
                ),
            },
        ],
        artifacts_read: &["the current git diff"],
        artifacts_written: &[
            "stdout findings (redirect to target/ripr/reports/ when a file is needed)",
            "--write-artifact PATH reusable findings artifact",
            "target/ripr/pilot packet (--out overrides) with the optional pilot step",
        ],
        recovery: &[
            WorkflowRecoveryRoute {
                when: "no diff can be resolved",
                route: "cmd:doctor",
            },
            WorkflowRecoveryRoute {
                when: "the finding id does not resolve against the diff",
                route: "cmd:check",
            },
        ],
        stop_conditions: &[
            "explicit refusal when no diff can be resolved",
            "context collection reaches its --max-related-tests bound",
            "agent repair refuses a probe: finding ID; re-select a seam ID from the pilot packet",
        ],
        advanced_alternatives: &[],
        limitations: "guidance only: these commands analyze static evidence; ripr never compiles, runs tests, or edits source from this workflow.",
    },
    WorkflowCatalogEntry {
        id: "guided-adoption",
        aliases: &["adoption"],
        command_tag: "adoption",
        purpose: "Bring one repository to its first trustworthy ripr read.",
        applicability: "A new workspace needs setup, a first gap read, and an evidence habit.",
        prerequisites: &[
            "the selected repository root",
            "a writable target/ directory for reports",
        ],
        first_command: "cmd:doctor",
        steps: &[
            step(
                "cmd:doctor",
                "probe the installed tool versions and environment readiness",
                CommandCost::Small,
                CommandOperation::ReadOnly,
                true,
                &[],
            ),
            step(
                "cmd:pilot",
                "find the top test gap and write an actionable pilot packet",
                CommandCost::Workspace,
                CommandOperation::WritesArtifacts,
                true,
                &["target/ripr/pilot packet (--out overrides)"],
            ),
            step(
                "cmd:check",
                "confirm the pilot's gap against the current diff",
                CommandCost::Analysis,
                CommandOperation::WritesArtifacts,
                true,
                &["--write-artifact PATH reusable findings artifact"],
            ),
        ],
        optional_steps: &[step(
            "cmd:first-pr",
            "compose the start-here adoption packet for the repository",
            CommandCost::Small,
            CommandOperation::WritesArtifacts,
            false,
            &["target/ripr/reports/start-here.json"],
        )],
        result_families: &[
            WorkflowResultFamily {
                from: "cmd:doctor",
                family: "environment ready",
                next: WorkflowNext::Command("cmd:pilot"),
            },
            WorkflowResultFamily {
                from: "cmd:pilot",
                family: "pilot packet written",
                next: WorkflowNext::Command("cmd:check"),
            },
            WorkflowResultFamily {
                from: "cmd:check",
                family: "gap confirmed against the diff",
                next: WorkflowNext::Command("cmd:first-pr"),
            },
            WorkflowResultFamily {
                from: "cmd:first-pr",
                family: "adoption packet composed",
                next: WorkflowNext::Terminal("stop: follow the packet's one next action"),
            },
        ],
        artifacts_read: &[
            "installed tool versions",
            "the current git diff or workspace",
        ],
        artifacts_written: &[
            "target/ripr/pilot packet (--out overrides)",
            "target/ripr/reports/start-here.json with first-pr",
        ],
        recovery: &[
            WorkflowRecoveryRoute {
                when: "a tool is missing or outdated",
                route: "cmd:doctor",
            },
            WorkflowRecoveryRoute {
                when: "the pilot budget is exhausted",
                route: "cmd:check",
            },
        ],
        stop_conditions: &[
            "the pilot writes a partial summary at its budget rather than exceeding it",
            "explicit refusal on invalid configuration",
        ],
        advanced_alternatives: &[],
        limitations: "guidance only: the pilot is bounded by --timeout-ms and points to one next action; ripr never runs mutation testing or repairs a gap from this workflow.",
    },
    WorkflowCatalogEntry {
        id: "repair-gap",
        aliases: &["fix-gap", "repair"],
        command_tag: "repair-loop",
        purpose: "Run the bounded before/edit/after repair transaction for one named seam.",
        applicability: "One selected seam has an authorized edit and verify route.",
        prerequisites: &[
            "--seam-id or --attempt identity",
            "the authorized verify route for the after phase (--verify-authorized)",
            "the edit itself stays with the user or agent outside ripr",
        ],
        first_command: "cmd:agent.repair",
        steps: &[step(
            "cmd:agent.repair",
            "the ordinary public route: before snapshot, edit, after snapshot, verify composition, receipt, status",
            CommandCost::Analysis,
            CommandOperation::WritesArtifacts,
            true,
            &["target/ripr/workflow/"],
        )],
        optional_steps: &[
            step(
                "cmd:rerun",
                "re-evaluate the static evidence for one edited test around the repair",
                CommandCost::Analysis,
                CommandOperation::WritesArtifacts,
                true,
                &["--out PATH targeted-rerun report JSON"],
            ),
            step(
                "cmd:agent.status",
                "read the local repair-loop state for one workspace",
                CommandCost::Small,
                CommandOperation::ReadOnly,
                false,
                &[],
            ),
        ],
        result_families: &[
            WorkflowResultFamily {
                from: "cmd:agent.repair",
                family: "before phase published",
                next: WorkflowNext::Terminal(
                    "stop: perform the authorized edit, then run the after phase",
                ),
            },
            WorkflowResultFamily {
                from: "cmd:agent.repair",
                family: "after phase recorded",
                next: WorkflowNext::Terminal(
                    "stop: read the attempt status and the issued receipt",
                ),
            },
            WorkflowResultFamily {
                from: "cmd:agent.repair",
                family: "verify refused",
                next: WorkflowNext::Limitation(
                    "the refusal without --verify-authorized and a matching authority is recorded on the attempt",
                ),
            },
            WorkflowResultFamily {
                from: "cmd:rerun",
                family: "static evidence re-evaluated",
                next: WorkflowNext::Terminal(
                    "stop: read the updated rerun report before deciding the next phase",
                ),
            },
        ],
        artifacts_read: &[
            "the workspace and current diff",
            "saved before and after snapshots",
        ],
        artifacts_written: &["target/ripr/workflow/ before and after snapshots"],
        recovery: &[
            WorkflowRecoveryRoute {
                when: "the verify phase refuses",
                route: "cmd:agent.status",
            },
            WorkflowRecoveryRoute {
                when: "static movement needs re-evaluation after an edit",
                route: "cmd:rerun",
            },
        ],
        stop_conditions: &[
            "--phase verify refuses without --verify-authorized and a matching authority",
            "a refused after phase is recorded on the attempt instead of repeating the command",
        ],
        advanced_alternatives: &[
            "cmd:agent.start",
            "cmd:agent.brief",
            "cmd:agent.packet",
            "cmd:agent.verify",
            "cmd:agent.verify-execute",
            "cmd:agent.receipt",
            "cmd:agent.review-summary",
        ],
        limitations: "ripr never performs the edit and never closes an attempt because a command was shown; the verify phase runs the authorized test command only for a trust-bound attempt with explicit --verify-authorized and a matching authority, and refuses otherwise. Durable attempt creation, continuation, and status follow the repair-attempt contract; static movement, verification, receipt issuance, and external edit authority stay separate.",
    },
    WorkflowCatalogEntry {
        id: "compose-pr-evidence",
        aliases: &["evidence-compose"],
        command_tag: "pr-evidence",
        purpose: "Compose the movement receipt set that evidences one PR's test-gap work.",
        applicability: "A PR needs its before/after exposure and movement receipt prepared for review.",
        prerequisites: &[
            "a git repository with the PR diff range",
            "two preserved repo-exposure snapshots, one before and one after the change",
            "a writable target/ripr/ directory for reports",
        ],
        first_command: "cmd:check",
        steps: &[
            step(
                "cmd:check",
                "save the before exposure snapshot with ripr check --format repo-exposure-json > before.repo-exposure.json; after the edit, repeat under a second path (re-running overwrites in place)",
                CommandCost::Analysis,
                CommandOperation::WritesArtifacts,
                true,
                &[],
            ),
            step(
                "cmd:outcome",
                "render the movement receipt between the two preserved snapshots (--before before.repo-exposure.json --after after.repo-exposure.json)",
                CommandCost::Small,
                CommandOperation::WritesArtifacts,
                false,
                &["a rendered receipt file with --out PATH"],
            ),
            step(
                "cmd:pr-summary",
                "compose the PR evidence summary over the saved artifacts",
                CommandCost::Small,
                CommandOperation::WritesArtifacts,
                false,
                &["target/ripr/reports/pr-evidence-summary.json"],
            ),
            step(
                "cmd:pr-ledger.record",
                "append the durable PR evidence ledger record",
                CommandCost::Small,
                CommandOperation::StateChanging,
                false,
                &["target/ripr/reports/pr-evidence-ledger.json"],
            ),
        ],
        optional_steps: &[
            step(
                "cmd:pr-evidence",
                "compose the PR packet wrapper for the PR body; its repo-exposure.json is a wrapper, not an outcome --before/--after input",
                CommandCost::Analysis,
                CommandOperation::WritesArtifacts,
                true,
                &["target/ripr/pr/repo-exposure.json"],
            ),
            step(
                "cmd:review-comments",
                "draft advisory review comments from saved artifacts; drafts are never posted",
                CommandCost::Analysis,
                CommandOperation::WritesArtifacts,
                true,
                &["target/ripr/review/comments.json"],
            ),
        ],
        result_families: &[
            WorkflowResultFamily {
                from: "cmd:check",
                family: "before and after snapshots saved",
                next: WorkflowNext::Command("cmd:outcome"),
            },
            WorkflowResultFamily {
                from: "cmd:outcome",
                family: "movement receipt rendered",
                next: WorkflowNext::Command("cmd:pr-summary"),
            },
            WorkflowResultFamily {
                from: "cmd:pr-summary",
                family: "summary composed",
                next: WorkflowNext::Command("cmd:pr-ledger.record"),
            },
            WorkflowResultFamily {
                from: "cmd:pr-ledger.record",
                family: "ledger record appended",
                next: WorkflowNext::Terminal("stop: attach the reports to the PR"),
            },
            WorkflowResultFamily {
                from: "cmd:pr-evidence",
                family: "PR packet composed",
                next: WorkflowNext::Terminal("stop: paste the packet into the PR body"),
            },
        ],
        artifacts_read: &["the PR diff range", "two preserved repo-exposure snapshots"],
        artifacts_written: &[
            "a rendered receipt file with --out PATH",
            "target/ripr/reports/pr-evidence-summary.json",
            "target/ripr/reports/pr-evidence-ledger.json",
            "target/ripr/pr/repo-exposure.json with the optional pr-evidence step",
        ],
        recovery: &[
            WorkflowRecoveryRoute {
                when: "outcome refuses a snapshot missing the seams or findings array",
                route: "cmd:check",
            },
            WorkflowRecoveryRoute {
                when: "advisory review comment drafts are needed",
                route: "cmd:review-comments",
            },
        ],
        stop_conditions: &[
            "explicit refusal on malformed inputs",
            "outcome refuses snapshots that are not raw repo-exposure or check-output JSON; the pr-evidence packet wrapper is not an outcome input",
            "the ledger append is durable only when --out-jsonl is explicit",
        ],
        advanced_alternatives: &[],
        limitations: "projections only: no command posts review comments, reruns analysis, or changes gate authority from this workflow; the outcome step consumes preserved raw snapshots, not the pr-evidence wrapper packet.",
    },
    WorkflowCatalogEntry {
        id: "adopt-ci",
        aliases: &["ci-adoption"],
        command_tag: "setup",
        purpose: "Install ripr's advisory, non-blocking CI configuration and check its policy posture.",
        applicability: "A repository wants advisory ripr checks in CI without merge blocking.",
        prerequisites: &[
            "the selected repository root",
            "a GitHub Actions workflow set the installer may extend",
        ],
        first_command: "cmd:init",
        steps: &[
            step(
                "cmd:init",
                "install ripr.toml and, with --ci github, the advisory workflow",
                CommandCost::Small,
                CommandOperation::StateChanging,
                false,
                &["ripr.toml", ".github/workflows/ripr.yml with --ci github"],
            ),
            step(
                "cmd:config.validate",
                "check the installed configuration before the first analysis",
                CommandCost::Small,
                CommandOperation::ReadOnly,
                false,
                &[],
            ),
            step(
                "cmd:doctor",
                "probe tool readiness for the CI environment",
                CommandCost::Small,
                CommandOperation::ReadOnly,
                true,
                &[],
            ),
            step(
                "cmd:policy.readiness",
                "render the advisory policy-readiness view over saved artifacts",
                CommandCost::Small,
                CommandOperation::WritesArtifacts,
                false,
                &["target/ripr/reports/policy-readiness.json"],
            ),
        ],
        optional_steps: &[
            step(
                "cmd:check",
                "run the first diff-scoped analysis the CI gate will wrap",
                CommandCost::Analysis,
                CommandOperation::WritesArtifacts,
                true,
                &["--write-artifact PATH reusable findings artifact"],
            ),
            step(
                "cmd:policy.history",
                "append the policy history ledger when --out-jsonl names it",
                CommandCost::Small,
                CommandOperation::StateChanging,
                false,
                &["--out-jsonl PATH append-only JSONL history"],
            ),
        ],
        result_families: &[
            WorkflowResultFamily {
                from: "cmd:init",
                family: "configuration installed",
                next: WorkflowNext::Command("cmd:config.validate"),
            },
            WorkflowResultFamily {
                from: "cmd:config.validate",
                family: "configuration valid",
                next: WorkflowNext::Command("cmd:doctor"),
            },
            WorkflowResultFamily {
                from: "cmd:doctor",
                family: "environment ready",
                next: WorkflowNext::Command("cmd:policy.readiness"),
            },
            WorkflowResultFamily {
                from: "cmd:policy.readiness",
                family: "readiness rendered",
                next: WorkflowNext::Terminal(
                    "stop: review the advisory report; the workflow stays non-blocking",
                ),
            },
        ],
        artifacts_read: &["the selected repository root"],
        artifacts_written: &[
            "ripr.toml",
            ".github/workflows/ripr.yml with --ci github",
            "target/ripr/reports/policy-readiness.json",
        ],
        recovery: &[
            WorkflowRecoveryRoute {
                when: "init refuses because a target exists",
                route: "cmd:init",
            },
            WorkflowRecoveryRoute {
                when: "the installed configuration is invalid",
                route: "cmd:config.validate",
            },
        ],
        stop_conditions: &[
            "explicit refusal",
            "the existing workflow is kept when writing cannot finish",
            "the readiness projection stays advisory and never blocks merges",
        ],
        advanced_alternatives: &[],
        limitations: "advisory only: ripr writes configuration and a workflow for review; it never edits source, gates merges by itself, or contacts the network.",
    },
];

/// The governed workflow catalog accessor, integrity-checked like the C1 and
/// C2 tables. The check runs only under debug assertions: release builds pay
/// nothing for the static table.
pub(crate) fn workflow_catalog() -> &'static [WorkflowCatalogEntry] {
    #[cfg(debug_assertions)]
    {
        let violations = workflow_catalog_violations(catalog(), metadata(), WORKFLOWS);
        debug_assert!(
            violations.is_empty(),
            "workflow catalog integrity failed: {violations:?}"
        );
    }
    WORKFLOWS
}

/// Look up one workflow by id or alias.
pub(crate) fn workflow_for(name: &str) -> Option<&'static WorkflowCatalogEntry> {
    WORKFLOWS
        .iter()
        .find(|row| row.id == name || row.aliases.contains(&name))
}

/// Fail-closed workflow catalog violations (issue #4824 required controls).
pub(crate) fn workflow_catalog_violations(
    entries: &[CommandCatalogEntry],
    command_rows: &[CommandMetadata],
    workflows: &[WorkflowCatalogEntry],
) -> Vec<String> {
    let mut violations = Vec::new();
    let mut ids = std::collections::BTreeSet::<&str>::new();
    let mut aliases = std::collections::BTreeSet::<&str>::new();
    let mut claimed_tags = std::collections::BTreeSet::<&str>::new();
    // Every canonical id is known up front, so an alias that collides with a
    // *later* row's id is rejected too, not only earlier ones.
    let all_ids: std::collections::BTreeSet<&str> = workflows.iter().map(|row| row.id).collect();

    for row in workflows {
        let label = row.id;
        if !is_kebab_case(row.id) {
            violations.push(format!("workflow {label:?} id is not stable kebab-case"));
        }
        if !ids.insert(row.id) {
            violations.push(format!("duplicate workflow identity {label:?}"));
        }
        for alias in row.aliases {
            if !is_kebab_case(alias) {
                violations.push(format!(
                    "workflow {label:?} alias {alias:?} is not kebab-case"
                ));
            }
            if *alias == row.id || all_ids.contains(alias) || !aliases.insert(alias) {
                violations.push(format!(
                    "workflow {label:?} alias {alias:?} collides with another identity"
                ));
            }
        }

        // Required control: a workflow promotes a real WORKFLOW_TAGS value,
        // claimed once, with at least one described command member.
        if !WORKFLOW_TAGS.contains(&row.command_tag) {
            violations.push(format!(
                "workflow {label:?} command tag {:?} is not in WORKFLOW_TAGS",
                row.command_tag
            ));
        } else if !claimed_tags.insert(row.command_tag) {
            violations.push(format!(
                "workflow {label:?} re-claims command tag {:?}",
                row.command_tag
            ));
        }
        if !command_rows
            .iter()
            .any(|command_row| command_row.workflows.contains(&row.command_tag))
        {
            violations.push(format!(
                "workflow {label:?} promotes tag {:?} that no described command uses",
                row.command_tag
            ));
        }

        if row.purpose.trim().is_empty() || row.purpose.lines().count() > 1 {
            violations.push(format!("workflow {label:?} purpose is empty or multi-line"));
        }
        if row.applicability.trim().is_empty() || row.applicability.lines().count() > 1 {
            violations.push(format!(
                "workflow {label:?} applicability is empty or multi-line"
            ));
        }
        if row.limitations.trim().is_empty() {
            violations.push(format!("workflow {label:?} limitations are empty"));
        }
        if row.steps.is_empty() {
            violations.push(format!("workflow {label:?} has no required command roles"));
        }
        if row.first_command != row.steps.first().map(|step| step.command).unwrap_or("") {
            violations.push(format!(
                "workflow {label:?} first command does not match its first required role"
            ));
        }
        if row.prerequisites.is_empty() {
            violations.push(format!("workflow {label:?} declares no prerequisites"));
        }
        if row.artifacts_read.is_empty() {
            violations.push(format!("workflow {label:?} declares no artifacts read"));
        }
        if row.result_families.is_empty() {
            violations.push(format!("workflow {label:?} declares no result families"));
        }
        if row.recovery.is_empty() {
            violations.push(format!("workflow {label:?} declares no recovery routes"));
        }

        // Host-independent text (required control 8): no backslashes or URLs
        // can leak host-specific quoting or environments into rendered output.
        let joined_fields = [
            row.artifacts_read.join(" "),
            row.artifacts_written.join(" "),
            row.stop_conditions.join(" "),
            row.prerequisites.join(" "),
        ];
        for field in [row.purpose, row.applicability, row.limitations]
            .into_iter()
            .chain(joined_fields.iter().map(String::as_str))
        {
            if field.contains('\\') || field.contains("://") {
                violations.push(format!(
                    "workflow {label:?} text field contains a host-specific path or URL"
                ));
            }
        }

        workflow_role_violations(entries, command_rows, row, &mut violations);
        workflow_graph_violations(row, &mut violations);

        // Repair workflow law (required control 6): the ordinary public route
        // is agent repair and the low-level commands stay advanced/control.
        if row.command_tag == "repair-loop"
            && row
                .steps
                .first()
                .is_some_and(|step| step.command != "cmd:agent.repair")
        {
            violations.push(format!(
                "workflow {label:?} must use cmd:agent.repair as its ordinary public route"
            ));
        }
    }
    violations
}

fn workflow_role_violations(
    entries: &[CommandCatalogEntry],
    command_rows: &[CommandMetadata],
    row: &WorkflowCatalogEntry,
    violations: &mut Vec<String>,
) {
    let label = row.id;
    let mut required_seen = std::collections::BTreeSet::<&str>::new();
    for workflow_step in row.steps {
        if !required_seen.insert(workflow_step.command) {
            violations.push(format!(
                "workflow {label:?} repeats required command role {:?}",
                workflow_step.command
            ));
        }
    }
    let mut optional_seen = std::collections::BTreeSet::<&str>::new();
    for workflow_step in row.optional_steps {
        if !optional_seen.insert(workflow_step.command) {
            violations.push(format!(
                "workflow {label:?} repeats optional command role {:?}",
                workflow_step.command
            ));
        }
        if required_seen.contains(workflow_step.command) {
            violations.push(format!(
                "workflow {label:?} lists {:?} as both required and optional",
                workflow_step.command
            ));
        }
    }

    let mut all_steps: Vec<WorkflowStep> = row.steps.to_vec();
    all_steps.extend_from_slice(row.optional_steps);
    for step in &all_steps {
        let Some(entry) = entries.iter().find(|entry| entry.id == step.command) else {
            violations.push(format!(
                "workflow {label:?} references missing command {:?}",
                step.command
            ));
            continue;
        };
        // Required control 1: workflow steps are registered
        // public/compatibility commands at the intended classification.
        if !matches!(
            entry.class,
            CommandClass::Public | CommandClass::Compatibility
        ) {
            violations.push(format!(
                "workflow {label:?} step {:?} is not a public/compatibility command",
                step.command
            ));
        }

        // Required control 5: mirrored facts must equal the C2 row, so a
        // command-side metadata change contradicting this workflow fails here.
        let Some(command_row) = command_rows
            .iter()
            .find(|command_row| command_row.id == step.command)
        else {
            violations.push(format!(
                "workflow {label:?} step {:?} has no command metadata row",
                step.command
            ));
            continue;
        };
        if step.cost != command_row.cost {
            violations.push(format!(
                "workflow {label:?} step {:?} claims cost that contradicts the command metadata",
                step.command
            ));
        }
        if step.operation != command_row.operation {
            violations.push(format!(
                "workflow {label:?} step {:?} claims an operation class that contradicts the command metadata",
                step.command
            ));
        }
        if step.may_compile != command_row.effects.may_compile
            || step.may_run_tests != command_row.effects.may_run_tests
            || step.may_start_child_process != command_row.effects.may_start_child_process
        {
            violations.push(format!(
                "workflow {label:?} step {:?} claims side effects that contradict the command metadata",
                step.command
            ));
        }
        for claim in step.writes {
            let mut declared = command_row
                .outputs
                .default
                .into_iter()
                .chain(command_row.outputs.optional.iter().copied());
            if !declared.any(|output| output.contains(claim)) {
                violations.push(format!(
                    "workflow {label:?} step {:?} claims artifact {claim:?} the command metadata does not declare",
                    step.command
                ));
            }
        }
    }

    for family in row.result_families {
        if family.family.trim().is_empty() {
            violations.push(format!(
                "workflow {label:?} has a result family with no name"
            ));
        }
        match family.next {
            WorkflowNext::Command(command) => {
                let Some(entry) = entries.iter().find(|entry| entry.id == command) else {
                    violations.push(format!(
                        "workflow {label:?} result family {:?} routes to missing command {command:?}",
                        family.family
                    ));
                    continue;
                };
                if !matches!(
                    entry.class,
                    CommandClass::Public | CommandClass::Compatibility
                ) {
                    violations.push(format!(
                        "workflow {label:?} result family {:?} routes to a non-public command {command:?}",
                        family.family
                    ));
                }
            }
            WorkflowNext::Terminal(stop) => {
                if !stop.starts_with("stop:") {
                    violations.push(format!(
                        "workflow {label:?} result family {:?} terminal route must start with 'stop:'",
                        family.family
                    ));
                }
            }
            WorkflowNext::Limitation(limitation) => {
                if limitation.trim().is_empty() {
                    violations.push(format!(
                        "workflow {label:?} result family {:?} limitation is blank",
                        family.family
                    ));
                }
            }
        }
    }

    for recovery in row.recovery {
        let Some(entry) = entries.iter().find(|entry| entry.id == recovery.route) else {
            violations.push(format!(
                "workflow {label:?} recovery route {:?} is missing",
                recovery.route
            ));
            continue;
        };
        if !matches!(
            entry.class,
            CommandClass::Public | CommandClass::Compatibility
        ) {
            violations.push(format!(
                "workflow {label:?} recovery route {:?} is not a public/compatibility command",
                recovery.route
            ));
        }
    }

    for alternative in row.advanced_alternatives {
        let Some(entry) = entries.iter().find(|entry| entry.id == *alternative) else {
            violations.push(format!(
                "workflow {label:?} advanced alternative {:?} is missing",
                alternative
            ));
            continue;
        };
        if entry.class != CommandClass::Advanced {
            violations.push(format!(
                "workflow {label:?} advanced alternative {:?} is not an Advanced-class command",
                alternative
            ));
        }
    }
}

/// Required control 2 (connectivity) and 3 (order independence): the role
/// graph is walked from declared family sources and recovery routes, never
/// from table position, so reordering the source rows cannot change the
/// verdict; a cycle or a stranded required role fails the check. Every
/// required step must produce at least one result family, so removing one
/// family cannot silently leave a step without its declared outcomes.
fn workflow_graph_violations(row: &WorkflowCatalogEntry, violations: &mut Vec<String>) {
    let label = row.id;
    let step_names: std::collections::BTreeSet<&str> = row
        .steps
        .iter()
        .chain(row.optional_steps.iter())
        .map(|step| step.command)
        .collect();

    // Family edges come from the declared producer of each family, so
    // removing a family removes exactly its edges; there are no implicit
    // adjacent-step edges that could mask the loss. A family that routes back
    // to its own producing step is a stay-route, not an edge.
    let mut edges: Vec<(&str, &str)> = Vec::new();
    for family in row.result_families {
        if !step_names.contains(family.from) {
            violations.push(format!(
                "workflow {label:?} result family {:?} names producer {:?} that is not a workflow step",
                family.family, family.from
            ));
        }
        if let WorkflowNext::Command(target) = family.next
            && family.from != target
        {
            edges.push((family.from, target));
        }
    }
    for step in row.steps {
        if !row
            .result_families
            .iter()
            .any(|family| family.from == step.command)
        {
            violations.push(format!(
                "workflow {label:?} required role {:?} produces no result family",
                step.command
            ));
        }
    }
    for recovery in row.recovery {
        if let Some(first) = row.steps.first() {
            // A recovery route back to the first command is a retry loop
            // (for example re-running `cmd:check` on a stale finding), not
            // a graph cycle; only cross-command recovery routes are edges.
            if first.command != recovery.route {
                edges.push((first.command, recovery.route));
            }
        }
    }

    // Cycle detection over the declared edges.
    let mut nodes: std::collections::BTreeSet<&str> = step_names;
    for (from, to) in &edges {
        nodes.insert(from);
        nodes.insert(to);
    }
    let mut visited = std::collections::BTreeSet::<&str>::new();
    let mut in_stack = std::collections::BTreeSet::<&str>::new();
    for node in nodes.iter().copied() {
        if visited.contains(node) {
            continue;
        }
        if has_cycle(node, &edges, &mut visited, &mut in_stack) {
            violations.push(format!(
                "workflow {label:?} role graph has a cycle at {node:?}"
            ));
            return;
        }
    }

    // Every required role must be reachable from the first command through
    // the declared edges.
    let Some(first) = row.steps.first().map(|step| step.command) else {
        return;
    };
    let mut reachable = std::collections::BTreeSet::<&str>::new();
    let mut queue = vec![first];
    while let Some(node) = queue.pop() {
        if !reachable.insert(node) {
            continue;
        }
        for (from, to) in &edges {
            if *from == node {
                queue.push(to);
            }
        }
    }
    for step in row.steps {
        if !reachable.contains(step.command) {
            violations.push(format!(
                "workflow {label:?} required role {:?} is unreachable from its first command",
                step.command
            ));
        }
    }
}

fn has_cycle<'a>(
    node: &'a str,
    edges: &[(&'a str, &'a str)],
    visited: &mut std::collections::BTreeSet<&'a str>,
    in_stack: &mut std::collections::BTreeSet<&'a str>,
) -> bool {
    if in_stack.contains(node) {
        return true;
    }
    if !visited.insert(node) {
        return false;
    }
    in_stack.insert(node);
    for (from, to) in edges {
        if *from == node && has_cycle(to, edges, visited, in_stack) {
            return true;
        }
    }
    in_stack.remove(node);
    false
}

fn is_kebab_case(text: &str) -> bool {
    !text.is_empty()
        && !text.starts_with('-')
        && !text.ends_with('-')
        && text
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
}

/// Render the bounded workflow listing for `ripr help workflow`.
pub(crate) fn render_workflow_listing() -> String {
    let mut out = String::from(
        "RIPR workflows — bounded task guidance. Nothing on this screen runs a command.\n\n",
    );
    let mut rows: Vec<&WorkflowCatalogEntry> = workflow_catalog().iter().collect();
    rows.sort_by(|left, right| left.id.cmp(right.id));
    for row in rows {
        out.push_str("  ");
        out.push_str(row.id);
        out.push_str("\n      ");
        out.push_str(row.purpose);
        out.push('\n');
    }
    out.push_str(
        "\nRun `ripr help workflow <name>` for one workflow's steps, artifacts, recovery, and stop conditions.",
    );
    out
}

/// Render one workflow for `ripr help workflow <name>`. Unknown names produce
/// scoped suggestions from workflow identities and aliases only — never from
/// command names — and the error family is distinct from unknown commands
/// (required control 7).
pub(crate) fn render_workflow(name: &str) -> Result<String, String> {
    let Some(row) = workflow_for(name) else {
        return Err(unknown_workflow_error(name));
    };
    let mut out = String::new();
    out.push_str("Workflow: ");
    out.push_str(row.id);
    out.push('\n');
    if !row.aliases.is_empty() {
        out.push_str("Aliases: ");
        out.push_str(&row.aliases.join(", "));
        out.push('\n');
    }
    out.push_str("Purpose: ");
    out.push_str(row.purpose);
    out.push_str("\nApplies when: ");
    out.push_str(row.applicability);
    out.push_str("\nTag: ");
    out.push_str(row.command_tag);
    out.push_str(" (commands carrying this workflow tag are this workflow's members)\n");

    out.push_str("\nPrerequisites:\n");
    for prerequisite in row.prerequisites {
        out.push_str("  - ");
        out.push_str(prerequisite);
        out.push('\n');
    }

    out.push_str("\nCommands (ordered):\n");
    for (index, workflow_step) in row.steps.iter().enumerate() {
        push_step_line(&mut out, &format!("  {}. ", index + 1), workflow_step);
    }
    if !row.optional_steps.is_empty() {
        out.push_str("\nOptional commands:\n");
        for workflow_step in row.optional_steps {
            push_step_line(&mut out, "  - ", workflow_step);
        }
    }

    out.push_str("\nResult families:\n");
    for family in row.result_families {
        out.push_str("  - ");
        out.push_str(family.family);
        out.push_str(": ");
        match family.next {
            WorkflowNext::Command(command) => {
                out.push_str("next `ripr ");
                out.push_str(command_path(command));
                out.push('`');
            }
            WorkflowNext::Terminal(stop) => out.push_str(stop),
            WorkflowNext::Limitation(limitation) => {
                out.push_str("limitation: ");
                out.push_str(limitation);
            }
        }
        out.push('\n');
    }

    out.push_str("\nArtifacts:\n  reads: ");
    out.push_str(&row.artifacts_read.join("; "));
    out.push_str("\n  writes: ");
    out.push_str(&row.artifacts_written.join("; "));
    out.push('\n');

    out.push_str("\nRecovery:\n");
    for recovery in row.recovery {
        out.push_str("  - ");
        out.push_str(recovery.when);
        out.push_str(": `ripr ");
        out.push_str(command_path(recovery.route));
        out.push_str("`\n");
    }

    out.push_str("\nStop conditions:\n");
    for stop in row.stop_conditions {
        out.push_str("  - ");
        out.push_str(stop);
        out.push('\n');
    }

    if !row.advanced_alternatives.is_empty() {
        out.push_str("\nAdvanced/control alternatives:\n");
        for alternative in row.advanced_alternatives {
            out.push_str("  - ripr ");
            out.push_str(command_path(alternative));
            if let Some(summary) = command_summary(alternative) {
                out.push_str(" — ");
                out.push_str(summary);
            }
            out.push('\n');
        }
    }

    out.push_str("\nLimitations: ");
    out.push_str(row.limitations);
    out.push('\n');
    Ok(out)
}

fn push_step_line(out: &mut String, prefix: &str, workflow_step: &WorkflowStep) {
    out.push_str(prefix);
    out.push_str("ripr ");
    out.push_str(command_path(workflow_step.command));
    out.push_str(" — ");
    out.push_str(workflow_step.role);
    out.push_str("\n     cost: ");
    out.push_str(cost_label(workflow_step.cost));
    out.push_str("; effects: ");
    out.push_str(&effects_label(workflow_step));
    if !workflow_step.writes.is_empty() {
        out.push_str("; writes: ");
        out.push_str(&workflow_step.writes.join("; "));
    }
    out.push('\n');
}

fn cost_label(cost: CommandCost) -> &'static str {
    match cost {
        CommandCost::Small => "small (projection over existing artifacts)",
        CommandCost::Analysis => "analysis (diff-scoped static analysis)",
        CommandCost::Workspace => "workspace (whole-workspace analysis)",
    }
}

fn effects_label(workflow_step: &WorkflowStep) -> String {
    let mut labels = Vec::new();
    if workflow_step.may_compile {
        labels.push("compiles");
    }
    if workflow_step.may_run_tests {
        labels.push("runs tests");
    }
    if workflow_step.may_start_child_process {
        labels.push("may start a bounded child process");
    }
    if labels.is_empty() {
        "no compile, test, or child-process effects".to_string()
    } else {
        labels.join(", ")
    }
}

fn command_path(command_id: &str) -> &'static str {
    catalog()
        .iter()
        .find(|entry| entry.id == command_id)
        .map(|entry| entry.path)
        .unwrap_or("?")
}

fn command_summary(command_id: &str) -> Option<&'static str> {
    catalog()
        .iter()
        .find(|entry| entry.id == command_id)
        .and_then(metadata_for)
        .map(|row| row.summary)
}

/// Unknown-workflow error, deliberately distinct from the unknown-command
/// parse error: suggestions draw only from workflow identities and aliases.
fn unknown_workflow_error(name: &str) -> String {
    match closest_workflow(name) {
        Some(suggestion) => format!(
            "unknown workflow {name:?}. Did you mean the workflow `{suggestion}`? Run `ripr help workflow` for the workflow list."
        ),
        None => {
            format!("unknown workflow {name:?}. Run `ripr help workflow` for the workflow list.")
        }
    }
}

fn closest_workflow(name: &str) -> Option<&'static str> {
    let typo_budget = if name.len() <= 4 { 1 } else { 3 };
    WORKFLOWS
        .iter()
        .flat_map(|row| std::iter::once(row.id).chain(row.aliases.iter().copied()))
        .map(|known| (known, edit_distance(name, known)))
        .filter(|(_, distance)| *distance <= typo_budget)
        .min_by_key(|(known, distance)| (*distance, *known))
        .map(|(known, _)| known)
}

fn edit_distance(left: &str, right: &str) -> usize {
    let right_chars: Vec<char> = right.chars().collect();
    let mut previous: Vec<usize> = (0..=right_chars.len()).collect();
    let mut current = vec![0; right_chars.len() + 1];

    for (left_idx, left_char) in left.chars().enumerate() {
        current[0] = left_idx + 1;
        for (right_idx, right_char) in right_chars.iter().enumerate() {
            let substitution_cost = usize::from(left_char != *right_char);
            let deletion = previous[right_idx + 1] + 1;
            let insertion = current[right_idx] + 1;
            let substitution = previous[right_idx] + substitution_cost;
            current[right_idx + 1] = deletion.min(insertion).min(substitution);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right_chars.len()]
}

#[cfg(test)]
mod tests {
    use super::{
        WORKFLOWS, WorkflowCatalogEntry, WorkflowNext, WorkflowResultFamily, WorkflowStep,
        render_workflow, render_workflow_listing, workflow_catalog, workflow_catalog_violations,
        workflow_for,
    };
    use crate::cli::command_catalog::{CommandClass, catalog};
    use crate::cli::command_metadata::{CommandCost, metadata};

    fn production_violations() -> Vec<String> {
        workflow_catalog_violations(catalog(), metadata(), WORKFLOWS)
    }

    /// Test-only promotion of a mutated owned Vec into the table's static
    /// slice shape. Bounded fixture use; production rows are const data.
    fn leaked_steps(steps: Vec<WorkflowStep>) -> &'static [WorkflowStep] {
        Box::leak(steps.into_boxed_slice())
    }

    fn leaked_families(families: Vec<WorkflowResultFamily>) -> &'static [WorkflowResultFamily] {
        Box::leak(families.into_boxed_slice())
    }

    fn with_mutated_row(
        id: &str,
        mutate: impl Fn(&mut WorkflowCatalogEntry),
    ) -> Result<Vec<WorkflowCatalogEntry>, String> {
        let mut rows: Vec<WorkflowCatalogEntry> = WORKFLOWS.to_vec();
        let Some(row) = rows.iter_mut().find(|row| row.id == id) else {
            return Err(format!("test table lacks workflow {id:?}"));
        };
        mutate(row);
        Ok(rows)
    }

    fn expect_single_violation(rows: &[WorkflowCatalogEntry], needle: &str) -> Result<(), String> {
        let violations = workflow_catalog_violations(catalog(), metadata(), rows);
        let matching: Vec<&String> = violations
            .iter()
            .filter(|violation| violation.contains(needle))
            .collect();
        match matching.len() {
            1 => Ok(()),
            0 => Err(format!(
                "expected exactly one {needle:?} violation, got {violations:?}"
            )),
            count => Err(format!(
                "expected exactly one {needle:?} violation, got {count}: {violations:?}"
            )),
        }
    }

    #[test]
    fn production_workflow_catalog_is_consistent() -> Result<(), String> {
        let violations = production_violations();
        if violations.is_empty() {
            return Ok(());
        }
        Err(format!("workflow catalog violations: {violations:?}"))
    }

    #[test]
    fn initial_denominator_is_present() -> Result<(), String> {
        for id in [
            "inspect-change",
            "guided-adoption",
            "repair-gap",
            "compose-pr-evidence",
            "adopt-ci",
        ] {
            if workflow_for(id).is_none() {
                return Err(format!("workflow {id:?} is missing from the catalog"));
            }
        }
        Ok(())
    }

    #[test]
    fn listing_render_is_sorted_bounded_and_host_independent() -> Result<(), String> {
        let listing = render_workflow_listing();
        let mut last: Option<usize> = None;
        for id in [
            "adopt-ci",
            "compose-pr-evidence",
            "guided-adoption",
            "inspect-change",
            "repair-gap",
        ] {
            let Some(pos) = listing.find(id) else {
                return Err(format!("listing omits workflow {id:?}:\n{listing}"));
            };
            if let Some(previous) = last
                && pos < previous
            {
                return Err(format!("listing is not sorted by id:\n{listing}"));
            }
            last = Some(pos);
        }
        if listing.lines().count() > 40 {
            return Err(format!("workflow listing exceeds its bound:\n{listing}"));
        }
        if listing.contains('\\') || listing.contains("://") || listing.contains("C:") {
            return Err(format!("listing leaks host-specific text:\n{listing}"));
        }
        Ok(())
    }

    #[test]
    fn single_workflow_render_is_deterministic() -> Result<(), String> {
        let first = render_workflow("repair-gap").map_err(|error| error.to_string())?;
        let second = render_workflow("repair-gap").map_err(|error| error.to_string())?;
        if first == second && first.lines().count() <= 80 {
            return Ok(());
        }
        Err("workflow render is not deterministic or exceeds its bound".to_string())
    }

    #[test]
    fn aliases_resolve_to_the_canonical_workflow() -> Result<(), String> {
        let by_alias = render_workflow("adoption").map_err(|error| error.to_string())?;
        let by_id = render_workflow("guided-adoption").map_err(|error| error.to_string())?;
        if by_alias == by_id && by_alias.starts_with("Workflow: guided-adoption") {
            return Ok(());
        }
        Err(format!("alias render diverged:\n{by_alias}"))
    }

    #[test]
    fn unknown_workflow_suggests_only_workflow_identities() -> Result<(), String> {
        let Err(error) = render_workflow("inspec-change") else {
            return Err("a typo workflow name must not render".to_string());
        };
        if !error.starts_with("unknown workflow") {
            return Err(format!("wrong error family: {error}"));
        }
        if error.contains("unknown command") {
            return Err(format!("used the unknown-command family: {error}"));
        }
        if !error.contains("`inspect-change`") {
            return Err(format!("missing scoped suggestion: {error}"));
        }
        let Err(command_like) = render_workflow("check") else {
            return Err("a bare command name must not render as a workflow".to_string());
        };
        if command_like.contains("Did you mean") && command_like.contains("`check`") {
            return Err(format!("suggested a command name: {command_like}"));
        }
        Ok(())
    }

    #[test]
    fn repair_gap_law_holds() -> Result<(), String> {
        let Some(row) = workflow_for("repair-gap") else {
            return Err("repair-gap missing".to_string());
        };
        let Some(first) = row.steps.first() else {
            return Err("repair-gap has no required steps".to_string());
        };
        if first.command != "cmd:agent.repair" {
            return Err(format!(
                "repair-gap must start at cmd:agent.repair, got {:?}",
                first.command
            ));
        }
        if row.advanced_alternatives.is_empty() {
            return Err(
                "repair-gap must keep the low-level commands as advanced/control routes"
                    .to_string(),
            );
        }
        for alternative in row.advanced_alternatives {
            let Some(entry) = catalog().iter().find(|entry| entry.id == *alternative) else {
                return Err(format!("advanced alternative {alternative:?} missing"));
            };
            if entry.class != CommandClass::Advanced {
                return Err(format!(
                    "repair-gap alternative {alternative:?} is not Advanced class"
                ));
            }
        }
        let rendered = render_workflow("repair-gap")?;
        for non_claim in [
            "never performs the edit",
            "never closes an attempt",
            "--verify-authorized and a matching authority",
        ] {
            if !rendered.contains(non_claim) {
                return Err(format!(
                    "repair-gap render lost the non-claim {non_claim:?}"
                ));
            }
        }
        if rendered.contains("never runs the authorized test command itself") {
            return Err(
                "repair-gap render still claims ripr never runs the verify command; the verify phase runs it when explicitly authorized"
                    .to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn missing_required_command_row_is_rejected() -> Result<(), String> {
        let rows = with_mutated_row("inspect-change", |row| {
            let mut steps = row.steps.to_vec();
            if steps.is_empty() {
                return;
            }
            steps[0].command = "cmd:not-a-command";
            row.first_command = "cmd:not-a-command";
            row.steps = leaked_steps(steps);
        })?;
        expect_single_violation(&rows, "references missing command")
    }

    #[test]
    fn duplicate_required_role_is_rejected() -> Result<(), String> {
        let rows = with_mutated_row("inspect-change", |row| {
            let mut steps = row.steps.to_vec();
            if steps.len() < 2 {
                return;
            }
            steps[1].command = steps[0].command;
            row.steps = leaked_steps(steps);
        })?;
        expect_single_violation(&rows, "repeats required command role")
    }

    #[test]
    fn cyclic_workflow_edge_is_rejected() -> Result<(), String> {
        // Route the "handoff context collected" family (produced by
        // cmd:context) back to the first command; this closes
        // check -> explain -> context -> check.
        let rows = with_mutated_row("inspect-change", |row| {
            let mut families: Vec<WorkflowResultFamily> = row.result_families.to_vec();
            if families.len() < 3 {
                return;
            }
            families[2].next = WorkflowNext::Command("cmd:check");
            row.result_families = leaked_families(families);
        })?;
        expect_single_violation(&rows, "cycle")
    }

    #[test]
    fn removing_a_workflow_edge_is_rejected() -> Result<(), String> {
        // Emptying the result families removes the typed next routes; the
        // families themselves are the workflow edges the catalog check owns.
        let rows = with_mutated_row("inspect-change", |row| {
            row.result_families = leaked_families(Vec::new());
        })?;
        let violations = workflow_catalog_violations(catalog(), metadata(), &rows);
        if violations
            .iter()
            .any(|violation| violation.contains("declares no result families"))
        {
            return Ok(());
        }
        Err(format!(
            "expected a missing-families violation, got {violations:?}"
        ))
    }

    #[test]
    fn removing_one_result_family_is_rejected() -> Result<(), String> {
        // Removing a single family must not silently pass: its producing step
        // loses its declared outcome even though the remaining family edges
        // still connect the step chain.
        let rows = with_mutated_row("inspect-change", |row| {
            let mut families: Vec<WorkflowResultFamily> = row.result_families.to_vec();
            if families.len() < 2 {
                return;
            }
            families.remove(1); // "finding explained", produced by cmd:explain
            row.result_families = leaked_families(families);
        })?;
        expect_single_violation(&rows, "produces no result family")
    }

    #[test]
    fn alias_collision_with_a_later_workflow_id_is_rejected() -> Result<(), String> {
        // An alias that equals the canonical id of a *later* row must still
        // collide; the first-match lookup would otherwise resolve the later
        // id to the earlier workflow.
        let rows = with_mutated_row("inspect-change", |row| {
            row.aliases = Box::leak(vec!["guided-adoption"].into_boxed_slice());
        })?;
        expect_single_violation(&rows, "collides with another identity")
    }

    #[test]
    fn non_public_step_classification_is_rejected() -> Result<(), String> {
        let rows = with_mutated_row("inspect-change", |row| {
            let mut steps = row.steps.to_vec();
            if steps.is_empty() {
                return;
            }
            steps[0].command = "cmd:agent.start";
            row.steps = leaked_steps(steps);
        })?;
        let violations = workflow_catalog_violations(catalog(), metadata(), &rows);
        if violations
            .iter()
            .any(|violation| violation.contains("not a public/compatibility command"))
        {
            return Ok(());
        }
        Err(format!(
            "expected classification violation, got {violations:?}"
        ))
    }

    #[test]
    fn advanced_alternative_must_be_advanced_class() -> Result<(), String> {
        let rows = with_mutated_row("repair-gap", |row| {
            row.advanced_alternatives = Box::leak(vec!["cmd:check"].into_boxed_slice());
        })?;
        expect_single_violation(&rows, "not an Advanced-class command")
    }

    #[test]
    fn step_cost_contradicting_command_metadata_is_rejected() -> Result<(), String> {
        // Simulate the command-side change: the C2 row for cmd:check drops to
        // Small while the workflow still claims Analysis for that step.
        let mut command_rows: Vec<crate::cli::command_metadata::CommandMetadata> =
            metadata().to_vec();
        let Some(check) = command_rows.iter_mut().find(|row| row.id == "cmd:check") else {
            return Err("cmd:check metadata row missing".to_string());
        };
        check.cost = CommandCost::Small;
        let violations = workflow_catalog_violations(catalog(), &command_rows, WORKFLOWS);
        if violations
            .iter()
            .any(|violation| violation.contains("contradicts the command metadata"))
        {
            return Ok(());
        }
        Err(format!(
            "expected a contradictory-metadata violation, got {violations:?}"
        ))
    }

    #[test]
    fn step_artifact_claim_not_in_command_metadata_is_rejected() -> Result<(), String> {
        let rows = with_mutated_row("inspect-change", |row| {
            let mut steps = row.steps.to_vec();
            if steps.is_empty() {
                return;
            }
            steps[0].writes = Box::leak(vec!["target/ripr/secret-report.json"].into_boxed_slice());
            row.steps = leaked_steps(steps);
        })?;
        expect_single_violation(&rows, "the command metadata does not declare")
    }

    #[test]
    fn tag_promotion_requires_command_members() -> Result<(), String> {
        let rows = with_mutated_row("inspect-change", |row| {
            row.command_tag = "no-such-tag";
        })?;
        let violations = workflow_catalog_violations(catalog(), metadata(), &rows);
        if violations
            .iter()
            .any(|violation| violation.contains("is not in WORKFLOW_TAGS"))
        {
            return Ok(());
        }
        Err(format!(
            "expected an unknown-tag violation, got {violations:?}"
        ))
    }

    #[test]
    fn repair_workflow_must_start_at_agent_repair() -> Result<(), String> {
        let rows = with_mutated_row("repair-gap", |row| {
            let mut steps = row.steps.to_vec();
            if steps.is_empty() {
                return;
            }
            steps[0].command = "cmd:rerun";
            row.first_command = "cmd:rerun";
            row.steps = leaked_steps(steps);
        })?;
        expect_single_violation(
            &rows,
            "must use cmd:agent.repair as its ordinary public route",
        )
    }

    #[test]
    fn workflow_accessor_exposes_the_governed_table() -> Result<(), String> {
        if workflow_catalog().len() == WORKFLOWS.len() && !workflow_catalog().is_empty() {
            return Ok(());
        }
        Err("workflow_catalog() does not expose the governed table".to_string())
    }
}
