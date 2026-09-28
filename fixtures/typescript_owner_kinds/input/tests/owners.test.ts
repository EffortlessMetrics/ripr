import { Cart, DEFAULT_RATE, formatPrice } from "../src/owners";

test("formats price", () => {
  expect(formatPrice(5)).toBe("$5.00");
});

test("cart total smoke", () => {
  const cart = new Cart();
  expect(cart.total()).toBeDefined();
});

test("static build smoke", () => {
  expect(Cart.build()).toBeDefined();
});

test("rate is visible", () => {
  expect(DEFAULT_RATE).toBeGreaterThan(0);
});

