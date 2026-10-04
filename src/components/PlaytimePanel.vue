<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { RouterLink } from 'vue-router';
import {
  api,
  desktop,
  errorText,
  notify,
  refreshLibrary,
} from '../stores/library';
import { subscribeEvent } from '../services/events';
import {
  correctionSeconds,
  durationText,
  sessionReason,
} from '../services/playtime';
import type { PlaySession, PlaytimeStats } from '../types/playtime';
const props = defineProps<{ gameId?: string }>();
const sessions = ref<PlaySession[]>([]);
const stats = ref<PlaytimeStats | null>(null);
const page = ref(1);
const total = ref(0);
const days = ref(30);
const loading = ref(false);
const error = ref('');
const editing = ref<PlaySession | null>(null);
const hours = ref(0);
const minutes = ref(0);
const seconds = ref(0);
const reason = ref('');
const saving = ref(false);
const confirmed = ref(false);
const editorHeading = ref<HTMLElement | null>(null);
const controller = new AbortController();
let revision = 0;
const chartMax = computed(() =>
  Math.max(1, ...(stats.value?.daily.map((d) => d.duration_seconds) ?? [])),
);
const correctedSeconds = computed(() =>
  correctionSeconds(hours.value, minutes.value, seconds.value),
);
const canSave = computed(
  () =>
    correctedSeconds.value !== null &&
    reason.value.trim().length > 0 &&
    confirmed.value &&
    !saving.value,
);
function dateText(value: string) {
  return new Date(value).toLocaleString('zh-CN', { hour12: false });
}
async function load() {
  if (!desktop) return;
  const ticket = ++revision;
  loading.value = true;
  error.value = '';
  try {
    const [records, summary] = await Promise.all([
      api('list_play_sessions', {
        game_id: props.gameId ?? null,
        page: page.value,
        page_size: 20,
      }),
      api('get_playtime_stats', {
        game_id: props.gameId ?? null,
        days: days.value,
      }),
    ]);
    if (ticket !== revision) return;
    sessions.value = records.items;
    total.value = records.total;
    stats.value = summary;
  } catch (e) {
    if (ticket === revision) error.value = errorText(e);
  } finally {
    if (ticket === revision) loading.value = false;
  }
}
function edit(session: PlaySession) {
  editing.value = session;
  hours.value = Math.floor(session.duration_seconds / 3600);
  minutes.value = Math.floor((session.duration_seconds % 3600) / 60);
  seconds.value = session.duration_seconds % 60;
  reason.value = '';
  confirmed.value = false;
  void nextTick(() => editorHeading.value?.focus());
}
async function save() {
  const session = editing.value;
  if (!session || !canSave.value || correctedSeconds.value === null) return;
  saving.value = true;
  try {
    await api('correct_play_session', {
      session_id: session.id,
      expected_duration_seconds: session.duration_seconds,
      duration_seconds: correctedSeconds.value,
      reason: reason.value.trim(),
      confirmed: true,
    });
    editing.value = null;
    notify('游玩时长已修正。');
    await Promise.all([load(), refreshLibrary()]);
  } catch (e) {
    notify(errorText(e));
  } finally {
    saving.value = false;
  }
}
watch(
  () => props.gameId,
  () => {
    page.value = 1;
    editing.value = null;
    void load();
  },
  { immediate: true },
);
watch([page, days], () => void load());
if (desktop) {
  for (const event of ['session_started', 'session_ended'] as const) {
    void subscribeEvent(event, () => void load(), controller.signal).catch(
      () => {},
    );
  }
}
const timer = desktop
  ? setInterval(() => {
      if (
        !document.hidden &&
        !loading.value &&
        !saving.value &&
        stats.value?.active_session_count
      )
        void load();
    }, 15000)
  : undefined;
