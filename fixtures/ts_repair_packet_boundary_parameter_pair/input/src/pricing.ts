export function discount(amount: number, threshold: number): number {
  if (amount >= threshold) {
    return Math.floor(amount / 10);
  }
  return 0;
}
