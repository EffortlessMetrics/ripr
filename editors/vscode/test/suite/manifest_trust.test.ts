import * as assert from 'assert';
import * as crypto from 'crypto';
import {
  admitInitialRequestTarget,
  admitManifestBytes,
  admitRedirectTarget,
  assetUrlForSubject,
  RELEASE_ASSET_HOSTS
} from '../../src/manifestTrust';

function sha256Hex(bytes: Buffer): string {
  return crypto.createHash('sha256').update(bytes).digest('hex');
}

function producerManifest(overrides: Record<string, unknown> = {}): Buffer {
  const manifest = {
    schema_version: '2',
    product_version: '0.11.0',
    distribution_generation: 'a'.repeat(64),
    source_repository: 'EffortlessMetrics/ripr',
    target_set: {
      targets: ['x86_64-unknown-linux-gnu'],
      digest: 'b'.repeat(64)
    },
    producer: { tool: 'xtask release-server-manifest', schema: 'server-manifest/2' },
    build_identity: {
      repository: 'EffortlessMetrics/ripr',
      candidate_sha: 'c'.repeat(40),
      candidate_tree: 'd'.repeat(40),
      toolchain: '1.95.0',
      toolchain_file_sha256: 'e'.repeat(64),
      cargo_lock_sha256: 'f'.repeat(64),
      profile: 'release',
      features: '',
      locked: true
    },
    assets: {
      'x86_64-unknown-linux-gnu': {
        subject: 'ripr-server-v0.11.0-x86_64-unknown-linux-gnu.tar.gz',
        archive_format: 'tar.gz',
        archive_size: 1234,
        sha256: '1'.repeat(64),
        executable: { path: 'ripr', size: 56, sha256: '2'.repeat(64) },
        receipt: {
          path: 'ripr-server-v0.11.0-x86_64-unknown-linux-gnu.receipt.json',
          sha256: '3'.repeat(64),
          schema_version: '0.2',
          target: 'x86_64-unknown-linux-gnu'
        }
      }
    },
    ...overrides
  };
  return Buffer.from(`${JSON.stringify(manifest, null, 2)}\n`, 'utf8');
}

suite('manifest trust admission', () => {
  test('admits exact producer bytes when the descriptor digest matches', () => {
    const raw = producerManifest();
    const admitted = admitManifestBytes(raw, sha256Hex(raw));
    assert.strictEqual(admitted.productVersion, '0.11.0');
    assert.strictEqual(
      admitted.assets['x86_64-unknown-linux-gnu'].sha256,
      '1'.repeat(64)
    );
  });

  test('rejects one-byte-tampered bytes against the same descriptor digest', () => {
    const raw = producerManifest();
    const digest = sha256Hex(raw);
    const tampered = Buffer.from(raw);
    tampered[tampered.length - 10] = tampered[tampered.length - 10] ^ 0xff;
    assert.throws(
      () => admitManifestBytes(tampered, digest),
      /digest/i
    );
  });

  test('rejects an unknown manifest schema version', () => {
    const raw = producerManifest({ schema_version: '99' });
    assert.throws(() => admitManifestBytes(raw, sha256Hex(raw)), /schema/i);
  });

  test('rejects an asset with an absolute URL subject', () => {
    const raw = producerManifest();
    const parsed = JSON.parse(raw.toString('utf8'));
    parsed.assets['x86_64-unknown-linux-gnu'].subject =
      'https://evil.example/ripr-server.tar.gz';
    const hostile = Buffer.from(JSON.stringify(parsed), 'utf8');
    assert.throws(
      () => admitManifestBytes(hostile, sha256Hex(hostile)),
      /subject/i
    );
  });

  test('rejects an asset with a traversal subject', () => {
    const raw = producerManifest();
    const parsed = JSON.parse(raw.toString('utf8'));
    parsed.assets['x86_64-unknown-linux-gnu'].subject = '../evil.tar.gz';
    const hostile = Buffer.from(JSON.stringify(parsed), 'utf8');
    assert.throws(
      () => admitManifestBytes(hostile, sha256Hex(hostile)),
      /subject/i
    );
  });

  test('rejects a manifest missing its asset digest', () => {
    const raw = producerManifest();
    const parsed = JSON.parse(raw.toString('utf8'));
    delete parsed.assets['x86_64-unknown-linux-gnu'].sha256;
    const hostile = Buffer.from(JSON.stringify(parsed), 'utf8');
    assert.throws(
      () => admitManifestBytes(hostile, sha256Hex(hostile)),
      /sha256|digest/i
    );
  });

  test('rejects non-object and malformed bodies before parsing trust', () => {
    for (const hostile of ['[1,2]', '"str"', 'null', '{bad']) {
      const raw = Buffer.from(hostile, 'utf8');
      assert.throws(
        () => admitManifestBytes(raw, sha256Hex(raw)),
        /manifest/i,
        hostile
      );
    }
  });
});

