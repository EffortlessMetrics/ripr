use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const SUBCOMMAND: &str = "resolve-network-policy";
const SCHEMA: &str = "ripr.source_promotion_network_policy_resolution.v1";
const POLICY_PATH: &str = "policy/network_allowlist.txt";
const MANIFEST_SCHEMA: &str = "ripr.source_promotion_resolution_manifest_fragment.v1";
const HEADER: &str = "# Allowlisted network surfaces.\n#\n# Format:\n# path|pattern|max_count|owner|reason\n#\n# Network behavior must stay in explicit adapter or release surfaces. Unit tests\n# and domain logic should not acquire hidden network dependencies.\n\n";

#[derive(Clone, Debug, Eq, PartialEq)]
struct PolicyRow {
    path: String,
    pattern: String,
    maximum: usize,
    owner: String,
    reason: String,
}

impl PolicyRow {
    fn key(&self) -> (String, String) {
        (self.path.clone(), self.pattern.clone())
    }

    fn line(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}",
            self.path, self.pattern, self.maximum, self.owner, self.reason
        )
    }

    fn json(&self) -> Value {
        json!({
            "path": self.path,
            "pattern": self.pattern,
            "maximum": self.maximum,
            "owner": self.owner,
            "reason": self.reason,
        })
    }
}

#[derive(Clone, Debug)]
struct Resolution {
    source: Option<PolicyRow>,
    swarm: Option<PolicyRow>,
    actual_count: usize,
    evidence_blob: Option<String>,
    selected: Option<PolicyRow>,
    disposition: &'static str,
    rationale: String,
}

#[derive(Clone, Debug)]
struct Inputs {
    preflight: PathBuf,
    decisions: PathBuf,
    preflight_sha256: String,
    p0_artifact_sha256: String,
    source: String,
    swarm: String,
    merge_base: String,
    preview_tree: String,
    rejected_j5: String,
    rejected_j5_tree: String,
    output_dir: PathBuf,
}

pub(crate) fn source_promotion_network_policy_resolution_handles(args: &[String]) -> bool {
    args.first().map(String::as_str) == Some(SUBCOMMAND)
}

pub(crate) fn source_promotion_network_policy_resolution(args: &[String]) -> Result<(), String> {
    let inputs = parse_inputs(args)?;
    generate(&inputs)
}

fn parse_inputs(args: &[String]) -> Result<Inputs, String> {
    if !source_promotion_network_policy_resolution_handles(args) {
        return Err(format!("expected source-promotion {SUBCOMMAND}"));
    }
    let values = parse_named_values(&args[1..])?;
    Ok(Inputs {
        preflight: required_path(&values, "--preflight")?,
        decisions: required_path(&values, "--decisions")?,
        preflight_sha256: required(&values, "--preflight-sha256")?,
        p0_artifact_sha256: required(&values, "--p0-artifact-sha256")?,
        source: required(&values, "--source")?,
        swarm: required(&values, "--swarm")?,
        merge_base: required(&values, "--merge-base")?,
        preview_tree: required(&values, "--preview-tree")?,
        rejected_j5: required(&values, "--rejected-j5")?,
        rejected_j5_tree: required(&values, "--rejected-j5-tree")?,
        output_dir: required_path(&values, "--output-dir")?,
    })
}

fn parse_named_values(args: &[String]) -> Result<BTreeMap<String, String>, String> {
    if !args.len().is_multiple_of(2) {
        return Err("resolution arguments must be --name value pairs".to_string());
    }
    let mut values = BTreeMap::new();
    for pair in args.chunks_exact(2) {
        let name = pair
            .first()
            .ok_or_else(|| "named argument pair is missing its name".to_string())?;
        let value = pair
            .get(1)
            .ok_or_else(|| format!("named argument {name} is missing its value"))?;
        if !name.starts_with("--") {
            return Err(format!("expected named argument, found {name}"));
        }
        if values.insert(name.clone(), value.clone()).is_some() {
            return Err(format!("duplicate argument {name}"));
        }
    }
    Ok(values)
}

fn required(values: &BTreeMap<String, String>, name: &str) -> Result<String, String> {
    values
        .get(name)
        .filter(|value| !value.is_empty())
        .cloned()
        .ok_or_else(|| format!("missing required argument {name}"))
}

fn required_path(values: &BTreeMap<String, String>, name: &str) -> Result<PathBuf, String> {
    required(values, name).map(PathBuf::from)
}

