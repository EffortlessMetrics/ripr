export function discountedTotal(amount: number, threshold: number): number {
    if (amount >= threshold) {
        return amount - 10;
    }
    return amount;
}
