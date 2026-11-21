import * as fs from 'fs';
import * as path from 'path';
import * as zlib from 'zlib';

/**
 * Archive inventory authority for #1641.
 *
 * Member bytes are decoded by repository-owned bounded parsers (tar, zip
 * central directory), every name passes one normalization, kinds pass a
 * closed policy, and resource budgets bind before any filesystem effect.
 * Only the resulting plan's regular files are ever materialized, each
 * created new; rejected archives leave no files behind. No
 * general-purpose system extractor touches untrusted bytes anywhere in
 * this path, and no new runtime dependency enters the lockfile for it.
 */

export type ArchiveFormat = 'tar.gz' | 'zip';

export interface ArchiveMember {
  readonly path: string;
  readonly kind: 'file' | 'directory';
  readonly size: number;
}

export interface ExtractionPlan {
  readonly files: readonly ArchiveMember[];
  readonly executable: ArchiveMember;
}

export interface ArchivePolicy {
  readonly maxEntries: number;
  readonly maxTotalUncompressedBytes: number;
  readonly maxEntryBytes: number;
  readonly maxPathChars: number;
  readonly maxCompressionRatio: number;
}

export interface ArchiveInventory {
  readonly members: readonly ArchiveMember[];
}

export const DEFAULT_ARCHIVE_POLICY: ArchivePolicy = {
  maxEntries: 4096,
  maxTotalUncompressedBytes: 512 * 1_048_576,
  maxEntryBytes: 256 * 1_048_576,
  maxPathChars: 1024,
  maxCompressionRatio: 64
};

const WINDOWS_DEVICE_PATTERN = /^(con|prn|aux|nul|com[1-9]|lpt[1-9])(\..*)?$/i;

/** Normalize one archive member name to a posix-relative path or throw. */
export function normalizeMemberPath(rawName: string, maxPathChars: number): string {
  if (rawName.length === 0 || rawName.length > maxPathChars) {
    throw new Error('Archive member name is empty or exceeds the path bound.');
  }
  for (let index = 0; index < rawName.length; index += 1) {
    const code = rawName.charCodeAt(index);
    if (code < 0x20 || code === 0x7f) {
      throw new Error('Archive member name carries control characters.');
    }
    if (code >= 0xd800 && code <= 0xdfff) {
      throw new Error('Archive member name carries an unpaired surrogate.');
    }
    if (code > 0x7e) {
      throw new Error('Archive member name carries non-ASCII bytes.');
    }
  }
  if (rawName.includes('\\')) {
    throw new Error('Archive member name mixes path separators.');
  }
  if (rawName.startsWith('/') || /^[a-zA-Z]:/.test(rawName)) {
    throw new Error('Archive member name is absolute or drive-prefixed.');
  }
  const trimmed = rawName.endsWith('/') ? rawName.slice(0, -1) : rawName;
  const parts = trimmed.split('/');
  for (const part of parts) {
    if (part.length === 0 || part === '.') {
      throw new Error('Archive member name carries an empty or dot component.');
    }
    if (part === '..') {
      throw new Error('Archive member name escapes its root.');
    }
    if (part.length > 255) {
      throw new Error('Archive member path component exceeds the bound.');
    }
    if (/[. ]$/.test(part) || WINDOWS_DEVICE_PATTERN.test(part)) {
      throw new Error(`Archive member name ${JSON.stringify(rawName)} is ambiguous on Windows targets.`);
    }
  }
  return parts.join('/');
}

interface RawMember {
  readonly path: string;
  readonly kind: 'file' | 'directory';
  readonly size: number;
  readonly data?: Buffer;
}

/** Decode, normalize, police, and budget raw archive bytes. */
export function inventoryArchiveBytes(
  bytes: Buffer,
  format: ArchiveFormat,
  policy: ArchivePolicy = DEFAULT_ARCHIVE_POLICY
): ArchiveInventory {
  const rawMembers = format === 'tar.gz' ? parseTarMembers(bytes) : parseZipMembers(bytes);
  return { members: policeMembers(rawMembers, bytes, policy) };
}

