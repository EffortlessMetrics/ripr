import { validateScore } from '../src/validator';

test('validateScore returns true for passing score', () => {
    expect(validateScore(75)).toBe(true);
});
