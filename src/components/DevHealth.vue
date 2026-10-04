<script setup lang="ts">
import { ref } from 'vue';
import { canInvokeDesktop, checkHealth } from '../services/health';
const desktop = canInvokeDesktop();
const pending = ref(false);
const status = ref(
  desktop ? '桌面环境：可检查 Rust 连通性' : '浏览器预览：Rust 后端不可用',
);
async function verify() {
  pending.value = true;
  try {
    const response = await checkHealth();
    status.value = response.success
      ? response.message + ' · ' + response.data.platform
      : response.message;
  } catch (error) {
    status.value =
      error instanceof Error
        ? error.message
        : '检查失败，本操作不修改游戏文件。';
  } finally {
    pending.value = false;
  }
}
</script>
<template>
  <section aria-label="开发连通性检查">
    <button :disabled="!desktop || pending" @click="verify">
      {{ pending ? '检查中…' : '检查 Rust 连通性' }}
    </button>
    <p role="status" aria-live="polite">{{ status }}</p>
  </section>
</template>
