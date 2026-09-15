import * as fs from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';
import { RiprConfig } from './config';
import { cachedServerInstallation, downloadServer } from './downloader';
import {
  combineActiveManagedServerIdentity,
  ManagedServerInstallation,
  validateManagedServerVersion
} from './managedServerInstall';
import { currentRiprPlatform, RiprPlatform } from './platform';
import {
  LspCompatibilityEvidence,
  LspCompatibilityFailure,
  probeProcessExitCode,
  probeStandardLspCompatibility,
  spawnProbeProcess,
  terminateProbeProcessTree
} from './lspCompatibility';

const START_TIMEOUT_MS = 10_000;

export type ServerSource = 'configured' | 'bundled' | 'managed_cache' | 'managed_download' | 'path';

export interface ResolvedServer {
  readonly command: string;
  readonly source: ServerSource;
  readonly detail: string;
  readonly binaryVersion?: string;
  readonly protocolVersion?: string;
  readonly assetDigest?: string;
  readonly installationState: 'unmanaged' | 'bundled' | 'complete';
  readonly compatibilityResult: LspCompatibilityEvidence;
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

export interface ServerResolverRuntime {
  readonly probeCandidate: (
    command: string,
    source: ServerSource,
    detail: string,
    useShell?: boolean,
    installationState?: ResolvedServer['installationState']
  ) => Promise<ResolvedServer | ResolveFailure>;
}

const defaultResolverRuntime: ServerResolverRuntime = { probeCandidate };

export async function resolveServer(
  context: vscode.ExtensionContext,
  config: RiprConfig,
  output: vscode.OutputChannel,
  runtime: ServerResolverRuntime = defaultResolverRuntime
): Promise<ResolvedServer | ResolveFailure> {
  const configuredPath = config.serverPath.trim();
  if (configuredPath.length > 0) {
    return runtime.probeCandidate(configuredPath, 'configured', `configured ripr.server.path ${configuredPath}`, false, 'unmanaged');
  }

  const platform = currentRiprPlatform();
  let version: string;
  try {
    version = requestedServerVersion(context, config);
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    return { message: 'ripr.server.version is invalid.', detail };
  }
  let downloadFailure: string | undefined;

  if (platform) {
    const bundled = bundledServerPath(context, platform);
    const bundledResult = await probeExistingCandidate(
      bundled,
      'bundled',
      `bundled server for ${platform.target}`,
      runtime
    );
    if (isResolved(bundledResult)) {
      return bundledResult;
    }
    if (fs.existsSync(bundled)) {
      output.appendLine(`Skipping bundled server: ${bundledResult.detail}`);
    }

    const cached = await cachedServerInstallation(context, version, platform);
    if (cached) {
      const cachedResult = await runtime.probeCandidate(
        cached.executablePath,
        'managed_cache',
        `completed cached server ${version} for ${platform.target}`,
        false,
        'complete'
      );
      if (isResolved(cachedResult)) {
        return withManagedIdentity(cachedResult, cached);
      }
      output.appendLine(`Skipping completed cached server: ${cachedResult.detail}`);
    }

    if (config.autoDownload) {
      try {
        const downloaded = await downloadServer(context, config, platform, version, output);
        const downloadedResult = await runtime.probeCandidate(
          downloaded.executablePath,
          'managed_download',
          `atomically installed server ${version} for ${platform.target}`,
          false,
          'complete'
        );
        if (isResolved(downloadedResult)) {
          return withManagedIdentity(downloadedResult, downloaded);
        }
        downloadFailure = downloadedResult.detail;
        output.appendLine(`Skipping downloaded server: ${downloadFailure}`);
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
  const pathResult = await runtime.probeCandidate('ripr', 'path', 'ripr on PATH', probeWithShell, 'unmanaged');
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

export function requestedServerVersion(context: vscode.ExtensionContext, config: RiprConfig): string {
  const configured = config.serverVersion.trim();
  if (configured.length > 0) {
    return validateManagedServerVersion(configured.replace(/^v/, ''));
  }
  const version = context.extension?.packageJSON?.version;
  return validateManagedServerVersion(typeof version === 'string' ? version.replace(/^v/, '') : '0.8.0');
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
  detail: string,
  runtime: ServerResolverRuntime
): Promise<ResolvedServer | ResolveFailure> {
  if (!fs.existsSync(command)) {
    return { message: `${detail} was not found.`, detail: `${command} does not exist.` };
  }
  return runtime.probeCandidate(command, source, detail, false, source === 'bundled' ? 'bundled' : 'unmanaged');
}

function probeCandidate(
  command: string,
  source: ServerSource,
  detail: string,
  useShell = false,
  installationState: ResolvedServer['installationState'] = 'unmanaged'
): Promise<ResolvedServer | ResolveFailure> {
  return probeServerVersion(command, detail, useShell).then(async (versionResult) => {
    if ('message' in versionResult) {
      return versionResult;
    }
    const compatibility = await probeStandardLspCompatibility(command, useShell, START_TIMEOUT_MS);
    if (compatibility.status === 'incompatible') {
      return compatibilityFailure(detail, compatibility);
    }
    return {
      command,
      source,
      detail,
      binaryVersion: versionResult.binaryVersion,
      installationState,
      compatibilityResult: compatibility
    };
  });
}

export function probeServerVersion(
  command: string,
  detail: string,
  useShell: boolean,
  timeoutMs = START_TIMEOUT_MS
): Promise<{ readonly binaryVersion?: string } | ResolveFailure> {
  return new Promise((resolve) => {
    // On POSIX the child leads a fresh process group so timeout cleanup can
    // terminate descendants as one bounded unit. Windows uses taskkill /T.
    const child = spawnProbeProcess(command, ['--version'], useShell);
    const stdoutChunks: Buffer[] = [];
    const stderrChunks: Buffer[] = [];
    let settled = false;
    const finish = (result: { readonly binaryVersion?: string } | ResolveFailure): void => {
      if (settled) {
        return;
      }
      settled = true;
      clearTimeout(timer);
      terminateProbeProcessTree(child);
      resolve(result);
    };
    const timer = setTimeout(() => {
      finish({
        message: `${detail} did not respond.`,
        detail: `Timed out after ${timeoutMs}ms while running ${command} --version.`
      });
    }, timeoutMs);

    child.stdout?.on('data', (chunk: Buffer) => stdoutChunks.push(chunk));
    child.stderr?.on('data', (chunk: Buffer) => stderrChunks.push(chunk));

    child.once('error', (error) => {
      finish({ message: `${detail} could not start.`, detail: error.message });
    });

    child.once('exit', (wrapperCode) => {
      const code = probeProcessExitCode(child, wrapperCode);
      if (code === 0) {
        finish({
          binaryVersion: firstOutputLine(stdoutChunks, stderrChunks)
        });
      } else {
        finish({ message: `${detail} failed version check.`, detail: `${command} --version exited with code ${code}.` });
      }
    });
  });
}

function compatibilityFailure(detail: string, failure: LspCompatibilityFailure): ResolveFailure {
  return {
    message: `${detail} is not LSP compatible.`,
    detail: `[${failure.kind}] ${failure.detail}`
  };
}

function withManagedIdentity(
  resolved: ResolvedServer,
  installation: ManagedServerInstallation
): ResolvedServer {
  return combineActiveManagedServerIdentity(resolved, installation);
}

function isResolved(result: ResolvedServer | ResolveFailure): result is ResolvedServer {
  return 'command' in result;
}

function firstOutputLine(stdoutChunks: Buffer[], stderrChunks: Buffer[]): string | undefined {
  const output = Buffer.concat(stdoutChunks.length > 0 ? stdoutChunks : stderrChunks).toString('utf8');
  return output
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find(
      (line) =>
        line.length > 0 &&
        line !== '#< CLIXML' &&
        !line.startsWith('__RIPR_PROBE_EXIT_CODE__')
    );
}
