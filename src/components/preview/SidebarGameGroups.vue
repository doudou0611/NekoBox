<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { RouterLink, useRoute, useRouter } from 'vue-router';
import { selectGalleryGroup } from '../../services/groupNavigation';
import type { GameQuery } from '../../types/domain';
import { STATUS_LABELS } from '../../preview/data';
import {
  sidebarGroups,
  galleryGroup,
  type PreviewGroup,
} from '../../preview/groups';
import {
  preview,
  savePreviewGroup,
  deletePreviewGroup,
  desktop,
  notify,
  movePreviewGroup,
} from '../../stores/library';
import PreviewCover from './PreviewCover.vue';
import LibraryContextMenu from '../LibraryContextMenu.vue';
import SidebarGameActions from '../SidebarGameActions.vue';
import {
  groupActionRequest,
  requestGroupAction,
  requestGameAction,
} from '../../services/libraryContextActions';
import PreviewIcon from './PreviewIcon.vue';
import {
  armGameDrag,
  gameDrag,
  gameDropCompleted,
  gameDropBusy,
} from '../../services/gameDrag';

defineProps<{ expanded: boolean }>();
const route = useRoute();
const router = useRouter();
async function openGroup(group_id: string) {
  selectGalleryGroup(group_id);
  await router.push({ name: 'games' });
  await nextTick();
  window.scrollTo({ top: 0 });
  document
    .querySelector<HTMLElement>('.games-page [data-page-heading]')
    ?.focus({ preventScroll: true });
}

