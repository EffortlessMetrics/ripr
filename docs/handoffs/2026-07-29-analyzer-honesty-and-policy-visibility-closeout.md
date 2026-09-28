# Analyzer Honesty and Policy Visibility Closeout

Date: 2026-07-29

Campaign: `analyzer-honesty-and-policy-visibility` (Campaign 34)

## Objective

Make analyzer and policy limitations visible instead of silently skipped or
over-credited, while preserving `ripr`'s conservative static, advisory
contract.

## Decision and landed work

- #2703 (`fix(policy): scan editor extension language files`) now includes
  `.ts`, `.js`, `.tsx`, and `.jsx` editor sources in the static-language policy
  scan, with focused xtask coverage.
- #2702 (`analysis: disclose lexical fallback in repo mode`) records parser to
  lexical fallback at the producer, carries the provenance through file-facts
  and classified-seam cache schemas and warm loads, and emits stable sorted
  disclosure in repo/seam inventory paths.
- The fallback change remains under-crediting and disclosure-only. It does not
  turn lexical facts into runtime mutation results, adequacy claims, or release
  authority.

## Merge and proof receipt

- PR #2703 merged at `b07f33ba` on main after its final `.tsx`/`.jsx` head and
  hosted required proof passed.
- PR #2702 merged at `02ec6069` on main from final head `c9e0c1b8` after the
  hosted required result passed.
- Required proof for #2702: `Ripr Rust Small Result`, hosted Rust gates,
  `ub-review`, `rust-tests-junit`, and `rust-coverage` passed for the final
  head. The routed workflow also evaluated source-truth, dependency, security,
  and policy evidence for the PR surface.
- The PR repair chain included formatting, dead-code, and type-complexity
  corrections; only proof for the final head is authoritative.

Local Cargo test and Clippy attempts were not-run-to-completion because the
Windows checkout was in the known Cargo/rustc contention window and produced
no diagnostic output. This is a proof boundary, not a product failure; hosted
current-head proof is the merge evidence.

## Claim boundary and follow-up

Users may believe that the policy scan covers the listed editor extension
languages and that repo/seam inventory discloses parser fallback, including
cache replay. They may not infer runtime mutation results, test adequacy,
coverage adequacy, release readiness, or default blocking from this campaign.

Issue #2699 is closed by its merged PR. Issue #2698 was closed in the campaign
closeout after #2702 merged; both execution trackers now point to their durable
receipts. No broader parser replacement, runtime mutation work, or default
policy hardening is included; those remain separate future slices.
