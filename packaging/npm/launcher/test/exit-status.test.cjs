"use strict";

const assert = require("node:assert/strict");
const { EventEmitter } = require("node:events");
const path = require("node:path");
const test = require("node:test");

const launcher = require("../lib/launcher.cjs");

test("preserves a zero child exit code", async () => {
  const child = new EventEmitter();
  child.killed = false;
  child.kill = () => true;
  const spawnImpl = () => {
    queueMicrotask(() => child.emit("exit", 0, null));
    return child;
  };

  const result = await launcher.runNative(path.resolve(process.execPath), [], {
    spawnImpl,
    forwardSignals: false,
  });

  assert.deepEqual(result, { code: 0, signal: null });
});
