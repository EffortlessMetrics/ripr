# Badge Policy

`ripr` exposes two badges. Both count **unresolved** static exposure gaps —
inbox-zero, not coverage. Diff-scoped badges preserve the legacy Finding
exposure basis; repo-scoped public badges use seam-native classified repo
seams. This doc fixes the vocabulary, the counting rule, the JSON shape, and
what the badge does and does not claim.

This is the contract that `ripr check --format badge-json` and
`--format badge-shields` will render against. It pairs with
[Static exposure model](STATIC_EXPOSURE_MODEL.md),
[Output schema](OUTPUT_SCHEMA.md), and
[Configuration](CONFIGURATION.md).

## Status

This is the policy document. The badge command, the test-intent and
suppressions config files, the diff-scoped CI artifact pipeline, the
repo-scoped artifact path, and the trunk-only public Shields endpoint
have all landed under Campaign 4A. The current implementation status
of each piece is tracked in the status table at the bottom of this
doc and in
[`.ripr/goals/active.toml`](../.ripr/goals/active.toml).

## What each badge means

### `RIPR gaps 0`

```text
RIPR found zero unsuppressed static exposure gaps under the configured policy.
```

Diff scope counts exposure-class findings from `ripr check`. Repo scope counts
classified seams from the repo seam inventory, using configured seam severity.
The badge is a count-and-render policy on top of existing analyzer output.

### `RIPR+ gaps 0`

```text
RIPR found zero unsuppressed static exposure gaps and zero unsuppressed
actionable test-efficiency findings.
```

`RIPR+ gaps` adds the test-efficiency signals from
`cargo xtask test-efficiency-report` to the count. A passing `RIPR+ gaps`
badge is strictly stronger than a passing `RIPR gaps` badge.

## Scope: diff vs repo

The same badge label renders at two different **scopes** depending on
input. The two scopes have different audiences and different meanings,
and conflating them produces misleading public signal.

### Diff scope (`scope: diff`)

The badge counts findings within the diff under analysis — typically
`origin/main...HEAD` for a PR. This is what `ripr check` produces by
default and what `cargo xtask badge-artifacts` writes for PR step
summaries.

- **Audience**: PR reviewers, CI step summary, PR artifact uploads.
- **Meaning**: "this PR's changed behavior has N unresolved
  findings under policy."
- **Not** a meaningful README / marketplace / store badge. On `main`
  itself the diff vs `origin/main` is empty, so a diff-scoped pure
  `RIPR gaps` badge always reports `0`. That is "nothing changed," not
  "the repo is clean."

### Repo scope (`scope: repo`)

The badge counts classified seams across the entire repo baseline. This is
the only scope that should be published as a public README, crate page, or
extension store badge.

- **Audience**: anyone reading the repo cold from outside.
- **Meaning**: "the current repo baseline has N unresolved seam-native
  exposure gaps under policy."
- Pure `ripr` repo scope is rendered from compact seam-class counts
  (`analysis::inventory_seam_grip_class_counts_at_with_config`) and
  uses `SeamGripClass` plus configured seam severity. The detailed
  `repo-exposure-*` reports still render full per-seam evidence; badge
  endpoints use the compact count cache under
  `target/ripr/cache/repo-seam-counts/` so public badges do not need to
  deserialize the full evidence cache. The CLI surface is
  `--format repo-badge-json`,
  `--format repo-badge-shields`, `--format repo-badge-plus-json`,
  and `--format repo-badge-plus-shields`; the xtask wrapper is
  `cargo xtask repo-badge-artifacts`.

### `ripr+` fact source vs aggregation scope

`cargo xtask test-efficiency-report` is repo-wide as a **fact source**:
it scans the entire test suite and records per-test evidence
(`reached_owners`, oracle kind/strength, observed values, declared
intent). Badge **aggregation** is scope-aware:

