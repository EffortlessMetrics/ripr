"""Prepare/inspect a Linux npm tarball from same-run qualified native bytes.

No npm credentials or registry writes. Product and packaging source identities are
separate: packaging never relabels native bytes as a new product build.
"""
import argparse
import base64
import csv
from email.parser import Parser
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import stat
import subprocess
import tarfile
import tempfile
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parents[2]
PACKAGE_NAME = "@effortlessmetrics/ripr"
FILES = {"package.json", "README.md", "LICENSE-MIT", "LICENSE-APACHE", "bin/ripr", "provenance.json", "sbom.cyclonedx.json"}
REPOSITORY = "EffortlessMetrics/ripr"
QUALIFIER = ".github/workflows/npm-package-qualification.yml"
CLIENTS = (("20.20.1", "10.8.2"), ("24.19.0", "11.9.0"))
NATIVE_JOBS = {"native / build and inspect Linux x64 wheel",
               "native / clean pip install and useful Python journey",
               "native / clean uv install and useful Python journey"}
ROUTES = ("local", "global", "npm-exec", "npx")
CONSUMER_CONTROLS = (
    "piped LSP initialize/shutdown preserves clean framed stdout",
    "invalid command returns nonzero", "planted PATH and project Python never execute",
    "explicit preview disablement retains incomplete status",
    "npm rejects contradictory os metadata", "npm rejects contradictory cpu metadata",
    "npm rejects contradictory libc metadata",
    "verified removal and fresh reinstall restore bytes and useful behavior; final uninstall preserves project",
)


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def canonical(value):
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def run(args, cwd=ROOT):
    return subprocess.check_output(args, cwd=cwd, text=True, timeout=120).strip()


def source_contract(root=ROOT):
    """Derive version authority from Cargo, never from a previous registry pin."""
    version_match = re.search(r'\[workspace.package\][\s\S]*?^version = "([^"]+)"', (root / "Cargo.toml").read_text(), re.MULTILINE)
    require(version_match is not None, "missing workspace version")
    version = version_match[1]
    parts = re.fullmatch(r"((?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*))-(alpha|beta|rc)\.((?:0|[1-9][0-9]*))", version)
    require(parts is not None, "prerelease version required")
    python_version = parts[1] + {"alpha": "a", "beta": "b", "rc": "rc"}[parts[2]] + parts[3]
    toolchain = re.search(r'^channel = "([^"]+)"$', (root / "rust-toolchain.toml").read_text(), re.MULTILINE)
    require(toolchain is not None and toolchain[1] == "1.95.0", "unsupported native build toolchain")
    features = re.search(r"^features = (\[[^\n]+\])$", (root / "packaging/python/pyproject.toml").read_text(), re.MULTILINE)
    require(features is not None and json.loads(features[1]) == ["lang-python", "lang-rust", "lang-typescript"], "native adapter feature contract changed")
    return dict(schema_version=2, repository=REPOSITORY, native_version=version, python_version=python_version,
                target="x86_64-unknown-linux-gnu", minimum_glibc="2.34", rust_toolchain=toolchain[1],
                features=json.loads(features[1]), cargo_lock_sha256=digest((root / "Cargo.lock").read_bytes()),
                wheel_filename=f"ripr_rs-{python_version}-py3-none-manylinux_2_34_x86_64.whl")


