"""Local fixtures only: no wheel installation or native execution."""
import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path
import tempfile
import unittest

import python_consumer_evidence as evidence


class ConsumerEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.producer = self.root / 'producer'
        self.producer.mkdir()
        self.output = self.root / 'output'
        self.identity = dict(source_sha='a' * 40, run_id=12, run_attempt=2,
                             wheel_sha256='b' * 64, native_sha256='c' * 64,
                             native_version='0.11.0', version='0.11.0', client='pip')
        self.qualification = dict(repository='EffortlessMetrics/ripr',
                                  source_tree='d' * 40, wheel_filename='fixture.whl',
                                  target='x86_64-unknown-linux-gnu',
                                  platform_tag='py3-none-manylinux_2_34_x86_64',
                                  **{k: v for k, v in self.identity.items()
                                     if k not in ('native_sha256', 'client')})
        self.receipt = dict(source_repository='EffortlessMetrics/ripr',
                            candidate_sha=self.identity['source_sha'], candidate_tree='d' * 40,
                            producer_run_id='12', producer_run_attempt='2',
                            native_version='0.11.0', python_version='0.11.0',
                            wheel=dict(filename='fixture.whl', sha256='b' * 64,
                                       installed_payload_sha256='c' * 64))
        self.write_producer()

    def write_producer(self):
        for name, value in [('qualification.json', self.qualification),
                            ('wheel-receipt.json', self.receipt)]:
            (self.producer / name).write_text(json.dumps(value))

    def start(self):
        evidence.initialize(self.producer, self.output, self.identity)

    def read(self):
        return json.loads((self.output / 'consumer.json').read_text())

    def test_complete_and_retained_output(self):
        self.start()
        for command in evidence.COMMANDS:
            evidence.transition(self.output, self.identity, command, 'running')
            if command in evidence.OUTPUTS:
                (self.output / evidence.OUTPUTS[command]).write_text('fixture output')
            evidence.transition(self.output, self.identity, command, 'passed')
        self.assertTrue(evidence.finalize(self.output, self.identity, self.producer))
        receipt = self.read()
        self.assertEqual(receipt['status'], 'complete')
        self.assertFalse(receipt['release_eligible'])
        self.assertEqual(receipt['commands'][2]['output']['sha256'],
                         hashlib.sha256(b'fixture output').hexdigest())
        self.assertTrue(all(row['selected'] and row['executed'] for row in receipt['commands']))

    def test_interruption_retains_partial_output_and_denominator(self):
        self.start()
        evidence.transition(self.output, self.identity, 'install', 'running')
        evidence.transition(self.output, self.identity, 'install', 'passed')
        evidence.transition(self.output, self.identity, 'native_identity', 'running')
        self.assertFalse(evidence.finalize(self.output, self.identity, self.producer))
        rows = self.read()['commands']
        self.assertEqual([r['status'] for r in rows[:3]], ['passed', 'failed', 'not_run'])
        self.assertTrue(rows[1]['executed'])
        self.assertFalse(rows[2]['executed'])

    def test_partial_failure_output(self):
        self.start()
        for command in evidence.COMMANDS[:2]:
            evidence.transition(self.output, self.identity, command, 'running')
            evidence.transition(self.output, self.identity, command, 'passed')
        evidence.transition(self.output, self.identity, 'preview', 'running')
        (self.output / 'check.json').write_text('{partial')
        evidence.transition(self.output, self.identity, 'preview', 'failed')
        self.assertFalse(evidence.finalize(self.output, self.identity, self.producer))
        self.assertEqual(self.read()['commands'][2]['output']['bytes'], 8)

    def test_mixed_producer_attempt_rejected(self):
        self.receipt['producer_run_attempt'] = '1'
        self.write_producer()
        with self.assertRaises(ValueError):
            self.start()
        self.assertEqual(self.read()['status'], 'rejected')
        self.assertTrue(all(row['status'] == 'not_run' for row in self.read()['commands']))

    def test_stale_identity_and_replaced_producer_rejected(self):
        self.start()
        stale = dict(self.identity, run_attempt=3)
        with self.assertRaises(ValueError):
            evidence.transition(self.output, stale, 'install', 'running')
        self.qualification['source_tree'] = 'e' * 40
        self.write_producer()
        with self.assertRaises(ValueError):
            evidence.finalize(self.output, self.identity, self.producer)

    def test_missing_output_and_out_of_order_pass_rejected(self):
        self.start()
        with self.assertRaises(ValueError):
            evidence.transition(self.output, self.identity, 'preview', 'passed')
        for command in evidence.COMMANDS[:2]:
            evidence.transition(self.output, self.identity, command, 'running')
            evidence.transition(self.output, self.identity, command, 'passed')
        evidence.transition(self.output, self.identity, 'preview', 'running')
        with self.assertRaises(ValueError):
            evidence.transition(self.output, self.identity, 'preview', 'passed')

    def test_cli_uv_complete_and_partial_exit(self):
        self.producer.rename(self.root / 'evidence')
        env = dict(os.environ, CANDIDATE_SHA='a' * 40, GITHUB_RUN_ID='12',
                   GITHUB_RUN_ATTEMPT='2', EXPECTED_WHEEL_SHA256='b' * 64,
                   EXPECTED_PAYLOAD_SHA256='c' * 64, RIPR_NATIVE_VERSION='0.11.0',
                   RIPR_PYTHON_VERSION='0.11.0', CLIENT='uv', CONSUMER_EVIDENCE=str(self.output))
        script = Path(evidence.__file__).resolve()

        def run(*args):
            return subprocess.run([sys.executable, script, *args], cwd=self.root,
                                  env=env, capture_output=True, text=True).returncode

        self.assertEqual(run('init'), 0)
        self.assertEqual(run('finalize'), 1)
        for command in evidence.COMMANDS:
            self.assertEqual(run('running', command), 0)
            if command in evidence.OUTPUTS:
                (self.output / evidence.OUTPUTS[command]).write_text('fixture')
            self.assertEqual(run('passed', command), 0)
        self.assertEqual(run('finalize'), 0)
        self.assertEqual(self.read()['identity']['client'], 'uv')
        self.assertFalse(self.read()['release_eligible'])

    def test_passed_output_substitution_rejected(self):
        self.start()
        for command in evidence.COMMANDS[:3]:
            evidence.transition(self.output, self.identity, command, 'running')
            if command == 'preview':
                (self.output / 'check.json').write_text('original')
            evidence.transition(self.output, self.identity, command, 'passed')
        (self.output / 'check.json').write_text('replacement')
        with self.assertRaisesRegex(ValueError, 'passed output changed'):
            evidence.finalize(self.output, self.identity, self.producer)

    def test_producer_refusal_bounds_outputs_and_persists_terminal_failure(self):
        self.start()
        original = self.read()['producer']
        evidence.transition(self.output, self.identity, 'install', 'running')
        qualification = self.producer / 'qualification.json'
        qualification.write_text(qualification.read_text() + '\n')
        (self.output / 'check.json').write_bytes(b'x' * (evidence.MAX_OUTPUT + 1))
        with self.assertRaisesRegex(ValueError, 'producer bytes changed'):
            evidence.finalize(self.output, self.identity, self.producer)
        receipt = self.read()
        self.assertEqual((self.output / 'check.json').stat().st_size, evidence.MAX_OUTPUT)
        self.assertEqual(receipt['status'], 'rejected')
        self.assertEqual(receipt['producer'], original)
        self.assertFalse(receipt['release_eligible'])
        self.assertEqual(receipt['commands'][0]['status'], 'failed')
        self.assertTrue(receipt['commands'][2]['output']['truncated'])

    def test_rejected_complete_receipt_cannot_recover_by_restoring_producer(self):
        self.start()
        for command in evidence.COMMANDS:
            evidence.transition(self.output, self.identity, command, 'running')
            if command in evidence.OUTPUTS:
                (self.output / evidence.OUTPUTS[command]).write_text('original')
            evidence.transition(self.output, self.identity, command, 'passed')
        self.assertTrue(evidence.finalize(self.output, self.identity, self.producer))
        qualification = self.producer / 'qualification.json'
        original = qualification.read_bytes()
        qualification.write_bytes(original + b'\n')
        with self.assertRaisesRegex(ValueError, 'producer bytes changed'):
            evidence.finalize(self.output, self.identity, self.producer)
        qualification.write_bytes(original)
        with self.assertRaisesRegex(ValueError, 'producer bytes changed'):
            evidence.finalize(self.output, self.identity, self.producer)
        self.assertEqual(self.read()['status'], 'rejected')

    def test_stale_consumer_identity_still_bounds_and_records_refusal(self):
        self.start()
        evidence.transition(self.output, self.identity, 'install', 'running')
        (self.output / 'explain.txt').write_bytes(b'x' * (evidence.MAX_OUTPUT + 1))
        stale = dict(self.identity, run_attempt=3)
        with self.assertRaisesRegex(ValueError, 'consumer identity mismatch'):
            evidence.finalize(self.output, stale, self.producer)
        receipt = self.read()
        self.assertEqual(receipt['identity'], self.identity)
        self.assertEqual(receipt['status'], 'rejected')
        self.assertEqual(receipt['commands'][0]['status'], 'failed')
        self.assertTrue(receipt['commands'][3]['output']['truncated'])
        self.assertEqual((self.output / 'explain.txt').stat().st_size, evidence.MAX_OUTPUT)

    def test_repeat_finalize_refusal_revokes_complete_without_rebinding_outputs(self):
        self.start()
        for command in evidence.COMMANDS:
            evidence.transition(self.output, self.identity, command, 'running')
            if command in evidence.OUTPUTS:
                (self.output / evidence.OUTPUTS[command]).write_text('original')
            evidence.transition(self.output, self.identity, command, 'passed')
        self.assertTrue(evidence.finalize(self.output, self.identity, self.producer))
        original = self.read()['commands'][2]['output']
        (self.output / 'check.json').write_text('replacement')
        with self.assertRaisesRegex(ValueError, 'passed output changed'):
            evidence.finalize(self.output, self.identity, self.producer)
        receipt = self.read()
        self.assertEqual(receipt['status'], 'rejected')
        self.assertEqual(receipt['commands'][2]['status'], 'failed')
        self.assertEqual(receipt['commands'][2]['output'], original)
        self.assertEqual(receipt['commands'][2]['observed_output']['sha256'],
                         hashlib.sha256(b'replacement').hexdigest())
        self.assertFalse(receipt['release_eligible'])
        (self.output / 'check.json').write_text('original')
        with self.assertRaises(ValueError):
            evidence.finalize(self.output, self.identity, self.producer)
        self.assertEqual(self.read()['status'], 'rejected')

    def test_oversized_pass_preserves_truncation_after_failure(self):
        self.start()
        for command in evidence.COMMANDS[:2]:
            evidence.transition(self.output, self.identity, command, 'running')
            evidence.transition(self.output, self.identity, command, 'passed')
        evidence.transition(self.output, self.identity, 'preview', 'running')
        (self.output / 'check.json').write_bytes(b'x' * (evidence.MAX_OUTPUT + 1))
        with self.assertRaisesRegex(ValueError, 'missing or oversized output'):
            evidence.transition(self.output, self.identity, 'preview', 'passed')
        self.assertFalse(evidence.finalize(self.output, self.identity, self.producer))
        self.assertTrue(self.read()['commands'][2]['output']['truncated'])
        self.assertEqual(self.read()['commands'][2]['status'], 'failed')

    def test_output_bound_and_no_overwrite(self):
        self.start()
        with self.assertRaises(FileExistsError):
            self.start()
        (self.output / 'check.json').write_bytes(b'x' * (evidence.MAX_OUTPUT + 1))
        self.assertFalse(evidence.finalize(self.output, self.identity, self.producer))
        retained = self.output / 'check.json'
        self.assertEqual(retained.stat().st_size, evidence.MAX_OUTPUT)

    def test_capture_refusal_still_bounds_other_outputs_and_revokes_complete(self):
        self.test_complete_and_retained_output()
        original = self.read()['commands'][2]['output']
        target = self.root / 'outside'
        target.write_text('do not read or modify')
        check = self.output / 'check.json'
        check.unlink()
        check.symlink_to(target)
        (self.output / 'explain.txt').write_bytes(b'x' * (evidence.MAX_OUTPUT + 1))
        with self.assertRaisesRegex(ValueError, 'symlink'):
            evidence.finalize(self.output, self.identity, self.producer)
        receipt = self.read()
        self.assertEqual(receipt['status'], 'rejected')
        self.assertEqual(receipt['commands'][2]['status'], 'failed')
        self.assertEqual(receipt['commands'][2]['output'], original)
        self.assertIn('removed without following', receipt['commands'][2]['output_error'])
        self.assertFalse(check.is_symlink())
        self.assertEqual(target.read_text(), 'do not read or modify')
        self.assertEqual((self.output / 'explain.txt').stat().st_size, evidence.MAX_OUTPUT)
        self.assertTrue(receipt['commands'][3]['observed_output']['truncated'])

    def test_observed_truncation_survives_repeat_finalization(self):
        self.test_complete_and_retained_output()
        original = self.read()['commands'][2]['output']
        (self.output / 'check.json').write_bytes(b'x' * (evidence.MAX_OUTPUT + 1))
        for _ in range(2):
            with self.assertRaises(ValueError):
                evidence.finalize(self.output, self.identity, self.producer)
            row = self.read()['commands'][2]
            self.assertEqual(row['output'], original)
            self.assertTrue(row['observed_output']['truncated'])
            self.assertEqual(row['status'], 'failed')


if __name__ == '__main__':
    unittest.main()
