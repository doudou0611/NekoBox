<script setup lang="ts">
import { useRoute, RouterLink } from 'vue-router';
import PreviewIcon from '../components/preview/PreviewIcon.vue';
import { desktop } from '../stores/library';
const route = useRoute();
</script>
<template>
  <div class="unavailable-page page-content">
    <header class="page-heading">
      <div>
        <p class="eyebrow">A CHAPTER STILL TO COME</p>
        <h1 tabindex="-1" data-page-heading>
          {{ route.meta.label }}，尚待下一次相遇。
        </h1>
      </div>
    </header>
    <section class="empty-stage unavailable-stage">
      <PreviewIcon :name="String(route.name)" :size="48" /><span
        class="mini-badge"
        >尚未实现 · 非可用功能</span
      >
      <h2>先把故事的入口，打磨得更温柔。</h2>
      <p>
        {{ desktop ? '当前先接通本地游戏库。' : '当前交付是视觉与交互原型。'
        }}{{ route.meta.label }}模块没有读取真实记录、执行备份或访问本地目录。
      </p>
      <RouterLink class="secondary-button" :to="{ name: 'home' }">
        回到作品展厅<PreviewIcon name="arrow" :size="18" />
      </RouterLink>
    </section>
  </div>
</template>
