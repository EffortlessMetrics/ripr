#!/usr/bin/env bash
set -euo pipefail

required_env=(
  OLD_SOURCE_PARENT SOURCE_PARENT SWARM_PARENT SWARM_REF BASE_J BASE_TREE
  CONTROL_BASE EXPECTED_PREVIEW_TREE EXPECTED_CONFLICTS_SHA256
  EXPECTED_SURVIVORS_SHA256 EXPECTED_SWARM_ONLY_SHA256
  EXPECTED_AUTHORITY_SHA256 EXPECTED_VERSION_STATE_SHA256
  EXPECTED_SOURCE_ALL_COUNT EXPECTED_SOURCE_FIRST_PARENT_COUNT
  EXPECTED_SOURCE_ALL_SHA256 EXPECTED_SOURCE_FIRST_PARENT_SHA256
  EXPECTED_SWARM_COUNT EXPECTED_SWARM_SHA256 J3_BRANCH CONTROL_BRANCH VERSION
)
for name in "${required_env[@]}"; do
  test -n "${!name:-}" || { printf 'missing required environment: %s\n' "$name" >&2; exit 2; }
done

root=$PWD
out="$root/target/j3-final"
rm -rf "$out"
mkdir -p "$out"
stage=initialization
cleanup_receipt() {
  status=$?
  if test "$status" -ne 0; then
    printf 'status=failure\nstage=%s\n' "$stage" > "$out/failure.env"
  fi
  exit "$status"
}
trap cleanup_receipt EXIT

git config user.name EffortlessSteven
git config user.email git@effortlesssteven.com

test "$(git rev-parse origin/main)" = "$SOURCE_PARENT"
test "$(git ls-remote --refs origin refs/heads/main | awk '{print $1}')" = "$SOURCE_PARENT"

stage=fetch_retained_authorities
git fetch --no-tags origin \
  "$OLD_SOURCE_PARENT" \
  "$SOURCE_PARENT" \
  "refs/heads/promote/0.11.0-swarm-w7-a072b7-j2-catalog:refs/remotes/origin/old-j2" \
  "refs/heads/release/0.11.0-promotion-control-a072b7-w7-j2:refs/remotes/origin/old-control"
test "$(git rev-parse refs/remotes/origin/old-j2)" = "$BASE_J"
test "$(git rev-parse refs/remotes/origin/old-control)" = "$CONTROL_BASE"
test "$(git rev-parse "$BASE_J^{tree}")" = "$BASE_TREE"
test "$(git show -s --format='%P' "$BASE_J")" = "$OLD_SOURCE_PARENT $SWARM_PARENT"
test "$(git -C swarm rev-parse HEAD)" = "$SWARM_PARENT"
test "$(git -C swarm rev-parse "$SWARM_REF^{commit}")" = "$SWARM_PARENT"

candidate_dir="$RUNNER_TEMP/ripr-j3-candidate"
control_dir="$RUNNER_TEMP/ripr-j3-control"
source_dir="$RUNNER_TEMP/ripr-j3-source"
rm -rf "$candidate_dir" "$control_dir" "$source_dir"
git worktree add --detach "$candidate_dir" "$BASE_J"
git worktree add --detach "$control_dir" "$CONTROL_BASE"
git worktree add --detach "$source_dir" "$SOURCE_PARENT"

stage=bind_source_delta
expected_paths="$RUNNER_TEMP/source-delta-paths.expected"
actual_paths="$RUNNER_TEMP/source-delta-paths.actual"
cat > "$expected_paths" <<'EOF'
.github/workflows/source-promotion-contract.yml
docs/specs/RIPR-SPEC-0150-source-promotion-ci-contract.md
policy/process_allowlist.txt
xtask/src/command.rs
xtask/tests/source_promotion_workflow_contract.rs
EOF
git diff --name-only "$OLD_SOURCE_PARENT" "$SOURCE_PARENT" | sort > "$actual_paths"
diff -u "$expected_paths" "$actual_paths"

