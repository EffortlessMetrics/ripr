export function computePrice(base: number, factor: number): number {
    if (base > 0) {
        return base * factor;
    }
    return 0;
}
