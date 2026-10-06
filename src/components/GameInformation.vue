<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue';
import {
  api,
  applyGameDetail,
  desktop,
  errorText,
  local,
  notify,
  preview,
} from '../stores/library';
import {
  changedMetadata,
  mergeMetadataDraft,
  metadataValues,
} from '../services/detailWorkspace';
import type { GameDetail } from '../types/domain';
import PreviewCover from './preview/PreviewCover.vue';
import PreviewIcon from './preview/PreviewIcon.vue';
import MetadataMatch from './MetadataMatch.vue';
import ScreenshotScanPanel from './ScreenshotScanPanel.vue';
import SourcesPanel from './SourcesPanel.vue';
import SettingsSwitch from './settings/SettingsSwitch.vue';
const props = defineProps<{ gameId: string }>();
const shown = computed(() =>
  preview.games.find((g) => g.game_id === props.gameId),
);
const game = computed(() =>
  desktop
    ? (local.records[props.gameId] as GameDetail | undefined)
    : ({
        ...shown.value,
        id: props.gameId,
        metadata: [],
        metadata_locked: false,
      } as Partial<GameDetail>),
);
const draft = reactive(metadataValues(undefined));
const baseline = reactive(metadataValues(undefined));
const locked = computed(() => !!game.value?.metadata_locked);
const saving = ref(false),
  locking = ref(false),
  error = ref('');
