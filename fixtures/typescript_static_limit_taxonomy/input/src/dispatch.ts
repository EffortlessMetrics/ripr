export function dispatch(actions: Record<string, () => void>, key: string): void {
    actions[key]();
}
