use crate::output::diff_report::{build_diff_report, render_diff_report_json};
use serde_json::Value;
use std::{fs, path::Path};

fn preview_entry(rendered: &str, context: &str) -> Result<Value, String> {
    let value: Value =
        serde_json::from_str(rendered).map_err(|error| format!("parse {context}: {error}"))?;
    value
        .get("preview_languages")
        .and_then(Value::as_array)
        .and_then(|entries| entries.first())
        .cloned()
        .ok_or_else(|| format!("{context} missing preview_languages[0]: {rendered}"))
}

fn write(path: &Path, contents: &str) -> Result<(), String> {
    fs::write(path, contents).map_err(|error| format!("write {}: {error}", path.display()))
}

fn temp_root(name: &str) -> Result<std::path::PathBuf, String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("system time: {error}"))?
        .as_nanos();
    let root = std::env::temp_dir().join(format!("ripr-{name}-{}-{stamp}", std::process::id()));
    fs::create_dir_all(&root).map_err(|error| format!("create {}: {error}", root.display()))?;
    Ok(root)
}

fn assert_renderer_agreement(
    output: &crate::CheckOutput,
    expected_enabled: bool,
    expected_analyzed: bool,
) -> Result<(), String> {
    let json = crate::render_check(output, &crate::OutputFormat::Json)?;
    let report = build_diff_report(
        output,
        "origin/main",
        "HEAD",
        Vec::new(),
        "target/ripr/receipts/test.json".to_string(),
    );
    let diff_json = render_diff_report_json(&report)?;
    for (context, rendered) in [
        ("check JSON", json.as_str()),
        ("diff report", diff_json.as_str()),
    ] {
        let preview = preview_entry(rendered, context)?;
        if preview.get("enabled").and_then(Value::as_bool) != Some(expected_enabled)
            || preview.get("analyzed").and_then(Value::as_bool) != Some(expected_analyzed)
        {
            return Err(format!(
                "{context} preview state disagreed with producer outcome: {preview}"
            ));
        }
    }
    Ok(())
}

#[test]
fn shared_preview_completion_predicate_fails_closed_for_every_non_success_status()
-> Result<(), String> {
    let advisory = crate::analysis::PreviewLanguageAdvisory {
        language: "python".to_string(),
        file_count: 1,
        sample_paths: vec!["src/app.py".to_string()],
        javascript_file_count: 0,
        enabled: true,
    };
    if !advisory.analyzed(&[]) {
        return Err("an enabled routed run without a failure record must be analyzed".to_string());
    }
    for status in [
        crate::analysis::LanguageRunStatus::Unavailable,
        crate::analysis::LanguageRunStatus::Partial,
        crate::analysis::LanguageRunStatus::Invalid,
    ] {
        let runs = vec![crate::analysis::LanguageRun {
            language: "python".to_string(),
            status,
            reason: Some("producer did not complete".to_string()),
        }];
        if advisory.analyzed(&runs) {
            return Err(format!(
                "non-success status {status:?} claimed analyzed=true"
            ));
        }
    }
    Ok(())
}

#[cfg(feature = "lang-perl")]
#[test]
fn malformed_perl_adapter_run_is_enabled_but_not_analyzed_in_every_renderer() -> Result<(), String>
{
    let root = temp_root("preview-perl-invalid")?;
    let proof = (|| -> Result<(), String> {
        let facts = root.join("bad-facts.json");
        write(
            &facts,
            r#"{
  "schema_version": "ripr-perl-facts-v1",
  "packet_id": "perl-facts:repo:bad",
  "packet_status": "complete",
  "packet_fingerprint": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
  "producer": {"name": "perl-lsp", "version": "0.0.0", "capabilities": ["syntax"]},
  "root": {"repo_relative": ".", "vcs_head": "abc", "path_style": "repo_relative"},
  "input": {"base": "origin/main", "head": "HEAD", "diff_id": null, "requested_fact_classes": []},
  "files": [], "owners": [], "changes": [], "tests": [], "oracles": [],
  "relations": [], "dynamic_boundaries": [], "verify_commands": [],
  "limitations": [], "provenance": []
}"#,
        )?;
        let diff = root.join("perl.diff");
        write(
            &diff,
            "diff --git a/lib/App.pm b/lib/App.pm\n--- /dev/null\n+++ b/lib/App.pm\n@@ -0,0 +1 @@\n+sub discount { return 0 }\n",
        )?;
        let config =
            crate::config::tests_only_parse("[languages]\nenabled = [\"rust\", \"perl\"]\n")?;
        let output = crate::app::check_workspace_with_config(
            crate::CheckInput {
                root: root.clone(),
                base: None,
                diff_file: Some(diff),
                mode: crate::Mode::Draft,
                format: crate::OutputFormat::Json,
                include_unchanged_tests: false,
                perl_facts_path: Some(facts),
                suppression_policy: None,
                git_timeout: None,
                git_candidate: None,
            },
            &config,
        )?;
        let perl_run = output
            .language_runs
            .iter()
            .find(|run| run.language == "perl")
            .ok_or_else(|| "missing invalid Perl language_run".to_string())?;
        if perl_run.status != crate::analysis::LanguageRunStatus::Invalid {
            return Err(format!("expected invalid Perl run, got {perl_run:?}"));
        }
        assert_renderer_agreement(&output, true, false)?;
        let human = crate::render_check(&output, &crate::OutputFormat::Human)?;
        if !human.contains("did not complete successfully (invalid)")
            || human.contains("analyzed under preview support")
        {
            return Err(format!("human check is not failure-aware: {human}"));
        }
        Ok(())
    })();
    let cleanup =
        fs::remove_dir_all(&root).map_err(|error| format!("remove {}: {error}", root.display()));
    proof?;
    cleanup
}

