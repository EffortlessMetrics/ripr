# Fixture: python_substring_sink_alignment (false-exposed regression guard)

Spec: RIPR-SPEC-0028

## Given

A Python predicate boundary changes (`<` -> `<=`) on a line that mentions a
common token (`buffer`), and a pytest test calls the changed owner with a strong
exact-value oracle whose text contains that token only as a **substring of a
different identifier** (`buffered_output`), and which does **not** exercise the
`len == limit` boundary the change moves:

```python
# src/pack.py            (owner: pack)
if len(buffer) <= limit:     # changed from `<`

# tests/test_pack.py
buffered_output = pack([1, 2], 5)
assert buffered_output == [1, 2]
```

The fixture workspace enables the Python preview adapter (`input/ripr.toml`).

## When

`ripr check` analyzes the diff against the Python preview adapter.

## Then

ripr classifies the change `weakly_exposed` with `oracle_alignment: orthogonal`
(`strong_oracle_observes_different_sink`). The strong oracle is linked (the test
calls `pack` directly) but does **not** observe the changed sink: `pack([1,2], 5)`
never reaches `len == limit`, so the assertion passes identically with `<` or
`<=`.

This pins the fix for a **confirmed false-exposed** (the dangerous over-credit
direction). Before identifier-boundary matching in `classify_sink_alignment`, the
changed-sink token `buffer` matched the substring inside `buffered_output`, and
ripr credited `oracle_alignment: changed_sink_token` -> `exposed` on a coincidental
co-occurrence — crediting proximity as discrimination, the "drift back to coverage"
the static model warns against. Whole-word tokens (e.g. `key` in `Invalid key`)
still observe, so genuine sink alignment is preserved.

## Must Not

- Credit `exposed` when the only token match is a substring of an unrelated
  identifier (no coincidental over-credit).
- Run any Python runtime; static preview evidence only.
