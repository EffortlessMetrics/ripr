export function discountedTotal(amount: number, threshold: number, discount: number): number {
    if (amount >= threshold) {
        return amount - discount;
    }
    return amount;
}
