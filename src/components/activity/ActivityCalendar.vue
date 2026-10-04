<script setup lang="ts">
import { computed, nextTick, ref, useId, watch } from 'vue';
import type { ActivitySnapshot } from '../../types/activity';
import {
  calendarCells,
  fullDateLabel,
  rangeLabel,
} from '../../services/activity';
import { durationText } from '../../services/playtime';
import ActivityTooltip from './ActivityTooltip.vue';
const props = defineProps<{
  snapshot: ActivitySnapshot;
  selected: string | null;
  loading: boolean;
}>();
const emit = defineEmits<{ select: [date: string]; year: [year: number] }>();
const cells = computed(() => calendarCells(props.snapshot));
const cols = computed(() => cells.value.length / 7);
const validIndices = computed(() =>
  cells.value.flatMap((c, i) => (c?.valid ? [i] : [])),
);
const focus = ref(0),
  target = ref<Element | null>(null),
  hovered = ref<number | null>(null);
const scroller = ref<HTMLElement>();
const tipId = useId();
const months = computed(() => {
  const labels: { index: number; text: string }[] = [];
  let last = '';
  cells.value.forEach((c, i) => {
    if (c && c.date.slice(0, 7) !== last) {
      labels.push({
        index: Math.floor(i / 7),
        text: `${Number(c.date.slice(5, 7))}月`,
      });
      last = c.date.slice(0, 7);
    }
  });
  return labels;
});
const active = computed(() =>
  hovered.value == null ? null : cells.value[hovered.value],
);
watch(
  () => [props.snapshot.calendar_year, props.snapshot.range],
  async () => {
    focus.value = validIndices.value.at(-1) ?? 0;
    target.value = null;
    hovered.value = null;
    await nextTick();
    const element = scroller.value?.querySelector<HTMLElement>(
      `[data-day-index="${focus.value}"]`,
    );
    if (element && scroller.value)
      scroller.value.scrollLeft = Math.max(
        0,
        element.offsetLeft - scroller.value.clientWidth + 60,
      );
  },
  { immediate: true },
);
function show(e: Event, i: number) {
  target.value = e.currentTarget as Element;
  hovered.value = i;
}
function hide() {
  target.value = null;
  hovered.value = null;
}
function key(e: KeyboardEvent, i: number) {
  if (
    ![
      'ArrowLeft',
      'ArrowRight',
      'ArrowUp',
      'ArrowDown',
      'Home',
      'End',
    ].includes(e.key)
  )
    return;
  e.preventDefault();
  let next =
    e.key === 'Home'
      ? validIndices.value[0]
      : e.key === 'End'
        ? validIndices.value.at(-1)!
        : i +
          (
            {
              ArrowLeft: -7,
              ArrowRight: 7,
              ArrowUp: -1,
              ArrowDown: 1,
            } as Record<string, number>
          )[e.key];
  if (!cells.value[next]?.valid) return;
  focus.value = next;
  scroller.value
    ?.querySelector<HTMLButtonElement>(`[data-day-index="${next}"]`)
    ?.focus();
}
</script>
<template>
  <section
    class="activity-section activity-calendar"
    aria-labelledby="calendar-title"
  >
    <header class="activity-section-heading">
      <div>
        <p class="activity-kicker">ONE DAY, ONE LITTLE MEMORY</p>
        <h2 id="calendar-title">游玩日历</h2>
      </div>
      <label v-if="snapshot.range === 'all'" class="activity-year-picker"
        ><span class="sr-only">日历年份，仅改变日历窗口</span
        ><select
          :value="snapshot.calendar_year"
          :disabled="loading"
          @change="
            emit('year', Number(($event.target as HTMLSelectElement).value))
          "
        >
          <option
            v-for="year in snapshot.calendar_years"
            :key="year"
            :value="year"
          >
            {{ year }} 年
          </option>
        </select></label
      ><span v-else class="activity-note">{{ snapshot.calendar_year }} 年</span>
    </header>
    <p class="activity-calendar-context">
      日历：{{ snapshot.calendar_year }} 年全年 <span>／</span> 统计：{{
        snapshot.range === 'all' ? '全部历史' : rangeLabel(snapshot.range)
      }}
    </p>
    <div class="calendar-layout">
      <div class="calendar-weekdays" aria-hidden="true">
        <span>一</span><span></span><span>三</span><span></span><span>五</span
        ><span></span><span>日</span>
      </div>
      <div ref="scroller" class="calendar-scroll" tabindex="-1">
        <div
          class="calendar-months"
          :style="{ '--calendar-columns': cols }"
          aria-hidden="true"
        >
          <span
            v-for="month in months"
            :key="`${month.index}-${month.text}`"
            :style="{ gridColumn: month.index + 1 }"
            >{{ month.text }}</span
          >
        </div>
        <div
          class="calendar-grid"
          :style="{ '--calendar-columns': cols }"
          role="group"
          aria-label="游玩日历，周一至周日，方向键浏览，Enter 选择日期"
        >
          <template
            v-for="(cell, index) in cells"
            :key="cell?.date ?? `pad-${index}`"
            ><button
              v-if="cell"
              class="calendar-cell"
              :class="[
                `heat-${cell.level}`,
                {
                  'is-selected': selected === cell.date,
                  'is-future': cell.future,
                },
              ]"
              :disabled="!cell.valid"
              :tabindex="cell.valid && focus === index ? 0 : -1"
              :data-date="cell.date"
              :data-day-index="index"
              :aria-pressed="selected === cell.date"
              :aria-label="`${cell.date}，${durationText(cell.duration_seconds)}，${cell.session_count}次会话${cell.future ? '，未来日期' : cell.valid ? '' : '，不在统计范围内'}`"
              :aria-describedby="target ? tipId : undefined"
              @pointerenter="show($event, index)"
              @pointerleave="hide"
              @focus="
                focus = index;
                show($event, index);
              "
              @blur="hide"
              @click="emit('select', cell.date)"
              @keydown="key($event, index)"
            ></button
            ><span v-else class="calendar-padding" aria-hidden="true"></span
          ></template>
        </div>
      </div>
    </div>
    <footer class="calendar-footer">
      <p>每一格，都是与故事相伴的一天。</p>
      <div
        class="calendar-legend"
        aria-label="时长分级：0，30分钟以内，30分钟至2小时，2至4小时，4小时以上"
      >
        <span>少</span
        ><i
          v-for="level in 5"
          :key="level"
          :class="`heat-${level - 1}`"
          :title="
            [
              '0 秒',
              '0～30 分钟',
              '30 分钟～2 小时',
              '2～4 小时',
              '4 小时以上',
            ][level - 1]
          "
        ></i
        ><span>多</span>
      </div>
    </footer>
    <details class="calendar-thresholds">
      <summary>查看时长分级</summary>
      <p>
        0 秒 · 30 分钟以内 · 30 分钟至 2 小时 · 2 至 4 小时 · 4
        小时以上。以保存时长按 UTC 会话开始日汇总。
      </p>
    </details>
    <ActivityTooltip
      :id="tipId"
      :target="target"
      :title="active ? fullDateLabel(active.date) : ''"
      :text="
        active
          ? active.future
            ? `${active.date} · 未来日期`
            : `${active.date}${active.valid ? '' : ' · 不在当前统计范围内'} · ${durationText(active.duration_seconds)} · ${active.session_count ? `${active.session_count} 次会话` : '没有游玩会话'}`
          : ''
      "
      @dismiss="hide"
    />
  </section>
</template>
