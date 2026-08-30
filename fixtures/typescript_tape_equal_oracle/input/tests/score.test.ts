import { score } from "../src/score";

test("scores the difference", t => {
    const result = score(10, 3);
    t.equal(result, 7);
});
