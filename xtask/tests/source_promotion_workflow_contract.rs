use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

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
        "\"$SOURCE_REPOSITORY/.github/workflows/source-promotion-admission.yml@\"*",
        "test \"$GITHUB_REF\" = refs/heads/main",
        "test \"$GITHUB_SHA\" = \"$SOURCE_PARENT_SHA\"",
        "test \"$WORKFLOW_SOURCE_SHA\" = \"$SOURCE_PARENT_SHA\"",
        "test \"$(git rev-parse HEAD)\" = \"$WORKFLOW_SOURCE_SHA\"",
        "ripr.source_promotion_admission_workflow.v1",
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
