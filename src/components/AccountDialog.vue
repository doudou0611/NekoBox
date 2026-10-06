<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import BangumiLogin from './BangumiLogin.vue';
import HikariFieldLogin from './HikariFieldLogin.vue';
import AccountCard from './account/AccountCard.vue';
import { hikariField, loadHikariField } from '../stores/hikariField';
const accountProviders = ['bangumi', 'hikarinagi', 'hikarifield'] as const;
import PreviewIcon from './preview/PreviewIcon.vue';
import { bangumi } from '../stores/bangumi';
import {
  hikariAccount,
  beginHikariLogin,
  cancelHikariLogin,
  refreshHikariAccount,
  logoutHikariAccount,
} from '../stores/hikarinagiAccount';
import { accountSync, syncAccount } from '../stores/accountSync';
import { desktop } from '../stores/library';
import type { AccountProvider } from '../stores/accountDialog';
const props = withDefaults(
  defineProps<{ initialProvider?: AccountProvider }>(),
  { initialProvider: 'bangumi' },
);
const emit = defineEmits<{ close: [] }>();
const dialog = ref<HTMLDialogElement>();
const account_card = ref<HTMLElement>();
const card_height = ref<number>();
let cardObserver: ResizeObserver | undefined;
function observeCard(element?: HTMLElement) {
  cardObserver?.disconnect();
  if (!element) return;
  const height = element.offsetHeight;
  if (height > 0) card_height.value = Math.ceil(height);
  cardObserver?.observe(element);
}
watch(account_card, observeCard, { flush: 'post' });
const tab = ref<AccountProvider>(props.initialProvider);
watch(
  () => props.initialProvider,
  (provider) => {
    tab.value = provider;
  },
);
const confirm_sync = ref(false);
const current = computed(() =>
  tab.value === 'bangumi' ? bangumi : hikariAccount,
);
let loginWindow: WebviewWindow | null = null;
let disposed = false;
const previous_focus =
  document.activeElement instanceof HTMLElement ? document.activeElement : null;
