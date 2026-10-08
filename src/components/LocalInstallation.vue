<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import {
  local,
  api,
  desktop,
  errorText,
  notify,
  loadDetail,
  launchInstallation,
} from '../stores/library';
import SettingsChoice from './settings/SettingsChoice.vue';
import SettingsSwitch from './settings/SettingsSwitch.vue';
import GameProcessPicker from './GameProcessPicker.vue';
import PreviewIcon from './preview/PreviewIcon.vue';
import { launchEnvironment } from '../services/detailWorkspace';
import type { RunningProcess } from '../types/detailWorkspace';
import type { InstallationDetails } from '../types/local';
const isSteam = computed(() => detail.value?.source === 'steam');
// eslint-disable-next-line vue/prop-name-casing -- Existing detail component uses IPC naming.
const props = defineProps<{ game_id: string }>();
const game = computed(() => local.records[props.game_id]);
const selected = ref(''),
  detail = ref<InstallationDetails | null>(null);
const executable = ref(''),
  entryMode = ref(''),
  args = ref<string[]>([]),
  environment = ref<{ key: string; value: string }[]>([]),
  work = ref(''),
  mainProcess = ref('');
const trackHandoff = ref(true),
  idleTimeout = ref(0),
  steamAppId = ref(''),
  useLE = ref('inherit'),
  useMagpie = ref('inherit');
const toolOptions = [
  { value: 'inherit', label: '跟随全局' },
  { value: 'on', label: '启用' },
  { value: 'off', label: '关闭' },
];
const error = ref(''),
  busy = ref(false),
  loading = ref(false),
  processMessage = ref('');
