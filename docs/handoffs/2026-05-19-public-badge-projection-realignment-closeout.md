# Handoff: Public Badge Projection Realignment Closeout

Date: 2026-05-19

Latest merged PR: #1325 `badge: refresh public endpoints`
(commit `d1892a67cae3afa66c9aab099f2b635f6ddab281`)

## Current Work Item

`campaign/public-badge-projection-realignment`

The public README / crate / store badge path now follows the same unit as the
local repair loop:

```text
canonical gap
-> repair route
-> related test / repair target
-> verify command
-> receipt path
-> advisory boundary
```

The public badge is no longer a seam-native inventory pressure gauge. Seam
inventory remains available as internal evidence-quality and static-limitation
pressure, but the headline badge now projects unresolved actionable canonical
repair items.

## Before And After

| Field | Before | After |
| --- | --- | --- |
| `ripr` public count | 24352 | 180 |
| `ripr+` public count | 24469 | 180 |
| Public basis | `seam_native` repo inventory | `canonical_actionable_gap` |
| Public meaning | classified repo seam pressure | unresolved actionable static repair gaps |
| Internal seam inventory | headline badge | internal report only |

Current endpoint files:

```text
badges/ripr.json       -> message 180
badges/ripr-plus.json  -> message 180
```

## PR Chain

- #1285 `badge: add public badge basis audit`
- #1296 `docs(badge): define actionable public badge basis`
- #1311 `badge: project public counts from actionable canonical gaps`
- #1289 `reports: expose seam-native inventory separately` closed from the
  badge-basis and repo-exposure report proof already present after #1311
- #1312 `badge: guard public badge basis`
- #1314 `badge: refresh public endpoints`
- #1315 `docs: align badge wording with actionable gaps`
- #1322 `docs(spec): lock public actionable projection`
- #1324 `fix(output): bump evidence health schema for row counts`
- #1325 `badge: refresh public endpoints`

## Proof Artifacts

Current generated audit:

```text
target/ripr/reports/badge-basis.md
target/ripr/reports/badge-basis.json
```

The current `cargo xtask badge-basis` run reports:

```text
Status: pass
badges/ripr.json:      180
badges/ripr-plus.json: 180
repo badge basis:      canonical_actionable_gap
ripr count:            180
ripr+ count:           180
```

Internal seam inventory remains available through:

```bash
cargo xtask badge-basis --include-seam-classes
cargo run -p ripr -- check --root . --format repo-exposure-json
cargo run -p ripr -- check --root . --format repo-exposure-md
```

The normal badge-basis report intentionally leaves the seam-native class
breakdown uncollected unless `--include-seam-classes` is requested. That keeps
routine badge closeouts focused on public projection while preserving the
internal pressure gauge on demand.

## Generator And Guards

Endpoint generator:

```bash
cargo xtask badges
```

The explicit endpoint refresh helper remains:

```bash
cargo xtask update-badge-endpoints
```

Endpoint and basis guards:

```bash
cargo xtask check-badge-endpoints
cargo xtask check-badge-diff-policy
cargo xtask check-generated-clean
```

The guard now rejects public README / crate / store badge surfaces that use
`seam_native` as a public repair badge basis unless the surface explicitly
labels the badge as seam inventory.

## Public Docs Updated

- `README.md`
- `crates/ripr/README.md`
- `docs/BADGE_POLICY.md`
- `docs/STATIC_EXPOSURE_MODEL.md`

Public copy now says:

- `ripr` counts unresolved actionable static repair gaps.
- `ripr+` adds only actionable test-efficiency repairs projected into the same
  repair / verify / receipt model.
- The badge is not coverage, mutation adequacy, all behavior seams, all
  untested code, merge approval, or runtime proof.

## Closeout Validation

Closeout validation for this PR:

```bash
cargo xtask badge-basis
cargo xtask check-badge-endpoints
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-pr
git diff --check
```

Result: pass before PR creation on 2026-05-19.

The preceding PRs also passed their scoped gates:

```bash
cargo xtask update-badge-endpoints
cargo xtask check-badge-endpoints
cargo xtask check-badge-diff-policy
cargo xtask check-generated-clean
cargo xtask check-readme-state
cargo xtask check-product-copy
cargo xtask check-positioning-language
cargo xtask check-static-language
cargo xtask markdown-links
cargo xtask check-pr
git diff --check
```

## Remaining Limits

- The count is static, advisory repair evidence.
- It is not runtime mutation confirmation.
- It is not coverage.
- It is not merge approval or policy gate authority.
- It is not full test adequacy.
- It is not a complete seam inventory.
- It does not claim every behavior seam is tested.
- It does not claim preview-language promotion.
- Internal seam-native inventory is still useful, but it belongs in detailed
  reports, not the public `ripr` / `ripr+` headline.

## What Not To Do

- Do not hand-edit badge endpoint values.
- Do not refresh `badges/*.json` from an intermediate moving base.
- Do not make seam-native inventory the public headline unless the badge is
  explicitly relabeled as seam inventory.
- Do not promote preview evidence, add policy authority, run mutation testing,
  publish PR comments, change editor behavior, create generated tests, call
  providers, edit source, tag, or publish from this lane.

## Recommended Next Action

No further behavior-bearing work is required for the public badge projection
realignment lane. Future badge refreshes should be generated-only endpoint PRs
after the code queue settles.
