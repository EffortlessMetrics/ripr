use crate::analysis;
use crate::app::{self, CheckInput, Mode, OutputFormat};
use crate::cli::help;
use crate::cli::parse::{expect_value, parse_format, parse_mode};
use crate::config::{
    CONFIG_FILE_NAME, CheckInputExplicit, DEFAULT_LSP_SEAM_DIAGNOSTICS, apply_to_check_input,
    generated_init_config, load_for_root,
};
use crate::output;
use std::path::{Path, PathBuf};

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
}

#[derive(Debug, PartialEq, Eq)]
struct OutcomeOptions {
    before: PathBuf,
    after: PathBuf,
    format: OutcomeFormat,
    out: Option<PathBuf>,
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

fn generated_github_actions_workflow() -> &'static str {
    r#"name: RIPR

on:
  pull_request:
  workflow_dispatch:

permissions:
  contents: read
  security-events: write

env:
  RIPR_UPLOAD_SARIF: "true"

jobs:
  ripr:
    name: RIPR advisory reports
    runs-on: ubuntu-latest
    continue-on-error: true
    steps:
      - uses: actions/checkout@v6
        with:
          fetch-depth: 0

      - uses: dtolnay/rust-toolchain@stable

      - name: Install ripr
        run: cargo install ripr --locked

      - name: Generate RIPR pilot packet
        continue-on-error: true
        run: |
          ripr pilot \
            --root . \
            --out target/ripr/pilot \
            --mode ready \
            --max-seams 5

      - name: Capture pull request diff
        if: github.event_name == 'pull_request'
        run: |
          mkdir -p target/ripr/reports
          git diff --binary "origin/${{ github.base_ref }}...HEAD" > target/ripr/reports/pr.diff

      - name: Render RIPR diff SARIF
        if: github.event_name == 'pull_request'
        continue-on-error: true
        run: |
          ripr check \
            --root . \
            --diff target/ripr/reports/pr.diff \
            --format sarif \
            > target/ripr/reports/ripr-findings.sarif

      - name: Render RIPR repo seam SARIF
        continue-on-error: true
        run: |
          mkdir -p target/ripr/reports
          ripr check \
            --root . \
            --mode ready \
            --format repo-sarif \
            > target/ripr/reports/ripr-seams.sarif

      - name: Render RIPR repo badge artifacts
        continue-on-error: true
        run: |
          mkdir -p target/ripr/reports
          ripr check \
            --root . \
            --mode ready \
            --format repo-badge-json \
            > target/ripr/reports/repo-ripr-badge.json
          ripr check \
            --root . \
            --mode ready \
            --format repo-badge-shields \
            > target/ripr/reports/repo-ripr-badge-shields.json

      - name: Add RIPR pilot summary
        if: always() && hashFiles('target/ripr/pilot/pilot-summary.md') != ''
        continue-on-error: true
        run: cat target/ripr/pilot/pilot-summary.md >> "$GITHUB_STEP_SUMMARY"

      - name: Upload RIPR report artifacts
        if: always()
        continue-on-error: true
        uses: actions/upload-artifact@v7
        with:
          name: ripr-reports
          path: |
            target/ripr/pilot
            target/ripr/reports
          if-no-files-found: ignore
          retention-days: 14

      - name: Upload RIPR diff findings
        if: env.RIPR_UPLOAD_SARIF == 'true' && github.event_name == 'pull_request' && hashFiles('target/ripr/reports/ripr-findings.sarif') != ''
        continue-on-error: true
        uses: github/codeql-action/upload-sarif@v4
        with:
          sarif_file: target/ripr/reports/ripr-findings.sarif
          category: ripr-findings

      - name: Upload RIPR repo seams
        if: env.RIPR_UPLOAD_SARIF == 'true' && hashFiles('target/ripr/reports/ripr-seams.sarif') != ''
        continue-on-error: true
        uses: github/codeql-action/upload-sarif@v4
        with:
          sarif_file: target/ripr/reports/ripr-seams.sarif
          category: ripr-seams
"#
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

    let classified = analysis::inventory_classified_seams_at_with_config(&input.root, &config)?;
    let artifacts = pilot_artifacts(&options.out_dir);

    std::fs::create_dir_all(&options.out_dir)
        .map_err(|err| format!("create {} failed: {err}", options.out_dir.display()))?;
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

    let context = output::pilot::PilotSummaryContext {
        root: &input.root,
        mode: &input.mode,
        config_path: config.source_path(),
        max_seams: options.max_seams,
        artifacts: &artifacts,
    };
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

fn parse_outcome_format(value: &str) -> Result<OutcomeFormat, String> {
    match value {
        "md" | "markdown" | "text" => Ok(OutcomeFormat::Markdown),
        "json" => Ok(OutcomeFormat::Json),
        _ => Err(format!("unknown outcome format {value:?}")),
    }
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
        assert_eq!(calibrate(&args(&["--help"])), Ok(()));
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
    fn pilot_parses_root_out_mode_and_max_seams() {
        let options = parse_pilot_options(&args(&[
            "--root",
            "repo",
            "--out",
            "target/pilot",
            "--mode",
            "ready",
            "--max-seams",
            "3",
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
            })
        );
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
        assert!(workflow.contains("continue-on-error: true"));
        assert!(workflow.contains("github/codeql-action/upload-sarif@v4"));
        assert!(workflow.contains("actions/upload-artifact@v7"));
        assert!(workflow.contains("RIPR_UPLOAD_SARIF"));
        assert!(workflow.contains("--format sarif"));
        assert!(workflow.contains("--format repo-sarif"));
        assert!(workflow.contains("--format repo-badge-json"));
        assert!(workflow.contains("ripr pilot"));
        assert!(!workflow.contains("fail-on-new-warning"));
        assert!(!workflow.contains("sarif-policy"));
    }

    #[test]
    fn init_generated_github_workflow_uploads_reports_and_makes_sarif_optional() {
        let workflow = generated_github_actions_workflow();
        assert!(workflow.contains("name: RIPR advisory reports"));
        assert!(workflow.contains("target/ripr/pilot"));
        assert!(workflow.contains("target/ripr/reports"));
        assert!(workflow.contains("name: ripr-reports"));
        assert!(workflow.contains("if: env.RIPR_UPLOAD_SARIF == 'true'"));
        assert!(workflow.contains("cat target/ripr/pilot/pilot-summary.md"));
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
