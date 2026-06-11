# RIPR-SPEC-0066: Repo Badge Use Case

Status: proposed

Owner: product / swarm

Created: 2026-06-06

Linked proposal:

- None yet

Linked ADRs:

- None yet

Linked plan:

- plans/use-case-specs/implementation-plan.md (planned)

Linked issues:

- None yet

Linked PRs:

- None yet

Support-tier impact:

- None. This spec defines the user-facing contract for the existing
  public repo badge surface. It promotes no language, surface, or
  evidence class. Preview-language evidence (TypeScript/Bun, Perl)
  remains excluded from public badge contribution per the existing
  preview evidence boundary in `docs/BLOCKING_READINESS.md`.
- Claim boundaries for this surface are governed by the canonical ledger in [support tiers](../status/SUPPORT_TIERS.md); nothing here promotes a tier.

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`.
- No new crates, binaries, dependencies, parsers, runtime executors,
  or files beyond the spec itself. The contract binds the existing
  badge model, xtask commands, and badge-endpoints workflow.

## Problem

This is use case UC1 of the evidence-to-repair roadmap
(RIPR-SPEC-0065). A repo owner or visitor reads the README badge and
asks one question:

```text
Is this repo currently clean, actionable, limited, or stale?
```

The mechanism already exists. `crates/ripr/src/output/badge/model.rs`
defines `BadgeKind` (`Ripr`, `RiprPlus`), `BadgeStatus` (`Pass`,
`Warn`, `Fail`), `BadgeBasis` (`FindingExposure`,
`CanonicalActionableGap`, `GapDecisionLedger`), `BadgeScope` (`Diff`,
`Repo`), and `BADGE_SCHEMA_VERSION = "0.5"`. The `cargo xtask badges`,
`cargo xtask badge-basis`, and `cargo xtask repo-badge-artifacts`
commands generate badge artifacts, and the `badge-endpoints` workflow
refreshes the committed Shields endpoints `badges/ripr.json` and
`badges/ripr-plus.json` via an automation PR.

What is missing is a written user-facing contract: which internal
states map to which badge message, what the badge must never claim,
and which degraded states must fail closed to `limited`, `stale`, or
`unknown` instead of rendering a number that looks clean. Without that
contract, a stale endpoint or a raw-finding count can silently present
partial evidence as a current, complete answer — the exact
overclaiming failure RIPR-SPEC-0065 names.

## Behavior

The user should be able to answer:

```text
Is this repo currently clean, actionable, limited, or stale —
and is the number I am looking at current and complete?
```

### User-facing badge states (closed vocabulary)

The public badge message renders exactly one of:

| Badge message | Meaning |
| --- | --- |
| `ripr: 0 actionable` | A full, current repo-scoped run found zero unresolved canonical actionable gaps. |
| `ripr: N actionable` | A full, current repo-scoped run found `N` unresolved canonical actionable gaps. |
| `ripr: limited` | The source report exists but its `run_status` is a `limited_*` state; counts are not safe to publish. |
| `ripr: stale` | The source report or committed endpoint exceeds the configured maximum age relative to its source; the last count is no longer claimed as current. |
| `ripr: unknown` | No consumable source report exists, or the only available basis is raw findings; no count is claimed. |

No other message is permitted on the public badge surface. New states
require a spec revision.

This closed vocabulary and the state mapping table below are scoped
to `BadgeKind::Ripr` (label `ripr`). The `ripr+` endpoint
(`badges/ripr-plus.json`, `BadgeKind::RiprPlus`) inherits the
identical state mapping with the `ripr+` label and its
test-efficiency-inclusive count semantics.

The committed endpoints today render the bare count (message `191`
with label `ripr`); the `N actionable` message format is part of the
badge projection implementation slice (see Implementation Mapping),
not a description of the current endpoints.

### Source of truth

The badge is a projection of canonical actionability plus runtime
completeness. Concretely:

- Scope must be `BadgeScope::Repo`. Diff-scoped badges feed PR step
  summaries only; a no-diff `main` run of the diff-scoped path always
  reports `0` regardless of the repo's actual exposure profile, so
  diff scope is never a public README endpoint.
- Basis must be `BadgeBasis::CanonicalActionableGap` or
  `BadgeBasis::GapDecisionLedger`. The public badge MUST NOT count raw
  findings: `BadgeBasis::FindingExposure` is a legacy/internal basis
  and is not a permitted public badge basis.
- Completeness comes from the Lane 1 `run_status` contract in
  `docs/OUTPUT_SCHEMA.md`: `full` versus `limited_timeout`,
  `limited_runner_failure`, `limited_large_cache_skip`,
  `limited_incomplete_input`, `limited_sampled_input`, and
  `limited_stale_input`.

### Required fields

The badge artifact (the machine-readable sidecar behind the Shields
endpoint, versioned under `BADGE_SCHEMA_VERSION`) must carry (planned
sidecar contract; see Implementation Mapping):

| Field | Meaning |
| --- | --- |
| `run_status` | Lane 1 completeness state of the source run (`full` or a named `limited_*` value). |
| `generated_at` | When the badge artifact was generated, for staleness evaluation. |
| `actionable_count` | Unresolved canonical actionable gap count; present only when the state is `0 actionable` or `N actionable`. |
| `limited_reason` | The `limitation_category` (and `repair_route` when present) explaining a `limited` state; `null` for full runs. |
| `stale_age` | Age of the artifact relative to its source at evaluation time, used against the configured maximum. |
| `source_report` | Repo-relative path of the report the badge was projected from (gap decision ledger or canonical actionable gap report). |

None of these six fields exist in the v0.5 native badge JSON emitted
by `crates/ripr/src/output/badge/render.rs` today; this table is the
implementation slice's target contract. Emitting these fields is a
public contract change that bumps `BADGE_SCHEMA_VERSION` past `0.5`.
Field additions or semantic changes thereafter likewise bump
`BADGE_SCHEMA_VERSION` and are public contract changes.

### State mapping rules (fail closed)

| Condition | Badge state |
| --- | --- |
| No source report, unreadable report, or schema mismatch | `ripr: unknown` |
| Source report `run_status` is any `limited_*` value | `ripr: limited` |
| Source report or endpoint older than the configured maximum age | `ripr: stale` |
| Only raw-finding (`finding_exposure`) data is available | `ripr: unknown` — never an actionable count |
| Full, current run; canonical basis; zero unresolved gaps | `ripr: 0 actionable` |
| Full, current run; canonical basis; `N > 0` unresolved gaps | `ripr: N actionable` |

Precedence when multiple conditions hold: `unknown` over `stale` over
`limited` over any count. A degraded input never resolves toward the
cleaner-looking state.

No maximum-age configuration exists in the repo today: the max-age
knob, age-based staleness evaluation, and the `ripr: stale` rendering
are new contract delivered by the badge projection slice. References
to "the configured maximum age" in this spec name that planned knob.

### Required versus forbidden wording for the clean state

Required meaning of the clean badge state:

```text
ripr: 0 actionable
= zero unresolved canonical actionable gaps
  under a full, current, repo-scoped run.
