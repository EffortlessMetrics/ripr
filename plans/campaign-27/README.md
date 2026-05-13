# Campaign 27 Plans

This folder holds campaign-specific implementation plans that need more
sequencing detail than the campaign ledger or active manifest should carry.
Plans here are execution guides only; they do not replace proposals, specs,
ADRs, the Campaign 27 ledger, or `.ripr/goals/active.toml`.

For Lane 3 editor preview routing, use the layers this way:

- proposal: why the editor should project opt-in preview evidence;
- spec: what routing, labels, static limits, and fail-closed states must do;
- ADR: the durable projection-only architecture decision;
- plan: PR sequence, acceptance, proof commands, and rollback per slice;
- active manifest: current machine-readable execution state only;
- lane tracker: Lane 3 scope, readiness, blockers, and maintenance evidence;
- closeout: final proof, landed scope, gaps, and future editor campaigns.

The Lane 3 implementation plan is
[lane3-editor-preview-routing.md](lane3-editor-preview-routing.md). The
Campaign 27 routing slice has landed after `analysis/python-preview-adapter`
emitted editor-projectable preview artifacts; generated CI language grouping
is the next unblocked projection item.

Related durable sources:

- [Campaign 27 ledger](../../docs/IMPLEMENTATION_CAMPAIGNS.md#campaign-27-language-adapter-preview)
- [Implementation plan](../../docs/IMPLEMENTATION_PLAN.md)
- [Lane 3 tracker](../../docs/lanes/LANE_3_EDITOR_LSP.md)
- [Repo tracking model](../../docs/REPO_TRACKING_MODEL.md)
- [Active goal manifest](../../.ripr/goals/active.toml)
