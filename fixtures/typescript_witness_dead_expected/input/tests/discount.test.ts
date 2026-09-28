import { applyDiscount } from "../src/discount";

test("wrong expected", () => {
    expect(applyDiscount(100)).toBe(999);
});
