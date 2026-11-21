export function checkStock(quantity: number, minimum: number): boolean {
    if (quantity > minimum) {
        return true;
    }
    return false;
}
