<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { api, desktop, errorText, loadDetail, notify } from '../stores/library';
import type { MetadataCandidate } from '../types/domain';
import {
  metadataProviders,
  type MetadataProvider,
} from '../services/metadataScraper';
import PreviewCover from './preview/PreviewCover.vue';
const props = defineProps<{
  gameId: string;
  title: string;
  metadataStatus?: string;
  locked?: boolean;
  compact?: boolean;
}>();
const query = ref(props.title);
const dialog = ref<HTMLDialogElement>();
const provider = ref<MetadataProvider | ''>('');
const candidates = ref<MetadataCandidate[]>([]);
const busy = ref(false);
const error = ref('');
const confirming = ref('');
const message = ref('');
const searched = ref(false);
let searchVersion = 0;
let disposed = false;
let searchController: AbortController | null = null;
let opener: HTMLElement | null = null;
function invalidateSearch() {
  searchVersion += 1;
  searchController?.abort();
  searchController = null;
  busy.value = false;
}
function close() {
  // A native dialog's close event is queued. Invalidate before closing so a
  // response resolving in the same turn cannot populate a reopened picker.
  invalidateSearch();
  dialog.value?.close();
  if (opener?.isConnected) opener.focus({ preventScroll: true });
}
function closed() {
  if (!dialog.value?.open) invalidateSearch();
}
async function openSearch() {
  if (props.locked || !query.value.trim() || confirming.value) return;
  invalidateSearch();
  const version = searchVersion;
  opener =
    document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null;
  provider.value = '';
  candidates.value = [];
  error.value = '';
  searched.value = false;
  await nextTick();
  if (disposed || version !== searchVersion) return;
  dialog.value?.showModal();
  dialog.value
    ?.querySelector<HTMLButtonElement>('[data-source-choice]')
    ?.focus();
}
async function search(source: MetadataProvider) {
  if (props.locked || confirming.value) return;
  invalidateSearch();
  const controller = new AbortController();
  searchController = controller;
  provider.value = source;
  busy.value = true;
  error.value = '';
  searched.value = false;
  candidates.value = [];
  const version = ++searchVersion;
  const gameId = props.gameId;
  try {
    const found = await api(
      'search_metadata',
      {
        query: query.value.trim(),
        providers: [source],
        manual: true,
      },
      { signal: controller.signal },
    );
    if (
      version === searchVersion &&
      gameId === props.gameId &&
      dialog.value?.open
    ) {
      candidates.value = found;
      searched.value = true;
    }
  } catch (cause) {
    if (version === searchVersion) error.value = errorText(cause);
  } finally {
    if (version === searchVersion) {
      busy.value = false;
      searchController = null;
    }
  }
}
async function confirm(candidate: MetadataCandidate) {
  if (props.locked || confirming.value) return;
  confirming.value = `${candidate.provider}:${candidate.remote_id}`;
  error.value = '';
  const gameId = props.gameId;
  try {
    const match = await api('confirm_metadata_match', {
      game_id: gameId,
      title_hint: candidate.title,
      provider: candidate.provider,
      remote_id: candidate.remote_id,
      manual: true,
    });
    if (disposed || gameId !== props.gameId) return;
    await loadDetail(gameId);
    if (disposed || gameId !== props.gameId) return;
    message.value = [match.cover_message, match.supplementation_message]
      .filter(Boolean)
      .join('；');
    notify(`已绑定 ${match.provider} 资料：${candidate.title}。`);
    close();
  } catch (cause) {
    if (!disposed && gameId === props.gameId) error.value = errorText(cause);
  } finally {
    confirming.value = '';
  }
}
watch(
  () => props.locked,
  (locked) => {
    if (locked) close();
  },
);
watch(
  () => props.gameId,
  () => {
    close();
    query.value = props.title;
    candidates.value = [];
    message.value = '';
    error.value = '';
  },
);
watch(
  () => props.title,
  (title) => {
    if (!query.value.trim()) query.value = title;
  },
);
onBeforeUnmount(() => {
  disposed = true;
  invalidateSearch();
  dialog.value?.close();
});
</script>
<template>
  <section
    v-if="desktop"
    class="metadata-match"
    :class="{ compact }"
    aria-labelledby="metadata-match-title"
  >
    <div v-if="!compact" class="metadata-match-heading">
      <div>
        <p class="eyebrow">METADATA</p>
        <h3 id="metadata-match-title">资料与作品封面</h3>
      </div>
      <span class="metadata-source">{{
        props.metadataStatus === 'synced' ? '已绑定来源资料' : '选择资料来源'
      }}</span>
    </div>
    <div class="metadata-search">
      <input
        v-model="query"
        maxlength="200"
        aria-label="资料搜索关键词"
        placeholder="输入作品名"
        :disabled="props.locked || !!confirming"
        @keydown.enter.prevent="openSearch"
      /><button
        class="secondary-button"
        :disabled="props.locked || !!confirming || !query.trim()"
        @click="openSearch"
      >
        {{ compact ? '重新刮削（手动）' : '搜索资料与封面' }}
      </button>
    </div>
    <p class="metadata-disclosure">
      手动选择搜索源；本次先获取所选来源的资料与封面，再按设置中其他启用来源的顺序补充。仅发送作品名，不上传本地路径。
    </p>
    <p v-if="message" role="status" class="metadata-disclosure">
      {{ message }}
    </p>
    <Teleport to="body"
      ><dialog
        ref="dialog"
        class="metadata-picker"
        aria-label="选择资料来源与作品"
        @close="closed"
        @cancel.prevent="!confirming && close()"
      >
        <header>
          <h2>选择资料来源与作品</h2>
          <button
            class="quiet-button"
            aria-label="关闭资料搜索"
            :disabled="props.locked || !!confirming"
            @click="close"
          >
            关闭
          </button>
        </header>
        <form
          class="metadata-search"
          @submit.prevent="provider && search(provider)"
        >
          <input
            v-model="query"
            aria-label="弹窗资料搜索关键词"
            maxlength="200"
            :disabled="props.locked || !!confirming"
          /><button
            class="secondary-button"
            :disabled="props.locked || !provider || !!confirming"
          >
            搜索
          </button>
        </form>
        <div class="metadata-provider" aria-label="选择搜索源">
          <button
            v-for="source in metadataProviders"
            :key="source"
            data-source-choice
            class="quiet-button"
            :class="{ active: provider === source }"
            :aria-pressed="provider === source"
            :disabled="props.locked || !!confirming"
            @click="search(source)"
          >
            {{
              source === 'hikarinagi'
                ? 'Hikarinagi'
                : source === 'bangumi'
                  ? 'Bangumi'
                  : 'VNDB'
            }}
          </button>
        </div>
        <p v-if="!provider">选择一个来源查看候选作品。</p>
        <p v-if="busy" role="status">正在搜索 {{ provider }}…</p>
        <p v-if="error" class="metadata-error" role="alert">{{ error }}</p>
        <p v-if="searched && !candidates.length">
          当前来源没有找到候选作品，可以更换来源或关键词。
        </p>
        <ul class="metadata-candidates">
          <li
            v-for="candidate in candidates"
            :key="`${candidate.provider}:${candidate.remote_id}`"
          >
            <PreviewCover
              class="candidate-cover"
              :cover_url="candidate.cover_url || ''"
              :title="candidate.title"
            />
            <div class="candidate-copy">
              <strong>{{ candidate.title }}</strong
              ><small v-if="candidate.subtitle">{{ candidate.subtitle }}</small
              ><small
                >{{ candidate.provider }} · {{ candidate.remote_id }} · 匹配度
                {{ Math.round(candidate.confidence * 100) }}%</small
              >
            </div>
            <button
              class="quiet-button"
              :disabled="props.locked || !!confirming || busy"
              @click="confirm(candidate)"
            >
              {{
                confirming === `${candidate.provider}:${candidate.remote_id}`
                  ? '获取资料中…'
                  : '确认绑定'
              }}
            </button>
          </li>
        </ul>
      </dialog></Teleport
    >
  </section>
