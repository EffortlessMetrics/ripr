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
- Reach is `no`: no test is seen calling `untested_rounding`, and no test
  invokes a macro that could hide the call.
- The finding is `no_static_path`, and the neighbour's strong assertion is not
  reported as observing or discriminating the change.

## Must Not

- Report `exposed` for `untested_rounding` because a neighbouring test in the
  same file has a strong assertion. Before this fixture it was reported
  `exposed` with confidence 1.00.
- Report `weakly_exposed`, which says a test reaches the change but
  discriminates it weakly, for a function no test calls.