fn generate(inputs: &Inputs) -> Result<(), String> {
    validate_hex_identity("source", &inputs.source, 40)?;
    validate_hex_identity("swarm", &inputs.swarm, 40)?;
    validate_hex_identity("merge base", &inputs.merge_base, 40)?;
    validate_hex_identity("preview tree", &inputs.preview_tree, 40)?;
    validate_hex_identity("rejected J5", &inputs.rejected_j5, 40)?;
    validate_hex_identity("rejected J5 tree", &inputs.rejected_j5_tree, 40)?;
    validate_hex_identity("preflight SHA-256", &inputs.preflight_sha256, 64)?;
    validate_hex_identity("P0 artifact SHA-256", &inputs.p0_artifact_sha256, 64)?;

    let preflight_bytes = fs::read(&inputs.preflight).map_err(|error| {
        format!(
            "failed to read preflight {}: {error}",
            inputs.preflight.display()
        )
    })?;
    let actual_preflight_sha256 = sha256(&preflight_bytes);
    if actual_preflight_sha256 != inputs.preflight_sha256 {
        return Err(format!(
            "preflight SHA-256 mismatch: expected {}, found {actual_preflight_sha256}",
            inputs.preflight_sha256
        ));
    }
    let preflight: Value = serde_json::from_slice(&preflight_bytes)
        .map_err(|error| format!("failed to parse preflight JSON: {error}"))?;
    validate_preflight(inputs, &preflight)?;
    validate_git_identity("source", &inputs.source, "commit")?;
    validate_git_identity("swarm", &inputs.swarm, "commit")?;
    validate_git_identity("preview tree", &inputs.preview_tree, "tree")?;
    validate_git_identity("rejected J5", &inputs.rejected_j5, "commit")?;
    validate_git_identity("rejected J5 tree", &inputs.rejected_j5_tree, "tree")?;
    let actual_j5_tree = git_output(&["rev-parse", &format!("{}^{{tree}}", inputs.rejected_j5)])?;
    if actual_j5_tree != inputs.rejected_j5_tree {
        return Err(format!(
            "rejected J5 tree mismatch: expected {}, found {actual_j5_tree}",
            inputs.rejected_j5_tree
        ));
    }

    let reproduced_preview = git_merge_tree(&inputs.source, &inputs.swarm)?;
    let reproduced_preview = reproduced_preview
        .lines()
        .next()
        .ok_or_else(|| "git merge-tree did not emit a preview tree".to_string())?;
    if reproduced_preview != inputs.preview_tree {
        return Err(format!(
            "preview-tree mismatch: expected {}, reproduced {reproduced_preview}",
            inputs.preview_tree
        ));
    }

    let source_policy_bytes = git_bytes(&format!("{}:{POLICY_PATH}", inputs.source))?;
    let swarm_policy_bytes = git_bytes(&format!("{}:{POLICY_PATH}", inputs.swarm))?;
    let source_policy_blob =
        git_output(&["rev-parse", &format!("{}:{POLICY_PATH}", inputs.source)])?;
    let swarm_policy_blob = git_output(&["rev-parse", &format!("{}:{POLICY_PATH}", inputs.swarm)])?;
    let source_rows = parse_policy("source", &source_policy_bytes)?;
    let swarm_rows = parse_policy("swarm", &swarm_policy_bytes)?;
    let decisions = parse_decisions(&inputs.decisions, inputs)?;
    let mut inventory = inventory_tree(&inputs.preview_tree)?;
    complete_parent_inventory(
        &inputs.preview_tree,
        source_rows.iter().chain(swarm_rows.iter()),
        &mut inventory,
    )?;
    let resolutions = reconcile(&source_rows, &swarm_rows, &decisions, &inventory)?;

    let ledger = render_ledger(&resolutions);
    let ledger_blob = git_hash_object(&ledger)?;
    let ledger_sha256 = sha256(ledger.as_bytes());
    let scratch = scratch_root()?;
    prepare_empty_dir(&scratch)?;
    let index = scratch.join("policy-only.index");
    let after_tree = policy_only_tree(&inputs.preview_tree, &ledger_blob, &index)?;
    let changed_paths = changed_paths(&inputs.preview_tree, &after_tree)?;
    if changed_paths != vec![POLICY_PATH.to_string()] {
        return Err(format!(
            "policy-only tree changed unexpected paths: {}",
            changed_paths.join(", ")
        ));
    }

    let checkout = scratch.join("checkout");
    materialize_tree(&after_tree, &index, &checkout)?;
    let source_tree = git_output(&["rev-parse", &format!("{}^{{tree}}", inputs.source)])?;
    let source_checkout = scratch.join("source-checker-checkout");
    materialize_tree(
        &source_tree,
        &scratch.join("source-checker.index"),
        &source_checkout,
    )?;
    let checker_source_blob = git_output(&[
        "rev-parse",
        &format!("{}:xtask/src/policy/network.rs", inputs.source),
    ])?;
    let checker_execution = run_production_checker(
        &source_checkout,
        &checkout,
        &scratch.with_file_name("source-promotion-network-policy-checker-target"),
    )
    .map_err(|error| {
        format!("reconciled policy failed production check-network-policy: {error}")
    })?;

    let source_control = evaluate_ledger(&source_rows, &resolutions)?;
    let swarm_control = evaluate_ledger(&swarm_rows, &resolutions)?;
    let union_rows = source_rows
        .iter()
        .chain(swarm_rows.iter())
        .cloned()
        .collect::<Vec<_>>();
    let union_control = evaluate_ledger(&union_rows, &resolutions)?;
    let controls = build_controls(
        &source_control,
        &swarm_control,
        &union_control,
        &resolutions,
        inputs,
        &preflight,
        &ledger_blob,
        &scratch,
    )?;

    let resolution_rows = resolutions.iter().map(resolution_json).collect::<Vec<_>>();
    let manifest = json!({
        "schema": MANIFEST_SCHEMA,
        "kind": "integrated_policy",
        "key": POLICY_PATH,
        "disposition": "integrated",
        "subject_blob": ledger_blob,
        "receipt_schema": SCHEMA,
        "receipt_subject_tree": after_tree,
    });
    let receipt = json!({
        "schema": SCHEMA,
        "claim": "semantic network-policy resolution for the frozen source/W7 pair",
        "p0": {
            "artifact_sha256": inputs.p0_artifact_sha256,
            "receipt_sha256": inputs.preflight_sha256,
            "source_parent": inputs.source,
            "swarm_parent": inputs.swarm,
            "merge_base": inputs.merge_base,
            "preview_tree_before": inputs.preview_tree,
        },
        "rejected_precedent": {
            "j5": inputs.rejected_j5,
            "j5_tree": inputs.rejected_j5_tree,
        },
        "policy_inputs": {
            "source_blob": source_policy_blob,
            "source_sha256": sha256(&source_policy_bytes),
            "swarm_blob": swarm_policy_blob,
            "swarm_sha256": sha256(&swarm_policy_bytes),
            "reviewer_decisions_sha256": sha256(&fs::read(&inputs.decisions).map_err(|error| format!("failed to read {}: {error}", inputs.decisions.display()))?),
        },
        "rows": resolution_rows,
        "final_ledger": {
            "path": POLICY_PATH,
            "blob": ledger_blob,
            "sha256": ledger_sha256,
            "bytes": ledger.len(),
        },
        "tree_delta": {
            "before": inputs.preview_tree,
            "after": after_tree,
            "changed_paths": changed_paths,
        },
        "production_checker": {
            "command": "cargo xtask check-network-policy",
            "source_parent": inputs.source,
            "source_tree": source_tree,
            "subject_tree": after_tree,
            "source_blob": checker_source_blob,
            "locked": true,
            "offline": true,
            "isolated_target_dir": true,
            "build_exit_code": checker_execution.build_exit_code,
            "build_stdout_sha256": checker_execution.build_stdout_sha256,
            "build_stderr_sha256": checker_execution.build_stderr_sha256,
            "checker_exit_code": checker_execution.checker_exit_code,
            "checker_stdout_sha256": checker_execution.checker_stdout_sha256,
            "checker_stderr_sha256": checker_execution.checker_stderr_sha256,
            "result": "pass",
        },
        "negative_controls": controls,
        "manifest_fragment": manifest,
        "no_ref_mutation": true,
        "non_claims": [
            "does not create the complete resolution manifest",
            "does not create or verify JOIN_TREE, P1, or J6",
            "does not move any source, promotion, tag, release, or publication ref",
            "does not authorize versioning, release, publication, signing, marketplace use, or back-sync"
        ],
    });

    fs::create_dir_all(&inputs.output_dir).map_err(|error| {
        format!(
            "failed to create output directory {}: {error}",
            inputs.output_dir.display()
        )
    })?;
    let ledger_path = inputs.output_dir.join("network-policy-ledger.txt");
    let receipt_path = inputs.output_dir.join("network-policy-resolution.json");
    let markdown_path = inputs.output_dir.join("network-policy-resolution.md");
    let manifest_path = inputs
        .output_dir
        .join("network-policy-manifest-fragment.json");
    fs::write(&ledger_path, &ledger)
        .map_err(|error| format!("failed to write {}: {error}", ledger_path.display()))?;

    let receipt_text = pretty_json(&receipt)?;
    let receipt_sha256 = sha256(receipt_text.as_bytes());
    fs::write(&receipt_path, &receipt_text)
        .map_err(|error| format!("failed to write {}: {error}", receipt_path.display()))?;
    let mut final_manifest = manifest;
    final_manifest["receipt_sha256"] = json!(receipt_sha256);
    fs::write(&manifest_path, pretty_json(&final_manifest)?)
        .map_err(|error| format!("failed to write {}: {error}", manifest_path.display()))?;
    fs::write(
        &markdown_path,
        render_markdown(&receipt, &receipt_path, &ledger_path, &receipt_sha256),
    )
    .map_err(|error| format!("failed to write {}: {error}", markdown_path.display()))?;

    let _ = fs::remove_dir_all(&scratch);
    println!("wrote {}", receipt_path.display());
    println!("ledger blob: {ledger_blob}");
    println!("policy-only tree: {after_tree}");
    Ok(())
}

