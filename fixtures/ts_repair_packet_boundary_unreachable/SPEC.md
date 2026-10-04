# Fixture: ts_repair_packet_boundary_unreachable

Spec: RIPR-SPEC-0087

## Given

A single-package TypeScript workspace where `login` has a boundary condition
change (`>` → `>=` on `user.length`) and an oracle-eligible related test with:

- A direct import-aware call relation (`import { login } from '../src/auth'`)
- A weak relational oracle (`expect(login('alice')).toBeGreaterThan(4)`) with a
  concrete literal expected value (`4`) and `has_dynamic_matcher_arg == false`
- A discoverable `package.json` with `jest` in `devDependencies` and `scripts.test`
- `package-lock.json` confirming npm runner
- A named missing discriminator (`user.length == 3`) from the boundary expression
- An observed oracle call input `'alice'` whose static string length (5) provably
  does NOT satisfy that discriminator boundary (issue #4105)

## When

```bash
ripr check \
  --root fixtures/ts_repair_packet_boundary_unreachable/input \
  --diff fixtures/ts_repair_packet_boundary_unreachable/diff.patch
```

## Then

The TypeScript preview adapter:

- Classifies the finding as `WeaklyExposed` (oracle strength is Weak, not Strong)
- Sets `actionability_category: incomplete_repair_packet` (G-A passes)
- Projects a `GapRecord` whose `assertion_shape` is the boundary placeholder
  `expect(login(/* boundary input for user.length == 3 */)).toBe(expected)` — it must NOT
  reuse the observed input `login('alice')`, which statically cannot reach the
  named boundary (G-G, #4105)
- Adds a stop condition forbidding reuse of the observed call input
- Fails the packet closed through the shared validator
  (`agent_packet` ineligible): `repair_packet_ready` stays `false`
- Keeps `actionability_category: incomplete_repair_packet` and
  `gap_state: advisory` (no complete-packet flip)
- Keeps `authority_boundary: preview_advisory_only` (TypeScript stays preview)
- Omits `typescript_repair_packet` from the check JSON (never emit a partial
  or implied packet)
- Shows the human limitation section with
  `target shape (not delegatable): expect(login(/* boundary input for user.length == 3 */)).toBe(expected)`

## Must Not

- Emit `repair_packet_ready: true` when the observed call input provably
  cannot reach the named discriminator boundary (#4105)
- Present the observed call input (`login('alice')`) as the assertion shape
  for the missing discriminator — an agent following it verbatim would
  duplicate a non-discriminating assertion
- Emit `typescript_repair_packet` without the shared validator returning `Ok(())`
- Change `schema_version`
- Add new public symbols
- Execute runtime code, call providers, or generate tests
