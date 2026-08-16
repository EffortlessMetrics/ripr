#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import os
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any


CATALOG_TEST_PATH = "xtask/src/tests.rs"
CATALOG_TEST_J2_BLOB = "1bff24fb3fb2f27f4db4b8a9f4ffe6c5f217a45b"
CATALOG_TEST_REPLACEMENTS = [
    (
        "fn command_catalog_pins_ci_enforced_classification() -> Result<(), String> {",
        "fn command_catalog_ci_enforced_flags_match_repo_workflows_pins_classification() -> Result<(), String>\n{",
    ),
    (
        '    assert!(!ci_enforced("precommit")?);',
        '    assert!(ci_enforced("precommit")?);',
    ),
    (
        '    assert!(!ci_enforced("check-architecture")?);',
        '    assert!(ci_enforced("check-architecture")?);',
    ),
    (
        '    assert!(!ci_enforced("check-doc-artifacts")?);',
        '    assert!(ci_enforced("check-doc-artifacts")?);',
    ),
    (
        '    assert!(!ci_enforced("markdown-links")?);',
        '    assert!(ci_enforced("markdown-links")?);',
    ),
]


@dataclass(frozen=True)
class Entry:
    mode: str
    kind: str
    oid: str


def git(repo: Path, *args: str, check: bool = True) -> subprocess.CompletedProcess[bytes]:
    proc = subprocess.run(
        ["git", "-C", str(repo), *args],
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
    proc = git(repo, "ls-tree", commit, "--", path, check=False)
    if proc.returncode != 0 or not proc.stdout:
        return None
    meta, listed = proc.stdout.decode("utf-8", "strict").rstrip("\n").split("\t", 1)
    if listed != path:
        raise RuntimeError(f"ls-tree returned {listed!r} for {path!r}")
    mode, kind, oid = meta.split(" ", 2)
    return Entry(mode, kind, oid)


def blob(repo: Path, oid: str) -> bytes:
    return git(repo, "cat-file", "blob", oid).stdout


def blob_oid(data: bytes) -> str:
    header = f"blob {len(data)}\0".encode("ascii")
    return hashlib.sha1(header + data).hexdigest()


def write_entry(
    repo: Path,
    worktree: Path,
    path: str,
    entry: Entry | None,
    data: bytes | None = None,
) -> None:
    target = worktree / path
    if entry is None:
        if target.is_symlink() or target.is_file():
            target.unlink()
        elif target.is_dir():
            raise RuntimeError(f"refusing directory removal for file delta {path}")
        return
    if entry.kind != "blob" or entry.mode not in {"100644", "100755", "120000"}:
        raise RuntimeError(f"unsupported tree entry for {path}: {entry}")
    content = data if data is not None else blob(repo, entry.oid)
    target.parent.mkdir(parents=True, exist_ok=True)
    if target.exists() or target.is_symlink():
        if target.is_dir() and not target.is_symlink():
            raise RuntimeError(f"refusing to replace directory with file {path}")
        target.unlink()
    if entry.mode == "120000":
        os.symlink(content.decode("utf-8", "strict"), target)
    else:
        target.write_bytes(content)
        target.chmod(0o755 if entry.mode == "100755" else 0o644)


def merge_text(path: str, base: bytes, ours: bytes, theirs: bytes) -> bytes:
    if any(b"\0" in value for value in (base, ours, theirs)):
        raise RuntimeError(f"binary both-changed path requires review: {path}")
    with tempfile.TemporaryDirectory(prefix="ripr-j5-merge-") as raw:
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
                "reviewed J2",
                "-L",
                "old source",
                "-L",
                "live source",
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
        f"incompatible both-changed mode for {path}: base={base.mode} j2={ours.mode} source={theirs.mode}"
    )


