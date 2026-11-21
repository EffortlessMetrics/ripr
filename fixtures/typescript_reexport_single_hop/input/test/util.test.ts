import { isRawNetworkError } from '../src/index';

test('isRawNetworkError returns true for network fetch errors', () => {
    const err = new Error('fetch failed');
    expect(isRawNetworkError(err)).toBe(true);
});

test('isRawNetworkError returns false for non-Error values', () => {
    expect(isRawNetworkError('string error')).toBe(false);
});
