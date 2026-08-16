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
        raise SystemExit("usage: patch_build.py <input-build.sh> <output-build.sh>")
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
    python - "$work/current" "$work/base" "$work/source" "$work/merged" <<'PY'
from collections import Counter
from pathlib import Path
import sys

current_path, base_path, source_path, output_path = map(Path, sys.argv[1:])
current = current_path.read_text(encoding="utf-8").splitlines(keepends=True)
base = base_path.read_text(encoding="utf-8").splitlines(keepends=True)
source = source_path.read_text(encoding="utf-8").splitlines(keepends=True)
base_counts = Counter(base)
source_counts = Counter(source)
removed = base_counts - source_counts
if removed:
    raise SystemExit(
        "process allowlist source movement is not additive: "
        + repr(dict(removed))
    )
added = source_counts - base_counts
if sum(added.values()) != 2:
    raise SystemExit(
        f"expected exactly two additive process-policy rows from #1560, found {sum(added.values())}"
    )
merged = list(current)
merged_counts = Counter(merged)
remaining = added.copy()
for line in source:
    if remaining[line] <= 0:
        continue
    remaining[line] -= 1
    if merged_counts[line] < source_counts[line]:
        if merged and not merged[-1].endswith("\n"):
            merged[-1] += "\n"
        merged.append(line if line.endswith("\n") else line + "\n")
        merged_counts[line] += 1
if any(remaining.values()):
    raise SystemExit("failed to consume additive process-policy rows")
output_path.write_text("".join(merged), encoding="utf-8")
PY
    status=0
  else
    set +e
    git merge-file -p "$work/current" "$work/base" "$work/source" > "$work/merged"
    status=$?
    set -e
  fi
  if test "$status" -ne 0; then
    echo "three-way source repair integration failed for $path with status $status" >&2
    cat "$work/merged" >&2 || true
    exit 1
  fi
  cp "$work/merged" "$j3_dir/$path"
done < "$RUNNER_TEMP/j3-changed-paths.txt"
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
    text = text.replace(
        '"reviewed five-path three-way source repair integration"',
        '"reviewed five-path source repair integration with additive process-policy reconciliation"',
    )
    target.write_text(text, encoding="utf-8")
    target.chmod(0o755)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
