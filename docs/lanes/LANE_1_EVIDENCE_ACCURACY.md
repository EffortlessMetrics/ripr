# Lane 1: Evidence Accuracy Evaluation

Lane 1's Evidence Spine Stabilization work is complete within the documented
v0.1 scope. The shared `evidence_record` is now stable enough for repo
exposure, agent seam packets, RIPR Zero repair routes, evidence movement,
assistant proof, baseline and PR ledger identity, canonical gap identity,
related-test ranking, oracle semantics, local delta flow, imported
static/runtime confidence labels, and gate baseline comparison.

PR #697, `gate: prefer canonical evidence identity`, is the final consumer
closeout for that stabilization pass. The next Lane 1 objective is no longer
more consumer wiring. It is evidence accuracy evaluation.

## Goal

Use the stable `evidence_record` to measure and improve evidence quality under
dogfood pressure. Lane 1 should answer:

```text
Where is RIPR's evidence still wrong, shallow, duplicated,
overconfident, underconfident, or uncalibrated?
```

The expected end state is:

- a dogfood evidence-quality report over current repo evidence;
- categorized top evidence-quality gaps;
- fixture-pinned high-impact duplicate groups, shallow related-test choices,
  static limitations, and calibration gaps;
- evidence-health consuming `evidence_record` consistently;
- calibration labels expanded only for checked fixture classes;
- capability promotions kept scoped to fixture-backed or calibrated evidence.

## Boundary

This campaign is measured expansion, not a new product surface.

Non-goals:

- no PR or CI front panel work;
- no PR review docs;
- no LSP or editor polish;
- no platform, MSRV, or dependency cleanup;
- no new gate policy;
- no default blocking;
- no provider integration;
- no generated tests;
- no mutation execution;
- no score redefinition.

Do not change `.ripr/goals/active.toml` for this lane unless the shared
tracker explicitly makes Lane 1 active. The repo-wide active manifest may point
at another lane without changing this Lane 1 plan.

## Planned Slices

| Slice | Intent | Gate |
| --- | --- | --- |
| `docs: open Lane 1 evidence accuracy evaluation` | Record evidence-spine completion and open this tracker. | No code behavior changes. |
| `report: add Lane 1 evidence quality audit` | Generate `target/ripr/reports/lane1-evidence-audit.{json,md}` from existing repo exposure and `evidence_record` data. | Repo-local report only. |
| `fixtures: pin top evidence-quality failures` | Fixture the top 3-5 audit findings before changing analyzer behavior. | Positive and negative cases both present. |
| `analysis: reduce duplicate canonical gap overcount` | Refine grouping only if audit and fixtures show duplicate groups are a top issue. | Same owner, seam kind, flow sink, and missing discriminator group together; different discriminators and owners stay separate. |
| `analysis: improve related-test ranking from audit cases` | Adjust ranking only for fixture-pinned misses from the audit. | Direct owner calls and stronger oracles remain primary; recency stays a tie-breaker. |
| `analysis: improve oracle semantics from audit cases` | Add supported oracle shapes only when the audit identifies misclassified cases. | Observed and missed behavior are explicit; unsupported helpers stay static limitations. |
| `calibration: expand runtime fixture classes` | Add checked runtime fixture classes for side effects, mock expectations, snapshots, and dynamic or opaque dispatch. | Runtime-only signal does not create a static gap; no CI mutation execution. |
| `report: evidence health consumes audit findings` | Fold durable audit fields into evidence-health. | No policy decisions or blocking. |
| `campaign: close Lane 1 evidence accuracy evaluation` | Close after at least one audit-driven improvement lands. | Future work listed by evidence class, not surface. |

## Evidence Quality Audit

The next implementation PR should add a repo-local audit command, such as:

```bash
cargo xtask lane1-evidence-audit
```

or:

```bash
cargo xtask evidence-quality-audit
```

It should write:

```text
target/ripr/reports/lane1-evidence-audit.json
target/ripr/reports/lane1-evidence-audit.md
```

The audit should summarize:

- raw headline gaps;
- canonical gap groups;
- largest canonical groups;
- duplicate-looking groups;
- missing discriminator classes;
- static limitations by reason;
- oracle semantics distribution;
- related-test ranking confidence;
- movement availability;
- calibration availability;
- calibrated versus uncalibrated classes;
- `evidence_record` missing or nullable fields;
- top files by unresolved evidence debt.

The audit should identify whether RIPR is overcounting equivalent gaps,
ranking weak related tests too highly, overstating broad oracles, missing
candidate values, or leaving calibration labels sparse. It should not change
gate behavior, PR or CI projection, LSP UX, analyzer claims, mutation execution,
or score definitions.

## Fixture-First Rule

After the audit lands, pick the highest-value real evidence-quality failure
modes and fixture them before analyzer changes. Candidate fixture classes:

- duplicate canonical gap overcount;
- wrong related-test top choice;
- missing equality boundary with nearby exact-value test;
- broad error oracle treated as stronger than it is;
- opaque helper that should remain a static limitation;
- cross-file constant that should remain unresolved;
- side-effect observer that should be recognized;
- runtime signal with ambiguous join.

Each fixture should state what RIPR should claim, what it should leave
unknown, and what must not be inferred.

## Calibration Rule

The existing checked `runtime-fixtures-v1` classes define the calibrated
boundary for imported static/runtime confidence labels. Side-effect observer,
mock expectation, snapshot oracle, and dynamic or opaque dispatch samples stay
outside calibrated scope until checked runtime fixtures land.

When expanding calibration:

- map imported runtime outcomes to existing static seams where possible;
- keep ambiguous joins ambiguous;
- do not create a static gap from runtime-only signal;
- keep static vocabulary within RIPR's conservative terms;
- do not run mutation execution in CI.

## Closeout Conditions

Close this campaign only after:

- an evidence audit exists, or evidence-health has equivalent fields;
- top evidence-quality failure modes are fixture-pinned;
- at least one high-value analyzer or calibration improvement lands from the
  audit;
- capability matrix updates remain honest;
- future work is listed by evidence class rather than downstream surface.
