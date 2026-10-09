export function discount(amount: number, threshold: number): number {
  threshold = Math.max(threshold, 1);
  if (amount >= threshold) {
    return Math.floor(amount / 10);
  }
  return 0;
}
