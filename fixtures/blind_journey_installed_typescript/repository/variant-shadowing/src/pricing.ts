/** Discounted total applies at and above the discount threshold. */
export const DISCOUNT_THRESHOLD = 10000;

export function discountedTotal(
  subtotal: number,
  threshold: number = DISCOUNT_THRESHOLD,
): number {
  if (subtotal >= threshold) {
    return subtotal - 500;
  }
  return subtotal;
}
