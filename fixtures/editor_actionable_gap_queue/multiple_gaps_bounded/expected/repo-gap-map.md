RIPR repo gap map

Scope
Artifact: target/ripr/reports/actionable-gaps.json
Queue state: topActionableGap
Actionable gaps: 3
Report-only gaps: 2
Static-limit-only gaps: 1

Top queue item
Canonical gap id: gap:rust:pricing:discount:threshold-boundary
Repair kind: strengthen_assertion
Related test: tests/pricing.rs::premium_customer_gets_discount

Safe next commands
- Copy the current repair packet for the top gap.
- Refresh before selecting the next gap.

Non-claims
- This map is read-only orientation.
- It is not a gate decision, merge approval, runtime proof, mutation proof, coverage claim, or policy eligibility claim.
