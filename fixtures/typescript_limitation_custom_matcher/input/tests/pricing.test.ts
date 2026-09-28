// This test uses a custom Jest matcher `toBeWithinRange` that is NOT in the
// recognized matcher set in oracle.rs. The adapter sees the matcher via the
// oxc-parsed `expect(x).toBeWithinRange(...)` call expression but returns
// OracleKind::Unknown because the matcher is unrecognised. This is a real
// producer for `typescript_custom_matcher_unresolved`.
import { computePrice } from "../src/pricing";

expect.extend({
  toBeWithinRange(received: number, floor: number, ceiling: number) {
    const pass = received >= floor && received <= ceiling;
    return { message: () => `expected ${received} within [${floor}, ${ceiling}]`, pass };
  },
});

test("price is in expected range", () => {
  const price = computePrice(10, 3);
  expect(price).toBeWithinRange(20, 40);
});
