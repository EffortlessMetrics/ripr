"""Admit one qualified source-owned prerelease wheel, without executing it.

The caller checks out this controller from the publisher's trusted main SHA,
not from an artifact. Only GitHub API metadata supplies run authority; a receipt
alone is never proof of successful qualification.
"""
import argparse
from email.parser import Parser
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import zipfile

REPOSITORY = "EffortlessMetrics/ripr"
TARGET = "x86_64-unknown-linux-gnu"
PLATFORM_TAG = "py3-none-manylinux_2_34_x86_64"
WORKFLOW = ".github/workflows/python-wheel-qualification.yml"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def validate_identity(identity):
    for field, pattern in {
        "run_id": r"[1-9][0-9]*",
        "source_sha": r"[0-9a-f]{40}",
        "source_tree": r"[0-9a-f]{40}",
        "wheel_sha256": r"[0-9a-f]{64}",
        # This initial publisher intentionally admits genuine prereleases only.
        "version": r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)(?:a|b|rc)(?:0|[1-9][0-9]*)",
    }.items():
        require(isinstance(identity.get(field), str) and re.fullmatch(pattern, identity[field]),
                f"invalid {field}")


def validate_run(run, commit, identity):
    validate_identity(identity)
    expected = dict(id=int(identity["run_id"]), event="workflow_dispatch", head_branch="main",
                    head_sha=identity["source_sha"], status="completed", conclusion="success", path=WORKFLOW)
    for field, value in expected.items():
        require(run.get(field) == value, f"qualification run {field} mismatch")
    for field in ("repository", "head_repository"):
        require(run.get(field, {}).get("full_name") == REPOSITORY, f"untrusted run {field}")
    attempt = run.get("run_attempt")
    require(type(attempt) is int and attempt > 0, "invalid run attempt")
    require(commit.get("sha") == identity["source_sha"], "source commit mismatch")
    require(commit.get("tree", {}).get("sha") == identity["source_tree"], "source tree mismatch")
    return attempt


def api(endpoint):
    # Endpoints are constructed only after strict identity validation. gh reads
    # the token from GH_TOKEN; no token is printed or added to command arguments.
    return json.loads(subprocess.check_output(["gh", "api", f"repos/{REPOSITORY}/{endpoint}"], text=True))


def authorize(identity):
    validate_identity(identity)
    run = api(f"actions/runs/{identity['run_id']}")
    commit = api(f"git/commits/{identity['source_sha']}")
    attempt = validate_run(run, commit, identity)
    name = f"pypi-qualified-wheel-{identity['run_id']}-{attempt}"
    result = api(f"actions/runs/{identity['run_id']}/artifacts?per_page=100")
    require(result.get("total_count", 0) <= 100, "artifact list exceeds admission bound")
    candidates = [a for a in result["artifacts"] if a.get("name") == name]
    require(len(candidates) == 1, "expected exactly one attempt-bound qualification artifact")
    artifact = candidates[0]
    require(artifact.get("expired") is False, "qualification artifact expired")
    require(type(artifact.get("id")) is int and artifact["id"] > 0, "invalid artifact ID")
    provenance = artifact.get("workflow_run", {})
    require(provenance.get("id") == int(identity["run_id"]), "artifact run mismatch")
    require(provenance.get("head_sha") == identity["source_sha"], "artifact source mismatch")
    return artifact["id"], attempt


def stage_wheel(root, destination, identity, attempt):
    validate_identity(identity)
    receipt_path = root / "qualification.json"
    require(receipt_path.is_file() and not receipt_path.is_symlink(), "missing or linked receipt")
    receipt = json.loads(receipt_path.read_text(encoding="utf-8"))
    filename = f"ripr_rs-{identity['version']}-{PLATFORM_TAG}.whl"
    prerelease = re.fullmatch(r"([0-9]+\.[0-9]+\.[0-9]+)(a|b|rc)([0-9]+)", identity["version"])
    phase = {"a": "alpha", "b": "beta", "rc": "rc"}[prerelease[2]]
    native_version = f"{prerelease[1]}-{phase}.{prerelease[3]}"
    expected = dict(schema_version=1, repository=REPOSITORY, run_id=int(identity["run_id"]),
                    run_attempt=attempt, native_version=native_version, source_sha=identity["source_sha"],
                    source_tree=identity["source_tree"], version=identity["version"],
                    wheel_filename=filename, wheel_sha256=identity["wheel_sha256"],
                    target=TARGET, platform_tag=PLATFORM_TAG)
    for field, value in expected.items():
        require(type(receipt.get(field)) is type(value) and receipt[field] == value,
                f"qualification receipt {field} mismatch")
    wheelhouse = root / "wheelhouse"
    require(wheelhouse.is_dir() and not wheelhouse.is_symlink(), "invalid wheelhouse")
    require(sorted(p.name for p in wheelhouse.iterdir()) == [filename], "unexpected wheel set")
    wheel = wheelhouse / filename
    require(wheel.is_file() and not wheel.is_symlink(), "invalid wheel file")
    require(hashlib.sha256(wheel.read_bytes()).hexdigest() == identity["wheel_sha256"], "wheel digest mismatch")
    with zipfile.ZipFile(wheel) as archive:
        names = archive.namelist()
        require(len(names) == len(set(names)), "duplicate wheel members")
        metadata_name = f"ripr_rs-{identity['version']}.dist-info/METADATA"
        wheel_name = f"ripr_rs-{identity['version']}.dist-info/WHEEL"
        require([n for n in names if n.endswith(".dist-info/METADATA")] == [metadata_name], "unexpected metadata set")
        require([n for n in names if n.endswith(".dist-info/WHEEL")] == [wheel_name], "unexpected WHEEL set")
        metadata = Parser().parsestr(archive.read(metadata_name).decode("utf-8"))
        require(metadata.get_all("Name") == ["ripr-rs"], "distribution mismatch")
        require(metadata.get_all("Version") == [identity["version"]], "version mismatch")
        tags = Parser().parsestr(archive.read(wheel_name).decode("utf-8")).get_all("Tag")
        require(tags == [PLATFORM_TAG], "wheel platform mismatch")
    destination.mkdir(parents=True, exist_ok=False)
    shutil.copyfile(wheel, destination / filename)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=("authorize", "stage"))
    parser.add_argument("--artifact-root", type=Path)
    parser.add_argument("--destination", type=Path)
    parser.add_argument("--attempt", type=int)
    args = parser.parse_args()
    identity = {key: os.environ[key.upper()] for key in
                ("run_id", "source_sha", "source_tree", "version", "wheel_sha256")}
    if args.operation == "authorize":
        artifact_id, attempt = authorize(identity)
        with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as output:
            output.write(f"artifact_id={artifact_id}\nattempt={attempt}\n")
    else:
        require(args.artifact_root is not None and args.destination is not None and args.attempt is not None,
                "stage requires artifact root, destination, and attempt")
        stage_wheel(args.artifact_root, args.destination, identity, args.attempt)


if __name__ == "__main__":
    main()
