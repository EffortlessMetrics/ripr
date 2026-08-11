from __future__ import annotations

import json
import re
import subprocess
import tomllib
from collections import OrderedDict
from pathlib import Path

SOURCE = "0b6073f88696ab70832dc7ea6410a0c285dc0f53"
SWARM = "335ae8a119872555e83e1f8dfcd23744d9e2a602"


def git_show(rev: str, path: str) -> str:
    return subprocess.check_output(["git", "show", f"{rev}:{path}"], text=True)


def write(path: str, text: str) -> None:
    Path(path).write_text(text, encoding="utf-8", newline="\n")


# Preserve the current producer specification while restoring the exact-file-
# byte binding required by the source v2 verifier.
spec_path = "docs/specs/RIPR-SPEC-0148-source-promotion-preflight.md"
spec = Path(spec_path).read_text(encoding="utf-8")
exact_bytes = """
Canonical receipt identity is the exact UTF-8 file written by the producer,
including pretty-print whitespace and its trailing LF. The source verifier
hashes those bytes before parsing JSON; a reserialized equivalent is not the
same preflight input. The corresponding exact-J consumer contract is
[RIPR-SPEC-0149](RIPR-SPEC-0149-source-promotion-verifier.md).

""".lstrip()
marker = "## Required Evidence\n"
if "Canonical receipt identity is the exact UTF-8 file" not in spec:
    if spec.count(marker) != 1:
        raise SystemExit("preflight spec insertion marker changed")
    spec = spec.replace(marker, exact_bytes + marker)
write(spec_path, spec)

# Union the spec index by canonical numeric identity. Current swarm rows win for
# shared specs; source-only release specs are retained.
index_path = "docs/specs/README.md"
source_index = git_show(SOURCE, index_path)
swarm_index = git_show(SWARM, index_path)
row_re = re.compile(r"^\| \[RIPR-SPEC-(\d{4})\].*$", re.M)
rows: dict[str, str] = {m.group(1): m.group(0) for m in row_re.finditer(source_index)}
rows.update({m.group(1): m.group(0) for m in row_re.finditer(swarm_index)})
matches = list(row_re.finditer(swarm_index))
if not matches:
    raise SystemExit("swarm spec index has no canonical rows")
prefix = swarm_index[: matches[0].start()]
suffix = swarm_index[matches[-1].end() :]
merged_rows = "\n".join(rows[key] for key in sorted(rows, key=int))
write(index_path, prefix + merged_rows + suffix)

# Union the artifact ledger by artifact id. Current swarm blocks win on shared
# identities; source-only release-control artifacts survive.
ledger_path = "policy/doc-artifacts.toml"
source_ledger = git_show(SOURCE, ledger_path)
swarm_ledger = git_show(SWARM, ledger_path)
block_re = re.compile(r"(?ms)^\[\[artifact\]\]\n.*?(?=^\[\[artifact\]\]\n|\Z)")
id_re = re.compile(r'^id = "([^"]+)"$', re.M)


def blocks(text: str) -> list[tuple[str, str]]:
    result = []
    for match in block_re.finditer(text):
        block = match.group(0).rstrip() + "\n"
        identity = id_re.search(block)
        if not identity:
            raise SystemExit("artifact block lacks id")
        result.append((identity.group(1), block))
    return result


merged: OrderedDict[str, str] = OrderedDict(blocks(source_ledger))
for identity, block in blocks(swarm_ledger):
    merged[identity] = block
first = block_re.search(swarm_ledger)
header = swarm_ledger[: first.start()] if first else 'schema_version = "1.0"\n\n'
write(ledger_path, header.rstrip() + "\n\n" + "\n".join(merged.values()))

# Allowlist files are line ledgers: retain one header and the sorted union of
# exact data rows from both repositories.
for allowlist in ["policy/network_allowlist.txt", "policy/process_allowlist.txt"]:
    source_text = git_show(SOURCE, allowlist)
    swarm_text = git_show(SWARM, allowlist)
    source_lines = source_text.splitlines()
    swarm_lines = swarm_text.splitlines()
    header_lines = []
    for line in swarm_lines:
        if line.startswith("#") or not line.strip():
            header_lines.append(line)
        else:
            break
    data_rows = sorted(
        {
            line
            for line in source_lines + swarm_lines
            if line.strip() and not line.startswith("#")
        }
    )
    write(
        allowlist,
        "\n".join(header_lines).rstrip() + "\n\n" + "\n".join(data_rows) + "\n",
    )

