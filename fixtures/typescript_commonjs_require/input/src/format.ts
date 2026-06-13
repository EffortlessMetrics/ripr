export function formatAmount(value: number, decimals: number): string {
    if (decimals < 0) {
        return value.toFixed(0);
    }
    return value.toFixed(decimals);
}
