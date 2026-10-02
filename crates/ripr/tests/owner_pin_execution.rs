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
