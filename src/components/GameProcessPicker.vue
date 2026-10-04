<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { api, errorText } from '../stores/library';
import type { RunningProcess } from '../types/detailWorkspace';
import PreviewIcon from './preview/PreviewIcon.vue';
const props = defineProps<{ installId: string; disabled?: boolean }>();
const emit = defineEmits<{
  selected: [process: RunningProcess, applied: boolean];
}>();
const dialog = ref<HTMLDialogElement>();
const items = ref<RunningProcess[]>([]),
  active = ref(false),
  busy = ref(false),
  selecting = ref(false),
  error = ref(''),
  filter = ref('');
let generation = 0,
  opener: HTMLElement | null = null;
function close() {
  if (selecting.value) return;
  generation++;
  busy.value = false;
  dialog.value?.close();
  if (opener?.isConnected) opener.focus({ preventScroll: true });
}
async function refresh() {
  const request = ++generation,
    id = props.installId;
  busy.value = true;
  error.value = '';
  try {
    const result = await api('list_game_processes', { install_id: id });
    if (request === generation && id === props.installId) {
      items.value = result.items;
      active.value = result.active_session;
    }
  } catch (e) {
    if (request === generation) error.value = errorText(e);
  } finally {
    if (request === generation) busy.value = false;
  }
}
async function open() {
  if (props.disabled || !props.installId) return;
  opener =
    document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null;
  items.value = [];
  filter.value = '';
  error.value = '';
  await nextTick();
  dialog.value?.showModal();
  void refresh();
}
async function choose(process: RunningProcess) {
  if (selecting.value || busy.value) return;
  const id = props.installId,
    request = generation;
  selecting.value = true;
  error.value = '';
  try {
    const selected = active.value
      ? await api('select_game_process', {
          install_id: id,
          pid: process.pid,
          created_at_ticks: process.created_at_ticks,
        })
      : process;
    if (id !== props.installId || request !== generation) return;
    emit('selected', selected, active.value);
    selecting.value = false;
    close();
  } catch (e) {
    if (id === props.installId && request === generation)
      error.value = errorText(e);
  } finally {
    selecting.value = false;
  }
}
watch(
  () => props.installId,
  () => {
    selecting.value = false;
    close();
    items.value = [];
  },
);
onBeforeUnmount(() => {
  generation++;
  dialog.value?.close();
});
</script>
<template>
  <button
    class="secondary-button"
    type="button"
    :disabled="disabled || !installId"
    @click="open"
  >
    <PreviewIcon name="games" :size="16" />选择运行中进程
  </button>
  <Teleport to="body"
    ><dialog
      ref="dialog"
      class="workspace-process-picker"
      aria-label="选择 Windows 主进程"
      @cancel.prevent="close"
    >
      <header>
        <div>
          <p class="eyebrow">WINDOWS PROCESS</p>
          <h2>选择主游戏进程</h2>
        </div>
        <button
          class="icon-button"
          aria-label="关闭进程选择"
          :disabled="selecting"
          @click="close"
        >
          <PreviewIcon name="close" :size="20" />
        </button>
      </header>
      <p>
        {{
          active
            ? '当前会话正在运行，选择后立即切换跟踪对象。'
            : '当前没有运行中的会话，选择后可保存为下次启动配置。'
        }}
      </p>
      <div class="workspace-process-search">
        <input
          v-model="filter"
          aria-label="筛选进程"
          placeholder="搜索进程名或 PID"
        /><button
          class="quiet-button"
          :disabled="busy || selecting"
          @click="refresh"
        >
          {{ busy ? '读取中…' : '刷新列表' }}
        </button>
      </div>
      <p v-if="error" class="workspace-error" role="alert">{{ error }}</p>
      <p v-if="!busy && !items.length && !error" class="workspace-empty">
        没有发现可选择的游戏进程。请启动游戏后刷新；列表仅包含此安装目录内的游戏进程。
      </p>
      <ul class="workspace-process-list" aria-label="Windows 游戏进程">
        <li
          v-for="process in items.filter((p) =>
            `${p.name} ${p.pid}`.toLowerCase().includes(filter.toLowerCase()),
          )"
          :key="`${process.pid}:${process.created_at_ticks}`"
        >
          <button :disabled="busy || selecting" @click="choose(process)">
            <span class="workspace-process-icon"
              ><PreviewIcon name="play" :size="18" /></span
            ><span
              ><strong>{{ process.name }}</strong
              ><small>{{ process.path }}</small></span
            ><code>PID {{ process.pid }}</code
            ><PreviewIcon name="arrow" :size="16" />
          </button>
        </li>
      </ul>
      <p class="workspace-hint">
        读取 Windows 系统进程列表。进程退出后不会继续计时。
      </p>
    </dialog></Teleport
  >
</template>
