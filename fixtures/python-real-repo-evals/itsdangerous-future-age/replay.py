"""Replay upstream behavior in fresh owned directories; never execute RIPR."""
import argparse
import hashlib
import json
from importlib import metadata
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parent
TIMED = 'src/itsdangerous/timed.py'
TEST = 'tests/test_itsdangerous/test_timed.py'
SELECTOR = TEST + '::TestTimestampSigner::test_future_age'
ORIGINAL = '''    def test_future_age(self, signer):
        signed = signer.sign("value")

        with freeze_time("1971-05-31"):
            with pytest.raises(SignatureExpired):
                signer.unsign(signed, max_age=10)
'''
WEAK = '''    def test_future_age(self, signer):
        signed = signer.sign("value")

        with freeze_time("1971-05-31"):
            assert signer.unsign(signed) == b"value"
'''


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def validate_source():
    inventory = json.loads((ROOT / 'retained-files.json').read_text())
    actual = {p.relative_to(ROOT / 'input').as_posix()
              for p in (ROOT / 'input').rglob('*') if p.is_file()}
    assert actual == {r['path'] for r in inventory}, 'unlisted or missing source'
    for row in inventory:
        raw = (ROOT / 'input' / row['path']).read_bytes()
        assert len(raw) == row['bytes'] and digest(raw) == row['sha256'], row['path']
        assert hashlib.sha1(f'blob {len(raw)}\0'.encode() + raw).hexdigest() == row['git_blob']
    broken = (ROOT / 'upstream/broken-timed.py').read_bytes()
    assert hashlib.sha1(f'blob {len(broken)}\0'.encode()+broken).hexdigest() == '2ae026195fdad135d8c4b828b64dd39cba8523e6'


def validate_environment():
    assert platform.python_implementation() == 'CPython' and sys.version_info[:2] == (3, 14), 'CPython 3.14 required'
    assert platform.system() == 'Windows' and sys.maxsize > 2**32, 'locked Windows x64 replay required'
    distributions = {}
    for line in (ROOT / 'requirements.lock').read_text().splitlines():
        if line and not line.startswith('#'):
            name, value = line.split('==', 1)
            pinned = value.split()[0]
            actual = metadata.version(name)
            assert actual == pinned, f'{name}: installed {actual}, locked {pinned}'
            distributions[name] = actual
    import pytest
    import freezegun
    assert pytest.__version__ == distributions['pytest']
    assert freezegun.__version__ == distributions['freezegun']
    return dict(python=platform.python_version(), implementation=platform.python_implementation(),
                system=platform.system(), pointer_bits=64, distributions=distributions,
                imported_modules={m.__name__: dict(version=m.__version__, init_sha256=digest(Path(m.__file__).read_bytes()))
                                  for m in [pytest, freezegun]})


