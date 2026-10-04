import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { GameSummary, GameDetail } from '../types/domain';
const transport = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({
  isTauri: () => true,
  invoke: transport.invoke,
  convertFileSrc: (path: string, protocol = 'asset') =>
    'http://' + protocol + '.localhost/' + encodeURIComponent(path),
}));
import {
  preview,
  local,
  refreshLibrary,
  loadDetail,
  toggleFavorite,
  updateLibraryGame,
  displayGame,
  cleanDescription,
  coverSource,
  batchUpdateLibraryGames,
  saveSmartCollection,
  query,
} from './library';
import { preview as demo, clearPreviewTimers } from '../preview/store';
function game(id: string): GameSummary {
  return {
    id,
    title: `本地 ${id}`,
    title_zh: null,
    title_ja: null,
    title_en: null,
    cover_url: null,
    developer: null,
    publisher: null,
    release_date: null,
    source_rating: null,
    source_tags: [],
    status: 'pending_confirmation',
    favorite: false,
    hidden: false,
    user_rating: null,
    total_playtime_seconds: 0,
    last_played_at: null,
    added_at: '2026-09-30T00:00:00Z',
    metadata_status: 'local_only',
    installations: [],
    tags: [],
  };
}
beforeEach(() => {
  transport.invoke.mockReset();
  local.records = {};
  local.collections = {};
  preview.groups = [];
  local.status = null;
  local.error = '';
  local.recommendation_error = '';
  local.home_error = '';
  local.scan = null;
  preview.games = [];
  clearPreviewTimers();
});
describe('production library boundary', () => {
  it('keeps real games and the last successful summaries when secondary home queries fail', async () => {
    const previousHome = {
      recent_game_ids: ['old'],
      week_playtime_seconds: 1,
      game_count: 1,
      playing_count: 0,
      completed_count: 0,
      pending_match_count: 0,
      save_issue_count: 0,
      continue_game_ids: [],
    };
    local.home = previousHome;
    local.recommendations = [];
    transport.invoke.mockImplementation(
      async (command: string, args: { request: { request_id: string } }) => ({
        request_id: args.request.request_id,
        success: !['get_recommendations', 'get_home_summary'].includes(command),
        error_code: ['get_recommendations', 'get_home_summary'].includes(
          command,
        )
          ? 'DATABASE_ERROR'
          : null,
        message: 'secondary unavailable',
        data: ['get_recommendations', 'get_home_summary'].includes(command)
          ? null
          : command === 'list_games'
            ? { items: [game('fresh')], total: 1, page: 1, page_size: 100 }
            : command === 'list_collections'
              ? []
              : {
                  data_directory: 'fixture/data',
                  schema_version: 1,
                  portable: true,
                  last_scan_task_id: null,
                },
      }),
    );
    await refreshLibrary();
    expect(Object.keys(local.records)).toEqual(['fresh']);
    expect(preview.games.map((g) => g.game_id)).toEqual(['fresh']);
    expect(local.home).toEqual(previousHome);
    expect(local.error).toBe('');
    expect(local.home_error).toContain('secondary unavailable');
    expect(local.recommendation_error).toContain('secondary unavailable');
    expect(local.loading).toBe(false);
  });

  it('uses the unified primary name and only Japanese as the secondary name', () => {
    const g = {
      ...game('titles'),
      title: '中文作品名',
      title_zh: '中文作品名',
      title_en: 'English Title',
      title_ja: ' 日本語の作品名 ',
    };
    expect(displayGame(g)).toMatchObject({
      title: '中文作品名',
      subtitle: '日本語の作品名',
    });
    expect(displayGame({ ...g, title: '我的自定义名称' }).title).toBe(
      '我的自定义名称',
    );
    expect(displayGame({ ...g, title_ja: null }).subtitle).toBe('');
    expect(displayGame({ ...g, title_ja: '  ' }).subtitle).toBe('');
    expect(displayGame({ ...g, title: '日本語の作品名' }).subtitle).toBe('');
  });
  it('reports partial batch failure and keeps actual successful records', async () => {
    const a = game('a'),
      b = game('b');
    local.records = { a, b };
    preview.games = [displayGame(a), displayGame(b)];
    transport.invoke.mockImplementation(
      async (
        command: string,
        args: {
          request: {
            request_id: string;
            payload: { game_id?: string; favorite?: boolean };
          };
        },
      ) => {
        const success = !(
          command === 'update_game' && args.request.payload.game_id === 'b'
        );
        return {
          success,
          error_code: success ? null : 'DATABASE_ERROR',
          message: success ? 'ok' : 'b 写入失败',
          request_id: args.request.request_id,
          data: !success
            ? null
            : command === 'update_game'
              ? {
                  ...a,
                  favorite: args.request.payload.favorite,
                  description: null,
                  metadata: [],
                }
              : command === 'list_games'
                ? {
                    items: [{ ...a, favorite: true }, b],
                    total: 2,
                    page: 1,
                    page_size: 100,
                  }
                : command === 'list_collections' ||
                    command === 'get_recommendations'
                  ? []
                  : command === 'backend_status'
                    ? {
                        data_directory: 'fixture/data',
                        schema_version: 1,
                        portable: true,
                        last_scan_task_id: null,
                      }
                    : {},
        };
      },
    );
    const result = await batchUpdateLibraryGames(['a', 'a', 'b'], {
      favorite: true,
    });
    expect(result).toEqual({ succeeded: ['a'], failed: ['b'] });
    expect(local.records.a.favorite).toBe(true);
    expect(local.records.b.favorite).toBe(false);
    expect(demo.toasts.at(-1)?.message).toContain('1 部失败');
  });
  it('saves every intelligent rule and never supplies manual members', async () => {
    const q = query({
      search: '作品',
      filters: { release_year: 2024, developer: 'Studio' },
    });
    const received: unknown[] = [];
    transport.invoke.mockImplementation(
      async (
        command: string,
        args: { request: { request_id: string; payload: unknown } },
      ) => {
        if (command === 'save_collection') received.push(args.request.payload);
        return {
          success: true,
          error_code: null,
          message: 'ok',
          request_id: args.request.request_id,
          data:
            command === 'list_collections' || command === 'get_recommendations'
              ? []
              : command === 'list_games'
                ? { items: [], total: 0, page: 1, page_size: 100 }
                : command === 'backend_status'
                  ? {
                      data_directory: 'fixture',
                      schema_version: 1,
                      portable: true,
                      last_scan_task_id: null,
                    }
                  : {},
        };
      },
    );
    expect(await saveSmartCollection('中文规则', q)).toBeNull();
    expect(received[0]).toMatchObject({
      kind: 'smart',
      query: {
        search: '作品',
        filters: { release_year: 2024, developer: 'Studio' },
      },
      member_ids: null,
    });
  });

  it('resolves cached covers against the current portable data directory', () => {
    const relative = 'covers/' + 'a'.repeat(64) + '.jpg';
    local.status = {
      data_directory: 'C:/Moved/data',
      schema_version: 1,
      portable: true,
      last_scan_task_id: null,
    };
    expect(coverSource(relative)).toBe(
      'http://cover.localhost/' + relative.slice('covers/'.length),
    );
    expect(coverSource('https://lain.bgm.tv/pic/cover/a.jpg')).toBe(
      'https://lain.bgm.tv/pic/cover/a.jpg',
    );
    expect(coverSource(null)).toBe('');
  });
  it.each([
    ['C:\\Games\\data', 'C:\\Games\\data'],
    ['\\\\?\\C:\\Games\\data\\', '\\\\?\\C:\\Games\\data'],
    ['\\\\Mac\\Home\\游戏 库\\data', '\\\\Mac\\Home\\游戏 库\\data'],
    [
      '\\\\?\\UNC\\Mac\\Home\\游戏 库\\data',
      '\\\\?\\UNC\\Mac\\Home\\游戏 库\\data',
    ],
    ['/Users/test/游戏 库/data/', '/Users/test/游戏 库/data'],
  ])(
    'loads portable covers without exposing native paths for %s',
    (directory, expected) => {
      local.status = {
        data_directory: directory,
        schema_version: 1,
        portable: true,
        last_scan_task_id: null,
      };
      const filename = 'a'.repeat(64) + '.jpg';
      const source = coverSource('covers/' + filename);
      expect(source).toBe('http://cover.localhost/' + filename);
      expect(source).not.toContain(expected);
    },
  );
  it('cleans legacy VNDB markup before rendering descriptions', () => {
    expect(cleanDescription('[url=/c123]角色[/url]\n\n[b]简介[/b]')).toBe(
      '角色\n\n简介',
    );
  });

  it('starts empty and loads all database pages without copying demonstration games', async () => {
    const rows = Array.from({ length: 101 }, (_, i) => game(String(i)));
    transport.invoke.mockImplementation(
      async (
        command: string,
        args: { request: { request_id: string; payload: { page?: number } } },
      ) => ({
        success: true,
        error_code: null,
        message: 'ok',
        request_id: args.request.request_id,
        data:
          command === 'backend_status'
            ? {
                data_directory: 'fixture/data',
                schema_version: 1,
                portable: true,
                last_scan_task_id: null,
              }
            : command === 'list_collections'
              ? []
              : {
                  items: rows.slice(
                    ((args.request.payload.page ?? 1) - 1) * 100,
                    (args.request.payload.page ?? 1) * 100,
                  ),
                  page: args.request.payload.page,
                  page_size: 100,
                  total: 101,
                },
      }),
    );
    expect(preview.games).toHaveLength(0);
    await refreshLibrary();
    expect(preview.games).toHaveLength(101);
    expect(preview.games.some((g) => g.game_id.startsWith('demo-'))).toBe(
      false,
    );
    expect(demo.games.length).toBeGreaterThan(0);
    expect(preview.games[0]?.cover_url).toBe('');
    expect(local.error).toBe('');
  });
  it('shows startup errors instead of inventing an empty database success', async () => {
    transport.invoke.mockImplementation(
      async (_command: string, args: { request: { request_id: string } }) => ({
        success: false,
        error_code: 'PERMISSION_DENIED',
        message: '目录无法写入',
        request_id: args.request.request_id,
        data: null,
      }),
    );
    await refreshLibrary();
    expect(local.status).toBeNull();
    expect(local.error).toBe('目录无法写入');
    expect(preview.games).toHaveLength(0);
  });
  it('serializes rapid favorite changes and applies only successful database responses', async () => {
    const g = game('local-id');
    local.records[g.id] = g;
    preview.games = [displayGame(g)];
    const saved: boolean[] = [];
    transport.invoke.mockImplementation(
      async (
        command: string,
        args: {
          request: { request_id: string; payload: { favorite: boolean } };
        },
      ) => {
        expect(command).toBe('update_game');
        saved.push(args.request.payload.favorite);
        return {
          success: true,
          error_code: null,
          message: 'ok',
          request_id: args.request.request_id,
          data: {
            ...g,
            favorite: args.request.payload.favorite,
            description: null,
            metadata: [],
          },
        };
      },
    );
    toggleFavorite(g.id);
    toggleFavorite(g.id);
    await vi.waitFor(() => expect(saved).toEqual([true, false]));
    expect(local.records[g.id]?.favorite).toBe(false);
    transport.invoke.mockImplementation(
      async (_command: string, args: { request: { request_id: string } }) => ({
        success: false,
        error_code: 'DATABASE_ERROR',
        message: '写入失败',
        request_id: args.request.request_id,
        data: null,
      }),
    );
    toggleFavorite(g.id);
    await vi.waitFor(() =>
      expect(demo.toasts.some((t) => t.message === '写入失败')).toBe(true),
    );
    expect(local.records[g.id]?.favorite).toBe(false);
  });
});