/// Perl enabled with no fact packet: the run is `unavailable`, the note agrees
/// with one file ("1 Perl file was"), and the reason names what to pass in
/// user terms rather than an internal campaign reference (2026-09-27 re-walk).
#[cfg(feature = "lang-perl")]
#[test]
fn missing_perl_fact_packet_reason_is_user_facing() -> Result<(), String> {
    let root = temp_root("preview-perl-missing-packet")?;
    let proof = (|| -> Result<(), String> {
        let diff = root.join("perl.diff");
        write(
            &diff,
            "diff --git a/lib/App.pm b/lib/App.pm\n--- /dev/null\n+++ b/lib/App.pm\n@@ -0,0 +1 @@\n+sub discount { return 0 }\n",
        )?;
        let config =
            crate::config::tests_only_parse("[languages]\nenabled = [\"rust\", \"perl\"]\n")?;
        let output = crate::app::check_workspace_with_config(
            crate::CheckInput {
                root: root.clone(),
                base: None,
                diff_file: Some(diff),
                mode: crate::Mode::Draft,
                format: crate::OutputFormat::Json,
                include_unchanged_tests: false,
                perl_facts_path: None,
                suppression_policy: None,
                git_timeout: None,
                git_candidate: None,
            },
            &config,
        )?;
        let perl_run = output
            .language_runs
            .iter()
            .find(|run| run.language == "perl")
            .ok_or_else(|| "missing Perl language_run".to_string())?;
        if perl_run.status != crate::analysis::LanguageRunStatus::Unavailable {
            return Err(format!("expected unavailable Perl run, got {perl_run:?}"));
        }
        let human = crate::render_check(&output, &crate::OutputFormat::Human)?;
        if !human.contains("so 1 Perl file was not analyzed") {
            return Err(format!("expected singular verb for one file: {human}"));
        }
        if !human.contains("--perl-facts <packet.json>") || human.contains("Campaign") {
            return Err(format!(
                "reason must name the flag, not a campaign: {human}"
            ));
        }
        Ok(())
    })();
    let cleanup =
        fs::remove_dir_all(&root).map_err(|error| format!("remove {}: {error}", root.display()));
    proof?;
    cleanup
}

#[cfg(feature = "lang-typescript")]
fn typescript_output(enabled: bool) -> Result<(std::path::PathBuf, crate::CheckOutput), String> {
    let root = temp_root(if enabled {
        "preview-typescript-success"
    } else {
        "preview-typescript-disabled"
    })?;
    let diff = root.join("typescript.diff");
    write(
        &diff,
        "diff --git a/src/discount.ts b/src/discount.ts\n--- /dev/null\n+++ b/src/discount.ts\n@@ -0,0 +1 @@\n+export const discount = (amount: number) => amount > 10 ? amount - 1 : amount;\n",
    )?;
    let config = if enabled {
        crate::config::tests_only_parse("[languages]\nenabled = [\"rust\", \"typescript\"]\n")?
    } else {
        crate::config::tests_only_parse("[languages]\nenabled = [\"rust\"]\n")?
    };
    let output = crate::app::check_workspace_with_config(
        crate::CheckInput {
            root: root.clone(),
            base: None,
            diff_file: Some(diff),
            mode: crate::Mode::Draft,
            format: crate::OutputFormat::Json,
            include_unchanged_tests: false,
            perl_facts_path: None,
            suppression_policy: None,
            git_timeout: None,
            git_candidate: None,
        },
        &config,
    )?;
    Ok((root, output))
}

