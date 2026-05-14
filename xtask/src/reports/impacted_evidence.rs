use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_PR_EVIDENCE_JSON: &str = "target/ripr/pr/repo-exposure.json";
const IMPACTED_JSON: &str = "target/xtask/impacted-evidence/latest.json";
const IMPACTED_MD: &str = "target/xtask/impacted-evidence/latest.md";

#[derive(Clone, Debug, Eq, PartialEq)]
struct ImpactedEvidenceOptions {
    pr_evidence: String,
    labels: Vec<String>,
    check: bool,
}

impl Default for ImpactedEvidenceOptions {
    fn default() -> Self {
        Self {
            pr_evidence: DEFAULT_PR_EVIDENCE_JSON.to_string(),
            labels: labels_from_env(),
            check: false,
        }
    }
}

pub(crate) fn impacted_evidence(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        return Ok(());
    }
    let options = parse_options(args)?;
    let repo = repo_root()?;
    let packet = impacted_evidence_packet(&repo, &options);
    let json_text = serde_json::to_string_pretty(&packet)
        .map_err(|err| format!("serialize impacted evidence: {err}"))?;
    let markdown = render_impacted_evidence_markdown(&packet);
    if options.check {
        check_outputs(&repo, &json_text, &markdown)
    } else {
        write_outputs(&repo, &json_text, &markdown)
    }
}

fn parse_options(args: &[String]) -> Result<ImpactedEvidenceOptions, String> {
    let mut options = ImpactedEvidenceOptions::default();
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--pr-evidence" => {
                i += 1;
                options.pr_evidence = non_empty_arg(args, i, "--pr-evidence")?.to_string();
            }
            "--label" => {
                i += 1;
                options
                    .labels
                    .push(non_empty_arg(args, i, "--label")?.to_string());
            }
            "--labels" => {
                i += 1;
                options
                    .labels
                    .extend(split_labels(non_empty_arg(args, i, "--labels")?));
            }
            "--check" => options.check = true,
            other => return Err(format!("unknown impacted-evidence argument {other:?}")),
        }
        i += 1;
    }
    options.labels = normalize_labels(&options.labels);
    Ok(options)
}

fn non_empty_arg<'a>(args: &'a [String], index: usize, flag: &str) -> Result<&'a str, String> {
    let Some(value) = args.get(index) else {
        return Err(format!("missing value for {flag}"));
    };
    if value.trim().is_empty() {
        return Err(format!(
            "impacted-evidence {flag} requires a non-empty value"
        ));
    }
    Ok(value)
}

fn print_help() {
    println!(
        "usage: cargo xtask impacted-evidence [--pr-evidence <path>] [--label <label>] [--labels <csv>] [--check]"
    );
}

