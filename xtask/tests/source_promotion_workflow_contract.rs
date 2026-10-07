use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn workflow_text() -> Result<String, String> {
    let xtask = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = xtask
        .parent()
        .ok_or_else(|| "xtask manifest directory has no repository parent".to_string())?;
    fs::read_to_string(root.join(".github/workflows/source-promotion-contract.yml"))
        .map_err(|error| format!("read source-promotion contract workflow: {error}"))
}

fn admission_workflow_text() -> Result<String, String> {
    let xtask = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = xtask
        .parent()
        .ok_or_else(|| "xtask manifest directory has no repository parent".to_string())?;
    fs::read_to_string(root.join(".github/workflows/source-promotion-admission.yml"))
        .map_err(|error| format!("read source-promotion admission workflow: {error}"))
}

fn git_output(
    cwd: &Path,
    args: &[&str],
    environment: &[(&str, &str)],
    input: Option<&[u8]>,
) -> Result<String, String> {
    let mut child = Command::new("git")
        .current_dir(cwd)
        .args(args)
        .envs(environment.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("failed to start git {args:?}: {error}"))?;
    if let Some(bytes) = input {
        child
            .stdin
            .take()
            .ok_or_else(|| "git child has no stdin".to_string())?
            .write_all(bytes)
            .map_err(|error| format!("failed to write git stdin: {error}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|error| format!("failed to wait for git {args:?}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_string())
        .map_err(|error| format!("git {args:?} output was not UTF-8: {error}"))
}

fn path_text(path: &Path) -> Result<&str, String> {
    path.to_str()
        .ok_or_else(|| format!("path is not UTF-8: {}", path.display()))
}

fn j5_request_identity(repo_root: &Path, scratch: &Path) -> Result<Value, String> {
    let source_parent = git_output(repo_root, &["rev-parse", "HEAD"], &[], None)?;
    let source_tree = git_output(
        repo_root,
        &["rev-parse", &format!("{source_parent}^{{tree}}")],
        &[],
        None,
    )?;
    let identity_repo = scratch.join("identity-repository");
    git_output(
        repo_root,
        &[
            "clone",
            "--config",
            "core.longpaths=true",
            "--local",
            "--no-hardlinks",
            "--no-checkout",
            "--quiet",
            path_text(repo_root)?,
            path_text(&identity_repo)?,
        ],
        &[],
        None,
    )?;
    git_output(
        &identity_repo,
        &["checkout", "--quiet", "--detach", &source_parent],
        &[],
        None,
    )?;
    let index = scratch.join("j5.index");
    let index_text = path_text(&index)?;
    let index_env = [("GIT_INDEX_FILE", index_text)];
    git_output(
        &identity_repo,
        &["read-tree", &source_tree],
        &index_env,
        None,
    )?;
    let inherited = git_output(
        &identity_repo,
        &["show", "HEAD:policy/network_allowlist.txt"],
        &[],
        None,
    )?;
    // Mirrors the production fixture: the J5 control must not inherit coverage
    // for its own synthetic surfaces from whatever ledger the current HEAD
    // carries, or the negative control silently becomes a passing one under a
    // reconciled source/W7 join ledger.
    let uncovered = [
        ".github/workflows/server-archive-qualification.yml|",
        "crates/ripr/src/output/perl_gap_record_projection.rs|",
        "xtask/src/tests.rs|",
    ];
    let mut ledger = inherited
        .lines()
        .filter(|line| !uncovered.iter().any(|prefix| line.starts_with(prefix)))
        .map(|line| format!("{line}\n"))
        .collect::<String>();
    ledger.push_str(
        ".github/workflows/stale-network-surface.yml|curl|3|source|stale zero-count row\n",
    );
    let repeated = |literal: &str, count: usize| {
        let mut value = std::iter::repeat_n(literal, count)
            .collect::<Vec<_>>()
            .join("\n");
        value.push('\n');
        value
    };
    for (path, contents) in [
        ("policy/network_allowlist.txt", ledger),
        (
            ".github/workflows/server-archive-qualification.yml",
            "name: server archive qualification\n# retained J5 fixture: curl\n".to_string(),
        ),
        (
            "crates/ripr/src/output/perl_gap_record_projection.rs",
            repeated("// retained J5 fixture: curl", 5),
        ),
        (
            "xtask/src/tests.rs",
            repeated("// retained J5 fixture: curl", 2),
        ),
        (
            ".github/workflows/stale-network-surface.yml",
            "name: stale network surface\n".to_string(),
        ),
    ] {
        let blob = git_output(
            &identity_repo,
            &["hash-object", "-w", "--stdin"],
            &[],
            Some(contents.as_bytes()),
        )?;
        let cache = format!("100644,{blob},{path}");
        git_output(
            &identity_repo,
            &["update-index", "--add", "--cacheinfo", &cache],
            &index_env,
            None,
        )?;
    }
    let reviewed_tree = git_output(&identity_repo, &["write-tree"], &index_env, None)?;
    let fixed = [
        ("GIT_AUTHOR_NAME", "RIPR Source Promotion Fixture"),
        ("GIT_AUTHOR_EMAIL", "source-promotion-fixture@invalid"),
        ("GIT_AUTHOR_DATE", "2000-01-01T00:00:00+00:00"),
        ("GIT_COMMITTER_NAME", "RIPR Source Promotion Fixture"),
        ("GIT_COMMITTER_EMAIL", "source-promotion-fixture@invalid"),
        ("GIT_COMMITTER_DATE", "2000-01-01T00:00:00+00:00"),
    ];
    let swarm_parent = git_output(
        &identity_repo,
        &["commit-tree", &reviewed_tree, "-p", &source_parent],
        &fixed,
        Some(b"test(promotion): deterministic protected W7 fixture\n"),
    )?;
    Ok(json!({
        "schema": "ripr.source_promotion_admission_request.v1",
        "source_repository": "EffortlessMetrics/ripr",
        "source_parent_sha": source_parent,
        "workflow_source_sha": source_parent,
        "trusted_checker_identity": format!("source-owned-xtask@{source_parent}"),
        "swarm_repository": "EffortlessMetrics/ripr-swarm",
        "protected_w7_ref": format!("refs/tags/ripr-release-0.11.0-{swarm_parent}"),
        "w7_peeled_sha": swarm_parent,
        "reviewed_tree_sha": reviewed_tree,
        "reviewed_tree_carrier_sha": "not_required",
        "preflight_locator": "",
        "resolution_manifest_locator": "",
        "validation_packet_locator": "",
        "integration_packet_locator": "",
        "qualification_receipt_locator": "",
        "receipt_schema": "ripr.source_promotion_admission_workflow.v1",
        "operation_mode": "constructor_dry_run",
        "execution_profile": "j5_negative",
    }))
}

fn require_fragment(text: &str, fragment: &str) -> Result<(), String> {
    if text.contains(fragment) {
        Ok(())
    } else {
        Err(format!("workflow contract missing fragment: {fragment}"))
    }
}

fn require_absent(text: &str, fragment: &str) -> Result<(), String> {
    if text.contains(fragment) {
        Err(format!(
            "workflow contract exposes forbidden fragment: {fragment}"
        ))
    } else {
        Ok(())
    }
}

fn require_order(text: &str, before: &str, after: &str) -> Result<(), String> {
    let before_offset = text
        .find(before)
        .ok_or_else(|| format!("workflow contract missing ordered fragment: {before}"))?;
    let after_offset = text
        .find(after)
        .ok_or_else(|| format!("workflow contract missing ordered fragment: {after}"))?;
    if before_offset < after_offset {
        Ok(())
    } else {
        Err(format!(
            "workflow contract orders {after:?} before {before:?}"
        ))
    }
}

fn validate_admission_workflow_contract(workflow: &str) -> Result<(), String> {
    let enforcement_calls: Vec<_> = workflow
        .split("source-promotion enforce-admission-workflow \\")
        .skip(1)
        .map(|tail| {
            tail.lines()
                .skip(1)
                .take(3)
                .map(str::trim)
                .collect::<Vec<_>>()
        })
        .collect();
    if enforcement_calls.len() != 2 {
        return Err("expected two admission enforcement commands".to_string());
    }
    for (call, packet) in enforcement_calls.iter().zip([
        "$ADMISSION_ROOT/downloaded/workflow-packet",
        "$ADMISSION_FINAL_EVIDENCE",
    ]) {
        if *call
            != [
                format!("--packet \"{packet}\" \\"),
                "--workspace-root \"$ADMISSION_ROOT\" \\".to_string(),
                "--expected-status admitted".to_string(),
            ]
        {
            return Err("each enforcement command requires its owned workspace root".to_string());
        }
    }
    for required in [
        "name: Source Promotion Admission",
        "  workflow_call:",
        "  workflow_dispatch:",
        "permissions:\n  contents: read",
        "      contents: read",
        "github.repository == 'EffortlessMetrics/ripr' &&",
        "github.ref == 'refs/heads/main' &&",
        "inputs.source_repository == 'EffortlessMetrics/ripr'",
        "runs-on: ubuntu-latest",
        "admission_root=\"$RUNNER_TEMP/ripr-source-promotion-admission\"",
        "\"ADMISSION_OUT=$admission_out\"",
        "\"ADMISSION_EVIDENCE=$admission_evidence\"",
        "\"ADMISSION_FINAL_EVIDENCE=$admission_final_evidence\"",
        "\"ADMISSION_WORKSPACE=$admission_workspace\"",
        "persist-credentials: false",
        "repository: ${{ job.workflow_repository }}",
        "ref: ${{ job.workflow_sha }}",
        "WORKFLOW_FILE_SHA: ${{ job.workflow_sha }}",
        "WORKFLOW_FILE_REF: ${{ job.workflow_ref }}",
        "test \"$WORKFLOW_FILE_SHA\" = \"$WORKFLOW_SOURCE_SHA\"",
        "test \"$REVIEWED_TREE_CARRIER_SHA\" = not_required",
        "\"$SOURCE_REPOSITORY/.github/workflows/source-promotion-admission.yml@\"*",
        "test \"$GITHUB_REF\" = refs/heads/main",
        "test \"$GITHUB_SHA\" = \"$SOURCE_PARENT_SHA\"",
        "test \"$WORKFLOW_SOURCE_SHA\" = \"$SOURCE_PARENT_SHA\"",
        "test \"$(git rev-parse HEAD)\" = \"$WORKFLOW_SOURCE_SHA\"",
        "ripr.source_promotion_admission_workflow.v1",
        "ripr.source_promotion_admission_request.v1",
        "@[0-9a-f]{40}:[A-Za-z0-9._/-]+#sha256:[0-9a-f]{64}",
        "source-promotion run-admission-workflow",
        "source-promotion verify-admission-workflow",
        "source-promotion enforce-admission-workflow",
        "source-promotion finalize-admission-workflow",
        "if: always() && steps.enforce.outcome == 'success'",
        "--admission-packet \"$ADMISSION_ROOT/downloaded/workflow-packet\"",
        "--out \"$ADMISSION_FINAL_EVIDENCE\"",
        "actions/download-artifact@3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c # v8",
        "sha256sum --check transport-index.sha256",
        "diff --recursive --no-dereference \"$ADMISSION_OUT\" \"$downloaded\"",
        "test \"$TRUSTED_CHECKER_IDENTITY\" = \"source-owned-xtask@$WORKFLOW_SOURCE_SHA\"",
        "^refs/tags/ripr-release-",
        "printf '%s\\n' \"$producer_exit\" > \"$ADMISSION_OUT/producer-exit-code.txt\"",
        "tail -c 1048576 \"$log\"",
        "\"truncated=$truncated\"",
        "\"original_bytes=$original_bytes\"",
        "identity_digest=$(sha256sum \"$admission_out/requested-identity.json\" | awk '{print $1}')",
        "\"identity_digest=$identity_digest\"",
        "--requested-identity \"$ADMISSION_OUT/requested-identity.json\"",
        "--requested-identity \"$downloaded/requested-identity.json\"",
        "--requested-identity-sha256 \"${{ steps.initialize.outputs.identity_digest }}\"",
        "artifact_name=source-promotion-admission-v1-$identity_digest-$GITHUB_RUN_ID-$GITHUB_RUN_ATTEMPT",
        "final_artifact_name=source-promotion-admission-final-v1-$identity_digest-$GITHUB_RUN_ID-$GITHUB_RUN_ATTEMPT",
        "name: ${{ steps.initialize.outputs.artifact_name }}",
        "name: ${{ steps.initialize.outputs.final_artifact_name }}",
        "test \"${{ steps.producer.outcome }}\" = success",
        "test \"${{ steps.finalize.outcome }}\" = success",
        "test -d \"$downloaded/workflow-packet\"",
        "test -d \"$ADMISSION_FINAL_EVIDENCE\"",
        "--expected-status admitted",
        "if-no-files-found: error",
    ] {
        require_fragment(workflow, required)?;
    }

    for input in [
        "source_repository",
        "source_parent_sha",
        "workflow_source_sha",
        "swarm_repository",
        "protected_w7_ref",
        "w7_peeled_sha",
        "reviewed_tree_sha",
        "reviewed_tree_carrier_sha",
        "preflight_locator",
        "resolution_manifest_locator",
        "validation_packet_locator",
        "integration_packet_locator",
        "qualification_receipt_locator",
        "receipt_schema",
        "operation_mode",
        "execution_profile",
        "trusted_checker_identity",
    ] {
        require_fragment(workflow, &format!("      {input}:"))?;
        require_fragment(workflow, &format!("--{}", input.replace('_', "-")))?;
    }
    require_fragment(workflow, "inputs: &admission_inputs")?;
    require_fragment(workflow, "inputs: *admission_inputs")?;
    for required_input in [
        "source_repository",
        "source_parent_sha",
        "workflow_source_sha",
        "swarm_repository",
        "protected_w7_ref",
        "w7_peeled_sha",
        "reviewed_tree_sha",
        "reviewed_tree_carrier_sha",
        "receipt_schema",
        "operation_mode",
        "execution_profile",
        "trusted_checker_identity",
    ] {
        let declaration = workflow
            .lines()
            .find(|line| line.trim_start().starts_with(&format!("{required_input}:")))
            .ok_or_else(|| format!("missing input declaration: {required_input}"))?;
        require_fragment(declaration, "required: true")?;
    }
    for optional_fixture_locator in [
        "preflight_locator",
        "resolution_manifest_locator",
        "validation_packet_locator",
        "integration_packet_locator",
        "qualification_receipt_locator",
    ] {
        let declaration = workflow
            .lines()
            .find(|line| {
                line.trim_start()
                    .starts_with(&format!("{optional_fixture_locator}:"))
            })
            .ok_or_else(|| format!("missing locator declaration: {optional_fixture_locator}"))?;
        require_fragment(declaration, "required: false")?;
        require_fragment(declaration, "default: ''")?;
    }

    for closed_value in [
        "admit_only",
        "constructor_dry_run",
        "positive_synthetic",
        "j5_negative",
    ] {
        require_fragment(workflow, closed_value)?;
    }

    for forbidden in [
        "pull_request_target:",
        "pull_request:",
        "contents: write",
        "pull-requests: write",
        "id-token: write",
        "attestations: write",
        "packages: write",
        "environment:",
        "self-hosted",
        "target/ripr-source-promotion-admission",
        "source-promotion publish-candidate-ref",
        "repository: ${{ inputs.source_repository }}",
        "ref: ${{ inputs.workflow_source_sha }}",
        "gh release",
        "git push",
    ] {
        require_absent(workflow, forbidden)?;
    }

    require_order(
        workflow,
        "- name: Run production admission controller and capture producer exit",
        "- name: Upload complete pre-enforcement evidence",
    )?;
    require_order(
        workflow,
        "- name: Upload complete pre-enforcement evidence",
        "- name: Download pre-enforcement evidence into a fresh runner-owned root",
    )?;
    require_order(
        workflow,
        "- name: Download pre-enforcement evidence into a fresh runner-owned root",
        "- name: Independently verify downloaded and local evidence identity",
    )?;
    require_order(
        workflow,
        "- name: Independently verify downloaded and local evidence identity",
        "- name: Enforce terminal admission before constructor",
    )?;
    require_order(
        workflow,
        "- name: Enforce terminal admission before constructor",
        "- name: Finalize guarded constructor disposition after admission",
    )?;
    require_order(
        workflow,
        "- name: Finalize guarded constructor disposition after admission",
        "- name: Upload final normalized workflow disposition",
    )?;
    require_order(
        workflow,
        "- name: Upload final normalized workflow disposition",
        "- name: Enforce final normalized workflow disposition",
    )?;

    for upload_name in [
        "- name: Upload complete pre-enforcement evidence",
        "- name: Upload final normalized workflow disposition",
    ] {
        let upload = workflow
            .split_once(upload_name)
            .map(|(_, suffix)| suffix)
            .ok_or_else(|| format!("missing upload step: {upload_name}"))?;
        let step = upload.split("\n      - name:").next().unwrap_or(upload);
        require_fragment(step, "if: always()")?;
        for raw_input in [
            "inputs.source_repository",
            "inputs.source_parent_sha",
            "inputs.workflow_source_sha",
            "inputs.swarm_repository",
            "inputs.protected_w7_ref",
            "inputs.w7_peeled_sha",
            "inputs.reviewed_tree_sha",
            "inputs.reviewed_tree_carrier_sha",
            "inputs.receipt_schema",
            "inputs.operation_mode",
            "inputs.execution_profile",
            "inputs.trusted_checker_identity",
        ] {
            require_absent(step, raw_input)?;
        }
    }

    let producer_step = workflow
        .split_once("- name: Run production admission controller and capture producer exit")
        .map(|(_, suffix)| suffix.split("\n      - name:").next().unwrap_or(suffix))
        .ok_or_else(|| "missing producer step".to_string())?;
    for required in ["continue-on-error: true", "exit \"$producer_exit\""] {
        require_fragment(producer_step, required)?;
    }
    let finalizer_step = workflow
        .split_once("- name: Finalize guarded constructor disposition after admission")
        .map(|(_, suffix)| suffix.split("\n      - name:").next().unwrap_or(suffix))
        .ok_or_else(|| "missing finalizer step".to_string())?;
    for required in ["continue-on-error: true", "exit \"$finalizer_exit\""] {
        require_fragment(finalizer_step, required)?;
    }
    Ok(())
}

fn jq_filter_matches(filter: &str, value: &Value) -> Result<bool, String> {
    let mut child = Command::new("jq")
        .args(["-e", filter])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("start jq for source-promotion contract test: {error}"))?;
    let input = serde_json::to_vec(value)
        .map_err(|error| format!("serialize jq contract fixture: {error}"))?;
    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| "jq contract test has no stdin".to_string())?;
        stdin
            .write_all(&input)
            .map_err(|error| format!("write jq contract fixture: {error}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|error| format!("wait for jq contract test: {error}"))?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        code => Err(format!(
            "jq contract filter failed with status {code:?}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )),
    }
}

fn workflow_disposition_is_authorized(manifest: &Value, path: &str) -> bool {
    let Some(dispositions) = manifest.get("dispositions").and_then(Value::as_array) else {
        return false;
    };
    let matching = dispositions
        .iter()
        .filter(|row| row.get("key").and_then(Value::as_str) == Some(path))
        .collect::<Vec<_>>();
    !matching.is_empty()
        && matching.iter().all(|row| {
            matches!(
                row.get("disposition").and_then(Value::as_str),
                Some("swarm_blob" | "integrated")
            )
        })
}

#[test]
fn changed_workflows_require_unanimous_reviewed_non_source_dispositions() -> Result<(), String> {
    let workflow = workflow_text()?;
    for needle in [
        "changed_workflows=$(git diff --name-only \"$BASE_SHA...$PR_HEAD\" -- .github/workflows)",
        "[.dispositions[]? | select(.key == $path)] | length",
        "[.dispositions[]? | select(.key == $path and (.disposition == \"swarm_blob\" or .disposition == \"integrated\"))] | length",
        "test \"$resolution_count\" -eq 0 || test \"$reviewed_count\" -ne \"$resolution_count\"",
        "promotion PR changes workflows without unanimous reviewed non-source dispositions",
    ] {
        assert!(
            workflow.contains(needle),
            "workflow missing contract fragment: {needle}"
        );
    }
    assert!(
        !workflow.contains("/^\\.github\\/workflows\\/source-promotion-contract\\.yml$/d"),
        "workflow must not substitute a hardcoded workflow exception for reviewed resolution authority"
    );
    Ok(())
}

#[test]
fn workflow_disposition_authority_rejects_missing_mixed_or_unreviewed_rows() {
    let path = ".github/workflows/routed-rust.yml";
    let rejected = [
        json!({"dispositions": []}),
        json!({"dispositions": [{"key": path, "disposition": "source_blob"}]}),
        json!({"dispositions": [
            {"key": path, "disposition": "swarm_blob"},
            {"key": path, "disposition": "source_blob"}
        ]}),
        json!({"dispositions": [
            {"key": path, "disposition": "integrated"},
            {"key": path, "disposition": "source_blob"}
        ]}),
    ];

    for manifest in rejected {
        assert!(
            !workflow_disposition_is_authorized(&manifest, path),
            "missing, mixed, or unreviewed workflow dispositions must fail closed: {manifest}"
        );
    }
}

#[test]
fn workflow_disposition_authority_accepts_one_or_more_allowed_category_rows() {
    let path = ".github/workflows/routed-rust.yml";
    for manifest in [
        json!({"dispositions": [{"key": path, "disposition": "swarm_blob"}]}),
        json!({"dispositions": [{"key": path, "disposition": "integrated"}]}),
        json!({"dispositions": [
            {"kind": "conflict", "key": path, "disposition": "integrated"},
            {"kind": "source_survivor", "key": path, "disposition": "swarm_blob"}
        ]}),
    ] {
        assert!(
            workflow_disposition_is_authorized(&manifest, path),
            "all category rows authorize non-source workflow movement: {manifest}"
        );
    }
}

#[test]
fn workflow_rejection_reason_is_single_line_after_multiple_unreviewed_paths() -> Result<(), String>
{
    let workflow = workflow_text()?;
    assert!(workflow.contains("unreviewed_workflows=\"$unreviewed_workflows,$workflow\""));
    assert!(workflow.contains("unreviewed_workflows=\"$workflow\""));
    assert!(
        !workflow.contains(
            "fail \"promotion PR changes non-contract workflows: $unexpected_workflows\""
        ),
        "multi-line git diff output must not flow directly into a single-line GITHUB_OUTPUT value"
    );
    Ok(())
}

#[test]
fn verifier_receipt_schema_predicate_is_balanced() -> Result<(), String> {
    let workflow = workflow_text()?;
    let line = workflow
        .lines()
        .find(|line| line.contains("jq -e '") && line.contains("verified_through_parent_2"))
        .ok_or_else(|| "missing verifier-receipt jq predicate".to_string())?;
    let prefix = "jq -e '";
    let suffix = "' \"$verification\"";
    let start = line
        .find(prefix)
        .ok_or_else(|| "verifier predicate has no jq prefix".to_string())?
        + prefix.len();
    let end = line
        .rfind(suffix)
        .ok_or_else(|| "verifier predicate has no verification-file suffix".to_string())?;
    let filter = &line[start..end];

    let valid = json!({
        "schema": "ripr.source_promotion_verification.v2",
        "status": "verified",
        "join_head": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "source_main": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "main_head": null,
        "parents": [
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "cccccccccccccccccccccccccccccccccccccccc"
        ],
        "tree": "dddddddddddddddddddddddddddddddddddddddd",
        "preflight_sha256": "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        "resolution_manifest_sha256": "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        "merge_base": "1111111111111111111111111111111111111111",
        "swarm_reachability": {
            "all_reachable_count": 1,
            "first_parent_count": 1,
            "all_reachable_sha256": "sha256:2222222222222222222222222222222222222222222222222222222222222222",
            "first_parent_ordered_sha256": "sha256:3333333333333333333333333333333333333333333333333333333333333333",
            "verified_through_parent_2": true
        },
        "release_metadata_surfaces": [],
        "checks": {},
        "failure_reasons": [],
        "invalidation_rules": [],
        "non_claims": []
    });
    assert!(
        jq_filter_matches(filter, &valid)?,
        "complete verifier-receipt jq predicate rejected a valid receipt"
    );

    let mut prefixed_sidecar = valid.clone();
    prefixed_sidecar["preflight_sha256"] =
        json!("sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee");
    if jq_filter_matches(filter, &prefixed_sidecar)? {
        return Err("verifier-receipt predicate accepted a prefixed sidecar digest".into());
    }

    let mut bare_ancestry = valid.clone();
    bare_ancestry["swarm_reachability"]["all_reachable_sha256"] =
        json!("2222222222222222222222222222222222222222222222222222222222222222");
    if jq_filter_matches(filter, &bare_ancestry)? {
        return Err("verifier-receipt predicate accepted a bare ancestry digest".into());
    }

    let mut malformed = valid;
    malformed["swarm_reachability"]["verified_through_parent_2"] = json!("true");
    assert!(
        !jq_filter_matches(filter, &malformed)?,
        "verifier-receipt jq predicate accepted a non-boolean parent-2 proof"
    );
    Ok(())
}

#[test]
fn normalized_contract_is_runner_owned_uploaded_then_enforced() -> Result<(), String> {
    let workflow = workflow_text()?;
    let (promotion_job, post_merge_job) = workflow
        .split_once("\n  post-merge-reachability:\n")
        .ok_or_else(|| "missing post-merge-reachability job".to_string())?;
    let runner_out = "SOURCE_PROMOTION_OUT: ${{ runner.temp }}/ripr-source-promotion";
    let runner_path = "path: ${{ runner.temp }}/ripr-source-promotion";

    for forbidden in [
        "SOURCE_PROMOTION_OUT: target/ripr/source-promotion",
        "path: target/ripr/source-promotion",
    ] {
        assert!(
            !workflow.contains(forbidden),
            "candidate checkout still owns promotion evidence: {forbidden}"
        );
    }
    for (lane_name, lane) in [
        ("promotion-contract", promotion_job),
        ("post-merge-reachability", post_merge_job),
    ] {
        assert_eq!(
            lane.matches(runner_out).count(),
            2,
            "{lane_name} must bind both verifier and receipt writer to runner-owned output"
        );
        assert_eq!(
            lane.matches(runner_path).count(),
            1,
            "{lane_name} artifact upload must use the runner-owned receipt path"
        );
    }
    for required in [
        "SOURCE_PARENT: ${{ steps.inputs.outputs.source_parent }}",
        "SOURCE_PROMOTION_CONTRACT: ${{ runner.temp }}/ripr-source-promotion/source-promotion-contract.json",
    ] {
        assert!(
            promotion_job.contains(required),
            "promotion contract missing runner-owned identity or receipt binding: {required}"
        );
    }

    let upload = promotion_job
        .find("- name: Upload SHA-bound promotion receipts")
        .ok_or_else(|| "missing promotion receipt upload step".to_string())?;
    let enforce = promotion_job
        .find("- name: Enforce normalized source-promotion contract")
        .ok_or_else(|| "missing terminal normalized-contract enforcement step".to_string())?;
    assert!(
        upload < enforce,
        "rejected evidence must be uploaded before the hosted job fails"
    );
    assert!(
        promotion_job[upload..enforce].contains("if: always()"),
        "promotion receipt upload must run on rejected paths"
    );
    let post_merge_upload = post_merge_job
        .find("- name: Upload SHA-bound post-merge receipts")
        .ok_or_else(|| "missing post-merge receipt upload step".to_string())?;
    assert!(
        post_merge_job[post_merge_upload..].contains("if: always()"),
        "post-merge receipt upload must run on rejected paths"
    );

    let enforcement = &promotion_job[enforce..];
    for required in [
        "if: always()",
        ".schema == \"ripr.source_promotion_contract.v2\"",
        ".status == \"verified\"",
        ".validation.status == \"passed\"",
        ".verifier_receipt_status == \"present\"",
        ".verifier_exit_code == \"0\"",
    ] {
        assert!(
            enforcement.contains(required),
            "terminal enforcement missing: {required}"
        );
    }
    Ok(())
}

#[test]
fn admission_workflow_has_closed_exact_transport_and_terminal_order() -> Result<(), String> {
    validate_admission_workflow_contract(&admission_workflow_text()?)
}

#[test]
fn admission_workflow_contract_rejects_security_and_order_mutations() -> Result<(), String> {
    let original = admission_workflow_text()?;
    let root_flag = "--workspace-root \"$ADMISSION_ROOT\"";
    let enforce_marker = "source-promotion enforce-admission-workflow";
    for occurrence in [0, 1] {
        let start = original
            .match_indices(enforce_marker)
            .nth(occurrence)
            .ok_or("missing enforcement mutation target")?
            .0;
        let offset = original[start..]
            .find(root_flag)
            .ok_or("missing enforcement root mutation target")?
            + start;
        for replacement in ["", "--workspace-root \"$ADMISSION_WORKSPACE\""] {
            let mut mutated = original.clone();
            mutated.replace_range(offset..offset + root_flag.len(), replacement);
            if validate_admission_workflow_contract(&mutated).is_ok() {
                return Err(format!(
                    "accepted enforcement root mutation at call {occurrence}"
                ));
            }
        }
    }
    let mut finalizer_only = original.clone();
    for occurrence in [1, 0] {
        let start = finalizer_only
            .match_indices(enforce_marker)
            .nth(occurrence)
            .ok_or("missing finalizer-only mutation target")?
            .0;
        let offset = finalizer_only[start..]
            .find(root_flag)
            .ok_or("missing finalizer-only root target")?
            + start;
        finalizer_only.replace_range(offset..offset + root_flag.len(), "");
    }
    if validate_admission_workflow_contract(&finalizer_only).is_ok() {
        return Err("accepted root flags absent from both enforcement calls".to_string());
    }
    let workflow = admission_workflow_text()?;
    let mutations = [
        (
            "name: Source Promotion Admission",
            "name: Candidate Selected Admission",
        ),
        ("contents: read", "contents: write"),
        ("${{ runner.temp }}", "target"),
        ("persist-credentials: false", "persist-credentials: true"),
        (
            "source-promotion verify-admission-workflow",
            "source-promotion publish-candidate-ref",
        ),
        (
            "- name: Upload complete pre-enforcement evidence\n        if: always()",
            "- name: Upload complete pre-enforcement evidence\n        if: success()",
        ),
        (
            "if: always() && steps.enforce.outcome == 'success'",
            "if: always()",
        ),
        (
            "identity_digest=$(sha256sum \"$admission_out/requested-identity.json\" | awk '{print $1}')",
            "identity_digest=$WORKFLOW_SOURCE_SHA",
        ),
        (
            "name: ${{ steps.initialize.outputs.artifact_name }}",
            "name: ${{ inputs.workflow_source_sha }}",
        ),
        (
            "github.repository == 'EffortlessMetrics/ripr' &&",
            "github.repository == inputs.source_repository &&",
        ),
        (
            "ref: ${{ job.workflow_sha }}",
            "ref: ${{ inputs.workflow_source_sha }}",
        ),
        ("--expected-status admitted", "--expected-status rejected"),
    ];
    for (needle, replacement) in mutations {
        let mutated = workflow.replace(needle, replacement);
        if mutated == workflow {
            return Err(format!("mutation fixture did not match workflow: {needle}"));
        }
        if validate_admission_workflow_contract(&mutated).is_ok() {
            return Err(format!(
                "admission workflow contract accepted mutation {needle:?} -> {replacement:?}"
            ));
        }
    }

    let upload = "- name: Upload complete pre-enforcement evidence";
    let enforce = "- name: Enforce terminal admission before constructor";
    require_fragment(&workflow, upload)?;
    require_fragment(&workflow, enforce)?;
    let reordered = workflow
        .replacen(upload, "- name: TEMPORARY ADMISSION STEP", 1)
        .replacen(enforce, upload, 1)
        .replacen("- name: TEMPORARY ADMISSION STEP", enforce, 1);
    if validate_admission_workflow_contract(&reordered).is_ok() {
        return Err("admission workflow contract accepted enforcement-before-upload".to_string());
    }
    Ok(())
}

#[test]
fn admission_workflow_does_not_accept_caller_selected_authority() -> Result<(), String> {
    let workflow = admission_workflow_text()?;
    for forbidden_input in [
        "runner:",
        "command:",
        "permissions:",
        "success:",
        "expected_status:",
        "target_ref:",
    ] {
        if workflow.contains(&format!("      {forbidden_input}")) {
            return Err(format!(
                "candidate-controlled authority input is forbidden: {forbidden_input}"
            ));
        }
    }
    Ok(())
}

#[test]
fn production_j5_rejection_is_a_self_verifying_workflow_packet() -> Result<(), String> {
    production_workflow_fixture("j5_negative")
}

#[test]
fn green_historical_diagnostics_are_rejected_by_the_real_final_gate() -> Result<(), String> {
    production_workflow_fixture("positive_synthetic")
}

// Diagnostics only: this cannot change the initial positive disposition,
// the 180-second governed command, or the later historical-v1 refusal oracle.
fn initial_file_policy_failure_output(root: &Path) -> String {
    use std::io::{Read, Seek, SeekFrom};
    const LIMIT: usize = 8 * 1024;
    const PARENT: [&str; 7] = [
        "workspace",
        "synthetic-fixture",
        "fixture-repository",
        ".git",
        "source-promotion-admission-fixture",
        "validation-packet",
        "commands",
    ];
    let mut report = String::new();
    for stream in ["stdout", "stderr"] {
        let name = format!("04-check-file-policy.{stream}.log");
        let read = || -> Result<(u64, Vec<u8>), String> {
            let mut path = root.to_path_buf();
            for (index, component) in PARENT.iter().copied().chain([name.as_str()]).enumerate() {
                path.push(component);
                let metadata = fs::symlink_metadata(&path)
                    .map_err(|error| format!("metadata: {:?}", error.kind()))?;
                if metadata.file_type().is_symlink()
                    || (index < PARENT.len() && !metadata.is_dir())
                    || (index == PARENT.len() && !metadata.is_file())
                {
                    return Err("unexpected owned diagnostic path type".to_string());
                }
            }
            let mut file =
                fs::File::open(path).map_err(|error| format!("open: {:?}", error.kind()))?;
            let total = file
                .metadata()
                .map_err(|error| format!("file metadata: {:?}", error.kind()))?
                .len();
            file.seek(SeekFrom::Start(total.saturating_sub(LIMIT as u64)))
                .map_err(|error| format!("seek: {:?}", error.kind()))?;
            let mut bytes = Vec::with_capacity(LIMIT);
            file.take(LIMIT as u64)
                .read_to_end(&mut bytes)
                .map_err(|error| format!("read: {:?}", error.kind()))?;
            Ok((total, bytes))
        };
        report.push_str(&format!("BEGIN INITIAL {name}\n"));
        match read() {
            Ok((total, bytes)) => {
                let mut text = String::from_utf8_lossy(&bytes).into_owned();
                let mut end = text.len().min(LIMIT);
                while !text.is_char_boundary(end) {
                    end -= 1;
                }
                text.truncate(end);
                report.push_str(&format!(
                    "state=observed file_bytes={total} tail_bytes={} limit={LIMIT} \
                     truncated={} tail_sha256={:x}\n{text}\n",
                    bytes.len(),
                    total > bytes.len() as u64,
                    Sha256::digest(&bytes)
                ));
            }
            Err(error) => report.push_str(&format!("state=unavailable reason={error}\n")),
        }
        report.push_str(&format!("END INITIAL {name}\n"));
    }
    report
}

#[test]
fn initial_file_policy_diagnostics_are_bounded_and_exactly_scoped() -> Result<(), String> {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("diagnostic fixture clock: {error}"))?
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "ripr-initial-file-policy-diagnostic-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&root).map_err(|error| format!("claim diagnostic fixture: {error}"))?;
    let result = (|| {
        let logs = root.join(
            "workspace/synthetic-fixture/fixture-repository/.git/\
             source-promotion-admission-fixture/validation-packet/commands",
        );
        fs::create_dir_all(&logs).map_err(|error| format!("create owned logs: {error}"))?;
        let bytes = [vec![b'x'; 16 * 1024], b"owned stdout tail".to_vec()].concat();
        fs::write(logs.join("04-check-file-policy.stdout.log"), &bytes)
            .map_err(|error| format!("write stdout: {error}"))?;
        fs::write(
            logs.join("03-check-workflows.stderr.log"),
            "UNRELATED_LOG_MUST_NOT_APPEAR",
        )
        .map_err(|error| format!("write unrelated log: {error}"))?;
        let first = initial_file_policy_failure_output(&root);
        if !first.contains("owned stdout tail")
            || !first.contains("tail_bytes=8192 limit=8192 truncated=true")
            || !first.contains("state=unavailable")
            || first.contains("UNRELATED_LOG_MUST_NOT_APPEAR")
            || first.len() > 18 * 1024
        {
            return Err("initial exact-log bounded/missing control failed".to_string());
        }
        fs::write(
            logs.join("04-check-file-policy.stderr.log"),
            "owned stderr build witness",
        )
        .map_err(|error| format!("write stderr: {error}"))?;
        let both = initial_file_policy_failure_output(&root);
        if !both.contains("owned stdout tail")
            || !both.contains("owned stderr build witness")
            || both.contains("state=unavailable")
            || both.contains("UNRELATED_LOG_MUST_NOT_APPEAR")
            || both.len() > 18 * 1024
        {
            return Err("initial exact two-stream control failed".to_string());
        }
        fs::remove_file(logs.join("04-check-file-policy.stdout.log"))
            .map_err(|error| format!("remove owned stdout: {error}"))?;
        fs::create_dir(logs.join("04-check-file-policy.stdout.log"))
            .map_err(|error| format!("create wrong-type control: {error}"))?;
        let wrong = initial_file_policy_failure_output(&root);
        if !wrong.contains("unexpected owned diagnostic path type")
            || !wrong.contains("owned stderr build witness")
        {
            return Err("wrong-type diagnostic control failed".to_string());
        }
        Ok(())
    })();
    let cleanup =
        fs::remove_dir_all(root).map_err(|error| format!("cleanup diagnostic fixture: {error}"));
    result.and(cleanup)
}