describe('detail survives summary refresh', () => {
  function backend(summary: GameSummary, detail: GameDetail) {
    transport.invoke.mockImplementation(
      async (command: string, args: { request: { request_id: string } }) => ({
        success: true,
        error_code: null,
        message: 'ok',
        request_id: args.request.request_id,
        data:
          command === 'get_game'
            ? detail
            : command === 'list_games'
              ? { items: [summary], total: 1, page: 1, page_size: 100 }
              : command === 'backend_status'
                ? {
                    data_directory: 'fixture',
                    schema_version: 1,
                    portable: true,
                    last_scan_task_id: null,
                  }
                : command === 'list_collections' ||
                    command === 'get_recommendations'
                  ? []
                  : {},
      }),
    );
  }
  it('preserves description and metadata while applying fresh playtime and favorites', async () => {
    const summary = game('a');
    const detail: GameDetail = {
      ...summary,
      description: '已经获取的中文简介。',
      metadata: [],
    };
    backend(summary, detail);
    await refreshLibrary();
    await loadDetail('a');
    expect(preview.games[0]?.description).toBe(detail.description);
    summary.total_playtime_seconds = 180;
    summary.favorite = true;
    for (let index = 0; index < 3; index++) await refreshLibrary();
    expect(preview.games[0]).toMatchObject({
      description: detail.description,
      favorite: true,
      playtime_seconds: 180,
    });
    expect(local.records.a).toMatchObject({
      description: detail.description,
      metadata: [],
      total_playtime_seconds: 180,
    });
    expect(
      transport.invoke.mock.calls.filter(([cmd]) => cmd === 'get_game'),
    ).toHaveLength(1);
  });
  it('refreshes loaded description and metadata after a metadata operation, while routine refresh remains light', async () => {
    const summary = game('a');
    const detail: GameDetail = {
      ...summary,
      description: 'Old description',
      metadata: [],
    };
    backend(summary, detail);
    await refreshLibrary();
    await loadDetail('a');
    backend(summary, {
      ...detail,
      description: 'Fresh description',
      metadata_locked: true,
    });
    await refreshLibrary({ reloadDetails: true });
    expect(local.records.a).toMatchObject({
      description: 'Fresh description',
      metadata_locked: true,
    });
    expect(preview.games[0]?.description).toBe('Fresh description');
    expect(
      transport.invoke.mock.calls.filter(([cmd]) => cmd === 'get_game'),
    ).toHaveLength(2);
  });
  it('prevents a late old detail request from undoing a metadata refresh', async () => {
    const summary = game('a');
    const oldDetail: GameDetail = {
      ...summary,
      description: 'Old',
      metadata: [],
    };
    const fresh: GameDetail = {
      ...oldDetail,
      description: 'Fresh',
      metadata_locked: true,
    };
    local.records.a = oldDetail;
    preview.games = [displayGame(summary, oldDetail)];
    backend(summary, fresh);
    const immediate = transport.invoke.getMockImplementation()!;
    let release = () => {};
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    let first = true;
    transport.invoke.mockImplementation(async (command, args) => {
      if (command === 'get_game' && first) {
        first = false;
        await gate;
        const response = await immediate(command, args);
        return { ...response, data: oldDetail };
      }
      return immediate(command, args);
    });
    const stale = loadDetail('a');
    await refreshLibrary({ reloadDetails: true });
    release();
    await stale;
    expect(local.records.a).toMatchObject({
      description: 'Fresh',
      metadata_locked: true,
    });
  });
  it('retains requested detail invalidation when a routine refresh overtakes it', async () => {
    const summary = game('a');
    const oldDetail: GameDetail = {
      ...summary,
      description: 'Old',
      metadata: [],
    };
    local.records.a = oldDetail;
    preview.games = [displayGame(summary, oldDetail)];
    backend(summary, { ...oldDetail, description: 'Fresh' });
    const immediate = transport.invoke.getMockImplementation()!;
    let release = () => {};
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    let first = true;
    transport.invoke.mockImplementation(async (command, args) => {
      if (command === 'get_game' && first) {
        first = false;
        await gate;
      }
      return immediate(command, args);
    });
    const metadata = refreshLibrary({ reloadDetails: true });
    await vi.waitFor(() => expect(first).toBe(false));
    await refreshLibrary();
    release();
    await metadata;
    expect(local.records.a).toMatchObject({ description: 'Fresh' });
    expect(local.loading).toBe(false);
  });
  it('keeps a detail that finishes while a summary refresh is pending', async () => {
    const summary = game('a');
    const detail: GameDetail = {
      ...summary,
      description: '并发加载的简介。',
      metadata: [],
    };
    backend(summary, detail);
    await refreshLibrary();
    let release: () => void = () => {};
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const immediate = transport.invoke.getMockImplementation()!;
    transport.invoke.mockImplementation(async (command, args) => {
      if (command === 'list_games') await gate;
      return immediate(command, args);
    });
    const refresh = refreshLibrary();
    await loadDetail('a');
    release();
    await refresh;
    expect(preview.games[0]?.description).toBe(detail.description);
  });
  it('honors a successful detail clearing and retains the last detail on fetch failure', async () => {
    const summary = game('a');
    const detail: GameDetail = {
      ...summary,
      description: '原有简介。',
      metadata: [],
    };
    backend(summary, detail);
    await refreshLibrary();
    await loadDetail('a');
    transport.invoke.mockRejectedValueOnce(new Error('temporary failure'));
    await loadDetail('a');
    expect(preview.games[0]?.description).toBe('原有简介。');
    detail.description = null;
    await loadDetail('a');
    await refreshLibrary();
    expect(preview.games[0]?.description).toContain('尚未填写');
  });
  it('evicts removed records instead of leaking old descriptions to a re-added summary', async () => {
    const summary = game('a');
    const detail: GameDetail = {
      ...summary,
      description: '已删除作品的简介。',
      metadata: [],
    };
    backend(summary, detail);
    await refreshLibrary();
    await loadDetail('a');
    const immediate = transport.invoke.getMockImplementation()!;
    transport.invoke.mockImplementation(async (command, args) => {
      const response = await immediate(command, args);
      if (command === 'list_games')
        response.data = { items: [], total: 0, page: 1, page_size: 100 };
      return response;
    });
    await refreshLibrary();
    expect(local.records.a).toBeUndefined();
    backend(summary, detail);
    await refreshLibrary();
    expect(preview.games[0]?.description).toContain('尚未填写');
  });
});

