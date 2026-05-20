#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum XtaskCommand {
    Shape,
    FixPr,
    InstallHooks(Vec<String>),
    Commands,
    PrSummary,
    PrReady,
    Cockpit,
    PrTriageReport,
    GhPrStatus(Vec<String>),
    SuggestedFixes,
    Precommit,
    CheckPr,
    Fixtures(Option<String>),
    Goldens(Vec<String>),
    Metrics,
    TestOracleReport,
    TestEfficiencyReport,
    BadgeArtifacts,
    RepoBadgeArtifacts(Vec<String>),
    BadgeBasis(Vec<String>),
    RepoSeamInventory,
    RepoExposureReport,
    RepoExposureLatencyReport,
    EvidenceHealth,
    Lane1EvidenceAudit,
    EvidenceQualityScorecard,
    EvidenceQualityTrend(Vec<String>),
    ActionableGapOutcomes(Vec<String>),
    AgentSeamPackets(Option<String>),
    RiprSwarm(Vec<String>),
    LspCockpitReport,
    OperatorCockpitReport,
    ReleaseReadiness(Vec<String>),
    ReleaseServerArchive(Vec<String>),
    ReleaseServerManifest(Vec<String>),
    ReleaseUploadAssets(Vec<String>),
    TargetedTestOutcome(Vec<String>),
    MutationCalibration(Vec<String>),
    RecommendationCalibration(Vec<String>),
    SarifPolicy(Vec<String>),
    ImpactedEvidence(Vec<String>),
    RiprPr(Vec<String>),
    FirstPr(Vec<String>),
    RiprReviewComments(Vec<String>),
    RiprPrSummary(Vec<String>),
    RiprAnnotations(Vec<String>),
    UpdateBadgeEndpoints(Vec<String>),
    CheckBadgeEndpoints(Vec<String>),
    Dogfood,
    Critic,
    Goals(Vec<String>),
    Reports(Vec<String>),
    Receipts(Vec<String>),
    Worktree(Vec<String>),
    Specs(Vec<String>),
    GoldenDrift,
    CiFast,
    CiFull,
    CheckStaticLanguage,
    CheckNoPanicFamily(Vec<String>),
    CheckAllowAttributes,
    CheckLocalContext,
    CheckFilePolicy,
    RustConversionCandidates,
    CheckExecutableFiles,
    CheckWorkflows,
    CheckDroidReviewConfig,
    CheckSpecFormat,
    CheckSpecNumbering,
    CheckFixtureContracts,
    CheckTraceability,
    CheckCapabilities,
    CheckWorkspaceShape,
    CheckArchitecture,
    CheckPublicApi,
    CheckOutputContracts,
    CheckDocIndex,
    CheckReadmeState,
    MarkdownLinks,
    CheckCampaign,
    CheckPrShape,
    CheckGenerated,
    CheckCommandCatalog,
    CheckBadgeDiffPolicy,
    CheckGeneratedClean,
    CheckVerificationContracts(Vec<String>),
    CheckDependencies,
    CheckSupplyChain,
    CheckProcessPolicy,
    CheckNetworkPolicy,
    CheckLintPolicy,
    CheckCiLaneWhitelist,
    CheckProductCopy,
    CheckPositioningLanguage,
    CheckDocRoles,
    VscodeCompile,
    VscodePackage,
    VscodeTest,
    VscodeTestE2e,
    Package,
    PublishDryRun,
    Help(Vec<String>),
    Unknown(String),
}

