import { multiply } from '../src/calculator';

test('multiply returns expected for positive inputs', () => {
    const result = multiply(3, 5);
    expect(result).toBeGreaterThan(10);
});
