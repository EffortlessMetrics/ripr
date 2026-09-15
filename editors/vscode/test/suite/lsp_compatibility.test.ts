import * as assert from 'assert';
import * as crypto from 'crypto';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import { probeStandardLspCompatibility } from '../../src/lspCompatibility';
import { probeServerVersion } from '../../src/serverResolver';

suite('Standard LSP compatibility probe', () => {
  const fakeProbeTimeoutMs = 8000;
  const temporaryRoots: string[] = [];

  teardown(() => {
    for (const root of temporaryRoots.splice(0)) {
      fs.rmSync(root, { recursive: true, force: true });
    }
  });

  test('accepts the real packaged server through initialize and shutdown', async function () {
    const server = process.env.RIPR_TEST_PACKAGED_SERVER_PATH;
    if (!server) {
      this.skip();
      return;
    }
    const expectedDigest = process.env.RIPR_TEST_PACKAGED_SERVER_SHA256;
    assert.ok(expectedDigest, 'packaged server proof requires the archive extraction digest');
    const actualDigest = crypto.createHash('sha256').update(fs.readFileSync(server)).digest('hex');
    assert.strictEqual(actualDigest, expectedDigest, 'the probed executable must be the extracted archive member');
    const result = await probeStandardLspCompatibility(server, false, 10_000);
    assert.strictEqual(result.status, 'compatible', JSON.stringify(result));
    if (result.status === 'compatible') {
      assert.strictEqual(result.serverName, 'ripr');
      assert.strictEqual(result.positionEncoding, 'utf-16');
      assert.strictEqual(result.processResult, 'clean_exit');
    }
  });

  test('rejects a version-only executable that cannot frame LSP', async () => {
    const fake = fakeServer('version-only');
    const result = await probeStandardLspCompatibility(fake.command, fake.useShell, fakeProbeTimeoutMs);
    assert.strictEqual(result.status, 'incompatible');
    assert.ok(result.status === 'incompatible' && ['framing_failure', 'process_failure'].includes(result.kind));
  });

  test('requires the exercised baseline but records genuinely optional omissions', async () => {
    const missing = fakeServer('missing-hover');
    const rejected = await probeStandardLspCompatibility(missing.command, missing.useShell, fakeProbeTimeoutMs);
    assert.deepStrictEqual(
      rejected.status === 'incompatible' ? rejected.kind : undefined,
      'missing_required_capability'
    );

    const optional = fakeServer('valid');
    const accepted = await probeStandardLspCompatibility(optional.command, optional.useShell, fakeProbeTimeoutMs);
    assert.strictEqual(accepted.status, 'compatible', JSON.stringify(accepted));
    if (accepted.status === 'compatible') {
      assert.deepStrictEqual(accepted.optional, {
        codeActionResolve: false,
        workDoneProgress: false
      });
    }
  });

  for (const mode of ['missing-diagnostics', 'missing-workspace-folders', 'missing-command'] as const) {
    test(`rejects the active-client baseline omission ${mode}`, async () => {
      const fake = fakeServer(mode);
      const result = await probeStandardLspCompatibility(fake.command, fake.useShell, fakeProbeTimeoutMs);
      assert.strictEqual(result.status, 'incompatible');
      assert.deepStrictEqual(result.status === 'incompatible' ? result.kind : undefined, 'missing_required_capability');
    });
  }

  for (const mode of [
    'missing-jsonrpc',
    'wrong-jsonrpc',
    'missing-response-payload',
    'wrong-response-id',
    'initialize-result-and-error',
    'initialize-malformed-error',
    'shutdown-result-and-error',
    'shutdown-malformed-error',
    'shutdown-non-null-result'
  ] as const) {
    test(`rejects the invalid JSON-RPC envelope ${mode}`, async () => {
      const fake = fakeServer(mode);
      const result = await probeStandardLspCompatibility(fake.command, fake.useShell, fakeProbeTimeoutMs);
      assert.strictEqual(result.status, 'incompatible');
      assert.deepStrictEqual(result.status === 'incompatible' ? result.kind : undefined, 'protocol_failure');
    });
  }

  test('accepts a structurally valid initialize error as an initialize rejection', async () => {
    const fake = fakeServer('initialize-error');
    const result = await probeStandardLspCompatibility(fake.command, fake.useShell, fakeProbeTimeoutMs);
    assert.deepStrictEqual(result.status === 'incompatible' ? result.kind : undefined, 'initialize_error');
  });

  test('POSIX version timeout terminates the spawned process group', async function () {
    if (process.platform === 'win32') {
      this.skip();
      return;
    }
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'ripr-version-tree-'));
    temporaryRoots.push(root);
    const pidPath = path.join(root, 'descendant.pid');
    const script = path.join(root, 'ripr');
    fs.writeFileSync(
      script,
      `#!/bin/sh\n(sleep 30) &\necho $! > "${pidPath}"\nwait\n`,
      { mode: 0o755 }
    );
    const result = await probeServerVersion(script, 'fixture', false, 300);
    assert.ok('message' in result && result.message.includes('did not respond'));
    const descendantPid = Number(fs.readFileSync(pidPath, 'utf8').trim());
    await new Promise((resolve) => setTimeout(resolve, 100));
    assert.throws(() => process.kill(descendantPid, 0), (error: NodeJS.ErrnoException) => error.code === 'ESRCH');
  });

  for (const [mode, succeeds] of [
    ['version-descendant-success', true],
    ['version-descendant-nonzero', false]
  ] as const) {
    test(`version ${mode} cannot leave its descendant alive after direct exit`, async () => {
      const pidPath = descendantPidPath('ripr-version-exit-tree-');
      const fake = fakeServer(mode, pidPath);
      const result = await probeServerVersion(fake.command, 'fixture', fake.useShell, 3000);
      assert.strictEqual('binaryVersion' in result, succeeds, JSON.stringify(result));
      await assertDescendantStopped(pidPath);
    });
  }

  for (const [mode, expected] of [
    ['lsp-descendant-crash', 'process_failure'],
    ['lsp-descendant-protocol', 'protocol_failure'],
    ['lsp-descendant-timeout', 'initialize_timeout']
  ] as const) {
    test(`LSP ${mode} cannot leave its descendant alive`, async () => {
      const pidPath = descendantPidPath('ripr-lsp-failure-tree-');
      const fake = fakeServer(mode, pidPath);
      const result = await probeStandardLspCompatibility(
        fake.command,
        fake.useShell,
        mode === 'lsp-descendant-timeout' ? 3000 : fakeProbeTimeoutMs
      );
      assert.deepStrictEqual(result.status === 'incompatible' ? result.kind : undefined, expected);
      await assertDescendantStopped(pidPath);
    });
  }

  test('normal LSP shutdown cannot leave a descendant alive after the server exits first', async () => {
    const pidPath = descendantPidPath('ripr-lsp-clean-tree-');
    const fake = fakeServer('lsp-descendant-clean-exit', pidPath);
    const result = await probeStandardLspCompatibility(fake.command, fake.useShell, fakeProbeTimeoutMs);
    assert.strictEqual(result.status, 'compatible', JSON.stringify(result));
    await assertDescendantStopped(pidPath);
  });

  test('rejects a server that selects a non-UTF-16 position encoding', async () => {
    const fake = fakeServer('utf8');
    const result = await probeStandardLspCompatibility(fake.command, fake.useShell, fakeProbeTimeoutMs);
    assert.deepStrictEqual(
      result.status === 'incompatible' ? result.kind : undefined,
      'position_encoding_mismatch'
    );
  });

  test('rejects a standard-capable process that does not identify as ripr', async () => {
    const fake = fakeServer('wrong-identity');
    const result = await probeStandardLspCompatibility(fake.command, fake.useShell, fakeProbeTimeoutMs);
    assert.deepStrictEqual(
      result.status === 'incompatible' ? result.kind : undefined,
      'server_identity_mismatch'
    );
  });

  for (const [mode, expected] of [
    ['timeout', 'initialize_timeout'],
    ['malformed', 'framing_failure'],
    ['partial', 'process_failure'],
    ['crash', 'process_failure'],
    ['nonzero', 'process_failure'],
    ['shutdown-failure', 'shutdown_failure']
  ] as const) {
    test(`classifies and cleans up ${mode}`, async () => {
      const fake = fakeServer(mode);
      const result = await probeStandardLspCompatibility(
        fake.command,
        fake.useShell,
        mode === 'timeout' ? 3000 : fakeProbeTimeoutMs
      );
      assert.strictEqual(result.status, 'incompatible');
      assert.deepStrictEqual(result.status === 'incompatible' ? result.kind : undefined, expected);
    });
  }

  test('a later compatible channel can be tried after an incompatible candidate', async function () {
    this.timeout(25_000);
    const first = fakeServer('missing-hover');
    const second = fakeServer('valid');
    const attempts = [first, second];
    let selected: number | undefined;
    for (let index = 0; index < attempts.length; index += 1) {
      const candidate = attempts[index];
      const result = await probeStandardLspCompatibility(candidate.command, candidate.useShell, fakeProbeTimeoutMs);
      if (result.status === 'compatible') {
        selected = index;
        break;
      }
    }
    assert.strictEqual(selected, 1);
  });

  function descendantPidPath(prefix: string): string {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), prefix));
    temporaryRoots.push(root);
    return path.join(root, 'descendant.pid');
  }

  function fakeServer(mode: string, descendantPath?: string): { command: string; useShell: boolean } {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'ripr-lsp-probe-'));
    temporaryRoots.push(root);
    const script = path.join(root, 'server.js');
    fs.writeFileSync(script, fakeServerSource(mode, descendantPath));
    if (process.platform === 'win32') {
      const command = path.join(root, 'ripr.cmd');
      fs.writeFileSync(command, `@echo off\r\n"${process.execPath}" "${script}" %*\r\nexit /b %errorlevel%\r\n`);
      return { command, useShell: true };
    }
    const command = path.join(root, 'ripr');
    fs.writeFileSync(command, `#!/bin/sh\nexec "${process.execPath}" "${script}" "$@"\n`, { mode: 0o755 });
    return { command, useShell: false };
  }
});

