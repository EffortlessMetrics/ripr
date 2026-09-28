//! #4275: `ripr pr-evidence` output must be admissible producer evidence for
//! `ripr review-comments --check-output`. Both commands run as the built
//! binary against one isolated two-commit repository, so the proof covers
//! the real subject writer and the real admission check, not a fabricated
//! subject.

use serde_json::Value;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::process::Output;
use std::time::{SystemTime, UNIX_EPOCH};

const LIB_BASE: &str = "pub fn discount(total: u32) -> u32 {
    let threshold = 100;
    let rate = 10;
    if total >= threshold {
        return total - rate;
    }
    total
}
";

#[test]
fn review_comments_admits_pr_evidence_producer_output() -> Result<(), Box<dyn Error>> {
    let repo = fixture_repo("admits")?;
    let evidence = ripr(&repo, &["pr-evidence", "--base", "base", "--head", "HEAD"])?;
    require_success("pr-evidence", &evidence)?;

    // The presentation packet keeps its context lines; the subject binds the
    // canonical zero-context analysis diff instead of these bytes.
    let packet = fs::read_to_string(repo.join("target/ripr/pr/pr.diff"))?;
    assert!(
        packet.contains("\n     let rate = 10;\n"),
        "pr.diff lost its context lines:\n{packet}"
    );

    let review = review_comments(&repo)?;
    require_success("review-comments", &review)?;
    let comments: Value = serde_json::from_str(&fs::read_to_string(
        repo.join("target/ripr/review/comments.json"),
    )?)?;
    let subject: Value = serde_json::from_str(&fs::read_to_string(
        repo.join("target/ripr/pr/check.subject.json"),
    )?)?;
    let bound_diff = subject["canonical_diff_sha256"]
        .as_str()
        .ok_or("subject canonical_diff_sha256 missing")?;
    assert_eq!(
        comments["run_receipt"]["analysis_identity"]["canonical_diff_sha256"].as_str(),
        Some(bound_diff),
        "review-comments did not render the admitted producer identity: {comments}"
    );
    // The producer analyzed exactly the bytes it bound.
    assert_eq!(
        subject["analysis_outcome"]["outcome"]["identity"]["input_identity"].as_str(),
        Some(bound_diff),
        "producer analysis input differs from the bound canonical diff: {subject}"
    );

    fs::remove_dir_all(repo)?;
    Ok(())
}

#[test]
fn review_comments_refuses_pr_evidence_after_head_moves() -> Result<(), Box<dyn Error>> {
    let repo = fixture_repo("head-moved")?;
    let evidence = ripr(&repo, &["pr-evidence", "--base", "base", "--head", "HEAD"])?;
    require_success("pr-evidence", &evidence)?;

    fs::write(
        repo.join("src/lib.rs"),
        LIB_BASE.replace("total >= threshold", "total != threshold"),
    )?;
    git(&repo, &["commit", "-q", "-am", "move head"])?;

    let review = review_comments(&repo)?;
    let stderr = String::from_utf8_lossy(&review.stderr);
    assert!(
        !review.status.success() && stderr.contains("producer_identity_mismatch"),
        "stale producer evidence must be refused: status={:?} stderr={stderr}",
        review.status.code()
    );

    fs::remove_dir_all(repo)?;
    Ok(())
}

fn review_comments(repo: &Path) -> Result<Output, Box<dyn Error>> {
    ripr(
        repo,
        &[
            "review-comments",
            "--base",
            "base",
            "--head",
            "HEAD",
            "--check-output",
            "target/ripr/pr/check.json",
            "--out",
            "target/ripr/review/comments.json",
        ],
    )
}

/// Two commits: `base` holds the function, `HEAD` edits the line in the
/// middle, so a three-context diff and a zero-context diff differ.
fn fixture_repo(label: &str) -> Result<PathBuf, Box<dyn Error>> {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let repo = std::env::temp_dir().join(format!(
        "ripr-pr-evidence-review-comments-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(repo.join("src"))?;
    fs::write(
        repo.join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )?;
    fs::write(repo.join("src/lib.rs"), LIB_BASE)?;
    git(&repo, &["init", "-q"])?;
    git(&repo, &["config", "user.email", "ripr@example.invalid"])?;
    git(&repo, &["config", "user.name", "RIPR Test"])?;
    git(&repo, &["config", "commit.gpgsign", "false"])?;
    git(&repo, &["add", "."])?;
    git(&repo, &["commit", "-q", "-m", "base"])?;
    git(&repo, &["tag", "base"])?;
    fs::write(
        repo.join("src/lib.rs"),
        LIB_BASE.replace("total >= threshold", "total > threshold"),
    )?;
    git(&repo, &["commit", "-q", "-am", "change"])?;
    Ok(repo)
}

fn git(repo: &Path, args: &[&str]) -> Result<(), Box<dyn Error>> {
    let output = run("git", repo, args)?;
    require_success("git", &output)
}

fn ripr(repo: &Path, args: &[&str]) -> Result<Output, Box<dyn Error>> {
    run(env!("CARGO_BIN_EXE_ripr"), repo, args)
}

fn run(program: &str, repo: &Path, args: &[&str]) -> Result<Output, Box<dyn Error>> {
    Ok(Command::new(program)
        .args(args)
        .current_dir(repo)
        .output()?)
}

fn require_success(label: &str, output: &Output) -> Result<(), Box<dyn Error>> {
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "{label} failed: status={:?} stdout={} stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .into())
}
