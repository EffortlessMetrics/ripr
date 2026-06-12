import { createToken } from '../src/token';

test('createToken returns prefixed token', () => {
    const token = createToken('user123');
    expect(token).toBe('token-user123');
});
