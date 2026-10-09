"""Discriminating controls for the real npm byte-admission functions."""
import base64
import copy
import csv
import hashlib
import importlib.util
import io
import json
import http.server
import os
import shutil
import subprocess
from pathlib import Path
import tarfile
import tempfile
import threading
import textwrap
import unittest
from unittest import mock
from types import SimpleNamespace
import zipfile

SPEC = importlib.util.spec_from_file_location("npm_package", Path(__file__).with_name("npm_package.py"))
PACKAGE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PACKAGE)
CONSUMER_SPEC = importlib.util.spec_from_file_location("npm_consumer", Path(__file__).with_name("npm_consumer.py"))
CONSUMER = importlib.util.module_from_spec(CONSUMER_SPEC)
CONSUMER_SPEC.loader.exec_module(CONSUMER)


def wheel_fixture():
    pin = PACKAGE.source_contract()
    prefix = f"ripr_rs-{pin['python_version']}"
    info = prefix + ".dist-info"
    binary = bytearray(64)
    binary[:6] = b"\x7fELF\x02\x01"
    binary[18:20] = (62).to_bytes(2, "little")
    pin["payload_sha256"] = PACKAGE.digest(binary)
    pin["wheel_sha256"] = "0" * 64
    files = {
        f"{prefix}.data/scripts/ripr": bytes(binary),
        f"{info}/METADATA": f"Name: ripr-rs\nVersion: {pin['python_version']}\n".encode(),
        f"{info}/WHEEL": b"Wheel-Version: 1.0\nTag: py3-none-manylinux_2_34_x86_64\n",
        f"{info}/sboms/ripr.cyclonedx.json": b'{"bomFormat":"CycloneDX"}',
    }
    for name in ("LICENSE-MIT", "LICENSE-APACHE"):
        files[f"{info}/licenses/{name}"] = (PACKAGE.ROOT / name).read_bytes()
    record = io.StringIO()
    writer = csv.writer(record)
    for name, data in files.items():
        writer.writerow([name, "sha256=" + base64.urlsafe_b64encode(bytes.fromhex(PACKAGE.digest(data))).rstrip(b"=").decode(), len(data)])
    writer.writerow([f"{info}/RECORD", "", ""])
    files[f"{info}/RECORD"] = record.getvalue().encode()
    return pin, files


def zip_bytes(files, link=None, no_exec=False):
    out = io.BytesIO()
    with zipfile.ZipFile(out, "w") as archive:
        for name, data in files.items():
            entry = zipfile.ZipInfo(name)
            # Preserve the wire name: ZipInfo normalizes backslashes on Windows
            # and truncates NUL suffixes before a malformed fixture is written.
            entry.filename = name
            logical_name = name.split("\x00", 1)[0].replace("\\", "/")
            entry.external_attr = (0o100755 if logical_name.endswith("/ripr") and not no_exec else 0o100644) << 16
            if name == link:
                entry.external_attr = 0o120755 << 16
            archive.writestr(entry, data)
    return out.getvalue()


def native_evidence_fixture():
    pin, members = wheel_fixture()
    wheel = zip_bytes(members)
    pin["wheel_sha256"] = PACKAGE.digest(wheel)
    source = {"sha": "a" * 40, "tree": "b" * 40}
    filename = pin["wheel_filename"]
    qualification = dict(schema_version=1, source_sha=source["sha"], source_tree=source["tree"],
                         version=pin["python_version"], native_version=pin["native_version"],
                         wheel_filename=filename, wheel_sha256=pin["wheel_sha256"],
                         repository=PACKAGE.REPOSITORY, run_id=123, run_attempt=2,
                         target=pin["target"], platform_tag="py3-none-manylinux_2_34_x86_64")
    receipt = dict(schema_version=2, channel="pypi-source-wheelhouse", source_repository=PACKAGE.REPOSITORY,
                   producer_run_id="123", producer_run_attempt="2", publication_attempted=False,
                   candidate_sha=source["sha"], candidate_tree=source["tree"],
                   cargo_lock_sha256=PACKAGE.digest((PACKAGE.ROOT / "Cargo.lock").read_bytes()),
                   native_version=pin["native_version"], python_version=pin["python_version"],
                   distribution="ripr-rs", executable="ripr", rust_target=pin["target"],
                   compatibility_state="auditwheel_manylinux_2_34", native_execution="ubuntu-22.04-x64",
                   scope="linux-x64-glibc-only", features=["lang-python", "lang-rust", "lang-typescript"],
                   wheel=dict(filename=filename, sha256=pin["wheel_sha256"], tag=qualification["platform_tag"],
                              installed_payload_sha256=pin["payload_sha256"]),
                   toolchain=dict(rustc="rustc 1.95.0 (fixture)\nhost: x86_64-unknown-linux-gnu\n"))
    files = {"qualification.json": PACKAGE.canonical(qualification),
             "wheel-receipt.json": PACKAGE.canonical(receipt), "wheelhouse/" + filename: wheel}
    archive = zip_bytes(files)
    artifact = dict(id=404, name="npm-native-wheel-123-2", digest="sha256:" + PACKAGE.digest(archive), size_in_bytes=len(archive))
    return source, files, artifact


