import { parseLimit } from '../src/limiter';

test('parseLimit parses a numeric limit', () => {
    expect(parseLimit('10')).toBe(10);
});
