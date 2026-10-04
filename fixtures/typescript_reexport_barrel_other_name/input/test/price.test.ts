// The test imports ONLY `formatCents` from the barrel; the changed
// `applyDiscount` is forwarded by the same barrel but never imported here.
import { describe, expect, test } from 'vitest';
import { formatCents } from '../src';

describe('price', () => {
    test('formats cents', () => {
        expect(formatCents(1990)).toBe('19.90');
    });
});
