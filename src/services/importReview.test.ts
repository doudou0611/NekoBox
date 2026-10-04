import { describe, expect, it } from 'vitest';
import {
  isPrepared,
  needsScrape,
  prepareReview,
  selectReviewMatch,
  type ScrapeReview,
} from './importReview';
import type { ImportPreparation } from '../types/importPreparation';

const preparation: ImportPreparation = {
  preparation_id: 'ready',
  title: '中文作品',
  subtitle: null,
  cover_path: 'covers/local.jpg',
  provider: 'bangumi',
  remote_id: '123',
  translation_message: null,
};
function item(): ScrapeReview {
  return {
    install_path: 'C:/Games/Game',
    selected: true,
    preview: { existing_game_id: null },
    scrape_status: 'pending',
    scrape_message: '',
    match: {
      provider: 'bangumi',
      remote_id: '123',
      title: '中文作品',
      confidence: 1,
      matched_fields: [],
      explanation: '',
      fetched_at: '',
      cached: false,
    },
    preparation: null,
    manual_match: false,
  };
}
describe('import preparation and retry boundaries', () => {
  it('manual selection only records the candidate until scraping is requested', async () => {
    const row = item();
    row.preparation = preparation;
    selectReviewMatch(row, row.match!);
    expect(row.scrape_status).toBe('matched');
    expect(row.preparation).toBeNull();
    expect(row.manual_match).toBe(true);
    expect(isPrepared(row)).toBe(false);
    expect(needsScrape(row)).toBe(true);
    await prepareReview(row, async () => preparation);
    expect(row.scrape_status).toBe('manual');
    expect(isPrepared(row)).toBe(true);
  });
  it('nine ready games are skipped and only the failed tenth is eligible', async () => {
    const items = Array.from({ length: 10 }, item);
    for (const row of items.slice(0, 9))
      await prepareReview(row, async () => preparation);
    items[9]!.scrape_status = 'failed';
    expect(items.filter(needsScrape)).toEqual([items[9]]);
    items[9]!.scrape_status = 'pending'; // user corrects the failed query
    expect(items.filter(needsScrape)).toHaveLength(1);
    await prepareReview(items[9]!, async () => preparation);
    expect(items.filter(needsScrape)).toHaveLength(0);
  });
  it('a candidate alone is not ready and a failed cover remains retryable', async () => {
    const row = item();
    row.manual_match = true;
    let resolve!: (value: ImportPreparation) => void;
    const pending = prepareReview(
      row,
      () =>
        new Promise((callback) => {
          resolve = callback;
        }),
    );
    expect(row.scrape_status).toBe('scraping');
    expect(isPrepared(row)).toBe(false);
    resolve(preparation);
    await pending;
    expect(row.scrape_status).toBe('manual');
    expect(needsScrape(row)).toBe(false);
    await expect(
      prepareReview(row, async () => {
        throw new Error('cover failed');
      }),
    ).rejects.toThrow('cover failed');
    expect(isPrepared(row)).toBe(false);
    expect(row.match?.remote_id).toBe('123');
    expect(row.manual_match).toBe(true);
    expect(needsScrape(row)).toBe(true);
  });
  it('unchecked and duplicate rows are excluded', () => {
    const row = item();
    row.selected = false;
    expect(needsScrape(row)).toBe(false);
    row.selected = true;
    row.preview!.existing_game_id = 'existing';
    expect(needsScrape(row)).toBe(false);
  });
});