full_patch="$RUNNER_TEMP/source-1560.delta.patch"
git diff --binary "$OLD_SOURCE_PARENT" "$SOURCE_PARENT" -- \
  .github/workflows/source-promotion-contract.yml \
  docs/specs/RIPR-SPEC-0150-source-promotion-ci-contract.md \
  policy/process_allowlist.txt \
  xtask/src/command.rs \
  xtask/tests/source_promotion_workflow_contract.rs > "$full_patch"
test -s "$full_patch"

clean_patch="$RUNNER_TEMP/source-1560.clean-paths.patch"
git diff --binary "$OLD_SOURCE_PARENT" "$SOURCE_PARENT" -- \
  .github/workflows/source-promotion-contract.yml \
  docs/specs/RIPR-SPEC-0150-source-promotion-ci-contract.md \
  xtask/src/command.rs \
  xtask/tests/source_promotion_workflow_contract.rs > "$clean_patch"
test -s "$clean_patch"
git -C "$candidate_dir" apply --3way --index "$clean_patch"
test -z "$(git -C "$candidate_dir" diff --name-only --diff-filter=U)"

stage=integrate_process_allowlist
old_allow="$RUNNER_TEMP/process-allowlist-old-source.txt"
new_allow="$RUNNER_TEMP/process-allowlist-new-source.txt"
base_allow="$RUNNER_TEMP/process-allowlist-j2.txt"
additions="$RUNNER_TEMP/process-allowlist-1560-additions.txt"
git show "$OLD_SOURCE_PARENT:policy/process_allowlist.txt" > "$old_allow"
git show "$SOURCE_PARENT:policy/process_allowlist.txt" > "$new_allow"
git show "$BASE_J:policy/process_allowlist.txt" > "$base_allow"
python - "$old_allow" "$new_allow" "$additions" <<'PY'
from pathlib import Path
import sys
old_path, new_path, additions_path = map(Path, sys.argv[1:])
old_lines = old_path.read_text(encoding="utf-8").splitlines()
new_lines = new_path.read_text(encoding="utf-8").splitlines()
old_set = set(old_lines)
new_set = set(new_lines)
removed = [line for line in old_lines if line and line not in new_set]
added = [line for line in new_lines if line and line not in old_set]
expected = [
    'xtask/tests/source_promotion_workflow_contract.rs|Command::new|1|source-promotion-workflow-contract|RIPR-SPEC-0150: focused contract proof executes the exact inline jq verifier-receipt predicate against valid and malformed JSON so syntax and parent-2 boolean validation cannot drift.',
    'xtask/tests/source_promotion_workflow_contract.rs|use std::process::{Command, Stdio}|1|source-promotion-workflow-contract|RIPR-SPEC-0150: focused contract proof imports bounded jq process types solely to execute the workflow predicate against in-memory fixtures.',
]
if removed:
    raise SystemExit(f"#1560 unexpectedly removed process-policy rows: {removed}")
if added != expected:
    raise SystemExit(f"#1560 process-policy additions drifted: {added!r}")
additions_path.write_text("\n".join(added) + "\n", encoding="utf-8")
PY
python - "$base_allow" "$additions" "$candidate_dir/policy/process_allowlist.txt" <<'PY'
from pathlib import Path
import sys
base_path, additions_path, output_path = map(Path, sys.argv[1:])
base = base_path.read_text(encoding="utf-8")
added = additions_path.read_text(encoding="utf-8").splitlines()
for row in added:
    if row in base.splitlines():
        raise SystemExit(f"new #1560 row already present in J2 baseline: {row}")
if not base.endswith("\n"):
    base += "\n"
result = base + "\n".join(added) + "\n"
output_path.write_text(result, encoding="utf-8")
if output_path.read_text(encoding="utf-8") != result:
    raise SystemExit("integrated process allowlist did not round-trip")
