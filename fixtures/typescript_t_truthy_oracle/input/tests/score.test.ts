import { score } from "../src/score";

test("scores a truthy result", t => {
    t.truthy(score(10, 3));
});
