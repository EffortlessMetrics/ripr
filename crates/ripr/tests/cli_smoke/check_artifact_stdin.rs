//! Stdin admission and named-file recovery for RIPR-SPEC-0140 (#5112).

use super::{
    assert_success, context_reuse_matches_fresh, ignore_remove_dir_all, spawn_command,
    unique_temp_workspace,
};
use std::path::Path;
use std::process::Output;

#[cfg(feature = "lang-python")]
const PYTHON_PATCH: &str = "--- a/src/discount.py\n+++ b/src/discount.py\n@@ -1,4 +1,4 @@\n def discount(amount):\n-    if amount > 100:\n+    if amount >= 100:\n         return 10\n     return 0\n";
#[cfg(feature = "lang-python")]
const PYTHON_COMMENT: &str = "--- a/src/discount.py\n+++ b/src/discount.py\n@@ -1,4 +1,5 @@\n+# pricing documentation\n def discount(amount):\n     if amount >= 100:\n         return 10\n     return 0\n";
#[cfg(feature = "lang-typescript")]
const JAVASCRIPT_PATCH: &str = "--- a/src/discount.js\n+++ b/src/discount.js\n@@ -1,3 +1,3 @@\n export function discount(amount) {\n-  return amount > 100;\n+  return amount >= 100;\n }\n";

fn run(root: &Path, args: &[&str], stdin: Option<&[u8]>) -> Result<Output, String> {
    spawn_command(
        env!("CARGO_BIN_EXE_ripr"),
        Some(root),
        args,
        &[],
        None,
        stdin,
    )
    .map_err(|error| format!("run ripr {args:?}: {error}"))
}

fn write(root: &Path, path: &str, bytes: impl AsRef<[u8]>) -> Result<(), String> {
    std::fs::write(root.join(path), bytes).map_err(|error| format!("write {path}: {error}"))
}

fn exercise(root: &Path, patch: &str, unrelated: &str, selector: &str) -> Result<(), String> {
    write(root, "change.diff", patch)?;
    let named = run(
        root,
        &[
            "check",
            "--root",
            ".",
            "--diff",
            "change.diff",
            "--json",
            "--write-artifact",
            "accepted.json",
        ],
        None,
    )?;
    assert_success(&named);
    let parsed: serde_json::Value = serde_json::from_slice(&named.stdout)
        .map_err(|error| format!("parse named check: {error}"))?;
    assert_eq!(parsed["summary"]["probes"], 1);
    assert_eq!(parsed["findings"].as_array().map(Vec::len), Some(1));
    let accepted = std::fs::read(root.join("accepted.json"))
        .map_err(|error| format!("read accepted artifact: {error}"))?;
    let ordinary = run(
        root,
        &["check", "--root", ".", "--diff", "-", "--json"],
        Some(patch.as_bytes()),
    )?;
    assert_success(&ordinary);
    let stdin_parsed: serde_json::Value = serde_json::from_slice(&ordinary.stdout)
        .map_err(|error| format!("parse stdin check: {error}"))?;
    assert_eq!(
        parsed, stdin_parsed,
        "ordinary stdin must retain named-file analysis"
    );

    // The unrelated cwd entry must not control admission or identity. Both
    // streams used to record the same identity despite different findings.
    // Admission receives EOF; no analysis/progress may happen first.
    for dash in [None, Some(""), Some(unrelated)] {
        if let Some(bytes) = dash {
            write(root, "-", bytes)?;
        }
        for destination in ["new.json", "accepted.json"] {
            let refused = run(
                root,
                &[
                    "check",
                    "--root",
                    ".",
                    "--diff",
                    "-",
                    "--json",
                    "--write-artifact",
                    destination,
                ],
                None,
            )?;
            let stderr = String::from_utf8_lossy(&refused.stderr);
            assert_eq!(refused.status.code(), Some(2), "{stderr}");
            assert!(refused.stdout.is_empty(), "refusal must precede output");
            assert!(
                stderr.contains(&format!(
                    "--write-artifact {destination} cannot be combined with --diff -"
                )),
                "{stderr}"
            );
            assert!(
                stderr.contains("save stdin to a named diff file"),
                "{stderr}"
            );
            assert!(stderr.contains("pass --diff <path>"), "{stderr}");
            assert!(!stderr.contains("ripr progress:"), "analysis ran: {stderr}");
            assert!(
                !root.join("new.json").exists(),
                "refusal created an artifact"
            );
            assert_eq!(
                std::fs::read(root.join("accepted.json"))
                    .map_err(|error| format!("read preserved artifact: {error}"))?,
                accepted,
                "refusal replaced the accepted artifact"
            );
        }
    }

    // Follow the recovery instruction: the saved named diff still writes
    // and reuses the nonempty result after every refusal.
    let recovered = run(
        root,
        &[
            "check",
            "--root",
            ".",
            "--diff",
            "change.diff",
            "--json",
            "--write-artifact",
            "accepted.json",
        ],
        None,
    )?;
    assert_success(&recovered);
    assert_eq!(
        std::fs::read(root.join("accepted.json"))
            .map_err(|error| format!("read recovered artifact: {error}"))?,
        accepted
    );
    let fresh = run(
        root,
        &[
            "context",
            "--root",
            ".",
            "--diff",
            "change.diff",
            "--at",
            selector,
            "--json",
        ],
        None,
    )?;
    assert_success(&fresh);
    let reused = run(
        root,
        &[
            "context",
            "--root",
            ".",
            "--from",
            "accepted.json",
            "--at",
            selector,
            "--json",
        ],
        None,
    )?;
    assert_success(&reused);
    context_reuse_matches_fresh(&fresh.stdout, &reused.stdout)?;

    // Only the exact '-' sentinel is stdin. An explicit path to a literal
    // file named '-' remains a supported named-file artifact source.
    write(root, "-", patch)?;
    let literal_path = "./-".to_string();
    let literal = run(
        root,
        &[
            "check",
            "--root",
            ".",
            "--diff",
            &literal_path,
            "--json",
            "--write-artifact",
            "literal.json",
        ],
        None,
    )?;
    assert_success(&literal);
    let literal_parsed: serde_json::Value = serde_json::from_slice(&literal.stdout)
        .map_err(|error| format!("parse literal-file check: {error}"))?;
    assert_eq!(parsed, literal_parsed);
    // --from treats --diff as an assertion. The exact stdin sentinel must
    // never be silently accepted as the literal file above.
    for command in ["explain", "context"] {
        let mut args = vec![
            command,
            "--root",
            ".",
            "--from",
            "literal.json",
            "--diff",
            "-",
        ];
        if command == "context" {
            args.push("--at");
        }
        args.push(selector);
        let refused = run(root, &args, None)?;
        let stderr = String::from_utf8_lossy(&refused.stderr);
        assert_eq!(refused.status.code(), Some(2), "{stderr}");
        assert!(refused.stdout.is_empty());
        assert!(
            stderr.contains("--from cannot be combined with --diff -"),
            "{stderr}"
        );
        assert!(stderr.contains("pass --diff <path>"), "{stderr}");
        let named_args = args
            .iter()
            .map(|arg| {
                if *arg == "-" {
                    literal_path.as_str()
                } else {
                    *arg
                }
            })
            .collect::<Vec<_>>();
        assert_success(&run(root, &named_args, None)?);
    }
    Ok(())
}

