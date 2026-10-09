# Fixture: ts_return_comparison_boundary

Spec: RIPR-SPEC-0027

## Given

A TypeScript owner `isLarge(total)` returns a relational comparison, and the diff
changes its operator from `>=` to `>`:

```
return total > 100;
```

The only related test asserts `expect(isLarge(500)).toBe(true)`. Both versions return true for
500, so the test cannot see the change. Only `total == 100` separates them.

## When

```bash
ripr check \
  --root fixtures/ts_return_comparison_boundary/input \
  --diff fixtures/ts_return_comparison_boundary/diff.patch
```

## Then

- The changed line is a `predicate` probe with the boundary `total == 100`,
  the same as `if total > 100` and the Rust tail expression `total > 100`.
- The finding stays `weakly_exposed` and names `total == 100` as the missing
  discriminator. A test calling `isLarge(100)` would credit it.

## Must Not

- Classify the line as `return_value` and credit any exact return-value
  oracle as observing the changed operator.
- Report `exposed` from an input on one side of the boundary.