fn validate_preflight(inputs: &Inputs, preflight: &Value) -> Result<(), String> {
    expect_json_string(preflight, "schema", "ripr.source_promotion_preflight.v1")?;
    expect_json_string(preflight, "source_parent", &inputs.source)?;
    expect_json_string(preflight, "source_main", &inputs.source)?;
    expect_json_string(preflight, "swarm_parent", &inputs.swarm)?;
    expect_json_string(preflight, "swarm_ref_sha", &inputs.swarm)?;
    expect_json_string(preflight, "merge_base", &inputs.merge_base)?;
    let dry_merge = preflight
        .get("dry_merge")
        .ok_or_else(|| "preflight is missing dry_merge".to_string())?;
    expect_json_string(dry_merge, "preview_tree", &inputs.preview_tree)?;
    if dry_merge.get("reviewed_resolved_tree") != Some(&Value::Null)
        || dry_merge.get("reviewed_resolved_tree_verified") != Some(&Value::Bool(false))
    {
        return Err("P0 must remain unfinalized".to_string());
    }
    Ok(())
}

fn expect_json_string(value: &Value, field: &str, expected: &str) -> Result<(), String> {
    let actual = value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or("<missing>");
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "preflight {field} mismatch: expected {expected}, found {actual}"
        ))
    }
}

fn parse_policy(label: &str, bytes: &[u8]) -> Result<Vec<PolicyRow>, String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|error| format!("{label} policy is not UTF-8: {error}"))?;
    let mut rows = Vec::new();
    let mut keys = BTreeSet::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields = line.splitn(5, '|').map(str::trim).collect::<Vec<_>>();
        if fields.len() != 5 || fields.iter().any(|field| field.is_empty()) {
            return Err(format!("{label} policy line {} is malformed", index + 1));
        }
        let [path, pattern, maximum_text, owner, reason] = fields.as_slice() else {
            return Err(format!("{label} policy line {} is malformed", index + 1));
        };
        let maximum = maximum_text.parse::<usize>().map_err(|error| {
            format!(
                "{label} policy line {} has invalid maximum: {error}",
                index + 1
            )
        })?;
        let row = PolicyRow {
            path: (*path).to_string(),
            pattern: (*pattern).to_string(),
            maximum,
            owner: (*owner).to_string(),
            reason: (*reason).to_string(),
        };
        if !keys.insert(row.key()) {
            return Err(format!(
                "{label} policy has duplicate semantic key {}|{}",
                row.path, row.pattern
            ));
        }
        rows.push(row);
    }
    rows.sort_by_key(PolicyRow::key);
    Ok(rows)
}

fn parse_decisions(
    path: &Path,
    inputs: &Inputs,
) -> Result<BTreeMap<(String, String), (PolicyRow, String)>, String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("failed to read decisions {}: {error}", path.display()))?;
    let document: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("failed to parse decisions JSON: {error}"))?;
    expect_json_string(
        &document,
        "schema",
        "ripr.source_promotion_network_policy_decisions.v1",
    )?;
    validate_decision_authority(&document, inputs)?;
    let entries = document
        .get("decisions")
        .and_then(Value::as_array)
        .ok_or_else(|| "decisions JSON is missing decisions array".to_string())?;
    let mut decisions = BTreeMap::new();
    for (index, entry) in entries.iter().enumerate() {
        let string = |field: &str| -> Result<String, String> {
            entry
                .get(field)
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .ok_or_else(|| format!("decision {} is missing {field}", index + 1))
        };
        let maximum = entry
            .get("maximum")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| format!("decision {} has invalid maximum", index + 1))?;
        let row = PolicyRow {
            path: string("path")?,
            pattern: string("pattern")?,
            maximum,
            owner: string("owner")?,
            reason: string("reason")?,
        };
        let rationale = string("rationale")?;
        if decisions
            .insert(row.key(), (row.clone(), rationale))
            .is_some()
        {
            return Err(format!(
                "decisions JSON has duplicate semantic key {}|{}",
                row.path, row.pattern
            ));
        }
    }
    Ok(decisions)
}

fn validate_decision_authority(document: &Value, inputs: &Inputs) -> Result<(), String> {
    expect_json_string(document, "p0_receipt_sha256", &inputs.preflight_sha256)?;
    expect_json_string(document, "p0_artifact_sha256", &inputs.p0_artifact_sha256)?;
    expect_json_string(document, "rejected_j5", &inputs.rejected_j5)?;
    expect_json_string(document, "rejected_j5_tree", &inputs.rejected_j5_tree)
}

fn reconcile(
    source_rows: &[PolicyRow],
    swarm_rows: &[PolicyRow],
    decisions: &BTreeMap<(String, String), (PolicyRow, String)>,
    inventory: &BTreeMap<(String, String), (usize, String)>,
) -> Result<Vec<Resolution>, String> {
    let source = keyed_rows("source", source_rows)?;
    let swarm = keyed_rows("swarm", swarm_rows)?;
    let keys = source
        .keys()
        .chain(swarm.keys())
        .chain(inventory.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    for key in decisions.keys().filter(|key| !keys.contains(*key)) {
        return Err(format!(
            "reviewer decision has no parent or live-preview semantic key: {}|{}",
            key.0, key.1
        ));
    }
    let mut resolutions = Vec::new();
    for key in keys {
        let source_row = source.get(&key).cloned();
        let swarm_row = swarm.get(&key).cloned();
        if (source_row.is_some() || swarm_row.is_some()) && !inventory.contains_key(&key) {
            return Err(format!(
                "parent policy key {}|{} has no exact preview path/count evidence",
                key.0, key.1
            ));
        }
        let (actual_count, evidence_blob) = inventory
            .get(&key)
            .map(|(count, blob)| (*count, Some(blob.clone())))
            .unwrap_or((0, None));
        let decision = decisions.get(&key);
        let metadata_conflict = matches!(
            (source_row.as_ref(), swarm_row.as_ref()),
            (Some(source), Some(swarm))
                if source.owner != swarm.owner || source.reason != swarm.reason
        );
        let decision_required = actual_count > 0
            && ((source_row.is_none() && swarm_row.is_none()) || metadata_conflict);
        if decision.is_some() && !decision_required {
            return Err(format!(
                "reviewer decision is stale or unnecessary for {}|{}",
                key.0, key.1
            ));
        }
        let (selected, disposition, rationale) = select_row(
            source_row.as_ref(),
            swarm_row.as_ref(),
            decision,
            actual_count,
        )?;
        resolutions.push(Resolution {
            source: source_row,
            swarm: swarm_row,
            actual_count,
            evidence_blob,
            selected,
            disposition,
            rationale,
        });
    }
    Ok(resolutions)
}

fn keyed_rows(
    label: &str,
    rows: &[PolicyRow],
) -> Result<BTreeMap<(String, String), PolicyRow>, String> {
    let mut keyed = BTreeMap::new();
    for row in rows {
        if keyed.insert(row.key(), row.clone()).is_some() {
            return Err(format!(
                "{label} policy has duplicate semantic key {}|{}",
                row.path, row.pattern
            ));
        }
    }
    Ok(keyed)
}

fn select_row(
    source: Option<&PolicyRow>,
    swarm: Option<&PolicyRow>,
    decision: Option<&(PolicyRow, String)>,
    actual: usize,
) -> Result<(Option<PolicyRow>, &'static str, String), String> {
    if actual == 0 {
        return Ok((
            None,
            "removed_orphan",
            "exact preview-tree count is zero".to_string(),
        ));
    }
    if source.is_none() && swarm.is_none() {
        let Some((row, rationale)) = decision else {
            return Err(format!(
                "live key has no parent authority and requires an explicit reviewer decision"
            ));
        };
        if row.maximum != actual {
            return Err(format!(
                "new reviewer decision maximum {} must equal exact live count {actual}",
                row.maximum
            ));
        }
        return Ok((Some(row.clone()), "added", rationale.clone()));
    }
    if let (Some(source), Some(swarm)) = (source, swarm)
        && (source.owner != swarm.owner || source.reason != swarm.reason)
    {
        let Some((row, rationale)) = decision else {
            return Err(format!(
                "conflicting owner/reason for {}|{} requires an explicit reviewer decision",
                source.path, source.pattern
            ));
        };
        let reviewed_maximum = [source.maximum, swarm.maximum]
            .into_iter()
            .filter(|maximum| *maximum >= actual)
            .min()
            .ok_or_else(|| {
                format!(
                    "live count {actual} exceeds every reviewed maximum; implicit widening is forbidden"
                )
            })?;
        if row.maximum != reviewed_maximum {
            return Err(format!(
                "reviewer decision maximum {} must preserve narrowest reviewed maximum {reviewed_maximum}",
                row.maximum,
            ));
        }
        return Ok((Some(row.clone()), "conflict_resolved", rationale.clone()));
    }
    let candidates = [source, swarm]
        .into_iter()
        .flatten()
        .filter(|row| row.maximum >= actual)
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Err(format!(
            "live count {actual} exceeds every reviewed maximum; implicit widening is forbidden"
        ));
    }
    let selected = candidates
        .into_iter()
        .min_by_key(|row| row.maximum)
        .cloned()
        .ok_or_else(|| "no selectable policy row".to_string())?;
    let disposition = match (source, swarm) {
        (Some(_), Some(_)) => "retained_shared",
        (Some(_), None) => "retained_source",
        (None, Some(_)) => "retained_swarm",
        (None, None) => return Err("semantic key has no parent row".to_string()),
    };
    Ok((
        Some(selected),
        disposition,
        "narrowest reviewed maximum covers the exact preview-tree count".to_string(),
    ))
}

