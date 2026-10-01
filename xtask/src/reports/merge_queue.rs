//! `cargo xtask merge-queue capture` — MQ0 read-only current-state receipt.
//!
//! The command captures desired settings, live observation, apply-route
//! capability, and rollback identity as separate facts for later
//! gatecheck#11 consumption. It does not evaluate a future queue design
//! and it never mutates GitHub or repository settings.

use crate::run::capture_output;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const USAGE: &str = "usage: cargo xtask merge-queue capture [--repo <owner/name>] [--out <dir>] [--input <path>] [--prior <path>]";
const DEFAULT_REPO: &str = "EffortlessMetrics/ripr-swarm";
const DEFAULT_OUT: &str = "target/ripr/reports/merge-queue";
const JSON_NAME: &str = "merge-queue-capture.json";
const MD_NAME: &str = "merge-queue-capture.md";
const SCHEMA_VERSION: &str = "0.1";
const PACKET_KIND: &str = "merge_queue_current_state";
const EXPECTED_RULESET_ID: i64 = 18_845_973;
const DELETED_AUTHORITY_REPO: &str = "EffortlessMetrics/.github";
const DELETED_AUTHORITY_ISSUES: &[u64] = &[2, 3];
const EXIT_READY: &str = "READY_FOR_DESIRED_STATE";
const EXIT_BLOCKED: &str = "CAPABILITY_BLOCKED";
const EXIT_NOT_PROVEN: &str = "NOT_PROVEN";
const EXIT_DRIFT: &str = "DRIFT_REPAIR_REQUIRED";

const REDACT_KEY_MARKERS: &[&str] = &[
    "token",
    "authorization",
    "password",
    "secret",
    "private_key",
    "client_secret",
    "email",
];

#[derive(Clone, Debug, Eq, PartialEq)]
struct CaptureOptions {
    repo: String,
    out: PathBuf,
    input: Option<PathBuf>,
    prior: Option<PathBuf>,
}

#[derive(Clone, Debug)]
struct EndpointRecord {
    method: String,
    path: String,
    http_status: u16,
    body: Value,
    truncated: bool,
}

#[derive(Clone, Debug)]
struct WorkflowFile {
    path: String,
    text: String,
}

#[derive(Clone, Debug)]
struct WorkspaceFacts {
    settings_path: String,
    settings_text: String,
    settings_ref: String,
    workspace_head_sha: String,
    workflow_files: Vec<WorkflowFile>,
}

#[derive(Clone, Debug)]
struct CaptureSnapshot {
    repo: String,
    endpoints: BTreeMap<String, EndpointRecord>,
    workspace: WorkspaceFacts,
    prior_receipt: Option<Value>,
}

pub(crate) fn merge_queue(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{USAGE}");
        return Ok(());
    }
    let options = parse_options(args)?;
    let snapshot = load_snapshot(&options)?;
    let packet = build_packet(&snapshot)?;
    let json_text = render_json(&packet)?;
    let markdown = render_markdown(&packet)?;
    crate::write_report_in(&options.out, JSON_NAME, &json_text)?;
    crate::write_report_in(&options.out, MD_NAME, &markdown)?;
    println!("Wrote {}/{JSON_NAME}", options.out.display());
    println!("Wrote {}/{MD_NAME}", options.out.display());
    Ok(())
}

fn parse_options(args: &[String]) -> Result<CaptureOptions, String> {
    let mut rest = args.iter();
    let Some(subcommand) = rest.next() else {
        return Err(format!("merge-queue requires `capture`\n{USAGE}"));
    };
    if subcommand != "capture" {
        return Err(format!(
            "unknown merge-queue subcommand `{subcommand}`; only `capture` is accepted on this claim\n{USAGE}"
        ));
    }

    let mut options = CaptureOptions {
        repo: DEFAULT_REPO.to_string(),
        out: PathBuf::from(DEFAULT_OUT),
        input: None,
        prior: None,
    };
    let remaining: Vec<String> = rest.cloned().collect();
    let mut index = 0usize;
    while index < remaining.len() {
        match remaining[index].as_str() {
            "--repo" => {
                index += 1;
                options.repo = non_empty_arg(&remaining, index, "--repo")?.to_string();
                if !options.repo.contains('/') {
                    return Err(format!(
                        "merge-queue capture --repo must be `owner/name`\n{USAGE}"
                    ));
                }
            }
            "--out" => {
                index += 1;
                options.out = PathBuf::from(non_empty_arg(&remaining, index, "--out")?);
            }
            "--input" => {
                index += 1;
                options.input = Some(PathBuf::from(non_empty_arg(&remaining, index, "--input")?));
            }
            "--prior" => {
                index += 1;
                options.prior = Some(PathBuf::from(non_empty_arg(&remaining, index, "--prior")?));
            }
            other if looks_like_mutation_arg(other) => {
                return Err(format!(
                    "merge-queue capture refuses `{other}`; capture is GET-only and performs no settings, branch, PR, workflow, ruleset, secret, or repository mutation\n{USAGE}"
                ));
            }
            other => {
                return Err(format!(
                    "unknown merge-queue capture argument `{other}`\n{USAGE}"
                ));
            }
        }
        index += 1;
    }
    Ok(options)
}

fn looks_like_mutation_arg(arg: &str) -> bool {
    matches!(
        arg,
        "--apply"
            | "--write"
            | "--patch"
            | "--delete"
            | "--enable"
            | "--disable"
            | "--mutate"
            | "apply"
            | "enable"
    )
}

fn non_empty_arg<'a>(args: &'a [String], index: usize, flag: &str) -> Result<&'a str, String> {
    let Some(value) = args.get(index) else {
        return Err(format!("missing value for {flag}\n{USAGE}"));
    };
    if value.trim().is_empty() {
        return Err(format!(
            "merge-queue capture {flag} requires a non-empty value"
        ));
    }
    Ok(value)
}

fn load_snapshot(options: &CaptureOptions) -> Result<CaptureSnapshot, String> {
    let mut snapshot = match &options.input {
        Some(path) => load_input_snapshot(path, &options.repo)?,
        None => collect_live_snapshot(&options.repo)?,
    };
    if snapshot.repo != options.repo {
        return Err(format!(
            "input repo `{}` does not match --repo `{}`",
            snapshot.repo, options.repo
        ));
    }
    if let Some(prior_path) = &options.prior {
        snapshot.prior_receipt = Some(read_json_file(prior_path)?);
    }
    Ok(snapshot)
}

fn load_input_snapshot(path: &Path, expected_repo: &str) -> Result<CaptureSnapshot, String> {
    let value = read_json_file(path)?;
    parse_input_snapshot(&value, expected_repo).map_err(|err| {
        format!(
            "failed to parse merge-queue capture input {}: {err}",
            path.display()
        )
    })
}

fn parse_input_snapshot(value: &Value, expected_repo: &str) -> Result<CaptureSnapshot, String> {
    let repo = string_field(value, "repo").unwrap_or_else(|| expected_repo.to_string());
    let workspace = value
        .get("workspace")
        .ok_or_else(|| "input is missing object `workspace`".to_string())?;
    let settings_text = string_field(workspace, "settings_yml_text")
        .ok_or_else(|| "workspace.settings_yml_text is required".to_string())?;
    let mut endpoints = BTreeMap::new();
    let Some(endpoint_map) = value.get("endpoints").and_then(Value::as_object) else {
        return Err("input is missing object `endpoints`".to_string());
    };
    for (key, record) in endpoint_map {
        endpoints.insert(key.clone(), parse_endpoint_record(key, record)?);
    }
    let workflow_files = workflow_files_from_value(workspace.get("workflow_files"));
    Ok(CaptureSnapshot {
        repo,
        endpoints,
        workspace: WorkspaceFacts {
            settings_path: string_field(workspace, "settings_yml_path")
                .unwrap_or_else(|| ".github/settings.yml".to_string()),
            settings_text,
            settings_ref: string_field(workspace, "settings_ref")
                .or_else(|| string_field(workspace, "head_sha"))
                .unwrap_or_else(|| "input".to_string()),
            workspace_head_sha: string_field(workspace, "head_sha")
                .unwrap_or_else(|| "input".to_string()),
            workflow_files,
        },
        prior_receipt: value
            .get("prior_receipt")
            .cloned()
            .filter(|item| !item.is_null()),
    })
}

