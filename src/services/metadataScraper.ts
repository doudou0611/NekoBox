import type { MetadataCandidate } from '../types/domain';

export const metadataProviders = ['bangumi', 'hikarinagi', 'vndb'] as const;
export type MetadataProvider = (typeof metadataProviders)[number];

export function automaticCandidate(candidates: MetadataCandidate[]) {
  const sorted = [...candidates].sort(
    (left, right) =>
      right.confidence - left.confidence ||
      left.remote_id.localeCompare(right.remote_id),
  );
  const best = sorted[0];
  if (
    !best ||
    best.confidence < 0.95 ||
    (sorted[1] && best.confidence - sorted[1].confidence < 0.05)
  )
    return null;
  return best;
}