fn inventory_tree(tree: &str) -> Result<BTreeMap<(String, String), (usize, String)>, String> {
    let patterns = crate::network_policy_patterns();
    let mut counts = BTreeMap::<(String, String), usize>::new();
    for pattern in &patterns {
        let output = git_grep_tree(tree, pattern)?;
        let prefix = format!("{tree}:");
        for line in output.lines().filter(|line| !line.is_empty()) {
            let rest = line.strip_prefix(&prefix).ok_or_else(|| {
                format!("git grep emitted an unexpected tree identity for {pattern}: {line}")
            })?;
            let (path_and_line, _) = rest
                .rsplit_once(':')
                .ok_or_else(|| format!("git grep output is missing match text: {line}"))?;
            let (path, line_number) = path_and_line
                .rsplit_once(':')
                .ok_or_else(|| format!("git grep output is missing a line number: {line}"))?;
            line_number
                .parse::<usize>()
                .map_err(|error| format!("git grep line number is invalid in {line}: {error}"))?;
            *counts
                .entry((path.to_string(), pattern.clone()))
                .or_default() += 1;
        }
    }
    let mut blobs = BTreeMap::<String, String>::new();
    let mut inventory = BTreeMap::new();
    for ((path, pattern), count) in counts {
        let blob = if let Some(blob) = blobs.get(&path) {
            blob.clone()
        } else {
            let blob = git_output(&["rev-parse", &format!("{tree}:{path}")])?;
            blobs.insert(path.clone(), blob.clone());
            blob
        };
        inventory.insert((path, pattern), (count, blob));
    }
    Ok(inventory)
}

fn complete_parent_inventory<'a>(
    tree: &str,
    rows: impl Iterator<Item = &'a PolicyRow>,
    inventory: &mut BTreeMap<(String, String), (usize, String)>,
) -> Result<(), String> {
    let mut blobs = BTreeMap::<String, (Vec<u8>, String)>::new();
    for row in rows {
        let key = row.key();
        if inventory.contains_key(&key) {
            continue;
        }
        let (bytes, blob) = if let Some(value) = blobs.get(&row.path) {
            value.clone()
        } else {
            let spec = format!("{tree}:{}", row.path);
            let bytes = git_bytes(&spec).map_err(|error| {
                format!(
                    "parent policy path {} is absent from exact preview tree {tree}: {error}",
                    row.path
                )
            })?;
            let blob = git_output(&["rev-parse", &spec])?;
            blobs.insert(row.path.clone(), (bytes.clone(), blob.clone()));
            (bytes, blob)
        };
        let pattern = row.pattern.as_bytes();
        let count = if pattern.is_empty() {
            0
        } else {
            bytes
                .windows(pattern.len())
                .filter(|window| *window == pattern)
                .count()
        };
        inventory.insert(key, (count, blob));
    }
    Ok(())
}

fn render_ledger(resolutions: &[Resolution]) -> String {
    let mut ledger = HEADER.to_string();
    for row in resolutions
        .iter()
        .filter_map(|resolution| resolution.selected.as_ref())
    {
        ledger.push_str(&row.line());
        ledger.push('\n');
    }
    ledger
}

fn evaluate_ledger(rows: &[PolicyRow], resolutions: &[Resolution]) -> Result<Value, String> {
    let mut keyed = BTreeMap::new();
    let mut duplicate_keys = BTreeSet::new();
    for row in rows {
        if keyed.insert(row.key(), row.clone()).is_some() {
            duplicate_keys.insert(row.key());
        }
    }
    let mut violations = Vec::new();
    for (path, pattern) in duplicate_keys {
        violations.push(json!({
            "kind": "duplicate_semantic_key",
            "path": path,
            "pattern": pattern,
        }));
    }
    let resolution_keys = resolutions
        .iter()
        .filter_map(|resolution| {
            resolution
                .source
                .as_ref()
                .or(resolution.swarm.as_ref())
                .or(resolution.selected.as_ref())
                .map(PolicyRow::key)
        })
        .collect::<BTreeSet<_>>();
    for key in keyed.keys().filter(|key| !resolution_keys.contains(*key)) {
        violations.push(json!({
            "kind": "unrecognized_semantic_key",
            "path": key.0,
            "pattern": key.1,
        }));
    }
    for resolution in resolutions {
        let key = resolution
            .source
            .as_ref()
            .or(resolution.swarm.as_ref())
            .or(resolution.selected.as_ref())
            .map(PolicyRow::key)
            .ok_or_else(|| "resolution has no semantic key".to_string())?;
        let maximum = keyed.get(&key).map(|row| row.maximum).unwrap_or(0);
        if resolution.actual_count > maximum {
            violations.push(json!({
                "kind": "missing_or_under_counted_live_row",
                "path": key.0,
                "pattern": key.1,
                "actual_count": resolution.actual_count,
                "selected_maximum": maximum,
            }));
        } else if resolution.actual_count == 0 && maximum > 0 {
            violations.push(json!({
                "kind": "orphaned_zero_count_row",
                "path": key.0,
                "pattern": key.1,
                "actual_count": 0,
                "selected_maximum": maximum,
            }));
        }
        if let (Some(candidate), Some(selected)) = (keyed.get(&key), resolution.selected.as_ref()) {
            if candidate.maximum > selected.maximum {
                violations.push(json!({
                    "kind": "implicit_maximum_widening",
                    "path": key.0,
                    "pattern": key.1,
                    "reviewed_maximum": selected.maximum,
                    "candidate_maximum": candidate.maximum,
                }));
            }
            if candidate.owner != selected.owner || candidate.reason != selected.reason {
                violations.push(json!({
                    "kind": "owner_or_reason_substitution",
                    "path": key.0,
                    "pattern": key.1,
                    "reviewed_owner": selected.owner,
                    "candidate_owner": candidate.owner,
                }));
            }
        }
    }
    Ok(json!({
        "status": if violations.is_empty() { "accepted" } else { "rejected" },
        "violations": violations,
    }))
}

