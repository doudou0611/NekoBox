import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { createPreviewGames } from './data';
import { sidebarGroups, validateGroupName } from './groups';
import {
  clearPreviewTimers,
  deletePreviewGroup,
  preview,
  resetPreview,
  savePreviewGroup,
  toggleFavorite,
} from './store';

beforeEach(() => {
  resetPreview();
  clearPreviewTimers();
});
afterEach(clearPreviewTimers);

describe('原型侧栏分组', () => {
  it('收藏独立显示，多组成员不重复进入未分组', () => {
    const games = createPreviewGames();
    games.forEach((game) => (game.favorite = false));
    games[0]!.favorite = true;
    const member = games[1]!.game_id;
    const groups = sidebarGroups(games, [
      {
        group_id: 'a',
        name: '计划补完',
        game_ids: [member, games[0]!.game_id],
      },
      { group_id: 'b', name: '短篇', game_ids: [member] },
    ]);
    expect(groups.map((group) => group.name)).toEqual([
      '收藏',
      '计划补完',
      '短篇',
      '未分组',
    ]);
    expect(groups[0]!.games).toEqual([games[0]]);
    expect(groups[1]!.games).toEqual([games[0], games[1]]);
    expect(groups[2]!.games).toEqual([games[1]]);
    expect(groups[3]!.games).toEqual(games.slice(2));
  });

  it('名称拒绝空白、超长、系统名称和大小写重复，允许保留自己的名称', () => {
    const groups = [{ group_id: 'a', name: 'Steam', game_ids: [] }];
    for (const name of [
      ' ',
      'a'.repeat(25),
      ' 收藏 ',
      '未分组',
      '全部游戏',
      'steam',
    ]) {
      expect(validateGroupName(name, groups)).not.toBeNull();
    }
    expect(validateGroupName(' Steam ', groups, 'a')).toBeNull();
    expect(validateGroupName(' 我的分组 ', groups)).toBeNull();
  });

  it('保存去掉无效和重复成员，重命名保留同一分组，失败不改动原数据', () => {
    const id = preview.games[0]!.game_id;
    expect(savePreviewGroup(' 待玩 ', [id, id, 'unknown'])).toBeNull();
    const group = preview.groups[0]!;
    expect(group.name).toBe('待玩');
    expect(group.game_ids).toEqual([id]);
    expect(savePreviewGroup('收藏', [], group.group_id)).not.toBeNull();
    expect(group.name).toBe('待玩');
    expect(savePreviewGroup('短篇', [], group.group_id)).toBeNull();
    expect(preview.groups).toHaveLength(1);
    expect(group.name).toBe('短篇');
    expect(group.game_ids).toEqual([]);
    expect(savePreviewGroup('不存在', [], 'missing')).not.toBeNull();
    expect(preview.groups).toHaveLength(1);
  });

  it('取消收藏、删除分组重新计算未分组，并保留游戏', () => {
    const game = preview.games[0]!;
    game.favorite = true;
    savePreviewGroup('待玩', [game.game_id]);
    toggleFavorite(game.game_id);
    expect(
      sidebarGroups(preview.games, preview.groups).at(-1)!.games,
    ).not.toContain(game);
    const snapshot = structuredClone(createPreviewGames());
    deletePreviewGroup(preview.groups[0]!.group_id);
    expect(
      sidebarGroups(preview.games, preview.groups).at(-1)!.games,
    ).toContain(game);
    expect(preview.games.map((item) => item.game_id)).toEqual(
      snapshot.map((item) => item.game_id),
    );
    expect(game.favorite).toBe(false);
  });

  it('空库分组没有成员，重置清除全部自定义组', () => {
    savePreviewGroup('待玩', [preview.games[0]!.game_id]);
    expect(
      sidebarGroups([], preview.groups).every(
        (group) => group.games.length === 0,
      ),
    ).toBe(true);
    resetPreview();
    expect(preview.groups).toEqual([]);
    expect(preview.games).toEqual(createPreviewGames());
  });
});

describe('动态与隐藏集合', () => {
  it('智能集合重新计算成员，隐藏集合不占用未分组', () => {
    const games = createPreviewGames();
    const query = {
      page: 1,
      page_size: 100,
      search: '月光',
      statuses: [],
      sources: [],
      tag_ids: [],
      favorite: null,
      collection_id: null,
      sort: 'added_at' as const,
      direction: 'desc' as const,
      filters: { release_year: 2023 },
    };
    const groups = [
      { group_id: 'smart', name: '月光', game_ids: [], smart: true, query },
      {
        group_id: 'hidden',
        name: '隐藏',
        hidden: true,
        game_ids: [games[2].game_id],
      },
    ];
    expect(sidebarGroups(games, groups)[1].games.map((g) => g.game_id)).toEqual(
      ['demo-forest'],
    );
    expect(sidebarGroups(games, groups).map((g) => g.group_id)).not.toContain(
      'hidden',
    );
    games[1].year = '2024';
    expect(sidebarGroups(games, groups)[1].games).toEqual([]);
    expect(sidebarGroups(games, groups).at(-1)?.games).toContain(games[2]);
  });
});

it('数据库智能集合直接使用权威成员，预览智能集合仍动态计算', () => {
  const games = createPreviewGames();
  const query = {
    page: 1,
    page_size: 100,
    search: 'Only an alias in database',
    statuses: [],
    sources: [],
    tag_ids: [],
    favorite: null,
    collection_id: null,
    sort: 'added_at' as const,
    direction: 'desc' as const,
  };
  const base = {
    group_id: 'smart',
    name: '数据库集合',
    game_ids: [games[0]!.game_id],
    smart: true,
    query,
  };
  expect(
    sidebarGroups(games, [{ ...base, member_source: 'database' }])[1]!.games,
  ).toEqual([games[0]]);
  expect(sidebarGroups(games, [base])[1]!.games).toEqual([]);
  expect(
    sidebarGroups(games, [{ ...base, member_source: 'database' }]).at(-1)!
      .games,
  ).not.toContain(games[0]);
});
