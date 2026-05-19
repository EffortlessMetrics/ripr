# RIPR-SPEC-0056: Public Actionable Projection

Status: accepted

## Problem

Public and first-use RIPR surfaces must lead with the work a user can actually
act on. Raw findings, seam-native inventory, static-limit pressure, and
unknown-only evidence are useful diagnostics, but they are not a public repair
queue.

The public badge previously risked presenting seam-native repo inventory as the
headline count. That made a large analyzer pressure number look like a manual
repair queue. The public badge, first-pr packet, PR/CI summaries, editor
packets, release copy, and scorecard lead sections need one shared public unit:

```text
canonical gap
-> repair route
-> related test / repair target
-> verify command
-> receipt command/state
-> advisory/static/preview boundary
```

## Behavior

RIPR public projection surfaces render user-actionable canonical repair work as
the headline unit. They may still expose raw findings, seam-native inventory,
static limitations, and preview boundaries as supporting evidence, but those
signals must not move public repair counts unless they satisfy the eligibility
rules below.

The public badge generator uses `canonical_actionable_gap` for repo-scoped
README, crate, store, and release headline badges. `seam_native` remains an
internal inventory basis. `gap_decision_ledger` remains the explicit
policy-backed bridge when a caller supplies projection targets.

## Definitions

| Term | Contract |
| --- | --- |
| Raw finding | Analyzer evidence. It is supporting input, not user-facing work by itself. |
| Seam-native inventory | Repo-level classified seam pressure. It is internal diagnostic inventory, not the public repair badge basis. |
| Canonical evidence item | Grouped evidence identity that can support user-facing projection. It is countable evidence, not always actionable repair work. |
| Actionable canonical gap | Unresolved canonical repair item with enough typed evidence to route one bounded repair and verification loop. |
| Public projection | Any README, crate, store, release, first-screen, PR/CI, first-pr, or editor headline that claims to show work remaining. |
| Internal inventory | Detailed reports and scorecards that explain analyzer pressure and evidence quality for maintainers. |

## Public Projection Eligibility

An item is eligible for public projection only when the effective
`public_projection_eligible` decision is true. Implementations may materialize
that decision as a field or derive it from typed fields, but they must not infer
it from prose.

`public_projection_eligible = true` requires:

- `gap_state = unresolved`;
- `actionability = actionable`;
- a canonical gap identity;
- a structured repair route;
- a related test, repair target, or safe typed unknown target fallback;
- a verify command;
- a receipt command or receipt state path that can be emitted;
- no suppression;
- no intentional disposition;
- no no-action or already-observed disposition;
- no unpromoted preview-only evidence;
- no runtime-only evidence requirement;
- no raw-only finding state;
- no seam-inventory-only state;
- no static limitation without actionability;
- current root, schema, freshness, path, and command safety checks.

Items that fail any requirement are still allowed in internal reports when they
are explicitly labeled with their limitation or exclusion reason.

## Basis Vocabulary

| Basis | Scope | Public headline allowed | Meaning |
| --- | --- | --- | --- |
| `canonical_actionable_gap` | repo | yes | Unresolved canonical repair items eligible for public projection. |
| `finding_exposure` | diff | no | Legacy PR-local Finding/ExposureClass count basis. |
| `seam_native` | repo inventory | no | Repo seam inventory and static limitation pressure. |
| `gap_decision_ledger` | repo or diff | yes when explicit | Policy-backed bridge for explicit projection targets. |

Public README, crate, store, and release headline `ripr` / `ripr+` badges must
use `canonical_actionable_gap` unless they are rendered from explicit
gap-decision-ledger projection targets. A public surface may mention
`seam_native` only when it is plainly labeled as seam inventory and does not
reuse the main `ripr` / `ripr+` repair badge headline.

## `ripr` and `ripr+`

`ripr` counts unresolved actionable canonical gaps eligible for public
projection.

`ripr+` counts:

- unresolved actionable canonical gaps; plus
- actionable test-efficiency repair items only after they are lifted into the
  same repair / verify / receipt model.

`ripr+` must not count every raw test-efficiency finding. A test-efficiency
item is eligible only when it has:

- a repair kind;
- a target test or owner;
- a verify command;
- a receipt path or command;
- no intentional disposition;
- no suppression;
- no unpromoted preview-only basis.

## Exclusion Reasons

Projection reports and downstream status surfaces should name exclusions using
stable reason strings where practical:

- `suppressed`;
- `intentional`;
- `no_action`;
- `already_observed`;
- `static_limited`;
- `missing_repair_route`;
- `missing_verify_command`;
- `missing_receipt_path`;
- `preview_unpromoted`;
- `runtime_only`;
- `raw_only`;
- `seam_inventory_only`;
- `malformed`;
- `stale`;
- `wrong_root`;
- `unsupported_schema`;
- `path_unsafe`;
- `command_unsafe`.

The absence of a public count must not hide these states. They remain evidence
for internal reports, setup guidance, or future repair work.

## Badge-Basis Audit Report

