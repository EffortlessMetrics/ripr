export function computePrice(base: number, multiplier: number): number {
    if (base > 0) {
        return base * multiplier;
    }
    return 0;
}
