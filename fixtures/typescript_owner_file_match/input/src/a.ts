export function alphaScore(value: number): number {
    if (value > 10) {
        return value + 1;
    }
    return value;
}
