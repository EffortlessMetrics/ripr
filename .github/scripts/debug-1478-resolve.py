#!/usr/bin/env python3
from pathlib import Path
import json
import os
import subprocess

SOURCE = os.environ['SOURCE_PARENT']
SWARM = os.environ['SWARM_PARENT']
EXPECTED = os.environ['EXPECTED_TREE']


def git(*args: str, capture: bool = False) -> str:
    if capture:
        return subprocess.check_output(['git', *args], text=True)
    subprocess.run(['git', *args], check=True)
    return ''


# Start from the current swarm implementation for conflicts whose source side is
# superseded, then layer the bounded source-only behavior and source authority.
git('checkout', '--theirs', '--',
    'crates/ripr/src/analysis/probes/diff.rs',
    'crates/ripr/src/output/review_comments.rs',
    'docs/specs/README.md',
    'editors/vscode/package.json',
    'editors/vscode/package-lock.json',
    'fixtures/boundary_gap/expected/pr-guidance/configured-off/comments.json',
    'policy/process_allowlist.txt')

source_changelog = git('show', 'source-parent:CHANGELOG.md', capture=True)
swarm_changelog = git('show', 'swarm-candidate:CHANGELOG.md', capture=True)
header, rest = swarm_changelog.split('## Unreleased\n', 1)
unreleased_body, history = rest.split('\n## 0.10.0 ', 1)
_, source_after = source_changelog.split('## Unreleased\n', 1)
source_0101, _ = source_after.split('\n## 0.10.0 ', 1)
source_0101 = source_0101.replace('RIPR-SPEC-0112', 'RIPR-SPEC-0144')
Path('CHANGELOG.md').write_text(
    header + '## Unreleased\n' + unreleased_body.rstrip() + '\n\n'
    + source_0101.strip() + '\n\n## 0.10.0 ' + history
)

p = Path('crates/ripr/src/analysis/probes/diff.rs')
s = p.read_text()
needle = 'use super::lexical::classify_changed_line;\n'
assert needle in s and 'bounded_subprocess_family' not in s
s = s.replace(needle, needle + 'use super::subprocess::bounded_subprocess_family;\n', 1)
needle = '''        if changed_line_owned_by_test(index, &changed.path, added.new_side_line) {
            continue;
        }
        let parser_shapes =
'''
insert = '''        if changed_line_owned_by_test(index, &changed.path, added.new_side_line) {
            continue;
        }
        if let Some(family) =
            bounded_subprocess_family(index, &changed.path, added.new_side_line, text)
        {
            probes.push(build_probe(
                &build_context,
                added,
                family,
                nearby_removed_line(added.new_side_line, text, changed),
                Some(text.to_string()),
            ));
            continue;
        }
        let parser_shapes =
'''
assert needle in s
p.write_text(s.replace(needle, insert, 1))

for path in ['editors/vscode/package.json', 'editors/vscode/package-lock.json']:
    p = Path(path)
    data = json.loads(p.read_text())
    data['version'] = '0.10.1'
    if path.endswith('package-lock.json'):
        data['packages']['']['version'] = '0.10.1'
    p.write_text(json.dumps(data, indent=2) + '\n')

old = Path('docs/specs/RIPR-SPEC-0112-bounded-subprocess-adapter-boundary.md')
new = Path('docs/specs/RIPR-SPEC-0144-bounded-subprocess-adapter-boundary.md')
assert old.exists() and not new.exists()
new.write_text(old.read_text().replace('RIPR-SPEC-0112', 'RIPR-SPEC-0144'))
old.unlink()

p = Path('docs/specs/README.md')
s = p.read_text()
row = '| [RIPR-SPEC-0144](RIPR-SPEC-0144-bounded-subprocess-adapter-boundary.md) | accepted | Bounded literal allowlisted subprocess adapters with argument, timeout, captured-output, and cleanup evidence use the existing side_effect probe family; dynamic, shell, and unbounded commands retain strict exposure behavior; closes #1454 |\n'
assert 'RIPR-SPEC-0144' not in s
anchor = next(line for line in s.splitlines(True) if line.startswith('| [RIPR-SPEC-0143]'))
p.write_text(s.replace(anchor, anchor + row, 1))

