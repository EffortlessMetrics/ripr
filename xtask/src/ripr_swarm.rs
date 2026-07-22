//! ripr swarm cluster: the `ripr-swarm` plan / attempt / attempt-ledger /
//! readiness / route-quality reports (dispatch entrypoint, argument parsing,
//! plan and dry-run builders, attempt-ledger import and history, readiness
//! summary, limitation routes, next-action routing, and their JSON/markdown
//! renderers).
//!
//! Extracted verbatim from `main.rs` as a behavior-preserving decomposition
//! slice of #2119. Items are `pub(crate)` and re-exported from `main.rs` so
//! existing call sites (`dispatch.rs`, `dogfood.rs`, and `tests.rs`) compile
//! unchanged.
//!
//! Three small helpers that sit physically inside this cluster but belong to
//! the audit / actionable-gap families (`audit_slug`,
//! `actionable_gap_outcome_state_counts_from_entries`,
//! `actionable_gap_outcomes_missing_verify_result_count`) intentionally remain
//! in `main.rs`; this module reaches them through `use super::*;`.

use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RiprSwarmAttemptLedgerArgs {
    pub(crate) swarm_plan_path: PathBuf,
    pub(crate) actionable_gap_outcomes_path: PathBuf,
    pub(crate) prior_ledger_path: PathBuf,
    pub(crate) real_repair_attempts_path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RiprSwarmCommand {
    Plan(RiprSwarmPlanArgs),
    Attempt(RiprSwarmAttemptArgs),
    AttemptLedger(RiprSwarmAttemptLedgerArgs),
    Readiness(RiprSwarmReadinessArgs),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RiprSwarmPlanArgs {
    pub(crate) top: usize,
    pub(crate) actionable_gaps_path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RiprSwarmAttemptArgs {
    pub(crate) packet_id: String,
    pub(crate) actionable_gaps_path: PathBuf,
    pub(crate) dry_run: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RiprSwarmReadinessArgs {
    pub(crate) swarm_plan_path: PathBuf,
    pub(crate) actionable_gap_outcomes_path: PathBuf,
    pub(crate) attempt_ledger_path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RiprSwarmPlanReport {
    pub(crate) status: String,
    pub(crate) runtime_status: Lane1RuntimeStatus,
    pub(crate) input_state: String,
    pub(crate) input_path: String,
    pub(crate) input_limitation: Option<String>,
    pub(crate) top_limit: usize,
    pub(crate) source_summary: Value,
    pub(crate) static_limitation_backlog: Value,
    pub(crate) packets: Vec<RiprSwarmPlanPacket>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RiprSwarmPlanPacket {
    pub(crate) packet_id: String,
    pub(crate) canonical_gap_id: String,
    pub(crate) evidence_class: String,
    pub(crate) source_file: String,
    pub(crate) repair_kind: String,
    pub(crate) target_test_type: String,
    pub(crate) assertion_shape: String,
    pub(crate) confidence_basis: String,
    pub(crate) swarm_state: String,
    pub(crate) score: usize,
    pub(crate) expected_canonical_gap_delta: usize,
    pub(crate) readiness_reasons: Vec<String>,
    pub(crate) blocked_reasons: Vec<String>,
    pub(crate) missing_context: Vec<String>,
    pub(crate) verify_command: Option<String>,
    pub(crate) receipt_command_or_path: Option<String>,
    pub(crate) related_test_or_observer_available: bool,
    pub(crate) must_not_change: Vec<String>,
    pub(crate) must_not_change_count: usize,
    pub(crate) allowed_edit_surface: Vec<String>,
    pub(crate) allowed_edit_surface_count: usize,
    pub(crate) raw_findings_count: usize,
    pub(crate) static_limitations_count: usize,
    pub(crate) public_projection_eligible: bool,
    pub(crate) projection_exclusion_reasons: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RiprSwarmAttemptDryRun {
    pub(crate) packet_id: String,
    pub(crate) canonical_gap_id: String,
    pub(crate) evidence_class: String,
    pub(crate) source_file: String,
    pub(crate) swarm_state: String,
    pub(crate) repair_kind: String,
    pub(crate) repair_route: String,
    pub(crate) target_test_type: String,
    pub(crate) assertion_shape: String,
    pub(crate) related_test_or_observer: String,
    pub(crate) verify_command: String,
    pub(crate) receipt_command_or_path: String,
    pub(crate) must_not_change: Vec<String>,
    pub(crate) raw_findings_count: usize,
    pub(crate) static_limitations_count: usize,
    pub(crate) confidence_basis: String,
    pub(crate) expected_evidence_movement: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RiprSwarmReadinessReport {
    pub(crate) status: String,
    pub(crate) readiness_state: String,
    pub(crate) runtime_status: Lane1RuntimeStatus,
    pub(crate) swarm_plan_path: String,
    pub(crate) swarm_plan_state: String,
    pub(crate) swarm_plan_limitation: Option<String>,
    pub(crate) actionable_gap_outcomes_path: String,
    pub(crate) actionable_gap_outcomes_state: String,
    pub(crate) actionable_gap_outcomes_limitation: Option<String>,
    pub(crate) attempt_ledger_path: String,
    pub(crate) attempt_ledger_state: String,
    pub(crate) attempt_ledger_limitation: Option<String>,
    pub(crate) summary: RiprSwarmReadinessSummary,
    pub(crate) attempt_history_summary: RiprSwarmAttemptLedgerHistorySummary,
    pub(crate) static_limitation_backlog: Value,
    pub(crate) top_limitation_routes: Vec<RiprSwarmLimitationRouteRow>,
    pub(crate) blocked_state_routes: Vec<RiprSwarmReadinessBlockedStateRoute>,
    pub(crate) repair_route_quality: Vec<RiprSwarmRepairRouteQualityRow>,
    pub(crate) language_repair_route_quality: Vec<RiprSwarmRepairRouteQualityRow>,
    pub(crate) cross_language_oracle_route_quality: Value,
    pub(crate) top_failing_repair_routes: Vec<RiprSwarmRepairRouteQualityRow>,
    pub(crate) top_missing_evidence_fields: Vec<RiprSwarmMissingEvidenceFieldRow>,
    pub(crate) next_actions: Vec<RiprSwarmReadinessNextAction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RiprSwarmRouteQualityReport {
    pub(crate) status: String,
    pub(crate) runtime_status: Lane1RuntimeStatus,
    pub(crate) generated_at: String,
    pub(crate) attempt_ledger_path: String,
    pub(crate) attempt_ledger_state: String,
    pub(crate) attempt_ledger_limitation: Option<String>,
    pub(crate) repair_route_quality_latest: Vec<RiprSwarmRepairRouteQualityRow>,
    pub(crate) repair_route_quality_historical: Vec<RiprSwarmRepairRouteQualityRow>,
    pub(crate) language_repair_route_quality_latest: Vec<RiprSwarmRepairRouteQualityRow>,
    pub(crate) language_repair_route_quality_historical: Vec<RiprSwarmRepairRouteQualityRow>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RiprSwarmAttemptLedgerReport {
    pub(crate) status: String,
    pub(crate) runtime_status: Lane1RuntimeStatus,
    pub(crate) generated_at: String,
    pub(crate) swarm_plan_path: String,
    pub(crate) swarm_plan_state: String,
    pub(crate) swarm_plan_limitation: Option<String>,
    pub(crate) actionable_gap_outcomes_path: String,
    pub(crate) actionable_gap_outcomes_state: String,
    pub(crate) actionable_gap_outcomes_limitation: Option<String>,
    pub(crate) prior_ledger_path: String,
    pub(crate) prior_ledger_state: String,
    pub(crate) prior_ledger_limitation: Option<String>,
    pub(crate) real_repair_attempts_path: String,
    pub(crate) real_repair_attempts_state: String,
    pub(crate) real_repair_attempts_limitation: Option<String>,
    pub(crate) attempts: Vec<RiprSwarmAttemptLedgerEntry>,
    pub(crate) latest_attempts: Vec<RiprSwarmAttemptLedgerEntry>,
    pub(crate) repair_route_quality: Vec<RiprSwarmRepairRouteQualityRow>,
    pub(crate) language_repair_route_quality: Vec<RiprSwarmRepairRouteQualityRow>,
    pub(crate) historical_repair_route_quality: Vec<RiprSwarmRepairRouteQualityRow>,
    pub(crate) historical_language_repair_route_quality: Vec<RiprSwarmRepairRouteQualityRow>,
    pub(crate) top_missing_evidence_fields: Vec<RiprSwarmMissingEvidenceFieldRow>,
    pub(crate) orphaned_receipts: Vec<Value>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RiprSwarmAttemptLedgerSummary {
    pub(crate) attempts_total: usize,
    pub(crate) canonical_gaps_total: usize,
    pub(crate) not_attempted: usize,
    pub(crate) attempted_no_receipt: usize,
    pub(crate) receipt_present: usize,
    pub(crate) missing_verify_result: usize,
    pub(crate) evidence_improved: usize,
    pub(crate) evidence_unchanged: usize,
    pub(crate) expected_unchanged: usize,
    pub(crate) evidence_regressed: usize,
    pub(crate) resolved: usize,
    pub(crate) unknown: usize,
    pub(crate) orphaned_receipts: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RiprSwarmAttemptLedgerHistorySummary {
    pub(crate) attempts_total: usize,
    pub(crate) durable_attempts_total: usize,
    pub(crate) canonical_gaps_total: usize,
    pub(crate) not_attempted: usize,
    pub(crate) attempted_no_receipt: usize,
    pub(crate) receipt_present: usize,
    pub(crate) missing_verify_result: usize,
    pub(crate) evidence_improved: usize,
    pub(crate) evidence_unchanged: usize,
    pub(crate) expected_unchanged: usize,
    pub(crate) evidence_regressed: usize,
    pub(crate) resolved: usize,
    pub(crate) unknown: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RiprSwarmRepairRouteQualityRow {
    pub(crate) language: Option<String>,
    pub(crate) repair_kind: String,
    pub(crate) attempted: usize,
    pub(crate) improved: usize,
    pub(crate) unchanged: usize,
    pub(crate) regressed: usize,
    pub(crate) resolved: usize,
    pub(crate) attempted_no_receipt: usize,
    pub(crate) receipt_present: usize,
    pub(crate) missing_verify_result: usize,
    pub(crate) expected_unchanged: usize,
    pub(crate) unknown: usize,
    pub(crate) sample_packet_ids: Vec<String>,
    pub(crate) sample_attempt_ids: Vec<String>,
    pub(crate) sample_canonical_gap_ids: Vec<String>,
    pub(crate) sample_missing_receipt_reasons: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RiprSwarmMissingEvidenceFieldRow {
    pub(crate) label: String,
    pub(crate) count: usize,
    pub(crate) sample_packet_ids: Vec<String>,
    pub(crate) sample_canonical_gap_ids: Vec<String>,
    pub(crate) sample_repair_kinds: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RiprSwarmLimitationRouteRow {
    pub(crate) repair_route: String,
    pub(crate) signal_count: usize,
    pub(crate) sample_packet_id: Option<String>,
    pub(crate) sample_limitation_category: Option<String>,
    pub(crate) sample_limitation_subroute: Option<String>,
    pub(crate) sample_canonical_gap_ids: Vec<String>,
    pub(crate) sample_sources: Vec<Lane1StaticLimitationBacklogSample>,
    pub(crate) dominant_evidence_class: Option<String>,
    pub(crate) why_not_actionable: Option<String>,
    pub(crate) unlock_condition: Option<String>,
    pub(crate) non_claims: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RiprSwarmAttemptLedgerEntry {
    pub(crate) packet_id: String,
    pub(crate) canonical_gap_id: String,
    pub(crate) attempt_id: String,
    pub(crate) language: Option<String>,
    pub(crate) evidence_class: Option<String>,
    pub(crate) source_file: Option<String>,
    pub(crate) repair_kind: Option<String>,
    pub(crate) target_test_type: Option<String>,
    pub(crate) assertion_shape: Option<String>,
    pub(crate) actor_kind: String,
    pub(crate) receipt_path: Option<String>,
    pub(crate) verify_command: String,
    pub(crate) verify_result: Option<String>,
    pub(crate) receipt_command: Option<String>,
    pub(crate) missing_receipt_reason: Option<String>,
    pub(crate) before_gap_state: Option<String>,
    pub(crate) after_gap_state: Option<String>,
    pub(crate) outcome: String,
    pub(crate) timestamp: Option<String>,
    pub(crate) receipt_state: String,
    pub(crate) movement_source: Option<String>,
    pub(crate) route_quality_expectation: Option<String>,
    pub(crate) reason: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RiprSwarmReadinessSummary {
    pub(crate) actionable_gaps_total: usize,
    pub(crate) public_projection_eligible_packets: usize,
    pub(crate) swarm_ready_packets: usize,
    pub(crate) blocked_packets: usize,
    pub(crate) blocked_by_missing_context_packets: usize,
    pub(crate) blocked_by_static_limitation_packets: usize,
    pub(crate) blocked_by_public_projection_exclusion_packets: usize,
    pub(crate) blocked_by_operator_judgment_packets: usize,
    pub(crate) public_projection_excluded_packets: usize,
    pub(crate) public_projection_exclusion_reasons: BTreeMap<String, usize>,
    pub(crate) missing_canonical_gap_id: usize,
    pub(crate) not_actionable_gap_state: usize,
    pub(crate) missing_verify_command: usize,
    pub(crate) missing_verify_result: usize,
    pub(crate) missing_receipt_command: usize,
    pub(crate) missing_repair_kind: usize,
    pub(crate) missing_repair_route: usize,
    pub(crate) missing_target_test_shape: usize,
    pub(crate) missing_must_not_change: usize,
    pub(crate) missing_allowed_edit_surface: usize,
    pub(crate) missing_confidence: usize,
    pub(crate) missing_raw_evidence_refs: usize,
    pub(crate) missing_related_test_or_observer: usize,
    pub(crate) related_context_missing: usize,
    pub(crate) static_limitation_packets: usize,
    pub(crate) static_limitation_backlog_packets: usize,
    pub(crate) static_limitation_backlog_signals: usize,
    pub(crate) high_confidence_packets: usize,
    pub(crate) attempted_packets: usize,
    pub(crate) attempted_no_receipt_packets: usize,
    pub(crate) receipt_present_packets: usize,
    pub(crate) improved_packets: usize,
    pub(crate) unchanged_packets: usize,
    pub(crate) expected_unchanged_packets: usize,
    pub(crate) regressed_packets: usize,
    pub(crate) resolved_packets: usize,
    pub(crate) orphaned_receipts: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RiprSwarmReadinessNextAction {
    pub(crate) kind: String,
    pub(crate) packet_id: Option<String>,
    pub(crate) attempt_id: Option<String>,
    pub(crate) canonical_gap_id: Option<String>,
    pub(crate) evidence_class: Option<String>,
    pub(crate) repair_kind: Option<String>,
    pub(crate) command: Option<String>,
    pub(crate) reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RiprSwarmReadinessBlockedStateRoute {
    pub(crate) state: String,
    pub(crate) count: usize,
    pub(crate) reason: String,
    pub(crate) next_action_kind: String,
    pub(crate) repair_route: String,
    pub(crate) example_packet_id: Option<String>,
    pub(crate) example_canonical_gap_id: Option<String>,
    pub(crate) example_repair_kind: Option<String>,
    pub(crate) example_receipt_path: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RiprSwarmReadinessBlockedStateSample {
    pub(crate) packet_id: Option<String>,
    pub(crate) canonical_gap_id: Option<String>,
    pub(crate) repair_kind: Option<String>,
    pub(crate) receipt_path: Option<String>,
}

pub(crate) struct RiprSwarmReadinessInput<'a> {
    pub(crate) path: String,
    pub(crate) state: String,
    pub(crate) limitation: Option<String>,
    pub(crate) value: Option<&'a Value>,
}

pub(crate) fn ripr_swarm(args: &[String]) -> Result<(), String> {
    match parse_ripr_swarm_args(args)? {
        RiprSwarmCommand::Plan(parsed) => ripr_swarm_plan_report(&parsed),
        RiprSwarmCommand::Attempt(parsed) => ripr_swarm_attempt_dry_run(&parsed),
        RiprSwarmCommand::AttemptLedger(parsed) => ripr_swarm_attempt_ledger_report(&parsed),
        RiprSwarmCommand::Readiness(parsed) => ripr_swarm_readiness_report(&parsed),
    }
}

pub(crate) fn parse_ripr_swarm_args(args: &[String]) -> Result<RiprSwarmCommand, String> {
    let Some(subcommand) = args.first() else {
        return Err(ripr_swarm_usage());
    };
    match subcommand.as_str() {
        "plan" => parse_ripr_swarm_plan_args(args).map(RiprSwarmCommand::Plan),
        "attempt" => parse_ripr_swarm_attempt_args(args).map(RiprSwarmCommand::Attempt),
        "attempt-ledger" => {
            parse_ripr_swarm_attempt_ledger_args(args).map(RiprSwarmCommand::AttemptLedger)
        }
        "readiness" => parse_ripr_swarm_readiness_args(args).map(RiprSwarmCommand::Readiness),
        _ => Err(format!(
            "unknown ripr-swarm subcommand `{subcommand}`\n{}",
            ripr_swarm_usage()
        )),
    }
}

pub(crate) fn parse_ripr_swarm_plan_args(args: &[String]) -> Result<RiprSwarmPlanArgs, String> {
    let Some(subcommand) = args.first() else {
        return Err(ripr_swarm_usage());
    };
    if subcommand != "plan" {
        return Err(format!(
            "unknown ripr-swarm subcommand `{subcommand}`\n{}",
            ripr_swarm_usage()
        ));
    }

    let mut top = 10usize;
    let mut actionable_gaps_path = PathBuf::from("target/ripr/reports/actionable-gaps.json");
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--top" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err("ripr-swarm plan --top requires a positive integer".to_string());
                };
                top = value.parse::<usize>().map_err(|err| {
                    format!("ripr-swarm plan --top requires a positive integer: {err}")
                })?;
                if top == 0 {
                    return Err("ripr-swarm plan --top must be greater than zero".to_string());
                }
            }
            "--actionable-gaps" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err("ripr-swarm plan --actionable-gaps requires a path".to_string());
                };
                actionable_gaps_path = PathBuf::from(value);
            }
            other => {
                return Err(format!(
                    "unknown ripr-swarm plan argument `{other}`\n{}",
                    ripr_swarm_usage()
                ));
            }
        }
        index += 1;
    }

    Ok(RiprSwarmPlanArgs {
        top,
        actionable_gaps_path,
    })
}

pub(crate) fn parse_ripr_swarm_attempt_args(
    args: &[String],
) -> Result<RiprSwarmAttemptArgs, String> {
    let Some(subcommand) = args.first() else {
        return Err(ripr_swarm_usage());
    };
    if subcommand != "attempt" {
        return Err(format!(
            "unknown ripr-swarm subcommand `{subcommand}`\n{}",
            ripr_swarm_usage()
        ));
    }

    let mut packet_id: Option<String> = None;
    let mut actionable_gaps_path = PathBuf::from("target/ripr/reports/actionable-gaps.json");
    let mut dry_run = false;
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--packet" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err("ripr-swarm attempt --packet requires a packet id".to_string());
                };
                if value.trim().is_empty() {
                    return Err(
                        "ripr-swarm attempt --packet requires a non-empty packet id".to_string()
                    );
                }
                packet_id = Some(value.to_string());
            }
            "--dry-run" => {
                dry_run = true;
            }
            "--actionable-gaps" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err("ripr-swarm attempt --actionable-gaps requires a path".to_string());
                };
                actionable_gaps_path = PathBuf::from(value);
            }
            other => {
                return Err(format!(
                    "unknown ripr-swarm attempt argument `{other}`\n{}",
                    ripr_swarm_usage()
                ));
            }
        }
        index += 1;
    }

    let Some(packet_id) = packet_id else {
        return Err("ripr-swarm attempt requires --packet <id>".to_string());
    };
    if !dry_run {
        return Err("ripr-swarm attempt currently requires --dry-run".to_string());
    }

    Ok(RiprSwarmAttemptArgs {
        packet_id,
        actionable_gaps_path,
        dry_run,
    })
}

pub(crate) fn parse_ripr_swarm_readiness_args(
    args: &[String],
) -> Result<RiprSwarmReadinessArgs, String> {
    let Some(subcommand) = args.first() else {
        return Err(ripr_swarm_usage());
    };
    if subcommand != "readiness" {
        return Err(format!(
            "unknown ripr-swarm subcommand `{subcommand}`\n{}",
            ripr_swarm_usage()
        ));
    }

    let mut swarm_plan_path = PathBuf::from("target/ripr/reports/swarm-plan.json");
    let mut actionable_gap_outcomes_path =
        PathBuf::from("target/ripr/reports/actionable-gap-outcomes.json");
    let mut attempt_ledger_path = PathBuf::from("target/ripr/reports/swarm-attempt-ledger.json");
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--swarm-plan" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err("ripr-swarm readiness --swarm-plan requires a path".to_string());
                };
                swarm_plan_path = PathBuf::from(value);
            }
            "--actionable-gap-outcomes" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(
                        "ripr-swarm readiness --actionable-gap-outcomes requires a path"
                            .to_string(),
                    );
                };
                actionable_gap_outcomes_path = PathBuf::from(value);
            }
            "--attempt-ledger" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err("ripr-swarm readiness --attempt-ledger requires a path".to_string());
                };
                attempt_ledger_path = PathBuf::from(value);
            }
            other => {
                return Err(format!(
                    "unknown ripr-swarm readiness argument `{other}`\n{}",
                    ripr_swarm_usage()
                ));
            }
        }
        index += 1;
    }

    Ok(RiprSwarmReadinessArgs {
        swarm_plan_path,
        actionable_gap_outcomes_path,
        attempt_ledger_path,
    })
}

pub(crate) fn parse_ripr_swarm_attempt_ledger_args(
    args: &[String],
) -> Result<RiprSwarmAttemptLedgerArgs, String> {
    let Some(subcommand) = args.first() else {
        return Err(ripr_swarm_usage());
    };
    if subcommand != "attempt-ledger" {
        return Err(format!(
            "unknown ripr-swarm subcommand `{subcommand}`\n{}",
            ripr_swarm_usage()
        ));
    }

    let mut swarm_plan_path = PathBuf::from("target/ripr/reports/swarm-plan.json");
    let mut actionable_gap_outcomes_path =
        PathBuf::from("target/ripr/reports/actionable-gap-outcomes.json");
    let mut prior_ledger_path = PathBuf::from("target/ripr/reports/swarm-attempt-ledger.json");
    let mut real_repair_attempts_path = PathBuf::from(REAL_REPAIR_ATTEMPTS_CORPUS);
    let mut index = 1usize;
    while index < args.len() {
        match args[index].as_str() {
            "--swarm-plan" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(
                        "ripr-swarm attempt-ledger --swarm-plan requires a path".to_string()
                    );
                };
                swarm_plan_path = PathBuf::from(value);
            }
            "--actionable-gap-outcomes" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(
                        "ripr-swarm attempt-ledger --actionable-gap-outcomes requires a path"
                            .to_string(),
                    );
                };
                actionable_gap_outcomes_path = PathBuf::from(value);
            }
            "--previous-ledger" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(
                        "ripr-swarm attempt-ledger --previous-ledger requires a path".to_string(),
                    );
                };
                prior_ledger_path = PathBuf::from(value);
            }
            "--real-repair-attempts" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err(
                        "ripr-swarm attempt-ledger --real-repair-attempts requires a path"
                            .to_string(),
                    );
                };
                real_repair_attempts_path = PathBuf::from(value);
            }
            other => {
                return Err(format!(
                    "unknown ripr-swarm attempt-ledger argument `{other}`\n{}",
                    ripr_swarm_usage()
                ));
            }
        }
        index += 1;
    }

    Ok(RiprSwarmAttemptLedgerArgs {
        swarm_plan_path,
        actionable_gap_outcomes_path,
        prior_ledger_path,
        real_repair_attempts_path,
    })
}

pub(crate) fn ripr_swarm_usage() -> String {
    "usage: cargo xtask ripr-swarm plan [--top <n>] [--actionable-gaps <path>]\n       cargo xtask ripr-swarm attempt --packet <id> --dry-run [--actionable-gaps <path>]\n       cargo xtask ripr-swarm attempt-ledger [--swarm-plan <path>] [--actionable-gap-outcomes <path>] [--previous-ledger <path>] [--real-repair-attempts <path>]\n       cargo xtask ripr-swarm readiness [--swarm-plan <path>] [--actionable-gap-outcomes <path>] [--attempt-ledger <path>]"
        .to_string()
}

pub(crate) fn ripr_swarm_plan_report(args: &RiprSwarmPlanArgs) -> Result<(), String> {
    let report = match fs::read_to_string(&args.actionable_gaps_path) {
        Ok(text) => match serde_json::from_str::<Value>(&text) {
            Ok(value) => ripr_swarm_plan_from_actionable_gaps_value(
                args.top,
                &args.actionable_gaps_path,
                &value,
            ),
            Err(err) => ripr_swarm_plan_blocked_report(
                args.top,
                &args.actionable_gaps_path,
                "malformed",
                format!(
                    "failed to parse actionable-gaps JSON: {err}; rerun `cargo xtask lane1-evidence-audit` before planning swarm repairs"
                ),
            ),
        },
        Err(err) => ripr_swarm_plan_blocked_report(
            args.top,
            &args.actionable_gaps_path,
            "missing",
            format!(
                "failed to read {}: {err}; run `cargo xtask lane1-evidence-audit` to create actionable-gaps.json before planning swarm repairs",
                args.actionable_gaps_path.display()
            ),
        ),
    };

    write_report("swarm-plan.json", &ripr_swarm_plan_json(&report)?)?;
    write_report("swarm-plan.md", &ripr_swarm_plan_markdown(&report))
}

pub(crate) fn ripr_swarm_attempt_dry_run(args: &RiprSwarmAttemptArgs) -> Result<(), String> {
    if !args.dry_run {
        return Err("ripr-swarm attempt currently requires --dry-run".to_string());
    }
    let value = read_json_value(&args.actionable_gaps_path)?;
    let attempt = ripr_swarm_attempt_dry_run_from_actionable_gaps_value(&value, &args.packet_id)?;
    println!("{}", ripr_swarm_attempt_dry_run_markdown(&attempt));
    Ok(())
}

pub(crate) fn ripr_swarm_attempt_dry_run_from_actionable_gaps_value(
    value: &Value,
    packet_id: &str,
) -> Result<RiprSwarmAttemptDryRun, String> {
    let packets = audit_get(value, &["packets"])
        .and_then(Value::as_array)
        .ok_or_else(|| "actionable-gaps JSON is missing a `packets` array".to_string())?;
    let Some(packet) = packets
        .iter()
        .find(|packet| ripr_swarm_attempt_packet_matches(packet, packet_id))
    else {
        return Err(format!(
            "ripr-swarm attempt could not find packet `{packet_id}` in actionable-gaps.json"
        ));
    };
    Ok(ripr_swarm_attempt_dry_run_from_packet(packet))
}

pub(crate) fn ripr_swarm_attempt_packet_matches(packet: &Value, packet_id: &str) -> bool {
    let mut requested = BTreeSet::new();
    actionable_gap_push_id_candidate(&mut requested, packet_id);
    let candidates = actionable_gap_id_candidates(packet);
    requested
        .iter()
        .any(|requested| candidates.contains(requested))
}

pub(crate) fn ripr_swarm_attempt_dry_run_from_packet(packet: &Value) -> RiprSwarmAttemptDryRun {
    let plan_packet = ripr_swarm_plan_packet_from_value(packet);
    let repair_route = ripr_swarm_attempt_repair_route_summary(packet);
    let related_test_or_observer = ripr_swarm_attempt_value_summary(
        audit_get(packet, &["related_test_or_observer"])
            .or_else(|| audit_get(packet, &["candidate_value_or_observer"])),
    );
    let must_not_change = audit_string_array(packet, &["must_not_change"]).unwrap_or_default();
    let expected_evidence_movement = if plan_packet.swarm_state == "queued" {
        format!(
            "-{} actionable canonical gap if receipt-backed evidence movement resolves or improves this packet",
            plan_packet.expected_canonical_gap_delta
        )
    } else {
        format!(
            "not repair-ready until blocked context is resolved: {}",
            plan_packet.blocked_reasons.join(", ")
        )
    };

    RiprSwarmAttemptDryRun {
        packet_id: plan_packet.packet_id,
        canonical_gap_id: plan_packet.canonical_gap_id,
        evidence_class: plan_packet.evidence_class,
        source_file: plan_packet.source_file,
        swarm_state: plan_packet.swarm_state,
        repair_kind: plan_packet.repair_kind,
        repair_route,
        target_test_type: plan_packet.target_test_type,
        assertion_shape: plan_packet.assertion_shape,
        related_test_or_observer,
        verify_command: plan_packet
            .verify_command
            .unwrap_or_else(|| "verify_command_unknown".to_string()),
        receipt_command_or_path: plan_packet
            .receipt_command_or_path
            .unwrap_or_else(|| "receipt_command_unknown".to_string()),
        must_not_change,
        raw_findings_count: plan_packet.raw_findings_count,
        static_limitations_count: plan_packet.static_limitations_count,
        confidence_basis: plan_packet.confidence_basis,
        expected_evidence_movement,
    }
}

pub(crate) fn ripr_swarm_attempt_repair_route_summary(packet: &Value) -> String {
    match audit_get(packet, &["repair_route"]) {
        Some(Value::Object(route)) => {
            let repair_kind = route
                .get("repair_kind")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| audit_non_empty_string(packet, &["repair_kind"]))
                .unwrap_or_else(|| "repair_kind_unknown".to_string());
            let target_test_type = route
                .get("target_test_type")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| audit_non_empty_string(packet, &["target_test_type"]))
                .unwrap_or_else(|| "target_test_type_unknown".to_string());
            let assertion_shape = route
                .get("assertion_shape")
                .and_then(Value::as_str)
                .or_else(|| route.get("suggested_assertion").and_then(Value::as_str))
                .map(str::to_string)
                .or_else(|| audit_non_empty_string(packet, &["assertion_shape"]))
                .unwrap_or_else(|| "assertion_shape_unknown".to_string());
            format!("{repair_kind} -> {target_test_type} -> {assertion_shape}")
        }
        Some(value) => ripr_swarm_attempt_value_summary(Some(value)),
        None => "repair_route_unknown".to_string(),
    }
}

pub(crate) fn ripr_swarm_attempt_value_summary(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(value)) => value.clone(),
        Some(Value::Object(object)) => {
            if let (Some(file), Some(name)) = (
                object.get("file").and_then(Value::as_str),
                object.get("name").and_then(Value::as_str),
            ) {
                format!("{file}::{name}")
            } else {
                serde_json::to_string(&Value::Object(object.clone()))
                    .unwrap_or_else(|_| "object".to_string())
            }
        }
        Some(Value::Array(values)) => format!("{} item(s)", values.len()),
        Some(Value::Number(value)) => value.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        Some(Value::Null) | None => "unknown".to_string(),
    }
}

pub(crate) fn ripr_swarm_attempt_dry_run_markdown(attempt: &RiprSwarmAttemptDryRun) -> String {
    let mut out = String::new();
    out.push_str("# RIPR Swarm Attempt Dry Run\n\n");
    out.push_str("This command prints bounded repair context only. It does not edit files, run tests, call providers, generate tests, create receipts, run mutation testing, merge code, or change public badge semantics.\n\n");
    out.push_str("## Copy-Ready Operator Packet\n\n");
    out.push_str("Task:\n");
    out.push_str(&format!(
        "- Repair one actionable canonical gap: `{}`.\n",
        audit_markdown_cell(&attempt.canonical_gap_id)
    ));
    out.push_str(&format!(
        "- Packet: `{}` (`{}`).\n",
        audit_markdown_cell(&attempt.packet_id),
        audit_markdown_cell(&attempt.swarm_state)
    ));
    out.push_str("\nAllowed files:\n");
    out.push_str(&format!(
        "- {}.\n",
        audit_markdown_cell(&ripr_swarm_attempt_allowed_file_line(attempt))
    ));
    out.push_str(&format!(
        "- Read-only context: `{}`.\n",
        audit_markdown_cell(&attempt.source_file)
    ));
    out.push_str("\nDo-not-change boundaries:\n");
    if attempt.must_not_change.is_empty() {
        out.push_str("- `must_not_change` is missing; stop before editing files.\n");
    } else {
        for boundary in &attempt.must_not_change {
            out.push_str(&format!("- {}\n", audit_markdown_cell(boundary)));
        }
    }
    out.push_str("\nRepair target:\n");
    out.push_str(&format!(
        "- Kind: `{}`.\n",
        audit_markdown_cell(&attempt.repair_kind)
    ));
    out.push_str(&format!(
        "- Route: {}.\n",
        audit_markdown_cell(&attempt.repair_route)
    ));
    out.push_str(&format!(
        "- Assertion or observer: {}.\n",
        audit_markdown_cell(&attempt.assertion_shape)
    ));
    out.push_str("\nVerify command:\n");
    out.push_str(&format!("```bash\n{}\n```\n\n", attempt.verify_command));
    out.push_str("Receipt command:\n");
    out.push_str(&format!(
        "```bash\n{}\n```\n\n",
        attempt.receipt_command_or_path
    ));
    out.push_str("Stop conditions:\n");
    out.push_str("- Stop if the actionable-gaps artifact is stale, wrong-root, malformed, or missing this packet.\n");
    out.push_str("- Stop if the required edit falls outside the allowed file or violates a do-not-change boundary.\n");
    out.push_str("- Stop if the verify or receipt command is missing, unsafe, or cannot run in this workspace.\n");
    out.push_str(
        "- Stop if static limitations or blocked context make this packet not repair-ready.\n\n",
    );
    out.push_str("Required return format:\n");
    out.push_str("- `packet_id`: packet attempted.\n");
    out.push_str("- `files_changed`: workspace-relative files changed, or `none`.\n");
    out.push_str("- `verify_result`: command run plus pass/fail/not-run.\n");
    out.push_str(
        "- `receipt_result`: receipt command result, path, or precise reason not emitted.\n",
    );
    out.push_str("- `remaining_blockers`: any stale, wrong-root, unsafe, static-limited, or scope blocker.\n\n");
    out.push_str("## Packet\n\n");
    out.push_str("| Field | Value |\n");
    out.push_str("| --- | --- |\n");
    out.push_str(&format!(
        "| Packet | `{}` |\n",
        audit_markdown_cell(&attempt.packet_id)
    ));
    out.push_str(&format!(
        "| Canonical gap | `{}` |\n",
        audit_markdown_cell(&attempt.canonical_gap_id)
    ));
    out.push_str(&format!(
        "| Evidence class | `{}` |\n",
        audit_markdown_cell(&attempt.evidence_class)
    ));
    out.push_str(&format!(
        "| Source file | `{}` |\n",
        audit_markdown_cell(&attempt.source_file)
    ));
    out.push_str(&format!(
        "| Swarm state | `{}` |\n",
        audit_markdown_cell(&attempt.swarm_state)
    ));
    out.push_str(&format!(
        "| Confidence | `{}` |\n",
        audit_markdown_cell(&attempt.confidence_basis)
    ));
    out.push('\n');

    out.push_str("## Repair Context\n\n");
    out.push_str("| Field | Value |\n");
    out.push_str("| --- | --- |\n");
    out.push_str(&format!(
        "| Repair kind | `{}` |\n",
        audit_markdown_cell(&attempt.repair_kind)
    ));
    out.push_str(&format!(
        "| Repair route | {} |\n",
        audit_markdown_cell(&attempt.repair_route)
    ));
    out.push_str(&format!(
        "| Target test type | `{}` |\n",
        audit_markdown_cell(&attempt.target_test_type)
    ));
    out.push_str(&format!(
        "| Assertion / observer shape | {} |\n",
        audit_markdown_cell(&attempt.assertion_shape)
    ));
    out.push_str(&format!(
        "| Related test / observer | {} |\n",
        audit_markdown_cell(&attempt.related_test_or_observer)
    ));
    out.push_str(&format!(
        "| Expected evidence movement | {} |\n\n",
        audit_markdown_cell(&attempt.expected_evidence_movement)
    ));

    out.push_str("## Commands\n\n");
    out.push_str(&format!(
        "- Verify: `{}`\n",
        audit_markdown_cell(&attempt.verify_command)
    ));
    out.push_str(&format!(
        "- Receipt: `{}`\n\n",
        audit_markdown_cell(&attempt.receipt_command_or_path)
    ));

    out.push_str("## Boundaries\n\n");
    if attempt.must_not_change.is_empty() {
        out.push_str("- `must_not_change` is missing; keep this packet blocked until Lane 1 emits boundaries.\n");
    } else {
        for boundary in &attempt.must_not_change {
            out.push_str(&format!("- {}\n", audit_markdown_cell(boundary)));
        }
    }
    out.push_str(&format!(
        "- Raw findings are supporting evidence only: {} finding(s).\n",
        attempt.raw_findings_count
    ));
    out.push_str(&format!(
        "- Static limitations attached to packet: {}.\n",
        attempt.static_limitations_count
    ));
    out
}

