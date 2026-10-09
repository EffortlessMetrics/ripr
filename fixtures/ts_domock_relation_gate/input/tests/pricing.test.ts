import { applyDiscount } from "../src/pricing";

jest.doMock("../src/pricing");

describe("pricing", () => {
    it("applies the discount at the boundary", () => {
        expect(applyDiscount(100, 100)).toBe(90);
    });
});
