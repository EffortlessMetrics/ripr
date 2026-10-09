//! `cargo xtask lsp-performance-report` — identity-bound saved-edit sequence
//! receipt for #1578.
//!
//! Discriminators live in the `ripr` test harness. This command runs that
//! sequence, overlays source/binary identity, and writes JSON/Markdown. The
//! historical 2s/10s/30s figures remain **proposals**. Existing rust tests
//! absorb the harness; this command is not a CI gate and does not add a
//! full-workspace job.

use crate::run::{capture_output_with_timeout, run_output};
use serde_json::{Value, json};
use std::fs;
use std::time::Duration;

const SCHEMA_VERSION: &str = "ripr-lsp-saved-edit-sequence-v1";
const REPORT_ENV: &str = "RIPR_LSP_PERFORMANCE_REPORT";
const HARNESS_FILTER: &str = "saved_edit_sequence";
const DEFAULT_TIMEOUT: Duration = Duration::from_mins(3);
const PROPOSED_ENVELOPE_NAMES: [&str; 3] = [
    "warm_bounded_save_p95_ms",
    "cold_small_project_ms",
    "warm_pr_sized_ms",
];

pub(crate) fn lsp_performance_report() -> Result<(), String> {
    let timeout = timeout_from_env();
    let output = capture_output_with_timeout(
        "cargo",
        &[
            "test".to_string(),
            "-p".to_string(),
            "ripr".to_string(),
            "--lib".to_string(),
            HARNESS_FILTER.to_string(),
            "--".to_string(),
            "--test-threads".to_string(),
            "1".to_string(),
        ],
        &[(REPORT_ENV, "1")],
        timeout,
        "lsp saved-edit sequence harness",
    )?;
    if output.timed_out {
        return Err(format!(
            "lsp-performance-report: sequence harness timed out after {} ms",
            timeout.as_millis()
        ));
    }
    let success = output.status.is_some_and(|status| status.success());
    if !success {
        return Err(format!(
            "lsp-performance-report: sequence harness failed\n{}",
            truncated_output(&output.stderr, &output.stdout)
        ));
    }

    let reports = crate::reports_dir();
    let json_path = reports.join("lsp-performance.json");
    if !json_path.is_file() {
        return Err(format!(
            "lsp-performance-report: harness did not write {}",
            json_path.display()
        ));
    }
    let raw = fs::read_to_string(&json_path)
        .map_err(|err| format!("read {}: {err}", json_path.display()))?;
    let mut receipt: Value = serde_json::from_str(&raw)
        .map_err(|err| format!("parse {}: {err}", json_path.display()))?;
    overlay_identity(&mut receipt)?;
    reject_promoted_proposals(&receipt)?;

    let json_text = serde_json::to_string_pretty(&receipt)
        .map_err(|err| format!("serialize lsp-performance receipt: {err}"))?;
    crate::write_report("lsp-performance.json", &format!("{json_text}\n"))?;
    crate::write_report("lsp-performance.md", &markdown_from_json(&receipt)?)?;
    println!("Wrote target/ripr/reports/lsp-performance.json");
    println!("Wrote target/ripr/reports/lsp-performance.md");
    Ok(())
}

