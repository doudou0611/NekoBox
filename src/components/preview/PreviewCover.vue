<script setup lang="ts">
import { ref, watch } from 'vue';
import { preview } from '../../preview/store';
import { desktop } from '../../stores/library';
import PreviewIcon from './PreviewIcon.vue';
import { remoteImage } from '../../services/remoteImages';
const props = withDefaults(
  defineProps<{
    // eslint-disable-next-line vue/prop-name-casing -- Existing cover data uses snake_case.
    cover_url: string;
    title: string;
    loading?: 'eager' | 'lazy';
  }>(),
  { loading: 'lazy' },
);
const failed = ref(false);
const resolved = ref('');
let token = 0;
watch(
  () => props.cover_url,
  async (url) => {
    const owner = ++token;
    failed.value = false;
    resolved.value = '';
    try {
      const value = await remoteImage(url);
      if (owner === token) resolved.value = value;
    } catch {
      if (owner === token) failed.value = true;
    }
  },
  { immediate: true },
);
</script>
<template>
  <div class="preview-cover">
    <img
      v-if="resolved && !failed && !preview.missing_cover"
      :src="resolved"
      :alt="`${title} · ${desktop ? '作品封面' : '离线自制演示封面'}`"
      draggable="false"
      :loading="loading"
      decoding="async"
      @error="failed = true"
    />
    <div
      v-else
      class="cover-placeholder"
      role="img"
      :aria-label="`${title} · 封面占位`"
    >
      <PreviewIcon name="spark" :size="40" /><span>故事的另一种留白</span
      ><small>{{
        desktop
          ? failed
            ? '封面加载失败，请在资料页重新刮削'
            : '封面待补充'
          : '演示封面暂不可用'
      }}</small>
    </div>
  </div>
</template>