describe('refresh and write ordering', () => {
  it('does not resurrect an old favorite after a write, and the next status write preserves it', async () => {
    const g = game('race');
    let saved = { ...g, description: null, metadata: [] } as GameDetail;
    local.records[g.id] = saved;
    preview.games = [displayGame(saved, saved)];
    let release = () => {};
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    let delayed = true;
    transport.invoke.mockImplementation(async (command, args) => {
      let data: unknown;
      if (command === 'list_games')
        data = { items: [{ ...saved }], total: 1, page: 1, page_size: 100 };
      else if (command === 'update_game') {
        saved = { ...saved, ...args.request.payload, id: g.id };
        data = saved;
      } else if (command === 'list_collections' && delayed) {
        delayed = false;
        await gate;
        data = [];
      } else if (
        command === 'list_collections' ||
        command === 'get_recommendations'
      )
        data = [];
      else data = { last_scan_task_id: null };
      return {
        success: true,
        error_code: null,
        message: 'ok',
        request_id: args.request.request_id,
        data,
      };
    });
    const refresh = refreshLibrary();
    await updateLibraryGame(g.id, () => ({ favorite: true }), false);
    release();
    await refresh;
    expect(local.records[g.id]?.favorite).toBe(true);
    expect(local.loading).toBe(false);
    await updateLibraryGame(g.id, () => ({ status: 'completed' }), false);
    expect(saved).toMatchObject({ favorite: true, status: 'completed' });
  });
});