def integrate_current_ci_catalog_test(
    repo: Path,
    worktree: Path,
    j2: str,
) -> dict[str, Any]:
    entry = entry_at(repo, j2, CATALOG_TEST_PATH)
    if entry is None or entry.kind != "blob" or entry.mode != "100644":
        raise RuntimeError(
            f"reviewed J2 must contain a regular {CATALOG_TEST_PATH}: {entry}"
        )
    if entry.oid != CATALOG_TEST_J2_BLOB:
        raise RuntimeError(
            f"reviewed J2 {CATALOG_TEST_PATH} blob moved: "
            f"expected={CATALOG_TEST_J2_BLOB} actual={entry.oid}"
        )

    target = worktree / CATALOG_TEST_PATH
    before = target.read_bytes()
    if blob_oid(before) != CATALOG_TEST_J2_BLOB:
        raise RuntimeError(
            f"source reconciliation unexpectedly changed {CATALOG_TEST_PATH} before the "
            "reviewed CI-classification repair"
        )
    text = before.decode("utf-8", "strict")
    applied: list[dict[str, str]] = []
    for old, new in CATALOG_TEST_REPLACEMENTS:
        count = text.count(old)
        if count != 1:
            raise RuntimeError(
                f"expected exactly one reviewed catalog-test preimage {old!r}, found {count}"
            )
        text = text.replace(old, new, 1)
        applied.append({"old": old, "new": new})

    for retained in [
        '    assert!(!ci_enforced("release-readiness --version <version>")?);',
        '    assert!(!ci_enforced("goldens bless <name> --reason <reason>")?);',
        '    assert!(!ci_enforced("check-supply-chain")?);',
    ]:
        if text.count(retained) != 1:
            raise RuntimeError(
                f"catalog-test repair lost retained advisory assertion {retained!r}"
            )

    after = text.encode("utf-8")
    target.write_bytes(after)
    target.chmod(0o644)
    return {
        "path": CATALOG_TEST_PATH,
        "resolution": "integrated_current_ci_catalog_truth",
        "j2_blob": CATALOG_TEST_J2_BLOB,
        "result_blob": blob_oid(after),
        "applied": applied,
        "retained_advisory_assertions": [
            "release-readiness --version <version>",
            "goldens bless <name> --reason <reason>",
            "check-supply-chain",
        ],
        "verification_filter": "command_catalog_ci_enforced_flags_match_repo_workflows",
        "reason": (
            "Current routed workflows and the integrated command catalog enforce precommit, "
            "check-architecture, check-doc-artifacts, and markdown-links. The inherited J2 "
            "test still pinned the superseded source topology."
        ),
    }


def main() -> int:
    if len(sys.argv) != 8:
        raise SystemExit(
            "usage: reconcile_tree.py <repo> <worktree> <old-source> <live-source> "
            "<j2> <receipt-json> <changed-paths-json>"
        )
    repo = Path(sys.argv[1]).resolve()
    worktree = Path(sys.argv[2]).resolve()
    old_source, live_source, j2 = sys.argv[3:6]
    receipt_path = Path(sys.argv[6])
    changed_path = Path(sys.argv[7])
    raw = git(repo, "diff", "--name-only", "-z", old_source, live_source).stdout
    paths = sorted(part.decode("utf-8", "strict") for part in raw.split(b"\0") if part)
    if len(paths) != len(set(paths)):
        raise RuntimeError("source delta contains duplicate paths")

    records: list[dict[str, Any]] = []
    for path in paths:
        base = entry_at(repo, old_source, path)
        ours = entry_at(repo, j2, path)
        theirs = entry_at(repo, live_source, path)
        record: dict[str, Any] = {
            "path": path,
            "old_source": None if base is None else {"mode": base.mode, "oid": base.oid},
            "j2": None if ours is None else {"mode": ours.mode, "oid": ours.oid},
            "live_source": None
            if theirs is None
            else {"mode": theirs.mode, "oid": theirs.oid},
        }
        if ours == base:
            write_entry(repo, worktree, path, theirs)
            record["resolution"] = "live_source"
        elif theirs == base:
            record["resolution"] = "reviewed_j2"
        elif ours == theirs:
            record["resolution"] = "identical"
        elif path == "policy/process_allowlist.txt":
            record["resolution"] = "deferred_process_policy_union"
        elif base is None or ours is None or theirs is None:
            raise RuntimeError(
                f"delete/add conflict requires review for {path}: old={base} j2={ours} source={theirs}"
            )
        elif any(
            entry.kind != "blob" or entry.mode == "120000"
            for entry in (base, ours, theirs)
        ):
            raise RuntimeError(f"non-regular both-changed path requires review: {path}")
        else:
            merged = merge_text(
                path,
                blob(repo, base.oid),
                blob(repo, ours.oid),
                blob(repo, theirs.oid),
            )
            mode = choose_mode(path, base, ours, theirs)
            write_entry(repo, worktree, path, Entry(mode, "blob", ours.oid), merged)
            record["resolution"] = "integrated_three_way"
            record["merged_sha256"] = hashlib.sha256(merged).hexdigest()
            record["mode"] = mode
        records.append(record)

    catalog_test_repair = integrate_current_ci_catalog_test(repo, worktree, j2)

    changed_path.parent.mkdir(parents=True, exist_ok=True)
    changed_path.write_text(json.dumps(paths, indent=2) + "\n", encoding="utf-8")
    receipt_path.parent.mkdir(parents=True, exist_ok=True)
    receipt_path.write_text(
        json.dumps(
            {
                "schema": "ripr.source_promotion_source_delta_resolution.v2",
                "old_source_parent": old_source,
                "live_source_parent": live_source,
                "reviewed_baseline_join": j2,
                "changed_path_count": len(paths),
                "changed_paths": paths,
                "resolutions": records,
                "reviewed_integration_repairs": [catalog_test_repair],
                "invariants": [
                    "source-only movement is copied exactly",
                    "J2-only movement is retained exactly",
                    "both-changed regular text requires conflict-free three-way integration",
                    "binary, delete/add, symlink, and incompatible mode conflicts fail closed",
                    "process policy is reconciled separately against the completed reviewed tree",
                    "the inherited catalog classification test is patched only from its exact reviewed J2 blob",
                    "the focused workflow-drift test filter also executes the repaired classification pin",
                ],
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
