# RIPR-SPEC-0101: TypeScript Package-Manager-Unresolved Disclosure

Status: accepted

Owner: product / swarm

Created: 2026-06-14

Linked issues:

- #1239

Linked PRs:

- None yet

Support-tier impact:

- Fail-closed honesty fix for TypeScript preview findings: the misleading
  `typescript_runner_hint_unresolved` limitation is no longer emitted when
  a framework is known and a verify command IS derivable. A new informational
  limitation `typescript_package_manager_unresolved` is introduced for this
  case. Language status stays Preview; no tier change.
  Claim boundaries and tier labels remain governed by the canonical ledger in
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- No new crates, binaries, or LSP servers.
- One new `TsPackageLimitation` variant (`PackageManagerUnresolved`) in
  `crates/ripr/src/analysis/language/typescript/package.rs`.
- One new wire name `typescript_package_manager_unresolved` registered in
  `docs/OUTPUT_SCHEMA.md`.
- No schema version bump. The new limitation wire name is additive evidence
  on existing findings; it does not change the JSON schema version.
- Register this spec in `policy/doc-artifacts.toml` and `docs/specs/README.md`.

## Problem

`crates/ripr/src/analysis/language/typescript/package.rs::resolve_package_discovery`
emits the limitation `typescript_runner_hint_unresolved` whenever
`runner_hint.is_none()`, regardless of whether a framework is known.

But `verify_command_for_discovery` (same file) derives a usable verify command
from `framework_hint` alone — for example, `Some(Vitest)` produces
`"vitest run <file>"` without consulting `runner_hint` at all.

