<script setup lang="ts">
import { computed, onUnmounted, reactive, ref, watch } from 'vue';
import HikarinagiReviewDialog from './HikarinagiReviewDialog.vue';
import { api, desktop, errorText, local } from '../stores/library';
import type { GameDetail } from '../types/domain';
import {
  createRatesLoader,
  type RatesWallState,
} from '../services/hikarinagiRates';
const props = defineProps<{ gameId: string }>();
const reviewDialog = ref<InstanceType<typeof HikarinagiReviewDialog>>();
const state = reactive<RatesWallState>({
  game_id: '',
  wall: null,
  loading: false,
  error: '',
});
const loader = createRatesLoader(
  state,
  (game_id, refresh) => api('get_hikarinagi_rates', { game_id, refresh }),
  errorText,
);
watch(
  [
    () => props.gameId,
    () => {
      const fields = (local.records[props.gameId] as GameDetail | undefined)
        ?.metadata;
      // A library summary refresh omits metadata; it is not an unbind.
      if (!fields) return null;
      return JSON.stringify(
        fields
          .filter(
            (field) =>
              field.provider === 'hikarinagi' && !field.manually_edited,
          )
          .map((field) => [field.field, field.value, field.fetched_at]),
      );
    },
  ],
  ([gameId, binding]) => {
    if (!desktop) return;
    if (binding === null) {
      if (state.game_id !== gameId) {
        loader.dispose();
        state.game_id = gameId;
        state.wall = null;
        state.loading = false;
        state.error = '';
      }
      return;
    }
    void loader.refresh(gameId);
  },
  { immediate: true },
);
onUnmounted(loader.dispose);
const bars = computed(() => {
  const rows = (state.wall?.distribution ?? [])
    .filter((row) => row.count > 0)
    .sort((a, b) => a.score - b.score);
  const peak = Math.max(1, ...rows.map((row) => row.count));
  return rows.map((row) => ({
    ...row,
    peak: row.count === peak,
    height: Math.max(10, Math.round((row.count / peak) * 100)),
  }));
});
const showHistogram = computed(
  () => (state.wall?.rated_count ?? 0) >= 4 && bars.value.length >= 2,
);
const statuses = computed(() =>
  [
    { key: 'completed' as const, label: '通关' },
    { key: 'going' as const, label: '在玩' },
    { key: 'on_hold' as const, label: '搁置' },
    { key: 'dropped' as const, label: '弃坑' },
  ]
    .map((row) => ({ ...row, count: state.wall?.status_counts[row.key] ?? 0 }))
    .filter((row) => row.count > 0),
);
const fetchedTime = computed(() => {
  if (!state.wall) return '';
  return new Date(state.wall.fetched_at).toLocaleString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  });
});
</script>
<template>
  <aside
    class="hikarinagi-wall"
    aria-labelledby="hikarinagi-wall-title"
    :aria-busy="state.loading"
  >
    <header class="wall-heading">
      <h2 id="hikarinagi-wall-title">安利墙</h2>
      <button
        v-if="desktop"
        class="wall-refresh"
        :disabled="state.loading"
        aria-label="刷新 Hikarinagi 安利墙"
        @click="loader.refresh(gameId, true)"
      >
        <svg
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="1.6"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <path
            d="M20 7v5h-5M4 17v-5h5M6 6a8 8 0 0 1 14 6M18 18a8 8 0 0 1-14-6"
          />
        </svg>
      </button>
    </header>
    <p v-if="state.loading" class="wall-empty" role="status">
      正在获取 Hikarinagi 安利墙…
    </p>
    <p v-else-if="!desktop" class="wall-empty">
      浏览器预览不获取社区评分。桌面版会展示已匹配作品的 Hikarinagi 安利墙。
    </p>
    <p v-else-if="!state.wall && !state.error" class="wall-empty">
      此作品尚未匹配 Hikarinagi，请先在「资料」中匹配作品。
    </p>
    <p v-if="state.error" class="wall-error" role="alert">{{ state.error }}</p>
    <template v-if="state.wall">
      <div class="wall-rating">
        <strong>{{
          state.wall.average == null ? '—' : state.wall.average.toFixed(1)
        }}</strong
        ><span>/ 10</span>
      </div>
      <div
        class="wall-stars"
        role="img"
        :aria-label="
          state.wall.average == null
            ? '暂无评分'
            : `${state.wall.average.toFixed(1)} 分，满分 10 分`
        "
      >
        <svg
          v-for="index in 10"
          :key="index"
          viewBox="0 0 24 24"
          aria-hidden="true"
          :class="{ filled: index <= Math.round(state.wall.average ?? 0) }"
        >
          <path
            d="m12 2.6 2.9 5.9 6.5.9-4.7 4.6 1.1 6.5-5.8-3.1-5.8 3.1 1.1-6.5-4.7-4.6 6.5-.9Z"
          />
        </svg>
      </div>
      <p class="wall-rater-count">{{ state.wall.rated_count }} 人评分</p>
      <div
        v-if="showHistogram"
        class="wall-histogram"
        role="img"
        :aria-label="`评分分布：${bars.map((bar) => `${bar.score} 分 ${bar.count} 人`).join('，')}`"
      >
        <div
          v-for="bar in bars"
          :key="bar.score"
          class="wall-bar-column"
          :class="{ peak: bar.peak }"
          :title="`${bar.score} 分 · ${bar.count} 人`"
        >
          <div class="wall-bar-track">
            <div class="wall-bar" :style="{ height: `${bar.height}%` }"></div>
          </div>
          <span>{{ bar.score }}</span>
        </div>
      </div>
      <button
        class="wall-rating-action"
        title="使用已登录的 Hikarinagi 账号评分与评论"
        @click="reviewDialog?.open()"
      >
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path
            d="m12 2.6 2.9 5.9 6.5.9-4.7 4.6 1.1 6.5-5.8-3.1-5.8 3.1 1.1-6.5-4.7-4.6 6.5-.9Z"
          /></svg
        >我来评分
      </button>
      <dl v-if="statuses.length" class="wall-statuses">
        <div v-for="status in statuses" :key="status.key">
          <dt>{{ status.label }}</dt>
          <dd>{{ status.count }}</dd>
        </div>
      </dl>
      <ul
        v-if="state.wall.keywords.length"
        class="wall-keywords"
        aria-label="Hikarinagi 安利墙热门标签"
      >
        <li v-for="keyword in state.wall.keywords" :key="keyword.word">
          <span aria-hidden="true">#</span><span>{{ keyword.word }}</span
          ><strong>{{ keyword.count }}</strong>
        </li>
      </ul>
      <p v-if="state.wall.message" class="wall-error" role="status">
        {{ state.wall.message }}
      </p>
      <footer class="wall-source">
        来源：Hikarinagi<br />{{ state.wall.cached ? '缓存于' : '更新于' }}
        {{ fetchedTime }}{{ state.wall.stale ? ' · 待更新' : '' }}<br /><span
          >Hikarinagi 社区评分</span
        >
      </footer>
    </template>
  </aside>
  <HikarinagiReviewDialog
    ref="reviewDialog"
    :game-id="gameId"
    :remote-id="state.wall?.remote_id ?? ''"
  />
