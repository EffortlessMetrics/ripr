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
