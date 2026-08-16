# RIPR-SPEC-0130: RIPR authority map

Status: proposed

This specification maps RIPR's source-of-truth authorities (the files and
directories that constitute the durable campaign record) to their
canonical artifact categories, so cargo-allow and other tools can consume
them without a parallel ledger. It is the dependency-zero artifact for the
spec-v2 walking skeleton (#1672/#1673).

## Problem

RIPR's source-of-truth artifacts are spread across multiple directories
and files: AGENTS.md, docs/specs/, docs/proposals/, docs/adr/, plans/,
.allow/spec-system/slices/, .ripr/traceability.toml, docs/status/SUPPORT_TIERS.md,
policy/*.toml, and more. A consumer (cargo-allow, an agent context
compiler, a report generator) that needs to navigate these artifacts
currently has to know each path individually. There is no single
authority map that declares what lives where and what category it
belongs to.

This spec defines that map. It is read-only guidance, not a gate —
the artifacts themselves remain the authority.

## Behavior

The authority map is a declarative mapping from artifact category to
canonical path(s). It does not create, modify, or validate artifacts;
it only documents where they live so consumers can find them.

### Artifact categories

| Category | Canonical path(s) | Purpose |
|---|---|---|
| `agent_instructions` | `AGENTS.md` | Workspace instructions for coding agents |
| `proposals` | `docs/proposals/RIPR-PROP-*.md` | Source-of-truth proposals for new initiatives |
| `specifications` | `docs/specs/RIPR-SPEC-*.md` | Normative behavior specs |
| `adr` | `docs/adr/*.md` | Architecture decision records |
| `plans` | `plans/*/implementation-plan.md` | Implementation plans for campaigns |
| `implementation_slices` | `.allow/spec-system/slices/*.toml` | One PR's scope and claim boundary (`ImplementationSliceV1`; no live execution state) |
| `live_state` | GitHub issues, PRs, checks, reviews, local worktrees | Live work selection and ownership (not a tracked file) |
| `traceability` | `.ripr/traceability.toml` | Spec-to-test-to-code traceability graph (legacy/derived compatibility context) |
| `support_tiers` | `docs/status/SUPPORT_TIERS.md` | Language/platform support tier declarations |
| `policy_ledgers` | `policy/*.toml`, `policy/*.txt` | Controlled vocabularies and allowlists |
| `receipts` | `target/ripr/receipts/`, `docs/handoffs/` | Proof receipts and closeout handoffs |
| `metrics` | `metrics/*.json` | Corpus/scorecard metrics |
| `capabilities` | `docs/CAPABILITY_MATRIX.md` | Capability status matrix |

No goal file may select or authorize repository-wide live work. The legacy
`.ripr/goals/` scheduler was deleted in #1701's PR 3; completed campaign
history lives in closeout and handoff documents plus Git history. The
spec-system profile runs `generation = "current-v2"` with no goals root and
no active-goal requirement.

## Required Evidence

- This spec registered in `docs/specs/README.md`.
- `.allow/profiles/spec-system.toml` references the same root paths.

## Non-Goals

- This spec does not define the spec-v2 requirement/slice model (#1667).
- This spec does not validate artifact contents — only documents their
  canonical locations.
- This spec does not replace `policy/doc-artifacts.toml` (which is the
  machine-readable ledger); it complements it with the human-readable map.

## Acceptance Examples

- A consumer reading this spec can find every RIPR authority artifact
  by category without guessing paths.

## Test Mapping

- `cargo xtask check-spec-format` validates this spec's format.
- `cargo xtask check-doc-index` validates the spec is indexed.

## Implementation Mapping

- `docs/specs/RIPR-SPEC-0130-ripr-authority-map.md` — this spec.
- `.allow/profiles/spec-system.toml` — the cargo-allow profile that
  consumes the authority map's root paths.

## Metrics

- Authority coverage: every category in the map has at least one
  canonical artifact that exists on `main`.