function keepFocus(event: KeyboardEvent) {
  if (event.key !== 'Tab' || !dialog.value) return;
  const controls = Array.from(
    dialog.value.querySelectorAll<HTMLElement>(
      'button:not(:disabled), input:not(:disabled), a[href], summary, [tabindex="0"]',
    ),
  ).filter((node) => node.tabIndex >= 0 && node.getClientRects().length > 0);
  const first = controls[0];
  const last = controls.at(-1);
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault();
    last?.focus();
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault();
    first?.focus();
  }
}
async function openHikariLogin() {
  const url = await beginHikariLogin();
  if (disposed) {
    await cancelHikariLogin();
    return;
  }
  if (!url) return;
  const flow_id = hikariAccount.flow_id;
  const login = await WebviewWindow.getByLabel(
    `hikarinagi-login-${flow_id}`,
  ).catch(() => null);
  if (!login) {
    await cancelHikariLogin();
    hikariAccount.error = '官方授权窗口不可用，请重新登录。';
    return;
  }
  loginWindow = login;
  void login.once('tauri://error', () => {
    if (hikariAccount.flow_id !== flow_id) return;
    void cancelHikariLogin();
    hikariAccount.error = '无法打开官方登录窗口，请重新尝试。';
  });
  void login.once('tauri://destroyed', () => {
    if (hikariAccount.flow_id === flow_id) void cancelHikariLogin();
  });
}
async function closeLoginWindow() {
  const login = loginWindow;
  loginWindow = null;
  if (login) await login.close().catch(() => {});
}
watch(
  () => hikariAccount.flow_id,
  (id, previous) => {
    if (!id && previous) void closeLoginWindow();
  },
);
watch(tab, () => {
  confirm_sync.value = false;
});
function selectTab(value: AccountProvider) {
  tab.value = value;
}
function onTabKey(event: KeyboardEvent) {
  if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
  event.preventDefault();
  const index = accountProviders.indexOf(tab.value);
  const next =
    event.key === 'Home'
      ? 0
      : event.key === 'End'
        ? 2
        : (index + (event.key === 'ArrowRight' ? 1 : -1) + 3) % 3;
  tab.value = accountProviders[next]!;
  void nextTick(() =>
    document.getElementById(`account-tab-${tab.value}`)?.focus(),
  );
}
function upload() {
  confirm_sync.value = false;
  if (tab.value !== 'hikarifield') void syncAccount(tab.value);
}
onMounted(() => {
  dialog.value?.showModal();
  cardObserver = new ResizeObserver(([entry]) => {
    if (entry && entry.target === account_card.value) {
      card_height.value = (entry.target as HTMLElement).offsetHeight;
    }
  });
  observeCard(account_card.value);
  void refreshHikariAccount();
  void loadHikariField();
});
onUnmounted(() => {
  disposed = true;
  cardObserver?.disconnect();
  void cancelHikariLogin();
  void closeLoginWindow();
  if (previous_focus?.isConnected) previous_focus.focus();
});
</script>
<template>
  <dialog
    ref="dialog"
    class="account-dialog"
    :data-provider="tab"
    aria-labelledby="account-title"
    @keydown="keepFocus"
    @cancel.prevent="emit('close')"
    @close="emit('close')"
  >
    <header class="account-heading">
      <div>
        <p class="eyebrow">CONNECTED ACCOUNTS</p>
        <h2 id="account-title">账户与同步</h2>
        <p class="account-heading-copy">连接你的游戏库，珍藏每一次游玩。</p>
      </div>
      <button
        type="button"
        class="icon-button"
        aria-label="关闭账户窗口"
        @click="emit('close')"
      >
        <PreviewIcon name="close" />
      </button>
    </header>
    <div
      class="account-tabs"
      role="tablist"
      aria-label="资料账户"
      @keydown="onTabKey"
    >
      <span
        class="account-tab-indicator"
        aria-hidden="true"
        :style="{
          transform: `translateX(${accountProviders.indexOf(tab) * 100}%)`,
        }"
      ></span
      ><button
        v-for="provider in accountProviders"
        :id="`account-tab-${provider}`"
        :key="provider"
        role="tab"
        type="button"
        :aria-selected="tab === provider"
        :aria-controls="`account-panel-${provider}`"
        :tabindex="tab === provider ? 0 : -1"
        @click="selectTab(provider)"
      >
        {{
          provider === 'bangumi'
            ? 'Bangumi'
            : provider === 'hikarinagi'
              ? 'Hikarinagi'
              : 'HIKARI FIELD'
        }}<span
          class="account-status-dot"
          :class="{
            connected:
              (provider === 'bangumi'
                ? bangumi
                : provider === 'hikarinagi'
                  ? hikariAccount
                  : hikariField
              ).account.status === 'authenticated',
          }"
        ></span>
      </button>
    </div>
    <div
      class="account-card-stage"
      :style="card_height ? { height: `${card_height}px` } : undefined"
    >
      <Transition name="account-card" mode="out-in">
        <section
          :id="`account-panel-${tab}`"
          ref="account_card"
          :key="tab"
          class="account-card"
          role="tabpanel"
          :aria-labelledby="`account-tab-${tab}`"
        >
          <BangumiLogin
            v-if="tab === 'bangumi'"
            embedded
            :locked="accountSync.busy"
          />
          <HikariFieldLogin v-else-if="tab === 'hikarifield'" />
          <AccountCard
            v-else
            :title="
              hikariAccount.account.profile
                ? 'Hikarinagi 账户'
                : '登录 Hikarinagi'
            "
            eyebrow="YOUR PERSONAL LIBRARY"
            logo="/brand/providers/hikarinagi.png"
            logo-alt="Hikarinagi 官方图标"
            :connected="hikariAccount.account.status === 'authenticated'"
            :profile="
              hikariAccount.account.profile
                ? {
                    name:
                      hikariAccount.account.profile.nickname ||
                      hikariAccount.account.profile.username,
                    detail: `@${hikariAccount.account.profile.username} · ID ${hikariAccount.account.profile.id}`,
                    avatar: hikariAccount.avatar_failed
                      ? null
                      : hikariAccount.account.profile.avatar_url,
                  }
                : undefined
            "
            @avatar-error="hikariAccount.avatar_failed = true"
          >
            <p v-if="hikariAccount.account.profile" class="provider-copy">
              {{ hikariAccount.account.message }}
            </p>
            <p v-if="!hikariAccount.account.profile" class="provider-copy">
              通过官方登录窗口授权自己的账户。登录后，刮削使用此账户的内容偏好，并可上传游玩状态、评分与通关耗时。
            </p>
            <div class="provider-actions">
              <button
                v-if="hikariAccount.account.status !== 'authenticated'"
                type="button"
                class="primary-button"
                :disabled="!desktop || hikariAccount.busy || accountSync.busy"
                @click="openHikariLogin"
              >
                {{
                  hikariAccount.busy ? '等待官方授权…' : '登录 Hikarinagi 账户'
                }}</button
              ><button
                v-if="hikariAccount.flow_id"
                type="button"
                class="quiet-button"
                @click="cancelHikariLogin"
              >
                取消登录</button
              ><template v-if="hikariAccount.account.profile"
                ><button
                  type="button"
                  class="primary-button"
                  :disabled="hikariAccount.busy || accountSync.busy"
                  @click="refreshHikariAccount"
                >
                  <PreviewIcon name="spark" :size="16" />刷新账号</button
                ><button
                  type="button"
                  class="quiet-button"
                  :disabled="hikariAccount.busy || accountSync.busy"
                  @click="logoutHikariAccount"
                >
                  退出登录
                </button></template
              >
            </div>
            <Transition name="provider-feedback" mode="out-in">
              <p
                v-if="hikariAccount.error"
                key="error"
                class="provider-feedback"
                role="alert"
              >
                {{ hikariAccount.error }}
              </p>
              <p
                v-else-if="hikariAccount.login_message"
                key="status"
                class="provider-feedback"
                role="status"
              >
                {{ hikariAccount.login_message }}
              </p>
            </Transition>
            <small class="provider-security">
              <PreviewIcon name="lock" :size="14" />
              <span
                >个人授权保存在本机系统凭据库。未登录也可使用内置应用刮削。</span
              >
            </small>
            <p v-if="!desktop" class="provider-copy">
              浏览器预览无法保存真实账户，请在桌面软件中登录。
            </p>
          </AccountCard>
          <div v-if="tab !== 'hikarifield'" class="account-sync-card">
            <div>
              <strong class="sync-heading"
                ><PreviewIcon name="activity" :size="17" />游玩数据同步</strong
              >
              <p>
                {{
                  tab === 'bangumi'
                    ? '上传游玩状态与评分。Bangumi 不支持耗时字段。'
                    : '上传游玩状态、评分，以及已通关作品的耗时。'
                }}
              </p>
              <small
                >仅同步已绑定此来源的作品；已有短评、细分评分与隐私设置保留。</small
              >
            </div>
            <button
              type="button"
              class="secondary-button"
              :disabled="
                !desktop ||
                current.account.status !== 'authenticated' ||
                current.busy ||
                accountSync.busy
              "
              @click="confirm_sync = true"
            >
              {{
                accountSync.busy && accountSync.provider === tab
                  ? '正在同步…'
                  : '同步游玩数据'
              }}
            </button>
            <div
              v-if="confirm_sync"
              class="sync-confirmation"
              role="group"
              aria-label="确认游玩同步"
            >
              <p>
                将本地状态与评分上传到
                {{
                  tab === 'bangumi' ? 'Bangumi' : 'Hikarinagi'
                }}，覆盖相应字段。新记录默认私密，不上传安装路径。
              </p>
              <div class="provider-actions">
                <button type="button" class="primary-button" @click="upload">
                  确认同步到此账户</button
                ><button
                  type="button"
                  class="quiet-button"
                  @click="confirm_sync = false"
                >
                  取消
                </button>
              </div>
            </div>
            <div
              v-if="accountSync.results[tab]"
              class="sync-result provider-feedback"
              role="status"
            >
              <p>
                成功 {{ accountSync.results[tab]!.synced }} 项 · 失败
                {{ accountSync.results[tab]!.failed }} 项 · 跳过
                {{ accountSync.results[tab]!.skipped }} 项
              </p>
              <details v-if="accountSync.results[tab]!.failures.length">
                <summary>查看失败原因</summary>
                <p
                  v-for="(failure, index) in accountSync.results[tab]!.failures"
                  :key="index"
                >
                  {{ failure.title }}：{{ failure.message }}
                </p>
              </details>
            </div>
            <p
              v-if="accountSync.error && accountSync.error_provider === tab"
              role="alert"
              class="provider-feedback"
            >
              {{ accountSync.error }}
            </p>
          </div>
        </section>
      </Transition>
    </div>
  </dialog>
