test("renders discounted price", () => {
  expect(PriceTag({ amount: 100, threshold: 50 })).toEqual(<span>{90}</span>);
});
