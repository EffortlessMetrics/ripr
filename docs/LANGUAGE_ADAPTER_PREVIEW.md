# Language Adapter Preview Workflow

Use this guide when a repository wants to evaluate TypeScript, JavaScript, or
Python evidence without changing the Rust-first default path.

Preview adapters are useful for advisory review, editor projection, and agent
handoff. They are not a maturity claim equal to Rust, they do not run a
typechecker or test runner, and they do not make generated CI blocking.

## Current Shape

| Language | Status | Default | Evidence scope |
| --- | --- | --- | --- |
| Rust | reference path | enabled | Rust static exposure evidence and the existing CLI, CI, editor, report, and gate surfaces. |
| TypeScript and JavaScript | preview | disabled | Syntax-first owners, tests, assertions, probes, related tests, and visible static limits for `.ts`, `.tsx`, `.js`, and `.jsx`. |
| Python | preview static facts; scoped repair routing is `usable alpha` | detected Python projects without `ripr.toml`; otherwise disabled unless configured | Syntax-first owners, tests, assertions, probes, related tests, RIPR-stage evidence, selected repair-class missing discriminators, fail-closed static limits, and bounded repair cards/packets for selected pytest/unittest routes. |

The preview adapters feed the same output schema and review surfaces as Rust.
Preview findings carry additive metadata such as `language`,
`language_status`, `owner_kind`, and `static_limit_kind` when the adapter can
populate them.

## Enable Preview Adapters

Add preview languages to the repo-root `ripr.toml` when you want explicit
control:

```toml
[languages]
enabled = ["rust", "typescript", "python"]
```

This runtime opt-in is separate from build-time adapter availability. The
default published `ripr` build includes the TypeScript/JavaScript and Python
preview adapter features. Rust-only binaries can be built with:

```bash
cargo build -p ripr --no-default-features --features lang-rust
```

If a Rust-only binary reads config that enables `typescript` or `python`, it
fails closed with a configuration error naming the missing Cargo feature. The
editor should surface that unavailable-adapter status instead of publishing
preview diagnostics.

Use the smallest list that matches the repository. For a Python-only preview
evaluation in an otherwise Rust repo, use:

```toml
[languages]
enabled = ["rust", "python"]
```

Confirm the loaded configuration:

```bash
ripr doctor --root .
```

`doctor` should show the loaded config and enabled languages. Missing
`ripr.toml` remains healthy. With no Python project markers, the built-in
Rust-only default is active. With Python project markers such as
`pyproject.toml`, `setup.py`, `requirements.txt`, `pytest.ini`, `tox.ini`,
`noxfile.py`, or Python files under `src/` or `tests/`, `ripr` enables Python
preview analysis for that repository root.

An explicit `ripr.toml` remains authoritative. To keep Python preview disabled
in a Python-shaped repo, add:

```toml
[languages]
enabled = ["rust"]
```

## Bun UB Advisory Profile

Bun stable-byte review can opt into the calibrated TypeScript-family preview
profile without making TypeScript or JavaScript default-on:

```toml
[languages]
enabled = ["rust", "typescript"]

[profiles.bun_ub]
test_roots = [
  "test/js/**/*.test.ts",
  "test/js/**/*.test.js",
]
bridge_hints = "ripr.bun.bridge.toml"
```

The `typescript` adapter covers `.ts`, `.tsx`, `.js`, and `.jsx` files, so the
profile does not add a separate `javascript` language key. `profiles.bun_ub`
is advisory configuration only: it records test roots and the bridge-hint file
for Bun Blob / ArrayBuffer cross-language evidence, and `ripr doctor --root .`
reports it when present. It does not run `tsc`, start `tsserver`, execute
Bun/Jest/Vitest, generate tests, edit source, affect gates, badges, baselines,
or RIPR Zero, or promote TypeScript/JavaScript beyond preview.

Use the [Bun UB TypeScript preview runbook](BUN_UB_TYPESCRIPT_PREVIEW_RUNBOOK.md)
for the operator loop that reads `rust_ungripped_ts_discriminated`,
`rust_ungripped_ts_missing_discriminator`, `ts_mention_not_observer`, and
`bridge_unknown` results, plus
`public_reachable_panic_boundary_unrevealed` FFI limitation receipts.

