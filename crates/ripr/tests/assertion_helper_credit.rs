//! Public-API controls for same-file assertion-helper credit (#4574).
//!
//! The helper's owner call and assertion are credited to the test that
//! calls it, but the caller's arguments are not: a helper that ignores them
//! does not see the caller's boundary value, and a helper asserting on
//! something else reads the same as that assertion written inline.

use ripr::{CheckInput, ExposureClass, Finding, Mode, OutputFormat, ProbeFamily, check_workspace};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const GATE: &str = "pub fn gate(input: u32) -> bool {\n    input >= 10\n}\n";

const DIFF: &str = "diff --git a/src/lib.rs b/src/lib.rs
index 1111111..2222222 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,3 +1,3 @@
 pub fn gate(input: u32) -> bool {
-    input > 10
+    input >= 10
 }
";

struct TempRepo {
    root: PathBuf,
}

impl TempRepo {
    fn create(tests: &str) -> Result<Self, String> {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "ripr-assertion-helper-credit-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src"))
            .map_err(|error| format!("create source directory failed: {error}"))?;
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"assertion-helper-credit\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[workspace]\n",
        )
        .map_err(|error| format!("write Cargo.toml failed: {error}"))?;
        std::fs::write(root.join("src/lib.rs"), format!("{GATE}{tests}"))
            .map_err(|error| format!("write source failed: {error}"))?;
        std::fs::write(root.join("diff.patch"), DIFF)
            .map_err(|error| format!("write diff failed: {error}"))?;
        Ok(Self { root })
    }

    /// The finding for the changed `input >= 10` predicate.
    fn predicate(&self) -> Result<Finding, String> {
        let output = check_workspace(CheckInput {
            root: self.root.clone(),
            base: None,
            diff_file: Some(self.root.join("diff.patch")),
            mode: Mode::Ready,
            format: OutputFormat::Json,
            include_unchanged_tests: true,
            ..CheckInput::default()
        })?;
        let mut predicates = output.findings.into_iter().filter(|finding| {
            finding.probe.family == ProbeFamily::Predicate && finding.probe.location.line == 2
        });
        match (predicates.next(), predicates.next()) {
            (Some(finding), None) => Ok(finding),
            other => Err(format!(
                "premise: exactly one predicate finding on line 2, got {other:?}"
            )),
        }
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn state(stage: &impl std::fmt::Debug) -> String {
    format!("{stage:?}")
}

fn tests_module(helper: &str, calls: &str) -> String {
    format!(
        "\n#[cfg(test)]\nmod tests {{\n    use super::*;\n\n{helper}\n    #[test]\n    fn boundary() {{\n{calls}    }}\n}}\n"
    )
}

#[test]
fn helper_ignoring_its_arguments_does_not_take_the_callers_boundary() -> Result<(), String> {
    let calls = "        check(10, true);\n        check(9, false);\n";
    // Positive control: the helper forwards the caller's `10` to `gate`.
    let forwarding = TempRepo::create(&tests_module(
        "    fn check(input: u32, expected: bool) {\n        assert!(gate(input) == expected);\n    }\n",
        calls,
    ))?
    .predicate()?;
    assert_eq!(state(&forwarding.ripr.reach.state), "Yes", "{forwarding:?}");
    assert_eq!(
        state(&forwarding.ripr.infect.state),
        "Yes",
        "premise: the forwarded boundary value is seen: {forwarding:?}"
    );

    // Same calls, but only the helper's own `gate(0)` runs.
    let ignoring = TempRepo::create(&tests_module(
        "    fn check(_input: u32, _expected: bool) {\n        assert!(gate(0) == false);\n    }\n",
        calls,
    ))?
    .predicate()?;
    assert_eq!(state(&ignoring.ripr.reach.state), "Yes", "{ignoring:?}");
    assert_ne!(state(&ignoring.ripr.infect.state), "Yes", "{ignoring:?}");
    assert_ne!(ignoring.class, ExposureClass::Exposed, "{ignoring:?}");
    Ok(())
}

#[test]
fn helper_asserting_on_something_else_reads_as_the_inline_assertion() -> Result<(), String> {
    // The helper calls `gate` but never observes its result.
    let helper = TempRepo::create(&tests_module(
        "    fn check(input: u32) {\n        let _ = gate(input);\n        assert!(input < 100);\n    }\n",
        "        check(10);\n",
    ))?
    .predicate()?;
    let inline = TempRepo::create(&tests_module(
        "",
        "        let input = 10;\n        let _ = gate(input);\n        assert!(input < 100);\n",
    ))?
    .predicate()?;

    assert_ne!(helper.class, ExposureClass::Exposed, "{helper:?}");
    assert_eq!(helper.class, inline.class, "{helper:?}\n{inline:?}");
    Ok(())
}
