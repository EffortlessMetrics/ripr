import * as crypto from 'crypto';

/**
 * Release-manifest admission authority for #1640.
 *
 * A fetched manifest is bytes first: when the resolved distribution carries
 * an expected manifest digest, the raw bytes must hash to it before any
 * field is read. Parsed fields are then validated against the typed
 * server-manifest/2 contract. No manifest field may be used for network
 * fetch, path construction, cache selection, output claims, or archive
 * admission until this function returns.
 */

export const MAX_MANIFEST_BYTES = 1_048_576;
/**
 * Bound for legacy-flow archives, which carry no admitted size anchor.
 * Generous against real server archives (tens of MB) while refusing
 * unbounded buffering.
 */
export const MAX_LEGACY_ARCHIVE_BYTES = 256 * 1_048_576;
const MAX_SHORT_STRING = 1024;
const SHA256_PATTERN = /^[0-9a-f]{64}$/i;
const SEMVER_PATTERN = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/;

export interface AdmittedManifestExecutable {
  readonly path: string;
  readonly size: number;
  readonly sha256: string;
}

export interface AdmittedManifestAsset {
  readonly subject: string;
  readonly archiveFormat: string;
  readonly archiveSize: number;
  readonly sha256: string;
  readonly executable: AdmittedManifestExecutable;
}

export interface AdmittedServerManifest {
  readonly productVersion: string;
  readonly distributionGeneration: string;
  readonly sourceRepository: string;
  readonly targetDigest: string;
  readonly assets: Record<string, AdmittedManifestAsset>;
}

/** Admit fetched manifest bytes against an optional expected SHA-256 digest. */
export function admitManifestBytes(raw: Buffer, expectedDigest?: string): AdmittedServerManifest {
  if (raw.length === 0 || raw.length > MAX_MANIFEST_BYTES) {
    throw new Error(
      `Server manifest has an inadmissible byte length ${raw.length}; expected 1..${MAX_MANIFEST_BYTES}.`
    );
  }
  if (expectedDigest !== undefined) {
    if (!SHA256_PATTERN.test(expectedDigest)) {
      throw new Error('Expected manifest digest is not a SHA-256 value.');
    }
    const actual = crypto.createHash('sha256').update(raw).digest();
    const expected = Buffer.from(expectedDigest.toLowerCase(), 'hex');
    if (actual.length !== expected.length || !crypto.timingSafeEqual(actual, expected)) {
      throw new Error('Server manifest digest does not match the descriptor-admitted manifest digest.');
    }
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw.toString('utf8'));
  } catch (error) {
    throw new Error(`Server manifest is not valid JSON: ${error instanceof Error ? error.message : String(error)}`);
  }
  if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
    throw new Error('Server manifest is not a JSON object.');
  }
  return admitManifestValue(parsed as Record<string, unknown>);
}

function admitManifestValue(manifest: Record<string, unknown>): AdmittedServerManifest {
  if (manifest['schema_version'] !== '2') {
    throw new Error(`Unsupported server manifest schema version ${JSON.stringify(manifest['schema_version'])}.`);
  }
  const productVersion = boundedString(manifest['product_version'], 'product_version');
  if (!SEMVER_PATTERN.test(productVersion)) {
    throw new Error('Server manifest product_version is not semantic.');
  }
  const distributionGeneration = boundedString(manifest['distribution_generation'], 'distribution_generation');
  if (!SHA256_PATTERN.test(distributionGeneration)) {
    throw new Error('Server manifest distribution_generation is not a SHA-256 value.');
  }
  const sourceRepository = boundedString(manifest['source_repository'], 'source_repository');
  const targetSet = manifest['target_set'];
  if (!targetSet || typeof targetSet !== 'object' || Array.isArray(targetSet)) {
    throw new Error('Server manifest is missing its target_set object.');
  }
  const targets = (targetSet as Record<string, unknown>)['targets'];
  if (!Array.isArray(targets) || targets.length === 0 || !targets.every((t) => typeof t === 'string' && t.length > 0 && t.length <= MAX_SHORT_STRING)) {
    throw new Error('Server manifest target_set.targets must be a non-empty string list.');
  }
  const targetDigest = boundedString((targetSet as Record<string, unknown>)['digest'], 'target_set.digest');
  if (!SHA256_PATTERN.test(targetDigest)) {
    throw new Error('Server manifest target_set.digest is not a SHA-256 value.');
  }
  const assets = manifest['assets'];
  if (!assets || typeof assets !== 'object' || Array.isArray(assets) || Object.keys(assets).length === 0) {
    throw new Error('Server manifest is missing its asset map.');
  }
  const admitted: Record<string, AdmittedManifestAsset> = {};
  for (const [target, value] of Object.entries(assets as Record<string, unknown>)) {
    admitted[target] = admitAsset(target, value);
  }
  return { productVersion, distributionGeneration, sourceRepository, targetDigest, assets: admitted };
}

