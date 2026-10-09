import * as cp from 'child_process';
import * as fs from 'fs';
import * as https from 'https';
import * as path from 'path';
import * as vscode from 'vscode';
import { extractAdmittedArchive } from './archiveInventory';
import { RiprConfig } from './config';
import {
  distributionManifestUrl,
  distributionPlacements,
  ResolvedDistributionRequest
} from './distributionDescriptor';
import {
  AdmittedServerManifest,
  admitInitialRequestTarget,
  admitManifestBytes,
  admitRedirectTarget,
  assetUrlForSubject,
  MAX_LEGACY_ARCHIVE_BYTES,
  MAX_MANIFEST_BYTES,
  RedirectPolicy,
  RELEASE_ASSET_HOSTS
} from './manifestTrust';
import {
  distributionGeneration,
  installManagedServer,
  ManagedServerInstallation,
  ManagedServerInstallRequest,
  readManagedServerInstallation,
  ResolvedArchive,
  validateManagedServerVersion
} from './managedServerInstall';
import { RiprPlatform } from './platform';

export interface ManifestAsset {
  readonly url: string;
  readonly sha256: string;
}

export interface ServerManifest {
  readonly version: string;
  readonly assets: Record<string, ManifestAsset>;
}

/**
 * How one manifest placement fetch ended (#3798 failure law): only a direct,
 * non-redirected HTTP 404 is authoritative absence; every other outcome is a
 * contradiction or transport failure and never authorizes a fallback.
 */
export type ManifestFetchOutcome =
  | { readonly kind: 'ok'; readonly bytes: Buffer }
  | { readonly kind: 'authoritative_absence'; readonly url: string }
  | {
      readonly kind: 'http_failure';
      readonly url: string;
      readonly statusCode: number;
      readonly redirected: boolean;
    }
  | { readonly kind: 'transport_failure'; readonly url: string; readonly message: string };

/** A manifest placement fetch that did not return a payload. */
export type ManifestFetchNonSuccess = Exclude<ManifestFetchOutcome, { readonly kind: 'ok' }>;

export type ManifestBytesFetcher = (url: string) => Promise<ManifestFetchOutcome>;

/**
 * Placement observation, kept separate from manifest and archive content
 * identity (#3798): which exact manifest URL was selected, and whether the RC
 * placement was selected only after authoritative stable absence.
 */
export type ManifestPlacementObservation =
  | { readonly placement: 'stable'; readonly manifestUrl: string }
  | {
      readonly placement: 'rc_after_stable_absent';
      readonly stableManifestUrl: string;
      readonly rcManifestUrl: string;
    };

export interface SelectedServerManifest {
  readonly manifest: ServerManifest;
  readonly manifestUrl: string;
  readonly observation: ManifestPlacementObservation;
}

export interface ManifestPlacementRequest {
  readonly requestedVersion: string;
  readonly generation: string;
  readonly platformTarget: string;
  readonly stableManifestUrl: string;
  readonly rcManifestUrl?: string;
  readonly admittedManifestSha256?: string;
}

export interface ManifestPlacementPlan {
  readonly generation: string;
  readonly stableManifestUrl: string;
  /** The one predeclared exact RC placement; never present for mirror routes. */
  readonly rcManifestUrl?: string;
}

export async function downloadServer(
  context: vscode.ExtensionContext,
  config: RiprConfig,
  platform: RiprPlatform,
  version: string,
  output: vscode.OutputChannel,
  distribution?: ResolvedDistributionRequest
): Promise<ManagedServerInstallation> {
  const managedVersion = validateManagedServerVersion(version);
  const origin = downloadOriginLabel(config, managedVersion);
  return vscode.window.withProgress(
    {
      location: vscode.ProgressLocation.Notification,
      title: `ripr: downloading server ${managedVersion} for ${platform.target} from ${origin}`,
      cancellable: false
    },
    (progress) =>
      downloadServerWithProgress(context, config, platform, managedVersion, output, progress, distribution)
  );
}

