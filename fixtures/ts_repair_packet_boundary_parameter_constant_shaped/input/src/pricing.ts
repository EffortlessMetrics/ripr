export function discount(amount: number, LIMIT: number): number {
  if (amount >= LIMIT) {
    return Math.floor(amount / 10);
  }
  return 0;
}
