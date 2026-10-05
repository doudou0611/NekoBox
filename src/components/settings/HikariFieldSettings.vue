<script setup lang="ts">
import SettingsSection from './SettingsSection.vue';
import { desktop } from '../../stores/library';
import { hikariField, changeHikariFolder } from '../../stores/hikariField';
</script>
<template>
  <SettingsSection
    title="HIKARI FIELD 游戏目录"
    description="为你已拥有的故事，留一个固定的位置。"
  >
    <div class="settings-card hf-settings">
      <label
        >游戏下载目录<output>{{
          hikariField.settings.root || '尚未设置 · 首次下载时选择'
        }}</output></label
      >
      <div class="hf-settings-footer">
        <p>
          全部游戏保存在 HikariFieldGames
          中。更改目录后，新下载使用新位置，已安装游戏仍可从原位置启动。
        </p>
        <button
          class="secondary-button"
          :disabled="
            !desktop ||
            hikariField.folder_busy ||
            hikariField.tasks.some((t) =>
              ['queued', 'running'].includes(t.status),
            )
          "
          @click="changeHikariFolder"
        >
          {{ hikariField.folder_busy ? '正在设置…' : '选择目录' }}
        </button>
      </div>
      <p v-if="hikariField.error" role="alert">{{ hikariField.error }}</p>
    </div>
  </SettingsSection>
</template>
<style scoped>
.hf-settings label {
  display: grid;
  gap: 12px;
  font-size: 13px;
  font-weight: 600;
}
.hf-settings output {
  padding: 14px 16px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--surface-hover);
  font-weight: 400;
  font-size: 13px;
  line-height: 1.7;
  overflow-wrap: anywhere;
}
.hf-settings-footer {
  display: flex;
  align-items: center;
  gap: 24px;
  margin-top: 16px;
}
.hf-settings-footer p {
  flex: 1;
  margin: 0;
  font-size: 12px;
  line-height: 1.8;
  color: var(--muted);
}
.hf-settings-footer button {
  flex-shrink: 0;
}
@media (max-width: 600px) {
  .hf-settings-footer {
    align-items: flex-start;
    flex-direction: column;
    gap: 16px;
  }
}
</style>
