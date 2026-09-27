//! A discovered Python file past the nesting budget must not abort `ripr check`
//! (#4109), even when that file is absent from the diff.
//!
//! Python-only: a build without `lang-python` refuses a Python project
//! before any parse runs, so neither the nesting limit nor its absence is
//! observable there (#4252).
#![cfg(feature = "lang-python")]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn run_ripr(root: &Path, args: &[&str]) -> Result<Output, String> {
    let bin = env!("CARGO_BIN_EXE_ripr");
    Command::new(bin)
        .current_dir(root)
        .args(args)
        .output()
        .map_err(|error| format!("spawn ripr {args:?} failed: {error}"))
}

fn nested_parens(depth: usize) -> String {
    let mut source = String::from("value = ");
    source.push_str(&"(".repeat(depth));
    source.push('1');
    source.push_str(&")".repeat(depth));
    source.push('\n');
    source
}

fn write_workspace(deep: &str) -> Result<PathBuf, String> {
    let root = std::env::temp_dir().join(format!(
        "ripr-py-nest-{}-{}",
        std::process::id(),
        deep.len()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("src")).map_err(|error| format!("mkdir: {error}"))?;
    fs::write(root.join("pyproject.toml"), "[project]\nname = \"nest\"\n")
        .map_err(|error| format!("write marker: {error}"))?;
    fs::write(
        root.join("src/app.py"),
        "def price(amount):\n    return amount - 1\n",
    )
    .map_err(|error| format!("write app: {error}"))?;
    fs::write(root.join("src/deep.py"), deep).map_err(|error| format!("write deep: {error}"))?;
    fs::write(
        root.join("change.diff"),
        "\
diff --git a/src/app.py b/src/app.py
index 0000000..1111111 100644
--- a/src/app.py
+++ b/src/app.py
@@ -1,2 +1,2 @@
 def price(amount):
-    return amount
+    return amount - 1
",
    )
    .map_err(|error| format!("write diff: {error}"))?;
    Ok(root)
}

fn check(root: &Path) -> Result<Output, String> {
    run_ripr(
        root,
        &[
            "check",
            "--root",
            ".",
            "--diff",
            "change.diff",
            "--format",
            "json",
        ],
    )
}

#[test]
fn unchanged_deep_file_is_a_named_limit_not_an_abort() -> Result<(), String> {
    let root = write_workspace(&nested_parens(20_000))?;
    let output = check(&root)?;
    let _ = fs::remove_dir_all(&root);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.code().is_none() {
        return Err(format!(
            "ripr aborted instead of reporting a nesting limit\nstderr: {stderr}\nstdout: {stdout}"
        ));
    }
    if !stdout.contains("parse_budget: nesting depth exceeded 128") {
        return Err(format!(
            "expected the nesting budget in JSON\nstatus: {:?}\nstderr: {stderr}\nstdout: {stdout}",
            output.status.code()
        ));
    }
    if !stdout.contains("language_scope_unsupported") {
        return Err(format!("expected language_scope_unsupported, got {stdout}"));
    }
    if !stdout.contains("src/deep.py") {
        return Err(format!(
            "limitation must name the unchanged file, got {stdout}"
        ));
    }
    Ok(())
}

#[test]
fn depth_at_budget_does_not_disclose_a_nesting_limit() -> Result<(), String> {
    let root = write_workspace(&nested_parens(128))?;
    let output = check(&root)?;
    let _ = fs::remove_dir_all(&root);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.code().is_none() {
        return Err(format!(
            "ripr aborted on a depth-128 file\nstderr: {stderr}"
        ));
    }
    if stdout.contains("parse_budget:") {
        return Err(format!("depth 128 must not trip the budget\n{stdout}"));
    }
    Ok(())
}
