"""Fail-closed admission controls; no network, package execution, or publication."""
import copy
import hashlib
import json
import os
import subprocess
import textwrap
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import pypi_admission as admission


class AdmissionTests(unittest.TestCase):
    def setUp(self):
        self.identity = dict(run_id="123", source_sha="a" * 40, source_tree="b" * 40,
                             version="0.11.0rc1", wheel_sha256="c" * 64)
        self.run = dict(id=123, run_attempt=2, event="workflow_dispatch", head_branch="main",
                        head_sha="a" * 40, status="completed", conclusion="success",
                        path=".github/workflows/python-wheel-qualification.yml",
                        repository={"full_name": admission.REPOSITORY},
                        head_repository={"full_name": admission.REPOSITORY})
        self.commit = {"sha": "a" * 40, "tree": {"sha": "b" * 40}}
        self.main_ref = {"ref": "refs/heads/main", "object": {"type": "commit", "sha": "a" * 40}}

    def test_admits_exact_successful_source_run(self):
        self.assertEqual(admission.validate_run(self.run, self.commit, self.identity), 2)

    def test_npm_reusable_native_run_is_not_pypi_publication_authority(self):
        npm_run = dict(self.run, path=".github/workflows/npm-package-qualification.yml")
        with self.assertRaisesRegex(ValueError, "qualification run path mismatch"):
            admission.validate_run(npm_run, self.commit, self.identity)

    def test_rejects_wrong_run_authority_and_identity(self):
        for field, value in [("id", 124), ("event", "pull_request"), ("head_branch", "feature"),
                             ("head_sha", "d" * 40), ("conclusion", "failure"),
                             ("status", "in_progress"), ("path", ".github/workflows/other.yml"),
                             ("run_attempt", 0), ("repository", {"full_name": "fork/ripr"}),
                             ("head_repository", {"full_name": "fork/ripr"})]:
            with self.subTest(field=field), self.assertRaises(ValueError):
                admission.validate_run(dict(self.run, **{field: value}), self.commit, self.identity)
        with self.assertRaises(ValueError):
            admission.validate_run(self.run, {"sha": "a" * 40, "tree": {"sha": "e" * 40}}, self.identity)

    def test_only_current_main_and_matching_publisher_snapshot(self):
        admission.validate_source_ref(self.main_ref, self.identity, "a" * 40)
        invalid_refs = [
            {"ref": "refs/tags/main", "object": {"type": "commit", "sha": "a" * 40}},
            {"ref": "refs/heads/main-old", "object": {"type": "commit", "sha": "a" * 40}},
            {"ref": "refs/heads/main", "object": {"type": "tag", "sha": "a" * 40}},
            {"ref": "refs/heads/main", "object": {"type": "commit", "sha": "d" * 40}},
            {"ref": "refs/heads/main"}, {},
        ]
        for ref in invalid_refs:
            with self.subTest(ref=ref), self.assertRaises(ValueError):
                admission.validate_source_ref(ref, self.identity, "a" * 40)
        for publisher_sha in ["d" * 40, "", None]:
            with self.subTest(publisher_sha=publisher_sha), self.assertRaises(ValueError):
                admission.validate_source_ref(self.main_ref, self.identity, publisher_sha)

    def test_short_main_tag_cannot_admit_a_non_main_commit(self):
        # All old run/commit gates pass for this tag-like run. Independent API
        # branch identity must reject it before fetching the artifact list.
        source_run = dict(self.run, head_sha="d" * 40)
        source_commit = {"sha": "d" * 40, "tree": {"sha": "b" * 40}}
        request = dict(self.identity, source_sha="d" * 40)
        self.assertEqual(admission.validate_run(source_run, source_commit, request), 2)
        with patch.object(admission, "api", return_value=self.main_ref) as api:
            with self.assertRaises(ValueError):
                admission.authorize(request, "d" * 40)
            api.assert_called_once_with("git/ref/heads/main")

    def test_only_canonical_prereleases(self):
        for version in ["0.11.0", "v0.11.0rc1", "0.11.0rc1+local", "0.11.0rc1\nx=y", "../evil", "00.11.0rc1"]:
            with self.subTest(version=version), self.assertRaises(ValueError):
                admission.validate_identity(dict(self.identity, version=version))
        for version in ["0.11.0a1", "0.11.0b2", "0.11.0rc1"]:
            admission.validate_identity(dict(self.identity, version=version))

    def test_artifact_authority_controls(self):
        artifact = dict(id=456, name="pypi-qualified-wheel-123-2", expired=False,
                        workflow_run={"id": 123, "head_sha": "a" * 40})
        result = dict(total_count=1, artifacts=[artifact])
        with patch.object(admission, "api", side_effect=[self.main_ref, self.run, self.commit, result]):
            self.assertEqual(admission.authorize(self.identity, "a" * 40), (456, 2))
        bad_results = [dict(total_count=0, artifacts=[]),
                       dict(total_count=101, artifacts=[artifact]),
                       dict(total_count=2, artifacts=[artifact, artifact])]
        for field, value in [("name", "pypi-qualified-wheel-123-1"), ("expired", True),
                             ("id", "456"), ("workflow_run", {"id": 999, "head_sha": "a" * 40}),
                             ("workflow_run", {"id": 123, "head_sha": "e" * 40})]:
            bad_results.append(dict(total_count=1, artifacts=[dict(artifact, **{field: value})]))
        for result in bad_results:
            with self.subTest(result=result), self.assertRaises(ValueError):
                with patch.object(admission, "api", side_effect=[self.main_ref, self.run, self.commit, result]):
                    admission.authorize(self.identity, "a" * 40)

    def test_actual_consumer_rerun_guard_and_attempt_wiring(self):
        workflow = (Path(__file__).resolve().parents[1] / "workflows" /
                    "python-wheel-qualification.yml").read_text()
        # Exercise the checked-in shell, rather than a second implementation of
        # its intended behavior. Match only this named step's literal run block.
        step = workflow.split("      - name: Require full qualification rerun\n", 1)[1].split("      - ", 1)[0]
        self.assertIn("qualification_attempt: ${{ github.run_attempt }}", workflow)
        self.assertIn("BUILT_ATTEMPT: ${{ needs.build-wheel.outputs.qualification_attempt }}", step)
        self.assertIn("CURRENT_ATTEMPT: ${{ github.run_attempt }}", step)
        self.assertIn("name: pypi-qualified-wheel-${{ github.run_id }}-${{ github.run_attempt }}", workflow)
        script = textwrap.dedent(step.split("        run: |\n", 1)[1])
        self.assertTrue(script.strip(), "guard must have executable statements")
        for built, current, allowed in [("1", "1", True), ("2", "2", True),
                                        ("1", "2", False), ("", "2", False),
                                        ("2", "", False), ("", "", False)]:
            with self.subTest(built=built, current=current):
                result = subprocess.run(["bash", "-euo", "pipefail", "-c", script],
                                        env=dict(os.environ, BUILT_ATTEMPT=built, CURRENT_ATTEMPT=current),
                                        capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode == 0, allowed, result.stdout + result.stderr)
                if not allowed:
                    self.assertIn("Re-run all jobs", result.stdout + result.stderr)

    def fixture(self, root):
        wheelhouse = root / "wheelhouse"
        wheelhouse.mkdir()
        wheel = wheelhouse / "ripr_rs-0.11.0rc1-py3-none-manylinux_2_34_x86_64.whl"
        with zipfile.ZipFile(wheel, "w") as z:
            z.writestr("ripr_rs-0.11.0rc1.dist-info/METADATA", "Name: ripr-rs\nVersion: 0.11.0rc1\n")
            z.writestr("ripr_rs-0.11.0rc1.dist-info/WHEEL", "Tag: py3-none-manylinux_2_34_x86_64\n")
        self.identity["wheel_sha256"] = hashlib.sha256(wheel.read_bytes()).hexdigest()
        receipt = dict(schema_version=1, repository=admission.REPOSITORY, run_id=123, run_attempt=2, native_version="0.11.0-rc.1",
                       source_sha=self.identity["source_sha"], source_tree=self.identity["source_tree"],
                       version=self.identity["version"], wheel_filename=wheel.name,
                       wheel_sha256=self.identity["wheel_sha256"], target=admission.TARGET,
                       platform_tag=admission.PLATFORM_TAG)
        (root / "qualification.json").write_text(json.dumps(receipt))
        return wheel, receipt

    def test_exact_wheel_staged_alone(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            wheel, _ = self.fixture(root)
            output = root / "dist"
            admission.stage_wheel(root, output, self.identity, 2)
            self.assertEqual([p.name for p in output.iterdir()], [wheel.name])
            self.assertEqual((output / wheel.name).read_bytes(), wheel.read_bytes())

    def test_source_alpha_version_mapping(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            wheel, receipt = self.fixture(root)
            self.identity["version"] = receipt["version"] = "0.11.0a1"
            receipt["native_version"] = "0.11.0-alpha.1"
            new_wheel = wheel.with_name(wheel.name.replace("rc1", "a1"))
            with zipfile.ZipFile(new_wheel, "w") as archive:
                archive.writestr("ripr_rs-0.11.0a1.dist-info/METADATA", "Name: ripr-rs\nVersion: 0.11.0a1\n")
                archive.writestr("ripr_rs-0.11.0a1.dist-info/WHEEL", "Tag: py3-none-manylinux_2_34_x86_64\n")
            wheel.unlink()
            receipt["wheel_filename"] = new_wheel.name
            receipt["wheel_sha256"] = self.identity["wheel_sha256"] = hashlib.sha256(new_wheel.read_bytes()).hexdigest()
            (root / "qualification.json").write_text(json.dumps(receipt))
            admission.stage_wheel(root, root / "dist", self.identity, 2)
            self.assertEqual((root / "dist" / new_wheel.name).read_bytes(), new_wheel.read_bytes())

    def test_rejects_changed_receipt_fields(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _, receipt = self.fixture(root)
            for field in receipt:
                changed = copy.deepcopy(receipt)
                changed[field] = "wrong"
                (root / "qualification.json").write_text(json.dumps(changed))
                with self.subTest(field=field), self.assertRaises(ValueError):
                    admission.stage_wheel(root, root / "dist", self.identity, 2)

    def test_rejects_changed_wheel_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            wheel, _ = self.fixture(root)
            with wheel.open("ab") as f:
                f.write(b"tampered")
            with self.assertRaisesRegex(ValueError, "digest"):
                admission.stage_wheel(root, root / "dist", self.identity, 2)

    def test_rejects_metadata_even_with_matching_digest(self):
        for member, content in [("METADATA", "Name: other\nVersion: 0.11.0rc1\n"),
                                ("METADATA", "Name: ripr-rs\nVersion: 9.0.0rc1\n"),
                                ("WHEEL", "Tag: py3-none-any\n")]:
            with self.subTest(member=member, content=content), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                wheel, receipt = self.fixture(root)
                with zipfile.ZipFile(wheel) as archive:
                    entries = {name: archive.read(name) for name in archive.namelist()}
                entries[f"ripr_rs-0.11.0rc1.dist-info/{member}"] = content.encode()
                with zipfile.ZipFile(wheel, "w") as archive:
                    for name, data in entries.items():
                        archive.writestr(name, data)
                digest = hashlib.sha256(wheel.read_bytes()).hexdigest()
                receipt["wheel_sha256"] = self.identity["wheel_sha256"] = digest
                (root / "qualification.json").write_text(json.dumps(receipt))
                with self.assertRaises(ValueError):
                    admission.stage_wheel(root, root / "dist", self.identity, 2)

    def test_rejects_extra_wheels_and_symlinks(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            wheel, _ = self.fixture(root)
            extra = wheel.parent / "extra.whl"
            extra.write_bytes(b"other")
            with self.assertRaises(ValueError):
                admission.stage_wheel(root, root / "dist", self.identity, 2)
            extra.unlink()
            target = root / "original"
            wheel.rename(target)
            wheel.symlink_to(target)
            with self.assertRaises(ValueError):
                admission.stage_wheel(root, root / "dist", self.identity, 2)


if __name__ == "__main__":
    unittest.main()
