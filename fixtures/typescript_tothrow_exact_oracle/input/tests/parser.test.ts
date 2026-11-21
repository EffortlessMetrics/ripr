import { parseUser, ParseError } from "../src/parser";

// UPGRADE: exact string payload -- discriminates the exact error message.
// Promoted to ExactErrorVariant / strong oracle.
test("empty user throws exact message", () => {
    expect(() => parseUser("")).toThrow("empty user");
});

// UPGRADE: exact object payload -- discriminates the error code field.
// Promoted to ExactErrorVariant / strong oracle.
test("empty user throws with error code", () => {
    expect(() => parseUser("")).toThrow({ code: "EMPTY_INPUT" });
});

// UPGRADE: class reference payload -- discriminates the error constructor.
// Promoted to ExactErrorVariant / strong oracle.
test("empty user throws ParseError class", () => {
    expect(() => parseUser("")).toThrow(ParseError);
});
