<script setup lang="ts">
import SettingsSection from './settings/SettingsSection.vue';
import { computed, onMounted, reactive, ref, onUnmounted } from 'vue';
import { open } from '@tauri-apps/plugin-dialog';
import { api, desktop, errorText, notify } from '../stores/library';
import { preview } from '../preview/store';
import { createOperation, updateOperation } from '../stores/operations';
import {
  BACKUP_CATEGORIES,
  type BackupConfig,
  type ApplicationBackup as Backup,
  type BackupStatus,
  type ApplicationRestorePreview,
} from '../types/backup';
import SettingsChecklist from './settings/SettingsChecklist.vue';
import SettingsSwitch from './settings/SettingsSwitch.vue';
const config = reactive<BackupConfig>({
  categories: BACKUP_CATEGORIES.filter((v) => v.value !== 'credentials').map(
    (v) => v.value,
  ),
  directory: '',
  automatic: false,
  on_startup: false,
  on_game_exit: false,
  interval_minutes: 0,
  retention: 10,
  upload_local: false,
  webdav_url: '',
  webdav_directory: '',
  webdav_username: '',
});
const password = ref('');
const webdavPassword = ref('');
const hasBackupPassword = ref(false);
const hasWebdavPassword = ref(false);
const history = ref<Backup[]>([]);
const remote = ref<{ file_name: string }[]>([]);
const restore = ref<ApplicationRestorePreview | null>(null);
const restoreCategories = ref<string[]>([]);
const destinations = reactive<Record<string, string>>({});
const busy = ref(false);
const loaded = ref(!desktop);
const error = ref('');
const state = ref<BackupStatus | null>(null);
const resultMessage = ref('');
const credentials = computed(() => config.categories.includes('credentials'));
const restoreOptions = computed(() =>
  BACKUP_CATEGORIES.filter((v) => restore.value?.categories.includes(v.value)),
);
const disabled = computed(
  () => !desktop || !loaded.value || busy.value || !!state.value?.running,
);
let timer: ReturnType<typeof setInterval> | undefined;
const cloudLabels: Record<string, string> = {
  uploading: '正在上传',
  downloading: '正在下载',
  uploaded: '上传完成',
  downloaded: '下载完成',
  failed: '操作失败',
};
const date = (value: string) => new Date(value).toLocaleString('zh-CN');
const size = (value: number) => `${(value / 1024 / 1024).toFixed(1)} MB`;
async function load() {
  if (!desktop) return;
  try {
    const value = await api('get_backup_settings', {});
    const { has_backup_password, has_webdav_password, ...settings } = value;
    Object.assign(config, settings);
    hasBackupPassword.value = has_backup_password;
    hasWebdavPassword.value = has_webdav_password;
    loaded.value = true;
    history.value = await api('list_application_backups', {});
    state.value = await api('get_application_backup_status', {});
  } catch (e) {
    error.value = errorText(e);
  }
}
onMounted(() => {
  void load();
  if (desktop)
    timer = setInterval(() => {
      void api('get_application_backup_status', {})
        .then((s) => (state.value = s))
        .catch(() => {});
    }, 3000);
});
onUnmounted(() => clearInterval(timer));
async function run<T>(
  title: string,
  action: () => Promise<T>,
): Promise<T | undefined> {
  busy.value = true;
  error.value = '';
  const operation = createOperation(title, 'sync');
  updateOperation(operation.id, { status: 'running', message: title });
  try {
    const result = await action();
    updateOperation(operation.id, {
      status: 'completed',
      message: '操作已完成',
    });
    return result;
  } catch (e) {
    error.value = errorText(e);
    updateOperation(operation.id, { status: 'failed', message: error.value });
  } finally {
    busy.value = false;
    if (desktop)
      void api('get_application_backup_status', {}).then(
        (s) => (state.value = s),
      );
  }
}
async function chooseDirectory() {
  const path = await open({
    directory: true,
    multiple: false,
    title: '选择本地备份目录',
  });
  if (typeof path === 'string') config.directory = path;
}
async function save() {
  const action = async () => {
    const result = await run('保存备份与云端配置', () =>
      api('save_backup_settings', {
        ...config,
        categories: [...config.categories],
        webdav_password: webdavPassword.value || null,
        backup_password: password.value || null,
        confirmed_cleanup: true,
      }),
    );
    if (result) {
      hasBackupPassword.value = result.has_backup_password;
      hasWebdavPassword.value = result.has_webdav_password;
      webdavPassword.value = '';
      notify('备份配置已保存。');
    }
  };
  if (config.automatic)
    preview.dialog = {
      title: '启用自动备份与保留策略？',
      description: `新自动备份完整生成后，超出 ${config.retention} 份的旧自动包会被清理。手动包和云端包保留。${credentials.value ? '加密密码保存在系统凭据库，供自动备份使用。' : ''}`,
      confirm_label: '保存并启用',
      action: () => void action(),
    };
  else await action();
}
async function create() {
  const result = await run('备份所选应用数据', () =>
    api('create_application_backup', {
      categories: [...config.categories],
      directory: config.directory,
      password: password.value || null,
    }),
  );
  if (result) {
    resultMessage.value = `备份已保存：${result.path}`;
    history.value = await api('list_application_backups', {});
  }
}
async function openRestore(path?: string) {
  let value = path;
  if (!value) {
    const chosen = await open({
      multiple: false,
      title: '选择软件备份包',
      filters: [{ name: 'NekoBox 备份', extensions: ['gmbak'] }],
    });
    if (typeof chosen !== 'string') return;
    value = chosen;
  }
  const result = await run('校验软件备份包', () =>
    api('preview_application_restore', {
      path: value!,
      password: password.value || null,
    }),
  );
  if (result) {
    restore.value = result;
    restoreCategories.value = [...result.categories];
    Object.keys(destinations).forEach((k) => delete destinations[k]);
    Object.assign(destinations, result.save_paths);
  }
}
async function confirmRestore() {
  const value = restore.value;
  if (!value) return;
  preview.dialog = {
    title: '重启时恢复所选内容？',
    description: `备份包含 ${value.game_count} 部游戏。恢复将替换所选类别，保留未选类别；重启前先结束游戏。实际存档恢复只写入此页面确认的目录，并保留恢复前安全副本。`,
    confirm_label: '确认，重启时恢复',
    action: () => {
      void run('安排重启恢复', () =>
        api('confirm_application_restore', {
          preview_id: value.preview_id,
          confirmation_token: value.confirmation_token,
          confirmed: true,
          categories: [...restoreCategories.value],
          save_destinations: { ...destinations },
        }),
      ).then((result) => {
        if (result) {
          restore.value = null;
          notify('恢复已安排，请结束游戏后重新启动软件。');
        }
      });
    },
  };
}
async function cloudList() {
  const result = await run('读取云端备份列表', () =>
    api('list_webdav_backups', {}),
  );
  if (result) remote.value = result;
}
async function cloudTest() {
  const result = await run('测试 WebDAV 连接', () => api('test_webdav', {}));
  if (result) notify('WebDAV 目录与列表访问成功。');
}
async function upload(item: Backup) {
  const result = await run('上传本地备份', () =>
    api('upload_webdav_backup', { backup_id: item.backup_id }),
  );
  if (result) notify('备份已上传到云端。');
}
function deleteBackup(item: Backup) {
  if (disabled.value || restore.value || state.value?.pending_restore) return;
  preview.dialog = {
    title: '删除这份本地备份？',
    description: `备份时间：${date(item.created_at)}。本地文件：${item.path}。确认后将备份包移入该目录的 .trash 回收目录，并从列表移除。云端副本、游戏文件及当前存档保留。`,
    confirm_label: '确认删除备份',
    action: () => {
      if (disabled.value || restore.value || state.value?.pending_restore)
        return;
      void run('删除本地备份', async () => {
        const result = await api('delete_application_backup', {
          backup_id: item.backup_id,
          path: item.path,
          confirmed: true,
        });
        history.value = history.value.filter((v) => v.path !== item.path);
        resultMessage.value = `备份已移入回收目录：${result.trash_path}`;
        notify('本地备份已删除，文件保留在回收目录。');
        return result;
      });
    },
  };
}
async function download(file_name: string) {
  const result = await run('下载并校验云备份', () =>
    api('download_webdav_backup', {
      file_name,
      password: password.value || null,
    }),
  );
  if (result) {
    resultMessage.value = `云备份已下载：${result.path}`;
    history.value = await api('list_application_backups', {});
    await openRestore(result.path);
  }
}
</script>
<template>
  <SettingsSection
    title="本地备份与恢复"
    description="选择要保存的内容，创建备份或预览恢复已有备份。"
  >
    <article class="settings-card">
      <SettingsChecklist
        v-model="config.categories"
        label="备份内容"
        :options="BACKUP_CATEGORIES"
        :disabled="busy"
      />
      <p class="settings-note">
        各类别包含必要的作品
        ID、名称和安装归属记录。游戏本体不打包；删除本地游戏不会清空软件记录。缺失附件会提示失败，可取消该类别后备份其余内容。
      </p>
      <div class="preference-fields">
        <label class="preference-field"
          >本地备份目录
          <div class="preference-path">
            <input
              v-model="config.directory"
              placeholder="留空使用软件 data/application-backups"
              :disabled="busy"
            /><button
              class="secondary-button"
              :disabled="disabled"
              @click="chooseDirectory"
            >
              选择目录
            </button>
          </div></label
        >
        <label class="preference-field"
          >备份 / 恢复密码<input
            v-model="password"
            type="password"
            autocomplete="new-password"
            placeholder="勾选凭证时至少 8 字节"
            :disabled="busy"
          /><small>{{
            hasBackupPassword
              ? '自动备份密码已保存在系统凭据库。'
              : '含凭证时加密整个包；恢复时输入同一密码。'
          }}</small></label
        >
      </div>
      <p v-if="!desktop" class="settings-note">
        在桌面软件中可创建、校验与恢复真实备份。
      </p>
      <div class="preference-actions">
        <button
          class="primary-button"
          :disabled="
            disabled ||
            !config.categories.length ||
            (credentials && password.length < 8)
          "
          @click="create"
        >
          {{ busy ? '正在处理…' : '备份所选内容' }}</button
        ><button
          class="secondary-button"
          :disabled="disabled"
          @click="openRestore()"
        >
          选择备份并预览恢复
        </button>
      </div>
      <p
        v-if="resultMessage"
        class="settings-note settings-backup-path"
        role="status"
      >
        {{ resultMessage }}
      </p>
      <p v-if="state?.message" class="settings-note" aria-live="polite">
        {{ state.message }}
      </p>
      <p v-if="error" class="settings-error" role="alert">{{ error }}</p>
      <div v-if="state?.pending_restore" class="settings-backup-item">
        <div>
          <strong>恢复已安排</strong
          ><small>结束游戏并重启软件后应用所选内容。</small>
        </div>
        <button
          class="secondary-button"
          :disabled="disabled"
          @click="
            run('取消待恢复任务', () => api('cancel_application_restore', {}))
          "
        >
          取消待恢复
        </button>
      </div>
      <div class="settings-backup-history">
        <div
          v-for="item in history.slice(0, 20)"
          :key="item.path"
          class="settings-backup-item"
        >
          <div>
            <strong>{{ date(item.created_at) }}</strong
            ><small
              >{{
                item.reason === 'manual'
                  ? '手动备份'
                  : item.reason === 'cloud_download'
                    ? '云端下载'
                    : '自动备份'
              }}
              · {{ size(item.size_bytes) }} ·
              {{ item.encrypted ? '密码加密' : '标准备份包' }}</small
            ><small>{{ item.categories.length }} 个内容类别</small>
          </div>
          <div class="preference-actions preference-actions-inline">
            <button
              class="secondary-button"
              :disabled="disabled"
              @click="openRestore(item.path)"
            >
              恢复预览</button
            ><button
              class="secondary-button"
              :disabled="disabled || !config.webdav_url"
              @click="upload(item)"
            >
              上传云端
            </button>
            <button
              class="secondary-button settings-backup-delete"
              :disabled="disabled || !!restore || !!state?.pending_restore"
              :aria-label="`删除 ${date(item.created_at)} 的本地备份`"
              :title="
                restore || state?.pending_restore
                  ? '请先关闭恢复预览或取消待恢复任务'
                  : '删除本地备份，云端副本保留'
              "
              @click="deleteBackup(item)"
            >
              删除
            </button>
          </div>
        </div>
      </div>
    </article>
  </SettingsSection>
  <SettingsSection
    v-if="restore"
    title="恢复预览"
    description="核对备份内容与实际存档目录，确认后在重启时恢复。"
  >
    <article class="settings-card">
      <p class="settings-note">
        {{ date(restore!.created_at) }} · {{ restore!.game_count }} 部作品 ·
        {{ restore!.attachment_count }} 个附件
      </p>
      <SettingsChecklist
        v-model="restoreCategories"
        label="选择要恢复的类别"
        :options="restoreOptions"
        :disabled="busy"
      />
      <div
        v-if="restoreCategories.includes('current_saves')"
        class="preference-fields"
      >
        <label
          v-for="(path, id) in restore!.save_paths"
          :key="id"
          class="preference-field"
          >实际存档恢复目录<input
            v-model="destinations[id]"
            :placeholder="path"
          /><small
            >逐项确认目录。目标目录必须存在，不能选择整个游戏目录。</small
          ></label
        >
      </div>
      <div class="preference-actions">
        <button
          class="primary-button"
          :disabled="disabled || !restoreCategories.length"
          @click="confirmRestore"
        >
          确认重启时恢复</button
        ><button
          class="secondary-button"
          :disabled="busy"
          @click="restore = null"
        >
          关闭预览
        </button>
      </div>
    </article>
  </SettingsSection>
  <SettingsSection
    title="自动备份"
    description="选择备份时机与保留数量，软件恢复运行后补跑错过的计划。"
  >
    <article class="settings-card">
      <SettingsSwitch
        v-model="config.automatic"
        label="启用自动备份"
        description="默认关闭；使用上方已保存的内容选择与本地目录。"
        :disabled="busy"
      />
      <div v-if="config.automatic">
        <SettingsSwitch
          v-model="config.on_startup"
          label="每次打开软件"
          description="在异常会话修复与待恢复任务完成后备份。"
          :disabled="busy"
        />
        <SettingsSwitch
          v-model="config.on_game_exit"
          label="游戏退出并同步完成后"
          description="等待最终游玩记录与退出存档快照处理完成。"
          :disabled="busy"
        />
        <div class="preference-fields">
          <label class="preference-field"
            >定时间隔（分钟）<input
              v-model.number="config.interval_minutes"
              type="number"
              min="0"
              max="10080"
            /><small>0 为关闭定时备份；应用保持运行时执行。</small></label
          ><label class="preference-field"
            >自动包最多保留<input
              v-model.number="config.retention"
              type="number"
              min="1"
              max="100"
            /><small
              >默认 10 份，范围 1～100；手动包和云端包不自动清理。</small
            ></label
          >
        </div>
      </div>
    </article>
  </SettingsSection>
  <SettingsSection
    title="WebDAV 云备份"
    description="配置云端服务并上传或下载完整备份，恢复仍由你确认。"
  >
    <article class="settings-card">
      <div class="preference-fields">
        <label class="preference-field"
          >WebDAV 服务地址<input
            v-model="config.webdav_url"
            placeholder="https://example.com/dav/"
            autocomplete="off"
        /></label>
        <label class="preference-field"
          >云端相对目录<input
            v-model="config.webdav_directory"
            placeholder="NekoBox/backups"
          /><small>不会自动删除云端备份。</small></label
        >
        <label class="preference-field"
          >用户名<input v-model="config.webdav_username" autocomplete="off"
        /></label>
        <label class="preference-field"
          >WebDAV 密码<input
            v-model="webdavPassword"
            type="password"
            autocomplete="new-password"
            :placeholder="
              hasWebdavPassword ? '已保存，留空保留' : '仅保存在系统凭据库'
            "
        /></label>
      </div>
      <SettingsSwitch
        v-model="config.upload_local"
        label="本地备份同步云端"
        description="成功生成本地包后上传；失败保留本地包，可手动重试。"
        :disabled="busy"
      />
      <div class="preference-actions">
        <button class="primary-button" :disabled="disabled" @click="save">
          保存备份与云端配置</button
        ><button
          class="secondary-button"
          :disabled="disabled || !config.webdav_url"
          @click="cloudTest"
        >
          测试已保存连接</button
        ><button
          class="secondary-button"
          :disabled="disabled || !config.webdav_url"
          @click="cloudList"
        >
          读取云端备份
        </button>
      </div>
      <p v-if="state?.cloud_status" class="settings-note" aria-live="polite">
        云端状态：{{ cloudLabels[state.cloud_status] ?? state.cloud_status
        }}{{ state.last_upload_at ? ` · ${date(state.last_upload_at)}` : '' }}
      </p>
      <div class="settings-backup-history">
        <div
          v-for="item in remote"
          :key="item.file_name"
          class="settings-backup-item"
        >
          <strong class="settings-backup-path">{{ item.file_name }}</strong
          ><button
            class="secondary-button"
            :disabled="disabled"
            @click="download(item.file_name)"
          >
            下载并预览
          </button>
        </div>
      </div>
    </article>
  </SettingsSection>
</template>
