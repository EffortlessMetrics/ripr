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
    data_started = False
    for number, raw in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        stripped = raw.strip()
        if not stripped or stripped.startswith("#"):
            if not data_started:
                header.append(raw)
            continue
        data_started = True
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


def changed_from_base(row: Row | None, base: Row | None) -> bool:
    if row is None:
        return base is not None
    if base is None:
        return True
    return row.render() != base.render()


def select_owner(base: Row | None, j2: Row | None, source: Row | None, actual: int) -> tuple[Row, str]:
    j2_changed = changed_from_base(j2, base)
    source_changed = changed_from_base(source, base)
    if j2 is not None and source is not None:
        if source_changed and not j2_changed:
            return source, "live_source_changed"
        if j2_changed and not source_changed:
            return j2, "reviewed_j2_changed"
        if j2.render() == source.render():
            return j2, "identical"
        candidates = [row for row in (j2, source) if row.maximum >= actual]
        if candidates:
            selected = min(candidates, key=lambda row: (row.maximum, 0 if row.origin == "j2" else 1))
            return selected, "integrated_both_changed_covered"
        return max((j2, source), key=lambda row: row.maximum), "integrated_both_changed_widened"
    if source is not None:
        return source, "live_source_only"
    if j2 is not None:
        return j2, "reviewed_j2_only"
    raise AssertionError("select_owner called without a candidate")


def main() -> int:
    if len(sys.argv) != 7:
        raise SystemExit(
            "usage: reconcile_process_policy.py <old-source-policy> <j2-policy> "
            "<live-source-policy> <reviewed-tree-root> <output-policy> <receipt-json>"
        )
    old_raw, j2_raw, source_raw, root_raw, output_raw, receipt_raw = map(Path, sys.argv[1:])
    header, old_rows = parse(old_raw, "old_source")
    _, j2_rows = parse(j2_raw, "j2")
    _, source_rows = parse(source_raw, "live_source")
    root = root_raw.resolve()

    keys = sorted(set(old_rows) | set(j2_rows) | set(source_rows))
    result: list[Row] = []
    decisions: list[dict[str, Any]] = []
    dropped: list[dict[str, Any]] = []
    for key in keys:
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
                    "reason": "literal absent from reviewed J4 tree",
                }
            )
            continue
        selected, decision = select_owner(base, j2, source, actual)
        row = Row(selected.path, selected.pattern, actual, selected.owner, selected.reason, selected.origin)
        result.append(row)
        decisions.append(
            {
                "path": row.path,
                "pattern": row.pattern,
                "actual_count": actual,
                "selected_origin": selected.origin,
                "decision": decision,
                "old_source_maximum": None if base is None else base.maximum,
                "j2_maximum": None if j2 is None else j2.maximum,
                "live_source_maximum": None if source is None else source.maximum,
                "result_maximum": actual,
            }
        )

    output_raw.parent.mkdir(parents=True, exist_ok=True)
    header_text = "\n".join(header).rstrip()
    body = "\n".join(row.render() for row in sorted(result, key=lambda row: row.key))
    output_raw.write_text(header_text + "\n\n" + body + "\n", encoding="utf-8")

    receipt = {
        "schema": "ripr.source_promotion_process_policy_resolution.v2",
        "inputs": {
            "old_source_sha256": sha256(old_raw),
            "j2_sha256": sha256(j2_raw),
            "live_source_sha256": sha256(source_raw),
        },
        "result_sha256": sha256(output_raw),
        "result_rows": len(result),
        "decisions": decisions,
        "dropped_orphaned": dropped,
        "invariants": [
            "policy ownership is reconciled by path and literal pattern",
            "source-only and J2-only live patterns survive",
            "both-changed rows prefer the narrowest existing maximum covering the reviewed tree",
            "every retained maximum equals the exact reviewed-tree occurrence count",
            "format-sensitive grouped jq imports use the stable prefix use std::process::{",
            "orphaned literals are removed",
            "check-process-policy remains the final detector for uncovered process surfaces",
        ],
    }
    receipt_raw.parent.mkdir(parents=True, exist_ok=True)
    receipt_raw.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
