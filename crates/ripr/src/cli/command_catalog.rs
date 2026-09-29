//! Typed public command catalog for RIPR CLI identity and alias parity.
//!
//! RIPR-SPEC-0184 / issue #4822 (command-discovery C1). This catalog owns
//! canonical command paths, aliases, and public/compatibility/advanced/internal
//! classification. Parser generation is not required: `CliCommand::from_parts`
//! keeps help/version dispatch, then resolves top-level spellings through this
//! catalog. Nested parsers stay in their owners; two-way tests keep them aligned.
//!
//! Out of scope for this slice (#4823/#4824/#4825): rich cost/side-effect
//! metadata, human-help rewrites, `help workflow`, and `help --json`.

use std::collections::{BTreeMap, BTreeSet};

const PARSER_TOP: &str = "cli::command";
const DISPATCH_TOP: &str = "cli::execute";
const HELP_TOP: &str = "cli::help";
const PARSER_AGENT: &str = "cli::agent";
const DISPATCH_AGENT: &str = "cli::commands::agent";
const HELP_AGENT: &str = "cli::help::agent";
const PARSER_COMMANDS: &str = "cli::commands";
const PARSER_CONFIG: &str = "cli::commands::config";
const PARSER_GATE: &str = "cli::commands::gate";
const PARSER_BASELINE: &str = "cli::commands::baseline";
const PARSER_RECEIPT: &str = "cli::commands::receipt";
const PARSER_FEEDBACK: &str = "cli::commands::feedback";
const PARSER_SWARM: &str = "cli::commands::swarm";
const PARSER_CACHE: &str = "cli::commands::cache";
const HELP_POLICY: &str = "cli::help::policy";
const HELP_REPORTS: &str = "cli::help::reports";
const HELP_SWARM: &str = "cli::help::swarm";
const HELP_PR: &str = "cli::help::pr";

#[derive(Clone, Copy)]
struct CommandOwners {
    parser: &'static str,
    dispatch: &'static str,
    help: &'static str,
}

const TOP: CommandOwners = CommandOwners {
    parser: PARSER_TOP,
    dispatch: DISPATCH_TOP,
    help: HELP_TOP,
};
const AGENT: CommandOwners = CommandOwners {
    parser: PARSER_AGENT,
    dispatch: DISPATCH_AGENT,
    help: HELP_AGENT,
};
const COMMANDS: CommandOwners = CommandOwners {
    parser: PARSER_COMMANDS,
    dispatch: DISPATCH_TOP,
    help: HELP_TOP,
};
const CONFIG: CommandOwners = CommandOwners {
    parser: PARSER_CONFIG,
    dispatch: DISPATCH_TOP,
    help: HELP_TOP,
};
const GATE: CommandOwners = CommandOwners {
    parser: PARSER_GATE,
    dispatch: DISPATCH_TOP,
    help: HELP_POLICY,
};
const BASELINE: CommandOwners = CommandOwners {
    parser: PARSER_BASELINE,
    dispatch: DISPATCH_TOP,
    help: HELP_POLICY,
};
const POLICY: CommandOwners = CommandOwners {
    parser: PARSER_COMMANDS,
    dispatch: DISPATCH_TOP,
    help: HELP_POLICY,
};
const PR: CommandOwners = CommandOwners {
    parser: PARSER_COMMANDS,
    dispatch: DISPATCH_TOP,
    help: HELP_PR,
};
const REPORTS: CommandOwners = CommandOwners {
    parser: PARSER_COMMANDS,
    dispatch: DISPATCH_TOP,
    help: HELP_REPORTS,
};
const RECEIPT: CommandOwners = CommandOwners {
    parser: PARSER_RECEIPT,
    dispatch: DISPATCH_TOP,
    help: HELP_TOP,
};
const FEEDBACK: CommandOwners = CommandOwners {
    parser: PARSER_FEEDBACK,
    dispatch: DISPATCH_TOP,
    help: HELP_TOP,
};
const SWARM: CommandOwners = CommandOwners {
    parser: PARSER_SWARM,
    dispatch: DISPATCH_TOP,
    help: HELP_SWARM,
};
const CACHE: CommandOwners = CommandOwners {
    parser: PARSER_CACHE,
    dispatch: DISPATCH_TOP,
    help: HELP_TOP,
};

/// Stable classification for a catalog row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommandClass {
    Public,
    Compatibility,
    Advanced,
    Internal,
}

impl CommandClass {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Compatibility => "compatibility",
            Self::Advanced => "advanced",
            Self::Internal => "internal",
        }
    }
}

/// Whether a row is listed in the ordinary public inventory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DiscoveryPosture {
    OrdinaryPublic,
    CompatibilityAlias,
    Advanced,
    Hidden,
}

impl DiscoveryPosture {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::OrdinaryPublic => "ordinary_public",
            Self::CompatibilityAlias => "compatibility_alias",
            Self::Advanced => "advanced",
            Self::Hidden => "hidden",
        }
    }
}

/// Replacement or retirement relation for a catalog row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommandRelation {
    Canonical,
    AliasOf(&'static str),
    Retired { replacement: Option<&'static str> },
}

/// Parser/dispatch family used after top-level spelling resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommandDispatch {
    Help,
    Init,
    Config,
    Pilot,
    Outcome,
    EvidenceHealth,
    ReviewComments,
    Gate,
    Baseline,
    Zero,
    Policy,
    PrLedger,
    PrComments,
    PrReview,
    CoverageGrip,
    AssistantLoop,
    FirstPr,
    FirstAction,
    Reports,
    Calibrate,
    Receipt,
    Feedback,
    Agent,
    Swarm,
    Diff,
    Check,
    Explain,
    Context,
    Doctor,
    Lsp,
    PrSummary,
    Annotations,
    PrEvidence,
    ImpactedEvidence,
    RiprPlus,
    Cache,
    Rerun,
    Mcp,
}

