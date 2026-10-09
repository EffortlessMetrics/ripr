export function shipping(amount: number): number {
  if (amount === 5000) return 0;
  if (amount > 5000) {
    return 1;
  }
  return 2;
}
