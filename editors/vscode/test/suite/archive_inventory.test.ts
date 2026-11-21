import * as assert from 'assert';
import * as zlib from 'zlib';
import {
  buildExtractionPlan,
  extractAdmittedArchive,
  inventoryArchiveBytes
} from '../../src/archiveInventory';

function gzipTar(parts: Buffer[]): Buffer {
  return zlib.gzipSync(Buffer.concat(parts));
}

/* Minimal tar builder: ustar header + data + end blocks. */

function tarHeader(name: string, size: number, typeflag: string, mode = 0o644): Buffer {
  const header = Buffer.alloc(512, 0);
  Buffer.from(name, 'utf8').copy(header, 0, 0, Math.min(name.length, 100));
  Buffer.from(mode.toString(8).padStart(7, '0').slice(-7), 'utf8').copy(header, 100);
  Buffer.from(size.toString(8).padStart(11, '0').slice(-12), 'utf8').copy(header, 124);
  Buffer.from('        ', 'utf8').copy(header, 148);
  header[156] = typeflag.charCodeAt(0);
  Buffer.from('ustar', 'utf8').copy(header, 257);
  let checksum = 0;
  for (const byte of header) {
    checksum += byte;
  }
  Buffer.from(`${checksum.toString(8).padStart(6, '0')}\0 `, 'utf8').copy(header, 148);
  return header;
}

function tarFile(name: string, contents: string, typeflag = '0'): Buffer {
  const data = Buffer.from(contents, 'utf8');
  const padded = Buffer.alloc(Math.ceil(data.length / 512) * 512, 0);
  data.copy(padded);
  return Buffer.concat([tarHeader(name, data.length, typeflag), padded]);
}

function tarEnd(): Buffer {
  return Buffer.alloc(1024, 0);
}

/* Minimal stored-method zip builder with a tiny CRC32. */

function crc32(data: Buffer): number {
  let table: number[] | undefined;
  const local = (globalThis as Record<string, unknown>).__riprCrcTable as number[] | undefined;
  if (local !== undefined) {
    table = local;
  } else {
    table = [];
    for (let n = 0; n < 256; n += 1) {
      let c = n;
      for (let k = 0; k < 8; k += 1) {
        c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
      }
      table[n] = c >>> 0;
    }
    (globalThis as Record<string, unknown>).__riprCrcTable = table;
  }
  let crc = 0xffffffff;
  for (const byte of data) {
    crc = (table as number[])[(crc ^ byte) & 0xff] ^ (crc >>> 8);
  }
  return (crc ^ 0xffffffff) >>> 0;
}

interface ZipEntry {
  readonly name: string;
  readonly data: string;
  readonly externalAttributes?: number;
}

function zipArchive(entries: ZipEntry[]): Buffer {
  const chunks: Buffer[] = [];
  const central: Buffer[] = [];
  let offset = 0;
  for (const entry of entries) {
    const name = Buffer.from(entry.name, 'utf8');
    const data = Buffer.from(entry.data, 'utf8');
    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4);
    local.writeUInt16LE(0, 8);
    local.writeUInt16LE(0, 8 + 2);
    local.writeUInt32LE(crc32(data), 14);
    local.writeUInt32LE(data.length, 18);
    local.writeUInt32LE(data.length, 22);
    local.writeUInt16LE(name.length, 26);
    local.writeUInt16LE(0, 28);
    chunks.push(local, name, data);
    const record = Buffer.alloc(46);
    record.writeUInt32LE(0x02014b50, 0);
    record.writeUInt16LE(20, 6);
    record.writeUInt16LE(0, 8);
    record.writeUInt16LE(0, 10);
    record.writeUInt32LE(crc32(data), 16);
    record.writeUInt32LE(data.length, 20);
    record.writeUInt32LE(data.length, 24);
    record.writeUInt16LE(name.length, 28);
    record.writeUInt32LE((entry.externalAttributes ?? 0) >>> 0, 38);
    record.writeUInt32LE(offset, 42);
    central.push(record, name);
    offset += local.length + name.length + data.length;
  }
  const centralStart = offset;
  const centralBytes = Buffer.concat(central);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(entries.length, 8);
  end.writeUInt16LE(entries.length, 10);
  end.writeUInt32LE(centralBytes.length, 12);
  end.writeUInt32LE(centralStart, 16);
  return Buffer.concat([...chunks, centralBytes, end]);
}

const S_IFREG = 0o100000;
const S_IFLNK = 0o120000;

