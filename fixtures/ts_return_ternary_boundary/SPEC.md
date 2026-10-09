# Fixture: ts_return_ternary_boundary

Spec: RIPR-SPEC-0027

## Given

A TypeScript owner `discount(total)` returns a conditional expression, and the
diff changes its condition from `total >= 100` to `total > 100`:

```ts
return total > 100 ? total * 0.9 : total;
```

The only related test calls `discount(500)` and asserts `toBe(450)`. Both the
old and new condition send 500 down the discount arm, so the test cannot see
the change. Only `total == 100` separates the two versions.

## When

```bash
ripr check \
  --root fixtures/ts_return_ternary_boundary/input \
  --diff fixtures/ts_return_ternary_boundary/diff.patch
```

## Then

- The changed line is a `predicate` probe whose boundary is the ternary's
  condition, `total == 100`, as it is for Python's conditional expression and
  for the equivalent `if (total > 100) { ... }`.
- The finding stays `weakly_exposed` and names `total == 100` as the missing
  discriminator.

## Must Not

- Classify the line as `return_value` because it starts with `return`, and so
  credit any exact return-value oracle as observing the changed condition.
- Report `exposed` or "no repair to make" from an input that lies on one side
  of the boundary.