/// One governed command identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CommandCatalogEntry {
    pub(crate) id: &'static str,
    pub(crate) path: &'static str,
    pub(crate) aliases: &'static [&'static str],
    pub(crate) class: CommandClass,
    pub(crate) dispatch: CommandDispatch,
    pub(crate) parser_owner: &'static str,
    pub(crate) dispatch_owner: &'static str,
    pub(crate) help_owner: &'static str,
    pub(crate) discovery: DiscoveryPosture,
    pub(crate) relation: CommandRelation,
}

const fn row(
    id: &'static str,
    path: &'static str,
    class: CommandClass,
    dispatch: CommandDispatch,
    owners: CommandOwners,
    discovery: DiscoveryPosture,
) -> CommandCatalogEntry {
    CommandCatalogEntry {
        id,
        path,
        aliases: &[],
        class,
        dispatch,
        parser_owner: owners.parser,
        dispatch_owner: owners.dispatch,
        help_owner: owners.help,
        discovery,
        relation: CommandRelation::Canonical,
    }
}

const fn public_top(
    id: &'static str,
    path: &'static str,
    dispatch: CommandDispatch,
) -> CommandCatalogEntry {
    row(
        id,
        path,
        CommandClass::Public,
        dispatch,
        TOP,
        DiscoveryPosture::OrdinaryPublic,
    )
}

const fn nested(
    id: &'static str,
    path: &'static str,
    class: CommandClass,
    dispatch: CommandDispatch,
    owners: CommandOwners,
    discovery: DiscoveryPosture,
) -> CommandCatalogEntry {
    row(id, path, class, dispatch, owners, discovery)
}

const fn compat_alias(
    id: &'static str,
    path: &'static str,
    canonical: &'static str,
    dispatch: CommandDispatch,
) -> CommandCatalogEntry {
    CommandCatalogEntry {
        id,
        path,
        aliases: &[],
        class: CommandClass::Compatibility,
        dispatch,
        parser_owner: TOP.parser,
        dispatch_owner: TOP.dispatch,
        help_owner: TOP.help,
        discovery: DiscoveryPosture::CompatibilityAlias,
        relation: CommandRelation::AliasOf(canonical),
    }
}

