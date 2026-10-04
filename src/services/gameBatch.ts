import {
  api,
  desktop,
  errorText,
  notify,
  preview,
  refreshLibrary,
} from '../stores/library';

/** Only call after the user confirms the captured list of records. */
export async function removeLibraryGames(ids: string[]) {
  const succeeded: string[] = [];
  const failed: string[] = [];
  for (const id of new Set(ids)) {
    try {
      if (desktop) {
        if (!(await api('remove_game', { id, confirmed: true }))) {
          failed.push(id);
          continue;
        }
      } else {
        if (!preview.games.some((game) => game.game_id === id)) {
          failed.push(id);
          continue;
        }
        preview.games = preview.games.filter((game) => game.game_id !== id);
        for (const group of preview.groups)
          group.game_ids = group.game_ids.filter((member) => member !== id);
      }
      succeeded.push(id);
    } catch {
      failed.push(id);
    }
  }
  if (desktop) await refreshLibrary();
  notify(
    `${desktop ? '' : '演示：'}已移除 ${succeeded.length} 部作品的库记录${failed.length ? `，${failed.length} 部失败，可重试` : ''}；本地文件保留。`,
  );
  return { succeeded, failed };
}

const groupAdds = new Map<string, Promise<string | null>>();
/** Serialize read/merge/write so consecutive drops cannot lose a member. */
export async function addLibraryGamesToGroup(groupId: string, ids: string[]) {
  return changeGroupMembers(groupId, ids, false);
}
export async function removeLibraryGamesFromGroup(
  groupId: string,
  ids: string[],
) {
  return changeGroupMembers(groupId, ids, true);
}
async function changeGroupMembers(
  groupId: string,
  ids: string[],
  remove: boolean,
) {
  const previous = groupAdds.get(groupId) ?? Promise.resolve(null);
  const operation = previous
    .catch(() => null)
    .then(() => addToGroup(groupId, ids, remove));
  groupAdds.set(groupId, operation);
  try {
    return await operation;
  } finally {
    if (groupAdds.get(groupId) === operation) groupAdds.delete(groupId);
  }
}
async function addToGroup(groupId: string, ids: string[], remove: boolean) {
  try {
    if (desktop) {
      // Read fresh members before replacing; never lose members outside the selection.
      const group = await api('get_collection', { collection_id: groupId });
      if (group.kind !== 'normal')
        return '智能分组按规则自动更新，不能手动修改成员。';
      await api('set_collection_members', {
        collection_id: groupId,
        game_ids: remove
          ? group.member_ids.filter((id) => !ids.includes(id))
          : [...new Set([...group.member_ids, ...ids])],
      });
      await refreshLibrary();
    } else {
      const group = preview.groups.find((item) => item.group_id === groupId);
      if (!group) return '分组已不存在，请重新选择。';
      if (group.smart) return '智能分组按规则自动更新，不能手动修改成员。';
      if (ids.some((id) => !preview.games.some((game) => game.game_id === id)))
        return '部分作品已不存在，请重新选择。';
      group.game_ids = remove
        ? group.game_ids.filter((id) => !ids.includes(id))
        : [...new Set([...group.game_ids, ...ids])];
    }
    notify(
      `${desktop ? '' : '演示：'}${remove ? '已移出当前分组，其他分组保留。' : '已添加到分组。'}`,
    );
    return null;
  } catch (error) {
    return errorText(error);
  }
}
