# Rust repair attempts corpus

The trust/adoption corpus is the governed input at
`metrics/rust-repair-trust/corpus.json`. Its JSON shape is defined by
[`schemas/ripr/rust-repair-trust-corpus.schema.json`](../../schemas/ripr/rust-repair-trust-corpus.schema.json)
and its report is generated with:

```text
cargo xtask rust-repair-trust-report
```

The command writes `target/ripr/reports/rust-repair-trust.json` and
`target/ripr/reports/rust-repair-trust.md`. It is a report-only proof executor;
it does not edit consumer repositories, generate tests, run mutation testing,
or infer missing route facts.

## Authorization

An authorized repository entry must name the repository, authorization
reference, exact authorized revision or branch, allowed artifact paths,
permitted analysis actions, write policy, and authorization date. Local
presence or organizational ownership alone is not authorization. The current
internal adopting test surface is explicitly recorded in the corpus for
`EffortlessMetrics/ripr-swarm`, `EffortlessMetrics/perl-lsp-swarm`, and
`EffortlessMetrics/ub-review`.

## Counted attempt

Each counted attempt must bind one changed behavior to one exact current-head
receipt and include:

- `attempt_id`, repository, and forty-character `analyzed_head_sha`;
- canonical gap and seam identity plus `file_line`;
- changed behavior, missing discriminator, related caller/test, and focused
  test intent;
- before and after receipts;
- test-only repair intent, changed test files, allowed edit surface, and
  explicit production-file exclusion;
- verification, targeted-rerun, receipt, and inspection commands with results;
- closed movement, limitations, source references, and claim boundary.

Only `closed`, `improved`, `unchanged`, `regressed`, and `limited` are valid
movement outcomes. Duplicate attempt IDs, unauthorized repositories, invalid
revisions, missing route fields, production edits, and malformed receipts are
excluded from the eligible denominator and retained as validation errors.

Synthetic fixtures may exercise the validator but must not be loaded as
real-repository attempts. Fewer than three authorized repositories or twenty
eligible attempts keeps the report `limited` with explicit denominators.
