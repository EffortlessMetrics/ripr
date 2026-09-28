//! #4360: shipped command-path controls for advisory output acquisition.

use super::*;
use std::time::{Duration, Instant};

struct Case {
    directory: PathBuf,
    root: PathBuf,
    out: PathBuf,
    arguments: Vec<String>,
}

impl Drop for Case {
    fn drop(&mut self) {
        ignore_remove_dir_all(&self.directory);
    }
}

fn case(surface: &str) -> Result<Case, Box<dyn std::error::Error>> {
    let directory = unique_temp_workspace(&format!("advisory-write-{surface}"));
    let root = directory.join("checkout");
    let out = if surface == "index" {
        root.join("target/ripr/reports/index.json")
    } else {
        root.join("report.json")
    };
    let mut fixture = Case {
        directory,
        root,
        out,
        arguments: Vec::new(),
    };
    let root = &fixture.root;
    let out = &fixture.out;
    std::fs::create_dir_all(root)?;
    let arguments = match surface {
        "index" => vec!["reports".into(), "index".into()],
        "outcome" => {
            write_outcome_snapshots(root)?;
            vec![
                "outcome".into(),
                "--before".into(),
                root.join("before.json").display().to_string(),
                "--after".into(),
                root.join("after.json").display().to_string(),
                "--format".into(),
                "json".into(),
                "--out".into(),
                out.display().to_string(),
            ]
        }
        "receipt" => {
            init_git_fixture_repo(root)?;
            let (_, _, verify, _) = write_moving_pair_and_verify(
                root,
                r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"weakly_gripped"}"#,
                r#"{"seam_id":"seam-a","kind":"predicate_boundary","file":"src/pricing.rs","line":42,"grip_class":"strongly_gripped"}"#,
            )?;
            vec![
                "agent".into(),
                "receipt".into(),
                "--root".into(),
                root.display().to_string(),
                "--verify-json".into(),
                verify.display().to_string(),
                "--seam-id".into(),
                "seam-a".into(),
                "--json".into(),
                "--out".into(),
                out.display().to_string(),
            ]
        }
        "calibrate" => {
            let fixtures = workspace_root().join("fixtures/boundary_gap/calibration");
            vec![
                "calibrate".into(),
                "cargo-mutants".into(),
                "--mutants-json".into(),
                fixtures.join("runtime-mutants.json").display().to_string(),
                "--repo-exposure-json".into(),
                fixtures
                    .join("after-targeted-test.repo-exposure.json")
                    .display()
                    .to_string(),
                "--format".into(),
                "json".into(),
                "--out".into(),
                out.display().to_string(),
            ]
        }
        other => return Err(format!("unknown advisory fixture surface: {other}").into()),
    };
    fixture.arguments = arguments;
    Ok(fixture)
}