// This is failure-only context, never an admission validator or a passed-state predicate.
const INITIAL_REQUIRED_COMMANDS: [&str; 13] = [
    "check-network-policy",
    "check-process-policy",
    "check-workflows",
    "check-file-policy",
    "check-dependencies",
    "check-generated-clean",
    "check-executable-files",
    "check-command-catalog",
    "check-spec-format",
    "check-traceability",
    "check-doc-artifacts",
    "check-public-api",
    "check-architecture",
];
const INITIAL_FIXTURE: &str =
    "workspace/synthetic-fixture/fixture-repository/.git/source-promotion-admission-fixture";

include!("source_promotion_workflow_contract/preparation_evidence.rs");

fn initial_diagnostic_path(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let root_metadata = fs::symlink_metadata(root).map_err(|_error| "owned root unavailable")?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err("unexpected owned diagnostic root type".into());
    }
    let mut path = root.to_path_buf();
    let components = Path::new(relative).components().collect::<Vec<_>>();
    for (index, component) in components.iter().enumerate() {
        let std::path::Component::Normal(part) = component else {
            return Err("diagnostic path is not an owned relative path".into());
        };
        path.push(part);
        let metadata =
            fs::symlink_metadata(&path).map_err(|_error| "owned diagnostic unavailable")?;
        if metadata.file_type().is_symlink()
            || (index + 1 < components.len() && !metadata.is_dir())
            || (index + 1 == components.len() && !metadata.is_file())
        {
            return Err("unexpected owned diagnostic path type".into());
        }
    }
    let canonical = path
        .canonicalize()
        .map_err(|_error| "diagnostic canonical path unavailable")?;
    let owner = root
        .canonicalize()
        .map_err(|_error| "owned root canonical path unavailable")?;
    if !canonical.starts_with(owner) {
        return Err("diagnostic escaped its owned root".into());
    }
    Ok(path)
}