pub(crate) fn ripr_swarm_attempt_allowed_file_line(attempt: &RiprSwarmAttemptDryRun) -> String {
    if attempt.swarm_state != "queued" {
        return format!(
            "No file edits are authorized while swarm_state is `{}`",
            attempt.swarm_state
        );
    }
    if let Some(target) = ripr_swarm_attempt_related_target_file(&attempt.related_test_or_observer)
    {
        return format!("{} (bounded repair target)", target);
    }
    "No typed edit target is available; do not edit files until the packet is regenerated"
        .to_string()
}

pub(crate) fn ripr_swarm_attempt_related_target_file(related: &str) -> Option<String> {
    if let Some((file, _)) = related.split_once("::") {
        return ripr_swarm_attempt_workspace_relative_file_token(file);
    }
    ripr_swarm_attempt_workspace_relative_file_token(related)
}

pub(crate) fn ripr_swarm_attempt_workspace_relative_file_token(value: &str) -> Option<String> {
    let normalized = value.trim().replace('\\', "/");
    if normalized.is_empty()
        || normalized.starts_with('/')
        || normalized.contains(':')
        || normalized.chars().any(char::is_whitespace)
        || !normalized.contains('.')
    {
        return None;
    }
    if normalized
        .split('/')
        .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return None;
    }
    Some(normalized)
}

pub(crate) fn ripr_swarm_plan_blocked_report(
    top_limit: usize,
    path: &Path,
    input_state: &str,
    input_limitation: String,
) -> RiprSwarmPlanReport {
    RiprSwarmPlanReport {
        status: "blocked".to_string(),
        runtime_status: lane1_runtime_status_limited_input(
            "actionable_gaps_input",
            "actionable-gaps",
            Some(&normalize_path(path)),
            "swarm_plan_input_unavailable",
            "rerun cargo xtask lane1-evidence-audit before planning swarm repairs",
            false,
        ),
        input_state: input_state.to_string(),
        input_path: path.display().to_string(),
        input_limitation: Some(input_limitation),
        top_limit,
        source_summary: Value::Null,
        static_limitation_backlog: Value::Null,
        packets: Vec::new(),
    }
}

pub(crate) fn ripr_swarm_plan_from_actionable_gaps_value(
    top_limit: usize,
    path: &Path,
    value: &Value,
) -> RiprSwarmPlanReport {
    let packets = match audit_get(value, &["packets"]) {
        Some(Value::Array(packets)) => packets
            .iter()
            .map(ripr_swarm_plan_packet_from_value)
            .collect::<Vec<_>>(),
        _ => {
            return ripr_swarm_plan_blocked_report(
                top_limit,
                path,
                "malformed",
                "actionable-gaps JSON is missing a `packets` array; rerun `cargo xtask lane1-evidence-audit` before planning swarm repairs."
                    .to_string(),
            );
        }
    };
    RiprSwarmPlanReport {
        status: "advisory".to_string(),
        runtime_status: lane1_runtime_status_with_input_path(
            lane1_runtime_status_from_report_value(value).unwrap_or_else(lane1_runtime_status_full),
            "actionable_gaps_input",
            &normalize_path(path),
        ),
        input_state: "read".to_string(),
        input_path: path.display().to_string(),
        input_limitation: None,
        top_limit,
        source_summary: audit_get(value, &["summary"])
            .cloned()
            .unwrap_or(Value::Null),
        static_limitation_backlog: audit_get(value, &["static_limitation_backlog"])
            .cloned()
            .unwrap_or(Value::Null),
        packets,
    }
}

pub(crate) fn ripr_swarm_plan_packet_from_value(packet: &Value) -> RiprSwarmPlanPacket {
    let canonical_gap_id =
        audit_non_empty_string(packet, &["canonical_gap_id"]).unwrap_or_default();
    let evidence_class = audit_non_empty_string(packet, &["evidence_class"])
        .unwrap_or_else(|| "unknown".to_string());
    let source_file =
        audit_non_empty_string(packet, &["source_file"]).unwrap_or_else(|| "unknown".to_string());
    let repair_kind = audit_non_empty_string(packet, &["repair_kind"])
        .unwrap_or_else(|| "repair_kind_unknown".to_string());
    let target_test_type = audit_non_empty_string(packet, &["target_test_type"])
        .unwrap_or_else(|| "target_test_type_unknown".to_string());
    let assertion_shape = audit_non_empty_string(packet, &["assertion_shape"])
        .unwrap_or_else(|| "assertion_shape_unknown".to_string());
    let explicit_target_test_shape = audit_non_empty_string(packet, &["target_test_shape"]);
    let target_test_shape = match explicit_target_test_shape.as_deref() {
        Some(value) if !ripr_swarm_plan_field_missing(value) => value.to_string(),
        Some(_) => "target_test_shape_unknown".to_string(),
        None => {
            if !ripr_swarm_plan_field_missing(&target_test_type)
                && !ripr_swarm_plan_field_missing(&assertion_shape)
            {
                audit_actionable_gap_target_test_shape(&target_test_type, &assertion_shape)
            } else {
                "target_test_shape_unknown".to_string()
            }
        }
    };
    let confidence_basis = audit_non_empty_string(packet, &["confidence_basis"])
        .unwrap_or_else(|| "unknown".to_string());
    let verify_command = audit_non_empty_string(packet, &["verify_command"]);
    let receipt_command = audit_non_empty_string(packet, &["receipt_command"]);
    let receipt_command_or_path = receipt_command
        .clone()
        .or_else(|| audit_non_empty_string(packet, &["receipt_command_or_path"]));
    let raw_evidence_refs_count =
        audit_structured_raw_evidence_refs_count(audit_array(packet, &["raw_evidence_refs"]));
    let raw_findings_count = raw_evidence_refs_count.max(audit_structured_raw_evidence_refs_count(
        audit_array(packet, &["raw_findings"]),
    ));
    let static_limitations_count = audit_array(packet, &["static_limitations"]).len();
    let must_not_change = audit_string_array(packet, &["must_not_change"]).unwrap_or_default();
    let must_not_change_count = must_not_change.len();
    let allowed_edit_surface = ripr_swarm_plan_allowed_edit_surface(packet);
    let allowed_edit_surface_count = allowed_edit_surface.len();
    let public_projection_eligible =
        audit_bool(packet, &["public_projection_eligible"]).unwrap_or(false);
    let mut projection_exclusion_reasons = audit_array(packet, &["projection_exclusion_reasons"])
        .iter()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|reason| !reason.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if verify_command
        .as_deref()
        .is_some_and(audit_verify_command_is_unbounded_repo_exposure_snapshot_compare)
    {
        audit_push_projection_exclusion_reason(
            &mut projection_exclusion_reasons,
            "unbounded_verify_command",
        );
    }
    let requires_operator_judgment = ripr_swarm_plan_requires_operator_judgment(
        &evidence_class,
        &repair_kind,
        &target_test_type,
        &confidence_basis,
    );
    let related_test_or_observer_available = ripr_swarm_plan_related_context_present(packet);
    let has_repair_route = ripr_swarm_plan_has_repair_route(packet);
    let repair_route_consistent = has_repair_route
        && ripr_swarm_plan_repair_route_matches_packet(
            packet,
            &repair_kind,
            &target_test_type,
            &assertion_shape,
        );
    let has_verify_command = verify_command
        .as_deref()
        .is_some_and(|command| !ripr_swarm_plan_field_missing(command));
    let has_receipt_command = receipt_command
        .as_deref()
        .is_some_and(|command| !ripr_swarm_plan_field_missing(command));
    let gap_state = audit_non_empty_string(packet, &["gap_state"]).unwrap_or_default();

    let mut missing_context = Vec::new();
    if canonical_gap_id.trim().is_empty() {
        missing_context.push("canonical_gap_id".to_string());
    }
    if projection_exclusion_reasons
        .iter()
        .any(|reason| reason == "missing_canonical_gap_id")
    {
        missing_context.push("canonical_gap_id".to_string());
    }
    if gap_state != "actionable"
        || projection_exclusion_reasons
            .iter()
            .any(|reason| reason == "not_actionable_gap_state")
    {
        missing_context.push("actionable_gap_state".to_string());
    }
    if !has_repair_route
        || projection_exclusion_reasons
            .iter()
            .any(|reason| reason == "missing_repair_route")
    {
        missing_context.push("repair_route".to_string());
    }
    if ripr_swarm_plan_field_missing(&repair_kind)
        || repair_kind == "repair_route_unknown"
        || projection_exclusion_reasons
            .iter()
            .any(|reason| reason == "missing_repair_kind")
    {
        missing_context.push("repair_kind".to_string());
    }
    if has_repair_route && !repair_route_consistent {
        missing_context.push("repair_route_consistency".to_string());
    }
    if gap_state == "actionable"
        && (ripr_swarm_plan_field_missing(&target_test_shape)
            || target_test_shape == "target_test_shape_unknown"
            || projection_exclusion_reasons
                .iter()
                .any(|reason| reason == "missing_target_test_shape"))
    {
        missing_context.push("target_test_shape".to_string());
    }
    if !related_test_or_observer_available
        || projection_exclusion_reasons
            .iter()
            .any(|reason| reason == "missing_related_test_or_observer")
    {
        missing_context.push("related_test_or_observer".to_string());
    }
    if !has_verify_command
        || projection_exclusion_reasons.iter().any(|reason| {
            reason == "missing_verify_command" || reason == "unbounded_verify_command"
        })
    {
        missing_context.push("verify_command".to_string());
    }
    if !has_receipt_command
        || projection_exclusion_reasons
            .iter()
            .any(|reason| reason == "missing_receipt_command")
    {
        missing_context.push("receipt_command".to_string());
    }
    if must_not_change_count == 0
        || projection_exclusion_reasons
            .iter()
            .any(|reason| reason == "missing_must_not_change")
    {
        missing_context.push("must_not_change".to_string());
    }
    if allowed_edit_surface_count == 0 {
        missing_context.push("allowed_edit_surface".to_string());
    }
    if raw_evidence_refs_count == 0
        || projection_exclusion_reasons
            .iter()
            .any(|reason| reason == "missing_raw_evidence_refs")
    {
        missing_context.push("raw_evidence_refs".to_string());
    }
    if ripr_swarm_plan_field_missing(&confidence_basis)
        || projection_exclusion_reasons
            .iter()
            .any(|reason| reason == "missing_confidence")
    {
        missing_context.push("confidence_basis".to_string());
    }

    let mut blocked_reasons = Vec::new();
    let static_limitation_present = static_limitations_count > 0
        || gap_state == "static_limitation"
        || projection_exclusion_reasons
            .iter()
            .any(|reason| reason == "static_limitation_present");
    let swarm_state = if static_limitation_present {
        blocked_reasons.push("static_limitation_present".to_string());
        "blocked_by_static_limitation".to_string()
    } else if !missing_context.is_empty() {
        blocked_reasons.extend(
            missing_context
                .iter()
                .map(|field| format!("missing_{field}")),
        );
        "blocked_by_missing_context".to_string()
    } else if !public_projection_eligible || !projection_exclusion_reasons.is_empty() {
        blocked_reasons.push("public_projection_excluded".to_string());
        "blocked_by_public_projection_exclusion".to_string()
    } else if requires_operator_judgment {
        blocked_reasons
            .push("static_only_predicate_boundary_requires_operator_judgment".to_string());
        "blocked_by_operator_judgment".to_string()
    } else {
        "queued".to_string()
    };

    let mut readiness_reasons = Vec::new();
    let mut score = 0usize;
    if repair_route_consistent {
        score += 20;
        readiness_reasons.push("repair_route_present".to_string());
    }
    if has_verify_command {
        score += 20;
        readiness_reasons.push("verify_command_present".to_string());
    }
    if has_receipt_command {
        score += 20;
        readiness_reasons.push("receipt_command_present".to_string());
    }
    if related_test_or_observer_available {
        score += 10;
        readiness_reasons.push("related_test_or_observer_present".to_string());
    }
    if must_not_change_count > 0 {
        score += 10;
        readiness_reasons.push("must_not_change_present".to_string());
    }
    if allowed_edit_surface_count > 0 {
        score += 10;
        readiness_reasons.push("allowed_edit_surface_present".to_string());
    }
    if public_projection_eligible {
        score += 10;
        readiness_reasons.push("public_projection_eligible".to_string());
    }
    if static_limitations_count == 0 {
        score += 10;
        readiness_reasons.push("no_static_limitation".to_string());
    }
    match confidence_basis.as_str() {
        "fixture_backed" | "calibrated" | "runtime_calibrated" => {
            score += 10;
            readiness_reasons.push(format!("confidence_basis_{confidence_basis}"));
        }
        "static_only" => {
            score += 3;
            readiness_reasons.push("confidence_basis_static_only".to_string());
        }
        _ => {}
    }

    RiprSwarmPlanPacket {
        packet_id: audit_non_empty_string(packet, &["packet_id"])
            .unwrap_or_else(|| canonical_gap_id.clone()),
        canonical_gap_id,
        evidence_class,
        source_file,
        repair_kind,
        target_test_type,
        assertion_shape,
        confidence_basis,
        swarm_state: swarm_state.clone(),
        score,
        expected_canonical_gap_delta: usize::from(swarm_state == "queued"),
        readiness_reasons,
        blocked_reasons,
        missing_context,
        verify_command,
        receipt_command_or_path,
        related_test_or_observer_available,
        must_not_change,
        must_not_change_count,
        allowed_edit_surface,
        allowed_edit_surface_count,
        raw_findings_count,
        static_limitations_count,
        public_projection_eligible,
        projection_exclusion_reasons,
    }
}

pub(crate) fn ripr_swarm_plan_has_repair_route(packet: &Value) -> bool {
    audit_get(packet, &["repair_route"]).is_some_and(|route| {
        route.is_object()
            && ripr_swarm_plan_non_missing_field(route, "repair_kind")
            && ripr_swarm_plan_non_missing_field(route, "target_test_type")
            && ripr_swarm_plan_non_missing_any_field(
                route,
                &["assertion_shape", "suggested_assertion"],
            )
    })
}

pub(crate) fn ripr_swarm_plan_repair_route_matches_packet(
    packet: &Value,
    repair_kind: &str,
    target_test_type: &str,
    assertion_shape: &str,
) -> bool {
    audit_get(packet, &["repair_route"]).is_some_and(|route| {
        audit_non_empty_string(route, &["repair_kind"]).as_deref() == Some(repair_kind)
            && audit_non_empty_string(route, &["target_test_type"]).as_deref()
                == Some(target_test_type)
            && (audit_non_empty_string(route, &["assertion_shape"]).as_deref()
                == Some(assertion_shape)
                || audit_non_empty_string(route, &["suggested_assertion"]).as_deref()
                    == Some(assertion_shape))
    })
}

pub(crate) fn ripr_swarm_plan_requires_operator_judgment(
    evidence_class: &str,
    repair_kind: &str,
    target_test_type: &str,
    confidence_basis: &str,
) -> bool {
    evidence_class == "predicate_boundary"
        && repair_kind == "add_boundary_assertion"
        && target_test_type == "boundary_discriminator"
        && confidence_basis == "static_only"
}

pub(crate) fn ripr_swarm_plan_related_context_present(packet: &Value) -> bool {
    audit_get(packet, &["related_test_or_observer"])
        .and_then(ripr_swarm_plan_related_target_file)
        .is_some()
        || audit_get(packet, &["candidate_value_or_observer"])
            .and_then(ripr_swarm_plan_related_target_file)
            .is_some()
}

pub(crate) fn ripr_swarm_plan_related_target_file(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => ripr_swarm_attempt_related_target_file(value),
        Value::Object(object) => object
            .get("file")
            .and_then(Value::as_str)
            .and_then(ripr_swarm_attempt_workspace_relative_file_token),
        Value::Array(values) => values.iter().find_map(ripr_swarm_plan_related_target_file),
        Value::Null | Value::Bool(_) | Value::Number(_) => None,
    }
}

pub(crate) fn ripr_swarm_plan_allowed_edit_surface(packet: &Value) -> Vec<String> {
    if ripr_swarm_readiness_packet_projection_exclusion(packet, "missing_allowed_edit_surface")
        || ripr_swarm_readiness_packet_projection_exclusion(
            packet,
            CROSS_LANGUAGE_TARGET_UNRESOLVED_CATEGORY,
        )
    {
        return Vec::new();
    }

    let mut values = audit_string_array(packet, &["allowed_edit_surface"]).unwrap_or_default();
    if values.is_empty()
        && let Some(target) = audit_get(packet, &["related_test_or_observer"])
            .or_else(|| audit_get(packet, &["candidate_value_or_observer"]))
            .and_then(ripr_swarm_plan_related_target_file)
    {
        values.push(target);
    }
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .filter_map(|value| ripr_swarm_attempt_workspace_relative_file_token(&value))
        .filter(|value| seen.insert(value.clone()))
        .collect()
}

pub(crate) fn ripr_swarm_plan_non_missing_field(value: &Value, field: &str) -> bool {
    audit_non_empty_string(value, &[field])
        .is_some_and(|field_value| !ripr_swarm_plan_field_missing(&field_value))
}

pub(crate) fn ripr_swarm_plan_non_missing_any_field(value: &Value, fields: &[&str]) -> bool {
    fields
        .iter()
        .any(|field| ripr_swarm_plan_non_missing_field(value, field))
}

pub(crate) fn ripr_swarm_plan_field_missing(value: &str) -> bool {
    audit_guidance_field_is_missing(value)
        || matches!(
            value.trim(),
            "verify_command_unknown"
                | "receipt_command_unknown"
                | "receipt_path_unknown"
                | "repair_route_unknown"
                | "target_test_type_unknown"
                | "assertion_shape_unknown"
                | "confidence_basis_unknown"
        )
}

pub(crate) fn ripr_swarm_plan_ready_packets(
    report: &RiprSwarmPlanReport,
) -> Vec<RiprSwarmPlanPacket> {
    let mut packets = report
        .packets
        .iter()
        .filter(|packet| packet.swarm_state == "queued")
        .cloned()
        .collect::<Vec<_>>();
    packets.sort_by(ripr_swarm_plan_rank_order);
    packets.truncate(report.top_limit);
    packets
}

pub(crate) fn ripr_swarm_plan_blocked_packets(
    report: &RiprSwarmPlanReport,
) -> Vec<RiprSwarmPlanPacket> {
    let mut packets = report
        .packets
        .iter()
        .filter(|packet| packet.swarm_state != "queued")
        .cloned()
        .collect::<Vec<_>>();
    packets.sort_by(|left, right| {
        left.swarm_state
            .cmp(&right.swarm_state)
            .then_with(|| left.canonical_gap_id.cmp(&right.canonical_gap_id))
    });
    packets.truncate(report.top_limit);
    packets
}

pub(crate) fn ripr_swarm_plan_missing_verify_or_receipt_packets(
    report: &RiprSwarmPlanReport,
) -> Vec<RiprSwarmPlanPacket> {
    let mut packets = report
        .packets
        .iter()
        .filter(|packet| {
            packet.verify_command.is_none()
                || packet
                    .verify_command
                    .as_deref()
                    .is_some_and(ripr_swarm_plan_field_missing)
                || packet
                    .projection_exclusion_reasons
                    .iter()
                    .any(|reason| reason == "missing_verify_command")
                || packet.receipt_command_or_path.is_none()
                || packet
                    .receipt_command_or_path
                    .as_deref()
                    .is_some_and(ripr_swarm_plan_field_missing)
                || packet
                    .projection_exclusion_reasons
                    .iter()
                    .any(|reason| reason == "missing_receipt_command")
        })
        .cloned()
        .collect::<Vec<_>>();
    packets.sort_by(|left, right| left.canonical_gap_id.cmp(&right.canonical_gap_id));
    packets.truncate(report.top_limit);
    packets
}

pub(crate) fn ripr_swarm_plan_rank_order(
    left: &RiprSwarmPlanPacket,
    right: &RiprSwarmPlanPacket,
) -> std::cmp::Ordering {
    right
        .score
        .cmp(&left.score)
        .then_with(|| left.evidence_class.cmp(&right.evidence_class))
        .then_with(|| left.source_file.cmp(&right.source_file))
        .then_with(|| left.canonical_gap_id.cmp(&right.canonical_gap_id))
}

pub(crate) fn ripr_swarm_plan_packet_is_high_confidence(packet: &RiprSwarmPlanPacket) -> bool {
    packet.swarm_state == "queued"
        && packet.score >= 80
        && matches!(
            packet.confidence_basis.as_str(),
            "fixture_backed" | "calibrated" | "runtime_calibrated"
        )
}

pub(crate) fn ripr_swarm_plan_json(report: &RiprSwarmPlanReport) -> Result<String, String> {
    let value = serde_json::json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "report": "swarm-plan",
        "scope": "repo",
        "status": report.status,
        "run_status": report.runtime_status.state.clone(),
        "runtime_status": lane1_runtime_status_json(&report.runtime_status),
        "input": {
            "actionable_gaps": report.input_path,
            "state": report.input_state,
            "limitation": report.input_limitation,
        },
        "source": "actionable-gaps.packets",
        "source_summary": report.source_summary,
        "static_limitation_backlog": report.static_limitation_backlog,
        "top_limit": report.top_limit,
        "summary": ripr_swarm_plan_summary_json(report),
        "top_ready_packets": ripr_swarm_plan_packets_json(
            &ripr_swarm_plan_ready_packets(report)
        ),
        "top_blocked_packets": ripr_swarm_plan_packets_json(
            &ripr_swarm_plan_blocked_packets(report)
        ),
        "blocked_state_examples": ripr_swarm_plan_blocked_state_examples_json(report),
        "top_missing_verify_or_receipt": ripr_swarm_plan_packets_json(
            &ripr_swarm_plan_missing_verify_or_receipt_packets(report)
        ),
        "must_not_infer": [
            "do not consume raw findings as swarm work",
            "do not rank static limitations as repair-ready",
            "do not rank static-only predicate-boundary packets as swarm-ready without stronger evidence",
            "do not rank packets without receipt_command as swarm-ready",
            "do not rank packets without verify_command as high confidence",
            "do not edit files, call providers, generate tests, run mutation testing, or create receipts from this plan"
        ],
    });
    serde_json::to_string_pretty(&value).map_err(|err| err.to_string())
}

pub(crate) fn ripr_swarm_plan_summary_json(report: &RiprSwarmPlanReport) -> Value {
    let ready = report
        .packets
        .iter()
        .filter(|packet| packet.swarm_state == "queued")
        .count();
    let missing_verify = report
        .packets
        .iter()
        .filter(|packet| {
            packet.verify_command.is_none()
                || packet
                    .verify_command
                    .as_deref()
                    .is_some_and(ripr_swarm_plan_field_missing)
                || packet
                    .missing_context
                    .iter()
                    .any(|field| field == "verify_command")
                || packet.projection_exclusion_reasons.iter().any(|reason| {
                    reason == "missing_verify_command" || reason == "unbounded_verify_command"
                })
        })
        .count();
    let missing_receipt = report
        .packets
        .iter()
        .filter(|packet| {
            packet
                .missing_context
                .iter()
                .any(|field| field == "receipt_command")
                || packet
                    .projection_exclusion_reasons
                    .iter()
                    .any(|reason| reason == "missing_receipt_command")
        })
        .count();
    let related_context_missing = report
        .packets
        .iter()
        .filter(|packet| {
            !packet.related_test_or_observer_available
                || packet
                    .missing_context
                    .iter()
                    .any(|field| field == "related_test_or_observer")
                || packet
                    .projection_exclusion_reasons
                    .iter()
                    .any(|reason| reason == "missing_related_test_or_observer")
        })
        .count();
    serde_json::json!({
        "packets_total": report.packets.len(),
        "swarm_ready_packets": ready,
        "blocked_packets": report.packets.len().saturating_sub(ready),
        "blocked_by_missing_context_packets": ripr_swarm_plan_packet_state_count(
            report,
            "blocked_by_missing_context",
        ),
        "blocked_by_static_limitation_packets": ripr_swarm_plan_packet_state_count(
            report,
            "blocked_by_static_limitation",
        ),
        "blocked_by_public_projection_exclusion_packets": ripr_swarm_plan_packet_state_count(
            report,
            "blocked_by_public_projection_exclusion",
        ),
        "blocked_by_operator_judgment_packets": ripr_swarm_plan_packet_state_count(
            report,
            "blocked_by_operator_judgment",
        ),
        "public_projection_excluded_packets": report
            .packets
            .iter()
            .filter(|packet| packet.swarm_state == "blocked_by_public_projection_exclusion")
            .count(),
        "public_projection_exclusion_reasons": audit_count_rows_json(
            &ripr_swarm_plan_public_projection_exclusion_reason_counts(report)
        ),
        "missing_canonical_gap_id": report
            .packets
            .iter()
            .filter(|packet| {
                packet
                    .missing_context
                    .iter()
                    .any(|field| field == "canonical_gap_id")
                    || packet
                        .projection_exclusion_reasons
                        .iter()
                        .any(|reason| reason == "missing_canonical_gap_id")
            })
            .count(),
        "not_actionable_gap_state": report
            .packets
            .iter()
            .filter(|packet| {
                packet
                    .missing_context
                    .iter()
                    .any(|field| field == "actionable_gap_state")
                    || packet
                        .projection_exclusion_reasons
                        .iter()
                        .any(|reason| reason == "not_actionable_gap_state")
            })
            .count(),
        "missing_verify_command": missing_verify,
        "missing_receipt_command": missing_receipt,
        "missing_repair_kind": report
            .packets
            .iter()
            .filter(|packet| {
                packet
                    .missing_context
                    .iter()
                    .any(|field| field == "repair_kind")
                    || packet
                        .projection_exclusion_reasons
                        .iter()
                        .any(|reason| reason == "missing_repair_kind")
            })
            .count(),
        "missing_repair_route": report
            .packets
            .iter()
            .filter(|packet| {
                packet
                    .missing_context
                    .iter()
                    .any(|field| field == "repair_route")
                    || packet
                        .projection_exclusion_reasons
                        .iter()
                        .any(|reason| reason == "missing_repair_route")
            })
            .count(),
        "missing_target_test_shape": report
            .packets
            .iter()
            .filter(|packet| {
                packet
                    .missing_context
                    .iter()
                    .any(|field| field == "target_test_shape")
                    || packet
                        .projection_exclusion_reasons
                        .iter()
                        .any(|reason| reason == "missing_target_test_shape")
            })
            .count(),
        "missing_must_not_change": report
            .packets
            .iter()
            .filter(|packet| {
                packet
                    .missing_context
                    .iter()
                    .any(|field| field == "must_not_change")
                    || packet
                        .projection_exclusion_reasons
                        .iter()
                        .any(|reason| reason == "missing_must_not_change")
            })
            .count(),
        "missing_allowed_edit_surface": report
            .packets
            .iter()
            .filter(|packet| {
                packet
                    .missing_context
                    .iter()
                    .any(|field| field == "allowed_edit_surface")
                    || packet
                        .projection_exclusion_reasons
                        .iter()
                        .any(|reason| reason == "missing_allowed_edit_surface")
            })
            .count(),
        "missing_confidence": report
            .packets
            .iter()
            .filter(|packet| {
                packet
                    .missing_context
                    .iter()
                    .any(|field| field == "confidence_basis")
                    || packet
                        .projection_exclusion_reasons
                        .iter()
                        .any(|reason| reason == "missing_confidence")
            })
            .count(),
        "missing_raw_evidence_refs": report
            .packets
            .iter()
            .filter(|packet| {
                packet
                    .missing_context
                    .iter()
                    .any(|field| field == "raw_evidence_refs")
                    || packet
                        .projection_exclusion_reasons
                        .iter()
                        .any(|reason| reason == "missing_raw_evidence_refs")
            })
            .count(),
        "missing_related_test_or_observer": related_context_missing,
        "related_context_missing": related_context_missing,
        "static_limitation_packets": report
            .packets
            .iter()
            .filter(|packet| {
                packet.static_limitations_count > 0
                    || packet
                        .blocked_reasons
                        .iter()
                        .any(|reason| reason == "static_limitation_present")
                    || packet
                        .projection_exclusion_reasons
                        .iter()
                        .any(|reason| reason == "static_limitation_present")
            })
            .count(),
        "static_limitation_backlog_packets":
            ripr_swarm_static_limitation_backlog_packet_count(&report.static_limitation_backlog),
        "static_limitation_backlog_signals":
            ripr_swarm_static_limitation_backlog_signal_count(&report.static_limitation_backlog),
        "high_confidence_packets": report
            .packets
            .iter()
            .filter(|packet| ripr_swarm_plan_packet_is_high_confidence(packet))
            .count(),
    })
}

pub(crate) fn ripr_swarm_static_limitation_backlog_packet_count(backlog: &Value) -> usize {
    audit_array(backlog, &["limitation_backlog_packets"]).len()
}

pub(crate) fn ripr_swarm_static_limitation_backlog_signal_count(backlog: &Value) -> usize {
    let packet_signals = audit_array(backlog, &["limitation_backlog_packets"])
        .iter()
        .filter_map(|packet| audit_usize(packet, &["signal_count"]))
        .sum::<usize>();
    let route_signals = audit_array(backlog, &["top_repair_routes"])
        .iter()
        .filter_map(|row| audit_usize(row, &["count"]))
        .sum::<usize>();
    let category_signals = audit_array(backlog, &["top_categories"])
        .iter()
        .filter_map(|row| audit_usize(row, &["count"]))
        .sum::<usize>();

    packet_signals.max(route_signals).max(category_signals)
}

pub(crate) fn ripr_swarm_plan_packet_state_count(
    report: &RiprSwarmPlanReport,
    state: &str,
) -> usize {
    report
        .packets
        .iter()
        .filter(|packet| packet.swarm_state == state)
        .count()
}

pub(crate) fn ripr_swarm_plan_public_projection_exclusion_reason_counts(
    report: &RiprSwarmPlanReport,
) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for packet in report
        .packets
        .iter()
        .filter(|packet| packet.swarm_state == "blocked_by_public_projection_exclusion")
    {
        if packet.projection_exclusion_reasons.is_empty() {
            audit_increment(&mut counts, "public_projection_eligible_false");
        } else {
            for reason in &packet.projection_exclusion_reasons {
                audit_increment(&mut counts, reason);
            }
        }
    }
    counts
}

pub(crate) fn ripr_swarm_plan_packets_json(packets: &[RiprSwarmPlanPacket]) -> Vec<Value> {
    packets
        .iter()
        .map(|packet| {
            serde_json::json!({
                "packet_id": packet.packet_id,
                "canonical_gap_id": packet.canonical_gap_id,
                "evidence_class": packet.evidence_class,
                "source_file": packet.source_file,
                "repair_kind": packet.repair_kind,
                "target_test_type": packet.target_test_type,
                "assertion_shape": packet.assertion_shape,
                "confidence_basis": packet.confidence_basis,
                "swarm_state": packet.swarm_state,
                "score": packet.score,
                "expected_canonical_gap_delta": packet.expected_canonical_gap_delta,
                "readiness_reasons": packet.readiness_reasons,
                "blocked_reasons": packet.blocked_reasons,
                "missing_context": packet.missing_context,
                "verify_command": packet.verify_command,
                "receipt_command": packet.receipt_command_or_path,
                "related_test_or_observer_available": packet.related_test_or_observer_available,
                "must_not_change": packet.must_not_change,
                "must_not_change_count": packet.must_not_change_count,
                "allowed_edit_surface": packet.allowed_edit_surface,
                "allowed_edit_surface_count": packet.allowed_edit_surface_count,
                "raw_findings_count": packet.raw_findings_count,
                "raw_findings_supporting_only": true,
                "static_limitations_count": packet.static_limitations_count,
                "public_projection_eligible": packet.public_projection_eligible,
                "projection_exclusion_reasons": packet.projection_exclusion_reasons,
            })
        })
        .collect()
}

