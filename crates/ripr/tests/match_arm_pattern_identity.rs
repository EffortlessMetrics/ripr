//! End-to-end witness for RIPR #1714.
//!
//! Retain the original literal-arm witness and adversarial cases for
//! observation that belongs to a sibling, diagnostic, unselected input, or
//! guarded arm. Every classification assertion selects one exact changed
//! arm; the enclosing match and duplicate subjects cannot stand in for it.

use ripr::{
    CheckInput, CheckOutput, ExposureClass, Mode, OutputFormat, ProbeFamily, check_workspace,
};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const CANDIDATE_SOURCE: &str = r#"pub fn route(kind: &str) -> &'static str {
    match kind {
        "sensor" => "sensor-v2",
        "focused-test" => "proof",
        _ => "other",
    }
}
"#;

const ALIGNED_TEST: &str = r#"use match_arm_pattern_identity::route;

#[test]
fn exact_sensor_arm_is_observed() {
    assert_eq!(route("sensor"), "sensor-v2");
}
"#;

const SIBLING_TEST: &str = r#"use match_arm_pattern_identity::route;

#[test]
fn sibling_proof_arm_is_observed() {
    assert_eq!(route("focused-test"), "proof");
}
"#;

const DIFF: &str = r#"diff --git a/src/lib.rs b/src/lib.rs
index 1111111..2222222 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,7 +1,7 @@
 pub fn route(kind: &str) -> &'static str {
     match kind {
-        "sensor" => "sensor-v1",
+        "sensor" => "sensor-v2",
         "focused-test" => "proof",
         _ => "other",
     }
 }
"#;

struct TempRepo {
    root: PathBuf,
}

impl TempRepo {
    fn create(test_source: &str) -> Result<Self, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("clock before Unix epoch: {error}"))?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "ripr-match-arm-pattern-identity-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(root.join("src"))
            .map_err(|error| format!("create source directory failed: {error}"))?;
        std::fs::create_dir_all(root.join("tests"))
            .map_err(|error| format!("create test directory failed: {error}"))?;
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"match-arm-pattern-identity\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[workspace]\n",
        )
        .map_err(|error| format!("write Cargo.toml failed: {error}"))?;
        std::fs::write(root.join("src/lib.rs"), CANDIDATE_SOURCE)
            .map_err(|error| format!("write source failed: {error}"))?;
        std::fs::write(root.join("tests/route.rs"), test_source)
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

fn changed_sensor_arm(output: &CheckOutput) -> Result<&ripr::Finding, String> {
    let mut matches = output.findings.iter().filter(|finding| {
        finding.probe.family == ProbeFamily::MatchArm
            && finding.probe.location.line == 3
            && (finding.probe.expression.starts_with("\"sensor\" =>")
                || finding.probe.expression.starts_with("\"sensor\" if"))
    });
    let finding = matches.next().ok_or_else(|| {
        let observed = output
            .findings
            .iter()
            .map(|finding| {
                format!(
                    "{}:{}:{}",
                    finding.probe.family.as_str(),
                    finding.probe.location.line,
                    finding.probe.expression
                )
            })
            .collect::<Vec<_>>()
            .join(" | ");
        format!("missing changed sensor arm at line 3; observed {observed}")
    })?;
    if matches.next().is_some() {
        return Err("duplicate changed sensor match-arm findings at line 3".to_string());
    }
    Ok(finding)
}

#[test]
fn exact_literal_input_and_result_certify_the_same_match_arm() -> Result<(), String> {
    let repo = TempRepo::create(ALIGNED_TEST)?;
    let output = repo.check()?;
    let finding = changed_sensor_arm(&output)?;

    assert_eq!(
        finding.class,
        ExposureClass::Exposed,
        "an exact call input and exact result assertion must certify the parser-owned literal arm; discriminator={:?}",
        finding.ripr.reveal.discriminate
    );
    assert!(finding.related_tests.iter().any(|test| {
        test.name == "exact_sensor_arm_is_observed"
            && test.oracle.as_deref().is_some_and(|oracle| {
                oracle.contains("route(\"sensor\")") && oracle.contains("\"sensor-v2\"")
            })
    }));
    Ok(())
}