onBeforeUnmount(() => {
  ++revision;
  controller.abort();
  clearInterval(timer);
});
</script>
<template>
  <div class="playtime-panel">
    <div v-if="!desktop" class="empty-stage">
      <p class="eyebrow">PLAYTIME JOURNAL</p>
      <h2>游玩的时间，会留在这里。</h2>
      <p>浏览器预览没有真实游玩记录，请在 Windows 客户端启动游戏后查看。</p>
    </div>
    <template v-else>
      <div class="playtime-toolbar">
        <h2>游玩记录</h2>
        <div>
          <label
            >统计范围
            <select v-model.number="days" :disabled="loading">
              <option :value="7">最近 7 天</option>
              <option :value="30">最近 30 天</option>
              <option :value="90">最近 90 天</option>
            </select></label
          >
          <button class="quiet-button" :disabled="loading" @click="load">
            {{ loading ? '读取中…' : '刷新' }}
          </button>
        </div>
      </div>
      <p v-if="error" role="alert" class="playtime-error">
        {{ error }} <button class="quiet-button" @click="load">重试</button>
      </p>
      <template v-else-if="stats">
        <div class="playtime-summary">
          <div>
            <span>累计游玩</span
            ><strong>{{ durationText(stats.total_seconds) }}</strong>
          </div>
          <div>
            <span>最近 {{ days }} 天</span
            ><strong>{{ durationText(stats.period_seconds) }}</strong>
          </div>
          <div>
            <span>会话次数</span
            ><strong
              >{{ stats.session_count
              }}<small v-if="stats.active_session_count">
                · {{ stats.active_session_count }} 个进行中</small
              ></strong
            >
          </div>
        </div>
        <figure class="playtime-chart">
          <div
            class="playtime-bars"
            role="list"
            :aria-label="`最近 ${days} 天的每日游玩时长`"
          >
            <div
              v-for="day in stats.daily"
              :key="day.date"
              class="playtime-bar-slot"
              role="listitem"
              tabindex="0"
              :aria-label="`${day.date}：${durationText(day.duration_seconds)}`"
              :title="`${day.date}：${durationText(day.duration_seconds)}`"
            >
              <span
                :style="{
                  height: `${day.duration_seconds ? Math.max(3, (day.duration_seconds / chartMax) * 100) : 0}%`,
                }"
              ></span>
            </div>
          </div>
          <figcaption>
            <span>{{ stats.daily[0]?.date }}</span
            ><span>{{ stats.daily.at(-1)?.date }}</span>
          </figcaption>
        </figure>
        <p class="playtime-hint">
          图表按 UTC 日期、会话开始日统计；进行中的时间每 15
          秒保存一次。客户端中断后仅保留已保存的时间。
        </p>
        <div v-if="!gameId && stats.games.length" class="playtime-ranking">
          <h3>这段时间的故事</h3>
          <RouterLink
            v-for="game in stats.games"
            :key="game.game_id"
            :to="{ name: 'game-detail', params: { game_id: game.game_id } }"
            ><span>{{ game.title }}</span
            ><strong>{{
              durationText(game.duration_seconds)
            }}</strong></RouterLink
          >
        </div>
        <section
          v-if="editing"
          class="playtime-editor"
          aria-label="修正游玩时长"
        >
          <h3 ref="editorHeading" tabindex="-1">
            修正 {{ editing.game_title }} 的这次时长
          </h3>
          <p>
            原记录：{{ durationText(editing.duration_seconds) }}（{{
              dateText(editing.started_at)
            }}）
          </p>
          <form @submit.prevent="save">
            <div class="playtime-fields">
              <label
                >小时<input
                  v-model.number="hours"
                  type="number"
                  min="0"
                  max="87600"
                  step="1"
                  required
                  :disabled="saving" /></label
              ><label
                >分钟<input
                  v-model.number="minutes"
                  type="number"
                  min="0"
                  max="59"
                  step="1"
                  required
                  :disabled="saving" /></label
              ><label
                >秒<input
                  v-model.number="seconds"
                  type="number"
                  min="0"
                  max="59"
                  step="1"
                  required
                  :disabled="saving"
              /></label>
            </div>
            <label class="playtime-reason"
              >修正原因<input
                v-model="reason"
                maxlength="500"
                required
                placeholder="例如：启动器提前退出，补记实际游玩时间"
                :disabled="saving"
            /></label>
            <label class="playtime-confirm"
              ><input
                v-model="confirmed"
                type="checkbox"
                :disabled="saving"
              />确认将本次会话更新为
              {{
                correctedSeconds === null
                  ? '有效时长待填写'
                  : durationText(correctedSeconds)
              }}</label
            >
            <div>
              <button class="primary-button" :disabled="!canSave" type="submit">
                {{ saving ? '保存中…' : '保存修正' }}</button
              ><button
                class="quiet-button"
                type="button"
                :disabled="saving"
                @click="editing = null"
              >
                取消
              </button>
            </div>
          </form>
        </section>
        <div v-if="!sessions.length" class="playtime-empty">
          还没有游玩记录。通过软件启动游戏后，这里会记录每次会话。
        </div>
        <ol v-else class="playtime-sessions">
          <li v-for="session in sessions" :key="session.id">
            <div>
              <RouterLink
                v-if="!gameId"
                :to="{
                  name: 'game-detail',
                  params: { game_id: session.game_id },
                }"
                >{{ session.game_title }}</RouterLink
              ><strong v-else>{{ session.game_title }}</strong>
              <p>
                {{ dateText(session.started_at) }} →
                {{ session.ended_at ? dateText(session.ended_at) : '进行中' }}
              </p>
              <span
                >{{ sessionReason(session.end_reason)
                }}<template v-if="session.process_name">
                  · {{ session.process_name }}</template
                ></span
              >
              <p v-if="session.corrected" class="playtime-hint">
                手动修正：{{ session.correction_reason }}
              </p>
            </div>
            <div class="playtime-session-actions">
              <strong>{{ durationText(session.duration_seconds) }}</strong
              ><button
                v-if="session.ended_at"
                class="quiet-button"
                :disabled="saving"
                @click="edit(session)"
              >
                修正时长</button
              ><span v-else class="mini-badge">计时中</span>
            </div>
          </li>
        </ol>
        <div v-if="total > 20" class="playtime-pagination">
          <button
            class="quiet-button"
            :disabled="page === 1 || loading"
            @click="page--"
          >
            上一页</button
          ><span
            >{{ page }} / {{ Math.ceil(total / 20) }} · {{ total }} 条记录</span
          ><button
            class="quiet-button"
            :disabled="page * 20 >= total || loading"
            @click="page++"
          >
            下一页
          </button>
        </div>
      </template>
      <p v-else-if="loading" role="status">正在读取游玩记录…</p>
    </template>
  </div>
