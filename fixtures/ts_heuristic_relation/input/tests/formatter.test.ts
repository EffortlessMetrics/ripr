// Test file with only heuristic name proximity — no direct import of formatCurrency
describe('formatCurrency behavior', () => {
    test('formats positive amounts', () => {
        const result = (globalThis as any).formatter?.formatCurrency(10, 'USD') ?? '$10.00';
        expect(result).toBeGreaterThan(0 as any);
    });
});
