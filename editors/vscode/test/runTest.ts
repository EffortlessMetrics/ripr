import * as path from 'path';
import * as fs from 'fs';
import { execFileSync, spawn } from 'child_process';
import { downloadAndUnzipVSCode, runTests } from '@vscode/test-electron';

function runGit(workspacePath: string, args: string[]): string {
  return execFileSync('git', args, {
    cwd: workspacePath,
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe']
  }).trim();
}

function stageIntegrationWorkspace(templatePath: string, workspacePath: string): string {
  fs.rmSync(workspacePath, { force: true, recursive: true });
  fs.cpSync(templatePath, workspacePath, { recursive: true });
  // The template carries editor fixtures, not a valid analysis cache for the
  // newly-created Git repository. Reusing it can make the server downgrade a
  // full refresh to `cache_limited` and suppress the evidence this composition
  // test is meant to exercise.
  fs.rmSync(path.join(workspacePath, 'target'), { force: true, recursive: true });
  fs.mkdirSync(path.join(workspacePath, 'src'), { recursive: true });
  fs.mkdirSync(path.join(workspacePath, 'tests'), { recursive: true });
  fs.mkdirSync(path.join(workspacePath, '.vscode'), { recursive: true });
  fs.writeFileSync(
    path.join(workspacePath, '.vscode', 'settings.json'),
    `${JSON.stringify({
      'ripr.seamDiagnostics': true,
      'ripr.diagnosticProfile': 'actionable'
    }, null, 2)}\n`
  );
  fs.writeFileSync(
    path.join(workspacePath, 'ripr.toml'),
    '[languages]\nenabled = ["rust", "typescript"]\n'
  );
  fs.writeFileSync(
    path.join(workspacePath, 'src', 'lib.rs'),
    [
      'pub fn discounted_total(amount: i32, discount_threshold: i32) -> i32 {',
      '    if amount > discount_threshold {',
      '        amount - 10',
      '    } else {',
      '        amount',
      '    }',
      '}',
      ''
    ].join('\n')
  );
  fs.writeFileSync(
    path.join(workspacePath, 'src', 'pricing.ts'),
    [
      'export function discountedTotal(amount: number, threshold: number): number {',
      '    if (amount > threshold) {',
      '        return amount - 10;',
      '    }',
      '    return amount;',
      '}',
      ''
    ].join('\n')
  );
  fs.writeFileSync(
    path.join(workspacePath, 'tests', 'pricing.rs'),
    [
      'use boundary_gap_fixture::discounted_total;',
      '',
      '#[test]',
      'fn below_threshold_has_no_discount() {',
      '    assert_eq!(discounted_total(50, 100), 50);',
      '}',
      '',
      '#[test]',
      'fn far_above_threshold_discounts() {',
      '    assert_eq!(discounted_total(10_000, 100), 9_990);',
      '}',
      ''
    ].join('\n')
  );
  fs.writeFileSync(
    path.join(workspacePath, 'tests', 'pricing.test.ts'),
    [
      "import { discountedTotal } from '../src/pricing';",
      '',
      "test('below threshold has no discount', () => {",
      '    const result = discountedTotal(50, 100);',
      '    if (result !== 50) {',
      "        throw new Error('expected 50');",
      '    }',
      '});',
      ''
    ].join('\n')
  );

  runGit(workspacePath, ['init', '--initial-branch', 'main']);
  runGit(workspacePath, ['config', 'user.name', 'ripr packaged integration']);
  runGit(workspacePath, ['config', 'user.email', 'ripr-packaged-integration@example.invalid']);
  runGit(workspacePath, ['add', '.']);
  runGit(workspacePath, ['commit', '-m', 'fixture: establish packaged integration baseline']);
  const baseRef = runGit(workspacePath, ['rev-parse', 'HEAD']);

  fs.writeFileSync(
    path.join(workspacePath, 'src', 'lib.rs'),
    fs.readFileSync(path.join(workspacePath, 'src', 'lib.rs'), 'utf8').replace('amount > discount_threshold', 'amount >= discount_threshold')
  );
  fs.writeFileSync(
    path.join(workspacePath, 'src', 'pricing.ts'),
    fs.readFileSync(path.join(workspacePath, 'src', 'pricing.ts'), 'utf8').replace('amount > threshold', 'amount >= threshold')
  );
  runGit(workspacePath, ['add', '.']);
  runGit(workspacePath, ['commit', '-m', 'fixture: publish changed behavior']);
  return baseRef;
}

