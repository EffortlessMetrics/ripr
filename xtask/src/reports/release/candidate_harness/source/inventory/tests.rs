use super::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Repo(PathBuf);
impl Drop for Repo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
impl Repo {
    fn new() -> Result<Self, String> {
        let root = std::env::temp_dir().join(format!(
            "ripr-source-budget-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        if let Some(parent) = root.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::create_dir(&root).map_err(|e| e.to_string())?;
        let repo = Self(root);
        git(&repo.0, &["init", "--quiet", "--template="])?;
        for (key, value) in [
            ("user.name", "RIPR fixture"),
            ("user.email", "fixture@example.invalid"),
            ("commit.gpgSign", "false"),
            ("core.autocrlf", "false"),
        ] {
            git(&repo.0, &["config", "--local", key, value])?;
        }
        for (name, bytes) in [
            ("alpha", b"12".as_slice()),
            ("beta", b"345".as_slice()),
            ("empty", b"".as_slice()),
        ] {
            std::fs::write(repo.0.join(name), bytes).map_err(|e| e.to_string())?;
        }
        git(&repo.0, &["-c", "core.hooksPath=", "add", "."])?;
        git(
            &repo.0,
            &[
                "-c",
                "core.hooksPath=",
                "commit",
                "--quiet",
                "-m",
                "source budget fixture",
            ],
        )?;
        Ok(repo)
    }
}
fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let args = args.iter().map(|v| v.to_string()).collect::<Vec<_>>();
    let output = crate::run::capture_bytes_in_dir_with_timeout(
        Path::new("git"),
        &args,
        root,
        &[],
        &[
            "GIT_DIR",
            "GIT_WORK_TREE",
            "GIT_INDEX_FILE",
            "GIT_NO_REPLACE_OBJECTS",
        ],
        Duration::from_secs(5),
        "source inventory fixture",
    )?;
    if output.timed_out || !output.status.is_some_and(|s| s.success()) {
        return Err(format!(
            "fixture Git failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(output.stdout)
}
fn refusal<T>(result: Result<T, String>, expected: &str) -> Result<(), String> {
    match result {
        Err(error) if error.contains(expected) => Ok(()),
        Err(error) => Err(format!("wrong refusal, expected {expected}: {error}")),
        Ok(_) => Err(format!("unexpected acceptance; expected {expected}")),
    }
}

#[test]
fn replacement_objects_cannot_change_inventory_or_admit_substituted_checkout() -> Result<(), String>
{
    let repo = Repo::new()?;
    let sha =
        String::from_utf8(git(&repo.0, &["rev-parse", "HEAD"])?).map_err(|e| e.to_string())?;
    let original = committed_blobs(&repo.0, sha.trim())?;
    let oid = String::from_utf8(git(&repo.0, &["rev-parse", "HEAD:alpha"])?)
        .map_err(|e| e.to_string())?;
    std::fs::write(repo.0.join(".git/replacement"), b"ab").map_err(|e| e.to_string())?;
    let replacement = String::from_utf8(git(&repo.0, &["hash-object", "-w", ".git/replacement"])?)
        .map_err(|e| e.to_string())?;
    git(&repo.0, &["replace", oid.trim(), replacement.trim()])?;
    if git(&repo.0, &["cat-file", "-p", oid.trim()])? != b"ab" {
        return Err("replacement fixture was not active".to_string());
    }
    let under_replacement = committed_blobs(&repo.0, sha.trim())?;
    if original != under_replacement {
        return Err("batch blobs followed replacement objects unlike source identity".to_string());
    }
    verify_checkout(&repo.0, &under_replacement)?;
    std::fs::write(repo.0.join("alpha"), b"ab").map_err(|e| e.to_string())?;
    git(&repo.0, &["update-index", "--skip-worktree", "alpha"])?;
    if !git(&repo.0, &["status", "--porcelain=v1"])?.is_empty() {
        return Err("skip-worktree control did not conceal checkout substitution".to_string());
    }
    refusal(
        verify_checkout(&repo.0, &under_replacement),
        "checkout bytes differ",
    )
}

#[test]
fn source_inventory_and_checkout_enforce_independent_precise_budgets() -> Result<(), String> {
    let repo = Repo::new()?;
    let sha =
        String::from_utf8(git(&repo.0, &["rev-parse", "HEAD"])?).map_err(|e| e.to_string())?;
    let budget = SourceBudget {
        file_bytes: 3,
        retained_bytes: 5,
        files: 3,
    };
    let blobs = committed_blobs_with_budget(&repo.0, sha.trim(), budget)?;
    if blobs.len() != 3 || blobs.values().map(Vec::len).sum::<usize>() != 5 {
        return Err("nonempty exact-budget fixture missing".to_string());
    }
    verify_checkout_with_budget(&repo.0, &blobs, budget)?;
    for (changed, reason) in [
        (
            SourceBudget {
                file_bytes: 2,
                ..budget
            },
            "source blob beta exceeds 2-byte file budget",
        ),
        (
            SourceBudget {
                retained_bytes: 4,
                ..budget
            },
            "4-byte aggregate retained-source budget",
        ),
        (
            SourceBudget { files: 2, ..budget },
            "2 ordinary-blob budget",
        ),
    ] {
        refusal(
            committed_blobs_with_budget(&repo.0, sha.trim(), changed),
            reason,
        )?;
        refusal(
            verify_checkout_with_budget(&repo.0, &blobs, changed),
            reason,
        )?;
    }
    std::fs::write(repo.0.join("alpha"), b"123").map_err(|e| e.to_string())?;
    refusal(verify_checkout(&repo.0, &blobs), "2-byte budget")?;
    std::fs::write(repo.0.join("alpha"), b"ab").map_err(|e| e.to_string())?;
    refusal(verify_checkout(&repo.0, &blobs), "checkout bytes differ")?;
    refusal(
        git_capture(
            &repo.0,
            &["cat-file", "-p", "HEAD:beta"],
            None,
            2,
            "bounded source output",
        ),
        "stdout exceeds its 2-byte output budget",
    )
}

#[test]
fn batch_size_identity_cannot_expand_the_reviewed_source_inventory() -> Result<(), String> {
    let entry = Entry {
        path: "alpha".to_string(),
        oid: "a".repeat(40),
        size: 2,
    };
    let budget = SourceBudget {
        file_bytes: 3,
        retained_bytes: 5,
        files: 3,
    };
    let wrong = format!("{} blob 3\n123\n", entry.oid);
    refusal(
        decode_batch(wrong.as_bytes(), &[entry], budget),
        "identity/type/size differs",
    )
}
