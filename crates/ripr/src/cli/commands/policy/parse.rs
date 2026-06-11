use crate::cli::commands_options::*;
use crate::output;
use std::path::PathBuf;

use super::super::{non_empty_path_arg, non_empty_string_arg};

pub(crate) fn parse_policy_readiness_options(
    args: &[String],
) -> Result<PolicyReadinessOptions, String> {
    let mut root = ".".to_string();
    let mut gate_decision = None;
    let mut baseline_delta = None;
    let mut recommendation_calibration = None;
    let mut mutation_calibration = None;
    let mut waiver_aging = None;
    let mut suppression_health = None;
    let mut repo_config = None;
    let mut previous_readiness = None;
    let mut out = PathBuf::from(output::policy_readiness::DEFAULT_POLICY_READINESS_OUT);
    let mut out_md = PathBuf::from(output::policy_readiness::DEFAULT_POLICY_READINESS_MD_OUT);

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = non_empty_string_arg(args, i, "--root", "policy readiness")?;
            }
            "--gate-decision" => {
                i += 1;
                gate_decision = Some(non_empty_path_arg(
                    args,
                    i,
                    "--gate-decision",
                    "policy readiness",
                )?);
            }
            "--baseline-delta" => {
                i += 1;
                baseline_delta = Some(non_empty_path_arg(
                    args,
                    i,
                    "--baseline-delta",
                    "policy readiness",
                )?);
            }
            "--recommendation-calibration" => {
                i += 1;
                recommendation_calibration = Some(non_empty_path_arg(
                    args,
                    i,
                    "--recommendation-calibration",
                    "policy readiness",
                )?);
            }
            "--mutation-calibration" => {
                i += 1;
                mutation_calibration = Some(non_empty_path_arg(
                    args,
                    i,
                    "--mutation-calibration",
                    "policy readiness",
                )?);
            }
            "--waiver-aging" => {
                i += 1;
                waiver_aging = Some(non_empty_path_arg(
                    args,
                    i,
                    "--waiver-aging",
                    "policy readiness",
                )?);
            }
            "--suppression-health" => {
                i += 1;
                suppression_health = Some(non_empty_path_arg(
                    args,
                    i,
                    "--suppression-health",
                    "policy readiness",
                )?);
            }
            "--repo-config" => {
                i += 1;
                repo_config = Some(non_empty_path_arg(
                    args,
                    i,
                    "--repo-config",
                    "policy readiness",
                )?);
            }
            "--previous-readiness" => {
                i += 1;
                previous_readiness = Some(non_empty_path_arg(
                    args,
                    i,
                    "--previous-readiness",
                    "policy readiness",
                )?);
            }
            "--out" => {
                i += 1;
                out = non_empty_path_arg(args, i, "--out", "policy readiness")?;
            }
            "--out-md" => {
                i += 1;
                out_md = non_empty_path_arg(args, i, "--out-md", "policy readiness")?;
            }
            other => return Err(format!("unknown policy readiness argument {other:?}")),
        }
        i += 1;
    }

    Ok(PolicyReadinessOptions {
        root,
        gate_decision,
        baseline_delta,
        recommendation_calibration,
        mutation_calibration,
        waiver_aging,
        suppression_health,
        repo_config,
        previous_readiness,
        out,
        out_md,
    })
}

