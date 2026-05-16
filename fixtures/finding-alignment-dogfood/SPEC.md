# Fixture Corpus: finding-alignment-dogfood

Spec: RIPR-SPEC-0045

## Given

RIPR has checked finding-alignment examples from real RIPR PR work. Each
example records raw finding counts, canonical item counts, evidence class,
gap state, actionability, repair or limitation route, verify command, and
must-not-claim guards.

## When

```bash
cargo xtask dogfood
cargo xtask check-fixture-contracts
```

## Then

The corpus pins the Lane 1 counting model:

- raw findings are supporting evidence;
- canonical evidence items are the countable unit;
- actionable canonical gaps are user work;
- already-observed and internal-only items are no-action;
- static limitations name analyzer repair routes.

## Must Not

- Infer actionability from a raw static class.
- Treat internal policy or config metadata as user test debt.
- Hide raw findings.
- Change PR/CI rendering, LSP/editor behavior, gates, public scores, source
  edits, generated tests, provider calls, or mutation execution.