async function downloadServerWithProgress(
  context: vscode.ExtensionContext,
  config: RiprConfig,
  platform: RiprPlatform,
  version: string,
  output: vscode.OutputChannel,
  progress: vscode.Progress<{ message?: string; increment?: number }>,
  distribution?: ResolvedDistributionRequest
): Promise<ManagedServerInstallation> {
  const request = installRequest(context, version, platform, distribution);
  return installManagedServer(request, {
    resolveArchive: async () => {
      // Descriptor-bound downloads never fetch or parse unadmitted bytes:
      // branch before any manifest fetch so a replaced manifest cannot even
      // be retrieved, let alone select its own asset host.
      if (distribution?.manifestSha256 !== undefined) {
        return downloadAdmittedAsset(config, distribution, platform, version, output, progress);
      }

      progress.report({ message: 'Fetching release manifest…' });
      let selectedManifestUrl = '';
      const manifest = await fetchManifestForDistribution(
        config.downloadBaseUrl,
        distribution,
        version,
        async (url) => {
          const fetched = await fetchManifest(url);
          selectedManifestUrl = url;
          return fetched;
        }
      );
      if (manifest.version !== version) {
        throw new Error(`Server manifest version ${manifest.version} does not match requested version ${version}.`);
      }
      const asset = manifest.assets[platform.target];
      if (!asset) {
        throw new Error(`No ripr server asset is listed for ${platform.target} in manifest ${manifest.version}.`);
      }

      output.appendLine(`Downloading ripr server ${version} for ${platform.target}.`);
      progress.report({ message: `Downloading ${platform.executableName}…` });
      const { body: bytes } = await fetchBuffer(asset.url, MAX_LEGACY_ARCHIVE_BYTES, fetchPolicyFor(asset.url));
      progress.report({ message: 'Verifying checksum…' });
      output.appendLine(`Selected exact server manifest ${version} at ${selectedManifestUrl}.`);
      return {
        manifestVersion: manifest.version,
        expectedSha256: asset.sha256,
        bytes,
        selectedManifestUrl,
        manifestUrl: selectedManifestUrl
      };
    },
    extractArchive: async (archivePath, destination) => {
      progress.report({ message: 'Extracting…' });
      // Inventory authority (#1641): checksum-verified bytes are decoded by
      // the owned parser, policed, and budgeted before any file is created.
      // No system extractor ever touches archive bytes on this path.
      await extractAdmittedArchive(
        await fs.promises.readFile(archivePath),
        platform.archiveExtension,
        destination,
        platform.executableName
      );
    },
    probeExecutable: (executablePath) => probeDownloadedExecutable(executablePath)
  });
}

export function cachedServerInstallation(
  context: vscode.ExtensionContext,
  version: string,
  platform: RiprPlatform,
  distribution?: ResolvedDistributionRequest
): Promise<ManagedServerInstallation | undefined> {
  return readManagedServerInstallation(installRequest(context, version, platform, distribution));
}

function installRequest(
  context: vscode.ExtensionContext,
  version: string,
  platform: RiprPlatform,
  distribution?: ResolvedDistributionRequest
): ManagedServerInstallRequest {
  return {
    serversRoot: path.join(context.globalStorageUri.fsPath, 'servers'),
    version,
    platformTarget: platform.target,
    executableName: platform.executableName,
    archiveExtension: platform.archiveExtension,
    ...(distribution !== undefined ? { distributionIdentity: distribution.descriptorIdentity } : {}),
    ...(distribution?.manifestSha256 !== undefined ? { expectedManifestSha256: distribution.manifestSha256 } : {})
  };
}

/**
 * Descriptor-bound asset download. The manifest digest gates the raw bytes
 * before any field is read; the asset URL composes from the accepted
 * placement plus the admitted bare subject, so a replaced manifest cannot
 * select its own host. The returned admission stamp flows into the install
 * receipt, binding the cache entry to the exact descriptor/manifest tuple.
 */
