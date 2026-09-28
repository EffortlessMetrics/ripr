import * as assert from 'assert';
import * as crypto from 'crypto';
import * as fs from 'fs';
import * as https from 'https';
import * as os from 'os';
import * as path from 'path';
import * as vscode from 'vscode';
import { RiprConfig } from '../../src/config';
import {
  ManifestBytesFetcher,
  ManifestFetchOutcome,
  ManifestPlacementRequest,
  downloadServer,
  manifestPlacementPlan,
  placementArchiveUrl,
  resolveServerManifestPlacement
} from '../../src/downloader';
import { RiprPlatform } from '../../src/platform';
import { ServerDistributionDescriptor } from '../../src/serverDescriptor';

interface FixtureRoute {
  readonly status: number;
  readonly body?: Buffer;
  readonly location?: string;
}

interface FixtureRequest {
  readonly method: string;
  readonly path: string;
}

let generatedFixtureTls: { readonly key: Buffer; readonly cert: Buffer } | undefined;

// A one-day self-signed localhost certificate. The committed pair this
// replaced was the same shape (CN 127.0.0.1, SAN localhost) and was not a
// credential for any other host. Nothing in the suite pins the certificate
// bytes; TLS verification is disabled for the suite.
function fixtureTlsMaterial(): Promise<{ readonly key: Buffer; readonly cert: Buffer }> {
  if (generatedFixtureTls) {
    return Promise.resolve(generatedFixtureTls);
  }
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'ripr-fixture-tls-'));
  const keyPath = path.join(dir, 'key.pem');
  const certPath = path.join(dir, 'cert.pem');
  return run('openssl', [
    'req',
    '-x509',
    '-newkey',
    'rsa:2048',
    '-sha256',
    '-days',
    '1',
    '-nodes',
    '-keyout',
    keyPath,
    '-out',
    certPath,
    '-subj',
    '/CN=127.0.0.1',
    '-addext',
    'subjectAltName=DNS:localhost,IP:127.0.0.1'
  ])
    .then(() => {
      generatedFixtureTls = {
        key: fs.readFileSync(keyPath),
        cert: fs.readFileSync(certPath)
      };
      return generatedFixtureTls;
    })
    .finally(() => {
      fs.rmSync(dir, { recursive: true, force: true });
    });
}

class ManifestFixtureServer {
  readonly requests: FixtureRequest[] = [];

  private readonly routes = new Map<string, FixtureRoute>();
  private server: https.Server | undefined;
  private port = 0;

  route(routePath: string, route: FixtureRoute): void {
    this.routes.set(routePath, route);
  }

  get base(): string {
    return `https://127.0.0.1:${this.port}`;
  }

  async start(): Promise<void> {
    const { key, cert } = await fixtureTlsMaterial();
    return new Promise((resolve, reject) => {
      const created = https.createServer({ key, cert }, (request, response) => {
        const requestPath = (request.url ?? '/').split('?')[0];
        this.requests.push({ method: request.method ?? 'GET', path: requestPath });
        const route = this.routes.get(requestPath);
        if (!route) {
          response.writeHead(404);
          response.end();
          return;
        }
        if (route.location) {
          response.writeHead(route.status, { location: route.location });
          response.end();
          return;
        }
        response.writeHead(route.status, { 'content-type': 'application/octet-stream' });
        response.end(route.body ?? Buffer.alloc(0));
      });
      created.on('error', reject);
      created.listen(0, '127.0.0.1', () => {
        const address = created.address();
        if (!address || typeof address === 'string') {
          reject(new Error('fixture manifest server did not bind a TCP port'));
          return;
        }
        this.port = address.port;
        this.server = created;
        resolve();
      });
    });
  }

  stop(): Promise<void> {
    const server = this.server;
    if (!server) {
      return Promise.resolve();
    }
    return new Promise((resolve, reject) => {
      server.close((error) => (error ? reject(error) : resolve()));
      server.closeAllConnections();
    });
  }
}

interface CapturedOutput {
  readonly channel: vscode.OutputChannel;
  readonly lines: string[];
}