pub(crate) fn ripr_swarm_plan_blocked_state_examples_json(
    report: &RiprSwarmPlanReport,
) -> Vec<Value> {
    let mut rows = Vec::new();
    ripr_swarm_plan_push_blocked_state_example(
        &mut rows,
        &report.packets,
        "blocked_by_missing_context",
        |packet| packet.swarm_state == "blocked_by_missing_context",
    );
    ripr_swarm_plan_push_blocked_state_example(
        &mut rows,
        &report.packets,
        "blocked_by_static_limitation",
        |packet| packet.swarm_state == "blocked_by_static_limitation",
    );
    ripr_swarm_plan_push_blocked_state_example(
        &mut rows,
        &report.packets,
        "blocked_by_public_projection_exclusion",
        |packet| packet.swarm_state == "blocked_by_public_projection_exclusion",
    );
    ripr_swarm_plan_push_blocked_state_example(
        &mut rows,
        &report.packets,
        "blocked_by_operator_judgment",
        |packet| packet.swarm_state == "blocked_by_operator_judgment",
    );
    ripr_swarm_plan_push_blocked_state_example(
        &mut rows,
        &report.packets,
        "missing_canonical_gap_id",
        |packet| {
            packet
                .missing_context
                .iter()
                .any(|field| field == "canonical_gap_id")
                || packet
                    .projection_exclusion_reasons
                    .iter()
                    .any(|reason| reason == "missing_canonical_gap_id")
        },
    );
    ripr_swarm_plan_push_blocked_state_example(
        &mut rows,
        &report.packets,
        "not_actionable_gap_state",
        |packet| {
            packet
                .missing_context
                .iter()
                .any(|field| field == "actionable_gap_state")
                || packet
                    .projection_exclusion_reasons
                    .iter()
                    .any(|reason| reason == "not_actionable_gap_state")
        },
    );
    ripr_swarm_plan_push_blocked_state_example(
        &mut rows,
        &report.packets,
        "missing_verify_command",
        |packet| {
            packet
                .missing_context
                .iter()
                .any(|field| field == "verify_command")
                || packet
                    .projection_exclusion_reasons
                    .iter()
                    .any(|reason| reason == "missing_verify_command")
        },
    );
    ripr_swarm_plan_push_blocked_state_example(
        &mut rows,
        &report.packets,
        "missing_receipt_command",
        |packet| {
            packet
                .missing_context
                .iter()
                .any(|field| field == "receipt_command")
                || packet
                    .projection_exclusion_reasons
                    .iter()
                    .any(|reason| reason == "missing_receipt_command")
        },
    );
    ripr_swarm_plan_push_blocked_state_example(
        &mut rows,
        &report.packets,
        "missing_repair_kind",
        |packet| {
            packet
                .missing_context
                .iter()
                .any(|field| field == "repair_kind")
                || packet
                    .projection_exclusion_reasons
                    .iter()
                    .any(|reason| reason == "missing_repair_kind")
        },
    );
    ripr_swarm_plan_push_blocked_state_example(
        &mut rows,
        &report.packets,
        "missing_repair_route",
        |packet| {
            packet
                .missing_context
                .iter()
                .any(|field| field == "repair_route")
        },
    );
    ripr_swarm_plan_push_blocked_state_example(
        &mut rows,
        &report.packets,
        "missing_target_test_shape",
        |packet| {
            packet
                .missing_context
                .iter()
                .any(|field| field == "target_test_shape")
                || packet
                    .projection_exclusion_reasons
                    .iter()
                    .any(|reason| reason == "missing_target_test_shape")
        },
    );
    ripr_swarm_plan_push_blocked_state_example(
        &mut rows,
        &report.packets,
        "missing_related_test_or_observer",
        |packet| {
            !packet.related_test_or_observer_available
                || packet
                    .missing_context
                    .iter()
                    .any(|field| field == "related_test_or_observer")
        },
    );
    ripr_swarm_plan_push_blocked_state_example(
        &mut rows,
        &report.packets,
        "missing_must_not_change",
        |packet| {
            packet
                .missing_context
                .iter()
                .any(|field| field == "must_not_change")
                || packet
                    .projection_exclusion_reasons
                    .iter()
                    .any(|reason| reason == "missing_must_not_change")
        },
    );
    ripr_swarm_plan_push_blocked_state_example(
        &mut rows,
        &report.packets,
        "missing_allowed_edit_surface",
        |packet| {
            packet
                .missing_context
                .iter()
                .any(|field| field == "allowed_edit_surface")
                || packet
                    .projection_exclusion_reasons
                    .iter()
                    .any(|reason| reason == "missing_allowed_edit_surface")
        },
    );
    ripr_swarm_plan_push_blocked_state_example(
        &mut rows,
        &report.packets,
        "missing_confidence",
        |packet| {
            packet
                .missing_context
                .iter()
                .any(|field| field == "confidence_basis")
                || packet
                    .projection_exclusion_reasons
                    .iter()
                    .any(|reason| reason == "missing_confidence")
        },
    );
    ripr_swarm_plan_push_blocked_state_example(
        &mut rows,
        &report.packets,
        "missing_raw_evidence_refs",
        |packet| {
            packet
                .missing_context
                .iter()
                .any(|field| field == "raw_evidence_refs")
                || packet
                    .projection_exclusion_reasons
                    .iter()
                    .any(|reason| reason == "missing_raw_evidence_refs")
        },
    );
    rows
}

pub(crate) fn ripr_swarm_plan_push_blocked_state_example<F>(
    rows: &mut Vec<Value>,
    packets: &[RiprSwarmPlanPacket],
    state: &str,
    predicate: F,
) where
    F: Fn(&RiprSwarmPlanPacket) -> bool,
{
    if let Some(packet) = packets.iter().find(|packet| predicate(packet)) {
        rows.push(serde_json::json!({
            "state": state,
            "example_packet_id": packet.packet_id,
            "example_canonical_gap_id": packet.canonical_gap_id,
            "example_repair_kind": packet.repair_kind,
            "example_missing_context": packet.missing_context,
            "example_projection_exclusion_reasons": packet.projection_exclusion_reasons,
            "example_blocked_reasons": packet.blocked_reasons,
        }));
    }
}

pub(crate) fn ripr_swarm_plan_markdown(report: &RiprSwarmPlanReport) -> String {
    let summary = ripr_swarm_plan_summary_json(report);
    let mut out = String::new();
    out.push_str("# RIPR Swarm Plan\n\n");
    out.push_str(&format!(
        "Run status: `{}`\n\n",
        report.runtime_status.state
    ));
    out.push_str(
        "Advisory dry-run plan over actionable canonical gap packets. Raw findings remain supporting evidence only.\n\n",
    );
    lane1_runtime_status_push_markdown(&mut out, &report.runtime_status);
    out.push_str("## Input\n\n");
    out.push_str("| Field | Value |\n");
    out.push_str("| --- | --- |\n");
    out.push_str(&format!(
        "| Actionable gaps | `{}` |\n",
        audit_markdown_cell(&report.input_path)
    ));
    out.push_str(&format!(
        "| State | `{}` |\n",
        audit_markdown_cell(&report.input_state)
    ));
    if let Some(limitation) = &report.input_limitation {
        out.push_str(&format!(
            "| Limitation | {} |\n",
            audit_markdown_cell(limitation)
        ));
    }
    out.push('\n');
    out.push_str("## Summary\n\n");
    out.push_str("| Metric | Count |\n");
    out.push_str("| --- | ---: |\n");
    for key in [
        "packets_total",
        "swarm_ready_packets",
        "blocked_packets",
        "blocked_by_missing_context_packets",
        "blocked_by_static_limitation_packets",
        "blocked_by_public_projection_exclusion_packets",
        "blocked_by_operator_judgment_packets",
        "missing_canonical_gap_id",
        "missing_verify_command",
        "missing_verify_result",
        "missing_receipt_command",
        "missing_repair_kind",
        "missing_repair_route",
        "missing_target_test_shape",
        "missing_must_not_change",
        "missing_allowed_edit_surface",
        "missing_confidence",
        "missing_raw_evidence_refs",
        "missing_related_test_or_observer",
        "related_context_missing",
        "static_limitation_packets",
        "static_limitation_backlog_packets",
        "static_limitation_backlog_signals",
        "high_confidence_packets",
    ] {
        out.push_str(&format!(
            "| {} | {} |\n",
            key.replace('_', " "),
            summary[key].as_u64().unwrap_or(0)
        ));
    }
    out.push('\n');
    let public_projection_exclusion_reasons = summary["public_projection_exclusion_reasons"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    ripr_swarm_push_count_rows_markdown(
        &mut out,
        "Public Projection Exclusion Reasons",
        &public_projection_exclusion_reasons,
    );
    ripr_swarm_push_static_limitation_backlog_markdown(&mut out, &report.static_limitation_backlog);
    ripr_swarm_plan_push_packet_table(
        &mut out,
        "Top Swarm-Ready Packets",
        &ripr_swarm_plan_ready_packets(report),
    );
    ripr_swarm_plan_push_packet_table(
        &mut out,
        "Top Blocked Packets",
        &ripr_swarm_plan_blocked_packets(report),
    );
    ripr_swarm_plan_push_packet_table(
        &mut out,
        "Top Packets Missing Verify Or Receipt",
        &ripr_swarm_plan_missing_verify_or_receipt_packets(report),
    );
    out.push_str("## Must Not Infer\n\n");
    out.push_str("- Do not consume raw findings as swarm work.\n");
    out.push_str("- Do not rank static limitations as repair-ready.\n");
    out.push_str(
        "- Do not rank static-only predicate-boundary packets as swarm-ready without stronger evidence.\n",
    );
    out.push_str("- Do not rank packets without `receipt_command` as swarm-ready.\n");
    out.push_str("- Do not rank packets without `verify_command` as high confidence.\n");
    out.push_str("- Do not edit files, call providers, generate tests, run mutation testing, or create receipts from this plan.\n");
    out
}

pub(crate) fn ripr_swarm_push_count_rows_markdown(out: &mut String, title: &str, rows: &[Value]) {
    if rows.is_empty() {
        return;
    }
    out.push_str(&format!("## {}\n\n", audit_markdown_cell(title)));
    out.push_str("| Reason | Count |\n");
    out.push_str("| --- | ---: |\n");
    for row in rows {
        let Some(label) = audit_non_empty_string(row, &["label"]) else {
            continue;
        };
        let count = audit_usize(row, &["count"]).unwrap_or_default();
        out.push_str(&format!(
            "| {} | {} |\n",
            audit_markdown_cell(&label),
            count
        ));
    }
    out.push('\n');
}

pub(crate) fn ripr_swarm_push_static_limitation_backlog_markdown(
    out: &mut String,
    backlog: &Value,
) {
    let top_categories = audit_array(backlog, &["top_categories"]);
    let top_subroutes = audit_array(backlog, &["top_subroutes"]);
    let top_repair_routes = audit_array(backlog, &["top_repair_routes"]);
    let limitation_backlog_packets = audit_array(backlog, &["limitation_backlog_packets"]);
    if top_categories.is_empty()
        && top_subroutes.is_empty()
        && top_repair_routes.is_empty()
        && limitation_backlog_packets.is_empty()
    {
        return;
    }

    out.push_str("## Static Limitation Backlog\n\n");
    out.push_str("Named limitations are analyzer backlog, not repair-ready packet work.\n\n");
    if !limitation_backlog_packets.is_empty() {
        out.push_str("### Backlog Packets\n\n");
        out.push_str(
            "| Packet | Category | Subroute | Count | Dominant class | Repair route | Unlock condition |\n",
        );
        out.push_str("| --- | --- | --- | ---: | --- | --- | --- |\n");
        for packet in limitation_backlog_packets {
            let packet_id = audit_non_empty_string(packet, &["packet_id"])
                .unwrap_or_else(|| "limitation:unknown".to_string());
            let category = audit_non_empty_string(packet, &["limitation_category"])
                .unwrap_or_else(|| "unknown".to_string());
            let subroute = audit_non_empty_string(packet, &["limitation_subroute"])
                .unwrap_or_else(|| audit_identifier_slug(&category));
            let repair_route = audit_non_empty_string(packet, &["repair_route"])
                .unwrap_or_else(|| static_limitation_repair_route(&category).to_string());
            let dominant_evidence_class =
                audit_non_empty_string(packet, &["dominant_evidence_class"])
                    .unwrap_or_else(|| "unknown".to_string());
            let unlock_condition = audit_non_empty_string(packet, &["unlock_condition"])
                .unwrap_or_else(|| {
                    "inspect the analyzer route before attempting repairs".to_string()
                });
            out.push_str(&format!(
                "| `{}` | `{}` | `{}` | {} | `{}` | `{}` | {} |\n",
                audit_markdown_cell(&packet_id),
                audit_markdown_cell(&category),
                audit_markdown_cell(&subroute),
                audit_usize(packet, &["signal_count"]).unwrap_or_default(),
                audit_markdown_cell(&dominant_evidence_class),
                audit_markdown_cell(&repair_route),
                audit_markdown_cell(&unlock_condition)
            ));
        }
        out.push('\n');
    }
    if !top_subroutes.is_empty() {
        out.push_str("### Top Subroutes\n\n");
        out.push_str("| Category | Subroute | Count | Repair route |\n");
        out.push_str("| --- | --- | ---: | --- |\n");
        for row in top_subroutes {
            let category =
                audit_non_empty_string(row, &["category"]).unwrap_or_else(|| "unknown".to_string());
            let subroute = audit_non_empty_string(row, &["subroute"])
                .unwrap_or_else(|| audit_identifier_slug(&category));
            let repair_route = audit_non_empty_string(row, &["repair_route"])
                .unwrap_or_else(|| static_limitation_repair_route(&category).to_string());
            out.push_str(&format!(
                "| `{}` | `{}` | {} | `{}` |\n",
                audit_markdown_cell(&category),
                audit_markdown_cell(&subroute),
                audit_usize(row, &["count"]).unwrap_or_default(),
                audit_markdown_cell(&repair_route)
            ));
        }
        out.push('\n');
    }
    if !top_categories.is_empty() {
        out.push_str("### Top Categories\n\n");
        out.push_str("| Category | Count | Repair route |\n");
        out.push_str("| --- | ---: | --- |\n");
        for row in top_categories {
            let category = audit_non_empty_string(row, &["category"])
                .or_else(|| audit_non_empty_string(row, &["label"]))
                .unwrap_or_else(|| "unknown".to_string());
            let repair_route = audit_non_empty_string(row, &["repair_route"])
                .unwrap_or_else(|| "analysis/static-limitation-taxonomy".to_string());
            out.push_str(&format!(
                "| `{}` | {} | `{}` |\n",
                audit_markdown_cell(&category),
                audit_usize(row, &["count"]).unwrap_or_default(),
                audit_markdown_cell(&repair_route)
            ));
        }
        out.push('\n');
    }
    if !top_repair_routes.is_empty() {
        out.push_str("### Top Repair Routes\n\n");
        out.push_str("| Repair route | Count |\n");
        out.push_str("| --- | ---: |\n");
        for row in top_repair_routes {
            let repair_route = audit_non_empty_string(row, &["repair_route"])
                .or_else(|| audit_non_empty_string(row, &["label"]))
                .unwrap_or_else(|| "analysis/static-limitation-taxonomy".to_string());
            out.push_str(&format!(
                "| `{}` | {} |\n",
                audit_markdown_cell(&repair_route),
                audit_usize(row, &["count"]).unwrap_or_default()
            ));
        }
        out.push('\n');
    }
}

pub(crate) fn ripr_swarm_plan_push_packet_table(
    out: &mut String,
    title: &str,
    packets: &[RiprSwarmPlanPacket],
) {
    out.push_str(&format!("## {title}\n\n"));
    if packets.is_empty() {
        out.push_str("No packets in this section.\n\n");
        return;
    }
    out.push_str(
        "| Gap | State | Score | Repair | Allowed edit surface | Must not change | Verify | Receipt | Blocked reasons |\n",
    );
    out.push_str("| --- | --- | ---: | --- | --- | --- | --- | --- | --- |\n");
    for packet in packets {
        let allowed_edit_surface = if packet.allowed_edit_surface.is_empty() {
            "missing".to_string()
        } else {
            packet.allowed_edit_surface.join(", ")
        };
        let must_not_change = if packet.must_not_change.is_empty() {
            "missing".to_string()
        } else {
            packet.must_not_change.join(", ")
        };
        out.push_str(&format!(
            "| `{}` | `{}` | {} | `{}` | {} | {} | {} | {} | {} |\n",
            audit_markdown_cell(&packet.canonical_gap_id),
            audit_markdown_cell(&packet.swarm_state),
            packet.score,
            audit_markdown_cell(&packet.repair_kind),
            audit_markdown_cell(&allowed_edit_surface),
            audit_markdown_cell(&must_not_change),
            audit_markdown_cell(packet.verify_command.as_deref().unwrap_or("missing")),
            audit_markdown_cell(
                packet
                    .receipt_command_or_path
                    .as_deref()
                    .unwrap_or("missing")
            ),
            audit_markdown_cell(&packet.blocked_reasons.join(", "))
        ));
    }
    out.push('\n');
}

pub(crate) fn ripr_swarm_attempt_ledger_report(
    args: &RiprSwarmAttemptLedgerArgs,
) -> Result<(), String> {
    let generated_at = generated_at_unix_ms()?;
    let (swarm_plan_state, swarm_plan_limitation, swarm_plan) =
        ripr_swarm_read_optional_json(&args.swarm_plan_path);
    let (
        actionable_gap_outcomes_state,
        actionable_gap_outcomes_limitation,
        actionable_gap_outcomes,
    ) = ripr_swarm_read_optional_json(&args.actionable_gap_outcomes_path);
    let (prior_ledger_state, prior_ledger_limitation, prior_ledger) =
        ripr_swarm_read_optional_json(&args.prior_ledger_path);
    let (real_repair_attempts_state, real_repair_attempts_limitation, real_repair_attempts) =
        ripr_swarm_read_optional_json(&args.real_repair_attempts_path);
    let report = ripr_swarm_attempt_ledger_from_values_with_real_repair_attempts(
        generated_at,
        RiprSwarmReadinessInput {
            path: normalize_path(&args.swarm_plan_path),
            state: swarm_plan_state,
            limitation: swarm_plan_limitation,
            value: swarm_plan.as_ref(),
        },
        RiprSwarmReadinessInput {
            path: normalize_path(&args.actionable_gap_outcomes_path),
            state: actionable_gap_outcomes_state,
            limitation: actionable_gap_outcomes_limitation,
            value: actionable_gap_outcomes.as_ref(),
        },
        RiprSwarmReadinessInput {
            path: normalize_path(&args.prior_ledger_path),
            state: prior_ledger_state,
            limitation: prior_ledger_limitation,
            value: prior_ledger.as_ref(),
        },
        RiprSwarmReadinessInput {
            path: normalize_path(&args.real_repair_attempts_path),
            state: real_repair_attempts_state,
            limitation: real_repair_attempts_limitation,
            value: real_repair_attempts.as_ref(),
        },
    );

    write_report(
        "swarm-attempt-ledger.json",
        &ripr_swarm_attempt_ledger_json(&report)?,
    )?;
    write_report(
        "swarm-attempt-ledger.md",
        &ripr_swarm_attempt_ledger_markdown(&report),
    )
}

pub(crate) fn ripr_swarm_route_quality_report(args: &[String]) -> Result<(), String> {
    let mut attempt_ledger_path = PathBuf::from("target/ripr/reports/swarm-attempt-ledger.json");
    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--attempt-ledger" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    return Err("route-quality --attempt-ledger requires a path".to_string());
                };
                attempt_ledger_path = PathBuf::from(value);
            }
            other => {
                return Err(format!(
                    "unknown route-quality argument `{other}`\nusage: cargo xtask route-quality [--attempt-ledger <path>]"
                ));
            }
        }
        index += 1;
    }
    let generated_at = generated_at_unix_ms()?;
    let (ledger_state, ledger_limitation, ledger_value) =
        ripr_swarm_read_optional_json(&attempt_ledger_path);
    let report = ripr_swarm_route_quality_from_ledger_value(
        generated_at,
        normalize_path(&attempt_ledger_path),
        ledger_state,
        ledger_limitation,
        ledger_value.as_ref(),
    );
    write_report(
        "route-quality.json",
        &ripr_swarm_route_quality_report_json(&report)?,
    )?;
    write_report(
        "route-quality.md",
        &ripr_swarm_route_quality_report_markdown(&report),
    )
}

pub(crate) fn ripr_swarm_route_quality_from_ledger_value(
    generated_at: String,
    attempt_ledger_path: String,
    attempt_ledger_state: String,
    attempt_ledger_limitation: Option<String>,
    attempt_ledger_value: Option<&Value>,
) -> RiprSwarmRouteQualityReport {
    let (
        status,
        runtime_status,
        rows_latest,
        rows_historical,
        rows_language_latest,
        rows_language_historical,
    ) = if let Some(ledger) = attempt_ledger_value {
        let attempts = ripr_swarm_attempt_ledger_entries_from_value(ledger);
        let latest_attempts = ripr_swarm_attempt_ledger_latest_attempts(&attempts);
        let rows_latest = ripr_swarm_attempt_ledger_repair_route_quality(&latest_attempts);
        let rows_language_latest =
            ripr_swarm_attempt_ledger_language_repair_route_quality(&latest_attempts);
        let rows_historical = ripr_swarm_attempt_ledger_repair_route_quality(&attempts);
        let rows_language_historical =
            ripr_swarm_attempt_ledger_language_repair_route_quality(&attempts);
        let runtime_status = lane1_runtime_status_from_report_value(ledger)
            .unwrap_or_else(lane1_runtime_status_full);
        let status = "advisory".to_string();
        (
            status,
            runtime_status,
            rows_latest,
            rows_historical,
            rows_language_latest,
            rows_language_historical,
        )
    } else {
        let runtime_status = lane1_runtime_status_limited_input(
            "attempt_ledger_input",
            "swarm-attempt-ledger",
            Some(&attempt_ledger_path),
            "attempt_ledger_input_unavailable",
            "run cargo xtask ripr-swarm attempt-ledger before building the route-quality report",
            false,
        );
        (
            "blocked".to_string(),
            runtime_status,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
    };
    RiprSwarmRouteQualityReport {
        status,
        runtime_status,
        generated_at,
        attempt_ledger_path,
        attempt_ledger_state,
        attempt_ledger_limitation,
        repair_route_quality_latest: rows_latest,
        repair_route_quality_historical: rows_historical,
        language_repair_route_quality_latest: rows_language_latest,
        language_repair_route_quality_historical: rows_language_historical,
    }
}

pub(crate) fn ripr_swarm_route_quality_report_json(
    report: &RiprSwarmRouteQualityReport,
) -> Result<String, String> {
    let value = serde_json::json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "report": "route-quality",
        "scope": "repo",
        "status": report.status,
        "run_status": report.runtime_status.state.clone(),
        "runtime_status": lane1_runtime_status_json(&report.runtime_status),
        "generated_at": report.generated_at,
        "metadata": {
            "attempt_ledger_path": report.attempt_ledger_path,
            "attempt_ledger_state": report.attempt_ledger_state,
            "attempt_ledger_limitation": report.attempt_ledger_limitation,
        },
        "repair_route_quality_latest": ripr_swarm_repair_route_quality_json(&report.repair_route_quality_latest),
        "repair_route_quality_historical": ripr_swarm_repair_route_quality_json(&report.repair_route_quality_historical),
        "language_repair_route_quality_latest": ripr_swarm_repair_route_quality_json(&report.language_repair_route_quality_latest),
        "language_repair_route_quality_historical": ripr_swarm_repair_route_quality_json(&report.language_repair_route_quality_historical),
        "must_not_infer": [
            "route-quality rows group latest attempt outcomes by repair_kind; they are a grouping signal, not a gate or ranking",
            "success_rate is (improved + resolved + expected_unchanged) / attempted and is null when attempted == 0; it does not weight by receipt presence",
            "empty arrays mean no attempts were found with matching outcomes; they do not imply zero quality",
            "route-quality does not report orphan receipt sources or stale receipt counts; no producer for those fields exists yet",
            "route-quality counts do not change public badge semantics or CI gate mode",
            "this report is advisory; do not promote or downgrade actionability from route-quality evidence alone"
        ],
    });
    serde_json::to_string_pretty(&value)
        .map(|mut rendered| {
            rendered.push('\n');
            rendered
        })
        .map_err(|err| format!("failed to render route-quality JSON: {err}"))
}

pub(crate) fn ripr_swarm_route_quality_report_markdown(
    report: &RiprSwarmRouteQualityReport,
) -> String {
    let mut out = String::new();
    out.push_str("# RIPR Route Quality Report\n\n");
    out.push_str(&format!(
        "Run status: `{}`\n\n",
        report.runtime_status.state
    ));
    out.push_str("Advisory grouping of repair-route outcomes by `repair_kind`. Does not execute repairs, create receipts, or change gate semantics.\n\n");
    lane1_runtime_status_push_markdown(&mut out, &report.runtime_status);
    out.push_str("## Inputs\n\n");
    out.push_str("| Input | State | Path | Limitation |\n");
    out.push_str("| --- | --- | --- | --- |\n");
    out.push_str(&format!(
        "| attempt ledger | `{}` | `{}` | {} |\n\n",
        audit_markdown_cell(&report.attempt_ledger_state),
        audit_markdown_cell(&report.attempt_ledger_path),
        audit_markdown_cell(report.attempt_ledger_limitation.as_deref().unwrap_or(""))
    ));
    out.push_str("## Latest Repair Route Quality\n\n");
    ripr_swarm_push_repair_route_quality_table(&mut out, &report.repair_route_quality_latest);
    out.push_str("## Latest Repair Route Quality By Language\n\n");
    ripr_swarm_push_repair_route_quality_table(
        &mut out,
        &report.language_repair_route_quality_latest,
    );
    out.push_str("## Historical Repair Route Quality\n\n");
    out.push_str("Durable history rows preserve older unchanged, regressed, and no-receipt attempts after a later attempt improves or resolves the same canonical gap.\n\n");
    ripr_swarm_push_repair_route_quality_table(&mut out, &report.repair_route_quality_historical);
    out.push_str("## Historical Repair Route Quality By Language\n\n");
    ripr_swarm_push_repair_route_quality_table(
        &mut out,
        &report.language_repair_route_quality_historical,
    );
    out.push_str("## Must Not Infer\n\n");
    out.push_str("- Route-quality rows group latest attempt outcomes by `repair_kind`; they are a grouping signal, not a ranking gate.\n");
    out.push_str("- `repair_kind_success_rate` is `(improved + resolved + expected_unchanged) / attempted`; it is `null` when `attempted == 0` — never `0.0` as a fake rate.\n");
    out.push_str("- Empty arrays mean no attempts with matching outcomes were found; they do not imply zero quality.\n");
    out.push_str("- Orphan receipt sources and stale receipt counts are not reported here; no real producer for those fields exists yet.\n");
    out.push_str("- Route-quality counts do not change public badge semantics or CI gate mode.\n");
    out.push_str("- This report is advisory; do not promote or downgrade actionability from route-quality evidence alone.\n");
    out
}

#[cfg(test)]
pub(crate) fn ripr_swarm_attempt_ledger_from_values(
    generated_at: String,
    swarm_plan: RiprSwarmReadinessInput<'_>,
    actionable_gap_outcomes: RiprSwarmReadinessInput<'_>,
    prior_ledger: RiprSwarmReadinessInput<'_>,
) -> RiprSwarmAttemptLedgerReport {
    ripr_swarm_attempt_ledger_from_values_with_real_repair_attempts(
        generated_at,
        swarm_plan,
        actionable_gap_outcomes,
        prior_ledger,
        RiprSwarmReadinessInput {
            path: normalize_path(Path::new(REAL_REPAIR_ATTEMPTS_CORPUS)),
            state: "not_supplied".to_string(),
            limitation: Some("real repair attempts input was not supplied".to_string()),
            value: None,
        },
    )
}

pub(crate) fn ripr_swarm_attempt_ledger_from_values_with_real_repair_attempts(
    generated_at: String,
    swarm_plan: RiprSwarmReadinessInput<'_>,
    actionable_gap_outcomes: RiprSwarmReadinessInput<'_>,
    prior_ledger: RiprSwarmReadinessInput<'_>,
    real_repair_attempts: RiprSwarmReadinessInput<'_>,
) -> RiprSwarmAttemptLedgerReport {
    let plan_packets = ripr_swarm_attempt_ledger_plan_packet_index(swarm_plan.value);
    let mut attempts = prior_ledger
        .value
        .map(|ledger| ripr_swarm_attempt_ledger_prior_entries_from_value(ledger, &plan_packets))
        .unwrap_or_default();
    if let Some(real_attempts) = real_repair_attempts.value {
        attempts.extend(ripr_swarm_attempt_ledger_entries_from_real_repair_attempts(
            &generated_at,
            real_attempts,
        ));
    }
    if let Some(outcomes) = actionable_gap_outcomes.value {
        attempts.extend(ripr_swarm_attempt_ledger_entries_from_outcomes(
            &generated_at,
            outcomes,
            &plan_packets,
        ));
    }
    attempts.extend(
        ripr_swarm_attempt_ledger_current_plan_not_attempted_entries(
            &generated_at,
            swarm_plan.value,
            &attempts,
        ),
    );
    attempts = ripr_swarm_attempt_ledger_dedupe_attempts(attempts);
    let latest_attempts = ripr_swarm_attempt_ledger_latest_attempts(&attempts);
    let repair_route_quality = ripr_swarm_attempt_ledger_repair_route_quality(&latest_attempts);
    let language_repair_route_quality =
        ripr_swarm_attempt_ledger_language_repair_route_quality(&latest_attempts);
    let historical_repair_route_quality = ripr_swarm_attempt_ledger_repair_route_quality(&attempts);
    let historical_language_repair_route_quality =
        ripr_swarm_attempt_ledger_language_repair_route_quality(&attempts);
    let top_missing_evidence_fields =
        ripr_swarm_attempt_ledger_top_missing_evidence_fields(&latest_attempts);
    let orphaned_receipts = actionable_gap_outcomes
        .value
        .map(|outcomes| audit_array(outcomes, &["orphaned_receipts"]).to_vec())
        .unwrap_or_default();
    let status = if actionable_gap_outcomes.value.is_some() {
        "advisory"
    } else {
        "blocked"
    }
    .to_string();
    let runtime_status =
        ripr_swarm_attempt_ledger_runtime_status(&swarm_plan, &actionable_gap_outcomes);

    RiprSwarmAttemptLedgerReport {
        status,
        runtime_status,
        generated_at,
        swarm_plan_path: swarm_plan.path,
        swarm_plan_state: swarm_plan.state,
        swarm_plan_limitation: swarm_plan.limitation,
        actionable_gap_outcomes_path: actionable_gap_outcomes.path,
        actionable_gap_outcomes_state: actionable_gap_outcomes.state,
        actionable_gap_outcomes_limitation: actionable_gap_outcomes.limitation,
        prior_ledger_path: prior_ledger.path,
        prior_ledger_state: prior_ledger.state,
        prior_ledger_limitation: prior_ledger.limitation,
        real_repair_attempts_path: real_repair_attempts.path,
        real_repair_attempts_state: real_repair_attempts.state,
        real_repair_attempts_limitation: real_repair_attempts.limitation,
        attempts,
        latest_attempts,
        repair_route_quality,
        language_repair_route_quality,
        historical_repair_route_quality,
        historical_language_repair_route_quality,
        top_missing_evidence_fields,
        orphaned_receipts,
    }
}

pub(crate) fn ripr_swarm_attempt_ledger_runtime_status(
    swarm_plan: &RiprSwarmReadinessInput<'_>,
    actionable_gap_outcomes: &RiprSwarmReadinessInput<'_>,
) -> Lane1RuntimeStatus {
    if actionable_gap_outcomes.value.is_none() {
        return lane1_runtime_status_limited_input(
            "actionable_gap_outcomes_input",
            "actionable-gap-outcomes",
            Some(&actionable_gap_outcomes.path),
            "actionable_gap_outcomes_input_unavailable",
            "run cargo xtask actionable-gap-outcomes before building the attempt ledger",
            false,
        );
    }
    let mut limited_inputs = Vec::new();
    if let Some(outcomes) = actionable_gap_outcomes.value
        && let Some(status) = lane1_runtime_status_from_report_value(outcomes)
        && status.state != "full"
    {
        limited_inputs.push(lane1_runtime_status_with_input_path(
            status,
            "actionable_gap_outcomes_input",
            &actionable_gap_outcomes.path,
        ));
    }
    if let Some(plan) = swarm_plan.value {
        if let Some(status) = lane1_runtime_status_from_report_value(plan)
            && status.state != "full"
        {
            limited_inputs.push(lane1_runtime_status_with_input_path(
                status,
                "swarm_plan_input",
                &swarm_plan.path,
            ));
        }
    } else {
        limited_inputs.push(lane1_runtime_status_limited_input(
            "swarm_plan_input",
            "swarm-plan",
            Some(&swarm_plan.path),
            "swarm_plan_input_unavailable",
            "run cargo xtask ripr-swarm plan before building a packet-complete attempt ledger",
            true,
        ));
    }
    if let Some(status) = limited_inputs
        .into_iter()
        .min_by_key(|status| lane1_runtime_status_priority(&status.state))
    {
        return status;
    }
    lane1_runtime_status_full()
}

pub(crate) fn ripr_swarm_attempt_ledger_plan_packet_index(
    swarm_plan: Option<&Value>,
) -> BTreeMap<String, Value> {
    let mut packets = BTreeMap::new();
    let Some(swarm_plan) = swarm_plan else {
        return packets;
    };
    for section in [
        "top_ready_packets",
        "top_blocked_packets",
        "top_missing_verify_or_receipt",
        "packets",
    ] {
        for packet in audit_array(swarm_plan, &[section]) {
            let canonical_gap_id = audit_non_empty_string(packet, &["canonical_gap_id"]);
            let packet_id = audit_non_empty_string(packet, &["packet_id"]);
            if let Some(id) = canonical_gap_id.as_ref() {
                packets.insert(id.clone(), packet.clone());
            }
            if let Some(id) = packet_id {
                packets.insert(id, packet.clone());
            }
        }
    }
    packets
}

pub(crate) fn ripr_swarm_attempt_ledger_current_plan_not_attempted_entries(
    generated_at: &str,
    swarm_plan: Option<&Value>,
    existing_attempts: &[RiprSwarmAttemptLedgerEntry],
) -> Vec<RiprSwarmAttemptLedgerEntry> {
    let Some(swarm_plan) = swarm_plan else {
        return Vec::new();
    };
    let mut occupied = BTreeSet::new();
    for attempt in existing_attempts {
        occupied.insert(attempt.packet_id.clone());
        occupied.insert(attempt.canonical_gap_id.clone());
    }
    let mut entries = Vec::new();
    for packet in audit_array(swarm_plan, &["top_ready_packets"]) {
        let Some(canonical_gap_id) = audit_non_empty_string(packet, &["canonical_gap_id"]) else {
            continue;
        };
        let packet_id = audit_non_empty_string(packet, &["packet_id"])
            .unwrap_or_else(|| canonical_gap_id.clone());
        if occupied.contains(&packet_id) || occupied.contains(&canonical_gap_id) {
            continue;
        }
        occupied.insert(packet_id.clone());
        occupied.insert(canonical_gap_id.clone());
        let verify_command = audit_non_empty_string(packet, &["verify_command"]);
        let reason = if verify_command.is_some() {
            "current swarm plan queued packet placeholder"
        } else {
            "current swarm plan queued packet placeholder missing verify_command"
        };
        entries.push(RiprSwarmAttemptLedgerEntry {
            packet_id,
            canonical_gap_id: canonical_gap_id.clone(),
            attempt_id: ripr_swarm_attempt_ledger_attempt_id(
                &canonical_gap_id,
                "not_attempted",
                RECEIPT_NOT_APPLICABLE,
                None,
                None,
                None,
            ),
            language: audit_non_empty_string(packet, &["language"]),
            evidence_class: audit_non_empty_string(packet, &["evidence_class"]),
            source_file: audit_non_empty_string(packet, &["source_file"]),
            repair_kind: audit_non_empty_string(packet, &["repair_kind"]),
            target_test_type: audit_non_empty_string(packet, &["target_test_type"]),
            assertion_shape: audit_non_empty_string(packet, &["assertion_shape"]),
            actor_kind: "none".to_string(),
            receipt_path: None,
            verify_command: verify_command.unwrap_or_else(|| "verify_command_unknown".to_string()),
            verify_result: None,
            receipt_command: audit_non_empty_string(packet, &["receipt_command"]),
            missing_receipt_reason: None,
            before_gap_state: None,
            after_gap_state: None,
            outcome: "not_attempted".to_string(),
            timestamp: Some(generated_at.to_string()),
            receipt_state: RECEIPT_NOT_APPLICABLE.to_string(),
            movement_source: None,
            route_quality_expectation: None,
            reason: reason.to_string(),
        });
    }
    entries
}

