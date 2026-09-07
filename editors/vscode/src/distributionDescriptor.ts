import * as crypto from 'crypto';

/** Describes an allowed public or development placement for one server generation. */
export type DistributionChannel = 'development' | 'rc' | 'stable';

export interface DistributionPlacement {
  readonly channel: DistributionChannel;
  readonly releaseTag: string;
  readonly releaseRef: string;
}

/** Describes the installed extension's server generation and allowed placements. */
export interface DistributionDescriptor {
  readonly schema: 1;
  readonly productVersion: string;
  /** Preferred placement. The final 0.11 catalog prefers stable. */
  readonly channel: DistributionChannel;
  readonly releaseTag: string;
  readonly releaseRef: string;
  /** Ordered bounded fallbacks for the same immutable server generation. */
  readonly fallbackPlacements?: readonly DistributionPlacement[];
  readonly manifestFile: string;
  readonly sourceRepository: string;
}

export type DistributionRequestOrigin = 'embedded_descriptor' | 'explicit_legacy_override' | 'development_fixture';

export interface ResolvedDistributionRequest {
  readonly productVersion: string;
  readonly manifestFile: string;
  readonly sourceRepository: string;
  readonly preferredPlacement: DistributionPlacement;
  readonly fallbackPlacements: readonly DistributionPlacement[];
  readonly origin: DistributionRequestOrigin;
  /** Placement-neutral identity for the immutable server generation. */
  readonly descriptorIdentity: string;
  /** Identity of the complete installed catalog, including allowed placements. */
  readonly catalogIdentity: string;
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
    'fallbackPlacements',
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

  const preferredPlacement = { channel, releaseTag, releaseRef };
  validatePlacement(productVersion, preferredPlacement, 'preferred');
  const fallbackPlacements = parseFallbackPlacements(value.fallbackPlacements, productVersion);
  if (fallbackPlacements.length > 0 && channel !== 'stable') {
    throw new Error('release descriptor fallbacks require stable as the preferred placement');
  }
  const seenTags = new Set([releaseTag]);
  for (const fallback of fallbackPlacements) {
    if (fallback.channel !== 'rc') {
      throw new Error('release descriptor fallback placement must use the RC channel');
    }
    if (seenTags.has(fallback.releaseTag)) {
      throw new Error(`release descriptor duplicates placement ${fallback.releaseTag}`);
    }
    seenTags.add(fallback.releaseTag);
  }

  try {
    const repository = new URL(sourceRepository);
    if (repository.protocol !== 'https:') {
      throw new Error('source repository must use HTTPS');
    }
  } catch (error) {
    throw new Error(`invalid source repository: ${error instanceof Error ? error.message : String(error)}`);
  }

  return {
    schema,
    productVersion,
    channel,
    releaseTag,
    releaseRef,
    fallbackPlacements,
    manifestFile,
    sourceRepository
  };
}

/** Binds a descriptor to the package version used by the installed extension. */
export function resolveDistributionRequest(
  packageVersion: string,
  descriptor: DistributionDescriptor,
  origin: DistributionRequestOrigin = 'embedded_descriptor'
): ResolvedDistributionRequest {
  if (packageVersion !== descriptor.productVersion) {
    throw new Error(`product version mismatch: package ${packageVersion}, descriptor ${descriptor.productVersion}`);
  }
  const validated = parseDistributionDescriptor(JSON.stringify(descriptor));
  return {
    productVersion: validated.productVersion,
    manifestFile: validated.manifestFile,
    sourceRepository: validated.sourceRepository,
    preferredPlacement: placementFromDescriptor(validated),
    fallbackPlacements: validated.fallbackPlacements ?? [],
    origin,
    descriptorIdentity: distributionDescriptorIdentity(validated),
    catalogIdentity: distributionCatalogIdentity(validated)
  };
}

