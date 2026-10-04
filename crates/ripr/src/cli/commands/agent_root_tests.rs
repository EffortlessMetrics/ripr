//! #5744: real snapshot/verify writers feed the unchanged receipt admission.

use super::*;
use std::os::unix::{ffi::OsStrExt, fs::MetadataExt};

struct OwnedRoot(PathBuf);

impl Drop for OwnedRoot {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) {
            eprintln!(
                "remove owned CLI root fixture {}: {error}",
                self.0.display()
            );
        }
    }
}

fn git(root: &Path, args: &[&str]) -> Result<(), String> {
    crate::testing::fixture_git::fixture_git_ok(root, args)
}

#[test]
fn cli_snapshot_verify_absolute_inputs_retain_literal_unix_root() -> Result<(), String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let parent = std::env::temp_dir().join(format!("ripr-cli-root-{}-{stamp}", std::process::id()));
    std::fs::create_dir(&parent).map_err(|error| error.to_string())?;
    let _owned = OwnedRoot(parent.clone());
    let root = parent.join("team\\repo 'quoted'");
    std::fs::create_dir(&root).map_err(|error| error.to_string())?;
    std::fs::create_dir(root.join("src")).map_err(|error| error.to_string())?;
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"literal_root_fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .map_err(|error| error.to_string())?;
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn identity(value: bool) -> bool { value }\n",
    )
    .map_err(|error| error.to_string())?;
    std::fs::write(root.join(".gitignore"), "/target/\n").map_err(|error| error.to_string())?;
    git(&root, &["init"])?;
    git(&root, &["config", "user.name", "RIPR test"])?;
    git(
        &root,
        &["config", "user.email", "ripr-test@example.invalid"],
    )?;
    git(&root, &["add", "."])?;
    git(&root, &["commit", "--no-gpg-sign", "-m", "before"])?;
    let before = root.join("target/identity/before.json");
    let after = root.join("target/identity/after.json");
    write_agent_repo_exposure_snapshot(&root, &before)?;
    // Real descendant movement, with identical producer-consumed manifests/config.
    std::fs::write(root.join("README.md"), "fixture revision movement\n")
        .map_err(|error| error.to_string())?;
    git(&root, &["add", "README.md"])?;
    git(&root, &["commit", "--no-gpg-sign", "-m", "after"])?;
    write_agent_repo_exposure_snapshot(&root, &after)?;
    let before_bytes = std::fs::read(&before).map_err(|error| error.to_string())?;
    let after_bytes = std::fs::read(&after).map_err(|error| error.to_string())?;
    let decoy_parent = parent.join("team");
    std::fs::create_dir(&decoy_parent).map_err(|error| error.to_string())?;
    let decoy = decoy_parent.join("repo 'quoted'");
    git(
        &parent,
        &[
            "clone",
            "--quiet",
            "--no-hardlinks",
            root.to_str()
                .ok_or_else(|| "fixture root is not UTF-8".to_string())?,
            decoy
                .to_str()
                .ok_or_else(|| "fixture decoy is not UTF-8".to_string())?,
        ],
    )?;
    let selected_meta = std::fs::metadata(&root).map_err(|error| error.to_string())?;
    let decoy_meta = std::fs::metadata(&decoy).map_err(|error| error.to_string())?;
    assert_ne!(
        (selected_meta.dev(), selected_meta.ino()),
        (decoy_meta.dev(), decoy_meta.ino())
    );
    assert_eq!(
        crate::agent::artifact::current_git_head(&root)?,
        crate::agent::artifact::current_git_head(&decoy)?
    );
    let decoy_snapshots = decoy.join("target/identity");
    std::fs::create_dir_all(&decoy_snapshots).map_err(|error| error.to_string())?;
    std::fs::copy(&before, decoy_snapshots.join("before.json"))
        .map_err(|error| error.to_string())?;
    std::fs::copy(&after, decoy_snapshots.join("after.json")).map_err(|error| error.to_string())?;
    let verify = render_agent_verify(&AgentVerifyOptions {
        root: root.clone(),
        before: before.clone(),
        after: after.clone(),
        json: true,
    })?;
    let value: serde_json::Value =
        serde_json::from_str(&verify).map_err(|error| error.to_string())?;
    assert_eq!(
        value["inputs"]["before"].as_str().map(str::as_bytes),
        Some(before.as_os_str().as_bytes()),
        "verify before input lost selected Unix root"
    );
    assert_eq!(
        value["inputs"]["after"].as_str().map(str::as_bytes),
        Some(after.as_os_str().as_bytes()),
        "verify after input lost selected Unix root"
    );
    app::agent_receipt::validate_agent_receipt_verify_json(&root, &verify)?;
    let foreign_refusal = app::agent_receipt::validate_agent_receipt_verify_json(&decoy, &verify)
        .err()
        .ok_or_else(|| "verify was admitted in slash-path decoy".to_string())?;
    assert!(foreign_refusal.contains("must stay under root"));
    let relative = render_agent_verify(&AgentVerifyOptions {
        root: root.clone(),
        before: PathBuf::from("target/identity/before.json"),
        after: PathBuf::from("target/identity/after.json"),
        json: true,
    })?;
    app::agent_receipt::validate_agent_receipt_verify_json(&root, &relative)?;
    assert_eq!(
        std::fs::read(&before).map_err(|error| error.to_string())?,
        before_bytes
    );
    assert_eq!(
        std::fs::read(&after).map_err(|error| error.to_string())?,
        after_bytes
    );
    Ok(())
}