**Concrete repro (issue #1239)**: a project with `vitest` in `devDependencies`
and no lockfile:

- `framework_hint = Some(Vitest)`
- `runner_hint = None`
- ripr emits BOTH:
  - `typescript_verify_command: vitest run <file>` (correct, usable)
  - `typescript_package_limitation: typescript_runner_hint_unresolved` (misleading)

A developer reading `runner_hint_unresolved` infers "no command is available,"
which is false. The command is available via the framework binary; only the
package manager (npm/pnpm/yarn/bun) is unknown.

## Solution

Split the `runner_hint.is_none()` limitation by whether a framework is known:

| Framework known? | Runner known? | Limitation emitted | Blocking? |
|---|---|---|---|
| yes | yes | none | — |
| yes | no | `typescript_package_manager_unresolved` | no (informational) |
| no | yes | none (runner suffices) | — |
| no | no | `typescript_runner_hint_unresolved` | yes (fail-closed) |

The strong fail-closed limitation `typescript_runner_hint_unresolved` is
**preserved** for the genuinely-unactionable case (no framework, no runner — no
command can be derived). This spec **rewrites** the misleading case; it does not
delete any disclosure.

## Behavior

### Limitation selection rule

In `resolve_package_discovery`, when `runner_hint.is_none()`:

- if `framework_hint.is_some()` → push `PackageManagerUnresolved`
  (wire: `typescript_package_manager_unresolved`).
  The framework binary IS the test runner; a direct command is derivable;
  only the package manager/lockfile is unknown. Informational, NOT implying
  the command is missing.
- if `framework_hint.is_none()` → push `RunnerHintMissing`
  (wire: `typescript_runner_hint_unresolved`).
  Genuinely no command can be derived. Strong fail-closed case, unchanged.

### New limitation wire name

Wire name: `typescript_package_manager_unresolved`

Semantics: "package manager (npm/pnpm/yarn/bun) not determined from lockfile
evidence; test command available via the framework binary."

This limitation is informational, not blocking. The verify command is still
emitted when the framework resolves. The real uncertainty disclosed is: which
package manager wraps the run, not whether a command exists.

### Honesty invariants

- NEVER claim the package manager is known when it is not: the new limitation
  still discloses the real uncertainty, just accurately.
- NEVER over-claim a command exists: `typescript_verify_command` continues to
  be emitted ONLY when `verify_command_for_discovery` returns `Some`
  (no change to command derivation logic).
- `typescript_runner_hint_unresolved` is preserved for the case where BOTH
  framework and runner are absent (no command derivable).

### Static-language compliance

Wire names and prose use only allowed static-language terms: `unresolved`,
`available`, `not determined`. Forbidden terms (`killed`, `survived`, `untested`,
`proven`, `adequate`, `covered`, `passing`) are absent.

## Required Evidence

Unit tests in
`crates/ripr/src/analysis/language/typescript/package.rs::tests`:

1. `gap3_vitest_no_lockfile_emits_package_manager_unresolved_not_runner_unresolved`
   — Control 1 (the repro): vitest devDep, no lockfile → framework=Vitest,
   runner=None → `typescript_package_manager_unresolved` emitted,
   `typescript_runner_hint_unresolved` NOT emitted, `vitest run` command
   available.

2. `gap3_no_framework_no_lockfile_emits_strong_runner_unresolved`
   — Control 2 (strong fail-closed preserved): package.json with only
   `typescript` devDep, no lockfile → framework=None, runner=None →
   `typescript_runner_hint_unresolved` emitted, `typescript_package_manager_unresolved`
   NOT emitted, no verify command.

3. `gap3_vitest_pnpm_lockfile_no_limitation`
   — Control 3 (both resolved, no limitation): vitest + pnpm-lock.yaml →
   framework=Vitest, runner=Pnpm → no runner/PM limitation, command available.

## Non-Goals

- Does NOT change command derivation (`verify_command_for_discovery` is unchanged).
- Does NOT resolve the package manager; the uncertainty is honestly disclosed.
- Does NOT change `schema_version` (additive evidence string).
- Does NOT change language status (TypeScript stays Preview).
- Does NOT bump crate version or trigger a publish.
- Does NOT affect the genuinely-unactionable case (no framework AND no runner).
- Does NOT add new fixture inputs (uses existing fixture infrastructure for
  golden re-bless).

## Acceptance Examples

### Example A: vitest + no lockfile (issue #1239 repro)

Input: `package.json` with `vitest` in devDependencies, no lockfile present.

```
typescript_package_limitation: typescript_package_manager_unresolved
typescript_verify_command: vitest run tests/foo.test.ts
```

`typescript_runner_hint_unresolved` must NOT appear.

### Example B: no framework, no lockfile (fail-closed preserved)

Input: `package.json` with only `typescript` in devDependencies, no lockfile.

```
typescript_package_limitation: typescript_framework_hint_unresolved
typescript_package_limitation: typescript_runner_hint_unresolved
```

No `typescript_verify_command` line. No `typescript_package_manager_unresolved`.

### Example C: vitest + pnpm-lock.yaml (both resolved)

Input: `package.json` with `vitest` in devDependencies, `pnpm-lock.yaml` present.

```
typescript_package_confidence: high
typescript_verify_command: vitest run tests/foo.test.ts
```

No runner or PM limitation emitted.

## Test Mapping

| Test | Control case |
|---|---|
| `gap3_vitest_no_lockfile_emits_package_manager_unresolved_not_runner_unresolved` | A (repro) |
| `gap3_no_framework_no_lockfile_emits_strong_runner_unresolved` | B (fail-closed) |
| `gap3_vitest_pnpm_lockfile_no_limitation` | C (both resolved) |

Golden fixtures re-blessed (framework known, no lockfile — previously wrong):

- `fixtures/ts_runner_detect_ava_devdep` — ava devdep, no lockfile
- `fixtures/ts_runner_detect_ava_script` — ava from composite script, no lockfile
- `fixtures/typescript_monorepo_package_local` — jest framework, no lockfile

Golden fixtures unchanged (no framework, no lockfile — keep strong limitation):

- `fixtures/ts_runner_detect_no_runner` — no framework, no lockfile (no change)

## Implementation Mapping

| Behavior | Code location |
|---|---|
| `PackageManagerUnresolved` variant and wire name | `crates/ripr/src/analysis/language/typescript/package.rs::TsPackageLimitation` |
| Limitation selection logic | `crates/ripr/src/analysis/language/typescript/package.rs::resolve_package_discovery` |
| Wire name documentation | `docs/OUTPUT_SCHEMA.md` |
| Spec registration | `policy/doc-artifacts.toml`, `docs/specs/README.md` |
| Traceability | `.ripr/traceability.toml` |

## Metrics

- `typescript_package_manager_unresolved_disclosure_honesty`: presence of the
  new informational limitation in framework-known, lockfile-absent findings.
  Expected: emitted for every fixture and real-world project with a known
  framework but no lockfile present.
