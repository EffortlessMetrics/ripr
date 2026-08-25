from pathlib import Path


main = Path("xtask/src/main.rs")
text = main.read_text(encoding="utf-8")

function_start = text.find("fn is_manifest_only_fixture_dir(path: &Path) -> bool {")
if function_start < 0:
    raise SystemExit("manifest-only fixture classifier was not found")
function_end_marker = "\n}\n\nfn fixture_contract_violations"
function_end = text.find(function_end_marker, function_start)
if function_end < 0:
    raise SystemExit("manifest-only fixture classifier boundary was not found")
function_block = text[function_start:function_end]

fixture_literal = '                    | "source_promotion_resolved_tree"\n'
if '"source_promotion_resolved_tree"' not in function_block:
    insertion_anchor = '                    | "surface-projection-alignment"\n'
    if function_block.count(insertion_anchor) != 1:
        raise SystemExit("manifest-only fixture insertion anchor is not unique")
    function_block = function_block.replace(
        insertion_anchor,
        fixture_literal + insertion_anchor,
        1,
    )
    text = text[:function_start] + function_block + text[function_end:]

regression_start = text.find(
    "fn evidence_quality_benchmark_is_manifest_only_fixture_dir() -> Result<(), String> {"
)
if regression_start < 0:
    raise SystemExit("manifest-only fixture regression test was not found")
regression_end = text.find("\n    }", regression_start)
if regression_end < 0:
    raise SystemExit("manifest-only fixture regression test boundary was not found")
regression_block = text[regression_start:regression_end]

regression_assert = '''        assert!(super::is_manifest_only_fixture_dir(Path::new(
            "fixtures/source_promotion_resolved_tree"
        )));
'''
if "fixtures/source_promotion_resolved_tree" not in regression_block:
    ok_anchor = "        Ok(())\n"
    if regression_block.count(ok_anchor) != 1:
        raise SystemExit("manifest-only fixture regression insertion anchor is not unique")
    regression_block = regression_block.replace(ok_anchor, regression_assert + ok_anchor, 1)
    text = text[:regression_start] + regression_block + text[regression_end:]

main.write_text(text, encoding="utf-8")

for transient in [
    Path(".github/workflows/repair-1605-manifest-fixture.yml"),
    Path(".github/scripts/repair_1605_manifest_fixture.py"),
]:
    transient.unlink(missing_ok=True)