pub(crate) fn parse_policy_operations_options(
    args: &[String],
) -> Result<PolicyOperationsOptions, String> {
    let mut root = ".".to_string();
    let mut policy_readiness = None;
    let mut waiver_aging = None;
    let mut suppression_health = None;
    let mut baseline_delta = None;
    let mut gate_decision = None;
    let mut recommendation_calibration = None;
    let mut mutation_calibration = None;
    let mut preview_boundary = None;
    let mut out = PathBuf::from(output::policy_operations::DEFAULT_POLICY_OPERATIONS_OUT);
    let mut out_md = PathBuf::from(output::policy_operations::DEFAULT_POLICY_OPERATIONS_MD_OUT);

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = non_empty_string_arg(args, i, "--root", "policy operations")?;
            }
            "--policy-readiness" => {
                i += 1;
                policy_readiness = Some(non_empty_path_arg(
                    args,
                    i,
                    "--policy-readiness",
                    "policy operations",
                )?);
            }
            "--waiver-aging" => {
                i += 1;
                waiver_aging = Some(non_empty_path_arg(
                    args,
                    i,
                    "--waiver-aging",
                    "policy operations",
                )?);
            }
            "--suppression-health" => {
                i += 1;
                suppression_health = Some(non_empty_path_arg(
                    args,
                    i,
                    "--suppression-health",
                    "policy operations",
                )?);
            }
            "--baseline-delta" => {
                i += 1;
                baseline_delta = Some(non_empty_path_arg(
                    args,
                    i,
                    "--baseline-delta",
                    "policy operations",
                )?);
            }
            "--gate-decision" => {
                i += 1;
                gate_decision = Some(non_empty_path_arg(
                    args,
                    i,
                    "--gate-decision",
                    "policy operations",
                )?);
            }
            "--recommendation-calibration" => {
                i += 1;
                recommendation_calibration = Some(non_empty_path_arg(
                    args,
                    i,
                    "--recommendation-calibration",
                    "policy operations",
                )?);
            }
            "--mutation-calibration" => {
                i += 1;
                mutation_calibration = Some(non_empty_path_arg(
                    args,
                    i,
                    "--mutation-calibration",
                    "policy operations",
                )?);
            }
            "--preview-boundary" => {
                i += 1;
                preview_boundary = Some(non_empty_path_arg(
                    args,
                    i,
                    "--preview-boundary",
                    "policy operations",
                )?);
            }
            "--out" => {
                i += 1;
                out = non_empty_path_arg(args, i, "--out", "policy operations")?;
            }
            "--out-md" => {
                i += 1;
                out_md = non_empty_path_arg(args, i, "--out-md", "policy operations")?;
            }
            other => return Err(format!("unknown policy operations argument {other:?}")),
        }
        i += 1;
    }

    let policy_readiness = policy_readiness
        .ok_or_else(|| "policy operations requires --policy-readiness <path>".to_string())?;

    Ok(PolicyOperationsOptions {
        root,
        policy_readiness: Some(policy_readiness),
        waiver_aging,
        suppression_health,
        baseline_delta,
        gate_decision,
        recommendation_calibration,
        mutation_calibration,
        preview_boundary,
        out,
        out_md,
    })
}

pub(crate) fn parse_policy_history_options(
    args: &[String],
) -> Result<PolicyHistoryOptions, String> {
    let mut root = ".".to_string();
    let mut current = None;
    let mut history = None;
    let mut commit = None;
    let mut pr_number = None;
    let mut out = PathBuf::from(output::policy_history::DEFAULT_POLICY_HISTORY_OUT);
    let mut out_md = PathBuf::from(output::policy_history::DEFAULT_POLICY_HISTORY_MD_OUT);

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = non_empty_string_arg(args, i, "--root", "policy history")?;
            }
            "--current" => {
                i += 1;
                current = Some(non_empty_path_arg(args, i, "--current", "policy history")?);
            }
            "--history" => {
                i += 1;
                history = Some(non_empty_path_arg(args, i, "--history", "policy history")?);
            }
            "--commit" => {
                i += 1;
                commit = Some(non_empty_string_arg(args, i, "--commit", "policy history")?);
            }
            "--pr-number" => {
                i += 1;
                pr_number = Some(non_empty_string_arg(
                    args,
                    i,
                    "--pr-number",
                    "policy history",
                )?);
            }
            "--out" => {
                i += 1;
                out = non_empty_path_arg(args, i, "--out", "policy history")?;
            }
            "--out-md" => {
                i += 1;
                out_md = non_empty_path_arg(args, i, "--out-md", "policy history")?;
            }
            other => return Err(format!("unknown policy history argument {other:?}")),
        }
        i += 1;
    }

    Ok(PolicyHistoryOptions {
        root,
        current: current.ok_or_else(|| "policy history requires --current <path>".to_string())?,
        history,
        commit,
        pr_number,
        out,
        out_md,
    })
}

