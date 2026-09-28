# Fixture: typescript_adversarial_owner_module_mock (false-exposed guard — test mocks the changed owner's own module)

Spec: RIPR-SPEC-0108

Corpus case: `ts_mocked_owner_module_unrelated_assertion` in
`fixtures/evidence-promotion-honesty-corpus/corpus.json` (issue #2269,
TypeScript family "mocked module or dependency with unrelated assertion" —
owner-module arm).

## Given

An adversarial **owner-module mock** trap. The changed owner is the exported
function `applyDiscount` (src/discount.ts), whose threshold predicate changes
from `amount > threshold` to `amount >= threshold`. The only test mocks the
changed owner's OWN module, stubs the mocked function, and asserts an exact
value under a strong oracle:

```ts
import { applyDiscount } from "../src/discount";

jest.mock("../src/discount");

const mockedApplyDiscount = applyDiscount as jest.Mock;
mockedApplyDiscount.mockReturnValue(90);

test("applyDiscount at threshold discounts", () => {
    const result = applyDiscount(100, 100);
    expect(result).toBe(90);
});
```

The tempting wrong relation is `direct_owner_call`: the test body calls
`applyDiscount(` and carries a strong exact-value oracle. But `jest.mock`
replaces the owner's whole module, so the call executes the MOCK, not the
changed code — the stubbed return value cannot observe the changed sink.
Crediting reach-through-a-mock as discrimination is the coverage mistake
(docs/STATIC_EXPOSURE_MODEL.md § Discrimination vs Coverage).

The dependency-mock arm — the test mocks a DIFFERENT module than the owner —
is already pinned by `fixtures/typescript_mocked_module_limit`; this fixture
pins the owner-module arm. The Perl analog
(`perl_mocked_module_unrelated_assertion`) stays `weakly_exposed`.

## When

```bash
cargo xtask fixtures typescript_adversarial_owner_module_mock
```

or:

```bash
ripr check \
  --root fixtures/typescript_adversarial_owner_module_mock/input \
  --diff fixtures/typescript_adversarial_owner_module_mock/diff.patch \
  --mode fast
```

## Then

ripr classifies the change at most `weakly_exposed`. The
`test_mocks_owner_module` guard in `owner_call_relation`
(`crates/ripr/src/analysis/language/typescript/related_tests.rs`) blocks the
`DirectOwnerCall` credit — the same guard the Method/ClassMethod/ModuleFunction
relation paths already apply — so the only remaining link is the same-file-stem
proximity heuristic, which is advisory-only and cannot borrow the stubbed
mock's strong assertion as proof. The `mocked_module` static limit and the
`typescript_mock_only_observer` named limitation stay disclosed, and
`repair_packet_ready` stays `false`.

**This fixture must NEVER read `exposed`.** Crediting the mocked call's
oracle as observing the changed owner would be exactly the
reach-through-a-mock false-`exposed` family this corpus exists to pin.

## Must Not

- Credit `DirectOwnerCall` (or any owner-call relation) when the test mocks
  the changed owner's own module.
- Borrow the stubbed mock's exact-value oracle as evidence about the changed
  `applyDiscount` owner.
- Suppress the `mocked_module` static limit or the
  `typescript_mock_only_observer` limitation; the limit is reported, and the
  class is downgraded.
- Flip `repair_packet_ready` to `true`.
- Run any TypeScript runtime; static preview evidence only.
