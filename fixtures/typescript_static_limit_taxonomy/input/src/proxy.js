export function wrap(target, handler) {
    return new Proxy(target, handler);
}
