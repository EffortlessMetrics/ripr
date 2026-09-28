export class ParseError extends Error {
    constructor(
        public readonly code: string,
        message: string,
    ) {
        super(message);
    }
}

export function parseUser(input: string): string {
    if (input.trim() === "") {
        throw new ParseError("EMPTY_INPUT", "empty user");
    }
    return input.trim();
}
