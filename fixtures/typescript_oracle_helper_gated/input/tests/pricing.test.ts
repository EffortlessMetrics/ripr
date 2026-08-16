import { computePrice } from "../src/pricing";

function assertPriceBoundary(received: number, floor: number): void {
  if (received < floor) {
    throw new Error(`expected ${received} to be at least ${floor}`);
  }
}

test("price is checked through helper", () => {
  assertPriceBoundary(computePrice(10, 3), 20);
});