# Route the current preflight producer and the source v2 exact-J verifier
# through one source-promotion command surface.
reports_path = "xtask/src/reports/mod.rs"
reports = Path(reports_path).read_text(encoding="utf-8")
module_anchor = "mod source_promotion;\n"
if "mod source_promotion_verify;" not in reports:
    if reports.count(module_anchor) != 1:
        raise SystemExit("source-promotion module anchor changed")
    reports = reports.replace(module_anchor, module_anchor + "mod source_promotion_verify;\n")
export = "pub(crate) use source_promotion::source_promotion;\n"
wrapper = """pub(crate) fn source_promotion(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("preflight") => source_promotion::source_promotion(args),
        Some("verify") => source_promotion_verify::source_promotion_verify(args),
        _ => Err("usage: cargo xtask source-promotion <preflight|verify> ...".to_string()),
    }
}
"""
if export in reports:
    reports = reports.replace(export, wrapper)
elif "pub(crate) fn source_promotion(args:" not in reports:
    raise SystemExit("source-promotion export anchor changed")
write(reports_path, reports)

# Current swarm dispatch is authoritative for new commands. Restore source-only
# variants only when the merged command enum still owns them.
command = Path("xtask/src/command.rs").read_text(encoding="utf-8")
dispatch_path = "xtask/src/dispatch.rs"
dispatch = Path(dispatch_path).read_text(encoding="utf-8")
source_only_arms = OrderedDict(
    [
        ("PrBody", "        XtaskCommand::PrBody(args) => super::pr_body(&args),\n"),
        ("Closeout", "        XtaskCommand::Closeout(args) => super::closeout(&args),\n"),
        ("Goals", "        XtaskCommand::Goals(args) => super::goals(&args),\n"),
        ("CheckCampaign", "        XtaskCommand::CheckCampaign => super::check_campaign(),\n"),
    ]
)
missing = []
for variant, arm in source_only_arms.items():
    token = f"XtaskCommand::{variant}"
    if token in command and token not in dispatch:
        missing.append(arm)
if missing:
    anchor = "        XtaskCommand::Help(args) => print_help(&args),\n"
    if dispatch.count(anchor) != 1:
        raise SystemExit("dispatch help anchor changed")
    dispatch = dispatch.replace(anchor, "".join(missing) + anchor)
write(dispatch_path, dispatch)

# The interim sync is metadata-neutral. Swarm may reorganize version
# inheritance, but every governed effective value remains source-equal.
source_crate = tomllib.loads(git_show(SOURCE, "crates/ripr/Cargo.toml"))
source_version = source_crate["package"]["version"]
root = tomllib.loads(Path("Cargo.toml").read_text(encoding="utf-8"))
crate = tomllib.loads(Path("crates/ripr/Cargo.toml").read_text(encoding="utf-8"))
candidate_version = crate["package"].get("version")
if isinstance(candidate_version, dict):
    if candidate_version != {"workspace": True}:
        raise SystemExit(f"unsupported inherited crate version shape: {candidate_version}")
    candidate_version = root["workspace"]["package"]["version"]
if candidate_version != source_version:
    raise SystemExit(
        f"crate version drift: source={source_version} candidate={candidate_version}"
    )
source_package = json.loads(git_show(SOURCE, "editors/vscode/package.json"))
candidate_package = json.loads(Path("editors/vscode/package.json").read_text())
if candidate_package["version"] != source_package["version"]:
    raise SystemExit("VS Code package version drift")
candidate_npm_lock = json.loads(Path("editors/vscode/package-lock.json").read_text())
expected_extension = source_package["version"]
if candidate_npm_lock["version"] != expected_extension:
    raise SystemExit("npm lock root version drift")
if candidate_npm_lock["packages"][""]["version"] != expected_extension:
    raise SystemExit("npm packages[''] version drift")
lock = tomllib.loads(Path("Cargo.lock").read_text(encoding="utf-8"))
ripr_versions = [
    package["version"]
    for package in lock["package"]
    if package.get("name") == "ripr"
]
if ripr_versions != [source_version]:
    raise SystemExit(f"Cargo.lock ripr version drift: {ripr_versions}")
