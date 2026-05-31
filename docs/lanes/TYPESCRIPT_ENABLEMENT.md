# TypeScript Enablement Lane

Status: Active successor lane; TypeScript preview completion is closed

Date: 2026-05-31

Scope: make TypeScript and JavaScript preview useful, honest,
repair-card-shaped, and promotion-ready without re-opening preview bootstrap.
This lane improves the quality of existing syntax-backed preview evidence. It
does not promote TypeScript/JavaScript support, add runtime analysis, or grant
preview evidence gate or badge authority.

## Current Boundary

TypeScript and JavaScript already have opt-in preview support. The closed
preview-completion lane and closeout prove syntax-backed owners, tests,
assertions, related-test matching, probe facts, static limitations, strict
preview actionability, LSP/CI projection, dogfood receipts, and route-quality
artifacts for the current preview scope.

This lane starts after that closeout. Do not restart router/config, owner
extraction, test/assertion extraction, probe-family classification, static-limit
taxonomy, preview actionability, LSP/CI projection, dogfood, or route-quality
bootstrap unless current `main` proves a regression.

## Product Goal

A TypeScript or JavaScript user who enables preview should get advisory guidance
that names:

- changed owner;
- related test or observer;
- oracle kind and oracle strength;
- changed behavior shape;
- missing discriminator or static limitation;
- suggested proof shape when safe;
- why a repair packet is not actionable when fields are missing;
- preview authority boundary.

The useful output is not Rust parity. It is a bounded preview card such as:

```text
TypeScript preview
Owner: src/pricing.ts::discountedTotal
Related evidence: tests/pricing.test.ts reaches this owner
Oracle strength: smoke-only
Changed behavior: return value
Recommendation: add exact return-value assertion
Authority: advisory preview; no default gate or badge contribution
```

## Non-Goals

This lane must not add:

- `tsc`, `tsserver`, package graph resolution, bundlers, or sourcemaps;
- Jest/Vitest runtime execution;
- mutation execution;
- provider/model calls;
- generated tests;
- source edits;
- default CI blocking;
- public badge, baseline, or RIPR Zero contribution;
- support-tier promotion without a separate promotion packet.

Heuristic related-test links remain advisory. Weak oracle evidence remains weak
unless a fixture-backed strict packet path lands.

## Ownership Boundaries

TypeScript enablement owns precision and usefulness for TypeScript-family
preview evidence.

Lane 1 owns the shared evidence spine, canonical actionability contract, and
repair-loop truth model.

Lane 3 owns editor UX projection. TypeScript enablement may only ask Lane 3 to
project TypeScript facts that already exist.

Lane 4 owns PR/CI review projection. TypeScript enablement may only ask Lane 4
to group or summarize TypeScript preview evidence within existing advisory
boundaries.

Python repair routing and future Perl work are parallel language-enablement
lanes. They may reuse patterns, but they do not change TypeScript support
claims.

## Active Queue

1. Sharpen weak-oracle preview guidance:
   - snapshots advise exact-value assertions alongside snapshots;
   - smoke-only truthiness advises exact-value assertions;
   - mock interaction without safe payload proof stays advisory;
   - bounded literal/object/count mock payload evidence may name the callee and
     expected value, but still stays advisory until verify command, receipt
     command, evidence refs, and edit boundaries exist;
   - literal `toThrow(...)`, `rejects.toThrow(...)`, and safe
     `rejects.toMatchObject(...)` payload evidence may count as exact
     error-variant evidence, but still does not emit repair packets without
     strict actionability fields;
   - broad error evidence stays weak until error payload or variant support can
     name a bounded repair route.
2. Audit false-actionable candidates:
   - mock interaction without payload proof;
   - broad `toThrow` / `rejects`;
   - snapshot-only and smoke-only evidence;
   - heuristic related-test links;
   - owner name in test title only;
   - method receiver repair-packet gaps and remaining ambiguity;
   - module initializer ambiguity;
   - mocked module, decorator, and dynamic-dispatch limits.
3. Expand narrow mock payload support only when syntax can safely name the
   target interaction.
4. Distinguish broad error checks from exact error payload or rejected-promise
   evidence.
5. Continue bounded method receiver relation support:
   - direct `new ClassName(...)` receiver calls are fixture-backed;
   - factories, dependency injection, mocked classes, prototype aliases, and
     dynamic property access stay advisory or unsupported.
6. Improve module initializer guidance while keeping dynamic initialization
   limited:
   - direct named-import and namespace `expect(...)` observers for exported
     module initializers are fixture-backed advisory evidence;
   - relational and broad observers stay non-actionable until target-shape,
     verify, receipt, evidence-ref, and edit-boundary fields are available;
   - helper-derived values, shadowed aliases, and dynamic initialization remain
     advisory or limited.
7. Emit TypeScript preview repair-card fields consistently through existing
   advisory surfaces.
8. Record real TypeScript/JavaScript dogfood receipts with at least one
   unchanged or intentionally skipped case.
9. Decide whether TypeScript/JavaScript remain preview or have enough false-
   actionable audit, dogfood, route-quality, and surface proof for a narrow
   promotion packet.

## False-Actionable Audit Packet

[typescript-preview-false-actionable-audit](../../fixtures/typescript-preview-false-actionable-audit/corpus.json)
is the current audit packet for preview cases that can look repair-shaped but
must not become actionable packets without stricter proof. It pins:

- mock interaction without payload proof;
- broad thrown or rejected error evidence without payload proof;
- snapshot-only and smoke-only weak oracles;
- heuristic related-test links and owner-name-only test titles;
- method receiver repair-packet gaps, remaining receiver ambiguity, and
  module-initializer missing target shape;
- mocked module, decorator-indirection, and dynamic-dispatch static limits.

Each row points at an existing checked TypeScript-family fixture finding,
records the current disposition, preserves `repair_packet_ready = false`, and
names the future support route required before any repair-card or promotion
claim can be considered.

## Validation

Use the narrowest relevant checks first. For TypeScript enablement slices,
prefer:

```bash
cargo test -p ripr typescript --lib
cargo xtask fixtures typescript_jest_vitest_assertion_facts
cargo xtask fixtures typescript_probe_facts
cargo xtask ripr-swarm attempt-ledger
cargo xtask ripr-swarm readiness
cargo xtask check-fixture-contracts
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-pr
git diff --check
```
Docs-only changes should run:

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
git diff --check
```
