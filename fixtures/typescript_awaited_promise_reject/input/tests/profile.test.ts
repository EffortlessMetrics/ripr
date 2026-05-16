test("rejects missing profile id", () => {
    expect(() => loadProfile("")).toThrow("missing id");
});

test("rejects missing publish id", () => {
    expect(() => publishProfile("", { publish: async () => {} })).toThrow("missing id");
});