pub(crate) fn ripr_swarm_attempt_ledger_entries_from_value(
    ledger: &Value,
) -> Vec<RiprSwarmAttemptLedgerEntry> {
    audit_array(ledger, &["attempts"])
        .iter()
        .filter_map(|entry| {
            let canonical_gap_id = audit_non_empty_string(entry, &["canonical_gap_id"])?;
            let outcome = audit_non_empty_string(entry, &["outcome"])
                .unwrap_or_else(|| "unknown".to_string());
            Some(RiprSwarmAttemptLedgerEntry {
                packet_id: audit_non_empty_string(entry, &["packet_id"])
                    .unwrap_or_else(|| canonical_gap_id.clone()),
                canonical_gap_id,
                attempt_id: audit_non_empty_string(entry, &["attempt_id"])
                    .unwrap_or_else(|| "attempt:unknown".to_string()),
                language: audit_non_empty_string(entry, &["language"]),
                evidence_class: audit_non_empty_string(entry, &["evidence_class"]),
                source_file: audit_non_empty_string(entry, &["source_file"]),
                repair_kind: audit_non_empty_string(entry, &["repair_kind"]),
                target_test_type: audit_non_empty_string(entry, &["target_test_type"]),
                assertion_shape: audit_non_empty_string(entry, &["assertion_shape"]),
                actor_kind: audit_non_empty_string(entry, &["actor_kind"])
                    .unwrap_or_else(|| "unknown".to_string()),
                receipt_path: audit_non_empty_string(entry, &["receipt_path"]),
                verify_command: audit_non_empty_string(entry, &["verify_command"])
                    .unwrap_or_else(|| "verify_command_unknown".to_string()),
                verify_result: audit_non_empty_string(entry, &["verify_result"]),
                receipt_command: audit_non_empty_string(entry, &["receipt_command"]),
                missing_receipt_reason: audit_non_empty_string(entry, &["missing_receipt_reason"]),
                before_gap_state: audit_non_empty_string(entry, &["before_gap_state"]),
                after_gap_state: audit_non_empty_string(entry, &["after_gap_state"]),
                outcome,
                timestamp: audit_non_empty_string(entry, &["timestamp"]),
                receipt_state: audit_non_empty_string(entry, &["receipt_state"])
                    .unwrap_or_else(|| RECEIPT_NOT_APPLICABLE.to_string()),
                movement_source: audit_non_empty_string(entry, &["movement_source"]),
                route_quality_expectation: audit_non_empty_string(
                    entry,
                    &["route_quality_expectation"],
                ),
                reason: audit_non_empty_string(entry, &["reason"])
                    .unwrap_or_else(|| "prior ledger entry".to_string()),
            })
        })
        .collect()
}

pub(crate) fn ripr_swarm_attempt_ledger_entries_from_real_repair_attempts(
    generated_at: &str,
    corpus: &Value,
) -> Vec<RiprSwarmAttemptLedgerEntry> {
    if audit_non_empty_string(corpus, &["kind"]).as_deref() != Some("real_repair_attempts_corpus") {
        return Vec::new();
    }
    audit_array(corpus, &["cases"])
        .iter()
        .enumerate()
        .filter_map(|(index, case)| {
            let canonical_gap_id = audit_non_empty_string(case, &["canonical_gap_id"])?;
            let outcome =
                audit_non_empty_string(case, &["outcome"]).unwrap_or_else(|| "unknown".to_string());
            let receipt_state = audit_non_empty_string(case, &["receipt_state"])
                .unwrap_or_else(|| RECEIPT_NOT_APPLICABLE.to_string());
            let case_id =
                audit_non_empty_string(case, &["id"]).unwrap_or_else(|| canonical_gap_id.clone());
            Some(RiprSwarmAttemptLedgerEntry {
                packet_id: audit_non_empty_string(case, &["packet_id"])
                    .unwrap_or_else(|| canonical_gap_id.clone()),
                canonical_gap_id: canonical_gap_id.clone(),
                attempt_id: ripr_swarm_attempt_ledger_attempt_id(
                    &canonical_gap_id,
                    &outcome,
                    &receipt_state,
                    Some("real_repair_attempts"),
                    None,
                    Some(&case_id),
                ),
                language: audit_non_empty_string(case, &["language"]),
                evidence_class: audit_non_empty_string(case, &["evidence_class"]),
                source_file: audit_non_empty_string(case, &["source_file"]),
                repair_kind: audit_non_empty_string(case, &["repair_kind"]),
                target_test_type: audit_non_empty_string(case, &["target_test_or_observer_shape"]),
                assertion_shape: audit_non_empty_string(case, &["target_test_or_observer_shape"]),
                actor_kind: audit_non_empty_string(case, &["actor_kind"])
                    .unwrap_or_else(|| "unknown".to_string()),
                receipt_path: audit_non_empty_string(case, &["receipt_path"]),
                verify_command: audit_non_empty_string(case, &["verify_command"])
                    .unwrap_or_else(|| "verify_command_unknown".to_string()),
                verify_result: audit_non_empty_string(case, &["verify_result"]),
                receipt_command: audit_non_empty_string(case, &["receipt_command"]),
                missing_receipt_reason: audit_non_empty_string(case, &["missing_receipt_reason"]),
                before_gap_state: audit_non_empty_string(case, &["before_gap_state"]),
                after_gap_state: audit_non_empty_string(case, &["after_gap_state"]),
                outcome,
                timestamp: Some(ripr_swarm_attempt_ledger_ordered_import_timestamp(
                    generated_at,
                    index,
                )),
                receipt_state,
                movement_source: Some("real_repair_attempts".to_string()),
                route_quality_expectation: audit_non_empty_string(
                    case,
                    &["route_quality_expectation"],
                ),
                reason: audit_non_empty_string(case, &["reason"])
                    .unwrap_or_else(|| "real repair attempt corpus case".to_string()),
            })
        })
        .collect()
}

pub(crate) fn ripr_swarm_attempt_ledger_ordered_import_timestamp(
    generated_at: &str,
    index: usize,
) -> String {
    if let Some(ms) = ripr_swarm_attempt_ledger_unix_ms_timestamp(Some(generated_at)) {
        return format!("unix_ms:{}", ms.saturating_add(index as u128));
    }
    format!("{generated_at}#{index:06}")
}

pub(crate) fn ripr_swarm_attempt_ledger_prior_entries_from_value(
    ledger: &Value,
    plan_packets: &BTreeMap<String, Value>,
) -> Vec<RiprSwarmAttemptLedgerEntry> {
    ripr_swarm_attempt_ledger_entries_from_value(ledger)
        .into_iter()
        .filter(|entry| ripr_swarm_attempt_ledger_should_preserve_prior_entry(entry, plan_packets))
        .collect()
}

pub(crate) fn ripr_swarm_attempt_ledger_should_preserve_prior_entry(
    entry: &RiprSwarmAttemptLedgerEntry,
    plan_packets: &BTreeMap<String, Value>,
) -> bool {
    if entry.outcome != "not_attempted" {
        return true;
    }
    if receipt_lifecycle_state_is_present(&entry.receipt_state) {
        return true;
    }
    if entry.receipt_path.is_some() {
        return true;
    }
    if entry
        .verify_result
        .as_deref()
        .is_some_and(|result| !ripr_swarm_plan_field_missing(result))
    {
        return true;
    }
    plan_packets.contains_key(&entry.packet_id)
        || plan_packets.contains_key(&entry.canonical_gap_id)
}

pub(crate) fn ripr_swarm_attempt_ledger_entries_from_outcomes(
    generated_at: &str,
    outcomes: &Value,
    plan_packets: &BTreeMap<String, Value>,
) -> Vec<RiprSwarmAttemptLedgerEntry> {
    let receipt_path = audit_non_empty_string(outcomes, &["inputs", "agent_receipt"]);
    let targeted_test_outcome_path =
        audit_non_empty_string(outcomes, &["inputs", "targeted_test_outcome"]);
    audit_array(outcomes, &["outcomes"])
        .iter()
        .map(|outcome| {
            ripr_swarm_attempt_ledger_entry_from_outcome(
                generated_at,
                outcome,
                plan_packets,
                receipt_path.as_deref(),
                targeted_test_outcome_path.as_deref(),
            )
        })
        .collect()
}

pub(crate) fn ripr_swarm_attempt_ledger_entry_from_outcome(
    generated_at: &str,
    outcome: &Value,
    plan_packets: &BTreeMap<String, Value>,
    receipt_path: Option<&str>,
    targeted_test_outcome_path: Option<&str>,
) -> RiprSwarmAttemptLedgerEntry {
    let canonical_gap_id = audit_non_empty_string(outcome, &["canonical_gap_id"])
        .unwrap_or_else(|| "canonical_gap_id_unknown".to_string());
    let packet = plan_packets.get(&canonical_gap_id).or_else(|| {
        audit_non_empty_string(outcome, &["packet_id"]).and_then(|id| plan_packets.get(&id))
    });
    let packet_id = packet
        .and_then(|packet| audit_non_empty_string(packet, &["packet_id"]))
        .or_else(|| audit_non_empty_string(outcome, &["packet_id"]))
        .unwrap_or_else(|| canonical_gap_id.clone());
    let outcome_state = audit_non_empty_string(outcome, &["outcome_state"])
        .unwrap_or_else(|| "unknown".to_string());
    let receipt_state = audit_non_empty_string(outcome, &["receipt_state"])
        .unwrap_or_else(|| RECEIPT_NOT_APPLICABLE.to_string());
    let movement_source = audit_non_empty_string(outcome, &["movement_source"]);
    let entry_receipt_path =
        if ripr_swarm_attempt_ledger_receipt_path_applies(&receipt_state, &outcome_state) {
            receipt_path.map(str::to_string)
        } else {
            None
        };
    let receipt_command = audit_non_empty_string(outcome, &["receipt_command_or_path"])
        .or_else(|| audit_non_empty_string(outcome, &["receipt_command"]))
        .or_else(|| packet.and_then(|packet| audit_non_empty_string(packet, &["receipt_command"])))
        .or_else(|| {
            packet.and_then(|packet| audit_non_empty_string(packet, &["receipt_command_or_path"]))
        });
    let verify_command = audit_non_empty_string(outcome, &["verify_command"])
        .or_else(|| packet.and_then(|packet| audit_non_empty_string(packet, &["verify_command"])))
        .unwrap_or_else(|| "verify_command_unknown".to_string());
    let repair_kind = audit_non_empty_string(outcome, &["repair_kind"])
        .or_else(|| packet.and_then(|packet| audit_non_empty_string(packet, &["repair_kind"])));
    let evidence_class = audit_non_empty_string(outcome, &["evidence_class"])
        .or_else(|| packet.and_then(|packet| audit_non_empty_string(packet, &["evidence_class"])));
    let source_file = audit_non_empty_string(outcome, &["source_file"])
        .or_else(|| packet.and_then(|packet| audit_non_empty_string(packet, &["source_file"])));
    let target_test_type = audit_non_empty_string(outcome, &["target_test_type"]).or_else(|| {
        packet.and_then(|packet| audit_non_empty_string(packet, &["target_test_type"]))
    });
    let assertion_shape = audit_non_empty_string(outcome, &["assertion_shape"])
        .or_else(|| packet.and_then(|packet| audit_non_empty_string(packet, &["assertion_shape"])));
    let actor_kind = ripr_swarm_attempt_ledger_actor_kind(
        movement_source.as_deref(),
        &receipt_state,
        &outcome_state,
    );
    let outcome_timestamp = audit_non_empty_string(outcome, &["timestamp"]);
    let attempt_instance = audit_non_empty_string(outcome, &["attempt_instance"]).or_else(|| {
        ripr_swarm_attempt_ledger_attempt_instance(
            outcome_timestamp.as_deref(),
            entry_receipt_path.as_deref(),
            movement_source.as_deref(),
            targeted_test_outcome_path,
        )
    });

    RiprSwarmAttemptLedgerEntry {
        packet_id,
        canonical_gap_id: canonical_gap_id.clone(),
        attempt_id: ripr_swarm_attempt_ledger_attempt_id(
            &canonical_gap_id,
            &outcome_state,
            &receipt_state,
            movement_source.as_deref(),
            audit_non_empty_string(outcome, &["seam_id"]).as_deref(),
            attempt_instance.as_deref(),
        ),
        language: audit_non_empty_string(outcome, &["language"])
            .or_else(|| packet.and_then(|packet| audit_non_empty_string(packet, &["language"]))),
        evidence_class,
        source_file,
        repair_kind,
        target_test_type,
        assertion_shape,
        actor_kind,
        receipt_path: entry_receipt_path,
        verify_command,
        verify_result: audit_non_empty_string(outcome, &["verify_result"]),
        receipt_command,
        missing_receipt_reason: audit_non_empty_string(outcome, &["missing_receipt_reason"]),
        before_gap_state: audit_non_empty_string(outcome, &["before"]),
        after_gap_state: audit_non_empty_string(outcome, &["after"]),
        outcome: outcome_state,
        timestamp: outcome_timestamp.or_else(|| Some(generated_at.to_string())),
        receipt_state,
        movement_source,
        route_quality_expectation: audit_non_empty_string(outcome, &["route_quality_expectation"]),
        reason: audit_non_empty_string(outcome, &["reason"])
            .unwrap_or_else(|| "current actionable-gap outcome join".to_string()),
    }
}

pub(crate) fn ripr_swarm_attempt_ledger_receipt_path_applies(
    receipt_state: &str,
    outcome_state: &str,
) -> bool {
    receipt_lifecycle_state_is_present(receipt_state)
        || matches!(
            outcome_state,
            "receipt_present"
                | "evidence_improved"
                | "evidence_unchanged"
                | "evidence_regressed"
                | "resolved"
        )
}

pub(crate) fn ripr_swarm_attempt_ledger_actor_kind(
    movement_source: Option<&str>,
    receipt_state: &str,
    outcome_state: &str,
) -> String {
    match movement_source {
        Some("agent_receipt") => "agent".to_string(),
        Some("targeted_test_outcome") => "targeted_test_outcome".to_string(),
        Some(source) => source.to_string(),
        None if receipt_lifecycle_state_is_present(receipt_state) => "agent".to_string(),
        None if outcome_state == "not_attempted" => "none".to_string(),
        None => "unknown".to_string(),
    }
}

pub(crate) fn ripr_swarm_attempt_ledger_attempt_id(
    canonical_gap_id: &str,
    outcome_state: &str,
    receipt_state: &str,
    movement_source: Option<&str>,
    seam_id: Option<&str>,
    attempt_instance: Option<&str>,
) -> String {
    let source = movement_source.unwrap_or("no_movement_source");
    let seam = seam_id.unwrap_or("no_seam");
    let base = format!(
        "attempt:{}:{}:{}:{}:{}",
        audit_slug(canonical_gap_id),
        audit_slug(outcome_state),
        audit_slug(receipt_state),
        audit_slug(source),
        audit_slug(seam)
    );
    if let Some(instance) = attempt_instance {
        format!("{base}:{}", audit_slug(instance))
    } else {
        base
    }
}

pub(crate) fn ripr_swarm_attempt_ledger_attempt_instance(
    outcome_timestamp: Option<&str>,
    receipt_path: Option<&str>,
    movement_source: Option<&str>,
    targeted_test_outcome_path: Option<&str>,
) -> Option<String> {
    if let Some(timestamp) = outcome_timestamp {
        return Some(format!("timestamp:{timestamp}"));
    }
    if let Some(path) = receipt_path {
        return Some(format!("receipt_path:{path}"));
    }
    if movement_source == Some("targeted_test_outcome") {
        return targeted_test_outcome_path.map(|path| format!("targeted_test_outcome_path:{path}"));
    }
    None
}

pub(crate) fn ripr_swarm_attempt_ledger_dedupe_attempts(
    attempts: Vec<RiprSwarmAttemptLedgerEntry>,
) -> Vec<RiprSwarmAttemptLedgerEntry> {
    let mut by_id = BTreeMap::new();
    for attempt in attempts {
        by_id.insert(attempt.attempt_id.clone(), attempt);
    }
    by_id.into_values().collect()
}

pub(crate) fn ripr_swarm_attempt_ledger_latest_attempts(
    attempts: &[RiprSwarmAttemptLedgerEntry],
) -> Vec<RiprSwarmAttemptLedgerEntry> {
    let mut latest = BTreeMap::<String, RiprSwarmAttemptLedgerEntry>::new();
    for attempt in attempts {
        let replace = latest
            .get(&attempt.canonical_gap_id)
            .map(|current| ripr_swarm_attempt_ledger_should_replace_latest(attempt, current))
            .unwrap_or(true);
        if replace {
            latest.insert(attempt.canonical_gap_id.clone(), attempt.clone());
        }
    }
    latest.into_values().collect()
}

pub(crate) fn ripr_swarm_attempt_ledger_should_replace_latest(
    attempt: &RiprSwarmAttemptLedgerEntry,
    current: &RiprSwarmAttemptLedgerEntry,
) -> bool {
    let attempt_is_durable = attempt.outcome != "not_attempted";
    let current_is_durable = current.outcome != "not_attempted";
    if attempt_is_durable != current_is_durable {
        return attempt_is_durable;
    }
    ripr_swarm_attempt_ledger_timestamp_is_newer_or_equal(attempt, current)
}

pub(crate) fn ripr_swarm_attempt_ledger_timestamp_is_newer_or_equal(
    attempt: &RiprSwarmAttemptLedgerEntry,
    current: &RiprSwarmAttemptLedgerEntry,
) -> bool {
    match (
        ripr_swarm_attempt_ledger_unix_ms_timestamp(attempt.timestamp.as_deref()),
        ripr_swarm_attempt_ledger_unix_ms_timestamp(current.timestamp.as_deref()),
    ) {
        (Some(attempt_ms), Some(current_ms)) => attempt_ms >= current_ms,
        _ => {
            attempt.timestamp.as_deref().unwrap_or("") >= current.timestamp.as_deref().unwrap_or("")
        }
    }
}

pub(crate) fn ripr_swarm_attempt_ledger_unix_ms_timestamp(timestamp: Option<&str>) -> Option<u128> {
    timestamp?
        .strip_prefix("unix_ms:")
        .and_then(|millis| millis.parse::<u128>().ok())
}

pub(crate) fn ripr_swarm_attempt_ledger_summary(
    report: &RiprSwarmAttemptLedgerReport,
) -> RiprSwarmAttemptLedgerSummary {
    let state_counts = actionable_gap_outcome_state_counts_from_entries(&report.latest_attempts);
    RiprSwarmAttemptLedgerSummary {
        attempts_total: report.attempts.len(),
        canonical_gaps_total: report.latest_attempts.len(),
        not_attempted: state_counts.get("not_attempted").copied().unwrap_or(0),
        attempted_no_receipt: state_counts
            .get("attempted_no_receipt")
            .copied()
            .unwrap_or(0),
        receipt_present: state_counts.get("receipt_present").copied().unwrap_or(0),
        missing_verify_result: ripr_swarm_attempt_ledger_missing_verify_result_count(
            &report.latest_attempts,
        ),
        evidence_improved: state_counts.get("evidence_improved").copied().unwrap_or(0),
        evidence_unchanged: state_counts.get("evidence_unchanged").copied().unwrap_or(0),
        expected_unchanged: report
            .latest_attempts
            .iter()
            .filter(|attempt| ripr_swarm_attempt_expected_unchanged_negative_capability(attempt))
            .count(),
        evidence_regressed: state_counts.get("evidence_regressed").copied().unwrap_or(0),
        resolved: state_counts.get("resolved").copied().unwrap_or(0),
        unknown: state_counts.get("unknown").copied().unwrap_or(0),
        orphaned_receipts: report.orphaned_receipts.len(),
    }
}

pub(crate) fn ripr_swarm_attempt_ledger_history_summary(
    report: &RiprSwarmAttemptLedgerReport,
) -> RiprSwarmAttemptLedgerHistorySummary {
    ripr_swarm_attempt_ledger_history_summary_from_entries(&report.attempts)
}

pub(crate) fn ripr_swarm_attempt_ledger_history_summary_from_entries(
    attempts: &[RiprSwarmAttemptLedgerEntry],
) -> RiprSwarmAttemptLedgerHistorySummary {
    let state_counts = actionable_gap_outcome_state_counts_from_entries(attempts);
    let canonical_gaps_total = attempts
        .iter()
        .map(|attempt| attempt.canonical_gap_id.as_str())
        .collect::<BTreeSet<_>>()
        .len();
    RiprSwarmAttemptLedgerHistorySummary {
        attempts_total: attempts.len(),
        durable_attempts_total: attempts
            .iter()
            .filter(|attempt| attempt.outcome != "not_attempted")
            .count(),
        canonical_gaps_total,
        not_attempted: state_counts.get("not_attempted").copied().unwrap_or(0),
        attempted_no_receipt: state_counts
            .get("attempted_no_receipt")
            .copied()
            .unwrap_or(0),
        receipt_present: state_counts.get("receipt_present").copied().unwrap_or(0),
        missing_verify_result: ripr_swarm_attempt_ledger_missing_verify_result_count(attempts),
        evidence_improved: state_counts.get("evidence_improved").copied().unwrap_or(0),
        evidence_unchanged: state_counts.get("evidence_unchanged").copied().unwrap_or(0),
        expected_unchanged: attempts
            .iter()
            .filter(|attempt| ripr_swarm_attempt_expected_unchanged_negative_capability(attempt))
            .count(),
        evidence_regressed: state_counts.get("evidence_regressed").copied().unwrap_or(0),
        resolved: state_counts.get("resolved").copied().unwrap_or(0),
        unknown: state_counts.get("unknown").copied().unwrap_or(0),
    }
}

pub(crate) fn ripr_swarm_attempt_ledger_repair_route_quality(
    attempts: &[RiprSwarmAttemptLedgerEntry],
) -> Vec<RiprSwarmRepairRouteQualityRow> {
    ripr_swarm_attempt_ledger_repair_route_quality_grouped(attempts, false)
}

pub(crate) fn ripr_swarm_attempt_ledger_language_repair_route_quality(
    attempts: &[RiprSwarmAttemptLedgerEntry],
) -> Vec<RiprSwarmRepairRouteQualityRow> {
    ripr_swarm_attempt_ledger_repair_route_quality_grouped(attempts, true)
}

pub(crate) fn ripr_swarm_attempt_ledger_repair_route_quality_grouped(
    attempts: &[RiprSwarmAttemptLedgerEntry],
    group_by_language: bool,
) -> Vec<RiprSwarmRepairRouteQualityRow> {
    let mut rows = BTreeMap::<String, RiprSwarmRepairRouteQualityRow>::new();
    for attempt in attempts {
        let repair_kind = attempt
            .repair_kind
            .as_deref()
            .filter(|kind| !ripr_swarm_plan_field_missing(kind))
            .unwrap_or("repair_kind_unknown")
            .to_string();
        let language = if group_by_language {
            let Some(language) = attempt
                .language
                .as_deref()
                .filter(|language| !ripr_swarm_plan_field_missing(language))
            else {
                continue;
            };
            Some(language.to_string())
        } else {
            None
        };
        let key = match language.as_deref() {
            Some(language) => format!("{language}\u{1f}{repair_kind}"),
            None => repair_kind.clone(),
        };
        let row = rows
            .entry(key)
            .or_insert_with(|| RiprSwarmRepairRouteQualityRow {
                language: language.clone(),
                repair_kind,
                ..RiprSwarmRepairRouteQualityRow::default()
            });
        if attempt.outcome != "not_attempted" {
            row.attempted += 1;
        }
        if ripr_swarm_attempt_missing_verify_result(attempt) {
            row.missing_verify_result += 1;
        }
        if ripr_swarm_attempt_expected_unchanged_negative_capability(attempt) {
            row.expected_unchanged += 1;
        }
        match attempt.outcome.as_str() {
            "attempted_no_receipt" => row.attempted_no_receipt += 1,
            "receipt_present" => row.receipt_present += 1,
            "evidence_improved" => row.improved += 1,
            "evidence_unchanged" => row.unchanged += 1,
            "evidence_regressed" => row.regressed += 1,
            "resolved" => row.resolved += 1,
            "unknown" => row.unknown += 1,
            "not_attempted" => {}
            _ => row.unknown += 1,
        }
        if attempt.outcome != "not_attempted" {
            ripr_swarm_push_limited_unique(&mut row.sample_packet_ids, &attempt.packet_id);
            ripr_swarm_push_limited_unique(&mut row.sample_attempt_ids, &attempt.attempt_id);
            ripr_swarm_push_limited_unique(
                &mut row.sample_canonical_gap_ids,
                &attempt.canonical_gap_id,
            );
        }
        if ripr_swarm_repair_route_quality_attempt_is_failure(attempt)
            && let Some(reason) = attempt.missing_receipt_reason.as_deref()
        {
            ripr_swarm_push_limited_unique(&mut row.sample_missing_receipt_reasons, reason);
        }
    }
    let mut rows = rows.into_values().collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        right
            .attempted
            .cmp(&left.attempted)
            .then_with(|| right.regressed.cmp(&left.regressed))
            .then_with(|| right.unchanged.cmp(&left.unchanged))
            .then_with(|| left.language.cmp(&right.language))
            .then_with(|| left.repair_kind.cmp(&right.repair_kind))
    });
    rows
}

pub(crate) fn ripr_swarm_attempt_ledger_top_missing_evidence_fields(
    attempts: &[RiprSwarmAttemptLedgerEntry],
) -> Vec<RiprSwarmMissingEvidenceFieldRow> {
    let mut rows = BTreeMap::<String, RiprSwarmMissingEvidenceFieldRow>::new();
    for attempt in attempts {
        if attempt
            .repair_kind
            .as_deref()
            .is_none_or(ripr_swarm_plan_field_missing)
        {
            ripr_swarm_missing_evidence_field_increment(&mut rows, "repair_kind", attempt);
        }
        if ripr_swarm_plan_field_missing(&attempt.verify_command) {
            ripr_swarm_missing_evidence_field_increment(&mut rows, "verify_command", attempt);
        }
        if ripr_swarm_attempt_missing_verify_result(attempt) {
            ripr_swarm_missing_evidence_field_increment(&mut rows, "verify_result", attempt);
        }
        if attempt
            .receipt_command
            .as_deref()
            .is_none_or(ripr_swarm_plan_field_missing)
        {
            ripr_swarm_missing_evidence_field_increment(&mut rows, "receipt_command", attempt);
        }
        if attempt.outcome == "attempted_no_receipt" {
            ripr_swarm_missing_evidence_field_increment(&mut rows, "attempt_receipt", attempt);
        }
    }
    let mut rows = rows.into_values().collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.label.cmp(&right.label))
    });
    rows.truncate(LANE1_EVIDENCE_AUDIT_TOP_LIMIT);
    rows
}

pub(crate) fn ripr_swarm_missing_evidence_field_increment(
    rows: &mut BTreeMap<String, RiprSwarmMissingEvidenceFieldRow>,
    label: &str,
    attempt: &RiprSwarmAttemptLedgerEntry,
) {
    let row = rows
        .entry(label.to_string())
        .or_insert_with(|| RiprSwarmMissingEvidenceFieldRow {
            label: label.to_string(),
            ..RiprSwarmMissingEvidenceFieldRow::default()
        });
    row.count += 1;
    ripr_swarm_push_limited_unique(&mut row.sample_packet_ids, &attempt.packet_id);
    ripr_swarm_push_limited_unique(&mut row.sample_canonical_gap_ids, &attempt.canonical_gap_id);
    if let Some(repair_kind) = attempt
        .repair_kind
        .as_deref()
        .filter(|kind| !ripr_swarm_plan_field_missing(kind))
    {
        ripr_swarm_push_limited_unique(&mut row.sample_repair_kinds, repair_kind);
    }
}

pub(crate) fn ripr_swarm_push_limited_unique(values: &mut Vec<String>, value: &str) {
    if values.len() >= 3 || value.trim().is_empty() || values.iter().any(|known| known == value) {
        return;
    }
    values.push(value.to_string());
}

pub(crate) fn ripr_swarm_attempt_ledger_missing_verify_result_count(
    attempts: &[RiprSwarmAttemptLedgerEntry],
) -> usize {
    attempts
        .iter()
        .filter(|attempt| ripr_swarm_attempt_missing_verify_result(attempt))
        .count()
}

pub(crate) fn ripr_swarm_attempt_missing_verify_result(
    attempt: &RiprSwarmAttemptLedgerEntry,
) -> bool {
    attempt.outcome != "not_attempted"
        && attempt
            .verify_result
            .as_deref()
            .is_none_or(ripr_swarm_plan_field_missing)
}

pub(crate) fn ripr_swarm_repair_route_quality_attempt_is_failure(
    attempt: &RiprSwarmAttemptLedgerEntry,
) -> bool {
    if ripr_swarm_attempt_missing_verify_result(attempt) {
        return true;
    }
    if ripr_swarm_attempt_expected_unchanged_negative_capability(attempt) {
        return false;
    }
    matches!(
        attempt.outcome.as_str(),
        "attempted_no_receipt" | "evidence_unchanged" | "evidence_regressed" | "unknown"
    ) || !matches!(
        attempt.outcome.as_str(),
        "not_attempted"
            | "receipt_present"
            | "evidence_improved"
            | "evidence_unchanged"
            | "evidence_regressed"
            | "resolved"
            | "attempted_no_receipt"
            | "unknown"
    )
}

pub(crate) fn ripr_swarm_attempt_expected_unchanged_negative_capability(
    attempt: &RiprSwarmAttemptLedgerEntry,
) -> bool {
    attempt.outcome == "evidence_unchanged"
        && attempt
            .route_quality_expectation
            .as_deref()
            .is_some_and(|expectation| {
                matches!(
                    expectation,
                    "expected_unchanged" | "expected_unchanged_negative_capability"
                )
            })
}

pub(crate) fn ripr_swarm_repair_route_quality_unexpected_unchanged(
    row: &RiprSwarmRepairRouteQualityRow,
) -> usize {
    row.unchanged.saturating_sub(row.expected_unchanged)
}

pub(crate) fn ripr_swarm_repair_route_quality_success_rate(
    row: &RiprSwarmRepairRouteQualityRow,
) -> Value {
    if row.attempted == 0 {
        Value::Null
    } else {
        let successful = row.improved + row.resolved + row.expected_unchanged;
        serde_json::json!(((successful as f64 / row.attempted as f64) * 1000.0).round() / 1000.0)
    }
}

pub(crate) fn ripr_swarm_repair_route_quality_json(
    rows: &[RiprSwarmRepairRouteQualityRow],
) -> Vec<Value> {
    rows.iter()
        .map(|row| {
            serde_json::json!({
                "language": row.language,
                "repair_kind": row.repair_kind,
                "repair_kind_attempted": row.attempted,
                "repair_kind_improved": row.improved,
                "repair_kind_unchanged": row.unchanged,
                "repair_kind_regressed": row.regressed,
                "repair_kind_resolved": row.resolved,
                "repair_kind_attempted_no_receipt": row.attempted_no_receipt,
                "repair_kind_receipt_present": row.receipt_present,
                "repair_kind_missing_verify_result": row.missing_verify_result,
                "repair_kind_expected_unchanged": row.expected_unchanged,
                "repair_kind_unknown": row.unknown,
                "repair_kind_failure_count": ripr_swarm_repair_route_quality_failure_count(row),
                "repair_kind_dominant_failure_reason": ripr_swarm_repair_route_quality_dominant_failure_reason(row),
                "repair_kind_success_rate": ripr_swarm_repair_route_quality_success_rate(row),
                "sample_packet_ids": row.sample_packet_ids,
                "sample_attempt_ids": row.sample_attempt_ids,
                "sample_canonical_gap_ids": row.sample_canonical_gap_ids,
                "sample_missing_receipt_reasons": row.sample_missing_receipt_reasons,
            })
        })
        .collect()
}

pub(crate) fn ripr_swarm_top_failing_repair_routes_json(
    rows: &[RiprSwarmRepairRouteQualityRow],
) -> Vec<Value> {
    let mut rows = rows
        .iter()
        .filter(|row| ripr_swarm_repair_route_quality_failure_count(row) > 0)
        .cloned()
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        ripr_swarm_repair_route_quality_failure_count(right)
            .cmp(&ripr_swarm_repair_route_quality_failure_count(left))
            .then_with(|| right.regressed.cmp(&left.regressed))
            .then_with(|| right.missing_verify_result.cmp(&left.missing_verify_result))
            .then_with(|| {
                ripr_swarm_repair_route_quality_unexpected_unchanged(right)
                    .cmp(&ripr_swarm_repair_route_quality_unexpected_unchanged(left))
            })
            .then_with(|| right.attempted_no_receipt.cmp(&left.attempted_no_receipt))
            .then_with(|| right.unknown.cmp(&left.unknown))
            .then_with(|| left.repair_kind.cmp(&right.repair_kind))
    });
    rows.truncate(LANE1_EVIDENCE_AUDIT_TOP_LIMIT);
    ripr_swarm_repair_route_quality_json(&rows)
}

