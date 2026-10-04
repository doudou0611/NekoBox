import { beforeEach, expect, it, vi } from 'vitest';
const mock = vi.hoisted(() => ({ api: vi.fn(), desktop: true }));
vi.mock('./library', () => ({
  api: mock.api,
  get desktop() {
    return mock.desktop;
  },
}));
let store: typeof import('./metadataSources');
beforeEach(async () => {
  vi.resetModules();
  mock.api.mockReset();
  mock.desktop = true;
  store = await import('./metadataSources');
});
it('fresh defaults enable only Hikarinagi and retain all sources for later opt-in', () => {
  expect(store.metadataSources.sources).toEqual([
    { provider: 'hikarinagi', enabled: true },
    { provider: 'bangumi', enabled: false },
    { provider: 'vndb', enabled: false },
  ]);
});
it('loading preserves existing enabled choices and order without saving over them', async () => {
  const sources = [
    { provider: 'vndb', enabled: true },
    { provider: 'bangumi', enabled: true },
    { provider: 'hikarinagi', enabled: false },
  ];
  mock.api.mockResolvedValue({ sources });
  await store.loadMetadataSources();
  expect(store.metadataSources.sources).toEqual(sources);
  expect(mock.api).toHaveBeenCalledExactlyOnceWith('get_metadata_sources', {});
});
it('other sources can still be enabled while refusing to turn off every source', async () => {
  const sources = store.metadataSources.sources.map((source) => ({
    ...source,
    enabled: true,
  }));
  await store.saveMetadataSources(sources);
  expect(mock.api).toHaveBeenCalledExactlyOnceWith('save_metadata_sources', {
    sources,
  });
  await expect(
    store.saveMetadataSources(
      sources.map((source) => ({ ...source, enabled: false })),
    ),
  ).rejects.toThrow('至少启用一个刮削源');
  expect(store.metadataSources.sources).toEqual(sources);
});
