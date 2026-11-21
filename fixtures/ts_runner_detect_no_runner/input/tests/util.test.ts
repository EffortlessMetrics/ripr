import { format } from '../src/util';

// Uses generic test() shape (would match jest/vitest syntax)
test('format returns string', () => {
    const result = format(42);
    expect(result).toBe('42');
});
