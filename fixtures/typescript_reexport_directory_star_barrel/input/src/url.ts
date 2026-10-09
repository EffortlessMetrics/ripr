/** Unrelated export that the barrel also forwards. */
export function hasProtocol(input: string): boolean {
    return input.includes('://');
}
