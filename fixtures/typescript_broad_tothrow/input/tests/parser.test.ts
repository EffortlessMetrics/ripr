import { parseUser } from "../src/parser";

test("empty user throws", () => {
    expect(() => parseUser("")).toThrow();
});