PY
git -C "$candidate_dir" add policy/process_allowlist.txt

git -C "$candidate_dir" diff --cached --name-only | sort > "$RUNNER_TEMP/candidate-paths.actual"
diff -u "$expected_paths" "$RUNNER_TEMP/candidate-paths.actual"
git -C "$candidate_dir" diff --cached --check
python - "$base_allow" "$additions" "$candidate_dir/policy/process_allowlist.txt" <<'PY'
from pathlib import Path
import sys
base_path, additions_path, candidate_path = map(Path, sys.argv[1:])
base = base_path.read_text(encoding="utf-8")
if not base.endswith("\n"):
    base += "\n"
expected = base + additions_path.read_text(encoding="utf-8")
actual = candidate_path.read_text(encoding="utf-8")
if actual != expected:
    raise SystemExit("process allowlist is not exact J2 bytes plus the two reviewed #1560 rows")
PY

stage=validate_integrated_catalog
python - "$candidate_dir/xtask/src/command.rs" <<'PY'
from pathlib import Path
import re
import sys
text = Path(sys.argv[1]).read_text(encoding="utf-8")
for needle in [
    "SOURCE_PROMOTION_OUT: $" + "{{ runner.temp }}/ripr-source-promotion",
    "SOURCE_PARENT: $" + "{{ steps.inputs.outputs.source_parent }}",
]:
    if needle not in text:
        raise SystemExit(f"missing #1560 command-catalog contract needle: {needle}")
targets = [
    "precommit", "check-agent-skills", "check-workspace-shape",
    "check-architecture", "check-public-api", "check-doc-artifacts",
    "check-readme-state", "markdown-links", "check-pr-shape",
    "check-lint-policy", "check-proof-packs", "check-release-targets",
]
blocks = list(re.finditer(r"(?ms)^        command_entry\(\n(?P<body>.*?)^        \),\n", text))
for target in targets:
    matches = []
    for match in blocks:
        body = match.group("body")
        first = re.match(r'^            "([^"]+)",\n', body)
        if first and (first.group(1) == target or first.group(1).startswith(target + " [")):
            matches.append((body, first.group(1)))
    if len(matches) != 1:
        raise SystemExit(f"expected one command entry for {target}, found {len(matches)}")
    body, specification = matches[0]
    booleans = list(re.finditer(r"(?m)^            (true|false),$", body))
    if len(booleans) < 2 or booleans[1].group(1) != "true":
        raise SystemExit(f"{specification} lost the reviewed ci_enforced=true disposition")
PY
grep -F 'Enforce normalized source-promotion contract' \
  "$candidate_dir/.github/workflows/source-promotion-contract.yml"
grep -F 'source-promotion-workflow-contract' "$candidate_dir/policy/process_allowlist.txt"

new_tree=$(git -C "$candidate_dir" write-tree)
git -C "$candidate_dir" diff-tree --no-commit-id --name-only -r "$BASE_TREE" "$new_tree" \
  | sort > "$RUNNER_TEMP/tree-paths.actual"
diff -u "$expected_paths" "$RUNNER_TEMP/tree-paths.actual"
printf 'candidate_tree=%s\n' "$new_tree" > "$out/candidate-tree.env"

stage=generate_resolved_preflight
preflight_out="$RUNNER_TEMP/current-pair-preflight"
rm -rf "$preflight_out"
mkdir -p "$preflight_out" "$root/swarm/target"
(
  cd "$root/swarm"
  CARGO_TARGET_DIR="$RUNNER_TEMP/ripr-w7-preflight-target" \
    cargo xtask source-promotion preflight \
      --source-parent "$SOURCE_PARENT" \
      --swarm-parent "$SWARM_PARENT" \
      --swarm-ref "$SWARM_REF" \
      --source-repo "$source_dir" \
      --swarm-repo "$root/swarm" \
      --source-main origin/main \
      --swarm-main origin/main \
      --version "$VERSION" \
      --resolved-tree "$new_tree" \
      --out "$preflight_out"
)

