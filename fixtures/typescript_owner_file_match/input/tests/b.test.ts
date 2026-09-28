import { betaScore } from "../src/b";

test("beta keeps its threshold", () => {
    expect(betaScore(12)).toBe(13);
});
