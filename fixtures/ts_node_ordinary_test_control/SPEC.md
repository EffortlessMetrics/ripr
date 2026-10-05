# Fixture: ts_node_ordinary_test_control

Spec: RIPR-SPEC-0027
Spec: RIPR-SPEC-0108

## Given

The same change as `ts_node_expect_failure_no_credit`, with an ordinary Node
test and no expected-failure option:

```ts
test("scores the difference", () => {
    assert.strictEqual(score(10, 3), 7);
});
```

## When

```bash
cargo xtask fixtures ts_node_ordinary_test_control
```

## Then

The ordinary test's `assert.strictEqual(...)` is an exact-value oracle and the
finding is `exposed`, advisory-only and not packet-ready.

## Must Not

- Withhold credit from an ordinary active test because a sibling fixture uses
  `expectFailure`.
- Emit a repair packet for an already observed preview finding.
