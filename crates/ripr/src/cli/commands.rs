use crate::agent::{loop_commands, provenance};
use crate::analysis;
use crate::app::agent_brief::{
    AgentBriefChangedOwner, AgentBriefLine, AgentBriefPolicy, AgentBriefResolvedWorkingSet,
    select_agent_brief_seams,
};
use crate::app::{self, CheckInput, Mode, OutputFormat};
use crate::cli::agent::{
    AgentBriefOptions, AgentBriefWorkingSet, AgentCommand, AgentPacketOptions, AgentReceiptOptions,
    AgentReviewSummaryOptions, AgentStartOptions, AgentStatusOptions, AgentVerifyOptions,
    parse_agent_args,
};
use crate::cli::help;
use crate::cli::init_workflow::generated_github_actions_workflow;
use crate::cli::parse::{expect_value, parse_format, parse_mode};
use crate::config::{
    CONFIG_FILE_NAME, CheckInputExplicit, DEFAULT_LSP_SEAM_DIAGNOSTICS, apply_to_check_input,
    config_fingerprint, generated_init_config, load_for_root,
};
use crate::output;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DEFAULT_PILOT_TIMEOUT_MS: u64 = 30_000;

#[derive(Debug, PartialEq, Eq)]
struct InitOptions {
    root: PathBuf,
    dry_run: bool,
    force: bool,
    ci: Option<InitCi>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum InitCi {
    Github,
}

#[derive(Debug, PartialEq, Eq)]
struct PilotOptions {
    root: PathBuf,
    out_dir: PathBuf,
    mode: Mode,
    explicit: CheckInputExplicit,
    max_seams: usize,
    timeout_ms: u64,
}

#[derive(Debug, PartialEq, Eq)]
struct OutcomeOptions {
    before: PathBuf,
    after: PathBuf,
    format: OutcomeFormat,
    out: Option<PathBuf>,
}

#[derive(Debug, PartialEq, Eq)]
struct EvidenceHealthOptions {
    root: PathBuf,
    out: PathBuf,
    out_md: PathBuf,
    mutation_calibration: Option<PathBuf>,
}

#[derive(Debug, PartialEq, Eq)]
struct ReviewCommentsOptions {
    root: PathBuf,
    base: String,
    head: String,
    out: PathBuf,
}

#[derive(Debug, PartialEq, Eq)]
struct GateOptions {
    input: output::gate::GateEvaluateInput,
    out: PathBuf,
    out_md: PathBuf,
}

#[derive(Debug, PartialEq, Eq)]
struct BaselineCreateOptions {
    from: PathBuf,
    out: PathBuf,
    dry_run: bool,
    force: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct BaselineDiffOptions {
    baseline: PathBuf,
    current: PathBuf,
    out: PathBuf,
    out_md: PathBuf,
}

#[derive(Debug, PartialEq, Eq)]
struct BaselineUpdateOptions {
    baseline: PathBuf,
    current: PathBuf,
    out: Option<PathBuf>,
    remove_resolved: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct RiprZeroStatusOptions {
    baseline: Option<PathBuf>,
    delta: PathBuf,
    gate: Option<PathBuf>,
    pr_guidance: Option<PathBuf>,
    recommendation_calibration: Option<PathBuf>,
    out: PathBuf,
    out_md: PathBuf,
}

#[derive(Debug, PartialEq, Eq)]
struct PrEvidenceLedgerOptions {
    pr_number: String,
    base: String,
    head: String,
    labels: Vec<String>,
    gate: Option<PathBuf>,
    baseline_delta: Option<PathBuf>,
    zero_status: Option<PathBuf>,
    pr_guidance: Option<PathBuf>,
    recommendation_calibration: Option<PathBuf>,
    agent_receipt: Option<PathBuf>,
    coverage: Option<PathBuf>,
    history: Option<PathBuf>,
    out: PathBuf,
    out_md: PathBuf,
}

#[derive(Debug, PartialEq, Eq)]
struct PrCommentsPlanOptions {
    root: String,
    pr_guidance: Option<PathBuf>,
    existing_comments: Option<PathBuf>,
    mode: output::pr_inline_comment_publish_plan::CommentMode,
    pull_request: Option<u64>,
    event_name: Option<String>,
    head_repo: Option<String>,
    base_repo: Option<String>,
    token_available: bool,
    write_permission: bool,
    max_inline_comments: usize,
    out: PathBuf,
    out_md: PathBuf,
}

#[derive(Debug, PartialEq, Eq)]
struct PrReviewFrontPanelOptions {
    root: String,
    pr_guidance: Option<PathBuf>,
    first_action: Option<PathBuf>,
    assistant_proof: Option<PathBuf>,
    assistant_health: Option<PathBuf>,
    ledger: Option<PathBuf>,
    baseline_delta: Option<PathBuf>,
    zero_status: Option<PathBuf>,
    gate_decision: Option<PathBuf>,
    recommendation_calibration: Option<PathBuf>,
    mutation_calibration: Option<PathBuf>,
    coverage_frontier: Option<PathBuf>,
    receipt: Option<PathBuf>,
    out: PathBuf,
    out_md: PathBuf,
}

#[derive(Debug, PartialEq, Eq)]
struct ReportPacketIndexOptions {
    root: String,
    reports_dir: PathBuf,
    review_dir: PathBuf,
    receipts_dir: PathBuf,
    workflow_dir: PathBuf,
    agent_dir: PathBuf,
    pilot_dir: PathBuf,
    ci_dir: PathBuf,
    out: PathBuf,
    out_md: PathBuf,
}

#[derive(Debug, PartialEq, Eq)]
struct CoverageGripFrontierOptions {
    coverage: Option<PathBuf>,
    ledger: Option<PathBuf>,
    baseline_delta: Option<PathBuf>,
    zero_status: Option<PathBuf>,
    out: PathBuf,
    out_md: PathBuf,
}

#[derive(Debug, PartialEq, Eq)]
struct AssistantLoopProofOptions {
    root: String,
    pr_guidance: Option<PathBuf>,
    agent_packet: Option<PathBuf>,
    before: Option<PathBuf>,
    after: Option<PathBuf>,
    receipt: Option<PathBuf>,
    ledger: Option<PathBuf>,
    coverage_frontier: Option<PathBuf>,
    gate_decision: Option<PathBuf>,
    out: PathBuf,
    out_md: PathBuf,
}

#[derive(Debug, PartialEq, Eq)]
struct AssistantLoopHealthOptions {
    root: String,
    proofs: Vec<PathBuf>,
    out: PathBuf,
    out_md: PathBuf,
}

#[derive(Debug, PartialEq, Eq)]
struct FirstActionOptions {
    root: String,
    pr_guidance: Option<PathBuf>,
    assistant_proof: Option<PathBuf>,
    ledger: Option<PathBuf>,
    baseline_delta: Option<PathBuf>,
    receipt: Option<PathBuf>,
    gate_decision: Option<PathBuf>,
    coverage_frontier: Option<PathBuf>,
    editor_context: Option<PathBuf>,
    out: PathBuf,
    out_md: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum OutcomeFormat {
    Markdown,
    Json,
}

#[derive(Debug, PartialEq, Eq)]
struct CalibrateOptions {
    mutants_json: PathBuf,
    repo_exposure_json: PathBuf,
    format: CalibrateFormat,
    out: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum CalibrateFormat {
    Markdown,
    Json,
}

pub(super) fn agent(args: &[String]) -> Result<(), String> {
    match parse_agent_args(args)? {
        AgentCommand::Help => {
            help::print_agent_help();
            Ok(())
        }
        AgentCommand::StartHelp => {
            help::print_agent_start_help();
            Ok(())
        }
        AgentCommand::BriefHelp => {
            help::print_agent_brief_help();
            Ok(())
        }
        AgentCommand::PacketHelp => {
            help::print_agent_packet_help();
            Ok(())
        }
        AgentCommand::VerifyHelp => {
            help::print_agent_verify_help();
            Ok(())
        }
        AgentCommand::ReceiptHelp => {
            help::print_agent_receipt_help();
            Ok(())
        }
        AgentCommand::StatusHelp => {
            help::print_agent_status_help();
            Ok(())
        }
        AgentCommand::ReviewSummaryHelp => {
            help::print_agent_review_summary_help();
            Ok(())
        }
        AgentCommand::Start(options) => run_agent_start(options),
        AgentCommand::Brief(options) => run_agent_brief(options),
        AgentCommand::Packet(options) => run_agent_packet(options),
        AgentCommand::Verify(options) => run_agent_verify(options),
        AgentCommand::Receipt(options) => run_agent_receipt(options),
        AgentCommand::Status(options) => run_agent_status(options),
        AgentCommand::ReviewSummary(options) => run_agent_review_summary(options),
    }
}

fn run_agent_start(options: AgentStartOptions) -> Result<(), String> {
    if !options.root.is_dir() {
        return Err(format!(
            "agent start root {} is not a directory",
            options.root.display()
        ));
    }

    let config = load_for_root(&options.root)?;
    let mut input = CheckInput {
        root: options.root.clone(),
        ..CheckInput::default()
    };
    apply_to_check_input(&mut input, &config, CheckInputExplicit::default());

    let working_set = AgentBriefResolvedWorkingSet::seam_id(options.seam_id.clone());
    let classified = analysis::inventory_classified_seams_at_with_config(&input.root, &config)?;
    let selection = select_agent_brief_seams(
        &classified,
        &working_set,
        1,
        AgentBriefPolicy::from_config(&config),
    );
    if selection.top_seams.is_empty() {
        return Err(format!(
            "agent start seam_id {} was not found or is hidden by config",
            options.seam_id
        ));
    }

    let out_dir = resolve_agent_start_out_dir(&input.root, &options.out_dir);
    std::fs::create_dir_all(&out_dir)
        .map_err(|err| format!("create {} failed: {err}", out_dir.display()))?;

    let agent_brief_json = output::agent_brief::render_agent_brief_json(
        &input.root,
        &input.mode,
        &config,
        &working_set,
        &selection,
    )?;
    let agent_brief_path = out_dir.join("agent-brief.json");
    write_text_file(&agent_brief_path, &agent_brief_json)?;

    let manifest = app::agent_workflow::build_agent_workflow_manifest(
        &input.root,
        &options.root,
        &input.mode,
        &options.out_dir,
        &options.seam_id,
        &agent_brief_json,
    )?;
    let workflow_json = output::agent_workflow::render_agent_workflow_json(&manifest)?;
    let commands_md = output::agent_workflow::render_agent_workflow_commands_md(&manifest);
    let workflow_path = out_dir.join("workflow.json");
    let commands_path = out_dir.join("commands.md");
    write_text_file(&workflow_path, &workflow_json)?;
    write_text_file(&commands_path, &commands_md)?;

    println!("Wrote {}", workflow_path.display());
    println!("Wrote {}", commands_path.display());
    println!("Wrote {}", agent_brief_path.display());
    if let Some(next) = manifest.missing_inputs.first() {
        println!("Next: {}", next.command);
    }
    Ok(())
}

fn run_agent_brief(options: AgentBriefOptions) -> Result<(), String> {
    if !options.root.is_dir() {
        return Err(format!(
            "agent brief root {} is not a directory",
            options.root.display()
        ));
    }

    let config = load_for_root(&options.root)?;
    let mut input = CheckInput {
        root: options.root.clone(),
        ..CheckInput::default()
    };
    apply_to_check_input(&mut input, &config, CheckInputExplicit::default());

    let working_set = resolve_agent_brief_working_set(&input.root, &options.working_set)?;
    let classified = analysis::inventory_classified_seams_at_with_config(&input.root, &config)?;
    let selection = select_agent_brief_seams(
        &classified,
        &working_set,
        options.max_seams,
        AgentBriefPolicy::from_config(&config),
    );
    let rendered = output::agent_brief::render_agent_brief_json(
        &input.root,
        &input.mode,
        &config,
        &working_set,
        &selection,
    )?;
    println!("{rendered}");
    Ok(())
}

fn run_agent_packet(options: AgentPacketOptions) -> Result<(), String> {
    if !options.root.is_dir() {
        return Err(format!(
            "agent packet root {} is not a directory",
            options.root.display()
        ));
    }

    let config = load_for_root(&options.root)?;
    let classified = analysis::inventory_classified_seams_at_with_config(&options.root, &config)?;
    let entry = classified
        .iter()
        .find(|entry| entry.seam.id().as_str() == options.seam_id)
        .ok_or_else(|| format!("agent packet seam_id {} was not found", options.seam_id))?;

    let policy = AgentBriefPolicy::from_config(&config);
    if let Some(reason) = policy.omission_reason_for_class(entry.class) {
        return Err(format!("agent packet seam_id {} {reason}", options.seam_id));
    }

    let rendered = output::agent_seam_packets::render_agent_seam_packet_json(entry);
    println!("{rendered}");
    Ok(())
}

fn run_agent_verify(options: AgentVerifyOptions) -> Result<(), String> {
    let before_path =
        validate_agent_verify_snapshot_path(&options.root, &options.before, "--before")?;
    let after_path = validate_agent_verify_snapshot_path(&options.root, &options.after, "--after")?;
    let before_json = read_agent_verify_snapshot(&before_path, "before")?;
    let after_json = read_agent_verify_snapshot(&after_path, "after")?;
    let report = output::outcome::targeted_test_outcome_report_from_json(
        &before_json,
        &after_json,
        output::outcome::display_path(&options.before),
        output::outcome::display_path(&options.after),
    )?;
    let rendered = output::outcome::render_agent_verify_json(&report)?;
    println!("{rendered}");
    Ok(())
}

fn run_agent_receipt(options: AgentReceiptOptions) -> Result<(), String> {
    if !options.root.is_dir() {
        return Err(format!(
            "agent receipt root {} is not a directory",
            options.root.display()
        ));
    }

    let verify_path = validate_agent_receipt_verify_path(&options.root, &options.verify_json)?;
    let verify_json = std::fs::read_to_string(&verify_path).map_err(|err| {
        format!(
            "read agent receipt verify JSON {} failed: {err}",
            output::outcome::display_path(&verify_path)
        )
    })?;
    let input_paths = output::agent_receipt::agent_receipt_input_paths(&verify_json)?;
    let provenance = build_agent_receipt_provenance(
        &options.root,
        &options.verify_json,
        &verify_path,
        &input_paths,
    )?;
    let rendered = output::agent_receipt::render_agent_receipt_json(
        &verify_json,
        output::outcome::display_path(&options.verify_json),
        &options.seam_id,
        options.test_changed.as_deref(),
        &options.commands_run,
        provenance,
    )?;

    match options.out {
        Some(path) => {
            if let Some(parent) = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
            {
                std::fs::create_dir_all(parent)
                    .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
            }
            std::fs::write(&path, rendered).map_err(|err| {
                format!(
                    "write {} failed: {err}",
                    output::outcome::display_path(&path)
                )
            })
        }
        None => {
            print!("{rendered}");
            Ok(())
        }
    }
}

fn run_agent_status(options: AgentStatusOptions) -> Result<(), String> {
    if !options.root.is_dir() {
        return Err(format!(
            "agent status root {} is not a directory",
            options.root.display()
        ));
    }

    let report = app::agent_status::build_agent_status_report(&options.root, &options.root);
    if options.json {
        let rendered = app::agent_status::render_agent_status_json(&report)?;
        print!("{rendered}");
    } else {
        let rendered = app::agent_status::render_agent_status_markdown(&report);
        print!("{rendered}");
    }
    Ok(())
}

fn run_agent_review_summary(options: AgentReviewSummaryOptions) -> Result<(), String> {
    if !options.root.is_dir() {
        return Err(format!(
            "agent review-summary root {} is not a directory",
            options.root.display()
        ));
    }

    let report =
        app::agent_review_summary::build_agent_review_summary_report(&options.root, &options.root);
    if options.json {
        let rendered = app::agent_review_summary::render_agent_review_summary_json(&report)?;
        print!("{rendered}");
    } else {
        let rendered = app::agent_review_summary::render_agent_review_summary_markdown(&report);
        print!("{rendered}");
    }
    Ok(())
}

fn resolve_agent_start_out_dir(root: &Path, out_dir: &Path) -> PathBuf {
    if out_dir.is_absolute() {
        out_dir.to_path_buf()
    } else {
        root.join(out_dir)
    }
}

fn write_text_file(path: &Path, rendered: &str) -> Result<(), String> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
    }
    std::fs::write(path, rendered).map_err(|err| {
        format!(
            "write {} failed: {err}",
            output::outcome::display_path(path)
        )
    })
}

fn validate_agent_receipt_verify_path(root: &Path, path: &Path) -> Result<PathBuf, String> {
    let root = root.canonicalize().map_err(|err| {
        format!(
            "canonicalize agent receipt root {} failed: {err}",
            root.display()
        )
    })?;
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let candidate = candidate.canonicalize().map_err(|err| {
        format!(
            "canonicalize agent receipt --verify-json {} failed: {err}",
            path.display()
        )
    })?;

    if !candidate.starts_with(&root) {
        return Err(format!(
            "agent receipt --verify-json {} must stay under root {}",
            path.display(),
            root.display()
        ));
    }

    Ok(candidate)
}

fn build_agent_receipt_provenance(
    root: &Path,
    verify_display_path: &Path,
    verify_path: &Path,
    input_paths: &output::agent_receipt::AgentReceiptInputPaths,
) -> Result<output::agent_receipt::AgentReceiptProvenance, String> {
    let before_artifact = agent_receipt_artifact_provenance(
        root,
        &input_paths.before,
        "before artifact",
        "before_artifact",
    )?;
    let after_artifact = agent_receipt_artifact_provenance(
        root,
        &input_paths.after,
        "after artifact",
        "after_artifact",
    )?;
    let verify_artifact = output::agent_receipt::AgentReceiptArtifactProvenance {
        path: output::outcome::display_path(verify_display_path),
        sha256: provenance::sha256_file(verify_path)?,
    };

    Ok(output::agent_receipt::AgentReceiptProvenance {
        ripr_version: env!("CARGO_PKG_VERSION").to_string(),
        repo_root: output::outcome::display_path(root),
        config_fingerprint: agent_receipt_config_fingerprint(root)?,
        command_template_version: loop_commands::AGENT_LOOP_COMMAND_TEMPLATE_VERSION.to_string(),
        generated_at: agent_receipt_generated_at()?,
        workflow_artifact: None,
        before_artifact,
        after_artifact,
        verify_artifact,
    })
}

fn agent_receipt_artifact_provenance(
    root: &Path,
    display_path: &str,
    role: &str,
    output_name: &str,
) -> Result<output::agent_receipt::AgentReceiptArtifactProvenance, String> {
    let resolved = validate_agent_receipt_artifact_path(root, Path::new(display_path), role)?;
    Ok(output::agent_receipt::AgentReceiptArtifactProvenance {
        path: display_path.replace('\\', "/"),
        sha256: provenance::sha256_file(&resolved).map_err(|err| {
            format!(
                "hash agent receipt {output_name} {} failed: {err}",
                display_path
            )
        })?,
    })
}

fn validate_agent_receipt_artifact_path(
    root: &Path,
    path: &Path,
    role: &str,
) -> Result<PathBuf, String> {
    let root = root.canonicalize().map_err(|err| {
        format!(
            "canonicalize agent receipt root {} failed: {err}",
            root.display()
        )
    })?;
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let candidate = candidate.canonicalize().map_err(|err| {
        format!(
            "canonicalize agent receipt {role} {} failed: {err}",
            path.display()
        )
    })?;

    if !candidate.starts_with(&root) {
        return Err(format!(
            "agent receipt {role} {} must stay under root {}",
            path.display(),
            root.display()
        ));
    }

    Ok(candidate)
}

fn agent_receipt_config_fingerprint(root: &Path) -> Result<Option<String>, String> {
    let path = root.join(CONFIG_FILE_NAME);
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(Some(config_fingerprint(&text))),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(format!("read {} failed: {err}", path.display())),
    }
}

fn agent_receipt_generated_at() -> Result<String, String> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("system clock before unix epoch: {err}"))?
        .as_millis();
    Ok(format!("unix_ms:{millis}"))
}

fn validate_agent_verify_snapshot_path(
    root: &Path,
    path: &Path,
    flag: &str,
) -> Result<PathBuf, String> {
    let root = root.canonicalize().map_err(|err| {
        format!(
            "canonicalize agent verify root {} failed: {err}",
            root.display()
        )
    })?;
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let candidate = candidate.canonicalize().map_err(|err| {
        format!(
            "canonicalize agent verify {flag} {} failed: {err}",
            path.display()
        )
    })?;

    if !candidate.starts_with(&root) {
        return Err(format!(
            "agent verify {flag} {} must stay under root {}",
            path.display(),
            root.display()
        ));
    }

    Ok(candidate)
}

fn read_agent_verify_snapshot(path: &Path, label: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|err| {
        format!(
            "read agent verify {label} snapshot {} failed: {err}",
            output::outcome::display_path(path)
        )
    })
}

fn resolve_agent_brief_working_set(
    root: &Path,
    working_set: &AgentBriefWorkingSet,
) -> Result<AgentBriefResolvedWorkingSet, String> {
    match working_set {
        AgentBriefWorkingSet::Diff(path) => {
            let diff_path = validate_agent_brief_diff_path(root, path)?;
            let diff_text = analysis::load_diff(root, None, Some(&diff_path))?;
            let changed_lines = agent_brief_lines_from_diff(root, &diff_text);
            let changed_owners = agent_brief_owners_for_lines(root, &changed_lines);
            Ok(AgentBriefResolvedWorkingSet::diff(
                path.clone(),
                changed_lines,
            ))
            .map(|working_set| working_set.with_changed_owners(changed_owners))
        }
        AgentBriefWorkingSet::Base(base) => {
            let diff_text = analysis::load_diff(root, Some(base.as_str()), None)?;
            let changed_lines = agent_brief_lines_from_diff(root, &diff_text);
            let changed_owners = agent_brief_owners_for_lines(root, &changed_lines);
            Ok(AgentBriefResolvedWorkingSet::base(
                base.clone(),
                changed_lines,
            ))
            .map(|working_set| working_set.with_changed_owners(changed_owners))
        }
        AgentBriefWorkingSet::Files(files) => Ok(AgentBriefResolvedWorkingSet::files(
            files
                .iter()
                .map(|file| normalize_agent_brief_path(root, file))
                .collect(),
        )),
        AgentBriefWorkingSet::SeamId(seam_id) => {
            Ok(AgentBriefResolvedWorkingSet::seam_id(seam_id.clone()))
        }
    }
}

fn validate_agent_brief_diff_path(root: &Path, path: &Path) -> Result<PathBuf, String> {
    let root = root.canonicalize().map_err(|err| {
        format!(
            "canonicalize agent brief root {} failed: {err}",
            root.display()
        )
    })?;
    let candidate = if path.is_absolute() || path.exists() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let candidate = candidate.canonicalize().map_err(|err| {
        format!(
            "canonicalize agent brief diff {} failed: {err}",
            path.display()
        )
    })?;

    if !candidate.starts_with(&root) {
        return Err(format!(
            "agent brief --diff {} must stay under root {}",
            path.display(),
            root.display()
        ));
    }

    Ok(candidate)
}

fn agent_brief_lines_from_diff(root: &Path, diff_text: &str) -> Vec<AgentBriefLine> {
    analysis::parse_unified_diff(diff_text)
        .into_iter()
        .flat_map(|file| {
            let path = normalize_agent_brief_path(root, &file.path);
            file.added_lines
                .into_iter()
                .map(move |line| AgentBriefLine::new(path.clone(), line.line))
        })
        .collect()
}

fn agent_brief_owners_for_lines(
    root: &Path,
    lines: &[AgentBriefLine],
) -> Vec<AgentBriefChangedOwner> {
    let owner_inputs = lines
        .iter()
        .map(|line| (line.file.clone(), line.line))
        .collect::<Vec<_>>();
    let Ok(owners) = analysis::owner_symbols_for_lines(root, &owner_inputs) else {
        return Vec::new();
    };

    owners
        .into_iter()
        .map(|owner| AgentBriefChangedOwner::new(owner.file, owner.line, owner.owner))
        .collect()
}

fn normalize_agent_brief_path(root: &Path, path: &Path) -> PathBuf {
    let path_text = normalized_path_text(path);
    for root_text in normalized_root_prefixes(root) {
        let prefix = format!("{root_text}/");
        if let Some(stripped) = path_text.strip_prefix(&prefix) {
            return PathBuf::from(stripped);
        }
    }
    PathBuf::from(path_text)
}

fn normalized_root_prefixes(root: &Path) -> Vec<String> {
    let mut prefixes = Vec::new();
    push_unique_normalized_path(&mut prefixes, root);
    if let Ok(root) = std::path::absolute(root) {
        push_unique_normalized_path(&mut prefixes, &root);
    }
    if let Ok(root) = root.canonicalize() {
        push_unique_normalized_path(&mut prefixes, &root);
    }
    prefixes
}

fn push_unique_normalized_path(prefixes: &mut Vec<String>, path: &Path) {
    let text = normalized_path_text(path);
    if !text.is_empty() && !prefixes.iter().any(|existing| existing == &text) {
        prefixes.push(text);
    }
}

fn normalized_path_text(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    text.strip_prefix("./").unwrap_or(&text).to_string()
}

pub(super) fn init(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_init_help();
        return Ok(());
    }
    let options = parse_init_options(args)?;
    if options.dry_run {
        print_init_dry_run(&options);
        return Ok(());
    }
    if !options.root.is_dir() {
        return Err(format!(
            "init root {} is not a directory",
            options.root.display()
        ));
    }
    let config_path = options.root.join(CONFIG_FILE_NAME);
    let workflow_path = options
        .ci
        .as_ref()
        .map(|ci| init_ci_workflow_path(&options.root, ci));

    if config_path.exists() && !options.force && options.ci.is_none() {
        return Err(format!(
            "{} already exists; rerun `ripr init --force` to overwrite it",
            config_path.display()
        ));
    }
    if let Some(path) = workflow_path
        .as_ref()
        .filter(|path| path.exists())
        .filter(|_| !options.force)
    {
        return Err(format!(
            "{} already exists; rerun `ripr init --ci github --force` to overwrite it",
            path.display()
        ));
    }

    if config_path.exists() && !options.force {
        println!("Left existing {} unchanged", config_path.display());
    } else {
        std::fs::write(&config_path, generated_init_config())
            .map_err(|err| format!("write {} failed: {err}", config_path.display()))?;
        println!("Wrote {}", config_path.display());
    }

    if let Some(ci) = options.ci.as_ref() {
        write_init_ci_workflow(&options.root, ci)?;
    }
    Ok(())
}

fn parse_init_options(args: &[String]) -> Result<InitOptions, String> {
    let mut options = InitOptions {
        root: PathBuf::from("."),
        dry_run: false,
        force: false,
        ci: None,
    };
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                options.root = PathBuf::from(expect_value(args, i, "--root")?);
            }
            "--ci" => {
                i += 1;
                options.ci = Some(parse_init_ci(expect_value(args, i, "--ci")?)?);
            }
            "--dry-run" => options.dry_run = true,
            "--force" => options.force = true,
            other => return Err(format!("unknown init argument {other:?}")),
        }
        i += 1;
    }
    Ok(options)
}

fn parse_init_ci(value: &str) -> Result<InitCi, String> {
    match value {
        "github" => Ok(InitCi::Github),
        _ => Err(format!("unknown init --ci provider {value:?}")),
    }
}

fn print_init_dry_run(options: &InitOptions) {
    if let Some(ci) = options.ci.as_ref() {
        println!("# {}", CONFIG_FILE_NAME);
        print!("{}", generated_init_config());
        println!();
        println!("# {}", init_ci_workflow_path(&options.root, ci).display());
        print!("{}", generated_github_actions_workflow());
    } else {
        print!("{}", generated_init_config());
    }
}

fn init_ci_workflow_path(root: &Path, ci: &InitCi) -> PathBuf {
    match ci {
        InitCi::Github => root.join(".github/workflows/ripr.yml"),
    }
}

fn write_init_ci_workflow(root: &Path, ci: &InitCi) -> Result<(), String> {
    match ci {
        InitCi::Github => {
            let path = init_ci_workflow_path(root, ci);
            if let Some(parent) = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
            {
                std::fs::create_dir_all(parent)
                    .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
            }
            std::fs::write(&path, generated_github_actions_workflow())
                .map_err(|err| format!("write {} failed: {err}", path.display()))?;
            println!("Wrote {}", path.display());
            Ok(())
        }
    }
}

pub(super) fn pilot(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_pilot_help();
        return Ok(());
    }

    let options = parse_pilot_options(args)?;
    if !options.root.is_dir() {
        return Err(format!(
            "pilot root {} is not a directory",
            options.root.display()
        ));
    }

    let config = load_for_root(&options.root)?;
    let mut input = CheckInput {
        root: options.root.clone(),
        mode: options.mode.clone(),
        ..CheckInput::default()
    };
    apply_to_check_input(&mut input, &config, options.explicit);

    let artifacts = pilot_artifacts(&options.out_dir);
    std::fs::create_dir_all(&options.out_dir)
        .map_err(|err| format!("create {} failed: {err}", options.out_dir.display()))?;

    let context = output::pilot::PilotSummaryContext {
        root: &input.root,
        mode: &input.mode,
        config_path: config.source_path(),
        max_seams: options.max_seams,
        timeout_ms: options.timeout_ms,
        artifacts: &artifacts,
    };

    let analysis_root = input.root.clone();
    let analysis_config = config.clone();
    let analysis_result = run_pilot_analysis_with_timeout(options.timeout_ms, move || {
        analysis::inventory_classified_seams_at_with_config(&analysis_root, &analysis_config)
    })?;
    let PilotAnalysisResult::Complete(classified) = analysis_result else {
        std::fs::write(
            &artifacts.pilot_summary_json,
            output::pilot::render_pilot_timeout_summary_json(context),
        )
        .map_err(|err| {
            format!(
                "write {} failed: {err}",
                artifacts.pilot_summary_json.display()
            )
        })?;
        std::fs::write(
            &artifacts.pilot_summary_md,
            output::pilot::render_pilot_timeout_summary_md(context),
        )
        .map_err(|err| {
            format!(
                "write {} failed: {err}",
                artifacts.pilot_summary_md.display()
            )
        })?;
        print!("{}", output::pilot::render_pilot_timeout_terminal(context));
        return Ok(());
    };

    std::fs::write(
        &artifacts.repo_exposure_json,
        output::repo_exposure::render_repo_exposure_json(&classified),
    )
    .map_err(|err| {
        format!(
            "write {} failed: {err}",
            artifacts.repo_exposure_json.display()
        )
    })?;
    std::fs::write(
        &artifacts.repo_exposure_md,
        output::repo_exposure::render_repo_exposure_md(&classified),
    )
    .map_err(|err| {
        format!(
            "write {} failed: {err}",
            artifacts.repo_exposure_md.display()
        )
    })?;
    std::fs::write(
        &artifacts.agent_seam_packets_json,
        output::agent_seam_packets::render_agent_seam_packets_json(&classified),
    )
    .map_err(|err| {
        format!(
            "write {} failed: {err}",
            artifacts.agent_seam_packets_json.display()
        )
    })?;

    std::fs::write(
        &artifacts.pilot_summary_json,
        output::pilot::render_pilot_summary_json(&classified, context),
    )
    .map_err(|err| {
        format!(
            "write {} failed: {err}",
            artifacts.pilot_summary_json.display()
        )
    })?;
    std::fs::write(
        &artifacts.pilot_summary_md,
        output::pilot::render_pilot_summary_md(&classified, context),
    )
    .map_err(|err| {
        format!(
            "write {} failed: {err}",
            artifacts.pilot_summary_md.display()
        )
    })?;

    print!(
        "{}",
        output::pilot::render_pilot_terminal(&classified, context)
    );
    Ok(())
}

