// Test imports from index.ts which is TWO hops from the owner in util.ts:
//   index.ts -> errors.ts -> util.ts
// The chain is within ripr's bounded re-export hop limit (RIPR-SPEC-0095), so
// this test is credited via re_export_chain_followed.
import { isRawNetworkError } from '../src/index';

test('isRawNetworkError via two-hop chain', () => {
    const err = new Error('fetch failed');
    expect(isRawNetworkError(err)).toBe(true);
});
