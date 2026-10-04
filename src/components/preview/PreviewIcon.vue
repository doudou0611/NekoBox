<script setup lang="ts">
withDefaults(defineProps<{ name: string; size?: number }>(), { size: 20 });
const paths: Record<string, string> = {
  folder: 'M3 6h7l2 2h9v12H3zM3 6V4h6l2 2',
  lock: 'M6 10h12v10H6zM8 10V7a4 4 0 0 1 8 0v3M12 14v2',
  home: 'M3 10 12 3l9 7M5 9v11h5v-6h4v6h5V9',
  games: 'M4 4h6v7H4zM14 4h6v7h-6zM4 15h6v5H4zM14 15h6v5h-6z',
  activity: 'M3 17l5-6 4 3 7-10M3 4v16h18',
  saves: 'M3 6h7l2 2h9v12H3zM3 6V4h6l2 2',
  settings:
    'M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8M12 2v3M12 19v3M2 12h3M19 12h3M5 5l2 2M17 17l2 2M5 19l2-2M17 7l2-2',
  tune: 'M4 6h5M15 6h5M4 12h11M17 12h3M4 18h3M11 18h9M11 4v4M15 10v4M9 16v4',
  search: 'M10.5 3a7.5 7.5 0 1 0 0 15 7.5 7.5 0 0 0 0-15M16 16l5 5',
  heart: 'M12 20 3.8 12A5.2 5.2 0 0 1 12 5a5.2 5.2 0 0 1 8.2 7Z',
  arrow: 'M4 12h16M14 6l6 6-6 6',
  'chevron-down': 'm5 9 7 7 7-7',
  back: 'M20 12H4M10 6l-6 6 6 6',
  play: 'm8 5 12 7-12 7Z',
  plus: 'M12 4v16M4 12h16',
  close: 'm5 5 14 14M19 5 5 19',
  grid: 'M3 3h7v7H3zM14 3h7v7h-7zM3 14h7v7H3zM14 14h7v7h-7z',
  list: 'M8 5h13M8 12h13M8 19h13M3 5h1M3 12h1M3 19h1',
  wall: 'M3 4h5v16H3zM11 4h5v16h-5zM19 4h2v16h-2z',
  sun: 'M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8M12 2v2M12 20v2M2 12h2M20 12h2M5 5l1 1M18 18l1 1M5 19l1-1M18 6l1-1',
  moon: 'M20 14A9 9 0 0 1 10 3a9 9 0 1 0 10 11Z',
  spark: 'm12 3 2.3 6.7L21 12l-6.7 2.3L12 21l-2.3-6.7L3 12l6.7-2.3Z',
  clock: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18M12 7v5l3 2',
  check: 'm5 12 4 4L19 6',
  shuffle: 'M3 6h3l12 12h3M17 14l4 4-4 4M3 18h3l4-4M14 10l4-4h3M17 2l4 4-4 4',
  more: 'M4 12h1M11.5 12h1M19 12h1',
  warning: 'M12 3 2 21h20L12 3ZM12 9v5M12 17v1',
  info: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18M12 11v6M12 7v1',
  download: 'M12 3v12M7 10l5 5 5-5M4 20h16',
};
</script>
<template>
  <svg
    :width="size"
    :height="size"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.6"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <path :d="paths[name] ?? paths.spark" />
  </svg>
</template>
