#!/usr/bin/env python3
"""Retain one Cargo-reported xtask image; observe read-only without rebuilding."""
import hashlib
import io
import json
import os
from pathlib import Path
import re
import selectors
import signal
import stat
import subprocess
import sys
import tempfile
import time
from types import SimpleNamespace

ROOT = Path(__file__).resolve().parents[2]
SELF = ".github/scripts/native_readiness_artifact.py"
INPUTS = (SELF, ".github/workflows/ci.yml", ".cargo/config.toml", "Cargo.toml",
          "Cargo.lock", "xtask/Cargo.toml", "xtask/src/main.rs", "crates/ripr/Cargo.toml",
          "xtask/src/run/hard_enforcement_readiness.rs", "xtask/src/reports/ci_budget.rs")
ENV = dict(os.environ, GIT_NO_LAZY_FETCH="1", GIT_OPTIONAL_LOCKS="0")
BUILD = ("cargo", "build", "--locked", "-p", "xtask", "--message-format=json-render-diagnostics")
OBSERVE = ("ci-budget", "--hard-enforcement-readiness")
REPORT = ROOT / "target/ripr/reports/native-calibration-observer-artifact.json"
# Hash only the initially observed existing image extent; retain no data copy.
CAPTURE_CAP = 8 * 1024 * 1024
RECEIPT_CAP = 64 * 1024
UNTIL = 0.0


def sha(data):
    return hashlib.sha256(data).hexdigest()


def interrupt(number, _frame):
    raise RuntimeError(f"catchable signal {number}: artifact route interrupted")


def regular(path, cap):
    if path.is_symlink() or not stat.S_ISREG(path.stat().st_mode):
        raise RuntimeError(f"not a regular admitted file: {path}")
    with path.open("rb") as stream:
        data = stream.read(cap + 1)
    if len(data) > cap:
        raise RuntimeError(f"read ceiling exceeded: {path}")
    return data


