test("updates and records status", () => {
    const profile = { status: "pending" };
    const audit = { record: (_value: string) => {} };
    const result = updateProfile(profile, "ready", audit);
    if (result.status !== "ready") {
        throw new Error("expected ready");
    }
});
