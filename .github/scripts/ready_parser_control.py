#!/usr/bin/env python3
"""Hosted-only, reversible wrong-parser control for the two owned Rust tests."""
import hashlib
import json
import os
from pathlib import Path
import re
import selectors
import signal
import stat
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[2]
MAIN = ROOT / "xtask/src/main.rs"
TESTS = ROOT / "xtask/src/ready_types_tests.rs"
WORKFLOW = ROOT / ".github/workflows/routed-rust.yml"
GOOD = b'            in_pull_request = line.strip_prefix("  ").map(str::trim_end) == Some("pull_request:");'
BAD = b'            in_pull_request = line.trim_end() == "pull_request:";'
POSITIVE = "ready_types_tests::source_routed_rust_ready_types_admit_exact_indented_event"
NEGATIVE = "ready_types_tests::source_routed_rust_ready_types_reject_indent_and_event_substitutes"
COMMAND = ("cargo", "test", "--locked", "-p", "xtask", "--bin", "xtask",
           "ready_types_tests", "--", "--test-threads=1")
EXPECTED_TESTS_SHA256 = "622a8076014ed819183906bc13a2ca2d2c4e236ad9b629ab69e86f4c468bfd2e"
SOURCE_CAP = 2 * 1024 * 1024
OUTPUT_CAP = 256 * 1024
DEADLINE = time.monotonic() + 270
ENV = dict(os.environ, GIT_NO_LAZY_FETCH="1", GIT_OPTIONAL_LOCKS="0")


def digest(data):
    return hashlib.sha256(data).hexdigest()


def interrupted(number, _frame):
    raise RuntimeError(f"catchable signal {number}: control interrupted")


def read_regular(path, cap):
    if path.is_symlink() or not stat.S_ISREG(path.stat().st_mode):
        raise RuntimeError(f"not a regular owned source file: {path}")
    with path.open("rb") as stream:
        data = stream.read(cap + 1)
    if len(data) > cap:
        raise RuntimeError(f"source byte ceiling exceeded: {path}")
    return data


