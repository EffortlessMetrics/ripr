//! Matched static/runtime controls for RIPR-SPEC-0197 (#4478).
//! Run the same tests against correct and deliberately wrong libraries;
//! compilation success and one executed test precede outcome assertions.

use ripr::{
    CheckInput, ExposureClass, Mode, OutputFormat, ProbeFamily, check_workspace, render_check,
};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

struct Scratch(PathBuf);

impl Scratch {
    fn create() -> Result<Self, String> {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "ripr-owner-pin-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).map_err(|error| error.to_string())?;
        Ok(Self(path))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run(program: &Path, arguments: &[&std::ffi::OsStr]) -> Result<Output, String> {
    Command::new(program)
        .args(arguments)
        .output()
        .map_err(|error| format!("{}: {error}", program.display()))
}

fn compiles(output: Output, phase: &str) -> Result<(), String> {
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{phase} compilation failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

#[test]
fn local_empty_macro_preserves_independent_equality_execution() -> Result<(), String> {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/predicate_oracle_execution_direct");
    let original = std::fs::read_to_string(fixture.join("input/src/lib.rs"))
        .map_err(|error| error.to_string())?;
    let (production, _) = original.split_once("#[cfg(test)]").ok_or("missing tests")?;
    let empty = "macro_rules! discard_tokens { ($($ignored:tt)*) => {} }";
    let returning = "macro_rules! discard_tokens { ($($ignored:tt)*) => { return; } }";
    for (name, definitions, before, admitted, mutant_detected) in [
        (
            "different_name_empty",
            empty.to_string(),
            "".to_string(),
            true,
            true,
        ),
        (
            "empty_discards_return_tokens",
            empty.to_string(),
            "".to_string(),
            true,
            true,
        ),
        (
            "returning",
            returning.to_string(),
            "".to_string(),
            false,
            false,
        ),
        (
            "shadowed",
            empty.to_string(),
            returning.to_string(),
            false,
            false,
        ),
        (
            "ambiguous",
            format!("{empty}\nmod elsewhere {{ {empty} }}"),
            "".to_string(),
            false,
            true,
        ),
        (
            "disabled",
            format!("#[cfg(any())]\n{empty}\n{returning}"),
            "".to_string(),
            false,
            false,
        ),
        (
            "imported",
            format!(
                "mod exports {{ {empty}\npub(crate) use discard_tokens; }}\nuse exports::discard_tokens;"
            ),
            "".to_string(),
            false,
            true,
        ),
        (
            "unproven_matcher",
            "macro_rules! discard_tokens { ($ignored:expr) => {} }".to_string(),
            "".to_string(),
            false,
            true,
        ),
        (
            "opaque_nonempty",
            "macro_rules! discard_tokens { ($($ignored:tt)*) => { let _marker = (); } }"
                .to_string(),
            "".to_string(),
            false,
            true,
        ),
    ] {
        let arguments = if name == "empty_discards_return_tokens" {
            "return;"
        } else {
            "discounted_total(100, 100)"
        };
        let source = format!(
            "{production}\n{definitions}\n#[cfg(test)]\nmod tests {{\nuse super::*;\n#[test]\nfn boundary() {{\n{before}\ndiscard_tokens!({arguments});\nassert_eq!(discounted_total(100, 100), 90);\n}}\n}}\n"
        );
        let scratch = Scratch::create()?;
        std::fs::create_dir(scratch.0.join("src")).map_err(|error| error.to_string())?;
        std::fs::copy(
            fixture.join("input/Cargo.toml"),
            scratch.0.join("Cargo.toml"),
        )
        .map_err(|error| error.to_string())?;
        std::fs::write(scratch.0.join("src/lib.rs"), &source).map_err(|error| error.to_string())?;
        // Establish the real test's value independently of conservative static
        // refusals for opaque, imported or ambiguous macro bindings.
        // Keep compiled variants outside the analyzed project: runtime.rs at
        // its root would add another macro definition and test to RustIndex.
        let runtime_scratch = Scratch::create()?;
        for wrong in [false, true] {
            let runtime_source = runtime_scratch.0.join("runtime.rs");
            assert!(!runtime_source.starts_with(&scratch.0));
            let runtime = runtime_scratch
                .0
                .join(format!("runtime{}", std::env::consts::EXE_SUFFIX));
            let subject = if wrong {
                source.replace(
                    "amount >= discount_threshold",
                    "amount > discount_threshold",
                )
            } else {
                source.clone()
            };
            if wrong {
                assert_ne!(
                    subject, source,
                    "{name}: mutation must change the runtime subject"
                );
            }
            std::fs::write(&runtime_source, subject).map_err(|error| error.to_string())?;
            compiles(
                run(
                    Path::new("rustc"),
                    &[
                        "--edition=2024".as_ref(),
                        "--test".as_ref(),
                        runtime_source.as_os_str(),
                        "-o".as_ref(),
                        runtime.as_os_str(),
                    ],
                )?,
                name,
            )?;
            let result = run(&runtime, &[])?;
            assert!(
                String::from_utf8_lossy(&result.stdout).contains("running 1 test"),
                "{name}: nonempty libtest subject"
            );
            assert_eq!(
                result.status.success(),
                !(wrong && mutant_detected),
                "{name}, wrong={wrong}"
            );
        }
        let mut entries = std::fs::read_dir(&scratch.0)
            .map_err(|error| error.to_string())?
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        entries.sort();
        assert_eq!(
            entries,
            ["Cargo.toml", "src"],
            "{name}: bounded analyzed root"
        );
        let sources = std::fs::read_dir(scratch.0.join("src"))
            .map_err(|error| error.to_string())?
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        assert_eq!(
            sources,
            ["lib.rs"],
            "{name}: exactly one analyzed Rust file"
        );
        let report = check_workspace(CheckInput {
            root: scratch.0.clone(),
            diff_file: Some(fixture.join("diff.patch")),
            mode: Mode::Fast,
            format: OutputFormat::Json,
            include_unchanged_tests: true,
            ..CheckInput::default()
        })?;
        let json: serde_json::Value =
            serde_json::from_str(&render_check(&report, &OutputFormat::Json)?)
                .map_err(|error| error.to_string())?;
        let findings = json["findings"].as_array().ok_or("missing findings")?;
        assert_eq!(findings.len(), 1, "{name}: unique predicate");
        let related = findings[0]["related_tests"]
            .as_array()
            .ok_or("missing related tests")?;
        assert_eq!(related.len(), 1, "{name}: one indexed test");
        assert_eq!(
            related[0]["file"], "src/lib.rs",
            "{name}: runtime source is not indexed"
        );
        assert_eq!(findings[0]["probe"]["family"], "predicate", "{name}");
        assert_eq!(
            findings[0]["oracle_strength"],
            if admitted { "strong" } else { "none" },
            "{name}"
        );
        if admitted {
            assert_eq!(findings[0]["classification"], "exposed", "{name}");
        }
    }
    Ok(())
}

#[test]
fn empty_macro_arguments_cannot_activate_a_real_far_oracle() -> Result<(), String> {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/predicate_empty_macro_boundary");
    let discarded = std::fs::read_to_string(fixture.join("input/src/lib.rs"))
        .map_err(|error| error.to_string())?;
    let far = discarded.replace("discard_tokens!(discounted_total(100, 100)); ", "");
    let positive = discarded.replace(
        "assert_eq!(discounted_total(90, 100), 90)",
        "assert_eq!(discounted_total(100, 100), 90)",
    );
    assert_ne!(far, discarded);
    assert_ne!(positive, discarded);
    let mut observations = Vec::new();
    for (name, source, exposed) in [
        ("discarded_boundary_far", discarded, false),
        ("far_only", far, false),
        ("real_boundary_positive", positive, true),
    ] {
        // The existing helper allocates a separate runtime Scratch, so none
        // of the compiled variants can enter the analyzed fixture below.
        source_runtime_control(&source, &format!("{name}-correct"), 1, false)?;
        let wrong = source.replace(
            "amount >= discount_threshold",
            "amount > discount_threshold",
        );
        assert_ne!(wrong, source);
        source_runtime_control(&wrong, &format!("{name}-wrong"), 1, exposed)?;
        let scratch = Scratch::create()?;
        std::fs::create_dir(scratch.0.join("src")).map_err(|error| error.to_string())?;
        std::fs::copy(
            fixture.join("input/Cargo.toml"),
            scratch.0.join("Cargo.toml"),
        )
        .map_err(|error| error.to_string())?;
        std::fs::write(scratch.0.join("src/lib.rs"), &source).map_err(|error| error.to_string())?;
        assert_eq!(
            std::fs::read_dir(&scratch.0)
                .map_err(|error| error.to_string())?
                .count(),
            2
        );
        let report = check_workspace(CheckInput {
            root: scratch.0.clone(),
            diff_file: Some(fixture.join("diff.patch")),
            mode: Mode::Fast,
            format: OutputFormat::Json,
            include_unchanged_tests: true,
            ..CheckInput::default()
        })?;
        let json: serde_json::Value =
            serde_json::from_str(&render_check(&report, &OutputFormat::Json)?)
                .map_err(|error| error.to_string())?;
        let findings = json["findings"].as_array().ok_or("missing findings")?;
        assert_eq!(findings.len(), 1, "{name}: one predicate");
        let finding = &findings[0];
        assert_eq!(finding["probe"]["family"], "predicate");
        assert_eq!(finding["related_tests"].as_array().map(Vec::len), Some(1));
        assert_eq!(finding["related_tests"][0]["file"], "src/lib.rs");
        assert_eq!(finding["oracle_strength"], "strong", "{name}");
        for stage in ["observe", "discriminate"] {
            assert_eq!(
                finding["ripr"][stage]["state"], "yes",
                "{name}: real far equality survives"
            );
        }
        assert_eq!(
            finding["classification"],
            if exposed { "exposed" } else { "weakly_exposed" },
            "{name}"
        );
        assert_eq!(
            finding["ripr"]["infect"]["state"],
            if exposed { "yes" } else { "weak" },
            "{name}"
        );
        if !exposed {
            assert!(
                finding["ripr"]["infect"]["summary"]
                    .as_str()
                    .is_some_and(|s| s.contains("equality-boundary discriminator is missing"))
            );
        }
        observations.push(finding.clone());
    }
    assert_eq!(
        observations[0]["classification"],
        observations[1]["classification"]
    );
    assert_eq!(
        observations[0]["ripr"]["infect"],
        observations[1]["ripr"]["infect"]
    );
    Ok(())
}

#[test]
fn owner_pin_matched_static_and_runtime_controls() -> Result<(), String> {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    for (case, exposed) in [
        ("direct", true),
        ("called_closure", true),
        ("uncalled_closure", false),
        ("shadowed_assertion", false),
        ("no_assertion", false),
        ("expired_closure_binding", false),
        ("deferred_closure_binding", false),
        ("macro_operand_return", false),
        ("macro_return", false),
        ("if_false", false),
        ("unpolled_async", false),
        ("token_overlap", false),
        ("token_direct", true),
        ("token_called_closure", true),
        ("unknown_singleton", false),
        ("nested_test", false),
        ("cfg_false_module", false),
        ("cfg_false_file", false),
        ("cfg_attr_module", false),
        ("out_of_line_cfg", false),
    ] {
        let fixture = fixtures.join(format!("owner_return_pin_{case}"));
        let report = check_workspace(CheckInput {
            root: fixture.join("input"),
            diff_file: Some(fixture.join("diff.patch")),
            mode: Mode::Fast,
            format: OutputFormat::Json,
            include_unchanged_tests: true,
            ..CheckInput::default()
        })?;
        let json: serde_json::Value =
            serde_json::from_str(&render_check(&report, &OutputFormat::Json)?)
                .map_err(|error| error.to_string())?;
        assert_eq!(
            json["analysis_outcome"]["analysis_complete"], true,
            "{case}"
        );
        assert_eq!(report.findings.len(), 1, "{case}: nonempty unique subject");
        let finding = &report.findings[0];
        assert_eq!(finding.probe.family, ProbeFamily::ReturnValue, "{case}");
        assert_eq!(
            finding.class,
            if exposed {
                ExposureClass::Exposed
            } else {
                ExposureClass::ReachableUnrevealed
            },
            "{case}"
        );
        if !exposed {
            let finding = &json["findings"][0];
            assert_eq!(finding["ripr"]["observe"]["state"], "no", "{case}");
            assert_eq!(finding["ripr"]["discriminate"]["state"], "no", "{case}");
            assert_eq!(finding["oracle_strength"], "none", "{case}");
            assert_eq!(
                finding["confidence"], 0.79,
                "advisory score follows unchanged stage arithmetic"
            );
            let tests = finding["related_tests"]
                .as_array()
                .ok_or("missing related test provenance")?;
            assert!(!tests.is_empty(), "{case}");
            assert!(
                tests
                    .iter()
                    .all(|test| test["oracle"].as_str().is_some_and(str::is_empty)
                        && test["oracle_strength"] == "none"),
                "{case}"
            );
            if case != "no_assertion" {
                assert!(
                    finding["ripr"]["observe"]["summary"]
                        .as_str()
                        .is_some_and(|text| text.contains("rust_assertion_context_unestablished")),
                    "{case}"
                );
                assert!(
                    finding["recommended_next_step"]
                        .as_str()
                        .is_some_and(|text| text.contains("executed path")),
                    "{case}"
                );
            }
        }
        let correct = std::fs::read_to_string(fixture.join("input/src/lib.rs"))
            .map_err(|error| error.to_string())?;
        assert_eq!(
            correct.matches("input * 3").count(),
            1,
            "{case}: mutation must alter one subject"
        );
        for broken in [false, true] {
            let scratch = Scratch::create()?;
            let lib = scratch.0.join("lib.rs");
            let rlib = scratch.0.join("libowner_pin_control.rlib");
            let executable = scratch
                .0
                .join(format!("weight_tests{}", std::env::consts::EXE_SUFFIX));
            std::fs::write(
                &lib,
                if broken {
                    correct.replace("input * 3", "input * 2")
                } else {
                    correct.clone()
                },
            )
            .map_err(|error| error.to_string())?;
            compiles(
                run(
                    Path::new("rustc"),
                    &[
                        "--edition=2024".as_ref(),
                        "--crate-name=owner_pin_control".as_ref(),
                        "--crate-type=rlib".as_ref(),
                        lib.as_os_str(),
                        "-o".as_ref(),
                        rlib.as_os_str(),
                    ],
                )?,
                "library",
            )?;
            let external = format!("owner_pin_control={}", rlib.display());
            compiles(
                run(
                    Path::new("rustc"),
                    &[
                        "--edition=2024".as_ref(),
                        "--test".as_ref(),
                        fixture.join("input/tests/weight_tests.rs").as_os_str(),
                        "--extern".as_ref(),
                        external.as_ref(),
                        "-o".as_ref(),
                        executable.as_os_str(),
                    ],
                )?,
                "test",
            )?;
            let runtime = run(&executable, &["--nocapture".as_ref()])?;
            let stdout = String::from_utf8_lossy(&runtime.stdout);
            assert!(stdout.contains("running 1 test"), "{case}: {stdout}");
            assert_eq!(
                runtime.status.success(),
                !broken || !exposed,
                "{case}, broken={broken}: {stdout}; {}",
                String::from_utf8_lossy(&runtime.stderr)
            );
            if broken && exposed {
                let stderr = String::from_utf8_lossy(&runtime.stderr);
                assert!(
                    stderr.contains("left: 8") && stderr.contains("right: 12"),
                    "{case}: {stderr}"
                );
                assert!(stdout.contains("1 failed"), "{case}: {stdout}");
            } else {
                assert!(stdout.contains("1 passed"), "{case}: {stdout}");
            }
            eprintln!(
                "owner-pin runtime: {case}, broken={broken}, {}, executed=1",
                runtime.status
            );
        }
    }
    Ok(())
}

#[test]
fn owner_pin_token_overlap_cannot_bypass_oracle_admission() -> Result<(), String> {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/owner_return_pin_token_overlap");
    let report = check_workspace(CheckInput {
        root: fixture.join("input"),
        diff_file: Some(fixture.join("diff.patch")),
        mode: Mode::Fast,
        format: OutputFormat::Json,
        include_unchanged_tests: true,
        ..CheckInput::default()
    })?;
    assert_eq!(
        report.findings.len(),
        1,
        "nonempty changed return-value subject"
    );
    assert_eq!(report.findings[0].class, ExposureClass::ReachableUnrevealed);
    let json: serde_json::Value =
        serde_json::from_str(&render_check(&report, &OutputFormat::Json)?)
            .map_err(|error| error.to_string())?;
    assert_eq!(json["findings"][0]["ripr"]["observe"]["state"], "no");
    assert_eq!(json["findings"][0]["ripr"]["discriminate"]["state"], "no");
    Ok(())
}

#[test]
fn owner_pin_shared_admission_keeps_credit_on_one_admitted_oracle() -> Result<(), String> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/owner_return_pin_direct");
    for (body, exposed, strength) in [
        (
            "let input = 4;\nassert_eq!(weight(input), 12);",
            true,
            "strong",
        ),
        (
            "let input = 4;\nlet check = || assert_eq!(weight(input), 12);\ncheck();",
            true,
            "strong",
        ),
        (
            "let input = 4;\nlet _later = || assert_eq!(weight(input), 12);\nassert_eq!(2 + 2, 4);",
            false,
            "strong",
        ),
        (
            "let input = 4;\nlet _later = || assert_eq!(weight(input), 12);\nassert!(weight(input) > 0);",
            false,
            "weak",
        ),
        (
            "let input = 4;\nassert_eq!(weight(input), 12);\nlet _later = || assert_eq!(weight(input), 12);",
            true,
            "strong",
        ),
        (
            "let input = 4;\nlet _later = || assert_eq!(weight(input), 12);\nassert_eq!(weight(input), 12);",
            true,
            "strong",
        ),
    ] {
        let scratch = Scratch::create()?;
        for directory in ["src", "tests"] {
            std::fs::create_dir(scratch.0.join(directory)).map_err(|error| error.to_string())?;
        }
        for file in ["Cargo.toml", "src/lib.rs"] {
            std::fs::copy(base.join("input").join(file), scratch.0.join(file))
                .map_err(|error| error.to_string())?;
        }
        std::fs::write(
            scratch.0.join("tests/weight_tests.rs"),
            format!("use owner_pin_control::weight;\n#[test]\nfn checks_weight() {{\n{body}\n}}\n"),
        )
        .map_err(|error| error.to_string())?;
        let report = check_workspace(CheckInput {
            root: scratch.0.clone(),
            diff_file: Some(base.join("diff.patch")),
            mode: Mode::Fast,
            format: OutputFormat::Json,
            include_unchanged_tests: true,
            ..CheckInput::default()
        })?;
        assert_eq!(report.findings.len(), 1, "{body}");
        assert_eq!(
            report.findings[0].class == ExposureClass::Exposed,
            exposed,
            "{body}"
        );
        let json: serde_json::Value =
            serde_json::from_str(&render_check(&report, &OutputFormat::Json)?)
                .map_err(|error| error.to_string())?;
        assert_eq!(
            json["analysis_outcome"]["analysis_complete"], true,
            "{body}"
        );
        assert_eq!(json["findings"][0]["oracle_strength"], strength, "{body}");
    }
    Ok(())
}

#[test]
fn owner_pin_refused_rows_do_not_crowd_out_admitted_oracles() -> Result<(), String> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/owner_return_pin_direct");
    let scratch = Scratch::create()?;
    for directory in ["src", "tests"] {
        std::fs::create_dir(scratch.0.join(directory)).map_err(|error| error.to_string())?;
    }
    for file in ["Cargo.toml", "src/lib.rs"] {
        std::fs::copy(base.join("input").join(file), scratch.0.join(file))
            .map_err(|error| error.to_string())?;
    }
    let tests = (0..8).map(|index| format!("#[test]\nfn checks_weight_{index}() {{\nlet input=4;\nlet _later = || assert_eq!(weight(input), 12);\nassert_eq!(weight(input), 12);\n}}\n")).collect::<String>();
    std::fs::write(
        scratch.0.join("tests/weight_tests.rs"),
        format!("use owner_pin_control::weight;\n{tests}"),
    )
    .map_err(|error| error.to_string())?;
    let report = check_workspace(CheckInput {
        root: scratch.0.clone(),
        diff_file: Some(base.join("diff.patch")),
        mode: Mode::Fast,
        format: OutputFormat::Json,
        include_unchanged_tests: true,
        ..CheckInput::default()
    })?;
    assert_eq!(report.findings.len(), 1);
    assert_eq!(report.findings[0].class, ExposureClass::Exposed);
    let json: serde_json::Value =
        serde_json::from_str(&render_check(&report, &OutputFormat::Json)?)
            .map_err(|error| error.to_string())?;
    let tests = json["findings"][0]["related_tests"]
        .as_array()
        .ok_or("missing related test rows")?;
    assert_eq!(tests.len(), 8);
    assert!(
        tests
            .iter()
            .all(|test| test["oracle_strength"] == "strong" && test["oracle"].is_string())
    );
    assert_eq!(json["findings"][0]["oracle_strength"], "strong");
    Ok(())
}

#[test]
fn owner_pin_review_admission_controls() -> Result<(), String> {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut mismatches = Vec::new();
    for case in [
        "unknown_singleton",
        "nested_test",
        "cfg_false_module",
        "cfg_false_file",
        "cfg_attr_module",
        "out_of_line_cfg",
    ] {
        let fixture = fixtures.join(format!("owner_return_pin_{case}"));
        let report = check_workspace(CheckInput {
            root: fixture.join("input"),
            diff_file: Some(fixture.join("diff.patch")),
            mode: Mode::Fast,
            format: OutputFormat::Json,
            include_unchanged_tests: true,
            ..CheckInput::default()
        })?;
        assert_eq!(report.findings.len(), 1, "{case}: nonempty unique subject");
        let json: serde_json::Value =
            serde_json::from_str(&render_check(&report, &OutputFormat::Json)?)
                .map_err(|error| error.to_string())?;
        let finding = &json["findings"][0];
        assert_eq!(
            json["analysis_outcome"]["analysis_complete"], true,
            "{case}"
        );
        if report.findings[0].class != ExposureClass::ReachableUnrevealed
            || finding["ripr"]["observe"]["state"] != "no"
            || finding["ripr"]["discriminate"]["state"] != "no"
            || finding["oracle_strength"] != "none"
        {
            mismatches.push(format!(
                "{case}: class={:?}, observe={}, discriminate={}, strength={}",
                report.findings[0].class,
                finding["ripr"]["observe"]["state"],
                finding["ripr"]["discriminate"]["state"],
                finding["oracle_strength"]
            ));
        }
    }
    if mismatches.is_empty() {
        Ok(())
    } else {
        Err(mismatches.join("\n"))
    }
}

/// Lexical enum facts remain useful even when the assertion is not admitted.
/// Their presentation must not imply the refused oracle observed a value.
#[test]
fn lexical_source_values_do_not_claim_observed_oracles() -> Result<(), String> {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    for case in [
        "direct",
        "called",
        "uncalled",
        "reached_uncalled",
        "shadowed",
        "false_branch",
    ] {
        let fixture = fixtures.join(format!("error_path_oracle_execution_{case}"));
        let report = check_workspace(CheckInput {
            root: fixture.join("input"),
            diff_file: Some(fixture.join("diff.patch")),
            mode: Mode::Fast,
            format: OutputFormat::Json,
            include_unchanged_tests: true,
            ..CheckInput::default()
        })?;
        let json: serde_json::Value =
            serde_json::from_str(&render_check(&report, &OutputFormat::Json)?)
                .map_err(|error| error.to_string())?;
        let selected = json["findings"]
            .as_array()
            .ok_or("missing findings")?
            .iter()
            .filter(|finding| finding["probe"]["family"] == "error_path")
            .collect::<Vec<_>>();
        assert_eq!(selected.len(), 1, "{case}: selected ErrorPath only");
        let finding = selected[0];
        let admitted = matches!(case, "direct" | "called");
        assert_eq!(
            finding["classification"],
            if admitted {
                "exposed"
            } else {
                "reachable_unrevealed"
            },
            "{case}"
        );
        assert_eq!(
            finding["oracle_strength"],
            if admitted { "strong" } else { "none" },
            "{case}"
        );
        for stage in ["observe", "discriminate"] {
            assert_eq!(
                finding["ripr"][stage]["state"],
                if admitted { "yes" } else { "no" },
                "{case}/{stage}"
            );
        }
        let facts = finding["observed_values"]
            .as_array()
            .ok_or("missing source values")?;
        let variants = facts
            .iter()
            .filter(|fact| {
                fact["context"] == "enum_variant" && fact["value"] == "io::ErrorKind::Other"
            })
            .collect::<Vec<_>>();
        assert!(!variants.is_empty(), "{case}: retain lexical enum facts");
        assert_eq!(
            finding["observed_values"],
            finding["activation"]["observed_values"]
        );
        let evidence = finding["evidence_path"]
            .as_array()
            .ok_or("missing evidence path")?;
        let human = render_check(&report, &OutputFormat::HumanFull)?;
        for variant in variants {
            let line = variant["line"].as_u64().ok_or("missing fact line")?;
            assert!(
                finding["assertion_texts"][line.to_string()]
                    .as_str()
                    .is_some_and(|text| text.contains("io::ErrorKind::Other")),
                "{case}: source provenance remains"
            );
            let label = format!("source enum variant value io::ErrorKind::Other at line {line}");
            assert!(
                evidence
                    .iter()
                    .any(|entry| entry.as_str() == Some(label.as_str())),
                "{case}: neutral structured evidence"
            );
            assert!(human.contains(&label), "{case}: neutral human evidence");
        }
        assert!(!evidence.iter().any(|entry| {
            entry
                .as_str()
                .is_some_and(|text| text.starts_with("observed enum variant value "))
        }));
        assert!(!human.contains("observed enum variant value "));
    }
    Ok(())
}

/// Shared execution provenance is independent of the changed behavior's family.
/// The error assertions use semantic operands, never diagnostic text (#5027).
#[test]
fn equality_oracle_family_matched_static_and_runtime_controls() -> Result<(), String> {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    for (family, probe_family, before, after) in [
        (
            "error_path",
            ProbeFamily::ErrorPath,
            "rdr.read(&mut buf).unwrap_or(0)",
            "rdr.read(&mut buf)?",
        ),
        (
            "predicate",
            ProbeFamily::Predicate,
            "amount > discount_threshold",
            "amount >= discount_threshold",
        ),
    ] {
        for case in [
            "direct",
            "called",
            "uncalled",
            "false_branch",
            "shadowed",
            "no_assertion",
            "reached_uncalled",
        ] {
            let exposed = matches!(case, "direct" | "called");
            let fixture = fixtures.join(format!("{family}_oracle_execution_{case}"));
            let report = check_workspace(CheckInput {
                root: fixture.join("input"),
                diff_file: Some(fixture.join("diff.patch")),
                mode: Mode::Fast,
                format: OutputFormat::Json,
                include_unchanged_tests: true,
                ..CheckInput::default()
            })?;
            let findings: Vec<_> = report
                .findings
                .iter()
                .filter(|finding| finding.probe.family == probe_family)
                .collect();
            assert_eq!(findings.len(), 1, "{family}/{case}: unique family subject");
            assert_eq!(
                findings[0].class,
                if exposed {
                    ExposureClass::Exposed
                } else {
                    ExposureClass::ReachableUnrevealed
                },
                "{family}/{case}"
            );
            let json: serde_json::Value =
                serde_json::from_str(&render_check(&report, &OutputFormat::Json)?)
                    .map_err(|error| error.to_string())?;
            assert_eq!(json["analysis_outcome"]["analysis_complete"], true);
            let finding = json["findings"]
                .as_array()
                .and_then(|findings| {
                    findings
                        .iter()
                        .find(|finding| finding["probe"]["family"] == family)
                })
                .ok_or("selected family disappeared from JSON")?;
            for stage in ["observe", "discriminate"] {
                assert_eq!(
                    finding["ripr"][stage]["state"],
                    if exposed { "yes" } else { "no" },
                    "{family}/{case}/{stage}"
                );
            }
            assert_eq!(
                finding["oracle_strength"],
                if exposed { "strong" } else { "none" },
                "{family}/{case}"
            );
            let related = finding["related_tests"]
                .as_array()
                .ok_or("missing related test provenance")?;
            assert_eq!(related.len(), 1, "{family}/{case}");
            if !exposed {
                assert_eq!(related[0]["oracle_strength"], "none");
                assert_eq!(related[0]["oracle"], "");
                if case != "no_assertion" {
                    assert!(
                        finding["ripr"]["observe"]["summary"]
                            .as_str()
                            .is_some_and(
                                |summary| summary.contains("rust_assertion_context_unestablished")
                            ),
                        "{family}/{case}"
                    );
                }
            }

            let correct = std::fs::read_to_string(fixture.join("input/src/lib.rs"))
                .map_err(|error| error.to_string())?;
            assert_eq!(correct.matches(after).count(), 1, "unique mutation subject");
            for broken in [false, true] {
                // Generated mutation sources must remain outside analyzed roots.
                let scratch = Scratch::create()?;
                let source = scratch.0.join("subject.rs");
                let executable = scratch
                    .0
                    .join(format!("family_tests{}", std::env::consts::EXE_SUFFIX));
                std::fs::write(
                    &source,
                    if broken {
                        correct.replace(after, before)
                    } else {
                        correct.clone()
                    },
                )
                .map_err(|error| error.to_string())?;
                compiles(
                    run(
                        Path::new("rustc"),
                        &[
                            "--edition=2024".as_ref(),
                            "--crate-name=oracle_family_control".as_ref(),
                            "--test".as_ref(),
                            source.as_os_str(),
                            "-o".as_ref(),
                            executable.as_os_str(),
                        ],
                    )?,
                    "family fixture",
                )?;
                let listed = run(&executable, &["--list".as_ref()])?;
                assert!(listed.status.success(), "{family}/{case}: list failed");
                assert!(
                    String::from_utf8_lossy(&listed.stdout)
                        .trim_end()
                        .ends_with("1 test, 0 benchmarks"),
                    "{family}/{case}: unique runtime subject"
                );
                let runtime = run(&executable, &["--nocapture".as_ref()])?;
                let stdout = String::from_utf8_lossy(&runtime.stdout);
                assert!(
                    stdout.contains("running 1 test"),
                    "{family}/{case}: {stdout}"
                );
                assert_eq!(
                    runtime.status.code(),
                    Some(if broken && exposed { 101 } else { 0 }),
                    "{family}/{case}, broken={broken}: {stdout}; {}",
                    String::from_utf8_lossy(&runtime.stderr)
                );
                eprintln!(
                    "family-oracle runtime: {family}/{case}, broken={broken}, {}, executed=1",
                    runtime.status
                );
            }
        }
    }
    Ok(())
}

#[test]
fn equality_family_admission_preserves_mixed_oracle_identity_and_projection() -> Result<(), String>
{
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    for (family, strong, weak, binding) in [
        (
            "error_path",
            "assert_eq!(read_all(rdr).unwrap_err().kind(), io::ErrorKind::Other);",
            "assert!(read_all(rdr).is_err());",
            "let rdr = BrokenReader;",
        ),
        (
            "predicate",
            "assert_eq!(discounted_total(100, 100), 90);",
            "assert!(discounted_total(100, 100) > 0);",
            "",
        ),
    ] {
        let fixture = fixtures.join(format!("{family}_oracle_execution_direct"));
        let original = std::fs::read_to_string(fixture.join("input/src/lib.rs"))
            .map_err(|error| error.to_string())?;
        let (prefix, _) = original
            .split_once("    #[test]\n")
            .ok_or("missing fixture test boundary")?;
        let deferred = format!("let _later = || {{ {binding} {strong} }};");
        for (body, expected_strength, exposed, copies) in [
            (format!("{strong}\n{deferred}"), "strong", true, 1),
            (format!("{deferred}\n{strong}"), "strong", true, 1),
            (format!("{weak}\n{deferred}"), "weak", false, 1),
            (format!("{deferred}\n{weak}"), "weak", false, 1),
            (format!("{deferred}\nassert_ready(true);"), "none", false, 1),
            (format!("{deferred}\n{strong}"), "strong", true, 10),
        ] {
            let scratch = Scratch::create()?;
            std::fs::create_dir(scratch.0.join("src")).map_err(|error| error.to_string())?;
            std::fs::copy(
                fixture.join("input/Cargo.toml"),
                scratch.0.join("Cargo.toml"),
            )
            .map_err(|error| error.to_string())?;
            let functions = (0..copies)
                .map(|index| {
                    format!("    #[test]\n    fn checks_{index}() {{\n{binding}\n{body}\n    }}\n")
                })
                .collect::<String>();
            std::fs::write(
                scratch.0.join("src/lib.rs"),
                format!("{prefix}    fn assert_ready(_: bool) {{}}\n{functions}}}\n"),
            )
            .map_err(|error| error.to_string())?;
            let report = check_workspace(CheckInput {
                root: scratch.0.clone(),
                diff_file: Some(fixture.join("diff.patch")),
                mode: Mode::Fast,
                format: OutputFormat::Json,
                include_unchanged_tests: true,
                ..CheckInput::default()
            })?;
            let json: serde_json::Value =
                serde_json::from_str(&render_check(&report, &OutputFormat::Json)?)
                    .map_err(|error| error.to_string())?;
            let findings = json["findings"]
                .as_array()
                .ok_or("missing findings")?
                .iter()
                .filter(|finding| finding["probe"]["family"] == family)
                .collect::<Vec<_>>();
            assert_eq!(findings.len(), 1, "{family}: {body}");
            let finding = findings[0];
            assert_eq!(
                finding["classification"] == "exposed",
                exposed,
                "{family}: {body}"
            );
            assert_eq!(
                finding["oracle_strength"], expected_strength,
                "{family}: {body}"
            );
            assert_eq!(json["analysis_outcome"]["analysis_complete"], true);
            let related = finding["related_tests"]
                .as_array()
                .ok_or("missing related-test rows")?;
            assert_eq!(related.len(), copies.min(8), "{family}: {body}");
            assert!(
                related
                    .iter()
                    .all(|test| test["oracle_strength"] == expected_strength),
                "{family}: {body}"
            );
            if expected_strength == "none" {
                assert_eq!(finding["ripr"]["observe"]["state"], "no");
                assert_eq!(finding["ripr"]["discriminate"]["state"], "no");
            }
        }
    }
    Ok(())
}

#[test]
fn equality_execution_uses_statement_prefix_and_closure_invocation() -> Result<(), String> {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    for (family, assertion, binding, correct_expression, wrong_expression) in [
        (
            "return_value",
            "assert_eq!(weight(4), 12);",
            "",
            "input * 3",
            "input * 2",
        ),
        (
            "error_path",
            "assert_eq!(read_all(rdr).unwrap_err().kind(), io::ErrorKind::Other);",
            "let rdr = BrokenReader;",
            "rdr.read(&mut buf)?",
            "rdr.read(&mut buf).unwrap_or(0)",
        ),
        (
            "predicate",
            "assert_eq!(discounted_total(100, 100), 90);",
            "",
            "amount >= discount_threshold",
            "amount > discount_threshold",
        ),
    ] {
        let fixture = fixtures.join(if family == "return_value" {
            "owner_return_pin_direct".to_string()
        } else {
            format!("{family}_oracle_execution_direct")
        });
        let original = std::fs::read_to_string(fixture.join("input/src/lib.rs"))
            .map_err(|error| error.to_string())?;
        let prefix = original
            .split_once("    #[test]\n")
            .map(|(prefix, _)| prefix.to_string())
            .unwrap_or_else(|| {
                format!("{original}\n#[cfg(test)]\nmod tests {{\n    use super::*;\n")
            });
        let direct = format!("{binding} {assertion}");
        // Keep this execution control independent of the narrower owner-pin
        // binding gate, which conservatively refuses any nested function.
        // The existing token-direct ReturnValue fixture uses this same shape.
        let helper_direct = if family == "return_value" {
            "let input = 4;\nassert_eq!(weight(input), 12);".to_string()
        } else {
            direct.clone()
        };
        for (case, body, exposed) in [
            ("later_return", format!("{direct}\nreturn;"), true),
            ("earlier_return", format!("return;\n{direct}"), false),
            (
                "invoked_before_return",
                format!("let check = || {{ {direct} }};\ncheck();\nreturn;"),
                true,
            ),
            (
                "invoked_after_return",
                format!("let check = || {{ {direct} }};\nreturn;\ncheck();"),
                false,
            ),
            (
                "closure_escape",
                format!("let check = || {{ return;\n{direct} }};\ncheck();"),
                false,
            ),
            (
                "nested_helper_return",
                format!("fn unrelated() {{ return; }}\n{helper_direct}"),
                true,
            ),
            (
                "disabled_nested_helper_return",
                format!("#[cfg(any())]\nfn unrelated() {{ return; }}\n{helper_direct}"),
                true,
            ),
            (
                "invoked_after_nested_helper_return",
                format!(
                    "fn unrelated() {{ return; }}\nlet check = || {{ {helper_direct} }};\ncheck();"
                ),
                true,
            ),
            (
                "invoked_after_disabled_nested_helper_return",
                format!(
                    "#[cfg(any())]\nfn unrelated() {{ return; }}\nlet check = || {{ {helper_direct} }};\ncheck();"
                ),
                true,
            ),
            (
                "async_return_direct",
                format!("let _future = async {{ return; }};\n{direct}"),
                true,
            ),
            (
                "async_return_invoked",
                format!(
                    "let _future = async {{ return; }};\nlet check = || {{ {direct} }}; check();"
                ),
                true,
            ),
            (
                "async_move_return_direct",
                format!("let _future = async move {{ return; }};\n{direct}"),
                true,
            ),
            (
                "async_move_return_invoked",
                format!(
                    "let _future = async move {{ return; }};\nlet check = || {{ {direct} }}; check();"
                ),
                true,
            ),
            (
                "async_return_then_outer_return",
                format!("let _future = async {{ return; }};\nreturn;\n{direct}"),
                false,
            ),
            (
                "assertion_inside_unpolled_async",
                format!("let _future = async {{ return;\n{direct} }};"),
                false,
            ),
        ] {
            let scratch = Scratch::create()?;
            let source = format!("{prefix}    #[test]\n    fn checks() {{\n{body}\n    }}\n}}\n");
            std::fs::create_dir(scratch.0.join("src")).map_err(|error| error.to_string())?;
            std::fs::copy(
                fixture.join("input/Cargo.toml"),
                scratch.0.join("Cargo.toml"),
            )
            .map_err(|error| error.to_string())?;
            std::fs::write(scratch.0.join("src/lib.rs"), &source)
                .map_err(|error| error.to_string())?;
            let report = check_workspace(CheckInput {
                root: scratch.0.clone(),
                diff_file: Some(fixture.join("diff.patch")),
                mode: Mode::Fast,
                format: OutputFormat::Json,
                include_unchanged_tests: true,
                ..CheckInput::default()
            })?;
            let json: serde_json::Value =
                serde_json::from_str(&render_check(&report, &OutputFormat::Json)?)
                    .map_err(|error| error.to_string())?;
            let findings = json["findings"]
                .as_array()
                .ok_or("missing findings")?
                .iter()
                .filter(|finding| finding["probe"]["family"] == family)
                .collect::<Vec<_>>();
            assert_eq!(findings.len(), 1, "{family}/{case}");
            assert_eq!(
                findings[0]["classification"],
                if exposed {
                    "exposed"
                } else {
                    "reachable_unrevealed"
                },
                "{family}/{case}"
            );
            assert_eq!(
                findings[0]["oracle_strength"],
                if exposed { "strong" } else { "none" },
                "{family}/{case}"
            );
            assert_eq!(source.matches(correct_expression).count(), 1);
            for wrong in [false, true] {
                let runtime_source = if wrong {
                    source.replace(correct_expression, wrong_expression)
                } else {
                    source.clone()
                };
                source_runtime_control(
                    &runtime_source,
                    &format!("prefix runtime: {family}/{case}, wrong={wrong}"),
                    1,
                    wrong && exposed,
                )?;
            }
        }
    }
    Ok(())
}

#[test]
fn async_test_discovery_does_not_supply_execution_provenance() -> Result<(), String> {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/rust_async_fn_owner");
    let report = check_workspace(CheckInput {
        root: fixture.join("input"),
        diff_file: Some(fixture.join("diff.patch")),
        mode: Mode::Fast,
        format: OutputFormat::Json,
        include_unchanged_tests: true,
        ..CheckInput::default()
    })?;
    assert_eq!(report.findings.len(), 1, "retain the actual async owner");
    assert_eq!(report.findings[0].class, ExposureClass::PropagationUnknown);
    let json: serde_json::Value =
        serde_json::from_str(&render_check(&report, &OutputFormat::Json)?)
            .map_err(|error| error.to_string())?;
    assert_eq!(json["analysis_outcome"]["analysis_complete"], true);
    let finding = &json["findings"][0];
    assert_eq!(finding["probe"]["family"], "predicate");
    assert_eq!(finding["ripr"]["observe"]["state"], "no");
    assert_eq!(finding["ripr"]["discriminate"]["state"], "no");
    assert_eq!(finding["oracle_strength"], "none");
    let tests = finding["related_tests"]
        .as_array()
        .ok_or("missing indexed async test")?;
    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0]["name"], "above_threshold_gets_reduced");
    assert_eq!(tests[0]["oracle_strength"], "none");
    // #5040 owns establishing the missing macro-binding/polling edge. The
    // independently executed Tokio removal controls show the real test works;
    // source discovery alone must not manufacture that execution provenance.
    Ok(())
}

