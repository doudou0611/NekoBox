import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from 'vue';
import { onBeforeRouteLeave } from 'vue-router';
import { api, desktop, errorText, local } from '../stores/library';
import { rangeBounds } from '../services/activity';
import { subscribeEvent } from '../services/events';
import type {
  ActivityQuery,
  ActivityRange,
  ActivitySnapshot,
} from '../types/activity';
let remembered: {
  range: ActivityRange;
  year: number | null;
  scroll: number;
} | null = null;
export function useActivity() {
  const range = ref<ActivityRange>(remembered?.range ?? 'days30');
  const year = ref<number | null>(remembered?.year ?? null);
  const date = ref<string | null>(null);
  const page = ref(1);
  const snapshot = ref<ActivitySnapshot | null>(null);
  const loading = ref(false),
    error = ref('');
  const controller = new AbortController();
  let revision = 0,
    timer: ReturnType<typeof setInterval> | undefined;
  const outdated = computed(
    () => !!snapshot.value && (loading.value || !!error.value),
  );
  async function load(): Promise<void> {
    if (date.value) {
      const bounds = rangeBounds(
        range.value,
        new Date(),
        snapshot.value?.range_start,
      );
      if (date.value < bounds.start || date.value >= bounds.end) {
        date.value = null;
        page.value = 1;
      }
    }
    const ticket = ++revision;
    const q: ActivityQuery = {
      range: range.value,
      calendar_year: range.value === 'all' ? year.value : null,
      session_date: date.value,
      page: page.value,
      page_size: 20,
    };
    loading.value = true;
    error.value = '';
    try {
      const result = desktop
        ? await api('get_activity_snapshot', q)
        : (await import('../preview/activity')).previewActivity(q);
      if (ticket !== revision) return;
      const lastPage = Math.max(
        1,
        Math.ceil(result.sessions.total / result.sessions.page_size),
      );
      if (result.sessions.page > lastPage) {
        page.value = lastPage;
        await load();
        return;
      }
      snapshot.value = result;
    } catch (cause) {
      if (ticket === revision) error.value = errorText(cause);
    } finally {
      if (ticket === revision) loading.value = false;
    }
  }
  function selectRange(value: ActivityRange) {
    if (range.value === value) return;
    range.value = value;
    year.value = null;
    date.value = null;
    page.value = 1;
    void load();
  }
  function selectYear(value: number) {
    year.value = value;
    date.value = null;
    page.value = 1;
    void load();
  }
  function selectDate(value: string | null) {
    date.value = date.value === value ? null : value;
    page.value = 1;
    void load();
  }
  function selectPage(value: number) {
    page.value = value;
    void load();
  }
  const reloadIfVisible = () => {
    if (!document.hidden && !loading.value) void load();
  };
  watch(
    () =>
      Object.values(local.records)
        .map(
          (g) => `${g.id}:${g.status}:${g.title}:${g.total_playtime_seconds}`,
        )
        .join('|'),
    () => {
      if (desktop && !document.hidden) void load();
    },
  );
  onBeforeRouteLeave(() => {
    remembered = {
      range: range.value,
      year: year.value,
      scroll: window.scrollY,
    };
  });
  onMounted(async () => {
    if (desktop) {
      for (const key of ['session_started', 'session_ended'] as const)
        void subscribeEvent(key, () => void load(), controller.signal).catch(
          () => {
            error.value = '实时事件订阅失败，可使用刷新获取最新记录。';
          },
        );
      timer = setInterval(() => {
        if (snapshot.value?.summary.active_session_count) reloadIfVisible();
      }, 15000);
    }
    document.addEventListener('visibilitychange', reloadIfVisible);
    await load();
    if (remembered && !controller.signal.aborted) {
      await nextTick();
      const scroll = remembered.scroll;
      requestAnimationFrame(() => {
        if (!controller.signal.aborted) window.scrollTo({ top: scroll });
      });
    }
  });
  onBeforeUnmount(() => {
    revision++;
    controller.abort();
    clearInterval(timer);
    document.removeEventListener('visibilitychange', reloadIfVisible);
  });
  return {
    range,
    year,
    date,
    page,
    snapshot,
    loading,
    error,
    outdated,
    load,
    selectRange,
    selectYear,
    selectDate,
    selectPage,
  };
}
