export function shipping(amount: number): number {
  if (amount >= 5000) {
    return 0;
  }
  return 500;
}