#[cfg(feature = "lang-typescript")]
#[test]
fn successful_enabled_preview_run_is_analyzed_in_every_renderer() -> Result<(), String> {
    let (root, output) = typescript_output(true)?;
    let proof = assert_renderer_agreement(&output, true, true);
    let cleanup =
        fs::remove_dir_all(&root).map_err(|error| format!("remove {}: {error}", root.display()));
    proof?;
    cleanup
}

#[cfg(feature = "lang-typescript")]
#[test]
fn disabled_preview_run_is_not_enabled_or_analyzed_in_every_renderer() -> Result<(), String> {
    let (root, output) = typescript_output(false)?;
    let proof = assert_renderer_agreement(&output, false, false);
    let cleanup =
        fs::remove_dir_all(&root).map_err(|error| format!("remove {}: {error}", root.display()));
    proof?;
    cleanup
}

/// #4372 / RIPR-SPEC-0082: a changed file that the enabled TypeScript adapter
/// refuses (#3743 excluded path, or a generated name) is not counted as
/// analyzed, and is still disclosed as a typed skipped-scope limitation so the
/// outcome is never a silently complete result.
#[cfg(feature = "lang-typescript")]
fn typescript_output_for_paths(
    name: &str,
    paths: &[&str],
) -> Result<(std::path::PathBuf, crate::CheckOutput), String> {
    let root = temp_root(name)?;
    let diff = root.join("typescript.diff");
    let text = paths
        .iter()
        .map(|path| {
            format!(
                "diff --git a/{path} b/{path}\n--- /dev/null\n+++ b/{path}\n@@ -0,0 +1 @@\n\
                 +export const discount = (amount: number) => amount > 10 ? amount - 1 : amount;\n"
            )
        })
        .collect::<String>();
    write(&diff, &text)?;
    let config =
        crate::config::tests_only_parse("[languages]\nenabled = [\"rust\", \"typescript\"]\n")?;
    let output = crate::app::check_workspace_with_config(
        crate::CheckInput {
            root: root.clone(),
            base: None,
            diff_file: Some(diff),
            mode: crate::Mode::Draft,
            format: crate::OutputFormat::Json,
            include_unchanged_tests: false,
            perl_facts_path: None,
            suppression_policy: None,
            git_timeout: None,
            git_candidate: None,
        },
        &config,
    )?;
    Ok((root, output))
}

#[cfg(feature = "lang-typescript")]
fn skipped_scope_limitation(json: &str) -> Result<(String, Value), String> {
    let value: Value =
        serde_json::from_str(json).map_err(|error| format!("parse check JSON: {error}"))?;
    let outcome = &value["analysis_outcome"]["outcome"];
    let kind = outcome["kind"].as_str().unwrap_or_default().to_string();
    let limitation = outcome["limitations"]
        .as_array()
        .and_then(|limitations| {
            limitations
                .iter()
                .find(|limitation| limitation["kind"] == "language_scope_unsupported")
        })
        .cloned()
        .ok_or_else(|| format!("missing language_scope_unsupported limitation: {json}"))?;
    Ok((kind, limitation))
}

#[cfg(feature = "lang-typescript")]
#[test]
fn excluded_only_typescript_diff_discloses_skipped_scope_in_human_and_json() -> Result<(), String> {
    let (root, output) =
        typescript_output_for_paths("preview-typescript-excluded-only", &["vendor/lib.ts"])?;
    let proof = (|| -> Result<(), String> {
        let json = crate::render_check(&output, &crate::OutputFormat::Json)?;
        let (kind, limitation) = skipped_scope_limitation(&json)?;
        if kind != "partial_with_limitations"
            || limitation["producer_stage"] != "language_adapter"
            || limitation["affected_items"] != 1
            || !limitation["recovery"]["detail"]
                .as_str()
                .is_some_and(|detail| detail.contains("vendor/lib.ts"))
        {
            return Err(format!("excluded file not disclosed as skipped: {json}"));
        }
        let human = crate::render_check(&output, &crate::OutputFormat::Human)?;
        if !human.contains("(analysis incomplete; partial_with_limitations)")
            || !human.contains("Limitation: some changed files were not analyzed during language analysis (language_scope_unsupported at language_adapter)")
            || !human.contains("vendor/lib.ts")
            || human.contains("analyzed under preview support")
        {
            return Err(format!("human output hides the skipped file: {human}"));
        }
        Ok(())
    })();
    let cleanup =
        fs::remove_dir_all(&root).map_err(|error| format!("remove {}: {error}", root.display()));
    proof?;
    cleanup
}

