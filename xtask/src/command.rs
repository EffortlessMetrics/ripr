#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum XtaskCommand {
    Shape,
    FixPr,
    InstallHooks(Vec<String>),
    Commands,
    PrSummary,
    Proof(Vec<String>),
    PrReady,
    Cockpit,
    PrTriageReport,
    GhPrStatus(Vec<String>),
    CiBudget(Vec<String>),
    ModuleHealth(Vec<String>),
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
    RiprPlus(Vec<String>),
    RepoSeamInventory,
    RepoExposureReport,
    RepoExposureSummaryReport,
    RepoExposureLatencyReport,
    RepoContractReport,
    PrBody(Vec<String>),
    Closeout(Vec<String>),
    EvidenceHealth,
    Lane1EvidenceAudit,
    EvidenceQualityScorecard,
    EvidenceQualityTrend(Vec<String>),
    ActionableGapOutcomes(Vec<String>),
    AgentSeamPackets(Option<String>),
    RiprSwarm(Vec<String>),
    RouteQuality(Vec<String>),
    LspCockpitReport,
    OperatorCockpitReport,
    ReleaseReadiness(Vec<String>),
    ReleaseServerArchive(Vec<String>),
    ReleaseServerManifest(Vec<String>),
    ReleaseUploadAssets(Vec<String>),
    SourcePromotion(Vec<String>),
    TargetedTestOutcome(Vec<String>),
    MutationCalibration(Vec<String>),
    BunUbCalibration(Vec<String>),
    BunUbPreviewSummary(Vec<String>),
    ConfiguredBridgeInventory(Vec<String>),
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
    EvalSweep(Vec<String>),
    Critic,
    Goals(Vec<String>),
    Reports(Vec<String>),
    Cache(Vec<String>),
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
    CheckDocArtifacts,
    CheckSupportTiers,
    CheckDocIndex,
    CheckReadmeState,
    MarkdownLinks,
    CheckCampaign,
    CheckPrShape,
    CheckGenerated,
    CheckCommandCatalog,
    CheckEvidencePromotionHonesty,
    CheckBadgeDiffPolicy,
    CheckGeneratedClean,
    CheckVerificationContracts(Vec<String>),
    CheckDependencies,
    CheckSupplyChain,
    CheckProcessPolicy,
    CheckNetworkPolicy,
    CheckLintPolicy,
    CheckCiLaneWhitelist,
    CheckProofPacks,
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
            "proof" => Self::Proof(rest),
            "pr-ready" => Self::PrReady,
            "cockpit" => Self::Cockpit,
            "pr-triage-report" => Self::PrTriageReport,
            "gh-pr-status" => Self::GhPrStatus(rest),
            "ci-budget" => Self::CiBudget(rest),
            "module-health" => Self::ModuleHealth(rest),
            "eval-sweep" => Self::EvalSweep(rest),
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
            "ripr-plus" => Self::RiprPlus(rest),
            "repo-seam-inventory" => Self::RepoSeamInventory,
            "repo-exposure-report" => Self::RepoExposureReport,
            "repo-exposure-summary-report" => Self::RepoExposureSummaryReport,
            "repo-exposure-latency-report" => Self::RepoExposureLatencyReport,
            "repo-contract-report" => Self::RepoContractReport,
            "pr-body" => Self::PrBody(rest),
            "closeout" => Self::Closeout(rest),
            "evidence-health" => Self::EvidenceHealth,
            "lane1-evidence-audit" | "evidence-quality-audit" => Self::Lane1EvidenceAudit,
            "evidence-quality-scorecard" => Self::EvidenceQualityScorecard,
            "evidence-quality-trend" => Self::EvidenceQualityTrend(rest),
            "actionable-gap-outcomes" => Self::ActionableGapOutcomes(rest),
            "agent-seam-packets" => Self::AgentSeamPackets(rest.first().cloned()),
            "ripr-swarm" => Self::RiprSwarm(rest),
            "route-quality" => Self::RouteQuality(rest),
            "lsp-cockpit-report" => Self::LspCockpitReport,
            "operator-cockpit" | "operator-cockpit-report" => Self::OperatorCockpitReport,
            "release-readiness" => Self::ReleaseReadiness(rest),
            "release-server-archive" => Self::ReleaseServerArchive(rest),
            "release-server-manifest" => Self::ReleaseServerManifest(rest),
            "release-upload-assets" => Self::ReleaseUploadAssets(rest),
            "source-promotion" => Self::SourcePromotion(rest),
            "targeted-test-outcome" => Self::TargetedTestOutcome(rest),
            "mutation-calibration" => Self::MutationCalibration(rest),
            "bun-ub-calibration" => Self::BunUbCalibration(rest),
            "bun-ub-preview-summary" => Self::BunUbPreviewSummary(rest),
            "configured-bridge-inventory" => Self::ConfiguredBridgeInventory(rest),
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
            "cache" => Self::Cache(rest),
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
            "check-evidence-promotion-honesty" => Self::CheckEvidencePromotionHonesty,
            "check-traceability" | "check-spec-ids" | "check-behavior-manifest" => {
                Self::CheckTraceability
            }
            "check-capabilities" => Self::CheckCapabilities,
            "check-workspace-shape" => Self::CheckWorkspaceShape,
            "check-architecture" => Self::CheckArchitecture,
            "check-public-api" => Self::CheckPublicApi,
            "check-output-contracts" => Self::CheckOutputContracts,
            "check-doc-artifacts" => Self::CheckDocArtifacts,
            "check-support-tiers" => Self::CheckSupportTiers,
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
            "check-proof-packs" => Self::CheckProofPacks,
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
        return Ok(format_top_level_help(&known_commands()));
    }

    let query = args.join(" ");
    let matches = help_entries_for_query(&query);
    if matches.is_empty() {
        return Err(unknown_command_message(&query));
    }

    Ok(format_help_entries(&query, &matches))
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
        "proof route [--base <rev>] [--head <rev>]",
        "proof preflight [--base <rev>] [--head <rev>]",
        "pr-ready",
        "cockpit",
        "pr-triage-report",
        "gh-pr-status --pr <number>",
        "ci-budget [--workflow <name>] [--limit <n>] [--input <path>]",
        "module-health [--threshold <n>]",
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
        "ripr-plus [--gap-ledger <path>] [--repo-exposure-summary <path>]",
        "repo-seam-inventory",
        "repo-exposure-report",
        "repo-exposure-summary-report",
        "repo-exposure-latency-report",
        "repo-contract-report",
        "pr-body --work-item <id>",
        "closeout --goal <goal-id>",
        "evidence-health",
        "lane1-evidence-audit",
        "evidence-quality-audit",
        "evidence-quality-scorecard",
        "evidence-quality-trend [--current <path>] [--previous <path>]",
        "actionable-gap-outcomes [--actionable-gaps <path>] [--agent-receipt <path>] [--targeted-test-outcome <path>]",
        "agent-seam-packets [root]",
        "ripr-swarm plan [--top <n>] [--actionable-gaps <path>]",
        "ripr-swarm attempt --packet <id> --dry-run [--actionable-gaps <path>]",
        "ripr-swarm attempt-ledger [--swarm-plan <path>] [--actionable-gap-outcomes <path>] [--previous-ledger <path>]",
        "ripr-swarm readiness [--swarm-plan <path>] [--actionable-gap-outcomes <path>] [--attempt-ledger <path>]",
        "route-quality [--attempt-ledger <path>]",
        "lsp-cockpit-report",
        "operator-cockpit",
        "operator-cockpit-report",
        "release-readiness --version <version>",
        "release-server-archive --version <version> --target <triple> --executable <name> --archive <zip|tar.gz>",
        "release-server-manifest --version <version> --repository <owner/repo>",
        "release-upload-assets --version <version>",
        "source-promotion verify --preflight <receipt.json> --resolution-manifest <manifest.json> --join-head <sha> --source-main <sha> [--main-head <sha>] [--out <dir>]",
        "source-promotion validate-resolved-tree --source-parent <sha> --swarm-parent <sha> --reviewed-tree <tree> --preflight <receipt.json> --preflight-sha256 <digest> --resolution-manifest <manifest.json> --resolution-sha256 <digest> [--out <dir>]",
        "source-promotion write-trusted-builder-receipt --source-parent <sha> --workflow-source-sha <sha> --executable <path> --cargo-target-dir <path> --locked-build --isolated-target-dir [--out <dir>]",
        "source-promotion admit-resolved-tree --source-parent <sha> --swarm-parent <sha> --join-tree <tree> --preflight <path> --preflight-sha256 <digest> --resolution-manifest <path> --resolution-sha256 <digest> --validation-packet <dir> --builder-packet <dir> --integration-index <path> --integration-index-sha256 <digest> [--out <dir>]",
        "source-promotion construct-exact-join --admission-packet <dir> --validation-packet <dir> --integration-index <path> --integration-index-sha256 <digest> --preflight <path> --resolution-manifest <path> --qualification-receipt <path> --qualification-receipt-sha256 <digest> --source-main-ref <ref> --swarm-ref <ref> --candidate-ref <ref> [--out <dir>]",
        "source-promotion publish-candidate-ref --construction-packet <dir> --source-main-ref <ref> --remote origin --target-ref <refs/heads/promote/0.11.0-...> (--expected-absent | --expected-old <sha>) [--out <dir>]",
        "targeted-test-outcome --before <path> --after <path>",
        "mutation-calibration [root] --mutants-json <path>",
        "bun-ub-calibration [--corpus <path>] [--out <path>] [--out-md <path>]",
        "bun-ub-preview-summary [--calibration-corpus <path>] [--graph-corpus <path>] [--dogfood-corpus <path>] [--out <path>] [--out-md <path>]",
        "configured-bridge-inventory [--graph-corpus <path>] [--out <path>] [--out-md <path>]",
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
        "cache report",
        "cache gc [--dry-run] [--max-size-gb <n>] [--ttl-days <n>]",
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
        "check-evidence-promotion-honesty",
        "check-traceability",
        "check-spec-ids",
        "check-behavior-manifest",
        "check-capabilities",
        "check-workspace-shape",
        "check-architecture",
        "check-public-api",
        "check-output-contracts",
        "check-doc-artifacts",
        "check-support-tiers",
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
        "check-proof-packs",
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
            "proof route [--base <rev>] [--head <rev>]",
            "report_only",
            "target/ripr/reports/proof-route.{json,md}",
            false,
            "Maps changed files onto proof packs and reports required, advisory, skipped, and never-routed CI lanes; read-only and advisory, it executes no proof commands and changes no CI behavior.",
        ),
        command_entry(
            "proof preflight [--base <rev>] [--head <rev>]",
            "report_only",
            "target/ripr/reports/proof-preflight.{json,md} plus the generated evidence the executed proof commands write under target/",
            false,
            "Executes the routed proof packs' required commands locally (deduplicated, fail-fast; advisory commands are listed but never run) and writes a local, advisory preflight receipt; it does not replace CI and changes no CI behavior.",
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
            "ci-budget [--workflow <name>] [--limit <n>] [--input <path>]",
            "external_state_read",
            "target/ripr/reports/ci-budget.{json,md}",
            false,
            "Reads recent routed-workflow runs through gh (or a supplied --input JSON file) and writes an advisory CI budget and merge-queue hygiene report; it separates disk-guard infrastructure tempfails (issue #1058) from product failures, never reruns or mutates any run, and changes no CI behavior.",
        ),
        command_entry(
            "module-health [--threshold <n>]",
            "report_only",
            "target/ripr/reports/module-health.{json,md}",
            false,
            "Walks crates/ripr/src/ and xtask/src/ for *.rs files, counts lines per file, and writes an advisory ranked report flagging files over the configurable line threshold (default 2000); always exits 0, never mutates source, and is never wired into CI gates.",
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
            "ripr-plus [--gap-ledger <path>] [--repo-exposure-summary <path>]",
            "report_only",
            "target/ripr/reports/ripr-plus.{json,md}",
            false,
            "Writes the repo-wide RIPR+ quality receipt from bounded repo-exposure-summary-json canonical actionable gaps, not raw seam inventory; --repo-exposure-summary reuses a downstream-consumable bounded summary artifact, and --gap-ledger uses an existing gap decision ledger through repo-badge-json to avoid an expensive fresh repo scan.",
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
            "Writes full evidence-heavy repo exposure reports for explicit deep inspection.",
        ),
        command_entry(
            "repo-exposure-summary-report",
            "report_only",
            "target/ripr/reports/repo-exposure-summary.json",
            false,
            "Writes the bounded repo exposure summary JSON for ordinary local metrics, planning, and CI-safe inspection.",
        ),
        command_entry(
            "repo-exposure-latency-report",
            "report_only",
            "target/ripr/reports/repo-exposure-latency.{json,md}",
            false,
            "Writes repo exposure latency reports.",
        ),
        command_entry(
            "repo-contract-report",
            "report_only",
            "target/ripr/reports/source-of-truth-graph.{md,json}",
            false,
            "Writes the source-of-truth contract graph report.",
        ),
        command_entry(
            "pr-body --work-item <id>",
            "report_only",
            "target/ripr/reports/source-of-truth-pr-body.md",
            false,
            "Writes a PR body scaffold from the active goal work item.",
        ),
        command_entry(
            "closeout --goal <goal-id>",
            "mutating",
            "docs/handoffs/<date>-<goal-id>-closeout.md and .ripr/goals/archive/<date>-<goal-id>.toml",
            true,
            "Writes a closeout scaffold and archived active-goal manifest for maintainer review.",
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
            "ripr-swarm attempt --packet <id> --dry-run [--actionable-gaps <path>]",
            "report_only",
            "stdout",
            false,
            "Prints one bounded swarm repair packet for operator handoff without editing files, running tests, calling providers, or creating receipts.",
        ),
        command_entry(
            "ripr-swarm attempt-ledger [--swarm-plan <path>] [--actionable-gap-outcomes <path>] [--previous-ledger <path>] [--real-repair-attempts <path>]",
            "report_only",
            "target/ripr/reports/swarm-attempt-ledger.{json,md}",
            false,
            "Builds durable attempt history from swarm plan, outcome, prior ledger, and real repair attempt artifacts without executing repairs.",
        ),
        command_entry(
            "ripr-swarm readiness [--swarm-plan <path>] [--actionable-gap-outcomes <path>] [--attempt-ledger <path>]",
            "report_only",
            "target/ripr/reports/swarm-readiness.{json,md}",
            false,
            "Rolls up swarm plan, actionable-gap outcome, and attempt-ledger artifacts into advisory repair-coordination readiness counts and next actions.",
        ),
        command_entry(
            "route-quality [--attempt-ledger <path>]",
            "report_only",
            "target/ripr/reports/route-quality.{json,md}",
            false,
            "Surfaces repair-route quality rows as a standalone report (RIPR-SPEC-0080). Reads from the swarm-attempt-ledger artifact; does not execute repairs or recompute attempt counts.",
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
            "source-promotion verify --preflight <receipt.json> --resolution-manifest <manifest.json> --join-head <sha> --source-main <sha> [--main-head <sha>] [--out <dir>]",
            "report_only",
            "target/ripr/source-promotion/source-promotion-verification.{json,md} or explicit --out <dir>",
            false,
            "Verifies an exact history-preserving join, reviewed resolution manifest, ancestry digests, and metadata identity without constructing or mutating Git refs.",
        ),
        command_entry(
            "source-promotion validate-resolved-tree --source-parent <sha> --swarm-parent <sha> --reviewed-tree <tree> --preflight <receipt.json> --preflight-sha256 <digest> --resolution-manifest <manifest.json> --resolution-sha256 <digest> [--out <dir>]",
            "report_only",
            "target/ripr/source-promotion/resolved-tree/{resolved-tree-validation.json,resolved-tree-validation.md,commands/**} or explicit --out <dir>; transient unreferenced Git object and worktree-registry entries in the caller repository are removed before return",
            false,
            "Validates one exact reviewed tree with the source-parent governance catalog and retained bounded evidence before direct-J construction; retained worktree state or authoritative ref movement rejects validation.",
        ),
        command_entry(
            "source-promotion write-trusted-builder-receipt --source-parent <sha> --workflow-source-sha <sha> --executable <path> --cargo-target-dir <path> --locked-build --isolated-target-dir [--out <dir>]",
            "report_only",
            "target/ripr/source-promotion/trusted-builder/{trusted-builder.json,packet-index.json} or explicit --out <dir>",
            false,
            "Writes a source-bound trusted-builder receipt without constructing a join or mutating Git refs.",
        ),
        command_entry(
            "source-promotion admit-resolved-tree --source-parent <sha> --swarm-parent <sha> --join-tree <tree> --preflight <path> --preflight-sha256 <digest> --resolution-manifest <path> --resolution-sha256 <digest> --validation-packet <dir> --builder-packet <dir> --integration-index <path> --integration-index-sha256 <digest> [--out <dir>]",
            "report_only",
            "target/ripr/source-promotion/resolved-tree-admission/{resolved-tree-admission.json,packet-index.json} or explicit --out <dir>",
            false,
            "Admits one exact resolved tree only after every bound source-owned validation, builder, and integration receipt passes and the integration-index bytes match the caller-bound lowercase SHA-256; it does not construct a join or mutate Git refs.",
        ),
        command_entry(
            "source-promotion construct-exact-join --admission-packet <dir> --validation-packet <dir> --integration-index <path> --integration-index-sha256 <digest> --preflight <path> --resolution-manifest <path> --qualification-receipt <path> --qualification-receipt-sha256 <digest> --source-main-ref <ref> --swarm-ref <ref> --candidate-ref <ref> [--out <dir>]",
            "mutating",
            "Git object database plus target/ripr/source-promotion/exact-join-construction/{exact-join-construction.json,packet-index.json} or explicit --out <dir>; no Git ref",
            false,
            "Constructs one deterministic unreferenced exact-J commit object only after admission, caller-bound integration-index rereads, and terminal qualification; it does not publish or mutate a Git ref.",
        ),
        command_entry(
            "source-promotion publish-candidate-ref --construction-packet <dir> --source-main-ref <ref> --remote origin --target-ref <refs/heads/promote/0.11.0-...> (--expected-absent | --expected-old <sha>) [--out <dir>]",
            "external_state_mutating",
            "local and remote candidate refs plus target/ripr/source-promotion/candidate-ref-publication/{candidate-ref-publication.json,packet-index.json} or explicit --out <dir>",
            true,
            "Publishes only the construction-bound candidate ref with an exact expected-state lease; this capability is judgment-required and does not authorize a real promotion.",
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
            "bun-ub-calibration [--corpus <path>] [--out <path>] [--out-md <path>]",
            "report_only",
            "target/ripr/reports/bun-ub-calibration.{json,md} or explicit --out paths",
            false,
            "Writes advisory Bun UB TypeScript calibration reports; does not run Bun, TypeScript, mutation, providers, generated tests, or source edits.",
        ),
        command_entry(
            "bun-ub-preview-summary [--calibration-corpus <path>] [--graph-corpus <path>] [--dogfood-corpus <path>] [--out <path>] [--out-md <path>]",
            "report_only",
            "target/ripr/reports/bun-ub-preview-summary.{json,md} or explicit --out paths",
            false,
            "Writes a compact advisory Bun UB preview summary from existing calibration, graph, and dogfood data; does not run Bun, TypeScript, mutation, providers, generated tests, or source edits.",
        ),
        command_entry(
            "configured-bridge-inventory [--graph-corpus <path>] [--out <path>] [--out-md <path>]",
            "report_only",
            "target/ripr/reports/configured-bridge-inventory.{json,md} or explicit --out paths",
            false,
            "Writes a report-only configured bridge inventory from existing cross-language oracle graph data; does not infer reachability, create repair packets, suggest placement from missing inventory rows, run Bun or TypeScript, create gates/badges, or promote support status.",
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
            "cache report",
            "report_only",
            "stdout and target/ripr/reports/cache-report.{md,json}",
            false,
            "Reports target/ripr/cache families, largest files, and sharded cache sets without reading or deleting source, build, report, receipt, PR, review, workflow, or agent artifacts.",
        ),
        command_entry(
            "cache gc [--dry-run] [--max-size-gb <n>] [--ttl-days <n>]",
            "argument_dependent",
            "target/ripr/cache and target/ripr/reports/cache-gc.{md,json}",
            false,
            "Depending on --dry-run, deletes only selected files under target/ripr/cache or writes the exact deletion plan without deleting files.",
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
            "check-evidence-promotion-honesty",
            "non_mutating_check",
            "target/ripr/reports/evidence-promotion-honesty.md",
            false,
            "Reads byte-pinned golden check.json files for each charter member and asserts that must_remain_non_promoted cases show no `exposed` finding and control cases retain at least one `exposed` finding; catches a dishonest golden re-bless that would bypass goldens check.",
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
            "check-doc-artifacts",
            "non_mutating_check",
            "target/ripr/reports/doc-artifacts.md",
            false,
            "Checks the source-of-truth document artifact ledger.",
        ),
        command_entry(
            "check-support-tiers",
            "non_mutating_check",
            "target/ripr/reports/support-tiers.md",
            false,
            "Checks support-tier claim proof mapping.",
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
            "check-proof-packs",
            "non_mutating_check",
            "target/ripr/reports/proof-packs.md",
            false,
            "Checks the proof-pack manifest structure; manifest-only, no routing.",
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
    let mut current_row = vec![0; rhs_len + 1];

    for (left_index, left_char) in lhs.chars().enumerate() {
        current_row[0] = left_index + 1;
        for (right_index, right_char) in rhs.chars().enumerate() {
            let insertion = current_row[right_index] + 1;
            let deletion = previous_row[right_index + 1] + 1;
            let substitution = previous_row[right_index] + usize::from(left_char != right_char);
            current_row[right_index + 1] = insertion.min(deletion).min(substitution);
        }
        std::mem::swap(&mut previous_row, &mut current_row);
    }

    previous_row[rhs_len]
}

