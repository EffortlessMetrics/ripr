//! Same-file inherited-macro witness for RIPR #1714 tuple-arm observation.
//!
//! A physical child module can contain both the changed owner and its test
//! while inheriting a parent `macro_rules! assert_eq`. Current source-role
//! facts intentionally do not compose an out-of-line child declared beneath
//! an inline module, so `owner.file == test.file` and empty provenance cannot
//! establish the canonical assertion macro.

use ripr::{
    CheckInput, CheckOutput, ExposureClass, Mode, OutputFormat, ProbeFamily, check_workspace,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const ROOT_SOURCE: &str = r#"#[cfg(test)]
mod outer {
    macro_rules! assert_eq { ($($ignored:tt)*) => {{}}; }
    mod subject;
}
"#;

const CHILD_SOURCE: &str = r#"pub fn relation(request_match: bool, task_match: bool) -> &'static str {
    match (request_match, task_match) {
        (true, true) => "request_and_task_identity",
        (true, false) => "request_identity_v2",
        (false, true) => "task_identity",
        (false, false) => "none",
    }
}

#[test]
fn exact_request_only_tuple_is_observed() {
    assert_eq!(relation(true, false), "request_identity_v2");
}
"#;

const DIFF: &str = r#"diff --git a/src/outer/subject.rs b/src/outer/subject.rs
index 1111111..2222222 100644
--- a/src/outer/subject.rs
+++ b/src/outer/subject.rs
@@ -1,8 +1,8 @@
 pub fn relation(request_match: bool, task_match: bool) -> &'static str {
     match (request_match, task_match) {
         (true, true) => "request_and_task_identity",
-        (true, false) => "request_identity_v1",
+        (true, false) => "request_identity_v2",
         (false, true) => "task_identity",
         (false, false) => "none",
     }
 }
"#;

struct TempRepo {
    root: PathBuf,
}

impl TempRepo {
    fn create() -> Result<Self, String> {
        // A process-wide sequence, not the clock: parallel tests on Windows
        // read the same `SystemTime`, share one root, and overwrite each
        // other's fixture.
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "ripr-match-arm-same-file-namespace-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(root.join("src/outer"))
            .map_err(|error| format!("create nested source directory failed: {error}"))?;
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"match-arm-same-file-namespace\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[workspace]\n",
        )
        .map_err(|error| format!("write Cargo.toml failed: {error}"))?;
        std::fs::write(root.join("src/lib.rs"), ROOT_SOURCE)
            .map_err(|error| format!("write root source failed: {error}"))?;
        std::fs::write(root.join("src/outer/subject.rs"), CHILD_SOURCE)
            .map_err(|error| format!("write child source failed: {error}"))?;
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

fn normalized(text: &str) -> String {
    text.trim()
        .trim_end_matches(',')
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn changed_request_arm(output: &CheckOutput) -> Result<&ripr::Finding, String> {
    let expected_full = normalized("(true, false) => \"request_identity_v2\",");
    let expected_boundary = normalized("(true, false) =>");
    let matches = output
        .findings
        .iter()
        .filter(|finding| {
            let expression = normalized(&finding.probe.expression);
            finding.probe.family == ProbeFamily::MatchArm
                && finding.probe.location.line == 4
                && (expression == expected_full || expression == expected_boundary)
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [finding] => Ok(*finding),
        _ => Err(format!(
            "expected one changed request-only arm, found {}: {matches:#?}",
            matches.len()
        )),
    }
}

#[test]
fn same_child_owner_and_test_cannot_certify_an_inherited_assertion_macro() -> Result<(), String> {
    let repo = TempRepo::create()?;
    let output = repo.check()?;
    let finding = changed_request_arm(&output)?;

    assert!(finding.related_tests.iter().any(|test| {
        test.file.ends_with("src/outer/subject.rs")
            && test.name == "exact_request_only_tuple_is_observed"
            && test.oracle.as_deref().is_some_and(|oracle| {
                oracle.contains("relation(true, false)")
                    && oracle.contains("\"request_identity_v2\"")
            })
    }));
    assert_eq!(
        finding.class,
        ExposureClass::WeaklyExposed,
        "a same-file child parse cannot prove which inherited assert_eq! macro runs: probe={:#?}; stages={:#?}; related={:#?}",
        finding.probe,
        finding.ripr,
        finding.related_tests
    );
    assert!(
        finding
            .ripr
            .reveal
            .discriminate
            .summary
            .contains("observation_unverified")
    );
    Ok(())
}
