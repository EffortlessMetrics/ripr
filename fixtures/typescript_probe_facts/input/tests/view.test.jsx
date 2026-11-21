test("StatusView smoke", () => {
    expect(StatusView({ status: "ready" })).toBeTruthy();
});
