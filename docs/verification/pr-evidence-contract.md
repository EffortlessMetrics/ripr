# PR Evidence Contract

PR evidence is diff-scoped. It tells reviewers and agents what changed, what
static evidence is weak, what focused test would improve confidence, and
whether expensive verification should run.

It is not a public badge, not release proof, and not a generic test-passed
blob.

## Canonical Questions

Every PR evidence packet should answer:

```text
What behavior moved?
What seams are newly weak or newly visible?
What focused test would improve confidence?
Do routing signals justify targeted mutation?
Where are the machine-readable receipts?
```

## Canonical Packet

The canonical PR evidence packet is:

```text
target/ripr/pr/repo-exposure.json
target/ripr/pr/repo-exposure.md
```

The JSON packet is validated by:

```text
schemas/ripr/pr-evidence.schema.json
```

It must include:

| Field | Contract |
| --- | --- |
| `schema_version` | PR evidence schema version. |
| `tool` | `ripr`. |
| `kind` | `pr_evidence`. |
| `scope` | `diff`. |
| `status` | `advisory`, `incomplete`, or `error`. |
| `root` | Workspace root used by the producer. |
| `base` / `head` | Git revisions used for the diff. |
| `summary` | Stable counts and routing booleans. |
| `artifacts[]` | Machine-readable and Markdown artifact paths. |
| `warnings[]` | Bounded warnings; never hidden in prose only. |
| `limits[]` | Advisory and non-proof boundaries. |

Missing data should be explicit. Use `0`, `false`, `null`, `not_available`,
`incomplete`, or a warning instead of omitting a field that agents need for
routing.

## Review Guidance

Line-placeable guidance is produced separately:

```text
target/ripr/review/comments.json
target/ripr/review/comments.md
```

`comments.json` is validated by:

```text
schemas/ripr/review-comments.schema.json
```

The JSON separates recommendations by placement and policy state:

| Collection | Contract |
| --- | --- |
| `comments[]` | Changed-line guidance that may become non-blocking annotations. |
| `summary_only[]` | Useful guidance that is not safe to attach to a changed line. |
| `suppressed[]` | Candidates suppressed by cap, policy, configured-off severity, nearby changed tests, or missing safe guidance. |
| `warnings[]` | Bounded producer warnings. |

Only `comments[]` can feed annotations. `summary_only[]` remains visible in
Markdown and job summaries, but never becomes an annotation or inline comment.

## Routing Fields

The PR evidence summary should keep these field names stable:

| Field | Meaning |
| --- | --- |
| `changed_files` | Number of changed files in the analyzed diff. |
| `comments` | Count of line-placeable review guidance items. |
| `summary_only` | Count of summary-only guidance items. |
| `suppressed` | Count of suppressed guidance items. |
| `weakly_exposed` | Count of weak static exposure signals. |
| `reachable_unrevealed` | Count of reachable but unrevealed signals. |
| `no_static_path` | Count of static path failures. |
| `severe_gaps` | Count of severe routing gaps under repo policy. |
| `requires_targeted_mutation` | Whether targeted mutation is routed for this PR. |

Targeted mutation routing should be justified by explicit evidence:

```text
mutation label
release-risk label
ripr severe gap
impacted evidence
full-owner mutation label
```

Full-owner mutation remains opt-in and should require an explicit label or
owner action.

## Check Mode

`cargo xtask ripr-pr --check` should fail for contract problems:

- missing expected output;
- invalid JSON;
- schema drift;
- bad base/head resolution;
- empty evidence when source changes require evidence.

It should not fail because RIPR found advisory gaps unless a later explicit
gate policy says those findings are blocking.

## Public Boundary

PR evidence must not be copied into `badges/`, README badge URLs, crate
metadata, release notes, or public support-tier claims. It can be linked from
CI summaries and uploaded as artifacts.
