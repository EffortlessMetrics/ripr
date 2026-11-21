type Handler = (value: number) => void;

export function summarize(amount: number, threshold: number, handlers: Record<string, Handler>, name: string): number {
    const total = applyDiscount(amount, threshold);
    handlers[name](total);
    return total;
}

function applyDiscount(amount: number, threshold: number): number {
    return amount >= threshold ? amount - 10 : amount;
}
