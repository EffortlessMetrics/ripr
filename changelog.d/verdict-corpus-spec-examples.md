<!-- section: Added -->
- Verdict corpus: 43 authored cases turn the acceptance examples of
  RIPR-SPEC-0225 to RIPR-SPEC-0228 into runtime-labeled rows, one isolated
  crate per example. Whole-value `assert_eq!`: 7 discriminated cases read as
  gaps (4 are the literal shapes the spec credits), and a tautological
  `retries: c.retries` reads `exposed`. Result-side oracles: an
  `is_err`/`is_ok` boundary test and the rule 3b `?` side flip read as gaps. Field writes:
  9 of 10 read `static_unknown`. Corpus 2026-10-04.7, 147 cases.
