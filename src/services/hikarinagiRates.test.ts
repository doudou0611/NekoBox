import { describe, expect, it } from 'vitest';
import {
  createRatesLoader,
  hasHikarinagiRates,
  metadataRatesFallback,
  type RatesWallState,
} from './hikarinagiRates';
import type { SourcedField } from '../types/domain';
import type { HikarinagiRatesWall } from '../types/hikarinagi';
const snapshot = (remote_id: string): HikarinagiRatesWall => ({
  remote_id,
  source_url: `https://www.hikarinagi.org/galgames/${remote_id}/rates`,
  average: 8.2,
  rated_count: 9,
  distribution: [],
  status_counts: { completed: 18, going: 1, on_hold: 1, dropped: 2 },
  keywords: [],
  fetched_at: '2026-10-03T00:00:00Z',
  cached: false,
  stale: false,
  message: null,
});
const initial = (): RatesWallState => ({
  game_id: '',
  wall: null,
  loading: false,
  error: '',
});
const deferred = () => {
  let resolve!: (value: { wall: HikarinagiRatesWall | null }) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<{ wall: HikarinagiRatesWall | null }>((a, b) => {
    resolve = a;
    reject = b;
  });
  return { promise, resolve, reject };
};
const field = (
  provider: string,
  name: string,
  value: string,
  extra: Partial<SourcedField> = {},
): SourcedField => ({
  provider,
  field: name,
  value: JSON.stringify(value),
  fetched_at: '2026-10-04T00:00:00Z',
  cached: false,
  manually_edited: false,
  ...extra,
});
describe('安利墙元数据回退', () => {
  it('保留已有评分、热门标签和离线 Hikarinagi 快照的优先级', () => {
    expect(hasHikarinagiRates(snapshot('897'))).toBe(true);
    expect(
      hasHikarinagiRates({ ...snapshot('897'), cached: true, stale: true }),
    ).toBe(true);
    expect(
      hasHikarinagiRates({
        ...snapshot('897'),
        average: null,
        rated_count: 0,
        keywords: [{ word: '剧情', count: 2 }],
      }),
    ).toBe(true);
  });
  it('未绑定或只有游玩计数的 17745 空评分墙允许回退', () => {
    expect(hasHikarinagiRates(null)).toBe(false);
    expect(
      hasHikarinagiRates({
        ...snapshot('17745'),
        average: null,
        rated_count: 0,
        status_counts: { completed: 2, going: 0, on_hold: 0, dropped: 0 },
      }),
    ).toBe(false);
  });
  it('从已刮削的同源同作品取评分与前六个有效标签，不读取本地个人评分', () => {
    const result = metadataRatesFallback([
      field('hikarinagi', 'source_rating', '9.0'),
      field('manual', 'source_rating', '10'),
      field('bangumi', 'source_rating', '7.2', { remote_id: '447039' }),
      field('bangumi', 'source_tags', '错误作品', { remote_id: 'other' }),
      field(
        'bangumi',
        'source_tags',
        '国产\n\nGalgame\ngalgame\nSLG\n2024\nPC\n后宫\n同居',
        { remote_id: '447039' },
      ),
    ]);
    expect(result).toEqual({
      provider: 'bangumi',
      label: 'Bangumi',
      average: 7.2,
      tags: ['国产', 'Galgame', 'SLG', '2024', 'PC', '后宫'],
      fetched_at: '2026-10-04T00:00:00Z',
    });
  });
  it('按作品元数据优先级选择 Bangumi 或 VNDB，不拼接不同来源标签', () => {
    const bgm = field('bangumi', 'source_rating', '7.2');
    const vndb = field('vndb', 'source_rating', '8.15');
    const tags = field('vndb', 'source_tags', 'Romance\nComedy');
    expect(metadataRatesFallback([vndb, bgm, tags])).toMatchObject({
      provider: 'vndb',
      average: 8.15,
      tags: ['Romance', 'Comedy'],
    });
    expect(metadataRatesFallback([bgm, vndb, tags])).toMatchObject({
      provider: 'bangumi',
      tags: [],
    });
  });
  it('无评分、无效 JSON、越界、零分、人工覆盖不会生成伪造评分', () => {
    for (const value of ['', 'NaN', 'Infinity', '0', '-1', '10.1', 'garbage']) {
      expect(
        metadataRatesFallback([field('bangumi', 'source_rating', value)]),
      ).toBeNull();
    }
    expect(
      metadataRatesFallback([
        field('bangumi', 'source_rating', '9', { manually_edited: true }),
      ]),
    ).toBeNull();
    expect(
      metadataRatesFallback([
        field('bangumi', 'source_rating', '9', { value: '{invalid' }),
      ]),
    ).toBeNull();
    expect(
      metadataRatesFallback([field('bangumi', 'source_tags', '国产')]),
    ).toBeNull();
    expect(
      metadataRatesFallback([
        field('bangumi', 'source_rating', '0'),
        field('vndb', 'source_rating', '8.0'),
      ]),
    ).toMatchObject({ provider: 'vndb', average: 8 });
  });
});
describe('安利墙异步状态', () => {
  it('切换作品后丢弃旧结果和旧错误', async () => {
    for (const fails of [false, true]) {
      const first = deferred(),
        second = deferred();
      const state = initial();
      const loader = createRatesLoader(
        state,
        (id) => (id === 'first' ? first.promise : second.promise),
        () => '旧错误',
      );
      const a = loader.refresh('first');
      const b = loader.refresh('second');
      second.resolve({ wall: snapshot('123') });
      await b;
      if (fails) first.reject(new Error('旧请求失败'));
      else first.resolve({ wall: snapshot('897') });
      await a;
      expect(state).toMatchObject({
        game_id: 'second',
        loading: false,
        error: '',
        wall: { remote_id: '123' },
      });
    }
  });
  it('同一作品重新绑定后立即清空原统计，未绑定不保留旧卡片', async () => {
    const state = initial();
    state.wall = snapshot('897');
    const next = deferred();
    const loader = createRatesLoader(
      state,
      () => next.promise,
      () => '错误',
    );
    const work = loader.refresh('same');
    expect(state.wall).toBeNull();
    expect(state.loading).toBe(true);
    next.resolve({ wall: null });
    await work;
    expect(state.wall).toBeNull();
    expect(state.loading).toBe(false);
  });
  it('卸载后到达的结果和错误不修改卡片状态', async () => {
    const state = initial(),
      pending = deferred();
    const loader = createRatesLoader(
      state,
      () => pending.promise,
      () => '错误',
    );
    const work = loader.refresh('first');
    loader.dispose();
    pending.resolve({ wall: snapshot('897') });
    await work;
    expect(state.wall).toBeNull();
  });
  it('刷新明确传递强制参数并显示实际错误，不生成统计', async () => {
    const state = initial();
    const loader = createRatesLoader(
      state,
      async (id, refresh) => {
        expect(id).toBe('local');
        expect(refresh).toBe(true);
        throw new Error('网络失败');
      },
      (error) => (error as Error).message,
    );
    await loader.refresh('local', true);
    expect(state).toMatchObject({
      wall: null,
      loading: false,
      error: '网络失败',
    });
  });
});