fn parse_pilot_options(args: &[String]) -> Result<PilotOptions, String> {
    let mut options = PilotOptions {
        root: PathBuf::from("."),
        out_dir: PathBuf::from("target/ripr/pilot"),
        mode: Mode::Draft,
        explicit: CheckInputExplicit::default(),
        max_seams: 5,
        timeout_ms: DEFAULT_PILOT_TIMEOUT_MS,
    };
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                options.root = PathBuf::from(expect_value(args, i, "--root")?);
            }
            "--out" => {
                i += 1;
                options.out_dir = PathBuf::from(expect_value(args, i, "--out")?);
            }
            "--mode" => {
                i += 1;
                options.mode = parse_mode(expect_value(args, i, "--mode")?)?;
                options.explicit.mode = true;
            }
            "--max-seams" => {
                i += 1;
                options.max_seams =
                    parse_positive_usize(expect_value(args, i, "--max-seams")?, "--max-seams")?;
            }
            "--timeout-ms" => {
                i += 1;
                options.timeout_ms =
                    parse_positive_u64(expect_value(args, i, "--timeout-ms")?, "--timeout-ms")?;
            }
            other => return Err(format!("unknown pilot argument {other:?}")),
        }
        i += 1;
    }
    Ok(options)
}

fn parse_positive_usize(value: &str, flag: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|err| format!("invalid {flag}: {err}"))?;
    if parsed == 0 {
        return Err(format!("invalid {flag}: expected a positive integer"));
    }
    Ok(parsed)
}

fn parse_positive_u64(value: &str, flag: &str) -> Result<u64, String> {
    let parsed = value
        .parse::<u64>()
        .map_err(|err| format!("invalid {flag}: {err}"))?;
    if parsed == 0 {
        return Err(format!("invalid {flag}: expected a positive integer"));
    }
    Ok(parsed)
}

enum PilotAnalysisResult {
    Complete(Vec<analysis::ClassifiedSeam>),
    TimedOut,
}

fn run_pilot_analysis_with_timeout<F>(
    timeout_ms: u64,
    runner: F,
) -> Result<PilotAnalysisResult, String>
where
    F: FnOnce() -> Result<Vec<analysis::ClassifiedSeam>, String> + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result = runner();
        let _ignored = tx.send(result);
    });

    match rx.recv_timeout(Duration::from_millis(timeout_ms)) {
        Ok(result) => result.map(PilotAnalysisResult::Complete),
        Err(mpsc::RecvTimeoutError::Timeout) => Ok(PilotAnalysisResult::TimedOut),
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err("pilot analysis stopped before producing a result".to_string())
        }
    }
}

fn pilot_artifacts(out_dir: &Path) -> output::pilot::PilotArtifacts {
    output::pilot::PilotArtifacts {
        repo_exposure_json: out_dir.join("repo-exposure.json"),
        repo_exposure_md: out_dir.join("repo-exposure.md"),
        agent_seam_packets_json: out_dir.join("agent-seam-packets.json"),
        pilot_summary_json: out_dir.join("pilot-summary.json"),
        pilot_summary_md: out_dir.join("pilot-summary.md"),
    }
}

pub(super) fn outcome(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_outcome_help();
        return Ok(());
    }

    let options = parse_outcome_options(args)?;
    let before_json = std::fs::read_to_string(&options.before).map_err(|err| {
        format!(
            "read {} failed: {err}",
            output::outcome::display_path(&options.before)
        )
    })?;
    let after_json = std::fs::read_to_string(&options.after).map_err(|err| {
        format!(
            "read {} failed: {err}",
            output::outcome::display_path(&options.after)
        )
    })?;
    let report = output::outcome::targeted_test_outcome_report_from_json(
        &before_json,
        &after_json,
        output::outcome::display_path(&options.before),
        output::outcome::display_path(&options.after),
    )?;
    let rendered = match options.format {
        OutcomeFormat::Markdown => output::outcome::render_targeted_test_outcome_md(&report),
        OutcomeFormat::Json => output::outcome::render_targeted_test_outcome_json(&report)?,
    };

    match options.out {
        Some(path) => {
            if let Some(parent) = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
            {
                std::fs::create_dir_all(parent)
                    .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
            }
            std::fs::write(&path, rendered).map_err(|err| {
                format!(
                    "write {} failed: {err}",
                    output::outcome::display_path(&path)
                )
            })
        }
        None => {
            print!("{rendered}");
            Ok(())
        }
    }
}

pub(super) fn evidence_health(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_evidence_health_help();
        return Ok(());
    }

    let options = parse_evidence_health_options(args)?;
    if !options.root.is_dir() {
        return Err(format!(
            "evidence-health root {} is not a directory",
            options.root.display()
        ));
    }

    let config = load_for_root(&options.root)?;
    let classified = analysis::inventory_classified_seams_at_with_config(&options.root, &config)?;
    let calibration = match &options.mutation_calibration {
        Some(path) => {
            let contents = std::fs::read_to_string(path).map_err(|err| {
                format!(
                    "read evidence-health calibration context {} failed: {err}",
                    output::outcome::display_path(path)
                )
            })?;
            output::evidence_health::EvidenceHealthCalibration::from_json(
                output::outcome::display_path(path),
                &contents,
            )?
        }
        None => output::evidence_health::EvidenceHealthCalibration::not_provided(),
    };
    let report = output::evidence_health::build_evidence_health_report(
        &classified,
        output::outcome::display_path(&options.root),
        calibration,
    );
    let rendered_json = output::evidence_health::render_evidence_health_json(&report)?;
    let rendered_md = output::evidence_health::render_evidence_health_markdown(&report);
    write_text_file(&options.out, &rendered_json)?;
    write_text_file(&options.out_md, &rendered_md)?;
    println!("Wrote {}", options.out.display());
    println!("Wrote {}", options.out_md.display());
    Ok(())
}

pub(super) fn review_comments(args: &[String]) -> Result<(), String> {
    review_comments_with_diff_loader(args, load_review_comments_diff)
}

pub(super) fn gate(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_gate_help();
        return Ok(());
    }
    let Some((subcommand, rest)) = args.split_first() else {
        return Err("gate requires subcommand `evaluate`".to_string());
    };
    if subcommand != "evaluate" {
        return Err(format!(
            "unknown gate subcommand {subcommand:?}; expected `evaluate`"
        ));
    }

    let options = parse_gate_options(rest)?;
    let report = output::gate::build_gate_decision_report(&options.input)?;
    let rendered_json = output::gate::render_gate_decision_json(&report)?;
    let rendered_md = output::gate::render_gate_decision_markdown(&report);
    write_text_file(&options.out, &rendered_json)?;
    write_text_file(&options.out_md, &rendered_md)?;
    println!("Wrote {}", options.out.display());
    println!("Wrote {}", options.out_md.display());
    if output::gate::gate_decision_should_fail(&report) {
        Err(format!(
            "ripr gate decision is {}; see {}",
            output::gate::gate_decision_status(&report),
            options.out.display()
        ))
    } else {
        Ok(())
    }
}

pub(super) fn baseline(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_baseline_help();
        return Ok(());
    }
    let Some((subcommand, rest)) = args.split_first() else {
        return Err("baseline requires subcommand `create`, `diff`, or `update`".to_string());
    };
    match subcommand.as_str() {
        "create" => baseline_create(rest),
        "diff" => baseline_diff(rest),
        "update" => baseline_update(rest),
        _ => Err(format!(
            "unknown baseline subcommand {subcommand:?}; expected `create`, `diff`, or `update`"
        )),
    }
}

pub(super) fn zero(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_zero_help();
        return Ok(());
    }
    let Some((subcommand, rest)) = args.split_first() else {
        return Err("zero requires subcommand `status`".to_string());
    };
    if subcommand != "status" {
        return Err(format!(
            "unknown zero subcommand {subcommand:?}; expected `status`"
        ));
    }
    ripr_zero_status(rest)
}

pub(super) fn pr_ledger(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_pr_ledger_help();
        return Ok(());
    }
    let Some((subcommand, rest)) = args.split_first() else {
        return Err("pr-ledger requires subcommand `record`".to_string());
    };
    if subcommand != "record" {
        return Err(format!(
            "unknown pr-ledger subcommand {subcommand:?}; expected `record`"
        ));
    }
    pr_evidence_ledger_record(rest)
}

pub(super) fn pr_comments(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_pr_comments_help();
        return Ok(());
    }
    let Some((subcommand, rest)) = args.split_first() else {
        return Err("pr-comments requires subcommand `plan`".to_string());
    };
    if subcommand != "plan" {
        return Err(format!(
            "unknown pr-comments subcommand {subcommand:?}; expected `plan`"
        ));
    }
    pr_comments_plan(rest)
}

pub(super) fn pr_review(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_pr_review_help();
        return Ok(());
    }
    let Some((subcommand, rest)) = args.split_first() else {
        return Err("pr-review requires subcommand `front-panel`".to_string());
    };
    if subcommand != "front-panel" {
        return Err(format!(
            "unknown pr-review subcommand {subcommand:?}; expected `front-panel`"
        ));
    }
    pr_review_front_panel(rest)
}

pub(super) fn coverage_grip(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_coverage_grip_help();
        return Ok(());
    }
    let Some((subcommand, rest)) = args.split_first() else {
        return Err("coverage-grip requires subcommand `frontier`".to_string());
    };
    if subcommand != "frontier" {
        return Err(format!(
            "unknown coverage-grip subcommand {subcommand:?}; expected `frontier`"
        ));
    }
    coverage_grip_frontier(rest)
}

pub(super) fn assistant_loop(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_assistant_loop_help();
        return Ok(());
    }
    let Some((subcommand, rest)) = args.split_first() else {
        return Err("assistant-loop requires subcommand `proof` or `health`".to_string());
    };
    match subcommand.as_str() {
        "proof" => assistant_loop_proof(rest),
        "health" => assistant_loop_health(rest),
        _ => Err(format!(
            "unknown assistant-loop subcommand {subcommand:?}; expected `proof` or `health`"
        )),
    }
}

pub(super) fn first_action(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_first_action_help();
        return Ok(());
    }

    let options = parse_first_action_options(args)?;
    let pr_guidance_path = options
        .pr_guidance
        .as_ref()
        .map(|path| output::first_useful_action::display_path(path));
    let assistant_proof_path = options
        .assistant_proof
        .as_ref()
        .map(|path| output::first_useful_action::display_path(path));
    let ledger_path = options
        .ledger
        .as_ref()
        .map(|path| output::first_useful_action::display_path(path));
    let baseline_delta_path = options
        .baseline_delta
        .as_ref()
        .map(|path| output::first_useful_action::display_path(path));
    let receipt_path = options
        .receipt
        .as_ref()
        .map(|path| output::first_useful_action::display_path(path));
    let gate_decision_path = options
        .gate_decision
        .as_ref()
        .map(|path| output::first_useful_action::display_path(path));
    let coverage_frontier_path = options
        .coverage_frontier
        .as_ref()
        .map(|path| output::first_useful_action::display_path(path));
    let editor_context_path = options
        .editor_context
        .as_ref()
        .map(|path| output::first_useful_action::display_path(path));
    let input = output::first_useful_action::FirstUsefulActionInput {
        root: options.root,
        generated_at: first_action_generated_at()?,
        pr_guidance_path,
        assistant_proof_path,
        ledger_path,
        baseline_delta_path,
        receipt_path,
        gate_decision_path,
        coverage_frontier_path,
        editor_context_path,
        pr_guidance_json: options
            .pr_guidance
            .as_ref()
            .map(|path| read_optional_text_for_report("PR guidance", path)),
        assistant_proof_json: options
            .assistant_proof
            .as_ref()
            .map(|path| read_optional_text_for_report("assistant proof", path)),
        ledger_json: options
            .ledger
            .as_ref()
            .map(|path| read_optional_text_for_report("PR evidence ledger", path)),
        baseline_delta_json: options
            .baseline_delta
            .as_ref()
            .map(|path| read_optional_text_for_report("baseline debt delta", path)),
        receipt_json: options
            .receipt
            .as_ref()
            .map(|path| read_optional_text_for_report("receipt", path)),
        gate_decision_json: options
            .gate_decision
            .as_ref()
            .map(|path| read_optional_text_for_report("gate decision", path)),
        coverage_frontier_json: options
            .coverage_frontier
            .as_ref()
            .map(|path| read_optional_text_for_report("coverage/grip frontier", path)),
        editor_context_json: options
            .editor_context
            .as_ref()
            .map(|path| read_optional_text_for_report("editor context", path)),
    };
    let report = output::first_useful_action::build_first_useful_action_report(input);
    let rendered_json = output::first_useful_action::render_first_useful_action_json(&report)?;
    let rendered_md = output::first_useful_action::render_first_useful_action_markdown(&report);
    write_text_file(&options.out, &rendered_json)?;
    write_text_file(&options.out_md, &rendered_md)?;
    println!("Wrote {}", options.out.display());
    println!("Wrote {}", options.out_md.display());
    Ok(())
}

pub(super) fn reports(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_reports_help();
        return Ok(());
    }
    let Some((subcommand, rest)) = args.split_first() else {
        return Err("reports requires subcommand `index`".to_string());
    };
    if subcommand != "index" {
        return Err(format!(
            "unknown reports subcommand {subcommand:?}; expected `index`"
        ));
    }
    report_packet_index(rest)
}

fn report_packet_index(args: &[String]) -> Result<(), String> {
    let options = parse_report_packet_index_options(args)?;
    let input = output::report_packet_index::ReportPacketIndexInput {
        root: options.root,
        generated_at: report_packet_index_generated_at()?,
        reports_dir: options.reports_dir,
        review_dir: options.review_dir,
        receipts_dir: options.receipts_dir,
        workflow_dir: options.workflow_dir,
        agent_dir: options.agent_dir,
        pilot_dir: options.pilot_dir,
        ci_dir: options.ci_dir,
    };
    let report = output::report_packet_index::build_report_packet_index_report(input);
    let rendered_json = output::report_packet_index::render_report_packet_index_json(&report)?;
    let rendered_md = output::report_packet_index::render_report_packet_index_markdown(&report);
    write_text_file(&options.out, &rendered_json)?;
    write_text_file(&options.out_md, &rendered_md)?;
    println!("Wrote {}", options.out.display());
    println!("Wrote {}", options.out_md.display());
    Ok(())
}

fn ripr_zero_status(args: &[String]) -> Result<(), String> {
    let options = parse_ripr_zero_status_options(args)?;
    let baseline_path = options
        .baseline
        .as_ref()
        .map(|path| output::ripr_zero_status::display_path(path));
    let gate_path = options
        .gate
        .as_ref()
        .map(|path| output::ripr_zero_status::display_path(path));
    let pr_guidance_path = options
        .pr_guidance
        .as_ref()
        .map(|path| output::ripr_zero_status::display_path(path));
    let recommendation_calibration_path = options
        .recommendation_calibration
        .as_ref()
        .map(|path| output::ripr_zero_status::display_path(path));
    let input = output::ripr_zero_status::RiprZeroStatusInput {
        root: ".".to_string(),
        generated_at: baseline_created_at()?,
        baseline_path,
        delta_path: output::ripr_zero_status::display_path(&options.delta),
        gate_path,
        pr_guidance_path,
        recommendation_calibration_path,
        baseline_json: options
            .baseline
            .as_ref()
            .map(|path| read_optional_text_for_report("baseline", path)),
        delta_json: read_optional_text_for_report("baseline debt delta", &options.delta),
        gate_json: options
            .gate
            .as_ref()
            .map(|path| read_optional_text_for_report("gate decision", path)),
        pr_guidance_json: options
            .pr_guidance
            .as_ref()
            .map(|path| read_optional_text_for_report("PR guidance", path)),
        recommendation_calibration_json: options
            .recommendation_calibration
            .as_ref()
            .map(|path| read_optional_text_for_report("recommendation calibration", path)),
    };
    let report = output::ripr_zero_status::build_ripr_zero_status_report(input);
    let rendered_json = output::ripr_zero_status::render_ripr_zero_status_json(&report)?;
    let rendered_md = output::ripr_zero_status::render_ripr_zero_status_markdown(&report);
    write_text_file(&options.out, &rendered_json)?;
    write_text_file(&options.out_md, &rendered_md)?;
    println!("Wrote {}", options.out.display());
    println!("Wrote {}", options.out_md.display());
    Ok(())
}

fn pr_evidence_ledger_record(args: &[String]) -> Result<(), String> {
    let options = parse_pr_evidence_ledger_options(args)?;
    let gate_path = options
        .gate
        .as_ref()
        .map(|path| output::pr_evidence_ledger::display_path(path));
    let baseline_delta_path = options
        .baseline_delta
        .as_ref()
        .map(|path| output::pr_evidence_ledger::display_path(path));
    let zero_status_path = options
        .zero_status
        .as_ref()
        .map(|path| output::pr_evidence_ledger::display_path(path));
    let pr_guidance_path = options
        .pr_guidance
        .as_ref()
        .map(|path| output::pr_evidence_ledger::display_path(path));
    let recommendation_calibration_path = options
        .recommendation_calibration
        .as_ref()
        .map(|path| output::pr_evidence_ledger::display_path(path));
    let agent_receipt_path = options
        .agent_receipt
        .as_ref()
        .map(|path| output::pr_evidence_ledger::display_path(path));
    let coverage_path = options
        .coverage
        .as_ref()
        .map(|path| output::pr_evidence_ledger::display_path(path));
    let history_path = options
        .history
        .as_ref()
        .map(|path| output::pr_evidence_ledger::display_path(path));
    let input = output::pr_evidence_ledger::PrEvidenceLedgerInput {
        root: ".".to_string(),
        generated_at: baseline_created_at()?,
        pr_number: options.pr_number,
        base: options.base,
        head: options.head,
        labels: options.labels,
        gate_path,
        baseline_delta_path,
        zero_status_path,
        pr_guidance_path,
        recommendation_calibration_path,
        agent_receipt_path,
        coverage_path,
        history_path,
        gate_json: options
            .gate
            .as_ref()
            .map(|path| read_optional_text_for_report("gate decision", path)),
        baseline_delta_json: options
            .baseline_delta
            .as_ref()
            .map(|path| read_optional_text_for_report("baseline debt delta", path)),
        zero_status_json: options
            .zero_status
            .as_ref()
            .map(|path| read_optional_text_for_report("RIPR Zero status", path)),
        pr_guidance_json: options
            .pr_guidance
            .as_ref()
            .map(|path| read_optional_text_for_report("PR guidance", path)),
        recommendation_calibration_json: options
            .recommendation_calibration
            .as_ref()
            .map(|path| read_optional_text_for_report("recommendation calibration", path)),
        agent_receipt_json: options
            .agent_receipt
            .as_ref()
            .map(|path| read_optional_text_for_report("agent receipt", path)),
        coverage_json: options
            .coverage
            .as_ref()
            .map(|path| read_optional_text_for_report("coverage", path)),
        history_json: options
            .history
            .as_ref()
            .map(|path| read_optional_text_for_report("history", path)),
    };
    let report = output::pr_evidence_ledger::build_pr_evidence_ledger_report(input);
    let rendered_json = output::pr_evidence_ledger::render_pr_evidence_ledger_json(&report)?;
    let rendered_md = output::pr_evidence_ledger::render_pr_evidence_ledger_markdown(&report);
    write_text_file(&options.out, &rendered_json)?;
    write_text_file(&options.out_md, &rendered_md)?;
    println!("Wrote {}", options.out.display());
    println!("Wrote {}", options.out_md.display());
    Ok(())
}

fn pr_comments_plan(args: &[String]) -> Result<(), String> {
    let options = parse_pr_comments_plan_options(args)?;
    let pr_guidance_path = options
        .pr_guidance
        .as_ref()
        .map(|path| output::pr_inline_comment_publish_plan::display_path(path));
    let existing_comments_path = options
        .existing_comments
        .as_ref()
        .map(|path| output::pr_inline_comment_publish_plan::display_path(path));
    let input = output::pr_inline_comment_publish_plan::CommentPublishPlanInput {
        root: options.root,
        generated_at: comment_publish_plan_generated_at()?,
        mode: options.mode,
        max_inline_comments: options.max_inline_comments,
        pr_guidance_path,
        pr_guidance_json: options
            .pr_guidance
            .as_ref()
            .map(|path| read_optional_text_for_report("PR guidance", path)),
        existing_comments_path,
        existing_comments_json: options
            .existing_comments
            .as_ref()
            .map(|path| read_optional_text_for_report("existing comments", path)),
        permission: output::pr_inline_comment_publish_plan::CommentPermissionContext {
            pull_request: options.pull_request,
            event_name: options.event_name,
            head_repo: options.head_repo,
            base_repo: options.base_repo,
            token_available: options.token_available,
            write_permission: options.write_permission,
        },
    };
    let report = output::pr_inline_comment_publish_plan::build_comment_publish_plan_report(input);
    let rendered_json =
        output::pr_inline_comment_publish_plan::render_comment_publish_plan_json(&report)?;
    let rendered_md =
        output::pr_inline_comment_publish_plan::render_comment_publish_plan_markdown(&report);
    write_text_file(&options.out, &rendered_json)?;
    write_text_file(&options.out_md, &rendered_md)?;
    println!("Wrote {}", options.out.display());
    println!("Wrote {}", options.out_md.display());
    Ok(())
}

fn pr_review_front_panel(args: &[String]) -> Result<(), String> {
    let options = parse_pr_review_front_panel_options(args)?;
    let pr_guidance_path = options
        .pr_guidance
        .as_ref()
        .map(|path| output::pr_review_front_panel::display_path(path));
    let first_action_path = options
        .first_action
        .as_ref()
        .map(|path| output::pr_review_front_panel::display_path(path));
    let assistant_proof_path = options
        .assistant_proof
        .as_ref()
        .map(|path| output::pr_review_front_panel::display_path(path));
    let assistant_health_path = options
        .assistant_health
        .as_ref()
        .map(|path| output::pr_review_front_panel::display_path(path));
    let ledger_path = options
        .ledger
        .as_ref()
        .map(|path| output::pr_review_front_panel::display_path(path));
    let baseline_delta_path = options
        .baseline_delta
        .as_ref()
        .map(|path| output::pr_review_front_panel::display_path(path));
    let zero_status_path = options
        .zero_status
        .as_ref()
        .map(|path| output::pr_review_front_panel::display_path(path));
    let gate_decision_path = options
        .gate_decision
        .as_ref()
        .map(|path| output::pr_review_front_panel::display_path(path));
    let recommendation_calibration_path = options
        .recommendation_calibration
        .as_ref()
        .map(|path| output::pr_review_front_panel::display_path(path));
    let mutation_calibration_path = options
        .mutation_calibration
        .as_ref()
        .map(|path| output::pr_review_front_panel::display_path(path));
    let coverage_frontier_path = options
        .coverage_frontier
        .as_ref()
        .map(|path| output::pr_review_front_panel::display_path(path));
    let receipt_path = options
        .receipt
        .as_ref()
        .map(|path| output::pr_review_front_panel::display_path(path));
    let input = output::pr_review_front_panel::PrReviewFrontPanelInput {
        root: options.root,
        generated_at: pr_review_front_panel_generated_at()?,
        out_md_path: output::pr_review_front_panel::display_path(&options.out_md),
        pr_guidance_path,
        first_action_path,
        assistant_proof_path,
        assistant_health_path,
        ledger_path,
        baseline_delta_path,
        zero_status_path,
        gate_decision_path,
        recommendation_calibration_path,
        mutation_calibration_path,
        coverage_frontier_path,
        receipt_path,
        pr_guidance_json: options
            .pr_guidance
            .as_ref()
            .map(|path| read_optional_text_for_report("PR guidance", path)),
        first_action_json: options
            .first_action
            .as_ref()
            .map(|path| read_optional_text_for_report("first useful action", path)),
        assistant_proof_json: options
            .assistant_proof
            .as_ref()
            .map(|path| read_optional_text_for_report("assistant proof", path)),
        assistant_health_json: options
            .assistant_health
            .as_ref()
            .map(|path| read_optional_text_for_report("assistant loop health", path)),
        ledger_json: options
            .ledger
            .as_ref()
            .map(|path| read_optional_text_for_report("PR evidence ledger", path)),
        baseline_delta_json: options
            .baseline_delta
            .as_ref()
            .map(|path| read_optional_text_for_report("baseline debt delta", path)),
        zero_status_json: options
            .zero_status
            .as_ref()
            .map(|path| read_optional_text_for_report("RIPR Zero status", path)),
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
        coverage_frontier_json: options
            .coverage_frontier
            .as_ref()
            .map(|path| read_optional_text_for_report("coverage/grip frontier", path)),
        receipt_json: options
            .receipt
            .as_ref()
            .map(|path| read_optional_text_for_report("receipt", path)),
    };
    let report = output::pr_review_front_panel::build_pr_review_front_panel_report(input);
    let rendered_json = output::pr_review_front_panel::render_pr_review_front_panel_json(&report)?;
    let rendered_md = output::pr_review_front_panel::render_pr_review_front_panel_markdown(&report);
    write_text_file(&options.out, &rendered_json)?;
    write_text_file(&options.out_md, &rendered_md)?;
    println!("Wrote {}", options.out.display());
    println!("Wrote {}", options.out_md.display());
    Ok(())
}

fn coverage_grip_frontier(args: &[String]) -> Result<(), String> {
    let options = parse_coverage_grip_frontier_options(args)?;
    let coverage_path = options
        .coverage
        .as_ref()
        .map(|path| output::coverage_grip_frontier::display_path(path));
    let ledger_path = options
        .ledger
        .as_ref()
        .map(|path| output::coverage_grip_frontier::display_path(path));
    let baseline_delta_path = options
        .baseline_delta
        .as_ref()
        .map(|path| output::coverage_grip_frontier::display_path(path));
    let zero_status_path = options
        .zero_status
        .as_ref()
        .map(|path| output::coverage_grip_frontier::display_path(path));
    let input = output::coverage_grip_frontier::CoverageGripFrontierInput {
        root: ".".to_string(),
        generated_at: baseline_created_at()?,
        coverage_path,
        ledger_path,
        baseline_delta_path,
        zero_status_path,
        coverage_json: options
            .coverage
            .as_ref()
            .map(|path| read_optional_text_for_report("coverage", path)),
        ledger_json: options
            .ledger
            .as_ref()
            .map(|path| read_optional_text_for_report("PR evidence ledger", path)),
        baseline_delta_json: options
            .baseline_delta
            .as_ref()
            .map(|path| read_optional_text_for_report("baseline debt delta", path)),
        zero_status_json: options
            .zero_status
            .as_ref()
            .map(|path| read_optional_text_for_report("RIPR Zero status", path)),
    };
    let report = output::coverage_grip_frontier::build_coverage_grip_frontier_report(input);
    let rendered_json =
        output::coverage_grip_frontier::render_coverage_grip_frontier_json(&report)?;
    let rendered_md =
        output::coverage_grip_frontier::render_coverage_grip_frontier_markdown(&report);
    write_text_file(&options.out, &rendered_json)?;
    write_text_file(&options.out_md, &rendered_md)?;
    println!("Wrote {}", options.out.display());
    println!("Wrote {}", options.out_md.display());
    Ok(())
}

fn assistant_loop_proof(args: &[String]) -> Result<(), String> {
    let options = parse_assistant_loop_proof_options(args)?;
    let pr_guidance_path = options
        .pr_guidance
        .as_ref()
        .map(|path| output::test_oracle_assistant_proof::display_path(path));
    let agent_packet_path = options
        .agent_packet
        .as_ref()
        .map(|path| output::test_oracle_assistant_proof::display_path(path));
    let before_path = options
        .before
        .as_ref()
        .map(|path| output::test_oracle_assistant_proof::display_path(path));
    let after_path = options
        .after
        .as_ref()
        .map(|path| output::test_oracle_assistant_proof::display_path(path));
    let receipt_path = options
        .receipt
        .as_ref()
        .map(|path| output::test_oracle_assistant_proof::display_path(path));
    let ledger_path = options
        .ledger
        .as_ref()
        .map(|path| output::test_oracle_assistant_proof::display_path(path));
    let coverage_frontier_path = options
        .coverage_frontier
        .as_ref()
        .map(|path| output::test_oracle_assistant_proof::display_path(path));
    let gate_decision_path = options
        .gate_decision
        .as_ref()
        .map(|path| output::test_oracle_assistant_proof::display_path(path));
    let input = output::test_oracle_assistant_proof::TestOracleAssistantProofInput {
        root: options.root,
        pr_guidance_path,
        agent_packet_path,
        before_path,
        after_path,
        receipt_path,
        ledger_path,
        coverage_frontier_path,
        gate_decision_path,
        pr_guidance_json: options
            .pr_guidance
            .as_ref()
            .map(|path| read_optional_text_for_report("PR guidance", path)),
        agent_packet_json: options
            .agent_packet
            .as_ref()
            .map(|path| read_optional_text_for_report("agent packet", path)),
        before_json: options
            .before
            .as_ref()
            .map(|path| read_optional_text_for_report("before evidence", path)),
        after_json: options
            .after
            .as_ref()
            .map(|path| read_optional_text_for_report("after evidence", path)),
        receipt_json: options
            .receipt
            .as_ref()
            .map(|path| read_optional_text_for_report("receipt", path)),
        ledger_json: options
            .ledger
            .as_ref()
            .map(|path| read_optional_text_for_report("PR evidence ledger", path)),
        coverage_frontier_json: options
            .coverage_frontier
            .as_ref()
            .map(|path| read_optional_text_for_report("coverage/grip frontier", path)),
        gate_decision_json: options
            .gate_decision
            .as_ref()
            .map(|path| read_optional_text_for_report("gate decision", path)),
    };
    let report =
        output::test_oracle_assistant_proof::build_test_oracle_assistant_proof_report(input);
    let rendered_json =
        output::test_oracle_assistant_proof::render_test_oracle_assistant_proof_json(&report)?;
    let rendered_md =
        output::test_oracle_assistant_proof::render_test_oracle_assistant_proof_markdown(&report);
    write_text_file(&options.out, &rendered_json)?;
    write_text_file(&options.out_md, &rendered_md)?;
    println!("Wrote {}", options.out.display());
    println!("Wrote {}", options.out_md.display());
    Ok(())
}

