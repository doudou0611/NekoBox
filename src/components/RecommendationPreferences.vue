<script setup lang="ts">
import { onMounted, ref } from 'vue';
import {
  api,
  errorText,
  notify,
  refreshRecommendations,
} from '../stores/library';
import type { RecommendationPreferenceEntry } from '../types/domain';
const items = ref<RecommendationPreferenceEntry[]>([]);
const busy = ref(false);
const error = ref('');
async function load() {
  busy.value = true;
  error.value = '';
  try {
    items.value = await api('list_recommendation_preferences', {});
  } catch (e) {
    error.value = errorText(e);
  } finally {
    busy.value = false;
  }
}
async function restore(gameId: string) {
  if (busy.value) return;
  busy.value = true;
  error.value = '';
  try {
    await api('set_recommendation_preference', {
      game_id: gameId,
      preference: 'none',
      expires_at: null,
    });
    items.value = items.value.filter((item) => item.game_id !== gameId);
    await refreshRecommendations();
    notify('已恢复此作品的推荐资格。');
  } catch (e) {
    error.value = errorText(e);
  } finally {
    busy.value = false;
  }
}
function label(item: RecommendationPreferenceEntry) {
  if (item.preference === 'not_interested') return '不感兴趣';
  if (!item.expires_at) return '暂不推荐';
  const date = new Date(item.expires_at);
  return date.getTime() <= Date.now()
    ? '暂缓已到期'
    : `暂不推荐至 ${date.toLocaleDateString('zh-CN')}`;
}
onMounted(load);
</script>
<template>
  <section class="settings-section">
    <div class="settings-intro">
      <h2>推荐偏好</h2>
      <p>让喜欢的故事重新出现。</p>
    </div>
    <div class="settings-card">
      <p v-if="error" class="preference-error" role="alert">{{ error }}</p>
      <p v-if="!items.length">
        {{ busy ? '正在读取偏好…' : '还没有排除或暂缓推荐的作品。' }}
      </p>
      <div v-for="item in items" :key="item.game_id" class="preference-row">
        <div>
          <strong>{{ item.game_title }}</strong
          ><small>{{ label(item) }}</small>
        </div>
        <button
          class="secondary-button"
          :disabled="busy"
          @click="restore(item.game_id)"
        >
          恢复推荐
        </button>
      </div>
      <button class="quiet-button" :disabled="busy" @click="load">
        重新读取
      </button>
    </div>
  </section>
</template>
<style scoped>
.preference-row {
  display: flex;
  gap: 16px;
  align-items: center;
  justify-content: space-between;
  padding: 12px 0;
  border-bottom: 1px solid var(--border);
  content-visibility: auto;
  contain-intrinsic-size: auto 72px;
}
.preference-row small {
  display: block;
  margin-top: 6px;
  color: var(--muted);
}
.preference-row strong {
  overflow-wrap: anywhere;
}
.preference-row button {
  flex-shrink: 0;
}
.preference-error {
  color: var(--danger);
}
</style>