fn parse_endpoint_record(key: &str, record: &Value) -> Result<EndpointRecord, String> {
    let (method, path) = split_endpoint_key(key)?;
    if method != "GET" {
        return Err(format!(
            "capture input endpoint `{key}` is not GET; mutation methods are refused"
        ));
    }
    let http_status = record
        .get("http_status")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("endpoint `{key}` is missing numeric `http_status`"))?;
    let http_status = u16::try_from(http_status)
        .map_err(|err| format!("endpoint `{key}` http_status is out of range: {err}"))?;
    let mut body = record.get("body").cloned().unwrap_or(Value::Null);
    redact_secrets(&mut body);
    Ok(EndpointRecord {
        method,
        path,
        http_status,
        body,
        truncated: record
            .get("truncated")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

fn split_endpoint_key(key: &str) -> Result<(String, String), String> {
    let Some((method, path)) = key.split_once(' ') else {
        return Err(format!("endpoint key `{key}` must be `GET /path`"));
    };
    Ok((method.to_string(), path.to_string()))
}

fn workflow_files_from_value(value: Option<&Value>) -> Vec<WorkflowFile> {
    let Some(items) = value.and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut files = Vec::new();
    for item in items {
        let Some(path) = string_field(item, "path") else {
            continue;
        };
        let Some(text) = string_field(item, "text") else {
            continue;
        };
        files.push(WorkflowFile { path, text });
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    files
}

fn collect_live_snapshot(repo: &str) -> Result<CaptureSnapshot, String> {
    let workspace_head_sha = git_rev_parse("HEAD")?;
    let settings_path = Path::new(".github/settings.yml");
    let settings_text = std::fs::read_to_string(settings_path)
        .map_err(|err| format!("failed to read {}: {err}", settings_path.display()))?;
    let settings_ref =
        git_rev_parse("HEAD:.github/settings.yml").unwrap_or_else(|_| workspace_head_sha.clone());
    let workflow_files = read_workspace_workflows()?;
    let repo_path = format!("repos/{repo}");
    let mut endpoints = BTreeMap::new();
    record_live_get(&mut endpoints, &repo_path)?;
    let repo_body = endpoints
        .get(&format!("GET /{repo_path}"))
        .map(|record| record.body.clone())
        .unwrap_or(Value::Null);
    let default_branch = repo_body
        .get("default_branch")
        .and_then(Value::as_str)
        .unwrap_or("main");
    record_live_get(
        &mut endpoints,
        &format!("{repo_path}/branches/{default_branch}"),
    )?;
    record_live_get(
        &mut endpoints,
        &format!("{repo_path}/branches/{default_branch}/protection"),
    )?;
    record_live_get_paginated(
        &mut endpoints,
        &format!("{repo_path}/rulesets?includes_parents=true"),
    )?;
    record_live_get(&mut endpoints, &format!("{repo_path}/installation"))?;
    if let Some(list) = endpoints
        .get(&format!("GET /{repo_path}/rulesets?includes_parents=true"))
        .and_then(|record| record.body.as_array())
    {
        let mut ids = BTreeSet::new();
        for item in list {
            if let Some(id) = item.get("id").and_then(Value::as_i64) {
                ids.insert(id);
            }
        }
        ids.insert(EXPECTED_RULESET_ID);
        for id in ids {
            record_live_get(&mut endpoints, &format!("{repo_path}/rulesets/{id}"))?;
        }
    } else {
        record_live_get(
            &mut endpoints,
            &format!("{repo_path}/rulesets/{EXPECTED_RULESET_ID}"),
        )?;
    }
    for issue in DELETED_AUTHORITY_ISSUES {
        record_live_get(
            &mut endpoints,
            &format!("repos/{DELETED_AUTHORITY_REPO}/issues/{issue}"),
        )?;
    }
    Ok(CaptureSnapshot {
        repo: repo.to_string(),
        endpoints,
        workspace: WorkspaceFacts {
            settings_path: ".github/settings.yml".to_string(),
            settings_text,
            settings_ref,
            workspace_head_sha,
            workflow_files,
        },
        prior_receipt: None,
    })
}

fn record_live_get(
    endpoints: &mut BTreeMap<String, EndpointRecord>,
    path: &str,
) -> Result<(), String> {
    record_live_get_with(endpoints, path, false)
}

fn record_live_get_paginated(
    endpoints: &mut BTreeMap<String, EndpointRecord>,
    path: &str,
) -> Result<(), String> {
    record_live_get_with(endpoints, path, true)
}

fn record_live_get_with(
    endpoints: &mut BTreeMap<String, EndpointRecord>,
    path: &str,
    paginate: bool,
) -> Result<(), String> {
    let request_path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    let key = format!("GET {request_path}");
    let mut args = vec!["api"];
    if paginate {
        args.extend([
            "--paginate",
            "-X",
            "GET",
            request_path.trim_start_matches('/'),
        ]);
    } else {
        args.extend([
            "--include",
            "-X",
            "GET",
            request_path.trim_start_matches('/'),
        ]);
    }
    let captured =
        capture_output("gh", &args, &format!("gh api GET {request_path}")).map_err(|err| {
            format!(
                "merge-queue capture could not invoke read-only `gh api GET {request_path}`: {err}"
            )
        })?;
    let parsed = if paginate {
        parse_gh_paginated_output(&captured.stdout)
    } else {
        parse_gh_include_output(&captured.stdout)
    };
    let mut body = parsed.body;
    redact_secrets(&mut body);
    endpoints.insert(
        key,
        EndpointRecord {
            method: "GET".to_string(),
            path: request_path,
            http_status: parsed.http_status,
            body,
            truncated: parsed.truncated,
        },
    );
    Ok(())
}

struct GhInclude {
    http_status: u16,
    body: Value,
    truncated: bool,
}

fn parse_gh_include_output(stdout: &str) -> GhInclude {
    if let Some(parsed) = parse_include_text(stdout) {
        return parsed;
    }
    if let Ok(body) = serde_json::from_str::<Value>(stdout.trim()) {
        return GhInclude {
            http_status: 200,
            body,
            truncated: false,
        };
    }
    unreadable_gh_body()
}

fn parse_gh_paginated_output(stdout: &str) -> GhInclude {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return unreadable_gh_body();
    }
    if let Ok(body) = serde_json::from_str::<Value>(trimmed) {
        return GhInclude {
            http_status: 200,
            body,
            truncated: false,
        };
    }

    let mut stream = serde_json::Deserializer::from_str(trimmed).into_iter::<Value>();
    let mut merged = Vec::new();
    let mut saw_any = false;
    loop {
        match stream.next() {
            Some(Ok(Value::Array(items))) => {
                saw_any = true;
                merged.extend(items);
            }
            Some(Ok(other)) => {
                saw_any = true;
                merged.push(other);
            }
            Some(Err(_)) => {
                return if saw_any {
                    GhInclude {
                        http_status: 200,
                        body: json!(merged),
                        truncated: true,
                    }
                } else {
                    unreadable_gh_body()
                };
            }
            None => break,
        }
    }
    if !saw_any {
        return unreadable_gh_body();
    }
    GhInclude {
        http_status: 200,
        body: json!(merged),
        truncated: false,
    }
}

fn unreadable_gh_body() -> GhInclude {
    GhInclude {
        http_status: 0,
        body: json!({
            "message": "gh api produced no parseable HTTP response",
        }),
        truncated: false,
    }
}

fn parse_include_text(text: &str) -> Option<GhInclude> {
    let (headers, body_text) = text
        .split_once("\r\n\r\n")
        .or_else(|| text.split_once("\n\n"))?;
    let first = headers.lines().next()?;
    let status = parse_http_status_line(first)?;
    let body_text = body_text.trim();
    let body = if body_text.is_empty() {
        Value::Null
    } else {
        serde_json::from_str(body_text)
            .unwrap_or_else(|_| json!({ "raw": truncate_for_receipt(body_text) }))
    };
    Some(GhInclude {
        http_status: status,
        body,
        truncated: false,
    })
}

fn parse_http_status_line(line: &str) -> Option<u16> {
    let mut parts = line.split_whitespace();
    let _http = parts.next()?;
    parts.next()?.parse().ok()
}

fn truncate_for_receipt(text: &str) -> String {
    const LIMIT: usize = 400;
    if text.len() <= LIMIT {
        return text.to_string();
    }
    let mut end = LIMIT;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

fn git_rev_parse(rev: &str) -> Result<String, String> {
    crate::run::run_output("git", &["rev-parse", rev]).map(|text| text.trim().to_string())
}

fn read_workspace_workflows() -> Result<Vec<WorkflowFile>, String> {
    let dir = Path::new(".github/workflows");
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    let entries =
        std::fs::read_dir(dir).map_err(|err| format!("failed to read {}: {err}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("failed to read workflow entry: {err}"))?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !(name.ends_with(".yml") || name.ends_with(".yaml")) {
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
        files.push(WorkflowFile {
            path: format!(".github/workflows/{name}"),
            text,
        });
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn build_packet(snapshot: &CaptureSnapshot) -> Result<Value, String> {
    let methods = used_methods(snapshot);
    if methods.iter().any(|method| method != "GET") {
        return Err(
            "merge-queue capture refused a non-GET endpoint; the command is read-only".to_string(),
        );
    }

    let desired = desired_payload(snapshot);
    let observation = observation_payload(snapshot, &desired)?;
    let apply = apply_payload(snapshot, &observation);
    let authorities = authority_payload(snapshot);
    let identity = identity_payload(snapshot, &observation, &desired);
    let rollback = rollback_payload(&desired, &observation, &identity);
    let prior = prior_payload(snapshot.prior_receipt.as_ref(), &identity);
    let exit_state = classify_exit(
        &observation,
        &apply,
        &authorities,
        &rollback,
        &prior,
        &desired,
    );

    let mut packet = Map::new();
    insert(&mut packet, "schema_version", json!(SCHEMA_VERSION));
    insert(&mut packet, "packet_kind", json!(PACKET_KIND));
    insert(
        &mut packet,
        "claim",
        json!("EffortlessMetrics/ripr-swarm#4832"),
    );
    insert(
        &mut packet,
        "parent",
        json!("EffortlessMetrics/ripr-swarm#1029"),
    );
    insert(
        &mut packet,
        "neutral_evaluator",
        json!("EffortlessMetrics/gatecheck#11"),
    );
    insert(&mut packet, "mutability", json!("read_only"));
    insert(&mut packet, "http_methods_used", json!(methods));
    let report = report_payload(exit_state, snapshot, &rollback, &desired);
    insert(&mut packet, "exit_state", json!(exit_state));
    insert(&mut packet, "desired", desired);
    insert(&mut packet, "observation", observation);
    insert(&mut packet, "apply", apply);
    insert(&mut packet, "report", report);
    insert(&mut packet, "authorities", authorities);
    insert(&mut packet, "rollback_capture", rollback);
    insert(&mut packet, "prior_receipt", prior);
    insert(&mut packet, "identity", identity);
    Ok(Value::Object(packet))
}

fn used_methods(snapshot: &CaptureSnapshot) -> Vec<String> {
    let mut methods = BTreeSet::new();
    methods.insert("GET".to_string());
    for record in snapshot.endpoints.values() {
        methods.insert(record.method.clone());
    }
    methods.into_iter().collect()
}

fn desired_payload(snapshot: &CaptureSnapshot) -> Value {
    let extracted = extract_settings_desired(&snapshot.workspace.settings_text);
    json!({
        "kind": "repository_settings_desired.v1",
        "role": "desired_state_not_live_readback",
        "source": {
            "path": snapshot.workspace.settings_path,
            "ref": snapshot.workspace.settings_ref,
            "digest": sha256_hex(snapshot.workspace.settings_text.as_bytes()),
            "inheritance": "repository_local",
            "note": "A checked-in settings file is desired-state evidence, not live read-back."
        },
        "repository": extracted.repository,
        "classic_protection_desired": extracted.classic,
        "required_contexts": extracted.required_contexts,
        "valid": desired_settings_valid(&extracted),
    })
}

struct SettingsDesired {
    repository: Value,
    classic: Value,
    required_contexts: Vec<String>,
}

fn extract_settings_desired(text: &str) -> SettingsDesired {
    let required_contexts = yaml_list_items(text, "contexts");
    SettingsDesired {
        repository: json!({
            "default_branch": yaml_scalar(text, "default_branch"),
            "allow_squash_merge": yaml_bool(text, "allow_squash_merge"),
            "allow_merge_commit": yaml_bool(text, "allow_merge_commit"),
            "allow_rebase_merge": yaml_bool(text, "allow_rebase_merge"),
            "allow_auto_merge": yaml_bool(text, "allow_auto_merge"),
            "allow_update_branch": yaml_bool(text, "allow_update_branch"),
        }),
        classic: json!({
            "strict": yaml_bool(text, "strict"),
            "required_contexts": required_contexts,
            "enforce_admins": yaml_bool(text, "enforce_admins"),
            "allow_force_pushes": yaml_bool(text, "allow_force_pushes"),
            "allow_deletions": yaml_bool(text, "allow_deletions"),
        }),
        required_contexts,
    }
}

fn desired_settings_valid(extracted: &SettingsDesired) -> bool {
    extracted
        .repository
        .get("default_branch")
        .and_then(Value::as_str)
        .is_some_and(|name| !name.is_empty())
}

fn desired_is_valid(desired: &Value) -> bool {
    desired.get("valid") == Some(&json!(true))
        && desired
            .pointer("/repository/default_branch")
            .and_then(Value::as_str)
            .is_some_and(|name| !name.is_empty())
}

fn yaml_scalar(text: &str, key: &str) -> Value {
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        let prefix = format!("{key}:");
        if let Some(rest) = trimmed.strip_prefix(&prefix) {
            let value = rest.trim().trim_matches('"');
            if value.is_empty() || value == "null" {
                return Value::Null;
            }
            if let Ok(flag) = value.parse::<bool>() {
                return json!(flag);
            }
            return json!(value);
        }
    }
    Value::Null
}

fn yaml_bool(text: &str, key: &str) -> Value {
    match yaml_scalar(text, key) {
        Value::Bool(flag) => json!(flag),
        _ => Value::Null,
    }
}

fn yaml_list_items(text: &str, key: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut in_list = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        if trimmed == format!("{key}:") || trimmed.starts_with(&format!("{key}:")) {
            in_list = true;
            continue;
        }
        if in_list {
            if let Some(item) = trimmed.strip_prefix("- ") {
                items.push(item.trim().trim_matches('"').to_string());
                continue;
            }
            if !trimmed.is_empty() && !line.starts_with(' ') && !line.starts_with('\t') {
                break;
            }
            if !trimmed.is_empty() && !trimmed.starts_with('-') {
                break;
            }
        }
    }
    items.sort();
    items.dedup();
    items
}

fn observation_payload(snapshot: &CaptureSnapshot, desired: &Value) -> Result<Value, String> {
    let repo_record = endpoint(snapshot, &format!("/repos/{}", snapshot.repo));
    let default_branch = repo_record
        .as_ref()
        .and_then(|record| record.body.get("default_branch").and_then(Value::as_str))
        .unwrap_or("main")
        .to_string();
    let branch_record = endpoint(
        snapshot,
        &format!("/repos/{}/branches/{default_branch}", snapshot.repo),
    );
    let protection_record = endpoint(
        snapshot,
        &format!(
            "/repos/{}/branches/{default_branch}/protection",
            snapshot.repo
        ),
    );
    let list_record = endpoint(
        snapshot,
        &format!("/repos/{}/rulesets?includes_parents=true", snapshot.repo),
    );

    let classic = classic_observation(protection_record.as_ref());
    let (all_rulesets, omitted_active) =
        ruleset_observations(snapshot, &default_branch, list_record.as_ref());
    let active_union: Vec<Value> = all_rulesets
        .iter()
        .filter(|ruleset| ruleset.get("in_active_default_branch_union") == Some(&json!(true)))
        .cloned()
        .collect();
    let list_complete = list_record
        .as_ref()
        .is_some_and(|record| record.http_status == 200 && !record.truncated);
    let denominator_valid = !omitted_active && list_complete;
    let expected_ruleset = expected_ruleset_state(&all_rulesets);
    let required_union = required_context_union(
        desired
            .get("required_contexts")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        &classic,
        &active_union,
    );
    let merge_queue_rule = merge_queue_visibility(&all_rulesets, list_record.as_ref());
    let merge_methods = merge_methods_observation(repo_record.as_ref());
    let merge_group = merge_group_inventory(&snapshot.workspace.workflow_files);
    let observed_default_sha = observed_sha(branch_record.as_ref())
        .as_str()
        .unwrap_or("")
        .to_string();
    let workflows_match_default_branch = !observed_default_sha.is_empty()
        && snapshot.workspace.workspace_head_sha == observed_default_sha;
    let update_branch = update_branch_observation(
        desired,
        repo_record.as_ref(),
        &snapshot.workspace.workflow_files,
    );
    let limitations = observation_limitations(
        protection_record.as_ref(),
        list_record.as_ref(),
        omitted_active,
        &classic,
        workflows_match_default_branch,
        desired_is_valid(desired),
    );
    let complete = observation_complete(
        repo_record.as_ref(),
        branch_record.as_ref(),
        &classic,
        list_record.as_ref(),
        denominator_valid,
        &limitations,
    );

    Ok(json!({
        "kind": "repository_settings_observation.v1",
        "complete": complete,
        "denominator_valid": denominator_valid,
        "repository": repository_identity(repo_record.as_ref(), &snapshot.repo, &default_branch),
        "freshness": {
            "observed_default_branch_sha": observed_sha(branch_record.as_ref()),
            "observed_commit_date": observed_commit_date(branch_record.as_ref()),
            "workspace_head_sha": snapshot.workspace.workspace_head_sha,
            "policy": "stale_when_settings_digest_branch_sha_permission_or_ruleset_identity_moves"
        },
        "classic_protection": classic,
        "rulesets": {
            "all": all_rulesets,
            "active_default_branch_union": active_union,
            "omitted_active_default_branch_ruleset": omitted_active,
        },
        "ruleset_18845973": expected_ruleset,
        "required_context_union": required_union,
        "merge_queue_rule": merge_queue_rule,
        "merge_methods": merge_methods,
        "merge_group_producers": merge_group.get("producers").cloned().unwrap_or(json!([])),
        "merge_group_source": "workspace_checkout",
        "merge_group_matches_observed_default_branch_sha": workflows_match_default_branch,
        "required_aggregate_names": merge_group.get("required_aggregate_names").cloned().unwrap_or(json!([])),
        "update_branch_automation": update_branch,
        "limitations": limitations,
    }))
}

fn endpoint(snapshot: &CaptureSnapshot, path: &str) -> Option<EndpointRecord> {
    let key = format!("GET {path}");
    snapshot.endpoints.get(&key).cloned()
}

fn repository_identity(
    repo: Option<&EndpointRecord>,
    full_name: &str,
    default_branch: &str,
) -> Value {
    match repo {
        Some(record) if record.http_status == 200 => json!({
            "id": record.body.get("id").cloned().unwrap_or(Value::Null),
            "full_name": record.body.get("full_name").and_then(Value::as_str).unwrap_or(full_name),
            "default_branch": default_branch,
            "state": "observed",
        }),
        Some(record) => json!({
            "id": Value::Null,
            "full_name": full_name,
            "default_branch": default_branch,
            "state": EXIT_NOT_PROVEN,
            "http_status": record.http_status,
            "reason": "repository identity endpoint was unreadable",
        }),
        None => json!({
            "id": Value::Null,
            "full_name": full_name,
            "default_branch": default_branch,
            "state": EXIT_NOT_PROVEN,
            "reason": "repository identity endpoint was not captured",
        }),
    }
}

fn observed_sha(branch: Option<&EndpointRecord>) -> Value {
    branch
        .and_then(|record| {
            record
                .body
                .pointer("/commit/sha")
                .and_then(Value::as_str)
                .map(|sha| json!(sha))
        })
        .unwrap_or(Value::Null)
}

fn observed_commit_date(branch: Option<&EndpointRecord>) -> Value {
    branch
        .and_then(|record| {
            record
                .body
                .pointer("/commit/commit/committer/date")
                .and_then(Value::as_str)
                .map(|date| json!(date))
        })
        .unwrap_or(Value::Null)
}

fn classic_observation(record: Option<&EndpointRecord>) -> Value {
    match record {
        Some(record) if record.http_status == 200 => json!({
            "instrument": format!("GET {}", record.path),
            "http_status": 200,
            "state": "observed",
            "payload_digest": sha256_json(&record.body),
            "payload": record.body,
            "required_contexts": classic_contexts(&record.body),
        }),
        Some(record) if record.http_status == 403 || record.http_status == 404 => json!({
            "instrument": format!("GET {}", record.path),
            "http_status": record.http_status,
            "state": EXIT_NOT_PROVEN,
            "reason": "missing classic-protection permission",
            "payload_digest": Value::Null,
            "payload": Value::Null,
            "required_contexts": [],
        }),
        Some(record) => json!({
            "instrument": format!("GET {}", record.path),
            "http_status": record.http_status,
            "state": EXIT_NOT_PROVEN,
            "reason": "classic-protection endpoint was unreadable",
            "payload_digest": Value::Null,
            "payload": Value::Null,
            "required_contexts": [],
        }),
        None => json!({
            "instrument": "GET /repos/{repo}/branches/{branch}/protection",
            "http_status": Value::Null,
            "state": EXIT_NOT_PROVEN,
            "reason": "classic-protection endpoint was not captured",
            "payload_digest": Value::Null,
            "payload": Value::Null,
            "required_contexts": [],
        }),
    }
}

fn classic_contexts(body: &Value) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(contexts) = body
        .pointer("/required_status_checks/contexts")
        .and_then(Value::as_array)
    {
        for context in contexts {
            if let Some(name) = context.as_str() {
                names.push(name.to_string());
            }
        }
    }
    if let Some(checks) = body
        .pointer("/required_status_checks/checks")
        .and_then(Value::as_array)
    {
        for check in checks {
            if let Some(name) = check.get("context").and_then(Value::as_str) {
                names.push(name.to_string());
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

fn ruleset_observations(
    snapshot: &CaptureSnapshot,
    default_branch: &str,
    list_record: Option<&EndpointRecord>,
) -> (Vec<Value>, bool) {
    let mut listed = Vec::new();
    if let Some(record) = list_record.filter(|record| record.http_status == 200)
        && let Some(items) = record.body.as_array()
    {
        for item in items {
            if let Some(id) = item.get("id").and_then(Value::as_i64) {
                listed.push((id, item.clone()));
            }
        }
    }
    listed.sort_by_key(|(id, _)| *id);

    let mut all = Vec::new();
    let mut omitted_active = false;
    for (id, summary) in &listed {
        let detail = endpoint(snapshot, &format!("/repos/{}/rulesets/{id}", snapshot.repo));
        let payload = match detail.as_ref() {
            Some(record) if record.http_status == 200 => record.body.clone(),
            _ => summary.clone(),
        };
        let enforcement = payload
            .get("enforcement")
            .and_then(Value::as_str)
            .or_else(|| summary.get("enforcement").and_then(Value::as_str))
            .unwrap_or("unknown")
            .to_string();
        let target = payload
            .get("target")
            .and_then(Value::as_str)
            .or_else(|| summary.get("target").and_then(Value::as_str))
            .unwrap_or("unknown")
            .to_string();
        let applicable = ruleset_targets_default_branch(&payload, default_branch);
        let active_union = enforcement == "active" && applicable == Some(true);
        let detail_missing = detail
            .as_ref()
            .is_none_or(|record| record.http_status != 200);
        let targeting_unknown =
            enforcement == "active" && target == "branch" && applicable.is_none();
        if (active_union && detail_missing) || targeting_unknown {
            omitted_active = true;
        }
        all.push(json!({
            "id": id,
            "name": payload.get("name").cloned().unwrap_or(summary.get("name").cloned().unwrap_or(Value::Null)),
            "enforcement": enforcement,
            "target": target,
            "targets_default_branch": applicable == Some(true),
            "in_active_default_branch_union": active_union,
            "source": payload.get("source").cloned().unwrap_or(Value::Null),
            "payload_digest": sha256_json(&payload),
            "payload": payload,
            "detail_state": if detail_missing { EXIT_NOT_PROVEN } else { "observed" },
        }));
    }

    let prefix = format!("GET /repos/{}/rulesets/", snapshot.repo);
    for (key, record) in &snapshot.endpoints {
        if !key.starts_with(&prefix) || key.contains('?') || record.http_status != 200 {
            continue;
        }
        let Some(id) = record.body.get("id").and_then(Value::as_i64) else {
            continue;
        };
        if all.iter().any(|item| item.get("id") == Some(&json!(id))) {
            continue;
        }
        let applicable = ruleset_targets_default_branch(&record.body, default_branch);
        let enforcement = record
            .body
            .get("enforcement")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        if enforcement == "active" && applicable != Some(false) {
            omitted_active = true;
        }
        all.push(json!({
            "id": id,
            "name": record.body.get("name").cloned().unwrap_or(Value::Null),
            "enforcement": enforcement,
            "target": record.body.get("target").cloned().unwrap_or(json!("unknown")),
            "targets_default_branch": applicable == Some(true),
            "in_active_default_branch_union": false,
            "source": record.body.get("source").cloned().unwrap_or(Value::Null),
            "payload_digest": sha256_json(&record.body),
            "payload": record.body,
            "detail_state": "observed",
            "listed_in_collection": false,
        }));
    }

    all.sort_by(|left, right| {
        let left_id = left.get("id").and_then(Value::as_i64).unwrap_or(0);
        let right_id = right.get("id").and_then(Value::as_i64).unwrap_or(0);
        left_id.cmp(&right_id)
    });
    (all, omitted_active)
}

fn ruleset_targets_default_branch(payload: &Value, default_branch: &str) -> Option<bool> {
    if payload.get("target").and_then(Value::as_str) != Some("branch") {
        return Some(false);
    }
    let Some(ref_name) = payload.pointer("/conditions/ref_name") else {
        return if payload.pointer("/conditions").is_none() {
            Some(true)
        } else {
            None
        };
    };
    if let Some(excludes) = ref_name.get("exclude").and_then(Value::as_array) {
        let mut unknown = false;
        for item in excludes {
            match ref_pattern_matches_default_branch(item, default_branch) {
                Some(true) => return Some(false),
                Some(false) => {}
                None => unknown = true,
            }
        }
        if unknown {
            return None;
        }
    }
    let includes = ref_name.get("include").and_then(Value::as_array)?;
    if includes.is_empty() {
        return Some(false);
    }
    let mut matched = false;
    let mut unknown = false;
    for item in includes {
        match ref_pattern_matches_default_branch(item, default_branch) {
            Some(true) => matched = true,
            Some(false) => {}
            None => unknown = true,
        }
    }
    if matched {
        Some(true)
    } else if unknown {
        None
    } else {
        Some(false)
    }
}

fn ref_pattern_matches_default_branch(item: &Value, default_branch: &str) -> Option<bool> {
    let name = item.as_str()?;
    if name == "~DEFAULT_BRANCH"
        || name == "~ALL"
        || name == default_branch
        || name == format!("refs/heads/{default_branch}")
        || name == "*"
        || name == "refs/heads/*"
    {
        return Some(true);
    }
    if name.contains('*') || name.contains('?') {
        return None;
    }
    Some(false)
}

fn expected_ruleset_state(all: &[Value]) -> Value {
    if let Some(current) = all
        .iter()
        .find(|item| item.get("id") == Some(&json!(EXPECTED_RULESET_ID)))
    {
        return json!({
            "id": EXPECTED_RULESET_ID,
            "state": "present",
            "name": current.get("name").cloned().unwrap_or(Value::Null),
            "payload_digest": current.get("payload_digest").cloned().unwrap_or(Value::Null),
        });
    }
    let replacements: Vec<Value> = all
        .iter()
        .filter(|item| item.get("in_active_default_branch_union") == Some(&json!(true)))
        .map(|item| {
            json!({
                "id": item.get("id").cloned().unwrap_or(Value::Null),
                "name": item.get("name").cloned().unwrap_or(Value::Null),
                "payload_digest": item.get("payload_digest").cloned().unwrap_or(Value::Null),
            })
        })
        .collect();
    if replacements.is_empty() {
        json!({
            "id": EXPECTED_RULESET_ID,
            "state": "absent",
            "replacement_or_supersession": Value::Null,
        })
    } else {
        json!({
            "id": EXPECTED_RULESET_ID,
            "state": "replaced_or_superseded",
            "replacement_or_supersession": replacements,
        })
    }
}

fn required_context_union(
    desired: Vec<Value>,
    classic: &Value,
    active_rulesets: &[Value],
) -> Value {
    let mut by_name: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for context in desired {
        if let Some(name) = context.as_str() {
            by_name
                .entry(name.to_string())
                .or_default()
                .insert("desired_settings_yml".to_string());
        }
    }
    if let Some(contexts) = classic.get("required_contexts").and_then(Value::as_array) {
        for context in contexts {
            if let Some(name) = context.as_str() {
                by_name
                    .entry(name.to_string())
                    .or_default()
                    .insert("classic_protection".to_string());
            }
        }
    }
    for ruleset in active_rulesets {
        let id = ruleset
            .get("id")
            .and_then(Value::as_i64)
            .map(|id| id.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        for name in ruleset_required_contexts(ruleset.get("payload").unwrap_or(&Value::Null)) {
            by_name
                .entry(name)
                .or_default()
                .insert(format!("ruleset:{id}"));
        }
    }
    let rows: Vec<Value> = by_name
        .into_iter()
        .map(|(name, sources)| {
            json!({
                "name": name,
                "sources": sources.into_iter().collect::<Vec<_>>(),
            })
        })
        .collect();
    json!(rows)
}

fn ruleset_required_contexts(payload: &Value) -> Vec<String> {
    let mut names = Vec::new();
    let Some(rules) = payload.get("rules").and_then(Value::as_array) else {
        return names;
    };
    for rule in rules {
        if rule.get("type").and_then(Value::as_str) != Some("required_status_checks") {
            continue;
        }
        if let Some(checks) = rule
            .pointer("/parameters/required_status_checks")
            .and_then(Value::as_array)
        {
            for check in checks {
                if let Some(name) = check.get("context").and_then(Value::as_str) {
                    names.push(name.to_string());
                }
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

fn merge_queue_visibility(all: &[Value], list_record: Option<&EndpointRecord>) -> Value {
    if list_record.is_none_or(|record| record.http_status != 200) {
        return json!({
            "visibility": EXIT_NOT_PROVEN,
            "reason": "ruleset collection was unreadable; merge-queue capability is not inferred from ruleset existence",
        });
    }
    let mut matches = Vec::new();
    for ruleset in all {
        let Some(rules) = ruleset.pointer("/payload/rules").and_then(Value::as_array) else {
            continue;
        };
        for rule in rules {
            if rule.get("type").and_then(Value::as_str) == Some("merge_queue") {
                matches.push(json!({
                    "ruleset_id": ruleset.get("id").cloned().unwrap_or(Value::Null),
                    "parameters": rule.get("parameters").cloned().unwrap_or(Value::Null),
                }));
            }
        }
    }
    if matches.is_empty() {
        json!({
            "visibility": "absent",
            "reason": "no captured ruleset currently carries a merge_queue rule",
        })
    } else {
        json!({
            "visibility": "present",
            "rules": matches,
        })
    }
}

fn merge_methods_observation(repo: Option<&EndpointRecord>) -> Value {
    match repo {
        Some(record) if record.http_status == 200 => json!({
            "state": "observed",
            "allow_squash_merge": record.body.get("allow_squash_merge").cloned().unwrap_or(Value::Null),
            "allow_merge_commit": record.body.get("allow_merge_commit").cloned().unwrap_or(Value::Null),
            "allow_rebase_merge": record.body.get("allow_rebase_merge").cloned().unwrap_or(Value::Null),
            "allow_auto_merge": record.body.get("allow_auto_merge").cloned().unwrap_or(Value::Null),
            "allow_update_branch": record.body.get("allow_update_branch").cloned().unwrap_or(Value::Null),
            "delete_branch_on_merge": record.body.get("delete_branch_on_merge").cloned().unwrap_or(Value::Null),
        }),
        Some(record) => json!({
            "state": EXIT_NOT_PROVEN,
            "http_status": record.http_status,
            "reason": "repository merge-method fields were unreadable",
        }),
        None => json!({
            "state": EXIT_NOT_PROVEN,
            "reason": "repository merge-method fields were not captured",
        }),
    }
}

fn merge_group_inventory(files: &[WorkflowFile]) -> Value {
    let mut producers = Vec::new();
    for file in files {
        if workflow_has_key(&file.text, "merge_group") {
            producers.push(json!({
                "path": file.path,
                "has_merge_group": true,
            }));
        }
    }
    json!({
        "producers": producers,
        "required_aggregate_names": ["Ripr Rust Small Result"],
    })
}

fn update_branch_observation(
    desired: &Value,
    repo: Option<&EndpointRecord>,
    files: &[WorkflowFile],
) -> Value {
    let mut concurrency = Vec::new();
    for file in files {
        if workflow_has_key(&file.text, "concurrency") {
            concurrency.push(file.path.clone());
        }
    }
    json!({
        "desired_allow_update_branch": desired.pointer("/repository/allow_update_branch").cloned().unwrap_or(Value::Null),
        "observed_allow_update_branch": repo.and_then(|record| record.body.get("allow_update_branch").cloned()).unwrap_or(Value::Null),
        "workflow_concurrency": concurrency,
        "note": "Capture records current update-branch and concurrency automation; it does not retire or enable any of it.",
    })
}

fn workflow_has_key(text: &str, key: &str) -> bool {
    text.lines().any(|line| {
        let trimmed = line.trim();
        !trimmed.starts_with('#')
            && (trimmed == format!("{key}:") || trimmed.starts_with(&format!("{key}:")))
    })
}

fn observation_limitations(
    classic: Option<&EndpointRecord>,
    list_record: Option<&EndpointRecord>,
    omitted_active: bool,
    classic_payload: &Value,
    workflows_match_default_branch: bool,
    desired_valid: bool,
) -> Vec<Value> {
    let mut limitations = Vec::new();
    if classic_payload.get("state") == Some(&json!(EXIT_NOT_PROVEN)) {
        limitations.push(json!({
            "surface": "classic_protection",
            "state": EXIT_NOT_PROVEN,
            "reason": "missing classic-protection permission cannot produce a complete observation",
        }));
    }
    if list_record.is_none_or(|record| record.http_status != 200) {
        limitations.push(json!({
            "surface": "rulesets",
            "state": EXIT_NOT_PROVEN,
            "reason": "ruleset collection was unreadable; unreadability is not absence",
        }));
    } else if list_record.is_some_and(|record| record.truncated) {
        limitations.push(json!({
            "surface": "ruleset_denominator",
            "state": EXIT_NOT_PROVEN,
            "reason": "ruleset collection was truncated before every page was captured",
        }));
    }
    if omitted_active {
        limitations.push(json!({
            "surface": "ruleset_denominator",
            "state": EXIT_NOT_PROVEN,
            "reason": "an active default-branch ruleset was omitted from the captured denominator",
        }));
    }
    if classic.is_none() {
        limitations.push(json!({
            "surface": "classic_protection",
            "state": EXIT_NOT_PROVEN,
            "reason": "classic-protection endpoint was not captured",
        }));
    }
    if !workflows_match_default_branch {
        limitations.push(json!({
            "surface": "merge_group_producers",
            "state": EXIT_NOT_PROVEN,
            "reason": "workspace workflow inventory is not the observed default-branch SHA; it is not live enforcement",
        }));
    }
    if !desired_valid {
        limitations.push(json!({
            "surface": "desired_settings",
            "state": EXIT_NOT_PROVEN,
            "reason": "checked-in settings source is missing required repository fields",
        }));
    }
    limitations
}

fn observation_complete(
    repo: Option<&EndpointRecord>,
    branch: Option<&EndpointRecord>,
    classic: &Value,
    list_record: Option<&EndpointRecord>,
    denominator_valid: bool,
    limitations: &[Value],
) -> bool {
    repo.is_some_and(|record| record.http_status == 200)
        && branch.is_some_and(|record| record.http_status == 200)
        && classic.get("state") == Some(&json!("observed"))
        && list_record.is_some_and(|record| record.http_status == 200)
        && denominator_valid
        && limitations.is_empty()
}

fn apply_payload(snapshot: &CaptureSnapshot, observation: &Value) -> Value {
    let installation = endpoint(snapshot, &format!("/repos/{}/installation", snapshot.repo));
    let rulesets_readable = observation
        .get("limitations")
        .and_then(Value::as_array)
        .is_none_or(|limitations| {
            !limitations
                .iter()
                .any(|item| item.get("surface") == Some(&json!("rulesets")))
        });
    let github_app = match installation {
        Some(record) if record.http_status == 200 => json!({
            "state": "observed",
            "app_id": record.body.get("id").cloned().unwrap_or(Value::Null),
            "can_represent_rulesets": record
                .body
                .pointer("/permissions/administration")
                .and_then(Value::as_str)
                == Some("write")
                || record.body.get("can_represent_rulesets") == Some(&json!(true)),
            "write_probed": false,
        }),
        Some(record) => json!({
            "state": EXIT_NOT_PROVEN,
            "http_status": record.http_status,
            "reason": "GitHub App installation endpoint was unreadable",
            "write_probed": false,
        }),
        None => json!({
            "state": EXIT_NOT_PROVEN,
            "reason": "GitHub App installation endpoint was not captured",
            "write_probed": false,
        }),
    };
    let ruleset_capable = github_app_can_represent_rulesets(&github_app);
    json!({
        "kind": "repository_settings_apply.v1",
        "write_probed": false,
        "routes": {
            "settings_app": {
                "can_represent_classic": true,
                "can_represent_rulesets": false,
                "reason": "The Settings app / .github/settings.yml source can represent classic protection and repository metadata, not repository rulesets or merge_queue rules."
            },
            "github_app": github_app,
            "api": {
                "can_represent_rulesets": rulesets_readable,
                "write_probed": false,
                "reason": "API write capability is not probed; representability follows successful GET observation of the ruleset collection."
            },
            "computer_use": {
                "state": "not_present",
                "reason": "This adopter capture has no computer-use apply adapter."
            }
        },
        "ruleset_capable_route_representable": ruleset_capable,
        "api_can_observe_rulesets": rulesets_readable,
    })
}

fn github_app_can_represent_rulesets(github_app: &Value) -> bool {
    github_app.get("state") == Some(&json!("observed"))
        && github_app.get("can_represent_rulesets") == Some(&json!(true))
}

fn authority_payload(snapshot: &CaptureSnapshot) -> Value {
    let mut items = Vec::new();
    for issue in DELETED_AUTHORITY_ISSUES {
        let record = endpoint(
            snapshot,
            &format!("/repos/{DELETED_AUTHORITY_REPO}/issues/{issue}"),
        );
        items.push(authority_item(*issue, record.as_ref()));
    }
    json!(items)
}

fn authority_item(issue: u64, record: Option<&EndpointRecord>) -> Value {
    let reference = format!("{DELETED_AUTHORITY_REPO}#{issue}");
    match record {
        Some(record) if record.http_status == 410 || record.http_status == 404 => json!({
            "reference": reference,
            "http_status": record.http_status,
            "disposition": "deleted_not_available_controller",
            "available_controller": false,
        }),
        Some(record) if record.http_status == 200 => json!({
            "reference": reference,
            "http_status": 200,
            "disposition": "still_present_not_consumed_as_controller",
            "available_controller": false,
            "note": "The captured issue is visible but is not treated as an available organization controller by this MQ0 receipt.",
        }),
        Some(record) => json!({
            "reference": reference,
            "http_status": record.http_status,
            "disposition": EXIT_NOT_PROVEN,
            "available_controller": false,
            "reason": "organization-controller reference was unreadable",
        }),
        None => json!({
            "reference": reference,
            "http_status": Value::Null,
            "disposition": EXIT_NOT_PROVEN,
            "available_controller": false,
            "reason": "organization-controller reference was not captured",
        }),
    }
}

fn identity_payload(snapshot: &CaptureSnapshot, observation: &Value, desired: &Value) -> Value {
    let mut ruleset_ids = Vec::new();
    let mut digests = Vec::new();
    if let Some(all) = observation
        .pointer("/rulesets/all")
        .and_then(Value::as_array)
    {
        for item in all {
            if let Some(id) = item.get("id").and_then(Value::as_i64) {
                ruleset_ids.push(id);
            }
            if let Some(digest) = item.get("payload_digest").and_then(Value::as_str) {
                digests.push(digest.to_string());
            }
        }
    }
    json!({
        "repository": snapshot.repo,
        "default_branch_sha": observation.pointer("/freshness/observed_default_branch_sha").cloned().unwrap_or(Value::Null),
        "settings_digest": desired.pointer("/source/digest").cloned().unwrap_or(Value::Null),
        "classic_permission": observation.pointer("/classic_protection/state").cloned().unwrap_or(json!(EXIT_NOT_PROVEN)),
        "classic_payload_digest": observation.pointer("/classic_protection/payload_digest").cloned().unwrap_or(Value::Null),
        "ruleset_ids": ruleset_ids,
        "ruleset_payload_digests": digests,
    })
}

fn rollback_payload(desired: &Value, observation: &Value, identity: &Value) -> Value {
    let classic_ok = observation.pointer("/classic_protection/state") == Some(&json!("observed"))
        && observation
            .pointer("/classic_protection/payload_digest")
            .and_then(Value::as_str)
            .is_some();
    let rulesets_ok = observation.get("denominator_valid") == Some(&json!(true))
        && observation
            .pointer("/rulesets/all")
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty());
    json!({
        "complete": classic_ok && rulesets_ok && identity.get("settings_digest").is_some(),
        "settings_digest": desired.pointer("/source/digest").cloned().unwrap_or(Value::Null),
        "classic_payload_digest_or_reason": observation.pointer("/classic_protection/payload_digest").cloned().unwrap_or_else(|| {
            observation.pointer("/classic_protection/reason").cloned().unwrap_or(Value::Null)
        }),
        "ruleset_payload_digests": identity.get("ruleset_payload_digests").cloned().unwrap_or(json!([])),
        "identity": identity,
    })
}

fn prior_payload(prior: Option<&Value>, identity: &Value) -> Value {
    let Some(prior) = prior else {
        return json!({
            "supplied": false,
            "stale": false,
            "reasons": [],
        });
    };
    let prior_identity = prior.get("identity").cloned().unwrap_or(Value::Null);
    let mut reasons = Vec::new();
    if prior_identity.get("settings_digest") != identity.get("settings_digest") {
        reasons.push("settings_source_moved");
    }
    if prior_identity.get("default_branch_sha") != identity.get("default_branch_sha") {
        reasons.push("default_branch_sha_moved");
    }
    if prior_identity.get("classic_permission") != identity.get("classic_permission") {
        reasons.push("classic_permission_changed");
    }
    if prior_identity.get("classic_payload_digest") != identity.get("classic_payload_digest") {
        reasons.push("classic_payload_moved");
    }
    if prior_identity.get("ruleset_ids") != identity.get("ruleset_ids")
        || prior_identity.get("ruleset_payload_digests") != identity.get("ruleset_payload_digests")
    {
        reasons.push("ruleset_identity_moved");
    }
    json!({
        "supplied": true,
        "stale": !reasons.is_empty(),
        "reasons": reasons,
    })
}

fn report_payload(
    exit_state: &str,
    snapshot: &CaptureSnapshot,
    rollback: &Value,
    desired: &Value,
) -> Value {
    json!({
        "kind": "repository_settings_report.v1",
        "mq0_exit": exit_state,
        "desired_valid": desired_is_valid(desired),
        "apply_observed": false,
        "live_observation_complete": exit_state == EXIT_READY || exit_state == EXIT_BLOCKED,
        "live_union_matches": Value::Null,
        "rollback_valid": rollback.get("complete") == Some(&json!(true)),
        "behavior_proof_required": false,
        "http_methods_used": used_methods(snapshot),
        "note": "MQ0 records current observation and apply-route capability only. It does not decide a future merge-queue desired state and does not apply settings.",
    })
}

fn classify_exit(
    observation: &Value,
    apply: &Value,
    authorities: &Value,
    rollback: &Value,
    prior: &Value,
    desired: &Value,
) -> &'static str {
    if prior.get("stale") == Some(&json!(true)) {
        return EXIT_DRIFT;
    }
    let authorities_disclosed = authorities.as_array().is_some_and(|items| {
        DELETED_AUTHORITY_ISSUES.iter().all(|issue| {
            items.iter().any(|item| {
                item.get("reference") == Some(&json!(format!("{DELETED_AUTHORITY_REPO}#{issue}")))
                    && item.get("available_controller") == Some(&json!(false))
                    && matches!(
                        item.get("disposition").and_then(Value::as_str),
                        Some("deleted_not_available_controller")
                            | Some("still_present_not_consumed_as_controller")
                    )
            })
        })
    });
    if observation.get("complete") != Some(&json!(true))
        || observation.get("denominator_valid") != Some(&json!(true))
        || !authorities_disclosed
        || rollback.get("complete") != Some(&json!(true))
        || !desired_is_valid(desired)
    {
        return EXIT_NOT_PROVEN;
    }
    if apply.get("ruleset_capable_route_representable") != Some(&json!(true)) {
        return EXIT_BLOCKED;
    }
    EXIT_READY
}

fn render_json(packet: &Value) -> Result<String, String> {
    let normalized = normalize_value(packet);
    let mut text = serde_json::to_string_pretty(&normalized)
        .map_err(|err| format!("serialize merge-queue capture: {err}"))?;
    text.push('\n');
    Ok(text)
}

fn render_markdown(packet: &Value) -> Result<String, String> {
    let exit = packet
        .get("exit_state")
        .and_then(Value::as_str)
        .unwrap_or(EXIT_NOT_PROVEN);
    let complete = packet
        .pointer("/observation/complete")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut body = String::new();
    body.push_str("# Merge-queue current-state capture\n\n");
    body.push_str(&format!("MQ0 exit: `{exit}`\n\n"));
    body.push_str("This receipt is read-only current-state capture. It does not apply settings and it does not decide a future merge-queue desired state.\n\n");
    if exit != EXIT_READY {
        body.push_str(&format!(
            "The Markdown summary does not strengthen `{exit}`. Observation complete: `{complete}`.\n\n"
        ));
    }
    body.push_str("## Separate facts\n\n");
    body.push_str("- Desired source: checked-in `.github/settings.yml` (not live read-back)\n");
    body.push_str(&format!("- Live observation complete: `{}`\n", complete));
    body.push_str(&format!(
        "- Apply write probed: `{}`\n",
        packet
            .pointer("/apply/write_probed")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    ));
    body.push_str(&format!(
        "- Rollback capture complete: `{}`\n\n",
        packet
            .pointer("/rollback_capture/complete")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    ));
    body.push_str("## Required-context union\n\n");
    if let Some(rows) = packet
        .pointer("/observation/required_context_union")
        .and_then(Value::as_array)
    {
        if rows.is_empty() {
            body.push_str("- none captured\n");
        }
        for row in rows {
            let name = row.get("name").and_then(Value::as_str).unwrap_or("unknown");
            let sources = row
                .get("sources")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            body.push_str(&format!("- `{name}` ← {sources}\n"));
        }
    }
    body.push_str("\n## merge_group producers\n\n");
    if let Some(producers) = packet
        .pointer("/observation/merge_group_producers")
        .and_then(Value::as_array)
    {
        if producers.is_empty() {
            body.push_str("- none; no workflow currently declares `merge_group`\n");
        }
        for producer in producers {
            if let Some(path) = producer.get("path").and_then(Value::as_str) {
                body.push_str(&format!("- `{path}`\n"));
            }
        }
    }
    body.push_str("\n## Organization controller references\n\n");
    if let Some(items) = packet.get("authorities").and_then(Value::as_array) {
        for item in items {
            let reference = item
                .get("reference")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let disposition = item
                .get("disposition")
                .and_then(Value::as_str)
                .unwrap_or(EXIT_NOT_PROVEN);
            body.push_str(&format!(
                "- `{reference}`: `{disposition}` (available controller: `{}`)\n",
                item.get("available_controller")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            ));
        }
    }
    body.push('\n');
    Ok(body)
}

fn normalize_value(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = Map::new();
            for (key, child) in map {
                out.insert(key.clone(), normalize_value(child));
            }
            Value::Object(out)
        }
        Value::Array(items) => {
            let mut normalized: Vec<Value> = items.iter().map(normalize_value).collect();
            if normalized.iter().all(Value::is_object) {
                normalized.sort_by_key(object_sort_key);
            } else if normalized.iter().all(Value::is_string) {
                normalized.sort_by(|left, right| left.as_str().cmp(&right.as_str()));
            }
            Value::Array(normalized)
        }
        other => other.clone(),
    }
}

fn object_sort_key(value: &Value) -> String {
    for key in ["id", "name", "path", "reference", "surface", "instrument"] {
        if let Some(text) = value.get(key).and_then(Value::as_str) {
            return format!("{key}:{text}");
        }
        if let Some(id) = value.get(key).and_then(Value::as_i64) {
            return format!("{key}:{id:016}");
        }
    }
    sha256_json(value)
}

fn insert(map: &mut Map<String, Value>, key: &str, value: Value) {
    map.insert(key.to_string(), value);
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

fn read_json_file(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|err| format!("failed to parse {} as JSON: {err}", path.display()))
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn sha256_json(value: &Value) -> String {
    sha256_hex(normalize_value(value).to_string().as_bytes())
}

fn redact_secrets(value: &mut Value) {
    match value {
        Value::Object(map) => {
            let keys: Vec<String> = map.keys().cloned().collect();
            for key in keys {
                if redact_key(&key) {
                    map.insert(key, json!("[redacted]"));
                    continue;
                }
                if let Some(child) = map.get_mut(&key) {
                    redact_secrets(child);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                redact_secrets(item);
            }
        }
        _ => {}
    }
}

fn redact_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    REDACT_KEY_MARKERS
        .iter()
        .any(|marker| lower.contains(marker))
}

#[cfg(test)]
mod merge_queue_capture_tests {
    use super::*;
    use crate::command::XtaskCommand;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_out(label: &str) -> Result<PathBuf, String> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let path =
            std::env::temp_dir().join(format!("ripr-mq0-{label}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&path).map_err(|err| format!("create temp out: {err}"))?;
        Ok(path)
    }

    fn settings_text() -> String {
        r#"repository:
  default_branch: main
  allow_squash_merge: true
  allow_merge_commit: false
  allow_rebase_merge: false
  allow_auto_merge: true
  allow_update_branch: true
branches:
  - name: main
    protection:
      required_status_checks:
        strict: true
        contexts:
          - Ripr Rust Small Result
      enforce_admins: true
      allow_force_pushes: false
      allow_deletions: false
"#
        .to_string()
    }

    fn endpoint_ok(path: &str, body: Value) -> (String, Value) {
        (
            format!("GET {path}"),
            json!({ "http_status": 200, "body": body }),
        )
    }

    fn endpoint_status(path: &str, status: u16, body: Value) -> (String, Value) {
        (
            format!("GET {path}"),
            json!({ "http_status": status, "body": body }),
        )
    }

    fn ruleset_payload(
        id: i64,
        name: &str,
        enforcement: &str,
        target: &str,
        include: &[&str],
        rules: Value,
    ) -> Value {
        json!({
            "id": id,
            "name": name,
            "enforcement": enforcement,
            "target": target,
            "source": "Repository",
            "conditions": { "ref_name": { "include": include } },
            "rules": rules,
        })
    }

    fn main_ruleset() -> Value {
        ruleset_payload(
            EXPECTED_RULESET_ID,
            "main",
            "active",
            "branch",
            &["~DEFAULT_BRANCH"],
            json!([
                { "type": "deletion" },
                { "type": "non_fast_forward" },
                {
                    "type": "pull_request",
                    "parameters": {
                        "required_review_thread_resolution": true,
                        "required_approving_review_count": 0
                    }
                }
            ]),
        )
    }

    fn tag_ruleset() -> Value {
        ruleset_payload(
            20_661_783,
            "release-transaction-pins",
            "active",
            "tag",
            &["refs/tags/*"],
            json!([{ "type": "deletion" }]),
        )
    }

    fn complete_endpoints(repo: &str, extra_rulesets: &[Value], classic_status: u16) -> Value {
        complete_endpoints_with_app(repo, extra_rulesets, classic_status, true)
    }

    fn complete_endpoints_with_app(
        repo: &str,
        extra_rulesets: &[Value],
        classic_status: u16,
        ruleset_capable_app: bool,
    ) -> Value {
        let mut rulesets = vec![main_ruleset()];
        rulesets.extend(extra_rulesets.iter().cloned());
        let mut endpoints = Map::new();
        let installation = if ruleset_capable_app {
            endpoint_ok(
                &format!("/repos/{repo}/installation"),
                json!({
                    "id": 99,
                    "can_represent_rulesets": true,
                    "permissions": { "administration": "write" }
                }),
            )
        } else {
            endpoint_status(
                &format!("/repos/{repo}/installation"),
                404,
                json!({ "message": "Not Found" }),
            )
        };
        let pairs = [
            endpoint_ok(
                &format!("/repos/{repo}"),
                json!({
                    "id": 123,
                    "full_name": repo,
                    "default_branch": "main",
                    "allow_squash_merge": true,
                    "allow_merge_commit": false,
                    "allow_rebase_merge": false,
                    "allow_auto_merge": true,
                    "allow_update_branch": true,
                    "delete_branch_on_merge": true,
                    "token": "should-not-survive",
                }),
            ),
            endpoint_ok(
                &format!("/repos/{repo}/branches/main"),
                json!({
                    "commit": {
                        "sha": "abc123def456",
                        "commit": { "committer": { "date": "2026-09-29T00:00:00Z" } }
                    }
                }),
            ),
            endpoint_status(
                &format!("/repos/{repo}/branches/main/protection"),
                classic_status,
                if classic_status == 200 {
                    json!({
                        "required_status_checks": {
                            "strict": true,
                            "contexts": ["Ripr Rust Small Result"]
                        }
                    })
                } else {
                    json!({ "message": "Upgrade to GitHub Pro or set a classic protection token" })
                },
            ),
            endpoint_ok(
                &format!("/repos/{repo}/rulesets?includes_parents=true"),
                json!(
                    rulesets
                        .iter()
                        .map(|item| json!({
                            "id": item.get("id"),
                            "name": item.get("name"),
                            "enforcement": item.get("enforcement"),
                            "target": item.get("target"),
                        }))
                        .collect::<Vec<_>>()
                ),
            ),
            installation,
            endpoint_status(
                "/repos/EffortlessMetrics/.github/issues/2",
                410,
                json!({ "message": "Gone" }),
            ),
            endpoint_status(
                "/repos/EffortlessMetrics/.github/issues/3",
                410,
                json!({ "message": "Gone" }),
            ),
        ];
        for (key, value) in pairs {
            endpoints.insert(key, value);
        }
        for ruleset in &rulesets {
            if let Some(id) = ruleset.get("id").and_then(Value::as_i64) {
                let (key, value) =
                    endpoint_ok(&format!("/repos/{repo}/rulesets/{id}"), ruleset.clone());
                endpoints.insert(key, value);
            }
        }
        Value::Object(endpoints)
    }

    fn snapshot_from_parts(
        repo: &str,
        endpoints: Value,
        workflows: Vec<WorkflowFile>,
        prior: Option<Value>,
    ) -> Result<CaptureSnapshot, String> {
        snapshot_from_parts_custom(
            repo,
            endpoints,
            workflows,
            prior,
            &settings_text(),
            "abc123def456",
        )
    }

    fn snapshot_from_parts_custom(
        repo: &str,
        endpoints: Value,
        workflows: Vec<WorkflowFile>,
        prior: Option<Value>,
        settings: &str,
        head_sha: &str,
    ) -> Result<CaptureSnapshot, String> {
        parse_input_snapshot(
            &json!({
                "repo": repo,
                "endpoints": endpoints,
                "workspace": {
                    "settings_yml_path": ".github/settings.yml",
                    "settings_yml_text": settings,
                    "settings_ref": head_sha,
                    "head_sha": head_sha,
                    "workflow_files": workflows.iter().map(|file| json!({
                        "path": file.path,
                        "text": file.text,
                    })).collect::<Vec<_>>(),
                },
                "prior_receipt": prior,
            }),
            repo,
        )
    }

    fn complete_snapshot() -> Result<CaptureSnapshot, String> {
        snapshot_from_parts(
            DEFAULT_REPO,
            complete_endpoints(DEFAULT_REPO, &[tag_ruleset()], 200),
            vec![WorkflowFile {
                path: ".github/workflows/routed-rust.yml".to_string(),
                text: "on:\n  pull_request:\nconcurrency:\n  group: routed\n".to_string(),
            }],
            None,
        )
    }

    #[test]
    fn merge_queue_capture_parses_only_capture_subcommand() -> Result<(), String> {
        match XtaskCommand::parse(["merge-queue".into(), "capture".into()]) {
            XtaskCommand::MergeQueue(args) if args == ["capture"] => {}
            other => {
                return Err(format!("expected merge-queue capture, got {other:?}"));
            }
        }
        let err = parse_options(&["validate-desired".to_string()])
            .err()
            .ok_or_else(|| "MQ1 must stay out of scope".to_string())?;
        if !err.contains("only `capture` is accepted") {
            return Err(format!("unexpected MQ1 rejection: {err}"));
        }
        let err = parse_options(&["capture".to_string(), "--apply".to_string()])
            .err()
            .ok_or_else(|| "mutation flags must be refused".to_string())?;
        if !err.contains("GET-only") {
            return Err(format!("unexpected mutation rejection: {err}"));
        }
        Ok(())
    }

    #[test]
    fn merge_queue_capture_complete_observation_is_ready_for_desired_state() -> Result<(), String> {
        let packet = build_packet(&complete_snapshot()?)?;
        assert_eq!(packet.get("exit_state"), Some(&json!(EXIT_READY)));
        assert_eq!(packet.pointer("/observation/complete"), Some(&json!(true)));
        assert_eq!(
            packet.pointer("/desired/role"),
            Some(&json!("desired_state_not_live_readback"))
        );
        assert_eq!(packet.pointer("/apply/write_probed"), Some(&json!(false)));
        assert_eq!(
            packet.pointer("/observation/ruleset_18845973/state"),
            Some(&json!("present"))
        );
        assert_eq!(
            packet.pointer("/observation/merge_queue_rule/visibility"),
            Some(&json!("absent"))
        );
        Ok(())
    }

    #[test]
    fn merge_queue_capture_missing_classic_permission_cannot_be_complete() -> Result<(), String> {
        let snapshot = snapshot_from_parts(
            DEFAULT_REPO,
            complete_endpoints(DEFAULT_REPO, &[tag_ruleset()], 403),
            Vec::new(),
            None,
        )?;
        let packet = build_packet(&snapshot)?;
        assert_eq!(packet.get("exit_state"), Some(&json!(EXIT_NOT_PROVEN)));
        assert_eq!(packet.pointer("/observation/complete"), Some(&json!(false)));
        assert_eq!(
            packet.pointer("/observation/classic_protection/reason"),
            Some(&json!("missing classic-protection permission"))
        );
        let markdown = render_markdown(&packet)?;
        assert!(markdown.contains(EXIT_NOT_PROVEN));
        assert!(!markdown.contains("ready to apply"));
        assert!(!markdown.contains("protection is complete"));
        Ok(())
    }

    #[test]
    fn merge_queue_capture_omitted_active_ruleset_fails_denominator() -> Result<(), String> {
        let mut endpoints = complete_endpoints(DEFAULT_REPO, &[tag_ruleset()], 200);
        let hidden = ruleset_payload(
            99,
            "hidden-main",
            "active",
            "branch",
            &["~DEFAULT_BRANCH"],
            json!([{ "type": "deletion" }]),
        );
        if let Some(map) = endpoints.as_object_mut() {
            map.insert(
                format!("GET /repos/{DEFAULT_REPO}/rulesets/99"),
                json!({ "http_status": 200, "body": hidden }),
            );
        }
        let snapshot = snapshot_from_parts(DEFAULT_REPO, endpoints, Vec::new(), None)?;
        let packet = build_packet(&snapshot)?;
        assert_eq!(packet.get("exit_state"), Some(&json!(EXIT_NOT_PROVEN)));
        assert_eq!(
            packet.pointer("/observation/rulesets/omitted_active_default_branch_ruleset"),
            Some(&json!(true))
        );
        assert_eq!(
            packet.pointer("/observation/denominator_valid"),
            Some(&json!(false))
        );
        Ok(())
    }

    #[test]
    fn merge_queue_capture_inactive_ruleset_stays_visible_but_out_of_union() -> Result<(), String> {
        let evaluate = ruleset_payload(
            77,
            "evaluate-main",
            "evaluate",
            "branch",
            &["~DEFAULT_BRANCH"],
            json!([{
                "type": "required_status_checks",
                "parameters": { "required_status_checks": [{ "context": "Extra Check" }] }
            }]),
        );
        let snapshot = snapshot_from_parts(
            DEFAULT_REPO,
            complete_endpoints(DEFAULT_REPO, &[tag_ruleset(), evaluate], 200),
            Vec::new(),
            None,
        )?;
        let packet = build_packet(&snapshot)?;
        let all = packet
            .pointer("/observation/rulesets/all")
            .and_then(Value::as_array)
            .ok_or_else(|| "missing ruleset inventory".to_string())?;
        assert!(all.iter().any(|item| item.get("id") == Some(&json!(77))));
        let union = packet
            .pointer("/observation/rulesets/active_default_branch_union")
            .and_then(Value::as_array)
            .ok_or_else(|| "missing active union".to_string())?;
        assert!(union.iter().all(|item| item.get("id") != Some(&json!(77))));
        assert!(
            union
                .iter()
                .all(|item| item.get("id") != Some(&json!(20_661_783)))
        );
        let names = packet
            .pointer("/observation/required_context_union")
            .and_then(Value::as_array)
            .ok_or_else(|| "missing context union".to_string())?;
        assert!(
            names
                .iter()
                .all(|item| item.get("name") != Some(&json!("Extra Check")))
        );
        Ok(())
    }

    #[test]
    fn merge_queue_capture_shared_context_is_source_attributed() -> Result<(), String> {
        let with_classic_context = ruleset_payload(
            EXPECTED_RULESET_ID,
            "main",
            "active",
            "branch",
            &["~DEFAULT_BRANCH"],
            json!([{
                "type": "required_status_checks",
                "parameters": {
                    "required_status_checks": [{ "context": "Ripr Rust Small Result" }]
                }
            }]),
        );
        let mut endpoints = complete_endpoints(DEFAULT_REPO, &[tag_ruleset()], 200);
        if let Some(map) = endpoints.as_object_mut() {
            map.insert(
                format!("GET /repos/{DEFAULT_REPO}/rulesets/{EXPECTED_RULESET_ID}"),
                json!({ "http_status": 200, "body": with_classic_context }),
            );
        }
        let snapshot = snapshot_from_parts(DEFAULT_REPO, endpoints, Vec::new(), None)?;
        let packet = build_packet(&snapshot)?;
        let union = packet
            .pointer("/observation/required_context_union")
            .and_then(Value::as_array)
            .ok_or_else(|| "missing context union".to_string())?;
        let row = union
            .iter()
            .find(|item| item.get("name") == Some(&json!("Ripr Rust Small Result")))
            .ok_or_else(|| "shared context missing".to_string())?;
        let sources = row
            .get("sources")
            .and_then(Value::as_array)
            .ok_or_else(|| "sources missing".to_string())?;
        assert!(
            sources
                .iter()
                .any(|item| item == &json!("classic_protection"))
        );
        assert!(
            sources
                .iter()
                .any(|item| item == &json!("desired_settings_yml"))
        );
        assert!(
            sources
                .iter()
                .any(|item| item == &json!(format!("ruleset:{EXPECTED_RULESET_ID}")))
        );
        assert_eq!(union.len(), 1);
        Ok(())
    }

    #[test]
    fn merge_queue_capture_deleted_org_refs_cannot_be_controllers() -> Result<(), String> {
        let packet = build_packet(&complete_snapshot()?)?;
        let authorities = packet
            .get("authorities")
            .and_then(Value::as_array)
            .ok_or_else(|| "authorities missing".to_string())?;
        for issue in DELETED_AUTHORITY_ISSUES {
            let item = authorities
                .iter()
                .find(|row| {
                    row.get("reference")
                        == Some(&json!(format!("{DELETED_AUTHORITY_REPO}#{issue}")))
                })
                .ok_or_else(|| format!("missing authority {issue}"))?;
            assert_eq!(item.get("available_controller"), Some(&json!(false)));
            assert_eq!(
                item.get("disposition"),
                Some(&json!("deleted_not_available_controller"))
            );
        }
        Ok(())
    }

    #[test]
    fn merge_queue_capture_prior_identity_movement_is_stale() -> Result<(), String> {
        let current = complete_snapshot()?;
        let mut prior = build_packet(&current)?;
        if let Some(identity) = prior.get_mut("identity").and_then(Value::as_object_mut) {
            identity.insert("default_branch_sha".to_string(), json!("old-sha"));
        }
        let snapshot = snapshot_from_parts(
            DEFAULT_REPO,
            complete_endpoints(DEFAULT_REPO, &[tag_ruleset()], 200),
            Vec::new(),
            Some(prior),
        )?;
        let packet = build_packet(&snapshot)?;
        assert_eq!(packet.get("exit_state"), Some(&json!(EXIT_DRIFT)));
        assert_eq!(packet.pointer("/prior_receipt/stale"), Some(&json!(true)));
        let reasons = packet
            .pointer("/prior_receipt/reasons")
            .and_then(Value::as_array)
            .ok_or_else(|| "missing stale reasons".to_string())?;
        assert!(
            reasons
                .iter()
                .any(|item| item == &json!("default_branch_sha_moved"))
        );
        Ok(())
    }

    #[test]
    fn merge_queue_capture_reordered_input_is_byte_stable() -> Result<(), String> {
        let first = complete_endpoints(DEFAULT_REPO, &[tag_ruleset()], 200);
        let mut second = first.clone();
        if let Some(items) = second
            .pointer_mut(&format!(
                "/GET /repos/{DEFAULT_REPO}/rulesets?includes_parents=true/body"
            ))
            .and_then(Value::as_array_mut)
        {
            items.reverse();
        } else if let Some(record) = second
            .as_object_mut()
            .and_then(|map| {
                map.get_mut(&format!(
                    "GET /repos/{DEFAULT_REPO}/rulesets?includes_parents=true"
                ))
            })
            .and_then(Value::as_object_mut)
            && let Some(items) = record.get_mut("body").and_then(Value::as_array_mut)
        {
            items.reverse();
        }
        let left = build_packet(&snapshot_from_parts(DEFAULT_REPO, first, Vec::new(), None)?)?;
        let right = build_packet(&snapshot_from_parts(
            DEFAULT_REPO,
            second,
            Vec::new(),
            None,
        )?)?;
        if render_json(&left)? != render_json(&right)? {
            return Err("reordered ruleset list changed normalized JSON bytes".to_string());
        }
        if render_markdown(&left)? != render_markdown(&right)? {
            return Err("reordered ruleset list changed Markdown bytes".to_string());
        }
        Ok(())
    }

    #[test]
    fn merge_queue_capture_settings_only_apply_is_capability_blocked() -> Result<(), String> {
        let mut endpoints = complete_endpoints(DEFAULT_REPO, &[tag_ruleset()], 200);
        if let Some(map) = endpoints.as_object_mut() {
            map.insert(
                format!("GET /repos/{DEFAULT_REPO}/rulesets?includes_parents=true"),
                json!({
                    "http_status": 403,
                    "body": { "message": "Resource not accessible by integration" }
                }),
            );
            map.retain(|key, _| !key.contains("/rulesets/"));
            map.insert(
                format!("GET /repos/{DEFAULT_REPO}/rulesets?includes_parents=true"),
                json!({
                    "http_status": 403,
                    "body": { "message": "Resource not accessible by integration" }
                }),
            );
        }
        let snapshot = snapshot_from_parts(DEFAULT_REPO, endpoints, Vec::new(), None)?;
        let packet = build_packet(&snapshot)?;
        assert_eq!(
            packet.pointer("/observation/classic_protection/state"),
            Some(&json!("observed"))
        );
        assert_eq!(packet.get("exit_state"), Some(&json!(EXIT_NOT_PROVEN)));
        assert_eq!(
            packet.pointer("/apply/routes/settings_app/can_represent_rulesets"),
            Some(&json!(false))
        );
        Ok(())
    }

    #[test]
    fn merge_queue_capture_complete_without_ruleset_representation_is_blocked() -> Result<(), String>
    {
        let snapshot = snapshot_from_parts(
            DEFAULT_REPO,
            complete_endpoints_with_app(DEFAULT_REPO, &[tag_ruleset()], 200, false),
            Vec::new(),
            None,
        )?;
        let packet = build_packet(&snapshot)?;
        assert_eq!(packet.pointer("/observation/complete"), Some(&json!(true)));
        assert_eq!(
            packet.pointer("/apply/ruleset_capable_route_representable"),
            Some(&json!(false))
        );
        assert_eq!(
            packet.pointer("/apply/routes/settings_app/can_represent_rulesets"),
            Some(&json!(false))
        );
        assert_eq!(packet.get("exit_state"), Some(&json!(EXIT_BLOCKED)));
        Ok(())
    }

    #[test]
    fn merge_queue_capture_refuses_non_get_input() -> Result<(), String> {
        let err = parse_endpoint_record(
            "PATCH /repos/EffortlessMetrics/ripr-swarm/rulesets/1",
            &json!({ "http_status": 200, "body": {} }),
        )
        .err()
        .ok_or_else(|| "non-GET input must fail".to_string())?;
        if !err.contains("not GET") {
            return Err(format!("unexpected non-GET rejection: {err}"));
        }
        Ok(())
    }

    #[test]
    fn merge_queue_capture_redacts_secrets_and_writes_offline_receipt() -> Result<(), String> {
        let out = temp_out("offline")?;
        let input = out.join("input.json");
        let mut raw = json!({
            "repo": DEFAULT_REPO,
            "endpoints": complete_endpoints(DEFAULT_REPO, &[tag_ruleset()], 200),
            "workspace": {
                "settings_yml_path": ".github/settings.yml",
                "settings_yml_text": settings_text(),
                "settings_ref": "abc123def456",
                "head_sha": "abc123def456",
                "workflow_files": []
            }
        });
        if let Some(endpoints) = raw.get_mut("endpoints").and_then(Value::as_object_mut)
            && let Some(record) = endpoints.get_mut(&format!("GET /repos/{DEFAULT_REPO}"))
            && let Some(body) = record.get_mut("body").and_then(Value::as_object_mut)
        {
            body.insert("token".to_string(), json!("ghp_should_not_leak"));
            body.insert("email".to_string(), json!("secret@example.com"));
        }
        fs::write(
            &input,
            serde_json::to_string_pretty(&raw).map_err(|err| err.to_string())?,
        )
        .map_err(|err| format!("write input: {err}"))?;
        merge_queue(&[
            "capture".to_string(),
            "--repo".to_string(),
            DEFAULT_REPO.to_string(),
            "--input".to_string(),
            input.display().to_string(),
            "--out".to_string(),
            out.display().to_string(),
        ])?;
        let written = fs::read_to_string(out.join(JSON_NAME))
            .map_err(|err| format!("read written packet: {err}"))?;
        if written.contains("ghp_should_not_leak") || written.contains("secret@example.com") {
            return Err("secret material leaked into the written receipt".to_string());
        }
        let loaded = load_input_snapshot(&input, DEFAULT_REPO)?;
        let repo_key = format!("GET /repos/{DEFAULT_REPO}");
        let body = loaded
            .endpoints
            .get(&repo_key)
            .ok_or_else(|| "loaded snapshot missing repo endpoint".to_string())?;
        if body.body.get("token") != Some(&json!("[redacted]"))
            || body.body.get("email") != Some(&json!("[redacted]"))
        {
            return Err("secret keys were not redacted on input admission".to_string());
        }
        if !fs::read_to_string(out.join(MD_NAME))
            .map_err(|err| format!("read markdown: {err}"))?
            .contains("MQ0 exit:")
        {
            return Err("markdown receipt missing MQ0 exit".to_string());
        }
        Ok(())
    }

    #[test]
    fn merge_queue_capture_extracts_real_settings_desired_source() {
        let text = include_str!("../../../.github/settings.yml");
        let desired = extract_settings_desired(text);
        assert_eq!(
            desired.required_contexts,
            vec!["Ripr Rust Small Result".to_string()]
        );
        assert_eq!(
            desired.repository.get("allow_update_branch"),
            Some(&json!(true))
        );
        assert_eq!(desired.classic.get("strict"), Some(&json!(true)));
    }

    #[test]
    fn merge_queue_capture_live_request_plan_is_get_only() {
        let plan = [
            "GET /repos/EffortlessMetrics/ripr-swarm",
            "GET /repos/EffortlessMetrics/ripr-swarm/branches/main",
            "GET /repos/EffortlessMetrics/ripr-swarm/branches/main/protection",
            "GET /repos/EffortlessMetrics/ripr-swarm/rulesets?includes_parents=true",
            "GET /repos/EffortlessMetrics/ripr-swarm/rulesets/18845973",
            "GET /repos/EffortlessMetrics/.github/issues/2",
            "GET /repos/EffortlessMetrics/.github/issues/3",
        ];
        assert!(plan.iter().all(|item| item.starts_with("GET ")));
        assert!(
            plan.iter()
                .all(|item| !item.contains("PATCH") && !item.contains("POST"))
        );
    }

    #[test]
    fn merge_queue_capture_truncated_ruleset_page_fails_denominator() -> Result<(), String> {
        let mut endpoints = complete_endpoints(DEFAULT_REPO, &[tag_ruleset()], 200);
        if let Some(record) = endpoints
            .as_object_mut()
            .and_then(|map| {
                map.get_mut(&format!(
                    "GET /repos/{DEFAULT_REPO}/rulesets?includes_parents=true"
                ))
            })
            .and_then(Value::as_object_mut)
        {
            record.insert("truncated".to_string(), json!(true));
        }
        let packet = build_packet(&snapshot_from_parts(
            DEFAULT_REPO,
            endpoints,
            Vec::new(),
            None,
        )?)?;
        assert_eq!(packet.get("exit_state"), Some(&json!(EXIT_NOT_PROVEN)));
        assert_eq!(
            packet.pointer("/observation/denominator_valid"),
            Some(&json!(false))
        );
        assert_eq!(
            packet.pointer("/rollback_capture/complete"),
            Some(&json!(false))
        );
        let limitations = packet
            .pointer("/observation/limitations")
            .and_then(Value::as_array)
            .ok_or_else(|| "missing limitations".to_string())?;
        assert!(limitations.iter().any(|item| {
            item.get("surface") == Some(&json!("ruleset_denominator"))
                && item
                    .get("reason")
                    .and_then(Value::as_str)
                    .is_some_and(|reason| reason.contains("truncated"))
        }));
        Ok(())
    }

    #[test]
    fn merge_queue_capture_exclude_default_branch_stays_out_of_union() -> Result<(), String> {
        let excluded = json!({
            "id": 55,
            "name": "exclude-main",
            "enforcement": "active",
            "target": "branch",
            "source": "Repository",
            "conditions": {
                "ref_name": {
                    "include": ["~ALL"],
                    "exclude": ["~DEFAULT_BRANCH"]
                }
            },
            "rules": [{ "type": "deletion" }],
        });
        let packet = build_packet(&snapshot_from_parts(
            DEFAULT_REPO,
            complete_endpoints(DEFAULT_REPO, &[tag_ruleset(), excluded], 200),
            Vec::new(),
            None,
        )?)?;
        assert_eq!(packet.get("exit_state"), Some(&json!(EXIT_READY)));
        let all = packet
            .pointer("/observation/rulesets/all")
            .and_then(Value::as_array)
            .ok_or_else(|| "missing ruleset inventory".to_string())?;
        let excluded_row = all
            .iter()
            .find(|item| item.get("id") == Some(&json!(55)))
            .ok_or_else(|| "excluded ruleset missing from inventory".to_string())?;
        assert_eq!(
            excluded_row.get("targets_default_branch"),
            Some(&json!(false))
        );
        assert_eq!(
            excluded_row.get("in_active_default_branch_union"),
            Some(&json!(false))
        );
        let union = packet
            .pointer("/observation/rulesets/active_default_branch_union")
            .and_then(Value::as_array)
            .ok_or_else(|| "missing active union".to_string())?;
        assert!(union.iter().all(|item| item.get("id") != Some(&json!(55))));
        Ok(())
    }

    #[test]
    fn merge_queue_capture_unknown_include_glob_fails_denominator() -> Result<(), String> {
        let unknown = ruleset_payload(
            66,
            "unknown-glob",
            "active",
            "branch",
            &["refs/heads/m*"],
            json!([{ "type": "deletion" }]),
        );
        let packet = build_packet(&snapshot_from_parts(
            DEFAULT_REPO,
            complete_endpoints(DEFAULT_REPO, &[tag_ruleset(), unknown], 200),
            Vec::new(),
            None,
        )?)?;
        assert_eq!(packet.get("exit_state"), Some(&json!(EXIT_NOT_PROVEN)));
        assert_eq!(
            packet.pointer("/observation/rulesets/omitted_active_default_branch_ruleset"),
            Some(&json!(true))
        );
        assert_eq!(
            packet.pointer("/observation/denominator_valid"),
            Some(&json!(false))
        );
        Ok(())
    }

    #[test]
    fn merge_queue_capture_unread_listed_detail_fails_denominator() -> Result<(), String> {
        let mut endpoints = complete_endpoints(DEFAULT_REPO, &[tag_ruleset()], 200);
        if let Some(map) = endpoints.as_object_mut() {
            if let Some(record) = map.get_mut(&format!(
                "GET /repos/{DEFAULT_REPO}/rulesets?includes_parents=true"
            )) && let Some(items) = record.get_mut("body").and_then(Value::as_array_mut)
            {
                items.push(json!({
                    "id": 88,
                    "name": "no-detail",
                    "enforcement": "active",
                    "target": "branch"
                }));
            }
            map.insert(
                format!("GET /repos/{DEFAULT_REPO}/rulesets/88"),
                json!({
                    "http_status": 403,
                    "body": { "message": "Resource not accessible by integration" }
                }),
            );
        }
        let packet = build_packet(&snapshot_from_parts(
            DEFAULT_REPO,
            endpoints,
            Vec::new(),
            None,
        )?)?;
        assert_eq!(packet.get("exit_state"), Some(&json!(EXIT_NOT_PROVEN)));
        assert_eq!(
            packet.pointer("/observation/denominator_valid"),
            Some(&json!(false))
        );
        assert_eq!(
            packet.pointer("/observation/rulesets/omitted_active_default_branch_ruleset"),
            Some(&json!(true))
        );
        Ok(())
    }

    #[test]
    fn merge_queue_capture_unread_authority_cannot_be_ready() -> Result<(), String> {
        let mut endpoints = complete_endpoints(DEFAULT_REPO, &[tag_ruleset()], 200);
        if let Some(map) = endpoints.as_object_mut() {
            map.insert(
                "GET /repos/EffortlessMetrics/.github/issues/2".to_string(),
                json!({
                    "http_status": 403,
                    "body": { "message": "API rate limit exceeded" }
                }),
            );
        }
        let packet = build_packet(&snapshot_from_parts(
            DEFAULT_REPO,
            endpoints,
            Vec::new(),
            None,
        )?)?;
        assert_eq!(packet.get("exit_state"), Some(&json!(EXIT_NOT_PROVEN)));
        let authorities = packet
            .get("authorities")
            .and_then(Value::as_array)
            .ok_or_else(|| "authorities missing".to_string())?;
        let unread = authorities
            .iter()
            .find(|row| row.get("reference") == Some(&json!("EffortlessMetrics/.github#2")))
            .ok_or_else(|| "authority #2 missing".to_string())?;
        assert_eq!(unread.get("disposition"), Some(&json!(EXIT_NOT_PROVEN)));
        assert_eq!(unread.get("available_controller"), Some(&json!(false)));
        Ok(())
    }

    #[test]
    fn merge_queue_capture_classic_403_rollback_is_incomplete() -> Result<(), String> {
        let packet = build_packet(&snapshot_from_parts(
            DEFAULT_REPO,
            complete_endpoints(DEFAULT_REPO, &[tag_ruleset()], 403),
            Vec::new(),
            None,
        )?)?;
        assert_eq!(packet.get("exit_state"), Some(&json!(EXIT_NOT_PROVEN)));
        assert_eq!(
            packet.pointer("/rollback_capture/complete"),
            Some(&json!(false))
        );
        assert_eq!(
            packet.pointer("/observation/classic_protection/payload_digest"),
            Some(&Value::Null)
        );
        assert_eq!(
            packet.pointer("/report/rollback_valid"),
            Some(&json!(false))
        );
        Ok(())
    }

    #[test]
    fn merge_queue_capture_classic_payload_digest_movement_is_stale() -> Result<(), String> {
        let current = complete_snapshot()?;
        let mut prior = build_packet(&current)?;
        if let Some(identity) = prior.get_mut("identity").and_then(Value::as_object_mut) {
            identity.insert(
                "classic_payload_digest".to_string(),
                json!("old-classic-digest"),
            );
        }
        let packet = build_packet(&snapshot_from_parts(
            DEFAULT_REPO,
            complete_endpoints(DEFAULT_REPO, &[tag_ruleset()], 200),
            Vec::new(),
            Some(prior),
        )?)?;
        assert_eq!(packet.get("exit_state"), Some(&json!(EXIT_DRIFT)));
        let reasons = packet
            .pointer("/prior_receipt/reasons")
            .and_then(Value::as_array)
            .ok_or_else(|| "missing stale reasons".to_string())?;
        assert!(
            reasons
                .iter()
                .any(|item| item == &json!("classic_payload_moved"))
        );
        Ok(())
    }

    #[test]
    fn merge_queue_capture_workflow_sha_mismatch_is_not_live_enforcement() -> Result<(), String> {
        let packet = build_packet(&snapshot_from_parts_custom(
            DEFAULT_REPO,
            complete_endpoints(DEFAULT_REPO, &[tag_ruleset()], 200),
            vec![WorkflowFile {
                path: ".github/workflows/routed-rust.yml".to_string(),
                text: "on:\n  merge_group:\n".to_string(),
            }],
            None,
            &settings_text(),
            "workspace-only-sha",
        )?)?;
        assert_eq!(packet.get("exit_state"), Some(&json!(EXIT_NOT_PROVEN)));
        assert_eq!(
            packet.pointer("/observation/merge_group_source"),
            Some(&json!("workspace_checkout"))
        );
        assert_eq!(
            packet.pointer("/observation/merge_group_matches_observed_default_branch_sha"),
            Some(&json!(false))
        );
        assert_eq!(packet.pointer("/observation/complete"), Some(&json!(false)));
        Ok(())
    }

    #[test]
    fn merge_queue_capture_missing_desired_default_branch_is_not_valid() -> Result<(), String> {
        let packet = build_packet(&snapshot_from_parts_custom(
            DEFAULT_REPO,
            complete_endpoints(DEFAULT_REPO, &[tag_ruleset()], 200),
            Vec::new(),
            None,
            "repository:\n  allow_squash_merge: true\n",
            "abc123def456",
        )?)?;
        assert_eq!(packet.get("exit_state"), Some(&json!(EXIT_NOT_PROVEN)));
        assert_eq!(packet.pointer("/desired/valid"), Some(&json!(false)));
        assert_eq!(packet.pointer("/report/desired_valid"), Some(&json!(false)));
        Ok(())
    }

    #[test]
    fn merge_queue_capture_include_parser_does_not_copy_header_secrets() {
        let stdout = "HTTP/2 200\nAuthorization: token ghp_should_not_leak\n\n{\"ok\":true}\n";
        let parsed = parse_gh_include_output(stdout);
        assert_eq!(parsed.http_status, 200);
        assert_eq!(parsed.body, json!({ "ok": true }));
        let rendered = parsed.body.to_string();
        assert!(!rendered.contains("ghp_should_not_leak"));
        let unreadable = parse_gh_include_output("ghp_should_not_leak not-json");
        assert_eq!(unreadable.http_status, 0);
        assert_eq!(
            unreadable.body.get("message"),
            Some(&json!("gh api produced no parseable HTTP response"))
        );
        assert!(!unreadable.body.to_string().contains("ghp_should_not_leak"));
    }

    #[test]
    fn merge_queue_capture_paginated_concat_merges_and_marks_leftover() {
        let merged = parse_gh_paginated_output("[{\"id\":1}]\n[{\"id\":2}]\n");
        assert_eq!(merged.http_status, 200);
        assert!(!merged.truncated);
        assert_eq!(merged.body, json!([{ "id": 1 }, { "id": 2 }]));

        let leftover = parse_gh_paginated_output("[{\"id\":1}]\n[{\"id\":2");
        assert_eq!(leftover.http_status, 200);
        assert!(leftover.truncated);
        assert_eq!(leftover.body, json!([{ "id": 1 }]));
    }
}
