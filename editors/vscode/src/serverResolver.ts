import * as cp from 'child_process';
import * as fs from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';
import { RiprConfig } from './config';
import { cachedServerPath, downloadServer } from './downloader';
import {
  DistributionDescriptor,
  distributionDescriptorIdentity,
  parseDistributionDescriptor,
  resolveDistributionRequest,
  ResolvedDistributionRequest
} from './distributionDescriptor';
import { currentRiprPlatform, RiprPlatform } from './platform';

const START_TIMEOUT_MS = 5000;

export type ServerSource = 'configured' | 'bundled' | 'downloaded' | 'path';

export interface ResolvedServer {
  readonly command: string;
  readonly source: ServerSource;
  readonly detail: string;
  readonly version?: string;
  /**
   * True when this server must be spawned through the shell (#2079): a
   * Windows `.cmd`/`.bat` PATH shim resolves via the shell probe, and the
   * client spawn must use the same launch semantics or startup fails the
   * same way the probe used to.
   */
  readonly needsShell?: boolean;
}

export interface ResolveFailure {
  readonly message: string;
  readonly detail: string;
}

export async function resolveServer(
  context: vscode.ExtensionContext,
  config: RiprConfig,
  output: vscode.OutputChannel
): Promise<ResolvedServer | ResolveFailure> {
  const configuredPath = config.serverPath.trim();
  if (configuredPath.length > 0) {
    return probeCandidate(configuredPath, 'configured', `configured ripr.server.path ${configuredPath}`);
  }

  const platform = currentRiprPlatform();
  const distribution = requestedServerDistribution(context, config);
  let downloadFailure: string | undefined;

  if (platform) {
    const bundled = bundledServerPath(context, platform);
    const bundledResult = await probeExistingCandidate(bundled, 'bundled', `bundled server for ${platform.target}`);
    if (isResolved(bundledResult)) {
      return bundledResult;
    }

    const cached = cachedServerPath(context, distribution, platform);
    const cachedResult = await probeExistingCandidate(
      cached,
      'downloaded',
      `cached server ${distribution.releaseTag} for ${platform.target}`
    );
    if (isResolved(cachedResult)) {
      return cachedResult;
    }

    if (config.autoDownload) {
      try {
        const downloaded = await downloadServer(context, config, platform, distribution, output);
        const downloadedResult = await probeCandidate(
          downloaded,
          'downloaded',
          `downloaded server ${distribution.releaseTag} for ${platform.target}`
        );
        if (isResolved(downloadedResult)) {
          return downloadedResult;
        }
        downloadFailure = downloadedResult.detail;
      } catch (error) {
        downloadFailure = error instanceof Error ? error.message : String(error);
        output.appendLine(`ripr server download failed: ${downloadFailure}`);
      }
    }
  } else {
    downloadFailure = `No prebuilt ripr server target is known for ${process.platform}/${process.arch}.`;
  }

  // On Windows, spawning with shell: false does no PATHEXT resolution, so
  // ripr.bat/ripr.cmd shims (Scoop, Chocolatey, manual PATH) fail to start
  // even though `ripr` works in a terminal (#2079). The command is the
  // constant string 'ripr --version' — no user input reaches the shell.
  const probeWithShell = process.platform === 'win32';
  const pathResult = await probeCandidate('ripr', 'path', 'ripr on PATH', probeWithShell);
  const resolvedPathResult: ResolvedServer | ResolveFailure =
    isResolved(pathResult) && probeWithShell ? { ...pathResult, needsShell: true } : pathResult;
  if (isResolved(resolvedPathResult)) {
    if (downloadFailure) {
      output.appendLine(`Using PATH fallback after managed server resolution failed: ${downloadFailure}`);
    }
    return resolvedPathResult;
  }

  const autoDownloadHint = config.autoDownload
    ? 'Automatic download was enabled but did not produce a usable server.'
    : 'Automatic download is disabled.';
  return {
    message: 'ripr server is not available.',
    detail: [
      downloadFailure,
      pathResult.detail,
      `${autoDownloadHint} Set ripr.server.path, enable ripr.server.autoDownload, or install with cargo install ripr.`
    ]
      .filter((line): line is string => Boolean(line))
      .join('\n')
  };
}

/** Returns the product version used in setup and diagnostic output. */
export function requestedServerVersion(context: vscode.ExtensionContext, config: RiprConfig): string {
  const configured = config.serverVersion.trim();
  if (configured.length > 0) {
    return configured.replace(/^v/, '').replace(/-rc\.\d+$/, '');
  }
  return requestedServerDistribution(context, config).productVersion;
}

/** Resolves embedded distribution identity, or an explicit legacy override. */
export function requestedServerDistribution(
  context: vscode.ExtensionContext,
  config: RiprConfig
): ResolvedDistributionRequest {
  const packageVersion = extensionPackageVersion(context);
  const configured = config.serverVersion.trim();
  if (configured.length > 0) {
    const releaseTag = configured.startsWith('v') ? configured : `v${configured}`;
    // An explicit version is a legacy, user-selected transport override. It
    // must not be represented as an embedded trusted distribution identity.
    return legacyRequest(packageVersion, releaseTag);
  }
  const descriptor = embeddedDistributionDescriptor(context, packageVersion);
  return resolveDistributionRequest(packageVersion, descriptor);
}

