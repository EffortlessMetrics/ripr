// CommonJS-style import: const { fn } = require('./path')
const { formatAmount } = require('../src/format');

test('formats amount with two decimals', () => {
    expect(formatAmount(1.5, 2)).toBe('1.50');
});
