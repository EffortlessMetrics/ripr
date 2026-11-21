import { applyDiscount } from "../src/discount";

// This test is in pkg-a and imports from pkg-a — it IS related.
test("applies discount within pkg-a", () => {
    expect(applyDiscount(100, 20)).toBe(80);
});