def inspect_native_evidence(files, source, run_id, attempt, artifact, root=ROOT):
    """Derive a native pin from independently bound same-source wheel evidence."""
    require(set(source) == {"sha", "tree"} and all(isinstance(v, str) and re.fullmatch(r"[0-9a-f]{40}", v) for v in source.values()), "invalid native source identity")
    require(all(isinstance(v, str) and re.fullmatch(r"[1-9][0-9]*", v) for v in (run_id, attempt)), "invalid native run identity")
    require(artifact.get("name") == f"npm-native-wheel-{run_id}-{attempt}" and type(artifact.get("id")) is int and artifact["id"] > 0, "wrong native artifact identity")
    require(isinstance(artifact.get("digest"), str) and re.fullmatch(r"sha256:[0-9a-f]{64}", artifact["digest"]), "missing native artifact digest")
    require(type(artifact.get("size_in_bytes")) is int and 0 < artifact["size_in_bytes"] <= 40_000_000, "invalid native artifact size")
    contract = source_contract(root)
    filename = contract["wheel_filename"]
    require(set(files) == {"qualification.json", "wheel-receipt.json", "wheelhouse/" + filename}, "native artifact inventory mismatch")
    wheel = files["wheelhouse/" + filename]
    wheel_hash = digest(wheel)
    qualification = json.loads(files["qualification.json"])
    expected = dict(schema_version=1, source_sha=source["sha"], source_tree=source["tree"],
                    version=contract["python_version"], native_version=contract["native_version"],
                    wheel_filename=filename, wheel_sha256=wheel_hash, repository=REPOSITORY,
                    run_id=int(run_id), run_attempt=int(attempt), target=contract["target"],
                    platform_tag="py3-none-manylinux_2_34_x86_64")
    require(isinstance(qualification, dict), "missing native qualification object")
    for key, value in expected.items():
        require(type(qualification.get(key)) is type(value) and qualification[key] == value, f"native qualification {key} mismatch")
    receipt = json.loads(files["wheel-receipt.json"])
    require(isinstance(receipt, dict), "missing native wheel receipt object")
    expected = dict(schema_version=2, channel="pypi-source-wheelhouse", source_repository=REPOSITORY,
                    producer_run_id=run_id, producer_run_attempt=attempt, publication_attempted=False,
                    candidate_sha=source["sha"], candidate_tree=source["tree"],
                    cargo_lock_sha256=contract["cargo_lock_sha256"], native_version=contract["native_version"],
                    python_version=contract["python_version"], distribution="ripr-rs", executable="ripr",
                    rust_target=contract["target"], compatibility_state="auditwheel_manylinux_2_34",
                    native_execution="ubuntu-22.04-x64", scope="linux-x64-glibc-only", features=contract["features"])
    for key, value in expected.items():
        require(type(receipt.get(key)) is type(value) and receipt[key] == value, f"native receipt {key} mismatch")
    toolchain = receipt.get("toolchain")
    require(isinstance(toolchain, dict) and isinstance(toolchain.get("rustc"), str) and
            toolchain["rustc"].startswith("rustc " + contract["rust_toolchain"] + " (") and
            "\nhost: x86_64-unknown-linux-gnu\n" in toolchain["rustc"], "native build toolchain mismatch")
    metadata = receipt.get("wheel")
    require(isinstance(metadata, dict) and metadata.get("filename") == filename and
            metadata.get("sha256") == wheel_hash and metadata.get("tag") == qualification["platform_tag"], "native wheel metadata mismatch")
    payload_hash = metadata.get("installed_payload_sha256")
    require(isinstance(payload_hash, str) and re.fullmatch(r"[0-9a-f]{64}", payload_hash), "missing native payload digest")
    pin = dict(contract, origin="same-run-qualified-wheel", product_source_sha=source["sha"],
               product_source_tree=source["tree"], wheel_sha256=wheel_hash, payload_sha256=payload_hash,
               qualification_run_id=int(run_id), qualification_run_attempt=int(attempt), native_artifact=artifact,
               native_qualification_url=f"https://github.com/{REPOSITORY}/actions/runs/{run_id}")
    return pin, inspect_wheel(wheel, pin, root)


def manifest(pin, root=ROOT):
    value = json.loads((root / "packaging/npm/package.template.json").read_text())
    require("version" not in value, "npm version must derive from native pin")
    value["version"] = pin["native_version"]
    validate_manifest(value, pin)
    return value


def validate_manifest(value, pin):
    require(value.get("name") == PACKAGE_NAME, "wrong npm name")
    require(value.get("version") == pin["native_version"], "wrong npm version")
    require(value.get("bin") == {"ripr": "bin/ripr"}, "wrong npm executable")
    require(value.get("os") == ["linux"] and value.get("cpu") == ["x64"] and value.get("libc") == ["glibc"], "wrong npm platform")
    require(value.get("engines") == {"npm": ">=10"}, "wrong npm client contract")
    require(value.get("publishConfig") == {"access": "public", "tag": "next"}, "wrong publish access or tag")
    require(value.get("repository") == {"type": "git", "url": "https://github.com/EffortlessMetrics/ripr.git", "directory": "packaging/npm"}, "wrong npm repository")
    require(value.get("license") == "MIT OR Apache-2.0", "wrong npm license")
    require(set(value.get("files", [])) == FILES - {"package.json"}, "wrong package file allowlist")
    for key in ("scripts", "dependencies", "optionalDependencies", "devDependencies", "bundledDependencies", "bundleDependencies", "tag"):
        require(key not in value, f"unexpected {key}")