impl XtaskCommand {
    pub(crate) fn parse(args: impl IntoIterator<Item = String>) -> Self {
        let mut args = args.into_iter();
        let Some(command) = args.next() else {
            return Self::Help(Vec::new());
        };
        let rest: Vec<String> = args.collect();
        match command.as_str() {
            "shape" => Self::Shape,
            "fix-pr" => Self::FixPr,
            "install-hooks" => Self::InstallHooks(rest),
            "commands" => Self::Commands,
            "pr-summary" => Self::PrSummary,
            "pr-ready" => Self::PrReady,
            "cockpit" => Self::Cockpit,
            "pr-triage-report" => Self::PrTriageReport,
            "gh-pr-status" => Self::GhPrStatus(rest),
            "suggested-fixes" => Self::SuggestedFixes,
            "precommit" => Self::Precommit,
            "check-pr" => Self::CheckPr,
            "fixtures" => Self::Fixtures(rest.first().cloned()),
            "goldens" => Self::Goldens(rest),
            "metrics" => Self::Metrics,
            "test-oracle-report" | "check-test-oracles" => Self::TestOracleReport,
            "test-efficiency-report" => Self::TestEfficiencyReport,
            "badge-artifacts" => Self::BadgeArtifacts,
            "repo-badge-artifacts" => Self::RepoBadgeArtifacts(rest),
            "badge-basis" => Self::BadgeBasis(rest),
            "repo-seam-inventory" => Self::RepoSeamInventory,
            "repo-exposure-report" => Self::RepoExposureReport,
            "repo-exposure-latency-report" => Self::RepoExposureLatencyReport,
            "evidence-health" => Self::EvidenceHealth,
            "lane1-evidence-audit" | "evidence-quality-audit" => Self::Lane1EvidenceAudit,
            "evidence-quality-scorecard" => Self::EvidenceQualityScorecard,
            "evidence-quality-trend" => Self::EvidenceQualityTrend(rest),
            "actionable-gap-outcomes" => Self::ActionableGapOutcomes(rest),
            "agent-seam-packets" => Self::AgentSeamPackets(rest.first().cloned()),
            "ripr-swarm" => Self::RiprSwarm(rest),
            "lsp-cockpit-report" => Self::LspCockpitReport,
            "operator-cockpit" | "operator-cockpit-report" => Self::OperatorCockpitReport,
            "release-readiness" => Self::ReleaseReadiness(rest),
            "release-server-archive" => Self::ReleaseServerArchive(rest),
            "release-server-manifest" => Self::ReleaseServerManifest(rest),
            "release-upload-assets" => Self::ReleaseUploadAssets(rest),
            "targeted-test-outcome" => Self::TargetedTestOutcome(rest),
            "mutation-calibration" => Self::MutationCalibration(rest),
            "recommendation-calibration" => Self::RecommendationCalibration(rest),
            "sarif-policy" => Self::SarifPolicy(rest),
            "impacted-evidence" => Self::ImpactedEvidence(rest),
            "ripr-pr" => Self::RiprPr(rest),
            "first-pr" => Self::FirstPr(rest),
            "ripr-review-comments" => Self::RiprReviewComments(rest),
            "ripr-pr-summary" => Self::RiprPrSummary(rest),
            "ripr-annotations" => Self::RiprAnnotations(rest),
            "badges" if rest.iter().any(|arg| arg == "--check") => Self::CheckBadgeEndpoints(rest),
            "badges" => Self::UpdateBadgeEndpoints(rest),
            "update-badge-endpoints" => Self::UpdateBadgeEndpoints(rest),
            "check-badge-endpoints" => Self::CheckBadgeEndpoints(rest),
            "dogfood" => Self::Dogfood,
            "critic" => Self::Critic,
            "goals" => Self::Goals(rest),
            "reports" => Self::Reports(rest),
            "receipts" => Self::Receipts(rest),
            "doctor" => Self::Worktree(vec!["doctor".to_string()]),
            "worktree" => Self::Worktree(rest),
            "specs" => Self::Specs(rest),
            "golden-drift" => Self::GoldenDrift,
            "ci-fast" => Self::CiFast,
            "ci-full" => Self::CiFull,
            "check-static-language" => Self::CheckStaticLanguage,
            "check-no-panic-family" => Self::CheckNoPanicFamily(rest),
            "check-allow-attributes" => Self::CheckAllowAttributes,
            "check-local-context" => Self::CheckLocalContext,
            "check-file-policy" => Self::CheckFilePolicy,
            "rust-conversion-candidates" => Self::RustConversionCandidates,
            "check-executable-files" => Self::CheckExecutableFiles,
            "check-workflows" => Self::CheckWorkflows,
            "check-droid-review-config" => Self::CheckDroidReviewConfig,
            "check-spec-format" => Self::CheckSpecFormat,
            "check-spec-numbering" => Self::CheckSpecNumbering,
            "check-fixture-contracts" => Self::CheckFixtureContracts,
            "check-traceability" | "check-spec-ids" | "check-behavior-manifest" => {
                Self::CheckTraceability
            }
            "check-capabilities" => Self::CheckCapabilities,
            "check-workspace-shape" => Self::CheckWorkspaceShape,
            "check-architecture" => Self::CheckArchitecture,
            "check-public-api" => Self::CheckPublicApi,
            "check-output-contracts" => Self::CheckOutputContracts,
            "check-doc-index" => Self::CheckDocIndex,
            "check-readme-state" => Self::CheckReadmeState,
            "markdown-links" => Self::MarkdownLinks,
            "check-campaign" | "check-goals" => Self::CheckCampaign,
            "check-pr-shape" => Self::CheckPrShape,
            "check-generated" => Self::CheckGenerated,
            "check-command-catalog" => Self::CheckCommandCatalog,
            "check-badge-diff-policy" => Self::CheckBadgeDiffPolicy,
            "check-generated-clean" => Self::CheckGeneratedClean,
            "check-verification-contracts" => Self::CheckVerificationContracts(rest),
            "check-dependencies" => Self::CheckDependencies,
            "check-supply-chain" => Self::CheckSupplyChain,
            "check-process-policy" => Self::CheckProcessPolicy,
            "check-network-policy" => Self::CheckNetworkPolicy,
            "check-lint-policy" => Self::CheckLintPolicy,
            "check-ci-lane-whitelist" => Self::CheckCiLaneWhitelist,
            "check-product-copy" => Self::CheckProductCopy,
            "check-positioning-language" => Self::CheckPositioningLanguage,
            "check-doc-roles" => Self::CheckDocRoles,
            "vscode-compile" => Self::VscodeCompile,
            "vscode-package" => Self::VscodePackage,
            "vscode-test" => Self::VscodeTest,
            "vscode-test-e2e" => Self::VscodeTestE2e,
            "package" => Self::Package,
            "publish-dry-run" => Self::PublishDryRun,
            "help" => Self::Help(rest),
            other => Self::Unknown(other.to_string()),
        }
    }
}

pub(crate) fn print_help(args: &[String]) -> Result<(), String> {
    println!("{}", help_message(args)?);
    Ok(())
}

