"""Replay the independently specified upstream bug, not a RIPR prediction.

Requires the explicit environment in README.md. Child pytest always loads the
unaltered upstream conftest, including its pytest-xprocess dependency.
"""
import hashlib
import importlib.metadata
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import xml.etree.ElementTree as ET

import pytest

CASE = Path(__file__).resolve().parent
SELECTOR = "tests/sansio/test_request.py::test_cookies"
EFFECTIVE = '    assert req.cookies.getlist("a") == ["b", "c"]\n'
INEFFECTIVE = '    assert req.cookies.get("a") == "b"\n'
FIXED = '        wsgi_combined_cookie = ";".join(self.headers.getlist("Cookie"))\n'
BROKEN_ARGUMENT = '            self.headers.get("Cookie"),\n'
FIXED_ARGUMENT = '            wsgi_combined_cookie,\n'
CONTROLS = '''import pytest
from werkzeug.datastructures import Headers
from werkzeug.sansio.request import Request

@pytest.mark.parametrize("headers, expected", [
    ([("Cookie", "a=b")], ["b"]),
    ([("Content-Type", "text"), ("Cookie", "a=b"), ("X-Unrelated", "c")], ["b"]),
    ([("Content-Type", "text"), ("X-Unrelated", "a=c")], []),
])
def test_single_cookie_and_unrelated_headers(headers, expected):
    req = Request("GET", "http", None, "", "", b"", Headers(headers), None)
    assert req.cookies.getlist("a") == expected
'''


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


@pytest.mark.parametrize("variant, selector, expected_tests, expected_failures", [
    ("fixed_full", SELECTOR, 1, 0),
    ("broken_full", SELECTOR, 1, 1),
    ("broken_assertion_removed", SELECTOR, 1, 0),
    ("fixed_assertion_removed", SELECTOR, 1, 0),
    ("fixed_upstream_file", "tests/sansio/test_request.py", 5, 0),
    ("fixed_positive_controls", "tests/sansio/test_cookie_controls.py", 3, 0),
    ("broken_positive_controls", "tests/sansio/test_cookie_controls.py", 3, 0),
])
def test_upstream_cookie_oracle(tmp_path, variant, selector, expected_tests, expected_failures):
    # The optional source is used only to validate the retained slice against
    # the independently downloaded, exact complete upstream fixed source.
    source = Path(os.environ.get("WERKZEUG_UPSTREAM_SOURCE", str(CASE / "input"))).resolve()
    root = tmp_path / "subject"
    shutil.copytree(source / "src", root / "src", ignore=shutil.ignore_patterns("__pycache__"))
    (root / "tests/sansio").mkdir(parents=True)
    for name in ["setup.cfg", "tests/conftest.py", "tests/sansio/test_request.py"]:
        shutil.copyfile(source / name, root / name)
    production = root / "src/werkzeug/sansio/request.py"
    test = root / "tests/sansio/test_request.py"
    original_test = test.read_text()
    assert original_test.count(EFFECTIVE) == 1
    assert original_test.count(INEFFECTIVE) == 1
    if variant.startswith("broken"):
        text = production.read_text()
        assert text.count(FIXED) == 1 and text.count(FIXED_ARGUMENT) == 1
        production.write_text(text.replace(FIXED, "").replace(FIXED_ARGUMENT, BROKEN_ARGUMENT))
        # Require the exact original upstream parent blob, not a toy mutant.
        blob = b"blob " + str(production.stat().st_size).encode() + b"\0" + production.read_bytes()
        assert hashlib.sha1(blob).hexdigest() == "8c22d4e2c98ddaef2530926eec0c8a697d7736b1"
    if variant.endswith("assertion_removed"):
        test.write_text(original_test.replace(EFFECTIVE, ""))
        assert INEFFECTIVE in test.read_text()
        assert test.read_text() + EFFECTIVE == original_test
    if variant.endswith("positive_controls"):
        (root / "tests/sansio/test_cookie_controls.py").write_text(CONTROLS)

    env = os.environ.copy()
    # Explicit src-layout setup, recorded below; no conftest or import bypass.
    env["PYTHONPATH"] = str(root / "src")
    env["PYTHONDONTWRITEBYTECODE"] = "1"
    env.pop("PYTEST_ADDOPTS", None)
    env.pop("PYTEST_PLUGINS", None)
    env.pop("PYTEST_DISABLE_PLUGIN_AUTOLOAD", None)
    identity = subprocess.run(
        [sys.executable, "-c", "import werkzeug,sys;print(werkzeug.__file__);print(sys.executable)"],
        cwd=root, env=env, text=True, capture_output=True, timeout=30,
    )
    assert identity.returncode == 0, identity.stderr
    assert Path(identity.stdout.splitlines()[0]).resolve() == root / "src/werkzeug/__init__.py"
    xml_path = tmp_path / "junit.xml"
    argv = [sys.executable, "-m", "pytest", selector, "-q", "--junitxml", str(xml_path)]
    result = subprocess.run(argv, cwd=root, env=env, text=True, capture_output=True, timeout=30)
    assert xml_path.is_file(), (result.returncode, result.stdout, result.stderr)
    xml = ET.parse(xml_path).getroot()
    nodes = list(xml.iter("testcase"))
    errors = list(xml.iter("error"))
    failures = list(xml.iter("failure"))
    skipped = list(xml.iter("skipped"))
    observation = {
        "variant": variant, "argv": argv, "cwd": str(root), "source_root": str(source),
        "pythonpath": env["PYTHONPATH"], "interpreter": sys.executable,
        "import_identity": identity.stdout.splitlines(), "exit_code": result.returncode,
        "selected_and_executed": len(nodes), "failures": len(failures),
        "errors": len(errors), "skipped": len(skipped),
        "test_ids": [n.attrib for n in nodes],
        "production_sha256": digest(production), "test_sha256": digest(test),
        "conftest_sha256": digest(root / "tests/conftest.py"),
        "driver_sha256": digest(Path(__file__)),
        "requirements_lock_sha256": digest(CASE / "requirements.lock"),
        "runtime_versions": {name: importlib.metadata.version(name) for name in ["pytest", "pytest-xprocess"]},
        "python_version": sys.version,
        "stdout": result.stdout, "stderr": result.stderr,
        "junit_xml": xml_path.read_text(),
    }
    (tmp_path / "observation.json").write_text(json.dumps(observation, indent=2) + "\n")
    assert len(nodes) == expected_tests and not errors and not skipped, observation
    assert len(failures) == expected_failures, observation
    assert result.returncode == (1 if expected_failures else 0), observation
    if expected_failures:
        failure = failures[0].text or ""
        assert EFFECTIVE.strip() in failure and "AssertionError" in failure, observation
        assert "['b'] == ['b', 'c']" in failure, observation
        assert nodes[0].get("name") == "test_cookies", observation
