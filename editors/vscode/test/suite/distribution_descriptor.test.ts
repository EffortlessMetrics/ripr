import * as assert from 'assert';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import { cachedServerPath } from '../../src/downloader';
import { requestedServerDistribution, requestedServerVersion } from '../../src/serverResolver';
import {
  DistributionDescriptor,
  distributionManifestUrl,
  distributionPlacements,
  parseDistributionDescriptor,
  resolveDistributionRequest
} from '../../src/distributionDescriptor';

suite('distribution descriptor', () => {
  const rc: DistributionDescriptor = {
    schema: 1,
    productVersion: '0.11.0',
    channel: 'rc',
    releaseTag: 'v0.11.0-rc.1',
    releaseRef: 'refs/tags/v0.11.0-rc.1',
    manifestFile: 'ripr-server-manifest-v0.11.0.json',
    sourceRepository: 'https://github.com/EffortlessMetrics/ripr'
  };
  const stable: DistributionDescriptor = {
    ...rc,
    channel: 'stable',
    releaseTag: 'v0.11.0',
    releaseRef: 'refs/tags/v0.11.0'
  };
  const catalog: DistributionDescriptor = {
    ...stable,
    fallbackPlacements: [
      {
        channel: 'rc',
        releaseTag: 'v0.11.0-rc.1',
        releaseRef: 'refs/tags/v0.11.0-rc.1'
      }
    ]
  };

  test('keeps package version distinct from an RC release placement', () => {
    const request = resolveDistributionRequest('0.11.0', rc);
    assert.strictEqual(request.productVersion, '0.11.0');
    assert.strictEqual(request.preferredPlacement.releaseTag, 'v0.11.0-rc.1');
    assert.strictEqual(request.manifestFile, 'ripr-server-manifest-v0.11.0.json');
    assert.match(request.descriptorIdentity, /^sha256:[0-9a-f]{64}$/);
    assert.match(request.catalogIdentity, /^sha256:[0-9a-f]{64}$/);
  });

  test('uses one generation identity across exact stable and RC placements', () => {
    const rcRequest = resolveDistributionRequest('0.11.0', rc);
    const stableRequest = resolveDistributionRequest('0.11.0', stable);
    const catalogRequest = resolveDistributionRequest('0.11.0', catalog);
    assert.strictEqual(rcRequest.descriptorIdentity, stableRequest.descriptorIdentity);
    assert.strictEqual(stableRequest.descriptorIdentity, catalogRequest.descriptorIdentity);
    assert.notStrictEqual(stableRequest.catalogIdentity, catalogRequest.catalogIdentity);
  });

  test('orders stable before the one predeclared RC fallback', () => {
    const request = resolveDistributionRequest('0.11.0', catalog);
    const placements = distributionPlacements(request);
    assert.deepStrictEqual(
      placements.map((placement) => [placement.channel, placement.releaseTag]),
      [
        ['stable', 'v0.11.0'],
        ['rc', 'v0.11.0-rc.1']
      ]
    );
    assert.strictEqual(
      distributionManifestUrl('', request, placements[0]),
      'https://github.com/EffortlessMetrics/ripr/releases/download/v0.11.0/ripr-server-manifest-v0.11.0.json'
    );
    assert.strictEqual(
      distributionManifestUrl('', request, placements[1]),
      'https://github.com/EffortlessMetrics/ripr/releases/download/v0.11.0-rc.1/ripr-server-manifest-v0.11.0.json'
    );
  });

  test('does not alias different server generations or source repositories', () => {
    const first = resolveDistributionRequest('0.11.0', rc);
    const second = resolveDistributionRequest('0.11.0', {
      ...rc,
      sourceRepository: 'https://mirror.invalid/ripr'
    });
    assert.notStrictEqual(first.descriptorIdentity, second.descriptorIdentity);
  });

  test('rejects a package version mismatch', () => {
    assert.throws(() => resolveDistributionRequest('0.10.1', rc), /product version mismatch/);
  });

  test('rejects a malformed or channel-inconsistent descriptor', () => {
    assert.throws(
      () => parseDistributionDescriptor(JSON.stringify({ ...rc, channel: 'stable' })),
      /stable channel requires a stable release tag/
    );
    assert.throws(() => parseDistributionDescriptor('{"schema":1}'), /missing release descriptor field/);
    assert.throws(
      () => parseDistributionDescriptor(JSON.stringify({ ...rc, channel: 'development', releaseTag: '', releaseRef: 'refs/tags/' })),
      /release tag must not be empty/
    );
  });

  test('rejects invalid or widened fallback placement catalogs', () => {
    assert.throws(
      () => parseDistributionDescriptor(JSON.stringify({ ...rc, fallbackPlacements: catalog.fallbackPlacements })),
      /fallbacks require stable/
    );
    assert.throws(
      () => parseDistributionDescriptor(JSON.stringify({ ...stable, fallbackPlacements: [catalog.fallbackPlacements?.[0], catalog.fallbackPlacements?.[0]] })),
      /at most one bounded fallback/
    );
    assert.throws(
      () => parseDistributionDescriptor(JSON.stringify({ ...stable, fallbackPlacements: [{ channel: 'stable', releaseTag: 'v0.11.0', releaseRef: 'refs/tags/v0.11.0' }] })),
      /fallback placement must use the RC channel/
    );
  });

  test('rejects a descriptor with a non-canonical release ref', () => {
    assert.throws(
      () => resolveDistributionRequest('0.11.0', { ...rc, releaseRef: 'refs/heads/main' }),
      /release ref must match release tag/
    );
  });

  test('rejects unknown fields and keeps mirror transport separate from identity', () => {
    assert.throws(() => parseDistributionDescriptor(JSON.stringify({ ...rc, extra: true })), /unsupported release descriptor field/);
    const request = resolveDistributionRequest('0.11.0', catalog);
    assert.strictEqual(
      distributionManifestUrl('https://mirror.invalid/ripr/', request),
      'https://mirror.invalid/ripr/ripr-server-manifest-v0.11.0.json'
    );
  });

  test('uses one Windows-safe cache generation across stable and RC placement', () => {
    const rcRequest = resolveDistributionRequest('0.11.0', rc);
    const stableRequest = resolveDistributionRequest('0.11.0', stable);
    const context = { globalStorageUri: { fsPath: 'ripr-storage' } } as never;
    const platform = {
      target: 'x86_64-pc-windows-msvc',
      executableName: 'ripr.exe',
      archiveExtension: 'zip' as const,
      displayName: 'Windows x64'
    };
    const rcCachePath = cachedServerPath(context, rcRequest, platform);
    const stableCachePath = cachedServerPath(context, stableRequest, platform);
    assert.strictEqual(rcCachePath, stableCachePath);
    assert.ok(!rcCachePath.includes('sha256:'), rcCachePath);
    assert.ok(rcCachePath.includes(rcRequest.descriptorIdentity.slice('sha256:'.length)), rcCachePath);
  });

  test('keeps configured server compatibility version separate from extension version', () => {
    const context = { extension: { packageJSON: { version: '0.10.1' } } } as never;
    const config = { serverVersion: '0.8.0' } as never;
    assert.strictEqual(requestedServerVersion(context, config), '0.8.0');
    const request = requestedServerDistribution(context, config);
    assert.strictEqual(request.productVersion, '0.8.0');
    assert.strictEqual(request.origin, 'explicit_legacy_override');
  });

  test('fails closed when a real installed extension is missing its descriptor', () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'ripr-missing-distribution-'));
    try {
      const context = {
        extension: { packageJSON: { version: '0.11.0' } },
        extensionUri: { fsPath: root }
      } as never;
      const config = { serverVersion: '' } as never;
      assert.throws(
        () => requestedServerDistribution(context, config),
        /installed ripr extension is missing distribution descriptor/
      );
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  });

  test('keeps context-less development explicit and without public placement authority', () => {
    const context = { extension: { packageJSON: { version: '0.10.1' } } } as never;
    const config = { serverVersion: '' } as never;
    const request = requestedServerDistribution(context, config);
    assert.strictEqual(request.origin, 'development_fixture');
    assert.throws(() => distributionManifestUrl('', request), /development distribution has no public release placement/);
  });
});
