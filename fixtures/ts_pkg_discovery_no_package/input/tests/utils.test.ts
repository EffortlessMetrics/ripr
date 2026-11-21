import { clamp } from '../src/utils';

test('clamp below min', () => {
    const result = clamp(-5, 0, 10);
    expect(result).toBe(0);
});
