import * as assert from 'assert';
import {
  DistributionDescriptor,
  resolveDistributionRequest
} from '../../src/distributionDescriptor';
import {
  FetchAdmittedManifest,
  FetchManifest,
  fetchAdmittedManifestForDistribution,
  fetchManifestForDistribution,
  isDirectManifestNotFound,
  ManifestFetchError,
  ServerManifest
} from '../../src/downloader';

const stable: DistributionDescriptor = {
  schema: 1,
  productVersion: '0.11.0',
  channel: 'stable',
  releaseTag: 'v0.11.0',
  releaseRef: 'refs/tags/v0.11.0',
  manifestFile: 'ripr-server-manifest-v0.11.0.json',
  sourceRepository: 'https://github.com/EffortlessMetrics/ripr'
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

function manifest(version: string): ServerManifest {
  return { version, assets: {} };
}

function notFound(url: string, redirected: boolean): ManifestFetchError {
  return new ManifestFetchError(`GET ${url} failed with HTTP 404.`, { statusCode: 404, redirected });
}

suite('Distribution manifest fallback', () => {
  test('falls back to the RC placement on a direct 404 of the preferred manifest', async () => {
    const distribution = resolveDistributionRequest('0.11.0', catalog);
    const requested: string[] = [];
    const fetchImpl: FetchManifest = async (url) => {
      requested.push(url);
      if (requested.length === 1) {
        throw notFound(url, false);
      }
      return manifest('0.11.0');
    };
    const result = await fetchManifestForDistribution('', distribution, '0.11.0', fetchImpl);
    assert.strictEqual(result.version, '0.11.0');
    assert.deepStrictEqual(requested, [
      'https://github.com/EffortlessMetrics/ripr/releases/download/v0.11.0/ripr-server-manifest-v0.11.0.json',
      'https://github.com/EffortlessMetrics/ripr/releases/download/v0.11.0-rc.1/ripr-server-manifest-v0.11.0.json'
    ]);
  });

  test('admits the exact RC placement with the same embedded digest after direct stable 404', async () => {
    const distribution = resolveDistributionRequest('0.11.0', {
      ...catalog,
      schema: 2,
      distributionGeneration: 'a'.repeat(64),
      manifestSha256: 'b'.repeat(64),
      targetSetDigest: 'c'.repeat(64)
    });
    const expectedDigest = distribution.manifestSha256 as string;
    const requested: Array<[string, string]> = [];
    const fetchImpl: FetchAdmittedManifest = async (url, digest) => {
      requested.push([url, digest]);
      if (requested.length === 1) {
        throw notFound(url, false);
      }
      return { productVersion: '0.11.0', assets: {} } as never;
    };

    const result = await fetchAdmittedManifestForDistribution(
      '',
      distribution,
      '0.11.0',
      expectedDigest,
      fetchImpl
    );

    assert.strictEqual(
      result.manifestUrl,
      'https://github.com/EffortlessMetrics/ripr/releases/download/v0.11.0-rc.1/ripr-server-manifest-v0.11.0.json'
    );
    assert.deepStrictEqual(requested, [
      [
        'https://github.com/EffortlessMetrics/ripr/releases/download/v0.11.0/ripr-server-manifest-v0.11.0.json',
        expectedDigest
      ],
      [
        'https://github.com/EffortlessMetrics/ripr/releases/download/v0.11.0-rc.1/ripr-server-manifest-v0.11.0.json',
        expectedDigest
      ]
    ]);
  });

  test('trusted manifest fallback propagates every non-authoritative absence unchanged', async () => {
    const distribution = resolveDistributionRequest('0.11.0', {
      ...catalog,
      schema: 2,
      distributionGeneration: 'a'.repeat(64),
      manifestSha256: 'b'.repeat(64),
      targetSetDigest: 'c'.repeat(64)
    });
    const failures: Array<[string, unknown]> = [
      ['http 500', new ManifestFetchError('GET https://example.invalid/stable failed with HTTP 500.', { statusCode: 500, redirected: false })],
      ['redirected 404', notFound('https://example.invalid/stable', true)],
      ['transport', new Error('socket hang up')],
      ['digest conflict', new Error('Server manifest SHA-256 does not match the admitted digest.')]
    ];

    for (const [name, failure] of failures) {
      let calls = 0;
      const fetchImpl: FetchAdmittedManifest = async () => {
        calls += 1;
        throw failure;
      };
      await assert.rejects(
        fetchAdmittedManifestForDistribution(
          '',
          distribution,
          '0.11.0',
          distribution.manifestSha256 as string,
          fetchImpl
        ),
        (error: unknown) => error === failure,
        name
      );
      assert.strictEqual(calls, 1, `${name} must not reach the trusted fallback`);
    }
  });

  test('trusted manifest fallback is disabled for an explicit mirror transport', async () => {
    const distribution = resolveDistributionRequest('0.11.0', {
      ...catalog,
      schema: 2,
      distributionGeneration: 'a'.repeat(64),
      manifestSha256: 'b'.repeat(64),
      targetSetDigest: 'c'.repeat(64)
    });
    const failure = notFound('https://mirror.invalid/ripr/ripr-server-manifest-v0.11.0.json', false);
    let calls = 0;
    const fetchImpl: FetchAdmittedManifest = async () => {
      calls += 1;
      throw failure;
    };

    await assert.rejects(
      fetchAdmittedManifestForDistribution(
        'https://mirror.invalid/ripr',
        distribution,
        '0.11.0',
        distribution.manifestSha256 as string,
        fetchImpl
      ),
      (error: unknown) => error === failure
    );
    assert.strictEqual(calls, 1);
  });

  test('propagates transport, server, redirect, and malformed failures without falling back', async () => {
    const distribution = resolveDistributionRequest('0.11.0', catalog);
    const failures: Array<[string, unknown]> = [
      ['http 500', new ManifestFetchError('GET https://example.invalid/stable failed with HTTP 500.', { statusCode: 500, redirected: false })],
      ['redirected 404', notFound('https://example.invalid/stable', true)],
      ['transport', new Error('socket hang up')],
      ['malformed', new Error('Server manifest is not an object.')]
    ];
    for (const [name, failure] of failures) {
      let calls = 0;
      const fetchImpl: FetchManifest = async () => {
        calls += 1;
        throw failure;
      };
      await assert.rejects(
        fetchManifestForDistribution('', distribution, '0.11.0', fetchImpl),
        (error: unknown) => error === failure,
        name
      );
      assert.strictEqual(calls, 1, `${name} must not reach the fallback`);
    }
  });

  test('propagates a failing fallback candidate instead of wrapping it', async () => {
    const distribution = resolveDistributionRequest('0.11.0', catalog);
    const serverError = new ManifestFetchError('GET https://example.invalid/rc failed with HTTP 500.', {
      statusCode: 500,
      redirected: false
    });
    const fetchImpl: FetchManifest = async (url) => {
      if (url.includes('v0.11.0-rc.1')) {
        throw serverError;
      }
      throw notFound(url, false);
    };
    await assert.rejects(
      fetchManifestForDistribution('', distribution, '0.11.0', fetchImpl),
      (error: unknown) => error === serverError
    );
  });

  test('propagates a single candidate failure with its original error', async () => {
    const failure = notFound('https://example.invalid/legacy', false);
    let calls = 0;
    const fetchImpl: FetchManifest = async () => {
      calls += 1;
      throw failure;
    };
    await assert.rejects(
      fetchManifestForDistribution('', undefined, '0.10.1', fetchImpl),
      (error: unknown) => error === failure
    );
    assert.strictEqual(calls, 1);
  });

  test('classifies only direct 404 responses as unpublished placements', () => {
    assert.strictEqual(isDirectManifestNotFound(notFound('https://example.invalid/x', false)), true);
    assert.strictEqual(isDirectManifestNotFound(notFound('https://example.invalid/x', true)), false);
    assert.strictEqual(
      isDirectManifestNotFound(
        new ManifestFetchError('GET https://example.invalid/x failed with HTTP 500.', {
          statusCode: 500,
          redirected: false
        })
      ),
      false
    );
    assert.strictEqual(isDirectManifestNotFound(new Error('socket hang up')), false);
  });
});
