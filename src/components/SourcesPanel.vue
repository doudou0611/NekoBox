<script setup lang="ts">
import { ref, watch } from 'vue';
import { api, desktop, errorText, loadDetail, notify } from '../stores/library';
import type { ExternalSource } from '../types/domain';
const props = defineProps<{ gameId: string; locked?: boolean }>();
const items = ref<ExternalSource[]>([]);
const error = ref('');
const busy = ref(false);
const value = ref('');
async function bind() {
  if (props.locked) return;
  busy.value = true;
  error.value = '';
  try {
    items.value = await api('bind_external_source', {
      game_id: props.gameId,
      install_id: null,
      provider: 'dlsite',
      value: value.value,
    });
    await loadDetail(props.gameId);
    notify(value.value.trim() ? '官方来源已绑定。' : '手动来源绑定已清除。');
  } catch (e) {
    error.value = errorText(e);
  } finally {
    busy.value = false;
  }
}
async function open(item: ExternalSource) {
  try {
    await api('open_external_source', { game_id: props.gameId, url: item.url });
  } catch (e) {
    error.value = errorText(e);
  }
}
async function refresh() {
  if (!desktop) return;
  busy.value = true;
  error.value = '';
  try {
    items.value = await api('list_external_sources', { game_id: props.gameId });
  } catch (e) {
    error.value = errorText(e);
  } finally {
    busy.value = false;
  }
}
watch(() => props.gameId, refresh, { immediate: true });
</script>
<template>
  <section v-if="desktop" class="sources-panel personal-panel">
    <header class="personal-heading">
      <div>
        <p class="eyebrow">OFFICIAL SOURCES</p>
        <h3>官方来源</h3>
      </div>
      <button class="quiet-button" :disabled="busy" @click="refresh">
        刷新
      </button>
    </header>
    <p v-if="error" class="personal-error" role="alert">{{ error }}</p>
    <p v-else-if="!items.length && !busy" class="muted">
      暂无已绑定的官方来源。可绑定 DLsite，或先完成 资料匹配。
    </p>
    <ul v-if="items.length" class="source-list">
      <li v-for="item in items" :key="item.url">
        <span class="source-badge">{{ item.kind.toUpperCase() }}</span>
        <div>
          <strong>{{ item.label }}</strong
          ><small>{{ item.remote_id || '官方页面' }}</small>
        </div>
        <button class="quiet-button" @click="open(item)">打开</button>
      </li>
    </ul>
    <form class="binding-form" @submit.prevent="bind">
      <label
        >DLsite 商品链接<input
          v-model="value"
          placeholder="https://www.dlsite.com/.../product_id/RJ….html"
          maxlength="2048"
          :disabled="locked"
      /></label>
      <button class="secondary-button" :disabled="busy || locked">
        保存绑定
      </button>
      <small class="muted"
        >留空保存会清除该手动绑定。链接在系统浏览器中打开。</small
      >
    </form>
  </section>
  <section v-else class="save-empty">
    <h3>版本与来源</h3>
    <p>浏览器预览不读取正式数据库来源。</p>
  </section>
</template>
<style scoped>
.binding-form {
  display: grid;
  gap: 12px;
}
.sources-panel {
  display: grid;
  gap: 16px;
}
.personal-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}
.personal-heading h3 {
  margin: 4px 0 0;
}
.personal-error {
  color: var(--danger);
}
.source-list {
  display: grid;
  gap: 10px;
  list-style: none;
  padding: 0;
  margin: 0;
}
.source-list li {
  display: grid;
  grid-template-columns: auto 1fr auto;
  align-items: center;
  gap: 12px;
  padding: 12px 14px;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface);
}
.source-list strong,
.source-list small {
  display: block;
}
.source-list small {
  color: var(--muted);
  margin-top: 3px;
}
.source-list a {
  color: var(--accent-ink, var(--accent));
}
.source-badge {
  padding: 4px 7px;
  border-radius: 999px;
  background: var(--accent-soft);
  color: var(--accent-ink, var(--accent));
  font-size: 10px;
  letter-spacing: 0.08em;
}
</style>
