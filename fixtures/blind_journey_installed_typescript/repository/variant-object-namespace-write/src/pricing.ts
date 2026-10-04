/** Discount settings the deployment preset may rewrite at startup. */
export const pricingConfig: { discountThreshold: number } = {
  discountThreshold: 10000,
};

export function applyDeploymentPreset(preset: "standard" | "enterprise"): void {
  if (preset === "enterprise") {
    pricingConfig.discountThreshold = 20000;
  }
}

export function discountedTotal(subtotal: number): number {
  if (subtotal >= pricingConfig.discountThreshold) {
    return subtotal - 500;
  }
  return subtotal;
}
