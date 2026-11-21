import { applyDiscount } from '../src/discount';

test('applyDiscount applies discount when amount meets threshold', () => {
    const result = applyDiscount(100, 100);
    expect(result).toBeGreaterThan(50);
});
