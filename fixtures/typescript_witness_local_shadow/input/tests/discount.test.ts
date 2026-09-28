import { applyDiscount } from "../src/discount";

test("local shadow at boundary", () => {
    function applyDiscount(total: number): number {
        return 42;
    }
    expect(applyDiscount(100)).toBe(42);
});