fn initial_diagnostic_bytes(root: &Path, relative: &str, cap: usize) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let path = initial_diagnostic_path(root, relative)?;
    let file = fs::File::open(path).map_err(|_error| "owned diagnostic open failed")?;
    let mut bytes = Vec::with_capacity(cap + 1);
    file.take((cap + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_error| "owned diagnostic read failed")?;
    if bytes.len() > cap {
        return Err("owned diagnostic input byte ceiling exceeded".into());
    }
    Ok(bytes)
}

fn initial_diagnostic_text(bytes: &[u8], cap: usize) -> String {
    let mut text = String::from_utf8_lossy(bytes).into_owned();
    let mut end = text.len().min(cap);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    text
}

fn initial_diagnostic_member<'a>(index: &'a Value, name: &str) -> Result<&'a Value, String> {
    let files = index["files"]
        .as_array()
        .ok_or("packet index files unavailable")?;
    // Two reports, 26 governed streams, one preparation manifest and ten
    // preparation streams (five Unix classes; Windows has four).
    if files.len() > 39 {
        return Err("packet index exceeds fixed command packet".into());
    }
    let mut matches = files
        .iter()
        .filter(|entry| entry["path"].as_str() == Some(name));
    let entry = matches
        .next()
        .ok_or("indexed diagnostic member unavailable")?;
    if matches.next().is_some() {
        return Err("duplicate indexed diagnostic member".into());
    }
    Ok(entry)
}

