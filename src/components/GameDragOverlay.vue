<script setup lang="ts">
import { onUnmounted } from 'vue';
import { useWindowSize } from '@vueuse/core';
import { gameDrag, cancelGameDrag } from '../services/gameDrag';
const { width, height } = useWindowSize();
// This overlay owns no pointer events; the original game retains its identity.
onUnmounted(cancelGameDrag);
</script>
<template>
  <Teleport to="body">
    <div
      v-if="gameDrag"
      class="game-drag-overlay"
      :data-allowed="gameDrag.allowed"
      :style="{
        left: `${Math.max(8, Math.min(gameDrag.x + 14, width - 256))}px`,
        top: `${Math.max(60, Math.min(gameDrag.y, height - 60))}px`,
      }"
      aria-hidden="true"
    >
      <strong>{{ gameDrag.title }}</strong>
      <span>{{ gameDrag.hint }}</span>
    </div>
  </Teleport>
</template>
<style>
.game-drag-overlay {
  position: fixed;
  z-index: 240;
  pointer-events: none;
  transform: translateY(-50%);
  max-width: min(240px, calc(100vw - 32px));
  padding: 10px 14px;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-sm);
  background: var(--surface);
  color: var(--text);
  box-shadow: var(--shadow-modal);
}
.game-drag-overlay strong,
.game-drag-overlay span {
  display: block;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}
.game-drag-overlay strong {
  font-size: var(--type-small);
}
.game-drag-overlay span {
  margin-top: 4px;
  font-size: 11px;
  color: var(--muted);
}
.game-drag-overlay[data-allowed='true'] {
  border-color: var(--accent);
}
</style>
