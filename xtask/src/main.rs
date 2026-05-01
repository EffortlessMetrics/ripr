#![forbid(unsafe_code)]

use std::process::{Command, ExitStatus};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let result = match args.get(1).map(|s| s.as_str()) {
        Some("ci-fast") => ci_fast(),
        Some("ci-full") => ci_full(),
        Some("package") => run("cargo", &["package", "-p", "ripr", "--list"]).map(|_| ()),
        Some("publish-dry-run") => {
            run("cargo", &["publish", "-p", "ripr", "--dry-run"]).map(|_| ())
        }
        Some("help") | None => {
            print_help();
            Ok(())
        }
        Some(other) => Err(format!("unknown xtask command {other}")),
    };
    if let Err(err) = result {
        eprintln!("xtask: {err}");
        std::process::exit(1);
    }
}

fn ci_fast() -> Result<(), String> {
    run("cargo", &["fmt", "--check"])?;
    run("cargo", &["test", "--workspace"]).map(|_| ())
}

fn ci_full() -> Result<(), String> {
    ci_fast()?;
    run(
        "cargo",
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    run("cargo", &["package", "-p", "ripr", "--list"]).map(|_| ())
}

fn run(program: &str, args: &[&str]) -> Result<ExitStatus, String> {
    eprintln!("$ {} {}", program, args.join(" "));
    let status = Command::new(program)
        .args(args)
        .status()
        .map_err(|err| format!("failed to run {program}: {err}"))?;
    if status.success() {
        Ok(status)
    } else {
        Err(format!("{program} {} failed with {status}", args.join(" ")))
    }
}

fn print_help() {
    println!("xtask commands:\n  ci-fast\n  ci-full\n  package\n  publish-dry-run");
}
