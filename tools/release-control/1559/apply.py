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


def main() -> int:
    if len(sys.argv) != 2:
        raise SystemExit("usage: apply_1559.py <repository-root>")
    root = Path(sys.argv[1]).resolve()
    workflow_path = root / ".github/workflows/source-promotion-contract.yml"
    command_path = root / "xtask/src/command.rs"
    integration_path = root / "xtask/tests/source_promotion_workflow_contract.rs"
    spec_path = root / "docs/specs/RIPR-SPEC-0150-source-promotion-ci-contract.md"

    workflow = workflow_path.read_text(encoding="utf-8")
    malformed = '(.swarm_reachability.verified_through_parent_2 | type) == "boolean")))\' "$verification"'
    balanced = '(.swarm_reachability.verified_through_parent_2 | type) == "boolean"))\' "$verification"'
    workflow = replace_exact(
        workflow,
        malformed,
        balanced,
        count=1,
        label="malformed verifier-receipt predicate",
    )

    workflow = replace_exact(
        workflow,
        "SOURCE_PROMOTION_OUT: target/ripr/source-promotion",
        "SOURCE_PROMOTION_OUT: ${{ runner.temp }}/ripr-source-promotion",
        count=4,
        label="candidate-owned receipt directory",
    )
    workflow = replace_exact(
        workflow,
        "          path: target/ripr/source-promotion",
        "          path: ${{ runner.temp }}/ripr-source-promotion",
        count=2,
        label="candidate-owned artifact path",
    )

    boundary = "\n  post-merge-reachability:\n"
    if "Enforce normalized source-promotion contract" in workflow:
        raise SystemExit("terminal source-promotion enforcement already exists")
    enforcement = "\n".join(
        [
            "      - name: Enforce normalized source-promotion contract",
            "        if: always()",
            "        env:",
            "          SOURCE_PROMOTION_CONTRACT: ${{ runner.temp }}/ripr-source-promotion/source-promotion-contract.json",
            "        run: |",
            "          set -euo pipefail",
            '          test -f "$SOURCE_PROMOTION_CONTRACT"',
            "          jq -e '",
            '            .schema == "ripr.source_promotion_contract.v2" and',
            '            .status == "verified" and',
            '            .validation.status == "passed" and',
            '            .verifier_receipt_status == "present" and',
            '            .verifier_exit_code == "0"',
            "          ' \"$SOURCE_PROMOTION_CONTRACT\"",
            "",
        ]
    )
    workflow = replace_exact(
        workflow,
        boundary,
        "\n" + enforcement + "  post-merge-reachability:\n",
        count=1,
        label="post-merge job boundary",
    )
    workflow_path.write_text(workflow, encoding="utf-8")

    command = command_path.read_text(encoding="utf-8")
    command = replace_exact(
        command,
        '            "SOURCE_PROMOTION_OUT: target/ripr/source-promotion",\n',
        '            "SOURCE_PROMOTION_OUT: ${{ runner.temp }}/ripr-source-promotion",\n',
        count=1,
        label="source-promotion command test output path",
    )
    command_path.write_text(command, encoding="utf-8")

    integration = integration_path.read_text(encoding="utf-8")
    if "fn verifier_receipt_schema_predicate_is_balanced" in integration:
        raise SystemExit("#1559 integration tests already exist")
    addition = textwrap.dedent(
        r'''
        #[test]
        fn verifier_receipt_schema_predicate_is_balanced() -> Result<(), String> {
            let workflow = workflow_text()?;
            let malformed =
                "(.swarm_reachability.verified_through_parent_2 | type) == \"boolean\")))' \"$verification\"";
            let balanced =
                "(.swarm_reachability.verified_through_parent_2 | type) == \"boolean\"))' \"$verification\"";
            assert!(
                !workflow.contains(malformed),
                "verifier-receipt jq predicate retains an unmatched parenthesis"
            );
            assert!(
                workflow.contains(balanced),
                "workflow no longer contains the balanced verified-receipt predicate"
            );
            Ok(())
        }

        #[test]
        fn normalized_contract_is_runner_owned_uploaded_then_enforced() -> Result<(), String> {
            let workflow = workflow_text()?;
            let promotion_job = workflow
                .split("\n  post-merge-reachability:\n")
                .next()
                .ok_or_else(|| "missing promotion-contract job".to_string())?;

            for forbidden in [
                "SOURCE_PROMOTION_OUT: target/ripr/source-promotion",
                "path: target/ripr/source-promotion",
            ] {
                assert!(
                    !promotion_job.contains(forbidden),
                    "candidate checkout still owns promotion evidence: {forbidden}"
                );
            }
            for required in [
                "SOURCE_PROMOTION_OUT: ${{ runner.temp }}/ripr-source-promotion",
                "path: ${{ runner.temp }}/ripr-source-promotion",
                "SOURCE_PROMOTION_CONTRACT: ${{ runner.temp }}/ripr-source-promotion/source-promotion-contract.json",
            ] {
                assert!(
                    promotion_job.contains(required),
                    "runner-owned promotion contract missing: {required}"
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
    ).lstrip()
    integration_path.write_text(integration.rstrip() + "\n\n" + addition, encoding="utf-8")

    spec = spec_path.read_text(encoding="utf-8")
    behavior_old = """The workflow runs `cargo xtask source-promotion verify` and emits one
normalized `ripr.source_promotion_contract.v2` receipt plus the verifier
JSON/Markdown receipts. Every receipt is bound to the PR head, control commit,
and input digests and is uploaded under a SHA-containing artifact name.
"""
    behavior_new = """The workflow runs `cargo xtask source-promotion verify` and emits one
normalized `ripr.source_promotion_contract.v2` receipt plus the verifier
JSON/Markdown receipts. PR and post-merge receipt directories live under the
runner-owned temporary directory rather than the candidate checkout. Every
receipt is bound to the PR head, control commit, and input digests and is
uploaded under a SHA-containing artifact name.

After the PR receipt upload, an always-run terminal enforcement step reads the
normalized contract and succeeds only when its schema is
`ripr.source_promotion_contract.v2`, its status is `verified`, validation is
`passed`, the verifier receipt is `present`, and the verifier exit code is zero.
Rejected evidence is therefore retained before the hosted job fails; a missing,
malformed, rejected, candidate-supplied, or non-zero-verifier receipt cannot
produce a green `Source Promotion Contract` check.
"""
    spec = replace_exact(
        spec,
        behavior_old,
        behavior_new,
        count=1,
        label="RIPR-SPEC-0150 receipt behavior",
    )
    evidence_old = """- uploaded receipts retain exact heads, ordered parents, input digests, checks,
  failure reasons, and claim boundaries.
"""
    evidence_new = """- uploaded receipts retain exact heads, ordered parents, input digests, checks,
  failure reasons, and claim boundaries;
- the retained verified `ripr.source_promotion_verification.v2` shape passes the
  balanced normalizer predicate while malformed shapes fail closed;
- a rejected normalized contract is uploaded and then fails the hosted job,
  while a verified, passed, present, zero-exit contract passes terminal
  enforcement;
- candidate-checkout files cannot substitute for runner-owned verifier or
  normalized-contract receipts.
"""
    spec = replace_exact(
        spec,
        evidence_old,
        evidence_new,
        count=1,
        label="RIPR-SPEC-0150 required evidence",
    )
    mapping_old = """contract tests in `xtask/tests/source_promotion_workflow_contract.rs`, including
missing/source-only, mixed allowed/source, and one-or-more unanimously allowed
workflow-disposition cases. The exact-J graph, `kind:key` completeness, duplicate
"""
    mapping_new = """contract tests in `xtask/tests/source_promotion_workflow_contract.rs`, including
missing/source-only, mixed allowed/source, one-or-more unanimously allowed
workflow-disposition cases, the balanced verifier-receipt predicate, and
runner-owned upload-before-enforcement ordering. The exact-J graph, `kind:key`
completeness, duplicate
"""
    spec = replace_exact(
        spec,
        mapping_old,
        mapping_new,
        count=1,
        label="RIPR-SPEC-0150 test mapping",
    )
    spec_path.write_text(spec, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
