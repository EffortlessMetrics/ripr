//! Analysis and continuation oracles for the #4626 consumer journey.
//!
//! No-config Python preview must produce nonzero probes/findings. Explicit
//! rust-only config must be a typed limitation, not a false clean result.
//! Continuation must come from the product-emitted `ripr explain` line.

use serde_json::Value;

pub(crate) fn require_no_config_python_preview(report: &Value) -> Result<String, String> {
    let probes = json_u64(report, "/summary/probes")?;
    let findings = json_u64(report, "/summary/findings")?;
    if probes == 0 || findings == 0 {
        return Err(format!(
            "no-config Python journey produced probes={probes} findings={findings}; `--version` or a zero-subject run is not proof"
        ));
    }
    let rust_files = json_u64(report, "/summary/changed_rust_files").unwrap_or(0);
    if rust_files != 0 {
        return Err(format!(
            "Python fixture unexpectedly reported {rust_files} changed Rust files"
        ));
    }
    require_python_language_row(report, true, true)?;
    let finding = report
        .pointer("/findings/0")
        .ok_or_else(|| "no-config journey JSON has no findings[0]".to_string())?;
    let language = finding
        .get("language")
        .and_then(Value::as_str)
        .ok_or_else(|| "finding[0] missing language".to_string())?;
    if language != "python" {
        return Err(format!("finding[0] language is `{language}`, not python"));
    }
    let status = finding
        .get("language_status")
        .and_then(Value::as_str)
        .unwrap_or("");
    if status != "preview" {
        return Err(format!(
            "finding[0] language_status is `{status}`, not preview; this slice must not promote Python"
        ));
    }
    finding
        .get("id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| "finding[0] missing id".to_string())
}

pub(crate) fn require_explicit_rust_only_is_limited_not_clean(
    report: &Value,
) -> Result<(), String> {
    let probes = json_u64(report, "/summary/probes")?;
    let findings = json_u64(report, "/summary/findings")?;
    if probes != 0 || findings != 0 {
        return Err(format!(
            "explicit rust-only config still produced probes={probes} findings={findings}"
        ));
    }
    let complete = report
        .pointer("/analysis_outcome/analysis_complete")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    if complete {
        return Err(
            "explicit rust-only Python fixture reported analysis_complete=true; empty is not Rust-grade clean"
                .to_string(),
        );
    }
    let limitations = report
        .pointer("/analysis_outcome/outcome/limitations")
        .and_then(Value::as_array)
        .ok_or_else(|| "explicit-config report missing typed limitations".to_string())?;
    let has_unavailable = limitations.iter().any(|limitation| {
        limitation.get("kind").and_then(Value::as_str) == Some("language_adapter_unavailable")
    });
    if !has_unavailable {
        return Err(
            "explicit rust-only config did not record language_adapter_unavailable; a false clean result is forbidden"
                .to_string(),
        );
    }
    require_python_language_row(report, false, false)
}

pub(crate) fn extract_explain_continuation(human: &str) -> Result<String, String> {
    let mut found = None;
    for line in human.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("ripr explain ") else {
            continue;
        };
        if rest.trim().is_empty() {
            continue;
        }
        if found.is_some() {
            return Err("human output emitted multiple `ripr explain` continuations".to_string());
        }
        found = Some(trimmed.to_string());
    }
    found.ok_or_else(|| {
        "human output did not emit a `ripr explain` continuation; a hardcoded finding id is not product-emitted"
            .to_string()
    })
}

pub(crate) fn bind_continuation_to_finding(command: &str, finding_id: &str) -> Result<(), String> {
    let Some(rest) = command.strip_prefix("ripr explain ") else {
        return Err(format!(
            "continuation `{command}` is not an explain command"
        ));
    };
    let last = rest
        .split_whitespace()
        .next_back()
        .unwrap_or("")
        .trim_matches(|c| c == '\'' || c == '"');
    if last != finding_id {
        return Err(format!(
            "continuation `{command}` does not name finding `{finding_id}`"
        ));
    }
    Ok(())
}

pub(crate) fn require_identity_preserving_explain(command: &str) -> Result<(), String> {
    if !command.contains("--root ") || !command.contains("--diff ") {
        return Err(format!(
            "continuation `{command}` dropped --root/--diff; the two-token `ripr explain <id>` form is not what `ripr check` emits"
        ));
    }
    Ok(())
}

fn require_python_language_row(
    report: &Value,
    enabled: bool,
    analyzed: bool,
) -> Result<(), String> {
    let rows = report
        .get("preview_languages")
        .and_then(Value::as_array)
        .ok_or_else(|| "report missing preview_languages".to_string())?;
    let python = rows
        .iter()
        .find(|row| row.get("language").and_then(Value::as_str) == Some("python"))
        .ok_or_else(|| "report missing python preview_languages row".to_string())?;
    let actual_enabled = python
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let actual_analyzed = python
        .get("analyzed")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if actual_enabled != enabled || actual_analyzed != analyzed {
        return Err(format!(
            "python preview row enabled={actual_enabled} analyzed={actual_analyzed}, expected enabled={enabled} analyzed={analyzed}"
        ));
    }
    if enabled {
        let category = python.get("category").and_then(Value::as_str).unwrap_or("");
        if category != "preview_language_advisory" {
            return Err(format!(
                "python preview category is `{category}`, not preview_language_advisory"
            ));
        }
    }
    Ok(())
}