</template>
<style scoped>
.account-dialog {
  margin: auto;
  width: min(640px, calc(100vw - 32px));
  max-height: calc(100dvh - 48px);
  padding: 32px;
  border: 1px solid var(--border-strong);
  border-radius: 24px;
  background: var(--surface);
  color: var(--text);
  box-shadow: var(--shadow-modal);
  overflow: auto;
  scrollbar-gutter: stable;
}
.account-dialog[open] {
  animation: account-open 420ms var(--ease-standard) both;
}
.account-dialog::backdrop {
  background: rgb(32 27 45 / 40%);
  backdrop-filter: blur(10px);
}
.account-heading {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 26px;
}
.account-heading h2 {
  margin: 8px 0 0;
  font-size: 28px;
  font-weight: 500;
  letter-spacing: -0.025em;
}
.account-heading-copy {
  font-size: 12px;
  color: var(--muted);
  line-height: 1.8;
  margin-top: 9px;
}
.account-heading .icon-button {
  width: 38px;
  height: 38px;
  flex-shrink: 0;
  border-radius: 12px;
}
.account-tabs {
  position: relative;
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  padding: 6px;
  border: 1px solid var(--border);
  background: color-mix(in srgb, var(--surface-hover) 75%, var(--surface));
  border-radius: 14px;
  margin-bottom: 28px;
}
.account-tabs button {
  position: relative;
  z-index: 1;
  display: flex;
  gap: 9px;
  align-items: center;
  justify-content: center;
  border: 0;
  padding: 12px 6px;
  min-height: 44px;
  border-radius: 9px;
  font-size: 12px;
  background: transparent;
  color: var(--muted);
  cursor: pointer;
  font-weight: 600;
  transition: color 220ms ease;
}
.account-tabs button:hover,
.account-tabs button[aria-selected='true'] {
  color: var(--accent-ink, var(--accent));
}
.account-tabs button:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}
.account-tab-indicator {
  position: absolute;
  left: 6px;
  top: 6px;
  width: calc((100% - 12px) / 3);
  height: calc(100% - 12px);
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 9px;
  box-shadow: var(--shadow-subtle);
  transition: transform 360ms var(--ease-standard);
}
.account-status-dot {
  flex: 0 0 6px;
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--border-strong);
  transition:
    background 220ms ease,
    box-shadow 220ms ease;
}
.account-status-dot.connected {
  background: var(--accent);
  box-shadow: 0 0 0 3px var(--accent-wash);
}
.account-card-stage {
  position: relative;
  transition: height 380ms var(--ease-standard);
}
.account-card {
  display: flow-root;
  min-width: 0;
  padding-bottom: 2px;
}
.account-sync-card {
  margin-top: 26px;
  padding: 20px;
  border: 1px solid var(--border);
  border-radius: 14px;
  background: color-mix(in srgb, var(--surface-hover) 35%, var(--surface));
}
.sync-heading {
  display: flex;
  align-items: center;
  gap: 9px;
  font-size: 13px;
  font-weight: 600;
}
.sync-heading svg {
  color: var(--accent-ink, var(--accent));
}
.account-sync-card > div > p {
  color: var(--muted);
  font-size: 12px;
  line-height: 1.85;
  margin: 10px 0;
}
.account-sync-card small {
  color: var(--muted);
  display: block;
  font-size: 11px;
  line-height: 1.8;
  margin-bottom: 18px;
}
.account-sync-card > .secondary-button {
  min-height: 44px;
  border-radius: 12px;
  font-size: 12px;
}
.sync-confirmation {
  margin-top: 20px;
  padding-top: 16px;
  border-top: 1px solid var(--border-strong);
}
.sync-confirmation > p {
  margin: 0 0 16px !important;
}
.account-sync-card .provider-feedback > p {
  margin: 0;
  color: inherit;
  font-size: 13px;
}
.account-card-enter-active {
  transition:
    opacity 260ms ease,
    transform 360ms var(--ease-standard);
}
.account-card-leave-active {
  transition:
    opacity 120ms ease,
    transform 160ms ease;
}
.account-card-enter-from {
  opacity: 0;
  transform: translateY(9px);
}
.account-card-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}
@keyframes account-open {
  from {
    opacity: 0;
    transform: translateY(12px) scale(0.985);
  }
  to {
    opacity: 1;
    transform: none;
  }
}
:global(:root[data-motion='light'] .account-card-enter-active),
:global(:root[data-motion='light'] .account-card-leave-active) {
  transition: opacity 120ms ease;
}
:global(:root[data-motion='light'] .account-card-enter-from),
:global(:root[data-motion='light'] .account-card-leave-to),
:global(:root[data-motion='reduced'] .account-card-enter-from),
:global(:root[data-motion='reduced'] .account-card-leave-to) {
  transform: none;
}
:global(:root[data-motion='light'] .account-card-stage) {
  transition-duration: 220ms;
}
:global(:root[data-motion='light'] .account-dialog[open]) {
  animation: none;
}
:global(:root[data-motion='reduced'] .account-card-enter-active),
:global(:root[data-motion='reduced'] .account-card-leave-active),
:global(:root[data-motion='reduced'] .account-tab-indicator),
:global(:root[data-motion='reduced'] .account-card-stage),
:global(:root[data-motion='reduced'] .account-tabs button),
:global(:root[data-motion='reduced'] .account-status-dot) {
  transition: none;
}
:global(:root[data-motion='reduced'] .account-dialog[open]) {
  animation: none;
}
@media (prefers-reduced-motion: reduce) {
  .account-card-enter-active,
  .account-card-leave-active,
  .account-tab-indicator,
  .account-card-stage,
  .account-tabs button,
  .account-status-dot {
    transition: none;
  }
  .account-card-enter-from,
  .account-card-leave-to {
    transform: none;
  }
  .account-dialog[open] {
    animation: none;
  }
}
@media (max-width: 480px) {
  .account-dialog {
    padding: 22px;
    border-radius: 20px;
  }
  .account-heading h2 {
    font-size: 25px;
  }
  .account-tabs button {
    font-size: 11px;
    gap: 6px;
    padding-inline: 2px;
    white-space: nowrap;
  }
  .account-sync-card {
    padding: 16px;
  }
}
</style>