const CATALOG: &[CommandCatalogEntry] = &[
    public_top("cmd:help", "help", CommandDispatch::Help),
    public_top("cmd:init", "init", CommandDispatch::Init),
    public_top("cmd:config", "config", CommandDispatch::Config),
    nested(
        "cmd:config.validate",
        "config validate",
        CommandClass::Public,
        CommandDispatch::Config,
        CONFIG,
        DiscoveryPosture::OrdinaryPublic,
    ),
    public_top("cmd:pilot", "pilot", CommandDispatch::Pilot),
    public_top("cmd:outcome", "outcome", CommandDispatch::Outcome),
    public_top(
        "cmd:evidence-health",
        "evidence-health",
        CommandDispatch::EvidenceHealth,
    ),
    public_top(
        "cmd:review-comments",
        "review-comments",
        CommandDispatch::ReviewComments,
    ),
    public_top("cmd:gate", "gate", CommandDispatch::Gate),
    nested(
        "cmd:gate.evaluate",
        "gate evaluate",
        CommandClass::Public,
        CommandDispatch::Gate,
        GATE,
        DiscoveryPosture::OrdinaryPublic,
    ),
    public_top("cmd:baseline", "baseline", CommandDispatch::Baseline),
    nested(
        "cmd:baseline.create",
        "baseline create",
        CommandClass::Public,
        CommandDispatch::Baseline,
        BASELINE,
        DiscoveryPosture::OrdinaryPublic,
    ),
    nested(
        "cmd:baseline.diff",
        "baseline diff",
        CommandClass::Public,
        CommandDispatch::Baseline,
        BASELINE,
        DiscoveryPosture::OrdinaryPublic,
    ),
    nested(
        "cmd:baseline.update",
        "baseline update",
        CommandClass::Public,
        CommandDispatch::Baseline,
        BASELINE,
        DiscoveryPosture::OrdinaryPublic,
    ),
    public_top("cmd:zero", "zero", CommandDispatch::Zero),
    nested(
        "cmd:zero.status",
        "zero status",
        CommandClass::Public,
        CommandDispatch::Zero,
        POLICY,
        DiscoveryPosture::OrdinaryPublic,
    ),
    public_top("cmd:policy", "policy", CommandDispatch::Policy),
    nested(
        "cmd:policy.readiness",
        "policy readiness",
        CommandClass::Public,
        CommandDispatch::Policy,
        POLICY,
        DiscoveryPosture::OrdinaryPublic,
    ),
    nested(
        "cmd:policy.operations",
        "policy operations",
        CommandClass::Public,
        CommandDispatch::Policy,
        POLICY,
        DiscoveryPosture::OrdinaryPublic,
    ),
    nested(
        "cmd:policy.history",
        "policy history",
        CommandClass::Public,
        CommandDispatch::Policy,
        POLICY,
        DiscoveryPosture::OrdinaryPublic,
    ),
    nested(
        "cmd:policy.promote",
        "policy promote",
        CommandClass::Public,
        CommandDispatch::Policy,
        POLICY,
        DiscoveryPosture::OrdinaryPublic,
    ),
    nested(
        "cmd:policy.preview-promote",
        "policy preview-promote",
        CommandClass::Public,
        CommandDispatch::Policy,
        POLICY,
        DiscoveryPosture::OrdinaryPublic,
    ),
    nested(
        "cmd:policy.waiver-aging",
        "policy waiver-aging",
        CommandClass::Public,
        CommandDispatch::Policy,
        POLICY,
        DiscoveryPosture::OrdinaryPublic,
    ),
    nested(
        "cmd:policy.suppression-health",
        "policy suppression-health",
        CommandClass::Public,
        CommandDispatch::Policy,
        POLICY,
        DiscoveryPosture::OrdinaryPublic,
    ),
    public_top("cmd:pr-ledger", "pr-ledger", CommandDispatch::PrLedger),
    nested(
        "cmd:pr-ledger.record",
        "pr-ledger record",
        CommandClass::Public,
        CommandDispatch::PrLedger,
        PR,
        DiscoveryPosture::OrdinaryPublic,
    ),
    public_top(
        "cmd:pr-comments",
        "pr-comments",
        CommandDispatch::PrComments,
    ),
    nested(
        "cmd:pr-comments.plan",
        "pr-comments plan",
        CommandClass::Public,
        CommandDispatch::PrComments,
        PR,
        DiscoveryPosture::OrdinaryPublic,
    ),
    public_top("cmd:pr-review", "pr-review", CommandDispatch::PrReview),
    nested(
        "cmd:pr-review.front-panel",
        "pr-review front-panel",
        CommandClass::Public,
        CommandDispatch::PrReview,
        PR,
        DiscoveryPosture::OrdinaryPublic,
    ),
    public_top(
        "cmd:coverage-grip",
        "coverage-grip",
        CommandDispatch::CoverageGrip,
    ),
    nested(
        "cmd:coverage-grip.frontier",
        "coverage-grip frontier",
        CommandClass::Public,
        CommandDispatch::CoverageGrip,
        PR,
        DiscoveryPosture::OrdinaryPublic,
    ),
    public_top(
        "cmd:assistant-loop",
        "assistant-loop",
        CommandDispatch::AssistantLoop,
    ),
    nested(
        "cmd:assistant-loop.proof",
        "assistant-loop proof",
        CommandClass::Public,
        CommandDispatch::AssistantLoop,
        PR,
        DiscoveryPosture::OrdinaryPublic,
    ),
    nested(
        "cmd:assistant-loop.health",
        "assistant-loop health",
        CommandClass::Public,
        CommandDispatch::AssistantLoop,
        PR,
        DiscoveryPosture::OrdinaryPublic,
    ),
    public_top("cmd:first-pr", "first-pr", CommandDispatch::FirstPr),
    compat_alias(
        "cmd:compat.start-here",
        "start-here",
        "first-pr",
        CommandDispatch::FirstPr,
    ),
    public_top(
        "cmd:first-action",
        "first-action",
        CommandDispatch::FirstAction,
    ),
    public_top("cmd:reports", "reports", CommandDispatch::Reports),
    nested(
        "cmd:reports.index",
        "reports index",
        CommandClass::Public,
        CommandDispatch::Reports,
        REPORTS,
        DiscoveryPosture::OrdinaryPublic,
    ),
    nested(
        "cmd:reports.gap-ledger",
        "reports gap-ledger",
        CommandClass::Public,
        CommandDispatch::Reports,
        REPORTS,
        DiscoveryPosture::OrdinaryPublic,
    ),
    nested(
        "cmd:reports.ts-limitations",
        "reports ts-limitations",
        CommandClass::Public,
        CommandDispatch::Reports,
        REPORTS,
        DiscoveryPosture::OrdinaryPublic,
    ),
    nested(
        "cmd:reports.ts-false-actionable",
        "reports ts-false-actionable",
        CommandClass::Public,
        CommandDispatch::Reports,
        REPORTS,
        DiscoveryPosture::OrdinaryPublic,
    ),
    public_top("cmd:calibrate", "calibrate", CommandDispatch::Calibrate),
    nested(
        "cmd:calibrate.cargo-mutants",
        "calibrate cargo-mutants",
        CommandClass::Public,
        CommandDispatch::Calibrate,
        COMMANDS,
        DiscoveryPosture::OrdinaryPublic,
    ),
    public_top("cmd:receipt", "receipt", CommandDispatch::Receipt),
    nested(
        "cmd:receipt.write",
        "receipt write",
        CommandClass::Public,
        CommandDispatch::Receipt,
        RECEIPT,
        DiscoveryPosture::OrdinaryPublic,
    ),
    nested(
        "cmd:receipt.check",
        "receipt check",
        CommandClass::Public,
        CommandDispatch::Receipt,
        RECEIPT,
        DiscoveryPosture::OrdinaryPublic,
    ),
    public_top("cmd:feedback", "feedback", CommandDispatch::Feedback),
    nested(
        "cmd:feedback.record",
        "feedback record",
        CommandClass::Public,
        CommandDispatch::Feedback,
        FEEDBACK,
        DiscoveryPosture::OrdinaryPublic,
    ),
    nested(
        "cmd:feedback.export",
        "feedback export",
        CommandClass::Public,
        CommandDispatch::Feedback,
        FEEDBACK,
        DiscoveryPosture::OrdinaryPublic,
    ),
    public_top("cmd:agent", "agent", CommandDispatch::Agent),
    nested(
        "cmd:agent.repair",
        "agent repair",
        CommandClass::Public,
        CommandDispatch::Agent,
        AGENT,
        DiscoveryPosture::OrdinaryPublic,
    ),
    nested(
        "cmd:agent.status",
        "agent status",
        CommandClass::Public,
        CommandDispatch::Agent,
        AGENT,
        DiscoveryPosture::OrdinaryPublic,
    ),
    nested(
        "cmd:agent.start",
        "agent start",
        CommandClass::Advanced,
        CommandDispatch::Agent,
        AGENT,
        DiscoveryPosture::Advanced,
    ),
    nested(
        "cmd:agent.brief",
        "agent brief",
        CommandClass::Advanced,
        CommandDispatch::Agent,
        AGENT,
        DiscoveryPosture::Advanced,
    ),
    nested(
        "cmd:agent.packet",
        "agent packet",
        CommandClass::Advanced,
        CommandDispatch::Agent,
        AGENT,
        DiscoveryPosture::Advanced,
    ),
    nested(
        "cmd:agent.verify",
        "agent verify",
        CommandClass::Advanced,
        CommandDispatch::Agent,
        AGENT,
        DiscoveryPosture::Advanced,
    ),
    nested(
        "cmd:agent.verify-execute",
        "agent verify-execute",
        CommandClass::Advanced,
        CommandDispatch::Agent,
        AGENT,
        DiscoveryPosture::Advanced,
    ),
    nested(
        "cmd:agent.receipt",
        "agent receipt",
        CommandClass::Advanced,
        CommandDispatch::Agent,
        AGENT,
        DiscoveryPosture::Advanced,
    ),
    nested(
        "cmd:agent.review-summary",
        "agent review-summary",
        CommandClass::Advanced,
        CommandDispatch::Agent,
        AGENT,
        DiscoveryPosture::Advanced,
    ),
    public_top("cmd:swarm", "swarm", CommandDispatch::Swarm),
    nested(
        "cmd:swarm.queue",
        "swarm queue",
        CommandClass::Public,
        CommandDispatch::Swarm,
        SWARM,
        DiscoveryPosture::OrdinaryPublic,
    ),
    nested(
        "cmd:swarm.ingest",
        "swarm ingest",
        CommandClass::Public,
        CommandDispatch::Swarm,
        SWARM,
        DiscoveryPosture::OrdinaryPublic,
    ),
    public_top("cmd:diff", "diff", CommandDispatch::Diff),
    public_top("cmd:check", "check", CommandDispatch::Check),
    public_top("cmd:explain", "explain", CommandDispatch::Explain),
    public_top("cmd:context", "context", CommandDispatch::Context),
    public_top("cmd:doctor", "doctor", CommandDispatch::Doctor),
    public_top("cmd:lsp", "lsp", CommandDispatch::Lsp),
    public_top("cmd:cache", "cache", CommandDispatch::Cache),
    nested(
        "cmd:cache.status",
        "cache status",
        CommandClass::Public,
        CommandDispatch::Cache,
        CACHE,
        DiscoveryPosture::OrdinaryPublic,
    ),
    nested(
        "cmd:cache.clear",
        "cache clear",
        CommandClass::Public,
        CommandDispatch::Cache,
        CACHE,
        DiscoveryPosture::OrdinaryPublic,
    ),
    public_top("cmd:pr-summary", "pr-summary", CommandDispatch::PrSummary),
    public_top(
        "cmd:annotations",
        "annotations",
        CommandDispatch::Annotations,
    ),
    public_top(
        "cmd:pr-evidence",
        "pr-evidence",
        CommandDispatch::PrEvidence,
    ),
    public_top(
        "cmd:impacted-evidence",
        "impacted-evidence",
        CommandDispatch::ImpactedEvidence,
    ),
    public_top("cmd:plus", "plus", CommandDispatch::RiprPlus),
    public_top("cmd:rerun", "rerun", CommandDispatch::Rerun),
    public_top("cmd:mcp", "mcp", CommandDispatch::Mcp),
];