def capture(command, cap, seconds, forward=False):
    """Bound streams before retention; process-group cleanup is not a hard provider."""
    deadline = min(UNTIL, time.monotonic() + seconds)
    if deadline <= time.monotonic():
        raise RuntimeError("artifact route deadline reached before spawn")
    child = subprocess.Popen(command, cwd=ROOT, env=ENV, stdin=subprocess.DEVNULL,
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                             start_new_session=True)
    selector = selectors.DefaultSelector()
    output = bytearray()
    total = 0
    try:
        selector.register(child.stdout, selectors.EVENT_READ, "stdout")
        selector.register(child.stderr, selectors.EVENT_READ, "stderr")
        while selector.get_map():
            if time.monotonic() >= deadline:
                raise RuntimeError("artifact command deadline reached")
            for key, _events in selector.select(min(0.2, max(0, deadline - time.monotonic()))):
                chunk = os.read(key.fileobj.fileno(), min(65536, cap - total + 1))
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                if total + len(chunk) > cap:
                    raise RuntimeError("artifact command output ceiling reached before retention")
                total += len(chunk)
                if key.data == "stdout":
                    output.extend(chunk)
                if forward or key.data == "stderr":
                    destination = sys.stdout.buffer if key.data == "stdout" else sys.stderr.buffer
                    destination.write(chunk)
                    destination.flush()
        status = child.wait(timeout=max(0.01, deadline - time.monotonic()))
        if status != 0:
            raise RuntimeError(f"artifact command failed: exit={status}; command={command[0]}")
        return bytes(output)
    finally:
        selector.close()
        try:
            os.killpg(child.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        child.wait(timeout=5)
        child.stdout.close()
        child.stderr.close()


def git(args, cap=65536):
    return capture(("git", "--no-replace-objects", *args), cap, 10)


def source():
    if sys.platform != "linux" or Path.cwd().resolve() != ROOT:
        raise RuntimeError("artifact route requires its owned Linux checkout")
    head = git(("rev-parse", "HEAD"), 128).decode("ascii").strip()
    tree = git(("rev-parse", "HEAD^{tree}"), 128).decode("ascii").strip()
    if not re.fullmatch("[0-9a-f]{40}", head) or not re.fullmatch("[0-9a-f]{40}", tree):
        raise RuntimeError("source commit/tree identity missing")
    if git(("status", "--porcelain=v1", "-z", "--untracked-files=normal")):
        raise RuntimeError("checkout is not clean for observer image admission")
    inputs = {}
    for name in INPUTS:
        data = regular(ROOT / name, 2 * 1024 * 1024)
        if data != git(("show", f"{head}:{name}"), 2 * 1024 * 1024):
            raise RuntimeError(f"source differs from HEAD: {name}")
        inputs[name] = {"bytes": len(data), "sha256": sha(data)}
    return {"head": head, "tree": tree, "inputs": inputs}


def same_source(expected):
    if source() != expected:
        raise RuntimeError("observer source identity drift")


def select_artifact(messages):
    candidates = []
    for row in messages:
        target = row.get("target") or {}
        if row.get("reason") == "compiler-artifact" and target.get("name") == "xtask" \
                and target.get("kind") == ["bin"] and row.get("executable"):
            candidates.append(row)
    if len(candidates) != 1:
        raise RuntimeError("exactly one normal xtask compiler artifact required")
    row = candidates[0]
    if (row.get("profile") or {}).get("test") is not False \
            or Path(row["manifest_path"]).resolve() != ROOT / "xtask/Cargo.toml" \
            or Path(row["target"]["src_path"]).resolve() != ROOT / "xtask/src/main.rs" \
            or row.get("features") != []:
        raise RuntimeError("xtask artifact target/profile/manifest/features mismatch")
    return row


def require_digest(actual, expected):
    if not re.fullmatch("[0-9a-f]{64}", expected) or actual != expected:
        raise RuntimeError("retained artifact digest mismatch")


def controls():
    """Pure selector/digest/extent/inode guards; no compiler or image execution."""
    extent_controls()
    row = {"reason": "compiler-artifact", "target": {"name": "xtask", "kind": ["bin"],
           "src_path": str(ROOT / "xtask/src/main.rs")}, "manifest_path": str(ROOT / "xtask/Cargo.toml"),
           "profile": {"test": False}, "features": [], "executable": "/unused/control"}
    if select_artifact([row]) != row:
        raise RuntimeError("artifact selector positive control failed")
    bad_profile = dict(row, profile={"test": True})
    bad_manifest = dict(row, manifest_path=str(ROOT / "Cargo.toml"))
    for rows in ([], [row, row], [bad_profile], [bad_manifest]):
        try:
            select_artifact(rows)
        except RuntimeError:
            pass
        else:
            raise RuntimeError("artifact selector negative control falsely accepted")
    require_digest(sha(b"owned"), sha(b"owned"))
    try:
        require_digest(sha(b"drift"), sha(b"owned"))
    except RuntimeError:
        return
    raise RuntimeError("artifact digest drift control falsely accepted")


def runner_temp():
    base = Path(os.environ["RUNNER_TEMP"])
    if not base.is_absolute() or base.is_symlink() or not base.is_dir():
        raise RuntimeError("RUNNER_TEMP is not an owned absolute directory")
    return base.resolve()


def image_identity(info):
    # ctime/nlink are intentionally not cross-step identity: adding/removing
    # OTHER hard-link names changes them without changing this image.
    return {"device": info.st_dev, "inode": info.st_ino, "bytes": info.st_size,
            "mtime_ns": info.st_mtime_ns, "mode": info.st_mode}


def require_stable_read(before, after):
    if image_identity(before) != image_identity(after) or before.st_ctime_ns != after.st_ctime_ns:
        raise RuntimeError("observer image metadata changed during digest read")


def digest_extent(stream, extent):
    if not isinstance(extent, int) or isinstance(extent, bool) or extent <= 0:
        raise RuntimeError("observer image extent must be a positive observed size")
    result = hashlib.sha256()
    count = 0
    while count < extent:
        if time.monotonic() >= UNTIL:
            raise RuntimeError("observer image hash deadline reached")
        chunk = stream.read(min(65536, extent - count))
        if not chunk:
            raise RuntimeError("observer image ended before observed extent")
        count += len(chunk)
        if count > extent:
            raise RuntimeError("observer image exceeded observed extent")
        result.update(chunk)
    if stream.read(1):
        raise RuntimeError("observer image grew beyond observed extent")
    return {"bytes": count, "sha256": result.hexdigest()}


def image_hash(path):
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    with os.fdopen(descriptor, "rb") as stream:
        before = os.fstat(stream.fileno())
        if not stat.S_ISREG(before.st_mode):
            raise RuntimeError("retained executable is not a regular file")
        result = digest_extent(stream, before.st_size)
        after = os.fstat(stream.fileno())
    require_stable_read(before, after)
    result.update(image_identity(before))
    return result


def extent_controls():
    # Virtual metadata may exceed the former arbitrary 256MiB ceiling
    # without allocating that extent; real digest controls use six bytes.
    before = SimpleNamespace(st_dev=7, st_ino=19, st_size=268435457,
                             st_mtime_ns=23, st_ctime_ns=31, st_nlink=1, st_mode=0o100755)
    linked = SimpleNamespace(**vars(before))
    linked.st_ctime_ns = 41
    linked.st_nlink = 2
    if image_identity(before) != image_identity(linked):
        raise RuntimeError("harmless link metadata changed pinned identity")
    if digest_extent(io.BytesIO(b"owned!"), 6) != {"bytes": 6, "sha256": sha(b"owned!")}:
        raise RuntimeError("observed-extent digest positive control failed")
    for data, extent, expected in ((b"owned!", 0, "positive observed size"),
                                  (b"owned!", 7, "ended before"),
                                  (b"owned!", 5, "grew beyond")):
        try:
            digest_extent(io.BytesIO(data), extent)
        except RuntimeError as error:
            if expected not in str(error):
                raise RuntimeError("extent control produced a wrong refusal") from error
        else:
            raise RuntimeError("extent guard falsely accepted")
    require_stable_read(before, SimpleNamespace(**vars(before)))
    for field in ("st_dev", "st_ino", "st_size", "st_mtime_ns", "st_ctime_ns", "st_mode"):
        drift = SimpleNamespace(**vars(before))
        setattr(drift, field, getattr(drift, field) + 1)
        try:
            require_stable_read(before, drift)
        except RuntimeError as error:
            if "metadata changed during" not in str(error):
                raise RuntimeError("metadata control produced a wrong refusal") from error
        else:
            raise RuntimeError("metadata guard falsely accepted")


def publish(receipt):
    data = (json.dumps(receipt, sort_keys=True, indent=2) + "\n").encode("utf-8")
    if len(data) > RECEIPT_CAP:
        raise RuntimeError("producer receipt byte ceiling exceeded")
    REPORT.parent.mkdir(parents=True, exist_ok=True)
    REPORT.write_bytes(data)
    return data


def prepare():
    controls()
    admitted = source()
    started = time.monotonic()
    messages = capture(BUILD, CAPTURE_CAP, 90 * 60)
    same_source(admitted)
    rows = [json.loads(line) for line in messages.splitlines() if line.strip()]
    artifact = select_artifact(rows)
    original = Path(artifact["executable"])
    if not original.is_absolute():
        raise RuntimeError("Cargo executable path must be absolute")
    identity = image_hash(original)
    directory = Path(tempfile.mkdtemp(prefix="ripr-native-observer-", dir=runner_temp()))
    image = directory / "xtask"
    original_stat = os.stat(original, follow_symlinks=False)
    if image_identity(original_stat) != {key: identity[key] for key in
                                        ("device", "inode", "bytes", "mtime_ns", "mode")}:
        raise RuntimeError("Cargo image changed before hard-link retention")
    try:
        os.link(original, image, follow_symlinks=False)
    except OSError as error:
        raise RuntimeError("exclusive same-filesystem observer hard link unavailable; no copy fallback") from error
    # Exclusive os.link does not overwrite or copy data, chmod, strip or rebuild.
    # It pins existing blocks; it is not an immutable snapshot of in-place writes.
    if image_hash(image) != identity or image_hash(original) != identity:
        raise RuntimeError("hard-linked image differs from Cargo artifact")
    linked_stat = os.stat(image, follow_symlinks=False)
    if linked_stat.st_blocks != original_stat.st_blocks or linked_stat.st_mode != original_stat.st_mode:
        raise RuntimeError("hard-link retention changed existing image allocation or mode")
    same_source(admitted)
    receipt = {"schema": "native_observer_artifact.1", "source": admitted,
               "artifact_root": str(directory), "image": identity,
               "physical_retention": {"method": "exclusive_hard_link", "added_image_data_bytes": 0,
                    "original_extent_bytes": identity["bytes"],
                    "pinned_existing_allocation_bytes": linked_stat.st_blocks * 512,
                    "source_mode_unchanged": linked_stat.st_mode == original_stat.st_mode,
                    "ctime_nlink_note": "Other-name link/unlink metadata may change; bytes/inode/extent/mtime remain bound.",
                    "snapshot_semantics": "NOT_IMMUTABLE; in-place modification refuses at sampled boundaries",
                    "metadata_logical_forecast_bytes": 2 * RECEIPT_CAP + 65536},
               "cargo_artifact": artifact, "build_argv": list(BUILD),
               "build_elapsed_seconds": time.monotonic() - started,
               "runner_observation": {key: os.environ.get(key) for key in
                    ("RUNNER_OS", "RUNNER_ARCH", "RUNNER_NAME", "ImageOS", "ImageVersion", "GITHUB_SHA")},
               "controls": "pure_selector_digest_extent_inode_passed",
               "observation": "NOT_RUN", "full_trial": "NOT_RUN", "hard_provider": "NOT_ESTABLISHED"}
    data = publish(receipt)
    (directory / "producer.json").write_bytes(data)
    if any(character in str(directory) for character in "\r\n"):
        raise RuntimeError("artifact output path contains a line delimiter")
    with Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8") as output:
        output.write(f"artifact-root={directory}\nreceipt-sha256={sha(data)}\n")
    print(f"Retained Cargo-reported observer image: bytes={identity['bytes']} sha256={identity['sha256']}")


def observe():
    controls()
    directory = Path(os.environ["RIPR_OBSERVER_ARTIFACT_ROOT"])
    if not directory.is_absolute() or directory.is_symlink() or directory.parent != runner_temp() \
            or not directory.name.startswith("ripr-native-observer-") or not directory.is_dir():
        raise RuntimeError("retained artifact root is outside the exclusive RUNNER_TEMP owner")
    data = regular(directory / "producer.json", RECEIPT_CAP)
    require_digest(sha(data), os.environ["RIPR_OBSERVER_RECEIPT_SHA256"])
    receipt = json.loads(data)
    if receipt.get("schema") != "native_observer_artifact.1" or receipt.get("artifact_root") != str(directory) \
            or receipt.get("build_argv") != list(BUILD):
        raise RuntimeError("producer receipt identity mismatch")
    same_source(receipt["source"])
    image = directory / "xtask"
    if image_hash(image) != receipt["image"]:
        raise RuntimeError("retained image drift before observation")
    started = time.monotonic()
    observed = capture((str(image), *OBSERVE), 128 * 1024, 55, forward=True)
    if image_hash(image) != receipt["image"]:
        raise RuntimeError("retained image drift after observation")
    same_source(receipt["source"])
    begin = b"BEGIN RIPR NATIVE CALIBRATION READINESS\n"
    end = b"END RIPR NATIVE CALIBRATION READINESS\n"
    if observed.count(begin) != 1 or observed.count(end) != 1:
        raise RuntimeError("actual observer report markers missing or duplicated")
    start = observed.index(begin) + len(begin)
    stop = observed.index(end)
    if stop <= start:
        raise RuntimeError("actual observer report markers out of order")
    report = regular(ROOT / "target/ripr/reports/native-calibration-readiness.json", RECEIPT_CAP)
    if observed[start:stop] != report:
        raise RuntimeError("observer stdout and retained report bytes differ")
    value = json.loads(report)
    if value.get("status") != "NOT_READY" or value.get("full_trial") != "NOT_RUN" \
            or value.get("compiled_observer_sha256") != receipt["source"]["inputs"][
                "xtask/src/run/hard_enforcement_readiness.rs"]["sha256"]:
        raise RuntimeError("observer capability/source conclusion mismatches the admitted image")
    receipt["observer_report"] = {"bytes": len(report), "sha256": sha(report)}
    receipt["observation"] = "READ_ONLY_COMMAND_COMPLETED"
    receipt["observer_argv"] = [str(image), *OBSERVE]
    receipt["observer_elapsed_seconds"] = time.monotonic() - started
    receipt["hard_provider"] = "NOT_ESTABLISHED"
    receipt["full_trial"] = "NOT_RUN"
    publish(receipt)


def main():
    global UNTIL
    if len(sys.argv) != 2 or sys.argv[1] not in ("prepare", "observe"):
        raise RuntimeError("expected prepare or observe")
    UNTIL = time.monotonic() + (90 * 60 if sys.argv[1] == "prepare" else 55)
    for number in (signal.SIGTERM, signal.SIGINT):
        signal.signal(number, interrupt)
    (prepare if sys.argv[1] == "prepare" else observe)()


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"Native observer artifact refusal: {error}", file=sys.stderr)
        sys.exit(1)
