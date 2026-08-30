# Golden Output Changes

## Pending

Reason:
Emit no probe for module-scope annotation-only variable changes (#1289 sub-item 1): Python does not enforce annotations at runtime at module scope, so an annotation-only change (identical target name and value, only the annotation differs) has no behavior delta. The guard is module-scope only and fails closed for class bodies (dataclass/Pydantic/attrs make class-body annotations runtime-meaningful).

Command:
`cargo xtask goldens bless python_noop_bare_var_annotation --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
#2103: additive changed_files_by_language field and changed_rust_files now Rust-only count

Command:
`cargo xtask goldens bless python_noop_bare_var_annotation --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0147: publish typed analysis outcome in human and JSON output.

Command:
`cargo xtask goldens bless python_noop_bare_var_annotation --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0147: align fixture outputs with the typed incomplete-outcome and unquoted human outcome contract.

Command:
`cargo xtask goldens bless python_noop_bare_var_annotation --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
