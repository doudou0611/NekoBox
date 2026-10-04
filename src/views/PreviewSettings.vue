<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, reactive, ref } from 'vue';
import { open } from '@tauri-apps/plugin-dialog';
import { preview } from '../preview/store';
import { api, desktop, errorText, notify } from '../stores/library';
import { global_glass_enabled } from '../composables/useDesktopMaterial';
import { PALETTES } from '../preview/palettes';
import {
  appSettings,
  loadAppSettings,
  saveAppSettings,
} from '../stores/settings';
import type { AppSettings } from '../types/settings';
import MetadataSettings from '../components/MetadataSettings.vue';
import TranslationSettings from '../components/TranslationSettings.vue';
import DatabaseTransfer from '../components/DatabaseTransfer.vue';
import MetadataRefresh from '../components/MetadataRefresh.vue';
import ApplicationBackup from '../components/ApplicationBackup.vue';
import SettingsSwitch from '../components/settings/SettingsSwitch.vue';
import SettingsChoice from '../components/settings/SettingsChoice.vue';
import SettingsSection from '../components/settings/SettingsSection.vue';

const chapters = [
  {
    id: 'appearance',
    title: '外观',
    description: '光色与材质，让每一次打开都合你心意。',
  },
  {
    id: 'metadata',
    title: '元数据',
    description: '从哪里获取故事，又以怎样的语言阅读。',
  },
  {
    id: 'launch',
    title: '启动设置',
    description: '启动工具、计时与网络，为游玩做好准备。',
  },
  {
    id: 'backup',
    title: '数据与备份',
    description: '为作品、记录和珍藏，留下一份安心。',
  },
  { id: 'updates', title: '应用更新', description: '关于当前版本与未来更新。' },
];
const active = ref('appearance');
const draft = reactive<AppSettings>({ ...appSettings.value });
const busy = ref(false);
const error = ref('');
const version = ref('');
const nav = ref<HTMLElement>();
const proxyProtocol = ref('http');
const proxyHost = ref('');
const proxyPort = ref('7890');
const coverOptions = [
  {
    value: 'hikarinagi',
    label: 'Hikarinagi 图床',
    description: '使用对应来源的图像服务',
  },
  {
    value: 'original',
    label: '原始图床',
    description: '直接使用来源网站的封面地址',
  },
];
async function save(keys: (keyof AppSettings)[]) {
  if (busy.value || !appSettings.loaded) return;
  busy.value = true;
  error.value = '';
  try {
    const value = { ...appSettings.value };
    for (const key of keys) Object.assign(value, { [key]: draft[key] });
    await saveAppSettings(value, keys);
    notify(desktop ? '设置已保存。' : '已在演示会话中应用。');
  } catch (cause) {
    error.value = errorText(cause);
  } finally {
    busy.value = false;
  }
}
async function saveProxy() {
  if (draft.proxy_mode === 'manual') {
    const host = proxyHost.value.trim();
    if (
      !host ||
      !/^\d+$/.test(proxyPort.value) ||
      Number(proxyPort.value) < 1 ||
      Number(proxyPort.value) > 65535
    ) {
      error.value = '请输入有效的代理主机与 1～65535 之间的端口。';
      return;
    }
    draft.proxy_url = `${proxyProtocol.value === 'socks5' ? 'socks5h' : proxyProtocol.value}://${host.includes(':') && !host.startsWith('[') ? `[${host}]` : host}:${proxyPort.value}`;
  }
  await save(['proxy_enabled', 'proxy_mode', 'proxy_url']);
}
async function chooseTool(key: 'locale_emulator_path' | 'magpie_path') {
  if (!desktop) return;
  try {
    const path = await open({
      multiple: false,
      directory: false,
      filters: [{ name: 'Windows 程序', extensions: ['exe'] }],
    });
    if (typeof path === 'string') draft[key] = path;
  } catch (cause) {
    error.value = errorText(cause);
  }
}
function jump(id: string) {
  const element = document.getElementById(`settings-${id}`);
  if (!element) return;
  const offset = (nav.value?.offsetHeight || 60) + 20;
  window.scrollTo({
    top: element.getBoundingClientRect().top + window.scrollY - offset,
    behavior: 'instant',
  });
  active.value = id;
  element.querySelector<HTMLElement>('h2')?.focus({ preventScroll: true });
}
let scrollFrame = 0;
function updateActive() {
  cancelAnimationFrame(scrollFrame);
  scrollFrame = requestAnimationFrame(() => {
    const threshold = (nav.value?.getBoundingClientRect().bottom || 60) + 32;
    let selected = chapters[0]!.id;
    for (const chapter of chapters) {
      if (
        (document
          .getElementById(`settings-${chapter.id}`)
          ?.getBoundingClientRect().top ?? Infinity) <= threshold
      )
        selected = chapter.id;
    }
    if (
      window.scrollY + window.innerHeight >=
      document.documentElement.scrollHeight - 3
    )
      selected = 'updates';
    active.value = selected;
  });
}
onMounted(async () => {
  window.addEventListener('scroll', updateActive, { passive: true });
  window.addEventListener('resize', updateActive);
  try {
    await loadAppSettings();
    Object.assign(draft, appSettings.value);
    if (draft.proxy_url) {
      const url = new URL(draft.proxy_url);
      proxyProtocol.value = url.protocol.startsWith('socks5')
        ? 'socks5'
        : url.protocol.slice(0, -1);
      proxyHost.value = url.hostname;
      proxyPort.value = url.port || '7890';
    }
    if (desktop) version.value = (await api('health_check', undefined)).version;
  } catch (cause) {
    error.value = errorText(cause);
  }
  await nextTick();
  updateActive();
});
onUnmounted(() => {
  window.removeEventListener('scroll', updateActive);
  window.removeEventListener('resize', updateActive);
  cancelAnimationFrame(scrollFrame);
});
</script>
<template>
  <div class="settings-page settings-v1 page-content">
    <header class="settings-v1-heading">
      <div>
        <h1 tabindex="-1" data-page-heading>设置</h1>
        <p>为你的作品展厅，调整每一个细节。</p>
      </div>
      <span v-if="!desktop" class="mini-badge">界面预览 · 仅会话内生效</span>
    </header>
    <nav ref="nav" class="settings-v1-nav" aria-label="设置分类快速定位">
      <button
        v-for="(chapter, index) in chapters"
        :key="chapter.id"
        :class="{ active: active === chapter.id }"
        :aria-current="active === chapter.id ? 'location' : undefined"
        :aria-controls="`settings-${chapter.id}`"
        @click="jump(chapter.id)"
      >
        <span>0{{ index + 1 }}</span
        >{{ chapter.title }}
      </button>
    </nav>
    <p v-if="error" class="preference-error" role="alert">{{ error }}</p>
    <section id="settings-appearance" class="settings-chapter">
      <header class="settings-chapter-heading">
        <span class="settings-chapter-number">01</span>
        <div>
          <h2 tabindex="-1">外观</h2>
          <p>{{ chapters[0]!.description }}</p>
        </div>
      </header>
      <SettingsSection
        title="光与材质"
        description="清晰的文字，柔和的层次。为故事换一束你喜欢的光。"
      >
        <div class="settings-card">
          <SettingsSwitch
            v-model="global_glass_enabled"
            label="全局模糊效果"
            description="侧栏与正文共用材质偏好，关闭后统一使用实色。"
          />
          <SettingsChoice
            v-model="preview.theme"
            label="主题"
            :options="[
              { value: 'light', label: '浅色展厅', description: '明亮而柔和' },
              {
                value: 'dark',
                label: '深色展厅',
                description: '安静的夜间氛围',
              },
            ]"
          />
          <h3>选择配色</h3>
          <div class="preference-palette-grid">
            <button
              v-for="palette in PALETTES"
              :key="palette.id"
              class="preference-palette"
              :class="{ selected: preview.palette === palette.id }"
              :aria-pressed="preview.palette === palette.id"
              :aria-label="`${palette.name}配色`"
              @click="preview.palette = palette.id"
            >
              <span
                class="preference-palette-swatch"
                :style="{ background: palette[preview.theme][0] }"
                ><i :style="{ background: palette[preview.theme][6] }" /><i
                  :style="{ background: palette[preview.theme][6] }" /><i
                  :style="{ background: palette[preview.theme][1] }" /></span
              ><strong>{{ palette.name }}</strong>
            </button>
          </div>
        </div>
      </SettingsSection>
      <SettingsSection
        title="打开时的风景"
        description="从展厅首页开始，或直接回到游戏库。"
      >
        <div class="settings-card">
          <SettingsChoice
            v-model="draft.startup_page"
            label="启动页"
            :options="[
              { value: 'home', label: '首页' },
              { value: 'games', label: '库' },
            ]"
            :disabled="busy || !appSettings.loaded"
          />
          <div class="preference-actions">
            <button
              class="secondary-button"
              :disabled="busy || !appSettings.loaded"
              @click="save(['startup_page'])"
            >
              保存启动页
            </button>
          </div>
        </div>
      </SettingsSection>
    </section>
    <section id="settings-metadata" class="settings-chapter">
      <header class="settings-chapter-heading">
        <span class="settings-chapter-number">02</span>
        <div>
          <h2 tabindex="-1">元数据</h2>
          <p>{{ chapters[1]!.description }}</p>
        </div>
      </header>
      <MetadataSettings />
      <SettingsSection
        title="封面与标签"
        description="图床选择相互独立。已保存的封面不自动替换。"
      >
        <form
          class="settings-card"
          @submit.prevent="
            save(['bangumi_cover_source', 'vndb_cover_source', 'tag_limit'])
          "
        >
          <SettingsChoice
            v-model="draft.bangumi_cover_source"
            label="Bangumi 封面来源"
            :options="coverOptions"
            :disabled="busy || !appSettings.loaded"
          /><SettingsChoice
            v-model="draft.vndb_cover_source"
            label="VNDB 封面来源"
            :options="coverOptions"
            :disabled="busy || !appSettings.loaded"
          /><label class="preference-field"
            >每部作品的自动标签上限<input
              v-model.number="draft.tag_limit"
              type="number"
              min="1"
              max="40"
              :disabled="busy || !appSettings.loaded"
            /><small
              >按来源顺序合并去重后保留 1～40 条，手工标签不受影响。</small
            ></label
          >
          <p class="settings-explanation">
            图床失败会显示真实错误，不自动切换。以上设置仅影响后续获取。
          </p>
          <div class="preference-actions">
            <button
              class="secondary-button"
              :disabled="busy || !appSettings.loaded"
            >
              保存封面与标签设置
            </button>
          </div>
        </form>
      </SettingsSection>
      <TranslationSettings />
      <MetadataRefresh />
    </section>
    <section id="settings-launch" class="settings-chapter">
      <header class="settings-chapter-heading">
        <span class="settings-chapter-number">03</span>
        <div>
          <h2 tabindex="-1">启动设置</h2>
          <p>{{ chapters[2]!.description }}</p>
        </div>
      </header>
      <SettingsSection
        title="启动与时长"
        description="等待真正的游戏进程，及时更新游玩中的记录。"
      >
        <form
          class="settings-card"
          @submit.prevent="save(['launch_wait_seconds', 'ui_refresh_seconds'])"
        >
          <div class="preference-fields">
            <label class="preference-field"
              >游戏进程启动等待（秒）<input
                v-model.number="draft.launch_wait_seconds"
                type="number"
                min="1"
                max="300"
                :disabled="busy || !appSettings.loaded"
              /><small>启动器退出后，继续等待主游戏进程。</small></label
            ><label class="preference-field"
              >游玩时长界面刷新（秒）<input
                v-model.number="draft.ui_refresh_seconds"
                type="number"
                min="1"
                max="300"
                :disabled="busy || !appSettings.loaded"
              /><small
                >仅影响界面刷新，数据库仍每 15 秒保存检查点。</small
              ></label
            >
          </div>
          <div class="preference-actions">
            <button
              class="secondary-button"
              :disabled="busy || !appSettings.loaded"
            >
              保存时间设置
            </button>
          </div>
        </form>
      </SettingsSection>
      <SettingsSection
        title="外部启动工具"
        description="为日文环境与窗口缩放，选择你熟悉的工具。"
      >
        <form
          class="settings-card"
          @submit.prevent="
            save([
              'locale_emulator_path',
              'magpie_path',
              'default_locale_emulator',
              'default_magpie',
            ])
          "
        >
          <div class="preference-fields">
            <label
              v-for="tool in [
                {
                  key: 'locale_emulator_path' as const,
                  label: 'Locale Emulator',
                },
                { key: 'magpie_path' as const, label: 'Magpie' },
              ]"
              :key="tool.key"
              class="preference-field preference-field-full"
              >{{ tool.label }} 路径
              <div class="preference-path">
                <input
                  v-model="draft[tool.key]"
                  placeholder="选择工具 EXE 的完整路径"
                  :disabled="busy || !appSettings.loaded"
                /><button
                  type="button"
                  class="secondary-button"
                  :disabled="!desktop || busy"
                  @click="chooseTool(tool.key)"
                >
                  选择文件
                </button>
              </div></label
            >
          </div>
          <SettingsSwitch
            v-model="draft.default_locale_emulator"
            label="默认使用 Locale Emulator"
            description="具体安装可单独覆盖。"
            :disabled="busy || !appSettings.loaded"
          /><SettingsSwitch
            v-model="draft.default_magpie"
            label="默认使用 Magpie"
            description="缩放辅助与游戏进程分别处理。"
            :disabled="busy || !appSettings.loaded"
          />
          <div class="preference-actions">
            <button
              class="secondary-button"
              :disabled="busy || !appSettings.loaded"
            >
              保存工具设置
            </button>
          </div>
        </form>
      </SettingsSection>
      <SettingsSection
        title="网络代理"
        description="为本应用的联网请求，选择合适的网络路径。"
      >
        <form class="settings-card" @submit.prevent="saveProxy">
          <SettingsSwitch
            v-model="draft.proxy_enabled"
            label="使用代理"
            description="关闭后本应用直接联网，不更改系统或游戏代理。"
            :disabled="busy || !appSettings.loaded"
          /><SettingsChoice
            v-if="draft.proxy_enabled"
            v-model="draft.proxy_mode"
            label="代理方式"
            :options="[
              { value: 'system', label: '跟随系统' },
              { value: 'manual', label: '自定义代理' },
            ]"
            :disabled="busy || !appSettings.loaded"
          /><template
            v-if="draft.proxy_enabled && draft.proxy_mode === 'manual'"
            ><SettingsChoice
              v-model="proxyProtocol"
              label="协议"
              :options="[
                { value: 'http', label: 'HTTP' },
                { value: 'https', label: 'HTTPS' },
                { value: 'socks5', label: 'SOCKS5' },
              ]" />
            <div class="preference-fields">
              <label class="preference-field"
                >服务器<input
                  v-model="proxyHost"
                  placeholder="127.0.0.1" /></label
              ><label class="preference-field"
                >端口<input
                  v-model="proxyPort"
                  type="number"
                  min="1"
                  max="65535"
              /></label></div
          ></template>
          <div class="preference-actions">
            <button
              class="secondary-button"
              :disabled="busy || !appSettings.loaded"
            >
              保存代理设置
            </button>
          </div>
        </form>
      </SettingsSection>
    </section>
    <section id="settings-backup" class="settings-chapter">
      <header class="settings-chapter-heading">
        <span class="settings-chapter-number">04</span>
        <div>
          <h2 tabindex="-1">数据与备份</h2>
          <p>{{ chapters[3]!.description }}</p>
        </div>
      </header>
      <ApplicationBackup />
      <DatabaseTransfer v-if="desktop" />
      <SettingsSection
        v-else
        title="数据库迁移"
        description="便携的本地资料，属于你自己的记录。"
      >
        <div class="settings-card">
          <p class="settings-explanation">
            数据库导出与恢复需要在桌面软件中执行。浏览器预览不读取或修改正式数据。
          </p>
        </div>
      </SettingsSection>
    </section>
    <section id="settings-updates" class="settings-chapter">
      <header class="settings-chapter-heading">
        <span class="settings-chapter-number">05</span>
        <div>
          <h2 tabindex="-1">应用更新</h2>
          <p>{{ chapters[4]!.description }}</p>
        </div>
      </header>
      <SettingsSection
        title="保持新鲜"
        description="当前版本与更新信息，在这里一目了然。"
      >
        <div class="settings-card settings-update-preview">
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
            {{
              version
                ? `当前版本 ${version}`
                : desktop
                  ? '正在读取当前版本…'
                  : '浏览器界面预览'
            }}
          </p>
          <span class="mini-badge">更新检测暂未开放</span>
          <p class="settings-explanation">
            更新服务开放后，你可以在这里查看新版本与更新说明。
          </p>
          <button class="secondary-button" disabled>检查更新</button>
        </div>
      </SettingsSection>
    </section>
  </div>
</template>
