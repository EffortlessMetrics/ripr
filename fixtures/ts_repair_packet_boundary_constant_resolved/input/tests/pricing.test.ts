import { discountedTotal } from '../src/pricing';

test('no discount below threshold', () => {
  expect(discountedTotal(5000)).toBe(5000);
});

test('discounts far above threshold', () => {
  expect(discountedTotal(20000)).toBe(18000);
});
