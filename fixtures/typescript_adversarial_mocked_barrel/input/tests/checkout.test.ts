// Adversarial mocked-barrel trap: the test imports the changed owner through
// the `../src` star barrel and mocks that BARREL, not the owner's own module.
// The call executes the mock, so the strong oracle cannot observe the changed
// sink.
import { applyDiscount } from "../src";

jest.mock("../src", () => ({ applyDiscount: jest.fn(() => 90) }));

test("applyDiscount at threshold discounts", () => {
    const result = applyDiscount(100, 100);
    expect(result).toBe(90);
});