function admitAsset(target: string, value: unknown): AdmittedManifestAsset {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`Server manifest asset ${target} is not an object.`);
  }
  const asset = value as Record<string, unknown>;
  const subject = admitSubject(boundedString(asset['subject'], `assets.${target}.subject`));
  const archiveFormat = boundedString(asset['archive_format'], `assets.${target}.archive_format`);
  if (archiveFormat !== 'tar.gz' && archiveFormat !== 'zip') {
    throw new Error(`Server manifest asset ${target} has an unsupported archive_format.`);
  }
  const archiveSize = asset['archive_size'];
  if (typeof archiveSize !== 'number' || !Number.isInteger(archiveSize) || archiveSize <= 0) {
    throw new Error(`Server manifest asset ${target} has an invalid archive_size.`);
  }
  const sha256 = boundedString(asset['sha256'], `assets.${target}.sha256`);
  if (!SHA256_PATTERN.test(sha256)) {
    throw new Error(`Server manifest asset ${target} has an invalid sha256 digest.`);
  }
  const executable = asset['executable'];
  if (!executable || typeof executable !== 'object' || Array.isArray(executable)) {
    throw new Error(`Server manifest asset ${target} is missing its executable record.`);
  }
  const execRecord = executable as Record<string, unknown>;
  const execPath = admitSubject(boundedString(execRecord['path'], `assets.${target}.executable.path`));
  const execSize = execRecord['size'];
  if (typeof execSize !== 'number' || !Number.isInteger(execSize) || execSize <= 0) {
    throw new Error(`Server manifest asset ${target} has an invalid executable size.`);
  }
  const execSha256 = boundedString(execRecord['sha256'], `assets.${target}.executable.sha256`);
  if (!SHA256_PATTERN.test(execSha256)) {
    throw new Error(`Server manifest asset ${target} has an invalid executable sha256 digest.`);
  }
  return {
    subject,
    archiveFormat,
    archiveSize,
    sha256,
    executable: { path: execPath, size: execSize, sha256: execSha256 }
  };
}

/**
 * Placement-independent subjects only: a bare relative filename. Absolute
 * paths, URLs, separators, drive prefixes, and parent traversal never enter
 * manifest bytes, so retrieval URLs compose later from the accepted
 * placement plus this subject.
 */
function admitSubject(value: string): string {
  if (
    value.length === 0
    || value.length > MAX_SHORT_STRING
    || value.includes('/') || value.includes('\\')
    || value.includes('\0')
    || value === '.' || value === '..'
    || value.includes('..')
    || /^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(value)
  ) {
    throw new Error(`Server manifest subject ${JSON.stringify(value)} is not a bare relative filename.`);
  }
  return value;
}

function boundedString(value: unknown, field: string): string {
  if (typeof value !== 'string' || value.length === 0 || value.length > MAX_SHORT_STRING) {
    throw new Error(`Server manifest field ${field} is missing or unbounded.`);
  }
  return value;
}

export interface RedirectPolicy {
  /** Host of the initial request URL; same-origin redirects always stay. */
  readonly initialHost: string;
  /** Explicitly admitted additional hosts (release-asset hosts, mirror). */
  readonly admittedHosts: readonly string[];
}

/** Default admitted release-asset hosts for the GitHub release family. */
export const RELEASE_ASSET_HOSTS: readonly string[] = [
  'github.com',
  'api.github.com',
  'objects.githubusercontent.com',
  'release-assets.githubusercontent.com'
];

/**
 * Admit one redirect target. Returns the absolute URL to follow. Rejects
 * scheme downgrade, credentials, localhost/private targets, unsupported
 * ports, and hosts outside the initial origin plus the admitted set.
 */
export function admitRedirectTarget(currentUrl: string, location: string, policy: RedirectPolicy): string {
  let next: URL;
  try {
    next = new URL(location, currentUrl);
  } catch {
    throw new Error(`Release redirect target ${JSON.stringify(location)} is not a valid URL.`);
  }
  if (next.protocol !== 'https:') {
    throw new Error(`Release redirect target ${next.toString()} is not HTTPS.`);
  }
  if (next.username.length > 0 || next.password.length > 0) {
    throw new Error('Release redirect target carries credentials.');
  }
  const host = next.hostname.toLowerCase();
  if (host.length === 0 || host === 'localhost' || host === '::1' || isPrivateHost(host)) {
    throw new Error(`Release redirect target ${host || '(empty)'} is not a public host.`);
  }
  if (next.port.length > 0 && next.port !== '443') {
    throw new Error(`Release redirect target uses unsupported port ${next.port}.`);
  }
  const admitted =
    host === policy.initialHost.toLowerCase() ||
    policy.admittedHosts.some((candidate) => host === candidate.toLowerCase());
  if (!admitted) {
    throw new Error(`Release redirect target host ${host} is outside the admitted origin set.`);
  }
  next.hash = '';
  return next.toString();
}

/**
 * Admit an initial request destination before any byte is fetched. Redirect
 * checks cannot protect hop zero, so the first URL passes the same scheme,
 * credential, visibility, and port rules; the allowlist leg always holds
 * because the policy host derives from this same URL.
 */
export function admitInitialRequestTarget(url: string, policy: RedirectPolicy): string {
  return admitRedirectTarget(url, url, policy);
}

function isPrivateHost(host: string): boolean {
  if (/^127\./.test(host) || host === '0.0.0.0') {
    return true;
  }
  if (/^10\./.test(host) || /^192\.168\./.test(host)) {
    return true;
  }
  const private172 = /^172\.(1[6-9]|2\d|3[01])\./;
  if (private172.test(host)) {
    return true;
  }
  if (/^[0-9a-f:]+$/i.test(host) && (host.startsWith('fc') || host.startsWith('fd') || host === '::1')) {
    return true;
  }
  return false;
}

/**
 * Compose an asset retrieval URL from an accepted placement base URL and an
 * admitted bare subject. The subject never contributes scheme, host, path,
 * query, or fragment (admitManifestBytes guarantees the bare filename).
 */
export function assetUrlForSubject(placementBaseUrl: string, subject: string): string {
  const base = placementBaseUrl.replace(/\/+$/, '');
  if (base.length === 0) {
    throw new Error('Asset placement base URL is empty.');
  }
  let parsed: URL;
  try {
    parsed = new URL(base);
  } catch {
    throw new Error('Asset placement base URL is not a valid URL.');
  }
  if (parsed.protocol !== 'https:') {
    throw new Error('Asset placement base URL is not HTTPS.');
  }
  return `${base}/${subject}`;
}
