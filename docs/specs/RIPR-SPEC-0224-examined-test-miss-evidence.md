# RIPR-SPEC-0224: Examined-test miss evidence

Status: proposed

Owner: analysis

Created: 2026-10-04

Linked issues:

- #5344 (finding says "Related tests were found" while listing none)
- #5329 (finding reports `reach: yes` with `related_tests_total: 0`)
- #5356 (`ripr explain` repeats the human-full block)
- #5508 (`observation_unconfirmed` read as an established miss)
- #5498 (Perl findings list tests without a reason)

Support-tier impact:

- None. No language support claim changes; the slice discloses facts the Rust
  classifier already computes.
  [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)

Policy impact:

- Adds the controlled output enum `related_test_miss` to
  `policy/output_contracts.txt` and `docs/OUTPUT_SCHEMA.md`. No gate, badge,
  or report file changes.

## Problem

A gap finding named at most the tests whose assertions matched the change. A
related test whose assertions all failed to match was dropped, so the finding
could say "Related tests were found" and `reach: yes` while listing no test
(16 findings across 4 of 12 shared-corpus repos, #5344). Where tests were
listed, the finding did not say why each one fails to catch the change, so a
developer who disagreed had to re-derive the analysis.

## Behavior

- Every related test the Rust classifier examines is retained in
  `related_tests`, including a test whose assertions match nothing. Such a
  test carries its first assertion as `oracle` (checked text), `oracle_kind:
  unknown` and `oracle_strength: none`, so it never supplies an oracle.
- Each retained test may carry `miss`, a `RelatedTestMiss` naming why it would
  not notice the change: `no_call_path`, `no_assertion`,
  `assertion_not_observing`, `assertion_not_credited`, `weak_assertion`,
  `missing_input`, `missing_exact_assertion`, `observation_unconfirmed`. The
  analyzer sets `no_assertion`, `assertion_not_observing`,
  `assertion_not_credited` and the name-only `no_call_path` while matching
  assertions; these are facts about the test and can appear under any class.
  The finding builder sets `no_call_path` for `no_static_path`, and
  `weak_assertion`, `missing_input` (predicate boundary facts only),
  `missing_exact_assertion` (error-variant and field facts) and
  `observation_unconfirmed` for `weakly_exposed` and `reachable_unrevealed`.
  `exposed` findings and the unknown classes get no class-level miss.
- `observation_unconfirmed` is an unknown, not an established miss: its sentence
  says ripr could not confirm that the assertion observes the changed
  behavior (#5508). Only `assertion_not_observing` claims the assertion
  observes something else.
- The Perl v1 producer (#5498) sets `observation_unconfirmed` on a row only
  when the finding is `weakly_exposed` and comes from a complete packet with
  no blocking limit (a concrete discriminator is not required), the row is a reachable direct owner call, its linked
  oracle is the strong exact, owner-targeted oracle that earned the weak
  exposure, and the shared sink-alignment check establishes no alignment for
  that row. Unequal sink text stays unconfirmed. Every other Perl row keeps no
  miss: advisory relations, weak or missing oracles, partial or blocked
  packets, and the finding-wide discriminator, which has no test identity.
- `miss` is evidence only. No stage, class, confidence, stop reason, or next
  step reads it. Post-classification gates that asked whether any related
  test survived oracle matching use `Finding::oracle_related_tests`, which
  skips `assertion_not_observing` rows, so their decisions are unchanged.
- Oracle rows are ranked and packed into the eight-row window exactly as
  before; `assertion_not_observing` rows take only the slots left free, so no
  window, fix site, exact-oracle alignment or repair readiness changes.
  Fix-site selection (`DiagnosticWitness`) reads only oracle rows.
- One prose owner, `output::related_test_miss`, renders the reason. The human
  digest appends it in parentheses after the related test; human-full prints
  `misses: <why>; checked <assertion>`; JSON, the context packet and MCP gap
  documents carry `miss` and `why`; LSP hover uses human-full's label and
  reason with its own row shapes (a matched row keeps its oracle strength and
  kind, only an unmatched row uses `<label>: ...; checked ...`, and a row with
  no recorded oracle shows only the reason, where human-full still prints its
  `uses none unknown oracle` projection first), and diagnostics add up to three
  related-information rows that open the examined test.
- `output::related_test_miss::related_test_miss_label` owns the word before
  the reason: `unconfirmed` for `observation_unconfirmed`, `misses` otherwise.
  Human-full, `ripr explain`, LSP hover and LSP related information use it;
  JSON, MCP and the context packet carry no label.
- `ripr explain` adds a "Why this verdict" section: every retained examined
  test with its verdict and checked assertion, what a test would need to change
  the verdict (gap classes only), and the meaning of each stop reason.

## Required Evidence

- A unit test that a test whose assertions match nothing is listed as
  `assertion_not_observing` with `none` strength and `unknown` kind, and the
  stages stay `no`.
- A unit test that eight matched tests keep the window when a ninth examined
  miss exists, and the total counts all nine; and that a test's second, strong
  assertion row keeps its slot when seven examined misses exist.
- A unit test that a finding whose only related tests are examined misses has
  no fix site.
- A unit test that each miss renders a reason naming a checkable fact.
- Golden fixtures re-blessed with no change to any finding's classification,
  confidence, severity, stop reasons, missing entries, or next step.

## Non-Goals

- Changing any verdict. Whether a self-contradicting gap should become an
  unknown is RIPR-SPEC-0221's admission rule.
- Splitting `assertion_not_credited` into proven-inert and admission-unproven
  cases; that needs the typed admission refusal from #5359.
- Python and TypeScript producers, which record no miss yet (#5491, #5495),
  and richer Perl reasons that need row-owned facts (#5562).

## Acceptance Examples

1. semver 1.0.23, `src/parse.rs:272` changed from `Some(&b'=')` to
   `Some(&b'>')`: the finding now lists `test_comparator_parse` with
   `miss: assertion_not_observing` and the checked
   `assert_to_string(parsed, "^1.2.3-alpha")`; before, it listed none while
   reporting `reach: yes`.
2. On the 20 fast-tier corpus repos, findings whose observe summary says
   "Related tests were found" with zero listed fall from 13 to 0, and
   `reach: yes` with zero listed falls from 5 to 0. Default human output keeps
   its line count (868 lines); JSON grows 5.1%.

## Test Mapping

- `crates/ripr/src/analysis/classify/reveal.rs::tests` — examined misses are
  listed and rank after matched tests.
- `crates/ripr/src/output/related_test_miss.rs::tests` — reason prose.
- `crates/ripr/src/analysis/language/perl/tests.rs` — the Perl v1 rule: only
  a direct, reachable, strong row without sink alignment is unconfirmed. These
  controls consume frozen packets (#5510); whether the live perl-lsp emitter
  produces complete, unblocked packets is tracked by #3216 and #3223.
- `crates/ripr/src/output/human/explain.rs::tests` — an unconfirmed row is not
  labelled a miss.
- Golden fixtures under `fixtures/*/expected/` — rendered parity.

## Implementation Mapping

| Surface | Responsibility |
| --- | --- |
| `crates/ripr/src/domain/probe.rs` | `RelatedTestMiss`, `RelatedTest::miss`, `Finding::oracle_related_tests` |
| `crates/ripr/src/analysis/classify/reveal.rs` | retain examined tests; set assertion-level misses; ranking |
| `crates/ripr/src/analysis/classifier/finding.rs` | class-level misses |
| `crates/ripr/src/analysis/language/perl/mod.rs` | Perl v1 `observation_unconfirmed` rows |
| `crates/ripr/src/output/related_test_miss.rs` | the one prose and label owner |
| `crates/ripr/src/output/human/{sections,evidence_lines,explain}.rs`, `output/json/report.rs`, `lsp/{hover,diagnostics}.rs`, `mcp/gaps.rs` | projections |

## Metrics

- `trust.self_contradictions` (dx-scoreboard rule R3): target 0.
- Share of gap findings whose listed tests carry a reason: 141 of 195 on the
  fast tier at this slice.
