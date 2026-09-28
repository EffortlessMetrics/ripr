export const DISCOUNT_THRESHOLD = 100;

export function applyDiscount(total: number): number {
    if (total >= DISCOUNT_THRESHOLD) {
        return 0.9;
    }
    return 1;
}

export function freeShipping(items: number): boolean {
    if (items >= 5) {
        return true;
    }
    return false;
}
