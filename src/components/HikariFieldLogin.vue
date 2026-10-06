<script setup lang="ts">
import { computed, ref } from 'vue';
import { desktop } from '../stores/library';
import {
  hikariField,
  loginHikariField,
  logoutHikariField,
  refreshOwnedHikariField,
} from '../stores/hikariField';
import PreviewIcon from './preview/PreviewIcon.vue';
const email = ref('');
const password = ref('');
const reveal = ref(false);
const downloading = computed(() =>
  hikariField.tasks.some((t) => ['queued', 'running'].includes(t.status)),
);
async function submit() {
  const secret = password.value;
  password.value = '';
  reveal.value = false;
  await loginHikariField(email.value.trim(), secret);
}
</script>
<template>
  <div class="hf-account">
    <div class="hf-intro">
      <span class="hf-emblem"><PreviewIcon name="spark" :size="26" /></span>
      <div>
        <p>YOUR OWNED STORIES</p>
        <h3>HIKARI FIELD</h3>
      </div>
    </div>
    <template v-if="hikariField.account.status === 'authenticated'">
      <div class="hf-profile">
        <span>{{ hikariField.account.profile?.name.slice(0, 1) || 'H' }}</span>
        <div>
          <strong>{{ hikariField.account.profile?.name }}</strong
          ><small>已连接 HIKARI FIELD 游戏库</small>
        </div>
        <PreviewIcon name="check" :size="20" />
      </div>
      <p class="hf-copy">
        你已拥有的游戏会保留在 NekoBox 中。选择一部故事，下载后即可开始游玩。
      </p>
      <div class="hf-actions">
        <button
          class="primary-button"
          :disabled="hikariField.busy"
          @click="refreshOwnedHikariField"
        >
          {{ hikariField.busy ? '正在同步游戏库…' : '同步已购游戏' }}</button
        ><button
          class="quiet-button"
          :disabled="hikariField.busy || downloading"
          @click="logoutHikariField"
        >
          退出登录
        </button>
      </div>
      <p v-if="downloading" class="hf-copy">下载完成或取消后可退出账户。</p>
    </template>
    <form v-else class="hf-form" @submit.prevent="submit">
      <p class="hf-copy">
        使用商城账户登录，自动将已拥有的正式版游戏加入你的游戏库。
      </p>
      <label
        >邮箱<input
          v-model="email"
          type="email"
          autocomplete="username"
          placeholder="你的 HIKARI FIELD 邮箱"
          maxlength="320"
          required
          :disabled="!desktop || hikariField.busy"
      /></label>
      <label
        >密码
        <div class="hf-password">
          <input
            v-model="password"
            :type="reveal ? 'text' : 'password'"
            autocomplete="current-password"
            placeholder="商城账户密码"
            maxlength="4096"
            required
            :disabled="!desktop || hikariField.busy"
          /><button
            type="button"
            class="quiet-button"
            :aria-label="reveal ? '隐藏密码' : '显示密码'"
            :aria-pressed="reveal"
            @click="reveal = !reveal"
          >
            {{ reveal ? '隐藏' : '显示' }}
          </button>
        </div></label
      >
      <button
        class="primary-button hf-submit"
        :disabled="!desktop || hikariField.busy || downloading"
      >
        <PreviewIcon
          :name="hikariField.busy ? 'refresh' : 'arrow'"
          :size="17"
        />{{ hikariField.busy ? '正在登录并同步…' : '登录并导入已购游戏' }}
      </button>
      <small class="hf-security"
        >密码仅用于官方登录，凭证保存在本机系统凭据库。</small
      >
      <p v-if="!desktop" class="hf-copy">请在桌面软件中登录账户。</p>
    </form>
    <Transition name="hf-feedback" mode="out-in"
      ><p v-if="hikariField.error" key="error" class="hf-error" role="alert">
        {{ hikariField.error }}
      </p>
      <p
        v-else-if="hikariField.sync_message"
        key="status"
        class="hf-success"
        role="status"
      >
        {{ hikariField.sync_message }}
      </p></Transition
    >
  </div>
</template>
<style scoped>
.hf-intro {
  display: flex;
  gap: 16px;
  align-items: center;
  margin-bottom: 24px;
}
.hf-emblem {
  display: grid;
  place-items: center;
  width: 54px;
  height: 54px;
  border-radius: 18px;
  background: var(--accent-wash);
  color: var(--accent);
}
.hf-intro p {
  margin: 0 0 6px;
  font-size: 10px;
  letter-spacing: 0.18em;
  color: var(--muted);
}
.hf-intro h3 {
  margin: 0;
  font-size: 22px;
  letter-spacing: 0.02em;
}
.hf-copy {
  color: var(--muted);
  font-size: 13px;
  line-height: 1.8;
  margin: 0 0 20px;
}
.hf-form {
  display: grid;
  gap: 18px;
}
.hf-form .hf-copy {
  margin: 0;
}
.hf-form label {
  display: grid;
  gap: 8px;
  font-size: 13px;
  font-weight: 600;
}
.hf-form input {
  width: 100%;
  min-width: 0;
  box-sizing: border-box;
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-sm);
  background: var(--surface-hover);
  color: var(--text);
  padding: 13px 14px;
  outline: none;
  transition:
    border-color 180ms,
    box-shadow 180ms;
}
.hf-form input:focus {
  border-color: var(--accent);
  box-shadow: 0 0 0 3px var(--accent-wash);
}
.hf-password {
  position: relative;
}
.hf-password input {
  padding-right: 66px;
}
.hf-password button {
  position: absolute;
  right: 8px;
  top: 50%;
  transform: translateY(-50%);
  font-size: 12px;
}
.hf-submit {
  justify-content: center;
  width: 100%;
  min-height: 46px;
  gap: 10px;
  margin-top: 2px;
}
.hf-security {
  font-size: 11px;
  color: var(--muted);
  text-align: center;
  line-height: 1.7;
}
.hf-profile {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 18px;
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  background: var(--accent-wash);
  margin-bottom: 20px;
}
.hf-profile > span {
  display: grid;
  place-items: center;
  background: var(--surface);
  width: 44px;
  height: 44px;
  border-radius: 50%;
  font-size: 20px;
  color: var(--accent);
}
.hf-profile > div {
  flex: 1;
  min-width: 0;
}
.hf-profile strong {
  overflow-wrap: anywhere;
}
.hf-profile small {
  display: block;
  color: var(--muted);
  margin-top: 6px;
}
.hf-profile > svg {
  color: var(--accent);
}
.hf-actions {
  display: flex;
  gap: 12px;
  flex-wrap: wrap;
}
.hf-error,
.hf-success {
  margin: var(--space-20) 0 0;
  padding: 12px 14px;
  border-radius: var(--radius-sm);
  font-size: 13px;
  line-height: 1.7;
}
.hf-error {
  color: var(--text);
  background: var(--surface-hover);
}
.hf-success {
  color: var(--accent-ink, var(--accent));
  background: var(--accent-wash);
}
.hf-feedback-enter-active,
.hf-feedback-leave-active {
  transition: opacity 160ms ease;
}
.hf-feedback-enter-from,
.hf-feedback-leave-to {
  opacity: 0;
}
:global(:root[data-motion='reduced'] .hf-feedback-enter-active),
:global(:root[data-motion='reduced'] .hf-feedback-leave-active) {
  transition: none;
}
@media (prefers-reduced-motion: reduce) {
  * {
    transition: none !important;
  }
}
</style>
