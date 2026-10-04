<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import {
  preview,
  desktop,
  local,
  refreshLibrary,
  openImport,
  batchUpdateLibraryGames,
  query,
  updateLibraryGame,
  notify,
} from '../stores/library';
import { galleryGroup } from '../preview/groups';
import { STATUS_LABELS } from '../preview/data';
import { matchesGameQuery, sortGames } from '../services/gameQuery';
import { selectGalleryGroup } from '../services/groupNavigation';
import {
  addLibraryGamesToGroup,
  removeLibraryGames,
} from '../services/gameBatch';
import VirtualGallery from '../components/preview/VirtualGallery.vue';
import PreviewIcon from '../components/preview/PreviewIcon.vue';
import LibraryContextMenu from '../components/LibraryContextMenu.vue';
import {
  requestGroupAction,
  type GameMenuAction,
} from '../services/libraryContextActions';

import {
  homeSearchIntent,
  homeFavoriteIntent,
} from '../services/homeNavigation';
const searchInput = ref<HTMLInputElement>();
onMounted(() => {
  if (!homeSearchIntent.value) return;
  homeSearchIntent.value = false;
  void nextTick().then(() =>
    requestAnimationFrame(() => searchInput.value?.focus()),
  );
});
const router = useRouter();
const renameDialog = ref<HTMLDialogElement>();
const renameInput = ref<HTMLInputElement>();
const renameId = ref('');
const renameName = ref('');
const renameError = ref('');
let menuOpener: HTMLElement | null = null;
async function gameAction(
  id: string,
  action: GameMenuAction,
  opener: HTMLElement | null,
) {
  if (action === 'add_to_group' || action === 'remove_from_group') return;
  if (busy.value) return;
  const game = preview.games.find((game) => game.game_id === id);
  if (!game) return;
  menuOpener = opener;
  if (action === 'manage') {
    await router.push({
      name: 'game-detail',
      params: { game_id: id },
      query: { panel: 'information' },
    });
  } else if (action === 'delete') {
    preview.dialog = {
      title: `删除“${game.title}”的库记录？`,
      opener: () => opener,
      description: desktop
        ? '先保存数据库安全快照，再移除库记录。本地游戏、存档和已有备份文件都会保留。'
        : '仅移除演示作品及分组关系，不访问数据库或本地文件。',
      confirm_label: '确认删除',
      action: () => {
        void runBatch(removeLibraryGames, [id], true);
      },
    };
  } else {
    renameId.value = id;
    renameName.value = game.title;
    renameError.value = '';
    await nextTick();
    renameDialog.value?.showModal();
    renameInput.value?.focus();
    renameInput.value?.select();
  }
}
async function saveGameName() {
  if (busy.value) return;
  const title = renameName.value.trim();
  if (!title || title.length > 200) {
    renameError.value = '作品名称需要 1～200 个字符。';
    renameInput.value?.focus();
    return;
  }
  busy.value = true;
  try {
    if (desktop) {
      if (!(await updateLibraryGame(renameId.value, () => ({ title })))) {
        renameError.value = '名称未保存，请检查错误提示后重试。';
        return;
      }
    } else {
      const game = preview.games.find(
        (game) => game.game_id === renameId.value,
      );
      if (!game) {
        renameError.value = '作品已不存在。';
        return;
      }
      game.title = title;
    }
    notify(`${desktop ? '' : '演示：'}作品名称已保存。`);
    renameDialog.value?.close();
  } finally {
    busy.value = false;
  }
}
function renameKey(event: KeyboardEvent) {
  if (event.key !== 'Tab') return;
  const controls = [
    ...(renameDialog.value?.querySelectorAll<HTMLElement>(
      'button:not(:disabled),input:not(:disabled)',
    ) ?? []),
  ];
  const first = controls[0],
    last = controls.at(-1);
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault();
    last?.focus();
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault();
    first?.focus();
  }
}
function restoreMenuFocus() {
  if (menuOpener?.isConnected) menuOpener.focus();
  else document.querySelector<HTMLElement>('[data-page-heading]')?.focus();
  menuOpener = null;
}
const selectedIds = ref<string[]>([]);
const selectionMode = ref(false);
const busy = ref(false);
const groupDialog = ref<HTMLDialogElement>();
const targetGroup = ref('');
const groupError = ref('');
const dialogIds = ref<string[]>([]);
const customGroups = computed(() =>
  preview.groups.filter((group) => !group.hidden),
);
const targetGroups = computed(() =>
  customGroups.value.filter((group) => !group.smart),
);
const activeGroup = computed(() =>
  galleryGroup(
    preview.games,
    preview.groups,
    preview.gallery_state.collection_id,
  ),
);
const activeQuery = computed(() =>
  query({
    search: preview.query,
    statuses:
      preview.status_filter !== 'all' && preview.status_filter !== 'favorite'
        ? [preview.status_filter]
        : [],
    favorite:
      preview.status_filter === 'favorite' || homeFavoriteIntent.value
        ? true
        : null,
    sort:
      preview.gallery_state.system === 'recently-played'
        ? 'last_played_at'
        : 'added_at',
    filters:
      preview.gallery_state.system === 'pending'
        ? { metadata_pending: true }
        : {},
  }),
);
const filtered = computed(() => {
  if (preview.show_empty) return [];
  const members = activeGroup.value
    ? new Set(activeGroup.value.games.map((game) => game.game_id))
    : null;
  return sortGames(
    preview.games.filter(
      (game) =>
        matchesGameQuery(game, activeQuery.value) &&
        (preview.gallery_state.system !== 'recently-played' ||
          game.last_played_order > 0) &&
        (!members || members.has(game.game_id)),
    ),
    activeQuery.value,
  );
});
const systemTabs = [
  { value: 'all' as const, label: '全部作品' },
  { value: 'favorite' as const, label: '我的收藏' },
  { value: 'playing' as const, label: STATUS_LABELS.playing },
  { value: 'completed' as const, label: STATUS_LABELS.completed },
  { value: 'paused' as const, label: STATUS_LABELS.paused },
  { value: 'dropped' as const, label: STATUS_LABELS.dropped },
];
function isSystemActive(value: string) {
  return preview.gallery_state.collection_id === 'favorites'
    ? value === 'favorite'
    : !preview.gallery_state.collection_id && preview.status_filter === value;
}
function openSystem(value: (typeof systemTabs)[number]['value']) {
  selectGalleryGroup();
  preview.status_filter = value;
}
watch([activeGroup, () => local.loading], ([group, loading]) => {
  if (preview.gallery_state.collection_id && !group && !loading)
    selectGalleryGroup();
});
watch(
  () => [preview.gallery_state.collection_id, preview.status_filter],
  () => {
    selectedIds.value = [];
    selectionMode.value = false;
    if (!busy.value) groupDialog.value?.close();
  },
);
watch(filtered, (games) => {
  if (!busy.value)
    selectedIds.value = selectedIds.value.filter((id) =>
      games.some((game) => game.game_id === id),
    );
});
const allVisibleSelected = computed(
  () =>
    filtered.value.length > 0 &&
    filtered.value.every((game) => selectedIds.value.includes(game.game_id)),
);
function toggleSelected(id: string) {
  if (busy.value || !selectionMode.value) return;
  selectedIds.value = selectedIds.value.includes(id)
    ? selectedIds.value.filter((item) => item !== id)
    : [...selectedIds.value, id];
}
function toggleSelectionMode() {
  if (busy.value) return;
  selectionMode.value = !selectionMode.value;
  selectedIds.value = [];
}
function toggleAllVisible() {
  if (busy.value) return;
  selectedIds.value = allVisibleSelected.value
    ? []
    : filtered.value.map((game) => game.game_id);
}
async function runBatch(
  action: (ids: string[]) => Promise<{ failed: string[] }>,
  ids = [...selectedIds.value],
  preserveSelection = false,
) {
  if (busy.value || !ids.length) return;
  busy.value = true;
  try {
    const result = await action(ids);
    selectedIds.value = (
      preserveSelection ? selectedIds.value : result.failed
    ).filter((id) => filtered.value.some((game) => game.game_id === id));
  } finally {
    busy.value = false;
  }
}
function confirmRemove() {
  const ids = [...selectedIds.value];
  if (busy.value || !ids.length) return;
  preview.dialog = {
    title: `删除 ${ids.length} 部作品的库记录？`,
    description: desktop
      ? '先保存数据库安全快照，再移除作品、安装及其库内关联记录。本地游戏、存档和已有备份文件都会保留。重新扫描可重新导入。'
      : '仅从本次演示内存中移除作品及分组关系，不访问数据库或本地文件。',
    confirm_label: '确认删除',
    action: () => {
      void runBatch(removeLibraryGames, ids);
    },
  };
}
async function openGroupDialog() {
  dialogIds.value = [...selectedIds.value];
  targetGroup.value = targetGroups.value[0]?.group_id ?? '';
  groupError.value = '';
  await nextTick();
  groupDialog.value?.showModal();
}
async function submitGroup() {
  if (busy.value || !targetGroup.value || !dialogIds.value.length) return;
  busy.value = true;
  try {
    groupError.value =
      (await addLibraryGamesToGroup(targetGroup.value, dialogIds.value)) ?? '';
    if (!groupError.value) {
      selectedIds.value = [];
      groupDialog.value?.close();
    }
  } finally {
    busy.value = false;
  }
}
function clearFilters() {
  preview.query = '';
}
</script>
<template>
  <div class="games-page page-content">
    <header class="page-heading">
      <div>
        <p class="eyebrow">A SHELF OF POSSIBILITIES</p>
        <h1 tabindex="-1" data-page-heading>
          {{ activeGroup?.name || '所有值得相遇的世界。' }}
        </h1>
        <p class="page-lead">
          {{
            activeGroup
              ? `这个分组中有 ${activeGroup.games.length} 部作品。`
              : '把未开始的期待，和不愿忘记的时光，一起收藏。'
          }}
        </p>
      </div>
      <button class="primary-button" @click="openImport">
        <PreviewIcon name="plus" />{{
          desktop ? '添加游戏' : '添加游戏 · 演示'
        }}
      </button>
    </header>
    <div class="gallery-tabs" aria-label="游戏与分组">
      <button
        v-for="tab in systemTabs"
        :key="tab.value"
        :class="{ active: isSystemActive(tab.value) }"
        :aria-pressed="isSystemActive(tab.value)"
        :disabled="busy"
        @click="openSystem(tab.value)"
      >
        {{ tab.label
        }}<span v-if="tab.value === 'all'">{{
          preview.show_empty ? 0 : preview.games.length
        }}</span>
      </button>
      <LibraryContextMenu
        v-for="group in customGroups"
        :key="group.group_id"
        :label="`${group.name}分组操作`"
        :disabled="busy"
        @action="
          (action, opener) => requestGroupAction(group.group_id, action, opener)
        "
      >
        <button
          :class="{
            active: preview.gallery_state.collection_id === group.group_id,
          }"
          :aria-pressed="preview.gallery_state.collection_id === group.group_id"
          :disabled="busy"
          @click="selectGalleryGroup(group.group_id)"
        >
          {{ group.name }}
        </button>
      </LibraryContextMenu>
    </div>
    <div class="games-tools">
      <label class="search-field">
        <PreviewIcon name="search" />
        <input
          ref="searchInput"
          v-model="preview.query"
          data-library-search
          type="search"
          aria-label="搜索作品"
          placeholder="搜索作品名称或关键词…"
          :disabled="busy"
        />
      </label>
      <button
        class="secondary-button"
        :aria-pressed="selectionMode"
        :disabled="busy"
        @click="toggleSelectionMode"
      >
        {{ selectionMode ? '取消选择' : '批量选择' }}
      </button>
    </div>
    <div
      v-if="selectionMode"
      class="batch-toolbar"
      aria-label="批量操作"
      :aria-busy="busy"
    >
      <label class="batch-select-all"
        ><input
          type="checkbox"
          :checked="allVisibleSelected"
          :disabled="busy || !filtered.length"
          @change="toggleAllVisible"
        />全选当前结果</label
      >
      <span class="selection-count" role="status"
        >已选择 {{ selectedIds.length }} 部作品{{
          busy ? ' · 正在处理…' : ''
        }}</span
      >
      <template v-if="selectedIds.length">
        <button
          class="secondary-button delete-button"
          :disabled="busy"
          @click="confirmRemove"
        >
          删除
        </button>
        <button
          class="secondary-button"
          :disabled="busy"
          @click="
            runBatch((ids) => batchUpdateLibraryGames(ids, { favorite: true }))
          "
        >
          收藏
        </button>
        <button
          class="secondary-button"
          :disabled="busy"
          @click="openGroupDialog"
        >
          添加到分组
        </button>
      </template>
    </div>
    <p class="gallery-count">
      {{ String(filtered.length).padStart(2, '0') }} 段故事
      <span>{{
        desktop
          ? '· 本地数据库，收藏与分组自动保存'
          : '· 演示数据，收藏与筛选只在内存中生效'
      }}</span>
    </p>
    <p v-if="local.error" role="alert">
      {{ local.error }}
      <button class="quiet-button" @click="refreshLibrary()">重试</button>
    </p>
    <p v-if="desktop && local.loading" role="status">正在读取游戏库…</p>
    <VirtualGallery
      v-if="filtered.length"
      :games="filtered"
      view="grid"
      :selectable="selectionMode"
      :selected="selectedIds"
      :busy="busy"
      @select="toggleSelected"
      @action="gameAction"
    />
    <section v-else class="empty-stage">
      <PreviewIcon name="search" :size="42" />
      <h2>
        {{
          preview.show_empty
            ? '展廊正等待你的第一段故事。'
            : '这一页，暂时没有相遇。'
        }}
      </h2>
      <p>
        {{
          preview.show_empty
            ? '空库演示不会添加或扫描真实文件。'
            : '试试别的关键词，或让筛选条件轻一点。'
        }}
      </p>
      <button
        class="secondary-button"
        @click="
          preview.show_empty || !preview.games.length
            ? openImport()
            : clearFilters()
        "
      >
        {{ preview.show_empty ? '添加作品 · 演示' : '清除筛选' }}
      </button>
    </section>
    <Teleport to="body">
      <dialog
        ref="groupDialog"
        class="games-group-dialog"
        aria-labelledby="add-group-title"
        @cancel="busy && $event.preventDefault()"
      >
        <form @submit.prevent="submitGroup">
          <h2 id="add-group-title">添加到分组</h2>
          <p>将 {{ dialogIds.length }} 部作品添加到所选分组，保留原有成员。</p>
          <template v-if="targetGroups.length">
            <label for="target-group">目标分组</label>
            <select
              id="target-group"
              v-model="targetGroup"
              :disabled="busy"
              required
            >
              <option
                v-for="group in targetGroups"
                :key="group.group_id"
                :value="group.group_id"
              >
                {{ group.name }}
              </option>
            </select>
          </template>
          <p v-else>
            暂无可添加的普通分组，请先在左侧「游戏总览」中新建分组。智能分组按规则自动更新。
          </p>
          <p v-if="groupError" role="alert">{{ groupError }}</p>
          <footer class="editor-actions">
            <button
              type="button"
              class="quiet-button"
              :disabled="busy"
              @click="groupDialog?.close()"
            >
              取消
            </button>
            <button
              class="primary-button"
              :disabled="busy || !targetGroups.length"
            >
              {{ busy ? '正在添加…' : '确认添加' }}
            </button>
          </footer>
        </form>
      </dialog>
    </Teleport>
    <Teleport to="body">
      <dialog
        ref="renameDialog"
        class="game-name-dialog"
        aria-labelledby="game-name-title"
        @cancel.prevent="!busy && renameDialog?.close()"
        @keydown="renameKey"
        @close="restoreMenuFocus"
      >
        <form @submit.prevent="saveGameName">
          <h2 id="game-name-title">重命名作品</h2>
          <label
            >作品名称<input
              ref="renameInput"
              v-model="renameName"
              maxlength="200"
              :disabled="busy"
              @input="renameError = ''"
          /></label>
          <p v-if="renameError" role="alert">{{ renameError }}</p>
          <div class="dialog-actions">
            <button
              type="button"
              class="secondary-button"
              :disabled="busy"
              @click="renameDialog?.close()"
            >
              取消
            </button>
            <button type="submit" class="primary-button" :disabled="busy">
              {{ busy ? '保存中…' : '保存名称' }}
            </button>
          </div>
        </form>
      </dialog>
    </Teleport>
  </div>