pub(crate) fn help_message(args: &[String]) -> Result<String, String> {
    if args.is_empty() {
        let commands = known_commands().join("\n  ");
        return Ok(format!(
            "xtask commands:\n\n  {commands}\n\nCommon starting points:\n  cargo xtask doctor      # setup and worktree hygiene\n  cargo xtask first-pr    # start-here packet with one safe next action\n  cargo xtask pr-ready    # local PR readiness packet\n  cargo xtask cockpit     # repo maintainer front panel\n  cargo xtask check-pr    # review-ready non-release gate\n\nStart-here language uses the same words for safe next action, missing artifact, stale evidence, wrong root, malformed artifact, no actionable gap, preview-limited evidence, verify command, receipt command, and receipt path.\n\nRun `cargo xtask help <command>` for mutability, writes, and notes.\nRun `cargo xtask commands` to write the full command catalog report."
        ));
    }

    let query = args.join(" ");
    let matches = help_entries_for_query(&query);
    if matches.is_empty() {
        return Err(unknown_command_message(&query));
    }

    let mut lines = vec![format!("xtask help: `{query}`"), String::new()];
    for entry in matches {
        lines.push(format!("Usage: cargo xtask {}", entry.command));
        lines.push(format!("Mutability: {}", entry.mutability));
        lines.push(format!("Writes: {}", entry.writes));
        lines.push(format!("Judgment required: {}", entry.judgment_required));
        lines.push(format!("Notes: {}", entry.notes));
        lines.push(String::new());
    }
    lines.push("Run `cargo xtask help` for the full command list.".to_string());
    Ok(lines.join("\n"))
}

fn help_entries_for_query(query: &str) -> Vec<CommandCatalogEntry> {
    let normalized = query.trim();
    let root = known_command_root(normalized);
    command_catalog()
        .into_iter()
        .filter(|entry| {
            entry.command == normalized
                || known_command_root(entry.command) == root
                || known_command_root(entry.command) == normalized
        })
        .collect()
}

pub(crate) fn known_commands() -> Vec<&'static str> {
    vec![
        "shape",
        "fix-pr",
        "install-hooks",
        "commands",
        "pr-summary",
        "pr-ready",
        "cockpit",
        "pr-triage-report",
        "gh-pr-status --pr <number>",
        "suggested-fixes",
        "precommit",
        "check-pr",
        "fixtures [name]",
        "goldens check",
        "goldens bless <name> --reason <reason>",
        "golden-drift",
        "metrics",
        "test-oracle-report",
        "check-test-oracles",
        "test-efficiency-report",
        "badge-artifacts",
        "repo-badge-artifacts [--gap-ledger <path>]",
        "badge-basis [--gap-ledger <path>] [--include-seam-classes]",
        "repo-seam-inventory",
        "repo-exposure-report",
        "repo-exposure-latency-report",
        "evidence-health",
        "lane1-evidence-audit",
        "evidence-quality-audit",
        "evidence-quality-scorecard",
        "evidence-quality-trend [--current <path>] [--previous <path>]",
        "actionable-gap-outcomes [--actionable-gaps <path>] [--agent-receipt <path>] [--targeted-test-outcome <path>]",
        "agent-seam-packets [root]",
        "ripr-swarm plan [--top <n>] [--actionable-gaps <path>]",
        "ripr-swarm readiness [--swarm-plan <path>] [--actionable-gap-outcomes <path>]",
        "lsp-cockpit-report",
        "operator-cockpit",
        "operator-cockpit-report",
        "release-readiness --version <version>",
        "release-server-archive --version <version> --target <triple> --executable <name> --archive <zip|tar.gz>",
        "release-server-manifest --version <version> --repository <owner/repo>",
        "release-upload-assets --version <version>",
        "targeted-test-outcome --before <path> --after <path>",
        "mutation-calibration [root] --mutants-json <path>",
        "recommendation-calibration [--root <path>] [--pr-guidance <path>] [--outcome-receipts <path>] [--out <path>]",
        "sarif-policy --current <path> [--baseline <path>]",
        "impacted-evidence [--pr-evidence <path>] [--label <label>] [--labels <csv>] [--check]",
        "ripr-pr [--base <rev>] [--head <rev>] [--root <path>] [--check]",
        "first-pr [--root <path>] [--base <rev>] [--head <rev>] [--gap-ledger <path>] [--out-dir <path>] [--check]",
        "ripr-review-comments [--base <rev>] [--head <rev>] [--root <path>] [--check]",
        "ripr-pr-summary [--check]",
        "ripr-annotations [--comments <path>] [--out <path>] [--check]",
        "badges [--check] [--gap-ledger <path>]",
        "update-badge-endpoints",
        "check-badge-endpoints",
        "dogfood",
        "critic",
        "goals status|next|report",
        "reports index",
        "receipts [check]",
        "doctor",
        "worktree doctor",
        "specs next",
        "ci-fast",
        "ci-full",
        "check-static-language",
        "check-no-panic-family [--propose]",
        "check-allow-attributes",
        "check-local-context",
        "check-file-policy",
        "rust-conversion-candidates",
        "check-executable-files",
        "check-workflows",
        "check-droid-review-config",
        "check-spec-format",
        "check-spec-numbering",
        "check-fixture-contracts",
        "check-traceability",
        "check-spec-ids",
        "check-behavior-manifest",
        "check-capabilities",
        "check-workspace-shape",
        "check-architecture",
        "check-public-api",
        "check-output-contracts",
        "check-doc-index",
        "check-readme-state",
        "markdown-links",
        "check-campaign",
        "check-goals",
        "check-pr-shape",
        "check-generated",
        "check-command-catalog",
        "check-badge-diff-policy",
        "check-generated-clean",
        "check-verification-contracts [--check]",
        "check-dependencies",
        "check-supply-chain",
        "check-process-policy",
        "check-network-policy",
        "check-lint-policy",
        "check-ci-lane-whitelist",
        "check-product-copy",
        "check-positioning-language",
        "check-doc-roles",
        "vscode-compile",
        "vscode-package",
        "vscode-test",
        "vscode-test-e2e",
        "package",
        "publish-dry-run",
    ]
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CommandCatalogEntry {
    pub(crate) command: &'static str,
    pub(crate) mutability: &'static str,
    pub(crate) writes: &'static str,
    pub(crate) judgment_required: bool,
    pub(crate) notes: &'static str,
}

