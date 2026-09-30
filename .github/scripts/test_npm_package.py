"""Discriminating controls for the real npm byte-admission functions."""
import base64
import copy
import csv
import importlib.util
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest
import zipfile

SPEC = importlib.util.spec_from_file_location("npm_package", Path(__file__).with_name("npm_package.py"))
PACKAGE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PACKAGE)


def wheel_fixture():
    pin = PACKAGE.source_pin()
    prefix = f"ripr_rs-{pin['python_version']}"
    info = prefix + ".dist-info"
    binary = bytearray(64)
    binary[:6] = b"\x7fELF\x02\x01"
    binary[18:20] = (62).to_bytes(2, "little")
    pin["payload_sha256"] = PACKAGE.digest(binary)
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
            entry.external_attr = (0o100755 if name.endswith("/ripr") and not no_exec else 0o100644) << 16
            if name == link:
                entry.external_attr = 0o120755 << 16
            archive.writestr(entry, data)
    return out.getvalue()


class NpmPackageTests(unittest.TestCase):
    def test_manifest_is_single_native_prerelease(self):
        pin = PACKAGE.source_pin()
        value = PACKAGE.manifest(pin)
        self.assertEqual(value["version"], pin["native_version"])
        self.assertNotIn("scripts", value)
        self.assertEqual(value["bin"], {"ripr": "bin/ripr"})

    def test_rejects_manifest_identity_platform_lifecycle_and_dependency_drift(self):
        pin = PACKAGE.source_pin()
        for key, value in {
            "name": "ripr-rs", "version": "0.11.0", "bin": {"ripr": "download.js"},
            "os": ["darwin"], "cpu": ["arm64"], "libc": ["musl"],
            "publishConfig": {"access": "public", "tag": "latest"},
            "scripts": {"postinstall": "false"}, "dependencies": {}, "optionalDependencies": {},
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


if __name__ == "__main__":
    unittest.main()
