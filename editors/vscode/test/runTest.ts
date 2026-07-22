import * as path from 'path';
import * as fs from 'fs';
import { pathToFileURL } from 'url';
import { DatabaseSync } from 'node:sqlite';
import { runTests } from '@vscode/test-electron';

const WORKSPACE_TRUST_STORAGE_KEY = 'content.trust.model.key';

async function main() {
  try {
    const extensionDevelopmentPath = path.resolve(__dirname, '../../');
    const extensionTestsPath = path.resolve(__dirname, './suite/index');
    const workspacePath = path.resolve(
      process.env.RIPR_TEST_WORKSPACE_PATH ??
        path.resolve(__dirname, '../../test-fixtures/workspace')
    );
    const cachePath = path.resolve(
      __dirname,
      '../../../../target/ripr/vscode-test-cache'
    );
    const runId = String(process.pid);
    const extensionsPath = path.resolve(
      __dirname,
      '../../../../target/ripr/vscode-test-extensions',
      runId
    );
    const userDataPath = path.resolve(
      __dirname,
      '../../../../target/ripr/vscode-test-user-data',
      runId
    );
    fs.mkdirSync(cachePath, { recursive: true });
    fs.mkdirSync(extensionsPath, { recursive: true });
    fs.mkdirSync(userDataPath, { recursive: true });
    seedTrustedWorkspaceProfile(userDataPath, workspacePath);
    const clipboardCapturePath = path.join(userDataPath, 'ripr-test-clipboard.txt');
    fs.rmSync(clipboardCapturePath, { force: true });
    process.env.RIPR_TEST_CLIPBOARD_CAPTURE_PATH = clipboardCapturePath;

    const launchArgs = [
      '--disable-workspace-trust',
      '--disable-extensions',
      '--extensions-dir',
      extensionsPath,
      '--user-data-dir',
      userDataPath,
      workspacePath,
    ];
    const testServerPath = process.env.RIPR_TEST_SERVER_PATH;
    if (testServerPath) {
      const userSettingsPath = path.join(userDataPath, 'User');
      fs.mkdirSync(userSettingsPath, { recursive: true });
      fs.writeFileSync(
        path.join(userSettingsPath, 'settings.json'),
        `${JSON.stringify({
          'security.workspace.trust.enabled': false,
          'ripr.server.path': testServerPath,
          'ripr.server.autoDownload': false,
          'ripr.baseRef': 'HEAD',
          'ripr.check.mode': 'instant',
        }, null, 2)}\n`
      );
    }

    await runTests({
      cachePath,
      extensionDevelopmentPath,
      extensionTestsPath,
      launchArgs,
    });
  } catch (err) {
    console.error('Failed to run tests:', err);
    process.exit(1);
  }
}

function seedTrustedWorkspaceProfile(userDataPath: string, workspacePath: string): void {
  // Workspace Trust is loaded from application storage before extension tests
  // activate. The isolated Electron profile therefore needs the fixture URI in
  // the same machine-scoped trust memento that VS Code writes through its UI.
  // This keeps production trust checks intact while making the trusted-path E2E
  // deterministic and independent of a developer's persisted profile.
  const globalStoragePath = path.join(userDataPath, 'User', 'globalStorage');
  fs.mkdirSync(globalStoragePath, { recursive: true });
  const database = new DatabaseSync(path.join(globalStoragePath, 'state.vscdb'));
  try {
    database.exec(
      'CREATE TABLE IF NOT EXISTS ItemTable (key TEXT UNIQUE ON CONFLICT REPLACE, value BLOB)'
    );
    const workspaceUrl = pathToFileURL(workspacePath);
    const trustState = JSON.stringify({
      uriTrustInfo: [
        {
          uri: {
            $mid: 1,
            scheme: workspaceUrl.protocol.slice(0, -1),
            authority: workspaceUrl.host,
            path: decodeURIComponent(workspaceUrl.pathname),
          },
          trusted: true,
        },
      ],
    });
    database
      .prepare('INSERT OR REPLACE INTO ItemTable (key, value) VALUES (?, ?)')
      .run(WORKSPACE_TRUST_STORAGE_KEY, trustState);
  } finally {
    database.close();
  }
}

main();
