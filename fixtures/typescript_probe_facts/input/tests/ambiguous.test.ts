test("summarize smoke", () => {
    const handlers = { ready: (_value: number) => {} };
    expect(summarize(100, 90, handlers, "ready")).toBeTruthy();
});
