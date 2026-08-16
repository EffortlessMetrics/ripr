#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable


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
        return (self.path, self.pattern)

    def render(self) -> str:
        return f"{self.path}|{self.pattern}|{self.maximum}|{self.owner}|{self.reason}"


def parse(path: Path, origin: str) -> tuple[list[str], dict[tuple[str, str], Row]]:
    header: list[str] = []
    rows: dict[tuple[str, str], Row] = {}
    seen_data = False
    for lineno, raw in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        line = raw.strip()
        if not line or line.startswith("#"):
            if not seen_data:
                header.append(raw)
            continue
        seen_data = True
        parts = raw.split("|", 4)
        if len(parts) != 5:
            raise SystemExit(f"{path}:{lineno}: malformed process-policy row")
        row_path, pattern, maximum, owner, reason = parts
        try:
            maximum_value = int(maximum)
        except ValueError as error:
            raise SystemExit(f"{path}:{lineno}: invalid maximum {maximum!r}") from error
        if maximum_value < 0:
            raise SystemExit(f"{path}:{lineno}: negative maximum")
        if (
            row_path == "xtask/tests/source_promotion_workflow_contract.rs"
            and pattern == "use std::process::{Command, Stdio}"
        ):
            pattern = "use std::process::{"
        row = Row(row_path, pattern, maximum_value, owner, reason, origin)
        if row.key in rows:
            raise SystemExit(f"{path}:{lineno}: duplicate process-policy key {row.key!r}")
        rows[row.key] = row
    return header, rows


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def actual_count(root: Path, row_path: str, pattern: str) -> int:
    target = root / row_path
    if not target.is_file():
        return 0
    return target.read_text(encoding="utf-8", errors="replace").count(pattern)


def select_row(current: Row | None, source: Row | None, actual: int) -> Row:
    if current is not None and actual <= current.maximum:
        return current
    if source is not None and actual <= source.maximum:
        return source
    candidates = [row for row in (current, source) if row is not None]
    if not candidates:
        raise AssertionError("select_row called without a candidate")
    return max(candidates, key=lambda row: row.maximum)


def render_keys(keys: Iterable[tuple[str, str]]) -> list[str]:
    return [f"{path}|{pattern}" for path, pattern in sorted(keys)]


def main() -> int:
    if len(sys.argv) != 7:
        raise SystemExit(
            "usage: merge_process_policy.py <j2-policy> <old-source-policy> "
            "<new-source-policy> <j3-root> <output-policy> <receipt-json>"
        )
    current_path, old_source_path, source_path, root_path, output_path, receipt_path = map(
        Path, sys.argv[1:]
    )
    header, current_rows = parse(current_path, "j2")
    _, old_source_rows = parse(old_source_path, "old_source")
    _, source_rows = parse(source_path, "source")
    root = root_path.resolve()

    removed_by_source = set(old_source_rows) - set(source_rows)
    added_by_source = set(source_rows) - set(old_source_rows)
    changed_by_source = {
        key
        for key in set(old_source_rows) & set(source_rows)
        if old_source_rows[key].render() != source_rows[key].render()
    }
    expected_source_additions = {
        (
            "xtask/tests/source_promotion_workflow_contract.rs",
            "Command::new",
        ),
        (
            "xtask/tests/source_promotion_workflow_contract.rs",
            "use std::process::{",
        ),
    }
    if removed_by_source or changed_by_source or added_by_source != expected_source_additions:
        raise SystemExit(
            "#1560 process-policy movement is not the reviewed two-row additive delta: "
            + json.dumps(
                {
                    "added": render_keys(added_by_source),
                    "removed": render_keys(removed_by_source),
                    "changed": render_keys(changed_by_source),
                },
                sort_keys=True,
            )
        )

    merged_rows: list[Row] = []
    dropped: list[dict[str, object]] = []
    decisions: list[dict[str, object]] = []
    widened: list[dict[str, object]] = []
    all_keys = sorted(set(current_rows) | set(source_rows))
    for key in all_keys:
        current = current_rows.get(key)
        source = source_rows.get(key)
        count = actual_count(root, key[0], key[1])
        if count == 0:
            dropped.append(
                {
                    "path": key[0],
                    "pattern": key[1],
                    "origins": [row.origin for row in (current, source) if row is not None],
                    "reason": "pattern absent from reviewed J3 tree",
                }
            )
            continue
        selected = select_row(current, source, count)
        maximum = max(selected.maximum, count)
        merged = Row(
            selected.path,
            selected.pattern,
            maximum,
            selected.owner,
            selected.reason,
            selected.origin,
        )
        merged_rows.append(merged)
        if maximum != selected.maximum:
            widened.append(
                {
                    "path": selected.path,
                    "pattern": selected.pattern,
                    "from": selected.maximum,
                    "to": maximum,
                    "actual": count,
                }
            )
        decisions.append(
            {
                "path": selected.path,
                "pattern": selected.pattern,
                "actual": count,
                "maximum": maximum,
                "selected_origin": selected.origin,
                "j2_maximum": None if current is None else current.maximum,
                "source_maximum": None if source is None else source.maximum,
            }
        )

    output_path.parent.mkdir(parents=True, exist_ok=True)
    body = "\n".join(row.render() for row in sorted(merged_rows, key=lambda row: row.key))
    header_text = "\n".join(header).rstrip()
    output_path.write_text(header_text + "\n\n" + body + "\n", encoding="utf-8")

    merged_keys = {row.key for row in merged_rows}
    receipt = {
        "schema": "ripr.source_promotion_process_policy_resolution.v1",
        "inputs": {
            "j2_sha256": sha256(current_path),
            "old_source_sha256": sha256(old_source_path),
            "new_source_sha256": sha256(source_path),
        },
        "source_delta": {
            "added": render_keys(added_by_source),
            "removed": [],
            "changed": [],
        },
        "result_sha256": sha256(output_path),
        "result_rows": len(merged_rows),
        "added_against_j2": render_keys(merged_keys - set(current_rows)),
        "retained_from_j2": render_keys(merged_keys & set(current_rows)),
        "dropped_orphaned": dropped,
        "widened": widened,
        "decisions": decisions,
        "invariants": [
            "#1560 source movement is exactly two additive policy rows",
            "row equality ignores provenance labels and compares rendered policy bytes",
            "J2 rows are preferred whenever they cover the reviewed J3 occurrence count",
            "source rows restore surviving source-owned process surfaces absent from J2 policy",
            "the grouped jq-test import is normalized to the formatting-stable prefix use std::process::{",
            "rows whose literal pattern is absent from the reviewed J3 tree are removed",
            "every retained maximum covers the exact reviewed-tree occurrence count",
            "cargo xtask check-process-policy remains the final detector for uncovered process surfaces",
        ],
    }
    receipt_path.parent.mkdir(parents=True, exist_ok=True)
    receipt_path.write_text(json.dumps(receipt, indent=2) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
