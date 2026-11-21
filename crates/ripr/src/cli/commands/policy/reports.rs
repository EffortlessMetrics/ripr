use crate::output;
use std::path::Path;

use super::super::{
    policy_readiness_generated_at, read_optional_manifest_for_report,
    read_optional_text_for_report, write_text_file,
};
use super::parse::{
    parse_policy_history_options, parse_policy_operations_options,
    parse_policy_preview_promotion_options, parse_policy_promotion_options,
    parse_policy_readiness_options, parse_policy_suppression_health_options,
    parse_policy_waiver_aging_options,
};

fn write_policy_report_files(
    out: &Path,
    out_md: &Path,
    rendered_json: &str,
    rendered_md: &str,
) -> Result<(), String> {
    write_text_file(out, rendered_json)?;
    write_text_file(out_md, rendered_md)?;
    println!("Wrote {}", out.display());
    println!("Wrote {}", out_md.display());
    Ok(())
}

pub(crate) fn policy_readiness(args: &[String]) -> Result<(), String> {
    let options = parse_policy_readiness_options(args)?;
    let input = output::policy_readiness::PolicyReadinessInput {
        root: options.root,
        generated_at: policy_readiness_generated_at()?,
        gate_decision_path: options
            .gate_decision
            .as_ref()
            .map(|path| output::policy_readiness::display_path(path)),
        baseline_delta_path: options
            .baseline_delta
            .as_ref()
            .map(|path| output::policy_readiness::display_path(path)),
        recommendation_calibration_path: options
            .recommendation_calibration
            .as_ref()
            .map(|path| output::policy_readiness::display_path(path)),
        mutation_calibration_path: options
            .mutation_calibration
            .as_ref()
            .map(|path| output::policy_readiness::display_path(path)),
        waiver_aging_path: options
            .waiver_aging
            .as_ref()
            .map(|path| output::policy_readiness::display_path(path)),
        suppression_health_path: options
            .suppression_health
            .as_ref()
            .map(|path| output::policy_readiness::display_path(path)),
        repo_config_path: options
            .repo_config
            .as_ref()
            .map(|path| output::policy_readiness::display_path(path)),
        previous_readiness_path: options
            .previous_readiness
            .as_ref()
            .map(|path| output::policy_readiness::display_path(path)),
        gate_decision_json: options
            .gate_decision
            .as_ref()
            .map(|path| read_optional_text_for_report("gate decision", path)),
        baseline_delta_json: options
            .baseline_delta
            .as_ref()
            .map(|path| read_optional_text_for_report("baseline debt delta", path)),
        recommendation_calibration_json: options
            .recommendation_calibration
            .as_ref()
            .map(|path| read_optional_text_for_report("recommendation calibration", path)),
        mutation_calibration_json: options
            .mutation_calibration
            .as_ref()
            .map(|path| read_optional_text_for_report("mutation calibration", path)),
        waiver_aging_json: options
            .waiver_aging
            .as_ref()
            .map(|path| read_optional_text_for_report("waiver aging", path)),
        suppression_health_json: options
            .suppression_health
            .as_ref()
            .map(|path| read_optional_text_for_report("suppression health", path)),
        repo_config_json: options
            .repo_config
            .as_ref()
            .map(|path| read_optional_text_for_report("repo config summary", path)),
        previous_readiness_json: options
            .previous_readiness
            .as_ref()
            .map(|path| read_optional_text_for_report("previous policy readiness", path)),
    };
    let report = output::policy_readiness::build_policy_readiness_report(input);
    let rendered_json = output::policy_readiness::render_policy_readiness_json(&report)?;
    let rendered_md = output::policy_readiness::render_policy_readiness_markdown(&report);
    write_policy_report_files(&options.out, &options.out_md, &rendered_json, &rendered_md)?;
    println!(
        "Status: {}",
        output::policy_readiness::policy_readiness_status(&report)
    );
    println!(
        "Recommended mode: {}",
        output::policy_readiness::policy_readiness_recommended_mode(&report)
    );
    Ok(())
}

