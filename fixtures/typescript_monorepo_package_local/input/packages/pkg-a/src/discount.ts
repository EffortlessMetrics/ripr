export function applyDiscount(price: number, discountPct: number): number {
    if (discountPct >= 100) {
        return 0;
    }
    return price * (1 - discountPct / 100);
}