pub(crate) fn command_catalog() -> Vec<CommandCatalogEntry> {
    vec![
        command_entry(
            "shape",
            "mutating",
            "source files and target/ripr/reports",
            false,
            "Runs deterministic local shaping such as formatting and repo shape report generation.",
        ),
        command_entry(
            "fix-pr",
            "mutating",
            "source files and target/ripr/reports",
            false,
            "Runs safe PR shaping and refreshes the reviewer packet.",
        ),
        command_entry(
            "install-hooks",
            "mutating",
            ".git/hooks",
            false,
            "Installs repo-managed local hooks.",
        ),
        command_entry(
            "commands",
            "report_only",
            "target/ripr/reports/commands.{md,json}",
            false,
            "Writes this command mutability catalog.",
        ),
        command_entry(
            "pr-summary",
            "report_only",
            "target/ripr/reports/pr-summary.md",
            false,
            "Summarizes the current diff for review.",
        ),
        command_entry(
            "pr-ready",
            "report_only",
            "target/ripr/reports/pr-ready.{md,json}, target/ripr/reports/index.{md,json}, and composed repo-ops reports",
            false,
            "Composes local readiness signals and points to safe next action, receipt state, and check-pr proof before opening or updating a PR.",
        ),
        command_entry(
            "cockpit",
            "external_state_read",
            "target/ripr/reports/cockpit.{md,json}, target/ripr/reports/index.{md,json}, and composed repo-ops reports",
            false,
            "Composes repo-level operating packets into an advisory front panel that names the next safe command and stop states before more work.",
        ),
        command_entry(
            "pr-triage-report",
            "external_state_read",
            "target/ripr/reports/pr-triage.{md,json}",
            false,
            "Reads GitHub PR metadata and writes an advisory queue report.",
        ),
        command_entry(
            "gh-pr-status --pr <number>",
            "external_state_read",
            "target/ripr/reports/gh-pr-status.{md,json}",
            false,
            "Reads one GitHub PR and reports safe next action.",
        ),
        command_entry(
            "suggested-fixes",
            "report_only",
            "target/ripr/reports/suggested-fixes.{patch,md}",
            false,
            "Emits deterministic repair suggestions only; never writes badge values, goldens, baselines, suppressions, dependency exceptions, or schema changes.",
        ),
        command_entry(
            "precommit",
            "non_mutating_check",
            "target/ripr/reports/precommit.md",
            false,
            "Cheap local guardrail for formatting and policy checks.",
        ),
        command_entry(
            "check-pr",
            "non_mutating_check",
            "target/ripr/reports and target/ripr/receipts",
            false,
            "Review-ready gate; must not mutate tracked files.",
        ),
        command_entry(
            "fixtures [name]",
            "report_only",
            "target/ripr/reports and fixture actual outputs under target",
            false,
            "Runs fixture checks and writes local evidence.",
        ),
        command_entry(
            "goldens check",
            "non_mutating_check",
            "target/ripr/reports/goldens.md",
            false,
            "Checks golden drift without updating expected outputs.",
        ),
        command_entry(
            "goldens bless <name> --reason <reason>",
            "mutating",
            "fixtures/**/expected/**",
            true,
            "Updates golden expected outputs and requires explicit review reason.",
        ),
        command_entry(
            "golden-drift",
            "report_only",
            "target/ripr/reports/golden-drift.{md,json}",
            false,
            "Reports golden drift without blessing changes.",
        ),
        command_entry(
            "metrics",
            "report_only",
            "target/ripr/reports/metrics.{md,json}",
            false,
            "Writes capability metrics reports.",
        ),
        command_entry(
            "test-oracle-report",
            "report_only",
            "target/ripr/reports/test-oracles.{md,json}",
            false,
            "Writes advisory test-oracle report.",
        ),
        command_entry(
            "check-test-oracles",
            "report_only",
            "target/ripr/reports/test-oracles.{md,json}",
            false,
            "Alias for test-oracle-report.",
        ),
        command_entry(
            "test-efficiency-report",
            "report_only",
            "target/ripr/reports/test-efficiency.{md,json}",
            false,
            "Writes advisory test-efficiency report.",
        ),
        command_entry(
            "badge-artifacts",
            "report_only",
            "target/ripr/reports",
            false,
            "Writes PR-scoped badge evidence under target.",
        ),
        command_entry(
            "repo-badge-artifacts [--gap-ledger <path>]",
            "report_only",
            "target/ripr/reports",
            false,
            "Writes repo-scoped badge evidence under target.",
        ),
        command_entry(
            "badge-basis [--gap-ledger <path>] [--include-seam-classes]",
            "report_only",
            "target/ripr/reports/badge-basis.{json,md}",
            false,
            "Audits public badge endpoint counts, current repo badge basis, seam-native inventory pressure, and the recommended actionable gap projection without editing badges/*.json; --include-seam-classes opts into the expensive full class breakdown.",
        ),
        command_entry(
            "repo-seam-inventory",
            "report_only",
            "target/ripr/reports/repo-seams.{json,md}",
            false,
            "Writes repo seam inventory reports.",
        ),
        command_entry(
            "repo-exposure-report",
            "report_only",
            "target/ripr/reports/repo-exposure.{json,md}",
            false,
            "Writes repo exposure reports.",
        ),
        command_entry(
            "repo-exposure-latency-report",
            "report_only",
            "target/ripr/reports/repo-exposure-latency.{json,md}",
            false,
            "Writes repo exposure latency reports.",
        ),
        command_entry(
            "evidence-health",
            "report_only",
            "target/ripr/reports/evidence-health.{json,md}",
            false,
            "Writes evidence-health reports.",
        ),
        command_entry(
            "lane1-evidence-audit",
            "report_only",
            "target/ripr/reports/lane1-evidence-audit.{json,md}",
            false,
            "Writes Lane 1 evidence audit reports.",
        ),
        command_entry(
            "evidence-quality-audit",
            "report_only",
            "target/ripr/reports/lane1-evidence-audit.{json,md}",
            false,
            "Alias for lane1-evidence-audit.",
        ),
        command_entry(
            "evidence-quality-scorecard",
            "report_only",
            "target/ripr/reports/evidence-quality-scorecard.{json,md}",
            false,
            "Writes evidence-quality scorecard reports.",
        ),
        command_entry(
            "evidence-quality-trend [--current <path>] [--previous <path>]",
            "report_only",
            "target/ripr/reports/evidence-quality-trend.{json,md}",
            false,
            "Writes evidence-quality trend reports.",
        ),
        command_entry(
            "actionable-gap-outcomes [--actionable-gaps <path>] [--agent-receipt <path>] [--targeted-test-outcome <path>]",
            "report_only",
            "target/ripr/reports/actionable-gap-outcomes.{json,md}",
            false,
            "Joins actionable gap packets with optional receipt and targeted-test outcome artifacts.",
        ),
        command_entry(
            "agent-seam-packets [root]",
            "report_only",
            "target/ripr/reports/agent-seam-packets.json",
            false,
            "Writes agent seam packets under target.",
        ),
        command_entry(
            "ripr-swarm plan [--top <n>] [--actionable-gaps <path>]",
            "report_only",
            "target/ripr/reports/swarm-plan.{json,md}",
            false,
            "Ranks existing actionable canonical gap packets into swarm-ready and blocked repair candidates; does not edit files, run tests, call providers, create receipts, or infer work from raw findings.",
        ),
        command_entry(
            "ripr-swarm readiness [--swarm-plan <path>] [--actionable-gap-outcomes <path>]",
            "report_only",
            "target/ripr/reports/swarm-readiness.{json,md}",
            false,
            "Rolls up swarm plan and actionable-gap outcome artifacts into advisory repair-coordination readiness counts.",
        ),
        command_entry(
            "lsp-cockpit-report",
            "report_only",
            "target/ripr/reports/lsp-cockpit.{json,md}",
            false,
            "Writes LSP cockpit reports.",
        ),
        command_entry(
            "operator-cockpit",
            "report_only",
            "target/ripr/reports/operator-cockpit.{json,md}",
            false,
            "Writes operator cockpit reports.",
        ),
        command_entry(
            "operator-cockpit-report",
            "report_only",
            "target/ripr/reports/operator-cockpit.{json,md}",
            false,
            "Alias for operator-cockpit.",
        ),
        command_entry(
            "release-readiness --version <version>",
            "report_only",
            "target/ripr/reports/release-readiness.{json,md}",
            false,
            "Writes release-readiness evidence; does not publish.",
        ),
        command_entry(
            "release-server-archive --version <version> --target <triple> --executable <name> --archive <zip|tar.gz>",
            "mutating",
            "target/release artifacts",
            false,
            "Builds local release server archive artifacts.",
        ),
        command_entry(
            "release-server-manifest --version <version> --repository <owner/repo>",
            "mutating",
            "target/release artifacts",
            false,
            "Builds local release server manifest artifacts.",
        ),
        command_entry(
            "release-upload-assets --version <version>",
            "external_state_mutating",
            "GitHub release assets",
            true,
            "Uploads release assets; requires explicit release approval.",
        ),
        command_entry(
            "targeted-test-outcome --before <path> --after <path>",
            "report_only",
            "target/ripr/reports/targeted-test-outcome.{json,md}",
            false,
            "Writes targeted-test outcome receipts under target.",
        ),
        command_entry(
            "mutation-calibration [root] --mutants-json <path>",
            "report_only",
            "target/ripr/reports/mutation-calibration.{json,md}",
            false,
            "Imports supplied runtime mutation results into advisory reports; does not run mutation testing.",
        ),
        command_entry(
            "recommendation-calibration [--root <path>] [--pr-guidance <path>] [--outcome-receipts <path>] [--out <path>]",
            "report_only",
            "target/ripr/reports or explicit --out",
            false,
            "Writes recommendation calibration reports.",
        ),
        command_entry(
            "sarif-policy --current <path> [--baseline <path>]",
            "report_only",
            "target/ripr/reports/sarif-policy.{json,md}",
            false,
            "Writes advisory SARIF policy report; blocking only if caller requests a failing policy mode.",
        ),
        command_entry(
            "impacted-evidence [--pr-evidence <path>] [--label <label>] [--labels <csv>] [--check]",
            "argument_dependent",
            "target/ripr/reports or check-only",
            false,
            "Writes or checks impacted-evidence reports depending on --check.",
        ),
        command_entry(
            "ripr-pr [--base <rev>] [--head <rev>] [--root <path>] [--check]",
            "argument_dependent",
            "target/ripr/reports or check-only",
            false,
            "Writes or checks PR evidence packets depending on --check.",
        ),
        command_entry(
            "first-pr [--root <path>] [--base <rev>] [--head <rev>] [--gap-ledger <path>] [--out-dir <path>] [--check]",
            "argument_dependent",
            "target/ripr/reports or check-only",
            false,
            "Writes the start-here packet when --check is absent; checks existing packets when --check is present. The packet names one repairable gap, fallback state, verify command, receipt command, and receipt path.",
        ),
        command_entry(
            "ripr-review-comments [--base <rev>] [--head <rev>] [--root <path>] [--check]",
            "argument_dependent",
            "target/ripr/reports or check-only",
            false,
            "Writes or checks review-comment wrapper output depending on --check.",
        ),
        command_entry(
            "ripr-pr-summary [--check]",
            "argument_dependent",
            "target/ripr/reports or check-only",
            false,
            "Writes or checks PR summary output depending on --check.",
        ),
        command_entry(
            "ripr-annotations [--comments <path>] [--out <path>] [--check]",
            "argument_dependent",
            "target/ripr/reports or explicit --out",
            false,
            "Writes or checks annotation output depending on --check.",
        ),
        command_entry(
            "badges",
            "mutating",
            "badges/*.json and target/ripr/reports",
            false,
            "Refreshes committed public badge endpoint JSON; use only in explicit badge refresh work.",
        ),
        command_entry(
            "badges --check",
            "non_mutating_check",
            "target/ripr/reports",
            false,
            "Compares generated badge endpoint output without updating committed badges/*.json.",
        ),
        command_entry(
            "update-badge-endpoints",
            "mutating",
            "badges/*.json",
            false,
            "Refreshes committed public badge endpoint JSON; use only in explicit badge refresh work.",
        ),
        command_entry(
            "check-badge-endpoints",
            "non_mutating_check",
            "target/ripr/reports/badge-endpoints.md",
            false,
            "Checks committed public badge endpoint JSON against generated target output.",
        ),
        command_entry(
            "dogfood",
            "report_only",
            "target/ripr/dogfood and target/ripr/reports",
            false,
            "Writes repo-local dogfood evidence and receipts under target.",
        ),
        command_entry(
            "critic",
            "report_only",
            "target/ripr/reports/critic.{md,json}",
            false,
            "Writes advisory reviewer-risk report.",
        ),
        command_entry(
            "goals status|next|report",
            "report_only",
            "target/ripr/reports/goals*.md",
            false,
            "Reports active goal state without changing manifests.",
        ),
        command_entry(
            "reports index",
            "report_only",
            "target/ripr/reports/index.{md,json}",
            false,
            "Indexes generated report packets under target.",
        ),
        command_entry(
            "receipts [check]",
            "argument_dependent",
            "target/ripr/receipts and target/ripr/reports/receipts.md",
            false,
            "Writes receipts by default; checks existing receipts with `receipts check`.",
        ),
        command_entry(
            "doctor",
            "report_only",
            "target/ripr/reports/worktree-doctor.md",
            false,
            "Shortcut for worktree doctor; use before first-pr when setup, missing artifacts, stale evidence, or wrong-root state is unclear.",
        ),
        command_entry(
            "worktree doctor",
            "report_only",
            "target/ripr/reports/worktree-doctor.md",
            false,
            "Writes advisory setup and worktree hygiene status before choosing a start-here repair path.",
        ),
        command_entry(
            "specs next",
            "report_only",
            "stdout",
            false,
            "Prints the next available RIPR-SPEC ID.",
        ),
        command_entry(
            "ci-fast",
            "non_mutating_check",
            "target/ripr/reports and target/ripr/receipts",
            false,
            "Runs the fast CI lane and writes local receipts.",
        ),
        command_entry(
            "ci-full",
            "non_mutating_check",
            "target/ripr/reports and target/ripr/receipts",
            false,
            "Runs the full CI lane and writes local receipts.",
        ),
        command_entry(
            "check-static-language",
            "non_mutating_check",
            "target/ripr/reports/static-language.md",
            false,
            "Checks static language policy.",
        ),
        command_entry(
            "check-no-panic-family [--propose]",
            "argument_dependent",
            "target/ripr/reports or proposal output",
            false,
            "Checks panic-family policy; --propose only emits proposed allowlist material for review.",
        ),
        command_entry(
            "check-allow-attributes",
            "non_mutating_check",
            "target/ripr/reports",
            false,
            "Checks allow-attribute policy.",
        ),
        command_entry(
            "check-local-context",
            "non_mutating_check",
            "target/ripr/reports/local-context.json",
            false,
            "Checks local-context leak policy.",
        ),
        command_entry(
            "check-file-policy",
            "non_mutating_check",
            "target/ripr/reports/file-policy.md",
            false,
            "Checks file policy.",
        ),
        command_entry(
            "rust-conversion-candidates",
            "report_only",
            "target/ripr/reports/rust-conversion-candidates.{md,json}",
            false,
            "Reports non-Rust and workflow-shell surfaces that are candidates for migration into Rust/xtask, while documenting approved external-runtime and fixture boundaries.",
        ),
        command_entry(
            "check-executable-files",
            "non_mutating_check",
            "target/ripr/reports/executable-files.md",
            false,
            "Checks executable-file policy.",
        ),
        command_entry(
            "check-workflows",
            "non_mutating_check",
            "target/ripr/reports/workflows.md",
            false,
            "Checks workflow policy.",
        ),
        command_entry(
            "check-droid-review-config",
            "non_mutating_check",
            "target/ripr/reports/droid-review-config.md",
            false,
            "Checks Droid review configuration.",
        ),
        command_entry(
            "check-spec-format",
            "non_mutating_check",
            "target/ripr/reports/spec-format.md",
            false,
            "Checks spec formatting.",
        ),
        command_entry(
            "check-spec-numbering",
            "non_mutating_check",
            "target/ripr/reports/spec-numbering.md",
            false,
            "Checks spec ID uniqueness and references.",
        ),
        command_entry(
            "check-fixture-contracts",
            "non_mutating_check",
            "target/ripr/reports/fixture-contracts.md",
            false,
            "Checks fixture contracts.",
        ),
        command_entry(
            "check-traceability",
            "non_mutating_check",
            "target/ripr/reports/traceability.md",
            false,
            "Checks traceability references.",
        ),
        command_entry(
            "check-spec-ids",
            "non_mutating_check",
            "target/ripr/reports/traceability.md",
            false,
            "Alias for check-traceability.",
        ),
        command_entry(
            "check-behavior-manifest",
            "non_mutating_check",
            "target/ripr/reports/traceability.md",
            false,
            "Alias for check-traceability.",
        ),
        command_entry(
            "check-capabilities",
            "non_mutating_check",
            "target/ripr/reports/capabilities.md",
            false,
            "Checks capability metadata.",
        ),
        command_entry(
            "check-workspace-shape",
            "non_mutating_check",
            "target/ripr/reports/workspace-shape.md",
            false,
            "Checks workspace shape.",
        ),
        command_entry(
            "check-architecture",
            "non_mutating_check",
            "target/ripr/reports/architecture.md",
            false,
            "Checks architecture boundaries.",
        ),
        command_entry(
            "check-public-api",
            "non_mutating_check",
            "target/ripr/reports/public-api.md",
            false,
            "Checks public API boundaries.",
        ),
        command_entry(
            "check-output-contracts",
            "non_mutating_check",
            "target/ripr/reports/output-contracts.md",
            false,
            "Checks output contract registry.",
        ),
        command_entry(
            "check-doc-index",
            "non_mutating_check",
            "target/ripr/reports/doc-index.md",
            false,
            "Checks documentation index coverage.",
        ),
        command_entry(
            "check-readme-state",
            "non_mutating_check",
            "target/ripr/reports/readme-state.md",
            false,
            "Checks README state.",
        ),
        command_entry(
            "markdown-links",
            "non_mutating_check",
            "target/ripr/reports/markdown-links.md",
            false,
            "Checks Markdown links.",
        ),
        command_entry(
            "check-campaign",
            "non_mutating_check",
            "target/ripr/reports/campaign.md",
            false,
            "Checks campaign/source-of-truth consistency.",
        ),
        command_entry(
            "check-goals",
            "non_mutating_check",
            "target/ripr/reports/campaign.md",
            false,
            "Alias for check-campaign.",
        ),
        command_entry(
            "check-pr-shape",
            "non_mutating_check",
            "target/ripr/reports/pr-shape.md",
            false,
            "Checks PR shape.",
        ),
        command_entry(
            "check-generated",
            "non_mutating_check",
            "target/ripr/reports/generated.md",
            false,
            "Checks generated file policy.",
        ),
        command_entry(
            "check-command-catalog",
            "non_mutating_check",
            "target/ripr/reports/command-catalog.md",
            false,
            "Checks that every xtask command is classified by the command mutability catalog.",
        ),
        command_entry(
            "check-badge-diff-policy",
            "non_mutating_check",
            "target/ripr/reports/badge-diff-policy.md",
            false,
            "Rejects ordinary badge endpoint diffs.",
        ),
        command_entry(
            "check-generated-clean",
            "non_mutating_check",
            "target/ripr/reports/generated-clean.md",
            false,
            "Rejects generated residue in ordinary PRs.",
        ),
        command_entry(
            "check-verification-contracts [--check]",
            "argument_dependent",
            "target/ripr/reports or check-only",
            false,
            "Writes or checks verification contract reports depending on --check.",
        ),
        command_entry(
            "check-dependencies",
            "non_mutating_check",
            "target/ripr/reports/dependencies.md",
            false,
            "Checks dependency policy.",
        ),
        command_entry(
            "check-supply-chain",
            "non_mutating_check",
            "target/ripr/reports/supply-chain.md",
            false,
            "Checks supply-chain policy.",
        ),
        command_entry(
            "check-process-policy",
            "non_mutating_check",
            "target/ripr/reports/process-policy.md",
            false,
            "Checks process policy.",
        ),
        command_entry(
            "check-network-policy",
            "non_mutating_check",
            "target/ripr/reports/network-policy.md",
            false,
            "Checks network policy.",
        ),
        command_entry(
            "check-lint-policy",
            "non_mutating_check",
            "target/ripr/reports/lint-policy.md",
            false,
            "Checks lint policy.",
        ),
        command_entry(
            "check-ci-lane-whitelist",
            "non_mutating_check",
            "target/ripr/reports/ci-lane-whitelist.md",
            false,
            "Checks CI lane whitelist.",
        ),
        command_entry(
            "check-product-copy",
            "non_mutating_check",
            "target/ripr/reports/product-copy.md",
            false,
            "Checks public product-copy policy.",
        ),
        command_entry(
            "check-positioning-language",
            "non_mutating_check",
            "target/ripr/reports/positioning-language.md",
            false,
            "Checks positioning-language policy.",
        ),
        command_entry(
            "check-doc-roles",
            "non_mutating_check",
            "target/ripr/reports/doc-roles.md",
            false,
            "Checks documentation role policy.",
        ),
        command_entry(
            "vscode-compile",
            "non_mutating_check",
            "editors/vscode build output",
            false,
            "Runs VS Code extension compile.",
        ),
        command_entry(
            "vscode-package",
            "mutating",
            "editors/vscode/dist",
            false,
            "Builds VS Code extension package artifacts.",
        ),
        command_entry(
            "vscode-test",
            "non_mutating_check",
            "editor test output",
            false,
            "Runs VS Code tests.",
        ),
        command_entry(
            "vscode-test-e2e",
            "non_mutating_check",
            "editor test output",
            false,
            "Runs VS Code end-to-end tests.",
        ),
        command_entry(
            "package",
            "non_mutating_check",
            "cargo package staging output",
            false,
            "Lists package contents without publishing.",
        ),
        command_entry(
            "publish-dry-run",
            "non_mutating_check",
            "cargo publish dry-run staging output",
            false,
            "Runs publish dry run without publishing.",
        ),
    ]
}