fn build_controls(
    source: &Value,
    swarm: &Value,
    union: &Value,
    resolutions: &[Resolution],
    inputs: &Inputs,
    preflight: &Value,
    ledger_blob: &str,
    scratch: &Path,
) -> Result<Value, String> {
    let source_violations = source["violations"]
        .as_array()
        .ok_or_else(|| "source control violations are not an array".to_string())?;
    let source_missing = source_violations
        .iter()
        .filter(|value| value["kind"] == "missing_or_under_counted_live_row")
        .count();
    let source_orphans = source_violations
        .iter()
        .filter(|value| value["kind"] == "orphaned_zero_count_row")
        .count();
    if source_missing != 6 || source_orphans != 0 || source_violations.len() != 6 {
        return Err(format!(
            "fresh raw-source denominator changed: expected 6 missing and 0 orphan, found {source_missing} missing, {source_orphans} orphan, {} total",
            source_violations.len()
        ));
    }
    require_exact_raw_source_violations(source_violations)?;
    let first = resolutions
        .iter()
        .find_map(|resolution| resolution.selected.clone())
        .ok_or_else(|| "no selected row for negative controls".to_string())?;
    let duplicate = evaluate_ledger(&[first.clone(), first.clone()], resolutions)?;
    let final_rows = resolutions
        .iter()
        .filter_map(|resolution| resolution.selected.clone())
        .collect::<Vec<_>>();
    let zero_row = PolicyRow {
        path: first.path.clone(),
        pattern: "ureq".to_string(),
        maximum: 1,
        owner: first.owner.clone(),
        reason: "deliberate zero-count negative control".to_string(),
    };
    let mut zero_resolutions = resolutions.to_vec();
    zero_resolutions.push(Resolution {
        source: Some(zero_row.clone()),
        swarm: None,
        actual_count: 0,
        evidence_blob: None,
        selected: None,
        disposition: "removed_orphan",
        rationale: "deliberate zero-count negative control".to_string(),
    });
    let mut zero_rows = final_rows.clone();
    zero_rows.push(zero_row);
    let zero_count = evaluate_ledger(&zero_rows, &zero_resolutions)?;
    let mut under_rows = final_rows.clone();
    let under_maximum = resolutions
        .iter()
        .find(|resolution| resolution.selected.as_ref().map(PolicyRow::key) == Some(first.key()))
        .map(|resolution| resolution.actual_count.saturating_sub(1))
        .ok_or_else(|| "under-count control could not find selected row".to_string())?;
    let under_row = under_rows
        .first_mut()
        .ok_or_else(|| "under-count control has no candidate row".to_string())?;
    under_row.maximum = under_maximum;
    let under_count = evaluate_ledger(&under_rows, resolutions)?;
    let mut widening_rows = final_rows.clone();
    let widening_row = widening_rows
        .first_mut()
        .ok_or_else(|| "widening control has no candidate row".to_string())?;
    widening_row.maximum = widening_row.maximum.saturating_add(1);
    let widening = evaluate_ledger(&widening_rows, resolutions)?;
    let mut metadata_rows = final_rows;
    let metadata_row = metadata_rows
        .first_mut()
        .ok_or_else(|| "metadata control has no candidate row".to_string())?;
    metadata_row.owner = "conflicting-owner".to_string();
    let metadata = evaluate_ledger(&metadata_rows, resolutions)?;
    require_rejected_control(
        "raw source",
        source,
        "missing_or_under_counted_live_row",
        None,
    )?;
    require_rejected_control("raw W7", swarm, "missing_or_under_counted_live_row", None)?;
    require_rejected_control("raw-line union", union, "duplicate_semantic_key", None)?;
    require_rejected_control(
        "duplicate semantic key",
        &duplicate,
        "duplicate_semantic_key",
        Some(&first.key()),
    )?;
    require_rejected_control(
        "live count above maximum",
        &under_count,
        "missing_or_under_counted_live_row",
        Some(&first.key()),
    )?;
    require_rejected_control(
        "zero-count row",
        &zero_count,
        "orphaned_zero_count_row",
        Some(&(first.path.clone(), "ureq".to_string())),
    )?;
    require_rejected_control(
        "implicit maximum widening",
        &widening,
        "implicit_maximum_widening",
        Some(&first.key()),
    )?;
    require_rejected_control(
        "metadata conflict",
        &metadata,
        "owner_or_reason_substitution",
        Some(&first.key()),
    )?;
    let mut moved_inputs = inputs.clone();
    let replacement = if moved_inputs.source.starts_with('0') {
        '1'
    } else {
        '0'
    };
    moved_inputs
        .source
        .replace_range(..1, &replacement.to_string());
    let identity_error = validate_preflight(&moved_inputs, preflight)
        .err()
        .ok_or_else(|| {
            "changed source identity negative control unexpectedly passed".to_string()
        })?;
    let identity_movement = json!({
        "status": "rejected",
        "changed_field": "source_parent",
        "error": identity_error,
    });
    let outside_policy_path = outside_path_control(
        &inputs.preview_tree,
        ledger_blob,
        &scratch.join("outside-path.index"),
    )?;
    Ok(json!({
        "raw_source": source,
        "raw_source_current_input_correction": "fresh ad291d source has exactly six missing live rows and no stale http orphan; three quoted-push surfaces emerge only under the source checker over the W7 preview, while rejected J5 had the orphan",
        "raw_swarm": swarm,
        "raw_line_union": union,
        "duplicate_semantic_key": duplicate,
        "live_count_above_selected_maximum": under_count,
        "zero_count_row": zero_count,
        "implicit_maximum_widening": widening,
        "metadata_conflict_without_decision": metadata,
        "identity_movement": identity_movement,
        "outside_policy_path": outside_policy_path,
    }))
}

fn require_exact_raw_source_violations(violations: &[Value]) -> Result<(), String> {
    let expected = BTreeSet::from([
        (
            ".github/workflows/server-archive-qualification.yml",
            "curl",
            1_u64,
        ),
        ("crates/ripr/src/lsp/backend.rs", "\"push\"", 1),
        ("crates/ripr/src/lsp/tests.rs", "\"push\"", 1),
        (
            "crates/ripr/src/output/perl_gap_record_projection.rs",
            "curl",
            5,
        ),
        ("xtask/src/branch_inventory.rs", "\"push\"", 2),
        ("xtask/src/tests.rs", "curl", 2),
    ]);
    let actual = violations
        .iter()
        .filter_map(|violation| {
            Some((
                violation.get("path")?.as_str()?,
                violation.get("pattern")?.as_str()?,
                violation.get("actual_count")?.as_u64()?,
            ))
        })
        .collect::<BTreeSet<_>>();
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "fresh raw-source violations changed: expected {expected:?}, found {actual:?}"
        ))
    }
}

