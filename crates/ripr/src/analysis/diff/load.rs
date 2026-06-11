use std::path::{Path, PathBuf};
use std::process::Command;

pub fn load_diff(
    root: &Path,
    base: Option<&str>,
    diff_file: Option<&PathBuf>,
) -> Result<String, String> {
    if let Some(diff_file) = diff_file {
        return std::fs::read_to_string(diff_file)
            .map_err(|err| format!("failed to read diff file {}: {err}", diff_file.display()));
    }

    let base = base.unwrap_or("origin/main");
    run_git_diff(root, &format!("{base}...HEAD"), &[])
}

pub fn load_diff_range(root: &Path, base: &str, head: &str) -> Result<String, String> {
    run_git_diff(
        root,
        &format!("{base}...{head}"),
        &["--unified=0", "--no-ext-diff"],
    )
}

fn run_git_diff(root: &Path, range: &str, extra_args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("diff")
        .args(extra_args)
        .arg(range)
        .current_dir(root)
        .output()
        .map_err(|err| format!("failed to run git diff: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "git diff failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "Test asserts an expected error variant via `.expect_err(\"why\")`; the closure-style helper makes the expected failure mode part of the assertion message."
)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn load_diff_from_file_returns_content() -> std::io::Result<()> {
        let dir = std::env::temp_dir().join("ripr-load-diff-test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir)?;
        let diff_file = dir.join("test.diff");
        fs::write(&diff_file, "test content")?;

        let result = load_diff(&dir, None, Some(&diff_file));
        assert_eq!(result.as_deref(), Ok("test content"));

        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn load_diff_with_missing_file_returns_error() -> std::io::Result<()> {
        let result = load_diff(
            &std::env::current_dir()?,
            None,
            Some(&PathBuf::from("/nonexistent/path/to/file")),
        );
        result.expect_err("expected diff load to fail for missing file");
        Ok(())
    }
}
