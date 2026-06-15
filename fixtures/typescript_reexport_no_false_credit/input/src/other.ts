/** A DIFFERENT function that happens to share the same name as an export. */
export function isRawNetworkError(error: unknown): boolean {
    // This is a completely unrelated implementation in a different module.
    return typeof error === 'string' && error.startsWith('NETWORK');
}
