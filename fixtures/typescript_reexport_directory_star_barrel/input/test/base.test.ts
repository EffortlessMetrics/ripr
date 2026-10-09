// The test imports through the DIRECTORY specifier `../src`, which resolves
// to `src/index.ts`, whose `export * from './utils'` forwards `withoutBase`.
import { describe, expect, test } from 'vitest';
import { withoutBase } from '../src';

describe('withoutBase', () => {
    test('strips the base prefix', () => {
        expect(withoutBase('/base/a', '/base')).toBe('/a');
    });

    test('keeps input outside the base', () => {
        expect(withoutBase('/other', '/base')).toBe('/other');
    });
});