def inspect_wheel(data, pin, root=ROOT):
    require(digest(data) == pin["wheel_sha256"], "wheel digest mismatch")
    prefix = f"ripr_rs-{pin['python_version']}"
    native = f"{prefix}.data/scripts/ripr"
    info = f"{prefix}.dist-info"
    expected = {native, f"{info}/METADATA", f"{info}/WHEEL", f"{info}/RECORD", f"{info}/licenses/LICENSE-MIT", f"{info}/licenses/LICENSE-APACHE", f"{info}/sboms/ripr.cyclonedx.json"}
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        entries = archive.infolist()
        require(len(entries) == len({x.filename for x in entries}), "duplicate wheel entry")
        for entry in entries:
            path = PurePosixPath(entry.filename)
            require(not path.is_absolute() and ".." not in path.parts and "\\" not in entry.filename, "unsafe wheel path")
            require(not stat.S_ISLNK(entry.external_attr >> 16), "wheel symlink")
            require(entry.file_size < 30_000_000, "oversized wheel entry")
        names = {x.filename for x in entries if not x.is_dir()}
        require(names == expected, "unexpected wheel file set")
        metadata = Parser().parsestr(archive.read(f"{info}/METADATA").decode())
        require(metadata.get_all("Name") == ["ripr-rs"] and metadata.get_all("Version") == [pin["python_version"]], "wrong wheel identity")
        require(not metadata.get_all("Requires-Dist"), "wheel acquired dependencies")
        require(Parser().parsestr(archive.read(f"{info}/WHEEL").decode()).get_all("Tag") == ["py3-none-manylinux_2_34_x86_64"], "wrong wheel platform")
        rows = list(csv.reader(io.StringIO(archive.read(f"{info}/RECORD").decode())))
        require(len(rows) == len(names) and {row[0] for row in rows} == names, "wrong RECORD file set")
        for name, algorithm_digest, size in rows:
            if name.endswith("/RECORD"):
                require(algorithm_digest == size == "", "wrong RECORD self-reference")
                continue
            member = archive.read(name)
            expected_digest = base64.urlsafe_b64encode(hashlib.sha256(member).digest()).rstrip(b"=").decode()
            require(algorithm_digest == "sha256=" + expected_digest and size == str(len(member)), "stale RECORD")
        binary = archive.read(native)
        require(digest(binary) == pin["payload_sha256"], "payload digest mismatch")
        require(binary[:6] == b"\x7fELF\x02\x01" and int.from_bytes(binary[18:20], "little") == 62, "payload is not Linux x86-64 ELF")
        require(archive.getinfo(native).external_attr >> 16 & 0o111, "wheel executable mode missing")
        output = {"bin/ripr": binary, "sbom.cyclonedx.json": archive.read(f"{info}/sboms/ripr.cyclonedx.json")}
        for name in ("LICENSE-MIT", "LICENSE-APACHE"):
            output[name] = archive.read(f"{info}/licenses/{name}")
            require(output[name] == (root / name).read_bytes(), "license bytes differ")
        return output


def validate_tarball(path, pin, provenance, root=ROOT):
    require(path.is_file() and not path.is_symlink(), "invalid tarball path")
    with tarfile.open(path, "r:gz") as archive:
        members = archive.getmembers()
        require(len(members) == len(FILES) and {x.name for x in members} == {"package/" + name for name in FILES}, "wrong npm file inventory")
        require(all(x.isfile() and not x.issym() and not x.islnk() and x.size < 30_000_000 for x in members), "invalid npm entry type or size")
        data = {x.name.removeprefix("package/"): archive.extractfile(x).read() for x in members}
        validate_manifest(json.loads(data["package.json"]), pin)
        require(json.loads(data["package.json"]) == manifest(pin, root), "npm manifest differs from trusted template")
        require(json.loads(data["provenance.json"]) == provenance, "npm provenance mismatch")
        require(data["provenance.json"] == canonical(provenance), "noncanonical npm provenance")
        require(digest(data["bin/ripr"]) == pin["payload_sha256"], "npm executable digest mismatch")
        require(archive.getmember("package/bin/ripr").mode & 0o111, "npm executable mode missing")
        for name in ("LICENSE-MIT", "LICENSE-APACHE"):
            require(data[name] == (root / name).read_bytes(), "npm license mismatch")
        require(data["README.md"] == (root / "packaging/npm/README.md").read_bytes(), "npm readme mismatch")
        require(digest(data["sbom.cyclonedx.json"]) == provenance["sbom_sha256"], "npm SBOM mismatch")
    return {"filename": path.name, "sha256": digest(path.read_bytes()), "integrity": "sha512-" + base64.b64encode(hashlib.sha512(path.read_bytes()).digest()).decode(), "files": sorted(FILES)}


def validate_release_identity(identity):
    """Reject ambiguous identities before forming paths, API requests or outputs."""
    for field, pattern in {
        "run_id": r"[1-9][0-9]*", "run_attempt": r"[1-9][0-9]*",
        "source_sha": r"[0-9a-f]{40}", "source_tree": r"[0-9a-f]{40}",
        "tarball_sha256": r"[0-9a-f]{64}",
        "version": r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)-(?:alpha|beta|rc)\.(?:0|[1-9][0-9]*)",
    }.items():
        require(isinstance(identity.get(field), str) and re.fullmatch(pattern, identity[field]),
                f"invalid release {field}")