async function downloadAdmittedAsset(
  config: RiprConfig,
  distribution: ResolvedDistributionRequest,
  platform: RiprPlatform,
  version: string,
  output: vscode.OutputChannel,
  progress: vscode.Progress<{ message?: string; increment?: number }>
): Promise<ResolvedArchive> {
  const expectedDigest = distribution.manifestSha256 as string;
  const admitted = await fetchAdmittedManifestForDistribution(
    config.downloadBaseUrl,
    distribution,
    version,
    expectedDigest
  );
  if (admitted.manifest.productVersion !== version) {
    throw new Error(
      `Admitted manifest product version ${admitted.manifest.productVersion} does not match requested version ${version}.`
    );
  }
  const asset = admitted.manifest.assets[platform.target];
  if (!asset) {
    throw new Error(`No ripr server asset is listed for ${platform.target} in the admitted manifest.`);
  }
  const placementBase = admitted.manifestUrl.slice(0, admitted.manifestUrl.lastIndexOf('/'));
  const assetUrl = assetUrlForSubject(placementBase, asset.subject);
  output.appendLine(`Downloading ripr server ${version} for ${platform.target} from admitted placement.`);
  progress.report({ message: `Downloading ${platform.executableName}…` });
  const { body: bytes } = await fetchBuffer(assetUrl, asset.archiveSize, fetchPolicyFor(assetUrl));
  progress.report({ message: 'Verifying checksum…' });
  return {
    manifestVersion: admitted.manifest.productVersion,
    expectedSha256: asset.sha256,
    bytes,
    admittedManifestSha256: expectedDigest,
    selectedManifestUrl: admitted.manifestUrl,
    manifestUrl: admitted.manifestUrl,
    ...(admitted.manifestSelection === 'fallback_exact_after_preferred_absent'
      ? { manifestPlacement: 'rc_after_stable_absent' as const }
      : distribution.preferredPlacement.channel === 'stable'
        ? { manifestPlacement: 'stable' as const }
        : {}),
    manifestSelection: admitted.manifestSelection,
    preferredManifestObservation: admitted.preferredManifestObservation,
    fallbackManifestObservation: admitted.fallbackManifestObservation
  };
}

export type FetchAdmittedManifest = (
  url: string,
  expectedDigest: string
) => Promise<AdmittedServerManifest>;

export interface AdmittedManifestSelection {
  readonly manifest: AdmittedServerManifest;
  readonly manifestUrl: string;
  readonly manifestSelection: 'preferred_exact' | 'fallback_exact_after_preferred_absent';
  readonly preferredManifestObservation: 'accepted' | 'direct_not_found';
  readonly fallbackManifestObservation: 'not_requested' | 'accepted';
}

/**
 * Fetches the descriptor-bound manifest from the preferred exact placement.
 * Only an authoritative direct 404 may select the one predeclared fallback.
 * Both placements are admitted against the same embedded manifest digest, so
 * fallback changes the observed location without changing content authority.
 */
export async function fetchAdmittedManifestForDistribution(
  baseUrl: string,
  distribution: ResolvedDistributionRequest,
  version: string,
  expectedDigest: string,
  fetchImpl: FetchAdmittedManifest = fetchAdmittedManifest
): Promise<AdmittedManifestSelection> {
  const [preferred, ...fallbacks] = manifestCandidatesForDistribution(baseUrl, distribution, version);
  try {
    return {
      manifest: await fetchImpl(preferred, expectedDigest),
      manifestUrl: preferred,
      manifestSelection: 'preferred_exact',
      preferredManifestObservation: 'accepted',
      fallbackManifestObservation: 'not_requested'
    };
  } catch (error) {
    const fallback = fallbacks[0];
    if (fallback === undefined || !isDirectManifestNotFound(error)) {
      throw error;
    }
    return {
      manifest: await fetchImpl(fallback, expectedDigest),
      manifestUrl: fallback,
      manifestSelection: 'fallback_exact_after_preferred_absent',
      preferredManifestObservation: 'direct_not_found',
      fallbackManifestObservation: 'accepted'
    };
  }
}

export async function fetchAdmittedManifest(url: string, expectedDigest: string): Promise<AdmittedServerManifest> {
  const { body } = await fetchBuffer(url, MAX_MANIFEST_BYTES, fetchPolicyFor(url));
  return admitManifestBytes(body, expectedDigest);
}

export function fetchPolicyFor(url: string): RedirectPolicy {
  let host = '';
  try {
    host = new URL(url).hostname;
  } catch {
    throw new Error(`Release URL ${JSON.stringify(url)} is not a valid URL.`);
  }
  return { initialHost: host, admittedHosts: RELEASE_ASSET_HOSTS };
}

/**
 * Ordered release-manifest URLs for one download: the preferred placement
 * first, then the bounded predeclared fallbacks. A configured mirror is an
 * explicit transport choice and serves the preferred placement only — it
 * never falls through to RC. Without a descriptor the legacy
 * version-pinned URL is the only candidate.
 */
export function manifestCandidatesForDistribution(
  baseUrl: string,
  distribution: ResolvedDistributionRequest | undefined,
  version: string
): string[] {
  if (!distribution) {
    return [manifestUrl(baseUrl, version)];
  }
  if (baseUrl.trim().length > 0) {
    return [distributionManifestUrl(baseUrl, distribution, distribution.preferredPlacement)];
  }
  return distributionPlacements(distribution).map((placement) =>
    distributionManifestUrl('', distribution, placement)
  );
}

