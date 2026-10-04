<script setup lang="ts">
import SettingsSection from './settings/SettingsSection.vue';
import { onMounted, ref } from 'vue';
import { api, errorText, notify } from '../stores/library';
import {
  chooseDatabaseExportDirectory,
  chooseDatabaseSnapshot,
} from '../services/databaseTransfer';
import type { DatabaseImportPreview } from '../types/transfer';
const busy = ref(false);
const error = ref('');
const pending = ref(false);
const preview = ref<DatabaseImportPreview | null>(null);
const dialog = ref<HTMLDialogElement | null>(null);
const savedPath = ref('');
async function exportLibrary() {
  if (busy.value) return;
  busy.value = true;
  error.value = '';
  try {
    const directory = await chooseDatabaseExportDirectory();
    if (!directory) return;
    const result = await api('export_database', { directory });
    savedPath.value = result.path;
    notify('游戏库数据库已导出。');
  } catch (e) {
    error.value = errorText(e);
  } finally {
    busy.value = false;
  }
}
async function selectImport() {
  if (busy.value || pending.value) return;
  busy.value = true;
  error.value = '';
  try {
    const path = await chooseDatabaseSnapshot();
    if (!path) return;
    preview.value = await api('preview_database_import', { path });
    dialog.value?.showModal();
  } catch (e) {
    error.value = errorText(e);
  } finally {
    busy.value = false;
  }
}
async function confirmImport() {
  if (busy.value || !preview.value) return;
  busy.value = true;
  error.value = '';
  try {
    pending.value = (
      await api('confirm_database_import', {
        confirmation_token: preview.value.confirmation_token,
        confirmed: true,
      })
    ).pending_restart;
    dialog.value?.close();
    notify('数据库导入已安排，请退出并重新打开软件。');
  } catch (e) {
    error.value = errorText(e);
  } finally {
    busy.value = false;
  }
}
async function cancelImport() {
  if (busy.value) return;
  busy.value = true;
  error.value = '';
  try {
    pending.value = (await api('cancel_database_import', {})).pending_restart;
    dialog.value?.close();
    preview.value = null;
    notify('已取消数据库导入。');
  } catch (e) {
    error.value = errorText(e);
  } finally {
    busy.value = false;
  }
}
onMounted(async () => {
  try {
    const status = await api('database_transfer_status', {});
    pending.value = status.pending_restart;
    error.value = status.last_import_error ?? '';
  } catch (e) {
    error.value = errorText(e);
  }
});
</script>
<template>
  <SettingsSection
    title="数据库迁移"
    description="导出或导入游戏库数据库快照，迁移作品资料与游玩记录。"
  >
    <div class="settings-card">
      <p>
        导出作品资料、分组、笔记、截图索引与游玩记录。文件包含本地安装路径，请妥善保存。
      </p>
      <p class="settings-explanation">
        数据库快照不包含游戏、图片、存档备份或登录凭证；完整迁移还需保留原 data
        目录和游戏文件。
      </p>
      <p v-if="error && !dialog?.open" class="transfer-error" role="alert">
        {{ error }}
      </p>
      <p v-if="savedPath" class="saved-path">已保存：{{ savedPath }}</p>
      <p v-if="pending" role="status">
        导入将在下次启动时替换当前库。重启前新增的记录不会合并到导入库，可先导出保存，或取消导入。
      </p>
      <div class="preference-actions">
        <button
          class="secondary-button"
          :disabled="busy"
          @click="exportLibrary"
        >
          导出数据库
        </button>
        <button
          class="secondary-button"
          :disabled="busy || pending"
          @click="selectImport"
        >
          导入数据库
        </button>
        <button
          v-if="pending"
          class="quiet-button"
          :disabled="busy"
          @click="cancelImport"
        >
          取消待导入任务
        </button>
      </div>
    </div>
  </SettingsSection>
  <dialog
    ref="dialog"
    class="transfer-dialog"
    aria-labelledby="database-import-title"
    @cancel.prevent="!busy && cancelImport()"
  >
    <h2 id="database-import-title">替换当前游戏库？</h2>
    <p v-if="preview">
      备份包含 {{ preview.counts.games }} 个作品、{{
        preview.counts.installations
      }}
      个安装、{{ preview.counts.collections }} 个分组和
      {{ preview.counts.sessions }} 次游玩记录。
    </p>
    <p>
      确认后，下次启动会先备份当前数据库，再替换为所选数据库。这不会合并两份库，也不会复制或修改游戏与存档文件。确认有效期为五分钟。
    </p>
    <p v-if="error" class="transfer-error" role="alert">{{ error }}</p>
    <div class="preference-actions">
      <button class="secondary-button" :disabled="busy" @click="cancelImport">
        取消</button
      ><button class="primary-button" :disabled="busy" @click="confirmImport">
        确认导入，重启后生效
      </button>
    </div>
  </dialog>
</template>
<style scoped>
.saved-path {
  overflow-wrap: anywhere;
  color: var(--muted);
}
.transfer-error {
  color: var(--danger);
}
.transfer-dialog {
  inset: 0;
  margin: auto;
  width: min(580px, calc(100vw - 40px));
  border: 1px solid var(--border);
  border-radius: 20px;
  padding: 24px;
  background: var(--surface);
  color: var(--text);
}
.transfer-dialog::backdrop {
  background: #11182788;
  backdrop-filter: blur(8px);
}
.transfer-dialog p {
  line-height: 1.7;
}
</style>
