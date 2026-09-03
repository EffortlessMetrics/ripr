import * as assert from 'assert';
import { cachedServerPath } from '../../src/downloader';
import { requestedServerDistribution, requestedServerVersion } from '../../src/serverResolver';
import {
  DistributionDescriptor,
  distributionManifestUrl,
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

  test('keeps package version distinct from the RC release placement', () => {
    const request = resolveDistributionRequest('0.11.0', rc);
    assert.strictEqual(request.productVersion, '0.11.0');
    assert.strictEqual(request.releaseTag, 'v0.11.0-rc.1');
    assert.strictEqual(request.manifestFile, 'ripr-server-manifest-v0.11.0.json');
    assert.match(request.descriptorIdentity, /^sha256:[0-9a-f]{64}$/);
  });

  test('does not alias different source repositories', () => {
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

  test('rejects a descriptor with a non-canonical release ref', () => {
    assert.throws(
      () => resolveDistributionRequest('0.11.0', { ...rc, releaseRef: 'refs/heads/main' }),
      /release ref must match release tag/
    );
  });

  test('rejects unknown fields and keeps mirror transport separate from identity', () => {
    assert.throws(() => parseDistributionDescriptor(JSON.stringify({ ...rc, extra: true })), /unsupported release descriptor field/);
    const request = resolveDistributionRequest('0.11.0', rc);
    assert.strictEqual(
      distributionManifestUrl('https://mirror.invalid/ripr/', request),
      'https://mirror.invalid/ripr/ripr-server-manifest-v0.11.0.json'
    );
    assert.strictEqual(
      distributionManifestUrl('', request),
      'https://github.com/EffortlessMetrics/ripr/releases/download/v0.11.0-rc.1/ripr-server-manifest-v0.11.0.json'
    );
  });

  test('uses a Windows-safe cache identity segment', () => {
    const request = resolveDistributionRequest('0.11.0', rc);
    const context = { globalStorageUri: { fsPath: 'C:\\ripr-storage' } } as never;
    const platform = {
      target: 'x86_64-pc-windows-msvc',
      executableName: 'ripr.exe',
      archiveExtension: 'zip' as const,
      displayName: 'Windows x64'
    };
    const cachePath = cachedServerPath(context, request, platform);
    assert.ok(!cachePath.includes('sha256:'), cachePath);
    assert.ok(cachePath.includes(request.descriptorIdentity.slice('sha256:'.length)), cachePath);
  });

  test('keeps configured server compatibility version separate from extension version', () => {
    const context = { extension: { packageJSON: { version: '0.10.1' } } } as never;
    const config = { serverVersion: '0.8.0' } as never;
    assert.strictEqual(requestedServerVersion(context, config), '0.8.0');
  });

  test('supports a context-less development fallback', () => {
    const context = { extension: { packageJSON: { version: '0.10.1' } } } as never;
    const config = { serverVersion: '' } as never;
    assert.doesNotThrow(() => requestedServerDistribution(context, config));
  });
});