/**
 * Failure of one manifest fetch that records whether the candidate is
 * simply unpublished. Only a direct (non-redirected) initial HTTP 404
 * means "this placement has no manifest"; every other failure —
 * transport errors, timeouts, HTTP 5xx, redirect-chain failures, malformed
 * bodies — propagates instead of selecting a different placement.
 */
export class ManifestFetchError extends Error {
  readonly statusCode?: number;
  readonly redirected: boolean;

  constructor(message: string, options: { statusCode?: number; redirected: boolean }) {
    super(message);
    this.name = 'ManifestFetchError';
    this.statusCode = options.statusCode;
    this.redirected = options.redirected;
  }
}

/** True when a manifest fetch failed because the candidate URL is unpublished. */
export function isDirectManifestNotFound(error: unknown): boolean {
  return error instanceof ManifestFetchError && error.statusCode === 404 && !error.redirected;
}

export type FetchManifest = (url: string) => Promise<ServerManifest>;

export async function fetchManifestForDistribution(
  baseUrl: string,
  distribution: ResolvedDistributionRequest | undefined,
  version: string,
  fetchImpl: FetchManifest = fetchManifest
): Promise<ServerManifest> {
  // The candidate list holds the preferred placement first and at most one
  // bounded predeclared fallback. Only the preferred placement may fall
  // through to that fallback, and only when its own initial response is a
  // direct 404. Every other failure propagates with its original error, and
  // a fetched manifest is validated by the caller, which fails fast: a
  // contradictory manifest must not be masked by a fallback.
  const [preferred, ...fallbacks] = manifestCandidatesForDistribution(baseUrl, distribution, version);
  try {
    return await fetchImpl(preferred);
  } catch (error) {
    const fallback = fallbacks[0];
    if (fallback === undefined || !isDirectManifestNotFound(error)) {
      throw error;
    }
    return fetchImpl(fallback);
  }
}

function downloadOriginLabel(config: RiprConfig, version: string): string {
  try {
    return new URL(manifestUrl(config.downloadBaseUrl, version)).host;
  } catch {
    return 'the configured download mirror';
  }
}

function manifestUrl(baseUrl: string, version: string): string {
  const file = `ripr-server-manifest-v${version}.json`;
  const base = baseUrl.trim();
  if (base.length > 0) {
    return `${base.replace(/\/+$/, '')}/${file}`;
  }
  return `https://github.com/EffortlessMetrics/ripr/releases/download/v${version}/${file}`;
}

async function fetchManifest(url: string): Promise<ServerManifest> {
  const { body } = await fetchBuffer(url, MAX_MANIFEST_BYTES, fetchPolicyFor(url));
  const parsed: unknown = JSON.parse(body.toString('utf8'));
  if (!parsed || typeof parsed !== 'object') {
    throw new Error('Server manifest is not an object.');
  }
  const manifest = parsed as Record<string, unknown>;
  if (manifest.schema_version === '2') {
    return serverManifestView(admitManifestBytes(body), url);
  }
  if (typeof manifest.version !== 'string' || !manifest.assets || typeof manifest.assets !== 'object') {
    throw new Error('Server manifest is missing a string version or asset map.');
  }
  for (const [target, value] of Object.entries(manifest.assets as Record<string, unknown>)) {
    if (!value || typeof value !== 'object') {
      throw new Error(`Server manifest asset ${target} is not an object.`);
    }
    const asset = value as Record<string, unknown>;
    if (typeof asset.url !== 'string' || typeof asset.sha256 !== 'string') {
      throw new Error(`Server manifest asset ${target} is missing its URL or SHA-256 digest.`);
    }
  }
  return parsed as ServerManifest;
}

