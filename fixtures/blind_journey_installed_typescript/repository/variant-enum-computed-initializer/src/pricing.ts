/** Discount tiers named by the pricing enum. */
export enum DiscountTier {
  Standard = 0,
  Volume = 10000,
}

export function discountedTotal(subtotal: number): number {
  if (subtotal >= DiscountTier.Volume) {
    return subtotal - 500;
  }
  return subtotal;
}
