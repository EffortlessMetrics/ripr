# Verification Contracts

This directory defines the portable verification contract for Rust repositories
that adopt RIPR-backed badges and PR evidence.

The contract keeps public repo state and pull-request evidence separate:

```text
Badges summarize repo state.
PR evidence routes work.
Neither impersonates the other.
```

Badges are the public front panel. They are repo-scoped, generated, and small
enough to be true. PR evidence is the instrument cluster. It is diff-scoped,
artifact-backed, and detailed enough to route review, agent work, and expensive
verification.

## Contract Map

| Contract | Owns |
| --- | --- |
| [Badge contract](badge-contract.md) | Public Shields endpoint JSON, repo-scope boundary, and badge claim limits. |
| [PR evidence contract](pr-evidence-contract.md) | Diff-scoped evidence summary, review guidance fields, and routing signal. |
| [Artifact layout](artifact-layout.md) | Standard paths for committed badge endpoints and generated CI artifacts. |
| [Annotation policy](annotation-policy.md) | Non-blocking annotation rules and the inline-comment opt-in boundary. |

Machine-readable schemas live under `schemas/`:

| Schema | Validates |
| --- | --- |
| [`schemas/badges/shields-endpoint.schema.json`](../../schemas/badges/shields-endpoint.schema.json) | Public committed badge endpoint JSON. |
| [`schemas/ripr/pr-evidence.schema.json`](../../schemas/ripr/pr-evidence.schema.json) | Canonical PR evidence summary packet. |
| [`schemas/ripr/review-comments.schema.json`](../../schemas/ripr/review-comments.schema.json) | `ripr review-comments` guidance output. |

The schema set and valid fixture packets are checked by:

```text
cargo xtask check-verification-contracts
cargo xtask check-verification-contracts --check
```

## Adoption Boundary

An adopted repository should expose the same command and path shape:

```text
cargo xtask badges
cargo xtask badges --check
cargo xtask ripr-pr
cargo xtask ripr-pr --check
cargo xtask ripr-review-comments
cargo xtask ripr-review-comments --check
```

The command set can be implemented in local `xtask` code, copied from a
template, or wrapped around public `ripr` commands. The contract is the stable
part: command names, output paths, schema shapes, and advisory defaults should
not drift across repos.

## Non-Goals

This contract does not:

- make RIPR findings blocking by default;
- publish inline PR comments by default;
- turn a badge into coverage, mutation proof, correctness proof, or release
  proof;
- require full mutation testing on every pull request;
- define a fleet dashboard;
- require a public helper crate.

## Relationship To Existing Docs

[`docs/VERIFICATION.md`](../VERIFICATION.md) is the short repo-local guide for
readers. These files are the portable implementation contract. Existing RIPR
workflow docs still own their detailed surfaces:

- [`docs/BADGE_POLICY.md`](../BADGE_POLICY.md) for RIPR badge counting policy;
- [`docs/PR_REVIEW_GUIDANCE.md`](../PR_REVIEW_GUIDANCE.md) for
  `ripr review-comments`;
- [`docs/PR_EVIDENCE_LEDGER_WORKFLOW.md`](../PR_EVIDENCE_LEDGER_WORKFLOW.md)
  for PR movement ledgers;
- [`docs/CI.md`](../CI.md) for generated GitHub workflow projection.