pub(crate) fn ripr_swarm_repair_route_quality_backlog_json(
    rows: &[RiprSwarmRepairRouteQualityRow],
) -> Vec<Value> {
    rows.iter()
        .filter(|row| ripr_swarm_repair_route_quality_failure_count(row) > 0)
        .map(|row| {
            let dominant_failure =
                ripr_swarm_repair_route_quality_dominant_failure_reason(row).unwrap_or("unknown");
            serde_json::json!({
                "packet_id": ripr_swarm_repair_route_quality_backlog_packet_id(row, dominant_failure),
                "repair_kind": row.repair_kind,
                "improvement_route": ripr_swarm_repair_route_quality_improvement_route(
                    row,
                    dominant_failure
                ),
                "failure_count": ripr_swarm_repair_route_quality_failure_count(row),
                "dominant_failure_reason": dominant_failure,
                "dominant_failure_count": ripr_swarm_repair_route_quality_dominant_failure_count(row),
                "sample_packet_ids": row.sample_packet_ids,
                "sample_attempt_ids": row.sample_attempt_ids,
                "sample_canonical_gap_ids": row.sample_canonical_gap_ids,
                "sample_missing_receipt_reasons": row.sample_missing_receipt_reasons,
                "why_action_required": ripr_swarm_repair_route_quality_why_action_required(
                    row,
                    dominant_failure
                ),
                "unlock_condition": ripr_swarm_repair_route_quality_unlock_condition(
                    row,
                    dominant_failure
                ),
                "non_claims": [
                    "not a public repair packet",
                    "not swarm-ready work",
                    "do not retry this repair kind from this backlog item alone",
                    "do not promote or downgrade actionability from route-quality evidence alone",
                    "do not change badge or gate semantics from route-quality evidence alone"
                ],
            })
        })
        .collect()
}

pub(crate) fn ripr_swarm_repair_route_quality_backlog_packet_id(
    row: &RiprSwarmRepairRouteQualityRow,
    dominant_failure: &str,
) -> String {
    format!(
        "route-quality:{}:{}",
        audit_slug(&row.repair_kind),
        audit_slug(dominant_failure)
    )
}

pub(crate) fn ripr_swarm_repair_route_quality_improvement_route(
    row: &RiprSwarmRepairRouteQualityRow,
    dominant_failure: &str,
) -> String {
    let repair_kind = audit_slug(&row.repair_kind);
    match dominant_failure {
        "regressed" => format!("analysis/repair-route-regression-review/{repair_kind}"),
        "missing_verify_result" => {
            format!("report/repair-route-verify-result-capture/{repair_kind}")
        }
        "unchanged" => format!("analysis/repair-route-guidance/{repair_kind}"),
        "attempted_no_receipt" if ripr_swarm_repair_route_quality_has_timeout_receipt(row) => {
            format!("report/repair-route-receipt-reliability/bounded-verify-route/{repair_kind}")
        }
        "attempted_no_receipt" => {
            format!("report/repair-route-receipt-reliability/{repair_kind}")
        }
        _ => format!("analysis/repair-route-outcome-classification/{repair_kind}"),
    }
}

pub(crate) fn ripr_swarm_repair_route_quality_has_timeout_receipt(
    row: &RiprSwarmRepairRouteQualityRow,
) -> bool {
    row.sample_missing_receipt_reasons.iter().any(|reason| {
        let normalized = reason.to_ascii_lowercase();
        normalized.contains("timed out") || normalized.contains("timeout")
    })
}

pub(crate) fn ripr_swarm_repair_route_quality_why_action_required(
    row: &RiprSwarmRepairRouteQualityRow,
    dominant_failure: &str,
) -> String {
    match dominant_failure {
        "regressed" => format!(
            "`{}` has regressed evidence in latest attempts; stop routing more packets until the guidance is narrowed",
            row.repair_kind
        ),
        "missing_verify_result" => format!(
            "`{}` attempts lack typed verify results; route quality cannot be trusted until receipts preserve pass/fail/not-run evidence",
            row.repair_kind
        ),
        "unchanged" => format!(
            "`{}` produced unchanged evidence; refine target shape, assertion guidance, or evidence expectations before increasing packet volume",
            row.repair_kind
        ),
        "attempted_no_receipt" if ripr_swarm_repair_route_quality_has_timeout_receipt(row) => {
            format!(
                "`{}` attempts timed out before receipt capture; repair guidance needs a bounded verify route before outcomes can be trusted",
                row.repair_kind
            )
        }
        "attempted_no_receipt" => format!(
            "`{}` attempts are missing receipts; repair guidance is not receiptable enough to claim outcomes",
            row.repair_kind
        ),
        _ => format!(
            "`{}` has unknown or unsupported latest attempt outcomes; classify the outcome before trusting route quality",
            row.repair_kind
        ),
    }
}

pub(crate) fn ripr_swarm_repair_route_quality_unlock_condition(
    row: &RiprSwarmRepairRouteQualityRow,
    dominant_failure: &str,
) -> String {
    match dominant_failure {
        "regressed" => format!(
            "identify why `{}` can regress evidence, add a fixture or projection guard, then require new attempts to avoid that regression",
            row.repair_kind
        ),
        "missing_verify_result" => format!(
            "preserve typed verify_result evidence for `{}` attempts in receipts or targeted-test outcomes",
            row.repair_kind
        ),
        "unchanged" => format!(
            "update `{}` guidance so a future attempt can produce evidence_improved or resolved instead of evidence_unchanged",
            row.repair_kind
        ),
        "attempted_no_receipt" if ripr_swarm_repair_route_quality_has_timeout_receipt(row) => {
            format!(
                "replace broad `{}` verify or receipt routes with a bounded command that can complete and emit a receipt inside the runtime budget",
                row.repair_kind
            )
        }
        "attempted_no_receipt" => format!(
            "make `{}` packets produce a runnable receipt command/path before treating the route as reliable",
            row.repair_kind
        ),
        _ => format!(
            "classify `{}` attempt outcomes into a known route-quality state before promoting more packets",
            row.repair_kind
        ),
    }
}

pub(crate) fn ripr_swarm_missing_evidence_fields_json(
    rows: &[RiprSwarmMissingEvidenceFieldRow],
) -> Vec<Value> {
    rows.iter()
        .map(|row| {
            serde_json::json!({
                "label": row.label,
                "count": row.count,
                "sample_packet_ids": row.sample_packet_ids,
                "sample_canonical_gap_ids": row.sample_canonical_gap_ids,
                "sample_repair_kinds": row.sample_repair_kinds,
            })
        })
        .collect()
}

pub(crate) fn ripr_swarm_attempt_ledger_json(
    report: &RiprSwarmAttemptLedgerReport,
) -> Result<String, String> {
    let summary = ripr_swarm_attempt_ledger_summary(report);
    let history_summary = ripr_swarm_attempt_ledger_history_summary(report);
    let value = serde_json::json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "report": "swarm-attempt-ledger",
        "scope": "repo",
        "status": report.status,
        "run_status": report.runtime_status.state.clone(),
        "runtime_status": lane1_runtime_status_json(&report.runtime_status),
        "generated_at": report.generated_at,
        "inputs": {
            "swarm_plan": {
                "path": report.swarm_plan_path,
                "state": report.swarm_plan_state,
                "limitation": report.swarm_plan_limitation,
            },
            "actionable_gap_outcomes": {
                "path": report.actionable_gap_outcomes_path,
                "state": report.actionable_gap_outcomes_state,
                "limitation": report.actionable_gap_outcomes_limitation,
            },
            "prior_ledger": {
                "path": report.prior_ledger_path,
                "state": report.prior_ledger_state,
                "limitation": report.prior_ledger_limitation,
            },
            "real_repair_attempts": {
                "path": report.real_repair_attempts_path,
                "state": report.real_repair_attempts_state,
                "limitation": report.real_repair_attempts_limitation,
            },
        },
        "summary": ripr_swarm_attempt_ledger_summary_json(&summary),
        "attempt_history_summary": ripr_swarm_attempt_ledger_history_summary_json(&history_summary),
        "repair_route_quality": ripr_swarm_repair_route_quality_json(&report.repair_route_quality),
        "language_repair_route_quality": ripr_swarm_repair_route_quality_json(
            &report.language_repair_route_quality
        ),
        "historical_repair_route_quality": ripr_swarm_repair_route_quality_json(
            &report.historical_repair_route_quality
        ),
        "historical_language_repair_route_quality": ripr_swarm_repair_route_quality_json(
            &report.historical_language_repair_route_quality
        ),
        "top_failing_repair_routes": ripr_swarm_top_failing_repair_routes_json(
            &report.repair_route_quality
        ),
        "top_historical_failing_repair_routes": ripr_swarm_top_failing_repair_routes_json(
            &report.historical_repair_route_quality
        ),
        "repair_route_quality_backlog": ripr_swarm_repair_route_quality_backlog_json(
            &ripr_swarm_readiness_top_failing_repair_routes(&report.repair_route_quality)
        ),
        "top_missing_evidence_fields": ripr_swarm_missing_evidence_fields_json(
            &report.top_missing_evidence_fields
        ),
        "attempts": report.attempts.iter().map(ripr_swarm_attempt_ledger_entry_json).collect::<Vec<_>>(),
        "latest_attempts": report.latest_attempts.iter().map(ripr_swarm_attempt_ledger_entry_json).collect::<Vec<_>>(),
        "orphaned_receipts": report.orphaned_receipts,
        "must_not_infer": [
            "attempt ledgers preserve existing artifact joins; they do not execute repairs",
            "not_attempted means no matching attempt artifact was supplied, not that repair failed",
            "receipt_present without movement is not evidence improvement",
            "orphaned receipts do not create new actionable gaps",
            "ledger counts do not change public badge semantics or CI gate mode"
        ],
    });
    serde_json::to_string_pretty(&value)
        .map(|mut rendered| {
            rendered.push('\n');
            rendered
        })
        .map_err(|err| format!("failed to render swarm attempt ledger JSON: {err}"))
}

pub(crate) fn ripr_swarm_attempt_ledger_summary_json(
    summary: &RiprSwarmAttemptLedgerSummary,
) -> Value {
    serde_json::json!({
        "attempts_total": summary.attempts_total,
        "canonical_gaps_total": summary.canonical_gaps_total,
        "not_attempted": summary.not_attempted,
        "attempted_no_receipt": summary.attempted_no_receipt,
        "receipt_present": summary.receipt_present,
        "missing_verify_result": summary.missing_verify_result,
        "evidence_improved": summary.evidence_improved,
        "evidence_unchanged": summary.evidence_unchanged,
        "expected_unchanged": summary.expected_unchanged,
        "evidence_regressed": summary.evidence_regressed,
        "resolved": summary.resolved,
        "unknown": summary.unknown,
        "orphaned_receipts": summary.orphaned_receipts,
    })
}

pub(crate) fn ripr_swarm_attempt_ledger_history_summary_json(
    summary: &RiprSwarmAttemptLedgerHistorySummary,
) -> Value {
    serde_json::json!({
        "attempts_total": summary.attempts_total,
        "durable_attempts_total": summary.durable_attempts_total,
        "canonical_gaps_total": summary.canonical_gaps_total,
        "not_attempted": summary.not_attempted,
        "attempted_no_receipt": summary.attempted_no_receipt,
        "receipt_present": summary.receipt_present,
        "missing_verify_result": summary.missing_verify_result,
        "evidence_improved": summary.evidence_improved,
        "evidence_unchanged": summary.evidence_unchanged,
        "expected_unchanged": summary.expected_unchanged,
        "evidence_regressed": summary.evidence_regressed,
        "resolved": summary.resolved,
        "unknown": summary.unknown,
    })
}

pub(crate) fn ripr_swarm_attempt_ledger_entry_json(entry: &RiprSwarmAttemptLedgerEntry) -> Value {
    serde_json::json!({
        "packet_id": entry.packet_id,
        "canonical_gap_id": entry.canonical_gap_id,
        "attempt_id": entry.attempt_id,
        "language": entry.language,
        "evidence_class": entry.evidence_class,
        "source_file": entry.source_file,
        "repair_kind": entry.repair_kind,
        "target_test_type": entry.target_test_type,
        "assertion_shape": entry.assertion_shape,
        "actor_kind": entry.actor_kind,
        "receipt_path": entry.receipt_path,
        "verify_command": entry.verify_command,
        "verify_result": entry.verify_result,
        "receipt_command": entry.receipt_command,
        "missing_receipt_reason": entry.missing_receipt_reason,
        "before_gap_state": entry.before_gap_state,
        "after_gap_state": entry.after_gap_state,
        "outcome": entry.outcome,
        "timestamp": entry.timestamp,
        "receipt_state": entry.receipt_state,
        "movement_source": entry.movement_source,
        "route_quality_expectation": entry.route_quality_expectation,
        "reason": entry.reason,
    })
}

pub(crate) fn ripr_swarm_attempt_ledger_markdown(report: &RiprSwarmAttemptLedgerReport) -> String {
    let summary = ripr_swarm_attempt_ledger_summary(report);
    let history_summary = ripr_swarm_attempt_ledger_history_summary(report);
    let mut out = String::new();
    out.push_str("# RIPR Swarm Attempt Ledger\n\n");
    out.push_str(&format!(
        "Run status: `{}`\n\n",
        report.runtime_status.state
    ));
    out.push_str("Durable advisory ledger over swarm packets, receipts, and evidence movement. It does not execute repairs or create receipts.\n\n");
    lane1_runtime_status_push_markdown(&mut out, &report.runtime_status);
    out.push_str("## Inputs\n\n");
    out.push_str("| Input | State | Path | Limitation |\n");
    out.push_str("| --- | --- | --- | --- |\n");
    out.push_str(&format!(
        "| swarm plan | `{}` | `{}` | {} |\n",
        audit_markdown_cell(&report.swarm_plan_state),
        audit_markdown_cell(&report.swarm_plan_path),
        audit_markdown_cell(report.swarm_plan_limitation.as_deref().unwrap_or(""))
    ));
    out.push_str(&format!(
        "| actionable gap outcomes | `{}` | `{}` | {} |\n",
        audit_markdown_cell(&report.actionable_gap_outcomes_state),
        audit_markdown_cell(&report.actionable_gap_outcomes_path),
        audit_markdown_cell(
            report
                .actionable_gap_outcomes_limitation
                .as_deref()
                .unwrap_or("")
        )
    ));
    out.push_str(&format!(
        "| prior ledger | `{}` | `{}` | {} |\n",
        audit_markdown_cell(&report.prior_ledger_state),
        audit_markdown_cell(&report.prior_ledger_path),
        audit_markdown_cell(report.prior_ledger_limitation.as_deref().unwrap_or(""))
    ));
    out.push_str(&format!(
        "| real repair attempts | `{}` | `{}` | {} |\n\n",
        audit_markdown_cell(&report.real_repair_attempts_state),
        audit_markdown_cell(&report.real_repair_attempts_path),
        audit_markdown_cell(
            report
                .real_repair_attempts_limitation
                .as_deref()
                .unwrap_or("")
        )
    ));
    out.push_str("## Summary\n\n");
    out.push_str("| Metric | Count |\n");
    out.push_str("| --- | ---: |\n");
    for (label, count) in [
        ("attempts_total", summary.attempts_total),
        ("canonical_gaps_total", summary.canonical_gaps_total),
        ("not_attempted", summary.not_attempted),
        ("attempted_no_receipt", summary.attempted_no_receipt),
        ("receipt_present", summary.receipt_present),
        ("missing_verify_result", summary.missing_verify_result),
        ("evidence_improved", summary.evidence_improved),
        ("evidence_unchanged", summary.evidence_unchanged),
        ("expected_unchanged", summary.expected_unchanged),
        ("evidence_regressed", summary.evidence_regressed),
        ("resolved", summary.resolved),
        ("unknown", summary.unknown),
        ("orphaned_receipts", summary.orphaned_receipts),
    ] {
        out.push_str(&format!("| {} | {} |\n", label.replace('_', " "), count));
    }
    out.push('\n');
    out.push_str("## Attempt History Summary\n\n");
    out.push_str("This table counts durable attempt history before the latest-attempt projection collapses repeated attempts by canonical gap. It preserves prior unchanged, no-receipt, and expected-unchanged evidence without making those older rows current route-quality failures.\n\n");
    out.push_str("| Metric | Count |\n");
    out.push_str("| --- | ---: |\n");
    for (label, count) in [
        ("attempts_total", history_summary.attempts_total),
        (
            "durable_attempts_total",
            history_summary.durable_attempts_total,
        ),
        ("canonical_gaps_total", history_summary.canonical_gaps_total),
        ("not_attempted", history_summary.not_attempted),
        ("attempted_no_receipt", history_summary.attempted_no_receipt),
        ("receipt_present", history_summary.receipt_present),
        (
            "missing_verify_result",
            history_summary.missing_verify_result,
        ),
        ("evidence_improved", history_summary.evidence_improved),
        ("evidence_unchanged", history_summary.evidence_unchanged),
        ("expected_unchanged", history_summary.expected_unchanged),
        ("evidence_regressed", history_summary.evidence_regressed),
        ("resolved", history_summary.resolved),
        ("unknown", history_summary.unknown),
    ] {
        out.push_str(&format!("| {} | {} |\n", label.replace('_', " "), count));
    }
    out.push('\n');
    out.push_str("## Repair Route Quality\n\n");
    ripr_swarm_push_repair_route_quality_table(&mut out, &report.repair_route_quality);
    out.push_str("## Repair Route Quality By Language\n\n");
    ripr_swarm_push_repair_route_quality_table(&mut out, &report.language_repair_route_quality);
    out.push_str("## Historical Repair Route Quality\n\n");
    out.push_str("Durable history rows preserve older unchanged, regressed, and no-receipt attempts after a later attempt improves or resolves the same canonical gap. They are audit evidence, not current routing state.\n\n");
    ripr_swarm_push_repair_route_quality_table(&mut out, &report.historical_repair_route_quality);
    out.push_str("## Historical Repair Route Quality By Language\n\n");
    ripr_swarm_push_repair_route_quality_table(
        &mut out,
        &report.historical_language_repair_route_quality,
    );
    out.push_str("## Historical Repair Route Quality Backlog\n\n");
    ripr_swarm_push_repair_route_quality_backlog_table(
        &mut out,
        &ripr_swarm_readiness_top_failing_repair_routes(&report.historical_repair_route_quality),
    );
    out.push_str("## Repair Route Quality Backlog\n\n");
    ripr_swarm_push_repair_route_quality_backlog_table(
        &mut out,
        &ripr_swarm_readiness_top_failing_repair_routes(&report.repair_route_quality),
    );
    if !report.top_missing_evidence_fields.is_empty() {
        out.push_str("## Top Missing Evidence Fields\n\n");
        ripr_swarm_push_missing_evidence_fields_table(
            &mut out,
            &report.top_missing_evidence_fields,
        );
    }
    out.push_str("## Latest Attempts By Canonical Gap\n\n");
    ripr_swarm_attempt_ledger_push_attempt_table(&mut out, &report.latest_attempts);
    out.push_str("## Full Attempt History\n\n");
    ripr_swarm_attempt_ledger_push_attempt_table(&mut out, &report.attempts);
    if !report.orphaned_receipts.is_empty() {
        out.push_str("## Orphaned Receipts\n\n");
        out.push_str(&format!(
            "{} orphaned receipt(s) did not match a current actionable canonical gap packet.\n\n",
            report.orphaned_receipts.len()
        ));
    }
    out.push_str("## Must Not Infer\n\n");
    out.push_str(
        "- Attempt ledgers preserve existing artifact joins; they do not execute repairs.\n",
    );
    out.push_str("- Repair-route quality is grouped from latest attempts by `repair_kind`; it is an improvement signal, not a ranking gate.\n");
    out.push_str("- Historical repair-route quality is durable audit evidence; current routing still comes from latest attempts.\n");
    out.push_str("- `not_attempted` means no matching attempt artifact was supplied, not that repair failed.\n");
    out.push_str("- `receipt_present` without movement is not evidence improvement.\n");
    out.push_str("- Orphaned receipts do not create new actionable gaps.\n");
    out.push_str("- Ledger counts do not change public badge semantics or CI gate mode.\n");
    out
}

pub(crate) fn ripr_swarm_push_repair_route_quality_table(
    out: &mut String,
    rows: &[RiprSwarmRepairRouteQualityRow],
) {
    if rows.is_empty() {
        out.push_str("No repair-route quality rows are available.\n\n");
        return;
    }
    out.push_str("| Language | Repair kind | Attempted | Improved | Unchanged | Expected unchanged | Regressed | Resolved | Failure count | Dominant failure | Success rate | Sample packets | Sample attempts | Sample gaps | Missing receipt reasons |\n");
    out.push_str(
        "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | ---: | --- | --- | --- | --- |\n",
    );
    for row in rows {
        let success_rate = match ripr_swarm_repair_route_quality_success_rate(row) {
            Value::Number(number) => number.to_string(),
            _ => "n/a".to_string(),
        };
        let failure_count = ripr_swarm_repair_route_quality_failure_count(row);
        let dominant_failure =
            ripr_swarm_repair_route_quality_dominant_failure_reason(row).unwrap_or("n/a");
        out.push_str(&format!(
            "| {} | `{}` | {} | {} | {} | {} | {} | {} | {} | `{}` | {} | {} | {} | {} | {} |\n",
            audit_markdown_cell(row.language.as_deref().unwrap_or("n/a")),
            audit_markdown_cell(&row.repair_kind),
            row.attempted,
            row.improved,
            row.unchanged,
            row.expected_unchanged,
            row.regressed,
            row.resolved,
            failure_count,
            audit_markdown_cell(dominant_failure),
            success_rate,
            audit_markdown_cell(&row.sample_packet_ids.join(", ")),
            audit_markdown_cell(&row.sample_attempt_ids.join(", ")),
            audit_markdown_cell(&row.sample_canonical_gap_ids.join(", ")),
            audit_markdown_cell(&row.sample_missing_receipt_reasons.join(", "))
        ));
    }
    out.push('\n');
}

pub(crate) fn ripr_swarm_push_repair_route_quality_backlog_table(
    out: &mut String,
    rows: &[RiprSwarmRepairRouteQualityRow],
) {
    let rows = ripr_swarm_repair_route_quality_backlog_json(rows);
    if rows.is_empty() {
        out.push_str("No repair-route quality backlog packets are available.\n\n");
        return;
    }
    out.push_str("| Packet | Repair kind | Failures | Dominant failure | Improvement route | Sample packets | Sample attempts | Sample gaps | Missing receipt reasons | Why action required | Unlock condition |\n");
    out.push_str("| --- | --- | ---: | --- | --- | --- | --- | --- | --- | --- | --- |\n");
    for row in rows {
        out.push_str(&format!(
            "| `{}` | `{}` | {} | `{}` | `{}` | {} | {} | {} | {} | {} | {} |\n",
            audit_markdown_cell(row["packet_id"].as_str().unwrap_or("")),
            audit_markdown_cell(row["repair_kind"].as_str().unwrap_or("")),
            row["failure_count"].as_u64().unwrap_or(0),
            audit_markdown_cell(row["dominant_failure_reason"].as_str().unwrap_or("")),
            audit_markdown_cell(row["improvement_route"].as_str().unwrap_or("")),
            audit_markdown_cell(
                &audit_string_array(&row, &["sample_packet_ids"])
                    .unwrap_or_default()
                    .join(", ")
            ),
            audit_markdown_cell(
                &audit_string_array(&row, &["sample_attempt_ids"])
                    .unwrap_or_default()
                    .join(", ")
            ),
            audit_markdown_cell(
                &audit_string_array(&row, &["sample_canonical_gap_ids"])
                    .unwrap_or_default()
                    .join(", ")
            ),
            audit_markdown_cell(
                &audit_string_array(&row, &["sample_missing_receipt_reasons"])
                    .unwrap_or_default()
                    .join(", ")
            ),
            audit_markdown_cell(row["why_action_required"].as_str().unwrap_or("")),
            audit_markdown_cell(row["unlock_condition"].as_str().unwrap_or(""))
        ));
    }
    out.push('\n');
}

pub(crate) fn ripr_swarm_push_missing_evidence_fields_table(
    out: &mut String,
    rows: &[RiprSwarmMissingEvidenceFieldRow],
) {
    if rows.is_empty() {
        out.push_str("No missing evidence field rows are available.\n\n");
        return;
    }
    out.push_str("| Field | Count | Sample packets | Sample gaps | Repair kinds |\n");
    out.push_str("| --- | ---: | --- | --- | --- |\n");
    for row in rows {
        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {} |\n",
            audit_markdown_cell(&row.label),
            row.count,
            audit_markdown_cell(&row.sample_packet_ids.join(", ")),
            audit_markdown_cell(&row.sample_canonical_gap_ids.join(", ")),
            audit_markdown_cell(&row.sample_repair_kinds.join(", "))
        ));
    }
    out.push('\n');
}

pub(crate) fn ripr_swarm_attempt_ledger_push_attempt_table(
    out: &mut String,
    attempts: &[RiprSwarmAttemptLedgerEntry],
) {
    if attempts.is_empty() {
        out.push_str("No attempts in this section.\n\n");
        return;
    }
    out.push_str("| Attempt | Gap | Packet | Repair | Outcome | Actor | Verify | Verify result | Receipt | Missing receipt reason |\n");
    out.push_str("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n");
    for attempt in attempts {
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | {} | `{}` | {} | {} |\n",
            audit_markdown_cell(&attempt.attempt_id),
            audit_markdown_cell(&attempt.canonical_gap_id),
            audit_markdown_cell(&attempt.packet_id),
            audit_markdown_cell(attempt.repair_kind.as_deref().unwrap_or("unknown")),
            audit_markdown_cell(&attempt.outcome),
            audit_markdown_cell(&attempt.actor_kind),
            audit_markdown_cell(&attempt.verify_command),
            audit_markdown_cell(attempt.verify_result.as_deref().unwrap_or("unknown")),
            audit_markdown_cell(attempt.receipt_command.as_deref().unwrap_or("missing")),
            audit_markdown_cell(attempt.missing_receipt_reason.as_deref().unwrap_or(""))
        ));
    }
    out.push('\n');
}

pub(crate) fn ripr_swarm_readiness_report(args: &RiprSwarmReadinessArgs) -> Result<(), String> {
    let (swarm_plan_state, swarm_plan_limitation, swarm_plan) =
        ripr_swarm_read_optional_json(&args.swarm_plan_path);
    let (
        actionable_gap_outcomes_state,
        actionable_gap_outcomes_limitation,
        actionable_gap_outcomes,
    ) = ripr_swarm_read_optional_json(&args.actionable_gap_outcomes_path);
    let (attempt_ledger_state, attempt_ledger_limitation, attempt_ledger) =
        ripr_swarm_read_optional_json(&args.attempt_ledger_path);
    let report = ripr_swarm_readiness_from_values(
        RiprSwarmReadinessInput {
            path: normalize_path(&args.swarm_plan_path),
            state: swarm_plan_state,
            limitation: swarm_plan_limitation,
            value: swarm_plan.as_ref(),
        },
        RiprSwarmReadinessInput {
            path: normalize_path(&args.actionable_gap_outcomes_path),
            state: actionable_gap_outcomes_state,
            limitation: actionable_gap_outcomes_limitation,
            value: actionable_gap_outcomes.as_ref(),
        },
        RiprSwarmReadinessInput {
            path: normalize_path(&args.attempt_ledger_path),
            state: attempt_ledger_state,
            limitation: attempt_ledger_limitation,
            value: attempt_ledger.as_ref(),
        },
    );

    write_report("swarm-readiness.json", &ripr_swarm_readiness_json(&report)?)?;
    write_report(
        "swarm-readiness.md",
        &ripr_swarm_readiness_markdown(&report),
    )
}

pub(crate) fn ripr_swarm_read_optional_json(
    path: &Path,
) -> (String, Option<String>, Option<Value>) {
    match fs::read_to_string(path) {
        Ok(text) => match serde_json::from_str::<Value>(&text) {
            Ok(value) => ("read".to_string(), None, Some(value)),
            Err(err) => (
                "malformed".to_string(),
                Some(format!("failed to parse {}: {err}", normalize_path(path))),
                None,
            ),
        },
        Err(err) => (
            "missing".to_string(),
            Some(format!("failed to read {}: {err}", normalize_path(path))),
            None,
        ),
    }
}

pub(crate) fn ripr_swarm_readiness_from_values(
    swarm_plan: RiprSwarmReadinessInput<'_>,
    actionable_gap_outcomes: RiprSwarmReadinessInput<'_>,
    attempt_ledger: RiprSwarmReadinessInput<'_>,
) -> RiprSwarmReadinessReport {
    let summary = ripr_swarm_readiness_summary(
        swarm_plan.value,
        actionable_gap_outcomes.value,
        attempt_ledger.value,
    );
    let attempt_history_summary =
        ripr_swarm_readiness_attempt_history_summary(attempt_ledger.value);
    let repair_route_quality = attempt_ledger
        .value
        .map(ripr_swarm_readiness_repair_route_quality)
        .unwrap_or_default();
    let language_repair_route_quality = attempt_ledger
        .value
        .map(ripr_swarm_readiness_language_repair_route_quality)
        .unwrap_or_default();
    let top_failing_repair_routes =
        ripr_swarm_readiness_top_failing_repair_routes(&repair_route_quality);
    let top_missing_evidence_fields = attempt_ledger
        .value
        .map(ripr_swarm_readiness_top_missing_evidence_fields)
        .unwrap_or_default();
    let static_limitation_backlog = swarm_plan
        .value
        .and_then(|plan| audit_get(plan, &["static_limitation_backlog"]))
        .cloned()
        .unwrap_or(Value::Null);
    let top_limitation_routes =
        ripr_swarm_readiness_top_limitation_routes(&static_limitation_backlog);
    let blocked_state_routes = ripr_swarm_readiness_blocked_state_routes(
        &summary,
        swarm_plan.value,
        attempt_ledger.value,
        &top_limitation_routes,
    );
    let runtime_status =
        ripr_swarm_readiness_runtime_status(&swarm_plan, &actionable_gap_outcomes, &attempt_ledger);
    let next_action_sources = RiprSwarmReadinessNextActionSources {
        swarm_plan: swarm_plan.value,
        top_failing_repair_routes: &top_failing_repair_routes,
        top_missing_evidence_fields: &top_missing_evidence_fields,
        top_limitation_routes: &top_limitation_routes,
        static_limitation_backlog: &static_limitation_backlog,
    };
    let next_actions = ripr_swarm_readiness_next_actions(
        &summary,
        next_action_sources,
        [&swarm_plan, &actionable_gap_outcomes, &attempt_ledger],
        &runtime_status,
    );
    let status = if swarm_plan.value.is_some() && runtime_status.downstream_consumable {
        "advisory"
    } else {
        "blocked"
    }
    .to_string();
    let readiness_state = ripr_swarm_readiness_state(&status, &runtime_status).to_string();

    RiprSwarmReadinessReport {
        status,
        readiness_state,
        runtime_status,
        swarm_plan_path: swarm_plan.path,
        swarm_plan_state: swarm_plan.state,
        swarm_plan_limitation: swarm_plan.limitation,
        actionable_gap_outcomes_path: actionable_gap_outcomes.path,
        actionable_gap_outcomes_state: actionable_gap_outcomes.state,
        actionable_gap_outcomes_limitation: actionable_gap_outcomes.limitation,
        attempt_ledger_path: attempt_ledger.path,
        attempt_ledger_state: attempt_ledger.state,
        attempt_ledger_limitation: attempt_ledger.limitation,
        summary,
        attempt_history_summary,
        static_limitation_backlog,
        top_limitation_routes,
        blocked_state_routes,
        repair_route_quality,
        language_repair_route_quality,
        cross_language_oracle_route_quality: cross_language_oracle_route_quality_report_value(),
        top_failing_repair_routes,
        top_missing_evidence_fields,
        next_actions,
    }
}

pub(crate) fn ripr_swarm_readiness_state(
    status: &str,
    runtime_status: &Lane1RuntimeStatus,
) -> &'static str {
    match runtime_status.state.as_str() {
        "full" => "full",
        "limited_stale_input" => "stale",
        _ if ripr_swarm_readiness_runtime_input_is_blocked(runtime_status) => "blocked",
        _ if runtime_status.state.starts_with("limited_") => "limited",
        _ if status == "blocked" => "blocked",
        _ => "limited",
    }
}

pub(crate) fn ripr_swarm_readiness_runtime_input_is_blocked(
    runtime_status: &Lane1RuntimeStatus,
) -> bool {
    matches!(
        runtime_status.limitation_category.as_deref(),
        Some(
            "swarm_plan_input_unavailable"
                | "actionable_gap_outcomes_input_unavailable"
                | "swarm_attempt_ledger_input_unavailable"
        )
    )
}

pub(crate) fn ripr_swarm_readiness_runtime_status(
    swarm_plan: &RiprSwarmReadinessInput<'_>,
    actionable_gap_outcomes: &RiprSwarmReadinessInput<'_>,
    attempt_ledger: &RiprSwarmReadinessInput<'_>,
) -> Lane1RuntimeStatus {
    let mut limited_inputs = Vec::new();
    if let Some(plan) = swarm_plan.value {
        if let Some(status) = lane1_runtime_status_from_report_value(plan)
            && status.state != "full"
        {
            limited_inputs.push(lane1_runtime_status_with_input_path(
                status,
                "swarm_plan_input",
                &swarm_plan.path,
            ));
        }
    } else {
        limited_inputs.push(lane1_runtime_status_limited_input(
            "swarm_plan_input",
            "swarm-plan",
            Some(&swarm_plan.path),
            "swarm_plan_input_unavailable",
            "run cargo xtask ripr-swarm plan before readiness",
            false,
        ));
    }

    if let Some(outcomes) = actionable_gap_outcomes.value {
        if let Some(status) = lane1_runtime_status_from_report_value(outcomes)
            && status.state != "full"
        {
            limited_inputs.push(lane1_runtime_status_with_input_path(
                status,
                "actionable_gap_outcomes_input",
                &actionable_gap_outcomes.path,
            ));
        }
    } else {
        limited_inputs.push(lane1_runtime_status_limited_input(
            "actionable_gap_outcomes_input",
            "actionable-gap-outcomes",
            Some(&actionable_gap_outcomes.path),
            "actionable_gap_outcomes_input_unavailable",
            "run cargo xtask actionable-gap-outcomes before claiming attempt outcomes",
            false,
        ));
    }

    if let Some(ledger) = attempt_ledger.value {
        if let Some(status) = lane1_runtime_status_from_report_value(ledger)
            && status.state != "full"
        {
            limited_inputs.push(lane1_runtime_status_with_input_path(
                status,
                "swarm_attempt_ledger_input",
                &attempt_ledger.path,
            ));
        }
    } else {
        limited_inputs.push(lane1_runtime_status_limited_input(
            "swarm_attempt_ledger_input",
            "swarm-attempt-ledger",
            Some(&attempt_ledger.path),
            "swarm_attempt_ledger_input_unavailable",
            "run cargo xtask ripr-swarm attempt-ledger before claiming durable attempt history",
            false,
        ));
    }

    if let Some(status) = limited_inputs
        .into_iter()
        .min_by_key(|status| lane1_runtime_status_priority(&status.state))
    {
        return status;
    }

    lane1_runtime_status_full()
}