## Run The Local Preview Loop

Start with the normal first-run loop:

```bash
ripr pilot --root .
```

For diff-scoped output:

```bash
ripr check --root . --format human
ripr check --root . --json > target/ripr/reports/check.json
```

Read preview findings as syntax-first advisory evidence:

- `language = "typescript"` or `language = "python"` identifies the adapter.
- `language_status = "preview"` means the finding is opt-in and advisory.
- `owner_kind` explains the syntactic owner shape when known.
- `static_limit_kind` names a known static limitation instead of hiding it.
- Python `missing_discriminators` are syntax-derived preview repair evidence
  only. Direct weak findings may name boundary, return, exception, field, or
  output/log/call-effect discriminators; heuristic, no-path, and static-limit
  findings do not become safe repair work.
- Detectable generated Python files such as `*_pb2.py`, `*_pb2_grpc.py`,
  `*.generated.py`, `*_generated.py`, and `generated_*.py` are excluded from
  preview diff analysis instead of being routed as repairable behavior changes.

Python repair routing has a separate scoped `usable alpha` support-tier row for
selected pytest/unittest workflows. That row covers direct weak findings that
carry a canonical gap, missing discriminator, suggested test target, verify
command, stop conditions, bounded agent packet, and before/after receipt. It
does not promote broader Python static facts, static limits, generated CI,
gates, badges, baselines, or RIPR Zero.

The exposure class still uses RIPR's normal conservative vocabulary:
`exposed`, `weakly_exposed`, `reachable_unrevealed`, `no_static_path`,
`infection_unknown`, `propagation_unknown`, and `static_unknown`.

## Interpret Static Limits

Static limits are part of the finding context, not a separate verdict. They
explain where syntax-first analysis can see a useful shape but cannot infer the
runtime behavior. See [Static limits](STATIC_LIMITS.md) for the user-facing
interpretation guide and integration rules.

| Static limit | How to read it |
| --- | --- |
| `dynamic_dispatch` | The call target is selected dynamically, such as computed member calls (`obj[name]` followed by invocation) or `getattr(obj, name)(...)`. |
| `metaprogramming` | The code shape can change behavior through metaprogramming, such as decorators, proxies, or metaclasses. |
| `missing_import_graph` | The adapter did not resolve a full project import graph. |
| `decorator_indirection` | A Python decorator may change the callable boundary. Simple route decorators such as `@api.post(...)` can still be treated as static route metadata when the changed body has a supported repair shape. |
| `mocked_module` | A test replaces or mocks the module or symbol under review. |
| `opaque_custom_assertion_helper` | A Python test uses a custom assertion helper whose body is not inspected. |
| `property_based_test` | A Python test uses generated inputs, such as Hypothesis `@given(...)`, whose concrete cases are not known statically. |
| `unresolved_pytest_fixture` | A Python pytest test depends on fixture-sourced values whose concrete inputs or expected values are not known statically. |
| `unsupported_syntax` | The parser or preview adapter found syntax outside the current preview contract. |

A static limit does not automatically erase a useful related test or strong
oracle. It means the reviewer should keep the limitation visible while deciding
the next test change.

## Read Mixed-Language Reports

In JSON outputs, preview findings remain in the same arrays as Rust findings.
Filter by `language` and `language_status` when a tool needs to separate them.

In human output, preview language/status and static-limit lines appear before
the evidence narrative. In review artifacts, preview evidence remains advisory
unless a later policy explicitly promotes it.

Promotion requires a policy-owned preview promotion packet, not just adapter
routing. The scoped Python repair-routing support-tier review is limited to
selected pytest/unittest repair work and keeps broader Python facts advisory.
Future promotion beyond that scope still requires fixture matrix coverage,
dogfood receipts, related-test accuracy review, static-limit taxonomy coverage,
false-positive review, false repair packet review, surface consistency, and
policy-owner signoff. JavaScript evidence shares the TypeScript-family adapter,
but it stays JavaScript preview evidence unless a later packet explicitly names
that scope.

