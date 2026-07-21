import * as path from 'path';
import { runTests } from '@vscode/test-electron';

async function main() {
    try {
        // The path to the test workspace (a fixture with a Cargo.toml).
        const workspacePath = process.env.RIPR_TEST_WORKSPACE_PATH ?? path.resolve(__dirname, '..', '..', '..', 'crates', 'ripr');

        // Download VS Code, unzip it, and run the tests.
        await runTests({
            extensionDevelopmentPath: path.resolve(__dirname, '..', '..'),
            extensionTestsPath: path.resolve(__dirname, 'suite', 'index'),
            launchArgs: [workspacePath],
        });
    } catch (err) {
        console.error('Failed to run tests:', err);
        process.exit(1);
    }
}

main();