fn require_rejected_control(
    label: &str,
    control: &Value,
    expected_kind: &str,
    expected_key: Option<&(String, String)>,
) -> Result<(), String> {
    if control.get("status").and_then(Value::as_str) != Some("rejected") {
        return Err(format!("{label} negative control unexpectedly accepted"));
    }
    let violations = control
        .get("violations")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{label} negative control has no violations"))?;
    let matched = violations.iter().any(|violation| {
        violation.get("kind").and_then(Value::as_str) == Some(expected_kind)
            && expected_key.is_none_or(|key| {
                violation.get("path").and_then(Value::as_str) == Some(key.0.as_str())
                    && violation.get("pattern").and_then(Value::as_str) == Some(key.1.as_str())
            })
    });
    if matched {
        Ok(())
    } else {
        Err(format!(
            "{label} negative control did not report {expected_kind} for the expected subject"
        ))
    }
}

fn outside_path_control(preview: &str, ledger_blob: &str, index: &Path) -> Result<Value, String> {
    let index_text = path_text(index)?;
    git_output_env(&["read-tree", preview], &[("GIT_INDEX_FILE", &index_text)])?;
    for path in [POLICY_PATH, "outside-policy-control.txt"] {
        git_output_env(
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                &format!("100644,{ledger_blob},{path}"),
            ],
            &[("GIT_INDEX_FILE", &index_text)],
        )?;
    }
    let control_tree = git_output_env(&["write-tree"], &[("GIT_INDEX_FILE", &index_text)])?;
    let paths = changed_paths(preview, &control_tree)?;
    if paths
        != vec![
            "outside-policy-control.txt".to_string(),
            POLICY_PATH.to_string(),
        ]
    {
        return Err(format!(
            "outside-path negative control produced unexpected paths: {}",
            paths.join(", ")
        ));
    }
    Ok(json!({
        "status": "rejected",
        "control_tree": control_tree,
        "changed_paths": paths,
        "violation": "changed path outside policy/network_allowlist.txt",
    }))
}

fn resolution_json(resolution: &Resolution) -> Value {
    let subject = resolution
        .source
        .as_ref()
        .or(resolution.swarm.as_ref())
        .or(resolution.selected.as_ref());
    json!({
        "path": subject.map(|row| row.path.as_str()),
        "pattern": subject.map(|row| row.pattern.as_str()),
        "source_row": resolution.source.as_ref().map(PolicyRow::json),
        "swarm_row": resolution.swarm.as_ref().map(PolicyRow::json),
        "actual_count": resolution.actual_count,
        "evidence_blob": resolution.evidence_blob,
        "selected_maximum": resolution.selected.as_ref().map(|row| row.maximum),
        "selected_owner": resolution.selected.as_ref().map(|row| row.owner.as_str()),
        "selected_reason": resolution.selected.as_ref().map(|row| row.reason.as_str()),
        "disposition": resolution.disposition,
        "rationale": resolution.rationale,
    })
}

fn policy_only_tree(preview: &str, blob: &str, index: &Path) -> Result<String, String> {
    let index_text = path_text(index)?;
    git_output_env(&["read-tree", preview], &[("GIT_INDEX_FILE", &index_text)])?;
    git_output_env(
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("100644,{blob},{POLICY_PATH}"),
        ],
        &[("GIT_INDEX_FILE", &index_text)],
    )?;
    git_output_env(&["write-tree"], &[("GIT_INDEX_FILE", &index_text)])
}

fn changed_paths(before: &str, after: &str) -> Result<Vec<String>, String> {
    let output = git_output(&[
        "diff-tree",
        "--no-commit-id",
        "--name-only",
        "-r",
        before,
        after,
    ])?;
    Ok(output
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect())
}

fn materialize_tree(tree: &str, index: &Path, checkout: &Path) -> Result<(), String> {
    fs::create_dir_all(checkout)
        .map_err(|error| format!("failed to create {}: {error}", checkout.display()))?;
    let index_text = path_text(index)?;
    let mut prefix = path_text(checkout)?.replace('\\', "/");
    if !prefix.ends_with('/') {
        prefix.push('/');
    }
    git_output_env(&["read-tree", tree], &[("GIT_INDEX_FILE", &index_text)])?;
    git_output_env(
        &[
            "checkout-index",
            "--all",
            "--force",
            &format!("--prefix={prefix}"),
        ],
        &[("GIT_INDEX_FILE", &index_text)],
    )?;
    git_output_at(checkout, &["init", "--quiet"])?;
    let common_dir = PathBuf::from(git_output(&["rev-parse", "--git-common-dir"])?);
    let common_dir = if common_dir.is_absolute() {
        common_dir
    } else {
        std::env::current_dir()
            .map_err(|error| format!("failed to read current directory: {error}"))?
            .join(common_dir)
    };
    let objects = common_dir.join("objects").canonicalize().map_err(|error| {
        format!(
            "failed to resolve source object directory {}: {error}",
            common_dir.join("objects").display()
        )
    })?;
    let alternates = checkout.join(".git/objects/info/alternates");
    fs::write(
        &alternates,
        format!("{}\n", path_text(&objects)?.replace('\\', "/")),
    )
    .map_err(|error| format!("failed to write {}: {error}", alternates.display()))?;
    git_output_at(checkout, &["read-tree", tree])?;
    Ok(())
}

struct CheckerExecution {
    build_exit_code: i32,
    build_stdout_sha256: String,
    build_stderr_sha256: String,
    checker_exit_code: i32,
    checker_stdout_sha256: String,
    checker_stderr_sha256: String,
}

fn run_production_checker(
    source_checkout: &Path,
    subject_checkout: &Path,
    target_dir: &Path,
) -> Result<CheckerExecution, String> {
    fs::create_dir_all(source_checkout.join("target")).map_err(|error| {
        format!(
            "failed to create source-checkout temporary directory {}: {error}",
            source_checkout.join("target").display()
        )
    })?;
    fs::create_dir_all(target_dir)
        .map_err(|error| format!("failed to create {}: {error}", target_dir.display()))?;
    let build = Command::new("cargo")
        .current_dir(source_checkout)
        .args(["build", "--quiet", "--locked", "--offline", "-p", "xtask"])
        .env("CARGO_TARGET_DIR", target_dir)
        .output()
        .map_err(|error| format!("failed to build exact-source xtask checker: {error}"))?;
    if !build.status.success() {
        return Err(format!(
            "exact-source cargo build exited with {}: stdout={} stderr={}",
            build.status,
            String::from_utf8_lossy(&build.stdout).trim(),
            String::from_utf8_lossy(&build.stderr).trim()
        ));
    }
    let executable =
        target_dir
            .join("debug")
            .join(if cfg!(windows) { "xtask.exe" } else { "xtask" });
    let checker = Command::new(&executable)
        .current_dir(subject_checkout)
        .arg("check-network-policy")
        .output()
        .map_err(|error| format!("failed to start exact-source xtask checker: {error}"))?;
    if !checker.status.success() {
        return Err(format!(
            "exact-source xtask checker exited with {}: stdout={} stderr={}",
            checker.status,
            String::from_utf8_lossy(&checker.stdout).trim(),
            String::from_utf8_lossy(&checker.stderr).trim()
        ));
    }
    Ok(CheckerExecution {
        build_exit_code: build.status.code().unwrap_or(-1),
        build_stdout_sha256: sha256(&build.stdout),
        build_stderr_sha256: sha256(&build.stderr),
        checker_exit_code: checker.status.code().unwrap_or(-1),
        checker_stdout_sha256: sha256(&checker.stdout),
        checker_stderr_sha256: sha256(&checker.stderr),
    })
}