`cargo xtask badge-basis` writes:

```text
target/ripr/reports/badge-basis.json
target/ripr/reports/badge-basis.md
```

The JSON report must include:

- `schema_version`;
- current public endpoint values;
- current repo badge native basis and counts;
- recommended public projection basis;
- canonical actionable gap counts;
- test-efficiency class counts when available;
- seam-native inventory status and class counts when collected;
- raw alignment signal status;
- canonical evidence item status;
- static-limit inventory status;
- suppressed or intentional item status/count;
- no-action item status/count;
- warnings;
- non-claims.

The audit is advisory and must not edit `badges/*.json`. It exists to prove the
public count, basis, and internal inventory split before endpoint refreshes.

## Internal Seam-Native Inventory

`seam_native` inventory is an internal pressure gauge. It may appear in:

- `cargo xtask badge-basis --include-seam-classes`;
- repo-exposure reports;
- seam-inventory reports;
- evidence-quality scorecards;
- closeout proof.

It must not drive the public `ripr` / `ripr+` badge message unless that badge
is explicitly relabeled as seam inventory.

## Surface Alignment

| Surface | Primary public unit | Supporting-only units |
| --- | --- | --- |
| Public badge | Actionable canonical gaps. | Seam inventory, raw findings, static limitations. |
| README / crate / store copy | Actionable static repair gaps and non-claims. | Analyzer pressure details. |
| LSP/editor packets | Current actionable gap or safe no-action state. | Preview/static-limit boundaries. |
| First-pr packet | Top repairable gap and receipt state. | Missing/malformed/no-action states. |
| PR/CI evidence summary | Canonical gap, repair route, verify command, receipt state. | Raw finding counts. |
| Evidence-quality scorecard | Actionable canonical gap lead section. | Inventory and quality pressure. |
| Badge-basis report | Public basis proof plus internal inventory. | Raw and seam-native diagnostics. |
| Release notes/support tiers | User-actionable workflow and advisory boundary. | Internal lane history. |

Raw findings, seam-native inventory, static limitations, and preview evidence
may be shown only when their status and non-claims are clear.

## Generated Endpoint Rule

Committed `badges/*.json` files are generated endpoint snapshots. They are not
hand-authored source of truth.

After the public projection contract lands:

- do not refresh public endpoint JSON from a `seam_native` basis;
- do not hand-edit badge values;
- do not merge badge endpoint diffs in ordinary feature or docs PRs;
- refresh endpoints only through the generated badge workflow or an explicitly
  scoped `badge: refresh public endpoints` PR;
- include badge-basis proof when public counts change.

## Executable Guards

The contract is executable through these commands:

```bash
cargo xtask badge-basis
cargo xtask badge-basis --include-seam-classes
cargo xtask check-badge-endpoints
cargo xtask check-badge-diff-policy
cargo xtask check-generated-clean
cargo xtask check-output-contracts
cargo xtask check-product-copy
cargo xtask check-traceability
cargo xtask check-capabilities
```

The guards must fail or warn when:

- public badge copy presents `seam_native` as the main repair badge basis;
- public endpoint JSON changes outside a badge refresh context;
- public badge wording claims actionable repair work while the generator basis
  is inventory;
- generated badge endpoint values are hand-edited;
- output schema or traceability no longer names the public projection basis.

## Required Evidence

The public actionable projection contract is supported by:

- committed public endpoint JSON under `badges/`;
- native badge output schema fields for `scope`, `basis`, `counts`, and
  `warnings`;
- `cargo xtask badge-basis` audit reports;
- badge-basis audit schema in `docs/OUTPUT_SCHEMA.md`;
- badge policy wording in `docs/BADGE_POLICY.md`;
- public copy in README and crate README;
- generated endpoint ownership guards;
- traceability from this spec to tests, code, outputs, and badge fixtures;
- closeout proof recording old count, new count, old basis, new basis,
  generator command, guard commands, and internal seam-inventory location.

## Inputs

- `badges/ripr.json`;
- `badges/ripr-plus.json`;
- native repo badge output from `ripr check --format repo-badge-json`;
- native repo plus badge output from `ripr check --format repo-badge-plus-json`;
- optional `gap-decision-ledger` projection targets;
- test-efficiency reports;
- optional repo-exposure reports for internal seam inventory;
- badge policy and public copy.

## Outputs

- public Shields endpoint JSON;
- native repo badge JSON;
- badge-basis JSON and Markdown;
- internal seam-native inventory in badge-basis, repo-exposure, or scorecard
  reports;
- public docs and release copy that explain actionable counts and non-claims.

## Non-Goals

- No analyzer expansion.
- No new evidence classes.
- No policy promotion.
- No mutation execution.
- No generated tests.
- No provider calls.
- No source edits.
- No default CI blocking.
- No LSP/editor behavior changes.
- No PR comment publishing.
- No release, tag, or publish work.
- No manual badge value edits.
- No use of seam-native inventory as the public repair badge basis.

## Non-Claims

Public projection is not:

