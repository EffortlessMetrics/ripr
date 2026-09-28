// Test imports from index.ts which is TWO hops from the owner in util.ts:
//   index.ts -> errors.ts -> util.ts
// ripr only follows ONE hop (fail-closed), so this test must stay uncredited.
import { isRawNetworkError } from '../src/index';

test('isRawNetworkError via two-hop chain (ripr limitation)', () => {
    const err = new Error('fetch failed');
    expect(isRawNetworkError(err)).toBe(true);
});