generated_preflight="$preflight_out/source-promotion-preflight.json"
preflight="$control_dir/docs/release/source-promotion/preflight.json"
resolution="$control_dir/docs/release/source-promotion/resolution-manifest.json"
inputs="$control_dir/docs/release/source-promotion/contract-inputs.json"
cp "$generated_preflight" "$preflight"

stage=validate_preflight_denominator
python - "$preflight" "$new_tree" <<'PY'
import hashlib
import json
import os
from pathlib import Path
import sys
path = Path(sys.argv[1])
expected_tree = sys.argv[2]
data = json.loads(path.read_text(encoding="utf-8"))
expected = {
    "source_parent": os.environ["SOURCE_PARENT"],
    "source_main": os.environ["SOURCE_PARENT"],
    "swarm_parent": os.environ["SWARM_PARENT"],
    "swarm_ref": os.environ["SWARM_REF"],
    "swarm_ref_sha": os.environ["SWARM_PARENT"],
    "merge_base": "36909460db013ed3a3238ee8b2fc3ccda1135c15",
}
for key, value in expected.items():
    if data.get(key) != value:
        raise SystemExit(f"preflight {key} mismatch: {data.get(key)!r} != {value!r}")
dry = data.get("dry_merge") or {}
if dry.get("preview_tree") != os.environ["EXPECTED_PREVIEW_TREE"]:
    raise SystemExit("dry-merge preview tree drifted")
if dry.get("reviewed_resolved_tree") != expected_tree or dry.get("reviewed_resolved_tree_verified") is not True:
    raise SystemExit("reviewed tree was not verified")
if data["source_range"]["all_reachable_count"] != int(os.environ["EXPECTED_SOURCE_ALL_COUNT"]):
    raise SystemExit("source all-reachable count drifted")
if data["source_range"]["first_parent_count"] != int(os.environ["EXPECTED_SOURCE_FIRST_PARENT_COUNT"]):
    raise SystemExit("source first-parent count drifted")
if data["source_range"]["all_reachable_sha256"] != os.environ["EXPECTED_SOURCE_ALL_SHA256"]:
    raise SystemExit("source all-reachable digest drifted")
if data["source_range"]["first_parent_ordered_sha256"] != os.environ["EXPECTED_SOURCE_FIRST_PARENT_SHA256"]:
    raise SystemExit("source first-parent digest drifted")
if data["swarm_range"]["all_reachable_count"] != int(os.environ["EXPECTED_SWARM_COUNT"]):
    raise SystemExit("swarm count drifted")
if data["swarm_range"]["all_reachable_sha256"] != os.environ["EXPECTED_SWARM_SHA256"]:
    raise SystemExit("swarm digest drifted")
if data["swarm_range"]["first_parent_ordered_sha256"] != os.environ["EXPECTED_SWARM_SHA256"]:
    raise SystemExit("swarm first-parent digest drifted")
