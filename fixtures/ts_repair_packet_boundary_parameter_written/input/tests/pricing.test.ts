import { discount } from '../src/pricing';

test('no discount below threshold', () => {
  expect(discount(50, 100)).toBe(0);
});

test('discounts far above threshold', () => {
  expect(discount(500, 100)).toBeGreaterThan(0);
});
