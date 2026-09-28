import { applyDiscount } from "../src/discount";

test("second arg carries 100 while total is 150", () => {
    expect(applyDiscount(150, 100)).toBe(135);
});
