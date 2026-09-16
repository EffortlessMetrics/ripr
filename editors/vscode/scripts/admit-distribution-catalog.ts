/**
 * Release catalog admission (#1682): the single Node admission authority
 * shared by `cargo xtask vscode-package`, every vsce publication path, and
 * the extension unit suite. It validates one staged candidate catalog with
 * the exact runtime parser semantics (parseDistributionDescriptor plus
 * resolveDistributionRequest) and emits an admission receipt on stdout.
 *
 * Usage:
 *   node out/scripts/admit-distribution-catalog.js \
 *     --catalog <path> --package-version <version> [--allow-dev]
 *
 * Exit 0 admits; exit 2 rejects with a bounded message on stderr. Release
 * catalogs must be schema 2 rc/stable producer output; development catalogs
 * are admitted only with --allow-dev for local development packaging.
 */
import * as crypto from 'crypto';
import * as fs from 'fs';
import {
  parseDistributionDescriptor,
  resolveDistributionRequest
} from '../src/distributionDescriptor';

const MAX_CATALOG_BYTES = 1_048_576;

function flagValue(name: string): string | undefined {
  const args = process.argv.slice(2);
  for (let index = 0; index < args.length; index += 1) {
    if (args[index] === `--${name}` && index + 1 < args.length) {
      return args[index + 1];
    }
    const inline = `--${name}=`;
    if (args[index].startsWith(inline)) {
      return args[index].slice(inline.length);
    }
  }
  return undefined;
}

function fail(message: string): never {
  process.stderr.write(`distribution catalog not admitted: ${message}\n`);
  process.exit(2);
}

function main(): void {
  const catalogPath = flagValue('catalog') ?? fail('missing --catalog <path>');
  const packageVersion =
    flagValue('package-version') ?? fail('missing --package-version <version>');
  const allowDev = process.argv.includes('--allow-dev');

  let serialized: string;
  try {
    const stat = fs.statSync(catalogPath);
    if (!stat.isFile()) {
      fail(`catalog path is not a regular file: ${catalogPath}`);
    }
    if (stat.size > MAX_CATALOG_BYTES) {
      fail(`catalog exceeds the ${MAX_CATALOG_BYTES}-byte bound`);
    }
    serialized = fs.readFileSync(catalogPath, 'utf8');
  } catch (error) {
    fail(`catalog file is unreadable: ${(error as Error).message}`);
  }

  try {
    const descriptor = parseDistributionDescriptor(serialized);
    if (!allowDev && descriptor.schema !== 2) {
      fail('release packaging requires a schema 2 producer catalog with bound release identity');
    }
    if (!allowDev && descriptor.channel === 'development') {
      fail('development catalogs are not release candidates; stage a producer catalog');
    }
    const request = resolveDistributionRequest(
      packageVersion,
      descriptor,
      allowDev && descriptor.channel === 'development'
        ? 'development_fixture'
        : 'embedded_descriptor'
    );
    const receipt = {
      admitted: true,
      schema: descriptor.schema,
      productVersion: request.productVersion,
      channel: request.preferredPlacement.channel,
      releaseTag: request.preferredPlacement.releaseTag,
      manifestFile: request.manifestFile,
      sourceRepository: request.sourceRepository,
      distributionGeneration: request.distributionGeneration ?? null,
      manifestSha256: request.manifestSha256 ?? null,
      targetSetDigest: request.targetSetDigest ?? null,
      catalogSha256: crypto.createHash('sha256').update(serialized, 'utf8').digest('hex'),
      descriptorIdentity: request.descriptorIdentity,
      catalogIdentity: request.catalogIdentity
    };
    process.stdout.write(`${JSON.stringify(receipt, null, 2)}\n`);
  } catch (error) {
    fail((error as Error).message);
  }
}

main();
