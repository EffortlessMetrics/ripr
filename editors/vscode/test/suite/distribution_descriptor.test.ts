import * as assert from 'assert';
import {
  DistributionDescriptor,
  distributionManifestUrl,
  parseDistributionDescriptor,
  resolveDistributionRequest
} from '../../src/distributionDescriptor';

describe('distribution descriptor', () => {
  const rc: DistributionDescriptor = {
    schema: 1,
    productVersion: '0.11.0',
    channel: 'rc',
    releaseTag: 'v0.11.0-rc.1',
    releaseRef: 'refs/tags/v0.11.0-rc.1',
    manifestFile: 'ripr-server-manifest-v0.11.0.json',
    sourceRepository: 'https://github.com/EffortlessMetrics/ripr'
  };

  it('keeps package version distinct from the RC release placement', () => {
    const request = resolveDistributionRequest('0.11.0', rc);
    assert.strictEqual(request.productVersion, '0.11.0');
    assert.strictEqual(request.releaseTag, 'v0.11.0-rc.1');
    assert.strictEqual(request.manifestFile, 'ripr-server-manifest-v0.11.0.json');
  });

  it('rejects a package version mismatch', () => {
    assert.throws(() => resolveDistributionRequest('0.10.1', rc), /product version mismatch/);
  });

  it('rejects a malformed or channel-inconsistent descriptor', () => {
    assert.throws(
      () => parseDistributionDescriptor(JSON.stringify({ ...rc, channel: 'stable' })),
      /stable channel requires a stable release tag/
    );
    assert.throws(() => parseDistributionDescriptor('{"schema":1}'), /missing release descriptor field/);
  });

  it('rejects a descriptor with a non-canonical release ref', () => {
    assert.throws(
      () => resolveDistributionRequest('0.11.0', { ...rc, releaseRef: 'refs/heads/main' }),
      /release ref must match release tag/
    );
  });

  it('rejects unknown fields and keeps mirror transport separate from identity', () => {
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
});
