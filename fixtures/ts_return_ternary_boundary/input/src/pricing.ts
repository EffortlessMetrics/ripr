export function discount(total: number): number {
    return total > 100 ? total * 0.9 : total;
}