/** Returns a placement-neutral identity for the immutable server generation. */
export function distributionDescriptorIdentity(descriptor: DistributionDescriptor): string {
  const canonical = JSON.stringify([
    descriptor.schema,
    descriptor.productVersion,
    descriptor.manifestFile,
    descriptor.sourceRepository
  ]);
  return sha256Identity(canonical);
}

/** Returns the identity of the complete installed placement catalog. */
export function distributionCatalogIdentity(descriptor: DistributionDescriptor): string {
  const canonical = JSON.stringify([
    descriptor.schema,
    descriptor.productVersion,
    descriptor.manifestFile,
    descriptor.sourceRepository,
    placementFromDescriptor(descriptor),
    descriptor.fallbackPlacements ?? []
  ]);
  return sha256Identity(canonical);
}

/** Builds a mirror or source-repository URL without changing server-generation identity. */
export function distributionManifestUrl(
  baseUrl: string,
  distribution: ResolvedDistributionRequest,
  placement: DistributionPlacement = distribution.preferredPlacement
): string {
  const file = distribution.manifestFile;
  const base = baseUrl.trim();
  if (base.length > 0) {
    return `${base.replace(/\/+$/, '')}/${file}`;
  }
  if (placement.channel === 'development') {
    throw new Error('development distribution has no public release placement');
  }
  return `${distribution.sourceRepository.replace(/\/+$/, '')}/releases/download/${placement.releaseTag}/${file}`;
}

/** Returns the preferred placement followed by its bounded fallbacks. */
export function distributionPlacements(distribution: ResolvedDistributionRequest): readonly DistributionPlacement[] {
  return [distribution.preferredPlacement, ...distribution.fallbackPlacements];
}

function parseFallbackPlacements(value: unknown, productVersion: string): readonly DistributionPlacement[] {
  if (value === undefined) {
    return [];
  }
  if (!Array.isArray(value)) {
    throw new Error('release descriptor fallbackPlacements must be an array');
  }
  if (value.length > 1) {
    throw new Error('release descriptor supports at most one bounded fallback placement');
  }
  return value.map((entry, index) => {
    if (!isRecord(entry)) {
      throw new Error(`release descriptor fallback placement ${index} must be an object`);
    }
    const allowed = new Set(['channel', 'releaseTag', 'releaseRef']);
    const unknown = Object.keys(entry).find((key) => !allowed.has(key));
    if (unknown) {
      throw new Error(`unsupported fallback placement field: ${unknown}`);
    }
    const channel = entry.channel;
    const releaseTag = entry.releaseTag;
    const releaseRef = entry.releaseRef;
    if (typeof channel !== 'string' || typeof releaseTag !== 'string' || typeof releaseRef !== 'string' || !isChannel(channel)) {
      throw new Error(`release descriptor fallback placement ${index} is malformed`);
    }
    const placement = { channel, releaseTag, releaseRef };
    validatePlacement(productVersion, placement, `fallback ${index}`);
    return placement;
  });
}

function validatePlacement(productVersion: string, placement: DistributionPlacement, role: string): void {
  if (placement.releaseTag.length === 0) {
    throw new Error(`release descriptor ${role} release tag must not be empty`);
  }
  if (placement.releaseRef !== `refs/tags/${placement.releaseTag}`) {
    throw new Error(`release descriptor ${role} release ref must match release tag`);
  }
  if (placement.channel === 'stable' && placement.releaseTag !== `v${productVersion}`) {
    throw new Error('stable channel requires a stable release tag');
  }
  if (placement.channel === 'rc' && !new RegExp(`^v${escapeRegExp(productVersion)}-rc\\.\\d+$`).test(placement.releaseTag)) {
    throw new Error('RC channel requires an RC release tag');
  }
}

function placementFromDescriptor(descriptor: DistributionDescriptor): DistributionPlacement {
  return {
    channel: descriptor.channel,
    releaseTag: descriptor.releaseTag,
    releaseRef: descriptor.releaseRef
  };
}

function sha256Identity(value: string): string {
  return `sha256:${crypto.createHash('sha256').update(value, 'utf8').digest('hex')}`;
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