pub(crate) fn parse_policy_promotion_options(
    args: &[String],
) -> Result<PolicyPromotionOptions, String> {
    let mut root = ".".to_string();
    let mut target_mode = None;
    let mut operations = None;
    let mut history = None;
    let mut out = None;
    let mut out_md = None;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = non_empty_string_arg(args, i, "--root", "policy promote")?;
            }
            "--to" => {
                i += 1;
                target_mode = Some(non_empty_string_arg(args, i, "--to", "policy promote")?);
            }
            "--operations" => {
                i += 1;
                operations = Some(non_empty_path_arg(
                    args,
                    i,
                    "--operations",
                    "policy promote",
                )?);
            }
            "--history" => {
                i += 1;
                history = Some(non_empty_path_arg(args, i, "--history", "policy promote")?);
            }
            "--out" => {
                i += 1;
                out = Some(non_empty_path_arg(args, i, "--out", "policy promote")?);
            }
            "--out-md" => {
                i += 1;
                out_md = Some(non_empty_path_arg(args, i, "--out-md", "policy promote")?);
            }
            other => return Err(format!("unknown policy promote argument {other:?}")),
        }
        i += 1;
    }

    let target_mode =
        target_mode.ok_or_else(|| "policy promote requires --to <mode>".to_string())?;
    if !output::policy_promotion::is_supported_target_mode(&target_mode) {
        return Err(format!(
            "unknown policy promotion target {target_mode:?}; expected `visible-only`, `acknowledgeable`, `baseline-check`, or `calibrated-gate`"
        ));
    }
    let operations =
        operations.ok_or_else(|| "policy promote requires --operations <path>".to_string())?;
    let out = out.unwrap_or_else(|| {
        PathBuf::from(output::policy_promotion::default_policy_promotion_out(
            &target_mode,
        ))
    });
    let out_md = out_md.unwrap_or_else(|| {
        PathBuf::from(output::policy_promotion::default_policy_promotion_md_out(
            &target_mode,
        ))
    });

    Ok(PolicyPromotionOptions {
        root,
        target_mode,
        operations,
        history,
        out,
        out_md,
    })
}

pub(crate) fn parse_policy_preview_promotion_options(
    args: &[String],
) -> Result<PolicyPreviewPromotionOptions, String> {
    let mut root = ".".to_string();
    let mut language = None;
    let mut candidate_class = None;
    let mut evidence = None;
    let mut out = None;
    let mut out_md = None;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = non_empty_string_arg(args, i, "--root", "policy preview-promote")?;
            }
            "--language" => {
                i += 1;
                language = Some(non_empty_string_arg(
                    args,
                    i,
                    "--language",
                    "policy preview-promote",
                )?);
            }
            "--class" => {
                i += 1;
                candidate_class = Some(non_empty_string_arg(
                    args,
                    i,
                    "--class",
                    "policy preview-promote",
                )?);
            }
            "--evidence" => {
                i += 1;
                evidence = Some(non_empty_path_arg(
                    args,
                    i,
                    "--evidence",
                    "policy preview-promote",
                )?);
            }
            "--out" => {
                i += 1;
                out = Some(non_empty_path_arg(
                    args,
                    i,
                    "--out",
                    "policy preview-promote",
                )?);
            }
            "--out-md" => {
                i += 1;
                out_md = Some(non_empty_path_arg(
                    args,
                    i,
                    "--out-md",
                    "policy preview-promote",
                )?);
            }
            other => {
                return Err(format!("unknown policy preview-promote argument {other:?}"));
            }
        }
        i += 1;
    }

    let language = language
        .ok_or_else(|| "policy preview-promote requires --language <language>".to_string())?;
    if !output::policy_preview_promotion::is_supported_language(&language) {
        return Err(format!(
            "unknown preview promotion language {language:?}; expected `typescript` or `python`"
        ));
    }
    let candidate_class = candidate_class
        .ok_or_else(|| "policy preview-promote requires --class <class>".to_string())?;
    let out = out.unwrap_or_else(|| {
        PathBuf::from(
            output::policy_preview_promotion::default_preview_promotion_out(
                &language,
                &candidate_class,
            ),
        )
    });
    let out_md = out_md.unwrap_or_else(|| {
        PathBuf::from(
            output::policy_preview_promotion::default_preview_promotion_md_out(
                &language,
                &candidate_class,
            ),
        )
    });

    Ok(PolicyPreviewPromotionOptions {
        root,
        language,
        candidate_class,
        evidence,
        out,
        out_md,
    })
}

