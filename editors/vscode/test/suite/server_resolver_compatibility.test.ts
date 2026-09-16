import * as assert from 'assert';
import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';
import * as vscode from 'vscode';
import { RiprConfig } from '../../src/config';
import { currentRiprPlatform } from '../../src/platform';
import { resolveServer, ServerResolverRuntime } from '../../src/serverResolver';
import { compatibleLspEvidence } from './testCompatibility';

suite('Server resolver compatibility fallback', () => {
  test('skips an incompatible bundled candidate and selects the next allowed channel', async function () {
    const platform = currentRiprPlatform();
    if (!platform) {
      this.skip();
      return;
    }
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'ripr-resolver-probe-'));
    const bundled = path.join(root, 'extension', 'server', platform.target, platform.executableName);
    fs.mkdirSync(path.dirname(bundled), { recursive: true });
    fs.writeFileSync(bundled, 'sentinel');
    const attempts: string[] = [];
    const runtime: ServerResolverRuntime = {
      probeCandidate: async (command, source, detail, _useShell, installationState = 'unmanaged') => {
        attempts.push(source);
        if (source === 'bundled') {
          return {
            message: `${detail} is not LSP compatible.`,
            detail: '[missing_required_capability] hoverProvider'
          };
        }
        return {
          command,
          source,
          detail,
          installationState,
          compatibilityResult: compatibleLspEvidence
        };
      }
    };
    const outputLines: string[] = [];
    const context = {
      extensionUri: vscode.Uri.file(path.join(root, 'extension')),
      globalStorageUri: vscode.Uri.file(path.join(root, 'storage')),
      extension: { packageJSON: { version: '0.10.0' } }
    } as unknown as vscode.ExtensionContext;
    try {
      const result = await resolveServer(context, config(), {
        appendLine: (line: string) => outputLines.push(line)
      } as unknown as vscode.OutputChannel, runtime);
      assert.ok('command' in result, JSON.stringify(result));
      assert.deepStrictEqual(attempts, ['bundled', 'path']);
      assert.ok(outputLines.some((line) => line.includes('Skipping bundled server')));
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  });

  test('fails closed when the embedded descriptor disagrees with the package version', async () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'ripr-resolver-descriptor-'));
    const descriptor = {
      schema: 1,
      productVersion: '0.11.0',
      channel: 'stable',
      releaseTag: 'v0.11.0',
      releaseRef: 'refs/tags/v0.11.0',
      manifestFile: 'ripr-server-manifest-v0.11.0.json',
      sourceRepository: 'https://github.com/EffortlessMetrics/ripr'
    };
    fs.writeFileSync(path.join(root, 'distribution.json'), JSON.stringify(descriptor));
    const attempts: string[] = [];
    const runtime: ServerResolverRuntime = {
      probeCandidate: async (command, source, detail, _useShell, installationState = 'unmanaged') => {
        attempts.push(source);
        return {
          command,
          source,
          detail,
          installationState,
          compatibilityResult: compatibleLspEvidence
        };
      }
    };
    const context = {
      extensionUri: vscode.Uri.file(root),
      globalStorageUri: vscode.Uri.file(path.join(root, 'storage')),
      extension: { packageJSON: { version: '0.10.0' } }
    } as unknown as vscode.ExtensionContext;
    try {
      const result = await resolveServer(context, { ...config(), serverVersion: '' }, {
        appendLine: () => undefined
      } as unknown as vscode.OutputChannel, runtime);
      if ('command' in result) {
        assert.fail(`expected a descriptor failure, got ${JSON.stringify(result)}`);
      }
      assert.match(result.message, /distribution descriptor is invalid/);
      assert.match(result.detail, /product version mismatch/);
      assert.deepStrictEqual(attempts, []);
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  });

  test('binds the managed version to a matching embedded descriptor', async () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'ripr-resolver-descriptor-'));
    const descriptor = {
      schema: 1,
      productVersion: '0.10.0',
      channel: 'stable',
      releaseTag: 'v0.10.0',
      releaseRef: 'refs/tags/v0.10.0',
      manifestFile: 'ripr-server-manifest-v0.10.0.json',
      sourceRepository: 'https://github.com/EffortlessMetrics/ripr'
    };
    fs.writeFileSync(path.join(root, 'distribution.json'), JSON.stringify(descriptor));
    const attempts: string[] = [];
    const runtime: ServerResolverRuntime = {
      probeCandidate: async (command, source, detail, _useShell, installationState = 'unmanaged') => {
        attempts.push(source);
        if (source === 'bundled') {
          return {
            message: `${detail} is not LSP compatible.`,
            detail: '[missing_required_capability] hoverProvider'
          };
        }
        return {
          command,
          source,
          detail,
          installationState,
          compatibilityResult: compatibleLspEvidence
        };
      }
    };
    const context = {
      extensionUri: vscode.Uri.file(root),
      globalStorageUri: vscode.Uri.file(path.join(root, 'storage')),
      extension: { packageJSON: { version: '0.10.0' } }
    } as unknown as vscode.ExtensionContext;
    try {
      const result = await resolveServer(context, { ...config(), serverVersion: '' }, {
        appendLine: () => undefined
      } as unknown as vscode.OutputChannel, runtime);
      assert.ok('command' in result, JSON.stringify(result));
      // No bundled server file exists, so only the PATH fallback is probed.
      assert.deepStrictEqual(attempts, ['path']);
      assert.strictEqual(result.command, 'ripr');
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
    }
  });
});

function config(): RiprConfig {
  return {
    enabled: true,
    serverPath: '',
    serverArgs: ['lsp', '--stdio'],
    autoDownload: false,
    serverVersion: '0.10.0',
    downloadBaseUrl: '',
    checkMode: 'draft',
    baseRef: 'origin/main',
    includeUnchangedTests: true,
    seamDiagnostics: true,
    diagnosticProfile: 'actionable',
    traceServer: 'off'
  };
}
