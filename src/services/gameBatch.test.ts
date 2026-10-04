import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { PreviewGame } from '../preview/data';
import type { PreviewGroup } from '../preview/groups';

const state = vi.hoisted(() => ({
  desktop: false,
  api: vi.fn(),
  notify: vi.fn(),
  refreshLibrary: vi.fn(),
  preview: { games: [] as PreviewGame[], groups: [] as PreviewGroup[] },
}));
vi.mock('../stores/library', () => ({
  get desktop() {
    return state.desktop;
  },
  api: state.api,
  notify: state.notify,
  refreshLibrary: state.refreshLibrary,
  preview: state.preview,
  errorText: (error: Error) => error.message,
}));
import {
  addLibraryGamesToGroup,
  removeLibraryGames,
  removeLibraryGamesFromGroup,
} from './gameBatch';

beforeEach(() => {
  vi.resetAllMocks();
  state.desktop = false;
  state.preview.games = ['one', 'two'].map(
    (game_id) => ({ game_id }) as PreviewGame,
  );
  state.preview.groups = [
    { group_id: 'group', name: '分组', game_ids: ['one'] },
  ];
});
describe('游戏批量操作的持久化边界', () => {
  it('从当前组移除保留其他成员、作品及其他组归属', async () => {
    state.preview.groups.push({
      group_id: 'other',
      name: '其他组',
      game_ids: ['one'],
    });
    state.preview.groups[0]!.game_ids.push('two');
    expect(await removeLibraryGamesFromGroup('group', ['one'])).toBeNull();
    expect(state.preview.groups[0]!.game_ids).toEqual(['two']);
    expect(state.preview.groups[1]!.game_ids).toEqual(['one']);
    expect(state.preview.games).toHaveLength(2);
    expect(state.api).not.toHaveBeenCalled();
  });
  it('桌面移出仅替换当前普通组成员，智能组拒绝修改', async () => {
    state.desktop = true;
    state.api
      .mockResolvedValueOnce({
        kind: 'normal',
        member_ids: ['one', 'two', 'other'],
      })
      .mockResolvedValueOnce({});
    expect(await removeLibraryGamesFromGroup('group', ['one'])).toBeNull();
    expect(state.api).toHaveBeenLastCalledWith('set_collection_members', {
      collection_id: 'group',
      game_ids: ['two', 'other'],
    });
    state.api.mockResolvedValueOnce({ kind: 'smart', member_ids: ['one'] });
    expect(await removeLibraryGamesFromGroup('smart', ['one'])).toContain(
      '智能分组',
    );
    expect(state.api).toHaveBeenCalledTimes(3);
  });
  it('演示删除清理组关系，去重并保留失败项，不调用桌面 API', async () => {
    expect(await removeLibraryGames(['one', 'missing', 'one'])).toEqual({
      succeeded: ['one'],
      failed: ['missing'],
    });
    expect(state.preview.games.map((game) => game.game_id)).toEqual(['two']);
    expect(state.preview.groups[0]!.game_ids).toEqual([]);
    expect(state.api).not.toHaveBeenCalled();
    expect(state.refreshLibrary).not.toHaveBeenCalled();
  });
  it('桌面删除使用明确确认，逐条失败不撤回成功项，只刷新一次', async () => {
    state.desktop = true;
    state.api
      .mockResolvedValueOnce(true)
      .mockRejectedValueOnce(new Error('正在游玩'))
      .mockResolvedValueOnce(false);
    expect(await removeLibraryGames(['one', 'two', 'missing', 'one'])).toEqual({
      succeeded: ['one'],
      failed: ['two', 'missing'],
    });
    expect(state.api.mock.calls).toEqual([
      ['remove_game', { id: 'one', confirmed: true }],
      ['remove_game', { id: 'two', confirmed: true }],
      ['remove_game', { id: 'missing', confirmed: true }],
    ]);
    expect(state.refreshLibrary).toHaveBeenCalledTimes(1);
    expect(state.preview.games).toHaveLength(2);
  });
  it('桌面添加读取最新成员并去重合并，不覆盖已有成员', async () => {
    state.desktop = true;
    state.api
      .mockResolvedValueOnce({
        kind: 'normal',
        member_ids: ['existing', 'two'],
      })
      .mockResolvedValueOnce({});
    expect(
      await addLibraryGamesToGroup('group', ['one', 'two', 'one']),
    ).toBeNull();
    expect(state.api.mock.calls).toEqual([
      ['get_collection', { collection_id: 'group' }],
      [
        'set_collection_members',
        { collection_id: 'group', game_ids: ['existing', 'two', 'one'] },
      ],
    ]);
    expect(state.refreshLibrary).toHaveBeenCalledTimes(1);
  });
  it('智能分组拒绝手动写入，失败显示错误且不报告成功', async () => {
    state.desktop = true;
    state.api.mockResolvedValueOnce({ kind: 'smart', member_ids: [] });
    expect(await addLibraryGamesToGroup('group', ['one'])).toContain(
      '智能分组',
    );
    expect(state.api).toHaveBeenCalledTimes(1);
    state.api.mockRejectedValueOnce(new Error('写入失败'));
    expect(await addLibraryGamesToGroup('group', ['one'])).toBe('写入失败');
    expect(state.notify).not.toHaveBeenCalled();
  });
  it('连续并发添加按组串行，第二次合并已保存的新成员', async () => {
    state.desktop = true;
    let members = ['existing'];
    let release!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    let firstWrite = true;
    state.api.mockImplementation(async (command, payload) => {
      if (command === 'get_collection')
        return { kind: 'normal', member_ids: [...members] };
      if (firstWrite) {
        firstWrite = false;
        await gate;
      }
      members = payload.game_ids;
      return {};
    });
    const first = addLibraryGamesToGroup('group', ['one']);
    const second = addLibraryGamesToGroup('group', ['two']);
    await vi.waitFor(() => expect(state.api).toHaveBeenCalledTimes(2));
    release();
    expect(await Promise.all([first, second])).toEqual([null, null]);
    expect(members).toEqual(['existing', 'one', 'two']);
  });
  it('一次写入失败后，后续添加仍能成功且不伪造第一次成功', async () => {
    state.desktop = true;
    state.api
      .mockRejectedValueOnce(new Error('写入失败'))
      .mockResolvedValueOnce({ kind: 'normal', member_ids: ['existing'] })
      .mockResolvedValueOnce({});
    expect(
      await Promise.all([
        addLibraryGamesToGroup('group', ['one']),
        addLibraryGamesToGroup('group', ['two']),
      ]),
    ).toEqual(['写入失败', null]);
    expect(state.notify).toHaveBeenCalledTimes(1);
    expect(state.api).toHaveBeenLastCalledWith('set_collection_members', {
      collection_id: 'group',
      game_ids: ['existing', 'two'],
    });
  });
  it('演示添加保留成员，失效选项不产生部分写入', async () => {
    expect(await addLibraryGamesToGroup('group', ['two', 'one'])).toBeNull();
    expect(state.preview.groups[0]!.game_ids).toEqual(['one', 'two']);
    expect(await addLibraryGamesToGroup('group', ['missing'])).toContain(
      '不存在',
    );
    expect(state.preview.groups[0]!.game_ids).toEqual(['one', 'two']);
    expect(state.api).not.toHaveBeenCalled();
  });
});