pub(crate) fn policy_operations(args: &[String]) -> Result<(), String> {
    let options = parse_policy_operations_options(args)?;
    let input = output::policy_operations::PolicyOperationsInput {
        root: options.root,
        generated_at: policy_readiness_generated_at()?,
        policy_readiness_path: options
            .policy_readiness
            .as_ref()
            .map(|path| output::policy_operations::display_path(path)),
        waiver_aging_path: options
            .waiver_aging
            .as_ref()
            .map(|path| output::policy_operations::display_path(path)),
        suppression_health_path: options
            .suppression_health
            .as_ref()
            .map(|path| output::policy_operations::display_path(path)),
        baseline_delta_path: options
            .baseline_delta
            .as_ref()
            .map(|path| output::policy_operations::display_path(path)),
        gate_decision_path: options
            .gate_decision
            .as_ref()
            .map(|path| output::policy_operations::display_path(path)),
        recommendation_calibration_path: options
            .recommendation_calibration
            .as_ref()
            .map(|path| output::policy_operations::display_path(path)),
        mutation_calibration_path: options
            .mutation_calibration
            .as_ref()
            .map(|path| output::policy_operations::display_path(path)),
        preview_boundary_path: options
            .preview_boundary
            .as_ref()
            .map(|path| output::policy_operations::display_path(path)),
        policy_readiness_json: options
            .policy_readiness
            .as_ref()
            .map(|path| read_optional_text_for_report("policy readiness", path)),
        waiver_aging_json: options
            .waiver_aging
            .as_ref()
            .map(|path| read_optional_text_for_report("waiver aging", path)),
        suppression_health_json: options
            .suppression_health
            .as_ref()
            .map(|path| read_optional_text_for_report("suppression health", path)),
        baseline_delta_json: options
            .baseline_delta
            .as_ref()
            .map(|path| read_optional_text_for_report("baseline debt delta", path)),
        gate_decision_json: options
            .gate_decision
            .as_ref()
            .map(|path| read_optional_text_for_report("gate decision", path)),
        recommendation_calibration_json: options
            .recommendation_calibration
            .as_ref()
            .map(|path| read_optional_text_for_report("recommendation calibration", path)),
        mutation_calibration_json: options
            .mutation_calibration
            .as_ref()
            .map(|path| read_optional_text_for_report("mutation calibration", path)),
        preview_boundary_json: options
            .preview_boundary
            .as_ref()
            .map(|path| read_optional_text_for_report("preview boundary", path)),
    };
    let report = output::policy_operations::build_policy_operations_report(input);
    let rendered_json = output::policy_operations::render_policy_operations_json(&report)?;
    let rendered_md = output::policy_operations::render_policy_operations_markdown(&report);
    write_policy_report_files(&options.out, &options.out_md, &rendered_json, &rendered_md)?;
    println!(
        "Current ceiling: {}",
        output::policy_operations::policy_operations_current_ceiling(&report)
    );
    println!(
        "Next safe action: {}",
        output::policy_operations::policy_operations_next_action(&report)
    );
    Ok(())
}

pub(crate) fn policy_history(args: &[String]) -> Result<(), String> {
    let options = parse_policy_history_options(args)?;
    let input = output::policy_history::PolicyHistoryInput {
        root: options.root,
        generated_at: policy_readiness_generated_at()?,
        current_path: output::policy_history::display_path(&options.current),
        history_path: options
            .history
            .as_ref()
            .map(|path| output::policy_history::display_path(path)),
        commit: options.commit,
        pr_number: options.pr_number,
        current_json: read_optional_text_for_report("policy operations", &options.current),
        history_jsonl: options
            .history
            .as_ref()
            .map(|path| read_optional_text_for_report("policy history", path)),
    };
    let report = output::policy_history::build_policy_history_report(input);
    let rendered_json = output::policy_history::render_policy_history_json(&report)?;
    let rendered_md = output::policy_history::render_policy_history_markdown(&report);
    write_policy_report_files(&options.out, &options.out_md, &rendered_json, &rendered_md)?;
    println!(
        "Current ceiling: {}",
        output::policy_history::policy_history_current_ceiling(&report)
    );
    println!(
        "Readiness trend: {}",
        output::policy_history::policy_history_trend_direction(&report)
    );
    Ok(())
}