function captureOutput(): CapturedOutput {
  const lines: string[] = [];
  const channel = {
    name: 'fixture',
    append: (value: string) => lines.push(value),
    appendLine: (value: string) => lines.push(value),
    replace: (value: string) => lines.push(value),
    clear: () => undefined,
    show: () => undefined,
    hide: () => undefined,
    dispose: () => undefined
  } as unknown as vscode.OutputChannel;
  return { channel, lines };
}

function mockContext(storageRoot: string): vscode.ExtensionContext {
  return { globalStorageUri: vscode.Uri.file(storageRoot) } as unknown as vscode.ExtensionContext;
}

function fixtureConfig(baseUrl: string): RiprConfig {
  return {
    enabled: true,
    serverPath: '',
    serverArgs: [],
    autoDownload: true,
    serverVersion: '',
    downloadBaseUrl: baseUrl,
    checkMode: 'draft',
    baseRef: 'origin/main',
    includeUnchangedTests: true,
    seamDiagnostics: true,
    diagnosticProfile: 'actionable',
    traceServer: 'off'
  };
}

function fixturePlatform(): RiprPlatform {
  return process.platform === 'win32'
    ? { target: 'fixture-target', executableName: 'ripr-fixture.exe', archiveExtension: 'zip', displayName: 'Fixture' }
    : { target: 'fixture-target', executableName: 'ripr-fixture', archiveExtension: 'tar.gz', displayName: 'Fixture' };
}

function probePayloadPath(): string {
  if (process.platform === 'win32') {
    // Composed from SystemDrive so the source carries no local absolute
    // Windows path (the local-context gate bans that literal).
    const systemRoot = process.env.SystemRoot ?? path.join(process.env.SystemDrive ?? 'C:', 'Windows');
    const payload = path.join(systemRoot, 'System32', 'tar.exe');
    assert.ok(fs.existsSync(payload), `fixture probe payload is missing: ${payload}`);
    return payload;
  }
  assert.ok(fs.existsSync('/bin/echo'), 'fixture probe payload /bin/echo is missing');
  return '/bin/echo';
}

function sha256File(filePath: string): string {
  return crypto.createHash('sha256').update(fs.readFileSync(filePath)).digest('hex');
}

function sha256Bytes(bytes: Buffer): string {
  return crypto.createHash('sha256').update(bytes).digest('hex');
}

function run(command: string, args: string[]): Promise<void> {
  return new Promise((resolve, reject) => {
    const child = require('child_process').spawn(command, args, { shell: false, stdio: 'ignore' });
    child.once('error', reject);
    child.once('exit', (code: number | null) => {
      if (code === 0) {
        resolve();
      } else {
        reject(new Error(`${command} exited with code ${code}`));
      }
    });
  });
}

async function buildFixtureArchive(workRoot: string): Promise<{ readonly bytes: Buffer; readonly sha256: string }> {
  const staging = path.join(workRoot, 'package');
  await fs.promises.mkdir(staging, { recursive: true });
  const isWindows = process.platform === 'win32';
  const executableName = isWindows ? 'ripr-fixture.exe' : 'ripr-fixture';
  await fs.promises.copyFile(probePayloadPath(), path.join(staging, executableName));
  const archivePath = path.join(workRoot, isWindows ? 'ripr-server.zip' : 'ripr-server.tar.gz');
  if (isWindows) {
    await run('powershell.exe', [
      '-NoProfile',
      '-NonInteractive',
      '-Command',
      `Compress-Archive -Path '${staging}\\*' -DestinationPath '${archivePath}' -Force`
    ]);
  } else {
    await run('tar', ['-czf', archivePath, '-C', staging, executableName]);
  }
  const bytes = await fs.promises.readFile(archivePath);
  return { bytes, sha256: sha256Bytes(bytes) };
}

function manifestJson(version: string, archiveUrl: string, archiveSha256: string): Buffer {
  return manifestBuffer(
    JSON.stringify(
      { version, assets: { 'fixture-target': { url: archiveUrl, sha256: archiveSha256 } } },
      null,
      2
    )
  );
}

function manifestBuffer(text: string): Buffer {
  return Buffer.from(`${text}\n`, 'utf8');
}

