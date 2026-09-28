use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const CACHE_DIR_ENV: &str = "RIPR_CACHE_DIR";
static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

fn temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let sequence = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "ripr-cache-cli-{label}-{}-{nonce}-{sequence}",
        std::process::id()
    ))
}

fn remove_base(base: &Path) -> Result<(), String> {
    if base.exists() {
        fs::remove_dir_all(base).map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// The one spawn site for the exact `ripr` binary in this file.
fn run_ripr(base: &Path, cache_dir: &Path, args: &[&str]) -> Result<std::process::Output, String> {
    Command::new(env!("CARGO_BIN_EXE_ripr"))
        .args(args)
        .current_dir(base)
        .env(CACHE_DIR_ENV, cache_dir)
        .output()
        .map_err(|error| format!("run exact ripr binary {args:?}: {error}"))
}

#[test]
fn cache_clear_force_preserves_unrelated_siblings_through_exact_binary() -> Result<(), String> {
    let base = temp_dir("mixed-root");
    let cache_dir = base.join("mixed-cache");
    let layer = cache_dir.join("repo-file-facts");
    fs::create_dir_all(&layer).map_err(|error| error.to_string())?;
    fs::write(layer.join("entry.json"), b"{}").map_err(|error| error.to_string())?;
    let sentinel = cache_dir.join("KEEP.txt");
    fs::write(&sentinel, b"unrelated user data").map_err(|error| error.to_string())?;

    let output = run_ripr(&base, &cache_dir, &["cache", "clear", "--force"])?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let safe = output.status.success()
        && !layer.exists()
        && sentinel.is_file()
        && cache_dir.is_dir()
        && stdout.contains(&format!("Cache dir: {}", cache_dir.display()))
        && stdout.contains("Preserved the cache root and unrelated siblings");
    remove_base(&base)?;

    if !safe {
        return Err(format!(
            "exact-binary cache clear did not preserve mixed-root ownership boundary\nstatus: {}\nstdout:\n{stdout}\nstderr:\n{stderr}",
            output.status
        ));
    }
    Ok(())
}

#[test]
fn cache_status_hint_names_a_published_command_that_clears_the_same_dir() -> Result<(), String> {
    // #4383: the hint used to route to `cargo xtask cache gc`, which does not
    // exist for users of the published crate.
    let base = temp_dir("status-hint");
    let cache_dir = base.join("hint-cache");
    let layer = cache_dir.join("repo-file-facts");
    fs::create_dir_all(&layer).map_err(|error| error.to_string())?;
    fs::write(layer.join("entry.json"), b"{}").map_err(|error| error.to_string())?;

    let run = |args: &[&str]| run_ripr(&base, &cache_dir, args);
    let status = run(&["cache", "status"])?;
    let hint = String::from_utf8_lossy(&status.stderr).to_string();
    let preview = run(&["cache", "clear", "--dry-run"])?;
    let preview_stdout = String::from_utf8_lossy(&preview.stdout).to_string();
    let entry_kept = layer.join("entry.json").is_file();
    remove_base(&base)?;

    let names_published_command = hint.contains("`ripr cache clear --dry-run`")
        && hint.contains("`ripr cache clear --force`")
        && !hint.contains("xtask");
    let preview_targets_same_dir = preview.status.success()
        && preview_stdout.contains(&format!("Cache dir: {}", cache_dir.display()))
        && entry_kept;
    if !(status.status.success() && names_published_command && preview_targets_same_dir) {
        return Err(format!(
            "cache status hint must name a runnable published command for the same dir\nhint:\n{hint}\npreview status: {}\npreview stdout:\n{preview_stdout}",
            preview.status
        ));
    }
    Ok(())
}
