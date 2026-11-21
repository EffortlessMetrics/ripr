export function PriceLabel({ amount, threshold }) {
  if (amount >= threshold) {
    return <span>{amount - 10}</span>;
  }
  return <span>{amount}</span>;
}
