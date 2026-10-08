<script setup lang="ts">
import type { PreviewGame } from '../../preview/data';
import CoverCard from '../preview/CoverCard.vue';
import { compactTime, relativePlayed } from '../../services/homeDashboard';
const props = defineProps<{
  games: PreviewGame[];
  context: string;
  recent?: boolean;
}>();
</script>
<template>
  <div class="home-strip-wrap">
    <div
      class="home-strip home-game-grid"
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
  </div>
</template>
<style scoped>
.home-strip {
  padding: 8px 8px 12px;
  margin: -8px;
}
.home-strip-item {
  min-width: 0;
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
  width: 100%;
  height: auto;
  aspect-ratio: 3/4;
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
</style>
