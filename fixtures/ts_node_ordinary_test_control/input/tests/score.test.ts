import assert from "node:assert/strict";
import { test } from "node:test";
import { score } from "../src/score";

test("scores the difference", () => {
    assert.strictEqual(score(10, 3), 7);
});
