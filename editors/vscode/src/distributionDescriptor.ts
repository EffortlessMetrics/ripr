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
  readonly schema: 1 | 2;
  readonly productVersion: string;
  /** Preferred placement. The final 0.11 catalog prefers stable. */
  readonly channel: DistributionChannel;
  readonly releaseTag: string;
  readonly releaseRef: string;
  /** Ordered bounded fallbacks for the same immutable server generation. */
  readonly fallbackPlacements?: readonly DistributionPlacement[];
  readonly manifestFile: string;
  readonly sourceRepository: string;
  /**
   * Schema 2 release identity, emitted by the release catalog producer and
   * required for rc/stable catalogs. Development catalogs must not carry
   * release identity; schema 1 catalogs predate it.
   */
  readonly distributionGeneration?: string;
  readonly manifestSha256?: string;
  readonly targetSetDigest?: string;
}

export type DistributionRequestOrigin = 'embedded_descriptor' | 'explicit_legacy_override' | 'development_fixture';

export interface ResolvedDistributionRequest {
  readonly productVersion: string;
  readonly manifestFile: string;
  readonly sourceRepository: string;
  readonly distributionGeneration?: string;
  readonly manifestSha256?: string;
  readonly targetSetDigest?: string;
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
    'sourceRepository',
    'distributionGeneration',
    'manifestSha256',
    'targetSetDigest'
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
    (schema !== 1 && schema !== 2) ||
    typeof productVersion !== 'string' ||
    typeof channel !== 'string' ||
    typeof releaseTag !== 'string' ||
    typeof releaseRef !== 'string' ||
    typeof manifestFile !== 'string' ||
    typeof sourceRepository !== 'string'
  ) {
    throw new Error('missing release descriptor field or unsupported schema');
  }
  validateReleaseIdentity(value.schema, value.channel, {
    distributionGeneration: value.distributionGeneration,
    manifestSha256: value.manifestSha256,
    targetSetDigest: value.targetSetDigest
  });
  if (!isChannel(channel)) {
    throw new Error(`unsupported release descriptor channel: ${channel}`);
  }
  if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(productVersion)) {
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

  const releaseIdentity =
    schema === 2
      ? {
          distributionGeneration: value.distributionGeneration as string,
          manifestSha256: value.manifestSha256 as string,
          targetSetDigest: value.targetSetDigest as string
        }
      : {};
  return {
    schema,
    productVersion,
    channel,
    releaseTag,
    releaseRef,
    fallbackPlacements,
    manifestFile,
    sourceRepository,
    ...releaseIdentity
  };
}

/**
 * Binds a descriptor to the package version used by the installed extension.
 * An embedded development catalog is never eligible for managed resolution:
 * it names no public placement, so accepting it would permit managed cache
 * reuse for a generation the download path must reject. The
 * `development_fixture` origin keeps development descriptors available to
 * unit harnesses.
 */
export function resolveDistributionRequest(
  packageVersion: string,
  descriptor: DistributionDescriptor,
  origin: DistributionRequestOrigin = 'embedded_descriptor'
): ResolvedDistributionRequest {
  if (origin === 'embedded_descriptor' && descriptor.channel === 'development') {
    throw new Error('embedded development distribution is not eligible for managed resolution');
  }
  if (packageVersion !== descriptor.productVersion) {
    throw new Error(`product version mismatch: package ${packageVersion}, descriptor ${descriptor.productVersion}`);
  }
  const validated = parseDistributionDescriptor(JSON.stringify(descriptor));
  return {
    productVersion: validated.productVersion,
    manifestFile: validated.manifestFile,
    sourceRepository: validated.sourceRepository,
    distributionGeneration: validated.distributionGeneration,
    manifestSha256: validated.manifestSha256,
    targetSetDigest: validated.targetSetDigest,
    preferredPlacement: placementFromDescriptor(validated),
    fallbackPlacements: validated.fallbackPlacements ?? [],
    origin,
    descriptorIdentity: distributionDescriptorIdentity(validated),
    catalogIdentity: distributionCatalogIdentity(validated)
  };
}

/** Returns a placement-neutral identity for the immutable server generation. */
export function distributionDescriptorIdentity(descriptor: DistributionDescriptor): string {
  const releaseIdentity =
    descriptor.schema === 2
      ? [
          descriptor.distributionGeneration,
          descriptor.manifestSha256,
          descriptor.targetSetDigest
        ]
      : [];
  const canonical = JSON.stringify([
    descriptor.schema,
    descriptor.productVersion,
    descriptor.manifestFile,
    descriptor.sourceRepository,
    ...releaseIdentity
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
  if (placement.channel === 'development') {
    throw new Error('development distribution has no public release placement');
  }
  if (base.length > 0) {
    return `${base.replace(/\/+$/, '')}/${file}`;
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

/**
 * Enforces schema-versioned release identity: schema 1 predates it, schema 2
 * development catalogs must not carry it, and schema 2 rc/stable catalogs
 * must bind all three digests. The digests mirror the release manifest v2
 * producer; the downloader admits them against fetched bytes downstream.
 */
function validateReleaseIdentity(
  schema: unknown,
  channel: unknown,
  identity: {
    distributionGeneration: unknown;
    manifestSha256: unknown;
    targetSetDigest: unknown;
  }
): void {
  const fields = [
    ['distributionGeneration', identity.distributionGeneration],
    ['manifestSha256', identity.manifestSha256],
    ['targetSetDigest', identity.targetSetDigest]
  ] as const;
  const carried = fields.find(([, field]) => field !== undefined);
  if (schema === 1) {
    if (carried) {
      throw new Error(`release descriptor field ${carried[0]} requires schema 2`);
    }
    return;
  }
  if (channel === 'development') {
    if (carried) {
      throw new Error(`development catalog must not carry release identity field ${carried[0]}`);
    }
    return;
  }
  for (const [name, field] of fields) {
    if (typeof field !== 'string' || !/^[0-9a-f]{64}$/.test(field)) {
      throw new Error(`release descriptor field ${name} must be a 64-character lowercase hex digest`);
    }
  }
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
  if (placement.channel === 'rc' && !new RegExp(`^v${escapeRegExp(productVersion)}-rc\\.(0|[1-9]\\d*)$`).test(placement.releaseTag)) {
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
