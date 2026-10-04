<script setup lang="ts">
import { computed, nextTick, onUnmounted, ref, watch } from 'vue';
import { ask } from '@tauri-apps/plugin-dialog';
import { api, desktop, errorText, notify } from '../stores/library';
import type { Screenshot } from '../types/domain';
import { screenshotResource } from '../services/screenshotResource';
const props = defineProps<{ gameId: string }>();
const items = ref<Screenshot[]>([]);
const busy = ref(false);
const acting = ref(false);
const error = ref('');
const selecting = ref(false);
const checked = ref(new Set<string>());
const selected = ref<Screenshot | null>(null);
const viewer = ref<HTMLDialogElement>();
const zoom = ref(1);
const imageFailed = ref(false);
const thumbnailFailures = ref(new Set<string>());
const previewFailures = ref(new Set<string>());
const index = computed(() =>
  items.value.findIndex((item) => item.id === selected.value?.id),
);
let generation = 0;
function previewSource(item: Screenshot) {
  return screenshotResource(
    thumbnailFailures.value.has(item.thumbnail_url)
      ? item.image_url
      : item.thumbnail_url,
  );
}
function previewError(item: Screenshot) {
  if (
    item.thumbnail_url === item.image_url ||
    thumbnailFailures.value.has(item.thumbnail_url)
  )
    previewFailures.value.add(item.image_url);
  else thumbnailFailures.value.add(item.thumbnail_url);
}
async function open(item: Screenshot) {
  if (selecting.value) {
    toggle(item.id);
    return;
  }
  selected.value = item;
  zoom.value = 1;
  imageFailed.value = false;
  await nextTick();
  if (!viewer.value?.open) viewer.value?.showModal();
}
function toggle(id: string) {
  if (checked.value.has(id)) checked.value.delete(id);
  else checked.value.add(id);
}
function close() {
  viewer.value?.close();
  selected.value = null;
}
function navigate(offset: number) {
  const item = items.value[index.value + offset];
  if (item) void open(item);
}
function viewerKey(event: KeyboardEvent) {
  if (event.key === 'ArrowLeft') navigate(-1);
  if (event.key === 'ArrowRight') navigate(1);
}
async function refresh() {
  if (!desktop) return;
  const current = generation;
  busy.value = true;
  error.value = '';
  try {
    const result = await api('list_screenshots', {
      game_id: props.gameId,
      include_spoilers: true,
    });
    if (current === generation) {
      items.value = result;
      checked.value = new Set(
        [...checked.value].filter((id) =>
          result.some((item) => item.id === id),
        ),
      );
    }
  } catch (cause) {
    if (current === generation) error.value = errorText(cause);
  } finally {
    if (current === generation) busy.value = false;
  }
}
async function save(ids: string[], batch: boolean) {
  if (!ids.length || acting.value) return;
  acting.value = true;
  error.value = '';
  try {
    const result = await api('export_screenshots', {
      game_id: props.gameId,
      screenshot_ids: ids,
      batch,
    });
    if (result.cancelled) return;
    if (result.completed) notify(`已保存 ${result.completed} 张原图。`);
    error.value = result.failures.join('；');
  } catch (cause) {
    error.value = errorText(cause);
  } finally {
    acting.value = false;
  }
}
async function remove(ids: string[]) {
  if (!ids.length || acting.value) return;
  acting.value = true;
  error.value = '';
  try {
    if (
      !(await ask(
        `确定删除选中的 ${ids.length} 张截图吗？这会删除磁盘原图及记录，程序会先创建安全备份。`,
        {
          title: '删除截图原图',
          kind: 'warning',
          okLabel: '删除',
          cancelLabel: '保留',
        },
      ))
    )
      return;
    const result = await api('delete_screenshots', {
      game_id: props.gameId,
      screenshot_ids: ids,
      confirmed: true,
    });
    if (result.completed) {
      notify(`已备份并删除 ${result.completed} 张截图。`);
      close();
      await refresh();
    }
    error.value = result.failures.join('；');
  } catch (cause) {
    error.value = errorText(cause);
  } finally {
    acting.value = false;
  }
}
watch(
  () => props.gameId,
  () => {
    generation++;
    close();
    selecting.value = false;
    checked.value.clear();
    thumbnailFailures.value.clear();
    previewFailures.value.clear();
    void refresh();
  },
  { immediate: true },
);
onUnmounted(() => {
  generation++;
  viewer.value?.close();
});
</script>
<template>
  <section
    v-if="desktop"
    class="personal-panel screenshots-panel"
    aria-label="作品截图"
  >
    <div class="gallery-toolbar">
      <span>{{ items.length }} 张</span
      ><button
        class="secondary-button"
        :disabled="acting"
        @click="
          selecting = !selecting;
          checked.clear();
        "
      >
        {{ selecting ? '退出选择' : '批量选择' }}
      </button>
      <template v-if="selecting"
        ><span>已选择 {{ checked.size }} 张</span
        ><button
          class="secondary-button"
          :disabled="!checked.size || acting"
          @click="save([...checked], true)"
        >
          保存</button
        ><button
          class="secondary-button delete-button"
          :disabled="!checked.size || acting"
          @click="remove([...checked])"
        >
          删除
        </button></template
      >
    </div>
    <p v-if="error" class="personal-error" role="alert">{{ error }}</p>
    <p v-if="busy" role="status">正在读取图片…</p>
    <p v-else-if="!items.length" class="muted">
      还没有截图。请前往“资料”扫描游戏目录。
    </p>
    <div class="screenshot-grid">
      <button
        v-for="(item, position) in items"
        :key="item.id"
        class="screenshot-card"
        :class="{ 'is-selected': checked.has(item.id) }"
        :aria-label="`查看图片 ${position + 1}`"
        :aria-pressed="selecting ? checked.has(item.id) : undefined"
        :disabled="acting"
        @click="open(item)"
      >
        <img
          v-if="!previewFailures.has(item.image_url)"
          :src="previewSource(item)"
          alt="游戏画面"
          loading="lazy"
          @error="previewError(item)"
        />
        <div v-else class="screenshot-unavailable">
          预览无法读取，请重新扫描
        </div>
        <span v-if="selecting" class="selection-check" aria-hidden="true">{{
          checked.has(item.id) ? '✓' : '○'
        }}</span>
      </button>
    </div>
    <dialog
      ref="viewer"
      class="screenshot-lightbox"
      aria-label="查看图片"
      @close="selected = null"
      @click="$event.target === viewer && close()"
      @keydown="viewerKey"
    >
      <template v-if="selected"
        ><div class="viewer-toolbar">
          <div class="viewer-navigation">
            <button
              class="quiet-button"
              :disabled="index <= 0"
              aria-label="上一张"
              @click="navigate(-1)"
            >
              ‹</button
            ><span>{{ index + 1 }} / {{ items.length }}</span
            ><button
              class="quiet-button"
              :disabled="index >= items.length - 1"
              aria-label="下一张"
              @click="navigate(1)"
            >
              ›</button
            ><button
              class="quiet-button"
              :disabled="zoom <= 0.5"
              aria-label="缩小"
              @click="zoom = Math.max(0.5, zoom - 0.25)"
            >
              −</button
            ><button
              class="quiet-button"
              aria-label="恢复比例"
              @click="zoom = 1"
            >
              {{ Math.round(zoom * 100) }}%</button
            ><button
              class="quiet-button"
              :disabled="zoom >= 3"
              aria-label="放大"
              @click="zoom = Math.min(3, zoom + 0.25)"
            >
              +
            </button>
          </div>
          <button class="quiet-button" aria-label="关闭图片" @click="close">
            ×
          </button>
        </div>
        <div class="viewer-image">
          <img
            v-if="!imageFailed"
            :src="screenshotResource(selected.image_url)"
            alt="游戏画面"
            :style="{ width: `${zoom * 100}%`, maxWidth: 'none' }"
            @error="imageFailed = true"
          />
          <p v-else>原图不可读取，请重新扫描。</p>
        </div>
        <p v-if="error" class="personal-error" role="alert">{{ error }}</p>
        <div class="viewer-actions">
          <button
            class="secondary-button"
            :disabled="acting"
            @click="save([selected.id], false)"
          >
            保存</button
          ><button
            class="secondary-button delete-button"
            :disabled="acting"
            @click="remove([selected.id])"
          >
            删除
          </button>
        </div>
      </template>
    </dialog>
  </section>
