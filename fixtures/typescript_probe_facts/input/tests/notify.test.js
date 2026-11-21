test("notify smoke", () => {
    const mockSend = () => {};
    expect(notify({ id: 1 }, mockSend)).toBeTruthy();
});
