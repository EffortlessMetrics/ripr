#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any


@dataclass(frozen=True)
class Row:
    path: str
    pattern: str
    maximum: int
    owner: str
    reason: str
    origin: str

    @property
    def key(self) -> tuple[str, str]:
        return self.path, self.pattern

    def render(self) -> str:
        return f"{self.path}|{self.pattern}|{self.maximum}|{self.owner}|{self.reason}"


def normalize_pattern(path: str, pattern: str) -> str:
    if path == "xtask/tests/source_promotion_workflow_contract.rs" and pattern == "use std::process::{Command, Stdio}":
        return "use std::process::{"
    return pattern


def parse(path: Path, origin: str) -> tuple[list[str], dict[tuple[str, str], Row]]:
    header: list[str] = []
    rows: dict[tuple[str, str], Row] = {}
    started = False
    for number, raw in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        stripped = raw.strip()
        if not stripped or stripped.startswith("#"):
            if not started:
                header.append(raw)
            continue
        started = True
        parts = raw.split("|", 4)
        if len(parts) != 5:
            raise SystemExit(f"{path}:{number}: malformed process-policy row")
        row_path, pattern, maximum, owner, reason = parts
        pattern = normalize_pattern(row_path, pattern)
        try:
            maximum_value = int(maximum)
        except ValueError as error:
            raise SystemExit(f"{path}:{number}: invalid maximum {maximum!r}") from error
        if maximum_value < 0:
            raise SystemExit(f"{path}:{number}: negative maximum")
        row = Row(row_path, pattern, maximum_value, owner, reason, origin)
        if row.key in rows:
            raise SystemExit(f"{path}:{number}: duplicate process-policy key {row.key!r}")
        rows[row.key] = row
    return header, rows


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def count_literal(root: Path, row: Row) -> int:
    target = root / row.path
    if not target.is_file():
        return 0
    return target.read_text(encoding="utf-8", errors="replace").count(row.pattern)


def changed(row: Row | None, base: Row | None) -> bool:
    if row is None:
        return base is not None
    if base is None:
        return True
    return row.render() != base.render()


def select(base: Row | None, j2: Row | None, source: Row | None, actual: int) -> tuple[Row, str]:
    covering = [row for row in (j2, source) if row is not None and row.maximum >= actual]
    if not covering:
        candidates = [row for row in (j2, source) if row is not None]
        maxima = {row.origin: row.maximum for row in candidates}
        probe = candidates[0] if candidates else base
        raise SystemExit(
            f"reviewed J5 count exceeds all declared maxima for "
            f"{probe.path if probe else '<unknown>'}|{probe.pattern if probe else '<unknown>'}: "
            f"actual={actual} maxima={maxima}"
        )
    if j2 is not None and source is not None:
        j2_changed = changed(j2, base)
        source_changed = changed(source, base)
        if source_changed and not j2_changed and source.maximum >= actual:
            return source, "live_source_changed"
        if j2_changed and not source_changed and j2.maximum >= actual:
            return j2, "reviewed_j2_changed"
        if j2.render() == source.render():
            return j2, "identical"
        selected = min(covering, key=lambda row: (row.maximum, 0 if row.origin == "j2" else 1))
        return selected, "integrated_both_changed_covered"
    selected = covering[0]
    return selected, "live_source_only" if selected.origin == "live_source" else "reviewed_j2_only"


def main() -> int:
    if len(sys.argv) != 7:
        raise SystemExit(
            "usage: reconcile_process_policy.py <old-source-policy> <j2-policy> "
            "<live-source-policy> <reviewed-tree-root> <output-policy> <receipt-json>"
        )
    old_path, j2_path, source_path, root_path, output_path, receipt_path = map(Path, sys.argv[1:])
    header, old_rows = parse(old_path, "old_source")
    _, j2_rows = parse(j2_path, "j2")
    _, source_rows = parse(source_path, "live_source")
    root = root_path.resolve()

    result: list[Row] = []
    decisions: list[dict[str, Any]] = []
    dropped: list[dict[str, Any]] = []
    for key in sorted(set(old_rows) | set(j2_rows) | set(source_rows)):
        base = old_rows.get(key)
        j2 = j2_rows.get(key)
        source = source_rows.get(key)
        probe = source or j2 or base
        if probe is None:
            continue
        actual = count_literal(root, probe)
        if actual == 0:
            dropped.append(
                {
                    "path": key[0],
                    "pattern": key[1],
                    "origins": [row.origin for row in (base, j2, source) if row is not None],
                    "reason": "literal absent from reviewed J5 tree",
                }
            )
            continue
        selected, decision = select(base, j2, source, actual)
        result.append(selected)
        decisions.append(
            {
                "path": selected.path,
                "pattern": selected.pattern,
                "actual_count": actual,
                "selected_origin": selected.origin,
                "decision": decision,
                "old_source_maximum": None if base is None else base.maximum,
                "j2_maximum": None if j2 is None else j2.maximum,
                "live_source_maximum": None if source is None else source.maximum,
                "result_maximum": selected.maximum,
            }
        )

    output_path.parent.mkdir(parents=True, exist_ok=True)
    header_text = "\n".join(header).rstrip()
    body = "\n".join(row.render() for row in sorted(result, key=lambda row: row.key))
    output_path.write_text(header_text + "\n\n" + body + "\n", encoding="utf-8")
    receipt_path.parent.mkdir(parents=True, exist_ok=True)
    receipt_path.write_text(
        json.dumps(
            {
                "schema": "ripr.source_promotion_process_policy_resolution.v2",
                "inputs": {
                    "old_source_sha256": sha256(old_path),
                    "j2_sha256": sha256(j2_path),
                    "live_source_sha256": sha256(source_path),
                },
                "result_sha256": sha256(output_path),
                "result_rows": len(result),
                "decisions": decisions,
                "dropped_orphaned": dropped,
                "invariants": [
                    "source-only and J2-only live patterns survive",
                    "both-changed rows use an existing maximum covering the reviewed tree",
                    "no maximum is widened implicitly",
                    "grouped jq imports use the formatting-stable prefix use std::process::{",
                    "orphaned literals are removed",
                    "check-process-policy remains the final uncovered-surface detector",
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
