import type { PreviewGame } from './data';
import type { GameQuery } from '../types/domain';
import { matchesGameQuery } from '../services/gameQuery';

export interface PreviewGroup {
  group_id: string;
  name: string;
  game_ids: string[];
  smart?: boolean;
  member_source?: 'database';
  query?: GameQuery | null;
  hidden?: boolean;
  icon?: string | null;
  color?: string | null;
  cover_url?: string | null;
}

export interface SidebarGroup {
  group_id: string;
  name: string;
  editable: boolean;
  smart?: boolean;
  games: PreviewGame[];
  icon?: string | null;
  color?: string | null;
}

export function validateGroupName(
  name: string,
  groups: PreviewGroup[],
  editing_group_id?: string,
): string | null {
  const value = name.trim();
  if (!value) return '请输入分组名称。';
  if (value.length > 24) return '分组名称最多 24 个字符。';
  const normalized = value.toLocaleLowerCase();
  if (['收藏', '未分组', '全部游戏'].includes(normalized))
    return '此名称已用于系统分组，请换一个名称。';
  if (
    groups.some(
      (group) =>
        group.group_id !== editing_group_id &&
        group.name.toLocaleLowerCase() === normalized,
    )
  )
    return '已有同名分组，请换一个名称。';
  return null;
}

function groupMembers(
  games: PreviewGame[],
  group: PreviewGroup,
): PreviewGame[] {
  return group.smart && group.query && group.member_source !== 'database'
    ? games.filter((game) => matchesGameQuery(game, group.query!))
    : games.filter((game) => group.game_ids.includes(game.game_id));
}

export function galleryGroup(
  games: PreviewGame[],
  groups: PreviewGroup[],
  group_id: string,
): SidebarGroup | undefined {
  if (!group_id) return undefined;
  const custom = groups.find((group) => group.group_id === group_id);
  if (custom)
    return { ...custom, editable: true, games: groupMembers(games, custom) };
  return sidebarGroups(games, groups).find(
    (group) => group.group_id === group_id,
  );
}

export function sidebarGroups(
  games: PreviewGame[],
  groups: PreviewGroup[],
): SidebarGroup[] {
  const visible = groups.filter((group) => !group.hidden);
  const assigned = new Set(
    visible.flatMap((group) =>
      groupMembers(games, group).map((g) => g.game_id),
    ),
  );
  return [
    {
      group_id: 'favorites',
      name: '收藏',
      editable: false,
      games: games.filter((game) => game.favorite),
    },
    ...visible.map((group) => ({
      group_id: group.group_id,
      name: group.name,
      editable: true,
      smart: group.smart,
      icon: group.icon,
      color: group.color,
      games: groupMembers(games, group),
    })),
    {
      group_id: 'ungrouped',
      name: '未分组',
      editable: false,
      games: games.filter(
        (game) => !game.favorite && !assigned.has(game.game_id),
      ),
    },
  ];
}
