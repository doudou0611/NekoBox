<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import PreviewIcon from './preview/PreviewIcon.vue';
import {
  updates,
  downloadAppUpdate,
  openUpdateRelease,
} from '../stores/updates';

const props = defineProps<{ visible: boolean }>();
const panel = ref<HTMLDialogElement>();
let previousFocus: HTMLElement | null = null;
watch(
  () => props.visible,
  (visible) => {
    if (visible)
      previousFocus =
        document.activeElement instanceof HTMLElement
          ? document.activeElement
          : null;
  },
);
const router = useRouter();
const downloading = computed(() => updates.status.phase === 'downloading');
const ready = computed(() => updates.status.phase === 'ready');
const progress = computed(() =>
  updates.status.total
    ? Math.min(100, (updates.status.downloaded / updates.status.total) * 100)
    : undefined,
);
const title = computed(() =>
  ready.value
    ? '更新已准备好'
    : downloading.value
      ? '正在迎接新版本'
      : 'NekoBox 有新版本了',
);
const announcement = computed(() =>
  ready.value
    ? '下载与签名校验已完成。方便时再前往设置安装。'
    : downloading.value
      ? '正在后台下载，你可以继续浏览和游玩。'
      : `发现新版本 ${updates.status.version}，可以随时查看或稍后更新。`,
);
const size = (bytes: number) => `${(bytes / 1024 / 1024).toFixed(1)} MB`;
function dismiss() {
  if (
    panel.value?.contains(document.activeElement) &&
    previousFocus?.isConnected
  )
    previousFocus.focus({ preventScroll: true });
  updates.notice = false;
}
function viewUpdates() {
  dismiss();
  void router.push({ name: 'settings', query: { section: 'updates' } });
}
</script>

<template>
  <Teleport to="body">
    <Transition name="update-notice">
      <!-- Non-modal: no showModal(), backdrop, autofocus or inert page content. -->
      <dialog
        v-if="visible"
        ref="panel"
        open
        class="ui-dialog startup-update-panel"
        aria-modal="false"
        aria-labelledby="startup-update-title"
        @keydown.esc.stop.prevent="dismiss"
      >
        <header class="startup-update-heading">
          <div class="startup-update-brand" aria-hidden="true">
            <img src="/brand/nekobox.png" alt="" width="48" height="48" />
            <span><PreviewIcon name="spark" :size="13" /></span>
          </div>
          <div>
            <p class="startup-update-eyebrow">A LITTLE MORE POSSIBILITY</p>
            <h2 id="startup-update-title">{{ title }}</h2>
          </div>
          <button
            class="icon-button startup-update-close"
            type="button"
            aria-label="关闭更新提示"
            @click="dismiss"
          >
            <PreviewIcon name="close" :size="18" />
          </button>
        </header>

        <div class="startup-update-versions" aria-label="版本信息">
          <span>{{ updates.status.current_version }}</span>
          <PreviewIcon name="arrow" :size="16" />
          <strong>{{ updates.status.version }}</strong>
          <span class="startup-update-tag">{{
            ready ? '已就绪' : '新版本'
          }}</span>
        </div>
        <p class="startup-update-message" role="status">{{ announcement }}</p>

        <details v-if="updates.status.notes" class="startup-update-notes">
          <summary>
            这次带来了什么<PreviewIcon name="chevron-down" :size="15" />
          </summary>
          <p>{{ updates.status.notes }}</p>
        </details>

        <div v-if="downloading" class="startup-update-download">
          <progress :value="progress" max="100" aria-label="更新下载进度" />
          <p>
            {{ size(updates.status.downloaded) }}
            <template v-if="updates.status.total">
              / {{ size(updates.status.total) }} ·
              {{ Math.floor(progress ?? 0) }}%
            </template>
          </p>
        </div>
        <p v-if="updates.error" class="form-error" role="status">
          {{ updates.error }}
        </p>

        <footer class="startup-update-actions">
          <button class="quiet-button" type="button" @click="dismiss">
            {{ downloading ? '收起，继续使用' : '稍后再说' }}
          </button>
          <button
            v-if="ready"
            class="primary-button"
            type="button"
            @click="viewUpdates"
          >
            前往安装<PreviewIcon name="arrow" :size="16" />
          </button>
          <button
            v-else-if="downloading"
            class="secondary-button"
            type="button"
            @click="viewUpdates"
          >
            查看下载
          </button>
          <button
            v-else-if="updates.status.signed"
            class="primary-button"
            type="button"
            :disabled="updates.busy"
            @click="downloadAppUpdate"
          >
            <PreviewIcon name="download" :size="16" />下载更新
          </button>
          <button
            v-else
            class="primary-button"
            type="button"
            @click="openUpdateRelease(Boolean(updates.status.download_url))"
          >
            {{
              updates.status.installation === 'portable' &&
              updates.status.download_url
                ? '下载便携包'
                : '查看更新'
            }}
            <PreviewIcon name="arrow" :size="16" />
          </button>
        </footer>
        <p class="startup-update-footnote">
          由你决定何时更新，不会自动安装或重启。
        </p>
      </dialog>
    </Transition>
  </Teleport>
