import * as crypto from 'crypto';
import * as fs from 'fs';
import * as path from 'path';

const RECEIPT_FILE = 'install-receipt.json';
const LOCK_WAIT_MS = 120_000;
const LOCK_POLL_MS = 50;
const LOCK_HEARTBEAT_MS = 30_000;
const MANAGED_VERSION_PATTERN = /^(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-(?:0|[1-9]\d*|[0-9A-Za-z-]*[A-Za-z-][0-9A-Za-z-]*)(?:\.(?:0|[1-9]\d*|[0-9A-Za-z-]*[A-Za-z-][0-9A-Za-z-]*))*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/;

export interface InstallReceiptV1 {
  readonly schemaVersion: 1;
  readonly installationState: 'complete';
  readonly requestedVersion: string;
  readonly manifestVersion: string;
  readonly platformTarget: string;
  readonly executableName: string;
  readonly archiveSha256: string;
  readonly executableSha256: string;
  readonly binaryVersion: string;
}

export interface ManagedServerInstallation {
  readonly executablePath: string;
  readonly receiptPath: string;
  readonly receipt: InstallReceiptV1;
}

export interface ManagedServerInstallRequest {
  readonly serversRoot: string;
  readonly version: string;
  readonly platformTarget: string;
  readonly executableName: string;
  readonly archiveExtension: string;
}

export interface ResolvedArchive {
  readonly manifestVersion: string;
  readonly expectedSha256: string;
  readonly bytes: Buffer;
}

export interface ManagedServerInstallOperations {
  resolveArchive(): Promise<ResolvedArchive>;
  extractArchive(archivePath: string, destination: string): Promise<void>;
  probeExecutable(executablePath: string): Promise<string>;
  lockWaitMs?: number;
  lockPollMs?: number;
}

export interface ManagedServerReceiptIdentity {
  readonly assetDigest: string;
  readonly installationState: 'complete';
}

export function validateManagedServerVersion(version: string): string {
  if (!MANAGED_VERSION_PATTERN.test(version)) {
    throw new Error(
      `Invalid ripr server version ${JSON.stringify(version)}. Expected a canonical semantic version such as 1.2.3 or 1.2.3-rc.1.`
    );
  }
  return version;
}

export function combineActiveManagedServerIdentity<T extends { readonly binaryVersion?: string }>(
  active: T,
  installation: ManagedServerInstallation
): T & ManagedServerReceiptIdentity {
  return {
    ...active,
    assetDigest: installation.receipt.archiveSha256,
    installationState: 'complete'
  };
}

export async function installManagedServer(
  request: ManagedServerInstallRequest,
  operations: ManagedServerInstallOperations
): Promise<ManagedServerInstallation> {
  const finalDir = installDir(request);
  await fs.promises.mkdir(path.dirname(finalDir), { recursive: true });
  const deadline = Date.now() + (operations.lockWaitMs ?? LOCK_WAIT_MS);
  const lockPollMs = operations.lockPollMs ?? LOCK_POLL_MS;
  const lockPath = `${finalDir}.install.lock`;
  assertDescendantPath(request.serversRoot, lockPath, 'install lock');

  for (;;) {
    const completed = await readManagedServerInstallation(request);
    if (completed) {
      return completed;
    }

    const lockToken = `${process.pid}:${crypto.randomUUID()}`;
    try {
      await fs.promises.mkdir(lockPath);
      try {
        await fs.promises.writeFile(path.join(lockPath, 'owner'), `${lockToken}\n`, { flag: 'wx' });
      } catch (error) {
        await fs.promises.rm(lockPath, { recursive: true, force: true });
        throw error;
      }
    } catch (error) {
      if (!isAlreadyExists(error)) {
        throw error;
      }
      if (Date.now() >= deadline) {
        throw new Error(`Timed out waiting for managed server install lock ${lockPath}.`);
      }
      await delay(lockPollMs);
      continue;
    }

    const heartbeat = setInterval(() => {
      fs.promises.utimes(lockPath, new Date(), new Date()).catch(() => undefined);
    }, LOCK_HEARTBEAT_MS);
    heartbeat.unref();
    try {
      const converged = await readManagedServerInstallation(request);
      if (converged) {
        return converged;
      }
      return await stageAndPromote(request, operations, finalDir);
    } finally {
      clearInterval(heartbeat);
      await fs.promises.rm(lockPath, { recursive: true, force: true });
    }
  }
}

export async function readManagedServerInstallation(
  request: ManagedServerInstallRequest
): Promise<ManagedServerInstallation | undefined> {
  const finalDir = installDir(request);
  return readInstallationAt(finalDir, request);
}

async function readInstallationAt(
  directory: string,
  request: ManagedServerInstallRequest
): Promise<ManagedServerInstallation | undefined> {
  const receiptPath = path.join(directory, RECEIPT_FILE);
  let parsed: unknown;
  try {
    parsed = JSON.parse(await fs.promises.readFile(receiptPath, 'utf8'));
  } catch {
    return undefined;
  }
  if (!isMatchingReceipt(parsed, request)) {
    return undefined;
  }

  const executablePath = path.join(directory, request.executableName);
  try {
    const stat = await fs.promises.stat(executablePath);
    if (!stat.isFile()) {
      return undefined;
    }
    const executableSha256 = await sha256File(executablePath);
    if (!safeDigestEquals(executableSha256, parsed.executableSha256)) {
      return undefined;
    }
  } catch {
    return undefined;
  }

  return { executablePath, receiptPath, receipt: parsed };
}

function installDir(request: ManagedServerInstallRequest): string {
  const version = validateManagedServerVersion(request.version);
  const root = path.resolve(request.serversRoot);
  const finalDir = path.resolve(root, version, request.platformTarget);
  assertDescendantPath(root, finalDir, 'managed server installation');
  return finalDir;
}

async function stageAndPromote(
  request: ManagedServerInstallRequest,
  operations: ManagedServerInstallOperations,
  finalDir: string
): Promise<ManagedServerInstallation> {
  const unique = `${process.pid}-${crypto.randomUUID()}`;
  const stagingDir = `${finalDir}.install-${unique}`;
  const displacedDir = `${finalDir}.replaced-${unique}`;
  assertDescendantPath(request.serversRoot, stagingDir, 'managed server staging directory');
  assertDescendantPath(request.serversRoot, displacedDir, 'managed server displaced directory');
  let displaced = false;
  await fs.promises.mkdir(stagingDir);

  try {
    const resolved = await operations.resolveArchive();
    if (resolved.manifestVersion !== request.version) {
      throw new Error(
        `Server manifest version ${resolved.manifestVersion} does not match requested version ${request.version}.`
      );
    }
    if (!isSha256(resolved.expectedSha256)) {
      throw new Error('Server manifest asset digest is not a SHA-256 value.');
    }
    const archiveSha256 = sha256Buffer(resolved.bytes);
    if (!safeDigestEquals(archiveSha256, resolved.expectedSha256)) {
      throw new Error(`Server archive checksum mismatch. Expected ${resolved.expectedSha256}, got ${archiveSha256}.`);
    }

    const archivePath = path.join(stagingDir, `ripr-server.${request.archiveExtension}`);
    const extractDir = path.join(stagingDir, 'extract');
    await fs.promises.writeFile(archivePath, resolved.bytes, { flag: 'wx' });
    await fs.promises.mkdir(extractDir);
    await operations.extractArchive(archivePath, extractDir);
    const foundExecutable = await findExecutable(extractDir, request.executableName);
    if (!foundExecutable) {
      throw new Error(`Downloaded archive did not contain ${request.executableName}.`);
    }

    const executablePath = path.join(stagingDir, request.executableName);
    await fs.promises.copyFile(foundExecutable, executablePath, fs.constants.COPYFILE_EXCL);
    if (process.platform !== 'win32') {
      await fs.promises.chmod(executablePath, 0o755);
    }
    const binaryVersion = (await operations.probeExecutable(executablePath)).trim();
    if (binaryVersion.length === 0) {
      throw new Error('Downloaded server executable did not report a version.');
    }
    const executableSha256 = await sha256File(executablePath);
    const receipt: InstallReceiptV1 = {
      schemaVersion: 1,
      installationState: 'complete',
      requestedVersion: request.version,
      manifestVersion: resolved.manifestVersion,
      platformTarget: request.platformTarget,
      executableName: request.executableName,
      archiveSha256,
      executableSha256,
      binaryVersion
    };
    await fs.promises.rm(archivePath, { force: true });
    await fs.promises.rm(extractDir, { recursive: true, force: true });
    await fs.promises.writeFile(path.join(stagingDir, RECEIPT_FILE), `${JSON.stringify(receipt, null, 2)}\n`, {
      flag: 'wx'
    });

    const staged = await readInstallationAt(stagingDir, request);
    if (!staged) {
      throw new Error('Staged managed server failed completed-receipt validation.');
    }

    try {
      await fs.promises.rename(finalDir, displacedDir);
      displaced = true;
    } catch (error) {
      if (!isMissing(error)) {
        throw error;
      }
    }
    try {
      await fs.promises.rename(stagingDir, finalDir);
    } catch (error) {
      if (displaced) {
        await fs.promises.rename(displacedDir, finalDir);
        displaced = false;
      }
      throw error;
    }
    if (displaced) {
      await fs.promises.rm(displacedDir, { recursive: true, force: true });
      displaced = false;
    }

    const promoted = await readManagedServerInstallation(request);
    if (!promoted) {
      throw new Error('Promoted managed server failed completed-receipt validation.');
    }
    return promoted;
  } finally {
    await fs.promises.rm(stagingDir, { recursive: true, force: true });
    if (displaced) {
      try {
        await fs.promises.rename(displacedDir, finalDir);
      } catch {
        // Preserve the original promotion error; a retained displaced directory
        // remains recoverable and is never cache-eligible at the final path.
      }
    }
  }
}

function isMatchingReceipt(value: unknown, request: ManagedServerInstallRequest): value is InstallReceiptV1 {
  if (!value || typeof value !== 'object') {
    return false;
  }
  const receipt = value as Record<string, unknown>;
  return receipt.schemaVersion === 1
    && receipt.installationState === 'complete'
    && receipt.requestedVersion === request.version
    && receipt.manifestVersion === request.version
    && receipt.platformTarget === request.platformTarget
    && receipt.executableName === request.executableName
    && typeof receipt.binaryVersion === 'string'
    && receipt.binaryVersion.trim().length > 0
    && typeof receipt.archiveSha256 === 'string'
    && isSha256(receipt.archiveSha256)
    && typeof receipt.executableSha256 === 'string'
    && isSha256(receipt.executableSha256);
}

function isSha256(value: string): boolean {
  return /^[0-9a-f]{64}$/i.test(value);
}

function safeDigestEquals(left: string, right: string): boolean {
  if (!isSha256(left) || !isSha256(right)) {
    return false;
  }
  return crypto.timingSafeEqual(Buffer.from(left.toLowerCase(), 'hex'), Buffer.from(right.toLowerCase(), 'hex'));
}

function sha256Buffer(data: Buffer): string {
  return crypto.createHash('sha256').update(data).digest('hex');
}

async function sha256File(filePath: string): Promise<string> {
  return new Promise((resolve, reject) => {
    const hash = crypto.createHash('sha256');
    const input = fs.createReadStream(filePath);
    input.on('error', reject);
    input.on('data', (chunk) => hash.update(chunk));
    input.on('end', () => resolve(hash.digest('hex')));
  });
}

async function findExecutable(root: string, executableName: string): Promise<string | undefined> {
  const entries = await fs.promises.readdir(root, { withFileTypes: true });
  for (const entry of entries) {
    const fullPath = path.join(root, entry.name);
    if (entry.isFile() && entry.name === executableName) {
      return fullPath;
    }
    if (entry.isDirectory()) {
      const found = await findExecutable(fullPath, executableName);
      if (found) {
        return found;
      }
    }
  }
  return undefined;
}

function assertDescendantPath(root: string, candidate: string, description: string): void {
  const canonicalRoot = path.resolve(root);
  const canonicalCandidate = path.resolve(candidate);
  const relative = path.relative(canonicalRoot, canonicalCandidate);
  if (relative.length === 0 || relative.startsWith(`..${path.sep}`) || relative === '..' || path.isAbsolute(relative)) {
    throw new Error(`${description} escapes the managed server cache root.`);
  }
}

function isAlreadyExists(error: unknown): boolean {
  return isNodeError(error) && error.code === 'EEXIST';
}

function isMissing(error: unknown): boolean {
  return isNodeError(error) && error.code === 'ENOENT';
}

function isNodeError(error: unknown): error is NodeJS.ErrnoException {
  return error instanceof Error && 'code' in error;
}

function delay(milliseconds: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}
