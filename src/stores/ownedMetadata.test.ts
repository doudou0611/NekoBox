import { beforeEach, describe, expect, it, vi } from 'vitest';
const mocks = vi.hoisted(() => ({
  api: vi.fn(),
  refresh: vi.fn(),
  load: vi.fn(),
  local: { records: {} as Record<string, { metadata_status: string }> },
  preview: {
    games: [] as {
      game_id: string;
      title: string;
      cover_url: string;
      hikari_field: { app_id: number; released: boolean } | null;
    }[],
  },
}));
vi.mock('./library', () => ({
  desktop: true,
  api: mocks.api,
  local: mocks.local,
  preview: mocks.preview,
  refreshLibrary: mocks.refresh,
  loadDetail: mocks.load,
  errorText: (e: unknown) => (e instanceof Error ? e.message : String(e)),
}));
import {
  ownedMetadata,
  queueOwnedMetadata,
  stopOwnedMetadata,
  confirmOwnedMetadata,
  retryOwnedMetadata,
} from './ownedMetadata';
import { operations, activeOperationProgress } from './operations';
const candidate = (provider: string, confidence = 1, remote = '1') => ({
  provider,
  remote_id: remote,
  title: '官方作品名',
  confidence,
  matched_fields: [],
  explanation: '',
  fetched_at: '',
  cached: false,
});
const sourceConfig = {
  sources: [
    { provider: 'vndb', enabled: true },
    { provider: 'hikarinagi', enabled: false },
    { provider: 'bangumi', enabled: true },
  ],
};
async function finished() {
  await vi.waitFor(() => expect(ownedMetadata.running).toBe(false));
}
beforeEach(() => {
  vi.clearAllMocks();
  Object.assign(ownedMetadata, {
    open: false,
    running: false,
    stopping: false,
    items: [],
    operation_id: '',
  });
  operations.splice(0);
  mocks.preview.games = [
    {
      game_id: 'owned',
      title: '官方作品名',
      cover_url: 'official.jpg',
      hikari_field: { app_id: 7, released: true },
    },
  ];
  mocks.local.records = { owned: { metadata_status: 'local_only' } };
  mocks.api.mockImplementation(async (c: string) => {
    if (c === 'get_game')
      return {
        metadata_locked: false,
        metadata: [{ provider: 'hikarifield', remote_id: '7', field: 'title' }],
      };
    if (c === 'get_metadata_sources') return sourceConfig;
    if (c === 'search_metadata') return [];
    if (c === 'confirm_metadata_match')
      return { cover_message: null, supplementation_message: null };
    throw Error(c);
  });
});
describe('owned metadata background workflow', () => {
  it('follows enabled source order and preserves settings on automatic confirmation', async () => {
    mocks.api.mockImplementation(
      async (c: string, p: { providers?: string[] }) => {
        if (c === 'get_game') return { metadata_locked: false, metadata: [] };
        if (c === 'get_metadata_sources') return sourceConfig;
        if (c === 'search_metadata')
          return p.providers?.[0] === 'bangumi' ? [candidate('bangumi')] : [];
        if (c === 'confirm_metadata_match') return {};
      },
    );
    queueOwnedMetadata();
    await finished();
    expect(
      mocks.api.mock.calls
        .filter(([c]) => c === 'search_metadata')
        .map(([, p]) => p.providers),
    ).toEqual([['vndb'], ['bangumi']]);
    expect(mocks.api).toHaveBeenCalledWith(
      'confirm_metadata_match',
      expect.objectContaining({ provider: 'bangumi', manual: false }),
    );
    expect(ownedMetadata.items[0].status).toBe('completed');
    expect(ownedMetadata.open).toBe(false);
    expect(mocks.load).toHaveBeenCalledWith('owned');
    expect(operations[0].progress).toBe(1);
    expect(operations[0].open_details).toBeTypeOf('function');
  });
  it('retains ambiguous candidates for explicit confirmation and does not repeat sync work', async () => {
    mocks.api.mockImplementation(async (c: string) => {
      if (c === 'get_game') return { metadata_locked: false, metadata: [] };
      if (c === 'get_metadata_sources') return sourceConfig;
      if (c === 'search_metadata')
        return [candidate('vndb', 1), candidate('vndb', 1, '2')];
      if (c === 'confirm_metadata_match') return {};
    });
    queueOwnedMetadata();
    await finished();
    expect(ownedMetadata.items[0].status).toBe('review');
    expect(
      mocks.api.mock.calls.some(([c]) => c === 'confirm_metadata_match'),
    ).toBe(false);
    const count = mocks.api.mock.calls.length;
    queueOwnedMetadata();
    await finished();
    expect(mocks.api.mock.calls.length).toBe(count);
    const item = ownedMetadata.items[0];
    confirmOwnedMetadata(item, item.candidates[0]);
    await finished();
    expect(item.status).toBe('completed');
    expect(mocks.api).toHaveBeenCalledWith(
      'confirm_metadata_match',
      expect.objectContaining({ remote_id: '1', manual: false }),
    );
  });
  it('skips locked games and concurrent existing bindings before searching', async () => {
    mocks.api.mockResolvedValue({ metadata_locked: true, metadata: [] });
    queueOwnedMetadata();
    await finished();
    expect(ownedMetadata.items[0].status).toBe('skipped');
    expect(mocks.api.mock.calls).toHaveLength(1);
    ownedMetadata.items = [];
    mocks.api.mockResolvedValue({
      metadata_locked: false,
      metadata: [{ provider: 'vndb', remote_id: 'v1' }],
    });
    queueOwnedMetadata();
    await finished();
    expect(ownedMetadata.items[0].status).toBe('skipped');
  });
  it('continues to later enabled sources after an error and supports retry without reimporting', async () => {
    mocks.api.mockImplementation(async (c: string) => {
      if (c === 'get_game') return { metadata_locked: false, metadata: [] };
      if (c === 'get_metadata_sources') return sourceConfig;
      if (c === 'search_metadata') throw Error('网络暂不可用');
      if (c === 'confirm_metadata_match') return {};
    });
    queueOwnedMetadata();
    await finished();
    expect(ownedMetadata.items[0].status).toBe('failed');
    expect(ownedMetadata.items[0].message).toContain('vndb');
    expect(ownedMetadata.items[0].message).toContain('bangumi');
    mocks.api.mockImplementation(async (c: string) => {
      if (c === 'get_game') return { metadata_locked: false, metadata: [] };
      if (c === 'get_metadata_sources') return sourceConfig;
      if (c === 'search_metadata') return [candidate('vndb')];
      if (c === 'confirm_metadata_match') return {};
    });
    retryOwnedMetadata();
    await finished();
    expect(ownedMetadata.items[0].status).toBe('completed');
  });
  it('aborts active searches and queued games, with no late confirmation', async () => {
    mocks.preview.games.push({ ...mocks.preview.games[0], game_id: 'second' });
    mocks.api.mockImplementation(
      async (c: string, _p: unknown, opts?: { signal: AbortSignal }) => {
        if (c === 'get_game') return { metadata_locked: false, metadata: [] };
        if (c === 'get_metadata_sources') return sourceConfig;
        if (c === 'search_metadata')
          return new Promise((_, reject) =>
            opts?.signal.addEventListener('abort', () =>
              reject(Error('已取消')),
            ),
          );
      },
    );
    queueOwnedMetadata();
    await vi.waitFor(() =>
      expect(mocks.api.mock.calls.some(([c]) => c === 'search_metadata')).toBe(
        true,
      ),
    );
    stopOwnedMetadata();
    await finished();
    expect(ownedMetadata.items.map((i) => i.status)).toEqual([
      'cancelled',
      'cancelled',
    ]);
    expect(operations[0].status).toBe('cancelled');
    expect(
      mocks.api.mock.calls.some(([c]) => c === 'confirm_metadata_match'),
    ).toBe(false);
  });
  it('finishes a started atomic save before stopping subsequent games', async () => {
    mocks.preview.games.push({ ...mocks.preview.games[0], game_id: 'second' });
    let finish: (value: object) => void = () => {};
    mocks.api.mockImplementation(async (c: string) => {
      if (c === 'get_game') return { metadata_locked: false, metadata: [] };
      if (c === 'get_metadata_sources') return sourceConfig;
      if (c === 'search_metadata') return [candidate('vndb')];
      if (c === 'confirm_metadata_match')
        return new Promise((resolve) => {
          finish = resolve;
        });
    });
    queueOwnedMetadata();
    await vi.waitFor(() =>
      expect(ownedMetadata.items[0].status).toBe('applying'),
    );
    stopOwnedMetadata();
    expect(ownedMetadata.running).toBe(true);
    finish({});
    await finished();
    expect(ownedMetadata.items.map((i) => i.status)).toEqual([
      'completed',
      'cancelled',
    ]);
    expect(
      mocks.api.mock.calls.filter(([c]) => c === 'confirm_metadata_match'),
    ).toHaveLength(1);
  });
  it('does not queue local or already scraped games', async () => {
    mocks.preview.games.push({
      ...mocks.preview.games[0],
      game_id: 'local',
      hikari_field: null,
    });
    mocks.local.records.owned.metadata_status = 'synced';
    queueOwnedMetadata();
    await finished();
    expect(ownedMetadata.items).toHaveLength(0);
    expect(mocks.api).not.toHaveBeenCalled();
  });
  it('averages byte and item progress without adding incompatible units', () => {
    const base = {
      status: 'running' as const,
      message: '',
      created_at: 0,
      completed_at: null,
      open_details: null,
    };
    operations.push(
      {
        ...base,
        id: 'bytes',
        title: '下载',
        kind: 'download',
        total: 1_000_000,
        progress: 500_000,
        progress_unit: 'bytes',
      },
      {
        ...base,
        id: 'games',
        title: '刮削',
        kind: 'scrape',
        total: 10,
        progress: 2,
      },
    );
    expect(activeOperationProgress.value).toBe(35);
  });
});