Useful references:

- [Output schema](OUTPUT_SCHEMA.md) for field contracts.
- [Support tiers](status/SUPPORT_TIERS.md) for maturity and trust boundaries.
- [Python repair routing proposal](proposals/RIPR-PROP-0017-python-repair-routing-lane.md)
  for the end-state repair-card and receipt loop and its scoped `usable alpha`
  support-tier boundary.
- [Python repair routing usable-alpha closeout](handoffs/2026-05-31-python-repair-routing-usable-alpha-closeout.md)
  for the proof and remaining limits behind the scoped support-tier row.
- [Preview promotion criteria](policy/PREVIEW_PROMOTION_CRITERIA.md) for the
  proof required before a preview language/class can be reviewed for stronger
  status.
- [Capability matrix](CAPABILITY_MATRIX.md) for proof artifacts.
- [Language adapter preview dogfood receipts](handoffs/2026-05-13-language-adapter-preview-receipts.md)
  for the checked TypeScript/Python preview receipt cases.
- [TypeScript preview static facts](specs/RIPR-SPEC-0027-typescript-preview-static-facts.md).
- [Python preview static facts](specs/RIPR-SPEC-0028-python-preview-static-facts.md).

## Editor Workflow

The VS Code extension can activate on TypeScript, TSX, JavaScript, JSX, and
Python files, but analysis still follows repo language selection. TypeScript
and JavaScript require `[languages]`; Python may be selected by project
detection when `ripr.toml` is absent. If an explicit repo config does not enable
a preview language, that language should not produce preview diagnostics.

When preview diagnostics appear:

- hover shows the preview syntax-first and advisory boundary before the RIPR
  evidence;
- static-limit kinds appear before action language;
- actions use structured diagnostic data, not parsed prose;
- Rust saved-workspace behavior remains the default path.

Use the normal editor recovery commands when diagnostics look stale:

```text
ripr: Show Status
ripr: Show Output
ripr: Restart Server
Refresh Analysis - Saved Workspace Check
```

## Generated CI Workflow

Generate or refresh the advisory GitHub workflow with:

```bash
ripr init --ci github
```

Generated CI remains advisory by default. When `ripr doctor` reports configured
preview languages, the job summary adds a `Language preview grouping` section
for TypeScript, JavaScript, and Python evidence. Rust-only configuration keeps
that section hidden at runtime.

When TypeScript preview is configured, the summary treats TypeScript-family
adapter output as separately labeled `typescript` and `javascript` preview
groups. Each group reports advisory artifact counts, preview-status counts,
static-limit context, actionability state/category counts,
repair-packet-ready counts, and `gate_impact = none`.

The generated summary may group and summarize preview artifacts, but pass/fail
authority remains with explicit gate-decision artifacts when a repository has
configured them. The summary itself is not a gate.

## Roll Back Preview Evidence

Remove the preview language from `ripr.toml`:

```toml
[languages]
enabled = ["rust"]
```

Then refresh the relevant surface:

```bash
ripr doctor --root .
ripr pilot --root .
```

For CI, commit the config change and rerun the generated workflow. For editor
state, save the workspace file or run `Refresh Analysis - Saved Workspace
Check`.

Rollback does not require deleting generated reports by hand. New runs should
stop producing preview findings for disabled languages. Treat older artifacts
as stale evidence from the prior configuration.

## What Not To Infer

Preview language evidence does not mean:

- runtime mutation execution happened;
- TypeScript or Python reached Rust maturity;
- a typechecker, import graph, virtual environment, package install, or test
  runner was invoked;
- generated CI became blocking;
- preview findings are eligible for RIPR Zero, baseline debt, or gate policy by
  default;
- RIPR edited source files or generated tests.

The useful claim is narrower: for explicitly enabled preview languages, RIPR can
surface syntax-first static evidence, related tests, missing discriminators,
static limits, and repair-oriented next actions in the same review surfaces used
by Rust.
