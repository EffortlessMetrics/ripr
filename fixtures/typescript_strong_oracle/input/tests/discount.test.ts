// Jest-style test that exercises the changed predicate with an
// exact-value assertion. The TypeScript preview adapter recognizes the
// `expect(...).toBe(...)` shape (issue #767) and classifies the
// changed line as `exposed`.
test("at threshold discounts", () => {
    expect(applyDiscount(100, 100)).toBe(90);
});

test("above threshold discounts", () => {
    expect(applyDiscount(10000, 100)).toBe(9990);
});
