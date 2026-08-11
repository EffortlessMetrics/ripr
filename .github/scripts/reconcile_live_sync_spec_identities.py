from __future__ import annotations

import re
import subprocess
from collections import OrderedDict
from pathlib import Path

SOURCE = "0b6073f88696ab70832dc7ea6410a0c285dc0f53"
SWARM = "335ae8a119872555e83e1f8dfcd23744d9e2a602"
INDEX = Path("docs/specs/README.md")
ROW_RE = re.compile(r"(?m)^\|\s*\[RIPR-SPEC-(\d{4})\]\([^\n]+\)\s*\|.*\|$")
BLOCK_RE = re.compile(r"(?ms)^\[\[(?:behavior|artifact)\]\]\n.*?(?=^\[\[(?:behavior|artifact)\]\]\n|\Z)")

MIGRATIONS = (
    (
        "RIPR-SPEC-0112",
        "RIPR-SPEC-0151",
        "docs/specs/RIPR-SPEC-0112-bounded-subprocess-adapter-boundary.md",
        "docs/specs/RIPR-SPEC-0151-bounded-subprocess-adapter-boundary.md",
    ),
    (
        "RIPR-SPEC-0149",
        "RIPR-SPEC-0152",
        "docs/specs/RIPR-SPEC-0149-source-promotion-verifier.md",
        "docs/specs/RIPR-SPEC-0152-source-promotion-verifier.md",
    ),
)


def git_show(rev: str, path: str) -> str:
    return subprocess.check_output(["git", "show", f"{rev}:{path}"], text=True)


def write(path: Path, text: str) -> None:
    path.write_text(text, encoding="utf-8", newline="\n")


def migrate_block_file(path: Path) -> None:
    text = path.read_text(encoding="utf-8")
    blocks = list(BLOCK_RE.finditer(text))
    if not blocks:
        return
    prefix = text[: blocks[0].start()]
    rendered: list[str] = []
    for match in blocks:
        block = match.group(0)
        for old_id, new_id, old_path, new_path in MIGRATIONS:
            if f'path = "{old_path}"' in block or f'spec = "{old_path}"' in block:
                block = block.replace(old_id, new_id).replace(old_path, new_path)
        rendered.append(block.rstrip() + "\n")
    write(path, prefix.rstrip() + "\n\n" + "\n".join(rendered))


# Rename the two source-only contracts into unused IDs. Current swarm retains
# its canonical 0112 and 0149 identities.
for old_id, new_id, old_path, new_path in MIGRATIONS:
    old = Path(old_path)
    new = Path(new_path)
    if not old.is_file():
        raise SystemExit(f"source-only spec is missing before migration: {old_path}")
    if new.exists():
        raise SystemExit(f"target spec identity already exists: {new_path}")
    old.rename(new)
    content = new.read_text(encoding="utf-8")
    content = content.replace(old_id, new_id).replace(old_path, new_path)
    write(new, content)

# Rebuild the index directly from both exact parents. Source rows are migrated
# before the union; current swarm wins only genuinely shared identities.
source_index = git_show(SOURCE, str(INDEX))
swarm_index = git_show(SWARM, str(INDEX))
source_rows: dict[str, str] = {}
for match in ROW_RE.finditer(source_index):
    identity = match.group(1)
    row = match.group(0)
    for old_id, new_id, old_path, new_path in MIGRATIONS:
        old_name = Path(old_path).name
        new_name = Path(new_path).name
        if old_name in row:
            row = row.replace(old_id, new_id).replace(old_name, new_name)
            identity = new_id.rsplit("-", 1)[1]
    source_rows[identity] = row
swarm_rows = {match.group(1): match.group(0) for match in ROW_RE.finditer(swarm_index)}
rows = dict(source_rows)
rows.update(swarm_rows)
source_matches = list(ROW_RE.finditer(source_index))
swarm_matches = list(ROW_RE.finditer(swarm_index))
if not source_matches or not swarm_matches:
    raise SystemExit("one exact parent has no canonical spec-index rows")
prefix = swarm_index[: swarm_matches[0].start()]
suffix = swarm_index[swarm_matches[-1].end() :]
write(INDEX, prefix + "\n".join(rows[key] for key in sorted(rows, key=int)) + suffix)
for required in ("0112", "0149", "0151", "0152"):
    if required not in rows:
        raise SystemExit(f"merged spec index is missing RIPR-SPEC-{required}")

# Migrate active source-owned ledger blocks while leaving current swarm blocks
# at their original identities.
migrate_block_file(Path(".ripr/traceability.toml"))
migrate_block_file(Path("policy/doc-artifacts.toml"))

# Update source-verifier documentation and fixture links. CHANGELOG.md is
# intentionally excluded: it is historical release copy and remains byte-equal
# to SOURCE_PARENT.
for relative in (
    "docs/specs/RIPR-SPEC-0148-source-promotion-preflight.md",
    "docs/DOCUMENTATION.md",
    "fixtures/source_promotion_verification/SPEC.md",
):
    path = Path(relative)
    if not path.is_file():
        raise SystemExit(f"expected source verifier reference surface missing: {relative}")
    text = path.read_text(encoding="utf-8")
    text = text.replace(
        "RIPR-SPEC-0149-source-promotion-verifier.md",
        "RIPR-SPEC-0152-source-promotion-verifier.md",
    ).replace("RIPR-SPEC-0149", "RIPR-SPEC-0152")
    write(path, text)

# Policy rows are keyed by the implementation path, which distinguishes the
# source contracts from the unrelated current swarm specs sharing the old IDs.
policy_rewrites = {
    Path("policy/network_allowlist.txt"): (
        ("crates/ripr/src/analysis/probes/subprocess.rs|", "RIPR-SPEC-0112", "RIPR-SPEC-0151"),
    ),
    Path("policy/process_allowlist.txt"): (
        ("crates/ripr/src/analysis/probes/subprocess.rs|", "RIPR-SPEC-0112", "RIPR-SPEC-0151"),
        ("xtask/src/reports/source_promotion_verify.rs|", "RIPR-SPEC-0149", "RIPR-SPEC-0152"),
    ),
}
for path, rewrites in policy_rewrites.items():
    lines = path.read_text(encoding="utf-8").splitlines()
    updated: list[str] = []
    for line in lines:
        for prefix_value, old_id, new_id in rewrites:
            if line.startswith(prefix_value):
                line = line.replace(old_id, new_id)
        updated.append(line)
    write(path, "\n".join(updated) + "\n")
