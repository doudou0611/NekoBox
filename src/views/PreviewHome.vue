<script setup lang="ts">
import {
  computed,
  nextTick,
  onMounted,
  onBeforeUnmount,
  ref,
  reactive,
  watch,
} from 'vue';
import { RouterLink, useRouter } from 'vue-router';
import { useIntersectionObserver } from '@vueuse/core';
import {
  preview,
  local,
  desktop,
  api,
  notify,
  errorText,
  openImport,
  refreshLibrary,
  launchGame,
  launchInstallation,
  setRecommendationPreference,
} from '../stores/library';
import {
  useHomeDashboard,
  homePosition,
} from '../composables/useHomeDashboard';
import { useCoverAtmosphere } from '../composables/useCoverAtmosphere';
import { openPreviewGame } from '../composables/useSharedTransition';
import {
  compactTime,
  localDate,
  greeting,
  relativePlayed,
  calendarEvents,
  randomCandidate,
} from '../services/homeDashboard';
import { selectGalleryGroup } from '../services/groupNavigation';
import {
  homeSearchIntent,
  homeFavoriteIntent,
} from '../services/homeNavigation';
import { demoHomePreferences, demoCalendarGames } from '../preview/home';
import type { PreviewGame } from '../preview/data';
import { bangumi } from '../stores/bangumi';
import { hikariAccount } from '../stores/hikarinagiAccount';
import { STATUS_LABELS } from '../preview/data';
import type {
  GameStatus,
  Recommendation,
  RecommendationPreferenceEntry,
  SourcedField,
} from '../types/domain';
import CoverCard from '../components/preview/CoverCard.vue';
import PreviewCover from '../components/preview/PreviewCover.vue';
import PreviewIcon from '../components/preview/PreviewIcon.vue';
import HomeStrip from '../components/home/HomeStrip.vue';
import HomeDialog from '../components/home/HomeDialog.vue';
const router = useRouter();
const {
  now,
  dashboard,
  counts,
  active,
  games,
  hero,
  wanted,
  recent,
  playable,
  loading,
  error,
  refresh,
} = useHomeDashboard();
const first_visit = !preview.home_visited;
preview.home_visited = true;
const { atmosphere } = useCoverAtmosphere(
  () => (preview.missing_cover ? undefined : hero.value?.cover_url),
  () => preview.theme,
);
const running = computed(
  () => !!hero.value && active.value.includes(hero.value.game_id),
);
const new_story = computed(
  () => hero.value?.status === 'not_started' && !running.value,
);
const hero_label = computed(() =>
  running.value ? '正在运行' : new_story.value ? '开启新故事' : '继续游玩',
);
const mainCounts: [GameStatus | 'all', string][] = [
  ['all', '全部'],
  ['playing', '游玩中'],
  ['completed', '已完成'],
  ['not_started', '未开始'],
];
const otherCounts: GameStatus[] = ['paused', 'dropped', 'pending_confirmation'];
const statsElement = ref<HTMLElement>(),
  statsInView = ref(false);
useIntersectionObserver(statsElement, ([entry]) => {
  statsInView.value = entry?.isIntersecting ?? false;
});
const heroElement = ref<HTMLElement>(),
  heroInView = ref(true);
useIntersectionObserver(heroElement, ([entry]) => {
  heroInView.value = entry?.isIntersecting ?? false;
});
const launching = ref(false),
  installGame = ref<PreviewGame>();
const preferences = ref<RecommendationPreferenceEntry[]>([]),
  preferenceError = ref(''),
  preferenceLoading = ref(false),
  preferenceBusy = ref(''),
  preferencesOpen = ref(false);
const randomGame = ref<PreviewGame>(),
  randomOpen = ref(false),
  randomBusy = ref(false),
  randomSource = ref('');
const drawn = new Set<string>();
const recommendationItems = ref<Recommendation[]>([]),
  recommendationError = ref(''),
  recommendationLoading = ref(false),
  batchFinished = ref(false);
const viewed = reactive(new Set<string>());
const excluded = computed(
  () =>
    new Set(
      preferences.value
        .filter(
          (p) =>
            p.preference === 'not_interested' ||
            !p.expires_at ||
            Date.parse(p.expires_at) > now.value.getTime(),
        )
        .map((p) => p.game_id),
    ),
);
const baseExcluded = computed(
  () =>
    new Set([
      ...(hero.value ? [hero.value.game_id] : []),
      ...wanted.value.map((g) => g.game_id),
      ...active.value,
    ]),
);
const suggestions = computed(() => {
  if (desktop)
    return recommendationItems.value
      .filter(
        (r) =>
          !baseExcluded.value.has(r.game_id) && !excluded.value.has(r.game_id),
      )
      .flatMap((r) => {
        const g = games.value.find((g) => g.game_id === r.game_id);
        return g ? [{ game: g, reason: r.reason }] : [];
      });
  return games.value
    .filter(
      (g) =>
        ['not_started', 'paused'].includes(g.status) &&
        !baseExcluded.value.has(g.game_id) &&
        !excluded.value.has(g.game_id) &&
        !viewed.has(g.game_id),
    )
    .slice(0, 4)
    .map((g) => ({
      game: g,
      reason:
        g.status === 'paused' ? '一段尚未结束的旅途' : '从你的未开始作品中推荐',
    }));
});
const calendarExpanded = ref(false),
  calendarLimit = ref(10);
