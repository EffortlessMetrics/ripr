use crate::command::{XtaskCommand, print_help, unknown_command_message};

pub(crate) fn execute(command: XtaskCommand) -> Result<(), String> {
    match command {
        XtaskCommand::Shape => super::shape(),
        XtaskCommand::FixPr => super::fix_pr(),
        XtaskCommand::InstallHooks(args) => super::install_hooks(&args),
        XtaskCommand::PrSummary => super::reports::pr_summary(),
        XtaskCommand::Precommit => super::precommit(),
        XtaskCommand::CheckPr => super::check_pr(),
        XtaskCommand::Fixtures(name) => super::reports::fixtures(name.as_ref()),
        XtaskCommand::Goldens(args) => super::reports::goldens(&args),
        XtaskCommand::Metrics => super::reports::metrics_report(),
        XtaskCommand::TestOracleReport => super::reports::test_oracle_report(),
        XtaskCommand::TestEfficiencyReport => super::reports::test_efficiency_report(),
        XtaskCommand::BadgeArtifacts => super::reports::badge_artifacts(),
        XtaskCommand::RepoBadgeArtifacts => super::reports::repo_badge_artifacts(),
        XtaskCommand::RepoSeamInventory => super::reports::repo_seam_inventory(),
        XtaskCommand::RepoExposureReport => super::reports::repo_exposure_report(),
        XtaskCommand::RepoExposureLatencyReport => super::reports::repo_exposure_latency_report(),
        XtaskCommand::EvidenceHealth => super::reports::evidence_health_report(),
        XtaskCommand::Lane1EvidenceAudit => super::reports::lane1_evidence_audit_report(),
        XtaskCommand::EvidenceQualityScorecard => {
            super::reports::evidence_quality_scorecard_report()
        }
        XtaskCommand::EvidenceQualityTrend(args) => {
            super::reports::evidence_quality_trend_report(&args)
        }
        XtaskCommand::AgentSeamPackets(root) => {
            super::reports::agent_seam_packets_report(root.as_ref())
        }
        XtaskCommand::LspCockpitReport => super::reports::lsp_cockpit_report(),
        XtaskCommand::OperatorCockpitReport => super::reports::operator_cockpit_report(),
        XtaskCommand::ReleaseReadiness(args) => super::reports::release_readiness(&args),
        XtaskCommand::ReleaseServerArchive(args) => super::release_server_archive(&args),
        XtaskCommand::ReleaseServerManifest(args) => super::release_server_manifest(&args),
        XtaskCommand::ReleaseUploadAssets(args) => super::release_upload_assets(&args),
        XtaskCommand::TargetedTestOutcome(args) => super::reports::targeted_test_outcome(&args),
        XtaskCommand::MutationCalibration(args) => super::reports::mutation_calibration(&args),
        XtaskCommand::RecommendationCalibration(args) => {
            super::reports::recommendation_calibration(&args)
        }
        XtaskCommand::SarifPolicy(args) => super::reports::sarif_policy(&args),
        XtaskCommand::UpdateBadgeEndpoints => super::reports::update_badge_endpoints(),
        XtaskCommand::CheckBadgeEndpoints => super::reports::check_badge_endpoints(),
        XtaskCommand::Dogfood => super::reports::dogfood(),
        XtaskCommand::Critic => super::reports::critic(),
        XtaskCommand::Goals(args) => super::goals(&args),
        XtaskCommand::Reports(args) => super::reports::reports(&args),
        XtaskCommand::Receipts(args) => super::reports::receipts(&args),
        XtaskCommand::Worktree(args) => super::worktree(&args),
        XtaskCommand::Specs(args) => super::specs(&args),
        XtaskCommand::GoldenDrift => super::reports::golden_drift(),
        XtaskCommand::CiFast => super::ci_fast(),
        XtaskCommand::CiFull => super::ci_full(),
        XtaskCommand::CheckStaticLanguage => super::check_static_language(),
        XtaskCommand::CheckNoPanicFamily(args) => super::check_no_panic_family_with_args(&args),
        XtaskCommand::CheckAllowAttributes => super::check_allow_attributes(),
        XtaskCommand::CheckLocalContext => super::check_local_context(),
        XtaskCommand::CheckFilePolicy => super::check_file_policy(),
        XtaskCommand::CheckExecutableFiles => super::check_executable_files(),
        XtaskCommand::CheckWorkflows => super::check_workflows(),
        XtaskCommand::CheckDroidReviewConfig => super::check_droid_review_config(),
        XtaskCommand::CheckSpecFormat => super::check_spec_format(),
        XtaskCommand::CheckSpecNumbering => super::check_spec_numbering(),
        XtaskCommand::CheckFixtureContracts => super::check_fixture_contracts(),
        XtaskCommand::CheckTraceability => super::check_traceability(),
        XtaskCommand::CheckCapabilities => super::check_capabilities(),
        XtaskCommand::CheckWorkspaceShape => super::check_workspace_shape(),
        XtaskCommand::CheckArchitecture => super::check_architecture(),
        XtaskCommand::CheckPublicApi => super::check_public_api(),
        XtaskCommand::CheckOutputContracts => super::check_output_contracts(),
        XtaskCommand::CheckDocIndex => super::check_doc_index(),
        XtaskCommand::CheckReadmeState => super::check_readme_state(),
        XtaskCommand::MarkdownLinks => super::markdown_links(),
        XtaskCommand::CheckCampaign => super::check_campaign(),
        XtaskCommand::CheckPrShape => super::check_pr_shape(),
        XtaskCommand::CheckGenerated => super::check_generated(),
        XtaskCommand::CheckBadgeDiffPolicy => super::check_badge_diff_policy(),
        XtaskCommand::CheckGeneratedClean => super::check_generated_clean(),
        XtaskCommand::CheckDependencies => super::check_dependencies(),
        XtaskCommand::CheckSupplyChain => super::check_supply_chain(),
        XtaskCommand::CheckProcessPolicy => super::check_process_policy(),
        XtaskCommand::CheckNetworkPolicy => super::check_network_policy(),
        XtaskCommand::CheckLintPolicy => super::check_lint_policy(),
        XtaskCommand::CheckCiLaneWhitelist => super::check_ci_lane_whitelist(),
        XtaskCommand::CheckProductCopy => super::check_product_copy(),
        XtaskCommand::CheckPositioningLanguage => super::check_positioning_language(),
        XtaskCommand::CheckDocRoles => super::check_doc_roles(),
        XtaskCommand::VscodeCompile => super::vscode_compile(),
        XtaskCommand::VscodePackage => super::vscode_package(),
        XtaskCommand::VscodeTest => super::vscode_test(),
        XtaskCommand::VscodeTestE2e => super::vscode_test_e2e(),
        XtaskCommand::Package => {
            super::run("cargo", &["package", "-p", "ripr", "--list"]).map(|_| ())
        }
        XtaskCommand::PublishDryRun => {
            super::run("cargo", &["publish", "-p", "ripr", "--dry-run"]).map(|_| ())
        }
        XtaskCommand::Help => {
            print_help();
            Ok(())
        }
        XtaskCommand::Unknown(command) => Err(unknown_command_message(&command)),
    }
}
