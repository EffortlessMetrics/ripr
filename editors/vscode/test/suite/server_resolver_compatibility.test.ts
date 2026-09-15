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
