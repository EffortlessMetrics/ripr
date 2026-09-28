import { validateScore } from '../src/validator';

test('validateScore returns true for passing score', () => {
    expect(validateScore(60)).toBe(true);
});
