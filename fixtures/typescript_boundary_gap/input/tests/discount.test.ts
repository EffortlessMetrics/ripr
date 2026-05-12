// Jest-style test. The TypeScript preview adapter does not yet
// extract assertion shape (see issue #767); the related-test match is
// by call-name reference within the test body.
test("below threshold has no discount", () => {
    const result = applyDiscount(50, 100);
    if (result !== 50) {
        throw new Error("expected 50");
    }
});

test("far above threshold discounts", () => {
    const result = applyDiscount(10000, 100);
    if (result !== 9990) {
        throw new Error("expected 9990");
    }
});
