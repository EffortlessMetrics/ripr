/** Threshold resolved through a control-flow join the contract does not cross. */
export function discountedTotal(subtotal: number): number {
  let threshold: number;
  if (subtotal > 0) {
    threshold = 10000;
  } else {
    threshold = 0;
  }
  if (subtotal >= threshold) {
    return subtotal - 500;
  }
  return subtotal;
}