fn timeout_from_env() -> Duration {
    std::env::var("RIPR_LSP_PERFORMANCE_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_millis)
        .unwrap_or(DEFAULT_TIMEOUT)
}

pub(crate) fn overlay_identity(receipt: &mut Value) -> Result<(), String> {
    let schema = receipt
        .get("schema_version")
        .and_then(Value::as_str)
        .unwrap_or("");
    if schema != SCHEMA_VERSION {
        return Err(format!(
            "lsp-performance-report: expected schema `{SCHEMA_VERSION}`, got `{schema}`"
        ));
    }
    let Some(identity) = receipt.get_mut("identity").and_then(Value::as_object_mut) else {
        return Err("lsp-performance-report: receipt is missing identity".to_string());
    };
    identity.insert("source_sha".to_string(), json!(git_revision()));
    // The sequence is a `ripr --lib` test harness, not a `target/debug/ripr`
    // process. Do not overlay an unexercised CLI binary or a null digest.
    identity.insert(
        "binary_path".to_string(),
        json!("ripr --lib saved_edit_sequence"),
    );
    identity.insert("binary_digest".to_string(), json!("not_measured"));
    identity.insert("host_class".to_string(), json!(host_class()));
    Ok(())
}

pub(crate) fn reject_promoted_proposals(receipt: &Value) -> Result<(), String> {
    let Some(envelopes) = receipt.get("proposed_envelopes").and_then(Value::as_array) else {
        return Err("lsp-performance-report: receipt is missing proposed_envelopes".to_string());
    };
    for envelope in envelopes {
        let name = envelope.get("name").and_then(Value::as_str).unwrap_or("");
        let class = envelope.get("class").and_then(Value::as_str).unwrap_or("");
        if PROPOSED_ENVELOPE_NAMES.contains(&name) && matches!(class, "gating" | "achieved") {
            return Err(format!(
                "lsp-performance-report: envelope `{name}` remains a proposal, not `{class}`"
            ));
        }
    }
    Ok(())
}

fn markdown_from_json(receipt: &Value) -> Result<String, String> {
    let identity = receipt
        .get("identity")
        .ok_or_else(|| "lsp-performance-report: missing identity for markdown".to_string())?;
    let steps = receipt
        .get("steps")
        .and_then(Value::as_array)
        .ok_or_else(|| "lsp-performance-report: missing steps for markdown".to_string())?;
    let envelopes = receipt
        .get("proposed_envelopes")
        .and_then(Value::as_array)
        .ok_or_else(|| "lsp-performance-report: missing envelopes for markdown".to_string())?;
    let mut body = String::from("# LSP saved-edit sequence\n\n");
    body.push_str(&format!(
        "- schema: `{SCHEMA_VERSION}`\n- binary: `{}`\n- source sha: `{}`\n- host class: `{}`\n- sample count: {}\n- cache reset: {}\n- optimization verdict: `{}`\n\n",
        json_str(identity, "binary_path"),
        json_str(identity, "source_sha"),
        json_str(identity, "host_class"),
        identity
            .get("sample_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        json_str(identity, "cache_reset_procedure"),
        receipt
            .get("optimization_verdict")
            .and_then(Value::as_str)
            .unwrap_or("not_established"),
    ));
    body.push_str("## Proposed envelopes (not gates)\n\n");
    for envelope in envelopes {
        body.push_str(&format!(
            "- `{}`: {} ms ({})\n",
            json_str(envelope, "name"),
            envelope
                .get("proposed_ms")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            json_str(envelope, "class")
        ));
    }
    body.push_str("\n## Steps\n\n");
    body.push_str(
        "| Step | Scope | Run status | Cache | Analyses Δ | Published bytes | Full rescan |\n",
    );
    body.push_str("| --- | --- | --- | --- | --- | --- | --- |\n");
    for step in steps {
        body.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            json_str(step, "step"),
            json_str(step, "semantic_scope"),
            json_str(step, "run_status"),
            json_str(step, "cache_load_status"),
            step.get("analyses_started_delta")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            step.get("published_payload_bytes")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            step.get("full_rescan")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        ));
    }
    body.push_str(
        "\nElapsed times, when present, are observations. They are not pass/fail gates.\n",
    );
    body.push_str(
        "\nA stale cached answer cannot satisfy a speed target. A fast elapsed time cannot hide a redundant full rescan or duplicate diagnostic publication.\n",
    );
    Ok(body)
}

fn json_str(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("not_measured")
        .to_string()
}

