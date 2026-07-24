#!/usr/bin/env python3
"""Produce a read-only, exact-parent source-promotion preflight receipt."""

from __future__ import annotations

import json
import os
import subprocess
import tempfile
from pathlib import Path
from typing import Any, Iterable

ROOT = Path(__file__).resolve().parents[1]
INPUT_PATH = ROOT / "docs/release/0.11.0/source-promotion-preflight-inputs.json"
OUTPUT_DIR = ROOT / "target/source-promotion-preflight"
SWARM_URL = "https://github.com/EffortlessMetrics/ripr-swarm.git"


def run(
    args: list[str],
    *,
    cwd: Path = ROOT,
    check: bool = True,
) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        args,
        cwd=cwd,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if check and result.returncode != 0:
        command = " ".join(args)
        raise RuntimeError(
            f"command failed ({result.returncode}): {command}\n"
            f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
        )
    return result


def git(*args: str, cwd: Path = ROOT, check: bool = True) -> subprocess.CompletedProcess[str]:
    return run(["git", *args], cwd=cwd, check=check)


def lines(result: subprocess.CompletedProcess[str]) -> list[str]:
    return sorted({line.strip() for line in result.stdout.splitlines() if line.strip()})


def git_show_text(revision: str, path: str) -> str | None:
    result = git("show", f"{revision}:{path}", check=False)
    return result.stdout if result.returncode == 0 else None


def cargo_version(revision: str) -> str | None:
    text = git_show_text(revision, "crates/ripr/Cargo.toml")
    if text is None:
        return None
    in_package = False
    for raw_line in text.splitlines():
        line = raw_line.strip()
        if line.startswith("["):
            in_package = line == "[package]"
            continue
        if in_package and line.startswith("version") and "=" in line:
            return line.split("=", 1)[1].strip().strip('"')
    return None


def extension_version(revision: str) -> str | None:
    text = git_show_text(revision, "editors/vscode/package.json")
    if text is None:
        return None
    return str(json.loads(text).get("version"))


def exists_at(revision: str, path: str) -> bool:
    return git("cat-file", "-e", f"{revision}:{path}", check=False).returncode == 0


def write_json(path: Path, value: Any) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def bullet(items: Iterable[str]) -> str:
    values = list(items)
    return "\n".join(f"- `{item}`" for item in values) if values else "- none"


