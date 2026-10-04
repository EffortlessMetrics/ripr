export function applyDiscount(total: number): number {
    if (total >= 100) {
        return 0.9;
    }
    return 1;
}
