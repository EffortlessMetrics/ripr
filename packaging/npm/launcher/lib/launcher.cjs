"use strict";

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { createRequire } = require("node:module");
const { spawn } = require("node:child_process");

const LAUNCHER_NAME = "@effortlessmetrics/ripr";
const NATIVE_SCHEMA_VERSION = 1;
const PRODUCT_NAME = "ripr";
const DISALLOWED_LIFECYCLE_SCRIPTS = ["preinstall", "install", "postinstall", "prepare"];
const SIGNALS_TO_FORWARD = ["SIGINT", "SIGTERM", "SIGHUP"];

class LauncherError extends Error {
  constructor(code, message, cause) {
    super(message, cause ? { cause } : undefined);
    this.name = "LauncherError";
    this.code = code;
  }
}

function readJsonFile(filePath, label) {
  let text;
  try {
    text = fs.readFileSync(filePath, "utf8");
  } catch (error) {
    throw new LauncherError("json_read_failed", `cannot read ${label} at ${filePath}: ${error.message}`, error);
  }
  try {
    return JSON.parse(text);
  } catch (error) {
    throw new LauncherError("json_invalid", `${label} at ${filePath} is not valid JSON: ${error.message}`, error);
  }
}

function assertPlainObject(value, label) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new LauncherError("manifest_invalid", `${label} must be an object`);
  }
  return value;
}

function validateRelativeExecutable(value, label = "native executable") {
  if (typeof value !== "string" || value.length === 0 || value.includes("\0") || value.includes("\\")) {
    throw new LauncherError("native_executable_invalid", `${label} must be a non-empty portable relative path`);
  }
  if (path.posix.isAbsolute(value)) {
    throw new LauncherError("native_executable_invalid", `${label} must be relative, got ${JSON.stringify(value)}`);
  }
  const components = value.split("/");
  if (components.some((component) => component === "" || component === "." || component === "..")) {
    throw new LauncherError("native_executable_invalid", `${label} escapes or is not normalized: ${JSON.stringify(value)}`);
  }
  return value;
}

function validateLauncherManifest(manifest) {
  const root = assertPlainObject(manifest, "launcher package manifest");
  if (root.name !== LAUNCHER_NAME) {
    throw new LauncherError("launcher_name_mismatch", `launcher package name must be ${LAUNCHER_NAME}, got ${JSON.stringify(root.name)}`);
  }
  if (typeof root.version !== "string" || root.version.length === 0 || root.version.trim() !== root.version) {
    throw new LauncherError("launcher_version_invalid", "launcher package version must be a non-empty normalized string");
  }
  const scripts = root.scripts === undefined ? {} : assertPlainObject(root.scripts, "scripts");
  for (const scriptName of DISALLOWED_LIFECYCLE_SCRIPTS) {
    if (Object.prototype.hasOwnProperty.call(scripts, scriptName)) {
      throw new LauncherError("install_script_forbidden", `launcher package must not define npm lifecycle script ${scriptName}`);
    }
  }
  if (!root.bin || root.bin.ripr !== "bin/ripr.cjs") {
    throw new LauncherError("launcher_bin_invalid", "launcher package must expose ripr through bin/ripr.cjs");
  }
  const optionalDependencies = assertPlainObject(root.optionalDependencies, "optionalDependencies");
  const ripr = assertPlainObject(root.ripr, "ripr package metadata");
  if (ripr.schemaVersion !== 1 || ripr.product !== PRODUCT_NAME || ripr.executable !== PRODUCT_NAME) {
    throw new LauncherError("launcher_metadata_invalid", "launcher ripr metadata must identify schema 1 and product/executable ripr");
  }
  if (!Array.isArray(ripr.platforms) || ripr.platforms.length === 0) {
    throw new LauncherError("platform_map_invalid", "launcher ripr.platforms must be a non-empty array");
  }

  const keys = new Set();
  const packages = new Set();
  const platforms = ripr.platforms.map((entry, index) => {
    const row = assertPlainObject(entry, `ripr.platforms[${index}]`);
    for (const field of ["nodePlatform", "nodeArch", "rustTarget", "package", "executable"]) {
      if (typeof row[field] !== "string" || row[field].length === 0) {
        throw new LauncherError("platform_map_invalid", `ripr.platforms[${index}].${field} must be a non-empty string`);
      }
    }
    if (row.libc !== null && row.libc !== "glibc") {
      throw new LauncherError("platform_map_invalid", `ripr.platforms[${index}].libc must be null or glibc`);
    }
    if (row.nodePlatform === "linux" && row.libc !== "glibc") {
      throw new LauncherError("platform_map_invalid", `Linux target ${row.rustTarget} must be explicitly glibc`);
    }
    if (row.nodePlatform !== "linux" && row.libc !== null) {
      throw new LauncherError("platform_map_invalid", `non-Linux target ${row.rustTarget} must not declare libc`);
    }
    validateRelativeExecutable(row.executable, `ripr.platforms[${index}].executable`);
    const key = platformKey(row.nodePlatform, row.nodeArch, row.libc);
    if (keys.has(key)) {
      throw new LauncherError("platform_map_duplicate", `duplicate launcher platform row ${key}`);
    }
    if (packages.has(row.package)) {
      throw new LauncherError("platform_package_duplicate", `duplicate native package ${row.package}`);
    }
    keys.add(key);
    packages.add(row.package);
    if (optionalDependencies[row.package] !== root.version) {
      throw new LauncherError(
        "native_dependency_version_mismatch",
        `optional dependency ${row.package} must equal launcher version ${root.version}, got ${JSON.stringify(optionalDependencies[row.package])}`,
      );
    }
    return Object.freeze({ ...row });
  });

  const dependencyNames = Object.keys(optionalDependencies).sort();
  const platformPackages = [...packages].sort();
  if (JSON.stringify(dependencyNames) !== JSON.stringify(platformPackages)) {
    throw new LauncherError(
      "native_dependency_set_mismatch",
      `optional dependency set must equal the registered native package set; expected ${platformPackages.join(", ")}, got ${dependencyNames.join(", ")}`,
    );
  }

  return Object.freeze({ version: root.version, platforms: Object.freeze(platforms) });
}