#[cfg(feature = "lang-typescript")]
#[test]
fn mixed_typescript_diff_counts_only_accepted_files_and_discloses_the_skipped() -> Result<(), String>
{
    let (root, output) = typescript_output_for_paths(
        "preview-typescript-mixed-excluded",
        &["src/discount.ts", "vendor/lib.ts"],
    )?;
    let proof = (|| -> Result<(), String> {
        let json = crate::render_check(&output, &crate::OutputFormat::Json)?;
        let preview = preview_entry(&json, "check JSON")?;
        if preview["file_count"] != 1 || preview["sample_paths"][0] != "src/discount.ts" {
            return Err(format!(
                "analyzed count includes the excluded file: {preview}"
            ));
        }
        let (_, limitation) = skipped_scope_limitation(&json)?;
        if limitation["affected_items"] != 1 {
            return Err(format!("skipped count wrong: {limitation}"));
        }
        let human = crate::render_check(&output, &crate::OutputFormat::Human)?;
        if !human.contains("Note: 1 TypeScript file analyzed under preview support") {
            return Err(format!("control file lost its preview note: {human}"));
        }
        Ok(())
    })();
    let cleanup =
        fs::remove_dir_all(&root).map_err(|error| format!("remove {}: {error}", root.display()));
    proof?;
    cleanup
}

/// Real-repo finding (bat): a Rust-only change was reported as partial
/// because an unrelated, deeply nested Python fixture exceeded the Python
/// parse budget. A refusal in a language the diff does not touch must not
/// degrade the outcome; the same refusal must still surface when the diff
/// does touch Python.
#[cfg(feature = "lang-python")]
fn limitation_kinds_for_diff(name: &str, changed: &[&str]) -> Result<Vec<String>, String> {
    let root = temp_root(name)?;
    fs::create_dir_all(root.join("src")).map_err(|error| format!("create src: {error}"))?;
    write(
        &root.join("src/lib.rs"),
        "pub fn discount(amount: u32) -> u32 {\n    if amount > 10 { amount - 1 } else { amount }\n}\n",
    )?;
    let nested = format!("x = {}1{}\n", "(".repeat(200), ")".repeat(200));
    write(&root.join("fixture.py"), &nested)?;
    let diff = root.join("change.diff");
    let text = changed
        .iter()
        .map(|path| {
            let body = if path.ends_with(".rs") {
                "+pub fn discount(amount: u32) -> u32 {\n"
            } else {
                "+x = 1\n"
            };
            format!(
                "diff --git a/{path} b/{path}\n--- /dev/null\n+++ b/{path}\n@@ -0,0 +1 @@\n{body}"
            )
        })
        .collect::<String>();
    write(&diff, &text)?;
    let config =
        crate::config::tests_only_parse("[languages]\nenabled = [\"rust\", \"python\"]\n")?;
    let output = crate::app::check_workspace_with_config(
        crate::CheckInput {
            root,
            base: None,
            diff_file: Some(diff),
            mode: crate::Mode::Draft,
            format: crate::OutputFormat::Json,
            include_unchanged_tests: false,
            perl_facts_path: None,
            suppression_policy: None,
            git_timeout: None,
            git_candidate: None,
        },
        &config,
    )?;
    let json = crate::render_check(&output, &crate::OutputFormat::Json)?;
    let value: Value = serde_json::from_str(&json).map_err(|error| format!("parse: {error}"))?;
    let outcome = value
        .get("analysis_outcome")
        .ok_or_else(|| format!("missing analysis_outcome: {json}"))?;
    let limitations = outcome
        .pointer("/outcome/limitations")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("missing limitations: {outcome}"))?;
    Ok(limitations
        .iter()
        .filter_map(|entry| entry.get("kind").and_then(Value::as_str).map(str::to_owned))
        .collect())
}

#[cfg(feature = "lang-python")]
#[test]
fn unrelated_python_refusal_does_not_degrade_a_rust_only_diff() -> Result<(), String> {
    let rust_only = limitation_kinds_for_diff("python-refusal-rust-only", &["src/lib.rs"])?;
    assert!(
        rust_only.is_empty(),
        "a Python file the diff does not touch must not add limitations: {rust_only:?}"
    );
    let touches_python =
        limitation_kinds_for_diff("python-refusal-touched", &["src/lib.rs", "fixture.py"])?;
    assert!(
        touches_python
            .iter()
            .any(|kind| kind == "language_scope_unsupported"),
        "the refusal must remain when the diff touches Python: {touches_python:?}"
    );
    Ok(())
}
