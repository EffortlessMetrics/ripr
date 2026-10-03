import { DISCOUNT_THRESHOLD } from "./constants";

export function discountedTotal(subtotal: number): number {
  if (subtotal >= DISCOUNT_THRESHOLD) {
    return subtotal - 500;
  }
  return subtotal;
}