async function runUntrustedTests(
  cachePath: string,
  extensionDevelopmentPath: string,
  extensionTestsPath: string,
  launchArgs: string[]
): Promise<void> {
  const vscodeExecutablePath = await downloadAndUnzipVSCode({
    cachePath,
    extensionDevelopmentPath
  });
  const args = [
    '--no-sandbox',
    '--disable-gpu-sandbox',
    '--disable-updates',
    '--skip-welcome',
    '--skip-release-notes',
    `--extensionTestsPath=${extensionTestsPath}`,
    `--extensionDevelopmentPath=${extensionDevelopmentPath}`,
    ...launchArgs
  ];

  await new Promise<void>((resolve, reject) => {
    const child = spawn(vscodeExecutablePath, args, {
      env: process.env,
      shell: false,
      stdio: 'inherit'
    });
    child.once('error', reject);
    child.once('close', (code, signal) => {
      if (code === 0) {
        resolve();
        return;
      }
      reject(new Error(`untrusted VS Code test host exited with ${code ?? signal}`));
    });
  });
}

async function main() {
  try {
    const extensionDevelopmentPath = path.resolve(
      process.env.RIPR_TEST_EXTENSION_PATH ?? path.resolve(__dirname, '../../')
    );
    const extensionTestsPath = path.resolve(__dirname, './suite/index');
    const templatePath = path.resolve(
      process.env.RIPR_TEST_WORKSPACE_PATH ??
        path.resolve(__dirname, '../../test-fixtures/workspace')
    );
    const artifactRoot = path.resolve(
      process.env.RIPR_TEST_ARTIFACT_ROOT ??
        path.resolve(__dirname, '../../../../target/ripr')
    );
    const cachePath = path.join(artifactRoot, 'vscode-test-cache');
    const runId = String(process.pid);
    const extensionsPath = path.join(artifactRoot, 'vscode-test-extensions', runId);
    const userDataPath = path.join(artifactRoot, 'vscode-test-user-data', runId);
    const workspacePath = path.join(artifactRoot, 'workspace');
    const baseRef = stageIntegrationWorkspace(templatePath, workspacePath);
    const workspaceTrustMode = parseWorkspaceTrustMode(process.env.RIPR_TEST_WORKSPACE_TRUST);
    process.env.RIPR_TEST_BASE_REF = baseRef;
    fs.mkdirSync(cachePath, { recursive: true });
    fs.mkdirSync(extensionsPath, { recursive: true });
    fs.mkdirSync(userDataPath, { recursive: true });
    const clipboardCapturePath = path.join(userDataPath, 'ripr-test-clipboard.txt');
    fs.rmSync(clipboardCapturePath, { force: true });
    process.env.RIPR_TEST_CLIPBOARD_CAPTURE_PATH = clipboardCapturePath;

    // VS Code parses workspace trust switches before the workspace path.
    // This affects only the isolated extension-test host; production keeps
    // defaultRuntime.isWorkspaceTrusted() bound to vscode.workspace.isTrusted.
    const launchArgs = [
      ...(workspaceTrustMode === 'untrusted' ? [] : ['--disable-workspace-trust']),
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
          'ripr.baseRef': baseRef,
          'ripr.check.mode': 'draft',
          'ripr.seamDiagnostics': true,
          'ripr.diagnosticProfile': 'actionable',
          'security.workspace.trust.enabled': workspaceTrustMode !== 'trusted',
        }, null, 2)}\n`
      );
    }

    if (workspaceTrustMode === 'untrusted') {
      await runUntrustedTests(cachePath, extensionDevelopmentPath, extensionTestsPath, launchArgs);
    } else {
      await runTests({
        cachePath,
        extensionDevelopmentPath,
        extensionTestsPath,
        launchArgs,
      });
    }
  } catch (err) {
    console.error('Failed to run tests:', err);
    process.exit(1);
  }
}

main();

function parseWorkspaceTrustMode(value: string | undefined): 'trusted' | 'untrusted' {
  const mode = value ?? 'trusted';
  if (mode === 'trusted' || mode === 'untrusted') {
    return mode;
  }
  throw new Error(
    `RIPR_TEST_WORKSPACE_TRUST must be exactly 'trusted' or 'untrusted', got ${JSON.stringify(value)}`
  );
}