fn source_runtime_control(
    source: &str,
    label: &str,
    expected_tests: usize,
    should_fail: bool,
) -> Result<(), String> {
    let runtime = Scratch::create()?;
    let path = runtime.0.join("subject.rs");
    std::fs::write(&path, source).map_err(|error| error.to_string())?;
    let executable = runtime
        .0
        .join(format!("test{}", std::env::consts::EXE_SUFFIX));
    compiles(
        run(
            Path::new("rustc"),
            &[
                "--edition=2024".as_ref(),
                "--test".as_ref(),
                path.as_os_str(),
                "-o".as_ref(),
                executable.as_os_str(),
            ],
        )?,
        label,
    )?;
    let listed = run(&executable, &["--list".as_ref()])?;
    assert!(listed.status.success(), "{label}");
    let suffix = format!(
        "{expected_tests} test{}, 0 benchmarks",
        if expected_tests == 1 { "" } else { "s" }
    );
    assert!(
        String::from_utf8_lossy(&listed.stdout)
            .trim_end()
            .ends_with(&suffix),
        "{label}: wrong runtime subject inventory"
    );
    let result = run(&executable, &["--nocapture".as_ref()])?;
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(
        stdout.contains(&format!("running {expected_tests} test")),
        "{label}: {stdout}"
    );
    let failed = usize::from(should_fail);
    assert!(
        stdout.contains(&format!(
            "{} passed; {failed} failed;",
            expected_tests - failed
        )),
        "{label}: expected all declared subjects to execute: {stdout}"
    );
    assert_eq!(
        result.status.code(),
        Some(if should_fail { 101 } else { 0 }),
        "{label}: {stdout}; {}",
        String::from_utf8_lossy(&result.stderr)
    );
    eprintln!("{label}, {}, executed={expected_tests}", result.status);
    Ok(())
}