function placementRequest(
  server: ManifestFixtureServer,
  overrides: Partial<ManifestPlacementRequest> = {}
): ManifestPlacementRequest {
  return {
    requestedVersion: '2.0.0-rc.1',
    generation: '2.0.0',
    platformTarget: 'fixture-target',
    stableManifestUrl: `${server.base}/releases/download/v2.0.0/ripr-server-manifest-v2.0.0.json`,
    rcManifestUrl: `${server.base}/releases/download/v2.0.0-rc.1/ripr-server-manifest-v2.0.0.json`,
    ...overrides
  };
}

function staticFetcher(outcomes: Record<string, ManifestFetchOutcome>): ManifestBytesFetcher {
  return async (url) =>
    outcomes[url] ?? { kind: 'transport_failure', url, message: `fixture has no outcome for ${url}` };
}

suite('Downloader Manifest Placement', () => {
  let previousTlsReject: string | undefined;

  suiteSetup(() => {
    // The fixture server generates a one-day localhost certificate in this
    // process. The test host must accept it for the duration of this suite only.
    previousTlsReject = process.env.NODE_TLS_REJECT_UNAUTHORIZED;
    process.env.NODE_TLS_REJECT_UNAUTHORIZED = '0';
  });

  suiteTeardown(() => {
    if (previousTlsReject === undefined) {
      delete process.env.NODE_TLS_REJECT_UNAUTHORIZED;
    } else {
      process.env.NODE_TLS_REJECT_UNAUTHORIZED = previousTlsReject;
    }
  });

  test('managed download installs a complete server from a single placement', async function () {
    this.timeout(60_000);
    const server = new ManifestFixtureServer();
    await server.start();
    const archiveRoot = await fs.promises.mkdtemp(path.join(os.tmpdir(), 'ripr-fixture-archive-'));
    const storage = await fs.promises.mkdtemp(path.join(os.tmpdir(), 'ripr-single-placement-'));
    try {
      const archive = await buildFixtureArchive(archiveRoot);
      const archivePath = `/fixtures/ripr-server-archive${process.platform === 'win32' ? '.zip' : '.tar.gz'}`;
      server.route('/ripr-server-manifest-v2.0.0.json', {
        status: 200,
        body: manifestJson('2.0.0', `${server.base}${archivePath}`, archive.sha256)
      });
      server.route(archivePath, { status: 200, body: archive.bytes });

      const output = captureOutput();
      const installed = await downloadServer(
        mockContext(storage),
        fixtureConfig(server.base),
        fixturePlatform(),
        '2.0.0',
        output.channel
      );

      assert.strictEqual(installed.receipt.installationState, 'complete');
      assert.strictEqual(installed.receipt.requestedVersion, '2.0.0');
      assert.strictEqual(installed.receipt.manifestVersion, '2.0.0');
      assert.strictEqual(installed.receipt.manifestPlacement, 'stable');
      assert.strictEqual(installed.receipt.manifestUrl, `${server.base}/ripr-server-manifest-v2.0.0.json`);
      assert.ok(installed.receipt.binaryVersion.trim().length > 0);
      const paths = server.requests.map((request) => request.path);
      assert.ok(paths.includes('/ripr-server-manifest-v2.0.0.json'), `manifest must be fetched; got: ${paths.join(' ')}`);
      assert.ok(paths.includes(archivePath), `archive must be fetched; got: ${paths.join(' ')}`);
      assert.ok(
        output.lines.some((line) => line.includes('Selected stable server manifest for generation 2.0.0')),
        `placement observation must be reported; got: ${output.lines.join(' | ')}`
      );
    } finally {
      await fs.promises.rm(storage, { recursive: true, force: true });
      await fs.promises.rm(archiveRoot, { recursive: true, force: true });
      await server.stop();
    }
  });

  test('RC-channel download targets the generation-keyed placement first and refuses unadmitted RC fallback', async function () {
    this.timeout(20_000);
    const server = new ManifestFixtureServer();
    await server.start();
    const storage = await fs.promises.mkdtemp(path.join(os.tmpdir(), 'ripr-rc-fallback-'));
    try {
      const generationManifest = '/ripr-server-manifest-v2.0.0.json';
      const legacyVersionedManifest = '/ripr-server-manifest-v2.0.0-rc.1.json';
      server.route(generationManifest, { status: 404 });
      server.route(legacyVersionedManifest, { status: 404 });

      const attempt = downloadServer(
        mockContext(storage),
        fixtureConfig(server.base),
        fixturePlatform(),
        '2.0.0-rc.1',
        captureOutput().channel
      );
      await assert.rejects(attempt, /ripr-server-manifest-v2\.0\.0\.json/);

      const paths = server.requests.map((request) => request.path);
      assert.ok(
        paths.includes(generationManifest),
        `the generation-keyed manifest placement must be requested; requested: ${paths.join(' ')}`
      );
      assert.ok(
        !paths.includes(legacyVersionedManifest),
        `the legacy version-keyed manifest placement must not be requested; requested: ${paths.join(' ')}`
      );
    } finally {
      await fs.promises.rm(storage, { recursive: true, force: true });
      await server.stop();
    }
  });

  test('placement plan keys the manifest by generation and predeclares exactly one RC row on the default route', () => {
    const generation = '2.0.0';
    const requested = '2.0.0-rc.1';
    const repository = 'https://github.com/EffortlessMetrics/ripr/releases/download';

    const prerelease = manifestPlacementPlan('', requested);
    assert.strictEqual(prerelease.generation, generation);
    assert.strictEqual(prerelease.stableManifestUrl, `${repository}/v2.0.0/ripr-server-manifest-v2.0.0.json`);
    assert.strictEqual(prerelease.rcManifestUrl, `${repository}/v2.0.0-rc.1/ripr-server-manifest-v2.0.0.json`);

    const stable = manifestPlacementPlan('', generation);
    assert.strictEqual(stable.stableManifestUrl, `${repository}/v2.0.0/ripr-server-manifest-v2.0.0.json`);
    assert.strictEqual(stable.rcManifestUrl, undefined);

    const mirror = manifestPlacementPlan('https://mirror.example/base/', requested);
    assert.strictEqual(mirror.stableManifestUrl, 'https://mirror.example/base/ripr-server-manifest-v2.0.0.json');
    assert.strictEqual(mirror.rcManifestUrl, undefined);
  });

  test('placement archive URL keeps the archive on the placement that served the manifest', () => {
    assert.strictEqual(
      placementArchiveUrl(
        'https://github.com/EffortlessMetrics/ripr/releases/download/v2.0.0-rc.1/ripr-server-manifest-v2.0.0.json',
        'https://github.com/EffortlessMetrics/ripr/releases/download/v2.0.0/ripr-server-v2.0.0-fixture-target.zip'
      ),
      'https://github.com/EffortlessMetrics/ripr/releases/download/v2.0.0-rc.1/ripr-server-v2.0.0-fixture-target.zip'
    );
    assert.throws(() => placementArchiveUrl('https://host/m.json', 'https://host/'), /does not name an archive file/);
  });

  test('stable exact manifest is accepted and the RC placement is never fetched', async function () {
    this.timeout(20_000);
    const server = new ManifestFixtureServer();
    await server.start();
    try {
      const manifestBytes = manifestJson(
        '2.0.0',
        `${server.base}/releases/download/v2.0.0/ripr-server-v2.0.0-fixture-target.zip`,
        '0'.repeat(64)
      );
      server.route('/releases/download/v2.0.0/ripr-server-manifest-v2.0.0.json', { status: 200, body: manifestBytes });
      const descriptor: ServerDistributionDescriptor = {
        generation: '2.0.0',
        manifestSha256: sha256Bytes(manifestBytes)
      };

      const selection = await resolveServerManifestPlacement(placementRequest(server, {
        admittedManifestSha256: descriptor.manifestSha256
      }));

      assert.strictEqual(selection.observation.placement, 'stable');
      assert.strictEqual(selection.manifest.version, '2.0.0');
      const paths = server.requests.map((request) => request.path);
      assert.ok(
        !paths.includes('/releases/download/v2.0.0-rc.1/ripr-server-manifest-v2.0.0.json'),
        'the RC placement must never be fetched while stable is accepted'
      );
    } finally {
      await server.stop();
    }
  });

  test('authoritative stable absence selects the exact RC placement admitted by the same manifest digest', async function () {
    this.timeout(20_000);
    const server = new ManifestFixtureServer();
    await server.start();
    try {
      // The placement-neutral generation manifest is byte-identical on both
      // placements; the RC placement serves it under the same generation-keyed
      // name from the requesting release's own tag.
      const manifestBytes = manifestJson(
        '2.0.0',
        `${server.base}/releases/download/v2.0.0/ripr-server-v2.0.0-fixture-target.zip`,
        '0'.repeat(64)
      );
      server.route('/releases/download/v2.0.0/ripr-server-manifest-v2.0.0.json', { status: 404 });
      server.route('/releases/download/v2.0.0-rc.1/ripr-server-manifest-v2.0.0.json', {
        status: 200,
        body: manifestBytes
      });
      const descriptor: ServerDistributionDescriptor = {
        generation: '2.0.0',
        manifestSha256: sha256Bytes(manifestBytes)
      };

      const selection = await resolveServerManifestPlacement(placementRequest(server, {
        admittedManifestSha256: descriptor.manifestSha256
      }));

      assert.strictEqual(selection.observation.placement, 'rc_after_stable_absent');
      assert.strictEqual(
        selection.observation.stableManifestUrl,
        `${server.base}/releases/download/v2.0.0/ripr-server-manifest-v2.0.0.json`
      );
      assert.strictEqual(
        selection.observation.rcManifestUrl,
        `${server.base}/releases/download/v2.0.0-rc.1/ripr-server-manifest-v2.0.0.json`
      );
      assert.strictEqual(selection.manifestUrl, selection.observation.rcManifestUrl);
      assert.strictEqual(selection.manifest.version, '2.0.0');
      const paths = server.requests.map((request) => request.path);
      assert.deepStrictEqual(paths, [
        '/releases/download/v2.0.0/ripr-server-manifest-v2.0.0.json',
        '/releases/download/v2.0.0-rc.1/ripr-server-manifest-v2.0.0.json'
      ]);
    } finally {
      await server.stop();
    }
  });

  test('redirected 404 is not absence and never falls back', async function () {
    this.timeout(20_000);
    const server = new ManifestFixtureServer();
    await server.start();
    try {
      server.route('/releases/download/v2.0.0/redirect', {
        status: 302,
        location: `${server.base}/releases/download/v2.0.0/missing`
      });
      const request = placementRequest(server, {
        stableManifestUrl: `${server.base}/releases/download/v2.0.0/redirect`,
        admittedManifestSha256: 'a'.repeat(64)
      });

      await assert.rejects(resolveServerManifestPlacement(request), /failed with HTTP 404 after redirect/);

      const paths = server.requests.map((requestPath) => requestPath.path);
      assert.ok(
        !paths.includes('/releases/download/v2.0.0-rc.1/ripr-server-manifest-v2.0.0.json'),
        'a redirected 404 must not authorize the RC placement'
      );
    } finally {
      await server.stop();
    }
  });

  test('a redirect hop to a non-HTTPS URL resolves the typed transport failure instead of crashing', async function () {
    this.timeout(20_000);
    const server = new ManifestFixtureServer();
    await server.start();
    try {
      // The initial placement itself is already a non-HTTPS URL: the
      // https request call
      // would refuse the protocol, and an unguarded throw must not escape the
      // fetch promise.
      await assert.rejects(
        resolveServerManifestPlacement(placementRequest(server, {
          stableManifestUrl: 'http://127.0.0.1:9/ripr-server-manifest-v2.0.0.json',
          admittedManifestSha256: 'a'.repeat(64)
        })),
        /Refusing non-HTTPS manifest URL http:\/\/127\.0\.0\.1:9\//
      );

      // The redirect hop: the https placement answers 302 with an `http:`
      // Location. Before the guarded attempt this threw ERR_INVALID_PROTOCOL
      // synchronously inside the response callback — an uncaught exception on
      // a hop the promise executor does not own — and the fetch promise never
      // settled, hanging the install while the lock heartbeat kept running.
      server.route('/releases/download/v2.0.0/redirect', {
        status: 302,
        location: 'http://127.0.0.1:9/ripr-server-manifest-v2.0.0.json'
      });
      await assert.rejects(
        resolveServerManifestPlacement(placementRequest(server, {
          stableManifestUrl: `${server.base}/releases/download/v2.0.0/redirect`,
          admittedManifestSha256: 'a'.repeat(64)
        })),
        /Refusing non-HTTPS manifest URL http:\/\//
      );

      const paths = server.requests.map((requestPath) => requestPath.path);
      assert.ok(
        !paths.includes('/releases/download/v2.0.0-rc.1/ripr-server-manifest-v2.0.0.json'),
        'a non-HTTPS transport refusal must not authorize the RC placement'
      );
    } finally {
      await server.stop();
    }
  });

  test('server errors and forbidden responses are not absence and never fall back', async function () {
    this.timeout(20_000);
    const server = new ManifestFixtureServer();
    await server.start();
    try {
      server.route('/releases/download/v2.0.0/unavailable', { status: 503 });
      server.route('/releases/download/v2.0.0/forbidden', { status: 403 });
      server.route('/releases/download/v2.0.0-rc.1/ripr-server-manifest-v2.0.0.json', { status: 200, body: Buffer.from('{}') });

      await assert.rejects(
        resolveServerManifestPlacement(placementRequest(server, {
          stableManifestUrl: `${server.base}/releases/download/v2.0.0/unavailable`,
          admittedManifestSha256: 'a'.repeat(64)
        })),
        /failed with HTTP 503\./
      );
      await assert.rejects(
        resolveServerManifestPlacement(placementRequest(server, {
          stableManifestUrl: `${server.base}/releases/download/v2.0.0/forbidden`,
          admittedManifestSha256: 'a'.repeat(64)
        })),
        /failed with HTTP 403\./
      );

      const paths = server.requests.map((request) => request.path);
      assert.strictEqual(
        paths.filter((requestPath) => requestPath === '/releases/download/v2.0.0-rc.1/ripr-server-manifest-v2.0.0.json')
          .length,
        0,
        'transport and authorization failures must not authorize the RC placement'
      );
    } finally {
      await server.stop();
    }
  });

  test('a wrong-digest stable manifest is a contradiction, not absence, and never falls back', async function () {
    this.timeout(20_000);
    const server = new ManifestFixtureServer();
    await server.start();
    try {
      const manifestBytes = manifestJson(
        '2.0.0',
        `${server.base}/releases/download/v2.0.0/ripr-server-v2.0.0-fixture-target.zip`,
        '0'.repeat(64)
      );
      server.route('/releases/download/v2.0.0/ripr-server-manifest-v2.0.0.json', { status: 200, body: manifestBytes });

      await assert.rejects(
        resolveServerManifestPlacement(placementRequest(server, {
          admittedManifestSha256: sha256Bytes(Buffer.from('different manifest bytes'))
        })),
        /does not match the admitted manifest SHA-256/
      );

      const paths = server.requests.map((request) => request.path);
      assert.ok(
        !paths.includes('/releases/download/v2.0.0-rc.1/ripr-server-manifest-v2.0.0.json'),
        'a wrong-digest stable manifest must not authorize the RC placement'
      );
    } finally {
      await server.stop();
    }
  });

  test('an RC placement that contradicts the admission or typed identity is rejected terminally', async function () {
    this.timeout(20_000);
    const server = new ManifestFixtureServer();
    await server.start();
    try {
      server.route('/releases/download/v2.0.0/ripr-server-manifest-v2.0.0.json', { status: 404 });
      const wrongDigest = manifestJson(
        '2.0.0',
        `${server.base}/releases/download/v2.0.0-rc.1/ripr-server-v2.0.0-fixture-target.zip`,
        '0'.repeat(64)
      );
      const wrongGeneration = manifestJson(
        '1.9.9',
        `${server.base}/releases/download/v2.0.0-rc.1/ripr-server-v1.9.9-fixture-target.zip`,
        '0'.repeat(64)
      );
      const missingTarget = manifestBuffer(
        JSON.stringify({
          version: '2.0.0',
          assets: { 'other-target': { url: `${server.base}/a.zip`, sha256: '0'.repeat(64) } }
        })
      );

      // An absent RC placement stays terminal: no further lookup is derived.
      await assert.rejects(
        resolveServerManifestPlacement(placementRequest(server, {
          admittedManifestSha256: 'a'.repeat(64)
        })),
        /v2\.0\.0-rc\.1\/ripr-server-manifest-v2\.0\.0\.json failed with HTTP 404\./
      );
      server.route('/releases/download/v2.0.0-rc.1/ripr-server-manifest-v2.0.0.json', {
        status: 200,
        body: wrongDigest
      });
      await assert.rejects(
        resolveServerManifestPlacement(placementRequest(server, {
          admittedManifestSha256: sha256Bytes(Buffer.from('expected other bytes'))
        })),
        /does not match the admitted manifest SHA-256/
      );
      server.route('/releases/download/v2.0.0-rc.1/ripr-server-manifest-v2.0.0.json', {
        status: 200,
        body: wrongGeneration
      });
      await assert.rejects(
        resolveServerManifestPlacement(placementRequest(server, {
          admittedManifestSha256: sha256Bytes(wrongGeneration)
        })),
        /does not match distribution generation 2\.0\.0/
      );
      server.route('/releases/download/v2.0.0-rc.1/ripr-server-manifest-v2.0.0.json', {
        status: 200,
        body: missingTarget
      });
      await assert.rejects(
        resolveServerManifestPlacement(placementRequest(server, {
          admittedManifestSha256: sha256Bytes(missingTarget)
        })),
        /No ripr server asset is listed for fixture-target/
      );
    } finally {
      await server.stop();
    }
  });

  test('without an admitted descriptor the RC placement is never fetched and stable absence stays terminal', async function () {
    this.timeout(20_000);
    const server = new ManifestFixtureServer();
    await server.start();
    try {
      server.route('/releases/download/v2.0.0/ripr-server-manifest-v2.0.0.json', { status: 404 });
      server.route('/releases/download/v2.0.0-rc.1/ripr-server-manifest-v2.0.0.json', { status: 200, body: Buffer.from('{}') });

      await assert.rejects(
        resolveServerManifestPlacement(placementRequest(server)),
        /Stable server manifest is absent .* and no admitted RC fallback placement is available for 2\.0\.0-rc\.1\./
      );

      const paths = server.requests.map((request) => request.path);
      assert.ok(
        !paths.includes('/releases/download/v2.0.0-rc.1/ripr-server-manifest-v2.0.0.json'),
        'a generation without an admitted descriptor has no fallback row'
      );
    } finally {
      await server.stop();
    }
  });

  test('transport failures are not absence and never fall back', async () => {
    const stableUrl = 'https://fixture.invalid/releases/download/v2.0.0/ripr-server-manifest-v2.0.0.json';
    const rcUrl = 'https://fixture.invalid/releases/download/v2.0.0-rc.1/ripr-server-manifest-v2.0.0.json';
    const fetchedUrls: string[] = [];
    const fetcher: ManifestBytesFetcher = async (url) => {
      fetchedUrls.push(url);
      return { kind: 'transport_failure', url, message: `Timed out while fetching ${url}.` };
    };

    await assert.rejects(
      resolveServerManifestPlacement(
        {
          requestedVersion: '2.0.0-rc.1',
          generation: '2.0.0',
          platformTarget: 'fixture-target',
          stableManifestUrl: stableUrl,
          rcManifestUrl: rcUrl,
          admittedManifestSha256: 'a'.repeat(64)
        },
        fetcher
      ),
      /Timed out while fetching/
    );
    assert.deepStrictEqual(fetchedUrls, [stableUrl], 'transport unavailability must not authorize the RC placement');
  });

  test('malformed stable manifests fail closed without a fallback attempt', async () => {
    const stableUrl = 'https://fixture.invalid/manifest.json';
    const rcUrl = 'https://fixture.invalid/rc/manifest.json';
    const fetchedUrls: string[] = [];
    const fetcher: ManifestBytesFetcher = async (url) => {
      fetchedUrls.push(url);
      return { kind: 'ok', bytes: Buffer.from('not json') };
    };

    await assert.rejects(
      resolveServerManifestPlacement(
        {
          requestedVersion: '2.0.0-rc.1',
          generation: '2.0.0',
          platformTarget: 'fixture-target',
          stableManifestUrl: stableUrl,
          rcManifestUrl: rcUrl
        },
        fetcher
      ),
      /Server manifest is not an object\.|Unexpected token/
    );
    assert.deepStrictEqual(fetchedUrls, [stableUrl], 'a malformed stable manifest must not authorize the RC placement');
  });
});
