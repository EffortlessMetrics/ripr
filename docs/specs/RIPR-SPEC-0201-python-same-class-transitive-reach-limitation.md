# RIPR-SPEC-0201: Python same-class transitive-reach limitation

Status: proposed

Owner: product / swarm

Created: 2026-09-29

Linked issues:

- [#4765](https://github.com/EffortlessMetrics/ripr-swarm/issues/4765) —
  method owners reached only through other methods are a silent `no_static_path`.

Linked PRs:

- None yet

Support-tier impact:

- No tier change. This spec adds an additive `static_limit_kind` value,
  `python_transitive_reach_unresolved`, on Python `no_static_path` findings
  where a test constructs or calls into the owner's class and a bounded
  same-class `self.` / `cls.` walk (depth ≤ 5) may reach the changed method.
  Classification stays `no_static_path` (fail-closed); no promotion to
  `weakly_exposed` or higher, and the witness is not a `related_tests` entry.
- No `schema_version` bump is required.
- Claim boundaries remain governed by the canonical ledger in
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- Register `PythonTransitiveReachUnresolved` in `crates/ripr/src/domain/language.rs`.
- No new crates, binaries, dependencies, parsers, runtime executors, or LSP
  servers introduced.

## Problem

A changed line inside a Python class method that tests reach only through the
object's public surface (another method, a protocol method such as
`__rich_console__`, or construction plus an internal `self.` path) is reported
`no_static_path` with 0 related tests and no limitation. The Rust adapter names
the analogous situation (`rust_transitive_reach_unresolved`, SPEC-0114). Python
says nothing, so the finding reads as "no test reaches this".

Relating those tests as `helper_owner_call` is a later slice and depends on
[#4568](https://github.com/EffortlessMetrics/ripr-swarm/issues/4568)
(function-to-helper / cross-module façade). This spec names the limitation only.

## Behavior

### Trigger condition

After the Python classifier returns `ExposureClass::NoStaticPath` with empty
`related_tests` and no existing `static_limit_kind`:

1. The owner is a class `Method` or `ClassMethod`.
2. Another method on the same class has a bounded same-class path to the owner
   through `self.` / `cls.` calls or bound-method aliases (`fn = self.x` then
   `fn(`), depth 1..=5.
3. A test import-provenances that class from the owner's module (full dotted
   module path, not a coincidental file stem) and either:
   - constructs it (`Class(` / alias / `module.Class(`), or
   - calls a class method that itself BFS-reaches the owner.

Nested functions, lambdas, `@staticmethod` bodies, `getattr`, `super()`, and
non-receiver attributes are not edges. A same-named class imported from another
module does not fire. A cross-module function façade that never constructs the
owner class does not fire.

### When found

Set `Finding.static_limit_kind = Some(StaticLimitKind::PythonTransitiveReachUnresolved)`.
Push `StopReason::TransitiveReachUnresolved`.
Push an honest "may" limitation message, a witness pointer, and the four
structured evidence prefixes (`limitation_last_established_edge`,
`limitation_first_unresolved_edge`, `limitation_analyzer_route`,
`limitation_non_claim`).

Replace any "No Python test references" missing line so the finding does not
claim that no tests exist.

The classification **stays `no_static_path`**. The witness is **not** added to
`related_tests`. This is a named limitation, not a reach or coverage claim.

### When not found

Leave the finding exactly as the direct-call classifier produced it.

## Required Evidence

- Positive fixture: `fixtures/python_transitive_reach_positive/` — limitation fires.
- Negative fixture: `fixtures/python_transitive_reach_negative/` — limitation does not fire
  for a same-named other-module class.
- Unit tests in `crates/ripr/src/analysis/language/python/transitive_reach.rs`
  covering construction, alias/module import, bound-method alias, classmethod
  `cls.` edges, import-without-construct, other-module class, missing self-path,
  getattr/foreign receiver, nested function, lambda, depth 5 vs 6, direct
  `.owner(` remaining related, and a `requests.post`-style façade staying silent.

## Non-Goals

- Relating the witnessing test at medium confidence (`helper_owner_call`).
- Tracing dunder protocol dispatch (`console.print(table)` → `__rich_console__`)
  except insofar as constructing the class plus an internal `self.` path is
  enough to *name* the limitation.
- Function-to-helper and cross-module façade reach (#4568).
- Promoting classification, emitting a repair packet, or claiming coverage.

## Acceptance Examples

| Case | Result |
|---|---|
| `table = Table()` and `render` → `_get_padding_width` via `self.` | `no_static_path` + `python_transitive_reach_unresolved` |
| `from other.models import Table; Table()` | silent `no_static_path` |
| `from src.table import Table` with no construct/call | silent `no_static_path` |
| `table._get_padding_width(` in the test | related test; this limitation does not attach |
| `requests.post(...)` without constructing `PreparedRequest` | silent `no_static_path` (#4568) |
| six-hop `self.` chain | silent `no_static_path` |

## Test Mapping

- `crates/ripr/src/analysis/language/python/transitive_reach.rs`
- `crates/ripr/src/domain/language.rs::tests::static_limit_kind_wire_strings_are_stable`
- `fixtures/python_transitive_reach_positive/`
- `fixtures/python_transitive_reach_negative/`

## Implementation Mapping

| Component | Location |
|---|---|
| `StaticLimitKind::PythonTransitiveReachUnresolved` | `crates/ripr/src/domain/language.rs` |
| Same-class callee facts | `crates/ripr/src/analysis/language/python/same_class_callees.rs` |
| BFS + post-classify attach | `crates/ripr/src/analysis/language/python/transitive_reach.rs` |
| Classifier wiring | `crates/ripr/src/analysis/language/python/classify.rs` |
| Stop-reason map | `crates/ripr/src/analysis/language/python.rs` |

## Metrics

- Gate: 0 golden drift on existing fixtures; both new transitive-reach fixtures passing.
- Behavioral repro: positive fixture shows `static_limit_kind: "python_transitive_reach_unresolved"`,
  classification stays `no_static_path`, `related_tests` empty. Negative fixture shows
  bare `no_static_path` with no `static_limit_kind`.
- Unit tests in `crates/ripr/src/analysis/language/python/transitive_reach.rs` cover
  construction, alias/module import, bound-method alias, classmethod, import-without-use,
  other-module class, missing self-path, getattr/foreign receiver, nested function, lambda,
  depth 5 vs 6, direct owner call, and a cross-module façade staying silent.

## Follow-up Boundary

Relating method-to-method tests as `helper_owner_call` remains owned by #4568
and is not part of this named-limitation slice.