class NpmNativeEvidenceTests(unittest.TestCase):
    def test_native_preparation_requires_same_run_completed_python_consumers_and_exact_zip(self):
        source, files, artifact = native_evidence_fixture()
        data = zip_bytes(files)
        authority = NpmReleaseAdmissionTests()
        authority.setUp()
        run_info = dict(authority.run, event="pull_request", head_branch="rehearsal", status="in_progress", conclusion=None)
        row = dict(artifact, expired=False, workflow_run=dict(id=123, head_sha=source["sha"], repository_id=88, head_repository_id=88))
        listing = {"total_count": 1, "artifacts": [row]}
        for control in ("valid", "stale-attempt", "wrong-source", "foreign-repo", "missing-pip",
                        "skipped-uv", "duplicate-build", "wrong-zip", "wrong-size",
                        "missing-jobs", "null-jobs", "invalid-job", "missing-artifacts", "null-artifacts", "invalid-artifact"):
            run = copy.deepcopy(run_info); jobs = copy.deepcopy(authority.jobs); selected = copy.deepcopy(listing)
            archive = data
            if control == "stale-attempt": run["run_attempt"] = 1
            if control == "wrong-source": run["head_sha"] = "e" * 40
            if control == "foreign-repo": run["head_repository"]["full_name"] = "foreign/ripr"
            if control == "missing-pip":
                jobs["jobs"] = [j for j in jobs["jobs"] if "clean pip" not in j["name"]]
                jobs["total_count"] -= 1
            if control == "skipped-uv":
                next(j for j in jobs["jobs"] if "clean uv" in j["name"])["conclusion"] = "skipped"
            if control == "duplicate-build":
                jobs["jobs"].append(next(j for j in jobs["jobs"] if "build and inspect" in j["name"]))
                jobs["total_count"] += 1
            if control == "missing-jobs": jobs = {"total_count": 0}
            if control == "null-jobs": jobs = {"total_count": 0, "jobs": None}
            if control == "invalid-job": jobs = {"total_count": 1, "jobs": [None]}
            if control == "missing-artifacts": selected = {"total_count": 0}
            if control == "null-artifacts": selected = {"total_count": 0, "artifacts": None}
            if control == "invalid-artifact": selected = {"total_count": 1, "artifacts": [None]}
            if control == "wrong-zip": archive = data[:-1] + bytes([data[-1] ^ 1])
            if control == "wrong-size": selected["artifacts"][0]["size_in_bytes"] += 1
            with self.subTest(control=control), mock.patch.object(PACKAGE, "github_api", side_effect=[run, jobs, selected, archive]):
                if control == "valid":
                    pin, native = PACKAGE.prepare_native_input(source, "123", "2")
                    self.assertEqual(pin["native_artifact"], artifact)
                    self.assertEqual(PACKAGE.digest(native["bin/ripr"]), pin["payload_sha256"])
                else:
                    with self.assertRaises(ValueError):
                        PACKAGE.prepare_native_input(source, "123", "2")

    def test_fresh_same_source_wheel_derives_native_pin_without_publication(self):
        source, files, artifact = native_evidence_fixture()
        pin, native = PACKAGE.inspect_native_evidence(files, source, "123", "2", artifact)
        self.assertEqual(pin["origin"], "same-run-qualified-wheel")
        self.assertEqual(pin["product_source_sha"], source["sha"])
        self.assertEqual(pin["native_artifact"], artifact)
        self.assertNotIn("wheel_url", pin)
        self.assertEqual(PACKAGE.digest(native["bin/ripr"]), pin["payload_sha256"])

    def test_native_receipt_source_attempt_version_toolchain_and_inventory_mismatches_fail(self):
        source, files, artifact = native_evidence_fixture()
        cases = []
        for name, changes in {
            "qualification.json": {"source_sha": "e" * 40, "source_tree": "e" * 40, "run_id": 999,
                                   "run_attempt": 1, "version": "0.11.0a99", "native_version": "0.11.0-alpha.99",
                                   "wheel_sha256": "e" * 64, "target": "aarch64-unknown-linux-gnu"},
            "wheel-receipt.json": {"candidate_sha": "e" * 40, "producer_run_attempt": "1",
                                   "cargo_lock_sha256": "e" * 64, "publication_attempted": True,
                                   "features": ["lang-rust"], "toolchain": {"rustc": "rustc 1.94.0 (wrong)"}},
        }.items():
            for key, value in changes.items():
                changed = dict(files)
                changed[name] = PACKAGE.canonical(dict(json.loads(files[name]), **{key: value}))
                cases.append((name + ":" + key, changed))
        cases.extend([("missing-receipt", {k: v for k, v in files.items() if k != "qualification.json"}),
                      ("extra-file", dict(files, extra=b"not admitted"))])
        for label, changed in cases:
            with self.subTest(label=label), self.assertRaises(ValueError):
                PACKAGE.inspect_native_evidence(changed, source, "123", "2", artifact)