fn initial_diagnostic_stream(
    root: &Path,
    relative: &str,
    receipt: &Value,
    stream: &str,
    member: &Value,
) -> Result<String, String> {
    use std::io::{Read, Seek, SeekFrom};
    const LIMIT: usize = 8 * 1024;
    const CAP: u64 = 2 * 1024 * 1024;
    let bytes_key = format!("{stream}_bytes");
    let hash_key = format!("{stream}_sha256");
    let expected = receipt[&bytes_key]
        .as_u64()
        .ok_or("stream byte identity unavailable")?;
    let digest = receipt[&hash_key]
        .as_str()
        .ok_or("stream digest unavailable")?;
    if expected > CAP
        || member["bytes"].as_u64() != Some(expected)
        || member["sha256"].as_str() != Some(digest)
    {
        return Err("stream receipt/index identity mismatch".into());
    }
    let path = initial_diagnostic_path(root, relative)?;
    let mut file = fs::File::open(&path).map_err(|_error| "stream open failed")?;
    let before = file
        .metadata()
        .map_err(|_error| "stream metadata unavailable")?;
    if before.len() != expected {
        return Err("stream size changed".into());
    }
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut read_bytes = 0u64;
    while read_bytes < expected {
        let remaining = usize::try_from((expected - read_bytes).min(buffer.len() as u64))
            .map_err(|_error| "stream remaining length overflow")?;
        let count = file
            .read(&mut buffer[..remaining])
            .map_err(|_error| "stream hash read failed")?;
        if count == 0 {
            return Err("stream ended before its receipt extent".into());
        }
        hash.update(&buffer[..count]);
        read_bytes += count as u64;
    }
    let mut growth = [0u8; 1];
    if file
        .read(&mut growth)
        .map_err(|_error| "stream growth read failed")?
        != 0
        || format!("{:x}", hash.finalize()) != digest
    {
        return Err("stream digest/extent changed".into());
    }
    file.seek(SeekFrom::Start(expected.saturating_sub(LIMIT as u64)))
        .map_err(|_error| "stream tail seek failed")?;
    let mut tail = Vec::with_capacity(LIMIT);
    (&mut file)
        .take(LIMIT as u64)
        .read_to_end(&mut tail)
        .map_err(|_error| "stream tail read failed")?;
    let after = file
        .metadata()
        .map_err(|_error| "stream final metadata unavailable")?;
    if after.len() != before.len()
        || after.modified().ok() != before.modified().ok()
        || initial_diagnostic_path(root, relative)? != path
    {
        return Err("stream changed during diagnostic read".into());
    }
    Ok(format!(
        "path={relative} file_bytes={expected} sha256={digest} tail_bytes={} limit={LIMIT} \
         truncated={} capture_truncated={} tail_sha256={:x}\n{}\n",
        tail.len(),
        expected > tail.len() as u64,
        receipt[format!("{stream}_truncated")]
            .as_bool()
            .ok_or("stream capture flag unavailable")?,
        Sha256::digest(&tail),
        initial_diagnostic_text(&tail, LIMIT),
    ))
}

