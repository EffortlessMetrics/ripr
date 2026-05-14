# Badge Contract

Badges are public, repo-scoped trust markers. They summarize the current
repository baseline through generated Shields endpoint JSON.

They do not route PR work. They do not summarize a diff. They do not claim
coverage, runtime mutation confirmation, correctness, or release readiness.

## Public Endpoint Files

Committed badge endpoint files live under:

```text
badges/*.json
```

Only committed Shields endpoint JSON belongs in `badges/`. Detailed reports,
native RIPR JSON, PR evidence, Markdown summaries, and release evidence stay in
`target/` artifacts or docs designed for those surfaces.

The committed endpoint shape is validated by:

```text
schemas/badges/shields-endpoint.schema.json
```

## Shields Endpoint Shape

Every public badge endpoint uses exactly this JSON shape:

```json
{
  "schemaVersion": 1,
  "label": "ripr+",
  "message": "0",
  "color": "brightgreen"
}
```

Fields:

| Field | Contract |
| --- | --- |
| `schemaVersion` | Required. Always `1` for Shields endpoint JSON. |
| `label` | Required short public label, such as `ripr` or `ripr+`. |
| `message` | Required short public value, usually a count or bounded status. |
| `color` | Required Shields color string. |

No extra top-level fields are allowed in committed public endpoint files.
Scope, basis, policy, counts, and warning detail belong in native RIPR reports
under `target/`, not in the public endpoint.

## Repo Scope Only

Public badge endpoints are repo-scoped. They are generated from repository
truth for the current `main` baseline, not from a pull-request diff.

Allowed:

```text
cargo xtask badges
cargo xtask badges --check
```

Not allowed:

```text
copy target/ripr/pr/*.json into badges/
copy target/ripr/review/*.json into badges/
publish diff-scoped RIPR output as README badge state
```

Diff-scoped evidence can report zero because the diff is empty. That is not a
repo-clean signal.

## Claim Limits

A public RIPR badge may claim:

```text
The generated endpoint reports the current repo-scoped RIPR badge state.
```

It must not claim:

- complete test adequacy;
- runtime mutation success;
- high coverage;
- absence of bugs;
- merge readiness for a PR;
- release readiness for a version.

If a repository has a repo-specific badge, such as `scanner-safe`, that badge
must be generated from a named local proof command and fail closed when the
proof is missing or invalid.

## Drift Handling

`cargo xtask badges --check` should compare committed `badges/*.json` against a
freshly generated repo-scoped endpoint in `target/`. It should fail only for
contract or drift problems, not because RIPR found advisory gaps.

Recommended behavior:

- write comparison outputs under `target/`;
- print the exact command that refreshes endpoints;
- leave PR-scoped evidence out of the comparison;
- keep endpoint changes reviewable in ordinary diffs.