```

Forbidden renderings:

- Presenting a `limited`, `stale`, or `unknown` input as
  `ripr: 0 actionable` or any numeric count.
- Rendering `0` from a diff-scoped run on the public badge.
- Rendering a count whose basis is raw finding exposure.
- Wording that implies the test suite is complete in the runtime
  sense, that mutation outcomes were observed, or that anything
  beyond canonical actionable gap state was established.

The badge README (`badges/README.md`) and any hover/link text must
use the same vocabulary.

## Non-Goals

Explicit non-claims — the badge does not and must not claim:

- It does not claim complete oracle coverage; it reports canonical
  actionable gap state under the conservative static vocabulary only.
- No mutation success claim. RIPR does not run mutants; the badge
  carries no runtime mutation evidence in either direction.
- No cross-language certainty. Preview TypeScript/Bun and Perl
  evidence is excluded from the public badge count per the preview
  evidence boundary; the badge speaks only for the supported stable
  evidence class.
- No coverage-dashboard semantics: the badge is not a line/branch
  coverage number and must not be read as one.
- No new badge endpoints, no new render pipeline, no badge-driven
  blocking. The badge remains an advisory projection; gate posture is
  RIPR-SPEC-0067's concern.
- No analyzer behavior changes in this spec; contract only.

## Required Evidence

- This spec registered in `docs/specs/README.md` and
  `policy/doc-artifacts.toml`.
- The badge projection implementation slice (see Implementation
  Mapping) cites this spec and lands with fixtures pinning every row
  of the state mapping table.
- `cargo xtask badges --check` (the committed-endpoint drift check)
  continues to fail on content drift — when `badges/*.json` no longer
  matches what regeneration from its source produces — and gains
  coverage for the `limited`/`stale`/`unknown` renderings once those
  states are emitted. It does not evaluate endpoint age today;
  age-based staleness arrives with the badge projection slice.
- Proof commands that exist today: `cargo xtask badges`,
  `cargo xtask badges --check`, `cargo xtask badge-basis`,
  `cargo xtask repo-badge-artifacts`, plus the docs-only gates
  (`cargo xtask check-doc-artifacts`, `check-doc-index`,
  `check-static-language`, `markdown-links`).

### Verifier reject-list

A verifier for this surface must REFUSE to render the following as a
clean or numeric success state:

- basis `finding_exposure` (raw findings) on the public badge;
- scope `diff` on the public badge;
- `run_status` equal to any `limited_*` value;
- missing, unreadable, or schema-incompatible `source_report`;
- missing `generated_at`, or `stale_age` over the configured maximum;
- an `actionable_count` present alongside a `limited`, `stale`, or
  `unknown` state;
- preview-language (TypeScript/Bun, Perl) counts folded into the
  public number;
- a sampled or partial run presented with `run_status = "full"`.

Each rejection produces the corresponding fail-closed badge state
(`unknown`, `limited`, or `stale`) plus a named reason in the badge
artifact — never a silent green.

## Acceptance Examples

### Clean repo, full current run

- Source: gap decision ledger, `run_status = "full"`, zero unresolved
  canonical actionable gaps, artifact within max age.
- Badge: `ripr: 0 actionable`, status `pass`.
- Sidecar: `actionable_count = 0`, `limited_reason = null`,
  `source_report` set.

### Actionable repo

- Source: same as above with 191 unresolved canonical gaps (the
  current committed endpoints render message `191`).
- Badge: `ripr: 191 actionable`, status `warn` or `fail` per policy.

### Limited run

- Source: report with `run_status = "limited_timeout"` and
  `limitation_category = "lane1_repo_exposure_timeout"`.
- Badge: `ripr: limited`. `limited_reason` carries the category and
  repair route. No count is rendered.

### Stale endpoint

- Source: committed `badges/ripr.json` older than the configured
  maximum age (the planned max-age knob) relative to its source
  report.
- Badge: `ripr: stale`. The previous count is not re-claimed. This
  rendering is new contract from the badge projection slice.
- Existing behavior, preserved as-is: `cargo xtask badges --check`
  fails on content drift (the committed endpoint no longer matches
  regeneration from its source) with the existing stale-endpoint
  message directing to a refresh. An aged endpoint whose content is
  unchanged passes `badges --check` today; age-based failure is part
  of the planned slice.

### Missing report

- Source: no gap decision ledger and no canonical actionable gap
  report; only raw findings exist.
- Badge: `ripr: unknown`. Never a count derived from raw findings.

### Skipped refresh (existing workflow behavior)

- The `badge-endpoints` workflow's refresh step does not succeed.
- Behavior today, kept by this contract: the job summary states that
  no public badge endpoint was refreshed and no public badge count is
  claimed from that run; no endpoint PR opens.

## Test Mapping

- None yet. Test mappings land with the badge projection
  implementation slice; traceability entries are added only when
  behavior and tests exist.

## Implementation Mapping

- docs/specs/RIPR-SPEC-0066-repo-badge-use-case.md — this document.
- plans/use-case-specs/implementation-plan.md (planned) — the "badge
  projection" slice: emit the `limited` / `stale` / `unknown` public
  states, the `N actionable` count-message format (replacing today's
  bare-count message), and the required sidecar fields (bumping
  `BADGE_SCHEMA_VERSION` past `0.5`) from the existing
  `cargo xtask badges` / `repo-badge-artifacts` pipeline; introduce
  the maximum-age configuration knob and age-based staleness
  evaluation; with fixtures for every state-mapping row and
  reject-list entry.
- Existing surfaces this contract binds:
  `crates/ripr/src/output/badge/model.rs`, `cargo xtask badges`,
  `cargo xtask badge-basis`, `cargo xtask repo-badge-artifacts`,
  `.github/workflows/badge-endpoints.yml`, and the committed
  `badges/ripr.json` / `badges/ripr-plus.json` Shields endpoints.

## Metrics

- State fidelity: zero occurrences of a numeric badge rendered from a
  reject-list condition, measured by fixtures and the
  `badges --check` gate.
- Staleness: age of the committed endpoint relative to its source
  report stays under the configured maximum on `main`.
- Basis purity: 100% of public badge renders use
  `canonical_actionable_gap` or `gap_decision_ledger` basis.
- Promotion rule: move this spec from `proposed` to `accepted` when
  the badge projection implementation satisfies every entry of the
  verifier reject-list with fixture-backed proof and the proof
  commands above exist and pass.

## Failure Modes

- Stale committed endpoint — content drift is caught by
  `cargo xtask badges --check` today; age-based staleness is
  evaluated against the planned max-age knob. In either case the
  public badge must move to `ripr: stale`, not keep the old count.
- Limited run projected as a count — reject-list violation; named
  defect against this spec, caught by state-mapping fixtures.
- Raw-finding basis reaching the public badge — reject-list
  violation; render `ripr: unknown`.
- Diff-scoped `0` published as a repo badge — forbidden by the scope
  rule; the always-zero no-diff artifact must never become a README
  endpoint.
- Badge schema drift without a `BADGE_SCHEMA_VERSION` bump — public
  contract violation; call it out in the PR per the existing model
  documentation.
- Refresh workflow failure — fail closed exactly as today: no
  endpoint update, no claimed count, visible skip summary.