def github_api(endpoint, binary=False):
    """Use the runner's read-only GH_TOKEN without copying it into arguments."""
    data = subprocess.check_output(["gh", "api", f"repos/{REPOSITORY}/{endpoint}"], timeout=120)
    return data if binary else json.loads(data)


def authorize_release(identity, publisher_sha):
    """Bind candidate, qualifier and artifacts to independent current-main facts."""
    validate_release_identity(identity)
    ref = github_api("git/ref/heads/main")
    require(ref.get("ref") == "refs/heads/main" and ref.get("object", {}).get("type") == "commit",
            "qualification authority is not the main branch")
    require(ref["object"].get("sha") == publisher_sha == identity["source_sha"],
            "publisher and qualified source must equal current main")
    run_info = github_api(f"actions/runs/{identity['run_id']}")
    commit = github_api(f"git/commits/{identity['source_sha']}")
    workflow = github_api("actions/workflows/npm-package-qualification.yml")
    expected = dict(id=int(identity["run_id"]), run_attempt=int(identity["run_attempt"]),
                    event="workflow_dispatch", head_branch="main", head_sha=identity["source_sha"],
                    status="completed", conclusion="success", path=QUALIFIER, workflow_id=workflow.get("id"))
    require(type(workflow.get("id")) is int and workflow.get("path") == QUALIFIER, "wrong qualifier workflow")
    for key, value in expected.items():
        require(type(run_info.get(key)) is type(value) and run_info[key] == value, f"qualification run {key} mismatch")
    for key in ("repository", "head_repository"):
        repository = run_info.get(key)
        require(isinstance(repository, dict), f"missing qualification {key} object")
        require(repository.get("full_name") == REPOSITORY, "foreign qualification repository")
        require(type(repository.get("id")) is int and repository["id"] > 0, "missing repository identity")
    require(commit.get("sha") == identity["source_sha"] and commit.get("tree", {}).get("sha") == identity["source_tree"], "source commit/tree mismatch")
    jobs = github_api(f"actions/runs/{identity['run_id']}/attempts/{identity['run_attempt']}/jobs?per_page=100")
    names = {*NATIVE_JOBS, "package", "publisher-controls", *(f"consumer ({node}, {npm})" for node, npm in CLIENTS)}
    require(jobs.get("total_count") == len(jobs.get("jobs", [])) == len(names), "missing qualification jobs")
    require({job.get("name") for job in jobs["jobs"]} == names, "qualification job set mismatch")
    require(all(job.get("status") == "completed" and job.get("conclusion") == "success" for job in jobs["jobs"]), "qualification job did not execute successfully")
    listing = github_api(f"actions/runs/{identity['run_id']}/artifacts?per_page=100")
    suffix = f"{identity['run_id']}-{identity['run_attempt']}"
    wanted = [f"npm-prepared-{suffix}", *(f"npm-consumer-{node}-{suffix}" for node, _ in CLIENTS),
              f"npm-publisher-controls-{suffix}", f"npm-native-wheel-{suffix}"]
    return {"identity": identity, "artifacts": select_artifacts(listing, wanted, run_info)}


def select_artifacts(listing, wanted, run_info):
    require(type(listing.get("total_count")) is int and listing["total_count"] == len(listing.get("artifacts", [])) <= 100, "incomplete artifact list")
    artifacts = []
    for name in wanted:
        matches = [artifact for artifact in listing["artifacts"] if artifact.get("name") == name]
        require(len(matches) == 1, "missing or duplicate attempt-bound artifact")
        artifact = matches[0]
        require(artifact.get("expired") is False, "qualification artifact expired")
        require(type(artifact.get("id")) is int and artifact["id"] > 0, "invalid artifact ID")
        require(type(artifact.get("size_in_bytes")) is int and 0 < artifact["size_in_bytes"] <= 40_000_000, "invalid artifact size")
        require(isinstance(artifact.get("digest"), str) and re.fullmatch(r"sha256:[0-9a-f]{64}", artifact["digest"]), "missing artifact digest")
        binding = artifact.get("workflow_run")
        require(isinstance(binding, dict), "missing artifact workflow_run object")
        require(binding.get("id") == run_info["id"] and binding.get("head_sha") == run_info["head_sha"], "artifact source/run mismatch")
        require(binding.get("repository_id") == run_info["repository"]["id"] and binding.get("head_repository_id") == run_info["head_repository"]["id"], "artifact repository mismatch")
        artifacts.append({key: artifact[key] for key in ("id", "name", "digest", "size_in_bytes")})
    require(len({artifact["id"] for artifact in artifacts}) == len(wanted), "duplicate artifact ID")
    return artifacts


