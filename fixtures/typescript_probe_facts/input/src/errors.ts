export function parseToken(token: string): string {
    if (!token) {
        throw new RangeError("missing token");
    }
    return token;
}
