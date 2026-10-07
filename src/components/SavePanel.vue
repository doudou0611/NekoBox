<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue';
import {
  api,
  desktop,
  errorText,
  loadDetail,
  local,
  notify,
  preview,
} from '../stores/library';
import { subscribeEvent } from '../services/events';
import {
  chooseSaveDirectory,
  formatBytes,
  snapshotReason,
} from '../services/saves';
import type { SaveProfile, RestorePreview } from '../types/saves';
import type { SaveSnapshot } from '../types/domain';
import PreviewIcon from './preview/PreviewIcon.vue';
const props = defineProps<{ gameId: string }>();
const game = computed(() => local.records[props.gameId]);
const profiles = ref<SaveProfile[]>([]);
const snapshots = ref<SaveSnapshot[]>([]);
const installation = ref('');
const selectedProfile = ref('');
const source = ref('');
const before = ref(false);
const after = ref(false);
const retention = ref(10);
const label = ref('');
const note = ref('');
const candidates = ref<string[]>([]);
const pending = ref<RestorePreview | null>(null);
const busy = ref(false);
const error = ref('');
const visibleCount = ref(20);
const configurationOpen = ref(true);
const snapshotFilter = ref('all');
const snapshotSearch = ref('');
const loading = ref(desktop);
const operation = ref('正在更新存档');
const filters = [
  { value: 'all', label: '全部' },
  { value: 'manual', label: '手动' },
  { value: 'automatic', label: '自动' },
  { value: 'safety', label: '安全' },
];
const abort = new AbortController();
let revision = 0;
let operationRevision = 0;
let disposed = false;
let ownedAction: (() => void) | undefined;
const currentProfiles = computed(() =>
  profiles.value.filter((p) => p.install_id === installation.value),
);
const profile = computed(() =>
  profiles.value.find((p) => p.id === selectedProfile.value),
);
const currentSnapshots = computed(() =>
  snapshots.value
    .filter((s) => s.save_profile_id === selectedProfile.value)
    .sort((a, b) => b.created_at.localeCompare(a.created_at)),
);
const filteredSnapshots = computed(() => {
  const search = snapshotSearch.value.trim().toLocaleLowerCase();
  return currentSnapshots.value.filter((s) => {
    const category =
      s.creation_reason === 'manual'
        ? 'manual'
        : s.creation_reason === 'safety_before_restore'
          ? 'safety'
          : ['before_launch', 'after_exit'].includes(s.creation_reason)
            ? 'automatic'
            : 'other';
    return (
      (snapshotFilter.value === 'all' || snapshotFilter.value === category) &&
      (!search ||
        `${s.label ?? ''} ${s.note ?? ''} ${snapshotReason(s.creation_reason)}`
          .toLocaleLowerCase()
          .includes(search))
    );
  });
});
const latestSnapshot = computed(() => currentSnapshots.value[0]);
const totalSize = computed(() =>
  currentSnapshots.value.reduce((total, s) => total + s.size_bytes, 0),
);
const configurationDirty = computed(
  () =>
    !!profile.value &&
    (source.value !== profile.value.source_path ||
      before.value !== profile.value.backup_before_launch ||
      after.value !== profile.value.backup_after_exit ||
      retention.value !== profile.value.retention_count),
);
const validConfiguration = computed(
  () =>
    !!source.value &&
    !!installation.value &&
    Number.isInteger(retention.value) &&
    retention.value >= 1 &&
    retention.value <= 1000,
);
const statusText = computed(() =>
  !profile.value
    ? '等待配置'
    : !profile.value.source_available
      ? '目录不可访问'
      : profile.value.last_error
        ? '最近备份异常'
        : '存档目录可用',
);
function snapshotDate(value: string) {
  return new Date(value).toLocaleString('zh-CN', {
    month: 'long',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    year: 'numeric',
  });
}
watch([snapshotSearch, snapshotFilter], () => {
  visibleCount.value = 20;
});
const immutableSource = computed(() => currentSnapshots.value.length > 0);
function closeConfirmation() {
  if (ownedAction && preview.dialog?.action === ownedAction)
    preview.dialog = null;
  ownedAction = undefined;
}
function editProfile(id: string) {
  closeConfirmation();
  selectedProfile.value = id;
  configurationOpen.value = true;
  label.value = '';
  note.value = '';
  snapshotFilter.value = 'all';
  snapshotSearch.value = '';
  const existing = profiles.value.find((p) => p.id === id);
  source.value = existing?.source_path ?? '';
  before.value = existing?.backup_before_launch ?? false;
  after.value = existing?.backup_after_exit ?? false;
  retention.value = existing?.retention_count ?? 10;
  candidates.value = [];
  pending.value = null;
  visibleCount.value = 20;
}
async function refresh() {
  if (disposed) return;
  const owner = ++revision;
  const gameId = props.gameId;
  const [p, s] = await Promise.all([
    api('list_save_profiles', { game_id: gameId }),
    api('list_save_snapshots', { game_id: gameId }),
  ]);
  if (owner !== revision) return;
  profiles.value = p;
  snapshots.value = s;
  if (!game.value?.installations.some((i) => i.id === installation.value))
    installation.value = game.value?.installations[0]?.id ?? '';
  if (!p.some((item) => item.id === selectedProfile.value))
    editProfile(
      p.find((item) => item.install_id === installation.value)?.id ?? '',
    );
}
async function run(task: () => Promise<void>, message = '正在更新存档') {
  if (busy.value) return;
  busy.value = true;
  operation.value = message;
  error.value = '';
  const gameId = props.gameId;
  const owner = ++operationRevision;
  try {
    await task();
  } catch (e) {
    if (!disposed && owner === operationRevision && gameId === props.gameId)
      error.value = errorText(e);
  } finally {
    if (!disposed && owner === operationRevision && gameId === props.gameId) {
      busy.value = false;
      loading.value = false;
    }
  }
}
watch(
  () => props.gameId,
  () => {
    closeConfirmation();
    operationRevision++;
    busy.value = false;
    revision++;
    profiles.value = [];
    snapshots.value = [];
    installation.value = '';
    loading.value = desktop;
    editProfile('');
    if (desktop)
      void run(async () => {
        await loadDetail(props.gameId);
        await refresh();
      });
  },
  { immediate: true },
);
watch(installation, () => editProfile(currentProfiles.value[0]?.id ?? ''));
if (desktop) {
  for (const key of ['backup_created', 'session_ended'] as const) {
    void subscribeEvent(
      key,
      (event) => {
        if (event.payload.game_id === props.gameId && !busy.value)
          void refresh().catch((e) => (error.value = errorText(e)));
      },
      abort.signal,
    ).catch(() => {});
  }
}
onUnmounted(() => {
  disposed = true;
  operationRevision++;
  revision++;
  abort.abort();
  closeConfirmation();
});
function choose() {
  void run(async () => {
    const path = await chooseSaveDirectory();
    if (path) source.value = path;
  }, '正在选择存档目录');
}
function detect() {
  void run(async () => {
    candidates.value = await api('detect_save_paths', {
      install_id: installation.value,
    });
    if (candidates.value.length === 0)
      notify('没有找到 Save/save 目录，可手动选择其他存档目录。');
    else if (candidates.value.length === 1) source.value = candidates.value[0]!;
  }, '正在识别存档目录');
}
function save() {
  if (!validConfiguration.value) return;
  void run(async () => {
    const p = await api('configure_save_profile', {
      id: selectedProfile.value || null,
      install_id: installation.value,
      source_path: source.value,
      backup_before_launch: before.value,
      backup_after_exit: after.value,
      retention_count: retention.value,
    });
    await refresh();
    editProfile(p.id);
    configurationOpen.value = false;
    notify('存档配置已保存。');
  }, '正在保存存档配置');
}
function backup() {
  const id = selectedProfile.value;
  void run(async () => {
    await api('create_save_snapshot', {
      profile_id: id,
      label: label.value.trim() || null,
      note: note.value.trim() || null,
    });
    label.value = '';
    note.value = '';
    await refresh();
    notify('ZIP 快照已保存并通过 SHA-256 校验。');
  }, '正在备份当前存档，请勿关闭应用');
}
function removeProfile() {
  const p = profile.value;
  if (!p || busy.value || pending.value) return;
  const count = currentSnapshots.value.length;
  ownedAction = () => {
    void run(async () => {
      await api('delete_save_profile', { profile_id: p.id, confirmed: true });
      pending.value = null;
      await refresh();
      notify('存档配置已删除，原游戏存档和历史 ZIP 已保留。');
    }, '正在删除存档配置并保存回收信息');
  };
  preview.dialog = {
    title: '删除这份存档配置？',
    description: `目录：${p.source_path}。删除后停止此配置的自动备份，并从列表移除其 ${count} 份历史快照。原游戏存档不修改；历史 ZIP 和配置回收信息保留，可在此目录重新建立配置。`,
    confirm_label: '确认删除存档配置',
    action: ownedAction,
  };
}
function inspect(s: SaveSnapshot) {
  void run(async () => {
    pending.value = await api('preview_save_restore', { snapshot_id: s.id });
  }, '正在校验快照与文件差异');
}
function restore() {
  const p = pending.value;
  if (!p) return;
  ownedAction = () => {
    void run(async () => {
      const result = await api('restore_save_snapshot', {
        snapshot_id: p.snapshot_id,
        preview_id: p.preview_id,
        confirmation_token: p.confirmation_token,
      });
      pending.value = null;
      await refresh();
      notify(
        `存档已恢复：新增 ${result.added_files} 个、修改 ${result.modified_files} 个、删除 ${result.deleted_files} 个文件，恢复前安全快照已保存。`,
      );
    }, '正在创建安全备份并恢复，开始后不可取消');
  };
  preview.dialog = {
    title: '确认恢复这份存档？',
    description: `目标：${p.source_path}。将新增 ${p.added_files} 个、修改 ${p.modified_files} 个、删除 ${p.deleted_files} 个文件，完整还原快照当时的存档。先备份当前存档，再切换目录；开始后不可取消。`,
    confirm_label: '创建安全备份并恢复',
    action: ownedAction,
  };
}
function remove(s: SaveSnapshot) {
  ownedAction = () => {
    void run(async () => {
      await api('delete_save_snapshot', { snapshot_id: s.id, confirmed: true });
      if (pending.value?.snapshot_id === s.id) pending.value = null;
      await refresh();
      notify('快照已移入 data/save-backups/.trash，原游戏存档未修改。');
    }, '正在将快照移入回收目录');
  };
  preview.dialog = {
    title: '移除这份历史快照？',
    description:
      'ZIP 文件将移入便携备份目录内的回收文件夹，并从列表移除。此操作不修改当前游戏存档。',
    confirm_label: '确认移除快照',
    action: ownedAction,
  };
}
</script>
<template>
  <section class="save-panel" aria-label="存档管理" :aria-busy="busy">
    <header class="save-heading">
      <div class="save-heading-copy">
        <span class="save-kicker">STORY ARCHIVE</span>
        <h3>给故事，留一条回来的路。</h3>
        <p>收藏每一次选择，随时回到想念的章节。</p>
      </div>
      <button
        v-if="desktop"
        class="secondary-button save-refresh"
        :disabled="busy"
        @click="run(refresh)"
      >
        <PreviewIcon name="clock" :size="16" />刷新记录
      </button>
    </header>

    <div v-if="!desktop" class="save-empty save-card">
      <div class="save-empty-art" aria-hidden="true">
        <PreviewIcon name="saves" :size="36" /><span /><span />
      </div>
      <h4>故事的下一次重逢，从这里开始。</h4>
      <p>
        在 Windows 客户端中选择存档目录，即可创建和管理自己的快照。<br />浏览器预览不读取或备份真实存档。
      </p>
    </div>
    <template v-else>
      <Transition name="save-reveal">
        <p v-if="error" role="alert" class="save-notice save-notice--error">
          <PreviewIcon name="warning" :size="18" />{{ error }}
        </p>
      </Transition>
      <div
        v-if="loading"
        class="save-skeleton save-card"
        role="status"
        aria-label="正在读取存档配置"
      >
        <span /><span /><span />
        <p>正在读取存档配置与历史快照…</p>
      </div>
      <p v-else-if="busy" role="status" class="save-notice">
        <PreviewIcon name="clock" :size="18" />{{ operation }}…
      </p>
      <div
        v-if="!game?.installations.length && !loading"
        class="save-empty save-card"
      >
        <div class="save-empty-art">
          <PreviewIcon name="saves" :size="36" />
        </div>
        <h4>还没有可以留存的章节</h4>
        <p>先导入游戏目录，再为这个故事建立存档。</p>
      </div>
      <template v-if="game?.installations.length && !loading">
        <section class="save-overview save-card" aria-label="当前存档概览">
          <div class="save-overview-main">
            <div class="save-folder-mark">
              <PreviewIcon name="saves" :size="28" />
            </div>
            <div class="save-location">
              <span
                class="save-status"
                :class="{
                  'save-status--warning':
                    !profile?.source_available || profile?.last_error,
                }"
                ><i />{{ statusText }}</span
              >
              <h4>{{ profile ? '故事已在这里安放' : '为故事选择一个位置' }}</h4>
              <p class="save-source" :title="profile?.source_path">
                {{
                  profile?.source_path ||
                  '识别 Save / save，或选择你自己的存档目录。'
                }}
              </p>
            </div>
          </div>
          <dl class="save-stats">
            <div>
              <dt>历史快照</dt>
              <dd>{{ currentSnapshots.length }}<span>份</span></dd>
            </div>
            <div>
              <dt>备份体积</dt>
              <dd class="save-size">
                {{ currentSnapshots.length ? formatBytes(totalSize) : '—' }}
              </dd>
            </div>
            <div>
              <dt>最近留存</dt>
              <dd class="save-recent">
                {{
                  latestSnapshot
                    ? snapshotDate(latestSnapshot.created_at)
                    : '等待第一份快照'
                }}
              </dd>
            </div>
          </dl>
        </section>
        <div class="save-workspace">
          <aside class="save-setup save-card" aria-label="存档配置">
            <button
              class="save-disclosure"
              :aria-expanded="configurationOpen"
              aria-controls="save-configuration"
              :disabled="busy"
              @click="configurationOpen = !configurationOpen"
            >
              <span class="save-section-icon"
                ><PreviewIcon name="tune" :size="18"
              /></span>
              <span
                ><strong>存档设置</strong
                ><small>{{
                  configurationDirty
                    ? '有未保存的更改'
                    : profile
                      ? '目录与自动备份'
                      : '从选择目录开始'
                }}</small></span
              >
              <PreviewIcon
                name="chevron-down"
                :size="18"
                class="save-chevron"
                :class="{ 'is-open': configurationOpen }"
              />
            </button>
            <div
              id="save-configuration"
              class="save-collapse"
              :class="{ 'is-open': configurationOpen }"
              :inert="!configurationOpen"
              :aria-hidden="!configurationOpen"
            >
              <div class="save-collapse-inner">
                <form @submit.prevent="save">
                  <fieldset :disabled="busy" class="save-config-fields">
                    <label
                      >安装版本<select v-model="installation">
                        <option
                          v-for="i in game.installations"
                          :key="i.id"
                          :value="i.id"
                        >
                          {{ i.absolute_path }}
                        </option>
                      </select></label
                    >
                    <label v-if="currentProfiles.length"
                      >存档配置<select
                        :value="selectedProfile"
                        @change="
                          editProfile(
                            ($event.target as HTMLSelectElement).value,
                          );
                          configurationOpen = true;
                        "
                      >
                        <option value="">新建配置</option>
                        <option
                          v-for="p in currentProfiles"
                          :key="p.id"
                          :value="p.id"
                        >
                          {{ p.source_path }}
                        </option>
                      </select></label
                    >
                    <div class="save-path">
                      <span class="save-field-label">存档目录</span>
                      <p class="save-path-value">
                        {{ source || '尚未选择目录' }}
                      </p>
                      <div class="save-path-actions">
                        <button
                          type="button"
                          class="secondary-button"
                          :disabled="immutableSource"
                          @click="choose"
                        >
                          <PreviewIcon name="saves" :size="15" />选择目录
                        </button>
                        <button
                          type="button"
                          class="quiet-button"
                          :disabled="immutableSource"
                          @click="detect"
                        >
                          <PreviewIcon name="search" :size="15" />识别 Save/save
                        </button>
                      </div>
                      <p v-if="immutableSource" class="save-hint">
                        此配置已有快照。更换目录时请新建配置。
                      </p>
                      <button
                        v-if="selectedProfile"
                        type="button"
                        class="save-text-button"
                        @click="editProfile('')"
                      >
                        <PreviewIcon name="plus" :size="14" />新建配置
                      </button>
                    </div>
                    <div v-if="candidates.length > 1" class="save-candidates">
                      <span class="save-field-label">选择识别到的目录</span
                      ><button
                        v-for="path in candidates"
                        :key="path"
                        type="button"
                        class="secondary-button"
                        :aria-pressed="source === path"
                        @click="source = path"
                      >
                        {{ path }}
                      </button>
                    </div>
                    <div class="save-policy">
                      <span class="save-field-label"
                        >自动留存 <small>保存配置后生效</small></span
                      >
                      <label class="save-toggle-row"
                        ><span
                          ><strong>启动前备份</strong
                          ><small>启程之前，留住此刻</small></span
                        ><input
                          v-model="before"
                          type="checkbox"
                          role="switch"
                          aria-label="启动前备份" /><span
                          class="save-switch"
                          aria-hidden="true"
                      /></label>
                      <label class="save-toggle-row"
                        ><span
                          ><strong>退出后备份</strong
                          ><small>故事暂停，进度留存</small></span
                        ><input
                          v-model="after"
                          type="checkbox"
                          role="switch"
                          aria-label="退出后备份" /><span
                          class="save-switch"
                          aria-hidden="true"
                      /></label>
                      <label class="save-retention"
                        ><span
                          >建议保留快照数<small
                            >超过数量时提醒，由你清理</small
                          ></span
                        ><input
                          v-model.number="retention"
                          type="number"
                          min="1"
                          max="1000"
                          required
                      /></label>
                    </div>
                    <button
                      type="submit"
                      class="primary-button save-config-submit"
                      :disabled="!validConfiguration"
                    >
                      <PreviewIcon name="check" :size="16" />保存存档配置
                    </button>
                    <p class="save-hint">
                      快照保存在 exe 旁的 data/save-backups。自动备份默认关闭。
                    </p>
                    <button
                      v-if="profile"
                      type="button"
                      class="save-text-button save-profile-delete"
                      :disabled="!!pending"
                      @click="removeProfile"
                    >
                      <PreviewIcon name="close" :size="14" />删除存档配置
                    </button>
                    <p v-if="profile && pending" class="save-hint">
                      关闭恢复预览后可删除此配置。
                    </p>
                  </fieldset>
                </form>
              </div>
            </div>
            <div
              v-if="!configurationOpen && profile"
              class="save-policy-summary"
            >
              <PreviewIcon name="info" :size="15" />
              <p>
                {{
                  profile.backup_before_launch || profile.backup_after_exit
                    ? [
                        profile.backup_before_launch ? '启动前' : '',
                        profile.backup_after_exit ? '退出后' : '',
                      ]
                        .filter(Boolean)
                        .join('、') + '自动备份已开启'
                    : '自动备份尚未开启'
                }}
              </p>
            </div>
          </aside>

          <div class="save-archive">
            <p
              v-if="profile?.last_error"
              role="alert"
              class="save-notice save-notice--error"
            >
              <PreviewIcon name="warning" :size="18" />最近一次存档操作：{{
                profile.last_error
              }}
            </p>
            <p
              v-if="profile && !profile.source_available"
              role="alert"
              class="save-notice save-notice--error"
            >
              <PreviewIcon
                name="warning"
                :size="18"
              />此存档目录不可访问，请检查路径和文件权限。
            </p>
            <template v-if="profile">
              <form class="save-composer save-card" @submit.prevent="backup">
                <fieldset :disabled="busy">
                  <div class="save-section-heading">
                    <span class="save-section-icon"
                      ><PreviewIcon name="plus" :size="20"
                    /></span>
                    <div>
                      <h4>留住这一刻</h4>
                      <p>给当前进度一个名字，日后再回来。</p>
                    </div>
                  </div>
                  <div class="save-manual-fields">
                    <label
                      >快照名称 <small>可选</small
                      ><input
                        v-model="label"
                        maxlength="80"
                        placeholder="例如：共通线结束" /></label
                    ><label
                      >备注 <small>可选</small
                      ><input
                        v-model="note"
                        maxlength="1000"
                        placeholder="记录路线、章节，或此刻的心情"
                    /></label>
                  </div>
                  <div class="save-composer-footer">
                    <span
                      ><PreviewIcon name="check" :size="14" />ZIP 快照 · SHA-256
                      校验</span
                    ><button
                      type="submit"
                      class="primary-button"
                      :disabled="!profile.source_available"
                    >
                      <PreviewIcon name="plus" :size="16" />备份当前存档
                    </button>
                  </div>
                </fieldset>
              </form>
              <Transition name="save-reveal">
                <section
                  v-if="pending"
                  class="save-restore-preview save-card"
                  aria-label="恢复差异预览"
                >
                  <div class="save-section-heading">
                    <span class="save-section-icon"
                      ><PreviewIcon name="back" :size="20"
                    /></span>
                    <div>
                      <h4>回到这个章节</h4>
                      <p>先查看变化，再确认恢复。</p>
                    </div>
                    <button
                      class="quiet-button"
                      :disabled="busy"
                      @click="pending = null"
                    >
                      关闭预览
                    </button>
                  </div>
                  <p class="save-source">{{ pending.source_path }}</p>
                  <dl class="save-diff-stats">
                    <div>
                      <dt>新增文件</dt>
                      <dd>{{ pending.added_files }}</dd>
                    </div>
                    <div>
                      <dt>修改文件</dt>
                      <dd>{{ pending.modified_files }}</dd>
                    </div>
                    <div>
                      <dt>删除文件</dt>
                      <dd>{{ pending.deleted_files }}</dd>
                    </div>
                  </dl>
                  <ul v-if="pending.changes.length" class="save-changes">
                    <li v-for="c in pending.changes" :key="c.path">
                      <span>{{
                        c.change === 'added'
                          ? '新增'
                          : c.change === 'deleted'
                            ? '删除'
                            : '修改'
                      }}</span
                      ><code>{{ c.path }}</code>
                    </li>
                  </ul>
                  <p v-if="pending.truncated" class="save-hint">
                    仅展示前 200 个文件，以上数量包含全部变化。
                  </p>
                  <p
                    v-if="
                      !pending.added_files &&
                      !pending.modified_files &&
                      !pending.deleted_files
                    "
                    class="save-hint"
                  >
                    当前文件已与快照一致，无需恢复。
                  </p>
                  <div class="save-restore-footer">
                    <p class="save-hint">
                      恢复前先创建安全快照，再完整还原；备份后新增文件会被移除。<br />预览有效期
                      5 分钟，存档变化后需重新预览。
                    </p>
                    <button
                      v-if="
                        pending.added_files ||
                        pending.modified_files ||
                        pending.deleted_files
                      "
                      class="primary-button"
                      :disabled="busy"
                      @click="restore"
                    >
                      确认恢复…<PreviewIcon name="arrow" :size="16" />
                    </button>
                  </div>
                </section>
              </Transition>
              <section class="save-history save-card" aria-label="历史快照">
                <div class="save-section-heading">
                  <span class="save-section-icon"
                    ><PreviewIcon name="clock" :size="20"
                  /></span>
                  <div>
                    <h4>
                      故事的时间线
                      <span class="save-count">{{
                        currentSnapshots.length
                      }}</span>
                    </h4>
                    <p>每一次留存，都是可以重返的片刻。</p>
                  </div>
                </div>
                <p
                  v-if="currentSnapshots.length > profile.retention_count"
                  class="save-notice"
                >
                  已有
                  {{ currentSnapshots.length }}
                  份快照，超过建议数量；可确认移除不再需要的快照。
                </p>
                <template v-if="currentSnapshots.length">
                  <div class="save-history-tools">
                    <div
                      class="save-filters"
                      role="group"
                      aria-label="快照类型"
                    >
                      <button
                        v-for="item in filters"
                        :key="item.value"
                        :aria-pressed="snapshotFilter === item.value"
                        :class="{ active: snapshotFilter === item.value }"
                        @click="snapshotFilter = item.value"
                      >
                        {{ item.label }}
                      </button>
                    </div>
                    <label class="save-search"
                      ><PreviewIcon name="search" :size="16" /><input
                        v-model="snapshotSearch"
                        type="search"
                        aria-label="搜索快照"
                        placeholder="寻找某个章节…"
                    /></label>
                  </div>
                  <TransitionGroup
                    name="save-timeline"
                    tag="div"
                    class="save-timeline"
                  >
                    <article
                      v-for="s in filteredSnapshots.slice(0, visibleCount)"
                      :key="s.id"
                      class="save-snapshot"
                      :class="{ 'is-selected': pending?.snapshot_id === s.id }"
                    >
                      <div class="save-timeline-dot" aria-hidden="true">
                        <PreviewIcon
                          :name="
                            s.creation_reason === 'safety_before_restore'
                              ? 'check'
                              : s.creation_reason === 'manual'
                                ? 'saves'
                                : 'clock'
                          "
                          :size="15"
                        />
                      </div>
                      <div class="save-snapshot-content">
                        <div class="save-snapshot-title">
                          <h5>
                            {{ s.label || snapshotReason(s.creation_reason) }}
                          </h5>
                          <span
                            class="save-reason"
                            :class="{
                              'save-reason--safety':
                                s.creation_reason === 'safety_before_restore',
                            }"
                            >{{ snapshotReason(s.creation_reason) }}</span
                          >
                        </div>
                        <time :datetime="s.created_at">{{
                          snapshotDate(s.created_at)
                        }}</time>
                        <p v-if="s.note" class="save-snapshot-note">
                          {{ s.note }}
                        </p>
                        <div class="save-snapshot-footer">
                          <span class="save-file-meta"
                            >{{ formatBytes(s.size_bytes) }}<i />{{
                              s.file_count
                            }}
                            个文件</span
                          >
                          <div class="save-snapshot-actions">
                            <button
                              class="save-text-button"
                              :disabled="busy || !profile.source_available"
                              @click="inspect(s)"
                            >
                              预览恢复<PreviewIcon
                                name="back"
                                :size="14"
                              /></button
                            ><button
                              class="save-delete"
                              :disabled="busy"
                              :aria-label="`移除快照：${s.label || snapshotReason(s.creation_reason)}`"
                              title="移除快照"
                              @click="remove(s)"
                            >
                              <PreviewIcon name="close" :size="15" />
                            </button>
                          </div>
                        </div>
                        <details class="save-integrity">
                          <summary>SHA-256 校验值</summary>
                          <code>{{ s.sha256 }}</code>
                        </details>
                      </div>
                    </article>
                  </TransitionGroup>
                  <p
                    v-if="!filteredSnapshots.length"
                    class="save-no-results"
                    role="status"
                  >
                    没有找到对应的快照，试试其他名称或类型。
                  </p>
                  <button
                    v-if="filteredSnapshots.length > visibleCount"
                    class="secondary-button save-load-more"
                    @click="visibleCount += 20"
                  >
                    显示更多快照<PreviewIcon name="chevron-down" :size="16" />
                  </button>
                </template>
                <div v-else class="save-history-empty">
                  <PreviewIcon name="saves" :size="28" />
                  <h5>第一份回忆，还在等待。</h5>
                  <p>备份当前存档，让故事有迹可循。</p>
                </div>
              </section>
            </template>
            <div v-else class="save-onboarding save-card">
              <div class="save-empty-art" aria-hidden="true">
                <PreviewIcon name="saves" :size="36" /><span /><span />
              </div>
              <span class="save-kicker">YOUR FIRST CHAPTER</span>
              <h4>从一份存档开始，<br />收藏属于你的故事。</h4>
              <p>
                选择目录并保存配置后，<br />就可以在这里备份进度、浏览历史与预览恢复。
              </p>
              <div class="save-onboarding-steps">
                <span>01 选择目录</span
                ><PreviewIcon name="arrow" :size="14" /><span>02 保存配置</span
                ><PreviewIcon name="arrow" :size="14" /><span>03 留下快照</span>
              </div>
            </div>
          </div>
        </div>
      </template>
    </template>
  </section>
