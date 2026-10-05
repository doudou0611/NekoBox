import { beforeEach, expect, it, vi } from 'vitest';
import { DEFAULT_SETTINGS } from '../types/settings';
const state = vi.hoisted(() => ({ desktop: true, api: vi.fn() }));
vi.mock('./library', () => ({
  get desktop() {
    return state.desktop;
  },
  api: state.api,
}));
let store: typeof import('./settings');
beforeEach(async () => {
  vi.resetModules();
  state.desktop = true;
  state.api.mockReset();
  store = await import('./settings');
});
it('loads legacy preferences with an empty sidebar order', async () => {
  state.api.mockResolvedValue({ theme: 'dark' });
  await store.loadAppSettings();
  expect(store.appSettings.value.sidebar_game_order).toEqual({});
  expect(store.appSettings.value.theme).toBe('dark');
  expect(store.appSettings.value.gallery_columns).toBe(5);
});
it('queued order and theme writes retain each other and survive reloading settings', async () => {
  let saved = structuredClone(DEFAULT_SETTINGS);
  state.api.mockImplementation(async (command, value) => {
    if (command === 'save_app_settings')
      saved = JSON.parse(JSON.stringify(value));
    return structuredClone(saved);
  });
  await store.loadAppSettings();
  const before = { ...store.appSettings.value };
  await Promise.all([
    store.saveAppSettings(
      { ...before, sidebar_game_order: { favorites: ['b', 'a'] } },
      ['sidebar_game_order'],
    ),
    store.saveAppSettings({ ...before, theme: 'dark' }, ['theme']),
    store.saveAppSettings({ ...before, gallery_columns: 9 }, [
      'gallery_columns',
    ]),
  ]);
  expect(saved.theme).toBe('dark');
  expect(saved.gallery_columns).toBe(9);
  expect(saved.sidebar_game_order.favorites).toEqual(['b', 'a']);
  store.appSettings.loaded = false;
  await store.loadAppSettings();
  expect(store.appSettings.value.gallery_columns).toBe(9);
  expect(store.appSettings.value.sidebar_game_order.favorites).toEqual([
    'b',
    'a',
  ]);
});
it('preview saves can copy nested reactive order maps while changing another preference', async () => {
  state.desktop = false;
  await store.saveAppSettings(
    {
      ...store.appSettings.value,
      sidebar_game_order: { favorites: ['b', 'a'] },
    },
    ['sidebar_game_order'],
  );
  await store.saveAppSettings({ ...store.appSettings.value, theme: 'dark' }, [
    'theme',
  ]);
  expect(store.appSettings.value.sidebar_game_order.favorites).toEqual([
    'b',
    'a',
  ]);
  expect(state.api).not.toHaveBeenCalled();
});
