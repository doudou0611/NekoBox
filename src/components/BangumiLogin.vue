<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue';
import { isTauri } from '@tauri-apps/api/core';
import { api, errorText } from '../stores/library';
import {
  bangumi,
  loginBangumi,
  logoutBangumi,
  refreshBangumi,
} from '../stores/bangumi';
import AccountCard from './account/AccountCard.vue';
import PreviewIcon from './preview/PreviewIcon.vue';
const props = defineProps<{ embedded?: boolean; locked?: boolean }>();

const emit = defineEmits<{ close: [] }>();
const error = ref('');
const token = ref('');
const token_input = ref<HTMLInputElement | null>(null);
const previous_focus =
  document.activeElement instanceof HTMLElement ? document.activeElement : null;
async function signIn() {
  const credential = token.value;
  token.value = '';
  await loginBangumi(credential);
}

function onKeydown(event: KeyboardEvent) {
  if (!props.embedded && event.key === 'Escape') emit('close');
}

function openLogin() {
  error.value = '';
  if (!isTauri()) {
    window.open(
      'https://next.bgm.tv/demo/access-token',
      '_blank',
      'noopener,noreferrer',
    );
    return;
  }
  void api('open_bangumi_login', {}).catch((e) => (error.value = errorText(e)));
}

onMounted(() => {
  window.addEventListener('keydown', onKeydown);
  if (!props.embedded) token_input.value?.focus();
});
onUnmounted(() => {
  window.removeEventListener('keydown', onKeydown);
  token.value = '';
  if (!props.embedded) previous_focus?.focus();
});
</script>

<template>
  <div
    :class="props.embedded ? 'embedded-bangumi' : 'bangumi-login-backdrop'"
    @click.self="!props.embedded && emit('close')"
  >
    <section
      class="bangumi-login-dialog"
      :role="props.embedded ? undefined : 'dialog'"
      :aria-modal="props.embedded ? undefined : true"
      aria-labelledby="bangumi-login-title"
    >
      <button
        v-if="!props.embedded"
        class="bangumi-login-close icon-button"
        type="button"
        aria-label="关闭 Bangumi 登录"
        @click="emit('close')"
      >
        <PreviewIcon name="close" />
      </button>
      <AccountCard
        :title="bangumi.account.profile ? 'Bangumi 账号' : '登录 Bangumi'"
        title-id="bangumi-login-title"
        eyebrow="YOUR STORY COLLECTION"
        logo="/brand/providers/bangumi.png"
        logo-alt="Bangumi 官方图标"
        :connected="bangumi.account.status === 'authenticated'"
        :profile="
          bangumi.account.profile
            ? {
                name:
                  bangumi.account.profile.nickname ||
                  bangumi.account.profile.username,
                detail: `@${bangumi.account.profile.username} · ID ${bangumi.account.profile.id}`,
                avatar: bangumi.avatar_failed
                  ? null
                  : bangumi.account.profile.avatar_url,
              }
            : undefined
        "
        @avatar-error="bangumi.avatar_failed = true"
      >
        <template v-if="bangumi.account.profile">
          <p class="provider-copy">{{ bangumi.account.message }}</p>
          <div class="provider-actions">
            <button
              class="primary-button"
              :disabled="bangumi.busy || props.locked"
              @click="refreshBangumi"
            >
              <PreviewIcon name="spark" :size="16" />刷新账号
            </button>
            <button
              class="quiet-button"
              :disabled="bangumi.busy || props.locked"
              @click="logoutBangumi"
            >
              退出登录
            </button>
          </div>
        </template>
        <form
          v-if="
            !bangumi.account.profile || bangumi.account.status === 'expired'
          "
          class="provider-form"
          :class="{ 'bangumi-reauthorize': bangumi.account.profile }"
          @submit.prevent="signIn"
        >
          <p class="provider-copy">
            在 Bangumi 官方页面生成 Access
            Token，粘贴一次即可保存授权并同步头像。
          </p>
          <div class="provider-actions">
            <button class="secondary-button" type="button" @click="openLogin">
              打开官方 Token 页面<PreviewIcon name="arrow" :size="16" />
            </button>
          </div>
          <label for="bangumi-token"
            >Access Token
            <input
              id="bangumi-token"
              ref="token_input"
              v-model="token"
              type="password"
              autocomplete="off"
              spellcheck="false"
              maxlength="4096"
              placeholder="粘贴官方生成的 Token"
              :disabled="bangumi.busy || props.locked || !isTauri()"
            />
          </label>
          <button
            class="primary-button provider-submit"
            type="submit"
            :disabled="
              bangumi.busy || props.locked || !token.trim() || !isTauri()
            "
          >
            <PreviewIcon name="arrow" :size="17" />{{
              bangumi.busy ? '验证并保存…' : '登录并保存授权'
            }}
          </button>
          <small class="provider-security">
            <PreviewIcon name="lock" :size="14" />
            <span
              >凭证保存在本机系统凭据库。在官网登录后，还需将 Token
              粘贴到这里。</span
            >
          </small>
          <p v-if="!isTauri()" class="provider-copy">
            浏览器预览不能保存桌面授权，请在桌面软件中登录。
          </p>
        </form>
        <Transition name="provider-feedback" mode="out-in">
          <p
            v-if="bangumi.error || error"
            class="provider-feedback"
            role="alert"
          >
            {{ bangumi.error || error }}
          </p>
        </Transition>
      </AccountCard>
    </section>
  </div>
</template>
<style scoped>
.embedded-bangumi .bangumi-login-dialog {
  width: 100%;
  padding: 0;
  border: 0;
  background: transparent;
  box-shadow: none;
}
.bangumi-login-backdrop {
  position: fixed;
  z-index: var(--z-modal);
  inset: 0;
  display: grid;
  place-items: center;
  padding: 24px;
  background: rgb(32 27 45 / 36%);
  backdrop-filter: blur(8px);
}
.bangumi-login-dialog {
  position: relative;
  width: min(460px, 100%);
  max-height: calc(100dvh - 48px);
  overflow: auto;
  padding: 28px;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-lg);
  background: var(--surface);
  box-shadow: var(--shadow-modal);
}
.bangumi-login-close {
  position: absolute;
  top: 16px;
  right: 16px;
}
.bangumi-reauthorize {
  margin-top: 24px;
}
</style>