/// The governed catalog of public command identity and aliases.
pub(crate) fn catalog() -> &'static [CommandCatalogEntry] {
    let violations = catalog_violations(CATALOG);
    debug_assert!(
        violations.is_empty(),
        "command catalog integrity failed: {violations:?}"
    );
    let _ = violations;
    CATALOG
}

/// Resolve a top-level spelling (canonical path or alias) to its catalog row.
pub(crate) fn resolve_top_level(spelling: &str) -> Option<&'static CommandCatalogEntry> {
    if spelling.contains(' ') {
        return None;
    }
    catalog().iter().find(|entry| {
        !entry.path.contains(' ') && (entry.path == spelling || entry.aliases.contains(&spelling))
    })
}

/// Canonical public paths that belong in the ordinary public inventory.
#[cfg(test)]
pub(crate) fn public_inventory_paths(entries: &[CommandCatalogEntry]) -> Vec<&'static str> {
    let mut paths: Vec<&'static str> = entries
        .iter()
        .filter(|entry| {
            entry.class == CommandClass::Public
                && entry.discovery == DiscoveryPosture::OrdinaryPublic
                && matches!(entry.relation, CommandRelation::Canonical)
        })
        .map(|entry| entry.path)
        .collect();
    paths.sort_unstable();
    paths
}

/// Top-level spellings used for typo suggestions.
pub(crate) fn typo_suggestable_spellings() -> Vec<&'static str> {
    typo_suggestable_spellings_from(catalog())
}