/**
 * Select the extraction plan: the single member whose final component
 * matches the manifest-contract executable name. Missing or ambiguous
 * selections fail closed instead of falling back to basename search.
 */
export function policeMembers(
  rawMembers: RawMember[],
  bytes: Buffer,
  policy: ArchivePolicy = DEFAULT_ARCHIVE_POLICY
): ArchiveMember[] {
  if (rawMembers.length === 0 || rawMembers.length > policy.maxEntries) {
    throw new Error('Archive carries an inadmissible member count.');
  }
  const members: ArchiveMember[] = [];
  const exact = new Set<string>();
  const folded = new Set<string>();
  let totalUncompressed = 0;
  for (const raw of rawMembers) {
    const normalized = normalizeMemberPath(raw.path, policy.maxPathChars);
    if (exact.has(normalized)) {
      throw new Error(`Archive carries a duplicate member path ${normalized}.`);
    }
    exact.add(normalized);
    const foldKey = normalized.toLowerCase();
    if (folded.has(foldKey)) {
      throw new Error(`Archive carries a case-folded member collision at ${normalized}.`);
    }
    folded.add(foldKey);
    if (!Number.isInteger(raw.size) || raw.size < 0 || raw.size > policy.maxEntryBytes) {
      throw new Error(`Archive member ${normalized} carries an inadmissible size.`);
    }
    totalUncompressed += raw.size;
    if (totalUncompressed > policy.maxTotalUncompressedBytes) {
      throw new Error('Archive exceeds the total uncompressed budget.');
    }
    members.push({ path: normalized, kind: raw.kind, size: raw.size });
  }
  for (const member of members) {
    for (const other of members) {
      if (other !== member && other.kind === 'directory' && member.path === other.path) {
        throw new Error(`Archive carries a file/directory collision at ${member.path}.`);
      }
    }
  }
  if (totalUncompressed > policy.maxCompressionRatio * bytes.length) {
    throw new Error('Archive exceeds the decompression-ratio budget.');
  }
  return members;
}

export function buildExtractionPlan(members: readonly ArchiveMember[], expectedExecutableName: string): ExtractionPlan {
  if (expectedExecutableName.length === 0 || expectedExecutableName.includes('/') || expectedExecutableName.includes('\\')) {
    throw new Error('Expected executable name is not a bare filename.');
  }
  const files = members.filter((member) => member.kind === 'file');
  const candidates = files.filter((member) => member.path.split('/').pop() === expectedExecutableName);
  if (candidates.length !== 1) {
    throw new Error(
      `Archive carries ${candidates.length} executable candidates for ${expectedExecutableName}; expected exactly one.`
    );
  }
  return { files, executable: candidates[0] as ArchiveMember };
}

/**
 * Inventory, plan, and materialize. Directories are created, regular files
 * are created new with 0o644; nothing else is ever materialized. Returns
 * the planned executable path. Rejection leaves the destination empty.
 */
export async function extractAdmittedArchive(
  bytes: Buffer,
  format: ArchiveFormat,
  destination: string,
  expectedExecutableName: string,
  policy: ArchivePolicy = DEFAULT_ARCHIVE_POLICY
): Promise<string> {
  const rawMembers = format === 'tar.gz' ? parseTarMembers(bytes) : parseZipMembers(bytes);
  const members = policeMembers(rawMembers, bytes, policy);
  const plan = buildExtractionPlan(members, expectedExecutableName);
  const byPath = new Map(rawMembers.map((member) => [normalizeMemberPath(member.path, policy.maxPathChars), member]));
  for (const member of members) {
    const target = path.join(destination, ...member.path.split('/'));
    assertInside(destination, target);
    if (member.kind === 'directory') {
      await fs.promises.mkdir(target, { recursive: true, mode: 0o755 });
      continue;
    }
    const raw = byPath.get(member.path);
    if (raw?.data === undefined) {
      throw new Error(`Archive member ${member.path} carries no materializable bytes.`);
    }
    await fs.promises.mkdir(path.dirname(target), { recursive: true, mode: 0o755 });
    await fs.promises.writeFile(target, raw.data, { flag: 'wx', mode: 0o644 });
  }
  return path.join(destination, ...plan.executable.path.split('/'));
}

