test("dispatch smoke", () => {
    dispatch({ ready: () => undefined }, "ready");
    expect(true).toBeTruthy();
});
