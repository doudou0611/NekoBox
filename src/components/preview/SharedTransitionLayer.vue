<script setup lang="ts">
import {
  shared_overlay,
  overlay_cover,
  overlay_title,
  transition_debug,
} from '../../composables/useSharedTransition';
import PreviewCover from './PreviewCover.vue';
</script>
<template>
  <div
    class="shared-transition-host"
    aria-hidden="true"
    :data-transition-phase="transition_debug.phase"
    :data-transition-token="transition_debug.token"
    :data-overlay-count="transition_debug.overlay_count"
  >
    <template v-if="shared_overlay">
      <div
        ref="overlay_cover"
        class="shared-cover-layer"
        data-shared-overlay="cover"
        :style="{
          left: `${shared_overlay.cover_rect.left}px`,
          top: `${shared_overlay.cover_rect.top}px`,
          width: `${shared_overlay.cover_rect.width}px`,
          height: `${shared_overlay.cover_rect.height}px`,
          borderRadius: shared_overlay.border_radius,
        }"
      >
        <PreviewCover
          :cover_url="shared_overlay.cover_url"
          :title="shared_overlay.title"
        />
      </div>
      <div
        ref="overlay_title"
        class="shared-title-layer"
        data-shared-overlay="title"
        :style="{
          left: `${shared_overlay.title_rect.left}px`,
          top: `${shared_overlay.title_rect.top}px`,
          width: `${shared_overlay.title_rect.width}px`,
          fontSize: `${shared_overlay.font_size}px`,
          color: shared_overlay.color,
          letterSpacing: shared_overlay.letter_spacing,
        }"
      >
        {{ shared_overlay.title }}
      </div>
    </template>
  </div>
</template>
