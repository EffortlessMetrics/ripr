import * as assert from 'assert';
import {
  NO_RUNNING_SERVER_MESSAGE,
  cockpitUnavailableMessage,
  interpretCockpitObjectResponse,
  messageClaimsHungServer
} from '../../src/cockpitRequest';

const HUNG_SERVER_MESSAGE = 'No receipt status available — server not responding.';

suite('Cockpit request absence split', () => {
  test('no-client message never claims the server hung', () => {
    const message = cockpitUnavailableMessage({ kind: 'no_client' }, HUNG_SERVER_MESSAGE);

    assert.strictEqual(message, NO_RUNNING_SERVER_MESSAGE);
    assert.strictEqual(messageClaimsHungServer(message), false);
    assert.ok(message.includes('No ripr server is running'));
  });

  test('unavailable keeps the caller hung-server wording', () => {
    const message = cockpitUnavailableMessage({ kind: 'unavailable' }, HUNG_SERVER_MESSAGE);

    assert.strictEqual(message, HUNG_SERVER_MESSAGE);
    assert.strictEqual(messageClaimsHungServer(message), true);
  });

  test('null, undefined, and non-object responses are unavailable, not no-client', () => {
    assert.deepStrictEqual(interpretCockpitObjectResponse(null), { kind: 'unavailable' });
    assert.deepStrictEqual(interpretCockpitObjectResponse(undefined), { kind: 'unavailable' });
    assert.deepStrictEqual(interpretCockpitObjectResponse(42), { kind: 'unavailable' });
    assert.deepStrictEqual(interpretCockpitObjectResponse('not-a-packet'), { kind: 'unavailable' });
  });

  test('object responses stay on the ok path', () => {
    const value = { kind: 'receipt_status', receipt_status: 'improved' };
    assert.deepStrictEqual(interpretCockpitObjectResponse(value), { kind: 'ok', value });
  });

  test('arrays count as objects and stay on the ok path for the caller to reject', () => {
    const value = ['not', 'a', 'packet'];
    assert.deepStrictEqual(interpretCockpitObjectResponse(value), { kind: 'ok', value });
  });
});