const difference = computed(() => changedMetadata(draft, baseline));
const dirty = computed(() => Object.keys(difference.value.changes).length > 0);
let currentId = '';
watch(
  game,
  (value) => {
    const next = metadataValues(value);
    if (currentId !== props.gameId) {
      currentId = props.gameId;
      Object.assign(draft, next);
      Object.assign(baseline, next);
      error.value = '';
    } else mergeMetadataDraft(draft, baseline, next);
  },
  { immediate: true },
);
const bindings = computed(() => {
  const unique = new Map<
    string,
    { provider: string; remote: string; fetched: string | null | undefined }
  >();
  for (const field of game.value?.metadata ?? []) {
    if (!field.remote_id) continue;
    const key = `${field.provider}:${field.remote_id}`;
    const before = unique.get(key);
    if (!before || (field.fetched_at ?? '') > (before.fetched ?? ''))
      unique.set(key, {
        provider: field.provider,
        remote: field.remote_id,
        fetched: field.fetched_at,
      });
  }
  return [...unique.values()];
});
function provenance(field: string) {
  const source = game.value?.metadata?.find(
    (f) =>
      f.field === field ||
      (field === 'description' && f.field === 'description_zh'),
  );
  return source
    ? source.manually_edited
      ? '手动设定'
      : source.provider === 'hikarinagi'
        ? 'Hikarinagi'
        : source.provider === 'hikarifield'
          ? 'HIKARI FIELD'
          : source.provider === 'bangumi'
            ? 'Bangumi'
            : source.provider.toUpperCase()
    : '待补充';
}
async function save() {
  if (!desktop || locked.value || saving.value || !dirty.value) return;
  const id = props.gameId;
  saving.value = true;
  error.value = '';
  try {
    const result = await api('update_game_metadata', {
      game_id: id,
      ...difference.value,
    });
    applyGameDetail(result);
    if (props.gameId !== id) return;
    Object.assign(draft, metadataValues(result));
    Object.assign(baseline, metadataValues(result));
    notify('资料已保存，作品名称、封面与简介已同步更新。');
  } catch (e) {
    if (props.gameId === id) error.value = errorText(e);
  } finally {
    saving.value = false;
  }
}
async function setLock(value: boolean) {
  if (!desktop || locking.value) return;
  const id = props.gameId;
  locking.value = true;
  error.value = '';
  try {
    applyGameDetail(
      await api('set_metadata_lock', { game_id: id, locked: value }),
    );
    notify(value ? '元数据已锁定。' : '元数据已解锁。');
  } catch (e) {
    if (props.gameId === id) error.value = errorText(e);
  } finally {
    locking.value = false;
  }
}
async function chooseCover() {
  if (!desktop || locked.value) return;
  const id = props.gameId;
  try {
    const { open } = await import('@tauri-apps/plugin-dialog');
    const file = await open({
      multiple: false,
      filters: [
        { name: '封面图片', extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif'] },
      ],
    });
    if (typeof file === 'string' && id === props.gameId && !locked.value)
      draft.cover_path = file;
  } catch (e) {
    error.value = errorText(e);
  }
}
function reload() {
  Object.assign(draft, metadataValues(game.value));
  Object.assign(baseline, metadataValues(game.value));
  error.value = '';
}
</script>
<template>
  <div class="detail-workspace information-workspace">
    <header class="workspace-heading">
      <div>
        <p class="eyebrow">THE STORY, IN YOUR WORDS</p>
        <h2>资料</h2>
        <p>为这个故事，留下准确的名字与来处。</p>
      </div>
      <span class="workspace-status" :class="{ 'is-locked': locked }"
        ><PreviewIcon :name="locked ? 'lock' : 'check'" :size="15" />{{
          locked ? '元数据已锁定' : '可编辑资料'
        }}</span
      >
    </header>
    <p v-if="!desktop" class="workspace-notice">
      离线原型资料 · 以下内容来自演示内存，编辑、刮削与扫描在桌面版可用。
    </p>
    <fieldset
      class="workspace-editable"
      :disabled="locked || saving || !desktop"
    >
      <section class="workspace-card" data-info-section="name">
        <header class="workspace-card-heading">
          <span class="workspace-number">01</span>
          <div>
            <h3>名称设定</h3>
            <p>用于作品展廊、侧栏与详情标题。</p>
          </div>
        </header>
        <label class="workspace-field"
          ><span
            >作品名称 <small>{{ provenance('title') }}</small></span
          ><input
            v-model="draft.title"
            maxlength="200"
            placeholder="为作品设定名称"
        /></label>
      </section>
      <section class="workspace-card" data-info-section="cover">
        <header class="workspace-card-heading">
          <span class="workspace-number">02</span>
          <div>
            <h3>封面路径</h3>
            <p>选择本地图片，保存后自动缓存到软件数据目录。</p>
          </div>
        </header>
        <div class="workspace-cover-row">
          <PreviewCover
            class="workspace-cover"
            :title="shown?.title || draft.title"
            :cover_url="shown?.cover_url || ''"
          />
          <div>
            <label class="workspace-field"
              ><span
                >图片路径 <small>{{ provenance('cover_path') }}</small></span
              ><input
                v-model="draft.cover_path"
                placeholder="选择图片，或输入完整图片路径"
                maxlength="4096" /></label
            ><button
              class="secondary-button"
              type="button"
              @click="chooseCover"
            >
              <PreviewIcon name="folder" :size="16" />选择图片
            </button>
          </div>
        </div>
      </section>
      <section class="workspace-card" data-info-section="binding">
        <header class="workspace-card-heading">
          <span class="workspace-number">03</span>
          <div>
            <h3>刮削绑定</h3>
            <p>手动搜索并确认作品，重新获取资料与封面。</p>
          </div>
        </header>
        <MetadataMatch
          v-if="desktop"
          :game-id="gameId"
          :title="game?.title || ''"
          :metadata-status="game?.metadata_status"
          :locked="locked"
          compact
        />
        <button v-else class="secondary-button" disabled>
          重新刮削（手动）
        </button>
      </section>
      <section class="workspace-card" data-info-section="metadata">
        <header class="workspace-card-heading">
          <span class="workspace-number">04</span>
          <div>
            <h3>作品资料</h3>
            <p>刮削结果自动填入；手动修改的字段会保留你的设定。</p>
          </div>
        </header>
        <div class="workspace-fields workspace-fields--three">
          <label class="workspace-field"
            ><span
              >开发商 <small>{{ provenance('developer') }}</small></span
            ><input
              v-model="draft.developer"
              maxlength="1024"
              placeholder="开发商名称"
          /></label>
          <label class="workspace-field"
            ><span
              >评分
              <small>{{ provenance('source_rating') }} · / 10</small></span
            ><input
              :value="draft.source_rating"
              type="number"
              min="0"
              max="10"
              step="0.1"
              placeholder="尚无评分"
              @input="
                draft.source_rating =
                  ($event.target as HTMLInputElement).value === ''
                    ? null
                    : Number(($event.target as HTMLInputElement).value)
              "
          /></label>
          <label class="workspace-field"
            ><span
              >发行日期 <small>{{ provenance('release_date') }}</small></span
            ><input v-model="draft.release_date" type="date"
          /></label>
        </div>
        <label class="workspace-field"
          ><span
            >简介编辑 <small>{{ provenance('description') }}</small></span
          ><textarea
            v-model="draft.description"
            rows="8"
            maxlength="100000"
            placeholder="写下这个故事的简介…"
          />
        </label>
      </section>
    </fieldset>
    <section class="workspace-card" data-info-section="sources">
      <header class="workspace-card-heading">
        <span class="workspace-number">05</span>
        <div>
          <h3>数据来源</h3>
          <p>当前作品的刮削来源与识别 ID，保留每一份资料的来处。</p>
        </div>
      </header>
      <div v-if="bindings.length" class="workspace-source-grid">
        <article
          v-for="item in bindings"
          :key="`${item.provider}:${item.remote}`"
          class="workspace-source"
        >
          <span class="workspace-source-mark">{{
            item.provider === 'hikarinagi'
              ? 'H'
              : item.provider === 'bangumi'
                ? 'B'
                : 'V'
          }}</span>
          <div>
            <strong>{{
              item.provider === 'hikarinagi'
                ? 'Hikarinagi'
                : item.provider === 'bangumi'
                  ? 'Bangumi'
                  : item.provider.toUpperCase()
            }}</strong
            ><code>ID · {{ item.remote }}</code
            ><small>{{
              item.fetched
                ? new Date(item.fetched).toLocaleString()
                : '暂无抓取时间'
            }}</small>
          </div>
        </article>
      </div>
      <p v-else class="workspace-empty">
        {{
          desktop
            ? '尚未绑定刮削来源，手动选择作品后会在这里显示识别 ID。'
            : '演示 fixture · 未识别远端作品'
        }}
      </p>
      <details v-if="desktop" class="workspace-extra">
        <summary>官方页面与手动链接</summary>
        <SourcesPanel :game-id="gameId" :locked="locked" />
      </details>
    </section>
    <section
      class="workspace-card workspace-lock-card"
      data-info-section="lock"
    >
      <header class="workspace-card-heading">
        <span class="workspace-number">06</span>
        <div>
          <h3>锁定元数据</h3>
          <p>锁定后不再参与任何自动或手动刮削，资料编辑框也会锁定。</p>
        </div>
      </header>
      <SettingsSwitch
        :model-value="locked"
        :disabled="!desktop || locking || saving"
        label="锁定元数据"
        :description="
          locked
            ? '当前资料已冻结，解锁后可继续编辑。'
            : '保留当前的名字、封面与作品资料。'
        "
        @update:model-value="setLock"
      />
    </section>
    <section
      class="workspace-card workspace-scan-card"
      data-info-section="screenshots"
    >
      <header class="workspace-card-heading">
        <span class="workspace-number">07</span>
        <div>
          <h3>截图扫描</h3>
          <p>索引安装目录中的图片，结果在“截图”栏目查看。</p>
        </div>
      </header>
      <ScreenshotScanPanel v-if="desktop" :game-id="gameId" compact />
      <p v-else class="workspace-empty">浏览器原型不扫描真实安装目录。</p>
    </section>
    <div
      v-if="desktop"
      class="workspace-save-bar"
      role="region"
      aria-label="资料保存"
    >
      <div>
        <p v-if="error" class="workspace-error" role="alert">{{ error }}</p>
        <p v-else>
          {{
            locking
              ? '正在更新锁定状态…'
              : dirty
                ? locked
                  ? '修改尚未保存；解锁后可继续。'
                  : '有未保存的修改'
                : locked
                  ? '当前资料已锁定'
                  : '资料已与作品同步'
          }}
        </p>
      </div>
      <button class="quiet-button" :disabled="!dirty || saving" @click="reload">
        重新载入</button
      ><button
        class="primary-button"
        :disabled="locked || saving || !dirty"
        @click="save"
      >
        <PreviewIcon name="check" :size="16" />{{
          saving ? '保存中…' : '保存资料'
        }}
      </button>
    </div>
  </div>
</template>
