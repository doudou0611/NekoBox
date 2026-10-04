<script setup lang="ts">
import { computed } from 'vue';
import { desktop } from '../../stores/library';
import { preview } from '../../preview/store';
import {
  updates,
  checkAppUpdate,
  downloadAppUpdate,
  installAppUpdate,
  openUpdateRelease,
} from '../../stores/updates';
const progress = computed(() =>
  updates.status.total
    ? Math.min(100, (updates.status.downloaded / updates.status.total) * 100)
    : undefined,
);
const phaseLabel = computed(
  () =>
    ({
      idle: '自动检查已启用',
      checking: '正在检查',
      current: '已是最新版本',
      available: '发现新版本',
      downloading: '正在下载',
      ready: '准备安装',
      installing: '正在安装',
      error: '检查失败',
    })[updates.status.phase],
);
const size = (bytes: number) => `${(bytes / 1024 / 1024).toFixed(1)} MB`;
function confirmInstall() {
  preview.dialog = {
    title: `安装 NekoBox ${updates.status.version}？`,
    description:
      '安装包已通过签名校验。请先退出游戏并完成扫描、备份等任务。软件将关闭，安装后重新打开，原有 data 目录会保留。',
    confirm_label: '关闭软件并安装',
    action: () => {
      void installAppUpdate();
    },
  };
}
</script>
<template>
  <div
    class="settings-card settings-update-preview app-updates"
    :aria-busy="updates.busy"
  >
    <div class="update-orbit">
      <img
        class="app-brand-icon"
        src="/brand/nekobox.png"
        alt=""
        width="68"
        height="68"
      />
    </div>
    <h3>NekoBox</h3>
    <p class="settings-explanation">
      当前版本 {{ updates.status.current_version
      }}<template
        v-if="desktop && updates.status.architecture !== 'unsupported'"
      >
        · {{ updates.status.architecture === 'arm64' ? 'ARM64' : 'x64' }} ·
        {{
          updates.status.installation === 'portable' ? '便携版' : '安装版'
        }}</template
      >
    </p>
    <span class="mini-badge">{{
      desktop ? phaseLabel : '浏览器界面预览'
    }}</span>
    <p class="settings-explanation" role="status">
      {{ desktop ? updates.status.message : '远程更新需要在桌面版中检查。' }}
    </p>
    <template v-if="updates.status.version">
      <h4>新版本 {{ updates.status.version }}</h4>
      <details v-if="updates.status.notes" class="update-notes" open>
        <summary>更新说明</summary>
        <p>{{ updates.status.notes }}</p>
      </details>
    </template>
    <div
      v-if="updates.status.phase === 'downloading'"
      class="update-download"
      aria-live="polite"
    >
      <progress :value="progress" max="100" aria-label="安装包下载进度" />
      <p>
        {{ size(updates.status.downloaded)
        }}<template v-if="updates.status.total">
          / {{ size(updates.status.total) }} ·
          {{ Math.floor(progress ?? 0) }}%</template
        >
      </p>
    </div>
    <p v-if="updates.error" class="form-error" role="alert">
      {{ updates.error }}
    </p>
    <div class="update-actions">
      <button
        v-if="updates.status.phase === 'available' && updates.status.signed"
        class="primary-button"
        :disabled="updates.busy"
        @click="downloadAppUpdate"
      >
        下载更新
      </button>
      <button
        v-else-if="updates.status.phase === 'ready'"
        class="primary-button"
        :disabled="updates.busy"
        @click="confirmInstall"
      >
        安装并重新打开
      </button>
      <button
        v-else-if="
          updates.status.phase === 'available' && updates.status.download_url
        "
        class="primary-button"
        :disabled="updates.busy"
        @click="openUpdateRelease(true)"
      >
        {{
          updates.status.installation === 'portable'
            ? '下载便携包'
            : '下载安装包'
        }}
      </button>
      <button
        class="secondary-button"
        :disabled="
          !desktop ||
          updates.busy ||
          ['ready', 'installing'].includes(updates.status.phase)
        "
        @click="checkAppUpdate()"
      >
        {{ updates.status.phase === 'checking' ? '正在检查…' : '检查更新' }}
      </button>
      <button
        class="secondary-button"
        :disabled="!desktop"
        @click="openUpdateRelease()"
      >
        查看发布页面
      </button>
    </div>
  </div>
</template>
<style scoped>
.app-updates .settings-explanation {
  max-width: 560px;
  margin: 12px auto;
}
.app-updates h4 {
  margin-top: 20px;
  font-weight: 500;
}
.update-actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: 10px;
}
.update-notes {
  text-align: left;
  margin: 16px auto;
  max-width: 600px;
  border: 1px solid var(--border-subtle, var(--accent-wash));
  border-radius: 14px;
  padding: 14px 18px;
}
.update-notes summary {
  cursor: pointer;
}
.update-notes p {
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  line-height: 1.8;
  margin-top: 12px;
  max-height: 280px;
  overflow-y: auto;
}
.update-download {
  max-width: 360px;
  margin: 18px auto;
}
.update-download progress {
  width: 100%;
  accent-color: var(--accent);
  height: 10px;
}
.update-download p {
  margin-top: 8px;
  font-variant-numeric: tabular-nums;
}
</style>
