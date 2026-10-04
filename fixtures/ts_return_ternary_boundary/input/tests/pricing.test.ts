import { discount } from "../src/pricing";

test("large orders get ten percent off", () => {
    expect(discount(500)).toBe(450);
});