const games = computed(() =>
  preview.show_empty
    ? []
    : [...preview.games].sort((a, b) => b.added_order - a.added_order),
);
const groups = computed(() => sidebarGroups(games.value, preview.groups));
const active_game_id = computed(() =>
  route.name === 'game-detail' ? String(route.params.game_id ?? '') : '',
);
const collapsed_groups = ref(new Set<string>());
watch(gameDropCompleted, (group_id) => {
  if (group_id) collapsed_groups.value.delete(group_id);
  gameDropCompleted.value = null;
});
const editor = ref<HTMLDialogElement>();
const name_input = ref<HTMLInputElement>();
const create_button = ref<HTMLButtonElement>();
const editing_id = ref<string>();
const draft_name = ref('');
const draft_members = ref<string[]>([]);
const error = ref('');
const saving = ref(false);
const draft_icon = ref('games');
const draft_color = ref('#80739b');
const draft_cover = ref('');
const draft_hidden = ref(false);
const draft_query = ref<GameQuery | null>(null);
const drag_group = ref('');
const sortTarget = ref('');
const sortPlacement = ref<'before' | 'after'>('before');
let cancelGroupDrag: (() => void) | undefined;
function startGroupDrag(
  event: PointerEvent,
  group_id: string,
  editable: boolean,
) {
  if (
    !editable ||
    saving.value ||
    gameDropBusy.value ||
    event.button !== 0 ||
    !event.isPrimary ||
    event.ctrlKey ||
    event.metaKey
  )
    return;
  cancelGroupDrag?.();
  const origin = { x: event.clientX, y: event.clientY };
  const pointerId = event.pointerId;
  let active = false;
  let cancelled = false;
  let frame = 0;
  const blockClick = (e: MouseEvent) => {
    e.preventDefault();
    e.stopImmediatePropagation();
  };
  const targetAt = (x: number, y: number) => {
    const target = document
      .elementFromPoint(x, y)
      ?.closest<HTMLElement>('[data-group-sort-id]');
    sortTarget.value = target?.dataset.groupSortId ?? '';
    if (target)
      sortPlacement.value =
        y < target.getBoundingClientRect().top + target.clientHeight / 2
          ? 'before'
          : 'after';
  };
  let position = origin;
  const scroll = () => {
    const list = document.querySelector<HTMLElement>(
      '#sidebar-game-overview .groups-scroll',
    );
    if (list) {
      const b = list.getBoundingClientRect();
      if (
        position.x >= b.left &&
        position.x <= b.right &&
        position.y >= b.top &&
        position.y <= b.bottom
      ) {
        list.scrollTop +=
          position.y < b.top + 28 ? -10 : position.y > b.bottom - 28 ? 10 : 0;
        targetAt(position.x, position.y);
      }
    }
    frame = requestAnimationFrame(scroll);
  };
  const cleanup = () => {
    window.removeEventListener('pointermove', move, true);
    window.removeEventListener('pointerup', up, true);
    window.removeEventListener('pointercancel', cleanup, true);
    window.removeEventListener('blur', cleanup);
    window.removeEventListener('keydown', key, true);
    cancelAnimationFrame(frame);
    drag_group.value = '';
    sortTarget.value = '';
    cancelGroupDrag = undefined;
    setTimeout(
      () => document.removeEventListener('click', blockClick, true),
      0,
    );
  };
  const move = (e: PointerEvent) => {
    if (e.pointerId !== pointerId || cancelled) return;
    if (!active && Math.hypot(e.clientX - origin.x, e.clientY - origin.y) < 8)
      return;
    if (!active) {
      active = true;
      drag_group.value = group_id;
      document.addEventListener('click', blockClick, true);
      frame = requestAnimationFrame(scroll);
    }
    e.preventDefault();
    position = { x: e.clientX, y: e.clientY };
    targetAt(e.clientX, e.clientY);
  };
  const up = (e: PointerEvent) => {
    if (e.pointerId !== pointerId) return;
    if (active && !cancelled) targetAt(e.clientX, e.clientY);
    const target = sortTarget.value;
    const placement = sortPlacement.value;
    cleanup();
    if (!active || cancelled || !target || target === group_id) return;
    saving.value = true;
    void movePreviewGroup(group_id, target, placement)
      .then((result) => {
        if (result) notify(result);
      })
      .finally(() => (saving.value = false));
  };
  const key = (e: KeyboardEvent) => {
    if (e.key !== 'Escape' || !active) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    cancelled = true;
    drag_group.value = '';
    sortTarget.value = '';
    cancelAnimationFrame(frame);
  };
  cancelGroupDrag = cleanup;
  window.addEventListener('pointermove', move, {
    capture: true,
    passive: false,
  });
  window.addEventListener('pointerup', up, true);
  window.addEventListener('pointercancel', cleanup, true);
  window.addEventListener('blur', cleanup);
  window.addEventListener('keydown', key, true);
}
onBeforeUnmount(() => cancelGroupDrag?.());
const editorMode = ref<'manage' | 'rename'>('manage');
const hiddenGroups = computed(() =>
  preview.groups.filter((group) => group.hidden),
);
watch(groupActionRequest, (request) => {
  if (!request) return;
  groupActionRequest.value = null;
  const group = preview.groups.find(
    (group) => group.group_id === request.group_id,
  );
  if (!group) return;
  if (request.action === 'delete') {
    preview.dialog = {
      title: `删除“${group.name}”分组？`,
      opener: () =>
        request.opener?.isConnected
          ? request.opener
          : (create_button.value ?? null),
      description: desktop
        ? '只移除分组及其成员关系，游戏、收藏和本地文件都会保留。删除前保存数据库安全快照。'
        : '只移除演示分组及其成员关系，作品和收藏保留。不访问数据库或本地文件。',
      confirm_label: '确认删除分组',
      action: () => {
        saving.value = true;
        void deletePreviewGroup(group.group_id)
          .then((result) => {
            if (result) notify(result);
            else if (!desktop) notify('演示：分组已删除，游戏与收藏保留。');
          })
          .finally(() => {
            saving.value = false;
          });
      },
    };
  } else {
    void openEditor(group, request.action, request.opener);
  }
});

const memberGames = computed(() =>
  draft_query.value
    ? (galleryGroup(games.value, preview.groups, editing_id.value ?? '')
        ?.games ?? [])
    : games.value,
);
let opener: HTMLElement | null = null;

