#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import os
import stat
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any


@dataclass(frozen=True)
class Entry:
    mode: str
    kind: str
    oid: str


def run(repo: Path, *args: str, check: bool = True, input_bytes: bytes | None = None) -> subprocess.CompletedProcess[bytes]:
    proc = subprocess.run(
        ["git", "-C", str(repo), *args],
        input=input_bytes,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if check and proc.returncode != 0:
        raise RuntimeError(
            f"git {' '.join(args)} failed ({proc.returncode}): "
            f"{proc.stderr.decode('utf-8', 'replace').strip()}"
        )
    return proc


def entry_at(repo: Path, commit: str, path: str) -> Entry | None:
    proc = run(repo, "ls-tree", commit, "--", path, check=False)
    if proc.returncode != 0 or not proc.stdout:
        return None
    line = proc.stdout.decode("utf-8", "strict").rstrip("\n")
    meta, listed = line.split("\t", 1)
    if listed != path:
        raise RuntimeError(f"ls-tree returned unexpected path {listed!r} for {path!r}")
    mode, kind, oid = meta.split(" ", 2)
    return Entry(mode, kind, oid)


def blob(repo: Path, oid: str) -> bytes:
    return run(repo, "cat-file", "blob", oid).stdout


def write_entry(worktree: Path, path: str, entry: Entry | None, content: bytes | None = None) -> None:
    target = worktree / path
    if entry is None:
        if target.is_symlink() or target.is_file():
            target.unlink()
        elif target.is_dir():
            raise RuntimeError(f"refusing to remove directory while reconciling file {path}")
        return
    if entry.kind != "blob" or entry.mode not in {"100644", "100755", "120000"}:
        raise RuntimeError(f"unsupported tree entry for {path}: {entry}")
    target.parent.mkdir(parents=True, exist_ok=True)
    data = content if content is not None else blob(REPO, entry.oid)
    if target.exists() or target.is_symlink():
        target.unlink()
    if entry.mode == "120000":
        os.symlink(data.decode("utf-8", "strict"), target)
        return
    target.write_bytes(data)
    target.chmod(0o755 if entry.mode == "100755" else 0o644)


def hash_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def changed_paths(repo: Path, old_source: str, new_source: str) -> list[str]:
    proc = run(repo, "diff", "--name-only", "-z", old_source, new_source)
    values = [part.decode("utf-8", "strict") for part in proc.stdout.split(b"\0") if part]
    if len(values) != len(set(values)):
        raise RuntimeError("source delta contains duplicate path names")
    return sorted(values)


def merge_regular_files(path: str, base: bytes, ours: bytes, theirs: bytes) -> bytes:
    if any(b"\0" in value for value in (base, ours, theirs)):
        raise RuntimeError(f"binary both-changed source delta requires review: {path}")
    with tempfile.TemporaryDirectory(prefix="ripr-j4-merge-") as raw:
        temp = Path(raw)
        ours_path = temp / "j2"
        base_path = temp / "old-source"
        theirs_path = temp / "live-source"
        ours_path.write_bytes(ours)
        base_path.write_bytes(base)
        theirs_path.write_bytes(theirs)
        proc = subprocess.run(
            [
                "git",
                "merge-file",
                "-p",
                "-L",
                "reviewed J2 tree",
                "-L",
                "old source parent",
                "-L",
                "live source parent",
                str(ours_path),
                str(base_path),
                str(theirs_path),
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        if proc.returncode != 0:
            raise RuntimeError(
                f"three-way source integration conflict for {path} ({proc.returncode}):\n"
                + proc.stdout.decode("utf-8", "replace")
            )
        return proc.stdout


def choose_mode(path: str, base: Entry, ours: Entry, theirs: Entry) -> str:
    if ours.mode == base.mode:
        return theirs.mode
    if theirs.mode == base.mode or ours.mode == theirs.mode:
        return ours.mode
    raise RuntimeError(
        f"both source and J2 changed file mode incompatibly for {path}: "
        f"base={base.mode} j2={ours.mode} source={theirs.mode}"
    )


def reconcile_path(repo: Path, worktree: Path, old_source: str, new_source: str, j2: str, path: str) -> dict[str, Any]:
    base = entry_at(repo, old_source, path)
    ours = entry_at(repo, j2, path)
    theirs = entry_at(repo, new_source, path)
    record: dict[str, Any] = {
        "path": path,
        "old_source": None if base is None else {"mode": base.mode, "oid": base.oid},
        "j2": None if ours is None else {"mode": ours.mode, "oid": ours.oid},
        "live_source": None if theirs is None else {"mode": theirs.mode, "oid": theirs.oid},
    }

    if ours == base:
        write_entry(worktree, path, theirs)
        record["resolution"] = "live_source"
        return record
    if theirs == base:
        record["resolution"] = "reviewed_j2"
        return record
    if ours == theirs:
        record["resolution"] = "identical"
        return record
    if path == "policy/process_allowlist.txt":
        record["resolution"] = "deferred_process_policy_union"
        return record
    if base is None or ours is None or theirs is None:
        raise RuntimeError(
            f"delete/add conflict requires explicit review for {path}: "
            f"old_source={base} j2={ours} live_source={theirs}"
        )
    if any(entry.kind != "blob" or entry.mode == "120000" for entry in (base, ours, theirs)):
        raise RuntimeError(f"non-regular both-changed path requires explicit review: {path}")
    merged = merge_regular_files(path, blob(repo, base.oid), blob(repo, ours.oid), blob(repo, theirs.oid))
    mode = choose_mode(path, base, ours, theirs)
    synthetic = Entry(mode, "blob", ours.oid)
    write_entry(worktree, path, synthetic, merged)
    record["resolution"] = "integrated_three_way"
    record["merged_sha256"] = hash_bytes(merged)
    record["mode"] = mode
    return record


def main() -> int:
    if len(sys.argv) != 8:
        raise SystemExit(
            "usage: reconcile_tree.py <repo> <worktree> <old-source> <live-source> "
            "<j2> <receipt-json> <changed-paths-json>"
        )
    global REPO
    repo_raw, worktree_raw, old_source, live_source, j2, receipt_raw, changed_raw = sys.argv[1:]
    REPO = Path(repo_raw).resolve()
    worktree = Path(worktree_raw).resolve()
    receipt_path = Path(receipt_raw)
    changed_path = Path(changed_raw)

    paths = changed_paths(REPO, old_source, live_source)
    records: list[dict[str, Any]] = []
    for path in paths:
        records.append(reconcile_path(REPO, worktree, old_source, live_source, j2, path))

    changed_path.parent.mkdir(parents=True, exist_ok=True)
    changed_path.write_text(json.dumps(paths, indent=2) + "\n", encoding="utf-8")
    receipt = {
        "schema": "ripr.source_promotion_source_delta_resolution.v1",
        "old_source_parent": old_source,
        "live_source_parent": live_source,
        "reviewed_baseline_join": j2,
        "changed_path_count": len(paths),
        "changed_paths": paths,
        "resolutions": records,
        "invariants": [
            "source-only movement is copied exactly",
            "J2-only movement is retained exactly",
            "both-changed regular text files require conflict-free three-way integration",
            "binary, delete/add, symlink, and incompatible mode conflicts fail closed",
            "process policy is reconciled separately against the completed reviewed tree",
        ],
    }
    receipt_path.parent.mkdir(parents=True, exist_ok=True)
    receipt_path.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
