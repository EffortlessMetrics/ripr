import { computePrice } from '../src/pricing';

test('computePrice returns expected', () => {
    const expected = getExpectedValue();
    const result = computePrice(10, 2);
    expect(result).toBe(expected);
});

function getExpectedValue() { return 20; }