async function openEditor(
  group?: PreviewGroup,
  mode: 'manage' | 'rename' = 'manage',
  anchor?: HTMLElement | null,
) {
  editorMode.value = mode;
  opener =
    anchor ??
    (document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null);
  editing_id.value = group?.group_id;
  draft_name.value = group?.name ?? '';
  draft_members.value = [...(group?.game_ids ?? [])];
  draft_icon.value = group?.icon ?? (group?.smart ? 'spark' : 'games');
  draft_color.value = group?.color ?? '#80739b';
  draft_cover.value = group?.cover_url ?? '';
  draft_hidden.value = group?.hidden ?? false;
  draft_query.value = group?.query
    ? JSON.parse(JSON.stringify(group.query))
    : null;
  if (draft_query.value && !draft_query.value.filters)
    draft_query.value.filters = {};

  error.value = '';
  await nextTick();
  editor.value?.showModal();
  name_input.value?.focus();
}
async function restoreHiddenGroup(group: PreviewGroup) {
  if (saving.value) return;
  saving.value = true;
  try {
    const result = await savePreviewGroup(
      group.name,
      group.game_ids,
      group.group_id,
      {
        icon: group.icon,
        color: group.color,
        cover_url: group.cover_url,
        hidden: false,
        query: group.query,
      },
    );
    if (result) notify(result);
  } finally {
    saving.value = false;
  }
}
async function saveGroup() {
  if (saving.value) return;
  saving.value = true;
  error.value =
    (await savePreviewGroup(
      draft_name.value,
      draft_members.value,
      editing_id.value,
      {
        icon: draft_icon.value,
        color: draft_color.value,
        cover_url: draft_cover.value || null,
        hidden: draft_hidden.value,
        query: draft_query.value,
      },
    )) ?? '';
  saving.value = false;
  if (error.value) {
    name_input.value?.focus();
    return;
  }
  editor.value?.close();
}
async function restoreFocus() {
  await nextTick();
  if (opener?.isConnected) opener.focus();
  else create_button.value?.focus();
}
function toggleGroup(group_id: string) {
  if (collapsed_groups.value.has(group_id))
    collapsed_groups.value.delete(group_id);
  else collapsed_groups.value.add(group_id);
}
function onEditorKeydown(event: KeyboardEvent) {
  if (event.key !== 'Tab') return;
  const controls = Array.from(
    editor.value?.querySelectorAll<HTMLElement>(
      'button:not(:disabled), input:not(:disabled), select:not(:disabled)',
    ) ?? [],
  ).filter((element) => element.getClientRects().length > 0);
  const first = controls[0];
  const last = controls.at(-1);
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault();
    last?.focus();
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault();
    first?.focus();
  }
}
</script>