const calendar = computed(() =>
  calendarEvents(
    desktop ? games.value : demoCalendarGames(games.value, now.value),
    now.value,
  ),
);
const calendarShown = computed(() =>
  calendar.value.slice(0, calendarExpanded.value ? calendarLimit.value : 3),
);
const sources = ref<Record<string, SourcedField | string>>({});
let alive = true,
  preferenceRevision = 0,
  recommendationRevision = 0,
  sourceRevision = 0,
  scrollFrame = 0;
async function library(
  view: GameStatus | 'all' | 'recent' | 'added' | 'wanted' | 'pending' = 'all',
  search = false,
) {
  selectGalleryGroup();
  if (view === 'wanted') {
    preview.status_filter = 'not_started';
    homeFavoriteIntent.value = true;
  } else if (view === 'recent')
    preview.gallery_state.system = 'recently-played';
  else if (view === 'pending') preview.gallery_state.system = 'pending';
  else if (view !== 'added') preview.status_filter = view;
  homeSearchIntent.value = search;
  await router.push({ name: 'games' });
}
async function start(game: PreviewGame) {
  if (launching.value) return;
  const installs = desktop
    ? local.records[game.game_id]?.installations.filter(
        (i) => i.path_valid && i.executable_path,
      )
    : [];
  if (installs && installs.length > 1) {
    installGame.value = game;
    return;
  }
  launching.value = true;
  try {
    await launchGame(game.game_id);
    await refresh();
  } finally {
    launching.value = false;
  }
}
async function startVersion(id: string) {
  if (launching.value) return;
  launching.value = true;
  try {
    await launchInstallation(id);
    installGame.value = undefined;
    await refresh();
  } finally {
    launching.value = false;
  }
}
async function loadPreferences() {
  const owner = ++preferenceRevision;
  preferenceLoading.value = true;
  try {
    const result = desktop
      ? await api('list_recommendation_preferences', {})
      : Object.entries(demoHomePreferences).map(([game_id, p]) => ({
          game_id,
          game_title:
            games.value.find((g) => g.game_id === game_id)?.title ?? '演示作品',
          ...p,
        }));
    if (alive && owner === preferenceRevision) {
      preferences.value = result;
      preferenceError.value = '';
    }
  } catch (e) {
    if (alive && owner === preferenceRevision)
      preferenceError.value = errorText(e);
  } finally {
    if (alive && owner === preferenceRevision) preferenceLoading.value = false;
  }
}
async function recommendations(reset = false) {
  const owner = ++recommendationRevision;
  recommendationLoading.value = true;
  if (reset) {
    viewed.clear();
    batchFinished.value = false;
  }
  try {
    if (desktop) {
      const result = await api('get_recommendations', {
        limit: 4,
        excluded_game_ids: [...new Set([...baseExcluded.value, ...viewed])],
      });
      if (alive && owner === recommendationRevision) {
        recommendationItems.value = result;
        batchFinished.value = !result.length && viewed.size > 0;
      }
    } else batchFinished.value = !suggestions.value.length && viewed.size > 0;
    if (alive && owner === recommendationRevision)
      recommendationError.value = '';
  } catch (e) {
    if (alive && owner === recommendationRevision)
      recommendationError.value = errorText(e);
  } finally {
    if (alive && owner === recommendationRevision)
      recommendationLoading.value = false;
  }
}
async function nextBatch() {
  if (recommendationLoading.value) return;
  suggestions.value.forEach((r) => viewed.add(r.game.game_id));
  await recommendations();
}
async function preference(
  id: string,
  value: 'not_interested' | 'snoozed' | 'none',
) {
  if (preferenceBusy.value) return;
  preferenceBusy.value = id;
  const expires =
    value === 'snoozed'
      ? new Date(Date.now() + 7 * 86400000).toISOString()
      : null;
  try {
    if (desktop) await setRecommendationPreference(id, value, expires);
    else if (value === 'none') delete demoHomePreferences[id];
    else demoHomePreferences[id] = { preference: value, expires_at: expires };
    await loadPreferences();
    await recommendations(true);
    notify(
      `${desktop ? '' : '演示：'}${value === 'none' ? '已恢复推荐' : value === 'snoozed' ? '七天内暂不推荐此作品' : '已保存不感兴趣偏好'}`,
    );
  } catch (e) {
    notify(errorText(e));
  } finally {
    preferenceBusy.value = '';
  }
}
async function pick() {
  if (randomBusy.value || preferenceLoading.value || preferenceError.value)
    return;
  randomBusy.value = true;
  try {
    let pool = games.value.filter(
      (g) =>
        g.favorite &&
        g.status === 'not_started' &&
        playable(g) &&
        !excluded.value.has(g.game_id) &&
        !active.value.includes(g.game_id),
    );
    randomSource.value = '从已收藏、尚未开始的可启动作品中抽取';
    if (!pool.length) {
      const ids = desktop
        ? (
            await api('get_recommendations', {
              limit: 50,
              excluded_game_ids: [...active.value, ...excluded.value],
            })
          ).map((r) => r.game_id)
        : games.value.map((g) => g.game_id);
      pool = games.value.filter(
        (g) =>
          ids.includes(g.game_id) &&
          ['not_started', 'paused'].includes(g.status) &&
          playable(g) &&
          !excluded.value.has(g.game_id) &&
          !active.value.includes(g.game_id),
      );
      randomSource.value = '从本地未开始与暂停的推荐作品中抽取';
    }
    if (!alive) return;
    const result = randomCandidate(pool, drawn);
    if (!result) {
      notify('目前没有符合条件的可启动候选，可添加作品或恢复推荐偏好。');
      return;
    }
    randomGame.value = result;
    randomOpen.value = true;
    if (pool.length === 1) randomSource.value += ' · 当前只有 1 部候选';
  } catch (e) {
    notify(errorText(e));
  } finally {
    randomBusy.value = false;
  }
}
function randomDetails() {
  const id = randomGame.value?.game_id;
  randomOpen.value = false;
  if (id) openPreviewGame(id);
}
async function randomLaunch() {
  const game = randomGame.value;
  if (!game) return;
  randomOpen.value = false;
  await start(game);
}
async function calendarSources() {
  const owner = ++sourceRevision;
  if (!desktop) return;
  const need = calendarShown.value.filter(
    (e) => !sources.value[`${e.game_id}:${e.release_date}`],
  );
  const results = await Promise.allSettled(
    need.map(async (e) => {
      const detail = await api('get_game', { game_id: e.game_id });
      return {
        key: `${e.game_id}:${e.release_date}`,
        field: detail.metadata.find(
          (f) => f.field === 'release_date' && f.value === e.release_date,
        ),
      };
    }),
  );
  if (!alive || owner !== sourceRevision) return;
  results.forEach((r, i) => {
    const key = `${need[i]!.game_id}:${need[i]!.release_date}`;
    sources.value[key] =
      r.status === 'fulfilled'
        ? (r.value.field ?? '本地已保存日期 · 来源待补充')
        : '来源信息暂不可用';
  });
}
function sourceText(id: string, date: string) {
  if (!desktop) return '演示资料 · 非真实发售消息';
  const f = sources.value[`${id}:${date}`];
  if (!f) return '读取日期来源…';
  if (typeof f === 'string') return f;
  return `${f.manually_edited ? '手工资料' : f.provider} · ${f.fetched_at ? new Date(f.fetched_at).toLocaleDateString('zh-CN') : '未记录抓取时间'}${f.cached ? ' · 缓存资料' : ''}`;
}
const weekMax = computed(() =>
  Math.max(
    1,
    ...(dashboard.value?.week_daily.map((d) => d.duration_seconds) ?? []),
  ),
);
const weekRows = computed(() => [
  ...(dashboard.value?.week_games ?? []).map((g) => ({
    ...g,
    title:
      games.value.find((item) => item.game_id === g.game_id)?.title ??
      '作品记录',
  })),
  ...(dashboard.value?.other_seconds
    ? [
        {
          game_id: '',
          title: '其他作品',
          duration_seconds: dashboard.value.other_seconds,
        },
      ]
    : []),
]);
const nickname = computed(
  () =>
    hikariAccount.account.profile?.nickname ||
    bangumi.account.profile?.nickname ||
    '',
);
const todayDate = computed(() => localDate(now.value));
watch(
  () =>
    games.value
      .map((g) => `${g.game_id}:${g.favorite}:${g.status}:${g.release_date}`)
      .join('|'),
  () => {
    void loadPreferences();
    void recommendations(true);
  },
);
watch(
  () => hero.value?.game_id,
  () => {
    void recommendations(true);
  },
);
watch(
  () => calendarShown.value.map((e) => e.key).join('|'),
  () => {
    void calendarSources();
  },
  { immediate: true },
);
onMounted(() => {
  void loadPreferences();
  void recommendations(true);
  void nextTick().then(() => {
    scrollFrame = requestAnimationFrame(() => {
      if (alive && homePosition.scroll)
        globalThis.scrollTo({ top: homePosition.scroll });
    });
  });
});
onBeforeUnmount(() => {
  alive = false;
  preferenceRevision++;
  recommendationRevision++;
  sourceRevision++;
  cancelAnimationFrame(scrollFrame);
});
</script>
<template>
  <div
    class="home-page home-v1 page-content"
    :class="{ 'first-visit': first_visit }"
    :data-running="active.length > 0"
    :data-hero-visible="heroInView"
    :data-stats-visible="statsInView"
  >
    <header class="home-welcome home-reveal">
      <div>
        <p class="eyebrow">YOUR STORY, STILL UNFOLDING</p>
        <h1 tabindex="-1" data-page-heading>
          {{ greeting(now.getHours()) }}{{ nickname ? `，${nickname}` : '' }}。
        </h1>
        <p class="home-lead">让故事，继续发生。</p>
      </div>
      <div class="home-welcome-actions">
        <button
          type="button"
          class="quiet-button"
          @click="library('all', true)"
        >
          <PreviewIcon name="search" :size="18" />搜索游戏</button
        ><button type="button" class="secondary-button" @click="openImport">
          <PreviewIcon name="plus" :size="18" />添加游戏
        </button>
      </div>
    </header>
    <section
      v-if="desktop && local.error && !games.length"
      class="home-empty home-panel"
      aria-label="游戏库读取失败"
    >
      <PreviewIcon name="warning" :size="36" />
      <h2>暂未能读取你的游戏库</h2>
      <p>{{ local.error }}</p>
      <button type="button" class="secondary-button" @click="refreshLibrary()">
        重新读取
      </button>
    </section>
    <section
      v-else-if="desktop && local.loading && !games.length"
      ref="heroElement"
      class="home-hero-skeleton home-panel"
      aria-label="正在读取游戏库"
      aria-busy="true"
    >
      <div class="home-skeleton"></div>
      <div class="home-skeleton"></div>
      <div class="home-skeleton"></div>
    </section>
    <section
      v-else-if="hero"
      ref="heroElement"
      class="hero-stage home-hero home-reveal"
      :data-preview-card="hero.game_id"
      :data-preview-card-key="`hero:${hero.game_id}`"
      :style="atmosphere"
      :aria-label="hero_label"
    >
      <img
        v-if="hero.cover_url && !preview.missing_cover"
        :key="hero.game_id + hero.cover_url"
        class="hero-scenery"
        :src="hero.cover_url"
        alt=""
        aria-hidden="true"
        @error="($event.target as HTMLImageElement).style.opacity = '0'"
      />
      <div class="hero-vignette" aria-hidden="true"></div>
      <div class="hero-halo" aria-hidden="true"></div>
      <div class="hero-copy">
        <span class="hero-kicker"
          ><span class="status-dot" :class="{ 'is-running': running }"></span
          >{{ hero_label
          }}<span class="mini-badge">{{
            desktop ? '本地作品' : '演示作品'
          }}</span></span
        >
        <p v-if="hero.subtitle" class="hero-subtitle">{{ hero.subtitle }}</p>
        <h2 class="hero-title" :data-shared-title="hero.game_id">
          {{ hero.title }}
        </h2>
        <p class="home-hero-meta">
          {{ STATUS_LABELS[hero.status] }}<span>·</span
          >{{
            hero.last_played_order
              ? `上次游玩：${relativePlayed(hero.last_played_order, now)}`
              : '新的故事，等你启程'
          }}
        </p>
        <p class="hero-footnote">
          <PreviewIcon name="clock" :size="16" />累计
          {{ compactTime(hero.playtime_seconds ?? hero.duration_minutes * 60) }}
        </p>
        <div class="hero-actions">
          <RouterLink
            v-if="running"
            class="primary-button"
            :to="{ name: 'activity' }"
            ><PreviewIcon name="clock" :size="18" />查看游玩记录</RouterLink
          ><button
            v-else
            type="button"
            class="primary-button"
            :disabled="launching"
            @click="start(hero)"
          >
            <PreviewIcon name="play" :size="18" />{{
              launching ? '正在启动…' : new_story ? '开始游玩' : '继续游玩'
            }}{{ desktop ? '' : ' · 演示' }}</button
          ><button
            type="button"
            class="hero-detail-button"
            :data-preview-open="`hero:${hero.game_id}`"
            @click="openPreviewGame(hero.game_id, $event)"
          >
            查看详情<PreviewIcon name="arrow" :size="18" />
          </button>
        </div>
      </div>
      <button
        type="button"
        class="hero-cover-frame home-hero-cover"
        :aria-label="`查看${hero.title}详情`"
        :data-preview-open="`hero:${hero.game_id}`"
        @click="openPreviewGame(hero.game_id, $event)"
      >
        <PreviewCover
          class="hero-cover"
          loading="eager"
          :data-shared-cover="hero.game_id"
          :cover_url="hero.cover_url"
          :title="hero.title"
        /><span class="hero-cover-caption">A STORY WORTH KEEPING</span></button
      ><span class="home-hero-number" aria-hidden="true">01 / YOUR STORY</span>
    </section>
    <section v-else class="home-empty home-panel home-reveal">
      <PreviewIcon name="spark" :size="42" />
      <p class="eyebrow">
        {{ games.length ? 'A WORLD WORTH REVISITING' : 'YOUR FIRST CHAPTER' }}
      </p>
      <h2>
        {{
          games.length
            ? '重新发现，留在收藏里的故事。'
            : '你的第一段故事，值得一个好位置。'
        }}
      </h2>
      <p>
        {{
          games.length
            ? '可以查看收藏，或为未开始的作品配置启动入口。'
            : desktop
              ? '从你选择的目录添加作品，开启属于你的旅程。'
              : '这是独立原型演示，不会扫描或修改本地文件。'
        }}
      </p>
      <button
        type="button"
        class="primary-button"
        @click="games.length ? library() : openImport()"
      >
        <PreviewIcon :name="games.length ? 'games' : 'plus'" />{{
          games.length ? '查看游戏库' : '添加游戏'
        }}
      </button>
    </section>
    <template v-if="games.length">
      <section
        class="home-section home-reveal"
        aria-labelledby="home-recent-heading"
      >
        <div class="home-section-heading">
          <h2 id="home-recent-heading">最近游玩</h2>
          <button type="button" class="text-link" @click="library('recent')">
            查看全部<PreviewIcon name="arrow" :size="16" />
          </button>
        </div>
        <HomeStrip
          v-if="recent.length"
          :games="recent"
          context="home-recent"
          recent
        />
        <p v-else class="home-small">
          还没有启动过游戏，开启一段故事后会显示在这里。
        </p>
      </section>
      <section
        class="home-section home-reveal"
        aria-labelledby="home-library-heading"
      >
        <div class="home-section-heading">
          <h2 id="home-library-heading">我的游戏</h2>
          <span class="home-small">每一个世界，都有自己的位置</span>
        </div>
        <div class="home-counts">
          <button
            v-for="[status, label] in mainCounts"
            :key="status"
            type="button"
            @click="library(status)"
          >
            <strong>{{
              status === 'all' ? counts.total : counts[status]
            }}</strong
            ><span>{{ label }}</span
            ><PreviewIcon name="arrow" :size="15" />
          </button>
        </div>
        <div
          v-if="otherCounts.some((s) => counts[s] > 0)"
          class="home-other-counts"
        >
          <span>还有</span
          ><button
            v-for="status in otherCounts.filter((s) => counts[s] > 0)"
            :key="status"
            type="button"
            @click="library(status)"
          >
            {{ STATUS_LABELS[status] }} {{ counts[status] }}</button
          ><span>部作品</span>
        </div>
      </section>
      <section
        ref="statsElement"
        class="home-section home-playtime home-reveal"
        aria-label="游玩近况"
        :aria-busy="loading"
      >
        <div class="home-panel home-today">
          <div class="home-section-heading">
            <h2>今日游玩</h2>
            <PreviewIcon name="clock" :size="20" />
          </div>
          <p
            v-if="!dashboard && loading"
            class="home-skeleton home-stat-skeleton"
          ></p>
          <strong v-else-if="dashboard" class="home-today-number">{{
            compactTime(dashboard.today_seconds)
          }}</strong>
          <p v-if="dashboard" class="home-week-total">
            本周 {{ compactTime(dashboard.week_seconds) }}
          </p>
          <div
            v-if="dashboard"
            class="home-week-chart"
            aria-label="本周七日游玩时长"
          >
            <div
              v-for="(day, i) in dashboard.week_daily"
              :key="day.date"
              class="home-week-day"
              :class="{
                'is-today': day.date === todayDate,
                'is-future': day.date > todayDate,
              }"
              tabindex="0"
              role="img"
              :aria-label="`${day.date}，${compactTime(day.duration_seconds)}`"
              :title="`${day.date} · ${compactTime(day.duration_seconds)}`"
            >
              <span class="home-week-bar"
                ><i
                  :style="{
                    height: `${Math.max(3, (day.duration_seconds / weekMax) * 100)}%`,
                  }"
                ></i></span
              ><span>{{ ['一', '二', '三', '四', '五', '六', '日'][i] }}</span>
            </div>
          </div>
          <p class="home-stat-note">
            本地自然日 / 周 · 按会话开始日归属<br />{{
              desktop
                ? '仅统计未隐藏作品，包含已保存的运行时长'
                : '演示统计 · 非真实游玩记录'
            }}
          </p>
        </div>
        <div class="home-panel home-distribution">
          <div class="home-section-heading">
            <h2>本周游玩分布</h2>
            <RouterLink class="text-link" :to="{ name: 'activity' }"
              >查看记录<PreviewIcon name="arrow" :size="15"
            /></RouterLink>
          </div>
          <div v-if="weekRows.length" class="home-distribution-rows">
            <div
              v-for="row in weekRows"
              :key="row.game_id"
              class="home-distribution-row"
            >
              <div>
                <button
                  v-if="row.game_id"
                  type="button"
                  @click="openPreviewGame(row.game_id)"
                >
                  {{ row.title }}</button
                ><span v-else>{{ row.title }}</span
                ><small>{{ compactTime(row.duration_seconds) }}</small
                ><span class="home-percentage"
                  >{{
                    Math.round(
                      (row.duration_seconds / (dashboard?.week_seconds || 1)) *
                        100,
                    )
                  }}%</span
                >
              </div>
              <meter
                min="0"
                max="100"
                :value="
                  (row.duration_seconds / (dashboard?.week_seconds || 1)) * 100
                "
                :aria-label="`${row.title}占本周游玩时长的比例`"
                :style="{
                  '--portion': `${(row.duration_seconds / (dashboard?.week_seconds || 1)) * 100}%`,
                }"
              ></meter>
            </div>
          </div>
          <div
            v-else-if="loading && !dashboard"
            class="home-skeleton home-stat-skeleton"
          ></div>
          <p v-else-if="dashboard" class="home-soft-empty">
            本周还没有游玩记录。<br />下一次启程，就从这里留下足迹。
          </p>
          <p v-else class="home-soft-empty">暂未能读取游玩记录。</p>
          <p v-if="error" class="home-error" role="status">
            {{ error
            }}<button type="button" class="text-link" @click="refresh">
              重新读取
            </button>
          </p>
          <p v-if="error && dashboard" class="home-stat-note">
            保留上次更新：{{
              new Date(dashboard.queried_at).toLocaleString('zh-CN')
            }}
          </p>
          <span class="home-distribution-art" aria-hidden="true"
            >TIME<br />WELL SPENT</span
          >
        </div>
      </section>
      <section
        class="home-section home-reveal"
        aria-labelledby="home-wanted-heading"
      >
        <div class="home-section-heading">
          <div>
            <h2 id="home-wanted-heading">想玩 / 待游玩</h2>
            <p class="home-small">已收藏，尚未开始</p>
          </div>
          <button
            type="button"
            class="quiet-button home-random-button"
            :disabled="randomBusy || preferenceLoading || !!preferenceError"
            @click="pick"
          >
            <PreviewIcon name="shuffle" :size="19" />{{
              randomBusy ? '正在抽取…' : '随机一个'
            }}
          </button>
        </div>
        <HomeStrip v-if="wanted.length" :games="wanted" context="home-wanted" />
        <p v-else class="home-inline-empty">
          收藏一部尚未开始的作品，放进下一段旅程。<button
            type="button"
            class="text-link"
            @click="library()"
          >
            前往游戏库<PreviewIcon name="arrow" :size="16" />
          </button>
        </p>
        <div v-if="wanted.length" class="home-section-tail">
          <button type="button" class="text-link" @click="library('wanted')">
            查看全部待游玩作品<PreviewIcon name="arrow" :size="16" />
          </button>
        </div>
        <p v-if="preferenceError" class="home-error">
          暂未能读取推荐偏好：{{ preferenceError
          }}<button type="button" class="text-link" @click="loadPreferences">
            重试
          </button>
        </p>
      </section>
      <section
        class="home-section home-reveal"
        aria-labelledby="home-recommend-heading"
        :aria-busy="recommendationLoading"
      >
        <div class="home-section-heading">
          <div>
            <p class="eyebrow">THE NEXT CHAPTER</p>
            <h2 id="home-recommend-heading">为你推荐</h2>
          </div>
          <button
            type="button"
            class="quiet-button"
            :disabled="!suggestions.length || recommendationLoading"
            @click="nextBatch"
          >
            换一批<PreviewIcon name="arrow" :size="17" />
          </button>
        </div>
        <TransitionGroup
          name="home-gallery"
          tag="div"
          class="home-recommendations"
          ><article
            v-for="{ game, reason } in suggestions"
            :key="game.game_id"
            class="home-recommendation"
          >
            <CoverCard :game="game" context="recommendation" :reason="reason" />
            <details
              class="home-rec-menu"
              @keydown.esc="
                ($event.currentTarget as HTMLDetailsElement).open = false
              "
            >
              <summary :aria-label="`${game.title}推荐选项`">
                <PreviewIcon name="more" :size="20" />
              </summary>
              <div>
                <button
                  type="button"
                  :disabled="!!preferenceBusy"
                  @click="preference(game.game_id, 'snoozed')"
                >
                  暂不推荐 7 天</button
                ><button
                  type="button"
                  :disabled="!!preferenceBusy"
                  @click="preference(game.game_id, 'not_interested')"
                >
                  不感兴趣
                </button>
              </div>
            </details>
          </article></TransitionGroup
        >
        <p
          v-if="recommendationLoading && !suggestions.length"
          class="home-small"
          role="status"
        >
          正在读取本地推荐…
        </p>
        <p v-if="recommendationError" class="home-error" role="status">
          {{ recommendationError
          }}<button type="button" class="text-link" @click="recommendations()">
            重试
          </button>
        </p>
        <p
          v-else-if="!suggestions.length && !recommendationLoading"
          class="home-inline-empty"
        >
          {{
            batchFinished
              ? '本轮候选已浏览完。'
              : '目前没有更多符合条件的候选，可查看待游玩作品或恢复推荐偏好。'
          }}<button
            v-if="batchFinished"
            type="button"
            class="text-link"
            @click="recommendations(true)"
          >
            重新浏览
          </button>
        </p>
        <div class="home-section-tail">
          <span class="home-small">{{
            desktop
              ? '基于本地作品与已记录的偏好'
              : '本地推荐方式演示 · 没有访问网络'
          }}</span
          ><button
            type="button"
            class="text-link"
            @click="
              preferencesOpen = true;
              loadPreferences();
            "
          >
            已排除作品{{ preferences.length ? ` · ${preferences.length}` : '' }}
          </button>
        </div>
      </section>
      <section
        class="home-section home-almanac home-reveal"
        aria-label="作品日期与回忆"
      >
        <div class="home-panel home-calendar">
          <div class="home-section-heading">
            <h2>游戏日历</h2>
            <span class="home-small">未来 30 天</span>
          </div>
          <div v-if="calendarShown.length" class="home-calendar-list">
            <div
              v-for="event in calendarShown"
              :key="event.key"
              class="home-calendar-row"
            >
              <time :datetime="event.date"
                ><span>{{ event.date.slice(5, 7) }}月</span
                ><strong>{{ event.date.slice(8) }}</strong></time
              >
              <div>
                <button type="button" @click="openPreviewGame(event.game_id)">
                  {{ games.find((g) => g.game_id === event.game_id)?.title }}
                </button>
                <p>
                  {{ event.label
                  }}<span v-if="event.date === todayDate" class="mini-badge"
                    >今天</span
                  >
                </p>
                <small>{{
                  sourceText(event.game_id, event.release_date)
                }}</small>
              </div>
            </div>
          </div>
          <p v-else class="home-soft-empty">
            近期没有已记录的作品日期。<br />完整的发售资料，会让这里慢慢丰富起来。
          </p>
          <p class="home-stat-note">根据已保存资料展示，发售日期可能调整</p>
          <button
            v-if="calendar.length > 3"
            type="button"
            class="text-link"
            :aria-expanded="calendarExpanded"
            @click="calendarExpanded = !calendarExpanded"
          >
            {{ calendarExpanded ? '收起日历' : '展开日历'
            }}<PreviewIcon name="arrow" :size="15" /></button
          ><button
            v-if="calendarExpanded && calendar.length > calendarLimit"
            type="button"
            class="text-link"
            @click="calendarLimit += 10"
          >
            更多日期
          </button>
        </div>
        <div
          v-if="dashboard?.memory_games.length"
          class="home-panel home-memory"
        >
          <div class="home-section-heading">
            <h2>一年前的今天</h2>
            <PreviewIcon name="spark" :size="19" />
          </div>
          <p class="home-memory-date">
            {{
              new Date(
                dashboard.window.memory_day!.start_at,
              ).toLocaleDateString('zh-CN')
            }}
          </p>
          <div
            v-for="memory in dashboard.memory_games"
            :key="memory.game_id"
            class="home-memory-entry"
          >
            <p>你游玩了</p>
            <button
              type="button"
              class="home-memory-title"
              @click="openPreviewGame(memory.game_id)"
            >
              《{{
                games.find((g) => g.game_id === memory.game_id)?.title ??
                '作品记录'
              }}》
            </button>
            <p class="home-memory-duration">
              当日 {{ compactTime(memory.duration_seconds)
              }}{{ desktop ? '' : ' · 演示回忆' }}
            </p>
            <details>
              <summary>查看那天的记录</summary>
              <ul>
                <li v-for="session in memory.sessions" :key="session.id">
                  {{
                    new Date(session.started_at).toLocaleTimeString('zh-CN', {
                      hour: '2-digit',
                      minute: '2-digit',
                    })
                  }}
                  · {{ compactTime(session.duration_seconds) }}
                </li>
              </ul>
              <small v-if="memory.sessions.length === 10"
                >最多展示前 10 次会话，合计包含全部有效记录</small
              >
            </details>
          </div>
          <span class="home-memory-watermark" aria-hidden="true"
            >MEMORIES<br />STAY WITH US.</span
          >
        </div>
      </section>
      <section
        v-if="
          desktop &&
          (local.home?.pending_match_count || local.home?.save_issue_count)
        "
        class="home-notices"
        aria-label="待处理事项"
      >
        <button
          v-if="local.home?.pending_match_count"
          type="button"
          @click="library('pending')"
        >
          <PreviewIcon name="spark" :size="16" />{{
            local.home.pending_match_count
          }}
          个作品资料待确认<PreviewIcon name="arrow" :size="16" /></button
        ><RouterLink v-if="local.home?.save_issue_count" to="/saves"
          ><PreviewIcon name="warning" :size="16" />{{
            local.home.save_issue_count
          }}
          个存档配置需要检查<PreviewIcon name="arrow" :size="16"
        /></RouterLink>
      </section>
    </template>
    <HomeDialog
      :open="randomOpen"
      title="下一段旅程，会是哪一部？"
      @close="randomOpen = false"
      ><div
        v-if="randomGame"
        :key="randomGame.game_id"
        class="home-random-result"
      >
        <PreviewCover
          :cover_url="randomGame.cover_url"
          :title="randomGame.title"
        />
        <div>
          <span class="home-small">{{
            desktop ? '本地候选' : '演示候选'
          }}</span>
          <h3>{{ randomGame.title }}</h3>
          <p>{{ randomSource }}</p>
          <p class="home-stat-note">抽取不会启动游戏，选择开始后才会运行。</p>
        </div>
      </div>
      <div class="home-dialog-actions">
        <button
          type="button"
          class="quiet-button"
          :disabled="randomBusy"
          @click="pick"
        >
          换一个</button
        ><button type="button" class="secondary-button" @click="randomDetails">
          查看详情</button
        ><button
          type="button"
          class="primary-button"
          :disabled="launching"
          @click="randomLaunch"
        >
          <PreviewIcon name="play" :size="16" />{{
            randomGame?.status === 'paused' ? '继续游玩' : '开始游玩'
          }}{{ desktop ? '' : ' · 演示' }}
        </button>
      </div></HomeDialog
    >
    <HomeDialog
      :open="preferencesOpen"
      title="已排除作品"
      @close="preferencesOpen = false"
      ><p class="home-small">恢复后作品会重新参与本地推荐与随机候选。</p>
      <p v-if="preferenceLoading">正在读取偏好…</p>
      <p v-else-if="preferenceError" class="home-error">
        {{ preferenceError
        }}<button type="button" class="text-link" @click="loadPreferences">
          重新读取
        </button>
      </p>
      <p v-else-if="!preferences.length" class="home-soft-empty">
        还没有已排除的作品。
      </p>
      <div
        v-for="p in preferences"
        :key="p.game_id"
        class="home-preference-row"
      >
        <div>
          <strong>{{ p.game_title }}</strong>
          <p>
            {{
              p.preference === 'not_interested'
                ? '不感兴趣'
                : p.expires_at
                  ? `暂不推荐至 ${new Date(p.expires_at).toLocaleDateString('zh-CN')}`
                  : '暂不推荐'
            }}
          </p>
        </div>
        <button
          type="button"
          class="secondary-button"
          :disabled="!!preferenceBusy"
          @click="preference(p.game_id, 'none')"
        >
          {{ preferenceBusy === p.game_id ? '正在恢复…' : '恢复推荐' }}
        </button>
      </div></HomeDialog
    >
    <HomeDialog
      :open="!!installGame"
      title="选择要启动的版本"
      @close="installGame = undefined"
      ><div
        v-for="(install, index) in local.records[
          installGame?.game_id ?? ''
        ]?.installations.filter((i) => i.path_valid && i.executable_path)"
        :key="install.id"
        class="home-preference-row"
      >
        <div>
          <strong>版本 {{ index + 1 }} · {{ install.source }}</strong>
          <p :title="install.absolute_path">
            {{ install.absolute_path.split(/[\\/]/).at(-1) }}
          </p>
          <small>{{ install.executable_path?.split(/[\\/]/).at(-1) }}</small>
        </div>
        <button
          type="button"
          class="primary-button"
          :disabled="launching"
          @click="startVersion(install.id)"
        >
          {{ launching ? '正在启动…' : '启动此版本' }}
        </button>
      </div></HomeDialog
    >
  </div>
</template>
<style src="../home.css"></style>
