#!/usr/bin/env node
"use strict";

const path = require("node:path");
const { launch } = require("../lib/launcher.cjs");

launch({
  launcherRoot: path.resolve(__dirname, ".."),
  argv: process.argv.slice(2),
}).catch((error) => {
  const message = error && typeof error.message === "string" ? error.message : String(error);
  process.stderr.write(`ripr npm launcher: ${message}\n`);
  process.exitCode = 1;
});
