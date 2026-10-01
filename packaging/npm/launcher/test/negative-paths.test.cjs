"use strict";

const assert = require("node:assert/strict");
const { EventEmitter } = require("node:events");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { createRequire } = require("node:module");
const { spawnSync } = require("node:child_process");
const test = require("node:test");

const launcher = require("../lib/launcher.cjs");
const manifest = require("../package.json");

function tempRoot(name) {
  return fs.mkdtempSync(path.join(os.tmpdir(), `ripr-npm-${name}-`));
}

function writeJson(filePath, value) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`);
}

function selectedLinuxRow() {
  return manifest.ripr.platforms.find(
    (entry) => entry.nodePlatform === "linux" && entry.nodeArch === "x64",
  );
}

function metadataFixture(overrides = {}) {
  const root = tempRoot("metadata");
  const launcherRoot = path.join(root, "launcher");
  const selected = selectedLinuxRow();
  writeJson(path.join(launcherRoot, "package.json"), manifest);

  const packageRoot = path.join(launcherRoot, "node_modules", ...selected.package.split("/"));
  writeJson(path.join(packageRoot, "package.json"), {
    name: overrides.name || selected.package,
    version: overrides.version || manifest.version,
    riprNative: {
      schemaVersion: overrides.schemaVersion === undefined ? 1 : overrides.schemaVersion,
      product: overrides.product || "ripr",
      target: overrides.target || selected.rustTarget,
      executable: selected.executable,
    },
  });
  const executable = path.join(packageRoot, ...selected.executable.split("/"));
  fs.mkdirSync(path.dirname(executable), { recursive: true });
  fs.copyFileSync(process.execPath, executable);
  fs.chmodSync(executable, 0o755);

  return { root, launcherRoot, selected };
}

test("rejects a traversal path in the launcher platform contract", () => {
  const mutated = structuredClone(manifest);
  mutated.ripr.platforms[0].executable = "../ripr";
  assert.throws(() => launcher.validateLauncherManifest(mutated), {
    code: "native_executable_invalid",
  });
});

test("rejects wrong native package name and metadata schema or product", () => {
  for (const [overrides, code] of [
    [{ name: "@effortlessmetrics/not-ripr" }, "native_package_name_mismatch"],
    [{ schemaVersion: 2 }, "native_metadata_invalid"],
    [{ product: "other" }, "native_metadata_invalid"],
  ]) {
    const fixture = metadataFixture(overrides);
    try {
      const resolver = createRequire(path.join(fixture.launcherRoot, "package.json"));
      assert.throws(
        () =>
          launcher.resolveNativeExecutable({
            launcherRoot: fixture.launcherRoot,
            selectedPlatform: fixture.selected,
            resolver,
          }),
        { code },
      );
    } finally {
      fs.rmSync(fixture.root, { recursive: true, force: true });
    }
  }
});

test("reports a native spawn error as a typed launch failure", async () => {
  const child = new EventEmitter();
  child.killed = false;
  child.kill = () => true;
  const spawnImpl = () => {
    queueMicrotask(() => child.emit("error", new Error("synthetic spawn failure")));
    return child;
  };

  await assert.rejects(
    launcher.runNative(path.resolve(process.execPath), [], {
      spawnImpl,
      forwardSignals: false,
    }),
    (error) =>
      error instanceof launcher.LauncherError &&
      error.code === "native_launch_failed" &&
      /synthetic spawn failure/.test(error.message),
  );
});

test("passes piped stdin through the launcher boundary without wrapper output", () => {
  const root = tempRoot("stdin");
  const probe = path.join(root, "probe.cjs");
  const parent = path.join(root, "parent.cjs");
  fs.writeFileSync(
    probe,
    'const fs = require("node:fs"); process.stdout.write(fs.readFileSync(0, "utf8"));\n',
  );
  fs.writeFileSync(
    parent,
    `const launcher = require(${JSON.stringify(path.join(__dirname, "..", "lib", "launcher.cjs"))});\nlauncher.runNative(process.execPath, [${JSON.stringify(probe)}], { forwardSignals: false }).then((result) => { process.exitCode = result.code; }).catch((error) => { process.stderr.write(error.message + "\\n"); process.exitCode = 1; });\n`,
  );

  try {
    const result = spawnSync(process.execPath, [parent], {
      input: "protocol-input\n",
      encoding: "utf8",
    });
    assert.equal(result.status, 0, result.stderr);
    assert.equal(result.stdout, "protocol-input\n");
    assert.equal(result.stderr, "");
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