async function assertDescendantStopped(pidPath: string): Promise<void> {
  assert.ok(fs.existsSync(pidPath), `fixture did not record a descendant pid at ${pidPath}`);
  const pid = Number(fs.readFileSync(pidPath, 'utf8').trim());
  assert.ok(Number.isSafeInteger(pid) && pid > 0, `fixture recorded invalid descendant pid ${String(pid)}`);
  const deadline = Date.now() + 3000;
  while (Date.now() < deadline) {
    try {
      process.kill(pid, 0);
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'ESRCH') {
        return;
      }
      throw error;
    }
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  assert.fail(`probe descendant ${pid} remained after process-tree cleanup`);
}

function fakeServerSource(mode: string, descendantPidPath?: string): string {
  return `
const cp = require('child_process');
const fs = require('fs');
const mode = ${JSON.stringify(mode)};
const descendantPidPath = ${JSON.stringify(descendantPidPath)};
function spawnDescendant() {
  const descendant = cp.spawn(process.execPath, ['-e', 'setInterval(() => {}, 1000)'], {
    detached: process.platform === 'win32',
    stdio: 'ignore'
  });
  descendant.unref();
  fs.writeFileSync(descendantPidPath, String(descendant.pid));
}
if (process.argv.includes('--version')) {
  if (mode === 'version-descendant-success' || mode === 'version-descendant-nonzero') spawnDescendant();
  console.log('ripr 9.9.9');
  process.exit(mode === 'version-descendant-nonzero' ? 17 : 0);
}
if (mode === 'version-only') { process.stdout.write('not lsp'); process.exit(0); }
if (mode === 'timeout') { setInterval(() => {}, 1000); return; }
if (mode === 'malformed') { process.stdout.write('Content-Length: nope\\r\\n\\r\\n{}'); return; }
if (mode === 'partial') { process.stdout.write('Content-Length: 100\\r\\n\\r\\n{'); process.exit(0); }
if (mode === 'crash') { process.exit(0); }
if (mode === 'nonzero') { process.exit(17); }
if (mode.startsWith('lsp-descendant-')) {
  spawnDescendant();
  if (mode === 'lsp-descendant-crash') process.exit(17);
  if (mode === 'lsp-descendant-timeout') { setInterval(() => {}, 1000); return; }
}
let input = Buffer.alloc(0);
process.stdin.on('data', chunk => { input = Buffer.concat([input, chunk]); consume(); });
function send(value) {
  const body = Buffer.from(JSON.stringify(value));
  process.stdout.write('Content-Length: ' + body.length + '\\r\\n\\r\\n');
  process.stdout.write(body);
}
function consume() {
  while (true) {
    const end = input.indexOf('\\r\\n\\r\\n'); if (end < 0) return;
    const header = input.subarray(0, end).toString();
    const match = /Content-Length:\\s*(\\d+)/i.exec(header); if (!match) process.exit(2);
    const length = Number(match[1]); const start = end + 4; if (input.length < start + length) return;
    const message = JSON.parse(input.subarray(start, start + length).toString()); input = input.subarray(start + length);
    if (message.method === 'initialize') {
      const commands = ['ripr.refresh','ripr.collectContext','ripr.collectEvidenceContext','ripr.collectWorkspaceStatus','ripr.collectRepairPacket','ripr.collectTopLimitation','ripr.collectReceiptStatus'];
      const capabilities = { textDocumentSync: 1, hoverProvider: true, codeActionProvider: true, diagnosticProvider: {}, executeCommandProvider: { commands }, workspace: { workspaceFolders: { supported: true } }, positionEncoding: mode === 'utf8' ? 'utf-8' : 'utf-16' };
      if (mode === 'missing-hover') delete capabilities.hoverProvider;
      if (mode === 'missing-diagnostics') delete capabilities.diagnosticProvider;
      if (mode === 'missing-workspace-folders') delete capabilities.workspace;
      if (mode === 'missing-command') capabilities.executeCommandProvider.commands.pop();
      const envelope = { jsonrpc: mode === 'wrong-jsonrpc' ? '1.0' : '2.0', id: message.id, result: { capabilities, serverInfo: { name: mode === 'wrong-identity' ? 'other' : 'ripr', version: '9.9.9' } } };
      if (mode === 'missing-jsonrpc') delete envelope.jsonrpc;
      if (mode === 'missing-response-payload') delete envelope.result;
      if (mode === 'wrong-response-id') envelope.id = 99;
      if (mode === 'initialize-result-and-error' || mode === 'lsp-descendant-protocol') envelope.error = null;
      if (mode === 'initialize-malformed-error') { delete envelope.result; envelope.error = { code: '-1' }; }
      if (mode === 'initialize-error') { delete envelope.result; envelope.error = { code: -32000, message: 'not compatible' }; }
      send(envelope);
    } else if (message.method === 'shutdown') {
      if (mode === 'shutdown-failure') send({ jsonrpc: '2.0', id: message.id, error: { code: -1, message: 'no' } });
      else if (mode === 'shutdown-result-and-error') send({ jsonrpc: '2.0', id: message.id, result: null, error: null });
      else if (mode === 'shutdown-malformed-error') send({ jsonrpc: '2.0', id: message.id, error: { code: -1 } });
      else if (mode === 'shutdown-non-null-result') send({ jsonrpc: '2.0', id: message.id, result: {} });
      else send({ jsonrpc: '2.0', id: message.id, result: null });
    } else if (message.method === 'exit') { process.exit(0); }
  }
}
`;
}
