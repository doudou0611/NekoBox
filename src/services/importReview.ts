import type { MetadataCandidate } from '../types/domain';
import type { ImportPreparation } from '../types/importPreparation';

export interface ScrapeReview {
  install_path: string;
  selected: boolean;
  preview: { existing_game_id: string | null } | null;
  scrape_status:
    | 'pending'
    | 'matched'
    | 'scraping'
    | 'scraped'
    | 'manual'
    | 'no_match'
    | 'failed';
  scrape_message: string;
  match: MetadataCandidate | null;
  preparation: ImportPreparation | null;
  manual_match: boolean;
}
export function isPrepared(item: ScrapeReview) {
  return (
    !!item.preparation && ['scraped', 'manual'].includes(item.scrape_status)
  );
}
export function isImported(item: Pick<ScrapeReview, 'preview'>) {
  return !!item.preview?.existing_game_id;
}
/** Previously imported paths are informational rows, never new scrape/import work. */
export function importReviewStats(items: readonly ScrapeReview[]) {
  const imported = items.filter(isImported).length;
  const available = items.filter((item) => !isImported(item));
  const scraped = available.filter(isPrepared).length;
  return {
    recognized: available.length,
    imported,
    scraped,
    pending: available.length - scraped,
    selected: available.filter((item) => item.selected).length,
  };
}
export function needsScrape(item: ScrapeReview) {
  return item.selected && !isImported(item) && !isPrepared(item);
}
/** Selecting a candidate never fetches metadata or marks the row as ready. */
export function selectReviewMatch(
  item: ScrapeReview,
  match: MetadataCandidate,
) {
  item.match = match;
  item.preparation = null;
  item.manual_match = true;
  item.scrape_status = 'matched';
  item.scrape_message = `已选择 ${match.provider.toUpperCase()} · ${match.title}，点击“开始刮削”获取资料和封面。`;
}
/** Success means the backend has fetched full metadata AND verified the cached cover. */
export async function prepareReview(
  item: ScrapeReview,
  prepare: (match: MetadataCandidate) => Promise<ImportPreparation>,
) {
  if (!item.match) throw new Error('请先匹配作品资料。');
  item.scrape_status = 'scraping';
  item.scrape_message = '正在获取完整资料并下载封面';
  item.preparation = null;
  try {
    const preparation = await prepare(item.match);
    item.preparation = preparation;
    item.scrape_status = item.manual_match ? 'manual' : 'scraped';
    item.scrape_message = `${preparation.provider.toUpperCase()} · ${preparation.title} · 资料和本地封面已准备完成`;
  } catch (cause) {
    item.scrape_status = 'failed';
    throw cause;
  }
}
