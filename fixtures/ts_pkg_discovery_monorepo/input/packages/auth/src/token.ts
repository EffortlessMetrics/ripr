export function createToken(userId: string): string {
    return `token-${userId}`;
}