class NpmPackageTests(unittest.TestCase):
    def test_consumer_rejects_missing_unscoped_and_foreign_package_identity(self):
        for name in (None, "ripr", "@other/ripr"):
            with self.subTest(name=name), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                artifact = root / "artifact"
                (artifact / "tarballs").mkdir(parents=True)
                (artifact / "fixture").mkdir()
                data = b"valid transfer digest before consumer execution"
                (artifact / "tarballs/input.tgz").write_bytes(data)
                receipt = {"package_name": name, "version": "0.11.0-alpha.1", "provenance": {"product_source": {"native_version": "0.11.0-alpha.1", "payload_sha256": PACKAGE.digest(data)}}, "tarball": {"filename": "input.tgz", "sha256": PACKAGE.digest(data)}}
                (artifact / "package-receipt.json").write_bytes(PACKAGE.canonical(receipt))
                args = SimpleNamespace(artifact=artifact, output=root / "output")
                with mock.patch.object(CONSUMER.argparse.ArgumentParser, "parse_args", return_value=args), mock.patch.object(CONSUMER, "command", side_effect=AssertionError("wrong package reached execution")):
                    with self.assertRaisesRegex(ValueError, "receipt package name mismatch"):
                        CONSUMER.main()

    def test_missing_node_or_npm_has_actionable_error(self):
        for missing in ("node", "npm", "npx"):
            with self.subTest(missing=missing), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                artifact = root / "artifact"
                (artifact / "tarballs").mkdir(parents=True)
                data = b"input identity is checked before tool discovery"
                (artifact / "tarballs/input.tgz").write_bytes(data)
                receipt = {"package_name": "@effortlessmetrics/ripr", "version": "0.11.0-alpha.1", "provenance": {"product_source": {"native_version": "0.11.0-alpha.1"}}, "tarball": {"filename": "input.tgz", "sha256": PACKAGE.digest(data)}}
                (artifact / "package-receipt.json").write_bytes(PACKAGE.canonical(receipt))
                args = SimpleNamespace(artifact=artifact, output=root / "output")
                real_which = CONSUMER.shutil.which
                with mock.patch.object(CONSUMER.argparse.ArgumentParser, "parse_args", return_value=args), mock.patch.object(CONSUMER.shutil, "which", side_effect=lambda name, **kwargs: None if name == missing else real_which(name, **kwargs)), mock.patch.object(CONSUMER, "command", side_effect=AssertionError("tool discovery did not fail before execution")):
                    with self.assertRaisesRegex(ValueError, missing + " executable not found in PATH"):
                        CONSUMER.main()

    @unittest.skipUnless(os.name == "posix", "Linux-native consumer uses POSIX executable links")
    def test_reinstall_rejects_noop_uninstall_and_noop_install(self):
        data = b"expected native payload"
        for control in ("noop-uninstall", "dangling-bin", "noop-install", "stale-unscoped", "wrong-bin-owner", "fresh-install"):
            with self.subTest(control=control), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                prefix = root / "global"
                package = prefix / "lib/node_modules/@effortlessmetrics/ripr"
                binary = prefix / "bin/ripr"
                unscoped = prefix / "lib/node_modules/ripr/bin/ripr"
                unscoped.parent.mkdir(parents=True)
                unscoped.write_bytes(data)
                def install():
                    (package / "bin").mkdir(parents=True)
                    (package / "bin/ripr").write_bytes(data)
                    binary.parent.mkdir(parents=True, exist_ok=True)
                    binary.symlink_to(package / "bin/ripr")
                install()
                observed = []
                def operation(args, cwd, env):
                    if "uninstall" in args:
                        self.assertEqual(args[-1], "@effortlessmetrics/ripr")
                        observed.append("uninstall")
                        if control != "noop-uninstall":
                            binary.unlink()
                            shutil.rmtree(package)
                            if control == "dangling-bin":
                                binary.symlink_to(root / "absent-native")
                    else:
                        observed.append("install")
                        if control == "fresh-install":
                            install()
                        elif control == "stale-unscoped":
                            package.mkdir(parents=True)
                            binary.symlink_to(unscoped)
                        elif control == "wrong-bin-owner":
                            install()
                            binary.unlink()
                            binary.symlink_to(unscoped)
                    return subprocess.CompletedProcess(args, 0, "", "")
                with mock.patch.object(CONSUMER, "command", side_effect=operation):
                    if control == "fresh-install":
                        CONSUMER.reinstall_global(["npm"], prefix, root / "input.tgz", root, {}, PACKAGE.digest(data))
                        self.assertEqual(observed, ["uninstall", "install"])
                        self.assertEqual(PACKAGE.digest(binary.read_bytes()), PACKAGE.digest(data))
                        self.assertEqual(unscoped.read_bytes(), data)
                    else:
                        message = "uninstall did not remove" if control in ("noop-uninstall", "dangling-bin") else "reinstall did not restore"
                        if control == "wrong-bin-owner":
                            message = "reinstall bin link targets wrong package"
                        with self.assertRaisesRegex(ValueError, message):
                            CONSUMER.reinstall_global(["npm"], prefix, root / "input.tgz", root, {}, PACKAGE.digest(data))
                        self.assertEqual(observed, ["uninstall"] if control in ("noop-uninstall", "dangling-bin") else ["uninstall", "install"])

    @unittest.skipUnless(os.name == "posix", "Linux-native consumer uses POSIX executable links")
    def test_public_npx_broken_wrapper_is_executed_and_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            artifact = root / "artifact"
            (artifact / "tarballs").mkdir(parents=True)
            (artifact / "fixture").mkdir()
            data = b"fixture input"
            (artifact / "tarballs/input.tgz").write_bytes(data)
            receipt = {"package_name": "@effortlessmetrics/ripr", "version": "0.11.0-alpha.1", "provenance": {"product_source": {"native_version": "0.11.0-alpha.1", "payload_sha256": PACKAGE.digest(data)}}, "tarball": {"filename": "input.tgz", "sha256": PACKAGE.digest(data)}}
            (artifact / "package-receipt.json").write_bytes(PACKAGE.canonical(receipt))
            tools = root / "tools"; tools.mkdir()
            (tools / "node").write_text('#!/bin/sh\nif [ "$1" = "--version" ]; then echo v24.19.0; else echo 11.9.0; fi\n')
            (tools / "npm").write_text('#!/bin/sh\necho 11.9.0\n')
            (tools / "npx").write_text('#!/bin/sh\necho broken-public-npx >&2\nexit 93\n')
            for tool in tools.iterdir():
                tool.chmod(0o755)
            args = SimpleNamespace(artifact=artifact, output=root / "output")
            with mock.patch.object(CONSUMER.argparse.ArgumentParser, "parse_args", return_value=args), mock.patch.dict(os.environ, {"PATH": str(tools)}):
                with self.assertRaisesRegex(ValueError, r"command failed \(93\).*npx"):
                    CONSUMER.main()

    def test_manifest_is_single_native_prerelease(self):
        pin = PACKAGE.source_contract()
        value = PACKAGE.manifest(pin)
        self.assertEqual(value["version"], pin["native_version"])
        self.assertEqual(value["name"], "@effortlessmetrics/ripr")
        self.assertNotIn("scripts", value)
        self.assertEqual(value["bin"], {"ripr": "bin/ripr"})

    def test_rejects_manifest_identity_platform_lifecycle_and_dependency_drift(self):
        pin = PACKAGE.source_contract()
        for key, value in {
            "name": "ripr", "version": "0.11.0", "bin": {"ripr": "download.js"},
            "os": ["darwin"], "cpu": ["arm64"], "libc": ["musl"],
            "publishConfig": {"access": "public", "tag": "latest"},
            "scripts": {"postinstall": "false"}, "dependencies": {}, "optionalDependencies": {},
            "bundleDependencies": [], "tag": "latest",
            "repository": {"url": "https://example.invalid"}, "files": ["*"],
        }.items():
            with self.subTest(key=key):
                changed = PACKAGE.manifest(pin)
                changed[key] = value
                with self.assertRaises(ValueError):
                    PACKAGE.validate_manifest(changed, pin)

    def test_exact_wheel_extracts_only_native_notices_and_sbom(self):
        pin, files = wheel_fixture()
        data = zip_bytes(files)
        pin["wheel_sha256"] = PACKAGE.digest(data)
        accepted = PACKAGE.inspect_wheel(data, pin)
        self.assertEqual(set(accepted), {"bin/ripr", "LICENSE-MIT", "LICENSE-APACHE", "sbom.cyclonedx.json"})
        self.assertEqual(PACKAGE.digest(accepted["bin/ripr"]), pin["payload_sha256"])

    def test_wheel_wrong_hash_rejected_before_archive_parsing(self):
        pin, _ = wheel_fixture()
        with self.assertRaisesRegex(ValueError, "wheel digest"):
            PACKAGE.inspect_wheel(b"not-a-wheel", pin)

    def test_missing_payload_stale_record_wrong_platform_and_symlink_rejected(self):
        pin, original = wheel_fixture()
        native = next(name for name in original if name.endswith("/scripts/ripr"))
        metadata = next(name for name in original if name.endswith("/METADATA"))
        wheel = next(name for name in original if name.endswith("/WHEEL"))
        cases = []
        missing = dict(original); del missing[native]; cases.append((missing, {}))
        stale = dict(original); stale[native] += b"changed"; cases.append((stale, {}))
        wrong_name = dict(original); wrong_name[metadata] = b"Name: ripr\nVersion: 0.11.0a1\n"; cases.append((wrong_name, {}))
        wrong_platform = dict(original); wrong_platform[wheel] = b"Tag: py3-none-any\n"; cases.append((wrong_platform, {}))
        traversal = dict(original); traversal["../escape"] = b"bad"; cases.append((traversal, {}))
        cases.extend([(original, {"link": native}), (original, {"no_exec": True})])
        for files, options in cases:
            with self.subTest(options=options, names=len(files)):
                data = zip_bytes(files, **options)
                changed_pin = {**pin, "wheel_sha256": PACKAGE.digest(data)}
                with self.assertRaises(ValueError):
                    PACKAGE.inspect_wheel(data, changed_pin)

    def test_native_elf_architecture_and_payload_identity_are_checked(self):
        pin, files = wheel_fixture()
        data = zip_bytes(files)
        pin["wheel_sha256"] = PACKAGE.digest(data)
        pin["payload_sha256"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "payload digest"):
            PACKAGE.inspect_wheel(data, pin)

    def test_wrong_elf_architecture_is_rejected_after_hash_and_record_agree(self):
        pin, files = wheel_fixture()
        native = next(name for name in files if name.endswith("/scripts/ripr"))
        binary = bytearray(files[native])
        binary[18:20] = (183).to_bytes(2, "little")
        files[native] = bytes(binary)
        pin["payload_sha256"] = PACKAGE.digest(binary)
        record_name = next(name for name in files if name.endswith("/RECORD"))
        output = io.StringIO()
        writer = csv.writer(output)
        for name, body in files.items():
            if name == record_name:
                writer.writerow([name, "", ""])
            else:
                encoded = base64.urlsafe_b64encode(bytes.fromhex(PACKAGE.digest(body))).rstrip(b"=").decode()
                writer.writerow([name, "sha256=" + encoded, len(body)])
        files[record_name] = output.getvalue().encode()
        data = zip_bytes(files)
        pin["wheel_sha256"] = PACKAGE.digest(data)
        with self.assertRaisesRegex(ValueError, "not Linux x86-64 ELF"):
            PACKAGE.inspect_wheel(data, pin)

    def test_tarball_negative_controls_reject_missing_mode_changed_bytes_and_provenance(self):
        pin, original = wheel_fixture()
        data = zip_bytes(original); pin["wheel_sha256"] = PACKAGE.digest(data)
        files = PACKAGE.inspect_wheel(data, pin)
        provenance = {"sbom_sha256": PACKAGE.digest(files["sbom.cyclonedx.json"])}
        files.update({"package.json": PACKAGE.canonical(PACKAGE.manifest(pin)), "provenance.json": PACKAGE.canonical(provenance), "README.md": (PACKAGE.ROOT / "packaging/npm/README.md").read_bytes()})
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for mutation in (None, "missing", "mode", "payload", "provenance", "symlink", "duplicate", "traversal"):
                with self.subTest(mutation=mutation):
                    path = root / "ripr.tgz"
                    changed = copy.deepcopy(files)
                    if mutation == "missing": del changed["bin/ripr"]
                    if mutation == "payload": changed["bin/ripr"] += b"changed"
                    if mutation == "provenance": changed["provenance.json"] = b"{}"
                    if mutation == "traversal": changed["../escape"] = b"bad"
                    with tarfile.open(path, "w:gz") as archive:
                        for name, body in changed.items():
                            member = tarfile.TarInfo("package/" + name)
                            member.size = len(body)
                            member.mode = 0o755 if name == "bin/ripr" and mutation != "mode" else 0o644
                            if mutation == "symlink" and name == "bin/ripr": member.type = tarfile.SYMTYPE; member.linkname = "/tmp/foreign"
                            archive.addfile(member, io.BytesIO(body))
                            if mutation == "duplicate" and name == "bin/ripr": archive.addfile(member, io.BytesIO(body))
                    if mutation is None:
                        self.assertEqual(PACKAGE.validate_tarball(path, pin, provenance)["files"], sorted(PACKAGE.FILES))
                    else:
                        with self.assertRaises(ValueError): PACKAGE.validate_tarball(path, pin, provenance)


