# Badge Policy

`ripr` badges are **repair counters**, not coverage scores.

They answer one bounded question:

```text
How many unresolved, policy-eligible static repair items are visible in this
selected scope?
```

Two scopes use the same labels but have different subjects and audiences:

- **diff scope** is a pull-request or local artifact derived from one selected
  diff; the repository-owned producer binds that diff to a governed Git range;
- **repo scope** is the public README/store signal derived from the repository
  baseline.

Do not publish a diff-scoped result as a repository badge. An empty PR diff
means “nothing changed in this range,” not “the repository has no gaps.”

This policy pairs with [RIPR-SPEC-0056](specs/RIPR-SPEC-0056-public-actionable-projection.md),
[Static exposure model](STATIC_EXPOSURE_MODEL.md),
[Output schema](OUTPUT_SCHEMA.md),
[Badge adoption](BADGE_ADOPTION.md), and
[Configuration](CONFIGURATION.md).

## What each badge means

### `ripr`

A measured `ripr` badge counts unresolved static repair items under the selected
scope and basis.

- In **diff scope**, it counts selected exposure-class findings in the supplied
  change surface.
- In **repo scope**, it counts unresolved canonical actionable gaps eligible for
  public projection.

### `ripr+`

A measured `ripr+` badge starts with the same `ripr` count and adds actionable
test-efficiency repairs that have entered the same repair, verification, and
receipt model.

`ripr+` is not measured when its test-efficiency input is missing. In that case
the renderer emits the neutral `needs test-efficiency` state; that state must
not be published or enforced as a real count.

## Scope: diff vs repo

| Scope | Primary producers | Subject | Basis | Use |
| --- | --- | --- | --- | --- |
| `diff` | `ripr check --format badge-*`; `cargo xtask badge-artifacts` | A selected diff; the repository wrapper uses one resolved base-to-head range | `finding_exposure` | PR summary and retained CI artifacts |
| `repo` | `ripr check --format repo-badge-*`; `cargo xtask repo-badge-artifacts` | Full repository baseline | `canonical_actionable_gap` or explicit `gap_decision_ledger` | README, crate, marketplace, or store endpoint |

### The repository diff producer uses one governed Git subject

`cargo xtask badge-artifacts` does not hardcode `origin/main` and does not
assemble an ambient `git diff` command.

It uses the same authorities as RIPR analysis:

```text
RIPR-SPEC-0084 default-base resolution
+ ripr::analysis::load_diff_range
= one resolved base commit, exact HEAD, and pinned diff byte stream
```

The pinned presentation currently includes:

```text
-c core.quotePath=true
--no-ext-diff
--no-textconv
--no-color
--unified=0
--inter-hunk-context=0
--submodule=short
```

The producer retains:

```text
resolved base ref and commit
HEAD commit and tree
diff byte count and SHA-256
presentation authority and argv contract
generator binary identity when available
badge output identities
```

The retained files are:

```text
target/ripr/badge-input.diff
target/ripr/reports/badge-artifacts-identity.json
```

Base-resolution and Git failures are named failures. They must not be converted
into a clean-looking zero badge.

For ordinary CLI use, omit `--base` to use the shared default-base authority or
supply the repository’s real base explicitly:

```bash
ripr check --format badge-json
ripr check --base <base-ref> --format badge-json
```

Direct CLI use may instead select an explicit `--diff` or immutable candidate
tree. Those inputs remain diff-scoped, but they do not acquire the
`badge-artifacts` base/head identity receipt merely by selecting a badge format;
the caller must preserve the explicit subject and its provenance.

`origin/main` is one possible repository ref, not the badge contract.

### Repo scope does not use a diff

Repo badge formats analyze the repository baseline. They do not consult a
base-to-head range and must not inherit PR-local emptiness.

Only repo-scoped native artifacts may feed public endpoints:

```text
target/ripr/reports/repo-ripr-badge.json
target/ripr/reports/repo-ripr-plus-badge.json
```

The public Shields projections are:

```text
badges/ripr.json
badges/ripr-plus.json
```

