import * as assert from 'assert';
import { execFileSync } from 'child_process';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

function scriptPath(): string {
  return path.join(__dirname, '..', '..', 'scripts', 'admit-distribution-catalog.js');
}

function writeCatalog(body: unknown): string {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'ripr-catalog-admit-'));
  const catalog = path.join(root, 'catalog.json');
  fs.writeFileSync(catalog, JSON.stringify(body));
  return catalog;
}

function admit(args: string[]): { status: number; stdout: string; stderr: string } {
  try {
    const stdout = execFileSync(process.execPath, [scriptPath(), ...args], {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe']
    }) as string;
    return { status: 0, stdout, stderr: '' };
  } catch (error) {
    const failure = error as { status?: number; stdout?: string; stderr?: string };
    return {
      status: failure.status ?? 1,
      stdout: failure.stdout ?? '',
      stderr: failure.stderr ?? ''
    };
  }
}

suite('catalog admission script', () => {
  const digest = (char: string) => char.repeat(64);
  const stableCatalog = {
    schema: 2,
    productVersion: '0.11.0',
    channel: 'stable',
    releaseTag: 'v0.11.0',
    releaseRef: 'refs/tags/v0.11.0',
    fallbackPlacements: [
      {
        channel: 'rc',
        releaseTag: 'v0.11.0-rc.1',
        releaseRef: 'refs/tags/v0.11.0-rc.1'
      }
    ],
    manifestFile: 'ripr-server-manifest-v0.11.0.json',
    sourceRepository: 'https://github.com/EffortlessMetrics/ripr',
    distributionGeneration: digest('a'),
    manifestSha256: digest('b'),
    targetSetDigest: digest('c')
  };

  test('admits a producer catalog and reports bound release identity', () => {
    const catalog = writeCatalog(stableCatalog);
    try {
      const result = admit(['--catalog', catalog, '--package-version', '0.11.0']);
      assert.strictEqual(result.status, 0, result.stderr);
      const receipt = JSON.parse(result.stdout) as Record<string, unknown>;
      assert.strictEqual(receipt['admitted'], true);
      assert.strictEqual(receipt['distributionGeneration'], digest('a'));
      assert.strictEqual(receipt['manifestSha256'], digest('b'));
      assert.match(String(receipt['catalogSha256']), /^[0-9a-f]{64}$/);
      assert.match(String(receipt['catalogIdentity']), /^sha256:[0-9a-f]{64}$/);
    } finally {
      fs.rmSync(path.dirname(catalog), { recursive: true, force: true });
    }
  });

  test('rejects stale package versions and development catalogs for release packaging', () => {
    const catalog = writeCatalog(stableCatalog);
    try {
      const stale = admit(['--catalog', catalog, '--package-version', '0.10.1']);
      assert.strictEqual(stale.status, 2);
      assert.match(stale.stderr, /product version mismatch/);
      const development = writeCatalog({ ...stableCatalog, schema: 1, channel: 'development' });
      try {
        const dev = admit(['--catalog', development, '--package-version', '0.11.0']);
        assert.strictEqual(dev.status, 2);
      } finally {
        fs.rmSync(path.dirname(development), { recursive: true, force: true });
      }
    } finally {
      fs.rmSync(path.dirname(catalog), { recursive: true, force: true });
    }
  });

  test('admits a development catalog only with the development flag', () => {
    const development = writeCatalog({
      schema: 1,
      productVersion: '0.11.0',
      channel: 'development',
      releaseTag: 'v0.11.0',
      releaseRef: 'refs/tags/v0.11.0',
      manifestFile: 'ripr-server-manifest-v0.11.0.json',
      sourceRepository: 'https://github.com/EffortlessMetrics/ripr'
    });
    try {
      const admitted = admit(['--catalog', development, '--package-version', '0.11.0', '--allow-dev']);
      assert.strictEqual(admitted.status, 0, admitted.stderr);
      const receipt = JSON.parse(admitted.stdout) as Record<string, unknown>;
      assert.strictEqual(receipt['admitted'], true);
    } finally {
      fs.rmSync(path.dirname(development), { recursive: true, force: true });
    }
  });
});
