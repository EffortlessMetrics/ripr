export const DISCOUNT_THRESHOLD = 10000;

export function discountedTotal(amount: number): number {
  if (amount >= DISCOUNT_THRESHOLD) {
    return amount - Math.floor(amount / 10);
  }
  return amount;
}
