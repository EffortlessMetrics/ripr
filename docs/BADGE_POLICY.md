# Badge Policy

`ripr` exposes two badges. Both count **unresolved** static findings —
inbox-zero, not coverage. This doc fixes the vocabulary, the counting rule,
the JSON shape, and what the badge does and does not prove.

This is the contract that `ripr check --format badge-json` and
`--format badge-shields` will render against. It pairs with
[Static exposure model](STATIC_EXPOSURE_MODEL.md),
[Output schema](OUTPUT_SCHEMA.md), and
[Configuration](CONFIGURATION.md).

## Status

This is the policy document. The badge command and the test-intent /
suppressions config files are planned work in Campaign 4A. The current
implementation status of each piece is tracked in the status table at the
bottom of this doc and in [`.ripr/goals/active.toml`](../.ripr/goals/active.toml).

## What each badge means

### `ripr 0`

```text
ripr found zero unsuppressed static exposure gaps under the configured policy.
```

Counts only exposure-class findings from `ripr check`. It is the same engine
and the same findings; the badge is a count-and-render policy on top.

### `ripr+ 0`

```text
ripr found zero unsuppressed static exposure gaps and zero unsuppressed
actionable test-efficiency findings.
```

`ripr+` adds the test-efficiency signals from
`cargo xtask test-efficiency-report` to the count. A passing `ripr+` is
strictly stronger than a passing `ripr`.

### What neither badge proves

A green badge does **not** mean:

- the code is fully tested
- mutants would fail under the test suite
- there are no behavioral bugs
- coverage is high