def prepare_native_input(source, run_id, attempt):
    """Qualification may run on a PR; release admission still requires main."""
    require(all(isinstance(v, str) and re.fullmatch(r"[1-9][0-9]*", v) for v in (run_id, attempt)), "qualification run/attempt required")
    run_info = github_api(f"actions/runs/{run_id}")
    require(type(run_info.get("id")) is int and run_info["id"] == int(run_id) and
            type(run_info.get("run_attempt")) is int and run_info["run_attempt"] == int(attempt), "native preparation attempt mismatch")
    require(run_info.get("head_sha") == source["sha"] and run_info.get("path") == QUALIFIER,
            "native preparation source/workflow mismatch")
    require(run_info.get("event") == "pull_request" or
            (run_info.get("event") == "workflow_dispatch" and run_info.get("head_branch") == "main"), "unsupported qualification event")
    for key in ("repository", "head_repository"):
        value = run_info.get(key)
        require(isinstance(value, dict) and value.get("full_name") == REPOSITORY and
                type(value.get("id")) is int and value["id"] > 0, "foreign native preparation repository")
    jobs = github_api(f"actions/runs/{run_id}/attempts/{attempt}/jobs?per_page=100")
    require(jobs.get("total_count") == len(jobs.get("jobs", [])) <= 100, "incomplete native preparation job list")
    native_jobs = [job for job in jobs["jobs"] if job.get("name") in NATIVE_JOBS]
    require(len(native_jobs) == len(NATIVE_JOBS) and {job["name"] for job in native_jobs} == NATIVE_JOBS and
            all(job.get("status") == "completed" and job.get("conclusion") == "success" for job in native_jobs), "native build and pip/uv consumers must execute successfully")
    listing = github_api(f"actions/runs/{run_id}/artifacts?per_page=100")
    artifact, = select_artifacts(listing, [f"npm-native-wheel-{run_id}-{attempt}"], run_info)
    data = github_api(f"actions/artifacts/{artifact['id']}/zip", binary=True)
    require(len(data) == artifact["size_in_bytes"], "native artifact transfer size mismatch")
    return inspect_native_evidence(read_qualification_zip(data, artifact["digest"]), source, run_id, attempt, artifact)


def read_qualification_zip(data, expected_digest):
    """Read bounded regular files; never extract or execute artifact paths."""
    require(len(data) <= 40_000_000 and "sha256:" + digest(data) == expected_digest, "artifact ZIP digest/size mismatch")
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        entries = archive.infolist()
        require(len(entries) <= 50 and len({entry.filename for entry in entries}) == len(entries), "duplicate or excessive artifact entries")
        require(sum(entry.file_size for entry in entries) <= 60_000_000, "oversized artifact content")
        files = {}
        for entry in entries:
            path = PurePosixPath(entry.filename)
            require(entry.filename and not path.is_absolute() and ".." not in path.parts and "\\" not in entry.filename and str(path) == entry.filename.rstrip("/"), "unsafe artifact path")
            mode = entry.external_attr >> 16
            require(stat.S_IFMT(mode) in (0, stat.S_IFREG, stat.S_IFDIR), "linked or special artifact entry")
            require(entry.file_size <= 30_000_000, "oversized artifact entry")
            if not entry.is_dir():
                require(not stat.S_ISDIR(mode), "invalid artifact file mode")
                files[entry.filename] = archive.read(entry)
        return files


def validate_consumer_proof(files, node, npm, tar_hash, native_hash):
    """Require both receipt denominators and the retained nonempty route outputs."""
    routes = (*ROUTES, "fresh-reinstall")
    expected_files = {"consumer.json", "lsp-stderr.txt", *(f"{route}-{kind}" for route in routes for kind in ("check.json", "explain.txt"))}
    require(set(files) == expected_files, "consumer artifact inventory mismatch")
    proof = json.loads(files["consumer.json"])
    expected = dict(schema_version=1, state="passed", node="v" + node, npm=npm, npx=npm,
                    tarball_sha256=tar_hash, payload_sha256=native_hash,
                    selected_routes=4, executed_routes=4, failed_routes=0)
    for key, value in expected.items():
        require(type(proof.get(key)) is type(value) and proof[key] == value, f"consumer {key} mismatch")
    observed = proof.get("routes", [])
    require(len(observed) == 4 and {row.get("route") for row in observed} == set(ROUTES), "missing or duplicate consumer route")
    reinstall = proof.get("reinstall_journey", {})
    require(reinstall.get("route") == "fresh-reinstall", "missing fresh reinstall journey")
    for row in [*observed, reinstall]:
        route = row["route"]
        for count in ("probes", "findings"):
            require(type(row.get(count)) is int and row[count] > 0, "empty consumer journey")
        actual = json.loads(files[route + "-check.json"])
        require(all(type(actual.get("summary", {}).get(key)) is int and actual["summary"][key] == row[key] for key in ("probes", "findings")), "route denominator differs from retained output")
        require(len(actual.get("findings", [])) == row["findings"], "retained finding count mismatch")
        require(isinstance(row.get("follow_up_finding"), str) and row["follow_up_finding"].startswith("probe:") and row["follow_up_finding"] in {finding.get("id") for finding in actual.get("findings", [])}, "follow-up finding absent from retained output")
        require(files[route + "-explain.txt"].strip(), "empty retained explanation")
    require(proof.get("negative_controls") == list(CONSUMER_CONTROLS), "missing consumer negative controls")
    return proof