fn initial_required_command_failure_output(root: &Path, catalog_only: bool) -> String {
    const TOTAL_LIMIT: usize = 48 * 1024;
    let read = || -> Result<String, String> {
        let packet = format!("{INITIAL_FIXTURE}/validation-packet");
        let report_bytes = initial_diagnostic_bytes(
            root,
            &format!("{packet}/resolved-tree-validation.json"),
            128 * 1024,
        )?;
        let report: Value = serde_json::from_slice(&report_bytes)
            .map_err(|_error| "validation report malformed")?;
        let index_bytes =
            initial_diagnostic_bytes(root, &format!("{packet}/packet-index.json"), 128 * 1024)?;
        let index: Value =
            serde_json::from_slice(&index_bytes).map_err(|_error| "packet index malformed")?;
        if report["schema"] != "ripr.source_promotion_resolved_tree_validation.v1"
            || report["tool_version"] != env!("CARGO_PKG_VERSION")
            || !matches!(report["status"].as_str(), Some("rejected" | "validated"))
            || index["schema"] != "ripr.source_promotion_resolved_tree_packet.v1"
            || index["status"] != report["status"]
            || index["complete"] != true
        {
            return Err("failure diagnostic packet schema/status mismatch".into());
        }
        let report_member = initial_diagnostic_member(&index, "resolved-tree-validation.json")?;
        if report_member["bytes"].as_u64() != Some(report_bytes.len() as u64)
            || report_member["sha256"].as_str()
                != Some(format!("{:x}", Sha256::digest(&report_bytes)).as_str())
        {
            return Err("validation report/index identity mismatch".into());
        }
        let preflight_bytes = initial_diagnostic_bytes(
            root,
            &format!("{INITIAL_FIXTURE}/preflight.json"),
            64 * 1024,
        )?;
        let resolution_bytes = initial_diagnostic_bytes(
            root,
            &format!("{INITIAL_FIXTURE}/resolution.json"),
            64 * 1024,
        )?;
        let preflight: Value = serde_json::from_slice(&preflight_bytes)
            .map_err(|_error| "owned preflight malformed")?;
        let resolution: Value = serde_json::from_slice(&resolution_bytes)
            .map_err(|_error| "owned resolution malformed")?;
        let head = initial_diagnostic_bytes(
            root,
            "workspace/synthetic-fixture/fixture-repository/.git/HEAD",
            64,
        )?;
        let head = std::str::from_utf8(&head)
            .map_err(|_error| "owned HEAD malformed")?
            .trim();
        let source = report["source_parent"]
            .as_str()
            .ok_or("source identity unavailable")?;
        let swarm = report["swarm_parent"]
            .as_str()
            .ok_or("swarm identity unavailable")?;
        let tree = report["reviewed_tree"]
            .as_str()
            .ok_or("tree identity unavailable")?;
        let hex = |value: &str, count: usize| {
            value.len() == count
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        };
        if !hex(source, 40)
            || !hex(swarm, 40)
            || !hex(tree, 40)
            || source != head
            || preflight["schema"] != "ripr.source_promotion_preflight.v1"
            || resolution["schema"] != "ripr.source_promotion_resolution.v1"
            || preflight["source_parent"] != source
            || preflight["swarm_parent"] != swarm
            || preflight
                .pointer("/dry_merge/reviewed_resolved_tree")
                .and_then(Value::as_str)
                != Some(tree)
            || resolution["source_parent"] != source
            || resolution["swarm_parent"] != swarm
            || resolution["reviewed_join_tree"] != tree
            || report
                .pointer("/materialization/reviewed_tree")
                .and_then(Value::as_str)
                != Some(tree)
            || report
                .pointer("/materialization/created")
                .and_then(Value::as_bool)
                != Some(true)
            || report
                .pointer("/trusted_checker/source_sha")
                .and_then(Value::as_str)
                != Some(source)
            || !report
                .pointer("/trusted_checker/executable_sha256")
                .and_then(Value::as_str)
                .is_some_and(|value| hex(value, 64))
            || report
                .pointer("/preflight/verified")
                .and_then(Value::as_bool)
                != Some(true)
            || report
                .pointer("/resolution_manifest/verified")
                .and_then(Value::as_bool)
                != Some(true)
        {
            return Err("owned synthetic fixture/report identity mismatch".into());
        }
        let preflight_hash = format!("{:x}", Sha256::digest(&preflight_bytes));
        let resolution_hash = format!("{:x}", Sha256::digest(&resolution_bytes));
        if report.pointer("/preflight/sha256").and_then(Value::as_str)
            != Some(preflight_hash.as_str())
            || resolution["preflight_sha256"] != preflight_hash
            || report
                .pointer("/resolution_manifest/sha256")
                .and_then(Value::as_str)
                != Some(resolution_hash.as_str())
        {
            return Err("owned input/report digest mismatch".into());
        }
        let catalog = report["required_command_catalog"]
            .as_array()
            .ok_or("required catalog unavailable")?;
        let commands = report["commands"]
            .as_array()
            .ok_or("required command receipts unavailable")?;
        if catalog.len() != INITIAL_REQUIRED_COMMANDS.len()
            || commands.len() != catalog.len()
            || !catalog
                .iter()
                .zip(INITIAL_REQUIRED_COMMANDS)
                .all(|(value, name)| value.as_str() == Some(name))
        {
            return Err("required command catalog mismatch".into());
        }
        let preparation = initial_preparation_failure_context(root, &packet, &report, &index)?;
        let mut preparation_stopped = false;
        let mut failed = None;
        for (position, (receipt, name)) in
            commands.iter().zip(INITIAL_REQUIRED_COMMANDS).enumerate()
        {
            let role = if name == "check-command-catalog" {
                "source_parent_trusted_checker_self_health"
            } else {
                "reviewed_tree_source_governance_contract"
            };
            if receipt["command"] != name
                || receipt["subject_role"] != role
                || receipt["timeout_bound_ms"] != 180_000
            {
                return Err("required command identity/role/bound mismatch".into());
            }
            match receipt["state"].as_str() {
                Some("passed")
                    if failed.is_none()
                        && !preparation_stopped
                        && receipt["exit_code"] == 0
                        && receipt["evidence_present"] == true => {}
                Some("failed")
                    if failed.is_none()
                        && !preparation_stopped
                        && receipt["evidence_present"] == true
                        && (receipt["exit_code"].is_null()
                            || receipt["exit_code"].as_i64().is_some_and(|code| code != 0)) =>
                {
                    failed = Some((position, receipt, name));
                }
                Some("not_run")
                    if !preparation_stopped
                        && failed.is_none()
                        && name == "check-file-policy"
                        && preparation.as_ref().is_some_and(|(reason, _)| {
                            receipt["failure_reason"].as_str() == Some(reason.as_str())
                        })
                        && receipt["exit_code"].is_null()
                        && receipt["evidence_present"] == false =>
                {
                    preparation_stopped = true;
                }
                Some("not_run")
                    if (failed.is_some() || preparation_stopped)
                        && receipt["exit_code"].is_null()
                        && receipt["evidence_present"] == false => {}
                _ => return Err("required command failure sequence mismatch".into()),
            }
        }
        if report["status"] == "validated" && (failed.is_some() || preparation_stopped) {
            return Err("failure sequence/report disposition mismatch".into());
        }
        if preparation_stopped {
            let (_, context) = preparation.ok_or("preparation failure context unavailable")?;
            return Ok(if catalog_only {
                format!("state=catalog_not_reached preparation_failure=true\n{context}")
            } else {
                context
            });
        }
        if !catalog_only && failed.is_none() {
            return Ok(format!(
                "state=no_failed_required_command validation_status={} source={source} swarm={swarm} tree={tree} report_sha256={:x}\n",
                report["status"]
                    .as_str()
                    .ok_or("validation status unavailable")?,
                Sha256::digest(&report_bytes)
            ));
        }
        let (position, receipt, name) = if catalog_only {
            let position = INITIAL_REQUIRED_COMMANDS
                .iter()
                .position(|name| *name == "check-command-catalog")
                .ok_or("fixed catalog owner unavailable")?;
            let receipt = &commands[position];
            if receipt["evidence_present"] != true {
                return Err("catalog command was not reached".into());
            }
            (position, receipt, INITIAL_REQUIRED_COMMANDS[position])
        } else {
            failed.ok_or("no evidenced failed required command")?
        };
        let reason = if receipt["state"] == "passed" {
            if !receipt["failure_reason"].is_null() {
                return Err("passed catalog has failure reason".into());
            }
            "none (actual command passed)"
        } else {
            receipt["failure_reason"]
                .as_str()
                .filter(|value| !value.is_empty())
                .ok_or("failed command reason unavailable")?
        };
        let mut context = format!(
            "state=observed command={name} role={} source={source} swarm={swarm} tree={tree} \
             checker_sha256={} report_sha256={:x} index_sha256={:x}\n\
             failure_reason excerpt_limit=16384 truncated={}\n{}\n",
            receipt["subject_role"]
                .as_str()
                .ok_or("command role unavailable")?,
            report["trusted_checker"]["executable_sha256"]
                .as_str()
                .ok_or("checker digest unavailable")?,
            Sha256::digest(&report_bytes),
            Sha256::digest(&index_bytes),
            reason.len() > 16 * 1024,
            initial_diagnostic_text(reason.as_bytes(), 16 * 1024),
        );
        for stream in ["stdout", "stderr"] {
            let relative = format!("commands/{:02}-{name}.{stream}.log", position + 1);
            if receipt[format!("{stream}_path")].as_str() != Some(relative.as_str()) {
                return Err("failed command log mapping mismatch".into());
            }
            let member = initial_diagnostic_member(&index, &relative)?;
            context.push_str(&initial_diagnostic_stream(
                root,
                &format!("{packet}/{relative}"),
                receipt,
                stream,
                member,
            )?);
        }
        if catalog_only {
            let relative =
                format!("{INITIAL_FIXTURE}/validation-packet.command-catalog-context.json");
            let bytes = initial_diagnostic_bytes(root, &relative, 64 * 1024)?;
            let sidecar: Value =
                serde_json::from_slice(&bytes).map_err(|_error| "catalog sidecar malformed")?;
            let snapshot = sidecar["snapshot"]
                .as_str()
                .ok_or("catalog snapshot unavailable")?;
            if sidecar["diagnostic_kind"] != "bounded_command_catalog_context_v1"
                || sidecar["diagnostic_only"] != true
                || sidecar["tool_version"] != env!("CARGO_PKG_VERSION")
                || sidecar["source_parent"] != source
                || sidecar["swarm_parent"] != swarm
                || sidecar["reviewed_tree"] != tree
                || sidecar["checker_sha256"] != report["trusted_checker"]["executable_sha256"]
                || sidecar.get("command_receipt") != Some(receipt)
                || snapshot.len() > 10 * 1024
                || sidecar["snapshot_bytes"].as_u64() != Some(snapshot.len() as u64)
                || sidecar["snapshot_sha256"].as_str()
                    != Some(format!("{:x}", Sha256::digest(snapshot.as_bytes())).as_str())
            {
                return Err("catalog sidecar/report identity mismatch".into());
            }
            context.push_str(&format!(
                "catalog_sidecar path={relative} bytes={} sha256={:x}\n{}\n",
                bytes.len(),
                Sha256::digest(&bytes),
                snapshot
            ));
        }
        Ok(context)
    };
    let context = match read() {
        Ok(context) => context,
        Err(reason) => format!("state=unavailable reason={reason}\n"),
    };
    format!(
        "BEGIN INITIAL REQUIRED COMMAND limit={TOTAL_LIMIT} truncated={}\n{}\
        END INITIAL REQUIRED COMMAND\n",
        context.len() > TOTAL_LIMIT,
        initial_diagnostic_text(context.as_bytes(), TOTAL_LIMIT)
    )
}

fn initial_workflow_terminal_context(root: &Path) -> String {
    const CAP: usize = 16 * 1024;
    let mut context = String::from("diagnostic_only=true acceptance_credit=false\n");
    for relative in [
        "workspace/workflow-packet/workflow-disposition.json",
        "workspace/resolved-tree-admission/resolved-tree-admission.json",
    ] {
        match initial_diagnostic_bytes(root, relative, CAP) {
            Ok(bytes) => {
                let rendered = format!("{:?}", String::from_utf8_lossy(&bytes));
                context.push_str(&format!(
                    "path={relative} bytes={} sha256={:x} rendered_limit={CAP} truncated={} text={}\n",
                    bytes.len(),
                    Sha256::digest(&bytes),
                    rendered.len() > CAP,
                    initial_diagnostic_text(rendered.as_bytes(), CAP),
                ));
            }
            Err(reason) => context.push_str(&format!("path={relative} unavailable={reason}\n")),
        }
    }
    context
}

fn retain_initial_required_command_context(
    repo: &Path,
    context: &str,
    nonce: u128,
) -> Result<String, String> {
    const CAP: usize = 64 * 1024;
    let directory = owned_initial_report_directory(repo)?;
    let name = format!(
        "source-promotion-initial-required-command-context-{}-{nonce}.txt",
        std::process::id()
    );
    let text = format!(
        "bounded_initial_command_context bytes_limit={CAP} truncated={}\n{}\n",
        context.len() > CAP - 256,
        initial_diagnostic_text(context.as_bytes(), CAP - 256)
    );
    if text.len() > CAP {
        return Err("report serialization ceiling exceeded".into());
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join(&name))
        .map_err(|_error| "exclusive report file claim failed")?;
    file.write_all(text.as_bytes())
        .map_err(|_error| "bounded report write failed")?;
    file.flush()
        .map_err(|_error| "bounded report flush failed")?;
    Ok(format!(
        "path=target/ripr/reports/{name} bytes={} sha256={:x}",
        text.len(),
        Sha256::digest(text.as_bytes())
    ))
}

