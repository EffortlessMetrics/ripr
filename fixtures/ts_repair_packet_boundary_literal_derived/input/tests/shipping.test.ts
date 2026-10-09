import { shipping } from '../src/shipping';

test('small order pays shipping', () => {
  expect(shipping(1000)).toBe(500);
});
