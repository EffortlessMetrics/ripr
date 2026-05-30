test("PriceBadge smoke", () => {
    expect(PriceBadge({ label: "total", amount: 42 })).toBeTruthy();
});