fn json_u64(value: &Value, pointer: &str) -> Result<u64, String> {
    value
        .pointer(pointer)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("JSON pointer {pointer} missing or not an integer"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::require_error;
    use serde_json::json;

    fn no_config_report() -> Value {
        json!({
            "summary": {
                "changed_rust_files": 0,
                "probes": 1,
                "findings": 1,
                "changed_files_by_language": [{"language": "python", "files": 1}]
            },
            "findings": [{
                "id": "probe:src_pricing.py:python_preview:aabbccdd",
                "language": "python",
                "language_status": "preview"
            }],
            "preview_languages": [{
                "language": "python",
                "enabled": true,
                "analyzed": true,
                "category": "preview_language_advisory"
            }]
        })
    }

    #[test]
    fn no_config_preview_requires_python_subjects_not_version() -> Result<(), String> {
        let id = require_no_config_python_preview(&no_config_report())?;
        if id != "probe:src_pricing.py:python_preview:aabbccdd" {
            return Err(format!("unexpected finding id `{id}`"));
        }

        let mut empty = no_config_report();
        empty["summary"]["probes"] = json!(0);
        empty["summary"]["findings"] = json!(0);
        let error = require_error(
            require_no_config_python_preview(&empty),
            "zero-subject report must fail",
        )?;
        if !error.contains("zero-subject") {
            return Err(format!("unexpected empty-report error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn explicit_rust_only_requires_typed_limitation_not_false_clean() -> Result<(), String> {
        let limited = json!({
            "summary": {"probes": 0, "findings": 0},
            "analysis_outcome": {
                "analysis_complete": false,
                "outcome": {
                    "limitations": [{"kind": "language_adapter_unavailable"}]
                }
            },
            "preview_languages": [{
                "language": "python",
                "enabled": false,
                "analyzed": false
            }]
        });
        require_explicit_rust_only_is_limited_not_clean(&limited)?;

        let false_clean = json!({
            "summary": {"probes": 0, "findings": 0},
            "analysis_outcome": {
                "analysis_complete": true,
                "outcome": {"limitations": []}
            },
            "preview_languages": [{
                "language": "python",
                "enabled": false,
                "analyzed": false
            }]
        });
        let error = require_error(
            require_explicit_rust_only_is_limited_not_clean(&false_clean),
            "false clean must fail",
        )?;
        if !error.contains("not Rust-grade clean") {
            return Err(format!("unexpected false-clean error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn continuation_must_be_the_product_explain_line() -> Result<(), String> {
        let finding = "probe:src_pricing.py:python_preview:aabbccdd";
        let product = format!(
            "ripr explain --root '/tmp/consumer ü/fixtures/python/basic' --diff '/tmp/consumer ü/fixtures/python/basic/diff.patch' --mode fast {finding}"
        );
        let human = format!(
            "Next: drill into the top finding:\n  {product}\n  ripr context --root '/tmp/consumer ü/fixtures/python/basic' --diff '/tmp/consumer ü/fixtures/python/basic/diff.patch' --mode fast --at {finding}\n"
        );
        let command = extract_explain_continuation(&human)?;
        if command != product {
            return Err(format!(
                "extractor returned `{command}`, not the product-emitted line"
            ));
        }
        bind_continuation_to_finding(&command, finding)?;
        require_identity_preserving_explain(&command)?;

        let missing = require_error(
            extract_explain_continuation("ripr 0.11.0\n"),
            "version text is not continuation",
        )?;
        if !missing.contains("hardcoded finding id is not product-emitted") {
            return Err(format!("unexpected missing-continuation error: {missing}"));
        }

        let two_token = format!("ripr explain {finding}");
        require_error(
            require_identity_preserving_explain(&two_token),
            "legacy two-token form must not close the check journey",
        )?;

        let mismatch = require_error(
            bind_continuation_to_finding("ripr explain probe:other", finding),
            "wrong id must fail",
        )?;
        if !mismatch.contains("does not name finding") {
            return Err(format!("unexpected mismatch error: {mismatch}"));
        }

        let id_only_in_root = format!(
            "ripr explain --root '/tmp/{finding}' --diff diff.patch --mode fast probe:other"
        );
        require_error(
            bind_continuation_to_finding(&id_only_in_root, finding),
            "finding id in a flag must not bind the wrong last token",
        )?;

        let context_only =
            format!("Next:\n  ripr context --root /tmp --diff /tmp/diff.patch --at {finding}\n");
        require_error(
            extract_explain_continuation(&context_only),
            "context is not an explain continuation",
        )?;

        let ambiguous = format!("{human}  ripr explain {finding}\n");
        let error = require_error(
            extract_explain_continuation(&ambiguous),
            "product plus two-token must not pick silently",
        )?;
        if !error.contains("multiple") {
            return Err(format!("unexpected ambiguous-continuation error: {error}"));
        }
        Ok(())
    }
}
