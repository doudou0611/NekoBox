import type { GameDetail } from '../types/domain';
import type { MetadataChanges, MetadataValues } from '../types/detailWorkspace';
export const metadataKeys = [
  'title',
  'cover_path',
  'developer',
  'source_rating',
  'release_date',
  'description',
] as const;
export function metadataValues(
  game: Partial<GameDetail> | undefined,
): MetadataValues {
  return {
    title: game?.title ?? '',
    cover_path: game?.cover_url ?? null,
    developer: game?.developer ?? null,
    source_rating: game?.source_rating ?? null,
    release_date: game?.release_date ?? null,
    description: game?.description ?? null,
  };
}
export function changedMetadata(
  draft: MetadataValues,
  baseline: MetadataValues,
) {
  const changes: MetadataChanges = {},
    expected: MetadataChanges = {};
  for (const field of metadataKeys) {
    const raw = draft[field];
    const value =
      typeof raw === 'string'
        ? raw.trim() || (field === 'title' ? '' : null)
        : raw;
    if (value !== baseline[field]) {
      Object.assign(changes, { [field]: value });
      Object.assign(expected, { [field]: baseline[field] });
    }
  }
  return { changes, expected };
}
// Fill fresh scraped values while retaining the old revision of edited fields.
export function mergeMetadataDraft(
  draft: MetadataValues,
  baseline: MetadataValues,
  next: MetadataValues,
) {
  for (const field of metadataKeys) {
    if (draft[field] === baseline[field]) {
      Object.assign(draft, { [field]: next[field] });
      Object.assign(baseline, { [field]: next[field] });
    }
  }
}
export function launchEnvironment(rows: { key: string; value: string }[]) {
  const env: Record<string, string> = {};
  for (const row of rows) {
    const key = row.key.trim();
    if (!key && !row.value) continue;
    if (!key || key.includes('=') || key.includes('\0'))
      throw new Error('环境变量需要有效名称，不能包含等号。');
    if (Object.hasOwn(env, key))
      throw new Error(`环境变量 ${key} 重复，请合并后保存。`);
    Object.defineProperty(env, key, { enumerable: true, value: row.value });
  }
  return env;
}
