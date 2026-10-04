<script setup lang="ts">
import { computed, ref, useId } from 'vue';
import { RouterLink } from 'vue-router';
import { useActivity } from '../../composables/useActivity';
import {
  ACTIVITY_RANGES,
  addDays,
  compactDuration,
  dateLabel,
  rangeLabel,
  sessionEnd,
  utcDay,
  utcTime,
} from '../../services/activity';
import { durationText, sessionReason } from '../../services/playtime';
import { coverSource, desktop, preview } from '../../stores/library';
import type { ActivityRange } from '../../types/activity';
import type { PlaySession } from '../../types/playtime';
import PreviewIcon from '../preview/PreviewIcon.vue';
import PreviewCover from '../preview/PreviewCover.vue';
import ActivityTrend from './ActivityTrend.vue';
import ActivityCalendar from './ActivityCalendar.vue';
import ActivityCorrection from './ActivityCorrection.vue';
import ActivityTooltip from './ActivityTooltip.vue';
import './activity.css';
const {
  range,
  date,
  snapshot,
  loading,
  error,
  outdated,
  load,
  selectRange,
  selectYear,
  selectDate,
  selectPage,
} = useActivity();
const correction = ref<InstanceType<typeof ActivityCorrection>>();
const hourTipId = useId();
const hourTarget = ref<Element | null>(null);
const hourActive = ref<{ hour: number; duration_seconds: number } | null>(null);
function showHour(
  event: Event,
  value: { hour: number; duration_seconds: number },
) {
  hourTarget.value = event.currentTarget as Element;
  hourActive.value = value;
}
function hideHour() {
  hourTarget.value = null;
  hourActive.value = null;
}
const expandedGames = ref(false),
  expandedHours = ref(false);