def main() -> int:
    inputs = json.loads(INPUT_PATH.read_text(encoding="utf-8"))
    source_parent = inputs["source_parent"]
    swarm_candidate = inputs["swarm_candidate"]
    expected_merge_base = inputs["expected_merge_base"]
    previous_swarm_promotion = inputs["previous_swarm_promotion"]

    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    if git("remote", "get-url", "swarm", check=False).returncode == 0:
        git("remote", "set-url", "swarm", SWARM_URL)
    else:
        git("remote", "add", "swarm", SWARM_URL)

    git("fetch", "--no-tags", "origin", source_parent)
    git("fetch", "--no-tags", "swarm", swarm_candidate)
    git("fetch", "--no-tags", "swarm", "+main:refs/remotes/swarm/main")

    git("cat-file", "-e", f"{source_parent}^{{commit}}")
    git("cat-file", "-e", f"{swarm_candidate}^{{commit}}")

    merge_base = git("merge-base", source_parent, swarm_candidate).stdout.strip()
    if merge_base != expected_merge_base:
        raise RuntimeError(
            f"merge-base mismatch: expected {expected_merge_base}, observed {merge_base}"
        )

    candidate_ancestor = (
        git(
            "merge-base",
            "--is-ancestor",
            swarm_candidate,
            "refs/remotes/swarm/main",
            check=False,
        ).returncode
        == 0
    )
    if not candidate_ancestor:
        raise RuntimeError("frozen swarm candidate is not an ancestor of swarm/main")

    source_changed = lines(git("diff", "--name-only", merge_base, source_parent))
    swarm_changed = lines(git("diff", "--name-only", merge_base, swarm_candidate))
    overlap = sorted(set(source_changed) & set(swarm_changed))
    source_only = sorted(set(source_changed) - set(swarm_changed))
    swarm_only_count = len(set(swarm_changed) - set(source_changed))

    source_tree = git("rev-parse", f"{source_parent}^{{tree}}").stdout.strip()
    swarm_tree = git("rev-parse", f"{swarm_candidate}^{{tree}}").stdout.strip()
    source_commits = int(git("rev-list", "--count", f"{merge_base}..{source_parent}").stdout)
    swarm_commits = int(git("rev-list", "--count", f"{merge_base}..{swarm_candidate}").stdout)
    included_since_previous = int(
        git("rev-list", "--count", f"{previous_swarm_promotion}..{swarm_candidate}").stdout
    )

    survivor_paths = [
        "crates/ripr/src/analysis/probes/subprocess.rs",
        "docs/specs/RIPR-SPEC-0112-bounded-subprocess-adapter-boundary.md",
        "docs/SOURCE_PROMOTION.md",
        ".github/settings.yml",
        "docs/release/0.11.0/post-freeze-source-survivors.json",
    ]
    source_survivors = {
        path: exists_at(source_parent, path) for path in survivor_paths
    }
    if not all(source_survivors.values()):
        missing = [path for path, present in source_survivors.items() if not present]
        raise RuntimeError(f"source survivor paths missing before merge: {missing}")

    source_versions = {
        "crate": cargo_version(source_parent),
        "extension": extension_version(source_parent),
    }
    swarm_versions = {
        "crate": cargo_version(swarm_candidate),
        "extension": extension_version(swarm_candidate),
    }
    if source_versions != {"crate": "0.10.1", "extension": "0.10.1"}:
        raise RuntimeError(f"unexpected source pre-metadata versions: {source_versions}")

    merge_stdout = ""
    merge_stderr = ""
    merge_returncode = 99
    conflict_files: list[str] = []
    merge_status = "not_attempted"
    prospective_tree: str | None = None

    with tempfile.TemporaryDirectory(prefix="ripr-source-preflight-") as temp_dir:
        worktree = Path(temp_dir) / "worktree"
        git("worktree", "add", "--detach", str(worktree), source_parent)
        try:
            merge = git(
                "merge",
                "--no-ff",
                "--no-commit",
                swarm_candidate,
                cwd=worktree,
                check=False,
            )
            merge_returncode = merge.returncode
            merge_stdout = merge.stdout
            merge_stderr = merge.stderr
            conflict_files = lines(
                git("diff", "--name-only", "--diff-filter=U", cwd=worktree, check=False)
            )
            if merge.returncode == 0:
                merge_status = "clean"
                prospective_tree = git("write-tree", cwd=worktree).stdout.strip()
            elif conflict_files:
                merge_status = "conflicts"
            else:
                merge_status = "error"
        finally:
            git("merge", "--abort", cwd=worktree, check=False)
            git("reset", "--hard", source_parent, cwd=worktree, check=False)
            git("worktree", "remove", "--force", str(worktree), check=False)

    if merge_status == "error":
        raise RuntimeError(
            "dry merge failed without conflict entries\n"
            f"stdout:\n{merge_stdout}\nstderr:\n{merge_stderr}"
        )

    receipt = {
        "schema_version": "1.0",
        "kind": "ripr_source_promotion_preflight",
        "status": "conflicts_require_review" if conflict_files else "clean_merge_requires_semantic_review",
        "release_line": inputs["release_line"],
        "source_parent": source_parent,
        "source_tree": source_tree,
        "swarm_candidate": swarm_candidate,
        "swarm_tree": swarm_tree,
        "swarm_freeze_ref": inputs["swarm_freeze_ref"],
        "merge_base": merge_base,
        "expected_merge_base_matched": True,
        "candidate_is_ancestor_of_swarm_main": candidate_ancestor,
        "counts": {
            "source_commits_since_merge_base": source_commits,
            "swarm_commits_since_merge_base": swarm_commits,
            "swarm_commits_since_previous_promotion": included_since_previous,
            "source_changed_paths": len(source_changed),
            "swarm_changed_paths": len(swarm_changed),
            "overlap_paths": len(overlap),
            "source_only_paths": len(source_only),
            "swarm_only_paths": swarm_only_count,
            "textual_conflicts": len(conflict_files),
        },
        "merge_attempt": {
            "command": f"git merge --no-ff --no-commit {swarm_candidate}",
            "returncode": merge_returncode,
            "status": merge_status,
            "prospective_tree_if_clean": prospective_tree,
            "conflict_files": conflict_files,
            "stdout": merge_stdout.strip(),
            "stderr": merge_stderr.strip(),
        },
        "overlap_paths": overlap,
        "source_only_paths": source_only,
        "source_versions": source_versions,
        "swarm_versions": swarm_versions,
        "source_survivor_presence": source_survivors,
        "required_resolution_policy": {
            "source_first_parent": True,
            "preserve_source_versions_until_metadata_pr": True,
            "preserve_source_release_and_repository_authority": True,
            "use_swarm_analyzer_editor_dependency_structure": True,
            "exclude_swarm_repository_settings_and_publish_authority": True,
            "no_appended_repair_commit": True,
        },
        "audit_record_set_sha256": inputs["audit_record_set_sha256"],
        "authority_boundary": inputs["authority_boundary"],
    }
    write_json(OUTPUT_DIR / "preflight.json", receipt)

    markdown = f"""# RIPR 0.11.0 source-promotion preflight

```text
source parent     {source_parent}
swarm candidate   {swarm_candidate}
merge base        {merge_base}
source tree       {source_tree}
swarm tree        {swarm_tree}
dry merge status  {merge_status}
text conflicts    {len(conflict_files)}
overlap paths     {len(overlap)}
```

## Textual conflicts

{bullet(conflict_files)}

## All paths changed on both sides

{bullet(overlap)}

## Source-only changed paths

{bullet(source_only)}

## Version boundary

- source crate: `{source_versions['crate']}`
- source extension: `{source_versions['extension']}`
- swarm crate: `{swarm_versions['crate']}`
- swarm extension: `{swarm_versions['extension']}`

The promotion join must retain source `0.10.1` metadata. The `0.11.0` bump remains a separate PR.

## Source survivor presence

{bullet(path for path, present in source_survivors.items() if present)}

## Claim boundary

{inputs['authority_boundary']}
"""
    (OUTPUT_DIR / "preflight.md").write_text(markdown, encoding="utf-8")

    summary_path = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary_path:
        with Path(summary_path).open("a", encoding="utf-8") as summary:
            summary.write(markdown)

    print(json.dumps(receipt, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
