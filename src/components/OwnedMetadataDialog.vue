<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import {
  ownedMetadata,
  ownedMetadataStats,
  stopOwnedMetadata,
  retryOwnedMetadata,
  queueOwnedMetadata,
  type OwnedMetadataStatus,
} from '../stores/ownedMetadata';
import PreviewCover from './preview/PreviewCover.vue';
import PreviewIcon from './preview/PreviewIcon.vue';
const dialog = ref<HTMLDialogElement>();
const current = computed(() =>
  ownedMetadata.items.find((i) => ['searching', 'applying'].includes(i.status)),
);
const retryable = computed(() =>
  ownedMetadata.items.some((i) =>
    ['failed', 'cancelled', 'no_match'].includes(i.status),
  ),
);
const percentage = computed(() =>
  ownedMetadataStats.value.total
    ? Math.round(
        (ownedMetadataStats.value.processed / ownedMetadataStats.value.total) *
          100,
      )
    : 0,
);
const labels: Record<OwnedMetadataStatus, string> = {
  queued: '等待中',
  searching: '匹配中',
  applying: '补充中',
  completed: '已补全',
  no_match: '未匹配',
  failed: '需重试',
  skipped: '已保留',
  cancelled: '已停止',
};
const heading = computed(() =>
  ownedMetadata.stopping
    ? '正在停止自动补全'
    : ownedMetadata.running
      ? '正在完善你的收藏'
      : ownedMetadataStats.value.attention
        ? '本轮处理已结束'
        : ownedMetadataStats.value.total
          ? '收藏资料已整理完成'
          : '等待已购游戏同步',
);
function sourceName(provider: string | null) {
  return provider === 'hikarinagi'
    ? 'Hikarinagi'
    : provider === 'bangumi'
      ? 'Bangumi'
      : provider === 'vndb'
        ? 'VNDB'
        : '官方资料';
}
function icon(status: OwnedMetadataStatus) {
  return status === 'completed'
    ? 'check'
    : ['failed', 'no_match'].includes(status)
      ? 'warning'
      : status === 'skipped'
        ? 'lock'
        : 'clock';
}
const opener =
  document.activeElement instanceof HTMLElement ? document.activeElement : null;
function close() {
  ownedMetadata.open = false;
  if (opener?.isConnected) opener.focus({ preventScroll: true });
}
onMounted(() => dialog.value?.showModal());
</script>
<template>
  <dialog
    ref="dialog"
    class="owned-metadata-dialog"
    aria-labelledby="owned-metadata-title"
    @cancel.prevent="close"
  >
    <header class="owned-heading">
      <div class="owned-heading-copy">
        <span class="owned-emblem"
          ><PreviewIcon name="spark" :size="24"
        /></span>
        <div>
          <p class="eyebrow">AUTOMATIC METADATA</p>
          <h2 id="owned-metadata-title">资料自动补全</h2>
        </div>
      </div>
      <button
        class="icon-button"
        aria-label="收起资料窗口"
        title="收起，后台继续"
        @click="close"
      >
        <PreviewIcon name="close" />
      </button>
    </header>
    <p class="owned-intro">
      使用 HIKARI FIELD 官方名称自动匹配，缺失资料按设置中的刮削源顺序补充。
    </p>
    <section class="owned-progress-panel" aria-label="自动补全总进度">
      <div class="owned-progress-heading">
        <div>
          <span
            class="owned-live-dot"
            :class="{
              running: ownedMetadata.running && !ownedMetadata.stopping,
            }"
            aria-hidden="true"
          /><strong>{{ heading }}</strong>
        </div>
        <span class="owned-percentage">{{ percentage }}<small>%</small></span>
      </div>
      <div
        class="owned-progress"
        role="progressbar"
        aria-label="已购游戏刮削进度"
        :aria-valuemin="0"
        :aria-valuemax="ownedMetadataStats.total || 1"
        :aria-valuenow="ownedMetadataStats.processed"
      >
        <span :style="{ width: `${percentage}%` }" />
      </div>
      <div class="owned-progress-caption">
        <span
          >已处理 {{ ownedMetadataStats.processed }} /
          {{ ownedMetadataStats.total }} 部</span
        ><span
          >已补全 {{ ownedMetadataStats.completed }} 部<span
            v-if="ownedMetadataStats.attention"
          >
            · 未完成 {{ ownedMetadataStats.attention }} 部</span
          ></span
        >
      </div>
      <div v-if="current" class="owned-current" aria-live="polite">
        <PreviewCover :cover_url="current.cover_url" :title="current.title" />
        <div>
          <small>正在处理 · {{ sourceName(current.provider) }}</small
          ><strong>{{ current.title }}</strong
          ><span>{{
            ownedMetadata.stopping
              ? '完成当前保存后停止后续任务'
              : current.message
          }}</span>
        </div>
        <PreviewIcon name="spark" :size="22" />
      </div>
      <p v-else class="owned-progress-note">
        {{
          ownedMetadataStats.attention
            ? '未完成的作品保留官方资料，不影响下载和启动。'
            : '已保存的资料会直接呈现在游戏库中。'
        }}
      </p>
    </section>
    <div class="owned-list-heading">
      <h3>游戏处理进度</h3>
      <span>自动匹配 · 自动保存</span>
    </div>
    <ul
      v-if="ownedMetadata.items.length"
      class="owned-list"
      aria-label="已购游戏刮削列表"
    >
      <li
        v-for="item in ownedMetadata.items"
        :key="item.game_id"
        class="owned-item"
        :class="{ active: ['searching', 'applying'].includes(item.status) }"
        :data-status="item.status"
      >
        <PreviewCover :cover_url="item.cover_url" :title="item.title" />
        <div class="owned-item-copy">
          <strong>{{ item.title }}</strong>
          <p>{{ item.message }}</p>
        </div>
        <span v-if="item.provider" class="owned-source">{{
          sourceName(item.provider)
        }}</span>
        <span class="owned-status"
          ><PreviewIcon :name="icon(item.status)" :size="14" />{{
            labels[item.status]
          }}</span
        >
      </li>
    </ul>
    <div v-else class="owned-empty">
      <PreviewIcon name="spark" :size="32" />
      <p>同步已购游戏后，资料会在后台自动补全。</p>
    </div>
    <footer>
      <p>
        <PreviewIcon name="info" :size="16" />收起后继续运行，左下角可查看进度。
      </p>
      <div>
        <button
          v-if="ownedMetadata.running"
          class="quiet-button"
          :disabled="ownedMetadata.stopping"
          @click="stopOwnedMetadata"
        >
          {{ ownedMetadata.stopping ? '正在停止…' : '停止补全' }}</button
        ><button
          v-else-if="retryable"
          class="secondary-button"
          @click="retryOwnedMetadata"
        >
          重试未完成项</button
        ><button
          v-else-if="!ownedMetadata.items.length"
          class="secondary-button"
          @click="queueOwnedMetadata"
        >
          检查缺失资料</button
        ><button class="primary-button" @click="close">
          {{ ownedMetadata.running ? '后台继续' : '完成' }}
        </button>
      </div>
    </footer>
  </dialog>
