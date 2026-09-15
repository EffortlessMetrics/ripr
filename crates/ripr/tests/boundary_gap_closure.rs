//! End-to-end product guard for the canonical predicate-boundary repair.
//!
//! The historical checked calibration artifacts predate the producer fix that
//! recognizes equal concrete arguments for parameter-to-parameter boundaries.
//! This journey exercises the built `ripr` binary so the diff-free repo
//! exposure and outcome surfaces cannot drift back to reporting the correctly
//! repaired equality boundary as still weak.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const LIB_SOURCE: &str = r#"pub fn discounted_total(amount: i32, discount_threshold: i32) -> i32 {
    if amount >= discount_threshold {
        amount - 10
    } else {
        amount
    }
}
"#;

const BEFORE_TESTS: &str = r#"use boundary_gap_closure::discounted_total;

#[test]
fn below_threshold_has_no_discount() {
    assert_eq!(discounted_total(50, 100), 50);
}

#[test]
fn far_above_threshold_discounts() {
    assert_eq!(discounted_total(10_000, 100), 9_990);
}
"#;

const AFTER_TESTS: &str = r#"use boundary_gap_closure::discounted_total;

#[test]
fn below_threshold_has_no_discount() {
    assert_eq!(discounted_total(50, 100), 50);
}

#[test]
fn far_above_threshold_discounts() {
    assert_eq!(discounted_total(10_000, 100), 9_990);
}

#[test]
fn equality_boundary_discounts() {
    assert_eq!(discounted_total(100, 100), 90);
}
"#;

struct TempRepo {
    root: PathBuf,
}

