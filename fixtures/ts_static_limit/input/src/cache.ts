export function dispatchAction(handlers: Record<string, () => void>, key: string): void {
    handlers[key]();
}
