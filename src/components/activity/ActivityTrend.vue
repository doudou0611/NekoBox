<script setup lang="ts">
import { computed, ref, useId, watch } from 'vue';
import type { ActivitySnapshot } from '../../types/activity';
import { compactDuration, trendGeometry } from '../../services/activity';
import { durationText } from '../../services/playtime';
import ActivityTooltip from './ActivityTooltip.vue';
const props = defineProps<{ snapshot: ActivitySnapshot }>();
const chartId = useId(),
  tipId = useId();
const focus = ref(0),
  hovering = ref<number | null>(null),
  target = ref<Element | null>(null);
const plot = computed(() =>
  trendGeometry(props.snapshot.trend.map((d) => d.duration_seconds)),
);
const grain = computed(
  () =>
    ({ day: '每日', week: '每周', month: '每月', year: '每年' })[
      props.snapshot.trend_granularity
    ],
);
const tickIndices = computed(() =>
  props.snapshot.trend.length
    ? [
        ...new Set([
          0,
          Math.floor((props.snapshot.trend.length - 1) / 2),
          props.snapshot.trend.length - 1,
        ]),
      ].filter((n) => n >= 0)
    : [],
);
const active = computed(() =>
  hovering.value == null ? null : props.snapshot.trend[hovering.value],
);
watch(
  () => props.snapshot.range,
  () => {
    focus.value = 0;
    hide();
  },
);
function show(event: Event, index: number) {
  hovering.value = index;
  target.value = event.currentTarget as Element;
}
function hide() {
  hovering.value = null;
  target.value = null;
}
function key(event: KeyboardEvent, index: number) {
  if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
  event.preventDefault();
  focus.value =
    event.key === 'Home'
      ? 0
      : event.key === 'End'
        ? plot.value.points.length - 1
        : Math.max(
            0,
            Math.min(
              plot.value.points.length - 1,
              index + (event.key === 'ArrowRight' ? 1 : -1),
            ),
          );
  (event.currentTarget as HTMLElement).parentElement
    ?.querySelector<HTMLButtonElement>(`[data-trend-index="${focus.value}"]`)
    ?.focus();
}
</script>
<template>
  <section
    class="activity-section activity-trend"
    aria-labelledby="trend-title"
  >
    <header class="activity-section-heading">
      <div>
        <p class="activity-kicker">THE RHYTHM OF YOUR STORIES</p>
        <h2 id="trend-title">游玩趋势</h2>
      </div>
      <span class="activity-note"
        >{{ grain }}游玩时长 <span aria-hidden="true">·</span> UTC</span
      >
    </header>
    <figure
      class="activity-chart"
      :aria-label="`${grain}游玩时长趋势，总计${durationText(snapshot.summary.total_seconds)}`"
    >
      <div class="trend-y-axis" aria-hidden="true">
        <span>{{
          compactDuration(
            plot.max === 1 && !snapshot.summary.total_seconds ? 0 : plot.max,
          )
        }}</span
        ><span>{{
          compactDuration(snapshot.summary.total_seconds ? plot.max / 2 : 0)
        }}</span
        ><span>0</span>
      </div>
      <div class="trend-plot" @pointerleave="hide">
        <div class="trend-guides" aria-hidden="true"><i></i><i></i><i></i></div>
        <svg
          viewBox="0 0 800 190"
          preserveAspectRatio="none"
          aria-hidden="true"
        >
          <defs>
            <linearGradient :id="chartId" x1="0" y1="0" x2="0" y2="1">
              <stop
                offset="0%"
                stop-color="var(--activity-ink)"
                stop-opacity=".22"
              />
              <stop
                offset="100%"
                stop-color="var(--activity-ink)"
                stop-opacity="0"
              />
            </linearGradient>
          </defs>
          <path :d="plot.area" :fill="`url(#${chartId})`" />
          <path
            :d="plot.line"
            class="trend-line"
            pathLength="1"
            vector-effect="non-scaling-stroke"
          />
          <g v-if="hovering != null && plot.points[hovering]">
            <line
              :x1="plot.points[hovering].x"
              :x2="plot.points[hovering].x"
              :y1="plot.points[hovering].y"
              y2="190"
              class="trend-guide"
            />
            <circle
              :cx="plot.points[hovering].x"
              :cy="plot.points[hovering].y"
              r="4"
              class="trend-dot"
              vector-effect="non-scaling-stroke"
            />
          </g>
          <circle
            v-else-if="plot.points.length === 1"
            :cx="plot.points[0].x"
            :cy="plot.points[0].y"
            r="3"
            fill="var(--activity-ink)"
          />
        </svg>
        <button
          v-for="(point, index) in plot.points"
          :key="snapshot.trend[index].date"
          class="trend-point"
          :style="{ left: `${point.x / 8}%`, top: `${point.y / 1.9}%` }"
          :tabindex="focus === index ? 0 : -1"
          :data-trend-index="index"
          :aria-label="`${snapshot.trend[index].date}至${snapshot.trend[index].end_date}，${durationText(snapshot.trend[index].duration_seconds)}`"
          :aria-describedby="target ? tipId : undefined"
          @pointerenter="show($event, index)"
          @focus="
            focus = index;
            show($event, index);
          "
          @blur="hide"
          @click="show($event, index)"
          @keydown="key($event, index)"
        ></button>
      </div>
      <figcaption class="trend-x-axis" aria-hidden="true">
        <span v-for="index in tickIndices" :key="index">{{
          snapshot.trend[index].date.replaceAll('-', '.')
        }}</span>
      </figcaption>
    </figure>
    <details class="activity-data-table">
      <summary>查看{{ grain }}数据</summary>
      <div class="activity-table-scroll">
        <table>
          <caption class="sr-only">
            {{
              grain
            }}游玩时长数据
          </caption>
          <thead>
            <tr>
              <th>日期范围</th>
              <th>时长</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="point in snapshot.trend" :key="point.date">
              <td>
                {{ point.date
                }}{{
                  point.end_date !== point.date ? ` — ${point.end_date}` : ''
                }}
              </td>
              <td>{{ durationText(point.duration_seconds) }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </details>
    <ActivityTooltip
      :id="tipId"
      :target="target"
      :title="
        active
          ? `${active.date}${active.end_date !== active.date ? ` — ${active.end_date}` : ''}`
          : ''
      "
      :text="active ? durationText(active.duration_seconds) : ''"
      @dismiss="hide"
    />
  </section>
</template>