fn git_revision() -> String {
    run_output("git", &["rev-parse", "HEAD"])
        .ok()
        .map(|output| output.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unavailable".to_string())
}

fn host_class() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

fn truncated_output(stderr: &str, stdout: &str) -> String {
    let combined = if stderr.trim().is_empty() {
        stdout
    } else {
        stderr
    };
    const LIMIT: usize = 4_000;
    if combined.len() <= LIMIT {
        combined.to_string()
    } else {
        combined[combined.len() - LIMIT..].to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_receipt() -> Value {
        json!({
            "schema_version": SCHEMA_VERSION,
            "tool": "ripr",
            "report": "lsp-performance",
            "identity": {
                "source_sha": Value::Null,
                "binary_path": "target/debug/ripr",
                "binary_digest": Value::Null,
                "host_class": "linux-x86_64",
                "features": ["lang-rust"],
                "cache_reset_procedure": "isolated fixture",
                "sample_count": 1
            },
            "proposed_envelopes": [
                {"name": "warm_bounded_save_p95_ms", "proposed_ms": 2000, "class": "proposal"},
                {"name": "cold_small_project_ms", "proposed_ms": 10000, "class": "proposal"},
                {"name": "warm_pr_sized_ms", "proposed_ms": 30000, "class": "proposal"}
            ],
            "steps": [
                {
                    "step": "cold_start",
                    "semantic_scope": "interactive",
                    "run_status": "seams_deferred",
                    "cache_load_status": "miss",
                    "analyses_started_delta": 1,
                    "published_payload_bytes": 8,
                    "full_rescan": false
                }
            ],
            "optimization_verdict": "no_change"
        })
    }

    #[test]
    fn overlay_identity_binds_source_sha_and_binary_path() -> Result<(), String> {
        let mut receipt = sample_receipt();
        overlay_identity(&mut receipt)?;
        let identity = receipt
            .get("identity")
            .ok_or_else(|| "missing identity".to_string())?;
        let sha = identity
            .get("source_sha")
            .and_then(Value::as_str)
            .unwrap_or("");
        if sha.is_empty() || sha == "null" {
            return Err(format!("expected a source sha, got {sha:?}"));
        }
        let binary = identity
            .get("binary_path")
            .and_then(Value::as_str)
            .unwrap_or("");
        if binary != "ripr --lib saved_edit_sequence" {
            return Err(format!(
                "binary_path must name the lib harness, got {binary:?}"
            ));
        }
        let digest = identity.get("binary_digest");
        if digest != Some(&json!("not_measured")) {
            return Err(format!(
                "lib harness digest must stay not_measured, got {digest:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn overlay_does_not_claim_an_unexercised_debug_binary() -> Result<(), String> {
        let mut receipt = sample_receipt();
        overlay_identity(&mut receipt)?;
        let identity = receipt
            .get("identity")
            .ok_or_else(|| "missing identity".to_string())?;
        let binary = identity
            .get("binary_path")
            .and_then(Value::as_str)
            .unwrap_or("");
        if binary.contains("target/debug/ripr") {
            return Err(format!(
                "must not overlay unexercised debug binary {binary}"
            ));
        }
        if identity.get("binary_digest") == Some(&Value::Null) {
            return Err("must not publish a null digest".to_string());
        }
        if identity
            .get("binary_digest")
            .and_then(Value::as_str)
            .is_some_and(|digest| digest.starts_with("sha256:"))
        {
            return Err("must not hash an unexercised CLI binary".to_string());
        }
        Ok(())
    }

    #[test]
    fn reject_promoted_proposals_keeps_historical_figures_as_proposals() -> Result<(), String> {
        let receipt = sample_receipt();
        reject_promoted_proposals(&receipt)?;
        let mut gating = sample_receipt();
        gating["proposed_envelopes"][0]["class"] = json!("gating");
        match reject_promoted_proposals(&gating) {
            Err(message) if message.contains("proposal") => Ok(()),
            other => Err(format!("expected gating rejection, got {other:?}")),
        }
    }

    #[test]
    fn markdown_names_proposals_and_refuses_speed_target_language() -> Result<(), String> {
        let markdown = markdown_from_json(&sample_receipt())?;
        if !markdown.contains("not gates") {
            return Err("markdown must label envelopes as not gates".to_string());
        }
        if markdown.contains("achieved") || markdown.contains("p95 gate") {
            return Err("markdown must not claim an achieved latency gate".to_string());
        }
        if !markdown.contains("stale cached answer cannot satisfy a speed target") {
            return Err("markdown must keep the stale-cache honesty line".to_string());
        }
        Ok(())
    }

    #[test]
    fn overlay_rejects_wrong_schema() -> Result<(), String> {
        let mut receipt = sample_receipt();
        receipt["schema_version"] = json!("not-this-schema");
        match overlay_identity(&mut receipt) {
            Err(err) if err.contains(SCHEMA_VERSION) => Ok(()),
            other => Err(format!("expected schema rejection, got {other:?}")),
        }
    }
}
