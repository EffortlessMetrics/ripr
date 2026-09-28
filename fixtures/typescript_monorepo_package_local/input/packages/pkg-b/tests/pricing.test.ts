import { computePrice } from "../src/pricing";

// This test is in pkg-b. It mentions applyDiscount in its body
// but has no import resolving to pkg-a. With package-local enforcement,
// this test must NOT be selected as a related test for pkg-a's applyDiscount.
test("computes price in pkg-b", () => {
    // Simulate a call that looks like applyDiscount but is in a different package.
    const applyDiscount = (price: number, pct: number) => price * (1 - pct / 100);
    expect(applyDiscount(100, 10)).toBe(90);
    expect(computePrice(50, 10)).toBe(60);
});
