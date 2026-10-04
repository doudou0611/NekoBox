<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import { gameActionRequest } from '../services/libraryContextActions';
import {
  addLibraryGamesToGroup,
  removeLibraryGamesFromGroup,
  removeLibraryGames,
} from '../services/gameBatch';
import {
  preview,
  desktop,
  updateLibraryGame,
  notify,
  errorText,
} from '../stores/library';

const router = useRouter();
const dialog = ref<HTMLDialogElement>();
const field = ref<HTMLInputElement | HTMLSelectElement>();
const mode = ref<'rename' | 'add'>('rename');
const gameId = ref('');
const title = ref('');
const groupId = ref('');
const error = ref('');
const busy = ref(false);
const groups = computed(() =>
  preview.groups.filter((group) => !group.smart && !group.hidden),
);
let opener: HTMLElement | null = null;
function restoreFocus() {
  if (opener?.isConnected) opener.focus({ preventScroll: true });
}
watch(gameActionRequest, async (request) => {
  if (!request) return;
  gameActionRequest.value = null;
  if (busy.value) return;
  const game = preview.games.find((game) => game.game_id === request.game_id);
  if (!game) return;
  opener = request.opener;
  if (request.action === 'manage') {
    await router.push({
      name: 'game-detail',
      params: { game_id: game.game_id },
      query: { panel: 'information' },
    });
  } else if (request.action === 'delete') {
    const id = game.game_id;
    preview.dialog = {
      title: `删除“${game.title}”的库记录？`,
      description: desktop
        ? '先保存数据库安全快照，再移除库记录。游戏、存档和备份文件保留。'
        : '仅移除演示作品及其分组关系，不访问本地文件。',
      confirm_label: '确认删除',
      opener: () => request.opener,
      action: () => {
        void removeLibraryGames([id]);
      },
    };
  } else if (request.action === 'remove_from_group' && request.group_id) {
    busy.value = true;
    try {
      const result = await removeLibraryGamesFromGroup(request.group_id, [
        game.game_id,
      ]);
      if (result) notify(result);
    } finally {
      busy.value = false;
    }
  } else if (request.action === 'rename' || request.action === 'add_to_group') {
    mode.value = request.action === 'rename' ? 'rename' : 'add';
    gameId.value = game.game_id;
    title.value = game.title;
    groupId.value = groups.value[0]?.group_id ?? '';
    error.value = '';
    await nextTick();
    dialog.value?.showModal();
    field.value?.focus();
    if (field.value instanceof HTMLInputElement) field.value.select();
  }
});
async function save() {
  if (busy.value) return;
  const game = preview.games.find((game) => game.game_id === gameId.value);
  if (!game) {
    error.value = '作品已不存在。';
    return;
  }
  if (
    mode.value === 'rename' &&
    (!title.value.trim() || [...title.value.trim()].length > 200)
  ) {
    error.value = '作品名称需要 1～200 个字符。';
    return;
  }
  if (
    mode.value === 'add' &&
    !groups.value.some((group) => group.group_id === groupId.value)
  ) {
    error.value = '请选择可添加的分组。';
    return;
  }
  busy.value = true;
  error.value = '';
  try {
    if (mode.value === 'add') {
      error.value =
        (await addLibraryGamesToGroup(groupId.value, [gameId.value])) ?? '';
    } else {
      if (desktop) {
        if (
          !(await updateLibraryGame(gameId.value, () => ({
            title: title.value.trim(),
          })))
        ) {
          error.value = '名称未保存，请检查错误提示后重试。';
          return;
        }
      } else game.title = title.value.trim();
      notify(`${desktop ? '' : '演示：'}作品名称已保存。`);
    }
    if (!error.value) dialog.value?.close();
  } catch (cause) {
    error.value = errorText(cause);
  } finally {
    busy.value = false;
  }
}
</script>
<template>
  <Teleport to="body">
    <dialog
      ref="dialog"
      class="sidebar-game-editor"
      :aria-label="mode === 'rename' ? '重命名作品' : '添加到分组'"
      @close="restoreFocus"
      @cancel="busy && $event.preventDefault()"
    >
      <form @submit.prevent="save">
        <h2>{{ mode === 'rename' ? '重命名作品' : '添加到分组' }}</h2>
        <label v-if="mode === 'rename'"
          >作品名称<input
            ref="field"
            v-model="title"
            maxlength="200"
            :disabled="busy"
        /></label>
        <label v-else-if="groups.length"
          >目标分组<select ref="field" v-model="groupId" :disabled="busy">
            <option
              v-for="group in groups"
              :key="group.group_id"
              :value="group.group_id"
            >
              {{ group.name }}
            </option>
          </select></label
        >
        <p v-else>暂无可添加的普通分组，请先新建分组。</p>
        <p v-if="error" role="alert">{{ error }}</p>
        <footer>
          <button
            type="button"
            class="quiet-button"
            :disabled="busy"
            @click="dialog?.close()"
          >
            取消</button
          ><button
            class="secondary-button"
            :disabled="busy || (mode === 'add' && !groups.length)"
          >
            {{
              busy ? '保存中…' : mode === 'rename' ? '保存名称' : '添加到分组'
            }}
          </button>
        </footer>
      </form>
    </dialog>
  </Teleport>
</template>
<style scoped>
.sidebar-game-editor {
  width: min(440px, calc(100vw - 40px));
  max-height: calc(100dvh - 48px);
  overflow: auto;
  padding: 28px;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-lg);
  background: var(--surface);
  color: var(--text);
}
.sidebar-game-editor::backdrop {
  background: #0005;
  backdrop-filter: blur(8px);
}
h2 {
  margin: 0 0 24px;
}
label {
  display: grid;
  gap: 10px;
}
input,
select {
  width: 100%;
  padding: 12px;
  background: var(--surface-hover);
  color: var(--text);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
}
footer {
  display: flex;
  justify-content: flex-end;
  gap: 12px;
  margin-top: 24px;
}
[role='alert'] {
  color: var(--danger);
}
</style>