#[cfg(test)]
mod tests {
    use super::{XtaskCommand, command_catalog, help_message, known_commands, levenshtein};

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
        assert!(
            note("ripr-plus [--gap-ledger <path>] [--repo-exposure-summary <path>]")
                .contains("canonical actionable gaps")
        );
        assert!(
            note("ripr-plus [--gap-ledger <path>] [--repo-exposure-summary <path>]")
                .contains("downstream-consumable bounded summary artifact")
        );
    }

    #[test]
    fn levenshtein_distance_handles_ascii_and_unicode_inputs() {
        assert_eq!(levenshtein("check-pr", "check-pr"), 0);
        assert_eq!(levenshtein("chek-pr", "check-pr"), 1);
        assert_eq!(levenshtein("réport", "report"), 1);
    }

    #[test]
    fn source_promotion_verify_cli_entrypoint() -> Result<(), String> {
        let command = XtaskCommand::parse([
            "source-promotion".to_string(),
            "verify".to_string(),
            "--preflight".to_string(),
            "preflight.json".to_string(),
        ]);
        match command {
            XtaskCommand::SourcePromotion(args) => {
                if args != ["verify", "--preflight", "preflight.json"] {
                    return Err(format!("unexpected source-promotion args: {args:?}"));
                }
            }
            other => return Err(format!("source-promotion did not dispatch: {other:?}")),
        }
        Ok(())
    }

    #[test]
    fn source_promotion_controller_commands_are_cataloged() -> Result<(), String> {
        let known = known_commands();
        let catalog = command_catalog();
        for (command, expected_mutability, expected_judgment) in [
            (
                "source-promotion write-trusted-builder-receipt --source-parent <sha> --workflow-source-sha <sha> --executable <path> --cargo-target-dir <path> --locked-build --isolated-target-dir [--out <dir>]",
                "report_only",
                false,
            ),
            (
                "source-promotion admit-resolved-tree --source-parent <sha> --swarm-parent <sha> --join-tree <tree> --preflight <path> --preflight-sha256 <digest> --resolution-manifest <path> --resolution-sha256 <digest> --validation-packet <dir> --builder-packet <dir> --integration-index <path> --integration-index-sha256 <digest> [--out <dir>]",
                "report_only",
                false,
            ),
            (
                "source-promotion construct-exact-join --admission-packet <dir> --validation-packet <dir> --integration-index <path> --integration-index-sha256 <digest> --preflight <path> --resolution-manifest <path> --qualification-receipt <path> --qualification-receipt-sha256 <digest> --source-main-ref <ref> --swarm-ref <ref> --candidate-ref <ref> [--out <dir>]",
                "mutating",
                false,
            ),
            (
                "source-promotion publish-candidate-ref --construction-packet <dir> --source-main-ref <ref> --remote origin --target-ref <refs/heads/promote/0.11.0-...> (--expected-absent | --expected-old <sha>) [--out <dir>]",
                "external_state_mutating",
                true,
            ),
        ] {
            if !known.contains(&command) {
                return Err(format!(
                    "source-promotion controller command is missing from help: {command}"
                ));
            }
            let entry = catalog
                .iter()
                .find(|entry| entry.command == command)
                .ok_or_else(|| {
                    format!("source-promotion controller command is not cataloged: {command}")
                })?;
            if entry.mutability != expected_mutability
                || entry.judgment_required != expected_judgment
            {
                return Err(format!(
                    "source-promotion controller command has incorrect safety metadata: {command}"
                ));
            }
        }
        Ok(())
    }

    fn source_promotion_workflow() -> Result<String, String> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .ok_or_else(|| "xtask manifest has no repository parent".to_string())?;
        std::fs::read_to_string(root.join(".github/workflows/source-promotion-contract.yml"))
            .map_err(|error| format!("failed to read source-promotion workflow: {error}"))
    }

    #[test]
    fn source_promotion_workflow_is_exact_head_and_read_only() -> Result<(), String> {
        let workflow = source_promotion_workflow()?;
        for needle in [
            "fetch-depth: 0",
            "ref: ${{ github.event.pull_request.head.sha }}",
            "join_head",
            "source-promotion-control: [0-9a-f]{40}",
            "marker_count",
            "grep -E '^<!-- source-promotion-control: [0-9a-f]{40} -->$'",
            "exactly one lowercase source-promotion-control marker",
            "git -C \"$control_dir\" fetch --no-tags origin \"$control_commit\"",
            "CONTROL_INPUTS_PATH: docs/release/source-promotion/contract-inputs.json",
            "PREFLIGHT_PATH: docs/release/source-promotion/preflight.json",
            "RESOLUTION_MANIFEST_PATH: docs/release/source-promotion/resolution-manifest.json",
            "ripr.source_promotion_ci_inputs.v2",
            "preflight_sha256",
            "git -C \"$control_dir\" show \"$control_commit:$PREFLIGHT_PATH\"",
            "--match-head-commit $PR_HEAD",
            "Build verifier from trusted base source",
            "\"$TRUSTED_VERIFIER\" source-promotion verify",
            "--main-head \"$MAIN_HEAD\"",
            "toolchain: 1.95.0",
            "PR_NUMBER: ${{ github.event.pull_request.number }}",
            "resolution_sha256",
            "validation_phase",
            "validation_reason",
            "validation_status",
            "version",
            "source_parent",
            "source_parent:$source_parent",
            "control_sidecars",
            "control_preflight_path",
            "control_resolution_manifest_path",
            "phase=$validation_phase",
            "reason=$validation_reason",
            "if: steps.inputs.outcome == 'success'",
            "if: steps.inputs.outcome == 'success' && steps.live-governance.outcome == 'success'",
            "source-promotion-validation.log",
            "mkdir -p \"$out\"",
            "trusted verifier checkout identity mismatch",
            "jq -e . \"$PREFLIGHT\"",
            "control preflight digest mismatch",
            "control resolution digest mismatch",
            "actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a # v7",
            "(.conditions.ref_name.exclude // []) == []",
            "permissions:\n  contents: read",
            "<!-- source-promotion: true -->",
            "This workflow never executes the merge command",
        ] {
            if !workflow.contains(needle) {
                return Err(format!(
                    "source-promotion workflow lost required contract: {needle}"
                ));
            }
        }
        if workflow
            .lines()
            .any(|line| line.trim_start().starts_with("gh pr merge"))
        {
            return Err(
                "source-promotion workflow must print, not execute, gh pr merge".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn source_promotion_workflow_rejects_symlink_and_path_escape_inputs() -> Result<(), String> {
        let workflow = source_promotion_workflow()?;
        for needle in [
            "validate_tracked_regular_file",
            "git -C \"$control_dir\" ls-tree \"$control_commit\"",
            "control input is not a tracked regular file",
            "control input path is not canonical",
            "fixed source repository",
        ] {
            if !workflow.contains(needle) {
                return Err(format!(
                    "workflow lacks symlink/path-escape guard: {needle}"
                ));
            }
        }
        for forbidden in [
            "git show \"$PR_HEAD:$CONTROL_INPUTS_PATH\"",
            "git show \"$PR_HEAD:$PREFLIGHT_PATH\"",
            "git show \"$PR_HEAD:$RESOLUTION_MANIFEST_PATH\"",
            "validate_tracked_regular_file \"$INPUTS_PATH\"",
        ] {
            if workflow.contains(forbidden) {
                return Err(format!(
                    "promotion workflow retained candidate-checkout authority: {forbidden}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn source_promotion_workflow_requires_external_fixed_sidecar() -> Result<(), String> {
        let workflow = source_promotion_workflow()?;
        for needle in [
            "CONTROL_REPOSITORY_URL: https://github.com/EffortlessMetrics/ripr.git",
            "git -C \"$control_dir\" rev-parse --verify \"$control_commit^{commit}\"",
            "test \"$(git -C \"$control_dir\" remote get-url origin)\" = \"$CONTROL_REPOSITORY_URL\"",
            "validate_tracked_regular_file \"$CONTROL_INPUTS_PATH\"",
            "validate_tracked_regular_file \"$PREFLIGHT_PATH\"",
            "validate_tracked_regular_file \"$RESOLUTION_MANIFEST_PATH\"",
            "sha256sum \"$preflight\"",
            "sha256sum \"$resolution_manifest\"",
            "control_commit:$control_commit",
            "ripr.source_promotion_post_merge_contract.v1",
            "source-promotion-post-merge-contract.json",
            "--arg control_commit \"$CONTROL_COMMIT\"",
            "CONTROL_COMMIT: ${{ inputs.control_commit }}",
            "if: always()",
        ] {
            if !workflow.contains(needle) {
                return Err(format!("workflow lacks immutable sidecar guard: {needle}"));
            }
        }
        for forbidden in [
            "validate_tracked_regular_file \"$INPUTS_PATH\"",
            "          INPUTS_PATH: docs/release/source-promotion/contract-inputs.json",
        ] {
            if workflow.contains(forbidden) {
                return Err(format!(
                    "workflow retained candidate-checkout input authority: {forbidden}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn source_promotion_workflow_binds_trusted_source_parent() -> Result<(), String> {
        let workflow = source_promotion_workflow()?;
        for needle in [
            "EXPECTED_SOURCE_PARENT: ${{ inputs.source_parent }}",
            "source_parent must be an exact lowercase SHA",
            "test \"$EXPECTED_SOURCE_PARENT\" = \"$source_main\" || fail \"source_parent must equal control source_main\"",
            "control commit must not be an ancestor of the join head",
            "control commit must not be a descendant of the join head",
        ] {
            if !workflow.contains(needle) {
                return Err(format!(
                    "workflow lacks trusted source-parent binding: {needle}"
                ));
            }
        }
        if workflow.contains("EXPECTED_SOURCE_PARENT: ${{ inputs.source_main }}") {
            return Err("workflow aliases source_parent from the wrong dispatch input".into());
        }
        Ok(())
    }

    #[test]
    fn source_promotion_post_merge_receipt_binds_dispatch_input_on_rejection() -> Result<(), String>
    {
        let workflow = source_promotion_workflow()?;
        let receipt = workflow
            .find("- name: Write SHA-bound post-merge contract receipt")
            .ok_or_else(|| "post-merge receipt step is missing".to_string())?;
        let post_merge = &workflow[receipt..];
        for needle in [
            "if: always()",
            "CONTROL_COMMIT: ${{ inputs.control_commit }}",
            "source-promotion-post-merge-contract.json",
            "ripr.source_promotion_post_merge_contract.v1",
        ] {
            if !post_merge.contains(needle) {
                return Err(format!("post-merge rejection receipt lacks: {needle}"));
            }
        }
        if post_merge.contains("CONTROL_COMMIT: ${{ steps.control.outputs.control_commit }}") {
            return Err("post-merge receipt depends on a failed validation step output".into());
        }
        Ok(())
    }

    #[test]
    fn source_promotion_workflow_rejects_placeholder_and_wrong_repo_commands() -> Result<(), String>
    {
        let workflow = source_promotion_workflow()?;
        for needle in [
            "gh pr merge $PR_NUMBER --repo EffortlessMetrics/ripr",
            "merge command must bind numeric PR",
            "merge_command=$(printf '%s\\n' \"$merge_block\"",
            "exactly one canonical merge command is required",
        ] {
            if !workflow.contains(needle) {
                return Err(format!("workflow lacks merge-command guard: {needle}"));
            }
        }
        Ok(())
    }

    #[test]
    fn source_promotion_workflow_rejects_candidate_verifier_bypass() -> Result<(), String> {
        let workflow = source_promotion_workflow()?;
        for needle in [
            "Build verifier from trusted base source",
            "git -C \"$trusted_dir\" checkout --detach \"$SOURCE_PARENT\"",
            "cargo build --locked --manifest-path \"$trusted_dir/Cargo.toml\" -p xtask --bin xtask",
            "TRUSTED_VERIFIER",
            "rev-parse HEAD)\" = \"$SOURCE_PARENT",
        ] {
            if !workflow.contains(needle) {
                return Err(format!("workflow lacks trusted-verifier guard: {needle}"));
            }
        }
        if workflow.contains("cargo build --manifest-path \"$trusted_dir/Cargo.toml\" --bin xtask")
        {
            return Err("candidate-verifier build shape is still accepted".into());
        }
        if workflow.matches("cargo build --locked --manifest-path \"$trusted_dir/Cargo.toml\" -p xtask --bin xtask").count() != 2 {
            return Err("both trusted verifier lanes must use the locked xtask package binary".into());
        }
        if workflow
            .contains("cargo build --locked --manifest-path \"$GITHUB_WORKSPACE/Cargo.toml\"")
            || workflow.contains("./target/debug/xtask source-promotion verify")
        {
            return Err("candidate checkout can still supply the verifier".into());
        }
        Ok(())
    }

    #[test]
    fn source_promotion_workflow_receipt_contract_distinguishes_success_missing_and_mismatch()
    -> Result<(), String> {
        let workflow = source_promotion_workflow()?;
        for needle in [
            "SOURCE_PROMOTION_OUT: ${{ runner.temp }}/ripr-source-promotion",
            "SOURCE_PARENT: ${{ steps.inputs.outputs.source_parent }}",
            "--out \"$SOURCE_PROMOTION_OUT\"",
            "out=\"$SOURCE_PROMOTION_OUT\"",
            "verification=\"$out/source-promotion-verification.json\"",
            "type == \"object\" and .schema == \"ripr.source_promotion_verification.v2\"",
            "verifier_receipt_status=present",
            "verifier_receipt_status=missing",
            "verifier_receipt_status=schema_mismatch",
            "verifier receipt schema mismatch",
            "trusted verifier receipt missing",
            "verifier_receipt_status:$verifier_receipt_status",
            "verifier_exit_code:$verifier_exit_code",
            "TRUSTED_VERIFIER_SHA: ${{ steps.trusted-verifier.outputs.sha }}",
            "LIVE_TAG: ${{ steps.live-governance.outputs.tag }}",
            "LIVE_RULESET: ${{ steps.live-governance.outputs.ruleset }}",
            "(.join_head | type) == \"string\"",
            "(.source_main | type) == \"string\"",
            "(.parents | type) == \"array\"",
            "(.swarm_reachability | type) == \"object\"",
            "(.release_metadata_surfaces | type) == \"array\"",
            "(.checks | type) == \"object\"",
            "(.failure_reasons | type) == \"array\"",
            "(.invalidation_rules | type) == \"array\"",
            "(.non_claims | type) == \"array\"",
            "(.status == \"rejected\" or ((.tree | type) == \"string\"",
            "(.parents | length) == 2",
            "all(.parents[]; type == \"string\")",
            "(.swarm_reachability.all_reachable_count | type) == \"number\"",
            "trusted verifier exited non-zero",
        ] {
            if !workflow.contains(needle) {
                return Err(format!(
                    "receipt contract lacks discriminating branch: {needle}"
                ));
            }
        }
        if workflow.contains("echo \"- PR head / candidate SHA: `")
            || workflow.contains("echo \"Failure reasons: `")
        {
            return Err("receipt summary still executes interpolated Markdown backticks".into());
        }
        Ok(())
    }

    #[test]
    fn source_promotion_workflow_has_failure_fixture_corpus() -> Result<(), String> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .ok_or_else(|| "xtask manifest has no repository parent".to_string())?
            .join("tests/fixtures/source_promotion_contract");
        for (name, expected_phase, expected_reason) in [
            (
                "malformed-control.json",
                "control_marker",
                "exactly one lowercase source-promotion-control marker is required",
            ),
            (
                "unreachable-control.json",
                "control_repository",
                "control commit is unreachable from the source repository",
            ),
            (
                "source-mismatch.json",
                "control_identity",
                "control source_main does not equal the PR base SHA",
            ),
            (
                "missing-receipt.json",
                "verifier_receipt",
                "trusted verifier receipt missing",
            ),
            (
                "schema-mismatch.json",
                "verifier_receipt",
                "trusted verifier receipt schema mismatch",
            ),
            (
                "empty-artifact-dir.json",
                "verifier_receipt",
                "trusted verifier receipt missing",
            ),
        ] {
            let path = root.join(name);
            let text = std::fs::read_to_string(&path)
                .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
            let value: serde_json::Value = serde_json::from_str(&text)
                .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
            for key in ["case", "validation_phase", "validation_reason", "status"] {
                if value.get(key).and_then(serde_json::Value::as_str).is_none() {
                    return Err(format!("{} is missing string field {key}", path.display()));
                }
            }
            if value["status"] != "rejected" {
                return Err(format!("{} must pin fail-closed rejection", path.display()));
            }
            if value["validation_phase"] != expected_phase {
                return Err(format!(
                    "{} must pin emitted validation phase {expected_phase}",
                    path.display()
                ));
            }
            if value["validation_reason"] != expected_reason {
                return Err(format!(
                    "{} must pin emitted validation reason {expected_reason}",
                    path.display()
                ));
            }
            if matches!(name, "missing-receipt.json" | "empty-artifact-dir.json")
                && value["receipt_status"] != "missing"
            {
                return Err(format!(
                    "{} must pin a missing verifier receipt",
                    path.display()
                ));
            }
            if name == "schema-mismatch.json" && value["receipt_status"] != "schema_mismatch" {
                return Err(format!(
                    "{} must pin a schema-mismatched verifier receipt",
                    path.display()
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn source_promotion_workflow_builds_xtask_explicitly_from_trusted_source() -> Result<(), String>
    {
        let workflow = source_promotion_workflow()?;
        let expected = "CARGO_TARGET_DIR=\"$trusted_target\" cargo build --locked --manifest-path \"$trusted_dir/Cargo.toml\" -p xtask --bin xtask";
        if workflow.contains(
            "CARGO_TARGET_DIR=\"$trusted_target\" cargo build --manifest-path \"$trusted_dir/Cargo.toml\" --bin xtask",
        ) {
            return Err("trusted-verifier build regressed to default-package selection".into());
        }

        for (label, next_label) in [
            (
                "Build verifier from trusted base source",
                "Verify promotion inputs and PR-head binding",
            ),
            (
                "Build verifier from trusted source parent",
                "Verify exact J reaches merged source main",
            ),
        ] {
            let start = workflow
                .find(label)
                .ok_or_else(|| format!("trusted-verifier lane is missing: {label}"))?;
            let lane = &workflow[start..];
            let end = lane.find(next_label).ok_or_else(|| {
                format!("trusted-verifier lane has no expected end marker: {label}")
            })?;
            let lane = &lane[..end];
            let build_count = lane.matches(expected).count();
            if build_count != 1 {
                return Err(format!(
                    "trusted-verifier lane {label:?} must contain exactly one explicit xtask build; found {build_count}"
                ));
            }
            for needle in [
                "verifier=\"$trusted_target/debug/xtask\"",
                "test -x \"$verifier\"",
            ] {
                if !lane.contains(needle) {
                    return Err(format!(
                        "trusted-verifier lane {label:?} must validate the selected xtask binary path: {needle}"
                    ));
                }
            }
        }
        Ok(())
    }

    #[test]
    fn source_promotion_workflow_rejects_mixed_merge_strategies() -> Result<(), String> {
        let workflow = source_promotion_workflow()?;
        for needle in [
            "exactly one canonical merge command is required",
            "--squash",
            "--rebase",
            "exactly one --merge strategy is required",
        ] {
            if !workflow.contains(needle) {
                return Err(format!("workflow lacks merge-strategy guard: {needle}"));
            }
        }
        Ok(())
    }

    #[test]
    fn source_promotion_workflow_accepts_documented_multiline_merge_command() -> Result<(), String>
    {
        let workflow = source_promotion_workflow()?;
        for needle in [
            "__RIPR_MERGE_BLOCK__",
            "exactly one fenced bash block may contain the merge command",
            "sed 's/\\\\$//' | tr '\\n' ' '",
            "gh pr merge $PR_NUMBER --repo EffortlessMetrics/ripr --merge --match-head-commit $PR_HEAD",
        ] {
            if !workflow.contains(needle) {
                return Err(format!(
                    "workflow lacks multiline merge acceptance: {needle}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn source_promotion_workflow_scans_all_bash_fences_for_merge_command() -> Result<(), String> {
        let workflow = source_promotion_workflow()?;
        for needle in [
            "inside && /```/",
            "if (block ~ /gh pr merge/)",
            "merge_block_count",
            "exactly one fenced bash block may contain the merge command",
        ] {
            if !workflow.contains(needle) {
                return Err(format!(
                    "workflow lacks multi-fence merge parsing: {needle}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn source_promotion_workflow_rejects_multiple_merge_fences() -> Result<(), String> {
        let workflow = source_promotion_workflow()?;
        if !workflow.contains("test \"$merge_block_count\" -eq 1") {
            return Err("workflow does not reject multiple merge-containing fences".to_string());
        }
        Ok(())
    }

    #[test]
    fn source_promotion_workflow_disables_checkout_credentials_before_code() -> Result<(), String> {
        let workflow = source_promotion_workflow()?;
        let count = workflow.matches("persist-credentials: false").count();
        if count != 2 {
            return Err(format!(
                "expected both source-promotion checkouts to disable credentials, found {count}"
            ));
        }
        Ok(())
    }

    #[test]
    fn source_promotion_workflow_refutes_crlf_rewrite_thread() -> Result<(), String> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .ok_or_else(|| "xtask manifest has no repository parent".to_string())?;
        let attributes = std::fs::read_to_string(root.join(".gitattributes"))
            .map_err(|error| format!("failed to read .gitattributes: {error}"))?;
        if !attributes.contains("* text=auto eol=lf") {
            return Err(".gitattributes does not enforce LF text checkout".to_string());
        }
        Ok(())
    }
}
#[path = "command/help.rs"]
mod help;

use help::{format_help_entries, format_top_level_help};
