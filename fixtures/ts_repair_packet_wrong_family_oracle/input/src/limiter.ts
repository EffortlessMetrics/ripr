export function parseLimit(raw: string): number {
    const n = Number(raw);
    if (Number.isNaN(n)) {
        throw new TypeError('invalid limit');
    }
    return n;
}
