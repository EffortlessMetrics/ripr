export function PriceTag({ amount, threshold }: { amount: number; threshold: number }) {
  if (amount >= threshold) {
    return <span>{amount - 10}</span>;
  }
  return <span>{amount}</span>;
}