function assertInside(root: string, candidate: string): void {
  const relative = path.relative(path.resolve(root), path.resolve(candidate));
  if (relative.length === 0 || relative.startsWith(`..${path.sep}`) || relative === '..' || path.isAbsolute(relative)) {
    throw new Error('Archive member escapes the destination root.');
  }
}

/* Tar parsing (ustar, pax name/size/linkname, GNU longname). */

function parseOctal(value: Buffer): number {
  if (value[0] === 0x80) {
    throw new Error('Archive carries a base-256 numeric field.');
  }
  const text = value.toString('utf8', 0, value.indexOf(0x00) === -1 ? value.length : value.indexOf(0x00)).trim();
  if (!/^[0-7]*$/.test(text)) {
    throw new Error('Archive carries a malformed octal field.');
  }
  const parsed = parseInt(text === '' ? '0' : text, 8);
  if (!Number.isSafeInteger(parsed)) {
    throw new Error('Archive carries an out-of-range numeric field.');
  }
  return parsed;
}

function parseTarMembers(bytes: Buffer): RawMember[] {
  let tar: Buffer;
  try {
    tar = zlib.gunzipSync(bytes, { maxOutputLength: DEFAULT_ARCHIVE_POLICY.maxTotalUncompressedBytes + 1 });
  } catch {
    throw new Error('Archive is not valid gzip data.');
  }
  const members: RawMember[] = [];
  let offset = 0;
  let pendingLongName: string | undefined;
  let pendingPax: Record<string, string> | undefined;
  const readBlock = (): Buffer => {
    if (offset + 512 > tar.length) {
      throw new Error('Archive ends mid-header.');
    }
    const block = tar.subarray(offset, offset + 512);
    offset += 512;
    return block;
  };
  for (;;) {
    const header = readBlock();
    if (header.every((byte) => byte === 0)) {
      const rest = tar.subarray(offset);
      if (!rest.every((byte) => byte === 0)) {
        throw new Error('Archive carries trailing data after its end blocks.');
      }
      return members;
    }
    const magic = header.toString('utf8', 257, 262);
    if (magic !== 'ustar') {
      throw new Error('Archive carries a non-ustar member header.');
    }
    let checksum = 0;
    for (let index = 0; index < 512; index += 1) {
      checksum += index >= 148 && index < 156 ? 0x20 : header[index] as number;
    }
    if (checksum !== parseOctal(header.subarray(148, 156))) {
      throw new Error('Archive carries a corrupt member checksum.');
    }
    const rawName = header.toString('utf8', 0, 100).split('\0')[0] as string;
    const prefix = header.toString('utf8', 345, 500).split('\0')[0] as string;
    const typeflag = String.fromCharCode(header[156] as number);
    const size = parseOctal(header.subarray(124, 136));
    const dataBlocks = Math.ceil(size / 512);
    if (offset + dataBlocks * 512 > tar.length) {
      throw new Error('Archive member data overruns the stream.');
    }
    const data = Buffer.from(tar.subarray(offset, offset + size));
    offset += dataBlocks * 512;
    if (typeflag === 'x' || typeflag === 'g') {
      pendingPax = parsePaxRecords(data);
      continue;
    }
    if (typeflag === 'L' || typeflag === 'K') {
      pendingLongName = data.toString('utf8').split('\0')[0];
      continue;
    }
    if (typeflag === 'S' || typeflag === 'M') {
      throw new Error('Archive carries a sparse or multi-volume member.');
    }
    const pax = pendingPax;
    pendingPax = undefined;
    const longName = pendingLongName;
    pendingLongName = undefined;
    if (pax?.['GNU.sparse.size'] !== undefined || pax?.['GNU.sparse.numblocks'] !== undefined) {
      throw new Error('Archive carries sparse metadata.');
    }
    if (pax?.linkname !== undefined || typeflag === '1' || typeflag === '2') {
      throw new Error('Archive carries a link member.');
    }
    if (typeflag === '3' || typeflag === '4' || typeflag === '6') {
      throw new Error('Archive carries a device or special member.');
    }
    const name = longName ?? pax?.path ?? (prefix.length > 0 ? `${prefix}/${rawName}` : rawName);
    const memberSize = pax?.size !== undefined ? parsePaxSize(pax.size) : size;
    if (typeflag === '5') {
      members.push({ path: name, kind: 'directory', size: 0 });
      continue;
    }
    if (typeflag === '0' || typeflag === '\0') {
      members.push({ path: name, kind: 'file', size: memberSize, data });
      continue;
    }
    throw new Error(`Archive carries member kind ${JSON.stringify(typeflag)}.`);
  }
}

