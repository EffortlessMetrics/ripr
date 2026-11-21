import { dispatchAction } from '../src/cache';

test('dispatchAction calls the named handler', () => {
    let called = false;
    dispatchAction({ run: () => { called = true; } }, 'run');
    expect(called).toBe(true);
});