pub(crate) fn ripr_swarm_readiness_summary(
    swarm_plan: Option<&Value>,
    actionable_gap_outcomes: Option<&Value>,
    attempt_ledger: Option<&Value>,
) -> RiprSwarmReadinessSummary {
    let mut summary = RiprSwarmReadinessSummary::default();
    if let Some(plan) = swarm_plan {
        summary.actionable_gaps_total = audit_usize(plan, &["source_summary", "actionable_gaps"])
            .or_else(|| audit_usize(plan, &["source_summary", "actionable_gaps_total"]))
            .unwrap_or_default();
        summary.public_projection_eligible_packets = audit_usize(
            plan,
            &["source_summary", "public_projection_eligible_packets"],
        )
        .unwrap_or_default();
        summary.swarm_ready_packets =
            audit_usize(plan, &["summary", "swarm_ready_packets"]).unwrap_or_default();
        summary.blocked_packets =
            audit_usize(plan, &["summary", "blocked_packets"]).unwrap_or_default();
        summary.blocked_by_missing_context_packets =
            audit_usize(plan, &["summary", "blocked_by_missing_context_packets"])
                .unwrap_or_default();
        summary.blocked_by_static_limitation_packets =
            audit_usize(plan, &["summary", "blocked_by_static_limitation_packets"])
                .unwrap_or_default();
        summary.blocked_by_public_projection_exclusion_packets = audit_usize(
            plan,
            &["summary", "blocked_by_public_projection_exclusion_packets"],
        )
        .or_else(|| audit_usize(plan, &["summary", "public_projection_excluded_packets"]))
        .unwrap_or_default();
        summary.blocked_by_operator_judgment_packets =
            audit_usize(plan, &["summary", "blocked_by_operator_judgment_packets"])
                .unwrap_or_default();
        summary.public_projection_excluded_packets =
            audit_usize(plan, &["summary", "public_projection_excluded_packets"])
                .unwrap_or_default();
        summary.public_projection_exclusion_reasons =
            audit_count_rows_map(plan, &["summary", "public_projection_exclusion_reasons"]);
        summary.missing_canonical_gap_id =
            audit_usize(plan, &["summary", "missing_canonical_gap_id"])
                .unwrap_or_default()
                .max(ripr_swarm_readiness_plan_packet_field_blocker_count(
                    plan,
                    "canonical_gap_id",
                    &["missing_canonical_gap_id"],
                ));
        summary.not_actionable_gap_state =
            audit_usize(plan, &["summary", "not_actionable_gap_state"])
                .unwrap_or_default()
                .max(ripr_swarm_readiness_plan_packet_field_blocker_count(
                    plan,
                    "actionable_gap_state",
                    &["not_actionable_gap_state"],
                ));
        summary.missing_verify_command = audit_usize(plan, &["summary", "missing_verify_command"])
            .unwrap_or_default()
            .max(ripr_swarm_readiness_plan_packet_field_blocker_count(
                plan,
                "verify_command",
                &["missing_verify_command", "unbounded_verify_command"],
            ));
        summary.missing_receipt_command =
            audit_usize(plan, &["summary", "missing_receipt_command"])
                .unwrap_or_default()
                .max(ripr_swarm_readiness_plan_packet_field_blocker_count(
                    plan,
                    "receipt_command",
                    &["missing_receipt_command"],
                ));
        summary.missing_repair_kind = audit_usize(plan, &["summary", "missing_repair_kind"])
            .unwrap_or_default()
            .max(ripr_swarm_readiness_plan_packet_field_blocker_count(
                plan,
                "repair_kind",
                &["missing_repair_kind"],
            ));
        summary.missing_repair_route = audit_usize(plan, &["summary", "missing_repair_route"])
            .unwrap_or_default()
            .max(ripr_swarm_readiness_plan_packet_field_blocker_count(
                plan,
                "repair_route",
                &["missing_repair_route"],
            ));
        summary.missing_target_test_shape =
            audit_usize(plan, &["summary", "missing_target_test_shape"])
                .unwrap_or_default()
                .max(ripr_swarm_readiness_plan_packet_field_blocker_count(
                    plan,
                    "target_test_shape",
                    &["missing_target_test_shape"],
                ));
        summary.missing_must_not_change =
            audit_usize(plan, &["summary", "missing_must_not_change"])
                .unwrap_or_default()
                .max(ripr_swarm_readiness_plan_packet_field_blocker_count(
                    plan,
                    "must_not_change",
                    &["missing_must_not_change"],
                ));
        summary.missing_allowed_edit_surface =
            audit_usize(plan, &["summary", "missing_allowed_edit_surface"])
                .unwrap_or_default()
                .max(ripr_swarm_readiness_plan_packet_field_blocker_count(
                    plan,
                    "allowed_edit_surface",
                    &["missing_allowed_edit_surface"],
                ));
        summary.missing_confidence = audit_usize(plan, &["summary", "missing_confidence"])
            .unwrap_or_default()
            .max(ripr_swarm_readiness_plan_packet_field_blocker_count(
                plan,
                "confidence_basis",
                &["missing_confidence"],
            ));
        summary.missing_raw_evidence_refs =
            audit_usize(plan, &["summary", "missing_raw_evidence_refs"])
                .unwrap_or_default()
                .max(ripr_swarm_readiness_plan_packet_field_blocker_count(
                    plan,
                    "raw_evidence_refs",
                    &["missing_raw_evidence_refs"],
                ));
        let related_context_missing =
            audit_usize(plan, &["summary", "missing_related_test_or_observer"])
                .unwrap_or_else(|| {
                    audit_usize(plan, &["summary", "related_context_missing"]).unwrap_or_default()
                })
                .max(ripr_swarm_readiness_plan_packet_field_blocker_count(
                    plan,
                    "related_test_or_observer",
                    &["missing_related_test_or_observer"],
                ));
        summary.missing_related_test_or_observer = related_context_missing;
        summary.related_context_missing = related_context_missing;
        summary.static_limitation_packets =
            audit_usize(plan, &["summary", "static_limitation_packets"]).unwrap_or_default();
        let static_limitation_backlog = audit_get(plan, &["static_limitation_backlog"]);
        summary.static_limitation_backlog_packets =
            audit_usize(plan, &["summary", "static_limitation_backlog_packets"])
                .or_else(|| {
                    static_limitation_backlog.map(ripr_swarm_static_limitation_backlog_packet_count)
                })
                .unwrap_or_default();
        summary.static_limitation_backlog_signals =
            audit_usize(plan, &["summary", "static_limitation_backlog_signals"])
                .or_else(|| {
                    static_limitation_backlog.map(ripr_swarm_static_limitation_backlog_signal_count)
                })
                .unwrap_or_default();
        summary.high_confidence_packets =
            audit_usize(plan, &["summary", "high_confidence_packets"]).unwrap_or_default();
    }
    if let Some(ledger) = attempt_ledger {
        let attempts = ripr_swarm_attempt_ledger_entries_from_value(ledger);
        if attempts.is_empty() {
            let attempts_total =
                audit_usize(ledger, &["summary", "attempts_total"]).unwrap_or_default();
            let not_attempted =
                audit_usize(ledger, &["summary", "not_attempted"]).unwrap_or_default();
            summary.attempted_packets = attempts_total.saturating_sub(not_attempted);
            summary.missing_verify_result =
                audit_usize(ledger, &["summary", "missing_verify_result"])
                    .or_else(|| {
                        ripr_swarm_top_missing_evidence_field_count(ledger, "verify_result")
                    })
                    .unwrap_or_default();
            summary.attempted_no_receipt_packets =
                audit_usize(ledger, &["summary", "attempted_no_receipt"]).unwrap_or_default();
            summary.receipt_present_packets =
                audit_usize(ledger, &["summary", "receipt_present"]).unwrap_or_default();
            summary.improved_packets =
                audit_usize(ledger, &["summary", "evidence_improved"]).unwrap_or_default();
            summary.unchanged_packets =
                audit_usize(ledger, &["summary", "evidence_unchanged"]).unwrap_or_default();
            summary.expected_unchanged_packets =
                audit_usize(ledger, &["summary", "expected_unchanged"]).unwrap_or_default();
            summary.regressed_packets =
                audit_usize(ledger, &["summary", "evidence_regressed"]).unwrap_or_default();
            summary.resolved_packets =
                audit_usize(ledger, &["summary", "resolved"]).unwrap_or_default();
            summary.orphaned_receipts =
                audit_usize(ledger, &["summary", "orphaned_receipts"]).unwrap_or_default();
        } else {
            let latest_attempts = ripr_swarm_attempt_ledger_latest_attempts(&attempts);
            let state_counts = actionable_gap_outcome_state_counts_from_entries(&latest_attempts);
            let not_attempted = state_counts.get("not_attempted").copied().unwrap_or(0);
            summary.attempted_packets = latest_attempts.len().saturating_sub(not_attempted);
            summary.missing_verify_result =
                ripr_swarm_attempt_ledger_missing_verify_result_count(&latest_attempts);
            summary.attempted_no_receipt_packets = state_counts
                .get("attempted_no_receipt")
                .copied()
                .unwrap_or_default();
            summary.receipt_present_packets = state_counts
                .get("receipt_present")
                .copied()
                .unwrap_or_default();
            summary.improved_packets = state_counts
                .get("evidence_improved")
                .copied()
                .unwrap_or_default();
            summary.unchanged_packets = state_counts
                .get("evidence_unchanged")
                .copied()
                .unwrap_or_default();
            summary.expected_unchanged_packets = latest_attempts
                .iter()
                .filter(|attempt| {
                    ripr_swarm_attempt_expected_unchanged_negative_capability(attempt)
                })
                .count();
            summary.regressed_packets = state_counts
                .get("evidence_regressed")
                .copied()
                .unwrap_or_default();
            summary.resolved_packets = state_counts.get("resolved").copied().unwrap_or_default();
            let orphaned_receipts = audit_array(ledger, &["orphaned_receipts"]).len();
            summary.orphaned_receipts = if orphaned_receipts > 0 {
                orphaned_receipts
            } else {
                audit_usize(ledger, &["summary", "orphaned_receipts"]).unwrap_or_default()
            };
        }
    } else if let Some(outcomes) = actionable_gap_outcomes {
        let outcomes_total =
            audit_usize(outcomes, &["summary", "outcomes_total"]).unwrap_or_default();
        let not_attempted =
            audit_usize(outcomes, &["summary", "not_attempted"]).unwrap_or_default();
        summary.attempted_packets = outcomes_total.saturating_sub(not_attempted);
        summary.missing_verify_result =
            actionable_gap_outcomes_missing_verify_result_count(outcomes);
        summary.attempted_no_receipt_packets =
            audit_usize(outcomes, &["summary", "attempted_no_receipt"]).unwrap_or_default();
        summary.receipt_present_packets =
            audit_usize(outcomes, &["summary", "receipt_present"]).unwrap_or_default();
        summary.improved_packets =
            audit_usize(outcomes, &["summary", "evidence_improved"]).unwrap_or_default();
        summary.unchanged_packets =
            audit_usize(outcomes, &["summary", "evidence_unchanged"]).unwrap_or_default();
        summary.regressed_packets =
            audit_usize(outcomes, &["summary", "evidence_regressed"]).unwrap_or_default();
        summary.resolved_packets =
            audit_usize(outcomes, &["summary", "resolved"]).unwrap_or_default();
        summary.orphaned_receipts =
            audit_usize(outcomes, &["summary", "orphaned_receipts"]).unwrap_or_default();
    }
    summary
}

pub(crate) fn ripr_swarm_readiness_plan_packet_field_blocker_count(
    plan: &Value,
    missing_context_field: &str,
    projection_exclusion_reasons: &[&str],
) -> usize {
    let blocked = audit_array(plan, &["top_blocked_packets"]);
    let ready = audit_array(plan, &["top_ready_packets"]);
    blocked
        .iter()
        .chain(ready.iter())
        .filter(|packet| {
            ripr_swarm_readiness_packet_missing_context(packet, missing_context_field)
                || projection_exclusion_reasons
                    .iter()
                    .any(|reason| ripr_swarm_readiness_packet_projection_exclusion(packet, reason))
        })
        .count()
}

pub(crate) fn ripr_swarm_readiness_attempt_history_summary(
    attempt_ledger: Option<&Value>,
) -> RiprSwarmAttemptLedgerHistorySummary {
    let Some(ledger) = attempt_ledger else {
        return RiprSwarmAttemptLedgerHistorySummary::default();
    };
    if let Some(summary) = audit_get(ledger, &["attempt_history_summary"]) {
        return ripr_swarm_attempt_ledger_history_summary_from_json(summary);
    }
    let attempts = ripr_swarm_attempt_ledger_entries_from_value(ledger);
    if attempts.is_empty() {
        return RiprSwarmAttemptLedgerHistorySummary::default();
    }
    ripr_swarm_attempt_ledger_history_summary_from_entries(&attempts)
}

pub(crate) fn ripr_swarm_attempt_ledger_history_summary_from_json(
    value: &Value,
) -> RiprSwarmAttemptLedgerHistorySummary {
    RiprSwarmAttemptLedgerHistorySummary {
        attempts_total: audit_usize(value, &["attempts_total"]).unwrap_or_default(),
        durable_attempts_total: audit_usize(value, &["durable_attempts_total"]).unwrap_or_default(),
        canonical_gaps_total: audit_usize(value, &["canonical_gaps_total"]).unwrap_or_default(),
        not_attempted: audit_usize(value, &["not_attempted"]).unwrap_or_default(),
        attempted_no_receipt: audit_usize(value, &["attempted_no_receipt"]).unwrap_or_default(),
        receipt_present: audit_usize(value, &["receipt_present"]).unwrap_or_default(),
        missing_verify_result: audit_usize(value, &["missing_verify_result"]).unwrap_or_default(),
        evidence_improved: audit_usize(value, &["evidence_improved"]).unwrap_or_default(),
        evidence_unchanged: audit_usize(value, &["evidence_unchanged"]).unwrap_or_default(),
        expected_unchanged: audit_usize(value, &["expected_unchanged"]).unwrap_or_default(),
        evidence_regressed: audit_usize(value, &["evidence_regressed"]).unwrap_or_default(),
        resolved: audit_usize(value, &["resolved"]).unwrap_or_default(),
        unknown: audit_usize(value, &["unknown"]).unwrap_or_default(),
    }
}

pub(crate) fn ripr_swarm_readiness_json(
    report: &RiprSwarmReadinessReport,
) -> Result<String, String> {
    let value = serde_json::json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "report": "swarm-readiness",
        "scope": "repo",
        "status": report.status,
        "readiness_state": report.readiness_state,
        "run_status": report.runtime_status.state.clone(),
        "runtime_status": lane1_runtime_status_json(&report.runtime_status),
        "inputs": {
            "swarm_plan": {
                "path": report.swarm_plan_path,
                "state": report.swarm_plan_state,
                "limitation": report.swarm_plan_limitation,
            },
            "actionable_gap_outcomes": {
                "path": report.actionable_gap_outcomes_path,
                "state": report.actionable_gap_outcomes_state,
                "limitation": report.actionable_gap_outcomes_limitation,
            },
            "attempt_ledger": {
                "path": report.attempt_ledger_path,
                "state": report.attempt_ledger_state,
                "limitation": report.attempt_ledger_limitation,
            },
        },
        "summary": ripr_swarm_readiness_summary_json(&report.summary),
        "attempt_history_summary": ripr_swarm_attempt_ledger_history_summary_json(
            &report.attempt_history_summary
        ),
        "static_limitation_backlog": report.static_limitation_backlog,
        "top_limitation_routes": ripr_swarm_limitation_routes_json(
            &report.top_limitation_routes
        ),
        "blocked_state_routes": ripr_swarm_readiness_blocked_state_routes_json(
            &report.blocked_state_routes
        ),
        "repair_route_quality": ripr_swarm_repair_route_quality_json(&report.repair_route_quality),
        "language_repair_route_quality": ripr_swarm_repair_route_quality_json(
            &report.language_repair_route_quality
        ),
        "cross_language_oracle_route_quality": report.cross_language_oracle_route_quality,
        "limitation_route_quality": ripr_swarm_limitation_route_quality_json(
            &report.top_limitation_routes
        ),
        "top_failing_repair_routes": ripr_swarm_repair_route_quality_json(
            &report.top_failing_repair_routes
        ),
        "repair_route_quality_backlog": ripr_swarm_repair_route_quality_backlog_json(
            &report.top_failing_repair_routes
        ),
        "top_missing_evidence_fields": ripr_swarm_missing_evidence_fields_json(
            &report.top_missing_evidence_fields
        ),
        "top_next_action": report
            .next_actions
            .first()
            .map(ripr_swarm_readiness_next_action_json),
        "next_actions": ripr_swarm_readiness_next_actions_json(&report.next_actions),
        "must_not_infer": [
            "readiness reports summarize existing swarm artifacts; they do not execute repairs",
            "raw findings remain supporting evidence, not swarm work",
            "missing outcome artifacts mean no outcome join is available, not that attempts failed",
            "repair-route quality is an analyzer improvement signal, not a public badge basis",
            "top_next_action is a projection of next_actions[0], not a separate ranking source",
            "readiness counts do not change public badge semantics",
            "static limitations and blocked packets are not repair-ready work"
        ],
    });
    serde_json::to_string_pretty(&value)
        .map(|mut rendered| {
            rendered.push('\n');
            rendered
        })
        .map_err(|err| format!("failed to render swarm readiness JSON: {err}"))
}

pub(crate) fn ripr_swarm_readiness_summary_json(summary: &RiprSwarmReadinessSummary) -> Value {
    serde_json::json!({
        "actionable_gaps_total": summary.actionable_gaps_total,
        "public_projection_eligible_packets": summary.public_projection_eligible_packets,
        "swarm_ready_packets": summary.swarm_ready_packets,
        "blocked_packets": summary.blocked_packets,
        "blocked_by_missing_context_packets": summary.blocked_by_missing_context_packets,
        "blocked_by_static_limitation_packets": summary.blocked_by_static_limitation_packets,
        "blocked_by_public_projection_exclusion_packets": summary
            .blocked_by_public_projection_exclusion_packets,
        "blocked_by_operator_judgment_packets": summary.blocked_by_operator_judgment_packets,
        "public_projection_excluded_packets": summary.public_projection_excluded_packets,
        "public_projection_exclusion_reasons": audit_count_rows_json(
            &summary.public_projection_exclusion_reasons
        ),
        "missing_canonical_gap_id": summary.missing_canonical_gap_id,
        "not_actionable_gap_state": summary.not_actionable_gap_state,
        "missing_verify_command": summary.missing_verify_command,
        "missing_verify_result": summary.missing_verify_result,
        "missing_receipt_command": summary.missing_receipt_command,
        "missing_repair_kind": summary.missing_repair_kind,
        "missing_repair_route": summary.missing_repair_route,
        "missing_target_test_shape": summary.missing_target_test_shape,
        "missing_must_not_change": summary.missing_must_not_change,
        "missing_allowed_edit_surface": summary.missing_allowed_edit_surface,
        "missing_confidence": summary.missing_confidence,
        "missing_raw_evidence_refs": summary.missing_raw_evidence_refs,
        "missing_related_test_or_observer": summary.missing_related_test_or_observer,
        "related_context_missing": summary.related_context_missing,
        "static_limitation_packets": summary.static_limitation_packets,
        "static_limitation_backlog_packets": summary.static_limitation_backlog_packets,
        "static_limitation_backlog_signals": summary.static_limitation_backlog_signals,
        "high_confidence_packets": summary.high_confidence_packets,
        "attempted_packets": summary.attempted_packets,
        "attempted_no_receipt_packets": summary.attempted_no_receipt_packets,
        "receipt_present_packets": summary.receipt_present_packets,
        "improved_packets": summary.improved_packets,
        "unchanged_packets": summary.unchanged_packets,
        "expected_unchanged_packets": summary.expected_unchanged_packets,
        "regressed_packets": summary.regressed_packets,
        "resolved_packets": summary.resolved_packets,
        "orphaned_receipts": summary.orphaned_receipts,
    })
}

pub(crate) fn ripr_swarm_readiness_blocked_state_routes(
    summary: &RiprSwarmReadinessSummary,
    swarm_plan: Option<&Value>,
    attempt_ledger: Option<&Value>,
    top_limitation_routes: &[RiprSwarmLimitationRouteRow],
) -> Vec<RiprSwarmReadinessBlockedStateRoute> {
    let mut routes = Vec::new();
    let top_static_limitation_route = top_limitation_routes.first();
    let static_limitation_repair_route = top_static_limitation_route
        .map(|route| route.repair_route.as_str())
        .unwrap_or("cargo xtask lane1-evidence-audit");
    let static_limitation_backlog_sample =
        ripr_swarm_readiness_limitation_route_sample(top_static_limitation_route);
    let static_limitation_sample =
        if ripr_swarm_readiness_blocked_sample_has_context(&static_limitation_backlog_sample) {
            static_limitation_backlog_sample
        } else {
            ripr_swarm_readiness_plan_packet_sample(
                swarm_plan,
                "blocked_by_static_limitation",
                |packet| {
                    audit_non_empty_string(packet, &["swarm_state"]).as_deref()
                        == Some("blocked_by_static_limitation")
                },
            )
        };
    ripr_swarm_readiness_push_blocked_state_route(
        &mut routes,
        "blocked_by_missing_context",
        summary.blocked_by_missing_context_packets,
        "required packet context is missing before the packet can be safely delegated",
        "inspect_blocked_missing_context",
        "cargo xtask lane1-evidence-audit",
        ripr_swarm_readiness_plan_packet_sample(
            swarm_plan,
            "blocked_by_missing_context",
            |packet| {
                audit_non_empty_string(packet, &["swarm_state"]).as_deref()
                    == Some("blocked_by_missing_context")
            },
        ),
    );
    ripr_swarm_readiness_push_blocked_state_route(
        &mut routes,
        "blocked_by_static_limitation",
        summary.blocked_by_static_limitation_packets,
        "a named static limitation prevents a safe bounded repair route",
        "route_static_limitations",
        static_limitation_repair_route,
        static_limitation_sample,
    );
    ripr_swarm_readiness_push_blocked_state_route(
        &mut routes,
        "blocked_by_public_projection_exclusion",
        summary.blocked_by_public_projection_exclusion_packets,
        &ripr_swarm_readiness_public_projection_exclusion_reason(summary),
        "inspect_public_projection_exclusions",
        "cargo xtask lane1-evidence-audit",
        ripr_swarm_readiness_plan_packet_sample(
            swarm_plan,
            "blocked_by_public_projection_exclusion",
            |packet| {
                audit_non_empty_string(packet, &["swarm_state"]).as_deref()
                    == Some("blocked_by_public_projection_exclusion")
            },
        ),
    );
    ripr_swarm_readiness_push_blocked_state_route(
        &mut routes,
        "blocked_by_operator_judgment",
        summary.blocked_by_operator_judgment_packets,
        "typed context exists, but default swarm routing still requires operator judgment",
        "route_operator_judgment_packets",
        "cargo xtask ripr-swarm plan --top 10",
        ripr_swarm_readiness_plan_packet_sample(
            swarm_plan,
            "blocked_by_operator_judgment",
            |packet| {
                audit_non_empty_string(packet, &["swarm_state"]).as_deref()
                    == Some("blocked_by_operator_judgment")
            },
        ),
    );
    ripr_swarm_readiness_push_blocked_state_route(
        &mut routes,
        "missing_canonical_gap_id",
        summary.missing_canonical_gap_id,
        "the packet has no stable canonical_gap_id identity",
        "fix_canonical_gap_identity",
        "cargo xtask lane1-evidence-audit",
        ripr_swarm_readiness_plan_packet_sample(swarm_plan, "missing_canonical_gap_id", |packet| {
            ripr_swarm_readiness_packet_missing_context(packet, "canonical_gap_id")
                || ripr_swarm_readiness_packet_projection_exclusion(
                    packet,
                    "missing_canonical_gap_id",
                )
        }),
    );
    ripr_swarm_readiness_push_blocked_state_route(
        &mut routes,
        "not_actionable_gap_state",
        summary.not_actionable_gap_state,
        "gap_state is not actionable, so the packet must stay out of public repair queues",
        "inspect_non_actionable_gap_state",
        "cargo xtask lane1-evidence-audit",
        ripr_swarm_readiness_plan_packet_sample(swarm_plan, "not_actionable_gap_state", |packet| {
            ripr_swarm_readiness_packet_missing_context(packet, "actionable_gap_state")
                || ripr_swarm_readiness_packet_projection_exclusion(
                    packet,
                    "not_actionable_gap_state",
                )
        }),
    );
    ripr_swarm_readiness_push_blocked_state_route(
        &mut routes,
        "missing_verify_command",
        summary.missing_verify_command,
        "the packet has no usable verify command",
        "fix_verify_command_source",
        "cargo xtask lane1-evidence-audit",
        ripr_swarm_readiness_plan_packet_sample(swarm_plan, "missing_verify_command", |packet| {
            ripr_swarm_readiness_packet_missing_context(packet, "verify_command")
                || ripr_swarm_readiness_packet_projection_exclusion(
                    packet,
                    "missing_verify_command",
                )
                || ripr_swarm_readiness_packet_projection_exclusion(
                    packet,
                    "unbounded_verify_command",
                )
        }),
    );
    ripr_swarm_readiness_push_blocked_state_route(
        &mut routes,
        "missing_receipt_command",
        summary.missing_receipt_command,
        "the packet has no usable receipt command",
        "fix_receipt_command_source",
        "cargo xtask lane1-evidence-audit",
        ripr_swarm_readiness_plan_packet_sample(swarm_plan, "missing_receipt_command", |packet| {
            ripr_swarm_readiness_packet_missing_context(packet, "receipt_command")
                || ripr_swarm_readiness_packet_projection_exclusion(
                    packet,
                    "missing_receipt_command",
                )
        }),
    );
    ripr_swarm_readiness_push_blocked_state_route(
        &mut routes,
        "missing_repair_kind",
        summary.missing_repair_kind,
        "the packet has no usable repair_kind",
        "fix_repair_kind_source",
        "cargo xtask lane1-evidence-audit",
        ripr_swarm_readiness_plan_packet_sample(swarm_plan, "missing_repair_kind", |packet| {
            ripr_swarm_readiness_packet_missing_context(packet, "repair_kind")
                || ripr_swarm_readiness_packet_projection_exclusion(packet, "missing_repair_kind")
        }),
    );
    ripr_swarm_readiness_push_blocked_state_route(
        &mut routes,
        "missing_repair_route",
        summary.missing_repair_route,
        "the packet has no structured repair route",
        "fix_repair_route_source",
        "cargo xtask lane1-evidence-audit",
        ripr_swarm_readiness_plan_packet_sample(swarm_plan, "missing_repair_route", |packet| {
            ripr_swarm_readiness_packet_missing_context(packet, "repair_route")
                || ripr_swarm_readiness_packet_projection_exclusion(packet, "missing_repair_route")
        }),
    );
    ripr_swarm_readiness_push_blocked_state_route(
        &mut routes,
        "missing_target_test_shape",
        summary.missing_target_test_shape,
        "the packet has no target_test_shape repair/test shape",
        "fix_target_test_shape",
        "cargo xtask lane1-evidence-audit",
        ripr_swarm_readiness_plan_packet_sample(
            swarm_plan,
            "missing_target_test_shape",
            |packet| {
                ripr_swarm_readiness_packet_missing_context(packet, "target_test_shape")
                    || ripr_swarm_readiness_packet_projection_exclusion(
                        packet,
                        "missing_target_test_shape",
                    )
            },
        ),
    );
    ripr_swarm_readiness_push_blocked_state_route(
        &mut routes,
        "missing_related_test_or_observer",
        summary.missing_related_test_or_observer,
        "the packet has no typed related test or observer target",
        "fix_related_test_or_observer",
        "cargo xtask lane1-evidence-audit",
        ripr_swarm_readiness_plan_packet_sample(
            swarm_plan,
            "missing_related_test_or_observer",
            |packet| {
                audit_bool(packet, &["related_test_or_observer_available"]) == Some(false)
                    || ripr_swarm_readiness_packet_missing_context(
                        packet,
                        "related_test_or_observer",
                    )
                    || ripr_swarm_readiness_packet_projection_exclusion(
                        packet,
                        "missing_related_test_or_observer",
                    )
            },
        ),
    );
    ripr_swarm_readiness_push_blocked_state_route(
        &mut routes,
        "missing_must_not_change",
        summary.missing_must_not_change,
        "the packet has no must_not_change boundaries",
        "fix_must_not_change_boundaries",
        "cargo xtask lane1-evidence-audit",
        ripr_swarm_readiness_plan_packet_sample(swarm_plan, "missing_must_not_change", |packet| {
            ripr_swarm_readiness_packet_missing_context(packet, "must_not_change")
                || ripr_swarm_readiness_packet_projection_exclusion(
                    packet,
                    "missing_must_not_change",
                )
        }),
    );
    ripr_swarm_readiness_push_blocked_state_route(
        &mut routes,
        "missing_allowed_edit_surface",
        summary.missing_allowed_edit_surface,
        "the packet has no allowed_edit_surface edit cage",
        "fix_allowed_edit_surface",
        "cargo xtask lane1-evidence-audit",
        ripr_swarm_readiness_plan_packet_sample(
            swarm_plan,
            "missing_allowed_edit_surface",
            |packet| {
                ripr_swarm_readiness_packet_missing_context(packet, "allowed_edit_surface")
                    || ripr_swarm_readiness_packet_projection_exclusion(
                        packet,
                        "missing_allowed_edit_surface",
                    )
            },
        ),
    );
    ripr_swarm_readiness_push_blocked_state_route(
        &mut routes,
        "missing_confidence",
        summary.missing_confidence,
        "the packet has no usable confidence basis",
        "fix_confidence_basis",
        "cargo xtask lane1-evidence-audit",
        ripr_swarm_readiness_plan_packet_sample(swarm_plan, "missing_confidence", |packet| {
            ripr_swarm_readiness_packet_missing_context(packet, "confidence_basis")
                || ripr_swarm_readiness_packet_projection_exclusion(packet, "missing_confidence")
        }),
    );
    ripr_swarm_readiness_push_blocked_state_route(
        &mut routes,
        "missing_raw_evidence_refs",
        summary.missing_raw_evidence_refs,
        "the packet has no structured raw evidence references",
        "fix_raw_evidence_refs",
        "cargo xtask lane1-evidence-audit",
        ripr_swarm_readiness_plan_packet_sample(
            swarm_plan,
            "missing_raw_evidence_refs",
            |packet| {
                ripr_swarm_readiness_packet_missing_context(packet, "raw_evidence_refs")
                    || ripr_swarm_readiness_packet_projection_exclusion(
                        packet,
                        "missing_raw_evidence_refs",
                    )
            },
        ),
    );
    ripr_swarm_readiness_push_blocked_state_route(
        &mut routes,
        "attempted_no_receipt",
        summary.attempted_no_receipt_packets,
        "an attempt exists without a matching receipt",
        "collect_missing_attempt_receipts",
        "cargo xtask ripr-swarm attempt-ledger",
        ripr_swarm_readiness_attempt_sample(attempt_ledger, |attempt| {
            attempt.outcome == "attempted_no_receipt"
        }),
    );
    ripr_swarm_readiness_push_blocked_state_route(
        &mut routes,
        "missing_verify_result",
        summary.missing_verify_result,
        "an attempt is missing typed verify_result evidence",
        "inspect_missing_verify_results",
        "cargo xtask ripr-swarm attempt-ledger",
        ripr_swarm_readiness_attempt_sample(attempt_ledger, |attempt| {
            attempt.outcome != "not_attempted"
                && attempt
                    .verify_result
                    .as_deref()
                    .is_none_or(ripr_swarm_plan_field_missing)
        }),
    );
    ripr_swarm_readiness_push_blocked_state_route(
        &mut routes,
        "orphan_receipt",
        summary.orphaned_receipts,
        "a receipt does not match any current actionable packet",
        "reconcile_orphaned_receipts",
        "cargo xtask ripr-swarm attempt-ledger",
        ripr_swarm_readiness_orphan_receipt_sample(attempt_ledger),
    );
    ripr_swarm_readiness_push_blocked_state_route(
        &mut routes,
        "unchanged_attempt",
        summary.unchanged_packets,
        "an attempted packet left evidence unchanged",
        "inspect_unchanged_attempts",
        "cargo xtask ripr-swarm attempt-ledger",
        ripr_swarm_readiness_attempt_sample(attempt_ledger, |attempt| {
            attempt.outcome == "evidence_unchanged"
        }),
    );
    ripr_swarm_readiness_push_blocked_state_route(
        &mut routes,
        "regressed_attempt",
        summary.regressed_packets,
        "an attempted packet regressed evidence",
        "inspect_regressed_attempts",
        "cargo xtask ripr-swarm attempt-ledger",
        ripr_swarm_readiness_attempt_sample(attempt_ledger, |attempt| {
            attempt.outcome == "evidence_regressed"
        }),
    );
    routes
}

pub(crate) fn ripr_swarm_readiness_public_projection_exclusion_reason(
    summary: &RiprSwarmReadinessSummary,
) -> String {
    format!(
        "public projection eligibility is false or projection_exclusion_reasons are present{}",
        ripr_swarm_readiness_public_projection_exclusion_detail(summary)
    )
}

pub(crate) fn ripr_swarm_readiness_public_projection_exclusion_detail(
    summary: &RiprSwarmReadinessSummary,
) -> String {
    ripr_swarm_readiness_top_public_projection_exclusion_reason(summary)
        .map(|(label, count)| format!("; top reason: {label} ({count})"))
        .unwrap_or_default()
}

pub(crate) fn ripr_swarm_readiness_top_public_projection_exclusion_reason(
    summary: &RiprSwarmReadinessSummary,
) -> Option<(&str, usize)> {
    summary
        .public_projection_exclusion_reasons
        .iter()
        .max_by(|left, right| left.1.cmp(right.1).then_with(|| right.0.cmp(left.0)))
        .map(|(label, count)| (label.as_str(), *count))
}

pub(crate) fn ripr_swarm_readiness_push_blocked_state_route(
    routes: &mut Vec<RiprSwarmReadinessBlockedStateRoute>,
    state: &str,
    count: usize,
    reason: &str,
    next_action_kind: &str,
    repair_route: &str,
    sample: RiprSwarmReadinessBlockedStateSample,
) {
    if count == 0 {
        return;
    }
    routes.push(RiprSwarmReadinessBlockedStateRoute {
        state: state.to_string(),
        count,
        reason: reason.to_string(),
        next_action_kind: next_action_kind.to_string(),
        repair_route: repair_route.to_string(),
        example_packet_id: sample.packet_id,
        example_canonical_gap_id: sample.canonical_gap_id,
        example_repair_kind: sample.repair_kind,
        example_receipt_path: sample.receipt_path,
    });
}

pub(crate) fn ripr_swarm_readiness_limitation_route_sample(
    route: Option<&RiprSwarmLimitationRouteRow>,
) -> RiprSwarmReadinessBlockedStateSample {
    let Some(route) = route else {
        return RiprSwarmReadinessBlockedStateSample::default();
    };
    RiprSwarmReadinessBlockedStateSample {
        packet_id: route.sample_packet_id.clone(),
        canonical_gap_id: route.sample_canonical_gap_ids.first().cloned(),
        repair_kind: None,
        receipt_path: None,
    }
}

