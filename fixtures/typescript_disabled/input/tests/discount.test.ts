test("at threshold discounts", () => {
  expect(applyDiscount(100, 100)).toBe(90);
});