#[test]
fn initial_required_command_diagnostics_follow_owned_receipts() -> Result<(), String> {
    let owner_source =
        include_str!("../src/reports/source_promotion_validate_resolved_tree/core.rs");
    let catalog_source = owner_source
        .split("pub(crate) const REQUIRED_COMMANDS: &[&str] = &[")
        .nth(1)
        .and_then(|text| text.split("];").next())
        .ok_or("required command source owner missing")?;
    let owner_names = catalog_source
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix('"')
                .and_then(|text| text.strip_suffix("\","))
        })
        .collect::<Vec<_>>();
    if owner_names != INITIAL_REQUIRED_COMMANDS {
        return Err(
            "diagnostic allowlist differs from actual source-owned required catalog".into(),
        );
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "ripr-required-diagnostic-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&root).map_err(|error| error.to_string())?;
    let result = (|| {
        let terminal_dir = root.join("workspace/workflow-packet");
        fs::create_dir_all(&terminal_dir).map_err(|error| error.to_string())?;
        let terminal = terminal_dir.join("workflow-disposition.json");
        fs::write(
            &terminal,
            br#"{"status":"rejected","failure_reasons":["actual-terminal-refusal"]}"#,
        )
        .map_err(|error| error.to_string())?;
        let context = initial_workflow_terminal_context(&root);
        if !context.contains("actual-terminal-refusal")
            || !context.contains("diagnostic_only=true acceptance_credit=false")
            || !context.contains("sha256=")
        {
            return Err("terminal workflow refusal was not retained as diagnostic evidence".into());
        }
        fs::write(&terminal, vec![b'x'; 16 * 1024 + 1]).map_err(|error| error.to_string())?;
        let oversized = initial_workflow_terminal_context(&root);
        if !oversized.contains("input byte ceiling exceeded") || oversized.len() > 34 * 1024 {
            return Err("terminal workflow diagnostic did not preserve its input bound".into());
        }
        fs::remove_file(&terminal).map_err(|error| error.to_string())?;
        if !initial_workflow_terminal_context(&root).contains("unavailable=") {
            return Err("missing terminal workflow diagnostic was treated as evidence".into());
        }
        let evidence = root.join(INITIAL_FIXTURE);
        let packet = evidence.join("validation-packet");
        let logs = packet.join("commands");
        fs::create_dir_all(&logs).map_err(|error| error.to_string())?;
        fs::write(
            root.join("workspace/synthetic-fixture/fixture-repository/.git/HEAD"),
            format!("{}\n", "a".repeat(40)),
        )
        .map_err(|error| error.to_string())?;
        let preflight = json!({"schema":"ripr.source_promotion_preflight.v1","source_parent":"a".repeat(40),
            "swarm_parent":"b".repeat(40),"dry_merge":{"reviewed_resolved_tree":"c".repeat(40)}});
        let preflight_bytes = serde_json::to_vec(&preflight).map_err(|error| error.to_string())?;
        let preflight_hash = format!("{:x}", Sha256::digest(&preflight_bytes));
        fs::write(evidence.join("preflight.json"), &preflight_bytes)
            .map_err(|error| error.to_string())?;
        let resolution = json!({"schema":"ripr.source_promotion_resolution.v1","source_parent":"a".repeat(40),
            "swarm_parent":"b".repeat(40),"reviewed_join_tree":"c".repeat(40),"preflight_sha256":preflight_hash});
        let resolution_bytes =
            serde_json::to_vec(&resolution).map_err(|error| error.to_string())?;
        fs::write(evidence.join("resolution.json"), &resolution_bytes)
            .map_err(|error| error.to_string())?;
        let stdout = [vec![b'x'; 16 * 1024], b"OWNED_STDOUT_TAIL".to_vec()].concat();
        let stderr = b"OWNED_STDERR".to_vec();
        fs::write(
            logs.join("03-check-workflows.stderr.log"),
            "UNRELATED_MUST_NOT_APPEAR",
        )
        .map_err(|error| error.to_string())?;
        for position in [3usize, 7] {
            let name = INITIAL_REQUIRED_COMMANDS[position];
            let mut commands = Vec::new();
            for (index, command) in INITIAL_REQUIRED_COMMANDS.into_iter().enumerate() {
                let mut row = json!({"command":command,"subject_role":if command=="check-command-catalog" {
                    "source_parent_trusted_checker_self_health"} else {"reviewed_tree_source_governance_contract"},
                    "timeout_bound_ms":180000,"state":if index<position {"passed"} else if index==position {"failed"} else {"not_run"},
                    "exit_code":if index<position {Some(0)} else if index==position {Some(1)} else {None},
                    "evidence_present":index<=position});
                if index == position {
                    row["failure_reason"] = json!("ACTUAL_FAILED_OWNER_REPORT");
                    for (stream, bytes) in [("stdout", &stdout), ("stderr", &stderr)] {
                        row[format!("{stream}_path")] =
                            json!(format!("commands/{:02}-{name}.{stream}.log", position + 1));
                        row[format!("{stream}_bytes")] = json!(bytes.len());
                        row[format!("{stream}_sha256")] =
                            json!(format!("{:x}", Sha256::digest(bytes)));
                        row[format!("{stream}_truncated")] = json!(false);
                        fs::write(
                            logs.join(format!("{:02}-{name}.{stream}.log", position + 1)),
                            bytes,
                        )
                        .map_err(|error| error.to_string())?;
                    }
                }
                commands.push(row);
            }
            let mut report = json!({"schema":"ripr.source_promotion_resolved_tree_validation.v1",
                "tool_version":env!("CARGO_PKG_VERSION"),"status":"rejected","source_parent":"a".repeat(40),
                "swarm_parent":"b".repeat(40),"reviewed_tree":"c".repeat(40),
                "preflight":{"verified":true,"sha256":preflight_hash},
                "resolution_manifest":{"verified":true,"sha256":format!("{:x}",Sha256::digest(&resolution_bytes))},
                "trusted_checker":{"source_sha":"a".repeat(40),"executable_sha256":"d".repeat(64)},
                "materialization":{"created":true,"reviewed_tree":"c".repeat(40)},
                "required_command_catalog":INITIAL_REQUIRED_COMMANDS,"commands":commands});
            let publish = |value: &Value| -> Result<(), String> {
                let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
                fs::write(packet.join("resolved-tree-validation.json"), &bytes)
                    .map_err(|error| error.to_string())?;
                let mut files = vec![
                    json!({"path":"resolved-tree-validation.json","bytes":bytes.len(),
                    "sha256":format!("{:x}",Sha256::digest(&bytes))}),
                ];
                for (stream, content) in [("stdout", &stdout), ("stderr", &stderr)] {
                    files.push(
                        json!({"path":format!("commands/{:02}-{name}.{stream}.log",position+1),
                        "bytes":content.len(),"sha256":format!("{:x}",Sha256::digest(content))}),
                    );
                }
                fs::write(
                    packet.join("packet-index.json"),
                    serde_json::to_vec(&json!({
                    "schema":"ripr.source_promotion_resolved_tree_packet.v1","status":"rejected",
                    "complete":true,"files":files}))
                    .map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())
            };
            report["commands"][position]["failure_reason"] = json!("ACTUAL_FAILED_OWNER_REPORT");
            publish(&report)?;
            let observed = initial_required_command_failure_output(&root, false);
            if !observed.contains(&format!("state=observed command={name}"))
                || !observed.contains("OWNED_STDOUT_TAIL")
                || !observed.contains("OWNED_STDERR")
                || !observed.contains("tail_bytes=8192 limit=8192 truncated=true")
                || !observed.contains("ACTUAL_FAILED_OWNER_REPORT")
                || observed.contains("UNRELATED_MUST_NOT_APPEAR")
                || observed.len() > 49 * 1024
            {
                return Err("actual failed-owner bounded context control failed".into());
            }
            if position == 7 {
                let snapshot = b"catalog_snapshot bytes_limit=9216 truncated=false\npath=target/ripr/reports/command-catalog.md\nACTUAL_CATALOG_REPORT";
                let sidecar_path = evidence.join("validation-packet.command-catalog-context.json");
                let mut green = report.clone();
                green["status"] = json!("validated");
                for row in green["commands"]
                    .as_array_mut()
                    .ok_or("green commands absent")?
                {
                    row["state"] = json!("passed");
                    row["exit_code"] = json!(0);
                    row["evidence_present"] = json!(true);
                    row["failure_reason"] = Value::Null;
                }
                let green_bytes = serde_json::to_vec(&green).map_err(|error| error.to_string())?;
                fs::write(packet.join("resolved-tree-validation.json"), &green_bytes)
                    .map_err(|error| error.to_string())?;
                let green_index_bytes = initial_diagnostic_bytes(
                    &root,
                    &format!("{INITIAL_FIXTURE}/validation-packet/packet-index.json"),
                    128 * 1024,
                )?;
                let mut green_index: Value = serde_json::from_slice(&green_index_bytes)
                    .map_err(|error| error.to_string())?;
                green_index["status"] = json!("validated");
                let files = green_index["files"]
                    .as_array_mut()
                    .ok_or("green index absent")?;
                files[0]["bytes"] = json!(green_bytes.len());
                files[0]["sha256"] = json!(format!("{:x}", Sha256::digest(&green_bytes)));
                fs::write(
                    packet.join("packet-index.json"),
                    serde_json::to_vec(&green_index).map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())?;
                let sidecar = json!({"diagnostic_kind":"bounded_command_catalog_context_v1","diagnostic_only":true,
                    "tool_version":env!("CARGO_PKG_VERSION"),"source_parent":green["source_parent"],
                    "swarm_parent":green["swarm_parent"],"reviewed_tree":green["reviewed_tree"],
                    "checker_sha256":green["trusted_checker"]["executable_sha256"],"command_receipt":green["commands"][position],
                    "snapshot_bytes":snapshot.len(),"snapshot_sha256":format!("{:x}",Sha256::digest(snapshot)),"snapshot":std::str::from_utf8(snapshot).map_err(|error| error.to_string())?});
                fs::write(
                    &sidecar_path,
                    serde_json::to_vec(&sidecar).map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())?;
                let catalog = initial_required_command_failure_output(&root, true);
                if !catalog.contains("state=observed command=check-command-catalog")
                    || !catalog.contains("none (actual command passed)")
                    || !catalog.contains("OWNED_STDOUT_TAIL")
                    || !catalog.contains("OWNED_STDERR")
                    || !catalog.contains("ACTUAL_CATALOG_REPORT")
                    || catalog.len() > 49 * 1024
                {
                    return Err(
                        "passed catalog streams/indexed-report retention control failed".into(),
                    );
                }
                let mut late_rejected = green.clone();
                late_rejected["status"] = json!("rejected");
                let late_bytes =
                    serde_json::to_vec(&late_rejected).map_err(|error| error.to_string())?;
                fs::write(packet.join("resolved-tree-validation.json"), &late_bytes)
                    .map_err(|error| error.to_string())?;
                let mut late_index = green_index.clone();
                late_index["status"] = json!("rejected");
                late_index["files"][0]["bytes"] = json!(late_bytes.len());
                late_index["files"][0]["sha256"] =
                    json!(format!("{:x}", Sha256::digest(&late_bytes)));
                fs::write(
                    packet.join("packet-index.json"),
                    serde_json::to_vec(&late_index).map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())?;
                let late_catalog = initial_required_command_failure_output(&root, true);
                let late_commands = initial_required_command_failure_output(&root, false);
                if !late_catalog.contains("none (actual command passed)")
                    || !late_catalog.contains("ACTUAL_CATALOG_REPORT")
                    || late_catalog.contains("state=unavailable")
                    || !late_commands
                        .contains("state=no_failed_required_command validation_status=rejected")
                {
                    return Err(
                        "later-authority rejection lost genuinely passed catalog diagnostics"
                            .into(),
                    );
                }
                fs::write(packet.join("resolved-tree-validation.json"), &green_bytes)
                    .map_err(|error| error.to_string())?;
                fs::write(
                    packet.join("packet-index.json"),
                    serde_json::to_vec(&green_index).map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())?;
                let mut changed = sidecar.clone();
                changed["snapshot"] = json!("CHANGED_SNAPSHOT");
                fs::write(
                    &sidecar_path,
                    serde_json::to_vec(&changed).map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())?;
                if !initial_required_command_failure_output(&root, true)
                    .contains("sidecar/report identity mismatch")
                {
                    return Err("catalog accepted snapshot digest drift".into());
                }
                publish(&report)?;
            }
            for (pointer, replacement) in [
                ("/source_parent", json!("e".repeat(40))),
                ("/reviewed_tree", json!("e".repeat(40))),
                ("/preflight/verified", json!(false)),
                ("/required_command_catalog/0", json!("check-forged")),
                ("/commands/0/timeout_bound_ms", json!(180001)),
                ("/commands/0/subject_role", json!("forged")),
                ("/commands/0/state", json!("not_run")),
            ] {
                let mut changed = report.clone();
                *changed
                    .pointer_mut(pointer)
                    .ok_or("negative pointer missing")? = replacement;
                publish(&changed)?;
                if !initial_required_command_failure_output(&root, false)
                    .contains("state=unavailable")
                {
                    return Err(format!(
                        "diagnostic accepted identity/catalog/sequence drift {pointer}"
                    ));
                }
            }
            for (field, replacement) in [
                ("stdout_path", json!("../unowned.log")),
                ("stdout_sha256", json!("e".repeat(64))),
                ("stdout_bytes", json!(2 * 1024 * 1024 + 1)),
            ] {
                let mut changed = report.clone();
                changed["commands"][position][field] = replacement;
                publish(&changed)?;
                if !initial_required_command_failure_output(&root, false)
                    .contains("state=unavailable")
                {
                    return Err(format!(
                        "diagnostic accepted log mapping/content drift {field}"
                    ));
                }
            }
            publish(&report)?;
            let stderr_path = logs.join(format!("{:02}-{name}.stderr.log", position + 1));
            fs::remove_file(&stderr_path).map_err(|error| error.to_string())?;
            fs::create_dir(&stderr_path).map_err(|error| error.to_string())?;
            if !initial_required_command_failure_output(&root, false)
                .contains("unexpected owned diagnostic path type")
            {
                return Err("diagnostic accepted wrong stream type".into());
            }
            fs::remove_dir(&stderr_path).map_err(|error| error.to_string())?;
            #[cfg(unix)]
            {
                std::os::unix::fs::symlink(
                    logs.join("03-check-workflows.stderr.log"),
                    &stderr_path,
                )
                .map_err(|error| error.to_string())?;
                if !initial_required_command_failure_output(&root, false)
                    .contains("unexpected owned diagnostic path type")
                {
                    return Err("diagnostic followed symlink stream".into());
                }
                fs::remove_file(&stderr_path).map_err(|error| error.to_string())?;
            }
            if !initial_required_command_failure_output(&root, false).contains("state=unavailable")
            {
                return Err("diagnostic accepted missing stream".into());
            }
        }
        let oversized_context = "CONTEXT_MARKER".repeat(10 * 1024);
        let exported = retain_initial_required_command_context(&root, &oversized_context, nonce)?;
        if !exported
            .contains("path=target/ripr/reports/source-promotion-initial-required-command-context-")
            || !exported.contains("sha256=")
        {
            return Err("retained context identity control failed".into());
        }
        let export_path = root.join(format!(
            "target/ripr/reports/source-promotion-initial-required-command-context-{}-{nonce}.txt",
            std::process::id()
        ));
        let exported_bytes = fs::read(&export_path).map_err(|error| error.to_string())?;
        if exported_bytes.len() > 64 * 1024
            || !String::from_utf8_lossy(&exported_bytes).contains("truncated=true")
            || !exported.contains(&format!("sha256={:x}", Sha256::digest(&exported_bytes)))
        {
            return Err("retained context cap/hash control failed".into());
        }
        match retain_initial_required_command_context(&root, "REPLACEMENT_MUST_NOT_APPEAR", nonce) {
            Err(reason) if reason == "exclusive report file claim failed" => {}
            other => return Err(format!("retained context lost exclusive owner: {other:?}")),
        }
        if fs::read(&export_path).map_err(|error| error.to_string())? != exported_bytes {
            return Err("retained context exclusive refusal changed original bytes".into());
        }
        fs::remove_file(&export_path).map_err(|error| error.to_string())?;
        fs::remove_dir_all(root.join("target")).map_err(|error| error.to_string())?;
        fs::write(root.join("target"), "WRONG_DIRECTORY_TYPE")
            .map_err(|error| error.to_string())?;
        match retain_initial_required_command_context(&root, "context", nonce) {
            Err(reason) if reason == "unexpected owned report directory type" => {}
            other => {
                return Err(format!(
                    "retained context accepted wrong directory type: {other:?}"
                ));
            }
        }
        fs::write(
            packet.join("resolved-tree-validation.json"),
            vec![b'x'; 128 * 1024 + 1],
        )
        .map_err(|error| error.to_string())?;
        if !initial_required_command_failure_output(&root, false)
            .contains("input byte ceiling exceeded")
        {
            return Err("diagnostic accepted oversized report".into());
        }
        Ok(())
    })();
    result.and(fs::remove_dir_all(&root).map_err(|error| error.to_string()))
}

