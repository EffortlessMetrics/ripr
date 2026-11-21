// Test imports isRawNetworkError from the index, but the index re-exports from
// OTHER.ts — NOT from util.ts (the changed owner). The test must NOT be credited.
import { isRawNetworkError } from '../src/index';

test('isRawNetworkError from index (unrelated chain)', () => {
    expect(isRawNetworkError('NETWORK error')).toBe(true);
});