#[cfg(feature = "lang-python")]
#[test]
fn python_stdin_artifact_refusal_preserves_named_recovery() -> Result<(), String> {
    let root = unique_temp_workspace("python-stdin-artifact");
    let result = (|| {
        std::fs::create_dir_all(root.join("src"))
            .and_then(|()| std::fs::create_dir_all(root.join("tests")))
            .map_err(|error| format!("create Python fixture: {error}"))?;
        write(
            &root,
            "ripr.toml",
            "[languages]\nenabled = [\"rust\", \"python\"]\n",
        )?;
        write(
            &root,
            "src/discount.py",
            "def discount(amount):\n    if amount >= 100:\n        return 10\n    return 0\n",
        )?;
        write(
            &root,
            "tests/test_discount.py",
            "from src.discount import discount\n\ndef test_below():\n    assert discount(99) == 0\n\ndef test_above():\n    assert discount(101) == 10\n",
        )?;
        exercise(&root, PYTHON_PATCH, PYTHON_COMMENT, "src/discount.py:2")
    })();
    ignore_remove_dir_all(&root);
    result
}

#[cfg(feature = "lang-typescript")]
#[test]
fn javascript_stdin_artifact_refusal_preserves_named_recovery() -> Result<(), String> {
    let root = unique_temp_workspace("javascript-stdin-artifact");
    let result = (|| {
        std::fs::create_dir_all(root.join("src"))
            .and_then(|()| std::fs::create_dir_all(root.join("tests")))
            .map_err(|error| format!("create JavaScript fixture: {error}"))?;
        write(
            &root,
            "ripr.toml",
            "[languages]\nenabled = [\"rust\", \"typescript\"]\n",
        )?;
        write(
            &root,
            "package.json",
            "{\"name\":\"artifact-recovery\",\"type\":\"module\",\"private\":true}\n",
        )?;
        write(
            &root,
            "src/discount.js",
            "export function discount(amount) {\n  return amount >= 100;\n}\n",
        )?;
        write(
            &root,
            "tests/discount.test.js",
            "import { expect, test } from \"vitest\";\nimport { discount } from \"../src/discount.js\";\ntest(\"below\", () => { expect(discount(99)).toBe(false); });\ntest(\"above\", () => { expect(discount(101)).toBe(true); });\n",
        )?;
        exercise(&root, JAVASCRIPT_PATCH, "", "src/discount.js:2")
    })();
    ignore_remove_dir_all(&root);
    result
}
