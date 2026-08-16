#!/usr/bin/env python3
from __future__ import annotations

import copy
import hashlib
import json
import subprocess
import sys
from pathlib import Path
from typing import Any

KIND_ORDER = {"conflict": 0, "source_survivor": 1, "swarm_exclusion": 2}


def git(repo: Path, *args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    proc = subprocess.run(
        ["git", "-C", str(repo), *args],
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if check and proc.returncode != 0:
        raise RuntimeError(f"git {' '.join(args)} failed: {proc.stderr.strip()}")
    return proc


def object_at(repo: Path, commit: str, path: str) -> str | None:
    probe = git(repo, "rev-parse", "--verify", f"{commit}:{path}", check=False)
    return probe.stdout.strip() if probe.returncode == 0 else None


def disposition_for(repo: Path, join: str, source: str, swarm: str, path: str) -> str:
    final_object = object_at(repo, join, path)
    source_object = object_at(repo, source, path)
    swarm_object = object_at(repo, swarm, path)
    if final_object is None:
        return "excluded"
    if final_object == source_object:
        return "source_blob"
    if final_object == swarm_object:
        return "swarm_blob"
    return "integrated"


def expected_inventory(preflight: dict[str, Any]) -> list[tuple[str, str]]:
    dry_merge = preflight.get("dry_merge")
    if not isinstance(dry_merge, dict):
        raise SystemExit("preflight dry_merge is malformed")
    inventories = [
        ("conflict", dry_merge.get("conflicts")),
        ("source_survivor", preflight.get("source_survivor_candidates")),
        ("swarm_exclusion", preflight.get("swarm_authority_resolution_candidates")),
    ]
    expected: list[tuple[str, str]] = []
    for kind, values in inventories:
        if not isinstance(values, list) or not all(isinstance(value, str) for value in values):
            raise SystemExit(f"preflight {kind} inventory is malformed")
        expected.extend((kind, value) for value in values)
    if len(expected) != len(set(expected)):
        raise SystemExit("fresh preflight contains duplicate kind:key inventory rows")
    return sorted(expected, key=lambda item: (KIND_ORDER[item[0]], item[1]))


def rationale(kind: str, disposition: str, path: str, changed: set[str]) -> str:
    if kind == "swarm_exclusion":
        return "Swarm-only repository authority remains excluded from the promoted source tree."
    if disposition == "source_blob":
        return "Live source-authoritative surface retained exactly from the frozen source parent."
    if disposition == "swarm_blob":
        return "Frozen W7 product surface retained exactly."
    if disposition == "excluded":
        return "Fresh exact-pair resolution excludes this path from the promoted tree."
    if path == "policy/process_allowlist.txt":
        return (
            "Integrated policy union preserves live J2/W7 ownership and live-source ownership, "
            "drops absent literals, tightens counts to the reviewed J4 tree, and is checked by "
            "the repository process-policy gate."
        )
    if path in changed:
        return (
            "Conflict-free three-way integration carries live source movement onto the previously "
            "reviewed J2 product tree without changing frozen W7."
        )
    return "Previously reviewed integrated J2 surface retained in the fresh exact-pair tree."


def main() -> int:
    if len(sys.argv) != 11:
        raise SystemExit(
            "usage: make_manifest.py <preflight> <old-manifest> <out-manifest> <delta-out> "
            "<repo> <source> <swarm> <join> <tree> <changed-paths-json>"
        )
    (
        preflight_path,
        old_manifest_path,
        out_path,
        delta_path,
        repo_path,
        source,
        swarm,
        join,
        tree,
        changed_path,
    ) = sys.argv[1:]
    preflight_bytes = Path(preflight_path).read_bytes()
    preflight = json.loads(preflight_bytes)
    old_manifest = json.loads(Path(old_manifest_path).read_text(encoding="utf-8"))
    changed_values = json.loads(Path(changed_path).read_text(encoding="utf-8"))
    if not isinstance(changed_values, list) or not all(isinstance(value, str) for value in changed_values):
        raise SystemExit("changed paths JSON is malformed")
    changed = set(changed_values)
    old_rows = {
        (row.get("kind"), row.get("key")): row
        for row in old_manifest.get("dispositions", [])
        if isinstance(row, dict)
    }
    expected = expected_inventory(preflight)
    expected_set = set(expected)
    old_set = set(old_rows)
    repo = Path(repo_path)
    rows: list[dict[str, Any]] = []
    classifications: dict[str, str] = {}
    for kind, key in expected:
        previous = copy.deepcopy(old_rows.get((kind, key), {}))
        disposition = (
            previous.get("disposition", "excluded")
            if kind == "swarm_exclusion"
            else disposition_for(repo, join, source, swarm, key)
        )
        if disposition not in {"source_blob", "swarm_blob", "integrated", "excluded"}:
            disposition = "excluded" if kind == "swarm_exclusion" else "integrated"
        row = {
            "kind": kind,
            "key": key,
            "disposition": disposition,
            "rationale": rationale(kind, disposition, key, changed),
            "evidence": (
                f"SOURCE_PARENT {source}; SWARM_PARENT {swarm}; "
                f"JOIN_TREE {tree}; exact J4 {join}"
            ),
        }
        rows.append(row)
        classifications[f"{kind}:{key}"] = disposition

    digest = "sha256:" + hashlib.sha256(preflight_bytes).hexdigest()
    merge_base = preflight.get("merge_base")
    if not isinstance(merge_base, str):
        raise SystemExit("preflight merge_base is malformed")
    manifest = {
        "schema": "ripr.source_promotion_resolution.v1",
        "preflight_sha256": digest,
        "source_parent": source,
        "swarm_parent": swarm,
        "merge_base": merge_base,
        "reviewed_join_tree": tree,
        "dispositions": rows,
    }
    Path(out_path).write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    delta = {
        "schema": "ripr.source_promotion_resolution_refresh.v2",
        "old_rows": len(old_set),
        "fresh_rows": len(expected_set),
        "retained_kind_keys": sorted(f"{kind}:{key}" for kind, key in old_set & expected_set),
        "added_kind_keys": sorted(f"{kind}:{key}" for kind, key in expected_set - old_set),
        "removed_kind_keys": sorted(f"{kind}:{key}" for kind, key in old_set - expected_set),
        "changed_source_paths": sorted(changed),
        "classifications": classifications,
    }
    Path(delta_path).write_text(json.dumps(delta, indent=2) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
