import { computeFee } from '../src/pricing';

test('computes the processing fee', () => {
  expect(computeFee(100)).toBe(3);
});
