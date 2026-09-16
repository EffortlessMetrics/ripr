import * as assert from 'assert';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import { manifestCandidatesForDistribution } from '../../src/downloader';
import {
  requestedServerDistribution,
  requestedServerVersion
} from '../../src/serverResolver';
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

  test('rejects leading-zero product and numeric RC versions', () => {
    for (const productVersion of ['01.11.0', '0.01.0', '0.11.00']) {
      assert.throws(() => parseDistributionDescriptor(JSON.stringify({
        ...stable, productVersion,
        releaseTag: `v${productVersion}`, releaseRef: `refs/tags/v${productVersion}`,
        manifestFile: `ripr-server-manifest-v${productVersion}.json`
      })), /product version is not semantic/);
    }
    assert.throws(() => parseDistributionDescriptor(JSON.stringify({
      ...rc, releaseTag: 'v0.11.0-rc.01', releaseRef: 'refs/tags/v0.11.0-rc.01'
    })), /RC channel requires an RC release tag/);
    for (const productVersion of ['0.0.0', '1.0.10']) {
      const parsed = parseDistributionDescriptor(JSON.stringify({
        ...stable, productVersion,
        releaseTag: `v${productVersion}`, releaseRef: `refs/tags/v${productVersion}`,
        manifestFile: `ripr-server-manifest-v${productVersion}.json`
      }));
      assert.strictEqual(parsed.productVersion, productVersion);
    }
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

  function contextWithDescriptor(descriptor: DistributionDescriptor | undefined, packageVersion: string) {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'ripr-distribution-'));
    if (descriptor) {
      fs.writeFileSync(path.join(root, 'distribution.json'), JSON.stringify(descriptor));
    }
    const context = {
      extensionUri: { fsPath: root },
      extension: { packageJSON: { version: packageVersion } }
    } as never;
    return { root, context };
  }

  test('binds the managed version to the embedded descriptor', () => {
    const { root, context } = contextWithDescriptor(stable, '0.11.0');
    try {
      const config = { serverVersion: '' } as never;
      assert.strictEqual(requestedServerVersion(context, config), '0.11.0');
      const distribution = requestedServerDistribution(context);
      assert.ok(distribution, 'embedded descriptor must resolve');
      assert.strictEqual(distribution?.origin, 'embedded_descriptor');
      assert.match(distribution?.descriptorIdentity ?? '', /^sha256:[0-9a-f]{64}$/);
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  });

  test('fails closed when the descriptor disagrees with the package version', () => {
    const { root, context } = contextWithDescriptor(stable, '0.10.0');
    try {
      const config = { serverVersion: '' } as never;
      assert.throws(() => requestedServerVersion(context, config), /product version mismatch/);
      assert.throws(() => requestedServerDistribution(context), /product version mismatch/);
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  });

  test('falls back to the package version when the asset is absent', () => {
    const { root, context } = contextWithDescriptor(undefined, '0.10.1');
    try {
      const config = { serverVersion: '' } as never;
      assert.strictEqual(requestedServerVersion(context, config), '0.10.1');
      assert.strictEqual(requestedServerDistribution(context), undefined);
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  });

  test('an explicit server version bypasses the descriptor', () => {
    const { root, context } = contextWithDescriptor(
      { ...stable, productVersion: '9.9.9' },
      '0.10.1'
    );
    try {
      const config = { serverVersion: '0.8.0' } as never;
      assert.strictEqual(requestedServerVersion(context, config), '0.8.0');
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  });

  test('manifest candidates order stable before RC, mirror serves preferred only', () => {
    const request = resolveDistributionRequest('0.11.0', catalog);
    assert.deepStrictEqual(manifestCandidatesForDistribution('', request, '0.11.0'), [
      'https://github.com/EffortlessMetrics/ripr/releases/download/v0.11.0/ripr-server-manifest-v0.11.0.json',
      'https://github.com/EffortlessMetrics/ripr/releases/download/v0.11.0-rc.1/ripr-server-manifest-v0.11.0.json'
    ]);
    assert.deepStrictEqual(
      manifestCandidatesForDistribution('https://mirror.invalid/ripr/', request, '0.11.0'),
      ['https://mirror.invalid/ripr/ripr-server-manifest-v0.11.0.json']
    );
    assert.deepStrictEqual(manifestCandidatesForDistribution('', undefined, '0.10.1'), [
      'https://github.com/EffortlessMetrics/ripr/releases/download/v0.10.1/ripr-server-manifest-v0.10.1.json'
    ]);
  });

  test('rejects an embedded development catalog but keeps the fixture origin', () => {
    const development: DistributionDescriptor = {
      ...rc,
      channel: 'development',
      releaseTag: 'v0.11.0',
      releaseRef: 'refs/tags/v0.11.0'
    };
    assert.throws(
      () => resolveDistributionRequest('0.11.0', development),
      /embedded development distribution is not eligible/
    );
    const fixture = resolveDistributionRequest('0.11.0', development, 'development_fixture');
    assert.strictEqual(fixture.origin, 'development_fixture');
    assert.strictEqual(fixture.preferredPlacement.channel, 'development');
  });

  test('treats an embedded development descriptor as absent for managed resolution', () => {
    const development: DistributionDescriptor = {
      ...rc,
      channel: 'development',
      releaseTag: 'v0.11.0',
      releaseRef: 'refs/tags/v0.11.0'
    };
    const { root, context } = contextWithDescriptor(development, '0.11.0');
    try {
      const config = { serverVersion: '' } as never;
      assert.strictEqual(requestedServerDistribution(context), undefined);
      assert.strictEqual(requestedServerVersion(context, config), '0.11.0');
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  });
});

suite('distribution descriptor schema 2', () => {
  const digest = (char: string) => char.repeat(64);
  const releaseIdentity = {
    distributionGeneration: digest('a'),
    manifestSha256: digest('b'),
    targetSetDigest: digest('c')
  };
  const stable2: DistributionDescriptor = {
    schema: 2,
    productVersion: '0.11.0',
    channel: 'stable',
    releaseTag: 'v0.11.0',
    releaseRef: 'refs/tags/v0.11.0',
    manifestFile: 'ripr-server-manifest-v0.11.0.json',
    sourceRepository: 'https://github.com/EffortlessMetrics/ripr',
    ...releaseIdentity
  };
  const catalog2: DistributionDescriptor = {
    ...stable2,
    fallbackPlacements: [
      {
        channel: 'rc',
        releaseTag: 'v0.11.0-rc.1',
        releaseRef: 'refs/tags/v0.11.0-rc.1'
      }
    ]
  };

  test('resolves a schema-2 catalog and exposes release identity', () => {
    const request = resolveDistributionRequest('0.11.0', catalog2);
    assert.strictEqual(request.distributionGeneration, digest('a'));
    assert.strictEqual(request.manifestSha256, digest('b'));
    assert.strictEqual(request.targetSetDigest, digest('c'));
    assert.strictEqual(request.descriptorIdentity, resolveDistributionRequest('0.11.0', stable2).descriptorIdentity);
    assert.notStrictEqual(request.catalogIdentity, resolveDistributionRequest('0.11.0', stable2).catalogIdentity);
  });

  test('binds descriptor identity to the release generation', () => {
    const first = resolveDistributionRequest('0.11.0', stable2);
    const second = resolveDistributionRequest('0.11.0', {
      ...stable2,
      distributionGeneration: digest('d')
    });
    assert.notStrictEqual(first.descriptorIdentity, second.descriptorIdentity);
  });

  test('rejects schema-2 release catalogs with missing or malformed digests', () => {
    const { distributionGeneration: _dropped, ...missingGeneration } = stable2;
    assert.throws(() => parseDistributionDescriptor(JSON.stringify(missingGeneration)), /distributionGeneration must be a 64-character/);
    assert.throws(() => parseDistributionDescriptor(JSON.stringify({ ...stable2, manifestSha256: digest('B') })), /manifestSha256 must be a 64-character/);
    assert.throws(() => parseDistributionDescriptor(JSON.stringify({ ...stable2, targetSetDigest: 'abc' })), /targetSetDigest must be a 64-character/);
    assert.throws(() => parseDistributionDescriptor(JSON.stringify({ ...stable2, distributionGeneration: 42 as unknown as string })), /distributionGeneration must be a 64-character/);
  });

  test('rejects release identity on development catalogs and predates it on schema 1', () => {
    const development2: DistributionDescriptor = {
      ...stable2,
      channel: 'development'
    };
    assert.throws(() => parseDistributionDescriptor(JSON.stringify(development2)), /development catalog must not carry release identity/);
    const { distributionGeneration: _generation, manifestSha256: _manifest, targetSetDigest: _targets, ...bareDevelopment2 } = development2;
    assert.strictEqual(parseDistributionDescriptor(JSON.stringify(bareDevelopment2)).schema, 2);
    assert.throws(() => parseDistributionDescriptor(JSON.stringify({ ...stable2, schema: 1 })), /requires schema 2/);
    assert.throws(() => parseDistributionDescriptor(JSON.stringify({ ...stable2, schema: 3 })), /unsupported schema/);
  });
});