function fetchBuffer(
  url: string,
  maxBytes: number,
  policy: RedirectPolicy,
  redirects = 0,
  redirected = false
): Promise<{ body: Buffer; redirected: boolean }> {
  let first: string;
  try {
    first = admitInitialRequestTarget(url, policy);
  } catch (error) {
    return Promise.reject(error instanceof Error ? error : new Error(String(error)));
  }
  return new Promise((resolve, reject) => {
    const request = https.get(first, (response) => {
      const statusCode = response.statusCode ?? 0;
      const location = response.headers.location;
      if (statusCode >= 300 && statusCode < 400 && location) {
        response.resume();
        if (redirects >= 5) {
          reject(new ManifestFetchError(`Too many redirects while fetching ${url}.`, { statusCode, redirected: true }));
          return;
        }
        let next: string;
        try {
          next = admitRedirectTarget(url, location, policy);
        } catch (error) {
          reject(error instanceof Error ? error : new Error(String(error)));
          return;
        }
        fetchBuffer(next, maxBytes, policy, redirects + 1, true).then(resolve, reject);
        return;
      }
      if (statusCode < 200 || statusCode >= 300) {
        response.resume();
        reject(new ManifestFetchError(`GET ${url} failed with HTTP ${statusCode}.`, { statusCode, redirected }));
        return;
      }

      const chunks: Buffer[] = [];
      let received = 0;
      let capped = false;
      response.on('data', (chunk: Buffer) => {
        received += chunk.length;
        if (received > maxBytes) {
          capped = true;
          request.destroy(new ManifestFetchError(`Response for ${url} exceeds the ${maxBytes}-byte bound.`, { statusCode, redirected }));
          return;
        }
        chunks.push(chunk);
      });
      let ended = false;
      response.on('error', reject);
      response.on('aborted', () => {
        reject(new ManifestFetchError(`Response for ${url} was aborted before completion.`, { statusCode, redirected }));
      });
      response.on('close', () => {
        if (!ended && !capped) {
          reject(new ManifestFetchError(`Response for ${url} closed before completion.`, { statusCode, redirected }));
        }
      });
      response.on('end', () => {
        ended = true;
        if (!capped && response.complete) {
          resolve({ body: Buffer.concat(chunks), redirected });
        } else if (!capped) {
          reject(new ManifestFetchError(`Response for ${url} ended before completion.`, { statusCode, redirected }));
        }
      });
    });
    request.on('error', reject);
    request.setTimeout(30_000, () => {
      request.destroy(new Error(`Timed out while fetching ${url}.`));
    });
  });
}

/** Compatibility placement APIs share the public schema and raw-byte admission. */
function serverManifestView(manifest: AdmittedServerManifest, manifestUrl: string): ServerManifest {
  const base = manifestUrl.slice(0, manifestUrl.lastIndexOf('/'));
  const assets: Record<string, ManifestAsset> = {};
  for (const [target, asset] of Object.entries(manifest.assets)) {
    assets[target] = { url: assetUrlForSubject(base, asset.subject), sha256: asset.sha256 };
  }
  return { version: manifest.productVersion, assets };
}

function placementManifest(bytes: Buffer, url: string, request: ManifestPlacementRequest): ServerManifest {
  const admitted = admitManifestBytes(bytes, request.admittedManifestSha256);
  if (admitted.productVersion !== request.generation) {
    throw new Error(`Server manifest version ${admitted.productVersion} does not match distribution generation ${request.generation}.`);
  }
  if (!admitted.assets[request.platformTarget]) {
    throw new Error(`No ripr server asset is listed for ${request.platformTarget} in manifest ${admitted.productVersion}.`);
  }
  return serverManifestView(admitted, url);
}

function placementFailure(outcome: ManifestFetchNonSuccess): Error {
  switch (outcome.kind) {
    case 'authoritative_absence':
      return new ManifestFetchError(`GET ${outcome.url} failed with HTTP 404.`, { statusCode: 404, redirected: false });
    case 'http_failure':
      return new ManifestFetchError(`GET ${outcome.url} failed with HTTP ${outcome.statusCode}${outcome.redirected ? ' after redirect' : ''}.`, {
        statusCode: outcome.statusCode, redirected: outcome.redirected
      });
    case 'transport_failure':
      return new Error(outcome.message);
  }
}

async function fetchManifestBytesOverHttps(url: string): Promise<ManifestFetchOutcome> {
  try {
    const { body } = await fetchBuffer(url, MAX_MANIFEST_BYTES, fetchPolicyFor(url));
    return { kind: 'ok', bytes: body };
  } catch (error) {
    if (isDirectManifestNotFound(error)) {
      return { kind: 'authoritative_absence', url };
    }
    if (error instanceof ManifestFetchError && error.statusCode !== undefined && error.statusCode >= 300) {
      return { kind: 'http_failure', url, statusCode: error.statusCode, redirected: error.redirected };
    }
    return { kind: 'transport_failure', url, message: error instanceof Error ? error.message : String(error) };
  }
}

