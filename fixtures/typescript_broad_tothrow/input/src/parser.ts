export function parseUser(input: string): string {
    if (input.trim() === "") {
        throw new Error("empty user");
    }
    return input.trim();
}
