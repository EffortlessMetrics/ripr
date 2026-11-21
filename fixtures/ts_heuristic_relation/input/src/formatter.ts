export function formatCurrency(amount: number, currency: string): string {
    if (amount < 0) {
        return `-${currency}${Math.abs(amount).toFixed(2)}`;
    }
    return `${currency}${amount.toFixed(2)}`;
}