## Basis vocabulary

Every native badge names the basis used to compute its count.

| Basis | Scope | Public headline? | Meaning |
| --- | --- | :---: | --- |
| `canonical_actionable_gap` | repo | yes | Unresolved canonical repair items with an actionable route, safe verification command, receipt path, and public-projection eligibility. |
| `finding_exposure` | diff | no | PR-local `Finding` / `ExposureClass` aggregation from the selected change surface. |
| `seam_native` | repo inventory | no | Internal seam inventory and static-limitation pressure. It is broader than the public repair queue. |
| `gap_decision_ledger` | repo projection | explicit bridge | Policy-selected `GapRecord` projection targets supplied by release or repository tooling. |

A seam inventory must not reuse the public `ripr` / `ripr+` headline unless the
surface is explicitly relabeled as inventory.

## Counting rules

### Diff-scoped `ripr`

The headline counts unsuppressed findings whose `exposure_class` is one of:

```text
weakly_exposed
reachable_unrevealed
no_static_path
```

These remain separate from unknown classifications:

```text
infection_unknown
propagation_unknown
static_unknown
```

Unknowns describe where static analysis stopped. They remain visible in native
artifacts but do not enter the default headline.

### Repo-scoped public `ripr`

A canonical item counts only when all of these are true:

```text
gap_state = unresolved
actionability = actionable
repair route exists
safe verification command exists
receipt path exists
not suppressed
not intentional
eligible for public projection
```

A limited, stale, unknown, missing, malformed, or wrong-subject source must not
resolve toward the cleaner-looking zero state.

### `ripr+`

`ripr+` adds supported test-efficiency entries whose class is:

```text
likely_vacuous
possibly_circular
smoke_only
duplicative
```

Declared intent and matching suppressions keep the original analyzer class
visible while removing the item from the actionable headline.

Diff-scoped `ripr+` includes only entries related to the changed owners or tests.
Repo-scoped `ripr+` includes only entries projected into the same actionable
repair model as the repo `ripr` count.

## Test-efficiency vocabulary (locked)

The recognized class and reason strings are a wire contract. Update this section
when the producer changes them.

### Class values

| `class` | Counts in measured `ripr+`? | Meaning |
| --- | :---: | --- |
| `strong_discriminator` | no | Strong check with no demoting condition. |
| `useful_but_broad` | no by default | A meaningful but broad check. |
| `smoke_only` | yes, unless intentional or suppressed | Smoke-strength check such as startup, `is_ok`, `unwrap`, or `expect`. |
| `likely_vacuous` | yes, unless intentional or suppressed | No detected assertion. |
| `possibly_circular` | yes, unless intentional or suppressed | Expected value is computed through the detected owner path. |
| `duplicative` | yes, unless intentional or suppressed | Duplicate owner, activation, and oracle shape within the supported grouping contract. |
| `opaque` | no | Static analysis could not resolve the reached owner. |

### Reason strings

| Reason | Meaning |
| --- | --- |
| `no_assertion_detected` | No detected assertion; supports `likely_vacuous`. |
| `smoke_oracle_only` | Only a smoke-strength oracle was found. |
| `relational_oracle` | A relational assertion was found. |
| `broad_oracle` | A broad, non-exact assertion was found. |
| `assertion_may_not_match_detected_owner` | The assertion may observe something other than the detected owner. |
| `opaque_helper_or_fixture_boundary` | Helper or fixture structure prevented owner resolution. |
| `no_activation_literal_detected` | No supported activation literal was found. |
| `expected_value_computed_from_detected_owner_path` | The expected side reuses the detected owner path. |
| `duplicate_activation_and_oracle_shape` | Another test shares the supported owner, activation, and oracle signature. |

### Intent and suppressions

Declared intent is additive metadata, not a replacement class. A declared smoke
test remains `smoke_only`; the declaration explains why it is intentional and
removes it from the actionable count.

Durable exceptions belong in `.ripr/suppressions.toml` with an owner and reason.
Expired or malformed suppressions must not silently hide debt.

## JSON wire shape

