# Fixture: typescript_adversarial_mocked_barrel (false-exposed guard — test mocks the barrel it imports the owner through)

Spec: RIPR-SPEC-0108

Corpus case: `ts_mocked_barrel_unrelated_assertion` in
`fixtures/evidence-promotion-honesty-corpus/corpus.json` (PR #4502 review,
TypeScript family "mocked module or dependency with unrelated assertion" —
barrel arm).

## Given

The changed owner is `applyDiscount` (src/discount.ts), whose threshold
predicate changes from `amount > threshold` to `amount >= threshold`.
`src/index.ts` star-exports it (`export * from "./discount";`). The only test
imports the owner through that barrel and mocks the BARREL, not the owner's
own module:

```ts
import { applyDiscount } from "../src";

jest.mock("../src", () => ({ applyDiscount: jest.fn(() => 90) }));

test("applyDiscount at threshold discounts", () => {
    const result = applyDiscount(100, 100);
    expect(result).toBe(90);
});
```

The tempting wrong relation is `re_export_chain_followed`: the chain resolves
`../src` → `src/index` → `src/discount`, and the owner-module mock guard only
compares the owner's own file. But the mock replaces every binding the barrel
forwards, so the call executes the mock, not the changed code.

## When

```bash
cargo xtask fixtures typescript_adversarial_mocked_barrel
```

## Then

The re-export resolver treats a mocked module anywhere on the chain (the
imported barrel or an intermediate hop) as unresolvable, so no
`re_export_chain_followed` relation is credited and the finding reads
`no_static_path`. `repair_packet_ready` stays `false`.

**This fixture must NEVER read `exposed`.** Before the guard it did
(`relation_reason: re_export_chain_followed`, strong exact-value oracle).

## Must Not

- Credit a re-export relation through a mocked barrel or intermediate hop.
- Borrow the stubbed mock's exact-value oracle as evidence about the changed
  `applyDiscount` owner.
- Flip `repair_packet_ready` to `true`.
- Run any TypeScript runtime; static preview evidence only.
