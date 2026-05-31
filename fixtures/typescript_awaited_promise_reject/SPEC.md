# Fixture: typescript_awaited_promise_reject

Spec: RIPR-SPEC-0027

## Given

A TypeScript production module has two syntactic Promise rejection paths:

```ts
return await Promise.reject(new Error("missing id"));
await Promise.reject(new Error("missing id"));
```

The fixture workspace enables the TypeScript preview adapter via
`ripr.toml`:

```toml
[languages]
enabled = ["rust", "typescript"]
```

Related Jest-style tests reference both changed owners and use syntactic
`await expect(...).rejects.toThrow(...)` assertions. The bare matcher is
intentionally broad error evidence in the preview adapter; a literal payload
matcher is exact error-payload evidence while still remaining preview/advisory
and non-actionable without the full repair-packet contract.

## When

```bash
cargo xtask fixtures typescript_awaited_promise_reject
```

or:

```bash
ripr check \
  --root fixtures/typescript_awaited_promise_reject/input \
  --diff fixtures/typescript_awaited_promise_reject/diff.patch \
  --mode fast
```

## Then

The TypeScript preview adapter:

- finds the `loadProfile` and `publishProfile` owners in `src/profile.ts`,
- finds related tests in `tests/profile.test.ts`,
- classifies both awaited `Promise.reject(...)` changed lines as
  `probe.family = error_path` with `probe.delta = control`,
- preserves preview metadata with `language = "typescript"` and
  `language_status = "preview"`,
- keeps bare `rejects.toThrow()` broad and weak,
- classifies literal `rejects.toThrow("...")` as exact error-variant evidence.

## Must Not

- Report `return await Promise.reject(...)` as `return_value`.
- Report bare `await Promise.reject(...)` as `side_effect`.
- Claim JavaScript runtime execution or Rust-level maturity for the
  TypeScript preview evidence.
- Change editor routing, VS Code selectors, policy, gates, or defaults.
