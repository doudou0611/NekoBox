<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import {
  ownedMetadata,
  ownedMetadataStats,
  queueOwnedMetadata,
  searchOwnedMetadata,
  confirmOwnedMetadata,
  retryOwnedMetadata,
  stopOwnedMetadata,
  type OwnedMetadataStatus,
} from '../stores/ownedMetadata';
import { coverSource, local } from '../stores/library';
import PreviewCover from './preview/PreviewCover.vue';
import PreviewIcon from './preview/PreviewIcon.vue';
const dialog = ref<HTMLDialogElement>();
const selectedId = ref('');
const selected = computed(
  () =>
    ownedMetadata.items.find((i) => i.game_id === selectedId.value) ??
    ownedMetadata.items.find((i) =>
      ['review', 'no_match', 'failed'].includes(i.status),
    ) ??
    ownedMetadata.items[0],
);
const detail = computed(() =>
  selected.value ? local.records[selected.value.game_id] : undefined,
);
const description = computed(() =>
  detail.value && 'description' in detail.value ? detail.value.description : '',
);
const labels: Record<OwnedMetadataStatus, string> = {
  queued: '等待中',
  searching: '正在匹配',
  applying: '正在补充',
  completed: '已补充',
  review: '待确认',
  no_match: '未匹配',
  failed: '需重试',
  skipped: '已保留',
  cancelled: '已停止',
};
const busy = computed(
  () =>
    selected.value &&
    ['queued', 'searching', 'applying'].includes(selected.value.status),
);
const retryable = computed(() =>
  ownedMetadata.items.some((i) => ['failed', 'cancelled'].includes(i.status)),
);
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
          <p class="eyebrow">YOUR STORIES, MORE COMPLETE</p>
          <h2 id="owned-metadata-title">已购游戏资料</h2>
        </div>
      </div>
      <button
        class="icon-button"
        aria-label="收起资料窗口"
        title="收起，任务在后台继续"
        @click="close"
      >
        <PreviewIcon name="close" />
      </button>
    </header>
    <p class="owned-intro">
      保留 HIKARI FIELD 提供的资料，缺失内容按设置中的刮削源顺序补充。
    </p>
    <div class="owned-overview" aria-live="polite">
      <div>
        <strong>{{ ownedMetadataStats.completed }}</strong
        ><span>已补充</span>
      </div>
      <div>
        <strong>{{ ownedMetadataStats.attention }}</strong
        ><span>待处理</span>
      </div>
      <div class="owned-progress-copy">
        <span>{{
          ownedMetadata.stopping
            ? '正在停止…'
            : ownedMetadata.running
              ? '正在完善你的收藏'
              : '资料整理进度'
        }}</span
        ><small
          >{{ ownedMetadataStats.processed }} /
          {{ ownedMetadataStats.total }} 部</small
        >
      </div>
      <div
        v-if="ownedMetadataStats.total"
        class="owned-progress"
        role="progressbar"
        aria-label="已购游戏刮削进度"
        :aria-valuemin="0"
        :aria-valuemax="ownedMetadataStats.total"
        :aria-valuenow="ownedMetadataStats.processed"
      >
        <span
          :style="{
            width: `${(ownedMetadataStats.processed / ownedMetadataStats.total) * 100}%`,
          }"
        />
      </div>
    </div>
    <div v-if="ownedMetadata.items.length" class="owned-workspace">
      <nav class="owned-list" aria-label="已购游戏刮削列表">
        <button
          v-for="item in ownedMetadata.items"
          :key="item.game_id"
          class="owned-item"
          :class="{ active: selected?.game_id === item.game_id }"
          :aria-pressed="selected?.game_id === item.game_id"
          @click="selectedId = item.game_id"
        >
          <PreviewCover :cover_url="item.cover_url" :title="item.title" />
          <span class="owned-item-copy"
            ><strong>{{ item.title }}</strong
            ><small :data-status="item.status">{{
              labels[item.status]
            }}</small></span
          >
          <PreviewIcon
            :name="
              item.status === 'completed'
                ? 'check'
                : ['failed', 'review', 'no_match'].includes(item.status)
                  ? 'warning'
                  : 'arrow'
            "
            :size="16"
          />
        </button>
      </nav>
      <section v-if="selected" class="owned-inspector" :aria-busy="busy">
        <div class="owned-current">
          <PreviewCover
            :cover_url="coverSource(detail?.cover_url || selected.cover_url)"
            :title="selected.title"
          />
          <div>
            <small>HIKARI FIELD · 已拥有</small>
            <h3>{{ selected.title }}</h3>
            <span class="owned-status" :data-status="selected.status">{{
              labels[selected.status]
            }}</span>
          </div>
        </div>
        <p class="owned-message" role="status">{{ selected.message }}</p>
        <template v-if="selected.status === 'completed'">
          <dl class="owned-fields">
            <div>
              <dt>开发商</dt>
              <dd>{{ detail?.developer || '来源暂未提供' }}</dd>
            </div>
            <div>
              <dt>发行日期</dt>
              <dd>{{ detail?.release_date || '来源暂未提供' }}</dd>
            </div>
          </dl>
          <p v-if="description" class="owned-description">{{ description }}</p>
          <p class="owned-note">
            已保存到游戏库。收藏、游玩记录和安装位置保持原有设置。
          </p>
        </template>
        <template
          v-if="
            ['review', 'no_match', 'failed', 'cancelled'].includes(
              selected.status,
            )
          "
        >
          <form
            class="owned-search"
            @submit.prevent="searchOwnedMetadata(selected!)"
          >
            <label for="owned-query">作品搜索关键词</label>
            <div>
              <input
                id="owned-query"
                v-model="selected.query"
                maxlength="200"
                :disabled="busy || ownedMetadata.stopping"
              /><button
                class="secondary-button"
                :disabled="
                  busy || ownedMetadata.stopping || !selected.query.trim()
                "
              >
                重新搜索
              </button>
            </div>
          </form>
          <p v-if="selected.candidates.length" class="owned-note">
            选择对应作品后补充资料，来源优先级仍沿用设置。
          </p>
          <ul v-if="selected.candidates.length" class="owned-candidates">
            <li
              v-for="candidate in selected.candidates"
              :key="`${candidate.provider}:${candidate.remote_id}`"
            >
              <PreviewCover
                :cover_url="candidate.cover_url || ''"
                :title="candidate.title"
              />
              <div>
                <small>{{ candidate.provider }}</small
                ><strong>{{ candidate.title }}</strong
                ><span v-if="candidate.subtitle">{{ candidate.subtitle }}</span>
              </div>
              <button
                class="secondary-button"
                :disabled="ownedMetadata.stopping"
                @click="confirmOwnedMetadata(selected!, candidate)"
              >
                使用此资料
              </button>
            </li>
          </ul>
        </template>
      </section>
    </div>
    <div v-else class="owned-empty">
      <PreviewIcon name="check" :size="36" />
      <h3>收藏已就绪</h3>
      <p>同步已购游戏后，未刮削的作品会在这里自动补充资料。</p>
    </div>
    <footer>
      <p>
        <PreviewIcon
          name="info"
          :size="16"
        />收起窗口后，左下角可继续查看任务进度。
      </p>
      <div>
        <button
          v-if="ownedMetadata.running"
          class="quiet-button"
          :disabled="ownedMetadata.stopping"
          @click="stopOwnedMetadata"
        >
          {{ ownedMetadata.stopping ? '正在停止…' : '停止刮削' }}</button
        ><button
          v-else-if="retryable"
          class="secondary-button"
          @click="retryOwnedMetadata"
        >
          重试未完成项</button
        ><button v-else class="secondary-button" @click="queueOwnedMetadata">
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
  width: min(920px, calc(100vw - 40px));
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
.owned-current,
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
h2 {
  font-size: 24px;
  margin: 5px 0 0;
}
.eyebrow {
  margin: 0;
  font-size: 9px;
  letter-spacing: 0.12em;
}
.owned-intro {
  margin: 18px 0;
  color: var(--muted);
  font-size: 13px;
  line-height: 1.8;
}
.owned-overview {
  position: relative;
  display: flex;
  align-items: center;
  gap: 32px;
  padding: 18px 22px 22px;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--accent-wash);
  overflow: hidden;
}
.owned-overview > div:not(.owned-progress) {
  display: flex;
  align-items: baseline;
  gap: 8px;
}
.owned-overview strong {
  font-size: 28px;
  font-weight: 500;
  color: var(--accent);
  font-variant-numeric: tabular-nums;
}
.owned-overview span,
.owned-overview small {
  font-size: 12px;
  color: var(--muted);
}
.owned-overview .owned-progress-copy {
  margin-left: auto;
  flex-direction: column;
  gap: 5px;
  align-items: flex-end;
}
.owned-progress {
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  height: 3px;
  background: var(--border);
}
.owned-progress > span {
  display: block;
  height: 100%;
  background: var(--accent);
  transition: width 250ms var(--ease-standard);
}
.owned-workspace {
  display: grid;
  grid-template-columns: 260px minmax(0, 1fr);
  gap: 24px;
  margin-top: 22px;
}
.owned-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: 420px;
  overflow: auto;
  padding: 2px;
}
.owned-item {
  display: flex;
  align-items: center;
  gap: 12px;
  width: 100%;
  padding: 10px;
  text-align: left;
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text);
  cursor: pointer;
  transition:
    background 160ms,
    border-color 160ms;
}
.owned-item:hover {
  background: var(--surface-hover);
}
.owned-item.active {
  background: var(--accent-wash);
  border-color: var(--border-strong);
}
.owned-item > .preview-cover {
  width: 38px;
  height: 52px;
  flex-shrink: 0;
  border-radius: 6px;
}
.owned-item-copy {
  min-width: 0;
  flex: 1;
}
.owned-item-copy strong {
  display: block;
  font-size: 12px;
  line-height: 1.6;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.owned-item small {
  display: block;
  margin-top: 4px;
  font-size: 10px;
  color: var(--muted);
}
.owned-item > svg {
  flex-shrink: 0;
  color: var(--muted);
}
.owned-inspector {
  min-width: 0;
  max-height: 420px;
  overflow: auto;
  padding: 6px 6px 6px 0;
}
.owned-current > .preview-cover {
  width: 70px;
  height: 96px;
  flex-shrink: 0;
  border-radius: 9px;
}
.owned-current > div:last-child {
  min-width: 0;
}
.owned-current small {
  font-size: 10px;
  color: var(--muted);
}
h3 {
  font-size: 19px;
  margin: 8px 0 12px;
  overflow-wrap: anywhere;
}
.owned-status {
  display: inline-block;
  padding: 4px 9px;
  border-radius: var(--radius-pill);
  font-size: 10px;
  background: var(--accent-wash);
  color: var(--accent);
}
.owned-message {
  margin: 20px 0;
  color: var(--muted);
  font-size: 12px;
  line-height: 1.9;
  overflow-wrap: anywhere;
}
.owned-fields {
  display: flex;
  gap: 30px;
  padding: 14px 0;
  border-top: 1px solid var(--border);
  border-bottom: 1px solid var(--border);
}
.owned-fields dt {
  color: var(--muted);
  font-size: 10px;
}
.owned-fields dd {
  margin: 7px 0 0;
  font-size: 12px;
}
.owned-description {
  display: -webkit-box;
  -webkit-line-clamp: 6;
  -webkit-box-orient: vertical;
  overflow: hidden;
  white-space: pre-line;
  font-size: 12px;
  line-height: 1.9;
}
.owned-note {
  color: var(--muted);
  font-size: 11px;
  line-height: 1.9;
}
.owned-search label {
  display: block;
  font-size: 11px;
  margin-bottom: 8px;
}
.owned-search > div {
  display: flex;
  gap: 8px;
}
.owned-search input {
  min-width: 0;
  flex: 1;
  padding: 10px 12px;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-sm);
  color: var(--text);
  background: var(--surface-input, var(--surface-hover));
}
.owned-search button {
  flex-shrink: 0;
}
.owned-candidates {
  list-style: none;
  padding: 0;
  margin: 14px 0 0;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.owned-candidates li {
  display: flex;
  align-items: center;
  gap: 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  padding: 12px;
}
.owned-candidates .preview-cover {
  width: 40px;
  height: 56px;
  flex-shrink: 0;
  border-radius: 6px;
}
.owned-candidates li > div:not(.preview-cover) {
  min-width: 0;
  flex: 1;
}
.owned-candidates small,
.owned-candidates strong,
.owned-candidates span {
  display: block;
  overflow-wrap: anywhere;
}
.owned-candidates small,
.owned-candidates span {
  color: var(--muted);
  font-size: 10px;
}
.owned-candidates strong {
  font-size: 12px;
  margin: 4px 0;
}
.owned-candidates button {
  padding: 8px 10px;
  flex-shrink: 0;
  font-size: 11px;
}
.owned-empty {
  text-align: center;
  padding: 50px 20px;
  color: var(--muted);
}
.owned-empty > svg {
  color: var(--accent);
}
.owned-empty p {
  font-size: 12px;
}
footer {
  border-top: 1px solid var(--border);
  margin-top: 24px;
  padding-top: 20px;
  justify-content: space-between;
  gap: 12px;
}
footer > p {
  color: var(--muted);
  font-size: 11px;
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
@media (max-width: 680px) {
  .owned-metadata-dialog {
    width: calc(100vw - 24px);
    padding: 20px;
  }
  .owned-overview {
    gap: 18px;
    padding: 12px 16px 16px;
  }
  .owned-overview .owned-progress-copy > span {
    display: none;
  }
  .owned-workspace {
    grid-template-columns: 1fr;
    gap: 16px;
  }
  .owned-list {
    flex-direction: row;
    max-height: none;
    overflow-x: auto;
  }
  .owned-item {
    min-width: 175px;
    width: 175px;
  }
  .owned-inspector {
    max-height: none;
  }
  footer {
    flex-wrap: wrap;
  }
  footer > div {
    width: 100%;
    justify-content: flex-end;
  }
  .eyebrow {
    font-size: 8px;
  }
  h2 {
    font-size: 21px;
  }
  .owned-candidates li {
    flex-wrap: wrap;
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