const fn command_entry(
    command: &'static str,
    mutability: &'static str,
    writes: &'static str,
    judgment_required: bool,
    notes: &'static str,
) -> CommandCatalogEntry {
    CommandCatalogEntry {
        command,
        mutability,
        writes,
        judgment_required,
        notes,
    }
}

pub(crate) fn unknown_command_message(command: &str) -> String {
    let normalized = command.trim();
    let suggestion = known_commands()
        .into_iter()
        .filter_map(|candidate| {
            let root = known_command_root(candidate);
            let distance = levenshtein(normalized, root);
            (distance <= 3).then_some((root, distance))
        })
        .min_by_key(|(_, distance)| *distance)
        .map(|(root, _)| root);
    match suggestion {
        Some(suggestion) => format!(
            "unknown xtask command `{normalized}`.\nDid you mean `{suggestion}`?\nRun `cargo xtask help` for the full list."
        ),
        None => format!(
            "unknown xtask command `{normalized}`.\nRun `cargo xtask help` for the full list."
        ),
    }
}

pub(crate) fn known_command_root(command: &str) -> &str {
    command
        .split_once(' ')
        .map_or(command, |(prefix, _)| prefix)
}

fn levenshtein(lhs: &str, rhs: &str) -> usize {
    if lhs.is_empty() {
        return rhs.chars().count();
    }
    if rhs.is_empty() {
        return lhs.chars().count();
    }

    let rhs_len = rhs.chars().count();
    let mut previous_row: Vec<usize> = (0..=rhs_len).collect();

    for (left_index, left_char) in lhs.chars().enumerate() {
        let mut current_row = vec![left_index + 1];
        for (right_index, right_char) in rhs.chars().enumerate() {
            let insertion = current_row[right_index] + 1;
            let deletion = previous_row[right_index + 1] + 1;
            let substitution = previous_row[right_index] + usize::from(left_char != right_char);
            current_row.push(insertion.min(deletion).min(substitution));
        }
        previous_row = current_row;
    }

    previous_row[rhs_len]
}