</template>
<style scoped>
.save-panel {
  width: 100%;
  min-width: 0;
  display: grid;
  gap: var(--space-24);
  container: save-panel / inline-size;
}
.save-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-16);
}
.save-kicker {
  color: var(--accent-ink, var(--accent));
  font-size: var(--type-caption);
  letter-spacing: 0.2em;
}
.save-heading h3 {
  font: 500 clamp(24px, 2.4vw, 32px)/1.5 var(--font-display);
  margin: var(--space-8) 0;
}
.save-panel p {
  font-size: 12px;
  line-height: 1.8;
  margin: 0;
}
.save-heading p,
.save-hint,
.save-section-heading p {
  color: var(--muted);
}
.save-card {
  min-width: 0;
  background: var(--surface-glass);
  border: 1px solid var(--border);
  border-radius: var(--radius-xl);
}
.save-overview {
  display: grid;
  gap: var(--space-24);
  padding: var(--space-24);
  position: relative;
  overflow: hidden;
  background:
    linear-gradient(120deg, var(--accent-wash), transparent 70%),
    var(--surface-glass);
}
.save-overview-main {
  display: flex;
  align-items: center;
  gap: var(--space-20);
  min-width: 0;
}
.save-folder-mark {
  display: grid;
  place-items: center;
  flex-shrink: 0;
  width: 64px;
  height: 64px;
  border-radius: var(--radius-lg);
  color: var(--accent-ink, var(--accent));
  background: var(--accent-wash);
  border: 1px solid var(--border);
  box-shadow: var(--shadow-subtle);
  transform: rotate(-5deg);
}
.save-location {
  min-width: 0;
}
.save-location h4 {
  font-size: 17px;
  font-weight: 500;
  margin: var(--space-8) 0 var(--space-4);
}
.save-source {
  color: var(--muted);
  overflow-wrap: anywhere;
  font-family: var(--font-mono);
  font-size: 11px !important;
}
.save-status {
  display: flex;
  align-items: center;
  gap: var(--space-8);
  color: var(--success);
  font-size: 11px;
}
.save-status i {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: currentColor;
  box-shadow: 0 0 0 4px color-mix(in srgb, currentColor 12%, transparent);
}
.save-status--warning {
  color: var(--warning);
}
.save-stats {
  display: grid;
  grid-template-columns: 0.65fr 0.85fr 1.5fr;
  gap: var(--space-16);
  margin: 0;
  padding-top: var(--space-20);
  border-top: 1px solid var(--border);
}
.save-stats > div {
  min-width: 0;
}
.save-stats dt,
.save-diff-stats dt {
  color: var(--muted);
  font-size: 11px;
  margin-bottom: var(--space-8);
}
.save-stats dd {
  margin: 0;
  font-size: 24px;
  font-weight: 400;
  font-variant-numeric: tabular-nums;
  line-height: 1.4;
}
.save-stats dd span {
  font-size: 11px;
  color: var(--muted);
  margin-left: var(--space-8);
}
.save-stats .save-size {
  font-size: 18px;
}
.save-stats .save-recent {
  font-size: 12px;
  line-height: 1.8;
}
.save-workspace,
.save-archive {
  display: grid;
  gap: var(--space-24);
  min-width: 0;
  align-items: start;
}
.save-setup {
  overflow: hidden;
}
.save-disclosure {
  display: flex;
  align-items: center;
  gap: var(--space-12);
  width: 100%;
  padding: var(--space-20);
  color: var(--text);
  background: transparent;
  border: 0;
  text-align: left;
  cursor: pointer;
}
.save-disclosure:hover {
  background: var(--accent-wash);
}
.save-disclosure strong {
  font-size: 14px;
  font-weight: 500;
}
.save-disclosure small {
  display: block;
  font-size: 11px;
  color: var(--muted);
  margin-top: var(--space-4);
}
.save-section-icon {
  display: grid;
  place-items: center;
  flex-shrink: 0;
  width: 36px;
  height: 36px;
  border-radius: var(--radius-md);
  background: var(--accent-wash);
  color: var(--accent-ink, var(--accent));
}
.save-chevron {
  margin-left: auto;
  transition: transform var(--layout-duration) var(--ease-standard);
}
.save-chevron.is-open {
  transform: rotate(180deg);
}
.save-collapse {
  display: grid;
  grid-template-rows: 0fr;
  visibility: hidden;
  opacity: 0;
  transition:
    grid-template-rows var(--layout-duration) var(--ease-standard),
    opacity var(--feedback-duration),
    visibility 0s var(--layout-duration);
}
.save-collapse.is-open {
  grid-template-rows: 1fr;
  visibility: visible;
  opacity: 1;
  transition-delay: 0s;
}
.save-collapse-inner {
  min-height: 0;
  overflow: hidden;
}
.save-panel fieldset {
  padding: 0;
  border: 0;
  min-width: 0;
  margin: 0;
}
.save-panel fieldset:disabled {
  opacity: 0.65;
}
.save-config-fields {
  display: grid;
  gap: var(--space-20);
  padding: 0 var(--space-20) var(--space-20) !important;
}
.save-panel label:not(.save-toggle-row):not(.save-retention):not(.save-search) {
  display: grid;
  grid-template-columns: 1fr auto;
  gap: var(--space-8);
  font-size: 11px;
  color: var(--muted);
  min-width: 0;
}
.save-panel label > input:not([type='checkbox']),
.save-panel label > select {
  grid-column: 1 / -1;
}
.save-panel label small {
  color: var(--subtle);
  font-size: var(--type-caption);
}
.save-field-label {
  display: block;
  color: var(--muted);
  font-size: 11px;
}
.save-panel input:not([type='checkbox']),
.save-panel select {
  width: 100%;
  min-width: 0;
  max-width: 100%;
  background: var(--surface);
  color: var(--text);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  padding: 11px 12px;
  font-size: 12px;
  transition:
    border-color var(--micro-duration),
    box-shadow var(--micro-duration);
}
.save-panel input::placeholder {
  color: var(--subtle);
}
.save-panel input:focus,
.save-panel select:focus {
  border-color: var(--accent-ink, var(--accent));
  box-shadow: 0 0 0 3px var(--accent-wash);
}
.save-panel select {
  text-overflow: ellipsis;
}
.save-path {
  display: grid;
  gap: var(--space-8);
}
.save-path-value {
  font-family: var(--font-mono);
  overflow-wrap: anywhere;
  background: var(--accent-wash);
  border: 1px dashed var(--border-strong);
  padding: var(--space-12);
  border-radius: var(--radius-md);
  font-size: 11px !important;
}
.save-path-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--space-8);
}
.save-path-actions button {
  padding: var(--space-8);
  font-size: 11px;
}
.save-candidates {
  display: grid;
  gap: var(--space-8);
}
.save-candidates button {
  text-align: left;
  overflow-wrap: anywhere;
  white-space: normal;
}
.save-candidates button[aria-pressed='true'] {
  border-color: var(--accent-ink, var(--accent));
  background: var(--accent-wash);
}
.save-policy {
  border-top: 1px solid var(--border);
  padding-top: var(--space-20);
}
.save-policy > .save-field-label {
  margin-bottom: var(--space-12);
}
.save-policy > .save-field-label small {
  font-size: var(--type-caption);
  margin-left: var(--space-8);
  color: var(--subtle);
}
.save-toggle-row {
  display: flex;
  align-items: center;
  gap: var(--space-12);
  padding: var(--space-12) 0;
  position: relative;
  cursor: pointer;
}
.save-toggle-row > span:first-child {
  flex: 1;
  min-width: 0;
}
.save-toggle-row strong {
  font-size: 12px;
  font-weight: 400;
}
.save-toggle-row small,
.save-retention small {
  display: block;
  margin-top: var(--space-4);
  font-size: var(--type-caption);
  color: var(--subtle);
}
.save-toggle-row input {
  position: absolute;
  right: 0;
  width: 38px;
  height: 24px;
  margin: 0;
  opacity: 0;
  cursor: pointer;
  z-index: 1;
}
.save-switch {
  width: 38px;
  height: 22px;
  flex-shrink: 0;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-pill);
  background: var(--surface-hover);
  transition:
    background var(--feedback-duration),
    border-color var(--feedback-duration);
}
.save-switch::after {
  content: '';
  display: block;
  width: 14px;
  height: 14px;
  margin: 3px;
  border-radius: 50%;
  background: var(--muted);
  box-shadow: var(--shadow-subtle);
  transition:
    transform var(--feedback-duration) var(--ease-standard),
    background var(--feedback-duration);
}
.save-toggle-row input:checked + .save-switch {
  background: var(--accent-ink, var(--accent));
  border-color: transparent;
}
.save-toggle-row input:checked + .save-switch::after {
  transform: translateX(16px);
  background: var(--surface);
}
.save-toggle-row input:focus-visible + .save-switch {
  outline: 2px solid var(--accent-ink, var(--accent));
  outline-offset: 4px;
}
.save-retention {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-12);
  margin-top: var(--space-12);
  padding-top: var(--space-16);
  border-top: 1px solid var(--border);
  font-size: 11px;
  color: var(--muted);
}
.save-retention input {
  width: 76px !important;
  text-align: center;
  flex-shrink: 0;
}
.save-config-submit {
  justify-content: center;
  width: 100%;
}
.save-hint {
  font-size: var(--type-caption) !important;
}
.save-policy-summary {
  display: flex;
  align-items: center;
  gap: var(--space-8);
  padding: 0 var(--space-20) var(--space-20);
  color: var(--muted);
}
.save-policy-summary p {
  font-size: 11px;
}
.save-composer,
.save-history,
.save-restore-preview {
  padding: var(--space-24);
}
.save-composer {
  background:
    linear-gradient(145deg, var(--accent-wash), transparent 55%),
    var(--surface-glass);
}
.save-composer fieldset {
  display: grid;
  gap: var(--space-20);
}
.save-section-heading {
  display: flex;
  align-items: center;
  gap: var(--space-12);
  min-width: 0;
}
.save-section-heading > div {
  min-width: 0;
}
.save-section-heading h4 {
  font-weight: 500;
  font-size: 16px;
  margin: 0 0 var(--space-4);
}
.save-section-heading p {
  font-size: 11px;
}
.save-section-heading > button {
  margin-left: auto;
  flex-shrink: 0;
}
.save-manual-fields {
  display: grid;
  gap: var(--space-16);
}
.save-composer-footer,
.save-restore-footer {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: var(--space-16);
  flex-wrap: wrap;
}
.save-composer-footer > span {
  display: flex;
  align-items: center;
  gap: var(--space-8);
  color: var(--muted);
  font-size: var(--type-caption);
}
.save-composer-footer .primary-button {
  margin-left: auto;
}
.save-notice {
  display: flex;
  align-items: center;
  gap: var(--space-12);
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
  padding: var(--space-12) var(--space-16);
  color: var(--muted);
  background: var(--accent-wash);
  overflow-wrap: anywhere;
}
.save-notice svg {
  flex-shrink: 0;
}
.save-notice--error {
  color: var(--danger);
  background: color-mix(in srgb, var(--danger) 7%, var(--surface));
  border-color: color-mix(in srgb, var(--danger) 22%, transparent);
}
.save-profile-delete {
  color: var(--danger);
  justify-self: start;
}
.save-restore-preview {
  display: grid;
  gap: var(--space-20);
  border-color: var(--border-strong);
  box-shadow: var(--shadow-subtle);
}
.save-diff-stats {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: var(--space-12);
  margin: 0;
}
.save-diff-stats > div {
  padding: var(--space-12);
  background: var(--accent-wash);
  border-radius: var(--radius-md);
}
.save-diff-stats dd {
  font-size: 24px;
  margin: 0;
  font-variant-numeric: tabular-nums;
  color: var(--accent-ink, var(--accent));
}
.save-changes {
  list-style: none;
  max-height: 220px;
  overflow: auto;
  display: grid;
  gap: var(--space-8);
  padding: var(--space-12);
  margin: 0;
  border: 1px solid var(--border);
  border-radius: var(--radius-md);
}
.save-changes li {
  display: flex;
  gap: var(--space-12);
  align-items: baseline;
}
.save-changes li span {
  font-size: var(--type-caption);
  color: var(--accent-ink, var(--accent));
  flex-shrink: 0;
}
.save-panel code {
  font-family: var(--font-mono);
  font-size: var(--type-caption);
  overflow-wrap: anywhere;
}
.save-count {
  display: inline-block;
  border-radius: var(--radius-pill);
  background: var(--accent-wash);
  padding: 2px var(--space-8);
  color: var(--accent-ink, var(--accent));
  font-family: var(--font-body);
  font-size: var(--type-caption);
  margin-left: var(--space-8);
  vertical-align: middle;
}
.save-history > .save-notice {
  margin-top: var(--space-20);
}
.save-history-tools {
  display: flex;
  align-items: center;
  gap: var(--space-12);
  flex-wrap: wrap;
  margin: var(--space-24) 0 var(--space-8);
}
.save-filters {
  display: flex;
  gap: var(--space-4);
  background: var(--surface);
  padding: var(--space-4);
  border: 1px solid var(--border);
  border-radius: var(--radius-pill);
}
.save-filters button {
  padding: 6px 14px;
  border: 0;
  border-radius: var(--radius-pill);
  background: transparent;
  color: var(--muted);
  font-size: 11px;
  transition:
    background var(--feedback-duration),
    color var(--feedback-duration),
    box-shadow var(--feedback-duration);
}
.save-filters button.active {
  background: var(--accent-wash);
  color: var(--accent-ink, var(--accent));
  box-shadow: var(--shadow-subtle);
}
.save-search {
  position: relative;
  display: flex;
  align-items: center;
  min-width: 140px;
  flex: 1;
}
.save-search svg {
  position: absolute;
  left: 12px;
  color: var(--subtle);
}
.save-search input {
  padding-left: 36px !important;
  background: transparent !important;
  border-radius: var(--radius-pill) !important;
}
.save-timeline {
  position: relative;
}
.save-snapshot {
  position: relative;
  display: flex;
  gap: var(--space-16);
  padding-top: var(--space-24);
  min-width: 0;
}
.save-snapshot::before {
  content: '';
  position: absolute;
  width: 1px;
  top: 0;
  bottom: 0;
  left: 14px;
  background: var(--border-strong);
}
.save-snapshot:first-child::before {
  top: 38px;
}
.save-snapshot:last-child::before {
  bottom: calc(100% - 38px);
}
.save-timeline-dot {
  display: grid;
  place-items: center;
  width: 29px;
  height: 29px;
  flex-shrink: 0;
  position: relative;
  border: 1px solid var(--border-strong);
  background: var(--surface);
  color: var(--accent-ink, var(--accent));
  border-radius: 50%;
  transition:
    background var(--feedback-duration),
    transform var(--feedback-duration) var(--ease-standard);
}
.save-snapshot:hover .save-timeline-dot {
  background: var(--accent-wash);
  transform: scale(1.08);
}
.save-snapshot-content {
  min-width: 0;
  flex: 1;
  padding: 0 0 var(--space-20);
  border-bottom: 1px solid var(--border);
}
.save-snapshot:last-child .save-snapshot-content {
  border-bottom: 0;
  padding-bottom: 0;
}
.save-snapshot-title {
  display: flex;
  align-items: baseline;
  gap: var(--space-8);
  flex-wrap: wrap;
  margin-bottom: var(--space-4);
}
.save-snapshot h5 {
  font-weight: 500;
  font-size: 14px;
  margin: 0;
  overflow-wrap: anywhere;
}
.save-reason {
  font-size: var(--type-caption);
  color: var(--muted);
  background: var(--surface-hover);
  border-radius: var(--radius-pill);
  padding: 2px var(--space-8);
}
.save-reason--safety {
  color: var(--success);
  background: color-mix(in srgb, var(--success) 10%, transparent);
}
.save-snapshot time {
  color: var(--subtle);
  font-size: var(--type-caption);
}
.save-snapshot-note {
  color: var(--muted);
  margin-top: var(--space-8) !important;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
.save-snapshot-footer {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-12);
  align-items: center;
  justify-content: space-between;
  margin-top: var(--space-12);
}
.save-file-meta {
  display: flex;
  align-items: center;
  gap: var(--space-8);
  color: var(--muted);
  font-size: var(--type-caption);
}
.save-file-meta i {
  width: 3px;
  height: 3px;
  background: var(--subtle);
  border-radius: 50%;
}
.save-snapshot-actions {
  display: flex;
  align-items: center;
  gap: var(--space-12);
}
.save-text-button {
  display: inline-flex;
  gap: var(--space-8);
  align-items: center;
  background: transparent;
  border: 0;
  padding: var(--space-4) 0;
  font-size: 11px;
  color: var(--accent-ink, var(--accent));
  cursor: pointer;
}
.save-text-button:hover {
  text-decoration: underline;
  text-underline-offset: 4px;
}
.save-delete {
  width: 28px;
  height: 28px;
  display: grid;
  place-items: center;
  border: 1px solid var(--border);
  background: transparent;
  color: var(--muted);
  border-radius: var(--radius-sm);
  transition:
    color var(--micro-duration),
    background var(--micro-duration),
    border-color var(--micro-duration);
}
.save-delete:hover {
  color: var(--danger);
  background: color-mix(in srgb, var(--danger) 8%, transparent);
  border-color: color-mix(in srgb, var(--danger) 25%, transparent);
}
.save-integrity {
  margin-top: var(--space-8);
  color: var(--subtle);
  font-size: var(--type-caption);
}
.save-integrity summary {
  width: fit-content;
  cursor: pointer;
}
.save-integrity code {
  display: block;
  margin-top: var(--space-8);
}
.save-snapshot.is-selected .save-timeline-dot {
  background: var(--accent-ink, var(--accent));
  color: var(--surface);
}
.save-load-more {
  margin: var(--space-24) auto 0;
  display: flex;
}
.save-no-results {
  text-align: center;
  padding: var(--space-32) 0;
  color: var(--muted);
}
.save-empty,
.save-onboarding {
  text-align: center;
  padding: var(--space-48) var(--space-24);
  display: grid;
  justify-items: center;
  gap: var(--space-16);
}
.save-empty h4,
.save-onboarding h4 {
  font: 500 24px/1.6 var(--font-display);
  margin: 0;
}
.save-empty p,
.save-onboarding p {
  color: var(--muted);
}
.save-empty-art {
  width: 100px;
  height: 100px;
  display: grid;
  place-items: center;
  position: relative;
  color: var(--accent-ink, var(--accent));
  margin: var(--space-12) 0 var(--space-24);
}
.save-empty-art::before,
.save-empty-art::after {
  content: '';
  position: absolute;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-xl);
  width: 80px;
  height: 80px;
  background: var(--surface);
  transform: rotate(-14deg) translate(-8px, -3px);
}
.save-empty-art::after {
  background: var(--accent-wash);
  transform: rotate(8deg) translate(4px, 5px);
}
.save-empty-art svg {
  position: relative;
  z-index: 1;
}
.save-empty-art > span {
  position: absolute;
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: var(--accent-ink, var(--accent));
  top: -4px;
  right: 5px;
}
.save-empty-art > span:last-child {
  width: 3px;
  height: 3px;
  top: auto;
  right: auto;
  bottom: 3px;
  left: -5px;
}
.save-onboarding {
  min-height: 440px;
  background:
    radial-gradient(ellipse at 50% 20%, var(--accent-wash), transparent 60%),
    var(--surface-glass);
}
.save-onboarding-steps {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  justify-content: center;
  gap: var(--space-12);
  padding-top: var(--space-20);
  color: var(--subtle);
  font-size: var(--type-caption);
}
.save-history-empty {
  text-align: center;
  padding: var(--space-40) var(--space-16);
  color: var(--muted);
}
.save-history-empty svg {
  color: var(--accent-ink, var(--accent));
}
.save-history-empty h5 {
  font: 500 20px var(--font-display);
  margin: var(--space-16) 0 var(--space-8);
  color: var(--text);
}
.save-skeleton {
  display: grid;
  gap: var(--space-16);
  padding: var(--space-32);
}
.save-skeleton span {
  height: 12px;
  border-radius: var(--radius-pill);
  background: var(--accent-wash);
}
.save-skeleton span:first-child {
  width: 40%;
  height: 24px;
}
.save-skeleton span:nth-child(2) {
  width: 65%;
}
.save-skeleton span:nth-child(3) {
  width: 85%;
}
.save-skeleton p {
  color: var(--muted);
}
.save-reveal-enter-active,
.save-reveal-leave-active,
.save-timeline-enter-active,
.save-timeline-leave-active,
.save-timeline-move {
  transition:
    opacity var(--feedback-duration),
    transform var(--layout-duration) var(--ease-standard);
}
.save-reveal-enter-from,
.save-reveal-leave-to,
.save-timeline-enter-from,
.save-timeline-leave-to {
  opacity: 0;
  transform: translateY(10px);
}
.save-timeline-leave-active {
  position: absolute;
  width: 100%;
}
@keyframes save-arrive {
  from {
    opacity: 0;
    transform: translateY(14px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
.save-overview,
.save-setup,
.save-composer,
.save-history {
  animation: save-arrive var(--hero-duration) var(--ease-standard) both;
}
.save-setup,
.save-composer {
  animation-delay: calc(var(--micro-duration) * 0.4);
}
.save-history {
  animation-delay: var(--micro-duration);
}
@keyframes save-loading {
  50% {
    opacity: 0.4;
  }
}
.save-skeleton span {
  animation: save-loading 1.6s ease-in-out infinite;
}
.save-skeleton span:nth-child(2) {
  animation-delay: 0.12s;
}
.save-skeleton span:nth-child(3) {
  animation-delay: 0.24s;
}
@container save-panel (min-width: 860px) {
  .save-workspace {
    grid-template-columns: 300px minmax(0, 1fr);
  }
  .save-overview {
    grid-template-columns: minmax(0, 1fr) minmax(0, 0.95fr);
    align-items: center;
  }
  .save-stats {
    border-top: 0;
    border-left: 1px solid var(--border);
    padding: 0 0 0 var(--space-24);
  }
  .save-manual-fields {
    grid-template-columns: minmax(0, 0.9fr) minmax(0, 1.1fr);
  }
}
@container save-panel (max-width: 480px) {
  .save-heading {
    align-items: flex-start;
    flex-direction: column;
  }
  .save-heading h3 {
    font-size: 24px;
  }
  .save-overview,
  .save-composer,
  .save-history,
  .save-restore-preview {
    padding: var(--space-20) var(--space-16);
    border-radius: var(--radius-lg);
  }
  .save-folder-mark {
    width: 44px;
    height: 44px;
  }
  .save-overview-main {
    gap: var(--space-12);
  }
  .save-stats {
    grid-template-columns: 1fr 1fr;
  }
  .save-stats > div:last-child {
    grid-column: 1 / -1;
  }
  .save-composer-footer > span {
    width: 100%;
  }
  .save-composer-footer .primary-button {
    width: 100%;
    justify-content: center;
  }
  .save-section-heading {
    flex-wrap: wrap;
  }
  .save-history-tools {
    flex-direction: column;
    align-items: stretch;
  }
  .save-filters {
    width: fit-content;
  }
  .save-snapshot {
    gap: var(--space-12);
  }
  .save-restore-footer > button {
    width: 100%;
    justify-content: center;
  }
}
:global(:root[data-motion='reduced']) .save-panel *,
:global(:root[data-motion='reduced']) .save-panel *::after {
  animation: none !important;
  transition: none !important;
}
:global(:root[data-motion='reduced']) .save-snapshot:hover .save-timeline-dot {
  transform: none;
}
@media (prefers-reduced-motion: reduce) {
  .save-panel *,
  .save-panel *::after {
    animation: none !important;
    transition: none !important;
  }
  .save-snapshot:hover .save-timeline-dot {
    transform: none;
  }
}
</style>