</template>
<style scoped>
.owned-metadata-dialog {
  margin: auto;
  width: min(760px, calc(100vw - 40px));
  max-height: calc(100dvh - 48px);
  padding: 28px;
  overflow: auto;
  box-sizing: border-box;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-lg);
  color: var(--text);
  background: var(--surface);
  box-shadow: var(--shadow-modal);
  animation: owned-reveal 220ms var(--ease-standard);
}
.owned-metadata-dialog::backdrop {
  background: rgb(32 27 45 / 38%);
  backdrop-filter: blur(10px);
}
.owned-heading,
.owned-heading-copy,
.owned-progress-heading,
.owned-progress-heading > div,
.owned-progress-caption,
.owned-current,
.owned-list-heading,
.owned-item,
footer,
footer > div,
footer > p {
  display: flex;
  align-items: center;
  gap: 14px;
}
.owned-heading {
  justify-content: space-between;
}
.owned-heading-copy {
  min-width: 0;
}
.owned-emblem {
  display: grid;
  place-items: center;
  flex-shrink: 0;
  width: 52px;
  height: 52px;
  color: var(--accent);
  background: var(--accent-wash);
  border-radius: 18px;
}
.eyebrow {
  margin: 0;
  font-size: 9px;
  letter-spacing: 0.15em;
}
h2 {
  margin: 5px 0 0;
  font-size: 24px;
}
.owned-intro {
  margin: 18px 0 22px;
  color: var(--muted);
  font-size: 12px;
  line-height: 1.9;
}
.owned-progress-panel {
  padding: 20px 22px;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--accent-wash);
}
.owned-progress-heading {
  justify-content: space-between;
}
.owned-progress-heading > div {
  gap: 9px;
}
.owned-progress-heading strong {
  font-size: 14px;
  font-weight: 500;
}
.owned-live-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--accent);
  flex-shrink: 0;
}
.owned-live-dot.running {
  animation: owned-breathe 1.8s ease-in-out infinite;
}
.owned-percentage {
  color: var(--accent);
  font-size: 30px;
  font-variant-numeric: tabular-nums;
  letter-spacing: -0.03em;
}
.owned-percentage small {
  font-size: 12px;
  margin-left: 3px;
}
.owned-progress {
  margin-top: 14px;
  height: 5px;
  overflow: hidden;
  border-radius: var(--radius-pill);
  background: var(--border);
}
.owned-progress > span {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: var(--accent);
  transition: width 250ms var(--ease-standard);
}
.owned-progress-caption {
  justify-content: space-between;
  flex-wrap: wrap;
  margin-top: 10px;
  gap: 5px 12px;
  font-size: 10px;
  color: var(--muted);
  font-variant-numeric: tabular-nums;
}
.owned-current {
  margin-top: 18px;
  padding-top: 16px;
  border-top: 1px solid var(--border);
}
.owned-current > .preview-cover {
  width: 38px;
  height: 52px;
  border-radius: 6px;
  flex-shrink: 0;
}
.owned-current > div:last-of-type {
  min-width: 0;
  flex: 1;
}
.owned-current small,
.owned-current strong,
.owned-current span {
  display: block;
  overflow-wrap: anywhere;
}
.owned-current small {
  font-size: 9px;
  color: var(--muted);
}
.owned-current strong {
  margin: 4px 0;
  font-size: 13px;
}
.owned-current span {
  font-size: 10px;
  line-height: 1.7;
  color: var(--muted);
}
.owned-current > svg {
  flex-shrink: 0;
  color: var(--accent);
}
.owned-progress-note {
  margin: 17px 0 0;
  font-size: 11px;
  line-height: 1.8;
  color: var(--muted);
}
.owned-list-heading {
  justify-content: space-between;
  margin: 23px 0 12px;
}
.owned-list-heading h3 {
  margin: 0;
  font-size: 13px;
  font-weight: 500;
}
.owned-list-heading span {
  font-size: 10px;
  color: var(--muted);
}
.owned-list {
  list-style: none;
  margin: 0;
  padding: 2px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: 340px;
  overflow: auto;
}
.owned-item {
  gap: 12px;
  padding: 12px;
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  transition:
    background 160ms,
    border-color 160ms;
}
.owned-item.active {
  background: var(--accent-wash);
  border-color: var(--border-strong);
}
.owned-item > .preview-cover {
  width: 38px;
  height: 52px;
  border-radius: 6px;
  flex-shrink: 0;
}
.owned-item-copy {
  min-width: 0;
  flex: 1;
}
.owned-item-copy strong {
  display: block;
  font-size: 12px;
  line-height: 1.6;
  overflow-wrap: anywhere;
}
.owned-item-copy p {
  color: var(--muted);
  font-size: 10px;
  line-height: 1.8;
  margin: 5px 0 0;
  overflow-wrap: anywhere;
}
.owned-source {
  color: var(--muted);
  font-size: 9px;
}
.owned-status {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: 10px;
  color: var(--muted);
  flex-shrink: 0;
}
.owned-item[data-status='completed'] .owned-status {
  color: var(--success);
}
.owned-item:is([data-status='failed'], [data-status='no_match']) .owned-status {
  color: var(--warning);
}
.owned-item.active .owned-status {
  color: var(--accent);
}
.owned-empty {
  text-align: center;
  padding: 24px;
  color: var(--muted);
  font-size: 12px;
}
.owned-empty svg {
  color: var(--accent);
}
footer {
  border-top: 1px solid var(--border);
  margin-top: 22px;
  padding-top: 20px;
  justify-content: space-between;
  gap: 12px;
}
footer > p {
  color: var(--muted);
  font-size: 10px;
  gap: 6px;
  margin: 0;
}
footer > p > svg {
  flex-shrink: 0;
}
footer > div {
  gap: 8px;
  flex-shrink: 0;
}
@keyframes owned-reveal {
  from {
    opacity: 0;
    transform: translateY(10px) scale(0.985);
  }
  to {
    opacity: 1;
    transform: none;
  }
}
@keyframes owned-breathe {
  0%,
  100% {
    opacity: 0.5;
    box-shadow: 0 0 0 2px var(--accent-wash);
  }
  50% {
    opacity: 1;
    box-shadow: 0 0 0 5px var(--accent-wash);
  }
}
@media (max-width: 580px) {
  .owned-metadata-dialog {
    width: calc(100vw - 24px);
    padding: 20px;
  }
  h2 {
    font-size: 21px;
  }
  .owned-progress-panel {
    padding: 16px;
  }
  .owned-progress-heading strong {
    font-size: 12px;
  }
  .owned-percentage {
    font-size: 26px;
  }
  .owned-list {
    max-height: min(340px, max(140px, calc(100dvh - 550px)));
  }
  .owned-item {
    gap: 9px;
    padding: 10px 6px;
    flex-wrap: wrap;
  }
  .owned-item-copy {
    flex-basis: calc(100% - 90px);
  }
  .owned-source {
    margin-left: 47px;
  }
  .owned-status {
    margin-left: auto;
  }
  footer {
    flex-wrap: wrap;
  }
  footer > div {
    width: 100%;
    justify-content: flex-end;
  }
}
@media (prefers-reduced-motion: reduce) {
  .owned-metadata-dialog,
  .owned-metadata-dialog * {
    animation: none !important;
    transition: none !important;
  }
}
:global(:root[data-motion='reduced'] .owned-metadata-dialog),
:global(:root[data-motion='reduced'] .owned-metadata-dialog *) {
  animation: none !important;
  transition: none !important;
}
</style>
