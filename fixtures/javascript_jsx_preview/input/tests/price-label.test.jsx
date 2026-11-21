test("renders discounted price", () => {
  expect(PriceLabel({ amount: 100, threshold: 100 })).toEqual(<span>{90}</span>);
});
