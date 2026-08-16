use crate::command::{XtaskCommand, print_help, unknown_command_message};

#[path = "precommit_v2.rs"]
mod precommit_v2;

pub(crate) fn execute(command: XtaskCommand) -> Result<(), String> {
    match command {
        XtaskCommand::Shape => super::shape(),
        XtaskCommand::FixPr => super::fix_pr(),
        XtaskCommand::InstallHooks(args) => super::install_hooks(&args),
        XtaskCommand::Commands => super::commands_report(),
        XtaskCommand::PrSummary => super::reports::pr_summary(),
        XtaskCommand::Proof(args) => super::reports::proof(&args),
        XtaskCommand::PrReady => super::pr_ready(),
        XtaskCommand::Cockpit => super::cockpit(),
        XtaskCommand::PrTriageReport => super::reports::pr_triage_report(),
        XtaskCommand::BranchInventory(args) => super::branch_inventory::run(&args),
        XtaskCommand::GhPrStatus(args) => super::reports::gh_pr_status(&args),
        XtaskCommand::CiBudget(args) => super::reports::ci_budget(&args),
        XtaskCommand::ModuleHealth(args) => super::reports::module_health(&args),
        XtaskCommand::WindowsAdvisorySummary(args) => super::windows_advisory::run(&args),
        XtaskCommand::EvalSweep(args) => super::reports::eval_sweep(&args),
        XtaskCommand::SuggestedFixes => super::suggested_fixes(),
        XtaskCommand::Precommit => precommit_v2::run(),
        XtaskCommand::CheckFast => super::check_fast(),
        XtaskCommand::CheckPr => super::check_pr(),
        XtaskCommand::Fixtures(args) => super::reports::fixtures_with_args(&args),
        XtaskCommand::Goldens(args) => super::reports::goldens(&args),
        XtaskCommand::Metrics => super::reports::metrics_report(),
        XtaskCommand::RustRepairTrustReport => super::reports::rust_repair_trust_report(),
        XtaskCommand::TestOracleReport => super::reports::test_oracle_report(),
        XtaskCommand::TestEfficiencyReport => super::reports::test_efficiency_report(),
        XtaskCommand::BadgeArtifacts => super::reports::badge_artifacts(),
        XtaskCommand::RepoBadgeArtifacts(args) => super::reports::repo_badge_artifacts(&args),
        XtaskCommand::BadgeBasis(args) => super::reports::badge_basis(&args),
        XtaskCommand::RiprPlus(args) => super::reports::ripr_plus(&args),
        XtaskCommand::RepoSeamInventory => super::reports::repo_seam_inventory(),
        XtaskCommand::RepoExposureReport => super::reports::repo_exposure_report(),
        XtaskCommand::RepoExposureSummaryReport => super::reports::repo_exposure_summary_report(),
        XtaskCommand::RepoExposureLatencyReport => super::reports::repo_exposure_latency_report(),
        XtaskCommand::TargetedRerunBenchmark(args) => {
            super::reports::targeted_rerun_benchmark(&args)
        }
        XtaskCommand::RepoContractReport => super::repo_contract_report(),
        XtaskCommand::EvidenceHealth => super::reports::evidence_health_report(),
        XtaskCommand::Lane1EvidenceAudit => super::reports::lane1_evidence_audit_report(),
        XtaskCommand::EvidenceQualityScorecard => {
            super::reports::evidence_quality_scorecard_report()
        }
        XtaskCommand::EvidenceQualityTrend(args) => {
            super::reports::evidence_quality_trend_report(&args)
        }
        XtaskCommand::ActionableGapOutcomes(args) => {
            super::reports::actionable_gap_outcomes_report(&args)
        }
        XtaskCommand::AgentSeamPackets(root) => {
            super::reports::agent_seam_packets_report(root.as_ref())
        }
        XtaskCommand::RiprSwarm(args) => super::ripr_swarm(&args),
        XtaskCommand::RouteQuality(args) => super::ripr_swarm_route_quality_report(&args),
        XtaskCommand::LspCockpitReport => super::reports::lsp_cockpit_report(),
        XtaskCommand::OperatorCockpitReport => super::reports::operator_cockpit_report(),
        XtaskCommand::ReleaseReadiness(args) => super::reports::release_readiness(&args),
        XtaskCommand::ReleaseNegativeCorpus(args) => super::reports::release_negative_corpus(&args),
        XtaskCommand::BumpVersion(args) => super::version::bump_version(&args),
        XtaskCommand::ReleaseControl(args) => super::reports::release_control(&args),
        XtaskCommand::ReleaseDenominator(args) => super::reports::release_denominator(&args),
        XtaskCommand::SourcePromotion(args) => {
            if args.first().is_some_and(|arg| arg == "verify") {
                super::reports::source_promotion_verify(&args)
            } else {
                super::reports::source_promotion(&args)
            }
        }
        XtaskCommand::BackSync(args) => super::reports::back_sync(&args),
        XtaskCommand::ReleaseScope(args) => super::reports::release_scope(&args),
        XtaskCommand::ReleaseServerArchive(args) => {
            super::reports::release_server::release_server_archive(&args)
        }
        XtaskCommand::ReleaseServerManifest(args) => {
            super::reports::release_server::release_server_manifest(&args)
        }
        XtaskCommand::ReleaseUploadAssets(args) => {
            super::reports::release_server::release_upload_assets(&args)
        }
        XtaskCommand::TargetedTestOutcome(args) => super::reports::targeted_test_outcome(&args),
        XtaskCommand::MutationCalibration(args) => super::reports::mutation_calibration(&args),
        XtaskCommand::BunUbCalibration(args) => super::reports::bun_ub_calibration(&args),
        XtaskCommand::BunUbPreviewSummary(args) => super::reports::bun_ub_preview_summary(&args),
        XtaskCommand::ConfiguredBridgeInventory(args) => {
            super::reports::configured_bridge_inventory(&args)
        }
        XtaskCommand::RecommendationCalibration(args) => {
            super::reports::recommendation_calibration(&args)
        }
        XtaskCommand::SarifPolicy(args) => super::reports::sarif_policy(&args),
        XtaskCommand::ImpactedEvidence(args) => super::reports::impacted_evidence(&args),
        XtaskCommand::RiprPr(args) => super::reports::ripr_pr(&args),
        XtaskCommand::FirstPr(args) => super::reports::first_pr(&args),
        XtaskCommand::RiprReviewComments(args) => super::reports::ripr_review_comments(&args),
        XtaskCommand::RiprPrSummary(args) => super::reports::ripr_pr_summary(&args),
        XtaskCommand::RiprAnnotations(args) => super::reports::ripr_annotations(&args),
        XtaskCommand::UpdateBadgeEndpoints(args) => super::reports::update_badge_endpoints(&args),
        XtaskCommand::CheckBadgeEndpoints(args) => super::reports::check_badge_endpoints(&args),
        XtaskCommand::Dogfood => super::reports::dogfood(),
        XtaskCommand::Critic => super::reports::critic(),
        XtaskCommand::Reports(args) => super::reports::reports(&args),
        XtaskCommand::Cache(args) => super::cache::run(&args),
        XtaskCommand::Receipts(args) => super::reports::receipts(&args),
        XtaskCommand::Worktree(args) => super::worktree(&args),
        XtaskCommand::Specs(args) => super::specs(&args),
        XtaskCommand::GoldenDrift => super::reports::golden_drift(),
        XtaskCommand::CiFast => super::ci_fast(),
        XtaskCommand::CiFull => super::ci_full(),
        XtaskCommand::CheckStaticLanguage => super::check_static_language(),
        XtaskCommand::CheckAgentSkills => super::agent_skills::check(),
        XtaskCommand::CheckNoPanicFamily(args) => {
            super::no_panic::check_no_panic_family_with_args(&args)
        }
        XtaskCommand::CheckAllowAttributes => super::check_allow_attributes(),
        XtaskCommand::CheckLocalContext => super::check_local_context(),
        XtaskCommand::CheckFilePolicy => super::check_file_policy(),
        XtaskCommand::RustConversionCandidates => super::rust_conversion_candidates(),
        XtaskCommand::CheckExecutableFiles => super::check_executable_files(),
        XtaskCommand::CheckWorkflows => super::check_workflows(),
        XtaskCommand::CheckDroidReviewConfig => super::check_droid_review_config(),
        XtaskCommand::CheckSpecFormat => super::check_spec_format(),
        XtaskCommand::CheckSpecNumbering => super::check_spec_numbering(),
        XtaskCommand::CheckFixtureContracts => super::check_fixture_contracts(),
        XtaskCommand::CheckEvidencePromotionHonesty(args) => {
            super::check_evidence_promotion_honesty(&args)
        }
        XtaskCommand::CheckTraceability => super::check_traceability(),
        XtaskCommand::CheckCapabilities => super::check_capabilities(),
        XtaskCommand::CheckCampaign => super::check_campaign(),
        XtaskCommand::CheckWorkspaceShape => super::check_workspace_shape(),
        XtaskCommand::CheckArchitecture => super::check_architecture(),
        XtaskCommand::CheckPublicApi => super::check_public_api(),
        XtaskCommand::CheckOutputContracts => super::check_output_contracts(),
        XtaskCommand::CheckDocArtifacts => super::check_doc_artifacts(),
        XtaskCommand::CheckSupportTiers => super::check_support_tiers(),
        XtaskCommand::CheckDocIndex => super::check_doc_index(),
        XtaskCommand::CheckReadmeState => super::check_readme_state(),
        XtaskCommand::MarkdownLinks => super::markdown_links(),
        XtaskCommand::CheckPrShape => super::check_pr_shape(),
        XtaskCommand::CheckGenerated => super::check_generated(),
        XtaskCommand::CheckCommandCatalog => super::check_command_catalog(),
        XtaskCommand::CheckBadgeDiffPolicy => super::check_badge_diff_policy(),
        XtaskCommand::CheckGeneratedClean => super::check_generated_clean(),
        XtaskCommand::CheckVerificationContracts(args) => {
            super::verification_contracts::check_verification_contracts(&args)
        }
        XtaskCommand::CheckDependencies => super::check_dependencies(),
        XtaskCommand::CheckSupplyChain => super::check_supply_chain(),
        XtaskCommand::CheckProcessPolicy => super::check_process_policy(),
        XtaskCommand::CheckNetworkPolicy => super::check_network_policy(),
        XtaskCommand::CheckLintPolicy => super::check_lint_policy(),
        XtaskCommand::CheckCiLaneWhitelist => super::check_ci_lane_whitelist(),
        XtaskCommand::CheckProofPacks => super::check_proof_packs(),
        XtaskCommand::CheckProductCopy => super::check_product_copy(),
        XtaskCommand::CheckPositioningLanguage => super::check_positioning_language(),
        XtaskCommand::CheckDocRoles => super::check_doc_roles(),
        XtaskCommand::CheckReleaseTargets => super::check_release_targets(),
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
        XtaskCommand::IssueIntake(args) => super::reports::issue_intake(&args),
        XtaskCommand::Help(args) => print_help(&args),
        XtaskCommand::Unknown(command) => Err(unknown_command_message(&command)),
    }
}

#[cfg(test)]
mod tests {
    use super::execute;
    use crate::command::XtaskCommand;

    #[test]
    fn rust_repair_trust_report_is_reachable_from_dispatch() -> Result<(), String> {
        // The report reads repo-relative paths, so the test changes the
        // process cwd — a global mutation that races other tests since the
        // suite went parallel (#2132; flake seen on CI in two unrelated PR
        // branches). Hold the write guard for the whole window.
        let _cwd_guard = crate::acquire_test_cwd_write_guard();
        let original_dir = std::env::current_dir().map_err(|error| error.to_string())?;
        let repository_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .ok_or_else(|| "xtask manifest must have a repository parent".to_string())?;
        std::env::set_current_dir(repository_root).map_err(|error| error.to_string())?;
        let result = execute(XtaskCommand::RustRepairTrustReport);
        std::env::set_current_dir(original_dir).map_err(|error| error.to_string())?;
        result
    }
}
