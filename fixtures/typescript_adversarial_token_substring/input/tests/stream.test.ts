import { bufferedStream } from '../src/stream';

test('counts the chunks in the stream', () => {
  expect(bufferedStream(["a", "b", "c"])).toBe(3);
});
