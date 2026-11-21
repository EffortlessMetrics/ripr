import * as assert from 'assert';
import * as crypto from 'crypto';
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
  return execute(process.execPath, [scriptPath(), ...args]);
}

function execute(program: string, args: string[]): { status: number; stdout: string; stderr: string } {
  try {
    const stdout = execFileSync(program, args, {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
      timeout: 10_000,
      maxBuffer: 1_048_576
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
    targetSetDigest: digest('c'),
    producer: {
      tool: 'xtask release-distribution-catalog',
      schema: 'distribution-catalog/1'
    }
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

  test('admits actual Rust producer bytes for stable and RC catalogs', () => {
    const xtask = process.env.RIPR_TEST_XTASK_PATH;
    assert.ok(xtask && path.isAbsolute(xtask), 'run through cargo xtask vscode-test with its exact producer executable');
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'ripr-producer catalog-'));
    const manifestPath = path.join(root, 'ripr-server-manifest-v0.11.0.json');
    const catalogPath = path.join(root, 'catalog.json');
    const manifest = {
      schema_version: '2',
      product_version: '0.11.0',
      source_repository: 'EffortlessMetrics/ripr',
      distribution_generation: digest('a'),
      target_set: { digest: digest('c') }
    };
    const producerArgs = (channel: string) => [
      'release-distribution-catalog', '--product-version', '0.11.0',
      '--channel', channel, '--stable-tag', 'v0.11.0', '--rc-tag', 'v0.11.0-rc.1',
      '--manifest', manifestPath, '--repository', 'EffortlessMetrics/ripr', '--out', catalogPath
    ];
    try {
      fs.writeFileSync(manifestPath, `${JSON.stringify(manifest)}\n`);
      const manifestSha256 = crypto.createHash('sha256').update(fs.readFileSync(manifestPath)).digest('hex');
      for (const channel of ['stable', 'rc']) {
        fs.rmSync(catalogPath, { force: true });
        const produced = execute(xtask, producerArgs(channel));
        assert.strictEqual(produced.status, 0, produced.stderr);
        const catalogBytes = fs.readFileSync(catalogPath);
        const catalog = JSON.parse(catalogBytes.toString('utf8'));
        assert.deepStrictEqual(catalog.producer, stableCatalog.producer);
        assert.strictEqual(catalog.manifestSha256, manifestSha256);
        const admitted = admit(['--catalog', catalogPath, '--package-version', '0.11.0']);
        assert.strictEqual(admitted.status, 0, admitted.stderr);
        const receipt = JSON.parse(admitted.stdout);
        assert.strictEqual(receipt.admitted, true);
        assert.strictEqual(receipt.channel, channel);
        assert.strictEqual(receipt.releaseTag, channel === 'stable' ? 'v0.11.0' : 'v0.11.0-rc.1');
        assert.strictEqual(receipt.manifestSha256, manifestSha256);
        assert.strictEqual(receipt.distributionGeneration, manifest.distribution_generation);
        assert.strictEqual(receipt.targetSetDigest, manifest.target_set.digest);
        assert.strictEqual(receipt.catalogSha256, crypto.createHash('sha256').update(catalogBytes).digest('hex'));
      }
      for (const invalid of [
        { ...manifest, distribution_generation: digest('A') },
        { ...manifest, target_set: { digest: digest('C') } }
      ]) {
        fs.writeFileSync(manifestPath, `${JSON.stringify(invalid)}\n`);
        fs.rmSync(catalogPath, { force: true });
        const rejected = execute(xtask, producerArgs('stable'));
        assert.notStrictEqual(rejected.status, 0);
        assert.match(rejected.stderr, /must be a 64-character lowercase hex digest/);
        assert.strictEqual(fs.existsSync(catalogPath), false);
      }
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  });

  test('rejects release catalogs without the canonical producer identity', () => {
    const { producer: _producer, ...withoutProducer } = stableCatalog;
    const missing = writeCatalog(withoutProducer);
    const wrong = writeCatalog({
      ...stableCatalog,
      producer: { tool: 'other-tool', schema: 'distribution-catalog/1' }
    });
    try {
      const missingResult = admit(['--catalog', missing, '--package-version', '0.11.0']);
      assert.strictEqual(missingResult.status, 2);
      assert.match(missingResult.stderr, /requires producer identity/);
      const wrongResult = admit(['--catalog', wrong, '--package-version', '0.11.0']);
      assert.strictEqual(wrongResult.status, 2);
      assert.match(wrongResult.stderr, /unsupported distribution catalog producer identity/);
    } finally {
      fs.rmSync(path.dirname(missing), { recursive: true, force: true });
      fs.rmSync(path.dirname(wrong), { recursive: true, force: true });
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
    for (const version of ['0.11.0', '0.11.0-alpha.2']) {
      const development = writeCatalog({
        schema: 1,
        productVersion: version,
        channel: 'development',
        releaseTag: `v${version}`,
        releaseRef: `refs/tags/v${version}`,
        manifestFile: `ripr-server-manifest-v${version}.json`,
        sourceRepository: 'https://github.com/EffortlessMetrics/ripr'
      });
      try {
        const rejected = admit(['--catalog', development, '--package-version', version]);
        assert.strictEqual(rejected.status, 2);
        assert.match(rejected.stderr, /release packaging requires a schema 2 producer catalog/);
        const admitted = admit(['--catalog', development, '--package-version', version, '--allow-dev']);
        assert.strictEqual(admitted.status, 0, admitted.stderr);
        const receipt = JSON.parse(admitted.stdout) as Record<string, unknown>;
        assert.strictEqual(receipt['admitted'], true);
        assert.strictEqual(receipt['productVersion'], version);
      } finally {
        fs.rmSync(path.dirname(development), { recursive: true, force: true });
      }
    }
  });
});
