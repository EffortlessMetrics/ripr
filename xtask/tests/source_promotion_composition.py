"""Execute the staged-promotion script against isolated, real Git objects.

These are script/graph controls, not native GitHub admission or release proof.
Run with: python3 xtask/tests/source_promotion_composition.py
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[2] / '.github/scripts/source-promotion-composition.sh'
MARKER = '<!-- source-promotion: true -->'
CONTROL = 'docs/release/source-promotion/contract-inputs.json'
PREFLIGHT = 'docs/release/source-promotion/preflight.json'


class CompositionTests(unittest.TestCase):
    def setUp(self) -> None:
        for tool in ('git', 'bash', 'jq', 'sha256sum'):
            if shutil.which(tool) is None:
                self.fail(f'required test tool is unavailable: {tool}')
        self.temp = tempfile.TemporaryDirectory(prefix='ripr-promotion-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        # Do not import caller Git aliases, signing, templates, hooks or a
        # selected index/worktree. Both setup and the production script use it.
        self.env = {k: v for k, v in os.environ.items() if not k.startswith('GIT_')}
        self.env.update(GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM='1',
                        GIT_TERMINAL_PROMPT='0', GIT_AUTHOR_NAME='Fixture',
                        GIT_AUTHOR_EMAIL='fixture@example.invalid',
                        GIT_COMMITTER_NAME='Fixture',
                        GIT_COMMITTER_EMAIL='fixture@example.invalid')
        self.git('-c', 'init.templateDir=', 'init', '-q')
        self.git('config', 'core.hooksPath', str(self.root / 'no-hooks'))
        (self.root / 'a').write_text('public\n', encoding='utf-8')
        self.git('add', 'a')
        self.git('commit', '-qm', 'P')
        self.p = self.git('rev-parse', 'HEAD')
        self.p_tree = self.git('rev-parse', 'HEAD^{tree}')
        (self.root / 'a').write_text('swarm\n', encoding='utf-8')
        self.git('commit', '-qam', 'S')
        self.s = self.git('rev-parse', 'HEAD')
        self.t = self.git('rev-parse', 'HEAD^{tree}')
        self.k = self.commit(self.t, self.p, self.s)
        self.git('update-ref', 'refs/pull/42/merge', self.k)
        self.git('remote', 'add', 'origin', str(self.root))
        self.pf = {'mode': 'fast_forward', 'source_parent': self.p,
                   'swarm_parent': self.s, 'dry_merge': {'preview_tree': self.t}}
        self.inputs = {'schema': 'ripr.source_promotion_ci_inputs.v2',
                       'source_main': self.p, 'join_head': self.k,
                       'preflight': PREFLIGHT}
        self.env.update(HEAD_REF='promote/0.11.0-test', BASE_SHA=self.p,
                        HEAD_SHA=self.s, MERGE_SHA=self.k, PR_NUMBER='42')
        self.control()

    def command(self, *args: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(args, cwd=self.root, env=self.env, text=True,
                              capture_output=True, timeout=15, check=False)

    def git(self, *args: str) -> str:
        result = self.command('git', *args)
        self.assertEqual(result.returncode, 0, result.stderr)
        return result.stdout.strip()

    def commit(self, tree: str, *parents: str) -> str:
        args = ['commit-tree', tree, '-m', 'fixture commit']
        for parent in parents:
            args.extend(('-p', parent))
        return self.git(*args)

    def control(self, *, digest: str | None = None, executable: bool = False,
                raw_preflight: bytes | None = None,
                duplicate_inputs: bool = False) -> None:
        path = self.root / PREFLIGHT
        path.parent.mkdir(parents=True, exist_ok=True)
        data = (raw_preflight if raw_preflight is not None
                else json.dumps(self.pf, sort_keys=True).encode())
        path.write_bytes(data)
        self.inputs['preflight_sha256'] = digest or hashlib.sha256(data).hexdigest()
        inputs_text = json.dumps(self.inputs)
        if duplicate_inputs:
            second = {key: self.inputs[key] for key in (
                'schema', 'source_main', 'join_head', 'preflight')}
            inputs_text += '\n' + json.dumps(second)
        (self.root / CONTROL).write_text(inputs_text, encoding='utf-8')
        self.git('add', 'docs')
        self.git('update-index', '--chmod=+x' if executable else '--chmod=-x', CONTROL)
        self.git('commit', '--allow-empty', '-qm', 'control')
        self.c = self.git('rev-parse', 'HEAD')
        self.env['PR_BODY'] = f'{MARKER}\n<!-- source-promotion-control: {self.c} -->'

    def check(self, reason: str | None = None) -> None:
        result = self.command('bash', str(SCRIPT))
        if reason is None:
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn('matches the control packet', result.stdout)
        else:
            self.assertNotEqual(result.returncode, 0, result.stdout)
            self.assertIn(f'source-promotion composition rejected: {reason}', result.stderr)

    def test_staged(self) -> None:
        self.check()

    def test_crlf_body(self) -> None:
        self.env['PR_BODY'] = self.env['PR_BODY'].replace('\n', '\r\n') + '\r\n'
        self.check()

    def test_ordinary_is_explicitly_not_a_promotion(self) -> None:
        self.env.update(PR_BODY='ordinary', HEAD_REF='feature')
        result = self.command('bash', str(SCRIPT))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('no source-promotion composition requested', result.stdout)
        self.assertNotIn('matches the control packet', result.stdout)

    def test_missing_pr_body_refuses_without_reclassifying_as_ordinary(self) -> None:
        self.env.pop('PR_BODY')
        self.check('event routing inputs missing')

    def test_missing_head_ref_refuses_without_reclassifying_as_ordinary(self) -> None:
        self.env.update(PR_BODY='ordinary')
        self.env.pop('HEAD_REF')
        self.check('event routing inputs missing')

    def test_missing_marker(self) -> None:
        self.env['PR_BODY'] = 'broken'
        self.check('promotion marker missing')

    def test_duplicate_control_marker(self) -> None:
        self.env['PR_BODY'] += f'\r\n<!-- source-promotion-control: {self.c} -->\r\n'
        self.check('exactly one control marker required')

    def test_wrong_base(self) -> None:
        self.env['BASE_SHA'] = self.s
        self.check('control identities do not bind event base and merge')

    def test_wrong_head(self) -> None:
        self.env['HEAD_SHA'] = self.p
        self.check('staged preflight does not bind event parents')

    def test_stale_merge_event(self) -> None:
        self.env['MERGE_SHA'] = self.p
        self.check('event is not the live test merge commit')

    def test_invalid_event_sha(self) -> None:
        self.env['HEAD_SHA'] = 'not-a-commit'
        self.check('invalid event SHA')

    def test_wrong_preflight_digest(self) -> None:
        self.control(digest='0' * 64)
        self.check('preflight digest mismatch')

    def test_nonregular_control(self) -> None:
        self.control(executable=True)
        self.check('control sidecar is not a regular tracked file')

    def test_missing_preflight_mode(self) -> None:
        del self.pf['mode']
        self.control()
        self.check('invalid or missing preflight mode')

    def test_null_preflight_mode(self) -> None:
        self.pf['mode'] = None
        self.control()
        self.check('invalid or missing preflight mode')

    def test_nonstring_preflight_mode(self) -> None:
        self.pf['mode'] = 1
        self.control()
        self.check('invalid or missing preflight mode')

    def test_multiple_preflight_objects_are_rejected(self) -> None:
        valid = json.dumps(self.pf, sort_keys=True).encode()
        second = {
            'mode': None,
            'source_parent': self.p,
            'swarm_parent': self.s,
            'dry_merge': {},
        }
        stream = valid + b'\n' + json.dumps(second).encode()
        self.control(raw_preflight=stream)
        self.check('preflight must contain exactly one JSON object')

    def test_multiple_control_objects_are_rejected(self) -> None:
        self.control(duplicate_inputs=True)
        self.check('control must contain exactly one JSON object')

    def test_nonobject_preflight_is_rejected(self) -> None:
        self.control(raw_preflight=b'[]')
        self.check('preflight must contain exactly one JSON object')

    def test_unknown_mode(self) -> None:
        self.pf['mode'] = 'unrecognized'
        self.control()
        self.check('unsupported promotion mode')

    def test_wrong_ordered_preview_parents(self) -> None:
        wrong = self.commit(self.t, self.s, self.p)
        self.git('update-ref', 'refs/pull/42/merge', wrong)
        self.inputs['join_head'] = wrong
        self.env['MERGE_SHA'] = wrong
        self.control()
        self.check('test merge parents differ from [P,head]')

    def test_wrong_preview_tree(self) -> None:
        wrong = self.commit(self.p_tree, self.p, self.s)
        self.git('update-ref', 'refs/pull/42/merge', wrong)
        self.inputs['join_head'] = wrong
        self.env['MERGE_SHA'] = wrong
        self.control()
        self.check('test merge tree differs from T')

    def test_forged_preview_tree_cannot_replace_head_tree(self) -> None:
        wrong = self.commit(self.p_tree, self.p, self.s)
        self.git('update-ref', 'refs/pull/42/merge', wrong)
        self.inputs['join_head'] = wrong
        self.env['MERGE_SHA'] = wrong
        self.pf['dry_merge']['preview_tree'] = self.p_tree
        self.control()
        self.check('head tree differs from T')

    def test_staged_requires_public_ancestry(self) -> None:
        orphan = self.commit(self.t)
        preview = self.commit(self.t, self.p, orphan)
        self.git('update-ref', 'refs/pull/42/merge', preview)
        self.pf['swarm_parent'] = orphan
        self.inputs['join_head'] = preview
        self.env.update(HEAD_SHA=orphan, MERGE_SHA=preview)
        self.control()
        self.check('P is not an ancestor of S')

    def test_fast_forward_join_head_must_be_live_preview(self) -> None:
        self.inputs['join_head'] = self.s
        self.control()
        self.check('control preview mismatch')

    def legacy(self, *, reverse: bool = False) -> None:
        parents = (self.s, self.p) if reverse else (self.p, self.s)
        join = self.commit(self.t, *parents)
        preview = self.commit(self.t, self.p, join)
        self.git('update-ref', 'refs/pull/42/merge', preview)
        self.pf = {'mode': 'two_parent_join', 'source_parent': self.p,
                   'swarm_parent': self.s, 'dry_merge': {'reviewed_resolved_tree': self.t}}
        self.inputs['join_head'] = join
        self.env.update(HEAD_SHA=join, MERGE_SHA=preview)
        self.control()

    def test_legacy_join(self) -> None:
        self.legacy()
        self.check()

    def test_legacy_join_parent_order(self) -> None:
        self.legacy(reverse=True)
        self.check('join parents differ from [P,S]')


if __name__ == '__main__':
    unittest.main()
