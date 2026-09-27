import { applyDiscount, DISCOUNT_THRESHOLD, freeShipping } from '../src/pricing';

test('threshold constant imported from the owner module', () => {
    expect(applyDiscount(DISCOUNT_THRESHOLD)).toBe(0.9);
});

test('threshold constant declared in the test body', () => {
    const BULK_ITEMS = 5;
    expect(freeShipping(BULK_ITEMS)).toBe(true);
});
