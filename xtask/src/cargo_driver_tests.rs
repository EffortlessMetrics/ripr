//! Native Cargo alias proof without rebuilding the real workspace recursively.

use crate::run::capture_output_in_dir_with_timeout_bounded;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DRIVER: &str = r#"
use std::path::PathBuf;
use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("RIPR_DRIVER_FIXTURE_V1");
    let running = std::env::current_exe()?.canonicalize()?;
    let expected = PathBuf::from(std::env::var_os("RIPR_EXPECTED_DRIVER")
        .ok_or_else(|| std::io::Error::other("fixture driver path missing"))?);
    if running != expected.canonicalize()? {
        return Err(std::io::Error::other("DRIVER_PATH_MISMATCH").into());
    }
    let products = PathBuf::from(std::env::var_os("CARGO_TARGET_DIR")
        .ok_or_else(|| std::io::Error::other("fixture target directory missing"))?);
    let ordinary = products.join("debug").join(format!("xtask{}", std::env::consts::EXE_SUFFIX));
    let status = Command::new(env!("CARGO"))
        .args(["build", "--workspace", "--all-targets"])
        .env("CARGO_ENCODED_RUSTFLAGS", "-Copt-level=1")
        .status()?;
    if !status.success() {
        return Err(std::io::Error::other(format!("WORKSPACE_REBUILD_FAILED: {status}")).into());
    }
    println!("WORKSPACE_REBUILD_OK");
    if running != ordinary.canonicalize()? {
        println!("DRIVER_OUTPUT_SEPARATED");
    }
    if std::env::args().any(|arg| arg == "--fail-gate") {
        return Err(std::io::Error::other("CONTROLLED_GATE_FAILURE").into());
    }
    println!("ALL_GATES_OK");
    Ok(())
}
"#;

#[test]
fn cargo_alias_keeps_live_driver_outside_workspace_rebuild() -> Result<(), String> {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("xtask manifest has no workspace parent")?;
    let config = fs::read_to_string(repo.join(".cargo/config.toml"))
        .map_err(|error| format!("read actual Cargo alias: {error}"))?;
    for layout in [
        ("target", "target", None),
        ("custom-products", "target", None),
        ("custom-products", "custom-intermediates", None),
        ("target", "target", Some("src")),
    ] {
        run_driver_case(repo, &config, layout, false, false)?;
    }
    run_driver_case(repo, &config, ("target", "target", None), true, false)?;
    #[cfg(windows)]
    {
        // Deliberately choosing the reserved driver directory defeats separation.
        run_driver_case(
            repo,
            &config,
            ("target/xtask-driver", "target", None),
            false,
            true,
        )?;
        let mut original: toml::Value = toml::from_str(&config)
            .map_err(|error| format!("parse original-alias control: {error}"))?;
        let alias = original
            .get_mut("alias")
            .and_then(toml::Value::as_table_mut)
            .ok_or("repository Cargo alias table missing")?;
        alias.insert(
            "xtask".into(),
            toml::Value::String("run -p xtask --".into()),
        );
        let original = toml::to_string(&original)
            .map_err(|error| format!("serialize original-alias control: {error}"))?;
        run_driver_case(repo, &original, ("target", "target", None), false, true)?;
    }
    Ok(())
}

fn run_driver_case(
    repo: &Path,
    config: &str,
    layout: (&str, &str, Option<&str>),
    fail_gate: bool,
    expect_replacement_failure: bool,
) -> Result<(), String> {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let fixture = repo
        .join("target")
        .join(format!("cargo-driver-{}-{unique}", std::process::id()));
    let result = run_driver_fixture(
        &fixture,
        config,
        layout,
        fail_gate,
        expect_replacement_failure,
    );
    // Retain failures for inspection, including any uncertain process teardown.
    result.map_err(|error| format!("{error}; fixture retained at {}", fixture.display()))?;
    fs::remove_dir_all(&fixture).map_err(|error| format!("cleanup {}: {error}", fixture.display()))
}

fn run_driver_fixture(
    fixture: &Path,
    config: &str,
    layout: (&str, &str, Option<&str>),
    fail_gate: bool,
    expect_replacement_failure: bool,
) -> Result<(), String> {
    fs::create_dir_all(fixture.join(".cargo")).map_err(|error| error.to_string())?;
    fs::create_dir_all(fixture.join("src")).map_err(|error| error.to_string())?;
    // The repository config directs native temporary files here even when the
    // alias is invoked below the workspace root.
    fs::create_dir_all(fixture.join("target")).map_err(|error| error.to_string())?;
    fs::write(fixture.join(".cargo/config.toml"), config).map_err(|error| error.to_string())?;
    fs::write(
        fixture.join("Cargo.toml"),
        "[package]\nname = \"xtask\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[workspace]\n",
    )
    .map_err(|error| error.to_string())?;
    fs::write(fixture.join("src/main.rs"), DRIVER).map_err(|error| error.to_string())?;
    let (target, build_dir, subdir) = layout;
    let products: PathBuf = fixture.join(target);
    let intermediate = fixture.join(build_dir);
    let invocation = subdir.map_or_else(|| fixture.to_path_buf(), |path| fixture.join(path));
    let driver_products = if expect_replacement_failure {
        products.clone()
    } else {
        invocation.join("target/xtask-driver")
    };
    let expected_driver = driver_products
        .join("debug")
        .join(format!("xtask{}", std::env::consts::EXE_SUFFIX))
        .display()
        .to_string();
    let products = products.display().to_string();
    let intermediate = intermediate.display().to_string();
    let mut args = vec!["xtask".to_string()];
    if fail_gate {
        args.push("--fail-gate".into());
    }
    let output = capture_output_in_dir_with_timeout_bounded(
        Path::new(env!("CARGO")),
        &args,
        &[
            ("CARGO_TARGET_DIR", &products),
            ("CARGO_BUILD_BUILD_DIR", &intermediate),
            ("RUSTFLAGS", ""),
            ("CARGO_ENCODED_RUSTFLAGS", "-Copt-level=0"),
            ("RIPR_EXPECTED_DRIVER", &expected_driver),
        ],
        &invocation,
        Duration::from_mins(2),
        256 * 1024,
        "Cargo driver lifecycle fixture",
    )?;
    if output.timed_out
        || output.stdout_truncated
        || output.stderr_truncated
        || !output.stdout.contains("RIPR_DRIVER_FIXTURE_V1")
    {
        return Err(format!(
            "fixture did not execute completely: {}\n{}",
            output.stdout, output.stderr
        ));
    }
    let success = output.status.is_some_and(|status| status.success());
    if expect_replacement_failure {
        if success
            || !output.stderr.contains("WORKSPACE_REBUILD_FAILED")
            || !output.stderr.contains("xtask.exe")
            || !output.stderr.contains("os error 5")
        {
            return Err(format!(
                "unseparated driver did not reproduce image replacement failure: {}\n{}",
                output.stdout, output.stderr
            ));
        }
    } else if !output.stdout.contains("WORKSPACE_REBUILD_OK")
        || !output.stdout.contains("DRIVER_OUTPUT_SEPARATED")
        || success == fail_gate
        || (fail_gate
            && (!output.stderr.contains("CONTROLLED_GATE_FAILURE")
                || output.stdout.contains("ALL_GATES_OK")))
        || (!fail_gate && !output.stdout.contains("ALL_GATES_OK"))
    {
        return Err(format!(
            "alias did not preserve rebuild and gate outcome: {}\n{}",
            output.stdout, output.stderr
        ));
    }
    Ok(())
}
