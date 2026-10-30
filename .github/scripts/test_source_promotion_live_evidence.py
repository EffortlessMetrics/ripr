import copy
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import source_promotion_live_evidence as live

H = 'a' * 40
B = 'b' * 40
P = live.PREFIX

def responses():
    pr = {'number': 7, 'state': 'open', 'draft': False, 'user': {'login': 'writer'},
          'head': {'sha': H, 'repo': {'full_name': live.REPOSITORY}},
          'base': {'sha': B, 'ref': 'main', 'repo': {'full_name': live.REPOSITORY}}}
    review = {'id': 10, 'submitted_at': '2026-10-30T01:02:03Z', 'user': {'login': 'reviewer'},
              'state': 'APPROVED', 'commit_id': H, 'author_association': 'MEMBER'}
    check = {'id': 33, 'name': 'Ripr Rust Small Result', 'head_sha': H, 'status': 'completed',
             'conclusion': 'success', 'app': {'slug': 'github-actions', 'id': 15368}, 'check_suite': {'id': 77},
             'details_url': 'https://github.com/EffortlessMetrics/ripr/actions/runs/44/job/55'}
    job = {'id': 55, 'run_id': 44, 'name': check['name'], 'status': 'completed', 'conclusion': 'success',
           'check_run_url': f'https://api.github.com/repos/{live.REPOSITORY}/check-runs/33', 'head_sha': H, 'run_attempt': 1}
    run = {'id': 44, 'workflow_id': 66, 'head_sha': H, 'event': 'pull_request', 'status': 'completed',
           'conclusion': 'success', 'repository': {'full_name': live.REPOSITORY},
           'head_repository': {'full_name': live.REPOSITORY}, 'path': live.WORKFLOW, 'run_attempt': 1, 'check_suite_id': 77,
           'pull_requests': [{'number': 7, 'head': {'sha': H, 'repo': {'url': f'https://api.github.com/repos/{live.REPOSITORY}'}},
                              'base': {'sha': B, 'ref': 'main', 'repo': {'url': f'https://api.github.com/repos/{live.REPOSITORY}'}}}]}
    return {P+'pulls/7': pr, P+'git/ref/heads/main': {'object': {'sha': B}},
            P+'pulls/7/reviews?per_page=100&page=1': [review],
            P+f'commits/{H}/check-runs?per_page=100&page=1': {'total_count': 1, 'check_runs': [check]},
            P+'actions/jobs/55': job, P+'actions/runs/44': run,
            P+'actions/workflows/66': {'id': 66, 'path': live.WORKFLOW}}

def reader(data):
    def read(path):
        if path not in data: raise ValueError('unavailable')
        return copy.deepcopy(data[path])
    return read

class LiveEvidenceTests(unittest.TestCase):
    def test_mocked_positive_is_only_observed_subset(self):
        result = live.verify(reader(responses()), 7, H, B)
        self.assertEqual(result['status'], 'observed_subset')
        self.assertIn('No complete required-result policy', result['non_claim'])

    def test_reject_missing_stale_review_and_workflow_adversaries(self):
        cases = []
        d = responses(); d[P+'pulls/7']['head']['sha'] = 'c'*40; cases.append(d)
        d = responses(); d[P+'git/ref/heads/main']['object']['sha'] = 'c'*40; cases.append(d)
        d = responses(); d[P+'pulls/7/reviews?per_page=100&page=1'] = []; cases.append(d)
        d = responses(); d[P+'pulls/7/reviews?per_page=100&page=1'][0]['state'] = 'DISMISSED'; cases.append(d)
        d = responses(); d[P+f'commits/{H}/check-runs?per_page=100&page=1']['check_runs'] = []; cases.append(d)
        d = responses(); d[P+f'commits/{H}/check-runs?per_page=100&page=1']['check_runs'][0]['head_sha'] = 'c'*40; cases.append(d)
        d = responses(); d[P+'actions/runs/44']['head_sha'] = 'c'*40; cases.append(d)
        d = responses(); d[P+'actions/runs/44']['pull_requests'][0]['number'] = 8; cases.append(d)
        d = responses(); d[P+'actions/runs/44']['pull_requests'][0]['base']['sha'] = 'c'*40; cases.append(d)
        d = responses(); d[P+'actions/runs/44']['pull_requests'] = []; cases.append(d)
        d = responses(); d[P+'actions/runs/44']['conclusion'] = 'failure'; cases.append(d)
        d = responses(); d[P+'actions/jobs/55']['run_attempt'] = 2; cases.append(d)
        d = responses(); d[P+'actions/jobs/55']['head_sha'] = 'c'*40; cases.append(d)
        d = responses(); d[P+'actions/runs/44']['check_suite_id'] = 78; cases.append(d)
        d = responses(); d[P+'actions/runs/44']['path'] = '.github/workflows/other.yml'; cases.append(d)
        d = responses(); d[P+f'commits/{H}/check-runs?per_page=100&page=1']['check_runs'][0]['app']['id'] = 42; cases.append(d)
        d = responses(); d[P+'actions/workflows/66']['path'] = '.github/workflows/other.yml'; cases.append(d)
        d = responses(); d[P+'actions/runs/44']['workflow_id'] = '../bad'; cases.append(d)
        d = responses(); del d[P+'actions/jobs/55']; cases.append(d)
        for sample in cases:
            with self.subTest(sample=sample), self.assertRaises(ValueError): live.verify(reader(sample), 7, H, B)

    def test_reject_change_between_observations(self):
        data = responses(); count = 0
        def changing(path):
            nonlocal count
            if path == P+'git/ref/heads/main':
                count += 1
                if count == 2: return {'object': {'sha': 'c'*40}}
            return reader(data)(path)
        with self.assertRaises(ValueError): live.verify(changing, 7, H, B)

    def test_reject_unbounded_pagination_and_bad_subject_before_read(self):
        with self.assertRaises(ValueError): live.pages(lambda _: [1]*100, P+'pulls/7/reviews')
        with self.assertRaises(ValueError): live.verify(lambda _: self.fail('unexpected read'), 7, '../bad', B)

    def test_stubbed_cli_reads_only_expected_nonadministrative_endpoints(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'responses.json').write_text(json.dumps(responses()))
            stub = root / 'gh'
            stub.write_text('#!' + sys.executable + '\nimport json, os, sys\np=sys.argv[-1]\nwith open(os.environ["CALLS"], "a") as f: f.write(p+"\\n")\nd=json.load(open(os.environ["RESPONSES"]))\nif p not in d: sys.exit(1)\nprint(json.dumps(d[p]))\n')
            stub.chmod(0o755)
            env = dict(os.environ, PATH=str(root) + os.pathsep + os.environ['PATH'],
                       CALLS=str(root/'calls'), RESPONSES=str(root/'responses.json'), PYTHONDONTWRITEBYTECODE='1')
            result = subprocess.run([sys.executable, str(Path(live.__file__)), '7', H, B],
                                    env=env, capture_output=True, text=True, timeout=30)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(json.loads(result.stdout)['status'], 'observed_subset')
            calls = (root/'calls').read_text().splitlines()
            self.assertEqual(len(calls), 14)
            self.assertTrue(all(path in responses() and 'protection' not in path and 'rulesets' not in path for path in calls))

if __name__ == '__main__': unittest.main()