</template>
<style scoped>
.screenshots-panel {
  width: 100%;
  min-width: 0;
}
.gallery-toolbar,
.viewer-toolbar,
.viewer-actions,
.viewer-navigation {
  display: flex;
  gap: 12px;
  align-items: center;
  flex-wrap: wrap;
}
.gallery-toolbar {
  margin-bottom: 20px;
}
.gallery-toolbar > span:first-child {
  margin-right: auto;
  color: var(--muted);
}
.screenshot-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(min(220px, 100%), 1fr));
  gap: 18px;
}
.screenshot-card {
  position: relative;
  aspect-ratio: 16/10;
  overflow: hidden;
  padding: 0;
  border: 2px solid transparent;
  border-radius: var(--radius-md);
  background: var(--surface-hover);
  cursor: pointer;
}
.screenshot-card img {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: cover;
  transition: transform 0.2s;
}
.screenshot-card:hover img {
  transform: scale(1.03);
}
.screenshot-card.is-selected {
  border-color: var(--accent-ink, var(--accent));
}
.selection-check {
  position: absolute;
  top: 10px;
  right: 10px;
  background: var(--accent);
  color: var(--accent-text);
  border-radius: 50%;
  width: 28px;
  height: 28px;
  display: grid;
  place-items: center;
}
.screenshot-unavailable {
  padding: 20px;
  color: var(--muted);
}
.delete-button {
  color: var(--danger);
  border-color: var(--danger);
}
.screenshot-lightbox {
  width: min(1120px, 94vw);
  max-height: 92dvh;
  padding: 20px;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-lg);
  background: var(--surface);
  color: var(--text);
}
.screenshot-lightbox::backdrop {
  background: rgb(0 0 0 / 66%);
}
.viewer-toolbar {
  justify-content: space-between;
  margin-bottom: 12px;
}
.viewer-image {
  max-height: 65dvh;
  overflow: auto;
  text-align: center;
}
.viewer-image img {
  display: block;
  height: auto;
}
.viewer-actions {
  justify-content: flex-end;
  margin-top: 16px;
}
.personal-error {
  color: var(--danger);
  overflow-wrap: anywhere;
}
@media (prefers-reduced-motion: reduce) {
  .screenshot-card img {
    transition: none;
  }
}
</style>
