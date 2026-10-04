<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import { RouterLink, RouterView, useRoute, useRouter } from 'vue-router';
import { selectGalleryGroup } from './services/groupNavigation';
import { NAVIGATION } from './router';
import {
  preview,
  clearPreviewTimers,
  desktop,
  local,
  notify,
  refreshLibrary,
} from './stores/library';
import LocalImport from './components/LocalImport.vue';
import { subscribeEvent } from './services/events';
const events_abort = new AbortController();
import { useMotionPolicy } from './composables/useMotionPolicy';
import { useDesktopMaterial } from './composables/useDesktopMaterial';
import { usePreviewAppearance } from './composables/usePreviewAppearance';
import {
  useSidebarLayout,
  sidebar_layout,
} from './composables/useSidebarLayout';
import {
  DEFAULT_SIDEBAR_WIDTH,
  DEFAULT_TOP_RATIO,
} from './preview/sidebarLayout';
import { useSharedTransitionHost } from './composables/useSharedTransition';
import PreviewIcon from './components/preview/PreviewIcon.vue';
import SidebarGameGroups from './components/preview/SidebarGameGroups.vue';
import PreviewFeedback from './components/preview/PreviewFeedback.vue';
import GameDragOverlay from './components/GameDragOverlay.vue';
import { gameDrag } from './services/gameDrag';
import SharedTransitionLayer from './components/preview/SharedTransitionLayer.vue';
import AccountDialog from './components/AccountDialog.vue';
import { accountDialog, openAccountDialog } from './stores/accountDialog';
import { refreshHikariAccount } from './stores/hikarinagiAccount';
import SidebarDock from './components/SidebarDock.vue';
import { refreshBangumi } from './stores/bangumi';
import {
  appSettings,
  loadAppSettings,
  saveAppSettings,
} from './stores/settings';
import { global_glass_enabled } from './composables/useDesktopMaterial';
import { startBackupPolling, stopBackupPolling } from './stores/backupTasks';
import { pollMetadataRefresh } from './stores/metadataRefresh';
import { checkStartupUpdate, updates } from './stores/updates';
const router = useRouter();
let playtimeTimer: ReturnType<typeof setInterval> | undefined;
let refreshInFlight = false;
function setPlaytimeTimer() {
  clearInterval(playtimeTimer);
  if (!desktop) return;
  playtimeTimer = setInterval(async () => {
    if (document.hidden || refreshInFlight) return;
    refreshInFlight = true;
    try {
      await refreshLibrary();
    } finally {
      refreshInFlight = false;
    }
  }, appSettings.value.ui_refresh_seconds * 1000);
}
watch(() => appSettings.value.ui_refresh_seconds, setPlaytimeTimer);
watch(
  [
    () => preview.theme,
    () => preview.palette,
    () => global_glass_enabled.value,
  ],
  ([theme, palette, glass]) => {
    if (!desktop || !appSettings.loaded) return;
    if (
      theme === appSettings.value.theme &&
      palette === appSettings.value.palette &&
      glass === appSettings.value.glass_enabled
    )
      return;
    void saveAppSettings(
      {
        ...appSettings.value,
        theme,
        palette,
        glass_enabled: glass,
      },
      ['theme', 'palette', 'glass_enabled'],
    ).catch(() => notify('外观设置保存失败。'));
  },
);

