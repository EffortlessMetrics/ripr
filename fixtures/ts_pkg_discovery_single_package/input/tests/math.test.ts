import { add } from '../src/math';

test('add returns sum', () => {
    const result = add(1, 2);
    expect(result).toBe(3);
});