fn assistant_loop_health(args: &[String]) -> Result<(), String> {
    let options = parse_assistant_loop_health_options(args)?;
    let proofs = options
        .proofs
        .iter()
        .map(|path| {
            let source_artifact = output::assistant_loop_health::display_path(path);
            let proof_json = read_optional_text_for_report("assistant proof", path);
            output::assistant_loop_health::AssistantLoopHealthProofInput {
                source_artifact,
                proof_json,
            }
        })
        .collect::<Vec<_>>();
    let report = output::assistant_loop_health::build_assistant_loop_health_report(
        output::assistant_loop_health::AssistantLoopHealthInput {
            root: options.root,
            generated_at: assistant_loop_health_generated_at()?,
            proofs,
        },
    );
    let rendered_json = output::assistant_loop_health::render_assistant_loop_health_json(&report)?;
    let rendered_md = output::assistant_loop_health::render_assistant_loop_health_markdown(&report);
    write_text_file(&options.out, &rendered_json)?;
    write_text_file(&options.out_md, &rendered_md)?;
    println!("Wrote {}", options.out.display());
    println!("Wrote {}", options.out_md.display());
    println!(
        "Proofs: {}",
        output::assistant_loop_health::assistant_loop_health_proof_count(&report)
    );
    Ok(())
}

fn baseline_create(args: &[String]) -> Result<(), String> {
    let options = parse_baseline_create_options(args)?;
    let gate_decision_json = std::fs::read_to_string(&options.from).map_err(|err| {
        format!(
            "read baseline create source {} failed: {err}",
            output::baseline::display_path(&options.from)
        )
    })?;
    let created_at = baseline_created_at()?;
    let source_report = output::baseline::display_path(&options.from);
    let report = output::baseline::baseline_create_report_from_gate_decision_json(
        &source_report,
        &created_at,
        &gate_decision_json,
    )?;
    let rendered = output::baseline::render_baseline_create_json(&report)?;
    if options.dry_run {
        print!("{rendered}");
        return Ok(());
    }
    if options.out.exists() && !options.force {
        return Err(format!(
            "{} already exists; rerun `ripr baseline create --force` to overwrite it",
            options.out.display()
        ));
    }
    write_text_file(&options.out, &rendered)?;
    println!("Wrote {}", options.out.display());
    println!(
        "Entries: {}",
        output::baseline::baseline_entry_count(&report)
    );
    Ok(())
}

fn baseline_diff(args: &[String]) -> Result<(), String> {
    let options = parse_baseline_diff_options(args)?;
    let baseline_path = output::baseline_delta::display_path(&options.baseline);
    let current_path = output::baseline_delta::display_path(&options.current);
    let baseline_json = read_optional_text_for_report("baseline", &options.baseline);
    let current_json = read_optional_text_for_report("current gate-decision", &options.current);
    let report = output::baseline_delta::build_baseline_delta_report(
        output::baseline_delta::BaselineDeltaInput {
            root: ".".to_string(),
            baseline_path,
            current_gate_decision_path: current_path,
            baseline_json,
            current_gate_decision_json: current_json,
        },
    );
    let rendered_json = output::baseline_delta::render_baseline_delta_json(&report)?;
    let rendered_md = output::baseline_delta::render_baseline_delta_markdown(&report);
    write_text_file(&options.out, &rendered_json)?;
    write_text_file(&options.out_md, &rendered_md)?;
    println!("Wrote {}", options.out.display());
    println!("Wrote {}", options.out_md.display());
    println!(
        "Items: {}",
        output::baseline_delta::baseline_delta_item_count(&report)
    );
    Ok(())
}

fn baseline_update(args: &[String]) -> Result<(), String> {
    let options = parse_baseline_update_options(args)?;
    if !options.remove_resolved {
        return Err(
            "baseline update requires --remove-resolved; adopting new debt is not supported"
                .to_string(),
        );
    }
    let baseline_path = output::baseline_update::display_path(&options.baseline);
    let current_path = output::baseline_update::display_path(&options.current);
    let baseline_json = std::fs::read_to_string(&options.baseline).map_err(|err| {
        format!(
            "read baseline update baseline {} failed: {err}",
            output::baseline_update::display_path(&options.baseline)
        )
    })?;
    let current_json = std::fs::read_to_string(&options.current).map_err(|err| {
        format!(
            "read baseline update current gate-decision {} failed: {err}",
            output::baseline_update::display_path(&options.current)
        )
    })?;
    let report = output::baseline_update::build_baseline_update_remove_resolved(
        output::baseline_update::BaselineUpdateInput {
            baseline_path,
            current_gate_decision_path: current_path,
            baseline_json,
            current_gate_decision_json: current_json,
        },
    )?;
    let rendered = output::baseline_update::render_baseline_update_json(&report)?;
    let out = options.out.unwrap_or_else(|| options.baseline.clone());
    write_text_file(&out, &rendered)?;
    println!("Wrote {}", out.display());
    println!(
        "Entries: {} -> {}",
        output::baseline_update::baseline_update_before_entry_count(&report),
        output::baseline_update::baseline_update_after_entry_count(&report)
    );
    println!(
        "Removed resolved: {}",
        output::baseline_update::baseline_update_removed_resolved_count(&report)
    );
    println!(
        "Ignored new current: {}",
        output::baseline_update::baseline_update_ignored_new_current_count(&report)
    );
    if output::baseline_update::baseline_update_warning_count(&report) > 0 {
        println!(
            "Warnings: {}",
            output::baseline_update::baseline_update_warning_count(&report)
        );
    }
    Ok(())
}

fn review_comments_with_diff_loader(
    args: &[String],
    load_diff: impl Fn(&Path, &str, &str) -> Result<String, String>,
) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_review_comments_help();
        return Ok(());
    }

    let options = parse_review_comments_options(args)?;
    if !options.root.is_dir() {
        return Err(format!(
            "review-comments root {} is not a directory",
            options.root.display()
        ));
    }

    let config = load_for_root(&options.root)?;
    let mut input = CheckInput {
        root: options.root.clone(),
        ..CheckInput::default()
    };
    apply_to_check_input(&mut input, &config, CheckInputExplicit::default());

    let diff_text = load_diff(&input.root, &options.base, &options.head)?;
    let changed_lines = agent_brief_lines_from_diff(&input.root, &diff_text);
    let changed_owners = agent_brief_owners_for_lines(&input.root, &changed_lines);
    let working_set = AgentBriefResolvedWorkingSet::base(options.base.clone(), changed_lines)
        .with_changed_owners(changed_owners);
    let classified = analysis::inventory_classified_seams_at_with_config(&input.root, &config)?;
    let selection = select_agent_brief_seams(
        &classified,
        &working_set,
        output::review_comments::DEFAULT_REVIEW_MAX_SUMMARY_ITEMS,
        AgentBriefPolicy::from_config(&config),
    );
    let rendered_json = output::review_comments::render_review_comments_json(
        &input.root,
        &options.base,
        &options.head,
        &input.mode,
        &config,
        &working_set,
        &selection,
    )?;
    let rendered_md = output::review_comments::render_review_comments_markdown(
        &input.root,
        &options.base,
        &options.head,
        &input.mode,
        &config,
        &working_set,
        &selection,
    );
    let markdown_path = review_comments_markdown_path(&options.out);
    write_text_file(&options.out, &rendered_json)?;
    write_text_file(&markdown_path, &rendered_md)?;
    println!("Wrote {}", options.out.display());
    println!("Wrote {}", markdown_path.display());
    Ok(())
}

pub(super) fn calibrate(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_calibrate_help();
        return Ok(());
    }

    let Some((subcommand, rest)) = args.split_first() else {
        return Err("calibrate requires subcommand `cargo-mutants`".to_string());
    };
    if subcommand != "cargo-mutants" {
        return Err(format!(
            "unknown calibrate subcommand {subcommand:?}; expected `cargo-mutants`"
        ));
    }

    let options = parse_calibrate_cargo_mutants_options(rest)?;
    let repo_exposure_json =
        std::fs::read_to_string(&options.repo_exposure_json).map_err(|err| {
            format!(
                "read {} failed: {err}",
                output::outcome::display_path(&options.repo_exposure_json)
            )
        })?;
    let mutants_json = read_calibration_mutants_json(&options.mutants_json)?;
    let report = output::mutation_calibration::mutation_calibration_report_from_json(
        &repo_exposure_json,
        &mutants_json,
    )?;
    let rendered = match options.format {
        CalibrateFormat::Markdown => {
            output::mutation_calibration::render_mutation_calibration_md(&report)
        }
        CalibrateFormat::Json => {
            output::mutation_calibration::render_mutation_calibration_json(&report)?
        }
    };

    match options.out {
        Some(path) => {
            if let Some(parent) = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
            {
                std::fs::create_dir_all(parent)
                    .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
            }
            std::fs::write(&path, rendered).map_err(|err| {
                format!(
                    "write {} failed: {err}",
                    output::outcome::display_path(&path)
                )
            })
        }
        None => {
            print!("{rendered}");
            Ok(())
        }
    }
}

fn parse_calibrate_cargo_mutants_options(args: &[String]) -> Result<CalibrateOptions, String> {
    let mut mutants_json: Option<PathBuf> = None;
    let mut repo_exposure_json: Option<PathBuf> = None;
    let mut format = CalibrateFormat::Markdown;
    let mut out: Option<PathBuf> = None;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--mutants-json" | "--cargo-mutants-json" | "--input" => {
                i += 1;
                mutants_json = Some(PathBuf::from(expect_value(args, i, "--mutants-json")?));
            }
            "--repo-exposure-json" | "--static-json" => {
                i += 1;
                repo_exposure_json = Some(PathBuf::from(expect_value(
                    args,
                    i,
                    "--repo-exposure-json",
                )?));
            }
            "--format" => {
                i += 1;
                format = parse_calibrate_format(expect_value(args, i, "--format")?)?;
            }
            "--out" => {
                i += 1;
                out = Some(PathBuf::from(expect_value(args, i, "--out")?));
            }
            other => {
                return Err(format!(
                    "unknown calibrate cargo-mutants argument {other:?}"
                ));
            }
        }
        i += 1;
    }

    let mutants_json = mutants_json
        .ok_or_else(|| "calibrate cargo-mutants requires --mutants-json <path>".to_string())?;
    let repo_exposure_json = repo_exposure_json.ok_or_else(|| {
        "calibrate cargo-mutants requires --repo-exposure-json <path>".to_string()
    })?;
    Ok(CalibrateOptions {
        mutants_json,
        repo_exposure_json,
        format,
        out,
    })
}

fn parse_calibrate_format(value: &str) -> Result<CalibrateFormat, String> {
    match value {
        "md" | "markdown" | "text" => Ok(CalibrateFormat::Markdown),
        "json" => Ok(CalibrateFormat::Json),
        _ => Err(format!("unknown calibrate format {value:?}")),
    }
}

fn read_calibration_mutants_json(path: &Path) -> Result<String, String> {
    if path.is_dir() {
        let outcomes_path = path.join("outcomes.json");
        let mutants_path = path.join("mutants.json");
        let outcomes_exists = outcomes_path.exists();
        let mutants_exists = mutants_path.exists();

        if outcomes_exists && mutants_exists {
            let outcomes = read_json_value(&outcomes_path)?;
            let mutants = read_json_value(&mutants_path)?;
            return serde_json::to_string(&serde_json::Value::Array(vec![outcomes, mutants]))
                .map_err(|err| format!("failed to combine cargo-mutants directory JSON: {err}"));
        }

        if outcomes_exists {
            return read_calibration_text(&outcomes_path);
        }
        if mutants_exists {
            return read_calibration_text(&mutants_path);
        }
        return Err(format!(
            "{} is a directory but contains neither outcomes.json nor mutants.json",
            output::outcome::display_path(path)
        ));
    }
    read_calibration_text(path)
}

fn read_json_value(path: &Path) -> Result<serde_json::Value, String> {
    let text = read_calibration_text(path)?;
    serde_json::from_str(&text).map_err(|err| {
        format!(
            "failed to parse JSON from {}: {err}",
            output::outcome::display_path(path)
        )
    })
}

fn read_calibration_text(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path)
        .map_err(|err| format!("read {} failed: {err}", output::outcome::display_path(path)))
}

fn parse_outcome_options(args: &[String]) -> Result<OutcomeOptions, String> {
    let mut before: Option<PathBuf> = None;
    let mut after: Option<PathBuf> = None;
    let mut format = OutcomeFormat::Markdown;
    let mut out: Option<PathBuf> = None;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--before" => {
                i += 1;
                before = Some(PathBuf::from(expect_value(args, i, "--before")?));
            }
            "--after" => {
                i += 1;
                after = Some(PathBuf::from(expect_value(args, i, "--after")?));
            }
            "--format" => {
                i += 1;
                format = parse_outcome_format(expect_value(args, i, "--format")?)?;
            }
            "--out" => {
                i += 1;
                out = Some(PathBuf::from(expect_value(args, i, "--out")?));
            }
            other => return Err(format!("unknown outcome argument {other:?}")),
        }
        i += 1;
    }

    let before = before.ok_or_else(|| "outcome requires --before <path>".to_string())?;
    let after = after.ok_or_else(|| "outcome requires --after <path>".to_string())?;
    Ok(OutcomeOptions {
        before,
        after,
        format,
        out,
    })
}

fn parse_evidence_health_options(args: &[String]) -> Result<EvidenceHealthOptions, String> {
    let mut root = PathBuf::from(".");
    let mut out = PathBuf::from("target/ripr/reports/evidence-health.json");
    let mut out_md = PathBuf::from("target/ripr/reports/evidence-health.md");
    let mut mutation_calibration: Option<PathBuf> = None;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = PathBuf::from(expect_value(args, i, "--root")?);
            }
            "--out" => {
                i += 1;
                out = PathBuf::from(expect_value(args, i, "--out")?);
            }
            "--out-md" => {
                i += 1;
                out_md = PathBuf::from(expect_value(args, i, "--out-md")?);
            }
            "--mutation-calibration" => {
                i += 1;
                mutation_calibration = Some(PathBuf::from(expect_value(
                    args,
                    i,
                    "--mutation-calibration",
                )?));
            }
            other => return Err(format!("unknown evidence-health argument {other:?}")),
        }
        i += 1;
    }

    Ok(EvidenceHealthOptions {
        root,
        out,
        out_md,
        mutation_calibration,
    })
}

fn parse_review_comments_options(args: &[String]) -> Result<ReviewCommentsOptions, String> {
    let mut root = PathBuf::from(".");
    let mut base: Option<String> = None;
    let mut head: Option<String> = None;
    let mut out = PathBuf::from("target/ripr/review/comments.json");

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = PathBuf::from(expect_value(args, i, "--root")?);
            }
            "--base" => {
                i += 1;
                let value = expect_value(args, i, "--base")?;
                if value.trim().is_empty() {
                    return Err("review-comments --base requires a non-empty revision".to_string());
                }
                base = Some(value.to_string());
            }
            "--head" => {
                i += 1;
                let value = expect_value(args, i, "--head")?;
                if value.trim().is_empty() {
                    return Err("review-comments --head requires a non-empty revision".to_string());
                }
                head = Some(value.to_string());
            }
            "--out" => {
                i += 1;
                let value = expect_value(args, i, "--out")?;
                if value.trim().is_empty() {
                    return Err("review-comments --out requires a non-empty path".to_string());
                }
                out = PathBuf::from(value);
            }
            other => return Err(format!("unknown review-comments argument {other:?}")),
        }
        i += 1;
    }

    Ok(ReviewCommentsOptions {
        root,
        base: base.ok_or_else(|| "review-comments requires --base <sha>".to_string())?,
        head: head.ok_or_else(|| "review-comments requires --head <sha>".to_string())?,
        out,
    })
}

fn parse_gate_options(args: &[String]) -> Result<GateOptions, String> {
    let mut root = PathBuf::from(".");
    let mut repo_exposure = None;
    let mut pr_guidance = None;
    let mut sarif_policy = None;
    let mut labels_json = None;
    let mut labels = Vec::new();
    let mut agent_verify = None;
    let mut agent_receipt = None;
    let mut recommendation_calibration = None;
    let mut mutation_calibration = None;
    let mut baseline = None;
    let mut mode = output::gate::GateMode::VisibleOnly;
    let mut acknowledgement_labels = Vec::new();
    let mut out = PathBuf::from(output::gate::DEFAULT_GATE_OUT);
    let mut out_md = None;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = non_empty_path_arg(args, i, "--root", "gate")?;
            }
            "--repo-exposure" => {
                i += 1;
                repo_exposure = Some(non_empty_path_arg(args, i, "--repo-exposure", "gate")?);
            }
            "--pr-guidance" => {
                i += 1;
                pr_guidance = Some(non_empty_path_arg(args, i, "--pr-guidance", "gate")?);
            }
            "--sarif-policy" => {
                i += 1;
                sarif_policy = Some(non_empty_path_arg(args, i, "--sarif-policy", "gate")?);
            }
            "--labels-json" => {
                i += 1;
                labels_json = Some(non_empty_path_arg(args, i, "--labels-json", "gate")?);
            }
            "--label" => {
                i += 1;
                labels.push(non_empty_string_arg(args, i, "--label", "gate")?);
            }
            "--agent-verify" => {
                i += 1;
                agent_verify = Some(non_empty_path_arg(args, i, "--agent-verify", "gate")?);
            }
            "--agent-receipt" => {
                i += 1;
                agent_receipt = Some(non_empty_path_arg(args, i, "--agent-receipt", "gate")?);
            }
            "--recommendation-calibration" => {
                i += 1;
                recommendation_calibration = Some(non_empty_path_arg(
                    args,
                    i,
                    "--recommendation-calibration",
                    "gate",
                )?);
            }
            "--mutation-calibration" => {
                i += 1;
                mutation_calibration = Some(non_empty_path_arg(
                    args,
                    i,
                    "--mutation-calibration",
                    "gate",
                )?);
            }
            "--baseline" => {
                i += 1;
                baseline = Some(non_empty_path_arg(args, i, "--baseline", "gate")?);
            }
            "--mode" => {
                i += 1;
                mode = output::gate::GateMode::parse(expect_value(args, i, "--mode")?)?;
            }
            "--acknowledgement-label" => {
                i += 1;
                acknowledgement_labels.push(non_empty_string_arg(
                    args,
                    i,
                    "--acknowledgement-label",
                    "gate",
                )?);
            }
            "--out" => {
                i += 1;
                out = non_empty_path_arg(args, i, "--out", "gate")?;
            }
            "--out-md" => {
                i += 1;
                out_md = Some(non_empty_path_arg(args, i, "--out-md", "gate")?);
            }
            other => return Err(format!("unknown gate argument {other:?}")),
        }
        i += 1;
    }

    let out_md = out_md.unwrap_or_else(|| output::gate::markdown_path_for(&out));
    Ok(GateOptions {
        input: output::gate::GateEvaluateInput {
            root,
            repo_exposure,
            pr_guidance: pr_guidance
                .ok_or_else(|| "gate evaluate requires --pr-guidance <path>".to_string())?,
            sarif_policy,
            labels_json,
            labels,
            agent_verify,
            agent_receipt,
            recommendation_calibration,
            mutation_calibration,
            baseline,
            mode,
            acknowledgement_labels,
        },
        out,
        out_md,
    })
}

fn parse_baseline_create_options(args: &[String]) -> Result<BaselineCreateOptions, String> {
    let mut from = None;
    let mut out = PathBuf::from(output::baseline::DEFAULT_BASELINE_OUT);
    let mut dry_run = false;
    let mut force = false;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--from" => {
                i += 1;
                from = Some(non_empty_path_arg(args, i, "--from", "baseline create")?);
            }
            "--out" => {
                i += 1;
                out = non_empty_path_arg(args, i, "--out", "baseline create")?;
            }
            "--dry-run" => dry_run = true,
            "--force" => force = true,
            other => return Err(format!("unknown baseline create argument {other:?}")),
        }
        i += 1;
    }

    Ok(BaselineCreateOptions {
        from: from.ok_or_else(|| "baseline create requires --from <path>".to_string())?,
        out,
        dry_run,
        force,
    })
}

fn parse_baseline_diff_options(args: &[String]) -> Result<BaselineDiffOptions, String> {
    let mut baseline = None;
    let mut current = None;
    let mut out = PathBuf::from(output::baseline_delta::DEFAULT_BASELINE_DELTA_OUT);
    let mut out_md = PathBuf::from(output::baseline_delta::DEFAULT_BASELINE_DELTA_MD_OUT);

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--baseline" => {
                i += 1;
                baseline = Some(non_empty_path_arg(args, i, "--baseline", "baseline diff")?);
            }
            "--current" => {
                i += 1;
                current = Some(non_empty_path_arg(args, i, "--current", "baseline diff")?);
            }
            "--out" => {
                i += 1;
                out = non_empty_path_arg(args, i, "--out", "baseline diff")?;
            }
            "--out-md" => {
                i += 1;
                out_md = non_empty_path_arg(args, i, "--out-md", "baseline diff")?;
            }
            other => return Err(format!("unknown baseline diff argument {other:?}")),
        }
        i += 1;
    }

    Ok(BaselineDiffOptions {
        baseline: baseline.ok_or_else(|| "baseline diff requires --baseline <path>".to_string())?,
        current: current.ok_or_else(|| "baseline diff requires --current <path>".to_string())?,
        out,
        out_md,
    })
}

fn parse_baseline_update_options(args: &[String]) -> Result<BaselineUpdateOptions, String> {
    let mut baseline = None;
    let mut current = None;
    let mut out = None;
    let mut remove_resolved = false;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--baseline" => {
                i += 1;
                baseline = Some(non_empty_path_arg(
                    args,
                    i,
                    "--baseline",
                    "baseline update",
                )?);
            }
            "--current" => {
                i += 1;
                current = Some(non_empty_path_arg(args, i, "--current", "baseline update")?);
            }
            "--out" => {
                i += 1;
                out = Some(non_empty_path_arg(args, i, "--out", "baseline update")?);
            }
            "--remove-resolved" => remove_resolved = true,
            other => return Err(format!("unknown baseline update argument {other:?}")),
        }
        i += 1;
    }

    Ok(BaselineUpdateOptions {
        baseline: baseline
            .ok_or_else(|| "baseline update requires --baseline <path>".to_string())?,
        current: current.ok_or_else(|| "baseline update requires --current <path>".to_string())?,
        out,
        remove_resolved,
    })
}

fn parse_ripr_zero_status_options(args: &[String]) -> Result<RiprZeroStatusOptions, String> {
    let mut baseline = None;
    let mut delta = None;
    let mut gate = None;
    let mut pr_guidance = None;
    let mut recommendation_calibration = None;
    let mut out = PathBuf::from(output::ripr_zero_status::DEFAULT_RIPR_ZERO_STATUS_OUT);
    let mut out_md = PathBuf::from(output::ripr_zero_status::DEFAULT_RIPR_ZERO_STATUS_MD_OUT);

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--baseline" => {
                i += 1;
                baseline = Some(non_empty_path_arg(args, i, "--baseline", "zero status")?);
            }
            "--delta" => {
                i += 1;
                delta = Some(non_empty_path_arg(args, i, "--delta", "zero status")?);
            }
            "--gate" => {
                i += 1;
                gate = Some(non_empty_path_arg(args, i, "--gate", "zero status")?);
            }
            "--pr-guidance" => {
                i += 1;
                pr_guidance = Some(non_empty_path_arg(args, i, "--pr-guidance", "zero status")?);
            }
            "--recommendation-calibration" => {
                i += 1;
                recommendation_calibration = Some(non_empty_path_arg(
                    args,
                    i,
                    "--recommendation-calibration",
                    "zero status",
                )?);
            }
            "--out" => {
                i += 1;
                out = non_empty_path_arg(args, i, "--out", "zero status")?;
            }
            "--out-md" => {
                i += 1;
                out_md = non_empty_path_arg(args, i, "--out-md", "zero status")?;
            }
            other => return Err(format!("unknown zero status argument {other:?}")),
        }
        i += 1;
    }

    Ok(RiprZeroStatusOptions {
        baseline,
        delta: delta.ok_or_else(|| "zero status requires --delta <path>".to_string())?,
        gate,
        pr_guidance,
        recommendation_calibration,
        out,
        out_md,
    })
}

fn parse_pr_evidence_ledger_options(args: &[String]) -> Result<PrEvidenceLedgerOptions, String> {
    let mut pr_number = None;
    let mut base = None;
    let mut head = None;
    let mut labels = Vec::new();
    let mut gate = None;
    let mut baseline_delta = None;
    let mut zero_status = None;
    let mut pr_guidance = None;
    let mut recommendation_calibration = None;
    let mut agent_receipt = None;
    let mut coverage = None;
    let mut history = None;
    let mut out = PathBuf::from(output::pr_evidence_ledger::DEFAULT_PR_EVIDENCE_LEDGER_OUT);
    let mut out_md = PathBuf::from(output::pr_evidence_ledger::DEFAULT_PR_EVIDENCE_LEDGER_MD_OUT);

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--pr-number" => {
                i += 1;
                pr_number = Some(non_empty_string_arg(
                    args,
                    i,
                    "--pr-number",
                    "pr-ledger record",
                )?);
            }
            "--base" => {
                i += 1;
                base = Some(non_empty_string_arg(args, i, "--base", "pr-ledger record")?);
            }
            "--head" => {
                i += 1;
                head = Some(non_empty_string_arg(args, i, "--head", "pr-ledger record")?);
            }
            "--label" => {
                i += 1;
                labels.push(non_empty_string_arg(
                    args,
                    i,
                    "--label",
                    "pr-ledger record",
                )?);
            }
            "--gate" => {
                i += 1;
                gate = Some(non_empty_path_arg(args, i, "--gate", "pr-ledger record")?);
            }
            "--baseline-delta" => {
                i += 1;
                baseline_delta = Some(non_empty_path_arg(
                    args,
                    i,
                    "--baseline-delta",
                    "pr-ledger record",
                )?);
            }
            "--zero-status" => {
                i += 1;
                zero_status = Some(non_empty_path_arg(
                    args,
                    i,
                    "--zero-status",
                    "pr-ledger record",
                )?);
            }
            "--pr-guidance" => {
                i += 1;
                pr_guidance = Some(non_empty_path_arg(
                    args,
                    i,
                    "--pr-guidance",
                    "pr-ledger record",
                )?);
            }
            "--recommendation-calibration" => {
                i += 1;
                recommendation_calibration = Some(non_empty_path_arg(
                    args,
                    i,
                    "--recommendation-calibration",
                    "pr-ledger record",
                )?);
            }
            "--agent-receipt" => {
                i += 1;
                agent_receipt = Some(non_empty_path_arg(
                    args,
                    i,
                    "--agent-receipt",
                    "pr-ledger record",
                )?);
            }
            "--coverage" => {
                i += 1;
                coverage = Some(non_empty_path_arg(
                    args,
                    i,
                    "--coverage",
                    "pr-ledger record",
                )?);
            }
            "--history" => {
                i += 1;
                history = Some(non_empty_path_arg(
                    args,
                    i,
                    "--history",
                    "pr-ledger record",
                )?);
            }
            "--out" => {
                i += 1;
                out = non_empty_path_arg(args, i, "--out", "pr-ledger record")?;
            }
            "--out-md" => {
                i += 1;
                out_md = non_empty_path_arg(args, i, "--out-md", "pr-ledger record")?;
            }
            other => return Err(format!("unknown pr-ledger record argument {other:?}")),
        }
        i += 1;
    }

    if gate.is_none() && baseline_delta.is_none() && zero_status.is_none() && pr_guidance.is_none()
    {
        return Err(
            "pr-ledger record requires at least one of --gate, --baseline-delta, --zero-status, or --pr-guidance"
                .to_string(),
        );
    }

    Ok(PrEvidenceLedgerOptions {
        pr_number: pr_number
            .ok_or_else(|| "pr-ledger record requires --pr-number <value>".to_string())?,
        base: base.ok_or_else(|| "pr-ledger record requires --base <revision>".to_string())?,
        head: head.ok_or_else(|| "pr-ledger record requires --head <revision>".to_string())?,
        labels,
        gate,
        baseline_delta,
        zero_status,
        pr_guidance,
        recommendation_calibration,
        agent_receipt,
        coverage,
        history,
        out,
        out_md,
    })
}