class NpmReleaseAdmissionTests(unittest.TestCase):
    def setUp(self):
        self.identity = dict(run_id="123", run_attempt="2", source_sha="a" * 40,
                             source_tree="b" * 40, version=PACKAGE.source_contract()["native_version"], tarball_sha256="c" * 64)
        self.ref = {"ref": "refs/heads/main", "object": {"type": "commit", "sha": "a" * 40}}
        self.run = dict(id=123, run_attempt=2, workflow_id=99, event="workflow_dispatch",
                        head_branch="main", head_sha="a" * 40, status="completed", conclusion="success",
                        path=".github/workflows/npm-package-qualification.yml",
                        repository={"full_name": "EffortlessMetrics/ripr", "id": 88},
                        head_repository={"full_name": "EffortlessMetrics/ripr", "id": 88})
        self.commit = {"sha": "a" * 40, "tree": {"sha": "b" * 40}}
        self.workflow = {"id": 99, "path": self.run["path"]}
        self.jobs = {"total_count": 7, "jobs": [dict(name=name, status="completed", conclusion="success")
                     for name in ("package", "consumer (20.20.1, 10.8.2)",
                                  "consumer (24.19.0, 11.9.0)", "publisher-controls", *sorted(PACKAGE.NATIVE_JOBS))]}
        self.artifacts = {"total_count": 5, "artifacts": [
            dict(id=index + 400, name=name, expired=False, digest="sha256:" + "d" * 64,
                 size_in_bytes=100, workflow_run={"id": 123, "head_sha": "a" * 40,
                                                 "repository_id": 88, "head_repository_id": 88})
            for index, name in enumerate(("npm-prepared-123-2", "npm-consumer-20.20.1-123-2",
                                          "npm-consumer-24.19.0-123-2", "npm-publisher-controls-123-2", "npm-native-wheel-123-2"))]}

    def authorize(self, **overrides):
        values = dict(ref=self.ref, run=self.run, commit=self.commit, workflow=self.workflow,
                      jobs=self.jobs, artifacts=self.artifacts)
        values.update(overrides)
        replies = [values[key] for key in ("ref", "run", "commit", "workflow", "jobs", "artifacts")]
        with mock.patch.object(PACKAGE, "github_api", side_effect=replies):
            return PACKAGE.authorize_release(self.identity, "a" * 40)

    def test_exact_main_run_and_all_attempt_bound_artifacts_are_admitted(self):
        result = self.authorize()
        self.assertEqual(result["identity"], self.identity)
        self.assertEqual([a["id"] for a in result["artifacts"]], [400, 401, 402, 403, 404])

    def test_old_four_job_or_reused_artifact_identity_cannot_authorize_fresh_native_release(self):
        old_jobs = dict(total_count=4, jobs=self.jobs["jobs"][:4])
        with self.assertRaises(ValueError):
            self.authorize(jobs=old_jobs)
        artifacts = copy.deepcopy(self.artifacts)
        artifacts["artifacts"][-1]["id"] = artifacts["artifacts"][0]["id"]
        with self.assertRaisesRegex(ValueError, "duplicate artifact ID"):
            self.authorize(artifacts=artifacts)

    def test_wrong_run_ref_attempt_job_and_artifact_authorities_fail(self):
        changes = [
            ("ref", {"ref": "refs/tags/main", "object": self.ref["object"]}),
            ("ref", {"ref": "refs/heads/main", "object": {"type": "commit", "sha": "e" * 40}}),
            ("commit", {"sha": "a" * 40, "tree": {"sha": "e" * 40}}),
            ("workflow", {"id": 100, "path": self.run["path"]}),
        ]
        for field, value in (("event", "pull_request"), ("head_branch", "feature"),
                             ("head_sha", "e" * 40), ("run_attempt", 1),
                             ("conclusion", "failure"), ("status", "in_progress"),
                             ("path", "other.yml"), ("repository", None), ("head_repository", None),
                             ("head_repository", {"full_name": "fork/ripr", "id": 88})):
            changes.append(("run", dict(self.run, **{field: value})))
        for index in range(7):
            jobs = copy.deepcopy(self.jobs); jobs["jobs"][index]["conclusion"] = "skipped"
            changes.append(("jobs", jobs))
        for field, value in (("name", "npm-prepared-123-1"), ("expired", True),
                             ("id", "400"), ("digest", None), ("size_in_bytes", 40_000_001),
                             ("workflow_run", None), ("workflow_run", {"id": 123, "head_sha": "e" * 40})):
            artifacts = copy.deepcopy(self.artifacts); artifacts["artifacts"][0][field] = value
            changes.append(("artifacts", artifacts))
        changes.extend([("artifacts", {"total_count": 101, "artifacts": []}),
                        ("artifacts", {"total_count": 0, "artifacts": []})])
        for field, value in changes:
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                self.authorize(**{field: value})

    def test_identity_rejects_shell_paths_stable_versions_and_missing_attempt(self):
        for field, value in (("run_id", "../1"), ("run_attempt", "0"), ("source_sha", "main"),
                             ("source_tree", "a\nx=y"), ("version", "0.11.0"),
                             ("version", "00.11.0-alpha.1"), ("tarball_sha256", "bad")):
            with self.subTest(field=field), self.assertRaises(ValueError):
                PACKAGE.validate_release_identity(dict(self.identity, **{field: value}))

    def test_zip_digest_traversal_links_duplicates_and_unbounded_entries_fail(self):
        data = zip_bytes({"consumer.json": b"{}"})
        self.assertEqual(PACKAGE.read_qualification_zip(data, "sha256:" + PACKAGE.digest(data)), {"consumer.json": b"{}"})
        with self.assertRaisesRegex(ValueError, "digest"):
            PACKAGE.read_qualification_zip(data, "sha256:" + "0" * 64)
        for name, linked in (("../outside", False), ("/absolute", False), ("a\\b", False),
                             ("a/./b", False), ("consumer.json", True)):
            data = zip_bytes({name: b"{}"}, link=name if linked else None)
            with self.subTest(name=name, linked=linked), self.assertRaises(ValueError):
                PACKAGE.read_qualification_zip(data, "sha256:" + PACKAGE.digest(data))
        output = io.BytesIO()
        with zipfile.ZipFile(output, "w") as archive:
            archive.writestr("same", b"one")
            with self.assertWarns(UserWarning): archive.writestr("same", b"two")
        for data in (output.getvalue(), zip_bytes({str(i): b"x" for i in range(51)}),
                     zip_bytes({"oversized": b"x" * 30_000_001})):
            with self.assertRaises(ValueError):
                PACKAGE.read_qualification_zip(data, "sha256:" + PACKAGE.digest(data))

    def test_artifact_zip_rejects_wire_names_normalized_by_zipinfo(self):
        for separator in ("/", "\\"):
            for name in ("a\\b", "consumer.json\x00suffix"):
                data = zip_bytes({name: b"{}"})
                with self.subTest(separator=separator, name=name):
                    # Exercise both reader behaviors on every host. Native
                    # Windows execution also reaches its real normalization.
                    with mock.patch.object(zipfile.os, "sep", separator):
                        with zipfile.ZipFile(io.BytesIO(data)) as archive:
                            self.assertEqual(archive.infolist()[0].orig_filename, name)
                        with self.assertRaises(ValueError):
                            PACKAGE.read_qualification_zip(data, "sha256:" + PACKAGE.digest(data))

    def test_wheel_zip_rejects_wire_alias_of_expected_member(self):
        pin, files = wheel_fixture()
        native = next(name for name in files if name.endswith("/scripts/ripr"))
        for separator in ("/", "\\"):
            for alias in (native.replace("/", "\\"), native + "\x00suffix"):
                malformed = dict(files)
                malformed[alias] = malformed.pop(native)
                data = zip_bytes(malformed)
                expected = dict(pin, wheel_sha256=PACKAGE.digest(data))
                with self.subTest(separator=separator, alias=alias):
                    with mock.patch.object(zipfile.os, "sep", separator):
                        with zipfile.ZipFile(io.BytesIO(data)) as archive:
                            entry = next(item for item in archive.infolist() if item.orig_filename == alias)
                            self.assertTrue(entry.external_attr >> 16 & 0o111)
                        with self.assertRaises(ValueError):
                            PACKAGE.inspect_wheel(data, expected)

    def proof(self):
        rows = [dict(route=route, probes=1, findings=1, follow_up_finding="probe:fixture")
                for route in ("local", "global", "npm-exec", "npx")]
        proof = dict(schema_version=1, state="passed", node="v20.20.1", npm="10.8.2", npx="10.8.2",
                     selected_routes=4, executed_routes=4, failed_routes=0,
                     tarball_sha256="c" * 64, payload_sha256="e" * 64, routes=rows,
                     reinstall_journey=dict(rows[0], route="fresh-reinstall"),
                     negative_controls=list(PACKAGE.CONSUMER_CONTROLS))
        files = {"consumer.json": PACKAGE.canonical(proof), "lsp-stderr.txt": b""}
        for row in rows + [proof["reinstall_journey"]]:
            files[row["route"] + "-check.json"] = PACKAGE.canonical({"summary": {"probes": 1, "findings": 1}, "findings": [{"id": "probe:fixture"}]})
            files[row["route"] + "-explain.txt"] = b"Useful explanation of probe:fixture"
        return proof, files

    def test_retained_explanations_require_the_selected_follow_up_finding(self):
        _, files = self.proof()
        for route in (*PACKAGE.ROUTES, "fresh-reinstall"):
            for explanation in (b"Useful explanation without a finding identity",
                                b"ERROR: unrelated finding probe:wrong was not found"):
                changed = dict(files, **{route + "-explain.txt": explanation})
                with self.subTest(route=route, explanation=explanation):
                    with self.assertRaisesRegex(ValueError, "follow-up finding absent from retained explanation"):
                        PACKAGE.validate_consumer_proof(changed, "20.20.1", "10.8.2", "c" * 64, "e" * 64)

    def test_consumer_receipts_require_executed_distinct_routes_and_retained_journeys(self):
        proof, files = self.proof()
        PACKAGE.validate_consumer_proof(files, "20.20.1", "10.8.2", "c" * 64, "e" * 64)
        for field, value in (("selected_routes", 0), ("executed_routes", 3), ("failed_routes", 1),
                             ("state", "prepared"), ("npm", "11.9.0"), ("npx", "wrong"),
                             ("tarball_sha256", "f" * 64), ("payload_sha256", "f" * 64),
                             ("routes", [proof["routes"][0]] * 4), ("reinstall_journey", {}),
                             ("negative_controls", [])):
            changed = dict(files, **{"consumer.json": PACKAGE.canonical(dict(proof, **{field: value}))})
            with self.subTest(field=field), self.assertRaises(ValueError):
                PACKAGE.validate_consumer_proof(changed, "20.20.1", "10.8.2", "c" * 64, "e" * 64)
        for name in ("npx-check.json", "fresh-reinstall-explain.txt"):
            with self.subTest(name=name), self.assertRaises(ValueError):
                PACKAGE.validate_consumer_proof(dict(files, **{name: b""}), "20.20.1", "10.8.2", "c" * 64, "e" * 64)

    def test_existing_public_version_is_not_stage_eligible(self):
        public = {"name": "@effortlessmetrics/ripr", "versions": {self.identity["version"]: {"name": "@effortlessmetrics/ripr", "version": self.identity["version"]}}}
        self.assertFalse(PACKAGE.stage_eligibility(public, self.identity["version"]))
        self.assertTrue(PACKAGE.stage_eligibility(public, "0.11.0-alpha.99"))
        for metadata in ({}, {"name": "ripr", "versions": {}},
                         {"name": "@effortlessmetrics/ripr", "versions": {}},
                         {"name": "@effortlessmetrics/ripr", "versions": {"0.0.0-stage": {}}}):
            with self.subTest(metadata=metadata), self.assertRaises(ValueError):
                PACKAGE.stage_eligibility(metadata, self.identity["version"])

    def test_staging_response_rejects_wrong_identity_digest_and_missing_stage_id(self):
        entry = dict(name=PACKAGE.PACKAGE_NAME, version=self.identity["version"],
                     id=PACKAGE.PACKAGE_NAME + "@" + self.identity["version"], integrity="sha512:fixture",
                     filename=f"effortlessmetrics-ripr-{self.identity['version']}.tgz", entryCount=7,
                     stageId="01234567-89ab-4cde-8fab-0123456789ab")
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "response.json"
            for key in (None, "name", "version", "id", "integrity", "filename", "entryCount", "stageId"):
                changed = dict(entry)
                if key: changed[key] = "wrong"
                path.write_bytes(PACKAGE.canonical({PACKAGE.PACKAGE_NAME: changed}))
                with self.subTest(key=key):
                    if key:
                        with self.assertRaises(ValueError): PACKAGE.inspect_staging_response(path, self.identity, "sha512:fixture")
                    else:
                        receipt = PACKAGE.inspect_staging_response(path, self.identity, "sha512:fixture")
                        self.assertEqual(receipt["state"], "staging_response_received")
                        self.assertFalse(receipt["staged_bytes_independently_verified"])
                        self.assertFalse(receipt["maintainer_approval_performed"])
                        self.assertFalse(receipt["public_delivery_verified"])

    def test_environment_requires_existing_main_branch_and_exact_reviewer(self):
        environment = dict(name="npm", id=17,
                           deployment_branch_policy=dict(protected_branches=False, custom_branch_policies=True),
                           protection_rules=[dict(type="required_reviewers", prevent_self_review=False,
                                                  reviewers=[dict(type="User", reviewer=dict(id=15812269, login="EffortlessSteven"))])])
        policies = dict(total_count=1, branch_policies=[dict(name="main", type="branch")])
        result = PACKAGE.validate_stage_environment(environment, policies)
        self.assertFalse(result["admin_bypass_api_verified"])
        for field, value in (("prevent_self_review", True), ("reviewers", []), ("reviewers", None),
                             ("reviewers", [dict(type="User", reviewer=None)]),
                             ("reviewers", [dict(type="User", reviewer=dict(id=15812269, login=None))]),
                             ("reviewers", [dict(type="User", reviewer=dict(id=1, login="other"))])):
            changed = copy.deepcopy(environment); changed["protection_rules"][0][field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                PACKAGE.validate_stage_environment(changed, policies)
        for field, value in (("name", "pypi"), ("id", None), ("protection_rules", []), ("protection_rules", None),
                             ("deployment_branch_policy", None), ("can_admins_bypass", True)):
            with self.subTest(field=field), self.assertRaises(ValueError):
                PACKAGE.validate_stage_environment(dict(environment, **{field: value}), policies)
        for invalid in (dict(total_count=0, branch_policies=[]),
                        dict(total_count=2, branch_policies=policies["branch_policies"] * 2),
                        dict(total_count=1, branch_policies=[dict(name="main", type="tag")]),
                        dict(total_count=1, branch_policies=[dict(name="*", type="branch")])):
            with self.subTest(policies=invalid), self.assertRaises(ValueError):
                PACKAGE.validate_stage_environment(environment, invalid)

    @unittest.skipUnless(os.name == "posix", "GitHub-hosted stage job uses bash")
    def test_actual_staging_confirmation_and_transferred_file_guards(self):
        workflow = (PACKAGE.ROOT / ".github/workflows/publish-npm.yml").read_text()
        step = workflow.split("      - name: Require exact explicit staging confirmation\n", 1)[1].split("      - ", 1)[0]
        guard = step.split("        run: ", 1)[1].strip()
        confirmation = "stage @effortlessmetrics/ripr 0.11.0-alpha.1 next " + "c" * 64
        for operation, text, allowed in (("admit_only", "", True), ("stage", "", False),
                                         ("stage", confirmation, True),
                                         ("stage", confirmation.replace("next", "latest"), False),
                                         ("stage", confirmation.replace("alpha.1", "alpha.2"), False)):
            result = subprocess.run(["bash", "-euo", "pipefail", "-c", guard],
                                    env=dict(os.environ, OPERATION=operation, CONFIRMATION=text,
                                             VERSION="0.11.0-alpha.1", TARBALL_SHA256="c" * 64), capture_output=True, check=False)
            self.assertEqual(result.returncode == 0, allowed)
        step = workflow.split("      - name: Recheck the only staged file after transfer\n", 1)[1].split("      - ", 1)[0]
        guard = textwrap.dedent(step.split("        run: |\n", 1)[1])
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary); (root / "dist").mkdir()
            tar = root / "dist/input.tgz"; tar.write_bytes(b"qualified fixture")
            environment = dict(os.environ, FILENAME="input.tgz", TARBALL_SHA256=PACKAGE.digest(tar.read_bytes()))
            for control in ("valid", "changed", "extra", "symlink"):
                if control == "changed": tar.write_bytes(b"different")
                if control == "extra": tar.write_bytes(b"qualified fixture"); (root / "dist/extra").write_bytes(b"extra")
                if control == "symlink":
                    (root / "dist/extra").unlink(); tar.rename(root / "target"); tar.symlink_to(root / "target")
                result = subprocess.run(["bash", "-euo", "pipefail", "-c", guard], cwd=root, env=environment, capture_output=True, check=False)
                self.assertEqual(result.returncode == 0, control == "valid")

    @unittest.skipUnless(os.name == "posix", "GitHub-hosted stage job uses bash")
    def test_actual_prewrite_guard_rereads_version_after_approval_delay(self):
        workflow = (PACKAGE.ROOT / ".github/workflows/publish-npm.yml").read_text()
        step = workflow.split("      - name: Recheck public version after environment approval\n", 1)[1].split("      - ", 1)[0]
        self.assertLess(workflow.index("      - name: Recheck public version after environment approval"),
                        workflow.index("      - name: Stage exact bytes through the stage-only trusted publisher"))
        self.assertNotIn("        if:", step)
        self.assertNotIn("continue-on-error", step)
        guard = textwrap.dedent(step.split("        run: |\n", 1)[1])
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary); tools = root / "tools"; tools.mkdir()
            curl = tools / "curl"
            curl.write_text('#!/bin/sh\ncp "$REGISTRY_FIXTURE" registry-state.json\nprintf called > curl-called\nprintf "%s" "$REGISTRY_STATUS"\n')
            curl.chmod(0o755)
            fixture = root / "registry-fixture.json"
            public = {"name": PACKAGE.PACKAGE_NAME, "versions": {"0.11.0-alpha.1": {}}}
            appeared = copy.deepcopy(public); appeared["versions"]["0.11.0-alpha.2"] = {}
            for label, body, status, allowed in (
                ("absent before approval", public, "200", True),
                ("published during approval", appeared, "200", False),
                ("wrong package", dict(public, name="other"), "200", False),
                ("missing versions", {"name": PACKAGE.PACKAGE_NAME}, "200", False),
                ("placeholder only", dict(public, versions={"0.0.0-stage": {}}), "200", False),
                ("unreadable registry", public, "503", False),
            ):
                fixture.write_bytes(PACKAGE.canonical(body))
                marker = root / "curl-called"
                if marker.exists(): marker.unlink()
                environment = dict(os.environ, PATH=str(tools) + os.pathsep + os.environ["PATH"],
                                   REGISTRY_FIXTURE=str(fixture), REGISTRY_STATUS=status,
                                   VERSION="0.11.0-alpha.2")
                result = subprocess.run(["bash", "-euo", "pipefail", "-c", guard], cwd=root,
                                        env=environment, capture_output=True, check=False)
                with self.subTest(label=label):
                    self.assertTrue(marker.exists(), "prewrite guard must perform a fresh registry read")
                    self.assertEqual(result.returncode == 0, allowed, result.stderr)

    def test_real_admission_keeps_only_exact_bytes_and_never_runs_artifact_code(self):
        source, native_files, native_artifact = native_evidence_fixture()
        pin, native = PACKAGE.inspect_native_evidence(native_files, source, "123", "2", native_artifact)
        provenance = dict(schema_version=1, product_source=pin,
                          packaging_source={"sha": "a" * 40, "tree": "b" * 40},
                          sbom_sha256=PACKAGE.digest(native["sbom.cyclonedx.json"]),
                          native_build_repeated=False, automatic_npm_oidc_provenance=False)
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary); filename = f"effortlessmetrics-ripr-{pin['native_version']}.tgz"
            tar = root / filename
            native.update({"package.json": PACKAGE.canonical(PACKAGE.manifest(pin)),
                           "provenance.json": PACKAGE.canonical(provenance),
                           "README.md": (PACKAGE.ROOT / "packaging/npm/README.md").read_bytes()})
            with tarfile.open(tar, "w:gz") as archive:
                for name, data in native.items():
                    entry = tarfile.TarInfo("package/" + name); entry.size = len(data)
                    entry.mode = 0o755 if name == "bin/ripr" else 0o644
                    archive.addfile(entry, io.BytesIO(data))
            self.identity["tarball_sha256"] = PACKAGE.digest(tar.read_bytes())
            tar_receipt = PACKAGE.validate_tarball(tar, pin, provenance)
            receipt = dict(schema_version=1, package_name=PACKAGE.PACKAGE_NAME, version=pin["native_version"],
                           provenance=provenance, tarball=tar_receipt, qualification_run_id="123",
                           qualification_run_attempt="2", qualification_state="prepared_only", publication_attempted=False)
            prepared = {"tarballs/" + filename: tar.read_bytes(), "package-receipt.json": PACKAGE.canonical(receipt),
                        "npm-pack.json": b"[]", "npm_consumer.py": Path(CONSUMER.__file__).read_bytes()}
            prepared.update({"fixture/" + p.relative_to(PACKAGE.ROOT / "fixtures/python/basic").as_posix(): p.read_bytes()
                             for p in (PACKAGE.ROOT / "fixtures/python/basic").rglob("*") if p.is_file()})
            bundles = [prepared]
            for node, npm in PACKAGE.CLIENTS:
                proof, files = self.proof()
                proof.update(node="v" + node, npm=npm, npx=npm, tarball_sha256=tar_receipt["sha256"], payload_sha256=pin["payload_sha256"])
                files["consumer.json"] = PACKAGE.canonical(proof); bundles.append(files)
            stage_proof = dict(schema_version=1, state="passed_local_transport_only", npm="11.15.0",
                               package=PACKAGE.PACKAGE_NAME, version=pin["native_version"],
                               tarball_sha256=tar_receipt["sha256"], integrity=tar_receipt["integrity"],
                               selected_requests=2, observed_requests=2, lifecycle_marker_created=False,
                               oidc_exercised=False, external_registry_write_attempted=False)
            bundles.append({"stage-cli-proof.json": PACKAGE.canonical(stage_proof)})
            bundles.append(native_files)
            for index, control in enumerate(("valid", "receipt-source", "foreign-pin", "empty-proof", "wrong-tar", "existing-version", "wrong-transport", "changed-authority")):
                with self.subTest(control=control):
                    selected = copy.deepcopy(bundles)
                    if control == "receipt-source":
                        value = copy.deepcopy(receipt); value["provenance"]["packaging_source"]["sha"] = "e" * 40
                        selected[0]["package-receipt.json"] = PACKAGE.canonical(value)
                    if control == "foreign-pin":
                        value = copy.deepcopy(receipt); value["provenance"]["product_source"]["product_source_sha"] = "e" * 40
                        selected[0]["package-receipt.json"] = PACKAGE.canonical(value)
                    if control == "empty-proof": selected[1]["npx-explain.txt"] = b""
                    if control == "wrong-tar": selected[0]["tarballs/" + filename] += b"wrong"
                    if control == "wrong-transport": selected[3]["stage-cli-proof.json"] = PACKAGE.canonical(dict(stage_proof, observed_requests=0))
                    archives = [zip_bytes(files) for files in selected]
                    authority = {"identity": self.identity, "artifacts": [dict(id=i + 400, name="npm-native-wheel-123-2" if i == 4 else "fixture", digest="sha256:" + PACKAGE.digest(data), size_in_bytes=len(data)) for i, data in enumerate(archives)]}
                    after = {} if control == "changed-authority" else authority
                    public = {"name": PACKAGE.PACKAGE_NAME, "versions": {"0.1.0": {}}}
                    if control == "existing-version": public["versions"][pin["native_version"]] = {}
                    destination = root / str(index)
                    env = dict(name="npm", id=17, deployment_branch_policy=dict(protected_branches=False, custom_branch_policies=True), protection_rules=[dict(type="required_reviewers", prevent_self_review=False, reviewers=[dict(type="User", reviewer=dict(id=15812269, login="EffortlessSteven"))])])
                    branches = dict(total_count=1, branch_policies=[dict(name="main", type="branch")])
                    with mock.patch.object(PACKAGE, "authorize_release", side_effect=[authority, after]), mock.patch.object(PACKAGE, "github_api", side_effect=[*archives, env, branches]), mock.patch.object(PACKAGE, "public_package", return_value=public), mock.patch.object(PACKAGE, "run", side_effect=AssertionError("admission must not execute packages")):
                        if control == "valid":
                            with mock.patch("builtins.print"):
                                result = PACKAGE.admit_release(destination, self.identity, "a" * 40, "stage")
                            self.assertTrue(result["stage_eligible"])
                            self.assertEqual((destination / "tarballs" / filename).read_bytes(), tar.read_bytes())
                            self.assertEqual(sorted(p.name for p in destination.iterdir()), ["release-admission.json", "tarballs"])
                        else:
                            with self.assertRaises(ValueError):
                                PACKAGE.admit_release(destination, self.identity, "a" * 40, "stage")
                            self.assertFalse(destination.exists())