def stage_eligibility(public, version):
    """Never bootstrap a placeholder or restage an immutable public version."""
    require(public.get("name") == PACKAGE_NAME, "npm package does not exist under the selected identity")
    versions = public.get("versions")
    require(isinstance(versions, dict) and any(key != "0.0.0-stage" for key in versions), "existing genuine npm package required; placeholder bootstrap is forbidden")
    return version not in versions


def validate_stage_transport(files, tar_hash, integrity, version):
    require(set(files) == {"stage-cli-proof.json"}, "stage transport artifact inventory mismatch")
    proof = json.loads(files["stage-cli-proof.json"])
    for key, value in dict(schema_version=1, state="passed_local_transport_only", npm="11.15.0",
                           package=PACKAGE_NAME, version=version, tarball_sha256=tar_hash,
                           integrity=integrity, selected_requests=2, observed_requests=2,
                           lifecycle_marker_created=False, oidc_exercised=False,
                           external_registry_write_attempted=False).items():
        require(type(proof.get(key)) is type(value) and proof[key] == value, f"stage transport {key} mismatch")
    return proof


def public_package():
    url = "https://registry.npmjs.org/@effortlessmetrics%2fripr"
    request = urllib.request.Request(url, headers={"Accept": "application/json", "Cache-Control": "no-cache"})
    with urllib.request.urlopen(request, timeout=30) as response:
        require(response.url == url, "registry metadata redirected")
        data = response.read(5_000_001)
    require(len(data) <= 5_000_000, "oversized registry metadata")
    return json.loads(data)


def inspect_staging_response(path, identity, integrity):
    """A response is not staged-byte inspection, approval, or public delivery."""
    validate_release_identity(identity)
    data = path.read_bytes()
    require(len(data) < 100_000, "oversized staging response")
    response = json.loads(data)
    require(isinstance(response, dict) and set(response) == {PACKAGE_NAME}, "staging response package mismatch")
    package = response[PACKAGE_NAME]
    expected = dict(name=PACKAGE_NAME, version=identity["version"],
                    id=f"{PACKAGE_NAME}@{identity['version']}", integrity=integrity,
                    filename=f"effortlessmetrics-ripr-{identity['version']}.tgz", entryCount=7)
    for key, value in expected.items():
        require(type(package.get(key)) is type(value) and package[key] == value, f"staging response {key} mismatch")
    stage_id = package.get("stageId")
    require(isinstance(stage_id, str) and re.fullmatch(r"[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}", stage_id), "missing or invalid stage ID; inspect registry before retry")
    return dict(schema_version=1, state="staging_response_received", package=PACKAGE_NAME,
                version=identity["version"], stage_id=stage_id, requested_tag="next", access="public",
                source_sha=identity["source_sha"], source_tree=identity["source_tree"],
                qualification_run_id=identity["run_id"], qualification_run_attempt=identity["run_attempt"],
                tarball_sha256=identity["tarball_sha256"], integrity=integrity,
                response_sha256=digest(data), staged_bytes_independently_verified=False,
                maintainer_approval_performed=False, public_delivery_verified=False)


def validate_stage_environment(environment, policies):
    """Require an existing main-only reviewer gate before scheduling an OIDC job."""
    require(environment.get("name") == "npm" and type(environment.get("id")) is int and environment["id"] > 0,
            "npm environment must be explicitly created before staging")
    require(environment.get("deployment_branch_policy") == {"protected_branches": False, "custom_branch_policies": True},
            "npm environment requires explicit branch policies")
    require(policies.get("total_count") == 1 and len(policies.get("branch_policies", [])) == 1,
            "npm environment must allow only one deployment branch")
    policy = policies["branch_policies"][0]
    require(policy.get("name") == "main" and policy.get("type") == "branch", "npm environment must allow main branch only, no tags")
    rules = environment.get("protection_rules")
    require(isinstance(rules, list) and all(isinstance(rule, dict) for rule in rules), "missing environment protection rules")
    gates = [rule for rule in rules if rule.get("type") == "required_reviewers"]
    require(len(gates) == 1 and gates[0].get("prevent_self_review") is False,
            "npm environment requires a reviewer gate usable by the sole maintainer")
    reviewers = gates[0].get("reviewers")
    require(isinstance(reviewers, list) and len(reviewers) == 1 and isinstance(reviewers[0], dict) and
            reviewers[0].get("type") == "User", "missing environment reviewer object")
    reviewer = reviewers[0].get("reviewer")
    require(isinstance(reviewer, dict) and reviewer.get("id") == 15812269 and
            isinstance(reviewer.get("login"), str) and reviewer["login"].lower() == "effortlesssteven",
            "npm environment reviewer does not match the release maintainer")
    # GitHub's documented GET schema may omit this setting. Its absence is not
    # evidence of disabled bypass; the operator must verify the UI prerequisite.
    if "can_admins_bypass" in environment:
        require(environment["can_admins_bypass"] is False, "npm environment permits admin bypass")
    return dict(name="npm", id=environment["id"], branch="main", reviewer="EffortlessSteven",
                admin_bypass_api_verified="can_admins_bypass" in environment)


