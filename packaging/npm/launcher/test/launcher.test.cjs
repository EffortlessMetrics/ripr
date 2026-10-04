"use strict";

const assert = require("node:assert/strict");
const { EventEmitter } = require("node:events");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawn, spawnSync } = require("node:child_process");
const test = require("node:test");
const { createRequire } = require("node:module");

const launcher = require("../lib/launcher.cjs");
const manifest = require("../package.json");

function fixtureRoot(name) {
  return fs.mkdtempSync(path.join(os.tmpdir(), `ripr-npm-${name}-`));
}

function writeJson(filePath, value) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`);
}

function launcherManifest(overrides = {}) {
  return { ...structuredClone(manifest), ...overrides };
}

function platformRow(packageName = "@effortlessmetrics/ripr-linux-x64-gnu") {
  return manifest.ripr.platforms.find((entry) => entry.package === packageName);
}

function nativeFixture(options = {}) {
  const root = fixtureRoot("native");
  const launcherRoot = path.join(root, "launcher");
  const selected = options.selected || platformRow();
  writeJson(path.join(launcherRoot, "package.json"), manifest);
  fs.mkdirSync(path.join(launcherRoot, "bin"), { recursive: true });
  fs.mkdirSync(path.join(launcherRoot, "lib"), { recursive: true });
  fs.copyFileSync(path.join(__dirname, "..", "bin", "ripr.cjs"), path.join(launcherRoot, "bin", "ripr.cjs"));
  fs.copyFileSync(path.join(__dirname, "..", "lib", "launcher.cjs"), path.join(launcherRoot, "lib", "launcher.cjs"));
  fs.chmodSync(path.join(launcherRoot, "bin", "ripr.cjs"), 0o755);
  const packageRoot = path.join(launcherRoot, "node_modules", ...selected.package.split("/"));
  const executableRelative = options.executable || selected.executable;
  writeJson(path.join(packageRoot, "package.json"), {
    name: options.name || selected.package,
    version: options.version || manifest.version,
    riprNative: {
      schemaVersion: 1,
      product: "ripr",
      target: options.target || selected.rustTarget,
      executable: executableRelative,
    },
  });
  if (options.createExecutable !== false) {
    const executablePath = path.join(packageRoot, ...executableRelative.split("/"));
    fs.mkdirSync(path.dirname(executablePath), { recursive: true });
    if (options.executableKind === "directory") {
      fs.mkdirSync(executablePath);
    } else if (options.executableKind === "symlink") {
      const target = path.join(root, "outside");
      fs.writeFileSync(target, "outside");
      fs.symlinkSync(target, executablePath);
    } else {
      fs.copyFileSync(process.execPath, executablePath);
      fs.chmodSync(executablePath, options.mode === undefined ? 0o755 : options.mode);
    }
  }
  return { root, launcherRoot, selected, packageRoot };
}

function cleanup(root) {
  fs.rmSync(root, { recursive: true, force: true });
}

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

async function waitForFile(filePath, timeoutMilliseconds = 5000) {
  const deadline = Date.now() + timeoutMilliseconds;
  while (Date.now() < deadline) {
    if (fs.existsSync(filePath)) {
      return fs.readFileSync(filePath, "utf8");
    }
    await delay(20);
  }
  throw new Error(`timed out waiting for ${filePath}`);
}

function waitForExit(child, timeoutMilliseconds = 10000) {
  return new Promise((resolve, reject) => {
    const stdout = [];
    const stderr = [];
    child.stdout?.on("data", (chunk) => stdout.push(chunk));
    child.stderr?.on("data", (chunk) => stderr.push(chunk));
    const timer = setTimeout(() => {
      reject(new Error(`timed out waiting for launcher process ${child.pid}`));
    }, timeoutMilliseconds);
    child.once("error", (error) => {
      clearTimeout(timer);
      reject(error);
    });
    child.once("exit", (code, signal) => {
      clearTimeout(timer);
      resolve({
        code,
        signal,
        stdout: Buffer.concat(stdout).toString("utf8"),
        stderr: Buffer.concat(stderr).toString("utf8"),
      });
    });
  });
}

function signalProbe(root, signalName) {
  const script = path.join(root, `${signalName}-signal-probe.cjs`);
  fs.writeFileSync(
    script,
    `"use strict";\nconst fs = require("node:fs");\nconst signal = process.env.RIPR_SIGNAL_NAME;\nconst ready = process.env.RIPR_SIGNAL_READY;\nconst receipt = process.env.RIPR_SIGNAL_RECEIPT;\nlet count = 0;\nprocess.on(signal, () => {\n  count += 1;\n  fs.writeFileSync(receipt, JSON.stringify({ signal, count, pid: process.pid }));\n  if (count === 1) {\n    setTimeout(() => process.exit(0), 150);\n  }\n});\nfs.writeFileSync(ready, String(process.pid));\nsetInterval(() => {}, 1000);\n`,
  );
  return script;
}

function spawnSignalLauncher(fixture, signalName) {
  const ready = path.join(fixture.root, `${signalName}-ready.txt`);
  const receipt = path.join(fixture.root, `${signalName}-receipt.json`);
  const bin = path.join(fixture.launcherRoot, "bin", "ripr.cjs");
  const child = spawn(process.execPath, [bin, signalProbe(fixture.root, signalName)], {
    cwd: fixture.root,
    detached: true,
    env: {
      ...process.env,
      RIPR_SIGNAL_NAME: signalName,
      RIPR_SIGNAL_READY: ready,
      RIPR_SIGNAL_RECEIPT: receipt,
    },
    stdio: ["ignore", "pipe", "pipe"],
  });
  return { child, ready, receipt };
}

function killProcessGroup(pid, signal = "SIGKILL") {
  try {
    process.kill(-pid, signal);
  } catch (error) {
    if (error.code !== "ESRCH") {
      throw error;
    }
  }
}

async function assertProcessGone(pid, timeoutMilliseconds = 3000) {
  const deadline = Date.now() + timeoutMilliseconds;
  while (Date.now() < deadline) {
    try {
      process.kill(pid, 0);
    } catch (error) {
      if (error.code === "ESRCH") {
        return;
      }
      throw error;
    }
    await delay(20);
  }
  throw new Error(`process ${pid} remained alive after launcher termination`);
}

function hostPlatformOrSkip(t) {
  try {
    return launcher.selectPlatform(launcher.validateLauncherManifest(manifest).platforms);
  } catch (error) {
    t.skip(`host platform is not a supported target: ${error.message}`);
    return null;
  }
}

test("validates the exact five-target contract", () => {
  const contract = launcher.validateLauncherManifest(manifest);
  assert.equal(contract.version, "0.11.0");
  assert.deepEqual(
    contract.platforms.map((entry) => launcher.platformKey(entry.nodePlatform, entry.nodeArch, entry.libc)),
    ["darwin/arm64/none", "darwin/x64/none", "linux/arm64/glibc", "linux/x64/glibc", "win32/x64/none"],
  );
  assert.deepEqual(
    Object.keys(manifest.optionalDependencies).sort(),
    contract.platforms.map((entry) => entry.package).sort(),
  );
});

test("rejects lifecycle scripts, version ranges, and dependency drift", () => {
  for (const scriptName of [
    "preinstall",
    "install",
    "postinstall",
    "preprepare",
    "prepare",
    "postprepare",
  ]) {
    const mutated = launcherManifest({
      scripts: { ...manifest.scripts, [scriptName]: "node should-not-run.cjs" },
    });
    assert.throws(() => launcher.validateLauncherManifest(mutated), {
      code: "install_script_forbidden",
    });
  }

  const ranged = launcherManifest({
    optionalDependencies: { ...manifest.optionalDependencies, [platformRow().package]: "^0.11.0" },
  });
  assert.throws(() => launcher.validateLauncherManifest(ranged), { code: "native_dependency_version_mismatch" });

  const extra = launcherManifest({
    optionalDependencies: { ...manifest.optionalDependencies, "@effortlessmetrics/ripr-extra": manifest.version },
  });
  assert.throws(() => launcher.validateLauncherManifest(extra), { code: "native_dependency_set_mismatch" });
});

test("detects glibc and rejects musl or unknown Linux libc", () => {
  assert.equal(launcher.detectLinuxLibc({ getReport: () => ({ header: { glibcVersionRuntime: "2.36" } }) }), "glibc");
  assert.equal(launcher.detectLinuxLibc({ getReport: () => ({ sharedObjects: ["/lib/ld-musl-x86_64.so.1"] }) }), "musl");
  assert.equal(launcher.detectLinuxLibc({ getReport: () => ({ header: {}, sharedObjects: [] }) }), "unknown");

  const contract = launcher.validateLauncherManifest(manifest);
  assert.equal(
    launcher.selectPlatform(contract.platforms, { platform: "linux", arch: "x64", libc: "glibc" }).rustTarget,
    "x86_64-unknown-linux-gnu",
  );
  assert.throws(() => launcher.selectPlatform(contract.platforms, { platform: "linux", arch: "x64", libc: "musl" }), {
    code: "platform_unsupported",
  });
  assert.throws(() => launcher.selectPlatform(contract.platforms, { platform: "freebsd", arch: "x64" }), {
    code: "platform_unsupported",
  });
});

test("resolves an exact native package and confines its executable", () => {
  const fixture = nativeFixture();
  try {
    const resolver = createRequire(path.join(fixture.launcherRoot, "package.json"));
    const resolved = launcher.resolveNativeExecutable({
      launcherRoot: fixture.launcherRoot,
      selectedPlatform: fixture.selected,
      resolver,
    });
    assert.equal(resolved.selected.package, fixture.selected.package);
    assert.equal(resolved.contract.version, manifest.version);
    assert.equal(fs.realpathSync(resolved.executablePath), resolved.executablePath);
    assert.ok(launcher.isPathInside(fs.realpathSync(fixture.packageRoot), resolved.executablePath));
  } finally {
    cleanup(fixture.root);
  }
});

test("rejects missing, wrong-version, wrong-target, traversal, symlink, directory, and non-executable payloads", () => {
  const missing = nativeFixture({ createExecutable: false });
  try {
    assert.throws(
      () => launcher.resolveNativeExecutable({ launcherRoot: missing.launcherRoot, selectedPlatform: missing.selected }),
      { code: "native_executable_missing" },
    );
  } finally {
    cleanup(missing.root);
  }

  for (const [name, options, code] of [
    ["version", { version: "0.11.1" }, "native_package_version_mismatch"],
    ["target", { target: "aarch64-unknown-linux-gnu" }, "native_target_mismatch"],
    ["traversal", { executable: "../ripr" }, "native_executable_mismatch"],
    ["symlink", { executableKind: "symlink" }, "native_executable_symlink"],
    ["directory", { executableKind: "directory" }, "native_executable_not_file"],
  ]) {
    const fixture = nativeFixture(options);
    try {
      assert.throws(
        () => launcher.resolveNativeExecutable({ launcherRoot: fixture.launcherRoot, selectedPlatform: fixture.selected }),
        { code },
        name,
      );
    } finally {
      cleanup(fixture.root);
    }
  }

  if (process.platform !== "win32") {
    const nonExecutable = nativeFixture({ mode: 0o644 });
    try {
      assert.throws(
        () => launcher.resolveNativeExecutable({ launcherRoot: nonExecutable.launcherRoot, selectedPlatform: nonExecutable.selected }),
        { code: "native_executable_not_executable" },
      );
    } finally {
      cleanup(nonExecutable.root);
    }
  }
});

test("spawns an absolute executable with literal argv, inherited stdio, cwd, env, and no shell", async () => {
  const child = new EventEmitter();
  child.killed = false;
  child.kill = () => true;
  let call;
  const spawnImpl = (executable, argv, options) => {
    call = { executable, argv, options };
    queueMicrotask(() => child.emit("exit", 23, null));
    return child;
  };
  const env = { RIPR_TEST_VALUE: "present" };
  const cwd = path.resolve(os.tmpdir());
  const argv = ["--flag", "$(touch should-not-run)", "semi;colon", "space value"];
  const result = await launcher.runNative(path.resolve(process.execPath), argv, {
    spawnImpl,
    cwd,
    env,
    forwardSignals: false,
  });
  assert.deepEqual(result, { code: 23, signal: null });
  assert.equal(call.executable, path.resolve(process.execPath));
  assert.deepEqual(call.argv, argv);
  assert.equal(call.options.shell, false);
  assert.equal(call.options.stdio, "inherit");
  assert.equal(call.options.cwd, cwd);
  assert.equal(call.options.env, env);
});

test("maps child signals to a nonzero conventional exit code", () => {
  assert.equal(launcher.signalExitCode("SIGTERM"), 128 + os.constants.signals.SIGTERM);
  assert.equal(launcher.signalExitCode("RIPR_UNKNOWN_SIGNAL"), 1);
});

test("launch sets a nonzero exit code before forwarding a child signal", async () => {
  const fixture = nativeFixture();
  const child = new EventEmitter();
  child.killed = false;
  child.kill = () => true;
  const spawnImpl = () => {
    queueMicrotask(() => child.emit("exit", null, "SIGTERM"));
    return child;
  };
  const resolver = createRequire(path.join(fixture.launcherRoot, "package.json"));
  const originalKill = process.kill;
  const originalExitCode = process.exitCode;
  const forwarded = [];
  process.kill = (pid, signal) => {
    forwarded.push({ pid, signal, exitCode: process.exitCode });
    return true;
  };
  process.exitCode = undefined;
  try {
    await launcher.launch({
      launcherRoot: fixture.launcherRoot,
      selectedPlatform: fixture.selected,
      resolver,
      spawnImpl,
      forwardSignals: false,
    });
    assert.equal(process.exitCode, 128 + os.constants.signals.SIGTERM);
    assert.deepEqual(forwarded, [
      { pid: process.pid, signal: "SIGTERM", exitCode: 128 + os.constants.signals.SIGTERM },
    ]);
  } finally {
    process.kill = originalKill;
    process.exitCode = originalExitCode;
    cleanup(fixture.root);
  }
});

test(
  "forwards direct SIGTERM to native child exactly once and re-emits signal",
  { skip: process.platform === "win32", timeout: 15000 },
  async () => {
    const fixture = nativeFixture();
    const { child, ready, receipt } = spawnSignalLauncher(fixture, "SIGTERM");
    let nativePid;
    try {
      nativePid = Number.parseInt(await waitForFile(ready), 10);
      assert.ok(Number.isInteger(nativePid));
      const exitPromise = waitForExit(child);
      process.kill(child.pid, "SIGTERM");
      const exit = await exitPromise;
      await waitForFile(receipt);
      await delay(250);
      assert.deepEqual(JSON.parse(fs.readFileSync(receipt, "utf8")), {
        signal: "SIGTERM",
        count: 1,
        pid: nativePid,
      });
      assert.equal(exit.signal, "SIGTERM");
      assert.equal(exit.stdout, "");
      assert.equal(exit.stderr, "");
      await assertProcessGone(nativePid);
    } finally {
      killProcessGroup(child.pid);
      cleanup(fixture.root);
    }
  },
);

test(
  "retains the first observed signal when a supervisor escalates SIGINT to SIGTERM",
  { skip: process.platform === "win32", timeout: 15000 },
  async () => {
    const fixture = nativeFixture();
    const { child, ready, receipt } = spawnSignalLauncher(fixture, "SIGTERM");
    let nativePid;
    try {
      nativePid = Number.parseInt(await waitForFile(ready), 10);
      assert.ok(Number.isInteger(nativePid));
      const exitPromise = waitForExit(child);
      process.kill(child.pid, "SIGINT");
      await delay(300);
      assert.ok(!fs.existsSync(receipt), "observed SIGINT must not be forwarded to the native child");
      process.kill(child.pid, "SIGTERM");
      const exit = await exitPromise;
      await waitForFile(receipt);
      await delay(250);
      assert.deepEqual(JSON.parse(fs.readFileSync(receipt, "utf8")), {
        signal: "SIGTERM",
        count: 1,
        pid: nativePid,
      });
      assert.equal(exit.signal, "SIGINT");
      assert.equal(exit.stdout, "");
      assert.equal(exit.stderr, "");
      await assertProcessGone(nativePid);
    } finally {
      killProcessGroup(child.pid);
      cleanup(fixture.root);
    }
  },
);

test(
  "observes terminal SIGINT and SIGHUP without forwarding duplicates to the native child",
  { skip: process.platform === "win32", timeout: 30000 },
  async () => {
    for (const signalName of ["SIGINT", "SIGHUP"]) {
      const fixture = nativeFixture();
      const { child, ready, receipt } = spawnSignalLauncher(fixture, signalName);
      let nativePid;
      try {
        nativePid = Number.parseInt(await waitForFile(ready), 10);
        assert.ok(Number.isInteger(nativePid));
        const exitPromise = waitForExit(child);
        process.kill(-child.pid, signalName);
        const exit = await exitPromise;
        await waitForFile(receipt);
        await delay(250);
        assert.deepEqual(JSON.parse(fs.readFileSync(receipt, "utf8")), {
          signal: signalName,
          count: 1,
          pid: nativePid,
        });
        assert.equal(exit.signal, signalName);
        assert.equal(exit.stdout, "");
        assert.equal(exit.stderr, "");
        await assertProcessGone(nativePid);
      } finally {
        killProcessGroup(child.pid);
        cleanup(fixture.root);
      }
    }
  },
);

test("runs a real synthetic executable and preserves argv, cwd, env, stdout, stderr, and exit status", (t) => {
  const selected = hostPlatformOrSkip(t);
  if (!selected) {
    return;
  }
  const fixture = nativeFixture({ selected });
  const script = path.join(fixture.root, "probe.cjs");
  fs.writeFileSync(
    script,
    `const fs = require("node:fs");\nfs.writeFileSync(process.env.RIPR_PROBE_OUT, JSON.stringify({argv: process.argv.slice(2), cwd: process.cwd(), env: process.env.RIPR_PROBE_VALUE}));\nprocess.stdout.write("probe-stdout\\n");\nprocess.stderr.write("probe-stderr\\n");\nprocess.exit(23);\n`,
  );
  const out = path.join(fixture.root, "probe.json");
  const bin = path.join(fixture.launcherRoot, "bin", "ripr.cjs");
  const result = spawnSync(process.execPath, [bin, script, "literal;value", "space value"], {
    cwd: fixture.root,
    env: {
      ...process.env,
      RIPR_PROBE_OUT: out,
      RIPR_PROBE_VALUE: "present",
    },
    encoding: "utf8",
  });
  try {
    assert.equal(result.status, 23, result.stderr);
    assert.equal(result.stdout, "probe-stdout\n");
    assert.equal(result.stderr, "probe-stderr\n");
    assert.deepEqual(JSON.parse(fs.readFileSync(out, "utf8")), {
      argv: ["literal;value", "space value"],
      cwd: fixture.root,
      env: "present",
    });
  } finally {
    cleanup(fixture.root);
  }
});

test("source bin missing-package failure never falls back to PATH or writes stdout", (t) => {
  if (!hostPlatformOrSkip(t)) {
    return;
  }
  const fakePath = fixtureRoot("path-fallback");
  const foreignCwd = fixtureRoot("foreign-cwd");
  const fakeExecutable = path.join(fakePath, process.platform === "win32" ? "ripr.cmd" : "ripr");
  fs.writeFileSync(fakeExecutable, process.platform === "win32" ? "@echo PATH-FALLBACK\r\n" : "#!/bin/sh\necho PATH-FALLBACK\n");
  fs.chmodSync(fakeExecutable, 0o755);
  const bin = path.join(__dirname, "..", "bin", "ripr.cjs");
  const result = spawnSync(process.execPath, [bin, "--version"], {
    cwd: foreignCwd,
    env: { ...process.env, PATH: `${fakePath}${path.delimiter}${process.env.PATH || ""}` },
    encoding: "utf8",
  });
  try {
    assert.equal(result.status, 1);
    assert.equal(result.stdout, "");
    assert.doesNotMatch(result.stdout + result.stderr, /PATH-FALLBACK/);
    assert.match(result.stderr, /required native package .*@0\.11\.0 is missing/);
  } finally {
    cleanup(fakePath);
    cleanup(foreignCwd);
  }
});

test("npm package contents are explicit and exclude tests and build residue", () => {
  const root = path.join(__dirname, "..");
  const npmCli = process.env.npm_execpath;
  const command = npmCli ? process.execPath : "npm";
  const args = npmCli
    ? [npmCli, "pack", "--dry-run", "--json", "--ignore-scripts"]
    : ["pack", "--dry-run", "--json", "--ignore-scripts"];
  const result = spawnSync(command, args, {
    cwd: root,
    encoding: "utf8",
  });
  assert.equal(result.status, 0, result.stderr);
  const report = JSON.parse(result.stdout);
  const files = report[0].files.map((entry) => entry.path).sort();
  assert.deepEqual(files, [
    "LICENSE-APACHE",
    "LICENSE-MIT",
    "README.md",
    "bin/ripr.cjs",
    "lib/launcher.cjs",
    "package.json",
  ]);
  assert.equal(report[0].name, "@effortlessmetrics/ripr");
  assert.equal(report[0].version, manifest.version);
});
