test("rejects missing profile id broadly", async () => {
    await expect(loadProfile("")).rejects.toThrow();
});

test("rejects missing publish id exactly", async () => {
    await expect(publishProfile("", { publish: async () => {} })).rejects.toThrow("missing id");
});
