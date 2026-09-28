//
// User-facing guidance contract.
//
// `cargo xtask` is ripr's own repository automation. A user's workspace has
// no xtask, so status text, notifications and copied guidance must name
// product commands (`ripr ...`) instead. The only allowed occurrences are
// the safe-command validator allowlists, which accept commands that
// artifacts produced inside ripr's own repository may carry, and comments.
//

import * as assert from 'assert';
import { promises as fs } from 'fs';
import * as path from 'path';

const ALLOWLIST_ENTRY = /^\s*'cargo xtask [a-z0-9 -]+',$/;

suite('User-facing guidance', () => {
  test('extension source never tells users to run cargo xtask', async () => {
    const srcDir = path.resolve(__dirname, '../../../src');
    const files = (await fs.readdir(srcDir)).filter((name) => name.endsWith('.ts'));
    assert.ok(files.length > 0, `no extension sources found under ${srcDir}`);
    const offenders: string[] = [];
    for (const name of files) {
      const lines = (await fs.readFile(path.join(srcDir, name), 'utf8')).split('\n');
      lines.forEach((line, index) => {
        const trimmed = line.trim();
        if (!line.includes('cargo xtask') || trimmed.startsWith('//') || trimmed.startsWith('*')) {
          return;
        }
        if (ALLOWLIST_ENTRY.test(line)) {
          return;
        }
        offenders.push(`${name}:${index + 1}: ${trimmed}`);
      });
    }
    assert.deepStrictEqual(offenders, []);
  });
});