suite('release redirect and asset URL policy', () => {
  const policy = (initialHost: string, admittedHosts: readonly string[] = RELEASE_ASSET_HOSTS) => ({
    initialHost,
    admittedHosts
  });

  test('follows same-origin and release-asset redirects', () => {
    assert.strictEqual(
      admitRedirectTarget(
        'https://github.com/EffortlessMetrics/ripr/releases/download/v0.11.0/m.json',
        'https://objects.githubusercontent.com/x?y=1',
        policy('github.com')
      ),
      'https://objects.githubusercontent.com/x?y=1'
    );
    assert.strictEqual(
      admitRedirectTarget(
        'https://mirror.example/m.json',
        '/other/m.json',
        policy('mirror.example', [...RELEASE_ASSET_HOSTS, 'mirror.example'])
      ),
      'https://mirror.example/other/m.json'
    );
  });

  test('rejects downgrade, credentials, private, and foreign redirect targets', () => {
    const current = 'https://github.com/a/m.json';
    const hostile = [
      'http://github.com/a/m.json',
      'https://user:pass@github.com/a/m.json',
      'https://127.0.0.1/a/m.json',
      'https://10.1.2.3/a/m.json',
      'https://evil.example/a/m.json',
      'https://github.com:8443/a/m.json'
    ];
    for (const location of hostile) {
      assert.throws(() => admitRedirectTarget(current, location, policy('github.com')), /redirect/i, location);
    }
  });

  test('admits a clean initial request destination before any byte is fetched', () => {
    const policy = { initialHost: 'github.com', admittedHosts: RELEASE_ASSET_HOSTS };
    assert.strictEqual(
      admitInitialRequestTarget('https://github.com/a/m.json', policy),
      'https://github.com/a/m.json'
    );
    for (const hostile of [
      'http://github.com/a/m.json',
      'https://user@github.com/a/m.json',
      'https://127.0.0.1/a/m.json',
      'https://[::1]/a/m.json'
    ]) {
      assert.throws(() => admitInitialRequestTarget(hostile, policy), /redirect|URL|host|HTTPS/i, hostile);
    }
  });

  test('composes asset URLs from placement base plus bare subject only', () => {
    assert.strictEqual(
      assetUrlForSubject(
        'https://github.com/EffortlessMetrics/ripr/releases/download/v0.11.0',
        'ripr-server-v0.11.0-x86_64-unknown-linux-gnu.tar.gz'
      ),
      'https://github.com/EffortlessMetrics/ripr/releases/download/v0.11.0/ripr-server-v0.11.0-x86_64-unknown-linux-gnu.tar.gz'
    );
    assert.throws(() => assetUrlForSubject('http://mirror.example/x', 'a.tar.gz'), /https/i);
    assert.throws(() => assetUrlForSubject('', 'a.tar.gz'), /empty/i);
  });
});