pub(crate) fn parse_policy_waiver_aging_options(
    args: &[String],
) -> Result<PolicyWaiverAgingOptions, String> {
    let mut root = ".".to_string();
    let mut ledger = None;
    let mut history = None;
    let mut out = PathBuf::from(output::waiver_aging::DEFAULT_WAIVER_AGING_OUT);
    let mut out_md = PathBuf::from(output::waiver_aging::DEFAULT_WAIVER_AGING_MD_OUT);

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = non_empty_string_arg(args, i, "--root", "policy waiver-aging")?;
            }
            "--ledger" => {
                i += 1;
                ledger = Some(non_empty_path_arg(
                    args,
                    i,
                    "--ledger",
                    "policy waiver-aging",
                )?);
            }
            "--history" => {
                i += 1;
                history = Some(non_empty_path_arg(
                    args,
                    i,
                    "--history",
                    "policy waiver-aging",
                )?);
            }
            "--out" => {
                i += 1;
                out = non_empty_path_arg(args, i, "--out", "policy waiver-aging")?;
            }
            "--out-md" => {
                i += 1;
                out_md = non_empty_path_arg(args, i, "--out-md", "policy waiver-aging")?;
            }
            other => return Err(format!("unknown policy waiver-aging argument {other:?}")),
        }
        i += 1;
    }

    Ok(PolicyWaiverAgingOptions {
        root,
        ledger,
        history,
        out,
        out_md,
    })
}

pub(crate) fn parse_policy_suppression_health_options(
    args: &[String],
) -> Result<PolicySuppressionHealthOptions, String> {
    let mut root = PathBuf::from(".");
    let mut manifest = PathBuf::from(output::suppressions::SUPPRESSIONS_PATH);
    let mut out = PathBuf::from(output::suppression_health::DEFAULT_SUPPRESSION_HEALTH_OUT);
    let mut out_md = PathBuf::from(output::suppression_health::DEFAULT_SUPPRESSION_HEALTH_MD_OUT);

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = non_empty_path_arg(args, i, "--root", "policy suppression-health")?;
            }
            "--manifest" => {
                i += 1;
                manifest = non_empty_path_arg(args, i, "--manifest", "policy suppression-health")?;
            }
            "--out" => {
                i += 1;
                out = non_empty_path_arg(args, i, "--out", "policy suppression-health")?;
            }
            "--out-md" => {
                i += 1;
                out_md = non_empty_path_arg(args, i, "--out-md", "policy suppression-health")?;
            }
            other => {
                return Err(format!(
                    "unknown policy suppression-health argument {other:?}"
                ));
            }
        }
        i += 1;
    }

    Ok(PolicySuppressionHealthOptions {
        root,
        manifest,
        out,
        out_md,
    })
}