#[test]
fn predicate_pairing_cannot_reuse_refused_boundary_equalities() -> Result<(), String> {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/predicate_oracle_execution_direct");
    let original = std::fs::read_to_string(fixture.join("input/src/lib.rs"))
        .map_err(|error| error.to_string())?;
    let (production, _) = original
        .split_once("#[cfg(test)]")
        .ok_or("missing test boundary")?;
    let boundary = "assert_eq!(discounted_total(100, 100), 90);";
    let far = "assert_eq!(discounted_total(101, 100), 91);";
    for separate in [true, false] {
        for (case, body, exposed) in [
            ("false_branch", format!("if false {{ {boundary} }}"), false),
            (
                "uncalled",
                format!("let _unused = || {{ {boundary} }};"),
                false,
            ),
            (
                "unpolled",
                format!("let _future = async {{ {boundary} }};"),
                false,
            ),
            ("direct", boundary.to_string(), true),
            (
                "invoked",
                format!("let check = || {{ {boundary} }}; check();"),
                true,
            ),
            (
                "removed",
                "let _ = discounted_total(100, 100);".to_string(),
                false,
            ),
        ] {
            let tests = if separate {
                format!("#[test]\nfn boundary() {{ {body} }}\n#[test]\nfn far() {{ {far} }}")
            } else {
                format!("#[test]\nfn mixed() {{ {body}\n{far} }}")
            };
            let source =
                format!("{production}#[cfg(test)]\nmod tests {{\nuse super::*;\n{tests}\n}}\n");
            let scratch = Scratch::create()?;
            std::fs::create_dir(scratch.0.join("src")).map_err(|error| error.to_string())?;
            std::fs::copy(
                fixture.join("input/Cargo.toml"),
                scratch.0.join("Cargo.toml"),
            )
            .map_err(|error| error.to_string())?;
            std::fs::write(scratch.0.join("src/lib.rs"), &source)
                .map_err(|error| error.to_string())?;
            let report = check_workspace(CheckInput {
                root: scratch.0.clone(),
                diff_file: Some(fixture.join("diff.patch")),
                mode: Mode::Fast,
                format: OutputFormat::Json,
                include_unchanged_tests: true,
                ..CheckInput::default()
            })?;
            let json: serde_json::Value =
                serde_json::from_str(&render_check(&report, &OutputFormat::Json)?)
                    .map_err(|error| error.to_string())?;
            assert_eq!(json["analysis_outcome"]["analysis_complete"], true);
            let findings = json["findings"]
                .as_array()
                .ok_or("missing findings")?
                .iter()
                .filter(|f| f["probe"]["family"] == "predicate")
                .collect::<Vec<_>>();
            assert_eq!(findings.len(), 1, "{case}, separate={separate}");
            let finding = findings[0];
            assert_eq!(
                finding["classification"],
                if exposed { "exposed" } else { "weakly_exposed" },
                "{case}, separate={separate}"
            );
            // The far oracle remains real strong evidence. It cannot donate
            // its strength to a refused boundary oracle or be erased wholesale.
            assert_eq!(finding["oracle_strength"], "strong", "{case}");
            assert_eq!(finding["ripr"]["observe"]["state"], "yes", "{case}");
            assert_eq!(
                finding["ripr"]["discriminate"]["state"],
                if exposed { "yes" } else { "weak" },
                "{case}"
            );
            if !exposed {
                assert!(
                    finding["ripr"]["discriminate"]["summary"]
                        .as_str()
                        .is_some_and(|s| s.contains("same_test_pairing_missing")
                            && !s.contains("different tests")),
                    "{case}"
                );
            }
            assert_eq!(source.matches("amount >= discount_threshold").count(), 1);
            for wrong in [false, true] {
                let runtime_source = if wrong {
                    source.replace(
                        "amount >= discount_threshold",
                        "amount > discount_threshold",
                    )
                } else {
                    source.clone()
                };
                source_runtime_control(
                    &runtime_source,
                    &format!("pairing runtime: {case}, separate={separate}, wrong={wrong}"),
                    if separate { 2 } else { 1 },
                    wrong && exposed,
                )?;
            }
        }
    }
    Ok(())
}
