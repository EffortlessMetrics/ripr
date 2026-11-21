function withLogging<T extends (...args: any[]) => any>(fn: T): T {
  return ((...args: any[]) => {
    console.log("audit", args[0]);
    return fn(...args);
  }) as T;
}

function computeTotal(amount: number): number {
  return amount * 1.2;
}

export const total = withLogging(computeTotal);
