import { alphaScore } from "../src/a";

test("alpha keeps its threshold", () => {
    expect(alphaScore(12)).toBe(13);
});