fn parse_pr_comments_plan_options(args: &[String]) -> Result<PrCommentsPlanOptions, String> {
    let mut root = ".".to_string();
    let mut pr_guidance = None;
    let mut existing_comments = None;
    let mut mode = output::pr_inline_comment_publish_plan::CommentMode::Off;
    let mut pull_request = None;
    let mut event_name = None;
    let mut head_repo = None;
    let mut base_repo = None;
    let mut token_available = false;
    let mut write_permission = true;
    let mut max_inline_comments =
        output::pr_inline_comment_publish_plan::DEFAULT_MAX_INLINE_COMMENTS;
    let mut out =
        PathBuf::from(output::pr_inline_comment_publish_plan::DEFAULT_COMMENT_PUBLISH_PLAN_OUT);
    let mut out_md =
        PathBuf::from(output::pr_inline_comment_publish_plan::DEFAULT_COMMENT_PUBLISH_PLAN_MD_OUT);

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = non_empty_string_arg(args, i, "--root", "pr-comments plan")?;
            }
            "--pr-guidance" => {
                i += 1;
                pr_guidance = Some(non_empty_path_arg(
                    args,
                    i,
                    "--pr-guidance",
                    "pr-comments plan",
                )?);
            }
            "--existing-comments" => {
                i += 1;
                existing_comments = Some(non_empty_path_arg(
                    args,
                    i,
                    "--existing-comments",
                    "pr-comments plan",
                )?);
            }
            "--mode" => {
                i += 1;
                mode = output::pr_inline_comment_publish_plan::CommentMode::parse(expect_value(
                    args, i, "--mode",
                )?)?;
            }
            "--pull-request" => {
                i += 1;
                let value = non_empty_string_arg(args, i, "--pull-request", "pr-comments plan")?;
                pull_request = Some(value.parse::<u64>().map_err(|err| {
                    format!("pr-comments plan --pull-request must be a positive integer: {err}")
                })?);
            }
            "--event-name" => {
                i += 1;
                event_name = Some(non_empty_string_arg(
                    args,
                    i,
                    "--event-name",
                    "pr-comments plan",
                )?);
            }
            "--head-repo" => {
                i += 1;
                head_repo = Some(non_empty_string_arg(
                    args,
                    i,
                    "--head-repo",
                    "pr-comments plan",
                )?);
            }
            "--base-repo" => {
                i += 1;
                base_repo = Some(non_empty_string_arg(
                    args,
                    i,
                    "--base-repo",
                    "pr-comments plan",
                )?);
            }
            "--token-available" => token_available = true,
            "--no-token" => token_available = false,
            "--write-permission" => write_permission = true,
            "--no-write-permission" => write_permission = false,
            "--max-inline-comments" => {
                i += 1;
                let value =
                    non_empty_string_arg(args, i, "--max-inline-comments", "pr-comments plan")?;
                max_inline_comments = value.parse::<usize>().map_err(|err| {
                    format!("pr-comments plan --max-inline-comments must be a number: {err}")
                })?;
            }
            "--out" => {
                i += 1;
                out = non_empty_path_arg(args, i, "--out", "pr-comments plan")?;
            }
            "--out-md" => {
                i += 1;
                out_md = non_empty_path_arg(args, i, "--out-md", "pr-comments plan")?;
            }
            other => return Err(format!("unknown pr-comments plan argument {other:?}")),
        }
        i += 1;
    }

    if max_inline_comments == 0 {
        return Err("pr-comments plan --max-inline-comments must be greater than zero".to_string());
    }

    Ok(PrCommentsPlanOptions {
        root,
        pr_guidance,
        existing_comments,
        mode,
        pull_request,
        event_name,
        head_repo,
        base_repo,
        token_available,
        write_permission,
        max_inline_comments,
        out,
        out_md,
    })
}

fn parse_pr_review_front_panel_options(
    args: &[String],
) -> Result<PrReviewFrontPanelOptions, String> {
    let mut root = ".".to_string();
    let mut pr_guidance = None;
    let mut first_action = None;
    let mut assistant_proof = None;
    let mut assistant_health = None;
    let mut ledger = None;
    let mut baseline_delta = None;
    let mut zero_status = None;
    let mut gate_decision = None;
    let mut recommendation_calibration = None;
    let mut mutation_calibration = None;
    let mut coverage_frontier = None;
    let mut receipt = None;
    let mut out = PathBuf::from(output::pr_review_front_panel::DEFAULT_PR_REVIEW_FRONT_PANEL_OUT);
    let mut out_md =
        PathBuf::from(output::pr_review_front_panel::DEFAULT_PR_REVIEW_FRONT_PANEL_MD_OUT);

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = non_empty_string_arg(args, i, "--root", "pr-review front-panel")?;
            }
            "--pr-guidance" => {
                i += 1;
                pr_guidance = Some(non_empty_path_arg(
                    args,
                    i,
                    "--pr-guidance",
                    "pr-review front-panel",
                )?);
            }
            "--first-action" => {
                i += 1;
                first_action = Some(non_empty_path_arg(
                    args,
                    i,
                    "--first-action",
                    "pr-review front-panel",
                )?);
            }
            "--assistant-proof" => {
                i += 1;
                assistant_proof = Some(non_empty_path_arg(
                    args,
                    i,
                    "--assistant-proof",
                    "pr-review front-panel",
                )?);
            }
            "--assistant-health" => {
                i += 1;
                assistant_health = Some(non_empty_path_arg(
                    args,
                    i,
                    "--assistant-health",
                    "pr-review front-panel",
                )?);
            }
            "--ledger" => {
                i += 1;
                ledger = Some(non_empty_path_arg(
                    args,
                    i,
                    "--ledger",
                    "pr-review front-panel",
                )?);
            }
            "--baseline-delta" => {
                i += 1;
                baseline_delta = Some(non_empty_path_arg(
                    args,
                    i,
                    "--baseline-delta",
                    "pr-review front-panel",
                )?);
            }
            "--zero-status" => {
                i += 1;
                zero_status = Some(non_empty_path_arg(
                    args,
                    i,
                    "--zero-status",
                    "pr-review front-panel",
                )?);
            }
            "--gate-decision" => {
                i += 1;
                gate_decision = Some(non_empty_path_arg(
                    args,
                    i,
                    "--gate-decision",
                    "pr-review front-panel",
                )?);
            }
            "--recommendation-calibration" => {
                i += 1;
                recommendation_calibration = Some(non_empty_path_arg(
                    args,
                    i,
                    "--recommendation-calibration",
                    "pr-review front-panel",
                )?);
            }
            "--mutation-calibration" => {
                i += 1;
                mutation_calibration = Some(non_empty_path_arg(
                    args,
                    i,
                    "--mutation-calibration",
                    "pr-review front-panel",
                )?);
            }
            "--coverage-frontier" => {
                i += 1;
                coverage_frontier = Some(non_empty_path_arg(
                    args,
                    i,
                    "--coverage-frontier",
                    "pr-review front-panel",
                )?);
            }
            "--receipt" => {
                i += 1;
                receipt = Some(non_empty_path_arg(
                    args,
                    i,
                    "--receipt",
                    "pr-review front-panel",
                )?);
            }
            "--out" => {
                i += 1;
                out = non_empty_path_arg(args, i, "--out", "pr-review front-panel")?;
            }
            "--out-md" => {
                i += 1;
                out_md = non_empty_path_arg(args, i, "--out-md", "pr-review front-panel")?;
            }
            other => return Err(format!("unknown pr-review front-panel argument {other:?}")),
        }
        i += 1;
    }

    if pr_guidance.is_none()
        && first_action.is_none()
        && assistant_proof.is_none()
        && assistant_health.is_none()
        && ledger.is_none()
        && baseline_delta.is_none()
        && zero_status.is_none()
        && gate_decision.is_none()
        && recommendation_calibration.is_none()
        && mutation_calibration.is_none()
        && coverage_frontier.is_none()
        && receipt.is_none()
    {
        return Err(
            "pr-review front-panel requires at least one explicit artifact input".to_string(),
        );
    }

    Ok(PrReviewFrontPanelOptions {
        root,
        pr_guidance,
        first_action,
        assistant_proof,
        assistant_health,
        ledger,
        baseline_delta,
        zero_status,
        gate_decision,
        recommendation_calibration,
        mutation_calibration,
        coverage_frontier,
        receipt,
        out,
        out_md,
    })
}

fn parse_report_packet_index_options(args: &[String]) -> Result<ReportPacketIndexOptions, String> {
    let mut root = ".".to_string();
    let mut reports_dir = PathBuf::from("target/ripr/reports");
    let mut review_dir = PathBuf::from("target/ripr/review");
    let mut receipts_dir = PathBuf::from("target/ripr/receipts");
    let mut workflow_dir = PathBuf::from("target/ripr/workflow");
    let mut agent_dir = PathBuf::from("target/ripr/agent");
    let mut pilot_dir = PathBuf::from("target/ripr/pilot");
    let mut ci_dir = PathBuf::from("target/ci");
    let mut out = PathBuf::from(output::report_packet_index::DEFAULT_REPORT_PACKET_INDEX_OUT);
    let mut out_md = PathBuf::from(output::report_packet_index::DEFAULT_REPORT_PACKET_INDEX_MD_OUT);

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = non_empty_string_arg(args, i, "--root", "reports index")?;
            }
            "--reports-dir" => {
                i += 1;
                reports_dir = non_empty_path_arg(args, i, "--reports-dir", "reports index")?;
            }
            "--review-dir" => {
                i += 1;
                review_dir = non_empty_path_arg(args, i, "--review-dir", "reports index")?;
            }
            "--receipts-dir" => {
                i += 1;
                receipts_dir = non_empty_path_arg(args, i, "--receipts-dir", "reports index")?;
            }
            "--workflow-dir" => {
                i += 1;
                workflow_dir = non_empty_path_arg(args, i, "--workflow-dir", "reports index")?;
            }
            "--agent-dir" => {
                i += 1;
                agent_dir = non_empty_path_arg(args, i, "--agent-dir", "reports index")?;
            }
            "--pilot-dir" => {
                i += 1;
                pilot_dir = non_empty_path_arg(args, i, "--pilot-dir", "reports index")?;
            }
            "--ci-dir" => {
                i += 1;
                ci_dir = non_empty_path_arg(args, i, "--ci-dir", "reports index")?;
            }
            "--out" => {
                i += 1;
                out = non_empty_path_arg(args, i, "--out", "reports index")?;
            }
            "--out-md" => {
                i += 1;
                out_md = non_empty_path_arg(args, i, "--out-md", "reports index")?;
            }
            other => return Err(format!("unknown reports index argument {other:?}")),
        }
        i += 1;
    }

    Ok(ReportPacketIndexOptions {
        root,
        reports_dir,
        review_dir,
        receipts_dir,
        workflow_dir,
        agent_dir,
        pilot_dir,
        ci_dir,
        out,
        out_md,
    })
}

fn parse_coverage_grip_frontier_options(
    args: &[String],
) -> Result<CoverageGripFrontierOptions, String> {
    let mut coverage = None;
    let mut ledger = None;
    let mut baseline_delta = None;
    let mut zero_status = None;
    let mut out = PathBuf::from(output::coverage_grip_frontier::DEFAULT_COVERAGE_GRIP_FRONTIER_OUT);
    let mut out_md =
        PathBuf::from(output::coverage_grip_frontier::DEFAULT_COVERAGE_GRIP_FRONTIER_MD_OUT);

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--coverage" => {
                i += 1;
                coverage = Some(non_empty_path_arg(
                    args,
                    i,
                    "--coverage",
                    "coverage-grip frontier",
                )?);
            }
            "--ledger" => {
                i += 1;
                ledger = Some(non_empty_path_arg(
                    args,
                    i,
                    "--ledger",
                    "coverage-grip frontier",
                )?);
            }
            "--baseline-delta" => {
                i += 1;
                baseline_delta = Some(non_empty_path_arg(
                    args,
                    i,
                    "--baseline-delta",
                    "coverage-grip frontier",
                )?);
            }
            "--zero-status" => {
                i += 1;
                zero_status = Some(non_empty_path_arg(
                    args,
                    i,
                    "--zero-status",
                    "coverage-grip frontier",
                )?);
            }
            "--out" => {
                i += 1;
                out = non_empty_path_arg(args, i, "--out", "coverage-grip frontier")?;
            }
            "--out-md" => {
                i += 1;
                out_md = non_empty_path_arg(args, i, "--out-md", "coverage-grip frontier")?;
            }
            other => return Err(format!("unknown coverage-grip frontier argument {other:?}")),
        }
        i += 1;
    }

    if ledger.is_none() && baseline_delta.is_none() && zero_status.is_none() {
        return Err(
            "coverage-grip frontier requires at least one of --ledger, --baseline-delta, or --zero-status"
                .to_string(),
        );
    }

    Ok(CoverageGripFrontierOptions {
        coverage,
        ledger,
        baseline_delta,
        zero_status,
        out,
        out_md,
    })
}

fn parse_assistant_loop_proof_options(
    args: &[String],
) -> Result<AssistantLoopProofOptions, String> {
    let mut root = ".".to_string();
    let mut pr_guidance = None;
    let mut agent_packet = None;
    let mut before = None;
    let mut after = None;
    let mut receipt = None;
    let mut ledger = None;
    let mut coverage_frontier = None;
    let mut gate_decision = None;
    let mut out =
        PathBuf::from(output::test_oracle_assistant_proof::DEFAULT_TEST_ORACLE_ASSISTANT_PROOF_OUT);
    let mut out_md = PathBuf::from(
        output::test_oracle_assistant_proof::DEFAULT_TEST_ORACLE_ASSISTANT_PROOF_MD_OUT,
    );

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = non_empty_string_arg(args, i, "--root", "assistant-loop proof")?;
            }
            "--pr-guidance" => {
                i += 1;
                pr_guidance = Some(non_empty_path_arg(
                    args,
                    i,
                    "--pr-guidance",
                    "assistant-loop proof",
                )?);
            }
            "--agent-packet" => {
                i += 1;
                agent_packet = Some(non_empty_path_arg(
                    args,
                    i,
                    "--agent-packet",
                    "assistant-loop proof",
                )?);
            }
            "--before" => {
                i += 1;
                before = Some(non_empty_path_arg(
                    args,
                    i,
                    "--before",
                    "assistant-loop proof",
                )?);
            }
            "--after" => {
                i += 1;
                after = Some(non_empty_path_arg(
                    args,
                    i,
                    "--after",
                    "assistant-loop proof",
                )?);
            }
            "--receipt" => {
                i += 1;
                receipt = Some(non_empty_path_arg(
                    args,
                    i,
                    "--receipt",
                    "assistant-loop proof",
                )?);
            }
            "--ledger" => {
                i += 1;
                ledger = Some(non_empty_path_arg(
                    args,
                    i,
                    "--ledger",
                    "assistant-loop proof",
                )?);
            }
            "--coverage-frontier" => {
                i += 1;
                coverage_frontier = Some(non_empty_path_arg(
                    args,
                    i,
                    "--coverage-frontier",
                    "assistant-loop proof",
                )?);
            }
            "--gate-decision" => {
                i += 1;
                gate_decision = Some(non_empty_path_arg(
                    args,
                    i,
                    "--gate-decision",
                    "assistant-loop proof",
                )?);
            }
            "--out" => {
                i += 1;
                out = non_empty_path_arg(args, i, "--out", "assistant-loop proof")?;
            }
            "--out-md" => {
                i += 1;
                out_md = non_empty_path_arg(args, i, "--out-md", "assistant-loop proof")?;
            }
            other => return Err(format!("unknown assistant-loop proof argument {other:?}")),
        }
        i += 1;
    }

    if pr_guidance.is_none()
        && agent_packet.is_none()
        && before.is_none()
        && after.is_none()
        && receipt.is_none()
        && ledger.is_none()
    {
        return Err(
            "assistant-loop proof requires at least one explicit artifact input".to_string(),
        );
    }

    Ok(AssistantLoopProofOptions {
        root,
        pr_guidance,
        agent_packet,
        before,
        after,
        receipt,
        ledger,
        coverage_frontier,
        gate_decision,
        out,
        out_md,
    })
}

fn parse_assistant_loop_health_options(
    args: &[String],
) -> Result<AssistantLoopHealthOptions, String> {
    let mut root = ".".to_string();
    let mut proofs = Vec::new();
    let mut out = PathBuf::from(output::assistant_loop_health::DEFAULT_ASSISTANT_LOOP_HEALTH_OUT);
    let mut out_md =
        PathBuf::from(output::assistant_loop_health::DEFAULT_ASSISTANT_LOOP_HEALTH_MD_OUT);

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = non_empty_string_arg(args, i, "--root", "assistant-loop health")?;
            }
            "--proof" => {
                i += 1;
                proofs.push(non_empty_path_arg(
                    args,
                    i,
                    "--proof",
                    "assistant-loop health",
                )?);
            }
            "--out" => {
                i += 1;
                out = non_empty_path_arg(args, i, "--out", "assistant-loop health")?;
            }
            "--out-md" => {
                i += 1;
                out_md = non_empty_path_arg(args, i, "--out-md", "assistant-loop health")?;
            }
            other => return Err(format!("unknown assistant-loop health argument {other:?}")),
        }
        i += 1;
    }

    if proofs.is_empty() {
        return Err("assistant-loop health requires at least one --proof path".to_string());
    }

    Ok(AssistantLoopHealthOptions {
        root,
        proofs,
        out,
        out_md,
    })
}

fn parse_first_action_options(args: &[String]) -> Result<FirstActionOptions, String> {
    let mut root = ".".to_string();
    let mut pr_guidance = None;
    let mut assistant_proof = None;
    let mut ledger = None;
    let mut baseline_delta = None;
    let mut receipt = None;
    let mut gate_decision = None;
    let mut coverage_frontier = None;
    let mut editor_context = None;
    let mut out = PathBuf::from(output::first_useful_action::DEFAULT_FIRST_USEFUL_ACTION_OUT);
    let mut out_md = PathBuf::from(output::first_useful_action::DEFAULT_FIRST_USEFUL_ACTION_MD_OUT);

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                root = non_empty_string_arg(args, i, "--root", "first-action")?;
            }
            "--pr-guidance" => {
                i += 1;
                pr_guidance = Some(non_empty_path_arg(
                    args,
                    i,
                    "--pr-guidance",
                    "first-action",
                )?);
            }
            "--assistant-proof" => {
                i += 1;
                assistant_proof = Some(non_empty_path_arg(
                    args,
                    i,
                    "--assistant-proof",
                    "first-action",
                )?);
            }
            "--ledger" => {
                i += 1;
                ledger = Some(non_empty_path_arg(args, i, "--ledger", "first-action")?);
            }
            "--baseline-delta" => {
                i += 1;
                baseline_delta = Some(non_empty_path_arg(
                    args,
                    i,
                    "--baseline-delta",
                    "first-action",
                )?);
            }
            "--receipt" => {
                i += 1;
                receipt = Some(non_empty_path_arg(args, i, "--receipt", "first-action")?);
            }
            "--gate-decision" => {
                i += 1;
                gate_decision = Some(non_empty_path_arg(
                    args,
                    i,
                    "--gate-decision",
                    "first-action",
                )?);
            }
            "--coverage-frontier" => {
                i += 1;
                coverage_frontier = Some(non_empty_path_arg(
                    args,
                    i,
                    "--coverage-frontier",
                    "first-action",
                )?);
            }
            "--editor-context" => {
                i += 1;
                editor_context = Some(non_empty_path_arg(
                    args,
                    i,
                    "--editor-context",
                    "first-action",
                )?);
            }
            "--out" => {
                i += 1;
                out = non_empty_path_arg(args, i, "--out", "first-action")?;
            }
            "--out-md" => {
                i += 1;
                out_md = non_empty_path_arg(args, i, "--out-md", "first-action")?;
            }
            other => return Err(format!("unknown first-action argument {other:?}")),
        }
        i += 1;
    }

    if pr_guidance.is_none()
        && assistant_proof.is_none()
        && ledger.is_none()
        && baseline_delta.is_none()
        && receipt.is_none()
        && gate_decision.is_none()
        && coverage_frontier.is_none()
        && editor_context.is_none()
    {
        return Err("first-action requires at least one explicit artifact input".to_string());
    }

    Ok(FirstActionOptions {
        root,
        pr_guidance,
        assistant_proof,
        ledger,
        baseline_delta,
        receipt,
        gate_decision,
        coverage_frontier,
        editor_context,
        out,
        out_md,
    })
}

fn baseline_created_at() -> Result<String, String> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("system clock before unix epoch: {err}"))?
        .as_millis();
    Ok(format!("unix_ms:{millis}"))
}

fn first_action_generated_at() -> Result<String, String> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("system clock before unix epoch: {err}"))?
        .as_millis();
    Ok(format!("unix_ms:{millis}"))
}

fn pr_review_front_panel_generated_at() -> Result<String, String> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("system clock before unix epoch: {err}"))?
        .as_millis();
    Ok(format!("unix_ms:{millis}"))
}

fn comment_publish_plan_generated_at() -> Result<String, String> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("system clock before unix epoch: {err}"))?
        .as_millis();
    Ok(format!("unix_ms:{millis}"))
}

fn report_packet_index_generated_at() -> Result<String, String> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("system clock before unix epoch: {err}"))?
        .as_millis();
    Ok(format!("unix_ms:{millis}"))
}

fn assistant_loop_health_generated_at() -> Result<String, String> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("system clock before unix epoch: {err}"))?
        .as_millis();
    Ok(format!("unix_ms:{millis}"))
}

fn read_optional_text_for_report(label: &str, path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|err| {
        format!(
            "read {label} {} failed: {err}",
            output::baseline_delta::display_path(path)
        )
    })
}

fn non_empty_path_arg(
    args: &[String],
    index: usize,
    flag: &str,
    command: &str,
) -> Result<PathBuf, String> {
    let value = non_empty_string_arg(args, index, flag, command)?;
    Ok(PathBuf::from(value))
}

fn non_empty_string_arg(
    args: &[String],
    index: usize,
    flag: &str,
    command: &str,
) -> Result<String, String> {
    let value = expect_value(args, index, flag)?;
    if value.trim().is_empty() {
        Err(format!("{command} {flag} requires a non-empty value"))
    } else {
        Ok(value.to_string())
    }
}

fn parse_outcome_format(value: &str) -> Result<OutcomeFormat, String> {
    match value {
        "md" | "markdown" | "text" => Ok(OutcomeFormat::Markdown),
        "json" => Ok(OutcomeFormat::Json),
        _ => Err(format!("unknown outcome format {value:?}")),
    }
}

fn load_review_comments_diff(root: &Path, base: &str, head: &str) -> Result<String, String> {
    let range = format!("{base}...{head}");
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("diff")
        .arg("--unified=0")
        .arg("--no-ext-diff")
        .arg(&range)
        .output()
        .map_err(|err| format!("failed to run git diff for review-comments: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "git diff for review-comments failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout)
        .map_err(|err| format!("git diff for review-comments was not UTF-8: {err}"))
}

fn review_comments_markdown_path(json_path: &Path) -> PathBuf {
    let mut path = json_path.to_path_buf();
    path.set_extension("md");
    path
}

pub(super) fn check(args: &[String]) -> Result<(), String> {
    let mut input = CheckInput::default();
    let mut explicit = CheckInputExplicit::default();
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                input.root = PathBuf::from(expect_value(args, i, "--root")?);
            }
            "--base" => {
                i += 1;
                input.base = Some(expect_value(args, i, "--base")?.to_string());
            }
            "--diff" => {
                i += 1;
                input.diff_file = Some(PathBuf::from(expect_value(args, i, "--diff")?));
            }
            "--mode" => {
                i += 1;
                input.mode = parse_mode(expect_value(args, i, "--mode")?)?;
                explicit.mode = true;
            }
            "--json" => input.format = OutputFormat::Json,
            "--format" => {
                i += 1;
                input.format = parse_format(expect_value(args, i, "--format")?)?;
            }
            "--no-unchanged-tests" => {
                input.include_unchanged_tests = false;
                explicit.include_unchanged_tests = true;
            }
            "--help" | "-h" => {
                help::print_check_help();
                return Ok(());
            }
            other => return Err(format!("unknown check argument {other:?}")),
        }
        i += 1;
    }
    let config = load_for_root(&input.root)?;
    apply_to_check_input(&mut input, &config, explicit);
    let format = input.format.clone();
    let output = if format.is_repo_seam_inventory() {
        // Repo seam-driven formats do not consume legacy repo `Findings`,
        // so skip `run_repo_analysis` and let `render_check` drive the
        // seam walker directly from `output.root`. The synthesized
        // `CheckOutput` carries only the fields these renderers read.
        app::repo_seam_inventory_input(input)
    } else if format.is_repo_scope() {
        app::check_workspace_repo_with_config(input, &config)?
    } else {
        app::check_workspace_with_config(input, &config)?
    };
    print!(
        "{}",
        app::render_check_with_config(&output, &format, &config)?
    );
    Ok(())
}

pub(super) fn explain(args: &[String]) -> Result<(), String> {
    let mut input = CheckInput::default();
    let mut selector: Option<String> = None;
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                input.root = PathBuf::from(expect_value(args, i, "--root")?);
            }
            "--base" => {
                i += 1;
                input.base = Some(expect_value(args, i, "--base")?.to_string());
            }
            "--diff" => {
                i += 1;
                input.diff_file = Some(PathBuf::from(expect_value(args, i, "--diff")?));
            }
            "--help" | "-h" => {
                help::print_explain_help();
                return Ok(());
            }
            value if selector.is_none() => selector = Some(value.to_string()),
            other => return Err(format!("unexpected explain argument {other:?}")),
        }
        i += 1;
    }
    let selector = selector.ok_or_else(|| "missing finding selector".to_string())?;
    let config = load_for_root(&input.root)?;
    apply_to_check_input(&mut input, &config, CheckInputExplicit::default());
    println!(
        "{}",
        app::explain_finding_with_config(input, &selector, &config)?
    );
    Ok(())
}

pub(super) fn context(args: &[String]) -> Result<(), String> {
    let mut input = CheckInput {
        format: OutputFormat::Json,
        ..CheckInput::default()
    };
    let mut selector: Option<String> = None;
    let mut max_tests = crate::config::DEFAULT_CONTEXT_RELATED_TESTS;
    let mut explicit_max_tests = false;
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                input.root = PathBuf::from(expect_value(args, i, "--root")?);
            }
            "--base" => {
                i += 1;
                input.base = Some(expect_value(args, i, "--base")?.to_string());
            }
            "--diff" => {
                i += 1;
                input.diff_file = Some(PathBuf::from(expect_value(args, i, "--diff")?));
            }
            "--at" => {
                i += 1;
                selector = Some(expect_value(args, i, "--at")?.to_string());
            }
            "--finding" => {
                i += 1;
                selector = Some(expect_value(args, i, "--finding")?.to_string());
            }
            "--max-related-tests" => {
                i += 1;
                max_tests = expect_value(args, i, "--max-related-tests")?
                    .parse::<usize>()
                    .map_err(|err| format!("invalid --max-related-tests: {err}"))?;
                explicit_max_tests = true;
            }
            "--json" => input.format = OutputFormat::Json,
            "--help" | "-h" => {
                help::print_context_help();
                return Ok(());
            }
            other => return Err(format!("unexpected context argument {other:?}")),
        }
        i += 1;
    }
    let selector = selector.ok_or_else(|| "missing --at or --finding selector".to_string())?;
    let config = load_for_root(&input.root)?;
    apply_to_check_input(&mut input, &config, CheckInputExplicit::default());
    if !explicit_max_tests {
        max_tests = config.reports().max_related_tests();
    }
    println!(
        "{}",
        app::collect_context_with_config(input, &selector, max_tests, &config)?
    );
    Ok(())
}

pub(super) fn doctor(args: &[String]) -> Result<(), String> {
    let root = match args {
        [] => PathBuf::from("."),
        [flag] if flag == "--help" || flag == "-h" => {
            help::print_doctor_help();
            return Ok(());
        }
        [flag] if flag == "--root" => return Err("missing value for --root".to_string()),
        [flag, value] if flag == "--root" => PathBuf::from(value),
        [other, ..] => return Err(format!("unknown doctor argument {other:?}")),
    };

    let mut ok = true;
    println!("ripr doctor");
    println!("- root: {}", root.display());

    if root.is_dir() {
        println!("✓ root directory exists");
    } else {
        println!("! root directory does not exist");
        ok = false;
    }

    if root.join("Cargo.toml").exists() {
        println!(
            "✓ Cargo.toml found at {}",
            root.join("Cargo.toml").display()
        );
    } else {
        println!("! no Cargo.toml found at {}", root.display());
        ok = false;
    }

    report_config_status(&root, &mut ok);

    for (tool, args) in [
        ("git", vec!["--version"]),
        ("cargo", vec!["--version"]),
        ("rustc", vec!["--version"]),
    ] {
        match std::process::Command::new(tool).args(&args).output() {
            Ok(output) if output.status.success() => {
                println!("✓ {}", String::from_utf8_lossy(&output.stdout).trim())
            }
            _ => {
                println!("! {tool} not available");
                ok = false;
            }
        }
    }

    if ok {
        println!("✓ doctor checks passed");
        Ok(())
    } else {
        println!("! doctor checks failed; run `ripr doctor --help` for usage");
        Err("doctor found issues".to_string())
    }
}