pub(crate) fn ripr_swarm_readiness_blocked_sample_has_context(
    sample: &RiprSwarmReadinessBlockedStateSample,
) -> bool {
    sample.packet_id.is_some() || sample.canonical_gap_id.is_some()
}

pub(crate) fn ripr_swarm_readiness_plan_packet_sample<F>(
    swarm_plan: Option<&Value>,
    state: &str,
    predicate: F,
) -> RiprSwarmReadinessBlockedStateSample
where
    F: Fn(&Value) -> bool,
{
    let Some(plan) = swarm_plan else {
        return RiprSwarmReadinessBlockedStateSample::default();
    };
    if let Some(example) = audit_array(plan, &["blocked_state_examples"])
        .iter()
        .find(|example| audit_non_empty_string(example, &["state"]).as_deref() == Some(state))
    {
        return RiprSwarmReadinessBlockedStateSample {
            packet_id: audit_non_empty_string(example, &["example_packet_id"]),
            canonical_gap_id: audit_non_empty_string(example, &["example_canonical_gap_id"]),
            repair_kind: audit_non_empty_string(example, &["example_repair_kind"]),
            receipt_path: None,
        };
    }
    let mut packets = audit_array(plan, &["top_blocked_packets"])
        .iter()
        .collect::<Vec<_>>();
    packets.extend(audit_array(plan, &["top_ready_packets"]).iter());
    packets
        .into_iter()
        .find(|packet| predicate(packet))
        .map(|packet| RiprSwarmReadinessBlockedStateSample {
            packet_id: audit_non_empty_string(packet, &["packet_id"]),
            canonical_gap_id: audit_non_empty_string(packet, &["canonical_gap_id"]),
            repair_kind: audit_non_empty_string(packet, &["repair_kind"]),
            receipt_path: None,
        })
        .unwrap_or_default()
}

pub(crate) fn ripr_swarm_readiness_attempt_sample<F>(
    attempt_ledger: Option<&Value>,
    predicate: F,
) -> RiprSwarmReadinessBlockedStateSample
where
    F: Fn(&RiprSwarmAttemptLedgerEntry) -> bool,
{
    let Some(ledger) = attempt_ledger else {
        return RiprSwarmReadinessBlockedStateSample::default();
    };
    let attempts = ripr_swarm_attempt_ledger_entries_from_value(ledger);
    let latest_attempts = ripr_swarm_attempt_ledger_latest_attempts(&attempts);
    latest_attempts
        .into_iter()
        .find(predicate)
        .map(|attempt| RiprSwarmReadinessBlockedStateSample {
            packet_id: Some(attempt.packet_id),
            canonical_gap_id: Some(attempt.canonical_gap_id),
            repair_kind: attempt.repair_kind,
            receipt_path: attempt.receipt_path,
        })
        .unwrap_or_default()
}

pub(crate) fn ripr_swarm_readiness_orphan_receipt_sample(
    attempt_ledger: Option<&Value>,
) -> RiprSwarmReadinessBlockedStateSample {
    let Some(ledger) = attempt_ledger else {
        return RiprSwarmReadinessBlockedStateSample::default();
    };
    audit_array(ledger, &["orphaned_receipts"])
        .first()
        .map(|receipt| RiprSwarmReadinessBlockedStateSample {
            packet_id: audit_non_empty_string(receipt, &["packet_id"]),
            canonical_gap_id: audit_non_empty_string(receipt, &["canonical_gap_id"]),
            repair_kind: audit_non_empty_string(receipt, &["repair_kind"]),
            receipt_path: audit_non_empty_string(receipt, &["receipt_path"])
                .or_else(|| audit_non_empty_string(receipt, &["path"])),
        })
        .unwrap_or_default()
}

pub(crate) fn ripr_swarm_readiness_packet_missing_context(packet: &Value, field: &str) -> bool {
    audit_array(packet, &["missing_context"])
        .iter()
        .any(|value| value.as_str() == Some(field))
}

pub(crate) fn ripr_swarm_readiness_packet_projection_exclusion(
    packet: &Value,
    reason: &str,
) -> bool {
    audit_array(packet, &["projection_exclusion_reasons"])
        .iter()
        .any(|value| value.as_str() == Some(reason))
}

pub(crate) fn ripr_swarm_readiness_blocked_state_routes_json(
    routes: &[RiprSwarmReadinessBlockedStateRoute],
) -> Vec<Value> {
    routes
        .iter()
        .map(|route| {
            serde_json::json!({
                "state": route.state,
                "count": route.count,
                "reason": route.reason,
                "next_action_kind": route.next_action_kind,
                "repair_route": route.repair_route,
                "example_packet_id": route.example_packet_id,
                "example_canonical_gap_id": route.example_canonical_gap_id,
                "example_repair_kind": route.example_repair_kind,
                "example_receipt_path": route.example_receipt_path,
            })
        })
        .collect()
}

pub(crate) fn ripr_swarm_readiness_repair_route_quality(
    attempt_ledger: &Value,
) -> Vec<RiprSwarmRepairRouteQualityRow> {
    let attempts = ripr_swarm_attempt_ledger_entries_from_value(attempt_ledger);
    if !attempts.is_empty() {
        let latest_attempts = ripr_swarm_attempt_ledger_latest_attempts(&attempts);
        return ripr_swarm_attempt_ledger_repair_route_quality(&latest_attempts);
    }
    audit_array(attempt_ledger, &["repair_route_quality"])
        .iter()
        .filter_map(ripr_swarm_repair_route_quality_row_from_value)
        .collect::<Vec<_>>()
}

pub(crate) fn ripr_swarm_readiness_language_repair_route_quality(
    attempt_ledger: &Value,
) -> Vec<RiprSwarmRepairRouteQualityRow> {
    let attempts = ripr_swarm_attempt_ledger_entries_from_value(attempt_ledger);
    if !attempts.is_empty() {
        let latest_attempts = ripr_swarm_attempt_ledger_latest_attempts(&attempts);
        return ripr_swarm_attempt_ledger_language_repair_route_quality(&latest_attempts);
    }
    audit_array(attempt_ledger, &["language_repair_route_quality"])
        .iter()
        .filter_map(ripr_swarm_repair_route_quality_row_from_value)
        .collect::<Vec<_>>()
}

pub(crate) fn ripr_swarm_readiness_top_limitation_routes(
    backlog: &Value,
) -> Vec<RiprSwarmLimitationRouteRow> {
    let mut rows = BTreeMap::<String, RiprSwarmLimitationRouteRow>::new();
    for row in audit_array(backlog, &["top_repair_routes"]) {
        let Some(repair_route) = audit_non_empty_string(row, &["repair_route"])
            .or_else(|| audit_non_empty_string(row, &["label"]))
        else {
            continue;
        };
        let signal_count = audit_usize(row, &["count"]).unwrap_or_default();
        rows.entry(repair_route.clone())
            .or_insert_with(|| RiprSwarmLimitationRouteRow {
                repair_route,
                signal_count,
                sample_packet_id: None,
                sample_limitation_category: None,
                sample_limitation_subroute: None,
                sample_canonical_gap_ids: Vec::new(),
                sample_sources: Vec::new(),
                dominant_evidence_class: None,
                why_not_actionable: None,
                unlock_condition: None,
                non_claims: Vec::new(),
            })
            .signal_count = signal_count;
    }
    for packet in audit_array(backlog, &["limitation_backlog_packets"]) {
        let category = audit_non_empty_string(packet, &["limitation_category"])
            .or_else(|| audit_non_empty_string(packet, &["category"]));
        let Some(repair_route) = audit_non_empty_string(packet, &["repair_route"]).or_else(|| {
            category
                .as_deref()
                .map(static_limitation_repair_route)
                .map(str::to_string)
        }) else {
            continue;
        };
        let signal_count = audit_usize(packet, &["signal_count"]).unwrap_or_default();
        let row = rows
            .entry(repair_route.clone())
            .or_insert_with(|| RiprSwarmLimitationRouteRow {
                repair_route: repair_route.clone(),
                signal_count,
                sample_packet_id: None,
                sample_limitation_category: None,
                sample_limitation_subroute: None,
                sample_canonical_gap_ids: Vec::new(),
                sample_sources: Vec::new(),
                dominant_evidence_class: None,
                why_not_actionable: None,
                unlock_condition: None,
                non_claims: Vec::new(),
            });
        row.signal_count = row.signal_count.max(signal_count);
        if row.sample_packet_id.is_none() {
            let category_for_defaults = category.clone();
            row.sample_packet_id = audit_non_empty_string(packet, &["packet_id"]);
            row.sample_limitation_category = category;
            row.sample_limitation_subroute =
                audit_non_empty_string(packet, &["limitation_subroute"]).or_else(|| {
                    row.sample_limitation_category
                        .as_deref()
                        .map(audit_identifier_slug)
                });
            row.sample_canonical_gap_ids = ripr_swarm_limitation_route_sample_gap_ids(packet);
            row.sample_sources = ripr_swarm_limitation_route_sample_sources(packet);
            row.dominant_evidence_class =
                audit_non_empty_string(packet, &["dominant_evidence_class"])
                    .or_else(|| Some("unknown".to_string()));
            row.why_not_actionable = audit_non_empty_string(packet, &["why_not_actionable"])
                .or_else(|| {
                    category_for_defaults
                        .as_deref()
                        .map(static_limitation_why_not_actionable)
                        .map(str::to_string)
                });
            row.unlock_condition =
                audit_non_empty_string(packet, &["unlock_condition"]).or_else(|| {
                    let subroute_for_defaults = row.sample_limitation_subroute.clone();
                    category_for_defaults.as_deref().map(|category| {
                        let subroute = subroute_for_defaults
                            .clone()
                            .unwrap_or_else(|| audit_identifier_slug(category));
                        static_limitation_unlock_condition(category, &subroute, &repair_route)
                    })
                });
            row.non_claims =
                ripr_swarm_limitation_route_non_claims(packet, category_for_defaults.as_deref());
        }
    }
    let mut rows = rows.into_values().collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        right
            .signal_count
            .cmp(&left.signal_count)
            .then_with(|| left.repair_route.cmp(&right.repair_route))
    });
    rows.truncate(LANE1_EVIDENCE_AUDIT_TOP_LIMIT);
    rows
}

pub(crate) fn ripr_swarm_limitation_route_sample_gap_ids(packet: &Value) -> Vec<String> {
    let direct = audit_string_array(packet, &["sample_canonical_gap_ids"]).unwrap_or_default();
    if !direct.is_empty() {
        return direct.into_iter().take(3).collect();
    }
    let source_ids = audit_array(packet, &["sample_sources"])
        .iter()
        .filter_map(|sample| audit_non_empty_string(sample, &["canonical_gap_id"]))
        .take(3)
        .collect::<Vec<_>>();
    if !source_ids.is_empty() {
        return source_ids;
    }
    audit_array(packet, &["samples"])
        .iter()
        .filter_map(|sample| audit_non_empty_string(sample, &["canonical_gap_id"]))
        .take(3)
        .collect()
}

pub(crate) fn ripr_swarm_limitation_route_sample_sources(
    packet: &Value,
) -> Vec<Lane1StaticLimitationBacklogSample> {
    audit_array(packet, &["sample_sources"])
        .iter()
        .filter_map(|sample| {
            let source_file = audit_non_empty_string(sample, &["source_file"])?;
            Some(Lane1StaticLimitationBacklogSample {
                canonical_gap_id: audit_non_empty_string(sample, &["canonical_gap_id"]),
                evidence_class: audit_non_empty_string(sample, &["evidence_class"])
                    .unwrap_or_else(|| "unknown".to_string()),
                source_file,
                line: audit_usize(sample, &["line"]),
                expression: audit_non_empty_string(sample, &["expression"]),
                limitation_reason: audit_non_empty_string(sample, &["limitation_reason"]),
            })
        })
        .take(3)
        .collect()
}

pub(crate) fn ripr_swarm_limitation_route_non_claims(
    packet: &Value,
    category: Option<&str>,
) -> Vec<String> {
    let mut claims = audit_string_array(packet, &["non_claims"]).unwrap_or_default();
    let subroute = audit_non_empty_string(packet, &["limitation_subroute"]);
    let required_claims = category
        .map(|category| static_limitation_backlog_packet_non_claims(category, subroute.as_deref()))
        .unwrap_or_else(|| static_limitation_backlog_packet_non_claims("unknown", None));
    for claim in required_claims {
        if !claims.iter().any(|existing| existing == &claim) {
            claims.push(claim);
        }
    }
    claims
}

pub(crate) fn ripr_swarm_limitation_routes_json(
    rows: &[RiprSwarmLimitationRouteRow],
) -> Vec<Value> {
    rows.iter()
        .map(|row| {
            serde_json::json!({
                "repair_route": row.repair_route,
                "signal_count": row.signal_count,
                "sample_packet_id": row.sample_packet_id,
                "sample_limitation_category": row.sample_limitation_category,
                "sample_limitation_subroute": row.sample_limitation_subroute,
                "sample_canonical_gap_ids": row.sample_canonical_gap_ids,
                "sample_sources": row
                    .sample_sources
                    .iter()
                    .map(lane1_static_limitation_backlog_sample_json)
                    .collect::<Vec<_>>(),
                "dominant_evidence_class": row.dominant_evidence_class,
                "why_not_actionable": row.why_not_actionable,
                "unlock_condition": row.unlock_condition,
                "non_claims": row.non_claims,
            })
        })
        .collect()
}

pub(crate) fn ripr_swarm_limitation_route_quality_json(
    rows: &[RiprSwarmLimitationRouteRow],
) -> Vec<Value> {
    rows.iter()
        .map(|row| {
            serde_json::json!({
                "repair_route": row.repair_route,
                "quality_basis": "static_limitation_backlog",
                "route_state": "non_actionable_limitation",
                "signal_count": row.signal_count,
                "sample_packet_id": row.sample_packet_id,
                "sample_limitation_category": row.sample_limitation_category,
                "sample_limitation_subroute": row.sample_limitation_subroute,
                "dominant_evidence_class": row.dominant_evidence_class,
                "sample_canonical_gap_ids": row.sample_canonical_gap_ids,
                "sample_sources": row
                    .sample_sources
                    .iter()
                    .map(lane1_static_limitation_backlog_sample_json)
                    .collect::<Vec<_>>(),
                "language_aware_placement": ripr_swarm_limitation_route_language_aware_placement_json(row),
                "why_not_actionable": row.why_not_actionable,
                "unlock_condition": row.unlock_condition,
                "packet_policy": "not_public_repair_packet",
                "non_claims": row.non_claims,
            })
        })
        .collect()
}

pub(crate) fn ripr_swarm_limitation_route_language_aware_placement_json(
    row: &RiprSwarmLimitationRouteRow,
) -> Value {
    serde_json::json!({
        "applies": ripr_swarm_limitation_route_is_language_aware(row),
        "status": ripr_swarm_limitation_route_language_aware_status(row),
        "navigation_only_external_target_status":
            ripr_swarm_limitation_route_navigation_only_target_status(row),
        "repair_route": row.repair_route,
        "category": row.sample_limitation_category,
        "non_claim": "navigation-only target evidence is context only until explicit target, verify, receipt, and edit-surface fields are present"
    })
}

pub(crate) fn ripr_swarm_limitation_route_is_language_aware(
    row: &RiprSwarmLimitationRouteRow,
) -> bool {
    matches!(
        row.sample_limitation_category.as_deref(),
        Some(CROSS_LANGUAGE_TARGET_UNRESOLVED_CATEGORY)
            | Some(CROSS_LANGUAGE_ORACLE_VISIBILITY_UNRESOLVED_CATEGORY)
    ) || matches!(
        row.repair_route.as_str(),
        CROSS_LANGUAGE_TARGET_UNRESOLVED_REPAIR_ROUTE
            | CROSS_LANGUAGE_ORACLE_VISIBILITY_REPAIR_ROUTE
    )
}

pub(crate) fn ripr_swarm_limitation_route_language_aware_status(
    row: &RiprSwarmLimitationRouteRow,
) -> &'static str {
    if row.sample_limitation_category.as_deref() == Some(CROSS_LANGUAGE_TARGET_UNRESOLVED_CATEGORY)
        || row.repair_route == CROSS_LANGUAGE_TARGET_UNRESOLVED_REPAIR_ROUTE
    {
        "cross_language_target_inference_unresolved"
    } else if row.sample_limitation_category.as_deref()
        == Some(CROSS_LANGUAGE_ORACLE_VISIBILITY_UNRESOLVED_CATEGORY)
        || row.repair_route == CROSS_LANGUAGE_ORACLE_VISIBILITY_REPAIR_ROUTE
    {
        "cross_language_oracle_visibility_unresolved"
    } else {
        "not_language_aware_placement_route"
    }
}

pub(crate) fn ripr_swarm_limitation_route_navigation_only_target_status(
    row: &RiprSwarmLimitationRouteRow,
) -> &'static str {
    if row.sample_limitation_category.as_deref() == Some(CROSS_LANGUAGE_TARGET_UNRESOLVED_CATEGORY)
        || row.repair_route == CROSS_LANGUAGE_TARGET_UNRESOLVED_REPAIR_ROUTE
    {
        "requires_explicit_external_observer_target_evidence"
    } else if row.sample_limitation_category.as_deref()
        == Some(CROSS_LANGUAGE_ORACLE_VISIBILITY_UNRESOLVED_CATEGORY)
        || row.repair_route == CROSS_LANGUAGE_ORACLE_VISIBILITY_REPAIR_ROUTE
    {
        "target_context_is_not_oracle_proof"
    } else {
        "not_applicable"
    }
}

pub(crate) fn ripr_swarm_readiness_top_failing_repair_routes(
    rows: &[RiprSwarmRepairRouteQualityRow],
) -> Vec<RiprSwarmRepairRouteQualityRow> {
    let mut rows = rows
        .iter()
        .filter(|row| ripr_swarm_repair_route_quality_failure_count(row) > 0)
        .cloned()
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        ripr_swarm_repair_route_quality_failure_count(right)
            .cmp(&ripr_swarm_repair_route_quality_failure_count(left))
            .then_with(|| right.regressed.cmp(&left.regressed))
            .then_with(|| right.missing_verify_result.cmp(&left.missing_verify_result))
            .then_with(|| {
                ripr_swarm_repair_route_quality_unexpected_unchanged(right)
                    .cmp(&ripr_swarm_repair_route_quality_unexpected_unchanged(left))
            })
            .then_with(|| right.attempted_no_receipt.cmp(&left.attempted_no_receipt))
            .then_with(|| right.unknown.cmp(&left.unknown))
            .then_with(|| left.repair_kind.cmp(&right.repair_kind))
    });
    rows.truncate(LANE1_EVIDENCE_AUDIT_TOP_LIMIT);
    rows
}

pub(crate) fn ripr_swarm_repair_route_quality_failure_count(
    row: &RiprSwarmRepairRouteQualityRow,
) -> usize {
    row.regressed
        + ripr_swarm_repair_route_quality_unexpected_unchanged(row)
        + row.attempted_no_receipt
        + row.missing_verify_result
        + row.unknown
}

pub(crate) fn ripr_swarm_repair_route_quality_dominant_failure_reason(
    row: &RiprSwarmRepairRouteQualityRow,
) -> Option<&'static str> {
    let mut dominant = None;
    for (reason, count) in [
        ("regressed", row.regressed),
        ("missing_verify_result", row.missing_verify_result),
        (
            "unchanged",
            ripr_swarm_repair_route_quality_unexpected_unchanged(row),
        ),
        ("attempted_no_receipt", row.attempted_no_receipt),
        ("unknown", row.unknown),
    ] {
        if count > dominant.map(|(_reason, current)| current).unwrap_or(0) {
            dominant = Some((reason, count));
        }
    }
    dominant.map(|(reason, _count)| reason)
}

pub(crate) fn ripr_swarm_repair_route_quality_dominant_failure_count(
    row: &RiprSwarmRepairRouteQualityRow,
) -> usize {
    [
        row.regressed,
        row.missing_verify_result,
        ripr_swarm_repair_route_quality_unexpected_unchanged(row),
        row.attempted_no_receipt,
        row.unknown,
    ]
    .into_iter()
    .max()
    .unwrap_or(0)
}

pub(crate) fn ripr_swarm_readiness_top_missing_evidence_fields(
    attempt_ledger: &Value,
) -> Vec<RiprSwarmMissingEvidenceFieldRow> {
    let attempts = ripr_swarm_attempt_ledger_entries_from_value(attempt_ledger);
    if !attempts.is_empty() {
        let latest_attempts = ripr_swarm_attempt_ledger_latest_attempts(&attempts);
        return ripr_swarm_attempt_ledger_top_missing_evidence_fields(&latest_attempts);
    }
    audit_array(attempt_ledger, &["top_missing_evidence_fields"])
        .iter()
        .filter_map(|row| {
            Some(RiprSwarmMissingEvidenceFieldRow {
                label: audit_non_empty_string(row, &["label"])?,
                count: audit_usize(row, &["count"]).unwrap_or_default(),
                sample_packet_ids: audit_string_array(row, &["sample_packet_ids"])
                    .unwrap_or_default(),
                sample_canonical_gap_ids: audit_string_array(row, &["sample_canonical_gap_ids"])
                    .unwrap_or_default(),
                sample_repair_kinds: audit_string_array(row, &["sample_repair_kinds"])
                    .unwrap_or_default(),
            })
        })
        .collect::<Vec<_>>()
}

pub(crate) fn ripr_swarm_top_missing_evidence_field_count(
    report: &Value,
    label: &str,
) -> Option<usize> {
    audit_array(report, &["top_missing_evidence_fields"])
        .iter()
        .find(|row| audit_non_empty_string(row, &["label"]).as_deref() == Some(label))
        .and_then(|row| audit_usize(row, &["count"]))
}

pub(crate) fn ripr_swarm_missing_evidence_field_sample(
    rows: &[RiprSwarmMissingEvidenceFieldRow],
    label: &str,
) -> (Option<String>, Option<String>, Option<String>) {
    let Some(row) = rows.iter().find(|row| row.label == label) else {
        return (None, None, None);
    };
    (
        row.sample_packet_ids.first().cloned(),
        row.sample_canonical_gap_ids.first().cloned(),
        row.sample_repair_kinds.first().cloned(),
    )
}

pub(crate) fn ripr_swarm_readiness_missing_receipt_action_from_route(
    route: &RiprSwarmRepairRouteQualityRow,
) -> RiprSwarmReadinessNextAction {
    ripr_swarm_readiness_missing_receipt_action(
        route.attempted_no_receipt,
        route.sample_packet_ids.first().cloned(),
        route.sample_canonical_gap_ids.first().cloned(),
        Some(route.repair_kind.clone()),
    )
}

pub(crate) fn ripr_swarm_readiness_missing_receipt_action(
    count: usize,
    packet_id: Option<String>,
    canonical_gap_id: Option<String>,
    repair_kind: Option<String>,
) -> RiprSwarmReadinessNextAction {
    RiprSwarmReadinessNextAction {
        kind: "collect_missing_attempt_receipts".to_string(),
        packet_id,
        attempt_id: None,
        canonical_gap_id,
        evidence_class: None,
        repair_kind,
        command: Some("cargo xtask ripr-swarm attempt-ledger".to_string()),
        reason: format!(
            "{count} attempted packet(s) have no matching receipt; run the packet receipt command and refresh the attempt ledger before claiming outcomes"
        ),
    }
}

pub(crate) fn ripr_swarm_readiness_repair_route_quality_action(
    route: &RiprSwarmRepairRouteQualityRow,
) -> RiprSwarmReadinessNextAction {
    let failures = ripr_swarm_repair_route_quality_failure_count(route);
    let dominant_reason =
        ripr_swarm_repair_route_quality_dominant_failure_reason(route).unwrap_or("unknown");
    let dominant_count = ripr_swarm_repair_route_quality_dominant_failure_count(route);
    let backlog_packet_id =
        ripr_swarm_repair_route_quality_backlog_packet_id(route, dominant_reason);
    let improvement_route =
        ripr_swarm_repair_route_quality_improvement_route(route, dominant_reason);
    let sample_packet = route
        .sample_packet_ids
        .first()
        .map_or("unknown", String::as_str);
    let sample_attempt = route
        .sample_attempt_ids
        .first()
        .map_or("unknown", String::as_str);
    RiprSwarmReadinessNextAction {
        kind: "improve_repair_route_quality".to_string(),
        packet_id: Some(backlog_packet_id.clone()),
        attempt_id: route.sample_attempt_ids.first().cloned(),
        canonical_gap_id: route.sample_canonical_gap_ids.first().cloned(),
        evidence_class: None,
        repair_kind: Some(route.repair_kind.clone()),
        command: Some("cargo xtask ripr-swarm readiness".to_string()),
        reason: format!(
            "`{}` has {} failing latest attempt(s); dominant reason `{}` appears {} time(s); route backlog packet `{}` through `{}` before increasing packet volume; sample failed packet `{}` attempt `{}`",
            route.repair_kind,
            failures,
            dominant_reason,
            dominant_count,
            backlog_packet_id,
            improvement_route,
            sample_packet,
            sample_attempt
        ),
    }
}

pub(crate) fn ripr_swarm_readiness_has_next_action_packet(
    actions: &[RiprSwarmReadinessNextAction],
    packet_id: Option<&str>,
) -> bool {
    let Some(packet_id) = packet_id else {
        return false;
    };
    actions
        .iter()
        .any(|action| action.packet_id.as_deref() == Some(packet_id))
}

pub(crate) fn ripr_swarm_json_string(row: &Value, path: &[&str]) -> Option<String> {
    audit_get(row, path)
        .and_then(Value::as_str)
        .filter(|value| !ripr_swarm_plan_field_missing(value))
        .map(str::to_owned)
}

pub(crate) fn ripr_swarm_attempt_ledger_outcome_sample(
    ledger: Option<&Value>,
    outcome: &str,
) -> (Option<String>, Option<String>, Option<String>) {
    let Some(ledger) = ledger else {
        return (None, None, None);
    };
    let latest_attempts = audit_array(ledger, &["latest_attempts"]);
    let attempts = if latest_attempts.is_empty() {
        audit_array(ledger, &["attempts"])
    } else {
        latest_attempts
    };
    let Some(attempt) = attempts
        .iter()
        .find(|attempt| audit_get(attempt, &["outcome"]).and_then(Value::as_str) == Some(outcome))
    else {
        return (None, None, None);
    };
    (
        ripr_swarm_json_string(attempt, &["packet_id"]),
        ripr_swarm_json_string(attempt, &["canonical_gap_id"]),
        ripr_swarm_json_string(attempt, &["repair_kind"]),
    )
}

pub(crate) fn ripr_swarm_attempt_ledger_orphaned_receipt_sample(
    ledger: Option<&Value>,
) -> (Option<String>, Option<String>, Option<String>) {
    let Some(ledger) = ledger else {
        return (None, None, None);
    };
    let receipts = audit_array(ledger, &["orphaned_receipts"]);
    let Some(receipt) = receipts.first() else {
        return (None, None, None);
    };
    (
        ripr_swarm_json_string(receipt, &["packet_id"]),
        ripr_swarm_json_string(receipt, &["canonical_gap_id"]),
        ripr_swarm_json_string(receipt, &["repair_kind"]),
    )
}

pub(crate) fn ripr_swarm_repair_route_quality_row_from_value(
    row: &Value,
) -> Option<RiprSwarmRepairRouteQualityRow> {
    let repair_kind = audit_non_empty_string(row, &["repair_kind"])?;
    Some(RiprSwarmRepairRouteQualityRow {
        language: audit_non_empty_string(row, &["language"]),
        repair_kind,
        attempted: audit_usize(row, &["repair_kind_attempted"]).unwrap_or_default(),
        improved: audit_usize(row, &["repair_kind_improved"]).unwrap_or_default(),
        unchanged: audit_usize(row, &["repair_kind_unchanged"]).unwrap_or_default(),
        regressed: audit_usize(row, &["repair_kind_regressed"]).unwrap_or_default(),
        resolved: audit_usize(row, &["repair_kind_resolved"]).unwrap_or_default(),
        attempted_no_receipt: audit_usize(row, &["repair_kind_attempted_no_receipt"])
            .unwrap_or_default(),
        receipt_present: audit_usize(row, &["repair_kind_receipt_present"]).unwrap_or_default(),
        missing_verify_result: audit_usize(row, &["repair_kind_missing_verify_result"])
            .unwrap_or_default(),
        expected_unchanged: audit_usize(row, &["repair_kind_expected_unchanged"])
            .unwrap_or_default(),
        unknown: audit_usize(row, &["repair_kind_unknown"]).unwrap_or_default(),
        sample_packet_ids: audit_string_array(row, &["sample_packet_ids"]).unwrap_or_default(),
        sample_attempt_ids: audit_string_array(row, &["sample_attempt_ids"]).unwrap_or_default(),
        sample_canonical_gap_ids: audit_string_array(row, &["sample_canonical_gap_ids"])
            .unwrap_or_default(),
        sample_missing_receipt_reasons: audit_string_array(
            row,
            &["sample_missing_receipt_reasons"],
        )
        .unwrap_or_default(),
    })
}

pub(crate) const RIPR_SWARM_READINESS_NEXT_ACTION_PACKET_LIMIT: usize = 5;

pub(crate) struct RiprSwarmReadinessNextActionSources<'a> {
    pub(crate) swarm_plan: Option<&'a Value>,
    pub(crate) top_failing_repair_routes: &'a [RiprSwarmRepairRouteQualityRow],
    pub(crate) top_missing_evidence_fields: &'a [RiprSwarmMissingEvidenceFieldRow],
    pub(crate) top_limitation_routes: &'a [RiprSwarmLimitationRouteRow],
    pub(crate) static_limitation_backlog: &'a Value,
}