A green badge means: under the static evidence `ripr` could gather, no
unresolved gaps or actionable test-efficiency findings remain after applying
the configured suppressions and test-intent declarations. Mutation testing
remains the runtime confirmation step. See the closing wording in the
[product contract](../AGENTS.md#product-contract) and
[`STATIC_EXPOSURE_MODEL.md`](STATIC_EXPOSURE_MODEL.md).

## Why no denominator

The badge does **not** show `0/2300`.

A denominator reads as a coverage fraction ("2300 things to cover, 0 covered"),
which is exactly the wrong mental model. `ripr` is not measuring coverage; it
is measuring whether changed behavior appears exposed to a meaningful oracle.

The badge is an **inbox-zero** counter: zero unresolved findings is the
target, like inbox zero. Scope, unknowns, suppressed findings, intentional
findings, and total analyzed counts all live in the detailed JSON and
markdown reports — not in the badge message.

Avoid:

```text
ripr 0/2300        # reads as incomplete coverage
ripr coverage 0    # ripr is not a coverage tool
ripr uncovered 0   # same problem
```

Prefer:

```text
ripr 0
ripr+ 0
```

Or, if disambiguation is needed in dense badge bars:

```text
ripr gaps 0
ripr+ issues 0
```

## Exposure-class counting

These come from `ExposureClass` in
[`crates/ripr/src/domain/`](../crates/ripr/src/domain/) and from the
classification table in
[`STATIC_EXPOSURE_MODEL.md`](STATIC_EXPOSURE_MODEL.md#exposure-classes).

| Exposure class | Counts in `ripr` | Counts in `ripr+` | Notes |
| --- | :---: | :---: | --- |
| `weakly_exposed` | yes | yes | Default exposure gap. |
| `reachable_unrevealed` | yes | yes | Default exposure gap. |
| `no_static_path` | yes | yes | Default exposure gap. |
| `exposed` | no | no | Already exposed; not a gap. |
| `infection_unknown` | no | no | Reported separately as `unknowns`. |
| `propagation_unknown` | no | no | Reported separately as `unknowns`. |
| `static_unknown` | no | no | Reported separately as `unknowns`. |

Unknowns are first-class in `ripr`. They mean static analysis stopped, not
that a gap exists. They are visible in the badge JSON's `counts.unknowns`,
and visible in the human and JSON reports with their stop reasons. They do
not move the badge number unless a future policy explicitly opts in via
`--include-unknowns`.

## Test-efficiency vocabulary (locked)

The badge counts use the exact strings emitted by
`cargo xtask test-efficiency-report`. The source of truth is
[`xtask/src/main.rs`](../xtask/src/main.rs) — function `test_efficiency_class`
for the class string and `test_efficiency_reasons` for the reason strings. If
you add a new class or reason there, update this table.

### Per-test class field (exactly seven values)

| `class` value | Counts in `ripr+` | Triggered when |
| --- | :---: | --- |
| `strong_discriminator` | no | Strong oracle and no other condition demoted the test. |
| `useful_but_broad` | no | Medium- or weak-strength oracle that still asserts something. Visible in reports. |
| `smoke_only` | yes (unless declared intent) | Smoke-strength oracle (e.g. `is_ok`, `is_err`, `unwrap`). |
| `likely_vacuous` | yes | A reason includes `no_assertion_detected`. |
| `possibly_circular` | yes (unless declared intent) | A reason includes `expected_value_computed_from_detected_owner_path`. |
| `duplicative` | yes (unless declared intent) | Test belongs to a duplicate-discriminator group: same owner set, role-aware activation signature, and oracle shape. Only `strong_discriminator`, `useful_but_broad`, and `smoke_only` entries are eligible to be promoted to `duplicative`; already-flagged classes are preserved. |
| `opaque` | no | No reached owners detected. Visible only; static analysis cannot judge. |

### Reason strings (exactly nine values)

These are not counted directly. They explain why a class fired and feed
suggested next steps. The table below documents them so the badge JSON's
`reason_counts` can be interpreted without reading source.

| Reason string | What it indicates |
| --- | --- |
| `no_assertion_detected` | The test body has no detected assertion. Demotes class to `likely_vacuous`. |
| `smoke_oracle_only` | Oracle class is `Smoke` (e.g. `is_ok`, `unwrap`, `expect`). |
| `relational_oracle` | Medium-strength relational assertion (`assert!(x > 0)`, `is_empty`, etc.). |
| `broad_oracle` | Weak-strength oracle that asserts something but not exact behavior. |
| `assertion_may_not_match_detected_owner` | Weak-oracle test where the assertion target may not be the changed owner. |
| `opaque_helper_or_fixture_boundary` | No owner call was statically resolved; demotes class to `opaque`. |
| `no_activation_literal_detected` | No literal activation values found in the test body. |
| `expected_value_computed_from_detected_owner_path` | The expected side of an `assert_eq!` calls back into the detected owner; demotes class to `possibly_circular`. |
| `duplicate_activation_and_oracle_shape` | The test shares an owner set, role-aware activation signature, and oracle shape with at least one other test; appended to existing reasons (e.g. `smoke_oracle_only`) and promotes the class to `duplicative`. |

### Visible-but-not-counted by default

- `opaque` — static analysis stopped. Counts in `unknowns_test_efficiency`,
  not in the `ripr+` headline. Intentionally distinct from "vacuous."
- `useful_but_broad` — broad oracle. Visible in reports as advisory. Becomes
  countable only when test-efficiency policy explicitly elevates it for the
  changed behavior, which is a future policy switch, not a v1 default.

### Test intent is additive metadata, not a class

Declared test intent (e.g. `intent = "smoke"` in `.ripr/test_intent.toml`)
is **not** rendered as a replacement `class` value. The original
`class` (`smoke_only`, `duplicative`, `useful_but_broad`, etc.) is
preserved so the report still tells reviewers what the static analyzer
saw. Intent is a layered, owner-and-reason-stamped declaration on top of
the signal:

```json
{
  "name": "cli_prints_help",
  "class": "smoke_only",
  "declared_intent": {
    "intent": "smoke",
    "owner": "devtools",
    "reason": "CLI startup and help text smoke test.",
    "source": ".ripr/test_intent.toml"
  }
}
```

`ripr+` consumes the `declared_intent` metadata to exclude declared
intentional findings from its count. There is no `intentional_smoke` or
`intentional_duplicate` *class* string — those would conflate the
analyzer's signal with the user's declaration.

The metric label `duplicate_discriminator_group_count` (delivered in
`test-efficiency/report-and-metrics`) is a count-of-groups label, not a
class. Today the equivalent value is `duplicate_groups.length` in the
test-efficiency JSON.

## Counting rule

```text
ripr count =
    findings where exposure_class ∈ { weakly_exposed,
                                      reachable_unrevealed,
                                      no_static_path }
    minus suppressed exposure-gap findings

ripr+ count =
    ripr count
  + tests where class ∈ { likely_vacuous,
                          possibly_circular,
                          smoke_only }
    and not declared intentional in .ripr/test_intent.toml
    and not suppressed in .ripr/suppressions.toml
  + tests in `duplicative` groups
    not declared intentional and not suppressed
```

`ripr` and `ripr+` are computed from the same `CheckOutput` and the same
test-efficiency JSON. The badge is a rendering policy over those, not a
separate analysis.

## JSON wire shape

There is **one** native schema. The Shields response is a projection at the
output boundary; it is never the source of truth.

### Native (`--format badge-json`)

```json
{
  "schema_version": "0.1",
  "kind": "ripr",
  "label": "ripr",
  "message": "0",
  "status": "pass",
  "color": "brightgreen",
  "counts": {
    "unsuppressed_exposure_gaps": 0,
    "unsuppressed_test_efficiency_findings": 0,
    "intentional_test_efficiency_findings": 0,
    "suppressed_exposure_gaps": 0,
    "suppressed_test_efficiency_findings": 0,
    "unknowns": 0,
    "unknowns_test_efficiency": 0,
    "analyzed_findings": 0,
    "analyzed_tests": 0
  },
  "reason_counts": {
    "no_assertion_detected": 0,
    "smoke_oracle_only": 0,
    "relational_oracle": 0,
    "broad_oracle": 0,
    "assertion_may_not_match_detected_owner": 0,
    "opaque_helper_or_fixture_boundary": 0,
    "no_activation_literal_detected": 0,
    "expected_value_computed_from_detected_owner_path": 0,
    "duplicate_activation_and_oracle_shape": 0
  },
  "policy": {
    "include_unknowns": false,
    "fail_on_nonzero": false,
    "test_intent_path": ".ripr/test_intent.toml",
    "suppressions_path": ".ripr/suppressions.toml"
  }
}
```

`kind` is `"ripr"` or `"ripr_plus"`. The `_plus` form adds
`unsuppressed_test_efficiency_findings` to its `message`; the schema is
otherwise identical so consumers can parse one shape.

`schema_version` is the same scheme as `ripr check --json` so consumers can
gate on a single version. Bumping it is a public-contract change and must be
called out in the PR.

### Shields projection (`--format badge-shields`)

```json
{
  "schemaVersion": 1,
  "label": "ripr",
  "message": "0",
  "color": "brightgreen"
}
```

Shields requires `schemaVersion` (camelCase) and exactly four top-level
fields. The projection is mechanical: drop everything except `label`,
`message`, `color`; map `schema_version` → `schemaVersion: 1`.

Both formats are derived from the same internal `BadgeSummary`. That type is
intentionally **not public** — it lives in a private rendering module
(`crates/ripr/src/output/badge.rs` when implemented) and the public API
remains the JSON shape. This keeps `cargo xtask check-public-api` green and
matches the existing pattern (`output::json::render` is private; the JSON
contract is what's stable).

## Colors and status thresholds

Conservative defaults. Tunable later.

| `count` | `status` | `color` |
| --- | --- | --- |
| 0 | `pass` | `brightgreen` |
| 1–3 | `warn` | `yellow` |
| 4+ | `warn` | `orange` |
| any, with `--fail-on-nonzero` and count > 0 | `fail` | `red` |

`status` is independent of CI exit code. CI exit is governed by
`--fail-on-nonzero`; the badge always renders. A `warn` status on `main`
should never block a release on its own.

These thresholds will trip noisily on small diffs that legitimately have 4
weak findings. A diff-relative threshold (e.g. yellow at any nonzero,
orange when ratio of unresolved-to-analyzed exceeds a bound) is on the table
for v2 once we have real-world numbers from CI artifacts (PR
`ci/badge-artifacts`). For v1, absolute is simpler to reason about.

## CLI shape

The badge is a render-time policy over `CheckOutput`. Reuse `ripr check`
rather than introducing a new top-level command:

```bash
ripr check --base origin/main --format badge-json
ripr check --base origin/main --format badge-shields

ripr check --base origin/main --format badge-plus-json
ripr check --base origin/main --format badge-plus-shields
```

The `badge-plus-*` formats read `target/ripr/reports/test-efficiency.json`
(relative to `--root`). If the report is missing, the command fails with a
clear error pointing at `cargo xtask test-efficiency-report`. CI artifact
wiring (`ci/badge-artifacts`) will eventually generate the report as part
of the badge pipeline; until then, callers must regenerate the report
explicitly when test-efficiency state changes.

Reasoning. The current top-level commands are `check`, `explain`, `context`,
`doctor`, `lsp`. Each is a distinct *operation*. A badge is the same
operation as `check` rendered differently. Keeping it as a `--format` choice:

- avoids growing the public CLI surface and the LSP/extension command tables
- means `--root`, `--base`, `--diff`, `--mode`,
  `--no-unchanged-tests` already work without re-implementation
- matches how `--json` and `--format github` already behave

If a dedicated `ripr badge` ergonomic alias is added later, this doc must be
updated to call it out as a deliberate choice.

### Useful flags (planned)

These belong on `ripr check` once the badge formats land. They are scoped to
the badge formats — they do not affect human/json/github output.

| Flag | Default | Effect |
| --- | --- | --- |
| `--include-unknowns` | off | Add unknowns to the badge count. |
| `--fail-on-nonzero` | off | Exit nonzero when count > 0. CI-only knob. |
| `--test-intent PATH` | `.ripr/test_intent.toml` | Override the test-intent file. |
| `--suppressions PATH` | `.ripr/suppressions.toml` | Override the suppressions file. |
| `--show-suppressed` | off | Include suppressed findings in the human badge summary. |

There are intentionally **no** inline allow/suppress CLI flags. Durable
exceptions belong in files with `reason` and `owner`, not in shell history.

## Test intent and suppressions

Two files, two purposes. Both are planned for Campaign 4A.

### `.ripr/test_intent.toml` — positive declarations

Use when a test is intentionally smoke, intentionally duplicates a structurally
similar test for a separate business case, or uses an opaque oracle by design.
Declared tests stay visible in the report but do not move the `ripr+` count.

```toml
[[test_intent]]
test = "cli_prints_help"
intent = "smoke"
reason = "CLI startup and help text smoke test."
owner = "devtools"
```

Supported intents (initial set): `smoke`, `business_case_duplicate`,
`opaque_external_oracle`, `integration_contract`, `performance_guard`,
`documentation_example`. Adding a new intent is a doc + schema PR, not an
ad-hoc string.

### `.ripr/suppressions.toml` — exceptions for non-intent cases

Use for known exposure gaps covered by oracles `ripr` cannot see today, or
for accepted-risk cases pending later work.

```toml
[[suppressions]]
kind = "exposure_gap"
finding_id = "probe:src/pricing.rs:88:predicate"
reason = "Covered by integration test in tests/billing/integration.rs that ripr cannot statically inspect yet."
owner = "billing"
expires = "2026-09-01"
```

Rules (enforced when the loader lands):

- `reason` required, free-form but durable
- `owner` required
- `expires` strongly encouraged; expired entries surface as a separate count
- suppressed findings remain visible in the report
- the badge `counts.suppressed_*` fields show the count

`test_intent` ships before `suppressions` so smoke and duplicate tests don't
have to be "suppressed" merely for being intentional.

## CI policy

Advisory by default.

```bash
ripr check --base origin/main --format badge-json     > target/ripr/reports/ripr-badge.json
ripr check --base origin/main --format badge-shields  > target/ripr/reports/ripr-badge-shields.json
ripr check --base origin/main --format badge-plus-json    > target/ripr/reports/ripr-plus-badge.json
ripr check --base origin/main --format badge-plus-shields > target/ripr/reports/ripr-plus-badge-shields.json
```

CI uploads these as artifacts and links them from
`target/ripr/reports/index.md`. CI does **not** fail on a nonzero badge
count unless a workflow explicitly passes `--fail-on-nonzero`. Trunk-only
publication of Shields endpoints (`badge/publish-main-endpoint`) requires a
`policy/network_allowlist.txt` entry and runs only from `main`, not from PR
workflows.

## Implementation status

Tracked alongside Campaign 4A in
[`.ripr/goals/active.toml`](../.ripr/goals/active.toml) and
[`docs/IMPLEMENTATION_CAMPAIGNS.md`](IMPLEMENTATION_CAMPAIGNS.md).

| Component | Status | Source |
| --- | --- | --- |
| Test fact ledger | done | `cargo xtask test-efficiency-report` |
| Vacuity signals (the 6-class table above, minus duplicate) | done | same |
| Duplicate-discriminator grouping | done | `test-efficiency/duplicate-discriminator-v1` |
| Test-efficiency report metrics | done | `test-efficiency/report-and-metrics` |
| Private `BadgeSummary` model and renderer | done | `badge/summary-renderer-v1` |
| `ripr check --format badge-json` / `badge-shields` | done | `badge/ripr-count-v1` |
| `.ripr/test_intent.toml` loader | done | `test-intent/v1` |
| `ripr check --format badge-plus-*` | done | `badge/ripr-plus-count-v1` |
| `.ripr/suppressions.toml` loader | ready | `suppressions/v1` |
| CI badge artifacts | not started | `ci/badge-artifacts` |
| Published Shields endpoint from `main` | not started | `badge/publish-main-endpoint` |

## See also

- [Static exposure model](STATIC_EXPOSURE_MODEL.md) — exposure classes and stage states.
- [Output schema](OUTPUT_SCHEMA.md) — stable JSON shape for `ripr check --json`.
- [Configuration](CONFIGURATION.md) — current vs planned config surfaces.
- [Implementation campaigns](IMPLEMENTATION_CAMPAIGNS.md) — Campaign 4A status.
- [Roadmap](ROADMAP.md) — long-range plan including badge work.