def admit_release(destination, identity, publisher_sha, operation):
    """Copy only admitted bytes for a separate OIDC job; never invoke npm here."""
    require(operation in ("admit_only", "stage"), "unsupported release operation")
    authority = authorize_release(identity, publisher_sha)
    bundles = []
    for artifact in authority["artifacts"]:
        data = github_api(f"actions/artifacts/{artifact['id']}/zip", binary=True)
        require(len(data) == artifact["size_in_bytes"], "artifact transfer size mismatch")
        bundles.append(read_qualification_zip(data, artifact["digest"]))
    source = {"sha": identity["source_sha"], "tree": identity["source_tree"]}
    pin, native = inspect_native_evidence(bundles[4], source, identity["run_id"], identity["run_attempt"], authority["artifacts"][4])
    require(pin["native_version"] == identity["version"], "release/native version mismatch")
    provenance = dict(schema_version=1, product_source=pin, packaging_source=source,
                      sbom_sha256=digest(native["sbom.cyclonedx.json"]),
                      native_build_repeated=False, automatic_npm_oidc_provenance=False)
    prepared = bundles[0]
    filename = f"effortlessmetrics-ripr-{identity['version']}.tgz"
    source_files = {"npm_consumer.py": (ROOT / ".github/scripts/npm_consumer.py").read_bytes()}
    source_files.update({"fixture/" + path.relative_to(ROOT / "fixtures/python/basic").as_posix(): path.read_bytes()
                         for path in (ROOT / "fixtures/python/basic").rglob("*") if path.is_file()})
    require(set(prepared) == {"package-receipt.json", "npm-pack.json", "tarballs/" + filename, *source_files}, "prepared artifact inventory mismatch")
    require(all(prepared[name] == data for name, data in source_files.items()), "qualification harness/fixture differs from trusted source")
    receipt = json.loads(prepared["package-receipt.json"])
    expected = dict(schema_version=1, package_name=PACKAGE_NAME, version=identity["version"],
                    provenance=provenance, qualification_run_id=identity["run_id"],
                    qualification_run_attempt=identity["run_attempt"],
                    qualification_state="prepared_only", publication_attempted=False)
    for key, value in expected.items():
        require(type(receipt.get(key)) is type(value) and receipt[key] == value, f"prepared receipt {key} mismatch")
    tar_data = prepared["tarballs/" + filename]
    require(digest(tar_data) == identity["tarball_sha256"], "authorized tarball digest mismatch")
    with tempfile.TemporaryDirectory(prefix="ripr-npm-admit-") as temporary:
        tar_path = Path(temporary) / filename; tar_path.write_bytes(tar_data)
        inspected = validate_tarball(tar_path, pin, provenance)
    require(receipt.get("tarball") == inspected, "prepared tarball receipt mismatch")
    proofs = [validate_consumer_proof(files, node, npm, inspected["sha256"], pin["payload_sha256"])
              for files, (node, npm) in zip(bundles[1:3], CLIENTS, strict=True)]
    validate_stage_transport(bundles[3], inspected["sha256"], inspected["integrity"], identity["version"])
    eligible = stage_eligibility(public_package(), identity["version"])
    require(operation != "stage" or eligible, "version already publicly exists; do not stage or rebuild it")
    environment = None
    if operation == "stage":
        environment = validate_stage_environment(github_api("environments/npm"),
                                                github_api("environments/npm/deployment-branch-policies?per_page=100"))
    require(authorize_release(identity, publisher_sha) == authority, "qualification authority changed during admission")
    destination.mkdir(parents=True, exist_ok=False)
    (destination / "tarballs").mkdir()
    (destination / "tarballs" / filename).write_bytes(tar_data)
    stage_eligible = eligible and operation == "stage"
    report = dict(schema_version=1, state="admitted_not_staged", stage_eligible=stage_eligible,
                  version_available=eligible,
                  operation=operation, package=PACKAGE_NAME, version=identity["version"],
                  environment=environment,
                  authority=authority, tarball=inspected, native_sha256=pin["payload_sha256"],
                  consumer_receipts=[digest(files["consumer.json"]) for files in bundles[1:3]],
                  stage_transport_receipt=digest(bundles[3]["stage-cli-proof.json"]),
                  selected_routes=sum(proof["selected_routes"] for proof in proofs),
                  executed_routes=sum(proof["executed_routes"] for proof in proofs),
                  failed_routes=sum(proof["failed_routes"] for proof in proofs))
    (destination / "release-admission.json").write_bytes(canonical(report))
    if os.environ.get("GITHUB_OUTPUT"):
        with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as output:
            output.write(f"stage_eligible={str(stage_eligible).lower()}\nfilename={filename}\nintegrity={inspected['integrity']}\n")
    print(canonical(report).decode())
    return report


