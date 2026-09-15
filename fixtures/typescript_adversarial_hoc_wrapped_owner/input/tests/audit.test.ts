import { total } from '../src/audit';

test('computes the total with tax', () => {
  expect(total(100)).toBe(120);
});
