export function userIsAdmin(order: { role?: string }): boolean {
    if (order?.role === "admin") {
        return true;
    }
    return false;
}

export function pickLabel(name: string | null): string {
    if (name ?? "temp") {
        return "named";
    }
    return "anonymous";
}

export function* thresholdsAbove(total: number): Generator<number> {
    yield total >= 50;
    yield total;
}