def capture(command, cap, seconds):
    remaining = DEADLINE - time.monotonic()
    if remaining <= 0:
        raise RuntimeError("control wall-clock ceiling reached before spawn")
    until = time.monotonic() + min(seconds, remaining)
    child = subprocess.Popen(command, cwd=ROOT, env=ENV, stdin=subprocess.DEVNULL,
                             stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                             start_new_session=True)
    data = bytearray()
    selector = selectors.DefaultSelector()
    try:
        selector.register(child.stdout, selectors.EVENT_READ)
        eof = False
        while not eof:
            if time.monotonic() >= until:
                raise RuntimeError("owned command wall-clock ceiling reached")
            for key, _events in selector.select(max(0, min(0.2, until - time.monotonic()))):
                chunk = os.read(key.fileobj.fileno(), min(65536, cap - len(data) + 1))
                if not chunk:
                    eof = True
                    break
                if len(data) + len(chunk) > cap:
                    raise RuntimeError("owned command output ceiling reached before retention")
                data.extend(chunk)
        status = child.wait(timeout=max(0.01, until - time.monotonic()))
        return status, bytes(data)
    finally:
        selector.close()
        # Reap this command and kill its owned process group on every outcome.
        # This is ordinary cleanup, not cgroup escape resistance or aggregate enforcement.
        try:
            os.killpg(child.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        child.wait(timeout=5)
        if child.stdout is not None:
            child.stdout.close()


def git_bytes(args, cap):
    status, data = capture(("git", "--no-replace-objects", *args), cap, 10)
    if status != 0:
        raise RuntimeError(f"source identity command failed with {status}")
    return data


def admit_sources():
    if sys.platform != "linux" or Path.cwd().resolve() != ROOT:
        raise RuntimeError("control requires its owned Ubuntu repository working directory")
    head = git_bytes(("rev-parse", "HEAD"), 128).decode("ascii").strip()
    tree = git_bytes(("rev-parse", "HEAD^{tree}"), 128).decode("ascii").strip()
    if not re.fullmatch("[0-9a-f]{40}", head) or not re.fullmatch("[0-9a-f]{40}", tree):
        raise RuntimeError("missing exact source commit/tree identity")
    inputs = {}
    for path, cap in ((MAIN, SOURCE_CAP), (TESTS, 16384), (WORKFLOW, 131072),
                      (Path(__file__).resolve(), 32768), (ROOT / ".github/workflows/ci.yml", 65536)):
        data = read_regular(path, cap)
        relative = path.relative_to(ROOT).as_posix()
        if data != git_bytes(("show", f"{head}:{relative}"), cap):
            raise RuntimeError(f"source differs from admitted HEAD: {relative}")
        inputs[relative] = {"bytes": len(data), "sha256": digest(data)}
    if inputs["xtask/src/ready_types_tests.rs"]["sha256"] != EXPECTED_TESTS_SHA256:
        raise RuntimeError("owned two-test module differs from the independently reviewed source")
    original = read_regular(MAIN, SOURCE_CAP)
    start = original.find(b"\nfn routed_rust_pull_request_types(")
    end = original.find(b"\nfn ", start + 1)
    if start < 0 or end < 0 or original.count(GOOD) != 1 or BAD in original:
        raise RuntimeError("corrected unique parser comparison is not present")
    if GOOD not in original[start:end]:
        raise RuntimeError("comparison is outside the actual scoped parser")
    return head, tree, inputs, original


def verify_current_sources(head, tree, inputs, expected_main):
    current_head = git_bytes(("rev-parse", "HEAD"), 128).decode("ascii").strip()
    current_tree = git_bytes(("rev-parse", "HEAD^{tree}"), 128).decode("ascii").strip()
    if current_head != head or current_tree != tree:
        raise RuntimeError("source commit/tree changed during focused proof")
    observed = {}
    for relative, bound in inputs.items():
        path = ROOT / relative
        cap = len(expected_main) if path == MAIN else bound["bytes"]
        data = read_regular(path, cap)
        if path == MAIN:
            if data != expected_main:
                raise RuntimeError("parser bytes changed during focused proof")
        elif len(data) != bound["bytes"] or digest(data) != bound["sha256"]:
            raise RuntimeError(f"admitted input changed during focused proof: {relative}")
        observed[relative] = {"bytes": len(data), "sha256": digest(data)}
    return observed


def require_harness(status, output, wrong):
    text = output.decode("utf-8")
    expected = {POSITIVE: "FAILED" if wrong else "ok", NEGATIVE: "ok"}
    if re.findall(r"^running (\d+) tests$", text, re.MULTILINE) != ["2"]:
        raise RuntimeError("focused harness did not execute exactly two tests")
    observed = re.findall(r"^test (ready_types_tests::\S+) \.\.\. (ok|FAILED)$",
                          text, re.MULTILINE)
    if len(observed) != 2 or dict(observed) != expected:
        raise RuntimeError(f"focused outcomes do not match the owned controls: {observed}")
    summaries = re.findall(
        r"^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; "
        r"(\d+) measured; (\d+) filtered out; finished in .+$", text, re.MULTILINE)
    target = ("FAILED", "1", "1", "0", "0") if wrong else ("ok", "2", "0", "0", "0")
    if len(summaries) != 1 or summaries[0][:5] != target:
        raise RuntimeError("missing or inconsistent native focused summary")
    if status != (101 if wrong else 0):
        raise RuntimeError(f"unexpected native Cargo exit status: {status}")
    if wrong and (re.search(r"left:\s+None", text) is None or
                  re.search(r'right:\s+Some\(\["ready_for_review"\]\)', text) is None):
        raise RuntimeError("wrong-parser positive failed without its exact None-versus-Ready oracle")
    return {"exit_status": status, "tests": expected, "summary": summaries[0],
            "output_bytes": len(output), "output_sha256": digest(output)}


def run_control():
    head, tree, inputs, original = admit_sources()
    wrong = original.replace(GOOD, BAD, 1)
    proof = {"schema": "ripr.ready_parser_wrong_implementation.v1",
             "commit": head, "tree": tree, "inputs": inputs,
             "command": list(COMMAND), "native_status": "INCOMPLETE",
             "hard_resource_enforcement": "NOT_ESTABLISHED",
             "wrong_main_sha256": digest(wrong), "source_observations": {}}
    try:
        # No ref/index/commit changes. Only this uniquely admitted comparison changes.
        MAIN.write_bytes(wrong)
        if read_regular(MAIN, SOURCE_CAP) != wrong:
            raise RuntimeError("wrong-parser write was not exact")
        proof["source_observations"]["before_wrong"] = verify_current_sources(head, tree, inputs, wrong)
        status, output = capture(COMMAND, OUTPUT_CAP, 120)
        proof["source_observations"]["after_wrong"] = verify_current_sources(head, tree, inputs, wrong)
        print("READY_PARSER_WRONG_NATIVE_LOG_BEGIN", flush=True)
        print(output.decode("utf-8"), end="", flush=True)
        print("READY_PARSER_WRONG_NATIVE_LOG_END", flush=True)
        proof["wrong_implementation"] = require_harness(status, output, True)
    finally:
        MAIN.write_bytes(original)
        if read_regular(MAIN, SOURCE_CAP) != original:
            raise RuntimeError("exact parser restoration failed; corrected proof refused")
    proof["restored_main_sha256"] = digest(read_regular(MAIN, SOURCE_CAP))
    proof["source_observations"]["before_corrected"] = verify_current_sources(head, tree, inputs, original)
    status, output = capture(COMMAND, OUTPUT_CAP, 120)
    proof["source_observations"]["after_corrected"] = verify_current_sources(head, tree, inputs, original)
    print("READY_PARSER_CORRECTED_NATIVE_LOG_BEGIN", flush=True)
    print(output.decode("utf-8"), end="", flush=True)
    print("READY_PARSER_CORRECTED_NATIVE_LOG_END", flush=True)
    proof["corrected_implementation"] = require_harness(status, output, False)
    if read_regular(MAIN, SOURCE_CAP) != original:
        raise RuntimeError("source changed during corrected proof")
    proof["native_status"] = "OWNED_TWO_TEST_WRONG_AND_CORRECTED_PROOF"
    print(json.dumps(proof, sort_keys=True), flush=True)


if __name__ == "__main__":
    signal.signal(signal.SIGINT, interrupted)
    signal.signal(signal.SIGTERM, interrupted)
    try:
        run_control()
    except (OSError, RuntimeError, UnicodeError, subprocess.SubprocessError) as error:
        print(json.dumps({"schema": "ripr.ready_parser_wrong_implementation.v1",
                          "native_status": "REFUSED_OR_INCOMPLETE",
                          "reason": str(error)[:1024]}), flush=True)
        raise SystemExit(1)