</template>
<style scoped>
.playtime-panel {
  width: 100%;
  min-width: 0;
}
.playtime-toolbar,
.playtime-toolbar > div,
.playtime-pagination {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  flex-wrap: wrap;
}
.playtime-toolbar select,
input:not([type='checkbox']) {
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text);
  border-radius: 8px;
  padding: 8px 10px;
  font: inherit;
}
.playtime-summary {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 20px;
  margin: 24px 0;
}
.playtime-summary > div {
  padding: 20px;
  border-radius: 18px;
  background: var(--surface);
  border: 1px solid var(--border);
}
.playtime-summary span,
.playtime-summary strong {
  display: block;
}
.playtime-summary span,
.playtime-hint,
.playtime-sessions p,
.playtime-sessions span {
  color: var(--muted);
}
.playtime-summary strong {
  font-size: 22px;
  margin-top: 8px;
}
.playtime-summary small {
  font-size: 12px;
  font-weight: normal;
}
.playtime-chart {
  margin: 28px 0 12px;
}
.playtime-bars {
  height: 140px;
  display: flex;
  align-items: end;
  gap: 3px;
  border-bottom: 1px solid var(--border);
}
.playtime-bar-slot {
  flex: 1;
  height: 100%;
  display: flex;
  align-items: end;
  min-width: 1px;
}
.playtime-bar-slot span {
  width: 100%;
  background: var(--accent);
  border-radius: 5px 5px 0 0;
  opacity: 0.75;
}
.playtime-bar-slot:focus-visible {
  outline: 2px solid var(--accent);
}
figcaption {
  display: flex;
  justify-content: space-between;
  color: var(--muted);
  font-size: 12px;
  margin-top: 8px;
}
.playtime-hint {
  font-size: 12px;
  line-height: 1.7;
}
.playtime-ranking {
  margin: 28px 0;
}
.playtime-ranking a {
  display: flex;
  justify-content: space-between;
  gap: 16px;
  padding: 10px 0;
  border-bottom: 1px solid var(--border);
}
.playtime-ranking strong {
  white-space: nowrap;
}
.playtime-sessions {
  list-style: none;
  padding: 0;
  margin: 24px 0;
}
.playtime-sessions li {
  padding: 18px 0;
  display: flex;
  justify-content: space-between;
  gap: 24px;
  border-bottom: 1px solid var(--border);
}
.playtime-sessions p {
  margin: 8px 0;
  font-size: 13px;
}
.playtime-sessions span {
  font-size: 12px;
}
.playtime-session-actions {
  display: flex;
  flex-direction: column;
  align-items: end;
  gap: 8px;
  flex-shrink: 0;
}
.playtime-editor {
  margin: 24px 0;
  padding: 24px;
  border: 1px solid var(--border);
  border-radius: 18px;
  background: var(--surface);
}
.playtime-fields {
  display: flex;
  gap: 16px;
  margin: 16px 0;
}
.playtime-fields label {
  display: grid;
  gap: 6px;
}
.playtime-fields input {
  width: 100px;
}
.playtime-reason {
  display: grid;
  gap: 8px;
}
.playtime-confirm {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 16px 0;
}
.playtime-empty {
  text-align: center;
  padding: 48px 16px;
  color: var(--muted);
}
.playtime-error {
  color: var(--text);
  padding: 16px;
  border: 1px solid var(--accent);
  border-radius: 12px;
}
@media (max-width: 800px) {
  .playtime-summary {
    grid-template-columns: 1fr;
    gap: 10px;
  }
  .playtime-sessions li {
    flex-direction: column;
    gap: 10px;
  }
  .playtime-session-actions {
    flex-direction: row;
    align-items: center;
  }
}
</style>