impl TempRepo {
    fn create() -> Result<Self, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("clock before Unix epoch: {error}"))?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "ripr-boundary-gap-closure-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(root.join("src"))
            .map_err(|error| format!("create {} failed: {error}", root.display()))?;
        std::fs::create_dir_all(root.join("tests"))
            .map_err(|error| format!("create tests directory failed: {error}"))?;
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"boundary-gap-closure\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[workspace]\n",
        )
        .map_err(|error| format!("write Cargo.toml failed: {error}"))?;
        std::fs::write(root.join("src/lib.rs"), LIB_SOURCE)
            .map_err(|error| format!("write src/lib.rs failed: {error}"))?;
        std::fs::write(root.join("tests/pricing.rs"), BEFORE_TESTS)
            .map_err(|error| format!("write tests/pricing.rs failed: {error}"))?;

        run_git(&root, &["init", "-q"])?;
        run_git(&root, &["config", "user.name", "ripr fixture"])?;
        run_git(&root, &["config", "user.email", "fixture@ripr.invalid"])?;
        run_git(&root, &["config", "core.autocrlf", "false"])?;
        run_git(&root, &["add", "."])?;
        run_git(&root, &["commit", "-qm", "initial boundary fixture"])?;

        Ok(Self { root })
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn run_git(root: &Path, args: &[&str]) -> Result<(), String> {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .map_err(|error| format!("git {args:?} failed to start: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    Err(command_failure(&format!("git {args:?}"), &output))
}

fn command_failure(label: &str, output: &Output) -> String {
    format!(
        "{label} failed with {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn run_repo_exposure(root: &Path) -> Result<String, String> {
    let output = Command::new(env!("CARGO_BIN_EXE_ripr"))
        .args(["check", "--root"])
        .arg(root)
        .args(["--mode", "ready", "--format", "repo-exposure-json"])
        .output()
        .map_err(|error| format!("ripr check failed to start: {error}"))?;
    if !output.status.success() {
        return Err(command_failure(
            "ripr check --format repo-exposure-json",
            &output,
        ));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| format!("ripr repo exposure emitted non-UTF-8 stdout: {error}"))
}

fn run_outcome(before: &Path, after: &Path) -> Result<Value, String> {
    let output = Command::new(env!("CARGO_BIN_EXE_ripr"))
        .args(["outcome", "--before"])
        .arg(before)
        .arg("--after")
        .arg(after)
        .args(["--format", "json"])
        .output()
        .map_err(|error| format!("ripr outcome failed to start: {error}"))?;
    if !output.status.success() {
        return Err(command_failure("ripr outcome --format json", &output));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("ripr outcome emitted invalid JSON: {error}"))
}

fn parse_json(label: &str, text: &str) -> Result<Value, String> {
    serde_json::from_str(text).map_err(|error| format!("{label} JSON failed to parse: {error}"))
}

fn boundary_seam(value: &Value) -> Result<&Value, String> {
    value
        .get("seams")
        .and_then(Value::as_array)
        .and_then(|seams| {
            seams.iter().find(|seam| {
                seam.get("kind").and_then(Value::as_str) == Some("predicate_boundary")
                    && seam
                        .get("expression")
                        .and_then(Value::as_str)
                        .is_some_and(|expression| {
                            expression.contains("amount >= discount_threshold")
                        })
            })
        })
        .ok_or_else(|| "repo exposure did not contain the boundary predicate seam".to_string())
}

fn missing_discriminators(seam: &Value) -> Result<&[Value], String> {
    seam.get("missing_discriminators")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| "boundary seam is missing the missing_discriminators array".to_string())
}

#[test]
fn equality_boundary_repair_closes_across_repo_exposure_and_outcome() -> Result<(), String> {
    let repo = TempRepo::create()?;

    let before_text = run_repo_exposure(&repo.root)?;
    let before = parse_json("before repo exposure", &before_text)?;
    let before_seam = boundary_seam(&before)?;
    assert_eq!(
        before_seam.get("grip_class").and_then(Value::as_str),
        Some("weakly_gripped")
    );
    assert_eq!(
        before_seam
            .get("headline_eligible")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert!(missing_discriminators(before_seam)?.iter().any(|fact| {
        fact.get("value")
            .and_then(Value::as_str)
            .is_some_and(|value| value.contains("equality boundary"))
    }));

    let before_path = repo.root.join("before.repo-exposure.json");
    std::fs::write(&before_path, &before_text)
        .map_err(|error| format!("write before snapshot failed: {error}"))?;

    std::fs::write(repo.root.join("tests/pricing.rs"), AFTER_TESTS)
        .map_err(|error| format!("write equality boundary test failed: {error}"))?;

    let after_text = run_repo_exposure(&repo.root)?;
    let after = parse_json("after repo exposure", &after_text)?;
    let after_seam = boundary_seam(&after)?;
    assert_eq!(
        after_seam.get("grip_class").and_then(Value::as_str),
        Some("strongly_gripped")
    );
    assert_eq!(
        after_seam.get("headline_eligible").and_then(Value::as_bool),
        Some(false)
    );
    assert!(missing_discriminators(after_seam)?.is_empty());
    assert!(
        after_seam
            .get("related_tests")
            .and_then(Value::as_array)
            .is_some_and(|tests| {
                tests.iter().any(|test| {
                    test.get("name").and_then(Value::as_str) == Some("equality_boundary_discounts")
                })
            })
    );

    let after_path = repo.root.join("after.repo-exposure.json");
    std::fs::write(&after_path, &after_text)
        .map_err(|error| format!("write after snapshot failed: {error}"))?;

    let outcome = run_outcome(&before_path, &after_path)?;
    assert_eq!(
        outcome.pointer("/summary/moved").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        outcome
            .pointer("/summary/unchanged")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        outcome
            .pointer("/summary/gap_movement/closed")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        outcome
            .pointer("/moved/0/gap_movement")
            .and_then(Value::as_str),
        Some("closed")
    );
    assert_eq!(
        outcome.pointer("/moved/0/before").and_then(Value::as_str),
        Some("weakly_gripped")
    );
    assert_eq!(
        outcome.pointer("/moved/0/after").and_then(Value::as_str),
        Some("strongly_gripped")
    );
    assert!(
        outcome
            .pointer("/moved/0/missing_discriminators_resolved")
            .and_then(Value::as_array)
            .is_some_and(|items| {
                items.iter().any(|item| {
                    item.as_str()
                        .is_some_and(|value| value.contains("equality boundary"))
                })
            })
    );

    Ok(())
}
