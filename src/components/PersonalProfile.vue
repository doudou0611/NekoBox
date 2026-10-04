<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import {
  api,
  updateLibraryGame,
  desktop,
  errorText,
  loadDetail,
  local,
  notify,
} from '../stores/library';
const props = defineProps<{ gameId: string }>();
const rating = ref<number | null>(null);
const tags = ref('');
const busy = ref(false);
const error = ref('');
const game = computed(() => local.records[props.gameId]);
function sync() {
  rating.value = game.value?.user_rating ?? null;
  tags.value = game.value?.tags.map((t) => t.name).join(', ') ?? '';
}
async function save() {
  if (!game.value) return;
  busy.value = true;
  error.value = '';
  try {
    const nextRating = rating.value;
    if (
      !(await updateLibraryGame(props.gameId, () => ({
        user_rating: nextRating,
      })))
    ) {
      error.value = '评分保存失败，请重试。';
      return;
    }
    await api('replace_game_tags', {
      game_id: props.gameId,
      tag_names: tags.value
        .split(',')
        .map((v) => v.trim())
        .filter(Boolean),
    });
    await loadDetail(props.gameId);
    notify('评分和标签已保存。');
  } catch (e) {
    error.value = errorText(e);
  } finally {
    busy.value = false;
  }
}
watch(
  () => props.gameId,
  () => {
    sync();
  },
  { immediate: true },
);
watch(
  () => game.value?.user_rating,
  () => {
    if (!busy.value) sync();
  },
);
</script>
<template>
  <section
    v-if="desktop"
    class="personal-profile"
    aria-labelledby="personal-profile-title"
  >
    <div>
      <p class="eyebrow">YOUR MARKS</p>
      <h3 id="personal-profile-title">个人资料</h3>
    </div>
    <div class="profile-fields">
      <label
        >我的评分<select v-model="rating">
          <option :value="null">未评分</option>
          <option v-for="score in 10" :key="score" :value="score">
            {{ score }} / 10
          </option>
        </select></label
      ><label
        >标签（用逗号分隔）<input
          v-model="tags"
          maxlength="2000"
          placeholder="治愈, 音乐, 青春"
      /></label>
    </div>
    <p v-if="error" class="personal-error" role="alert">{{ error }}</p>
    <button class="primary-button" :disabled="busy" @click="save">
      {{ busy ? '保存中…' : '保存个人资料' }}
    </button>
  </section>
</template>
<style scoped>
.personal-profile {
  display: grid;
  gap: 16px;
  margin-top: 24px;
  padding-top: 20px;
  border-top: 1px solid var(--border);
}
.personal-profile h3 {
  margin: 4px 0 0;
}
.profile-fields {
  display: grid;
  grid-template-columns: minmax(140px, 220px) 1fr;
  gap: 16px;
}
.profile-fields label {
  display: grid;
  gap: 8px;
  color: var(--muted);
  font-size: 12px;
}
.profile-fields input,
.profile-fields select {
  padding: 10px 12px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--surface);
  color: var(--text);
}
.personal-error {
  color: var(--danger);
}
@media (max-width: 640px) {
  .profile-fields {
    grid-template-columns: 1fr;
  }
}
</style>
