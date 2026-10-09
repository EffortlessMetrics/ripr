import { isLarge } from "../src/pricing";

test("large orders", () => {
    expect(isLarge(500)).toBe(true);
});