#[test]
fn parent_phase_export_retains_framed_boundary_without_scope_or_cap_escape() -> Result<(), String> {
    let prefix = "ripr_covered_by_parent ";
    let argument = "\\\"\n界".repeat(150);
    let mut value = serde_json::json!({
        "event":"phase_summary","checker_pid":9,"first_phase":{"argv":["test",argument]},
        "last_phase":{"argv":["test","last"]},"phases_complete":true
    });
    let initial = format!("{prefix}{value}");
    if initial.len() > 4096 {
        return Err("export fixture exceeds boundary".into());
    }
    value["padding"] = serde_json::json!("");
    let framed = format!("{prefix}{value}");
    value["padding"] = serde_json::json!("x".repeat(4096 - framed.len()));
    let line = format!("{prefix}{value}\n");
    if line.len() != 4097
        || bounded_parent_phase_observation_for_scope(line.as_bytes(), true) != line
        || bounded_parent_phase_observation_for_scope(line.as_bytes(), false) != "NOT_ENABLED"
    {
        return Err("actual framed exporter boundary or scope lost".into());
    }
    value["padding"] = serde_json::json!(format!(
        "{}x",
        value["padding"].as_str().ok_or("padding missing")?
    ));
    let oversized = format!("{prefix}{value}\n");
    let refused = bounded_parent_phase_observation_for_scope(oversized.as_bytes(), true);
    if refused.contains("phase_summary") || !refused.contains("parent_observation_truncated=true") {
        return Err("exporter per-line ceiling weakened".into());
    }
    let doubled = format!("{line}{line}");
    let capped = bounded_parent_phase_observation_for_scope(doubled.as_bytes(), true);
    if capped.len() > 8 * 1024 || !capped.contains("parent_observation_truncated=true") {
        return Err("exporter aggregate ceiling weakened".into());
    }
    Ok(())
}

fn bounded_parent_phase_observation(bytes: &[u8]) -> String {
    let enabled =
        std::env::var("RIPR_SOURCE_PROMOTION_PHASE_DIAGNOSTICS").is_ok_and(|value| value == "1");
    bounded_parent_phase_observation_for_scope(bytes, enabled)
}
fn bounded_parent_phase_observation_for_scope(bytes: &[u8], enabled: bool) -> String {
    const PREFIX: &[u8] = b"ripr_covered_by_parent ";
    const CAP: usize = 8 * 1024;
    if !enabled {
        return "NOT_ENABLED".into();
    }
    let mut retained = Vec::new();
    let mut truncated = false;
    for line in bytes.split(|byte| *byte == b'\n') {
        if !line.starts_with(PREFIX) {
            continue;
        }
        if line.len() > 4096 || retained.len().saturating_add(line.len() + 1) > CAP - 128 {
            truncated = true;
            continue;
        }
        let Ok(value) = serde_json::from_slice::<Value>(&line[PREFIX.len()..]) else {
            truncated = true;
            continue;
        };
        if !matches!(
            value["event"].as_str(),
            Some(
                "post_capture"
                    | "phase_summary"
                    | "process_liveness"
                    | "diagnostic_budget_exhausted"
            )
        ) {
            truncated = true;
            continue;
        }
        retained.extend_from_slice(line);
        retained.push(b'\n');
    }
    if truncated {
        retained.extend_from_slice(b"parent_observation_truncated=true\n")
    }
    if retained.is_empty() {
        return "NOT_REACHED_OR_UNAVAILABLE".into();
    }
    String::from_utf8(retained).unwrap_or_else(|_error| "parent_observation_invalid_utf8".into())
}