@unittest.skipUnless(os.environ.get("RIPR_NPM_STAGE_ARTIFACT"), "real CLI transport is a separate read-only qualification job")
class NpmStageCliTests(unittest.TestCase):
    def test_stage_cli_preserves_qualified_tar_and_disables_lifecycle(self):
        artifact = Path(os.environ["RIPR_NPM_STAGE_ARTIFACT"])
        receipt = json.loads((artifact / "package-receipt.json").read_text())
        tar = (artifact / "tarballs" / receipt["tarball"]["filename"]).resolve()
        self.assertEqual(PACKAGE.digest(tar.read_bytes()), receipt["tarball"]["sha256"])
        npm = shutil.which("npm"); self.assertIsNotNone(npm)
        self.assertEqual(subprocess.check_output([npm, "--version"], text=True).strip(), "11.15.0")
        workflow = (PACKAGE.ROOT / ".github/workflows/publish-npm.yml").read_text()
        commands = [line.strip().removeprefix("run: ") for line in workflow.splitlines()
                    if line.strip().startswith("run: npm stage publish ")]
        self.assertEqual(len(commands), 1)
        command = commands[0]
        # Exercise the actual workflow's shell invocation. Only the registry and
        # live attestation request differ in this credential-free loopback test.
        for flag in ("--registry=https://registry.npmjs.org", "--provenance", "--ignore-scripts"):
            self.assertEqual(command.count(flag), 1)
        command = command.replace("--registry=https://registry.npmjs.org", '--registry="$TEST_REGISTRY"')
        command = command.replace(" --provenance", "")
        requests = []
        class Registry(http.server.BaseHTTPRequestHandler):
            def log_message(self, *_args): pass
            def do_GET(self):
                data = PACKAGE.canonical({"name": PACKAGE.PACKAGE_NAME, "versions": {"0.1.0": {"version": "0.1.0"}}})
                self.send_response(200); self.send_header("Content-Type", "application/json"); self.end_headers(); self.wfile.write(data)
            def do_POST(self):
                requests.append((self.path, json.loads(self.rfile.read(int(self.headers["Content-Length"])))))
                self.send_response(201); self.send_header("Content-Type", "application/json"); self.end_headers()
                self.wfile.write(b'{"stageId":"01234567-89ab-4cde-8fab-0123456789ab"}')
        server = http.server.HTTPServer(("127.0.0.1", 0), Registry)
        thread = threading.Thread(target=server.serve_forever, daemon=True); thread.start()
        try:
            with tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary); registry = f"http://127.0.0.1:{server.server_port}/"
                config = root / "npmrc"
                # This inert fixture value is accepted only by the loopback mock,
                # never by npm or another external service.
                config.write_text(f"//127.0.0.1:{server.server_port}/:_authToken=local-fixture-only\n")
                environment = {"PATH": os.environ["PATH"], "HOME": str(root), "NPM_CONFIG_USERCONFIG": str(config),
                               "NPM_CONFIG_GLOBALCONFIG": str(root / "empty-global"),
                               "NPM_CONFIG_CACHE": str(root / "cache"), "NPM_CONFIG_UPDATE_NOTIFIER": "false",
                               "TEST_REGISTRY": registry, "GIT_CONFIG_COUNT": "1",
                               "GIT_CONFIG_KEY_0": "protocol.allow", "GIT_CONFIG_VALUE_0": "never"}
                (root / "dist").mkdir()
                scripted = root / "scripted.tgz"
                with tarfile.open(scripted, "w:gz") as archive:
                    package = dict(name=PACKAGE.PACKAGE_NAME, version=receipt["version"],
                                   scripts={event: "node -e \"require('fs').writeFileSync('lifecycle-marker','unexpected')\"" for event in ("prepublishOnly", "prepack", "prepare", "postpack", "publish", "postpublish")})
                    data = PACKAGE.canonical(package); entry = tarfile.TarInfo("package/package.json"); entry.size = len(data)
                    archive.addfile(entry, io.BytesIO(data))
                for source in (tar, scripted):
                    shutil.copyfile(source, root / "dist" / source.name)
                    environment["FILENAME"] = source.name
                    result = subprocess.run(["bash", "-c", command],
                                            cwd=root, env=environment, capture_output=True, text=True, timeout=60, check=False)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    output = json.loads((root / "stage-result.json").read_text())[PACKAGE.PACKAGE_NAME]
                    self.assertEqual(output["stageId"], "01234567-89ab-4cde-8fab-0123456789ab")
                    self.assertEqual(output["integrity"], "sha512-" + base64.b64encode(hashlib.sha512(source.read_bytes()).digest()).decode())
                    path, posted = requests[-1]
                    self.assertEqual(path, "/-/stage/package/@effortlessmetrics%2fripr")
                    self.assertEqual(posted["dist-tags"], {"next": receipt["version"]}); self.assertEqual(posted["access"], "public")
                    submitted = [base64.b64decode(value["data"]) for name, value in posted["_attachments"].items() if name.endswith(".tgz")]
                    self.assertEqual(submitted, [source.read_bytes()])
                self.assertEqual(len(requests), 2); self.assertFalse((root / "lifecycle-marker").exists())
                proof = dict(schema_version=1, state="passed_local_transport_only", npm="11.15.0",
                             package=PACKAGE.PACKAGE_NAME, version=receipt["version"],
                             tarball_sha256=receipt["tarball"]["sha256"], integrity=receipt["tarball"]["integrity"],
                             selected_requests=2, observed_requests=len(requests), lifecycle_marker_created=False,
                             oidc_exercised=False, external_registry_write_attempted=False)
                output = Path(os.environ["RIPR_NPM_STAGE_PROOF"]); output.mkdir(parents=True, exist_ok=False)
                (output / "stage-cli-proof.json").write_bytes(PACKAGE.canonical(proof))
        finally:
            server.shutdown(); server.server_close(); thread.join()


if __name__ == "__main__":
    unittest.main()