</template>

<style scoped>
.startup-update-panel {
  inset: auto 24px 24px auto;
  margin: 0;
  width: min(424px, calc(100vw - 32px));
  max-height: calc(100dvh - 48px);
  padding: 24px;
  z-index: calc(var(--z-navigation) + 1);
  animation: none !important;
  box-shadow:
    inset 0 1px 0 var(--dialog-highlight),
    var(--shadow-floating);
}
.startup-update-heading {
  display: flex;
  align-items: center;
  gap: 14px;
}
.startup-update-heading > div:nth-child(2) {
  flex: 1;
  min-width: 0;
}
.startup-update-heading h2 {
  margin: 3px 0 0;
  font-size: 22px;
  line-height: 1.5;
}
.startup-update-eyebrow {
  margin: 0;
  color: var(--subtle);
  font: 8px var(--font-mono);
  letter-spacing: 0.13em;
}
.startup-update-brand {
  position: relative;
  flex: 0 0 48px;
}
.startup-update-brand img {
  display: block;
  border-radius: 14px;
  box-shadow: var(--shadow-subtle);
}
.startup-update-brand > span {
  position: absolute;
  right: -4px;
  bottom: -4px;
  display: grid;
  place-items: center;
  width: 22px;
  height: 22px;
  border: 2px solid var(--surface);
  border-radius: var(--radius-pill);
  background: var(--accent);
  color: var(--on-accent, #fff);
}
.startup-update-close {
  flex: 0 0 30px;
  width: 30px;
  height: 30px;
  border-radius: var(--radius-pill);
  align-self: flex-start;
}
.startup-update-versions {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-top: 24px;
  color: var(--subtle);
  font-size: 13px;
  font-variant-numeric: tabular-nums;
  overflow-wrap: anywhere;
}
.startup-update-versions strong {
  color: var(--text);
  font-weight: 500;
}
.startup-update-tag {
  margin-left: auto;
  padding: 4px 9px;
  border-radius: var(--radius-pill);
  background: var(--accent-wash);
  color: var(--accent-ink, var(--accent));
  font-size: 10px;
  flex-shrink: 0;
}
.startup-update-message {
  margin: 12px 0 0;
  color: var(--muted);
  font-size: 13px;
  line-height: 1.8;
}
.startup-update-notes {
  margin-top: 18px;
  padding: 12px 0;
  border-block: 1px solid var(--border);
}
.startup-update-notes summary {
  display: flex;
  align-items: center;
  justify-content: space-between;
  list-style: none;
  cursor: pointer;
  color: var(--text);
  font-size: 12px;
}
.startup-update-notes summary::-webkit-details-marker {
  display: none;
}
.startup-update-notes summary svg {
  transition: transform var(--micro-duration) var(--ease-standard);
}
.startup-update-notes[open] summary svg {
  transform: rotate(180deg);
}
.startup-update-notes p {
  margin-top: 12px;
  max-height: 150px;
  overflow: auto;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  color: var(--muted);
  font-size: 12px;
  line-height: 1.8;
}
.startup-update-actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  margin-top: 22px;
}
.startup-update-actions button {
  min-height: 40px;
  gap: 8px;
}
.startup-update-footnote {
  margin: 14px 0 0;
  color: var(--subtle);
  font-size: 10px;
  line-height: 1.6;
}
.startup-update-download {
  margin-top: 18px;
}
.startup-update-download progress {
  width: 100%;
  height: 6px;
  accent-color: var(--accent);
}
.startup-update-download p {
  margin-top: 8px;
  color: var(--subtle);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
}
.update-notice-enter-active {
  transition:
    opacity var(--layout-duration) var(--ease-standard),
    transform var(--layout-duration) var(--ease-standard);
}
.update-notice-leave-active {
  pointer-events: none;
  transition:
    opacity var(--micro-duration) var(--ease-exit),
    transform var(--micro-duration) var(--ease-exit);
}
.update-notice-enter-from,
.update-notice-leave-to {
  opacity: 0;
  transform: translateY(16px) scale(0.98);
}
:global(:root[data-motion='light']) .update-notice-enter-from,
:global(:root[data-motion='light']) .update-notice-leave-to {
  transform: none;
}
:global(:root[data-motion='reduced']) .startup-update-panel {
  transition: none;
}
@media (prefers-reduced-motion: reduce) {
  .startup-update-panel,
  .startup-update-notes summary svg {
    transition: none;
  }
  .update-notice-enter-from,
  .update-notice-leave-to {
    transform: none;
  }
}
@media (max-width: 600px) {
  .startup-update-panel {
    right: 16px;
    bottom: 16px;
    padding: 20px;
    max-height: calc(100dvh - 32px);
  }
  .startup-update-heading h2 {
    font-size: 18px;
  }
  .startup-update-heading {
    gap: 10px;
  }
}
</style>
