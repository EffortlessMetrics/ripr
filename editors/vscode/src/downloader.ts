import * as cp from 'child_process';
import * as https from 'https';
import * as path from 'path';
import * as vscode from 'vscode';
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
      const manifest = await fetchManifestForDistribution(
        config.downloadBaseUrl,
        distribution,
        version
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
      return { manifestVersion: manifest.version, expectedSha256: asset.sha256, bytes };
    },
    extractArchive: async (archivePath, destination) => {
      progress.report({ message: 'Extracting…' });
      await extractArchive(archivePath, destination, platform);
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
  const [preferred, ...fallbacks] = manifestCandidatesForDistribution(config.downloadBaseUrl, distribution, version);
  let admitted: { manifest: AdmittedServerManifest; manifestUrl: string };
  try {
    admitted = {
      manifest: await fetchAdmittedManifest(preferred, expectedDigest),
      manifestUrl: preferred
    };
  } catch (error) {
    const fallback = fallbacks[0];
    if (fallback === undefined || !isDirectManifestNotFound(error)) {
      throw error;
    }
    throw new Error(
      `Preferred placement manifest is unpublished and the fallback carries no admitted digest; refusing unadmitted fallback ${fallback}.`
    );
  }
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
  return { manifestVersion: admitted.manifest.productVersion, expectedSha256: asset.sha256, bytes, admittedManifestSha256: expectedDigest };
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
      response.on('end', () => {
        if (!capped) {
          resolve({ body: Buffer.concat(chunks), redirected });
        }
      });
    });
    request.on('error', reject);
    request.setTimeout(30_000, () => {
      request.destroy(new Error(`Timed out while fetching ${url}.`));
    });
  });
}

function extractArchive(archivePath: string, destination: string, platform: RiprPlatform): Promise<void> {
  if (platform.archiveExtension === 'zip') {
    return runProcess('powershell.exe', [
      '-NoProfile',
      '-ExecutionPolicy',
      'Bypass',
      '-Command',
      `Expand-Archive -LiteralPath ${quotePowerShell(archivePath)} -DestinationPath ${quotePowerShell(destination)} -Force`
    ]);
  }
  return runProcess('tar', ['-xzf', archivePath, '-C', destination]);
}

function runProcess(command: string, args: string[]): Promise<void> {
  return new Promise((resolve, reject) => {
    cp.execFile(command, args, (error, _stdout, stderr) => {
      if (error) {
        reject(new Error(stderr.trim() || error.message));
      } else {
        resolve();
      }
    });
  });
}

function quotePowerShell(value: string): string {
  return `'${value.replace(/'/g, "''")}'`;
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
