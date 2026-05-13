#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum XtaskCommand {
    Shape,
    FixPr,
    InstallHooks(Vec<String>),
    PrSummary,
    Precommit,
    CheckPr,
    Fixtures(Option<String>),
    Goldens(Vec<String>),
    Metrics,
    TestOracleReport,
    TestEfficiencyReport,
    BadgeArtifacts,
    RepoBadgeArtifacts,
    RepoSeamInventory,
    RepoExposureReport,
    RepoExposureLatencyReport,
    EvidenceHealth,
    Lane1EvidenceAudit,
    EvidenceQualityScorecard,
    AgentSeamPackets(Option<String>),
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
    UpdateBadgeEndpoints,
    CheckBadgeEndpoints,
    Dogfood,
    Critic,
    Goals(Vec<String>),
    Reports(Vec<String>),
    Receipts(Vec<String>),
    GoldenDrift,
    CiFast,
    CiFull,
    CheckStaticLanguage,
    CheckNoPanicFamily(Vec<String>),
    CheckAllowAttributes,
    CheckLocalContext,
    CheckFilePolicy,
    CheckExecutableFiles,
    CheckWorkflows,
    CheckDroidReviewConfig,
    CheckSpecFormat,
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
    Help,
    Unknown(String),
}

impl XtaskCommand {
    pub(crate) fn parse(args: impl IntoIterator<Item = String>) -> Self {
        let mut args = args.into_iter();
        let Some(command) = args.next() else {
            return Self::Help;
        };
        let rest: Vec<String> = args.collect();
        match command.as_str() {
            "shape" => Self::Shape,
            "fix-pr" => Self::FixPr,
            "install-hooks" => Self::InstallHooks(rest),
            "pr-summary" => Self::PrSummary,
            "precommit" => Self::Precommit,
            "check-pr" => Self::CheckPr,
            "fixtures" => Self::Fixtures(rest.first().cloned()),
            "goldens" => Self::Goldens(rest),
            "metrics" => Self::Metrics,
            "test-oracle-report" | "check-test-oracles" => Self::TestOracleReport,
            "test-efficiency-report" => Self::TestEfficiencyReport,
            "badge-artifacts" => Self::BadgeArtifacts,
            "repo-badge-artifacts" => Self::RepoBadgeArtifacts,
            "repo-seam-inventory" => Self::RepoSeamInventory,
            "repo-exposure-report" => Self::RepoExposureReport,
            "repo-exposure-latency-report" => Self::RepoExposureLatencyReport,
            "evidence-health" => Self::EvidenceHealth,
            "lane1-evidence-audit" | "evidence-quality-audit" => Self::Lane1EvidenceAudit,
            "evidence-quality-scorecard" => Self::EvidenceQualityScorecard,
            "agent-seam-packets" => Self::AgentSeamPackets(rest.first().cloned()),
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
            "badges" if rest.iter().any(|arg| arg == "--check") => Self::CheckBadgeEndpoints,
            "badges" => Self::UpdateBadgeEndpoints,
            "update-badge-endpoints" => Self::UpdateBadgeEndpoints,
            "check-badge-endpoints" => Self::CheckBadgeEndpoints,
            "dogfood" => Self::Dogfood,
            "critic" => Self::Critic,
            "goals" => Self::Goals(rest),
            "reports" => Self::Reports(rest),
            "receipts" => Self::Receipts(rest),
            "golden-drift" => Self::GoldenDrift,
            "ci-fast" => Self::CiFast,
            "ci-full" => Self::CiFull,
            "check-static-language" => Self::CheckStaticLanguage,
            "check-no-panic-family" => Self::CheckNoPanicFamily(rest),
            "check-allow-attributes" => Self::CheckAllowAttributes,
            "check-local-context" => Self::CheckLocalContext,
            "check-file-policy" => Self::CheckFilePolicy,
            "check-executable-files" => Self::CheckExecutableFiles,
            "check-workflows" => Self::CheckWorkflows,
            "check-droid-review-config" => Self::CheckDroidReviewConfig,
            "check-spec-format" => Self::CheckSpecFormat,
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
            "help" => Self::Help,
            other => Self::Unknown(other.to_string()),
        }
    }
}

pub(crate) fn print_help() {
    let commands = known_commands().join("\n  ");
    println!("xtask commands:\n  {commands}");
}

pub(crate) fn known_commands() -> Vec<&'static str> {
    vec![
        "shape",
        "fix-pr",
        "install-hooks",
        "pr-summary",
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
        "repo-badge-artifacts",
        "repo-seam-inventory",
        "repo-exposure-report",
        "repo-exposure-latency-report",
        "evidence-health",
        "lane1-evidence-audit",
        "evidence-quality-audit",
        "evidence-quality-scorecard",
        "agent-seam-packets [root]",
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
        "badges",
        "update-badge-endpoints",
        "check-badge-endpoints",
        "dogfood",
        "critic",
        "goals status|next|report",
        "reports index",
        "receipts [check]",
        "ci-fast",
        "ci-full",
        "check-static-language",
        "check-no-panic-family [--propose]",
        "check-allow-attributes",
        "check-local-context",
        "check-file-policy",
        "check-executable-files",
        "check-workflows",
        "check-droid-review-config",
        "check-spec-format",
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
