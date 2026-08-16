#!/usr/bin/env bash
set -euo pipefail

: "${SOURCE_BASE:?SOURCE_BASE is required}"
: "${TRANSPORT_BRANCH:?TRANSPORT_BRANCH is required}"
: "${RUNNER_TEMP:?RUNNER_TEMP is required}"

control_root=$(git rev-parse --show-toplevel)
source_dir="$RUNNER_TEMP/ripr-1559-source"
apply_script="$control_root/tools/release-control/1559/apply.py"
verified_receipt="$control_root/tools/release-control/1559/verified-j2-receipt.json"
rejected_contract="$control_root/tools/release-control/1559/rejected-j2-contract.json"

for path in "$apply_script" "$verified_receipt" "$rejected_contract"; do
  test -f "$path"
done

git config user.name EffortlessSteven
git config user.email git@effortlesssteven.com
rm -rf "$source_dir"
git worktree add --detach "$source_dir" "$SOURCE_BASE"
python "$apply_script" "$source_dir"

cargo fmt --manifest-path "$source_dir/Cargo.toml" --all
git -C "$source_dir" diff --check
cargo test --manifest-path "$source_dir/Cargo.toml" --locked -p xtask --test source_promotion_workflow_contract
cargo test --manifest-path "$source_dir/Cargo.toml" --locked -p xtask source_promotion_workflow -- --nocapture
cargo run --manifest-path "$source_dir/Cargo.toml" --locked -p xtask -- check-workflows
cargo run --manifest-path "$source_dir/Cargo.toml" --locked -p xtask -- check-spec-format
cargo run --manifest-path "$source_dir/Cargo.toml" --locked -p xtask -- check-traceability
cargo run --manifest-path "$source_dir/Cargo.toml" --locked -p xtask -- check-doc-artifacts

python - "$source_dir/.github/workflows/source-promotion-contract.yml" "$RUNNER_TEMP/verifier-filter.jq" "$RUNNER_TEMP/contract-filter.jq" <<'PY'
from pathlib import Path
import sys

workflow_path, verifier_filter_path, contract_filter_path = map(Path, sys.argv[1:])
workflow = workflow_path.read_text(encoding="utf-8")

start_marker = 'if test -f "$verification"; then\n            if jq -e \' '
start_marker = start_marker[:-1]
end_marker = '\' "$verification" >/dev/null 2>&1; then'
start = workflow.index(start_marker) + len(start_marker)
end = workflow.index(end_marker, start)
verifier_filter_path.write_text(workflow[start:end] + "\n", encoding="utf-8")

block_start = workflow.index("- name: Enforce normalized source-promotion contract")
block_end = workflow.index("\n  post-merge-reachability:\n", block_start)
block = workflow[block_start:block_end]
filter_start_marker = "jq -e '\n"
filter_end_marker = "\n          ' \"$SOURCE_PROMOTION_CONTRACT\""
start = block.index(filter_start_marker) + len(filter_start_marker)
end = block.index(filter_end_marker, start)
contract_filter_path.write_text(block[start:end] + "\n", encoding="utf-8")
PY

jq -e -f "$RUNNER_TEMP/verifier-filter.jq" "$verified_receipt" >/dev/null
jq '.swarm_reachability.verified_through_parent_2 = "true"' "$verified_receipt" > "$RUNNER_TEMP/malformed-verifier-receipt.json"
if jq -e -f "$RUNNER_TEMP/verifier-filter.jq" "$RUNNER_TEMP/malformed-verifier-receipt.json" >/dev/null; then
  echo "malformed verifier receipt unexpectedly passed" >&2
  exit 1
fi

cat > "$RUNNER_TEMP/verified-normalized-contract.json" <<'JSON'
{"schema":"ripr.source_promotion_contract.v2","status":"verified","validation":{"status":"passed"},"verifier_receipt_status":"present","verifier_exit_code":"0"}
JSON
jq -e -f "$RUNNER_TEMP/contract-filter.jq" "$RUNNER_TEMP/verified-normalized-contract.json" >/dev/null
if jq -e -f "$RUNNER_TEMP/contract-filter.jq" "$rejected_contract" >/dev/null; then
  echo "retained rejected normalized contract unexpectedly passed" >&2
  exit 1
fi

workflow="$source_dir/.github/workflows/source-promotion-contract.yml"
command_tests="$source_dir/xtask/src/command.rs"
integration_tests="$source_dir/xtask/tests/source_promotion_workflow_contract.rs"
spec="$source_dir/docs/specs/RIPR-SPEC-0150-source-promotion-ci-contract.md"
for path in "$workflow" "$command_tests" "$integration_tests" "$spec"; do
  test -f "$path"
done

cd "$control_root"
git reset --hard "$SOURCE_BASE"
git clean -fdx
git switch -C "$TRANSPORT_BRANCH"
transport=transport/1559-final
mkdir -p "$transport"
cp "$workflow" "$transport/source-promotion-contract.yml"
cp "$command_tests" "$transport/command.rs"
cp "$integration_tests" "$transport/source_promotion_workflow_contract.rs"
cp "$spec" "$transport/RIPR-SPEC-0150-source-promotion-ci-contract.md"
sha256sum "$transport"/*.md "$transport"/*.rs "$transport"/*.yml > "$transport/sha256sums.txt"
jq -n \
  --arg source_base "$SOURCE_BASE" \
  --arg workflow_sha256 "$(sha256sum "$transport/source-promotion-contract.yml" | awk '{print $1}')" \
  --arg command_sha256 "$(sha256sum "$transport/command.rs" | awk '{print $1}')" \
  --arg integration_sha256 "$(sha256sum "$transport/source_promotion_workflow_contract.rs" | awk '{print $1}')" \
  --arg spec_sha256 "$(sha256sum "$transport/RIPR-SPEC-0150-source-promotion-ci-contract.md" | awk '{print $1}')" \
  '{schema:"ripr.source_promotion_contract_repair_transport.v1",source_base:$source_base,files:{workflow:$workflow_sha256,command_tests:$command_sha256,integration_tests:$integration_sha256,spec:$spec_sha256},proof:["cargo fmt --all","git diff --check","cargo test -p xtask --test source_promotion_workflow_contract","cargo test -p xtask source_promotion_workflow","check-workflows","check-spec-format","check-traceability","check-doc-artifacts","retained verified J2 receipt accepted by exact jq filter","malformed verifier receipt rejected","verified normalized contract accepted","retained rejected normalized contract rejected"],non_claims:["no merge","no J reconstruction","no version change","no tag","no publication","no signing","no marketplace mutation","no secret use","no back-sync"]}' \
  > "$transport/receipt.json"

git add "$transport"
git commit -m "chore(transport): retain verified #1559 source repair blobs"
git push --force origin "HEAD:refs/heads/$TRANSPORT_BRANCH"
