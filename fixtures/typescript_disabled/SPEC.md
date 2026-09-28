# Fixture: typescript_disabled

Spec: RIPR-SPEC-0027

## Given

A TypeScript production diff exists, but the fixture keeps the default Rust-only
language configuration:

```toml
[languages]
enabled = ["rust"]
```

## When

```bash
cargo xtask fixtures typescript_disabled
```

or:

```bash
ripr check \
  --root fixtures/typescript_disabled/input \
  --diff fixtures/typescript_disabled/diff.patch \
  --mode fast
```

## Then

The TypeScript-family adapter does not run, so no TS/JS diagnostics or preview
findings are emitted.

## Must Not

- Emit TypeScript findings when `typescript` is not enabled.
- Generate advisory preview grouping from disabled TypeScript evidence.
- Promote preview evidence into gates, badges, baselines, or RIPR Zero.