</template>
<style scoped>
.metadata-match {
  margin-top: var(--space-24);
  padding-top: var(--space-20);
  border-top: 1px solid var(--border);
}
.metadata-match.compact {
  margin: 0;
  padding: 0;
  border: 0;
}
.metadata-match-heading,
.metadata-search,
.metadata-picker header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-12);
}
.metadata-match-heading h3 {
  margin: 4px 0 0;
  font-size: 17px;
}
.metadata-source,
.metadata-disclosure,
small {
  color: var(--muted);
  font-size: var(--type-small);
  line-height: 1.6;
}
.metadata-search {
  margin-top: var(--space-16);
}
.metadata-search input {
  min-width: 0;
  flex: 1;
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--surface-hover);
  color: var(--text);
}
.metadata-picker {
  margin: auto;
  width: min(850px, calc(100vw - 40px));
  max-height: calc(100dvh - 48px);
  overflow: auto;
  padding: 28px;
  background: var(--surface);
  color: var(--text);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-lg);
}
.metadata-picker::backdrop {
  background: #0005;
  backdrop-filter: blur(10px);
}
.metadata-picker h2 {
  margin: 0;
}
.metadata-provider {
  display: flex;
  gap: 8px;
  margin-top: 16px;
  flex-wrap: wrap;
}
.metadata-provider .active {
  color: var(--accent-ink);
  background: var(--surface-hover);
}
.metadata-candidates {
  display: grid;
  gap: 12px;
  list-style: none;
  padding: 0;
}
.metadata-candidates li {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
}
.candidate-cover {
  flex: 0 0 72px;
  width: 72px;
  height: 100px;
  border-radius: 8px;
  overflow: hidden;
}
.candidate-copy {
  display: grid;
  gap: 5px;
  flex: 1;
  min-width: 0;
  overflow-wrap: anywhere;
}
.metadata-error {
  color: var(--danger);
}
@media (max-width: 640px) {
  .metadata-picker {
    padding: 18px;
  }
  .metadata-candidates li {
    gap: 10px;
    flex-wrap: wrap;
  }
  .metadata-candidates li > button {
    margin-left: auto;
  }
  .metadata-match-heading {
    align-items: flex-start;
  }
}
</style>
