import { vi } from 'vitest';
vi.mock('../src/nativeModule');

import { processData } from '../src/bridge';

test('processData trims and lowercases', () => {
    expect(processData('Hello')).toBe('hello');
});
