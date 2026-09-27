//! Public-API witnesses for RIPR #1714 tuple-pattern identity.
//!
//! Select the exact candidate-side arm, not a removed-side probe sharing its
//! pattern and line. Keep unsupported input mappings explicitly unverified.
//! These tests execute the analyzer; they never generate replacement source.

use ripr::{
    CheckInput, CheckOutput, ExposureClass, Mode, OutputFormat, ProbeFamily, check_workspace,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const CANDIDATE_SOURCE: &str = r#"pub fn relation(request_match: bool, task_match: bool) -> &'static str {
    match (request_match, task_match) {
        (true, true) => "request_and_task_identity",
        (true, false) => "request_identity_v2",
        (false, true) => "task_identity",
        (false, false) => "none",
    }
}
"#;

const DIFF: &str = r#"diff --git a/src/lib.rs b/src/lib.rs
index 1111111..2222222 100644
--- a/src/lib.rs
+++ b/src/lib.rs
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

const ALIGNED_TEST: &str = r#"use match_arm_tuple_identity::relation;

#[test]
fn exact_request_only_tuple_is_observed() {
    assert_eq!(relation(true, false), "request_identity_v2");
}
"#;

const SIBLING_TEST: &str = r#"use match_arm_tuple_identity::relation;

#[test]
fn task_only_sibling_tuple_is_observed() {
    assert_eq!(relation(false, true), "task_identity");
}
"#;

struct TempRepo {
    root: PathBuf,
}

