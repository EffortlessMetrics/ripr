import { applyDiscount } from '../src/pricing';

test('applyDiscount at the boundary via a local binding', () => {
    const result = applyDiscount(100);
    expect(result).toBe(0.9);
});
