import * as fs from 'fs';
import * as path from 'path';
import Mocha from 'mocha';

export function run(): Promise<void> {
  const mocha = new Mocha({
    ui: 'tdd',
    color: true,
    timeout: 10000,
  });

  const testsRoot = path.resolve(__dirname);
  const files = findTestFiles(testsRoot);
  files.forEach((f) => mocha.addFile(path.resolve(testsRoot, f)));

  return new Promise((resolve, reject) => {
    mocha.run((failures: number) => {
      if (failures > 0) {
        reject(new Error(`${failures} tests failed.`));
      } else {
        resolve();
      }
    });
  });
}

function findTestFiles(dir: string, relative = ''): string[] {
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  const files: string[] = [];
  for (const entry of entries) {
    const name = entry.name;
    const rel = path.join(relative, name);
    if (entry.isDirectory()) {
      files.push(...findTestFiles(path.join(dir, name), rel));
    } else if (name.endsWith('.test.js')) {
      files.push(rel);
    }
  }
  return files;
}
