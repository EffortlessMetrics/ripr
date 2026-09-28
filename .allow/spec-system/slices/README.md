# Implementation slices (`ImplementationSliceV1`)

One behavior-changing PR owns one small slice in this directory. A slice
declares scope and claims for a single reviewed change; it is not a task
database and must not carry live execution state.

A slice may contain: requirement IDs and generations, change class, owned /
shared / forbidden seams, evidence obligations, non-goals, return conditions,
and claim boundaries.

A slice must not contain mutable worker, model, branch, worktree, PR, CI,
reviewer, priority, timing, scheduling, session, or progress state. Live work
selection and ownership come from GitHub issues, PRs, checks, reviews, and the
local worktree — never from a tracked file in this repository.

Slices compose: any number of unrelated slices may coexist. No slice is a
"current" or "active" pointer, and no slice authorizes another slice's seams.
