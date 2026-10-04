import { describe, expect, it } from 'vitest';
import { createRatesLoader, type RatesWallState } from './hikarinagiRates';
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