/** A contradictory stable response never authorizes a second placement. */
export async function resolveServerManifestPlacement(
  request: ManifestPlacementRequest,
  fetchImpl: ManifestBytesFetcher = fetchManifestBytesOverHttps
): Promise<SelectedServerManifest> {
  if (request.generation !== distributionGeneration(request.requestedVersion)) {
    throw new Error(`Requested version ${request.requestedVersion} does not belong to generation ${request.generation}.`);
  }
  const stableUrl = admitInitialRequestTarget(request.stableManifestUrl, fetchPolicyFor(request.stableManifestUrl));
  const stable = await fetchImpl(stableUrl);
  if (stable.kind === 'ok') {
    return {
      manifest: placementManifest(stable.bytes, stableUrl, request),
      manifestUrl: stableUrl,
      observation: { placement: 'stable', manifestUrl: stableUrl }
    };
  }
  if (stable.kind !== 'authoritative_absence' || request.rcManifestUrl === undefined
      || request.admittedManifestSha256 === undefined) {
    throw placementFailure(stable);
  }
  // An immutable digest is required before the fallback is fetched. Parsing
  // either placement uses the same schema2 raw-byte admission, without a
  // permissive legacy parser or an independent transport policy.
  if (!/^[0-9a-f]{64}$/i.test(request.admittedManifestSha256)) {
    throw new Error('Admitted manifest digest is not a SHA-256 value.');
  }
  const rcUrl = admitInitialRequestTarget(request.rcManifestUrl, fetchPolicyFor(request.rcManifestUrl));
  const rc = await fetchImpl(rcUrl);
  if (rc.kind !== 'ok') {
    throw placementFailure(rc);
  }
  return {
    manifest: placementManifest(rc.bytes, rcUrl, request),
    manifestUrl: rcUrl,
    observation: { placement: 'rc_after_stable_absent', stableManifestUrl: stableUrl, rcManifestUrl: rcUrl }
  };
}

/** Pure URL plan; this does not create a digest or authorize runtime fallback. */
export function manifestPlacementPlan(baseUrl: string, requestedVersion: string): ManifestPlacementPlan {
  const requested = validateManagedServerVersion(requestedVersion);
  const generation = distributionGeneration(requested);
  const file = `ripr-server-manifest-v${generation}.json`;
  const base = baseUrl.trim();
  if (base.length > 0) {
    return { generation, stableManifestUrl: `${base.replace(/\/+$/, '')}/${file}` };
  }
  const releaseBase = 'https://github.com/EffortlessMetrics/ripr/releases/download';
  return {
    generation,
    stableManifestUrl: `${releaseBase}/v${generation}/${file}`,
    ...(requested !== generation ? { rcManifestUrl: `${releaseBase}/v${requested}/${file}` } : {})
  };
}

/** Legacy API shape; only a validated bare archive subject crosses placement. */
export function placementArchiveUrl(manifestUrl: string, assetUrl: string): string {
  const parsed = new URL(assetUrl);
  admitInitialRequestTarget(assetUrl, fetchPolicyFor(assetUrl));
  if (parsed.search !== '' || parsed.hash !== '') {
    throw new Error('Archive subject URL must not carry a query or fragment.');
  }
  const subject = decodeURIComponent(parsed.pathname.slice(parsed.pathname.lastIndexOf('/') + 1));
  if (subject.length === 0) {
    throw new Error(`Server manifest asset URL ${assetUrl} does not name an archive file.`);
  }
  const base = manifestUrl.slice(0, manifestUrl.lastIndexOf('/'));
  const resolved = assetUrlForSubject(base, subject);
  return admitInitialRequestTarget(resolved, fetchPolicyFor(manifestUrl));
}

function probeDownloadedExecutable(executablePath: string): Promise<string> {
  return new Promise((resolve, reject) => {
    cp.execFile(executablePath, ['--version'], { timeout: 5000 }, (error, stdout, stderr) => {
      if (error) {
        reject(new Error(`Downloaded server failed its version probe: ${stderr.trim() || error.message}`));
        return;
      }
      const version = firstNonemptyLine(stdout, stderr);
      if (!version) {
        reject(new Error('Downloaded server version probe produced no version text.'));
        return;
      }
      resolve(version);
    });
  });
}

function firstNonemptyLine(stdout: string, stderr: string): string | undefined {
  return (stdout || stderr)
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find((line) => line.length > 0);
}