fn scratch_root() -> Result<PathBuf, String> {
    let root = std::env::current_dir()
        .map_err(|error| format!("failed to read current directory: {error}"))?
        .join("target")
        .join("ripr")
        .join(format!(
            "source-promotion-network-policy-resolution-{}",
            std::process::id()
        ));
    Ok(root)
}

fn prepare_empty_dir(path: &Path) -> Result<(), String> {
    let current = std::env::current_dir()
        .map_err(|error| format!("failed to read current directory: {error}"))?;
    let allowed = current.join("target").join("ripr");
    if !path.starts_with(&allowed) {
        return Err(format!(
            "refusing to clean scratch path outside {}: {}",
            allowed.display(),
            path.display()
        ));
    }
    if path.exists() {
        fs::remove_dir_all(path)
            .map_err(|error| format!("failed to clean {}: {error}", path.display()))?;
    }
    fs::create_dir_all(path)
        .map_err(|error| format!("failed to create {}: {error}", path.display()))
}

fn validate_git_identity(label: &str, object: &str, expected_type: &str) -> Result<(), String> {
    let actual = git_output(&["cat-file", "-t", object])?;
    if actual == expected_type {
        Ok(())
    } else {
        Err(format!(
            "{label} {object} has Git type {actual}, expected {expected_type}"
        ))
    }
}

fn validate_hex_identity(label: &str, value: &str, length: usize) -> Result<(), String> {
    if value.len() == length && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(format!(
            "{label} must be exactly {length} hexadecimal characters"
        ))
    }
}

