export function applyDiscount(total: number): number {
    if (total >= 100) {
        return total * 0.9;
    }
    return total;
}