const route = useRoute();
const expanded = ref(true);
async function openLibrary() {
  selectGalleryGroup();
  await nextTick();
  window.scrollTo({ top: 0 });
}
const upper_navigation = NAVIGATION.filter(
  (item) => !['settings', 'saves'].includes(item.name),
);
const {
  panels,
  width,
  bounds,
  partition,
  dragging,
  beginResize,
  moveResize,
  endResize,
  onResizeKey,
} = useSidebarLayout();
usePreviewAppearance();
useMotionPolicy();
useDesktopMaterial();
useSharedTransitionHost(router);
async function focusSearch() {
  await router.push({ name: 'games' });
  await nextTick();
  document.querySelector<HTMLInputElement>('[data-library-search]')?.focus();
}
function onShortcut(event: KeyboardEvent) {
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k') {
    event.preventDefault();
    void focusSearch();
  }
}
function preventDesktopContextMenu(event: MouseEvent) {
  event.preventDefault();
}
onMounted(() => {
  window.addEventListener('keydown', onShortcut);
  if (desktop)
    document.addEventListener('contextmenu', preventDesktopContextMenu);
  void loadAppSettings()
    .then(async () => {
      if (
        route.path === '/' &&
        !route.query.panel &&
        appSettings.value.startup_page === 'games'
      )
        await router.replace({ name: 'games' });
      if (desktop) {
        preview.theme = appSettings.value.theme;
        preview.palette = appSettings.value.palette;
        global_glass_enabled.value = appSettings.value.glass_enabled;
        void pollMetadataRefresh();
        startBackupPolling();
        void checkStartupUpdate();
      }
      setPlaytimeTimer();
    })
    .catch((cause) =>
      notify(cause instanceof Error ? cause.message : '无法读取启动设置。'),
    );
  void refreshLibrary();
  void refreshBangumi();
  void refreshHikariAccount();
  if (desktop)
    void subscribeEvent(
      'session_ended',
      (event) => {
        void refreshLibrary();
        if (event.payload.save_backup_error)
          notify(`退出后存档备份：${event.payload.save_backup_error}`);
      },
      events_abort.signal,
    ).catch(() => {});
  if (desktop)
    void subscribeEvent(
      'backup_created',
      (event) => {
        if (
          ['before_launch', 'after_exit'].includes(
            event.payload.creation_reason,
          )
        )
          notify(
            `${event.payload.creation_reason === 'before_launch' ? '启动前' : '退出后'}存档快照已保存。`,
          );
      },
      events_abort.signal,
    ).catch(() => {});
});
onUnmounted(() => {
  clearInterval(playtimeTimer);
  stopBackupPolling();
  window.removeEventListener('keydown', onShortcut);
  if (desktop)
    document.removeEventListener('contextmenu', preventDesktopContextMenu);
  events_abort.abort();
  clearPreviewTimers();
});
</script>
<template>
  <div
    class="exhibition-app"
    :data-game-dragging="Boolean(gameDrag)"
    :class="{ 'rail-expanded': expanded, 'is-resizing': Boolean(dragging) }"
    :style="{
      '--rail-user-width': `${expanded ? width : 84}px`,
      '--sidebar-top-size': `${partition.top}px`,
    }"
  >
    <div class="app-ambience" aria-hidden="true"></div>
    <aside
      id="navigation-rail"
      class="navigation-rail"
      aria-label="展厅导航与游戏总览"
    >
      <div ref="panels" class="sidebar-panels">
        <div id="sidebar-navigation" class="rail-upper">
          <button
            class="exhibition-mark"
            type="button"
            :aria-label="expanded ? '收起游戏总览侧栏' : '展开游戏总览侧栏'"
            :title="expanded ? '收起游戏总览侧栏' : '展开游戏总览侧栏'"
            :aria-expanded="expanded"
            aria-controls="sidebar-game-overview"
            @click="
              endResize();
              expanded = !expanded;
            "
          >
            <img
              class="app-brand-icon"
              src="/brand/nekobox.png"
              alt=""
              width="40"
              height="40"
            /><span v-if="expanded">NekoBox</span>
          </button>
          <nav class="primary-navigation" aria-label="主导航">
            <RouterLink
              v-for="item in upper_navigation"
              :key="item.name"
              :to="{ name: item.name }"
              :aria-label="item.label"
              :class="{
                'is-selected':
                  (route.name === item.name &&
                    (item.name !== 'games' ||
                      !preview.gallery_state.collection_id)) ||
                  (item.name === 'games' &&
                    route.name === 'game-detail' &&
                    !preview.gallery_state.collection_id),
              }"
              @click="item.name === 'games' && openLibrary()"
            >
              <PreviewIcon :name="item.name" :size="21" /><span>{{
                item.label
              }}</span>
            </RouterLink>
          </nav>
        </div>
        <div
          class="sidebar-splitter"
          role="separator"
          tabindex="0"
          aria-label="导航与游戏列表占比"
          aria-orientation="horizontal"
          aria-controls="sidebar-navigation sidebar-game-overview"
          :aria-valuemin="
            Math.round((partition.min / partition.available) * 100)
          "
          :aria-valuemax="
            Math.round((partition.max / partition.available) * 100)
          "
          :aria-valuenow="
            Math.round((partition.top / partition.available) * 100)
          "
          :aria-valuetext="`导航占 ${Math.round((partition.top / partition.available) * 100)}%`"
          title="拖动调整上下占比；方向键调整，双击恢复"
          @pointerdown="beginResize('split', $event)"
          @pointermove="moveResize"
          @pointerup="endResize"
          @pointercancel="endResize"
          @lostpointercapture="endResize"
          @keydown="onResizeKey('split', $event)"
          @dblclick="sidebar_layout.top_ratio = DEFAULT_TOP_RATIO"
        ></div>
        <SidebarGameGroups :expanded="expanded" />
      </div>
      <div class="rail-bottom">
        <SidebarDock :expanded="expanded" @account="openAccountDialog()" />
      </div>
      <div
        v-if="expanded"
        class="sidebar-width-handle"
        role="separator"
        tabindex="0"
        aria-label="侧栏宽度"
        aria-orientation="vertical"
        aria-controls="navigation-rail main-content"
        :aria-valuemin="bounds.min"
        :aria-valuemax="bounds.max"
        :aria-valuenow="Math.round(width)"
        :aria-valuetext="`${Math.round(width)} 像素`"
        title="拖动调整侧栏宽度；方向键调整，双击恢复"
        @pointerdown="beginResize('width', $event)"
        @pointermove="moveResize"
        @pointerup="endResize"
        @pointercancel="endResize"
        @lostpointercapture="endResize"
        @keydown="onResizeKey('width', $event)"
        @dblclick="sidebar_layout.width = DEFAULT_SIDEBAR_WIDTH"
      ></div>
    </aside>
    <div class="exhibition-main">
      <main id="main-content" class="route-stage">
        <LocalImport />
        <div
          v-if="desktop && updates.notice && route.name !== 'settings'"
          class="app-update-notice"
          role="status"
        >
          <RouterLink :to="{ name: 'settings', query: { section: 'updates' } }"
            >NekoBox {{ updates.status.version }} 已发布 · 查看更新</RouterLink
          >
          <button
            class="quiet-button"
            aria-label="稍后查看更新"
            @click="updates.notice = false"
          >
            稍后
          </button>
        </div>
        <p v-if="desktop && local.error" class="page-content" role="alert">
          {{ local.error }}
          <button class="quiet-button" @click="refreshLibrary()">
            重试连接
          </button>
        </p>
        <RouterView v-slot="{ Component, route: current_route }">
          <Transition name="page">
            <component
              :is="Component"
              :key="
                current_route.name === 'game-detail'
                  ? `detail:${String(current_route.params.game_id)}`
                  : String(current_route.name)
              "
            />
          </Transition>
        </RouterView>
      </main>
    </div>
    <SharedTransitionLayer /><PreviewFeedback /><GameDragOverlay />
    <AccountDialog
      v-if="accountDialog.open"
      :initial-provider="accountDialog.provider"
      @close="accountDialog.open = false"
    />
  </div>
