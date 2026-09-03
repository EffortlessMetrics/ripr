import * as cp from 'child_process';
import * as crypto from 'crypto';
import * as fs from 'fs';
import * as https from 'https';
import * as path from 'path';
import * as vscode from 'vscode';
import { RiprConfig } from './config';
import { RiprPlatform } from './platform';
import { distributionManifestUrl, ResolvedDistributionRequest } from './distributionDescriptor';

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
  distribution: ResolvedDistributionRequest,
  output: vscode.OutputChannel
): Promise<string> {
  const origin = downloadOriginLabel(config, distribution);
  return vscode.window.withProgress(
    {
      location: vscode.ProgressLocation.Notification,
      title: `ripr: downloading server ${distribution.releaseTag} for ${platform.target} from ${origin}`,
      cancellable: false
    },
    (progress) => downloadServerWithProgress(context, config, platform, distribution, output, progress)
  );
}

async function downloadServerWithProgress(
  context: vscode.ExtensionContext,
  config: RiprConfig,
  platform: RiprPlatform,
  distribution: ResolvedDistributionRequest,
  output: vscode.OutputChannel,
  progress: vscode.Progress<{ message?: string; increment?: number }>
): Promise<string> {
  const cacheDir = serverCacheDir(context, distribution.releaseTag, platform.target);
  const executablePath = path.join(cacheDir, platform.executableName);
  await fs.promises.mkdir(cacheDir, { recursive: true });

  progress.report({ message: 'Fetching release manifest…' });
  const manifest = await fetchManifest(distributionManifestUrl(config.downloadBaseUrl, distribution));
  const asset = manifest.assets[platform.target];
  if (!asset) {
    throw new Error(`No ripr server asset is listed for ${platform.target} in manifest ${manifest.version}.`);
  }

  output.appendLine(`Downloading ripr server ${distribution.releaseTag} for ${platform.target}.`);
  progress.report({ message: `Downloading ${platform.executableName}…` });
  const archive = await fetchBuffer(asset.url);
  progress.report({ message: 'Verifying checksum…' });
  const actualSha = sha256Hex(archive);
  if (actualSha.toLowerCase() !== asset.sha256.toLowerCase()) {
    throw new Error(`Checksum mismatch for ${asset.url}. Expected ${asset.sha256}, got ${actualSha}.`);
  }
  progress.report({ message: 'Extracting…' });

  const archivePath = path.join(cacheDir, `ripr-server.${platform.archiveExtension}`);
  const extractDir = path.join(cacheDir, 'extract');
  await fs.promises.writeFile(archivePath, archive);
  await fs.promises.rm(extractDir, { recursive: true, force: true });
  await fs.promises.mkdir(extractDir, { recursive: true });
  await extractArchive(archivePath, extractDir, platform);
  const foundExecutable = await findExecutable(extractDir, platform.executableName);
  if (!foundExecutable) {
    throw new Error(`Downloaded archive did not contain ${platform.executableName}.`);
  }
  if (foundExecutable !== executablePath) {
    await fs.promises.copyFile(foundExecutable, executablePath);
  }
  if (process.platform !== 'win32') {
    await fs.promises.chmod(executablePath, 0o755);
  }
  await fs.promises.writeFile(path.join(cacheDir, 'sha256.txt'), `${actualSha}\n`);
  return executablePath;
}

export function cachedServerPath(context: vscode.ExtensionContext, releaseTag: string, platform: RiprPlatform): string {
  return path.join(serverCacheDir(context, releaseTag, platform.target), platform.executableName);
}

function serverCacheDir(context: vscode.ExtensionContext, version: string, target: string): string {
  return path.join(context.globalStorageUri.fsPath, 'servers', version, target);
}

function downloadOriginLabel(config: RiprConfig, distribution: ResolvedDistributionRequest): string {
  try {
    return new URL(distributionManifestUrl(config.downloadBaseUrl, distribution)).host;
  } catch {
    return 'the configured download mirror';
  }
}

async function fetchManifest(url: string): Promise<ServerManifest> {
  const body = await fetchBuffer(url);
  return JSON.parse(body.toString('utf8')) as ServerManifest;
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

function sha256Hex(data: Buffer): string {
  return crypto.createHash('sha256').update(data).digest('hex');
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

async function findExecutable(root: string, executableName: string): Promise<string | undefined> {
  const entries = await fs.promises.readdir(root, { withFileTypes: true });
  for (const entry of entries) {
    const fullPath = path.join(root, entry.name);
    if (entry.isFile() && entry.name === executableName) {
      return fullPath;
    }
    if (entry.isDirectory()) {
      const found = await findExecutable(fullPath, executableName);
      if (found) {
        return found;
      }
    }
  }
  return undefined;
}
