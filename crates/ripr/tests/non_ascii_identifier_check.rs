//! End-to-end witness for the non-ASCII whole-word search panic.
//!
//! `ripr check` panicked with "byte index is not a char boundary" when a
//! changed function used a non-ASCII identifier and a test held a longer
//! identifier ending in it: the rejected first match advanced one byte into
//! the multibyte character. This drives the public check path, not the
//! search helpers alone.

use ripr::{CheckInput, CheckOutput, Mode, OutputFormat, check_workspace};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const SOURCE: &str = "pub fn total(заказ: u32) -> u32 {\n    заказ + 1\n}\n";

const TEST: &str = r#"use non_ascii_identifier_check::total;

#[test]
fn total_works() {
    let new_заказ = 2;
    assert_eq!(total(new_заказ), 3);
}
"#;

const DIFF: &str = "diff --git a/src/lib.rs b/src/lib.rs
index 1111111..2222222 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,3 +1,3 @@
 pub fn total(заказ: u32) -> u32 {
-    заказ + 2
+    заказ + 1
 }
";

struct TempRepo {
    root: PathBuf,
}

impl TempRepo {
    fn create() -> Result<Self, String> {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "ripr-non-ascii-identifier-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(root.join("src"))
            .map_err(|error| format!("create source directory failed: {error}"))?;
        std::fs::create_dir_all(root.join("tests"))
            .map_err(|error| format!("create test directory failed: {error}"))?;
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"non-ascii-identifier-check\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[workspace]\n",
        )
        .map_err(|error| format!("write Cargo.toml failed: {error}"))?;
        std::fs::write(root.join("src/lib.rs"), SOURCE)
            .map_err(|error| format!("write source failed: {error}"))?;
        std::fs::write(root.join("tests/total.rs"), TEST)
            .map_err(|error| format!("write test failed: {error}"))?;
        std::fs::write(root.join("diff.patch"), DIFF)
            .map_err(|error| format!("write diff failed: {error}"))?;
        Ok(Self { root })
    }

    fn check(&self) -> Result<CheckOutput, String> {
        check_workspace(CheckInput {
            root: self.root.clone(),
            base: None,
            diff_file: Some(self.root.join("diff.patch")),
            mode: Mode::Ready,
            format: OutputFormat::Json,
            include_unchanged_tests: true,
            ..CheckInput::default()
        })
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn check_on_non_ascii_identifier_completes_with_the_changed_probe() -> Result<(), String> {
    let repo = TempRepo::create()?;
    let output = repo.check()?;

    // The subject must be the changed body line, related to the test that
    // names the longer identifier; a vacuous empty run would not prove the
    // search loops ran over the non-ASCII text.
    let finding = output
        .findings
        .iter()
        .find(|finding| {
            finding.probe.location.line == 2 && finding.probe.expression.contains("заказ + 1")
        })
        .ok_or_else(|| {
            format!(
                "missing changed probe at src/lib.rs:2; observed {:?}",
                output
                    .findings
                    .iter()
                    .map(|finding| (finding.probe.location.line, &finding.probe.expression))
                    .collect::<Vec<_>>()
            )
        })?;
    assert!(
        finding
            .related_tests
            .iter()
            .any(|test| test.name == "total_works"),
        "related tests: {:?}",
        finding
            .related_tests
            .iter()
            .map(|test| &test.name)
            .collect::<Vec<_>>()
    );
    Ok(())
}