fn typo_suggestable_spellings_from(entries: &[CommandCatalogEntry]) -> Vec<&'static str> {
    let mut spellings = BTreeSet::new();
    for entry in entries {
        if !typo_suggestable(entry) {
            continue;
        }
        if !entry.path.contains(' ') {
            spellings.insert(entry.path);
        }
        for alias in entry.aliases {
            spellings.insert(*alias);
        }
    }
    spellings.into_iter().collect()
}

fn typo_suggestable(entry: &CommandCatalogEntry) -> bool {
    entry.class != CommandClass::Internal
        && !is_retired_relation(entry.relation)
        && entry.discovery != DiscoveryPosture::Hidden
}

fn is_retired_relation(relation: CommandRelation) -> bool {
    relation == CommandRelation::Retired { replacement: None }
        || matches!(
            relation,
            CommandRelation::Retired {
                replacement: Some(_)
            }
        )
}

/// Byte-stable normalized catalog text, independent of source-row order.
#[cfg(test)]
pub(crate) fn normalized_catalog_text(entries: &[CommandCatalogEntry]) -> String {
    let mut rows: Vec<&CommandCatalogEntry> = entries.iter().collect();
    rows.sort_by_key(|entry| (entry.path, entry.id));
    let mut out = String::new();
    for entry in rows {
        out.push_str(entry.id);
        out.push('\t');
        out.push_str(entry.path);
        out.push('\t');
        let mut aliases = entry.aliases.to_vec();
        aliases.sort_unstable();
        out.push_str(&aliases.join(","));
        out.push('\t');
        out.push_str(entry.class.as_str());
        out.push('\t');
        out.push_str(entry.discovery.as_str());
        out.push('\t');
        match entry.relation {
            CommandRelation::Canonical => out.push_str("canonical"),
            CommandRelation::AliasOf(canonical) => {
                out.push_str("alias_of:");
                out.push_str(canonical);
            }
            CommandRelation::Retired {
                replacement: Some(replacement),
            } => {
                out.push_str("retired:");
                out.push_str(replacement);
            }
            CommandRelation::Retired { replacement: None } => out.push_str("retired"),
        }
        out.push('\n');
    }
    out
}

/// Fail-closed catalog integrity violations.
pub(crate) fn catalog_violations(entries: &[CommandCatalogEntry]) -> Vec<String> {
    let mut violations = Vec::new();
    let mut ids = BTreeMap::<&str, usize>::new();
    let mut paths = BTreeMap::<&str, usize>::new();
    let mut spellings = BTreeMap::<&str, Vec<&str>>::new();
    let path_set: BTreeSet<&str> = entries.iter().map(|entry| entry.path).collect();

    for entry in entries {
        *ids.entry(entry.id).or_insert(0) += 1;
        *paths.entry(entry.path).or_insert(0) += 1;
        if entry.path.is_empty() || entry.path.trim() != entry.path || entry.path.contains("  ") {
            violations.push(format!(
                "catalog path {:?} is empty or not a normalized space-separated command path",
                entry.path
            ));
        }
        spellings.entry(entry.path).or_default().push(entry.id);
        for alias in entry.aliases {
            spellings.entry(*alias).or_default().push(entry.id);
            if path_set.contains(alias) {
                violations.push(format!(
                    "alias {alias:?} on {} collides with a canonical catalog path",
                    entry.id
                ));
            }
        }
        let expected = expected_discovery(entry);
        if entry.discovery != expected {
            violations.push(format!(
                "{} discovery {} does not match class {} relation {:?}",
                entry.id,
                entry.discovery.as_str(),
                entry.class.as_str(),
                expected
            ));
        }
        if let CommandRelation::AliasOf(canonical) = entry.relation {
            if !path_set.contains(canonical) {
                violations.push(format!(
                    "{} aliases unknown canonical path {canonical:?}",
                    entry.id
                ));
            }
            if canonical == entry.path {
                violations.push(format!("{} aliases itself", entry.id));
            }
        }
    }

    for (id, count) in ids {
        if count > 1 {
            violations.push(format!("duplicate catalog identity {id:?}"));
        }
    }
    for (path, count) in paths {
        if count > 1 {
            violations.push(format!("duplicate canonical path {path:?}"));
        }
    }
    for (spelling, owners) in spellings {
        if owners.len() > 1 {
            violations.push(format!(
                "alias or path collision on {spelling:?} owned by {owners:?}"
            ));
        }
    }

    violations.extend(alias_cycle_violations(entries));
    violations
}

fn expected_discovery(entry: &CommandCatalogEntry) -> DiscoveryPosture {
    match (entry.class, entry.relation) {
        (CommandClass::Internal, _) | (_, CommandRelation::Retired { .. }) => {
            DiscoveryPosture::Hidden
        }
        (_, CommandRelation::AliasOf(_)) | (CommandClass::Compatibility, _) => {
            DiscoveryPosture::CompatibilityAlias
        }
        (CommandClass::Public, CommandRelation::Canonical) => DiscoveryPosture::OrdinaryPublic,
        (CommandClass::Advanced, CommandRelation::Canonical) => DiscoveryPosture::Advanced,
    }
}

fn alias_cycle_violations(entries: &[CommandCatalogEntry]) -> Vec<String> {
    let mut by_path = BTreeMap::new();
    for entry in entries {
        by_path.insert(entry.path, entry);
    }
    let mut violations = Vec::new();
    for entry in entries {
        let CommandRelation::AliasOf(mut current) = entry.relation else {
            continue;
        };
        let mut seen = BTreeSet::from([entry.path]);
        loop {
            if !seen.insert(current) {
                violations.push(format!("alias cycle involving {} at {current:?}", entry.id));
                break;
            }
            match by_path.get(current).map(|row| row.relation) {
                Some(CommandRelation::AliasOf(next)) => current = next,
                Some(_) => break,
                None => break,
            }
        }
    }
    violations
}

