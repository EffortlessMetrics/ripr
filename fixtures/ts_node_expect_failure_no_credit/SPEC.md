# Fixture: ts_node_expect_failure_no_credit

Spec: RIPR-SPEC-0027
Spec: RIPR-SPEC-0108

## Given

A TypeScript production module changes a return expression in `score(...)`.
The only related test is a Node test registered as expected to fail:

```ts
test("scores the difference", { expectFailure: true }, () => {
    assert.strictEqual(score(10, 3), 7);
});
```

The fixture workspace enables the TypeScript preview adapter through
`ripr.toml`.

## When

```bash
cargo xtask fixtures ts_node_expect_failure_no_credit
```

## Then

The TypeScript preview adapter does not extract the expected-failure test as
ordinary evidence, so its `assert.strictEqual(...)` cannot donate an
exact-value oracle and the finding is not promoted to `exposed`.

## Must Not

- Classify the finding as `exposed` through the expected-failure test.
- Emit a receipt command or claim runtime authority.

The independent control is `ts_node_ordinary_test_control`: the same test
without `expectFailure` stays exposed.
