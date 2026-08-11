from __future__ import annotations

import json
import re
import subprocess
import tomllib
from collections import OrderedDict
from pathlib import Path
from typing import Any

SOURCE = "0b6073f88696ab70832dc7ea6410a0c285dc0f53"
SWARM = "335ae8a119872555e83e1f8dfcd23744d9e2a602"
PATH = ".ripr/traceability.toml"
BLOCK_RE = re.compile(r"(?ms)^\[\[behavior\]\]\n.*?(?=^\[\[behavior\]\]\n|\Z)")
ID_RE = re.compile(r'^id = "([^"]+)"$', re.M)
FIELD_ORDER = ["id", "name", "spec", "tests", "fixtures", "code", "outputs", "metrics"]


def git_show(rev: str) -> str:
    return subprocess.check_output(["git", "show", f"{rev}:{PATH}"], text=True)


def blocks(text: str) -> list[tuple[str, str]]:
    result: list[tuple[str, str]] = []
    for match in BLOCK_RE.finditer(text):
        block = match.group(0).rstrip() + "\n"
        identity = ID_RE.search(block)
        if identity is None:
            raise SystemExit("traceability behavior block lacks id")
        result.append((identity.group(1), block))
    return result


def block_map(text: str) -> dict[str, str]:
    result: dict[str, str] = {}
    for identity, block in blocks(text):
        if identity in result:
            raise SystemExit(f"parent traceability duplicates {identity}")
        result[identity] = block
    return result


def parse_behavior(block: str) -> dict[str, Any]:
    parsed = tomllib.loads(block)
    behaviors = parsed.get("behavior")
    if not isinstance(behaviors, list) or len(behaviors) != 1:
        raise SystemExit("traceability block did not parse as one behavior")
    behavior = behaviors[0]
    if not isinstance(behavior, dict):
        raise SystemExit("traceability behavior is not a table")
    return behavior


def ordered_union(primary: list[str], secondary: list[str]) -> list[str]:
    return list(OrderedDict.fromkeys([*primary, *secondary]))


def merge_behavior(identity: str, source_block: str, swarm_block: str) -> dict[str, Any]:
    source = parse_behavior(source_block)
    swarm = parse_behavior(swarm_block)
    primary, secondary = (source, swarm) if identity == "RIPR-SPEC-0149" else (swarm, source)
    merged: dict[str, Any] = {}
    for key in OrderedDict.fromkeys([*primary.keys(), *secondary.keys()]):
        primary_value = primary.get(key)
        secondary_value = secondary.get(key)
        if isinstance(primary_value, list) or isinstance(secondary_value, list):
            if not isinstance(primary_value, list) or not isinstance(secondary_value, list):
                raise SystemExit(f"traceability field shape differs for {identity}.{key}")
            if not all(isinstance(item, str) for item in [*primary_value, *secondary_value]):
                raise SystemExit(f"traceability list is not string-only for {identity}.{key}")
            merged[key] = ordered_union(primary_value, secondary_value)
        elif primary_value is not None:
            merged[key] = primary_value
        else:
            merged[key] = secondary_value
    if merged.get("id") != identity:
        raise SystemExit(f"merged traceability identity drift for {identity}")
    return merged


def quote(value: str) -> str:
    return json.dumps(value, ensure_ascii=False)


def render_behavior(behavior: dict[str, Any]) -> str:
    lines = ["[[behavior]]"]
    keys = [key for key in FIELD_ORDER if key in behavior]
    keys.extend(key for key in behavior if key not in keys)
    for key in keys:
        value = behavior[key]
        if isinstance(value, str):
            lines.append(f"{key} = {quote(value)}")
        elif isinstance(value, list):
            lines.append(f"{key} = [")
            lines.extend(f"  {quote(item)}," for item in value)
            lines.append("]")
        elif isinstance(value, bool):
            lines.append(f"{key} = {'true' if value else 'false'}")
        elif isinstance(value, int):
            lines.append(f"{key} = {value}")
        else:
            raise SystemExit(f"unsupported traceability value for {behavior.get('id')}.{key}")
    return "\n".join(lines) + "\n"


working = Path(PATH).read_text(encoding="utf-8")
working_blocks = blocks(working)
counts: dict[str, int] = {}
for identity, _ in working_blocks:
    counts[identity] = counts.get(identity, 0) + 1
duplicates = {identity for identity, count in counts.items() if count > 1}
expected = {"RIPR-SPEC-0112", "RIPR-SPEC-0148", "RIPR-SPEC-0149"}
if duplicates != expected:
    raise SystemExit(
        f"unexpected duplicate traceability identities: expected={sorted(expected)} actual={sorted(duplicates)}"
    )

source_blocks = block_map(git_show(SOURCE))
swarm_blocks = block_map(git_show(SWARM))
for identity in duplicates:
    if identity not in source_blocks or identity not in swarm_blocks:
        raise SystemExit(f"duplicate {identity} is not present in both exact parents")

first = BLOCK_RE.search(working)
if first is None:
    raise SystemExit("working traceability manifest has no behavior blocks")
header = working[: first.start()].rstrip()
seen: set[str] = set()
resolved_blocks: list[str] = []
for identity, block in working_blocks:
    if identity in seen:
        continue
    seen.add(identity)
    if identity in duplicates:
        block = render_behavior(
            merge_behavior(identity, source_blocks[identity], swarm_blocks[identity])
        )
    resolved_blocks.append(block.rstrip() + "\n")

Path(PATH).write_text(
    header + "\n\n" + "\n".join(resolved_blocks),
    encoding="utf-8",
    newline="\n",
)