/// Two-way coverage between catalog paths and parser-accepted paths.
#[cfg(test)]
pub(crate) fn two_way_path_violations(
    catalog_paths: &[&str],
    parser_paths: &[&str],
) -> Vec<String> {
    let catalog: BTreeSet<&str> = catalog_paths.iter().copied().collect();
    let parser: BTreeSet<&str> = parser_paths.iter().copied().collect();
    let mut violations = Vec::new();
    for path in catalog.difference(&parser) {
        violations.push(format!("catalog-only command path {path:?}"));
    }
    for path in parser.difference(&catalog) {
        violations.push(format!("parser-only command path {path:?}"));
    }
    violations
}

#[cfg(test)]
fn backtick_tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('`') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('`') else {
            break;
        };
        tokens.push(after[..end].to_string());
        rest = &after[end + 1..];
    }
    tokens
}

/// Parse the expected-subcommand list from a parser unknown-subcommand error.
#[cfg(test)]
pub(crate) fn expected_subcommands_from_unknown_error(
    message: &str,
) -> Result<Vec<String>, String> {
    let Some(idx) = message.find("expected `") else {
        return Err(format!(
            "unknown-subcommand error did not list expected tokens: {message}"
        ));
    };
    Ok(backtick_tokens(&message[idx..]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::agent::parse_agent_args;
    use crate::cli::command::CliCommand;
    use crate::cli::commands;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    fn entry_with(
        path: &'static str,
        class: CommandClass,
        discovery: DiscoveryPosture,
        relation: CommandRelation,
    ) -> CommandCatalogEntry {
        CommandCatalogEntry {
            id: "cmd:test",
            path,
            aliases: &[],
            class,
            dispatch: CommandDispatch::Check,
            parser_owner: PARSER_TOP,
            dispatch_owner: DISPATCH_TOP,
            help_owner: HELP_TOP,
            discovery,
            relation,
        }
    }

    #[test]
    fn production_catalog_is_well_formed() {
        let violations = catalog_violations(catalog());
        assert!(
            violations.is_empty(),
            "production catalog violations: {violations:?}"
        );
    }

    #[test]
    fn catalog_lookup_is_pure_static_data() {
        let rows = catalog();
        assert!(
            !rows.is_empty(),
            "the public command catalog must contain the current parser surface"
        );
        assert!(resolve_top_level("check").is_some());
        assert!(resolve_top_level("start-here").is_some());
        assert!(resolve_top_level("agent repair").is_none());
        assert!(resolve_top_level("unknown-command").is_none());
    }

    #[test]
    fn reordered_source_rows_are_byte_stable_when_normalized() {
        let production = normalized_catalog_text(catalog());
        let mut reversed: Vec<CommandCatalogEntry> = catalog().to_vec();
        reversed.reverse();
        assert_eq!(normalized_catalog_text(&reversed), production);
        let mid = reversed.len() / 3;
        reversed.rotate_left(mid);
        assert_eq!(normalized_catalog_text(&reversed), production);
    }

    #[test]
    fn typo_suggestions_derive_from_the_catalog() {
        let spellings = typo_suggestable_spellings();
        assert!(spellings.contains(&"check"));
        assert!(spellings.contains(&"start-here"));
        assert!(spellings.contains(&"help"));
        assert!(spellings.contains(&"mcp"));
        assert!(
            !spellings.iter().any(|spelling| spelling.contains(' ')),
            "typo suggestions stay top-level: {spellings:?}"
        );
    }

    #[test]
    fn catalog_retains_the_pre_c1_known_command_surface() {
        // Frozen snapshot of the `KNOWN_COMMANDS` list this slice replaced.
        // It is a regression control, not a second live authority.
        const PRE_C1_KNOWN_COMMANDS: &[&str] = &[
            "init",
            "config",
            "help",
            "pilot",
            "outcome",
            "evidence-health",
            "review-comments",
            "gate",
            "baseline",
            "zero",
            "policy",
            "pr-ledger",
            "pr-comments",
            "pr-review",
            "coverage-grip",
            "assistant-loop",
            "first-pr",
            "start-here",
            "first-action",
            "reports",
            "calibrate",
            "receipt",
            "feedback",
            "agent",
            "swarm",
            "diff",
            "check",
            "explain",
            "context",
            "doctor",
            "lsp",
            "cache",
            "pr-summary",
            "annotations",
            "pr-evidence",
            "impacted-evidence",
            "plus",
            "rerun",
            "mcp",
        ];
        let spellings = typo_suggestable_spellings();
        let missing: Vec<&str> = PRE_C1_KNOWN_COMMANDS
            .iter()
            .copied()
            .filter(|command| !spellings.contains(command))
            .collect();
        assert!(
            missing.is_empty(),
            "catalog typo surface omitted a previously parser-accepted command: {missing:?}"
        );
    }

    #[test]
    fn public_inventory_excludes_compatibility_and_advanced() {
        let public = public_inventory_paths(catalog());
        assert!(public.contains(&"first-pr"));
        assert!(public.contains(&"agent repair"));
        assert!(
            !public.contains(&"start-here"),
            "compatibility alias leaked into the ordinary public inventory: {public:?}"
        );
        assert!(
            !public.contains(&"agent brief"),
            "advanced command leaked into the ordinary public inventory: {public:?}"
        );
        assert!(public.windows(2).all(|pair| pair[0] <= pair[1]));
    }

    fn catalog_path(path: &str) -> Result<&'static CommandCatalogEntry, String> {
        catalog()
            .iter()
            .find(|entry| entry.path == path)
            .ok_or_else(|| format!("catalog missing path {path:?}"))
    }

    fn require_top_level(spelling: &str) -> Result<&'static CommandCatalogEntry, String> {
        resolve_top_level(spelling)
            .ok_or_else(|| format!("catalog missing top-level spelling {spelling:?}"))
    }

    #[test]
    fn nested_and_top_level_same_spelling_keep_distinct_identities() -> Result<(), String> {
        assert_ne!(
            catalog_path("receipt")?.id,
            catalog_path("agent receipt")?.id
        );
        assert_ne!(catalog_path("check")?.id, catalog_path("receipt check")?.id);
        assert_ne!(catalog_path("diff")?.id, catalog_path("baseline diff")?.id);
        Ok(())
    }

    #[test]
    fn start_here_parses_to_first_pr() -> Result<(), String> {
        let alias = require_top_level("start-here")?;
        let canonical = require_top_level("first-pr")?;
        assert_eq!(alias.dispatch, canonical.dispatch);
        assert_eq!(alias.class, CommandClass::Compatibility);
        assert_eq!(alias.relation, CommandRelation::AliasOf("first-pr"));
        assert_eq!(
            CliCommand::from_parts(Some("start-here"), Vec::new()),
            Ok(CliCommand::FirstPr(Vec::new()))
        );
        Ok(())
    }

    #[test]
    fn every_catalog_top_level_spelling_is_accepted_by_the_parser() {
        for entry in catalog() {
            if entry.path.contains(' ') {
                continue;
            }
            assert!(
                CliCommand::from_parts(Some(entry.path), Vec::new()).is_ok(),
                "catalog path {:?} is not accepted by from_parts",
                entry.path
            );
            for alias in entry.aliases {
                assert!(
                    CliCommand::from_parts(Some(alias), Vec::new()).is_ok(),
                    "catalog alias {alias:?} is not accepted by from_parts"
                );
            }
        }
    }

    #[test]
    fn two_way_top_level_parity_matches_parser_and_catalog() {
        let catalog_paths: Vec<&str> = catalog()
            .iter()
            .filter(|entry| !entry.path.contains(' '))
            .map(|entry| entry.path)
            .collect();
        let mut parser_paths = Vec::new();
        for path in &catalog_paths {
            if CliCommand::from_parts(Some(path), Vec::new()).is_ok() {
                parser_paths.push(*path);
            }
        }
        let violations = two_way_path_violations(&catalog_paths, &parser_paths);
        assert!(
            violations.is_empty(),
            "top-level parser/catalog drift: {violations:?}"
        );
    }

    #[test]
    fn parser_only_and_catalog_only_paths_fail_closed() {
        let catalog = ["check", "doctor"];
        let parser_only = two_way_path_violations(&catalog, &["check", "doctor", "ghost"]);
        assert!(
            parser_only.iter().any(|row| row.contains("parser-only")),
            "parser-only extra path must fail: {parser_only:?}"
        );
        let catalog_only =
            two_way_path_violations(&["check", "doctor", "ghost"], &["check", "doctor"]);
        assert!(
            catalog_only.iter().any(|row| row.contains("catalog-only")),
            "catalog-only extra path must fail: {catalog_only:?}"
        );
    }

    #[test]
    fn duplicate_alias_and_cycle_fail_closed() {
        let duplicate_alias = [
            CommandCatalogEntry {
                aliases: &["twin"],
                ..entry_with(
                    "alpha",
                    CommandClass::Public,
                    DiscoveryPosture::OrdinaryPublic,
                    CommandRelation::Canonical,
                )
            },
            CommandCatalogEntry {
                id: "cmd:beta",
                path: "beta",
                aliases: &["twin"],
                class: CommandClass::Public,
                dispatch: CommandDispatch::Doctor,
                parser_owner: PARSER_TOP,
                dispatch_owner: DISPATCH_TOP,
                help_owner: HELP_TOP,
                discovery: DiscoveryPosture::OrdinaryPublic,
                relation: CommandRelation::Canonical,
            },
        ];
        let duplicate = catalog_violations(&duplicate_alias);
        assert!(
            duplicate.iter().any(|row| row.contains("collision")),
            "duplicate alias must fail: {duplicate:?}"
        );

        let cycle = [
            CommandCatalogEntry {
                id: "cmd:a",
                relation: CommandRelation::AliasOf("b"),
                class: CommandClass::Compatibility,
                discovery: DiscoveryPosture::CompatibilityAlias,
                ..entry_with(
                    "a",
                    CommandClass::Compatibility,
                    DiscoveryPosture::CompatibilityAlias,
                    CommandRelation::AliasOf("b"),
                )
            },
            CommandCatalogEntry {
                id: "cmd:b",
                path: "b",
                aliases: &[],
                class: CommandClass::Compatibility,
                dispatch: CommandDispatch::Doctor,
                parser_owner: PARSER_TOP,
                dispatch_owner: DISPATCH_TOP,
                help_owner: HELP_TOP,
                discovery: DiscoveryPosture::CompatibilityAlias,
                relation: CommandRelation::AliasOf("a"),
            },
        ];
        let cycle_violations = catalog_violations(&cycle);
        assert!(
            cycle_violations.iter().any(|row| row.contains("cycle")),
            "alias cycle must fail: {cycle_violations:?}"
        );
    }

    #[test]
    fn internal_and_retired_rows_cannot_leak_into_public_or_typo_surfaces() {
        let leaked = [
            entry_with(
                "check",
                CommandClass::Public,
                DiscoveryPosture::OrdinaryPublic,
                CommandRelation::Canonical,
            ),
            CommandCatalogEntry {
                id: "cmd:internal.secret",
                path: "secret-internal",
                aliases: &[],
                class: CommandClass::Internal,
                dispatch: CommandDispatch::Doctor,
                parser_owner: PARSER_TOP,
                dispatch_owner: DISPATCH_TOP,
                help_owner: HELP_TOP,
                discovery: DiscoveryPosture::Hidden,
                relation: CommandRelation::Canonical,
            },
            CommandCatalogEntry {
                id: "cmd:retired.old",
                path: "old-command",
                aliases: &[],
                class: CommandClass::Internal,
                dispatch: CommandDispatch::Doctor,
                parser_owner: PARSER_TOP,
                dispatch_owner: DISPATCH_TOP,
                help_owner: HELP_TOP,
                discovery: DiscoveryPosture::Hidden,
                relation: CommandRelation::Retired {
                    replacement: Some("check"),
                },
            },
        ];
        assert!(catalog_violations(&leaked).is_empty());
        let public = public_inventory_paths(&leaked);
        assert_eq!(public, vec!["check"]);
        let typos = typo_suggestable_spellings_from(&leaked);
        assert_eq!(typos, vec!["check"]);
        assert!(!typos.contains(&"secret-internal"));
        assert!(!typos.contains(&"old-command"));
    }

    fn probe_family(
        family: &str,
        parse: impl Fn(&[String]) -> Result<(), String>,
    ) -> Result<Vec<String>, String> {
        match parse(&args(&["__catalog_probe__"])) {
            Err(error) => expected_subcommands_from_unknown_error(&error)
                .map_err(|parse_error| format!("{family}: {parse_error}")),
            Ok(()) => Err(format!(
                "{family} accepted an unknown nested probe instead of listing expected subcommands"
            )),
        }
    }

    fn catalog_children(family: &str) -> Vec<&'static str> {
        let prefix = format!("{family} ");
        catalog()
            .iter()
            .filter_map(|entry| entry.path.strip_prefix(&prefix))
            .filter(|child| !child.contains(' '))
            .collect()
    }

    #[test]
    fn nested_parser_families_have_two_way_parity_with_the_catalog() {
        let mut failures = Vec::new();
        type NestedParse = fn(&[String]) -> Result<(), String>;
        let families: &[(&str, NestedParse)] = &[
            ("config", commands::config),
            ("baseline", commands::baseline),
            ("zero", commands::zero),
            ("policy", commands::policy),
            ("pr-ledger", commands::pr_ledger),
            ("pr-comments", commands::pr_comments),
            ("pr-review", commands::pr_review),
            ("coverage-grip", commands::coverage_grip),
            ("assistant-loop", commands::assistant_loop),
            ("reports", commands::reports),
            ("calibrate", commands::calibrate),
            ("receipt", commands::receipt),
            ("feedback", commands::feedback),
            ("swarm", commands::swarm),
            ("cache", commands::cache),
        ];
        for (family, parse) in families {
            match probe_family(family, parse) {
                Ok(parser_children) => {
                    let catalog_children = catalog_children(family);
                    let parser_refs: Vec<&str> =
                        parser_children.iter().map(String::as_str).collect();
                    let violations = two_way_path_violations(&catalog_children, &parser_refs);
                    if !violations.is_empty() {
                        failures.push(format!("{family}: {violations:?}"));
                    }
                    for child in &catalog_children {
                        if let Err(error) = parse(&args(&[child, "--help"])) {
                            failures.push(format!("{family} {child} --help was rejected: {error}"));
                        }
                    }
                }
                Err(error) => failures.push(error),
            }
        }

        match probe_family("gate", |args| {
            commands::gate(args).map_err(|error| error.to_string())
        }) {
            Ok(parser_children) => {
                let catalog_children = catalog_children("gate");
                let parser_refs: Vec<&str> = parser_children.iter().map(String::as_str).collect();
                let violations = two_way_path_violations(&catalog_children, &parser_refs);
                if !violations.is_empty() {
                    failures.push(format!("gate: {violations:?}"));
                }
                for child in &catalog_children {
                    if let Err(error) =
                        commands::gate(&args(&[child, "--help"])).map_err(|error| error.to_string())
                    {
                        failures.push(format!("gate {child} --help was rejected: {error}"));
                    }
                }
            }
            Err(error) => failures.push(error),
        }

        match parse_agent_args(&args(&["__catalog_probe__"])) {
            Err(error) => match expected_subcommands_from_unknown_error(&error) {
                Ok(parser_children) => {
                    let catalog_children = catalog_children("agent");
                    let parser_refs: Vec<&str> =
                        parser_children.iter().map(String::as_str).collect();
                    let violations = two_way_path_violations(&catalog_children, &parser_refs);
                    if !violations.is_empty() {
                        failures.push(format!("agent: {violations:?}"));
                    }
                    for child in catalog_children {
                        if let Err(error) = parse_agent_args(&args(&[child, "--help"])) {
                            failures.push(format!("agent {child} --help was rejected: {error}"));
                        }
                    }
                }
                Err(error) => failures.push(error),
            },
            Ok(_) => failures.push(
                "agent accepted an unknown nested probe instead of listing expected subcommands"
                    .to_string(),
            ),
        }

        assert!(
            failures.is_empty(),
            "nested parser/catalog drift: {failures:?}"
        );
    }
}
