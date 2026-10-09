"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const test = require("node:test");

test("the launcher bin remains executable in the checkout and packed package", (t) => {
  if (process.platform === "win32") {
    t.skip("Windows does not preserve POSIX executable mode bits");
    return;
  }

  const root = path.join(__dirname, "..");
  const bin = path.join(root, "bin", "ripr.cjs");
  assert.notEqual(fs.statSync(bin).mode & 0o111, 0, "bin/ripr.cjs must be executable in the checkout");

  const npmCli = process.env.npm_execpath;
  const command = npmCli ? process.execPath : "npm";
  const args = npmCli
    ? [npmCli, "pack", "--dry-run", "--json", "--ignore-scripts"]
    : ["pack", "--dry-run", "--json", "--ignore-scripts"];
  const result = spawnSync(command, args, { cwd: root, encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);

  const report = JSON.parse(result.stdout);
  const packedBin = report[0].files.find((entry) => entry.path === "bin/ripr.cjs");
  assert.ok(packedBin, "packed package must contain bin/ripr.cjs");
  assert.notEqual(packedBin.mode & 0o111, 0, "packed bin/ripr.cjs must be executable");
});
