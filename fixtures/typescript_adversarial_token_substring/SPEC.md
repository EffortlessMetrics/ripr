# Fixture: typescript_adversarial_token_substring (false-exposed guard — token substring collision)

Spec: RIPR-SPEC-0108

Corpus case: `ts_token_substring` in
`fixtures/evidence-promotion-honesty-corpus/corpus.json` (issue #1983,
TypeScript family "token substring collision").

## Given

An adversarial **token-substring** over-credit trap, the TypeScript analogue
of `fixtures/python_adversarial_buffer_token`. The changed owner is the short,
common token `buffer` (src/stream.ts), whose join separator changes from
`","` to `"|"`. The only test exercises a **different function** whose name
merely *contains* the owner token as a substring — `bufferedStream` — under a
strong exact-value oracle:

```ts
// changed owner: buffer
return chunks.join("|");

// the ONLY test — a DIFFERENT function: `buffer` ⊂ `bufferedStream`
import { bufferedStream } from '../src/stream';
expect(bufferedStream(["a", "b", "c"])).toBe(3);
```

The tempting wrong relation is the substring `buffer ⊂ bufferedStream`: a
substring matcher would link the test to the owner even though
`bufferedStream` is a distinct exported function that never calls `buffer`.

## When

`ripr check` analyzes the diff against the TypeScript preview adapter.

## Then

ripr classifies the change `weakly_exposed`. The call-name matcher requires a
call boundary, so `bufferedStream(` does not credit a `buffer(` owner call; the
import of `bufferedStream` from the owner file does not name the owner either.
The only link is the same-file-stem proximity heuristic, which is
advisory-only and cannot borrow the extracted strong assertion as proof.

**This fixture must NEVER read `exposed`.** Before call-boundary matching this
family read `exposed` on substring coincidence alone (see `docs/LEARNINGS.md`
§ Token coincidence is a false-`exposed` family).

## Must Not

- Credit a direct owner call from a substring (`buffer` inside
  `bufferedStream`) token match.
- Borrow the `bufferedStream` exact-value oracle as evidence about `buffer`.
- Run any TypeScript runtime; static preview evidence only.