/** Reads and normalizes the installed extension package version. */
function extensionPackageVersion(context: vscode.ExtensionContext): string {
  const version = context.extension?.packageJSON?.version;
  return typeof version === 'string' ? version.replace(/^v/, '') : '0.8.0';
}

/** Loads the installed descriptor or constructs the development fallback. */
function embeddedDistributionDescriptor(context: vscode.ExtensionContext, productVersion: string): DistributionDescriptor {
  const extensionUri = context.extensionUri;
  if (!extensionUri) {
    return legacyDescriptor(productVersion, `v${productVersion}`);
  }
  const descriptorPath = path.join(extensionUri.fsPath, 'distribution.json');
  if (fs.existsSync(descriptorPath)) {
    return parseDistributionDescriptor(fs.readFileSync(descriptorPath, 'utf8'));
  }
  return legacyDescriptor(productVersion, `v${productVersion}`);
}

/** Constructs the compatibility descriptor used when no embedded file exists. */
function legacyDescriptor(productVersion: string, releaseTag: string): DistributionDescriptor {
  const channel = /-rc\.\d+$/.test(releaseTag) ? 'rc' : 'stable';
  return {
    schema: 1,
    productVersion,
    channel,
    releaseTag,
    releaseRef: `refs/tags/${releaseTag}`,
    manifestFile: `ripr-server-manifest-v${productVersion}.json`,
    sourceRepository: 'https://github.com/EffortlessMetrics/ripr'
  };
}

/** Constructs an explicitly selected, legacy transport request. */
function legacyRequest(productVersion: string, releaseTag: string): ResolvedDistributionRequest {
  const releaseVersion = releaseTag.replace(/^v/, '').replace(/-rc\.\d+$/, '');
  const descriptor: DistributionDescriptor = {
    schema: 1,
    productVersion,
    releaseTag,
    releaseRef: `refs/tags/${releaseTag}`,
    manifestFile: `ripr-server-manifest-v${releaseVersion}.json`,
    sourceRepository: 'https://github.com/EffortlessMetrics/ripr',
    channel: /-rc\.\d+$/.test(releaseTag) ? 'rc' : 'stable'
  };
  return { ...descriptor, descriptorIdentity: distributionDescriptorIdentity(descriptor) };
}

function bundledServerPath(context: vscode.ExtensionContext, platform: RiprPlatform): string {
  // Dormant by design (#2085): no platform VSIX ships a bundled server
  // today, so this candidate never exists on disk and resolution falls
  // through to the cache/download path. Kept as the documented first
  // preference for when #1443 / #1624 ship platform VSIXs.
  return path.join(context.extensionUri.fsPath, 'server', platform.target, platform.executableName);
}

async function probeExistingCandidate(
  command: string,
  source: ServerSource,
  detail: string
): Promise<ResolvedServer | ResolveFailure> {
  if (!fs.existsSync(command)) {
    return { message: `${detail} was not found.`, detail: `${command} does not exist.` };
  }
  return probeCandidate(command, source, detail);
}

function probeCandidate(command: string, source: ServerSource, detail: string, useShell = false): Promise<ResolvedServer | ResolveFailure> {
  return new Promise((resolve) => {
    const child = cp.spawn(command, ['--version'], { shell: useShell });
    const stdoutChunks: Buffer[] = [];
    const stderrChunks: Buffer[] = [];
    const timer = setTimeout(() => {
      child.kill();
      resolve({
        message: `${detail} did not respond.`,
        detail: `Timed out after ${START_TIMEOUT_MS}ms while running ${command} --version.`
      });
    }, START_TIMEOUT_MS);

    child.stdout?.on('data', (chunk: Buffer) => stdoutChunks.push(chunk));
    child.stderr?.on('data', (chunk: Buffer) => stderrChunks.push(chunk));

    child.once('error', (error) => {
      clearTimeout(timer);
      resolve({ message: `${detail} could not start.`, detail: error.message });
    });

    child.once('exit', (code) => {
      clearTimeout(timer);
      if (code === 0) {
        resolve({
          command,
          source,
          detail,
          version: firstOutputLine(stdoutChunks, stderrChunks)
        });
      } else {
        resolve({ message: `${detail} failed version check.`, detail: `${command} --version exited with code ${code}.` });
      }
    });
  });
}

function isResolved(result: ResolvedServer | ResolveFailure): result is ResolvedServer {
  return 'command' in result;
}

function firstOutputLine(stdoutChunks: Buffer[], stderrChunks: Buffer[]): string | undefined {
  const output = Buffer.concat(stdoutChunks.length > 0 ? stdoutChunks : stderrChunks).toString('utf8');
  return output
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find((line) => line.length > 0);
}
