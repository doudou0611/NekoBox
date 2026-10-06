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

/** Store names are authoritative: a unique exact title/alias wins over noisy scores. */
export function automaticOwnedCandidate(
  query: string,
  candidates: MetadataCandidate[],
) {
  const normalize = (value: string) =>
    value
      .normalize('NFKC')
      .toLowerCase()
      .replace(/[^\p{L}\p{N}]/gu, '');
  const key = normalize(query);
  if (!key) return null;
  const unique = [
    ...new Map(
      candidates.map((c) => [`${c.provider}:${c.remote_id}`, c]),
    ).values(),
  ];
  const exact = unique.filter((c) =>
    [c.title, c.subtitle ?? ''].some((t) => normalize(t) === key),
  );
  if (exact.length === 1) return exact[0];
  if (exact.length > 1) return null;
  return automaticCandidate(unique);
}
