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
