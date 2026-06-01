# Fixture: python_same_line_duplicate_collapse

Spec: RIPR-SPEC-0028

## Given

A Python function changes one returned dict line that contains a return
boundary, field construction, and string literals, and a pytest test only
checks that the payload exists.

The fixture workspace enables the Python preview adapter explicitly:

```toml
[languages]
enabled = ["rust", "python"]
```

## When

```bash
cargo xtask fixtures python_same_line_duplicate_collapse
```

or:

```bash
ripr check \
  --root fixtures/python_same_line_duplicate_collapse/input \
  --diff fixtures/python_same_line_duplicate_collapse/diff.patch \
  --mode fast
```

## Then

The Python preview adapter:

- finds the `checkout_payload` function owner,
- collapses the same-line return/dict/string signals into one user-facing
  canonical repair gap,
- classifies the line as a field/object repair shape,
- keeps the broad pytest assertion weak, and
- emits the concrete missing discriminator `status == "paid"` with a
  returned-mapping assertion shape such as `assert result["status"] == "paid"`.

## Must Not

- Emit separate user-facing return-value, field-value, and string-literal
  findings for the same changed Python line.
- Inflate `canonical_gap_group_size` above one for this single repairable
  behavior.
- Execute pytest or resolve runtime dict semantics.
