export const formatPrice = (amount: number) => {
  return `$${amount.toFixed(3)}`;
};

export class Cart {
  total() {
    return this.items.length + 1;
  }

  static build() {
    return new Cart({ tracked: true });
  }
}

export const DEFAULT_RATE = 0.09;
