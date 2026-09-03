/** Describes the installed extension's exact server distribution placement. */
export type DistributionChannel = 'development' | 'rc' | 'stable';

export interface DistributionDescriptor {
  readonly schema: 1;
  readonly productVersion: string;
  readonly channel: DistributionChannel;
  readonly releaseTag: string;
  readonly releaseRef: string;
  readonly manifestFile: string;
  readonly sourceRepository: string;
}

export interface ResolvedDistributionRequest {
  readonly productVersion: string;
  readonly releaseTag: string;
  readonly releaseRef: string;
  readonly manifestFile: string;
  readonly sourceRepository: string;
  readonly channel: DistributionChannel;
  readonly descriptorIdentity: string;
}

/** Parses and validates one embedded distribution descriptor. */
export function parseDistributionDescriptor(serialized: string): DistributionDescriptor {
  let value: unknown;
  try {
    value = JSON.parse(serialized) as unknown;
  } catch (error) {
    throw new Error(`malformed release descriptor: ${error instanceof Error ? error.message : String(error)}`);
  }
  if (!isRecord(value)) {
    throw new Error('malformed release descriptor: expected an object');
  }

  const allowedFields = new Set([
    'schema',
    'productVersion',
    'channel',
    'releaseTag',
    'releaseRef',
    'manifestFile',
    'sourceRepository'
  ]);
  const unknownField = Object.keys(value).find((key) => !allowedFields.has(key));
  if (unknownField) {
    throw new Error(`unsupported release descriptor field: ${unknownField}`);
  }

  const schema = value.schema;
  const productVersion = value.productVersion;
  const channel = value.channel;
  const releaseTag = value.releaseTag;
  const releaseRef = value.releaseRef;
  const manifestFile = value.manifestFile;
  const sourceRepository = value.sourceRepository;
  if (
    schema !== 1 ||
    typeof productVersion !== 'string' ||
    typeof channel !== 'string' ||
    typeof releaseTag !== 'string' ||
    typeof releaseRef !== 'string' ||
    typeof manifestFile !== 'string' ||
    typeof sourceRepository !== 'string'
  ) {
    throw new Error('missing release descriptor field or unsupported schema');
  }
  if (!isChannel(channel)) {
    throw new Error(`unsupported release descriptor channel: ${channel}`);
  }
  if (!/^\d+\.\d+\.\d+$/.test(productVersion)) {
    throw new Error('release descriptor product version is not semantic');
  }
  if (manifestFile !== `ripr-server-manifest-v${productVersion}.json`) {
    throw new Error('release descriptor manifest filename does not match product version');
  }
  if (channel === 'stable' && releaseTag !== `v${productVersion}`) {
    throw new Error('stable channel requires a stable release tag');
  }
  if (channel === 'rc' && !new RegExp(`^v${escapeRegExp(productVersion)}-rc\\.\\d+$`).test(releaseTag)) {
    throw new Error('RC channel requires an RC release tag');
  }
  if (releaseRef !== `refs/tags/${releaseTag}`) {
    throw new Error('release ref must match release tag');
  }
  try {
    const repository = new URL(sourceRepository);
    if (repository.protocol !== 'https:') {
      throw new Error('source repository must use HTTPS');
    }
  } catch (error) {
    throw new Error(`invalid source repository: ${error instanceof Error ? error.message : String(error)}`);
  }
  return { schema, productVersion, channel, releaseTag, releaseRef, manifestFile, sourceRepository };
}

/** Binds a descriptor to the package version used by the installed extension. */
export function resolveDistributionRequest(
  packageVersion: string,
  descriptor: DistributionDescriptor
): ResolvedDistributionRequest {
  if (packageVersion !== descriptor.productVersion) {
    throw new Error(`product version mismatch: package ${packageVersion}, descriptor ${descriptor.productVersion}`);
  }
  const validated = parseDistributionDescriptor(JSON.stringify(descriptor));
  return {
    productVersion: validated.productVersion,
    releaseTag: validated.releaseTag,
    releaseRef: validated.releaseRef,
    manifestFile: validated.manifestFile,
    sourceRepository: validated.sourceRepository,
    channel: validated.channel,
    descriptorIdentity: distributionDescriptorIdentity(validated)
  };
}

/** Returns the stable semantic identity used to partition managed-server caches. */
export function distributionDescriptorIdentity(descriptor: DistributionDescriptor): string {
  const canonical = JSON.stringify([
    descriptor.schema,
    descriptor.productVersion,
    descriptor.channel,
    descriptor.releaseTag,
    descriptor.releaseRef,
    descriptor.manifestFile,
    descriptor.sourceRepository
  ]);
  return `sha256:${crypto.createHash('sha256').update(canonical, 'utf8').digest('hex')}`;
}

/** Builds a mirror or source-repository URL without changing distribution identity. */
export function distributionManifestUrl(baseUrl: string, distribution: ResolvedDistributionRequest): string {
  const file = distribution.manifestFile;
  const base = baseUrl.trim();
  if (base.length > 0) {
    return `${base.replace(/\/+$/, '')}/${file}`;
  }
  return `${distribution.sourceRepository}/releases/download/${distribution.releaseTag}/${file}`;
}

/** Narrows parsed JSON to a non-null object. */
function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/** Narrows descriptor channel values to the supported set. */
function isChannel(value: string): value is DistributionChannel {
  return value === 'development' || value === 'rc' || value === 'stable';
}

/** Escapes a semantic version before using it in a validation expression. */
function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}
import * as crypto from 'crypto';
