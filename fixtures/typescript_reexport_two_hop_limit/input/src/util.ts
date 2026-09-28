/** Returns true when the error is a raw network error (no response body). */
export function isRawNetworkError(error: unknown): boolean {
    if (!(error instanceof Error)) {
        return false;
    }
    return error.message.includes('fetch') && !('response' in error);
}
