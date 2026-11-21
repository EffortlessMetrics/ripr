import { applyDiscount as subject } from "../src/pricing";
import * as pricing from "../src/pricing";
import { applyDiscount as otherSubject } from "../src/other-pricing";
import { applyDiscount } from "../src/other-pricing";
import type { applyDiscount as typeOnlySubject } from "../src/pricing";

test("alias import observes threshold", () => {
    expect(subject(100, 100)).toBe(90);
});

test("namespace import observes threshold", () => {
    expect(pricing.applyDiscount(100, 100)).toBe(90);
});

test("wrong import path does not relate", () => {
    expect(otherSubject(100, 100)).toBe(90);
});

test("wrong direct import path does not relate", () => {
    expect(applyDiscount(100, 100)).toBe(90);
});

test("type only import does not relate", () => {
    expect(typeOnlySubject(100, 100)).toBe(90);
});

test("object method does not relate", () => {
    const order = { applyDiscount: () => 90 };
    expect(order.applyDiscount(100)).toBe(90);
});

test("string and comment mentions do not relate", () => {
    // applyDiscount(100, 100)
    expect("applyDiscount(").toContain("applyDiscount(");
});
