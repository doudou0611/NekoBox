<script setup lang="ts">
import { ref } from 'vue';
import type { PreviewGame } from '../../preview/data';
import CoverCard from '../preview/CoverCard.vue';
import PreviewIcon from '../preview/PreviewIcon.vue';
import { compactTime, relativePlayed } from '../../services/homeDashboard';
import { motion_mode } from '../../composables/useMotionPolicy';
const props = defineProps<{
  games: PreviewGame[];
  context: string;
  recent?: boolean;
}>();
const rail = ref<HTMLElement>();
function scroll(direction: number) {
  rail.value?.scrollBy({
    left: direction * rail.value.clientWidth * 0.8,
    behavior: motion_mode.value === 'reduced' ? 'instant' : 'smooth',
  });
}
</script>
<template>
  <div class="home-strip-wrap">
    <div
      ref="rail"
      class="home-strip"
      :aria-label="recent ? '最近作品' : '待游玩作品'"
    >
      <div
        v-for="game in props.games"
        :key="game.game_id"
        class="home-strip-item"
      >
        <CoverCard :game="game" :context="context" />
        <p
          :title="`累计 ${compactTime(game.playtime_seconds ?? game.duration_minutes * 60)}`"
        >
          {{
            recent
              ? relativePlayed(game.last_played_order)
              : game.launchable === false
                ? '待配置入口'
                : compactTime(
                    game.playtime_seconds ?? game.duration_minutes * 60,
                  )
          }}
        </p>
      </div>
    </div>
    <div v-if="games.length > 2" class="home-strip-controls">
      <button
        type="button"
        class="icon-button"
        aria-label="上一组作品"
        @click="scroll(-1)"
      >
        <PreviewIcon name="back" :size="16" /></button
      ><button
        type="button"
        class="icon-button"
        aria-label="下一组作品"
        @click="scroll(1)"
      >
        <PreviewIcon name="arrow" :size="16" />
      </button>
    </div>
  </div>
</template>
<style scoped>
.home-strip {
  display: flex;
  gap: var(--space-20);
  overflow-x: auto;
  scroll-snap-type: x proximity;
  padding: 8px 8px 12px;
  margin: -8px;
}
.home-strip-item {
  flex: 0 0 148px;
  min-width: 0;
  scroll-snap-align: start;
}
.home-strip-item p {
  font-size: var(--type-small);
  color: var(--muted);
  margin: 8px 2px 0;
}
.home-strip :deep(.cover-card) {
  border-radius: var(--radius-md);
  overflow: visible;
}
.home-strip :deep(.cover-open) {
  display: block;
}
.home-strip :deep(.card-art) {
  width: 148px;
  height: 194px;
  aspect-ratio: auto;
  border-radius: var(--radius-md);
  overflow: hidden;
}
.home-strip :deep(.card-text) {
  position: static;
  display: block;
  padding: 12px 0 0;
  background: none;
  color: var(--text);
}
.home-strip :deep(.card-title) {
  font-family: var(--font-body);
  font-size: 14px;
  line-height: 1.6;
  min-height: 44px;
}
.home-strip :deep(.card-subtitle),
.home-strip :deep(.card-number) {
  display: none;
}
.home-strip :deep(.card-favorite) {
  top: 8px;
  bottom: auto;
  right: 8px;
  width: 44px;
  height: 44px;
  min-height: 44px;
  background: var(--surface-glass);
}
.home-strip :deep(.cover-card > .tooltip-wrap) {
  position: absolute;
  top: 8px;
  right: 8px;
  bottom: auto;
}
.home-strip-controls {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
  margin-top: 8px;
}
.home-strip-controls button {
  width: 44px;
  height: 44px;
  min-height: 44px;
}
@container home (max-width:559px) {
  .home-strip-item {
    flex-basis: 136px;
  }
  .home-strip :deep(.card-art) {
    width: 136px;
    height: 182px;
  }
}
</style>
