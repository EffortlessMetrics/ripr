from __future__ import annotations

import subprocess
from collections import OrderedDict
from pathlib import Path

SOURCE = "0b6073f88696ab70832dc7ea6410a0c285dc0f53"
SWARM = "335ae8a119872555e83e1f8dfcd23744d9e2a602"

path = Path("xtask/src/command.rs")
text = path.read_text(encoding="utf-8")
old = '''        command_entry(
            "source-promotion preflight --source-parent <sha> --swarm-parent <sha> --swarm-ref <immutable-ref> --source-repo <path> --swarm-repo <path> --version <version> [--resolved-tree <full-tree-sha>] [--swarm-main <rev>] [--source-main <rev>] [--out <dir>]",
            "external_state_read",
            "<out>/source-promotion-preflight.{json,md}",
            false,
            false,
            "Validates exact source/swarm parents, repository identity and reachability, deterministic ancestry counts/digests, and a disposable merge-tree conflict inventory; it never mutates either repository, constructs a join, changes versions, or publishes.",
        ),
'''
new = '''        command_entry(
            "source-promotion <preflight|verify> ...",
            "argument_dependent",
            "preflight or exact-J verification receipts under explicit --out or target/ripr/source-promotion",
            false,
            false,
            "Runs the read-only exact-pair preflight producer or the v2 exact history-preserving join verifier. Neither subcommand constructs a join, mutates Git refs, changes versions, or publishes.",
        ),
'''
count = text.count(old)
if count != 1:
    raise SystemExit(f"expected one swarm source-promotion catalog entry, found {count}")
text = text.replace(old, new)
path.write_text(text, encoding="utf-8", newline="\n")

# The raw allowlist union treats different ceilings for one (path, pattern) as
# different rows. Rebuild process authority by semantic key: source supplies
# source-only verifier/probe entries, current swarm owns shared product/test
# surfaces, and observed merged counts tighten the exact-J verifier.
def git_show(rev: str, relative: str) -> str:
    return subprocess.check_output(["git", "show", f"{rev}:{relative}"], text=True)


def parse_process_policy(text: str) -> tuple[str, OrderedDict[tuple[str, str], str]]:
    lines = text.splitlines()
    first_data = next(
        (index for index, line in enumerate(lines) if line.strip() and not line.startswith("#")),
        len(lines),
    )
    header = "\n".join(lines[:first_data]).rstrip()
    rows: OrderedDict[tuple[str, str], str] = OrderedDict()
    for line in lines[first_data:]:
        if not line.strip() or line.startswith("#"):
            continue
        fields = line.split("|", 4)
        if len(fields) != 5:
            raise SystemExit(f"malformed process-policy row: {line}")
        rows[(fields[0], fields[1])] = line
    return header, rows


policy_relative = "policy/process_allowlist.txt"
_, source_rows = parse_process_policy(git_show(SOURCE, policy_relative))
header, swarm_rows = parse_process_policy(git_show(SWARM, policy_relative))
rows = OrderedDict(source_rows)
for key, row in swarm_rows.items():
    rows[key] = row

# Current swarm no longer imports Command directly in this smoke-test file.
rows.pop(("crates/ripr/tests/cli_smoke.rs", "use std::process::Command"), None)

# The combined exact-J verifier has four Command::new call sites. Keep the
# source-owned reason and tighten the ceiling rather than accepting stale debt.
verifier_key = ("xtask/src/reports/source_promotion_verify.rs", "Command::new")
verifier_row = rows.get(verifier_key)
if verifier_row is None:
    raise SystemExit("source exact-J verifier process allowance is missing")
verifier_fields = verifier_row.split("|", 4)
verifier_fields[2] = "4"
rows[verifier_key] = "|".join(verifier_fields)

policy_path = Path(policy_relative)
policy_path.write_text(
    header + "\n\n" + "\n".join(rows.values()) + "\n",
    encoding="utf-8",
    newline="\n",
)
