use crate::app::{self, CheckInput, Mode, OutputFormat};
use std::path::PathBuf;

pub fn run(args: Vec<String>) -> Result<(), String> {
    match args.get(1).map(|s| s.as_str()) {
        None | Some("--help" | "-h") => {
            print_help();
            Ok(())
        }
        Some("--version" | "-V") => {
            println!("ripr {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("check") => check(&args[2..]),
        Some("explain") => explain(&args[2..]),
        Some("context") => context(&args[2..]),
        Some("doctor") => doctor(&args[2..]),
        Some("lsp") => crate::lsp::serve(),
        Some(command) => Err(format!("unknown command {command:?}. Run `ripr --help`.")),
    }
}

fn check(args: &[String]) -> Result<(), String> {
    let mut input = CheckInput::default();
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
            }
            "--json" => input.format = OutputFormat::Json,
            "--format" => {
                i += 1;
                input.format = parse_format(expect_value(args, i, "--format")?)?;
            }
            "--no-unchanged-tests" => input.include_unchanged_tests = false,
            "--help" | "-h" => {
                print_check_help();
                return Ok(());
            }
            other => return Err(format!("unknown check argument {other:?}")),
        }
        i += 1;
    }
    let format = input.format.clone();
    let output = app::check_workspace(input)?;
    print!("{}", app::render_check(&output, &format));
    Ok(())
}

fn explain(args: &[String]) -> Result<(), String> {
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
                println!(
                    "Usage: ripr explain [--root PATH] [--base REV|--diff PATH] <finding-id|file:line>"
                );
                return Ok(());
            }
            value if selector.is_none() => selector = Some(value.to_string()),
            other => return Err(format!("unexpected explain argument {other:?}")),
        }
        i += 1;
    }
    let selector = selector.ok_or_else(|| "missing finding selector".to_string())?;
    println!("{}", app::explain_finding_with_input(input, &selector)?);
    Ok(())
}

fn context(args: &[String]) -> Result<(), String> {
    let mut input = CheckInput {
        format: OutputFormat::Json,
        ..CheckInput::default()
    };
    let mut selector: Option<String> = None;
    let mut max_tests = 5usize;
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
            }
            "--json" => input.format = OutputFormat::Json,
            "--help" | "-h" => {
                println!(
                    "Usage: ripr context [--root PATH] [--base REV|--diff PATH] --at <finding-id|file:line> [--max-related-tests N] [--json]"
                );
                return Ok(());
            }
            other => return Err(format!("unexpected context argument {other:?}")),
        }
        i += 1;
    }
    let selector = selector.ok_or_else(|| "missing --at or --finding selector".to_string())?;
    println!(
        "{}",
        app::collect_context_with_input(input, &selector, max_tests)?
    );
    Ok(())
}

fn doctor(args: &[String]) -> Result<(), String> {
    let root = if args.first().map(|s| s.as_str()) == Some("--root") {
        args.get(1)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
    } else {
        PathBuf::from(".")
    };
    let mut ok = true;
    println!("ripr doctor");
    if root.join("Cargo.toml").exists() {
        println!(
            "✓ Cargo.toml found at {}",
            root.join("Cargo.toml").display()
        );
    } else {
        println!("! no Cargo.toml found at {}", root.display());
        ok = false;
    }
    match std::process::Command::new("git").arg("--version").output() {
        Ok(output) if output.status.success() => {
            println!("✓ {}", String::from_utf8_lossy(&output.stdout).trim())
        }
        _ => {
            println!("! git not available");
            ok = false;
        }
    }
    match std::process::Command::new("cargo")
        .arg("--version")
        .output()
    {
        Ok(output) if output.status.success() => {
            println!("✓ {}", String::from_utf8_lossy(&output.stdout).trim())
        }
        _ => {
            println!("! cargo not available");
            ok = false;
        }
    }
    if ok {
        Ok(())
    } else {
        Err("doctor found issues".to_string())
    }
}

fn parse_mode(value: &str) -> Result<Mode, String> {
    match value {
        "instant" => Ok(Mode::Instant),
        "draft" => Ok(Mode::Draft),
        "fast" => Ok(Mode::Fast),
        "deep" => Ok(Mode::Deep),
        "ready" => Ok(Mode::Ready),
        _ => Err(format!("unknown mode {value:?}")),
    }
}

fn parse_format(value: &str) -> Result<OutputFormat, String> {
    match value {
        "human" | "text" => Ok(OutputFormat::Human),
        "json" => Ok(OutputFormat::Json),
        "github" => Ok(OutputFormat::Github),
        _ => Err(format!("unknown format {value:?}")),
    }
}

fn expect_value<'a>(args: &'a [String], idx: usize, flag: &str) -> Result<&'a str, String> {
    args.get(idx)
        .map(|s| s.as_str())
        .ok_or_else(|| format!("missing value for {flag}"))
}

fn print_help() {
    println!(
        r#"ripr — static RIPR mutation exposure analysis for Rust

Usage:
  ripr check [--base origin/main] [--diff PATH] [--mode draft] [--format human|json|github]
  ripr explain [--base REV|--diff PATH] <finding-id|file:line>
  ripr context [--base REV|--diff PATH] --at <finding-id|file:line>
  ripr lsp
  ripr doctor

What it does:
  Reads changed Rust code, creates mutation-like probes, and estimates whether
  tests appear to reach, infect, propagate, and reveal the changed behavior
  through meaningful oracles. It does not run mutants.
"#
    );
}

fn print_check_help() {
    println!(
        r#"Usage: ripr check [OPTIONS]

Options:
  --root PATH              Workspace root. Defaults to current directory.
  --base REV               Base revision for git diff. Defaults to origin/main.
  --diff PATH              Read a unified diff file instead of running git diff.
  --mode MODE              instant, draft, fast, deep, or ready. Defaults to draft.
  --format FORMAT          human, json, or github. Defaults to human.
  --json                   Shortcut for --format json.
  --no-unchanged-tests     Reserved for future indexed mode.
"#
    );
}
