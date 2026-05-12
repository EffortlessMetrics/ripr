import * as path from 'path';
import * as fs from 'fs';
import { runTests } from '@vscode/test-electron';

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
    const clipboardCapturePath = path.join(userDataPath, 'ripr-test-clipboard.txt');
    fs.rmSync(clipboardCapturePath, { force: true });
    process.env.RIPR_TEST_CLIPBOARD_CAPTURE_PATH = clipboardCapturePath;

    const launchArgs = [
      workspacePath,
      '--disable-extensions',
      '--extensions-dir',
      extensionsPath,
      '--user-data-dir',
      userDataPath,
    ];
    const testServerPath = process.env.RIPR_TEST_SERVER_PATH;
    if (testServerPath) {
      const userSettingsPath = path.join(userDataPath, 'User');
      fs.mkdirSync(userSettingsPath, { recursive: true });
      fs.writeFileSync(
        path.join(userSettingsPath, 'settings.json'),
        `${JSON.stringify({
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

main();
