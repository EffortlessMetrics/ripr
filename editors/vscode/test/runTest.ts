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
    const userDataPath = path.resolve(
      __dirname,
      '../../../../target/ripr/vscode-test-user-data'
    );
    fs.mkdirSync(cachePath, { recursive: true });

    const launchArgs = [workspacePath, '--disable-extensions'];
    const testServerPath = process.env.RIPR_TEST_SERVER_PATH;
    if (testServerPath) {
      const userSettingsPath = path.join(userDataPath, 'User');
      fs.mkdirSync(userSettingsPath, { recursive: true });
      fs.writeFileSync(
        path.join(userSettingsPath, 'settings.json'),
        `${JSON.stringify({
          'ripr.server.path': testServerPath,
          'ripr.server.autoDownload': false,
        }, null, 2)}\n`
      );
      launchArgs.push('--user-data-dir', userDataPath);
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
