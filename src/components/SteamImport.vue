<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue';
import { open } from '@tauri-apps/plugin-dialog';
import PreviewIcon from './preview/PreviewIcon.vue';
import PreviewCover from './preview/PreviewCover.vue';
import {
  api,
  coverSource,
  errorText,
  local,
  notify,
  refreshLibrary,
} from '../stores/library';
import type { SteamLocalGame } from '../types/api';
import type { ImportPreparation } from '../types/importPreparation';
import type { MetadataCandidate } from '../types/domain';
interface Row extends SteamLocalGame {
  selected: boolean;
  preparation?: ImportPreparation;
  failure: string;
  localOnly: boolean;
  community?: string;
}
const dialog = ref<HTMLDialogElement>();
const rows = ref<Row[]>([]),
  warnings = ref<string[]>([]),
  error = ref(''),
  query = ref('');
const phase = ref<'scan' | 'review'>('scan'),
  busy = ref<'scan' | 'prepare' | 'commit' | ''>('');
const supplement = ref(true),
  libraryCount = ref(0),
  completed = ref(0),
  progress = ref(0);
const matching = ref<Row>(),
  matchQuery = ref(''),
  matches = ref<MetadataCandidate[]>([]),
  searching = ref(false);
let owner = 0,
  batch = '',
  path = '',
  searchOwner = 0;
