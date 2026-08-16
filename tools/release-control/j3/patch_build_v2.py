#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path


def replace_exact(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")
    return text.replace(old, new, 1)


def main() -> int:
    if len(sys.argv) != 3:
        raise SystemExit("usage: patch_build_v2.py <input-build.sh> <output-build.sh>")
    source = Path(sys.argv[1])
    target = Path(sys.argv[2])
    text = source.read_text(encoding="utf-8")

    old_loop = r'''while IFS= read -r path; do
  test -n "$path" || continue
  work="$RUNNER_TEMP/merge-${path//\//_}"
  mkdir -p "$work"
  cp "$j3_dir/$path" "$work/current"
  git -C "$source_repo" show "$OLD_SOURCE_PARENT:$path" > "$work/base"
  git -C "$source_repo" show "$SOURCE_PARENT:$path" > "$work/source"
  set +e
  git merge-file -p "$work/current" "$work/base" "$work/source" > "$work/merged"
  status=$?
  set -e
  if test "$status" -ne 0; then
    echo "three-way source repair integration failed for $path with status $status" >&2
    cat "$work/merged" >&2 || true
    exit 1
  fi
  cp "$work/merged" "$j3_dir/$path"
done < "$RUNNER_TEMP/j3-changed-paths.txt"
'''
    new_loop = r'''while IFS= read -r path; do
  test -n "$path" || continue
  work="$RUNNER_TEMP/merge-${path//\//_}"
  mkdir -p "$work"
  cp "$j3_dir/$path" "$work/current"
  git -C "$source_repo" show "$OLD_SOURCE_PARENT:$path" > "$work/base"
  git -C "$source_repo" show "$SOURCE_PARENT:$path" > "$work/source"
  if test "$path" = "policy/process_allowlist.txt"; then
    continue
  fi
  set +e
  git merge-file -p "$work/current" "$work/base" "$work/source" > "$work/merged"
  status=$?
  set -e
  if test "$status" -ne 0; then
    echo "three-way source repair integration failed for $path with status $status" >&2
    cat "$work/merged" >&2 || true
    exit 1
  fi
  cp "$work/merged" "$j3_dir/$path"
done < "$RUNNER_TEMP/j3-changed-paths.txt"

policy_work="$RUNNER_TEMP/merge-policy_process_allowlist.txt"
python "$root/tools/release-control/j3/merge_process_policy.py" \
  "$policy_work/current" \
  "$policy_work/base" \
  "$policy_work/source" \
  "$j3_dir" \
  "$j3_dir/policy/process_allowlist.txt" \
  "$RUNNER_TEMP/j3-process-policy-resolution.json"
'''
    text = replace_exact(text, old_loop, new_loop, "J3 source repair loop")

    old_push = r'''git -C "$source_repo" push origin "$j3:refs/heads/$J3_BRANCH"
git -C "$control_dir" push origin "$control_commit:refs/heads/$CONTROL_BRANCH"
'''
    new_push = r'''git -C "$root" fetch "$source_repo" "$j3"
git -C "$root" fetch "$control_dir" "$control_commit"
test "$(git -C "$root" rev-parse "$j3^{commit}")" = "$j3"
test "$(git -C "$root" rev-parse "$control_commit^{commit}")" = "$control_commit"
git -C "$root" push origin "$j3:refs/heads/$J3_BRANCH"
git -C "$root" push origin "$control_commit:refs/heads/$CONTROL_BRANCH"
'''
    text = replace_exact(text, old_push, new_push, "authenticated J3 publication")

    old_packet = 'cp "$RUNNER_TEMP/j3-changed-paths.json" "$packet/changed-paths.json"\n'
    new_packet = (
        old_packet
        + 'cp "$RUNNER_TEMP/j3-process-policy-resolution.json" '
        + '"$packet/process-policy-resolution.json"\n'
    )
    text = replace_exact(text, old_packet, new_packet, "process-policy receipt retention")
    text = text.replace(
        '"reviewed five-path three-way source repair integration"',
        '"reviewed four-path three-way source repair integration plus exact process-policy union"',
    )
    target.write_text(text, encoding="utf-8")
    target.chmod(0o755)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
