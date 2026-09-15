import * as cp from 'child_process';
import * as https from 'https';
import * as path from 'path';
import * as vscode from 'vscode';
import { RiprConfig } from './config';
import {
  installManagedServer,
  ManagedServerInstallation,
  ManagedServerInstallRequest,
  readManagedServerInstallation,
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
  output: vscode.OutputChannel
): Promise<ManagedServerInstallation> {
  const managedVersion = validateManagedServerVersion(version);
  const origin = downloadOriginLabel(config, managedVersion);
  return vscode.window.withProgress(
    {
      location: vscode.ProgressLocation.Notification,
      title: `ripr: downloading server ${managedVersion} for ${platform.target} from ${origin}`,
      cancellable: false
    },
    (progress) => downloadServerWithProgress(context, config, platform, managedVersion, output, progress)
  );
}

async function downloadServerWithProgress(
  context: vscode.ExtensionContext,
  config: RiprConfig,
  platform: RiprPlatform,
  version: string,
  output: vscode.OutputChannel,
  progress: vscode.Progress<{ message?: string; increment?: number }>
): Promise<ManagedServerInstallation> {
  const request = installRequest(context, version, platform);
  return installManagedServer(request, {
    resolveArchive: async () => {
      progress.report({ message: 'Fetching release manifest…' });
      const manifest = await fetchManifest(manifestUrl(config.downloadBaseUrl, version));
      if (manifest.version !== version) {
        throw new Error(`Server manifest version ${manifest.version} does not match requested version ${version}.`);
      }
      const asset = manifest.assets[platform.target];
      if (!asset) {
        throw new Error(`No ripr server asset is listed for ${platform.target} in manifest ${manifest.version}.`);
      }

      output.appendLine(`Downloading ripr server ${version} for ${platform.target}.`);
      progress.report({ message: `Downloading ${platform.executableName}…` });
      const bytes = await fetchBuffer(asset.url);
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
  platform: RiprPlatform
): Promise<ManagedServerInstallation | undefined> {
  return readManagedServerInstallation(installRequest(context, version, platform));
}

function installRequest(
  context: vscode.ExtensionContext,
  version: string,
  platform: RiprPlatform
): ManagedServerInstallRequest {
  return {
    serversRoot: path.join(context.globalStorageUri.fsPath, 'servers'),
    version,
    platformTarget: platform.target,
    executableName: platform.executableName,
    archiveExtension: platform.archiveExtension
  };
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
  const body = await fetchBuffer(url);
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

function fetchBuffer(url: string, redirects = 0): Promise<Buffer> {
  return new Promise((resolve, reject) => {
    const request = https.get(url, (response) => {
      const statusCode = response.statusCode ?? 0;
      const location = response.headers.location;
      if (statusCode >= 300 && statusCode < 400 && location) {
        response.resume();
        if (redirects >= 5) {
          reject(new Error(`Too many redirects while fetching ${url}.`));
          return;
        }
        const redirected = new URL(location, url).toString();
        fetchBuffer(redirected, redirects + 1).then(resolve, reject);
        return;
      }
      if (statusCode < 200 || statusCode >= 300) {
        response.resume();
        reject(new Error(`GET ${url} failed with HTTP ${statusCode}.`));
        return;
      }

      const chunks: Buffer[] = [];
      response.on('data', (chunk: Buffer) => chunks.push(chunk));
      response.on('end', () => resolve(Buffer.concat(chunks)));
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
