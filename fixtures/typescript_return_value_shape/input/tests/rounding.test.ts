// A related test exists (matches by `roundCents(` reference) but uses
// no `expect(...).matcher(...)` chain, so the strongest extracted
// oracle is `unknown`. With the probe-shape classifier (#768) the
// finding's probe family is `return_value` instead of the default
// `predicate`.
test("rounds two decimals", () => {
    const result = roundCents(1.005);
    if (result !== 1.01) {
        throw new Error("expected 1.01");
    }
});