pub(crate) fn policy_promotion(args: &[String]) -> Result<(), String> {
    let options = parse_policy_promotion_options(args)?;
    let input = output::policy_promotion::PolicyPromotionInput {
        root: options.root,
        generated_at: policy_readiness_generated_at()?,
        target_mode: options.target_mode,
        operations_path: output::policy_promotion::display_path(&options.operations),
        history_path: options
            .history
            .as_ref()
            .map(|path| output::policy_promotion::display_path(path)),
        operations_json: read_optional_text_for_report("policy operations", &options.operations),
        history_json: options
            .history
            .as_ref()
            .map(|path| read_optional_text_for_report("policy history", path)),
    };
    let report = output::policy_promotion::build_policy_promotion_report(input);
    let rendered_json = output::policy_promotion::render_policy_promotion_json(&report)?;
    let rendered_md = output::policy_promotion::render_policy_promotion_markdown(&report);
    write_policy_report_files(&options.out, &options.out_md, &rendered_json, &rendered_md)?;
    println!(
        "Allowed now: {}",
        if output::policy_promotion::policy_promotion_allowed_now(&report) {
            "yes"
        } else {
            "no"
        }
    );
    Ok(())
}

pub(crate) fn policy_preview_promotion(args: &[String]) -> Result<(), String> {
    let options = parse_policy_preview_promotion_options(args)?;
    let input = output::policy_preview_promotion::PreviewPromotionInput {
        root: options.root,
        generated_at: policy_readiness_generated_at()?,
        language: options.language,
        candidate_class: options.candidate_class,
        evidence_path: options
            .evidence
            .as_ref()
            .map(|path| output::policy_preview_promotion::display_path(path)),
        evidence_json: options
            .evidence
            .as_ref()
            .map(|path| read_optional_text_for_report("preview promotion evidence", path)),
    };
    let report = output::policy_preview_promotion::build_preview_promotion_report(input);
    let rendered_json = output::policy_preview_promotion::render_preview_promotion_json(&report)?;
    let rendered_md = output::policy_preview_promotion::render_preview_promotion_markdown(&report);
    write_policy_report_files(&options.out, &options.out_md, &rendered_json, &rendered_md)?;
    println!(
        "Allowed now: {}",
        if output::policy_preview_promotion::preview_promotion_allowed_now(&report) {
            "yes"
        } else {
            "no"
        }
    );
    Ok(())
}

pub(crate) fn policy_waiver_aging(args: &[String]) -> Result<(), String> {
    let options = parse_policy_waiver_aging_options(args)?;
    let input = output::waiver_aging::WaiverAgingInput {
        root: options.root,
        generated_at: policy_readiness_generated_at()?,
        ledger_path: options
            .ledger
            .as_ref()
            .map(|path| output::waiver_aging::display_path(path)),
        history_path: options
            .history
            .as_ref()
            .map(|path| output::waiver_aging::display_path(path)),
        ledger_json: options
            .ledger
            .as_ref()
            .map(|path| read_optional_text_for_report("PR evidence ledger", path)),
        history_json: options
            .history
            .as_ref()
            .map(|path| read_optional_text_for_report("PR evidence ledger history", path)),
    };
    let report = output::waiver_aging::build_waiver_aging_report(input);
    let rendered_json = output::waiver_aging::render_waiver_aging_json(&report)?;
    let rendered_md = output::waiver_aging::render_waiver_aging_markdown(&report);
    write_policy_report_files(&options.out, &options.out_md, &rendered_json, &rendered_md)?;
    println!(
        "Status: {}",
        output::waiver_aging::waiver_aging_status(&report)
    );
    Ok(())
}

pub(crate) fn policy_suppression_health(args: &[String]) -> Result<(), String> {
    let options = parse_policy_suppression_health_options(args)?;
    let input = output::suppression_health::SuppressionHealthInput {
        root: output::suppression_health::display_path(&options.root),
        generated_at: policy_readiness_generated_at()?,
        today: output::suppressions::current_iso_date(),
        manifest_path: output::suppression_health::display_path(&options.manifest),
        manifest_text: read_optional_manifest_for_report(&options.root, &options.manifest),
    };
    let report = output::suppression_health::build_suppression_health_report(input);
    let rendered_json = output::suppression_health::render_suppression_health_json(&report)?;
    let rendered_md = output::suppression_health::render_suppression_health_markdown(&report);
    write_policy_report_files(&options.out, &options.out_md, &rendered_json, &rendered_md)?;
    println!(
        "Status: {}",
        output::suppression_health::suppression_health_status(&report)
    );
    Ok(())
}