fn impacted_evidence_packet(repo: &Path, options: &ImpactedEvidenceOptions) -> Value {
    let input = load_pr_evidence(repo, &options.pr_evidence);
    let ripr_severe_gap = input
        .value
        .as_ref()
        .and_then(|value| value.pointer("/summary/ripr_severe_gap"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let pr_requires_targeted = input
        .value
        .as_ref()
        .and_then(|value| value.pointer("/summary/requires_targeted_mutation"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let decision = routing_decision(&options.labels, ripr_severe_gap || pr_requires_targeted);
    let warnings = input.warning(&options.pr_evidence);

    json!({
        "schema_version": "0.1",
        "tool": "ripr",
        "kind": "impacted_evidence",
        "scope": "diff",
        "status": if warnings.is_empty() { "advisory" } else { "incomplete" },
        "inputs": {
            "pr_evidence": options.pr_evidence,
            "labels": options.labels
        },
        "summary": {
            "mutation_mode": decision.mode,
            "requires_targeted_mutation": decision.requires_targeted_mutation,
            "requires_full_owner_mutation": decision.requires_full_owner_mutation,
            "ripr_severe_gap": ripr_severe_gap,
            "routing_reason": decision.reason
        },
        "artifacts": [
            {
                "label": "impacted evidence JSON",
                "path": IMPACTED_JSON,
                "kind": "json",
                "scope": "diff",
                "available": true,
                "required": true
            },
            {
                "label": "impacted evidence Markdown",
                "path": IMPACTED_MD,
                "kind": "markdown",
                "scope": "diff",
                "available": true
            },
            {
                "label": "PR evidence JSON",
                "path": options.pr_evidence,
                "kind": "json",
                "scope": "diff",
                "available": input.value.is_some()
            }
        ],
        "warnings": warnings,
        "advisory_limits": [
            "Impacted evidence routes mutation; it does not execute mutation.",
            "Full-owner mutation requires an explicit mutation/full-owner label."
        ]
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RoutingDecision {
    mode: &'static str,
    requires_targeted_mutation: bool,
    requires_full_owner_mutation: bool,
    reason: Value,
}

fn routing_decision(labels: &[String], ripr_routes_targeted: bool) -> RoutingDecision {
    let has = |needle: &str| labels.iter().any(|label| label == needle);
    if has("mutation/full-owner") {
        return RoutingDecision {
            mode: "full_owner",
            requires_targeted_mutation: false,
            requires_full_owner_mutation: true,
            reason: json!("mutation/full-owner label"),
        };
    }
    if has("mutation") || has("mutation/targeted") {
        return targeted("mutation label");
    }
    if has("release-risk") {
        return targeted("release-risk label");
    }
    if ripr_routes_targeted {
        return targeted("ripr severe gap");
    }
    RoutingDecision {
        mode: "fast_only",
        requires_targeted_mutation: false,
        requires_full_owner_mutation: false,
        reason: Value::Null,
    }
}

fn targeted(reason: &'static str) -> RoutingDecision {
    RoutingDecision {
        mode: "targeted",
        requires_targeted_mutation: true,
        requires_full_owner_mutation: false,
        reason: json!(reason),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PrEvidenceInput {
    value: Option<Value>,
    state: InputState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum InputState {
    Present,
    Missing,
    Invalid(String),
}

impl PrEvidenceInput {
    fn warning(&self, path: &str) -> Vec<Value> {
        match &self.state {
            InputState::Present => Vec::new(),
            InputState::Missing => vec![json!({
                "kind": "missing_artifact",
                "message": "PR evidence JSON is missing; mutation routing uses labels only.",
                "path": path
            })],
            InputState::Invalid(err) => vec![json!({
                "kind": "invalid_json",
                "message": format!("PR evidence JSON is invalid: {err}"),
                "path": path
            })],
        }
    }
}

fn load_pr_evidence(repo: &Path, relative: &str) -> PrEvidenceInput {
    let path = repo.join(relative);
    let Ok(text) = fs::read_to_string(&path) else {
        return PrEvidenceInput {
            value: None,
            state: InputState::Missing,
        };
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(value) => PrEvidenceInput {
            value: Some(value),
            state: InputState::Present,
        },
        Err(err) => PrEvidenceInput {
            value: None,
            state: InputState::Invalid(first_line(&err.to_string())),
        },
    }
}

fn render_impacted_evidence_markdown(packet: &Value) -> String {
    let summary = packet.get("summary").and_then(Value::as_object);
    let inputs = packet.get("inputs").and_then(Value::as_object);
    let labels = inputs
        .and_then(|inputs| inputs.get("labels"))
        .and_then(Value::as_array)
        .map(|labels| {
            labels
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|labels| !labels.is_empty())
        .unwrap_or_else(|| "none".to_string());

    let mut out = String::new();
    out.push_str("# Impacted Evidence\n\n");
    out.push_str("## Routing\n\n");
    out.push_str(&format!(
        "- mutation_mode: `{}`\n",
        summary_string(summary, "mutation_mode", "unknown")
    ));
    out.push_str(&format!(
        "- requires_targeted_mutation: {}\n",
        summary_bool(summary, "requires_targeted_mutation")
    ));
    out.push_str(&format!(
        "- requires_full_owner_mutation: {}\n",
        summary_bool(summary, "requires_full_owner_mutation")
    ));
    out.push_str(&format!(
        "- ripr_severe_gap: {}\n",
        summary_bool(summary, "ripr_severe_gap")
    ));
    out.push_str(&format!(
        "- routing_reason: `{}`\n\n",
        summary_string_or_null(summary, "routing_reason")
    ));

    out.push_str("## Inputs\n\n");
    out.push_str(&format!(
        "- PR evidence: `{}`\n",
        inputs
            .and_then(|inputs| inputs.get("pr_evidence"))
            .and_then(Value::as_str)
            .map(md_escape)
            .unwrap_or_else(|| "not_available".to_string())
    ));
    out.push_str(&format!("- labels: `{}`\n\n", md_escape(&labels)));

    out.push_str("## Artifacts\n\n");
    out.push_str("| Artifact | Path | Available |\n");
    out.push_str("| --- | --- | --- |\n");
    if let Some(artifacts) = packet.get("artifacts").and_then(Value::as_array) {
        for artifact in artifacts {
            out.push_str(&format!(
                "| {} | `{}` | {} |\n",
                md_escape(string_field(artifact, "label", "artifact")),
                md_escape(string_field(artifact, "path", "unknown")),
                artifact
                    .get("available")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            ));
        }
    }

    if let Some(warnings) = packet.get("warnings").and_then(Value::as_array)
        && !warnings.is_empty()
    {
        out.push_str("\n## Warnings\n\n");
        for warning in warnings {
            out.push_str(&format!(
                "- {}: {}\n",
                md_escape(string_field(warning, "kind", "warning")),
                md_escape(string_field(warning, "message", "unknown warning"))
            ));
        }
    }

    out.push_str("\n_This receipt routes verification work. It does not execute mutation._\n");
    out
}

fn summary_string(
    summary: Option<&serde_json::Map<String, Value>>,
    key: &str,
    fallback: &str,
) -> String {
    summary
        .and_then(|summary| summary.get(key))
        .and_then(Value::as_str)
        .map(md_escape)
        .unwrap_or_else(|| fallback.to_string())
}

fn summary_bool(summary: Option<&serde_json::Map<String, Value>>, key: &str) -> String {
    summary
        .and_then(|summary| summary.get(key))
        .and_then(Value::as_bool)
        .map(|value| value.to_string())
        .unwrap_or_else(|| "not_available".to_string())
}

fn summary_string_or_null(summary: Option<&serde_json::Map<String, Value>>, key: &str) -> String {
    let Some(value) = summary.and_then(|summary| summary.get(key)) else {
        return "not_available".to_string();
    };
    if value.is_null() {
        "none".to_string()
    } else {
        value
            .as_str()
            .map(md_escape)
            .unwrap_or_else(|| "invalid".to_string())
    }
}

fn string_field<'a>(value: &'a Value, key: &str, fallback: &'a str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or(fallback)
}

fn check_outputs(repo: &Path, json_text: &str, markdown: &str) -> Result<(), String> {
    let json_path = repo.join(IMPACTED_JSON);
    let md_path = repo.join(IMPACTED_MD);
    let actual_json = fs::read_to_string(&json_path)
        .map_err(|err| format!("missing or unreadable {IMPACTED_JSON}: {err}"))?;
    let actual_md = fs::read_to_string(&md_path)
        .map_err(|err| format!("missing or unreadable {IMPACTED_MD}: {err}"))?;
    if actual_json == format!("{json_text}\n") && actual_md == markdown {
        println!("Impacted evidence contract ok: {IMPACTED_JSON}");
        Ok(())
    } else {
        Err("impacted evidence is stale; run `cargo xtask impacted-evidence`".to_string())
    }
}

fn write_outputs(repo: &Path, json_text: &str, markdown: &str) -> Result<(), String> {
    let json_path = repo.join(IMPACTED_JSON);
    let md_path = repo.join(IMPACTED_MD);
    fs::create_dir_all(
        json_path
            .parent()
            .ok_or_else(|| "impacted evidence JSON path has no parent".to_string())?,
    )
    .map_err(|err| format!("create impacted evidence dir: {err}"))?;
    fs::write(&json_path, format!("{json_text}\n"))
        .map_err(|err| format!("failed to write {IMPACTED_JSON}: {err}"))?;
    fs::write(&md_path, markdown).map_err(|err| format!("failed to write {IMPACTED_MD}: {err}"))?;
    println!("Wrote {IMPACTED_JSON}");
    println!("Wrote {IMPACTED_MD}");
    Ok(())
}

fn labels_from_env() -> Vec<String> {
    env::var("GITHUB_PR_LABELS")
        .or_else(|_| env::var("PR_LABELS"))
        .map(|labels| normalize_labels(&split_labels(&labels)))
        .unwrap_or_default()
}

fn split_labels(labels: &str) -> Vec<String> {
    labels
        .split([',', '\n', ';'])
        .map(str::trim)
        .filter(|label| !label.is_empty())
        .map(str::to_string)
        .collect()
}

fn normalize_labels(labels: &[String]) -> Vec<String> {
    labels
        .iter()
        .map(|label| label.trim().to_ascii_lowercase())
        .filter(|label| !label.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn md_escape(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

fn first_line(value: &str) -> String {
    value.lines().next().unwrap_or(value).trim().to_string()
}

fn repo_root() -> Result<PathBuf, String> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.parent().map(Path::to_path_buf).ok_or_else(|| {
        format!(
            "failed to resolve repo root from {}",
            manifest_dir.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_accepts_labels_and_check() -> Result<(), String> {
        let parsed = parse_options(&[
            "--label".to_string(),
            "release-risk".to_string(),
            "--labels".to_string(),
            "mutation, docs".to_string(),
            "--check".to_string(),
        ])?;
        assert_eq!(
            parsed.labels,
            vec![
                "docs".to_string(),
                "mutation".to_string(),
                "release-risk".to_string()
            ]
        );
        assert!(parsed.check);
        Ok(())
    }

    #[test]
    fn ripr_severe_gap_routes_targeted_mutation() -> Result<(), String> {
        let repo = temp_repo("impacted-ripr-route")?;
        write_pr_evidence(&repo, true, false)?;
        let packet = impacted_evidence_packet(&repo, &ImpactedEvidenceOptions::default());
        assert_eq!(packet["summary"]["mutation_mode"], "targeted");
        assert_eq!(packet["summary"]["requires_targeted_mutation"], true);
        assert_eq!(packet["summary"]["ripr_severe_gap"], true);
        assert_eq!(packet["summary"]["routing_reason"], "ripr severe gap");
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))
    }

    #[test]
    fn explicit_full_owner_label_routes_full_owner_only() -> Result<(), String> {
        let repo = temp_repo("impacted-full-owner")?;
        let options = ImpactedEvidenceOptions {
            labels: vec!["mutation/full-owner".to_string()],
            ..ImpactedEvidenceOptions::default()
        };
        let packet = impacted_evidence_packet(&repo, &options);
        assert_eq!(packet["summary"]["mutation_mode"], "full_owner");
        assert_eq!(packet["summary"]["requires_targeted_mutation"], false);
        assert_eq!(packet["summary"]["requires_full_owner_mutation"], true);
        assert_eq!(
            packet["summary"]["routing_reason"],
            "mutation/full-owner label"
        );
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))
    }

    #[test]
    fn missing_pr_evidence_is_explicit_and_label_only() -> Result<(), String> {
        let repo = temp_repo("impacted-missing")?;
        let packet = impacted_evidence_packet(&repo, &ImpactedEvidenceOptions::default());
        assert_eq!(packet["status"], "incomplete");
        assert_eq!(packet["summary"]["mutation_mode"], "fast_only");
        assert_eq!(packet["warnings"][0]["kind"], "missing_artifact");
        let markdown = render_impacted_evidence_markdown(&packet);
        assert!(markdown.contains("# Impacted Evidence"));
        assert!(markdown.contains("- mutation_mode: `fast_only`"));
        assert!(markdown.contains("PR evidence JSON is missing"));
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))
    }

    #[test]
    fn missing_custom_pr_evidence_reports_custom_path() -> Result<(), String> {
        let repo = temp_repo("impacted-custom-missing")?;
        let options = ImpactedEvidenceOptions {
            pr_evidence: "target/ripr/pr/custom-evidence.json".to_string(),
            ..ImpactedEvidenceOptions::default()
        };
        let packet = impacted_evidence_packet(&repo, &options);
        assert_eq!(
            packet["warnings"][0]["path"],
            "target/ripr/pr/custom-evidence.json"
        );
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))
    }

    #[test]
    fn check_rejects_stale_outputs() -> Result<(), String> {
        let repo = temp_repo("impacted-stale")?;
        write_pr_evidence(&repo, false, false)?;
        let packet = impacted_evidence_packet(&repo, &ImpactedEvidenceOptions::default());
        let json_text =
            serde_json::to_string_pretty(&packet).map_err(|err| format!("serialize: {err}"))?;
        let markdown = render_impacted_evidence_markdown(&packet);
        write_file(&repo, IMPACTED_JSON, "stale\n")?;
        write_file(&repo, IMPACTED_MD, &markdown)?;
        let err = match check_outputs(&repo, &json_text, &markdown) {
            Ok(()) => return Err("stale output should fail".to_string()),
            Err(err) => err,
        };
        assert!(err.contains("impacted evidence is stale"));
        fs::remove_dir_all(&repo).map_err(|err| format!("cleanup {}: {err}", repo.display()))
    }

    fn write_pr_evidence(
        repo: &Path,
        ripr_severe_gap: bool,
        requires_targeted_mutation: bool,
    ) -> Result<(), String> {
        write_file(
            repo,
            DEFAULT_PR_EVIDENCE_JSON,
            &serde_json::to_string_pretty(&json!({
                "summary": {
                    "ripr_severe_gap": ripr_severe_gap,
                    "requires_targeted_mutation": requires_targeted_mutation
                }
            }))
            .map_err(|err| format!("serialize PR evidence: {err}"))?,
        )
    }

    fn temp_repo(name: &str) -> Result<PathBuf, String> {
        let unique = format!(
            "{}-{}-{}",
            name,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|err| format!("system clock before epoch: {err}"))?
                .as_nanos()
        );
        let path = env::temp_dir().join(unique);
        fs::create_dir_all(&path).map_err(|err| format!("create {}: {err}", path.display()))?;
        Ok(path)
    }

    fn write_file(repo: &Path, relative: &str, text: &str) -> Result<(), String> {
        let path = repo.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("create {}: {err}", parent.display()))?;
        }
        fs::write(&path, text).map_err(|err| format!("write {}: {err}", path.display()))
    }
}