</template>

<style scoped>
.exhibition-app {
  --rail-width: var(--rail-user-width) !important;
}
.navigation-rail {
  padding: var(--space-24) var(--space-12) var(--space-20);
  overflow: hidden;
}
.sidebar-panels {
  display: grid;
  grid-template-rows: minmax(0, var(--sidebar-top-size)) 12px minmax(0, 1fr);
  flex: 1;
  width: 100%;
  min-height: 0;
}
.rail-upper {
  display: flex;
  flex-direction: column;
  min-height: 0;
}
.rail-upper nav {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
}
.sidebar-panels :deep(.sidebar-groups) {
  margin-top: 0;
  padding-top: var(--space-12);
  border-top: 0;
}
.rail-bottom {
  padding-top: var(--space-12);
}
.sidebar-splitter,
.sidebar-width-handle {
  position: relative;
  touch-action: none;
}
.sidebar-splitter {
  width: 100%;
  cursor: row-resize;
}
.sidebar-splitter::after {
  content: '';
  position: absolute;
  inset: 5px 0;
  background: var(--border-strong);
  border-radius: var(--radius-pill);
}
.sidebar-width-handle {
  position: absolute;
  inset: 0 0 0 auto;
  width: 8px;
  cursor: col-resize;
}
.sidebar-width-handle::after {
  content: '';
  position: absolute;
  inset: 0 0 0 auto;
  width: 2px;
  background: transparent;
}
.sidebar-splitter:hover::after,
.sidebar-splitter:focus-visible::after,
.sidebar-width-handle:hover::after,
.sidebar-width-handle:focus-visible::after {
  background: var(--accent);
}
.sidebar-width-handle:focus-visible {
  outline-offset: -3px;
}
.is-resizing,
.is-resizing :deep(*) {
  user-select: none;
}
.is-resizing .navigation-rail,
.is-resizing .exhibition-main,
.is-resizing .app-ambience {
  transition: none !important;
}
.exhibition-mark {
  flex-shrink: 0;
  height: 48px;
  margin-bottom: var(--space-24);
  justify-content: flex-start;
  padding: 0 var(--space-12);
  border: 0;
  background: transparent;
  color: var(--text);
  text-align: left;
}
.exhibition-mark:hover {
  color: var(--accent-ink, var(--accent));
}
.navigation-rail nav,
.rail-bottom {
  flex-shrink: 0;
  gap: var(--space-8);
  align-items: stretch;
}
.navigation-rail nav a,
.rail-bottom > a {
  flex-direction: row;
  justify-content: flex-start;
  width: 100%;
  min-height: 44px;
  padding: var(--space-8) var(--space-12);
  gap: var(--space-12);
  font-size: var(--type-small);
}
.exhibition-app:not(.rail-expanded) .exhibition-mark {
  justify-content: center;
  padding: 0;
}
.exhibition-app:not(.rail-expanded) .navigation-rail nav,
.exhibition-app:not(.rail-expanded) .rail-bottom {
  align-items: center;
}
.exhibition-app:not(.rail-expanded) .navigation-rail nav a,
.exhibition-app:not(.rail-expanded) .rail-bottom > a {
  justify-content: center;
  width: 56px;
  padding: var(--space-8);
}
.exhibition-app:not(.rail-expanded) .navigation-rail nav a span,
.exhibition-app:not(.rail-expanded) .rail-bottom > a span {
  display: none;
}
@media (max-width: 800px) {
  .rail-expanded .exhibition-mark span,
  .rail-expanded .navigation-rail nav a span,
  .rail-expanded .rail-bottom > a span {
    display: inline;
  }
  .rail-expanded .navigation-rail nav a,
  .rail-expanded .rail-bottom > a {
    width: auto;
    flex-direction: row;
    padding: var(--space-8) var(--space-12);
  }
}
</style>

<style scoped>
.app-update-notice {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 12px;
  margin: 14px 24px 0;
  padding: 12px 18px;
  border-radius: 16px;
  background: var(--accent-wash);
  color: var(--accent-ink);
  font-size: 13px;
}
.app-update-notice a {
  color: inherit;
}
</style>
