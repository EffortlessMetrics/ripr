import { discount } from '../src/pricing';

test('no discount at or below limit', () => {
  expect(discount(50, 100)).toBe(0);
});

test('discounts far above limit', () => {
  expect(discount(500, 100)).toBeGreaterThan(0);
});
