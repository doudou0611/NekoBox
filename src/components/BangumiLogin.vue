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
  token_input.value?.focus();
});
onUnmounted(() => {
  window.removeEventListener('keydown', onKeydown);
  token.value = '';
  previous_focus?.focus();
});
</script>

<template>
  <div
    :class="props.embedded ? 'embedded-bangumi' : 'bangumi-login-backdrop'"
    @click.self="!props.embedded && emit('close')"
  >
    <section
      class="bangumi-login-dialog"
      :role="props.embedded ? 'region' : 'dialog'"
      :aria-modal="props.embedded ? undefined : true"
      aria-labelledby="bangumi-login-title"
    >
      <button
        v-if="!props.embedded"
        class="bangumi-login-close"
        type="button"
        aria-label="关闭 Bangumi 登录"
        @click="emit('close')"
      >
        ×
      </button>
      <p v-if="!props.embedded" class="eyebrow">BANGUMI ACCOUNT</p>
      <h2 id="bangumi-login-title">
        {{ bangumi.account.profile ? 'Bangumi 账号' : '登录 Bangumi' }}
      </h2>
      <template v-if="bangumi.account.profile">
        <div class="account-profile">
          <img
            v-if="bangumi.account.profile.avatar_url && !bangumi.avatar_failed"
            :src="bangumi.account.profile.avatar_url"
            alt="Bangumi 头像"
            @error="bangumi.avatar_failed = true"
          />
          <div>
            <strong>{{
              bangumi.account.profile.nickname ||
              bangumi.account.profile.username
            }}</strong
            ><small
              >@{{ bangumi.account.profile.username }} · ID
              {{ bangumi.account.profile.id }}</small
            >
          </div>
        </div>
        <p>{{ bangumi.account.message }}</p>
        <div class="account-actions">
          <button
            class="secondary-button"
            :disabled="bangumi.busy || props.locked"
            @click="refreshBangumi"
          >
            刷新账号
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
        v-if="!bangumi.account.profile || bangumi.account.status === 'expired'"
        @submit.prevent="signIn"
      >
        <p>
          在 Bangumi 官方页面生成 Access Token，粘贴一次即可保存授权并同步头像。
        </p>
        <button class="secondary-button" type="button" @click="openLogin">
          打开官方 Token 页面
        </button>
        <label for="bangumi-token">Access Token</label>
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
        <button
          class="primary-button"
          type="submit"
          :disabled="
            bangumi.busy || props.locked || !token.trim() || !isTauri()
          "
        >
          {{ bangumi.busy ? '验证并保存…' : '登录并保存授权' }}
        </button>
        <small
          >凭证保存在本机系统凭据库，后续刮削自动使用。仅在官网登录后，还需将
          Token 粘贴到这里。</small
        >
        <small v-if="!isTauri()"
          >浏览器预览不能保存桌面授权，请在 Windows 软件中登录。</small
        >
      </form>
      <p v-if="bangumi.error" role="alert">{{ bangumi.error }}</p>
      <p v-if="error" role="alert">{{ error }}</p>
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
.embedded-bangumi .bangumi-login-dialog h2 {
  font-size: 22px;
  margin: 0 0 20px;
}
.bangumi-login-backdrop {
  position: fixed;
  z-index: 20;
  inset: 0;
  display: grid;
  place-items: center;
  padding: var(--space-24);
  background: rgb(32 27 45 / 36%);
  backdrop-filter: blur(8px);
}
.bangumi-login-dialog {
  position: relative;
  width: min(420px, 100%);
  padding: var(--space-32);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-lg);
  background: var(--surface);
  box-shadow: 0 28px 80px rgb(35 25 48 / 24%);
}
.bangumi-login-dialog h2 {
  margin: var(--space-8) 0 var(--space-12);
}
.bangumi-login-dialog p:not(.eyebrow) {
  margin: 0 0 var(--space-20);
  color: var(--muted);
  line-height: 1.7;
}
.bangumi-login-dialog small {
  display: block;
  margin-top: var(--space-16);
  color: var(--muted);
  line-height: 1.6;
}
.bangumi-login-close {
  position: absolute;
  top: var(--space-12);
  right: var(--space-16);
  border: 0;
  background: transparent;
  color: var(--muted);
  font-size: 24px;
  cursor: pointer;
}
.account-profile,
.account-actions {
  display: flex;
  align-items: center;
  gap: 16px;
  margin-bottom: 20px;
}
.account-profile img {
  width: 64px;
  height: 64px;
  border-radius: 50%;
  object-fit: cover;
}
.account-profile small {
  margin-top: 4px;
}
form label {
  display: block;
  margin-top: 20px;
  color: var(--muted);
}
form input {
  box-sizing: border-box;
  width: 100%;
  margin: 8px 0 16px;
  padding: 12px;
  background: var(--surface-hover);
  color: var(--text);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
}
</style>