let version = 0;
watch(
  () => [props.game_id, game.value?.installations.map((i) => i.id).join(',')],
  () => {
    if (!game.value?.installations.some((i) => i.id === selected.value))
      selected.value = game.value?.installations[0]?.id ?? '';
  },
  { immediate: true },
);
function assign(value: InstallationDetails) {
  detail.value = value;
  executable.value = value.executable_path ?? '';
  entryMode.value =
    executable.value &&
    !value.candidates.some((c) => c.path === executable.value)
      ? '__custom__'
      : executable.value;
  args.value = [...value.arguments];
  environment.value = Object.entries(value.environment).map(([key, value]) => ({
    key,
    value,
  }));
  work.value = value.working_directory ?? '';
  mainProcess.value = value.main_process_name ?? '';
  trackHandoff.value = value.track_after_launcher_exit;
  idleTimeout.value = value.idle_timeout_minutes ?? 0;
  steamAppId.value = value.steam_app_id ?? '';
  useLE.value =
    value.use_locale_emulator == null
      ? 'inherit'
      : value.use_locale_emulator
        ? 'on'
        : 'off';
  useMagpie.value =
    value.use_magpie == null ? 'inherit' : value.use_magpie ? 'on' : 'off';
}
watch(
  selected,
  async (id) => {
    const request = ++version;
    detail.value = null;
    error.value = '';
    processMessage.value = '';
    loading.value = !!id;
    if (!id || !desktop) return;
    try {
      const value = await api('get_installation', { install_id: id });
      if (request === version) assign(value);
    } catch (e) {
      if (request === version) error.value = errorText(e);
    } finally {
      if (request === version) loading.value = false;
    }
  },
  { immediate: true },
);
onBeforeUnmount(() => version++);
async function save() {
  if (!detail.value || busy.value) return;
  const id = selected.value,
    gameId = props.game_id;
  busy.value = true;
  error.value = '';
  try {
    const env = launchEnvironment(environment.value);
    const result = await api('configure_installation', {
      install_id: id,
      executable_path: executable.value,
      arguments: [...args.value],
      working_directory: work.value.trim() || null,
      environment: env,
      steam_app_id: steamAppId.value.trim() || null,
      main_process_name: mainProcess.value.trim() || null,
      track_after_launcher_exit: trackHandoff.value,
      idle_timeout_minutes: idleTimeout.value || null,
      use_locale_emulator:
        useLE.value === 'inherit' ? null : useLE.value === 'on',
      use_magpie:
        useMagpie.value === 'inherit' ? null : useMagpie.value === 'on',
    });
    if (id !== selected.value || gameId !== props.game_id) return;
    assign(result);
    await loadDetail(gameId);
    notify('启动配置已保存，下次启动时生效。');
  } catch (e) {
    if (id === selected.value) error.value = errorText(e);
  } finally {
    busy.value = false;
  }
}
async function choosePath(kind: 'executable' | 'work') {
  const id = selected.value;
  try {
    const { open } = await import('@tauri-apps/plugin-dialog');
    const file = await open(
      kind === 'work'
        ? {
            directory: true,
            multiple: false,
            defaultPath: work.value || detail.value?.absolute_path,
          }
        : {
            multiple: false,
            defaultPath: detail.value?.absolute_path,
            filters: [{ name: '游戏入口', extensions: ['exe', 'bat'] }],
          },
    );
    if (typeof file !== 'string' || id !== selected.value) return;
    if (kind === 'work') work.value = file;
    else {
      executable.value = file;
      entryMode.value = '__custom__';
    }
  } catch (e) {
    error.value = errorText(e);
  }
}
function chosenProcess(process: RunningProcess, applied: boolean) {
  mainProcess.value = process.name;
  trackHandoff.value = true;
  processMessage.value = applied
    ? `当前会话已切换到 ${process.name}（PID ${process.pid}）。保存配置可用于下次启动。`
    : `已选择 ${process.name}，保存配置后用于下次启动。`;
}
</script>
<template>
  <div class="detail-workspace launch-workspace">
    <header class="workspace-heading">
      <div>
        <p class="eyebrow">A DOOR TO THE STORY</p>
        <h2>启动</h2>
        <p>把入口准备好，让下一次相遇更轻松。</p>
      </div>
      <span class="workspace-status"
        ><PreviewIcon name="play" :size="15" />{{
          game?.installations.length || 0
        }}
        个安装版本</span
      >
    </header>
    <p v-if="!desktop" class="workspace-notice">
      离线原型没有真实安装实例，配置与进程选择仅在桌面版可用。
    </p>
    <section class="workspace-card" data-launch-section="installation">
      <header class="workspace-card-heading">
        <span class="workspace-number">01</span>
        <div>
          <h3>安装版本</h3>
          <p>每个版本保留独立的入口与启动偏好。</p>
        </div>
      </header>
      <label class="workspace-field"
        ><span>安装版本</span
        ><select v-model="selected" :disabled="busy || !desktop">
          <option v-if="!game?.installations.length" value="">
            尚无安装版本
          </option>
          <option
            v-for="(install, index) in game?.installations"
            :key="install.id"
            :value="install.id"
          >
            版本 {{ index + 1 }} · {{ install.absolute_path }}
          </option>
        </select></label
      >
      <p v-if="detail" class="workspace-hint">{{ detail.absolute_path }}</p>
      <p v-if="loading" role="status">正在读取此版本的配置…</p>
    </section>
    <fieldset
      class="workspace-editable"
      :disabled="busy || loading || !detail || !desktop"
    >
      <section
        v-if="isSteam"
        class="workspace-card"
        data-launch-section="entry"
      >
        <header class="workspace-card-heading">
          <span class="workspace-number">02</span>
          <div>
            <h3>通过 Steam 启动</h3>
            <p>AppID {{ steamAppId }} · 自动追踪安装目录中的游戏进程。</p>
          </div>
        </header>
      </section>
      <section v-else class="workspace-card" data-launch-section="entry">
        <header class="workspace-card-heading">
          <span class="workspace-number">02</span>
          <div>
            <h3>候选入口</h3>
            <p>选择实际运行游戏的文件。</p>
          </div>
        </header>
        <div class="workspace-path-row">
          <label class="workspace-field"
            ><span>候选入口</span
            ><select
              v-model="entryMode"
              @change="entryMode !== '__custom__' && (executable = entryMode)"
            >
              <option value="">请选择游戏入口</option>
              <option
                v-for="candidate in detail?.candidates"
                :key="candidate.path"
                :value="candidate.path"
              >
                {{
                  candidate.product_name || candidate.path.split(/[\\/]/).pop()
                }}
                · {{ candidate.path }}
              </option>
              <option value="__custom__">自定义入口</option>
            </select></label
          ><button
            class="secondary-button"
            type="button"
            @click="choosePath('executable')"
          >
            <PreviewIcon name="folder" :size="16" />浏览文件
          </button>
        </div>
        <label v-if="entryMode === '__custom__'" class="workspace-field"
          ><span>完整 EXE / BAT 路径</span
          ><input v-model="executable" placeholder="游戏安装目录内的入口"
        /></label>
      </section>
      <section class="workspace-card" data-launch-section="arguments">
        <header class="workspace-card-heading">
          <span class="workspace-number">03</span>
          <div>
            <h3>启动参数</h3>
            <p>每行一个参数，路径包含空格也无需额外引号。</p>
          </div>
        </header>
        <div class="workspace-edit-rows">
          <div
            v-for="(_, index) in args"
            :key="index"
            class="workspace-argument-row"
          >
            <span>{{ String(index + 1).padStart(2, '0') }}</span
            ><input
              v-model="args[index]"
              :aria-label="`启动参数 ${index + 1}`"
              placeholder="例如 --windowed"
              maxlength="4096"
            /><button
              class="icon-button"
              type="button"
              :aria-label="`移除启动参数 ${index + 1}`"
              @click="args.splice(index, 1)"
            >
              <PreviewIcon name="close" :size="16" />
            </button>
          </div>
        </div>
        <p v-if="!args.length" class="workspace-empty">
          没有附加参数，按默认方式启动。
        </p>
        <button
          class="quiet-button workspace-add"
          type="button"
          :disabled="args.length >= 128"
          @click="args.push('')"
        >
          <PreviewIcon name="plus" :size="16" />添加参数
        </button>
      </section>
      <section class="workspace-card" data-launch-section="work">
        <header class="workspace-card-heading">
          <span class="workspace-number">04</span>
          <div>
            <h3>工作目录</h3>
            <p>留空时使用当前游戏安装目录。</p>
          </div>
        </header>
        <div class="workspace-path-row">
          <label class="workspace-field"
            ><span>工作目录</span
            ><input
              v-model="work"
              :placeholder="detail?.absolute_path || '使用安装目录'" /></label
          ><button
            class="secondary-button"
            type="button"
            @click="choosePath('work')"
          >
            <PreviewIcon name="folder" :size="16" />选择目录
          </button>
        </div>
      </section>
      <section class="workspace-card" data-launch-section="environment">
        <header class="workspace-card-heading">
          <span class="workspace-number">05</span>
          <div>
            <h3>环境变量</h3>
            <p>为此游戏单独提供需要的环境值。</p>
          </div>
        </header>
        <div class="workspace-edit-rows">
          <div
            v-for="(row, index) in environment"
            :key="index"
            class="workspace-env-row"
          >
            <input
              v-model="row.key"
              :aria-label="`环境变量名称 ${index + 1}`"
              placeholder="变量名称"
              maxlength="128"
            /><span>=</span
            ><input
              v-model="row.value"
              :aria-label="`环境变量值 ${index + 1}`"
              placeholder="变量值"
              maxlength="4096"
            /><button
              class="icon-button"
              type="button"
              :aria-label="`移除环境变量 ${index + 1}`"
              @click="environment.splice(index, 1)"
            >
              <PreviewIcon name="close" :size="16" />
            </button>
          </div>
        </div>
        <p v-if="!environment.length" class="workspace-empty">
          没有单独配置环境变量。
        </p>
        <button
          class="quiet-button workspace-add"
          type="button"
          :disabled="environment.length >= 64"
          @click="environment.push({ key: '', value: '' })"
        >
          <PreviewIcon name="plus" :size="16" />添加环境变量
        </button>
      </section>
      <section class="workspace-card" data-launch-section="process">
        <header class="workspace-card-heading">
          <span class="workspace-number">06</span>
          <div>
            <h3>主进程选择</h3>
            <p>默认自动识别；游戏启动后，也可以手动选择真实进程。</p>
          </div>
        </header>
        <label class="workspace-field"
          ><span
            >主游戏进程名
            <small>{{ mainProcess ? '指定进程' : '自动识别' }}</small></span
          ><input
            v-model="mainProcess"
            maxlength="256"
            placeholder="自动识别，或填写 game.exe"
        /></label>
        <div class="workspace-process-actions">
          <GameProcessPicker
            :install-id="selected"
            :disabled="busy || !detail"
            @selected="chosenProcess"
          /><button
            class="quiet-button"
            type="button"
            :disabled="!mainProcess"
            @click="
              mainProcess = '';
              processMessage = '保存后恢复自动识别。';
            "
          >
            恢复自动识别
          </button>
        </div>
        <p v-if="processMessage" class="workspace-hint" role="status">
          {{ processMessage }}
        </p>
        <details class="workspace-extra">
          <summary>计时与启动器跟踪</summary>
          <SettingsSwitch
            v-model="trackHandoff"
            :disabled="isSteam"
            :description="
              isSteam
                ? 'Steam 安装始终跟踪实际游戏进程。'
                : '自动跟踪本次启动后安装目录内的新游戏进程。'
            "
            label="启动器关闭后继续跟踪游戏"
          /><label class="workspace-field"
            ><span>空闲计时</span
            ><select v-model.number="idleTimeout">
              <option :value="0">持续计时（适合阅读）</option>
              <option :value="5">5 分钟无输入后暂停</option>
              <option :value="10">10 分钟无输入后暂停</option>
              <option :value="30">30 分钟无输入后暂停</option>
            </select></label
          >
          <p class="workspace-hint">
            系统睡眠和休眠不计时。恢复键鼠输入后继续累加。
          </p>
        </details>
      </section>
      <section class="workspace-card" data-launch-section="tools">
        <header class="workspace-card-heading">
          <span class="workspace-number">07</span>
          <div>
            <h3>外挂工具</h3>
            <p>跟随全局偏好，或为这个版本单独设定。</p>
          </div>
        </header>
        <div class="workspace-tool-grid">
          <SettingsChoice
            v-if="!isSteam"
            v-model="useLE"
            label="Locale Emulator"
            :options="toolOptions"
          /><SettingsChoice
            v-model="useMagpie"
            label="Magpie"
            :options="toolOptions"
          />
        </div>
        <p class="workspace-hint">
          工具路径在设置中统一配置。Magpie 缩放方案由 Magpie 管理。
        </p>
      </section>
    </fieldset>
    <div
      v-if="desktop"
      class="workspace-save-bar"
      role="region"
      aria-label="启动配置保存"
    >
      <div>
        <p v-if="error" class="workspace-error" role="alert">{{ error }}</p>
        <p v-else>
          {{ detail ? '保存配置后，下次启动时生效。' : '请先导入安装版本。' }}
        </p>
      </div>
      <button
        class="secondary-button"
        :disabled="busy || (!detail?.executable_path && !isSteam)"
        @click="detail && launchInstallation(detail.id)"
      >
        启动此版本</button
      ><button class="primary-button" :disabled="busy || !detail" @click="save">
        <PreviewIcon name="check" :size="16" />{{
          busy ? '保存中…' : '保存启动配置'
        }}
      </button>
    </div>
  </div>
</template>
