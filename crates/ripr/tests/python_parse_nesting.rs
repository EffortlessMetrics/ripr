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
use std::sync::atomic::{AtomicUsize, Ordering};

static WORKSPACE_COUNTER: AtomicUsize = AtomicUsize::new(0);

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
    // Unique per call: tests run in parallel and two sources may share a length.
    let call = WORKSPACE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "ripr-py-nest-{}-{call}-{}",
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

fn assert_tree_budget_disclosed(label: &str, deep: &str) -> Result<(), String> {
    let root = write_workspace(deep)?;
    let output = check(&root)?;
    let _ = fs::remove_dir_all(&root);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.code().is_none() {
        return Err(format!(
            "{label}: ripr aborted instead of reporting a tree-depth limit\nstderr: {stderr}"
        ));
    }
    if !stdout.contains("parse_budget: syntax tree depth estimate exceeded 256") {
        return Err(format!(
            "{label}: expected the tree-depth budget in JSON\nstatus: {:?}\nstderr: {stderr}\nstdout: {stdout}",
            output.status.code()
        ));
    }
    if !stdout.contains("language_scope_unsupported") || !stdout.contains("src/deep.py") {
        return Err(format!(
            "{label}: limitation must be language_scope_unsupported naming src/deep.py, got {stdout}"
        ));
    }
    Ok(())
}

#[test]
fn unchanged_deep_operator_chain_is_a_named_limit_not_an_abort() -> Result<(), String> {
    assert_tree_budget_disclosed(
        "binary",
        &format!("def f(x):\n    return x{}\n", " + 1".repeat(60_000)),
    )
}

#[test]
fn unchanged_deep_unary_chain_is_a_named_limit_not_an_abort() -> Result<(), String> {
    assert_tree_budget_disclosed(
        "unary",
        &format!("def f(x):\n    return {}x\n", "not ".repeat(50_000)),
    )
}

#[test]
fn unchanged_deep_elif_chain_is_a_named_limit_not_an_abort() -> Result<(), String> {
    let mut source = String::from("def f(x):\n    if x == 0:\n        return 0\n");
    for arm in 1..12_000 {
        source.push_str(&format!("    elif x == {arm}:\n        return {arm}\n"));
    }
    assert_tree_budget_disclosed("elif", &source)
}

#[test]
fn unchanged_deep_parameterized_lambda_chain_is_a_named_limit_not_an_abort() -> Result<(), String> {
    assert_tree_budget_disclosed(
        "lambda",
        &format!("value = {}0\n", "lambda a, b: ".repeat(50_000)),
    )
}

#[test]
fn unchanged_deep_fstring_field_is_a_named_limit_not_an_abort() -> Result<(), String> {
    assert_tree_budget_disclosed(
        "f-string",
        &format!("value = f\"{{x{}}}\"\n", " + 1".repeat(60_000)),
    )
}

#[test]
fn unchanged_deep_cr_only_elif_chain_is_a_named_limit_not_an_abort() -> Result<(), String> {
    let mut source = String::from("def f(x):\r    if x == 0:\r        return 0\r");
    for arm in 1..12_000 {
        source.push_str(&format!("    elif x == {arm}:\r        return {arm}\r"));
    }
    assert_tree_budget_disclosed("CR elif", &source)
}
