import { clamp } from "../src/clamp";

// The matcher argument is a non-literal variable: `expectedMin`.
// This triggers typescript_dynamic_assertion_unresolved because the adapter
// cannot statically resolve the expected value from a local variable.
test("clamps below minimum", () => {
    const expectedMin = computeExpected(0);
    expect(clamp(-5, 0, 10)).toBe(expectedMin);
});

function computeExpected(base: number): number {
    return base;
}
