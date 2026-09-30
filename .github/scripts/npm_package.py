"""Prepare/inspect a Linux npm tarball from a pinned, published native wheel.

No registry writes or credentials. Product and packaging source identities are
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
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parents[2]
PACKAGE_NAME = "@effortlessmetrics/ripr"
FILES = {"package.json", "README.md", "LICENSE-MIT", "LICENSE-APACHE", "bin/ripr", "provenance.json", "sbom.cyclonedx.json"}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def canonical(value):
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def run(args, cwd=ROOT):
    return subprocess.check_output(args, cwd=cwd, text=True, timeout=120).strip()


def source_pin(root=ROOT):
    pin = json.loads((root / "packaging/npm/native-source.json").read_text())
    require(pin.get("schema_version") == 1, "unsupported native pin schema")
    require(pin.get("repository") == "EffortlessMetrics/ripr", "wrong native repository")
    for key in ("product_source_sha", "product_source_tree"):
        require(re.fullmatch(r"[0-9a-f]{40}", pin.get(key, "")), f"invalid {key}")
    for key in ("cargo_lock_sha256", "wheel_sha256", "payload_sha256"):
        require(re.fullmatch(r"[0-9a-f]{64}", pin.get(key, "")), f"invalid {key}")
    require(re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+-(alpha|beta|rc)\.[0-9]+", pin.get("native_version", "")), "prerelease version required")
    require(pin.get("target") == "x86_64-unknown-linux-gnu" and pin.get("minimum_glibc") == "2.34", "unsupported native target")
    version_match = re.search(r'\[workspace.package\][\s\S]*?^version = "([^"]+)"', (root / "Cargo.toml").read_text(), re.MULTILINE)
    require(version_match is not None and version_match[1] == pin["native_version"], "workspace/native version drift; qualify a new native release")
    require(pin["wheel_filename"] == f"ripr_rs-{pin['python_version']}-py3-none-manylinux_2_34_x86_64.whl", "wrong wheel filename")
    require(re.fullmatch(r"https://files\.pythonhosted\.org/packages/[0-9a-f/]+/" + re.escape(pin["wheel_filename"]), pin["wheel_url"]), "unexpected wheel origin")
    return pin


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
    for key in ("scripts", "dependencies", "optionalDependencies", "devDependencies", "bundledDependencies"):
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
        require(json.loads(data["provenance.json"]) == provenance, "npm provenance mismatch")
        require(data["provenance.json"] == canonical(provenance), "noncanonical npm provenance")
        require(digest(data["bin/ripr"]) == pin["payload_sha256"], "npm executable digest mismatch")
        require(archive.getmember("package/bin/ripr").mode & 0o111, "npm executable mode missing")
        for name in ("LICENSE-MIT", "LICENSE-APACHE"):
            require(data[name] == (root / name).read_bytes(), "npm license mismatch")
        require(data["README.md"] == (root / "packaging/npm/README.md").read_bytes(), "npm readme mismatch")
        require(digest(data["sbom.cyclonedx.json"]) == provenance["sbom_sha256"], "npm SBOM mismatch")
    return {"filename": path.name, "sha256": digest(path.read_bytes()), "integrity": "sha512-" + base64.b64encode(hashlib.sha512(path.read_bytes()).digest()).decode(), "files": sorted(FILES)}


def prepare(destination, wheel=None):
    pin = source_pin()
    require(not destination.exists(), "destination already exists")
    # A real release candidate is committed; untracked output under target is fine.
    require(not run(["git", "diff", "--name-only", "HEAD"]), "tracked source is dirty")
    for required in (".github/scripts/npm_package.py", "packaging/npm/native-source.json", "packaging/npm/package.template.json", "packaging/npm/README.md"):
        run(["git", "ls-files", "--error-unmatch", required])
    source = {"sha": run(["git", "rev-parse", "HEAD"]), "tree": run(["git", "rev-parse", "HEAD^{tree}"])}
    if wheel:
        require(wheel.is_file() and not wheel.is_symlink(), "invalid input wheel")
        wheel_data = wheel.read_bytes()
    else:
        with urllib.request.urlopen(pin["wheel_url"], timeout=60) as response:
            require(response.url == pin["wheel_url"], "wheel redirected")
            wheel_data = response.read(30_000_001)
        require(len(wheel_data) <= 30_000_000, "oversized wheel download")
    files = inspect_wheel(wheel_data, pin)
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
    parser.add_argument("--wheel", type=Path)
    args = parser.parse_args()
    prepare(args.destination.resolve(), args.wheel)


if __name__ == "__main__":
    main()
