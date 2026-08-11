from __future__ import annotations

import re
import subprocess
from pathlib import Path

SOURCE = "0b6073f88696ab70832dc7ea6410a0c285dc0f53"
PATH = Path("policy/doc-artifacts.toml")
BLOCK_RE = re.compile(r"(?ms)^\[\[artifact\]\]\n.*?(?=^\[\[artifact\]\]\n|\Z)")
ID_RE = re.compile(r'^id = "([^"]+)"$', re.M)


def source_ledger() -> str:
    return subprocess.check_output(
        ["git", "show", f"{SOURCE}:{PATH.as_posix()}"], text=True
    )


def blocks(text: str) -> dict[str, str]:
    result: dict[str, str] = {}
    for match in BLOCK_RE.finditer(text):
        block = match.group(0).rstrip() + "\n"
        identity = ID_RE.search(block)
        if identity is None:
            raise SystemExit("document-artifact block lacks id")
        if identity.group(1) in result:
            raise SystemExit(f"document-artifact ledger duplicates {identity.group(1)}")
        result[identity.group(1)] = block
    return result


text = PATH.read_text(encoding="utf-8")
current = blocks(text)
for identity in ("RIPR-SPEC-0151", "RIPR-SPEC-0152"):
    if identity in current:
        raise SystemExit(f"migrated artifact identity already exists: {identity}")

subprocess_block = '''[[artifact]]
id = "RIPR-SPEC-0151"
kind = "spec"
path = "docs/specs/RIPR-SPEC-0151-bounded-subprocess-adapter-boundary.md"
status = "accepted"
owner = "analysis-swarm"
standalone_reason = "Accepted bounded subprocess-classification contract migrated from source RIPR-SPEC-0112 because current swarm independently owns that canonical ID for working-tree disclosure."
'''

source = blocks(source_ledger())
old_verifier = source.get("RIPR-SPEC-0149")
if old_verifier is None:
    raise SystemExit("source ledger lacks the exact-J verifier artifact")
verifier_block = old_verifier.replace("RIPR-SPEC-0149", "RIPR-SPEC-0152").replace(
    "RIPR-SPEC-0149-source-promotion-verifier.md",
    "RIPR-SPEC-0152-source-promotion-verifier.md",
)
if 'id = "RIPR-SPEC-0152"' not in verifier_block:
    raise SystemExit("exact-J verifier artifact migration failed")

for relative in (
    "docs/specs/RIPR-SPEC-0151-bounded-subprocess-adapter-boundary.md",
    "docs/specs/RIPR-SPEC-0152-source-promotion-verifier.md",
):
    if not Path(relative).is_file():
        raise SystemExit(f"migrated artifact path is missing: {relative}")

PATH.write_text(
    text.rstrip() + "\n\n" + subprocess_block.rstrip() + "\n\n" + verifier_block.rstrip() + "\n",
    encoding="utf-8",
    newline="\n",
)

updated = blocks(PATH.read_text(encoding="utf-8"))
for identity in ("RIPR-SPEC-0151", "RIPR-SPEC-0152"):
    if identity not in updated:
        raise SystemExit(f"migrated artifact identity was not registered: {identity}")
