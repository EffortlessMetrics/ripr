import { score } from "../src/score";

test("unknown t method mentions the result", t => {
    t.frobnicate(score(10, 3), 7);
});
