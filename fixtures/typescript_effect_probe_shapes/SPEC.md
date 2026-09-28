# Fixture: typescript_effect_probe_shapes

Spec: RIPR-SPEC-0027

## Given

A TypeScript production function changes both a property assignment and a
call-expression statement:

```ts
profile.status = status;
audit.record(status);
```

The fixture workspace enables the TypeScript preview adapter via
`ripr.toml`:

```toml
[languages]
enabled = ["rust", "typescript"]
```

A related test references `updateProfile(` but does not expose a structured
`expect(...).matcher(...)` assertion. That keeps the fixture focused on
public probe-family projection rather than oracle precision.

## When

```bash
cargo xtask fixtures typescript_effect_probe_shapes
```

or:

```bash
ripr check \
  --root fixtures/typescript_effect_probe_shapes/input \
  --diff fixtures/typescript_effect_probe_shapes/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- finds the `updateProfile` owner in `src/profile.ts`,
- finds the related test in `tests/profile.test.ts`,
- classifies the property assignment as `probe.family = field_construction`
  with `probe.delta = value`,
- classifies the call-expression statement as `probe.family = side_effect`
  with `probe.delta = effect`,
- preserves preview metadata with `language = "typescript"` and
  `language_status = "preview"`.

## Must Not

- Collapse `profile.status = status;` to the generic `predicate` fallback.
- Collapse `audit.record(status);` to the generic `predicate` fallback.
- Claim runtime execution, generated tests, provider calls, or Rust-level
  maturity for the TypeScript preview evidence.
- Change editor routing, VS Code selectors, policy, gates, or defaults.
