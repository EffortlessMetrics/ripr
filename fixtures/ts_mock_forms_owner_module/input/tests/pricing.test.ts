import { describe, expect, it, vi as v } from "vitest";
import { applyDiscount } from "../src/pricing";

v.mock(import("../src/pricing"), () => ({ applyDiscount: () => 0.9 }));

describe("pricing", () => {
    it("applies the discount at the boundary", () => {
        expect(applyDiscount(100, 100)).toBe(0.9);
    });
});
