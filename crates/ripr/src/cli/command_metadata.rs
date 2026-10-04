//! Rich per-command metadata projected into human help and inventories.
//!
//! RIPR-SPEC-0187 / issue #4823 (command-discovery C2). The C1 catalog
//! ([`crate::cli::command_catalog`]) owns command identity, aliases, and
//! classification; this module owns what each command does, what it may cost
//! or change, what it consumes and produces, and where it sits in the
//! canonical task hierarchy. Human surfaces validate or render from these two
//! tables; they do not scrape each other.
//!
//! Field authorities, one per material fact:
//!
//! - identity, aliases, classification, discovery posture: the C1 catalog
//!   (`CommandCatalogEntry`), never re-declared here;
//! - operation, outputs, and side-effect flags: existing command behavior and
//!   help, under the cost/side-effect vocabulary of #1572 (projection-only,
//!   `will_scan_workspace`, `will_compile`, `will_run_tests`,
//!   `will_create_or_refresh_baseline`, reads/writes disclosure) and the
//!   bounded-help boundary from PR #2789;
//! - cost class: `check --mode` help (`Cost class: whole-workspace modes ...
//!   order of magnitude longer`) and #1572's `cost_class` vocabulary;
//! - the no-mutation boundary: the advisory footer shared by the help
//!   screens ("It does not run mutants"), so no row may claim
//!   `may_run_mutation`;
//! - canonical task labels: `docs/COMMAND_HIERARCHY.md` and the default help
//!   task map (one vocabulary, checked here in both directions);
//! - workflow memberships: a closed tag set in [`WORKFLOW_TAGS`], promoted to
//!   the typed workflow catalog by #4824
//!   ([`crate::cli::workflow_catalog`]) and never re-derived from prose.
//!
//! The #4825 `help --json` child (RIPR-SPEC-0190) is the second production
//! consumer: it projects this table into the versioned machine-discovery
//! document, so a command-side metadata edit remints the document digest. The
//! seam for the JSON child is [`metadata()`] plus [`catalog()`]: a complete
//! typed command description with no human-text scraping. The #4824 workflow
//! catalog is the first production consumer of this table; its validators
//! cross-check workflow summaries against these rows so a command-side change
//! contradicting a workflow fails visibly.
//!
//! The whole module is the C2 query surface consumed by both production
//! children.

use crate::cli::command_catalog::{CommandCatalogEntry, CommandClass, catalog};

#[cfg(test)]
use crate::cli::command_catalog::{CommandRelation, DiscoveryPosture};

/// Closed workflow-membership tag set. #4824 promotes these tags into the
/// typed workflow catalog; until then they are opaque labels, not a state
/// model.
pub(crate) const WORKFLOW_TAGS: &[&str] = &[
    "setup",
    "adoption",
    "inspect-change",
    "repair-loop",
    "pr-evidence",
    "review",
    "policy-gate",
    "reports",
    "calibration",
    "editor-agent",
    "feedback",
];

/// What running a command may change. A row cannot declare a change class the
/// command does not already perform (issue #4823 required control 2).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommandOperation {
    /// Reads workspace state and writes nothing.
    ReadOnly,
    /// Writes product artifacts, by default under `target/ripr/` or an
    /// explicit `--out` path.
    WritesArtifacts,
    /// Mutates durable state outside a single product-artifact write (cache
    /// entries, installed workflows, ledgers the next run consumes).
    StateChanging,
}

impl CommandOperation {
    /// Machine token for the versioned discovery document (#4825); the
    /// human help labels render separately.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::WritesArtifacts => "writes-artifacts",
            Self::StateChanging => "state-changing",
        }
    }
}

/// Cost class vocabulary shared with #1572 and the `check --mode` help.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommandCost {
    /// Projection over existing artifacts; seconds.
    Small,
    /// Diff-scoped static analysis; seconds to minutes.
    Analysis,
    /// Whole-workspace analysis; an order of magnitude longer (check --mode).
    Workspace,
}

impl CommandCost {
    /// Machine token for the versioned discovery document (#4825); the
    /// human help labels render separately.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Analysis => "analysis",
            Self::Workspace => "workspace",
        }
    }
}

/// Independent side-effect disclosures (#1572 vocabulary plus child
/// processes). Each flag is explicit on every row so a cost/side-effect fact
/// can never be inferred away (issue #4823 required control 4).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CommandEffects {
    pub(crate) may_run_analysis: bool,
    pub(crate) may_compile: bool,
    pub(crate) may_run_tests: bool,
    pub(crate) may_run_mutation: bool,
    pub(crate) may_use_network: bool,
    pub(crate) may_start_child_process: bool,
}

/// Output roles. `default` is what an unremarkable invocation writes;
/// `optional` names further product outputs a row may write.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CommandOutputs {
    pub(crate) default: Option<&'static str>,
    pub(crate) optional: &'static [&'static str],
}

/// Rich metadata for one catalog row, keyed by the catalog identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CommandMetadata {
    /// Catalog identity (`CommandCatalogEntry::id`); the one join key.
    pub(crate) id: &'static str,
    /// One-line statement of what the command does.
    pub(crate) summary: &'static str,
    /// Canonical task label from the hierarchy vocabulary; free text is
    /// allowed but the pinned primary-job rows must match the docs exactly.
    pub(crate) task: &'static str,
    /// Membership tags drawn from [`WORKFLOW_TAGS`].
    pub(crate) workflows: &'static [&'static str],
    pub(crate) operation: CommandOperation,
    pub(crate) cost: CommandCost,
    pub(crate) effects: CommandEffects,
    /// What the command reads; empty only for commands that need no input
    /// beyond the workspace itself.
    pub(crate) primary_inputs: &'static [&'static str],
    pub(crate) outputs: CommandOutputs,
    /// Durable state this command mutates; required for
    /// [`CommandOperation::StateChanging`].
    pub(crate) state_target: Option<&'static str>,
    /// Whether the command has a machine/JSON result document.
    pub(crate) json_support: bool,
    /// Example invocation synopsis, rendered in `help --all`.
    pub(crate) example: &'static str,
    /// Catalog paths a reader can reasonably run next.
    pub(crate) next_routes: &'static [&'static str],
    /// Explicit stop or terminal states the command names.
    pub(crate) stop_states: &'static [&'static str],
    /// Advisory boundaries and non-claims; never empty for a described row.
    pub(crate) limitations: &'static str,
    /// Explicit not-applicable reason. When set, the content fields above are
    /// not required; required control 1 is satisfied by the reason itself.
    pub(crate) not_applicable_reason: Option<&'static str>,
}

/// Rows must not claim these tokens; they belong to the runtime-mutation
/// contract, which RIPR does not perform ("It does not run mutants" is the
/// shared help-screen boundary and RIPR's product contract).
const BANNED_CLAIM_TOKENS: &[&str] = &[
    "proven", // ripr-allow: static-language: validator must name the prohibited tokens to reject them
    "killed", // ripr-allow: static-language: validator must name the prohibited tokens to reject them
    "survived", // ripr-allow: static-language: validator must name the prohibited tokens to reject them
    "untested", // ripr-allow: static-language: validator must name the prohibited tokens to reject them
    "adequate", // ripr-allow: static-language: validator must name the prohibited tokens to reject them
];

/// Family effect blocks shared by rows with identical behavior. Keeping them
/// as named constants makes a family-wide side-effect change one visible edit.
const READS_ONLY: CommandEffects = CommandEffects {
    may_run_analysis: false,
    may_compile: false,
    may_run_tests: false,
    may_run_mutation: false,
    may_use_network: false,
    may_start_child_process: false,
};

/// Analysis rows recompute static facts and may start one bounded child
/// process (Git revision resolution through `crate::git`, or a workspace
/// index probe); they never compile, run tests, or use the network.
const ANALYSIS_RUNNER: CommandEffects = CommandEffects {
    may_run_analysis: true,
    may_compile: false,
    may_run_tests: false,
    may_run_mutation: false,
    may_use_network: false,
    may_start_child_process: true,
};

/// The verify-execute child is exactly one bounded `ripr agent verify --json`
/// projection over saved snapshots; no compile, no test run, no network.
const VERIFY_PACKET_RUNNER: CommandEffects = CommandEffects {
    may_run_analysis: false,
    may_compile: false,
    may_run_tests: false,
    may_run_mutation: false,
    may_use_network: false,
    may_start_child_process: true,
};

/// Doctor probes installed tool versions but runs no analysis.
const TOOL_PROBES: CommandEffects = CommandEffects {
    may_run_analysis: false,
    may_compile: false,
    may_run_tests: false,
    may_run_mutation: false,
    may_use_network: false,
    may_start_child_process: true,
};

