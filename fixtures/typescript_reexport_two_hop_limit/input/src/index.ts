// Hop 2: index.ts re-exports from errors.ts (two-hop chain: util -> errors -> index)
export { isRawNetworkError } from './errors';
