# Rust governed pilot — 2026-07-13

This is the fresh two-candidate-per-repository pilot after the corpus
reconciliation. Each candidate ran in a clean detached worktree at the exact
merged PR head. The first step was analysis-only `ripr review-comments`; no
consumer production source was edited, pushed, or counted as an eligible
repair attempt.

## Results

| Repository / candidate | Head | Analysis result | Route result | Corpus treatment |
| --- | --- | --- | --- | --- |
| ripr-swarm #1581 | `86fbe048eeedd0eeb6090db96355791c40b3486b` | `advisory`; 3 comments, 7 summary-only, 0 routes | producer-owned `missing_discriminator_evidence`; no test/verify/receipt route | static-limitation exclusion; analyzer follow-up #1427 and witness/fix-site follow-up #1567 |
| ripr-swarm #1580 | `2f71673f9c2da4532a1c1fb8e011dc6fd586ad2f` | `advisory`; 3 comments, 7 summary-only, 0 routes | producer-owned `missing_discriminator_evidence`; no test/verify/receipt route | static-limitation exclusion; analyzer follow-up #1427 and witness/fix-site follow-up #1567 |
| perl-lsp-swarm #4022 | `968464954e5da8e69a0b6b55de8ac349056924f6` | bounded run timed out at 900 seconds | no eligibility result | timeout exclusion; performance/timeout evidence remains separate from product failure |
| perl-lsp-swarm #4037 | `6f175766ee03921a7ae13887d915ce9331647516` | bounded run timed out at 900 seconds | no eligibility result | timeout exclusion; performance/timeout evidence remains separate from product failure |
| ub-review #772 | `217633ca232120a021c7dc975973abdcb5056d39` | `advisory`; 3 comments, 7 summary-only, 0 routes | producer-owned `missing_discriminator_evidence`; no test/verify/receipt route | static-limitation exclusion; analyzer follow-up #1427 and witness/fix-site follow-up #1567 |
| ub-review #744 | `84a365e4f509c866e01b51cbd5e5ae0c22b7302b` | `advisory`; 3 comments, 7 summary-only, 0 routes | producer-owned `missing_discriminator_evidence`; no test/verify/receipt route | static-limitation exclusion; analyzer follow-up #1427 and witness/fix-site follow-up #1567 |

### perl-lsp-4022

The bounded invocation was `ripr review-comments --root . --base
1a25013ad341caacbaf96ab2298edeea4f1a5e90 --head
968464954e5da8e69a0b6b55de8ac349056924f6 --out
target/ripr/fresh-pilot-4022.json`. The operator bound was 900 seconds;
the run timed out before producing a report.

### perl-lsp-4037

The bounded invocation was `ripr review-comments --root . --base
26ed34f8f1357076f91aed418949dbe7288c93b3 --head
6f175766ee03921a7ae13887d915ce9331647516 --out
target/ripr/fresh-pilot-4037.json`. The operator bound was 900 seconds;
the run timed out before producing a report.

The completed reports are retained in each adopting worktree under
`target/ripr/fresh-pilot-{1581,1580,772,744}.json`. The two timeout commands
did not produce a completed report; their exact command and 900-second
operator bound are recorded in the corpus exclusion rows and this document.

## Counting boundary

The pilot adds six observed runs, six unique exclusions, two timeout
observations, and zero eligible attempts. It does not add a case. The prior
repeated ub-review #747 observation remains one duplicate observation and is
not re-counted here.

These results establish only that the six selected routes were not eligible
under the producer-owned readiness contract at their exact heads. They do not
establish route quality, repair usefulness, support promotion, or a universal
Rust-seam limitation.
