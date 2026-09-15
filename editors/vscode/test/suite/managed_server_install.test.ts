import * as assert from 'assert';
import * as crypto from 'crypto';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import {
  combineActiveManagedServerIdentity,
  installManagedServer,
  ManagedServerInstallOperations,
  ManagedServerInstallRequest,
  readManagedServerInstallation
} from '../../src/managedServerInstall';

suite('Managed Server Installation', () => {
  let root: string;

  setup(async () => {
    root = await fs.promises.mkdtemp(path.join(os.tmpdir(), 'ripr-managed-install-'));
  });

  teardown(async () => {
    await fs.promises.rm(root, { recursive: true, force: true });
  });

  test('cache admission requires a matching complete receipt and executable digest', async () => {
    const request = installRequest(root, '1.2.3');
    const finalDir = path.join(request.serversRoot, request.version, request.platformTarget);
    await fs.promises.mkdir(finalDir, { recursive: true });
    await fs.promises.writeFile(path.join(finalDir, request.executableName), 'partial binary');

    assert.strictEqual(await readManagedServerInstallation(request), undefined);

    const installed = await installManagedServer(request, operations('binary-v1', '1.2.3'));
    assert.strictEqual(installed.receipt.installationState, 'complete');
    assert.strictEqual(installed.receipt.requestedVersion, '1.2.3');
    assert.strictEqual(await fs.promises.readFile(installed.executablePath, 'utf8'), 'binary-v1');

    await fs.promises.writeFile(installed.executablePath, 'tampered');
    assert.strictEqual(await readManagedServerInstallation(request), undefined);
  });

  test('concurrent installs stage once and converge on one completed installation', async () => {
    const request = installRequest(root, '2.0.0');
    let extractCalls = 0;
    const shared = operations('converged', '2.0.0', async () => {
      extractCalls += 1;
      await new Promise((resolve) => setTimeout(resolve, 100));
    });

    const [first, second] = await Promise.all([
      installManagedServer(request, shared),
      installManagedServer(request, shared)
    ]);

    assert.strictEqual(extractCalls, 1);
    assert.strictEqual(first.executablePath, second.executablePath);
    assert.deepStrictEqual(first.receipt, second.receipt);
    const siblings = await fs.promises.readdir(path.join(request.serversRoot, request.version));
    assert.deepStrictEqual(siblings, [request.platformTarget]);
  });

  test('failed upgrade leaves no eligible partial cache and preserves the prior usable version', async () => {
    const priorRequest = installRequest(root, '3.0.0');
    const prior = await installManagedServer(priorRequest, operations('prior-known-good', '3.0.0'));
    const upgradeRequest = installRequest(root, '3.1.0');
    const failing = operations('upgrade', '3.1.0');
    failing.probeExecutable = async () => {
      throw new Error('probe rejected upgrade');
    };

    await assert.rejects(installManagedServer(upgradeRequest, failing), /probe rejected upgrade/);

    const retained = await readManagedServerInstallation(priorRequest);
    assert.ok(retained);
    assert.strictEqual(retained.executablePath, prior.executablePath);
    assert.strictEqual(await fs.promises.readFile(retained.executablePath, 'utf8'), 'prior-known-good');
    assert.strictEqual(await readManagedServerInstallation(upgradeRequest), undefined);
    const versionEntries = await fs.promises.readdir(path.join(upgradeRequest.serversRoot, upgradeRequest.version));
    assert.deepStrictEqual(versionEntries, []);
  });

  test('rejects traversal and rooted version forms before any filesystem mutation', async () => {
    const invalidVersions = [
      '../escaped',
      '..\\..\\escaped',
      '1.2.3/../../escaped',
      '1.2.3\\..\\escaped',
      '/tmp/escaped',
      '\\server\\share',
      '\\\\server\\share',
      ['C:', '\\escaped'].join(''),
      '01.2.3',
      '.',
      '..'
    ];

    for (const version of invalidVersions) {
      const request = installRequest(root, version);
      await assert.rejects(installManagedServer(request, operations('never', version)), /Invalid ripr server version/);
    }

    assert.strictEqual(fs.existsSync(path.join(root, 'servers')), false);
    assert.strictEqual(fs.existsSync(path.join(root, 'escaped')), false);
    const valid = await installManagedServer(
      installRequest(root, '4.0.0-rc.1+build.7'),
      operations('valid-prerelease', '4.0.0-rc.1+build.7')
    );
    assert.strictEqual(valid.receipt.requestedVersion, '4.0.0-rc.1+build.7');
  });

  test('never reclaims a stale or replaced lock owned by another installer', async () => {
    const request = installRequest(root, '5.0.0');
    const versionDir = path.join(request.serversRoot, request.version);
    const finalDir = path.join(versionDir, request.platformTarget);
    const lockPath = `${finalDir}.install.lock`;
    await fs.promises.mkdir(versionDir, { recursive: true });
    await fs.promises.mkdir(lockPath);
    const ownerPath = path.join(lockPath, 'owner');
    await fs.promises.writeFile(ownerPath, 'stale-owner\n');
    const old = new Date(Date.now() - 60 * 60_000);
    await fs.promises.utimes(lockPath, old, old);
    let extractionCalls = 0;
    const blocked = operations('must-not-install', '5.0.0');
    blocked.lockWaitMs = 80;
    blocked.lockPollMs = 10;
    blocked.extractArchive = async () => {
      extractionCalls += 1;
    };

    const attempt = installManagedServer(request, blocked);
    await new Promise((resolve) => setTimeout(resolve, 20));
    await fs.promises.writeFile(ownerPath, 'fresh-replacement-owner\n');
    await assert.rejects(attempt, /Timed out waiting for managed server install lock/);

    assert.strictEqual(extractionCalls, 0);
    assert.strictEqual(await fs.promises.readFile(ownerPath, 'utf8'), 'fresh-replacement-owner\n');
    assert.strictEqual(await readManagedServerInstallation(request), undefined);
  });

  test('active probe version remains authoritative over receipt-time identity', async () => {
    const installation = await installManagedServer(
      installRequest(root, '6.0.0'),
      operations('identity-binary', '6.0.0', undefined, 'ripr receipt-time')
    );
    const active = { binaryVersion: 'ripr active-probe' };

    const combined = combineActiveManagedServerIdentity(active, installation);

    assert.strictEqual(installation.receipt.binaryVersion, 'ripr receipt-time');
    assert.strictEqual(combined.binaryVersion, 'ripr active-probe');
    assert.strictEqual(combined.assetDigest, installation.receipt.archiveSha256);
    assert.strictEqual(combined.installationState, 'complete');
  });

  test('authoritative provisioning guide matches the completed receipt contract', async () => {
    const repoRoot = path.resolve(__dirname, '../../../../..');
    const guide = await fs.promises.readFile(path.join(repoRoot, 'docs', 'SERVER_PROVISIONING.md'), 'utf8');
    const normalizedGuide = guide.replace(/\s+/g, ' ');

    assert.ok(guide.includes('install-receipt.json'));
    assert.ok(guide.includes('`installationState: "complete"`'));
    assert.ok(guide.includes('current executable SHA-256 matches'));
    assert.ok(normalizedGuide.includes('not a producer provenance attestation'));
    assert.strictEqual(guide.includes('sha256.txt'), false);
  });
});

function installRequest(root: string, version: string): ManagedServerInstallRequest {
  return {
    serversRoot: path.join(root, 'servers'),
    version,
    platformTarget: 'test-target',
    executableName: process.platform === 'win32' ? 'ripr.exe' : 'ripr',
    archiveExtension: 'test'
  };
}

function operations(
  executableContents: string,
  version: string,
  beforeExtract?: (() => Promise<void>) | undefined,
  reportedVersion = 'ripr 1.2.3'
): ManagedServerInstallOperations {
  const bytes = Buffer.from(`archive:${executableContents}`);
  return {
    resolveArchive: async () => ({
      manifestVersion: version,
      expectedSha256: crypto.createHash('sha256').update(bytes).digest('hex'),
      bytes
    }),
    extractArchive: async (_archivePath, destination) => {
      await beforeExtract?.();
      const executableName = process.platform === 'win32' ? 'ripr.exe' : 'ripr';
      await fs.promises.writeFile(path.join(destination, executableName), executableContents);
    },
    probeExecutable: async () => reportedVersion
  };
}
