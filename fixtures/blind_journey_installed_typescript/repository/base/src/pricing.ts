/** Discounted total applies only above the discount threshold. */
export const DISCOUNT_THRESHOLD = 10000;

export function discountedTotal(subtotal: number): number {
  if (subtotal > DISCOUNT_THRESHOLD) {
    return subtotal - 500;
  }
  return subtotal;
}
