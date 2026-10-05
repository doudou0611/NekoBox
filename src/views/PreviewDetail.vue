<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { useRoute } from 'vue-router';
import { returnFromPreviewDetail } from '../composables/useSharedTransition';
import { useCoverAtmosphere } from '../composables/useCoverAtmosphere';
import {
  preview,
  toggleFavorite,
  showDemoAction,
  notify,
  desktop,
  loadDetail,
  launchGame,
} from '../stores/library';
import LocalInstallation from '../components/LocalInstallation.vue';
import {
  hikariField,
  gameDownload,
  startHikariDownload,
} from '../stores/hikariField';
import PlaytimePanel from '../components/PlaytimePanel.vue';
import SavePanel from '../components/SavePanel.vue';
import GameInformation from '../components/GameInformation.vue';
import ScreenshotsPanel from '../components/ScreenshotsPanel.vue';
import HikarinagiRatesWall from '../components/HikarinagiRatesWall.vue';
import { formatPlaytime } from '../preview/data';
import PreviewCover from '../components/preview/PreviewCover.vue';
import PreviewIcon from '../components/preview/PreviewIcon.vue';
import PreviewTooltip from '../components/preview/PreviewTooltip.vue';
import GameStatusSelect from '../components/GameStatusSelect.vue';
const route = useRoute();
const game = computed(() =>
  preview.games.find((item) => item.game_id === route.params.game_id),
);
const tab = ref('overview');
const remoteOnly = computed(
  () => Boolean(game.value?.hikari_field) && !game.value?.launchable,
);
const download = computed(() => gameDownload(game.value?.game_id || ''));
const startingDownload = computed(() =>
  hikariField.starting.includes(game.value?.game_id || ''),
);
const { atmosphere } = useCoverAtmosphere(
  () => (preview.missing_cover ? undefined : game.value?.cover_url),
  () => preview.theme,
);
const tabs = [
  { key: 'overview', label: '概览' },
  { key: 'activity', label: '游玩记录' },
  { key: 'saves', label: '存档' },
  { key: 'screenshots', label: '截图' },
  { key: 'information', label: '资料' },
  { key: 'launch', label: '启动' },
];
watch(
  () => [route.params.game_id, route.query.panel],
  () => {
    tab.value =
      route.query.panel === 'information' || route.query.panel === 'launch'
        ? route.query.panel
        : 'overview';
    if (desktop) void loadDetail(String(route.params.game_id ?? ''));
  },
  { immediate: true },
);
function moveTab(event: KeyboardEvent) {
  if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
  event.preventDefault();
  const index = tabs.findIndex((item) => item.key === tab.value);
  const next =
    event.key === 'Home'
      ? 0
      : event.key === 'End'
        ? tabs.length - 1
        : (index + (event.key === 'ArrowRight' ? 1 : -1) + tabs.length) %
          tabs.length;
  tab.value = tabs[next]!.key;
  document
    .querySelector<HTMLElement>(`[data-detail-tab="${tab.value}"]`)
    ?.focus();
}
</script>
<template>
  <div class="detail-page page-content" data-detail-stage :style="atmosphere">
    <div class="detail-navigation">
      <button
        class="back-button detail-back-button"
        type="button"
        aria-label="返回原展廊位置"
        @click="returnFromPreviewDetail"
      >
        <PreviewIcon name="back" :size="18" />返回故事来处<span>Esc</span>
      </button>
    </div>
    <template v-if="game">
      <div class="detail-atmosphere" aria-hidden="true">
        <img
          v-if="game.cover_url && !preview.missing_cover"
          class="detail-scenery"
          :src="game.cover_url"
          alt=""
        />
      </div>
      <section class="detail-hero">
        <div class="detail-cover-frame">
          <PreviewCover
            class="detail-cover"
            :data-shared-cover="game.game_id"
            :cover_url="game.cover_url"
            :title="game.title"
          />
        </div>
        <div class="detail-copy">
          <h1
            tabindex="-1"
            data-detail-heading
            :data-shared-title="game.game_id"
          >
            {{ game.title }}
          </h1>
          <p
            v-if="game.subtitle"
            class="detail-subtitle"
            :lang="desktop ? 'ja' : undefined"
          >
            {{ game.subtitle }}
          </p>
          <div class="detail-meta" aria-label="作品资料">
            <span
              ><small>评分</small
              ><strong v-if="game.source_rating != null"
                >{{ game.source_rating.toFixed(1) }} / 10</strong
              ><strong v-else>暂无评分</strong></span
            >
            <span
              ><small>开发商</small
              ><strong>{{ game.developer || '开发商待补充' }}</strong></span
            >
            <span
              ><small>发行日期</small
              ><strong>{{ game.release_date || '日期待补充' }}</strong></span
            >
          </div>
          <div class="detail-tags" aria-label="作品标签">
            <span
              v-for="tag in desktop ? game.source_tags || [] : game.tags"
              :key="tag"
              >{{ tag }}</span
            >
            <span
              v-if="!(desktop ? game.source_tags?.length : game.tags.length)"
              >暂无标签</span
            >
          </div>
          <p class="detail-intro">
            {{
              remoteOnly
                ? '已拥有此游戏 · HIKARI FIELD'
                : game.description.split('\n')[0]
            }}
          </p>
          <div class="detail-actions">
            <button
              class="primary-button"
              :disabled="
                remoteOnly &&
                Boolean(
                  download || startingDownload || !game.hikari_field?.released,
                )
              "
              @click="
                remoteOnly
                  ? startHikariDownload(game.game_id)
                  : desktop
                    ? launchGame(game.game_id)
                    : showDemoAction('启动游戏')
              "
            >
              <PreviewIcon
                :name="remoteOnly ? 'download' : 'play'"
                :size="18"
              />{{
                remoteOnly
                  ? download
                    ? '正在下载…'
                    : startingDownload
                      ? '正在准备…'
                      : !game.hikari_field?.released
                        ? '暂未开放下载'
                        : '下载游戏'
                  : desktop
                    ? '启动游戏'
                    : '启动故事 · 演示'
              }}</button
            ><PreviewTooltip
              :text="game.favorite ? '取消演示收藏' : '加入演示收藏'"
            >
              <button
                class="secondary-button favorite-detail"
                :class="{ 'is-favorite': game.favorite }"
                :aria-label="`${game.favorite ? '取消收藏' : '收藏'}${game.title}`"
                :aria-pressed="game.favorite"
                :data-favorite="game.game_id"
                @click="toggleFavorite(game.game_id)"
              >
                <PreviewIcon name="heart" :size="18" />{{
                  game.favorite ? '已收藏' : '收藏'
                }}
              </button>
            </PreviewTooltip>
            <GameStatusSelect
              :key="game.game_id"
              :game-id="game.game_id"
              :status="game.status"
            />
            <div class="detail-playtime">
              <small>{{ desktop ? '故事记录时长' : '演示游玩时长' }}</small>
              <strong>{{ formatPlaytime(game.duration_minutes) }}</strong>
            </div>
          </div>
        </div>
      </section>
      <nav
        class="detail-tabs"
        role="tablist"
        aria-label="作品详情内容"
        @keydown="moveTab"
      >
        <button
          v-for="item in tabs"
          :id="`tab-${item.key}`"
          :key="item.key"
          role="tab"
          :data-detail-tab="item.key"
          :aria-selected="tab === item.key"
          :tabindex="tab === item.key ? 0 : -1"
          :aria-controls="`panel-${item.key}`"
          :class="{ active: tab === item.key }"
          @click="tab = item.key"
        >
          {{ item.label }}
        </button>
      </nav>
      <Transition name="tab" mode="out-in">
        <section
          :id="`panel-${tab}`"
          :key="tab"
          class="detail-panel"
          :class="{
            'detail-panel--workspace':
              tab === 'information' || tab === 'launch',
            'detail-panel--saves': tab === 'saves',
            'detail-panel--overview': tab === 'overview',
          }"
          role="tabpanel"
          :aria-labelledby="`tab-${tab}`"
        >
          <GameInformation
            v-if="tab === 'information'"
            :key="game.game_id"
            :game-id="game.game_id"
          />
          <LocalInstallation
            v-else-if="tab === 'launch'"
            :key="game.game_id"
            :game_id="game.game_id"
          />
          <template v-else-if="tab === 'overview'">
            <div class="detail-prose">
              <p class="eyebrow">ABOUT THIS STORY</p>
              <h2>{{ desktop ? '作品简介' : '把未说出口的话，交给风。' }}</h2>
              <p
                v-for="(paragraph, index) in game.description
                  .split('\n')
                  .filter(Boolean)"
                :key="index"
              >
                {{ paragraph }}
              </p>
              <p v-if="!desktop" class="demo-disclosure">
                作品名称、文字及配色均为待审阅的原型方向。没有真实安装实例，也没有远端资料请求。
              </p>
            </div>
            <HikarinagiRatesWall :game-id="game.game_id" /> </template
          ><PlaytimePanel
            v-else-if="tab === 'activity'"
            :game-id="game.game_id"
          /><SavePanel
            v-else-if="tab === 'saves'"
            :key="game.game_id"
            :game-id="game.game_id"
          /><ScreenshotsPanel
            v-else-if="tab === 'screenshots'"
            :game-id="game.game_id"
          /><template v-else>
            <div class="detail-prose">
              <p class="eyebrow">A SPACE FOR FUTURE MEMORIES</p>
              <h2>
                {{
                  tabs.find((item) => item.key === tab)?.label
                }}，将在这里延续。
              </h2>
              <p>
                {{
                  desktop
                    ? '此面板尚未接入，不读取存档或截图；当前只提供顶部的基本进程时长记录。'
                    : '此面板尚未实现。原型不读取真实记录、存档或截图，也不会创建模拟数据库记录。'
                }}
              </p>
              <button
                class="secondary-button"
                @click="
                  showDemoAction(
                    tabs.find((item) => item.key === tab)?.label ?? '此功能',
                  )
                "
              >
                {{ desktop ? '了解功能状态' : '了解演示边界'
                }}<PreviewIcon name="arrow" :size="17" />
              </button>
            </div>
          </template>
        </section>
      </Transition>
      <details v-if="!desktop" class="prototype-tools">
        <summary>原型异常测试 · 不属于正式产品界面</summary>
        <div>
          <button
            class="quiet-button"
            aria-label="演示源卡消失"
            @click="
              preview.show_empty = !preview.show_empty;
              notify(
                preview.show_empty
                  ? '源展廊已切为空库；返回将柔和降级。'
                  : '源展廊已恢复。',
              );
            "
          >
            {{ preview.show_empty ? '恢复源展廊' : '演示源卡消失' }}</button
          ><button
            class="quiet-button"
            aria-label="演示缺失封面"
            @click="preview.missing_cover = !preview.missing_cover"
          >
            {{ preview.missing_cover ? '恢复封面' : '演示缺失封面' }}
          </button>
        </div>
      </details>
    </template>
    <section v-else class="empty-stage">
      <PreviewIcon name="spark" :size="44" />
      <h1 tabindex="-1" data-detail-heading>这个故事，还不在展廊里。</h1>
      <p>
        {{
          desktop
            ? '未找到本地作品，请检查是否已移除库记录或数据库是否可用。'
            : '未找到演示作品。此原型不查询真实数据库或远端服务。'
        }}
      </p>
    </section>
  </div>
</template>
<style scoped>
.detail-panel--workspace {
  background: transparent;
  border: 0;
  box-shadow: none;
  padding: 0;
  backdrop-filter: none;
  display: grid;
  grid-template-columns: minmax(0, 1fr);
}
@media (max-width: 1000px) {
  .detail-panel--overview {
    flex-direction: column;
  }
  .detail-panel--overview :deep(.hikarinagi-wall) {
    width: 100%;
    flex-basis: auto;
  }
}
</style>

<style scoped>
.detail-panel--saves {
  background: transparent;
  border: 0;
  box-shadow: none;
  padding: 0;
  backdrop-filter: none;
  min-width: 0;
}
.detail-panel--saves :deep(input),
.detail-panel--saves :deep(select) {
  min-width: 0;
  max-width: 100%;
}
.detail-panel--saves :deep(p),
.detail-panel--saves :deep(code) {
  overflow-wrap: anywhere;
}
</style>
