export function applyDiscount(amount: number, threshold: number): number {
    if (amount >= threshold) {
        return amount * 0.9;
    }
    return amount;
}
