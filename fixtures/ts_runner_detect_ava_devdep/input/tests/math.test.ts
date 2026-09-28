import test from 'ava';
import { add } from '../src/math';

test('add returns sum', t => {
    const result = add(1, 2);
    t.is(result, 3);
});