pub(crate) fn ripr_swarm_readiness_next_actions(
    summary: &RiprSwarmReadinessSummary,
    sources: RiprSwarmReadinessNextActionSources<'_>,
    inputs: [&RiprSwarmReadinessInput<'_>; 3],
    runtime_status: &Lane1RuntimeStatus,
) -> Vec<RiprSwarmReadinessNextAction> {
    let [
        swarm_plan_input,
        actionable_gap_outcomes_input,
        attempt_ledger_input,
    ] = inputs;
    let mut actions = Vec::new();
    let runtime_not_downstream_consumable =
        runtime_status.state != "full" && !runtime_status.downstream_consumable;
    let defer_sampled_runtime_action = runtime_not_downstream_consumable
        && ripr_swarm_readiness_runtime_is_sampled_work_queue(runtime_status);
    if runtime_not_downstream_consumable
        && !defer_sampled_runtime_action
        && swarm_plan_input.state == "read"
        && actionable_gap_outcomes_input.state == "read"
        && attempt_ledger_input.state == "read"
    {
        actions.push(ripr_swarm_readiness_limited_runtime_action(runtime_status));
    }
    if swarm_plan_input.state != "read" {
        actions.push(RiprSwarmReadinessNextAction {
            kind: "refresh_swarm_plan".to_string(),
            packet_id: None,
            attempt_id: None,
            canonical_gap_id: None,
            evidence_class: None,
            repair_kind: None,
            command: Some("cargo xtask ripr-swarm plan --top 10".to_string()),
            reason: swarm_plan_input
                .limitation
                .as_deref()
                .unwrap_or("swarm-plan input is not readable")
                .to_string(),
        });
    }
    if actionable_gap_outcomes_input.state != "read" {
        actions.push(RiprSwarmReadinessNextAction {
            kind: "refresh_outcome_report".to_string(),
            packet_id: None,
            attempt_id: None,
            canonical_gap_id: None,
            evidence_class: None,
            repair_kind: None,
            command: Some("cargo xtask actionable-gap-outcomes".to_string()),
            reason: actionable_gap_outcomes_input
                .limitation
                .as_deref()
                .unwrap_or("actionable-gap outcome input is not readable")
                .to_string(),
        });
    }
    if attempt_ledger_input.state != "read" {
        actions.push(RiprSwarmReadinessNextAction {
            kind: "refresh_attempt_ledger".to_string(),
            packet_id: None,
            attempt_id: None,
            canonical_gap_id: None,
            evidence_class: None,
            repair_kind: None,
            command: Some("cargo xtask ripr-swarm attempt-ledger".to_string()),
            reason: attempt_ledger_input
                .limitation
                .as_deref()
                .unwrap_or("swarm-attempt-ledger input is not readable")
                .to_string(),
        });
    }
    if summary.not_actionable_gap_state > 0 {
        actions.push(RiprSwarmReadinessNextAction {
            kind: "inspect_non_actionable_gap_state".to_string(),
            packet_id: None,
            attempt_id: None,
            canonical_gap_id: None,
            evidence_class: None,
            repair_kind: None,
            command: Some("cargo xtask lane1-evidence-audit".to_string()),
            reason: format!(
                "{} packet(s) have non-actionable gap_state; keep them out of public repair queues and inspect the named limitation or advisory route",
                summary.not_actionable_gap_state
            ),
        });
    }
    if summary.missing_receipt_command > 0 {
        actions.push(RiprSwarmReadinessNextAction {
            kind: "fix_receipt_command_source".to_string(),
            packet_id: None,
            attempt_id: None,
            canonical_gap_id: None,
            evidence_class: None,
            repair_kind: None,
            command: Some("cargo xtask lane1-evidence-audit".to_string()),
            reason: format!(
                "{} packet(s) are missing receipt commands; repair canonical_item.receipt_command projection before attempting swarm work",
                summary.missing_receipt_command
            ),
        });
    }
    if summary.missing_verify_command > 0 {
        actions.push(RiprSwarmReadinessNextAction {
            kind: "fix_verify_command_source".to_string(),
            packet_id: None,
            attempt_id: None,
            canonical_gap_id: None,
            evidence_class: None,
            repair_kind: None,
            command: Some("cargo xtask lane1-evidence-audit".to_string()),
            reason: format!(
                "{} packet(s) are missing verify commands; improve canonical item verify routing before ranking them as repair-ready",
                summary.missing_verify_command
            ),
        });
    }
    if summary.missing_repair_route > 0 {
        actions.push(RiprSwarmReadinessNextAction {
            kind: "fix_repair_route_source".to_string(),
            packet_id: None,
            attempt_id: None,
            canonical_gap_id: None,
            evidence_class: None,
            repair_kind: None,
            command: Some("cargo xtask lane1-evidence-audit".to_string()),
            reason: format!(
                "{} packet(s) are missing structured repair routes; repair canonical item route projection before attempting swarm work",
                summary.missing_repair_route
            ),
        });
    }
    if summary.missing_repair_kind > 0 {
        actions.push(RiprSwarmReadinessNextAction {
            kind: "fix_repair_kind_source".to_string(),
            packet_id: None,
            attempt_id: None,
            canonical_gap_id: None,
            evidence_class: None,
            repair_kind: None,
            command: Some("cargo xtask lane1-evidence-audit".to_string()),
            reason: format!(
                "{} packet(s) are missing repair_kind; repair canonical item route-kind projection before attempting swarm work",
                summary.missing_repair_kind
            ),
        });
    }
    if summary.missing_related_test_or_observer > 0 {
        actions.push(RiprSwarmReadinessNextAction {
            kind: "fix_related_test_or_observer".to_string(),
            packet_id: None,
            attempt_id: None,
            canonical_gap_id: None,
            evidence_class: None,
            repair_kind: None,
            command: Some("cargo xtask lane1-evidence-audit".to_string()),
            reason: format!(
                "{} packet(s) are missing typed related test or observer context; repair target projection before attempting swarm work",
                summary.missing_related_test_or_observer
            ),
        });
    }
    if summary.missing_target_test_shape > 0 {
        actions.push(RiprSwarmReadinessNextAction {
            kind: "fix_target_test_shape".to_string(),
            packet_id: None,
            attempt_id: None,
            canonical_gap_id: None,
            evidence_class: None,
            repair_kind: None,
            command: Some("cargo xtask lane1-evidence-audit".to_string()),
            reason: format!(
                "{} packet(s) are missing target_test_shape repair/test shape; repair packet target-shape projection before attempting swarm work",
                summary.missing_target_test_shape
            ),
        });
    }
    if summary.missing_canonical_gap_id > 0 {
        actions.push(RiprSwarmReadinessNextAction {
            kind: "fix_canonical_gap_identity".to_string(),
            packet_id: None,
            attempt_id: None,
            canonical_gap_id: None,
            evidence_class: None,
            repair_kind: None,
            command: Some("cargo xtask lane1-evidence-audit".to_string()),
            reason: format!(
                "{} packet(s) are missing stable canonical_gap_id identity; repair canonical grouping before attempting swarm work",
                summary.missing_canonical_gap_id
            ),
        });
    }
    if summary.missing_must_not_change > 0 {
        actions.push(RiprSwarmReadinessNextAction {
            kind: "fix_must_not_change_boundaries".to_string(),
            packet_id: None,
            attempt_id: None,
            canonical_gap_id: None,
            evidence_class: None,
            repair_kind: None,
            command: Some("cargo xtask lane1-evidence-audit".to_string()),
            reason: format!(
                "{} packet(s) are missing must_not_change boundaries; repair packet constraints before attempting swarm work",
                summary.missing_must_not_change
            ),
        });
    }
    if summary.missing_allowed_edit_surface > 0 {
        actions.push(RiprSwarmReadinessNextAction {
            kind: "fix_allowed_edit_surface".to_string(),
            packet_id: None,
            attempt_id: None,
            canonical_gap_id: None,
            evidence_class: None,
            repair_kind: None,
            command: Some("cargo xtask lane1-evidence-audit".to_string()),
            reason: format!(
                "{} packet(s) are missing allowed_edit_surface edit cages; repair packet edit bounds before attempting swarm work",
                summary.missing_allowed_edit_surface
            ),
        });
    }
    if summary.missing_confidence > 0 {
        actions.push(RiprSwarmReadinessNextAction {
            kind: "fix_confidence_basis".to_string(),
            packet_id: None,
            attempt_id: None,
            canonical_gap_id: None,
            evidence_class: None,
            repair_kind: None,
            command: Some("cargo xtask lane1-evidence-audit".to_string()),
            reason: format!(
                "{} packet(s) are missing confidence basis; repair canonical item confidence projection before delegating repair work",
                summary.missing_confidence
            ),
        });
    }
    if summary.missing_raw_evidence_refs > 0 {
        actions.push(RiprSwarmReadinessNextAction {
            kind: "fix_raw_evidence_refs".to_string(),
            packet_id: None,
            attempt_id: None,
            canonical_gap_id: None,
            evidence_class: None,
            repair_kind: None,
            command: Some("cargo xtask lane1-evidence-audit".to_string()),
            reason: format!(
                "{} packet(s) are missing structured raw evidence references; repair evidence lineage before attempting swarm work",
                summary.missing_raw_evidence_refs
            ),
        });
    }
    if summary.blocked_by_missing_context_packets > 0 {
        actions.push(RiprSwarmReadinessNextAction {
            kind: "inspect_blocked_missing_context".to_string(),
            packet_id: None,
            attempt_id: None,
            canonical_gap_id: None,
            evidence_class: None,
            repair_kind: None,
            command: Some("cargo xtask lane1-evidence-audit".to_string()),
            reason: format!(
                "{} packet(s) are blocked by missing context; repair canonical packet fields before attempting swarm work",
                summary.blocked_by_missing_context_packets
            ),
        });
    }
    if summary.public_projection_excluded_packets > 0 {
        actions.push(RiprSwarmReadinessNextAction {
            kind: "inspect_public_projection_exclusions".to_string(),
            packet_id: None,
            attempt_id: None,
            canonical_gap_id: None,
            evidence_class: None,
            repair_kind: None,
            command: Some("cargo xtask lane1-evidence-audit".to_string()),
            reason: format!(
                "{} packet(s) are excluded from public projection; inspect projection_exclusion_reasons before attempting swarm work{}",
                summary.public_projection_excluded_packets,
                ripr_swarm_readiness_public_projection_exclusion_detail(summary)
            ),
        });
    }
    if summary.attempted_no_receipt_packets > 0 {
        let timeout_receipt_route = sources.top_failing_repair_routes.iter().find(|route| {
            route.attempted_no_receipt > 0
                && ripr_swarm_repair_route_quality_has_timeout_receipt(route)
        });
        if let Some(route) = timeout_receipt_route {
            actions.push(ripr_swarm_readiness_repair_route_quality_action(route));
        }
        if let Some(route) = sources.top_failing_repair_routes.iter().find(|route| {
            route.attempted_no_receipt > 0
                && !ripr_swarm_repair_route_quality_has_timeout_receipt(route)
        }) {
            actions.push(ripr_swarm_readiness_missing_receipt_action_from_route(
                route,
            ));
        } else if timeout_receipt_route.is_none() {
            let (packet_id, canonical_gap_id, repair_kind) =
                ripr_swarm_missing_evidence_field_sample(
                    sources.top_missing_evidence_fields,
                    "attempt_receipt",
                );
            actions.push(ripr_swarm_readiness_missing_receipt_action(
                summary.attempted_no_receipt_packets,
                packet_id,
                canonical_gap_id,
                repair_kind,
            ));
        }
    }
    if summary.missing_verify_result > 0 {
        let (packet_id, canonical_gap_id, repair_kind) = ripr_swarm_missing_evidence_field_sample(
            sources.top_missing_evidence_fields,
            "verify_result",
        );
        actions.push(RiprSwarmReadinessNextAction {
            kind: "inspect_missing_verify_results".to_string(),
            packet_id,
            attempt_id: None,
            canonical_gap_id,
            evidence_class: None,
            repair_kind,
            command: Some("cargo xtask ripr-swarm attempt-ledger".to_string()),
            reason: format!(
                "{} attempted packet(s) are missing typed verify_result evidence; preserve pass/fail/not-run from receipts or targeted-test outcomes before claiming route quality",
                summary.missing_verify_result
            ),
        });
    }
    if summary.receipt_present_packets > 0 {
        let (packet_id, canonical_gap_id, repair_kind) =
            ripr_swarm_attempt_ledger_outcome_sample(attempt_ledger_input.value, "receipt_present");
        actions.push(RiprSwarmReadinessNextAction {
            kind: "join_receipt_evidence_movement".to_string(),
            packet_id,
            attempt_id: None,
            canonical_gap_id,
            evidence_class: None,
            repair_kind,
            command: Some("cargo xtask actionable-gap-outcomes".to_string()),
            reason: format!(
                "{} receipt-backed packet(s) still need before/after evidence movement joined before route quality can claim improvement or regression",
                summary.receipt_present_packets
            ),
        });
    }
    if summary.orphaned_receipts > 0 {
        let (packet_id, canonical_gap_id, repair_kind) =
            ripr_swarm_attempt_ledger_orphaned_receipt_sample(attempt_ledger_input.value);
        actions.push(RiprSwarmReadinessNextAction {
            kind: "reconcile_orphaned_receipts".to_string(),
            packet_id,
            attempt_id: None,
            canonical_gap_id,
            evidence_class: None,
            repair_kind,
            command: Some("cargo xtask ripr-swarm attempt-ledger".to_string()),
            reason: format!(
                "{} receipt(s) did not match a current actionable packet; inspect receipt identity before using outcome counts",
                summary.orphaned_receipts
            ),
        });
    }
    if summary.regressed_packets > 0 {
        let (packet_id, canonical_gap_id, repair_kind) = ripr_swarm_attempt_ledger_outcome_sample(
            attempt_ledger_input.value,
            "evidence_regressed",
        );
        actions.push(RiprSwarmReadinessNextAction {
            kind: "inspect_regressed_attempts".to_string(),
            packet_id,
            attempt_id: None,
            canonical_gap_id,
            evidence_class: None,
            repair_kind,
            command: Some("cargo xtask ripr-swarm attempt-ledger".to_string()),
            reason: format!(
                "{} attempted packet(s) regressed evidence; inspect receipts and stop repeating that repair route",
                summary.regressed_packets
            ),
        });
    }
    if let Some(route) = sources.top_failing_repair_routes.first() {
        let action = ripr_swarm_readiness_repair_route_quality_action(route);
        if !ripr_swarm_readiness_has_next_action_packet(&actions, action.packet_id.as_deref()) {
            actions.push(action);
        }
    }
    let unexpected_unchanged_packets = summary
        .unchanged_packets
        .saturating_sub(summary.expected_unchanged_packets);
    if unexpected_unchanged_packets > 0 {
        let (packet_id, canonical_gap_id, repair_kind) = ripr_swarm_attempt_ledger_outcome_sample(
            attempt_ledger_input.value,
            "evidence_unchanged",
        );
        actions.push(RiprSwarmReadinessNextAction {
            kind: "inspect_unchanged_attempts".to_string(),
            packet_id,
            attempt_id: None,
            canonical_gap_id,
            evidence_class: None,
            repair_kind,
            command: Some("cargo xtask ripr-swarm attempt-ledger".to_string()),
            reason: format!(
                "{} attempted packet(s) left evidence unexpectedly unchanged; refine the repair route before retrying",
                unexpected_unchanged_packets
            ),
        });
    }
    if summary.static_limitation_packets > 0 {
        actions.push(RiprSwarmReadinessNextAction {
            kind: "route_static_limitations".to_string(),
            packet_id: None,
            attempt_id: None,
            canonical_gap_id: None,
            evidence_class: None,
            repair_kind: None,
            command: Some("cargo xtask lane1-evidence-audit".to_string()),
            reason: format!(
                "{} packet(s) are blocked by static limitations; route them to the Lane 1 analyzer backlog, not repair execution",
                summary.static_limitation_packets
            ),
        });
    }
    if summary.swarm_ready_packets == 0
        && let Some(route) = sources.top_limitation_routes.first()
    {
        let sample_packet = route.sample_packet_id.as_deref().unwrap_or("unknown");
        actions.push(RiprSwarmReadinessNextAction {
            kind: "route_static_limitation_backlog".to_string(),
            packet_id: route.sample_packet_id.clone(),
            attempt_id: None,
            canonical_gap_id: route.sample_canonical_gap_ids.first().cloned(),
            evidence_class: route.dominant_evidence_class.clone(),
            repair_kind: None,
            command: Some("cargo xtask lane1-evidence-audit".to_string()),
            reason: ripr_swarm_readiness_limitation_route_action_reason(route, sample_packet),
        });
    } else if summary.swarm_ready_packets == 0
        && let Some((category, count, repair_route)) =
            ripr_swarm_static_limitation_backlog_top_category(sources.static_limitation_backlog)
    {
        actions.push(RiprSwarmReadinessNextAction {
            kind: "route_static_limitation_backlog".to_string(),
            packet_id: None,
            attempt_id: None,
            canonical_gap_id: None,
            evidence_class: None,
            repair_kind: None,
            command: Some("cargo xtask lane1-evidence-audit".to_string()),
            reason: format!(
                "No swarm-ready packets are available; top static limitation `{category}` appears {count} time(s), route analyzer work to `{repair_route}` instead of attempting user test repairs"
            ),
        });
    }
    if defer_sampled_runtime_action
        && swarm_plan_input.state == "read"
        && actionable_gap_outcomes_input.state == "read"
        && attempt_ledger_input.state == "read"
    {
        actions.push(ripr_swarm_readiness_limited_runtime_action(runtime_status));
    }
    if let Some(plan) = sources.swarm_plan {
        if let Some(action) = ripr_swarm_readiness_operator_judgment_action(plan) {
            actions.push(action);
        }
        if !runtime_not_downstream_consumable {
            actions.extend(ripr_swarm_readiness_ready_packet_actions(plan));
        }
    }
    if actions.is_empty() {
        actions.push(RiprSwarmReadinessNextAction {
            kind: "no_ready_action".to_string(),
            packet_id: None,
            attempt_id: None,
            canonical_gap_id: None,
            evidence_class: None,
            repair_kind: None,
            command: None,
            reason: "no ready packets or blocking readiness issues were reported".to_string(),
        });
    }
    actions
}

pub(crate) fn ripr_swarm_readiness_limitation_route_action_reason(
    route: &RiprSwarmLimitationRouteRow,
    sample_packet: &str,
) -> String {
    let mut reason = format!(
        "No swarm-ready packets are available; top limitation route `{}` has {} signal(s), sample packet `{sample_packet}`",
        route.repair_route, route.signal_count
    );
    if let Some(subroute) = route.sample_limitation_subroute.as_deref() {
        reason.push_str(&format!(", subroute `{subroute}`"));
    }
    if let Some(why_not_actionable) = route.why_not_actionable.as_deref() {
        reason.push_str(&format!("; why not actionable: {why_not_actionable}"));
    }
    reason.push_str("; route analyzer work instead of attempting user test repairs");
    reason
}

pub(crate) fn ripr_swarm_readiness_runtime_is_sampled_work_queue(
    runtime_status: &Lane1RuntimeStatus,
) -> bool {
    runtime_status.state == "limited_sampled_input"
        && runtime_status
            .limitation_category
            .as_deref()
            .is_some_and(|category| category == "lane1_repo_exposure_sampled")
}

pub(crate) fn ripr_swarm_static_limitation_backlog_top_category(
    backlog: &Value,
) -> Option<(String, usize, String)> {
    audit_array(backlog, &["top_categories"])
        .first()
        .map(|row| {
            let category = audit_non_empty_string(row, &["category"])
                .or_else(|| audit_non_empty_string(row, &["label"]))
                .unwrap_or_else(|| "unknown".to_string());
            let count = audit_usize(row, &["count"]).unwrap_or_default();
            let repair_route = audit_non_empty_string(row, &["repair_route"])
                .unwrap_or_else(|| static_limitation_repair_route(&category).to_string());
            (category, count, repair_route)
        })
}

pub(crate) fn ripr_swarm_readiness_limited_runtime_action(
    runtime_status: &Lane1RuntimeStatus,
) -> RiprSwarmReadinessNextAction {
    let phase = runtime_status.phase.as_deref().unwrap_or("unknown_phase");
    let input_kind = runtime_status
        .input_kind
        .as_deref()
        .unwrap_or("unknown_input");
    let limitation_category = runtime_status
        .limitation_category
        .as_deref()
        .unwrap_or("unknown_limitation");
    let repair_route = runtime_status
        .repair_route
        .as_deref()
        .unwrap_or("inspect the limited input and regenerate the readiness source artifacts");
    RiprSwarmReadinessNextAction {
        kind: "resolve_limited_runtime_status".to_string(),
        packet_id: None,
        attempt_id: None,
        canonical_gap_id: None,
        evidence_class: None,
        repair_kind: None,
        command: ripr_swarm_readiness_limited_runtime_command(runtime_status),
        reason: format!(
            "readiness input `{input_kind}` is `{}` during `{phase}` with limitation `{limitation_category}`; repair route: {repair_route}",
            runtime_status.state
        ),
    }
}

pub(crate) fn ripr_swarm_readiness_limited_runtime_command(
    runtime_status: &Lane1RuntimeStatus,
) -> Option<String> {
    match runtime_status.limitation_category.as_deref()? {
        "lane1_repo_exposure_sampled"
        | "lane1_repo_exposure_incomplete"
        | "lane1_repo_exposure_timeout"
        | "lane1_repo_exposure_runner_error" => {
            Some("cargo xtask lane1-evidence-audit".to_string())
        }
        "lane1_repo_exposure_large_cache_preflight_skip" => {
            Some("cargo xtask cache report && cargo xtask cache gc --dry-run".to_string())
        }
        "lane1_repo_exposure_cache_store_skipped_large_entry" => {
            Some("cargo xtask cache report".to_string())
        }
        "swarm_plan_input_unavailable" => Some("cargo xtask ripr-swarm plan --top 10".to_string()),
        "actionable_gap_outcomes_input_unavailable" => {
            Some("cargo xtask actionable-gap-outcomes".to_string())
        }
        "swarm_attempt_ledger_input_unavailable" => {
            Some("cargo xtask ripr-swarm attempt-ledger".to_string())
        }
        _ => None,
    }
}

pub(crate) fn ripr_swarm_readiness_operator_judgment_action(
    swarm_plan: &Value,
) -> Option<RiprSwarmReadinessNextAction> {
    let blocked = audit_array(swarm_plan, &["top_blocked_packets"]);
    let mut operator_judgment_packets = blocked.iter().filter(|packet| {
        audit_non_empty_string(packet, &["swarm_state"]).as_deref()
            == Some("blocked_by_operator_judgment")
            || audit_array(packet, &["blocked_reasons"])
                .iter()
                .any(|reason| {
                    reason.as_str()
                        == Some("static_only_predicate_boundary_requires_operator_judgment")
                })
    });
    let first = operator_judgment_packets.next()?;
    let count = 1 + operator_judgment_packets.count();
    Some(RiprSwarmReadinessNextAction {
        kind: "route_operator_judgment_packets".to_string(),
        packet_id: audit_non_empty_string(first, &["packet_id"])
            .or_else(|| audit_non_empty_string(first, &["canonical_gap_id"])),
        attempt_id: None,
        canonical_gap_id: audit_non_empty_string(first, &["canonical_gap_id"]),
        evidence_class: audit_non_empty_string(first, &["evidence_class"]),
        repair_kind: audit_non_empty_string(first, &["repair_kind"]),
        command: Some("cargo xtask ripr-swarm plan --top 10".to_string()),
        reason: format!(
            "{count} top blocked packet(s) require operator judgment; improve upstream evidence confidence or choose a manual repair outside the default swarm-ready queue"
        ),
    })
}

pub(crate) fn ripr_swarm_readiness_ready_packet_actions(
    swarm_plan: &Value,
) -> Vec<RiprSwarmReadinessNextAction> {
    audit_array(swarm_plan, &["top_ready_packets"])
        .iter()
        .take(RIPR_SWARM_READINESS_NEXT_ACTION_PACKET_LIMIT)
        .map(|packet| {
            let packet_id = audit_non_empty_string(packet, &["packet_id"])
                .or_else(|| audit_non_empty_string(packet, &["canonical_gap_id"]));
            let canonical_gap_id = audit_non_empty_string(packet, &["canonical_gap_id"]);
            let evidence_class = audit_non_empty_string(packet, &["evidence_class"]);
            let repair_kind = audit_non_empty_string(packet, &["repair_kind"]);
            let command = packet_id
                .as_ref()
                .map(|id| format!("cargo xtask ripr-swarm attempt --packet {id} --dry-run"));
            RiprSwarmReadinessNextAction {
                kind: "attempt_ready_packet".to_string(),
                packet_id,
                attempt_id: None,
                canonical_gap_id,
                evidence_class,
                repair_kind,
                command,
                reason: "packet is queued with repair, verify, receipt, and no static limitation"
                    .to_string(),
            }
        })
        .collect()
}

pub(crate) fn ripr_swarm_readiness_next_actions_json(
    actions: &[RiprSwarmReadinessNextAction],
) -> Vec<Value> {
    actions
        .iter()
        .map(ripr_swarm_readiness_next_action_json)
        .collect()
}

pub(crate) fn ripr_swarm_readiness_next_action_json(
    action: &RiprSwarmReadinessNextAction,
) -> Value {
    serde_json::json!({
        "kind": action.kind,
        "packet_id": action.packet_id,
        "attempt_id": action.attempt_id,
        "canonical_gap_id": action.canonical_gap_id,
        "evidence_class": action.evidence_class,
        "repair_kind": action.repair_kind,
        "command": action.command,
        "reason": action.reason,
    })
}

pub(crate) fn ripr_swarm_readiness_markdown(report: &RiprSwarmReadinessReport) -> String {
    let summary = ripr_swarm_readiness_summary_json(&report.summary);
    let mut out = String::new();
    out.push_str("# RIPR Swarm Readiness\n\n");
    out.push_str(&format!(
        "Readiness state: `{}`\n\n",
        report.readiness_state
    ));
    out.push_str(&format!(
        "Run status: `{}`\n\n",
        report.runtime_status.state
    ));
    out.push_str(
        "Advisory roll-up over swarm-plan and actionable-gap-outcome artifacts. It does not execute repairs or consume raw findings as work.\n\n",
    );
    lane1_runtime_status_push_markdown(&mut out, &report.runtime_status);
    out.push_str("## Inputs\n\n");
    out.push_str("| Input | State | Path | Limitation |\n");
    out.push_str("| --- | --- | --- | --- |\n");
    out.push_str(&format!(
        "| swarm plan | `{}` | `{}` | {} |\n",
        audit_markdown_cell(&report.swarm_plan_state),
        audit_markdown_cell(&report.swarm_plan_path),
        audit_markdown_cell(report.swarm_plan_limitation.as_deref().unwrap_or(""))
    ));
    out.push_str(&format!(
        "| actionable gap outcomes | `{}` | `{}` | {} |\n",
        audit_markdown_cell(&report.actionable_gap_outcomes_state),
        audit_markdown_cell(&report.actionable_gap_outcomes_path),
        audit_markdown_cell(
            report
                .actionable_gap_outcomes_limitation
                .as_deref()
                .unwrap_or("")
        )
    ));
    out.push_str(&format!(
        "| attempt ledger | `{}` | `{}` | {} |\n\n",
        audit_markdown_cell(&report.attempt_ledger_state),
        audit_markdown_cell(&report.attempt_ledger_path),
        audit_markdown_cell(report.attempt_ledger_limitation.as_deref().unwrap_or(""))
    ));
    out.push_str("## Summary\n\n");
    out.push_str("| Metric | Count |\n");
    out.push_str("| --- | ---: |\n");
    for key in [
        "actionable_gaps_total",
        "public_projection_eligible_packets",
        "swarm_ready_packets",
        "blocked_packets",
        "blocked_by_missing_context_packets",
        "blocked_by_static_limitation_packets",
        "blocked_by_public_projection_exclusion_packets",
        "blocked_by_operator_judgment_packets",
        "public_projection_excluded_packets",
        "missing_canonical_gap_id",
        "not_actionable_gap_state",
        "missing_verify_command",
        "missing_verify_result",
        "missing_receipt_command",
        "missing_repair_kind",
        "missing_repair_route",
        "missing_target_test_shape",
        "missing_must_not_change",
        "missing_allowed_edit_surface",
        "missing_confidence",
        "missing_raw_evidence_refs",
        "missing_related_test_or_observer",
        "related_context_missing",
        "static_limitation_packets",
        "static_limitation_backlog_packets",
        "static_limitation_backlog_signals",
        "high_confidence_packets",
        "attempted_packets",
        "attempted_no_receipt_packets",
        "receipt_present_packets",
        "improved_packets",
        "unchanged_packets",
        "expected_unchanged_packets",
        "regressed_packets",
        "resolved_packets",
        "orphaned_receipts",
    ] {
        out.push_str(&format!(
            "| {} | {} |\n",
            key.replace('_', " "),
            summary[key].as_u64().unwrap_or(0)
        ));
    }
    out.push('\n');
    out.push_str("## Attempt History Summary\n\n");
    out.push_str("This table preserves durable attempt history from the swarm attempt ledger before readiness uses latest attempts for current routing counts and repair-route quality.\n\n");
    out.push_str("| Metric | Count |\n");
    out.push_str("| --- | ---: |\n");
    for (label, count) in [
        (
            "attempts_total",
            report.attempt_history_summary.attempts_total,
        ),
        (
            "durable_attempts_total",
            report.attempt_history_summary.durable_attempts_total,
        ),
        (
            "canonical_gaps_total",
            report.attempt_history_summary.canonical_gaps_total,
        ),
        (
            "not_attempted",
            report.attempt_history_summary.not_attempted,
        ),
        (
            "attempted_no_receipt",
            report.attempt_history_summary.attempted_no_receipt,
        ),
        (
            "receipt_present",
            report.attempt_history_summary.receipt_present,
        ),
        (
            "missing_verify_result",
            report.attempt_history_summary.missing_verify_result,
        ),
        (
            "evidence_improved",
            report.attempt_history_summary.evidence_improved,
        ),
        (
            "evidence_unchanged",
            report.attempt_history_summary.evidence_unchanged,
        ),
        (
            "expected_unchanged",
            report.attempt_history_summary.expected_unchanged,
        ),
        (
            "evidence_regressed",
            report.attempt_history_summary.evidence_regressed,
        ),
        ("resolved", report.attempt_history_summary.resolved),
        ("unknown", report.attempt_history_summary.unknown),
    ] {
        out.push_str(&format!("| {} | {} |\n", label.replace('_', " "), count));
    }
    out.push('\n');
    let public_projection_exclusion_reasons = summary["public_projection_exclusion_reasons"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    ripr_swarm_push_count_rows_markdown(
        &mut out,
        "Public Projection Exclusion Reasons",
        &public_projection_exclusion_reasons,
    );
    ripr_swarm_readiness_push_blocked_state_routes_table(&mut out, &report.blocked_state_routes);
    ripr_swarm_push_static_limitation_backlog_markdown(&mut out, &report.static_limitation_backlog);
    ripr_swarm_readiness_push_top_limitation_routes_table(&mut out, &report.top_limitation_routes);
    ripr_swarm_readiness_push_limitation_route_quality_table(
        &mut out,
        &report.top_limitation_routes,
    );
    cross_language_oracle_route_quality_push_markdown(
        &mut out,
        &report.cross_language_oracle_route_quality,
    );
    out.push_str("\n## Repair Route Quality\n\n");
    ripr_swarm_push_repair_route_quality_table(&mut out, &report.repair_route_quality);
    out.push_str("## Repair Route Quality By Language\n\n");
    ripr_swarm_push_repair_route_quality_table(&mut out, &report.language_repair_route_quality);
    out.push_str("## Repair Route Quality Backlog\n\n");
    ripr_swarm_push_repair_route_quality_backlog_table(&mut out, &report.top_failing_repair_routes);
    if !report.top_missing_evidence_fields.is_empty() {
        out.push_str("## Top Missing Evidence Fields\n\n");
        ripr_swarm_push_missing_evidence_fields_table(
            &mut out,
            &report.top_missing_evidence_fields,
        );
    }
    out.push_str("\n## Top Next Action\n\n");
    if let Some(action) = report.next_actions.first() {
        ripr_swarm_readiness_push_next_actions_table(&mut out, std::slice::from_ref(action));
    } else {
        out.push_str("No next action is available.\n\n");
    }
    out.push_str("\n## Next Actions\n\n");
    ripr_swarm_readiness_push_next_actions_table(&mut out, &report.next_actions);
    out.push_str("\n## Must Not Infer\n\n");
    out.push_str(
        "- Readiness reports summarize existing swarm artifacts; they do not execute repairs.\n",
    );
    out.push_str("- Raw findings remain supporting evidence, not swarm work.\n");
    out.push_str("- Missing outcome artifacts mean no outcome join is available, not that attempts failed.\n");
    out.push_str(
        "- Repair-route quality is an analyzer improvement signal, not a public badge basis.\n",
    );
    out.push_str("- `top_next_action` is a projection of `next_actions[0]`, not a separate ranking source.\n");
    out.push_str("- Readiness counts do not change public badge semantics.\n");
    out.push_str("- Static limitations and blocked packets are not repair-ready work.\n");
    out
}

pub(crate) fn ripr_swarm_readiness_push_blocked_state_routes_table(
    out: &mut String,
    routes: &[RiprSwarmReadinessBlockedStateRoute],
) {
    out.push_str("## Blocked State Routes\n\n");
    out.push_str("| State | Count | Example packet | Example gap | Example receipt | Next action | Repair route | Reason |\n");
    out.push_str("| --- | ---: | --- | --- | --- | --- | --- | --- |\n");
    for route in routes {
        out.push_str(&format!(
            "| `{}` | {} | `{}` | `{}` | `{}` | `{}` | `{}` | {} |\n",
            audit_markdown_cell(&route.state),
            route.count,
            audit_markdown_cell(route.example_packet_id.as_deref().unwrap_or("unknown")),
            audit_markdown_cell(
                route
                    .example_canonical_gap_id
                    .as_deref()
                    .unwrap_or("unknown")
            ),
            audit_markdown_cell(route.example_receipt_path.as_deref().unwrap_or("n/a")),
            audit_markdown_cell(&route.next_action_kind),
            audit_markdown_cell(&route.repair_route),
            audit_markdown_cell(&route.reason),
        ));
    }
    if routes.is_empty() {
        out.push_str("| none | 0 |  |  |  |  |  | no blocked packet states reported |\n");
    }
    out.push('\n');
}

pub(crate) fn ripr_swarm_readiness_push_top_limitation_routes_table(
    out: &mut String,
    rows: &[RiprSwarmLimitationRouteRow],
) {
    if rows.is_empty() {
        return;
    }
    out.push_str("\n## Top Limitation Routes\n\n");
    out.push_str(
        "Limitation routes are analyzer backlog signals, not swarm-ready repair work.\n\n",
    );
    out.push_str("| Repair route | Signals | Sample packet | Category | Subroute | Dominant class | Sample sources | Why not actionable | Unlock condition | Non-claims |\n");
    out.push_str("| --- | ---: | --- | --- | --- | --- | --- | --- | --- | --- |\n");
    for row in rows {
        out.push_str(&format!(
            "| `{}` | {} | `{}` | `{}` | `{}` | `{}` | {} | {} | {} | {} |\n",
            audit_markdown_cell(&row.repair_route),
            row.signal_count,
            audit_markdown_cell(row.sample_packet_id.as_deref().unwrap_or("unknown")),
            audit_markdown_cell(
                row.sample_limitation_category
                    .as_deref()
                    .unwrap_or("unknown")
            ),
            audit_markdown_cell(
                row.sample_limitation_subroute
                    .as_deref()
                    .unwrap_or("unknown")
            ),
            audit_markdown_cell(row.dominant_evidence_class.as_deref().unwrap_or("unknown")),
            audit_markdown_cell(&ripr_swarm_limitation_route_sample_sources_markdown(row)),
            audit_markdown_cell(
                row.why_not_actionable.as_deref().unwrap_or(
                    "static evidence is insufficient to provide a bounded repair packet"
                )
            ),
            audit_markdown_cell(
                row.unlock_condition
                    .as_deref()
                    .unwrap_or("inspect the analyzer route before attempting repairs")
            ),
            audit_markdown_cell(&row.non_claims.join(", "))
        ));
    }
    out.push('\n');
}

pub(crate) fn ripr_swarm_readiness_push_limitation_route_quality_table(
    out: &mut String,
    rows: &[RiprSwarmLimitationRouteRow],
) {
    out.push_str("\n## Limitation Route Quality\n\n");
    if rows.is_empty() {
        out.push_str("No limitation-route quality rows are available.\n\n");
        return;
    }
    out.push_str("Limitation-route quality summarizes analyzer backlog routes. These rows are not repair attempts, not public repair packets, and not badge or gate inputs.\n\n");
    out.push_str("| Repair route | Signals | Route state | Language-aware status | Navigation-only target status | Packet policy | Sample packet | Sample sources | Unlock condition |\n");
    out.push_str("| --- | ---: | --- | --- | --- | --- | --- | --- | --- |\n");
    for row in rows {
        out.push_str(&format!(
            "| `{}` | {} | `non_actionable_limitation` | `{}` | `{}` | `not_public_repair_packet` | `{}` | {} | {} |\n",
            audit_markdown_cell(&row.repair_route),
            row.signal_count,
            audit_markdown_cell(ripr_swarm_limitation_route_language_aware_status(row)),
            audit_markdown_cell(ripr_swarm_limitation_route_navigation_only_target_status(row)),
            audit_markdown_cell(row.sample_packet_id.as_deref().unwrap_or("unknown")),
            audit_markdown_cell(&ripr_swarm_limitation_route_sample_sources_markdown(row)),
            audit_markdown_cell(
                row.unlock_condition
                    .as_deref()
                    .unwrap_or("inspect the analyzer route before attempting repairs")
            ),
        ));
    }
    out.push('\n');
}

pub(crate) fn ripr_swarm_limitation_route_sample_sources_markdown(
    row: &RiprSwarmLimitationRouteRow,
) -> String {
    if row.sample_sources.is_empty() {
        return "unknown".to_string();
    }
    row.sample_sources
        .iter()
        .map(|sample| {
            let gap = sample.canonical_gap_id.as_deref().unwrap_or("unknown");
            let location = sample
                .line
                .map(|line| format!("{}:{line}", sample.source_file))
                .unwrap_or_else(|| sample.source_file.clone());
            let expression = sample.expression.as_deref().unwrap_or("unknown expression");
            let reason = sample
                .limitation_reason
                .as_deref()
                .unwrap_or("unknown limitation");
            format!("{location} ({gap}; {expression}; {reason})")
        })
        .collect::<Vec<_>>()
        .join("<br>")
}

pub(crate) fn ripr_swarm_readiness_push_next_actions_table(
    out: &mut String,
    actions: &[RiprSwarmReadinessNextAction],
) {
    out.push_str("| Kind | Packet | Attempt | Evidence class | Repair | Command | Reason |\n");
    out.push_str("| --- | --- | --- | --- | --- | --- | --- |\n");
    for action in actions {
        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} | {} |\n",
            audit_markdown_cell(&action.kind),
            audit_markdown_cell(
                action
                    .packet_id
                    .as_deref()
                    .or(action.canonical_gap_id.as_deref())
                    .unwrap_or("")
            ),
            audit_markdown_cell(action.attempt_id.as_deref().unwrap_or("")),
            audit_markdown_cell(action.evidence_class.as_deref().unwrap_or("")),
            audit_markdown_cell(action.repair_kind.as_deref().unwrap_or("")),
            audit_markdown_cell(action.command.as_deref().unwrap_or("")),
            audit_markdown_cell(&action.reason),
        ));
    }
}