Native badge JSON uses schema version `0.8`. It is the audit artifact and source
of truth. The Shields response is a four-field projection.

A representative native repo badge contains the full count, reason, policy, and
public-projection fields:

```json
{
  "schema_version": "0.8",
  "kind": "ripr",
  "scope": "repo",
  "basis": "canonical_actionable_gap",
  "label": "ripr",
  "message": "0 actionable",
  "status": "pass",
  "color": "brightgreen",
  "analysis_complete": null,
  "analysis_outcome": null,
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
    "analyzed_gap_records": 0,
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
  },
  "warnings": [],
  "preview_skipped": [],
  "public_projection": {
    "state": "zero_actionable",
    "message": "0 actionable",
    "run_status": "full",
    "generated_at": "2026-06-20T00:00:00Z",
    "actionable_count": 0,
    "limited_reason": null,
    "stale_age_secs": 0,
    "source_report": "target/ripr/reports/repo-ripr-badge.json"
  }
}
```

`kind` is `ripr` or `ripr_plus`. Diff-scoped badges use the same native schema,
carry their typed analysis outcome and completeness state, and omit the
repo-only public projection.

The closed public-projection states are:

```text
zero_actionable
actionable
limited
stale
unknown
```

Degraded-state precedence is:

```text
unknown > stale > limited > count
```

The projection does not carry a reassuring count beside a degraded state.

### Shields projection

The public endpoint contains exactly four fields:

```json
{
  "schemaVersion": 1,
  "label": "ripr",
  "message": "0 actionable",
  "color": "brightgreen"
}
```

Scope, basis, counts, warnings, limitations, and retained identity stay in the
native artifact.

## Preview-language honesty

Diff-scoped native JSON carries `preview_skipped` when a preview-language file
was detected but its adapter was not enabled.

A non-empty value is not a clean Rust-grade result. When the count would
otherwise be zero, the badge is downgraded to a warning state such as:

```text
preview-skipped: typescript
```

A missing preview adapter, absent packet, or limited downstream route must not
be interpreted as evidence that the change is safe.

## Colors and status

The ordinary count thresholds are:

| Count | Status | Color |
| ---: | --- | --- |
| `0` | `pass` | `brightgreen` |
| `1–3` | `warn` | `yellow` |
| `4+` | `warn` | `orange` |
| nonzero with `--fail-on-nonzero` | `fail` | `red` |

Limited, stale, unknown, preview-skipped, or missing-input states override these
count thresholds. Badge status and CI exit policy remain separate decisions.

## CLI shape

Badges are renderings of `ripr check`, not a separate analyzer.

### Diff artifacts

```bash
ripr check --format badge-json
ripr check --format badge-shields

ripr check --base <base-ref> --format badge-plus-json
ripr check --base <base-ref> --format badge-plus-shields
```

### Repo artifacts

```bash
ripr check --root . --mode ready --format repo-badge-json
ripr check --root . --mode ready --format repo-badge-shields

ripr check --root . --mode ready --format repo-badge-plus-json
ripr check --root . --mode ready --format repo-badge-plus-shields
```

When a gap decision ledger is the explicit projection authority, pass it rather
than recalculating from another basis:

```bash
ripr check \
  --root . \
  --mode ready \
  --format repo-badge-json \
  --gap-ledger target/ripr/reports/gap-decision-ledger.json
```

The `*-plus-*` formats require
`target/ripr/reports/test-efficiency.json`. See
[Badge adoption](BADGE_ADOPTION.md) for the downstream portability boundary.

### Repository wrappers

This repository provides wrappers for its own CI and endpoint maintenance:

```bash
cargo xtask badge-artifacts
cargo xtask repo-badge-artifacts
cargo xtask badge-basis
cargo xtask badges
cargo xtask badges --check
cargo xtask check-badge-diff-policy
cargo xtask check-badge-endpoints
```

`badge-artifacts` is diff-scoped. `repo-badge-artifacts` and `badges` are
repo-scoped. Do not substitute one for the other.

## CI policy

### Pull requests

