#!/usr/bin/env python3
"""Stdlib consumer for a mounted portable native ripr packet.

This file is the packet-local transport adapter. It verifies integrity, launches
only the packet-relative payload by absolute path, and projects a consumption
receipt. Product meaning stays in existing ripr output contracts.

Python 3.11+ standard library only. The consumer never searches PATH for the
payload, never downloads, and never invokes a compiler.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import signal
import subprocess
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Mapping

PACKET_SCHEMA = "ripr.portable_consumer.packet/v1"
RECEIPT_SCHEMA = "ripr.portable_consumer.receipt/v1"
ALLOWED_OPERATIONS = frozenset({"check", "pilot"})
COMPLETE_OUTCOME_KINDS = frozenset(
    {
        "no_scope",
        "no_changed_lines",
        "no_behavioral_candidates",
        "complete_no_findings",
        "complete_with_findings",
    }
)
INCOMPLETE_OUTCOME_KINDS = frozenset(
    {
        "partial_with_limitations",
        "unsupported_input",
        "analysis_failed",
    }
)
DEFAULT_TIMEOUT_SECONDS = 60
MAX_SUBJECT_FILES = 1000
RECEIPT_NAME = "packet-consumption-receipt.json"
STDOUT_BIN = "stdout.bin"
STDERR_BIN = "stderr.bin"
DEFAULT_CHECK_ARGV = ["check", "--root", "{subject_root}", "--format", "json"]
DEFAULT_PILOT_ARGV = ["pilot", "--root", "{subject_root}", "--out", "{out_dir}"]
PRODUCT_SCHEMA_VERSION = "0.2"
PILOT_SUMMARY_NAME = "pilot-summary.json"

CLASS_COMPLETE = "complete"
CLASS_DIGEST_MISMATCH = "digest_mismatch"
CLASS_MISSING_EXECUTABLE = "missing_executable"
CLASS_INCOMPATIBLE_PAYLOAD = "incompatible_payload"
CLASS_ENVIRONMENT_UNAVAILABLE = "environment_unavailable"
CLASS_EXECUTION_FAILURE = "execution_failure"
CLASS_TIMEOUT = "timeout"
CLASS_MALFORMED_PRODUCT_OUTPUT = "malformed_product_output"
CLASS_PARTIAL_PRODUCT_OUTPUT = "partial_product_output"
CLASS_TYPED_PRODUCT_LIMITATION = "typed_product_limitation"
CLASS_SUBJECT_IDENTITY_DRIFT = "subject_identity_drift"
CLASS_UNWRITABLE_OUTPUT = "unwritable_output"
CLASS_ZERO_SUBJECTS = "zero_required_subjects"
CLASS_UNKNOWN_OPERATION = "unknown_operation"

COMPLETE_MESSAGES = {
    CLASS_COMPLETE: "packet consumption completed",
    CLASS_ZERO_SUBJECTS: "declared nonzero fixture produced zero subjects",
    CLASS_PARTIAL_PRODUCT_OUTPUT: "product output is incomplete",
    CLASS_TYPED_PRODUCT_LIMITATION: "product completed with a typed limitation",
}

NON_CLAIMS = (
    "Static packet consumption only; not a runtime mutation result.",
    "Does not establish correctness, test adequacy, or merge readiness.",
    "Does not compile or claim an unbuilt RIPR source change.",
)


class ConsumerError(Exception):
    def __init__(
        self,
        classification: str,
        message: str,
        *,
        exit_code: int | None = None,
        stdout: bytes | None = None,
        stderr: bytes | None = None,
        elapsed_ms: str | None = None,
        argv: list[str] | None = None,
        selected: int | None = None,
        limitations: list[str] | None = None,
        identities: dict[str, str] | None = None,
    ) -> None:
        super().__init__(message)
        self.classification = classification
        self.message = message
        self.exit_code = exit_code
        self.stdout = stdout
        self.stderr = stderr
        self.elapsed_ms = elapsed_ms
        self.argv = argv
        self.selected = selected
        self.limitations = list(limitations or [])
        self.identities = identities


@dataclass
class Attempt:
    classification: str = CLASS_EXECUTION_FAILURE
    message: str = "packet consumption did not complete"
    manifest: dict[str, Any] | None = None
    identities: dict[str, str] | None = None
    argv: list[str] | None = None
    subject_digest: str | None = None
    limitations: list[str] = field(default_factory=list)
    stdout: bytes | None = None
    stderr: bytes | None = None
    exit_code: int | None = None
    elapsed_ms: str | None = None
    selected: int | None = None

    def absorb_error(self, error: ConsumerError) -> None:
        self.classification = error.classification
        self.message = error.message
        if error.exit_code is not None:
            self.exit_code = error.exit_code
        if error.stdout is not None:
            self.stdout = error.stdout
        if error.stderr is not None:
            self.stderr = error.stderr
        if error.elapsed_ms is not None:
            self.elapsed_ms = error.elapsed_ms
        if error.argv is not None:
            self.argv = error.argv
        if error.selected is not None:
            self.selected = error.selected
        if error.limitations:
            self.limitations.extend(error.limitations)
        if error.identities:
            self.identities = error.identities


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while True:
            chunk = handle.read(1024 * 1024)
            if not chunk:
                break
            digest.update(chunk)
    return digest.hexdigest()


def packet_digest_from_files(files: Mapping[str, str]) -> str:
    lines = [f"{name}={digest}" for name, digest in sorted(files.items())]
    return sha256_bytes("\n".join(lines).encode("utf-8"))


def load_json_object(path: Path, classification: str) -> dict[str, Any]:
    try:
        raw = path.read_text(encoding="utf-8")
    except OSError as exc:
        raise ConsumerError(classification, f"failed to read {path}: {exc}") from exc
    try:
        value = json.loads(raw)
    except json.JSONDecodeError as exc:
        raise ConsumerError(classification, f"{path} is not JSON: {exc}") from exc
    if not isinstance(value, dict):
        raise ConsumerError(classification, f"{path} is not a JSON object")
    return value


def require_str(obj: Mapping[str, Any], key: str, classification: str) -> str:
    value = obj.get(key)
    if not isinstance(value, str) or not value:
        raise ConsumerError(classification, f"manifest field {key!r} must be a nonempty string")
    return value


def contained_path(root: Path, relative: str, classification: str) -> Path:
    if Path(relative).is_absolute() or ".." in Path(relative).parts:
        raise ConsumerError(classification, f"path {relative!r} escapes the packet root")
    resolved_root = root.resolve()
    candidate = (root / relative).resolve()
    try:
        candidate.relative_to(resolved_root)
    except ValueError as exc:
        raise ConsumerError(classification, f"path {relative!r} escapes the packet root") from exc
    return candidate


def python_version_ok(required: str) -> bool:
    if not required.startswith(">="):
        return False
    parts = required[2:].split(".")
    try:
        wanted = tuple(int(part) for part in parts)
    except ValueError:
        return False
    return sys.version_info[: len(wanted)] >= wanted


def normalize_machine(machine: str) -> str:
    lowered = machine.lower()
    if lowered in {"amd64", "x64"}:
        return "x86_64"
    if lowered in {"arm64", "aarch64"}:
        return "aarch64"
    return lowered


def host_platform() -> str:
    machine = normalize_machine(platform.machine())
    system = sys.platform
    if system.startswith("linux"):
        system = "linux"
    elif system.startswith("win"):
        system = "windows"
    elif system.startswith("darwin"):
        system = "macos"
    return f"{system}-{machine}"


def tree_digest(root: Path) -> str:
    files: dict[str, str] = {}
    for dirpath, dirnames, filenames in os.walk(root, followlinks=False):
        dirnames[:] = sorted(name for name in dirnames if name not in {".git", "target"})
        for name in sorted(filenames):
            path = Path(dirpath) / name
            if path.is_symlink() or not path.is_file():
                continue
            relative = path.relative_to(root).as_posix()
            files[relative] = sha256_file(path)
            if len(files) > MAX_SUBJECT_FILES:
                raise ConsumerError(
                    CLASS_ENVIRONMENT_UNAVAILABLE,
                    f"subject root exceeds {MAX_SUBJECT_FILES} files; --subject-digest is not available for this tree",
                )
    return packet_digest_from_files(files)


def load_manifest(packet_root: Path) -> dict[str, Any]:
    manifest_path = packet_root / "manifest.json"
    if not manifest_path.is_file():
        raise ConsumerError(CLASS_INCOMPATIBLE_PAYLOAD, "packet is missing manifest.json")
    manifest = load_json_object(manifest_path, CLASS_INCOMPATIBLE_PAYLOAD)
    if manifest.get("schema") != PACKET_SCHEMA:
        raise ConsumerError(
            CLASS_INCOMPATIBLE_PAYLOAD,
            f"unsupported packet schema {manifest.get('schema')!r}",
        )
    return manifest


def verify_integrity(packet_root: Path, manifest: Mapping[str, Any]) -> dict[str, str]:
    payload = manifest.get("native_payload")
    consumer = manifest.get("consumer")
    if not isinstance(payload, dict) or not isinstance(consumer, dict):
        raise ConsumerError(CLASS_INCOMPATIBLE_PAYLOAD, "manifest is missing payload or consumer")

    relative_binary = require_str(payload, "relative_path", CLASS_INCOMPATIBLE_PAYLOAD)
    expected_binary = require_str(payload, "sha256", CLASS_DIGEST_MISMATCH)
    relative_script = require_str(consumer, "script", CLASS_INCOMPATIBLE_PAYLOAD)
    expected_script = require_str(consumer, "sha256", CLASS_DIGEST_MISMATCH)
    python_requires = require_str(consumer, "python_requires", CLASS_INCOMPATIBLE_PAYLOAD)
    if not python_version_ok(python_requires):
        raise ConsumerError(
            CLASS_ENVIRONMENT_UNAVAILABLE,
            f"python {python_requires} required, found {sys.version.split()[0]}",
        )

    binary = contained_path(packet_root, relative_binary, CLASS_INCOMPATIBLE_PAYLOAD)
    script = contained_path(packet_root, relative_script, CLASS_INCOMPATIBLE_PAYLOAD)
    if not binary.exists():
        raise ConsumerError(CLASS_MISSING_EXECUTABLE, f"packet payload {relative_binary} is missing")
    if not binary.is_file():
        raise ConsumerError(CLASS_MISSING_EXECUTABLE, f"packet payload {relative_binary} is not a file")
    if not os.access(binary, os.X_OK):
        raise ConsumerError(CLASS_MISSING_EXECUTABLE, f"packet payload {relative_binary} is not executable")
    if not script.is_file():
        raise ConsumerError(CLASS_INCOMPATIBLE_PAYLOAD, f"consumer script {relative_script} is missing")

    actual_binary = sha256_file(binary)
    actual_script = sha256_file(script)
    identities = {
        "binary": str(binary),
        "script": str(script),
        "relative_binary": relative_binary,
        "observed_binary_sha256": actual_binary,
        "observed_script_sha256": actual_script,
    }
    if actual_binary != expected_binary:
        raise ConsumerError(
            CLASS_DIGEST_MISMATCH,
            "native payload digest does not match the manifest",
            identities=identities,
        )
    if actual_script != expected_script:
        raise ConsumerError(
            CLASS_DIGEST_MISMATCH,
            "consumer script digest does not match the manifest",
            identities=identities,
        )

    expected_packet = manifest.get("packet_digest")
    actual_packet = packet_digest_from_files(
        {relative_binary: actual_binary, relative_script: actual_script}
    )
    identities["observed_packet_digest"] = actual_packet
    if not isinstance(expected_packet, str) or actual_packet != expected_packet:
        raise ConsumerError(
            CLASS_DIGEST_MISMATCH,
            "packet digest does not match the listed files",
            identities=identities,
        )

    expected_platform = payload.get("platform")
    if isinstance(expected_platform, str) and expected_platform and expected_platform != host_platform():
        raise ConsumerError(
            CLASS_INCOMPATIBLE_PAYLOAD,
            f"payload platform {expected_platform} does not match host {host_platform()}",
        )
    return identities


def render_argv(template: list[Any], mapping: Mapping[str, str]) -> list[str]:
    argv: list[str] = []
    for item in template:
        if not isinstance(item, str):
            raise ConsumerError(CLASS_INCOMPATIBLE_PAYLOAD, "argv template entries must be strings")
        rendered = item
        for key, value in mapping.items():
            rendered = rendered.replace("{" + key + "}", value)
        if "{" in rendered and "}" in rendered:
            raise ConsumerError(CLASS_INCOMPATIBLE_PAYLOAD, f"unresolved argv placeholder in {item!r}")
        argv.append(rendered)
    return argv


def operation_argv(
    manifest: Mapping[str, Any],
    operation: str,
    subject_root: Path,
    out_dir: Path,
    diff: Path | None,
) -> list[str]:
    if operation not in ALLOWED_OPERATIONS:
        raise ConsumerError(CLASS_UNKNOWN_OPERATION, f"operation {operation!r} is not allowlisted")
    declared = manifest.get("operations")
    if isinstance(declared, list) and operation not in declared:
        raise ConsumerError(CLASS_UNKNOWN_OPERATION, f"operation {operation!r} is not in this packet")
    profiles = manifest.get("operation_profiles")
    template: list[Any]
    if isinstance(profiles, dict) and isinstance(profiles.get(operation), dict):
        raw = profiles[operation].get("argv_template")
        if not isinstance(raw, list):
            raise ConsumerError(CLASS_INCOMPATIBLE_PAYLOAD, f"{operation} argv_template must be a list")
        template = raw
    elif operation == "check":
        template = list(DEFAULT_CHECK_ARGV)
    else:
        template = list(DEFAULT_PILOT_ARGV)
    argv = render_argv(
        template,
        {"subject_root": str(subject_root), "out_dir": str(out_dir)},
    )
    if not argv or argv[0] != operation:
        raise ConsumerError(
            CLASS_INCOMPATIBLE_PAYLOAD,
            f"{operation} argv_template must start with {operation!r}",
        )
    if diff is not None:
        if operation != "check":
            raise ConsumerError(
                CLASS_UNKNOWN_OPERATION,
                f"--diff is only valid with check, not {operation!r}",
            )
        argv.extend(["--diff", str(diff)])
    return argv


def ensure_out_dir(out_dir: Path) -> None:
    if out_dir.exists() and not out_dir.is_dir():
        raise ConsumerError(CLASS_UNWRITABLE_OUTPUT, f"output path {out_dir} is not a directory")
    try:
        out_dir.mkdir(parents=True, exist_ok=True)
    except OSError as exc:
        raise ConsumerError(CLASS_UNWRITABLE_OUTPUT, f"output directory is not writable: {exc}") from exc
    probe = out_dir / ".portable-consumer-write-probe"
    if probe.is_symlink():
        raise ConsumerError(CLASS_UNWRITABLE_OUTPUT, "output write probe is a symlink")
    write_bytes_unfollowed(probe, b"ok\n")
    try:
        probe.unlink()
    except OSError as exc:
        raise ConsumerError(CLASS_UNWRITABLE_OUTPUT, f"output directory is not writable: {exc}") from exc
    for name in (STDOUT_BIN, STDERR_BIN, RECEIPT_NAME, PILOT_SUMMARY_NAME):
        path = out_dir / name
        if path.is_symlink():
            raise ConsumerError(
                CLASS_UNWRITABLE_OUTPUT,
                f"output artifact {name} is a symlink",
            )


def clear_stale_pilot_summary(out_dir: Path, operation: str) -> None:
    if operation != "pilot":
        return
    summary = out_dir / PILOT_SUMMARY_NAME
    if summary.is_symlink():
        raise ConsumerError(CLASS_UNWRITABLE_OUTPUT, f"output artifact {PILOT_SUMMARY_NAME} is a symlink")
    if not summary.exists():
        return
    try:
        summary.unlink()
    except OSError as exc:
        raise ConsumerError(
            CLASS_UNWRITABLE_OUTPUT,
            f"cannot clear stale {PILOT_SUMMARY_NAME}: {exc}",
        ) from exc


def terminate_payload(proc: Any) -> None:
    if os.name == "posix":
        try:
            os.killpg(proc.pid, signal.SIGKILL)
            return
        except OSError:
            pass
    proc.kill()


def launch_payload(
    binary: Path,
    argv: list[str],
    cwd: Path,
    timeout_seconds: int,
) -> tuple[int | None, bytes, bytes, str]:
    env = os.environ.copy()
    started = time.monotonic()
    popen_kwargs: dict[str, Any] = {}
    if os.name == "posix":
        popen_kwargs["start_new_session"] = True
    try:
        proc = subprocess.Popen(
            [str(binary), *argv],
            cwd=str(cwd),
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            shell=False,
            **popen_kwargs,
        )
    except OSError as exc:
        raise ConsumerError(
            CLASS_ENVIRONMENT_UNAVAILABLE,
            f"host cannot launch the native payload: {exc}",
        ) from exc
    try:
        stdout, stderr = proc.communicate(timeout=timeout_seconds)
    except subprocess.TimeoutExpired as exc:
        terminate_payload(proc)
        try:
            stdout, stderr = proc.communicate(timeout=5)
        except subprocess.TimeoutExpired:
            stdout = exc.stdout or b""
            stderr = exc.stderr or b""
        elapsed_ms = str(int((time.monotonic() - started) * 1000))
        raise ConsumerError(
            CLASS_TIMEOUT,
            f"payload exceeded {timeout_seconds}s",
            exit_code=proc.returncode,
            stdout=stdout,
            stderr=stderr,
            elapsed_ms=elapsed_ms,
            argv=argv,
        ) from exc
    elapsed_ms = str(int((time.monotonic() - started) * 1000))
    code = proc.returncode
    if code is None:
        raise ConsumerError(
            CLASS_EXECUTION_FAILURE,
            "payload returned no exit code",
            stdout=stdout,
            stderr=stderr,
            elapsed_ms=elapsed_ms,
            argv=argv,
        )
    return code, stdout, stderr, elapsed_ms


def selected_subject_count(value: Mapping[str, Any]) -> tuple[int | None, list[str]]:
    findings = value.get("findings")
    if isinstance(findings, list):
        return len(findings), []
    raw = value.get("selected_subject_count")
    if isinstance(raw, int):
        return raw, []
    return None, [
        "check JSON has no selected/executed denominator; receipt uses findings length when present"
    ]


def classify_analysis_outcome(value: Mapping[str, Any]) -> str | None:
    outcome = value.get("analysis_outcome")
    if not isinstance(outcome, dict):
        return None
    inner = outcome.get("outcome")
    kind = inner.get("kind") if isinstance(inner, dict) else None
    if kind in INCOMPLETE_OUTCOME_KINDS:
        if kind == "partial_with_limitations":
            return CLASS_TYPED_PRODUCT_LIMITATION
        return CLASS_PARTIAL_PRODUCT_OUTPUT
    if outcome.get("analysis_complete") is False:
        return CLASS_PARTIAL_PRODUCT_OUTPUT
    if kind is None or kind in COMPLETE_OUTCOME_KINDS:
        return None
    raise ConsumerError(
        CLASS_MALFORMED_PRODUCT_OUTPUT,
        f"unknown analysis_outcome kind {kind!r}",
    )


def classify_product_json(
    stdout: bytes,
    require_nonzero: bool,
    operation: str = "check",
) -> tuple[str, dict[str, Any] | None, int | None, list[str]]:
    if not stdout.strip():
        return CLASS_PARTIAL_PRODUCT_OUTPUT, None, None, ["stdout was empty"]
    try:
        text = stdout.decode("utf-8")
        value = json.loads(text)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ConsumerError(CLASS_MALFORMED_PRODUCT_OUTPUT, f"product stdout is not JSON: {exc}") from exc
    if not isinstance(value, dict):
        raise ConsumerError(CLASS_MALFORMED_PRODUCT_OUTPUT, "product stdout is not a JSON object")
    version = value.get("schema_version")
    if version != PRODUCT_SCHEMA_VERSION:
        raise ConsumerError(
            CLASS_MALFORMED_PRODUCT_OUTPUT,
            f"product JSON schema_version {version!r} is not {PRODUCT_SCHEMA_VERSION}; consumer does not invent a schema",
        )
    selected, limitations = selected_subject_count(value)
    if operation == "check" and selected is None:
        raise ConsumerError(
            CLASS_MALFORMED_PRODUCT_OUTPUT,
            "check JSON has no findings list or selected_subject_count",
        )
    outcome_class = classify_analysis_outcome(value)
    if outcome_class is not None:
        return outcome_class, value, selected, limitations
    if require_nonzero and (selected is None or selected == 0):
        return CLASS_ZERO_SUBJECTS, value, selected, limitations
    return CLASS_COMPLETE, value, selected, limitations


def product_json_bytes(operation: str, stdout: bytes, out_dir: Path) -> bytes:
    if operation != "pilot":
        return stdout
    summary = out_dir / PILOT_SUMMARY_NAME
    if not summary.is_file():
        raise ConsumerError(
            CLASS_PARTIAL_PRODUCT_OUTPUT,
            f"{PILOT_SUMMARY_NAME} was not produced under {out_dir}",
        )
    try:
        return summary.read_bytes()
    except OSError as exc:
        raise ConsumerError(
            CLASS_PARTIAL_PRODUCT_OUTPUT,
            f"failed to read {PILOT_SUMMARY_NAME}: {exc}",
        ) from exc


def write_bytes_unfollowed(path: Path, data: bytes) -> None:
    flags = os.O_WRONLY | os.O_CREAT | os.O_TRUNC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        fd = os.open(path, flags, 0o644)
    except OSError as exc:
        raise ConsumerError(CLASS_UNWRITABLE_OUTPUT, f"cannot write {path.name}: {exc}") from exc
    try:
        os.write(fd, data)
    except OSError as exc:
        raise ConsumerError(CLASS_UNWRITABLE_OUTPUT, f"cannot write {path.name}: {exc}") from exc
    finally:
        os.close(fd)


def write_receipt(path: Path, body: Mapping[str, Any]) -> None:
    payload = (json.dumps(body, indent=2, sort_keys=True) + "\n").encode("utf-8")
    write_bytes_unfollowed(path, payload)


def receipt_body(
    *,
    classification: str,
    message: str,
    packet_root: Path,
    manifest: Mapping[str, Any] | None,
    identities: Mapping[str, str] | None,
    subject_root: Path | None,
    subject_digest: str | None,
    operation: str,
    argv: list[str] | None,
    cwd: Path | None,
    exit_code: int | None,
    elapsed_ms: str | None,
    stdout: bytes | None,
    stderr: bytes | None,
    selected: int | None,
    limitations: list[str],
) -> dict[str, Any]:
    payload = manifest.get("native_payload") if isinstance(manifest, dict) else {}
    consumer = manifest.get("consumer") if isinstance(manifest, dict) else {}
    observed_binary = identities.get("observed_binary_sha256") if identities else None
    observed_script = identities.get("observed_script_sha256") if identities else None
    observed_packet = identities.get("observed_packet_digest") if identities else None
    return {
        "schema": RECEIPT_SCHEMA,
        "classification": classification,
        "message": message,
        "packet": {
            "root": str(packet_root),
            "schema": manifest.get("schema") if isinstance(manifest, dict) else None,
            "packet_digest": observed_packet,
        },
        "consumer": {
            "script": consumer.get("script") if isinstance(consumer, dict) else "run.py",
            "sha256": observed_script,
            "python": sys.version.split()[0],
        },
        "binary": {
            "path": identities.get("binary") if identities else None,
            "relative_path": identities.get("relative_binary") if identities else None,
            "sha256": observed_binary,
            "version": payload.get("version") if isinstance(payload, dict) else None,
            "build_identity": payload.get("build_identity") if isinstance(payload, dict) else None,
            "source_route": payload.get("source_route") if isinstance(payload, dict) else None,
            "platform": payload.get("platform") if isinstance(payload, dict) else None,
        },
        "subject": {
            "root": str(subject_root) if subject_root else None,
            "digest": subject_digest,
        },
        "operation": operation,
        "argv": argv or [],
        "cwd": str(cwd) if cwd else None,
        "exit_code": exit_code,
        "elapsed_ms": int(elapsed_ms) if elapsed_ms is not None else None,
        "stdout_digest": sha256_bytes(stdout) if stdout is not None else None,
        "stderr_digest": sha256_bytes(stderr) if stderr is not None else None,
        "selected_subject_count": selected,
        "executed_subject_count": None,
        "limitations": limitations,
        "non_claims": list(NON_CLAIMS),
    }


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Verify and launch a portable native ripr packet without PATH or compiler fallback."
    )
    parser.add_argument("--packet", required=True, help="Extracted packet directory")
    parser.add_argument("--subject-root", required=True, help="Explicit analyzed root")
    parser.add_argument("--out", required=True, help="Writable output directory for receipts and artifacts")
    parser.add_argument("--operation", default="check", help="Allowlisted operation (check or pilot)")
    parser.add_argument("--diff", help="Optional explicit diff path passed through to check")
    parser.add_argument("--timeout-seconds", type=int, default=0, help="Payload timeout; 0 uses the manifest default")
    parser.add_argument("--foreign-cwd", help="Process working directory; defaults to the subject root")
    parser.add_argument("--subject-digest", help="Expected subject tree digest; fail closed on drift")
    parser.add_argument(
        "--require-nonzero-subjects",
        action="store_true",
        help="Fail closed when product JSON reports zero findings/subjects",
    )
    return parser.parse_args(argv)


def resolve_timeout(manifest: Mapping[str, Any], requested: int) -> int:
    if requested > 0:
        return requested
    raw_timeout = manifest.get("timeout_seconds_default", DEFAULT_TIMEOUT_SECONDS)
    if isinstance(raw_timeout, int) and raw_timeout > 0:
        return raw_timeout
    return DEFAULT_TIMEOUT_SECONDS


def run_attempt(
    attempt: Attempt,
    args: argparse.Namespace,
    packet_root: Path,
    subject_root: Path,
    out_dir: Path,
    cwd: Path,
    diff: Path | None,
) -> None:
    if not packet_root.is_dir():
        raise ConsumerError(CLASS_INCOMPATIBLE_PAYLOAD, f"packet {packet_root} is not a directory")
    if not subject_root.is_dir():
        raise ConsumerError(CLASS_INCOMPATIBLE_PAYLOAD, f"subject root {subject_root} is not a directory")
    ensure_out_dir(out_dir)
    clear_stale_pilot_summary(out_dir, args.operation)
    attempt.manifest = load_manifest(packet_root)
    attempt.identities = verify_integrity(packet_root, attempt.manifest)
    if args.subject_digest:
        attempt.subject_digest = tree_digest(subject_root)
        if attempt.subject_digest != args.subject_digest:
            raise ConsumerError(CLASS_SUBJECT_IDENTITY_DRIFT, "subject tree digest does not match")
    timeout = resolve_timeout(attempt.manifest, args.timeout_seconds)
    attempt.argv = operation_argv(attempt.manifest, args.operation, subject_root, out_dir.resolve(), diff)
    binary = Path(attempt.identities["binary"])
    attempt.exit_code, attempt.stdout, attempt.stderr, attempt.elapsed_ms = launch_payload(
        binary,
        attempt.argv,
        cwd,
        timeout,
    )
    if args.subject_digest:
        after = tree_digest(subject_root)
        if after != args.subject_digest:
            attempt.subject_digest = after
            raise ConsumerError(
                CLASS_SUBJECT_IDENTITY_DRIFT,
                "subject tree digest changed during payload execution",
                exit_code=attempt.exit_code,
                stdout=attempt.stdout,
                stderr=attempt.stderr,
                elapsed_ms=attempt.elapsed_ms,
                argv=attempt.argv,
            )
    if attempt.exit_code != 0:
        raise ConsumerError(
            CLASS_EXECUTION_FAILURE,
            f"payload exited {attempt.exit_code}",
            exit_code=attempt.exit_code,
            stdout=attempt.stdout,
            stderr=attempt.stderr,
            elapsed_ms=attempt.elapsed_ms,
            argv=attempt.argv,
        )
    product = product_json_bytes(args.operation, attempt.stdout or b"", out_dir.resolve())
    classification, _product, selected, extra = classify_product_json(
        product,
        args.require_nonzero_subjects,
        args.operation,
    )
    attempt.classification = classification
    attempt.selected = selected
    attempt.limitations.extend(extra)
    attempt.message = COMPLETE_MESSAGES.get(classification, classification)
    if classification != CLASS_COMPLETE:
        raise ConsumerError(
            classification,
            attempt.message,
            exit_code=attempt.exit_code,
            stdout=attempt.stdout,
            stderr=attempt.stderr,
            elapsed_ms=attempt.elapsed_ms,
            argv=attempt.argv,
            selected=selected,
            limitations=extra,
        )


def consume(args: argparse.Namespace) -> int:
    packet_root = Path(args.packet).expanduser()
    subject_root = Path(args.subject_root).expanduser().resolve()
    out_dir = Path(args.out).expanduser()
    cwd = Path(args.foreign_cwd).expanduser().resolve() if args.foreign_cwd else subject_root
    diff = Path(args.diff).expanduser().resolve() if args.diff else None
    attempt = Attempt()
    try:
        run_attempt(attempt, args, packet_root, subject_root, out_dir, cwd, diff)
    except ConsumerError as exc:
        attempt.absorb_error(exc)
    except Exception as exc:  # last-resort environment boundary; still emit a receipt
        attempt.classification = CLASS_ENVIRONMENT_UNAVAILABLE
        attempt.message = f"consumer failed before launch: {exc}"

    receipt_path = out_dir / RECEIPT_NAME
    body = receipt_body(
        classification=attempt.classification,
        message=attempt.message,
        packet_root=packet_root,
        manifest=attempt.manifest,
        identities=attempt.identities,
        subject_root=subject_root,
        subject_digest=attempt.subject_digest,
        operation=args.operation,
        argv=attempt.argv,
        cwd=cwd,
        exit_code=attempt.exit_code,
        elapsed_ms=attempt.elapsed_ms,
        stdout=attempt.stdout,
        stderr=attempt.stderr,
        selected=attempt.selected,
        limitations=attempt.limitations,
    )
    try:
        ensure_out_dir(out_dir)
        if attempt.stdout is not None:
            write_bytes_unfollowed(out_dir / STDOUT_BIN, attempt.stdout)
        if attempt.stderr is not None:
            write_bytes_unfollowed(out_dir / STDERR_BIN, attempt.stderr)
        write_receipt(receipt_path, body)
    except ConsumerError:
        sys.stderr.write(f"{attempt.classification}: {attempt.message}\n")
        return 2
    except OSError as exc:
        sys.stderr.write(f"{CLASS_UNWRITABLE_OUTPUT}: {exc}\n")
        return 2

    sys.stderr.write(f"{attempt.classification}: {attempt.message}\n")
    return 0 if attempt.classification == CLASS_COMPLETE else 1


def main() -> int:
    return consume(parse_args(sys.argv[1:]))


if __name__ == "__main__":
    sys.exit(main())
