export function applyDiscount(amount: number, threshold: number): number {
    if (amount >= threshold) {
        return 0.9;
    }
    return 1;
}