#[cfg(test)]
mod tests {
    use super::{command_catalog, help_message};

    #[test]
    fn top_level_help_pins_start_here_front_door_language() -> Result<(), String> {
        let help = help_message(&[])?;
        assert!(help.contains("cargo xtask doctor"));
        assert!(help.contains("cargo xtask first-pr"));
        assert!(help.contains("safe next action"));
        assert!(help.contains("missing artifact"));
        assert!(help.contains("stale evidence"));
        assert!(help.contains("wrong root"));
        assert!(help.contains("malformed artifact"));
        assert!(help.contains("no actionable gap"));
        assert!(help.contains("preview-limited evidence"));
        assert!(help.contains("verify command"));
        assert!(help.contains("receipt command"));
        assert!(help.contains("receipt path"));
        Ok(())
    }

    #[test]
    fn command_catalog_pins_start_here_notes() {
        let catalog = command_catalog();
        let note = |command: &str| {
            catalog
                .iter()
                .find(|entry| entry.command == command)
                .map(|entry| entry.notes)
                .unwrap_or("")
        };
        assert!(note("first-pr [--root <path>] [--base <rev>] [--head <rev>] [--gap-ledger <path>] [--out-dir <path>] [--check]").contains("start-here packet"));
        assert!(note("first-pr [--root <path>] [--base <rev>] [--head <rev>] [--gap-ledger <path>] [--out-dir <path>] [--check]").contains("verify command"));
        assert!(note("pr-ready").contains("safe next action"));
        assert!(note("cockpit").contains("stop states"));
        assert!(note("doctor").contains("missing artifacts"));
        assert!(note("worktree doctor").contains("start-here repair path"));
        assert!(
            note("badge-basis [--gap-ledger <path>] [--include-seam-classes]")
                .contains("Audits public badge endpoint counts")
        );
    }
}