def run_row(run_root, implementation, variant):
    work = run_root / (implementation + '-' + variant)
    shutil.copytree(ROOT / 'input', work)
    production = work / TIMED
    if implementation == 'broken':
        production.write_bytes((ROOT / 'upstream/broken-timed.py').read_bytes())
    test_path = work / TEST
    original = test_path.read_text(encoding='utf-8')
    assert original.count(ORIGINAL) == 1, 'wrong source test identity'
    if variant == 'weak':
        changed = original.replace(ORIGINAL, WEAK)
    elif variant in {'skip-method', 'xfail'}:
        mark = "skip(reason='activation control')" if variant == 'skip-method' else "xfail(strict=False, reason='activation control')"
        changed = original.replace(ORIGINAL, '    @pytest.mark.' + mark + '\n' + ORIGINAL)
    elif variant == 'skip-class':
        changed = original.replace('class TestTimestampSigner(', "@pytest.mark.skip(reason='activation control')\nclass TestTimestampSigner(")
    else:
        changed = original
    test_path.write_bytes(changed.encode('utf-8'))
    selectors = [SELECTOR]
    if variant == 'neighbors':
        selectors = [TEST + '::TestTimestampSigner::test_max_age', TEST + '::TestTimestampSigner::test_return_timestamp']
    if variant == 'full-file':
        selectors = [TEST]
    env = dict(os.environ, PYTEST_DISABLE_PLUGIN_AUTOLOAD='1', PYTHONDONTWRITEBYTECODE='1',
               PYTHONPATH=os.pathsep.join([str(work / 'src'), str(work / 'tests')]))
    argv = [sys.executable, '-m', 'pytest', '-q', '-o', 'addopts=', '--junitxml=result.xml', *selectors]
    proc = subprocess.run(argv, cwd=work, env=env, capture_output=True, timeout=30)
    (work/'stdout.raw').write_bytes(proc.stdout)
    (work/'stderr.raw').write_bytes(proc.stderr)
    tree = ET.parse(work/'result.xml')
    suites = list(tree.getroot().iter('testsuite'))
    assert len(suites) == 1, 'one expected suite must have run'
    suite = suites[0]
    cases = list(suite.iter('testcase'))
    row = dict(implementation=implementation, variant=variant, exit_code=proc.returncode,
               registered=int(suite.attrib['tests']), errors=int(suite.attrib['errors']),
               failures=int(suite.attrib['failures']), skipped=int(suite.attrib['skipped']),
               test_ids=[dict(classname=c.attrib['classname'], name=c.attrib['name'],
                              outcome='failure' if c.find('failure') is not None else
                              'skipped' if c.find('skipped') is not None else 'pass',
                              reason=c.find('skipped').attrib.get('type') if c.find('skipped') is not None else None)
                         for c in cases],
               production_sha256=digest(production.read_bytes()), test_sha256=digest(test_path.read_bytes()),
               stdout_sha256=digest(proc.stdout), stderr_sha256=digest(proc.stderr),
               junit_sha256=digest((work/'result.xml').read_bytes()),
               driver_sha256=digest(Path(__file__).read_bytes()),
               requirements_lock_sha256=digest((ROOT/'requirements.lock').read_bytes()),
               command=['<REPLAY_PYTHON>', *argv[1:]],
               raw_artifacts='<' + implementation + '-' + variant + '>/')
    row['executed'] = row['registered'] - row['skipped']
    assert row['registered'] == len(cases) > 0 and row['errors'] == 0, row
    if variant == 'effective' and implementation == 'broken':
        assert row['registered'] == row['failures'] == proc.returncode == 1, row
        assert b'DID NOT RAISE' in proc.stdout and b'SignatureExpired' in proc.stdout, 'wrong red location'
    else:
        assert row['failures'] == proc.returncode == 0, row
    if variant in {'skip-method', 'skip-class'}:
        assert row['registered'] == row['skipped'] == 1 and row['executed'] == 0, row
    elif variant == 'xfail' and implementation == 'broken':
        assert row['registered'] == row['skipped'] == 1 and row['test_ids'][0]['reason'] == 'pytest.xfail', row
        # xfail bodies ran; the runner's skipped bucket means a suppressed failure.
        row['executed'] = 1
    else:
        assert row['skipped'] == 0, row
    if variant not in {'neighbors', 'full-file'}:
        assert row['registered'] == 1 and row['test_ids'][0]['name'] == 'test_future_age', row
    if variant == 'neighbors':
        assert row['registered'] == 2, row
    print(json.dumps({k:row[k] for k in ['implementation','variant','registered','executed','failures','skipped','exit_code']}), flush=True)
    return row


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--work-dir', type=Path, required=True)
    args = parser.parse_args()
    validate_source()
    environment = validate_environment()
    args.work_dir.mkdir(parents=True, exist_ok=True)
    run_root = Path(tempfile.mkdtemp(prefix='itsdangerous-native-', dir=args.work_dir)).resolve()
    rows = [run_row(run_root, impl, variant) for impl in ['fixed', 'broken']
            for variant in ['effective', 'weak', 'skip-method', 'skip-class', 'xfail', 'neighbors']]
    rows.append(run_row(run_root, 'fixed', 'full-file'))
    receipt = dict(kind='upstream_native_test_understanding', schema_version='0.1',
                   python=platform.python_version(), platform=platform.platform(),
                   environment=environment,
                   fixed='c30678d19e37011890e2374cca04f7789e101793',
                   broken='1a9b8d1f9968eb248bda69f39d373a6330b692ba',
                   analyzer_execution='not_established', rows=rows,
                   non_claims=['historical_runtime','full_upstream_suite','installed_ripr','support_tier_promotion','population_rates'])
    (run_root/'receipt.json').write_text(json.dumps(receipt, indent=2)+'\n', encoding='utf-8')
    print('Retained raw run: ' + str(run_root), flush=True)


if __name__ == '__main__':
    main()