</template>
<style scoped>
.hikarinagi-wall {
  --wall-stars: #f3a900;
  width: 300px;
  flex: 0 0 300px;
  align-self: flex-start;
  padding: 24px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--surface-glass);
  color: var(--text);
}
.wall-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 24px;
}
.wall-heading h2 {
  margin: 0;
  font-family: var(--font-body);
  font-size: 22px;
  font-weight: 700;
}
.wall-refresh {
  display: grid;
  place-items: center;
  padding: 6px;
  border: 0;
  border-radius: 6px;
  color: var(--muted);
  background: transparent;
}
.wall-refresh:hover {
  color: var(--text);
  background: var(--accent-wash);
}
.wall-rating {
  display: flex;
  align-items: baseline;
  gap: 6px;
  font-variant-numeric: tabular-nums;
}
.wall-rating strong {
  font-size: 58px;
  font-weight: 650;
  line-height: 1;
  letter-spacing: -0.04em;
}
.wall-rating > span {
  color: var(--muted);
  font-size: 18px;
}
.wall-stars {
  display: flex;
  gap: 3px;
  margin-top: 10px;
}
.wall-stars svg {
  width: 18px;
  height: 18px;
  fill: none;
  stroke: var(--border-strong);
  stroke-width: 2;
  stroke-linejoin: round;
}
.wall-stars svg.filled {
  fill: var(--wall-stars);
  stroke: var(--wall-stars);
}
.wall-rater-count {
  margin: 10px 0 0;
  color: var(--muted);
  font-size: 14px;
}
.wall-histogram {
  display: flex;
  gap: 5px;
  margin: 22px 0;
}
.wall-bar-column {
  flex: 1;
  min-width: 0;
  max-width: 56px;
  text-align: center;
  color: var(--muted);
  font-variant-numeric: tabular-nums;
  font-size: 14px;
}
.wall-bar-track {
  height: 58px;
  display: flex;
  align-items: flex-end;
}
.wall-bar {
  width: 100%;
  border-radius: 8px;
  background: color-mix(in srgb, var(--accent) 30%, transparent);
}
.wall-bar-column.peak {
  color: var(--text);
  font-weight: 600;
}
.wall-bar-column.peak .wall-bar {
  background: var(--accent);
}
.wall-bar-column > span {
  display: block;
  margin-top: 9px;
}
.wall-rating-action {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  width: 100%;
  min-height: 43px;
  margin-top: 20px;
  border: 1px solid var(--border-strong);
  border-radius: 10px;
  background: transparent;
  color: var(--text);
  font-size: 17px;
  font-weight: 600;
}
.wall-rating-action:hover {
  background: var(--accent-wash);
}
.wall-rating-action svg {
  width: 20px;
  height: 20px;
  stroke: currentColor;
  fill: none;
  stroke-width: 2;
  stroke-linejoin: round;
}
.wall-statuses {
  display: grid;
  gap: 10px;
  margin: 22px 0 0;
  padding-top: 22px;
  border-top: 1px solid var(--border);
}
.wall-statuses > div {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  font-size: 15px;
}
.wall-statuses dt {
  color: var(--muted);
}
.wall-statuses dd {
  margin: 0;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}
.wall-keywords {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  padding: 20px 0 0;
  margin: 22px 0 0;
  list-style: none;
  border-top: 1px solid var(--border);
}
.wall-keywords li {
  display: inline-flex;
  align-items: baseline;
  gap: 5px;
  max-width: 100%;
  padding: 3px 9px;
  border: 1px solid var(--border-strong);
  border-radius: 999px;
  font-size: 13px;
  overflow-wrap: anywhere;
}
.wall-keywords li > span:first-child {
  color: var(--subtle);
}
.wall-keywords strong {
  font-variant-numeric: tabular-nums;
}
.wall-source {
  margin-top: 18px;
  color: var(--subtle);
  font-size: 10px;
  line-height: 1.7;
}
.wall-empty,
.wall-error {
  color: var(--muted);
  font-size: 12px;
  line-height: 1.8;
}
.wall-error {
  margin-top: 14px;
}
@media (max-width: 1100px) {
  .hikarinagi-wall {
    width: 260px;
    flex-basis: 260px;
    padding: 20px;
  }
}
@media (max-width: 780px) {
  .hikarinagi-wall {
    width: 100%;
    flex-basis: auto;
  }
}
</style>
