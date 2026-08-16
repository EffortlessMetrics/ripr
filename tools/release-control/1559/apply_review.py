#!/usr/bin/env python3
from __future__ import annotations

import sys
import textwrap
from pathlib import Path


def replace_exact(text: str, old: str, new: str, *, count: int, label: str) -> str:
    found = text.count(old)
    if found != count:
        raise SystemExit(f"{label}: expected {count} match(es), found {found}")
    return text.replace(old, new, count)


def replace_test(text: str, name: str, replacement: str) -> str:
    marker = f"#[test]\nfn {name}"
    start = text.find(marker)
    if start < 0:
        raise SystemExit(f"missing test function: {name}")
    next_test = text.find("\n#[test]\n", start + len(marker))
    end = len(text) if next_test < 0 else next_test + 1
    return text[:start] + replacement.rstrip() + "\n" + text[end:]


def main() -> int:
    if len(sys.argv) != 2:
        raise SystemExit("usage: apply_review.py <repository-root>")
    root = Path(sys.argv[1]).resolve()
    workflow_path = root / ".github/workflows/source-promotion-contract.yml"
    command_path = root / "xtask/src/command.rs"
    integration_path = root / "xtask/tests/source_promotion_workflow_contract.rs"
    spec_path = root / "docs/specs/RIPR-SPEC-0150-source-promotion-ci-contract.md"
    process_policy_path = root / "policy/process_allowlist.txt"

    workflow = workflow_path.read_text(encoding="utf-8")
    workflow = replace_exact(
        workflow,
        "          SOURCE_MAIN: ${{ steps.inputs.outputs.source_main }}\n          JOIN_HEAD: ${{ steps.inputs.outputs.join_head }}\n",
        "          SOURCE_MAIN: ${{ steps.inputs.outputs.source_main }}\n          SOURCE_PARENT: ${{ steps.inputs.outputs.source_parent }}\n          JOIN_HEAD: ${{ steps.inputs.outputs.join_head }}\n",
        count=1,
        label="normalized receipt source-parent handoff",
    )
    workflow_path.write_text(workflow, encoding="utf-8")

    command = command_path.read_text(encoding="utf-8")
    command = replace_exact(
        command,
        '            "SOURCE_PROMOTION_OUT: ${{ runner.temp }}/ripr-source-promotion",\n',
        '            "SOURCE_PROMOTION_OUT: ${{ runner.temp }}/ripr-source-promotion",\n            "SOURCE_PARENT: ${{ steps.inputs.outputs.source_parent }}",\n',
        count=1,
        label="normalized receipt source-parent contract needle",
    )
    command_path.write_text(command, encoding="utf-8")

    integration = integration_path.read_text(encoding="utf-8")
    integration = replace_exact(
        integration,
        "use std::fs;\nuse std::path::PathBuf;\n",
        "use std::fs;\nuse std::io::Write;\nuse std::path::PathBuf;\nuse std::process::{Command, Stdio};\n",
        count=1,
        label="jq test process imports",
    )
    helper = textwrap.dedent(
        r'''
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

        '''
    )
    integration = replace_exact(
        integration,
        "fn workflow_disposition_is_authorized(manifest: &Value, path: &str) -> bool {\n",
        helper + "fn workflow_disposition_is_authorized(manifest: &Value, path: &str) -> bool {\n",
        count=1,
        label="jq contract helper insertion",
    )

    predicate_test = textwrap.dedent(
        r'''
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
                "preflight_sha256": "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
                "resolution_manifest_sha256": "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
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

            let mut malformed = valid;
            malformed["swarm_reachability"]["verified_through_parent_2"] = json!("true");
            assert!(
                !jq_filter_matches(filter, &malformed)?,
                "verifier-receipt jq predicate accepted a non-boolean parent-2 proof"
            );
            Ok(())
        }
        '''
    )
    integration = replace_test(
        integration,
        "verifier_receipt_schema_predicate_is_balanced",
        predicate_test,
    )

    lane_test = textwrap.dedent(
        r'''
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
        '''
    )
    integration = replace_test(
        integration,
        "normalized_contract_is_runner_owned_uploaded_then_enforced",
        lane_test,
    )
    integration_path.write_text(integration, encoding="utf-8")

    spec = spec_path.read_text(encoding="utf-8")
    spec = replace_exact(
        spec,
        "receipt is bound to the PR head, control commit, and input digests and is\nuploaded under a SHA-containing artifact name.\n",
        "receipt is bound to the PR head, control commit, and input digests and is\nuploaded under a SHA-containing artifact name. The normalized PR receipt also\npreserves the trusted `source_parent` identity emitted by input validation.\n",
        count=1,
        label="source-parent receipt specification",
    )
    spec_path.write_text(spec, encoding="utf-8")

    policy = process_policy_path.read_text(encoding="utf-8").rstrip() + "\n"
    rows = [
        "xtask/tests/source_promotion_workflow_contract.rs|Command::new|1|source-promotion-workflow-contract|RIPR-SPEC-0150: focused contract proof executes the exact inline jq verifier-receipt predicate against valid and malformed JSON so syntax and parent-2 boolean validation cannot drift.",
        "xtask/tests/source_promotion_workflow_contract.rs|use std::process::{Command, Stdio}|1|source-promotion-workflow-contract|RIPR-SPEC-0150: focused contract proof imports bounded jq process types solely to execute the workflow predicate against in-memory fixtures.",
    ]
    for row in rows:
        if row in policy:
            raise SystemExit(f"process allowlist row already exists: {row}")
    process_policy_path.write_text(policy + "\n".join(rows) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
