/**
 * Format a price as a string.
 */
export function formatPrice(amount: number): string {
  return `$${amount.toFixed(2)}`;
}
