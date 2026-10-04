<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue';
import { api, desktop, errorText, local, notify } from '../stores/library';

const props = defineProps<{ gameId: string; compact?: boolean }>();
const installations = computed(
  () => local.records[props.gameId]?.installations ?? [],
);
const scanning = ref('');
const error = ref('');
let generation = 0;
watch(
  () => props.gameId,
  () => {
    generation++;
    scanning.value = '';
    error.value = '';
  },
);
onUnmounted(() => generation++);

async function scan(installId: string) {
  if (scanning.value) return;
  const current = generation;
  scanning.value = installId;
  error.value = '';
  try {
    const count = await api('scan_screenshots', { install_id: installId });
    notify(`已索引 ${count} 张截图，可在“截图”页查看。`);
  } catch (e) {
    if (current === generation) error.value = errorText(e);
    else notify(errorText(e));
  } finally {
    if (current === generation) scanning.value = '';
  }
}
</script>
<template>
  <section
    v-if="desktop"
    class="screenshot-scan-panel"
    :class="{ compact }"
    :aria-labelledby="compact ? undefined : 'screenshot-scan-title'"
    aria-label="截图扫描"
  >
    <header v-if="!compact">
      <p class="eyebrow">SCREENSHOT SOURCES</p>
      <h3 id="screenshot-scan-title">截图扫描</h3>
      <p class="muted">读取游戏安装目录中的图片，扫描结果在“截图”页展示。</p>
    </header>
    <ul v-if="installations.length" class="screenshot-scan-sources">
      <li v-for="(installation, index) in installations" :key="installation.id">
        <div class="screenshot-scan-path">
          <strong>安装版本 {{ index + 1 }}</strong>
          <span>{{ installation.absolute_path }}</span>
        </div>
        <button
          class="secondary-button"
          :disabled="Boolean(scanning)"
          :aria-label="`扫描截图：${installation.absolute_path}`"
          @click="scan(installation.id)"
        >
          {{ scanning === installation.id ? '扫描中…' : '扫描截图' }}
        </button>
      </li>
    </ul>
    <p v-else class="muted">尚无安装版本，请先导入游戏。</p>
    <p v-if="error" class="screenshot-scan-error" role="alert">{{ error }}</p>
  </section>
</template>
<style scoped>
.screenshot-scan-panel {
  display: grid;
  gap: var(--space-16);
  min-width: 0;
  padding-top: var(--space-20);
  border-top: 1px solid var(--border);
}
.screenshot-scan-panel.compact {
  border: 0;
  padding: 0;
}
.screenshot-scan-panel h3 {
  margin: var(--space-8) 0;
}
.screenshot-scan-sources {
  display: grid;
  gap: var(--space-12);
  list-style: none;
  margin: 0;
  padding: 0;
}
.screenshot-scan-sources li {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-16);
  padding: var(--space-16);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  background: var(--surface-glass);
}
.screenshot-scan-path {
  display: grid;
  gap: var(--space-8);
  flex: 1 1 240px;
  min-width: 0;
}
.screenshot-scan-path strong {
  font-size: var(--type-small);
  font-weight: 500;
}
.screenshot-scan-path span {
  color: var(--muted);
  font-size: var(--type-small);
  overflow-wrap: anywhere;
}
.screenshot-scan-sources button {
  flex-shrink: 0;
}
.screenshot-scan-error {
  color: var(--danger);
}
</style>
