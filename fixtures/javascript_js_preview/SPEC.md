# Fixture: javascript_js_preview

Spec: RIPR-SPEC-0027

## Given

A JavaScript `.js` production module changes a predicate from
`amount > threshold` to `amount >= threshold`.

The fixture workspace enables the TypeScript-family preview adapter:

```toml
[languages]
enabled = ["rust", "typescript"]
```

## When

```bash
cargo xtask fixtures javascript_js_preview
```

or:

```bash
ripr check \
  --root fixtures/javascript_js_preview/input \
  --diff fixtures/javascript_js_preview/diff.patch \
  --mode fast
```

## Then

The TypeScript-family adapter parses `.js` source and `.test.js` tests while
labeling the finding as `language = "javascript"` and
`language_status = "preview"`.

## Must Not

- Relabel JavaScript findings as TypeScript.
- Claim package graph or runtime test execution.
- Promote preview evidence into gates, badges, baselines, or RIPR Zero.