#[test]
fn a_sibling_literal_arm_oracle_cannot_certify_the_changed_arm() -> Result<(), String> {
    let repo = TempRepo::create(SIBLING_TEST)?;
    let output = repo.check()?;
    let finding = changed_sensor_arm(&output)?;

    assert_eq!(
        finding.class,
        ExposureClass::WeaklyExposed,
        "a focused-test assertion must not certify the changed sensor arm"
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
        test.name == "sibling_proof_arm_is_observed"
            && test.oracle.as_deref().is_some_and(|oracle| {
                oracle.contains("route(\"focused-test\")") && oracle.contains("\"proof\"")
            })
    }));
    Ok(())
}

#[test]
fn fixture_paths_remain_inside_the_ephemeral_root() -> Result<(), String> {
    let repo = TempRepo::create(ALIGNED_TEST)?;
    for relative in ["Cargo.toml", "src/lib.rs", "tests/route.rs", "diff.patch"] {
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

#[test]
fn shared_result_literal_does_not_observe_a_sibling_arm() -> Result<(), String> {
    let test_source = SIBLING_TEST.replace("\"proof\"", "\"sensor-v2\"");
    let repo = TempRepo::create(&test_source)?;
    let sibling = "\"focused-test\" => \"proof\"";
    let shared_result = "\"focused-test\" => \"sensor-v2\"";
    std::fs::write(
        repo.root.join("src/lib.rs"),
        CANDIDATE_SOURCE.replace(sibling, shared_result),
    )
    .map_err(|error| format!("write shared-result source failed: {error}"))?;
    std::fs::write(
        repo.root.join("diff.patch"),
        DIFF.replace(sibling, shared_result),
    )
    .map_err(|error| format!("write shared-result diff failed: {error}"))?;

    let output = repo.check()?;
    let finding = changed_sensor_arm(&output)?;
    assert_eq!(finding.probe.location.line, 3);
    assert!(finding.probe.expression.starts_with("\"sensor\" =>"));
    assert_eq!(
        finding.class,
        ExposureClass::WeaklyExposed,
        "a sibling call returning the same literal never selects the changed sensor arm"
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
fn diagnostic_literal_does_not_select_the_changed_arm() -> Result<(), String> {
    let test_source = r#"use match_arm_pattern_identity::route;

#[test]
fn sibling_assertion_mentions_sensor_only_in_its_message() {
    assert_eq!(route("focused-test"), "proof", "sensor");
}
"#;
    let repo = TempRepo::create(test_source)?;
    let output = repo.check()?;
    let finding = changed_sensor_arm(&output)?;
    assert_eq!(finding.probe.location.line, 3);
    assert!(finding.probe.expression.starts_with("\"sensor\" =>"));
    assert_eq!(
        finding.class,
        ExposureClass::WeaklyExposed,
        "assertion diagnostic text is not a changed-arm input or observation"
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
fn diagnostic_owner_call_does_not_observe_the_changed_arm() -> Result<(), String> {
    let test_source = r#"use match_arm_pattern_identity::route;

#[test]
fn only_the_sibling_result_is_compared() {
    assert_eq!(route("focused-test"), "proof", "changed arm: {}", route("sensor"));
}
"#;
    let repo = TempRepo::create(test_source)?;
    let output = repo.check()?;
    let finding = changed_sensor_arm(&output)?;
    assert_eq!(finding.probe.location.line, 3);
    assert!(finding.probe.expression.starts_with("\"sensor\" =>"));
    assert!(finding.related_tests.iter().any(|test| {
        test.name == "only_the_sibling_result_is_compared"
            && test
                .oracle
                .as_deref()
                .is_some_and(|oracle| oracle.contains("route(\"focused-test\")"))
    }));
    assert_eq!(
        finding.class,
        ExposureClass::WeaklyExposed,
        "an owner call in diagnostic arguments has no observed result"
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
fn unselected_literal_inside_an_argument_is_not_the_owner_input() -> Result<(), String> {
    let test_source = r#"use match_arm_pattern_identity::route;

#[test]
fn conditional_input_selects_the_sibling() {
    assert_eq!(route(if false { "sensor" } else { "focused-test" }), "proof");
}
"#;
    let repo = TempRepo::create(test_source)?;
    let output = repo.check()?;
    let finding = changed_sensor_arm(&output)?;
    assert_eq!(finding.probe.location.line, 3);
    assert!(finding.probe.expression.starts_with("\"sensor\" =>"));
    assert_eq!(
        finding.class,
        ExposureClass::WeaklyExposed,
        "a literal in the unselected branch is not the value passed to route"
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
fn a_matching_literal_does_not_bypass_the_changed_arm_guard() -> Result<(), String> {
    let test_source = SIBLING_TEST.replace("route(\"focused-test\")", "route(\"sensor\")");
    let repo = TempRepo::create(&test_source)?;
    let pattern = "\"sensor\" =>";
    let guarded = "\"sensor\" if kind.len() > 10 =>";
    let sibling = "\"focused-test\" => \"proof\"";
    let fallback = "\"sensor\" => \"proof\"";
    std::fs::write(
        repo.root.join("src/lib.rs"),
        CANDIDATE_SOURCE
            .replace(pattern, guarded)
            .replace(sibling, fallback),
    )
    .map_err(|error| format!("write guarded source failed: {error}"))?;
    std::fs::write(
        repo.root.join("diff.patch"),
        DIFF.replace(pattern, guarded).replace(sibling, fallback),
    )
    .map_err(|error| format!("write guarded diff failed: {error}"))?;

    let output = repo.check()?;
    let finding = changed_sensor_arm(&output)?;
    assert_eq!(finding.probe.location.line, 3);
    assert!(finding.probe.expression.starts_with("\"sensor\" if"));
    assert_eq!(
        finding.class,
        ExposureClass::WeaklyExposed,
        "a false guard sends the matching literal to the unchanged sibling"
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
fn observed_owner_result_with_diagnostic_arguments_remains_exposed() -> Result<(), String> {
    let test_source = r#"use match_arm_pattern_identity::route;

#[test]
fn the_sensor_result_is_compared_with_an_extra_diagnostic() {
    assert_eq!(route("sensor"), "sensor-v2", "sibling: {}", route("focused-test"));
}
"#;
    let repo = TempRepo::create(test_source)?;
    let output = repo.check()?;
    let finding = changed_sensor_arm(&output)?;
    assert_eq!(finding.probe.location.line, 3);
    assert!(finding.probe.expression.starts_with("\"sensor\" =>"));
    assert_eq!(
        finding.class,
        ExposureClass::Exposed,
        "diagnostic arguments must not erase a genuinely observed owner result"
    );
    assert!(finding.related_tests.iter().any(|test| {
        test.name == "the_sensor_result_is_compared_with_an_extra_diagnostic"
            && test.oracle.as_deref().is_some_and(|oracle| {
                oracle.contains("route(\"sensor\")") && oracle.contains("\"sensor-v2\"")
            })
    }));
    Ok(())
}

fn check_literal_syntax_case(
    pattern: &str,
    compared_call: &str,
    expected_result: &str,
    diagnostic: &str,
    expected_class: ExposureClass,
) -> Result<(), String> {
    let assertion = format!("assert_eq!({compared_call}, {expected_result}{diagnostic});");
    let test_source =
        ALIGNED_TEST.replace("assert_eq!(route(\"sensor\"), \"sensor-v2\");", &assertion);
    let repo = TempRepo::create(&test_source)?;
    let prefix = format!("{pattern} =>");
    std::fs::write(
        repo.root.join("src/lib.rs"),
        CANDIDATE_SOURCE.replace("\"sensor\" =>", &prefix),
    )
    .map_err(|error| format!("write literal syntax source failed: {error}"))?;
    std::fs::write(
        repo.root.join("diff.patch"),
        DIFF.replace("\"sensor\" =>", &prefix),
    )
    .map_err(|error| format!("write literal syntax diff failed: {error}"))?;
    let output = repo.check()?;
    let findings = output
        .findings
        .iter()
        .filter(|finding| {
            finding.probe.family == ProbeFamily::MatchArm
                && finding.probe.location.line == 3
                && finding.probe.expression.starts_with(&prefix)
        })
        .collect::<Vec<_>>();
    assert_eq!(findings.len(), 1, "missing or duplicate arm: {pattern}");
    let finding = findings[0];
    assert!(finding.related_tests.iter().any(|test| {
        test.name == "exact_sensor_arm_is_observed"
            && test.oracle.as_deref().is_some_and(|oracle| {
                oracle.contains(compared_call) && oracle.contains(expected_result)
            })
    }));
    assert_eq!(
        finding.class, expected_class,
        "pattern={pattern}; assertion={assertion}; discriminator={:?}",
        finding.ripr.reveal.discriminate
    );
    if expected_class == ExposureClass::WeaklyExposed {
        assert!(
            finding
                .ripr
                .reveal
                .discriminate
                .summary
                .contains("observation_unverified")
        );
    }
    Ok(())
}

#[test]
fn exact_raw_string_inputs_observe_the_literal_arm() -> Result<(), String> {
    for input in ["r\"sensor\"", "r#\"sensor\"#", "r###\"sensor\"###"] {
        check_literal_syntax_case(
            "\"sensor\"",
            &format!("route({input})"),
            "\"sensor-v2\"",
            "",
            ExposureClass::Exposed,
        )?;
    }
    Ok(())
}

#[test]
fn embedded_arrow_is_literal_content_not_an_arm_separator() -> Result<(), String> {
    for pattern in ["\"sensor=>legacy\"", "r#\"sensor=>legacy\"#"] {
        check_literal_syntax_case(
            pattern,
            "route(\"sensor=>legacy\")",
            "\"sensor-v2\"",
            "",
            ExposureClass::Exposed,
        )?;
    }
    Ok(())
}

#[test]
fn raw_pattern_escape_bytes_do_not_borrow_a_cooked_input() -> Result<(), String> {
    check_literal_syntax_case(
        "r\"sensor\\n\"",
        "route(\"sensor\\n\")",
        "\"other\"",
        "",
        ExposureClass::WeaklyExposed,
    )
}

#[test]
fn raw_diagnostic_call_never_supplies_the_compared_input() -> Result<(), String> {
    check_literal_syntax_case(
        "\"sensor\"",
        "route(\"focused-test\")",
        "\"proof\"",
        ", \"{}\", route(r#\"sensor\"#)",
        ExposureClass::WeaklyExposed,
    )
}

#[test]
fn quoted_pattern_comments_do_not_select_the_changed_arm() -> Result<(), String> {
    for pattern in [
        "\"sensor\" /* \"focused-test\" */",
        "\"sensor\" /* outer /* r#\"focused-test\"# */ tail */",
    ] {
        check_literal_syntax_case(
            pattern,
            "route(\"focused-test\")",
            "\"proof\"",
            "",
            ExposureClass::WeaklyExposed,
        )?;
    }
    Ok(())
}

#[test]
fn quoted_pattern_comments_preserve_real_observation() -> Result<(), String> {
    for pattern in [
        "\"sensor\" /* \"focused-test\" */",
        "\"sensor\" /* outer /* r#\"focused-test\"# */ tail */",
    ] {
        check_literal_syntax_case(
            pattern,
            "route(\"sensor\")",
            "\"sensor-v2\"",
            "",
            ExposureClass::Exposed,
        )?;
    }
    Ok(())
}