def digest(value):
    raw = (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()
    return hashlib.sha256(raw).hexdigest()
checks = [
    (dry.get("conflicts"), "EXPECTED_CONFLICTS_SHA256", 20),
    (data.get("source_survivor_candidates"), "EXPECTED_SURVIVORS_SHA256", 100),
    (data.get("swarm_only_paths"), "EXPECTED_SWARM_ONLY_SHA256", 1774),
    (data.get("swarm_authority_resolution_candidates"), "EXPECTED_AUTHORITY_SHA256", 57),
]
for value, env_key, count in checks:
    if not isinstance(value, list) or len(value) != count:
        raise SystemExit(f"{env_key} denominator count drifted")
    if digest(value) != os.environ[env_key]:
        raise SystemExit(f"{env_key} denominator identity drifted")
if digest(data.get("version_state")) != os.environ["EXPECTED_VERSION_STATE_SHA256"]:
    raise SystemExit("version-state identity drifted")
PY

stage=refresh_resolution_manifest
preflight_sha=$(sha256sum "$preflight" | awk '{print $1}')
python - "$resolution" "$inputs" "$preflight_sha" "$new_tree" <<'PY'
import hashlib
import json
import os
from pathlib import Path
import sys
resolution_path = Path(sys.argv[1])
inputs_path = Path(sys.argv[2])
preflight_sha = sys.argv[3]
new_tree = sys.argv[4]
old_source = os.environ["OLD_SOURCE_PARENT"]
new_source = os.environ["SOURCE_PARENT"]
old_tree = os.environ["BASE_TREE"]
resolution = json.loads(resolution_path.read_text(encoding="utf-8"))
if resolution.get("source_parent") != old_source or resolution.get("reviewed_join_tree") != old_tree:
    raise SystemExit("control manifest is not the accepted J2 baseline")
resolution["source_parent"] = new_source
resolution["preflight_sha256"] = "sha256:" + preflight_sha
resolution["reviewed_join_tree"] = new_tree
counts = {"command_source": 0, "allow_conflict": 0, "allow_source": 0}
for row in resolution.get("dispositions", []):
    evidence = row.get("evidence")
    if isinstance(evidence, str):
        row["evidence"] = evidence.replace(old_source, new_source).replace(old_tree, new_tree)
    kind = row.get("kind")
    key = row.get("key")
    if kind == "source_survivor" and key == "xtask/src/command.rs":
        counts["command_source"] += 1
        row["disposition"] = "integrated"
        row["rationale"] = "Integrated tree retains the reviewed W7 ci_enforced catalog correction and the #1560 source receipt-contract assertions."
        row["evidence"] = f"JOIN_TREE {new_tree}; #1557 catalog repair; #1560 receipt-contract delta; base J2 {os.environ['BASE_J']}"
    if key == "policy/process_allowlist.txt" and kind in {"conflict", "source_survivor"}:
        counts["allow_conflict" if kind == "conflict" else "allow_source"] += 1
        row["disposition"] = "integrated"
        row["rationale"] = "Integrated tree preserves the accepted J2 process-policy denominator and appends exactly the two reviewed #1560 workflow-contract test rows."
        row["evidence"] = f"JOIN_TREE {new_tree}; exact J2 bytes plus #1560 two-row source delta"
if counts != {"command_source": 1, "allow_conflict": 1, "allow_source": 1}:
    raise SystemExit(f"unexpected integrated disposition counts: {counts}")
resolution_path.write_text(json.dumps(resolution, indent=2) + "\n", encoding="utf-8")
resolution_sha = hashlib.sha256(resolution_path.read_bytes()).hexdigest()
inputs = json.loads(inputs_path.read_text(encoding="utf-8"))
inputs["source_main"] = new_source
inputs["preflight_sha256"] = preflight_sha
inputs["resolution_manifest_sha256"] = resolution_sha
inputs_path.write_text(json.dumps(inputs, indent=2) + "\n", encoding="utf-8")
PY

stage=verify_resolution_authority
jq -r '.dispositions[] | select(.kind == "source_survivor" and .disposition == "source_blob") | .key' \
  "$resolution" > "$RUNNER_TEMP/source-blob-paths"
while IFS= read -r path; do
  test -n "$path"
  candidate_blob=$(git -C "$candidate_dir" rev-parse "$new_tree:$path")
  source_blob=$(git -C "$source_dir" rev-parse "$SOURCE_PARENT:$path")
  if test "$candidate_blob" != "$source_blob"; then
    printf 'source_blob disposition mismatch for %s: candidate=%s source=%s\n' \
      "$path" "$candidate_blob" "$source_blob" >&2
    exit 1
  fi
done < "$RUNNER_TEMP/source-blob-paths"
test "$(jq -r '[.dispositions[] | select(.kind == "source_survivor" and .key == "xtask/src/command.rs" and .disposition == "integrated")] | length' "$resolution")" = 1
test "$(jq -r '[.dispositions[] | select(.key == "policy/process_allowlist.txt" and .disposition == "integrated")] | length' "$resolution")" = 2
test "$(jq -r '[.dispositions[] | select(.kind == "conflict")] | length' "$resolution")" = 20
test "$(jq -r '[.dispositions[] | select(.kind == "source_survivor")] | length' "$resolution")" = 100
test "$(jq -r '[.dispositions[] | select(.kind == "swarm_authority")] | length' "$resolution")" = 57

stage=candidate_focused_proof
candidate_target="$RUNNER_TEMP/ripr-j3-candidate-target"
(
  cd "$candidate_dir"
  export CARGO_TARGET_DIR="$candidate_target"
  cargo fmt --all -- --check
  cargo test -p xtask --test source_promotion_workflow_contract --locked
  cargo test -p xtask --locked source_promotion -- --nocapture
  cargo test -p xtask --locked command_catalog_ci_enforced_flags_match_repo_workflows -- --nocapture
  cargo xtask check-workflows
  cargo xtask check-process-policy
  cargo xtask check-doc-artifacts
  cargo xtask check-spec-format
  cargo xtask check-spec-numbering
  cargo xtask check-traceability
  cargo xtask precommit
)

stage=create_exact_j3
export GIT_AUTHOR_NAME=EffortlessSteven GIT_AUTHOR_EMAIL=git@effortlesssteven.com
export GIT_COMMITTER_NAME=EffortlessSteven GIT_COMMITTER_EMAIL=git@effortlesssteven.com
export GIT_AUTHOR_DATE=2026-08-16T10:00:00Z GIT_COMMITTER_DATE=2026-08-16T10:00:00Z
j3=$(printf '%s\n' \
  'promote: join current source with frozen ripr-swarm W7 for 0.11.0' '' \
  'Reuse the accepted J2 resolution, integrate the reviewed #1560' \
  'receipt-contract delta, and preserve both exact parent graphs.' \
  | git -C "$candidate_dir" commit-tree "$new_tree" -p "$SOURCE_PARENT" -p "$SWARM_PARENT")
test "$(git -C "$candidate_dir" show -s --format='%P' "$j3")" = "$SOURCE_PARENT $SWARM_PARENT"
test "$(git -C "$candidate_dir" rev-parse "$j3^{tree}")" = "$new_tree"
git -C "$candidate_dir" merge-base --is-ancestor "$SOURCE_PARENT" "$j3"
git -C "$candidate_dir" merge-base --is-ancestor "$SWARM_PARENT" "$j3"

resolution_sha=$(sha256sum "$resolution" | awk '{print $1}')
python - "$inputs" "$j3" "$preflight_sha" "$resolution_sha" <<'PY'
import json
from pathlib import Path
import sys
path = Path(sys.argv[1])
join, preflight_sha, resolution_sha = sys.argv[2:]
data = json.loads(path.read_text(encoding="utf-8"))
data["join_head"] = join
data["preflight_sha256"] = preflight_sha
data["resolution_manifest_sha256"] = resolution_sha
path.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
PY

stage=trusted_source_verification
trusted_target="$RUNNER_TEMP/ripr-j3-trusted-target"
CARGO_TARGET_DIR="$trusted_target" \
  cargo build --locked --manifest-path "$source_dir/Cargo.toml" -p xtask --bin xtask
"$trusted_target/debug/xtask" source-promotion verify \
  --preflight "$preflight" \
  --resolution-manifest "$resolution" \
  --join-head "$j3" \
  --source-main "$SOURCE_PARENT" \
  --out "$RUNNER_TEMP/source-promotion-j3"
jq -e '.status == "verified" and .join_head == $join and .tree == $tree' \
  --arg join "$j3" --arg tree "$new_tree" \
  "$RUNNER_TEMP/source-promotion-j3/source-promotion-verification.json" >/dev/null

stage=create_control_commit
git -C "$control_dir" add docs/release/source-promotion
control_tree=$(git -C "$control_dir" write-tree)
export GIT_AUTHOR_DATE=2026-08-16T10:01:00Z GIT_COMMITTER_DATE=2026-08-16T10:01:00Z
control_commit=$(printf '%s\n' \
  'release(control): bind current-source W7 J3 inputs' '' \
  'Retain the fresh preflight, reviewed resolution manifest, exact J3,' \
  'and source-promotion verifier identities.' \
  | git -C "$control_dir" commit-tree "$control_tree" -p "$CONTROL_BASE")
merge_base=$(git -C "$control_dir" merge-base "$control_commit" "$j3" || true)
test "$merge_base" != "$control_commit"
test "$merge_base" != "$j3"

stage=publish_exact_refs
test "$(git ls-remote --refs origin "refs/heads/$J3_BRANCH" | wc -l)" -eq 0
test "$(git ls-remote --refs origin "refs/heads/$CONTROL_BRANCH" | wc -l)" -eq 0
test "$(git ls-remote --refs origin refs/heads/main | awk '{print $1}')" = "$SOURCE_PARENT"
git push origin "$j3:refs/heads/$J3_BRANCH"
git push origin "$control_commit:refs/heads/$CONTROL_BRANCH"

stage=retain_packet
cp "$inputs" "$out/contract-inputs.json"
cp "$preflight" "$out/preflight.json"
cp "$resolution" "$out/resolution-manifest.json"
cp "$RUNNER_TEMP/source-promotion-j3/source-promotion-verification.json" "$out/verification.json"
cp "$full_patch" "$out/source-1560.delta.patch"
cp "$clean_patch" "$out/source-1560.clean-paths.patch"
cp "$additions" "$out/process-allowlist-1560-additions.txt"
cp "$expected_paths" "$out/source-delta-paths.txt"
jq -n \
  --arg old_source "$OLD_SOURCE_PARENT" \
  --arg source "$SOURCE_PARENT" \
  --arg swarm "$SWARM_PARENT" \
  --arg base_j "$BASE_J" \
  --arg base_tree "$BASE_TREE" \
  --arg j3 "$j3" \
  --arg tree "$new_tree" \
  --arg control_base "$CONTROL_BASE" \
  --arg control "$control_commit" \
  --arg preflight_sha "$preflight_sha" \
  --arg resolution_sha "$resolution_sha" \
  '{schema:"ripr.current_source_w7_j3_builder.v2",old_source_parent:$old_source,source_parent:$source,swarm_parent:$swarm,baseline:{join:$base_j,tree:$base_tree},source_delta:{commit:"6cc5d6135593d9fb9a745eb215c5b0f92cbd14d5",paths:5,process_allowlist:"exact J2 bytes plus two reviewed rows"},j3:{commit:$j3,tree:$tree,parents:[$source,$swarm]},control:{base:$control_base,commit:$control,preflight_sha256:$preflight_sha,resolution_sha256:$resolution_sha},verification:"verified",proof:["source-promotion workflow contract","source-promotion focused tests","command-catalog workflow parity","workflow policy","process policy","doc artifacts","spec format and numbering","traceability","precommit"],non_claims:["no merge","no version change","no tag","no publication","no signing","no marketplace mutation","no secret use","no back-sync"]}' \
  > "$out/receipt.json"
printf 'J3=%s\nJ3_TREE=%s\nCONTROL_COMMIT=%s\nPREFLIGHT_SHA256=%s\nRESOLUTION_SHA256=%s\n' \
  "$j3" "$new_tree" "$control_commit" "$preflight_sha" "$resolution_sha" \
  | tee "$out/identities.env"
sha256sum "$out"/* | sort > "$out/SHA256SUMS"
printf 'status=success\nstage=complete\n' > "$out/status.env"
stage=complete