<template>
  <section
    id="sidebar-game-overview"
    class="sidebar-groups"
    aria-label="游戏总览"
  >
    <div v-if="expanded" class="overview-heading">
      <span
        >游戏总览 <span class="group-count">{{ games.length }}</span></span
      >
      <button
        ref="create_button"
        class="group-action"
        aria-label="新建分组"
        title="新建分组"
        @click="openEditor()"
      >
        <PreviewIcon name="plus" :size="17" />
      </button>
    </div>
    <details v-if="expanded && hiddenGroups.length" class="hidden-groups">
      <summary>已隐藏 {{ hiddenGroups.length }} 个分组</summary>
      <LibraryContextMenu
        v-for="group in hiddenGroups"
        :key="group.group_id"
        :label="`${group.name}分组操作`"
        :disabled="saving"
        @action="
          (action, anchor) => requestGroupAction(group.group_id, action, anchor)
        "
      >
        <div class="hidden-group-entry">
          <button
            type="button"
            class="group-open"
            :aria-label="`打开${group.name}分组`"
            @click="openGroup(group.group_id)"
          >
            {{ group.name }}
          </button>
          <button
            type="button"
            class="group-action restore-group"
            :disabled="saving"
            :aria-label="`恢复${group.name}分组显示`"
            @click="restoreHiddenGroup(group)"
          >
            恢复显示
          </button>
        </div>
      </LibraryContextMenu>
    </details>
    <div v-if="expanded" class="groups-scroll" aria-label="演示游戏分组">
      <section
        v-for="group in groups"
        :key="group.group_id"
        class="game-group"
        :data-game-drop-group="group.group_id"
        :class="{
          'is-group-dragging': drag_group === group.group_id,
          'is-group-sort-before':
            sortTarget === group.group_id &&
            sortPlacement === 'before' &&
            drag_group !== group.group_id,
          'is-group-sort-after':
            sortTarget === group.group_id &&
            sortPlacement === 'after' &&
            drag_group !== group.group_id,
          'is-game-drop-target':
            gameDrag?.group_id === group.group_id && gameDrag.allowed,
        }"
        :aria-label="`${group.name}分组`"
      >
        <LibraryContextMenu
          :label="`${group.name}分组操作`"
          :disabled="!group.editable || saving"
          @action="
            (action, anchor) =>
              requestGroupAction(group.group_id, action, anchor)
          "
        >
          <div
            class="group-heading"
            :data-group-sort-id="group.editable ? group.group_id : undefined"
            :draggable="false"
            @pointerdown="
              startGroupDrag($event, group.group_id, group.editable)
            "
            @dragstart.prevent
          >
            <button
              class="group-disclosure"
              :aria-expanded="!collapsed_groups.has(group.group_id)"
              :aria-controls="`group-${group.group_id}`"
              :aria-label="`${collapsed_groups.has(group.group_id) ? '展开' : '收起'}${group.name}分组`"
              :title="`${collapsed_groups.has(group.group_id) ? '展开' : '收起'}${group.name}分组`"
              @click="toggleGroup(group.group_id)"
            >
              <PreviewIcon
                name="back"
                :size="12"
                class="group-chevron"
                :class="{ folded: collapsed_groups.has(group.group_id) }"
              />
            </button>
            <button
              class="group-open"
              :class="{
                'is-selected':
                  route.name === 'games' &&
                  preview.gallery_state.collection_id === group.group_id,
              }"
              :aria-pressed="
                route.name === 'games' &&
                preview.gallery_state.collection_id === group.group_id
              "
              :aria-label="`打开${group.name}分组`"
              :title="`打开${group.name}分组`"
              @click="openGroup(group.group_id)"
            >
              <PreviewIcon
                :name="
                  group.group_id === 'favorites'
                    ? 'heart'
                    : (group.icon ?? 'games')
                "
                :style="{ color: group.color ?? undefined }"
                :size="14"
              />
              <span class="group-name">{{ group.name }}</span>
              <span class="group-count">{{ group.games.length }}</span>
            </button>
          </div>
        </LibraryContextMenu>
        <div
          v-show="!collapsed_groups.has(group.group_id)"
          :id="`group-${group.group_id}`"
          class="group-games"
        >
          <LibraryContextMenu
            v-for="game in group.games"
            :key="game.game_id"
            :label="`${game.title}作品操作`"
            game
            :removable="group.editable && !group.smart"
            :disabled="saving || gameDropBusy"
            @action="
              (action, anchor) =>
                requestGameAction(
                  game.game_id,
                  action,
                  anchor,
                  group.editable && !group.smart ? group.group_id : undefined,
                )
            "
          >
            <RouterLink
              class="group-game"
              :class="{ 'is-selected': active_game_id === game.game_id }"
              :to="{ name: 'game-detail', params: { game_id: game.game_id } }"
              :aria-label="`打开${game.title}详情`"
              :draggable="false"
              @pointerdown="
                armGameDrag($event, game.game_id, saving || gameDropBusy)
              "
              @dragstart.prevent
            >
              <PreviewCover
                class="group-cover"
                :cover_url="game.cover_url"
                :title="game.title"
                aria-hidden="true"
              />
              <span class="game-copy"
                ><span class="game-title">{{ game.title }}</span
                ><span class="game-status">{{
                  STATUS_LABELS[game.status]
                }}</span></span
              >
            </RouterLink>
          </LibraryContextMenu>
          <p v-if="!group.games.length" class="group-empty">
            {{
              group.group_id === 'favorites'
                ? '收藏的作品会出现在这里'
                : group.editable
                  ? preview.groups.some(
                      (item) => item.group_id === group.group_id && item.smart,
                    )
                    ? '暂无符合智能规则的作品'
                    : '拖入游戏，或右键组名管理成员'
                  : '暂无未分组作品'
            }}
          </p>
        </div>
      </section>
    </div>
    <div v-else class="groups-scroll compact-games" aria-label="演示游戏列表">
      <LibraryContextMenu
        v-for="game in games"
        :key="game.game_id"
        :label="`${game.title}作品操作`"
        game
        :disabled="saving || gameDropBusy"
        @action="
          (action, anchor) => requestGameAction(game.game_id, action, anchor)
        "
      >
        <RouterLink
          class="group-game"
          :class="{ 'is-selected': active_game_id === game.game_id }"
          :to="{ name: 'game-detail', params: { game_id: game.game_id } }"
          :aria-label="`打开${game.title}详情`"
          :title="game.title"
          :draggable="false"
          @pointerdown="
            armGameDrag($event, game.game_id, saving || gameDropBusy)
          "
          @dragstart.prevent
        >
          <PreviewCover
            class="group-cover"
            :cover_url="game.cover_url"
            :title="game.title"
            aria-hidden="true"
          />
        </RouterLink>
      </LibraryContextMenu>
    </div>
  </section>
  <SidebarGameActions />
  <Teleport to="body">
    <dialog
      ref="editor"
      class="group-editor"
      aria-labelledby="group-editor-title"
      @keydown.stop="onEditorKeydown"
      @close="restoreFocus"
    >
      <form @submit.prevent="saveGroup">
        <header class="editor-heading">
          <h2 id="group-editor-title">
            {{
              editorMode === 'rename'
                ? '重命名分组'
                : editing_id
                  ? '管理分组'
                  : '新建分组'
            }}
          </h2>
          <button
            type="button"
            class="group-action"
            aria-label="关闭分组编辑器"
            @click="editor?.close()"
          >
            <PreviewIcon name="close" />
          </button>
        </header>
        <label class="name-label" for="group-name-input">分组名称</label>
        <input
          id="group-name-input"
          ref="name_input"
          v-model="draft_name"
          maxlength="24"
          placeholder="例如：正在游玩、计划补完"
          :aria-invalid="Boolean(error)"
          :aria-describedby="error ? 'group-name-error' : undefined"
          @input="error = ''"
        />
        <p v-if="error" id="group-name-error" class="editor-error" role="alert">
          {{ error }}
        </p>
        <template v-if="editorMode === 'manage'">
          <div class="members-heading">
            分组作品
            <span class="group-count"
              >已选
              {{
                draft_query ? memberGames.length : draft_members.length
              }}</span
            >
          </div>
          <div class="member-list">
            <label
              v-for="game in memberGames"
              :key="game.game_id"
              class="member-option"
              ><input
                v-if="draft_query"
                type="checkbox"
                checked
                disabled
                :aria-label="`${game.title}（按规则归类）`" /><input
                v-else
                v-model="draft_members"
                type="checkbox"
                :disabled="saving"
                :value="game.game_id" /><span>{{ game.title }}</span
              ><PreviewIcon
                v-if="game.favorite"
                name="heart"
                :size="13"
                aria-label="已收藏"
            /></label>
            <p v-if="!memberGames.length" class="group-empty">
              {{
                draft_query ? '暂无符合分组规则的作品。' : '当前没有可选作品。'
              }}
            </p>
          </div>
        </template>
        <footer class="editor-actions">
          <span class="action-spacer"></span>
          <button type="button" class="editor-button" @click="editor?.close()">
            取消
          </button>
          <button
            type="submit"
            class="editor-button primary"
            :disabled="saving"
          >
            {{
              saving
                ? '保存中…'
                : editorMode === 'rename'
                  ? '保存名称'
                  : '保存分组'
            }}
          </button>
        </footer>
      </form>
    </dialog>
  </Teleport>