suite('archive inventory authority', () => {
  test('admits a minimal well-formed tar with the expected executable', () => {
    const bytes = gzipTar([
      tarFile('pkg/ripr', 'binary'),
      tarFile('pkg/README.md', 'docs'),
      tarEnd()
    ]);
    const inventory = inventoryArchiveBytes(bytes, 'tar.gz');
    assert.deepStrictEqual(
      inventory.members.filter((m) => m.kind === 'file').map((m) => m.path),
      ['pkg/ripr', 'pkg/README.md']
    );
    assert.strictEqual(buildExtractionPlan(inventory.members, 'ripr').executable.path, 'pkg/ripr');
  });

  test('rejects traversal, absolute, and drive-prefixed members before materialization', () => {
    for (const hostile of ['../evil', '/abs', 'C:/path/to/evil', '..\\win', 'pkg/../../evil']) {
      const bytes = gzipTar([tarFile(hostile, 'x'), tarFile('pkg/ripr', 'binary'), tarEnd()]);
      assert.throws(() => inventoryArchiveBytes(bytes, 'tar.gz'), /archive/i, hostile);
    }
  });

  test('rejects symlink, hardlink, and device members', () => {
    for (const typeflag of ['1', '2', '3', '4', '6']) {
      const bytes = gzipTar([tarFile('pkg/link', 'x', typeflag), tarFile('pkg/ripr', 'binary'), tarEnd()]);
      assert.throws(() => inventoryArchiveBytes(bytes, 'tar.gz'), /archive|member|kind/i, `typeflag ${typeflag}`);
    }
  });

  test('rejects duplicate and case-folded member paths', () => {
    const duplicate = gzipTar([tarFile('pkg/ripr', 'a'), tarFile('pkg/ripr', 'b'), tarEnd()]);
    assert.throws(() => inventoryArchiveBytes(duplicate, 'tar.gz'), /duplicate/i);
    const folded = gzipTar([tarFile('pkg/RIPR', 'a'), tarFile('pkg/ripr', 'b'), tarEnd()]);
    assert.throws(() => inventoryArchiveBytes(folded, 'tar.gz'), /collision|duplicate/i);
  });

  test('rejects corrupt checksum and absurd declared sizes', () => {
    const header = tarHeader('pkg/ripr', 4, '0');
    header[148] = header[148] === 0x30 ? 0x31 : 0x30;
    const corrupt = gzipTar([header, Buffer.alloc(512, 0), tarEnd()]);
    assert.throws(() => inventoryArchiveBytes(corrupt, 'tar.gz'), /checksum|archive/i);
    const huge = gzipTar([tarHeader('pkg/ripr', 0o177777777777, '0'), tarEnd()]);
    assert.throws(() => inventoryArchiveBytes(huge, 'tar.gz'), /size|bound|archive|overrun/i);
  });

  test('admits a minimal stored zip and rejects traversal plus symlink entries', () => {
    const good = zipArchive([
      { name: 'pkg/', data: '', externalAttributes: (0o040000 | 0o755) << 16 },
      { name: 'pkg/ripr', data: 'binary', externalAttributes: (S_IFREG | 0o755) << 16 }
    ]);
    const inventory = inventoryArchiveBytes(good, 'zip');
    assert.strictEqual(buildExtractionPlan(inventory.members, 'ripr').executable.path, 'pkg/ripr');

    const traversal = zipArchive([{ name: '../evil', data: 'x' }]);
    assert.throws(() => inventoryArchiveBytes(traversal, 'zip'), /archive/i);
    const link = zipArchive([
      { name: 'pkg/ripr', data: 'binary' },
      { name: 'pkg/link', data: 'pkg/ripr', externalAttributes: (S_IFLNK | 0o777) << 16 }
    ]);
    assert.throws(() => inventoryArchiveBytes(link, 'zip'), /link|kind|archive/i);
  });

  test('executable selection fails closed on missing or ambiguous candidates', () => {
    const missing = gzipTar([tarFile('pkg/README.md', 'docs'), tarEnd()]);
    const missingMembers = inventoryArchiveBytes(missing, 'tar.gz').members;
    assert.throws(() => buildExtractionPlan(missingMembers, 'ripr'), /exactly one/i);
    const ambiguous = gzipTar([tarFile('pkg/ripr', 'a'), tarFile('other/ripr', 'b'), tarEnd()]);
    const ambiguousMembers = inventoryArchiveBytes(ambiguous, 'tar.gz').members;
    assert.throws(() => buildExtractionPlan(ambiguousMembers, 'ripr'), /exactly one/i);
  });

  test('materializes only admitted files and selects the planned executable', async () => {
    const fs = await import('fs');
    const os = await import('os');
    const path = await import('path');
    const root = await fs.promises.mkdtemp(path.join(os.tmpdir(), 'ripr-archive-'));
    try {
      const bytes = gzipTar([tarFile('pkg/ripr', 'binary'), tarEnd()]);
      const executablePath = await extractAdmittedArchive(bytes, 'tar.gz', root, 'ripr');
      assert.strictEqual(executablePath, path.join(root, 'pkg', 'ripr'));
      assert.strictEqual(await fs.promises.readFile(executablePath, 'utf8'), 'binary');
    } finally {
      await fs.promises.rm(root, { recursive: true, force: true });
    }
  });

  test('hostile archives leave no materialized files behind', async () => {
    const fs = await import('fs');
    const os = await import('os');
    const path = await import('path');
    const root = await fs.promises.mkdtemp(path.join(os.tmpdir(), 'ripr-archive-'));
    try {
      const hostile = gzipTar([tarFile('../evil', 'x'), tarFile('pkg/ripr', 'binary'), tarEnd()]);
      await assert.rejects(extractAdmittedArchive(hostile, 'tar.gz', root, 'ripr'), /archive/i);
      const remaining = await fs.promises.readdir(root);
      assert.deepStrictEqual(remaining, []);
    } finally {
      await fs.promises.rm(root, { recursive: true, force: true });
    }
  });
});
