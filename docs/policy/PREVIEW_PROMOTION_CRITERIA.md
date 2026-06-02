# Preview Promotion Criteria

Status: active policy reference

Spec: [RIPR-SPEC-0044](../specs/RIPR-SPEC-0044-preview-evidence-promotion-packet.md)

Related support tier: [Support tiers](../status/SUPPORT_TIERS.md)

## Purpose

This page defines the proof required before TypeScript, JavaScript, or Python
preview evidence can be reviewed for a stronger support tier or policy role.

Preview promotion is not automatic. A complete packet only means maintainers
may review a future explicit policy or support-tier change. Until that later
change lands, preview evidence remains opt-in, visibly labeled, advisory, and
non-gating.

## Required Evidence

| Evidence | Required proof |
| --- | --- |
| Fixture matrix | Representative fixtures cover the candidate language, evidence class, and known static-limit shapes. |
| Dogfood receipts | External-style receipts exercise the language/class through start-here, verify, receipt, and closeout surfaces. |
| Related-test accuracy | Maintainer-reviewed samples show related-test routing does not send repair packets to the wrong tests. |
| Static-limit taxonomy | Known parser, import, dynamic-dispatch, metaprogramming, mock, decorator, and unsupported-syntax limits are labeled or excluded. |
| False-positive review | Maintainers review a sample of surfaced gaps and record false-positive boundaries. |
| False repair packet review | Maintainers review generated repair packets for overclaiming, invented safe repairs, and missing non-claims. |
| Surface consistency | CLI, generated CI, PR evidence, editor projection, receipts, docs, and support tiers use the same preview/advisory boundary. |
| Support-tier claim update | Any stronger user-facing claim updates `docs/status/SUPPORT_TIERS.md` with proof commands and known limits. |
| Generated CI posture | Generated CI remains advisory and non-blocking unless a separate explicit gate policy changes that. |
| Baseline behavior | Preview findings do not become default RIPR Zero, baseline, or gate debt by accident. |
| Waiver and suppression behavior | Waivers and suppressions preserve owner, reason, scope, language, and `language_status = "preview"`. |
| Rollback path | The packet documents how to return to preview/advisory status. |
| Policy signoff | A policy owner explicitly signs off on the narrow language/class and target status. |

Optional mutation calibration can add context, but missing mutation calibration
must not be treated as calibrated confidence and must not be inferred from Rust.

## Review States

| State | Meaning |
| --- | --- |
| `blocked` | Required evidence is missing or malformed. Preview status stays unchanged. |
| `reviewable` | Required evidence is present. Maintainers may review a separate policy/support-tier change. |
| `rejected` | Evidence exists but does not justify the requested stronger claim. Preview status stays unchanged. |

The default packet state is blocked:

```bash
ripr policy preview-promote --language typescript --class boundary_gap
```

The command must produce `allowed_now = false` unless explicit evidence is
supplied and recognized by the packet implementation.

## Non-Claims

A preview promotion packet does not:

- promote the language or class by itself;
- make preview evidence gate-eligible;
- add RIPR Zero or baseline blocking debt;
- prove runtime behavior, coverage adequacy, mutation adequacy, or correctness;
- mutate config, baselines, suppressions, workflows, branch protection, history,
  generated CI defaults, or source files;
- authorize PR comment publishing or default CI blocking.

## Rollback

Rollback keeps or restores advisory preview status:

1. Keep or return the language/class to `preview` in support tiers.
2. Remove any manually reviewed promotion config in a separate policy PR.
3. Regenerate policy operations and preview-promotion packets.
4. Preserve the receipts that explain why stronger status was declined or
   reverted.