function platformKey(nodePlatform, nodeArch, libc) {
  return `${nodePlatform}/${nodeArch}/${libc || "none"}`;
}

function detectLinuxLibc(reportProvider = process.report) {
  let report;
  try {
    report = reportProvider && typeof reportProvider.getReport === "function" ? reportProvider.getReport() : null;
  } catch (error) {
    throw new LauncherError("linux_libc_detection_failed", `cannot inspect Linux libc: ${error.message}`, error);
  }
  const header = report && typeof report === "object" ? report.header : null;
  if (header && typeof header.glibcVersionRuntime === "string" && header.glibcVersionRuntime.length > 0) {
    return "glibc";
  }
  const sharedObjects = report && Array.isArray(report.sharedObjects) ? report.sharedObjects : [];
  if (sharedObjects.some((entry) => typeof entry === "string" && /(?:^|[/\\])(?:ld-musl|libc\.musl)/i.test(entry))) {
    return "musl";
  }
  return "unknown";
}

function selectPlatform(platforms, options = {}) {
  const nodePlatform = options.platform || process.platform;
  const nodeArch = options.arch || process.arch;
  const libc = nodePlatform === "linux" ? options.libc || detectLinuxLibc(options.reportProvider) : null;
  if (nodePlatform === "linux" && libc !== "glibc") {
    throw new LauncherError(
      "platform_unsupported",
      `unsupported Linux libc ${libc}; this release provides glibc payloads only`,
    );
  }
  const key = platformKey(nodePlatform, nodeArch, libc);
  const selected = platforms.find((entry) => platformKey(entry.nodePlatform, entry.nodeArch, entry.libc) === key);
  if (!selected) {
    throw new LauncherError("platform_unsupported", `unsupported platform ${key}`);
  }
  return selected;
}

function isPathInside(root, candidate) {
  const relative = path.relative(root, candidate);
  return relative === "" || (!relative.startsWith(`..${path.sep}`) && relative !== ".." && !path.isAbsolute(relative));
}

