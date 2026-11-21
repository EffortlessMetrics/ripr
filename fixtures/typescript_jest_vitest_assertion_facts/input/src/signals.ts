export function applyDiscount(amount: number, threshold: number): number {
  if (amount >= threshold) {
    return amount - 10;
  }
  return amount;
}

export function renderSummary(status: string): string {
  return `summary:${status.trim()}`;
}

export function isReady(count: number): boolean {
  return count >= 1;
}

export function notifyStatus(status: string, sink: { record(value: string): void }): void {
  sink.record(status);
}

export function notifyLiteral(sink: { record(value: string): void }): void {
  sink.record("ready");
}

export async function loadProfile(id: string): Promise<string> {
  return await Promise.resolve(`profile:${id}`);
}