const selected = computed(() =>
  rows.value.filter((r) => r.selected && !r.existing_game_id),
);
const ready = computed(
  () =>
    selected.value.length > 0 &&
    selected.value.every((r) => r.preparation || r.localOnly),
);
const visible = computed(() =>
  rows.value.filter((r) =>
    `${r.name} ${r.app_id}`
      .toLowerCase()
      .includes(query.value.trim().toLowerCase()),
  ),
);
async function clean() {
  const id = batch;
  batch = '';
  if (id) await api('cancel_import_batch', { batch_id: id });
}
async function close() {
  if (busy.value === 'commit') return;
  ++owner;
  ++searchOwner;
  local.steam_import_open = false;
  try {
    await clean();
  } catch (e) {
    notify(errorText(e));
  }
}
async function scan() {
  const token = ++owner;
  busy.value = 'scan';
  phase.value = 'scan';
  warnings.value = [];
  libraryCount.value = 0;
  error.value = '';
  matching.value = undefined;
  try {
    await clean();
    const result = await api(
      'scan_steam_games',
      path ? { steam_path: path } : {},
    );
    if (token !== owner) return;
    rows.value = result.games.map((g) => ({
      ...g,
      selected: !g.existing_game_id,
      failure: '',
      localOnly: false,
    }));
    warnings.value = result.warnings;
    libraryCount.value = result.library_count;
    completed.value = 0;
  } catch (e) {
    if (token === owner) {
      error.value = errorText(e);
      rows.value = [];
    }
  } finally {
    if (token === owner) {
      busy.value = '';
      phase.value = 'review';
    }
  }
}
async function chooseLibrary() {
  try {
    const value = await open({
      directory: true,
      multiple: false,
      title: '选择 Steam 或 SteamLibrary 文件夹',
    });
    if (typeof value === 'string' && local.steam_import_open) {
      path = value;
      await scan();
    }
  } catch (e) {
    error.value = errorText(e);
  }
}
function selectVisible(value: boolean) {
  for (const row of visible.value)
    if (!row.existing_game_id) row.selected = value;
}
async function prepare(targets = selected.value, community?: string) {
  if (busy.value) return;
  const token = owner;
  busy.value = 'prepare';
  error.value = '';
  progress.value = 0;
  try {
    if (!batch) {
      const result = await api('begin_import_batch', {});
      if (token !== owner) {
        await api('cancel_import_batch', { batch_id: result.batch_id });
        return;
      }
      batch = result.batch_id;
    }
    for (const row of targets) {
      if (token !== owner) return;
      if (row.preparation && !community) {
        progress.value++;
        continue;
      }
      row.failure = '';
      try {
        if (community && row.preparation) {
          const old = row.preparation.preparation_id;
          row.preparation = undefined;
          await api('discard_import_metadata', { preparation_ids: [old] });
        }
        const binding = community ?? row.community;
        const prepared = await api('prepare_steam_import', {
          app_id: row.app_id,
          directory: row.directory,
          batch_id: batch,
          match_hikarinagi: supplement.value,
          ...(binding ? { hikarinagi_remote_id: binding } : {}),
        });
        if (token !== owner) return;
        if (row.preparation)
          await api('discard_import_metadata', {
            preparation_ids: [row.preparation.preparation_id],
          });
        row.preparation = prepared;
        row.localOnly = false;
      } catch (e) {
        if (token !== owner) return;
        row.failure = errorText(e);
      }
      progress.value++;
    }
  } catch (e) {
    if (token === owner) error.value = errorText(e);
  } finally {
    if (token === owner) busy.value = '';
  }
}
async function importSelected() {
  if (!ready.value || busy.value) return;
  busy.value = 'commit';
  error.value = '';
  completed.value = 0;
  try {
    for (const row of [...selected.value]) {
      try {
        const result = await api('import_steam_game', {
          app_id: row.app_id,
          directory: row.directory,
          preparation_id: row.localOnly
            ? null
            : row.preparation!.preparation_id,
        });
        row.existing_game_id = result.game.id;
        row.selected = false;
        row.preparation = undefined;
        row.failure = '';
        completed.value++;
      } catch (e) {
        row.failure = errorText(e);
      }
    }
    await refreshLibrary({ reloadDetails: true });
    notify(`已导入 ${completed.value} 款 Steam 游戏`);
  } catch (e) {
    error.value = errorText(e);
  } finally {
    busy.value = '';
  }
}
function openMatching(row: Row) {
  matching.value = row;
  matchQuery.value = row.preparation?.title ?? row.name;
  matches.value = [];
  error.value = '';
}
async function searchCommunity() {
  if (!matchQuery.value.trim()) return;
  const token = ++searchOwner;
  searching.value = true;
  error.value = '';
  try {
    const result = await api('search_metadata', {
      query: matchQuery.value.trim(),
      providers: ['hikarinagi'],
      manual: true,
      cache: false,
    });
    if (token === searchOwner && local.steam_import_open)
      matches.value = result;
  } catch (e) {
    if (token === searchOwner) error.value = errorText(e);
  } finally {
    if (token === searchOwner) searching.value = false;
  }
}
async function chooseCommunity(candidate: MetadataCandidate) {
  const row = matching.value;
  if (!row) return;
  matching.value = undefined;
  row.community = candidate.remote_id;
  await prepare([row], candidate.remote_id);
}
watch(supplement, async () => {
  const ids = rows.value.flatMap((r) =>
    r.preparation ? [r.preparation.preparation_id] : [],
  );
  for (const row of rows.value) {
    row.preparation = undefined;
    row.failure = '';
  }
  if (ids.length)
    try {
      await api('discard_import_metadata', { preparation_ids: ids });
    } catch (e) {
      error.value = errorText(e);
    }
});
watch(
  () => local.steam_import_open,
  async (value) => {
    if (value) {
      query.value = '';
      path = '';
      searching.value = false;
      await nextTick();
      dialog.value?.showModal();
      await scan();
    } else if (dialog.value?.open) dialog.value.close();
  },
);
onBeforeUnmount(() => {
  ++owner;
  ++searchOwner;
  void clean().catch(() => {});
});
</script>
<template>
  <Teleport to="body">
    <dialog
      ref="dialog"
      class="ui-dialog steam-import"
      aria-labelledby="steam-import-title"
      @cancel.prevent="close"
      @close="local.steam_import_open && close()"
    >
      <header class="steam-head">
        <div class="steam-brand">
          <span class="steam-emblem"
            ><PreviewIcon name="steam" :size="26"
          /></span>
          <div>
            <p class="eyebrow">YOUR STEAM LIBRARY</p>
            <h2 id="steam-import-title">从 Steam 导入</h2>
          </div>
        </div>
        <button
          class="import-close"
          aria-label="关闭 Steam 导入"
          :disabled="busy === 'commit'"
          @click="close"
        >
          <PreviewIcon name="close" />
        </button>
      </header>
      <p class="steam-lead">把 Steam 中的喜欢，带进你的游戏库。</p>
      <div class="steam-steps" aria-label="导入步骤">
        <span class="active">01 选择作品</span><i></i
        ><span :class="{ active: rows.some((r) => r.preparation) }"
          >02 准备资料</span
        ><i></i><span :class="{ active: completed > 0 }">03 确认入库</span>
      </div>
      <div class="steam-tools">
        <button class="quiet-button" :disabled="!!busy" @click="chooseLibrary">
          <PreviewIcon name="folder" :size="16" />选择其他库</button
        ><button class="quiet-button" :disabled="!!busy" @click="scan">
          重新扫描</button
        ><span v-if="libraryCount"
          >{{ libraryCount }} 个库 · {{ rows.length }} 款已安装</span
        >
      </div>
      <Transition name="steam-stage" mode="out-in">
        <div v-if="phase === 'scan'" class="steam-empty" role="status">
          <span class="steam-orbit"></span
          ><strong>正在发现你的 Steam 游戏</strong
          ><small>读取本机安装清单…</small>
        </div>
        <div v-else-if="!rows.length" class="steam-empty">
          <PreviewIcon name="games" :size="36" /><strong>{{
            error ? '暂时无法读取 Steam 库' : '尚未发现已安装游戏'
          }}</strong
          ><small>选择 Steam 安装目录或 SteamLibrary 文件夹后重试。</small>
        </div>
        <section v-else class="steam-review">
          <div class="steam-selection">
            <input
              v-model="query"
              type="search"
              placeholder="搜索作品或 AppID"
              aria-label="搜索 Steam 游戏"
              :disabled="!!busy"
            /><button
              class="quiet-button"
              :disabled="!!busy"
              @click="selectVisible(true)"
            >
              全选</button
            ><button
              class="quiet-button"
              :disabled="!!busy"
              @click="selectVisible(false)"
            >
              清空
            </button>
          </div>
          <div class="steam-list" aria-label="Steam 游戏列表">
            <article
              v-for="row in visible"
              :key="row.app_id"
              class="steam-row"
              :class="{
                selected: row.selected,
                imported: row.existing_game_id,
              }"
            >
              <label class="steam-row-choice"
                ><input
                  v-model="row.selected"
                  type="checkbox"
                  :disabled="!!busy || !!row.existing_game_id"
                  :aria-label="`选择 ${row.name}`"
                /><PreviewCover
                  v-if="row.preparation"
                  class="steam-cover"
                  :cover_url="coverSource(row.preparation.cover_path)"
                  :title="row.preparation.title"
                /><span v-else class="steam-cover steam-placeholder"
                  ><PreviewIcon
                    :name="row.existing_game_id ? 'check' : 'games'"
                    :size="22" /></span
                ><span class="steam-copy"
                  ><strong>{{ row.preparation?.title ?? row.name }}</strong
                  ><small :title="row.directory"
                    >AppID {{ row.app_id }} · {{ row.directory }}</small
                  ><span class="steam-status">{{
                    row.existing_game_id
                      ? '已在游戏库'
                      : row.localOnly
                        ? '仅导入安装信息'
                        : row.preparation
                          ? '资料与封面已准备'
                          : '等待准备资料'
                  }}</span></span
                ></label
              >
              <p
                v-if="row.preparation?.supplementation_message"
                class="steam-note"
              >
                {{ row.preparation.supplementation_message }}
              </p>
              <p v-if="row.failure" class="steam-failure" role="alert">
                {{ row.failure }}
              </p>
              <div
                v-if="!row.existing_game_id && row.selected"
                class="steam-row-actions"
              >
                <button
                  v-if="row.preparation && supplement"
                  class="quiet-button"
                  :disabled="!!busy"
                  @click="openMatching(row)"
                >
                  选择安利墙关联</button
                ><button
                  v-if="row.failure"
                  class="quiet-button"
                  :disabled="!!busy"
                  @click="prepare([row])"
                >
                  重试准备</button
                ><label v-if="row.failure && !row.preparation"
                  ><input
                    v-model="row.localOnly"
                    type="checkbox"
                    :disabled="!!busy"
                  />仅导入安装信息，稍后刮削</label
                >
              </div>
            </article>
            <p v-if="!visible.length" class="steam-no-results">
              没有符合搜索的作品。
            </p>
          </div>
          <label class="steam-supplement"
            ><input
              v-model="supplement"
              type="checkbox"
              :disabled="!!busy"
            /><span
              >自动关联 Hikarinagi 安利墙<small
                >作品资料、简介与封面以 Steam
                为准。只自动关联唯一同名作品。</small
              ></span
            ></label
          >
        </section>
      </Transition>
      <section
        v-if="matching"
        class="steam-matching"
        aria-label="选择安利墙作品"
      >
        <header>
          <strong>关联安利墙 · {{ matching.name }}</strong
          ><button
            class="import-close"
            aria-label="关闭关联选择"
            @click="
              matching = undefined;
              ++searchOwner;
              searching = false;
            "
          >
            <PreviewIcon name="close" :size="16" />
          </button>
        </header>
        <form @submit.prevent="searchCommunity">
          <input
            v-model="matchQuery"
            aria-label="安利墙作品名称"
            placeholder="输入作品名称"
          /><button class="secondary-button" :disabled="searching">
            {{ searching ? '搜索中…' : '搜索' }}
          </button>
        </form>
        <div class="steam-matches">
          <button
            v-for="candidate in matches"
            :key="candidate.remote_id"
            class="quiet-button"
            @click="chooseCommunity(candidate)"
          >
            <strong>{{ candidate.title }}</strong
            ><small>{{ candidate.subtitle }} · #{{ candidate.remote_id }}</small
            ><PreviewIcon name="arrow" :size="16" />
          </button>
          <p v-if="!searching && !matches.length" class="steam-note">
            搜索后选择对应作品。此关联仅用于安利墙。
          </p>
        </div>
      </section>
      <details v-if="warnings.length" class="steam-warnings">
        <summary>{{ warnings.length }} 条扫描提示</summary>
        <p v-for="warning in warnings" :key="warning">{{ warning }}</p>
      </details>
      <p v-if="error" class="steam-failure" role="alert">{{ error }}</p>
      <footer class="steam-footer">
        <div aria-live="polite">
          <strong>{{
            completed ? `已导入 ${completed} 款` : `已选 ${selected.length} 款`
          }}</strong
          ><small>{{
            busy === 'prepare'
              ? `正在准备 ${progress} / ${selected.length}`
              : busy === 'commit'
                ? '正在保存到游戏库…'
                : '确认入库前，游戏库不会新增条目。'
          }}</small>
        </div>
        <div class="steam-footer-buttons">
          <button
            class="secondary-button"
            :disabled="!!busy || !selected.length"
            @click="prepare()"
          >
            {{ busy === 'prepare' ? '准备中…' : '准备资料' }}</button
          ><button
            class="primary-button"
            :disabled="!!busy || !ready"
            @click="importSelected"
          >
            {{ busy === 'commit' ? '导入中…' : '确认导入' }}
          </button>
        </div>
      </footer>
    </dialog>
  </Teleport>