fn report_config_status(root: &Path, ok: &mut bool) {
    match load_for_root(root) {
        Ok(config) => {
            match config.source_path() {
                Some(path) => {
                    println!("✓ Config: loaded {CONFIG_FILE_NAME}");
                    println!("- Config path: {}", path.display());
                }
                None => println!("✓ Config: not found; using built-in defaults"),
            }
            let analysis_mode = config
                .analysis()
                .mode()
                .map(Mode::as_str)
                .unwrap_or_else(|| Mode::Draft.as_str());
            println!("- Analysis mode default: {analysis_mode}");
            println!(
                "- LSP seam diagnostics default: {}",
                config
                    .lsp()
                    .seam_diagnostics()
                    .unwrap_or(DEFAULT_LSP_SEAM_DIAGNOSTICS)
            );
            println!(
                "- Suppressions path: {}",
                config.suppressions().display_path()
            );
            let languages = config
                .languages()
                .enabled()
                .iter()
                .map(|language| language.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            println!("- Enabled languages: {languages}");
        }
        Err(err) => {
            println!("! Config: invalid {CONFIG_FILE_NAME}");
            println!("- Config path: {}", root.join(CONFIG_FILE_NAME).display());
            println!("  error: {err}");
            *ok = false;
        }
    }
}

pub(super) fn lsp(args: &[String]) -> Result<(), String> {
    for arg in args {
        match arg.as_str() {
            "--stdio" => {}
            "--version" | "-V" => {
                println!("ripr-lsp {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--help" | "-h" => {
                help::print_lsp_help();
                return Ok(());
            }
            other => return Err(format!("unknown lsp argument {other:?}")),
        }
    }
    crate::lsp::serve()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    fn unique_command_test_dir(label: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "ripr-command-{label}-{}-{stamp}",
            std::process::id()
        ))
    }

    fn unique_repo_relative_test_dir(label: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        PathBuf::from("target/ripr").join(format!(
            "ripr-command-{label}-{}-{stamp}",
            std::process::id()
        ))
    }

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    }

    struct GeneratedWorkflowSmokeFixture<'a> {
        commands: &'a [&'a str],
        artifact_paths: &'a [&'a str],
        summary_sections: &'a [&'a str],
        non_blocking_steps: &'a [&'a str],
        optional_sarif_steps: &'a [&'a str],
        forbidden_fragments: &'a [&'a str],
    }

    fn generated_workflow_smoke_fixture() -> GeneratedWorkflowSmokeFixture<'static> {
        GeneratedWorkflowSmokeFixture {
            commands: &[
                "ripr pilot",
                "ripr agent start",
                "ripr agent packet",
                "ripr check",
                "ripr agent verify",
                "ripr agent receipt",
                "ripr outcome",
                "ripr review-comments",
                "gate evaluate",
                "ripr baseline diff",
                "zero status",
                "pr-ledger record",
                "assistant-loop proof",
                "assistant-loop health",
                "first-action",
                "pr-review front-panel",
                "reports index",
                "pr-comments plan",
                "ripr agent status",
                "ripr agent review-summary",
                "cargo xtask operator-cockpit",
            ],
            artifact_paths: &[
                "target/ripr/pilot",
                "target/ripr/agent",
                "target/ripr/workflow",
                "target/ripr/reports",
                "target/ripr/review",
                "target/ripr/workflow/before.repo-exposure.json",
                "target/ripr/workflow/after.repo-exposure.json",
                "target/ripr/workflow/agent-packet.json",
                "target/ripr/workflow/agent-brief.json",
                "target/ripr/workflow/agent-verify.json",
                "target/ripr/reports/agent-receipt.json",
                "target/ripr/workflow/agent-status.json",
                "target/ripr/workflow/agent-status.md",
                "target/ripr/workflow/agent-review-summary.json",
                "target/ripr/workflow/agent-review-summary.md",
                "target/ripr/reports/targeted-test-outcome.json",
                "target/ripr/reports/ripr-findings.sarif",
                "target/ripr/reports/ripr-seams.sarif",
                "target/ripr/reports/repo-ripr-badge.json",
                "target/ripr/reports/repo-ripr-badge-shields.json",
                "target/ripr/reports/gate-decision.json",
                "target/ripr/reports/gate-decision.md",
                "target/ripr/reports/baseline-debt-delta.json",
                "target/ripr/reports/baseline-debt-delta.md",
                "target/ripr/reports/ripr-zero-status.json",
                "target/ripr/reports/ripr-zero-status.md",
                "target/ripr/reports/pr-evidence-ledger.json",
                "target/ripr/reports/pr-evidence-ledger.md",
                "target/ripr/reports/test-oracle-assistant-proof.json",
                "target/ripr/reports/test-oracle-assistant-proof.md",
                "target/ripr/reports/assistant-loop-health.json",
                "target/ripr/reports/assistant-loop-health.md",
                "target/ripr/reports/first-useful-action.json",
                "target/ripr/reports/first-useful-action.md",
                "target/ripr/reports/pr-review-front-panel.json",
                "target/ripr/reports/pr-review-front-panel.md",
                "target/ripr/reports/index.json",
                "target/ripr/reports/index.md",
                "target/ripr/review/comments.json",
                "target/ripr/review/existing-comments.json",
                "target/ripr/review/comment-publish-plan.json",
                "target/ripr/review/comment-publish-plan.md",
                "target/ci/labels.json",
            ],
            summary_sections: &[
                "## RIPR advisory summary",
                "### PR review summary",
                "#### PR review at a glance",
                "### Recommended next test",
                "#### Recommended next test at a glance",
                "### Top recommendation",
                "### Agent review packet",
                "### Artifact packet",
                "### Uploaded review artifacts",
                "#### Uploaded artifacts at a glance",
                "### Gate decision",
                "#### Gate decision at a glance",
                "### Baseline debt delta",
                "#### Baseline debt movement",
                "### RIPR Zero status",
                "#### RIPR Zero at a glance",
                "### PR evidence ledger",
                "#### PR movement at a glance",
                "### Test-oracle assistant proof",
                "#### Assistant proof at a glance",
                "### Agent proof status",
                "#### Agent proof status at a glance",
                "### SARIF and badge status",
                "### PR guidance annotations",
                "### PR inline comments",
                "### Known limits",
            ],
            non_blocking_steps: &[
                "Generate RIPR pilot packet",
                "Prepare RIPR editor-agent artifacts",
                "Generate RIPR agent loop artifacts",
                "Render RIPR diff SARIF",
                "Render RIPR repo seam SARIF",
                "Render RIPR repo badge artifacts",
                "Render RIPR operator cockpit",
                "Render RIPR baseline debt delta",
                "Render RIPR Zero status",
                "Render RIPR PR evidence ledger",
                "Render RIPR test-oracle assistant proof",
                "Render RIPR assistant loop health",
                "Render RIPR first useful action",
                "Render RIPR PR review front panel",
                "Render RIPR report packet index",
                "Render RIPR LLM work-loop summaries",
                "Run RIPR PR guidance report",
                "Capture existing RIPR inline comments",
                "Plan RIPR inline comments",
                "Publish RIPR inline comments",
                "Capture RIPR gate labels",
                "Emit RIPR PR guidance annotations",
                "Add RIPR advisory summary",
                "Upload RIPR report artifacts",
                "Upload RIPR diff findings",
                "Upload RIPR repo seams",
            ],
            optional_sarif_steps: &[
                "Render RIPR diff SARIF",
                "Render RIPR repo seam SARIF",
                "Upload RIPR diff findings",
                "Upload RIPR repo seams",
            ],
            forbidden_fragments: &[
                "fail-on-new-warning",
                "RIPR_PR_COMMENTS",
                "RIPR_GATE_MODE: \"acknowledgeable\"",
                "RIPR_GATE_MODE: \"baseline-check\"",
                "RIPR_GATE_MODE: \"calibrated-gate\"",
            ],
        }
    }

    fn workflow_step<'a>(workflow: &'a str, name: &str) -> &'a str {
        let marker = format!("      - name: {name}");
        let Some(start) = workflow.find(&marker) else {
            return "";
        };
        let rest = &workflow[start..];
        let end = rest.find("\n\n      - ").unwrap_or(rest.len());
        &rest[..end]
    }

    fn assert_contains_all(haystack: &str, label: &str, needles: &[&str]) {
        for needle in needles {
            assert!(
                haystack.contains(needle),
                "generated workflow missing {label} `{needle}`"
            );
        }
    }

    fn assert_step_before(workflow: &str, earlier: &str, later: &str) {
        let earlier_marker = format!("      - name: {earlier}");
        let later_marker = format!("      - name: {later}");
        assert!(
            workflow.contains(&earlier_marker),
            "generated workflow missing step `{earlier}`"
        );
        assert!(
            workflow.contains(&later_marker),
            "generated workflow missing step `{later}`"
        );
        let earlier_index = workflow.find(&earlier_marker).unwrap_or(usize::MAX);
        let later_index = workflow.find(&later_marker).unwrap_or(usize::MAX);
        assert!(
            earlier_index < later_index,
            "`{earlier}` must run before `{later}`"
        );
    }

    #[test]
    fn check_requires_values_for_value_flags() {
        assert_eq!(
            check(&args(&["--diff"])),
            Err("missing value for --diff".to_string())
        );
        assert_eq!(
            check(&args(&["--mode"])),
            Err("missing value for --mode".to_string())
        );
    }

    #[test]
    fn command_help_branches_return_ok() {
        assert_eq!(init(&args(&["--help"])), Ok(()));
        assert_eq!(pilot(&args(&["--help"])), Ok(()));
        assert_eq!(review_comments(&args(&["--help"])), Ok(()));
        assert_eq!(gate(&args(&["--help"])), Ok(()));
        assert_eq!(calibrate(&args(&["--help"])), Ok(()));
        assert_eq!(agent(&args(&["--help"])), Ok(()));
        assert_eq!(agent(&args(&["start", "--help"])), Ok(()));
        assert_eq!(agent(&args(&["brief", "--help"])), Ok(()));
        assert_eq!(agent(&args(&["status", "--help"])), Ok(()));
        assert_eq!(check(&args(&["--help"])), Ok(()));
        assert_eq!(explain(&args(&["--help"])), Ok(()));
        assert_eq!(context(&args(&["--help"])), Ok(()));
        assert_eq!(doctor(&args(&["--help"])), Ok(()));
        assert_eq!(lsp(&args(&["--help"])), Ok(()));
    }

    #[test]
    fn pilot_requires_values_for_value_flags() {
        assert_eq!(
            pilot(&args(&["--root"])),
            Err("missing value for --root".to_string())
        );
        assert_eq!(
            pilot(&args(&["--out"])),
            Err("missing value for --out".to_string())
        );
        assert_eq!(
            pilot(&args(&["--mode"])),
            Err("missing value for --mode".to_string())
        );
        assert_eq!(
            pilot(&args(&["--max-seams"])),
            Err("missing value for --max-seams".to_string())
        );
        assert_eq!(
            pilot(&args(&["--timeout-ms"])),
            Err("missing value for --timeout-ms".to_string())
        );
    }

    #[test]
    fn pilot_rejects_unknown_arguments() {
        assert_eq!(
            pilot(&args(&["--wat"])),
            Err("unknown pilot argument \"--wat\"".to_string())
        );
    }

    #[test]
    fn pilot_rejects_non_positive_max_seams() {
        assert_eq!(
            parse_pilot_options(&args(&["--max-seams", "0"])),
            Err("invalid --max-seams: expected a positive integer".to_string())
        );
    }

    #[test]
    fn pilot_rejects_non_positive_timeout() {
        assert_eq!(
            parse_pilot_options(&args(&["--timeout-ms", "0"])),
            Err("invalid --timeout-ms: expected a positive integer".to_string())
        );
    }

    #[test]
    fn pilot_parses_root_out_mode_max_seams_and_timeout() {
        let options = parse_pilot_options(&args(&[
            "--root",
            "repo",
            "--out",
            "target/pilot",
            "--mode",
            "ready",
            "--max-seams",
            "3",
            "--timeout-ms",
            "120000",
        ]));

        assert_eq!(
            options,
            Ok(PilotOptions {
                root: PathBuf::from("repo"),
                out_dir: PathBuf::from("target/pilot"),
                mode: Mode::Ready,
                explicit: CheckInputExplicit {
                    mode: true,
                    include_unchanged_tests: false,
                },
                max_seams: 3,
                timeout_ms: 120_000,
            })
        );
    }

    #[test]
    fn pilot_analysis_timeout_returns_partial_result() {
        let result = run_pilot_analysis_with_timeout(1, || {
            std::thread::sleep(std::time::Duration::from_millis(20));
            Ok(Vec::new())
        });

        assert!(matches!(result, Ok(PilotAnalysisResult::TimedOut)));
    }

    #[test]
    fn outcome_parses_required_paths_format_and_out() {
        assert_eq!(
            parse_outcome_options(&args(&[
                "--before",
                "before.json",
                "--after",
                "after.json",
                "--format",
                "json",
                "--out",
                "target/ripr/outcome/targeted-test-outcome.json",
            ])),
            Ok(OutcomeOptions {
                before: PathBuf::from("before.json"),
                after: PathBuf::from("after.json"),
                format: OutcomeFormat::Json,
                out: Some(PathBuf::from(
                    "target/ripr/outcome/targeted-test-outcome.json"
                )),
            })
        );
    }

    #[test]
    fn evidence_health_parses_default_and_full_option_surface() {
        assert_eq!(
            parse_evidence_health_options(&args(&[])),
            Ok(EvidenceHealthOptions {
                root: PathBuf::from("."),
                out: PathBuf::from("target/ripr/reports/evidence-health.json"),
                out_md: PathBuf::from("target/ripr/reports/evidence-health.md"),
                mutation_calibration: None,
            })
        );
        assert_eq!(
            parse_evidence_health_options(&args(&[
                "--root",
                "repo",
                "--out",
                "health.json",
                "--out-md",
                "health.md",
                "--mutation-calibration",
                "target/ripr/reports/mutation-calibration.json",
            ])),
            Ok(EvidenceHealthOptions {
                root: PathBuf::from("repo"),
                out: PathBuf::from("health.json"),
                out_md: PathBuf::from("health.md"),
                mutation_calibration: Some(PathBuf::from(
                    "target/ripr/reports/mutation-calibration.json"
                )),
            })
        );
    }

    #[test]
    fn evidence_health_rejects_unknown_arguments() {
        assert_eq!(
            parse_evidence_health_options(&args(&["--bad"])),
            Err("unknown evidence-health argument \"--bad\"".to_string())
        );
    }

    #[test]
    fn review_comments_parses_required_revisions_and_out() {
        assert_eq!(
            parse_review_comments_options(&args(&[
                "--root",
                "repo",
                "--base",
                "origin/main",
                "--head",
                "HEAD",
                "--out",
                "target/ripr/review/comments.json",
            ])),
            Ok(ReviewCommentsOptions {
                root: PathBuf::from("repo"),
                base: "origin/main".to_string(),
                head: "HEAD".to_string(),
                out: PathBuf::from("target/ripr/review/comments.json"),
            })
        );
    }

    #[test]
    fn review_comments_requires_base_and_head() {
        assert_eq!(
            parse_review_comments_options(&args(&["--head", "HEAD"])),
            Err("review-comments requires --base <sha>".to_string())
        );
        assert_eq!(
            parse_review_comments_options(&args(&["--base", "main"])),
            Err("review-comments requires --head <sha>".to_string())
        );
        assert_eq!(
            parse_review_comments_options(&args(&["--base"])),
            Err("missing value for --base".to_string())
        );
    }

    #[test]
    fn review_comments_rejects_empty_values_and_unknown_args() {
        assert_eq!(
            parse_review_comments_options(&args(&["--base", "", "--head", "HEAD"])),
            Err("review-comments --base requires a non-empty revision".to_string())
        );
        assert_eq!(
            parse_review_comments_options(&args(&["--base", "main", "--head", ""])),
            Err("review-comments --head requires a non-empty revision".to_string())
        );
        assert_eq!(
            parse_review_comments_options(&args(&[
                "--base", "main", "--head", "HEAD", "--out", "",
            ])),
            Err("review-comments --out requires a non-empty path".to_string())
        );
        assert_eq!(
            parse_review_comments_options(&args(&["--base", "main", "--head", "HEAD", "--bad"])),
            Err("unknown review-comments argument \"--bad\"".to_string())
        );
    }

    #[test]
    fn review_comments_markdown_path_replaces_json_extension() {
        assert_eq!(
            review_comments_markdown_path(Path::new("target/ripr/review/comments.json")),
            PathBuf::from("target/ripr/review/comments.md")
        );
    }

    #[test]
    fn gate_parses_full_option_surface() {
        let options = parse_gate_options(&args(&[
            "--root",
            "repo",
            "--repo-exposure",
            "target/ripr/reports/repo-exposure.json",
            "--pr-guidance",
            "target/ripr/review/comments.json",
            "--sarif-policy",
            "target/ripr/reports/sarif-policy.json",
            "--labels-json",
            "target/ci/labels.json",
            "--label",
            "ripr-waive",
            "--agent-verify",
            "target/ripr/workflow/agent-verify.json",
            "--agent-receipt",
            "target/ripr/reports/agent-receipt.json",
            "--recommendation-calibration",
            "target/ripr/reports/recommendation-calibration.json",
            "--mutation-calibration",
            "target/ripr/reports/mutation-calibration.json",
            "--baseline",
            "target/ripr/reports/gate-baseline.json",
            "--mode",
            "calibrated-gate",
            "--acknowledgement-label",
            "custom-waive",
            "--out",
            "target/ripr/reports/gate-decision.json",
        ]));

        assert_eq!(
            options,
            Ok(GateOptions {
                input: output::gate::GateEvaluateInput {
                    root: PathBuf::from("repo"),
                    repo_exposure: Some(PathBuf::from("target/ripr/reports/repo-exposure.json")),
                    pr_guidance: PathBuf::from("target/ripr/review/comments.json"),
                    sarif_policy: Some(PathBuf::from("target/ripr/reports/sarif-policy.json")),
                    labels_json: Some(PathBuf::from("target/ci/labels.json")),
                    labels: vec!["ripr-waive".to_string()],
                    agent_verify: Some(PathBuf::from("target/ripr/workflow/agent-verify.json")),
                    agent_receipt: Some(PathBuf::from("target/ripr/reports/agent-receipt.json")),
                    recommendation_calibration: Some(PathBuf::from(
                        "target/ripr/reports/recommendation-calibration.json"
                    )),
                    mutation_calibration: Some(PathBuf::from(
                        "target/ripr/reports/mutation-calibration.json"
                    )),
                    baseline: Some(PathBuf::from("target/ripr/reports/gate-baseline.json")),
                    mode: output::gate::GateMode::CalibratedGate,
                    acknowledgement_labels: vec!["custom-waive".to_string()],
                },
                out: PathBuf::from("target/ripr/reports/gate-decision.json"),
                out_md: PathBuf::from("target/ripr/reports/gate-decision.md"),
            })
        );
    }

    #[test]
    fn gate_requires_pr_guidance_and_rejects_unknown_args() {
        assert_eq!(
            gate(&args(&[])),
            Err("gate requires subcommand `evaluate`".to_string())
        );
        assert_eq!(
            gate(&args(&["inspect"])),
            Err("unknown gate subcommand \"inspect\"; expected `evaluate`".to_string())
        );
        assert_eq!(
            parse_gate_options(&args(&["--mode", "strict"])),
            Err("unknown gate mode `strict`".to_string())
        );
        assert_eq!(
            parse_gate_options(&args(&["--out", ""])),
            Err("gate --out requires a non-empty value".to_string())
        );
        assert_eq!(
            parse_gate_options(&args(&["--bad"])),
            Err("unknown gate argument \"--bad\"".to_string())
        );
        assert_eq!(
            parse_gate_options(&args(&[])),
            Err("gate evaluate requires --pr-guidance <path>".to_string())
        );
    }

    #[test]
    fn gate_command_writes_visible_only_reports() -> Result<(), String> {
        let dir = unique_command_test_dir("gate-visible");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create gate dir: {err}"))?;
        let out = dir.join("gate-decision.json");
        let out_md = dir.join("gate-decision.md");
        gate(&args(&[
            "evaluate",
            "--root",
            &repo_root().display().to_string(),
            "--pr-guidance",
            "fixtures/boundary_gap/expected/pr-guidance/exact-line/comments.json",
            "--out",
            &out.display().to_string(),
            "--out-md",
            &out_md.display().to_string(),
        ]))?;

        let json_text =
            std::fs::read_to_string(&out).map_err(|err| format!("read gate json: {err}"))?;
        let md_text =
            std::fs::read_to_string(&out_md).map_err(|err| format!("read gate md: {err}"))?;
        assert!(json_text.contains("\"status\": \"advisory\""));
        assert!(json_text.contains("\"mode\": \"visible-only\""));
        assert!(md_text.contains("# RIPR Gate Decision"));
        assert!(md_text.contains("Decision: advisory"));
        std::fs::remove_dir_all(&dir).map_err(|err| format!("remove gate dir: {err}"))?;
        Ok(())
    }

    #[test]
    fn gate_command_writes_blocked_report_before_error() -> Result<(), String> {
        let dir = unique_command_test_dir("gate-blocked");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create gate dir: {err}"))?;
        let out = dir.join("gate-decision.json");
        let result = gate(&args(&[
            "evaluate",
            "--root",
            &repo_root().display().to_string(),
            "--pr-guidance",
            "fixtures/boundary_gap/expected/pr-guidance/exact-line/comments.json",
            "--mode",
            "acknowledgeable",
            "--out",
            &out.display().to_string(),
        ]));

        assert!(matches!(result, Err(message) if message.contains("blocked")));
        let json_text =
            std::fs::read_to_string(&out).map_err(|err| format!("read gate json: {err}"))?;
        assert!(json_text.contains("\"status\": \"blocked\""));
        assert!(json_text.contains("\"decision\": \"blocking\""));
        std::fs::remove_dir_all(&dir).map_err(|err| format!("remove gate dir: {err}"))?;
        Ok(())
    }

    #[test]
    fn baseline_create_parses_option_surface() {
        assert_eq!(
            parse_baseline_create_options(&args(&[
                "--from",
                "target/ripr/reports/gate-decision.json",
                "--out",
                ".ripr/gate-baseline.json",
                "--dry-run",
                "--force",
            ])),
            Ok(BaselineCreateOptions {
                from: PathBuf::from("target/ripr/reports/gate-decision.json"),
                out: PathBuf::from(".ripr/gate-baseline.json"),
                dry_run: true,
                force: true,
            })
        );
        assert_eq!(
            parse_baseline_create_options(&args(&["--from", "gate.json"])),
            Ok(BaselineCreateOptions {
                from: PathBuf::from("gate.json"),
                out: PathBuf::from(".ripr/gate-baseline.json"),
                dry_run: false,
                force: false,
            })
        );
    }

    #[test]
    fn baseline_create_requires_source_and_rejects_unknown_args() {
        assert_eq!(
            baseline(&args(&[])),
            Err("baseline requires subcommand `create`, `diff`, or `update`".to_string())
        );
        assert_eq!(
            baseline(&args(&["unknown"])),
            Err(
                "unknown baseline subcommand \"unknown\"; expected `create`, `diff`, or `update`"
                    .to_string()
            )
        );
        assert_eq!(
            parse_baseline_create_options(&args(&[])),
            Err("baseline create requires --from <path>".to_string())
        );
        assert_eq!(
            parse_baseline_create_options(&args(&["--from", ""])),
            Err("baseline create --from requires a non-empty value".to_string())
        );
        assert_eq!(
            parse_baseline_create_options(&args(&["--bad"])),
            Err("unknown baseline create argument \"--bad\"".to_string())
        );
    }

    #[test]
    fn baseline_diff_parses_option_surface() {
        assert_eq!(
            parse_baseline_diff_options(&args(&[
                "--baseline",
                ".ripr/gate-baseline.json",
                "--current",
                "target/ripr/reports/gate-decision.json",
                "--out",
                "target/ripr/reports/baseline-debt-delta.json",
                "--out-md",
                "target/ripr/reports/baseline-debt-delta.md",
            ])),
            Ok(BaselineDiffOptions {
                baseline: PathBuf::from(".ripr/gate-baseline.json"),
                current: PathBuf::from("target/ripr/reports/gate-decision.json"),
                out: PathBuf::from("target/ripr/reports/baseline-debt-delta.json"),
                out_md: PathBuf::from("target/ripr/reports/baseline-debt-delta.md"),
            })
        );
    }

    #[test]
    fn baseline_diff_requires_inputs_and_rejects_unknown_args() {
        assert_eq!(
            parse_baseline_diff_options(&args(&[])),
            Err("baseline diff requires --baseline <path>".to_string())
        );
        assert_eq!(
            parse_baseline_diff_options(&args(&["--baseline", ".ripr/gate-baseline.json"])),
            Err("baseline diff requires --current <path>".to_string())
        );
        assert_eq!(
            parse_baseline_diff_options(&args(&["--baseline", ""])),
            Err("baseline diff --baseline requires a non-empty value".to_string())
        );
        assert_eq!(
            parse_baseline_diff_options(&args(&["--bad"])),
            Err("unknown baseline diff argument \"--bad\"".to_string())
        );
    }

    #[test]
    fn baseline_update_parses_option_surface() {
        assert_eq!(
            parse_baseline_update_options(&args(&[
                "--baseline",
                ".ripr/gate-baseline.json",
                "--current",
                "target/ripr/reports/gate-decision.json",
                "--remove-resolved",
                "--out",
                ".ripr/gate-baseline.updated.json",
            ])),
            Ok(BaselineUpdateOptions {
                baseline: PathBuf::from(".ripr/gate-baseline.json"),
                current: PathBuf::from("target/ripr/reports/gate-decision.json"),
                out: Some(PathBuf::from(".ripr/gate-baseline.updated.json")),
                remove_resolved: true,
            })
        );
        assert_eq!(
            parse_baseline_update_options(&args(&[
                "--baseline",
                ".ripr/gate-baseline.json",
                "--current",
                "target/ripr/reports/gate-decision.json",
            ])),
            Ok(BaselineUpdateOptions {
                baseline: PathBuf::from(".ripr/gate-baseline.json"),
                current: PathBuf::from("target/ripr/reports/gate-decision.json"),
                out: None,
                remove_resolved: false,
            })
        );
    }

    #[test]
    fn baseline_update_requires_inputs_remove_resolved_and_rejects_unknown_args() {
        assert_eq!(
            parse_baseline_update_options(&args(&[])),
            Err("baseline update requires --baseline <path>".to_string())
        );
        assert_eq!(
            parse_baseline_update_options(&args(&["--baseline", ".ripr/gate-baseline.json"])),
            Err("baseline update requires --current <path>".to_string())
        );
        assert_eq!(
            parse_baseline_update_options(&args(&["--baseline", ""])),
            Err("baseline update --baseline requires a non-empty value".to_string())
        );
        assert_eq!(
            parse_baseline_update_options(&args(&["--bad"])),
            Err("unknown baseline update argument \"--bad\"".to_string())
        );
        assert_eq!(
            parse_baseline_update_options(&args(&["--adopt-new"])),
            Err("unknown baseline update argument \"--adopt-new\"".to_string())
        );
        assert_eq!(
            baseline(&args(&[
                "update",
                "--baseline",
                ".ripr/gate-baseline.json",
                "--current",
                "target/ripr/reports/gate-decision.json",
            ])),
            Err(
                "baseline update requires --remove-resolved; adopting new debt is not supported"
                    .to_string()
            )
        );
    }

    #[test]
    fn ripr_zero_status_parses_option_surface() {
        assert_eq!(
            parse_ripr_zero_status_options(&args(&[
                "--baseline",
                ".ripr/gate-baseline.json",
                "--delta",
                "target/ripr/reports/baseline-debt-delta.json",
                "--gate",
                "target/ripr/reports/gate-decision.json",
                "--pr-guidance",
                "target/ripr/review/comments.json",
                "--recommendation-calibration",
                "target/ripr/reports/recommendation-calibration.json",
                "--out",
                "target/ripr/reports/ripr-zero-status.json",
                "--out-md",
                "target/ripr/reports/ripr-zero-status.md",
            ])),
            Ok(RiprZeroStatusOptions {
                baseline: Some(PathBuf::from(".ripr/gate-baseline.json")),
                delta: PathBuf::from("target/ripr/reports/baseline-debt-delta.json"),
                gate: Some(PathBuf::from("target/ripr/reports/gate-decision.json")),
                pr_guidance: Some(PathBuf::from("target/ripr/review/comments.json")),
                recommendation_calibration: Some(PathBuf::from(
                    "target/ripr/reports/recommendation-calibration.json",
                )),
                out: PathBuf::from("target/ripr/reports/ripr-zero-status.json"),
                out_md: PathBuf::from("target/ripr/reports/ripr-zero-status.md"),
            })
        );
    }

    #[test]
    fn ripr_zero_status_requires_inputs_and_rejects_unknown_args() {
        assert_eq!(
            zero(&args(&[])),
            Err("zero requires subcommand `status`".to_string())
        );
        assert_eq!(
            zero(&args(&["unknown"])),
            Err("unknown zero subcommand \"unknown\"; expected `status`".to_string())
        );
        assert_eq!(
            parse_ripr_zero_status_options(&args(&[])),
            Err("zero status requires --delta <path>".to_string())
        );
        assert_eq!(
            parse_ripr_zero_status_options(&args(&["--delta", ""])),
            Err("zero status --delta requires a non-empty value".to_string())
        );
        assert_eq!(
            parse_ripr_zero_status_options(&args(&["--bad"])),
            Err("unknown zero status argument \"--bad\"".to_string())
        );
    }

    #[test]
    fn pr_evidence_ledger_parses_option_surface() {
        assert_eq!(
            parse_pr_evidence_ledger_options(&args(&[
                "--pr-number",
                "123",
                "--base",
                "base",
                "--head",
                "head",
                "--label",
                "ripr-waive",
                "--gate",
                "target/ripr/reports/gate-decision.json",
                "--baseline-delta",
                "target/ripr/reports/baseline-debt-delta.json",
                "--zero-status",
                "target/ripr/reports/ripr-zero-status.json",
                "--pr-guidance",
                "target/ripr/review/comments.json",
                "--recommendation-calibration",
                "target/ripr/reports/recommendation-calibration.json",
                "--agent-receipt",
                "target/ripr/reports/agent-receipt.json",
                "--coverage",
                "target/ripr/reports/coverage-summary.json",
                "--history",
                ".ripr/pr-evidence-ledger.jsonl",
                "--out",
                "target/ripr/reports/pr-evidence-ledger.json",
                "--out-md",
                "target/ripr/reports/pr-evidence-ledger.md",
            ])),
            Ok(PrEvidenceLedgerOptions {
                pr_number: "123".to_string(),
                base: "base".to_string(),
                head: "head".to_string(),
                labels: vec!["ripr-waive".to_string()],
                gate: Some(PathBuf::from("target/ripr/reports/gate-decision.json")),
                baseline_delta: Some(PathBuf::from(
                    "target/ripr/reports/baseline-debt-delta.json"
                )),
                zero_status: Some(PathBuf::from("target/ripr/reports/ripr-zero-status.json")),
                pr_guidance: Some(PathBuf::from("target/ripr/review/comments.json")),
                recommendation_calibration: Some(PathBuf::from(
                    "target/ripr/reports/recommendation-calibration.json"
                )),
                agent_receipt: Some(PathBuf::from("target/ripr/reports/agent-receipt.json")),
                coverage: Some(PathBuf::from("target/ripr/reports/coverage-summary.json")),
                history: Some(PathBuf::from(".ripr/pr-evidence-ledger.jsonl")),
                out: PathBuf::from("target/ripr/reports/pr-evidence-ledger.json"),
                out_md: PathBuf::from("target/ripr/reports/pr-evidence-ledger.md"),
            })
        );
    }

    #[test]
    fn pr_evidence_ledger_requires_identity_and_evidence() {
        assert_eq!(
            pr_ledger(&args(&[])),
            Err("pr-ledger requires subcommand `record`".to_string())
        );
        assert_eq!(
            pr_ledger(&args(&["unknown"])),
            Err("unknown pr-ledger subcommand \"unknown\"; expected `record`".to_string())
        );
        assert_eq!(
            parse_pr_evidence_ledger_options(&args(&[
                "--pr-number",
                "123",
                "--base",
                "base",
                "--head",
                "head"
            ])),
            Err(
                "pr-ledger record requires at least one of --gate, --baseline-delta, --zero-status, or --pr-guidance"
                    .to_string()
            )
        );
        assert_eq!(
            parse_pr_evidence_ledger_options(&args(&[
                "--base",
                "base",
                "--head",
                "head",
                "--gate",
                "gate.json"
            ])),
            Err("pr-ledger record requires --pr-number <value>".to_string())
        );
        assert_eq!(
            parse_pr_evidence_ledger_options(&args(&[
                "--pr-number",
                "",
                "--base",
                "base",
                "--head",
                "head",
                "--gate",
                "gate.json"
            ])),
            Err("pr-ledger record --pr-number requires a non-empty value".to_string())
        );
        assert_eq!(
            parse_pr_evidence_ledger_options(&args(&["--bad"])),
            Err("unknown pr-ledger record argument \"--bad\"".to_string())
        );
    }

    #[test]
    fn pr_comments_plan_parses_option_surface() {
        assert_eq!(
            parse_pr_comments_plan_options(&args(&[
                "--root",
                ".",
                "--pr-guidance",
                "target/ripr/review/comments.json",
                "--existing-comments",
                "target/ripr/review/existing-comments.json",
                "--mode",
                "inline",
                "--pull-request",
                "123",
                "--event-name",
                "pull_request",
                "--head-repo",
                "EffortlessMetrics/ripr",
                "--base-repo",
                "EffortlessMetrics/ripr",
                "--token-available",
                "--no-write-permission",
                "--max-inline-comments",
                "2",
                "--out",
                "target/ripr/review/comment-publish-plan.json",
                "--out-md",
                "target/ripr/review/comment-publish-plan.md",
            ])),
            Ok(PrCommentsPlanOptions {
                root: ".".to_string(),
                pr_guidance: Some(PathBuf::from("target/ripr/review/comments.json")),
                existing_comments: Some(PathBuf::from("target/ripr/review/existing-comments.json")),
                mode: output::pr_inline_comment_publish_plan::CommentMode::Inline,
                pull_request: Some(123),
                event_name: Some("pull_request".to_string()),
                head_repo: Some("EffortlessMetrics/ripr".to_string()),
                base_repo: Some("EffortlessMetrics/ripr".to_string()),
                token_available: true,
                write_permission: false,
                max_inline_comments: 2,
                out: PathBuf::from("target/ripr/review/comment-publish-plan.json"),
                out_md: PathBuf::from("target/ripr/review/comment-publish-plan.md"),
            })
        );
    }

    #[test]
    fn pr_comments_plan_rejects_bad_subcommands_and_options() {
        assert_eq!(
            pr_comments(&args(&[])),
            Err("pr-comments requires subcommand `plan`".to_string())
        );
        assert_eq!(
            pr_comments(&args(&["publish"])),
            Err("unknown pr-comments subcommand \"publish\"; expected `plan`".to_string())
        );
        assert_eq!(
            parse_pr_comments_plan_options(&args(&["--mode", "post"])),
            Err(
                "unknown pr-comments plan mode \"post\"; expected `off`, `plan`, or `inline`"
                    .to_string()
            )
        );
        assert_eq!(
            parse_pr_comments_plan_options(&args(&["--pr-guidance", ""])),
            Err("pr-comments plan --pr-guidance requires a non-empty value".to_string())
        );
        assert_eq!(
            parse_pr_comments_plan_options(&args(&["--max-inline-comments", "0"])),
            Err("pr-comments plan --max-inline-comments must be greater than zero".to_string())
        );
        assert_eq!(
            parse_pr_comments_plan_options(&args(&["--bad"])),
            Err("unknown pr-comments plan argument \"--bad\"".to_string())
        );
    }

    #[test]
    fn pr_comments_plan_writes_json_and_markdown_reports() -> Result<(), String> {
        let dir = unique_command_test_dir("pr-comments-plan");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create temp dir: {err}"))?;
        let comments = dir.join("comments.json");
        let out = dir.join("comment-publish-plan.json");
        let out_md = dir.join("comment-publish-plan.md");
        std::fs::write(
            &comments,
            r#"{"comments":[{"id":"ripr-review-a","dedupe_key":"ripr:a","placement":{"path":"src/lib.rs","line":7,"side":"RIGHT","mode":"exact_seam_line"},"reason":"add focused assertion"}],"summary_only":[],"suppressed":[]}"#,
        )
        .map_err(|err| format!("write comments: {err}"))?;

        pr_comments(&args(&[
            "plan",
            "--pr-guidance",
            &comments.display().to_string(),
            "--mode",
            "plan",
            "--out",
            &out.display().to_string(),
            "--out-md",
            &out_md.display().to_string(),
        ]))?;

        let json = std::fs::read_to_string(&out)
            .map_err(|err| format!("read publish plan JSON: {err}"))?;
        let markdown = std::fs::read_to_string(&out_md)
            .map_err(|err| format!("read publish plan Markdown: {err}"))?;
        assert!(json.contains(r#""kind": "pr_inline_comment_publish_plan""#));
        assert!(json.contains(r#""planned_create": 1"#));
        assert!(markdown.contains("# RIPR Inline Comment Publish Plan"));
        assert!(markdown.contains("publishable comments: 1"));
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn first_action_parses_option_surface() {
        assert_eq!(
            parse_first_action_options(&args(&[
                "--root",
                ".",
                "--pr-guidance",
                "target/ripr/review/comments.json",
                "--assistant-proof",
                "target/ripr/reports/test-oracle-assistant-proof.json",
                "--ledger",
                "target/ripr/reports/pr-evidence-ledger.json",
                "--baseline-delta",
                "target/ripr/reports/baseline-debt-delta.json",
                "--receipt",
                "target/ripr/reports/agent-receipt.json",
                "--gate-decision",
                "target/ripr/reports/gate-decision.json",
                "--coverage-frontier",
                "target/ripr/reports/coverage-grip-frontier.json",
                "--editor-context",
                "target/ripr/workflow/evidence-context.json",
                "--out",
                "target/ripr/reports/first-useful-action.json",
                "--out-md",
                "target/ripr/reports/first-useful-action.md",
            ])),
            Ok(FirstActionOptions {
                root: ".".to_string(),
                pr_guidance: Some(PathBuf::from("target/ripr/review/comments.json")),
                assistant_proof: Some(PathBuf::from(
                    "target/ripr/reports/test-oracle-assistant-proof.json",
                )),
                ledger: Some(PathBuf::from("target/ripr/reports/pr-evidence-ledger.json")),
                baseline_delta: Some(PathBuf::from(
                    "target/ripr/reports/baseline-debt-delta.json",
                )),
                receipt: Some(PathBuf::from("target/ripr/reports/agent-receipt.json")),
                gate_decision: Some(PathBuf::from("target/ripr/reports/gate-decision.json")),
                coverage_frontier: Some(PathBuf::from(
                    "target/ripr/reports/coverage-grip-frontier.json",
                )),
                editor_context: Some(PathBuf::from("target/ripr/workflow/evidence-context.json")),
                out: PathBuf::from("target/ripr/reports/first-useful-action.json"),
                out_md: PathBuf::from("target/ripr/reports/first-useful-action.md"),
            })
        );
    }

    #[test]
    fn first_action_requires_input_and_rejects_unknown_args() {
        assert_eq!(
            parse_first_action_options(&args(&[])),
            Err("first-action requires at least one explicit artifact input".to_string())
        );
        assert_eq!(
            parse_first_action_options(&args(&["--pr-guidance", ""])),
            Err("first-action --pr-guidance requires a non-empty value".to_string())
        );
        assert_eq!(
            parse_first_action_options(&args(&["--bad"])),
            Err("unknown first-action argument \"--bad\"".to_string())
        );
    }

    #[test]
    fn baseline_create_writes_baseline_without_overwriting_by_default() -> Result<(), String> {
        let dir = unique_command_test_dir("baseline-create");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create baseline dir: {err}"))?;
        let out = dir.join("gate-baseline.json");
        let from = repo_root().join(
            "fixtures/boundary_gap/expected/calibrated-gate/visible-only-advisory/gate-decision.json",
        );
        baseline(&args(&[
            "create",
            "--from",
            &from.display().to_string(),
            "--out",
            &out.display().to_string(),
        ]))?;

        let json_text =
            std::fs::read_to_string(&out).map_err(|err| format!("read baseline json: {err}"))?;
        assert!(json_text.contains("\"kind\": \"gate_baseline\""));
        assert!(json_text.contains("\"reviewed\": false"));
        assert!(json_text.contains("\"source_report\""));
        assert!(json_text.contains("\"seam_id\": \"8f7fa8644fd12280\""));
        assert!(json_text.contains("\"entries\": 1"));

        let second = baseline(&args(&[
            "create",
            "--from",
            &from.display().to_string(),
            "--out",
            &out.display().to_string(),
        ]));
        assert!(matches!(second, Err(message) if message.contains("--force")));

        baseline(&args(&[
            "create",
            "--from",
            &from.display().to_string(),
            "--out",
            &out.display().to_string(),
            "--force",
        ]))?;

        std::fs::remove_dir_all(&dir).map_err(|err| format!("remove baseline dir: {err}"))?;
        Ok(())
    }

    #[test]
    fn ripr_zero_status_writes_json_and_markdown_reports() -> Result<(), String> {
        let dir = unique_command_test_dir("ripr-zero-status");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create zero status dir: {err}"))?;
        let out = dir.join("ripr-zero-status.json");
        let out_md = dir.join("ripr-zero-status.md");
        let baseline = repo_root()
            .join("fixtures/boundary_gap/expected/baseline-debt-delta/mixed/baseline.json");
        let delta = repo_root().join(
            "fixtures/boundary_gap/expected/baseline-debt-delta/mixed/baseline-debt-delta.json",
        );

        zero(&args(&[
            "status",
            "--baseline",
            &baseline.display().to_string(),
            "--delta",
            &delta.display().to_string(),
            "--out",
            &out.display().to_string(),
            "--out-md",
            &out_md.display().to_string(),
        ]))?;

        let json_text =
            std::fs::read_to_string(&out).map_err(|err| format!("read zero json: {err}"))?;
        assert!(json_text.contains("\"kind\": \"ripr_zero_status\""));
        assert!(json_text.contains("\"status\": \"advisory\""));
        assert!(json_text.contains("\"baseline_debt_delta\""));

        let markdown =
            std::fs::read_to_string(&out_md).map_err(|err| format!("read zero md: {err}"))?;
        assert!(markdown.starts_with("# RIPR Zero Status"));
        assert!(markdown.contains("Visible unresolved gaps"));

        std::fs::remove_dir_all(&dir).map_err(|err| format!("remove zero status dir: {err}"))?;
        Ok(())
    }

    #[test]
    fn pr_evidence_ledger_writes_json_and_markdown_reports() -> Result<(), String> {
        let dir = unique_command_test_dir("pr-evidence-ledger");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create ledger dir: {err}"))?;
        let out = dir.join("pr-evidence-ledger.json");
        let out_md = dir.join("pr-evidence-ledger.md");
        let fixture = repo_root().join("fixtures/boundary_gap/expected/pr-evidence-ledger/mixed");

        pr_ledger(&args(&[
            "record",
            "--pr-number",
            "123",
            "--base",
            "base",
            "--head",
            "head",
            "--gate",
            &fixture.join("gate-decision.json").display().to_string(),
            "--baseline-delta",
            &fixture
                .join("baseline-debt-delta.json")
                .display()
                .to_string(),
            "--zero-status",
            &fixture.join("ripr-zero-status.json").display().to_string(),
            "--pr-guidance",
            &fixture.join("comments.json").display().to_string(),
            "--agent-receipt",
            &fixture.join("agent-receipt.json").display().to_string(),
            "--history",
            &fixture.join("history.jsonl").display().to_string(),
            "--out",
            &out.display().to_string(),
            "--out-md",
            &out_md.display().to_string(),
        ]))?;

        let json_text =
            std::fs::read_to_string(&out).map_err(|err| format!("read ledger json: {err}"))?;
        assert!(json_text.contains("\"kind\": \"pr_evidence_ledger\""));
        assert!(json_text.contains("\"baseline_resolved\": 3"));
        let md_text =
            std::fs::read_to_string(&out_md).map_err(|err| format!("read ledger md: {err}"))?;
        assert!(md_text.contains("# RIPR PR Evidence Ledger"));
        assert!(md_text.contains("Gate: acknowledgeable / acknowledged"));

        std::fs::remove_dir_all(&dir).map_err(|err| format!("remove ledger dir: {err}"))?;
        Ok(())
    }

    #[test]
    fn coverage_grip_frontier_parses_option_surface() {
        assert_eq!(
            parse_coverage_grip_frontier_options(&args(&[
                "--coverage",
                "target/ripr/reports/coverage-summary.json",
                "--ledger",
                "target/ripr/reports/pr-evidence-ledger.json",
                "--baseline-delta",
                "target/ripr/reports/baseline-debt-delta.json",
                "--zero-status",
                "target/ripr/reports/ripr-zero-status.json",
                "--out",
                "target/ripr/reports/coverage-grip-frontier.json",
                "--out-md",
                "target/ripr/reports/coverage-grip-frontier.md",
            ])),
            Ok(CoverageGripFrontierOptions {
                coverage: Some(PathBuf::from("target/ripr/reports/coverage-summary.json")),
                ledger: Some(PathBuf::from("target/ripr/reports/pr-evidence-ledger.json")),
                baseline_delta: Some(PathBuf::from(
                    "target/ripr/reports/baseline-debt-delta.json"
                )),
                zero_status: Some(PathBuf::from("target/ripr/reports/ripr-zero-status.json")),
                out: PathBuf::from("target/ripr/reports/coverage-grip-frontier.json"),
                out_md: PathBuf::from("target/ripr/reports/coverage-grip-frontier.md"),
            })
        );
    }

    #[test]
    fn coverage_grip_frontier_requires_movement_input() {
        assert_eq!(
            coverage_grip(&args(&[])),
            Err("coverage-grip requires subcommand `frontier`".to_string())
        );
        assert_eq!(
            coverage_grip(&args(&["unknown"])),
            Err("unknown coverage-grip subcommand \"unknown\"; expected `frontier`".to_string())
        );
        assert_eq!(
            parse_coverage_grip_frontier_options(&args(&["--coverage", "coverage.json"])),
            Err(
                "coverage-grip frontier requires at least one of --ledger, --baseline-delta, or --zero-status"
                    .to_string()
            )
        );
        assert_eq!(
            parse_coverage_grip_frontier_options(&args(&["--bad"])),
            Err("unknown coverage-grip frontier argument \"--bad\"".to_string())
        );
    }

    #[test]
    fn coverage_grip_frontier_writes_json_and_markdown_reports() -> Result<(), String> {
        let dir = unique_command_test_dir("coverage-grip-frontier");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create frontier dir: {err}"))?;
        let coverage = dir.join("coverage-summary.json");
        let ledger = repo_root().join(
            "fixtures/boundary_gap/expected/pr-evidence-ledger/mixed/pr-evidence-ledger.json",
        );
        let out = dir.join("coverage-grip-frontier.json");
        let out_md = dir.join("coverage-grip-frontier.md");
        std::fs::write(
            &coverage,
            r#"{"coverage_delta_percent":0.0,"ripr_visible_unresolved_delta":-3}"#,
        )
        .map_err(|err| format!("write coverage: {err}"))?;

        coverage_grip(&args(&[
            "frontier",
            "--coverage",
            &coverage.display().to_string(),
            "--ledger",
            &ledger.display().to_string(),
            "--out",
            &out.display().to_string(),
            "--out-md",
            &out_md.display().to_string(),
        ]))?;

        let rendered =
            std::fs::read_to_string(&out).map_err(|err| format!("read frontier JSON: {err}"))?;
        let markdown = std::fs::read_to_string(&out_md)
            .map_err(|err| format!("read frontier Markdown: {err}"))?;
        assert!(rendered.contains(r#""kind": "coverage_grip_frontier""#));
        assert!(rendered.contains("behavioral grip improved without line-coverage movement"));
        assert!(markdown.contains("# RIPR Coverage / Grip Frontier"));
        std::fs::remove_dir_all(&dir).map_err(|err| format!("remove frontier dir: {err}"))?;
        Ok(())
    }

    #[test]
    fn review_comments_rejects_missing_root_before_loading_diff() -> Result<(), String> {
        let root = unique_command_test_dir("review-comments-missing-root");
        let root_arg = root.display().to_string();
        let result = review_comments_with_diff_loader(
            &args(&["--root", &root_arg, "--base", "main", "--head", "HEAD"]),
            |_root, _base, _head| Ok(String::new()),
        );

        let err = match result {
            Ok(_) => return Err("missing root should be rejected".to_string()),
            Err(err) => err,
        };
        assert!(err.contains("is not a directory"));
        Ok(())
    }

    #[test]
    fn review_comments_returns_diff_loader_errors() -> Result<(), String> {
        let root = unique_command_test_dir("review-comments-diff-error");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        let root_arg = root.display().to_string();
        let result = review_comments_with_diff_loader(
            &args(&["--root", &root_arg, "--base", "main", "--head", "HEAD"]),
            |_root, _base, _head| Err("synthetic diff failure".to_string()),
        );

        assert_eq!(result, Err("synthetic diff failure".to_string()));
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove temp root: {err}"))?;
        Ok(())
    }

    #[test]
    fn review_comments_writes_json_and_markdown_from_loaded_diff() -> Result<(), String> {
        let root = unique_command_test_dir("review-comments");
        std::fs::create_dir_all(root.join("src")).map_err(|err| format!("create src: {err}"))?;
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"review_comments_fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .map_err(|err| format!("write Cargo.toml: {err}"))?;
        std::fs::write(
            root.join("src/lib.rs"),
            "pub fn discounted_total(amount: i32) -> i32 {\n    if amount > 10 { amount - 1 } else { amount }\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn above_threshold_gets_discount() {\n        assert_eq!(discounted_total(11), 10);\n    }\n}\n",
        )
        .map_err(|err| format!("write src/lib.rs: {err}"))?;

        let out = root.join("target/ripr/review/comments.json");
        let root_arg = root.display().to_string();
        let out_arg = out.display().to_string();
        review_comments_with_diff_loader(
            &args(&[
                "--root", &root_arg, "--base", "HEAD~1", "--head", "HEAD", "--out", &out_arg,
            ]),
            |diff_root, base, head| {
                assert_eq!(diff_root, root.as_path());
                assert_eq!(base, "HEAD~1");
                assert_eq!(head, "HEAD");
                Ok("diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -2 +2 @@\n-    if amount >= 10 { amount - 1 } else { amount }\n+    if amount > 10 { amount - 1 } else { amount }\n".to_string())
            },
        )?;

        let rendered_json = std::fs::read_to_string(&out)
            .map_err(|err| format!("read review comments JSON: {err}"))?;
        let rendered_md = std::fs::read_to_string(out.with_extension("md"))
            .map_err(|err| format!("read review comments Markdown: {err}"))?;
        assert!(rendered_json.contains("\"schema_version\": \"0.1\""));
        assert!(rendered_json.contains("\"status\": \"advisory\""));
        assert!(rendered_json.contains("\"base\": \"HEAD~1\""));
        assert!(rendered_json.contains("\"head\": \"HEAD\""));
        assert!(rendered_md.contains("# RIPR PR Guidance"));
        assert!(rendered_md.contains("Advisory static evidence only"));

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove temp root: {err}"))?;
        Ok(())
    }

    #[test]
    fn outcome_defaults_to_markdown_stdout_shape() {
        assert_eq!(
            parse_outcome_options(&args(
                &["--before", "before.json", "--after", "after.json",]
            )),
            Ok(OutcomeOptions {
                before: PathBuf::from("before.json"),
                after: PathBuf::from("after.json"),
                format: OutcomeFormat::Markdown,
                out: None,
            })
        );
    }

    #[test]
    fn outcome_requires_before_and_after() {
        assert_eq!(
            parse_outcome_options(&args(&["--after", "after.json"])),
            Err("outcome requires --before <path>".to_string())
        );
        assert_eq!(
            parse_outcome_options(&args(&["--before", "before.json"])),
            Err("outcome requires --after <path>".to_string())
        );
    }

    #[test]
    fn outcome_help_returns_ok() {
        assert_eq!(outcome(&args(&["--help"])), Ok(()));
    }

    #[test]
    fn evidence_health_help_returns_ok() {
        assert_eq!(evidence_health(&args(&["--help"])), Ok(()));
    }

    #[test]
    fn outcome_command_writes_json_file() -> Result<(), String> {
        let dir = unique_command_test_dir("outcome");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create temp dir: {err}"))?;
        let before = dir.join("before.json");
        let after = dir.join("after.json");
        let out = dir.join("nested/targeted-test-outcome.json");
        std::fs::write(&before, outcome_before_json())
            .map_err(|err| format!("write before snapshot: {err}"))?;
        std::fs::write(&after, outcome_after_json())
            .map_err(|err| format!("write after snapshot: {err}"))?;

        outcome(&args(&[
            "--before",
            &before.display().to_string(),
            "--after",
            &after.display().to_string(),
            "--format",
            "json",
            "--out",
            &out.display().to_string(),
        ]))?;

        let rendered =
            std::fs::read_to_string(&out).map_err(|err| format!("read outcome output: {err}"))?;
        assert!(rendered.contains(r#""schema_version": "0.1""#));
        assert!(rendered.contains(r#""moved": 1"#));
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn outcome_command_reports_read_failures() -> Result<(), String> {
        let dir = unique_command_test_dir("outcome-read");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create temp dir: {err}"))?;
        let before = dir.join("before.json");
        std::fs::write(&before, outcome_before_json())
            .map_err(|err| format!("write before snapshot: {err}"))?;

        let missing_before = outcome(&args(&[
            "--before",
            &dir.join("missing-before.json").display().to_string(),
            "--after",
            &dir.join("missing-after.json").display().to_string(),
        ]));
        assert!(matches!(missing_before, Err(message) if message.contains("read")));

        let missing_after = outcome(&args(&[
            "--before",
            &before.display().to_string(),
            "--after",
            &dir.join("missing-after.json").display().to_string(),
        ]));
        assert!(matches!(missing_after, Err(message) if message.contains("read")));
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn calibrate_parses_required_inputs_format_and_out() {
        assert_eq!(
            parse_calibrate_cargo_mutants_options(&args(&[
                "--mutants-json",
                "target/mutants/outcomes.json",
                "--repo-exposure-json",
                "target/ripr/after.repo-exposure.json",
                "--format",
                "json",
                "--out",
                "target/ripr/calibration/mutation-calibration.json",
            ])),
            Ok(CalibrateOptions {
                mutants_json: PathBuf::from("target/mutants/outcomes.json"),
                repo_exposure_json: PathBuf::from("target/ripr/after.repo-exposure.json"),
                format: CalibrateFormat::Json,
                out: Some(PathBuf::from(
                    "target/ripr/calibration/mutation-calibration.json"
                )),
            })
        );
    }

    #[test]
    fn calibrate_requires_subcommand_and_inputs() {
        assert_eq!(
            calibrate(&args(&[])),
            Err("calibrate requires subcommand `cargo-mutants`".to_string())
        );
        assert_eq!(
            calibrate(&args(&["runtime"])),
            Err("unknown calibrate subcommand \"runtime\"; expected `cargo-mutants`".to_string())
        );
        assert_eq!(
            parse_calibrate_cargo_mutants_options(&args(&["--repo-exposure-json", "repo.json"])),
            Err("calibrate cargo-mutants requires --mutants-json <path>".to_string())
        );
        assert_eq!(
            parse_calibrate_cargo_mutants_options(&args(&["--mutants-json", "mutants.json"])),
            Err("calibrate cargo-mutants requires --repo-exposure-json <path>".to_string())
        );
    }

    #[test]
    fn calibrate_help_returns_ok() {
        assert_eq!(calibrate(&args(&["--help"])), Ok(()));
        assert_eq!(calibrate(&args(&["cargo-mutants", "--help"])), Ok(()));
    }

    #[test]
    fn agent_rejects_unknown_subcommands() {
        assert_eq!(
            agent(&args(&["unknown"])),
            Err(
                "unknown agent subcommand \"unknown\"; expected `start`, `brief`, `packet`, `verify`, `receipt`, `status`, or `review-summary`"
                    .to_string()
            )
        );
    }

    #[test]
    fn agent_start_rejects_missing_root_before_analysis() {
        assert_eq!(
            agent(&args(&[
                "start",
                "--root",
                "target/ripr/missing-agent-start-root",
                "--seam-id",
                "f3c9e4d21a0b7c88",
            ])),
            Err(
                "agent start root target/ripr/missing-agent-start-root is not a directory"
                    .to_string()
            )
        );
    }

    #[test]
    fn agent_status_rejects_missing_root_before_reading_artifacts() {
        assert_eq!(
            agent(&args(&[
                "status",
                "--root",
                "target/ripr/missing-agent-status-root",
                "--json",
            ])),
            Err(
                "agent status root target/ripr/missing-agent-status-root is not a directory"
                    .to_string()
            )
        );
    }

    #[test]
    fn agent_review_summary_rejects_missing_root_before_reading_artifacts() {
        assert_eq!(
            agent(&args(&[
                "review-summary",
                "--root",
                "target/ripr/missing-agent-review-summary-root",
                "--json",
            ])),
            Err(
                "agent review-summary root target/ripr/missing-agent-review-summary-root is not a directory"
                    .to_string()
            )
        );
    }

    #[test]
    fn agent_packet_rejects_missing_root_before_analysis() {
        assert_eq!(
            agent(&args(&[
                "packet",
                "--root",
                "target/ripr/missing-agent-packet-root",
                "--seam-id",
                "f3c9e4d21a0b7c88",
                "--json",
            ])),
            Err(
                "agent packet root target/ripr/missing-agent-packet-root is not a directory"
                    .to_string()
            )
        );
    }

    #[test]
    fn agent_verify_reports_read_failures() -> Result<(), String> {
        let dir = unique_command_test_dir("agent-verify-read");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create temp dir: {err}"))?;
        let before = dir.join("before.json");
        std::fs::write(&before, outcome_before_json())
            .map_err(|err| format!("write before snapshot: {err}"))?;

        let missing_before = agent(&args(&[
            "verify",
            "--root",
            &dir.display().to_string(),
            "--before",
            &dir.join("missing-before.json").display().to_string(),
            "--after",
            &dir.join("missing-after.json").display().to_string(),
            "--json",
        ]));
        assert!(
            matches!(missing_before, Err(message) if message.contains("canonicalize agent verify --before"))
        );

        let missing_after = agent(&args(&[
            "verify",
            "--root",
            &dir.display().to_string(),
            "--before",
            &before.display().to_string(),
            "--after",
            &dir.join("missing-after.json").display().to_string(),
            "--json",
        ]));
        assert!(
            matches!(missing_after, Err(message) if message.contains("canonicalize agent verify --after"))
        );
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn agent_verify_rejects_snapshots_outside_root() -> Result<(), String> {
        let root = unique_command_test_dir("agent-verify-root");
        let outside = unique_command_test_dir("agent-verify-outside");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root dir: {err}"))?;
        std::fs::create_dir_all(&outside).map_err(|err| format!("create outside dir: {err}"))?;
        let before = outside.join("before.json");
        let after = root.join("after.json");
        std::fs::write(&before, outcome_before_json())
            .map_err(|err| format!("write before snapshot: {err}"))?;
        std::fs::write(&after, outcome_after_json())
            .map_err(|err| format!("write after snapshot: {err}"))?;

        let result = agent(&args(&[
            "verify",
            "--root",
            &root.display().to_string(),
            "--before",
            &before.display().to_string(),
            "--after",
            &after.display().to_string(),
            "--json",
        ]));

        assert!(matches!(result, Err(message) if message.contains("must stay under root")));
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&outside);
        Ok(())
    }

    #[test]
    fn agent_receipt_reports_read_failures() -> Result<(), String> {
        let dir = unique_command_test_dir("agent-receipt-read");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create temp dir: {err}"))?;

        let missing = agent(&args(&[
            "receipt",
            "--root",
            &dir.display().to_string(),
            "--verify-json",
            &dir.join("missing-agent-verify.json").display().to_string(),
            "--seam-id",
            "seam-a",
            "--json",
        ]));
        assert!(
            matches!(missing, Err(message) if message.contains("canonicalize agent receipt --verify-json"))
        );
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn agent_receipt_rejects_verify_json_outside_root() -> Result<(), String> {
        let root = unique_command_test_dir("agent-receipt-root");
        let outside = unique_command_test_dir("agent-receipt-outside");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root dir: {err}"))?;
        std::fs::create_dir_all(&outside).map_err(|err| format!("create outside dir: {err}"))?;
        let verify = outside.join("agent-verify.json");
        std::fs::write(&verify, "{}").map_err(|err| format!("write verify JSON: {err}"))?;

        let result = agent(&args(&[
            "receipt",
            "--root",
            &root.display().to_string(),
            "--verify-json",
            &verify.display().to_string(),
            "--seam-id",
            "seam-a",
            "--json",
        ]));

        assert!(matches!(result, Err(message) if message.contains("must stay under root")));
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&outside);
        Ok(())
    }

    #[test]
    fn agent_brief_rejects_missing_root_before_analysis() {
        assert_eq!(
            agent(&args(&[
                "brief",
                "--root",
                "target/ripr/missing-agent-brief-root",
                "--diff",
                "change.diff",
                "--json",
            ])),
            Err(
                "agent brief root target/ripr/missing-agent-brief-root is not a directory"
                    .to_string()
            )
        );
    }

    #[test]
    fn agent_brief_diff_lines_are_normalized_to_requested_root() {
        let diff = "diff --git a/crates/ripr/examples/sample/src/lib.rs b/crates/ripr/examples/sample/src/lib.rs\n--- a/crates/ripr/examples/sample/src/lib.rs\n+++ b/crates/ripr/examples/sample/src/lib.rs\n@@ -8,1 +8,1 @@\n-old\n+new\n";
        let lines = agent_brief_lines_from_diff(Path::new("crates/ripr/examples/sample"), diff);

        assert_eq!(
            lines,
            vec![AgentBriefLine::new(PathBuf::from("src/lib.rs"), 8)]
        );
    }

    #[test]
    fn agent_brief_owner_lines_are_resolved_from_changed_lines() -> Result<(), String> {
        let root = unique_command_test_dir("agent-brief-owner-lines");
        std::fs::create_dir_all(root.join("src")).map_err(|err| format!("create src: {err}"))?;
        std::fs::write(
            root.join("src/lib.rs"),
            "pub fn discounted_total(amount: i32) -> i32 {\n    let discount = 10;\n    amount - discount\n}\n",
        )
        .map_err(|err| format!("write src/lib.rs: {err}"))?;
        let lines = vec![AgentBriefLine::new(PathBuf::from("src/lib.rs"), 3)];

        let owners = agent_brief_owners_for_lines(&root, &lines);

        assert_eq!(owners.len(), 1);
        assert_eq!(owners[0].line, 3);
        assert!(owners[0].owner.ends_with("discounted_total"));
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove temp root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_brief_owner_lines_are_best_effort_for_missing_files() -> Result<(), String> {
        let root = unique_command_test_dir("agent-brief-owner-missing");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        let lines = vec![AgentBriefLine::new(PathBuf::from("src/missing.rs"), 3)];

        let owners = agent_brief_owners_for_lines(&root, &lines);

        assert!(owners.is_empty());
        std::fs::remove_dir_all(&root).map_err(|err| format!("remove temp root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_brief_normalizes_absolute_diff_paths_against_relative_root() -> Result<(), String> {
        let root = unique_repo_relative_test_dir("agent-brief-normalize");
        let src = root.join("src");
        std::fs::create_dir_all(&src).map_err(|err| format!("create src dir: {err}"))?;
        let absolute_file = std::env::current_dir()
            .map_err(|err| format!("read current dir: {err}"))?
            .join(&root)
            .join("src/lib.rs");

        assert_eq!(
            normalize_agent_brief_path(&root, &absolute_file),
            PathBuf::from("src/lib.rs")
        );

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove temp root: {err}"))?;
        Ok(())
    }

    #[test]
    fn agent_brief_diff_path_must_stay_under_root() -> Result<(), String> {
        let root = unique_command_test_dir("agent-brief-root");
        let outside = unique_command_test_dir("agent-brief-outside");
        std::fs::create_dir_all(&root).map_err(|err| format!("create root: {err}"))?;
        std::fs::create_dir_all(&outside).map_err(|err| format!("create outside: {err}"))?;
        let outside_diff = outside.join("change.diff");
        std::fs::write(&outside_diff, "diff --git a/src/lib.rs b/src/lib.rs\n")
            .map_err(|err| format!("write outside diff: {err}"))?;

        let result = resolve_agent_brief_working_set(
            &root,
            &AgentBriefWorkingSet::Diff(outside_diff.clone()),
        );
        let err = match result {
            Ok(_) => return Err("outside diff path should be rejected".to_string()),
            Err(err) => err,
        };

        assert!(
            err.contains("must stay under root"),
            "unexpected error: {err}"
        );

        std::fs::remove_dir_all(&root).map_err(|err| format!("remove root: {err}"))?;
        std::fs::remove_dir_all(&outside).map_err(|err| format!("remove outside: {err}"))?;
        Ok(())
    }

    #[test]
    fn calibrate_command_writes_json_file() -> Result<(), String> {
        let dir = unique_command_test_dir("calibrate");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create temp dir: {err}"))?;
        let repo = dir.join("repo-exposure.json");
        let mutants = dir.join("mutants.json");
        let out = dir.join("nested/mutation-calibration.json");
        std::fs::write(&repo, calibration_repo_json())
            .map_err(|err| format!("write repo exposure: {err}"))?;
        std::fs::write(&mutants, calibration_mutants_json())
            .map_err(|err| format!("write mutants: {err}"))?;

        calibrate(&args(&[
            "cargo-mutants",
            "--mutants-json",
            &mutants.display().to_string(),
            "--repo-exposure-json",
            &repo.display().to_string(),
            "--format",
            "json",
            "--out",
            &out.display().to_string(),
        ]))?;

        let rendered = std::fs::read_to_string(&out)
            .map_err(|err| format!("read calibration output: {err}"))?;
        assert!(rendered.contains(r#""schema_version": "0.1""#));
        assert!(rendered.contains(r#""static_gap_and_runtime_signal": 1"#));
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn calibrate_reads_cargo_mutants_directory() -> Result<(), String> {
        let dir = unique_command_test_dir("calibrate-dir");
        let mutants_dir = dir.join("cargo-mutants");
        std::fs::create_dir_all(&mutants_dir)
            .map_err(|err| format!("create mutants dir: {err}"))?;
        std::fs::write(
            mutants_dir.join("mutants.json"),
            r#"{"mutants":[{"id":"m1","seam_id":"seam-a","operator":"replace"}]}"#,
        )
        .map_err(|err| format!("write mutants.json: {err}"))?;
        std::fs::write(
            mutants_dir.join("outcomes.json"),
            r#"{"outcomes":[{"id":"m1","outcome":"missed"}]}"#,
        )
        .map_err(|err| format!("write outcomes.json: {err}"))?;

        let combined = read_calibration_mutants_json(&mutants_dir)?;
        assert!(combined.contains("mutants"));
        assert!(combined.contains("outcomes"));
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn context_rejects_invalid_max_related_tests() {
        let result = context(&args(&[
            "--at",
            "probe:file.rs:1:predicate",
            "--max-related-tests",
            "many",
        ]));
        assert!(
            matches!(result, Err(message) if message.starts_with("invalid --max-related-tests:"))
        );
    }

    #[test]
    fn doctor_requires_root_value() {
        assert_eq!(
            doctor(&args(&["--root"])),
            Err("missing value for --root".to_string())
        );
    }

    #[test]
    fn init_requires_root_value() {
        assert_eq!(
            init(&args(&["--root"])),
            Err("missing value for --root".to_string())
        );
        assert_eq!(
            init(&args(&["--ci"])),
            Err("missing value for --ci".to_string())
        );
    }

    #[test]
    fn init_rejects_unknown_arguments() {
        assert_eq!(
            init(&args(&["--wat"])),
            Err("unknown init argument \"--wat\"".to_string())
        );
        assert_eq!(
            init(&args(&["--ci", "gitlab"])),
            Err("unknown init --ci provider \"gitlab\"".to_string())
        );
    }

    #[test]
    fn init_parses_root_dry_run_and_force() {
        assert_eq!(
            parse_init_options(&args(&[
                "--root",
                "repo",
                "--dry-run",
                "--force",
                "--ci",
                "github",
            ])),
            Ok(InitOptions {
                root: PathBuf::from("repo"),
                dry_run: true,
                force: true,
                ci: Some(InitCi::Github),
            })
        );
    }

    #[test]
    fn init_generated_github_workflow_is_advisory() {
        let workflow = generated_github_actions_workflow();
        assert!(workflow.contains(
            "continue-on-error: ${{ vars.RIPR_GATE_MODE == '' || vars.RIPR_GATE_MODE == 'visible-only' }}"
        ));
        assert!(workflow.contains("github/codeql-action/upload-sarif@v4"));
        assert!(workflow.contains("actions/upload-artifact@v7"));
        assert!(workflow.contains("RIPR_UPLOAD_SARIF"));
        assert!(workflow.contains("RIPR_GATE_MODE: ${{ vars.RIPR_GATE_MODE || '' }}"));
        assert!(workflow.contains("RIPR_GATE_BASELINE: ${{ vars.RIPR_GATE_BASELINE || '' }}"));
        assert!(workflow.contains("RIPR_COMMENT_MODE: ${{ vars.RIPR_COMMENT_MODE || 'off' }}"));
        assert!(workflow.contains("pull-requests: write"));
        assert!(workflow.contains("--format sarif"));
        assert!(workflow.contains("--format repo-sarif"));
        assert!(workflow.contains("--format repo-badge-json"));
        assert!(workflow.contains("ripr pilot"));
        assert!(workflow.contains("ripr agent start"));
        assert!(workflow.contains("ripr agent packet"));
        assert!(workflow.contains("ripr agent verify"));
        assert!(workflow.contains("ripr agent receipt"));
        assert!(workflow.contains("ripr agent status"));
        assert!(workflow.contains("ripr agent review-summary"));
        assert!(workflow.contains("ripr outcome"));
        assert!(workflow.contains("target/ripr/workflow/agent-packet.json"));
        assert!(workflow.contains("target/ripr/workflow/agent-brief.json"));
        assert!(workflow.contains("target/ripr/workflow/agent-verify.json"));
        assert!(workflow.contains("target/ripr/reports/agent-receipt.json"));
        assert!(workflow.contains("target/ripr/workflow/agent-status.json"));
        assert!(workflow.contains("target/ripr/workflow/agent-status.md"));
        assert!(workflow.contains("target/ripr/workflow/agent-review-summary.json"));
        assert!(workflow.contains("target/ripr/workflow/agent-review-summary.md"));
        assert!(workflow.contains("target/ripr/agent/agent-packet.json"));
        assert!(workflow.contains("target/ripr/agent/agent-brief.json"));
        assert!(workflow.contains("target/ripr/agent/agent-verify.json"));
        assert!(workflow.contains("target/ripr/agent/agent-receipt.json"));
        assert!(workflow.contains("target/ripr/reports/targeted-test-outcome.json"));
        assert!(workflow.contains("target/ripr/reports/gate-decision.json"));
        assert!(workflow.contains("target/ripr/reports/gate-decision.md"));
        assert!(workflow.contains("target/ripr/reports/baseline-debt-delta.json"));
        assert!(workflow.contains("target/ripr/reports/baseline-debt-delta.md"));
        assert!(workflow.contains("target/ripr/reports/ripr-zero-status.json"));
        assert!(workflow.contains("target/ripr/reports/ripr-zero-status.md"));
        assert!(workflow.contains("target/ripr/reports/pr-evidence-ledger.json"));
        assert!(workflow.contains("target/ripr/reports/pr-evidence-ledger.md"));
        assert!(workflow.contains("target/ripr/reports/test-oracle-assistant-proof.json"));
        assert!(workflow.contains("target/ripr/reports/test-oracle-assistant-proof.md"));
        assert!(workflow.contains("target/ripr/reports/assistant-loop-health.json"));
        assert!(workflow.contains("target/ripr/reports/assistant-loop-health.md"));
        assert!(workflow.contains("target/ripr/reports/first-useful-action.json"));
        assert!(workflow.contains("target/ripr/reports/first-useful-action.md"));
        assert!(workflow.contains("target/ripr/reports/pr-review-front-panel.json"));
        assert!(workflow.contains("target/ripr/reports/pr-review-front-panel.md"));
        assert!(workflow.contains("target/ripr/reports/index.json"));
        assert!(workflow.contains("target/ripr/reports/index.md"));
        assert!(workflow.contains("target/ci/labels.json"));
        assert!(workflow.contains("target/ripr/review/comments.json"));
        assert!(workflow.contains("target/ripr/review/existing-comments.json"));
        assert!(workflow.contains("target/ripr/review/comment-publish-plan.json"));
        assert!(workflow.contains("target/ripr/review/comment-publish-plan.md"));
        assert!(workflow.contains("target/ripr/review"));
        assert!(workflow.contains("target/ci"));
        assert!(workflow.contains("name: Capture existing RIPR inline comments"));
        assert!(workflow.contains("name: Plan RIPR inline comments"));
        assert!(workflow.contains("name: Publish RIPR inline comments"));
        assert!(workflow.contains("name: Capture RIPR gate labels"));
        assert!(workflow.contains("name: Evaluate RIPR gate decision"));
        assert!(workflow.contains("name: Render RIPR baseline debt delta"));
        assert!(workflow.contains("name: Emit RIPR PR guidance annotations"));
        assert!(workflow.contains("name: Render RIPR test-oracle assistant proof"));
        assert!(workflow.contains("name: Render RIPR assistant loop health"));
        assert!(workflow.contains("name: Render RIPR first useful action"));
        assert!(workflow.contains("name: Render RIPR PR review front panel"));
        assert!(workflow.contains("name: Render RIPR report packet index"));
        assert!(workflow.contains("escape_github_property()"));
        assert!(workflow.contains("annotation_path=\"$(escape_github_property \"$path\")\""));
        assert!(workflow.contains("::warning file=$annotation_path,line=$annotation_line"));
        assert!(workflow.contains("title=$annotation_title"));
        assert!(workflow.contains("name: Add RIPR advisory summary"));
        assert!(workflow.contains("## RIPR advisory summary"));
        assert!(workflow.contains("### PR review summary"));
        assert!(workflow.contains("#### PR review at a glance"));
        assert!(workflow.contains("### Recommended next test"));
        assert!(workflow.contains("#### Recommended next test at a glance"));
        assert!(workflow.contains("### Top recommendation"));
        assert!(workflow.contains("### Artifact packet"));
        assert!(workflow.contains("### Uploaded review artifacts"));
        assert!(workflow.contains("#### Uploaded artifacts at a glance"));
        assert!(workflow.contains("### Gate decision"));
        assert!(workflow.contains("#### Gate decision at a glance"));
        assert!(workflow.contains("### Baseline debt delta"));
        assert!(workflow.contains("#### Baseline debt movement"));
        assert!(workflow.contains("### RIPR Zero status"));
        assert!(workflow.contains("#### RIPR Zero at a glance"));
        assert!(workflow.contains("### PR evidence ledger"));
        assert!(workflow.contains("#### PR movement at a glance"));
        assert!(workflow.contains("### Test-oracle assistant proof"));
        assert!(workflow.contains("#### Assistant proof at a glance"));
        assert!(workflow.contains("### Agent proof status"));
        assert!(workflow.contains("#### Agent proof status at a glance"));
        assert!(workflow.contains("markdown_inline()"));
        assert!(workflow.contains("Active PR labels"));
        assert!(workflow.contains("Applied waiver label"));
        assert!(workflow.contains("Baseline artifact"));
        assert!(workflow.contains("Recommendation calibration"));
        assert!(workflow.contains("Mutation calibration"));
        assert!(workflow.contains("Blocking reason"));
        assert!(workflow.contains("Gate artifacts"));
        assert!(workflow.contains("Baseline delta artifacts"));
        assert!(workflow.contains("Proof artifacts"));
        assert!(workflow.contains("Action artifacts"));
        assert!(workflow.contains("Front-panel artifacts"));
        assert!(workflow.contains("Index artifacts"));
        assert!(workflow.contains("### SARIF and badge status"));
        assert!(workflow.contains("### PR guidance annotations"));
        assert!(workflow.contains("### PR inline comments"));
        assert!(workflow.contains("### Known limits"));
        assert!(workflow.contains("Inline comments are disabled by default"));
        assert!(!workflow.contains("fail-on-new-warning"));
        assert!(!workflow.contains("pull_request_target"));
        assert!(!workflow.contains("RIPR_GATE_MODE: \"acknowledgeable\""));
        assert!(!workflow.contains("RIPR_GATE_MODE: \"baseline-check\""));
        assert!(!workflow.contains("RIPR_GATE_MODE: \"calibrated-gate\""));
    }

    #[test]
    fn init_generated_github_workflow_uploads_reports_and_makes_sarif_optional() {
        let workflow = generated_github_actions_workflow();
        assert!(workflow.contains("name: RIPR advisory reports"));
        assert!(workflow.contains("target/ripr/pilot"));
        assert!(workflow.contains("target/ripr/agent"));
        assert!(workflow.contains("target/ripr/workflow"));
        assert!(workflow.contains("target/ripr/reports"));
        assert!(workflow.contains("target/ripr/review"));
        assert!(workflow.contains("target/ci"));
        assert!(workflow.contains("name: ripr-reports"));
        assert!(workflow.contains("RIPR_TOP_SEAM_ID"));
        assert!(workflow.contains(".top_actionable_seams[0].seam_id"));
        assert!(!workflow.contains(".top_seams[0].seam_id"));
        assert!(workflow.contains("cargo xtask operator-cockpit"));
        assert!(workflow.contains("cat target/ripr/pilot/pilot-summary.md"));
        assert!(workflow.contains("cat target/ripr/workflow/agent-review-summary.md"));
        assert!(workflow.contains("repo-ripr-badge.json"));
        assert!(workflow.contains("repo-ripr-badge-shields.json"));
        assert!(workflow.contains(".summary.comments // 0"));
        assert!(workflow.contains(".summary.summary_only // 0"));
        assert!(workflow.contains(".summary.suppressed // 0"));
        assert!(workflow.contains(".summary.unknown_confidence // 0"));
        assert!(workflow.contains(".inputs.labels // []"));
        assert!(workflow.contains(".policy.acknowledgement_labels // []"));
        assert!(workflow.contains(".policy.acknowledgement_label"));
        assert!(workflow.contains(".inputs.baseline // \"not supplied\""));
        assert!(workflow.contains(".inputs.recommendation_calibration // \"not supplied\""));
        assert!(workflow.contains(".inputs.mutation_calibration // \"not supplied\""));
        assert!(workflow.contains(".evidence.recommendation_calibration.confidence_effect"));
        assert!(workflow.contains(".evidence.mutation_calibration.confidence_effect"));
        assert!(workflow.contains(".gate_reason"));
        assert!(workflow.contains("blocking=\"$(markdown_inline \"$blocking\")\""));
        assert!(workflow.contains("Counts: blocking=\\`$blocking\\`"));
        assert!(workflow.contains(".delta.still_present // 0"));
        assert!(workflow.contains(".delta.resolved // 0"));
        assert!(workflow.contains(".delta.new_policy_eligible // 0"));
        assert!(workflow.contains(".delta.acknowledged // 0"));
        assert!(workflow.contains(".delta.suppressed // 0"));
        assert!(workflow.contains(".delta.stale_baseline_entry // 0"));
        assert!(workflow.contains(".delta.invalid_baseline_entry // 0"));
        assert!(workflow.contains(".delta.missing_current_input // 0"));
        assert!(workflow.contains("Counts: still_present=\\`$still_present\\`"));
        assert!(workflow.contains(".movement.new_policy_eligible // 0"));
        assert!(workflow.contains(".movement.baseline_still_present // 0"));
        assert!(workflow.contains(".movement.baseline_resolved // 0"));
        assert!(workflow.contains(".movement.acknowledged // 0"));
        assert!(workflow.contains(".movement.suppressed // 0"));
        assert!(workflow.contains(".movement.blocking_candidates // 0"));
        assert!(workflow.contains(".movement.visible_unresolved // 0"));
        assert!(workflow.contains(".coverage_grip_frontier.status // \"not_available\""));
        assert!(workflow.contains(".history.trend // \"not_available\""));
        assert!(workflow.contains("Counts: new_policy_eligible=\\`$ledger_new_policy_eligible\\`"));
        assert!(workflow.contains("sed 's/`/\\\\`/g'"));
        assert!(workflow.contains("Blocking reason: \\`$blocking_reason\\`"));
        assert!(workflow.contains("Boundary: $limits_note"));
        assert!(workflow.contains("Pass/fail authority remains \\`ripr gate evaluate\\`"));
        assert!(workflow.contains("cat target/ripr/reports/pr-evidence-ledger.md"));
        assert!(workflow.contains("Set `RIPR_GATE_BASELINE`"));
        assert!(workflow.contains("RIPR_GATE_MODE"));
        assert!(workflow.contains("RIPR_GATE_BASELINE"));
        assert!(workflow.contains("RIPR_COMMENT_MODE"));
        assert!(workflow.contains("existing-comments.raw.json"));
        assert!(workflow.contains("<!-- ripr:dedupe="));
        assert!(workflow.contains("--mode \"$RIPR_COMMENT_MODE\""));
        assert!(workflow.contains("--existing-comments target/ripr/review/existing-comments.json"));
        assert!(workflow.contains("--token-available"));
        assert!(workflow.contains("--write-permission"));
        assert!(workflow.contains("jq -e '.summary.safe_to_publish == true'"));
        assert!(workflow.contains("gh api --method POST"));
        assert!(workflow.contains("gh api --method PATCH"));
        assert!(workflow.contains("assistant-loop proof"));
        assert!(workflow.contains("first-action"));
        assert!(workflow.contains("--pr-guidance target/ripr/review/comments.json"));
        assert!(workflow.contains("--agent-packet target/ripr/workflow/agent-brief.json"));
        assert!(workflow.contains("--before target/ripr/workflow/before.repo-exposure.json"));
        assert!(workflow.contains("--after target/ripr/workflow/after.repo-exposure.json"));
        assert!(workflow.contains("--receipt target/ripr/reports/agent-receipt.json"));
        assert!(workflow.contains("--ledger target/ripr/reports/pr-evidence-ledger.json"));
        assert!(
            workflow
                .contains("--coverage-frontier target/ripr/reports/coverage-grip-frontier.json")
        );
        assert!(workflow.contains("--gate-decision target/ripr/reports/gate-decision.json"));
        assert!(workflow.contains("pr-review front-panel"));
        assert!(workflow.contains("reports index"));
        assert!(workflow.contains("front_panel_has_input=true"));
        assert!(workflow.contains("--first-action target/ripr/reports/first-useful-action.json"));
        assert!(
            workflow.contains("--assistant-health target/ripr/reports/assistant-loop-health.json")
        );
        assert!(workflow.contains("--ledger target/ripr/reports/pr-evidence-ledger.json"));
        assert!(workflow.contains("--baseline-delta target/ripr/reports/baseline-debt-delta.json"));
        assert!(workflow.contains("--zero-status target/ripr/reports/ripr-zero-status.json"));
        assert!(
            workflow
                .contains("--mutation-calibration target/ripr/reports/mutation-calibration.json")
        );
        assert!(workflow.contains("--receipt target/ripr/reports/agent-receipt.json"));
        assert!(workflow.contains("ripr \"${gate_args[@]}\""));
        assert!(workflow.contains("ripr \"${proof_args[@]}\""));
        assert!(workflow.contains("ripr \"${first_action_args[@]}\""));
        assert!(workflow.contains("ripr \"${front_panel_args[@]}\""));
        assert!(workflow.contains("ripr reports index"));
        assert!(workflow.contains("index_has_input=true"));
        assert!(workflow.contains("Set `RIPR_GATE_MODE`"));
        assert!(workflow.contains("No runtime mutation execution is performed"));
        assert!(workflow.contains("hashFiles('crates/ripr/Cargo.toml')"));
        assert!(workflow.contains("hashFiles('xtask/src/reports/operator.rs')"));
        assert!(workflow.contains("if: env.RIPR_UPLOAD_SARIF == 'true'"));
        assert!(workflow.contains(
            "if: env.RIPR_UPLOAD_SARIF == 'true' && github.event_name == 'pull_request'"
        ));
    }

    #[test]
    fn init_generated_github_workflow_matches_smoke_fixture() {
        let workflow = generated_github_actions_workflow();
        let fixture = generated_workflow_smoke_fixture();

        assert!(workflow.contains("RIPR_UPLOAD_SARIF: \"true\""));
        assert!(workflow.contains("RIPR_GATE_MODE: ${{ vars.RIPR_GATE_MODE || '' }}"));
        assert!(workflow.contains("actions/upload-artifact@v7"));
        assert!(workflow.contains("github/codeql-action/upload-sarif@v4"));
        assert_contains_all(&workflow, "command", fixture.commands);
        assert_contains_all(&workflow, "artifact path", fixture.artifact_paths);
        assert_contains_all(&workflow, "summary section", fixture.summary_sections);

        let prepare = workflow_step(&workflow, "Prepare RIPR editor-agent artifacts");
        assert!(prepare.contains("RIPR_TOP_SEAM_ID"));
        assert!(prepare.contains(".top_actionable_seams[0].seam_id"));
        assert!(
            !prepare.contains(".top_seams[0].seam_id"),
            "top seam extraction must use pilot-summary top_actionable_seams"
        );

        let agent_loop = workflow_step(&workflow, "Generate RIPR agent loop artifacts");
        assert!(agent_loop.contains("cp target/ripr/workflow/agent-packet.json"));
        assert!(agent_loop.contains("cp target/ripr/workflow/agent-brief.json"));
        assert!(agent_loop.contains("cp target/ripr/workflow/agent-verify.json"));
        assert!(agent_loop.contains("cp target/ripr/reports/agent-receipt.json"));
        assert!(agent_loop.contains("--format repo-exposure-json"));

        let guidance = workflow_step(&workflow, "Run RIPR PR guidance report");
        assert!(guidance.contains("github.event_name == 'pull_request'"));
        assert!(guidance.contains("mkdir -p target/ripr/review"));
        assert!(guidance.contains("ripr review-comments"));
        assert!(guidance.contains("--base \"origin/${{ github.base_ref }}\""));
        assert!(guidance.contains("--head HEAD"));
        assert!(guidance.contains("--out target/ripr/review/comments.json"));

        let existing_comments = workflow_step(&workflow, "Capture existing RIPR inline comments");
        assert!(existing_comments.contains("env.RIPR_COMMENT_MODE != 'off'"));
        assert!(existing_comments.contains("GH_TOKEN: ${{ github.token }}"));
        assert!(existing_comments.contains("gh api --paginate --slurp"));
        assert!(
            existing_comments.contains("pulls/${{ github.event.pull_request.number }}/comments")
        );
        assert!(existing_comments.contains("target/ripr/review/existing-comments.json"));
        assert!(existing_comments.contains("capture(\"<!-- ripr:dedupe=(?<key>[^ ]+) -->\")"));

        let comment_plan = workflow_step(&workflow, "Plan RIPR inline comments");
        assert!(comment_plan.contains("env.RIPR_COMMENT_MODE != 'off'"));
        assert!(comment_plan.contains("hashFiles('target/ripr/review/comments.json')"));
        assert!(comment_plan.contains("pr-comments plan"));
        assert!(comment_plan.contains("--pr-guidance target/ripr/review/comments.json"));
        assert!(comment_plan.contains("--mode \"$RIPR_COMMENT_MODE\""));
        assert!(comment_plan.contains("--event-name \"${{ github.event_name }}\""));
        assert!(
            comment_plan.contains("--pull-request \"${{ github.event.pull_request.number }}\"")
        );
        assert!(
            comment_plan
                .contains("--head-repo \"${{ github.event.pull_request.head.repo.full_name }}\"")
        );
        assert!(comment_plan.contains("--base-repo \"${{ github.repository }}\""));
        assert!(comment_plan.contains("--out target/ripr/review/comment-publish-plan.json"));
        assert!(comment_plan.contains("--out-md target/ripr/review/comment-publish-plan.md"));
        assert!(
            comment_plan.contains("--existing-comments target/ripr/review/existing-comments.json")
        );
        assert!(comment_plan.contains("--token-available"));
        assert!(comment_plan.contains("--no-token"));
        assert!(comment_plan.contains("--write-permission"));

        let publish_comments = workflow_step(&workflow, "Publish RIPR inline comments");
        assert!(publish_comments.contains("env.RIPR_COMMENT_MODE == 'inline'"));
        assert!(
            publish_comments.contains("hashFiles('target/ripr/review/comment-publish-plan.json')")
        );
        assert!(publish_comments.contains("jq -e '.summary.safe_to_publish == true'"));
        assert!(publish_comments.contains("select(.safe_to_publish == true)"));
        assert!(publish_comments.contains("<!-- ripr:dedupe=%s -->"));
        assert!(publish_comments.contains("github.event.pull_request.head.sha"));
        assert!(publish_comments.contains("gh api --method POST"));
        assert!(publish_comments.contains("gh api --method PATCH"));
        assert_step_before(
            &workflow,
            "Run RIPR PR guidance report",
            "Capture existing RIPR inline comments",
        );
        assert_step_before(
            &workflow,
            "Capture existing RIPR inline comments",
            "Plan RIPR inline comments",
        );
        assert_step_before(
            &workflow,
            "Plan RIPR inline comments",
            "Publish RIPR inline comments",
        );
        assert_step_before(
            &workflow,
            "Plan RIPR inline comments",
            "Evaluate RIPR gate decision",
        );
        assert_step_before(
            &workflow,
            "Capture RIPR gate labels",
            "Evaluate RIPR gate decision",
        );
        assert_step_before(
            &workflow,
            "Evaluate RIPR gate decision",
            "Render RIPR baseline debt delta",
        );
        assert_step_before(
            &workflow,
            "Render RIPR baseline debt delta",
            "Render RIPR Zero status",
        );
        assert_step_before(
            &workflow,
            "Render RIPR Zero status",
            "Render RIPR PR evidence ledger",
        );
        assert_step_before(
            &workflow,
            "Render RIPR PR evidence ledger",
            "Render RIPR test-oracle assistant proof",
        );
        assert_step_before(
            &workflow,
            "Render RIPR test-oracle assistant proof",
            "Render RIPR assistant loop health",
        );
        assert_step_before(
            &workflow,
            "Render RIPR assistant loop health",
            "Render RIPR first useful action",
        );
        assert_step_before(
            &workflow,
            "Render RIPR first useful action",
            "Render RIPR PR review front panel",
        );
        assert_step_before(
            &workflow,
            "Render RIPR PR review front panel",
            "Render RIPR report packet index",
        );
        assert_step_before(
            &workflow,
            "Render RIPR report packet index",
            "Render RIPR LLM work-loop summaries",
        );
        assert_step_before(
            &workflow,
            "Render RIPR PR evidence ledger",
            "Emit RIPR PR guidance annotations",
        );
        assert_step_before(
            &workflow,
            "Run RIPR PR guidance report",
            "Add RIPR advisory summary",
        );

        let artifact_upload = workflow_step(&workflow, "Upload RIPR report artifacts");
        assert!(artifact_upload.contains("if-no-files-found: ignore"));
        for path in [
            "target/ripr/pilot",
            "target/ripr/agent",
            "target/ripr/workflow",
            "target/ripr/reports",
            "target/ripr/review",
            "target/ci",
        ] {
            assert!(
                artifact_upload.contains(path),
                "artifact upload must include {path}"
            );
        }

        let gate = workflow_step(&workflow, "Evaluate RIPR gate decision");
        assert!(gate.contains("env.RIPR_GATE_MODE != ''"));
        assert!(gate.contains("hashFiles('target/ripr/review/comments.json')"));
        assert!(gate.contains("gate evaluate"));
        assert!(gate.contains("--pr-guidance target/ripr/review/comments.json"));
        assert!(gate.contains("--mode \"$RIPR_GATE_MODE\""));
        assert!(gate.contains("--out target/ripr/reports/gate-decision.json"));
        assert!(gate.contains("--out-md target/ripr/reports/gate-decision.md"));
        assert!(gate.contains("--labels-json target/ci/labels.json"));
        assert!(gate.contains("--sarif-policy target/ripr/reports/sarif-policy.json"));
        assert!(gate.contains(
            "--recommendation-calibration target/ripr/reports/recommendation-calibration.json"
        ));
        assert!(
            gate.contains("--mutation-calibration target/ripr/reports/mutation-calibration.json")
        );
        assert!(gate.contains("--baseline \"$RIPR_GATE_BASELINE\""));
        assert!(!gate.contains("continue-on-error: true"));

        let baseline_delta = workflow_step(&workflow, "Render RIPR baseline debt delta");
        assert!(baseline_delta.contains("always() && env.RIPR_GATE_BASELINE != ''"));
        assert!(baseline_delta.contains("hashFiles('target/ripr/reports/gate-decision.json')"));
        assert!(baseline_delta.contains("continue-on-error: true"));
        assert!(baseline_delta.contains("ripr baseline diff"));
        assert!(baseline_delta.contains("--baseline \"$RIPR_GATE_BASELINE\""));
        assert!(baseline_delta.contains("--current target/ripr/reports/gate-decision.json"));
        assert!(baseline_delta.contains("--out target/ripr/reports/baseline-debt-delta.json"));
        assert!(baseline_delta.contains("--out-md target/ripr/reports/baseline-debt-delta.md"));

        let zero_status = workflow_step(&workflow, "Render RIPR Zero status");
        assert!(zero_status.contains("hashFiles('target/ripr/reports/baseline-debt-delta.json')"));
        assert!(zero_status.contains("continue-on-error: true"));
        assert!(zero_status.contains("zero status"));
        assert!(zero_status.contains("--delta target/ripr/reports/baseline-debt-delta.json"));
        assert!(zero_status.contains("--out target/ripr/reports/ripr-zero-status.json"));
        assert!(zero_status.contains("--out-md target/ripr/reports/ripr-zero-status.md"));
        assert!(zero_status.contains("--baseline \"$RIPR_GATE_BASELINE\""));
        assert!(zero_status.contains("--gate target/ripr/reports/gate-decision.json"));
        assert!(zero_status.contains("--pr-guidance target/ripr/review/comments.json"));
        assert!(zero_status.contains(
            "--recommendation-calibration target/ripr/reports/recommendation-calibration.json"
        ));

        let pr_ledger = workflow_step(&workflow, "Render RIPR PR evidence ledger");
        assert!(pr_ledger.contains("github.event_name == 'pull_request'"));
        assert!(pr_ledger.contains("hashFiles('target/ripr/review/comments.json')"));
        assert!(pr_ledger.contains("continue-on-error: true"));
        assert!(pr_ledger.contains("pr-ledger record"));
        assert!(pr_ledger.contains("--pr-number \"${{ github.event.pull_request.number }}\""));
        assert!(pr_ledger.contains("--base \"origin/${{ github.base_ref }}\""));
        assert!(pr_ledger.contains("--head HEAD"));
        assert!(pr_ledger.contains("--pr-guidance target/ripr/review/comments.json"));
        assert!(pr_ledger.contains("--gate target/ripr/reports/gate-decision.json"));
        assert!(
            pr_ledger.contains("--baseline-delta target/ripr/reports/baseline-debt-delta.json")
        );
        assert!(pr_ledger.contains("--zero-status target/ripr/reports/ripr-zero-status.json"));
        assert!(pr_ledger.contains(
            "--recommendation-calibration target/ripr/reports/recommendation-calibration.json"
        ));
        assert!(pr_ledger.contains("--agent-receipt target/ripr/reports/agent-receipt.json"));
        assert!(pr_ledger.contains("--coverage target/ripr/reports/coverage-summary.json"));
        assert!(pr_ledger.contains("--history .ripr/pr-evidence-ledger.jsonl"));
        assert!(pr_ledger.contains("ledger_args+=(--label \"$label\")"));
        assert!(pr_ledger.contains("ripr \"${ledger_args[@]}\""));

        let assistant_proof = workflow_step(&workflow, "Render RIPR test-oracle assistant proof");
        assert!(assistant_proof.contains("hashFiles('target/ripr/review/comments.json')"));
        assert!(assistant_proof.contains("hashFiles('target/ripr/workflow/agent-brief.json')"));
        assert!(
            assistant_proof.contains("hashFiles('target/ripr/workflow/before.repo-exposure.json')")
        );
        assert!(
            assistant_proof.contains("hashFiles('target/ripr/workflow/after.repo-exposure.json')")
        );
        assert!(assistant_proof.contains("hashFiles('target/ripr/reports/agent-receipt.json')"));
        assert!(
            assistant_proof.contains("hashFiles('target/ripr/reports/pr-evidence-ledger.json')")
        );
        assert!(assistant_proof.contains("continue-on-error: true"));
        assert!(assistant_proof.contains("assistant-loop proof"));
        assert!(assistant_proof.contains("--root ."));
        assert!(assistant_proof.contains("--pr-guidance target/ripr/review/comments.json"));
        assert!(assistant_proof.contains("--agent-packet target/ripr/workflow/agent-brief.json"));
        assert!(
            assistant_proof.contains("--before target/ripr/workflow/before.repo-exposure.json")
        );
        assert!(assistant_proof.contains("--after target/ripr/workflow/after.repo-exposure.json"));
        assert!(assistant_proof.contains("--receipt target/ripr/reports/agent-receipt.json"));
        assert!(assistant_proof.contains("--ledger target/ripr/reports/pr-evidence-ledger.json"));
        assert!(
            assistant_proof.contains("--out target/ripr/reports/test-oracle-assistant-proof.json")
        );
        assert!(
            assistant_proof.contains("--out-md target/ripr/reports/test-oracle-assistant-proof.md")
        );
        assert!(
            assistant_proof
                .contains("--coverage-frontier target/ripr/reports/coverage-grip-frontier.json")
        );
        assert!(assistant_proof.contains("--gate-decision target/ripr/reports/gate-decision.json"));
        assert!(assistant_proof.contains("ripr \"${proof_args[@]}\""));

        let assistant_health = workflow_step(&workflow, "Render RIPR assistant loop health");
        assert!(
            assistant_health
                .contains("hashFiles('target/ripr/reports/test-oracle-assistant-proof.json')")
        );
        assert!(assistant_health.contains("continue-on-error: true"));
        assert!(assistant_health.contains("assistant-loop health"));
        assert!(assistant_health.contains("--root ."));
        assert!(
            assistant_health
                .contains("--proof target/ripr/reports/test-oracle-assistant-proof.json")
        );
        assert!(assistant_health.contains("--out target/ripr/reports/assistant-loop-health.json"));
        assert!(assistant_health.contains("--out-md target/ripr/reports/assistant-loop-health.md"));

        let first_action = workflow_step(&workflow, "Render RIPR first useful action");
        assert!(first_action.contains("continue-on-error: true"));
        assert!(first_action.contains("first-action"));
        assert!(first_action.contains("--root ."));
        assert!(first_action.contains("--pr-guidance target/ripr/review/comments.json"));
        assert!(
            first_action
                .contains("--assistant-proof target/ripr/reports/test-oracle-assistant-proof.json")
        );
        assert!(first_action.contains("--ledger target/ripr/reports/pr-evidence-ledger.json"));
        assert!(
            first_action.contains("--baseline-delta target/ripr/reports/baseline-debt-delta.json")
        );
        assert!(first_action.contains("--receipt target/ripr/reports/agent-receipt.json"));
        assert!(first_action.contains("--gate-decision target/ripr/reports/gate-decision.json"));
        assert!(
            first_action
                .contains("--coverage-frontier target/ripr/reports/coverage-grip-frontier.json")
        );
        assert!(
            first_action.contains("--editor-context target/ripr/workflow/evidence-context.json")
        );
        assert!(first_action.contains("--out target/ripr/reports/first-useful-action.json"));
        assert!(first_action.contains("--out-md target/ripr/reports/first-useful-action.md"));
        assert!(first_action.contains("first_action_has_input=true"));
        assert!(first_action.contains("ripr \"${first_action_args[@]}\""));

        let front_panel = workflow_step(&workflow, "Render RIPR PR review front panel");
        assert!(front_panel.contains("continue-on-error: true"));
        assert!(front_panel.contains("pr-review front-panel"));
        assert!(front_panel.contains("--root ."));
        assert!(front_panel.contains("--pr-guidance target/ripr/review/comments.json"));
        assert!(
            front_panel.contains("--first-action target/ripr/reports/first-useful-action.json")
        );
        assert!(
            front_panel
                .contains("--assistant-proof target/ripr/reports/test-oracle-assistant-proof.json")
        );
        assert!(
            front_panel
                .contains("--assistant-health target/ripr/reports/assistant-loop-health.json")
        );
        assert!(front_panel.contains("--ledger target/ripr/reports/pr-evidence-ledger.json"));
        assert!(
            front_panel.contains("--baseline-delta target/ripr/reports/baseline-debt-delta.json")
        );
        assert!(front_panel.contains("--zero-status target/ripr/reports/ripr-zero-status.json"));
        assert!(front_panel.contains("--gate-decision target/ripr/reports/gate-decision.json"));
        assert!(front_panel.contains(
            "--recommendation-calibration target/ripr/reports/recommendation-calibration.json"
        ));
        assert!(
            front_panel
                .contains("--mutation-calibration target/ripr/reports/mutation-calibration.json")
        );
        assert!(
            front_panel
                .contains("--coverage-frontier target/ripr/reports/coverage-grip-frontier.json")
        );
        assert!(front_panel.contains("--receipt target/ripr/reports/agent-receipt.json"));
        assert!(front_panel.contains("--out target/ripr/reports/pr-review-front-panel.json"));
        assert!(front_panel.contains("--out-md target/ripr/reports/pr-review-front-panel.md"));
        assert!(front_panel.contains("front_panel_has_input=true"));
        assert!(front_panel.contains("ripr \"${front_panel_args[@]}\""));
        assert!(front_panel.contains("No RIPR PR review front-panel inputs were available."));

        let packet_index = workflow_step(&workflow, "Render RIPR report packet index");
        assert!(packet_index.contains("continue-on-error: true"));
        assert!(packet_index.contains("reports index"));
        assert!(packet_index.contains("--reports-dir target/ripr/reports"));
        assert!(packet_index.contains("--review-dir target/ripr/review"));
        assert!(packet_index.contains("--receipts-dir target/ripr/receipts"));
        assert!(packet_index.contains("--workflow-dir target/ripr/workflow"));
        assert!(packet_index.contains("--agent-dir target/ripr/agent"));
        assert!(packet_index.contains("--pilot-dir target/ripr/pilot"));
        assert!(packet_index.contains("--ci-dir target/ci"));
        assert!(packet_index.contains("--out target/ripr/reports/index.json"));
        assert!(packet_index.contains("--out-md target/ripr/reports/index.md"));
        assert!(packet_index.contains("target/ripr/reports/pr-review-front-panel.md"));
        assert!(packet_index.contains("target/ripr/review/comments.json"));
        assert!(packet_index.contains("target/ripr/reports/gate-decision.md"));
        assert!(packet_index.contains("target/ripr/reports/agent-receipt.json"));
        assert!(packet_index.contains("index_has_input=true"));
        assert!(packet_index.contains("No RIPR report-packet index inputs were available."));

        let annotations = workflow_step(&workflow, "Emit RIPR PR guidance annotations");
        assert!(annotations.contains("hashFiles('target/ripr/review/comments.json')"));
        assert!(annotations.contains("escape_github_message()"));
        assert!(annotations.contains("escape_github_property()"));
        assert!(annotations.contains("::warning file=$annotation_path,line=$annotation_line"));

        let summary = workflow_step(&workflow, "Add RIPR advisory summary");
        assert!(summary.contains("### PR review summary"));
        assert!(summary.contains("#### PR review at a glance"));
        assert!(summary.contains("target/ripr/reports/pr-review-front-panel.json"));
        assert!(summary.contains("target/ripr/reports/pr-review-front-panel.md"));
        assert!(summary.contains(".summary.headline // \"not_available\""));
        assert!(summary.contains(".summary.top_issue_state // \"unknown\""));
        assert!(summary.contains(".summary.policy_state // \"none\""));
        assert!(summary.contains(".summary.placement // \"not_available\""));
        assert!(summary.contains(".summary.movement_state // \"unknown\""));
        assert!(summary.contains(".summary.coverage_grip_state // \"not_available\""));
        assert!(summary.contains(".summary.new_policy_eligible // 0"));
        assert!(summary.contains(".summary.baseline_still_present // 0"));
        assert!(summary.contains(".summary.baseline_resolved // 0"));
        assert!(summary.contains(".summary.blocking_candidates // 0"));
        assert!(summary.contains(".top_issue.missing_discriminator // \"not_available\""));
        assert!(summary.contains(".top_issue.suggested_test // \"not_available\""));
        assert!(summary.contains(".top_issue.verify_command // \"not_available\""));
        assert!(summary.contains(".top_issue.agent_command // \"not_available\""));
        assert!(summary.contains(".top_issue.receipt.artifact // \"not_available\""));
        assert!(summary.contains(".policy.mode // \"not_available\""));
        assert!(summary.contains(".policy.decision // \"not_available\""));
        assert!(summary.contains("cat target/ripr/reports/pr-review-front-panel.md"));
        assert!(summary.contains("PR review summary was not generated"));
        assert!(summary.contains("### Recommended next test"));
        assert!(summary.contains("#### Recommended next test at a glance"));
        assert!(summary.contains("target/ripr/reports/first-useful-action.json"));
        assert!(summary.contains("target/ripr/reports/first-useful-action.md"));
        assert!(summary.contains(".action_kind // \"unknown\""));
        assert!(summary.contains(".commands.verify // \"not_available\""));
        assert!(summary.contains(".commands.receipt // \"not_available\""));
        assert!(summary.contains(".fallback.kind // \"none\""));
        assert!(summary.contains("cat target/ripr/reports/first-useful-action.md"));
        assert!(summary.contains("Recommended next test was not generated"));
        assert!(summary.contains("cat target/ripr/pilot/pilot-summary.md"));
        assert!(summary.contains("cat target/ripr/workflow/agent-review-summary.md"));
        assert!(summary.contains("### Uploaded review artifacts"));
        assert!(summary.contains("#### Uploaded artifacts at a glance"));
        assert!(summary.contains("target/ripr/reports/index.json"));
        assert!(summary.contains("target/ripr/reports/index.md"));
        assert!(summary.contains(".summary.entries // 0"));
        assert!(summary.contains(".summary.available // 0"));
        assert!(summary.contains(".summary.missing_expected // 0"));
        assert!(summary.contains(".summary.start_here // \"not_available\""));
        assert!(summary.contains(".summary.gate_authority // \"not_available\""));
        assert!(summary.contains(".missing_expected[]?.label"));
        assert!(summary.contains(".warnings[]?.kind"));
        assert!(summary.contains("cat target/ripr/reports/index.md"));
        assert!(summary.contains("Uploaded review artifacts summary was not generated"));
        assert!(summary.contains("#### Gate decision at a glance"));
        assert!(summary.contains("markdown_inline()"));
        assert!(summary.contains("gate_status=\"$(jq -r '.status // \"unknown\"'"));
        assert!(summary.contains("gate_mode=\"$(jq -r '.mode // \"unknown\"'"));
        assert!(summary.contains(".summary.blocking // 0"));
        assert!(summary.contains(".summary.acknowledged // 0"));
        assert!(summary.contains(".summary.advisory // 0"));
        assert!(summary.contains(".summary.suppressed // 0"));
        assert!(summary.contains(".summary.not_applicable // 0"));
        assert!(summary.contains(".summary.unknown_confidence // 0"));
        assert!(summary.contains("blocking=\"$(markdown_inline \"$blocking\")\""));
        assert!(summary.contains("Counts: blocking=\\`$blocking\\`"));
        assert!(summary.contains("Active PR labels"));
        assert!(summary.contains("Acknowledgement labels"));
        assert!(summary.contains("Applied waiver label"));
        assert!(summary.contains("Baseline artifact"));
        assert!(summary.contains("Recommendation calibration"));
        assert!(summary.contains("Mutation calibration"));
        assert!(summary.contains("Blocking reason: \\`$blocking_reason\\`"));
        assert!(summary.contains("target/ripr/reports/gate-decision.json"));
        assert!(summary.contains("target/ci/labels.json"));
        assert!(summary.contains("cat target/ripr/reports/gate-decision.md"));
        assert!(summary.contains("Gate decision was not run"));
        assert!(summary.contains("### Baseline debt delta"));
        assert!(summary.contains("#### Baseline debt movement"));
        assert!(summary.contains("target/ripr/reports/baseline-debt-delta.json"));
        assert!(summary.contains("target/ripr/reports/baseline-debt-delta.md"));
        assert!(summary.contains("cat target/ripr/reports/baseline-debt-delta.md"));
        assert!(summary.contains(".baseline.path // .inputs.baseline // \"unknown\""));
        assert!(summary.contains(".delta.still_present // 0"));
        assert!(summary.contains(".delta.resolved // 0"));
        assert!(summary.contains(".delta.new_policy_eligible // 0"));
        assert!(summary.contains(".delta.acknowledged // 0"));
        assert!(summary.contains(".delta.suppressed // 0"));
        assert!(summary.contains(".delta.stale_baseline_entry // 0"));
        assert!(summary.contains(".delta.invalid_baseline_entry // 0"));
        assert!(summary.contains(".delta.missing_current_input // 0"));
        assert!(summary.contains("Set `RIPR_GATE_BASELINE`"));
        assert!(summary.contains("Baseline debt delta was not run"));
        assert!(summary.contains("Baseline debt delta was not generated"));
        assert!(summary.contains("### RIPR Zero status"));
        assert!(summary.contains("#### RIPR Zero at a glance"));
        assert!(summary.contains("target/ripr/reports/ripr-zero-status.json"));
        assert!(summary.contains("target/ripr/reports/ripr-zero-status.md"));
        assert!(summary.contains(".ripr_zero.state // \"unknown\""));
        assert!(summary.contains(".ripr_zero.visible_unresolved // 0"));
        assert!(summary.contains(".ripr_zero.new_policy_eligible // 0"));
        assert!(summary.contains(".ripr_zero.blocking_candidates // 0"));
        assert!(summary.contains(".baseline.metadata.stale // 0"));
        assert!(summary.contains(".top_debt_areas[0].area // \"none\""));
        assert!(summary.contains("cat target/ripr/reports/ripr-zero-status.md"));
        assert!(summary.contains("RIPR Zero status was not run"));
        assert!(summary.contains("RIPR Zero status was not generated"));
        assert!(summary.contains("### PR evidence ledger"));
        assert!(summary.contains("#### PR movement at a glance"));
        assert!(summary.contains("target/ripr/reports/pr-evidence-ledger.json"));
        assert!(summary.contains("target/ripr/reports/pr-evidence-ledger.md"));
        assert!(summary.contains(".movement.new_policy_eligible // 0"));
        assert!(summary.contains(".movement.baseline_still_present // 0"));
        assert!(summary.contains(".movement.baseline_resolved // 0"));
        assert!(summary.contains(".movement.acknowledged // 0"));
        assert!(summary.contains(".movement.suppressed // 0"));
        assert!(summary.contains(".movement.blocking_candidates // 0"));
        assert!(summary.contains(".movement.visible_unresolved // 0"));
        assert!(summary.contains(".coverage_grip_frontier.status // \"not_available\""));
        assert!(summary.contains(".history.trend // \"not_available\""));
        assert!(summary.contains(".top_repair_route.verify_command // \"not_available\""));
        assert!(summary.contains(".top_repair_route.agent_command // \"not_available\""));
        assert!(summary.contains("Pass/fail authority remains \\`ripr gate evaluate\\`"));
        assert!(summary.contains("cat target/ripr/reports/pr-evidence-ledger.md"));
        assert!(summary.contains("PR evidence ledger was not generated"));
        assert!(summary.contains("PR evidence ledger was not run"));
        assert!(summary.contains("### Test-oracle assistant proof"));
        assert!(summary.contains("#### Assistant proof at a glance"));
        assert!(summary.contains("target/ripr/reports/test-oracle-assistant-proof.json"));
        assert!(summary.contains("target/ripr/reports/test-oracle-assistant-proof.md"));
        assert!(summary.contains(".seam.missing_discriminator // \"not_available\""));
        assert!(summary.contains(".recommendation.placement // \"not_available\""));
        assert!(summary.contains(".evidence_movement.state // \"unknown\""));
        assert!(summary.contains(".ci_projection.gate_decision // \"not_supplied\""));
        assert!(summary.contains(".ci_projection.coverage_frontier // \"not_supplied\""));
        assert!(summary.contains("cat target/ripr/reports/test-oracle-assistant-proof.md"));
        assert!(summary.contains("### Agent proof status"));
        assert!(summary.contains("#### Agent proof status at a glance"));
        assert!(summary.contains("target/ripr/reports/assistant-loop-health.json"));
        assert!(summary.contains("target/ripr/reports/assistant-loop-health.md"));
        assert!(summary.contains(".summary.proofs // 0"));
        assert!(summary.contains(".summary.complete // 0"));
        assert!(summary.contains(".summary.partial // 0"));
        assert!(summary.contains(".summary.missing_required_input // 0"));
        assert!(summary.contains(".summary.missing_optional_input // 0"));
        assert!(summary.contains(".summary.improved // 0"));
        assert!(summary.contains(".summary.unchanged // 0"));
        assert!(summary.contains(".summary.regressed // 0"));
        assert!(summary.contains(".summary.unknown_movement // 0"));
        assert!(summary.contains(".summary.repair_queue // 0"));
        assert!(summary.contains(".warning_summary[]?"));
        assert!(summary.contains(".repair_queue[]?.repair_kind"));
        assert!(summary.contains("cat target/ripr/reports/assistant-loop-health.md"));
        assert!(summary.contains("advisory static health over proof artifacts"));
        assert!(summary.contains(".summary.comments // 0"));
        assert!(summary.contains(".summary.summary_only // 0"));
        assert!(summary.contains(".summary.suppressed // 0"));
        assert!(summary.contains("No runtime mutation execution is performed"));

        for step in fixture.non_blocking_steps {
            let block = workflow_step(&workflow, step);
            assert!(
                block.contains("continue-on-error: true"),
                "`{step}` must remain advisory/non-blocking"
            );
        }

        for step in fixture.optional_sarif_steps {
            let block = workflow_step(&workflow, step);
            assert!(
                block.contains("env.RIPR_UPLOAD_SARIF == 'true'"),
                "`{step}` must stay gated by RIPR_UPLOAD_SARIF"
            );
        }

        for forbidden in fixture.forbidden_fragments {
            assert!(
                !workflow.contains(forbidden),
                "generated workflow must not enable `{forbidden}` by default"
            );
        }
    }

    #[test]
    fn init_ci_github_writes_workflow_and_preserves_existing_config() -> Result<(), String> {
        let dir = unique_command_test_dir("init-ci");
        std::fs::create_dir_all(&dir).map_err(|err| format!("create temp dir: {err}"))?;
        let config = dir.join(CONFIG_FILE_NAME);
        std::fs::write(&config, "# existing policy\n")
            .map_err(|err| format!("write existing config: {err}"))?;

        init(&args(&[
            "--root",
            &dir.display().to_string(),
            "--ci",
            "github",
        ]))?;

        let config_text =
            std::fs::read_to_string(&config).map_err(|err| format!("read config: {err}"))?;
        let workflow_path = dir.join(".github/workflows/ripr.yml");
        let workflow = std::fs::read_to_string(&workflow_path)
            .map_err(|err| format!("read workflow: {err}"))?;
        assert_eq!(config_text, "# existing policy\n");
        assert!(workflow.contains("RIPR advisory reports"));
        assert!(workflow.contains("continue-on-error: true"));
        assert!(workflow.contains("actions/upload-artifact@v7"));
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn init_ci_github_refuses_existing_workflow_without_force() -> Result<(), String> {
        let dir = unique_command_test_dir("init-ci-existing");
        let workflow_dir = dir.join(".github/workflows");
        std::fs::create_dir_all(&workflow_dir)
            .map_err(|err| format!("create workflow dir: {err}"))?;
        let workflow = workflow_dir.join("ripr.yml");
        std::fs::write(&workflow, "name: Existing\n")
            .map_err(|err| format!("write existing workflow: {err}"))?;

        let result = init(&args(&[
            "--root",
            &dir.display().to_string(),
            "--ci",
            "github",
        ]));
        assert!(matches!(result, Err(message) if message.contains("already exists")));
        assert!(!dir.join(CONFIG_FILE_NAME).exists());
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn doctor_rejects_unknown_arguments() {
        assert_eq!(
            doctor(&args(&["--verbose"])),
            Err("unknown doctor argument \"--verbose\"".to_string())
        );
    }

    #[test]
    fn doctor_accepts_default_root() {
        assert_eq!(doctor(&args(&[])), Ok(()));
    }

    #[test]
    fn lsp_version_returns_ok() {
        assert_eq!(lsp(&args(&["--version"])), Ok(()));
    }

    #[test]
    fn lsp_rejects_unknown_arguments() {
        assert_eq!(
            lsp(&args(&["--bad"])),
            Err("unknown lsp argument \"--bad\"".to_string())
        );
    }

    #[test]
    fn check_rejects_unknown_argument() {
        assert_eq!(
            check(&args(&["--wat"])),
            Err("unknown check argument \"--wat\"".to_string())
        );
    }

    #[test]
    fn check_requires_values_for_all_value_flags() {
        assert_eq!(
            check(&args(&["--root"])),
            Err("missing value for --root".to_string())
        );
        assert_eq!(
            check(&args(&["--base"])),
            Err("missing value for --base".to_string())
        );
        assert_eq!(
            check(&args(&["--format"])),
            Err("missing value for --format".to_string())
        );
    }

    #[test]
    fn explain_requires_selector() {
        assert_eq!(
            explain(&args(&[])),
            Err("missing finding selector".to_string())
        );
    }

    #[test]
    fn explain_rejects_unexpected_argument_after_selector() {
        assert_eq!(
            explain(&args(&["probe:src_lib_rs:10:return_value", "extra"])),
            Err("unexpected explain argument \"extra\"".to_string())
        );
    }

    #[test]
    fn explain_requires_values_for_value_flags() {
        assert_eq!(
            explain(&args(&["--root"])),
            Err("missing value for --root".to_string())
        );
        assert_eq!(
            explain(&args(&["--base"])),
            Err("missing value for --base".to_string())
        );
        assert_eq!(
            explain(&args(&["--diff"])),
            Err("missing value for --diff".to_string())
        );
    }

    #[test]
    fn context_requires_selector() {
        assert_eq!(
            context(&args(&[])),
            Err("missing --at or --finding selector".to_string())
        );
    }

    #[test]
    fn context_rejects_unknown_argument() {
        assert_eq!(
            context(&args(&["--unknown", "value"])),
            Err("unexpected context argument \"--unknown\"".to_string())
        );
    }

    #[test]
    fn context_requires_values_for_value_flags() {
        assert_eq!(
            context(&args(&["--at"])),
            Err("missing value for --at".to_string())
        );
        assert_eq!(
            context(&args(&["--finding"])),
            Err("missing value for --finding".to_string())
        );
        assert_eq!(
            context(&args(&["--root"])),
            Err("missing value for --root".to_string())
        );
    }

    #[test]
    fn lsp_accepts_stdio_flag() {
        // lsp function doesn't reject --stdio, it just processes it
        assert_eq!(lsp(&args(&["--stdio"])), Ok(()));
    }

    #[test]
    fn lsp_version_returns_ok_with_short_flag() {
        assert_eq!(lsp(&args(&["-V"])), Ok(()));
    }

    fn outcome_before_json() -> &'static str {
        r#"{
  "schema_version": "0.2",
  "scope": "repo",
  "seams": [
    {
      "seam_id": "seam-a",
      "kind": "predicate_boundary",
      "file": "src/pricing.rs",
      "line": 42,
      "grip_class": "weakly_gripped",
      "related_tests": [
        {"oracle_kind": "exact_value", "oracle_strength": "weak"}
      ],
      "observed_values": ["50"],
      "missing_discriminators": [
        {"value": "threshold equality", "reason": "not observed"}
      ]
    }
  ]
}"#
    }

    fn outcome_after_json() -> &'static str {
        r#"{
  "schema_version": "0.2",
  "scope": "repo",
  "seams": [
    {
      "seam_id": "seam-a",
      "kind": "predicate_boundary",
      "file": "src/pricing.rs",
      "line": 42,
      "grip_class": "strongly_gripped",
      "related_tests": [
        {"oracle_kind": "exact_value", "oracle_strength": "strong"}
      ],
      "observed_values": ["50", "100"],
      "missing_discriminators": []
    }
  ]
}"#
    }

    fn calibration_repo_json() -> &'static str {
        r#"{
  "schema_version": "0.2",
  "scope": "repo",
  "seams": [
    {
      "seam_id": "seam-a",
      "kind": "predicate_boundary",
      "file": "src/pricing.rs",
      "line": 42,
      "grip_class": "weakly_gripped",
      "related_tests": [],
      "observed_values": [],
      "missing_discriminators": []
    }
  ]
}"#
    }

    fn calibration_mutants_json() -> &'static str {
        r#"[{"id":"m1","seam_id":"seam-a","outcome":"missed","operator":"replace"}]"#
    }
}
