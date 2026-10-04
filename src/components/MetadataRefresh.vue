<script setup lang="ts">
import SettingsSection from './settings/SettingsSection.vue';
import { computed, onMounted } from 'vue';
import { preview } from '../preview/store';
import { api, desktop, errorText } from '../stores/library';
import {
  metadataRefresh,
  pollMetadataRefresh,
  startMetadataRefresh,
} from '../stores/metadataRefresh';
const running = computed(() => metadataRefresh.state?.status === 'running');
onMounted(() => void pollMetadataRefresh());
function confirm() {
  preview.dialog = {
    title: '更新全部已导入游戏的元数据？',
    description:
      '按已绑定来源重新获取资料；开启翻译时重新翻译。手工内容、分组和游玩记录保留。未绑定来源的游戏会跳过，失败的游戏保留原资料。',
    confirm_label: '开始全库更新',
    action: () => {
      void startMetadataRefresh().catch(
        (e) => (metadataRefresh.error = errorText(e)),
      );
    },
  };
}
async function cancel() {
  try {
    await api('cancel_metadata_refresh', {});
  } catch (e) {
    metadataRefresh.error = errorText(e);
  }
}
</script>
<template>
  <SettingsSection
    title="更新已导入游戏"
    description="沿用已确认的来源重新获取全库资料，保留手工内容和个人记录。"
  >
    <article class="settings-card">
      <template v-if="metadataRefresh.state?.status">
        <p aria-live="polite">
          {{
            running
              ? (metadataRefresh.state.current ?? '正在准备')
              : '本轮更新已结束'
          }}
          · {{ metadataRefresh.state.processed }} /
          {{ metadataRefresh.state.total }}
        </p>
        <div class="settings-task-progress">
          <span
            :style="{
              width: `${metadataRefresh.state.total ? (metadataRefresh.state.processed / metadataRefresh.state.total) * 100 : 0}%`,
            }"
          />
        </div>
        <p class="settings-note">
          更新 {{ metadataRefresh.state.succeeded }} · 跳过
          {{ metadataRefresh.state.skipped }} · 失败
          {{ metadataRefresh.state.failed }}
        </p>
        <details v-if="metadataRefresh.state.messages.length">
          <summary>查看执行详情</summary>
          <p v-for="message in metadataRefresh.state.messages" :key="message">
            {{ message }}
          </p>
        </details>
      </template>
      <p v-if="metadataRefresh.error" class="settings-error" role="alert">
        {{ metadataRefresh.error }}
      </p>
      <div class="preference-actions">
        <button
          class="primary-button"
          :disabled="!desktop || running"
          @click="confirm"
        >
          {{ running ? '正在后台更新…' : '更新全库元数据' }}</button
        ><button v-if="running" class="secondary-button" @click="cancel">
          停止后续更新
        </button>
      </div>
      <p class="settings-note">
        可以离开本页，任务会继续执行。{{
          desktop ? '取消会保留已完成的更新。' : '桌面软件中可执行真实更新。'
        }}
      </p>
    </article>
  </SettingsSection>
</template>
