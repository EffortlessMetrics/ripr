type Props = { label: string; amount: number };

export function PriceBadge({ label, amount }: Props) {
    return { status: "ready", label, amount };
}