fn git_hash_object(contents: &str) -> Result<String, String> {
    let mut child = Command::new("git")
        .args(["hash-object", "-w", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("failed to start git hash-object: {error}"))?;
    let Some(mut stdin) = child.stdin.take() else {
        return Err("git hash-object stdin was unavailable".to_string());
    };
    stdin
        .write_all(contents.as_bytes())
        .map_err(|error| format!("failed to write git hash-object stdin: {error}"))?;
    drop(stdin);
    output_text(
        child
            .wait_with_output()
            .map_err(|error| format!("failed to wait for git hash-object: {error}"))?,
    )
}

fn git_bytes(spec: &str) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .args(["show", spec])
        .output()
        .map_err(|error| format!("failed to run git show {spec}: {error}"))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

fn git_output(args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .output()
        .map_err(|error| format!("failed to run git {}: {error}", args.join(" ")))?;
    output_text(output)
}

fn git_merge_tree(source: &str, swarm: &str) -> Result<String, String> {
    let output = Command::new("git")
        .args(["merge-tree", "--write-tree", source, swarm])
        .output()
        .map_err(|error| format!("failed to run git merge-tree: {error}"))?;
    if output.status.success() || output.status.code() == Some(1) {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        output_text(output)
    }
}

fn git_grep_tree(tree: &str, pattern: &str) -> Result<String, String> {
    let output = Command::new("git")
        .args([
            "grep", "-I", "-F", "-o", "-n", "-e", pattern, tree, "--", "*.rs", "*.ts", "*.yml",
            "*.yaml",
        ])
        .output()
        .map_err(|error| format!("failed to run git grep for {pattern}: {error}"))?;
    if output.status.success() || output.status.code() == Some(1) {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        output_text(output)
    }
}

fn git_output_at(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .map_err(|error| format!("failed to run git {}: {error}", args.join(" ")))?;
    output_text(output)
}

fn git_output_env(args: &[&str], env: &[(&str, &str)]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .envs(env.iter().copied())
        .output()
        .map_err(|error| format!("failed to run git {}: {error}", args.join(" ")))?;
    output_text(output)
}

fn output_text(output: std::process::Output) -> Result<String, String> {
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(format!(
            "command failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn path_text(path: &Path) -> Result<String, String> {
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| format!("path is not valid UTF-8: {}", path.display()))
}

fn portable_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn pretty_json(value: &Value) -> Result<String, String> {
    let mut text = serde_json::to_string_pretty(value)
        .map_err(|error| format!("failed to serialize resolution JSON: {error}"))?;
    text.push('\n');
    Ok(text)
}

fn render_markdown(
    receipt: &Value,
    receipt_path: &Path,
    ledger_path: &Path,
    receipt_sha256: &str,
) -> String {
    format!(
        "# Source/W7 semantic network-policy resolution\n\n- Schema: `{}`\n- Source parent: `{}`\n- W7: `{}`\n- P0 receipt SHA-256: `{}`\n- Preview tree before: `{}`\n- Policy-only tree after: `{}`\n- Final ledger blob: `{}`\n- Final ledger SHA-256: `{}`\n- Production checker: **pass**\n- Changed path: `{}` only\n- Receipt SHA-256: `{}`\n\nMachine-readable receipt: `{}`\nLedger bytes: `{}`\n\nThis control does not create the complete manifest, JOIN_TREE, P1, or J6 and moves no ref.\n",
        SCHEMA,
        receipt["p0"]["source_parent"].as_str().unwrap_or("unknown"),
        receipt["p0"]["swarm_parent"].as_str().unwrap_or("unknown"),
        receipt["p0"]["receipt_sha256"]
            .as_str()
            .unwrap_or("unknown"),
        receipt["tree_delta"]["before"]
            .as_str()
            .unwrap_or("unknown"),
        receipt["tree_delta"]["after"].as_str().unwrap_or("unknown"),
        receipt["final_ledger"]["blob"]
            .as_str()
            .unwrap_or("unknown"),
        receipt["final_ledger"]["sha256"]
            .as_str()
            .unwrap_or("unknown"),
        POLICY_PATH,
        receipt_sha256,
        portable_path(receipt_path),
        portable_path(ledger_path),
    )
}

#[cfg(test)]
mod tests {
    use super::{
        Inputs, PolicyRow, evaluate_ledger, parse_policy, reconcile, require_rejected_control,
        select_row, validate_decision_authority,
    };
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn row(path: &str, pattern: &str, maximum: usize, owner: &str) -> PolicyRow {
        PolicyRow {
            path: path.to_string(),
            pattern: pattern.to_string(),
            maximum,
            owner: owner.to_string(),
            reason: "reviewed reason".to_string(),
        }
    }

    #[test]
    fn semantic_reconciliation_retains_narrowest_reviewed_rows_and_drops_orphans()
    -> Result<(), String> {
        let source = vec![
            row("shared.rs", "curl", 4, "shared"),
            row("source.rs", "curl", 6, "source"),
        ];
        let swarm = vec![
            row("shared.rs", "curl", 5, "shared"),
            row("swarm.rs", "curl", 2, "swarm"),
            row("orphan.rs", "curl", 1, "swarm"),
        ];
        let inventory = BTreeMap::from([
            (
                ("shared.rs".to_string(), "curl".to_string()),
                (4, "blob".to_string()),
            ),
            (
                ("source.rs".to_string(), "curl".to_string()),
                (6, "blob".to_string()),
            ),
            (
                ("swarm.rs".to_string(), "curl".to_string()),
                (2, "blob".to_string()),
            ),
            (
                ("orphan.rs".to_string(), "curl".to_string()),
                (0, "orphan-blob".to_string()),
            ),
        ]);
        let resolved = reconcile(&source, &swarm, &BTreeMap::new(), &inventory)?;
        let selected = resolved
            .iter()
            .filter_map(|item| item.selected.as_ref())
            .collect::<Vec<_>>();
        if selected.len() != 3
            || selected.first().map(|row| row.maximum) != Some(4)
            || resolved
                .iter()
                .filter(|item| item.disposition == "removed_orphan")
                .count()
                != 1
        {
            return Err(
                "reconciliation did not retain three narrow rows and remove one orphan".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn duplicate_semantic_key_is_rejected() -> Result<(), String> {
        let text = b"a.rs|curl|1|owner|reason\na.rs|curl|2|owner|reason\n";
        let error = parse_policy("fixture", text)
            .err()
            .ok_or_else(|| "duplicate fixture unexpectedly passed".to_string())?;
        if !error.contains("duplicate semantic key") {
            return Err(format!("unexpected error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn under_count_and_metadata_conflict_are_rejected_without_widening_or_decision()
    -> Result<(), String> {
        let source = row("a.rs", "curl", 1, "source");
        if select_row(Some(&source), None, None, 2).is_ok() {
            return Err("implicit widening unexpectedly passed".to_string());
        }
        let swarm = row("a.rs", "curl", 1, "swarm");
        if select_row(Some(&source), Some(&swarm), None, 1).is_ok() {
            return Err("metadata conflict unexpectedly passed".to_string());
        }
        let widened = row("a.rs", "curl", 2, "reviewer");
        if select_row(
            Some(&source),
            Some(&swarm),
            Some(&(widened, "explicit review".to_string())),
            1,
        )
        .is_ok()
        {
            return Err("metadata decision implicitly widened the maximum".to_string());
        }
        Ok(())
    }

    #[test]
    fn raw_union_and_zero_count_rows_are_negative_controls() -> Result<(), String> {
        let source = row("a.rs", "curl", 1, "owner");
        let inventory = BTreeMap::from([(
            ("a.rs".to_string(), "curl".to_string()),
            (1, "blob".to_string()),
        )]);
        let resolved = reconcile(
            std::slice::from_ref(&source),
            &[],
            &BTreeMap::new(),
            &inventory,
        )?;
        let union = vec![source.clone(), source];
        let report = evaluate_ledger(&union, &resolved)?;
        if report["status"] != "rejected" {
            return Err("raw union duplicate unexpectedly passed".to_string());
        }
        let zero = row("a.rs", "curl", 1, "owner");
        let selection = select_row(Some(&zero), None, None, 0)?;
        if selection.1 != "removed_orphan" {
            return Err("zero-count row was not removed".to_string());
        }
        Ok(())
    }

    #[test]
    fn new_live_key_requires_exact_reviewer_decision() -> Result<(), String> {
        let inventory = BTreeMap::from([(
            ("new.rs".to_string(), "curl".to_string()),
            (2, "blob".to_string()),
        )]);
        if reconcile(&[], &[], &BTreeMap::new(), &inventory).is_ok() {
            return Err("new live key passed without reviewer decision".to_string());
        }
        let decision_row = row("new.rs", "curl", 3, "reviewer");
        let wrong = BTreeMap::from([(
            decision_row.key(),
            (decision_row, "explicit review".to_string()),
        )]);
        if reconcile(&[], &[], &wrong, &inventory).is_ok() {
            return Err("widened reviewer decision unexpectedly passed".to_string());
        }
        let stale_row = row("stale.rs", "curl", 1, "reviewer");
        let stale = BTreeMap::from([(stale_row.key(), (stale_row, "stale review".to_string()))]);
        if reconcile(&[], &[], &stale, &inventory).is_ok() {
            return Err("stale reviewer decision unexpectedly passed".to_string());
        }
        let decision_row = row("new.rs", "curl", 2, "reviewer");
        let exact = BTreeMap::from([(
            decision_row.key(),
            (decision_row, "explicit review".to_string()),
        )]);
        let resolved = reconcile(&[], &[], &exact, &inventory)?;
        if resolved.len() != 1 || resolved.first().map(|item| item.disposition) != Some("added") {
            return Err("exact reviewer decision was not selected as added".to_string());
        }
        Ok(())
    }

    #[test]
    fn parent_rows_require_exact_path_and_count_evidence() -> Result<(), String> {
        let parent = row("missing.rs", "curl", 1, "owner");
        let error = reconcile(&[parent], &[], &BTreeMap::new(), &BTreeMap::new())
            .err()
            .ok_or_else(|| "parent row without preview evidence unexpectedly passed".to_string())?;
        if !error.contains("no exact preview path/count evidence") {
            return Err(format!("unexpected error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn reviewer_authority_binds_artifact_and_rejected_precedent() -> Result<(), String> {
        let inputs = Inputs {
            preflight: PathBuf::from("preflight.json"),
            decisions: PathBuf::from("decisions.json"),
            preflight_sha256: "a".repeat(64),
            p0_artifact_sha256: "b".repeat(64),
            source: "c".repeat(40),
            swarm: "d".repeat(40),
            merge_base: "e".repeat(40),
            preview_tree: "f".repeat(40),
            rejected_j5: "1".repeat(40),
            rejected_j5_tree: "2".repeat(40),
            output_dir: PathBuf::from("out"),
        };
        let document = json!({
            "p0_receipt_sha256": inputs.preflight_sha256,
            "p0_artifact_sha256": inputs.p0_artifact_sha256,
            "rejected_j5": inputs.rejected_j5,
            "rejected_j5_tree": inputs.rejected_j5_tree,
        });
        validate_decision_authority(&document, &inputs)?;
        for field in [
            "p0_receipt_sha256",
            "p0_artifact_sha256",
            "rejected_j5",
            "rejected_j5_tree",
        ] {
            let mut changed = document.clone();
            changed[field] = json!("0");
            if validate_decision_authority(&changed, &inputs).is_ok() {
                return Err(format!("changed {field} unexpectedly passed"));
            }
        }
        Ok(())
    }

    #[test]
    fn negative_controls_must_reject_the_expected_subject() -> Result<(), String> {
        let key = ("a.rs".to_string(), "curl".to_string());
        let rejected = json!({
            "status": "rejected",
            "violations": [{"kind": "orphaned_zero_count_row", "path": "a.rs", "pattern": "curl"}],
        });
        require_rejected_control("fixture", &rejected, "orphaned_zero_count_row", Some(&key))?;
        let accepted = json!({"status": "accepted", "violations": []});
        if require_rejected_control("fixture", &accepted, "orphaned_zero_count_row", Some(&key))
            .is_ok()
        {
            return Err("accepted negative control unexpectedly passed".to_string());
        }
        Ok(())
    }
}
