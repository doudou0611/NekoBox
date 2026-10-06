<script setup lang="ts">
import { computed, ref } from 'vue';
import type { PreviewGame } from '../../preview/data';
import { toggleFavorite } from '../../stores/library';
import { motion_mode } from '../../composables/useMotionPolicy';
import { openPreviewGame } from '../../composables/useSharedTransition';
import PreviewCover from './PreviewCover.vue';
import PreviewIcon from './PreviewIcon.vue';
import PreviewTooltip from './PreviewTooltip.vue';
import { armGameDrag } from '../../services/gameDrag';
import { gameDownload, startHikariDownload } from '../../stores/hikariField';
const props = withDefaults(
  defineProps<{
    game: PreviewGame;
    context?: string;
    reason?: string;
    selectable?: boolean;
    selected?: boolean;
    selectionDisabled?: boolean;
    draggableGame?: boolean;
  }>(),
  {
    context: 'gallery',
    reason: '',
    selectable: false,
    selected: false,
    selectionDisabled: false,
    draggableGame: false,
  },
);
const emit = defineEmits<{ select: [gameId: string] }>();
const tilt = ref({ x: 0, y: 0 });
const download = computed(() => gameDownload(props.game.game_id));
function updateTilt(event: PointerEvent) {
  if (motion_mode.value !== 'full' || event.pointerType !== 'mouse') return;
  const bounds = (event.currentTarget as HTMLElement).getBoundingClientRect();
  tilt.value = {
    x: ((event.clientY - bounds.top) / bounds.height) * -4 + 2,
    y: ((event.clientX - bounds.left) / bounds.width) * 4 - 2,
  };
}
</script>
<template>
  <article
    class="cover-card"
    :class="{ 'is-selected': props.selectable && props.selected }"
    :data-game-id="game.game_id"
    :data-preview-card="game.game_id"
    :data-preview-card-key="`${context}:${game.game_id}`"
    :style="{
      '--cover-accent': game.accent,
      '--tilt-x': `${tilt.x}deg`,
      '--tilt-y': `${tilt.y}deg`,
    }"
    @pointerdown.capture="
      draggableGame && armGameDrag($event, game.game_id, selectionDisabled)
    "
    @dragstart="draggableGame && $event.preventDefault()"
    @pointermove="updateTilt"
    @pointerleave="tilt = { x: 0, y: 0 }"
  >
    <label v-if="props.selectable" class="card-select" @click.stop>
      <input
        type="checkbox"
        :checked="props.selected"
        :disabled="props.selectionDisabled"
        :aria-label="`选择${game.title}`"
        @change="emit('select', game.game_id)"
      />
    </label>
    <button
      class="cover-open"
      :data-preview-open="`${context}:${game.game_id}`"
      :aria-label="
        props.selectable
          ? `${props.selected ? '取消选择' : '选择'}${game.title}`
          : `查看${game.title}详情`
      "
      :disabled="props.selectable && props.selectionDisabled"
      @click="
        props.selectable
          ? emit('select', game.game_id)
          : openPreviewGame(game.game_id, $event)
      "
    >
      <PreviewCover
        class="card-art"
        :data-shared-cover="game.game_id"
        :cover_url="game.cover_url"
        :title="game.title"
      />
      <span class="card-text"
        ><span class="card-subtitle" :title="game.subtitle">{{
          game.subtitle
        }}</span
        ><span
          class="card-title"
          :data-shared-title="game.game_id"
          :title="game.title"
          >{{ game.title }}</span
        ><span v-if="reason" class="recommendation-reason">{{
          reason
        }}</span></span
      >
    </button>
    <button
      v-if="game.hikari_field && !game.launchable && !props.selectable"
      class="card-download icon-button"
      :aria-label="download ? `${game.title}正在下载` : `下载${game.title}`"
      :title="
        download
          ? '正在下载，可在左下角查看进度'
          : game.hikari_field.released
            ? '已拥有，尚未安装 · 下载游戏'
            : '已拥有，尚未开放下载'
      "
      :disabled="!!download || !game.hikari_field.released"
      @click.stop="startHikariDownload(game.game_id)"
    >
      <PreviewIcon :name="download ? 'clock' : 'download'" :size="18" />
    </button>
    <PreviewTooltip
      v-if="!props.selectable"
      :text="game.favorite ? '取消收藏' : '收藏这段故事'"
    >
      <button
        class="card-favorite icon-button"
        :class="{ 'is-favorite': game.favorite }"
        :aria-label="`${game.favorite ? '取消收藏' : '收藏'}${game.title}`"
        :aria-pressed="game.favorite"
        :data-favorite="game.game_id"
        @click="toggleFavorite(game.game_id)"
      >
        <PreviewIcon name="heart" />
      </button>
    </PreviewTooltip>
  </article>
</template>
<style scoped>
.cover-card.is-selected {
  outline: 2px solid var(--accent);
  outline-offset: 3px;
}
.card-download {
  position: absolute;
  left: var(--space-12);
  top: var(--space-12);
  border-radius: var(--radius-pill);
  color: var(--hero-ink);
  background: var(--hero-shade-soft);
  border-color: var(--edge-highlight);
  backdrop-filter: blur(6px);
}
.card-download:hover {
  background: var(--hero-shade);
}
.card-download:disabled {
  opacity: 0.85;
  cursor: default;
}
</style>
