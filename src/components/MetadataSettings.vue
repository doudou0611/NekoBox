<script setup lang="ts">
import SettingsSection from './settings/SettingsSection.vue';
import { onMounted, ref } from 'vue';
import SettingsSwitch from './settings/SettingsSwitch.vue';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { api, desktop, errorText, notify } from '../stores/library';
import {
  metadataSources,
  loadMetadataSources,
  saveMetadataSources,
  type MetadataSource,
} from '../stores/metadataSources';
const dragging = ref<number | null>(null);
const dragTarget = ref<number | null>(null);
const sourceBusy = ref(false);
function startSourceDrag(event: PointerEvent, index: number) {
  if (sourceBusy.value || event.button !== 0) return;
  event.preventDefault();
  dragging.value = index;
  dragTarget.value = index;
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
}
function moveSourceDrag(event: PointerEvent) {
  if (dragging.value === null) return;
  const row = document
    .elementFromPoint(event.clientX, event.clientY)
    ?.closest<HTMLElement>('.metadata-source-order li');
  if (row) dragTarget.value = Number(row.dataset.sourceIndex);
}
function resetSourceDrag() {
  dragging.value = null;
  dragTarget.value = null;
}
function finishSourceDrag(event: PointerEvent) {
  const from = dragging.value;
  const to = dragTarget.value;
  const handle = event.currentTarget as HTMLElement;
  if (handle.hasPointerCapture(event.pointerId))
    handle.releasePointerCapture(event.pointerId);
  resetSourceDrag();
  if (from !== null && to !== null) reorder(from, to);
}
async function changeSources(sources: MetadataSource[]) {
  if (sourceBusy.value) return;
  sourceBusy.value = true;
  error.value = '';
  try {
    await saveMetadataSources(sources);
  } catch (cause) {
    error.value = errorText(cause);
  } finally {
    sourceBusy.value = false;
  }
}
function toggleSource(index: number) {
  const sources = metadataSources.sources.map((s, i) => ({
    ...s,
    enabled: i === index ? !s.enabled : s.enabled,
  }));
  void changeSources(sources);
}
function reorder(from: number, to: number) {
  if (from === to) return;
  const sources = [...metadataSources.sources];
  const item = sources.splice(from, 1)[0];
  if (!item) return;
  sources.splice(to, 0, item);
  void changeSources(sources);
}
function dropSource(to: number) {
  if (dragging.value !== null) reorder(dragging.value, to);
  dragging.value = null;
}
const token = ref('');
const has_token = ref(false);
const clear = ref(false);
const busy = ref(false);
const loaded = ref(false);
const error = ref('');
const result = ref('');
function openVndbTokenPage() {
  if (!desktop) return;
  const window = new WebviewWindow(`vndb-token-${crypto.randomUUID()}`, {
    url: 'https://vndb.org/u/tokens',
    title: 'VNDB API Token',
    width: 900,
    height: 720,
  });
  void window.once('tauri://error', () => {
    error.value = '无法打开 VNDB 官方 Token 页面，请稍后重试。';
  });
}
async function load() {
  if (!desktop) return;
  busy.value = true;
  error.value = '';
  try {
    has_token.value = (await api('get_vndb_settings', {})).has_api_token;
    loaded.value = true;
  } catch (cause) {
    error.value = errorText(cause);
  } finally {
    busy.value = false;
  }
}
async function save(test = false) {
  if (!desktop || busy.value || !loaded.value) return;
  busy.value = true;
  error.value = '';
  result.value = '';
  try {
    has_token.value = (
      await api('save_vndb_settings', {
        api_token: token.value.trim() || null,
        clear_api_token: clear.value,
      })
    ).has_api_token;
    token.value = '';
    clear.value = false;
    if (test) {
      const account = await api('test_vndb_connection', {});
      result.value = `VNDB 授权有效 · ${account.username}`;
    }
    notify(test ? 'VNDB 授权验证成功。' : '元数据设置已保存。');
  } catch (cause) {
    error.value = errorText(cause);
  } finally {
    busy.value = false;
  }
}
async function testHikarinagi() {
  if (!desktop || busy.value) return;
  busy.value = true;
  error.value = '';
  result.value = '';
  try {
    if (await api('test_hikarinagi_connection', {}))
      result.value = 'Hikarinagi 元数据连接成功。';
  } catch (cause) {
    error.value = errorText(cause);
  } finally {
    busy.value = false;
  }
}
onMounted(() => {
  void load();
  void loadMetadataSources().catch((cause) => (error.value = errorText(cause)));
});
</script>
<template>
  <SettingsSection
    id="metadata-settings"
    title="来源顺序与授权"
    description="按优先顺序获取资料，管理刮削源开关与个人 API 授权。"
  >
    <form class="settings-card" @submit.prevent="save()">
      <div class="source-summary">
        <h4 class="settings-field-title">来源顺序与启用状态</h4>
        <p class="settings-note">
          按下列顺序优先获取资料，后续来源补充缺失字段；关闭的来源不会参与刮削。设置立即保存，仅影响下一次任务。
        </p>
        <button
          class="quiet-button"
          type="button"
          :disabled="!desktop || busy"
          @click="testHikarinagi"
        >
          测试 Hikarinagi 连接
        </button>
      </div>

      <ol class="metadata-source-order" aria-label="刮削源顺序">
        <li
          v-for="(source, index) in metadataSources.sources"
          :key="source.provider"
          :data-source-index="index"
          :class="{ 'drag-target': dragTarget === index && dragging !== index }"
          :draggable="!sourceBusy"
          @dragstart="dragging = index"
          @dragend="dragging = null"
          @dragover.prevent
          @drop.prevent="dropSource(index)"
        >
          <span
            class="drag-handle"
            aria-label="拖动排序"
            @pointerdown="startSourceDrag($event, index)"
            @pointermove="moveSourceDrag"
            @pointerup="finishSourceDrag"
            @pointercancel="resetSourceDrag"
            @lostpointercapture="resetSourceDrag"
            >⠿</span
          ><span
            >{{ index + 1 }} ·
            {{
              source.provider === 'hikarinagi'
                ? 'Hikarinagi'
                : source.provider === 'bangumi'
                  ? 'Bangumi'
                  : 'VNDB'
            }}</span
          >
          <button
            type="button"
            class="quiet-button"
            :disabled="sourceBusy || index === 0"
            :aria-label="`${source.provider} 上移`"
            @click="reorder(index, index - 1)"
          >
            ↑
          </button>
          <button
            type="button"
            class="quiet-button"
            :disabled="sourceBusy || index === 2"
            :aria-label="`${source.provider} 下移`"
            @click="reorder(index, index + 1)"
          >
            ↓
          </button>
          <button
            type="button"
            class="source-switch"
            role="switch"
            :aria-checked="source.enabled"
            :aria-label="`启用 ${source.provider}`"
            :disabled="sourceBusy"
            @click="toggleSource(index)"
          >
            {{ source.enabled ? '已启用' : '已关闭' }}
          </button>
        </li>
      </ol>
      <label class="preference-field"
        >VNDB API Token<input
          v-model="token"
          type="password"
          aria-label="VNDB API Token"
          maxlength="8192"
          autocomplete="new-password"
          spellcheck="false"
          :placeholder="
            has_token
              ? '已保存，留空保留'
              : '可选：填写自己 VNDB 账户的 API Token'
          "
          :disabled="!loaded || busy || clear"
        /><small
          >接口：api.vndb.org/kana。未填写时使用匿名公共资料接口；Token
          仅保存在本机系统凭据库。</small
        ></label
      >
      <button
        class="quiet-button"
        type="button"
        :disabled="!desktop"
        @click="openVndbTokenPage"
      >
        获取 VNDB Token
      </button>
      <SettingsSwitch
        v-if="has_token"
        v-model="clear"
        label="清除已保存的 VNDB Token"
        :disabled="busy"
        @update:model-value="token = ''"
      />
      <p v-if="!desktop" class="settings-explanation">
        浏览器预览不能保存真实授权，请在桌面软件中配置。
      </p>
      <p v-if="error" class="metadata-error" role="alert">
        {{ error
        }}<button
          v-if="!loaded && desktop"
          class="quiet-button"
          type="button"
          :disabled="busy"
          @click="load"
        >
          重新读取
        </button>
      </p>
      <p v-if="result" role="status">{{ result }}</p>
      <div class="preference-actions">
        <button
          type="submit"
          class="primary-button"
          :disabled="!loaded || busy"
        >
          {{ busy ? '正在处理…' : '保存元数据设置' }}</button
        ><button
          class="secondary-button"
          type="button"
          :disabled="!loaded || busy || clear || (!has_token && !token.trim())"
          @click="save(true)"
        >
          保存并测试 VNDB
        </button>
      </div>
    </form>
  </SettingsSection>
</template>
<style scoped>
.metadata-error {
  color: var(--danger);
}
</style>

<style scoped>
.metadata-source-order {
  padding: 0;
  list-style: none;
  display: grid;
  gap: 10px;
  margin: 22px 0;
}
.metadata-source-order li {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--surface);
}
.metadata-source-order li > span:nth-child(2) {
  flex: 1;
  min-width: 0;
}
.drag-handle {
  cursor: grab;
  font-size: 22px;
  color: var(--muted);
  touch-action: none;
  user-select: none;
}
.metadata-source-order li.drag-target {
  border-color: var(--accent-ink, var(--accent));
}
.source-switch {
  padding: 8px 12px;
  border: 1px solid var(--border-strong);
  border-radius: 20px;
  background: var(--surface-hover);
  color: var(--muted);
}
.source-switch[aria-checked='true'] {
  background: var(--accent);
  color: var(--accent-text);
}
</style>
