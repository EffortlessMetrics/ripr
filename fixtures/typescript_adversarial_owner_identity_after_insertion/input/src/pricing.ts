export interface FeeRule {
  bps: number;
  cap: number;
}

export const DEFAULT_FEE_RULE: FeeRule = { bps: 300, cap: 50 };

export function computeFee(amount: number): number {
  return amount * 0.03;
}

export function applyDiscount(amount: number, threshold: number): number {
  if (amount >= threshold) {
    return amount * 0.9;
  }
  return amount;
}
