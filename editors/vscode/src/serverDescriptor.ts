import * as fs from 'fs';
import * as path from 'path';
import { parseDistributionDescriptor } from './distributionDescriptor';
import { distributionGeneration } from './managedServerInstall';

/** Compatibility view of the producer-owned distribution.json catalog. */
export interface ServerDistributionDescriptor {
  readonly generation: string;
  readonly manifestSha256: string;
}

/**
 * This view never independently creates or admits a digest. The public catalog
 * parser owns schema, producer, version, placements, and immutable identity.
 * A development catalog supplies no release row and cannot authorize fallback.
 */
export function serverDistributionDescriptorsFromCatalog(serialized: string): readonly ServerDistributionDescriptor[] {
  const catalog = parseDistributionDescriptor(serialized);
  if (catalog.channel === 'development' || catalog.manifestSha256 === undefined) {
    return [];
  }
  return [{ generation: distributionGeneration(catalog.productVersion), manifestSha256: catalog.manifestSha256 }];
}

export const SERVER_DISTRIBUTION_DESCRIPTORS: readonly ServerDistributionDescriptor[] =
  serverDistributionDescriptorsFromCatalog(
    fs.readFileSync(path.join(__dirname, '..', '..', 'distribution.json'), 'utf8')
  );