</template>
<style scoped>
.steam-import {
  width: min(850px, calc(100vw - 32px));
}
.steam-import[open] {
  display: flex;
  flex-direction: column;
  overflow: auto;
}
.steam-head,
.steam-lead,
.steam-steps,
.steam-tools,
.steam-footer,
.steam-supplement {
  flex-shrink: 0;
}
.steam-review {
  display: flex;
  flex-direction: column;
  min-height: 0;
  flex: 1;
}
.steam-review .steam-list {
  min-height: 80px;
  flex: 1;
}
.steam-selection {
  flex-shrink: 0;
}
.steam-footer {
  position: sticky;
  bottom: 0;
  z-index: 2;
  background: transparent;
}
@media (max-height: 650px) {
  .steam-lead {
    display: none;
  }
  .steam-steps {
    padding: 8px 0;
  }
  .steam-tools {
    padding-bottom: 8px;
  }
  .steam-supplement small {
    display: none;
  }
  .steam-footer {
    margin-top: 10px;
    padding-top: 10px;
  }
}
.steam-head,
.steam-brand,
.steam-tools,
.steam-selection,
.steam-footer,
.steam-footer-buttons,
.steam-steps {
  display: flex;
  align-items: center;
  gap: 12px;
}
.steam-head,
.steam-footer {
  justify-content: space-between;
}
.steam-head h2 {
  margin: 0;
  font-family: var(--font-display);
}
.steam-emblem {
  display: grid;
  place-items: center;
  width: 52px;
  height: 52px;
  border-radius: 18px;
  color: var(--accent);
  background: var(--accent-wash);
  box-shadow: 0 0 0 7px color-mix(in srgb, var(--accent) 5%, transparent);
}
.steam-lead {
  margin: 18px 0;
  color: var(--muted);
}
.steam-steps {
  font-size: 11px;
  color: var(--muted);
  padding: 12px 0 20px;
}
.steam-steps .active {
  color: var(--accent);
}
.steam-steps i {
  width: 24px;
  height: 1px;
  background: var(--border);
}
.steam-tools {
  flex-wrap: wrap;
  padding-bottom: 16px;
}
.steam-tools > span {
  margin-left: auto;
  color: var(--muted);
  font-size: 12px;
}
.steam-selection {
  margin-bottom: 12px;
}
.steam-selection input,
.steam-matching input {
  min-width: 0;
  flex: 1;
  padding: 10px 14px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--surface-glass);
  color: var(--text);
}
.steam-list {
  display: grid;
  gap: 10px;
  max-height: min(43vh, 440px);
  overflow: auto;
  padding: 2px 4px 6px;
  overscroll-behavior: contain;
}
.steam-row {
  border: 1px solid var(--border);
  border-radius: 16px;
  padding: 12px;
  background: var(--surface-glass);
  transition:
    background 240ms ease,
    border-color 240ms ease;
}
.steam-row.selected {
  border-color: color-mix(in srgb, var(--accent) 46%, var(--border));
  background: color-mix(in srgb, var(--accent) 5%, var(--surface-glass));
}
.steam-row.imported {
  opacity: 0.65;
}
.steam-row-choice {
  display: grid;
  grid-template-columns: 20px 46px minmax(0, 1fr);
  gap: 14px;
  align-items: center;
  cursor: pointer;
}
input[type='checkbox'] {
  accent-color: var(--accent);
  width: 16px;
  height: 16px;
}
.steam-cover {
  width: 46px;
  height: 62px;
  border-radius: 9px;
  overflow: hidden;
}
.steam-placeholder {
  display: grid;
  place-items: center;
  color: var(--accent);
  background: var(--accent-wash);
}
.steam-copy {
  min-width: 0;
}
.steam-copy strong,
.steam-copy small {
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.steam-copy strong {
  font-size: 15px;
}
.steam-copy small {
  font-size: 11px;
  color: var(--muted);
  margin: 5px 0;
}
.steam-status {
  font-size: 10px;
  color: var(--accent);
}
.steam-note {
  color: var(--muted);
  font-size: 11px;
  line-height: 1.65;
  margin: 8px 0 0 80px;
}
.steam-failure {
  color: var(--danger);
  font-size: 12px;
  line-height: 1.7;
  overflow-wrap: anywhere;
}
.steam-row-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
  margin-left: 80px;
  font-size: 11px;
  color: var(--muted);
}
.steam-row-actions .quiet-button {
  padding: 5px 0;
  font-size: 11px;
}
.steam-supplement {
  display: flex;
  gap: 10px;
  margin-top: 16px;
  font-size: 12px;
}
.steam-supplement small {
  display: block;
  color: var(--muted);
  font-size: 11px;
  margin-top: 5px;
  line-height: 1.6;
}
.steam-empty {
  display: grid;
  justify-items: center;
  gap: 12px;
  padding: 50px 16px;
  color: var(--muted);
  text-align: center;
}
.steam-empty strong {
  color: var(--text);
  font-family: var(--font-display);
  font-size: 20px;
}
.steam-empty small {
  font-size: 12px;
}
.steam-orbit {
  width: 36px;
  height: 36px;
  border: 2px solid var(--accent-wash);
  border-top-color: var(--accent);
  border-radius: 50%;
  animation: orbit 1.1s linear infinite;
}
.steam-no-results {
  padding: 24px;
  text-align: center;
  color: var(--muted);
}
.steam-footer {
  border-top: 1px solid var(--border);
  margin-top: 18px;
  padding-top: 18px;
}
.steam-footer strong,
.steam-footer small {
  display: block;
}
.steam-footer strong {
  font-size: 13px;
}
.steam-footer small {
  font-size: 11px;
  color: var(--muted);
  margin-top: 5px;
}
.steam-warnings {
  color: var(--muted);
  font-size: 11px;
  margin-top: 12px;
  overflow-wrap: anywhere;
}
.steam-warnings p {
  line-height: 1.6;
}
.steam-matching {
  margin-top: 14px;
  border: 1px solid var(--border);
  padding: 14px;
  border-radius: 14px;
  background: var(--accent-wash);
}
.steam-matching header,
.steam-matching form {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  margin-bottom: 10px;
}
.steam-matches {
  max-height: 170px;
  overflow: auto;
}
.steam-matches button {
  display: flex;
  gap: 10px;
  text-align: left;
  width: 100%;
}
.steam-matches small {
  color: var(--muted);
}
.steam-matching .steam-note {
  margin-left: 0;
}
.steam-stage-enter-active,
.steam-stage-leave-active {
  transition:
    opacity 220ms ease,
    transform 280ms cubic-bezier(0.2, 0.8, 0.2, 1);
}
.steam-stage-enter-from {
  opacity: 0;
  transform: translateY(8px);
}
.steam-stage-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}
@keyframes orbit {
  to {
    transform: rotate(360deg);
  }
}
@media (max-width: 600px) {
  .steam-import {
    padding: 20px;
  }
  .steam-footer {
    align-items: stretch;
    flex-direction: column;
  }
  .steam-footer-buttons button {
    flex: 1;
  }
  .steam-tools > span {
    width: 100%;
    margin: 0;
  }
  .steam-row-choice {
    gap: 9px;
  }
  .steam-note,
  .steam-row-actions {
    margin-left: 0;
  }
  .steam-steps {
    gap: 8px;
  }
  .steam-steps i {
    width: 12px;
  }
  .steam-matches button {
    flex-wrap: wrap;
  }
}
@media (prefers-reduced-motion: reduce) {
  .steam-stage-enter-active,
  .steam-stage-leave-active,
  .steam-row {
    transition: none;
  }
  .steam-orbit {
    animation: none;
  }
}
:global([data-motion='reduced']) .steam-orbit {
  animation: none;
}
:global([data-motion='reduced']) .steam-stage-enter-active,
:global([data-motion='reduced']) .steam-stage-leave-active {
  transition: none;
}
</style>
