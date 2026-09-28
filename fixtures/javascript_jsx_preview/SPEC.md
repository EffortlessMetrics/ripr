# Fixture: javascript_jsx_preview

Spec: RIPR-SPEC-0027

## Given

A JavaScript JSX `.jsx` production module changes a predicate from
`amount > threshold` to `amount >= threshold`.

The fixture workspace enables the TypeScript-family preview adapter:

```toml
[languages]
enabled = ["rust", "typescript"]
```

## When

```bash
cargo xtask fixtures javascript_jsx_preview
```

or:

```bash
ripr check \
  --root fixtures/javascript_jsx_preview/input \
  --diff fixtures/javascript_jsx_preview/diff.patch \
  --mode fast
```

## Then

The TypeScript-family adapter parses `.jsx` source and `.test.jsx` tests while
labeling the finding as `language = "javascript"` and
`language_status = "preview"`.

## Must Not

- Treat JSX parsing as React semantic analysis.
- Relabel JavaScript findings as TypeScript.
- Promote preview evidence into gates, badges, baselines, or RIPR Zero.
