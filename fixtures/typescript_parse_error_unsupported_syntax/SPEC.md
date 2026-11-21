# Fixture: typescript_parse_error_unsupported_syntax

Spec: RIPR-SPEC-0027

## Given

A TypeScript production file contains malformed syntax on the changed line.

The fixture workspace enables the TypeScript preview adapter:

```toml
[languages]
enabled = ["rust", "typescript"]
```

## When

```bash
cargo xtask fixtures typescript_parse_error_unsupported_syntax
```

or:

```bash
ripr check \
  --root fixtures/typescript_parse_error_unsupported_syntax/input \
  --diff fixtures/typescript_parse_error_unsupported_syntax/diff.patch \
  --mode fast
```

## Then

The adapter emits one preview static limitation with
`static_limit_kind = "unsupported_syntax"` instead of silently dropping the
changed file or emitting an actionable repair packet.

## Must Not

- Claim an owner, oracle, or repair route when syntax facts cannot be built.
- Treat parse failure as a user-test adequacy verdict.
- Promote preview evidence into gates, badges, baselines, or RIPR Zero.
