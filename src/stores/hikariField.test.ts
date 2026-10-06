import { beforeEach, describe, expect, it, vi } from 'vitest';
const mocks = vi.hoisted(() => ({
  api: vi.fn(),
  refresh: vi.fn(),
  notify: vi.fn(),
  open: vi.fn(),
}));
vi.mock('./library', () => ({
  desktop: true,
  api: mocks.api,
  refreshLibrary: mocks.refresh,
  notify: mocks.notify,
  errorText: (e: unknown) => (e instanceof Error ? e.message : String(e)),
}));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: mocks.open }));
vi.mock('./ownedMetadata', () => ({ queueOwnedMetadata: vi.fn() }));
import {
  hikariField,
  loginHikariField,
  startHikariDownload,
  confirmHikariFolder,
  pollHikariDownloads,
  stopHikariPolling,
} from './hikariField';
import { operations, removeOperation } from './operations';
import { queueOwnedMetadata } from './ownedMetadata';
const account = {
  status: 'authenticated' as const,
  profile: { id: 7, name: '测试玩家' },
  message: '已登录',
};
const task = {
  id: 'fixture',
  game_id: 'game',
  title: '已购故事',
  status: 'running' as const,
  message: '正在下载',
  downloaded: 30,
  total: 100,
  speed: 10,
  install_path: 'D:\\Games\\HikariFieldGames\\fixture',
};
beforeEach(() => {
  stopHikariPolling();
  vi.clearAllMocks();
  operations.splice(0);
  Object.assign(hikariField, {
    account,
    settings: { root: null, uuid: '' },
    loaded: true,
    busy: false,
    error: '',
    tasks: [],
    starting: [],
    folder_game: '',
    folder_parent: '',
    folder_busy: false,
    folder_error: '',
  });
  mocks.api.mockImplementation(async (command: string) => {
    if (command === 'login_hikarifield') return account;
    if (command === 'sync_hikarifield') return { owned: 2, imported: 1 };
    if (command === 'start_hikarifield_download') return task;
    if (command === 'list_hikarifield_downloads') return [task];
    if (command === 'set_hikarifield_path')
      return { root: 'D:\\Games\\HikariFieldGames', uuid: 'fixture' };
    throw new Error(command);
  });
});
describe('HIKARI FIELD account, first download and native task lifecycle', () => {
  it('automatically imports purchases after login and refreshes the existing library', async () => {
    await loginHikariField('test@example.test', 'fixture-password');
    expect(mocks.api.mock.calls.map(([command]) => command)).toEqual([
      'login_hikarifield',
      'sync_hikarifield',
    ]);
    expect(hikariField.account.profile?.id).toBe(7);
    expect(hikariField.sync_message).toContain('2 部');
    expect(mocks.refresh).toHaveBeenCalledWith({ reloadDetails: true });
    expect(queueOwnedMetadata).toHaveBeenCalledOnce();
    expect(JSON.stringify(hikariField)).not.toContain('fixture-password');
  });
  it('asks for a directory before the first download and reuses it afterward', async () => {
    await startHikariDownload('game');
    expect(hikariField.folder_game).toBe('game');
    expect(mocks.api).not.toHaveBeenCalled();
    hikariField.folder_parent = 'D:\\Games';
    await confirmHikariFolder();
    expect(mocks.api.mock.calls.map(([command]) => command)).toEqual([
      'set_hikarifield_path',
      'start_hikarifield_download',
      'list_hikarifield_downloads',
    ]);
    expect(hikariField.folder_game).toBe('');
    expect(hikariField.settings.root).toContain('HikariFieldGames');
    expect(operations[0].kind).toBe('download');
    expect(operations[0].cancel).toBeTypeOf('function');
    hikariField.tasks = [];
    await startHikariDownload('next');
    expect(hikariField.folder_game).toBe('');
    expect(
      mocks.api.mock.calls.filter(([c]) => c === 'set_hikarifield_path'),
    ).toHaveLength(1);
  });
  it('keeps task history, refreshes launchability when complete and does not resurrect erased tasks', async () => {
    await pollHikariDownloads();
    mocks.api.mockResolvedValue([
      { ...task, status: 'completed', downloaded: 100, speed: 0 },
    ]);
    await pollHikariDownloads();
    expect(operations[0].status).toBe('completed');
    expect(operations[0].cancel).toBeUndefined();
    expect(mocks.refresh).toHaveBeenCalledWith({ reloadDetails: true });
    removeOperation(operations[0].id);
    await pollHikariDownloads();
    expect(operations).toHaveLength(0);
  });
  it('retains the first-download request after a directory permission failure', async () => {
    await startHikariDownload('game');
    hikariField.folder_parent = 'D:\\Games';
    mocks.api.mockRejectedValue(new Error('目录不可写'));
    await confirmHikariFolder();
    expect(hikariField.folder_game).toBe('game');
    expect(hikariField.settings.root).toBeNull();
    expect(hikariField.folder_error).toBe('目录不可写');
    expect(hikariField.folder_busy).toBe(false);
  });
  it('refreshes launchability when native verification completes before the first poll', async () => {
    hikariField.settings.root = 'D:\\Games\\HikariFieldGames';
    mocks.api.mockImplementation(async (command: string) => {
      if (command === 'start_hikarifield_download')
        return { ...task, status: 'queued' };
      if (command === 'list_hikarifield_downloads')
        return [{ ...task, status: 'completed', speed: 0 }];
      throw new Error(command);
    });
    await startHikariDownload('game');
    expect(mocks.refresh).toHaveBeenCalledWith({ reloadDetails: true });
    expect(operations[0].status).toBe('completed');
  });
});