p = Path('.ripr/traceability.toml')
s = p.read_text()
start = s.index('[[behavior]]\nid = "RIPR-SPEC-0112"\nname = "bounded subprocess adapter boundary"')
end = s.index('\n[[behavior]]', start + 1)
block = s[start:end].replace('RIPR-SPEC-0112', 'RIPR-SPEC-0144')
p.write_text(s[:start] + block + s[end:])

p = Path('policy/doc-artifacts.toml')
s = p.read_text()
assert 'id = "RIPR-SPEC-0144"' not in s
entry = '''
[[artifact]]
id = "RIPR-SPEC-0144"
kind = "spec"
path = "docs/specs/RIPR-SPEC-0144-bounded-subprocess-adapter-boundary.md"
status = "accepted"
owner = "product-source"
standalone_reason = "Source-only staged 0.10.1 bounded subprocess adapter boundary (#1454): recognizes only one deny-by-default literal allowlisted command shape with visible arguments, timeout, captured output, cleanup, and error handling; dynamic, shell, and unbounded commands remain on the conservative path. Renumbered during the replacement source/swarm join because swarm already owns RIPR-SPEC-0112 and RIPR-SPEC-0133."
'''
p.write_text(s.rstrip() + '\n' + entry)

p = Path('policy/process_allowlist.txt')
s = p.read_text()
line = 'crates/ripr/src/analysis/probes/subprocess.rs|Command::new|7|analysis/probes|RIPR-SPEC-0144: bounded subprocess adapter recognizer inspects source text and fixture bodies for one literal allowlisted command; it does not execute a subprocess or broaden the runtime command surface.\n'
assert 'analysis/probes/subprocess.rs|Command::new' not in s
anchor = next(l for l in s.splitlines(True) if l.startswith('crates/ripr/src/analysis/diff/load.rs|Command::new'))
p.write_text(s.replace(anchor, anchor + line, 1))

p = Path('policy/network_allowlist.txt')
s = p.read_text()
updated = s.replace(
    'crates/ripr/src/analysis/probes/subprocess.rs|curl|6|analysis/probes|RIPR-SPEC-0112:',
    'crates/ripr/src/analysis/probes/subprocess.rs|curl|6|analysis/probes|RIPR-SPEC-0144:'
)
assert updated != s
p.write_text(updated)

p = Path('docs/release/0.11.0/post-freeze-source-survivors.json')
data = json.loads(p.read_text())
data['source_parent'] = SOURCE
for row in data['must_survive_from_source']:
    if row['id'] == 'bounded_subprocess_adapter':
        row['paths_or_seams'] = [x.replace('RIPR-SPEC-0133', 'RIPR-SPEC-0144') for x in row['paths_or_seams']]
for row in data['semantic_conflict_resolution']:
    if row['id'] == 'spec_and_traceability_union':
        row['resolution'] = ('Retain every accepted post-freeze swarm specification and slice; '
                             'renumber the source bounded subprocess contract to RIPR-SPEC-0144 because '
                             'swarm owns RIPR-SPEC-0112 and RIPR-SPEC-0133; update traceability and '
                             'process/network policy without dropping either history.')
p.write_text(json.dumps(data, indent=2) + '\n')

# Source repository settings and protected checks remain source authority. Carry
# the additive status-label registry from swarm without importing swarm merge or
# branch-protection policy. Development-only ub-review does not cross to source.
source_settings = git('show', 'source-parent:.github/settings.yml', capture=True)
swarm_settings = git('show', 'swarm-candidate:.github/settings.yml', capture=True)
marker = '  # Status labels —'
assert marker in swarm_settings and marker not in source_settings
Path('.github/settings.yml').write_text(source_settings.rstrip() + '\n' + swarm_settings[swarm_settings.index(marker):])
Path('.github/workflows/ub-review.yml').unlink(missing_ok=True)

git('add', '-A')
assert not git('diff', '--name-only', '--diff-filter=U', capture=True).strip()
git('diff', '--cached', '--check')
git('commit', '-m', 'preflight: resolve exact 0.11 source/swarm tree')
assert git('rev-parse', 'HEAD^{tree}', capture=True).strip() == EXPECTED
assert git('show', '-s', '--format=%P', 'HEAD', capture=True).strip() == f'{SOURCE} {SWARM}'
