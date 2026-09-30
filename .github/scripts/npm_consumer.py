"""Clean npm tarball consumer proof; requires no source checkout or Rust.

Only installed native bytes execute. The resulting JSON keeps denominators and
names each tested route instead of treating a package upload as useful delivery.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import select
import time
import subprocess
import tarfile
import tempfile


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def command(args, cwd, env, *, success=True, data=None):
    result = subprocess.run([str(arg) for arg in args], cwd=cwd, env=env, input=data,
                            capture_output=True, text=True, timeout=120)
    if success:
        require(result.returncode == 0, f"command failed ({result.returncode}): {args}\n{result.stdout}\n{result.stderr}")
    return result


def useful_journey(prefix, project, env, output, route):
    args = ["check", "--root", project, "--diff", project / "diff.patch", "--mode", "fast", "--json"]
    result = command([*prefix, *args], output, env)
    report = json.loads(result.stdout)
    require(report["summary"]["probes"] > 0 and report["summary"]["findings"] > 0 and report["findings"], "empty analysis denominator")
    require(any(row["language"] == "python" and row["enabled"] and row["analyzed"] for row in report["preview_languages"]), "Python preview did not run")
    require(report["findings"][0]["language"] == "python", "unexpected finding language")
    finding = report["findings"][0]["id"]
    explanation = command([*prefix, "explain", "--root", project, "--diff", project / "diff.patch", "--mode", "fast", finding], output, env)
    require(finding in explanation.stdout, "follow-up did not explain the real finding")
    (output / f"{route}-check.json").write_text(result.stdout)
    (output / f"{route}-explain.txt").write_text(explanation.stdout)
    return {"route": route, "probes": report["summary"]["probes"], "findings": report["summary"]["findings"], "follow_up_finding": finding}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("artifact", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    artifact = args.artifact.resolve()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    receipt = json.loads((artifact / "package-receipt.json").read_text())
    pin = receipt["provenance"]["product_source"]
    tarball = artifact / "tarballs" / receipt["tarball"]["filename"]
    require(sha(tarball) == receipt["tarball"]["sha256"], "tarball transfer digest mismatch")
    require(pin["native_version"] == receipt["version"], "receipt version mismatch")
    node_path = shutil.which("node")
    require(node_path is not None, "node executable not found in PATH")
    node = Path(node_path).resolve()
    npm_path = shutil.which("npm")
    require(npm_path is not None, "npm executable not found in PATH")
    npm = Path(npm_path).resolve()
    with tempfile.TemporaryDirectory(prefix="ripr-npm-consumer-") as temporary:
        root = Path(temporary)
        tools = root / "tools"; tools.mkdir()
        for name in ("node", "git", "sh"):
            source = shutil.which(name)
            if source: (tools / name).symlink_to(Path(source).resolve())
        planted = root / "planted"; planted.mkdir()
        marker = root / "wrong-ripr"
        (planted / "ripr").write_text(f'#!/bin/sh\nprintf wrong > "{marker}"\nexit 97\n')
        (planted / "ripr").chmod(0o755)
        home = root / "home"; home.mkdir()
        env = {"HOME": str(home), "PATH": str(tools) + os.pathsep + str(planted), "LANG": "C.UTF-8", "npm_config_cache": str(root / "cache"), "npm_config_userconfig": str(root / "empty.npmrc"), "npm_config_audit": "false", "npm_config_fund": "false", "npm_config_update_notifier": "false", "npm_config_registry": "http://127.0.0.1:9", "HTTP_PROXY": "http://127.0.0.1:9", "HTTPS_PROXY": "http://127.0.0.1:9", "NO_PROXY": ""}
        (root / "empty.npmrc").write_text("")
        require(shutil.which("cargo", path=env["PATH"]) is None and shutil.which("rustc", path=env["PATH"]) is None, "Rust leaked into consumer PATH")
        project = root / "project ü space"
        shutil.copytree(artifact / "fixture", project)
        project_python = project / ".venv/bin/python"; project_python.parent.mkdir(parents=True)
        project_marker = root / "project-python-executed"
        project_python.write_text(f'#!/bin/sh\nprintf wrong > "{project_marker}"\nexit 98\n'); project_python.chmod(0o755)
        npm_command = [node, npm]
        proof = {"node": command([node, "--version"], root, env).stdout.strip(), "npm": command([*npm_command, "--version"], root, env).stdout.strip(), "tarball_sha256": sha(tarball), "payload_sha256": pin["payload_sha256"], "routes": [], "negative_controls": []}
        require(int(proof["npm"].split(".")[0]) >= 10, "unsupported npm client")
        local = root / "local"; local.mkdir()
        (local / "package.json").write_text('{"name":"ripr-clean-consumer","version":"0.0.0","private":true}')
        command([*npm_command, "install", "--ignore-scripts", "--offline", "--no-audit", "--no-fund", tarball], local, env)
        local_binary = local / "node_modules/ripr/bin/ripr"
        require(sha(local_binary) == pin["payload_sha256"], "local native bytes mismatch")
        require((local / "node_modules/.bin/ripr").resolve() == local_binary, "npm did not link native executable directly")
        local_prefix = [local / "node_modules/.bin/ripr"]
        require(command([*local_prefix, "--version"], root, env).stdout.strip() == "ripr " + receipt["version"], "native version mismatch")
        proof["routes"].append(useful_journey(local_prefix, project, env, output, "local"))
        global_root = root / "global"
        command([*npm_command, "install", "--global", "--prefix", global_root, "--ignore-scripts", "--offline", "--no-audit", "--no-fund", tarball], root, env)
        global_binary = global_root / "bin/ripr"
        require(sha(global_binary) == pin["payload_sha256"], "global native bytes mismatch")
        global_env = {**env, "PATH": str(global_root / "bin") + os.pathsep + env["PATH"]}
        proof["routes"].append(useful_journey(["ripr"], project, global_env, output, "global"))
        npx_prefix = [*npm_command, "exec", "--yes", "--ignore-scripts", "--offline", "--package=" + str(tarball), "--", "ripr"]
        proof["routes"].append(useful_journey(npx_prefix, project, env, output, "npm-exec"))
        npx_cli = npm.with_name("npx-cli.js")
        require(npx_cli.is_file(), "npx CLI is unavailable")
        actual_npx = [node, npx_cli, "--yes", "--ignore-scripts", "--offline", "--package=" + str(tarball), "ripr"]
        proof["routes"].append(useful_journey(actual_npx, project, env, output, "npx"))
        npx_files = list((root / "cache/_npx").glob("*/node_modules/ripr/bin/ripr"))
        require(len(npx_files) == 1 and sha(npx_files[0]) == pin["payload_sha256"], "npm exec payload identity missing")
        # Exchange frames sequentially: pipelining exit can correctly terminate
        # the server before asynchronous request responses are written.
        with (output / "lsp-stderr.txt").open("wb") as stderr:
            server = subprocess.Popen([str(global_binary), "lsp", "--stdio"], cwd=root, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=stderr)
            pending = bytearray()
            def send(message):
                body = json.dumps(message).encode()
                server.stdin.write(f"Content-Length: {len(body)}\r\n\r\n".encode() + body)
                server.stdin.flush()
            def response(wanted):
                deadline = time.monotonic() + 20
                while True:
                    while b"\r\n\r\n" not in pending:
                        require(time.monotonic() < deadline, "LSP response timed out")
                        ready, _, _ = select.select([server.stdout], [], [], 1)
                        if ready:
                            chunk = os.read(server.stdout.fileno(), 65536)
                            require(chunk, "LSP exited before response")
                            pending.extend(chunk)
                    header, rest = bytes(pending).split(b"\r\n\r\n", 1)
                    require(header.startswith(b"Content-Length: "), "non-protocol LSP stdout")
                    size = int(header.split(b": ", 1)[1])
                    while len(rest) < size:
                        require(time.monotonic() < deadline, "LSP body timed out")
                        ready, _, _ = select.select([server.stdout], [], [], 1)
                        if ready:
                            chunk = os.read(server.stdout.fileno(), 65536)
                            require(chunk, "LSP body truncated")
                            rest += chunk
                    pending[:] = rest[size:]
                    message = json.loads(rest[:size])
                    if message.get("id") == wanted:
                        require("result" in message, "LSP request error: " + json.dumps(message))
                        return message
            try:
                send({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"processId": None, "rootUri": None, "capabilities": {}}})
                response(1)
                send({"jsonrpc": "2.0", "method": "initialized", "params": {}})
                send({"jsonrpc": "2.0", "id": 2, "method": "shutdown", "params": None})
                response(2)
                send({"jsonrpc": "2.0", "method": "exit", "params": None})
                server.stdin.close()
                require(server.wait(timeout=20) == 0, "LSP exit failed")
            finally:
                if server.poll() is None:
                    server.terminate()
                    server.wait(timeout=10)
        proof["negative_controls"].append("piped LSP initialize/shutdown preserves clean framed stdout")
        failure = command([global_binary, "--not-a-ripr-command"], root, env, success=False)
        require(failure.returncode != 0, "nonzero native failure lost")
        proof["negative_controls"].append("invalid command returns nonzero")
        require(not marker.exists() and not project_marker.exists(), "ambient command was executed")
        proof["negative_controls"].append("planted PATH and project Python never execute")
        (project / "ripr.toml").write_text('[languages]\nenabled = ["rust"]\n')
        disabled = json.loads(command([global_binary, "check", "--root", project, "--diff", project / "diff.patch", "--mode", "fast", "--json"], root, env).stdout)
        require(disabled["summary"]["probes"] == 0 and disabled["summary"]["findings"] == 0 and not disabled["analysis_outcome"]["analysis_complete"], "disabled preview looked complete")
        proof["negative_controls"].append("explicit preview disablement retains incomplete status")
        # Mutate desired platform metadata, not CLI --os/--cpu/--libc overrides:
        # npm's nonoptional install path uses actual host platform authority.
        with tarfile.open(tarball, "r:gz") as archive:
            members = archive.getmembers()
            require(all(m.isfile() and m.name.startswith("package/") and ".." not in Path(m.name).parts for m in members), "unsafe platform-control input")
            contents = {m.name: archive.extractfile(m).read() for m in members}
        for key, value in (("os", ["darwin"]), ("cpu", ["arm64"]), ("libc", ["musl"])):
            stage = root / ("wrong-" + key); stage.mkdir()
            for name, body in contents.items():
                path = stage / name.removeprefix("package/"); path.parent.mkdir(parents=True, exist_ok=True); path.write_bytes(body)
            package = json.loads((stage / "package.json").read_text()); package[key] = value
            (stage / "package.json").write_text(json.dumps(package))
            packed = json.loads(command([*npm_command, "pack", "--ignore-scripts", "--json"], stage, env).stdout)
            foreign = root / ("consumer-" + key); foreign.mkdir()
            attempt = command([*npm_command, "install", "--ignore-scripts", "--offline", "--no-audit", "--no-fund", stage / packed[0]["filename"]], foreign, env, success=False)
            require(attempt.returncode != 0 and "EBADPLATFORM" in attempt.stderr, "wrong platform was accepted: " + key)
            proof["negative_controls"].append("npm rejects contradictory " + key + " metadata")
        command([*npm_command, "install", "--global", "--prefix", global_root, "--ignore-scripts", "--offline", "--no-audit", "--no-fund", tarball], root, env)
        require(sha(global_binary) == pin["payload_sha256"], "reinstall native bytes mismatch")
        command([*npm_command, "uninstall", "--global", "--prefix", global_root, "--ignore-scripts", "ripr"], root, env)
        require(not global_binary.exists() and (project / "src/pricing.py").is_file(), "uninstall contract failed")
        proof["negative_controls"].append("reinstall preserves bytes; uninstall preserves project")
        require(len(proof["routes"]) == 4, "route denominator incomplete")
        proof.update({"schema_version": 1, "state": "passed", "selected_routes": 4, "executed_routes": 4, "failed_routes": 0})
        (output / "consumer.json").write_text(json.dumps(proof, indent=2) + "\n")
        print(json.dumps(proof, indent=2))


if __name__ == "__main__":
    main()
