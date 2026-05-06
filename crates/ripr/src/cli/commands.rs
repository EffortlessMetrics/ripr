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
}

#[derive(Debug, PartialEq, Eq)]
struct PilotOptions {
    root: PathBuf,
    out_dir: PathBuf,
    mode: Mode,
    explicit: CheckInputExplicit,
    max_seams: usize,
}

pub(super) fn init(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        help::print_init_help();
        return Ok(());
    }
    let options = parse_init_options(args)?;
    if options.dry_run {
        print!("{}", generated_init_config());
        return Ok(());
    }
    if !options.root.is_dir() {
        return Err(format!(
            "init root {} is not a directory",
            options.root.display()
        ));
    }
    let config_path = options.root.join(CONFIG_FILE_NAME);
    if config_path.exists() && !options.force {
        return Err(format!(
            "{} already exists; rerun `ripr init --force` to overwrite it",
            config_path.display()
        ));
    }
    std::fs::write(&config_path, generated_init_config())
        .map_err(|err| format!("write {} failed: {err}", config_path.display()))?;
    println!("Wrote {}", config_path.display());
    Ok(())
}

fn parse_init_options(args: &[String]) -> Result<InitOptions, String> {
    let mut options = InitOptions {
        root: PathBuf::from("."),
        dry_run: false,
        force: false,
    };
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                options.root = PathBuf::from(expect_value(args, i, "--root")?);
            }
            "--dry-run" => options.dry_run = true,
            "--force" => options.force = true,
            other => return Err(format!("unknown init argument {other:?}")),
        }
        i += 1;
    }
    Ok(options)
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
    }

    #[test]
    fn init_rejects_unknown_arguments() {
        assert_eq!(
            init(&args(&["--ci", "github"])),
            Err("unknown init argument \"--ci\"".to_string())
        );
    }

    #[test]
    fn init_parses_root_dry_run_and_force() {
        assert_eq!(
            parse_init_options(&args(&["--root", "repo", "--dry-run", "--force"])),
            Ok(InitOptions {
                root: PathBuf::from("repo"),
                dry_run: true,
                force: true,
            })
        );
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
}
