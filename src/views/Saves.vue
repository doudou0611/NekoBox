<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { desktop, preview } from '../stores/library';
import SavePanel from '../components/SavePanel.vue';
const gameId = ref('');
const games = computed(() => preview.games);
watch(
  () => games.value.map((g) => g.game_id).join(','),
  () => {
    if (!games.value.some((g) => g.game_id === gameId.value))
      gameId.value = games.value[0]?.game_id ?? '';
  },
  { immediate: true },
);
</script>
<template>
  <div class="page-content">
    <header class="page-heading">
      <div>
        <p class="eyebrow">KEEP YOUR CHOICES SAFE</p>
        <h1 tabindex="-1" data-page-heading>每一次选择，都值得留存。</h1>
        <p class="page-lead">
          按作品和安装版本管理存档，给故事留一条回来的路。
        </p>
      </div>
    </header>
    <label v-if="desktop && games.length" class="save-game-select"
      >选择作品<select v-model="gameId">
        <option v-for="g in games" :key="g.game_id" :value="g.game_id">
          {{ g.title }}
        </option>
      </select></label
    >
    <p v-if="desktop && !games.length" class="muted">
      尚无作品，请先导入游戏目录。
    </p>
    <SavePanel v-if="gameId || !desktop" :key="gameId" :game-id="gameId" />
  </div>
</template>
<style scoped>
.save-game-select {
  display: grid;
  gap: 8px;
  margin-bottom: 24px;
  max-width: 480px;
  color: var(--muted);
  font-size: 12px;
}
select {
  color: var(--text);
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  padding: 12px;
}
</style>
