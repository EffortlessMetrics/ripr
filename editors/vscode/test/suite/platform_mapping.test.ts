import * as assert from 'assert';
import { riprPlatformFor } from '../../src/platform';

suite('Managed server platform mapping', () => {
  test('Windows x64 maps to the x64 MSVC target', () => {
    assert.strictEqual(riprPlatformFor('win32', 'x64')?.target, 'x86_64-pc-windows-msvc');
  });

  test('Windows arm64 uses the emulated x64 MSVC target', () => {
    const mapped = riprPlatformFor('win32', 'arm64');
    assert.ok(mapped, 'Windows arm64 must be offered a managed server download');
    assert.strictEqual(mapped.target, 'x86_64-pc-windows-msvc');
    assert.strictEqual(mapped.executableName, 'ripr.exe');
    assert.strictEqual(mapped.archiveExtension, 'zip');
  });

  test('unsupported Windows architectures stay unmapped', () => {
    assert.strictEqual(riprPlatformFor('win32', 'ia32'), undefined);
  });

  test('non-Windows mappings are unchanged', () => {
    assert.strictEqual(riprPlatformFor('linux', 'arm64')?.target, 'aarch64-unknown-linux-gnu');
    assert.strictEqual(riprPlatformFor('darwin', 'arm64')?.target, 'aarch64-apple-darwin');
    assert.strictEqual(riprPlatformFor('freebsd', 'x64'), undefined);
  });
});
