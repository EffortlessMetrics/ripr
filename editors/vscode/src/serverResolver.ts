import * as cp from 'child_process';
import * as fs from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';
import { RiprConfig } from './config';
import { cachedServerPath, downloadServer } from './downloader';
import { currentRiprPlatform, RiprPlatform } from './platform';

const START_TIMEOUT_MS = 5000;

export type ServerSource = 'configured' | 'bundled' | 'downloaded' | 'path';

export interface ResolvedServer {
  readonly command: string;
  readonly source: ServerSource;
  readonly detail: string;
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
  const version = requestedServerVersion(context, config);
  let downloadFailure: string | undefined;

  if (platform) {
    const bundled = bundledServerPath(context, platform);
    const bundledResult = await probeExistingCandidate(bundled, 'bundled', `bundled server for ${platform.target}`);
    if (isResolved(bundledResult)) {
      return bundledResult;
    }

    const cached = cachedServerPath(context, version, platform);
    const cachedResult = await probeExistingCandidate(cached, 'downloaded', `cached server ${version} for ${platform.target}`);
    if (isResolved(cachedResult)) {
      return cachedResult;
    }

    if (config.autoDownload) {
      try {
        const downloaded = await downloadServer(context, config, platform, version, output);
        const downloadedResult = await probeCandidate(
          downloaded,
          'downloaded',
          `downloaded server ${version} for ${platform.target}`
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

  const pathResult = await probeCandidate('ripr', 'path', 'ripr on PATH');
  if (isResolved(pathResult)) {
    if (downloadFailure) {
      output.appendLine(`Using PATH fallback after managed server resolution failed: ${downloadFailure}`);
    }
    return pathResult;
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
    return configured.replace(/^v/, '');
  }
  const version = context.extension.packageJSON?.version;
  return typeof version === 'string' ? version.replace(/^v/, '') : '0.3.0';
}

function bundledServerPath(context: vscode.ExtensionContext, platform: RiprPlatform): string {
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

function probeCandidate(command: string, source: ServerSource, detail: string): Promise<ResolvedServer | ResolveFailure> {
  return new Promise((resolve) => {
    const child = cp.spawn(command, ['--version'], { shell: false });
    const timer = setTimeout(() => {
      child.kill();
      resolve({
        message: `${detail} did not respond.`,
        detail: `Timed out after ${START_TIMEOUT_MS}ms while running ${command} --version.`
      });
    }, START_TIMEOUT_MS);

    child.once('error', (error) => {
      clearTimeout(timer);
      resolve({ message: `${detail} could not start.`, detail: error.message });
    });

    child.once('exit', (code) => {
      clearTimeout(timer);
      if (code === 0) {
        resolve({ command, source, detail });
      } else {
        resolve({ message: `${detail} failed version check.`, detail: `${command} --version exited with code ${code}.` });
      }
    });
  });
}

function isResolved(result: ResolvedServer | ResolveFailure): result is ResolvedServer {
  return 'command' in result;
}