- **Diff-scoped `ripr+`** (`--format badge-plus-json`,
  `--format badge-plus-shields`) filters the test-efficiency ledger to
  entries *related to* the changed owners / findings in the analyzed
  diff. A test-efficiency entry is related when **either**:
  - the entry's bare or `<path>::<name>` form appears in any diff
    `Finding.related_tests`, or
  - the entry's `reached_owners` intersect the changed/probed owners
    drawn from `Finding.probe.owner`.

  Unrelated repo-wide debt (e.g. a `likely_vacuous` test in a module
  this PR doesn't touch) does **not** move the diff `ripr+` headline.
  Suppressions and declared intent still apply to the filtered set:
  declared intent moves a related entry into
  `intentional_test_efficiency_findings` (out of the headline);
  matched suppressions move a related actionable entry from
  `unsuppressed_test_efficiency_findings` to
  `suppressed_test_efficiency_findings`.

- **Repo-scoped `ripr+`** (`--format repo-badge-plus-json`,
  `--format repo-badge-plus-shields`) aggregates the repo-wide
  test-efficiency ledger directly — no relatedness filter. This is
  the intended source for public README / store badges.

The split keeps PR badges scoped to the tests that act on the changed
code while README / store badges remain repo-baseline signals. Native
badge JSON carries `basis` so consumers can distinguish
`finding_exposure` diff artifacts from `seam_native` repo artifacts.

#### What repo seam-native scope means — and does not mean

The repo baseline counts classified behavior seams from the seam inventory.
At repo root, seam discovery excludes repository automation and fixture data
(`xtask/` and top-level `fixtures/`) so the public badge represents the
published `ripr` package surface instead of the repository harness. An
individual fixture workspace is still analyzable when passed as `--root`.

Badge classification uses a compact, ranked related-test sample per seam.
That keeps public endpoint refreshes operable while preserving the same
`SeamGripClass` headline mapping as the detailed repo exposure report.
It is **not**:

- a complete inventory of every behavior seam in the repo
- confirmation that every behavior is tested
- confirmation of mutation adequacy
- a coverage metric

Public README / store badges that derive from
`cargo xtask badge-artifacts` are unsafe — that task generates
diff-scoped artifacts only.

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

The badge is an **inbox-zero** counter: zero unresolved gaps is the
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
RIPR gaps 0
RIPR+ gaps 0
```

## Exposure-class counting

These come from `ExposureClass` in
[`crates/ripr/src/domain/`](../crates/ripr/src/domain/) and from the
classification table in
[`STATIC_EXPOSURE_MODEL.md`](STATIC_EXPOSURE_MODEL.md#exposure-classes).

Exposure-class counting is the `finding_exposure` basis used by diff-scoped
badge artifacts. It remains documented and versioned for PR summaries and
legacy consumers.

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

## Seam-native repo counting

Repo-scoped badge artifacts use the `seam_native` basis. These counts come
from `SeamGripClass` in RIPR-SPEC-0005 and consume the configured seam
severity from `ripr.toml`.

| Seam grip class | Counts in repo `ripr` | Notes |
| --- | :---: | --- |
| `weakly_gripped` | yes, unless configured `off` | Headline-eligible seam gap. |
| `ungripped` | yes, unless configured `off` | Headline-eligible seam gap. |
| `reachable_unrevealed` | yes, unless configured `off` | Headline-eligible seam gap. |
| `activation_unknown` | yes, unless configured `off` | Headline-eligible static limitation. |
| `propagation_unknown` | yes, unless configured `off` | Headline-eligible static limitation. |
| `observation_unknown` | yes, unless configured `off` | Headline-eligible static limitation. |
| `discrimination_unknown` | yes, unless configured `off` | Headline-eligible static limitation. |
| `opaque` | no | Reported separately as `unknowns` when configured visible. |
| `strongly_gripped` | no | Default severity is `off`. |
| `intentional` | no | Default severity is `off`. |
| `suppressed` | no | Default severity is `off`; if configured visible, counted in `suppressed_exposure_gaps`. |

`severity.seams.<class> = "off"` omits that class from the badge headline and
visible count buckets. Other severities keep the class visible to the badge
mapping but do not change the badge color thresholds.

## Counting rule

Diff-scoped `ripr` count:

```text
ripr count =
    findings where exposure_class ∈ { weakly_exposed,
                                      reachable_unrevealed,
                                      no_static_path }
    minus suppressed exposure-gap findings
```

Repo-scoped `ripr` count:

```text
ripr count =
    seams where seam_grip_class is headline eligible
    and configured seam severity != off
```

`ripr+` adds test-efficiency findings to whichever `ripr` exposure basis was
selected by scope:

```text
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

Diff-scoped `ripr+` uses the diff's related-test filter. Repo-scoped `ripr+`
uses the repo-wide test-efficiency ledger directly. The badge is a rendering
policy over analyzer reports, not a separate analysis.

## JSON wire shape

There is **one** native schema. The Shields response is a projection at the
output boundary; it is never the source of truth.

### Native (`--format badge-json`)

```json
{
  "schema_version": "0.3",
  "kind": "ripr",
  "scope": "repo",
  "basis": "seam_native",
  "label": "RIPR gaps",
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
    "analyzed_seams": 120,
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

`schema_version` is the badge-native schema. Bumping it is a public-contract
change and must be called out in the PR. `0.3` adds `basis` and
`counts.analyzed_seams`.

### Scope and basis metadata (native only)

A `scope` field distinguishes PR artifacts from public repo badges. A `basis`
field distinguishes legacy diff finding counts from seam-native repo counts:

```json
{
  "schema_version": "0.3",
  "kind": "ripr",
  "scope": "diff",
  "basis": "finding_exposure",
  "label": "RIPR gaps",
  "message": "3",
  "...": "..."
}
```

- `"scope": "diff"` — diff-scoped (PR artifacts). Native JSON SHOULD
  also record `base` and `head` git refs so consumers can reproduce.
- `"scope": "repo"` — repo-scoped (README / main endpoint).
- `"basis": "finding_exposure"` — legacy `Finding`/`ExposureClass`
  aggregation, currently used by diff-scoped badge artifacts.
- `"basis": "seam_native"` — `RepoSeam`/`SeamGripClass` aggregation,
  currently used by repo-scoped badge artifacts.

The Shields projection remains exactly four fields. Scope and basis metadata
live only in native JSON, docs, and consumer tooling.

### Shields projection (`--format badge-shields`)

```json
{
  "schemaVersion": 1,
  "label": "RIPR gaps",
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

Advisory by default. PR runs and `main` runs render different surfaces.

### PR runs — diff-scoped

`cargo xtask badge-artifacts` invokes `ripr check` with the per-PR diff
(`git diff origin/main...HEAD`) and writes diff-scoped artifacts:

```bash
cargo xtask badge-artifacts
# writes target/ripr/reports/ripr-badge.json (scope: diff, planned)
# writes target/ripr/reports/ripr-badge-shields.json
# writes target/ripr/reports/ripr-plus-badge.json
# writes target/ripr/reports/ripr-plus-badge-shields.json
# writes target/ripr/reports/ripr-badges.md
```

Used for the PR step summary and uploaded as the `ripr-pr-reports`
artifact. CI does **not** fail on a nonzero badge count unless a
workflow explicitly passes `--fail-on-nonzero`. **These artifacts are
not safe to publish as README badges** — see "Scope: diff vs repo."

### `main` runs — repo-scoped

`cargo xtask repo-badge-artifacts` (`badge/repo-scope-artifacts`)
analyzes the full repo baseline rather than a diff and writes repo-
scoped artifacts:

```bash
cargo xtask repo-badge-artifacts
# writes target/ripr/reports/repo-ripr-badge.json (scope: repo)
# writes target/ripr/reports/repo-ripr-badge-shields.json
# writes target/ripr/reports/repo-ripr-plus-badge.json
# writes target/ripr/reports/repo-ripr-plus-badge-shields.json
# writes target/ripr/reports/repo-ripr-badges.md
```

### `ripr` badge product contract

The `ripr` badge product contract is a single sentence:

> `ripr` emits Shields-compatible JSON.

To put a `ripr` value into a README, that JSON has to be available at a
stable public URL. Hosting the JSON is a **separate, replaceable
layer** — `ripr` itself does not require any specific host.

| Question | Answer |
| --- | --- |
| Who computes the value? | A normal CI job that runs `cargo xtask repo-badge-artifacts`. |
| Who hosts the value? | Any stable public surface that can serve the resulting Shields JSON. |
| Does a downstream user have to enable GitHub Pages? | **No.** Pages is one host; it is not a requirement of `ripr`. |

Compare to existing badges in this repo's README:

| Badge | Computation | Hosting |
| --- | --- | --- |
| CI status | GitHub Actions | GitHub |
| Codecov | CI uploads coverage | Codecov |
| crates.io version | `cargo publish` | crates.io / Shields |
| Open VSX downloads | registry | Open VSX / Shields |
| `RIPR gaps` / `RIPR+ gaps` | `ripr` CI computes JSON | self-hosted (see below) |

The RIPR badge row is the one without a third-party host. Long-term
that gap is intended to close — see `deferred/hosted-badge-service` in
[`docs/DEFERRED.md`](DEFERRED.md). In the meantime, a self-hosted host
is required.

### Self-hosted dogfood endpoint (this repo)

This repo's v1 dogfood endpoint is **two Shields JSON files committed
to `main`**, served via `raw.githubusercontent.com`. It is not the
standard downstream-user publishing model.

The committed files live at:

```text
badges/ripr.json
badges/ripr-plus.json
```

Each is a minimal four-field Shields object, e.g.:

```json
{
  "schemaVersion": 1,
  "label": "RIPR gaps",
  "message": "163",
  "color": "orange"
}
```

The README renders those endpoints via:

```text
https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/EffortlessMetrics/ripr/main/badges/ripr.json
https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/EffortlessMetrics/ripr/main/badges/ripr-plus.json
```

Refreshing the committed files is one xtask command:

```bash
cargo xtask update-badge-endpoints
```

That regenerates `target/ripr/reports/repo-ripr-{badge,plus-badge}-shields.json`
via `repo_badge_artifacts()` and copies the two Shields projections
into `badges/`. Commit the resulting diff.

#### Pinned contract for the endpoint

- Only the two `badges/*.json` files are part of the public endpoint
  surface — no reports, no markdown, no diff-scoped artifacts, no
  `target/` snapshots.
- The endpoint URL points at the `main` branch via
  `raw.githubusercontent.com`. Shields/CDN cache layers can take
  minutes to refresh after a `main` push.
- Diff-scoped artifacts (`ripr-badge-shields.json`,
  `ripr-plus-badge-shields.json`) stay in per-PR step summaries and
  CI artifact uploads — never linked from public docs.
- `cargo xtask check-badge-endpoints` verifies the committed files
  against a fresh `repo-badge-artifacts` run. It is **not** wired into
  the default CI gate set in v1: the headline drifts whenever
  production code or tests change, and requiring every PR to also
  refresh `badges/` would be too much friction before the count
  stabilizes. Use it locally before campaign closeouts and after
  material analyzer changes.
- The `RIPR gaps 0` headline on `main` means: zero configured-visible
  seam-native unresolved exposure gaps under the current repo baseline.
  It does not mean the repo is fully tested, that all behavior seams
  are gripped by oracles, or that runtime mutation confirmation would
  pass — see "What repo seam-native scope means — and does not mean"
  above.

#### Why checked-in JSON, not GitHub Pages

An earlier shape of this work used a Pages deployment workflow with
first-party `actions/configure-pages` / `actions/upload-pages-artifact`
/ `actions/deploy-pages`. That was over-engineered for v1 dogfood:

- it required the repo owner to enable Pages
- it added a workflow + `policy/workflow_allowlist.txt` entry +
  Pages permissions surface
- it implied that downstream users should also enable Pages, which is
  not the long-term `ripr` user story

Checked-in JSON gives the same public-URL-on-`main` outcome with much
less machinery, and badge changes show up in PR diffs — which is
useful while the repo headline is still stabilizing.

#### What downstream users should do

If you want `RIPR gaps` and `RIPR+ gaps` badges in your own README today:

1. Run `ripr` in your CI.
2. Pick **any** stable public surface to serve the resulting Shields
   JSON: a committed `badges/` directory in your repo (the pattern
   this repo uses), GitHub Pages, an organization-level badge-host
   repo, a static asset bucket, a Gist, or a hosted host. None is
   required.
3. Point Shields at your URL via the
   `https://img.shields.io/endpoint?url=...` pattern.

A general-purpose hosted `ripr` badge service (so step 2 disappears) is
tracked as `deferred/hosted-badge-service`. Until that lands,
self-hosting is the v1 path.

## Implementation status

Tracked alongside Campaign 4A and Campaign 5B in
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
| `.ripr/suppressions.toml` loader | done | `suppressions/v1` |
| CI badge artifacts (diff-scoped, PR) | done | `ci/badge-artifacts` |
| Repo-scoped badge artifacts | done | `badge/repo-scope-artifacts` (`cargo xtask repo-badge-artifacts`) |
| Published Shields endpoint from `main` | done | `badge/publish-main-endpoint` (committed `badges/*.json` served via `raw.githubusercontent.com`; refresh with `cargo xtask update-badge-endpoints`) |
| Diff-scope `ripr+` related-tests filter | done | `badge/diff-ripr-plus-related-tests` |
| Seam-native repo badge mapping | done | `badge/seam-native-count-mapping` |

## See also

- [Static exposure model](STATIC_EXPOSURE_MODEL.md) — exposure classes and stage states.
- [Output schema](OUTPUT_SCHEMA.md) — stable JSON shape for `ripr check --json`.
- [Configuration](CONFIGURATION.md) — current vs planned config surfaces.
- [Implementation campaigns](IMPLEMENTATION_CAMPAIGNS.md) — Campaign 4A status.
- [Roadmap](ROADMAP.md) — long-range plan including badge work.
