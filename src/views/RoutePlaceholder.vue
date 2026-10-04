<script setup lang="ts">
import { useRoute } from 'vue-router';
import DevHealth from '../components/DevHealth.vue';
const route = useRoute();
</script>
<template>
  <section>
    <h2>{{ route.meta.label }}</h2>
    <p>模块占位，功能尚未实现。此页面不执行扫描、启动或存档操作。</p>
    <p v-if="route.name === 'game-detail'">
      作品 ID：{{ route.params.game_id }}
    </p>
    <DevHealth v-if="route.name === 'settings'" />
  </section>
</template>