- coverage;
- mutation adequacy;
- runtime proof;
- merge approval;
- policy eligibility;
- gate authority;
- preview-language promotion;
- complete seam inventory.

## Acceptance Examples

Public actionable badge:

- Given committed public badge endpoints and native repo badge output with
  `basis = "canonical_actionable_gap"`, the README / crate / store headline may
  render the badge as unresolved actionable repair work.

Internal seam inventory:

- Given `cargo xtask badge-basis --include-seam-classes`, the report may show
  seam-native class counts such as `seams_total`, `headline_eligible`, and
  `activation_unknown`, but the committed `ripr` and `ripr+` endpoint messages
  remain the actionable canonical counts.

Unlabeled seam-native public copy:

- Given README or store copy that describes the public `ripr` badge as
  `seam_native`, `check-badge-diff-policy` rejects the surface unless it is
  explicitly labeled as seam inventory and does not reuse the main repair badge
  headline.

Generated endpoint refresh:

- Given a change to `badges/ripr.json` or `badges/ripr-plus.json`, ordinary PR
  checks reject it unless the PR is an explicit badge refresh context and the
  generated endpoint checks pass.

`ripr+` test-efficiency inventory:

- Given raw test-efficiency findings without repair / verify / receipt
  projection, the findings remain inventory and do not increase the public
  `ripr+` headline.

## Test Mapping

Current tests and checks:

- `xtask::tests::badge_basis_default_derives_repo_plus_without_second_badge_job`
- `xtask::tests::badge_basis_derived_repo_plus_keeps_canonical_headline_and_te_context`
- `xtask::tests::badge_basis_derived_repo_plus_handles_missing_test_efficiency_context`
- `xtask::tests::badge_basis_seam_native_counts_are_internal_when_public_basis_is_canonical`
- `xtask::tests::badge_basis_report_markdown_names_actionable_recommendation`
- `xtask::tests::public_badge_basis_guard_rejects_seam_native_repair_badge_copy`
- `xtask::tests::public_badge_basis_guard_allows_explicit_seam_inventory_badge_copy`
- `xtask::tests::check_badge_diff_policy_rejects_public_surface_seam_native_from_git_status`
- `cargo xtask badge-basis`
- `cargo xtask badge-basis --include-seam-classes`
- `cargo xtask check-badge-endpoints`
- `cargo xtask check-badge-diff-policy`
- `cargo xtask check-generated-clean`
- `cargo xtask check-output-contracts`
- `cargo xtask check-product-copy`
- `cargo xtask check-traceability`

## Implementation Mapping

Landed implementation surfaces:

- `xtask/src/main.rs` implements badge-basis reporting, generated endpoint
  refresh, native basis parsing, and badge diff policy guards.
- `docs/BADGE_POLICY.md` defines public badge meaning and internal inventory
  separation.
- `docs/OUTPUT_SCHEMA.md` defines native badge output and badge-basis audit
  report fields.
- `target/ripr/reports/actionable-gaps.{json,md}` records packet-level
  `public_projection_eligible` and `projection_exclusion_reasons[]` readiness
  diagnostics for emitted actionable packets without changing endpoint counts.
- `badges/ripr.json` and `badges/ripr-plus.json` are generated public endpoint
  snapshots.
- `docs/handoffs/2026-05-19-public-badge-projection-realignment-closeout.md`
  records proof and remaining non-claims.

## Metrics

Traceability uses these contract metrics:

- `public_actionable_projection_badge_basis_locked`;
- `public_actionable_projection_seam_inventory_internal`;
- `public_actionable_projection_badge_basis_guarded`.

The contract is evaluated through these report values:

- public `ripr` endpoint message;
- public `ripr+` endpoint message;
- native repo badge `basis`;
- native repo badge `counts.unsuppressed_exposure_gaps`;
- native repo badge `counts.unsuppressed_test_efficiency_findings`;
- badge-basis `canonical_actionable_gap.ripr_count`;
- badge-basis `canonical_actionable_gap.ripr_plus_count`;
- badge-basis `seam_native.counts_by_class`;
- badge-basis `supporting_signals` statuses.

## Current Implementation Evidence

The landed implementation proves:

- public endpoint messages are generated from `canonical_actionable_gap`;
- the old seam-native count remains available through internal reports;
- `badge-basis` decomposes current public values and recommended basis;
- `check-badge-diff-policy` guards public surfaces from unlabeled
  `seam_native` drift;
- `check-generated-clean` and `check-badge-endpoints` protect generated
  endpoint ownership;
- public copy explains the actionable gap meaning and non-claims;
- actionable-gap packets distinguish agent-usable repair packets from public
  projection readiness by naming missing receipt or canonical guidance
  prerequisites;
- packet-level readiness remains fail-closed for observed, no-action,
  suppressed, or intentional dispositions even if malformed upstream packets
  carry repair, verify, and receipt fields;
- the closeout handoff records old count, new count, old basis, new basis,
  generator command, guard command, and internal seam-inventory location.
