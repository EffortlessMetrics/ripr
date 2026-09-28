export function thresholdLabel(amount: number): string {
    if (amount >= 100) {
        return "high";
    }
    return "low";
}