function resolveNativeExecutable(options) {
  const launcherRoot = fs.realpathSync(options.launcherRoot);
  const launcherManifestPath = path.join(launcherRoot, "package.json");
  const launcherManifest = options.launcherManifest || readJsonFile(launcherManifestPath, "launcher package manifest");
  const contract = validateLauncherManifest(launcherManifest);
  const selected = options.selectedPlatform || selectPlatform(contract.platforms, options.platformOptions);
  const resolver = options.resolver || createRequire(launcherManifestPath);

  let nativeManifestPath;
  try {
    nativeManifestPath = resolver.resolve(`${selected.package}/package.json`);
  } catch (error) {
    throw new LauncherError(
      "native_package_missing",
      `required native package ${selected.package}@${contract.version} is missing; reinstall ${LAUNCHER_NAME}@${contract.version} with optional dependencies enabled`,
      error,
    );
  }
  const nativeRoot = fs.realpathSync(path.dirname(nativeManifestPath));
  const nativeManifest = readJsonFile(nativeManifestPath, `native package ${selected.package}`);
  if (nativeManifest.name !== selected.package) {
    throw new LauncherError(
      "native_package_name_mismatch",
      `resolved native package name must be ${selected.package}, got ${JSON.stringify(nativeManifest.name)}`,
    );
  }
  if (nativeManifest.version !== contract.version) {
    throw new LauncherError(
      "native_package_version_mismatch",
      `native package ${selected.package} must be version ${contract.version}, got ${JSON.stringify(nativeManifest.version)}`,
    );
  }
  const nativeMetadata = assertPlainObject(nativeManifest.riprNative, `native package ${selected.package} riprNative metadata`);
  if (nativeMetadata.schemaVersion !== NATIVE_SCHEMA_VERSION || nativeMetadata.product !== PRODUCT_NAME) {
    throw new LauncherError("native_metadata_invalid", `native package ${selected.package} metadata must identify schema 1 and product ripr`);
  }
  if (nativeMetadata.target !== selected.rustTarget) {
    throw new LauncherError(
      "native_target_mismatch",
      `native package ${selected.package} must target ${selected.rustTarget}, got ${JSON.stringify(nativeMetadata.target)}`,
    );
  }
  if (nativeMetadata.executable !== selected.executable) {
    throw new LauncherError(
      "native_executable_mismatch",
      `native package ${selected.package} executable must be ${selected.executable}, got ${JSON.stringify(nativeMetadata.executable)}`,
    );
  }
  validateRelativeExecutable(nativeMetadata.executable);
  const executablePath = path.resolve(nativeRoot, ...nativeMetadata.executable.split("/"));
  if (!isPathInside(nativeRoot, executablePath)) {
    throw new LauncherError("native_executable_escape", `native executable escapes package root: ${nativeMetadata.executable}`);
  }

  let stat;
  try {
    stat = fs.lstatSync(executablePath);
  } catch (error) {
    throw new LauncherError(
      "native_executable_missing",
      `native package ${selected.package}@${contract.version} does not contain ${nativeMetadata.executable}`,
      error,
    );
  }
  if (stat.isSymbolicLink()) {
    throw new LauncherError("native_executable_symlink", `native executable must not be a symbolic link: ${nativeMetadata.executable}`);
  }
  if (!stat.isFile()) {
    throw new LauncherError("native_executable_not_file", `native executable must be a regular file: ${nativeMetadata.executable}`);
  }
  const realExecutable = fs.realpathSync(executablePath);
  if (!isPathInside(nativeRoot, realExecutable)) {
    throw new LauncherError("native_executable_escape", `native executable resolves outside package root: ${nativeMetadata.executable}`);
  }
  if (selected.nodePlatform !== "win32" && (stat.mode & 0o111) === 0) {
    throw new LauncherError("native_executable_not_executable", `native executable is not executable: ${nativeMetadata.executable}`);
  }

  return Object.freeze({ executablePath: realExecutable, contract, selected, nativeManifestPath, nativeRoot });
}

function runNative(executablePath, argv, options = {}) {
  const spawnImpl = options.spawnImpl || spawn;
  const child = spawnImpl(executablePath, argv, {
    cwd: options.cwd || process.cwd(),
    env: options.env || process.env,
    stdio: "inherit",
    shell: false,
    windowsHide: false,
  });

  const forwarded = new Map();
  if (options.forwardSignals !== false && process.platform !== "win32") {
    for (const signal of SIGNALS_TO_FORWARD) {
      const handler = () => {
        if (!child.killed) {
          child.kill(signal);
        }
      };
      forwarded.set(signal, handler);
      process.on(signal, handler);
    }
  }
  const cleanup = () => {
    for (const [signal, handler] of forwarded) {
      process.removeListener(signal, handler);
    }
  };

  return new Promise((resolve, reject) => {
    child.once("error", (error) => {
      cleanup();
      reject(new LauncherError("native_launch_failed", `failed to launch native ripr at ${executablePath}: ${error.message}`, error));
    });
    child.once("exit", (code, signal) => {
      cleanup();
      if (signal) {
        resolve({ code: null, signal });
      } else {
        resolve({ code: typeof code === "number" ? code : 1, signal: null });
      }
    });
  });
}

function signalExitCode(signal) {
  const signalNumber = os.constants.signals[signal];
  return typeof signalNumber === "number" ? 128 + signalNumber : 1;
}

async function launch(options = {}) {
  const launcherRoot = options.launcherRoot || path.resolve(__dirname, "..");
  const resolved = resolveNativeExecutable({
    launcherRoot,
    launcherManifest: options.launcherManifest,
    selectedPlatform: options.selectedPlatform,
    platformOptions: options.platformOptions,
    resolver: options.resolver,
  });
  const result = await runNative(resolved.executablePath, options.argv || [], {
    spawnImpl: options.spawnImpl,
    cwd: options.cwd,
    env: options.env,
    forwardSignals: options.forwardSignals,
  });
  if (result.signal) {
    process.exitCode = signalExitCode(result.signal);
    process.kill(process.pid, result.signal);
    return;
  }
  process.exitCode = result.code;
}

module.exports = {
  LAUNCHER_NAME,
  LauncherError,
  detectLinuxLibc,
  isPathInside,
  launch,
  platformKey,
  readJsonFile,
  resolveNativeExecutable,
  runNative,
  selectPlatform,
  signalExitCode,
  validateLauncherManifest,
  validateRelativeExecutable,
};
