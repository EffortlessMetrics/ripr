import { shipping } from '../src/shipping';

test('large order ships at the reduced rate', () => {
  expect(shipping(6000)).toBe(1);
});