/// Keep a missing nonblocking flag from hanging the whole suite. Redirecting
/// both streams avoids pipe backpressure; timeout owns termination and reap.
fn run_bounded(case: &Case) -> Result<Output, Box<dyn std::error::Error>> {
    let stdout = case.directory.join("command.stdout");
    let stderr = case.directory.join("command.stderr");
    let mut command = Command::new(env!("CARGO_BIN_EXE_ripr"));
    command
        .current_dir(&case.root)
        .args(&case.arguments)
        .stdout(std::fs::File::create(&stdout)?)
        .stderr(std::fs::File::create(&stderr)?);
    let mut child = ripr::process_owner::OwnedProcess::spawn(command)?;
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() >= Duration::from_secs(10) {
            child.terminate_tree()?;
            return Err("advisory CLI exceeded ten seconds; child terminated and reaped".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    Ok(Output {
        status,
        stdout: std::fs::read(stdout)?,
        stderr: std::fs::read(stderr)?,
    })
}

fn successful_render(case: &Case) -> Result<(), Box<dyn std::error::Error>> {
    let output = run_bounded(case)?;
    assert_success(&output);
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(&case.out)?)?;
    assert!(
        value.is_object(),
        "actual output must be a rendered JSON report"
    );
    assert!(
        value.get("schema_version").is_some(),
        "report subject must have schema identity"
    );
    Ok(())
}

#[test]
fn advisory_write_fresh_and_regular_overwrite() -> Result<(), Box<dyn std::error::Error>> {
    for surface in ["index", "outcome", "receipt", "calibrate"] {
        let case = case(surface)?;
        successful_render(&case)?;
        std::fs::write(&case.out, "old regular report")?;
        successful_render(&case)?;
        println!("advisory-write positive: {surface}: fresh and existing regular file");
    }
    Ok(())
}

#[cfg(unix)]
fn planted_symlink(surface: &str) -> Result<(), Box<dyn std::error::Error>> {
    let case = case(surface)?;
    // The same valid inputs and actual command succeed before hostile setup.
    successful_render(&case)?;
    std::fs::remove_file(&case.out)?;
    let sentinel = case.directory.join("outside-checkout.txt");
    let original = b"outside sentinel must not change";
    std::fs::write(&sentinel, original)?;
    std::os::unix::fs::symlink(&sentinel, &case.out)?;
    assert!(
        std::fs::symlink_metadata(&case.out)?
            .file_type()
            .is_symlink()
    );
    let output = run_bounded(&case)?;
    let observed = std::fs::read(&sentinel)?;
    if observed != original {
        return Err(format!(
            "real {surface} command followed planted output symlink: status={}, outside sentinel changed to {} bytes starting {:?}",
            output.status, observed.len(), &observed[..observed.len().min(32)],
        ).into());
    }
    assert_failure(&output);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("output"),
        "refusal must identify output acquisition, not an unrelated input error: {:?}",
        output
    );
    Ok(())
}

#[test]
fn advisory_write_refuses_directory_outputs() -> Result<(), Box<dyn std::error::Error>> {
    for surface in ["index", "outcome", "receipt", "calibrate"] {
        let case = case(surface)?;
        successful_render(&case)?;
        std::fs::remove_file(&case.out)?;
        std::fs::create_dir(&case.out)?;
        let protected = case.out.join("protected.txt");
        std::fs::write(&protected, b"directory sentinel")?;
        let output = run_bounded(&case)?;
        assert_failure(&output);
        assert!(String::from_utf8_lossy(&output.stderr).contains("output"));
        assert_eq!(std::fs::read(protected)?, b"directory sentinel");
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn reports_index_rejects_planted_output_symlink() -> Result<(), Box<dyn std::error::Error>> {
    planted_symlink("index")
}

#[cfg(unix)]
#[test]
fn outcome_rejects_planted_output_symlink() -> Result<(), Box<dyn std::error::Error>> {
    planted_symlink("outcome")
}

#[cfg(unix)]
#[test]
fn agent_receipt_rejects_planted_output_symlink() -> Result<(), Box<dyn std::error::Error>> {
    planted_symlink("receipt")
}

#[cfg(unix)]
#[test]
fn calibrate_rejects_planted_output_symlink() -> Result<(), Box<dyn std::error::Error>> {
    planted_symlink("calibrate")
}

#[cfg(all(
    any(target_os = "linux", target_os = "macos"),
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
fn reports_index_refuses_fifo_without_waiting_for_reader() -> Result<(), Box<dyn std::error::Error>>
{
    use std::os::unix::fs::FileTypeExt as _;
    let case = case("index")?;
    successful_render(&case)?;
    std::fs::remove_file(&case.out)?;
    let setup = run_command(
        "mkfifo",
        None,
        &[case.out.to_str().ok_or("non-UTF8 FIFO path")?],
    )?;
    assert_success(&setup);
    assert!(std::fs::symlink_metadata(&case.out)?.file_type().is_fifo());
    let output = run_bounded(&case)?;
    assert_failure(&output);
    assert!(String::from_utf8_lossy(&output.stderr).contains("output"));
    Ok(())
}