</template>

<style scoped>
.sidebar-groups {
  display: flex;
  width: 100%;
  flex: 1;
  flex-direction: column;
  min-height: 0;
  margin-top: var(--space-24);
  padding-top: var(--space-16);
  border-top: 1px solid var(--border);
}
.overview-heading,
.group-heading,
.group-disclosure,
.group-open,
.editor-heading,
.members-heading,
.editor-actions {
  display: flex;
  align-items: center;
}
.overview-heading {
  justify-content: space-between;
  padding: 0 var(--space-8) var(--space-12);
  font-size: var(--type-small);
  color: var(--muted);
}
.group-count {
  color: var(--subtle);
  font-size: var(--type-small);
  font-variant-numeric: tabular-nums;
  margin-left: var(--space-8);
}
.groups-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  overscroll-behavior: contain;
  scrollbar-width: thin;
}
.hidden-groups {
  margin: 0 var(--space-8) var(--space-12);
  color: var(--muted);
  font-size: var(--type-small);
}
.hidden-groups summary {
  padding: var(--space-4) 0;
  cursor: pointer;
}
.hidden-groups .group-open {
  width: 100%;
}
.hidden-group-entry {
  display: flex;
  align-items: center;
}
.restore-group {
  width: auto;
  padding: 0 var(--space-8);
  white-space: nowrap;
  font-size: 11px;
}
.game-group + .game-group {
  margin-top: var(--space-16);
}
.game-group {
  border-radius: var(--radius-sm);
  transition:
    background var(--micro-duration) var(--ease-standard),
    outline-color var(--micro-duration) var(--ease-standard);
}
.game-group.is-group-dragging {
  opacity: 0.5;
}
.game-group.is-group-sort-before {
  box-shadow: 0 -2px var(--accent);
}
.game-group.is-group-sort-after {
  box-shadow: 0 2px var(--accent);
}
.game-group.is-game-drop-target {
  outline: 2px solid var(--accent);
  outline-offset: -2px;
  background: var(--surface-hover);
}
.group-heading {
  gap: var(--space-4);
  margin-bottom: var(--space-4);
}
.group-open {
  flex: 1;
  min-width: 0;
  gap: var(--space-8);
  min-height: 32px;
  padding: var(--space-4) var(--space-8);
  border: 0;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--muted);
  text-align: left;
}
.group-open:hover,
.group-disclosure:hover,
.group-action:hover {
  background: var(--surface-hover);
  color: var(--text);
}
.group-disclosure {
  display: inline-flex;
  flex-shrink: 0;
  justify-content: center;
  width: 28px;
  height: 32px;
  padding: 0;
  border: 0;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--muted);
}
.group-open.is-selected {
  color: var(--accent-ink, var(--accent));
  background: var(--surface-hover);
}
.group-name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--type-small);
}
.group-chevron {
  flex-shrink: 0;
  transform: rotate(-90deg);
}
.group-chevron.folded {
  transform: rotate(180deg);
}
.group-action {
  display: inline-flex;
  flex-shrink: 0;
  align-items: center;
  justify-content: center;
  width: 30px;
  height: 30px;
  border: 0;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--subtle);
}
.group-action:disabled {
  opacity: 0.35;
  cursor: not-allowed;
}
.group-game {
  content-visibility: auto;
  contain-intrinsic-size: auto 54px;
  display: flex;
  align-items: center;
  gap: var(--space-12);
  min-height: 54px;
  padding: var(--space-8);
  margin: 2px 0;
  border-radius: var(--radius-sm);
  color: var(--muted);
  text-decoration: none;
}
.group-game:hover {
  color: var(--text);
  background: var(--surface-hover);
}
.group-game.is-selected {
  color: var(--text);
  background: var(--accent-wash);
  box-shadow: inset 2px 0 var(--accent);
}
.group-game.is-selected::before {
  display: none;
}
.group-cover {
  flex-shrink: 0;
  width: 28px;
  height: 38px;
  overflow: hidden;
  border-radius: var(--radius-sm);
}
.game-copy {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  min-width: 0;
}
.game-title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--type-small);
  font-weight: 500;
}
.game-status {
  color: var(--subtle);
  font-size: var(--type-small);
}
.group-empty {
  margin: var(--space-8);
  font-size: var(--type-small);
  color: var(--subtle);
  line-height: 1.6;
}
.compact-games {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-4);
}
.compact-games .group-game {
  flex-shrink: 0;
  justify-content: center;
  width: 52px;
  padding: var(--space-4);
}
.group-editor {
  margin: auto;
  width: min(480px, calc(100vw - 48px));
  max-height: calc(100dvh - 48px);
  padding: var(--space-24);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  background: var(--surface);
  color: var(--text);
  box-shadow: var(--shadow-modal);
}
.group-editor::backdrop {
  background: rgb(0 0 0 / 48%);
  backdrop-filter: blur(8px);
}
.editor-heading {
  margin-bottom: var(--space-20);
  justify-content: space-between;
  gap: var(--space-16);
}
.editor-heading h2 {
  margin: 0;
  font-size: 20px;
}
.name-label {
  display: block;
  margin-bottom: var(--space-8);
  font-size: var(--type-small);
}
#group-name-input {
  width: 100%;
  min-height: 42px;
  padding: var(--space-8) var(--space-12);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--background);
  color: var(--text);
}
.editor-error {
  color: var(--danger);
  font-size: var(--type-small);
  margin: var(--space-8) 0;
}
.members-heading {
  justify-content: space-between;
  margin-top: var(--space-20);
  font-size: var(--type-small);
}
.member-list {
  max-height: 240px;
  overflow-y: auto;
  margin: var(--space-8) 0 var(--space-20);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
}
.member-option {
  display: flex;
  align-items: center;
  gap: var(--space-12);
  min-height: 40px;
  padding: var(--space-8) var(--space-12);
  font-size: var(--type-small);
  cursor: pointer;
}
.member-option:hover {
  background: var(--surface-hover);
}
.member-option input {
  accent-color: var(--accent-ink, var(--accent));
}
.member-option span {
  flex: 1;
}
.member-option svg {
  color: var(--accent-ink, var(--accent));
}
.editor-actions {
  gap: var(--space-8);
  flex-wrap: wrap;
}
.action-spacer {
  flex: 1;
}
.editor-button {
  min-height: 36px;
  padding: var(--space-8) var(--space-12);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text);
  font-size: var(--type-small);
}
.editor-button.primary {
  background: var(--accent);
  border-color: var(--accent-ink, var(--accent));
  color: var(--accent-text);
}
</style>
