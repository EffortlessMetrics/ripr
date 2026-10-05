Custom workflow directories selected with `ripr agent start --out` now retain
literal Unix filename characters in generated commands and artifact locators.
With an absolute repository root without a literal Unix backslash in its name,
replaying the workflow from another directory keeps snapshots, verification and
receipt input in the selected workflow directory.
