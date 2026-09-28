import { applyDiscount } from '../src/pricing';

test('applyDiscount discounts above the threshold', () => {
    expect(applyDiscount(150, 100)).toBe(0.9);
});
