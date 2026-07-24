#!/usr/bin/env python3
"""Build and validate the exact RIPR 0.11.0 source-promotion join."""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import traceback
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
PLAN_PATH = ROOT / "docs/release/0.11.0/source-promotion-resolution-plan.json"
OUTPUT_DIR = ROOT / "target/source-promotion-builder"
SWARM_URL = "https://github.com/EffortlessMetrics/ripr-swarm.git"


def run(
    args: list[str],
    *,
    cwd: Path = ROOT,
    check: bool = True,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        args,
        cwd=cwd,
        check=False,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=env,
    )
    if check and result.returncode != 0:
        raise RuntimeError(
            f"command failed ({result.returncode}): {' '.join(args)}\n"
            f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
        )
    return result


def git(*args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    return run(["git", *args], check=check)


def git_text(revision: str, path: str) -> str:
    result = git("show", f"{revision}:{path}", check=False)
    if result.returncode != 0:
        raise RuntimeError(f"missing {path} at {revision}: {result.stderr.strip()}")
    return result.stdout


def write(path: str, content: str) -> None:
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(content, encoding="utf-8")


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def checkout(side: str, *paths: str) -> None:
    git("checkout", f"--{side}", "--", *paths)


def set_cargo_package_version(path: str, version: str) -> None:
    text = (ROOT / path).read_text(encoding="utf-8")
    package_start = text.find("[package]")
    if package_start < 0:
        raise RuntimeError(f"missing [package] in {path}")
    match = re.search(r'(?m)^version\s*=\s*"[^"]+"', text[package_start:])
    if match is None:
        raise RuntimeError(f"missing package version in {path}")
    start = package_start + match.start()
    end = package_start + match.end()
    text = text[:start] + f'version = "{version}"' + text[end:]
    write(path, text)


def set_cargo_lock_version(version: str) -> None:
    path = "Cargo.lock"
    text = (ROOT / path).read_text(encoding="utf-8")
    pattern = re.compile(r'(\[\[package\]\]\nname = "ripr"\nversion = ")[^"]+("\n)')
    text, count = pattern.subn(rf"\g<1>{version}\g<2>", text, count=1)
    if count != 1:
        raise RuntimeError("could not set ripr version in Cargo.lock")
    write(path, text)


def set_node_versions(version: str) -> None:
    package_path = ROOT / "editors/vscode/package.json"
    package = json.loads(package_path.read_text(encoding="utf-8"))
    package["version"] = version
    package_path.write_text(json.dumps(package, indent=2) + "\n", encoding="utf-8")

    lock_path = ROOT / "editors/vscode/package-lock.json"
    lock = json.loads(lock_path.read_text(encoding="utf-8"))
    lock["version"] = version
    packages = lock.get("packages")
    if not isinstance(packages, dict) or "" not in packages:
        raise RuntimeError("package-lock root package is missing")
    packages[""]["version"] = version
    lock_path.write_text(json.dumps(lock, indent=2) + "\n", encoding="utf-8")


def integrate_bounded_subprocess() -> None:
    path = "crates/ripr/src/analysis/probes/diff.rs"
    text = (ROOT / path).read_text(encoding="utf-8")
    import_line = "use super::subprocess::bounded_subprocess_family;"
    if import_line not in text:
        marker = "use super::lexical::classify_changed_line;\n"
        if marker not in text:
            raise RuntimeError("bounded-subprocess import marker not found")
        text = text.replace(marker, marker + import_line + "\n", 1)

    marker = """        if changed_line_owned_by_test(index, &changed.path, added.new_side_line) {
            continue;
        }
        let parser_shapes =
"""
    bounded = """        if changed_line_owned_by_test(index, &changed.path, added.new_side_line) {
            continue;
        }
        if let Some(family) =
            bounded_subprocess_family(index, &changed.path, added.new_side_line, text)
        {
            probes.push(build_probe(
                &build_context,
                added,
                family,
                nearby_removed_line(added.new_side_line, text, changed),
                Some(text.to_string()),
            ));
            continue;
        }
        let parser_shapes =
"""
    if "bounded_subprocess_family(index" not in text:
        if marker not in text:
            raise RuntimeError("bounded-subprocess insertion marker not found")
        text = text.replace(marker, bounded, 1)
    write(path, text)


def migrate_bounded_subprocess_spec(source_parent: str) -> None:
    old_id = "RIPR-SPEC-0112"
    new_id = "RIPR-SPEC-0144"
    old_path = "docs/specs/RIPR-SPEC-0112-bounded-subprocess-adapter-boundary.md"
    new_path = "docs/specs/RIPR-SPEC-0144-bounded-subprocess-adapter-boundary.md"

    source_spec = git_text(source_parent, old_path).replace(old_id, new_id)
    write(new_path, source_spec)
    git("rm", "--ignore-unmatch", old_path)

    changelog_path = ROOT / "CHANGELOG.md"
    changelog = changelog_path.read_text(encoding="utf-8")
    old_entry = "Bounded subprocess adapter classification** (RIPR-SPEC-0112"
    if old_entry not in changelog:
        raise RuntimeError("bounded subprocess changelog entry not found")
    changelog = changelog.replace(old_entry, "Bounded subprocess adapter classification** (RIPR-SPEC-0144", 1)
    changelog_path.write_text(changelog, encoding="utf-8")

    trace_path = ROOT / ".ripr/traceability.toml"
    trace = trace_path.read_text(encoding="utf-8")
    sections = trace.split("[[behavior]]")
    changed = 0
    for index, section in enumerate(sections):
        if "crates/ripr/src/analysis/probes/subprocess.rs" in section:
            updated = section.replace(old_id, new_id).replace(old_path, new_path)
            if updated != section:
                sections[index] = updated
                changed += 1
    if changed != 1:
        raise RuntimeError(f"expected one bounded-subprocess traceability block, changed {changed}")
    trace_path.write_text("[[behavior]]".join(sections), encoding="utf-8")

    index_path = ROOT / "docs/specs/README.md"
    index_text = index_path.read_text(encoding="utf-8")
    row = (
        "| [RIPR-SPEC-0144](RIPR-SPEC-0144-bounded-subprocess-adapter-boundary.md) "
        "| accepted | Bounded, deny-by-default subprocess adapter classification as the existing "
        "side_effect family; literal curl only, with arguments, timeout, captured output, cleanup, "
        "and explicit error handling; no process execution or runtime-safety claim |\n"
    )
    if "RIPR-SPEC-0144" in index_text:
        raise RuntimeError("RIPR-SPEC-0144 is already occupied")
    if not index_text.endswith("\n"):
        index_text += "\n"
    index_path.write_text(index_text + row, encoding="utf-8")


def add_allowlist_rows() -> None:
    process_path = ROOT / "policy/process_allowlist.txt"
    process = process_path.read_text(encoding="utf-8")
    process_row = (
        "crates/ripr/src/analysis/probes/subprocess.rs|Command::new|7|analysis/probes|"
        "RIPR-SPEC-0144: bounded subprocess adapter recognizer inspects source text and fixture "
        "bodies for one literal allowlisted command; it does not execute a subprocess or broaden "
        "the runtime command surface.\n"
    )
    if "crates/ripr/src/analysis/probes/subprocess.rs|Command::new" not in process:
        if not process.endswith("\n"):
            process += "\n"
        process += process_row
    process_path.write_text(process, encoding="utf-8")

    network_path = ROOT / "policy/network_allowlist.txt"
    network = network_path.read_text(encoding="utf-8")
    network_row = (
        "crates/ripr/src/analysis/probes/subprocess.rs|curl|6|analysis/probes|"
        "RIPR-SPEC-0144: bounded subprocess adapter recognizer and fixture corpus inspect an "
        "allowlisted literal command; this source does not execute networking or add a network client.\n"
    )
    if "crates/ripr/src/analysis/probes/subprocess.rs|curl" not in network:
        if not network.endswith("\n"):
            network += "\n"
        network += network_row
    network_path.write_text(network, encoding="utf-8")


def preserve_source_authority(source_parent: str, plan: dict[str, Any]) -> None:
    for path in plan["source_authority_paths"]:
        content = git_text(source_parent, path)
        write(path, content)

    repo_settings = ROOT / "docs/REPO_SETTINGS.md"
    text = repo_settings.read_text(encoding="utf-8")
    old = (
        "This checkout is `EffortlessMetrics/ripr-swarm`, the public development landing\n"
        "zone for trusted same-repo `ripr` PRs. The release-facing source repository\n"
        "remains `EffortlessMetrics/ripr`."
    )
    new = (
        "This checkout is `EffortlessMetrics/ripr`, the release-facing source and\n"
        "distribution authority. Normal analyzer, editor, fixture, and development work lands\n"
        "in `EffortlessMetrics/ripr-swarm` and reaches this repository only through a reviewed\n"
        "history-preserving source promotion."
    )
    if old in text:
        text = text.replace(old, new, 1)
    repo_settings.write_text(text, encoding="utf-8")

    for path in plan["excluded_swarm_paths"]:
        git("rm", "--ignore-unmatch", path)

    workflow_allowlist = ROOT / "policy/workflow_allowlist.txt"
    if workflow_allowlist.exists():
        rows = [
            line
            for line in workflow_allowlist.read_text(encoding="utf-8").splitlines()
            if ".github/workflows/ub-review.yml" not in line
        ]
        workflow_allowlist.write_text("\n".join(rows) + "\n", encoding="utf-8")


def resolve_conflicts(plan: dict[str, Any]) -> None:
    source_parent = plan["source_parent"]

    checkout("ours", "CHANGELOG.md")
    checkout(
        "theirs",
        "crates/ripr/src/analysis/probes/diff.rs",
        "crates/ripr/src/output/review_comments.rs",
        "docs/specs/README.md",
        "editors/vscode/package-lock.json",
        "editors/vscode/package.json",
        "fixtures/boundary_gap/expected/pr-guidance/configured-off/comments.json",
        "policy/process_allowlist.txt",
    )

    integrate_bounded_subprocess()
    migrate_bounded_subprocess_spec(source_parent)
    add_allowlist_rows()
    preserve_source_authority(source_parent, plan)

    set_cargo_package_version("crates/ripr/Cargo.toml", "0.10.1")
    set_cargo_lock_version("0.10.1")
    set_node_versions("0.10.1")


def validate_tree(plan: dict[str, Any]) -> None:
    source_parent = plan["source_parent"]
    swarm_parent = plan["swarm_parent"]

    unmerged = git("diff", "--name-only", "--diff-filter=U").stdout.strip()
    if unmerged:
        raise RuntimeError(f"unresolved merge entries remain:\n{unmerged}")

    git("diff", "--cached", "--check")

    required_paths = [
        "crates/ripr/src/analysis/probes/subprocess.rs",
        "docs/specs/RIPR-SPEC-0144-bounded-subprocess-adapter-boundary.md",
        "docs/release/0.11.0/post-freeze-delta-ledger.json",
        "docs/SOURCE_PROMOTION.md",
        ".github/settings.yml",
    ]
    missing = [path for path in required_paths if not (ROOT / path).exists()]
    if missing:
        raise RuntimeError(f"required source survivors are missing: {missing}")
    if (ROOT / ".github/workflows/ub-review.yml").exists():
        raise RuntimeError("swarm-only ub-review workflow crossed into source")

    set_version = {
        "crate": re.search(
            r'(?ms)^\[package\].*?^version\s*=\s*"([^"]+)"',
            (ROOT / "crates/ripr/Cargo.toml").read_text(encoding="utf-8"),
        ).group(1),
        "extension": json.loads(
            (ROOT / "editors/vscode/package.json").read_text(encoding="utf-8")
        )["version"],
    }
    if set_version != {"crate": "0.10.1", "extension": "0.10.1"}:
        raise RuntimeError(f"promotion metadata drifted: {set_version}")

    trace = (ROOT / ".ripr/traceability.toml").read_text(encoding="utf-8")
    if "RIPR-SPEC-0144" not in trace or "RIPR-SPEC-0112-bounded-subprocess" in trace:
        raise RuntimeError("bounded subprocess traceability was not migrated to 0144")

    git("add", "-A")
    git("diff", "--cached", "--check")
    git("commit", "-m", "promote: history-preserving 0.11 replacement candidate")

    join = git("rev-parse", "HEAD").stdout.strip()
    parents = git("show", "-s", "--format=%P", "HEAD").stdout.strip().split()
    if parents != [source_parent, swarm_parent]:
        raise RuntimeError(f"wrong join parents: {parents}")

    checks = [
        ["cargo", "fmt", "--all", "--", "--check"],
        ["cargo", "test", "-p", "ripr", "subprocess", "--locked"],
        [
            "cargo",
            "test",
            "-p",
            "ripr",
            "probes_for_file_emits_side_effect_for_bounded_adapter_line",
            "--locked",
        ],
        ["cargo", "xtask", "check-spec-format"],
        ["cargo", "xtask", "check-spec-numbering"],
        ["cargo", "xtask", "check-traceability"],
        ["cargo", "xtask", "check-doc-index"],
        ["cargo", "xtask", "check-doc-artifacts"],
        ["cargo", "xtask", "check-file-policy"],
        ["cargo", "xtask", "check-static-language"],
        ["cargo", "xtask", "check-workflows"],
        ["cargo", "xtask", "check-process-policy"],
        ["cargo", "xtask", "check-network-policy"],
        ["cargo", "xtask", "check-architecture"],
        ["cargo", "xtask", "check-workspace-shape"],
        ["cargo", "xtask", "check-output-contracts"],
        ["cargo", "xtask", "goldens", "check"],
        ["cargo", "check", "--workspace", "--all-targets", "--all-features", "--locked"],
    ]
    results: list[dict[str, Any]] = []
    for command in checks:
        result = run(command, check=False)
        results.append(
            {
                "command": command,
                "returncode": result.returncode,
                "stdout_tail": result.stdout.splitlines()[-20:],
                "stderr_tail": result.stderr.splitlines()[-20:],
            }
        )
        if result.returncode != 0:
            write_json(OUTPUT_DIR / "checks.json", results)
            raise RuntimeError(f"validation failed: {' '.join(command)}")

    target_branch = plan["target_branch"]
    remote = git("ls-remote", "--heads", "origin", f"refs/heads/{target_branch}").stdout.strip()
    if remote:
        remote_sha = remote.split()[0]
        if remote_sha != join:
            raise RuntimeError(
                f"target branch already exists at {remote_sha}; refusing to rewrite it with {join}"
            )
    else:
        git("push", "origin", f"HEAD:refs/heads/{target_branch}")

    receipt = {
        "schema_version": "1.0",
        "kind": "ripr_source_promotion_builder_receipt",
        "status": "join_built_validated_and_pushed",
        "join_sha": join,
        "tree_sha": git("rev-parse", "HEAD^{tree}").stdout.strip(),
        "parents": parents,
        "target_branch": target_branch,
        "checks": results,
        "authority_boundary": plan["authority_boundary"],
    }
    write_json(OUTPUT_DIR / "builder-receipt.json", receipt)
    print(json.dumps(receipt, indent=2, sort_keys=True))


def build() -> int:
    plan = json.loads(PLAN_PATH.read_text(encoding="utf-8"))
    source_parent = plan["source_parent"]
    swarm_parent = plan["swarm_parent"]
    expected_conflicts = sorted(plan["expected_conflicts"])

    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    git("config", "--local", "user.name", "RIPR Source Promotion Builder")
    git("config", "--local", "user.email", "source-promotion-builder@example.invalid")

    if git("remote", "get-url", "swarm", check=False).returncode == 0:
        git("remote", "set-url", "swarm", SWARM_URL)
    else:
        git("remote", "add", "swarm", SWARM_URL)
    git("fetch", "--no-tags", "origin", source_parent)
    git("fetch", "--no-tags", "swarm", swarm_parent)

    git("checkout", "--detach", source_parent)
    git("switch", "-c", "_promotion_build")
    merge = git("merge", "--no-ff", "--no-commit", swarm_parent, check=False)
    conflicts = sorted(
        line.strip()
        for line in git("diff", "--name-only", "--diff-filter=U", check=False).stdout.splitlines()
        if line.strip()
    )
    if conflicts != expected_conflicts:
        raise RuntimeError(
            f"conflict set changed\nexpected={expected_conflicts}\nobserved={conflicts}\n"
            f"merge stdout:\n{merge.stdout}\nmerge stderr:\n{merge.stderr}"
        )

    resolve_conflicts(plan)
    validate_tree(plan)
    return 0


def main() -> int:
    try:
        return build()
    except Exception as error:  # noqa: BLE001 - persist exact builder failure
        OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
        failure = {
            "schema_version": "1.0",
            "kind": "ripr_source_promotion_builder_failure",
            "status": "failed",
            "error_type": type(error).__name__,
            "message": str(error),
            "traceback": traceback.format_exc().splitlines()[-30:],
        }
        write_json(OUTPUT_DIR / "builder-failure.json", failure)
        print(f"source-promotion builder failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
