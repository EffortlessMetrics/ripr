export const DISCOUNT_THRESHOLD = 10000;

export function discountedTotal(amount: number): number {
  if (DISCOUNT_THRESHOLD < amount) {
    return amount - Math.floor(amount / 10);
  }
  return amount;
}