/// The governed rich-metadata table. One row per described catalog entry.
const METADATA: &[CommandMetadata] = &[
    CommandMetadata {
        id: "cmd:help",
        summary: "Route to per-command options and the exhaustive reference.",
        task: "Read detailed help",
        workflows: &["setup"],
        operation: CommandOperation::ReadOnly,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &[],
        outputs: CommandOutputs {
            default: None,
            optional: &[],
        },
        state_target: None,
        json_support: false,
        example: "ripr help <command>",
        next_routes: &["help", "doctor", "check"],
        stop_states: &[],
        limitations: "prints text only; performs no analysis, compilation, test, process, network, mutation, or product-artifact work.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:init",
        summary: "Install the ripr.toml configuration (and, with --ci, an advisory workflow).",
        task: "Add advisory CI",
        workflows: &["setup"],
        operation: CommandOperation::StateChanging,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &["the selected repository root"],
        outputs: CommandOutputs {
            default: Some("ripr.toml"),
            optional: &[".github/workflows/ripr.yml with --ci github"],
        },
        state_target: Some(
            "ripr.toml, plus the repository's GitHub Actions workflow set with --ci",
        ),
        json_support: false,
        example: "ripr init",
        next_routes: &["doctor", "check"],
        stop_states: &[
            "explicit refusal",
            "existing workflow kept when writing cannot finish",
        ],
        limitations: "writes ripr.toml by default and an advisory, non-blocking workflow only with --ci github; never edits source, gates merges, or contacts the network.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:config",
        summary: "Show the configuration command surface.",
        task: "Check configuration",
        workflows: &["setup"],
        operation: CommandOperation::ReadOnly,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &[],
        outputs: CommandOutputs {
            default: None,
            optional: &[],
        },
        state_target: None,
        json_support: false,
        example: "ripr config",
        next_routes: &["config validate", "doctor"],
        stop_states: &[],
        limitations: "prints the config surface; validation lives in `ripr config validate`.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:config.validate",
        summary: "Validate ripr.toml without running analysis.",
        task: "Check configuration",
        workflows: &["setup"],
        operation: CommandOperation::ReadOnly,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &["ripr.toml (optional)"],
        outputs: CommandOutputs {
            default: None,
            optional: &[],
        },
        state_target: None,
        json_support: false,
        example: "ripr config validate --root PATH",
        next_routes: &["doctor", "check"],
        stop_states: &["explicit refusal on invalid configuration"],
        limitations: "checks configuration only; it does not analyze, compile, or run tests.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:pilot",
        summary: "Find the top test gap and write an actionable pilot packet.",
        task: "Explore the repository",
        workflows: &["adoption", "inspect-change"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Workspace,
        effects: ANALYSIS_RUNNER,
        primary_inputs: &["the current git diff or workspace"],
        outputs: CommandOutputs {
            default: Some("target/ripr/pilot packet (--out overrides)"),
            optional: &[],
        },
        state_target: None,
        json_support: false,
        example: "ripr pilot --root .",
        next_routes: &["check", "agent repair", "doctor"],
        stop_states: &[
            "no actionable gap",
            "partial summary when the budget is reached",
        ],
        limitations: "bounded by --timeout-ms and writes a partial summary rather than exceeding it; points to one next action and does not run mutation testing.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:outcome",
        summary: "Render the movement receipt between two saved snapshots.",
        task: "Compose PR evidence",
        workflows: &["pr-evidence"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &["before and after repo-exposure JSON artifacts"],
        outputs: CommandOutputs {
            default: None,
            optional: &["a rendered receipt file with --out PATH"],
        },
        state_target: None,
        json_support: true,
        example: "ripr outcome --before PATH --after PATH",
        next_routes: &["first-pr", "check"],
        stop_states: &["explicit refusal on malformed snapshots"],
        limitations: "compares two saved snapshots; it does not rerun analysis.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:evidence-health",
        summary: "Summarize how strong the current static evidence looks.",
        task: "Inspect one change",
        workflows: &["inspect-change", "reports"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &[
            "saved reports under target/ripr",
            "optional imported mutation-calibration JSON",
        ],
        outputs: CommandOutputs {
            default: Some("target/ripr/reports/evidence-health.json"),
            optional: &["target/ripr/reports/evidence-health.md"],
        },
        state_target: None,
        json_support: true,
        example: "ripr evidence-health --root PATH",
        next_routes: &["check", "reports index"],
        stop_states: &["explicit refusal on unreadable inputs"],
        limitations: "an advisory analyzer-health projection over saved artifacts; imported calibration availability is disclosed, not re-measured.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:review-comments",
        summary: "Analyze a PR range and draft review comments with evidence links.",
        task: "Compose PR evidence",
        workflows: &["review"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Analysis,
        effects: ANALYSIS_RUNNER,
        primary_inputs: &[
            "--base and --head revisions",
            "cooperative time budget (default 120000 ms)",
        ],
        outputs: CommandOutputs {
            default: Some("target/ripr/review/comments.json"),
            optional: &[],
        },
        state_target: None,
        json_support: true,
        example: "ripr review-comments --root . --base SHA --head SHA",
        next_routes: &["pr-comments plan", "annotations"],
        stop_states: &["explicit refusal on unresolvable revisions"],
        limitations: "bounded by the cooperative budget; drafts stay advisory and are never posted.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:gate",
        summary: "Show the gate command surface.",
        task: "Adopt advisory CI",
        workflows: &["policy-gate"],
        operation: CommandOperation::ReadOnly,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &[],
        outputs: CommandOutputs {
            default: None,
            optional: &[],
        },
        state_target: None,
        json_support: false,
        example: "ripr gate",
        next_routes: &["gate evaluate", "baseline create"],
        stop_states: &[],
        limitations: "prints the gate surface; evaluation lives in `ripr gate evaluate`.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:gate.evaluate",
        summary: "Evaluate advisory gate modes over PR guidance.",
        task: "Adopt advisory CI",
        workflows: &["policy-gate"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &[
            "PR guidance JSON (--pr-guidance)",
            "optional waiver labels JSON",
        ],
        outputs: CommandOutputs {
            default: Some("target/ripr/reports/gate-decision.json (--out overrides)"),
            optional: &[],
        },
        state_target: None,
        json_support: false,
        example: "ripr gate evaluate --pr-guidance PATH",
        next_routes: &["baseline create", "policy readiness", "zero status"],
        stop_states: &["no actionable gap"],
        limitations: "advisory gate evaluation over supplied guidance; acknowledgeable mode only blocks eligible gaps when the configured waiver label is present, and nothing here makes CI blocking.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:baseline",
        summary: "Show the baseline command surface.",
        task: "Adopt advisory CI",
        workflows: &["policy-gate"],
        operation: CommandOperation::ReadOnly,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &[],
        outputs: CommandOutputs {
            default: None,
            optional: &[],
        },
        state_target: None,
        json_support: false,
        example: "ripr baseline",
        next_routes: &["baseline create", "baseline diff"],
        stop_states: &[],
        limitations: "prints the baseline surface; create, diff, and update carry the behavior.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:baseline.create",
        summary: "Create the gate baseline artifact the next runs compare against.",
        task: "Adopt advisory CI",
        workflows: &["policy-gate"],
        operation: CommandOperation::StateChanging,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &["a gate-decision JSON (--from)"],
        outputs: CommandOutputs {
            default: Some(".ripr/gate-baseline.json (--out overrides)"),
            optional: &[],
        },
        state_target: Some(".ripr/gate-baseline.json, consumed by later baseline and zero runs"),
        json_support: false,
        example: "ripr baseline create --from target/ripr/reports/gate-decision.json",
        next_routes: &["baseline diff", "gate evaluate"],
        stop_states: &["explicit refusal without --force when a baseline exists"],
        limitations: "--dry-run previews the write; without --force an existing baseline is kept rather than overwritten.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:baseline.diff",
        summary: "Compare a gate decision against the stored baseline.",
        task: "Adopt advisory CI",
        workflows: &["policy-gate"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &["the stored baseline", "a current gate-decision JSON"],
        outputs: CommandOutputs {
            default: Some("target/ripr/reports/baseline-debt-delta.json"),
            optional: &["target/ripr/reports/baseline-debt-delta.md"],
        },
        state_target: None,
        json_support: true,
        example: "ripr baseline diff --baseline .ripr/gate-baseline.json --current target/ripr/reports/gate-decision.json",
        next_routes: &["zero status", "policy readiness", "baseline update"],
        stop_states: &["explicit refusal on a missing or malformed baseline"],
        limitations: "a projection over two gate artifacts; it does not rerun analysis or mutate the baseline.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:baseline.update",
        summary: "Fold resolved gate debt back into the stored baseline.",
        task: "Adopt advisory CI",
        workflows: &["policy-gate"],
        operation: CommandOperation::StateChanging,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &["the stored baseline", "a current gate-decision JSON"],
        outputs: CommandOutputs {
            default: Some(".ripr/gate-baseline.json (--out overrides)"),
            optional: &[],
        },
        state_target: Some(".ripr/gate-baseline.json, rewritten for later runs"),
        json_support: false,
        example: "ripr baseline update --baseline .ripr/gate-baseline.json --current target/ripr/reports/gate-decision.json --remove-resolved",
        next_routes: &["baseline diff", "zero status"],
        stop_states: &["explicit refusal without --force"],
        limitations: "rewrites the durable baseline the next runs consume; --dry-run previews the change.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:zero",
        summary: "Show the RIPR Zero command surface.",
        task: "Adopt advisory CI",
        workflows: &["policy-gate"],
        operation: CommandOperation::ReadOnly,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &[],
        outputs: CommandOutputs {
            default: None,
            optional: &[],
        },
        state_target: None,
        json_support: false,
        example: "ripr zero",
        next_routes: &["zero status", "policy readiness"],
        stop_states: &[],
        limitations: "prints the zero surface; status carries the behavior.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:zero.status",
        summary: "Report RIPR Zero state from the current policy artifacts.",
        task: "Adopt advisory CI",
        workflows: &["policy-gate"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &[
            "baseline-debt-delta JSON",
            "optional gate-decision and gap-ledger JSON",
        ],
        outputs: CommandOutputs {
            default: Some("target/ripr/reports/ripr-zero-status.json"),
            optional: &["target/ripr/reports/ripr-zero-status.md"],
        },
        state_target: None,
        json_support: true,
        example: "ripr zero status --delta target/ripr/reports/baseline-debt-delta.json",
        next_routes: &["policy readiness", "policy operations"],
        stop_states: &["explicit refusal on missing inputs"],
        limitations: "a projection over saved policy artifacts; it does not rerun analysis or change gate authority.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:policy",
        summary: "Show the policy command surface.",
        task: "Adopt advisory CI",
        workflows: &["policy-gate"],
        operation: CommandOperation::ReadOnly,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &[],
        outputs: CommandOutputs {
            default: None,
            optional: &[],
        },
        state_target: None,
        json_support: false,
        example: "ripr policy",
        next_routes: &["policy readiness", "policy operations", "policy history"],
        stop_states: &[],
        limitations: "prints the policy surface; the subcommands carry the behavior.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:policy.readiness",
        summary: "Project policy readiness from gate and baseline artifacts.",
        task: "Adopt advisory CI",
        workflows: &["policy-gate"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &["optional gate-decision and baseline-debt-delta JSON"],
        outputs: CommandOutputs {
            default: Some("target/ripr/reports/policy-readiness.json"),
            optional: &["target/ripr/reports/policy-readiness.md"],
        },
        state_target: None,
        json_support: true,
        example: "ripr policy readiness",
        next_routes: &["policy operations", "policy promote"],
        stop_states: &["explicit refusal on malformed inputs"],
        limitations: "advisory readiness projection; it does not change gate authority or make CI blocking.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:policy.operations",
        summary: "Compose the policy-operations view over readiness inputs.",
        task: "Adopt advisory CI",
        workflows: &["policy-gate"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &[
            "policy-readiness JSON",
            "optional waiver-aging and suppression-health JSON",
        ],
        outputs: CommandOutputs {
            default: Some("target/ripr/reports/policy-operations.json"),
            optional: &["target/ripr/reports/policy-operations.md"],
        },
        state_target: None,
        json_support: true,
        example: "ripr policy operations --policy-readiness target/ripr/reports/policy-readiness.json",
        next_routes: &["policy history", "policy promote", "policy readiness"],
        stop_states: &["explicit refusal on malformed inputs"],
        limitations: "a projection over saved policy artifacts; it does not change gate authority.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:policy.history",
        summary: "Append a policy-history record and render the history view.",
        task: "Adopt advisory CI",
        workflows: &["policy-gate"],
        operation: CommandOperation::StateChanging,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &["policy-operations JSON (--current)"],
        outputs: CommandOutputs {
            default: Some("target/ripr/reports/policy-history.json (--out overrides)"),
            optional: &[
                "target/ripr/reports/policy-history.md",
                "--out-jsonl PATH append-only JSONL history",
            ],
        },
        state_target: Some(".ripr/policy-history.jsonl, appended only when --out-jsonl names it"),
        json_support: true,
        example: "ripr policy history --current target/ripr/reports/policy-operations.json",
        next_routes: &["policy operations", "policy readiness"],
        stop_states: &["explicit refusal on malformed inputs"],
        limitations: "appends to the durable history ledger only when --out-jsonl names it; without that flag it writes the rendered reports and no history.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:policy.promote",
        summary: "Render a promotion view for the next policy mode.",
        task: "Adopt advisory CI",
        workflows: &["policy-gate"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &["policy-operations JSON", "optional policy-history JSON"],
        outputs: CommandOutputs {
            default: Some("target/ripr/reports/policy-promotion-baseline-check.json"),
            optional: &["target/ripr/reports/policy-promotion-baseline-check.md"],
        },
        state_target: None,
        json_support: true,
        example: "ripr policy promote --to baseline-check --operations target/ripr/reports/policy-operations.json",
        next_routes: &["policy readiness", "gate evaluate"],
        stop_states: &["explicit refusal on malformed inputs"],
        limitations: "renders a promotion view for review; it does not itself change gate authority.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:policy.preview-promote",
        summary: "Render a preview-promotion evidence view for a language class.",
        task: "Adopt advisory CI",
        workflows: &["policy-gate"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &["optional preview-promotion evidence JSON"],
        outputs: CommandOutputs {
            default: Some("target/ripr/reports/preview-promotion-typescript-boundary-gap.json"),
            optional: &["target/ripr/reports/preview-promotion-typescript-boundary-gap.md"],
        },
        state_target: None,
        json_support: true,
        example: "ripr policy preview-promote --language typescript --class boundary_gap",
        next_routes: &["policy promote", "policy readiness"],
        stop_states: &["explicit refusal on malformed inputs"],
        limitations: "preview-limited evidence stays syntax-first and advisory; promotion does not change analyzer behavior.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:policy.waiver-aging",
        summary: "Project waiver-aging health from the PR evidence ledger.",
        task: "Adopt advisory CI",
        workflows: &["policy-gate"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &["optional PR evidence ledger and history JSONL"],
        outputs: CommandOutputs {
            default: Some("target/ripr/reports/waiver-aging.json"),
            optional: &["target/ripr/reports/waiver-aging.md"],
        },
        state_target: None,
        json_support: true,
        example: "ripr policy waiver-aging",
        next_routes: &["policy operations", "pr-ledger record"],
        stop_states: &["explicit refusal on malformed inputs"],
        limitations: "an aging projection over saved evidence; it does not expire waivers on its own.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:policy.suppression-health",
        summary: "Audit suppression declarations for stale or uncovered entries.",
        task: "Adopt advisory CI",
        workflows: &["policy-gate"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &[
            "the selected workspace root",
            "optional suppressions manifest",
        ],
        outputs: CommandOutputs {
            default: Some("target/ripr/reports/suppression-health.json"),
            optional: &["target/ripr/reports/suppression-health.md"],
        },
        state_target: None,
        json_support: true,
        example: "ripr policy suppression-health --root .",
        next_routes: &["policy operations", "policy readiness"],
        stop_states: &["explicit refusal on malformed inputs"],
        limitations: "reports stale or uncovered suppressions; it does not edit the manifest.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:pr-ledger",
        summary: "Show the PR evidence ledger surface.",
        task: "Compose PR evidence",
        workflows: &["pr-evidence"],
        operation: CommandOperation::ReadOnly,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &[],
        outputs: CommandOutputs {
            default: None,
            optional: &[],
        },
        state_target: None,
        json_support: false,
        example: "ripr pr-ledger",
        next_routes: &["pr-ledger record"],
        stop_states: &[],
        limitations: "prints the ledger surface; record carries the behavior.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:pr-ledger.record",
        summary: "Record one PR's evidence state into the ledger and history.",
        task: "Compose PR evidence",
        workflows: &["pr-evidence"],
        operation: CommandOperation::StateChanging,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &[
            "--pr-number, --base, and --head identities",
            "optional gate, baseline-delta, zero-status, and guidance artifacts",
        ],
        outputs: CommandOutputs {
            default: Some("target/ripr/reports/pr-evidence-ledger.json"),
            optional: &[
                "target/ripr/reports/pr-evidence-ledger.md",
                "an append-only JSONL history via --out-jsonl",
            ],
        },
        state_target: Some(
            "the append-only JSONL history (--out-jsonl), consumed by policy waiver-aging and policy history",
        ),
        json_support: true,
        example: "ripr pr-ledger record --pr-number 123 --base SHA --head SHA",
        next_routes: &[
            "policy waiver-aging",
            "policy history",
            "coverage-grip frontier",
        ],
        stop_states: &["explicit refusal on malformed inputs"],
        limitations: "appends history only when --out-jsonl is explicit; generated CI never passes that flag.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:pr-comments",
        summary: "Show the PR comments surface.",
        task: "Compose PR evidence",
        workflows: &["review"],
        operation: CommandOperation::ReadOnly,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &[],
        outputs: CommandOutputs {
            default: None,
            optional: &[],
        },
        state_target: None,
        json_support: false,
        example: "ripr pr-comments",
        next_routes: &["pr-comments plan"],
        stop_states: &[],
        limitations: "prints the comments surface; plan carries the behavior.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:pr-comments.plan",
        summary: "Plan create/update/keep/delete operations for PR comments.",
        task: "Compose PR evidence",
        workflows: &["review"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &[
            "PR guidance JSON from review-comments",
            "optional existing-comment metadata",
        ],
        outputs: CommandOutputs {
            default: Some("target/ripr/review/comment-publish-plan.json"),
            optional: &["target/ripr/review/comment-publish-plan.md"],
        },
        state_target: None,
        json_support: true,
        example: "ripr pr-comments plan --pr-guidance target/ripr/review/comments.json",
        next_routes: &["review-comments", "pr-review front-panel"],
        stop_states: &["blocked operations stay visible in the plan instead of posting"],
        limitations: "a read-only advisory projection; it never posts comments or calls GitHub.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:pr-review",
        summary: "Show the PR review surface.",
        task: "Compose PR evidence",
        workflows: &["review"],
        operation: CommandOperation::ReadOnly,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &[],
        outputs: CommandOutputs {
            default: None,
            optional: &[],
        },
        state_target: None,
        json_support: false,
        example: "ripr pr-review",
        next_routes: &["pr-review front-panel"],
        stop_states: &[],
        limitations: "prints the review surface; front-panel carries the behavior.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:pr-review.front-panel",
        summary: "Compose the first-screen PR review summary from existing artifacts.",
        task: "Compose PR evidence",
        workflows: &["review"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &[
            "one or more of pr-guidance, first-action, assistant-proof, assistant-health, or ledger artifacts",
        ],
        outputs: CommandOutputs {
            default: Some("target/ripr/reports/pr-review-front-panel.json"),
            optional: &["target/ripr/reports/pr-review-front-panel.md"],
        },
        state_target: None,
        json_support: true,
        example: "ripr pr-review front-panel --pr-guidance target/ripr/review/comments.json",
        next_routes: &["pr-comments plan", "first-action"],
        stop_states: &["explicit refusal on malformed inputs"],
        limitations: "a projection over named review artifacts; flag examples document inputs, not defaults that are read for you.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:coverage-grip",
        summary: "Show the coverage-grip surface.",
        task: "Compose PR evidence",
        workflows: &["review"],
        operation: CommandOperation::ReadOnly,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &[],
        outputs: CommandOutputs {
            default: None,
            optional: &[],
        },
        state_target: None,
        json_support: false,
        example: "ripr coverage-grip",
        next_routes: &["coverage-grip frontier"],
        stop_states: &[],
        limitations: "prints the coverage-grip surface; frontier carries the behavior.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:coverage-grip.frontier",
        summary: "Project the coverage-grip frontier from ledger, delta, and zero-status artifacts.",
        task: "Compose PR evidence",
        workflows: &["review"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &[
            "pr evidence ledger, baseline-delta, or zero-status artifact",
            "optional coverage summary",
        ],
        outputs: CommandOutputs {
            default: Some("target/ripr/reports/coverage-grip-frontier.json"),
            optional: &["target/ripr/reports/coverage-grip-frontier.md"],
        },
        state_target: None,
        json_support: true,
        example: "ripr coverage-grip frontier --ledger target/ripr/reports/pr-evidence-ledger.json",
        next_routes: &["zero status", "policy readiness"],
        stop_states: &["explicit refusal on malformed inputs"],
        limitations: "advisory frontier projection; it does not run coverage or mutation tooling.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:assistant-loop",
        summary: "Show the assistant-loop surface.",
        task: "Compose PR evidence",
        workflows: &["review"],
        operation: CommandOperation::ReadOnly,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &[],
        outputs: CommandOutputs {
            default: None,
            optional: &[],
        },
        state_target: None,
        json_support: false,
        example: "ripr assistant-loop",
        next_routes: &["assistant-loop proof", "assistant-loop health"],
        stop_states: &[],
        limitations: "prints the assistant-loop surface; proof and health carry the behavior.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:assistant-loop.proof",
        summary: "Compose the assistant test-oracle proof from review and repair artifacts.",
        task: "Compose PR evidence",
        workflows: &["review"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &[
            "pr guidance, agent packet, before/after exposure, and receipt artifacts",
        ],
        outputs: CommandOutputs {
            default: Some("target/ripr/reports/test-oracle-assistant-proof.json"),
            optional: &["target/ripr/reports/test-oracle-assistant-proof.md"],
        },
        state_target: None,
        json_support: true,
        example: "ripr assistant-loop proof --pr-guidance target/ripr/review/comments.json --agent-packet target/ripr/workflow/agent-brief.json --before target/ripr/pilot/repo-exposure.json --after target/ripr/pilot/after.repo-exposure.json --receipt target/ripr/reports/agent-receipt.json",
        next_routes: &["assistant-loop health", "pr-review front-panel"],
        stop_states: &["explicit refusal on malformed inputs"],
        limitations: "a composition over named artifacts; it does not rerun the assistant loop.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:assistant-loop.health",
        summary: "Project assistant-loop health from one or more proof artifacts.",
        task: "Compose PR evidence",
        workflows: &["review"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &["assistant proof JSON (--proof, repeatable)"],
        outputs: CommandOutputs {
            default: Some("target/ripr/reports/assistant-loop-health.json"),
            optional: &["target/ripr/reports/assistant-loop-health.md"],
        },
        state_target: None,
        json_support: true,
        example: "ripr assistant-loop health --proof target/ripr/reports/test-oracle-assistant-proof.json",
        next_routes: &["pr-review front-panel"],
        stop_states: &["explicit refusal on malformed inputs"],
        limitations: "health counts stay advisory; it does not gate the assistant loop.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:first-pr",
        summary: "Compose the start-here PR evidence bundle from existing artifacts.",
        task: "Compose PR evidence",
        workflows: &["pr-evidence"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &[
            "existing check-output, gap-ledger, first-action, review-comments, agent-packet, or gate-decision artifacts",
        ],
        outputs: CommandOutputs {
            default: Some("target/ripr/reports/start-here.json"),
            optional: &["target/ripr/reports/start-here.md"],
        },
        state_target: None,
        json_support: true,
        example: "ripr first-pr --root . --base BASE --head HEAD",
        next_routes: &["agent repair", "reports index"],
        stop_states: &["--check fails when the composed bundle is stale"],
        limitations: "projection-only composition (#1572); it does not run analysis or repair a gap.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:compat.start-here",
        summary: "Compatibility alias for first-pr; compose the same start-here bundle.",
        task: "Compose PR evidence",
        workflows: &["pr-evidence"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &["the same artifact inputs as first-pr"],
        outputs: CommandOutputs {
            default: Some("target/ripr/reports/start-here.json"),
            optional: &["target/ripr/reports/start-here.md"],
        },
        state_target: None,
        json_support: true,
        example: "ripr start-here [same options as first-pr]",
        next_routes: &["first-pr"],
        stop_states: &["--check fails when the composed bundle is stale"],
        limitations: "same composition as first-pr; kept for existing docs and scripts during the rename.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:first-action",
        summary: "Project the first useful action from review and evidence artifacts.",
        task: "Compose PR evidence",
        workflows: &["review"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &["one of pr-guidance, assistant-proof, gap-ledger, or ledger artifacts"],
        outputs: CommandOutputs {
            default: Some("target/ripr/reports/first-useful-action.json"),
            optional: &["target/ripr/reports/first-useful-action.md"],
        },
        state_target: None,
        json_support: true,
        example: "ripr first-action --pr-guidance target/ripr/review/comments.json",
        next_routes: &["agent repair", "pr-review front-panel"],
        stop_states: &["no-action is an explicit terminal state"],
        limitations: "names repair one gap, regenerate an artifact, or stop; it does not execute the action.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:reports",
        summary: "Show the reports surface.",
        task: "Inspect one change",
        workflows: &["reports"],
        operation: CommandOperation::ReadOnly,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &[],
        outputs: CommandOutputs {
            default: None,
            optional: &[],
        },
        state_target: None,
        json_support: false,
        example: "ripr reports",
        next_routes: &["reports index", "reports gap-ledger"],
        stop_states: &[],
        limitations: "prints the reports surface; index, gap-ledger, and the audits carry the behavior.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:reports.index",
        summary: "Index the reports and review directories into one manifest.",
        task: "Inspect one change",
        workflows: &["reports"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &["--reports-dir and --review-dir trees"],
        outputs: CommandOutputs {
            default: Some("target/ripr/reports/index.json"),
            optional: &["target/ripr/reports/index.md"],
        },
        state_target: None,
        json_support: true,
        example: "ripr reports index",
        next_routes: &["reports gap-ledger", "pr-summary"],
        stop_states: &["explicit refusal on unreadable inputs"],
        limitations: "an index of what exists; it does not judge report freshness.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:reports.ci-summary",
        summary: "Print the generated CI workflow's step summary from existing artifacts.",
        task: "Compose PR evidence",
        workflows: &["reports", "pr-evidence"],
        operation: CommandOperation::ReadOnly,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &["target/ripr and target/ci artifacts under --root"],
        outputs: CommandOutputs {
            default: None,
            optional: &[],
        },
        state_target: None,
        json_support: false,
        example: "ripr reports ci-summary --root . --base-ref main >> \"$GITHUB_STEP_SUMMARY\"",
        next_routes: &["reports index", "first-pr"],
        stop_states: &["a missing or malformed artifact prints its regeneration route"],
        limitations: "renders what earlier steps wrote; it does not rerun analysis or decide pass/fail.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:reports.gap-ledger",
        summary: "Aggregate records, repo exposure, or check output into the gap decision ledger.",
        task: "Compose PR evidence",
        workflows: &["reports", "pr-evidence"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &["--records, --repo-exposure, or --check-output artifact"],
        outputs: CommandOutputs {
            default: Some("target/ripr/reports/gap-decision-ledger.json"),
            optional: &["target/ripr/reports/gap-decision-ledger.md"],
        },
        state_target: None,
        json_support: true,
        example: "ripr reports gap-ledger --check-output target/ripr/reports/check.json",
        next_routes: &["plus", "swarm queue", "receipt write"],
        stop_states: &["explicit refusal on malformed inputs"],
        limitations: "aggregation over named inputs; it does not rerun analysis.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:reports.ts-limitations",
        summary: "Project TypeScript limitation counts from a check output artifact.",
        task: "Inspect one change",
        workflows: &["reports", "calibration"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &["existing ripr check --json output (--check-output)"],
        outputs: CommandOutputs {
            default: Some("target/ripr/reports/typescript-limitations.json"),
            optional: &["target/ripr/reports/typescript-limitations.md"],
        },
        state_target: None,
        json_support: true,
        example: "ripr reports ts-limitations --check-output target/ripr/reports/check.json",
        next_routes: &["reports index"],
        stop_states: &["explicit refusal on malformed inputs"],
        limitations: "counts limitations in one artifact; it does not re-analyze TypeScript.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:reports.ts-false-actionable",
        summary: "Audit the TypeScript false-actionable corpus.",
        task: "Inspect one change",
        workflows: &["reports", "calibration"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &["false-actionable corpus JSON (--corpus)"],
        outputs: CommandOutputs {
            default: Some("target/ripr/reports/typescript-false-actionable-audit.json"),
            optional: &["target/ripr/reports/typescript-false-actionable-audit.md"],
        },
        state_target: None,
        json_support: true,
        example: "ripr reports ts-false-actionable --corpus target/ripr/reports/typescript-false-actionable-corpus.json",
        next_routes: &["reports index"],
        stop_states: &["explicit refusal on malformed inputs"],
        limitations: "audits the named corpus; it does not widen the corpus.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:calibrate",
        summary: "Show the calibration surface.",
        task: "Inspect one change",
        workflows: &["calibration"],
        operation: CommandOperation::ReadOnly,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &[],
        outputs: CommandOutputs {
            default: None,
            optional: &[],
        },
        state_target: None,
        json_support: false,
        example: "ripr calibrate",
        next_routes: &["calibrate cargo-mutants"],
        stop_states: &[],
        limitations: "prints the calibration surface; cargo-mutants carries the behavior.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:calibrate.cargo-mutants",
        summary: "Calibrate static exposure against a cargo-mutants output JSON.",
        task: "Inspect one change",
        workflows: &["calibration"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &[
            "cargo-mutants output JSON (--mutants-json)",
            "ripr repo-exposure JSON (--repo-exposure-json)",
        ],
        outputs: CommandOutputs {
            default: None,
            optional: &[
                "--out PATH rendered calibration report (--format md|json; stdout when --out is omitted)",
            ],
        },
        state_target: None,
        json_support: true,
        example: "ripr calibrate cargo-mutants --mutants-json PATH --repo-exposure-json PATH",
        next_routes: &["evidence-health", "reports index"],
        stop_states: &["explicit refusal on malformed inputs"],
        limitations: "reads mutation output produced elsewhere; it does not run mutants.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:receipt",
        summary: "Show the receipt surface.",
        task: "Repair a selected Rust gap",
        workflows: &["repair-loop"],
        operation: CommandOperation::ReadOnly,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &[],
        outputs: CommandOutputs {
            default: None,
            optional: &[],
        },
        state_target: None,
        json_support: false,
        example: "ripr receipt",
        next_routes: &["receipt write", "receipt check"],
        stop_states: &[],
        limitations: "prints the receipt surface; write and check carry the behavior.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:receipt.write",
        summary: "Author the canonical repair receipt for one completed attempt.",
        task: "Repair a selected Rust gap",
        workflows: &["repair-loop"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &["--gap identity", "--verify-command and --status outcomes"],
        outputs: CommandOutputs {
            default: Some("target/ripr/receipts/<canonical_gap_id>.json (--out overrides)"),
            optional: &[],
        },
        state_target: None,
        json_support: true,
        example: "ripr receipt write --gap <canonical_gap_id> --verify-command \"cargo test\" --status passed",
        next_routes: &["receipt check", "agent repair"],
        stop_states: &[
            "fail-closed refusal on missing gap, verify-command, or status",
            "--current-head mismatch against the actual HEAD is rejected when --root is provided",
        ],
        limitations: "a receipt records what ran; it does not prove runtime outcomes or gate approval.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:receipt.check",
        summary: "Validate a receipt JSON against structure and the live gap set.",
        task: "Repair a selected Rust gap",
        workflows: &["repair-loop"],
        operation: CommandOperation::ReadOnly,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &["receipt JSON (--path, or resolved via --gap)"],
        outputs: CommandOutputs {
            default: None,
            optional: &[],
        },
        state_target: None,
        json_support: true,
        example: "ripr receipt check --path PATH",
        next_routes: &["receipt write", "reports gap-ledger"],
        stop_states: &["orphan_receipt and receipt_gap_mismatch exit non-zero"],
        limitations: "structural and ledger cross-reference only; without --ledger the cross-reference stays not_available.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:feedback",
        summary: "Show the feedback surface.",
        task: "Record result usefulness",
        workflows: &["feedback"],
        operation: CommandOperation::ReadOnly,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &[],
        outputs: CommandOutputs {
            default: None,
            optional: &[],
        },
        state_target: None,
        json_support: false,
        example: "ripr feedback",
        next_routes: &["feedback record", "feedback export"],
        stop_states: &[],
        limitations: "prints the feedback surface; record and export carry the behavior.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:feedback.record",
        summary: "Record one local usefulness-feedback receipt.",
        task: "Record result usefulness",
        workflows: &["feedback"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &[
            "--snapshot identity",
            "optional --item, --attempt, and --receipt identities",
        ],
        outputs: CommandOutputs {
            default: Some("target/ripr/feedback/<idempotency-key>.json"),
            optional: &[],
        },
        state_target: None,
        json_support: true,
        example: "ripr feedback record --snapshot ID --reason CODE",
        next_routes: &["feedback export"],
        stop_states: &[
            "fail-closed refusal on known secret patterns in --note",
            "same key with a different payload is a conflict",
        ],
        limitations: "feedback changes no classification, policy, or gate state; silence is not a vote.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:feedback.export",
        summary: "Join usefulness-feedback receipts onto route-quality rows.",
        task: "Record result usefulness",
        workflows: &["feedback"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &[
            "target/ripr/feedback/*.json receipts",
            "existing route-quality JSON (--route-quality)",
        ],
        outputs: CommandOutputs {
            default: None,
            optional: &["--out PATH join JSON (stdout when --out is omitted)"],
        },
        state_target: None,
        json_support: true,
        example: "ripr feedback export --root .",
        next_routes: &["reports index"],
        stop_states: &[
            "missing route-quality input reports unmatched receipts without inventing movement",
        ],
        limitations: "keeps objective counts separate from subjective usefulness; unreviewed states are counts, not percentages.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:agent",
        summary: "Show the agent surface.",
        task: "Repair a selected Rust gap",
        workflows: &["repair-loop"],
        operation: CommandOperation::ReadOnly,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &[],
        outputs: CommandOutputs {
            default: None,
            optional: &[],
        },
        state_target: None,
        json_support: false,
        example: "ripr agent",
        next_routes: &["agent repair", "agent status"],
        stop_states: &[],
        limitations: "prints the agent surface; repair and status carry the loop.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:agent.repair",
        summary: "Run the before/edit/after repair transaction for one named seam.",
        task: "Repair a selected Rust gap",
        workflows: &["repair-loop"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Analysis,
        effects: ANALYSIS_RUNNER,
        primary_inputs: &[
            "--seam-id or --attempt identity",
            "the authorized verify route for the after phase (--verify-authorized)",
        ],
        outputs: CommandOutputs {
            default: Some("target/ripr/workflow/ (before and after snapshots)"),
            optional: &[],
        },
        state_target: None,
        json_support: false,
        example: "ripr agent repair --root . --seam-id ID --phase before",
        next_routes: &["agent status", "receipt write"],
        stop_states: &[
            "--phase verify refuses without --verify-authorized and a matching authority",
        ],
        limitations: "before and after phases record static snapshots and the verify phase composes `ripr agent verify` snapshot comparison; ripr never compiles or runs tests itself (test execution stays outside ripr). No network, no mutation, no source edits by ripr itself.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:agent.status",
        summary: "Show the local repair-loop state for one workspace.",
        task: "Resume a repair",
        workflows: &["repair-loop"],
        operation: CommandOperation::ReadOnly,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &["target/ripr/workflow state"],
        outputs: CommandOutputs {
            default: None,
            optional: &[],
        },
        state_target: None,
        json_support: true,
        example: "ripr agent status --root .",
        next_routes: &["agent repair", "agent review-summary"],
        stop_states: &[],
        limitations: "reads workflow state; it does not resume or mutate the loop.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:agent.start",
        summary: "Write a source-edit-free workflow packet for one seam.",
        task: "Repair a selected Rust gap",
        workflows: &["repair-loop", "editor-agent"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &["one visible seam (--seam-id)"],
        outputs: CommandOutputs {
            default: Some("target/ripr/workflow/ (workflow.json, commands.md, agent-brief.json)"),
            optional: &[],
        },
        state_target: None,
        json_support: true,
        example: "ripr agent start --root . --seam-id ID",
        next_routes: &["agent brief", "agent packet"],
        stop_states: &[],
        limitations: "writes command templates only; it does not call an LLM API, generate tests, or edit files.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:agent.brief",
        summary: "Write a bounded coding-agent brief over the current diff or change.",
        task: "Repair a selected Rust gap",
        workflows: &["repair-loop", "editor-agent"],
        operation: CommandOperation::ReadOnly,
        cost: CommandCost::Analysis,
        effects: ANALYSIS_RUNNER,
        primary_inputs: &["the current diff (--diff, --base, --files, or --seam-id)"],
        outputs: CommandOutputs {
            default: None,
            optional: &[],
        },
        state_target: None,
        json_support: true,
        example: "ripr agent brief --root . --diff PATH --json",
        next_routes: &["agent packet", "agent repair"],
        stop_states: &[],
        limitations: "--json is required until a human brief surface exists.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:agent.packet",
        summary: "Write the bounded repair packet for one seam or gap.",
        task: "Repair a selected Rust gap",
        workflows: &["repair-loop", "editor-agent"],
        operation: CommandOperation::ReadOnly,
        cost: CommandCost::Analysis,
        effects: ANALYSIS_RUNNER,
        primary_inputs: &["--seam-id, or --gap-ledger with --gap-id"],
        outputs: CommandOutputs {
            default: None,
            optional: &[],
        },
        state_target: None,
        json_support: true,
        example: "ripr agent packet --root . --seam-id ID --json",
        next_routes: &["agent verify", "swarm queue"],
        stop_states: &[],
        limitations: "--json is required until a human packet surface exists.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:agent.card",
        summary: "Hand off one seam as the compact default repair card.",
        task: "Repair a selected Rust gap",
        workflows: &["repair-loop", "editor-agent"],
        operation: CommandOperation::ReadOnly,
        cost: CommandCost::Analysis,
        effects: ANALYSIS_RUNNER,
        primary_inputs: &["--seam-id"],
        outputs: CommandOutputs {
            default: None,
            optional: &[],
        },
        state_target: None,
        json_support: true,
        example: "ripr agent card --root . --seam-id ID",
        next_routes: &["agent packet", "agent repair"],
        stop_states: &[],
        limitations: "the complete canonical packet stays behind the explicit `ripr agent packet` route; the card never embeds it.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:agent.verify",
        summary: "Compare before and after exposure JSONs directly.",
        task: "Repair a selected Rust gap",
        workflows: &["repair-loop", "editor-agent"],
        operation: CommandOperation::ReadOnly,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &["before and after exposure JSONs"],
        outputs: CommandOutputs {
            default: None,
            optional: &[],
        },
        state_target: None,
        json_support: true,
        example: "ripr agent verify --root . --before before.json --after after.json --json",
        next_routes: &["agent verify-execute", "agent receipt"],
        stop_states: &[],
        limitations: "direct, no-network, no-write comparison; it does not run the verify command.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:agent.verify-execute",
        summary: "Execute the packet's authorized verify projection and record the result.",
        task: "Repair a selected Rust gap",
        workflows: &["repair-loop", "editor-agent"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Small,
        effects: VERIFY_PACKET_RUNNER,
        primary_inputs: &["repair packet JSON (--packet)"],
        outputs: CommandOutputs {
            default: Some("--result-json PATH (the recorded verify outcome)"),
            optional: &[],
        },
        state_target: None,
        json_support: true,
        example: "ripr agent verify-execute --root . --packet packet.json --result-json result.json --authorize --json",
        next_routes: &["agent receipt"],
        stop_states: &["refusal without --authorize and a matching authority"],
        limitations: "executes exactly one bounded `ripr agent verify --json` child projection over saved snapshots; it compiles nothing, runs no tests, and uses no network.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:agent.receipt",
        summary: "Legacy alias for receipt write over an agent verify result.",
        task: "Repair a selected Rust gap",
        workflows: &["repair-loop", "editor-agent"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &["agent verify JSON (--verify-json)", "--seam-id identity"],
        outputs: CommandOutputs {
            default: None,
            optional: &["--out PATH receipt JSON"],
        },
        state_target: None,
        json_support: true,
        example: "ripr agent receipt --root . --verify-json agent-verify.json --seam-id ID --json",
        next_routes: &["receipt write"],
        stop_states: &[],
        limitations: "legacy alias during the receipt transition (#1123); new emitters use receipt write.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:agent.review-summary",
        summary: "Summarize the local repair loop for review handoff.",
        task: "Repair a selected Rust gap",
        workflows: &["repair-loop", "editor-agent"],
        operation: CommandOperation::ReadOnly,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &["target/ripr/workflow state"],
        outputs: CommandOutputs {
            default: None,
            optional: &[],
        },
        state_target: None,
        json_support: true,
        example: "ripr agent review-summary --root .",
        next_routes: &["agent status", "pr-summary"],
        stop_states: &[],
        limitations: "read-only summary; it does not change loop state.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:swarm",
        summary: "Show the swarm surface.",
        task: "Repair a selected Rust gap",
        workflows: &["repair-loop"],
        operation: CommandOperation::ReadOnly,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &[],
        outputs: CommandOutputs {
            default: None,
            optional: &[],
        },
        state_target: None,
        json_support: false,
        example: "ripr swarm",
        next_routes: &["swarm queue", "swarm ingest"],
        stop_states: &[],
        limitations: "prints the swarm surface; queue and ingest carry the behavior.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:swarm.queue",
        summary: "Rank gap-ledger records into a bounded, assignable repair queue.",
        task: "Repair a selected Rust gap",
        workflows: &["repair-loop"],
        operation: CommandOperation::ReadOnly,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &["gap decision ledger JSON (--gap-ledger)"],
        outputs: CommandOutputs {
            default: None,
            optional: &[],
        },
        state_target: None,
        json_support: true,
        example: "ripr swarm queue --root .",
        next_routes: &["agent packet", "reports gap-ledger"],
        stop_states: &[
            "malformed ledgers and root mismatches emit blocked envelopes instead of queue rows",
        ],
        limitations: "queues only live-current assignable records; blocked candidates stay visible with their refresh route.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:swarm.ingest",
        summary: "Classify one external agent result for safe ingestion.",
        task: "Repair a selected Rust gap",
        workflows: &["repair-loop"],
        operation: CommandOperation::ReadOnly,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &["agent result JSON under --root (--result)"],
        outputs: CommandOutputs {
            default: None,
            optional: &[],
        },
        state_target: None,
        json_support: true,
        example: "ripr swarm ingest --root . --result target/ripr/workflow/agent-result.json",
        next_routes: &["agent status", "swarm queue"],
        stop_states: &["missing verify evidence is never classified as success"],
        limitations: "classification only; it does not trust or execute the result's claims.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:diff",
        summary: "Analyze the diff between two revisions and write the JSON result to stdout.",
        task: "Inspect one change",
        workflows: &["inspect-change"],
        operation: CommandOperation::ReadOnly,
        cost: CommandCost::Analysis,
        effects: ANALYSIS_RUNNER,
        primary_inputs: &["--base and --head revisions (--root . when omitted)"],
        outputs: CommandOutputs {
            default: None,
            optional: &[],
        },
        state_target: None,
        json_support: true,
        example: "ripr diff --root . --base REV --head HEAD --json",
        next_routes: &["check", "explain"],
        stop_states: &["explicit refusal on unresolvable revisions"],
        limitations: "delivers JSON on stdout; redirect to a file when a report artifact is needed.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:check",
        summary: "Analyze the current diff and name the top actionable gap.",
        task: "Inspect one change",
        workflows: &["inspect-change"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Analysis,
        effects: ANALYSIS_RUNNER,
        primary_inputs: &["the current diff (--base, --worktree, or --diff PATH)"],
        outputs: CommandOutputs {
            default: None,
            optional: &["--write-artifact PATH reusable findings artifact"],
        },
        state_target: None,
        json_support: true,
        example: "ripr check --base REV",
        next_routes: &["explain", "context", "agent repair"],
        stop_states: &["explicit refusal when no diff can be resolved"],
        limitations: "writes results to stdout by convention; shell redirection to target/ripr/reports/check.json is the report path and --write-artifact PATH also saves the reusable findings artifact.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:explain",
        summary: "Explain one finding against the current diff.",
        task: "Inspect a finding",
        workflows: &["inspect-change"],
        operation: CommandOperation::ReadOnly,
        cost: CommandCost::Analysis,
        effects: ANALYSIS_RUNNER,
        primary_inputs: &[
            "finding id or file:line",
            "the current diff (--base or --diff)",
        ],
        outputs: CommandOutputs {
            default: None,
            optional: &[],
        },
        state_target: None,
        json_support: false,
        example: "ripr explain ABC123 --base REV",
        next_routes: &["context", "check"],
        stop_states: &[],
        limitations: "recomputes analysis context for the finding; it does not change analysis scope.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:context",
        summary: "Collect the tests and seams related to one finding.",
        task: "Hand off a finding",
        workflows: &["inspect-change"],
        operation: CommandOperation::ReadOnly,
        cost: CommandCost::Analysis,
        effects: ANALYSIS_RUNNER,
        primary_inputs: &[
            "finding id or file:line (--at)",
            "the current diff (--base or --diff)",
        ],
        outputs: CommandOutputs {
            default: None,
            optional: &[],
        },
        state_target: None,
        json_support: true,
        example: "ripr context --at <finding-id>",
        next_routes: &["explain", "agent repair"],
        stop_states: &[],
        limitations: "bounded by --max-related-tests; it is context collection, not a fix.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:doctor",
        summary: "Probe the local tool versions and config this workspace needs.",
        task: "Diagnose setup",
        workflows: &["setup"],
        operation: CommandOperation::ReadOnly,
        cost: CommandCost::Small,
        effects: TOOL_PROBES,
        primary_inputs: &["the selected workspace root"],
        outputs: CommandOutputs {
            default: None,
            optional: &[],
        },
        state_target: None,
        json_support: true,
        example: "ripr doctor",
        next_routes: &["init", "config validate"],
        stop_states: &["missing tools print as skipped with the reason and do not fail the run"],
        limitations: "reports environment readiness; it does not run verification.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:lsp",
        summary: "Run the experimental language server over stdio until the client disconnects.",
        task: "Work in an editor",
        workflows: &["editor-agent"],
        operation: CommandOperation::ReadOnly,
        cost: CommandCost::Analysis,
        effects: ANALYSIS_RUNNER,
        primary_inputs: &["the opened workspace"],
        outputs: CommandOutputs {
            default: None,
            optional: &[],
        },
        state_target: None,
        json_support: false,
        example: "ripr lsp --stdio",
        next_routes: &["mcp", "doctor"],
        stop_states: &["server exits when the client disconnects"],
        limitations: "on-demand refresh re-runs analysis; merely starting the server is immediate.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:cache",
        summary: "Show the cache surface.",
        task: "Diagnose setup",
        workflows: &["setup"],
        operation: CommandOperation::ReadOnly,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &[],
        outputs: CommandOutputs {
            default: None,
            optional: &[],
        },
        state_target: None,
        json_support: false,
        example: "ripr cache",
        next_routes: &["cache status", "cache clear"],
        stop_states: &[],
        limitations: "prints the cache surface; status and clear carry the behavior.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:cache.status",
        summary: "Show cache occupancy and entry health.",
        task: "Diagnose setup",
        workflows: &["setup"],
        operation: CommandOperation::ReadOnly,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &["the ripr cache root"],
        outputs: CommandOutputs {
            default: None,
            optional: &[],
        },
        state_target: None,
        json_support: true,
        example: "ripr cache status --json",
        next_routes: &["cache clear"],
        stop_states: &[],
        limitations: "read-only; it does not evict entries.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:cache.clear",
        summary: "Evict ripr cache entries.",
        task: "Diagnose setup",
        workflows: &["setup"],
        operation: CommandOperation::StateChanging,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &["the ripr cache root"],
        outputs: CommandOutputs {
            default: None,
            optional: &["--dry-run prints the eviction plan without changing state"],
        },
        state_target: Some("the ripr cache root"),
        json_support: false,
        example: "ripr cache clear --dry-run",
        next_routes: &["cache status"],
        stop_states: &["refusal without --force when entries would be removed"],
        limitations: "--dry-run previews; real eviction requires --force.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:pr-summary",
        summary: "Write the PR evidence summary from existing artifacts.",
        task: "Compose PR evidence",
        workflows: &["pr-evidence"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &[
            "existing RIPR artifacts under --root",
            "optional before snapshot (--baseline)",
        ],
        outputs: CommandOutputs {
            default: Some("target/ripr/reports/pr-evidence-summary.json"),
            optional: &[
                "target/ripr/reports/pr-evidence-summary.md",
                "target/ripr/pr/summary.md",
            ],
        },
        state_target: None,
        json_support: true,
        example: "ripr pr-summary",
        next_routes: &["pr-evidence", "reports index"],
        stop_states: &["--check fails when the summary is stale"],
        limitations: "a projection over saved artifacts; it does not rerun analysis.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:annotations",
        summary: "Render review comments as GitHub Actions warning annotations.",
        task: "Compose PR evidence",
        workflows: &["pr-evidence"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &["comments JSON (default target/ripr/review/comments.json)"],
        outputs: CommandOutputs {
            default: Some("target/ripr/review/annotations.txt"),
            optional: &[],
        },
        state_target: None,
        json_support: false,
        example: "ripr annotations --check",
        next_routes: &["review-comments"],
        stop_states: &["a missing comments file yields empty output, not an error"],
        limitations: "renders annotation lines only; it does not publish them.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:pr-evidence",
        summary: "Write the diff-scoped PR evidence packet for one base and head.",
        task: "Compose PR evidence",
        workflows: &["pr-evidence"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Analysis,
        effects: ANALYSIS_RUNNER,
        primary_inputs: &["--base and --head revisions"],
        outputs: CommandOutputs {
            default: Some("target/ripr/pr/repo-exposure.json"),
            optional: &["target/ripr/pr/repo-exposure.md", "target/ripr/pr/pr.diff"],
        },
        state_target: None,
        json_support: true,
        example: "ripr pr-evidence --base origin/main",
        next_routes: &["pr-summary", "impacted-evidence"],
        stop_states: &["unresolvable revisions are an explicit error"],
        limitations: "diff-scoped and advisory; it does not post review comments or change gate semantics.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:impacted-evidence",
        summary: "Route later mutation work from PR evidence and labels.",
        task: "Compose PR evidence",
        workflows: &["pr-evidence"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &["PR evidence packet (--pr-evidence)", "optional PR labels"],
        outputs: CommandOutputs {
            default: Some("target/xtask/impacted-evidence/latest.json"),
            optional: &["target/xtask/impacted-evidence/latest.md"],
        },
        state_target: None,
        json_support: true,
        example: "ripr impacted-evidence --label ripr-targeted",
        next_routes: &["pr-evidence"],
        stop_states: &["--check fails when the outputs are stale"],
        limitations: "routes mutation work produced elsewhere; it does not run mutants.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:plus",
        summary: "Compose the repo-wide RIPR+ receipt from one existing artifact.",
        task: "Compose PR evidence",
        workflows: &["reports"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &["repo-exposure summary JSON or gap decision ledger"],
        outputs: CommandOutputs {
            default: Some("target/ripr/reports/ripr-plus.json"),
            optional: &["target/ripr/reports/ripr-plus.md"],
        },
        state_target: None,
        json_support: true,
        example: "ripr plus --gap-ledger target/ripr/reports/gap-decision-ledger.json",
        next_routes: &["reports gap-ledger", "reports index"],
        stop_states: &[
            "exit 2 when the artifact cannot be composed or --check cannot establish zero",
        ],
        limitations: "informational legacy composition; complete test-quality and current-candidate zero are not established.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:rerun",
        summary: "Re-evaluate static evidence affected by one edited Rust test.",
        task: "Repair a selected Rust gap",
        workflows: &["inspect-change", "repair-loop"],
        operation: CommandOperation::WritesArtifacts,
        cost: CommandCost::Analysis,
        effects: ANALYSIS_RUNNER,
        primary_inputs: &[
            "changed test path (--changed-test)",
            "optional before artifact (--before)",
        ],
        outputs: CommandOutputs {
            default: None,
            optional: &["--out PATH targeted-rerun report JSON (stdout when omitted)"],
        },
        state_target: None,
        json_support: true,
        example: "ripr rerun --changed-test PATH[::TEST_NODE]",
        next_routes: &["check", "receipt write"],
        stop_states: &["explicit refusal when the changed test cannot be resolved"],
        limitations: "recomputes static evidence only; it never compiles or runs the edited test. --out PATH saves the report JSON, stdout is the default surface.",
        not_applicable_reason: None,
    },
    CommandMetadata {
        id: "cmd:mcp",
        summary: "Serve the read-only MCP adapter over stdio until the client disconnects.",
        task: "Work in an editor",
        workflows: &["editor-agent"],
        operation: CommandOperation::ReadOnly,
        cost: CommandCost::Small,
        effects: READS_ONLY,
        primary_inputs: &["the selected workspace root"],
        outputs: CommandOutputs {
            default: None,
            optional: &[],
        },
        state_target: None,
        json_support: false,
        example: "ripr mcp --stdio",
        next_routes: &["lsp"],
        stop_states: &["server exits when the client disconnects"],
        limitations: "read-only adapter; it carries no edit or execute authority.",
        not_applicable_reason: None,
    },
];

/// The governed metadata table accessor, integrity-checked like the catalog.
pub(crate) fn metadata() -> &'static [CommandMetadata] {
    let violations = metadata_violations(catalog(), METADATA);
    debug_assert!(
        violations.is_empty(),
        "command metadata integrity failed: {violations:?}"
    );
    let _ = violations;
    METADATA
}

/// Look up the metadata row for a catalog entry.
pub(crate) fn metadata_for(entry: &CommandCatalogEntry) -> Option<&'static CommandMetadata> {
    METADATA.iter().find(|row| row.id == entry.id)
}

/// Fail-closed metadata violations (issue #4823 required controls 1-4).
pub(crate) fn metadata_violations(
    entries: &[CommandCatalogEntry],
    rows: &[CommandMetadata],
) -> Vec<String> {
    let mut violations = Vec::new();
    let mut ids = std::collections::BTreeMap::<&str, usize>::new();
    let path_set: std::collections::BTreeSet<&str> = entries.iter().map(|e| e.path).collect();

    for row in rows {
        *ids.entry(row.id).or_insert(0) += 1;
        let Some(entry) = entries.iter().find(|entry| entry.id == row.id) else {
            violations.push(format!("{} has no catalog entry", row.id));
            continue;
        };
        if let Some(reason) = row.not_applicable_reason {
            if reason.trim().is_empty() {
                violations.push(format!("{} not-applicable reason is blank", row.id));
            }
            continue;
        }
        violations.extend(described_row_violations(entry, row, &path_set));
    }

    for (id, count) in ids {
        if count > 1 {
            violations.push(format!("duplicate metadata identity {id:?}"));
        }
    }

    // Required control 1: every public-facing catalog row is described or
    // explicitly not applicable. Internal rows stay out of scope and must
    // remain hidden.
    for entry in entries {
        let needs_metadata = matches!(
            entry.class,
            CommandClass::Public | CommandClass::Compatibility | CommandClass::Advanced
        );
        if needs_metadata && !rows.iter().any(|row| row.id == entry.id) {
            violations.push(format!(
                "{} ({}) has no metadata and no not-applicable reason",
                entry.id, entry.path
            ));
        }
    }
    violations
}

fn described_row_violations(
    entry: &CommandCatalogEntry,
    row: &CommandMetadata,
    path_set: &std::collections::BTreeSet<&str>,
) -> Vec<String> {
    let mut violations = Vec::new();
    let label = format!("{} ({})", row.id, entry.path);

    if row.summary.trim().is_empty() || row.summary.lines().count() > 1 {
        violations.push(format!("{label} summary is empty or multi-line"));
    }
    for token in BANNED_CLAIM_TOKENS {
        if row.summary.contains(token) || row.limitations.contains(token) {
            violations.push(format!(
                "{label} uses runtime-mutation claim token {token:?}"
            ));
        }
    }
    if row.task.trim().is_empty() {
        violations.push(format!("{label} task is empty"));
    }
    for tag in row.workflows {
        if !WORKFLOW_TAGS.contains(tag) {
            violations.push(format!(
                "{label} workflow tag {tag:?} is not in WORKFLOW_TAGS"
            ));
        }
    }
    if row.limitations.trim().is_empty() {
        violations.push(format!("{label} limitations are empty"));
    }
    let expected_example_prefix = format!("ripr {}", entry.path);
    if !row.example.starts_with(&expected_example_prefix) {
        violations.push(format!(
            "{label} example {:?} does not start with {expected_example_prefix:?}",
            row.example
        ));
    }
    for route in row.next_routes {
        if !path_set.contains(route) {
            violations.push(format!(
                "{label} next route {route:?} is not a catalog path"
            ));
        }
    }

    // Required control 2: read-only rows declare no product writes; writing
    // rows cannot silently omit their output role.
    match row.operation {
        CommandOperation::ReadOnly => {
            if row.outputs.default.is_some() || !row.outputs.optional.is_empty() {
                violations.push(format!("{label} is read-only but declares product writes"));
            }
            if row.state_target.is_some() {
                violations.push(format!("{label} is read-only but declares a state target"));
            }
        }
        CommandOperation::WritesArtifacts => {
            if row.outputs.default.is_none() && row.outputs.optional.is_empty() {
                violations.push(format!(
                    "{label} writes artifacts but declares no output role"
                ));
            }
        }
        CommandOperation::StateChanging => {
            if row.state_target.is_none() {
                violations.push(format!("{label} changes state but omits its state target"));
            }
        }
    }

    // Required control 3: cost/side-effect facts stay inside the canonical
    // contract (#1572 vocabulary and the check --mode cost classes).
    let effects = row.effects;
    if effects.may_run_mutation {
        violations.push(format!(
            "{label} claims may_run_mutation; RIPR does not run mutants"
        ));
    }
    if row.cost == CommandCost::Small && effects.may_run_analysis {
        violations.push(format!(
            "{label} is projection-cost but claims analysis work"
        ));
    }
    if (effects.may_compile || effects.may_run_tests) && row.cost == CommandCost::Small {
        violations.push(format!(
            "{label} compiles or runs tests but claims projection-only cost"
        ));
    }
    if effects.may_run_tests && !effects.may_compile {
        violations.push(format!(
            "{label} runs tests without the compile the test run requires"
        ));
    }
    let _ = effects.may_use_network;
    violations
}

/// Human-projection agreement between the rendered help surfaces and the
/// typed tables (issue #4823 acceptance: concise help, exhaustive help,
/// inventory, and hierarchy documentation agree on command identity).
///
/// `help_all` must be the rendered `ripr help --all` text and `help` the
/// rendered default screen. Non-public rows must carry their class marker;
/// public rows must not.
#[cfg(test)]
pub(crate) fn human_projection_violations(help: &str, help_all: &str) -> Vec<String> {
    let entries = catalog();
    let mut violations = Vec::new();

    let paths: Vec<&'static str> = entries.iter().map(|entry| entry.path).collect();

    // The grouped per-command listing (Setup: .. What it does:) must name each
    // ordinary or advanced command exactly once and carry class markers. Route
    // mentions elsewhere (task map, quick start, start-here path) may repeat
    // and carry no markers, but every `ripr ` line must resolve to the catalog.
    let (listing, mentions) = help_all_command_lines(help_all, &paths);
    for line in help_all.lines() {
        let Some(rest) = line.strip_prefix("  ripr ") else {
            continue;
        };
        if longest_catalog_prefix(rest, &paths).is_none() {
            violations.push(format!("help --all line routes to unknown command: {line}"));
        }
    }
    let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    let mut seen_lines: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for (path, line) in &listing {
        seen.insert(path);
        // The grouped listing promises one line per command; the same path may
        // legitimately appear on distinct lines (agent repair phases, rerun
        // selectors), but an identical line twice is an inventory drift.
        if !seen_lines.insert(line) {
            violations.push(format!("help --all lists the identical line twice: {line}"));
        }
        let Some(entry) = entries.iter().find(|entry| entry.path == *path) else {
            violations.push(format!(
                "help --all listing has unknown command {path:?}: {line}"
            ));
            continue;
        };
        match entry.class {
            CommandClass::Public => {
                if line.contains("[advanced]") || line.contains("[compatibility]") {
                    violations.push(format!(
                        "public row {} carries a non-public class marker: {line}",
                        entry.path
                    ));
                }
            }
            CommandClass::Advanced => {
                if !line.ends_with("[advanced]") {
                    violations.push(format!(
                        "advanced row {} lacks its [advanced] marker: {line}",
                        entry.path
                    ));
                }
            }
            CommandClass::Compatibility => {
                if !line.ends_with("[compatibility]") {
                    violations.push(format!(
                        "compatibility row {} lacks its [compatibility] marker: {line}",
                        entry.path
                    ));
                }
            }
            CommandClass::Internal => {
                violations.push(format!(
                    "internal row {} appears in help --all: {line}",
                    entry.path
                ));
            }
        }
        if entry.discovery == DiscoveryPosture::Hidden
            || matches!(entry.relation, CommandRelation::Retired { .. })
        {
            violations.push(format!(
                "hidden or retired row {} appears in help --all: {line}",
                entry.path
            ));
        }
    }
    for (path, line) in &mentions {
        let Some(entry) = entries.iter().find(|entry| entry.path == *path) else {
            violations.push(format!(
                "help --all routes to unknown command {path:?}: {line}"
            ));
            continue;
        };
        if entry.discovery == DiscoveryPosture::Hidden
            || matches!(entry.relation, CommandRelation::Retired { .. })
        {
            violations.push(format!(
                "hidden or retired row {} appears in help --all: {line}",
                entry.path
            ));
        }
    }
    // Presence: every ordinary or advanced row is documented by its own line
    // or by a child line (family umbrellas such as `cache` dispatch to help
    // and are documented through `cache status` / `cache clear`). `help`
    // documents itself in the header and the default screen's `More:` lines.
    for entry in entries {
        let ordinary = entry.class == CommandClass::Public
            && entry.discovery == DiscoveryPosture::OrdinaryPublic
            && matches!(entry.relation, CommandRelation::Canonical);
        let advanced = entry.class == CommandClass::Advanced
            && matches!(entry.relation, CommandRelation::Canonical);
        if !(ordinary || advanced) {
            continue;
        }
        let covered_by_own_line = seen.contains(entry.path);
        let covered_by_child = seen
            .iter()
            .any(|listed| listed.starts_with(&format!("{} ", entry.path)));
        if entry.path != "help" && !covered_by_own_line && !covered_by_child {
            violations.push(format!("help --all omits {} ({})", entry.id, entry.path));
        }
    }

    // The bounded default screen routes only to catalog commands. Only the
    // structured blocks are parsed; prose may use "ripr" as the product name.
    for line in concise_command_lines(help) {
        let Some(rest) = line.split("ripr ").nth(1) else {
            continue;
        };
        if longest_catalog_prefix(rest, &paths).is_none() {
            violations.push(format!("default help routes to unknown command: {line}"));
        }
    }
    violations
}

/// Command-bearing lines of the bounded default screen: the "Try this first"
/// rows, the "What are you trying to do?" task rows, and the "More:" routes.
/// Everything else on the screen is prose or advisory boundary text.
#[cfg(test)]
fn concise_command_lines(help: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut in_block = false;
    for line in help.lines() {
        if line == "Try this first:" || line == "What are you trying to do?" || line == "More:" {
            in_block = true;
            continue;
        }
        if in_block {
            if line.trim().is_empty() {
                in_block = false;
                continue;
            }
            if line.contains("ripr ") {
                lines.push(line);
            }
        }
    }
    lines
}

/// Resolved command lines with their catalog path: the grouped per-command
/// listing and the surrounding route mentions.
#[cfg(test)]
type HelpAllLines<'a> = (Vec<(&'static str, &'a str)>, Vec<(&'static str, &'a str)>);

/// Split `help --all` command lines into the grouped per-command listing
/// (between the "Setup:" header and the "What it does:" boundary) and the
/// surrounding route mentions (task map, quick start, start-here path). Only
/// lines that resolve to a catalog path are returned; unresolvable `ripr `
/// lines are reported by the caller's own pass.
#[cfg(test)]
fn help_all_command_lines<'a>(help_all: &'a str, paths: &[&'static str]) -> HelpAllLines<'a> {
    let mut listing = Vec::new();
    let mut mentions = Vec::new();
    let mut in_listing = false;
    for line in help_all.lines() {
        if line == "Setup:" {
            in_listing = true;
            continue;
        }
        if line == "What it does:" {
            in_listing = false;
            continue;
        }
        let Some(rest) = line.strip_prefix("  ripr ") else {
            continue;
        };
        let Some(path) = longest_catalog_prefix(rest, paths) else {
            continue;
        };
        if in_listing {
            listing.push((path, line));
        } else {
            mentions.push((path, line));
        }
    }
    (listing, mentions)
}

#[cfg(test)]
fn longest_catalog_prefix<'a>(rest: &str, paths: &[&'a str]) -> Option<&'a str> {
    paths
        .iter()
        .filter(|path| rest.starts_with(**path))
        .max_by_key(|path| path.len())
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::help;

    /// The eight task rows the hierarchy documentation pins, kept in sync with
    /// `assert_doc_command_routes` in `crates/ripr/tests/cli_help_hierarchy.rs`.
    const PINNED_TASK_ROUTES: &[(&str, &str)] = &[
        ("Inspect one change", "check"),
        ("Explore the repository", "pilot"),
        ("Repair a selected Rust gap", "agent repair"),
        ("Resume a repair", "agent status"),
        ("Compose PR evidence", "first-pr"),
        ("Add advisory CI", "init"),
        ("Diagnose setup", "doctor"),
        ("Record result usefulness", "feedback record"),
    ];

    fn with_mutated_row(
        id: &str,
        mutate: impl FnOnce(&mut CommandMetadata),
    ) -> Result<Vec<CommandMetadata>, String> {
        let mut rows = METADATA.to_vec();
        let Some(row) = rows.iter_mut().find(|row| row.id == id) else {
            return Err(format!("fixture row {id} missing"));
        };
        mutate(row);
        Ok(rows)
    }

    fn expect_single_violation(rows: &[CommandMetadata], needle: &str) -> Result<(), String> {
        let violations = metadata_violations(catalog(), rows);
        if violations.len() == 1 && violations[0].contains(needle) {
            return Ok(());
        }
        Err(format!(
            "expected exactly one {needle:?} violation, got {violations:?}"
        ))
    }

    #[test]
    fn production_table_is_complete_and_consistent() -> Result<(), String> {
        let violations = metadata_violations(catalog(), metadata());
        if violations.is_empty() {
            return Ok(());
        }
        Err(format!("production metadata violations: {violations:?}"))
    }

    #[test]
    fn every_public_catalog_row_is_described() -> Result<(), String> {
        // Required control 1: no public-facing catalog row may sit between
        // "has metadata" and "explicitly not applicable" without a row.
        let mut undescribed = Vec::new();
        for entry in catalog() {
            let needs_metadata = matches!(
                entry.class,
                CommandClass::Public | CommandClass::Compatibility | CommandClass::Advanced
            );
            if needs_metadata && metadata_for(entry).is_none() {
                undescribed.push(entry.id);
            }
        }
        if undescribed.is_empty() {
            return Ok(());
        }
        Err(format!("undescribed public rows: {undescribed:?}"))
    }

    #[test]
    fn readonly_row_declaring_a_product_write_is_rejected() -> Result<(), String> {
        let rows = with_mutated_row("cmd:explain", |row| {
            row.outputs.default = Some("target/ripr/reports/explain.json");
        })?;
        expect_single_violation(&rows, "read-only but declares product writes")
    }

    #[test]
    fn write_claim_without_an_output_role_is_rejected() -> Result<(), String> {
        let rows = with_mutated_row("cmd:explain", |row| {
            row.operation = CommandOperation::WritesArtifacts;
        })?;
        expect_single_violation(&rows, "writes artifacts but declares no output role")
    }

    #[test]
    fn state_change_without_a_state_target_is_rejected() -> Result<(), String> {
        let rows = with_mutated_row("cmd:baseline.create", |row| {
            row.state_target = None;
        })?;
        expect_single_violation(&rows, "changes state but omits its state target")
    }

    #[test]
    fn mutation_claim_is_rejected_everywhere() -> Result<(), String> {
        let rows = with_mutated_row("cmd:check", |row| {
            row.effects.may_run_mutation = true;
        })?;
        expect_single_violation(&rows, "claims may_run_mutation")
    }

    #[test]
    fn projection_cost_with_analysis_work_is_rejected() -> Result<(), String> {
        let rows = with_mutated_row("cmd:check", |row| {
            row.cost = CommandCost::Small;
        })?;
        expect_single_violation(&rows, "projection-cost but claims analysis work")
    }

    #[test]
    fn test_run_without_its_compile_is_rejected() -> Result<(), String> {
        let rows = with_mutated_row("cmd:check", |row| {
            row.effects.may_run_tests = true;
        })?;
        expect_single_violation(&rows, "runs tests without the compile")
    }

    #[test]
    fn next_route_outside_the_catalog_is_rejected() -> Result<(), String> {
        let rows = with_mutated_row("cmd:check", |row| {
            row.next_routes = &["not-a-command"];
        })?;
        expect_single_violation(&rows, "is not a catalog path")
    }

    #[test]
    fn runtime_mutation_claim_token_in_summary_is_rejected() -> Result<(), String> {
        let rows = with_mutated_row("cmd:check", |row| {
            row.summary = "reports survived mutants"; // ripr-allow: static-language: contradiction fixture must name a prohibited token to prove the validator rejects it
        })?;
        expect_single_violation(&rows, "uses runtime-mutation claim token")
    }

    #[test]
    fn human_surfaces_agree_with_the_typed_tables() -> Result<(), String> {
        let (help, help_all) = help::discovery_surfaces();
        let violations = human_projection_violations(help, help_all);
        if violations.is_empty() {
            return Ok(());
        }
        Err(format!("human projection violations: {violations:?}"))
    }

    #[test]
    fn duplicated_help_all_listing_line_is_reported() -> Result<(), String> {
        // The same path may appear on distinct lines (agent repair phases,
        // rerun selectors); an identical line twice is inventory drift.
        let help_all = "Setup:\n  ripr check --base REV  Analyze the diff.\n  ripr check --base REV  Analyze the diff.\nWhat it does:\n";
        let violations = human_projection_violations("ripr", help_all);
        if violations
            .iter()
            .any(|violation| violation.contains("lists the identical line twice"))
        {
            return Ok(());
        }
        Err(format!(
            "expected a duplicate-line violation, got: {violations:?}"
        ))
    }

    #[test]
    fn hierarchy_doc_commands_resolve_to_catalog_rows() -> Result<(), String> {
        let doc = include_str!("../../../../docs/COMMAND_HIERARCHY.md");
        let paths: Vec<&'static str> = catalog().iter().map(|entry| entry.path).collect();
        let mut violations = Vec::new();
        // Odd segments of a backtick split are code spans.
        for segment in doc.split('`').skip(1).step_by(2) {
            let Some(rest) = segment.strip_prefix("ripr ") else {
                continue;
            };
            let Some(path) = longest_catalog_prefix(rest, &paths) else {
                violations.push(format!("doc span has unknown command: {segment}"));
                continue;
            };
            let Some(entry) = catalog().iter().find(|entry| entry.path == path) else {
                violations.push(format!("doc span resolved without entry: {segment}"));
                continue;
            };
            if metadata_for(entry).is_none() {
                violations.push(format!("doc command {} has no metadata row", entry.id));
            }
        }
        if violations.is_empty() {
            return Ok(());
        }
        Err(format!("hierarchy doc violations: {violations:?}"))
    }

    #[test]
    fn pinned_task_rows_match_the_hierarchy_doc_labels() -> Result<(), String> {
        let doc = include_str!("../../../../docs/COMMAND_HIERARCHY.md");
        for (label, path) in PINNED_TASK_ROUTES {
            if !doc.contains(label) {
                return Err(format!("hierarchy doc no longer pins task label {label:?}"));
            }
            let Some(entry) = catalog().iter().find(|entry| entry.path == *path) else {
                return Err(format!("catalog path {path:?} missing"));
            };
            let Some(row) = metadata_for(entry) else {
                return Err(format!("{} has no metadata row", entry.id));
            };
            if row.task != *label {
                return Err(format!(
                    "{} task {:?} does not match the doc label {label:?}",
                    entry.path, row.task
                ));
            }
        }
        Ok(())
    }
}