</template>
<style scoped>
.gallery-tabs {
  overflow-x: auto;
  gap: var(--space-24);
}
.gallery-tabs button {
  flex-shrink: 0;
  white-space: nowrap;
}
.games-tools {
  display: flex;
  align-items: center;
  gap: var(--space-12);
  margin-top: var(--space-24);
}
.games-tools .search-field {
  flex: 0 1 28%;
}
.games-tools .secondary-button {
  flex-shrink: 0;
  min-height: 45px;
}
.selection-count {
  color: var(--muted);
  font-size: 12px;
}
.batch-toolbar .secondary-button {
  min-height: 36px;
  padding: 0 var(--space-12);
}
.delete-button {
  color: var(--danger);
}
.games-group-dialog {
  width: min(440px, calc(100vw - 48px));
  margin: auto;
  padding: var(--space-24);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  background: var(--surface);
  color: var(--text);
  box-shadow: var(--shadow-modal);
}
.games-group-dialog::backdrop {
  background: rgb(0 0 0 / 48%);
  backdrop-filter: blur(8px);
}
.games-group-dialog h2 {
  margin-top: 0;
}
.games-group-dialog p {
  font-size: 12px;
  color: var(--muted);
}
.games-group-dialog label {
  display: block;
  margin: var(--space-16) 0 var(--space-8);
}
.games-group-dialog select {
  width: 100%;
  padding: var(--space-12);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--text);
  background: var(--background);
}
.editor-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-12);
  margin-top: var(--space-24);
}
@media (max-width: 600px) {
  .games-tools {
    flex-wrap: wrap;
  }
  .games-tools .search-field {
    flex-basis: 100%;
  }
  .batch-select-all {
    margin-right: 0;
  }
}
</style>

<style scoped>
.game-name-dialog {
  margin: auto;
  width: min(440px, calc(100vw - 32px));
  padding: 24px;
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  background: var(--surface);
  color: var(--text);
}
.game-name-dialog label {
  display: grid;
  gap: 10px;
  margin: 24px 0;
}
.game-name-dialog input {
  padding: 12px;
  width: 100%;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--surface-hover);
  color: var(--text);
}
.game-name-dialog [role='alert'] {
  color: var(--danger);
}
.game-name-dialog::backdrop {
  background: #0007;
}
</style>
