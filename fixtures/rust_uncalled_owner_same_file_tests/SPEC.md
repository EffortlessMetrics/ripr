# Fixture: rust_uncalled_owner_same_file_tests

Spec: RIPR-SPEC-0001

## Given

`untested_rounding` changes its rounding constant (`+ 50` → `+ 49`). No test
calls it. The file's inline `#[cfg(test)]` module holds two strong tests for a
different function, `discount`, including `assert_eq!(discount(100), 90)`.

## When

```bash
ripr check \
  --root fixtures/rust_uncalled_owner_same_file_tests/input \
  --diff fixtures/rust_uncalled_owner_same_file_tests/diff.patch
```

## Then

- The inline tests stay listed as related (`same_test_file`), as the likely
  place to add a test.
- Reach is `weak`: no test is seen calling `untested_rounding`.
- The finding is not `exposed`.

## Must Not

- Report `exposed` for `untested_rounding` because a neighbouring test in the
  same file has a strong assertion. Before this fixture it was reported
  `exposed` with confidence 1.00.
