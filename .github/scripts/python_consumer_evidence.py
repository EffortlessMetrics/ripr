"""Bounded diagnostic evidence for the existing checkout-free wheel consumers.

This recorder does not install packages, execute native code, or grant admission.
A running row left by interruption is never a passed row. Runner loss can still
prevent upload; absence of an artifact is not evidence of completion.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path

COMMANDS = ('install', 'native_identity', 'preview', 'explain', 'disable', 'reinstall', 'uninstall')
OUTPUTS = {'preview': 'check.json', 'explain': 'explain.txt', 'disable': 'disabled.json'}
MAX_OUTPUT = 1024 * 1024


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(output, receipt):
    temporary = output / 'consumer.json.tmp'
    temporary.write_text(json.dumps(receipt, indent=2) + '\n')
    temporary.replace(output / 'consumer.json')


def producer_identity(producer, identity):
    qualification = json.loads((producer / 'qualification.json').read_text())
    native = json.loads((producer / 'wheel-receipt.json').read_text())
    require(identity['client'] in ('pip', 'uv'), 'unknown client')
    for key in ('source_sha', 'run_id', 'run_attempt', 'wheel_sha256', 'native_version', 'version'):
        require(qualification[key] == identity[key], f'producer {key} mismatch')
    require(qualification['repository'] == native['source_repository'] == 'EffortlessMetrics/ripr', 'repository mismatch')
    require(qualification['source_sha'] == native['candidate_sha'], 'source mismatch')
    require(qualification['source_tree'] == native['candidate_tree'], 'tree mismatch')
    require(str(qualification['run_id']) == native['producer_run_id'], 'run mismatch')
    require(str(qualification['run_attempt']) == native['producer_run_attempt'], 'attempt mismatch')
    require(qualification['wheel_filename'] == native['wheel']['filename'], 'wheel filename mismatch')
    require(qualification['wheel_sha256'] == native['wheel']['sha256'], 'wheel digest mismatch')
    require(identity['native_sha256'] == native['wheel']['installed_payload_sha256'], 'native digest mismatch')
    require(identity['native_version'] == native['native_version'], 'native version mismatch')
    require(identity['version'] == native['python_version'], 'Python version mismatch')
    require(qualification['target'] == 'x86_64-unknown-linux-gnu', 'unsupported target')
    require(qualification['platform_tag'] == 'py3-none-manylinux_2_34_x86_64', 'unsupported platform')
    return {name: digest(producer / name) for name in ('qualification.json', 'wheel-receipt.json')}


def initialize(producer, output, identity):
    output.mkdir(parents=True, exist_ok=False)
    receipt = dict(schema_version=1, identity=identity, producer=None,
                   scope='linux-x64-glibc-only', release_eligible=False,
                   status='partial', commands=[dict(command=name, selected=True,
                   executed=False, status='not_run', output=None) for name in COMMANDS])
    write(output, receipt)
    try:
        receipt['producer'] = producer_identity(producer, identity)
    except (ValueError, KeyError, OSError) as error:
        receipt.update(status='rejected', error=str(error))
        write(output, receipt)
        raise
    write(output, receipt)


def validate_receipt(receipt, identity):
    require(receipt['identity'] == identity, 'consumer identity mismatch')
    require([row['command'] for row in receipt['commands']] == list(COMMANDS), 'command denominator mismatch')
    require(receipt['release_eligible'] is False, 'diagnostics cannot grant admission')


def read(output, identity):
    receipt = json.loads((output / 'consumer.json').read_text())
    validate_receipt(receipt, identity)
    return receipt


def capture(output, command):
    if command not in OUTPUTS:
        return None
    path = output / OUTPUTS[command]
    require(not path.is_symlink(), 'output must not be a symlink')
    if not path.exists():
        return None
    size = path.stat().st_size
    with path.open('rb') as stream:
        data = stream.read(MAX_OUTPUT)
    if size > MAX_OUTPUT:
        path.write_bytes(data)
    return dict(path=path.name, bytes=len(data), sha256=hashlib.sha256(data).hexdigest(),
                truncated=size > MAX_OUTPUT)


def transition(output, identity, command, state):
    receipt = read(output, identity)
    index = COMMANDS.index(command)
    row = receipt['commands'][index]
    if state == 'running':
        require(row['status'] == 'not_run', 'command already started')
        require(all(r['status'] == 'passed' for r in receipt['commands'][:index]), 'prior command incomplete')
        row.update(executed=True, status='running')
    else:
        require(state in ('passed', 'failed') and row['status'] == 'running', 'invalid terminal transition')
        captured = capture(output, command)
        if state == 'passed' and command in OUTPUTS:
            if captured is None or captured['bytes'] == 0 or captured['truncated']:
                row.update(status='failed', output=captured)
                write(output, receipt)
                raise ValueError('missing or oversized output')
        row.update(status=state, output=captured)
    write(output, receipt)


def finalize(output, identity, producer):
    # Bound every permitted output before any identity refusal can abort cleanup.
    captures, capture_errors = {}, {}
    for command, filename in OUTPUTS.items():
        try:
            captures[command] = capture(output, command)
        except (ValueError, OSError) as refusal:
            capture_errors[command] = str(refusal)
            path = output / filename
            if path.is_symlink():
                # Upload must not follow a refused link outside the evidence.
                try:
                    path.unlink()
                    capture_errors[command] += '; symlink removed without following'
                except OSError as removal_error:
                    capture_errors[command] += f'; removal failed: {removal_error}'
    receipt = json.loads((output / 'consumer.json').read_text())
    error = receipt.get('error') if receipt.get('status') == 'rejected' else None
    try:
        validate_receipt(receipt, identity)
        require(receipt['producer'] == producer_identity(producer, identity), 'producer bytes changed')
    except (ValueError, KeyError, TypeError, OSError) as refusal:
        error = error or str(refusal)
    error = error or next(iter(capture_errors.values()), None)
    for row in receipt['commands']:
        captured = captures.get(row['command'])
        previous = row['output']
        # A second capture of already bounded bytes must retain truncation history.
        for history in (previous, row.get('observed_output')):
            if history and captured and history.get('truncated'):
                if all(history[key] == captured[key] for key in ('path', 'bytes', 'sha256')):
                    captured['truncated'] = True
        if row['command'] in capture_errors:
            row['output_error'] = capture_errors[row['command']]
            row['observed_output'] = None
            row['status'] = 'failed'
            continue
        if previous is not None and captured != previous:
            error = error or ('passed output changed' if row['status'] == 'passed'
                              else 'recorded output changed')
            # Keep the original binding; changed bytes are observations, not proof.
            row['observed_output'] = captured
            row['status'] = 'failed'
        else:
            row['output'] = captured
        if row['status'] == 'running' or (captured and captured['truncated']):
            row['status'] = 'failed'
        if row['status'] == 'passed' and row['command'] in OUTPUTS:
            if not captured or not captured['bytes']:
                error = error or 'passed output disappeared'
                row['status'] = 'failed'
    complete = not error and all(row['selected'] and row['executed'] and row['status'] == 'passed'
                                for row in receipt['commands'])
    receipt['release_eligible'] = False
    receipt['status'] = 'rejected' if error else ('complete' if complete else 'partial')
    if error:
        receipt['error'] = error
    write(output, receipt)
    if error:
        raise ValueError(error)
    return complete


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=('init', 'running', 'passed', 'failed', 'finalize'))
    parser.add_argument('command', nargs='?', choices=COMMANDS)
    args = parser.parse_args()
    identity = dict(source_sha=os.environ['CANDIDATE_SHA'].lower(),
                    run_id=int(os.environ['GITHUB_RUN_ID']), run_attempt=int(os.environ['GITHUB_RUN_ATTEMPT']),
                    wheel_sha256=os.environ['EXPECTED_WHEEL_SHA256'], native_sha256=os.environ['EXPECTED_PAYLOAD_SHA256'],
                    native_version=os.environ['RIPR_NATIVE_VERSION'], version=os.environ['RIPR_PYTHON_VERSION'],
                    client=os.environ['CLIENT'])
    output = Path(os.environ['CONSUMER_EVIDENCE'])
    producer = Path('evidence')
    if args.action == 'init':
        initialize(producer, output, identity)
    elif args.action == 'finalize':
        raise SystemExit(0 if finalize(output, identity, producer) else 1)
    else:
        require(args.command is not None, 'command required')
        transition(output, identity, args.command, args.action)


if __name__ == '__main__':
    main()