fn production_workflow_fixture(profile: &str) -> Result<(), String> {
    let xtask = PathBuf::from(env!("CARGO_BIN_EXE_xtask"));
    // Stage a private copy of the xtask binary beside the original. The suite
    // runs a nested `cargo test -p xtask` concurrently
    // (test_covered_by_production_wrapper_enumerates_through_cargo), and cargo
    // rebuilds and replaces target/debug/xtask mid-run after a nextest build
    // (fingerprint skew between the two drivers). A replaced executable reads
    // back as "... (deleted)", and the workflow's fail-closed executable
    // binding then reports unavailable instead of rejected. The staged copy is
    // byte-identical (same digest) but never rebuilt, and it keeps the
    // target/debug layout so the workflow's target-directory derivation still
    // resolves.
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("test clock precedes epoch: {error}"))?
        .as_nanos();
    let staged = xtask.with_file_name(format!(
        "xtask-j5-production-stage-{}-{nonce}{}",
        std::process::id(),
        std::env::consts::EXE_SUFFIX
    ));
    fs::copy(&xtask, &staged)
        .map_err(|error| format!("failed to stage production J5 xtask copy: {error}"))?;
    // Reuse Cargo's owned Linux build cache while still compiling and enumerating
    // the actual reviewed tree. Windows keeps isolated targets because nested
    // Cargo may replace this running integration executable there.
    #[cfg(unix)]
    let owned_target = xtask
        .parent()
        .and_then(Path::parent)
        .ok_or("production checker has no owned target directory")?;
    let cache_command = |mut command: Command| {
        #[cfg(unix)]
        command.env("CARGO_TARGET_DIR", owned_target);
        #[cfg(windows)]
        command
            .env_remove("CARGO_TARGET_DIR")
            .env_remove("CARGO_BUILD_BUILD_DIR");
        #[cfg(not(any(unix, windows)))]
        let _ = &mut command;
        command
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&staged, fs::Permissions::from_mode(0o755))
            .map_err(|error| format!("failed to mark staged J5 xtask executable: {error}"))?;
    }
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| "xtask manifest directory has no repository parent".to_string())?
        .to_path_buf();
    // Keep the task-owned fixture and both materializations in the existing
    // target allocation, including on restricted Windows source-sync hosts.
    let root = repo_root.join("target").join(format!(
        ".ripr-production-j5-workflow-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(repo_root.join("target"))
        .map_err(|error| format!("failed to create owned production target parent: {error}"))?;
    fs::create_dir(&root)
        .map_err(|error| format!("failed to create production J5 test root: {error}"))?;
    let result = (|| {
        let mut request = j5_request_identity(&repo_root, &root)?;
        request["execution_profile"] = Value::String(profile.to_string());
        if profile == "positive_synthetic" {
            request["reviewed_tree_sha"] = Value::String(git_output(
                &repo_root,
                &["rev-parse", "HEAD^{tree}"],
                &[],
                None,
            )?);
        }
        let request_bytes = serde_json::to_vec_pretty(&request)
            .map_err(|error| format!("failed to serialize production J5 request: {error}"))?;
        let request_sha256 = format!("{:x}", Sha256::digest(&request_bytes));
        let request_path = root.join("requested-identity.json");
        fs::write(&request_path, &request_bytes)
            .map_err(|error| format!("failed to write production J5 request: {error}"))?;
        let workspace = root.join("workspace");
        fs::create_dir(&workspace)
            .map_err(|error| format!("failed to create production J5 workspace: {error}"))?;
        let packet = workspace.join("workflow-packet");
        let required = |key: &str| {
            request
                .get(key)
                .and_then(Value::as_str)
                .ok_or_else(|| format!("production J5 request is missing {key}"))
        };
        let output = cache_command(Command::new(&staged))
            .current_dir(&repo_root)
            .args([
                "source-promotion",
                "run-admission-workflow",
                "--source-repository",
                required("source_repository")?,
                "--source-parent-sha",
                required("source_parent_sha")?,
                "--workflow-source-sha",
                required("workflow_source_sha")?,
                "--trusted-checker-identity",
                required("trusted_checker_identity")?,
                "--swarm-repository",
                required("swarm_repository")?,
                "--protected-w7-ref",
                required("protected_w7_ref")?,
                "--w7-peeled-sha",
                required("w7_peeled_sha")?,
                "--reviewed-tree-sha",
                required("reviewed_tree_sha")?,
                "--reviewed-tree-carrier-sha",
                "not_required",
                "--preflight-locator",
                "",
                "--resolution-manifest-locator",
                "",
                "--validation-packet-locator",
                "",
                "--integration-packet-locator",
                "",
                "--qualification-receipt-locator",
                "",
                "--receipt-schema",
                "ripr.source_promotion_admission_workflow.v1",
                "--operation-mode",
                "constructor_dry_run",
                "--execution-profile",
                profile,
                "--requested-identity",
                path_text(&request_path)?,
                "--requested-identity-sha256",
                &request_sha256,
                "--workspace-root",
                path_text(&workspace)?,
                "--out",
                path_text(&packet)?,
            ])
            .output()
            .map_err(|error| format!("failed to run production J5 workflow: {error}"))?;
        if profile == "positive_synthetic" {
            // Export the indexed preparation evidence before interpreting the
            // governed sequence: setup failure means file policy was not run.
            retain_initial_preparation_evidence(
                &repo_root,
                &root,
                &format!("{INITIAL_FIXTURE}/validation-packet"),
                "initial",
                nonce,
            );
            let initial_context = format!(
                "workflow_terminal_context={}\nfailed_required_context={}\ncatalog_context={}\nparent_phase_observation={}",
                initial_workflow_terminal_context(&root),
                initial_required_command_failure_output(&root, false),
                initial_required_command_failure_output(&root, true),
                bounded_parent_phase_observation(&output.stderr)
            );
            match retain_initial_required_command_context(&repo_root, &initial_context, nonce) {
                Ok(identity) => println!("retained_initial_command_context {identity}"),
                Err(reason) => eprintln!("initial command context retention unavailable: {reason}"),
            }
            if !output.status.success() {
                return Err(format!(
                    "positive workflow failed; bounded_initial_required_command_output={}; stderr={}",
                    initial_diagnostic_text(initial_context.as_bytes(), 64 * 1024),
                    initial_diagnostic_text(&output.stderr, 64 * 1024)
                ));
            }
            println!(
                "bounded_initial_catalog_output={}",
                initial_required_command_failure_output(&root, true)
            );
            let finalized = workspace.join("final-workflow-packet");
            let final_output = cache_command(Command::new(&staged))
                .current_dir(&repo_root)
                .args([
                    "source-promotion",
                    "finalize-admission-workflow",
                    "--admission-packet",
                    path_text(&packet)?,
                    "--workspace-root",
                    path_text(&workspace)?,
                    "--out",
                    path_text(&finalized)?,
                ])
                .output()
                .map_err(|error| format!("positive finalizer launch: {error}"))?;
            if !final_output.status.success() {
                return Err(format!(
                    "positive finalizer failed: {}",
                    String::from_utf8_lossy(&final_output.stderr)
                ));
            }
            let construction: Value = serde_json::from_slice(
                &fs::read(workspace.join("exact-join-construction/exact-join-construction.json"))
                    .map_err(|error| format!("positive construction receipt: {error}"))?,
            )
            .map_err(|error| error.to_string())?;
            if construction["status"] != "constructed"
                || construction
                    .get("native_acceptance_preflight_bytes")
                    .is_some()
            {
                return Err("synthetic construction claimed native authority or failed".to_string());
            }
            let fixture_repo = workspace.join("synthetic-fixture/fixture-repository");
            for workflow_packet in [&packet, &finalized] {
                let enforcement = Command::new(&staged)
                    .current_dir(&repo_root)
                    .args([
                        "source-promotion",
                        "enforce-admission-workflow",
                        "--packet",
                        path_text(workflow_packet)?,
                        "--workspace-root",
                        path_text(&workspace)?,
                        "--expected-status",
                        "admitted",
                    ])
                    .output()
                    .map_err(|error| format!("positive enforcement launch: {error}"))?;
                if !enforcement.status.success() {
                    return Err(format!(
                        "positive enforcement failed: {}",
                        String::from_utf8_lossy(&enforcement.stderr)
                    ));
                }
            }
            let refs_before = git_output(&fixture_repo, &["show-ref"], &[], None)?;
            let publisher = Command::new(&staged)
                .current_dir(&fixture_repo)
                .args([
                    "source-promotion",
                    "publish-candidate-ref",
                    "--construction-packet",
                    path_text(&workspace.join("exact-join-construction"))?,
                    "--source-main-ref",
                    "refs/heads/main",
                    "--remote",
                    "origin",
                    "--target-ref",
                    construction["candidate_ref"]
                        .as_str()
                        .ok_or("missing construction candidate ref")?,
                    "--expected-absent",
                    "--out",
                    path_text(&workspace.join("public-publication-refusal"))?,
                ])
                .output()
                .map_err(|error| format!("public publisher launch: {error}"))?;
            if publisher.status.success()
                || git_output(&fixture_repo, &["show-ref"], &[], None)? != refs_before
            {
                return Err(format!(
                    "synthetic construction escaped public publication boundary: {}",
                    String::from_utf8_lossy(&publisher.stderr)
                ));
            }
            let evidence = fixture_repo.join(".git/source-promotion-admission-fixture");
            let preflight_path = evidence.join("preflight.json");
            let resolution_path = evidence.join("resolution.json");
            let preflight_bytes = fs::read(&preflight_path).map_err(|error| error.to_string())?;
            let preflight: Value =
                serde_json::from_slice(&preflight_bytes).map_err(|error| error.to_string())?;
            let resolution_bytes = fs::read(&resolution_path).map_err(|error| error.to_string())?;
            let normalized: Value = serde_json::from_slice(
                &fs::read(packet.join("workflow-disposition.json"))
                    .map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
            let closure = packet.join("evidence");
            let integration = closure.join("locators/integration_packet/integration-index.json");
            let admission_out = workspace.join("public-admission-refusal");
            let admission = Command::new(&staged)
                .current_dir(&fixture_repo)
                .args([
                    "source-promotion",
                    "admit-resolved-tree",
                    "--source-parent",
                    preflight["source_parent"].as_str().ok_or("source")?,
                    "--swarm-parent",
                    preflight["swarm_parent"].as_str().ok_or("swarm")?,
                    "--join-tree",
                    preflight["dry_merge"]["reviewed_resolved_tree"]
                        .as_str()
                        .ok_or("tree")?,
                    "--preflight",
                    path_text(&preflight_path)?,
                    "--preflight-sha256",
                    &format!("{:x}", Sha256::digest(&preflight_bytes)),
                    "--resolution-manifest",
                    path_text(&resolution_path)?,
                    "--resolution-sha256",
                    &format!("{:x}", Sha256::digest(&resolution_bytes)),
                    "--validation-packet",
                    path_text(&evidence.join("validation-packet"))?,
                    "--builder-packet",
                    path_text(&closure.join("trusted-builder"))?,
                    "--integration-index",
                    path_text(&integration)?,
                    "--integration-index-sha256",
                    normalized["locators"]["integration_packet"]["sha256"]
                        .as_str()
                        .ok_or("integration digest")?,
                    "--out",
                    path_text(&admission_out)?,
                ])
                .output()
                .map_err(|error| error.to_string())?;
            if admission.status.success()
                || !String::from_utf8_lossy(&admission.stderr).contains("requires preflight v2")
            {
                return Err(format!(
                    "public admission missed native boundary: {}",
                    String::from_utf8_lossy(&admission.stderr)
                ));
            }
            let live_construction_out = workspace.join("public-construction-refusal");
            let live_construction = Command::new(&staged)
                .current_dir(&fixture_repo)
                .args([
                    "source-promotion",
                    "construct-exact-join",
                    "--admission-packet",
                    path_text(&closure.join("resolved-tree-admission"))?,
                    "--validation-packet",
                    path_text(&evidence.join("validation-packet"))?,
                    "--integration-index",
                    path_text(&integration)?,
                    "--integration-index-sha256",
                    normalized["locators"]["integration_packet"]["sha256"]
                        .as_str()
                        .ok_or("integration digest")?,
                    "--preflight",
                    path_text(&preflight_path)?,
                    "--resolution-manifest",
                    path_text(&resolution_path)?,
                    "--qualification-receipt",
                    path_text(&closure.join("locators/qualification_receipt/input"))?,
                    "--qualification-receipt-sha256",
                    normalized["locators"]["qualification_receipt"]["sha256"]
                        .as_str()
                        .ok_or("qualification digest")?,
                    "--source-main-ref",
                    "refs/heads/main",
                    "--swarm-ref",
                    normalized["protected_w7_ref"].as_str().ok_or("W7 ref")?,
                    "--candidate-ref",
                    "refs/heads/promote/0.11.0-public-refusal",
                    "--out",
                    path_text(&live_construction_out)?,
                ])
                .output()
                .map_err(|error| error.to_string())?;
            let refused: Value = serde_json::from_slice(
                &fs::read(live_construction_out.join("exact-join-construction.json"))
                    .map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
            if live_construction.status.success()
                || !String::from_utf8_lossy(&live_construction.stderr)
                    .contains("requires preflight v2")
                || refused["authoritative_commit_attempted"] != false
                || refused["commit_tree_attempts"] != 0
                || git_output(&fixture_repo, &["show-ref"], &[], None)? != refs_before
            {
                return Err(format!(
                    "public construction missed native boundary: {} receipt={refused}",
                    String::from_utf8_lossy(&live_construction.stderr)
                ));
            }
            let live_out = workspace.join("public-live-validation");
            let output = cache_command(Command::new(&staged))
                .current_dir(&fixture_repo)
                .args([
                    "source-promotion",
                    "validate-resolved-tree",
                    "--source-parent",
                    preflight["source_parent"]
                        .as_str()
                        .ok_or("missing source parent")?,
                    "--swarm-parent",
                    preflight["swarm_parent"]
                        .as_str()
                        .ok_or("missing swarm parent")?,
                    "--reviewed-tree",
                    preflight["dry_merge"]["reviewed_resolved_tree"]
                        .as_str()
                        .ok_or("missing tree")?,
                    "--preflight",
                    path_text(&preflight_path)?,
                    "--preflight-sha256",
                    &format!("{:x}", Sha256::digest(&preflight_bytes)),
                    "--resolution-manifest",
                    path_text(&resolution_path)?,
                    "--resolution-sha256",
                    &format!("{:x}", Sha256::digest(&resolution_bytes)),
                    "--out",
                    path_text(&live_out)?,
                ])
                .output()
                .map_err(|error| format!("actual public validator launch: {error}"))?;
            let validation: Value = serde_json::from_slice(
                &fs::read(live_out.join("resolved-tree-validation.json"))
                    .map_err(|error| format!("missing historical diagnostic receipt: {error}"))?,
            )
            .map_err(|error| format!("decode historical diagnostic receipt: {error}"))?;
            retain_initial_preparation_evidence(
                &repo_root,
                &root,
                "workspace/public-live-validation",
                "public",
                nonce,
            );
            let required = [
                "check-network-policy",
                "check-process-policy",
                "check-workflows",
                "check-file-policy",
                "check-dependencies",
                "check-generated-clean",
                "check-executable-files",
                "check-command-catalog",
                "check-spec-format",
                "check-traceability",
                "check-doc-artifacts",
                "check-public-api",
                "check-architecture",
            ];
            let commands = validation["commands"]
                .as_array()
                .ok_or_else(|| "historical diagnostic commands missing".to_string())?;
            if commands.len() != required.len()
                || commands.iter().zip(required).any(|(row, name)| {
                    row["command"].as_str() != Some(name)
                        || row["state"].as_str() != Some("passed")
                        || row["exit_code"].as_i64() != Some(0)
                })
            {
                let mut failed_logs = Vec::new();
                for row in commands.iter().filter(|row| row["state"] != "passed") {
                    for stream in ["stdout_path", "stderr_path"] {
                        if let Some(log) = row[stream].as_str() {
                            match fs::read(live_out.join(log)) {
                                Ok(bytes) => {
                                    let tail = &bytes[bytes.len().saturating_sub(8192)..];
                                    failed_logs.push(String::from_utf8_lossy(tail).into_owned());
                                }
                                Err(error) => {
                                    failed_logs.push(format!("{log}: unavailable: {error}"))
                                }
                            }
                        }
                    }
                }
                return Err(format!(
                    "historical diagnostic did not execute all green commands: receipt={validation}; bounded_failed_output={failed_logs:?}"
                ));
            }
            if output.status.success()
                || validation["status"].as_str() != Some("rejected")
                || !validation["failure_reasons"]
                    .as_array()
                    .is_some_and(|reasons| {
                        reasons.iter().any(|reason| {
                            reason
                                .as_str()
                                .is_some_and(|value| value.contains("requires preflight v2"))
                        })
                    })
                || validation["materialization"]["worktree_remove_succeeded"].as_bool()
                    != Some(true)
                || validation["materialization"]["directory_removed"].as_bool() != Some(true)
                || validation["repository_observation"]["ref_mutation_observed"].as_bool()
                    != Some(false)
                || validation["repository_observation"]["worktree_registry_changed"].as_bool()
                    != Some(false)
            {
                return Err(format!(
                    "green historical diagnostics escaped final native gate: {validation}"
                ));
            }
            return Ok(());
        }
        if output.status.success()
            || !String::from_utf8_lossy(&output.stderr)
                .contains("produced a complete rejected packet")
        {
            let disposition = match fs::read_to_string(packet.join("workflow-disposition.json")) {
                Ok(value) => value,
                Err(error) => format!("unavailable: {error}"),
            };
            return Err(format!(
                "production J5 workflow did not return its expected rejected disposition: status={} stdout={:?} stderr={:?} disposition={disposition}",
                output.status,
                String::from_utf8_lossy(&output.stdout).trim(),
                String::from_utf8_lossy(&output.stderr).trim(),
            ));
        }
        let verification = Command::new(&staged)
            .current_dir(&repo_root)
            .args([
                "source-promotion",
                "verify-admission-workflow",
                "--packet",
                path_text(&packet)?,
                "--requested-identity",
                path_text(&request_path)?,
                "--requested-identity-sha256",
                &request_sha256,
            ])
            .output()
            .map_err(|error| format!("failed to verify production J5 packet: {error}"))?;
        if !verification.status.success() {
            return Err(format!(
                "production J5 packet was not self-verifying: status={} stdout={:?} stderr={:?}",
                verification.status,
                String::from_utf8_lossy(&verification.stdout).trim(),
                String::from_utf8_lossy(&verification.stderr).trim(),
            ));
        }
        let report: Value = serde_json::from_slice(
            &fs::read(packet.join("workflow-disposition.json"))
                .map_err(|error| format!("failed to read production J5 disposition: {error}"))?,
        )
        .map_err(|error| format!("production J5 disposition is malformed: {error}"))?;
        if report.get("status").and_then(Value::as_str) != Some("rejected")
            || report.get("execution_profile").and_then(Value::as_str) != Some("j5_negative")
            || report["controller_packets"]["trusted_builder"]["status"].as_str() != Some("built")
            || report["controller_packets"]["resolved_tree_admission"]["status"].as_str()
                != Some("rejected")
            || report["attempts"]["constructor_commit_tree_attempts"].as_u64() != Some(0)
        {
            return Err("production J5 packet has the wrong disposition or attempts".to_string());
        }
        Ok(())
    })();
    let staged_cleanup = fs::remove_file(&staged)
        .map_err(|error| format!("failed to clean staged production J5 xtask copy: {error}"));
    let cleanup = if result.is_ok() {
        fs::remove_dir_all(&root)
            .map_err(|error| format!("failed to clean production J5 test root: {error}"))
    } else {
        eprintln!(
            "Retained production workflow failure evidence at {}",
            root.display()
        );
        Ok(())
    };
    result.and(staged_cleanup).and(cleanup)
}
