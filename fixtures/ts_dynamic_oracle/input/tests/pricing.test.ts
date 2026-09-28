import { computePrice } from '../src/pricing';

test('computePrice returns expected', () => {
    const expected = getExpectedValue();
    expect(computePrice(0, 2)).toBe(expected);
});

function getExpectedValue() { return 0; }
