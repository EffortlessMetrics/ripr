//! Exercise the crate landing page in an archive and a nested checkout.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct SampleCrate {
    sandbox: PathBuf,
    root: PathBuf,
}

impl SampleCrate {
    fn new(checkout: bool) -> Result<Self, Box<dyn std::error::Error>> {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let sandbox = std::env::temp_dir().join(format!(
            "ripr crate readme α {}-{nonce}",
            std::process::id()
        ));
        let root = if checkout {
            sandbox.join("crates/ripr")
        } else {
            sandbox.clone()
        };
        let fixture = Self { sandbox, root };
        std::fs::create_dir_all(&fixture.root)?;
        // The archive case cannot borrow ancestor source files. The checkout
        // case has a real ancestor workspace so removing --root . must fail.
        // The workspace markers also isolate Cargo's target/ temporary root
        // from the repository in which this test itself is running.
        if checkout {
            std::fs::write(
                fixture.sandbox.join("Cargo.toml"),
                "[workspace]\nmembers = [\"crates/ripr\"]\n",
            )?;
        }
        let workspace_marker = if checkout { "" } else { "[workspace]\n" };
        std::fs::write(
            fixture.root.join("Cargo.toml"),
            format!(
                "{workspace_marker}[package]\nname = \"ripr\"\nversion = \"0.1.0\"\nedition = \"2024\"\n"
            ),
        )?;
        let source = Path::new(env!("CARGO_MANIFEST_DIR"));
        for relative in [
            "examples/library.diff",
            "examples/sample/example.diff",
            "examples/sample/src/lib.rs",
            "examples/sample/tests/pricing.rs",
        ] {
            let target = fixture.root.join(relative);
            let parent = target.parent().ok_or("sample target has no parent")?;
            std::fs::create_dir_all(parent)?;
            std::fs::copy(source.join(relative), target)?;
        }
        Ok(fixture)
    }

    fn run(&self, args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
        let output = Command::new(env!("CARGO_BIN_EXE_ripr"))
            .current_dir(&self.root)
            .args(args)
            .output()?;
        assert!(
            output.status.success(),
            "README command `ripr {}` failed:\n{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(String::from_utf8(output.stdout)?)
    }
}

impl Drop for SampleCrate {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.sandbox);
    }
}

#[test]
fn crate_readme_sample_commands_analyze_shipped_sources() -> Result<(), Box<dyn std::error::Error>>
{
    exercise_readme_sample(false)
}

#[test]
fn crate_readme_checkout_commands_keep_the_crate_root() -> Result<(), Box<dyn std::error::Error>> {
    exercise_readme_sample(true)
}

fn exercise_readme_sample(checkout: bool) -> Result<(), Box<dyn std::error::Error>> {
    let readme = include_str!("../README.md");
    let (_, quickstart) = readme
        .split_once("## Quick Start\n")
        .ok_or("crate README has no Quick Start")?;
    // These documentation commands deliberately use simple argv, not a shell.
    let commands: Vec<Vec<&str>> = quickstart
        .lines()
        .take_while(|line| !line.starts_with("## "))
        .filter_map(|line| line.strip_prefix("ripr "))
        .filter(|line| line.split_whitespace().any(|arg| arg == "--diff"))
        .map(|line| line.split_whitespace().collect())
        .collect();
    assert_eq!(
        commands.len(),
        5,
        "exercise every documented sample command"
    );
    let json_check = commands
        .iter()
        .find(|args| args.first() == Some(&"check") && args.contains(&"--json"))
        .ok_or("crate README has no sample JSON check command")?;
    let fixture = SampleCrate::new(checkout)?;
    let output = fixture.run(json_check)?;
    let report: Value = serde_json::from_str(&output)?;
    let findings = report["findings"].as_array().ok_or("missing findings")?;
    assert!(!findings.is_empty(), "the sample must exercise real probes");
    for finding in findings {
        let file = finding["probe"]["file"]
            .as_str()
            .ok_or("finding has no source path")?;
        assert!(
            fixture.root.join(file).is_file(),
            "README sample names a source file absent from the crate: {file}"
        );
        assert_eq!(
            fixture.root.join(file).canonicalize()?,
            fixture
                .root
                .join("examples/sample/src/lib.rs")
                .canonicalize()?,
            "the finding must refer to the isolated sample, not the checkout"
        );
        assert!(
            finding["related_tests_total"].as_u64().unwrap_or(0) > 0,
            "the sample must find its shipped test, not only parse diff text"
        );
    }
    let probe_id = findings
        .first()
        .and_then(|finding| finding["id"].as_str())
        .ok_or("sample finding has no id")?;
    for command in &commands {
        let args: Vec<&str> = command
            .iter()
            .map(|arg| if *arg == "<probe-id>" { probe_id } else { arg })
            .collect();
        let rendered = fixture.run(&args)?;
        if matches!(args.first(), Some(&"explain" | &"context")) {
            assert!(
                rendered.contains(probe_id),
                "the documented finding route must retain the selected id"
            );
        }
        if args.contains(&"--json") {
            let _: Value = serde_json::from_str(&rendered)?;
        }
    }
    Ok(())
}
