# RIPR-SPEC-0212: npm Native Launcher Boundary

Status: accepted

Owner: product / swarm

Created: 2026-09-29

Renumbered: from RIPR-SPEC-0181 after main landed the inline test-module
region cage spec under RIPR-SPEC-0181 first.

Linked issues: #4491, #4710

Linked PRs: #4716

Support-tier impact: none; see [support tiers](../status/SUPPORT_TIERS.md).
This spec defines the source-package and process boundary for the npm launcher.
It does not claim that any native npm payload package has been published or
that any platform has passed installed-product qualification.

Policy impact: package boundary, process lifecycle, distribution policy, and a
path-filtered no-publish CI lane. Registry, credential, environment, release,
and tag state remain source-repository authorities.

## Problem

The npm convenience layer must not become a second implementation of `ripr`, a
runtime downloader, or an ambiguous process wrapper. A successful `npm install`
is insufficient when the selected native package can drift, lifecycle scripts
can run hidden setup, a different `ripr` can be found on `PATH`, or terminal
signals can be delivered twice to the native process.

Static checks that merely find expected words in a test file are also
insufficient. The package proof must fail when a required behavioral control no
longer executes and passes.

## Behavior

The package `@effortlessmetrics/ripr`:

- declares exactly the five target packages from `policy/distribution.toml` as
  exact-version optional dependencies;
- rejects `preinstall`, `install`, `postinstall`, `preprepare`, `prepare`, and
  `postprepare`; installation never depends on lifecycle scripts;
- resolves the selected package relative to the launcher, validates its name,
  version, target, schema, product, and executable metadata, and confines the
  executable to that package's real root;
- never downloads, compiles, invokes a shell, or falls back to an ambient
  `ripr` executable;
- starts one absolute executable with literal arguments, inherited working
  directory, environment, and standard streams, and `shell: false`;
- on POSIX, forwards a direct supervisor `SIGTERM` once to the native child;
- on POSIX, observes terminal-generated `SIGINT` and `SIGHUP` without forwarding
  a duplicate to a child that shares the foreground process group;
- retains the first observed termination signal until the native child exits,
  then exits with signal semantics rather than converting interruption into
  success;
- runs tests through a guarded runner that fails unless the required named
  package, negative, and signal controls actually execute and pass.

## Required Evidence

- manifest controls reject each forbidden lifecycle hook and any dependency
  range or target-set drift;
- native resolution controls reject missing, mismatched, escaping, symlinked,
  directory, and non-executable payloads;
- a real POSIX subprocess control sends `SIGTERM` only to the launcher and
  observes exactly one child delivery with no surviving child;
- a real POSIX escalation control sends `SIGINT` then `SIGTERM` only to the
  launcher and observes no child `SIGINT`, exactly one child `SIGTERM`, and a
  launcher exit that retains the first observed `SIGINT`;
- a real POSIX process-group control sends terminal-style `SIGINT` and `SIGHUP`
  and observes exactly one child delivery each with no launcher duplication or
  surviving child;
- a planted `ripr` on `PATH` is never selected when the required package is
  absent;
- the packed launcher inventory contains only the declared runtime files and
  notices;
- the guarded test runner observes the required test names as non-skipped TAP
  passes and fails closed when any required control is absent or failing;
- `cargo xtask check-release-targets` validates the manifest, source guards,
  guarded-runner contract, notices, and target projection.

## Non-Goals

- publishing the launcher or any native package;
- defining platform package production or final payload identity;
- claiming Windows descendant-process cleanup or POSIX behavior on Windows;
- claiming pnpm, Yarn, Bun, Plug'n'Play, or local-registry compatibility;
- changing `ripr` analysis, configuration, findings, output schemas, or support
  tiers;
- handling uncatchable launcher termination such as `SIGKILL`.

## Acceptance Examples

1. A manifest containing `preprepare` or `postprepare` fails both the JavaScript
   and Rust-owned package validators.
2. A terminal `SIGINT` delivered to the launcher's POSIX process group reaches
   the native child once, not twice, and neither process remains alive.
3. A direct `SIGTERM` delivered only to the launcher is forwarded once to the
   native child and the launcher re-emits termination after cleanup.
4. Deleting or skipping a required named control makes `npm test` fail even if
   other Node tests remain green.
5. Removing the matching native package produces an actionable error on stderr,
   writes nothing to protocol stdout, and never executes a planted `ripr` from
   `PATH`.

## Test Mapping

- `packaging/npm/launcher/test/run-tests.test.cjs`
- `packaging/npm/launcher/test/launcher.test.cjs::rejects lifecycle scripts, version ranges, and dependency drift`
- `packaging/npm/launcher/test/launcher.test.cjs::rejects missing, wrong-version, wrong-target, traversal, symlink, directory, and non-executable payloads`
- `packaging/npm/launcher/test/launcher.test.cjs::forwards direct SIGTERM to native child exactly once and re-emits signal`
- `packaging/npm/launcher/test/launcher.test.cjs::retains the first observed signal when a supervisor escalates SIGINT to SIGTERM`
- `packaging/npm/launcher/test/launcher.test.cjs::observes terminal SIGINT and SIGHUP without forwarding duplicates to the native child`
- `packaging/npm/launcher/test/launcher.test.cjs::source bin missing-package failure never falls back to PATH or writes stdout`
- `packaging/npm/launcher/test/launcher.test.cjs::npm package contents are explicit and exclude tests and build residue`
- `xtask/src/policy/distribution/tests.rs`

## Implementation Mapping

- `packaging/npm/launcher/package.json` owns the npm package shape, exact
  dependency family, test entrypoint, and lifecycle-script absence.
- `packaging/npm/launcher/bin/ripr.cjs` owns the public executable entrypoint.
- `packaging/npm/launcher/lib/launcher.cjs` owns platform selection, native
  package validation, executable confinement, process launch, signal handling,
  and exit projection.
- `packaging/npm/launcher/test/run-tests.test.cjs` owns executed-control
  admission for the Node proof.
- `xtask/src/policy/distribution/npm_launcher.rs` owns repository-level package,
  source, notice, and guarded-runner conformance.
- `.github/workflows/npm-launcher.yml` runs the no-publish package proof.
- `policy/distribution.toml` remains the target and identity authority.

## Metrics

- `npm_launcher_required_control_pass_rate`
- `npm_launcher_package_inventory_drift_count`
- `npm_launcher_signal_delivery_regression_count`
