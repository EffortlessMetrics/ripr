# Badge Adoption Guide

This guide describes external-adoption guidance for repo-scoped public `ripr`
README badges and the narrower, preconditioned `ripr+` badge.

## Hard rules

1. README badges must be **repo-scoped**, not PR/diff-scoped.
2. Badge JSON must be **generated**, not hand-authored.
3. External `ripr+` adoption must not depend on `ripr` repo-only `xtask`
   internals.
4. Do not publish a `ripr+` badge unless the downstream repo has an explicit
   supported path that writes `target/ripr/reports/test-efficiency.json`.

See [BADGE_POLICY.md](BADGE_POLICY.md) for the product contract and allowed
claims.

## Artifact model

For a plain `ripr` repo badge, generate and retain both artifacts:

- Native audit artifact:
  `target/ripr/reports/repo-ripr-badge.json`
- Public Shields endpoint:
  `badges/ripr.json`

For a `ripr+` repo badge, generate and retain both artifacts:

- Native audit artifact:
  `target/ripr/reports/repo-ripr-plus-badge.json`
- Public Shields endpoint:
  `badges/ripr-plus.json`

The native artifact must remain policy-rich (`kind`, `scope`, `basis`, plus
counts). The public endpoint must be a compact four-field Shields payload.

## Validation guardrails

Fail badge refresh when a non-repo badge leaks into public endpoints.

Validate native repo artifact:

```bash
jq -e '
  (.kind == "ripr" or .kind == "ripr_plus")
  and .scope == "repo"
  and .basis == "canonical_actionable_gap"
  and (.message | type == "string")
  and (.color | type == "string")
' target/ripr/reports/repo-ripr-badge.json
```

Validate public Shields artifact:

```bash
jq -e '
  .schemaVersion == 1
  and (.label == "ripr" or .label == "ripr+")
  and (.message | type == "string")
  and (.color | type == "string")
  and ((keys | sort) == ["color", "label", "message", "schemaVersion"])
' badges/ripr.json
```

## Current portability boundary for `ripr+`

`ripr+` formats read `target/ripr/reports/test-efficiency.json`.

If that file is missing, `badge-plus-*` and `repo-badge-plus-*` formats render a
neutral badge-generator-safe response instead of exiting nonzero:

```json
{
  "schemaVersion": 1,
  "label": "ripr+",
  "message": "needs test-efficiency",
  "color": "lightgrey"
}
```

Native JSON uses the same badge schema and includes the diagnostic in
`warnings[]`. The neutral badge is not a measured `ripr+` value; generate the
report before publishing or enforcing a `ripr+` count.

Today that report is typically produced by:

```bash
cargo xtask test-efficiency-report
```

That command is appropriate for this repository, but external repositories
should not be required to copy or vendor repo-private `xtask` internals.

### Productization target

Provide a public command contract that external repositories can call directly.
That contract does not exist in the current `ripr reports` CLI; the current
public report commands are `ripr reports index` and `ripr reports gap-ledger`.
A future portable command must write
`target/ripr/reports/test-efficiency.json` without requiring downstream repos to
copy this repository's `xtask` wrapper.

Until a public contract like this exists, recommend:

- external repos can adopt plain `ripr` badge flows now;
- external repos adopt `ripr+` only when they already have a supported
  `test-efficiency.json` generation path.

### Minimum `test-efficiency.json` shape

The current badge parser expects this top-level shape:

```json
{
  "schema_version": "0.1",
  "tests": [
    {
      "name": "test_name",
      "class": "smoke_only",
      "path": "tests/example.rs",
      "reached_owners": ["crate::module::owner"]
    }
  ],
  "metrics": {
    "tests_scanned": 1,
    "reason_counts": {
      "smoke_oracle_only": 1
    }
  }
}
```

Recognized `class` values and reason strings are defined in
[BADGE_POLICY.md](BADGE_POLICY.md#test-efficiency-vocabulary-locked).

## Recommended downstream command sequence

### Plain `ripr` badge

```bash
mkdir -p target/ripr/reports badges

ripr check \
  --root . \
  --mode ready \
  --format repo-badge-json \
  > target/ripr/reports/repo-ripr-badge.json

ripr check \
  --root . \
  --mode ready \
  --format repo-badge-shields \
  > badges/ripr.json

jq -e '
  .kind == "ripr"
  and .scope == "repo"
  and .basis == "canonical_actionable_gap"
' target/ripr/reports/repo-ripr-badge.json

jq -e '
  .schemaVersion == 1
  and .label == "ripr"
  and ((keys | sort) == ["color", "label", "message", "schemaVersion"])
' badges/ripr.json
```

### Conditional `ripr+` badge

First generate `target/ripr/reports/test-efficiency.json` through a supported
downstream mechanism. In this repository that source is
`cargo xtask test-efficiency-report`; external repositories should not copy that
private wrapper. Do not run the `ripr+` commands below until the JSON file
exists and is part of the downstream repo's badge refresh contract.

```bash
jq -e '.schema_version' target/ripr/reports/test-efficiency.json

ripr check \
  --root . \
  --mode ready \
  --format repo-badge-plus-json \
  > target/ripr/reports/repo-ripr-plus-badge.json

jq -e '
  .kind == "ripr_plus"
  and .scope == "repo"
  and .basis == "canonical_actionable_gap"
' target/ripr/reports/repo-ripr-plus-badge.json

ripr check \
  --root . \
  --mode ready \
  --format repo-badge-plus-shields \
  > badges/ripr-plus.json

jq -e '
  .schemaVersion == 1
  and .label == "ripr+"
  and ((keys | sort) == ["color", "label", "message", "schemaVersion"])
' badges/ripr-plus.json
```

## CI workflow shape

Prefer a **scheduled/manual badge refresh workflow** that opens a dedicated PR,
not silent endpoint mutation in unrelated product PRs.

Minimum properties:

- trigger: `workflow_dispatch` and schedule;
- pin released `ripr` version;
- generate native + Shields artifacts;
- for `ripr+`, generate or provide `test-efficiency.json` through an explicit
  downstream-supported path;
- validate `kind`/`scope`/`basis` and Shields schema;
- open a narrowly scoped badge refresh PR.

## README usage

Use repo-specific endpoint URLs, e.g.:

```md
[![ripr](https://img.shields.io/endpoint?url=https%3A%2F%2Fraw.githubusercontent.com%2FOWNER%2FREPO%2Fmain%2Fbadges%2Fripr.json)](https://github.com/EffortlessMetrics/ripr/blob/main/docs/BADGE_POLICY.md)
[![ripr+](https://img.shields.io/endpoint?url=https%3A%2F%2Fraw.githubusercontent.com%2FOWNER%2FREPO%2Fmain%2Fbadges%2Fripr-plus.json)](https://github.com/EffortlessMetrics/ripr/blob/main/docs/BADGE_POLICY.md)
```

## Allowed vs forbidden badge claims

Allowed wording should stay in static-evidence language (repair gaps,
actionable findings, receipt model).

Forbidden wording includes claims like:

- 100% tested
- mutation clean
- all mutants killed
- full coverage
- no bugs
- complete test adequacy

## Adoption roadmap

1. Productize portable test-efficiency report generation.
2. Add badge endpoint verification UX (`ripr badge verify` or equivalent).
3. Add generated CI template support for badge refresh.
4. Keep this guide synchronized with policy and output schema.
