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
.ripr/goals/, .ripr/traceability.toml, docs/status/SUPPORT_TIERS.md,
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
| `goals` | `.ripr/goals/*.toml` | Campaign goal manifests |
| `traceability` | `.ripr/traceability.toml` | Spec-to-test-to-code traceability graph |
| `support_tiers` | `docs/status/SUPPORT_TIERS.md` | Language/platform support tier declarations |
| `policy_ledgers` | `policy/*.toml`, `policy/*.txt` | Controlled vocabularies and allowlists |
| `receipts` | `target/ripr/receipts/`, `docs/handoffs/` | Proof receipts and closeout handoffs |
| `metrics` | `metrics/*.json` | Corpus/scorecard metrics |
| `capabilities` | `docs/CAPABILITY_MATRIX.md` | Capability status matrix |

### Conformance fixture

A conformance fixture (`.allow/conformance/legacy-dialect.json`) asserts
that the legacy TOML dialects (`.ripr/goals/active.toml`,
`.ripr/traceability.toml`) parse without error against their expected
schema. This fixture is consumed by cargo-allow's spec-system profile.

## Required Evidence

- This spec registered in `docs/specs/README.md`.
- `.allow/profiles/spec-system.toml` references the same root paths.
- The conformance fixture exists at `.allow/conformance/legacy-dialect.json`.

## Non-Goals

- This spec does not define the spec-v2 requirement/slice model (#1667).
- This spec does not validate artifact contents — only documents their
  canonical locations.
- This spec does not replace `policy/doc-artifacts.toml` (which is the
  machine-readable ledger); it complements it with the human-readable map.

## Acceptance Examples

- A consumer reading this spec can find every RIPR authority artifact
  by category without guessing paths.
- The conformance fixture parses both legacy TOML files without error.

## Test Mapping

- `cargo xtask check-spec-format` validates this spec's format.
- `cargo xtask check-doc-index` validates the spec is indexed.
- The conformance fixture is validated by cargo-allow's spec-system
  profile (advisory, not a CI gate).

## Implementation Mapping

- `docs/specs/RIPR-SPEC-0130-ripr-authority-map.md` — this spec.
- `.allow/profiles/spec-system.toml` — the cargo-allow profile that
  consumes the authority map's root paths.
- `.allow/conformance/legacy-dialect.json` — the conformance fixture.

## Metrics

- Authority coverage: every category in the map has at least one
  canonical artifact that exists on `main`.
- Conformance: both legacy TOML files parse without error.