function parsePaxRecords(data: Buffer): Record<string, string> {
  const records: Record<string, string> = {};
  let offset = 0;
  while (offset < data.length) {
    const newline = data.indexOf(0x0a, offset);
    if (newline === -1) {
      throw new Error('Archive carries a malformed pax record.');
    }
    const line = data.toString('utf8', offset, newline);
    const firstSpace = line.indexOf(' ');
    const secondSpace = line.indexOf(' ', firstSpace + 1);
    if (firstSpace === -1 || secondSpace === -1) {
      throw new Error('Archive carries a malformed pax record.');
    }
    const keyValue = line.slice(secondSpace + 1);
    const separator = keyValue.indexOf('=');
    if (separator === -1) {
      throw new Error('Archive carries a malformed pax record.');
    }
    records[keyValue.slice(0, separator)] = keyValue.slice(separator + 1);
    offset = newline + 1;
  }
  return records;
}

function parsePaxSize(value: string): number {
  if (!/^[0-9]+$/.test(value)) {
    throw new Error('Archive carries a malformed pax size.');
  }
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed)) {
    throw new Error('Archive carries an out-of-range pax size.');
  }
  return parsed;
}

/* Zip parsing over the central directory; data verified against it. */

function crc32(data: Buffer): number {
  let table = (globalThis as Record<string, unknown>).__riprZipCrc as number[] | undefined;
  if (table === undefined) {
    table = [];
    for (let n = 0; n < 256; n += 1) {
      let c = n;
      for (let k = 0; k < 8; k += 1) {
        c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
      }
      table[n] = c >>> 0;
    }
    (globalThis as Record<string, unknown>).__riprZipCrc = table;
  }
  let crc = 0xffffffff;
  for (const byte of data) {
    crc = (table as number[])[(crc ^ byte) & 0xff] ^ (crc >>> 8);
  }
  return (crc ^ 0xffffffff) >>> 0;
}

interface ZipCentral {
  readonly name: string;
  readonly method: number;
  readonly flags: number;
  readonly crc: number;
  readonly compressedSize: number;
  readonly uncompressedSize: number;
  readonly externalAttributes: number;
  readonly localHeaderOffset: number;
}

function findEndOfCentral(bytes: Buffer): number {
  for (let offset = bytes.length - 22; offset >= 0; offset -= 1) {
    if (bytes.readUInt32LE(offset) === 0x06054b50) {
      const commentLength = bytes.readUInt16LE(offset + 20);
      if (offset + 22 + commentLength === bytes.length) {
        if (bytes.readUInt16LE(offset + 4) !== 0 || bytes.readUInt16LE(offset + 6) !== 0) {
          throw new Error('Archive carries multi-disk zip metadata.');
        }
        return offset;
      }
    }
  }
  throw new Error('Archive carries no zip end-of-central record.');
}

