import test from 'ava';
import { buildUrl } from '../src/fetch';

test('buildUrl joins base and path', t => {
    const result = buildUrl('https://example.com', 'api');
    t.is(result, 'https://example.com/api');
});
