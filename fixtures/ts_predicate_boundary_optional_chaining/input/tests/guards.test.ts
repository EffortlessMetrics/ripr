import { userIsAdmin, pickLabel, thresholdsAbove } from '../src/guards';

test('admin role behind optional chaining', () => {
    expect(userIsAdmin({ role: "admin" })).toBe(true);
});

test('nullish fallback boundary input', () => {
    expect(pickLabel(null)).toBe("named");
});

test('yield tail at the boundary', () => {
    expect(thresholdsAbove(50)).toBe(true);
});
