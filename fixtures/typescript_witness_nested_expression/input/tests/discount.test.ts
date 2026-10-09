import { applyDiscount } from "../src/discount";

test("input is price + 100 with price = 60, i.e. 160, NOT the boundary", () => {
    const price = 60;
    expect(applyDiscount(price + 100)).toBe(144);
});
