// Adversarial owner-module mock trap: the test mocks the CHANGED OWNER'S OWN
// module (`../src/discount`), stubs the mocked `applyDiscount` to return 90,
// and asserts an exact value. The owner call executes the mock, not the
// changed code, so the strong oracle cannot observe the changed sink.
import { applyDiscount } from "../src/discount";

jest.mock("../src/discount");

const mockedApplyDiscount = applyDiscount as jest.Mock;
mockedApplyDiscount.mockReturnValue(90);

test("applyDiscount at threshold discounts", () => {
    const result = applyDiscount(100, 100);
    expect(result).toBe(90);
});