function parseZipMembers(bytes: Buffer): RawMember[] {
  const endOffset = findEndOfCentral(bytes);
  const count = bytes.readUInt16LE(endOffset + 10);
  if (count === 0xffff) {
    throw new Error('Archive carries zip64 metadata.');
  }
  let offset = bytes.readUInt32LE(endOffset + 16);
  const centrals: ZipCentral[] = [];
  for (let index = 0; index < count; index += 1) {
    if (offset + 46 > bytes.length || bytes.readUInt32LE(offset) !== 0x02014b50) {
      throw new Error('Archive carries a malformed zip central record.');
    }
    const method = bytes.readUInt16LE(offset + 10);
    const flags = bytes.readUInt16LE(offset + 8);
    const nameLength = bytes.readUInt16LE(offset + 28);
    const extraLength = bytes.readUInt16LE(offset + 30);
    const commentLength = bytes.readUInt16LE(offset + 32);
    const name = bytes.toString('utf8', offset + 46, offset + 46 + nameLength);
    if (flags & 0x1) {
      throw new Error('Archive carries an encrypted zip entry.');
    }
    if (method !== 0 && method !== 8) {
      throw new Error('Archive carries an unsupported zip compression method.');
    }
    centrals.push({
      name,
      method,
      flags,
      crc: bytes.readUInt32LE(offset + 16),
      compressedSize: bytes.readUInt32LE(offset + 20),
      uncompressedSize: bytes.readUInt32LE(offset + 24),
      externalAttributes: bytes.readUInt32LE(offset + 38),
      localHeaderOffset: bytes.readUInt32LE(offset + 42)
    });
    offset += 46 + nameLength + extraLength + commentLength;
  }
  return centrals.map((central) => zipMemberData(bytes, central));
}

function zipMemberData(bytes: Buffer, central: ZipCentral): RawMember {
  const attributes = central.externalAttributes;
  const unixMode = attributes >>> 16;
  const unixType = (unixMode >>> 12) & 0o17;
  const dosDirectory = (attributes & 0x10) !== 0;
  if (unixType === 0o12) {
    throw new Error('Archive carries a zip symlink entry.');
  }
  if (unixType !== 0 && unixType !== 0o04 && unixType !== 0o10) {
    throw new Error('Archive carries a zip special entry.');
  }
  const kind = central.name.endsWith('/') || unixType === 0o04 || dosDirectory ? 'directory' : 'file';
  const headerOffset = central.localHeaderOffset;
  if (headerOffset + 30 > bytes.length || bytes.readUInt32LE(headerOffset) !== 0x04034b50) {
    throw new Error('Archive carries a malformed zip local header.');
  }
  const localNameLength = bytes.readUInt16LE(headerOffset + 26);
  const localExtraLength = bytes.readUInt16LE(headerOffset + 28);
  const dataOffset = headerOffset + 30 + localNameLength + localExtraLength;
  const localName = bytes.toString('utf8', headerOffset + 30, headerOffset + 30 + localNameLength);
  if (localName !== central.name) {
    throw new Error('Archive carries a zip name mismatch between headers.');
  }
  if (dataOffset + central.compressedSize > bytes.length) {
    throw new Error('Archive zip entry overruns the stream.');
  }
  const stored = bytes.subarray(dataOffset, dataOffset + central.compressedSize);
  let data: Buffer;
  if (central.method === 0) {
    data = Buffer.from(stored);
  } else {
    try {
      data = zlib.inflateRawSync(stored, { maxOutputLength: central.uncompressedSize + 1 });
    } catch {
      throw new Error('Archive carries an undecodable deflated entry.');
    }
  }
  if (data.length !== central.uncompressedSize) {
    throw new Error('Archive zip entry size disagrees with its central record.');
  }
  if (crc32(data) !== central.crc) {
    throw new Error('Archive zip entry checksum disagrees with its central record.');
  }
  if (kind === 'directory') {
    return { path: central.name.endsWith('/') ? central.name.slice(0, -1) : central.name, kind: 'directory', size: 0 };
  }
  return { path: central.name, kind: 'file', size: central.uncompressedSize, data };
}