impl TempRepo {
    fn create(test_source: &str) -> Result<Self, String> {
        // A process-wide sequence, not the clock: parallel tests on Windows
        // read the same `SystemTime`, share one root, and overwrite each
        // other's fixture.
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "ripr-match-arm-tuple-identity-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(root.join("src"))
            .map_err(|error| format!("create source directory failed: {error}"))?;
        std::fs::create_dir_all(root.join("tests"))
            .map_err(|error| format!("create test directory failed: {error}"))?;
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"match-arm-tuple-identity\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[workspace]\n",
        )
        .map_err(|error| format!("write Cargo.toml failed: {error}"))?;
        std::fs::write(root.join("src/lib.rs"), CANDIDATE_SOURCE)
            .map_err(|error| format!("write source failed: {error}"))?;
        std::fs::write(root.join("tests/relation.rs"), test_source)
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

/// Fixture-local comparison: these literal values contain no whitespace.
fn normalized_arm(text: &str) -> String {
    text.trim()
        .trim_end_matches(',')
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

/// Candidate currentness is carried by `after`, not a shared pattern prefix.
/// Do not filter by classification: a wrong class must reach the test oracle.
fn changed_request_only_arm(output: &CheckOutput) -> Result<&ripr::Finding, String> {
    let expected_full = normalized_arm("(true, false) => \"request_identity_v2\",");
    let expected_boundary = normalized_arm("(true, false) =>");
    let expected_before = normalized_arm("(true, false) => \"request_identity_v1\",");
    let is_current_claim = |text: &str| {
        let text = normalized_arm(text);
        text == expected_full || text == expected_boundary
    };
    let matches = output
        .findings
        .iter()
        .filter(|finding| {
            finding.probe.family == ProbeFamily::MatchArm
                && finding.probe.location.line == 4
                && is_current_claim(&finding.probe.expression)
                && finding
                    .probe
                    .after
                    .as_deref()
                    .is_some_and(&is_current_claim)
                && finding
                    .probe
                    .before
                    .as_deref()
                    .is_none_or(|before| normalized_arm(before) == expected_before)
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [finding] => Ok(*finding),
        _ => {
            let observed = output
                .findings
                .iter()
                .filter(|finding| finding.probe.family == ProbeFamily::MatchArm)
                .map(|finding| {
                    format!(
                        "id={:?}; line={}; before={:?}; after={:?}; expression={:?}; class={:?}",
                        finding.probe.id,
                        finding.probe.location.line,
                        finding.probe.before,
                        finding.probe.after,
                        finding.probe.expression,
                        finding.class
                    )
                })
                .collect::<Vec<_>>()
                .join(" | ");
            Err(format!(
                "expected exactly one candidate-current request arm at line 4, found {}; all match subjects: {observed}",
                matches.len()
            ))
        }
    }
}

#[test]
fn exact_tuple_input_and_result_certify_the_same_match_arm() -> Result<(), String> {
    let repo = TempRepo::create(ALIGNED_TEST)?;
    let output = repo.check()?;
    let finding = changed_request_only_arm(&output)?;

    assert_eq!(
        finding.class,
        ExposureClass::Exposed,
        "an exact `(true, false)` owner input and exact result must certify only that parser-owned arm; probe={:#?}; stages={:#?}; related={:#?}",
        finding.probe,
        finding.ripr,
        finding.related_tests
    );
    assert!(finding.related_tests.iter().any(|test| {
        test.name == "exact_request_only_tuple_is_observed"
            && test.oracle.as_deref().is_some_and(|oracle| {
                oracle.contains("relation(true, false)")
                    && oracle.contains("\"request_identity_v2\"")
            })
    }));
    Ok(())
}

#[test]
fn sibling_tuple_input_cannot_certify_the_changed_arm() -> Result<(), String> {
    let repo = TempRepo::create(SIBLING_TEST)?;
    let output = repo.check()?;
    let finding = changed_request_only_arm(&output)?;

    assert_eq!(
        finding.class,
        ExposureClass::WeaklyExposed,
        "the `(false, true)` sibling must not certify the changed `(true, false)` arm"
    );
    assert!(
        finding
            .ripr
            .reveal
            .discriminate
            .summary
            .contains("observation_unverified"),
        "the sibling-only witness must retain the explicit unverified discriminator"
    );
    assert!(finding.related_tests.iter().any(|test| {
        test.name == "task_only_sibling_tuple_is_observed"
            && test.oracle.as_deref().is_some_and(|oracle| {
                oracle.contains("relation(false, true)") && oracle.contains("\"task_identity\"")
            })
    }));
    Ok(())
}

#[test]
fn concurrent_fixtures_get_distinct_roots() -> Result<(), String> {
    let first = TempRepo::create(ALIGNED_TEST)?;
    let second = TempRepo::create(SIBLING_TEST)?;
    assert_ne!(first.root, second.root);
    Ok(())
}

#[test]
fn fixture_paths_remain_inside_the_ephemeral_root() -> Result<(), String> {
    let repo = TempRepo::create(ALIGNED_TEST)?;
    for relative in [
        "Cargo.toml",
        "src/lib.rs",
        "tests/relation.rs",
        "diff.patch",
    ] {
        let path = repo.root.join(relative);
        if !path.starts_with(&repo.root) || !path.is_file() {
            return Err(format!(
                "fixture path escaped or is missing: {}",
                path.display()
            ));
        }
    }
    Ok(())
}

/// Keep argument spelling distinct from the value selected by the match.
fn check_non_identity_scrutinee(scrutinee: &str, expected_result: &str) -> Result<(), String> {
    let original = "match (request_match, task_match) {";
    let replacement = format!("match {scrutinee} {{");
    assert_eq!(CANDIDATE_SOURCE.matches(original).count(), 1);
    assert_eq!(DIFF.matches(original).count(), 1);
    let test_source = ALIGNED_TEST.replace("\"request_identity_v2\"", expected_result);
    let repo = TempRepo::create(&test_source)?;
    std::fs::write(
        repo.root.join("src/lib.rs"),
        CANDIDATE_SOURCE.replace(original, &replacement),
    )
    .map_err(|error| format!("write non-identity source failed: {error}"))?;
    std::fs::write(
        repo.root.join("diff.patch"),
        DIFF.replace(original, &replacement),
    )
    .map_err(|error| format!("write non-identity diff failed: {error}"))?;

    let output = repo.check()?;
    let finding = changed_request_only_arm(&output)?;
    assert!(finding.related_tests.iter().any(|test| {
        test.name == "exact_request_only_tuple_is_observed"
            && test.oracle.as_deref().is_some_and(|oracle| {
                oracle.contains("relation(true, false)") && oracle.contains(expected_result)
            })
    }));
    assert_eq!(
        finding.class,
        ExposureClass::WeaklyExposed,
        "call arguments are not the matched tuple for {scrutinee}: {:?}",
        finding.ripr.reveal.discriminate
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

#[test]
fn reordered_scrutinee_does_not_borrow_argument_position() -> Result<(), String> {
    check_non_identity_scrutinee("(task_match, request_match)", "\"task_identity\"")
}

#[test]
fn transformed_scrutinee_does_not_borrow_argument_values() -> Result<(), String> {
    check_non_identity_scrutinee("(!request_match, task_match)", "\"none\"")
}

#[test]
fn constant_scrutinee_does_not_borrow_unused_argument_values() -> Result<(), String> {
    check_non_identity_scrutinee("(false, true)", "\"task_identity\"")
}

/// A physical module file does not establish the macro namespace it inherits.
#[test]
fn inherited_assertion_namespace_cannot_certify_the_tuple_arm() -> Result<(), String> {
    for prefix in [
        "",
        "#[cfg(test)]\nmacro_rules! assert_eq { ($($ignored:tt)*) => {{}}; }\n",
    ] {
        let repo = TempRepo::create(ALIGNED_TEST)?;
        std::fs::remove_file(repo.root.join("tests/relation.rs"))
            .map_err(|error| format!("remove standalone test failed: {error}"))?;
        let source = format!("{CANDIDATE_SOURCE}{prefix}#[cfg(test)]\nmod tuple_tests;\n");
        std::fs::write(repo.root.join("src/lib.rs"), source)
            .map_err(|error| format!("write parent test namespace failed: {error}"))?;
        std::fs::write(
            repo.root.join("src/tuple_tests.rs"),
            ALIGNED_TEST.replace(
                "use match_arm_tuple_identity::relation;",
                "use crate::relation;",
            ),
        )
        .map_err(|error| format!("write child test module failed: {error}"))?;

        let output = repo.check()?;
        let finding = changed_request_only_arm(&output)?;
        assert_eq!(
            finding.class,
            ExposureClass::WeaklyExposed,
            "a child-file parse cannot establish its inherited assertion namespace: {prefix:?}"
        );
        assert!(
            finding
                .ripr
                .reveal
                .discriminate
                .summary
                .contains("observation_unverified")
        );
        assert!(finding.related_tests.iter().any(|test| {
            test.file.ends_with("src/tuple_tests.rs")
                && test.name == "exact_request_only_tuple_is_observed"
                && test.oracle.as_deref().is_some_and(|oracle| {
                    oracle.contains("relation(true, false)")
                        && oracle.contains("\"request_identity_v2\"")
                })
        }));
    }
    Ok(())
}

/// A new file has no removed-side context to pair with its current arms.
#[test]
fn added_only_tuple_arm_requires_the_matching_input_and_result() -> Result<(), String> {
    let added = CANDIDATE_SOURCE
        .lines()
        .map(|line| format!("+{line}"))
        .collect::<Vec<_>>()
        .join("\n");
    let line_count = CANDIDATE_SOURCE.lines().count();
    let diff = format!(
        "diff --git a/src/lib.rs b/src/lib.rs\nnew file mode 100644\n--- /dev/null\n+++ b/src/lib.rs\n@@ -0,0 +1,{line_count} @@\n{added}\n"
    );
    for (test_source, expected) in [
        (ALIGNED_TEST, ExposureClass::Exposed),
        (SIBLING_TEST, ExposureClass::WeaklyExposed),
    ] {
        let repo = TempRepo::create(test_source)?;
        std::fs::write(repo.root.join("diff.patch"), &diff)
            .map_err(|error| format!("write added-only diff failed: {error}"))?;
        let output = repo.check()?;
        let finding = changed_request_only_arm(&output)?;
        assert!(
            finding.probe.before.is_none(),
            "an added-only probe must not borrow an old value: {:?}",
            finding.probe
        );
        assert_eq!(
            finding.class, expected,
            "added-only arm observation: probe={:?}; stages={:?}",
            finding.probe, finding.ripr
        );
        assert!(!finding.related_tests.is_empty());
        if expected == ExposureClass::WeaklyExposed {
            assert!(
                finding
                    .ripr
                    .reveal
                    .discriminate
                    .summary
                    .contains("observation_unverified")
            );
        }
    }
    Ok(())
}