def prepare(destination):
    require(not destination.exists(), "destination already exists")
    # A real release candidate is committed; untracked output under target is fine.
    require(not run(["git", "diff", "--name-only", "HEAD"]), "tracked source is dirty")
    for required in (".github/scripts/npm_package.py", "packaging/python/pyproject.toml", "packaging/npm/package.template.json", "packaging/npm/README.md"):
        run(["git", "ls-files", "--error-unmatch", required])
    source = {"sha": run(["git", "rev-parse", "HEAD"]), "tree": run(["git", "rev-parse", "HEAD^{tree}"])}
    pin, files = prepare_native_input(source, os.environ.get("GITHUB_RUN_ID"), os.environ.get("GITHUB_RUN_ATTEMPT"))
    provenance = {"schema_version": 1, "product_source": pin, "packaging_source": source,
                  "sbom_sha256": digest(files["sbom.cyclonedx.json"]), "native_build_repeated": False,
                  "automatic_npm_oidc_provenance": False}
    files.update({"package.json": canonical(manifest(pin)), "README.md": (ROOT / "packaging/npm/README.md").read_bytes(), "provenance.json": canonical(provenance)})
    package = destination / "package"
    package.mkdir(parents=True)
    for name, data in files.items():
        target = package / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
        target.chmod(0o755 if name == "bin/ripr" else 0o644)
    output = destination / "tarballs"
    output.mkdir()
    packed = json.loads(run(["npm", "pack", "--ignore-scripts", "--json", "--pack-destination", str(output.resolve())], package))
    expected_filename = f"{PACKAGE_NAME.removeprefix('@').replace('/', '-')}-{pin['native_version']}.tgz"
    require(len(packed) == 1 and packed[0]["filename"] == expected_filename, "unexpected npm pack result")
    require({row["path"] for row in packed[0]["files"]} == FILES, "npm pack inventory differs")
    result = validate_tarball(output / packed[0]["filename"], pin, provenance)
    receipt = {"schema_version": 1, "version": pin["native_version"], "package_name": PACKAGE_NAME, "provenance": provenance, "tarball": result,
               "qualification_run_id": os.environ.get("GITHUB_RUN_ID"), "qualification_run_attempt": os.environ.get("GITHUB_RUN_ATTEMPT"),
               "qualification_state": "prepared_only", "publication_attempted": False}
    (destination / "package-receipt.json").write_bytes(canonical(receipt))
    (destination / "npm-pack.json").write_bytes(canonical(packed))
    shutil.copytree(ROOT / "fixtures/python/basic", destination / "fixture")
    print(json.dumps(receipt, indent=2))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("destination", type=Path)
    parser.add_argument("--admit-release", action="store_true")
    parser.add_argument("--inspect-staging-response", type=Path)
    parser.add_argument("--operation", choices=("admit_only", "stage"), default="admit_only")
    args = parser.parse_args()
    if args.admit_release or args.inspect_staging_response:
        require(not (args.admit_release and args.inspect_staging_response), "choose one release operation")
        identity = {key: os.environ[key.upper()] for key in
                    ("run_id", "run_attempt", "source_sha", "source_tree", "version", "tarball_sha256")}
        if args.admit_release:
            admit_release(args.destination.resolve(), identity, os.environ["GITHUB_SHA"], args.operation)
        else:
            result = inspect_staging_response(args.inspect_staging_response, identity, os.environ["EXPECTED_INTEGRITY"])
            require(not args.destination.exists(), "staging receipt already exists")
            args.destination.write_bytes(canonical(result))
            print(canonical(result).decode())
    else:
        require(args.operation == "admit_only", "preparation cannot stage")
        prepare(args.destination.resolve())


if __name__ == "__main__":
    main()
