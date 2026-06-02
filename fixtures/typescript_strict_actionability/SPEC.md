# Fixture: typescript_strict_actionability

Spec: RIPR-SPEC-0061

## Given

A TypeScript preview workspace opts into the TypeScript-family adapter and
contains three changed owners:

- a weakly observed predicate with a direct Jest owner call and only a smoke
  assertion;
- an already observed return value with a strong exact assertion;
- a changed owner with no trusted related test.

## When

```bash
cargo xtask fixtures typescript_strict_actionability
```

or:

```bash
ripr check \
  --root fixtures/typescript_strict_actionability/input \
  --diff fixtures/typescript_strict_actionability/diff.patch \
  --mode fast
```

## Then

The TypeScript preview findings stay advisory and fail closed:

- weak direct evidence emits `gap_state: advisory` with
  `actionability_category: incomplete_repair_packet`;
- strong related evidence emits `gap_state: already_observed`;
- missing related evidence emits `gap_state: advisory` with
  `actionability_category: missing_context`;
- each finding carries `why_not_actionable`, `repair_route`, and a raw
  preview evidence reference.

## Must Not

- Emit a TypeScript repair packet.
- Claim default gate, badge, baseline, or RIPR Zero authority.
- Invoke `tsc`, `tsserver`, Jest, Vitest, provider calls, generated tests, or
  source edits.
