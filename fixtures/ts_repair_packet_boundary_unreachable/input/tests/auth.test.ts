import { login } from '../src/auth';

test('login returns a long session token', () => {
  expect(login('alice')).toBeGreaterThan(4);
});