const selectedIndex = computed(() =>
  ACTIVITY_RANGES.findIndex((r) => r.id === range.value),
);
const games = computed(
  () => snapshot.value?.games.slice(0, expandedGames.value ? 10 : 5) ?? [],
);
const hours = computed(() => {
  const values = snapshot.value?.hours ?? [];
  return expandedHours.value
    ? values
    : [...values]
        .filter((h) => h.duration_seconds > 0)
        .sort(
          (a, b) => b.duration_seconds - a.duration_seconds || a.hour - b.hour,
        )
        .slice(0, 3);
});
const maxHour = computed(() =>
  Math.max(1, ...(snapshot.value?.hours.map((h) => h.duration_seconds) ?? [])),
);
const sessionGroups = computed(() => {
  const groups: { date: string; sessions: PlaySession[] }[] = [];
  for (const s of snapshot.value?.sessions.items ?? []) {
    const day = utcDay(s.started_at);
    if (groups.at(-1)?.date !== day) groups.push({ date: day, sessions: [] });
    groups.at(-1)!.sessions.push(s);
  }
  return groups;
});
const totalParts = computed(() => {
  const text = compactDuration(snapshot.value?.summary.total_seconds ?? 0);
  const match = text.match(/^(<*\d+)(.*)$/);
  return { number: match?.[1] ?? text, unit: match?.[2] ?? '' };
});
function cover(id: string) {
  return coverSource(
    snapshot.value?.games.find((g) => g.game_id === id)?.cover_url ??
      preview.games.find((g) => g.game_id === id)?.cover_url ??
      null,
  );
}
function changeRange(value: ActivityRange) {
  expandedGames.value = false;
  expandedHours.value = false;
  selectRange(value);
}
function rangeKey(e: KeyboardEvent, index: number) {
  if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(e.key)) return;
  e.preventDefault();
  const next =
    e.key === 'Home'
      ? 0
      : e.key === 'End'
        ? 4
        : (index + (e.key === 'ArrowRight' ? 1 : -1) + 5) % 5;
  changeRange(ACTIVITY_RANGES[next].id);
  (e.currentTarget as HTMLElement).parentElement
    ?.querySelector<HTMLButtonElement>(
      `[data-range="${ACTIVITY_RANGES[next].id}"]`,
    )
    ?.focus();
}
</script>
<template>
  <div class="activity-dashboard">
    <div class="activity-toolbar">
      <div
        class="activity-ranges"
        role="group"
        aria-label="游玩统计时间范围"
        :style="{ '--range-index': selectedIndex }"
      >
        <button
          v-for="(item, index) in ACTIVITY_RANGES"
          :key="item.id"
          :data-range="item.id"
          :aria-pressed="range === item.id"
          :tabindex="range === item.id ? 0 : -1"
          @click="changeRange(item.id)"
          @keydown="rangeKey($event, index)"
        >
          {{ item.label }}
        </button>
      </div>
      <button
        class="activity-refresh quiet-button"
        :disabled="loading"
        aria-label="刷新活动记录"
        title="刷新活动记录"
        @click="load"
      >
        <PreviewIcon name="activity" :size="15" /><span>{{
          loading ? '读取中' : '刷新'
        }}</span>
      </button>
    </div>
    <div class="activity-context">
      <span v-if="snapshot"
        >{{ snapshot.range_start.replaceAll('-', '.') }} —
        {{ addDays(snapshot.range_end, -1).replaceAll('-', '.') }}</span
      ><span v-else>正在读取游玩记录</span
      ><span>{{
        desktop ? '日期按 UTC 统计' : '隔离演示数据 · 日期按 UTC 统计'
      }}</span>
    </div>
    <p v-if="error" class="activity-error-banner" role="alert">
      {{ error }}<button class="quiet-button" @click="load">重试</button
      ><span v-if="snapshot"
        >保留的是上次读取的{{ rangeLabel(snapshot.range) }}记录。</span
      >
    </p>
    <div
      v-if="!snapshot && loading"
      class="activity-skeleton"
      role="status"
      aria-label="正在读取活动记录"
    >
      <div class="skeleton-metrics"><i v-for="n in 4" :key="n"></i></div>
      <div class="skeleton-chart"></div>
      <div class="skeleton-calendar"></div>
      <span class="sr-only">正在读取活动记录</span>
    </div>
    <div
      v-if="snapshot"
      class="activity-content"
      :class="{ 'is-updating': outdated }"
      :aria-busy="loading"
    >
      <section class="activity-metrics" aria-label="所选范围游玩概览">
        <div
          class="activity-metric activity-metric-time"
          :title="durationText(snapshot.summary.total_seconds)"
        >
          <span class="metric-symbol"
            ><PreviewIcon name="clock" :size="15" /></span
          ><strong data-metric="total"
            ><span>{{ totalParts.number }}</span
            ><small>{{ totalParts.unit }}</small></strong
          ><span class="metric-label">总时长</span
          ><span class="metric-detail"
            >{{ rangeLabel(snapshot.range) }}的故事时间</span
          >
        </div>
        <div class="activity-metric">
          <span class="metric-symbol"
            ><PreviewIcon name="games" :size="15" /></span
          ><strong data-metric="played"
            >{{ snapshot.summary.played_count }}<small>部</small></strong
          ><span class="metric-label">游玩作品</span
          ><span class="metric-detail">每部作品只计一次</span>
        </div>
        <div class="activity-metric" title="所选范围内玩过，且当前已完成的作品">
          <span class="metric-symbol"
            ><PreviewIcon name="check" :size="15" /></span
          ><strong data-metric="completed"
            >{{ snapshot.summary.completed_count }}<small>部</small></strong
          ><span class="metric-label">已完成</span
          ><span class="metric-detail">区间内玩过 · 当前已完成</span>
        </div>
        <div class="activity-metric">
          <span class="metric-symbol"
            ><PreviewIcon name="sun" :size="15" /></span
          ><strong data-metric="days"
            >{{ snapshot.summary.active_days }}<small>天</small></strong
          ><span class="metric-label">活跃天数</span
          ><span class="metric-detail">有游玩时长的日子</span>
        </div>
      </section>
      <div v-if="!snapshot.summary.session_count" class="activity-empty-notice">
        <PreviewIcon name="spark" :size="18" />
        <p>
          这段时间还没有游玩记录。<span
            >选择更长的范围，或从游戏页开启下一段故事。</span
          >
        </p>
        <RouterLink :to="{ name: 'games' }" class="quiet-button"
          >前往游戏 <PreviewIcon name="arrow" :size="14"
        /></RouterLink>
      </div>
      <ActivityTrend :snapshot="snapshot" />
      <ActivityCalendar
        :snapshot="snapshot"
        :selected="snapshot.session_date"
        :loading="loading"
        @select="selectDate"
        @year="selectYear"
      />
      <div class="activity-two-columns">
        <section
          class="activity-section activity-ranking"
          aria-labelledby="ranking-title"
        >
          <header class="activity-section-heading">
            <div>
              <p class="activity-kicker">STORIES YOU STAYED WITH</p>
              <h2 id="ranking-title">游玩最多</h2>
            </div>
            <span class="activity-note">{{ rangeLabel(snapshot.range) }}</span>
          </header>
          <ol v-if="games.length" class="activity-ranking-list">
            <li v-for="(game, index) in games" :key="game.game_id">
              <RouterLink
                :to="{ name: 'game-detail', params: { game_id: game.game_id } }"
                class="activity-ranking-row"
                :title="`${game.title} · ${durationText(game.duration_seconds)}`"
                ><span class="ranking-number">{{
                  String(index + 1).padStart(2, '0')
                }}</span
                ><PreviewCover
                  class="activity-mini-cover"
                  :cover_url="coverSource(game.cover_url)"
                  :title="game.title"
                />
                <div class="ranking-story">
                  <span>{{ game.title }}</span>
                  <div class="ranking-track" aria-hidden="true">
                    <i
                      :style="{
                        width: `${(game.duration_seconds / (snapshot.games[0]?.duration_seconds || 1)) * 100}%`,
                      }"
                    ></i>
                  </div>
                </div>
                <strong>{{
                  compactDuration(game.duration_seconds)
                }}</strong></RouterLink
              >
            </li>
          </ol>
          <p v-else class="activity-local-empty">
            等下一段故事，在这里留下名字。
          </p>
          <button
            v-if="snapshot.games.length > 5"
            class="activity-expand"
            :aria-expanded="expandedGames"
            @click="expandedGames = !expandedGames"
          >
            {{ expandedGames ? '收起排行' : '查看前十名'
            }}<PreviewIcon name="chevron-down" :size="14" />
          </button>
        </section>
        <section
          class="activity-section activity-hours"
          aria-labelledby="hours-title"
        >
          <header class="activity-section-heading">
            <div>
              <p class="activity-kicker">YOUR FAVORITE HOURS</p>
              <h2 id="hours-title">游玩时间分布</h2>
            </div>
            <PreviewIcon name="moon" :size="18" />
          </header>
          <p class="activity-hours-caption">
            按会话开始时段汇总
            <span
              >· {{ expandedHours ? '完整 24 小时' : '最常开始的时段' }}</span
            >
          </p>
          <ol
            v-if="hours.length"
            class="activity-hour-list"
            :class="{ 'is-expanded': expandedHours }"
          >
            <li
              v-for="hour in hours"
              :key="hour.hour"
              tabindex="0"
              :aria-describedby="hourTarget ? hourTipId : undefined"
              :title="`${String(hour.hour).padStart(2, '0')}:00 开始的会话：${durationText(hour.duration_seconds)}，占总时长${snapshot.summary.total_seconds ? ((hour.duration_seconds / snapshot.summary.total_seconds) * 100).toFixed(1) : '0'}%`"
              @pointerenter="showHour($event, hour)"
              @pointerleave="hideHour"
              @focus="showHour($event, hour)"
              @blur="hideHour"
              @click="showHour($event, hour)"
            >
              <span>{{ String(hour.hour).padStart(2, '0') }}:00</span>
              <div
                class="hour-track"
                role="img"
                :aria-label="`${hour.hour}点开始，累计${durationText(hour.duration_seconds)}`"
              >
                <i
                  :style="{
                    width: `${(hour.duration_seconds / maxHour) * 100}%`,
                  }"
                ></i>
              </div>
              <strong>{{ compactDuration(hour.duration_seconds) }}</strong>
            </li>
          </ol>
          <p v-else class="activity-local-empty">
            还没有记录下偏爱的游玩时段。
          </p>
          <button
            v-if="snapshot.summary.total_seconds"
            class="activity-expand"
            :aria-expanded="expandedHours"
            @click="expandedHours = !expandedHours"
          >
            {{ expandedHours ? '收起时段' : '查看 24 小时'
            }}<PreviewIcon name="chevron-down" :size="14" />
          </button>
        </section>
      </div>
      <section
        class="activity-section activity-journal"
        aria-labelledby="journal-title"
      >
        <header class="activity-section-heading">
          <div>
            <p class="activity-kicker">THE LITTLE MOMENTS, KEPT</p>
            <h2 id="journal-title">最近活动</h2>
          </div>
          <span class="activity-note"
            >{{ snapshot.sessions.total }} 次会话
            <template v-if="snapshot.summary.active_session_count"
              >· {{ snapshot.summary.active_session_count }} 个进行中</template
            ></span
          >
        </header>
        <div v-if="date" class="activity-date-filter">
          <PreviewIcon name="clock" :size="14" /><span>{{
            snapshot.session_date ?? date
          }}</span
          ><button
            class="quiet-button"
            :disabled="loading"
            @click="selectDate(null)"
          >
            清除筛选 <PreviewIcon name="close" :size="12" />
          </button>
        </div>
        <div v-if="!sessionGroups.length" class="activity-local-empty">
          {{
            snapshot.session_date
              ? '这一天没有游玩会话。清除日期筛选，继续看看其他故事。'
              : '每一次启程，都会留在这里。'
          }}
        </div>
        <div
          v-for="group in sessionGroups"
          :key="group.date"
          class="journal-day"
        >
          <h3>
            {{ dateLabel(group.date, snapshot.as_of)
            }}<small
              v-if="
                ['今天', '昨天'].includes(dateLabel(group.date, snapshot.as_of))
              "
              >{{ group.date.replaceAll('-', '.') }}</small
            >
          </h3>
          <ol>
            <li
              v-for="session in group.sessions"
              :key="session.id"
              class="journal-session"
              :data-session="session.id"
            >
              <span
                class="journal-node"
                :class="{ 'is-running': !session.ended_at }"
                aria-hidden="true"
              ></span
              ><RouterLink
                :to="{
                  name: 'game-detail',
                  params: { game_id: session.game_id },
                }"
                class="journal-game-link"
                ><PreviewCover
                  class="activity-mini-cover"
                  :cover_url="cover(session.game_id)"
                  :title="session.game_title"
                />
                <div>
                  <strong :title="session.game_title">{{
                    session.game_title
                  }}</strong
                  ><span class="journal-time"
                    >{{ utcTime(session.started_at) }} —
                    {{ sessionEnd(session.started_at, session.ended_at) }}
                    <i>·</i>
                    {{ compactDuration(session.duration_seconds) }}</span
                  >
                </div></RouterLink
              >
              <div class="journal-actions">
                <span v-if="!session.ended_at" class="journal-badge"
                  >计时中</span
                ><span
                  v-else-if="session.corrected"
                  class="journal-badge"
                  :title="session.correction_reason ?? ''"
                  >已修正</span
                >
                <details
                  v-if="
                    session.corrected ||
                    !['process_exit', 'tracked_process_exit', null].includes(
                      session.end_reason,
                    )
                  "
                  class="journal-details"
                >
                  <summary>记录说明</summary>
                  <p>
                    {{ sessionReason(session.end_reason)
                    }}{{
                      session.corrected ? ` · ${session.correction_reason}` : ''
                    }}
                  </p>
                </details>
                <button
                  v-if="session.ended_at && desktop"
                  class="journal-edit quiet-button"
                  :aria-label="`修正${session.game_title}这次时长`"
                  @click="correction?.open(session, $event)"
                >
                  修正时长
                </button>
              </div>
            </li>
          </ol>
        </div>
        <nav
          v-if="snapshot.sessions.total > 20"
          class="journal-pagination"
          aria-label="游玩会话分页"
        >
          <button
            class="quiet-button"
            :disabled="snapshot.sessions.page <= 1 || loading"
            @click="selectPage(snapshot.sessions.page - 1)"
          >
            上一页</button
          ><span
            >{{ snapshot.sessions.page }} /
            {{ Math.ceil(snapshot.sessions.total / 20) }}</span
          ><button
            class="quiet-button"
            :disabled="
              snapshot.sessions.page * 20 >= snapshot.sessions.total || loading
            "
            @click="selectPage(snapshot.sessions.page + 1)"
          >
            下一页
          </button>
        </nav>
        <p v-if="!desktop" class="activity-demo-footnote">
          这里是隔离的演示游玩日志。正式记录由客户端读取，修正仅在客户端可用。
        </p>
      </section>
      <footer class="activity-footnote">
        <PreviewIcon name="info" :size="13" />
        <p>
          按 UTC 会话开始日记录，时长以已保存记录为准。<span
            >跨日会话归于开始日，进行中的时间随检查点更新。</span
          >
        </p>
      </footer>
    </div>
    <ActivityTooltip
      :id="hourTipId"
      :target="hourTarget"
      :title="
        hourActive
          ? `${String(hourActive.hour).padStart(2, '0')}:00 开始的会话`
          : ''
      "
      :text="
        hourActive
          ? `${durationText(hourActive.duration_seconds)} · 占总时长${snapshot?.summary.total_seconds ? ((hourActive.duration_seconds / snapshot.summary.total_seconds) * 100).toFixed(1) : '0'}%`
          : ''
      "
      @dismiss="hideHour"
    />
    <ActivityCorrection ref="correction" @saved="load" />
  </div>
</template>
