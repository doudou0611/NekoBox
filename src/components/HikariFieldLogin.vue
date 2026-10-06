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
import AccountCard from './account/AccountCard.vue';
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
  <AccountCard
    class="hf-account"
    title="HIKARI FIELD"
    eyebrow="YOUR OWNED STORIES"
    icon="spark"
    :connected="hikariField.account.status === 'authenticated'"
    :profile="
      hikariField.account.status === 'authenticated'
        ? {
            name: hikariField.account.profile?.name || 'HIKARI FIELD 用户',
            detail: '已连接 HIKARI FIELD 游戏库',
          }
        : undefined
    "
  >
    <template v-if="hikariField.account.status === 'authenticated'">
      <p class="provider-copy">
        你已拥有的游戏会保留在 NekoBox 中。选择一部故事，下载后即可开始游玩。
      </p>
      <div class="provider-actions hf-actions">
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
      <p v-if="downloading" class="provider-copy">
        下载完成或取消后可退出账户。
      </p>
    </template>
    <form v-else class="provider-form" @submit.prevent="submit">
      <p class="provider-copy">
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
      <label>
        <span id="hikarifield-password-label">密码</span>
        <div class="hf-password">
          <input
            v-model="password"
            aria-labelledby="hikarifield-password-label"
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
        class="primary-button provider-submit"
        :disabled="!desktop || hikariField.busy || downloading"
      >
        <PreviewIcon
          :name="hikariField.busy ? 'refresh' : 'arrow'"
          :size="17"
        />{{ hikariField.busy ? '正在登录并同步…' : '登录并导入已购游戏' }}
      </button>
      <small class="provider-security">
        <PreviewIcon name="lock" :size="14" />
        <span>密码仅用于官方登录，凭证保存在本机系统凭据库。</span>
      </small>
      <p v-if="!desktop" class="provider-copy">请在桌面软件中登录账户。</p>
    </form>
    <Transition name="provider-feedback" mode="out-in"
      ><p
        v-if="hikariField.error"
        key="error"
        class="provider-feedback hf-error"
        role="alert"
      >
        {{ hikariField.error }}
      </p>
      <p
        v-else-if="hikariField.sync_message"
        key="status"
        class="provider-feedback hf-success"
        role="status"
      >
        {{ hikariField.sync_message }}
      </p></Transition
    >
  </AccountCard>
</template>
<style scoped>
.hf-password {
  position: relative;
}
.hf-password input {
  padding-right: 76px;
}
.hf-password button {
  position: absolute;
  right: 8px;
  top: 50%;
  transform: translateY(-50%);
  font-size: 12px;
  min-height: 34px;
}
</style>
