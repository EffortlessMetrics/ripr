use crate::cli::help;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(super) fn run(args: &[String]) -> Result<(), String> {
    let Some(root) = parse_root(args)? else {
        return Ok(());
    };

    let mut ok = true;
    println!("ripr doctor");
    ok &= check_cargo_toml(&root);
    ok &= check_tool("git", "--version");
    ok &= check_tool("cargo", "--version");

    if ok {
        Ok(())
    } else {
        Err("doctor found issues".to_string())
    }
}

fn parse_root(args: &[String]) -> Result<Option<PathBuf>, String> {
    match args {
        [] => Ok(Some(PathBuf::from("."))),
        [flag] if flag == "--help" || flag == "-h" => {
            help::print_doctor_help();
            Ok(None)
        }
        [flag] if flag == "--root" => Err("missing value for --root".to_string()),
        [flag, value] if flag == "--root" => Ok(Some(PathBuf::from(value))),
        [other, ..] => Err(format!("unknown doctor argument {other:?}")),
    }
}

fn check_cargo_toml(root: &Path) -> bool {
    let path = root.join("Cargo.toml");
    if path.exists() {
        println!("✓ Cargo.toml found at {}", path.display());
        true
    } else {
        println!("! no Cargo.toml found at {}", root.display());
        false
    }
}

fn check_tool(program: &str, arg: &str) -> bool {
    match Command::new(program).arg(arg).output() {
        Ok(output) if output.status.success() => {
            println!("✓ {}", String::from_utf8_lossy(&output.stdout).trim());
            true
        }
        _ => {
            println!("! {program} not available");
            false
        }
    }
}
