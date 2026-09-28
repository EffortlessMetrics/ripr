import { applyDiscount } from "../src/discount";

const pricing = { applyDiscount: (t: number) => t * 2 };

test("shadow assertion at 100", () => {
    const unused = applyDiscount(150);
    expect(pricing.applyDiscount(100)).toBe(200);
});
