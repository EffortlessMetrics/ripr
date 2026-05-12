// Jest-style test asserts the result with an exact-value matcher,
// but the test file ALSO mocks the `./api` module via `vi.mock(...)`.
// The TypeScript preview adapter has no module resolver, so it cannot
// know what the mock substitutes for the changed code. The adapter
// surfaces this honestly as the `mocked_module` static-limit (#769)
// while still classifying the strong `toBe` oracle.
import { vi } from "vitest";

vi.mock("./api");

test("at threshold discounts", () => {
    expect(applyDiscount(100, 100)).toBe(90);
});
