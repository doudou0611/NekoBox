import { describe, expect, it } from 'vitest';
import {
  isPrepared,
  isImported,
  importReviewStats,
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
  it('reimporting a folder only counts new games, independently of selection or old scrape state', () => {
    const existing = item();
    existing.preview!.existing_game_id = 'existing';
    existing.preparation = preparation;
    existing.scrape_status = 'scraped';
    const ready = item();
    ready.preparation = preparation;
    ready.scrape_status = 'scraped';
    const pending = item();
    const unchecked = item();
    unchecked.selected = false;
    expect(isImported(existing)).toBe(true);
    expect(importReviewStats([existing, ready, pending, unchecked])).toEqual({
      recognized: 3,
      imported: 1,
      scraped: 1,
      pending: 2,
      selected: 2,
    });
    expect([existing, ready, pending, unchecked].filter(needsScrape)).toEqual([
      pending,
    ]);
  });
  it('all existing paths produce zero new work, and successful partial imports leave only failures to retry', () => {
    const first = item(),
      second = item();
    first.preview!.existing_game_id = 'existing';
    expect(importReviewStats([first])).toEqual({
      recognized: 0,
      imported: 1,
      scraped: 0,
      pending: 0,
      selected: 0,
    });
    second.scrape_status = 'failed';
    expect(importReviewStats([first, second])).toEqual({
      recognized: 1,
      imported: 1,
      scraped: 0,
      pending: 1,
      selected: 1,
    });
    second.preview!.existing_game_id = 'newly-imported';
    second.selected = false;
    expect(importReviewStats([first, second]).recognized).toBe(0);
    expect([first, second].filter(needsScrape)).toHaveLength(0);
  });
});
