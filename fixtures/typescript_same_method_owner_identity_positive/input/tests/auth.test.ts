import { TokenValidator } from '../src/auth';

test('validates a trimmed token', () => {
  const validator = new TokenValidator(["abc"]);
  expect(validator.validate(" abc ")).toBe(true);
});
