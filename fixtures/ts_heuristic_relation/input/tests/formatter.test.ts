// Test file whose only link to formatCurrency is a non-call reference: the
// owner is passed around as a value and never called by name, so only a
// heuristic same-file-proximity relation is established.
import { formatCurrency } from '../src/formatter';

describe('formatCurrency behavior', () => {
    test('formats positive amounts', () => {
        const format = formatCurrency;
        expect(format(10, 'USD')).toBeGreaterThan(0 as any);
    });
});
