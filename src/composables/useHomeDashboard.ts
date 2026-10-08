import { installationLaunchable } from '../services/steamImport';
import { computed, onMounted, onBeforeUnmount, ref, watch } from 'vue';
import { api, desktop, errorText, local, preview } from '../stores/library';
import {
  homeWindow,
  statusCounts,
  selectHomeHero,
} from '../services/homeDashboard';
import { demoHomeDashboard } from '../preview/home';
import { subscribeEvent } from '../services/events';
import type { HomeDashboard } from '../types/home';
import type { PreviewGame } from '../preview/data';
const saved = ref<HomeDashboard | null>(null);
export const homePosition = { scroll: 0, hero_id: '' };
export function useHomeDashboard() {
  const now = ref(new Date()),
    loading = ref(false),
    error = ref(''),
    visible = ref(!document.hidden);
  const games = computed(() => (preview.show_empty ? [] : preview.games));
  const window = computed(() => homeWindow(now.value));
  const dashboard = computed(() =>
    desktop ? saved.value : demoHomeDashboard(games.value, window.value),
  );
  const counts = computed(
    () => dashboard.value?.status_counts ?? statusCounts(games.value),
  );
  const active = computed(() => dashboard.value?.active_game_ids ?? []);
  const playable = (g: PreviewGame) =>
    desktop
      ? Boolean(
          local.records[g.game_id]?.installations.some((i) =>
            installationLaunchable(i),
          ),
        )
      : g.launchable !== false;
  const wanted = computed(() =>
    [...games.value]
      .filter((g) => g.favorite && g.status === 'not_started')
      .sort(
        (a, b) =>
          Number(playable(b)) - Number(playable(a)) ||
          b.added_order - a.added_order,
      )
      .slice(0, 4),
  );
  const hero = computed(() => {
    const selected = selectHomeHero(games.value, active.value, playable);
    const previous = games.value.find(
      (g) => g.game_id === homePosition.hero_id,
    );
    const running = active.value
      .map((id) => games.value.find((g) => g.game_id === id))
      .find(Boolean);
    if (running) {
      homePosition.hero_id = running.game_id;
      return running;
    }
    if (
      previous &&
      playable(previous) &&
      (previous.status === 'not_started' ||
        (['playing', 'paused'].includes(previous.status) &&
          previous.last_played_order > 0))
    )
      return previous;
    homePosition.hero_id = selected?.game_id ?? '';
    return selected;
  });
  const recentlyPlayed = computed(() => {
    if (desktop && dashboard.value)
      return dashboard.value.recently_played_game_ids
        .map((id) => games.value.find((g) => g.game_id === id))
        .filter((g): g is PreviewGame => !!g);
    return [...games.value]
      .filter((g) => g.last_played_order > 0)
      .sort((a, b) => b.last_played_order - a.last_played_order);
  });
  const recent = computed(() => recentlyPlayed.value.slice(0, 4));
  let alive = true,
    revision = 0,
    pending = false,
    timer: ReturnType<typeof setInterval> | undefined;
  const controller = new AbortController();
  async function refresh() {
    if (!desktop || !alive) return;
    if (loading.value) {
      pending = true;
      return;
    }
    const owner = ++revision;
    loading.value = true;
    now.value = new Date();
    try {
      const response = await api('get_home_summary', {
        dashboard_window: homeWindow(now.value),
      });
      if (alive && owner === revision) {
        if (!response.dashboard) throw new Error('首页摘要尚未返回统计数据。');
        saved.value = response.dashboard;
        local.home = response;
        error.value = '';
      }
    } catch (e) {
      if (alive && owner === revision) error.value = errorText(e);
    } finally {
      if (alive && owner === revision) {
        loading.value = false;
        if (pending) {
          pending = false;
          void refresh();
        }
      }
    }
  }
  function visibility() {
    visible.value = !document.hidden;
    if (visible.value) {
      now.value = new Date();
      void refresh();
    }
  }
  watch(
    () => local.records,
    () => {
      void refresh();
    },
  );
  onMounted(() => {
    void refresh();
    document.addEventListener('visibilitychange', visibility);
    timer = setInterval(() => {
      if (!visible.value) return;
      const next = new Date();
      const changed =
        homeWindow(next).today.start_at !== window.value.today.start_at ||
        homeWindow(next).time_zone !== window.value.time_zone;
      now.value = next;
      if (active.value.length || changed) void refresh();
    }, 30000);
    if (desktop)
      for (const name of ['session_started', 'session_ended'] as const)
        void subscribeEvent(
          name,
          () => {
            void refresh();
          },
          controller.signal,
        ).catch((e) => {
          if (alive) error.value = errorText(e);
        });
  });
  onBeforeUnmount(() => {
    alive = false;
    revision++;
    controller.abort();
    clearInterval(timer);
    document.removeEventListener('visibilitychange', visibility);
    homePosition.scroll = globalThis.scrollY;
  });
  return {
    now,
    window,
    dashboard,
    counts,
    active,
    games,
    hero,
    wanted,
    recent,
    recentlyPlayed,
    playable,
    loading,
    error,
    refresh,
    visible,
  };
}
