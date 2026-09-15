# Fixture: typescript_adversarial_render_unobserved_sink (false-exposed guard — reaches but does not observe)

Spec: RIPR-SPEC-0108

Corpus case: `ts_render_reaches_unobserved_sink` in
`fixtures/evidence-promotion-honesty-corpus/corpus.json` (issue #1983,
TypeScript family "component/render test that reaches but does not observe the
changed sink").

## Given

An adversarial **render-test proximity** over-credit trap. The changed owner is
the helper `formatBadge` (src/badge.tsx), whose overflow boundary flips from
`count > 9 ? "9+"` to `count > 99 ? "99+"`. The only test renders the
surrounding `Badge` component with `count: 5` — far from the changed boundary
— and asserts the rendered markup exactly:

```ts
// changed owner: formatBadge (boundary 9 -> 99)
return count > 99 ? "99+" : String(count);

// the ONLY test — renders Badge with count=5, never touches the boundary
import { Badge } from '../src/badge';
const html = Badge({ count: 5 });
expect(html).toBe('<span class="badge">5</span>');
```

The tempting wrong relation is render proximity: the test lives in the
matching `badge.test.tsx` file and carries a strong exact-value assertion, so
a proximity-plus-oracle matcher would promote. But the test never calls
`formatBadge` directly, and its `count: 5` input does not exercise the changed
`> 99` boundary — the strong assertion observes the unchanged small-count
path, not the changed sink.

## When

`ripr check` analyzes the diff against the TypeScript preview adapter.

## Then

ripr classifies the change `weakly_exposed`. The render test does not call the
changed owner (`Badge(...)` is not `formatBadge(...)`), so the only link is
the same-file-stem proximity heuristic — advisory-only, and heuristic links
cannot borrow the extracted strong assertion as proof of observation.

**This fixture must NEVER read `exposed`.** Crediting reach-plus-a-strong-
oracle as `exposed` without evidence the assertion observes the changed sink
is the coverage mistake (see `docs/STATIC_EXPOSURE_MODEL.md` § Discrimination
vs Coverage).

## Must Not

- Credit an exact-value assertion that observes the unchanged small-count path
  as a discriminator for the changed overflow boundary.
- Borrow assertions through a heuristic (same-file-stem) relation.
- Run any TypeScript runtime or renderer; static preview evidence only.