PR workflows may generate diff-scoped native and Shields artifacts for the job
summary and artifact upload. They are advisory unless the repository separately
adopts a gate policy.

Do not link these artifacts from a README or store listing.

### Public endpoints

Public endpoints must be generated from current repo-scoped native artifacts.
Ordinary product PRs should not hand-edit `badges/*.json` or carry unrelated
endpoint refreshes.

Use the generated badge refresh route and validate:

```bash
cargo xtask badges
cargo xtask badges --check
```

For a policy-backed endpoint refresh, provide the same explicit ledger to both
commands:

```bash
cargo xtask badges \
  --gap-ledger target/ripr/reports/gap-decision-ledger.json
cargo xtask badges --check \
  --gap-ledger target/ripr/reports/gap-decision-ledger.json
```

`cargo xtask check-badge-diff-policy` enforces the endpoint ownership boundary.
`cargo xtask check-badge-endpoints` compares committed endpoints with a fresh
repo-scoped generation; it is an explicit freshness check rather than a default
requirement for every product PR.

A public badge refresh still does not authorize release publication or establish
runtime mutation results.

## Self-hosted dogfood endpoint

The published first-party endpoint lives on the source repository’s `main`
branch:

```text
EffortlessMetrics/ripr
  badges/ripr.json
  badges/ripr-plus.json
```

The public README resolves those payloads through:

```text
https://raw.githubusercontent.com/EffortlessMetrics/ripr/main/badges/ripr.json
https://raw.githubusercontent.com/EffortlessMetrics/ripr/main/badges/ripr-plus.json
```

Only those source-repository endpoint projections are public badge authority.
Equivalent files and generation runs in `ripr-swarm` are development rehearsal
and evidence until they move through the governed source-integration path.
Native reports, Markdown summaries, diff-scoped artifacts, and `target/`
snapshots remain review evidence rather than public endpoints.

The checked-in files are generated Shields projections, not audit artifacts or
hand-authored status copy.

### Why checked-in JSON, not GitHub Pages

The earlier Pages design required repository settings, deployment permissions,
and additional workflow machinery while suggesting that downstream users also
needed Pages.

Checked-in JSON provides the same stable-public-URL property with less machinery
and leaves endpoint changes visible in review. Hosting remains replaceable: a
downstream repository may use checked-in JSON, Pages, an asset bucket, an
organization badge host, a Gist, or a future hosted RIPR service.

## What neither badge proves

A green badge does not establish:

- full test coverage;
- correctness or absence of bugs;
- that every behavior was analyzed;
- that every mutant would fail;
- runtime mutation adequacy;
- merge approval; or
- release qualification.

It means only that the selected, current, non-degraded static evidence produced
no counted actionable items under the named scope, basis, and policy.

## Why there is no denominator

A label such as `0/2300` reads like a coverage fraction. RIPR does not measure
coverage completeness, so the public badge is an inbox-zero counter:

```text
ripr 0 actionable
ripr+ 0 actionable
```

Detailed denominators, unknowns, suppressed items, intent, analyzed counts, and
limitations belong in the native artifact and reports.

## Validation

For a diff-scoped change, retain and review:

```bash
cargo xtask badge-artifacts
cargo xtask check-badge-diff-policy
```

For repo-scoped endpoint work, retain and review:

```bash
cargo xtask repo-badge-artifacts
cargo xtask badge-basis
cargo xtask badges --check
cargo xtask check-badge-endpoints
```

Before integration, also run the repository’s normal documentation and PR gate
suite. A green command is evidence for that exact head and subject only.

## See also

- [Badge adoption](BADGE_ADOPTION.md) — downstream generation and validation.
- [Static exposure model](STATIC_EXPOSURE_MODEL.md) — exposure classes and stage states.
- [Output schema](OUTPUT_SCHEMA.md) — machine-readable output contracts.
- [Configuration](CONFIGURATION.md) — intent, suppressions, modes, and limits.
- [Verification](VERIFICATION.md) — evidence and non-claim boundaries.
- [Deferred work](DEFERRED.md) — hosted badge service and other non-current surfaces.
