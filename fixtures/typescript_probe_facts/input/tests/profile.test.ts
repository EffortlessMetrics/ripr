test("updateProfile smoke", () => {
    const profile = { status: "pending" };
    const audit = { record: (_value: string) => {} };
    const result = updateProfile(profile, "ready", audit);
    expect(result).toBeTruthy();
});
